#![feature(rustc_private)]
#![recursion_limit = "1024"]

extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_errors;
#[cfg(feature = "timing")]
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_lint;
extern crate rustc_macros;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

mod callbacks;
pub use callbacks::{DefaultCallbacks, RPL_ARGS_ENV, RPL_PATS_ENV, RplCallbacks, RustcCallbacks};
