use syn::parse::{Parse, ParseStream, Result};
use syn::token::Token;
use syn::{Ident, Token};

use crate::*;

#[derive(thiserror::Error, Debug)]
pub enum ParseError {
    #[error("unrecognized region {0}, expect `'static` or `'_`")]
    UnrecognizedRegion(String),
    #[error("`,` is needed for single-element tuple")]
    ExpectTuple,
    #[error("type declaration with generic arguments are not supported")]
    TypeWithGenericsNotSupported,
    #[error("expect integer suffix")]
    ExpectIntSuffix,
    #[error("unrecognized integer suffix {0}")]
    UnrecognizedIntSuffix(String),
    #[error("possible missing operands?")]
    MissingOperands,
    #[error("expect `{}`", _0())]
    ExpectToken(fn() -> &'static str),
}

impl Region {
    pub fn parse_opt(input: ParseStream<'_>) -> Result<Option<Self>> {
        input.peek(syn::Lifetime).then(|| input.parse()).transpose()
    }
}

impl Parse for Region {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let lifetime: syn::Lifetime = input.parse()?;
        let ident = &lifetime.ident;
        let kind = if ident == "static" {
            Token![static](ident.span()).into()
        } else if ident == "_" {
            Token![_](ident.span()).into()
        } else {
            return Err(syn::Error::new_spanned(
                &lifetime,
                ParseError::UnrecognizedRegion(lifetime.to_string()),
            ));
        };
        let span = lifetime.span();
        Ok(Region { span, kind })
    }
}

impl Parse for GenericConst {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(if input.peek(token::Brace) {
            let content;
            let brace = Some(syn::braced!(content in input));
            let konst = content.parse()?;
            GenericConst { brace, konst }
        } else {
            let konst = input.parse()?;
            GenericConst { brace: None, konst }
        })
    }
}

pub(super) fn parse_angle_bracketed<T: Parse, P: token::Token + Parse>(
    input: ParseStream<'_>,
) -> Result<Punctuated<T, P>> {
    let mut punctuated = Punctuated::new();

    loop {
        let value = input.parse()?;
        punctuated.push_value(value);
        if !P::peek(input.cursor()) {
            break;
        }
        let punct = input.parse()?;
        punctuated.push_punct(punct);
        if input.peek(Token![>]) {
            break;
        }
    }

    Ok(punctuated)
}

impl AngleBracketedGenericArguments {
    pub fn parse_turbofish(input: ParseStream<'_>) -> Result<Self> {
        Ok(AngleBracketedGenericArguments {
            tk_colon2: Some(input.parse()?),
            tk_lt: input.parse()?,
            args: parse_angle_bracketed(input)?,
            tk_gt: input.parse()?,
        })
    }
    pub fn peek(input: ParseStream<'_>) -> bool {
        input.peek(Token![<]) || input.peek(Token![::]) && input.peek3(Token![<])
    }
    pub fn parse_opt(input: ParseStream<'_>) -> Result<Option<Self>> {
        Self::peek(input).then(|| input.parse()).transpose()
    }
}

impl PathArguments {
    fn parse_turbofish(input: ParseStream<'_>) -> Result<Self> {
        Ok(if input.peek(Token![::]) {
            PathArguments::AngleBracketed(input.call(AngleBracketedGenericArguments::parse_turbofish)?)
        } else {
            PathArguments::None
        })
    }
}

impl PathSegment {
    fn parse_turbofish(input: ParseStream<'_>) -> Result<Self> {
        Ok(PathSegment {
            ident: input.parse()?,
            arguments: input.call(PathArguments::parse_turbofish)?,
        })
    }
}

impl Path {
    fn lookahead(lookahead: &syn::parse::Lookahead1<'_>) -> bool {
        lookahead.peek(Ident) || lookahead.peek(Token![::]) || lookahead.peek(Token![$])
    }
}

