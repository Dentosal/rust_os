// Lints
#![forbid(private_in_public)]
#![warn(bare_trait_objects)]
#![deny(unused_must_use)]
#![deny(unused_assignments)]
#![deny(overflowing_literals)]
#![deny(clippy::missing_safety_doc)]
// no_std
#![no_std]
// Unstable features
#![feature(integer_atomics)]
#![feature(allocator_api)]

#[macro_use]
extern crate alloc;

mod kernel_constants;
mod syscall;

pub mod ipc;
pub mod process;
pub mod processor_info;

pub use self::kernel_constants::{PROCESS_DYNAMIC_MEMORY, PROCESS_STACK_END};
pub use self::syscall::*;
