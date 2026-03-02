use std::fmt;

use rustc_data_structures::stack::ensure_sufficient_stack;
use rustc_index::{Idx, IndexVec};

use crate::CountedMatch;

/// A slot for a single matchable variable within a domain.
/// Holds the current match state and the list of candidate values.
pub struct VarSlot<T: Copy + PartialEq> {
    pub(crate) matched: CountedMatch<T>,
    pub(crate) candidates: Vec<T>,
}

impl<T: Copy + PartialEq + fmt::Debug> fmt::Debug for VarSlot<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VarSlot")
            .field("matched", &self.matched)
            .field("candidates", &self.candidates)
            .finish()
    }
}

impl<T: Copy + PartialEq> VarSlot<T> {
    pub fn new() -> Self {
        Self {
            matched: CountedMatch::new(),
            candidates: Vec::new(),
        }
    }

    pub fn with_candidates(candidates: Vec<T>) -> Self {
        Self {
            matched: CountedMatch::new(),
            candidates,
        }
    }

    pub fn get(&self) -> Option<T> {
        self.matched.get()
    }

    #[track_caller]
    pub fn force_get(&self) -> T {
        self.matched.get().expect("bug: variable not matched")
    }

    pub fn has_empty_candidates(&self) -> bool {
        self.candidates.is_empty()
    }
}

/// A domain of matchable variables of the same kind.
///
/// Encapsulates a set of indexed variables along with their
/// candidate values from the target MIR.
pub struct VarDomain<I: Idx, T: Copy + PartialEq> {
    pub(crate) vars: IndexVec<I, VarSlot<T>>,
}

impl<I: Idx, T: Copy + PartialEq + fmt::Debug> fmt::Debug for VarDomain<I, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VarDomain")
            .field("vars", &self.vars)
            .finish()
    }
}

impl<I: Idx, T: Copy + PartialEq> VarDomain<I, T> {
    pub fn new(size: usize) -> Self {
        Self {
            vars: IndexVec::from_fn_n(|_| VarSlot::new(), size),
        }
    }

    pub fn from_candidates(candidates: impl IntoIterator<Item = Vec<T>>) -> Self {
        Self {
            vars: candidates
                .into_iter()
                .map(VarSlot::with_candidates)
                .collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.vars.len()
    }

    pub fn has_empty_candidates(&self) -> bool {
        self.vars.iter().any(|slot| slot.has_empty_candidates())
    }

    /// Run backtracking search over all variables in this domain,
    /// starting from variable index `start`.
    ///
    /// For each variable, iterates over its candidates:
    /// - Calls `try_assign(var, candidate)` to check consistency
    /// - If consistent, recurses to the next variable
    /// - On return, calls `on_unassign(var)` to undo any side effects
    /// - The CountedMatch state is managed automatically
    ///
    /// When all variables are assigned, calls `on_complete()`.
    pub fn backtrack(
        &self,
        start: I,
        try_assign: &impl Fn(I, T) -> bool,
        on_unassign: &impl Fn(I),
        on_complete: &mut impl FnMut(),
    ) {
        if start == self.vars.next_index() {
            on_complete();
            return;
        }
        for &candidate in &self.vars[start].candidates {
            if try_assign(start, candidate) && self.vars[start].matched.r#match(candidate) {
                ensure_sufficient_stack(|| {
                    self.backtrack(start.plus(1), try_assign, on_unassign, on_complete);
                });
                self.vars[start].matched.unmatch();
                on_unassign(start);
            }
        }
    }

    /// Get the matched value for a variable, if any.
    pub fn get(&self, var: I) -> Option<T> {
        self.vars[var].get()
    }

    /// Get the matched value for a variable, panicking if not matched.
    #[track_caller]
    pub fn force_get(&self, var: I) -> T {
        self.vars[var].force_get()
    }

