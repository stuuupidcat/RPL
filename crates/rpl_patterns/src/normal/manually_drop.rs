use rpl_context::PatCtxt;
use rpl_mir::{pat, CheckMirCtxt};
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::TyCtxt;
use rustc_span::{Span, Symbol};

#[instrument(level = "info", skip_all)]
pub fn check_item(tcx: TyCtxt<'_>, pcx: PatCtxt<'_>, item_id: hir::ItemId) {
    let item = tcx.hir().item(item_id);
    // let def_id = item_id.owner_id.def_id;
    let mut check_ctxt = CheckFnCtxt { tcx, pcx };
    check_ctxt.visit_item(item);
}

struct CheckFnCtxt<'pcx, 'tcx> {
    tcx: TyCtxt<'tcx>,
    pcx: PatCtxt<'pcx>,
}

impl<'tcx> Visitor<'tcx> for CheckFnCtxt<'_, 'tcx> {
    type NestedFilter = All;
    fn nested_visit_map(&mut self) -> Self::Map {
        self.tcx.hir()
    }

    #[instrument(level = "debug", skip_all, fields(?item.owner_id))]
    fn visit_item(&mut self, item: &'tcx hir::Item<'tcx>) -> Self::Result {
        match item.kind {
            hir::ItemKind::Trait(hir::IsAuto::No, hir::Safety::Safe, ..)
            | hir::ItemKind::Impl(_)
            | hir::ItemKind::Fn { .. } => {},
            _ => return,
        }
        intravisit::walk_item(self, item);
    }

    #[instrument(level = "info", skip_all, fields(?def_id))]
    fn visit_fn(
        &mut self,
        kind: intravisit::FnKind<'tcx>,
        decl: &'tcx hir::FnDecl<'tcx>,
        body_id: hir::BodyId,
        _span: Span,
        def_id: LocalDefId,
    ) -> Self::Result {
        // let attrs: Vec<_> = self
        //     .tcx
        //     .get_attrs_by_path(def_id.to_def_id(), &[Symbol::intern("rpl"), Symbol::intern("check")])
        //     .collect();
        // info!("attrs: {:?}", attrs);
        // if attrs.is_empty() {
        //     return;
        // }

        if self.tcx.is_mir_available(def_id) {
            let body = self.tcx.optimized_mir(def_id);

            macro_rules! check_mir {
                ($name:ident) => {{
                    let $name = $name(self.pcx);
                    for matches in CheckMirCtxt::new(self.tcx, self.pcx, body, $name.pattern, $name.fn_pat).check() {
                        let create = matches[$name.create].span_no_inline(body);
                        let fn_1 = $name.fn_1;
                        let fn_2 = $name.fn_2;
                        let call_1 = matches[$name.call_1].span_no_inline(body);
                        let call_2 = matches[$name.call_2].span_no_inline(body);
                        self.tcx.emit_node_span_lint(
                            crate::lints::BAD_MANUALLY_DROP_OPERATION_SEQUENCE,
                            self.tcx.local_def_id_to_hir_id(def_id),
                            call_2,
                            crate::errors::BadManuallyDropOperationSequence {
                                create,
                                call_1,
                                call_2,
                                fn_1,
                                fn_2,
                            },
                        );
                    }
                }};
            }

            check_mir!(double_drop);
            check_mir!(double_take);
            check_mir!(take_after_drop);
            check_mir!(drop_after_take);
            check_mir!(into_inner_after_drop);
            check_mir!(into_inner_after_take);
        }

        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
}

struct BadManuallyOperation<'pcx> {
    pattern: &'pcx pat::Pattern<'pcx>,
    fn_pat: &'pcx pat::Fn<'pcx>,
    create: pat::Location,
    fn_1: &'static str,
    fn_2: &'static str,
    call_1: pat::Location,
    call_2: pat::Location,
}

