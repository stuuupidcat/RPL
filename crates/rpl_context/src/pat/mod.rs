use std::fmt::Debug;
use std::hash::Hash;
use std::ops::Deref;
use std::sync::Arc;

pub use error::DynamicError;
use error::DynamicErrorBuilder;
use rpl_constraints::Constraints;
use rpl_meta::collect_elems_separated_by_comma;
use rpl_meta::meta::PattSymbolTables;
use rpl_meta::symbol_table::WithPath;
use rpl_parser::generics::{Choice2, Choice3, Choice4};
use rpl_parser::pairs;
use rustc_data_structures::fx::{FxHashMap, FxIndexMap};
use rustc_hir::FnDecl;
use rustc_middle::mir::Body;
use rustc_span::Symbol;
use rustc_span::source_map::SourceMap;

use crate::PatCtxt;
use crate::pat::table::ColumnType;
use crate::pat::utils::Ident;

mod attr;
mod error;
mod item;
mod matched;
mod mir;
mod non_local_meta_vars;
mod pretty;
mod table;
mod ty;
mod utils;

pub use attr::PatAttr;
pub use item::*;
pub use matched::{Matched, MatchedMap};
pub use mir::*;
pub use non_local_meta_vars::*;
pub(crate) use table::TableHead;
pub use ty::*;

pub type Label = Symbol;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Spanned {
    Location(mir::Location),
    Local(mir::Local),
    Body,
    Output,
}

pub type LabelMap = FxHashMap<Label, Spanned>;

#[derive(Debug, Clone, Copy)]
pub enum PattOrUtil {
    Patt,
    Util,
}

pub enum PatternItem<'pcx> {
    RustItems(RustItems<'pcx>),
    RPLPatternOperation(PatternOperation<'pcx>),
}

impl<'pcx> PatternItem<'pcx> {
    pub fn meta(&self) -> &NonLocalMetaVars<'_> {
        match self {
            PatternItem::RustItems(items) => &items.meta,
            PatternItem::RPLPatternOperation(op) => &op.meta,
        }
    }
    pub(crate) fn diag_name(&self) -> Option<Symbol> {
        match self {
            PatternItem::RustItems(items) => items.attr.diag,
            PatternItem::RPLPatternOperation(op) => op.attr.diag,
        }
    }
    pub(crate) fn consts(&self) -> &FxHashMap<Symbol, &'pcx str> {
        match self {
            PatternItem::RustItems(items) => &items.attr.consts,
            PatternItem::RPLPatternOperation(op) => &op.attr.consts,
        }
    }
    pub(crate) fn table_head(&self) -> TableHead {
        match self {
            PatternItem::RustItems(items) => items.table_head(),
            PatternItem::RPLPatternOperation(op) => op.table_head(),
        }
    }
    pub fn expect_rust_items(&self) -> &RustItems<'pcx> {
        match self {
            PatternItem::RustItems(items) => items,
            PatternItem::RPLPatternOperation(_) => panic!("Expected RustItems, found PatternOperation"),
        }
    }

    /// Collect label → [`Spanned`] kinds for diagnostic primary validation.
    fn label_map(&self) -> FxHashMap<Symbol, Spanned> {
        match self {
            PatternItem::RustItems(items) => {
                let mut map = FxHashMap::default();
                for fn_pat in &items.fns {
                    if let Some(body) = fn_pat.body {
                        map.extend(body.labels.iter().map(|(&k, &v)| (k, v)));
                    }
                }
                for impl_pat in &items.impls {
                    for fn_pat in impl_pat.fns.values() {
                        if let Some(body) = fn_pat.body {
                            map.extend(body.labels.iter().map(|(&k, &v)| (k, v)));
                        }
                    }
                }
                map
            },
            PatternItem::RPLPatternOperation(op) => {
                let mut map = FxHashMap::default();
                for (_, item, matched_map) in &op.positive {
                    for (label, spanned) in item.label_map() {
                        let mapped = *matched_map.labels.get(&label).unwrap_or(&label);
                        map.insert(mapped, spanned);
                    }
                }
                map
            },
        }
    }
}

pub struct RustItems<'pcx> {
    pub pcx: PatCtxt<'pcx>,
    pub meta: Arc<NonLocalMetaVars<'pcx>>,
    pub adts: FxHashMap<Symbol, Adt<'pcx>>,
    pub fns: FnPatterns<'pcx>,
    pub impls: Vec<Impl<'pcx>>,
    pub item_constraints: Option<Constraints>,
    pub attr: PatAttr<'pcx>,
}

impl<'pcx> RustItems<'pcx> {
    pub(crate) fn new(
        pcx: PatCtxt<'pcx>,
        meta: Arc<NonLocalMetaVars<'pcx>>,
        item_constraints: Option<Constraints>,
        attr: PatAttr<'pcx>,
    ) -> Self {
        Self {
            pcx,
            meta,
            adts: Default::default(),
            fns: Default::default(),
            impls: Default::default(),
            item_constraints,
            attr,
        }
    }

