#![feature(box_patterns)]
#![feature(let_chains)]

use proc_macro2::Span;
use syn::punctuated::Punctuated;
use syn::{token, Ident, Token};

#[macro_use]
mod auto_derive;

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
    syn::custom_keyword!(Discriminant);
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

auto_derive! {
    #[auto_derive(ToTokens, From)]
    #[derive(Clone, Copy)]
    pub enum RegionKind {
        ReAny(Token!(_)),
        ReStatic(Token![static]),
    }
}

auto_derive! {
    #[auto_derive(ToTokens, From)]
    #[derive(Default, Clone, Copy)]
    pub enum Mutability {
        #[default]
        Not,
        Mut(Token![mut]),
    }
}

auto_derive! {
    #[auto_derive(ToTokens, Parse, From)]
    #[derive(Clone, Copy)]
    pub enum PtrMutability {
        Const(Token![const]),
        Mut(Token![mut]),
    }
}

auto_derive! {
    #[auto_derive(ToTokens)]
    #[derive(Clone)]
    pub struct TypeDecl {
        tk_type: Token![type],
        pub ident: Ident,
        tk_eq: Token![=],
        pub ty: Type,
        tk_semi: Token![;],
    }
}

#[derive(Clone)]
pub struct TypeArray {
    bracket: token::Bracket,
    pub ty: Box<Type>,
    tk_semi: Token![;],
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

auto_derive! {
    #[auto_derive(ToTokens, Parse, From)]
    #[derive(Clone, Copy)]
    pub struct TypeNever {
        tk_bang: Token![!],
    }

}

#[derive(Clone)]
pub struct TypeParen {
    paren: token::Paren,
    pub ty: Box<Type>,
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
pub struct GenericConst {
    brace: Option<token::Brace>,
    pub konst: Const,
}

auto_derive! {
    #[auto_derive(ToTokens, From)]
    #[derive(Clone)]
    pub enum GenericArgument {
        /// A region argument.
        Region(Region),
        /// A type argument.
        Type(Type),
        /// A const argument.
        Const(GenericConst),
    }

}

auto_derive! {
    #[auto_derive(ToTokens)]
    #[derive(Clone)]
    pub struct AngleBracketedGenericArguments {
        tk_colon2: Option<Token![::]>,
        tk_lt: Token![<],
        pub args: Punctuated<GenericArgument, Token![,]>,
        tk_gt: Token![>],
    }
}

#[derive(Clone)]
pub enum ReturnType {
    Default,
    Type(Token![->], Box<Type>),
}

#[derive(Clone)]
pub struct ParenthesizedGenericArguments {
    paren: token::Paren,
    /// `(A, B)`
    pub inputs: Punctuated<Type, Token![,]>,
    /// `C`
    pub output: syn::ReturnType,
}

auto_derive! {
    #[auto_derive(ToTokens, From)]
    #[derive(Clone)]
    pub enum PathArguments {
        None,
        /// The `<'a, T>` in `std::slice::iter<'a, T>`.
        AngleBracketed(AngleBracketedGenericArguments),
        // /// The `(A, B) -> C` in `Fn(A, B) -> C`.
        // Parenthesized(ParenthesizedGenericArguments),
    }

}

auto_derive! {
    #[auto_derive(Parse, ToTokens)]
    #[derive(Clone)]
    pub struct PathSegment {
        pub ident: Ident,
        pub arguments: PathArguments,
    }
}

auto_derive! {
    #[auto_derive(Parse, ToTokens)]
    #[derive(Clone, Copy)]
    pub struct PathCrate {
        tk_dollar: Token![$],
        pub tk_crate: Token![crate],
        colon: Token![::],
    }
}

auto_derive! {
    #[auto_derive(ToTokens)]
    #[derive(Clone, Copy)]
    pub enum PathLeading {
        None,
        Colon(Token![::]),
        Crate(PathCrate),
    }
}

auto_derive! {
    #[auto_derive(ToTokens)]
    #[derive(Clone)]
    pub struct Path {
        pub leading: PathLeading,
        pub segments: Punctuated<PathSegment, Token![::]>,
    }
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
pub struct TypePath {
    pub qself: Option<QSelf>,
    pub path: Path,
}

auto_derive! {
    #[auto_derive(ToTokens, Parse)]
    #[derive(Clone)]
    pub struct TypePtr {
        tk_star: Token![*],
        pub mutability: PtrMutability,
        pub ty: Box<Type>,
    }

}

auto_derive! {
    #[auto_derive(ToTokens)]
    #[derive(Clone)]
    pub struct TypeReference {
        tk_and: Token![&],
        pub region: Option<Region>,
        pub mutability: Mutability,
        pub ty: Box<Type>,
    }

}

#[derive(Clone)]
pub struct TypeSlice {
    bracket: token::Bracket,
    pub ty: Box<Type>,
}

#[derive(Clone)]
pub struct TypeTuple {
    paren: token::Paren,
    pub tys: Punctuated<Type, Token![,]>,
}

auto_derive! {
    #[auto_derive(ToTokens, Parse)]
    #[derive(Clone)]
    pub struct TypeVar {
        tk_dollar: Token![$],
        pub ident: Ident,
    }
}

auto_derive! {
    #[auto_derive(ToTokens, From)]
    #[derive(Clone)]
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
        LangItem(LangItem),
    }
}

auto_derive! {
    #[auto_derive(ToTokens, Parse, From)]
    #[derive(Clone)]
    pub enum PlaceLocal {
        Local(Ident),
        SelfValue(Token![self]),
    }
}

impl std::fmt::Display for PlaceLocal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlaceLocal::Local(ident) => ident.fmt(f),
            PlaceLocal::SelfValue(_) => f.write_str("self"),
        }
    }
}

