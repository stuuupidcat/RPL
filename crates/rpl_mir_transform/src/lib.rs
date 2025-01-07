#![feature(rustc_private)]
#![feature(box_patterns)]

extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_mir_build;
extern crate rustc_mir_transform;
extern crate rustc_session;
extern crate rustc_trait_selection;
#[macro_use]
extern crate tracing;

use rustc_data_structures::steal::Steal;
use rustc_hir::def_id::LocalDefId;
use rustc_middle::mir::{Body, TerminatorKind, START_BLOCK};
use rustc_middle::ty::{self, TyCtxt, TypeVisitableExt};
use rustc_middle::util::Providers;
use rustc_mir_transform::run_analysis_to_runtime_passes;
use rustc_trait_selection::traits;

mod unify_comparison;

pub fn provide(providers: &mut Providers) {
    providers.queries.mir_drops_elaborated_and_const_checked = mir_drops_elaborated_and_const_checked;
}

/// See <https://doc.rust-lang.org/nightly/nightly-rustc/src/rustc_mir_transform/lib.rs.html#479-554>
///
/// This is not the most suitable query to override, but it's the only one that is easy to work
/// with.
///
/// Obtain just the main MIR (no promoteds) and run some cleanups on it. This also runs
/// mir borrowck *before* doing so in order to ensure that borrowck can be run and doesn't
/// end up missing the source MIR due to stealing happening.
fn mir_drops_elaborated_and_const_checked(tcx: TyCtxt<'_>, def: LocalDefId) -> &Steal<Body<'_>> {
    if tcx.is_coroutine(def.to_def_id()) {
        tcx.ensure_with_value().mir_coroutine_witnesses(def);
    }

    // We only need to borrowck non-synthetic MIR.
    let tainted_by_errors = if !tcx.is_synthetic_mir(def) {
        tcx.mir_borrowck(def).tainted_by_errors
    } else {
        None
    };

    let is_fn_like = tcx.def_kind(def).is_fn_like();
    if is_fn_like {
        //FIXME: We can't use `MirPass` trait because it's private, so the call graph is always computed.
        // // Do not compute the mir call graph without said call graph actually being used.
        // pm::should_run_pass(tcx, &inline::Inline)
        tcx.ensure_with_value()
            .mir_inliner_callees(ty::InstanceKind::Item(def.to_def_id()));
    }

    let (body, _) = tcx.mir_promoted(def);
    let mut body = body.steal();

    if let Some(error_reported) = tainted_by_errors {
        body.tainted_by_errors = Some(error_reported);
    }

    // Check if it's even possible to satisfy the 'where' clauses
    // for this item.
    //
    // This branch will never be taken for any normal function.
    // However, it's possible to `#!feature(trivial_bounds)]` to write
    // a function with impossible to satisfy clauses, e.g.:
    // `fn foo() where String: Copy {}`
    //
    // We don't usually need to worry about this kind of case,
    // since we would get a compilation error if the user tried
    // to call it. However, since we optimize even without any
    // calls to the function, we need to make sure that it even
    // makes sense to try to evaluate the body.
    //
    // If there are unsatisfiable where clauses, then all bets are
    // off, and we just give up.
    //
    // We manually filter the predicates, skipping anything that's not
    // "global". We are in a potentially generic context
    // (e.g. we are evaluating a function without instantiating generic
    // parameters, so this filtering serves two purposes:
    //
    // 1. We skip evaluating any predicates that we would
    // never be able prove are unsatisfiable (e.g. `<T as Foo>`
    // 2. We avoid trying to normalize predicates involving generic
    // parameters (e.g. `<T as Foo>::MyItem`). This can confuse
    // the normalization code (leading to cycle errors), since
    // it's usually never invoked in this way.
    let predicates = tcx
        .predicates_of(body.source.def_id())
        .predicates
        .iter()
        .filter_map(|(p, _)| if p.is_global() { Some(*p) } else { None });
    if traits::impossible_predicates(tcx, traits::elaborate(tcx, predicates).collect()) {
        trace!("found unsatisfiable predicates for {:?}", body.source);
        // Clear the body to only contain a single `unreachable` statement.
        let bbs = body.basic_blocks.as_mut();
        bbs.raw.truncate(1);
        bbs[START_BLOCK].statements.clear();
        bbs[START_BLOCK].terminator_mut().kind = TerminatorKind::Unreachable;
        body.var_debug_info.clear();
        body.local_decls.raw.truncate(body.arg_count + 1);
    }

    run_analysis_to_runtime_passes(tcx, &mut body);
    unify_comparison::unify_comparison(tcx, &mut body);

    // Now that drop elaboration has been performed, we can check for
    // unconditional drop recursion.
    rustc_mir_build::lints::check_drop_recursion(tcx, &body);

    tcx.alloc_steal_mir(body)
}