#[rpl_macros::pattern_def]
fn double_drop(pcx: PatCtxt<'_>) -> BadManuallyOperation<'_> {
    let create;
    let call_1;
    let call_2;
    let pattern = rpl! {
        #[meta($T: ty)]
        fn $pattern (..) -> _ = mir! {
            #[export(create)]
            let $manually_drop: core::mem::ManuallyDrop<$T> = _;

            let $mut_ref_1: &mut core::mem::ManuallyDrop<$T> = &mut $manually_drop;
            #[export(call_1)]
            let $drop_1: () = core::mem::ManuallyDrop::drop(copy $mut_ref_1);

            let $mut_ref_2: &mut core::mem::ManuallyDrop<$T> = &mut $manually_drop;
            #[export(call_2)]
            let $drop_2: () = core::mem::ManuallyDrop::drop(copy $mut_ref_2);
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    BadManuallyOperation {
        pattern,
        fn_pat,
        create,
        call_1,
        call_2,
        fn_1: "drop",
        fn_2: "drop",
    }
}

#[rpl_macros::pattern_def]
fn double_take(pcx: PatCtxt<'_>) -> BadManuallyOperation<'_> {
    let create;
    let call_1;
    let call_2;
    let pattern = rpl! {
        #[meta($T: ty)]
        fn $pattern (..) -> _ = mir! {
            #[export(create)]
            let $manually_drop: core::mem::ManuallyDrop<$T> = _;

            let $mut_ref_1: &mut core::mem::ManuallyDrop<$T> = &mut $manually_drop;
            #[export(call_1)]
            let $take_1: $T = core::mem::ManuallyDrop::take(copy $mut_ref_1);

            let $mut_ref_2: &mut core::mem::ManuallyDrop<$T> = &mut $manually_drop;
            #[export(call_2)]
            let $take_2: $T = core::mem::ManuallyDrop::take(copy $mut_ref_2);
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    BadManuallyOperation {
        pattern,
        fn_pat,
        create,
        call_1,
        call_2,
        fn_1: "take",
        fn_2: "take",
    }
}

#[rpl_macros::pattern_def]
fn drop_after_take(pcx: PatCtxt<'_>) -> BadManuallyOperation<'_> {
    let create;
    let call_1;
    let call_2;
    let pattern = rpl! {
        #[meta($T: ty)]
        fn $pattern (..) -> _ = mir! {
            #[export(create)]
            let $manually_drop: core::mem::ManuallyDrop<$T> = _;

            let $mut_ref_1: &mut core::mem::ManuallyDrop<$T> = &mut $manually_drop;
            #[export(call_1)]
            let $drop_1: () = core::mem::ManuallyDrop::drop(copy $mut_ref_1);

            let $mut_ref_2: &mut core::mem::ManuallyDrop<$T> = &mut $manually_drop;
            #[export(call_2)]
            let $take_2: $T = core::mem::ManuallyDrop::take(copy $mut_ref_2);
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    BadManuallyOperation {
        pattern,
        fn_pat,
        create,
        call_1,
        call_2,
        fn_1: "drop",
        fn_2: "take",
    }
}

#[rpl_macros::pattern_def]
fn take_after_drop(pcx: PatCtxt<'_>) -> BadManuallyOperation<'_> {
    let create;
    let call_1;
    let call_2;
    let pattern = rpl! {
        #[meta($T: ty)]
        fn $pattern (..) -> _ = mir! {
            #[export(create)]
            let $manually_drop: core::mem::ManuallyDrop<$T> = _;

            let $mut_ref_1: &mut core::mem::ManuallyDrop<$T> = &mut $manually_drop;
            #[export(call_1)]
            let $take_1: $T = core::mem::ManuallyDrop::take(copy $mut_ref_1);

            let $mut_ref_2: &mut core::mem::ManuallyDrop<$T> = &mut $manually_drop;
            #[export(call_2)]
            let $take_2: () = core::mem::ManuallyDrop::drop(copy $mut_ref_2);
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    BadManuallyOperation {
        pattern,
        fn_pat,
        create,
        call_1,
        call_2,
        fn_1: "take",
        fn_2: "drop",
    }
}

#[rpl_macros::pattern_def]
fn into_inner_after_take(pcx: PatCtxt<'_>) -> BadManuallyOperation<'_> {
    let create;
    let call_1;
    let call_2;
    let pattern = rpl! {
        #[meta($T: ty)]
        fn $pattern (..) -> _ = mir! {
            #[export(create)]
            let $manually_drop: core::mem::ManuallyDrop<$T> = _;

            let $mut_ref_1: &mut core::mem::ManuallyDrop<$T> = &mut $manually_drop;
            #[export(call_1)]
            let $take_1: $T = core::mem::ManuallyDrop::take(copy $mut_ref_1);

            let $manually_drop_2: core::mem::ManuallyDrop<$T> = move $manually_drop;
            #[export(call_2)]
            let $take_2: $T = core::mem::ManuallyDrop::into_inner(move $manually_drop_2);
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    BadManuallyOperation {
        pattern,
        fn_pat,
        create,
        call_1,
        call_2,
        fn_1: "into_inner",
        fn_2: "take",
    }
}

#[rpl_macros::pattern_def]
fn into_inner_after_drop(pcx: PatCtxt<'_>) -> BadManuallyOperation<'_> {
    let create;
    let call_1;
    let call_2;
    let pattern = rpl! {
        #[meta($T: ty)]
        fn $pattern (..) -> _ = mir! {
            #[export(create)]
            let $manually_drop: core::mem::ManuallyDrop<$T> = _;

            let $mut_ref_1: &mut core::mem::ManuallyDrop<$T> = &mut $manually_drop;
            #[export(call_1)]
            let $drop_1: () = core::mem::ManuallyDrop::drop(copy $mut_ref_1);

            let $manually_drop_2: core::mem::ManuallyDrop<$T> = move $manually_drop;
            #[export(call_2)]
            let $take_2: $T = core::mem::ManuallyDrop::into_inner(move $manually_drop_2);
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    BadManuallyOperation {
        pattern,
        fn_pat,
        create,
        call_1,
        call_2,
        fn_1: "into_inner",
        fn_2: "drop",
    }
}
