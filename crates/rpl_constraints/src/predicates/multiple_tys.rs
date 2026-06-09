use rustc_abi::Niche;
use rustc_middle::ty::{self, Ty, TyCtxt};

pub type MultipleTysPredsFnPtr = for<'tcx> fn(TyCtxt<'tcx>, ty::TypingEnv<'tcx>, Vec<Ty<'tcx>>) -> bool;

/// Check if all tys' sizes are the same
pub fn same_size<'tcx>(tcx: TyCtxt<'tcx>, typing_env: ty::TypingEnv<'tcx>, tys: Vec<Ty<'tcx>>) -> bool {
    let mut layout_res = tys.iter().map(|ty| tcx.layout_of(typing_env.as_query_input(*ty)));
    if layout_res.any(|layout| layout.is_err()) {
        return false;
    }
    // if all layouts are ok, check if all sizes are the same
    let layouts = layout_res
        .map(|layout| layout.unwrap().layout.size())
        .collect::<Vec<_>>();
    layouts.windows(2).all(|w| w[0] == w[1])
}

/// Check if all tys' alignments are the same
pub fn same_abi_and_pref_align<'tcx>(tcx: TyCtxt<'tcx>, typing_env: ty::TypingEnv<'tcx>, tys: Vec<Ty<'tcx>>) -> bool {
    let mut layout_res = tys.iter().map(|ty| tcx.layout_of(typing_env.as_query_input(*ty)));
    if layout_res.any(|layout| layout.is_err()) {
        return false;
    }
    // if all layouts are ok, check if all alignments are the same
    let layouts = layout_res
        .map(|layout| layout.unwrap().layout.align())
        .collect::<Vec<_>>();
    layouts.windows(2).all(|w| w[0] == w[1])
}

/// Check if the first type's layout is compatible with the rest of the types' layouts.
#[instrument(level = "debug", skip(tcx, typing_env), ret)]
pub fn compatible_layout<'tcx>(tcx: TyCtxt<'tcx>, typing_env: ty::TypingEnv<'tcx>, tys: Vec<Ty<'tcx>>) -> bool {
    fn compatible_layout<'tcx>(
        tcx: TyCtxt<'tcx>,
        typing_env: ty::TypingEnv<'tcx>,
        from: Ty<'tcx>,
        to: Ty<'tcx>,
    ) -> bool {
        if let Ok(from) = tcx.try_normalize_erasing_regions(typing_env, ty::Unnormalized::new_wip(from))
            && let Ok(to) = tcx.try_normalize_erasing_regions(typing_env, ty::Unnormalized::new_wip(to))
            && let Ok(from_layout) = tcx.layout_of(typing_env.as_query_input(from))
            && let Ok(to_layout) = tcx.layout_of(typing_env.as_query_input(to))
        {
            from_layout.size == to_layout.size && from_layout.align.abi == to_layout.align.abi
        } else {
            // no idea about layout, so don't lint
            true
        }
    }

    if let Some((first_ty, remained_tys)) = tys.split_first() {
        // Check if all types have the same layout as the first type
        return remained_tys
            .iter()
            .all(|ty| compatible_layout(tcx, typing_env, *first_ty, *ty));
    }
    true
}

/// Check if niche of those types are ordered increasingly
#[instrument(level = "debug", skip(tcx, typing_env), ret)]
pub fn niche_ordered<'tcx>(tcx: TyCtxt<'tcx>, typing_env: ty::TypingEnv<'tcx>, tys: Vec<Ty<'tcx>>) -> bool {
    #[instrument(level = "trace", ret)]
    fn range_fully_contained(from: Option<Niche>, to: Option<Niche>) -> bool {
        match (from, to) {
            (Some(from), Some(to)) => {
                let from = from.valid_range;
                let to = to.valid_range;
                to.contains(from.start) && to.contains(from.end)
            },
            (Some(_), None) => true,
            _ => false,
        }
    }

    if let Some((first_ty, remained_tys)) = tys.split_first() {
        let Ok(prev) = tcx.layout_of(typing_env.as_query_input(*first_ty)) else {
            return false;
        };
        let mut prev_niche = prev.largest_niche;

        for ty in remained_tys {
            let Ok(layout) = tcx.layout_of(typing_env.as_query_input(*ty)) else {
                return false;
            };
            let niche = layout.largest_niche;
            if !range_fully_contained(prev_niche, niche) {
                return false;
            }
            prev_niche = niche;
        }
    }
    true
}

// /// Check if all types but the first are borrowed from the first
// #[instrument(level = "debug", skip(tcx), ret)]
// pub fn borrow_from<'tcx>(tcx: TyCtxt<'tcx>, _: ty::TypingEnv<'tcx>, tys: Vec<Ty<'tcx>>) -> bool {
//     /// Visit `ty` and collect the all the lifetimes appearing in it, implicit or not.
//     ///
//     /// The second field of the vector's elements indicate if the lifetime is attached to a
//     /// shared reference, a mutable reference, or neither.
//     #[instrument(level = "trace", skip(tcx), ret)]
//     fn get_lifetimes<'tcx>(ty: Ty<'tcx>, tcx: TyCtxt<'tcx>) -> Vec<(ty::EarlyParamRegion,
// Mutability)> {         let mut results = Vec::new();
//         let mut queue = vec![ty];
//         while let Some(ty) = queue.pop() {
//             match ty.kind() {
//                 TyKind::Ref(region, ty, mutability) => {
//                     match region.kind() {
//                         RegionKind::ReEarlyParam(param) => results.push((param, *mutability)),
//                         _ => (),
//                     }
//                     queue.push(*ty);
//                 },
//                 TyKind::Adt(adt_def, args) => {
//                     for field in adt_def.all_fields() {
//                         queue.push(field.ty(tcx, args));
//                     }
//                 },
//                 TyKind::Tuple(tys) => {
//                     for ty in tys.iter() {
//                         queue.push(ty);
//                     }
//                 },
//                 TyKind::Array(ty, _) | TyKind::Slice(ty) | TyKind::Pat(ty, _) |
// TyKind::RawPtr(ty, _) => {                     queue.push(*ty);
//                 },
//                 TyKind::Bool
//                 | TyKind::Char
//                 | TyKind::Int(_)
//                 | TyKind::Uint(_)
//                 | TyKind::Float(_)
//                 | TyKind::Foreign(_)
//                 | TyKind::Str
//                 | TyKind::FnDef(_, _)
//                 | TyKind::FnPtr(_, _)
//                 | TyKind::UnsafeBinder(_)
//                 | TyKind::Dynamic(_, _, _)
//                 | TyKind::Closure(_, _)
//                 | TyKind::CoroutineClosure(_, _)
//                 | TyKind::Coroutine(_, _)
//                 | TyKind::CoroutineWitness(_, _)
//                 | TyKind::Never
//                 | TyKind::Alias(_, _)
//                 | TyKind::Param(_)
//                 | TyKind::Bound(_, _) => {},
//                 TyKind::Placeholder(_) | TyKind::Infer(_) | TyKind::Error(_) => {},
//             }
//         }
//         results
//     }

//     if let Some((first_ty, remained_tys)) = tys.split_first() {
//         let regions = get_lifetimes(*first_ty, tcx);
//         // Check if all types are borrowed from the first type
//         // by checking if their regions are contained in the first type's regions.
//         let first_regions: FxHashSet<_> = regions.iter().map(|(vid, _)| vid).collect();
//         return remained_tys.iter().all(|ty| {
//             get_lifetimes(*ty, tcx)
//                 .iter()
//                 .any(|(vid, _)| first_regions.contains(vid))
//         });
//     }
//     true
// }
