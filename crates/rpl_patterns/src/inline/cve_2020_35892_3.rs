use rpl_context::PatCtxt;
use rpl_mir::{CheckMirCtxt, pat};
use rustc_errors::MultiSpan;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{self as hir};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::TyCtxt;
use rustc_span::{Span, Symbol};

use crate::lints::OFFSET_BY_ONE;

#[instrument(level = "info", skip_all)]
pub fn check_item(tcx: TyCtxt<'_>, pcx: PatCtxt<'_>, item_id: hir::ItemId) {
    let item = tcx.hir().item(item_id);
    // let def_id = item_id.owner_id.def_id;
    let mut check_ctxt = CheckFnCtxt::new(tcx, pcx);
    check_ctxt.visit_item(item);
}

struct CheckFnCtxt<'pcx, 'tcx> {
    tcx: TyCtxt<'tcx>,
    pcx: PatCtxt<'pcx>,
}

impl<'pcx, 'tcx> CheckFnCtxt<'pcx, 'tcx> {
    fn new(tcx: TyCtxt<'tcx>, pcx: PatCtxt<'pcx>) -> Self {
        Self { tcx, pcx }
    }
}

impl<'tcx> Visitor<'tcx> for CheckFnCtxt<'_, 'tcx> {
    type NestedFilter = All;
    fn nested_visit_map(&mut self) -> Self::Map {
        self.tcx.hir()
    }

    #[instrument(level = "debug", skip_all, fields(?item.owner_id))]
    fn visit_item(&mut self, item: &'tcx hir::Item<'tcx>) -> Self::Result {
        match item.kind {
            hir::ItemKind::Trait(hir::IsAuto::No, ..) | hir::ItemKind::Impl(_) | hir::ItemKind::Fn { .. } => {},
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

            let (pattern, fn_pat) = pattern_loop(self.pcx);
            let mir_pat = fn_pat.expect_mir_body();
            for matches in CheckMirCtxt::new(self.tcx, self.pcx, body, pattern, fn_pat).check() {
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
                            .map(move |(index, stmt)| (bb, index, stmt))
                    })
                    .for_each(|(bb, index, stmt)| {
                        let span = stmt.span_no_inline(body);
                        #[allow(rustc::untranslatable_diagnostic)]
                        multi_span.push_span_label(
                            span,
                            format!("{:?} <=> {:?}", mir_pat[bb].debug_stmt_at(index), stmt.debug_with(body)),
                        );
                    });
                #[expect(rustc::untranslatable_diagnostic)]
                #[expect(rustc::diagnostic_outside_of_impl)]
                self.tcx.dcx().span_note(multi_span, "MIR pattern matched");
            }
            let pattern = pattern_offset_by_len(self.pcx);
            for matches in CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.pattern, pattern.fn_pat).check() {
                let read = matches[pattern.read].span_no_inline(body);
                let ptr = matches[pattern.ptr].span_no_inline(body);
                let len = matches[pattern.len].span_no_inline(body);
                debug!(?ptr, ?read, ?len);
                let len_local = self
                    .tcx
                    .sess
                    .source_map()
                    .span_to_snippet(len)
                    .unwrap_or_else(|_| "{expr}".to_string());
                self.tcx.emit_node_span_lint(
                    OFFSET_BY_ONE,
                    self.tcx.local_def_id_to_hir_id(def_id),
                    read,
                    crate::errors::OffsetByOne {
                        read,
                        ptr,
                        len,
                        len_local,
                    },
                );
            }
        }
        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
}

#[rpl_macros::pattern_def]
fn pattern_loop(pcx: PatCtxt<'_>) -> (&pat::Pattern<'_>, &pat::Fn<'_>) {
    let pattern = rpl! {
        #[meta($T:ty)]
        struct $SlabT {
            $mem: *mut $T,
            $len: usize,
        }

        #[meta($T:ty)]
        fn $pattern (..) -> _ = mir! {
            let $self: &mut $SlabT;
            let $len: usize;
            let $x1: usize;
            let $x2: usize;
            let $opt: #[lang = "Option"]<usize>;
            let $discr: isize;
            let $x: usize;
            let $start_ref: &usize;
            let $end_ref: &usize;
            let $start: usize;
            let $end: usize;
            let $range: core::ops::range::Range<usize>;
            let mut $iter: core::ops::range::Range<usize>;
            let mut $iter_mut: &mut core::ops::range::Range<usize>;
            let mut $base: *mut $T;
            let $offset: isize;
            let $elem_ptr: *mut $T;
            let $cmp: bool;

            $len = copy (*$self).$len;
            $range = core::ops::range::Range { start: const 0_usize, end: move $len };
            $iter = move $range;
            loop {
                $iter_mut = &mut $iter;
                $start_ref = &(*$iter_mut).start;
                $start = copy *$start_ref;
                $end_ref = &(*$iter_mut).end;
                $end = copy *$end_ref;
                $cmp = Lt(move $start, move $end);
                switchInt(move $cmp) {
                    false => $opt = #[lang = "None"],
                    _ => {
                        $x1 = copy (*$iter_mut).start;
                        $x2 = core::iter::range::Step::forward_unchecked(copy $x1, const 1_usize);
                        (*$iter_mut).start = move $x2;
                        $opt = #[lang = "Some"](copy $x1);
                    }
                }
                $discr = discriminant($opt);
                switchInt(move $discr) {
                    0_isize => break,
                    1_isize => {
                        $x = copy ($opt as Some).0;
                        $base = copy (*$self).$mem;
                        $offset = copy $x as isize (IntToInt);
                        $elem_ptr = Offset(copy $base, copy $offset);
                        _ = core::ptr::drop_in_place(copy $elem_ptr);
                    }
                }
            }
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();
    (pattern, fn_pat)
}

struct PatternOffsetByLen<'pcx> {
    pattern: &'pcx pat::Pattern<'pcx>,
    fn_pat: &'pcx pat::Fn<'pcx>,
    len: pat::Location,
    ptr: pat::Location,
    read: pat::Location,
}

#[rpl_macros::pattern_def]
fn pattern_offset_by_len(pcx: PatCtxt<'_>) -> PatternOffsetByLen<'_> {
    let len;
    let ptr;
    let read;
    let pattern = rpl! {
        #[meta($T:ty)]
        struct $SlabT {
            $mem: *mut $T,
            $len: usize,
        }

        #[meta($T:ty)]
        fn $pattern(..) -> _ = mir! {
            let $self: &mut $SlabT;
            #[export(len)]
            let $len: usize = copy (*$self).$len;
            let $len_isize: isize = move $len as isize (IntToInt);
            let $base: *mut $T = copy (*$self).$mem;
            #[export(ptr)]
            let $ptr_mut: *mut $T = Offset(copy $base, copy $len_isize);
            let $ptr: *const $T = copy $ptr_mut as *const $T (PtrToPtr);
            #[export(read)]
            let $elem: $T = copy (*$ptr);
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    PatternOffsetByLen {
        pattern,
        fn_pat,
        len,
        ptr,
        read,
    }
}
