use std::ops::Not;

use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::{Ty, TyCtxt};
use rustc_middle::{mir, ty};
use rustc_span::{sym, Span, Symbol};

use rpl_mir_pattern::{pat, CheckMirCtxt};

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
            use std::ops::ControlFlow::{Break, Continue};
            let match_span = |pat| {
                mcx.patterns.try_for_matched_patterns(pat, |mat| match mat.span(body) {
                    Some(span) => Break(span),
                    None => Continue(()),
                })
            };
            let cast_from = match_span(pattern.cast_from);
            let cast_from_mut = match_span(pattern.cast_from_mut);
            let cast_to = match_span(pattern.cast_to);
            let cast_to_mut = match_span(pattern.cast_to_mut);
            let ty = mcx.patterns.try_for_matched_types(pattern.ty_var, |ty| {
                if !ty.is_primitive() && is_all_safe_trait(self.tcx, self.tcx.predicates_of(def_id), ty) {
                    return Break(ty);
                }
                Continue(())
            });
            debug!(?cast_from, ?cast_from_mut, ?cast_to, ?cast_to_mut, ?ty);
            if let Break(ty) = ty {
                if let Break(cast_from) = cast_from
                    && let Break(cast_to) = cast_to
                {
                    self.tcx.dcx().emit_err(crate::errors::UnsoundSliceCast {
                        cast_from,
                        cast_to,
                        ty,
                        mutability: ty::Mutability::Not.into(),
                    });
                } else if let Break(cast_from) = cast_from_mut
                    && let Break(cast_to) = cast_to_mut
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

/// patterns:
/// ```ignore
/// let from_slice: &[T];
/// let from_raw_slice: *const [T] = &raw const *from_slice;
/// let from_len: usize = Len(from_slice);
/// let ty_size: usize = SizeOf($T);
/// let to_ptr: *const u8 = from_ptr as *const u8;
/// let to_len: usize = Mul(from_len, ty_size);
/// let to_raw_slice: *const [u8] = Aggregate(*const [u8], [to_ptr, t_len]);
/// let to_slice: &u8 = &*to_raw_slice;
/// ```
fn pattern<'tcx>(tcx: TyCtxt<'tcx>, patterns: &mut pat::Patterns<'tcx>) -> Pattern {
    let ty_var = patterns.new_ty_var();
    let ty = patterns.mk_ty(tcx, pat::TyKind::TyVar(ty_var));
    let slice_ty = patterns.mk_ty(tcx, pat::TyKind::Slice(ty));
    let ref_slice_ty = patterns.mk_ty(
        tcx,
        pat::TyKind::Ref(pat::RegionKind::ReErased, slice_ty, mir::Mutability::Not),
    );
    let mut_slice_ty = patterns.mk_ty(
        tcx,
        pat::TyKind::Ref(pat::RegionKind::ReErased, slice_ty, mir::Mutability::Mut),
    );
    let raw_slice_ty = patterns.mk_ty(tcx, pat::TyKind::RawPtr(slice_ty, mir::Mutability::Not));
    let raw_mut_slice_ty = patterns.mk_ty(tcx, pat::TyKind::RawPtr(slice_ty, mir::Mutability::Mut));
    let u8_ty = patterns.mk_ty(tcx, pat::TyKind::Uint(ty::UintTy::U8));
    let usize_ty = patterns.mk_ty(tcx, pat::TyKind::Uint(ty::UintTy::Usize));
    let u8_ptr_ty = patterns.mk_ty(tcx, pat::TyKind::RawPtr(u8_ty, mir::Mutability::Not));
    let u8_mut_ptr_ty = patterns.mk_ty(tcx, pat::TyKind::RawPtr(u8_ty, mir::Mutability::Mut));
    let u8_slice_ty = patterns.mk_ty(tcx, pat::TyKind::Slice(u8_ty));
    let u8_slice_ptr_ty = patterns.mk_ty(tcx, pat::TyKind::RawPtr(u8_slice_ty, mir::Mutability::Not));
    let u8_slice_mut_ptr_ty = patterns.mk_ty(tcx, pat::TyKind::RawPtr(u8_slice_ty, mir::Mutability::Mut));
    let u8_slice_ref_ty = patterns.mk_ty(
        tcx,
        pat::TyKind::Ref(pat::RegionKind::ReErased, u8_slice_ty, mir::Mutability::Not),
    );
    let u8_slice_mut_ty = patterns.mk_ty(
        tcx,
        pat::TyKind::Ref(pat::RegionKind::ReErased, u8_slice_ty, mir::Mutability::Mut),
    );

    let from_slice = patterns.mk_local(ref_slice_ty);
    let from_mut_slice = patterns.mk_local(mut_slice_ty);
    let from_raw = patterns.mk_local(raw_slice_ty).into_place();
    let from_raw_mut = patterns.mk_local(raw_mut_slice_ty).into_place();
    let to_ptr = patterns.mk_local(u8_ptr_ty).into_place();
    let to_mut_ptr = patterns.mk_local(u8_mut_ptr_ty).into_place();
    let from_len = patterns.mk_local(usize_ty).into_place();
    // FIXME: unnecessary pattern
    let from_len_mut = patterns.mk_local(usize_ty).into_place();
    let ty_size = patterns.mk_local(usize_ty).into_place();
    // FIXME: unnecessary pattern
    let ty_size_mut = patterns.mk_local(usize_ty).into_place();
    let to_len = patterns.mk_local(usize_ty).into_place();
    let to_len_mut = patterns.mk_local(usize_ty).into_place();
    let to_raw_slice = patterns.mk_local(u8_slice_ptr_ty);
    let to_mut_raw_slice = patterns.mk_local(u8_slice_mut_ptr_ty);
    let to_slice = patterns.mk_local(u8_slice_ref_ty).into_place();
    let to_mut_slice = patterns.mk_local(u8_slice_mut_ty).into_place();
    let from_slice_deref = patterns.mk_place(tcx, from_slice, &[mir::ProjectionElem::Deref]);
    let from_mut_slice_deref = patterns.mk_place(tcx, from_mut_slice, &[mir::ProjectionElem::Deref]);
    let to_raw_slice_deref = patterns.mk_place(tcx, to_raw_slice, &[mir::ProjectionElem::Deref]);
    let to_mut_raw_slice_deref = patterns.mk_place(tcx, to_mut_raw_slice, &[mir::ProjectionElem::Deref]);

    let cast_from = patterns.mk_init(from_slice);
    let cast_from_mut = patterns.mk_init(from_mut_slice);
    patterns.mk_assign(from_raw, pat::Rvalue::AddressOf(mir::Mutability::Not, from_slice_deref));
    patterns.mk_assign(
        from_raw_mut,
        pat::Rvalue::AddressOf(mir::Mutability::Mut, from_mut_slice_deref),
    );
    patterns.mk_assign(
        to_ptr,
        pat::Rvalue::Cast(mir::CastKind::PtrToPtr, pat::Copy(from_raw), u8_ptr_ty),
    );
    patterns.mk_assign(
        to_mut_ptr,
        pat::Rvalue::Cast(mir::CastKind::PtrToPtr, pat::Copy(from_raw_mut), u8_mut_ptr_ty),
    );
    patterns.mk_assign(from_len, pat::Rvalue::Len(from_slice_deref));
    // FIXME: unnecessary pattern
    patterns.mk_assign(from_len_mut, pat::Rvalue::Len(from_mut_slice_deref));
    patterns.mk_assign(ty_size, pat::Rvalue::NullaryOp(mir::NullOp::SizeOf, ty));
    // FIXME: unnecessary pattern
    patterns.mk_assign(ty_size_mut, pat::Rvalue::NullaryOp(mir::NullOp::SizeOf, ty));
    patterns.mk_assign(
        to_len,
        pat::Rvalue::BinaryOp(mir::BinOp::Mul, Box::new([pat::Move(from_len), pat::Move(ty_size)])),
    );
    // FIXME: unnecessary pattern
    patterns.mk_assign(
        to_len_mut,
        pat::Rvalue::BinaryOp(
            mir::BinOp::Mul,
            Box::new([pat::Move(from_len_mut), pat::Move(ty_size_mut)]),
        ),
    );
    patterns.mk_assign(
        to_raw_slice.into_place(),
        pat::Rvalue::Aggregate(
            pat::AggKind::RawPtr(u8_slice_ty, ty::Mutability::Not),
            [pat::Copy(to_ptr), pat::Copy(to_len)].into_iter().collect(),
        ),
    );
    patterns.mk_assign(
        to_mut_raw_slice.into_place(),
        pat::Rvalue::Aggregate(
            pat::AggKind::RawPtr(u8_slice_ty, ty::Mutability::Mut),
            [pat::Copy(to_mut_ptr), pat::Copy(to_len_mut)].into_iter().collect(),
        ),
    );
    let cast_to = patterns.mk_assign(
        to_slice,
        pat::Rvalue::Ref(pat::RegionKind::ReErased, mir::BorrowKind::Shared, to_raw_slice_deref),
    );
    let cast_to_mut = patterns.mk_assign(
        to_mut_slice,
        pat::Rvalue::Ref(
            pat::RegionKind::ReErased,
            mir::BorrowKind::Mut {
                kind: mir::MutBorrowKind::Default,
            },
            to_mut_raw_slice_deref,
        ),
    );
    Pattern {
        ty_var,
        cast_from,
        cast_from_mut,
        cast_to,
        cast_to_mut,
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
