pub mod error;
pub mod parser;
pub mod position;
pub mod span;

use pest::Stack;
use pest_typed::tracker::Tracker;
use std::path::Path;

pub use error::ParseError;
pub use parser::{generics, pairs, rules, Grammar, Rule};

pub use position::PositionWrapper;
pub use span::SpanWrapper;

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
