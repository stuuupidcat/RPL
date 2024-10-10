use rustc_data_structures::packed::Pu128;
use rustc_middle::mir;
use rustc_middle::mir::visit::{PlaceContext, Visitor};

use crate::graph::{BlockDataDepGraph, ControlFlowGraph, DataDepGraph, SwitchTargets, TerminatorEdges};

pub type MirDataDepGraph = DataDepGraph<mir::BasicBlock, mir::Local>;
pub type MirControlFlowGraph<'a> = ControlFlowGraph<'a, mir::BasicBlock>;
pub type MirTerminatorEdges<'a> = TerminatorEdges<'a, mir::BasicBlock>;
pub type MirSwitchTargets = SwitchTargets<mir::BasicBlock>;

impl MirDataDepGraph {
    pub fn from_mir_body(body: &mir::Body<'_>) -> Self {
        let mut this = Self::new(
            body.basic_blocks.len(),
            |bb| body.basic_blocks[bb].statements.len() + 1,
            body.local_decls.len(),
        );
        for (bb, block) in body.basic_blocks.iter_enumerated() {
            this.blocks[bb].visit_basic_block_data(bb, block);
        }
        this
    }
}

impl<'tcx> Visitor<'tcx> for BlockDataDepGraph<mir::Local> {
    fn visit_local(&mut self, local: mir::Local, pcx: PlaceContext, location: mir::Location) {
        self.access_local(local, pcx, location.statement_index);
    }
    fn visit_statement(&mut self, statement: &mir::Statement<'tcx>, location: mir::Location) {
        self.super_statement(statement, location);
        self.update_deps(location.statement_index);
    }
    fn visit_terminator(&mut self, terminator: &mir::Terminator<'tcx>, location: mir::Location) {
        self.super_terminator(terminator, location);
        self.update_deps(location.statement_index);
        self.update_dep_end();
    }
}

impl<'a> MirControlFlowGraph<'a> {
    pub fn from_mir_body(body: &'a mir::Body<'_>) -> Self {
        Self {
            terminator_edges: body
                .basic_blocks
                .iter()
                .map(|block| MirTerminatorEdges::from(&block.terminator().kind))
                .collect(),
        }
    }
}

impl<'a> MirTerminatorEdges<'a> {
    fn from(termiantor: &'a mir::TerminatorKind<'_>) -> Self {
        match termiantor.edges() {
            mir::TerminatorEdges::None => TerminatorEdges::None,
            mir::TerminatorEdges::Single(bb) => TerminatorEdges::Single(bb),
            mir::TerminatorEdges::Double(bb0, bb1) => TerminatorEdges::Double(bb0, bb1),
            mir::TerminatorEdges::AssignOnReturn { return_, cleanup, .. } => {
                TerminatorEdges::AssignOnReturn { return_, cleanup }
            },
            mir::TerminatorEdges::SwitchInt { targets, .. } => {
                TerminatorEdges::SwitchInt(MirSwitchTargets::from(targets))
            },
        }
    }
}

impl MirSwitchTargets {
    fn from(targets: &mir::SwitchTargets) -> Self {
        Self {
            targets: targets.iter().map(|(value, bb)| (Pu128(value), bb)).collect(),
            otherwise: Some(targets.otherwise()),
        }
    }
}
