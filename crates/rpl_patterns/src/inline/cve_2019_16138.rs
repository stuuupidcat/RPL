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
        if self.tcx.is_mir_available(def_id) {
            let body = self.tcx.optimized_mir(def_id);
            let pattern_cast = pattern_set_len_uninitialized_inlined(self.pcx);
            for matches in
                CheckMirCtxt::new(self.tcx, self.pcx, body, pattern_cast.pattern, pattern_cast.fn_pat).check()
            {
                let vec = matches[pattern_cast.vec].span_no_inline(body);
                let set_len = matches[pattern_cast.set_len].span_no_inline(body);
                debug!(?vec, ?set_len);
                self.tcx
                    .dcx()
                    .emit_err(crate::errors::SetLenUninitialized { vec, set_len });
            }
        }
        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
}

struct Pattern<'pcx> {
    pattern: &'pcx pat::Pattern<'pcx>,
    fn_pat: &'pcx pat::Fn<'pcx>,
    vec: pat::Location,
    set_len: pat::Location,
}

#[rpl_macros::pattern_def]
fn pattern_set_len_uninitialized_inlined(pcx: PatCtxt<'_>) -> Pattern<'_> {
    let vec;
    let set_len;
    let pattern = rpl! {
        #[meta($T:ty)]
        fn $pattern (..) -> _ = mir! {
            // let vec: std::vec::Vec<$T> = std::vec::Vec::with_capacity(_);
            let $raw_vec_inner: alloc::raw_vec::RawVecInner = alloc::raw_vec::RawVecInner::with_capacity_in(_, _, _);
            let $raw_vec: alloc::raw_vec::RawVec<$T> = alloc::raw_vec::RawVec::<$T> {
                inner: move $raw_vec_inner,
                _marker: const std::marker::PhantomData::<$T>
            };
            #[export(vec)]
            let $vec: std::vec::Vec<$T> = std::vec::Vec::<$T> { buf: move $raw_vec, len: const 0_usize };

            let $vec_ref: &mut std::vec::Vec<$T> = &mut $vec;

            #[export(set_len)]
            // _ = std::vec::Vec::set_len(move vec_ref, _);
            ((*$vec_ref).len) = _;
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    Pattern {
        pattern,
        fn_pat,
        vec,
        set_len,
    }
}
