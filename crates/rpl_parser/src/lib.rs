use colored::Colorize;
use pest_typed::tracker::Tracker;
use pest_typed::{Position, Stack};
use serde::ser::SerializeTuple;
use serde::Serialize;
use std::cmp::Ordering;
use std::fmt::{Debug, Display};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

mod parser {
    use pest_typed_derive::TypedParser;
    /// Underlying definition of the parser written with Pest.
    #[derive(TypedParser)]
    #[grammar = "grammar/RPL.pest"]
    #[emit_rule_reference]
    pub struct Grammar;
}

pub use parser::{generics, pairs, rules, Grammar, Rule};

/// Human-readable and serializable wrapper of [Span](pest_typed::Span).
///
/// TODO: use [Path](std::path::Path) instead of [PathBuf].
#[derive(Clone, Copy)]
pub struct SpanWrapper<'i> {
    inner: pest_typed::Span<'i>,
    path: &'i Path,
}
impl<'i> SpanWrapper<'i> {
    fn new(value: pest_typed::Span<'i>, path: &'i Path) -> Self {
        Self { inner: value, path }
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
        // TODO: Insert file name.
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

impl Ord for SpanWrapper<'_> {
    fn cmp(&self, _other: &Self) -> Ordering {
        Ordering::Equal
    }
}
impl PartialOrd for SpanWrapper<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for SpanWrapper<'_> {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}
impl Eq for SpanWrapper<'_> {}
impl Hash for SpanWrapper<'_> {
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

impl<'i> SpanWrapper<'i> {
    /// Get the inner [Span](pest_typed::Span).
    pub fn inner(&self) -> pest_typed::Span<'i> {
        self.inner
    }
    /// Get the path.
    pub fn path(&self) -> &'i Path {
        self.path
    }
}

/// A helper for create a [SpanWrapper].
pub trait SpanWrapperCreator<'i, T: Sized>: Sized {
    /// Create a [SpanWrapper] with a reference to `T`.
    fn wrap_with_ref<'t>(self, t: &'t T) -> SpanWrapper<'i>;
    /// Create a [SpanWrapper] with `T`.
    fn wrap_with_val(self, t: T) -> SpanWrapper<'i> {
        self.wrap_with_ref(&t)
    }
}
impl<'i> SpanWrapperCreator<'i, &'i Path> for pest_typed::Span<'i> {
    fn wrap_with_ref(self, t: &&'i Path) -> SpanWrapper<'i> {
        Self::wrap_with_val(self, t)
    }
    fn wrap_with_val(self, t: &'i Path) -> SpanWrapper<'i> {
        SpanWrapper::new(self, t)
    }
}

/// Colored wrapper of [Position](pest_typed::Position).
#[derive(Clone)]
pub struct PositionWrapper<'i> {
    inner: pest_typed::Position<'i>,
    path: PathBuf,
}
impl<'i> PositionWrapper<'i> {
    fn new(value: pest_typed::Position<'i>, path: PathBuf) -> Self {
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
        // TODO: Insert file name.
        let path = self.path.to_string_lossy();
        let (line, col) = self.inner.line_col();
        writeln!(f, "   {} {}:{}:{}", "-->".blue(), path, line, col)?;
        self.inner.display(f, {
            let mut opt = Default::default();
            if false {
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

fn format_tracker<'i>(
    value: Tracker<'i, Rule>,
    path: &Path,
    replacer: impl FnOnce(Position<'i>) -> Position<'i>,
) -> Result<ParseError, std::fmt::Error> {
    use std::fmt::Write;
    fn format_rule(rule: Option<Rule>, f: &mut impl Write) -> std::fmt::Result {
        match rule {
            None => write!(f, "Root Rule")?,
            Some(rule) => write!(f, "{:?}", rule)?,
        }
        Ok(())
    }
    let (pos, rules) = value.finish();
    let pos = replacer(pos);
    let mut msg = format!("{}", PositionWrapper::new(pos, path.to_owned()));
    let log10 = {
        let mut n = pos.line_col().0;
        let mut i = 1;
        while n >= 10 {
            n /= 10;
            i += 1;
        }
        i
    };
    write!(msg, "\n{} {}", " ".repeat(log10), "= Parse error.".blue())?;
    if cfg!(debug_assertions) {
        write!(msg, "{}", "Possible rules".blue())?;
        let mut iter = rules.keys().cloned();
        if let Some(rule) = iter.next() {
            format_rule(rule, &mut msg)?;
        }
        for rule in iter {
            write!(msg, ",")?;
            format_rule(rule, &mut msg)?;
        }
        write!(msg, ".")?;
    }
    Ok(ParseError { msg })
}

/// Errors from parser.
#[derive(Clone, Debug, Serialize)]
pub struct ParseError {
    msg: String,
}

impl ParseError {
    /// Create a [ParseError].
    pub fn new(tracker: Tracker<'_, Rule>, path: &Path) -> Self {
        format_tracker(tracker, path, |pos| pos).unwrap()
    }
    /// Create a [ParseError].
    pub fn new_with<'i>(
        tracker: Tracker<'i, Rule>,
        path: &Path,
        replacer: impl FnOnce(Position<'i>) -> Position<'i>,
    ) -> Self {
        format_tracker(tracker, path, replacer).unwrap()
    }
}

impl Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.msg)?;
        Ok(())
    }
}
impl std::error::Error for ParseError {}

pub fn parse<'i, T: pest_typed::ParsableTypedNode<'i, Rule>>(
    input: impl pest_typed::AsInput<'i>,
    path: &Path,
) -> Result<T, ParseError> {
    let input = input.as_input();
    let mut stack = Stack::new();
    let mut tracker = Tracker::new(input);
    match T::try_parse_with(input, &mut stack, &mut tracker) {
        Some(res) => Ok(res),
        None => Err(ParseError::new(tracker, path)),
    }
}

/// Parse input to [main](pairs::main).
pub fn parse_main<'i>(input: &'i str, path: &Path) -> Result<pairs::main<'i>, ParseError> {
    parse(input, path)
}