    fn add_item(
        &mut self,
        pat_name: Option<Symbol>,
        item: WithPath<'pcx, &'pcx pairs::RustItemWithConstraint<'pcx>>,
        meta: Arc<NonLocalMetaVars<'pcx>>,
        symbol_table: &'pcx rpl_meta::symbol_table::SymbolTable<'pcx>,
    ) {
        let path = item.path;
        let (attr, item, where_block) = item.get_matched();
        let constraints = Constraints::from_where_block_opt(attr.iter_matched(), where_block, path)
            .unwrap_or_else(|err| panic!("unexpected error in constraints:\n{err}"));
        match item.deref() {
            Choice4::_0(rust_fn) => {
                let fn_name = rust_fn.FnSig().FnName().span.as_str();
                let fn_symbol_table = symbol_table.get_fn(fn_name).unwrap();
                self.add_fn(WithPath::new(path, rust_fn), meta, fn_symbol_table, constraints);
            },
            Choice4::_1(rust_struct) => {
                self.add_struct(pat_name, with_path(path, rust_struct), meta, symbol_table, constraints)
            },
            Choice4::_2(rust_enum) => {
                self.add_enum(pat_name, with_path(path, rust_enum), meta, symbol_table, constraints)
            },
            Choice4::_3(rust_impl) => {
                self.add_impl(pat_name, with_path(path, rust_impl), meta, symbol_table, constraints)
            },
        }
    }

    #[instrument(level = "debug", skip(self, rust_fn, meta, fn_symbol_table))]
    fn add_fn(
        &mut self,
        rust_fn: WithPath<'pcx, &'pcx pairs::Fn<'pcx>>,
        meta: Arc<NonLocalMetaVars<'pcx>>,
        fn_symbol_table: &'pcx FnSymbolTable<'pcx>,
        constraints: Constraints,
    ) {
        let fn_pat = FnPattern::from(rust_fn, self.pcx, fn_symbol_table, meta, constraints);
        let fn_pat = self.pcx.alloc_fn(fn_pat);
        let fn_name = fn_pat.name;
        self.fns.all_fns.push(fn_pat);
        match fn_name.as_str() {
            "_" => {
                // unnamed function, add it to the unnamed_fns
                self.fns.unnamed_fns.push(fn_pat);
            },
            _ => {
                // named function, add it to the named_fns
                self.fns.named_fns.insert(fn_name, fn_pat);
            },
        }
    }

