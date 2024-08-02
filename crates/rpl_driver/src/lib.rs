#![feature(rustc_private)]

extern crate rustc_data_structures;
extern crate rustc_errors;
extern crate rustc_fluent_macro;
extern crate rustc_middle;

rustc_fluent_macro::fluent_messages! { "../messages.en.ftl" }

use rustc_middle::ty::TyCtxt;
use rustc_middle::util::Providers;

pub fn provide(_providers: &mut Providers) {}

pub fn check_crate(tcx: TyCtxt<'_>) {
    _ = tcx.hir_crate_items(()).par_items(|item_id| {
        rpl_patterns::check_item(tcx, item_id);
        Ok(())
    });
}
