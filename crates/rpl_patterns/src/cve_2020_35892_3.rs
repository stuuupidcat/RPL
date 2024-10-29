use rpl_mir::pat::{self, PatternsBuilder};
use rpl_mir::CheckMirCtxt;
use rustc_errors::MultiSpan;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{self as hir};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::TyCtxt;
use rustc_span::Span;

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

    #[instrument(level = "debug", skip_all, fields(?item.owner_id))]
    fn visit_item(&mut self, item: &'tcx hir::Item<'tcx>) -> Self::Result {
        match item.kind {
            hir::ItemKind::Trait(hir::IsAuto::No, ..) | hir::ItemKind::Impl(_) | hir::ItemKind::Fn(..) => {},
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
        span: Span,
        def_id: LocalDefId,
    ) -> Self::Result {
        if self.tcx.is_mir_available(def_id) {
            let body = self.tcx.optimized_mir(def_id);
            let mut patterns = PatternsBuilder::new(&self.tcx.arena.dropless);
            pattern_loop(&mut patterns);
            let patterns = patterns.build();
            if let Some(matches) = CheckMirCtxt::new(self.tcx, body, &patterns).check() {
                let mut multi_span = MultiSpan::from_span(span);
                matches
                    .basic_blocks
                    .iter_enumerated()
                    .flat_map(|(bb, block)| {
                        block
                            .statements
                            .iter()
                            .copied()
                            .enumerate()
                            .filter_map(move |(index, stmt)| Some((bb, index, stmt?)))
                    })
                    .for_each(|(bb, index, stmt)| {
                        let span = stmt.span_no_inline(body);
                        #[allow(rustc::untranslatable_diagnostic)]
                        multi_span.push_span_label(
                            span,
                            format!(
                                "{:?} <=> {:?}",
                                patterns[bb].debug_stmt_at(index),
                                stmt.debug_with(body)
                            ),
                        );
                    });
                #[allow(rustc::untranslatable_diagnostic)]
                #[allow(rustc::diagnostic_outside_of_impl)]
                self.tcx.dcx().span_err(multi_span, "MIR pattern matched");
            }
        }
        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
}

#[rpl_macros::mir_pattern]
fn pattern_loop(patterns: &mut pat::PatternsBuilder<'_>) {
    mir! {
        meta!($T:ty, $SlabT:ty);

        let self: &mut $SlabT;
        let len: usize; // _2
        let mut x0: usize; // _17
        let x1: usize; // _14
        let x2: usize; // _15
        let x3: #[lang = "Option"]<usize>; // _3
        let x: usize; // _4
        let mut base: *mut $T; // _6
        let offset: isize; // _7
        let elem_ptr: *mut $T; // _5
        let x_cmp: usize; // _16
        let cmp: bool; // _13

        len = copy (*self).len;
        x0 = const 0_usize;
        loop {
            x_cmp = copy x0;
            cmp = Lt(move x_cmp, copy len);
            switchInt(move cmp) {
                false => break,
                _ => {
                    x1 = copy x0;
                    x2 = core::iter::range::Step::forward_unchecked(copy x1, const 1_usize);
                    // x0 = move x2;
                    x3 = #[lang = "Some"](copy x1);
                    x = copy (x3 as Some).0;
                    base = copy (*self).mem;
                    offset = copy x as isize (IntToInt);
                    elem_ptr = Offset(copy base, copy offset);
                    _ = core::ptr::drop_in_place(copy elem_ptr);
                }
            }
        }
    }

    patterns.set_ty_var(SlabT_ty_var, |_tcx, _params_env, ty| ty.is_adt());
}
