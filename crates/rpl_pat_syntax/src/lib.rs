#![feature(box_patterns)]
#![feature(let_chains)]

use derive_more::{Deref, Display, From, IntoIterator};
use proc_macro2::Span;
use syn::punctuated::Punctuated;
use syn::{token, Ident, Token};
use syn_derive::{Parse, ToTokens};

#[macro_use]
mod to_tokens;
#[macro_use]
mod parse;

#[cfg(test)]
mod tests;

pub use parse::ParseError;

pub(crate) mod kw {
    // Metadata
    syn::custom_keyword!(meta);
    syn::custom_keyword!(ty);
    syn::custom_keyword!(lang);
    syn::custom_keyword!(ctor);

    // Statement
    syn::custom_keyword!(drop);
    syn::custom_keyword!(switchInt);

    // Operand
    syn::custom_keyword!(copy);

    // place
    syn::custom_keyword!(of);

    // Rvalue
    syn::custom_keyword!(Len);
    syn::custom_keyword!(discriminant);
    syn::custom_keyword!(raw);

    // CastKind
    syn::custom_keyword!(PtrToPtr);
    syn::custom_keyword!(IntToInt);
    syn::custom_keyword!(Transmute);

    // BinOp
    syn::custom_keyword!(Add);
    syn::custom_keyword!(Sub);
    syn::custom_keyword!(Mul);
    syn::custom_keyword!(Div);
    syn::custom_keyword!(Rem);
    syn::custom_keyword!(Lt);
    syn::custom_keyword!(Gt);
    syn::custom_keyword!(Le);
    syn::custom_keyword!(Ge);
    syn::custom_keyword!(Eq);
    syn::custom_keyword!(Ne);
    syn::custom_keyword!(Offset);

    // NullOp
    syn::custom_keyword!(SizeOf);
    syn::custom_keyword!(AlignOf);
    syn::custom_keyword!(OffsetOf);

    // UnOp
    syn::custom_keyword!(Neg);
    syn::custom_keyword!(Not);
    syn::custom_keyword!(PtrMetadata);

    // Aggregate
    syn::custom_keyword!(from);
}

#[derive(Clone, Copy)]
pub struct Region {
    span: Span,
    pub kind: RegionKind,
}

#[derive(Clone, Copy, ToTokens, From)]
pub enum RegionKind {
    ReAny(Token!(_)),
    ReStatic(Token![static]),
}

#[derive(Default, Clone, Copy, ToTokens, Parse, From)]
pub enum Mutability {
    #[parse(peek = Token![mut])]
    Mut(Token![mut]),
    #[default]
    Not,
}

#[derive(Clone, Copy, ToTokens, Parse, From)]
pub enum PtrMutability {
    #[parse(peek = Token![const])]
    Const(Token![const]),
    #[parse(peek = Token![mut])]
    Mut(Token![mut]),
}

#[derive(Clone, ToTokens, Parse)]
pub struct TypeDecl {
    tk_type: Token![type],
    pub ident: Ident,
    #[parse(
        |input| if input.peek(Token![<]) {
            Err(input.error(ParseError::TypeWithGenericsNotSupported))
        } else {
            input.parse()
        }
    )]
    tk_eq: Token![=],
    pub ty: Type,
    tk_semi: Token![;],
}

#[derive(Clone, Parse, ToTokens)]
pub struct TypeArray {
    #[syn(bracketed)]
    bracket: token::Bracket,
    #[syn(in = bracket)]
    pub ty: Box<Type>,
    #[syn(in = bracket)]
    tk_semi: Token![;],
    #[syn(in = bracket)]
    pub len: syn::LitInt,
}

/*
pub struct FnPtrArg {
    pub name: Option<(Ident, Token![:])>,
    pub ty: Type,
}
pub struct FnPtrVariadic {
    pub name: Option<(Ident, Token![:])>,
    tk_dots: Token![...],
    tk_comma: Option<Token![,]>,
}

pub struct TypeFnPtr {
    pub lifetimes: Option<syn::BoundLifetimes>,
    pub unsafety: Option<Token![unsafe]>,
    pub abi: Option<syn::Abi>,
    tk_fn: Token![fn],
    paren: token::Paren,
    pub inputs: Punctuated<FnPtrArg, Token![,]>,
    pub variadic: Option<syn::BareVariadic>,
    pub output: syn::ReturnType,
}
*/

