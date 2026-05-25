//! Error types for rpldoc.

use std::io;
use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum RpldocError {
    #[error("failed to read {path}: {source}", path = path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    /// Parser produced an error. We carry the formatted message rather than the
    /// raw `rpl_parser::ParseError<'_>` so that this enum is `'static`.
    #[error("parse error in {path}:\n{message}", path = path.display())]
    Parse { path: PathBuf, message: String },

    #[error("{path}: file has no `pattern <Name>` header", path = path.display())]
    MissingPatternHeader { path: PathBuf },

    #[error("{path}: file is not valid UTF-8", path = path.display())]
    NotUtf8 { path: PathBuf },

    #[error("failed to write {path}: {source}", path = path.display())]
    OutputWrite {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}
