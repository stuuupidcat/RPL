use rpl_mir::pat::TyVarIdx;
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::TyCtxt;
use rustc_span::{Span, Symbol};

use rpl_context::{pat, PatCtxt};
use rpl_mir::CheckMirCtxt;

use crate::lints::USE_AFTER_MOVE;

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

    fn visit_item(&mut self, item: &'tcx hir::Item<'tcx>) -> Self::Result {
        match item.kind {
            hir::ItemKind::Trait(hir::IsAuto::No, hir::Safety::Safe, ..)
            | hir::ItemKind::Impl(_)
            | hir::ItemKind::Fn { .. } => {},
            _ => return,
        }
        intravisit::walk_item(self, item);
    }

    #[instrument(level = "info", skip(self, kind, decl, _span))]
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
            let pattern = pattern(self.pcx);
            for matches in CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.pattern, pattern.fn_pat).check() {
                let use_span = matches[pattern.ptr_use].span_no_inline(body);
                let move_span = matches[pattern.vec_move].span_no_inline(body);
                let ty = matches[TyVarIdx::from_u16(1)];
                // let global = self.tcx.type_of(global_did).instantiate_identity();
                self.tcx.emit_node_span_lint(
                    USE_AFTER_MOVE,
                    self.tcx.local_def_id_to_hir_id(def_id),
                    use_span,
                    crate::errors::UseAfterMove {
                        use_span,
                        move_span,
                        ty,
                    },
                );
            }
        }
        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
}

struct Pattern<'pcx> {
    pattern: &'pcx pat::Pattern<'pcx>,
    fn_pat: &'pcx pat::Fn<'pcx>,
    vec_move: pat::Location,
    ptr_use: pat::Location,
}

#[rpl_macros::pattern_def]
fn pattern(pcx: PatCtxt<'_>) -> Pattern<'_> {
    let vec_move;
    let ptr_use;
    let pattern = rpl! {
        // #[meta($T:ty)]
        // struct $Ptr {
        //     $ptr: core::ptr::NonNull<$T>,
        // }

        // struct $Vec {
        //     $ptr: $Ptr,
        // }

        #[meta($T:ty, $Vec:ty)]
        fn $pattern(..) -> _ = mir! {
            // let $ptr: BitPtr<$T> = _; // _2
            let $ptr_1: *mut $T = _; // _14
            let $ptr_mut_slice: *mut [$T] = *mut [$T] from (copy $ptr_1, _); // _21
            let $ref_mut_slice: &mut [$T] = &mut (*$ptr_mut_slice); // _3
            let $ptr_slice: *mut [$T] = &raw mut (*$ref_mut_slice); // _22
            let $ptr_2: *mut $T = move $ptr_slice as *mut $T (PtrToPtr); // _6
            let $ptr_3: *const u8 = copy $ptr_2 as *const u8 (PtrToPtr); // _29
            let $non_null: core::ptr::NonNull<u8> = core::ptr::NonNull<u8> { pointer: copy $ptr_3 }; // _28
            #[export(ptr_use)]
            let $unique: core::ptr::Unique<u8> = core::ptr::Unique<u8> { pointer: move $non_null, _marker: const core::marker::PhantomData::<u8> }; // _27
            let $raw_vec_inner: alloc::raw_vec::RawVecInner = alloc::raw_vec::RawVecInner { ptr: move $unique, cap: _, alloc: _ }; // _25
            let $raw_vec: alloc::raw_vec::RawVec<$T> = alloc::raw_vec::RawVec<$T> { inner: move $raw_vec_inner, _marker: const core::marker::PhantomData::<$T> }; // _23
            let $vec: $Vec /* alloc::vec::Vec<$T> */ = alloc::vec::Vec<$T> { buf: move $raw_vec, len: _ }; // _5
            #[export(vec_move)]
            _ = alloc::vec::Vec::into_boxed_slice(move $vec); // _10
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();
    // let ptr = pattern.get_adt(Symbol::intern("Ptr")).unwrap();

    Pattern {
        pattern,
        fn_pat,
        vec_move,
        ptr_use,
    }
}