#[derive(Clone)]
pub struct TypeGroup {
    tk_group: token::Group,
    pub ty: Box<Type>,
}

#[derive(Clone, Copy, ToTokens, Parse, From)]
pub struct TypeNever {
    tk_bang: Token![!],
}

pub type TypeParen = Parenthesized<Box<Type>>;

#[derive(Clone)]
pub struct GenericConst {
    brace: Option<token::Brace>,
    pub konst: Const,
}

#[derive(Clone, ToTokens, Parse, From)]
pub enum GenericArgument {
    /// A region argument.
    #[parse(peek = syn::Lifetime)]
    Region(Region),
    /// A type argument.
    #[parse(peek_func = |input| input.fork().parse::<Type>().is_ok())]
    Type(Type),
    /// A const argument.
    Const(GenericConst),
}

#[derive(Clone, ToTokens, Parse)]
pub struct AngleBracketedGenericArguments {
    tk_colon2: Option<Token![::]>,
    tk_lt: Token![<],
    #[parse(parse::parse_angle_bracketed)]
    pub args: Punctuated<GenericArgument, Token![,]>,
    tk_gt: Token![>],
}

#[derive(Clone, ToTokens, Parse)]
pub enum ReturnType {
    #[parse(peek = Token![->])]
    Type(Token![->], Box<Type>),
    Default,
}

#[derive(Clone, Parse, ToTokens)]
pub struct ParenthesizedGenericArguments {
    #[syn(parenthesized)]
    paren: token::Paren,
    /// `(A, B)`
    #[syn(in = paren)]
    #[parse(Punctuated::parse_terminated)]
    pub inputs: Punctuated<Type, Token![,]>,
    /// `C`
    pub output: syn::ReturnType,
}

#[derive(Clone, ToTokens, Parse, From)]
pub enum PathArguments {
    /// The `<'a, T>` in `std::slice::iter<'a, T>`.
    #[parse(peek_func = |input| input.peek(Token![<]) || input.peek(Token![::]) && input.peek3(Token![<]))]
    AngleBracketed(AngleBracketedGenericArguments),
    // /// The `(A, B) -> C` in `Fn(A, B) -> C`.
    // #[parse(peek = token::Paren)]
    // Parenthesized(ParenthesizedGenericArguments),
    None,
}

#[derive(Clone, Parse, ToTokens)]
pub struct PathSegment {
    pub ident: Ident,
    pub arguments: PathArguments,
}

#[derive(Clone, Copy, Parse, ToTokens)]
pub struct PathCrate {
    tk_dollar: Token![$],
    pub tk_crate: Token![crate],
    colon: Token![::],
}

#[derive(Clone, Copy, ToTokens, Parse)]
pub enum PathLeading {
    #[parse(peek = Token![::])]
    Colon(Token![::]),
    #[parse(peek = Token![$])]
    Crate(PathCrate),
    None,
}

#[derive(Clone, ToTokens, Parse)]
pub struct Path {
    pub leading: PathLeading,
    #[parse(Punctuated::parse_separated_nonempty)]
    pub segments: Punctuated<PathSegment, Token![::]>,
}

impl Path {
    pub fn as_ident(&self) -> Option<&Ident> {
        let PathLeading::None = self.leading else { return None };
        if self.segments.len() != 1 || self.segments.trailing_punct() {
            return None;
        }
        self.ident()
    }
    pub fn ident(&self) -> Option<&Ident> {
        Some(&self.segments.last()?.ident)
    }
}

#[derive(Clone)]
pub struct QSelf {
    tk_lt: Token![<],
    pub ty: Box<Type>,
    pub position: usize,
    tk_as: Option<Token![as]>,
    tk_gt: Token![>],
}

#[derive(Clone)]
pub struct TypePath {
    pub qself: Option<QSelf>,
    pub path: Path,
}

