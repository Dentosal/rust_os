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


// mod filesystem;

#[no_mangle]
fn main() -> ! {
    println!("VFS daemon starting");
    loop {}
}
