use rustc_span::Symbol;

use super::{PatId, Ty};

pub struct FnDef {}

pub struct AdtDef<'tcx> {
    pid: PatId,
    variants: Vec<VariantDef<'tcx>>,
}

pub struct VariantDef<'tcx> {
    fields: Vec<FieldDef<'tcx>>,
}

pub struct FieldDef<'tcx> {
    name: Symbol,
    ty: Ty<'tcx>,
}

pub struct Impl {
    pid: PatId,
    adt: PatId,
    trait_id: Option<PatId>,
}

pub struct Trait {
    pid: PatId,
}
