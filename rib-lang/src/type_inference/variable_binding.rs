use crate::{MatchIdentifier, VariableId};
use std::collections::HashMap;

use crate::expr_arena::{
    ArmPatternId, ArmPatternNode, ExprArena, ExprId, ExprKind, MatchArmNode, TypeTable,
};
use crate::type_inference::expr_visitor::arena::children_of;

// -----------------------------------------------------------------------
// bind_variables_of_let_assignment
// -----------------------------------------------------------------------

/// Arena version: assigns local `VariableId`s to `Let` nodes and propagates
/// them to matching `Identifier` use-sites.
pub fn bind_variables_of_let_assignment(root: ExprId, arena: &mut ExprArena, _types: &TypeTable) {
    let mut state: HashMap<String, VariableId> = HashMap::new();

    // Post-order: children before parents — so identifiers inside a let's
    // rhs are processed before the let itself.
    let mut order = Vec::new();
    collect_post_order(root, arena, &mut order);

    for id in order {
        let kind = arena.expr(id).kind.clone();
        match kind {
            ExprKind::Let { variable_id, .. } => {
                let name = variable_id.name();
                let next = state
                    .entry(name.clone())
                    .and_modify(|x| *x = x.increment_local_variable_id())
                    .or_insert_with(|| VariableId::local(&name, 0))
                    .clone();
                if let ExprKind::Let {
                    variable_id: ref mut vid,
                    ..
                } = arena.expr_mut(id).kind
                {
                    *vid = next;
                }
            }
            ExprKind::Identifier { variable_id } if !variable_id.is_match_binding() => {
                let name = variable_id.name();
                if let Some(latest) = state.get(&name).cloned() {
                    if let ExprKind::Identifier {
                        variable_id: ref mut vid,
                    } = arena.expr_mut(id).kind
                    {
                        *vid = latest;
                    }
                }
            }
            _ => {}
        }
    }
}

// -----------------------------------------------------------------------
// bind_variables_of_list_comprehension
// -----------------------------------------------------------------------

pub fn bind_variables_of_list_comprehension(
    root: ExprId,
    arena: &mut ExprArena,
    _types: &TypeTable,
) {
    // Pre-order: process parent before children so the updated variable is
    // used when we patch identifiers inside the yield expression.
    let mut order = Vec::new();
    collect_pre_order(root, arena, &mut order);

    for id in order {
        let kind = arena.expr(id).kind.clone();
        if let ExprKind::ListComprehension {
            mut iterated_variable,
            yield_expr,
            ..
        } = kind
        {
            let new_var = VariableId::list_comprehension_identifier(iterated_variable.name());
            iterated_variable = new_var.clone();

            // patch the node
            if let ExprKind::ListComprehension {
                iterated_variable: ref mut v,
                ..
            } = arena.expr_mut(id).kind
            {
                *v = new_var.clone();
            }

            patch_identifier_in_subtree(yield_expr, arena, &iterated_variable);
        }
    }
}

// -----------------------------------------------------------------------
// bind_variables_of_list_reduce
// -----------------------------------------------------------------------

pub fn bind_variables_of_list_reduce(root: ExprId, arena: &mut ExprArena, _types: &TypeTable) {
    let mut order = Vec::new();
    collect_pre_order(root, arena, &mut order);

    for id in order {
        let kind = arena.expr(id).kind.clone();
        if let ExprKind::ListReduce {
            mut reduce_variable,
            mut iterated_variable,
            yield_expr,
            ..
        } = kind
        {
            let new_iter = VariableId::list_comprehension_identifier(iterated_variable.name());
            let new_reduce = VariableId::list_reduce_identifier(reduce_variable.name());
            iterated_variable = new_iter.clone();
            reduce_variable = new_reduce.clone();

            if let ExprKind::ListReduce {
                reduce_variable: ref mut rv,
                iterated_variable: ref mut iv,
                ..
            } = arena.expr_mut(id).kind
            {
                *rv = new_reduce.clone();
                *iv = new_iter.clone();
            }

            patch_two_identifiers_in_subtree(
                yield_expr,
                arena,
                &iterated_variable,
                &reduce_variable,
            );
        }
    }
}

// -----------------------------------------------------------------------
// bind_variables_of_pattern_match
// -----------------------------------------------------------------------

pub fn bind_variables_of_pattern_match(root: ExprId, arena: &mut ExprArena, _types: &TypeTable) {
    bind_pattern_match_internal(root, arena, 0, &mut []);
}

