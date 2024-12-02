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

use rpl_context::{PatCtxt, PatternCtxt};
use rustc_hir::ItemId;
use rustc_middle::ty::TyCtxt;

pub(crate) mod errors;

mod cve_2018_21000_inlined;
mod cve_2020_25016;
mod cve_2020_35892_3;
// mod cve_2020_35873;

rustc_fluent_macro::fluent_messages! { "../messages.en.ftl" }

static ALL_PATTERNS: &[for<'tcx> fn(TyCtxt<'tcx>, PatCtxt<'_, 'tcx>, ItemId)] = &[
    cve_2018_21000_inlined::check_item,
    cve_2020_25016::check_item,
    cve_2020_35892_3::check_item,
];

pub fn check_item<'tcx>(tcx: TyCtxt<'tcx>, item: ItemId) {
    rustc_data_structures::sync::par_for_each_in(ALL_PATTERNS, |check| {
        PatternCtxt::entered(tcx, |pcx| check(tcx, pcx, item))
    })
    // ALL_PATTERNS.iter().for_each(|check| check(tcx, item))
}
