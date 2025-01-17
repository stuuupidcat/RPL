use crate::parser::Rule;
use crate::position::PositionWrapper;
use colored::Colorize;
use pest_typed::tracker::Tracker;
use serde::Serialize;
use std::fmt::Display;
use std::path::Path;

fn format_tracker<'i>(
    value: Tracker<'i, Rule>,
    path: &Path,
    replacer: impl FnOnce(pest_typed::Position<'i>) -> pest_typed::Position<'i>,
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
        replacer: impl FnOnce(pest_typed::Position<'i>) -> pest_typed::Position<'i>,
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
