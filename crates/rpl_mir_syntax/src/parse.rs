use quote::ToTokens;
use syn::parse::{Parse, ParseStream, Result};
use syn::{Ident, Token};

use crate::*;

macro_rules! Parse {
    (
        enum $ident:ident {
            $( $( #[$variant_meta:meta] )* $variant:ident ($ty:ty) ),* $(,)?
        }
    ) => {
        impl ::syn::parse::Parse for $ident {
            fn parse(input: ::syn::parse::ParseStream<'_>) -> ::syn::parse::Result<Self> {
                $(
                    if let Ok(Some(parsed)) = input.parse() {
                        return Ok($ident::$variant(parsed));
                    }
                )*
                Err(input.error(concat!("expect ", stringify!($ident))))
            }
        }
    };

    (
        struct $ident:ident {
            $( $field_vis:vis $field:ident: $ty:ty, )*
        }
    ) => {
        impl ::syn::parse::Parse for $ident {
            fn parse(input: ::syn::parse::ParseStream<'_>) -> ::syn::parse::Result<Self> {
                $( let $field = input.parse()?; )*
                Ok($ident { $( $field, )* })
            }
        }
    };
}

#[derive(thiserror::Error, Debug)]
pub enum ParseError {
    #[error("unrecognized region {0}, expect `'static` or `'_`")]
    UnrecognizedRegion(String),
    #[error("expect `{{` or `(`")]
    ExpectBraceOrParenthesis,
    #[error("`,` is needed for single-element tuple")]
    ExpectTuple,
    #[error("type declaration with generic arguments are not supported")]
    TypeWithGenericsNotSupported,
    #[error("expect `(`, `[`, or `{{")]
    ExpectDelimiter,
    #[error("expect `type`, `use`, or `let")]
    ExpectDeclaration,
    #[error("expect integer suffix")]
    ExpectIntSuffix,
    #[error("unrecognized integer suffix {0}")]
    UnrecognizedIntSuffix(String),
    #[error("expect `,` or `;`")]
    ExpectCommaOrSemicolon,
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

impl Parse for Mutability {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(match input.parse()? {
            None => Mutability::Not,
            Some(mutability) => Mutability::Mut(mutability),
        })
    }
}

impl Parse for TypeDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let tk_type = input.parse()?;
        let ident = input.parse()?;
        if input.peek(Token![<]) {
            return Err(input.error(ParseError::TypeWithGenericsNotSupported));
        }
        let tk_eq = input.parse()?;
        let ty = input.parse()?;
        let tk_semi = input.parse()?;
        Ok(TypeDecl {
            tk_type,
            ident,
            tk_eq,
            ty,
            tk_semi,
        })
    }
}

impl Parse for ReturnType {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(if input.peek(Token![->]) {
            ReturnType::Type(input.parse()?, input.parse()?)
        } else {
            ReturnType::Default
        })
    }
}

impl Parse for Const {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(if input.peek(syn::Lit) {
            Const::Lit(input.parse()?)
        } else {
            Const::Path(input.parse()?)
        })
    }
}

impl Parse for LangItem {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        Ok(LangItem {
            tk_pound: input.parse()?,
            bracket: syn::bracketed!(content in input),
            kw_lang: content.parse()?,
            tk_eq: content.parse()?,
            item: content.parse()?,
            args: input.peek(Token![<]).then(|| input.parse()).transpose()?,
        })
    }
}

impl Parse for ConstOperand {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(if input.peek(Token![const]) {
            ConstOperand::Lit(input.parse()?)
        } else if input.peek(Token![#]) {
            ConstOperand::LangItem(input.parse()?)
        } else {
            ConstOperand::Path(input.parse()?)
        })
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

impl Parse for GenericArgument {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(if input.peek(syn::Lifetime) {
            GenericArgument::Region(input.parse()?)
        } else if input.fork().parse::<Type>().is_ok() {
            GenericArgument::Type(input.parse()?)
        } else {
            GenericArgument::Const(input.parse()?)
        })
    }
}

fn parse_angle_bracketed<T: Parse, P: token::Token + Parse>(input: ParseStream<'_>) -> Result<Punctuated<T, P>> {
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
}

impl Parse for AngleBracketedGenericArguments {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(AngleBracketedGenericArguments {
            tk_colon2: input.parse()?,
            tk_lt: input.parse()?,
            args: parse_angle_bracketed(input)?,
            tk_gt: input.parse()?,
        })
    }
}

