use std::cell::RefCell;

use rpl_context::PatCtxt;
pub use rpl_context::pat;
pub use rpl_context::pat::MatchedMap;
use rustc_data_structures::fx::FxIndexSet;
use rustc_index::IndexVec;
use rustc_index::bit_set::MixedBitSet;
use rustc_middle::mir::interpret::PointerArithmetic;
use rustc_middle::ty::TyCtxt;
use rustc_middle::{mir, ty};
use rustc_span::Symbol;

use crate::graph::{MirControlFlowGraph, MirDataDepGraph, PatControlFlowGraph, PatDataDepGraph};
use crate::matches::{Matched, matches};
use crate::statement::MatchStatement;
use crate::ty::MatchTy as _;
use crate::{MatchPlaceCtxt, MatchTyCtxt};

/// The public entry point alias for downstream crates.
pub type CheckMirCtxt<'a, 'pcx, 'tcx> = MatchContext<'a, 'pcx, 'tcx>;

/// Read-only context for pattern matching against MIR.
///
/// Holds all the immutable data needed for matching: the MIR body,
/// the pattern, graph representations, and type/place matching contexts.
/// Interior mutability (RefCell) is used for candidate accumulation
/// during the candidate-building phase.
pub struct MatchContext<'a, 'pcx, 'tcx> {
    pub(crate) ty: MatchTyCtxt<'pcx, 'tcx>,
    pub(crate) place: MatchPlaceCtxt<'pcx, 'tcx>,
    pub(crate) body: &'a mir::Body<'tcx>,
    pub(crate) has_self: bool,
    pub(crate) self_ty: Option<ty::Ty<'tcx>>,
    pub(crate) pat_name: Symbol,
    pub(crate) fn_pat: &'a pat::FnPattern<'pcx>,
    pub(crate) mir_pat: &'a pat::FnPatternBody<'pcx>,
    pub(crate) pat_cfg: PatControlFlowGraph,
    pub(crate) pat_ddg: PatDataDepGraph,
    pub(crate) mir_cfg: &'a MirControlFlowGraph,
    pub(crate) mir_ddg: &'a MirDataDepGraph,
    pub(crate) locals: IndexVec<pat::Local, RefCell<MixedBitSet<mir::Local>>>,
    pub(crate) places: IndexVec<pat::PlaceVarIdx, RefCell<FxIndexSet<mir::PlaceRef<'tcx>>>>,
}

impl<'a, 'pcx, 'tcx> MatchContext<'a, 'pcx, 'tcx> {
    #[expect(clippy::too_many_arguments)]
    #[instrument(level = "debug", skip_all, fields(
        def_id = ?body.source.def_id(),
        pat_name = ?pat_name,
        ?has_self,
        ?self_ty,
    ))]
    pub fn new(
        tcx: TyCtxt<'tcx>,
        pcx: PatCtxt<'pcx>,
        body: &'a mir::Body<'tcx>,
        has_self: bool,
        self_ty: Option<ty::Ty<'tcx>>,
        pat: &'pcx pat::RustItems<'pcx>,
        pat_name: Symbol,
        fn_pat: &'a pat::FnPattern<'pcx>,
        mir_cfg: &'a MirControlFlowGraph,
        mir_ddg: &'a MirDataDepGraph,
    ) -> Self {
        let typing_env = ty::TypingEnv::post_analysis(tcx, body.source.def_id());
        let ty = MatchTyCtxt::new(tcx, pcx, typing_env, self_ty, pat, &fn_pat.meta);
        let place = MatchPlaceCtxt::new(tcx, pcx, &fn_pat.meta);
        let mir_pat = fn_pat.expect_body();
        let pat_cfg = crate::graph::pat_control_flow_graph(mir_pat, tcx.pointer_size().bytes());
        let pat_ddg = crate::graph::pat_data_dep_graph(mir_pat, &pat_cfg);
        Self {
            ty,
            place,
            body,
            has_self,
            self_ty,
            pat_name,
            fn_pat,
            mir_pat,
            pat_cfg,
            pat_ddg,
            mir_cfg,
            mir_ddg,
            locals: IndexVec::from_elem_n(
                RefCell::new(MixedBitSet::new_empty(body.local_decls.len())),
                mir_pat.locals.len(),
            ),
            places: IndexVec::from_elem_n(RefCell::new(FxIndexSet::default()), fn_pat.meta.place_vars.len()),
        }
    }
    #[instrument(level = "info", skip_all, fields(
        def_id = ?self.body.source.def_id(),
        pat_name = ?self.pat_name,
    ))]
    pub fn check(&self) -> Vec<Matched<'tcx>> {
        matches(self)
    }
}

