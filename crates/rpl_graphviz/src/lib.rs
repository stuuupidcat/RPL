#![allow(internal_features)]
#![feature(rustc_private)]
#![feature(rustc_attrs)]
#![feature(let_chains)]
#![feature(if_let_guard)]
#![feature(box_patterns)]
#![feature(try_trait_v2)]
#![feature(debug_closure_helpers)]
#![feature(iter_chain)]
#![feature(iterator_try_collect)]
#![feature(cell_update)]

extern crate rustc_graphviz;
extern crate rustc_index;
extern crate rustc_middle;

use std::fs::File;
use std::io::{BufRead, Write};

use gsgdt::{Edge, Graph, GraphvizSettings, MultiGraph, Node, NodeStyle};
use rpl_mir::graph::{normalized_terminator_edges, pat_data_dep_graph};
use rpl_mir::pattern::Patterns;
use rpl_mir_graph::TerminatorEdges;

pub fn write_cfg_graphviz<'tcx>(patterns: &Patterns<'tcx>, f: &mut File) {
    let graph = patterns_cfg_to_generic_graph(&patterns);
    let settings = GraphvizSettings::default();
    let _ = graph.to_dot(f, &settings, false);
}

pub fn write_ddg_graphviz<'tcx>(patterns: &Patterns<'tcx>, f: &mut File) {
    let (graphs, cross_graph_edges) = patterns_ddg_to_generic_graphs(&patterns);
    let multi_graph = MultiGraph::new("name".to_string(), graphs);
    // write graphs
    let mut buf = Vec::new();
    let _res = multi_graph.to_dot(&mut buf, &GraphvizSettings::default());
    // write all cross graph edges to the '}' before the last line
    let mut lines = buf
        .lines()
        .collect::<Vec<_>>()
        .into_iter()
        .map(|line| line.unwrap())
        .collect::<Vec<_>>();
    let _last_line = lines.pop().unwrap();
    let mut last_line = String::new();
    for edge in cross_graph_edges.iter() {
        last_line.push_str(&format!(
            "\n    {} -> {} [label=\"{}\"]",
            edge.from, edge.to, edge.label
        ));
    }
    last_line.push_str("\n}");
    lines.push(last_line);
    let _ = f.write_all(lines.join("\n").as_bytes());
}

pub fn patterns_cfg_to_generic_graph<'tcx>(patterns: &'tcx Patterns) -> Graph {
    let mut nodes: Vec<Node> = Vec::new();
    let mut edges: Vec<Edge> = Vec::new();

    let bbs = &patterns.basic_blocks;
    for (bb, block) in bbs.iter_enumerated() {
        let mut stmts: Vec<String> = block.statements.iter().map(|stmt| format!("{:?}", stmt)).collect();
        stmts.push(format!("{:?}", block.terminator()).replace('?', ""));
        let label = format!("{:?}", bb).replace('?', "");
        let title = format!("{:?}", bb).replace('?', "");
        let style = NodeStyle::default();
        let node = Node::new(stmts, label, title, style);
        nodes.push(node);

        let terminator = block.terminator();
        let normalized_terminator_edge = normalized_terminator_edges(Some(terminator), 8);
        use TerminatorEdges::{AssignOnReturn, Double, None, Single, SwitchInt};
        match normalized_terminator_edge {
            None => {},
            Single(target) => {
                let src = format!("{:?}", bb).replace('?', "");
                let dst = format!("{:?}", target).replace('?', "");
                let label = "".to_string();
                let edge = Edge::new(src, dst, label);
                edges.push(edge);
            },
            Double(bb0, bb1) => {
                let src = format!("{:?}", bb).replace('?', "");
                let dst0 = format!("{:?}", bb0).replace('?', "");
                let dst1 = format!("{:?}", bb1).replace('?', "");
                let label0 = "".to_string();
                let label1 = "".to_string();
                let edge0 = Edge::new(src.clone(), dst0, label0);
                let edge1 = Edge::new(src, dst1, label1);
                edges.push(edge0);
                edges.push(edge1);
            },
            AssignOnReturn { return_, cleanup } => {
                let src = format!("{:?}", bb).replace('?', "");
                for target in return_.iter() {
                    let dst = format!("{:?}", target).replace('?', "");
                    let label = "AssignOnReturn".to_string();
                    let edge = Edge::new(src.clone(), dst, label);
                    edges.push(edge);
                }
                if let Some(cleanup) = cleanup {
                    let src = format!("{:?}", bb).replace('?', "");
                    let dst = format!("{:?}", cleanup).replace('?', "");
                    let label = "Cleanup".to_string();
                    let edge = Edge::new(src, dst, label);
                    edges.push(edge);
                }
            },
            SwitchInt(targets) => {
                for (value, target) in targets.targets.iter() {
                    let src = format!("{:?}", bb).replace('?', "");
                    let dst = format!("{:?}", target).replace('?', "");
                    let label = format!("{:?}", value);
                    let label = label[6..label.len() - 1].to_string();
                    let edge = Edge::new(src, dst, label);
                    edges.push(edge);
                }
                if let Some(otherwise) = targets.otherwise {
                    let src = format!("{:?}", bb).replace('?', "");
                    let dst = format!("{:?}", otherwise).replace('?', "");
                    let label = "Otherwise".to_string();
                    let edge = Edge::new(src, dst, label);
                    edges.push(edge);
                }
            },
        }
    }
    Graph::new("name".to_string(), nodes, edges)
}

