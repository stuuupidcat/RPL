use rustc_hir as hir;
use rustc_middle::ty::{self, AdtKind, Ty, TyCtxt, TypingMode};
use rustc_span::{DUMMY_SP, Symbol, sym};

pub type SingleTyPredsFnPtr = for<'tcx> fn(TyCtxt<'tcx>, ty::TypingEnv<'tcx>, Ty<'tcx>) -> bool;

/// Check if self_ty's trait bounds are all safe.
#[instrument(level = "debug", skip(tcx), ret)]
pub fn is_all_safe_trait<'tcx>(tcx: TyCtxt<'tcx>, typing_env: ty::TypingEnv<'tcx>, ty: Ty<'tcx>) -> bool {
    const EXCLUDED_DIAG_ITEMS: &[Symbol] = &[sym::Send, sym::Sync];
    typing_env
        .param_env
        .caller_bounds()
        .iter()
        .filter_map(|clause| clause.as_trait_clause())
        // FIXME: help find out why `no_bound_vars` cannot be unwrapped
        .filter(|clause| clause.self_ty().no_bound_vars() == Some(ty))
        .map(|clause| clause.def_id())
        .filter(|&def_id| {
            tcx.get_diagnostic_name(def_id)
                .is_none_or(|name| !EXCLUDED_DIAG_ITEMS.contains(&name))
        })
        .map(|def_id| tcx.trait_def(def_id))
        .inspect(|trait_def| debug!(?trait_def))
        .all(|trait_def| matches!(trait_def.safety, hir::Safety::Safe))
}

/// Check if ty is [`Copy`].
#[instrument(level = "debug", skip(tcx), ret)]
pub fn is_copy<'tcx>(tcx: TyCtxt<'tcx>, typing_env: ty::TypingEnv<'tcx>, ty: Ty<'tcx>) -> bool {
    tcx.type_is_copy_modulo_regions(typing_env, ty)
}

/// Check if ty is not unpin.
#[instrument(level = "debug", skip(tcx), ret)]
#[allow(unused_variables)]
pub fn is_not_unpin<'tcx>(tcx: TyCtxt<'tcx>, typing_env: ty::TypingEnv<'tcx>, ty: Ty<'tcx>) -> bool {
    !ty.is_unpin(tcx, typing_env)
}

/// Check if ty is send.
#[instrument(level = "debug", skip(tcx), ret)]
pub fn is_send<'tcx>(tcx: TyCtxt<'tcx>, typing_env: ty::TypingEnv<'tcx>, ty: Ty<'tcx>) -> bool {
    use rustc_infer::infer::TyCtxtInferExt;
    let infcx = tcx.infer_ctxt().build(TypingMode::PostAnalysis);
    let trait_def_id = tcx.get_diagnostic_item(sym::Send).unwrap();

    rustc_trait_selection::traits::type_known_to_meet_bound_modulo_regions(
        &infcx,
        typing_env.param_env,
        ty,
        trait_def_id,
    )
}

/// Check if ty is sync.
#[instrument(level = "debug", skip(tcx), ret)]
pub fn is_sync<'tcx>(tcx: TyCtxt<'tcx>, typing_env: ty::TypingEnv<'tcx>, ty: Ty<'tcx>) -> bool {
    use rustc_infer::infer::TyCtxtInferExt;
    let infcx = tcx.infer_ctxt().build(TypingMode::PostAnalysis);
    let trait_def_id = tcx.require_lang_item(hir::LangItem::Sync, DUMMY_SP);
    rustc_trait_selection::traits::type_known_to_meet_bound_modulo_regions(
        &infcx,
        typing_env.param_env,
        ty,
        trait_def_id,
    )
}

/// Check if ty is integral.
#[instrument(level = "debug", skip(tcx), ret)]
#[allow(unused_variables)]
pub fn is_integral<'tcx>(tcx: TyCtxt<'tcx>, typing_env: ty::TypingEnv<'tcx>, ty: Ty<'tcx>) -> bool {
    ty.is_integral()
}

/// Check if ty is a pointer.
#[instrument(level = "debug", skip(tcx), ret)]
#[allow(unused_variables)]
pub fn is_ptr<'tcx>(tcx: TyCtxt<'tcx>, typing_env: ty::TypingEnv<'tcx>, ty: Ty<'tcx>) -> bool {
    ty.is_any_ptr()
}