#[derive(Clone, ToTokens, Parse)]
pub struct TypePtr {
    tk_star: Token![*],
    pub mutability: PtrMutability,
    pub ty: Box<Type>,
}

#[derive(Clone, ToTokens, Parse)]
pub struct TypeReference {
    tk_and: Token![&],
    #[parse(Region::parse_opt)]
    pub region: Option<Region>,
    pub mutability: Mutability,
    pub ty: Box<Type>,
}

#[derive(Clone, ToTokens, Parse)]
pub struct TypeSlice {
    #[syn(bracketed)]
    bracket: token::Bracket,
    #[syn(in = bracket)]
    pub ty: Box<Type>,
}

#[derive(Clone, ToTokens)]
pub struct TypeTuple {
    #[syn(parenthesized)]
    paren: token::Paren,
    #[syn(in = paren)]
    pub tys: Punctuated<Type, Token![,]>,
}

#[derive(Clone, ToTokens, Parse)]
pub struct TypeVar {
    tk_dollar: Token![$],
    pub ident: Ident,
}

#[derive(Clone, ToTokens, From)]
pub enum Type {
    /// A fixed size array type: `[T; n]`.
    Array(TypeArray),

    // /// A function pointer type: `fn(usize) -> bool`.
    // FnPtr(syn::TypeBareFn),
    /// A type contained within invisible delimiters.
    Group(TypeGroup),

    /// The never type: `!`.
    Never(TypeNever),

    /// A parenthesized type equivalent to the inner type.
    Paren(TypeParen),

    /// A path like `std::slice::Iter`, optionally qualified with a
    /// self-type as in `<Vec<T> as SomeTrait>::Associated`.
    Path(TypePath),

    /// A raw pointer type: `*const T` or `*mut T`.
    Ptr(TypePtr),

    /// A reference type: `&'a T` or `&'a mut T`.
    Reference(TypeReference),

    /// A dynamically sized slice type: `[T]`.
    Slice(TypeSlice),

    // /// A trait object type `dyn Bound1 + Bound2 + Bound3` where `Bound` is a
    // /// trait or a lifetime.
    // TraitObject(TypeTraitObject),
    /// A tuple type: `(A, B, C, String)`.
    Tuple(TypeTuple),

    /// A `TyVar` from `meta!($T:ty)`.
    TyVar(TypeVar),

    /// A languate item
    LangItem(LangItemWithArgs),
}

#[derive(Clone, ToTokens, Parse, From, Display)]
pub enum PlaceLocal {
    #[parse(peek = Ident)]
    #[display("{_0}")]
    Local(Ident),
    #[parse(peek = Token![self])]
    #[display("self")]
    SelfValue(Token![self]),
}

impl PlaceLocal {
    pub fn span(&self) -> Span {
        match self {
            PlaceLocal::Local(ident) => ident.span(),
            PlaceLocal::SelfValue(self_value) => self_value.span,
        }
    }
}

#[derive(Clone, ToTokens)]
pub struct PlaceParen {
    #[syn(parenthesized)]
    paren: token::Paren,
    #[syn(in = paren)]
    pub place: Box<Place>,
}

#[derive(Clone, ToTokens, Parse)]
pub struct PlaceDeref {
    tk_star: Token![*],
    pub place: Box<Place>,
}

#[derive(Clone, ToTokens)]
pub struct PlaceField {
    pub place: Box<Place>,
    tk_dot: Token![.],
    pub field: syn::Member,
}

#[derive(Clone, ToTokens)]
pub struct PlaceIndex {
    pub place: Box<Place>,
    #[syn(bracketed)]
    bracket: token::Bracket,
    #[syn(in = bracket)]
    pub index: Ident,
}

#[derive(Clone, ToTokens)]
pub struct PlaceConstIndex {
    pub place: Box<Place>,
    #[syn(bracketed)]
    bracket: token::Bracket,
    #[syn(in = bracket)]
    pub from_end: Option<Token![-]>,
    #[syn(in = bracket)]
    pub index: syn::Index,
    #[syn(in = bracket)]
    kw_of: kw::of,
    #[syn(in = bracket)]
    pub min_length: syn::Index,
}

