//! Unresolved-type checking on `Expr` trees used to live here; the compiler now runs the same
//! rules on lowered IR in [`crate::type_checker::checker`]. Integration tests below still assert
//! end-to-end diagnostics.

#[cfg(test)]
mod unresolved_types_tests {
    use crate::{Expr, RibCompiler};
    use test_r::test;

    fn strip_spaces(input: &str) -> String {
        let lines = input.lines();

        let first_line = lines
            .clone()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("");
        let margin_width = first_line.chars().take_while(|c| c.is_whitespace()).count();

        let result = lines
            .map(|line| {
                if line.trim().is_empty() {
                    String::new()
                } else {
                    line[margin_width..].to_string()
                }
            })
            .collect::<Vec<String>>()
            .join("\n");

        result.strip_prefix("\n").unwrap_or(&result).to_string()
    }

    #[test]
    fn test_unresolved_types_identifier() {
        let expr = Expr::from_text("hello").unwrap();
        let compiler = RibCompiler::default();
        let error_msg = compiler.compile(expr).unwrap_err().to_string();

        let error = r#"
        error in the following rib found at line 1, column 1
        `hello`
        cause: cannot determine the type
        help: try specifying the expected type explicitly
        help: if the issue persists, please review the script for potential type inconsistencies
        help: make sure `hello` is a valid identifier
        "#;

        assert_eq!(error_msg, strip_spaces(error));
    }

    #[test]
    fn test_unresolved_type_nested_record_index() {
        let expr = Expr::from_text("{foo: {a: \"bar\", b: (\"foo\", hello)}}").unwrap();
        let compiler = RibCompiler::default();
        let error_msg = compiler.compile(expr).unwrap_err().to_string();

        let expected = r#"
        error in the following rib found at line 1, column 29
        `hello`
        cause: cannot determine the type
        unresolved type at path: `foo.b[1]`
        help: try specifying the expected type explicitly
        help: if the issue persists, please review the script for potential type inconsistencies
        help: make sure `hello` is a valid identifier
        "#;

        assert_eq!(error_msg, strip_spaces(expected));
    }

    #[test]
    fn test_unresolved_type_result_ok() {
        let expr = Expr::from_text("ok(hello)").unwrap();
        let compiler = RibCompiler::default();
        let error_msg = compiler.compile(expr).unwrap_err().to_string();

        let expected = r#"
        error in the following rib found at line 1, column 4
        `hello`
        cause: cannot determine the type
        help: try specifying the expected type explicitly
        help: if the issue persists, please review the script for potential type inconsistencies
        help: make sure `hello` is a valid identifier
        "#;

        assert_eq!(error_msg, strip_spaces(expected));
    }

    #[test]
    fn test_unresolved_type_result_err() {
        let expr = Expr::from_text("err(hello)").unwrap();

        let compiler = RibCompiler::default();
        let error_msg = compiler.compile(expr).unwrap_err().to_string();

        let expected = r#"
        error in the following rib found at line 1, column 5
        `hello`
        cause: cannot determine the type
        help: try specifying the expected type explicitly
        help: if the issue persists, please review the script for potential type inconsistencies
        help: make sure `hello` is a valid identifier
        "#;

        assert_eq!(error_msg, strip_spaces(expected));
    }
}
