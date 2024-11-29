use rustc_data_structures::packed::Pu128;
use rustc_hir::LangItem;
use rustc_middle::mir;
use rustc_middle::ty::{self, TyCtxt};
use rustc_span::Symbol;

rustc_index::newtype_index! {
    #[debug_format = "?T{}"]
    pub struct TyVarIdx {}
}

rustc_index::newtype_index! {
    #[debug_format = "?C{}"]
    pub struct ConstVarIdx {}
}

// FIXME: Use interning for the types
#[derive(Clone, Copy)]
#[rustc_pass_by_value]
pub struct Ty<'tcx>(pub(crate) &'tcx TyKind<'tcx>);

impl<'tcx> Ty<'tcx> {
    pub fn kind(self) -> &'tcx TyKind<'tcx> {
        self.0
    }
}

#[derive(Clone, Copy)]
pub enum RegionKind {
    ReAny,
    ReStatic,
}

#[derive(Clone, Copy)]
pub enum TyKind<'tcx> {
    TyVar(TyVar<'tcx>),
    Array(Ty<'tcx>, Const<'tcx>),
    Slice(Ty<'tcx>),
    Tuple(&'tcx [Ty<'tcx>]),
    Ref(RegionKind, Ty<'tcx>, mir::Mutability),
    RawPtr(Ty<'tcx>, mir::Mutability),
    Path(PathWithArgs<'tcx>),
    Uint(ty::UintTy),
    Int(ty::IntTy),
    Float(ty::FloatTy),
    Bool,
    Str,
}

#[derive(Clone, Copy)]
pub enum GenericArgKind<'tcx> {
    Lifetime(RegionKind),
    Type(Ty<'tcx>),
    Const(Const<'tcx>),
}

impl From<RegionKind> for GenericArgKind<'_> {
    fn from(region: RegionKind) -> Self {
        GenericArgKind::Lifetime(region)
    }
}

impl<'tcx> From<Ty<'tcx>> for GenericArgKind<'tcx> {
    fn from(ty: Ty<'tcx>) -> Self {
        GenericArgKind::Type(ty)
    }
}

impl<'tcx> From<Const<'tcx>> for GenericArgKind<'tcx> {
    fn from(konst: Const<'tcx>) -> Self {
        GenericArgKind::Const(konst)
    }
}

#[derive(Clone, Copy)]
pub struct ItemPath<'tcx>(pub &'tcx [Symbol]);

#[derive(Clone, Copy)]
pub enum Path<'tcx> {
    Item(ItemPath<'tcx>),
    TypeRelative(Ty<'tcx>, Symbol),
    LangItem(LangItem),
}

impl<'tcx> From<ItemPath<'tcx>> for Path<'tcx> {
    fn from(item: ItemPath<'tcx>) -> Self {
        Path::Item(item)
    }
}

impl<'tcx> From<(Ty<'tcx>, Symbol)> for Path<'tcx> {
    fn from((ty, path): (Ty<'tcx>, Symbol)) -> Self {
        Path::TypeRelative(ty, path)
    }
}

impl<'tcx> From<(Ty<'tcx>, &str)> for Path<'tcx> {
    fn from((ty, path): (Ty<'tcx>, &str)) -> Self {
        (ty, Symbol::intern(path)).into()
    }
}

#[derive(Clone, Copy)]
pub struct GenericArgsRef<'tcx>(pub &'tcx [GenericArgKind<'tcx>]);

impl<'tcx> std::ops::Deref for GenericArgsRef<'tcx> {
    type Target = [GenericArgKind<'tcx>];

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl From<LangItem> for Path<'_> {
    fn from(lang_item: LangItem) -> Self {
        Path::LangItem(lang_item)
    }
}

#[derive(Clone, Copy)]
pub struct PathWithArgs<'tcx> {
    pub path: Path<'tcx>,
    pub args: GenericArgsRef<'tcx>,
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum IntTy {
    NegInt(ty::IntTy),
    Int(ty::IntTy),
    Uint(ty::UintTy),
    Bool,
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct IntValue {
    pub value: Pu128,
    pub ty: IntTy,
}

impl IntValue {
    pub fn normalize(self, pointer_bytes: u64) -> Pu128 {
        use ty::IntTy::{Isize, I128, I16, I32, I64, I8};
        use IntTy::{Bool, Int, NegInt, Uint};

        let IntValue { ty, value } = self;
        let mask: u128 = match ty {
            NegInt(I8) => u8::MAX.into(),
            NegInt(I16) => u16::MAX.into(),
            NegInt(I32) => u32::MAX.into(),
            NegInt(I64) => u64::MAX.into(),
            NegInt(I128) => u128::MAX,
            NegInt(Isize) => match pointer_bytes {
                2 => u128::from(u16::MAX),
                4 => u128::from(u32::MAX),
                8 => u128::from(u64::MAX),
                _ => panic!("unsupported pointer size: {pointer_bytes}"),
            },
            Int(_) | Uint(_) | Bool => return value,
        };
        Pu128((value.get() ^ mask).wrapping_add(1) & mask)
    }
}

macro_rules! impl_uint {
    ($($ty:ident => $variant:ident),* $(,)?) => {$(
        impl From<$ty> for IntValue {
            fn from(value: $ty) -> Self {
                Self {
                    value: Pu128(value as u128),
                    ty: IntTy::Uint(ty::UintTy::$variant),
                }
            }
        }
    )* };
}

macro_rules! impl_int {
    ($($ty:ident => $variant:ident),* $(,)?) => {$(
        impl From<$ty> for IntValue {
            fn from(value: $ty) -> Self {
                let ty = if value < 0 { IntTy::NegInt } else { IntTy::Int };
                Self {
                    value: Pu128(value.unsigned_abs() as u128),
                    ty: ty(ty::IntTy::$variant),
                }
            }
        }
    )* };
}

impl_uint!(u8 => U8, u16 => U16, u32 => U32, u64 => U64, u128 => U128, usize => Usize);
impl_int!(i8 => I8, i16 => I16, i32 => I32, i64 => I64, i128 => I128, isize => Isize);

impl From<bool> for IntValue {
    fn from(value: bool) -> Self {
        Self {
            value: Pu128(value.into()),
            ty: IntTy::Bool,
        }
    }
}

pub type TyPred<'tcx> = fn(TyCtxt<'tcx>, ty::ParamEnv<'tcx>, ty::Ty<'tcx>) -> bool;

#[derive(Debug, Clone, Copy)]
pub enum Const<'tcx> {
    ConstVar(ConstVar<'tcx>),
    Value(IntValue),
}

#[derive(Clone, Copy)]
pub struct TyVar<'tcx> {
    pub idx: TyVarIdx,
    pub pred: Option<TyPred<'tcx>>,
}

#[derive(Clone, Copy)]
pub struct ConstVar<'tcx> {
    pub idx: ConstVarIdx,
    pub ty: Ty<'tcx>,
}
