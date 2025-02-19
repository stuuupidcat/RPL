use rpl_meta_pest::collect_elems_separated_by_comma;
use rpl_parser::generics::{Choice3, Choice4};
use rpl_parser::pairs;
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
    // FIXME: remove it
    pub pcx: PatCtxt<'pcx>,
    pub adts: FxHashMap<Symbol, Adt<'pcx>>,
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
    pub fn from_parsed(pcx: PatCtxt<'pcx>, pat_item: &pairs::pattBlockItem<'_>) -> Self {
        let mut pattern = Self::new(pcx);
        let (_, meta_decls, _, _, item_or_patt_op, _) = pat_item.get_matched();
        if let Some(meta_decls) = meta_decls {
            pattern.add_meta_decls(meta_decls);
        }
        pattern.add_item_or_patt_op(item_or_patt_op);
        pattern
    }
    fn add_meta_decls(&mut self, meta_decls: &pairs::MetaVariableDeclList<'_>) {
        if let Some(decls) = meta_decls.get_matched().1 {
            let decls = collect_elems_separated_by_comma!(decls).collect::<Vec<_>>();
            for decl in decls {
                let (ident, _, ty) = decl.get_matched();
                todo!()
            }
        }
    }
    fn add_item_or_patt_op(&mut self, item_or_patt_op: &pairs::RustItemOrPatternOperation<'_>) {
        match &**item_or_patt_op {
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
    fn add_item(&mut self, item: &pairs::RustItem<'_>) {
        match &**item {
            Choice4::_0(rust_fn) => self.add_fn(rust_fn),
            Choice4::_1(rust_struct) => self.add_struct(rust_struct),
            Choice4::_2(rust_enum) => self.add_enum(rust_enum),
            Choice4::_3(_rust_impl) => todo!("check impl in meta pass"),
        }
    }
    fn add_fn(&mut self, rust_fn: &pairs::Fn<'_>) {
        todo!()
    }
    fn add_struct(&mut self, rust_struct: &pairs::Struct<'_>) {
        todo!()
    }
    fn add_enum(&mut self, rust_enum: &pairs::Enum<'_>) {
        todo!()
    }
}
