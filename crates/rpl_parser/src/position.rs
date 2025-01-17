use colored::Colorize;
use std::fmt::{Debug, Display};
use std::path::PathBuf;

/// Colored wrapper of [Position](pest_typed::Position).
#[derive(Clone)]
pub struct PositionWrapper<'i> {
    inner: pest_typed::Position<'i>,
    path: PathBuf,
}

impl<'i> PositionWrapper<'i> {
    pub fn new(value: pest_typed::Position<'i>, path: PathBuf) -> Self {
        Self { inner: value, path }
    }
}

impl<'i> Debug for PositionWrapper<'i> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.inner, f)
    }
}

impl<'i> Display for PositionWrapper<'i> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let path = self.path.to_string_lossy();
        let (line, col) = self.inner.line_col();
        writeln!(f, "   {} {}:{}:{}", "-->".blue(), path, line, col)?;
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
