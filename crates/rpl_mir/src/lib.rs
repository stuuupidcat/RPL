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

extern crate rustc_abi;
extern crate rustc_arena;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_fluent_macro;
extern crate rustc_hash;
extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_macros;
extern crate rustc_middle;
extern crate rustc_span;
extern crate rustc_target;
#[macro_use]
extern crate tracing;
extern crate smallvec;

pub mod graph;
pub mod pattern;

mod matches;

use std::cell::RefCell;
use std::iter::zip;

use crate::graph::{MirControlFlowGraph, MirDataDepGraph, PatControlFlowGraph, PatDataDepGraph};
use rpl_mir_graph::TerminatorEdges;
use rustc_abi::VariantIdx;
use rustc_hash::FxHashMap;
use rustc_hir::def::CtorKind;
use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_hir::definitions::DefPathData;
use rustc_index::bit_set::HybridBitSet;
use rustc_index::{IndexSlice, IndexVec};
use rustc_middle::mir::interpret::PointerArithmetic;
use rustc_middle::mir::tcx::PlaceTy;
use rustc_middle::ty::{Ty, TyCtxt};
use rustc_middle::{mir, ty};
use rustc_span::symbol::kw;
use rustc_span::Symbol;
use rustc_target::abi::FieldIdx;

pub use crate::pattern as pat;
pub use matches::{Matches, StatementMatch};

pub struct CheckMirCtxt<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    param_env: ty::ParamEnv<'tcx>,
    body: &'a mir::Body<'tcx>,
    patterns: &'a pat::Patterns<'tcx>,
    pat_cfg: PatControlFlowGraph,
    pat_ddg: PatDataDepGraph,
    mir_cfg: MirControlFlowGraph,
    mir_ddg: MirDataDepGraph,
    // pat_pdg: PatProgramDepGraph,
    // mir_pdg: MirProgramDepGraph,
    locals: IndexVec<pat::LocalIdx, RefCell<HybridBitSet<mir::Local>>>,
    ty_vars: IndexVec<pat::TyVarIdx, RefCell<Vec<Ty<'tcx>>>>,
}

