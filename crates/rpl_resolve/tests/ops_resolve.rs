//! Resolver well-formedness checks for `ops { ... }` blocks.
//!
//! Tests R1, R2, and R3 — the checks that guard `OpsMetaLookup`'s
//! `unreachable!()` contracts against malformed input.
//!
//! These tests delegate to `rpl_context::pat::check_ops_block`, which is the
//! pre-lowering validator.  The `rpl_resolve` crate itself handles Rust
//! def-path resolution (path → `DefId`); the ops well-formedness rules are
//! pattern-AST concerns that live in `rpl_context`.  This file keeps the
//! specified test locations while delegating to the correct crate.

#![allow(internal_features)]
#![feature(rustc_private)]
#![feature(rustc_attrs)]
#![feature(let_chains)]
#![feature(box_patterns)]
#![feature(debug_closure_helpers)]
#![recursion_limit = "256"]

extern crate rustc_data_structures;
extern crate rustc_span;

use std::path::PathBuf;

use rpl_context::pat::check_ops_block;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Parse `src`, run `check_ops_block` on every `opsBlock` found, and return
/// all error messages as display strings.
fn run_resolver_on_src(src: &str) -> Vec<String> {
    let arena: &'static rpl_meta::arena::Arena<'static> =
        Box::leak(Box::new(rpl_meta::arena::Arena::default()));
    let path_and_content: &'static Vec<(PathBuf, String)> =
        Box::leak(Box::new(vec![(PathBuf::from("test.rpl"), src.to_string())]));

    let mctx: &'static rpl_meta::context::MetaContext<'static> = Box::leak(Box::new(
        rpl_meta::parse_and_collect(arena, path_and_content, |err| {
            panic!("RPL parse/collect error: {err}");
        }),
    ));

    let mut errors = Vec::new();
    for syntax_tree in mctx.syntax_trees.iter() {
        let (_, _, ops, _) = rpl_meta::meta::collect_blocks(syntax_tree);
        for ops_block in ops {
            let errs = check_ops_block(ops_block);
            errors.extend(errs.into_iter().map(|e| e.to_string()));
        }
    }
    errors
}

// ---------------------------------------------------------------------------
// R1: op-level meta-vars must be of kind `type`
// ---------------------------------------------------------------------------

#[test]
fn r1_non_type_op_meta_var_is_rejected() {
    let src = r#"
pattern test
ops {
    sync[$N: const(usize)] = { fn $lock() -> _; }
}
patt { p[] = fn _ () -> _ {} }
"#;
    let errs = run_resolver_on_src(src);
    assert!(
        errs.iter().any(|e| e.contains("must be of kind 'type'")),
        "R1: expected an error about non-type op-level meta-var, got: {errs:?}"
    );
}

// ---------------------------------------------------------------------------
// R2: op signatures may only reference meta-vars from the same group
// ---------------------------------------------------------------------------

#[test]
fn r2_op_signature_uses_undeclared_meta_var() {
    let src = r#"
pattern test
ops {
    sync[$T: type] = { fn $lock(&mut $Z) -> _; }
}
patt { p[] = fn _ () -> _ {} }
"#;
    let errs = run_resolver_on_src(src);
    assert!(
        errs.iter().any(|e| e.contains("'$Z' is not declared in op group 'sync'")),
        "R2: expected undeclared-meta-var error, got: {errs:?}"
    );
}

// ---------------------------------------------------------------------------
// R3: op signatures must not contain concrete Rust paths/types
// ---------------------------------------------------------------------------

#[test]
fn r3_op_signature_with_concrete_path_is_rejected() {
    let src = r#"
pattern test
ops {
    sync[$T: type] = { fn $lock(&mut std::sync::Mutex<$T>) -> _; }
}
patt { p[] = fn _ () -> _ {} }
"#;
    let errs = run_resolver_on_src(src);
    assert!(
        errs.iter().any(|e| e.contains("concrete types belong in rpl.toml")),
        "R3: expected concrete-path error, got: {errs:?}"
    );
}
