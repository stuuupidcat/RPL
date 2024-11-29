#![allow(internal_features)]
#![feature(rustc_private)]
#![feature(rustc_attrs)]

extern crate rustc_arena;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_middle;
extern crate rustc_span;

mod context;
pub mod pat;

pub use context::{PatCtxt, PatternCtxt, PrimitiveTypes};
