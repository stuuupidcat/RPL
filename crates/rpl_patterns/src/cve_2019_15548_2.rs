use std::ops::Not;

use rpl_context::PatCtxt;
use rpl_mir::pat::MirPattern;
use rpl_mir::{pat, CheckMirCtxt};
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::TyCtxt;
use rustc_span::Span;

use crate::lints::LENGTHLESS_BUFFER_PASSED_TO_EXTERN_FUNCTION;

#[instrument(level = "info", skip(tcx, pcx))]
pub fn check_item(tcx: TyCtxt<'_>, pcx: PatCtxt<'_>, item_id: hir::ItemId) {
    let item = tcx.hir().item(item_id);
    // let def_id = item_id.owner_id.def_id;
    let mut check_ctxt = CheckFnCtxt { tcx, pcx };
    check_ctxt.visit_item(item);
}

struct CheckFnCtxt<'pcx, 'tcx> {
    tcx: TyCtxt<'tcx>,
    pcx: PatCtxt<'pcx>,
}

impl<'tcx> Visitor<'tcx> for CheckFnCtxt<'_, 'tcx> {
    type NestedFilter = All;
    fn nested_visit_map(&mut self) -> Self::Map {
        self.tcx.hir()
    }

    #[instrument(level = "debug", skip_all, fields(?item.owner_id))]
    fn visit_item(&mut self, item: &'tcx hir::Item<'tcx>) -> Self::Result {
        match item.kind {
            hir::ItemKind::Trait(hir::IsAuto::No, hir::Safety::Safe, ..)
            | hir::ItemKind::Impl(_)
            | hir::ItemKind::Fn(..) => {},
            _ => return,
        }
        intravisit::walk_item(self, item);
    }

    #[instrument(level = "info", skip_all, fields(?def_id))]
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
            #[allow(irrefutable_let_patterns)]
            if let pattern_ptr = pattern_pass_a_pointer_to_c(self.pcx)
                && let Some(matches) = CheckMirCtxt::new(self.tcx, self.pcx, body, &pattern_ptr.pattern).check()
                && let Some(ptr) = matches[pattern_ptr.ptr]
                && let ptr = ptr.span_no_inline(body)
            {
                debug!(?ptr);
                self.tcx.emit_node_span_lint(
                    LENGTHLESS_BUFFER_PASSED_TO_EXTERN_FUNCTION,
                    self.tcx.local_def_id_to_hir_id(def_id),
                    ptr,
                    crate::errors::LengthlessBufferPassedToExternFunction { ptr },
                );
            }
        }
        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
}

struct PatternPointer<'pcx> {
    pattern: MirPattern<'pcx>,
    ptr: pat::Location,
}

// FIXME: this should work for functions other than `crate::ll::instr`.
#[rpl_macros::pattern_def]
fn pattern_pass_a_pointer_to_c(pcx: PatCtxt<'_>) -> PatternPointer<'_> {
    rpl! {
        fn $pattern (..) -> _ = mir! {
            type c_char = libc::c_char;

            let ptr: *const c_char = _;
            _ = $crate::ll::instr(move ptr);
        }
    }

    PatternPointer {
        pattern,
        ptr: ptr_stmt,
        // ty_var: c_char_ty,
    }
}
