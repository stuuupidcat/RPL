#![feature(rustc_private)]
#![warn(unused_qualifications)]
extern crate rustc_data_structures;
extern crate rustc_errors;
extern crate rustc_fluent_macro;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_lint_defs;
extern crate rustc_macros;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;
#[macro_use]
extern crate tracing;
extern crate either;

rustc_fluent_macro::fluent_messages! { "../messages.en.ftl" }

use std::borrow::Cow;
use std::cell::RefCell;
use std::convert::identity;

use either::Either;
use rpl_config::RawOpInstance;
use rpl_constraints::predicates::BodyInfoCache;
use rpl_context::PatCtxt;
use rpl_context::pat::DynamicError;
use rpl_context::pat::ops_resolved::{OpsConfig, resolve_ops_config};
use rpl_match::graph::{MirControlFlowGraph, MirDataDepGraph};
use rpl_match::matches::Matched;
use rpl_match::matches::artifact::NormalizedMatched;
use rpl_match::mir::pat::PatternItem;
use rpl_match::mir::{CheckMirCtxt, pat};
use rpl_match::predicate_evaluator::PredicateEvaluator;
use rpl_meta::context::MetaContext;
use rustc_data_structures::fx::{FxHashMap, FxHashSet};
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{self as hir, FnHeader};
use rustc_lint_defs::RegisteredTools;
use rustc_macros::{Diagnostic, LintDiagnostic};
use rustc_middle::hir::nested_filter;
use rustc_middle::mir;
use rustc_middle::ty::{self, TyCtxt};
use rustc_middle::util::Providers;
use rustc_session::declare_tool_lint;
use rustc_span::symbol::Ident;
use rustc_span::{Span, Symbol};

// ---------------------------------------------------------------------------
// Cartesian-product helper
// ---------------------------------------------------------------------------

/// Generate the cartesian product of a sequence of factors.
///
/// - Empty input (no factors): yields a single empty combination.
/// - Any factor that is empty: yields zero combinations (fold-on-empty semantics).
/// - Otherwise: yields every combination of one element from each factor,
///   with the rightmost index varying fastest (odometer order).
///
/// The "fold on empty instances" property falls out of the helper: an empty
/// factor folds the product to zero combinations, which contributes the empty
/// match-set to set-op composition (Task 11 spec).
pub fn cartesian<T: Clone, I, II>(factors: II) -> CartesianIter<T>
where
    I: Iterator<Item = T>,
    II: IntoIterator<Item = I>,
{
    let factors: Vec<Vec<T>> = factors.into_iter().map(|i| i.collect()).collect();

    if factors.is_empty() {
        // No factors → yield one empty combination.
        return CartesianIter { factors: Vec::new(), indices: Vec::new(), done: false, empty_fold: false };
    }
    if factors.iter().any(|f| f.is_empty()) {
        // Any empty factor → fold to zero combos.
        return CartesianIter { factors: Vec::new(), indices: Vec::new(), done: true, empty_fold: true };
    }

    let n = factors.len();
    CartesianIter { factors, indices: vec![0usize; n], done: false, empty_fold: false }
}

/// Iterator returned by [`cartesian`].
pub struct CartesianIter<T> {
    factors: Vec<Vec<T>>,
    indices: Vec<usize>,
    done: bool,
    /// `true` when an empty factor forced the product to zero — done before any yield.
    empty_fold: bool,
}

impl<T: Clone> Iterator for CartesianIter<T> {
    type Item = Vec<T>;

    fn next(&mut self) -> Option<Vec<T>> {
        if self.done || self.empty_fold {
            return None;
        }
        if self.factors.is_empty() {
            // No-factors case: one empty combo, then done.
            self.done = true;
            return Some(Vec::new());
        }

        let combo: Vec<T> = self.factors.iter().zip(&self.indices).map(|(f, &i)| f[i].clone()).collect();

        // Increment odometer (rightmost index varies fastest).
        let mut k = self.factors.len();
        loop {
            if k == 0 {
                self.done = true;
                break;
            }
            k -= 1;
            self.indices[k] += 1;
            if self.indices[k] < self.factors[k].len() {
                break;
            }
            self.indices[k] = 0;
        }
        Some(combo)
    }
}

// ---------------------------------------------------------------------------
// ResolvedOpBindings — one (group → instance) assignment for a cartesian combo
// ---------------------------------------------------------------------------

