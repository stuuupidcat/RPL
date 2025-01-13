use rpl_context::PatCtxt;
use rpl_match::MatchAdtCtxt;
use rpl_mir::{pat, CheckMirCtxt};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{self as hir};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::TyCtxt;
use rustc_span::{Span, Symbol};

#[instrument(level = "info", skip(tcx, pcx))]
pub fn check_item(tcx: TyCtxt<'_>, pcx: PatCtxt<'_>, item_id: hir::ItemId) {
    let item = tcx.hir().item(item_id);
    // let def_id = item_id.owner_id.def_id;
    let mut check_ctxt = CheckFnCtxt::new(tcx, pcx);
    check_ctxt.visit_item(item);
}

struct CheckFnCtxt<'pcx, 'tcx> {
    tcx: TyCtxt<'tcx>,
    pcx: PatCtxt<'pcx>,
}

impl<'pcx, 'tcx> CheckFnCtxt<'pcx, 'tcx> {
    fn new(tcx: TyCtxt<'tcx>, pcx: PatCtxt<'pcx>) -> Self {
        Self { tcx, pcx }
    }
}

impl<'tcx> Visitor<'tcx> for CheckFnCtxt<'_, 'tcx> {
    type NestedFilter = All;
    fn nested_visit_map(&mut self) -> Self::Map {
        self.tcx.hir()
    }

    #[instrument(level = "debug", skip_all, fields(?item.owner_id))]
    fn visit_item(&mut self, item: &'tcx hir::Item<'tcx>) -> Self::Result {
        match item.kind {
            hir::ItemKind::Trait(hir::IsAuto::No, ..) | hir::ItemKind::Impl(_) | hir::ItemKind::Fn(..) => {},
            hir::ItemKind::Struct(..) => {
                #[allow(irrefutable_let_patterns)]
                if let (pat, adt_pat) = pattern_cell_t(self.pcx)
                    && let Some(adt_match) = MatchAdtCtxt::new(self.tcx, self.pcx, pat, adt_pat)
                        .match_adt(self.tcx.adt_def(item.owner_id.def_id))
                {
                    #[expect(rustc::untranslatable_diagnostic)]
                    #[expect(rustc::diagnostic_outside_of_impl)]
                    self.tcx.dcx().span_note(
                        self.tcx.def_span(adt_match.adt.did()),
                        format!("Adt pattern `{adt_pat}` matched"),
                    );
                }
            },
            _ => return,
        }
        intravisit::walk_item(self, item);
    }

    #[instrument(level = "info", skip_all, fields(?def_id))]
    #[allow(unused_variables)]
    fn visit_fn(
        &mut self,
        kind: intravisit::FnKind<'tcx>,
        decl: &'tcx hir::FnDecl<'tcx>,
        body_id: hir::BodyId,
        span: Span,
        def_id: LocalDefId,
    ) -> Self::Result {
        if self.tcx.is_mir_available(def_id) {
            let body = self.tcx.optimized_mir(def_id);

            #[allow(irrefutable_let_patterns)]
            if let pattern_offset_by_len = pattern_rc_unsafe_cell_get_mut(self.pcx)
                && let Some(matches) = CheckMirCtxt::new(
                    self.tcx,
                    self.pcx,
                    body,
                    pattern_offset_by_len.pattern,
                    pattern_offset_by_len.fn_pat,
                )
                .check()
                && let Some(get_mut) = matches[pattern_offset_by_len.get_mut]
                && let get_mut = get_mut.span_no_inline(body)
            {
                debug!(?get_mut);
                self.tcx.dcx().emit_err(crate::errors::GetMutInRcUnsafeCell { get_mut });
            }
        }
        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
}

#[rpl_macros::pattern_def]
fn pattern_cell_t(pcx: PatCtxt<'_>) -> (&pat::Pattern<'_>, &pat::Adt<'_>) {
    let pattern = rpl! {
        #[meta($T:ty)]
        struct $CellT {
            $inner: alloc::rc::Rc<core::cell::UnsafeCell<$T>>,
        }
    };
    let adt_pat = pattern.get_adt(Symbol::intern("CellT")).unwrap();
    (pattern, adt_pat)
}

struct PatternRcUnsafeCellGetMut<'pcx> {
    pattern: &'pcx pat::Pattern<'pcx>,
    fn_pat: &'pcx pat::Fn<'pcx>,
    get_mut: pat::Location,
}

#[rpl_macros::pattern_def]
fn pattern_rc_unsafe_cell_get_mut(pcx: PatCtxt<'_>) -> PatternRcUnsafeCellGetMut<'_> {
    let get_mut;
    let pattern: &pat::Pattern<'_> = rpl! {
        #[meta($T:ty, $CellT:ty = |_tcx, _paramse_env, ty| ty.is_adt())]
        fn $pattern(..) -> _ = mir! {
            type UnsafeCellT = core::cell::UnsafeCell::<$T>;
            type RcUnsafeCellT = alloc::rc::Rc::<core::cell::UnsafeCell::<$T>>;
            type NonNullRcInnerUnsafeCellT = core::ptr::NonNull::<alloc::rc::RcInner::<core::cell::UnsafeCell::<$T>>>;
            type RcInnerUnsafeCellT = alloc::rc::RcInner::<core::cell::UnsafeCell::<$T>>;

            let self: &mut $CellT;
            let inner_ref: &RcUnsafeCellT = &((*self).inner);
            let inner_ptr: NonNullRcInnerUnsafeCellT = copy ((*inner_ref).ptr);
            let const_ptr: *const RcInnerUnsafeCellT = copy(inner_ptr.pointer);
            let unsafe_cell: &UnsafeCellT = &((*const_ptr).value);
            let unsafe_cell_ptr: *const UnsafeCellT = &raw const (*unsafe_cell);
            let value_ptr: *mut $T = copy unsafe_cell_ptr as *mut $T (PtrToPtr);
            #[export(get_mut)]
            let value_mut_ref: &mut $T = &mut (*value_ptr);
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    PatternRcUnsafeCellGetMut {
        pattern,
        fn_pat,
        get_mut,
    }
}