    #[instrument(level = "debug", skip(self, rust_struct, symbol_table))]
    fn add_struct<'mcx>(
        &mut self,
        pat_name: Option<Symbol>,
        rust_struct: WithPath<'mcx, &'mcx pairs::Struct<'mcx>>,
        meta: Arc<NonLocalMetaVars<'pcx>>,
        symbol_table: &rpl_meta::symbol_table::SymbolTable<'mcx>,
        constraints: Constraints,
    ) {
        let mut struct_inner = StructInner::default();
        let (_, _, name, generic_wildcard, _, fields, _) = rust_struct.get_matched();
        if let Some(fields) = fields {
            let (fields, rest): (Vec<_>, RestPat) = if let Some(fields) = fields.NonExhaustiveFields() {
                (fields.Field(), RestPat::Rest)
            } else {
                (
                    collect_elems_separated_by_comma!(
                        fields
                            .FieldsSeparatedByComma()
                            .expect("StructFields must contain one alternative")
                    )
                    .collect(),
                    RestPat::Exact,
                )
            };
            struct_inner.rest = rest;
            for field in fields {
                let (name, _, ty) = field.get_matched();
                let name = Symbol::intern(name.span.as_str());
                let ty = Ty::from(with_path(rust_struct.path, ty), self.pcx, symbol_table);
                let field = Field { ty };
                struct_inner.fields.insert(name, field);
            }
        }

        let struct_pat = Adt::new_struct(
            struct_inner,
            meta,
            ItemGenericsPat {
                rest: if generic_wildcard.is_some() {
                    RestPat::Rest
                } else {
                    RestPat::Exact
                },
            },
            constraints,
        );
        // let struct_pat = self.pcx.alloc_struct(struct_pat);
        self.adts.insert(Symbol::intern(name.span.as_str()), struct_pat);
    }

    #[instrument(level = "debug", skip(self, rust_enum, symbol_table))]
    fn add_enum<'mcx>(
        &mut self,
        pat_name: Option<Symbol>,
        rust_enum: WithPath<'mcx, &'mcx pairs::Enum<'mcx>>,
        meta: Arc<NonLocalMetaVars<'pcx>>,
        symbol_table: &'mcx rpl_meta::symbol_table::SymbolTable<'mcx>,
        constraints: Constraints,
    ) {
        let mut enum_inner = EnumInner::default();
        let name = rust_enum.MetaVariable();

        if let Some(variants) = rust_enum.EnumVariantsSeparatedByComma() {
            let variants = collect_elems_separated_by_comma!(variants);
            for variant in variants {
                let mut enum_variant = Variant::default();
                let identifier = match variant.deref() {
                    Choice3::_0(variant) => {
                        if let Some(fields) = variant.get_matched().2 {
                            let fields = collect_elems_separated_by_comma!(fields);
                            for field in fields {
                                let (name, _, ty) = field.get_matched();
                                let name = Symbol::intern(name.span.as_str());
                                let ty = Ty::from(with_path(rust_enum.path, ty), self.pcx, symbol_table);
                                let field = Field { ty };
                                enum_variant.fields.insert(name, field);
                            }
                        }
                        variant.get_matched().0
                    },
                    Choice3::_1(variant) => {
                        let (name, _, ty, _) = variant.get_matched();
                        let name = Symbol::intern(name.span.as_str());
                        let ty = Ty::from(with_path(rust_enum.path, ty), self.pcx, symbol_table);
                        let field = Field { ty };
                        enum_variant.fields.insert(name, field);
                        variant.get_matched().0
                    },
                    Choice3::_2(unit) => unit,
                };
                let ident = Ident::from(identifier);
                enum_inner.insert(ident.name, enum_variant);
            }
        }

        let enum_pat = Adt::new_enum(enum_inner, meta, constraints);
        // let struct_pat = self.pcx.alloc_struct(struct_pat);
        self.adts.insert(Symbol::intern(name.span.as_str()), enum_pat);
    }

    #[instrument(level = "debug", skip(self, rust_impl, meta, symbol_table))]
    fn add_impl<'mcx: 'pcx>(
        &mut self,
        _pat_name: Option<Symbol>,
        rust_impl: WithPath<'pcx, &'pcx pairs::Impl<'pcx>>,
        meta: Arc<NonLocalMetaVars<'pcx>>,
        symbol_table: &'mcx rpl_meta::symbol_table::SymbolTable<'mcx>,
        constraints: Constraints,
    ) {
        let p = rust_impl.path;
        let (binding, unsafety, _, generics, impl_kind, self_ty, where_clause, _, fns, _) = rust_impl.get_matched();
        let impl_sym_tab = symbol_table.get_impl(self_ty, impl_kind.as_ref()).unwrap();
        let self_ty = if let Some(adt_ty) = self_ty.ItemAdtType() {
            ImplSelfTy {
                ty: self
                    .pcx
                    .mk_adt_pat_ty(Symbol::intern(adt_ty.MetaVariable().span.as_str())),
                generic_args: RestPat::Rest,
            }
        } else {
            ImplSelfTy {
                ty: Ty::from(
                    WithPath::new(p, self_ty.Type().expect("ImplSelfType must contain one alternative")),
                    self.pcx,
                    symbol_table,
                ),
                generic_args: RestPat::Exact,
            }
        };
        let trait_path = impl_kind.as_ref().map(|impl_kind| {
            Path::from_imported_pairs(WithPath::new(p, impl_kind.get_matched().0), self.pcx, symbol_table)
        });
        let fns = fns
            .iter_matched()
            .map(|rust_fn| {
                let (rust_fn, where_block) = rust_fn.get_matched();
                // FIXME: attributes on associated functions are not supported yet
                let constraints = Constraints::from_where_block_opt(std::iter::empty(), where_block, p)
                    .expect("unexpected error in constraints");
                let fn_name = rust_fn.FnSig().FnName().span.as_str();
                let fn_sym_tab = impl_sym_tab.inner.get_fn(fn_name).unwrap();
                let fn_def = FnPattern::from(
                    WithPath::new(p, rust_fn),
                    self.pcx,
                    fn_sym_tab,
                    Arc::clone(&meta),
                    constraints,
                );
                (Symbol::intern(fn_name), fn_def)
            })
            .collect();
        let impl_pat = Impl {
            binding: binding
                .as_ref()
                .map(|binding| Symbol::intern(binding.MetaVariable().span.as_str())),
            safety: if unsafety.is_some() {
                SafetyPat::Unsafe
            } else {
                SafetyPat::Safe
            },
            polarity: ImplPolarityPat::Positive,
            generics: ItemGenericsPat {
                rest: if generics.is_some() {
                    RestPat::Rest
                } else {
                    RestPat::Exact
                },
            },
            meta,
            self_ty,
            trait_path,
            where_clause: if where_clause.is_some() {
                RestPat::Rest
            } else {
                RestPat::Exact
            },
            fns,
            constraints,
        };
        debug!(
            ty = ?impl_pat.self_ty,
            trait_path = ?impl_pat.trait_path,
            fns = ?impl_pat.fns.keys()
        );
        self.impls.push(impl_pat);
    }

    #[instrument(level = "trace", skip(self), fields(adts = ?self.adts.keys()), ret)]
    pub fn get_adt(&self, adt: Symbol) -> Option<&Adt<'pcx>> {
        self.adts.get(&adt)
    }

    /// Compatibility bridge for the function-rooted matcher.
    ///
    /// Preserve the old effective last-impl-wins behavior for function-rooted
    /// matching while retaining every impl in [`Self::impls`] for item matching.
    pub fn legacy_impl_for_function_matching(&self) -> Option<&Impl<'pcx>> {
        self.legacy_function_matching_enabled()
            .then(|| self.impls.last())
            .flatten()
    }

    /// Item constraints may refer to ADT and impl bindings, which the
    /// function-rooted matcher cannot represent.
    pub fn legacy_function_matching_enabled(&self) -> bool {
        self.item_constraints.is_none()
    }

    fn table_head(&self) -> TableHead {
        let mut columns = FxHashMap::default();

        self.meta.table_head(&mut columns);

        // FIX: should self.attr be included in the table head?

        for pat in &self.fns {
            if pat.name.as_str().starts_with("$") {
                columns.try_insert(pat.name, ColumnType::Ty).unwrap();
            }
            for label in pat.expect_body().labels.keys() {
                columns.try_insert(*label, ColumnType::Label).unwrap();
            }
        }

        columns
    }

    pub fn post_process<M: Eq + Hash + Debug>(&self, iter: impl Iterator<Item = M>) -> impl Iterator<Item = M> {
        self.attr.post_process(iter)
    }
}