struct DDGConfig {
    pub in2out: bool,         //  Whether set an edge from IN to OUT, if there is no statement in the block,
    pub in2first_stmt: bool,  // Whether set an edge from IN to the first statement when there are no in2 edges
    pub terminator2out: bool, // Whether set an edge from Terminator to OUT, if there are no 2out edges
    pub ignore_isolated_terminator: bool, // Whether ignore the isolated terminator node
}

impl Default for DDGConfig {
    fn default() -> Self {
        Self {
            in2out: true,
            in2first_stmt: true,
            ignore_isolated_terminator: true,
            terminator2out: true,
        }
    }
}

pub fn patterns_ddg_to_generic_graphs(patterns: &Patterns) -> (Vec<Graph>, Vec<Edge>) {
    let ddg = pat_data_dep_graph(patterns);
    let mut graphs: Vec<Graph> = Vec::new();
    let mut cross_subgraph_edges: Vec<Edge> = Vec::new();
    let ddg_config = DDGConfig::default();
    let basic_blocks = &patterns.basic_blocks;

    for (bb, block) in ddg.blocks() {
        let basic_block_data = &basic_blocks[bb];
        let mut nodes: Vec<Node> = Vec::new();
        let mut edges: Vec<Edge> = Vec::new();
        let nodes_num = block.num_statements();
        let mut terminator_needed = false; // whether there is a TERMINATOR -> OUT edge

        // IN node
        let label = format!("{:?}IN", bb).replace("?", "");
        let title = String::new();
        let style = NodeStyle::default();
        let node = Node::new(vec![label.clone()], label.clone(), title, style);
        nodes.push(node);
        // In edges
        let mut has_in2_edge = false;
        for dep in block.rdep_start() {
            if !has_in2_edge {
                has_in2_edge = true;
            }
            let src = format!("{:?}IN", bb).replace("?", "");
            let dst = format!("{:?}stmt{:?}", bb, dep).replace("?", "");
            if dep == nodes_num - 1 {
                // IN -> terminator
                terminator_needed = true;
            }
            let label = String::new();
            let edge = Edge::new(src, dst, label);
            edges.push(edge);
        }

        // OUT node
        let label = format!("{:?}OUT", bb).replace("?", "");
        let title = String::new();
        let style = NodeStyle::default();
        let node = Node::new(vec!["OUT".to_string()], label.clone(), title, style);
        nodes.push(node);
        // Out edges
        let mut has_2out_edge = false;
        for dep in block.dep_end() {
            if !has_2out_edge {
                has_2out_edge = true;
            }
            let src = format!("{:?}stmt{:?}", bb, dep).replace("?", "");
            let dst = format!("{:?}OUT", bb).replace("?", "");
            let label = String::new();
            let edge = Edge::new(src, dst, label);
            edges.push(edge);
        }

        // Terminator -> OUT
        if ddg_config.terminator2out && !has_2out_edge && has_in2_edge
        // last condition means no IN -> OUT edge
        {
            assert!(
                nodes_num > 0,
                "There should be at least one statement or terminator in the block"
            );
            terminator_needed = true;
            let src = format!("{:?}stmt{:?}", bb, nodes_num - 1).replace("?", "");
            let dst = format!("{:?}OUT", bb).replace("?", "");
            let label = String::new();
            let edge = Edge::new(src, dst, label);
            edges.push(edge);
        }

        // In -> OUT
        if ddg_config.in2out && !has_in2_edge && !has_2out_edge
        // last condition means no Node -> OUT edge
        {
            let src = format!("{:?}IN", bb).replace("?", "");
            let dst = format!("{:?}OUT", bb).replace("?", "");
            let label = String::new();
            let edge = Edge::new(src, dst, label);
            edges.push(edge);
        }

        // IN -> First stmt
        if ddg_config.in2first_stmt && !has_in2_edge && has_2out_edge {
            let src = format!("{:?}IN", bb).replace("?", "");
            let dst = format!("{:?}stmt0", bb).replace("?", "");
            let label = String::new();
            let edge = Edge::new(src, dst, label);
            edges.push(edge);
        }

        // stmt nodes and stmt edges
        for idx in 0..nodes_num {
            // node
            let label = format!("{:?}stmt{:?}", bb, idx).replace("?", "");
            let title = String::new();
            let style = NodeStyle::default();
            let stmt = if idx < nodes_num - 1 {
                format!("{:?}", basic_block_data.statements[idx]).replace("?", "")
            } else {
                format!("{:?}", basic_block_data.terminator()).replace("?", "")
            };
            let node = Node::new(vec![stmt], label, title, style);

            // node depedencies
            let deps = block.deps(idx);
            let mut deps_empty = true;
            for dep in deps {
                if deps_empty {
                    deps_empty = false
                };
                let src = format!("{:?}stmt{:?}", bb, dep).replace("?", "");
                let dst = format!("{:?}stmt{:?}", bb, idx).replace("?", "");
                let label = String::new();
                let edge = Edge::new(src, dst, label);
                edges.push(edge);
            }
            if ddg_config.ignore_isolated_terminator && idx == nodes_num - 1 && deps_empty && !terminator_needed {
                // ignore the terminator
                println!(
                    "ignore the isolated terminator `{:?}` of block `{:?}`",
                    basic_block_data.terminator(),
                    bb
                );
            } else {
                nodes.push(node);
            }
        }

        // cfg edges: OUT->IN (cross subgraph edges)
        let terminator = basic_block_data.terminator();
        let normalized_terminator_edge = normalized_terminator_edges(Some(terminator), 8);
        use TerminatorEdges::{AssignOnReturn, Double, None, Single, SwitchInt};
        match normalized_terminator_edge {
            None => {},
            Single(target) => {
                let src = format!("{:?}OUT", bb).replace("?", "");
                let dst = format!("{:?}IN", target).replace("?", "");
                let label = String::new();
                let edge = Edge::new(src, dst, label);
                cross_subgraph_edges.push(edge);
            },
            Double(bb0, bb1) => {
                let src = format!("{:?}OUT", bb).replace("?", "");
                let dst0 = format!("{:?}IN", bb0).replace("?", "");
                let dst1 = format!("{:?}IN", bb1).replace("?", "");
                let label0 = String::new();
                let label1 = String::new();
                let edge0 = Edge::new(src.clone(), dst0, label0);
                let edge1 = Edge::new(src, dst1, label1);
                cross_subgraph_edges.push(edge0);
                cross_subgraph_edges.push(edge1);
            },
            AssignOnReturn { return_, cleanup } => {
                let src = format!("{:?}OUT", bb).replace("?", "");
                for target in return_.iter() {
                    let dst = format!("{:?}IN", target).replace("?", "");
                    let label = "AssignOnReturn".to_string();
                    let edge = Edge::new(src.clone(), dst, label);
                    cross_subgraph_edges.push(edge);
                }
                if let Some(cleanup) = cleanup {
                    let src = format!("{:?}OUT", bb).replace("?", "");
                    let dst = format!("{:?}IN", cleanup).replace("?", "");
                    let label = "Cleanup".to_string();
                    let edge = Edge::new(src, dst, label);
                    cross_subgraph_edges.push(edge);
                }
            },
            SwitchInt(targets) => {
                for (value, target) in targets.targets.iter() {
                    let src = format!("{:?}OUT", bb).replace("?", "");
                    let dst = format!("{:?}IN", target).replace("?", "");
                    let label = format!("{:?}", value).replace('?', "");
                    let label = label[6..label.len() - 1].to_string();
                    let edge = Edge::new(src, dst, label);
                    cross_subgraph_edges.push(edge);
                }
                if let Some(otherwise) = targets.otherwise {
                    let src = format!("{:?}OUT", bb).replace("?", "");
                    let dst = format!("{:?}IN", otherwise).replace("?", "");
                    let label = "Otherwise".to_string();
                    let edge = Edge::new(src, dst, label);
                    cross_subgraph_edges.push(edge);
                }
            },
        }
        let graph = Graph::new(format!("{:?}", bb).replace("?", ""), nodes, edges);
        graphs.push(graph);
    }
    (graphs, cross_subgraph_edges)
}
