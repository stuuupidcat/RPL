use rpl_meta::collect_elems_separated_by_comma;
use rpl_meta::symbol_table::NonLocalMetaSymTab;
use rpl_meta::utils::Ident;
use rpl_parser::generics::{Choice2, Choice3, Choice4};
use rpl_parser::pairs;
use rustc_data_structures::fx::FxHashMap;
use rustc_index::IndexVec;
use rustc_span::Symbol;
use std::ops::Deref;
use std::sync::Arc;

use crate::PatCtxt;

mod item;
mod mir;
mod pretty;
mod ty;
mod utils;

pub use item::*;
pub use mir::*;
pub use ty::*;

#[derive(Default, Debug)]
pub struct NonLocalMetaVars<'pcx> {
    pub ty_vars: IndexVec<TyVarIdx, TyVar>,
    pub const_vars: IndexVec<ConstVarIdx, ConstVar<'pcx>>,
    pub place_vars: IndexVec<PlaceVarIdx, PlaceVar<'pcx>>,
}

impl<'pcx> NonLocalMetaVars<'pcx> {
    pub fn add_ty_var(&mut self, name: Symbol, pred: Option<TyPred>) {
        let idx = self.ty_vars.next_index();
        let ty_var = TyVar { idx, name, pred };
        self.ty_vars.push(ty_var);
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

    pub fn from_meta_decls(
        meta_decls: Option<&pairs::MetaVariableDeclList<'_>>,
        pcx: PatCtxt<'pcx>,
        sym_tab: Arc<NonLocalMetaSymTab>,
    ) -> Self {
        let mut meta = Self::default();
        if let Some(decls) = meta_decls
            && let Some(decls) = decls.get_matched().1
        {
            let decls = collect_elems_separated_by_comma!(decls).collect::<Vec<_>>();
            // handle the type meta variable first
            let mut type_vars = Vec::new();
            let mut konst_vars = Vec::new();
            let mut place_vars = Vec::new();
            for decl in decls {
                let (ident, _, ty) = decl.get_matched();
                let ident = Symbol::intern(ident.span.as_str());
                match ty.deref() {
                    Choice3::_0(_ty) => type_vars.push(ident),
                    Choice3::_1(konst) => konst_vars.push((ident, konst)),
                    Choice3::_2(place) => place_vars.push((ident, place)),
                }
            }
            for ident in type_vars {
                meta.add_ty_var(ident, None);
            }
            for (ident, konst) in konst_vars {
                let ty = Ty::from(konst.get_matched().2, pcx, sym_tab.clone());
                meta.add_const_var(ident, ty);
            }
            for (ident, place) in place_vars {
                let ty = Ty::from(place.get_matched().2, pcx, sym_tab.clone());
                meta.add_place_var(ident, ty);
            }
        }
        meta
    }
}

pub struct Pattern<'pcx> {
    // FIXME: remove it
    pub pcx: PatCtxt<'pcx>,
    pub meta: Arc<NonLocalMetaVars<'pcx>>,
    pub adts: FxHashMap<Symbol, Adt<'pcx>>,
    pub fns: Fns<'pcx>,
    #[expect(dead_code)]
    impls: Vec<Impl<'pcx>>,
}

impl<'pcx> Pattern<'pcx> {
    pub(crate) fn new(pcx: PatCtxt<'pcx>) -> Self {
        Self {
            pcx,
            meta: Default::default(),
            adts: Default::default(),
            fns: Default::default(),
            impls: Default::default(),
        }
    }

    // FIXME: remove it when pest parser is ready
    pub fn new_struct(&mut self, name: Symbol) -> &mut Adt<'pcx> {
        self.adts.entry(name).or_insert_with(Adt::new_struct)
        // .non_enum_variant_mut()
    }
    // FIXME: remove it when pest parser is ready
    pub fn new_enum(&mut self, name: Symbol) -> &mut Adt<'pcx> {
        self.adts.entry(name).or_insert_with(Adt::new_enum)
    }

    pub fn get_adt(&self, name: Symbol) -> Option<&Adt<'pcx>> {
        self.adts.get(&name)
    }
}

impl<'pcx> Pattern<'pcx> {
    pub fn from_parsed(
        pcx: PatCtxt<'pcx>,
        pat_item: &pairs::pattBlockItem<'pcx>,
        symbol_table: &'pcx rpl_meta::symbol_table::SymbolTable<'_>,
    ) -> Self {
        let mut pattern = Self::new(pcx);
        let (_, meta_decls, _, _, item_or_patt_op, _) = pat_item.get_matched();
        pattern.meta = Arc::new(NonLocalMetaVars::from_meta_decls(
            meta_decls.as_ref(),
            pcx,
            symbol_table.meta_vars.clone(),
        ));
        pattern.add_item_or_patt_op(item_or_patt_op, symbol_table);
        pattern
    }

    fn add_item_or_patt_op(
        &mut self,
        item_or_patt_op: &pairs::RustItemOrPatternOperation<'pcx>,
        symbol_table: &'pcx rpl_meta::symbol_table::SymbolTable<'_>,
    ) {
        match item_or_patt_op.deref() {
            Choice3::_2(_patt_op) => {
                // FIXME: process the patt operation
                todo!()
            },
            _ => {
                let item = item_or_patt_op.RustItem();
                let items = item_or_patt_op.RustItems();
                let items = if let Some(items) = items {
                    items.get_matched().1.iter_matched().collect::<Vec<_>>()
                } else {
                    // unwrap here is safe because the `RustItem` or `RustItems` is not `None`
                    vec![item.unwrap()]
                };
                for item in items {
                    self.add_item(item, self.meta.clone(), symbol_table);
                }
            },
        }
    }
    fn add_item(
        &mut self,
        item: &pairs::RustItem<'pcx>,
        meta: Arc<NonLocalMetaVars<'pcx>>,
        symbol_table: &'pcx rpl_meta::symbol_table::SymbolTable<'_>,
    ) {
        match &**item {
            Choice4::_0(rust_fn) => {
                let fn_name = Symbol::intern(rust_fn.FnSig().FnName().span.as_str());
                let fn_symbol_table = symbol_table.get_fn(fn_name).unwrap();
                self.add_fn(rust_fn, meta, fn_symbol_table);
            },
            Choice4::_1(rust_struct) => self.add_struct(rust_struct),
            Choice4::_2(rust_enum) => self.add_enum(rust_enum),
            Choice4::_3(_rust_impl) => todo!("check impl in meta pass"),
        }
    }
}

// fn-related methods
impl<'pcx> Pattern<'pcx> {
    fn add_fn(
        &mut self,
        rust_fn: &pairs::Fn<'pcx>,
        meta: Arc<NonLocalMetaVars<'pcx>>,
        fn_symbol_table: &FnSymbolTable<'pcx>,
    ) {
        let fn_pat = Fn::from(rust_fn, self.pcx, fn_symbol_table, meta);
        self.fns.unnamed_fns.push(fn_pat);
    }
}

// struct-related methods
impl Pattern<'_> {
    fn add_struct(&mut self, _rust_struct: &pairs::Struct<'_>) {
        todo!()
    }
}

// enum-related methods
impl Pattern<'_> {
    fn add_enum(&mut self, _rust_enum: &pairs::Enum<'_>) {
        todo!()
    }
}