/// `positive` is a list of positive pattern items, `negative` is a list of negative pattern items,
/// they are joined together to form a pattern operation.
///
/// `(positive_1 | positive_2 | ... | positive_n) & !(negative_1 | negative_2 | ... | negative_m)`
pub struct PatternOperation<'pcx> {
    pub pcx: PatCtxt<'pcx>,
    pub meta: Arc<NonLocalMetaVars<'pcx>>,
    pub positive: Vec<(Symbol, &'pcx PatternItem<'pcx>, MatchedMap)>,
    pub negative: Vec<(Symbol, &'pcx PatternItem<'pcx>, MatchedMap)>,
    pub attr: PatAttr<'pcx>,
}

impl PatternOperation<'_> {
    fn table_head(&self) -> TableHead {
        let head = self.positive.first().unwrap().1.table_head();
        debug_assert!(
            self.positive.iter().all(|(_, item, _)| item.table_head() == head),
            "All positive pattern items should have the same table head"
        );
        debug_assert!(
            self.negative.iter().all(|(_, item, _)| item.table_head() == head),
            "All negative pattern items should have the same table head as the positive one"
        );
        head
    }

    pub fn post_process<M: Eq + Hash + Debug>(&self, iter: impl Iterator<Item = M>) -> impl Iterator<Item = M> {
        self.attr.post_process(iter)
    }
}

/// Corresponds to a pattern file in RPL, not a pattern item.
pub struct Pattern<'pcx> {
    pub pcx: PatCtxt<'pcx>,
    pub patt_block: FxIndexMap<Symbol, PatternItem<'pcx>>, // indexed by pat_name
    pub util_block: FxIndexMap<Symbol, &'pcx PatternItem<'pcx>>, // indexed by pat_name
    diag_block: FxHashMap<Symbol, DynamicErrorBuilder<'pcx>>,
}

impl<'pcx> Pattern<'pcx> {
    pub(crate) fn new(pcx: PatCtxt<'pcx>) -> Self {
        Self {
            pcx,
            patt_block: Default::default(),
            util_block: Default::default(),
            diag_block: Default::default(),
        }
    }