#[derive(Clone)]
pub struct PlaceParen {
    paren: token::Paren,
    pub place: Box<Place>,
}

auto_derive! {
    #[auto_derive(ToTokens, Parse)]
    #[derive(Clone)]
    pub struct PlaceDeref {
        tk_star: Token![*],
        pub place: Box<Place>,
    }
}

auto_derive! {
    #[auto_derive(ToTokens)]
    #[derive(Clone)]
    pub struct PlaceField {
        pub place: Box<Place>,
        tk_dot: Token![.],
        pub field: syn::Member,
    }
}

#[derive(Clone)]
pub struct PlaceIndex {
    pub place: Box<Place>,
    bracket: token::Bracket,
    pub index: Ident,
}

#[derive(Clone)]
pub struct PlaceConstIndex {
    pub place: Box<Place>,
    bracket: token::Bracket,
    pub from_end: Option<Token![-]>,
    pub index: syn::Index,
    kw_of: kw::of,
    pub min_length: syn::Index,
}

#[derive(Clone)]
pub struct PlaceSubslice {
    pub place: Box<Place>,
    bracket: token::Bracket,
    pub from: Option<syn::Index>,
    tk_colon: Token![:],
    pub from_end: Option<Token![-]>,
    pub to: syn::Index,
}

auto_derive! {
    #[auto_derive(ToTokens)]
    #[derive(Clone)]
    pub struct PlaceDowncast {
        pub place: Box<Place>,
        tk_as: Token![as],
        pub variant: Ident,
    }

}

