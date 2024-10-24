use std::ops::Index;

use crate::rwstate::RWStates;
use rustc_data_structures::fx::FxIndexMap;
use rustc_data_structures::packed::Pu128;
use rustc_index::bit_set::{HybridBitSet, SparseBitMatrix};
use rustc_index::{Idx, IndexVec};
use rustc_middle::mir::visit::{MutatingUseContext, NonMutatingUseContext, PlaceContext};

rustc_index::newtype_index! {
    #[debug_format = "Node{}"]
    #[orderable]
    pub struct NodeIdx {}
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind<BasicBlock, Local> {
    StmtOrTerm(BasicBlock, usize),
    BlockEnter(BasicBlock),
    BlockExit(BasicBlock),
    Local(Local),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EdgeKind {
    /// There is a goto edge from the `from` block to the `to` block.
    Goto,
    /// There is a goto edge from the `from` block to the `to` block when it unwinds.
    UnwindGoto,
    /// There is a reversed data dependency from the `from` statement to the `to` statement.
    ///
    /// This means that the `from` statement must be executed before the `to` statement.
    DataRdep,
    /// There is an access to a local variable, by given access pattern and order.
    Access(usize, PlaceContext),
    /// There is a switch jump with the given value.
    SwitchInt(Pu128),
    /// There is a switch jump of the otherwise branch.
    Otherwise,
}

pub struct ProgramDepGraph<BasicBlock: Idx, Local: Idx> {
    nodes: IndexVec<NodeIdx, NodeKind<BasicBlock, Local>>,
    edges: Vec<Edge>,
    locals_offset: NodeIdx,
    blocks_offset: NodeIdx,
    stmts_offsets: IndexVec<BasicBlock, NodeIdx>,
}

impl<BasicBlock: Idx, Local: Idx> Index<NodeIdx> for ProgramDepGraph<BasicBlock, Local> {
    type Output = NodeKind<BasicBlock, Local>;

    fn index(&self, node: NodeIdx) -> &Self::Output {
        &self.nodes[node]
    }
}

pub struct Edge {
    pub from: NodeIdx,
    pub to: NodeIdx,
    pub kind: EdgeKind,
}

impl Edge {
    fn new(from: NodeIdx, to: NodeIdx, kind: EdgeKind) -> Self {
        Self { from, to, kind }
    }
    pub fn nodes(&self) -> (NodeIdx, NodeIdx) {
        (self.from, self.to)
    }
}

impl<BasicBlock: Idx, Local: Idx> ProgramDepGraph<BasicBlock, Local> {
    pub fn num_nodes(&self) -> usize {
        self.nodes.len()
    }
    pub fn num_edges(&self) -> usize {
        self.edges.len()
    }
    pub fn nodes(&self) -> impl Iterator<Item = (NodeIdx, NodeKind<BasicBlock, Local>)> + '_ {
        self.nodes.iter_enumerated().map(|(idx, &node)| (idx, node))
    }
    pub fn edges(&self) -> impl Iterator<Item = &Edge> + '_ {
        self.edges.iter()
    }
    pub fn find_edge(&self, from: NodeIdx, to: NodeIdx) -> Option<&Edge> {
        Some(&self.edges[self.search_edge(from, to).ok()?])
    }
    pub fn edges_from(&self, from: NodeIdx) -> &[Edge] {
        &self.edges[self.search_edges_for_node(from)]
    }
    pub fn block_nodes(&self, bb: BasicBlock) -> [NodeIdx; 2] {
        let start = NodeIdx::plus(self.blocks_offset, bb.index() * 2);
        let end = start.plus(1);
        [start, end]
    }
    pub fn local_node(&self, local: Local) -> NodeIdx {
        NodeIdx::plus(self.locals_offset, local.index())
    }
    pub fn stmt_node(&self, bb: BasicBlock, statement_index: usize) -> NodeIdx {
        NodeIdx::plus(self.stmts_offsets[bb], statement_index)
    }

    pub fn build_from(cfg: &ControlFlowGraph<BasicBlock>, ddg: &DataDepGraph<BasicBlock, Local>) -> Self {
        ProgramDepGraphBuilder::new(cfg, ddg).build()
    }
    fn search_edge(&self, from: NodeIdx, to: NodeIdx) -> Result<usize, usize> {
        self.edges.binary_search_by_key(&(from, to), Edge::nodes)
    }
    fn search_edges_for_node(&self, from: NodeIdx) -> std::ops::Range<usize> {
        let start = self
            .search_edge(from, NodeIdx::ZERO)
            .unwrap_or_else(std::convert::identity);
        let end = self
            .search_edge(from.plus(1), NodeIdx::ZERO)
            .unwrap_or_else(std::convert::identity);
        start..end
    }
    fn add_locals(&mut self, num_locals: usize) -> impl Copy + Fn(Local) -> NodeIdx {
        self.locals_offset = self.nodes.next_index();
        let offset = self.locals_offset;
        self.nodes.extend((0..num_locals).map(Local::new).map(NodeKind::Local));
        move |local| NodeIdx::plus(offset, local.index())
    }
    fn add_block_starts_and_ends(
        &mut self,
        num_blocks: usize,
    ) -> (
        impl Copy + Fn(BasicBlock) -> NodeIdx,
        impl Copy + Fn(BasicBlock) -> NodeIdx,
    ) {
        self.blocks_offset = self.nodes.next_index();
        let offset = self.blocks_offset;
        self.nodes.extend(
            (0..num_blocks)
                .map(BasicBlock::new)
                .flat_map(|bb| [NodeKind::BlockEnter(bb), NodeKind::BlockExit(bb)]),
        );
        let starts = move |bb: BasicBlock| NodeIdx::plus(offset, bb.index() * 2);
        let ends = move |bb: BasicBlock| NodeIdx::plus(starts(bb), 1);
        (starts, ends)
    }
    fn add_statements_and_terminator(
        &mut self,
        bb: BasicBlock,
        block: &BlockDataDepGraph<Local>,
    ) -> impl Copy + Fn(usize) -> NodeIdx {
        self.stmts_offsets[bb] = self.nodes.next_index();
        let offset = self.stmts_offsets[bb];
        self.nodes
            .extend((0..block.num_statements()).map(|stmt| NodeKind::StmtOrTerm(bb, stmt)));
        move |stmt| NodeIdx::plus(offset, stmt)
    }
    fn add_accesses(
        &mut self,
        block: &BlockDataDepGraph<Local>,
        stmts: impl Copy + Fn(usize) -> NodeIdx,
        locals: impl Copy + Fn(Local) -> NodeIdx,
    ) {
        self.edges
            .extend(block.accesses.iter().enumerate().flat_map(|(stmt, accesses)| {
                accesses.iter().enumerate().map(move |(idx, &(local, access))| {
                    Edge::new(stmts(stmt), locals(local), EdgeKind::Access(idx, access))
                })
            }));
    }

    fn add_deps(
        &mut self,
        block: &BlockDataDepGraph<Local>,
        stmts: impl Copy + Fn(usize) -> NodeIdx,
        start: NodeIdx,
        end: NodeIdx,
    ) {
        self.edges.extend((0..block.num_statements()).flat_map(|stmt| {
            block
                .deps(stmt)
                .map(move |dep| Edge::new(stmts(dep), stmts(stmt), EdgeKind::DataRdep))
        }));
        self.edges.extend(
            block
                .rdep_start()
                .map(|rdep| Edge::new(start, stmts(rdep), EdgeKind::DataRdep)),
        );
        self.edges.extend(
            block
                .dep_end()
                .map(|dep| Edge::new(stmts(dep), end, EdgeKind::DataRdep)),
        );
    }

    fn add_cfg_edges(
        &mut self,
        bb: BasicBlock,
        terminator_edges: &TerminatorEdges<BasicBlock>,
        starts: impl Copy + Fn(BasicBlock) -> NodeIdx,
        ends: impl Copy + Fn(BasicBlock) -> NodeIdx,
    ) {
        match terminator_edges {
            TerminatorEdges::None => {},
            &TerminatorEdges::Single(target) => {
                self.edges.push(Edge::new(ends(bb), starts(target), EdgeKind::Goto));
            },
            &TerminatorEdges::Double(target, unwind) => {
                self.edges.push(Edge::new(ends(bb), starts(target), EdgeKind::Goto));
                self.edges
                    .push(Edge::new(ends(bb), starts(unwind), EdgeKind::UnwindGoto));
            },
            TerminatorEdges::AssignOnReturn { return_, cleanup } => {
                self.edges.extend(
                    return_
                        .iter()
                        .map(|&ret| Edge::new(ends(bb), starts(ret), EdgeKind::Goto)),
                );
                if let &Some(cleanup) = cleanup {
                    self.edges
                        .push(Edge::new(ends(bb), starts(cleanup), EdgeKind::UnwindGoto));
                }
            },
            TerminatorEdges::SwitchInt(switch_targets) => {
                self.edges.extend(
                    switch_targets
                        .targets
                        .iter()
                        .map(|(&value, &next)| Edge::new(ends(bb), starts(next), EdgeKind::SwitchInt(value))),
                );
                if let Some(otherwise) = switch_targets.otherwise {
                    self.edges
                        .push(Edge::new(ends(bb), starts(otherwise), EdgeKind::Otherwise));
                }
            },
        }
    }
}

