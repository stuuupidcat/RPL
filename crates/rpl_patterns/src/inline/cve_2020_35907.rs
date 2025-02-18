use rpl_context::PatCtxt;
use rpl_mir::{pat, CheckMirCtxt};
use rustc_hir::def_id::{LocalDefId, CRATE_DEF_ID};
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{self as hir};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::{self, Ty, TyCtxt, TypingMode};
use rustc_span::{Span, Symbol};

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
        _span: Span,
        def_id: LocalDefId,
    ) -> Self::Result {
        let vis = self.tcx.local_visibility(def_id);
        if self.tcx.is_mir_available(def_id)
            && (vis == ty::Visibility::Public || vis == ty::Visibility::Restricted(CRATE_DEF_ID))
        {
            let body = self.tcx.optimized_mir(def_id);

            let pattern = pattern_thread_local_static(self.pcx);
            for matches in CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.pattern, pattern.fn_pat).check() {
                let thread_local = matches[pattern.thread_local].span_no_inline(body);
                let ret = matches[pattern.ret].span_no_inline(body);
                let ty = matches[pattern.ty_var.idx];
                debug!(?thread_local, ?ty);
                let span = decl.output.span();
                self.tcx.dcx().emit_err(crate::errors::ThreadLocalStaticRef {
                    span,
                    thread_local,
                    ret,
                    ty,
                });
            }
        }
        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
}

struct PatternThreadLocalStatic<'pcx> {
    pattern: &'pcx pat::Pattern<'pcx>,
    fn_pat: &'pcx pat::Fn<'pcx>,
    thread_local: pat::Location,
    ret: pat::Location,
    ty_var: pat::TyVar,
}

#[rpl_macros::pattern_def]
fn pattern_thread_local_static(pcx: PatCtxt<'_>) -> PatternThreadLocalStatic<'_> {
    let ty_var;
    let thread_local;
    let ret;
    #[allow(non_snake_case)]
    let pattern = rpl! {
        #[meta(#[export(ty_var)] $T:ty = is_sync)]
        // FIXME: the return type is not actually checked to be matched
        fn $pattern(..) -> &'static $T = mir! {
            #[export(thread_local)]
            let $local_key: &std::thread::LocalKey::<std::cell::UnsafeCell<$T>> = _;
            #[export(ret)]
            let $RET: &T =
                std::thread::LocalKey::<std::cell::UnsafeCell<$T>>::with::<_, _>(move $local_key, _);
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    PatternThreadLocalStatic {
        pattern,
        fn_pat,
        thread_local,
        ret,
        ty_var,
    }
}

#[instrument(level = "debug", skip(tcx), ret)]
fn is_sync<'tcx>(tcx: TyCtxt<'tcx>, typing_env: ty::TypingEnv<'tcx>, ty: Ty<'tcx>) -> bool {
    use rustc_infer::infer::TyCtxtInferExt;
    let infcx = tcx.infer_ctxt().build(TypingMode::PostAnalysis);
    let trait_def_id = tcx.require_lang_item(hir::LangItem::Sync, None);
    rustc_trait_selection::traits::type_known_to_meet_bound_modulo_regions(
        &infcx,
        typing_env.param_env,
        ty,
        trait_def_id,
    )
}
