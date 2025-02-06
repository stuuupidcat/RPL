//! Error type from RPL meta pass.

use error_enum::error_type;
use parser::{ParseError, SpanWrapper};
use pest_typed::Span;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;

// TODO: 排版
error_type!(
    #[derive(Clone, Debug)]
    pub RPLMetaError<'i>
        #[color = "red"]
        #[bold]
        Error "Error" {
            000 ParseError {
                error: ParseError,
            }
                "Parse error.\n {error}",
            100 FileError {
                /// Referencing file.
                path: PathBuf,
                /// Cause.
                error: Arc<std::io::Error>,
            }
                "Cannot locate RPL pattern file `{path:?}`. Caused by:\n{error}",
            200 ImportError {
                /// Referencing position.
                span: Span<'i>,
                /// Referencing file.
                path: PathBuf,
                /// Cause.
                error: Arc<std::io::Error>,
            }
                "Cannot locate RPL pattern file `{path:?}` at {span}. Caused by:\n{error}",
            301 SymbolAlreadyDeclared {
                ident: &'i str,
                span: SpanWrapper<'i>,
            }
                "Symbol `{ident}` is already declared. \n{span}",
            302 SymbolNotDeclared {
                ident: &'i str,
                span: SpanWrapper<'i>,
            }
                "Symbol `{ident}` is not declared. \n{span}",
            303 TypeMetaVariableAlreadyDeclared {
                meta_var: &'i str,
                span: SpanWrapper<'i>,
            }
                "Type meta variable `{meta_var}` is already declared. \n{span}",
            304 TypeMetaVariableNotDeclared {
                meta_var: &'i str,
                span: SpanWrapper<'i>,
            }
                "Type variable `{meta_var}` is not declared. \n{span}",
            305 ExportAlreadyDeclared {
                _span: Span<'i>,
            }
                "Export is already declared.",
            306 TypeOrPathAlreadyDeclared {
                type_or_path: &'i str,
                span: SpanWrapper<'i>,
            }
                "Type or path `{type_or_path}` is already declared. \n{span}",
            307 TypeOrPathNotDeclared {
                type_or_path: &'i str,
                span: SpanWrapper<'i>,
            }
                "Type or path `{type_or_path}` is not declared. \n{span}",
            308 MethodAlreadyDeclared {
                _span: Span<'i>,
            }
                "Method is already declared.",
            309 MethodNotDeclared {
            }
                "Method is not declared.",
            310 SelfNotDeclared {
                span: SpanWrapper<'i>,
            }
                "`self` is not declared. \n{span}",
            311 SelfAlreadyDeclared {
                span: SpanWrapper<'i>,
            }
                "`self` is already declared. \n{span}",
            312 SelfValueOutsideImpl {
            }
                "Using `self` value outside of an `impl` item.",
            313 SelfTypeOutsideImpl {
                span: SpanWrapper<'i>,
            }
                "Using `Self` type outside of an `impl` item. \n{span}",
            314 ConstantIndexOutOfBound {
                index: SpanWrapper<'i>,
                min_length: SpanWrapper<'i>,
            }
                "Constant index out of bound for minimum length. \n Index: {index} \n Minimum length: {min_length}",
            315 MultipleOtherwiseInSwitchInt {
                span: SpanWrapper<'i>,
            }
                "Multiple otherwise (`_`) branches in switchInt statement. \n{span}",
            316 MissingSuffixInSwitchInt {
                span: SpanWrapper<'i>,
            }
                "Missing integer suffix in switchInt statement. \n{span}",
            317 UnknownLangItem {
                value: &'i str,
                span: SpanWrapper<'i>,
            }
                "Unknown lang item `{value}`. \n{span}",
            318 RetNotDeclared {
                span: SpanWrapper<'i>,
            }
                "The return value `RET` in MIR pattern is not declared. \n{span}",
        }
);

impl<'a> From<ParseError> for RPLMetaError<'a> {
    fn from(value: ParseError) -> Self {
        Self::ParseError { error: value }
    }
}
impl<'a> RPLMetaError<'a> {
    /// Wrap [`std::io::Error`] as canonicalizating failure.
    pub fn file_error(error: std::io::Error, span: Option<Span<'a>>, path: PathBuf) -> Self {
        let error = Arc::new(error);
        if let Some(span) = span {
            Self::ImportError { path, error, span }
        } else {
            Self::FileError { path, error }
        }
    }
}

impl<'a> std::error::Error for RPLMetaError<'a> {}

pub(crate) type RPLMetaResult<'a, T> = Result<T, RPLMetaError<'a>>;
