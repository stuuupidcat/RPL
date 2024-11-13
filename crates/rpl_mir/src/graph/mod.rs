mod mir;
mod pat;

pub use mir::{
    mir_control_flow_graph, mir_data_dep_graph, mir_program_dep_graph, mir_switch_targets, MirControlFlowGraph,
    MirDataDepGraph, MirProgramDepGraph, MirSwitchTargets,
};
pub use pat::{
    normalized_terminator_edges, pat_control_flow_graph, pat_data_dep_graph, pat_normalized_switch_targets,
    pat_program_dep_graph, PatControlFlowGraph, PatDataDepGraph, PatProgramDepGraph, PatSwitchTargets,
};
