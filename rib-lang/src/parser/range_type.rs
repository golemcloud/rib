use combine::parser::char::{spaces, string};
use combine::{optional, ParseError, Parser, Stream};

use crate::parser::errors::RibParseError;
use crate::rib_source_span::GetSourcePosition;

// This is range avoiding left recursion
#[derive(Clone, Debug)]
pub enum RangeType {
    Inclusive,
    Exclusive,
}
pub fn range_type<Input>() -> impl Parser<Input, Output = RangeType>
where
    Input: Stream<Token = char>,
    RibParseError: Into<
        <Input::Error as ParseError<Input::Token, Input::Range, Input::Position>>::StreamError,
    >,
    Input::Position: GetSourcePosition,
{
    (string(".."), optional(string("=").skip(spaces()))).map(|(_, d): (_, Option<_>)| match d {
        Some(_) => RangeType::Inclusive,
        None => RangeType::Exclusive,
    })
}
