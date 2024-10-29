use rpl_mir::pat::PatternsBuilder;
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::TyCtxt;
use rustc_span::Span;

use rpl_mir::{pat, CheckMirCtxt};

#[instrument(level = "info", skip(tcx))]
pub fn check_item(tcx: TyCtxt<'_>, item_id: hir::ItemId) {
    let item = tcx.hir().item(item_id);
    // let def_id = item_id.owner_id.def_id;
    let mut check_ctxt = CheckFnCtxt { tcx };
    check_ctxt.visit_item(item);
}

struct CheckFnCtxt<'tcx> {
    tcx: TyCtxt<'tcx>,
}

impl<'tcx> Visitor<'tcx> for CheckFnCtxt<'tcx> {
    type NestedFilter = All;
    fn nested_visit_map(&mut self) -> Self::Map {
        self.tcx.hir()
    }

    fn visit_item(&mut self, i: &'tcx rustc_hir::Item<'tcx>) -> Self::Result {
        match i.kind {
            hir::ItemKind::Fn(..) => {},
            _ => return,
        }
        intravisit::walk_item(self, i);
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
            #[allow(irrefutable_let_patterns)]
            if let mut patterns_reversed_paras = PatternsBuilder::new(&self.tcx.arena.dropless)
                && let pattern_reversed_para = pattern_reversed_para(&mut patterns_reversed_paras)
                && let Some(matches) = CheckMirCtxt::new(self.tcx, body, &patterns_reversed_paras.build()).check()
                && let Some(from_raw_parts) = matches[pattern_reversed_para.from_raw_parts]
                && let span = from_raw_parts.span_no_inline(body)
            {
                debug!(?span);
                self.tcx.dcx().emit_err(crate::errors::MisorderedParameters { span });
            }
        }
    }
}

struct PatternMisorderedParam {
    from_raw_parts: pat::Location,
}

#[rpl_macros::mir_pattern]
fn pattern_reversed_para(patterns: &mut pat::PatternsBuilder<'_>) -> PatternMisorderedParam {
    mir! {
        meta!{$T:ty}

        let from_vec: alloc::vec::Vec<u8, alloc::alloc::Global> = _;
        let to_vec: alloc::vec::Vec<$T, alloc::alloc::Global>;
        let to_vec_cap: usize;
        let mut from_vec_cap: usize;
        let mut tsize: usize;
        let to_vec_len: usize;
        let mut from_vec_len: usize;
        let mut from_vec_ptr: core::ptr::non_null::NonNull<u8>;
        let mut to_raw_vec: alloc::raw_vec::RawVec<$T, alloc::alloc::Global>;
        let mut to_raw_vec_inner: alloc::raw_vec::RawVecInner<alloc::alloc::Global>;
        let mut to_vec_wrapped_len: alloc::raw_vec::Cap;
        let mut from_vec_unique_ptr: core::ptr::unique::Unique<u8>;

        from_vec_ptr = copy from_vec.buf.inner.ptr.pointer;
        from_vec_cap = copy from_vec.buf.inner.cap.0;
        tsize = SizeOf($T);
        to_vec_cap = Div(move from_vec_cap, copy tsize);
        from_vec_len = copy from_vec.len;
        to_vec_len = Div(move from_vec_len, copy tsize);
        to_vec_wrapped_len = #[ctor] alloc::raw_vec::Cap(copy to_vec_len);
        from_vec_unique_ptr = core::ptr::unique::Unique<u8> {
            pointer: copy from_vec_ptr,
            _marker: core::marker::PhantomData<u8>,
        };
        to_raw_vec_inner = alloc::raw_vec::RawVecInner<alloc::alloc::Global> {
            ptr: move from_vec_unique_ptr,
            cap: copy to_vec_wrapped_len,
            alloc: alloc::alloc::Global,
        };
        to_raw_vec = alloc::raw_vec::RawVec<$T, alloc::alloc::Global> {
            inner: move to_raw_vec_inner,
            _marker: core::marker::PhantomData<$T>,
        };
        to_vec = alloc::vec::Vec<$T, alloc::alloc::Global> {
            buf: move to_raw_vec,
            len: copy to_vec_cap,
        };
    }

    PatternMisorderedParam {
        from_raw_parts: to_vec_stmt,
    }
}
