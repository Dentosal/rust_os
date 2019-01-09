#![allow(dead_code)]
#![allow(unused_variables)]

#![feature(nll)]
#![feature(box_syntax)]
#![feature(box_patterns)]
#![feature(const_fn)]
#![feature(const_vec_new)]
#![feature(const_string_new)]
#![feature(alloc)]
#![feature(never_type)]
#![feature(vec_remove_item)]
#![feature(allocator_api)]


#![no_std]

#[cfg(test)]
#[macro_use]
extern crate std;

extern crate alloc;
use alloc::boxed::Box;
use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;



// https://github.com/rust-lang/rust/issues/45599

#[cfg(any(test))]
extern crate alloc_system;

#[cfg(any(test))]
#[global_allocator]
static A: alloc_system::System = alloc_system::System;



#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeType {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileSystemError {
    AlreadyExists,
    DoesNotExist,
    FileRequired,
    DirectoryRequired,
    SeekPastEnd,
    IncorrectOpenMode,
}

// TODO: Implement Append mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenMode {
    Read,
    Write,
    ReadWrite,
}
impl OpenMode {
    fn can_read(&self) -> bool {
        use OpenMode::*;
        match self {
            Read => true,
            Write => false,
            ReadWrite => true
        }
    }
    fn can_write(&self) -> bool {
        use OpenMode::*;
        match self {
            Read => false,
            Write => true,
            ReadWrite => true
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileDescriptorId(u32);
impl FileDescriptorId {
    pub fn new(id: u32) -> FileDescriptorId {
        assert!(id != 0);
        FileDescriptorId(id)
    }

    pub fn invalid() -> FileDescriptorId {
        FileDescriptorId(0)
    }

    pub fn is_valid(&self) -> bool {
        self.0 != 0
    }
}

#[derive(Debug)]
pub struct FileDescriptor {
    id: FileDescriptorId,
    path: Vec<String>,
    cursor: usize,
    mode: OpenMode,
}
impl FileDescriptor {
    fn new(id: FileDescriptorId, path: Vec<&str>, mode: OpenMode) -> FileDescriptor {
        assert!(id.is_valid());
        FileDescriptor {
            id,
            path: path.iter().map(|v| (*v).to_owned()).collect(),
            cursor: 0,
            mode
        }
    }

    pub fn close(&mut self) -> Result<(), !> {
        assert!(self.id.is_valid());
        self.id = FileDescriptorId::invalid();
        debug_assert!(!self.id.is_valid());
        Ok(())
    }
}
impl PartialEq for FileDescriptor {
    fn eq(&self, other: &FileDescriptor) -> bool {
        return self.id != FileDescriptorId::invalid() && self.id == other.id;
    }
}
impl Drop for FileDescriptor {
    fn drop(&mut self) {
        if self.id.is_valid() {
            // println!("{:?}", self);
            // panic!("Close FileDescriptor before it goes out of scope");
        }
    }
}

#[derive(Debug)]
pub struct RamFS {
    tree: Box<Node>,
    open_fds: Vec<FileDescriptor>,
}
impl RamFS {
    pub fn new() -> RamFS {
        RamFS {
            tree: Box::new(Node::root()),
            open_fds: Vec::new(),
        }
    }

    fn focus_rec<T>(mut cont: &mut Box<Node>, mut path: Vec<&str>, f: &mut FnMut(&mut Box<Node>) -> Result<T, FileSystemError>) -> Result<T, FileSystemError> {
        if path.is_empty() {
            return f(&mut cont);
        }

        match cont {
            box Node::Leaf(_) => unimplemented!(),
            box Node::Branch(ref mut branch) => {
                let name = path.remove(0);

                let mut i = 0;
                while i < branch.branches.len() {
                    if name == branch.branches[i].name() {
                        return Self::focus_rec(&mut branch.branches[i], path, f);
                    }
                    i += 1;
                }
                return Err(FileSystemError::DoesNotExist);
            }
        }
    }

    fn focus<T>(&mut self, path: Vec<&str>, f: &mut FnMut(&mut Box<Node>) -> Result<T, FileSystemError>) -> Result<T, FileSystemError> {
        RamFS::focus_rec(&mut self.tree, path, f)
    }

    pub fn node_type(&mut self, path: Vec<&str>) -> Result<NodeType, FileSystemError> {
        self.focus(path, &mut |cont| Ok(cont.kind()))
    }

    pub fn exists(&mut self, path: Vec<&str>) -> bool {
        self.node_type(path).is_ok()
    }

    pub fn create_directory(&mut self, path: Vec<&str>) -> Result<(), FileSystemError> {
        assert!(!path.is_empty());

        let mut pre_path: Vec<&str> = path.clone();
        let name = pre_path.pop().unwrap();

        self.focus(pre_path, &mut |cont| {
            match *cont {
                box Node::Leaf(_) => unimplemented!(),
                box Node::Branch(ref mut b) => {
                    b.add_branch(box Node::Branch(Branch::new(name)));
                    Ok(())
                }
            }
        })
    }

    // pub fn remove_directory(&mut self, path: Vec<&str>, recursive: bool) -> Result<(), FileSystemError>;
    // pub fn rename_directory(&mut self, path: Vec<&str>, new_path: Vec<&str>) -> Result<(), FileSystemError>;

    pub fn create_file(&mut self, path: Vec<&str>, overwrite: bool) -> Result<(), FileSystemError> {
        assert!(!path.is_empty());

        let mut pre_path: Vec<&str> = path.clone();
        let name = pre_path.pop().unwrap();

        self.focus(pre_path, &mut |cont| {
            match *cont {
                box Node::Leaf(_) => unimplemented!(),
                box Node::Branch(ref mut b) => {
                    b.add_branch(box Node::Leaf(Leaf::new(name)));
                    Ok(())
                }
            }
        })
    }

    // pub fn remove_file(&mut self, path: Vec<&str>, recursive: bool) -> Result<(), FileSystemError>;
    // pub fn rename_file(&mut self, path: Vec<&str>, new_path: Vec<&str>) -> Result<(), FileSystemError>;

    /// Get next free file descriptor id. Suboptimal.
    fn get_free_fd_id(&self) -> FileDescriptorId {
        let mut cand = 1;
        loop {
            let fd_id = FileDescriptorId::new(cand);
            let mut free = true;
            for fd in self.open_fds.iter() {
                if fd.id == fd_id {
                    free = false;
                    break;
                }
            }
            if free {
                return fd_id;
            }

            cand += 1;
        }
    }

    pub fn open_file(&mut self, path: Vec<&str>, mode: OpenMode) -> Result<FileDescriptor, FileSystemError> {
        // TODO: check that path+mode can be opened
        // ^: is it already open? etc.

        Ok(FileDescriptor::new(self.get_free_fd_id(), path, mode))
    }

    fn close_file(&mut self, mut fd: FileDescriptor) -> Result<(), !> {
        self.open_fds.remove_item(&fd);
        fd.id = FileDescriptorId::invalid();
        Ok(())
    }

    pub fn file_size(&mut self, fd: &FileDescriptor) -> Result<usize, FileSystemError> {
        assert!(fd.id.is_valid());

        self.focus(fd.path.iter().map(|v| v.as_str()).collect(), &mut |cont| {
            match cont {
                box Node::Leaf(ref mut leaf) => Ok(leaf.bytes.len()),
                box Node::Branch(_) => Err(FileSystemError::FileRequired)
            }
        })
    }

    pub fn file_seek(&mut self, fd: &mut FileDescriptor, offset: usize) -> Result<usize, FileSystemError> {
        assert!(fd.id.is_valid());

        // Prevent lookup when offset == 0
        if offset != 0 && offset > self.file_size(fd)? {
            return Err(FileSystemError::SeekPastEnd);
        }

        fd.cursor = offset;
        Ok(fd.cursor)
    }

    pub fn file_read(&mut self, fd: &mut FileDescriptor, count: Option<usize>) -> Result<Vec<u8>, FileSystemError> {
        assert!(fd.id.is_valid());

        if !fd.mode.can_read() {
            return Err(FileSystemError::IncorrectOpenMode);
        }

        let cursor = fd.cursor;

        let result: Result<Vec<u8>, FileSystemError> = self.focus(fd.path.iter().map(|v| v.as_str()).collect(), &mut |cont| {
            match cont {
                box Node::Leaf(ref mut leaf) => {
                    let iter = leaf.bytes.iter().skip(cursor);
                    if let Some(limit) = count {
                        Ok(iter.take(limit).map(|v| *v).collect())
                    }
                    else {
                        Ok(iter.map(|v| *v).collect())
                    }
                }
                box Node::Branch(_) => Err(FileSystemError::FileRequired)
            }
        });

        if let Ok(ref bytes) = result {
            fd.cursor = bytes.len();
        }
        result

    }

    pub fn file_write(&mut self, fd: &mut FileDescriptor, write_bytes: Vec<u8>) -> Result<(), FileSystemError> {
        assert!(fd.id.is_valid());

        if !fd.mode.can_write() {
            return Err(FileSystemError::IncorrectOpenMode);
        }

        let mut cursor = fd.cursor;

        let result = self.focus(fd.path.iter().map(|v| v.as_str()).collect(), &mut |node| {
            match node {
                box Node::Leaf(ref mut leaf) => {
                    for i in 0..write_bytes.len() {
                        if cursor == leaf.bytes.len() {
                            leaf.bytes.push(write_bytes[i]);
                        }
                        else {
                            leaf.bytes[cursor] = write_bytes[i];

                        }

                        cursor += 1;
                    }
                    Ok(())
                },
                box Node::Branch(_) => Err(FileSystemError::FileRequired)
            }
        });

        if result.is_ok() {
            fd.cursor = cursor;
        }
        result
    }
}


#[derive(Debug)]
enum Node {
    Branch(Branch),
    Leaf(Leaf)
}
impl Node {
    pub const fn root() -> Node {
        Node::Branch(Branch::new_root())
    }

    pub fn name(&self) -> String {
        use Node::*;
        match self {
            Branch(n)   => n.name(),
            Leaf(n)     => n.name()
        }
    }

    pub fn kind(&self) -> NodeType {
        use Node::*;
        match self {
            Branch(n)   => NodeType::Directory,
            Leaf(n)     => NodeType::File
        }
    }
}

/// Branch, a directory
#[derive(Debug)]
struct Branch {
    name: String,
    branches: Vec<Box<Node>>,
}

/// Leaf, a file
#[derive(Debug)]
struct Leaf {
    name: String,
    bytes: Vec<u8>,
}


impl Branch {
    pub const fn new_root() -> Self {
        Self {
            name: String::new(),
            branches: Vec::new(),
        }
    }

    fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            branches: Vec::new(),
        }
    }

    pub fn name(&self) -> String {
        self.name.clone()
    }

    pub fn add_branch(&mut self, branch: Box<Node>) {
        self.branches.push(branch);
    }
}

impl Leaf {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            bytes: Vec::new(),
        }
    }

    pub fn name(&self) -> String {
        self.name.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::{RamFS, OpenMode};

    #[test]
    fn test_simple_create() {
        let mut fs = RamFS::new();

        assert!( fs.exists(vec![]));
        assert!(!fs.exists(vec!["test"]));
        assert!(!fs.exists(vec!["test", "test_inner"]));

        fs.create_directory(vec!["test"]).unwrap();

        assert!( fs.exists(vec![]));
        assert!( fs.exists(vec!["test"]));
        assert!(!fs.exists(vec!["test", "test_inner"]));

        fs.create_directory(vec!["test", "test_inner"]).unwrap();

        assert!( fs.exists(vec![]));
        assert!( fs.exists(vec!["test"]));
        assert!( fs.exists(vec!["test", "test_inner"]));
        assert!(!fs.exists(vec!["file.txt"]));


        assert!(!fs.exists(vec!["file.txt"]));
        assert!(!fs.exists(vec!["test", "file.txt"]));
        assert!(!fs.exists(vec!["test", "test_inner", "file.txt"]));

        fs.create_file(vec!["test", "file.txt"], false).unwrap();

        assert!(!fs.exists(vec!["file.txt"]));
        assert!( fs.exists(vec!["test", "file.txt"]));
        assert!(!fs.exists(vec!["test", "test_inner", "file.txt"]));

        fs.create_file(vec!["file.txt"], false).unwrap();

        assert!( fs.exists(vec!["file.txt"]));
        assert!( fs.exists(vec!["test", "file.txt"]));
        assert!(!fs.exists(vec!["test", "test_inner", "file.txt"]));

        fs.create_file(vec!["test", "test_inner", "file.txt"], false).unwrap();

        assert!( fs.exists(vec!["file.txt"]));
        assert!( fs.exists(vec!["test", "file.txt"]));
        assert!( fs.exists(vec!["test", "test_inner", "file.txt"]));
    }

    #[test]
    fn test_simple_read_write() {
        let mut fs = RamFS::new();
        fs.create_file(vec!["file.txt"], false).unwrap();

        let fd1 = fs.open_file(vec!["file.txt"], OpenMode::Read).unwrap();
        assert_eq!(fs.file_size(&fd1), Ok(0));
        fs.close_file(fd1).unwrap(); // fd1 moved, cannot be reused

        let mut fd2 = fs.open_file(vec!["file.txt"], OpenMode::Write).unwrap();
        assert_eq!(fs.file_size(&fd2), Ok(0));
        fs.file_write(&mut fd2, "Hello World!".bytes().collect()).unwrap();
        assert_eq!(fs.file_size(&fd2), Ok(12));
        fs.close_file(fd2).unwrap(); // fd2 moved, cannot be reused

        let mut fd3 = fs.open_file(vec!["file.txt"], OpenMode::Read).unwrap();
        assert_eq!(fs.file_size(&fd3), Ok(12));
        assert_eq!(fs.file_read(&mut fd3, None), Ok("Hello World!".bytes().collect()));
        fs.close_file(fd3).unwrap(); // fd1 moved, cannot be reused
    }
}
