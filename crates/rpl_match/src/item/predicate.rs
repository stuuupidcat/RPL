use rpl_constraints::Constraints;
use rpl_constraints::predicates::{
    ItemPredicate, PredicateArg, PredicateClause, PredicateConjunction, PredicateKind, PredicateTerm,
};
use rpl_constraints::tribool::TriBool;
use rustc_data_structures::fx::{FxHashMap, FxHashSet};
use rustc_hir::def_id::{DefId, LocalDefId};
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
enum ItemValue<'tcx> {
    Adt(DefId),
    Impl(LocalDefId),
    TypeParameter(TypeParameter<'tcx>),
    Ty(Ty<'tcx>),
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

    fn error(&self, term: &PredicateTerm, reason: &str) -> UnsupportedItemPredicate {
        UnsupportedItemPredicate {
            name: term.name.clone(),
            reason: reason.to_string(),
        }
    }
}
