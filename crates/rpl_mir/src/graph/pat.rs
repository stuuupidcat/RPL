use crate::pat;
use crate::pat::visitor::{PatternVisitor, PlaceContext};

use rpl_mir_graph::{
    BlockDataDepGraph, ControlFlowGraph, DataDepGraph, ProgramDepGraph, SwitchTargets, TerminatorEdges,
};

pub type PatProgramDepGraph = ProgramDepGraph<pat::BasicBlock, pat::LocalIdx>;
pub type PatDataDepGraph = DataDepGraph<pat::BasicBlock, pat::LocalIdx>;
pub type PatControlFlowGraph = ControlFlowGraph<pat::BasicBlock>;
pub type PatSwitchTargets = SwitchTargets<pat::BasicBlock>;
type PatTerminatorEdges = TerminatorEdges<pat::BasicBlock>;

pub fn pat_program_dep_graph(patterns: &pat::Patterns<'_>, pointer_bytes: u64) -> PatProgramDepGraph {
    ProgramDepGraph::build_from(
        &pat_control_fow_graph(patterns, pointer_bytes),
        &pat_data_dep_graph(patterns),
    )
}

pub fn pat_data_dep_graph(patterns: &pat::Patterns<'_>) -> PatDataDepGraph {
    let mut graph = DataDepGraph::new(
        patterns.basic_blocks.len(),
        |bb| patterns[bb].num_statements_and_terminator(),
        patterns.locals.len(),
    );
    for (bb, block) in patterns.basic_blocks.iter_enumerated() {
        graph.blocks[bb].visit_basic_block_data(bb, block);
    }
    graph
}

pub fn pat_control_fow_graph(patterns: &pat::Patterns<'_>, pointer_bytes: u64) -> PatControlFlowGraph {
    ControlFlowGraph::new(patterns.basic_blocks.len(), |block| {
        normalized_terminator_edges(patterns[block].terminator.as_ref(), pointer_bytes)
    })
}

impl<'tcx> PatternVisitor<'tcx> for BlockDataDepGraph<pat::LocalIdx> {
    fn visit_local(&mut self, local: pat::LocalIdx, pcx: PlaceContext, location: pat::Location) {
        self.access_local(local, pcx, location.statement_index);
    }
    fn visit_statement(&mut self, statement: &pat::StatementKind<'tcx>, location: pat::Location) {
        self.super_statement(statement, location);
        self.update_deps(location.statement_index);
    }
    fn visit_terminator(&mut self, terminator: &pat::TerminatorKind<'tcx>, location: pat::Location) {
        self.super_terminator(terminator, location);
        self.update_deps(location.statement_index);
        self.update_dep_end();
    }
}

pub fn normalized_terminator_edges(terminator: Option<&pat::TerminatorKind<'_>>, pointer_bytes: u64) -> PatTerminatorEdges {
    use pat::TerminatorKind::{Call, Drop, Goto, PatEnd, Return, SwitchInt};
    match terminator {
        None | Some(Return | PatEnd) => TerminatorEdges::None,
        Some(&Goto(target) | &Drop { target, .. }) => TerminatorEdges::Single(target),
        Some(&Call { target, .. }) => TerminatorEdges::AssignOnReturn {
            return_: Box::new([target]),
            cleanup: None,
        },
        Some(SwitchInt { targets, .. }) => {
            TerminatorEdges::SwitchInt(pat_normalized_switch_targets(targets, pointer_bytes))
        },
    }
}

pub fn pat_normalized_switch_targets(targets: &pat::SwitchTargets, pointer_bytes: u64) -> PatSwitchTargets {
    PatSwitchTargets {
        targets: targets
            .targets
            .iter()
            .map(|(&value, &bb)| (value.normalize(pointer_bytes), bb))
            .collect(),
        otherwise: targets.otherwise,
    }
}
