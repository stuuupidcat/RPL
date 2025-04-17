#![allow(unused)]
use std::ops::Not;

use rpl_context::PatCtxt;
use rpl_mir::{CheckMirCtxt, pat};
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::{self, Ty, TyCtxt};
use rustc_span::{Span, Symbol};

#[instrument(level = "info", skip_all)]
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
            | hir::ItemKind::Fn { .. } => {},
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
            let pattern_transmute_int_to_ptr = pattern_transmute_int_to_ptr(self.pcx);
            for matches in CheckMirCtxt::new(
                self.tcx,
                self.pcx,
                body,
                pattern_transmute_int_to_ptr.pattern,
                pattern_transmute_int_to_ptr.fn_pat,
            )
            .check()
            {
                let transmute_from = matches[pattern_transmute_int_to_ptr.transmute_from].span_no_inline(body);
                let transmute_to = matches[pattern_transmute_int_to_ptr.transmute_to].span_no_inline(body);
                debug!(?transmute_from, ?transmute_to);
                self.tcx.emit_node_span_lint(
                    crate::lints::TRANSMUTING_INT_TO_PTR,
                    self.tcx.local_def_id_to_hir_id(def_id),
                    transmute_from,
                    crate::errors::TransmutingIntToPtr {
                        from: transmute_from,
                        to: transmute_to,
                    },
                );
            }
        }
        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
}

struct PatternTransmute<'pcx> {
    pattern: &'pcx pat::Pattern<'pcx>,
    fn_pat: &'pcx pat::Fn<'pcx>,
    transmute_from: pat::Location,
    transmute_to: pat::Location,
}

#[rpl_macros::pattern_def]
fn pattern_transmute_int_to_ptr(pcx: PatCtxt<'_>) -> PatternTransmute<'_> {
    let transmute_from;
    let transmute_to;
    let pattern = rpl! {
        #[meta($INT: ty = is_integral, $PTR:ty = is_ptr)]
        fn $pattern (..) -> _ = mir! {
            #[export(transmute_from)]
            let $transmute_from: $INT = _;
            #[export(transmute_to)]
            // FIXME: move and copy are both allowed here
            let $transmute_to: $PTR = copy $transmute_from as $PTR (Transmute);
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    PatternTransmute {
        pattern,
        fn_pat,
        transmute_from,
        transmute_to,
    }
}

#[instrument(level = "debug", skip(tcx), ret)]
#[allow(unused_variables)]
fn is_integral<'tcx>(tcx: TyCtxt<'tcx>, typing_env: ty::TypingEnv<'tcx>, ty: Ty<'tcx>) -> bool {
    ty.is_integral()
}

#[instrument(level = "debug", skip(tcx), ret)]
#[allow(unused_variables)]
fn is_ptr<'tcx>(tcx: TyCtxt<'tcx>, typing_env: ty::TypingEnv<'tcx>, ty: Ty<'tcx>) -> bool {
    ty.is_any_ptr()
}
