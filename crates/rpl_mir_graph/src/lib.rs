#![feature(rustc_private)]
#![feature(gen_blocks)]

extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_index;
extern crate rustc_middle;
#[macro_use]
extern crate tracing;

pub(crate) mod rwstate;

mod graph;

pub use graph::{
    Access, BlockDataDepGraph, ControlFlowGraph, DataDepGraph, Edge, EdgeKind, NodeIdx, NodeKind, ProgramDepGraph,
    SwitchTargets, TerminatorEdges,
};
