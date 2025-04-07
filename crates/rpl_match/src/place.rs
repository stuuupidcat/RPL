use rpl_context::{PatCtxt, pat};
use rustc_index::IndexVec;
use rustc_middle::ty::TyCtxt;

pub struct MatchPlaceCtxt<'pcx, 'tcx> {
    pub tcx: TyCtxt<'tcx>,
    pub pcx: PatCtxt<'pcx>,
    pub places: IndexVec<pat::PlaceVarIdx, pat::Ty<'pcx>>,
}

impl<'pcx, 'tcx> MatchPlaceCtxt<'pcx, 'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>, pcx: PatCtxt<'pcx>, meta: &pat::NonLocalMetaVars<'pcx>) -> Self {
        let places = meta.place_vars.iter().map(|var| var.ty).collect(); //FIXME: implement this
        Self { tcx, pcx, places }
    }
}