auto_derive! {
    #[auto_derive(ToTokens, From)]
    #[derive(Clone)]
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

auto_derive! {
    #[auto_derive(ToTokens, Parse)]
    #[derive(Clone)]
    pub struct ConstLit {
        tk_const: Token![const],
        pub lit: syn::Lit,
    }
}

auto_derive! {
    #[auto_derive(ToTokens)]
    #[derive(Clone)]
    pub enum Const {
        Lit(syn::Lit),
        Path(TypePath),
    }
}

#[derive(Clone)]
pub struct LangItem {
    tk_pound: Token![#],
    bracket: token::Bracket,
    kw_lang: kw::lang,
    tk_eq: Token![=],
    pub item: syn::LitStr,
    pub args: Option<AngleBracketedGenericArguments>,
}

auto_derive! {
    #[auto_derive(ToTokens)]
    #[derive(Clone)]
    pub enum ConstOperand {
        Lit(ConstLit),
        Path(TypePath),
        LangItem(LangItem),
    }
}

auto_derive! {
    #[auto_derive(ToTokens, Parse)]
    #[derive(Clone)]
    pub struct OperandCopy {
        kw_copy: kw::copy,
        pub place: Place,
    }

}

auto_derive! {
    #[auto_derive(ToTokens, Parse)]
    #[derive(Clone)]
    pub struct OperandMove {
        tk_move: Token![move],
        pub place: Place,
    }

}

auto_derive! {
    #[auto_derive(ToTokens, From)]
    #[derive(Clone)]
    pub enum Operand {
        Copy(OperandCopy),
        Move(OperandMove),
        Constant(ConstOperand),
    }
}

#[derive(Clone)]
pub struct RvalueUse {
    paren: Option<token::Paren>,
    pub operand: Operand,
}

#[derive(Clone)]
pub struct RvalueRepeat {
    bracket: token::Bracket,
    pub operand: Operand,
    tk_semi: Token![;],
    pub len: syn::LitInt,
}

auto_derive! {
    #[auto_derive(ToTokens)]
    #[derive(Clone)]
    pub struct RvalueRef {
        tk_and: Token![&],
        pub region: Option<Region>,
        pub mutability: Mutability,
        pub place: Place,
    }

}

auto_derive! {
    #[auto_derive(ToTokens, Parse)]
    #[derive(Clone)]
    pub struct RvalueRawPtr {
        tk_and: Token![&],
        kw_raw: kw::raw,
        pub mutability: PtrMutability,
        pub place: Place,
    }

}

#[derive(Clone)]
pub struct RvalueLen {
    kw_len: kw::Len,
    paren: token::Paren,
    pub place: Place,
}

auto_derive! {
    #[auto_derive(ToTokens, Parse, From)]
    #[derive(Clone, Copy)]
    pub enum CastKind {
        PtrToPtr(kw::PtrToPtr),
        IntToInt(kw::IntToInt),
        Transmute(kw::Transmute),
    }

}

#[derive(Clone)]
pub struct RvalueCast {
    pub operand: Operand,
    tk_as: Token![as],
    pub ty: Type,
    paren: token::Paren,
    pub cast_kind: CastKind,
}

auto_derive! {
    #[auto_derive(ToTokens, Parse, From)]
    #[derive(Clone, Copy)]
    pub enum BinOp {
        Add(kw::Add),
        Sub(kw::Sub),
        Mul(kw::Mul),
        Div(kw::Div),
        Rem(kw::Rem),
        Lt(kw::Lt),
        Gt(kw::Gt),
        Le(kw::Le),
        Ge(kw::Ge),
        Eq(kw::Eq),
        Ne(kw::Ne),
        Offset(kw::Offset),
    }
}

#[derive(Clone)]
pub struct RvalueBinOp {
    pub op: BinOp,
    paren: token::Paren,
    pub lhs: Operand,
    tk_comma: Token![,],
    pub rhs: Operand,
}

auto_derive! {
    #[auto_derive(ToTokens, Parse, From)]
    #[derive(Clone, Copy)]
    pub enum NullOp {
        SizeOf(kw::SizeOf),
        AlignOf(kw::AlignOf),
        // OffsetOf(kw::OffsetOf),
    }

}

#[derive(Clone)]
pub struct RvalueNullOp {
    pub op: NullOp,
    paren: token::Paren,
    pub ty: Type,
}

auto_derive! {
    #[auto_derive(ToTokens, Parse, From)]
    #[derive(Clone, Copy)]
    pub enum UnOp {
        Neg(kw::Neg),
        Not(kw::Not),
        PtrMetadata(kw::PtrMetadata),
    }

}

#[derive(Clone)]
pub struct RvalueUnOp {
    pub op: UnOp,
    paren: token::Paren,
    pub operand: Operand,
}

#[derive(Clone)]
pub struct RvalueDiscriminant {
    kw_discr: kw::Discriminant,
    paren: token::Paren,
    pub place: Place,
}

#[derive(Clone)]
pub struct AggregateArray {
    // bracket: token::Bracket,
    // pub ty: Box<Type>,
    // tk_semi: Token![;],
    // tk_underscore: Token![_],
    // kw_from: kw::from,
    pub operands: BracketedOperands,
}

auto_derive! {
    #[auto_derive(ToTokens, From)]
    #[derive(Clone)]
    pub struct AggregateTuple {
        pub operands: ParenthesizedOperands,
    }
}

auto_derive! {
    #[auto_derive(ToTokens, Parse)]
    #[derive(Clone)]
    pub struct StructField {
        pub ident: Ident,
        tk_colon: Token![:],
        pub operand: Operand, /* FIXME _marker: std::marker::PhantomData `::` <u8> */
    }
}

#[derive(Clone)]
pub struct StructFields {
    brace: token::Brace,
    pub fields: Punctuated<StructField, Token![,]>,
}

auto_derive! {
    #[auto_derive(ToTokens, Parse)]
    #[derive(Clone)]
    pub struct AggregateAdt {
        pub adt: Path,
        pub fields: StructFields,
    }
}

#[derive(Clone)]
pub struct Ctor {
    pub pound: Token![#],
    pub bracket: token::Bracket,
    pub kw_ctor: kw::ctor,
}

auto_derive! {
    #[auto_derive(ToTokens, Parse)]
    #[derive(Clone)]
    pub struct AggregateAdtTuple {
        ctor: Ctor,
        pub adt: Path,
        pub fields: ParenthesizedOperands,
    }
}

#[derive(Clone)]
pub struct AggregateRawPtr {
    pub ty: TypePtr,
    kw_from: kw::from,
    paren: token::Paren,
    pub ptr: Operand,
    tk_comma: Token![,],
    pub metadata: Operand,
}

auto_derive! {
    #[auto_derive(ToTokens, From)]
    #[derive(Clone)]
    pub enum RvalueAggregate {
        Array(AggregateArray),
        Tuple(AggregateTuple),
        Adt(AggregateAdt),
        AdtTuple(AggregateAdtTuple),
        RawPtr(AggregateRawPtr),
    }
}

auto_derive! {
    #[auto_derive(ToTokens, From)]
    #[derive(Clone)]
    pub enum Rvalue {
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

}

#[derive(Clone)]
pub struct ParenthesizedOperands {
    paren: token::Paren,
    pub operands: Punctuated<Operand, Token![,]>,
}

#[derive(Clone)]
pub struct BracketedOperands {
    bracket: token::Bracket,
    pub operands: Punctuated<Operand, Token![,]>,
}

#[derive(Clone)]
pub struct BracedOperands {
    brace: token::Brace,
    pub operands: Punctuated<Operand, Token![,]>,
    tk_dotdot: Option<Token![..]>,
}

auto_derive! {
    #[auto_derive(ToTokens, From)]
    #[derive(Clone)]
    pub enum CallOperands {
        Ordered(ParenthesizedOperands),
        Unordered(BracedOperands),
    }
}

auto_derive! {
    #[auto_derive(ToTokens, Parse)]
    #[derive(Clone)]
    pub struct Call {
        pub func: Operand,
        pub operands: CallOperands,
    }
}

pub struct Macro<K, C, P = parse::ParseParse> {
    kw: K,
    tk_bang: Token![!],
    delim: syn::MacroDelimiter,
    pub content: C,
    parse: P,
}

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

auto_derive! {
    #[auto_derive(ToTokens, From)]
    #[derive(Clone)]
    pub enum RvalueOrCall {
        Rvalue(Rvalue),
        Call(Call),
        Any(Token![_]),
    }
}

auto_derive! {
    #[auto_derive(ToTokens, Parse)]
    #[derive(Clone)]
    pub struct UsePath {
        tk_use: Token![use],
        pub path: Path,
        tk_semi: Token![;],
    }
}

auto_derive! {
    #[auto_derive(ToTokens, Parse)]
    #[derive(Clone)]
    pub struct LocalInit {
        tk_eq: Token![=],
        pub rvalue_or_call: RvalueOrCall,
    }
}

auto_derive! {
    #[auto_derive(ToTokens)]
    #[derive(Clone)]
    pub struct LocalDecl {
        tk_let: Token![let],
        tk_mut: Option<Token![mut]>,
        pub ident: Ident,
        tk_colon: Token![:],
        pub ty: Type,
        pub init: Option<LocalInit>,
        tk_semi: Token![;],
    }
}

impl LocalDecl {
    pub fn is_mut(&self) -> bool {
        self.tk_mut.is_some()
    }
}

auto_derive! {
    #[auto_derive(ToTokens, Parse)]
    #[derive(Clone)]
    pub struct Assign {
        pub place: Place,
        tk_eq: Token![=],
        pub rvalue_or_call: RvalueOrCall,
    }
}

auto_derive! {
    #[auto_derive(ToTokens, Parse)]
    #[derive(Clone)]
    pub struct CallIgnoreRet {
        tk_underscore : Token![_],
        tk_eq: Token![=],
        pub call: Call,
    }
}

#[derive(Clone)]
pub struct Drop {
    kw_drop: kw::drop,
    paren: token::Paren,
    pub place: Place,
}

auto_derive! {
    #[auto_derive(Parse, ToTokens)]
    #[derive(Clone)]
    pub struct SelfDecl {
        tk_let: Token![let],
        tk_mut: Option<Token![mut]>,
        tk_self: Token![self],
        tk_colon: Token![:],
        pub ty: Type,
        tk_semi: Token![;],
    }
}

auto_derive! {
    #[auto_derive(ToTokens, From)]
    #[derive(Clone)]
    pub enum Declaration {
        TypeDecl(TypeDecl),
        UsePath(UsePath),
        LocalDecl(LocalDecl),
        SelfDecl(SelfDecl),
    }
}

impl Declaration {
    fn can_start(input: syn::parse::ParseStream<'_>) -> bool {
        input.peek(Token![type]) || input.peek(Token![use]) || input.peek(Token![let])
    }
}

auto_derive! {
    #[auto_derive(ToTokens, Parse)]
    #[derive(Clone)]
    pub struct Loop {
        pub label: Option<syn::Label>,
        tk_loop: Token![loop],
        pub block: Block,
    }
}

#[derive(Clone)]
pub enum Control {
    Break(Token![break], Option<syn::Label>),
    Continue(Token![continue], Option<syn::Label>),
}

#[derive(Clone)]
pub struct Block {
    brace: token::Brace,
    pub statements: Vec<Statement>,
}

#[derive(Clone)]
pub enum SwitchBody {
    Statement(Statement<syn::parse::Nothing>, Token![,]),
    Block(Block),
}

auto_derive! {
    #[auto_derive(ToTokens, Parse)]
    #[derive(Clone)]
    pub struct SwitchTarget {
        pub value: SwitchValue,
        tk_arrow: Token![=>],
        pub body: SwitchBody,
    }
}

auto_derive! {
    #[auto_derive(ToTokens, From)]
    #[derive(Clone)]
    pub enum SwitchValue {
        Bool(syn::LitBool),
        Int(syn::LitInt),
        Underscore(Token![_]),
    }
}

#[derive(Clone)]
pub struct SwitchInt {
    kw_switch_int: kw::switchInt,
    paren: token::Paren,
    pub operand: Operand,
    brace: token::Brace,
    pub targets: Vec<SwitchTarget>,
}

#[derive(Clone)]
pub enum Statement<End = Token![;]> {
    Assign(Assign, End),
    Call(CallIgnoreRet, End),
    Drop(Drop, End),
    Control(Control, End),
    Loop(Loop),
    SwitchInt(SwitchInt),
}

auto_derive! {
    #[auto_derive(ToTokens, Parse, From)]
    #[derive(Clone, Copy)]
    pub enum MetaKind {
        Ty(kw::ty),
    }
}

auto_derive! {
    #[auto_derive(ToTokens, Parse)]
    #[derive(Clone)]
    pub struct MetaItem {
        tk_dollar: Token![$],
        pub ident: Ident,
        tk_colon: Token![:],
        pub kind: MetaKind,
    }
}

auto_derive! {
    #[auto_derive(ToTokens)]
    pub struct Meta {
        pub meta: Macro<kw::meta, Punctuated<MetaItem, Token![,]>, parse::PunctuatedParseTerminated>,
        tk_semi: Option<Token![;]>,
    }
}

pub struct Mir {
    pub metas: Vec<Meta>,
    pub declarations: Vec<Declaration>,
    pub statements: Vec<Statement>,
}
