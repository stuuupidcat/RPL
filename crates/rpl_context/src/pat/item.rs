use rustc_data_structures::fx::{FxHashMap, FxIndexMap};
use rustc_middle::mir;
use rustc_span::Symbol;

use super::{Path, Ty};

pub struct AdtDef<'pcx> {
    #[expect(dead_code)]
    kind: AdtKind<'pcx>,
}

pub enum AdtKind<'pcx> {
    Struct(VariantDef<'pcx>),
    Enum(FxHashMap<Symbol, VariantDef<'pcx>>),
}

pub struct VariantDef<'pcx> {
    #[expect(dead_code)]
    fields: FxHashMap<Symbol, FieldDef<'pcx>>,
}

pub struct FieldDef<'pcx> {
    #[expect(dead_code)]
    ty: Ty<'pcx>,
}

pub struct ImplDef<'pcx> {
    #[expect(dead_code)]
    ty: Ty<'pcx>,
    #[expect(dead_code)]
    trait_id: Option<Path<'pcx>>,
    #[expect(dead_code)]
    fns: FxHashMap<Symbol, FnDef<'pcx>>,
}

pub struct FnDef<'pcx> {
    #[expect(dead_code)]
    params: FxIndexMap<Symbol, ParamDef<'pcx>>,
    #[expect(dead_code)]
    ret: Ty<'pcx>,
    #[expect(dead_code)]
    body: Option<FnBody<'pcx>>,
}

pub struct ParamDef<'pcx> {
    #[expect(dead_code)]
    mutability: mir::Mutability,
    #[expect(dead_code)]
    ty: Ty<'pcx>,
}

pub struct TraitDef {}

pub enum FnBody<'pcx> {
    Hir(),
    Mir(&'pcx mir::Body<'pcx>),
}