#[derive(Clone, ToTokens)]
pub struct PlaceSubslice {
    pub place: Box<Place>,
    #[syn(bracketed)]
    bracket: token::Bracket,
    #[syn(in = bracket)]
    pub from: Option<syn::Index>,
    #[syn(in = bracket)]
    tk_colon: Token![:],
    #[syn(in = bracket)]
    pub from_end: Option<Token![-]>,
    #[syn(in = bracket)]
    pub to: syn::Index,
}

#[derive(Clone, ToTokens)]
pub struct PlaceDowncast {
    pub place: Box<Place>,
    tk_as: Token![as],
    pub variant: Ident,
}

#[derive(Clone, ToTokens, From)]
pub enum Place {
    /// `local`
    Local(PlaceLocal),
    /// `(place)`
    Paren(PlaceParen),
    /// `*place`
    Deref(PlaceDeref),
    /// `place.field`
    Field(PlaceField),
    /// `place[index]`
    Index(PlaceIndex),
    /// `place[const_index]`
    ConstIndex(PlaceConstIndex),
    /// `place[from..to]`
    Subslice(PlaceSubslice),
    /// `place as Variant`
    DownCast(PlaceDowncast),
}

impl Place {
    pub fn local(&self) -> &PlaceLocal {
        match self {
            Place::Local(local) => local,
            Place::Paren(PlaceParen { box place, .. })
            | Place::Deref(PlaceDeref { box place, .. })
            | Place::Field(PlaceField { box place, .. })
            | Place::Index(PlaceIndex { box place, .. })
            | Place::ConstIndex(PlaceConstIndex { box place, .. })
            | Place::Subslice(PlaceSubslice { box place, .. })
            | Place::DownCast(PlaceDowncast { box place, .. }) => place.local(),
        }
    }
    pub fn into_local(self) -> PlaceLocal {
        match self {
            Place::Local(local) => local,
            Place::Paren(PlaceParen { box place, .. })
            | Place::Deref(PlaceDeref { box place, .. })
            | Place::Field(PlaceField { box place, .. })
            | Place::Index(PlaceIndex { box place, .. })
            | Place::ConstIndex(PlaceConstIndex { box place, .. })
            | Place::Subslice(PlaceSubslice { box place, .. })
            | Place::DownCast(PlaceDowncast { box place, .. }) => place.into_local(),
        }
    }
}

#[derive(Clone, ToTokens, Parse)]
pub enum Const {
    #[parse(peek = syn::Lit)]
    Lit(syn::Lit),
    Path(TypePath),
}

