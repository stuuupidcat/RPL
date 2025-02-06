#![feature(rustc_private)]
#![feature(let_chains)]
#![feature(path_add_extension)]
#![feature(try_blocks)]

extern crate rustc_ast;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_fluent_macro;
extern crate rustc_hir;
extern crate rustc_hir_pretty;
extern crate rustc_macros;
extern crate rustc_middle;
extern crate rustc_span;
extern crate tracing;

use rustc_middle::ty::TyCtxt;

mod errors;
mod utils;

rustc_fluent_macro::fluent_messages! { "../messages.en.ftl" }

pub fn visit_crate(tcx: TyCtxt<'_>) {
    utils::visit_crate(tcx);
}