struct ProgramDepGraphBuilder<'a, BasicBlock: Idx, Local: Idx> {
    cfg: &'a ControlFlowGraph<BasicBlock>,
    ddg: &'a DataDepGraph<BasicBlock, Local>,
    graph: ProgramDepGraph<BasicBlock, Local>,
}

impl<'a, BasicBlock: Idx, Local: Idx> ProgramDepGraphBuilder<'a, BasicBlock, Local> {
    fn new(cfg: &'a ControlFlowGraph<BasicBlock>, ddg: &'a DataDepGraph<BasicBlock, Local>) -> Self {
        assert_eq!(cfg.num_blocks(), ddg.num_blocks(), "Mismatched number of blocks");
        Self {
            cfg,
            ddg,
            graph: ProgramDepGraph {
                nodes: IndexVec::new(),
                edges: Vec::new(),
                locals_offset: NodeIdx::ZERO,
                blocks_offset: NodeIdx::ZERO,
                stmts_offsets: IndexVec::from_elem_n(NodeIdx::ZERO, cfg.num_blocks()),
            },
        }
    }
    fn build(mut self) -> ProgramDepGraph<BasicBlock, Local> {
        let locals = self.graph.add_locals(self.ddg.num_locals());
        let (starts, ends) = self.graph.add_block_starts_and_ends(self.ddg.num_blocks());
        let graph = &mut self.graph;
        for (bb, block) in self.ddg.blocks() {
            let stmts = graph.add_statements_and_terminator(bb, block);
            graph.add_accesses(block, stmts, locals);
            graph.add_deps(block, stmts, starts(bb), ends(bb));
            graph.add_cfg_edges(bb, &self.cfg[bb], starts, ends);
        }
        graph.edges.sort_unstable_by_key(Edge::nodes);
        self.graph
    }
}

