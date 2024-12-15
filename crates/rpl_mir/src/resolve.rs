//! Resolve an item path.
//!
//! See https://doc.rust-lang.org/nightly/nightly-rustc/src/clippy_utils/lib.rs.html#691
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::{CrateNum, LocalDefId, LOCAL_CRATE};
use rustc_hir::{ImplItemRef, ItemKind, Node, OwnerId, PrimTy, TraitItemRef};
use rustc_middle::ty::fast_reject::SimplifiedType;
use rustc_middle::ty::{FloatTy, IntTy, Mutability, TyCtxt, UintTy};
use rustc_span::def_id::DefId;
use rustc_span::symbol::Ident;
use rustc_span::Symbol;

/// Resolves a def path like `std::vec::Vec`.
///
/// Can return multiple resolutions when there are multiple versions of the same crate, e.g.
/// `memchr::memchr` could return the functions from both memchr 1.0 and memchr 2.0.
///
/// Also returns multiple results when there are multiple paths under the same name e.g. `std::vec`
/// would have both a [`DefKind::Mod`] and [`DefKind::Macro`].
///
/// This function is expensive and should be used sparingly.
#[instrument(level = "debug", skip(tcx), ret)]
pub fn def_path_res(tcx: TyCtxt<'_>, path: &[Symbol]) -> Vec<Res> {
    let (base, path) = match path {
        [primitive] => {
            return vec![PrimTy::from_name(*primitive).map_or(Res::Err, Res::PrimTy)];
        },
        [base, path @ ..] => (base, path),
        [] => return Vec::new(),
    };

    // let base_sym = Symbol::intern(base);

    let local_crate = if tcx.crate_name(LOCAL_CRATE) == *base || "crate" == base.as_str() {
        Some(LOCAL_CRATE.as_def_id())
    } else {
        None
    };

    let crates = find_primitive_impls(tcx, *base)
        .chain(local_crate)
        .map(|id| Res::Def(tcx.def_kind(id), id))
        .chain(find_crates(tcx, *base))
        .collect();

    trace!(?crates);

    def_path_res_with_base(tcx, crates, path)
}

/// Resolves a def path like `vec::Vec` with the base `std`.
///
/// This is lighter than [`def_path_res`], and should be called with [`find_crates`] looking up
/// items from the same crate repeatedly, although should still be used sparingly.
#[instrument(level = "debug", skip(tcx), ret)]
pub fn def_path_res_with_base(tcx: TyCtxt<'_>, mut base: Vec<Res>, mut path: &[Symbol]) -> Vec<Res> {
    while let [segment, rest @ ..] = path {
        path = rest;
        // let segment = Symbol::intern(segment);
        let segment = *segment;

        base = base
            .into_iter()
            .filter_map(|res| res.opt_def_id())
            .flat_map(|def_id| {
                // When the current def_id is e.g. `struct S`, check the impl items in
                // `impl S { ... }`
                let inherent_impl_children = tcx
                    .inherent_impls(def_id)
                    .iter()
                    .flat_map(|&impl_def_id| item_children_by_name(tcx, impl_def_id, segment));

                let direct_children = item_children_by_name(tcx, def_id, segment);

                inherent_impl_children.chain(direct_children)
            })
            .collect();

        trace!(?segment, ?rest, ?base);
    }

    base
}

#[instrument(level = "trace", skip(tcx), ret)]
fn non_local_item_children_by_name(tcx: TyCtxt<'_>, def_id: DefId, name: Symbol) -> Vec<Res> {
    match tcx.def_kind(def_id) {
        DefKind::Mod | DefKind::Enum | DefKind::Trait => tcx
            .module_children(def_id)
            .iter()
            .filter(|item| item.ident.name == name)
            .map(|child| child.res.expect_non_local())
            .collect(),
        DefKind::Impl { .. } => tcx
            .associated_item_def_ids(def_id)
            .iter()
            .copied()
            .filter(|assoc_def_id| tcx.item_name(*assoc_def_id) == name)
            .map(|assoc_def_id| Res::Def(tcx.def_kind(assoc_def_id), assoc_def_id))
            .collect(),
        _ => Vec::new(),
    }
}

