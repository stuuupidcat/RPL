#![feature(rustc_private)]
#![feature(map_try_insert)]
#![feature(box_patterns)]
#![feature(if_let_guard)]
#![feature(impl_trait_in_fn_trait_return)]
#![feature(let_chains)]
#![feature(macro_metavar_expr_concat)]

extern crate rpl_parser as parser;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_hash;

pub mod error;
pub mod symbol_table;
