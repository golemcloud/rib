use combine::parser::char::{spaces, string};
use combine::{ParseError, Parser};

use crate::expr::Expr;
use crate::parser::errors::RibParseError;
use crate::parser::rib_expr::rib_expr;
use crate::rib_source_span::GetSourcePosition;

pub fn not<Input>() -> impl Parser<Input, Output = Expr>
where
    Input: combine::Stream<Token = char>,
    RibParseError: Into<
        <Input::Error as ParseError<Input::Token, Input::Range, Input::Position>>::StreamError,
    >,
    Input::Position: GetSourcePosition,
{
    spaces().with((string("!").skip(spaces()), rib_expr()).map(|(_, expr)| Expr::not(expr)))
}

#[cfg(test)]
mod tests {
    use test_r::test;

    use super::*;

    #[test]
    fn test_not_identifier() {
        let input = "!foo";
        let result = Expr::from_text(input);
        assert_eq!(result, Ok(Expr::not(Expr::identifier_global("foo", None))));
    }

    #[test]
    fn test_not_sequence() {
        let input = "![foo, bar]";
        let result = Expr::from_text(input);
        assert_eq!(
            result,
            Ok(Expr::not(Expr::sequence(
                vec![
                    Expr::identifier_global("foo", None),
                    Expr::identifier_global("bar", None)
                ],
                None
            )))
        );
    }

    #[test]
    fn test_not_not() {
        let input = "! !foo";
        let result = Expr::from_text(input);
        assert_eq!(
            result,
            Ok(Expr::not(Expr::not(Expr::identifier_global("foo", None))))
        );
    }
}