use rpl_context::pat::ops_resolved::{ResolvedOpBindings, ResolvedOpInstance};

// ---------------------------------------------------------------------------

#[cfg(feature = "timing")]
mod errors;

#[cfg(feature = "timing")]
pub use errors::{TIMING, Timing};

declare_tool_lint! {
    /// The `rpl::error_found` lint detects an error.
    ///
    /// ### Example
    ///
    /// ```rust
    /// ```
    ///
    /// {{produces}}
    ///
    /// ### Explanation
    ///
    /// This lint detects an error.
    pub rpl::ERROR_FOUND,
    Deny,
    "detects an error"
}

#[derive(Diagnostic, LintDiagnostic)]
#[diag(rpl_driver_error_found_with_pattern)]
pub struct ErrorFound;

impl From<ErrorFound> for rustc_errors::DiagMessage {
    fn from(_: ErrorFound) -> Self {
        Self::Str(Cow::Borrowed("An error was found with input RPL pattern(s)"))
    }
}

pub fn provide(providers: &mut Providers) {
    providers.registered_tools = registered_tools;
}

fn registered_tools(tcx: TyCtxt<'_>, (): ()) -> RegisteredTools {
    let mut registered_tools = (rustc_interface::DEFAULT_QUERY_PROVIDERS.registered_tools)(tcx, ());
    registered_tools.insert(Ident::from_str("rpl"));
    registered_tools
}

pub fn check_crate<'tcx, 'pcx, 'mcx: 'pcx>(tcx: TyCtxt<'tcx>, pcx: PatCtxt<'pcx>, mctx: &'mcx MetaContext<'mcx>) {
    #[cfg(feature = "timing")]
    let start = std::time::Instant::now();

    pcx.add_parsed_patterns(mctx);

    #[cfg(feature = "timing")]
    {
        use rustc_hir::def_id::CrateNum;

        use crate::errors::TIMING;

        let time = start.elapsed().as_nanos().try_into().unwrap();
        let hir_id = rustc_hir::hir_id::CRATE_HIR_ID;
        let crate_name = tcx.crate_name(CrateNum::ZERO);
        tcx.emit_node_span_lint(
            TIMING,
            hir_id,
            tcx.hir().span(hir_id),
            Timing {
                time,
                stage: "add_parsed_patterns",
                crate_name,
            },
        );
    }

    #[cfg(feature = "timing")]
    let start = std::time::Instant::now();

    // Resolve raw ops from rpl.toml into typed bindings against the loaded patterns.
    // `load_raw_ops` returns an empty map when there is no rpl.toml or no [ops] table.
    let raw_ops: Vec<(String, Vec<RawOpInstance>)> =
        match rpl_config::load_raw_ops(None) {
            Ok(map) => map.into_iter().collect(),
            Err(err) => {
                // Config loading failure is non-fatal: surface as a warning and continue
                // without any op instances.
                tcx.dcx().warn(format!("rpl: failed to load rpl.toml ops: {err}"));
                Vec::new()
            },
        };

    // _ = tcx.hir_crate_items(()).par_items(|item_id| {
    //     check_item(tcx, pcx, item_id);
    //     Ok(())
    // });

    let mut check_ctxt = CheckFnCtxt {
        tcx,
        pcx,
        body_caches: RefCell::default(),
        raw_ops,
    };
    tcx.hir().walk_toplevel_module(&mut check_ctxt);
    rpl_utils::visit_crate(tcx);

    #[cfg(feature = "timing")]
    {
        use rustc_hir::def_id::CrateNum;

        use crate::errors::TIMING;

        let time = start.elapsed().as_nanos().try_into().unwrap();
        let hir_id = rustc_hir::hir_id::CRATE_HIR_ID;
        let crate_name = tcx.crate_name(CrateNum::ZERO);
        tcx.emit_node_span_lint(
            TIMING,
            hir_id,
            tcx.hir().span(hir_id),
            Timing {
                time,
                stage: "do_match",
                crate_name,
            },
        );
    }
}

// pub fn check_item(tcx: TyCtxt<'_>, pcx: PatCtxt<'_>, item_id: hir::ItemId) {
//     let item = tcx.hir().item(item_id);
//     // let def_id = item_id.owner_id.def_id;
//     let mut check_ctxt = CheckFnCtxt { tcx, pcx };
//     check_ctxt.visit_item(item);
// }

