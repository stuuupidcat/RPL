use rpl_context::PatCtxt;
use rpl_mir::pat::MirPattern;
use rpl_mir::{pat, CheckMirCtxt};
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::TyCtxt;
use rustc_span::Span;

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
                && let Some(matches) = CheckMirCtxt::new(self.tcx, body, &pattern.pattern).check()
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
                && let Some(matches) = CheckMirCtxt::new(self.tcx, body, &pattern.pattern).check()
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
                && let Some(matches) = CheckMirCtxt::new(self.tcx, body, &pattern.pattern).check()
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
    pattern: MirPattern<'pcx>,
    len: pat::Location,
    set_len: pat::Location,
    // vec: pat::LocalIdx,
    // vec: pat::Location,
}

#[rpl_macros::pattern_def]
fn pattern_trust_len(pcx: PatCtxt<'_>) -> PatternTrustLen<'_> {
    rpl! {
        fn $pattern (..) -> _ = mir! {
            meta!{
                $T:ty,
                $I:ty,
            }

            // let len: usize = <$I as std::iter::ExactSizeIterator>::len(iter);
            let iter: $I = _;
            let len: usize = std::iter::ExactSizeIterator::len(move iter);
            // let len: usize = std::iter::ExactSizeIterator::len(_);
            // let len: usize = _;
            let vec: &mut alloc::vec::Vec<$T> = _;
            let set_len: () = alloc::vec::Vec::set_len(move vec, copy len);
        }
    }

    PatternTrustLen {
        pattern,
        len: len_stmt,
        set_len: set_len_stmt,
        // vec: vec_local,
        // vec: vec_stmt,
    }
}

struct PatternFromRawParts<'pcx> {
    pattern: MirPattern<'pcx>,
    ptr: pat::Location,
    len: pat::Location,
    vec: pat::Location,
    slice: pat::Location,
}

#[rpl_macros::pattern_def]
fn pattern_uninitialized_slice(pcx: PatCtxt<'_>) -> PatternFromRawParts<'_> {
    rpl! {
        fn $pattern (..) -> _ = mir! {
            meta!{
                $T:ty,
            }

            let len: usize = _;
            let vec: alloc::vec::Vec<$T> = alloc::vec::Vec::with_capacity(_);
            let vec_ref: &alloc::vec::Vec<$T> = &vec;
            let ptr: *const $T = alloc::vec::Vec::as_ptr(move vec_ref);
            let slice: &[$T] = std::slice::from_raw_parts::<'_, $T>(move ptr, copy len);
        }
    }

    PatternFromRawParts {
        pattern,
        ptr: ptr_stmt,
        len: len_stmt,
        vec: vec_stmt,
        slice: slice_stmt,
    }
}

#[rpl_macros::pattern_def]
fn pattern_uninitialized_slice_mut(pcx: PatCtxt<'_>) -> PatternFromRawParts<'_> {
    rpl! {
        fn $pattern (..) -> _ = mir! {
            meta!{
                $T:ty,
            }

            let len: usize = _;
            let vec: alloc::vec::Vec<$T> = alloc::vec::Vec::with_capacity(_);
            let vec_ref: &mut alloc::vec::Vec<$T> = &mut vec;
            let ptr: *mut $T = alloc::vec::Vec::as_mut_ptr(move vec_ref);
            let slice: &mut [$T] = std::slice::from_raw_parts_mut::<'_, $T>(move ptr, copy len);
        }
    }

    PatternFromRawParts {
        pattern,
        ptr: ptr_stmt,
        // ptr: vec_stmt,
        len: len_stmt,
        vec: vec_stmt,
        slice: slice_stmt,
        // slice: vec_stmt,
    }
}
