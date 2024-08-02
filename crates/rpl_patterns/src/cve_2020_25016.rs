use rustc_hir as hir;
use rustc_middle::ty;
use rustc_middle::ty::TyCtxt;
use rustc_span::symbol::kw;
use rustc_span::{sym, Symbol};

#[instrument(level = "info", skip(tcx))]
pub fn check_item(tcx: TyCtxt<'_>, item_id: hir::ItemId) {
    let item = tcx.hir().item(item_id);
    let def_id = item_id.owner_id.def_id;
    if let hir::ItemKind::Trait(hir::IsAuto::No, hir::Safety::Safe, .., trait_items) = item.kind {
        let mut as_bytes = Vec::new();
        for trait_item_ref in trait_items {
            let trait_item_owner_id = trait_item_ref.id.owner_id;
            let trait_item_def_id = trait_item_owner_id.def_id;
            let trait_item = tcx.hir_owner_node(trait_item_owner_id).expect_trait_item();
            debug!("trait item: {}", tcx.hir().node_to_string(trait_item.hir_id()));
            if let hir::AssocItemKind::Fn { has_self: true } = trait_item_ref.kind
                && let hir::TraitItemKind::Fn(hir::FnSig { span, .. }, hir::TraitFn::Provided(_)) = trait_item.kind
                && let fn_sig = tcx.fn_sig(trait_item_def_id).instantiate_identity().skip_binder()
                && let Some(mutbly) = is_safe_as_bytes_or_as_mut_bytes(&fn_sig)
                && is_all_safe_trait(tcx, tcx.predicates_of(def_id))
            {
                as_bytes.push(crate::errors::UnsoundAsBytesMethod {
                    span,
                    name: trait_item.ident.name,
                    ref_mutbly: mutbly.ref_prefix_str(),
                });
            }
        }
        if !as_bytes.is_empty() {
            tcx.dcx().emit_err(crate::errors::UnsoundAsBytesTrait {
                span: item.span,
                as_bytes,
                unsafe_sugg: item.vis_span.shrink_to_hi(),
            });
        }
    }
}

#[instrument(level = "debug", ret)]
fn is_safe_as_bytes_or_as_mut_bytes(fn_sig: &ty::FnSig<'_>) -> Option<hir::Mutability> {
    if let hir::Safety::Safe = fn_sig.safety
        && let [self_ty, ret_ty] = fn_sig.inputs_and_output.as_slice()
        && let &ty::Ref(self_region, self_ty, self_mut) = self_ty.kind()
        && let &ty::Ref(ret_region, ret_ty, ret_mut) = ret_ty.kind()
        && let ty::Param(ty::ParamTy {
            index: 0,
            name: kw::SelfUpper,
        }) = self_ty.kind()
        && let ty::Slice(slice_ty) = ret_ty.kind()
        && let ty::Uint(ty::UintTy::U8) = slice_ty.kind()
        && self_region == ret_region
        && self_mut == ret_mut
    {
        return Some(self_mut);
    }
    None
}

#[instrument(level = "debug", skip(tcx), ret)]
fn is_all_safe_trait<'tcx>(tcx: TyCtxt<'tcx>, predicates: ty::GenericPredicates<'tcx>) -> bool {
    const EXCLUDED_DIAG_ITEMS: &[Symbol] = &[sym::Send, sym::Sync];
    predicates
        .predicates
        .iter()
        .inspect(|(clause, span)| debug!("clause at {span:?}: {clause:?}"))
        .filter_map(|(clause, _)| Some(clause.as_trait_clause()?.def_id()))
        .filter(|&def_id| {
            tcx.get_diagnostic_name(def_id)
                .is_none_or(|name| !EXCLUDED_DIAG_ITEMS.contains(&name))
        })
        .map(|def_id| tcx.trait_def(def_id))
        .inspect(|trait_def| debug!(?trait_def))
        .all(|trait_def| matches!(trait_def.safety, hir::Safety::Safe))
}