fn bind_pattern_match_internal(
    root: ExprId,
    arena: &mut ExprArena,
    previous_index: usize,
    match_identifiers: &mut [MatchIdentifier],
) -> usize {
    let mut index = previous_index;
    let mut shadowed_let_bindings: Vec<String> = vec![];

    let mut order = Vec::new();
    collect_pre_order(root, arena, &mut order);

    for id in order {
        let kind = arena.expr(id).kind.clone();
        match kind {
            ExprKind::PatternMatch { match_arms, .. } => {
                for arm in match_arms {
                    index += 1;
                    index = process_arm_arena(arm, index, arena);
                }
            }
            ExprKind::Let { variable_id, .. } => {
                shadowed_let_bindings.push(variable_id.name());
            }
            ExprKind::Identifier { variable_id } => {
                let name = variable_id.name();
                if let Some(mi) = match_identifiers.iter().find(|x| x.name == name) {
                    if !shadowed_let_bindings.contains(&name) {
                        if let ExprKind::Identifier {
                            variable_id: ref mut vid,
                        } = arena.expr_mut(id).kind
                        {
                            *vid = VariableId::MatchIdentifier(mi.clone());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    index
}

fn process_arm_arena(arm: MatchArmNode, global_arm_index: usize, arena: &mut ExprArena) -> usize {
    let mut match_identifiers = vec![];
    collect_identifiers_from_arm_pattern(
        arm.arm_pattern,
        global_arm_index,
        arena,
        &mut match_identifiers,
    );
    bind_pattern_match_internal(
        arm.arm_resolution_expr,
        arena,
        global_arm_index,
        &mut match_identifiers,
    )
}

fn collect_identifiers_from_arm_pattern(
    pat_id: ArmPatternId,
    global_arm_index: usize,
    arena: &mut ExprArena,
    out: &mut Vec<MatchIdentifier>,
) {
    let pat = arena.pattern(pat_id).clone();
    match pat {
        ArmPatternNode::Literal(expr_id) => {
            update_identifiers_in_pattern_expr(expr_id, global_arm_index, arena, out);
        }
        ArmPatternNode::WildCard => {}
        ArmPatternNode::As(name, inner) => {
            out.push(MatchIdentifier::new(name, global_arm_index));
            collect_identifiers_from_arm_pattern(inner, global_arm_index, arena, out);
        }
        ArmPatternNode::Constructor(_, children)
        | ArmPatternNode::TupleConstructor(children)
        | ArmPatternNode::ListConstructor(children) => {
            for child in children {
                collect_identifiers_from_arm_pattern(child, global_arm_index, arena, out);
            }
        }
        ArmPatternNode::RecordConstructor(fields) => {
            for (_, child) in fields {
                collect_identifiers_from_arm_pattern(child, global_arm_index, arena, out);
            }
        }
    }
}

fn update_identifiers_in_pattern_expr(
    expr_id: ExprId,
    global_arm_index: usize,
    arena: &mut ExprArena,
    out: &mut Vec<MatchIdentifier>,
) {
    let mut order = Vec::new();
    collect_post_order(expr_id, arena, &mut order);
    for id in order {
        let kind = arena.expr(id).kind.clone();
        if let ExprKind::Identifier { variable_id } = kind {
            let mi = MatchIdentifier::new(variable_id.name(), global_arm_index);
            out.push(mi.clone());
            if let ExprKind::Identifier {
                variable_id: ref mut vid,
            } = arena.expr_mut(id).kind
            {
                *vid = VariableId::match_identifier(variable_id.name(), global_arm_index);
            }
        }
    }
}

// -----------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------

fn patch_identifier_in_subtree(root: ExprId, arena: &mut ExprArena, target: &VariableId) {
    let mut order = Vec::new();
    collect_pre_order(root, arena, &mut order);
    for id in order {
        let kind = arena.expr(id).kind.clone();
        if let ExprKind::Identifier { variable_id } = kind {
            if variable_id.name() == target.name() {
                if let ExprKind::Identifier {
                    variable_id: ref mut vid,
                } = arena.expr_mut(id).kind
                {
                    *vid = target.clone();
                }
            }
        }
    }
}

fn patch_two_identifiers_in_subtree(
    root: ExprId,
    arena: &mut ExprArena,
    iter_var: &VariableId,
    reduce_var: &VariableId,
) {
    let mut order = Vec::new();
    collect_pre_order(root, arena, &mut order);
    for id in order {
        let kind = arena.expr(id).kind.clone();
        if let ExprKind::Identifier { variable_id } = kind {
            let name = variable_id.name();
            let new_vid = if name == iter_var.name() {
                Some(iter_var.clone())
            } else if name == reduce_var.name() {
                Some(reduce_var.clone())
            } else {
                None
            };
            if let Some(new_vid) = new_vid {
                if let ExprKind::Identifier {
                    variable_id: ref mut vid,
                } = arena.expr_mut(id).kind
                {
                    *vid = new_vid;
                }
            }
        }
    }
}

fn collect_post_order(root: ExprId, arena: &ExprArena, out: &mut Vec<ExprId>) {
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

fn collect_pre_order(root: ExprId, arena: &ExprArena, out: &mut Vec<ExprId>) {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        out.push(id);
        for child in children_of(id, arena).into_iter().rev() {
            stack.push(child);
        }
    }
}

#[cfg(test)]
mod name_binding_tests {
    use bigdecimal::BigDecimal;
    use test_r::test;

    use crate::call_type::CallType;
    use crate::function_name::{DynamicParsedFunctionName, DynamicParsedFunctionReference};
    use crate::{Expr, InferredType, ParsedFunctionSite, VariableId};

    /// Same pipeline as [`crate::type_inference::initial_arena_phase`]: lower → arena bind → rebuild.
    fn bind_let_assignment_via_arena(expr: &mut Expr) {
        let (mut arena, types, root) = crate::expr_arena::lower(expr);
        super::bind_variables_of_let_assignment(root, &mut arena, &types);
        *expr = crate::expr_arena::rebuild_expr(root, &arena, &types);
    }

    fn bind_pattern_match_via_arena(expr: &mut Expr) {
        let (mut arena, types, root) = crate::expr_arena::lower(expr);
        super::bind_variables_of_pattern_match(root, &mut arena, &types);
        *expr = crate::expr_arena::rebuild_expr(root, &arena, &types);
    }

    #[test]
    fn test_name_binding_simple() {
        let rib_expr = r#"
          let x = 1;
          foo(x)
        "#;

        let mut expr = Expr::from_text(rib_expr).unwrap();

        bind_let_assignment_via_arena(&mut expr);

        let let_binding = Expr::let_binding_with_variable_id(
            VariableId::local("x", 0),
            Expr::number(BigDecimal::from(1)),
            None,
        );

        let call_expr = Expr::call(
            CallType::function_call(
                DynamicParsedFunctionName {
                    site: ParsedFunctionSite::Global,
                    function: DynamicParsedFunctionReference::Function {
                        function: "foo".to_string(),
                    },
                },
                None,
            ),
            vec![Expr::identifier_local("x", 0, None)],
        );

        let expected = Expr::expr_block(vec![let_binding, call_expr]);

        assert_eq!(expr, expected);
    }

    #[test]
    fn test_name_binding_shadowing() {
        let rib_expr = r#"
          let x = 1;
          foo(x);
          let x = 2;
          foo(x)
        "#;

        let mut expr = Expr::from_text(rib_expr).unwrap();

        bind_let_assignment_via_arena(&mut expr);

        let let_binding1 = Expr::let_binding_with_variable_id(
            VariableId::local("x", 0),
            Expr::number(BigDecimal::from(1)),
            None,
        );

        let let_binding2 = Expr::let_binding_with_variable_id(
            VariableId::local("x", 1),
            Expr::number(BigDecimal::from(2)),
            None,
        );

        let call_expr1 = Expr::call(
            CallType::function_call(
                DynamicParsedFunctionName {
                    site: ParsedFunctionSite::Global,
                    function: DynamicParsedFunctionReference::Function {
                        function: "foo".to_string(),
                    },
                },
                None,
            ),
            vec![Expr::identifier_local("x", 0, None)],
        );

        let call_expr2 = Expr::call(
            CallType::function_call(
                DynamicParsedFunctionName {
                    site: ParsedFunctionSite::Global,
                    function: DynamicParsedFunctionReference::Function {
                        function: "foo".to_string(),
                    },
                },
                None,
            ),
            vec![Expr::identifier_local("x", 1, None)],
        );

        let expected = Expr::expr_block(vec![let_binding1, call_expr1, let_binding2, call_expr2]);

        assert_eq!(expr, expected);
    }

    #[test]
    fn test_simple_pattern_match_name_binding() {
        let expr_string = r#"
          match some(x) {
            some(x) => x,
            none => 0
          }
        "#;

        let mut expr = Expr::from_text(expr_string).unwrap();

        bind_pattern_match_via_arena(&mut expr);

        assert_eq!(expr, expectations::expected_match(1));
    }

    #[test]
    fn test_simple_pattern_match_name_binding_block() {
        let expr_string = r#"
          match some(x) {
            some(x) => x,
            none => 0
          };

          match some(x) {
            some(x) => x,
            none => 0
          }
        "#;

        let mut expr = Expr::from_text(expr_string).unwrap();

        bind_pattern_match_via_arena(&mut expr);

        let first_expr = expectations::expected_match(1);
        let second_expr = expectations::expected_match(3);

        let block = Expr::expr_block(vec![first_expr, second_expr])
            .with_inferred_type(InferredType::unknown());

        assert_eq!(expr, block);
    }

    mod expectations {
        use crate::{ArmPattern, Expr, InferredType, MatchArm, MatchIdentifier, VariableId};
        use bigdecimal::BigDecimal;

        pub fn expected_match(index: usize) -> Expr {
            Expr::pattern_match(
                Expr::option(Some(Expr::identifier_global("x", None)))
                    .with_inferred_type(InferredType::option(InferredType::unknown())),
                vec![
                    MatchArm {
                        arm_pattern: ArmPattern::constructor(
                            "some",
                            vec![ArmPattern::literal(Expr::identifier_with_variable_id(
                                VariableId::MatchIdentifier(MatchIdentifier::new(
                                    "x".to_string(),
                                    index,
                                )),
                                None,
                            ))],
                        ),
                        arm_resolution_expr: Box::new(Expr::identifier_with_variable_id(
                            VariableId::MatchIdentifier(MatchIdentifier::new(
                                "x".to_string(),
                                index,
                            )),
                            None,
                        )),
                    },
                    MatchArm {
                        arm_pattern: ArmPattern::constructor("none", vec![]),
                        arm_resolution_expr: Box::new(Expr::number(BigDecimal::from(0))),
                    },
                ],
            )
        }
    }
}
