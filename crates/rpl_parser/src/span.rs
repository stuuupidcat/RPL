use colored::Colorize;
use serde::ser::SerializeTuple;
use serde::Serialize;
use std::fmt::{Debug, Display};
use std::path::{Path, PathBuf};

/// Human-readable and serializable wrapper of [Span](pest_typed::Span).
#[derive(Clone)]
pub struct SpanWrapper<'i> {
    inner: pest_typed::Span<'i>,
    path: PathBuf,
}

impl<'i> SpanWrapper<'i> {
    /// Create a new span
    fn new(value: pest_typed::Span<'i>, path: PathBuf) -> Self {
        Self { inner: value, path }
    }

    /// Get the inner [Span](pest_typed::Span).
    pub fn inner(&self) -> pest_typed::Span<'i> {
        self.inner
    }
    /// Get the path.
    pub fn path(&self) -> &Path {
        self.path.as_path()
    }
}

impl<'i> Debug for SpanWrapper<'i> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.inner, f)
    }
}

impl<'i> Serialize for SpanWrapper<'i> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let path = self.path.to_string_lossy();
        let start = self.inner.start_pos().line_col();
        let end = self.inner.end_pos().line_col();
        let mut s = serializer.serialize_tuple(5)?;
        s.serialize_element(path.as_ref())?;
        s.serialize_element(&start.0)?;
        s.serialize_element(&start.1)?;
        s.serialize_element(&end.0)?;
        s.serialize_element(&end.1)?;
        s.end()
    }
}

impl<'i> Display for SpanWrapper<'i> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let path = self.path.to_string_lossy();
        let start = self.inner.start_pos().line_col();
        let end = self.inner.end_pos().line_col();
        writeln!(
            f,
            "   {} \"{}\":{}.{}-{}.{}",
            "-->".blue(),
            path,
            start.0,
            start.1,
            end.0,
            end.1
        )?;
        self.inner.display(f, {
            let mut opt = Default::default();
            if false {
                // for opt's type inference
                opt
            } else {
                opt.marker_formatter = |s, f| write!(f, "{}", s.yellow());
                opt.number_formatter = |s, f| write!(f, "{}", s.blue());
                opt.span_formatter = |s, f| write!(f, "{}", s.red());
                opt
            }
        })
    }
}
