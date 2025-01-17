use crate::error::ParseError;
use pest_typed::tracker::Tracker;
use pest_typed::Stack;
use pest_typed_derive::TypedParser;
use std::path::Path;

/// Underlying definition of the parser written with Pest.
#[derive(TypedParser)]
#[grammar = "grammar/RPL.pest"]
#[emit_rule_reference]
pub struct Grammar;

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
