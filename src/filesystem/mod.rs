use alloc::prelude::v1::*;
use hashbrown::HashMap;
use lazy_static::lazy_static;
use spin::Mutex;

use d7abi::FileDescriptor;
use d7ramfs;

use crate::multitasking::process::ProcessId;

pub mod error;
pub mod file;
pub mod path;
pub mod special_files;
pub mod staticfs;
pub mod tree;

use self::error::{ErrorCode, IoError, IoResult};
use self::file::FileOps;
use self::path::{Path, PathBuf};
use self::special_files::*;
use self::tree::{Node, NodeId};

const ROOT_ID: NodeId = NodeId::first();

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

pub struct VirtualFS {
    nodes: HashMap<NodeId, Node>,
    next_nodeid: NodeId,
    descriptors: HashMap<ProcessId, ProcessDescriptors>,
}
impl VirtualFS {
    pub fn new() -> Self {
        let mut nodes = HashMap::new();
        nodes.insert(ROOT_ID, Node::new_branch());
        Self {
            nodes,
            descriptors: HashMap::new(),
            next_nodeid: ROOT_ID.next(),
        }
    }

    /// Traverse tree and return NodeId
    pub fn resolve(&self, path: Path) -> IoResult<NodeId> {
        assert!(path.is_absolute());
        let mut cursor = ROOT_ID;
        for c in path.components() {
            cursor = self.nodes[&cursor].get_child(c)?;
        }
        Ok(cursor)
    }

    pub fn node(&self, id: NodeId) -> &Node {
        self.nodes.get(&id).unwrap()
    }

    pub fn node_mut(&mut self, id: NodeId) -> &mut Node {
        self.nodes.get_mut(&id).unwrap()
    }

    /// Returns None if the node desn't exist
    pub fn list_directory(&mut self, path: Path) -> IoResult<Vec<String>> {
        let id = self.resolve(path)?;
        unimplemented!("LIST DIR")
    }

    /// Create a new node
    pub fn create(&mut self, path: Path, new_node: Node) -> IoResult<NodeId> {
        assert!(!path.is_root());
        assert!(path.is_absolute());

        let file_name = path.file_name().expect("File name missing");
        let parent = path.parent().expect("Parent missing");
        let parent_node = self.resolve(parent)?;
        let id = self.next_nodeid;
        let p = self.node_mut(parent_node);
        if p.has_child(&(*file_name).to_owned())? {
            Err(IoError::Code(ErrorCode::fs_node_exists))
        } else {
            p.add_child(id, file_name)?;
            self.nodes.insert(id, new_node);
            self.next_nodeid = id.next();
            Ok(id)
        }
    }

    /// Create a new node
    pub fn create_branch(&mut self, path: Path) -> IoResult<NodeId> {
        self.create(path, Node::new_branch())
    }

    /// Mount a special device
    fn create_special(&mut self, path: Path, dev: Box<dyn FileOps>) -> IoResult<NodeId> {
        self.create(path.clone(), Node::new_special(dev))
    }

    /// File info (system call)
    pub fn fileinfo(&mut self, path: &str) -> IoResult<d7abi::FileInfo> {
        let path = Path::new(path);
        Ok(self.node(self.resolve(path)?).fileinfo())
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
    pub fn open(&mut self, path: &str, pid: ProcessId) -> IoResult<FileDescriptor> {
        let path = Path::new(path);
        let node_id = self.resolve(path)?;
        self.node_mut(node_id).open()?;
        let process = self.process_mut(pid);
        Ok(process.create(node_id))
    }

    /// Resolves file descriptor for a process
    pub fn resolve_fd(&mut self, fd: FileDescriptor, pid: ProcessId) -> IoResult<NodeId> {
        Ok(self
            .process(pid)
            .resolve(fd)
            .expect("No such file descriptor for process"))
    }

    /// Read from file (system call)
    pub fn read(&mut self, fd: FileDescriptor, pid: ProcessId, buf: &mut [u8]) -> IoResult<usize> {
        let node_id = self.resolve_fd(fd, pid)?;
        self.node_mut(node_id).read(fd, buf)
    }

    /// Update when a process completes.
    /// Closes all files opened by the process
    /// TODO: flush/synchronize buffers?
    pub fn on_process_over(&mut self, completed: ProcessId) {
        if let Some(pd) = self.descriptors.remove(&completed) {
            for node_id in pd.descriptors.values() {
                self.node_mut(*node_id).close();
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
    fs.create_special(Path::new("/dev/null"), Box::new(NullDevice))?;
    fs.create_special(Path::new("/dev/zero"), Box::new(ZeroDevice))?;
    fs.create_special(Path::new("/dev/test"), Box::new(TestDevice { rounds: 3 }))?;

    Ok(())
}

pub fn init() {
    let mut fs = FILESYSTEM.lock();
    create_fs(&mut *fs).expect("Could not init filesystem");
}
