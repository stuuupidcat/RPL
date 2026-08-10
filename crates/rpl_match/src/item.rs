use rpl_constraints::Constraints;
use rpl_constraints::predicates::PredicateKind;
use rpl_context::pat;
use rpl_resolve::{PatItemKind, def_path_res};
use rustc_data_structures::fx::FxHashMap;
use rustc_hir as hir;
use rustc_hir::def::Res;
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_middle::ty::{self, Ty, TyCtxt};
use rustc_span::Symbol;

#[derive(Debug)]
pub struct ItemMatched<'tcx> {
    pub root_impl: LocalDefId,
    pub self_ty: Ty<'tcx>,
    pub self_args: ty::GenericArgsRef<'tcx>,
    pub adts: FxHashMap<Symbol, DefId>,
    pub impls: FxHashMap<Symbol, LocalDefId>,
}

pub struct MatchItemCtxt<'pat, 'pcx, 'tcx> {
    tcx: TyCtxt<'tcx>,
    pat: &'pat pat::RustItems<'pcx>,
}

impl<'pat, 'pcx, 'tcx> MatchItemCtxt<'pat, 'pcx, 'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>, pat: &'pat pat::RustItems<'pcx>) -> Self {
        Self { tcx, pat }
    }

    #[instrument(level = "debug", skip(self, impl_), ret)]
    pub fn match_impl(&self, impl_def_id: LocalDefId, impl_: &hir::Impl<'tcx>) -> Option<ItemMatched<'tcx>> {
        if !matches!(impl_.safety, hir::Safety::Unsafe) || !matches!(impl_.polarity, hir::ImplPolarity::Positive) {
            return None;
        }

        let impl_pat = self.supported_impl_pattern()?;
        let trait_ref = self.tcx.impl_trait_ref(impl_def_id.to_def_id())?.instantiate_identity();
        if !trait_path_matches(self.tcx, impl_pat.trait_path?, trait_ref.def_id) {
            return None;
        }

        let self_ty = trait_ref.self_ty();
        let ty::Adt(adt, self_args) = *self_ty.kind() else {
            return None;
        };
        if !adt.is_struct() || !adt.did().is_local() {
            return None;
        }

        let pat::TyKind::AdtPat(wrapper) = impl_pat.self_ty.ty.kind() else {
            return None;
        };
        let adt_pat = self.pat.get_adt(*wrapper)?;
        if !supported_struct_pattern(adt_pat) {
            return None;
        }

        let marker = impl_pat.binding?;
        let mut adts = FxHashMap::default();
        adts.insert(*wrapper, adt.did());
        let mut impls = FxHashMap::default();
        impls.insert(marker, impl_def_id);

        Some(ItemMatched {
            root_impl: impl_def_id,
            self_ty,
            self_args,
            adts,
            impls,
        })
    }

    fn supported_impl_pattern(&self) -> Option<&pat::Impl<'pcx>> {
        if self.pat.adts.len() != 1
            || self.pat.impls.len() != 1
            || self.pat.fns.iter().next().is_some()
            || self.pat.meta.adt_vars.len() != 1
            || self
                .pat
                .meta
                .adt_vars
                .iter()
                .any(|adt_var| !adt_var.pred.clauses.is_empty())
            || !self.pat.meta.ty_vars.is_empty()
            || !self.pat.meta.const_vars.is_empty()
            || !self.pat.meta.place_vars.is_empty()
        {
            return None;
        }

        let impl_pat = self.pat.impls.first()?;
        (impl_pat.binding.is_some()
            && impl_pat.safety == pat::SafetyPat::Unsafe
            && impl_pat.polarity == pat::ImplPolarityPat::Positive
            && impl_pat.generics.rest == pat::RestPat::Rest
            && impl_pat.self_ty.generic_args == pat::RestPat::Rest
            && impl_pat.where_clause == pat::RestPat::Rest
            && impl_pat.fns.is_empty()
            && impl_pat.constraints.preds.is_empty()
            && !impl_pat.constraints.has_attributes)
            .then_some(impl_pat)
    }
}

