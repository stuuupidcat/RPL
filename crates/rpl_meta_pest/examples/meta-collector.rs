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

use rpl_meta_pest::context::RPLMetaContext;
use rpl_meta_pest::meta::RPLMeta;
use rpl_utils::cli::cli;
use rpl_utils::unwrap;

fn main() {
    cli(|input, path| {
        let mut mctx = RPLMetaContext::default();
        let x = unwrap!(RPLMeta::collect_from_path(path.into(), input.into(), &mut mctx));
        "Succeed".to_string()
    });
}
