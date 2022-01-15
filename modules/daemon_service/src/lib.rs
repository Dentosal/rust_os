//! Provides the following functionality:
//! * Starts services on startup
//! * Service status annoncements
//! * Service running status queries
//! * Service registration/discovery

#![no_std]
#![feature(drain_filter)]
#![deny(unused_must_use)]

#[macro_use]
extern crate alloc;

#[macro_use]
extern crate libd7;

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;
use hashbrown::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use libd7::{
    d7abi::{
        ipc::protocol::{service::*, ProcessTerminated},
        process::ProcessResult,
    },
    ipc::{self, AcknowledgeContext, SubscriptionId},
    pinecone,
    process::{Process, ProcessId},
    select,
    syscall::SyscallResult,
};

/// Analogous to systemd service files
#[derive(Debug, Clone, Deserialize, Serialize)]
struct ServiceDefinition {
    /// Name of the service
    name: ServiceName,
    /// A (short) description of the service
    description: Option<String>,
    /// Requires these services to be running before starting
    requires: HashSet<ServiceName>,
    /// Executable points to initrd
    from_initrd: bool,
    /// Absolute path to the executable
    executable: String,
}

#[derive(Debug)]
struct Services {
    /// Definitions for managed services
    definitions: Vec<ServiceDefinition>,
    /// Queue of managed services to start
    start_queue: Vec<ServiceName>,
    /// Running managed services
    managed: HashMap<ProcessId, (Process, ServiceName)>,
    /// Services that are running, and bool for oneshot status.
    /// I.e. if the bool is true, never remove the item
    discovery: HashMap<ServiceName, bool>,
    waiting_for_all: Vec<(HashSet<ServiceName>, AcknowledgeContext)>,
    waiting_for_any: Vec<(HashSet<ServiceName>, AcknowledgeContext)>,
}
impl Services {
    pub fn new(path: &str) -> SyscallResult<Self> {
        let s: Vec<u8> = ipc::request("initrd/read", path.to_owned())?;
        let definitions: Vec<ServiceDefinition> = serde_json::from_slice(&s).unwrap();
        let start_queue = definitions.iter().map(|s| s.name.clone()).collect();

        Ok(Self {
            definitions,
            start_queue,
            managed: HashMap::new(),
            discovery: HashMap::new(),
            waiting_for_all: Vec::new(),
            waiting_for_any: Vec::new(),
        })
    }

    fn definition_by_name(&self, name: &ServiceName) -> Option<ServiceDefinition> {
        for def in &self.definitions {
            if def.name == *name {
                return Some(def.clone());
            }
        }
        return None;
    }

    fn is_registered(&self, name: &ServiceName) -> bool {
        self.discovery.contains_key(name)
    }

    /// Check requirements
    fn are_requirements_up(&self, def: &ServiceDefinition) -> bool {
        def.requires.iter().all(|reg| self.is_registered(&reg))
    }

    /// Start a service if it's not already running
    /// The requirements MUST BE met before calling this
    fn start(&mut self, def: &ServiceDefinition) {
        log::info!("Spawning process: {}", def.name);
        assert!(
            def.from_initrd,
            "Non-initrd executables are not supported yet"
        );
        let process = Process::spawn(&def.executable, &[]).unwrap();
        self.managed
            .insert(process.pid(), (process, def.name.clone()));
    }

    fn step(&mut self) {
        let mut start_indices = Vec::new();
        for (i, name) in self.start_queue.iter().enumerate() {
            let def = self.definition_by_name(&name).unwrap();
            if self.are_requirements_up(&def) {
                start_indices.push(i);
                log::debug!("All requirements are up for {}, starting", name);
            } else {
                log::debug!("Not all requirements are up for {}", name);
            }
        }
        while let Some(i) = start_indices.pop() {
            let name = self.start_queue.remove(i);
            let def = self.definition_by_name(&name).unwrap();
            self.start(&def);
        }
    }

    /// Ack if name is free, otherwise deny
    fn on_register(&mut self, (ack_ctx, reg): (AcknowledgeContext, Registration)) {
        if self.discovery.contains_key(&reg.name) {
            ack_ctx.nack().unwrap();
        } else {
            self.discovery.insert(reg.name, reg.oneshot);
            ack_ctx.ack().unwrap();

            // Update waiting processes
            let mut completed = Vec::new();
            for (i, (set, _)) in self.waiting_for_all.iter().enumerate() {
                if set.iter().all(|s| self.is_registered(s)) {
                    completed.push(i);
                }
            }
            while let Some(i) = completed.pop() {
                let (_, ack_ctx) = self.waiting_for_all.remove(i);
                ack_ctx.ack().unwrap();
                println!("WAKEUP delayed any");
            }

            let mut completed = Vec::new();
            for (i, (set, _)) in self.waiting_for_any.iter().enumerate() {
                if set.iter().any(|s| self.is_registered(s)) {
                    completed.push(i);
                }
            }
            while let Some(i) = completed.pop() {
                let (_, ack_ctx) = self.waiting_for_any.remove(i);
                ack_ctx.ack().unwrap();
                println!("WAKEUP delayed all");
            }
        }
    }

    /// Ack only after any of the services is available
    fn on_waitfor_any(&mut self, (ack_ctx, names): (AcknowledgeContext, HashSet<ServiceName>)) {
        if names.iter().any(|s| self.is_registered(s)) {
            ack_ctx.ack().unwrap();
            println!("WAKEUP immeadiate any");
        } else {
            self.waiting_for_any.push((names, ack_ctx));
        }
    }

    /// Ack only after all of the services are available
    fn on_waitfor_all(&mut self, (ack_ctx, names): (AcknowledgeContext, HashSet<ServiceName>)) {
        if names.iter().all(|s| self.is_registered(s)) {
            ack_ctx.ack().unwrap();
            println!("WAKEUP immeadiate all");
        } else {
            self.waiting_for_all.push((names, ack_ctx));
        }
    }

    fn on_process_completed(&mut self, terminated: ProcessTerminated) {
        if let Some((process, name)) = self.managed.remove(&terminated.pid) {
            if let Some(oneshot) = self.discovery.get(&name) {
                if !(*oneshot && matches!(terminated.result, ProcessResult::Completed(0))) {
                    self.discovery.remove(&name);
                }
            }
        }
    }
}

#[no_mangle]
fn main() -> ! {
    println!("Service daemon starting");

    let mut services = Services::new("startup_services.json").unwrap();

    // For managed services to register themselves
    let register = ipc::ReliableSubscription::<Registration>::exact("serviced/register").unwrap();

    // Wait until a service comes online
    let waitfor_any =
        ipc::ReliableSubscription::<HashSet<ServiceName>>::exact("serviced/waitfor/any").unwrap();
    let waitfor_all =
        ipc::ReliableSubscription::<HashSet<ServiceName>>::exact("serviced/waitfor/all").unwrap();

    // Subscripbe for process termination messages
    let terminated =
        ipc::UnreliableSubscription::<ProcessTerminated>::exact("process/terminated").unwrap();

    loop {
        println!("service step");
        services.step();
        select! {
            one(terminated) => services.on_process_completed(terminated.receive().unwrap()),
            one(register) => services.on_register(register.receive().unwrap()),
            one(waitfor_any) => services.on_waitfor_any(waitfor_any.receive().unwrap()),
            one(waitfor_all) => services.on_waitfor_all(waitfor_all.receive().unwrap())
        };
    }
}
