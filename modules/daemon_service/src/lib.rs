//! Provides the following services:
//! * Starts processes on startup
//! * Program status annoncements
//! * Program running status queries

#![no_std]
#![feature(alloc_prelude)]
#![deny(unused_must_use)]

#[macro_use]
extern crate alloc;

#[macro_use]
extern crate libd7;

use alloc::collections::VecDeque;
use alloc::prelude::v1::*;
use hashbrown::HashSet;
use serde::{Deserialize, Serialize};

use libd7::{
    attachment::{self, RequestFileOperation, ResponseFileOperation},
    d7abi::fs::FileDescriptor,
    fs, pinecone,
    process::{Process, ProcessId},
    select,
    syscall::SyscallResult,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(transparent)]
pub struct ServiceName(String);

/// Analogous to systemd service files
#[derive(Debug, Clone, Deserialize, Serialize)]
struct ServiceDefinition {
    /// Name of the service
    name: ServiceName,
    /// A (short) description of the service
    description: Option<String>,
    /// Requires these services to be running before starting
    requires: HashSet<String>,
    /// Absolute path to the executable
    executable: String,
}

#[derive(Debug)]
struct RunningService {
    name: ServiceName,
    process: Process,
    startup_complete: bool,
}

#[derive(Debug)]
struct Services {
    definitions: Vec<ServiceDefinition>,
    start_queue: VecDeque<ServiceName>,
    running: Vec<RunningService>,
}
impl Services {
    pub fn new(path: &str) -> SyscallResult<Self> {
        let s = fs::read(path)?;
        let definitions: Vec<ServiceDefinition> = serde_json::from_slice(&s).unwrap();
        let start_queue = definitions.iter().map(|s| s.name.clone()).collect();

        Ok(Self {
            definitions,
            start_queue,
            running: Vec::new(),
        })
    }

    // All descriptors. Used for `select!`.
    fn fds(&self) -> HashSet<FileDescriptor> {
        self.running.iter().map(|r| r.process.fd).collect()
    }

    fn definition_by_name(&self, name: &ServiceName) -> Option<ServiceDefinition> {
        for def in &self.definitions {
            if def.name == *name {
                return Some(def.clone());
            }
        }
        return None;
    }

    fn get_running(&mut self, name: &ServiceName) -> Option<&RunningService> {
        for r in &self.running {
            if r.name == *name {
                return Some(&r);
            }
        }
        return None;
    }

    /// Check requirements
    fn are_requirements_up(&mut self, def: &ServiceDefinition) -> bool {
        for req in &def.requires {
            if self.get_running(&def.name).is_none() {
                return false;
            }
        }
        return true;
    }

    /// Start a service if it's not already running
    /// The requirements MUST BE met before calling this
    fn start(&mut self, def: ServiceDefinition) {
        if self.get_running(&def.name).is_some() {
            return;
        }

        let process = Process::spawn(&def.executable).unwrap();
        self.running.push(RunningService {
            name: def.name,
            process,
            startup_complete: false,
        });
    }

    /// Returns true if no more if the called should block
    /// TODO: ^ better explanation
    fn step(&mut self) -> bool {
        if let Some(name) = self.start_queue.pop_front() {
            let def = self.definition_by_name(&name).unwrap();
            if self.are_requirements_up(&def) {
                self.start(def);
                return false;
            }
        }

        return true;
    }

    fn on_process_up(&mut self, pid: ProcessId) {
        for r in &mut self.running {
            if r.process.pid() == pid {
                r.startup_complete = true;
            }
        }
        println!("Process with returned fd was not running");
    }

    fn on_process_completed(&mut self, completed_fd: FileDescriptor) {
        let index = self
            .running
            .iter()
            .position(|r| r.process.fd == completed_fd)
            .expect("Process with returned fd was not running");

        let service = self.running.remove(index);
        todo!("PROC WAIT {:?}", service.name);
        let result = service.process.wait();

        todo!("Process over, result {:?}", result);
    }
}

fn on_request(a: &attachment::Leaf, services: &mut Services) {
    let req = a.next_request().unwrap();
    match &req.operation {
        RequestFileOperation::Write(data) => {
            // Any writes should (for now, at least)
            // be "service has started up" announcements
            assert!(data == &[1]);
            services.on_process_up(req.sender.pid);
            a.reply(req.response(ResponseFileOperation::Write(1)))
                .unwrap();
        }
        RequestFileOperation::Close => {}
        other => panic!("Unsupported operation to a netd ({:?})", other),
    }
}

#[no_mangle]
fn main() -> ! {
    println!("Service daemon starting");

    let mut services = Services::new("/mnt/staticfs/startup_services.json").unwrap();
    let a = attachment::Leaf::new("/srv/service").unwrap();

    loop {
        let should_block = services.step();

        // Only block if there are no processes in startup queue
        if should_block {
            select! {
                any(services.fds()) -> fd => services.on_process_completed(fd),
                one(a.fd) => on_request(&a, &mut services)
            };
        } else {
            select! {
                any(services.fds()) -> fd => services.on_process_completed(fd),
                one(a.fd) => on_request(&a, &mut services),
                would_block => {}
            };
        }
    }
}