/// Check if ty needs to be dropped.
#[instrument(level = "debug", skip(tcx), ret)]
pub fn needs_drop<'tcx>(tcx: TyCtxt<'tcx>, typing_env: ty::TypingEnv<'tcx>, ty: Ty<'tcx>) -> bool {
    ty.needs_drop(tcx, typing_env)
}

/// Check if ty is a primitive type.
pub fn is_primitive<'tcx>(_tcx: TyCtxt<'tcx>, _typing_env: ty::TypingEnv<'tcx>, ty: Ty<'tcx>) -> bool {
    ty.is_primitive()
}

/// Check if ty is a floating-point type.
pub fn is_float<'tcx>(_tcx: TyCtxt<'tcx>, _typing_env: ty::TypingEnv<'tcx>, ty: Ty<'tcx>) -> bool {
    ty.is_floating_point()
}

/// Check if ty is `char`.
pub fn is_char<'tcx>(_tcx: TyCtxt<'tcx>, _typing_env: ty::TypingEnv<'tcx>, ty: Ty<'tcx>) -> bool {
    ty.is_char()
}

/// Check if ty is a reference type.
pub fn is_ref<'tcx>(_tcx: TyCtxt<'tcx>, _typing_env: ty::TypingEnv<'tcx>, ty: Ty<'tcx>) -> bool {
    ty.is_ref()
}

/// Check if ty is a function pointer type.
pub fn is_fn_ptr<'tcx>(_tcx: TyCtxt<'tcx>, _typing_env: ty::TypingEnv<'tcx>, ty: Ty<'tcx>) -> bool {
    ty.is_fn_ptr()
}

/// Check if ty is a ZST.
pub fn is_zst<'tcx>(tcx: TyCtxt<'tcx>, typing_env: ty::TypingEnv<'tcx>, ty: Ty<'tcx>) -> bool {
    if let Ok(layout) = tcx.layout_of(typing_env.as_query_input(ty)) {
        layout.layout.is_zst()
    } else {
        false
    }
}

/// Check if ty can be uninitialized, AKA safe to be used in `std::mem::uninitialized` or similar
/// APIs.
// FIXME: Chances are that this function can overflow the stack.
// `typing_env` is part of the `SingleTy` predicate signature and is threaded through the
// recursion; this arm doesn't read it directly.
#[allow(clippy::only_used_in_recursion)]
#[instrument(level = "debug", skip(tcx, typing_env), ret)]
pub fn can_be_uninit<'tcx>(tcx: TyCtxt<'tcx>, typing_env: ty::TypingEnv<'tcx>, ty: Ty<'tcx>) -> bool {
    match ty.kind() {
        ty::Bool | ty::Char | ty::Int(_) | ty::Uint(_) | ty::Float(_) => false,
        ty::Adt(adt_def, args) => {
            adt_def.is_phantom_data() || adt_def.is_payloadfree() || {
                match adt_def.adt_kind() {
                    AdtKind::Union => adt_def
                        .all_fields()
                        .any(|field| can_be_uninit(tcx, typing_env, field.ty(tcx, args).skip_normalization())),
                    AdtKind::Struct => adt_def
                        .all_fields()
                        .all(|field| can_be_uninit(tcx, typing_env, field.ty(tcx, args).skip_normalization())),
                    _ => false,
                }
            }
        },
        ty::Foreign(_) => false, // Who knows what foreign types do?
        ty::Str => false,
        ty::Array(ty, len) => {
            can_be_uninit(tcx, typing_env, *ty) || len.try_to_target_usize(tcx).is_some_and(|len| len == 0)
        },
        ty::Pat(ty, _) => can_be_uninit(tcx, typing_env, *ty), // FIXME: handle pattern parts
        ty::Slice(ty) => can_be_uninit(tcx, typing_env, *ty),
        ty::RawPtr(_, _) => false,
        ty::Ref(_, _, _) => false,
        ty::FnDef(_, _) => false,
        ty::FnPtr(_, _) => false,
        ty::UnsafeBinder(_) => false,
        ty::Dynamic(_, _) => false,
        ty::Closure(_, _) => false,
        ty::CoroutineClosure(_, _) => false,
        ty::Coroutine(_, _) => false,
        ty::CoroutineWitness(_, _) => false,
        ty::Never => true, // Never type is singular, so it can be uninitialized.
        ty::Tuple(tys) => tys.iter().all(|ty| can_be_uninit(tcx, typing_env, ty)),
        ty::Alias(alias_ty) => can_be_uninit(tcx, typing_env, alias_ty.self_ty()),
        // If it's a type parameter, we assume it can be uninitialized if it has any unsafe traits.
        ty::Param(_) => false, // !is_all_safe_trait(tcx, typing_env, ty),
        ty::Bound(_, _) => false,
        ty::Placeholder(_) => false,
        ty::Infer(_) => false,
        ty::Error(_) => false,
    }
}