pub struct ControlFlowGraph<BasicBlock: Idx> {
    pub(crate) terminator_edges: IndexVec<BasicBlock, TerminatorEdges<BasicBlock>>,
}

impl<BasicBlock: Idx> ControlFlowGraph<BasicBlock> {
    pub fn new(num_blocks: usize, terminator_edges: impl FnMut(BasicBlock) -> TerminatorEdges<BasicBlock>) -> Self {
        Self {
            terminator_edges: IndexVec::from_fn_n(terminator_edges, num_blocks),
        }
    }
    pub fn num_blocks(&self) -> usize {
        self.terminator_edges.len()
    }
}

impl<BasicBlock: Idx> Index<BasicBlock> for ControlFlowGraph<BasicBlock> {
    type Output = TerminatorEdges<BasicBlock>;

    fn index(&self, bb: BasicBlock) -> &Self::Output {
        &self.terminator_edges[bb]
    }
}

pub enum TerminatorEdges<BasicBlock: Idx> {
    /// For terminators that have no successor, like `return`.
    None,
    /// For terminators that a single successor, like `goto`, and `assert` without cleanup block.
    Single(BasicBlock),
    /// For terminators that two successors, `assert` with cleanup block and `falseEdge`.
    Double(BasicBlock, BasicBlock),
    /// Special action for `Yield`, `Call` and `InlineAsm` terminators.
    AssignOnReturn {
        return_: Box<[BasicBlock]>,
        cleanup: Option<BasicBlock>,
    },
    /// Special edge for `SwitchInt`.
    SwitchInt(SwitchTargets<BasicBlock>),
}

pub struct SwitchTargets<BasicBlock: Idx> {
    pub targets: FxIndexMap<Pu128, BasicBlock>,
    pub otherwise: Option<BasicBlock>,
}

pub struct DataDepGraph<BasicBlock: Idx, Local: Idx> {
    num_locals: usize,
    pub blocks: IndexVec<BasicBlock, BlockDataDepGraph<Local>>,
}

impl<BasicBlock: Idx, Local: Idx> Index<BasicBlock> for DataDepGraph<BasicBlock, Local> {
    type Output = BlockDataDepGraph<Local>;

    fn index(&self, bb: BasicBlock) -> &Self::Output {
        &self.blocks[bb]
    }
}

impl<BasicBlock: Idx, Local: Idx> DataDepGraph<BasicBlock, Local> {
    pub fn new(num_blocks: usize, mut num_statements: impl FnMut(BasicBlock) -> usize, num_locals: usize) -> Self {
        Self {
            blocks: IndexVec::from_fn_n(|bb| BlockDataDepGraph::new(num_statements(bb), num_locals), num_blocks),
            num_locals,
        }
    }
    pub fn blocks(&self) -> impl Iterator<Item = (BasicBlock, &BlockDataDepGraph<Local>)> + '_ {
        self.blocks.iter_enumerated()
    }
    pub(crate) fn num_blocks(&self) -> usize {
        self.blocks.len()
    }
    pub(crate) fn num_locals(&self) -> usize {
        self.num_locals
    }
}

