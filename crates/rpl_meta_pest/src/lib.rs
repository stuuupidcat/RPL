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
extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_span;

pub mod check;
pub mod cli;
pub mod context;
pub mod error;
pub mod idx;
pub mod meta;
pub mod symbol_table;
pub(crate) mod utils;

use context::RPLMetaContext;
pub use error::RPLMetaError;
use meta::RPLMeta;

pub fn parse<'mctx>() -> RPLMetaContext<'mctx> {
    let mut mctx = RPLMetaContext::default();
    // FIXME: cli arguments should be compatible with rustc
    // let vec = collect_file_cli();
    let vec = Vec::new();
    for (buf, path) in vec {
        let meta = RPLMeta::parse_and_collect(path, buf, &mut mctx);
        match meta {
            Ok(meta) => {
                meta.show_error(&mut std::io::stderr());
                mctx.add_meta(meta.idx, meta);
            },
            Err(err) => {
                // display error
                eprintln!("{}", err);
                std::process::exit(err.get_number().parse().unwrap_or(-1));
            },
        }
    }
    mctx
}
