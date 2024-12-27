#![feature(rustc_private)]
#![feature(let_chains)]

extern crate rustc_abi;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_middle;
extern crate rustc_span;
#[macro_use]
extern crate tracing;

mod adt;
mod fns;
pub(crate) mod resolve;
mod ty;

pub use adt::{AdtMatch, FieldMatch, MatchAdtCtxt, VariantMatch};
pub use fns::MatchFnCtxt;
pub use ty::MatchTyCtxt;
