use crate::expr_arena::{ExprArena, ExprId, TypeTable};

// Given `f` executes inference, find expr where `f(expr) = expr`
// The pass `scan_and_infer` receives `(root, &arena, &mut type_table)` and
// updates `type_table` in place.  Convergence is detected by comparing a
// cheap snapshot of the type table before and after each iteration.
pub fn type_inference_fix_point<F, E>(
    mut scan_and_infer: F,
    root: ExprId,
    arena: &mut ExprArena,
    type_table: &mut TypeTable,
) -> Result<(), E>
where
    F: FnMut(ExprId, &mut ExprArena, &mut TypeTable) -> Result<(), E>,
{
    loop {
        let before = type_table.snapshot();

        scan_and_infer(root, arena, type_table)?;

        if type_table.same_as_snapshot(&before) {
            break;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use bigdecimal::BigDecimal;

    use test_r::test;

    use crate::parser::type_name::TypeName;
    use crate::{ComponentDependency, Expr, InferredType, VariableId};

    #[test]
    fn test_fix_point() {
        let expr = r#"
        let x: u64 = 1;
        let y: u64 = 2;
        if x == x then x else y
        "#;

        let mut expr = Expr::from_text(expr).unwrap();

        expr.infer_types(&ComponentDependency::default(), &[])
            .unwrap();
        let expected = Expr::expr_block(vec![
            Expr::let_binding_with_variable_id(
                VariableId::local("x", 0),
                Expr::number_inferred(BigDecimal::from(1), None, InferredType::u64()),
                Some(TypeName::U64),
            ),
            Expr::let_binding_with_variable_id(
                VariableId::local("y", 0),
                Expr::number_inferred(BigDecimal::from(2), None, InferredType::u64()),
                Some(TypeName::U64),
            ),
            Expr::cond(
                Expr::equal_to(
                    Expr::identifier_local("x", 0, None).with_inferred_type(InferredType::u64()),
                    Expr::identifier_local("x", 0, None).with_inferred_type(InferredType::u64()),
                ),
                Expr::identifier_local("x", 0, None).with_inferred_type(InferredType::u64()),
                Expr::identifier_local("y", 0, None).with_inferred_type(InferredType::u64()),
            )
            .with_inferred_type(InferredType::u64()),
        ])
        .with_inferred_type(InferredType::u64());

        assert_eq!(expr, expected)
    }
}