#[instrument(level = "trace", skip(tcx), ret)]
fn local_item_children_by_name(tcx: TyCtxt<'_>, local_id: LocalDefId, name: Symbol) -> Vec<Res> {
    let hir = tcx.hir();

    let root_mod;
    let item_kind = match tcx.hir_node_by_def_id(local_id) {
        Node::Crate(r#mod) => {
            root_mod = ItemKind::Mod(r#mod);
            &root_mod
        },
        Node::Item(item) => &item.kind,
        _ => return Vec::new(),
    };

    trace!(?item_kind);

    let res = |ident: Ident, owner_id: OwnerId| {
        trace!(?ident, ?name, ?owner_id);
        if ident.name == name {
            let def_id = owner_id.to_def_id();
            Some(Res::Def(tcx.def_kind(def_id), def_id))
        } else {
            None
        }
    };

    match item_kind {
        ItemKind::Mod(r#mod) => r#mod
            .item_ids
            .iter()
            .filter_map(|&item_id| {
                let item = hir.item(item_id);
                match item.kind {
                    ItemKind::ForeignMod { abi: _, items } => {
                        items.iter().find_map(|item| res(item.ident, item.id.owner_id))
                    },
                    _ => res(item.ident, item_id.owner_id),
                }
            })
            .collect(),
        ItemKind::Impl(r#impl) => r#impl
            .items
            .iter()
            .filter_map(|&ImplItemRef { ident, id, .. }| res(ident, id.owner_id))
            .collect(),
        ItemKind::Trait(.., trait_item_refs) => trait_item_refs
            .iter()
            .filter_map(|&TraitItemRef { ident, id, .. }| res(ident, id.owner_id))
            .collect(),
        _ => Vec::new(),
    }
}

#[instrument(level = "debug", skip(tcx), ret)]
fn item_children_by_name(tcx: TyCtxt<'_>, def_id: DefId, name: Symbol) -> Vec<Res> {
    if let Some(local_id) = def_id.as_local() {
        local_item_children_by_name(tcx, local_id, name)
    } else {
        non_local_item_children_by_name(tcx, def_id, name)
    }
}

/// Finds the crates called `name`, may be multiple due to multiple major versions.
pub fn find_crates(tcx: TyCtxt<'_>, name: Symbol) -> Vec<Res> {
    tcx.crates(())
        .iter()
        .copied()
        .filter(move |&num| tcx.crate_name(num) == name)
        .map(CrateNum::as_def_id)
        .map(|id| Res::Def(tcx.def_kind(id), id))
        .collect()
}

fn find_primitive_impls(tcx: TyCtxt<'_>, name: Symbol) -> impl Iterator<Item = DefId> + '_ {
    let ty = match name.as_str() {
        "bool" => SimplifiedType::Bool,
        "char" => SimplifiedType::Char,
        "str" => SimplifiedType::Str,
        "array" => SimplifiedType::Array,
        "slice" => SimplifiedType::Slice,
        // FIXME: rustdoc documents these two using just `pointer`.
        //
        // Maybe this is something we should do here too.
        "const_ptr" => SimplifiedType::Ptr(Mutability::Not),
        "mut_ptr" => SimplifiedType::Ptr(Mutability::Mut),
        "isize" => SimplifiedType::Int(IntTy::Isize),
        "i8" => SimplifiedType::Int(IntTy::I8),
        "i16" => SimplifiedType::Int(IntTy::I16),
        "i32" => SimplifiedType::Int(IntTy::I32),
        "i64" => SimplifiedType::Int(IntTy::I64),
        "i128" => SimplifiedType::Int(IntTy::I128),
        "usize" => SimplifiedType::Uint(UintTy::Usize),
        "u8" => SimplifiedType::Uint(UintTy::U8),
        "u16" => SimplifiedType::Uint(UintTy::U16),
        "u32" => SimplifiedType::Uint(UintTy::U32),
        "u64" => SimplifiedType::Uint(UintTy::U64),
        "u128" => SimplifiedType::Uint(UintTy::U128),
        "f32" => SimplifiedType::Float(FloatTy::F32),
        "f64" => SimplifiedType::Float(FloatTy::F64),
        _ => {
            return [].iter().copied();
        },
    };

    tcx.incoherent_impls(ty).iter().copied()
}
