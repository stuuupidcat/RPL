#![allow(internal_features)]
#![feature(rustc_private)]
#![feature(rustc_attrs)]
#![feature(let_chains)]
#![feature(if_let_guard)]
#![feature(is_none_or)]
#![feature(box_patterns)]
#![feature(try_trait_v2)]

extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_fluent_macro;
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

use rustc_index::bit_set::BitSet;
use rustc_index::Idx;
use rustc_middle::ty::TyCtxt;
use rustc_middle::{mir, ty};

pub use crate::pattern as pat;

pub struct CheckMirCtxt<'tcx> {
    tcx: TyCtxt<'tcx>,
    body: &'tcx mir::Body<'tcx>,
    pub patterns: pat::Patterns<'tcx>,
}

impl<'tcx> CheckMirCtxt<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>, body: &'tcx mir::Body<'tcx>) -> Self {
        Self {
            tcx,
            body,
            patterns: Default::default(),
        }
    }
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

    #[instrument(level = "info", skip(self))]
    pub fn match_statement(
        &mut self,
        location: mir::Location,
        statement: &mir::Statement<'tcx>,
    ) -> Option<pat::MatchIdx> {
        let mut mat = None;
        for (pat, pattern) in self.patterns.ready_patterns() {
            let matched = match pattern.kind {
                pat::PatternKind::Init(local_pat) => match statement.kind {
                    mir::StatementKind::Assign(box (place, _)) => {
                        place.as_local().is_some_and(|local| self.match_local(local_pat, local))
                    },
                    _ => continue,
                },
                pat::PatternKind::Statement(ref statement_pat) => match (statement_pat, &statement.kind) {
                    (
                        &pat::StatementKind::Assign(place_pat, ref rvalue_pat),
                        &mir::StatementKind::Assign(box (place, ref rvalue)),
                    ) => self.match_rvalue(rvalue_pat, rvalue) && self.match_place(place_pat, place),
                    _ => continue,
                },
                _ => continue,
            };
            if !matched {
                continue;
            }
            info!("matched {pat:?}: {:?}", pattern.kind);
            let new_mat = self.patterns.add_match(pat, pat::MatchKind::Statement(location));
            _ = mat.get_or_insert(new_mat);
        }
        mat
    }

    #[instrument(level = "info", skip(self, terminator), fields(terminator = ?terminator.kind))]
    pub fn match_terminator(
        &mut self,
        block: mir::BasicBlock,
        terminator: &mir::Terminator<'tcx>,
    ) -> Option<pat::MatchIdx> {
        let mut mat = None;
        for (pat, pattern) in self.patterns.ready_patterns() {
            let target = match &pattern.kind {
                &pat::PatternKind::Init(local_pat) => match terminator.kind {
                    mir::TerminatorKind::Call {
                        destination,
                        target: Some(target),
                        ..
                    } => destination
                        .as_local()
                        .is_some_and(|local| self.match_local(local_pat, local))
                        .then_some(target),
                    _ => continue,
                },
                pat::PatternKind::Terminator(terminator_pat) => match (terminator_pat, &terminator.kind) {
                    (
                        &pattern::TerminatorKind::Call {
                            func: ref func_pat,
                            args: ref args_pat,
                            destination: destination_pat,
                        },
                        &mir::TerminatorKind::Call {
                            ref func,
                            box ref args,
                            target: Some(target),
                            destination,
                            ..
                        },
                    ) => (self.match_operand(func_pat, func)
                        && self.match_operands(args_pat, args)
                        && self.match_place(destination_pat, destination))
                    .then_some(target),
                    (
                        &pattern::TerminatorKind::Drop { place: place_pat },
                        &mir::TerminatorKind::Drop { place, target, .. },
                    ) => self.match_place(place_pat, place).then_some(target),
                    _ => continue,
                },
                _ => continue,
            };
            if let Some(target) = target {
                info!("matched {pat:?}: {:?}", pattern.kind);
                let new_mat = self
                    .patterns
                    .add_match(pat, pat::MatchKind::Terminator(block, Some(target)));
                _ = mat.get_or_insert(new_mat);
            }
        }
        mat
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
            (&pat::Rvalue::UnaryOp(mir::UnOp::PtrMetadata, pat::Copy(place_pat)), &mir::Rvalue::Len(place)) => {
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
                self.match_agg_kind(agg_kind_pat, agg_kind)
                    && operands_pat.len() == operands.len()
                    && core::iter::zip(operands_pat, operands)
                        .all(|(operand_pat, operand)| self.match_operand(operand_pat, operand))
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
        let matched = match (pat, operand) {
            (&pat::Operand::Copy(place_pat), &mir::Operand::Copy(place))
            | (&pat::Operand::Move(place_pat), &mir::Operand::Move(place)) => self.match_place(place_pat, place),
            (pat::Operand::Constant(konst_pat), mir::Operand::Constant(box konst)) => {
                self.match_const_operand(konst_pat, konst.const_)
            },
            _ => return false,
        };
        debug!(?pat, ?operand, matched, "match_operand");
        matched
    }

    pub fn match_const_operand(&self, pat: &pat::ConstOperand<'tcx>, operand: mir::Const<'tcx>) -> bool {
        match (pat, operand) {
            (&pat::ConstOperand::Ty(ty_pat, konst_pat), mir::Const::Ty(ty, konst)) => {
                self.match_ty(ty_pat, ty) && self.match_const(konst_pat, konst)
            },
            (&pat::ConstOperand::Val(value_pat, ty_pat), mir::Const::Val(value, ty)) => {
                self.match_const_value(value_pat, value) && self.match_ty(ty_pat, ty)
            },
            _ => false,
        }
    }

    pub fn match_const_value(&self, pat: pat::ConstValue, value: mir::ConstValue<'tcx>) -> bool {
        match (pat, value) {
            (pattern::ConstValue::Scalar(scalar_pat), mir::ConstValue::Scalar(scalar)) => scalar_pat == scalar,
            (pattern::ConstValue::ZeroSized, mir::ConstValue::ZeroSized) => true,
            _ => false,
        }
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

    fn match_agg_kind(&self, pat: &pat::AggKind<'tcx>, agg_kind: &mir::AggregateKind<'tcx>) -> bool {
        self.patterns.match_agg_kind(self.tcx, pat, agg_kind)
    }

    fn match_ty(&self, ty_pat: pat::Ty<'tcx>, ty: ty::Ty<'tcx>) -> bool {
        self.patterns.match_ty(self.tcx, ty_pat, ty)
    }
    fn match_const(&self, konst_pat: pat::Const<'tcx>, konst: ty::Const<'tcx>) -> bool {
        self.patterns.match_const(self.tcx, konst_pat, konst)
    }
    fn match_region(&self, region_pat: pat::RegionKind, region: ty::Region<'tcx>) -> bool {
        self.patterns.match_region(region_pat, region)
    }
}
