use crate::*;

use crate::parse::ParseError;

use quote::{quote, ToTokens};
use syn::parse::Parse;

#[track_caller]
fn test_pass<T: Parse + ToTokens>(input: impl ToTokens) {
    let input = input.into_token_stream();
    let input_string = input.to_string();
    let mir_patterns: T = syn::parse2(input).unwrap();
    let output = mir_patterns.into_token_stream();
    assert_eq!(input_string, output.to_string());
}

#[track_caller]
fn test_fail<T: Parse + ToTokens>(input: impl ToTokens, msg: impl ToString) {
    match syn::parse2::<T>(input.into_token_stream()) {
        Ok(t) => panic!("unexpected successful parse: {}", t.into_token_stream()),
        Err(e) => assert_eq!(e.to_string(), msg.to_string()),
    }
}

macro_rules! pass {
    ($test_struct:ident!( $( $tt:tt )* ) $(,)?) => {
        test_pass::<$test_struct>(quote!($($tt)*));
    };
    ($test_struct:ident!{ $( $tt:tt )* } $(,)?) => {
        pass!($test_struct!( $($tt)* ));
    };
    ($test_struct:ident![ $( $tt:tt )* ] $(,)?) => {
        pass!($test_struct!( $($tt)* ));
    };
}

macro_rules! fail {
    ($test_struct:ident!( $( $tt:tt )* ), $msg:expr $(,)?) => {
        test_fail::<$test_struct>(quote!($($tt)*), $msg);
    };
    ($test_struct:ident!{ $( $tt:tt )* }, $msg:expr $(,)?) => {
        fail!($test_struct!( $($tt)* ), $msg);
    };
    ($test_struct:ident![ $( $tt:tt )* ], $msg:expr $(,)?) => {
        fail!($test_struct!( $($tt)* ), $msg);
    };
}

#[test]
#[rustfmt::skip]
fn test_type_decl() {
    pass!(TypeDecl!( type SliceT = [T]; ));
    fail!(
        TypeDecl!( type SliceT<T> = [T]; ),
        ParseError::TypeWithGenericsNotSupported
    );
}

#[test]
fn test_path() {
    pass!(PathSegment!(std));
    pass!(Path!(std::mem::take));
    pass!(Path!(Vec<T>));
    pass!(Path!(core::ffi::c_str::CStr));
    pass!(TypePath!(<Vec<T> >));
    pass!(TypePath!(<Vec<T> as Clone>::clone));
    pass!(TypePath!(<core::ffi::c_str::CStr>));
    pass!(TypePath!(<core::ffi::c_str::CStr>::from_bytes_with_nul_unchecked));
    #[rustfmt::skip]
    pass!(TypePath!(< <core::ffi::c_str::CStr>::from_bytes_with_nul_unchecked>::___rt_impl));

    fail!(Path!(crate::crate), ParseError::UnexpectedCrateInPath);
    fail!(Path!(std::crate), ParseError::UnexpectedCrateInPath);
    fail!(
        Path!(crate),
        format!("unexpected end of input, {}", ParseError::CrateAloneInPath)
    );
    fail!(Path!(crate::), "unexpected end of input, expected identifier");
    fail!(Path!(from_ptr as), "unexpected token");
    fail!(TypePath!(from_ptr as), "unexpected token");
}

#[test]
fn test_type() {
    pass!(Type!(*const u8));
    pass!(Type!([T]));
    #[rustfmt::skip]
    pass!(Type!(< <core::ffi::c_str::CStr>::from_bytes_with_nul_unchecked>::___rt_impl));

    fail!(Type!(*const u8(PtrToPtr)), "unexpected token");
}

#[test]
fn test_place() {
    pass!(Place!(x));
    pass!(Place!(x.0));
    pass!(Place!((*x.0)));
    pass!(Place!((*x.0)[2]));
    pass!(Place!((*x.0)[y]));
    pass!(Place!((*x.0)[-3]));
    pass!(Place!((*x.0)[1..3]));
    pass!(Place!((*x.0)[1..-3]));

    fail!(Place!(from_ptr as), "unexpected token");
}

#[test]
fn test_operand() {
    pass!(Operand!(std::mem::take));
    pass!(Operand!(move y));
    fail!(Operand!(from_ptr as), "unexpected token");
}

