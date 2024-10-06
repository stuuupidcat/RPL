use rustc_middle::ty::TyCtxt;

use super::*;

impl<'tcx> Patterns<'tcx> {
    pub fn match_local(&self, tcx: TyCtxt<'tcx>, body: &mir::Body<'tcx>, pat: LocalIdx, local: mir::Local) -> bool {
        self.match_ty(tcx, self.locals[pat], body.local_decls[local].ty)
    }
    pub fn match_place_ref(
        &self,
        tcx: TyCtxt<'tcx>,
        body: &mir::Body<'tcx>,
        pat: Place<'tcx>,
        place: mir::PlaceRef<'tcx>,
    ) -> bool {
        use self::Field::{Named, Unnamed};
        use mir::tcx::PlaceTy;
        use mir::ProjectionElem::*;
        let place_proj_and_ty = place
            .projection
            .iter()
            .scan(PlaceTy::from_ty(body.local_decls[place.local].ty), |place_ty, &proj| {
                Some((proj, std::mem::replace(place_ty, place_ty.projection_ty(tcx, proj))))
            });
        self.match_local(tcx, body, pat.local, place.local)
            && pat.projection.len() == place.projection.len()
            && std::iter::zip(pat.projection, place_proj_and_ty).all(|(&proj_pat, (proj, place_ty))| {
                match (place_ty.ty.kind(), proj_pat, proj) {
                    (_, PlaceElem::Deref, Deref) => true,
                    (ty::Adt(adt, _), PlaceElem::Field(field), Field(idx, _)) => {
                        let variant = match place_ty.variant_index {
                            None => adt.non_enum_variant(),
                            Some(idx) => adt.variant(idx),
                        };
                        match (variant.ctor, field) {
                            (None, Named(name)) => variant.ctor.is_none() && variant.fields[idx].name == name,
                            (Some((CtorKind::Fn, _)), Unnamed(idx_pat)) => idx_pat == idx,
                            _ => false,
                        }
                    },
                    (_, PlaceElem::Index(local_pat), Index(local)) => self.match_local(tcx, body, local_pat, local),
                    (
                        _,
                        PlaceElem::ConstantIndex {
                            offset: offset_pat,
                            from_end: from_end_pat,
                            min_length: min_length_pat,
                        },
                        ConstantIndex {
                            offset,
                            from_end,
                            min_length,
                        },
                    ) => (offset_pat, from_end_pat, min_length_pat) == (offset, from_end, min_length),
                    (
                        _,
                        PlaceElem::Subslice {
                            from: from_pat,
                            to: to_pat,
                            from_end: from_end_pat,
                        },
                        Subslice { from, to, from_end },
                    ) => (from_pat, to_pat, from_end_pat) == (from, to, from_end),
                    (ty::Adt(adt, _), PlaceElem::Downcast(sym), Downcast(_, idx)) => {
                        adt.is_enum() && adt.variant(idx).name == sym
                    },
                    (_, PlaceElem::OpaqueCast(ty_pat), OpaqueCast(ty))
                    | (_, PlaceElem::Subtype(ty_pat), Subtype(ty)) => self.match_ty(tcx, ty_pat, ty),
                    _ => false,
                }
            })
    }

    pub fn match_ty(&self, tcx: TyCtxt<'tcx>, ty_pat: Ty<'tcx>, ty: ty::Ty<'tcx>) -> bool {
        let matched = match (*ty_pat.kind(), *ty.kind()) {
            (TyKind::TyVar(_), _) => {
                // self.add_ty_match(ty_var, ty);
                true
            },
            (TyKind::Array(ty_pat, konst_pat), ty::Array(ty, konst)) => {
                self.match_ty(tcx, ty_pat, ty) && self.match_const(tcx, konst_pat, konst)
            },
            (TyKind::Slice(ty_pat), ty::Slice(ty)) => self.match_ty(tcx, ty_pat, ty),
            (TyKind::Tuple(tys_pat), ty::Tuple(tys)) => {
                tys_pat.len() == tys.len() && zip(tys_pat, tys).all(|(&ty_pat, ty)| self.match_ty(tcx, ty_pat, ty))
            },
            (TyKind::Ref(region_pat, pat_ty, pat_mutblty), ty::Ref(region, ty, mutblty)) => {
                self.match_region(region_pat, region) && pat_mutblty == mutblty && self.match_ty(tcx, pat_ty, ty)
            },
            (TyKind::RawPtr(ty_pat, mutability_pat), ty::RawPtr(ty, mutblty)) => {
                mutability_pat == mutblty && self.match_ty(tcx, ty_pat, ty)
            },
            (TyKind::Uint(ty_pat), ty::Uint(ty)) => ty_pat == ty,
            (TyKind::Int(ty_pat), ty::Int(ty)) => ty_pat == ty,
            (TyKind::Float(ty_pat), ty::Float(ty)) => ty_pat == ty,
            (TyKind::Adt(path, args_pat), ty::Adt(adt, args)) => {
                matches!(self.match_item_path(tcx, path, adt.did()), Some([]))
                    && self.match_generic_args(tcx, args_pat, args)
            },
            (TyKind::FnDef(path, args_pat), ty::FnDef(def_id, args)) => {
                self.match_path(tcx, path, def_id) && self.match_generic_args(tcx, args_pat, args)
            },
            (TyKind::Alias(alias_kind_pat, path, args), ty::Alias(alias_kind, alias)) => {
                alias_kind_pat == alias_kind
                    && self.match_path(tcx, path, alias.def_id)
                    && self.match_generic_args(tcx, args, alias.args)
            },
            _ => false,
        };
        debug!(?ty_pat, ?ty, matched, "match_ty");
        matched
    }

