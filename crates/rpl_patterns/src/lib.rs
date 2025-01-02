#![feature(rustc_private)]
#![feature(let_chains)]
#![feature(if_let_guard)]

extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_fluent_macro;
extern crate rustc_hir;
extern crate rustc_lint_defs;
extern crate rustc_macros;
extern crate rustc_middle;
extern crate rustc_span;
#[macro_use]
extern crate tracing;

extern crate rpl_macros;

use rpl_context::PatCtxt;
use rustc_hir::ItemId;
use rustc_middle::ty::TyCtxt;

mod cve_2018_20992;
mod cve_2018_21000;
mod cve_2019_15548;
mod cve_2019_16138;
mod cve_2020_25016;
mod cve_2020_35873;
mod cve_2020_35881;
mod cve_2020_35892_3;
mod cve_2021_27376;
mod cve_2021_29941_2;
pub(crate) mod errors;
mod lints;

rustc_fluent_macro::fluent_messages! { "../messages.en.ftl" }

static ALL_PATTERNS: &[fn(TyCtxt<'_>, PatCtxt<'_>, ItemId)] = &[
    cve_2018_20992::truncate::check_item,
    cve_2018_20992::extend::check_item,
    cve_2018_21000::t_to_u8::check_item,
    cve_2018_21000::u8_to_t::check_item,
    cve_2019_15548::check_item,
    cve_2019_16138::check_item,
    cve_2020_25016::check_item,
    cve_2020_35873::check_item,
    cve_2020_35892_3::check_item,
    cve_2020_35881::const_const_Transmute_ver::check_item,
    cve_2020_35881::mut_mut_Transmute_ver::check_item,
    cve_2020_35881::mut_const_PtrToPtr_ver::check_item,
    cve_2021_27376::check_item,
    cve_2021_29941_2::check_item,
];

pub fn check_item(tcx: TyCtxt<'_>, pcx: PatCtxt<'_>, item: ItemId) {
    rustc_data_structures::sync::par_for_each_in(ALL_PATTERNS, |check| check(tcx, pcx, item))
    // ALL_PATTERNS.iter().for_each(|check| check(tcx, item))
}