/// Used for finding pattern matches in given Rust crate.
struct CheckFnCtxt<'pcx, 'tcx> {
    tcx: TyCtxt<'tcx>,
    pcx: PatCtxt<'pcx>,
    body_caches: RefCell<FxHashMap<DefId, BodyInfoCache>>,
    /// Raw op-group instances loaded from `rpl.toml` (empty when no file exists).
    /// Stored as a pre-collected `Vec` so `resolve_ops_config` can be called
    /// inside each `for_each_rpl_pattern` closure without lifetime trouble.
    raw_ops: Vec<(String, Vec<RawOpInstance>)>,
}

impl<'tcx> Visitor<'tcx> for CheckFnCtxt<'_, 'tcx> {
    type NestedFilter = nested_filter::All;
    fn nested_visit_map(&mut self) -> Self::Map {
        self.tcx.hir()
    }

    #[instrument(level = "debug", skip_all, fields(item_id = ?item.owner_id.def_id))]
    fn visit_item(&mut self, item: &'tcx hir::Item<'tcx>) -> Self::Result {
        match item.kind {
            // hir::ItemKind::Trait(hir::IsAuto::No, hir::Safety::Safe, ..) | hir::ItemKind::Fn { .. } => {},
            hir::ItemKind::Trait(_, _, _, _, impl_) => {
                for trait_item in impl_ {
                    self.check_trait_item_ref(trait_item, None);
                }
            },
            hir::ItemKind::Impl(impl_) => self.check_impl(
                impl_,
                Some(self.tcx.type_of(item.owner_id.def_id).instantiate_identity()),
            ),
            // hir::ItemKind::Fn { sig, .. } => self.check_fn(
            //     Some(item.ident),
            //     &sig.decl,
            //     Some(sig.header),
            //     sig.decl.implicit_self.has_implicit_self(),
            //     item.owner_id.def_id,
            // ),
            // hir::ItemKind::Struct(struct_, generics) => self.check_struct(item.owner_id.def_id, struct_, generics),
            // hir::ItemKind::Enum(enum_, generics) => self.check_enum(item.owner_id.def_id, enum_, generics),
            _ => {},
        }
        intravisit::walk_item(self, item);
    }

