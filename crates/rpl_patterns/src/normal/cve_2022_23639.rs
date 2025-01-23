use rpl_context::PatCtxt;
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::TyCtxt;
use rustc_span::{Span, Symbol};

use rpl_mir::{pat, CheckMirCtxt};

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
            hir::ItemKind::Trait(hir::IsAuto::No, ..) | hir::ItemKind::Impl(_) | hir::ItemKind::Fn(..) => {},
            _ => return,
        }
        intravisit::walk_item(self, item);
    }

    fn visit_fn(
        &mut self,
        _kind: intravisit::FnKind<'tcx>,
        _decl: &'tcx hir::FnDecl<'tcx>,
        _body_id: hir::BodyId,
        _span: Span,
        def_id: LocalDefId,
    ) -> Self::Result {
        if self.tcx.visibility(def_id).is_public() && self.tcx.is_mir_available(def_id) {
            let body = self.tcx.optimized_mir(def_id);
            let pattern = pattern_unsound_cast_between_u64_and_atomic_u64(self.pcx);
            for matches in CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.pattern, pattern.fn_pat).check() {
                let transmute = matches[pattern.transmute].span_no_inline(body);
                let src = matches[pattern.src].span_no_inline(body);
                self.tcx
                    .dcx()
                    .emit_err(crate::errors::UnsoundCastBetweenU64AndAtomicU64 { transmute, src });
            }
        }
    }
}

struct UnsoundCastBetweenU64AndAtomicU64<'pcx> {
    pattern: &'pcx pat::Pattern<'pcx>,
    fn_pat: &'pcx pat::Fn<'pcx>,
    transmute: pat::Location,
    src: pat::Location,
}

#[rpl_macros::pattern_def]
fn pattern_unsound_cast_between_u64_and_atomic_u64(pcx: PatCtxt<'_>) -> UnsoundCastBetweenU64AndAtomicU64<'_> {
    let transmute;
    let src;
    let pattern = rpl! {
        fn $pattern(..) -> _ = mir! {
            type AtomicU64 = std::sync::atomic::AtomicU64;

            #[export(src)]
            let src: *mut u64 = _;
            #[export(transmute)]
            let dst: *const AtomicU64 = move src as *const AtomicU64 (PtrToPtr);
        }
    };

    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();
    UnsoundCastBetweenU64AndAtomicU64 {
        pattern,
        fn_pat,
        transmute,
        src,
    }
}
