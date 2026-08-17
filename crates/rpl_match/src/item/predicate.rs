use rpl_constraints::Constraints;
use rpl_constraints::predicates::{
    ItemPredicate, PredicateArg, PredicateClause, PredicateConjunction, PredicateKind, PredicateTerm,
};
use rpl_constraints::tribool::TriBool;
use rustc_data_structures::fx::{FxHashMap, FxHashSet};
use rustc_hir::def::DefKind;
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_hir::{Mutability, Safety};
use rustc_middle::ty::{self, Ty, TyCtxt};
use rustc_span::Symbol;

use super::ItemMatched;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsupportedItemPredicate {
    pub name: String,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TypeParameter<'tcx> {
    owner: DefId,
    def_id: DefId,
    index: u32,
    ty: Ty<'tcx>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AccessContext<'tcx> {
    method: DefId,
    resource: Ty<'tcx>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ItemValue<'tcx> {
    Adt(DefId),
    Impl(LocalDefId),
    TypeParameter(TypeParameter<'tcx>),
    Ty(Ty<'tcx>),
    Access(AccessContext<'tcx>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SharedRefAccessKind {
    Exclusive,
    Concurrent,
}

#[derive(Debug)]
struct AccessRows<'tcx> {
    accesses: Vec<AccessContext<'tcx>>,
    complete: bool,
}

#[derive(Clone, Debug, Default)]
struct ItemBindings<'tcx> {
    values: FxHashMap<Symbol, ItemValue<'tcx>>,
}

impl<'tcx> ItemBindings<'tcx> {
    fn seed(matched: &ItemMatched<'tcx>) -> Self {
        let mut values = FxHashMap::default();
        values.extend(
            matched
                .adts
                .iter()
                .map(|(name, def_id)| (*name, ItemValue::Adt(*def_id))),
        );
        values.extend(
            matched
                .impls
                .iter()
                .map(|(name, def_id)| (*name, ItemValue::Impl(*def_id))),
        );
        Self { values }
    }

    fn get(&self, name: Symbol) -> Option<ItemValue<'tcx>> {
        self.values.get(&name).copied()
    }

    fn bind(mut self, name: Symbol, value: ItemValue<'tcx>) -> Option<Self> {
        match self.values.get(&name) {
            Some(previous) if *previous != value => None,
            Some(_) => Some(self),
            None => {
                self.values.insert(name, value);
                Some(self)
            },
        }
    }
}

#[derive(Debug)]
struct EvalRows<'tcx> {
    rows: Vec<ItemBindings<'tcx>>,
    complete: bool,
}

impl<'tcx> EvalRows<'tcx> {
    fn one(row: ItemBindings<'tcx>) -> Self {
        Self {
            rows: vec![row],
            complete: true,
        }
    }

    fn empty() -> Self {
        Self {
            rows: Vec::new(),
            complete: true,
        }
    }

    fn unknown() -> Self {
        Self {
            rows: Vec::new(),
            complete: false,
        }
    }

    fn decision(row: ItemBindings<'tcx>, result: TriBool) -> Self {
        match result {
            TriBool::True => Self::one(row),
            TriBool::False => Self::empty(),
            TriBool::Unknown => Self::unknown(),
        }
    }

    fn truth(&self) -> TriBool {
        if !self.rows.is_empty() {
            TriBool::True
        } else if self.complete {
            TriBool::False
        } else {
            TriBool::Unknown
        }
    }
}

pub struct ItemPredicateEvaluator<'matched, 'tcx> {
    tcx: TyCtxt<'tcx>,
    matched: &'matched ItemMatched<'tcx>,
}

impl<'matched, 'tcx> ItemPredicateEvaluator<'matched, 'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>, matched: &'matched ItemMatched<'tcx>) -> Self {
        Self { tcx, matched }
    }

    #[instrument(level = "debug", skip(self, constraints), fields(root_impl = ?self.matched.root_impl), ret)]
    pub fn evaluate(&self, constraints: Option<&Constraints>) -> Result<TriBool, UnsupportedItemPredicate> {
        let Some(constraints) = constraints else {
            return Ok(TriBool::True);
        };
        if constraints.has_attributes {
            return Err(UnsupportedItemPredicate {
                name: "<attribute>".to_string(),
                reason: "attributes are not supported in item guards".to_string(),
            });
        }

        let mut result = EvalRows::one(ItemBindings::seed(self.matched));
        for conjunction in &constraints.preds {
            result = self.evaluate_conjunction(result, conjunction)?;
            if result.rows.is_empty() && result.complete {
                break;
            }
        }
        Ok(result.truth())
    }

    fn evaluate_conjunction(
        &self,
        mut input: EvalRows<'tcx>,
        conjunction: &PredicateConjunction,
    ) -> Result<EvalRows<'tcx>, UnsupportedItemPredicate> {
        for clause in &conjunction.clauses {
            input = self.evaluate_clause(input, clause)?;
            if input.rows.is_empty() && input.complete {
                break;
            }
        }
        Ok(input)
    }

    fn evaluate_clause(
        &self,
        input: EvalRows<'tcx>,
        clause: &PredicateClause,
    ) -> Result<EvalRows<'tcx>, UnsupportedItemPredicate> {
        if clause.terms.len() == 1 {
            let term = &clause.terms[0];
            let mut output = EvalRows {
                rows: Vec::new(),
                complete: input.complete,
            };
            for row in input.rows {
                let result = self.evaluate_term(row, term)?;
                output.rows.extend(result.rows);
                output.complete &= result.complete;
            }
            return Ok(output);
        }

        // Meta checking restricts disjunctions to closed decision predicates, so a successful
        // alternative keeps the original row and an unknown alternative matters only when none
        // of the alternatives succeeds.
        let mut output = EvalRows {
            rows: Vec::new(),
            complete: input.complete,
        };
        for row in input.rows {
            let mut succeeded = false;
            let mut complete = true;
            for term in &clause.terms {
                let result = self.evaluate_term(row.clone(), term)?;
                if !result.rows.is_empty() {
                    succeeded = true;
                    break;
                }
                complete &= result.complete;
            }
            if succeeded {
                output.rows.push(row);
            } else {
                output.complete &= complete;
            }
        }
        Ok(output)
    }

    fn evaluate_term(
        &self,
        row: ItemBindings<'tcx>,
        term: &PredicateTerm,
    ) -> Result<EvalRows<'tcx>, UnsupportedItemPredicate> {
        let positive = self.evaluate_positive_term(row.clone(), term)?;
        if !term.is_neg {
            return Ok(positive);
        }
        if !positive.rows.is_empty() {
            Ok(EvalRows::empty())
        } else if positive.complete {
            Ok(EvalRows::one(row))
        } else {
            Ok(EvalRows::unknown())
        }
    }

    fn evaluate_positive_term(
        &self,
        row: ItemBindings<'tcx>,
        term: &PredicateTerm,
    ) -> Result<EvalRows<'tcx>, UnsupportedItemPredicate> {
        match term.kind {
            PredicateKind::Trivial(predicate) if term.args.is_empty() => {
                Ok(EvalRows::decision(row, predicate().into()))
            },
            PredicateKind::Item(predicate) => self.evaluate_item_predicate(row, term, predicate),
            _ => Err(self.error(term, "predicate is not supported by the item matcher")),
        }
    }

    fn evaluate_item_predicate(
        &self,
        row: ItemBindings<'tcx>,
        term: &PredicateTerm,
        predicate: ItemPredicate,
    ) -> Result<EvalRows<'tcx>, UnsupportedItemPredicate> {
        match predicate {
            ItemPredicate::HasTypeParameters => self.has_type_parameters(row, term),
            ItemPredicate::TypeParameterOf => self.type_parameter_of(row, term),
            ItemPredicate::TypeParameterMapsTo => self.type_parameter_maps_to(row, term),
            ItemPredicate::OwnsType => self.owns_type(row, term),
            ItemPredicate::IsSendIn => self.is_send_in(row, term),
            ItemPredicate::IsSyncIn => self.is_sync_in(row, term),
            ItemPredicate::IsSendForAccessIn => self.is_send_for_access_in(row, term),
            ItemPredicate::IsSyncForAccessIn => self.is_sync_for_access_in(row, term),
            ItemPredicate::ResourceExclusivelyAccessibleFromSharedRefIn => {
                self.resource_accessible_from_shared_ref_in(row, term, SharedRefAccessKind::Exclusive)
            },
            ItemPredicate::ResourceConcurrentlyAccessibleFromSharedRefIn => {
                self.resource_accessible_from_shared_ref_in(row, term, SharedRefAccessKind::Concurrent)
            },
        }
    }

    fn has_type_parameters(
        &self,
        row: ItemBindings<'tcx>,
        term: &PredicateTerm,
    ) -> Result<EvalRows<'tcx>, UnsupportedItemPredicate> {
        let wrapper = self.adt_arg(&row, term, 0)?;
        let has_type_parameters = self
            .tcx
            .generics_of(wrapper)
            .own_params
            .iter()
            .any(|param| matches!(param.kind, ty::GenericParamDefKind::Type { .. }));
        Ok(EvalRows::decision(row, has_type_parameters.into()))
    }

    fn type_parameter_of(
        &self,
        row: ItemBindings<'tcx>,
        term: &PredicateTerm,
    ) -> Result<EvalRows<'tcx>, UnsupportedItemPredicate> {
        let output = self.meta_arg(term, 0)?;
        let wrapper = self.adt_arg(&row, term, 1)?;
        let mut rows = Vec::new();
        for param in self
            .tcx
            .generics_of(wrapper)
            .own_params
            .iter()
            .filter(|param| matches!(param.kind, ty::GenericParamDefKind::Type { .. }))
        {
            let Some(param_ty) = self.tcx.mk_param_from_def(param).as_type() else {
                continue;
            };
            let parameter = TypeParameter {
                owner: wrapper,
                def_id: param.def_id,
                index: param.index,
                ty: param_ty,
            };
            if let Some(row) = row.clone().bind(output, ItemValue::TypeParameter(parameter)) {
                rows.push(row);
            }
        }
        Ok(EvalRows { rows, complete: true })
    }

    fn type_parameter_maps_to(
        &self,
        row: ItemBindings<'tcx>,
        term: &PredicateTerm,
    ) -> Result<EvalRows<'tcx>, UnsupportedItemPredicate> {
        let parameter = self.type_parameter_arg(&row, term, 0)?;
        let output = self.meta_arg(term, 1)?;
        let marker = self.impl_arg(&row, term, 2)?;
        let trait_ref = self
            .tcx
            .impl_trait_ref(marker.to_def_id())
            .ok_or_else(|| self.error(term, "impl binding does not name a trait impl"))?
            .instantiate_identity();
        let ty::Adt(adt, args) = *trait_ref.self_ty().kind() else {
            return Ok(EvalRows::empty());
        };
        if adt.did() != parameter.owner {
            return Ok(EvalRows::empty());
        }
        let Some(mapped) = args.get(parameter.index as usize).and_then(|arg| arg.as_type()) else {
            return Ok(EvalRows::empty());
        };
        Ok(row
            .bind(output, ItemValue::Ty(mapped))
            .map_or_else(EvalRows::empty, EvalRows::one))
    }

    fn owns_type(
        &self,
        row: ItemBindings<'tcx>,
        term: &PredicateTerm,
    ) -> Result<EvalRows<'tcx>, UnsupportedItemPredicate> {
        let wrapper = self.adt_arg(&row, term, 0)?;
        let parameter = self.type_parameter_arg(&row, term, 1)?;
        if wrapper != parameter.owner {
            return Ok(EvalRows::empty());
        }

        // Deliberately naive ownership backend: follow by-value fields recursively, ignore
        // PhantomData and non-owning pointer/reference edges, and preserve `Unknown` for aliases.
        // More precise ownership analyses can replace this method without changing row planning.
        let args = ty::GenericArgs::identity_for_item(self.tcx, wrapper);
        let mut visited = FxHashSet::default();
        let mut result = TriBool::False;
        for field in self.tcx.adt_def(wrapper).all_fields() {
            result = result | self.ty_owns_parameter(field.ty(self.tcx, args), parameter, &mut visited);
            if result == TriBool::True {
                break;
            }
        }
        Ok(EvalRows::decision(row, result))
    }

    fn ty_owns_parameter(
        &self,
        ty: Ty<'tcx>,
        parameter: TypeParameter<'tcx>,
        visited: &mut FxHashSet<Ty<'tcx>>,
    ) -> TriBool {
        if !visited.insert(ty) {
            return TriBool::False;
        }
        match ty.kind() {
            ty::Param(param) => (param.index == parameter.index).into(),
            ty::Tuple(types) => types.iter().fold(TriBool::False, |result, ty| {
                result | self.ty_owns_parameter(ty, parameter, visited)
            }),
            ty::Array(element, _) | ty::Slice(element) | ty::Pat(element, _) => {
                self.ty_owns_parameter(*element, parameter, visited)
            },
            ty::Adt(adt, _) if adt.is_phantom_data() => TriBool::False,
            ty::Adt(adt, args) if adt.is_box() => args
                .iter()
                .find_map(|arg| arg.as_type())
                .map_or(TriBool::Unknown, |ty| self.ty_owns_parameter(ty, parameter, visited)),
            ty::Adt(adt, args) => adt.all_fields().fold(TriBool::False, |result, field| {
                result | self.ty_owns_parameter(field.ty(self.tcx, args), parameter, visited)
            }),
            ty::Alias(..) => TriBool::Unknown,
            ty::Ref(..) | ty::RawPtr(..) | ty::FnDef(..) | ty::FnPtr(..) => TriBool::False,
            _ => TriBool::False,
        }
    }

    fn is_send_in(
        &self,
        row: ItemBindings<'tcx>,
        term: &PredicateTerm,
    ) -> Result<EvalRows<'tcx>, UnsupportedItemPredicate> {
        let ty = self.ty_arg(&row, term, 0)?;
        let marker = self.impl_arg(&row, term, 1)?;
        let typing_env = ty::TypingEnv::post_analysis(self.tcx, marker.to_def_id());

        // This is the deliberately naive backend for the first implementation. The relational
        // evaluator already carries incompleteness separately, so a future analysis can return
        // `Unknown` instead of treating every failed trait proof as `False`.
        let result = rpl_constraints::predicates::is_send(self.tcx, typing_env, ty);
        Ok(EvalRows::decision(row, result.into()))
    }

    fn is_sync_in(
        &self,
        row: ItemBindings<'tcx>,
        term: &PredicateTerm,
    ) -> Result<EvalRows<'tcx>, UnsupportedItemPredicate> {
        let ty = self.ty_arg(&row, term, 0)?;
        let marker = self.impl_arg(&row, term, 1)?;
        let typing_env = ty::TypingEnv::post_analysis(self.tcx, marker.to_def_id());

        // Keep the same deliberately naive proof backend as `is_send_in`. The relational
        // evaluator already preserves `Unknown`, so this can become a complete three-valued
        // decision later without changing the predicate contract.
        let result = rpl_constraints::predicates::is_sync(self.tcx, typing_env, ty);
        Ok(EvalRows::decision(row, result.into()))
    }

    fn resource_accessible_from_shared_ref_in(
        &self,
        row: ItemBindings<'tcx>,
        term: &PredicateTerm,
        kind: SharedRefAccessKind,
    ) -> Result<EvalRows<'tcx>, UnsupportedItemPredicate> {
        let access_output = self.meta_arg(term, 0)?;
        let resource_output = self.meta_arg(term, 1)?;
        let wrapper = self.adt_arg(&row, term, 2)?;
        let parameter = self.type_parameter_arg(&row, term, 3)?;
        let mapped = self.ty_arg(&row, term, 4)?;
        let marker = self.impl_arg(&row, term, 5)?;

        if parameter.owner != wrapper || !self.marker_maps_parameter_to(marker, parameter, mapped) {
            return Ok(EvalRows::empty());
        }

        // Phase one recognizes direct safe `&self` signatures and uses the matched type
        // argument as the resource. Each emitted access retains its method-specific typing
        // environment, so later predicates do not join evidence from unrelated APIs. The public
        // relation is intentionally broader: a future implementation may emit projected or
        // carrier types after analyzing method bodies, guards, trait APIs, and specialized
        // substitutions.
        let access_rows = self.shared_ref_accesses(wrapper, parameter, kind);
        let mut rows = Vec::new();
        for access in access_rows.accesses {
            let Some(row) = row.clone().bind(access_output, ItemValue::Access(access)) else {
                continue;
            };
            if let Some(row) = row.bind(resource_output, ItemValue::Ty(mapped)) {
                rows.push(row);
            }
        }
        Ok(EvalRows {
            rows,
            complete: access_rows.complete,
        })
    }

    fn is_send_for_access_in(
        &self,
        row: ItemBindings<'tcx>,
        term: &PredicateTerm,
    ) -> Result<EvalRows<'tcx>, UnsupportedItemPredicate> {
        self.trait_for_access_in(row, term, rpl_constraints::predicates::is_send)
    }

    fn is_sync_for_access_in(
        &self,
        row: ItemBindings<'tcx>,
        term: &PredicateTerm,
    ) -> Result<EvalRows<'tcx>, UnsupportedItemPredicate> {
        self.trait_for_access_in(row, term, rpl_constraints::predicates::is_sync)
    }

    fn trait_for_access_in(
        &self,
        row: ItemBindings<'tcx>,
        term: &PredicateTerm,
        prove: impl Fn(TyCtxt<'tcx>, ty::TypingEnv<'tcx>, Ty<'tcx>) -> bool,
    ) -> Result<EvalRows<'tcx>, UnsupportedItemPredicate> {
        let resource = self.ty_arg(&row, term, 0)?;
        let access = self.access_arg(&row, term, 1)?;
        let marker = self.impl_arg(&row, term, 2)?;

        // This first backend accepts a proof from either side of the correlated access:
        // bounds on the unsafe marker impl constrain its mapped resource, while bounds on the
        // method constrain the resource as named by that inherent impl. A complete evaluator can
        // replace this with one instantiated combined environment without changing the surface
        // predicate or the access correlation key.
        let marker_env = ty::TypingEnv::post_analysis(self.tcx, marker.to_def_id());
        let access_env = ty::TypingEnv::post_analysis(self.tcx, access.method);
        let result = prove(self.tcx, marker_env, resource) || prove(self.tcx, access_env, access.resource);
        Ok(EvalRows::decision(row, result.into()))
    }

    fn marker_maps_parameter_to(&self, marker: LocalDefId, parameter: TypeParameter<'tcx>, mapped: Ty<'tcx>) -> bool {
        let Some(trait_ref) = self.tcx.impl_trait_ref(marker.to_def_id()) else {
            return false;
        };
        let trait_ref = trait_ref.instantiate_identity();
        let ty::Adt(adt, args) = *trait_ref.self_ty().kind() else {
            return false;
        };
        adt.did() == parameter.owner
            && args
                .get(parameter.index as usize)
                .and_then(|arg| arg.as_type())
                .is_some_and(|ty| ty == mapped)
    }

    fn shared_ref_accesses(
        &self,
        wrapper: DefId,
        parameter: TypeParameter<'tcx>,
        kind: SharedRefAccessKind,
    ) -> AccessRows<'tcx> {
        let mut accesses = Vec::new();
        let mut complete = true;
        for &impl_def_id in self.tcx.inherent_impls(wrapper).iter() {
            let impl_self_ty = self.tcx.type_of(impl_def_id).instantiate_identity();
            let ty::Adt(adt, args) = *impl_self_ty.kind() else {
                continue;
            };
            if adt.did() != wrapper {
                continue;
            }
            let Some(impl_parameter) = args.get(parameter.index as usize).and_then(|arg| arg.as_type()) else {
                complete = false;
                continue;
            };
            if !matches!(impl_parameter.kind(), ty::Param(_)) {
                // Specialized inherent impls need unification with the matched marker impl. Keep
                // this incomplete rather than joining evidence from incompatible substitutions.
                complete = false;
                continue;
            }

            for &method in self.tcx.associated_item_def_ids(impl_def_id) {
                if self.tcx.def_kind(method) != DefKind::AssocFn {
                    continue;
                }
                let signature = self.tcx.fn_sig(method).instantiate_identity().skip_binder();
                if signature.safety != Safety::Safe {
                    continue;
                }
                let inputs = signature.inputs();
                let Some((&receiver, arguments)) = inputs.split_first() else {
                    continue;
                };
                if !self.is_direct_shared_receiver(receiver, wrapper) {
                    continue;
                }

                let method_result = match kind {
                    SharedRefAccessKind::Exclusive => {
                        arguments.iter().copied().fold(TriBool::False, |found, ty| {
                            found | Self::contains_owned_parameter(ty, impl_parameter, &mut FxHashSet::default())
                        }) | Self::contains_exclusive_output_parameter(
                            signature.output(),
                            impl_parameter,
                            &mut FxHashSet::default(),
                        )
                    },
                    SharedRefAccessKind::Concurrent => Self::contains_shared_output_parameter(
                        signature.output(),
                        impl_parameter,
                        &mut FxHashSet::default(),
                    ),
                };
                match method_result {
                    TriBool::True => accesses.push(AccessContext {
                        method,
                        resource: impl_parameter,
                    }),
                    TriBool::False => {},
                    TriBool::Unknown => complete = false,
                }
            }
        }
        AccessRows { accesses, complete }
    }

    fn is_direct_shared_receiver(&self, receiver: Ty<'tcx>, wrapper: DefId) -> bool {
        let ty::Ref(_, self_ty, Mutability::Not) = *receiver.kind() else {
            return false;
        };
        matches!(self_ty.kind(), ty::Adt(adt, _) if adt.did() == wrapper)
    }

    fn contains_owned_parameter(ty: Ty<'tcx>, parameter: Ty<'tcx>, visited: &mut FxHashSet<Ty<'tcx>>) -> TriBool {
        if ty == parameter {
            return TriBool::True;
        }
        if !visited.insert(ty) {
            return TriBool::False;
        }
        match ty.kind() {
            ty::Tuple(types) => types.iter().fold(TriBool::False, |found, ty| {
                found | Self::contains_owned_parameter(ty, parameter, visited)
            }),
            ty::Array(element, _) | ty::Slice(element) | ty::Pat(element, _) => {
                Self::contains_owned_parameter(*element, parameter, visited)
            },
            ty::Adt(adt, _) if adt.is_phantom_data() => TriBool::False,
            ty::Adt(_, args) => args
                .iter()
                .filter_map(|arg| arg.as_type())
                .fold(TriBool::False, |found, ty| {
                    found | Self::contains_owned_parameter(ty, parameter, visited)
                }),
            ty::Alias(..) => TriBool::Unknown,
            ty::Ref(..) | ty::RawPtr(..) | ty::FnDef(..) | ty::FnPtr(..) => TriBool::False,
            _ => TriBool::False,
        }
    }

    fn contains_exclusive_output_parameter(
        ty: Ty<'tcx>,
        parameter: Ty<'tcx>,
        visited: &mut FxHashSet<Ty<'tcx>>,
    ) -> TriBool {
        if ty == parameter {
            return TriBool::True;
        }
        if !visited.insert(ty) {
            return TriBool::False;
        }
        match ty.kind() {
            ty::Ref(_, referent, Mutability::Mut) => Self::contains_parameter_dependency(*referent, parameter, visited),
            ty::Ref(_, _, Mutability::Not) | ty::RawPtr(..) => TriBool::False,
            ty::Tuple(types) => types.iter().fold(TriBool::False, |found, ty| {
                found | Self::contains_exclusive_output_parameter(ty, parameter, visited)
            }),
            ty::Array(element, _) | ty::Slice(element) | ty::Pat(element, _) => {
                Self::contains_exclusive_output_parameter(*element, parameter, visited)
            },
            ty::Adt(adt, _) if adt.is_phantom_data() => TriBool::False,
            ty::Adt(_, args) => args
                .iter()
                .filter_map(|arg| arg.as_type())
                .fold(TriBool::False, |found, ty| {
                    Self::contains_exclusive_output_parameter(ty, parameter, visited) | found
                }),
            ty::Alias(..) => TriBool::Unknown,
            ty::FnDef(..) | ty::FnPtr(..) => TriBool::False,
            _ => TriBool::False,
        }
    }

    fn contains_shared_output_parameter(
        ty: Ty<'tcx>,
        parameter: Ty<'tcx>,
        visited: &mut FxHashSet<Ty<'tcx>>,
    ) -> TriBool {
        if !visited.insert(ty) {
            return TriBool::False;
        }
        match ty.kind() {
            ty::Ref(_, referent, Mutability::Not) => Self::contains_parameter_dependency(*referent, parameter, visited),
            ty::Ref(_, _, Mutability::Mut) | ty::RawPtr(..) => TriBool::False,
            ty::Tuple(types) => types.iter().fold(TriBool::False, |found, ty| {
                found | Self::contains_shared_output_parameter(ty, parameter, visited)
            }),
            ty::Array(element, _) | ty::Slice(element) | ty::Pat(element, _) => {
                Self::contains_shared_output_parameter(*element, parameter, visited)
            },
            ty::Adt(adt, _) if adt.is_phantom_data() => TriBool::False,
            ty::Adt(_, args) => args
                .iter()
                .filter_map(|arg| arg.as_type())
                .fold(TriBool::False, |found, ty| {
                    found | Self::contains_shared_output_parameter(ty, parameter, visited)
                }),
            ty::Alias(..) => TriBool::Unknown,
            _ => TriBool::False,
        }
    }

    fn contains_parameter_dependency(ty: Ty<'tcx>, parameter: Ty<'tcx>, visited: &mut FxHashSet<Ty<'tcx>>) -> TriBool {
        if ty == parameter {
            return TriBool::True;
        }
        if !visited.insert(ty) {
            return TriBool::False;
        }
        match ty.kind() {
            ty::Tuple(types) => types.iter().fold(TriBool::False, |found, ty| {
                found | Self::contains_parameter_dependency(ty, parameter, visited)
            }),
            ty::Array(element, _) | ty::Slice(element) | ty::Pat(element, _) | ty::Ref(_, element, _) => {
                Self::contains_parameter_dependency(*element, parameter, visited)
            },
            // Merely sharing storage for a raw pointer or PhantomData does not provide a safe
            // path to the pointed-to or marker type.
            ty::RawPtr(..) => TriBool::False,
            ty::Adt(adt, _) if adt.is_phantom_data() => TriBool::False,
            ty::Adt(_, args) => {
                let dependency = args
                    .iter()
                    .filter_map(|arg| arg.as_type())
                    .fold(TriBool::False, |found, ty| {
                        found | Self::contains_parameter_dependency(ty, parameter, visited)
                    });
                if dependency == TriBool::False {
                    TriBool::False
                } else {
                    // A reference to an arbitrary carrier is not automatically a safe reference
                    // to its generic argument. Body/API analysis is needed to decide projection.
                    TriBool::Unknown
                }
            },
            ty::FnDef(..) | ty::FnPtr(..) => TriBool::False,
            ty::Alias(..) => TriBool::Unknown,
            _ => TriBool::False,
        }
    }

    fn meta_arg(&self, term: &PredicateTerm, index: usize) -> Result<Symbol, UnsupportedItemPredicate> {
        match term.args.get(index) {
            Some(PredicateArg::MetaVar(name)) => Ok(*name),
            _ => Err(self.error(term, &format!("argument {index} is not a metavariable"))),
        }
    }

    fn value_arg(
        &self,
        row: &ItemBindings<'tcx>,
        term: &PredicateTerm,
        index: usize,
    ) -> Result<ItemValue<'tcx>, UnsupportedItemPredicate> {
        let name = self.meta_arg(term, index)?;
        row.get(name)
            .ok_or_else(|| self.error(term, &format!("metavariable `{name}` is not bound")))
    }

    fn adt_arg(
        &self,
        row: &ItemBindings<'tcx>,
        term: &PredicateTerm,
        index: usize,
    ) -> Result<DefId, UnsupportedItemPredicate> {
        match self.value_arg(row, term, index)? {
            ItemValue::Adt(def_id) => Ok(def_id),
            _ => Err(self.error(term, &format!("argument {index} is not an ADT"))),
        }
    }

    fn impl_arg(
        &self,
        row: &ItemBindings<'tcx>,
        term: &PredicateTerm,
        index: usize,
    ) -> Result<LocalDefId, UnsupportedItemPredicate> {
        match self.value_arg(row, term, index)? {
            ItemValue::Impl(def_id) => Ok(def_id),
            _ => Err(self.error(term, &format!("argument {index} is not an impl binding"))),
        }
    }

    fn type_parameter_arg(
        &self,
        row: &ItemBindings<'tcx>,
        term: &PredicateTerm,
        index: usize,
    ) -> Result<TypeParameter<'tcx>, UnsupportedItemPredicate> {
        match self.value_arg(row, term, index)? {
            ItemValue::TypeParameter(parameter) => Ok(parameter),
            _ => Err(self.error(term, &format!("argument {index} is not a type parameter"))),
        }
    }

    fn ty_arg(
        &self,
        row: &ItemBindings<'tcx>,
        term: &PredicateTerm,
        index: usize,
    ) -> Result<Ty<'tcx>, UnsupportedItemPredicate> {
        match self.value_arg(row, term, index)? {
            ItemValue::TypeParameter(parameter) => Ok(parameter.ty),
            ItemValue::Ty(ty) => Ok(ty),
            _ => Err(self.error(term, &format!("argument {index} is not a type"))),
        }
    }

    fn access_arg(
        &self,
        row: &ItemBindings<'tcx>,
        term: &PredicateTerm,
        index: usize,
    ) -> Result<AccessContext<'tcx>, UnsupportedItemPredicate> {
        match self.value_arg(row, term, index)? {
            ItemValue::Access(access) => Ok(access),
            _ => Err(self.error(term, &format!("argument {index} is not an access"))),
        }
    }

    fn error(&self, term: &PredicateTerm, reason: &str) -> UnsupportedItemPredicate {
        UnsupportedItemPredicate {
            name: term.name.clone(),
            reason: reason.to_string(),
        }
    }
}
