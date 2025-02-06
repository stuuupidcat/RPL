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

extern crate rustc_span;

use std::io::stderr;

use rpl_meta_pest::cli::collect_file_cli;
use rpl_meta_pest::context::RPLMetaContext;
use rpl_meta_pest::meta::RPLMeta;

fn main() {
    rustc_span::create_session_if_not_set_then(rustc_span::edition::LATEST_STABLE_EDITION, |_| {
        let mut mctx = RPLMetaContext::default();
        let vec = collect_file_cli();
        for (buf, path) in vec {
            let meta = RPLMeta::parse_and_collect(path, buf, &mut mctx);
            match meta {
                Ok(meta) => {
                    meta.show_error(&mut stderr());
                    mctx.add_meta(meta.idx, meta);
                },
                Err(meta) => {
                    // display error
                    eprintln!("{}", meta);
                },
            }
        }
    });
}
