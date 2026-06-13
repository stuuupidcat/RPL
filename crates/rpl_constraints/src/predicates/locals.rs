use std::fmt;

use rustc_index::IndexVec;
use rustc_middle::mir::{self};
use rustc_middle::ty::{self, TyCtxt, TypingEnv};

pub struct BodyInfoCache {
    /// `null[i]` is `Some(true)` if `i` is null, and `Some(false)` if `i` is not null,
    /// `None` if the information is not available.
    null: IndexVec<mir::Local, Option<bool>>,
    /// `product_of[i][j]` is `Some(true)` if `i` may be a product of `j`, `Some(false)` if `i` may
    /// be a quotient of `j`, and `None` if there is no relationship.
    product_of: IndexVec<mir::Local, IndexVec<mir::Local, Option<bool>>>,
    // /// `derive_from[i][j]` is `true` if `i` may be computed from `j`, `false` if there is no
    // /// relationship.
    // derive_from: IndexVec<mir::Local, IndexVec<mir::Local, bool>>,
}

impl fmt::Debug for BodyInfoCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Null<'a>(&'a IndexVec<mir::Local, Option<bool>>);
        impl fmt::Debug for Null<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_list()
                    .entries(self.0.iter().enumerate().filter_map(|(i, b)| b.map(|b| (i, b))))
                    .finish()
            }
        }

        struct ProductOf<'a>(&'a IndexVec<mir::Local, IndexVec<mir::Local, Option<bool>>>);
        impl fmt::Debug for ProductOf<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_list()
                    .entries(
                        self.0
                            .iter_enumerated()
                            .flat_map(|(i, j)| j.iter_enumerated().filter_map(move |(k, v)| v.map(|b| (i, k, b)))),
                    )
                    .finish()
            }
        }

        f.debug_struct("BodyInfoCache")
            .field("null", &Null(&self.null))
            .field("product_of", &ProductOf(&self.product_of))
            // .field("derive_from", &self.derive_from) // Uncomment if derive_from is implemented
            .finish()
    }
}

