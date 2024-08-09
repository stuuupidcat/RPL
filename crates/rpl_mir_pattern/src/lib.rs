#![allow(internal_features)]
#![feature(rustc_private)]
#![feature(rustc_attrs)]
#![feature(let_chains)]
#![feature(if_let_guard)]
#![feature(is_none_or)]
#![feature(box_patterns)]
#![feature(try_trait_v2)]

extern crate rustc_data_structures;
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

use rustc_index::bit_set::BitSet;
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
        for (block, block_data) in self.body.basic_blocks.iter_enumerated() {
            if !visited.insert(block) {
                continue;
            }
            self.check_block(block, block_data);
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

    #[instrument(level = "info", skip_all, fields(?block))]
    fn check_block(
        &mut self,
        block: mir::BasicBlock,
        block_data: &'tcx mir::BasicBlockData<'tcx>,
    ) -> Option<mir::TerminatorEdges<'tcx, 'tcx>> {
        debug!("BasicBlock: {}", {
            let mut buffer = Vec::new();
            mir::pretty::write_basic_block(self.tcx, block, self.body, &mut |_, _| Ok(()), &mut buffer).unwrap();
            String::from_utf8_lossy(&buffer).into_owned()
        });
        for (statement_index, statement) in block_data.statements.iter().enumerate() {
            let location = mir::Location { block, statement_index };
            self.check_statement(location, statement);
        }
        self.check_terminator(block, block_data.terminator())
    }

    fn check_statement(&mut self, location: mir::Location, statement: &mir::Statement<'tcx>) {
        self.match_statement(location, statement);
    }
    fn check_terminator(
        &mut self,
        block: mir::BasicBlock,
        terminator: &'tcx mir::Terminator<'tcx>,
    ) -> Option<mir::TerminatorEdges<'tcx, 'tcx>> {
        self.match_terminator(block, terminator).map(|_| terminator.edges())
    }
}

impl<'tcx> CheckMirCtxt<'tcx> {
    pub fn match_local(&self, pat: pat::LocalIdx, local: mir::Local) -> bool {
        self.patterns.match_local(self.tcx, self.body, pat, local)
    }
    pub fn match_place(&self, pat: pat::Place<'tcx>, place: mir::Place<'tcx>) -> bool {
        self.match_place_ref(pat, place.as_ref())
    }
    pub fn match_place_ref(&self, pat: pat::Place<'tcx>, place: mir::PlaceRef<'tcx>) -> bool {
        self.patterns.match_place_ref(self.tcx, self.body, pat, place)
    }

    #[instrument(level = "debug", skip(self))]
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
            debug!("{pat:?} {:?} matched", pattern.kind);
            let new_mat = self.patterns.add_match(pat, pat::MatchKind::Statement(location));
            _ = mat.get_or_insert(new_mat);
        }
        mat
    }

    #[instrument(level = "debug", skip(self, terminator), fields(terminator = ?terminator.kind))]
    pub fn match_terminator(
        &mut self,
        block: mir::BasicBlock,
        terminator: &mir::Terminator<'tcx>,
    ) -> Option<pat::MatchIdx> {
        let mut mat = None;
        for (pat, pattern) in self.patterns.ready_patterns() {
            match pattern.kind {
                pat::PatternKind::Init(local_pat) => match terminator.kind {
                    mir::TerminatorKind::Call {
                        destination,
                        target: Some(target),
                        ..
                    } => {
                        if destination
                            .as_local()
                            .is_some_and(|local| self.match_local(local_pat, local))
                        {
                            debug!("{pat:?} {:?} matched", pattern.kind);
                            let new_mat = self
                                .patterns
                                .add_match(pat, pat::MatchKind::Terminator(block, Some(target)));
                            _ = mat.get_or_insert(new_mat);
                        }
                    },
                    _ => continue,
                },
                pat::PatternKind::Terminator(_) => todo!(),
                _ => continue,
            };
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
                if let [mir::ProjectionElem::Deref, projection @ ..] = place_pat.projection {
                    let place_pat = pat::Place {
                        local: place_pat.local,
                        projection,
                    };
                    return self.match_place(place_pat, place);
                }
                false
            },
            (&pat::Rvalue::UnaryOp(mir::UnOp::PtrMetadata, pat::Copy(place_pat)), &mir::Rvalue::Len(place)) => {
                if let [mir::ProjectionElem::Deref, projection @ ..] = place.as_ref().projection {
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
        debug!(matched, "matching {pat:?} with {rvalue:?}");
        matched
    }

    pub fn match_operand(&self, pat: &pat::Operand<'tcx>, operand: &mir::Operand<'tcx>) -> bool {
        let matched = match (pat, operand) {
            (&pat::Operand::Copy(place_pat), &mir::Operand::Copy(place))
            | (&pat::Operand::Move(place_pat), &mir::Operand::Move(place)) => self.match_place(place_pat, place),
            (pat::Operand::Constant(_), mir::Operand::Constant(_)) => todo!(),
            _ => return false,
        };
        debug!(matched, "matching {pat:?} with {operand:?}");
        matched
    }

    fn match_agg_kind(&self, agg_kind_pat: &pat::AggKind<'tcx>, agg_kind: &mir::AggregateKind<'tcx>) -> bool {
        self.patterns.match_agg_kind(self.tcx, agg_kind_pat, agg_kind)
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
