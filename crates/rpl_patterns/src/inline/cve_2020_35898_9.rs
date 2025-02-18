use rpl_context::PatCtxt;
use rpl_mir::{CheckMirCtxt, pat};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{self as hir};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::TyCtxt;
use rustc_span::{Span, Symbol};

use crate::lints::GET_MUT_IN_RC_UNSAFECELL;

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
            hir::ItemKind::Trait(hir::IsAuto::No, ..) | hir::ItemKind::Impl(_) | hir::ItemKind::Fn { .. } => {},
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

            let pattern = pattern_rc_unsafe_cell_get_mut(self.pcx);
            for matches in CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.pattern, pattern.fn_pat).check() {
                let get_mut = matches[pattern.get_mut].span_no_inline(body);
                debug!(?get_mut);
                self.tcx.emit_node_span_lint(
                    GET_MUT_IN_RC_UNSAFECELL,
                    self.tcx.local_def_id_to_hir_id(def_id),
                    get_mut,
                    crate::errors::GetMutInRcUnsafeCell { get_mut },
                );
            }
        }
        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
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
        #[meta($T:ty)]
        struct $CellT {
            $inner: alloc::rc::Rc<core::cell::UnsafeCell<$T>>,
        }

        #[meta($T:ty)]
        fn $pattern(..) -> _ = mir! {
            type UnsafeCellT = core::cell::UnsafeCell::<$T>;
            type RcUnsafeCellT = alloc::rc::Rc::<core::cell::UnsafeCell::<$T>>;
            type NonNulArcInnerUnsafeCellT = core::ptr::NonNull::<alloc::rc::RcInner::<core::cell::UnsafeCell::<$T>>>;
            type RcInnerUnsafeCellT = alloc::rc::RcInner::<core::cell::UnsafeCell::<$T>>;

            let $self: &mut $CellT;
            let $inner_ref: &RcUnsafeCellT = &((*$self).$inner);
            let $inner_ptr: NonNulArcInnerUnsafeCellT = copy ((*$inner_ref).ptr);
            let $const_ptr: *const RcInnerUnsafeCellT = copy $inner_ptr as *const RcInnerUnsafeCellT (Transmute);
            let $unsafe_cell: &UnsafeCellT = &((*$const_ptr).value);
            let $unsafe_cell_ptr: *const UnsafeCellT = &raw const (*$unsafe_cell);
            let $value_ptr: *mut $T = copy $unsafe_cell_ptr as *mut $T (PtrToPtr);
            #[export(get_mut)]
            let $value_mut_ref: &mut $T = &mut (*$value_ptr);
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    PatternRcUnsafeCellGetMut {
        pattern,
        fn_pat,
        get_mut,
    }
}