// /// Check if ty can be uninitialized, AKA safe to be used in `std::mem::uninitialized` or similar
// /// APIs.
// #[instrument(level = "debug", skip(tcx, typing_env), ret)]
// pub fn can_be_uninit<'tcx>(tcx: TyCtxt<'tcx>, typing_env: ty::TypingEnv<'tcx>, ty: Ty<'tcx>) ->
// bool {     fn can_be_uninit_impl<'tcx>(
//         tcx: TyCtxt<'tcx>,
//         typing_env: ty::TypingEnv<'tcx>,
//         ty: Ty<'tcx>,
//         visited: &mut FxHashSet<DefId>,
//     ) -> bool {
// use rustc_data_structures::fx::{FxHashMap, FxHashSet};
// use rustc_hir::def_id::DefId;
//         let mut queue = vec![ty];
//         while let Some(ty) = queue.pop() {
//             match ty.kind() {
//                 ty::Bool | ty::Char | ty::Int(_) | ty::Uint(_) | ty::Float(_) => return false,
//                 ty::Adt(adt_def, args) => {
//                     let adt_did = adt_def.did();
//                     if !visited.contains(&adt_did) && !adt_def.is_phantom_data() &&
// !adt_def.is_payloadfree() {                         visited.insert(adt_did);
//                         match adt_def.adt_kind() {
//                             AdtKind::Union => {
//                                 if !adt_def.all_fields().any(|field| {
//                                     can_be_uninit_impl(tcx, typing_env, field.ty(tcx, args), &mut
// visited.clone())                                 }) {
//                                     return false;
//                                 }
//                             },
//                             AdtKind::Struct => adt_def.all_fields().for_each(|field|
// queue.push(field.ty(tcx, args))),                             _ => return false,
//                         }
//                     }
//                 },
//                 ty::Foreign(_) => return false, // Who knows what foreign types do?
//                 ty::Str => return false,
//                 ty::Array(ty, len) => {
//                     if !len.try_to_target_usize(tcx).is_some_and(|len| len == 0) {
//                         queue.push(*ty);
//                     }
//                 },
//                 ty::Pat(ty, _) => queue.push(*ty), // FIXME: handle pattern parts
//                 ty::Slice(ty) => queue.push(*ty),
//                 ty::RawPtr(_, _)
//                 | ty::Ref(_, _, _)
//                 | ty::FnDef(_, _)
//                 | ty::FnPtr(_, _)
//                 | ty::UnsafeBinder(_)
//                 | ty::Dynamic(_, _, _)
//                 | ty::Closure(_, _)
//                 | ty::CoroutineClosure(_, _)
//                 | ty::Coroutine(_, _)
//                 | ty::CoroutineWitness(_, _) => return false,
//                 ty::Never => (), // Never type is singular, so it can be uninitialized.
//                 ty::Tuple(tys) => queue.extend(tys.iter()),
//                 ty::Alias(_, alias_ty) => queue.push(alias_ty.to_ty(tcx)),
//                 // If it's a type parameter, we assume it can be uninitialized if it has any
// unsafe traits.                 ty::Param(_) => return false, // !is_all_safe_trait(tcx,
// typing_env, ty),                 ty::Bound(_, _) | ty::Placeholder(_) | ty::Infer(_) |
// ty::Error(_) => return false,             }
//         }
//         true
//     }
//     can_be_uninit_impl(tcx, typing_env, ty, &mut Default::default())
// }