    #[instrument(level = "debug", skip(self, kind, decl, body_id, _span))]
    fn visit_fn(
        &mut self,
        kind: intravisit::FnKind<'tcx>,
        decl: &'tcx hir::FnDecl<'tcx>,
        body_id: hir::BodyId,
        _span: Span,
        def_id: LocalDefId,
    ) -> Self::Result {
        let (name, header) = match kind {
            intravisit::FnKind::ItemFn(name, _, fn_header) => (Some(name), Some(fn_header)),
            intravisit::FnKind::Method(name, fn_sig) => (Some(name), Some(fn_sig.header)),
            intravisit::FnKind::Closure => (None, None),
        };

        let self_ty = self
            .tcx
            .impl_of_method(def_id.into())
            .map(|impl_| self.tcx.type_of(impl_).instantiate_identity());

        self.check_fn(
            name,
            decl,
            header,
            decl.implicit_self.has_implicit_self(),
            self_ty,
            def_id,
        );

        let attrs: Vec<_> = self
            .tcx
            .get_attrs_by_path(def_id.to_def_id(), &[Symbol::intern("rpl"), Symbol::intern("dynamic")])
            .collect();
        for attr in &attrs {
            let error = DynamicError::from_attr(attr, self.tcx.def_span(def_id.to_def_id()));
            self.tcx.emit_node_span_lint(
                error.lint(),
                self.tcx.local_def_id_to_hir_id(def_id),
                error.primary_span().clone(),
                error,
            );
        }

        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
}

impl<'tcx, 'pcx> CheckFnCtxt<'pcx, 'tcx> {
    #[expect(clippy::too_many_arguments)]
    #[instrument(level = "trace", skip(self, rpl_rust_items, header, body, mir_cfg, mir_ddg), fields(pat_name = ?name))]
    fn impl_matched<'a>(
        &self,
        name: Symbol,
        rpl_rust_items: &'pcx pat::RustItems<'pcx>,
        def_id: LocalDefId,
        header: Option<FnHeader>,
        has_self: bool,
        self_ty: Option<ty::Ty<'tcx>>,
        body: &'a mir::Body<'tcx>,
        mir_cfg: &'a MirControlFlowGraph,
        mir_ddg: &'a MirDataDepGraph,
        op_bindings: ResolvedOpBindings,
    ) -> impl Iterator<Item = NormalizedMatched<'tcx>> {
        let iter = rpl_rust_items.impls.values().flat_map(move |impl_pat| {
            // FIXME: check impl_pat.ty and impl_pat.trait_id
            let op_bindings = op_bindings.clone();
            impl_pat
                .fns
                .values()
                .filter(move |fn_pat| fn_pat.filter(self.tcx, def_id, header, body))
                .filter_map(move |fn_pat| Some((fn_pat, fn_pat.extra_span(self.tcx, def_id)?)))
                .flat_map(move |(fn_pat, attr_map)| {
                    // FIXME: sometimes we need to check function name
                    // if *fn_name != impl_item.ident.name {
                    //     continue;
                    // }

                    CheckMirCtxt::new_with_bindings(
                        self.tcx,
                        self.pcx,
                        body,
                        has_self,
                        self_ty,
                        rpl_rust_items,
                        name,
                        fn_pat,
                        mir_cfg,
                        mir_ddg,
                        op_bindings.clone(),
                    )
                    .check()
                    .into_iter()
                    .filter(move |matched| self.check_constraints(name, fn_pat, body, matched))
                    .map(move |matched| {
                        let labels = &fn_pat.expect_body().labels;
                        (matched, labels, attr_map.clone())
                    })
                })
                .map(|(matched, label_map, attr_map)| NormalizedMatched::new(&matched, label_map, &attr_map))
        });
        rpl_rust_items.post_process(iter)
    }

    #[instrument(level = "trace", skip(self, ops, pat_op, header, body, mir_cfg, mir_ddg), fields(pat_name = ?name))]
    #[expect(clippy::too_many_arguments)]
    fn impl_matched_pat_op<'a>(
        &self,
        name: Symbol,
        ops: &OpsConfig,
        pat_op: &pat::PatternOperation<'pcx>,
        def_id: LocalDefId,
        header: Option<FnHeader>,
        has_self: bool,
        self_ty: Option<ty::Ty<'tcx>>,
        body: &'a mir::Body<'tcx>,
        mir_cfg: &'a MirControlFlowGraph,
        mir_ddg: &'a MirDataDepGraph,
    ) -> impl Iterator<Item = NormalizedMatched<'tcx>> {
        // Determine which op groups any operand of this PatternOperation references.
        let groups_used: Vec<Symbol> = pat_op.referenced_op_groups().into_iter().collect();

        // Build the cartesian product of (group → instance choices).
        // If groups_used is empty, cartesian yields one empty combo and the
        // behavior is identical to the original (no-op-groups) path.
        // If any group has zero instances, cartesian yields zero combos and the
        // match-set is empty (fold-on-no-instances).
        let factors: Vec<Vec<&ResolvedOpInstance>> = groups_used
            .iter()
            .map(|g| ops.instances_of(g.as_str()).iter().collect())
            .collect();

        let mut all: Vec<NormalizedMatched<'tcx>> = Vec::new();
        for combo in cartesian(factors.into_iter().map(|v| v.into_iter())) {
            let bindings = ResolvedOpBindings::from_combo(&groups_used, combo);
            all.extend(self.impl_matched_pat_op_with_bindings(
                name, ops, pat_op, &bindings, def_id, header, has_self, self_ty, body, mir_cfg, mir_ddg,
            ));
        }
        all.into_iter()
    }

    /// Inner body of `impl_matched_pat_op`, lifted out so the cartesian loop
    /// can call it once per (group → instance) combination.
    ///
    /// `bindings` carries the current combo assignment; for patterns that
    /// reference no op groups `bindings` is empty and behavior is identical
    /// to the original implementation.
    #[expect(clippy::too_many_arguments)]
    fn impl_matched_pat_op_with_bindings<'a>(
        &self,
        _name: Symbol,
        ops: &OpsConfig,
        pat_op: &pat::PatternOperation<'pcx>,
        bindings: &ResolvedOpBindings,
        def_id: LocalDefId,
        header: Option<FnHeader>,
        has_self: bool,
        self_ty: Option<ty::Ty<'tcx>>,
        body: &'a mir::Body<'tcx>,
        mir_cfg: &'a MirControlFlowGraph,
        mir_ddg: &'a MirDataDepGraph,
    ) -> impl Iterator<Item = NormalizedMatched<'tcx>> {
        let positive: Vec<_> = pat_op
            .positive
            .iter()
            .flat_map(|positive| {
                self.impl_matched_pat_item(
                    positive.0, ops, positive.1, bindings, def_id, header, has_self, self_ty, body, mir_cfg, mir_ddg,
                )
                .map(|matched| matched.map(&positive.2))
            })
            .collect();
        let negative: FxHashSet<_> = pat_op
            .negative
            .iter()
            .flat_map(|negative| {
                self.impl_matched_pat_item(
                    negative.0, ops, negative.1, bindings, def_id, header, has_self, self_ty, body, mir_cfg, mir_ddg,
                )
                .map(|matched| matched.map(&negative.2))
            })
            .collect();
        debug!(?positive, ?negative, "impl_matched_pat_op_with_bindings");

        let iter = positive
            .into_iter()
            .filter(move |matched| {
                debug_assert!(negative.iter().all(|neg| neg.has_same_head(matched)));
                !negative.contains(matched)
            })
            .collect::<Vec<_>>()
            .into_iter();

        pat_op.post_process(iter)
    }

    #[expect(clippy::too_many_arguments)]
    fn impl_matched_pat_item<'a>(
        &self,
        name: Symbol,
        ops: &OpsConfig,
        pat_item: &'pcx PatternItem<'pcx>,
        bindings: &ResolvedOpBindings,
        def_id: LocalDefId,
        header: Option<FnHeader>,
        has_self: bool,
        self_ty: Option<ty::Ty<'tcx>>,
        body: &'a mir::Body<'tcx>,
        mir_cfg: &'a MirControlFlowGraph,
        mir_ddg: &'a MirDataDepGraph,
    ) -> impl Iterator<Item = NormalizedMatched<'tcx>> {
        match pat_item {
            PatternItem::RustItems(rust_items) => Either::Left(self.impl_matched(
                name, rust_items, def_id, header, has_self, self_ty, body, mir_cfg, mir_ddg, bindings.clone(),
            )),
            PatternItem::RPLPatternOperation(pat_op) => Either::Right(
                self.impl_matched_pat_op(name, ops, pat_op, def_id, header, has_self, self_ty, body, mir_cfg, mir_ddg),
            ),
        }
    }

    #[expect(clippy::too_many_arguments)]
    #[instrument(level = "trace", skip(self, rpl_rust_items, header, body, mir_cfg, mir_ddg), fields(pat_name = ?name))]
    fn fn_matched<'a>(
        &self,
        name: Symbol,
        rpl_rust_items: &'pcx pat::RustItems<'pcx>,
        def_id: LocalDefId,
        header: Option<FnHeader>,
        has_self: bool,
        self_ty: Option<ty::Ty<'tcx>>,
        body: &'a mir::Body<'tcx>,
        mir_cfg: &'a MirControlFlowGraph,
        mir_ddg: &'a MirDataDepGraph,
        op_bindings: ResolvedOpBindings,
    ) -> impl Iterator<Item = NormalizedMatched<'tcx>> {
        let iter = rpl_rust_items
            .fns
            .iter()
            .filter(move |fn_pat| fn_pat.filter(self.tcx, def_id, header, body))
            .filter_map(move |fn_pat| Some((fn_pat, fn_pat.extra_span(self.tcx, def_id)?)))
            .flat_map(move |(fn_pat, attr_map)| {
                CheckMirCtxt::new_with_bindings(
                    self.tcx,
                    self.pcx,
                    body,
                    has_self,
                    self_ty,
                    rpl_rust_items,
                    name,
                    fn_pat,
                    mir_cfg,
                    mir_ddg,
                    op_bindings.clone(),
                )
                .check()
                .into_iter()
                .filter(move |matched| self.check_constraints(name, fn_pat, body, matched))
                .map(move |matched| {
                    let labels = &fn_pat.expect_body().labels;
                    (matched, labels, attr_map.clone())
                })
            })
            .map(|(matched, label_map, attr_map)| NormalizedMatched::new(&matched, label_map, &attr_map));

        rpl_rust_items.post_process(iter)
    }

    #[instrument(level = "trace", skip(self, ops, pat_op, header, body, mir_cfg, mir_ddg), fields(pat_name = ?name))]
    #[expect(clippy::too_many_arguments)]
    fn fn_matched_pat_op<'a>(
        &self,
        name: Symbol,
        ops: &OpsConfig,
        pat_op: &pat::PatternOperation<'pcx>,
        def_id: LocalDefId,
        header: Option<FnHeader>,
        has_self: bool,
        self_ty: Option<ty::Ty<'tcx>>,
        body: &'a mir::Body<'tcx>,
        mir_cfg: &'a MirControlFlowGraph,
        mir_ddg: &'a MirDataDepGraph,
    ) -> impl Iterator<Item = NormalizedMatched<'tcx>> {
        // Determine which op groups any operand of this PatternOperation references.
        let groups_used: Vec<Symbol> = pat_op.referenced_op_groups().into_iter().collect();

        // Cartesian product over op-group instance vectors (see impl_matched_pat_op).
        let factors: Vec<Vec<&ResolvedOpInstance>> = groups_used
            .iter()
            .map(|g| ops.instances_of(g.as_str()).iter().collect())
            .collect();

        let mut all: Vec<NormalizedMatched<'tcx>> = Vec::new();
        for combo in cartesian(factors.into_iter().map(|v| v.into_iter())) {
            let bindings = ResolvedOpBindings::from_combo(&groups_used, combo);
            all.extend(self.fn_matched_pat_op_with_bindings(
                name, ops, pat_op, &bindings, def_id, header, has_self, self_ty, body, mir_cfg, mir_ddg,
            ));
        }
        all.into_iter()
    }

    /// Inner body of `fn_matched_pat_op`, lifted out so the cartesian loop
    /// can call it once per (group → instance) combination.
    #[expect(clippy::too_many_arguments)]
    fn fn_matched_pat_op_with_bindings<'a>(
        &self,
        _name: Symbol,
        ops: &OpsConfig,
        pat_op: &pat::PatternOperation<'pcx>,
        bindings: &ResolvedOpBindings,
        def_id: LocalDefId,
        header: Option<FnHeader>,
        has_self: bool,
        self_ty: Option<ty::Ty<'tcx>>,
        body: &'a mir::Body<'tcx>,
        mir_cfg: &'a MirControlFlowGraph,
        mir_ddg: &'a MirDataDepGraph,
    ) -> impl Iterator<Item = NormalizedMatched<'tcx>> {
        let positive: Vec<_> = pat_op
            .positive
            .iter()
            .flat_map(|positive| {
                self.fn_matched_pat_item(
                    positive.0, ops, positive.1, bindings, def_id, header, has_self, self_ty, body, mir_cfg, mir_ddg,
                )
                .map(|matched| matched.map(&positive.2))
            })
            .collect();
        let negative: FxHashSet<_> = pat_op
            .negative
            .iter()
            .flat_map(|negative| {
                self.fn_matched_pat_item(
                    negative.0, ops, negative.1, bindings, def_id, header, has_self, self_ty, body, mir_cfg, mir_ddg,
                )
                .map(|matched| matched.map(&negative.2))
            })
            .collect();
        debug!(?positive, ?negative, "fn_matched_pat_op_with_bindings");

        let iter = positive
            .into_iter()
            .filter(move |matched| {
                debug_assert!(negative.iter().all(|neg| neg.has_same_head(matched)));
                !negative.contains(matched)
            })
            .collect::<Vec<_>>()
            .into_iter();

        pat_op.post_process(iter)
    }

    #[expect(clippy::too_many_arguments)]
    fn fn_matched_pat_item<'a>(
        &self,
        name: Symbol,
        ops: &OpsConfig,
        pat_item: &'pcx PatternItem<'pcx>,
        bindings: &ResolvedOpBindings,
        def_id: LocalDefId,
        header: Option<FnHeader>,
        has_self: bool,
        self_ty: Option<ty::Ty<'tcx>>,
        body: &'a mir::Body<'tcx>,
        mir_cfg: &'a MirControlFlowGraph,
        mir_ddg: &'a MirDataDepGraph,
    ) -> impl Iterator<Item = NormalizedMatched<'tcx>> {
        match pat_item {
            PatternItem::RustItems(rust_items) => Either::Left(self.fn_matched(
                name, rust_items, def_id, header, has_self, self_ty, body, mir_cfg, mir_ddg, bindings.clone(),
            )),
            PatternItem::RPLPatternOperation(pat_op) => Either::Right(
                self.fn_matched_pat_op(name, ops, pat_op, def_id, header, has_self, self_ty, body, mir_cfg, mir_ddg),
            ),
        }
    }

    #[instrument(level = "debug", skip(self, fn_pat, body), fields(pat_name = ?name, fn_name = ?fn_pat.name, constraints = ?fn_pat.constraints), ret)]
    fn check_constraints(
        &self,
        name: Symbol,
        fn_pat: &pat::FnPattern<'pcx>,
        body: &mir::Body<'tcx>,
        matched: &Matched<'tcx>,
    ) -> bool {
        let mut cache = self.body_caches.borrow_mut();
        let typing_env = ty::TypingEnv::post_analysis(self.tcx, body.source.def_id());
        let cache = cache
            .entry(body.source.def_id())
            .or_insert_with(|| BodyInfoCache::new(self.tcx, typing_env, body));
        let evaluator = PredicateEvaluator::new(
            self.tcx,
            typing_env,
            body,
            &fn_pat.expect_body().labels,
            matched,
            cache,
            fn_pat.symbol_table,
        );
        evaluator.evaluate_constraint(&fn_pat.constraints)
    }
}

