//! Virtual "filesystem", a key-value storage where keys represent *paths*
//! and values are *file objects*. Each key must have unique prefix.
//! Directories without files inside them cannot exit, similarly to git.
//! The data structure is closely related to [tries](https://en.wikipedia.org/wiki/Trie).
//!
//! Supported operations for the "filesystem" itself are:
//! * Insert (key, value) pair
//! * Get object from key
//! * Get all keys starting with a prefix

use alloc::prelude::v1::*;
use core::convert::TryInto;
use hashbrown::HashMap;
use lazy_static::lazy_static;
use serde::Serialize;
use spin::Mutex;

use d7abi::fs::FileDescriptor;

use crate::memory::MemoryController;
use crate::multitasking::{
    process::{ProcessId, ProcessResult},
    ElfImage, Scheduler, WaitFor,
};

mod attachment;
mod node;
mod path;

pub mod file;
pub mod result;
pub mod staticfs;

use self::file::*;
use self::result::{ErrorCode, IoContext, IoResult, IoResultPure};

pub use self::node::*;
pub use self::path::{Path, PathBuf};

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

/// File opened by Open
#[derive(Debug, Clone)]
struct OpenFile {
    /// NodeId
    node_id: NodeId,
    /// Relative path left after the node
    suffix: Option<PathBuf>,
}

/// NodeId and descriptors for a process
struct ProcessDescriptors {
    /// Node id of the process
    node_id: NodeId,
    /// Descriptors owned by the process
    descriptors: HashMap<FileDescriptor, OpenFile>,
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

    fn resolve(&self, fd: FileDescriptor) -> Option<OpenFile> {
        self.descriptors.get(&fd).cloned()
    }

    fn create_id(&mut self, node_id: NodeId, suffix: Option<PathBuf>) -> FileDescriptor {
        let fd = self.next_fd;
        self.descriptors.insert(fd, OpenFile { node_id, suffix });
        self.next_fd = unsafe { self.next_fd.next() };
        fd
    }
}

/// Read all bytes. Used when the kernel needs to read all data from a file.
/// # Safety
/// This is marked unsafe, as it discards data if an io error occurs during it.
unsafe fn read_to_end(file: &mut dyn FileOps, fc: FileClientId) -> IoResult<Vec<u8>> {
    const IO_BUFFER_SIZE: usize = 4096;

    let mut result = Vec::new();
    let mut buffer = [0u8; IO_BUFFER_SIZE];
    loop {
        let (count, events) = file.read(fc, &mut buffer)?;
        assert!(events.is_empty()); // TODO
        result.extend(buffer[..count].iter());
        if count < IO_BUFFER_SIZE {
            // EOF
            break;
        }
    }
    IoResult::success(result)
}

/// Write all bytes. Used when the kernel needs to write a fixed amount of data.
/// # Safety
/// This is marked unsafe, as it discards data if an io error occurs during it.
unsafe fn write_all(file: &mut dyn FileOps, fc: FileClientId, data: &[u8]) -> IoResult<()> {
    const IO_BUFFER_SIZE: usize = 4096;

    let mut offset: usize = 0;
    let mut ctx = IoContext::new();
    while offset < data.len() {
        let (count, new_ctx) = file.write(fc, &data[offset..]).with_context(ctx)?;
        ctx = new_ctx;
        offset += count;
        if count == 0 {
            panic!("Write failed");
        }
    }
    IoResult::success(()).with_context(ctx)
}

/// Serialize with Pinecone, and write all bytes
unsafe fn write_all_ser<T: Serialize>(
    file: &mut dyn FileOps, fc: FileClientId, data: &T,
) -> IoResult<()> {
    let data = pinecone::to_vec(data).expect("Failed to encode");
    write_all(file, fc, &data)
}