impl<'pcx, 'tcx> MatchStatement<'pcx, 'tcx> for MatchContext<'_, 'pcx, 'tcx> {
    fn body(&self) -> &mir::Body<'tcx> {
        self.body
    }
    fn fn_pat(&self) -> &pat::FnPattern<'pcx> {
        self.fn_pat
    }
    fn mir_pat(&self) -> &pat::FnPatternBody<'pcx> {
        self.mir_pat
    }

    fn pat_cfg(&self) -> &PatControlFlowGraph {
        &self.pat_cfg
    }
    fn pat_ddg(&self) -> &PatDataDepGraph {
        &self.pat_ddg
    }
    fn mir_cfg(&self) -> &MirControlFlowGraph {
        self.mir_cfg
    }
    fn mir_ddg(&self) -> &MirDataDepGraph {
        self.mir_ddg
    }

    fn pat(&self) -> &'pcx pat::RustItems<'pcx> {
        self.ty.pat
    }
    fn pcx(&self) -> PatCtxt<'pcx> {
        self.ty.pcx
    }
    fn tcx(&self) -> TyCtxt<'tcx> {
        self.ty.tcx
    }
    fn typing_env(&self) -> ty::TypingEnv<'tcx> {
        self.ty.typing_env
    }

    type MatchTy = MatchTyCtxt<'pcx, 'tcx>;
    fn ty(&self) -> &Self::MatchTy {
        &self.ty
    }

    #[instrument(level = "debug", skip(self), ret)]
    fn match_local(&self, pat: pat::Local, local: mir::Local) -> bool {
        let mut locals = self.locals[pat].borrow_mut();
        debug!(?locals, ?pat, ?local, "match_local");
        if locals.contains(local) {
            return true;
        }

        if self.mir_pat.params_idx.contains(&pat) {
            // If the local variable is a parameter, we only need to match the
            // corresponding local variable in the MIR graph.
            trace!(?pat, "expected parameter");
            if !(local.as_usize() > 0 && local.as_usize() <= self.body.arg_count) {
                debug!(?local, "found non-parameter local");
                return false;
            }
        }

        let matched = self
            .ty()
            .match_ty(self.mir_pat().locals[pat], self.body().local_decls[local].ty);
        debug!(?pat, ?local, matched, "match_local");
        if matched {
            locals.insert(local);
        }
        matched
    }
    #[instrument(level = "trace", skip(self), ret)]
    fn match_place_var(&self, pat: pat::PlaceVarIdx, place: mir::PlaceRef<'tcx>) -> bool {
        let mut places = self.places[pat].borrow_mut();
        trace!(?places, ?pat, ?place, "match_place_var");
        if places.contains(&place) {
            return true;
        }
        let place_ty = place.ty(&self.body.local_decls, self.ty().tcx);
        let matched = self.ty().match_ty(self.place.places[pat], place_ty.ty);
        debug!(?pat, ?place, matched, "match_place_var");
        if matched {
            places.insert(place);
        }
        matched
    }

    fn get_place_ty_from_place_var(&self, var: pat::PlaceVarIdx) -> pat::PlaceTy<'pcx> {
        pat::PlaceTy::from_ty(self.place.places[var])
    }
}
