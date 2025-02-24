use rpl_context::PatCtxt;
use rpl_mir::{pat, CheckMirCtxt};
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::TyCtxt;
use rustc_span::{Span, Symbol};
use std::ops::Not;

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
        if kind.header().is_none_or(|header| header.is_unsafe().not()) && self.tcx.is_mir_available(def_id) {
            let body = self.tcx.optimized_mir(def_id);
            let pattern = pattern_from_raw_parts_iter(self.pcx);
            for matches in CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.pattern, pattern.fn_pat).check() {
                let fn_name = pattern.fn_name;
                let slice = matches[pattern.slice].span_no_inline(body);
                let len = matches[pattern.len].span_no_inline(body);
                let vec = matches[pattern.vec].span_no_inline(body);
                let ptr = matches[pattern.ptr].span_no_inline(body);
                self.tcx.dcx().emit_err(crate::errors::SliceFromRawPartsUninitialized {
                    fn_name,
                    len,
                    vec,
                    ptr,
                    slice,
                });
            }
        }
        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
}

struct PatternFromRawParts<'pcx> {
    pattern: &'pcx pat::Pattern<'pcx>,
    fn_pat: &'pcx pat::Fn<'pcx>,
    slice: pat::Location,
    vec: pat::Location,
    len: pat::Location,
    ptr: pat::Location,
    fn_name: &'static str,
}

#[rpl_macros::pattern_def]
fn pattern_from_raw_parts_iter(pcx: PatCtxt<'_>) -> PatternFromRawParts<'_> {
    let vec;
    let len;
    let ptr;
    let slice;
    let pattern = rpl! {
        #[meta($T:ty)]
        fn $pattern (..) -> _ = mir! {
            #[export(vec)]
            let $src: alloc::vec::Vec<$T> = _; // _1
            let $src_ref_1: &alloc::vec::Vec<$T> = &$src; // _3
            #[export(len)]
            // let $len: usize = _; // _2
            let $len: usize = alloc::vec::Vec::len(move $src_ref_1); // _2
            let $src_ref_2: &mut alloc::vec::Vec<$T> = &mut $src; // _7
            #[export(ptr)]
            let $ptr: *mut $T = alloc::vec::Vec::as_mut_ptr(move $src_ref_2); // _6
            let $len_1: isize = copy $len as isize (IntToInt); // _8
            let $ptr_1: *mut $T = mut_ptr::offset(move $ptr, move $len_1); // _5
            let $src_ref_2: &alloc::vec::Vec<$T> = &$src; // _11
            let $capacity: usize = alloc::vec::Vec::capacity(move $src_ref_2); // _10
            let $slice_len: usize = Sub(move $capacity, copy $len); // _9

            // let $ptr: *mut $T = _; // _6
            // let $slice_len: usize = _; // _9
            #[export(slice)]
            let $slice: &mut [$T] = core::slice::from_raw_parts_mut::<'_, u8>(move $ptr_1, move $slice_len); // _4
            // let $slice: &mut [$T] = _; // _4
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    PatternFromRawParts {
        pattern,
        fn_pat,
        fn_name: "std::slice::from_raw_parts_mut",
        vec,
        len,
        ptr,
        slice,
    }
}
