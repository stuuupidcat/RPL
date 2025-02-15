use parser::pairs;
use rustc_arena::declare_arena;

declare_arena!(
    [
        [] syntax_tree: pairs::main<'tcx>,
    ]
);