impl Parse for ParenthesizedGenericArguments {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        Ok(ParenthesizedGenericArguments {
            paren: syn::parenthesized!(content in input),
            inputs: Punctuated::parse_terminated(&content)?,
            output: input.parse()?,
        })
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

impl Parse for PathArguments {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(
            if input.peek(Token![<]) || input.peek(Token![::]) && input.peek2(Token![<]) {
                PathArguments::AngleBracketed(input.parse()?)
            // } else if input.peek(token::Paren) {
            //     PathArguments::Parenthesized(input.parse()?)
            } else {
                PathArguments::None
            },
        )
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

impl Parse for PathLeading {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        if input.peek(Token![::]) {
            Ok(PathLeading::Colon(input.parse()?))
        } else if input.peek(Token![$]) {
            Ok(PathLeading::Crate(input.parse()?))
        } else {
            Ok(PathLeading::None)
        }
    }
}

impl Parse for Path {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let leading: PathLeading = input.parse()?;
        let segments = Punctuated::parse_separated_nonempty(input)?;
        Ok(Path { leading, segments })
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

impl Parse for TypeReference {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(TypeReference {
            tk_and: input.parse()?,
            region: input.peek(syn::Lifetime).then(|| input.parse()).transpose()?,
            mutability: input.parse()?,
            ty: input.parse()?,
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
                ty: Box::new(ty),
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
        let field = input.parse()?;
        Ok(Place::Field(PlaceField {
            place: Box::new(self),
            tk_dot,
            field,
        }))
    }
    fn parse_downcast(self, input: ParseStream<'_>) -> Result<Self> {
        let tk_as = input.parse()?;
        let variant = input.parse()?;
        Ok(Place::DownCast(PlaceDowncast {
            place: Box::new(self),
            tk_as,
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
                if content.peek(Ident) {
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

impl Parse for Operand {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(if input.peek(Token![move]) {
            Operand::Move(input.parse()?)
        } else if input.peek(kw::copy) {
            Operand::Copy(input.parse()?)
        } else {
            Operand::Constant(input.parse()?)
        })
    }
}

impl ParenthesizedOperands {
    fn parse_tuple_like(input: ParseStream<'_>) -> Result<Self> {
        let content;
        let paren = syn::parenthesized!(content in input);
        let operands =
            is_tuple_like(Punctuated::parse_terminated(&content)?).map_err(|_| input.error(ParseError::ExpectTuple))?;
        Ok(ParenthesizedOperands { paren, operands })
    }
}

impl Parse for ParenthesizedOperands {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        Ok(ParenthesizedOperands {
            paren: syn::parenthesized!(content in input),
            operands: Punctuated::parse_terminated(&content)?,
        })
    }
}

impl Parse for BracketedOperands {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        Ok(BracketedOperands {
            bracket: syn::bracketed!(content in input),
            operands: Punctuated::parse_terminated(&content)?,
        })
    }
}

impl Parse for BracedOperands {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        let brace = syn::braced!(content in input);
        let operands = Punctuated::parse_separated_nonempty(&content)?;
        let tk_dotdot = operands.trailing_punct().then(|| content.parse()).transpose()?;
        Ok(BracedOperands {
            brace,
            operands,
            tk_dotdot,
        })
    }
}

impl Parse for CallOperands {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(if input.peek(token::Paren) {
            CallOperands::Ordered(input.parse()?)
        } else if input.peek(token::Brace) {
            CallOperands::Unordered(input.parse()?)
        } else {
            return Err(input.error(ParseError::ExpectBraceOrParenthesis));
        })
    }
}

impl Parse for RvalueRef {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(RvalueRef {
            tk_and: input.parse()?,
            region: input.peek(syn::Lifetime).then(|| input.parse()).transpose()?,
            mutability: input.parse()?,
            place: input.parse()?,
        })
    }
}

impl Parse for RvalueCast {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let operand: Operand = input.parse()?;
        eprintln!("{}", operand.to_token_stream());
        let content;
        Ok(RvalueCast {
            // operand: input.parse()?,
            operand,
            tk_as: input.parse()?,
            ty: input.parse()?,
            paren: syn::parenthesized!(content in input),
            cast_kind: content.parse()?,
        })
    }
}

impl Parse for RvalueLen {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        Ok(RvalueLen {
            kw_len: input.parse()?,
            paren: syn::parenthesized!(content in input),
            place: content.parse()?,
        })
    }
}

impl Parse for RvalueNullOp {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        Ok(RvalueNullOp {
            op: input.parse()?,
            paren: syn::parenthesized!(content in input),
            ty: content.parse()?,
        })
    }
}

impl Parse for RvalueUnOp {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        Ok(RvalueUnOp {
            op: input.parse()?,
            paren: syn::parenthesized!(content in input),
            operand: content.parse()?,
        })
    }
}

