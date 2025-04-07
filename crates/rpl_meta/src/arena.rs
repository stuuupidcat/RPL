use parser::pairs;
use std::path::PathBuf;
use sync_arena::declare_arena;

declare_arena!(
    [
        [] syntax_tree: pairs::main<'tcx>,
        [] path_and_content: Vec<(PathBuf, String)>,
    ]
);
