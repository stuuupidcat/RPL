//! Error type from RPL meta pass.

use error_enum::error_type;
use parser::{ParseError, SpanWrapper as Span};
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;

// TODO: 排版
error_type!(
    #[derive(Clone, Debug)]
    pub RPLMetaError<'i>
        #[color = "red"]
        #[bold]
        Error "错误。" {
            0 ParseError {
                /// Wrapped error.
                error: ParseError,
            }
                "{error}",
            1 CanonicalizationError {
                /// Referencing file.
                path: PathBuf,
                /// Cause.
                error: Arc<std::io::Error>,
            }
                "Cannot locate RPL pattern file `{path:?}`. Caused by:\n{error}",
            2 ImportError {
                /// Referencing position.
                span: Span<'i>,
                /// Referencing file.
                path: PathBuf,
                /// Cause.
                error: Arc<std::io::Error>,
            }
                "Cannot locate RPL pattern file `{path:?}` at {span}. Caused by:\n{error}",
            301 SymbolAlreadyDeclared {
            }
                "Symbol is already declared.",
            302 SymbolNotDeclared {
            }
                "Symbol is not declared.",
            303 TypeVarAlreadyDeclared {
            }
                "Type variable is already declared.",
            304 TypeVarNotDeclared {
            }
                "Type variable is not declared.",
            305 ExportAlreadyDeclared {
            }
                "Export is already declared.",
            306 TypeOrPathAlreadyDeclared {
            }
                "Type or path is already declared.",
            307 TypeOrPathNotDeclared {
            }
                "Type or path is not declared.",
            308 FnIdentMissingDollar {
            }
                "Missing `$` in before function identifier.",
            309 MethodAlreadyDeclared {
            }
                "Method is already declared.",
            310 MethodNotDeclared {
            }
                "Method is not declared.",
            311 SelfNotDeclared {
            }
                "`self` is not declared.",
            312 SelfAlreadyDeclared {
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

impl<'i> From<ParseError> for RPLMetaError<'i> {
    fn from(value: ParseError) -> Self {
        Self::ParseError { error: value }
    }
}
impl<'i> RPLMetaError<'i> {
    /// Wrap [`std::io::Error`] as canonicalizating failure.
    pub fn file_error(error: std::io::Error, span: Option<Span<'i>>, path: PathBuf) -> Self {
        let error = Arc::new(error);
        if let Some(span) = span {
            Self::ImportError { path, error, span }
        } else {
            Self::CanonicalizationError { path, error }
        }
    }
}

impl<'i> std::error::Error for RPLMetaError<'i> {}