#[test]
fn test_rvalue() {
    pass!(CastKind!(PtrToPtr));
    pass!(RvalueCast!(from_ptr as *const u8(PtrToPtr)));

    pass!(RvalueOrCall!(&x));
    pass!(RvalueOrCall!(&mut y));
    pass!(RvalueOrCall!(&raw const *x));
    pass!(RvalueOrCall!(&raw mut *y));
    pass!(RvalueOrCall!([i32; _] from [0, 1, 2, 3, 4]));
    pass!(RvalueOrCall!((0, 1, 2, 3, 4)));
    pass!(RvalueOrCall!(Test { x: 0 }));
    pass!(RvalueOrCall!(*const [i32] from (ptr, meta)));

    fail!(
        RvalueCast!(from_ptr as *const u8),
        "unexpected end of input, expected parentheses"
    );
}

#[test]
fn test_call() {
    pass!(Call!( std::mem::take(move y) ));
    pass!(RvalueOrCall!( std::mem::take(move y) ));
    #[rustfmt::skip]
    pass!(Call!( < <core::ffi::c_str::CStr>::from_bytes_with_nul_unchecked>::___rt_impl(move uslice) ));
    #[rustfmt::skip]
    pass!(RvalueOrCall!( < <core::ffi::c_str::CStr>::from_bytes_with_nul_unchecked>::___rt_impl(move uslice) ));

    pass!(Call!( crate::ffi::sqlite3session_attach(move s, move iptr) ));
    pass!(RvalueOrCall!( crate::ffi::sqlite3session_attach(move s, move iptr) ));
}

#[test]
fn test_assign() {
    pass!(Assign!( *x = std::mem::take(move y); ));
}

#[test]
fn test_statement() {
    pass!(Statement!( type T = ...; ));
    #[rustfmt::skip]
    pass!(Statement!( type SliceT = [T]; ));
    pass!(Statement!( let x: u32 = 0; ));
    pass!(Statement!( *x = y.0; ));
    pass!(Statement!( *x = std::mem::take(move y); ));
    pass!(Statement!( drop(y[x]); ));
    pass!(Statement!( let to_ptr: *const u8 = from_ptr as *const u8 (PtrToPtr); ));
}

#[test]
fn test_mir_pattern() {
    pass!(MirPattern!());
    pass!(MirPattern! {
        type T = ...;
        type SliceT = [T];
        type RefSliceT = &SliceT;
        type PtrSliceT = *const SliceT;
        type PtrU8 = *const u8;
        type SliceU8 = [u8];
        type PtrSliceU8 = *const SliceU8;
        type RefSliceU8 = &SliceU8;

        let from_slice: SliceT = ...;
        let from_raw_slice: PtrSliceT = &raw const *from_slice;
        let from_len: usize = Len(from_slice);
        let ty_size: usize = SizeOf(T);
        let to_ptr: PtrU8 = from_ptr as PtrU8 (PtrToPtr);
        let to_len: usize = Mul(from_len, ty_size);
        let to_raw_slice: PtrSliceU8 = *const SliceU8 from (to_ptr, t_len);
        let to_slice: RefSliceU8 = &*to_raw_slice;
    });
    pass!(MirPattern! {
        use core::ffi::c_str::CString;
        use core::ffi::c_str::Cstr;
        use core::ptr::non_null::NonNull;
        use crate::ffi::sqlite3session_attach;

        type NonNullSliceU8 = NonNull<[u8]>;
        type PtrSliceU8 = *const [u8];
        type RefSliceU8 = &[u8];
        type PtrCStr = *const CStr;
        type RefCStr = &CStr;
        type PtrSliceI8 = *const [i8];
        type PtrI8 = *const i8;

        let cstring: CString = ...;
        let non_null: NonNullSliceU8 = (((cstring.inner).0).pointer);
        let uslice_ptr: PtrSliceU8 = (non_null.pointer);
        let cstr: PtrCStr = uslice_ptr as PtrCStr (PtrToPtr);
        // /*
        let uslice: RefSliceU8 = &(*uslice_ptr);
        let cstr: RefCStr = < <CStr>::from_bytes_with_nul_unchecked>::___rt_impl(move uslice);
        // */
        let islice: PtrSliceI8 = &raw const ((*cstr).inner);
        let iptr: PtrI8 = move islice as PtrI8 (PtrToPtr);
        drop(cstring);
        let s: i32 = ...;
        let ret: i32 = sqlite3session_attach(move s, move iptr);
    });
}
