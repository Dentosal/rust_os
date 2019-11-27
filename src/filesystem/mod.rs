use alloc::prelude::v1::*;
use hashbrown::HashMap;
use lazy_static::lazy_static;
use serde::Serialize;
use spin::Mutex;

use d7abi::fs::{FileDescriptor, FileInfo};

use crate::memory::MemoryController;
use crate::multitasking::{
    process::{ProcessId, ProcessResult},
    ElfImage, Scheduler,
};

pub mod error;
pub mod file;
mod node;
mod path;
pub mod staticfs;

use self::error::{ErrorCode, IoError, IoResult};
use self::file::*;

pub use self::node::*;
pub use self::path::{Path, PathBuf};

const ROOT_ID: NodeId = NodeId::first();

/// Globally unique file descriptor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileClientId {
    /// `None` here repensents that the operation
    /// was initiated by the kernel, and not any process.
    pub process: Option<ProcessId>,
    /// Per-process file descriptor
    pub fd: FileDescriptor,
}
impl FileClientId {
    fn kernel(fd: FileDescriptor) -> Self {
        Self { process: None, fd }
    }

    pub fn process(pid: ProcessId, fd: FileDescriptor) -> Self {
        Self {
            process: Some(pid),
            fd,
        }
    }

    pub fn is_kernel(&self) -> bool {
        self.process.is_none()
    }
}

/// NodeId and descriptors for a process
struct ProcessDescriptors {
    /// Node id of the process
    node_id: NodeId,
    /// Descriptors owned by the process
    descriptors: HashMap<FileDescriptor, NodeId>,
    /// Next available file descriptor
    next_fd: FileDescriptor,
}
impl ProcessDescriptors {
    fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            descriptors: HashMap::new(),
            next_fd: unsafe { FileDescriptor::from_u64(0) },
        }
    }

    fn resolve(&self, fd: FileDescriptor) -> Option<NodeId> {
        self.descriptors.get(&fd).copied()
    }

    fn create_id(&mut self, node_id: NodeId) -> FileDescriptor {
        let fd = self.next_fd;
        self.descriptors.insert(fd, node_id);
        self.next_fd = unsafe { self.next_fd.next() };
        fd
    }
}

/// Read all bytes. Used when the kernel needs to read all data.
/// # Safety
/// This is marked unsafe, as it discards data if an io error occurs during it.
unsafe fn read_to_end(file: &mut dyn FileOps, fc: FileClientId) -> IoResult<Vec<u8>> {
    const IO_BUFFER_SIZE: usize = 1024;

    let mut result = Vec::new();
    let mut buffer = [0u8; IO_BUFFER_SIZE];
    loop {
        let count = file.read(fc, &mut buffer)?;
        if count == 0 {
            // EOF
            break;
        }
        result.extend(buffer.iter());
    }
    Ok(result)
}

/// Write all bytes. Used when the kernel needs to write a fixed amount of data.
/// # Safety
/// This is marked unsafe, as it discards data if an io error occurs during it.
unsafe fn write_all(file: &mut dyn FileOps, fc: FileClientId, data: &[u8]) -> IoResult<()> {
    const IO_BUFFER_SIZE: usize = 1024;

    let mut offset: usize = 0;
    while offset < data.len() {
        let count = file.write(fc, &data[offset..])?;
        offset += count;
        if count == 0 {
            panic!("Write failed");
        }
    }
    Ok(())
}

/// Serialize with Pinecone, and write all bytes
unsafe fn write_all_ser<T: Serialize>(
    file: &mut dyn FileOps, fc: FileClientId, data: &T,
) -> IoResult<()> {
    let data = pinecone::to_vec(data).unwrap();
    write_all(file, fc, &data)
}

pub struct VirtualFS {
    nodes: HashMap<NodeId, Node>,
    descriptors: HashMap<ProcessId, ProcessDescriptors>,
    next_node_id: NodeId,
    next_kernel_fd: FileDescriptor,
}
impl VirtualFS {
    pub fn new() -> Self {
        let mut nodes = HashMap::new();
        let mut root_node = Node::new(Box::new(InternalBranch::new()));
        root_node.inc_ref(); // Root node refers to "itself", and will not be removed
        nodes.insert(ROOT_ID, root_node);
        Self {
            nodes,
            descriptors: HashMap::new(),
            next_node_id: ROOT_ID.next(),
            next_kernel_fd: unsafe { FileDescriptor::from_u64(0) },
        }
    }