    pub fn match_path(&self, tcx: TyCtxt<'tcx>, path: Path<'tcx>, def_id: DefId) -> bool {
        let matched = match path {
            Path::Item(path) => matches!(self.match_item_path(tcx, path, def_id), Some([])),
            Path::TypeRelative(ty, name) => {
                tcx.item_name(def_id) == name
                    && tcx
                        .opt_parent(def_id)
                        .is_some_and(|did| self.match_ty(tcx, ty, tcx.type_of(did).instantiate_identity()))
            },
            Path::LangItem(lang_item) => tcx.is_lang_item(def_id, lang_item),
        };
        debug!(?path, ?def_id, matched, "match_path");
        matched
    }

    pub fn match_item_path(&self, tcx: TyCtxt<'tcx>, path: ItemPath<'tcx>, def_id: DefId) -> Option<&[Symbol]> {
        let &[krate, ref in_crate @ ..] = path.0 else {
            return None;
        };
        let def_path = tcx.def_path(def_id);
        let matched = match def_path.krate {
            LOCAL_CRATE => krate == kw::Crate,
            _ => tcx.crate_name(def_path.krate) == krate,
        };
        let mut pat_iter = in_crate.iter();
        use DefPathData::{Impl, TypeNs, ValueNs};
        let mut iter = def_path
            .data
            .iter()
            .filter(|data| matches!(data.data, Impl | TypeNs(_) | ValueNs(_)));
        let matched = matched
            && std::iter::zip(pat_iter.by_ref(), iter.by_ref())
                .all(|(&path, data)| data.data.get_opt_name().is_some_and(|name| name == path));
        let matched = matched && iter.next().is_none();
        debug!(?path, ?def_id, matched, "match_item_path");
        matched.then_some(pat_iter.as_slice())
    }

    pub fn match_generic_args(
        &self,
        tcx: TyCtxt<'tcx>,
        args_pat: GenericArgsRef<'tcx>,
        args: ty::GenericArgsRef<'tcx>,
    ) -> bool {
        args_pat.len() == args.len()
            && zip(&*args_pat, args).all(|(&arg_pat, arg)| self.match_generic_arg(tcx, arg_pat, arg))
    }

    pub fn match_generic_arg(
        &self,
        tcx: TyCtxt<'tcx>,
        arg_pat: GenericArgKind<'tcx>,
        arg: ty::GenericArg<'tcx>,
    ) -> bool {
        match (arg_pat, arg.unpack()) {
            (GenericArgKind::Lifetime(region_pat), ty::GenericArgKind::Lifetime(region)) => {
                self.match_region(region_pat, region)
            },
            (GenericArgKind::Type(ty_pat), ty::GenericArgKind::Type(ty)) => self.match_ty(tcx, ty_pat, ty),
            (GenericArgKind::Const(konst_pat), ty::GenericArgKind::Const(konst)) => {
                self.match_const(tcx, konst_pat, konst)
            },
            _ => false,
        }
    }