    #[instrument(level = "debug", skip(self, source_map, body, decl))]
    pub fn get_diag<'tcx>(
        &self,
        pat_name: Symbol,
        source_map: &SourceMap,
        fn_name: Option<Symbol>,
        body: &Body<'tcx>,
        decl: &FnDecl<'tcx>,
        matched: &impl Matched<'tcx>,
    ) -> Result<Box<DynamicError>, Box<DynamicError>> {
        Ok(Box::new(
            self.diag_block
                .get(&pat_name)
                .ok_or_else(|| Box::new(DynamicError::default_diagnostic(pat_name, body.span)))?
                .build(source_map, fn_name, body, decl, matched),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use pest_typed::ParsableTypedNode as _;
    use rpl_meta::arena::Arena;
    use rpl_meta::context::MetaContext;
    use rpl_meta::symbol_table::SymbolTable;
    use rpl_parser::pairs;
    use rustc_span::Symbol;

    use super::{ImplPolarityPat, Path as PatPath, PattOrUtil, RestPat, SafetyPat, WithPath};
    use crate::PatternCtxt;

    fn item_meta_errors(source: &str, import_sources: &[&str]) -> Vec<String> {
        let arena = &*Box::leak(Box::new(Arena::default()));
        let mctx = &*Box::leak(Box::new(MetaContext::new(arena)));
        let source = arena.alloc_str(source);
        let item = &*Box::leak(Box::new(
            pairs::RPLPatternItem::try_parse(source).expect("parse item pattern"),
        ));
        let imports: Vec<_> = import_sources
            .iter()
            .map(|source| {
                &*Box::leak(Box::new(
                    pairs::UsePath::try_parse(arena.alloc_str(source)).expect("parse import"),
                ))
            })
            .collect();
        let path = Path::new("/synthetic/item-pattern.rpl");
        mctx.set_active_path(Some(path));
        let mut errors = Vec::new();

        SymbolTable::collect_symbol_tables(mctx, &imports, std::iter::once(item), &mut errors);
        errors.into_iter().map(|error| error.to_string()).collect()
    }

    fn send_item_pattern(guard: &str) -> String {
        [
            "p[$Wrapper: adt, $Parameter: type, $MappedType: type] = {",
            "    struct $Wrapper<..> { .. }",
            "    $marker: unsafe impl<..> core::marker::Send for $Wrapper<..> where .. {}",
            "} where {",
            guard,
            "}",
        ]
        .join("\n")
    }

    #[test]
    fn lowers_item_pattern_syntax_flags() {
        let arena = &*Box::leak(Box::new(Arena::default()));
        let mctx = &*Box::leak(Box::new(MetaContext::new(arena)));
        let source = arena.alloc_str(
            r#"
send_variance[
    $Wrapper: adt where true(),
    $Parameter: type,
    $MappedType: type,
] = {
    struct $Wrapper<..> { .. }

    $send_impl:
    unsafe impl<..> Send for $Wrapper<..>
    where ..
    {}

    $sync_impl:
    unsafe impl<..> Sync for $Wrapper<..>
    where ..
    {}
} where {
    has_type_parameters($Wrapper)
    && type_parameter_of($Parameter, $Wrapper)
    && type_parameter_maps_to($Parameter, $MappedType, $send_impl)
    && owns_type($Wrapper, $Parameter)
    && !is_send_in($MappedType, $send_impl)
}
"#,
        );
        let item = &*Box::leak(Box::new(
            pairs::RPLPatternItem::try_parse(source).expect("parse item pattern"),
        ));
        let path = Path::new("/synthetic/item-pattern.rpl");
        mctx.set_active_path(Some(path));
        let send_import = &*Box::leak(Box::new(
            pairs::UsePath::try_parse(arena.alloc_str("use core::marker::Send;")).expect("parse Send import"),
        ));
        let sync_import = &*Box::leak(Box::new(
            pairs::UsePath::try_parse(arena.alloc_str("use core::marker::Sync;")).expect("parse Sync import"),
        ));
        let imports = [send_import, sync_import];

        let mut errors = Vec::new();
        let symbol_tables = SymbolTable::collect_symbol_tables(mctx, &imports, std::iter::once(item), &mut errors);
        assert!(errors.is_empty(), "meta errors: {errors:#?}");
        let symbol_tables = &*Box::leak(Box::new(symbol_tables));

        PatternCtxt::entered_no_tcx(|pcx| {
            let pattern = pcx.new_pattern();
            pattern.add_pattern_item(WithPath::new(path, item), symbol_tables, PattOrUtil::Patt);

            let rust_items = pattern
                .patt_block
                .values()
                .next()
                .expect("lowered pattern item")
                .expect_rust_items();
            assert_eq!(rust_items.adts.len(), 1);
            assert_eq!(rust_items.impls.len(), 2, "impl patterns must not overwrite each other");
            assert_eq!(
                rust_items
                    .item_constraints
                    .as_ref()
                    .expect("item constraints")
                    .preds
                    .len(),
                1
            );
            assert_eq!(rust_items.meta.adt_vars.len(), 1);
            assert_eq!(
                rust_items
                    .meta
                    .adt_vars
                    .iter()
                    .next()
                    .expect("ADT variable")
                    .pred
                    .clauses
                    .len(),
                1
            );

            let adt = rust_items.adts.values().next().expect("struct pattern");
            assert_eq!(adt.generics.rest, RestPat::Rest);
            assert_eq!(adt.non_enum_variant().rest, RestPat::Rest);

            let send_impl = &rust_items.impls[0];
            assert_eq!(send_impl.binding.expect("impl binding").as_str(), "$send_impl");
            assert_eq!(send_impl.safety, SafetyPat::Unsafe);
            assert_eq!(send_impl.polarity, ImplPolarityPat::Positive);
            assert_eq!(send_impl.generics.rest, RestPat::Rest);
            assert_eq!(send_impl.self_ty.generic_args, RestPat::Rest);
            assert_eq!(send_impl.where_clause, RestPat::Rest);
            let PatPath::Item(send_path) = send_impl.trait_path.expect("Send trait path") else {
                panic!("Send should lower as an item path")
            };
            assert_eq!(
                send_path.0,
                &[Symbol::intern("core"), Symbol::intern("marker"), Symbol::intern("Send")]
            );

            let sync_impl = &rust_items.impls[1];
            assert_eq!(sync_impl.binding.expect("impl binding").as_str(), "$sync_impl");
            assert_eq!(sync_impl.safety, SafetyPat::Unsafe);
            let PatPath::Item(sync_path) = sync_impl.trait_path.expect("Sync trait path") else {
                panic!("Sync should lower as an item path")
            };
            assert_eq!(
                sync_path.0,
                &[Symbol::intern("core"), Symbol::intern("marker"), Symbol::intern("Sync")]
            );
            assert!(
                !rust_items.legacy_function_matching_enabled(),
                "item-guarded bundles must wait for the item matcher"
            );
            assert!(rust_items.legacy_impl_for_function_matching().is_none());
        });
    }

    #[test]
    fn rejects_unimported_bare_impl_trait_path() {
        let errors = item_meta_errors(
            r#"
p[$Wrapper: adt] = {
    struct $Wrapper<..> { .. }
    $marker: unsafe impl<..> Send for $Wrapper<..> {}
}
"#,
            &[],
        );

        assert!(
            errors
                .iter()
                .any(|error| error.contains("Impl trait path `Send` is unqualified")),
            "expected unimported-trait-path error, got: {errors:#?}"
        );
    }

    #[test]
    fn rejects_invalid_impl_trait_path_imports_and_arguments() {
        let pattern = r#"
p[$Wrapper: adt] = {
    struct $Wrapper<..> { .. }
    $marker: unsafe impl<..> Send for $Wrapper<..> {}
}
"#;
        let errors = item_meta_errors(pattern, &["use Send;"]);
        assert!(
            errors.iter().any(|error| error.contains("Cyclic imports")),
            "expected self-import cycle error, got: {errors:#?}"
        );

        let recursive_pattern = r#"
p[$Wrapper: adt] = {
    struct $Wrapper<..> { .. }
    $marker: unsafe impl<..> A for $Wrapper<..> {}
}
"#;
        let errors = item_meta_errors(recursive_pattern, &["use B::A;", "use A::B;"]);
        assert!(
            errors.iter().any(|error| error.contains("Cyclic imports")),
            "expected recursive-import cycle error, got: {errors:#?}"
        );

        let generic_pattern = r#"
p[$Wrapper: adt] = {
    struct $Wrapper<..> { .. }
    $marker: unsafe impl<..> Send<u8> for $Wrapper<..> {}
}
"#;
        let errors = item_meta_errors(generic_pattern, &["use core::marker::Send;"]);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("Generic arguments in impl trait paths are not supported")),
            "expected generic-argument error, got: {errors:#?}"
        );
        let errors = item_meta_errors(pattern, &["use core::marker::Send<u8>;"]);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("Generic arguments in impl trait paths are not supported")),
            "expected imported generic-argument error, got: {errors:#?}"
        );

        let absolute_bare_pattern = r#"
