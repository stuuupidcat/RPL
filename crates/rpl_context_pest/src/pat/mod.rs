use rpl_meta_pest::collect_elems_separated_by_comma;
use rpl_meta_pest::utils::Ident;
use rpl_parser::generics::{Choice14, Choice2, Choice3, Choice4};
use rpl_parser::pairs;
use rustc_ast_ir::Mutability;
use rustc_data_structures::fx::FxHashMap;
use rustc_hir::Safety;
use rustc_index::IndexVec;
use rustc_middle::middle::region;
use rustc_span::Symbol;
use std::ops::Deref;

use crate::PatCtxt;

mod item;
mod mir;
mod pretty;
mod ty;
mod utils;

pub use item::*;
pub use mir::*;
pub use ty::*;

#[derive(Default)]
pub struct MetaVars<'pcx> {
    pub ty_vars: IndexVec<TyVarIdx, TyVar<'pcx>>,
    pub const_vars: IndexVec<ConstVarIdx, ConstVar<'pcx>>,
}

impl<'pcx> MetaVars<'pcx> {
    pub fn add_ty_var(&mut self, name: Ident<'pcx>, pred: Option<TyPred>) {
        let idx = self.ty_vars.next_index();
        let ty_var = TyVar { idx, name, pred };
        self.ty_vars.push(ty_var);
    }
    pub fn add_const_var(&mut self, name: Ident<'pcx>, ty: Ty<'pcx>) {
        let idx = self.const_vars.next_index();
        let const_var = ConstVar { idx, name, ty };
        self.const_vars.push(const_var);
    }
}

pub struct Pattern<'pcx> {
    // FIXME: remove it
    pub pcx: PatCtxt<'pcx>,
    pub meta: MetaVars<'pcx>,
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
    pub fn from_parsed(pcx: PatCtxt<'pcx>, pat_item: &'pcx pairs::pattBlockItem<'_>) -> Self {
        let mut pattern = Self::new(pcx);
        let (_, meta_decls, _, _, item_or_patt_op, _) = pat_item.get_matched();
        let meta = if let Some(meta_decls) = meta_decls {
            pattern.add_meta(meta_decls);
        };
        pattern.add_item_or_patt_op(item_or_patt_op);
        pattern
    }
    fn add_meta(&mut self, meta_decls: &'pcx pairs::MetaVariableDeclList<'_>) {
        if let Some(decls) = meta_decls.get_matched().1 {
            let decls = collect_elems_separated_by_comma!(decls).collect::<Vec<_>>();
            for decl in decls {
                let (ident, _, ty) = decl.get_matched();
                match ty.span.as_str() {
                    "Ty" => self.meta.add_ty_var(ident.into(), None),
                    // FIXME: add more types
                    // Place, Const ..
                    _ => panic!("unsupported meta type: {:?}", ty.span.as_str()),
                }
            }
        }
    }
    fn add_item_or_patt_op(&mut self, item_or_patt_op: &'pcx pairs::RustItemOrPatternOperation<'_>) {
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
                    self.add_item(item);
                }
            },
        }
    }
    fn add_item(&mut self, item: &'pcx pairs::RustItem<'_>) {
        match &**item {
            Choice4::_0(rust_fn) => self.add_fn(rust_fn),
            Choice4::_1(rust_struct) => self.add_struct(rust_struct),
            Choice4::_2(rust_enum) => self.add_enum(rust_enum),
            Choice4::_3(_rust_impl) => todo!("check impl in meta pass"),
        }
    }
}

// fn-related methods
impl<'pcx> Pattern<'pcx> {
    fn add_fn(&mut self, rust_fn: &'pcx pairs::Fn<'_>) {
        let fn_pat = Fn::from(rust_fn, self.pcx);
        self.fns.fn_pats.insert(fn_pat.name, fn_pat);
    }
}

// struct-related methods
impl<'pcx> Pattern<'pcx> {
    fn add_struct(&mut self, rust_struct: &pairs::Struct<'_>) {
        todo!()
    }
}

// enum-related methods
impl<'pcx> Pattern<'pcx> {
    fn add_enum(&mut self, rust_enum: &pairs::Enum<'_>) {
        todo!()
    }
}
