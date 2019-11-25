use alloc::prelude::v1::*;
use hashbrown::HashMap;
use lazy_static::lazy_static;
use spin::Mutex;

use d7abi::fs::{FileDescriptor, FileInfo};
use d7ramfs;

use crate::multitasking::process::ProcessId;

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

struct ProcessDescriptors {
    descriptors: HashMap<FileDescriptor, NodeId>,
    next_fd: FileDescriptor,
}
impl ProcessDescriptors {
    fn new() -> Self {
        Self {
            descriptors: HashMap::new(),
            next_fd: unsafe { FileDescriptor::from_u64(0) },
        }
    }

    fn resolve(&self, fd: FileDescriptor) -> Option<NodeId> {
        self.descriptors.get(&fd).copied()
    }

    fn create(&mut self, node_id: NodeId) -> FileDescriptor {
        let fd = self.next_fd;
        self.descriptors.insert(fd, node_id);
        self.next_fd = unsafe { self.next_fd.next() };
        fd
    }
}

/// Read all bytes. Used when the kernel needs to read all data.
/// # Safety
/// This is marked unsafe, as it discards data if an io error occurs during it.
unsafe fn read_to_end(file: &mut dyn FileOps, fd: FileClientId) -> IoResult<Vec<u8>> {
    const IO_BUFFER_SIZE: usize = 1024;

    let mut result = Vec::new();
    let mut buffer = [0u8; IO_BUFFER_SIZE];
    loop {
        let count = file.read(fd, &mut buffer)?;
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
unsafe fn write_all(file: &mut dyn FileOps, fd: FileClientId, data: &[u8]) -> IoResult<()> {
    const IO_BUFFER_SIZE: usize = 1024;

    let mut offset: usize = 0;
    while offset < data.len() {
        let count = file.write(fd, &data[offset..])?;
        offset += count;
        if count == 0 {
            panic!("Write failed");
        }
    }
    Ok(())
}

pub struct VirtualFS {
    nodes: HashMap<NodeId, Node>,
    descriptors: HashMap<ProcessId, ProcessDescriptors>,
    next_nodeid: NodeId,
    next_kernel_fd: FileDescriptor,
}
impl VirtualFS {
    pub fn new() -> Self {
        let mut nodes = HashMap::new();
        nodes.insert(ROOT_ID, Node::new(Box::new(InternalBranch::new())));
        Self {
            nodes,
            descriptors: HashMap::new(),
            next_nodeid: ROOT_ID.next(),
            next_kernel_fd: unsafe { FileDescriptor::from_u64(0) },
        }
    }

    /// Traverse tree and return NodeId
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
        let new_node_id = self.next_nodeid;
        if self.has_child(parent_node_id, &(*file_name).to_owned())? {
            Err(IoError::Code(ErrorCode::fs_node_exists))
        } else if self.node(parent_node_id).leafness() == Leafness::Leaf {
            Err(IoError::Code(ErrorCode::fs_node_is_leaf))
        } else {
            self.add_child(parent_node_id, file_name, new_node_id)?;
            self.nodes.insert(new_node_id, new_node);
            self.next_nodeid = new_node_id.next();
            Ok(new_node_id)
        }
    }

    /// Mount a special device
    fn create(&mut self, path: Path, dev: Box<dyn FileOps>) -> IoResult<NodeId> {
        self.create_node(path.clone(), Node::new(dev))
    }

    /// Create a new node
    pub fn create_branch(&mut self, path: Path) -> IoResult<NodeId> {
        self.create(path, Box::new(InternalBranch::new()))
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
                let bytes = unsafe { read_to_end(&mut *node.data, fc)? };
                node.close(fc);
                Ok(pinecone::from_bytes(&bytes).unwrap())
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
                unsafe {
                    write_all(&mut *node.data, fc, &pinecone::to_vec(&data).unwrap())?;
                }
                node.close(fc);
                Ok(())
            },
        }
    }
    /// Get process descriptors
    fn process(&self, pid: ProcessId) -> &ProcessDescriptors {
        lazy_static! {
            static ref EMPTY: ProcessDescriptors = ProcessDescriptors::new();
        }
        self.descriptors.get(&pid).unwrap_or_else(|| &EMPTY)
    }

    /// Get process descriptors
    fn process_mut(&mut self, pid: ProcessId) -> &mut ProcessDescriptors {
        self.descriptors
            .entry(pid)
            .or_insert_with(ProcessDescriptors::new)
    }

    /// Open a file (system call)
    pub fn open(&mut self, path: &str, pid: ProcessId) -> IoResult<FileClientId> {
        let path = Path::new(path);
        let node_id = self.resolve(path)?;
        let process = self.process_mut(pid);
        let fd = process.create(node_id);
        let fc = FileClientId::process(pid, fd);
        self.node_mut(node_id).open(fc)?;
        Ok(fc)
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

    /// Update when a process completes.
    /// Closes all files opened by the process
    /// TODO: flush/synchronize buffers?
    pub fn on_process_over(&mut self, pid: ProcessId) {
        if let Some(pd) = self.descriptors.remove(&pid) {
            for (fd, node_id) in pd.descriptors.into_iter() {
                self.node_mut(node_id).close(FileClientId::process(pid, fd));
            }
        }
    }
}

lazy_static! {
    pub static ref FILESYSTEM: Mutex<VirtualFS> = Mutex::new(VirtualFS::new());
}

fn create_fs(fs: &mut VirtualFS) -> IoResult<()> {
    // Create top-level fs hierarchy
    fs.create_branch(Path::new("/bin"))?;
    fs.create_branch(Path::new("/cfg"))?;
    fs.create_branch(Path::new("/dev"))?;
    fs.create_branch(Path::new("/mnt"))?;

    // Insert special files
    fs.create(Path::new("/dev/null"), Box::new(NullDevice))?;
    fs.create(Path::new("/dev/zero"), Box::new(ZeroDevice))?;
    fs.create(Path::new("/dev/test"), Box::new(TestDevice { rounds: 3 }))?;

    Ok(())
}

pub fn init() {
    let mut fs = FILESYSTEM.lock();
    create_fs(&mut *fs).expect("Could not init filesystem");
}
