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
    pub ty_vars: IndexVec<TyVarIdx, TyVar>,
    pub const_vars: IndexVec<ConstVarIdx, ConstVar<'pcx>>,
}

pub struct Pattern<'pcx> {
    pub pcx: PatCtxt<'pcx>,
    adts: FxHashMap<Symbol, Adt<'pcx>>,
    pub fns: Fns<'pcx>,
    #[expect(dead_code)]
    impls: Vec<Impl<'pcx>>,
}

impl<'pcx> MetaVars<'pcx> {
    pub fn new_ty_var(&mut self, pred: Option<TyPred>) -> TyVar {
        let idx = self.ty_vars.next_index();
        let ty_var = TyVar { idx, pred };
        self.ty_vars.push(ty_var);
        ty_var
    }
    pub fn new_const_var(&mut self, ty: Ty<'pcx>) -> ConstVar<'pcx> {
        let idx = self.const_vars.next_index();
        let const_var = ConstVar { idx, ty };
        self.const_vars.push(const_var);
        const_var
    }
}

impl<'pcx> Pattern<'pcx> {
    pub(crate) fn new(pcx: PatCtxt<'pcx>) -> Self {
        Self {
            pcx,
            adts: Default::default(),
            fns: Default::default(),
            impls: Default::default(),
        }
    }
    pub fn new_struct(&mut self, name: Symbol) -> &mut Variant<'pcx> {
        self.adts
            .entry(name)
            .or_insert_with(Adt::new_struct)
            .non_enum_variant_mut()
    }
    pub fn new_enum(&mut self, name: Symbol) -> &mut Adt<'pcx> {
        self.adts.entry(name).or_insert_with(Adt::new_enum)
    }
}
