use core::iter::Iterator;

use gsgdt::{Edge, Graph, GraphvizSettings, Node, NodeStyle};
use rpl_mir::graph::{mir_control_flow_graph, mir_data_dep_graph, pat_control_fow_graph, pat_data_dep_graph};
use rpl_mir::pat;
use rpl_mir_graph::{ControlFlowGraph, DataDepGraph, TerminatorEdges};
use rustc_index::{Idx, IndexSlice};
use rustc_middle::mir;

pub type CfgBuilder<'a, B> = CfgBuilderImpl<'a, B, CfgBlockLabel>;

pub struct CfgBuilderImpl<'a, B: HasBasicBlocks, L> {
    basic_blocks: &'a B,
    cfg: ControlFlowGraph<B::BasicBlock>,
    node_style: NodeStyle,
    _l: std::marker::PhantomData<L>,
}

impl<'a, 'tcx, L: BlockLabel> CfgBuilderImpl<'a, mir::Body<'tcx>, L> {
    pub fn from_mir(body: &'a mir::Body<'tcx>, node_style: NodeStyle) -> Self {
        CfgBuilderImpl {
            basic_blocks: body,
            cfg: mir_control_flow_graph(body),
            node_style,
            _l: std::marker::PhantomData,
        }
    }
}
impl<'a, 'tcx, L: BlockLabel> CfgBuilderImpl<'a, pat::Patterns<'tcx>, L> {
    pub fn from_patterns(patterns: &'a pat::Patterns<'tcx>, pointer_bytes: u64, node_style: NodeStyle) -> Self {
        CfgBuilderImpl {
            basic_blocks: patterns,
            cfg: pat_control_fow_graph(patterns, pointer_bytes),
            node_style,
            _l: std::marker::PhantomData,
        }
    }
}

pub struct DdgBuilder<'a, B: HasBasicBlocks + HasLocals> {
    cfg_builder: CfgBuilderImpl<'a, B, DdgBlockLabel>,
    ddg: DataDepGraph<B::BasicBlock, B::Local>,
    config: DdgConfig,
}

#[derive(Clone)]
pub struct DdgConfig {
    /// Whether ignore the isolated nodes
    pub ignore_isolated: bool,
}

impl Default for DdgConfig {
    fn default() -> Self {
        DdgConfig { ignore_isolated: true }
    }
}

impl<'a, 'tcx> DdgBuilder<'a, mir::Body<'tcx>> {
    pub fn from_mir(body: &'a mir::Body<'tcx>, node_style: NodeStyle, config: DdgConfig) -> Self {
        DdgBuilder {
            cfg_builder: CfgBuilderImpl::from_mir(body, node_style),
            ddg: mir_data_dep_graph(body),
            config,
        }
    }
}
impl<'a, 'tcx> DdgBuilder<'a, pat::Patterns<'tcx>> {
    pub fn from_patterns(
        patterns: &'a pat::Patterns<'tcx>,
        pointer_bytes: u64,
        node_style: NodeStyle,
        config: DdgConfig,
    ) -> Self {
        DdgBuilder {
            cfg_builder: CfgBuilderImpl::from_patterns(patterns, pointer_bytes, node_style),
            ddg: pat_data_dep_graph(patterns),
            config,
        }
    }
}

pub trait BasicBlock: Idx {
    fn block_label(self) -> String {
        format!("bb{}", self.index())
    }
    fn block_title(self) -> String {
        format!("{self:?}")
    }
    fn in_label(self) -> String {
        format!("bb{:?}IN", self.index())
    }
    fn in_title(self) -> String {
        format!("{self:?}[IN]")
    }
    fn out_label(self) -> String {
        format!("bb{:?}OUT", self.index())
    }
    fn out_title(self) -> String {
        format!("{self:?}[OUT]")
    }
    fn stmt_label(self, index: usize) -> String {
        format!("bb{:?}stmt{index}", self.index())
    }
}

pub trait BlockLabel {
    fn in_label<B: BasicBlock>(bb: B) -> String;
    fn out_label<B: BasicBlock>(bb: B) -> String;
}

pub struct CfgBlockLabel;
struct DdgBlockLabel;

impl BlockLabel for CfgBlockLabel {
    fn in_label<B: BasicBlock>(bb: B) -> String {
        bb.block_label()
    }
    fn out_label<B: BasicBlock>(bb: B) -> String {
        bb.block_label()
    }
}

impl BlockLabel for DdgBlockLabel {
    fn in_label<B: BasicBlock>(bb: B) -> String {
        bb.in_label()
    }
    fn out_label<B: BasicBlock>(bb: B) -> String {
        bb.out_label()
    }
}

pub trait FmtDebug {
    fn fmt_debug(&self) -> &dyn std::fmt::Debug;
}

