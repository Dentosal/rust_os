// Lints
#![forbid(private_in_public)]
#![forbid(bare_trait_objects)]
#![deny(unused_must_use)]
#![deny(unused_assignments)]
#![deny(clippy::missing_safety_doc)]
// no_std
#![no_std]
// Unstable features
#![feature(const_fn)]
#![feature(integer_atomics)]

mod kernel_constants;
mod syscall;
mod fs;

pub use self::kernel_constants::PROCESS_DYNAMIC_MEMORY;
pub use self::syscall::*;
pub use self::fs::*;
