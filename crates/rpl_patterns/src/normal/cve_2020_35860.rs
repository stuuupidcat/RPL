use rpl_context::PatCtxt;
use rpl_mir::{pat, CheckMirCtxt};
use rustc_errors::MultiSpan;
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
            hir::ItemKind::Trait(hir::IsAuto::No, ..) | hir::ItemKind::Impl(_) | hir::ItemKind::Fn(..) => {},
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
        span: Span,
        def_id: LocalDefId,
    ) -> Self::Result {
        if self.tcx.is_mir_available(def_id) {
            let body = self.tcx.optimized_mir(def_id);

            let pattern = pattern_offset_by_len(self.pcx);
            for matches in CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.pattern, pattern.fn_pat).check() {
                let read = matches[pattern.read].span_no_inline(body);
                let ptr = matches[pattern.ptr].span_no_inline(body);
                let len = matches[pattern.len].span_no_inline(body);
                debug!(?ptr, ?read, ?len);
                let len_local = self
                    .tcx
                    .sess
                    .source_map()
                    .span_to_snippet(len)
                    .unwrap_or_else(|_| "{expr}".to_string());
                self.tcx.dcx().emit_err(crate::errors::OffsetByOne {
                    read,
                    ptr,
                    len,
                    len_local,
                });
            }
        }
        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
}

struct PatternDerefNullPointer<'pcx> {
    pattern: &'pcx pat::Pattern<'pcx>,
    fn_pat: &'pcx pat::Fn<'pcx>,
    len: pat::Location,
    ptr: pat::Location,
    read: pat::Location,
}

#[rpl_macros::pattern_def]
fn pattern_offset_by_len(pcx: PatCtxt<'_>) -> PatternOffsetByLen<'_> {
    let len;
    let ptr;
    let read;
    let pattern = rpl! {
        #[meta($T:ty)]
        struct $CBox {
            $ptr: *mut $T,
        }

        #[meta($T:ty)]
        fn $pattern(..) -> _ = mir! {
            let self: & $CBox;
            #[export(len)]
            let ptr: *mut $T = copy (*self).$ptr;
            let len_isize: isize = move len as isize (IntToInt);
            let base: *mut $T = copy (*self).$mem;
            #[export(ptr)]
            let ptr_mut: *mut $T = Offset(copy base, copy len_isize);
            let ptr: *const $T = copy ptr_mut as *const $T (PtrToPtr);
            #[export(read)]
            let elem: $T = copy (*ptr);
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    PatternOffsetByLen {
        pattern,
        fn_pat,
        len,
        ptr,
        read,
    }
}
