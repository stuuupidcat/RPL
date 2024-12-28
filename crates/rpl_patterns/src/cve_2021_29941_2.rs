use rpl_context::PatCtxt;
use rpl_mir::{pat, CheckMirCtxt};
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::TyCtxt;
use rustc_span::{Span, Symbol};

#[instrument(level = "info", skip(tcx, pcx))]
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
            | hir::ItemKind::Fn(..) => {},
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
            #[allow(irrefutable_let_patterns)]
            if let pattern = pattern_trust_len(self.pcx)
                && let Some(matches) = CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.fn_pat).check()
                && let Some(len) = matches[pattern.len]
                && let len = len.span_no_inline(body)
                && let Some(set_len) = matches[pattern.set_len]
                && let set_len = set_len.span_no_inline(body)
            {
                debug!(?len, ?set_len);
                self.tcx
                    .dcx()
                    .emit_err(crate::errors::TrustExactSizeIterator { len, set_len });
            }
            #[allow(irrefutable_let_patterns)]
            if let pattern = pattern_uninitialized_slice(self.pcx)
                && let Some(matches) = CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.fn_pat).check()
                && let Some(len) = matches[pattern.len]
                && let len = len.span_no_inline(body)
                && let Some(ptr) = matches[pattern.ptr]
                && let ptr = ptr.span_no_inline(body)
                && let Some(vec) = matches[pattern.vec]
                && let vec = vec.span_no_inline(body)
                && let Some(slice) = matches[pattern.slice]
                && let slice = slice.span_no_inline(body)
            {
                debug!(?len, ?ptr, ?vec, ?slice);
                self.tcx.dcx().emit_err(crate::errors::SliceFromRawPartsUninitialized {
                    len,
                    ptr,
                    vec,
                    slice,
                    fn_name: "std::slice::from_raw_parts",
                });
            }
            #[allow(irrefutable_let_patterns)]
            if let pattern = pattern_uninitialized_slice_mut(self.pcx)
                && let Some(matches) = CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.fn_pat).check()
                && let Some(len) = matches[pattern.len]
                && let len = len.span_no_inline(body)
                && let Some(ptr) = matches[pattern.ptr]
                && let ptr = ptr.span_no_inline(body)
                && let Some(vec) = matches[pattern.vec]
                && let vec = vec.span_no_inline(body)
                && let Some(slice) = matches[pattern.slice]
                && let slice = slice.span_no_inline(body)
            {
                debug!(?len, ?ptr, ?vec, ?slice);
                self.tcx.dcx().emit_err(crate::errors::SliceFromRawPartsUninitialized {
                    len,
                    ptr,
                    vec,
                    slice,
                    fn_name: "std::slice::from_raw_parts_mut",
                });
            }
        }
        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
}

struct PatternTrustLen<'pcx> {
    fn_pat: &'pcx pat::Fn<'pcx>,
    len: pat::Location,
    set_len: pat::Location,
}

#[rpl_macros::pattern_def]
fn pattern_trust_len(pcx: PatCtxt<'_>) -> PatternTrustLen<'_> {
    let len;
    let set_len;
    let pattern = rpl! {
        #[meta($T:ty, $I:ty)]
        fn $pattern (..) -> _ = mir! {
            // let len: usize = <$I as std::iter::ExactSizeIterator>::len(iter);
            let iter: $I = _;
            #[export(len)]
            let len: usize = std::iter::ExactSizeIterator::len(move iter);
            // let len: usize = std::iter::ExactSizeIterator::len(_);
            // let len: usize = _;
            let vec: &mut alloc::vec::Vec<$T> = _;
            #[export(set_len)]
            let set_len: () = alloc::vec::Vec::set_len(move vec, copy len);
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    PatternTrustLen { fn_pat, len, set_len }
}

struct PatternFromRawParts<'pcx> {
    fn_pat: &'pcx pat::Fn<'pcx>,
    ptr: pat::Location,
    len: pat::Location,
    vec: pat::Location,
    slice: pat::Location,
}

#[rpl_macros::pattern_def]
fn pattern_uninitialized_slice(pcx: PatCtxt<'_>) -> PatternFromRawParts<'_> {
    let ptr;
    let len;
    let vec;
    let slice;
    let pattern = rpl! {
        #[meta($T:ty)]
        fn $pattern (..) -> _ = mir! {
            #[export(len)]
            let len: usize = _;
            #[export(vec)]
            let vec: alloc::vec::Vec<$T> = alloc::vec::Vec::with_capacity(_);
            let vec_ref: &alloc::vec::Vec<$T> = &vec;
            #[export(ptr)]
            let ptr: *const $T = alloc::vec::Vec::as_ptr(move vec_ref);
            #[export(slice)]
            let slice: &[$T] = std::slice::from_raw_parts::<'_, $T>(move ptr, copy len);
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    PatternFromRawParts {
        fn_pat,
        ptr,
        len,
        vec,
        slice,
    }
}

#[rpl_macros::pattern_def]
fn pattern_uninitialized_slice_mut(pcx: PatCtxt<'_>) -> PatternFromRawParts<'_> {
    let ptr;
    let len;
    let vec;
    let slice;
    let pattern = rpl! {
        #[meta($T:ty)]
        fn $pattern (..) -> _ = mir! {
            #[export(len)]
            let len: usize = _;
            #[export(vec)]
            let vec: alloc::vec::Vec<$T> = alloc::vec::Vec::with_capacity(_);
            let vec_ref: &mut alloc::vec::Vec<$T> = &mut vec;
            #[export(ptr)]
            let ptr: *mut $T = alloc::vec::Vec::as_mut_ptr(move vec_ref);
            #[export(slice)]
            let slice: &mut [$T] = std::slice::from_raw_parts_mut::<'_, $T>(move ptr, copy len);
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    PatternFromRawParts {
        fn_pat,
        ptr,
        len,
        vec,
        slice,
    }
}
