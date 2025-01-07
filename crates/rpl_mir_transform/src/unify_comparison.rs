use std::mem::swap;

use rustc_middle::mir::{BinOp, Body, Rvalue, StatementKind};
use rustc_middle::ty::TyCtxt;

// #[instrument(level = "info", skip_all)]
// pub(crate) fn transform(tcx: TyCtxt<'_>, item_id: hir::ItemId) {
//     let item = tcx.hir().item(item_id);
//     let mut check_ctxt = UnifyCmp { tcx };
//     check_ctxt.visit_item(item);
// }
// pub(crate) struct UnifyCmp<'tcx> {
//     pub tcx: TyCtxt<'tcx>,
// }

/// Convert comparison operations to `Lt` or `Le`.
/// For an example, it turns something like
///
/// ```ignore (MIR)
/// _3 = Gt(move _1, move _2);
/// _4 = Ge(move _1, move _2);
/// ```
///
/// into:
///
/// ```ignore (MIR)
/// _3 = Lt(move _2, move _1);
/// _4 = Le(move _1, move _2);
/// ```
pub(crate) fn unify_comparison(_tcx: TyCtxt<'_>, body: &mut Body<'_>) {
    for block in body.basic_blocks_mut() {
        for statement in &mut block.statements {
            if let StatementKind::Assign(box (_, Rvalue::BinaryOp(op, box (lhs, rhs)))) = &mut statement.kind {
                *op = match op {
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le => continue,
                    BinOp::Gt => BinOp::Lt,
                    BinOp::Ge => BinOp::Le,
                    _ => continue,
                };
                swap(lhs, rhs);
            }
        }
    }
}
