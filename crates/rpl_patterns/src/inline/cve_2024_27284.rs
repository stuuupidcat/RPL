use rpl_context::PatCtxt;
use rpl_mir::{CheckMirCtxt, pat};
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::TyCtxt;
use rustc_span::{Span, Symbol};
use std::ops::Not;

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
            let pattern = pattern_cass_iter_next_aggmeta(self.pcx);
            for matches in CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.pattern, pattern.fn_pat).check() {
                let cass_iter_next = matches[pattern.cass_iter_next].span_no_inline(body);
                self.tcx.emit_node_span_lint(
                    crate::lints::CASSANDRA_ITER_NEXT_PTR_PASSED_TO_CASS_ITER_GET,
                    self.tcx.local_def_id_to_hir_id(def_id),
                    cass_iter_next,
                    crate::errors::CassandraIterNextPtrPassedToCassIterGet { cass_iter_next },
                );
            }
        }
        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
}

struct CassandraIterNextPtrPassedToCassIterGet<'pcx> {
    pattern: &'pcx pat::Pattern<'pcx>,
    fn_pat: &'pcx pat::Fn<'pcx>,
    cass_iter_next: pat::Location,
}

#[rpl_macros::pattern_def]
fn pattern_cass_iter_next_aggmeta(pcx: PatCtxt<'_>) -> CassandraIterNextPtrPassedToCassIterGet<'_> {
    let cass_iter_next;
    let pattern = rpl! {
        fn $pattern (..) -> _ = mir! {
            type CassIterator = cassandra_cpp_sys::CassIterator_;
            type CassBool = cassandra_cpp_sys::cass_bool_t;
            type AggregateMeta = cassandra_cpp_sys::CassAggregateMeta_;


            let $cass_mut_iter1: *mut CassIterator = _;
            #[export(cass_iter_next)]
            let $next_res: CassBool = cassandra_cpp_sys::cass_iterator_next(move $cass_mut_iter1);
            let $discr: u32 = discriminant($next_res);

            let $cass_mut_iter2: *mut CassIterator;
            let $cass_const_iter: *const CassIterator;
            let $cass_meta_ptr: *const AggregateMeta;

            switchInt(move $discr) {
                0_usize => {
                    $cass_mut_iter2 = _;
                    $cass_const_iter = move $cass_mut_iter2 as *const CassIterator (PtrToPtr);
                    $cass_meta_ptr = cassandra_cpp_sys::cass_iterator_get_aggregate_meta(move $cass_const_iter);
                }
                _ => {}
            }
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    CassandraIterNextPtrPassedToCassIterGet {
        pattern,
        fn_pat,
        cass_iter_next,
    }
}