impl BodyInfoCache {
    // Helper retained for future use; it currently has no caller after the migration.
    #[allow(dead_code)]
    #[instrument(level = "trace", skip(tcx), ret)]
    fn ty_const_is_null<'tcx>(tcx: TyCtxt<'tcx>, const_: ty::Const<'tcx>) -> Option<bool> {
        let val = const_.try_to_value()?;
        let val = tcx.valtree_to_const_val(val);
        let scalar = val.try_to_scalar()?;
        match scalar {
            mir::interpret::Scalar::Int(i) => Some(i.is_null()),
            mir::interpret::Scalar::Ptr(_, _) => Some(false),
        }
    }
    #[instrument(level = "trace", skip(tcx), ret)]
    fn mir_const_is_null<'tcx>(
        tcx: TyCtxt<'tcx>,
        typing_env: TypingEnv<'tcx>,
        const_: mir::Const<'tcx>,
    ) -> Option<bool> {
        let scalar = const_.try_eval_scalar(tcx, typing_env)?;
        match scalar {
            mir::interpret::Scalar::Int(i) => Some(i.is_null()),
            mir::interpret::Scalar::Ptr(_, _) => Some(false),
        }
    }
    #[instrument(level = "debug", skip(tcx, body), fields(n = body.local_decls.len()), ret)]
    pub fn new<'tcx>(tcx: TyCtxt<'tcx>, typing_env: TypingEnv<'tcx>, body: &mir::Body<'tcx>) -> Self {
        let n = body.local_decls.len();

        let mut null: IndexVec<mir::Local, Option<bool>> = IndexVec::from_elem_n(None, n);
        // Track the product relationship among locals, true for product, false for quotient
        let mut product_of: IndexVec<mir::Local, IndexVec<mir::Local, Option<bool>>> =
            IndexVec::from_fn_n(|_| IndexVec::from_elem_n(None, n), n);
        for i in 0..n {
            let i = mir::Local::from_usize(i);
            product_of[i][i] = Some(true);
        }
        for local in body.basic_blocks.iter() {
            for stmt in &local.statements {
                // Check if the statement is an assignment
                if let mir::StatementKind::Assign(box (ref lhs, ref rhs)) = stmt.kind
                    && let Some(lhs) = lhs.as_local()
                {
                    match rhs {
                        mir::Rvalue::Cast(_, mir::Operand::Constant(box c), _)
                        | mir::Rvalue::Use(mir::Operand::Constant(box c), _) => {
                            null[lhs] = Self::mir_const_is_null(tcx, typing_env, c.const_)
                        },
                        mir::Rvalue::Cast(_, mir::Operand::Copy(rhs) | mir::Operand::Move(rhs), _)
                        | mir::Rvalue::Use(mir::Operand::Copy(rhs) | mir::Operand::Move(rhs), _) => {
                            if let Some(rhs) = rhs.as_local() {
                                null[lhs] = null[rhs];
                            }
                        },
                        _ => {},
                    }
                }
                // Check if the statement is a product or quotient
                if let mir::StatementKind::Assign(box (ref lhs, ref rhs)) = stmt.kind
                    && let Some(lhs) = lhs.as_local()
                {
                    match rhs {
                        mir::Rvalue::BinaryOp(
                            mir::BinOp::Mul
                            | mir::BinOp::MulUnchecked
                            | mir::BinOp::MulWithOverflow
                            | mir::BinOp::Add
                            | mir::BinOp::AddUnchecked
                            | mir::BinOp::AddWithOverflow
                            | mir::BinOp::Sub
                            | mir::BinOp::SubUnchecked
                            | mir::BinOp::SubWithOverflow,
                            box rhs,
                        ) => {
                            if let mir::Operand::Copy(rhs1) | mir::Operand::Move(rhs1) = rhs.0
                                && let Some(rhs1) = rhs1.as_local()
                            {
                                product_of[lhs][rhs1] = Some(true);
                            }
                            if let mir::Operand::Copy(rhs2) | mir::Operand::Move(rhs2) = rhs.1
                                && let Some(rhs2) = rhs2.as_local()
                            {
                                product_of[lhs][rhs2] = Some(true);
                            }
                        },
                        mir::Rvalue::BinaryOp(mir::BinOp::Div, box rhs) => {
                            if let mir::Operand::Copy(rhs1) | mir::Operand::Move(rhs1) = rhs.0
                                && let Some(rhs1) = rhs1.as_local()
                            {
                                product_of[lhs][rhs1] = Some(true);
                            }
                            if let mir::Operand::Copy(rhs2) | mir::Operand::Move(rhs2) = rhs.1
                                && let Some(rhs2) = rhs2.as_local()
                            {
                                product_of[lhs][rhs2] = Some(false);
                            }
                        },
                        mir::Rvalue::Use(mir::Operand::Copy(rhs) | mir::Operand::Move(rhs), _)
                        | mir::Rvalue::Cast(_, mir::Operand::Copy(rhs) | mir::Operand::Move(rhs), _) => {
                            if let Some(rhs) = rhs.as_local() {
                                product_of[lhs][rhs] = Some(true);
                            }
                        },
                        _ => (),
                    }
                }
            }
        }
        for j in 0..n {
            let j = mir::Local::from_usize(j);
            for i in 0..n {
                for k in 0..n {
                    if i == k {
                        continue;
                    }
                    let i = mir::Local::from_usize(i);
                    let k = mir::Local::from_usize(k);
                    if let (Some(s1), Some(s2)) = (product_of[i][j], product_of[j][k]) {
                        product_of[i][k] = Some(s1 == s2);
                    }
                }
            }
        }
        Self { null, product_of }
    }
}

// FIX: consider a more general way for error handling
pub type SingleLocalPredsFnPtr =
    for<'tcx> fn(TyCtxt<'tcx>, ty::TypingEnv<'tcx>, &mir::Body<'tcx>, &BodyInfoCache, mir::Local) -> bool;

/// Check if a local is null
#[instrument(level = "debug", skip(cache), ret)]
pub(crate) fn is_null<'tcx>(
    _: TyCtxt<'tcx>,
    _: ty::TypingEnv<'tcx>,
    _: &mir::Body<'tcx>,
    cache: &BodyInfoCache,
    local: mir::Local,
) -> bool {
    cache.null[local].unwrap_or(false)
}

// FIX: consider a more general way for error handling
pub type MultipleLocalsPredsFnPtr =
    for<'tcx> fn(TyCtxt<'tcx>, ty::TypingEnv<'tcx>, &mir::Body<'tcx>, &BodyInfoCache, Vec<mir::Local>) -> bool;

/// Check if former local is a product of latter local for every two consecutive locals
#[instrument(level = "debug", skip(cache), ret)]
pub(crate) fn product_of<'tcx>(
    _: TyCtxt<'tcx>,
    _: ty::TypingEnv<'tcx>,
    _: &mir::Body<'tcx>,
    cache: &BodyInfoCache,
    locals: Vec<mir::Local>,
) -> bool {
    locals.windows(2).all(|pair| {
        let (first, second) = (pair[0], pair[1]);
        cache.product_of[first][second].unwrap_or(false)
    })
}