/// Tree, Path -> NodeId mapping
#[derive(Debug)]
pub enum Tree {
    Branch(HashMap<String, Tree>),
    Leaf(NodeId),
}
impl Tree {
    /// Finds prefix path in tree, returns NodeId and the rest of the path
    /// Path must be relative.
    pub fn resolve<'a>(&self, path: &'a Path) -> Option<(NodeId, Option<&'a Path>)> {
        assert!(path.is_relative());
        self._resolve_inner(Some(path))
    }

    fn _resolve_inner<'a>(&self, path: Option<&'a Path>) -> Option<(NodeId, Option<&'a Path>)> {
        match self {
            Self::Leaf(node_id) => Some((*node_id, path)),
            Self::Branch(children) => {
                if let Some(p) = path {
                    let (head, tail) = p.split_left();
                    if let Some(subtree) = children.get(head) {
                        subtree._resolve_inner(tail)
                    } else {
                        None
                    }
                } else {
                    None
                }
            },
        }
    }

    /// Inserts a value to the given path. Return error if node already exists.
    pub fn insert(&mut self, path: &Path, node_id: NodeId) -> IoResultPure<()> {
        assert!(path.is_relative());

        let (head, tail) = path.split_left();
        if let Some(t) = tail {
            match self {
                Self::Leaf(node_id) => IoResultPure::Error(ErrorCode::fs_node_path_blocked),
                Self::Branch(children) => {
                    if let Some(child) = children.get_mut(head) {
                        child.insert(&t, node_id)
                    } else {
                        let mut subtree = Self::Branch(HashMap::new());
                        subtree.insert(&t, node_id).unwrap();
                        children.insert(head.to_owned(), subtree);
                        IoResultPure::Success(())
                    }
                },
            }
        } else {
            match self {
                Self::Leaf(node_id) => IoResultPure::Error(ErrorCode::fs_node_exists),
                Self::Branch(children) => {
                    children.insert(head.to_owned(), Tree::Leaf(node_id));
                    IoResultPure::Success(())
                },
            }
        }
    }
}

pub struct VirtualFS {
    /// Tree, Path -> NodeId mapping
    tree: Tree,
    /// The actual file objects
    nodes: HashMap<NodeId, Leaf>,
    /// Process file descriptor -> NodeId mapping
    descriptors: HashMap<ProcessId, ProcessDescriptors>,
    /// Next node id
    next_node_id: NodeId,
    /// Next file descriptor for the kernel
    next_kernel_fd: FileDescriptor,
}
impl VirtualFS {
    pub fn new() -> Self {
        Self {
            tree: Tree::Branch(HashMap::new()),
            nodes: HashMap::new(),
            descriptors: HashMap::new(),
            next_node_id: NodeId::first(),
            next_kernel_fd: unsafe { FileDescriptor::from_u64(0) },
        }
    }

    pub fn take_kernel_fc(&mut self) -> FileClientId {
        let fd = self.next_kernel_fd;
        self.next_kernel_fd = unsafe { self.next_kernel_fd.next() };
        FileClientId::kernel(fd)
    }

