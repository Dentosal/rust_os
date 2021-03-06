// Lints
#![forbid(private_in_public)]
#![warn(bare_trait_objects)]
#![deny(unused_must_use)]
#![deny(unused_assignments)]
#![deny(clippy::missing_safety_doc)]
// no_std
#![no_std]
// Unstable features
#![feature(const_fn)]
#![feature(integer_atomics)]
#![feature(allocator_api)]
#![feature(alloc_prelude)]

#[macro_use]
extern crate alloc;

mod kernel_constants;
mod syscall;

pub mod fs;
pub mod ipc;
pub mod process;

pub use self::kernel_constants::PROCESS_DYNAMIC_MEMORY;
pub use self::syscall::*;