p[$Wrapper: adt] = {
    struct $Wrapper<..> { .. }
    $marker: unsafe impl<..> ::Send for $Wrapper<..> {}
}
"#;
        let errors = item_meta_errors(absolute_bare_pattern, &[]);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("Impl trait path `::Send` is unqualified")),
            "expected absolute-bare-path error, got: {errors:#?}"
        );
    }

    #[test]
    fn rejects_duplicate_impl_bindings() {
        let arena = &*Box::leak(Box::new(Arena::default()));
        let mctx = &*Box::leak(Box::new(MetaContext::new(arena)));
        let source = arena.alloc_str(
            r#"
p[$Wrapper: adt] = {
    struct $Wrapper<..> { .. }
    $marker: unsafe impl<..> core::marker::Send for $Wrapper<..> {}
    $marker: unsafe impl<..> core::marker::Sync for $Wrapper<..> {}
}
"#,
        );
        let item = &*Box::leak(Box::new(
            pairs::RPLPatternItem::try_parse(source).expect("parse item pattern"),
        ));
        let path = Path::new("/synthetic/duplicate-item-binding.rpl");
        mctx.set_active_path(Some(path));
        let mut errors = Vec::new();

        SymbolTable::collect_symbol_tables(mctx, &[], std::iter::once(item), &mut errors);

        assert!(
            errors
                .iter()
                .any(|error| error.to_string().contains("Symbol `$marker` is already declared")),
            "expected duplicate item-binding error, got: {errors:#?}"
        );
    }

    #[test]
    fn validates_bundle_predicates_during_meta_checking() {
        let arena = &*Box::leak(Box::new(Arena::default()));
        let mctx = &*Box::leak(Box::new(MetaContext::new(arena)));
        let source = arena.alloc_str(
            r#"
p[$Wrapper: adt] = {
    struct $Wrapper<..> { .. }
} where {
    not_a_predicate()
}
"#,
        );
        let item = &*Box::leak(Box::new(
            pairs::RPLPatternItem::try_parse(source).expect("parse item pattern"),
        ));
        let path = Path::new("/synthetic/invalid-bundle-predicate.rpl");
        mctx.set_active_path(Some(path));
        let mut errors = Vec::new();

        SymbolTable::collect_symbol_tables(mctx, &[], std::iter::once(item), &mut errors);

        assert!(
            errors
                .iter()
                .any(|error| error.to_string().contains("Invalid predicate: not_a_predicate")),
            "expected invalid-predicate error, got: {errors:#?}"
        );
    }

    #[test]
    fn rejects_function_predicates_in_item_guards() {
        let arena = &*Box::leak(Box::new(Arena::default()));
        let mctx = &*Box::leak(Box::new(MetaContext::new(arena)));
        let source = arena.alloc_str(
            r#"
p[$Wrapper: adt, $T: type] = {
    struct $Wrapper<..> { .. }
} where {
    is_send($T)
}
"#,
        );
        let item = &*Box::leak(Box::new(
            pairs::RPLPatternItem::try_parse(source).expect("parse item pattern"),
        ));
        let path = Path::new("/synthetic/unsupported-item-predicate.rpl");
        mctx.set_active_path(Some(path));
        let mut errors = Vec::new();

        SymbolTable::collect_symbol_tables(mctx, &[], std::iter::once(item), &mut errors);

        assert!(
            errors.iter().any(|error| error
                .to_string()
                .contains("Predicate `is_send` is not supported in an item guard")),
            "expected unsupported-item-predicate error, got: {errors:#?}"
        );
    }

    #[test]
    fn rejects_arguments_and_attributes_in_item_guards() {
        let arena = &*Box::leak(Box::new(Arena::default()));
        let mctx = &*Box::leak(Box::new(MetaContext::new(arena)));
        let source = arena.alloc_str(
            r#"
p[$Wrapper: adt] = {
    struct $Wrapper<..> { .. }
} where {
    true($Wrapper),
    safety = safe
}
"#,
        );
        let item = &*Box::leak(Box::new(
            pairs::RPLPatternItem::try_parse(source).expect("parse item pattern"),
        ));
        let path = Path::new("/synthetic/invalid-item-guard.rpl");
        mctx.set_active_path(Some(path));
        let mut errors = Vec::new();

        SymbolTable::collect_symbol_tables(mctx, &[], std::iter::once(item), &mut errors);

        assert!(
            errors.iter().any(|error| error
                .to_string()
                .contains("Item guard predicate `true` does not accept arguments")),
            "expected item-predicate-arguments error, got: {errors:#?}"
        );
        assert!(
            errors.iter().any(|error| error
                .to_string()
                .contains("Attributes are not supported in an item guard")),
            "expected unsupported-item-attribute error, got: {errors:#?}"
        );
    }

    #[test]
    fn validates_item_predicate_modes_and_sorts() {
        let errors = item_meta_errors(&send_item_pattern("has_type_parameters()"), &[]);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("expects 1 arguments, but received 0")),
            "expected arity error, got: {errors:#?}"
        );

        let errors = item_meta_errors(&send_item_pattern("owns_type($Parameter, $Parameter)"), &[]);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("Argument `$Parameter`") && error.contains("must be a adt")),
            "expected argument-sort error, got: {errors:#?}"
        );

        let errors = item_meta_errors(
            &send_item_pattern(
                "type_parameter_maps_to($Parameter, $MappedType, $marker)\n\
                 && type_parameter_of($Parameter, $Wrapper)",
            ),
            &[],
        );
        assert!(
            errors
                .iter()
                .any(|error| { error.contains("Input `$Parameter`") && error.contains("is not bound") }),
            "expected binding-order error, got: {errors:#?}"
        );

        let errors = item_meta_errors(&send_item_pattern("!type_parameter_of($Parameter, $Wrapper)"), &[]);
        assert!(
            errors.iter().any(|error| error.contains("cannot be negated")),
            "expected closed-negation error, got: {errors:#?}"
        );

        let errors = item_meta_errors(
            &send_item_pattern("(type_parameter_of($Parameter, $Wrapper) || true())"),
            &[],
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("not supported inside a disjunction")),
            "expected relational-disjunction error, got: {errors:#?}"
        );
    }
}

