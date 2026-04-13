use crate::expr_arena::{ExprArena, ExprId, ExprKind, TypeTable};
use crate::type_checker::Path;
use crate::type_checker::PathElem;
use crate::type_inference::expr_visitor::arena::children_of;
use crate::InferredType;
use crate::VariableId;

#[derive(Clone, Debug)]
pub struct GlobalVariableTypeSpec {
    variable_id: VariableId,
    path: Path,
    inferred_type: InferredType,
}

impl GlobalVariableTypeSpec {
    pub fn variable(&self) -> String {
        self.variable_id.name()
    }

    // Constructs a new `GlobalVariableTypeSpec`, which associates a specific inferred type
    // with a global variable and its nested path.
    //
    // A path denotes access to nested fields within a variable, where each field
    // may be typed explicitly. For example:
    //   - A specification like `a.*` implies that all fields under `a` are of type `Str`.
    //   - Similarly, `a.b.*` indicates that all fields under `a.b` are of type `Str`.
    //
    // Paths are expected to reference at least one nested field.
    //
    // The type system enforces consistency across declared paths. If contradictory default
    // types are specified for the same or overlapping paths, a compilation error will occur.
    //
    // Parameters:
    // - `variable_name`: The name of the root global variable (e.g., `"a"` in `a.b.c`).
    // - `path`: A `Path` representing the sequence of nested fields from the root variable
    //            (e.g., `[b, c]` for `a.b.c`).
    // - `inferred_type`: The enforced type (e.g., `Str`, `U64`) for the value
    //                    located at the specified path.
    // Note that the inferred_type is applied only to the element that exists after the end of the `path`.
    // For example, if the path is `a.b` and the inferred type is `Str`, then the type of `a.b.c` will be `Str`
    // and not for `a.b`
    pub fn new(
        variable_name: &str,
        path: Path,
        inferred_type: InferredType,
    ) -> GlobalVariableTypeSpec {
        GlobalVariableTypeSpec {
            variable_id: VariableId::global(variable_name.to_string()),
            path,
            inferred_type,
        }
    }
}

pub fn bind_global_variable_types_lowered(
    root: ExprId,
    arena: &ExprArena,
    types: &mut TypeTable,
    type_specs: &[GlobalVariableTypeSpec],
) {
    for spec in type_specs {
        override_type_arena(root, arena, types, spec);
    }
}

fn override_type_arena(
    root: ExprId,
    arena: &ExprArena,
    types: &mut TypeTable,
    spec: &GlobalVariableTypeSpec,
) {
    let full_path = {
        let mut p = spec.path.clone();
        p.push_front(PathElem::Field(spec.variable_id.to_string()));
        p
    };

    let mut order = Vec::new();
    collect_post_order_global_var(root, arena, &mut order);

    let mut current_path = full_path.clone();
    let mut previous_id: Option<ExprId> = None;

    for id in order {
        let node = arena.expr(id);
        match &node.kind {
            ExprKind::Identifier { variable_id } => {
                if variable_id == &spec.variable_id {
                    current_path.progress();
                    if spec.path.is_empty() {
                        types.set(id, spec.inferred_type.clone());
                        previous_id = None;
                        current_path = full_path.clone();
                    } else {
                        previous_id = Some(id);
                    }
                } else {
                    previous_id = None;
                    current_path = full_path.clone();
                }
            }
            ExprKind::SelectField {
                expr: inner_id,
                field,
            } => {
                if let Some(prev_id) = previous_id {
                    if *inner_id == prev_id {
                        if current_path.is_empty() {
                            types.set(id, spec.inferred_type.clone());
                            previous_id = None;
                            current_path = full_path.clone();
                        } else if current_path.current()
                            == Some(&PathElem::Field(field.to_string()))
                        {
                            current_path.progress();
                            previous_id = Some(id);
                        } else {
                            previous_id = None;
                            current_path = full_path.clone();
                        }
                    } else {
                        previous_id = None;
                        current_path = full_path.clone();
                    }
                }
            }
            _ => {
                previous_id = None;
                current_path = full_path.clone();
            }
        }
    }
}

fn collect_post_order_global_var(root: ExprId, arena: &ExprArena, out: &mut Vec<ExprId>) {
    let mut stack = vec![(root, false)];
    while let Some((id, visited)) = stack.pop() {
        if visited {
            out.push(id);
        } else {
            stack.push((id, true));
            for child in children_of(id, arena).into_iter().rev() {
                stack.push((child, false));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rib_source_span::SourceSpan;
    use crate::{ComponentDependency, Expr, Id, RibCompiler, RibCompilerConfig, TypeName};
    use test_r::test;

    #[test]
    fn test_override_types_5() {
        let expr = Expr::from_text(
            r#"
             let res = foo.bar.user-id;
             let hello: u64 = foo.bar.number;
             hello
        "#,
        )
        .unwrap();

        let type_spec = GlobalVariableTypeSpec {
            variable_id: VariableId::global("foo".to_string()),
            path: Path::from_elems(vec!["bar"]),
            inferred_type: InferredType::string(),
        };

        let rib_compiler = RibCompiler::new(RibCompilerConfig::new(
            ComponentDependency::default(),
            vec![type_spec],
            vec![],
        ));

        let inferred_expr = rib_compiler.infer_types(expr).unwrap();

        let expected = Expr::expr_block(vec![
            Expr::Let {
                variable_id: VariableId::Local("res".to_string(), Some(Id(0))),
                type_annotation: None,
                expr: Box::new(
                    Expr::select_field(
                        Expr::select_field(
                            Expr::identifier_global("foo", None).with_inferred_type(
                                InferredType::record(vec![(
                                    "bar".to_string(),
                                    InferredType::record(vec![
                                        ("number".to_string(), InferredType::u64()),
                                        ("user-id".to_string(), InferredType::string()),
                                    ]),
                                )]),
                            ),
                            "bar",
                            None,
                        )
                        .with_inferred_type(InferredType::record(vec![
                            ("number".to_string(), InferredType::u64()),
                            ("user-id".to_string(), InferredType::string()),
                        ])),
                        "user-id",
                        None,
                    )
                    .with_inferred_type(InferredType::string()),
                ),
                inferred_type: InferredType::tuple(vec![]),
                source_span: SourceSpan::default(),
            },
            Expr::Let {
                variable_id: VariableId::Local("hello".to_string(), Some(Id(0))),
                type_annotation: Some(TypeName::U64),
                expr: Box::new(
                    Expr::select_field(
                        Expr::select_field(
                            Expr::identifier_global("foo", None).with_inferred_type(
                                InferredType::record(vec![(
                                    "bar".to_string(),
                                    InferredType::record(vec![
                                        ("number".to_string(), InferredType::u64()),
                                        ("user-id".to_string(), InferredType::string()),
                                    ]),
                                )]),
                            ),
                            "bar",
                            None,
                        )
                        .with_inferred_type(InferredType::record(vec![
                            ("number".to_string(), InferredType::u64()),
                            ("user-id".to_string(), InferredType::string()),
                        ])),
                        "number",
                        None,
                    )
                    .with_inferred_type(InferredType::u64()),
                ),
                inferred_type: InferredType::tuple(vec![]),
                source_span: SourceSpan::default(),
            },
            Expr::Identifier {
                variable_id: VariableId::Local("hello".to_string(), Some(Id(0))),
                type_annotation: None,
                inferred_type: InferredType::u64(),
                source_span: SourceSpan::default(),
            },
        ])
        .with_inferred_type(InferredType::u64());

        assert_eq!(inferred_expr.get_expr(), &expected);
    }
}
