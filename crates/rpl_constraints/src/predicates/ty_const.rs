use rustc_middle::mir;
use rustc_middle::ty::{self, Ty, TyCtxt};

use crate::Const;

pub type TyConstPredsFnPtr =
    for<'tcx> fn(TyCtxt<'tcx>, body: &mir::Body<'tcx>, ty::TypingEnv<'tcx>, Ty<'tcx>, Const<'tcx>) -> bool;

/// Check if `alignment` is enough for the given type `ty`.
#[instrument(level = "debug", skip(tcx), ret)]
pub fn maybe_misaligned<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &mir::Body<'tcx>,
    typing_env: ty::TypingEnv<'tcx>,
    ty: Ty<'tcx>,
    alignment: Const<'tcx>,
) -> bool {
    let typing_env = ty::TypingEnv::post_analysis(tcx, body.source.def_id());
    match ty.kind() {
        // Param types can be anything, and we don't know the alignment.
        // Also, param types with unsafe traits have been filtered out in `is_all_safe_trait`.
        ty::TyKind::Param(_) => true,
        // foreign types are opaque to Rust
        ty::TyKind::Foreign(_) => true,
        _ => {
            let layout = tcx.layout_of(typing_env.as_query_input(ty)).unwrap();
            alignment
                .try_eval_target_usize(tcx, typing_env)
                .is_none_or(|alignment| alignment < layout.align.abi.bytes())
        },
    }
}

/// Check if the constant `size` is `size_of::<ty>()`.
///
/// Since the MIR `NullOp::SizeOf` was removed, `size_of::<T>()` lowers to the
/// associated constant `<T as core::mem::SizedTypeProperties>::SIZE`. When that
/// const is folded into a const operand of surrounding arithmetic
/// (e.g. `Mul(const <T>::SIZE, const N)`) there is no longer a `let $size = SizeOf($T)`
/// statement to bind, so patterns bind the const operand directly and check it here.
///
/// We deliberately match only the *symbolic* unevaluated `SizedTypeProperties::SIZE`
/// const, NOT a const that merely *evaluates* to `size_of::<ty>()`. A value-based
/// comparison would coincidentally match any literal that happens to equal the type's
/// size (e.g. a `* 2` factor when `ty` is `u16`), producing false positives.
#[instrument(level = "debug", skip(tcx), ret)]
pub fn is_size_of<'tcx>(
    tcx: TyCtxt<'tcx>,
    _body: &mir::Body<'tcx>,
    _typing_env: ty::TypingEnv<'tcx>,
    ty: Ty<'tcx>,
    size: Const<'tcx>,
) -> bool {
    // Symbolic form: `<ty as SizedTypeProperties>::SIZE` (what `size_of::<ty>()`
    // lowers to). Matches even for generic/param `ty`, since we compare type args.
    //
    // Use `opt_item_name` (not `item_name`): `size` is an arbitrary bound constant, so
    // `uv.def` / its parent can be a nameless item (e.g. an anonymous or closure-scoped
    // const), and `item_name` ICEs on those.
    if let Const::MIR(mir::Const::Unevaluated(uv, _)) = size
        && tcx.opt_item_name(uv.def).is_some_and(|name| name.as_str() == "SIZE")
        && tcx
            .opt_parent(uv.def)
            .and_then(|parent| tcx.opt_item_name(parent))
            .is_some_and(|name| name.as_str() == "SizedTypeProperties")
    {
        return uv.args.type_at(0) == ty;
    }
    false
}