impl<'tcx> CheckFnCtxt<'_, 'tcx> {
    #[instrument(level = "debug", skip(self, trait_item), fields(trait_item_id = ?trait_item.id))]
    fn check_trait_item_ref(&mut self, trait_item: &'tcx hir::TraitItemRef, self_ty: Option<ty::Ty<'tcx>>) {
        if let hir::AssocItemKind::Fn { has_self } = trait_item.kind {
            let id = trait_item.id;
            let trait_item = self.tcx.hir().trait_item(id);
            let def_id = trait_item.owner_id.def_id;
            match trait_item.kind {
                hir::TraitItemKind::Fn(sig, _) => {
                    self.check_assoc_fn(has_self, self_ty, &sig, def_id);
                },
                _ => (), // Actually impossible, but we handle it gracefully.
            }
        }
    }
    #[instrument(level = "debug", skip(self, impl_))]
    fn check_impl(&mut self, impl_: &hir::Impl<'tcx>, self_ty: Option<ty::Ty<'tcx>>) {
        for impl_item in impl_.items {
            self.check_impl_item_ref(impl_item, self_ty);
        }
    }
    #[instrument(level = "debug", skip(self, impl_item), fields(impl_item_id = ?impl_item.id))]
    fn check_impl_item_ref(&mut self, impl_item: &'tcx hir::ImplItemRef, self_ty: Option<ty::Ty<'tcx>>) {
        if let hir::AssocItemKind::Fn { has_self } = impl_item.kind {
            let id = impl_item.id;
            let impl_item = self.tcx.hir().impl_item(id);
            let def_id = impl_item.owner_id.def_id;
            match impl_item.kind {
                hir::ImplItemKind::Fn(sig, _) => {
                    self.check_assoc_fn(has_self, self_ty, &sig, def_id);
                },
                _ => (), // Actually impossible, but we handle it gracefully.
            }
        }
    }
    #[instrument(level = "debug", skip(self, sig), fields(is_mir_available = ?self.tcx.is_mir_available(def_id)))]
    fn check_assoc_fn(
        &mut self,
        has_self: bool,
        self_ty: Option<ty::Ty<'tcx>>,
        sig: &hir::FnSig<'tcx>,
        def_id: LocalDefId,
    ) {
        if self.tcx.is_mir_available(def_id) {
            let decl = sig.decl;
            let body = self.tcx.optimized_mir(def_id);
            let mir_cfg = rpl_match::graph::mir_control_flow_graph(body);
            let mir_ddg = rpl_match::graph::mir_data_dep_graph(body, &mir_cfg);
            let header = Some(sig.header);
            let source_map = self.tcx.sess.source_map();
            // Clone so the borrow of `self.raw_ops` does not conflict with
            // the `&self` receiver used inside the closure body.
            let raw_ops: Vec<(String, Vec<RawOpInstance>)> = self.raw_ops.clone();
            self.pcx.for_each_rpl_pattern(|_id, pattern| {
                let (ops, _ops_diags) = resolve_ops_config(pattern, &raw_ops);
                // _ops_diags: resolution diagnostics will be surfaced in Task 14.
                for (&name, pat_item) in &pattern.patt_block {
                    for matched in self.impl_matched_pat_item(
                        name, &ops, pat_item, &ResolvedOpBindings::empty(), def_id, header, has_self, self_ty, body,
                        &mir_cfg, &mir_ddg,
                    ) {
                        let error = pattern
                            .get_diag(name, source_map, None, body, decl, &matched)
                            .unwrap_or_else(identity);
                        self.tcx.emit_node_span_lint(
                            error.lint(),
                            self.tcx.local_def_id_to_hir_id(def_id),
                            error.primary_span().clone(),
                            error,
                        );
                    }
                }
            });
        }
    }
    #[instrument(level = "debug", skip(self, decl, header))]
    fn check_fn(
        &mut self,
        fn_name: Option<Ident>,
        decl: &hir::FnDecl<'tcx>,
        header: Option<FnHeader>,
        has_self: bool,
        self_ty: Option<ty::Ty<'tcx>>,
        def_id: LocalDefId,
    ) {
        trace!(is_mir_available = ?self.tcx.is_mir_available(def_id), "check_fn");
        if self.tcx.is_mir_available(def_id) {
            let body = self.tcx.optimized_mir(def_id);
            let mir_cfg = rpl_match::graph::mir_control_flow_graph(body);
            let mir_ddg = rpl_match::graph::mir_data_dep_graph(body, &mir_cfg);
            let fn_name = fn_name.map(|ident| ident.name);
            let source_map = self.tcx.sess.source_map();
            // Clone so the borrow of `self.raw_ops` does not conflict with
            // the `&self` receiver used inside the closure body.
            let raw_ops: Vec<(String, Vec<RawOpInstance>)> = self.raw_ops.clone();
            self.pcx.for_each_rpl_pattern(|_id, pattern| {
                let (ops, _ops_diags) = resolve_ops_config(pattern, &raw_ops);
                // _ops_diags: resolution diagnostics will be surfaced in Task 14.
                for (&name, pat_item) in &pattern.patt_block {
                    for matched in self.fn_matched_pat_item(
                        name, &ops, pat_item, &ResolvedOpBindings::empty(), def_id, header, has_self, self_ty,
                        body, &mir_cfg, &mir_ddg,
                    ) {
                        let error = pattern
                            .get_diag(name, source_map, fn_name, body, decl, &matched)
                            .unwrap_or_else(identity);
                        self.tcx.emit_node_span_lint(
                            error.lint(),
                            self.tcx.local_def_id_to_hir_id(def_id),
                            error.primary_span().clone(),
                            error,
                        );
                    }
                }
            });
        }
    }
    // #[instrument(level = "debug", skip(self))]
    // fn check_struct(
    //     &mut self,
    //     def_id: LocalDefId,
    //     variant: hir::VariantData<'tcx>,
    //     generics: &'tcx hir::Generics<'tcx>,
    // ) {
    //     let adt_def = self.tcx.adt_def(def_id);
    //     self.pcx.for_each_rpl_pattern(|_id, pattern| {
    //         for (&name, pat_item) in &pattern.patt_block {
    //             match pat_item {
    //                 rpl_context::pat::PatternItem::RustItems(rpl_rust_items) => {
    //                     for (_, adt_pat) in &rpl_rust_items.adts {
    //                         for matched in
    //                             MatchAdtCtxt::new(self.tcx, self.pcx, rpl_rust_items,
    // adt_pat).match_adt(adt_def)                         {
    //                             let error = pattern
    //                                 .get_diag(name, &fn_pat.expect_mir_body().labels, body, &matched)
    //                                 .unwrap_or_else(identity);
    //                             self.tcx.emit_node_span_lint(
    //                                 error.lint(),
    //                                 self.tcx.local_def_id_to_hir_id(def_id),
    //                                 error.primary_span(),
    //                                 error,
    //                             );
    //                         }
    //                     }
    //                 },
    //                 _ => unreachable!(),
    //             }
    //         }
    //     });
    // }
    // #[instrument(level = "debug", skip(self))]
    // fn check_enum(&mut self, def_id: LocalDefId, variants: hir::EnumDef<'tcx>, generics: &'tcx
    // hir::Generics<'tcx>) {}
}
