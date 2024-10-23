use crate::pat;
use crate::pat::visitor::{PatternVisitor, PlaceContext};
use rustc_data_structures::packed::Pu128;
use rustc_middle::ty;

use rpl_mir_graph::{
    BlockDataDepGraph, ControlFlowGraph, DataDepGraph, ProgramDepGraph, SwitchTargets, TerminatorEdges,
};

pub type PatProgramDepGraph = ProgramDepGraph<pat::BasicBlock, pat::LocalIdx>;
pub type PatDataDepGraph = DataDepGraph<pat::BasicBlock, pat::LocalIdx>;
pub type PatControlFlowGraph = ControlFlowGraph<pat::BasicBlock>;
pub type PatSwitchTargets = SwitchTargets<pat::BasicBlock>;
type PatTerminatorEdges = TerminatorEdges<pat::BasicBlock>;

pub fn pat_program_dep_graph(patterns: &pat::Patterns<'_>, pointer_bytes: usize) -> PatProgramDepGraph {
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

pub fn pat_control_fow_graph(patterns: &pat::Patterns<'_>, pointer_bytes: usize) -> PatControlFlowGraph {
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

fn normalized_terminator_edges(
    termiantor: Option<&pat::TerminatorKind<'_>>,
    pointer_bytes: usize,
) -> PatTerminatorEdges {
    use pat::TerminatorKind::{Call, Drop, Goto, PatEnd, Return, SwitchInt};
    match termiantor {
        None | Some(Return | PatEnd) => TerminatorEdges::None,
        Some(&Goto(target) | &Call { target, .. } | &Drop { target, .. }) => TerminatorEdges::Single(target),
        Some(SwitchInt { targets, .. }) => {
            TerminatorEdges::SwitchInt(pat_normalized_switch_targets(targets, pointer_bytes))
        },
    }
}

pub fn pat_normalized_switch_targets(targets: &pat::SwitchTargets, pointer_bytes: usize) -> PatSwitchTargets {
    use pat::IntTy::{Bool, Int, NegInt, Uint};
    use ty::IntTy::{Isize, I128, I16, I32, I64, I8};
    PatSwitchTargets {
        targets: targets
            .targets
            .iter()
            .map(|(&value, &bb)| {
                let pat::IntValue { ty, value } = value;
                let value = match ty {
                    NegInt(I8) => Pu128(value.get() ^ u128::from(u8::MAX)),
                    NegInt(I16) => Pu128(value.get() ^ u128::from(u16::MAX)),
                    NegInt(I32) => Pu128(value.get() ^ u128::from(u32::MAX)),
                    NegInt(I64) => Pu128(value.get() ^ u128::from(u64::MAX)),
                    NegInt(I128) => Pu128(value.get() ^ u128::MAX),
                    NegInt(Isize) => {
                        let pointer_mask = match pointer_bytes {
                            2 => u128::from(u16::MAX),
                            4 => u128::from(u32::MAX),
                            8 => u128::from(u64::MAX),
                            _ => panic!("unsupported pointer size: {pointer_bytes}"),
                        };
                        Pu128(value.get() ^ pointer_mask)
                    },
                    Int(_) | Uint(_) | Bool => value,
                };
                (value, bb)
            })
            .collect(),
        otherwise: targets.otherwise,
    }
}