impl<'pcx> Pattern<'pcx> {
    pub fn add_pattern_item(
        &mut self,
        pat_item: WithPath<'pcx, &'pcx pairs::RPLPatternItem<'pcx>>,
        symbol_tables: &'pcx PattSymbolTables<'_>,
        block_type: PattOrUtil,
    ) {
        let p = pat_item.path;
        let (_, attr, name, meta_decls, _, item_or_patt_op) = pat_item.get_matched();
        let name = name.span.as_str();
        let symbol_table = symbol_tables.get(&name).unwrap();
        let meta = Arc::new(NonLocalMetaVars::from_meta_decls(
            meta_decls.as_ref().map(|meta_decls| with_path(p, meta_decls)),
            self.pcx,
            symbol_table,
        ));
        let name = Symbol::intern(name);
        self.add_item_or_patt_op(
            name,
            attr.iter_matched(),
            with_path(p, item_or_patt_op),
            symbol_table,
            meta,
            block_type,
        );
    }

    #[instrument(level = "debug", skip(self, attr, item_or_patt_op, symbol_table, meta), fields(patt_block = ?self.patt_block.keys(), util_block = ?self.util_block.keys()))]
    fn add_item_or_patt_op(
        &mut self,
        pat_name: Symbol,
        attr: impl Iterator<Item = &'pcx pairs::Attr<'pcx>>,
        item_or_patt_op: WithPath<'pcx, &'pcx pairs::RustItemsOrPatternOperation<'pcx>>,
        symbol_table: &'pcx rpl_meta::symbol_table::SymbolTable<'_>,
        meta: Arc<NonLocalMetaVars<'pcx>>,
        block_type: PattOrUtil,
    ) {
        let p = item_or_patt_op.path;
        match &***item_or_patt_op {
            Choice3::_0(item) => {
                self.add_items(
                    pat_name,
                    attr,
                    with_path(p, std::iter::once(item)),
                    symbol_table,
                    meta,
                    None,
                    block_type,
                );
            },
            Choice3::_1(items) => {
                let (_, rust_items, _, where_block) = items.get_matched();
                let item_constraints = where_block.as_ref().map(|_| {
                    Constraints::from_where_block_opt(std::iter::empty(), where_block, p)
                        .unwrap_or_else(|err| panic!("unexpected error in pattern constraints:\n{err}"))
                });
                self.add_items(
                    pat_name,
                    attr,
                    with_path(p, rust_items.iter_matched()),
                    symbol_table,
                    meta,
                    item_constraints,
                    block_type,
                );
            },
            Choice3::_2(patt_op) => {
                self.add_patt_op(pat_name, attr, with_path(p, patt_op), meta, block_type);
            },
        }
    }

    fn patt_op(
        &self,
        meta: &NonLocalMetaVars<'pcx>,
        pat_cfg: &'pcx pairs::PatternConfiguration<'pcx>,
    ) -> (Symbol, &'pcx PatternItem<'pcx>, MatchedMap) {
        let name = Ident::from(pat_cfg.Identifier()).name;
        let item = *self.util_block.get(&name).unwrap();
        let map = MatchedMap::new(
            meta,
            item.meta(),
            pat_cfg
                .MetaVariableAssignList()
                .and_then(|list| list.MetaVariableAssignsSeparatedByComma()),
        );
        (name, item, map)
    }