pub trait BasicBlockData {
    type Statement: FmtDebug;
    type Terminator: FmtDebug;
    fn statements(&self) -> &[Self::Statement];
    fn terminator(&self) -> &Self::Terminator;
    fn num_statements(&self) -> usize {
        self.statements().len()
    }
}

pub trait HasBasicBlocks {
    type BasicBlock: BasicBlock;
    type BasicBlockData: BasicBlockData;
    fn basic_blocks(&self) -> &IndexSlice<Self::BasicBlock, Self::BasicBlockData>;
}

pub trait HasLocals {
    type Local: Idx;
}

impl<B: HasBasicBlocks, L: BlockLabel> CfgBuilderImpl<'_, B, L> {
    pub fn build(&self) -> Graph {
        let nodes = self.build_nodes();
        let edges = self.build_edges();
        Graph::new("ControlFlowGraph".into(), nodes, edges)
    }
    fn new_node(&self, bb: B::BasicBlock, block: &B::BasicBlockData) -> Node {
        Node::new(
            std::iter::chain(
                block.statements().iter().map(FmtDebug::fmt_debug),
                Some(block.terminator().fmt_debug()),
            )
            .map(|f| format!("{f:?}"))
            .collect(),
            bb.block_label(),
            bb.block_title(),
            self.node_style.clone(),
        )
    }
    fn build_nodes(&self) -> Vec<Node> {
        self.basic_blocks
            .basic_blocks()
            .iter_enumerated()
            .map(|(bb, block)| self.new_node(bb, block))
            .collect()
    }
    fn new_edge(&self, from: B::BasicBlock, to: B::BasicBlock, label: impl Into<String>) -> Edge {
        Edge::new(L::out_label(from), L::in_label(to), label.into())
    }
    fn build_edges(&self) -> Vec<Edge> {
        self.cfg
            .blocks()
            .iter_enumerated()
            .fold(Vec::new(), |mut edges, (bb, terminator)| {
                let new_edge = |succ, label| self.new_edge(bb, succ, label);
                use TerminatorEdges::{AssignOnReturn, Double, None, Single, SwitchInt};
                match terminator {
                    None => {},
                    &Single(target) => {
                        edges.push(new_edge(target, "".to_string()));
                    },
                    &Double(bb0, bb1) => {
                        edges.push(new_edge(bb0, "return".to_string()));
                        edges.push(new_edge(bb1, "unwind".to_string()));
                    },
                    AssignOnReturn { return_, cleanup } => {
                        edges.extend(return_.iter().map(|&target| new_edge(target, "return".to_string())));
                        if let &Some(cleanup) = cleanup {
                            edges.push(new_edge(cleanup, "unwind".to_string()));
                        }
                    },
                    SwitchInt(targets) => {
                        edges.extend(
                            targets
                                .targets
                                .iter()
                                .map(|(&value, &target)| new_edge(target, format!("{value}"))),
                        );
                        if let Some(otherwise) = targets.otherwise {
                            edges.push(new_edge(otherwise, "otherwise".to_string()));
                        }
                    },
                }
                edges
            })
    }
}

pub struct MultiGraph {
    pub name: String,
    pub graphs: Vec<Graph>,
    pub inter_graph_edges: Vec<Edge>,
}

impl MultiGraph {
    pub fn new(name: String, graphs: Vec<Graph>, inter_graph_edges: Vec<Edge>) -> Self {
        MultiGraph {
            name,
            graphs,
            inter_graph_edges,
        }
    }
    pub fn to_dot<W: std::io::Write>(&self, f: &mut W, settings: &GraphvizSettings) -> std::io::Result<()> {
        writeln!(f, "digraph {} {{", self.name)?;
        for graph in &self.graphs {
            graph.to_dot(f, settings, true)?;
        }
        for edge in &self.inter_graph_edges {
            edge.to_dot(f)?;
        }
        writeln!(f, "}}")
    }
}