#[derive(Clone, ToTokens, Parse)]
pub struct LangItemWithArgs {
    tk_pound: Token![#],
    #[syn(bracketed)]
    bracket: token::Bracket,
    #[syn(in = bracket)]
    kw_lang: kw::lang,
    #[syn(in = bracket)]
    tk_eq: Token![=],
    #[syn(in = bracket)]
    pub item: syn::LitStr,
    #[parse(|input| input.peek(Token![<]).then(|| input.parse()).transpose())]
    pub args: Option<AngleBracketedGenericArguments>,
}

#[derive(Clone, Parse, ToTokens)]
pub enum ConstOperandKind {
    #[parse(peek = syn::Lit)]
    Lit(syn::Lit),
    #[parse(peek = Token![#])]
    LangItem(LangItemWithArgs),
    Type(TypePath),
}

#[derive(Clone, ToTokens, Parse)]
pub struct ConstOperand {
    tk_const: Token![const],
    pub kind: ConstOperandKind,
}

#[derive(Clone, ToTokens, Parse)]
pub struct OperandCopy {
    kw_copy: kw::copy,
    pub place: Place,
}

#[derive(Clone, ToTokens, Parse)]
pub struct OperandMove {
    tk_move: Token![move],
    pub place: Place,
}

#[derive(Clone, Parse, ToTokens, From)]
pub enum Operand {
    #[parse(peek = Token![_])]
    Any(Token![_]),
    #[parse(peek = Token![..])]
    AnyMultiple(Token![..]),
    #[parse(peek = Token![move])]
    Move(OperandMove),
    #[parse(peek = kw::copy)]
    Copy(OperandCopy),
    #[parse(peek = Token![const])]
    Constant(ConstOperand),
}

#[derive(Clone, ToTokens, From)]
pub enum FnOperand {
    Copy(Parenthesized<OperandCopy>),
    Move(Parenthesized<OperandMove>),
    Type(TypePath),
    LangItem(LangItemWithArgs),
}

#[derive(Clone, Parse, ToTokens)]
pub struct Parenthesized<T: quote::ToTokens, P: parse::ParseFn<T> = parse::ParseParse> {
    #[syn(parenthesized)]
    paren: token::Paren,
    #[syn(in = paren)]
    #[parse(P::parse)]
    pub value: T,
    #[parse(|_| Ok(P::default()))]
    #[to_tokens(|_, _| {})]
    _parse: P,
}

#[derive(Clone)]
pub struct RvalueUse {
    paren: Option<token::Paren>,
    pub operand: Operand,
}

impl From<Parenthesized<OperandMove>> for RvalueUse {
    fn from(operand: Parenthesized<OperandMove>) -> Self {
        Self {
            paren: Some(operand.paren),
            operand: Operand::Move(operand.value),
        }
    }
}

impl From<Parenthesized<OperandCopy>> for RvalueUse {
    fn from(operand: Parenthesized<OperandCopy>) -> Self {
        Self {
            paren: Some(operand.paren),
            operand: Operand::Copy(operand.value),
        }
    }
}

#[derive(Clone, ToTokens, Parse)]
pub struct RvalueRepeat {
    #[syn(bracketed)]
    bracket: token::Bracket,
    #[syn(in = bracket)]
    pub operand: Operand,
    #[syn(in = bracket)]
    tk_semi: Token![;],
    #[syn(in = bracket)]
    pub len: syn::LitInt,
}

#[derive(Clone, ToTokens, Parse)]
pub struct RvalueRef {
    tk_and: Token![&],
    #[parse(Region::parse_opt)]
    pub region: Option<Region>,
    pub mutability: Mutability,
    pub place: Place,
}

#[derive(Clone, ToTokens, Parse)]
pub struct RvalueRawPtr {
    tk_and: Token![&],
    kw_raw: kw::raw,
    pub mutability: PtrMutability,
    pub place: Place,
}

#[derive(Clone, ToTokens, Parse)]
pub struct RvalueLen {
    kw_len: kw::Len,
    #[syn(parenthesized)]
    paren: token::Paren,
    #[syn(in = paren)]
    pub place: Place,
}

#[derive(Clone, Copy, ToTokens, Parse, From)]
pub enum CastKind {
    #[parse(peek = kw::PtrToPtr)]
    PtrToPtr(kw::PtrToPtr),
    #[parse(peek = kw::IntToInt)]
    IntToInt(kw::IntToInt),
    #[parse(peek = kw::Transmute)]
    Transmute(kw::Transmute),
}

#[derive(Clone, ToTokens, Parse)]
pub struct RvalueCast {
    pub operand: Operand,
    tk_as: Token![as],
    pub ty: Type,
    #[syn(parenthesized)]
    paren: token::Paren,
    #[syn(in = paren)]
    pub cast_kind: CastKind,
}

#[derive(Clone, Copy, Parse, ToTokens, From)]
#[rustfmt::skip]
pub enum BinOp {
    #[parse(peek = kw::Add)] Add(kw::Add),
    #[parse(peek = kw::Sub)] Sub(kw::Sub),
    #[parse(peek = kw::Mul)] Mul(kw::Mul),
    #[parse(peek = kw::Div)] Div(kw::Div),
    #[parse(peek = kw::Rem)] Rem(kw::Rem),
    #[parse(peek = kw::Lt)] Lt(kw::Lt),
    #[parse(peek = kw::Gt)] Gt(kw::Gt),
    #[parse(peek = kw::Le)] Le(kw::Le),
    #[parse(peek = kw::Ge)] Ge(kw::Ge),
    #[parse(peek = kw::Eq)] Eq(kw::Eq),
    #[parse(peek = kw::Ne)] Ne(kw::Ne),
    #[parse(peek = kw::Offset)] Offset(kw::Offset),
}

#[derive(Clone, ToTokens, Parse)]
pub struct RvalueBinOp {
    pub op: BinOp,
    #[syn(parenthesized)]
    paren: token::Paren,
    #[syn(in = paren)]
    pub lhs: Operand,
    #[syn(in = paren)]
    tk_comma: Token![,],
    #[syn(in = paren)]
    pub rhs: Operand,
}

#[derive(Clone, Copy, ToTokens, Parse, From)]
#[rustfmt::skip]
pub enum NullOp {
    #[parse(peek = kw::SizeOf)] SizeOf(kw::SizeOf),
    #[parse(peek = kw::AlignOf)] AlignOf(kw::AlignOf),
    // OffsetOf(kw::OffsetOf),
}

#[derive(Clone, ToTokens, Parse)]
pub struct RvalueNullOp {
    pub op: NullOp,
    #[syn(parenthesized)]
    paren: token::Paren,
    #[syn(in = paren)]
    pub ty: Type,
}

#[derive(Clone, Copy, ToTokens, Parse, From)]
#[rustfmt::skip]
pub enum UnOp {
    #[parse(peek = kw::Neg)] Neg(kw::Neg),
    #[parse(peek = kw::Not)] Not(kw::Not),
    #[parse(peek = kw::PtrMetadata)] PtrMetadata(kw::PtrMetadata),
}

#[derive(Clone, ToTokens, Parse)]
pub struct RvalueUnOp {
    pub op: UnOp,
    #[syn(parenthesized)]
    paren: token::Paren,
    #[syn(in = paren)]
    pub operand: Operand,
}

#[derive(Clone, ToTokens, Parse)]
pub struct RvalueDiscriminant {
    kw_discr: kw::discriminant,
    #[syn(parenthesized)]
    paren: token::Paren,
    #[syn(in = paren)]
    pub place: Place,
}

#[derive(Clone, ToTokens, Parse)]
pub struct AggregateArray {
    // bracket: token::Bracket,
    // pub ty: Box<Type>,
    // tk_semi: Token![;],
    // tk_underscore: Token![_],
    // kw_from: kw::from,
    pub operands: BracketedOperands,
}

#[derive(Clone, ToTokens, Parse, From)]
pub struct AggregateTuple {
    #[parse(ParenthesizedOperands::parse_tuple_like)]
    pub operands: ParenthesizedOperands,
}

#[derive(Clone, ToTokens, Parse, From)]
pub struct StructField {
    pub ident: Ident,
    tk_colon: Token![:],
    pub operand: Operand, /* FIXME _marker: std::marker::PhantomData `::` <u8> */
}

#[derive(Clone, ToTokens, Parse)]
pub struct StructFields {
    #[syn(braced)]
    brace: token::Brace,
    #[syn(in = brace)]
    #[parse(Punctuated::parse_terminated)]
    pub fields: Punctuated<StructField, Token![,]>,
}

#[derive(Clone, ToTokens, Parse, From)]
pub enum PathOrLangItem {
    #[parse(peek = Token![#])]
    LangItem(LangItemWithArgs),
    Path(Path),
}

#[derive(Clone, ToTokens, Parse)]
pub struct AggregateAdtStruct {
    pub adt: PathOrLangItem,
    pub fields: StructFields,
}

#[derive(Clone, ToTokens, Parse)]
pub struct AggregateAdtUnit {
    pub adt: PathOrLangItem,
}

#[derive(Clone, ToTokens, Parse)]
pub struct Ctor {
    pub pound: Token![#],
    #[syn(bracketed)]
    bracket: token::Bracket,
    #[syn(in = bracket)]
    pub kw_ctor: kw::ctor,
}

#[derive(Clone, ToTokens, Parse)]
pub struct AggregateAdtTuple {
    ctor: Ctor,
    pub adt: Path,
    pub fields: ParenthesizedOperands,
}

#[derive(Clone, ToTokens, Parse)]
pub struct AggregateRawPtr {
    pub ty: TypePtr,
    kw_from: kw::from,
    #[syn(parenthesized)]
    paren: token::Paren,
    #[syn(in = paren)]
    pub ptr: Operand,
    #[syn(in = paren)]
    tk_comma: Token![,],
    #[syn(in = paren)]
    pub metadata: Operand,
}

#[derive(Clone, ToTokens, From)]
pub enum RvalueAggregate {
    Array(AggregateArray),
    Tuple(AggregateTuple),
    AdtStruct(AggregateAdtStruct),
    AdtTuple(AggregateAdtTuple),
    AdtUnit(AggregateAdtUnit),
    RawPtr(AggregateRawPtr),
}

#[derive(Clone, ToTokens, From)]
pub enum Rvalue {
    Any(Token![_]),
    Use(RvalueUse),
    Repeat(RvalueRepeat),
    Ref(RvalueRef),
    RawPtr(RvalueRawPtr),
    Len(RvalueLen),
    Cast(RvalueCast),
    BinaryOp(RvalueBinOp),
    NullaryOp(RvalueNullOp),
    UnaryOp(RvalueUnOp),
    Discriminant(RvalueDiscriminant),
    Aggregate(RvalueAggregate),
    // ShallowInitBox(Operand<'tcx>, Ty<'tcx>),
    // CopyForDeref(CopyForDerefValue),
}

pub type ParenthesizedOperands = Parenthesized<Punctuated<Operand, Token![,]>, parse::PunctuatedParseTerminated>;

#[derive(Clone, Parse, ToTokens)]
pub struct BracketedOperands {
    #[syn(bracketed)]
    bracket: token::Bracket,
    #[syn(in = bracket)]
    #[parse(Punctuated::parse_terminated)]
    pub operands: Punctuated<Operand, Token![,]>,
}

#[derive(Clone, ToTokens, Parse)]
pub struct Call {
    pub func: FnOperand,
    pub operands: ParenthesizedOperands,
}

pub struct Macro<K, C, P = parse::ParseParse> {
    kw: K,
    tk_bang: Token![!],
    delim: syn::MacroDelimiter,
    pub content: C,
    parse: P,
}

// `Token![!]` is not `Clone`able.
impl<K: Clone, C: Clone, P: Clone> Clone for Macro<K, C, P> {
    fn clone(&self) -> Self {
        Self {
            kw: self.kw.clone(),
            tk_bang: self.tk_bang,
            delim: self.delim.clone(),
            content: self.content.clone(),
            parse: self.parse.clone(),
        }
    }
}

#[derive(Clone, ToTokens, From)]
pub enum RvalueOrCall {
    Rvalue(Rvalue),
    Call(Call),
}

#[derive(Clone, ToTokens, Parse)]
pub struct UsePath {
    tk_use: Token![use],
    pub path: Path,
    tk_semi: Token![;],
}

#[derive(Clone, ToTokens, Parse)]
pub struct LocalInit {
    tk_eq: Token![=],
    pub rvalue_or_call: RvalueOrCall,
}

#[derive(Clone, Parse, ToTokens)]
pub struct LocalDecl {
    tk_let: Token![let],
    tk_mut: Option<Token![mut]>,
    pub local: PlaceLocal,
    tk_colon: Token![:],
    pub ty: Type,
    #[parse(|input| input.peek(Token![=]).then(|| input.parse()).transpose())]
    pub init: Option<LocalInit>,
    tk_semi: Token![;],
}

impl LocalDecl {
    pub fn is_mut(&self) -> bool {
        self.tk_mut.is_some()
    }
}

#[derive(Clone, ToTokens, Parse)]
pub struct Assign {
    pub place: Place,
    tk_eq: Token![=],
    pub rvalue_or_call: RvalueOrCall,
}

#[derive(Clone, ToTokens, Parse)]
pub struct CallIgnoreRet {
    tk_underscore: Token![_],
    tk_eq: Token![=],
    pub call: Call,
}

#[derive(Clone, ToTokens, Parse)]
pub struct Drop {
    kw_drop: kw::drop,
    #[syn(parenthesized)]
    paren: token::Paren,
    #[syn(in = paren)]
    pub place: Place,
}

#[derive(Clone, ToTokens, Parse, From)]
pub enum Declaration {
    #[parse(peek = Token![type])]
    TypeDecl(TypeDecl),
    #[parse(peek = Token![use])]
    UsePath(UsePath),
    #[parse(peek = Token![let])]
    LocalDecl(LocalDecl),
}

#[derive(Clone, ToTokens, Parse)]
pub struct Loop {
    pub label: Option<syn::Label>,
    tk_loop: Token![loop],
    pub block: Block,
}

#[derive(Clone, ToTokens, Parse)]
pub enum Control {
    #[parse(peek = Token![break])]
    Break(Token![break], Option<syn::Label>),
    #[parse(peek = Token![continue])]
    Continue(Token![continue], Option<syn::Label>),
}

#[derive(Clone, ToTokens, Parse, IntoIterator, Deref)]
pub struct Many<T: syn::parse::Parse + quote::ToTokens>(
    #[parse(|input| Ok(std::iter::from_fn(|| input.parse().ok()).collect()))]
    #[to_tokens(|tokens, elems: &Vec<T>| elems.iter().for_each(|elem| elem.to_tokens(tokens)))]
    #[into_iterator(owned, ref)]
    #[deref]
    pub Vec<T>,
);

#[derive(Clone, ToTokens, Parse)]
pub struct Block {
    #[syn(braced)]
    brace: token::Brace,
    #[syn(in = brace)]
    pub statements: Many<Statement>,
}

#[derive(Clone, ToTokens, Parse)]
pub enum SwitchBody {
    #[parse(peek = token::Brace)]
    Block(Block),
    Statement(Statement<syn::parse::Nothing>, Token![,]),
}

#[derive(Clone, ToTokens, Parse)]
pub struct SwitchTarget {
    pub value: SwitchValue,
    tk_arrow: Token![=>],
    pub body: SwitchBody,
}

#[derive(Clone, ToTokens, From)]
pub enum SwitchValue {
    Bool(syn::LitBool),
    Int(syn::LitInt),
    Underscore(Token![_]),
}

#[derive(Clone, Parse, ToTokens)]
pub struct SwitchInt {
    kw_switch_int: kw::switchInt,
    #[syn(parenthesized)]
    paren: token::Paren,
    #[syn(in = paren)]
    pub operand: Operand,
    #[syn(braced)]
    brace: token::Brace,
    #[syn(in = brace)]
    pub targets: Many<SwitchTarget>,
}

#[derive(Clone, Parse, ToTokens)]
pub enum Statement<End: quote::ToTokens + syn::parse::Parse = Token![;]> {
    #[parse(peek = Token![_])]
    Call(CallIgnoreRet, End),
    #[parse(peek_func = |input| input.peek(kw::drop) && input.peek2(token::Paren))]
    Drop(Drop, End),
    #[parse(peek_func = Control::peek)]
    Control(Control, End),
    #[parse(peek = Token![loop])]
    Loop(Loop),
    #[parse(peek = kw::switchInt)]
    SwitchInt(SwitchInt),
    Assign(Assign, End),
}

#[derive(Clone, Copy, ToTokens, Parse, From)]
pub enum MetaKind {
    Ty(kw::ty),
}

#[derive(Clone, ToTokens, Parse)]
pub struct MetaItem {
    tk_dollar: Token![$],
    pub ident: Ident,
    tk_colon: Token![:],
    pub kind: MetaKind,
}

#[derive(ToTokens)]
pub struct Meta {
    pub meta: Macro<kw::meta, Punctuated<MetaItem, Token![,]>, parse::PunctuatedParseTerminated>,
    tk_semi: Option<Token![;]>,
}

#[derive(Parse, ToTokens)]
pub struct Mir {
    pub metas: Many<Meta>,
    pub declarations: Many<Declaration>,
    pub statements: Many<Statement>,
}
