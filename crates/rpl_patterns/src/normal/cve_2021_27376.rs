use rpl_context::PatCtxt;
use rpl_mir::{pat, CheckMirCtxt};
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::TyCtxt;
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
            | hir::ItemKind::Fn{..} => {},
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
        if self.tcx.is_mir_available(def_id) {
            let body = self.tcx.optimized_mir(def_id);
            let pattern = pattern_cast_socket_addr_v6(self.pcx);
            for matches in CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.pattern, pattern.fn_pat).check() {
                let cast_from = matches[pattern.cast_from].span_no_inline(body);
                let cast_to = matches[pattern.cast_to].span_no_inline(body);
                debug!(?cast_from, ?cast_to);
                self.tcx
                    .dcx()
                    .emit_err(crate::errors::WrongAssumptionOfLayoutCompatibility {
                        cast_from,
                        cast_to,
                        type_from: pattern.type_from,
                        type_to: pattern.type_to,
                    });
            }
            let pattern = pattern_cast_socket_addr_v4(self.pcx);
            for matches in CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.pattern, pattern.fn_pat).check() {
                let cast_from = matches[pattern.cast_from].span_no_inline(body);
                let cast_to = matches[pattern.cast_to].span_no_inline(body);
                debug!(?cast_from, ?cast_to);
                self.tcx
                    .dcx()
                    .emit_err(crate::errors::WrongAssumptionOfLayoutCompatibility {
                        cast_from,
                        cast_to,
                        type_from: pattern.type_from,
                        type_to: pattern.type_to,
                    });
            }
        }
        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
}

struct PatternCast<'pcx> {
    pattern: &'pcx pat::Pattern<'pcx>,
    fn_pat: &'pcx pat::Fn<'pcx>,
    cast_from: pat::Location,
    cast_to: pat::Location,
    type_from: &'static str,
    type_to: &'static str,
}

#[rpl_macros::pattern_def]
fn pattern_cast_socket_addr_v6(pcx: PatCtxt<'_>) -> PatternCast<'_> {
    let cast_from;
    let cast_to;
    let pattern = rpl! {
        fn $pattern (..) -> _ = mir! {
            #[export(cast_from)]
            let $src: *const std::net::SocketAddrV6 = _;
            #[export(cast_to)]
            let $dst: *const libc::sockaddr = move $src as *const libc::sockaddr (PtrToPtr);
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    PatternCast {
        pattern,
        fn_pat,

        cast_from,
        cast_to,
        type_from: "std::net::SocketAddrV6",
        type_to: "libc::sockaddr",
    }
}

#[rpl_macros::pattern_def]
fn pattern_cast_socket_addr_v4(pcx: PatCtxt<'_>) -> PatternCast<'_> {
    let cast_from;
    let cast_to;
    let pattern = rpl! {
        fn $pattern (..) -> _ = mir! {
            #[export(cast_from)]
            let $src: *const std::net::SocketAddrV4 = _;
            #[export(cast_to)]
            let $dst: *const libc::sockaddr = move $src as *const libc::sockaddr (PtrToPtr);
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    PatternCast {
        pattern,
        fn_pat,
        cast_from,
        cast_to,
        type_from: "std::net::SocketAddrV4",
        type_to: "libc::sockaddr",
    }
}
