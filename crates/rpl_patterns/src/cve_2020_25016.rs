use std::ops::Not;

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
            let mut mcx = CheckMirCtxt::new(self.tcx, body);
            let pattern = pattern(self.tcx, &mut mcx.patterns);
            mcx.check();
            let match_span = |pat| mcx.patterns.first_matched_span(body, pat);
            let cast_from = match_span(pattern.cast_from);
            let cast_from_mut = match_span(pattern.cast_from_mut);
            let cast_to = match_span(pattern.cast_to);
            let cast_to_mut = match_span(pattern.cast_to_mut);
            let ty = mcx
                .patterns
                .try_for_matched_types(pattern.ty_var, |ty| {
                    if !ty.is_primitive() && is_all_safe_trait(self.tcx, self.tcx.predicates_of(def_id), ty) {
                        return Err(ty);
                    }
                    Ok(())
                })
                .err();
            debug!(?cast_from, ?cast_from_mut, ?cast_to, ?cast_to_mut, ?ty);
            if let Some(ty) = ty {
                if let Some(cast_from) = cast_from
                    && let Some(cast_to) = cast_to
                {
                    self.tcx.dcx().emit_err(crate::errors::UnsoundSliceCast {
                        cast_from,
                        cast_to,
                        ty,
                        mutability: ty::Mutability::Not.into(),
                    });
                } else if let Some(cast_from) = cast_from_mut
                    && let Some(cast_to) = cast_to_mut
                {
                    self.tcx.dcx().emit_err(crate::errors::UnsoundSliceCast {
                        cast_from,
                        cast_to,
                        ty,
                        mutability: ty::Mutability::Mut.into(),
                    });
                }
            }
        }
        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
}

struct Pattern {
    ty_var: pat::TyVarIdx,
    cast_from: pat::PatternIdx,
    cast_from_mut: pat::PatternIdx,
    cast_to: pat::PatternIdx,
    cast_to_mut: pat::PatternIdx,
}

#[rpl_macros::mir_pattern]
fn pattern<'tcx>(tcx: TyCtxt<'tcx>, patterns: &mut pat::Patterns<'tcx>) -> Pattern {
    mir! {
        meta!($T:ty);

        let from_slice: &[$T] = ...;
        let from_slice_mut: &mut [$T] = ...;

        let from_raw: *const [$T] = &raw const *from_slice;
        let from_raw_mut: *mut [$T] = &raw mut *from_slice_mut;

        let from_len: usize = Len(*from_slice);
        let from_len_mut: usize = Len(*from_slice_mut);

        let ty_size: usize = SizeOf($T);
        let ty_size_mut: usize = SizeOf($T);

        let to_ptr: *const u8 = from_raw as *const u8 (PtrToPtr);
        let to_ptr_mut: *mut u8 = from_raw_mut as *mut u8 (PtrToPtr);

        let to_len: usize = Mul(move from_len, move ty_size);
        let to_len_mut: usize = Mul(move from_len_mut, move ty_size_mut);

        let to_raw: *const [u8] = *const [u8] from (to_ptr, to_len);
        let to_raw_mut: *mut [u8] = *mut [u8] from (to_ptr_mut, to_len_mut);

        let to_slice: &[u8] = &*to_raw;
        let to_slice_mut: &mut [u8] = &mut *to_raw_mut;
    }

    Pattern {
        ty_var: T_ty_var,
        cast_from: from_slice_stmt,
        cast_from_mut: from_slice_mut_stmt,
        cast_to: to_slice_stmt,
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
