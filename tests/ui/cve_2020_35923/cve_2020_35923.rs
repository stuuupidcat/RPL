//@ ignore-on-host

use std::cmp::Ordering;
use std::ops::AddAssign;

pub trait Float: Copy + PartialOrd + std::fmt::Debug {
    fn is_nan(self) -> bool;
}

/// A wrapper around floats providing an implementation of Eq, Ord and Hash.
/// A NaN value cannot be stored in this type.
#[derive(PartialOrd, PartialEq, Debug, Default, Clone, Copy)]
pub struct NotNan<T>(T);

impl<T: Float + PartialEq> Eq for NotNan<T> {}

impl<T: Float + AddAssign> AddAssign<T> for NotNan<T> {
    fn add_assign(&mut self, other: T) {
        self.0 += other;
        assert!(!self.0.is_nan(), "Addition resulted in NaN");
    }
}

impl<T: Float> Ord for NotNan<T> {
    fn cmp(&self, other: &NotNan<T>) -> Ordering {
        match self.partial_cmp(&other) {
            Some(ord) => ord,
            None => unsafe { std::hint::unreachable_unchecked() },
        }
    }
}
