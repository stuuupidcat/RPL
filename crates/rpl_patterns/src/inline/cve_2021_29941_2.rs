#![allow(unused)] //FIXME: fix or remove unused items
use rpl_context::PatCtxt;
use rpl_mir::{CheckMirCtxt, pat};
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::TyCtxt;
use rustc_span::{Span, Symbol};

use crate::lints::{SLICE_FROM_RAW_PARTS_UNINITIALIZED, TRUST_EXACT_SIZE_ITERATOR};

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

            let pattern = pattern_trust_len_inlined(self.pcx);
            for matches in CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.pattern, pattern.fn_pat).check() {
                let len = matches[pattern.len].span_no_inline(body);
                let set_len = matches[pattern.set_len].span_no_inline(body);
                debug!(?len, ?set_len);
                self.tcx.emit_node_span_lint(
                    TRUST_EXACT_SIZE_ITERATOR,
                    self.tcx.local_def_id_to_hir_id(def_id),
                    set_len,
                    crate::errors::TrustExactSizeIterator {
                        len,
                        set_len,
                        fn_name: "Vec::set_len",
                    },
                );
            }

            /*
            let pattern = pattern_uninitialized_slice_inlined(self.pcx);
            for matches in CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.pattern, pattern.fn_pat).check() {
                let len = matches[pattern.len].span_no_inline(body);
                let ptr = matches[pattern.ptr].span_no_inline(body);
                let vec = matches[pattern.vec].span_no_inline(body);
                let slice = matches[pattern.slice].span_no_inline(body);

                debug!(?len, ?ptr, ?vec, ?slice);
                self.tcx.emit_node_span_lint(
                    SLICE_FROM_RAW_PARTS_UNINITIALIZED,
                    self.tcx.local_def_id_to_hir_id(def_id),
                    slice,
                    crate::errors::SliceFromRawPartsUninitialized {
                        len,
                        ptr,
                        vec,
                        slice,
                        fn_name: "from_raw_parts",
                    },
                );
            }

            let pattern = pattern_uninitialized_slice_mut_inlined(self.pcx);
            for matches in CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.pattern, pattern.fn_pat).check() {
                let len = matches[pattern.len].span_no_inline(body);
                let ptr = matches[pattern.ptr].span_no_inline(body);
                let vec = matches[pattern.vec].span_no_inline(body);
                let slice = matches[pattern.slice].span_no_inline(body);
                debug!(?len, ?ptr, ?vec, ?slice);

                self.tcx.emit_node_span_lint(
                    SLICE_FROM_RAW_PARTS_UNINITIALIZED,
                    self.tcx.local_def_id_to_hir_id(def_id),
                    slice,
                    crate::errors::SliceFromRawPartsUninitialized {
                        len,
                        ptr,
                        vec,
                        slice,
                        fn_name: "from_raw_parts_mut",
                    },
                );
            }
             */
        }
        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
}

struct PatternTrustLen<'pcx> {
    pattern: &'pcx pat::Pattern<'pcx>,
    fn_pat: &'pcx pat::Fn<'pcx>,
    len: pat::Location,
    set_len: pat::Location,
}