    pub fn take_kernel_fc(&mut self) -> FileClientId {
        let fd = self.next_kernel_fd;
        self.next_kernel_fd = unsafe { self.next_kernel_fd.next() };
        FileClientId::kernel(fd)
    }

    /// Traverse tree and return NodeId
    pub fn resolve(&mut self, path: Path) -> IoResult<NodeId> {
        assert!(path.is_absolute());
        let mut cursor: NodeId = ROOT_ID;
        for c in path.components() {
            cursor = self.get_child(cursor, c)?;
        }
        Ok(cursor)
    }

    pub fn node(&self, id: NodeId) -> &Node {
        self.nodes.get(&id).unwrap()
    }

    pub fn node_mut(&mut self, id: NodeId) -> &mut Node {
        self.nodes.get_mut(&id).unwrap()
    }

    /// Create a new node
    pub fn create_node(&mut self, path: Path, new_node: Node) -> IoResult<NodeId> {
        assert!(!path.is_root());
        assert!(path.is_absolute());

        let file_name = path.file_name().expect("File name missing");
        let parent = path.parent().expect("Parent missing");
        let parent_node_id = self.resolve(parent)?;
        let new_node_id = self.next_node_id;
        if self.has_child(parent_node_id, &(*file_name).to_owned())? {
            Err(IoError::Code(ErrorCode::fs_node_exists))
        } else if self.node(parent_node_id).leafness() == Leafness::Leaf {
            Err(IoError::Code(ErrorCode::fs_node_is_leaf))
        } else {
            self.add_child(parent_node_id, file_name, new_node_id)?;
            self.nodes.insert(new_node_id, new_node);
            self.next_node_id = new_node_id.next();
            Ok(new_node_id)
        }
    }

    /// Create a new node, and give it one initial reference so it
    /// will not be removed automatically
    fn create_static(&mut self, path: Path, dev: Box<dyn FileOps>) -> IoResult<NodeId> {
        let id = self.create_node(path.clone(), Node::new(dev))?;
        self.node_mut(id).inc_ref();
        Ok(id)
    }

    /// Create a new branch node, and give it one initial reference so it
    /// will not be removed automatically
    pub fn create_static_branch(&mut self, path: Path) -> IoResult<NodeId> {
        self.create_static(path, Box::new(InternalBranch::new()))
    }

    /// File info (system call)
    pub fn fileinfo(&mut self, path: &str) -> IoResult<FileInfo> {
        let path = Path::new(path);
        let id = self.resolve(path)?;
        Ok(self.node(id).fileinfo())
    }

    pub fn get_childrem(&mut self, node_id: NodeId) -> IoResult<HashMap<String, NodeId>> {
        match self.node(node_id).leafness() {
            Leafness::Leaf => Err(IoError::Code(ErrorCode::fs_node_is_leaf)),
            Leafness::Branch => {
                // Use standard `ReadBranch` protocol.
                let fc = self.take_kernel_fc();
                let node = self.node_mut(node_id);
                node.open(fc)?;
                let bytes = unsafe { read_to_end(&mut *node.data, fc)? };
                unimplemented!()
            },
            Leafness::InternalBranch => {
                // Use internal branch protocol.
                let fc = self.take_kernel_fc();
                let node = self.node_mut(node_id);
                node.open(fc)?;
                let result = unsafe { read_to_end(&mut *node.data, fc) };
                let keep = node.close(fc);
                assert!(keep, "Not possible");
                Ok(pinecone::from_bytes(&result?).unwrap())
            },
        }
    }

    pub fn get_child(&mut self, node_id: NodeId, name: &str) -> IoResult<NodeId> {
        self.get_childrem(node_id)?
            .get(name)
            .copied()
            .ok_or(IoError::Code(ErrorCode::fs_node_not_found))
    }

    pub fn has_child(&mut self, node_id: NodeId, name: &str) -> IoResult<bool> {
        match self.get_child(node_id, name) {
            Ok(_) => Ok(true),
            Err(IoError::Code(ErrorCode::fs_node_not_found)) => Ok(false),
            Err(other) => Err(other),
        }
    }

