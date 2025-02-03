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
    pub RPLMetaError<'a>
        #[color = "red"]
        #[bold]
        Error "Error" {
            000 ParseError {
                /// Wrapped error.
                error: ParseError,
            }
                "Parse error.\n {error}",
            100 CanonicalizationError {
                /// Referencing file.
                path: PathBuf,
                /// Cause.
                error: Arc<std::io::Error>,
            }
                "Cannot locate RPL pattern file `{path:?}`. Caused by:\n{error}",
            200 ImportError {
                /// Referencing position.
                span: Span<'a>,
                /// Referencing file.
                path: PathBuf,
                /// Cause.
                error: Arc<std::io::Error>,
            }
                "Cannot locate RPL pattern file `{path:?}` at {span}. Caused by:\n{error}",
            301 SymbolAlreadyDeclared {
                span: Span<'a>,
            }
                "Symbol is already declared.",
            302 SymbolNotDeclared {
                span: Span<'a>,
            }
                "Symbol is not declared.",
            303 TypeVarAlreadyDeclared {
                span: Span<'a>,
            }
                "Type variable is already declared.",
            304 TypeVarNotDeclared {
                span: Span<'a>,
            }
                "Type variable is not declared.",
            305 ExportAlreadyDeclared {
                span: Span<'a>,
            }
                "Export is already declared.",
            306 TypeOrPathAlreadyDeclared {
                span: Span<'a>,
            }
                "Type or path is already declared.",
            307 TypeOrPathNotDeclared {
                span: Span<'a>,
            }
                "Type or path is not declared.",
            308 FnIdentMissingDollar {
                span: Span<'a>,
            }
                "Missing `$` in before function identifier.",
            309 MethodAlreadyDeclared {
                span: Span<'a>,
            }
                "Method is already declared.",
            310 MethodNotDeclared {
            }
                "Method is not declared.",
            311 SelfNotDeclared {
            }
                "`self` is not declared.",
            312 SelfAlreadyDeclared {
                span: Span<'a>,
            }
                "`self` is already declared.",
            313 SelfValueOutsideImpl {
            }
                "Using `self` value outside of an `impl` item.",
            314 SelfTypeOutsideImpl {
            }
                "Using `Self` type outside of an `impl` item.",
            315 ConstantIndexOutOfBound {
            }
                "Constant index out of bound for minimum length.",
            316 MultipleOtherwiseInSwitchInt {
            }
                "Multiple otherwise (`_`) branches in switchInt statement.",
            317 MissingSuffixInSwitchInt {
            }
                "Missing integer suffix in switchInt statement.",
            318 UnknownLangItem {
            }
                "Unknown language item.",
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
            Self::CanonicalizationError { path, error }
        }
    }
}

impl<'a> std::error::Error for RPLMetaError<'a> {}

pub(crate) type RPLMetaResult<'a, T> = Result<T, RPLMetaError<'a>>;