impl QSelf {
    fn parse_with_path(input: ParseStream<'_>) -> Result<(Self, Path)> {
        let tk_lt = input.parse()?;
        let ty = input.parse()?;
        let tk_as = input.parse()?;
        let tk_gt;
        let leading;
        let mut segments = Punctuated::new();
        let mut position = 0;
        match tk_as {
            None => {
                tk_gt = input.parse()?;
                leading = input.parse()?;
            },
            Some(_) => {
                leading = input.parse()?;
                loop {
                    segments.push_value(input.parse()?);
                    position += 1;
                    if input.peek(Token![>]) {
                        tk_gt = input.parse()?;
                        break;
                    }
                    segments.push_punct(input.parse()?);
                }
            },
        }
        let qself = QSelf {
            tk_lt,
            ty,
            position,
            tk_as,
            tk_gt,
        };
        let path = Path { leading, segments };
        Ok((qself, path))
    }
}

impl TypePath {
    fn lookahead(lookahead: &syn::parse::Lookahead1<'_>) -> bool {
        lookahead.peek(Token![<]) || Path::lookahead(lookahead)
    }
}

impl Parse for TypePath {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(if input.peek(Token![<]) {
            let (qself, mut path) = input.call(QSelf::parse_with_path)?;
            if input.peek(Ident) {
                path.segments.push_value(input.call(PathSegment::parse_turbofish)?);
            }
            while input.peek(Token![::]) {
                path.segments.push_punct(input.parse()?);
                path.segments.push_value(input.call(PathSegment::parse_turbofish)?);
            }
            TypePath {
                qself: Some(qself),
                path,
            }
        } else {
            TypePath {
                qself: None,
                path: input.parse()?,
            }
        })
    }
}

fn is_single_no_trailing<T, P>(mut punctuated: Punctuated<T, P>) -> std::result::Result<T, Punctuated<T, P>> {
    if punctuated.len() == 1
        && !punctuated.trailing_punct()
        && let Some(syn::punctuated::Pair::End(single)) = punctuated.pop()
    {
        Ok(single)
    } else {
        Err(punctuated)
    }
}

fn is_tuple_like<T, P>(punctuated: Punctuated<T, P>) -> std::result::Result<Punctuated<T, P>, T> {
    match is_single_no_trailing(punctuated) {
        Ok(single) => Err(single),
        Err(punctuated) => Ok(punctuated),
    }
}

impl Type {
    fn parse_tuple_or_paren(input: ParseStream<'_>) -> Result<Type> {
        let content;
        let paren = syn::parenthesized!(content in input);
        Ok(match is_single_no_trailing(Punctuated::parse_terminated(&content)?) {
            Ok(ty) => Type::Paren(TypeParen {
                paren,
                value: Box::new(ty),
                _parse: Default::default(),
            }),
            Err(tys) => TypeTuple { paren, tys }.into(),
        })
    }
    fn parse_array_or_slice(input: ParseStream<'_>) -> Result<Type> {
        let content;
        let bracket = syn::bracketed!(content in input);
        let ty = content.parse()?;
        Ok(if content.peek(Token![;]) {
            let tk_semi = content.parse()?;
            let len = content.parse()?;
            Type::Array(TypeArray {
                bracket,
                ty,
                tk_semi,
                len,
            })
        } else {
            TypeSlice { bracket, ty }.into()
        })
    }
}

impl Parse for Type {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        if input.peek(token::Paren) {
            input.call(Type::parse_tuple_or_paren)
        } else if input.peek(token::Bracket) {
            input.call(Type::parse_array_or_slice)
        } else if input.peek(Token![_]) {
            Ok(Type::Any(input.parse()?))
        } else if input.peek(Token![*]) {
            Ok(Type::Ptr(input.parse()?))
        } else if input.peek(Token![&]) {
            Ok(Type::Reference(input.parse()?))
        } else if input.peek(Token![!]) {
            Ok(Type::Never(input.parse()?))
        } else if input.peek(Token![#]) {
            Ok(Type::LangItem(input.parse()?))
        } else if input.peek(Token![$]) {
            if input.peek2(Token![crate]) {
                Ok(Type::Path(input.parse()?))
            } else {
                Ok(Type::TyVar(input.parse()?))
            }
        } else if input.peek(Token![Self]) {
            Ok(Type::SelfType(input.parse()?))
        } else {
            Ok(Type::Path(input.parse()?))
        }
    }
}

impl Parse for PlaceParen {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        Ok(PlaceParen {
            paren: syn::parenthesized!(content in input),
            place: Box::new(Place::parse_allowing_cast(&content)?),
        })
    }
}