    fn resolve<'a>(&mut self, path: &'a Path) -> IoResultPure<(NodeId, Option<&'a Path>)> {
        assert!(path.is_absolute());
        if let Some(r) = self.tree.resolve(path.to_relative()) {
            IoResultPure::Success(r)
        } else {
            IoResultPure::Error(ErrorCode::fs_node_not_found)
        }
    }

    pub fn node(&self, id: NodeId) -> IoResultPure<&Leaf> {
        if let Some(node) = self.nodes.get(&id) {
            IoResultPure::Success(node)
        } else {
            log::trace!("Attempting to access destroyed node {:?}", id);
            IoResultPure::Error(ErrorCode::fs_file_destroyed)
        }
    }

    pub fn node_mut(&mut self, id: NodeId) -> IoResultPure<&mut Leaf> {
        if let Some(node) = self.nodes.get_mut(&id) {
            IoResultPure::Success(node)
        } else {
            log::trace!("Attempting to access destroyed node {:?}", id);
            IoResultPure::Error(ErrorCode::fs_file_destroyed)
        }
    }

    /// Create a new node
    pub fn create_node(&mut self, path: &Path, new_node: Leaf) -> IoResultPure<NodeId> {
        self.create_node_with(path, |_, _| (new_node, ()))
            .map(|t| t.0)
    }

    /// Create a new node, with contents created using a closure
    pub fn create_node_with<F, R>(&mut self, path: &Path, f: F) -> IoResultPure<(NodeId, R)>
    where F: FnOnce(&mut Self, NodeId) -> (Leaf, R) {
        let new_node_id = self.next_node_id;
        let (new_node, result) = f(self, new_node_id);
        self.nodes.insert(new_node_id, new_node);
        self.tree.insert(path.to_relative(), new_node_id)?;
        self.next_node_id = new_node_id.next();
        IoResultPure::Success((new_node_id, result))
    }

    /// Create a new node, and give it one initial reference so it
    /// will not be removed automatically
    fn create_static(&mut self, path: &Path, obj: Box<dyn FileOps>) -> IoResultPure<NodeId> {
        let node_id = self.create_node(path, Leaf::new(obj))?;
        let fc = self.take_kernel_fc();
        let node = self.node_mut(node_id).unwrap();
        node.inc_ref();
        IoResultPure::Success(node_id)
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
    pub fn open(
        &mut self, sched: &mut Scheduler, pid: ProcessId, path: &str,
    ) -> IoResultPure<FileClientId> {
        let path = Path::new(path);
        let (node_id, suffix) = self.resolve(path)?;
        let process = self.process_mut(pid);
        let fd = process.create_id(node_id, suffix.map(|s| s.to_owned()));
        let fc = FileClientId::process(pid, fd);

        let node = self.node_mut(node_id).unwrap();
        match &mut node.data {
            LeafData::FileObject(f) => {},
            LeafData::Attachment(a) => a
                .open(suffix.map(|p| p.to_owned()), fc)
                .consume_events(sched)?,
        };
        node.inc_ref();
        log::trace!(
            "open pid={} path={:?} suffix={:?} node_id={:?} new refcount={}",
            pid,
            path,
            suffix,
            node_id,
            node.fc_refcount
        );
        IoResultPure::Success(fc)
    }

    /// Exec a file (system call)
    pub fn exec(
        &mut self, mem_ctrl: &mut MemoryController, sched: &mut Scheduler, owner_pid: ProcessId,
        path: &str,
    ) -> IoResultPure<FileClientId>
    {
        self.exec_optional_owner(mem_ctrl, sched, path, Some(owner_pid))
            .consume_events(sched)
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
        let elfimage = self.load_module(mem_ctrl, path).consume_events(sched)?;

        // Open the executable file for the owner of the process
        let path = Path::new(path);
        let (owner_node_id, suffix) = self.resolve(path).expect("Not found");
        assert!(suffix.is_none(), "Exec from attachment not supported yet");
        let fc = {
            if let Some(pid) = owner_pid {
                let process = self.process_mut(pid);
                let fd = process.create_id(owner_node_id, None);
                FileClientId::process(pid, fd)
            } else {
                self.take_kernel_fc()
            }
        };
        self.node_mut(owner_node_id)?.inc_ref();

        // Spawn the new process
        let new_pid = unsafe { sched.spawn(mem_ctrl, elfimage) };

        // Create a node and descriptors for the process
        let node_id = self.create_node(
            &Path::new(&format!("/prc/{}", new_pid)),
            Leaf::new(Box::new(ProcessFile::new(new_pid, fc))),
        )?;
        self.descriptors
            .insert(new_pid, ProcessDescriptors::new(node_id));

        // Open a file descriptor to the process for the owner
        let fc = {
            if let Some(pid) = owner_pid {
                let process = self.process_mut(pid);
                let fd = process.create_id(node_id, None);
                FileClientId::process(pid, fd)
            } else {
                self.take_kernel_fc()
            }
        };
        self.node_mut(node_id)?.inc_ref();
        IoResult::success(fc)
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

        let bytes = self
            .read_file(path)
            .expect_events("load_module doesn't support events yet")?;

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
        IoResult::success(elf)
    }

    /// Resolves file descriptor for a process
    fn resolve_fc(&mut self, fc: FileClientId) -> IoResultPure<OpenFile> {
        IoResultPure::Success(
            self.process(fc.process.expect("Kernel not supported here"))
                .resolve(fc.fd)
                .expect("No such file descriptor for process"),
        )
    }

    /// Attach to fs (system call)
    /// Give empty path to not bind the attachment directly to VFS,
    /// this is used to create subnodes for attached branches
    pub fn attach(
        &mut self, sched: &mut Scheduler, pid: ProcessId, path: &str, is_leaf: bool,
    ) -> IoResultPure<FileClientId> {
        let path = Path::new(path);
        // Create with an attachment point
        let (node_id, fc) = self.create_node_with(path, |s, id| {
            let process = s.process_mut(pid);
            let fd = process.create_id(id, None);
            let fc = FileClientId::process(pid, fd);
            (Leaf::new_attachment(fc, is_leaf), fc)
        })?;

        log::trace!("Attach {:?} {:?}", node_id, fc);

        // Open file descriptor for the manager
        let node = self.node_mut(node_id).unwrap();
        match &mut node.data {
            LeafData::FileObject(f) => unreachable!(),
            LeafData::Attachment(a) => a.open(None, fc).consume_events(sched)?,
        };
        self.node_mut(node_id)?.inc_ref();
        IoResultPure::Success(fc)
    }

    /// Close a file descriptor (system call)
    /// If the process owning the file descriptor is already
    /// dead, then the close request is simply ignored
    pub fn close(&mut self, sched: &mut Scheduler, fc: FileClientId) {
        let open_file = if let IoResultPure::Success(v) = self.resolve_fc(fc) {
            v
        } else {
            return;
        };
        let node = if let IoResultPure::Success(v) = self.node_mut(open_file.node_id) {
            v
        } else {
            return;
        };

        log::debug!("Closed {:?} old refcount={}", open_file, node.fc_refcount);

        let result = match &mut node.data {
            LeafData::FileObject(f) => f.close(fc),
            LeafData::Attachment(a) => a.close(open_file.suffix, fc),
        }
        .consume_events(sched);
        let action = result.expect("Close must not fail");
        let keep = node.dec_ref();
        if action == CloseAction::Destroy || !keep {
            if action == CloseAction::Destroy {
                log::trace!("close: Destroy request from node");
            } else {
                log::trace!("close: Destroy refcount == 0");
            }
            self._destroy(sched, open_file.node_id);
        }
    }

    /// Close a file descriptor using node_id
    /// Can be used to close file descriptors for already-killed process.
    /// This function never fails, but may trigger events.
    /// No suffix support, as the attachment is always destroyed using this.
    pub fn close_removed(&mut self, sched: &mut Scheduler, fc: FileClientId, node_id: NodeId) {
        if let Some(node) = self.nodes.get_mut(&node_id) {
            let result = match &mut node.data {
                LeafData::FileObject(f) => f.close(fc),
                LeafData::Attachment(a) => a.close(None, fc),
            }
            .consume_events(sched);
            let action = result.expect("Close must not fail");
            let keep = node.dec_ref();
            if action == CloseAction::Destroy || !keep {
                if action == CloseAction::Destroy {
                    log::trace!("close_removed: Destroy request from node");
                } else {
                    log::trace!("close_removed: Destroy refcount == 0");
                }
                self._destroy(sched, node_id);
            }
        }
    }

    /// Remove node by id, running destructors
    pub fn _destroy(&mut self, sched: &mut Scheduler, node_id: NodeId) {
        log::debug!("Destroying node {:?}", node_id);
        let mut node = self.nodes.remove(&node_id).expect("Node does not exist");
        let trigger = match node.data {
            LeafData::FileObject(mut f) => f.destroy(),
            LeafData::Attachment(mut a) => a.destroy(),
        };
        trigger.run(sched, self);
    }

    /// Read from file (system call)
    pub fn read(
        &mut self, sched: &mut Scheduler, fc: FileClientId, buf: &mut [u8],
    ) -> IoResultPure<usize> {
        let open_file = self.resolve_fc(fc)?;
        assert!(open_file.suffix.is_none(), "TODO: Suffix support");
        let node = self.node_mut(open_file.node_id)?;
        match &mut self.node_mut(open_file.node_id)?.data {
            LeafData::FileObject(f) => {
                if open_file.suffix.is_some() {
                    panic!("Normal file objects do not support suffixes.");
                } else {
                    f.read(fc, buf)
                }
            },
            LeafData::Attachment(a) => a.read(open_file.suffix, fc, buf),
        }
        .consume_events(sched)
    }

    /// Read waiting for (used by system call: fd_select)
    pub fn read_waiting_for(
        &mut self, sched: &mut Scheduler, fc: FileClientId,
    ) -> IoResultPure<WaitFor> {
        let open_file = self.resolve_fc(fc)?;
        assert!(open_file.suffix.is_none(), "TODO: Suffix support");

        IoResultPure::Success(match &mut self.node_mut(open_file.node_id)?.data {
            LeafData::FileObject(f) => {
                if open_file.suffix.is_some() {
                    panic!("Normal file objects do not support suffixes.");
                } else {
                    f.read_waiting_for(fc)
                }
            },
            LeafData::Attachment(a) => a.read_waiting_for(open_file.suffix, fc),
        })
    }

    /// Write to file (system call)
    pub fn write(
        &mut self, sched: &mut Scheduler, fc: FileClientId, buf: &[u8],
    ) -> IoResultPure<usize> {
        let open_file = self.resolve_fc(fc)?;
        log::trace!("Write {:?} {:?}", fc, open_file);
        match &mut self.node_mut(open_file.node_id)?.data {
            LeafData::FileObject(f) => {
                if open_file.suffix.is_some() {
                    panic!("Normal file objects do not support suffixes.");
                } else {
                    f.write(fc, buf)
                }
            },
            LeafData::Attachment(a) => a.write(open_file.suffix, fc, buf),
        }
        .consume_events(sched)
    }

    /// Get pid (system call)
    pub fn get_pid(&mut self, fc: FileClientId) -> IoResultPure<ProcessId> {
        let open_file = self.resolve_fc(fc)?;
        match &self.node(open_file.node_id)?.data {
            LeafData::FileObject(f) => f.pid(),
            LeafData::Attachment(a) => a.pid(),
        }
    }

    /// Read a file for other kernel component
    /// Discards data on error.
    pub fn read_file(&mut self, path: &str) -> IoResult<Vec<u8>> {
        let (node_id, suffix) = self.resolve(Path::new(path))?;
        assert!(suffix.is_none(), "TODO: Suffix support");
        self.temp_open(node_id, |node, fc| match &mut node.data {
            LeafData::FileObject(p) => unsafe { read_to_end(p.as_mut(), fc) },
            LeafData::Attachment(p) => todo!("VFS.read_file: attachement"),
        })
    }

    /// Temporarily open a file for other functions here
    pub fn temp_open<F, R>(&mut self, node_id: NodeId, f: F) -> IoResult<R>
    where F: FnOnce(&mut Leaf, FileClientId) -> IoResult<R> {
        let fc = self.take_kernel_fc();
        let node = self.node_mut(node_id)?;

        match &mut node.data {
            LeafData::FileObject(f) => {},
            LeafData::Attachment(a) => a
                .open(None, fc)
                .expect_events("Attachment produced events in temp_open (open)")?,
        };

        let r = f(node, fc);

        let result = match &mut node.data {
            LeafData::FileObject(f) => f.close(fc),
            LeafData::Attachment(a) => a.close(None, fc),
        }
        .expect_events("Attachment produced events in temp_open (close)");
        let action = result.expect("Close must not fail");
        if action == CloseAction::Destroy {
            panic!("Destroying node after temp_open is not allowed.");
        }

        r
    }

    /// Update when a process completes.
    /// Closes all files opened by the process, and send wakeup signals for them if required.
    /// TODO: flush/synchronize buffers?
    pub fn on_process_over(
        &mut self, sched: &mut Scheduler, pid: ProcessId, status: ProcessResult,
    ) {
        if let Some(pd) = self.descriptors.remove(&pid) {
            // Inform process about its status.
            // The node will not exist iff all process having access to
            // it's result are already dead. In this case the status
            // doesn't have to be stored
            if self.nodes.contains_key(&pd.node_id) {
                self.temp_open(pd.node_id, |mut node, fc| match &mut node.data {
                    LeafData::FileObject(p) => unsafe { write_all_ser(p.as_mut(), fc, &status) },
                    LeafData::Attachment(p) => panic!("Attachment is not a process"),
                })
                .unwrap()
            }

            // Close files process was holding open
            for (fd, open_file) in pd.descriptors.into_iter() {
                let fc = FileClientId::process(pid, fd);
                self.close_removed(sched, fc, open_file.node_id);
            }
        }
    }
}

