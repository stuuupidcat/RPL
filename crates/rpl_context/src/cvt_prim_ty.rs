use rustc_ast::{FloatTy as AstFloatTy, IntTy as AstIntTy, UintTy as AstUintTy};
use rustc_middle::ty::{FloatTy, IntTy, UintTy};

pub trait CvtPrimTy<From>: Sized {
    fn cvt(ty: From) -> Self;
}

impl CvtPrimTy<IntTy> for AstIntTy {
    fn cvt(ty: IntTy) -> Self {
        match ty {
            IntTy::Isize => AstIntTy::Isize,
            IntTy::I8 => AstIntTy::I8,
            IntTy::I16 => AstIntTy::I16,
            IntTy::I32 => AstIntTy::I32,
            IntTy::I64 => AstIntTy::I64,
            IntTy::I128 => AstIntTy::I128,
        }
    }
}

impl CvtPrimTy<AstIntTy> for IntTy {
    fn cvt(ty: AstIntTy) -> Self {
        match ty {
            AstIntTy::Isize => IntTy::Isize,
            AstIntTy::I8 => IntTy::I8,
            AstIntTy::I16 => IntTy::I16,
            AstIntTy::I32 => IntTy::I32,
            AstIntTy::I64 => IntTy::I64,
            AstIntTy::I128 => IntTy::I128,
        }
    }
}

impl CvtPrimTy<AstUintTy> for UintTy {
    fn cvt(ty: AstUintTy) -> Self {
        match ty {
            AstUintTy::Usize => UintTy::Usize,
            AstUintTy::U8 => UintTy::U8,
            AstUintTy::U16 => UintTy::U16,
            AstUintTy::U32 => UintTy::U32,
            AstUintTy::U64 => UintTy::U64,
            AstUintTy::U128 => UintTy::U128,
        }
    }
}

impl CvtPrimTy<UintTy> for AstUintTy {
    fn cvt(ty: UintTy) -> Self {
        match ty {
            UintTy::Usize => AstUintTy::Usize,
            UintTy::U8 => AstUintTy::U8,
            UintTy::U16 => AstUintTy::U16,
            UintTy::U32 => AstUintTy::U32,
            UintTy::U64 => AstUintTy::U64,
            UintTy::U128 => AstUintTy::U128,
        }
    }
}

impl CvtPrimTy<AstFloatTy> for FloatTy {
    fn cvt(ty: AstFloatTy) -> Self {
        match ty {
            AstFloatTy::F16 => FloatTy::F16,
            AstFloatTy::F32 => FloatTy::F32,
            AstFloatTy::F64 => FloatTy::F64,
            AstFloatTy::F128 => FloatTy::F128,
        }
    }
}

impl CvtPrimTy<FloatTy> for AstFloatTy {
    fn cvt(ty: FloatTy) -> Self {
        match ty {
            FloatTy::F16 => AstFloatTy::F16,
            FloatTy::F32 => AstFloatTy::F32,
            FloatTy::F64 => AstFloatTy::F64,
            FloatTy::F128 => AstFloatTy::F128,
        }
    }
}