    /// Collect all matched values into an IndexVec.
    /// Panics if any variable is unmatched.
    pub fn to_matched(&self) -> IndexVec<I, T> {
        self.vars
            .iter_enumerated()
            .map(|(idx, slot)| {
                slot.get()
                    .unwrap_or_else(|| panic!("bug: variable {:?} not matched", idx.index()))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustc_index::newtype_index;

    newtype_index! {
        struct TestIdx {}
    }

    #[test]
    fn var_slot_starts_empty() {
        let slot = VarSlot::<u32>::new();
        assert!(slot.get().is_none());
        assert!(slot.has_empty_candidates());
    }

    #[test]
    fn var_slot_with_candidates() {
        let slot = VarSlot::with_candidates(vec![1u32, 2, 3]);
        assert!(slot.get().is_none());
        assert!(!slot.has_empty_candidates());
    }

    #[test]
    fn var_domain_empty() {
        let domain = VarDomain::<TestIdx, u32>::new(0);
        assert_eq!(domain.len(), 0);
        assert!(!domain.has_empty_candidates());
    }

    #[test]
    fn var_domain_detects_empty_candidates() {
        let domain = VarDomain::<TestIdx, u32>::from_candidates(vec![
            vec![1, 2],
            vec![], // empty!
            vec![3],
        ]);
        assert!(domain.has_empty_candidates());
    }

    #[test]
    fn backtrack_single_var_single_candidate() {
        let domain = VarDomain::<TestIdx, u32>::from_candidates(vec![vec![42]]);
        let mut results = Vec::new();
        domain.backtrack(
            TestIdx::ZERO,
            &|_var, _val| true,
            &|_var| {},
            &mut || {
                results.push(domain.force_get(TestIdx::ZERO));
            },
        );
        assert_eq!(results, vec![42]);
    }

    #[test]
    fn backtrack_single_var_multiple_candidates() {
        let domain = VarDomain::<TestIdx, u32>::from_candidates(vec![vec![1, 2, 3]]);
        let mut results = Vec::new();
        domain.backtrack(
            TestIdx::ZERO,
            &|_var, _val| true,
            &|_var| {},
            &mut || {
                results.push(domain.force_get(TestIdx::ZERO));
            },
        );
        assert_eq!(results, vec![1, 2, 3]);
    }

    #[test]
    fn backtrack_two_vars_cartesian_product() {
        let domain = VarDomain::<TestIdx, u32>::from_candidates(vec![
            vec![1, 2],
            vec![10, 20],
        ]);
        let mut results = Vec::new();
        domain.backtrack(
            TestIdx::ZERO,
            &|_var, _val| true,
            &|_var| {},
            &mut || {
                let a = domain.force_get(TestIdx::from_u32(0));
                let b = domain.force_get(TestIdx::from_u32(1));
                results.push((a, b));
            },
        );
        assert_eq!(results, vec![(1, 10), (1, 20), (2, 10), (2, 20)]);
    }

    #[test]
    fn backtrack_with_filtering() {
        let domain = VarDomain::<TestIdx, u32>::from_candidates(vec![
            vec![1, 2, 3],
            vec![1, 2, 3],
        ]);
        let mut results = Vec::new();
        domain.backtrack(
            TestIdx::ZERO,
            &|var, val| {
                // Constraint: second var must be greater than first
                if var == TestIdx::from_u32(1) {
                    let first = domain.force_get(TestIdx::ZERO);
                    val > first
                } else {
                    true
                }
            },
            &|_var| {},
            &mut || {
                let a = domain.force_get(TestIdx::from_u32(0));
                let b = domain.force_get(TestIdx::from_u32(1));
                results.push((a, b));
            },
        );
        assert_eq!(results, vec![(1, 2), (1, 3), (2, 3)]);
    }

    #[test]
    fn backtrack_counted_match_allows_reuse() {
        // Two variables can match the same value (CountedMatch increments)
        let domain = VarDomain::<TestIdx, u32>::from_candidates(vec![
            vec![1],
            vec![1],
        ]);
        let mut count = 0;
        domain.backtrack(
            TestIdx::ZERO,
            &|_var, _val| true,
            &|_var| {},
            &mut || { count += 1; },
        );
        assert_eq!(count, 1);
    }

    #[test]
    fn to_matched_collects_all() {
        let domain = VarDomain::<TestIdx, u32>::from_candidates(vec![
            vec![10],
            vec![20],
        ]);
        domain.backtrack(
            TestIdx::ZERO,
            &|_var, _val| true,
            &|_var| {},
            &mut || {
                let matched = domain.to_matched();
                assert_eq!(matched[TestIdx::from_u32(0)], 10);
                assert_eq!(matched[TestIdx::from_u32(1)], 20);
            },
        );
    }
}
