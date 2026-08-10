use std::ops::Deref;

use rpl_constraints::predicates::PredicateConjunction;
use rpl_meta::collect_elems_separated_by_comma;
use rpl_meta::symbol_table::{GetType, WithPath};
use rpl_parser::generics::Choice4;
use rpl_parser::pairs;
use rustc_index::IndexVec;
use rustc_span::Symbol;

use crate::PatCtxt;
use crate::pat::{ColumnType, Local, TableHead, Ty, with_path};

rustc_index::newtype_index! {
    #[debug_format = "?T{}"]
    #[orderable]
    pub struct TyVarIdx {}
}

rustc_index::newtype_index! {
    #[debug_format = "?C{}"]
    #[orderable]
    pub struct ConstVarIdx {}
}

rustc_index::newtype_index! {
    #[debug_format = "?A{}"]
    #[orderable]
    pub struct AdtVarIdx {}
}

rustc_index::newtype_index! {
    #[debug_format = "?P{}"]
    #[orderable]
    pub struct PlaceVarIdx {}
}

#[derive(Clone)]
pub struct TyVar {
    pub idx: TyVarIdx,
    pub name: Symbol,
    pub pred: PredicateConjunction,
}

#[derive(Clone, Debug)]
pub struct AdtVar {
    pub idx: AdtVarIdx,
    pub name: Symbol,
    pub pred: PredicateConjunction,
}

#[derive(Clone, Copy)]
pub struct ConstVar<'pcx> {
    pub idx: ConstVarIdx,
    pub name: Symbol,
    pub ty: Ty<'pcx>,
}

impl<'pcx> ConstVar<'pcx> {
    #[expect(unused_variables, reason = "predicates on const variables are not handle yet")] //FIXME
    pub(crate) fn from<'mcx>(
        pcx: PatCtxt<'pcx>,
        fn_sym_tab: &impl GetType<'mcx>,
        idx: usize,
        ty: WithPath<'mcx, &'mcx pairs::Type<'mcx>>,
        pred: PredicateConjunction,
    ) -> Self {
        let name = Symbol::intern(ty.span.as_str());
        let ty = Ty::from(ty, pcx, fn_sym_tab);
        Self {
            idx: idx.into(),
            name,
            ty,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PlaceVar<'pcx> {
    pub idx: PlaceVarIdx,
    pub name: Symbol,
    pub ty: Ty<'pcx>,
}

impl<'pcx> PlaceVar<'pcx> {
    pub fn new(idx: PlaceVarIdx, name: Symbol, ty: Ty<'pcx>) -> Self {
        Self { idx, name, ty }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LocalVar<'pcx> {
    pub idx: Local,
    pub name: Symbol,
    pub ty: Ty<'pcx>,
}

impl<'pcx> LocalVar<'pcx> {
    pub fn new(idx: Local, name: Symbol, ty: Ty<'pcx>) -> Self {
        Self { idx, name, ty }
    }
}

#[derive(Default, Debug)]
pub struct NonLocalMetaVars<'pcx> {
    pub ty_var_names: IndexVec<TyVarIdx, Symbol>,
    pub const_var_names: IndexVec<ConstVarIdx, Symbol>,
    pub place_var_names: IndexVec<PlaceVarIdx, Symbol>,
    pub local_names: IndexVec<Local, Symbol>,
    pub ty_vars: IndexVec<TyVarIdx, TyVar>,
    pub adt_vars: IndexVec<AdtVarIdx, AdtVar>,
    pub const_vars: IndexVec<ConstVarIdx, ConstVar<'pcx>>,
    pub place_vars: IndexVec<PlaceVarIdx, PlaceVar<'pcx>>,
    pub locals: IndexVec<Local, LocalVar<'pcx>>,
}

impl<'pcx> NonLocalMetaVars<'pcx> {
    pub fn add_ty_var(&mut self, name: Symbol, preds: Option<PredicateConjunction>) {
        let idx = self.ty_vars.next_index();
        let pred = preds.unwrap_or_default();
        let ty_var = TyVar { idx, name, pred };
        self.ty_vars.push(ty_var);
    }
    pub fn add_adt_var(&mut self, name: Symbol, preds: Option<PredicateConjunction>) {
        let idx = self.adt_vars.next_index();
        let pred = preds.unwrap_or_default();
        self.adt_vars.push(AdtVar { idx, name, pred });
    }
    pub fn add_const_var(&mut self, name: Symbol, ty: Ty<'pcx>) {
        let idx = self.const_vars.next_index();
        let const_var = ConstVar { idx, name, ty };
        self.const_vars.push(const_var);
    }
    pub fn add_place_var(&mut self, name: Symbol, ty: Ty<'pcx>) {
        let idx = self.place_vars.next_index();
        let place_var = PlaceVar { idx, name, ty };
        self.place_vars.push(place_var);
    }

    pub fn from_meta_decls<'mcx>(
        meta_decls: Option<WithPath<'mcx, &'mcx pairs::MetaVariableDeclList<'mcx>>>,
        pcx: PatCtxt<'pcx>,
        fn_sym_tab: &'mcx impl GetType<'mcx>,
    ) -> Self {
        let mut meta = Self::default();
        if let Some(decls) = meta_decls
            && let p = decls.path
            && let Some(decls) = decls.get_matched().1
        {
            let decls = collect_elems_separated_by_comma!(decls).collect::<Vec<_>>();
            // handle the type meta variable first
            let mut type_vars = Vec::new();
            let mut adt_vars = Vec::new();
            let mut konst_vars = Vec::new();
            let mut place_vars = Vec::new();
            for decl in decls {
                let (ident, _, ty, preds) = decl.get_matched();
                let ident = Symbol::intern(ident.span.as_str());
                let preds = preds
                    .as_ref()
                    .map(|preds| preds.get_matched().1)
                    .map(|pred| PredicateConjunction::from_pairs(pred, p))
                    .transpose()
                    .expect("invalid predicates in meta variable decls");
                match ty.deref() {
                    Choice4::_0(_ty) => type_vars.push((ident, preds)),
                    Choice4::_1(_adt) => adt_vars.push((ident, preds)),
                    Choice4::_2(konst) => konst_vars.push((ident, konst)),
                    Choice4::_3(place) => place_vars.push((ident, place)),
                }
            }
            for (ident, pred_opt) in type_vars {
                meta.add_ty_var(ident, pred_opt);
            }
            for (ident, pred_opt) in adt_vars {
                meta.add_adt_var(ident, pred_opt);
            }
            for (ident, konst) in konst_vars {
                let ty = Ty::from(with_path(p, konst.get_matched().2), pcx, fn_sym_tab);
                // FIXME: konst vars' predicates
                meta.add_const_var(ident, ty);
            }
            for (ident, place) in place_vars {
                let ty = Ty::from(with_path(p, place.get_matched().2), pcx, fn_sym_tab);
                // FIXME: place vars' predicates
                meta.add_place_var(ident, ty);
            }
        }
        meta
    }

    pub(crate) fn table_head(&self, head: &mut TableHead) {
        for name in self.ty_var_names.iter() {
            head.insert(*name, ColumnType::Ty);
        }
        for name in self.const_var_names.iter() {
            head.insert(*name, ColumnType::Const);
        }
        for name in self.place_var_names.iter() {
            head.insert(*name, ColumnType::Place);
        }
    }
}