    /// Adds a child. This does not work with non-internal branches;
    /// they must be written by userspace processes only.
    fn add_child(&mut self, parent_id: NodeId, child_name: &str, child_id: NodeId) -> IoResult<()> {
        match self.node(parent_id).leafness() {
            Leafness::Leaf => Err(IoError::Code(ErrorCode::fs_node_is_leaf)),
            Leafness::Branch => panic!("add_child only supports internal branches"),
            Leafness::InternalBranch => {
                // Use internal branch protocol.
                let fc = self.take_kernel_fc();
                let node = self.node_mut(parent_id);
                node.open(fc)?;
                let data = (child_name, child_id);
                let result =
                    unsafe { write_all(&mut *node.data, fc, &pinecone::to_vec(&data).unwrap()) };
                let keep = node.close(fc);
                assert!(keep, "Not possible");
                result
            },
        }
    }

    /// Get process descriptors
    fn process(&self, pid: ProcessId) -> &ProcessDescriptors {
        self.descriptors.get(&pid).expect("No such process")
    }

    /// Get process descriptors mutably
    fn process_mut(&mut self, pid: ProcessId) -> &mut ProcessDescriptors {
        self.descriptors.get_mut(&pid).expect("No such process")
    }

    /// Open a file (system call)
    pub fn open(&mut self, path: &str, pid: ProcessId) -> IoResult<FileClientId> {
        let path = Path::new(path);
        let node_id = self.resolve(path)?;
        let process = self.process_mut(pid);
        let fd = process.create_id(node_id);
        let fc = FileClientId::process(pid, fd);
        self.node_mut(node_id).open(fc)?;
        Ok(fc)
    }

    /// Exec a file (system call)
    pub fn exec(
        &mut self, mem_ctrl: &mut MemoryController, sched: &mut Scheduler, path: &str,
        owner_pid: ProcessId,
    ) -> IoResult<FileClientId>
    {
        self.exec_optional_owner(mem_ctrl, sched, path, Some(owner_pid))
    }

    /// Exec a file without a parent process, i.e. init
    pub fn kernel_exec(
        &mut self, mem_ctrl: &mut MemoryController, sched: &mut Scheduler, path: &str,
    ) -> IoResult<FileClientId> {
        self.exec_optional_owner(mem_ctrl, sched, path, None)
    }

    fn exec_optional_owner(
        &mut self, mem_ctrl: &mut MemoryController, sched: &mut Scheduler, path: &str,
        owner_pid: Option<ProcessId>,
    ) -> IoResult<FileClientId>
    {
        // Load elf image
        let elfimage = self.load_module(mem_ctrl, path)?;

        // Open the executable file for the owner of the process
        let path = Path::new(path);
        let owner_node_id = self.resolve(path)?;
        let fc = {
            if let Some(pid) = owner_pid {
                let process = self.process_mut(pid);
                let fd = process.create_id(owner_node_id);
                FileClientId::process(pid, fd)
            } else {
                self.take_kernel_fc()
            }
        };
        self.node_mut(owner_node_id).inc_ref();

        // Spawn the new process
        let new_pid = unsafe { sched.spawn(mem_ctrl, elfimage) };

        // Create a node and descriptors for the process
        let node_id = self.create_node(
            Path::new(&format!("/prc/{}", new_pid)),
            Node::new(Box::new(ProcessFile::new(new_pid, fc))),
        )?;
        self.descriptors
            .insert(new_pid, ProcessDescriptors::new(node_id));

        // Open a file descriptor to the process for the owner
        let fc = {
            if let Some(pid) = owner_pid {
                let process = self.process_mut(pid);
                let fd = process.create_id(node_id);
                FileClientId::process(pid, fd)
            } else {
                self.take_kernel_fc()
            }
        };
        self.node_mut(node_id).open(fc).unwrap();
        Ok(fc)
    }

