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
extern crate tracing;

use rustc_data_structures::steal::Steal;
use rustc_hir::def_id::LocalDefId;
use rustc_middle::mir::Body;
use rustc_middle::ty::{self, TyCtxt};
use rustc_middle::util::Providers;
use rustc_mir_transform::run_analysis_to_runtime_passes;

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
        tcx.ensure_done().mir_coroutine_witnesses(def);
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
        tcx.ensure_done()
            .mir_inliner_callees(ty::InstanceKind::Item(def.to_def_id()));
    }

    let (body, _) = tcx.mir_promoted(def);
    let mut body = body.steal();

    if let Some(error_reported) = tainted_by_errors {
        body.tainted_by_errors = Some(error_reported);
    }

    run_analysis_to_runtime_passes(tcx, &mut body);
    unify_comparison::unify_comparison(tcx, &mut body);

    tcx.alloc_steal_mir(body)
}
