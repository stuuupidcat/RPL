//! This module defines several types, including counters and unique ID numbers,
//! which is needed in meta data.

use rustc_index::Idx;
use schemars::JsonSchema;
use serde::Serialize;
use std::marker::PhantomData;

macro_rules! index {
    ($doc:literal $id:ident $counter_doc:literal $counter:ident) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $id(
            /// Wrapped number. Please don't modify this.
            pub usize,
        );
        impl std::fmt::Display for $id {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl Idx for $id {
            #[inline]
            fn new(idx: usize) -> Self {
                Self(idx)
            }
            #[inline]
            fn index(self) -> usize {
                self.0
            }
        }
        impl From<usize> for $id {
            fn from(idx: usize) -> Self {
                Self::new(idx)
            }
        }
        #[doc = $counter_doc]
        pub type $counter = Counter<$id>;
    };
    ($doc:literal . $id:ident $counter_doc:literal $counter:ident) => {
        index! {$doc $id $counter_doc $counter}
        impl $id {
            /// Root.
            pub fn root() -> Self {
                Self(0)
            }
            /// Check if is root.
            pub fn is_root(&self) -> bool {
                self.0 == 0
            }
        }
    };
}

index! {
    "ID of a RPL pattern file."
    RPLIdx
    "Counter for [RPLID]."
    RPLCounter
}

/// A counter that supports only incrementation.
#[derive(Clone, Debug)]
pub struct Counter<T> {
    count: usize,
    _phantom: PhantomData<T>,
}
impl<T: Idx> Counter<T> {
    /// Return a new number and increment the counter.
    pub fn get(&mut self) -> T {
        let id = self.count;
        self.count += 1;
        T::new(id)
    }
    /// Get the total count of emitted indices.
    pub fn total(&self) -> usize {
        self.count
    }
}
impl<T: Idx> Default for Counter<T> {
    fn default() -> Self {
        Self {
            count: 0,
            _phantom: PhantomData,
        }
    }
}
