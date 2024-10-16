#![allow(internal_features)]
#![feature(rustc_private)]
#![feature(rustc_attrs)]
#![feature(let_chains)]
#![feature(if_let_guard)]
#![feature(is_none_or)]
#![feature(box_patterns)]
#![feature(try_trait_v2)]
#![feature(debug_closure_helpers)]

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

pub mod pattern;

use std::iter::zip;

use rustc_index::IndexSlice;
use rustc_middle::ty::TyCtxt;
use rustc_middle::{mir, ty};
use rustc_target::abi::FieldIdx;

pub use crate::pattern as pat;

pub struct CheckMirCtxt<'tcx> {
    tcx: TyCtxt<'tcx>,
    body: &'tcx mir::Body<'tcx>,
    pub patterns: pat::Patterns<'tcx>,
}

impl<'tcx> CheckMirCtxt<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>, body: &'tcx mir::Body<'tcx>, patterns: pat::Patterns<'tcx>) -> Self {
        Self { tcx, body, patterns }
    }
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

impl<'tcx> CheckMirCtxt<'tcx> {
    pub fn match_local(&self, pat: pat::LocalIdx, local: mir::Local) -> bool {
        let matched = self.patterns.match_local(self.tcx, self.body, pat, local);
        debug!(?pat, ?local, matched, "match_local");
        matched
    }
    pub fn match_place(&self, pat: pat::Place<'tcx>, place: mir::Place<'tcx>) -> bool {
        self.match_place_ref(pat, place.as_ref())
    }
    pub fn match_place_ref(&self, pat: pat::Place<'tcx>, place: mir::PlaceRef<'tcx>) -> bool {
        self.patterns.match_place_ref(self.tcx, self.body, pat, place)
    }

    #[instrument(level = "info", skip_all, fields(?pat, ?statement), ret)]
    pub fn match_statement(&mut self, pat: &pat::StatementKind<'tcx>, statement: &mir::Statement<'tcx>) -> bool {
        match (pat, &statement.kind) {
            (&pat::StatementKind::Init(place_pat), &mir::StatementKind::Assign(box (place, _))) => {
                self.match_place(place_pat, place)
            },
            (
                &pat::StatementKind::Assign(place_pat, ref rvalue_pat),
                &mir::StatementKind::Assign(box (place, ref rvalue)),
            ) => self.match_rvalue(rvalue_pat, rvalue) && self.match_place(place_pat, place),
            _ => false,
        }
    }

    #[instrument(level = "info", skip_all, fields(?pat, terminator = ?terminator.kind), ret)]
    pub fn match_terminator(&mut self, pat: &pat::TerminatorKind<'tcx>, terminator: &mir::Terminator<'tcx>) -> bool {
        match (pat, &terminator.kind) {
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
            _ => false,
        }
    }

    pub fn match_rvalue(&self, pat: &pat::Rvalue<'tcx>, rvalue: &mir::Rvalue<'tcx>) -> bool {
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
            (&pat::Rvalue::AddressOf(mutability_pat, place_pat), &mir::Rvalue::AddressOf(mutability, place)) => {
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

    pub fn match_operand(&self, pat: &pat::Operand<'tcx>, operand: &mir::Operand<'tcx>) -> bool {
        self.patterns.match_operand(self.tcx, self.body, pat, operand)
    }

    pub fn match_const_operand(&self, pat: &pat::ConstOperand<'tcx>, operand: mir::Const<'tcx>) -> bool {
        self.patterns.match_const_operand(self.tcx, pat, operand)
    }

    pub fn match_operands(
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

    fn match_aggregate(
        &self,
        agg_kind_pat: &pat::AggKind<'tcx>,
        operands_pat: &[pat::Operand<'tcx>],
        agg_kind: &mir::AggregateKind<'tcx>,
        operands: &IndexSlice<FieldIdx, mir::Operand<'tcx>>,
    ) -> bool {
        self.patterns
            .match_aggregate(self.tcx, self.body, agg_kind_pat, operands_pat, agg_kind, operands)
    }

    fn match_ty(&self, ty_pat: pat::Ty<'tcx>, ty: ty::Ty<'tcx>) -> bool {
        self.patterns.match_ty(self.tcx, ty_pat, ty)
    }
    fn match_const(&self, konst_pat: pat::Const, konst: ty::Const<'tcx>) -> bool {
        self.patterns.match_const(self.tcx, konst_pat, konst)
    }
    fn match_region(&self, region_pat: pat::RegionKind, region: ty::Region<'tcx>) -> bool {
        self.patterns.match_region(region_pat, region)
    }
}