pub struct BlockDataDepGraph<Local: Idx> {
    deps: SparseBitMatrix<usize, usize>,
    rdeps: SparseBitMatrix<usize, usize>,
    rdep_start: HybridBitSet<usize>,
    dep_end: HybridBitSet<usize>,
    rw_states: RWStates<Local>,
    accesses: Vec<Vec<(Local, PlaceContext)>>,
}

impl<Local: Idx> BlockDataDepGraph<Local> {
    pub fn num_statements(&self) -> usize {
        self.rdep_start.domain_size()
    }
    pub fn is_rdep_start(&self, statement: usize) -> bool {
        self.rdep_start.contains(statement)
    }
    pub fn rdep_start(&self) -> impl Iterator<Item = usize> + '_ {
        self.rdep_start.iter()
    }
    pub fn dep_end(&self) -> impl Iterator<Item = usize> + '_ {
        self.dep_end.iter()
    }
    pub fn deps(&self, statement: usize) -> impl Iterator<Item = usize> + '_ {
        self.deps.iter(statement)
    }
    pub fn rdeps(&self, statement: usize) -> impl Iterator<Item = usize> + '_ {
        self.rdeps.iter(statement)
    }
    pub fn accesses(&self, statement: usize) -> &[(Local, PlaceContext)] {
        &self.accesses[statement]
    }

    pub(crate) fn new(statements: usize, locals: usize) -> Self {
        Self {
            deps: SparseBitMatrix::new(locals),
            rdeps: SparseBitMatrix::new(locals),
            rdep_start: HybridBitSet::new_empty(statements),
            dep_end: HybridBitSet::new_empty(statements),
            rw_states: RWStates::new(statements, locals),
            accesses: vec![Vec::new(); statements],
        }
    }
    fn set_read(&mut self, statement: usize, local: Local) {
        self.rw_states.set_read(statement, local);
    }
    fn set_write(&mut self, statement: usize, local: Local) {
        self.rw_states.set_write(statement, local);
    }
    fn set_read_write(&mut self, statement: usize, local: Local) {
        self.rw_states.set_read_write(statement, local);
    }
    pub fn update_deps(&mut self, stmt: usize) {
        for local in self.rw_states.get_reads(stmt) {
            match (0..stmt)
                .rev()
                .find(|&prev_stmt| self.rw_states.get_write(prev_stmt, local))
            {
                Some(prev_stmt) => {
                    self.deps.insert(stmt, prev_stmt);
                    self.rdeps.insert(prev_stmt, stmt);
                },
                None => {
                    self.rdep_start.insert(stmt);
                },
            }
        }
    }
    pub fn access_local(&mut self, local: Local, pcx: PlaceContext, statement: usize) {
        self.accesses[statement].push((local, pcx));
        match Access::from_place_context(pcx) {
            Access::Read => self.set_read(statement, local),
            Access::Write => self.set_write(statement, local),
            Access::ReadWrite => self.set_read_write(statement, local),
            Access::NoAccess => {},
        }
    }
    pub fn update_dep_end(&mut self) {
        let num_statements = self.num_statements();
        for stmt in 0..num_statements {
            for local in self.rw_states.get_writes(stmt) {
                if !(stmt + 1..num_statements).any(|next_stmt| self.rw_states.get_read(next_stmt, local)) {
                    self.dep_end.insert(stmt);
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Access {
    Read,
    Write,
    ReadWrite,
    NoAccess,
}

impl Access {
    pub fn from_place_context(pcx: PlaceContext) -> Self {
        use PlaceContext::{MutatingUse, NonMutatingUse, NonUse};
        match pcx {
            NonMutatingUse(pcx) => {
                use NonMutatingUseContext::{
                    Copy, FakeBorrow, Inspect, Move, PlaceMention, Projection, RawBorrow, SharedBorrow,
                };
                match pcx {
                    Inspect | Copy | Move | SharedBorrow | FakeBorrow | RawBorrow | Projection => Self::Read,
                    PlaceMention => Self::NoAccess,
                }
            },
            MutatingUse(pcx) => {
                use MutatingUseContext::{
                    AsmOutput, Borrow, Call, Deinit, Drop, Projection, RawBorrow, Retag, SetDiscriminant, Store, Yield,
                };
                match pcx {
                    Store | AsmOutput | Call | Yield => Self::Write,
                    Borrow | RawBorrow | Projection => Self::Read,
                    SetDiscriminant | Drop => Self::ReadWrite,
                    Retag | Deinit => Self::NoAccess,
                }
            },
            NonUse(_) => Self::NoAccess,
        }
    }
}
