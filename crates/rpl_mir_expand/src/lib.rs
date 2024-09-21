#![feature(rustc_private)]
#![feature(map_try_insert)]
#![feature(box_patterns)]
#![feature(if_let_guard)]
#![feature(impl_trait_in_fn_trait_return)]
#![feature(let_chains)]

extern crate rpl_mir_syntax as syntax;

extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_hash;
extern crate rustc_index;

mod check;
pub(crate) mod expand;
mod parse;
pub(crate) mod symbol_table;

#[cfg(test)]
mod tests;

pub(crate) use check::check_mir;
pub use expand::{expand, expand_mir};
#[cfg(test)]
pub(crate) use expand::{expand_impl, Expand};
pub use parse::MirPatternFn;
pub(crate) use symbol_table::is_primitive;
pub use symbol_table::SymbolTable;