    #[instrument(level = "debug", skip(self, attr, patt_op, meta), fields(patt_block = ?self.patt_block.keys(), util_block = ?self.util_block.keys()))]
    fn add_patt_op<'mcx: 'pcx>(
        &mut self,
        pat_name: Symbol,
        attr: impl Iterator<Item = &'mcx pairs::Attr<'mcx>>,
        patt_op: WithPath<'mcx, &'mcx pairs::PatternOperation<'mcx>>,
        meta: Arc<NonLocalMetaVars<'pcx>>,
        block_type: PattOrUtil,
    ) {
        let patt_op = patt_op.PatternExpression();
        let (pos, pos_, neg) = patt_op.get_matched();
        let positive = std::iter::once(pos)
            .chain(pos_.iter_matched().map(|pos_| pos_.get_matched().1))
            .map(|pos| self.patt_op(&meta, pos))
            .collect();
        let negative = neg
            .iter_matched()
            .map(|negative| self.patt_op(&meta, negative.get_matched().1))
            .collect();
        let attr = PatAttr::parse_all(attr);
        let pat_ops = PatternOperation {
            pcx: self.pcx,
            meta,
            positive,
            negative,
            attr,
        };
        match block_type {
            PattOrUtil::Patt => self
                .patt_block
                .entry(pat_name)
                .or_insert(PatternItem::RPLPatternOperation(pat_ops)),
            PattOrUtil::Util => *self
                .util_block
                .entry(pat_name)
                .or_insert_with(|| self.pcx.alloc_pattern_item(PatternItem::RPLPatternOperation(pat_ops))),
        }
        .table_head();
    }

    #[expect(clippy::too_many_arguments)]
    #[instrument(level = "debug", skip(self, attr, items, symbol_table, meta))]
    fn add_items(
        &mut self,
        pat_name: Symbol,
        attr: impl Iterator<Item = &'pcx pairs::Attr<'pcx>>,
        items: WithPath<'pcx, impl Iterator<Item = &'pcx pairs::RustItemWithConstraint<'pcx>>>,
        symbol_table: &'pcx rpl_meta::symbol_table::SymbolTable<'_>,
        meta: Arc<NonLocalMetaVars<'pcx>>,
        item_constraints: Option<Constraints>,
        block_type: PattOrUtil,
    ) {
        let p = items.path;
        match block_type {
            PattOrUtil::Patt => {
                self.patt_block.entry(pat_name).or_insert_with(|| {
                    let attr = PatAttr::parse_all(attr);
                    let mut rpl_rust_items = RustItems::new(self.pcx, meta.clone(), item_constraints, attr);
                    for item in items.inner {
                        rpl_rust_items.add_item(Some(pat_name), with_path(p, item), meta.clone(), symbol_table);
                    }
                    PatternItem::RustItems(rpl_rust_items)
                });
            },
            PattOrUtil::Util => {
                self.util_block.entry(pat_name).or_insert_with(|| {
                    let attr = PatAttr::parse_all(attr);
                    let mut rpl_rust_items = RustItems::new(self.pcx, meta.clone(), item_constraints, attr);
                    for item in items.inner {
                        rpl_rust_items.add_item(Some(pat_name), with_path(p, item), meta.clone(), symbol_table);
                    }
                    self.pcx.alloc_pattern_item(PatternItem::RustItems(rpl_rust_items))
                });
            },
        };
    }

    pub fn add_diag<'mcx: 'pcx>(
        &mut self,
        diag: WithPath<'mcx, &'mcx pairs::diagBlock<'mcx>>,
        diag_symbol_tables: &rpl_meta::meta::DiagSymbolTables<'mcx>,
        symbol_tables: &PattSymbolTables<'mcx>,
    ) {
        let mut items = FxHashMap::default();
        for item in diag.get_matched().2.iter_matched() {
            let (_, ident, _, _, _, _, _) = item.get_matched();
            let name = Symbol::intern(ident.span.as_str());
            let prev = items.insert(name, item);
            debug_assert!(prev.is_none(), "Duplicate diagnostic for {:?}", name); //FIXME: raise an error
        }

        for (name, pat_item) in &self.patt_block {
            let symbol_table = symbol_tables.get(&name.as_str()).unwrap();

            let diag_name = pat_item.diag_name().unwrap_or(*name);
            if let Some(diag_item) = items.get(&diag_name) {
                let labels = symbol_table.labels().map(Symbol::intern);
                let label_map = pat_item.label_map();
                let diag = DynamicErrorBuilder::<'pcx>::from_item(
                    WithPath::new(diag.path, diag_item),
                    &symbol_table.meta_vars,
                    pat_item.consts(),
                    &labels.collect(),
                    &label_map,
                    diag_symbol_tables
                        .get(&diag_name.as_str())
                        .unwrap_or_else(|| panic!("No diagnostic symbol table found for {diag_name}")),
                )
                .unwrap_or_else(|err| panic!("{err}"));
                let prev = self.diag_block.insert(*name, diag);
                debug_assert!(prev.is_none(), "Duplicate diagnostic for {:?}", name); //FIXME: raise an error
            } else {
                warn!("No diagnostic found for pattern item {:?} ({:?})", name, diag_name);
            }
        }
    }

    /// Primary label names for a pattern's diagnostic, in declaration order.
    pub fn primary_labels(&self, pat_name: Symbol) -> Option<&[&str]> {
        self.diag_block.get(&pat_name).map(|diag| diag.primary_labels())
    }
}