impl<'a, 'tcx> CheckMirCtxt<'a, 'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>, body: &'a mir::Body<'tcx>, patterns: &'a pat::Patterns<'tcx>) -> Self {
        // let pat_pdg = crate::graph::pat_program_dep_graph(&patterns, tcx.pointer_size().bytes_usize());
        // let mir_pdg = crate::graph::mir_program_dep_graph(body);
        let pat_cfg = crate::graph::pat_control_fow_graph(patterns, tcx.pointer_size().bytes());
        let pat_ddg = crate::graph::pat_data_dep_graph(patterns);
        let mir_cfg = crate::graph::mir_control_flow_graph(body);
        let mir_ddg = crate::graph::mir_data_dep_graph(body);
        Self {
            tcx,
            param_env: tcx.param_env_reveal_all_normalized(body.source.def_id()),
            body,
            patterns,
            pat_cfg,
            pat_ddg,
            mir_cfg,
            mir_ddg,
            // pat_pdg,
            // mir_pdg,
            locals: IndexVec::from_elem_n(
                RefCell::new(HybridBitSet::new_empty(body.local_decls.len())),
                patterns.locals.len(),
            ),
            ty_vars: IndexVec::from_elem(RefCell::new(Vec::new()), &patterns.ty_vars),
        }
    }
    pub fn check(&self) -> Option<Matches<'tcx>> {
        matches::matches(self)
    }
    /*
    pub fn check(&self) {
        use NodeKind::{BlockEnter, BlockExit, Local, StmtOrTerm};
        for (bb_pat, block_pat) in self.patterns.basic_blocks.iter_enumerated() {
            for (bb, block) in self.body.basic_blocks.iter_enumerated() {}
        }
        for (pat_node_idx, pat_node) in self.pat_pdg.nodes() {
            for (mir_node_idx, mir_node) in self.mir_pdg.nodes() {
                let matched = match (pat_node, mir_node) {
                    (StmtOrTerm(bb_pat, stmt_pat), StmtOrTerm(block, statement_index)) => self
                        .match_statement_or_terminator(
                            (bb_pat, stmt_pat).into(),
                            mir::Location { block, statement_index },
                        ),
                    (BlockEnter(_), BlockEnter(_)) | (BlockExit(_), BlockExit(_)) => true,
                    (Local(local_pat), Local(local)) => self.match_local(local_pat, local),
                    _ => continue,
                };
                if matched {
                    self.candidates[pat_node_idx].push(NodeMatch {
                        mir_node_idx,
                        edges_matched: 0,
                    });
                }
            }
        }
        // Pattern:               MIR:
        //             alignment
        // pat_node(u1) ------> mir_node(u2)
        //     |                   |
        //     | pat_edge          | mir_edge
        //     |                   |
        //     v       alignment   v
        // pat_node(v1) ------> mir_node(v2)
        //
        for (pat_node_idx, _) in self.pat_pdg.nodes() {
            let mut iter = self.candidates[pat_node_idx].iter().enumerate().skip(0);
            while let Some((candidate_idx, &NodeMatch { mir_node_idx, .. })) = iter.next() {
                let edges_matched = self
                    .pat_pdg
                    .edges_from(pat_node_idx)
                    .iter()
                    .filter(|pat_edge| {
                        self.candidates[pat_edge.to].iter().any(
                            |&NodeMatch {
                                 mir_node_idx: mir_node_to,
                                 ..
                             }| {
                                self.mir_pdg.find_edge(mir_node_idx, mir_node_to).is_some()
                            },
                        )
                    })
                    .count();
                self.candidates[pat_node_idx][candidate_idx].edges_matched = edges_matched;
                iter = self.candidates[pat_node_idx].iter().enumerate().skip(candidate_idx + 1);
            }
        }
        for candidate in &mut self.candidates {
            candidate.sort_unstable_by_key(|candicate| std::cmp::Reverse(candicate.edges_matched));
        }
    }
    */

    /*
    #[instrument(level = "info", skip(self), fields(def_id = ?self.body.source.def_id()))]
    pub fn check(&mut self) {
        self.check_args();
        let mut visited = BitSet::new_empty(self.body.basic_blocks.len());
        let mut block = Some(mir::START_BLOCK);
        let next_block = |b: mir::BasicBlock| {
            if b.as_usize() + 1 == self.body.basic_blocks.len() {
                mir::START_BLOCK
            } else {
                b.plus(1)
            }
        };
        let mut num_visited = 0;
        while let Some(b) = block {
            if !visited.insert(b) {
                debug!("skip visited block {b:?}");
                block = Some(next_block(b));
                continue;
            }
            let matched = self.check_block(b).is_some();
            let &mut b = block.insert(match self.body[b].terminator().edges() {
                mir::TerminatorEdges::None => next_block(b),
                mir::TerminatorEdges::Single(next) => next,
                mir::TerminatorEdges::Double(next, _) => next,
                mir::TerminatorEdges::AssignOnReturn { return_: &[next], .. } => next,
                _ => next_block(b),
                // mir::TerminatorEdges::AssignOnReturn { .. } => todo!(),
                // mir::TerminatorEdges::SwitchInt { targets, discr } => todo!(),
            });
            debug!("jump to block {b:?}");
            if matched {
                visited.remove(b);
            }
            num_visited += 1;
            if num_visited >= self.body.basic_blocks.len() {
                debug!("all blocks has been visited");
                break;
            }
        }
    }

    fn check_args(&mut self) {
        for (pat, pattern) in self.patterns.ready_patterns() {
            let pat::PatternKind::Init(local) = pattern.kind else {
                continue;
            };
            for arg in self.body.args_iter() {
                if self.match_local(local, arg) {
                    self.patterns.add_match(pat, pat::MatchKind::Argument(arg));
                }
            }
        }
    }

    #[instrument(level = "info", skip(self))]
    fn check_block(&mut self, block: mir::BasicBlock) -> Option<pat::MatchIdx> {
        info!("BasicBlock: {}", {
            let mut buffer = Vec::new();
            mir::pretty::write_basic_block(self.tcx, block, self.body, &mut |_, _| Ok(()), &mut buffer).unwrap();
            String::from_utf8_lossy(&buffer).into_owned()
        });
        for (statement_index, statement) in self.body[block].statements.iter().enumerate() {
            let location = mir::Location { block, statement_index };
            self.check_statement(location, statement);
        }
        self.check_terminator(block, self.body[block].terminator())
    }

    fn check_statement(&mut self, location: mir::Location, statement: &mir::Statement<'tcx>) {
        self.match_statement(location, statement);
    }
    fn check_terminator(
        &mut self,
        block: mir::BasicBlock,
        terminator: &'tcx mir::Terminator<'tcx>,
    ) -> Option<pat::MatchIdx> {
        self.match_terminator(block, terminator)
    }
    */
}