    /// Loads elf image to ram and returns it
    pub fn load_module(
        &mut self, mem_ctrl: &mut MemoryController, path: &str,
    ) -> IoResult<ElfImage> {
        use core::ptr;
        use x86_64::structures::paging::PageTableFlags as Flags;

        use crate::memory::prelude::*;
        use crate::memory::Area;
        use crate::memory::{self, Page};

        let bytes = self.read_file(path)?;

        let size_pages =
            memory::page_align(PhysAddr::new(bytes.len() as u64), true).as_u64() / Page::SIZE;

        // Allocate load buffer
        let area = mem_ctrl.alloc_pages(size_pages as usize, Flags::PRESENT | Flags::WRITABLE);

        // Store the file to buffer
        let base: *mut u8 = area.start.as_mut_ptr();
        let mut it = bytes.into_iter();
        for page_offset in 0..size_pages {
            for byte_offset in 0..PAGE_SIZE_BYTES {
                let i = page_offset * PAGE_SIZE_BYTES + byte_offset;
                unsafe {
                    ptr::write(base.add(i as usize), it.next().unwrap_or(0));
                }
            }
        }

        let elf = unsafe { ElfImage::new(area) };
        elf.verify();
        Ok(elf)
    }

    /// Resolves file descriptor for a process
    pub fn resolve_fc(&mut self, fc: FileClientId) -> IoResult<NodeId> {
        Ok(self
            .process(fc.process.expect("Kernel not supported here"))
            .resolve(fc.fd)
            .expect("No such file descriptor for process"))
    }

    /// Read from file (system call)
    pub fn read(&mut self, fc: FileClientId, buf: &mut [u8]) -> IoResult<usize> {
        let node_id = self.resolve_fc(fc)?;
        self.node_mut(node_id).read(fc, buf)
    }

    /// Write to file (system call)
    pub fn write(&mut self, fc: FileClientId, buf: &mut [u8]) -> IoResult<usize> {
        let node_id = self.resolve_fc(fc)?;
        self.node_mut(node_id).read(fc, buf)
    }

    /// Read a file for other kernel component
    /// Discards data on error.
    pub fn read_file(&mut self, path: &str) -> IoResult<Vec<u8>> {
        let node_id = self.resolve(Path::new(path))?;
        let fc = self.take_kernel_fc();
        let node = self.node_mut(node_id);
        node.open(fc)?;
        let result = unsafe { read_to_end(&mut *node.data, fc) };
        let keep = node.close(fc);
        assert!(keep, "Not possible");
        result
    }

    /// Remove a closed node
    pub fn remove_node(&mut self, node_id: NodeId) {
        self.nodes.remove(&node_id);
    }

    /// Update when a process completes.
    /// Closes all files opened by the process
    /// TODO: flush/synchronize buffers?
    pub fn on_process_over(&mut self, pid: ProcessId, status: ProcessResult) {
        if let Some(pd) = self.descriptors.remove(&pid) {
            // Inform process about its status
            {
                let fc = self.take_kernel_fc();
                let node = self.node_mut(pd.node_id);
                node.open(fc).unwrap();
                unsafe { write_all_ser(&mut *node.data, fc, &status).expect("??") };
                let keep = node.close(fc);
                assert!(keep, "Not possible");
            }
            // Close files process was holding open
            for (fd, node_id) in pd.descriptors.into_iter() {
                let fc = FileClientId::process(pid, fd);
                let keep = self.node_mut(node_id).close(FileClientId::process(pid, fd));
                if !keep {
                    self.remove_node(node_id);
                }
            }
        }
    }
}

lazy_static! {
    pub static ref FILESYSTEM: Mutex<VirtualFS> = Mutex::new(VirtualFS::new());
}

fn create_fs(fs: &mut VirtualFS) -> IoResult<()> {
    // Create top-level fs hierarchy
    fs.create_static_branch(Path::new("/bin"))?;
    fs.create_static_branch(Path::new("/cfg"))?;
    fs.create_static_branch(Path::new("/dev"))?;
    fs.create_static_branch(Path::new("/mnt"))?;
    fs.create_static_branch(Path::new("/prc"))?;

    // Insert special files
    fs.create_static(Path::new("/dev/null"), Box::new(NullDevice))?;
    fs.create_static(Path::new("/dev/zero"), Box::new(ZeroDevice))?;
    fs.create_static(Path::new("/dev/test"), Box::new(TestDevice { rounds: 3 }))?;

    Ok(())
}

pub fn init() {
    let mut fs = FILESYSTEM.lock();
    create_fs(&mut *fs).expect("Could not init filesystem");
}
