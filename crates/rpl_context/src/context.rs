use std::ops::Deref;

use rustc_arena::DroplessArena;
use rustc_hir as hir;
use rustc_middle::ty::TyCtxt;
use rustc_middle::{mir, ty};
use rustc_span::Symbol;

use crate::pat::{self, Ty, TyKind};

pub struct PrimitiveTypes<'tcx> {
    pub u8: Ty<'tcx>,
    pub u16: Ty<'tcx>,
    pub u32: Ty<'tcx>,
    pub u64: Ty<'tcx>,
    pub u128: Ty<'tcx>,
    pub usize: Ty<'tcx>,
    pub i8: Ty<'tcx>,
    pub i16: Ty<'tcx>,
    pub i32: Ty<'tcx>,
    pub i64: Ty<'tcx>,
    pub i128: Ty<'tcx>,
    pub isize: Ty<'tcx>,
    pub bool: Ty<'tcx>,
    pub str: Ty<'tcx>,
}

impl<'tcx> PrimitiveTypes<'tcx> {
    fn new(arena: &'tcx DroplessArena) -> Self {
        Self {
            u8: Ty(arena.alloc(TyKind::Uint(ty::UintTy::U8))),
            u16: Ty(arena.alloc(TyKind::Uint(ty::UintTy::U16))),
            u32: Ty(arena.alloc(TyKind::Uint(ty::UintTy::U32))),
            u64: Ty(arena.alloc(TyKind::Uint(ty::UintTy::U64))),
            u128: Ty(arena.alloc(TyKind::Uint(ty::UintTy::U128))),
            usize: Ty(arena.alloc(TyKind::Uint(ty::UintTy::Usize))),
            i8: Ty(arena.alloc(TyKind::Int(ty::IntTy::I8))),
            i16: Ty(arena.alloc(TyKind::Int(ty::IntTy::I16))),
            i32: Ty(arena.alloc(TyKind::Int(ty::IntTy::I32))),
            i64: Ty(arena.alloc(TyKind::Int(ty::IntTy::I64))),
            i128: Ty(arena.alloc(TyKind::Int(ty::IntTy::I128))),
            isize: Ty(arena.alloc(TyKind::Int(ty::IntTy::Isize))),
            bool: Ty(arena.alloc(TyKind::Bool)),
            str: Ty(arena.alloc(TyKind::Str)),
        }
    }
}

#[derive(Clone, Copy)]
pub struct PatCtxt<'pcx, 'tcx> {
    pcx: &'pcx PatternCtxt<'tcx>,
}

impl<'tcx> Deref for PatCtxt<'_, 'tcx> {
    type Target = PatternCtxt<'tcx>;

    fn deref(&self) -> &Self::Target {
        self.pcx
    }
}

pub struct PatternCtxt<'tcx> {
    arena: &'tcx DroplessArena,
    pub primitive_types: PrimitiveTypes<'tcx>,
}

impl<'tcx> PatternCtxt<'tcx> {
    pub fn entered<T>(tcx: TyCtxt<'tcx>, f: impl FnOnce(PatCtxt<'_, 'tcx>) -> T) -> T {
        let arena = &tcx.arena.dropless;
        let pcx = &PatternCtxt {
            arena,
            primitive_types: PrimitiveTypes::new(arena),
        };
        f(PatCtxt { pcx })
    }
    pub fn entered_no_tcx<T>(f: impl FnOnce(PatCtxt<'_, '_>) -> T) -> T {
        let arena = &DroplessArena::default();
        let pcx = &PatternCtxt {
            arena,
            primitive_types: PrimitiveTypes::new(arena),
        };
        rustc_span::create_session_if_not_set_then(rustc_span::edition::LATEST_STABLE_EDITION, |_| f(PatCtxt { pcx }))
    }
    pub fn mk_symbols(&self, syms: &[&str]) -> &'tcx [Symbol] {
        self.arena.alloc_from_iter(syms.iter().copied().map(Symbol::intern))
    }
    pub fn mk_slice<T: Copy>(&self, slice: &[T]) -> &'tcx [T] {
        if slice.is_empty() {
            return &[];
        }
        self.arena.alloc_slice(slice)
    }
    fn mk_generic_args(&self, generics: &[pat::GenericArgKind<'tcx>]) -> pat::GenericArgsRef<'tcx> {
        pat::GenericArgsRef(self.mk_slice(generics))
    }
    pub fn mk_type_relative(&self, ty: Ty<'tcx>, path: &str) -> pat::Path<'tcx> {
        pat::Path::TypeRelative(ty, Symbol::intern(path))
    }
    pub fn mk_lang_item(&self, item: &str) -> pat::Path<'tcx> {
        hir::LangItem::from_name(Symbol::intern(item))
            .unwrap_or_else(|| panic!("unknown language item \"{item}\""))
            .into()
    }
    pub fn mk_item_path(&self, path: &[&str]) -> pat::ItemPath<'tcx> {
        pat::ItemPath(self.mk_symbols(path))
    }
    pub fn mk_path_with_args(
        &self,
        path: impl Into<pat::Path<'tcx>>,
        generics: &[pat::GenericArgKind<'tcx>],
    ) -> pat::PathWithArgs<'tcx> {
        let path = path.into();
        let args = self.mk_generic_args(generics);
        pat::PathWithArgs { path, args }
    }
    pub fn mk_path_ty(&self, path_with_args: pat::PathWithArgs<'tcx>) -> Ty<'tcx> {
        self.mk_ty(TyKind::Path(path_with_args))
    }
    pub fn mk_adt_ty(&self, path_with_args: pat::PathWithArgs<'tcx>) -> Ty<'tcx> {
        self.mk_path_ty(path_with_args)
    }
    pub fn mk_array_ty(&self, ty: Ty<'tcx>, len: pat::Const<'tcx>) -> Ty<'tcx> {
        self.mk_ty(TyKind::Array(ty, len))
    }
    pub fn mk_slice_ty(&self, ty: Ty<'tcx>) -> Ty<'tcx> {
        self.mk_ty(TyKind::Slice(ty))
    }
    pub fn mk_tuple_ty(&self, ty: &[Ty<'tcx>]) -> Ty<'tcx> {
        self.mk_ty(TyKind::Tuple(self.mk_slice(ty)))
    }
    pub fn mk_ref_ty(&self, region: pat::RegionKind, ty: Ty<'tcx>, mutability: mir::Mutability) -> Ty<'tcx> {
        self.mk_ty(TyKind::Ref(region, ty, mutability))
    }
    pub fn mk_raw_ptr_ty(&self, ty: Ty<'tcx>, mutability: mir::Mutability) -> Ty<'tcx> {
        self.mk_ty(TyKind::RawPtr(ty, mutability))
    }
    pub fn mk_fn(&self, path_with_args: pat::PathWithArgs<'tcx>) -> Ty<'tcx> {
        self.mk_path_ty(path_with_args)
    }
    pub fn mk_var_ty(&self, ty_var: pat::TyVar<'tcx>) -> Ty<'tcx> {
        self.mk_ty(TyKind::TyVar(ty_var))
    }
    fn mk_ty(&self, kind: TyKind<'tcx>) -> Ty<'tcx> {
        Ty(self.arena.alloc(kind))
    }
}