impl<'tcx> CheckMirCtxt<'_, 'tcx> {
    pub fn match_local(&self, pat: pat::LocalIdx, local: mir::Local) -> bool {
        let mut locals = self.locals[pat].borrow_mut();
        if locals.contains(local) {
            return true;
        }
        let matched = self.match_ty(self.patterns.locals[pat], self.body.local_decls[local].ty);
        debug!(?pat, ?local, matched, "match_local");
        if matched {
            locals.insert(local);
        }
        matched
    }
    pub fn match_place(&self, pat: pat::Place<'tcx>, place: mir::Place<'tcx>) -> bool {
        self.match_place_ref(pat, place.as_ref())
    }
    fn match_place_ref(&self, pat: pat::Place<'tcx>, place: mir::PlaceRef<'tcx>) -> bool {
        use mir::ProjectionElem::*;
        use pat::Field::{Named, Unnamed};
        let place_proj_and_ty = place.projection.iter().scan(
            PlaceTy::from_ty(self.body.local_decls[place.local].ty),
            |place_ty, &proj| {
                Some((
                    proj,
                    std::mem::replace(place_ty, place_ty.projection_ty(self.tcx, proj)),
                ))
            },
        );
        self.match_local(pat.local, place.local)
            && pat.projection.len() == place.projection.len()
            && std::iter::zip(pat.projection, place_proj_and_ty).all(|(&proj_pat, (proj, place_ty))| {
                match (place_ty.ty.kind(), proj_pat, proj) {
                    (_, pat::PlaceElem::Deref, Deref) => true,
                    (ty::Adt(adt, _), pat::PlaceElem::Field(field), Field(idx, _)) => {
                        let variant = match place_ty.variant_index {
                            None => adt.non_enum_variant(),
                            Some(idx) => adt.variant(idx),
                        };
                        match (variant.ctor, field) {
                            (None, Named(name)) => variant.ctor.is_none() && variant.fields[idx].name == name,
                            (Some((CtorKind::Fn, _)), Unnamed(idx_pat)) => idx_pat == idx,
                            _ => false,
                        }
                    },
                    (_, pat::PlaceElem::Index(local_pat), Index(local)) => self.match_local(local_pat, local),
                    (
                        _,
                        pat::PlaceElem::ConstantIndex {
                            offset: offset_pat,
                            from_end: from_end_pat,
                            min_length: min_length_pat,
                        },
                        ConstantIndex {
                            offset,
                            from_end,
                            min_length,
                        },
                    ) => (offset_pat, from_end_pat, min_length_pat) == (offset, from_end, min_length),
                    (
                        _,
                        pat::PlaceElem::Subslice {
                            from: from_pat,
                            to: to_pat,
                            from_end: from_end_pat,
                        },
                        Subslice { from, to, from_end },
                    ) => (from_pat, to_pat, from_end_pat) == (from, to, from_end),
                    (ty::Adt(adt, _), pat::PlaceElem::Downcast(sym), Downcast(_, idx)) => {
                        adt.is_enum() && adt.variant(idx).name == sym
                    },
                    (_, pat::PlaceElem::OpaqueCast(ty_pat), OpaqueCast(ty))
                    | (_, pat::PlaceElem::Subtype(ty_pat), Subtype(ty)) => self.match_ty(ty_pat, ty),
                    _ => false,
                }
            })
    }

    pub fn match_statement_or_terminator(&self, pat: pat::Location, loc: mir::Location) -> bool {
        let block_pat = &self.patterns[pat.block];
        let block = &self.body[loc.block];
        match (
            pat.statement_index < block_pat.statements.len(),
            loc.statement_index < block.statements.len(),
        ) {
            (true, true) => self.match_statement(
                &block_pat.statements[pat.statement_index],
                &block.statements[loc.statement_index],
            ),
            (true, false)
                if let pat::StatementKind::Init(place_pat) = block_pat.statements[pat.statement_index]
                    && let mir::TerminatorKind::Call { destination, .. } = block.terminator().kind =>
            {
                self.match_place(place_pat, destination)
            },
            (false, false) => self.match_terminator(pat, loc, block_pat.terminator(), block.terminator()),
            (true, false) | (false, true) => false,
        }
    }

    pub fn match_statement(&self, pat: &pat::StatementKind<'tcx>, statement: &mir::Statement<'tcx>) -> bool {
        let matched = match (pat, &statement.kind) {
            (&pat::StatementKind::Init(place_pat), &mir::StatementKind::Assign(box (place, _))) => {
                self.match_place(place_pat, place)
            },
            (
                &pat::StatementKind::Assign(place_pat, ref rvalue_pat),
                &mir::StatementKind::Assign(box (place, ref rvalue)),
            ) => self.match_rvalue(rvalue_pat, rvalue) && self.match_place(place_pat, place),
            _ => false,
        };
        if matched {
            info!(?pat, ?statement, "candidate matched");
        }
        matched
    }

    pub fn match_terminator(
        &self,
        loc_pat: pat::Location,
        loc: mir::Location,
        pat: &pat::TerminatorKind<'tcx>,
        terminator: &mir::Terminator<'tcx>,
    ) -> bool {
        let matched = match (
            self.patterns[loc_pat.block].terminator(),
            &self.body[loc.block].terminator().kind,
        ) {
            // (&pat::StatementKind::Init(local_pat) ,
            //     mir::TerminatorKind::Call {
            //         destination,
            //         target: Some(target),
            //         ..
            //     }) => destination
            //         .as_local()
            //         .is_some_and(|local| self.match_local(local_pat, local))
            //         .then_some(target),
            (
                &pat::TerminatorKind::Call {
                    func: ref func_pat,
                    args: ref args_pat,
                    target: _,
                    destination: destination_pat,
                },
                &mir::TerminatorKind::Call {
                    ref func,
                    box ref args,
                    target: Some(_),
                    destination,
                    ..
                },
            ) => {
                self.match_operand(func_pat, func)
                    && self.match_operands(args_pat, args)
                    && destination_pat.is_none_or(|destination_pat| self.match_place(destination_pat, destination))
            },
            (
                &pat::TerminatorKind::Drop {
                    place: place_pat,
                    target: _,
                },
                &mir::TerminatorKind::Drop { place, target: _, .. },
            ) => self.match_place(place_pat, place),
            (pat::TerminatorKind::Goto(_), mir::TerminatorKind::Goto { .. })
            | (pat::TerminatorKind::Return, mir::TerminatorKind::Return)
            | (pat::TerminatorKind::PatEnd, _) => true,
            (
                pat::TerminatorKind::SwitchInt { operand, targets: _ },
                mir::TerminatorKind::SwitchInt { discr, targets: _ },
            ) => self.match_operand(operand, discr) && self.match_switch_targets(loc_pat.block, loc.block),
            _ => false,
        };
        if matched {
            info!(?pat, ?terminator.kind, "candidate matched");
        }
        matched
    }

    fn match_switch_targets(&self, bb_pat: pat::BasicBlock, bb: mir::BasicBlock) -> bool {
        let (TerminatorEdges::SwitchInt(pat), TerminatorEdges::SwitchInt(targets)) =
            (&self.pat_cfg[bb_pat], &self.mir_cfg[bb])
        else {
            return false;
        };
        pat.targets.keys().all(|value| targets.targets.contains_key(value))
            && pat.otherwise.is_none_or(|_| targets.otherwise.is_some())
    }

    fn match_rvalue(&self, pat: &pat::Rvalue<'tcx>, rvalue: &mir::Rvalue<'tcx>) -> bool {
        let matched = match (pat, rvalue) {
            // Special case of `Len(*p)` <=> `PtrMetadata(p)`
            (
                &pat::Rvalue::Len(place_pat),
                &mir::Rvalue::UnaryOp(mir::UnOp::PtrMetadata, mir::Operand::Copy(place)),
            ) => {
                if let [pat::PlaceElem::Deref, projection @ ..] = place_pat.projection {
                    let place_pat = pat::Place {
                        local: place_pat.local,
                        projection,
                    };
                    return self.match_place(place_pat, place);
                }
                false
            },
            (
                &pat::Rvalue::UnaryOp(mir::UnOp::PtrMetadata, pat::Operand::Copy(place_pat)),
                &mir::Rvalue::Len(place),
            ) => {
                if let [mir::PlaceElem::Deref, projection @ ..] = place.as_ref().projection {
                    let place = mir::PlaceRef {
                        local: place.local,
                        projection,
                    };
                    return self.match_place_ref(place_pat, place);
                }
                false
            },

            (pat::Rvalue::Use(operand_pat), mir::Rvalue::Use(operand)) => self.match_operand(operand_pat, operand),
            (&pat::Rvalue::Repeat(ref operand_pat, konst_pat), &mir::Rvalue::Repeat(ref operand, konst)) => {
                self.match_operand(operand_pat, operand) && self.match_const(konst_pat, konst)
            },
            (
                &pat::Rvalue::Ref(region_pat, borrow_kind_pat, place_pat),
                &mir::Rvalue::Ref(region, borrow_kind, place),
            ) => {
                self.match_region(region_pat, region)
                    && borrow_kind_pat == borrow_kind
                    && self.match_place(place_pat, place)
            },
            (&pat::Rvalue::RawPtr(mutability_pat, place_pat), &mir::Rvalue::RawPtr(mutability, place)) => {
                mutability_pat == mutability && self.match_place(place_pat, place)
            },
            (&pat::Rvalue::Len(place_pat), &mir::Rvalue::Len(place))
            | (&pat::Rvalue::Discriminant(place_pat), &mir::Rvalue::Discriminant(place))
            | (&pat::Rvalue::CopyForDeref(place_pat), &mir::Rvalue::CopyForDeref(place)) => {
                self.match_place(place_pat, place)
            },
            (
                &pat::Rvalue::Cast(cast_kind_pat, ref operand_pat, ty_pat),
                &mir::Rvalue::Cast(cast_kind, ref operand, ty),
            ) => cast_kind_pat == cast_kind && self.match_operand(operand_pat, operand) && self.match_ty(ty_pat, ty),
            (pat::Rvalue::BinaryOp(op_pat, box [lhs_pat, rhs_pat]), mir::Rvalue::BinaryOp(op, box (lhs, rhs))) => {
                op_pat == op && self.match_operand(lhs_pat, lhs) && self.match_operand(rhs_pat, rhs)
            },
            (&pat::Rvalue::NullaryOp(op_pat, ty_pat), &mir::Rvalue::NullaryOp(op, ty)) => {
                op_pat == op && self.match_ty(ty_pat, ty)
            },
            (pat::Rvalue::UnaryOp(op_pat, operand_pat), mir::Rvalue::UnaryOp(op, operand)) => {
                op_pat == op && self.match_operand(operand_pat, operand)
            },
            (pat::Rvalue::Aggregate(agg_kind_pat, operands_pat), mir::Rvalue::Aggregate(box agg_kind, operands)) => {
                self.match_aggregate(agg_kind_pat, operands_pat, agg_kind, operands)
            },
            (&pat::Rvalue::ShallowInitBox(ref operand_pat, ty_pat), &mir::Rvalue::ShallowInitBox(ref operand, ty)) => {
                self.match_operand(operand_pat, operand) && self.match_ty(ty_pat, ty)
            },
            _ => return false,
        };
        debug!(?pat, ?rvalue, matched, "match_rvalue");
        matched
    }

    fn match_operand(&self, pat: &pat::Operand<'tcx>, operand: &mir::Operand<'tcx>) -> bool {
        let matched = match (pat, operand) {
            (&pat::Operand::Copy(place_pat), &mir::Operand::Copy(place))
            | (&pat::Operand::Move(place_pat), &mir::Operand::Move(place)) => {
                self.match_place_ref(place_pat, place.as_ref())
            },
            (pat::Operand::Constant(konst_pat), mir::Operand::Constant(box konst)) => {
                self.match_const_operand(konst_pat, konst.const_)
            },
            _ => return false,
        };
        debug!(?pat, ?operand, matched, "match_operand");
        matched
    }

    fn match_operands(
        &self,
        pat: &pat::List<pat::Operand<'tcx>>,
        operands: &[rustc_span::source_map::Spanned<mir::Operand<'tcx>>],
    ) -> bool {
        match pat.mode {
            pat::ListMatchMode::Ordered => {
                pat.data.len() == operands.len()
                    && zip(&pat.data, operands)
                        .all(|(operand_pat, operand)| self.match_operand(operand_pat, &operand.node))
            },
            pat::ListMatchMode::Unordered => todo!(),
        }
    }

    // FIXME
    #[allow(clippy::too_many_arguments)]
    fn match_agg_adt(
        &self,
        path: pat::Path<'tcx>,
        def_id: DefId,
        variant_idx: VariantIdx,
        fields: Option<&[Symbol]>,
        field_idx: Option<FieldIdx>,
        operands_pat: &[pat::Operand<'tcx>],
        operands: &IndexSlice<FieldIdx, mir::Operand<'tcx>>,
    ) -> bool {
        let adt = self.tcx.adt_def(def_id);
        let variant = adt.variant(variant_idx);
        let variant_matched = match path {
            pattern::Path::Item(path) => match self.match_item_path(path, def_id) {
                Some([]) => {
                    variant_idx.as_u32() == 0 && matches!(adt.adt_kind(), ty::AdtKind::Struct | ty::AdtKind::Union)
                },
                Some(&[name]) => variant.name == name,
                _ => false,
            },
            pattern::Path::TypeRelative(_ty, _symbol) => false,
            pattern::Path::LangItem(lang_item) => self.tcx.is_lang_item(variant.def_id, lang_item),
        };
        let fields_matched = match (fields, field_idx) {
            (None, None) => variant.ctor.is_some() && self.match_agg_operands(operands_pat, &operands.raw),
            (Some(&[name]), Some(field_idx)) => adt.is_union() && variant.fields[field_idx].name == name,
            (Some(names), None) => {
                let indices = names
                    .iter()
                    .enumerate()
                    .map(|(idx, &name)| (name, idx))
                    .collect::<FxHashMap<_, _>>();
                variant.ctor.is_none()
                    && operands_pat.len() == operands.len()
                    && operands.iter_enumerated().all(|(idx, operand)| {
                        indices
                            .get(&variant.fields[idx].name)
                            .is_some_and(|&idx| self.match_operand(&operands_pat[idx], operand))
                    })
            },
            _ => false,
        };
        let matched = variant_matched && fields_matched;
        debug!(
            ?path,
            ?variant.def_id,
            ?operands_pat,
            ?operands,
            matched,
            "match_agg_adt",
        );
        matched
    }

    fn match_aggregate(
        &self,
        agg_kind_pat: &pat::AggKind<'tcx>,
        operands_pat: &[pat::Operand<'tcx>],
        agg_kind: &mir::AggregateKind<'tcx>,
        operands: &IndexSlice<FieldIdx, mir::Operand<'tcx>>,
    ) -> bool {
        let matched = match (agg_kind_pat, agg_kind) {
            (&pat::AggKind::Array, &mir::AggregateKind::Array(_))
            | (pat::AggKind::Tuple, mir::AggregateKind::Tuple) => self.match_agg_operands(operands_pat, &operands.raw),
            (&pat::AggKind::Adt(path, ref fields), &mir::AggregateKind::Adt(def_id, variant_idx, _, _, field_idx)) => {
                self.match_agg_adt(
                    path,
                    def_id,
                    variant_idx,
                    fields.as_deref(),
                    field_idx,
                    operands_pat,
                    operands,
                )
            },
            (&pat::AggKind::RawPtr(ty_pat, mutability_pat), &mir::AggregateKind::RawPtr(ty, mutability)) => {
                self.match_ty(ty_pat, ty)
                    && mutability_pat == mutability
                    && self.match_agg_operands(operands_pat, &operands.raw)
            },
            _ => false,
        };
        debug!(
            ?agg_kind_pat,
            ?operands_pat,
            ?agg_kind,
            ?operands,
            matched,
            "match_aggregate",
        );
        matched
    }
    fn match_agg_operands(&self, operands_pat: &[pat::Operand<'tcx>], operands: &[mir::Operand<'tcx>]) -> bool {
        operands_pat.len() == operands.len()
            && core::iter::zip(operands_pat, operands)
                .all(|(operand_pat, operand)| self.match_operand(operand_pat, operand))
    }

    fn match_ty(&self, ty_pat: pat::Ty<'tcx>, ty: ty::Ty<'tcx>) -> bool {
        let matched = match (*ty_pat.kind(), *ty.kind()) {
            (pat::TyKind::TyVar(ty_var), _)
                if self.patterns.ty_vars[ty_var].is_none_or(|ty_pred| ty_pred(self.tcx, self.param_env, ty)) =>
            {
                self.ty_vars[ty_var].borrow_mut().push(ty);
                true
            },
            (pat::TyKind::Array(ty_pat, konst_pat), ty::Array(ty, konst)) => {
                self.match_ty(ty_pat, ty) && self.match_const(konst_pat, konst)
            },
            (pat::TyKind::Slice(ty_pat), ty::Slice(ty)) => self.match_ty(ty_pat, ty),
            (pat::TyKind::Tuple(tys_pat), ty::Tuple(tys)) => {
                tys_pat.len() == tys.len() && zip(tys_pat, tys).all(|(&ty_pat, ty)| self.match_ty(ty_pat, ty))
            },
            (pat::TyKind::Ref(region_pat, pat_ty, pat_mutblty), ty::Ref(region, ty, mutblty)) => {
                self.match_region(region_pat, region) && pat_mutblty == mutblty && self.match_ty(pat_ty, ty)
            },
            (pat::TyKind::RawPtr(ty_pat, mutability_pat), ty::RawPtr(ty, mutblty)) => {
                mutability_pat == mutblty && self.match_ty(ty_pat, ty)
            },
            (pat::TyKind::Uint(ty_pat), ty::Uint(ty)) => ty_pat == ty,
            (pat::TyKind::Int(ty_pat), ty::Int(ty)) => ty_pat == ty,
            (pat::TyKind::Float(ty_pat), ty::Float(ty)) => ty_pat == ty,
            (pat::TyKind::Adt(path, args_pat), ty::Adt(adt, args)) => {
                self.match_path(path, adt.did()) && self.match_generic_args(args_pat, args)
            },
            (pat::TyKind::FnDef(path, _args_pat), ty::FnDef(def_id, _args)) => {
                self.match_path(path, def_id) // && self.match_generic_args(args_pat, args)
            },
            (pat::TyKind::Alias(alias_kind_pat, path, args), ty::Alias(alias_kind, alias)) => {
                alias_kind_pat == alias_kind
                    && self.match_path(path, alias.def_id)
                    && self.match_generic_args(args, alias.args)
            },
            (pat::TyKind::Bool, ty::Bool) => true,
            _ => false,
        };
        debug!(?ty_pat, ?ty, matched, "match_ty");
        matched
    }

    fn match_path(&self, path: pat::Path<'tcx>, def_id: DefId) -> bool {
        let matched = match path {
            pat::Path::Item(path) => matches!(self.match_item_path(path, def_id), Some([])),
            pat::Path::TypeRelative(ty, name) => {
                self.tcx.item_name(def_id) == name
                    && self
                        .tcx
                        .opt_parent(def_id)
                        .is_some_and(|did| self.match_ty(ty, self.tcx.type_of(did).instantiate_identity()))
            },
            pat::Path::LangItem(lang_item) => self.tcx.is_lang_item(def_id, lang_item),
        };
        debug!(?path, ?def_id, matched, "match_path");
        matched
    }

    fn match_item_path(&self, path: pat::ItemPath<'tcx>, def_id: DefId) -> Option<&[Symbol]> {
        let &[krate, ref in_crate @ ..] = path.0 else {
            return None;
        };
        let def_path = self.tcx.def_path(def_id);
        let matched = match def_path.krate {
            LOCAL_CRATE => krate == kw::Crate,
            _ => self.tcx.crate_name(def_path.krate) == krate,
        };
        let mut pat_iter = in_crate.iter();
        use DefPathData::{Impl, TypeNs, ValueNs};
        let mut iter = def_path
            .data
            .iter()
            .filter(|data| matches!(data.data, Impl | TypeNs(_) | ValueNs(_)));
        let matched = matched
            && std::iter::zip(pat_iter.by_ref(), iter.by_ref())
                .all(|(&path, data)| data.data.get_opt_name().is_some_and(|name| name == path));
        let matched = matched && iter.next().is_none();
        debug!(?path, ?def_id, matched, "match_item_path");
        matched.then_some(pat_iter.as_slice())
    }

    fn match_generic_args(&self, args_pat: pat::GenericArgsRef<'tcx>, args: ty::GenericArgsRef<'tcx>) -> bool {
        args_pat.len() == args.len()
            && zip(&*args_pat, args).all(|(&arg_pat, arg)| self.match_generic_arg(arg_pat, arg))
    }

    fn match_generic_arg(&self, arg_pat: pat::GenericArgKind<'tcx>, arg: ty::GenericArg<'tcx>) -> bool {
        match (arg_pat, arg.unpack()) {
            (pat::GenericArgKind::Lifetime(region_pat), ty::GenericArgKind::Lifetime(region)) => {
                self.match_region(region_pat, region)
            },
            (pat::GenericArgKind::Type(ty_pat), ty::GenericArgKind::Type(ty)) => self.match_ty(ty_pat, ty),
            (pat::GenericArgKind::Const(konst_pat), ty::GenericArgKind::Const(konst)) => {
                self.match_const(konst_pat, konst)
            },
            _ => false,
        }
    }

    fn match_const(&self, konst_pat: pat::Const, konst: ty::Const<'tcx>) -> bool {
        match (konst_pat, konst.kind()) {
            (pat::Const::ConstVar(const_var), _) => self.match_const_var(const_var, konst),
            (pat::Const::Value(_value_pat), ty::Value(_ty, ty::ValTree::Leaf(_value))) => todo!(),
            _ => false,
        }
    }
    fn match_const_var(&self, const_var: pat::ConstVarIdx, konst: ty::Const<'tcx>) -> bool {
        if let ty::ConstKind::Value(ty, _) = konst.kind()
            && self.match_ty(self.patterns.const_vars[const_var], ty)
        {
            // self.const_vars[const_var].borrow_mut().push(konst);
            return true;
        }
        false
    }

    fn match_const_operand(&self, konst_pat: &pat::ConstOperand<'tcx>, konst: mir::Const<'tcx>) -> bool {
        match (konst_pat, konst) {
            (&pat::ConstOperand::ConstVar(const_var), mir::Const::Ty(_, konst)) => {
                self.match_const_var(const_var, konst)
            },
            (&pat::ConstOperand::ScalarInt(value_pat), mir::Const::Val(mir::ConstValue::Scalar(value), ty)) => {
                (match (value_pat.ty, *ty.kind()) {
                    (pat::IntTy::NegInt(ty_pat), ty::Int(ty)) => ty_pat == ty,
                    (pat::IntTy::Int(ty_pat), ty::Int(ty)) => ty_pat == ty,
                    (pat::IntTy::Uint(ty_pat), ty::Uint(ty)) => ty_pat == ty,
                    (pat::IntTy::Bool, ty::Bool) => true,
                    _ => return false,
                }) && value.to_scalar_int().discard_err().is_some_and(|value| {
                    value_pat.normalize(self.tcx.pointer_size().bytes()) == value.to_bits_unchecked()
                })
            },
            (&pat::ConstOperand::ZeroSized(ty_pat), mir::Const::Val(mir::ConstValue::ZeroSized, ty)) => {
                self.match_ty(ty_pat, ty)
            },
            _ => false,
        }
    }
    fn match_region(&self, pat: pat::RegionKind, region: ty::Region<'tcx>) -> bool {
        matches!(
            (pat, region.kind()),
            (pat::RegionKind::ReStatic, ty::RegionKind::ReStatic) | (pat::RegionKind::ReAny, _)
        )
    }
}
