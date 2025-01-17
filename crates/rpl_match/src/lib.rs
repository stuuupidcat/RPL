#![feature(rustc_private)]
#![feature(let_chains)]
#![feature(if_let_guard)]
#![feature(cell_update)]

extern crate rustc_abi;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_middle;
extern crate rustc_span;
#[macro_use]
extern crate tracing;

use std::cell::Cell;
use std::fmt;
use std::num::NonZero;

mod adt;
mod fns;
pub(crate) mod resolve;
mod ty;

pub use adt::{AdtMatch, Candidates, MatchAdtCtxt};
pub use fns::MatchFnCtxt;
pub use ty::MatchTyCtxt;

pub struct CountedMatch<T>(Cell<Option<Counted<T>>>);

impl<T> Default for CountedMatch<T> {
    fn default() -> Self {
        Self(Cell::new(None))
    }
}

impl<T: Copy> Clone for CountedMatch<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: Copy + PartialEq> CountedMatch<T> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn get(&self) -> Option<T> {
        Some(self.0.get()?.value)
    }
    pub fn r#match(&self, value: T) -> bool {
        match self.0.get() {
            None => self.0.set(Some(Counted::new(value))),
            Some(l) if l.value == value => self.0.set(Some(l.inc())),
            Some(_) => return false,
        }
        true
    }
    pub fn unmatch(&self) {
        self.0.update(|m| m.and_then(Counted::dec));
    }
    pub fn try_take(&self) -> Option<T> {
        self.0.get().map(Counted::into_inner)
    }
}

#[derive(Clone, Copy)]
pub struct Counted<T> {
    value: T,
    count: NonZero<u32>,
}

impl<T> Counted<T> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            count: NonZero::<u32>::MIN,
        }
    }
    pub fn into_inner(self) -> T {
        self.value
    }
    pub fn inc(self) -> Self {
        Self {
            count: self.count.checked_add(1).unwrap(),
            ..self
        }
    }
    pub fn dec(self) -> Option<Self> {
        Some(Self {
            count: NonZero::new(self.count.get().wrapping_sub(1))?,
            ..self
        })
    }
}

impl<T: fmt::Debug> fmt::Debug for Counted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} {{{}}}", self.value, self.count)
    }
}

impl<T: Copy + fmt::Debug> fmt::Debug for CountedMatch<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.get().fmt(f)
    }
}