impl Place {
    fn parse_field(self, input: ParseStream<'_>) -> Result<Self> {
        let tk_dot = input.parse()?;
        let tk_dollar = input.parse()?;
        let field = input.parse()?;
        Ok(Place::Field(PlaceField {
            place: Box::new(self),
            tk_dot,
            tk_dollar,
            field,
        }))
    }
    fn parse_downcast(self, input: ParseStream<'_>) -> Result<Self> {
        let tk_as = input.parse()?;
        let tk_dollar = input.parse()?;
        let variant = input.parse()?;
        Ok(Place::DownCast(PlaceDowncast {
            place: Box::new(self),
            tk_as,
            tk_dollar,
            variant,
        }))
    }
    fn parse_index(self, bracket: token::Bracket, content: ParseStream<'_>) -> Result<Self> {
        let index = content.parse()?;
        Ok(Place::Index(PlaceIndex {
            place: Box::new(self),
            bracket,
            index,
        }))
    }
    fn parse_const_index(self, bracket: token::Bracket, content: ParseStream<'_>) -> Result<Self> {
        Ok(Place::ConstIndex(PlaceConstIndex {
            place: Box::new(self),
            bracket,
            from_end: content.parse()?,
            index: content.parse()?,
            kw_of: content.parse()?,
            min_length: content.parse()?,
        }))
    }
    fn parse_subslice(self, bracket: token::Bracket, content: ParseStream<'_>) -> Result<Self> {
        let from = if content.peek(syn::LitInt) {
            Some(content.parse()?)
        } else {
            None
        };
        let tk_colon = content.parse()?;
        let from_end = content.parse()?;
        let to = content.parse()?;
        Ok(Place::Subslice(PlaceSubslice {
            place: Box::new(self),
            bracket,
            from,
            tk_colon,
            from_end,
            to,
        }))
    }
}

impl Place {
    fn parse_impl(input: ParseStream<'_>, allows_cast: bool) -> Result<Self> {
        let mut place = if input.peek(token::Paren) {
            Place::Paren(input.parse()?)
        } else if input.peek(Token![*]) {
            Place::Deref(input.parse()?)
        } else {
            Place::Local(input.parse()?)
        };
        loop {
            place = if input.peek(Token![.]) {
                place.parse_field(input)?
            } else if allows_cast && input.peek(Token![as]) && input.peek2(Ident) {
                place.parse_downcast(input)?
            } else if input.peek(token::Bracket) {
                let content;
                let bracket = syn::bracketed!(content in input);
                if content.peek(Token![$]) && content.peek2(Ident) {
                    place.parse_index(bracket, &content)?
                } else if content.peek(Token![:]) || content.peek2(Token![:]) {
                    place.parse_subslice(bracket, &content)?
                } else {
                    place.parse_const_index(bracket, &content)?
                }
            } else {
                break Ok(place);
            };
        }
    }
    fn parse_allowing_cast(input: ParseStream<'_>) -> Result<Self> {
        Place::parse_impl(input, true)
    }
}

impl Parse for Place {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Place::parse_impl(input, false)
    }
}

impl Operand {
    fn lookahead(lookahead: &syn::parse::Lookahead1<'_>) -> bool {
        lookahead.peek(Token![_])
            || lookahead.peek(Token![..])
            || lookahead.peek(Token![move])
            || lookahead.peek(kw::copy)
            || lookahead.peek(Token![const])
    }
}

impl FnOperand {
    fn lookahead(lookahead: &syn::parse::Lookahead1<'_>) -> bool {
        lookahead.peek(token::Paren) || PathOrLangItem::lookahead(lookahead)
    }
}

