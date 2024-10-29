#![feature(rustc_private)]
#![feature(let_chains)]
#![feature(if_let_guard)]

extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_fluent_macro;
extern crate rustc_hir;
extern crate rustc_macros;
extern crate rustc_middle;
extern crate rustc_span;
#[macro_use]
extern crate tracing;

extern crate rpl_macros;

use rustc_hir::ItemId;
use rustc_middle::ty::TyCtxt;

pub(crate) mod errors;

mod cve_2020_25016;
mod cve_2020_35892_3;
// mod cve_2020_35873;

rustc_fluent_macro::fluent_messages! { "../messages.en.ftl" }

static ALL_PATTERNS: &[fn(TyCtxt<'_>, ItemId)] = &[cve_2020_25016::check_item, cve_2020_35892_3::check_item];

pub fn check_item(tcx: TyCtxt<'_>, item: ItemId) {
    rustc_data_structures::sync::par_for_each_in(ALL_PATTERNS, |check| check(tcx, item))
    // ALL_PATTERNS.iter().for_each(|check| check(tcx, item))
}
