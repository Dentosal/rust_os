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

use self::error::{FsError, FsResult};
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
    pub fn resolve(&self, path: Path) -> FsResult<NodeId> {
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
    pub fn list_directory(&mut self, path: Path) -> FsResult<Vec<String>> {
        let id = self.resolve(path)?;
        unimplemented!("LIST DIR")
    }

    /// Create a new node
    pub fn create(&mut self, path: Path, new_node: Node) -> FsResult<NodeId> {
        assert!(!path.is_root());
        assert!(path.is_absolute());

        let file_name = path.file_name().expect("File name missing");
        let parent = path.parent().expect("Parent missing");
        let parent_node = self.resolve(parent)?;
        let id = self.next_nodeid;
        let p = self.node_mut(parent_node);
        if p.has_child(&(*file_name).to_owned())? {
            Err(FsError::NodeExists)
        } else {
            p.add_child(id, file_name)?;
            self.nodes.insert(id, new_node);
            self.next_nodeid = id.next();
            Ok(id)
        }
    }

    /// Create a new node
    pub fn create_branch(&mut self, path: Path) -> FsResult<NodeId> {
        self.create(path, Node::new_branch())
    }

    /// Mount a special device
    fn create_special(&mut self, path: Path, dev: Box<dyn FileOps>) -> FsResult<NodeId> {
        self.create(path.clone(), Node::new_special(dev))
    }

    /// File info (system call)
    pub fn fileinfo(&mut self, path: &str) -> FsResult<d7abi::FileInfo> {
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
    pub fn open(&mut self, path: &str, pid: ProcessId) -> FsResult<FileDescriptor> {
        let path = Path::new(path);
        let node_id = self.resolve(path)?;
        self.node_mut(node_id).open()?;
        let process = self.process_mut(pid);
        Ok(process.create(node_id))
    }

    /// Resolves file descriptor for a process
    pub fn resolve_fd(&mut self, fd: FileDescriptor, pid: ProcessId) -> FsResult<NodeId> {
        Ok(self
            .process(pid)
            .resolve(fd)
            .expect("No such file descriptor for process"))
    }

    /// Read from file (system call)
    pub fn read(&mut self, fd: FileDescriptor, pid: ProcessId, buf: &mut [u8]) -> FsResult<usize> {
        let node_id = self.resolve_fd(fd, pid)?;
        self.node_mut(node_id).read(fd, buf)
    }
}

lazy_static! {
    pub static ref FILESYSTEM: Mutex<VirtualFS> = Mutex::new(VirtualFS::new());
}

fn create_fs(fs: &mut VirtualFS) -> FsResult<()> {
    // Create top-level fs hierarchy
    fs.create_branch(Path::new("/bin"))?;
    fs.create_branch(Path::new("/cfg"))?;
    fs.create_branch(Path::new("/dev"))?;
    fs.create_branch(Path::new("/mnt"))?;

    // Insert special files
    fs.create_special(Path::new("/dev/null"), Box::new(NullDevice))?;
    fs.create_special(Path::new("/dev/zero"), Box::new(ZeroDevice))?;

    Ok(())
}

pub fn init() {
    let mut fs = FILESYSTEM.lock();
    create_fs(&mut *fs).expect("Could not init filesystem");
}