lazy_static! {
    pub static ref FILESYSTEM: Mutex<VirtualFS> = Mutex::new(VirtualFS::new());
}

fn create_fs(fs: &mut VirtualFS) -> IoResult<()> {
    // Create top-level fs hierarchy
    // TODO: Move to documentation
    // fs.create_static_branch(Path::new("/bin"))?; // Binaries
    // fs.create_static_branch(Path::new("/cfg"))?; // System configuration
    // fs.create_static_branch(Path::new("/dev"))?; // Device files
    // fs.create_static_branch(Path::new("/mnt"))?; // Fs mount points
    // fs.create_static_branch(Path::new("/prc"))?; // Processes
    // fs.create_static_branch(Path::new("/srv"))?; // Service endpoints

    // Insert special files
    fs.create_static(Path::new("/dev/null"), Box::new(NullDevice))?;
    fs.create_static(Path::new("/dev/zero"), Box::new(ZeroDevice))?;
    fs.create_static(Path::new("/dev/test"), Box::new(TestDevice { rounds: 3 }))?;

    // Kernel console
    fs.create_static(
        Path::new("/dev/console"),
        Box::new(KernelConsoleDevice::new()),
    )?;

    // NIC interface
    fs.create_static(Path::new("/dev/nic"), Box::new(NetworkDevice))?;
    fs.create_static(Path::new("/dev/nic_mac"), Box::new(MacAddrDevice))?;

    IoResult::success(())
}

pub fn init() {
    let mut fs = FILESYSTEM.lock();
    create_fs(&mut *fs).expect("Could not init filesystem");
}
