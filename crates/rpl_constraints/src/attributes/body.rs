use core::ops::ControlFlow;

use rustc_hir::intravisit::{Visitor, walk_block};
use rustc_hir::{Block, BlockCheckMode, Expr, UnsafeSource};
use rustc_middle::hir::nested_filter;
use rustc_middle::ty::TyCtxt;

/// Checks if the given expression contains an unsafe block
pub fn contains_unsafe_block<'tcx>(tcx: TyCtxt<'tcx>, e: &'tcx Expr<'tcx>) -> bool {
    struct V<'tcx> {
        tcx: TyCtxt<'tcx>,
    }
    impl<'tcx> Visitor<'tcx> for V<'tcx> {
        type Result = ControlFlow<()>;
        type NestedFilter = nested_filter::OnlyBodies;

        fn maybe_tcx(&mut self) -> Self::MaybeTyCtxt {
            self.tcx
        }

        fn visit_block(&mut self, b: &'tcx Block<'_>) -> Self::Result {
            if b.rules == BlockCheckMode::UnsafeBlock(UnsafeSource::UserProvided) {
                ControlFlow::Break(())
            } else {
                walk_block(self, b)
            }
        }
    }
    let mut v = V { tcx };
    v.visit_expr(e).is_break()
}
