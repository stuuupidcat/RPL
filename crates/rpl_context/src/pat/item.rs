use rustc_span::Symbol;

use super::{PatId, Ty};

pub struct FnDef {}

pub struct AdtDef<'pcx> {
    pid: PatId,
    variants: Vec<VariantDef<'pcx>>,
}

pub struct VariantDef<'pcx> {
    fields: Vec<FieldDef<'pcx>>,
}

pub struct FieldDef<'pcx> {
    name: Symbol,
    ty: Ty<'pcx>,
}

pub struct Impl {
    pid: PatId,
    adt: PatId,
    trait_id: Option<PatId>,
}

pub struct Trait {
    pid: PatId,
}
