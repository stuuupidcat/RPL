// use std::sync::LazyLock;

// use rpl_middle::ty::{RplConfig, RplCtxt, RtyCtxt, LaterLintStoreMarker};
// use rpl_middle::utils::Providers;
// use rustc_middle::ty::TyCtxt;

/*
pub fn create_rpl_ctxt<'bcx>(
    tcx: TyCtxt<'_>,
    lint_store: Option<Box<dyn LaterLintStoreMarker>>,
    config: RplConfig,
) -> RplCtxt<'bcx, '_> {
    let providers = *DEFAULT_QUERY_PROVIDERS;
    RtyCtxt::create_rpl_ctxt(
        tcx,
        rpl_query_impl::query_system(providers.queries, providers.extern_queries),
        lint_store,
        config,
    )
}

pub static DEFAULT_QUERY_PROVIDERS: LazyLock<Providers> = LazyLock::new(|| {
    let providers = &mut Providers::default();
    rpl_middle::provide(providers);
    rpl_mir_analysis::provide(providers);
    rpl_later_lint::provide(providers);
    *providers
});

*/
