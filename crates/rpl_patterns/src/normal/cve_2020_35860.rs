use rpl_context::PatCtxt;
use rpl_mir::{pat, CheckMirCtxt};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{self as hir};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::TyCtxt;
use rustc_span::{Span, Symbol};

#[instrument(level = "info", skip_all)]
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
            hir::ItemKind::Trait(hir::IsAuto::No, ..) | hir::ItemKind::Impl(_) | hir::ItemKind::Fn{..} => {},
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
        if self.tcx.is_mir_available(def_id) {
            let body = self.tcx.optimized_mir(def_id);

            let pattern = pattern_deref_null_pointer(self.pcx);
            for matches in CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.pattern, pattern.fn_pat).check() {
                let ptr = matches[pattern.ptr].span_no_inline(body);
                let from_ptr_func_call = matches[pattern.from_ptr_func_call].span_no_inline(body);
                debug!(?ptr, ?from_ptr_func_call);
                self.tcx.dcx().emit_err(crate::errors::DerefNullPointer {
                    ptr,
                    deref: from_ptr_func_call,
                });
            }
        }
        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
}

struct PatternDerefNullPointer<'pcx> {
    pattern: &'pcx pat::Pattern<'pcx>,
    fn_pat: &'pcx pat::Fn<'pcx>,
    ptr: pat::Location,
    from_ptr_func_call: pat::Location,
}

#[rpl_macros::pattern_def]
fn pattern_deref_null_pointer(pcx: PatCtxt<'_>) -> PatternDerefNullPointer<'_> {
    let ptr;
    let from_ptr_func_call;
    let pattern = rpl! {
        #[meta($T:ty)]
        struct $CBox {
            $ptr: *mut $T,
        }

        #[meta($T:ty)]
        fn $pattern(..) -> _ = mir! {
            let $self: &$CBox;
            #[export(ptr)]
            let $self_ptr: *mut $T = copy (*$self).$ptr;
            let $ptr: *const i8 = move $self_ptr as *const i8 (PtrToPtr);
            #[export(from_ptr_func_call)]
            _ = std::ffi::CStr::from_ptr::<'_>(move $ptr);
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    PatternDerefNullPointer {
        pattern,
        fn_pat,
        ptr,
        from_ptr_func_call,
    }
}