impl Parse for RvalueBinOp {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        Ok(RvalueBinOp {
            op: input.parse()?,
            paren: syn::parenthesized!(content in input),
            lhs: content.parse()?,
            tk_comma: content.parse()?,
            rhs: content.parse()?,
        })
    }
}

impl Parse for RvalueDiscriminant {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        Ok(RvalueDiscriminant {
            kw_discr: input.parse()?,
            paren: syn::parenthesized!(content in input),
            place: content.parse()?,
        })
    }
}

impl Parse for StructFields {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        Ok(StructFields {
            brace: syn::braced!(content in input),
            fields: Punctuated::parse_terminated(&content)?,
        })
    }
}

impl Parse for AggregateTuple {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(AggregateTuple {
            operands: ParenthesizedOperands::parse_tuple_like(input)?,
        })
    }
}

impl Parse for AggregateRawPtr {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        Ok(AggregateRawPtr {
            ty: input.parse()?,
            kw_from: input.parse()?,
            paren: syn::parenthesized!(content in input),
            ptr: content.parse()?,
            tk_comma: content.parse()?,
            metadata: content.parse()?,
        })
    }
}

impl Rvalue {
    fn parse_array(input: ParseStream<'_>) -> Result<Self> {
        let content;
        let bracket = syn::bracketed!(content in input);
        let operand = content.parse()?;
        if content.peek(Token![;]) {
            Ok(Rvalue::Repeat(RvalueRepeat {
                bracket,
                operand,
                tk_semi: content.parse()?,
                len: content.parse()?,
            }))
        } else if !content.is_empty() {
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
            Err(content.error(ParseError::ExpectCommaOrSemicolon))
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
    #[allow(irrefutable_let_patterns)]
    fn parse_operand_any_or_aggregate(input: ParseStream<'_>) -> Result<Self> {
        Ok(
            if input.peek(Token![*]) && (input.peek2(Token![const]) || input.peek2(Token![mut])) {
                Rvalue::Aggregate(RvalueAggregate::RawPtr(input.parse()?)).into()
            } else if let forked = input.fork()
                && forked.parse::<Path>().is_ok()
                && forked.peek(token::Brace)
            {
                Rvalue::Aggregate(RvalueAggregate::Adt(input.parse()?)).into()
            } else if input.fork().parse::<Operand>().is_ok() {
                let operand: Operand = input.parse()?;
                if input.peek(Token![as]) {
                    let content;
                    Rvalue::Cast(RvalueCast {
                        operand,
                        tk_as: input.parse()?,
                        ty: input.parse()?,
                        paren: syn::parenthesized!(content in input),
                        cast_kind: content.parse()?,
                    })
                    .into()
                } else if input.peek(token::Paren) || input.peek(token::Brace) {
                    Call {
                        func: operand,
                        operands: input.parse()?,
                    }
                    .into()
                } else {
                    Rvalue::Use(RvalueUse { paren: None, operand }).into()
                }
            } else {
                Rvalue::Aggregate(RvalueAggregate::Tuple(input.parse()?)).into()
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
        } else if input.peek(kw::Discriminant) {
            Rvalue::Discriminant(input.parse()?).into()
        } else {
            RvalueOrCall::Call(input.parse()?)
        })
    }
}

impl Parse for RvalueOrCall {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        if input.peek(token::Bracket) {
            Ok(Rvalue::parse_array(input)?.into())
        } else if input.peek(Token![&]) {
            Ok(Rvalue::parse_ref_or_raw_ptr(input)?.into())
        } else if input.peek(Token![_]) {
            Ok(RvalueOrCall::Any(input.parse()?))
        } else if input.peek(Token![<]) {
            Ok(RvalueOrCall::Call(input.parse()?))
        } else if input.peek(kw::copy) || input.peek(Token![move]) || input.peek(Token![const]) {
            input.call(RvalueOrCall::parse_operand_any_or_aggregate)
        } else if input.peek(syn::Ident) && input.peek2(token::Paren) {
            input.call(RvalueOrCall::parse_opertion_or_call)
        } else {
            input.call(RvalueOrCall::parse_operand_any_or_aggregate)
        }
    }
}

impl Parse for Drop {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        Ok(Drop {
            kw_drop: input.parse()?,
            paren: syn::parenthesized!(content in input),
            place: content.parse()?,
        })
    }
}

impl Parse for LocalDecl {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(LocalDecl {
            tk_let: input.parse()?,
            tk_mut: input.parse()?,
            ident: input.parse()?,
            tk_colon: input.parse()?,
            ty: input.parse()?,
            init: input.peek(Token![=]).then(|| input.parse()).transpose()?,
            tk_semi: input.parse()?,
        })
    }
}

