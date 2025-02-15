#![allow(internal_features)]
#![feature(rustc_private)]
#![feature(rustc_attrs)]
#![feature(let_chains)]
#![feature(if_let_guard)]
#![feature(box_patterns)]
#![feature(try_trait_v2)]
#![feature(debug_closure_helpers)]
#![feature(iter_chain)]
#![feature(iterator_try_collect)]
#![feature(cell_update)]

extern crate rustc_data_structures;
extern crate rustc_span;

use rpl_meta_pest::cli::collect_file_from_args_for_test;
use rustc_data_structures::sync::{Registry, WorkerLocal};
use std::num::NonZero;

fn main() {
    // Only for testing purposes
    Registry::new(NonZero::new(1).unwrap()).register();
    rustc_span::create_session_if_not_set_then(rustc_span::edition::LATEST_STABLE_EDITION, |_| {
        let mctx_arena = WorkerLocal::<rpl_meta_pest::arena::Arena<'_>>::default();
        let _mctx = rpl_meta_pest::parse_and_collect(&mctx_arena, &collect_file_from_args_for_test());
    });
}
