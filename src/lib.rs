//! A Rust port of [jakogut/tinyvm][tinyvm].
//!
//! [tinyvm]: https://github.com/jakogut/tinyvm

mod htab;

pub use htab::*;

#[allow(non_camel_case_types, non_snake_case)]
pub mod ffi;
