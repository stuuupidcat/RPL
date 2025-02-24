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

use rustc_data_structures::sync::{Registry, WorkerLocal};
use std::num::NonZero;

use rpl_meta_pest::cli::{read_file_from_path_buf, traverse_rpl};
use rpl_meta_pest::RPLMetaError;
use std::path::PathBuf;
use std::sync::Arc;

pub fn collect_file_from_args_for_test() -> Vec<(PathBuf, String)> {
    let args = std::env::args();
    let args = args.skip(1);
    if args.len() == 0 {
        eprintln!("Usage: cargo run --package rpl_meta_pest --example meta-collector <file1> <file2> ...");
        vec![]
    } else {
        let mut res = vec![];
        for arg in args {
            traverse_rpl(arg.into(), |path| {
                let buf = read_file_from_path_buf(&path);
                let buf = match buf {
                    Ok(buf) => buf,
                    Err(err) => {
                        eprintln!(
                            "{}",
                            RPLMetaError::FileError {
                                path,
                                error: Arc::new(err)
                            }
                        );
                        return;
                    },
                };
                res.push((path, buf));
            });
        }
        res
    }
}

fn main() {
    // Only for testing purposes
    Registry::new(NonZero::new(1).unwrap()).register();
    rustc_span::create_session_if_not_set_then(rustc_span::edition::LATEST_STABLE_EDITION, |_| {
        let mctx_arena = WorkerLocal::<rpl_meta_pest::arena::Arena<'_>>::default();
        let _mctx = rpl_meta_pest::parse_and_collect(&mctx_arena, &collect_file_from_args_for_test());
    });
}
