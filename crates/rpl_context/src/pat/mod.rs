use rustc_data_structures::fx::FxHashMap;
use rustc_index::IndexVec;
use rustc_span::Symbol;

use crate::PatCtxt;

mod item;
mod mir;
mod pretty;
mod ty;

pub use item::*;
pub use mir::*;
pub use ty::*;

#[derive(Default)]
pub struct MetaVars<'pcx> {
    ty_vars: IndexVec<TyVarIdx, TyVar>,
    const_vars: IndexVec<ConstVarIdx, ConstVar<'pcx>>,
}

pub struct Pattern<'pcx> {
    pub pcx: PatCtxt<'pcx>,
    pub meta: MetaVars<'pcx>,
    #[expect(dead_code)]
    adts: FxHashMap<Symbol, AdtDef<'pcx>>,
    #[expect(dead_code)]
    fns: FxHashMap<Symbol, FnDef<'pcx>>,
    #[expect(dead_code)]
    unnamed_fns: Vec<FnDef<'pcx>>,
    #[expect(dead_code)]
    impls: Vec<ImplDef<'pcx>>,
}

impl<'pcx> MetaVars<'pcx> {
    pub fn mk_ty_var(&mut self, pred: Option<TyPred>) -> TyVar {
        let idx = self.ty_vars.next_index();
        let ty_var = TyVar { idx, pred };
        self.ty_vars.push(ty_var);
        ty_var
    }
    pub fn mk_const_var(&mut self, ty: Ty<'pcx>) -> ConstVar<'pcx> {
        let idx = self.const_vars.next_index();
        let const_var = ConstVar { idx, ty };
        self.const_vars.push(const_var);
        const_var
    }
}
