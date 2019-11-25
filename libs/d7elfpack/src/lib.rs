#![feature(stmt_expr_attributes)]
#![cfg_attr(not(any(test, feature = "use-std")), no_std)]
#![allow(unused_imports)]

#[cfg(all(test, not(feature = "use-std")))]
compile_error!("Trying to run tests without std. Supply --features use-std to run.");

mod huffman;

/// Re-export
pub use bit_vec::BitVec;

#[cfg(feature = "use-std")]
pub use huffman::{build_code, compress, decompress};

pub type Book = huffman_compress::Book<u8>;
pub type Tree = huffman_compress::Tree<u8>;
