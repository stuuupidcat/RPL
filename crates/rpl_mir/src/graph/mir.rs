use rustc_data_structures::packed::Pu128;
use rustc_middle::mir;
use rustc_middle::mir::visit::Visitor;

use rpl_mir_graph::{ControlFlowGraph, DataDepGraph, ProgramDepGraph, SwitchTargets, TerminatorEdges};

pub type MirProgramDepGraph = ProgramDepGraph<mir::BasicBlock, mir::Local>;
pub type MirDataDepGraph = DataDepGraph<mir::BasicBlock, mir::Local>;
pub type MirControlFlowGraph = ControlFlowGraph<mir::BasicBlock>;
pub type MirSwitchTargets = SwitchTargets<mir::BasicBlock>;
type MirTerminatorEdges = TerminatorEdges<mir::BasicBlock>;

pub fn mir_program_dep_graph(body: &mir::Body<'_>) -> MirProgramDepGraph {
    let cfg = mir_control_flow_graph(body);
    ProgramDepGraph::build_from(&cfg, &mir_data_dep_graph(body, &cfg))
}

pub fn mir_data_dep_graph(body: &mir::Body<'_>, cfg: &MirControlFlowGraph) -> MirDataDepGraph {
    let mut graph = DataDepGraph::new(
        body.basic_blocks.len(),
        |bb| body.basic_blocks[bb].statements.len() + 1,
        body.local_decls.len(),
    );
    for (bb, block) in body.basic_blocks.iter_enumerated() {
        graph.blocks[bb].visit_basic_block_data(bb, block);
    }
    graph.build_interblock_edges(cfg);
    graph
}

pub fn mir_control_flow_graph(body: &mir::Body<'_>) -> MirControlFlowGraph {
    ControlFlowGraph::new(body.basic_blocks.len(), |block| {
        terminator_edges(&body[block].terminator().kind)
    })
}

fn terminator_edges(termiantor: &mir::TerminatorKind<'_>) -> MirTerminatorEdges {
    match termiantor.edges() {
        mir::TerminatorEdges::None => TerminatorEdges::None,
        mir::TerminatorEdges::Single(bb) => TerminatorEdges::Single(bb),
        mir::TerminatorEdges::Double(bb0, bb1) => TerminatorEdges::Double(bb0, bb1),
        mir::TerminatorEdges::AssignOnReturn { return_, cleanup, .. } => TerminatorEdges::AssignOnReturn {
            return_: return_.into(),
            cleanup,
        },
        mir::TerminatorEdges::SwitchInt { targets, .. } => TerminatorEdges::SwitchInt(mir_switch_targets(targets)),
    }
}

pub fn mir_switch_targets(targets: &mir::SwitchTargets) -> MirSwitchTargets {
    MirSwitchTargets {
        targets: targets.iter().map(|(value, bb)| (Pu128(value), bb)).collect(),
        otherwise: Some(targets.otherwise()),
    }
}
