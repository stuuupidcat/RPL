use rustc_data_structures::fx::FxHashMap;
use rustc_middle::mir;
use rustc_span::Symbol;

use super::{MirPattern, Path, Ty};

pub struct AdtDef<'pcx> {
    kind: AdtKind<'pcx>,
}

pub enum AdtKind<'pcx> {
    Struct(VariantDef<'pcx>),
    Enum(FxHashMap<Symbol, VariantDef<'pcx>>),
}

#[derive(Default)]
pub struct VariantDef<'pcx> {
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

#[derive(Default)]
pub struct FnsDef<'pcx> {
    fns: FxHashMap<Symbol, FnDef<'pcx>>,
    fn_pats: FxHashMap<Symbol, FnDef<'pcx>>,
    unnamed_fns: Vec<FnDef<'pcx>>,
}

pub struct FnDef<'pcx> {
    pub params: ParamsDef<'pcx>,
    pub ret: Ty<'pcx>,
    pub body: Option<FnBody<'pcx>>,
}

#[derive(Default)]
pub struct ParamsDef<'pcx> {
    params: Vec<ParamDef<'pcx>>,
    non_exhaustive: bool,
}

pub struct ParamDef<'pcx> {
    pub mutability: mir::Mutability,
    pub ident: Symbol,
    pub ty: Ty<'pcx>,
}

pub struct TraitDef {}

#[derive(Clone, Copy)]
pub enum FnBody<'pcx> {
    Mir(&'pcx MirPattern<'pcx>),
}

impl<'pcx> AdtDef<'pcx> {
    pub(crate) fn new_struct() -> Self {
        Self {
            kind: AdtKind::Struct(Default::default()),
        }
    }
    pub(crate) fn new_enum() -> Self {
        Self {
            kind: AdtKind::Enum(Default::default()),
        }
    }
    pub(crate) fn non_enum_variant_mut(&mut self) -> &mut VariantDef<'pcx> {
        match &mut self.kind {
            AdtKind::Struct(variant) => variant,
            AdtKind::Enum(_) => panic!("cannot mutate non-enum variant of enum"),
        }
    }
    pub fn add_variant(&mut self, name: Symbol) -> &mut VariantDef<'pcx> {
        match &mut self.kind {
            AdtKind::Struct(_) => panic!("cannot add variant to struct"),
            AdtKind::Enum(variants) => variants.entry(name).or_insert_with(VariantDef::default),
        }
    }
    pub fn non_enum_variant(&self) -> &VariantDef<'pcx> {
        match &self.kind {
            AdtKind::Struct(variant) => variant,
            AdtKind::Enum(_) => panic!("cannot access non-enum variant of enum"),
        }
    }
}

impl<'pcx> VariantDef<'pcx> {
    pub fn add_field(&mut self, name: Symbol, ty: Ty<'pcx>) {
        self.fields.insert(name, FieldDef { ty });
    }
}

impl<'pcx> FnsDef<'pcx> {
    pub fn get_fn_pat(&self, name: Symbol) -> Option<&FnDef<'pcx>> {
        self.fn_pats.get(&name)
    }
    // FIXME: remove this when all kinds of patterns are implemented
    pub fn get_fn_pat_mir_body(&self, name: Symbol) -> Option<&MirPattern<'pcx>> {
        let FnBody::Mir(mir_body) = self.fn_pats.get(&name)?.body?;
        Some(mir_body)
    }
    pub fn new_fn(&mut self, name: Symbol, ret: Ty<'pcx>) -> &mut FnDef<'pcx> {
        self.fns.entry(name).or_insert_with(|| FnDef::new(ret))
    }
    pub fn new_fn_pat(&mut self, name: Symbol, ret: Ty<'pcx>) -> &mut FnDef<'pcx> {
        self.fn_pats.entry(name).or_insert_with(|| FnDef::new(ret))
    }
    pub fn new_unnamed(&mut self, ret: Ty<'pcx>) -> &mut FnDef<'pcx> {
        self.unnamed_fns.push(FnDef::new(ret));
        self.unnamed_fns.last_mut().unwrap()
    }
}

impl<'pcx> FnDef<'pcx> {
    pub(crate) fn new(ret: Ty<'pcx>) -> Self {
        Self {
            params: ParamsDef::default(),
            ret,
            body: None,
        }
    }
    pub fn set_body(&mut self, body: FnBody<'pcx>) {
        self.body = Some(body);
    }
}

impl<'pcx> ParamsDef<'pcx> {
    pub fn add_param(&mut self, ident: Symbol, mutability: mir::Mutability, ty: Ty<'pcx>) {
        self.params.push(ParamDef { mutability, ident, ty });
    }
    pub fn set_non_exhaustive(&mut self) {
        self.non_exhaustive = true;
    }
}
