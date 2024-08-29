use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::{Ty, TyCtxt};
use rustc_middle::{mir, ty};
use rustc_span::symbol::kw;
use rustc_span::{sym, Span, Symbol};

use rpl_mir_pattern::{pat, CheckMirCtxt};

#[instrument(level = "info", skip(tcx))]
pub fn check_item(tcx: TyCtxt<'_>, item_id: hir::ItemId) {
    let item = tcx.hir().item(item_id);
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
        if self.tcx.is_mir_available(def_id)
            && let Some(cstring_did) = self.tcx.get_diagnostic_item(sym::cstring_type)
        {
            let body = self.tcx.optimized_mir(def_id);
            let mut mcx = CheckMirCtxt::new(self.tcx, body);
            let pattern = pattern(self.tcx, &mut mcx.patterns);
            mcx.check();
            let match_span = |pat| mcx.patterns.first_matched_span(body, pat);
            let ptr_usage = match_span(pattern.ptr_usage);
            let cstring_drop = match_span(pattern.cstring_drop);
            debug!(?ptr_usage, ?cstring_drop);
            if let Some(use_span) = ptr_usage
                && let Some(drop_span) = cstring_drop
            {
                self.tcx.dcx().emit_err(crate::errors::UseAfterDrop {
                    use_span,
                    drop_span,
                    ty: self.tcx.type_of(cstring_did).instantiate_identity(),
                });
            }
        }
        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
}

struct Pattern {
    cstring_drop: pat::PatternIdx,
    ptr_usage: pat::PatternIdx,
}

