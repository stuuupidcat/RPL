#![allow(rustc::usage_of_ty_tykind)]

// `'tcx` instead of `'pcx` used here because of the `rustc_arena::declare_arena` macro
#[macro_export]
macro_rules! arena_types {
    ($macro:path) => (
        $macro!([
            [] patterns: $crate::pat::Pattern<'tcx>,
            [] mir_patterns: $crate::pat::MirPattern<'tcx>,
        ]);
    )
}

arena_types!(rustc_arena::declare_arena);