#[rpl_macros::pattern_def]
fn pattern_trust_len_inlined(pcx: PatCtxt<'_>) -> PatternTrustLen<'_> {
    let len;
    let set_len;
    let pattern = rpl! {
        #[meta($T:ty, $I:ty)]
        fn $pattern (..) -> _ = mir! {
            let $iter: $I = _;

            #[export(len)]
            let $len: usize = std::iter::ExactSizeIterator::len(move $iter);

            let $vec: &mut alloc::vec::Vec<$T> = _;

            // #[export(set_len)]
            // let set_len: () = alloc::vec::Vec::set_len(move vec, copy len);
            #[export(set_len)]
            ((*$vec).len) = copy $len;
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    PatternTrustLen {
        pattern,
        fn_pat,
        len,
        set_len,
    }
}

struct PatternFromRawParts<'pcx> {
    pattern: &'pcx pat::Pattern<'pcx>,
    fn_pat: &'pcx pat::Fn<'pcx>,
    ptr: pat::Location,
    len: pat::Location,
    vec: pat::Location,
    slice: pat::Location,
}

#[rpl_macros::pattern_def]
fn pattern_uninitialized_slice_inlined(pcx: PatCtxt<'_>) -> PatternFromRawParts<'_> {
    let ptr;
    let len;
    let vec;
    let slice;
    let pattern = rpl! {
        #[meta($T:ty)]
        fn $pattern (..) -> _ = mir! {
            #[export(len)]
            let $len: usize = _;

            // let vec: std::vec::Vec<$T> = std::vec::Vec::with_capacity(_);
            let $raw_vec_inner: alloc::raw_vec::RawVecInner = alloc::raw_vec::RawVecInner::with_capacity_in(_, _, _);
            let $raw_vec: alloc::raw_vec::RawVec<$T> = alloc::raw_vec::RawVec::<$T> {
                inner: move $raw_vec_inner,
                _marker: const std::marker::PhantomData::<$T>
            };
            #[export(vec)]
            let $vec: std::vec::Vec<$T> = std::vec::Vec::<$T> { buf: move $raw_vec, len: const 0_usize };

            let $vec_ref: &alloc::vec::Vec<$T> = &$vec;

            // #[export(ptr)]
            // let ptr: *const $T = alloc::vec::Vec::as_ptr(move vec_ref);
            let $vec_inner_non_null: alloc::raw_vec::RawVecInner = copy (*$vec_ref).buf.inner.ptr;
            #[export(ptr)]
            let $ptr: *const $T = copy $vec_inner_non_null as *const $T (Transmute);

            // let slice: &[$T] = std::slice::from_raw_parts::<'_, $T>(move ptr, copy len);
            let $slice_ptr: *const [$T] = *const [$T] from (copy $ptr, copy $len);
            #[export(slice)]
            let $slice: &[$T] = &*$slice_ptr;
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    PatternFromRawParts {
        pattern,
        fn_pat,
        ptr,
        len,
        vec,
        slice,
    }
}

#[rpl_macros::pattern_def]
fn pattern_uninitialized_slice_mut_inlined(pcx: PatCtxt<'_>) -> PatternFromRawParts<'_> {
    let ptr;
    let len;
    let vec;
    let slice;
    let pattern = rpl! {
        #[meta($T:ty)]
        fn $pattern (..) -> _ = mir! {
            // #[export(len)]
            // let len: usize = _;

            // let vec: std::vec::Vec<$T> = std::vec::Vec::with_capacity(_);
            let $raw_vec_inner: alloc::raw_vec::RawVecInner = alloc::raw_vec::RawVecInner::with_capacity_in(_, _, _);
            let $raw_vec: alloc::raw_vec::RawVec<$T> = alloc::raw_vec::RawVec::<$T> {
                inner: move $raw_vec_inner,
                _marker: const std::marker::PhantomData::<$T>
            };
            #[export(vec)]
            let $vec: std::vec::Vec<$T> = std::vec::Vec::<$T> { buf: move $raw_vec, len: const 0_usize };

            let $vec_ref: &mut alloc::vec::Vec<$T> = &mut $vec;

            // #[export(ptr)]
            // let ptr: *mut $T = alloc::vec::Vec::as_ptr(move vec_ref);
            let $vec_inner_non_null: std::ptr::NonNull<u8> = copy (*$vec_ref).buf.inner.ptr.pointer;
            #[export(ptr)]
            let $ptr: *mut $T = copy $vec_inner_non_null as *mut $T (Transmute);

            // #[export(slice)]
            // let slice: &mut [$T] = std::slice::from_raw_parts_mut::<'_, $T>(move ptr, copy len);

            // FIXME: if this statement is put in the beginning, it may fail to match the code where
            // `Vec::with_capacity` is called in advance of `ExactSizeIterator::len`.
            // See `rpl_mir::matches::match_block_ends_with` for more details.
            #[export(len)]
            let $len: usize = _;

            let $slice_ptr: *mut [$T] = *mut [$T] from (copy $ptr, copy $len);
            #[export(slice)]
            let $slice: &mut [$T] = &mut *$slice_ptr;
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    PatternFromRawParts {
        pattern,
        fn_pat,
        ptr,
        len,
        vec,
        slice,
    }
}