/// patterns:
/// ```ignore
/// let cstring   : CString       = ...                                      ;
/// let non_null  : NonNull<[u8]> = (((cstring.inner).0).pointer)            ;
/// let uslice_ptr: *const [u8]   = (non_null.pointer)                       ;
/// let cstr      : *const CStr   = uslice_ptr as *const CStr (PtrToPtr)     ;
/// /*
/// let uslice    : &[u8]         = &(*uslice_ptr)                           ;
/// let cstr      : &CStr         = from_bytes___rt_impl(move uslice)        ;
/// */
/// let islice    : *const [i8]   = &raw const ((*cstr).inner)               ;
/// let iptr      : *const i8     = move islice as *const i8 (PtrToPtr)      ;
/// drop(cstring)                                                            ;
/// let s         : i32           = ...                                      ;
/// let ret       : i32           = sqlite3session_attach(move s, move iptr) ;
/// ```
fn pattern<'tcx>(tcx: TyCtxt<'tcx>, patterns: &mut pat::Patterns<'tcx>) -> Pattern {
    let cstr_ty = patterns.mk_adt_ty(tcx, (tcx, &[sym::core, sym::ffi, sym::c_str, sym::CStr]), &[]);
    let cstring_ty = patterns.mk_adt_ty(
        tcx,
        (tcx, &[sym::alloc, sym::ffi, sym::c_str, Symbol::intern("CString")]),
        &[],
    );

    let u8_ty = patterns.primitive_types.u8;
    let u8_slice_ty = patterns.mk_slice_ty(tcx, u8_ty);
    // let u8_slice_ref_ty = patterns.mk_ref_ty(
    //     tcx, pat::RegionKind::ReErased, u8_slice_ty, ty::Mutability::Not,
    // );
    let u8_slice_ptr_ty = patterns.mk_raw_ptr_ty(tcx, u8_slice_ty, mir::Mutability::Not);
    let non_null_u8_slice_ty = patterns.mk_adt_ty(
        tcx,
        (tcx, &[sym::core, sym::ptr, Symbol::intern("non_null"), sym::NonNull]),
        (tcx, &[u8_slice_ty.into()]),
    );

    let i32_ty = patterns.primitive_types.i32;
    let i8_ty = patterns.primitive_types.i8;
    let i8_ptr_ty = patterns.mk_raw_ptr_ty(tcx, i8_ty, mir::Mutability::Not);
    let i8_slice_ty = patterns.mk_slice_ty(tcx, i8_ty);
    let i8_slice_ptr_ty = patterns.mk_raw_ptr_ty(tcx, i8_slice_ty, mir::Mutability::Not);

    // let cstr_ref_ty = patterns.mk_ref_ty(
    //     tcx, pat::RegionKind::ReErased, cstr_ty, mir::Mutability::Not,
    // );
    let cstr_ptr_ty = patterns.mk_raw_ptr_ty(tcx, cstr_ty, mir::Mutability::Not);

    let cstring_local = patterns.mk_local(cstring_ty);
    let non_null_local = patterns.mk_local(non_null_u8_slice_ty);
    let uslice_ptr_local = patterns.mk_local(u8_slice_ptr_ty);
    // let uslice_local = patterns.mk_local(u8_slice_ref_ty).into_place();
    // let cstr_local = patterns.mk_local(cstr_ref_ty);
    let cstr_local = patterns.mk_local(cstr_ptr_ty);
    let islice_local = patterns.mk_local(i8_slice_ptr_ty).into_place();
    let iptr_local = patterns.mk_local(i8_ptr_ty).into_place();
    let i32_local = patterns.mk_local(i32_ty);
    let ret_local = patterns.mk_local(i32_ty).into_place();

    patterns.mk_init(cstring_local);

    let non_null_field_place = patterns.mk_place(
        cstring_local,
        (
            tcx,
            &[
                pat::PlaceElem::Field("inner".into()),
                pat::PlaceElem::Field(0.into()),
                pat::PlaceElem::Field(sym::pointer.into()),
            ],
        ),
    );
    patterns.mk_assign(
        non_null_local.into_place(),
        pat::Rvalue::Use(pat::Copy(non_null_field_place)),
    );
    let uslice_field_place = patterns.mk_place(non_null_local, (tcx, &[pat::PlaceElem::Field(sym::pointer.into())]));
    patterns.mk_assign(
        uslice_ptr_local.into_place(),
        pat::Rvalue::Use(pat::Copy(uslice_field_place)),
    );
    // let uslice_ptr_deref_place = patterns.mk_place(uslice_ptr_local, (tcx,
    // &[pat::PlaceElem::Deref])); patterns.mk_assign(
    //     uslice_local,
    //     pat::Rvalue::Ref(
    //         pat::RegionKind::ReErased,
    //         mir::BorrowKind::Shared,
    //         uslice_ptr_deref_place,
    //     ),
    // );

    // let fn_ty = patterns.mk_fn(tcx, (cstr_ty, "from_bytes_with_nul_unchecked"), &[]);
    // patterns.mk_fn_call(
    //     tcx,
    //     (fn_ty, "rt_impl"),
    //     &[],
    //     pat::List::ordered([pat::Move(uslice_local)]),
    //     cstr_local.into_place(),
    // );
    patterns.mk_assign(
        cstr_local.into_place(),
        pat::Rvalue::Cast(
            mir::CastKind::PtrToPtr,
            pat::Copy(uslice_ptr_local.into_place()),
            cstr_ptr_ty,
        ),
    );
    let cstr_deref_place = patterns.mk_place(
        cstr_local,
        (tcx, &[pat::PlaceElem::Deref, pat::PlaceElem::Field("inner".into())]),
    );
    patterns.mk_assign(
        islice_local,
        pat::Rvalue::AddressOf(mir::Mutability::Not, cstr_deref_place),
    );
    patterns.mk_assign(
        iptr_local,
        pat::Rvalue::Cast(mir::CastKind::PtrToPtr, pat::Move(islice_local), i8_ptr_ty),
    );
    let cstring_drop = patterns.mk_drop(cstring_local.into_place());
    patterns.mk_init(i32_local);
    let ptr_usage = patterns.mk_fn_call(
        tcx,
        (tcx, &[kw::Crate, sym::ffi, Symbol::intern("sqlite3session_attach")]),
        &[],
        pat::List::ordered([pat::Move(i32_local.into_place()), pat::Move(iptr_local)]),
        ret_local,
    );
    patterns.add_dependency(ptr_usage, cstring_drop);

    Pattern {
        cstring_drop,
        ptr_usage,
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