impl Parse for Declaration {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(if input.peek(Token![type]) {
            Declaration::TypeDecl(input.parse()?)
        } else if input.peek(Token![use]) {
            Declaration::UsePath(input.parse()?)
        } else if input.peek(Token![let]) && (input.peek2(Token![self]) || input.peek3(Token![self])) {
            Declaration::SelfDecl(input.parse()?)
        } else if input.peek(Token![let]) {
            Declaration::LocalDecl(input.parse()?)
        } else {
            return Err(input.error(ParseError::ExpectDeclaration));
        })
    }
}

impl Parse for Block {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        Ok(Block {
            brace: syn::braced!(content in input),
            statements: std::iter::from_fn(|| content.parse().ok()).collect(),
        })
    }
}

impl Parse for SwitchBody {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(if input.peek(token::Brace) {
            SwitchBody::Block(input.parse()?)
        } else {
            SwitchBody::Statement(input.parse()?, input.parse()?)
        })
    }
}

impl Parse for SwitchValue {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(if input.peek(Token![_]) {
            SwitchValue::Underscore(input.parse()?)
        } else if input.peek(syn::LitBool) {
            SwitchValue::Bool(input.parse()?)
        } else {
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
            value.into()
        })
    }
}

impl Parse for SwitchInt {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut content;
        Ok(SwitchInt {
            kw_switch_int: input.parse()?,
            paren: syn::parenthesized!(content in input),
            operand: content.parse()?,
            brace: syn::braced!(content in input),
            targets: std::iter::from_fn(|| content.parse().ok()).collect(),
        })
    }
}

impl Parse for Control {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(if input.peek(Token![break]) {
            Control::Break(input.parse()?, input.parse()?)
        } else {
            Control::Continue(input.parse()?, input.parse()?)
        })
    }
}

impl<End: Parse> Parse for Statement<End> {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(if input.peek(kw::drop) && input.peek2(token::Paren) {
            Statement::Drop(input.parse()?, input.parse()?)
        } else if input.peek(Token![break]) || input.peek(Token![continue]) {
            Statement::Control(input.parse()?, input.parse()?)
        } else if input.peek(Token![loop]) {
            Statement::Loop(input.parse()?)
        } else if input.peek(kw::switchInt) {
            Statement::SwitchInt(input.parse()?)
        } else if input.peek(Token![_]) {
            Statement::Call(input.parse()?, input.parse()?)
        } else {
            Statement::Assign(input.parse()?, input.parse()?)
        })
    }
}

#[macro_export]
macro_rules! macro_delimiter {
    ($content:ident in $input:expr) => {
        if $input.peek(::syn::token::Paren) {
            ::syn::MacroDelimiter::Paren(::syn::parenthesized!($content in $input))
        } else if $input.peek(::syn::token::Bracket) {
            ::syn::MacroDelimiter::Bracket(::syn::bracketed!($content in $input))
        } else if $input.peek(::syn::token::Brace) {
            ::syn::MacroDelimiter::Brace(::syn::braced!($content in $input))
        } else {
            return Err($input.error($crate::ParseError::ExpectDelimiter));
        }
    }
}

impl<K: Parse, C, P: ParseFn<C>> Parse for Macro<K, C, P> {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        Ok(Macro {
            kw: input.parse()?,
            tk_bang: input.parse()?,
            delim: macro_delimiter!(content in input),
            content: P::parse(&content)?,
            parse: P::default(),
        })
    }
}

impl Parse for Meta {
    fn parse(input: ParseStream) -> Result<Self> {
        let meta: Macro<_, _, _> = input.parse()?;
        let tk_semi = match meta.delim {
            syn::MacroDelimiter::Paren(_) | syn::MacroDelimiter::Bracket(_) => Some(input.parse()?),
            syn::MacroDelimiter::Brace(_) => input.parse()?,
        };
        Ok(Meta { meta, tk_semi })
    }
}

impl Parse for Mir {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut metas = Vec::new();
        while input.peek(kw::meta) {
            metas.push(input.parse()?);
        }
        let mut declarations = Vec::new();
        while Declaration::can_start(input) {
            declarations.push(input.parse()?);
        }
        let mut statements = Vec::new();
        while !input.is_empty() {
            statements.push(input.parse()?);
        }
        Ok(Mir {
            metas,
            declarations,
            statements,
        })
    }
}

#[derive(Default, Clone, Copy)]
pub struct ParseParse;

#[derive(Default, Clone, Copy)]
pub struct PunctuatedParseTerminated;

pub trait ParseFn<T>: Default {
    fn parse(input: ParseStream<'_>) -> Result<T>;
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