impl<B: HasBasicBlocks + HasLocals> DdgBuilder<'_, B> {
    pub fn build(&self) -> MultiGraph {
        let blocks = self.build_blocks();
        let edges = self.cfg_builder.build_edges();
        MultiGraph::new("DataDependencyGraph".into(), blocks, edges)
    }
    fn build_blocks(&self) -> Vec<Graph> {
        self.cfg_builder
            .basic_blocks
            .basic_blocks()
            .iter_enumerated()
            .map(|(bb, block)| self.build_block(bb, block))
            .collect()
    }
    fn build_block(&self, bb: B::BasicBlock, block: &B::BasicBlockData) -> Graph {
        let nodes = self.build_stmts(bb, block);
        let edges = self.build_data_deps(bb, block);
        Graph::new(bb.block_label(), nodes, edges)
    }
    fn new_node(&self, label: impl Into<String>, content: impl Into<String>) -> Node {
        Node::new(
            vec![content.into()],
            label.into(),
            String::new(),
            self.cfg_builder.node_style.clone(),
        )
    }
    fn should_ignore(&self, bb: B::BasicBlock, stmt: usize) -> bool {
        let term = self.cfg_builder.basic_blocks.basic_blocks()[bb].num_statements();
        self.config.ignore_isolated
            && self.ddg.blocks[bb].deps(stmt).next().is_none()
            && self.ddg.blocks[bb].rdeps(stmt).next().is_none()
            && !self.ddg.blocks[bb].is_rdep_start(stmt)
            && !self.ddg.blocks[bb].is_dep_end(stmt)
            && (stmt < term
                || matches!(
                    self.cfg_builder.cfg.blocks()[bb],
                    TerminatorEdges::None | TerminatorEdges::Single(_)
                ))
    }

    fn build_stmts(&self, bb: B::BasicBlock, block: &B::BasicBlockData) -> Vec<Node> {
        let mut stmts = vec![
            self.new_node(bb.in_label(), bb.in_title()),
            self.new_node(bb.out_label(), bb.out_title()),
        ];
        stmts.extend(
            std::iter::chain(
                block.statements().iter().map(FmtDebug::fmt_debug),
                Some(block.terminator().fmt_debug()),
            )
            .enumerate()
            .filter(|&(stmt, _)| !self.should_ignore(bb, stmt))
            .map(|(index, stmt)| self.new_node(bb.stmt_label(index), format!("{stmt:?}"))),
        );
        stmts
    }
    fn new_edge(&self, from: impl Into<String>, to: impl Into<String>) -> Edge {
        Edge::new(from.into(), to.into(), "".to_string())
    }
    fn build_data_deps(&self, bb: B::BasicBlock, block: &B::BasicBlockData) -> Vec<Edge> {
        std::iter::chain(
            self.ddg.blocks[bb]
                .rdep_start()
                .map(|stmt| self.new_edge(bb.in_label(), bb.stmt_label(stmt))),
            self.ddg.blocks[bb]
                .dep_end()
                .map(|stmt| self.new_edge(bb.stmt_label(stmt), bb.out_label())),
        )
        .chain(
            (0..=block.num_statements())
                .filter(|&stmt| !self.should_ignore(bb, stmt))
                .flat_map(|from| {
                    self.ddg.blocks[bb]
                        .rdeps(from)
                        .map(move |to| self.new_edge(bb.stmt_label(from), bb.stmt_label(to)))
                }),
        )
        .collect()
    }
}

impl BasicBlock for mir::BasicBlock {}
impl BasicBlock for pat::BasicBlock {}

impl<'tcx> HasBasicBlocks for mir::Body<'tcx> {
    type BasicBlock = mir::BasicBlock;

    type BasicBlockData = mir::BasicBlockData<'tcx>;

    fn basic_blocks(&self) -> &IndexSlice<Self::BasicBlock, Self::BasicBlockData> {
        &self.basic_blocks
    }
}
impl<'tcx> HasBasicBlocks for pat::Patterns<'tcx> {
    type BasicBlock = pat::BasicBlock;

    type BasicBlockData = pat::BasicBlockData<'tcx>;

    fn basic_blocks(&self) -> &IndexSlice<Self::BasicBlock, Self::BasicBlockData> {
        &self.basic_blocks
    }
}

impl HasLocals for mir::Body<'_> {
    type Local = mir::Local;
}
impl HasLocals for pat::Patterns<'_> {
    type Local = pat::LocalIdx;
}

impl<'tcx> BasicBlockData for mir::BasicBlockData<'tcx> {
    type Statement = mir::Statement<'tcx>;
    type Terminator = mir::Terminator<'tcx>;

    fn statements(&self) -> &[Self::Statement] {
        &self.statements
    }
    fn terminator(&self) -> &Self::Terminator {
        self.terminator()
    }
}
impl<'tcx> BasicBlockData for pat::BasicBlockData<'tcx> {
    type Statement = pat::StatementKind<'tcx>;
    type Terminator = pat::TerminatorKind<'tcx>;

    fn statements(&self) -> &[Self::Statement] {
        &self.statements
    }
    fn terminator(&self) -> &Self::Terminator {
        self.terminator()
    }
}

impl FmtDebug for mir::Statement<'_> {
    fn fmt_debug(&self) -> &dyn std::fmt::Debug {
        self
    }
}
impl FmtDebug for pat::StatementKind<'_> {
    fn fmt_debug(&self) -> &dyn std::fmt::Debug {
        self
    }
}

impl FmtDebug for mir::Terminator<'_> {
    fn fmt_debug(&self) -> &dyn std::fmt::Debug {
        &self.kind
    }
}

impl FmtDebug for pat::TerminatorKind<'_> {
    fn fmt_debug(&self) -> &dyn std::fmt::Debug {
        self
    }
}