impl Parse for FnOperand {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let lookahead = input.lookahead1();
        Ok(if lookahead.peek(token::Paren) {
            let content;
            syn::parenthesized!(content in input.fork());
            let lookahead = content.lookahead1();
            if lookahead.peek(Token![move]) {
                FnOperand::Move(input.parse()?)
            } else if lookahead.peek(kw::copy) {
                FnOperand::Copy(input.parse()?)
            } else if lookahead.peek(Token![#]) {
                FnOperand::LangItem(input.parse()?)
            } else if TypePath::lookahead(&lookahead) {
                FnOperand::Type(input.parse()?)
            } else {
                return Err(lookahead.error());
            }
        } else if lookahead.peek(Token![#]) {
            FnOperand::LangItem(input.parse()?)
        } else if lookahead.peek(Token![$]) && input.peek2(Ident) {
            FnOperand::FnPat(input.parse()?, input.parse()?)
        } else if TypePath::lookahead(&lookahead) {
            FnOperand::Type(input.parse()?)
        } else {
            return Err(lookahead.error());
        })
    }
}

impl ParenthesizedOperands {
    pub fn parse_tuple_like(input: ParseStream<'_>) -> Result<Self> {
        let content;
        let paren = syn::parenthesized!(content in input);
        let operands =
            is_tuple_like(Punctuated::parse_terminated(&content)?).map_err(|_| input.error(ParseError::ExpectTuple))?;
        Ok(ParenthesizedOperands {
            paren,
            value: operands,
            _parse: Default::default(),
        })
    }
}

impl PathOrLangItem {
    fn lookahead(lookahead: &syn::parse::Lookahead1<'_>) -> bool {
        lookahead.peek(Token![#]) || Path::lookahead(lookahead)
    }
}

impl Rvalue {
    fn parse_array(input: ParseStream<'_>) -> Result<Self> {
        let content;
        let bracket = syn::bracketed!(content in input);
        let operand = content.parse()?;
        let lookahead = content.lookahead1();
        if lookahead.peek(Token![;]) {
            Ok(Rvalue::Repeat(RvalueRepeat {
                bracket,
                operand,
                tk_semi: content.parse()?,
                len: content.parse()?,
            }))
        } else if lookahead.peek(Token![,]) {
            let mut operands = Punctuated::new();
            operands.push_value(operand);
            operands.push_punct(content.parse()?);
            while !content.is_empty() {
                operands.push_value(content.parse()?);
                if content.is_empty() {
                    break;
                }
                operands.push_punct(content.parse()?);
            }
            Ok(RvalueAggregate::Array(AggregateArray {
                operands: BracketedOperands { bracket, operands },
            })
            .into())
        } else {
            Err(lookahead.error())
        }
    }
    fn parse_ref_or_raw_ptr(input: ParseStream<'_>) -> Result<Self> {
        let tk_and = input.parse()?;
        Ok(if input.peek(kw::raw) {
            Rvalue::RawPtr(RvalueRawPtr {
                tk_and,
                kw_raw: input.parse()?,
                mutability: input.parse()?,
                place: input.parse()?,
            })
        } else {
            Rvalue::Ref(RvalueRef {
                tk_and,
                region: input.peek(syn::Lifetime).then(|| input.parse()).transpose()?,
                mutability: input.parse()?,
                place: input.parse()?,
            })
        })
    }
}

impl RvalueOrCall {
    fn parse_operand(input: ParseStream<'_>) -> Result<Self> {
        let operand: Operand = input.parse()?;
        Ok(if input.peek(Token![as]) {
            let content;
            Rvalue::Cast(RvalueCast {
                operand,
                tk_as: input.parse()?,
                ty: input.parse()?,
                paren: syn::parenthesized!(content in input),
                cast_kind: content.parse()?,
            })
            .into()
        } else {
            Rvalue::Use(RvalueUse { paren: None, operand }).into()
        })
    }

    #[allow(irrefutable_let_patterns)]
    fn parse_fn_operand_or_aggregate(input: ParseStream<'_>, lookahead: syn::parse::Lookahead1<'_>) -> Result<Self> {
        Ok(
            if lookahead.peek(Token![*]) && (input.peek2(Token![const]) || input.peek2(Token![mut])) {
                Rvalue::Aggregate(RvalueAggregate::RawPtr(input.parse()?)).into()
            } else if lookahead.peek(Token![#])
                && let forked = input.fork()
                && forked.parse::<Ctor>().is_ok()
            {
                Rvalue::Aggregate(RvalueAggregate::AdtTuple(input.parse()?)).into()
            } else if Path::lookahead(&lookahead)
                && let forked = input.fork()
                && forked.parse::<Path>().is_ok()
                && forked.peek(token::Brace)
            {
                Rvalue::Aggregate(RvalueAggregate::AdtStruct(input.parse()?)).into()
            } else if FnOperand::lookahead(&lookahead) && input.fork().parse::<FnOperand>().is_ok() {
                let operand: FnOperand = input.parse()?;
                if input.peek(token::Paren) || input.peek(token::Brace) {
                    Call {
                        func: operand,
                        operands: input.parse()?,
                    }
                    .into()
                } else {
                    use FnOperand::{Copy, FnPat, LangItem, Move, Type};
                    use RvalueUse;
                    RvalueOrCall::Rvalue(match operand {
                        Move(inner) => RvalueUse::from(inner).into(),
                        Copy(inner) => RvalueUse::from(inner).into(),
                        Type(TypePath { qself: Some(_), .. }) => return Err(input.error(ParseError::MissingOperands)),
                        Type(TypePath { qself: None, path }) => {
                            RvalueAggregate::AdtUnit(AggregateAdtUnit { adt: path.into() }).into()
                        },
                        LangItem(lang_item) => {
                            RvalueAggregate::AdtUnit(AggregateAdtUnit { adt: lang_item.into() }).into()
                        },
                        FnPat(_, _fn_pat) => unimplemented!("FnPat not implemented"),
                    })
                }
            } else if lookahead.peek(token::Paren) {
                Rvalue::Aggregate(RvalueAggregate::Tuple(input.parse()?)).into()
            } else {
                return Err(lookahead.error());
            },
        )
    }

    fn parse_opertion_or_call(input: ParseStream<'_>) -> Result<Self> {
        Ok(if input.peek(kw::Len) {
            Rvalue::Len(input.parse()?).into()
        } else if input.fork().parse::<NullOp>().is_ok() {
            Rvalue::NullaryOp(input.parse()?).into()
        } else if input.fork().parse::<UnOp>().is_ok() {
            Rvalue::UnaryOp(input.parse()?).into()
        } else if input.fork().parse::<BinOp>().is_ok() {
            Rvalue::BinaryOp(input.parse()?).into()
        } else if input.peek(kw::discriminant) {
            Rvalue::Discriminant(input.parse()?).into()
        } else {
            RvalueOrCall::Call(input.parse()?)
        })
    }
}

impl Parse for RvalueOrCall {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(token::Bracket) {
            Ok(Rvalue::parse_array(input)?.into())
        } else if lookahead.peek(Token![&]) {
            Ok(Rvalue::parse_ref_or_raw_ptr(input)?.into())
        } else if lookahead.peek(Token![_]) {
            Ok(Rvalue::Any(input.parse()?).into())
        } else if lookahead.peek(Token![<]) {
            Ok(RvalueOrCall::Call(input.parse()?))
        } else if Operand::lookahead(&lookahead) {
            RvalueOrCall::parse_operand(input)
        } else if lookahead.peek(syn::Ident) && input.peek2(token::Paren) {
            RvalueOrCall::parse_opertion_or_call(input)
        } else {
            RvalueOrCall::parse_fn_operand_or_aggregate(input, lookahead)
        }
    }
}

impl SwitchValue {
    pub fn parse_int_value(input: ParseStream<'_>) -> Result<syn::LitInt> {
        let value: syn::LitInt = input.parse()?;
        const INT_SUFFIXES: &[&str] = &[
            "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
        ];
        let suffix = value.suffix().trim_start_matches("_");
        if suffix.is_empty() {
            return Err(input.error(ParseError::ExpectIntSuffix));
        } else if !INT_SUFFIXES.contains(&suffix) {
            return Err(syn::Error::new(
                value.span(),
                ParseError::UnrecognizedIntSuffix(suffix.to_string()),
            ));
        }
        Ok(value)
    }
}

impl Parse for SwitchValue {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(if input.peek(Token![_]) {
            SwitchValue::Underscore(input.parse()?)
        } else if input.peek(syn::LitBool) {
            SwitchValue::Bool(input.parse()?)
        } else {
            SwitchValue::Int(SwitchValue::parse_int_value(input)?)
        })
    }
}

impl Control {
    pub fn peek(input: ParseStream<'_>) -> bool {
        input.peek(Token![break]) || input.peek(Token![continue])
    }
}

#[macro_export]
macro_rules! macro_delimiter {
    ($content:ident in $input:expr) => {{
        let lookahead = $input.lookahead1();
        if lookahead.peek(::syn::token::Paren) {
            ::syn::MacroDelimiter::Paren(::syn::parenthesized!($content in $input))
        } else if lookahead.peek(::syn::token::Bracket) {
            ::syn::MacroDelimiter::Bracket(::syn::bracketed!($content in $input))
        } else if lookahead.peek(::syn::token::Brace) {
            ::syn::MacroDelimiter::Brace(::syn::braced!($content in $input))
        } else {
            return Err(lookahead.error());
        }
    }}
}

impl<K: Parse, C, P: ParseFn<C>> Parse for Macro<K, C, P> {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        Ok(Macro {
            kw: input.parse()?,
            tk_bang: input.parse()?,
            delim: macro_delimiter!(content in input),
            content: P::parse(&content)?,
            _parse: PhantomData,
        })
    }
}

impl<T, P, End> Parse for PunctuatedWithEnd<T, P, End>
where
    T: quote::ToTokens + Parse,
    P: quote::ToTokens + Token + Parse,
    End: quote::ToTokens + Token + Parse,
{
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut punctuated = Punctuated::new();
        while !input.cursor().eof() && !End::peek(input.cursor()) {
            punctuated.push_value(input.parse()?);
            if !P::peek(input.cursor()) {
                if !input.is_empty() {
                    return Err(input.error(ParseError::ExpectToken(P::display)));
                }
                break;
            }
            punctuated.push_punct(input.parse()?);
        }
        let end = input.parse()?;
        Ok(PunctuatedWithEnd { punctuated, end })
    }
}

impl<P: syn::parse::Parse + quote::ToTokens + token::Token, T: syn::parse::Parse + quote::ToTokens> PunctAnd<P, T> {
    pub fn parse_opt(input: ParseStream<'_>) -> Result<Option<Self>> {
        P::peek(input.cursor()).then(|| input.parse()).transpose()
    }
}

impl<P: syn::parse::Parse + quote::ToTokens + token::Token, I: quote::ToTokens, Parse: ParseFn<I>>
    Attribute<P, I, Parse>
{
    pub fn parse_opt(input: ParseStream<'_>) -> Result<Option<Self>> {
        if let Some((punct, cursor)) = input.cursor().punct()
            && punct.as_char() == '#'
            && let Some((cursor, ..)) = cursor.group(proc_macro2::Delimiter::Bracket)
            && P::peek(cursor)
        {
            return input.parse().map(Some);
        }
        Ok(None)
    }
}

impl SelfParam {
    pub fn peek(input: ParseStream<'_>) -> bool {
        input.peek(Token![self])
            || input.peek(Token![&])
                && (input.peek2(Token![self]) || input.peek2(Token![mut]) && input.peek3(Token![self]))
    }
}

impl ParamPat {
    pub fn parse_opt(input: ParseStream<'_>) -> Result<Option<Self>> {
        let forked = input.fork();
        _ = forked.parse::<Mutability>();
        (forked.peek(Token![$]) && forked.peek2(Ident) && forked.peek3(Token![:]))
            .then(|| input.parse())
            .transpose()
    }
}

#[derive(Default, Clone, Copy)]
pub struct ParseParse;

#[derive(Default, Clone, Copy)]
pub struct PunctuatedParseTerminated;

pub trait ParseFn<T> {
    fn parse(input: ParseStream<'_>) -> Result<T>;
    fn parse_many(input: ParseStream<'_>) -> Result<Vec<T>> {
        Ok(std::iter::from_fn(|| {
            if Self::parse(&input.fork()).is_ok() {
                return Self::parse(input).ok();
            }
            None
        })
        .collect())
    }
}

impl<T: Parse> ParseFn<T> for ParseParse {
    fn parse(input: ParseStream<'_>) -> Result<T> {
        input.parse()
    }
}

impl<T: Parse, P: Parse> ParseFn<Punctuated<T, P>> for PunctuatedParseTerminated {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Punctuated<T, P>> {
        Punctuated::parse_terminated(input)
    }
}
