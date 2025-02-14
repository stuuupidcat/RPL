#![feature(rustc_private)]

extern crate rustc_data_structures;
extern crate rustc_errors;
extern crate rustc_fluent_macro;
extern crate rustc_interface;
extern crate rustc_lint_defs;
extern crate rustc_middle;
extern crate rustc_span;

rustc_fluent_macro::fluent_messages! { "../messages.en.ftl" }

use rpl_context::PatCtxt;
use rpl_meta_pest::context::RPLMetaContext;
use rustc_lint_defs::RegisteredTools;
use rustc_middle::ty::TyCtxt;
use rustc_middle::util::Providers;
use rustc_span::symbol::Ident;

pub fn provide(providers: &mut Providers) {
    providers.registered_tools = registered_tools;
}

fn registered_tools(tcx: TyCtxt<'_>, (): ()) -> RegisteredTools {
    let mut registered_tools = (rustc_interface::DEFAULT_QUERY_PROVIDERS.registered_tools)(tcx, ());
    registered_tools.insert(Ident::from_str("rpl"));
    registered_tools
}

pub fn check_crate(tcx: TyCtxt<'_>, pcx: PatCtxt<'_>, mctx: &RPLMetaContext<'_>) {
    // TODO
    // pcx.add_parsed_patterns(mctx);
    _ = tcx.hir_crate_items(()).par_items(|item_id| {
        rpl_patterns::check_item(tcx, pcx, item_id);
        // TODO
        // pcx.for_each_pattern(|_name, _pat| todo!());
        Ok(())
    });
    rpl_utils::visit_crate(tcx);
}
