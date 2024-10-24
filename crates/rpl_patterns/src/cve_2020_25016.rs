use std::ops::Not;

use rpl_mir::pat::PatternsBuilder;
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::{self, Ty, TyCtxt};
use rustc_span::{sym, Span, Symbol};

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

    fn visit_item(&mut self, item: &'tcx hir::Item<'tcx>) -> Self::Result {
        match item.kind {
            hir::ItemKind::Trait(hir::IsAuto::No, hir::Safety::Safe, ..)
            | hir::ItemKind::Impl(_)
            | hir::ItemKind::Fn(..) => {},
            _ => return,
        }
        intravisit::walk_item(self, item);
    }

    fn visit_fn(
        &mut self,
        kind: intravisit::FnKind<'tcx>,
        decl: &'tcx hir::FnDecl<'tcx>,
        body_id: hir::BodyId,
        _span: Span,
        def_id: LocalDefId,
    ) -> Self::Result {
        if self.tcx.visibility(def_id).is_public()
            && kind.header().is_none_or(|header| header.is_unsafe().not())
            && self.tcx.is_mir_available(def_id)
        {
            let body = self.tcx.optimized_mir(def_id);
            let check_ty =
                |ty: Ty<'tcx>| !ty.is_primitive() && is_all_safe_trait(self.tcx, self.tcx.predicates_of(def_id), ty);
            #[allow(irrefutable_let_patterns)]
            if let mut patterns_cast = PatternsBuilder::new(&self.tcx.arena.dropless)
                && let pattern_cast = pattern_cast(&mut patterns_cast)
                && let Some(matches) = CheckMirCtxt::new(self.tcx, body, &patterns_cast.build()).check()
                && let Some(cast_from) = matches[pattern_cast.cast_from]
                && let cast_from = cast_from.span_no_inline(body)
                && let Some(cast_to) = matches[pattern_cast.cast_to]
                && let cast_to = cast_to.span_no_inline(body)
                && let ty = matches[pattern_cast.ty_var]
                && check_ty(ty)
            {
                debug!(?cast_from, ?cast_to, ?ty);
                self.tcx.dcx().emit_err(crate::errors::UnsoundSliceCast {
                    cast_from,
                    cast_to,
                    ty,
                    mutability: ty::Mutability::Not.into(),
                });
            } else if let mut patterns_cast_mut = PatternsBuilder::new(&self.tcx.arena.dropless)
                && let pattern_cast_mut = pattern_cast_mut(&mut patterns_cast_mut)
                && let Some(matches) = CheckMirCtxt::new(self.tcx, body, &patterns_cast_mut.build()).check()
                && let Some(cast_from) = matches[pattern_cast_mut.cast_from_mut]
                && let cast_from = cast_from.span_no_inline(body)
                && let Some(cast_to) = matches[pattern_cast_mut.cast_to_mut]
                && let cast_to = cast_to.span_no_inline(body)
                && let ty = matches[pattern_cast_mut.ty_var]
                && check_ty(ty)
            {
                debug!(?cast_from, ?cast_to, ?ty);
                self.tcx.dcx().emit_err(crate::errors::UnsoundSliceCast {
                    cast_from,
                    cast_to,
                    ty,
                    mutability: ty::Mutability::Mut.into(),
                });
            }
        }
        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
}

struct PatternCast {
    ty_var: pat::TyVarIdx,
    cast_from: pat::Location,
    cast_to: pat::Location,
}

struct PatternCastMut {
    ty_var: pat::TyVarIdx,
    cast_from_mut: pat::Location,
    cast_to_mut: pat::Location,
}

#[rpl_macros::mir_pattern]
fn pattern_cast(patterns: &mut pat::PatternsBuilder<'_>) -> PatternCast {
    mir! {
        meta!($T:ty);

        let from_slice: &[$T] = _;
        let from_raw: *const [$T] = &raw const *from_slice;
        let from_len: usize = PtrMetadata(copy from_slice);
        let ty_size: usize = SizeOf($T);
        let to_ptr: *const u8 = copy from_raw as *const u8 (PtrToPtr);
        let to_len: usize = Mul(move from_len, move ty_size);
        let to_raw: *const [u8] = *const [u8] from (copy to_ptr, copy to_len);
        let to_slice: &[u8] = &*to_raw;
    }

    PatternCast {
        ty_var: T_ty_var,
        cast_from: from_slice_stmt,
        cast_to: to_slice_stmt,
    }
}

#[rpl_macros::mir_pattern]
fn pattern_cast_mut(patterns: &mut pat::PatternsBuilder<'_>) -> PatternCastMut {
    mir! {
        meta!($T:ty);

        let from_slice_mut: &mut [$T] = _;
        let from_raw_mut: *mut [$T] = &raw mut *from_slice_mut;
        let from_len_mut: usize = PtrMetadata(copy from_slice_mut);
        let ty_size: usize = SizeOf($T);
        let ty_size_mut: usize = SizeOf($T);
        let to_ptr_mut: *mut u8 = copy from_raw_mut as *mut u8 (PtrToPtr);
        let to_len_mut: usize = Mul(move from_len_mut, move ty_size_mut);
        let to_raw_mut: *mut [u8] = *mut [u8] from (copy to_ptr_mut, copy to_len_mut);
        let to_slice_mut: &mut [u8] = &mut *to_raw_mut;
    }

    PatternCastMut {
        ty_var: T_ty_var,
        cast_from_mut: from_slice_mut_stmt,
        cast_to_mut: to_slice_mut_stmt,
    }
}

#[instrument(level = "debug", skip(tcx), ret)]
fn is_all_safe_trait<'tcx>(tcx: TyCtxt<'tcx>, predicates: ty::GenericPredicates<'tcx>, self_ty: Ty<'tcx>) -> bool {
    const EXCLUDED_DIAG_ITEMS: &[Symbol] = &[sym::Send, sym::Sync];
    predicates
        .predicates
        .iter()
        .inspect(|(clause, span)| debug!("clause at {span:?}: {clause:?}"))
        .filter_map(|(clause, _span)| clause.as_trait_clause())
        .filter(|clause| clause.self_ty().no_bound_vars().expect("Unhandled bound vars") == self_ty)
        .map(|clause| clause.def_id())
        .filter(|&def_id| {
            tcx.get_diagnostic_name(def_id)
                .is_none_or(|name| !EXCLUDED_DIAG_ITEMS.contains(&name))
        })
        .map(|def_id| tcx.trait_def(def_id))
        .inspect(|trait_def| debug!(?trait_def))
        .all(|trait_def| matches!(trait_def.safety, hir::Safety::Safe))
}
