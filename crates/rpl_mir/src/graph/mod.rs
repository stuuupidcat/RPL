use rpl_mir_graph::BlockDataDepGraph;
use rustc_index::Idx;

mod mir;
mod pat;

pub use mir::{
    MirControlFlowGraph, MirDataDepGraph, MirProgramDepGraph, MirSwitchTargets, mir_control_flow_graph,
    mir_data_dep_graph, mir_program_dep_graph, mir_switch_targets,
};
pub use pat::{
    PatControlFlowGraph, PatDataDepGraph, PatProgramDepGraph, PatSwitchTargets, normalized_terminator_edges,
    pat_control_flow_graph, pat_data_dep_graph, pat_normalized_switch_targets, pat_program_dep_graph,
};

struct BlockDataDepGraphVisitor<'a, Local: Idx> {
    graph: &'a mut BlockDataDepGraph<Local>,
}

impl<'a, Local: Idx> BlockDataDepGraphVisitor<'a, Local> {
    fn new(graph: &'a mut BlockDataDepGraph<Local>) -> Self {
        Self { graph }
    }
}