    pub fn match_const(&self, tcx: TyCtxt<'tcx>, konst_pat: Const, konst: ty::Const<'tcx>) -> bool {
        match (konst_pat, konst.kind()) {
            (Const::ConstVar(const_var), _) => self.match_const_var(tcx, const_var, konst),
            (Const::Value(_value_pat), ty::Value(_ty, ty::ValTree::Leaf(_value))) => todo!(),
            _ => false,
        }
    }
    pub fn match_const_var(&self, tcx: TyCtxt<'tcx>, const_var: ConstVarIdx, konst: ty::Const<'tcx>) -> bool {
        if let ty::ConstKind::Value(ty, _) = konst.kind() {
            return self.match_ty(tcx, self.const_vars[const_var], ty);
        }
        false
    }
    pub fn match_const_operand(
        &self,
        tcx: TyCtxt<'tcx>,
        konst_pat: &ConstOperand<'tcx>,
        konst: mir::Const<'tcx>,
    ) -> bool {
        match (konst_pat, konst) {
            (&ConstOperand::ConstVar(const_var), mir::Const::Ty(_, konst)) => {
                self.match_const_var(tcx, const_var, konst)
            },
            (&ConstOperand::ScalarInt(_value_pat), mir::Const::Val(mir::ConstValue::Scalar(_value), _ty)) => {
                todo!()
            },
            (&ConstOperand::ZeroSized(ty_pat), mir::Const::Val(mir::ConstValue::ZeroSized, ty)) => {
                self.match_ty(tcx, ty_pat, ty)
            },
            _ => false,
        }
    }
    pub fn match_region(&self, pat: RegionKind, region: ty::Region<'tcx>) -> bool {
        matches!(
            (pat, region.kind()),
            (RegionKind::ReStatic, ty::RegionKind::ReStatic) | (RegionKind::ReAny, _)
        )
    }
    pub fn match_aggregate(
        &self,
        tcx: TyCtxt<'tcx>,
        body: &mir::Body<'tcx>,
        agg_kind_pat: &AggKind<'tcx>,
        operands_pat: &[Operand<'tcx>],
        agg_kind: &mir::AggregateKind<'tcx>,
        operands: &IndexSlice<FieldIdx, mir::Operand<'tcx>>,
    ) -> bool {
        match (agg_kind_pat, agg_kind) {
            (&AggKind::Array, &mir::AggregateKind::Array(_)) | (AggKind::Tuple, mir::AggregateKind::Tuple) => {
                self.match_agg_operands(tcx, body, operands_pat, &operands.raw)
            },
            (
                &AggKind::Adt(path, args_pat, ref fields),
                &mir::AggregateKind::Adt(def_id, variant_idx, args, _, field_idx),
            ) if let Some(remainder) = self.match_item_path(tcx, path, def_id) => {
                let adt = tcx.adt_def(def_id);
                let variant = adt.variant(variant_idx);
                let variant_matched = match remainder {
                    [] => {
                        variant_idx.as_u32() == 0 && matches!(adt.adt_kind(), ty::AdtKind::Struct | ty::AdtKind::Union)
                    },
                    &[name] => variant.name == name,
                    _ => false,
                };
                let generics_matched = self.match_generic_args(tcx, args_pat, args);
                let fields_matched = match (fields, field_idx) {
                    (None, None) => {
                        variant.ctor.is_some() && self.match_agg_operands(tcx, body, operands_pat, &operands.raw)
                    },
                    (&Some(box [name]), Some(field_idx)) => adt.is_union() && variant.fields[field_idx].name == name,
                    (Some(box ref names), None) => {
                        let indices = names
                            .iter()
                            .enumerate()
                            .map(|(idx, &name)| (name, idx))
                            .collect::<FxHashMap<_, _>>();
                        variant.ctor.is_none()
                            && operands_pat.len() == operands.len()
                            && operands.iter_enumerated().all(|(idx, operand)| {
                                indices
                                    .get(&variant.fields[idx].name)
                                    .is_some_and(|&idx| self.match_operand(tcx, body, &operands_pat[idx], operand))
                            })
                    },
                    _ => false,
                };
                variant_matched && generics_matched && fields_matched
            },
            (&AggKind::RawPtr(ty_pat, mutability_pat), &mir::AggregateKind::RawPtr(ty, mutability)) => {
                self.match_ty(tcx, ty_pat, ty)
                    && mutability_pat == mutability
                    && self.match_agg_operands(tcx, body, operands_pat, &operands.raw)
            },
            _ => false,
        }
    }

    fn match_agg_operands(
        &self,
        tcx: TyCtxt<'tcx>,
        body: &mir::Body<'tcx>,
        operands_pat: &[Operand<'tcx>],
        operands: &[mir::Operand<'tcx>],
    ) -> bool {
        operands_pat.len() == operands.len()
            && core::iter::zip(operands_pat, operands)
                .all(|(operand_pat, operand)| self.match_operand(tcx, body, operand_pat, operand))
    }

    pub fn match_operand(
        &self,
        tcx: TyCtxt<'tcx>,
        body: &mir::Body<'tcx>,
        pat: &Operand<'tcx>,
        operand: &mir::Operand<'tcx>,
    ) -> bool {
        let matched = match (pat, operand) {
            (&Operand::Copy(place_pat), &mir::Operand::Copy(place))
            | (&Operand::Move(place_pat), &mir::Operand::Move(place)) => {
                self.match_place_ref(tcx, body, place_pat, place.as_ref())
            },
            (Operand::Constant(konst_pat), mir::Operand::Constant(box konst)) => {
                self.match_const_operand(tcx, konst_pat, konst.const_)
            },
            _ => return false,
        };
        debug!(?pat, ?operand, matched, "match_operand");
        matched
    }
}