fn supported_struct_pattern(adt_pat: &pat::Adt<'_>) -> bool {
    let pat::AdtKind::Struct(variant) = &adt_pat.kind else {
        return false;
    };
    adt_pat.generics.rest == pat::RestPat::Rest
        && variant.rest == pat::RestPat::Rest
        && variant.fields.is_empty()
        && adt_pat.constraints.preds.is_empty()
        && !adt_pat.constraints.has_attributes
}

fn trait_path_matches(tcx: TyCtxt<'_>, path: pat::Path<'_>, actual: DefId) -> bool {
    let pat::Path::Item(path) = path else {
        return false;
    };
    let mut resolved = def_path_res(tcx, path.0, PatItemKind::Trait)
        .into_iter()
        .filter_map(|res| match res {
            Res::Def(_, def_id) => Some(def_id),
            _ => None,
        });
    let Some(expected) = resolved.next() else {
        return false;
    };
    expected == actual && resolved.all(|def_id| def_id == expected)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsupportedItemPredicate {
    pub name: String,
}

pub struct ItemPredicateEvaluator<'matched, 'tcx> {
    matched: &'matched ItemMatched<'tcx>,
}

impl<'matched, 'tcx> ItemPredicateEvaluator<'matched, 'tcx> {
    pub fn new(matched: &'matched ItemMatched<'tcx>) -> Self {
        Self { matched }
    }

    #[instrument(level = "debug", skip(self, constraints), fields(root_impl = ?self.matched.root_impl), ret)]
    pub fn evaluate(&self, constraints: Option<&Constraints>) -> Result<bool, UnsupportedItemPredicate> {
        evaluate_item_constraints(constraints)
    }
}

fn evaluate_item_constraints(constraints: Option<&Constraints>) -> Result<bool, UnsupportedItemPredicate> {
    let Some(constraints) = constraints else {
        return Ok(true);
    };
    if constraints.has_attributes {
        return Err(UnsupportedItemPredicate {
            name: "<attribute>".to_string(),
        });
    }

    let mut result = true;
    for conjunction in &constraints.preds {
        let mut conjunction_result = true;
        for clause in &conjunction.clauses {
            let mut clause_result = false;
            for term in &clause.terms {
                let term_result = match term.kind {
                    PredicateKind::Trivial(predicate)
                        if matches!(term.name.as_str(), "true" | "false") && term.args.is_empty() =>
                    {
                        predicate()
                    },
                    _ => {
                        return Err(UnsupportedItemPredicate {
                            name: term.name.clone(),
                        });
                    },
                };
                clause_result |= if term.is_neg { !term_result } else { term_result };
            }
            conjunction_result &= clause_result;
        }
        result &= conjunction_result;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use rpl_constraints::Constraints;
    use rpl_constraints::predicates::{
        PredicateClause, PredicateConjunction, PredicateKind, PredicateTerm, TrivialPredsFnPtr,
    };

    use super::evaluate_item_constraints;

    fn guard(name: &str, predicate: TrivialPredsFnPtr) -> Constraints {
        Constraints {
            preds: vec![PredicateConjunction {
                clauses: vec![PredicateClause {
                    terms: vec![PredicateTerm {
                        name: name.to_string(),
                        kind: PredicateKind::Trivial(predicate),
                        args: Vec::new(),
                        is_neg: false,
                    }],
                }],
            }],
            attrs: Default::default(),
            has_attributes: false,
        }
    }

    #[test]
    fn evaluates_true_and_false_item_guards() {
        let true_guard = guard("true", rpl_constraints::predicates::r#true);
        let false_guard = guard("false", rpl_constraints::predicates::r#false);

        assert_eq!(evaluate_item_constraints(Some(&true_guard)), Ok(true));
        assert_eq!(evaluate_item_constraints(Some(&false_guard)), Ok(false));
    }
}
