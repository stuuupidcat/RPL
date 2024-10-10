#![feature(rustc_private)]

extern crate rustc_data_structures;
extern crate rustc_hash;
extern crate rustc_index;
extern crate rustc_middle;

pub(crate) mod rwstate;

pub mod graph;
pub mod mir;
pub mod pattern;

pub use pattern as pat;
