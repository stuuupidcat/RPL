use rpl_mir::pat;
use rpl_mir::pat::visitor::{PatternVisitor, PlaceContext};
use rustc_data_structures::packed::Pu128;
use rustc_middle::ty;

use crate::graph::{BlockDataDepGraph, ControlFlowGraph, DataDepGraph, SwitchTargets, TerminatorEdges};

pub type PatDataDepGraph = DataDepGraph<pat::BasicBlock, pat::LocalIdx>;
pub type PatControlFlowGraph<'a> = ControlFlowGraph<'a, pat::BasicBlock>;
pub type PatTerminatorEdges<'a> = TerminatorEdges<'a, pat::BasicBlock>;
pub type PatSwitchTargets = SwitchTargets<pat::BasicBlock>;

impl PatDataDepGraph {
    pub fn from_patterns(patterns: &pat::Patterns<'_>) -> Self {
        let mut this = Self::new(
            patterns.basic_blocks.len(),
            |bb| patterns.basic_blocks[bb].num_statements_and_terminator(),
            patterns.num_locals(),
        );
        for (bb, block) in patterns.basic_blocks.iter_enumerated() {
            this.blocks[bb].visit_basic_block_data(bb, block);
        }
        this
    }
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

impl<'a> PatControlFlowGraph<'a> {
    pub fn from_patterns(patterns: &'a pat::Patterns<'_>, pointer_bytes: usize) -> Self {
        Self {
            terminator_edges: patterns
                .basic_blocks
                .iter()
                .map(|block| PatTerminatorEdges::from_normalized(block.terminator.as_ref(), pointer_bytes))
                .collect(),
        }
    }
}

impl<'a> PatTerminatorEdges<'a> {
    fn from_normalized(termiantor: Option<&'a pat::TerminatorKind<'_>>, pointer_bytes: usize) -> Self {
        use pat::TerminatorKind::{Call, Drop, Goto, PatEnd, Return, SwitchInt};
        match termiantor {
            None | Some(Return | PatEnd) => TerminatorEdges::None,
            Some(&Goto(target) | &Call { target, .. } | &Drop { target, .. }) => TerminatorEdges::Single(target),
            Some(SwitchInt { targets, .. }) => {
                TerminatorEdges::SwitchInt(PatSwitchTargets::from_normalized(targets, pointer_bytes))
            },
        }
    }
}

impl PatSwitchTargets {
    fn from_normalized(targets: &pat::SwitchTargets, pointer_bytes: usize) -> Self {
        use pat::IntTy::{Bool, Int, NegInt, Uint};
        use ty::IntTy::{Isize, I128, I16, I32, I64, I8};
        Self {
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
}
