use rpl_context::{pat, PatCtxt};
use rustc_abi::{FieldIdx, VariantIdx};
use rustc_data_structures::fx::FxHashMap;
use rustc_index::IndexSlice;
use rustc_middle::ty::{self, TyCtxt};
use rustc_span::Symbol;

use crate::MatchTyCtxt;

pub struct MatchAdtCtxt<'pcx, 'tcx> {
    tcx: TyCtxt<'tcx>,
    pcx: PatCtxt<'pcx>,
    meta: &'pcx pat::MetaVars<'pcx>,
}

impl<'pcx, 'tcx> MatchAdtCtxt<'pcx, 'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>, pcx: PatCtxt<'pcx>, meta: &'pcx pat::MetaVars<'pcx>) -> Self {
        Self { tcx, pcx, meta }
    }

    pub fn match_adt(&self, adt_pat: &pat::Adt<'pcx>, adt: ty::AdtDef<'tcx>) -> Option<AdtMatch<'tcx>> {
        let kind = match (&adt_pat.kind, adt.adt_kind()) {
            (pat::AdtKind::Struct(variant_pat), ty::AdtKind::Struct) => {
                AdtMatchKind::Struct(self.match_fields(&variant_pat.fields, &adt.non_enum_variant().fields)?)
            },
            (pat::AdtKind::Enum(variants_pat), ty::AdtKind::Enum) => AdtMatchKind::Enum(
                variants_pat
                    .iter()
                    .map(|(&name, variant_pat)| {
                        let variant_match = adt
                            .variants()
                            .iter_enumerated()
                            .find_map(|(variant_idx, variant)| self.match_variant(variant_pat, variant, variant_idx))?;
                        Some((name, variant_match))
                    })
                    .collect::<Option<_>>()?,
            ),
            (
                pat::AdtKind::Struct(_) | pat::AdtKind::Enum(_),
                ty::AdtKind::Struct | ty::AdtKind::Enum | ty::AdtKind::Union,
            ) => return None,
        };
        Some(AdtMatch { adt, kind })
    }

    fn match_variant(
        &self,
        variant_pat: &pat::Variant<'pcx>,
        variant: &'tcx ty::VariantDef,
        variant_idx: VariantIdx,
    ) -> Option<VariantMatch<'tcx>> {
        self.match_fields(&variant_pat.fields, &variant.fields)
            .map(|fields| VariantMatch {
                variant_idx,
                variant,
                fields,
            })
    }

    fn match_fields(
        &self,
        fields_pat: &FxHashMap<Symbol, pat::Field<'pcx>>,
        fields: &'tcx IndexSlice<FieldIdx, ty::FieldDef>,
    ) -> Option<FxHashMap<Symbol, FieldMatch<'tcx>>> {
        fields_pat
            .iter()
            .map(|(&name, field_pat)| {
                let field_match = fields
                    .iter_enumerated()
                    .find_map(|(field_idx, field)| self.match_field(field_pat, field, field_idx))?;
                Some((name, field_match))
            })
            .collect()
    }

    fn match_field(
        &self,
        field_pat: &pat::Field<'pcx>,
        field: &'tcx ty::FieldDef,
        field_idx: FieldIdx,
    ) -> Option<FieldMatch<'tcx>> {
        let pat_ty = field_pat.ty;
        let ty = self.tcx.type_of(field.did).instantiate_identity();
        MatchTyCtxt::new(self.tcx, self.pcx, ty::ParamEnv::reveal_all(), self.meta)
            .match_ty(pat_ty, ty)
            .then_some(FieldMatch { field_idx, field, ty })
    }
}

pub struct AdtMatch<'tcx> {
    pub adt: ty::AdtDef<'tcx>,
    kind: AdtMatchKind<'tcx>,
}

enum AdtMatchKind<'tcx> {
    Struct(FxHashMap<Symbol, FieldMatch<'tcx>>),
    Enum(FxHashMap<Symbol, VariantMatch<'tcx>>),
}

impl<'tcx> AdtMatch<'tcx> {
    pub fn expect_struct(&self) -> &FxHashMap<Symbol, FieldMatch<'tcx>> {
        match &self.kind {
            AdtMatchKind::Struct(variant_match) => variant_match,
            AdtMatchKind::Enum(_) => panic!("expected struct, got enum"),
        }
    }
    pub fn expect_enum(&self) -> &FxHashMap<Symbol, VariantMatch<'tcx>> {
        match &self.kind {
            AdtMatchKind::Enum(variants) => variants,
            AdtMatchKind::Struct(_) => panic!("expected enum, got struct"),
        }
    }
}

pub struct VariantMatch<'tcx> {
    pub variant_idx: VariantIdx,
    pub variant: &'tcx ty::VariantDef,
    pub fields: FxHashMap<Symbol, FieldMatch<'tcx>>,
}

pub struct FieldMatch<'tcx> {
    pub field_idx: FieldIdx,
    pub field: &'tcx ty::FieldDef,
    pub ty: ty::Ty<'tcx>,
}
