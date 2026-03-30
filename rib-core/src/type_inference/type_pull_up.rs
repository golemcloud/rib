// Copyright 2024-2025 Golem Cloud
//
// Licensed under the Golem Source License v1.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://license.golem.cloud/LICENSE
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::rib_type_error::RibTypeErrorInternal;
use crate::Expr;

use crate::expr_arena::{
    CallTypeNode, ExprArena, ExprId, ExprKind, InstanceIdentifierNode, MatchArmNode, RangeKind,
    ResultExprKind, TypeTable,
};
use crate::type_inference::expr_visitor::arena::children_of;
use crate::type_inference::type_hint::{GetTypeHint, TypeHint};
use crate::type_refinement::precise_types::{ListType, RecordType};
use crate::type_refinement::TypeRefinement;
use crate::{
    ActualType, ComponentDependencies, ExpectedType, FullyQualifiedResourceMethod, FunctionName,
    InferredType, InterfaceName, PackageName, Path, TypeInternal, TypeMismatchError,
};

/// Runs [`type_pull_up_lowered`] on a lowered copy of `expr` and writes back the
/// rebuilt tree. All pull-up logic lives in the arena implementation.
pub fn type_pull_up(
    expr: &mut Expr,
    component_dependencies: &ComponentDependencies,
) -> Result<(), RibTypeErrorInternal> {
    let (mut expr_arena, mut types, root) = crate::expr_arena::lower(expr);
    type_pull_up_lowered(root, &mut expr_arena, &mut types, component_dependencies)?;
    *expr = crate::expr_arena::rebuild_expr(root, &expr_arena, &types);
    Ok(())
}

/// Arena version of `type_pull_up`.
///
/// Post-order traversal: for each node, read the types of its children from
/// `TypeTable` to compute/update the node's own type.
pub fn type_pull_up_lowered(
    root: ExprId,
    arena: &mut ExprArena,
    types: &mut TypeTable,
    component_dependencies: &ComponentDependencies,
) -> Result<(), RibTypeErrorInternal> {
    // Collect post-order first to avoid borrow conflicts.
    let mut order = Vec::new();
    collect_post_order(root, arena, &mut order);

    for id in order {
        let node = arena.expr(id);
        let kind = node.kind.clone();
        let span = node.source_span.clone();

        match kind {
            ExprKind::Tuple { exprs } => {
                let elem_types: Vec<InferredType> =
                    exprs.iter().map(|&e| types.get(e).clone()).collect();
                let new_tuple = InferredType::tuple(elem_types);
                let current = types.get(id).clone();
                types.set(id, current.merge(new_tuple));
            }

            ExprKind::InvokeMethodLazy {
                lhs,
                ref method,
                ref generic_type_parameter,
                ref args,
            } => {
                let method = method.clone();
                let gtp = generic_type_parameter.clone();
                let args_ids: Vec<ExprId> = args.clone();
                let lhs_type = types.get(lhs).clone();
                let new_call = handle_residual_method_invokes_arena(
                    lhs,
                    &lhs_type,
                    &method,
                    &span,
                    &args_ids,
                    component_dependencies,
                    arena,
                )?;
                // Replace this node's ExprKind with the resolved Call
                let node_mut = arena.expr_mut(id);
                node_mut.kind = new_call.0;
                types.set(id, new_call.1);
                let _ = gtp;
            }

            ExprKind::SelectField {
                expr: inner_id,
                ref field,
            } => {
                let field = field.clone();
                let inner_type = types.get(inner_id).clone();
                let inner_span = arena.expr(inner_id).source_span.clone();
                let result = get_inferred_type_of_selected_field_arena(
                    &inner_type,
                    &inner_span,
                    &field,
                    arena,
                    inner_id,
                )?;
                let current = types.get(id).clone();
                types.set(id, current.merge(result));
            }

            ExprKind::SelectIndex {
                expr: inner_id,
                index: index_id,
            } => {
                let inner_type = types.get(inner_id).clone();
                if !inner_type.is_unknown() {
                    let index_type = types.get(index_id).clone();
                    let inner_span = arena.expr(inner_id).source_span.clone();
                    let result = get_inferred_type_of_selection_dynamic_arena(
                        &inner_type,
                        &inner_span,
                        &index_type,
                        arena,
                        inner_id,
                        index_id,
                    )?;
                    let current = types.get(id).clone();
                    types.set(id, current.merge(result));
                }
            }

            ExprKind::Result {
                expr: ResultExprKind::Ok(ok_id),
            } => {
                let ok_type = types.get(ok_id).clone();
                let result_type = InferredType::result(Some(ok_type), None);
                let current = types.get(id).clone();
                types.set(id, current.merge(result_type));
            }

            ExprKind::Result {
                expr: ResultExprKind::Err(err_id),
            } => {
                let err_type = types.get(err_id).clone();
                let result_type = InferredType::result(None, Some(err_type));
                let current = types.get(id).clone();
                types.set(id, current.merge(result_type));
            }

            ExprKind::Option {
                expr: Some(inner_id),
            } => {
                let inner_type = types.get(inner_id).clone();
                let option_type = InferredType::option(inner_type);
                let current = types.get(id).clone();
                types.set(id, current.merge(option_type));
            }

            ExprKind::Cond { lhs, rhs, .. } => {
                let lhs_type = types.get(lhs).clone();
                let rhs_type = types.get(rhs).clone();
                let current = types.get(id).clone();
                types.set(id, current.merge(lhs_type.merge(rhs_type)));
            }

            ExprKind::PatternMatch { ref match_arms, .. } => {
                let arms: Vec<MatchArmNode> = match_arms.clone();
                let arm_types: Vec<InferredType> = arms
                    .iter()
                    .map(|arm| types.get(arm.arm_resolution_expr).clone())
                    .collect();
                let merged = InferredType::all_of(arm_types);
                let current = types.get(id).clone();
                types.set(id, current.merge(merged));
            }

            ExprKind::ExprBlock { ref exprs } => {
                if let Some(&last_id) = exprs.last() {
                    let last_type = types.get(last_id).clone();
                    let current = types.get(id).clone();
                    types.set(id, current.merge(last_type));
                }
            }

            ExprKind::Sequence { ref exprs } => {
                if let Some(&first_id) = exprs.first() {
                    let first_type = types.get(first_id).clone();
                    let list_type = InferredType::list(first_type);
                    let current = types.get(id).clone();
                    types.set(id, current.merge(list_type));
                }
            }

            ExprKind::Record { ref fields } => {
                let field_types: Vec<(String, InferredType)> = fields
                    .iter()
                    .map(|(name, fid)| (name.clone(), types.get(*fid).clone()))
                    .collect();
                let record_type = InferredType::record(field_types);
                let current = types.get(id).clone();
                types.set(id, current.merge(record_type));
            }

            ExprKind::Plus { lhs, rhs }
            | ExprKind::Minus { lhs, rhs }
            | ExprKind::Multiply { lhs, rhs }
            | ExprKind::Divide { lhs, rhs } => {
                let lhs_type = types.get(lhs).clone();
                let rhs_type = types.get(rhs).clone();
                let current = types.get(id).clone();
                types.set(id, current.merge(lhs_type).merge(rhs_type));
            }

            ExprKind::Unwrap { expr: inner_id } | ExprKind::GetTag { expr: inner_id } => {
                let inner_type = types.get(inner_id).clone();
                let current = types.get(id).clone();
                types.set(id, current.merge(inner_type));
            }

            ExprKind::ListComprehension { yield_expr, .. } => {
                let yield_type = types.get(yield_expr).clone();
                let list_type = InferredType::list(yield_type);
                let current = types.get(id).clone();
                types.set(id, current.merge(list_type));
            }

            ExprKind::ListReduce {
                init_value_expr, ..
            } => {
                let init_type = types.get(init_value_expr).clone();
                let current = types.get(id).clone();
                types.set(id, current.merge(init_type));
            }

            ExprKind::Range { range } => {
                let new_type = match range {
                    RangeKind::Range { from, to } | RangeKind::RangeInclusive { from, to } => {
                        let from_type = types.get(from).clone();
                        let to_type = types.get(to).clone();
                        InferredType::range(from_type, Some(to_type))
                    }
                    RangeKind::RangeFrom { from } => {
                        let from_type = types.get(from).clone();
                        InferredType::range(from_type, None)
                    }
                };
                types.set(id, new_type);
            }

            // Leaves and nodes whose type comes purely from children via
            // other passes (identifiers, flags, literals, comparisons, etc.)
            ExprKind::Identifier { .. }
            | ExprKind::Flags { .. }
            | ExprKind::Concat { .. }
            | ExprKind::Not { .. }
            | ExprKind::GreaterThan { .. }
            | ExprKind::GreaterThanOrEqualTo { .. }
            | ExprKind::LessThanOrEqualTo { .. }
            | ExprKind::EqualTo { .. }
            | ExprKind::LessThan { .. }
            | ExprKind::And { .. }
            | ExprKind::Or { .. }
            | ExprKind::Let { .. }
            | ExprKind::Literal { .. }
            | ExprKind::Number { .. }
            | ExprKind::Boolean { .. }
            | ExprKind::Call { .. }
            | ExprKind::Length { .. }
            | ExprKind::Throw { .. }
            | ExprKind::GenerateWorkerName { .. }
            | ExprKind::Option { expr: None } => {}
        }
    }

    Ok(())
}

// -----------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------

/// Resolves an `InvokeMethodLazy` node into its `Call` equivalent.
/// Returns the new `ExprKind` and its `InferredType`.
fn handle_residual_method_invokes_arena(
    lhs_id: ExprId,
    lhs_type: &InferredType,
    method: &str,
    source_span: &crate::rib_source_span::SourceSpan,
    args: &[ExprId],
    component_dependencies: &ComponentDependencies,
    _arena: &ExprArena,
) -> Result<(ExprKind, InferredType), RibTypeErrorInternal> {
    match lhs_type.internal_type() {
        TypeInternal::Resource { name, owner, .. } => {
            let name = name.clone();
            let owner = owner.clone();

            let fqrm = if let Some(owner_str) = owner {
                let parts: Vec<&str> = owner_str.split('/').collect();
                let ns_pkg = parts.first().map(|s| s.to_string());
                let namespace = ns_pkg
                    .as_ref()
                    .and_then(|s| s.split(':').next())
                    .map(|s| s.to_string());
                let pkg = ns_pkg
                    .as_ref()
                    .and_then(|s| s.split(':').nth(1))
                    .map(|s| s.to_string());
                let iface = parts.get(1).map(|s| s.to_string());
                FullyQualifiedResourceMethod {
                    package_name: namespace.map(|ns| PackageName {
                        namespace: ns,
                        package_name: pkg.unwrap(),
                        version: None,
                    }),
                    interface_name: iface.map(|n| InterfaceName {
                        name: n,
                        version: None,
                    }),
                    resource_name: name.clone().unwrap(),
                    method_name: method.to_string(),
                    static_function: false,
                }
            } else {
                FullyQualifiedResourceMethod {
                    package_name: None,
                    interface_name: None,
                    resource_name: name.clone().unwrap(),
                    method_name: method.to_string(),
                    static_function: false,
                }
            };

            let fn_name = FunctionName::ResourceMethod(fqrm.clone());
            let (key, fn_type) = component_dependencies
                .get_function_type(&None, &fn_name)
                .unwrap();

            let inferred = fn_type.return_type.unwrap_or_else(InferredType::unit);

            let dpfn = fqrm.dynamic_parsed_function_name().unwrap();

            // lhs_id is the variable id if the lhs was an Identifier — we
            // store it as the worker_name inside WitResource.
            let ii_node = InstanceIdentifierNode::WitResource {
                variable_id: None, // will be filled by a later pass
                worker_name: None,
                resource_name: name.unwrap().to_string(),
            };

            let call_kind = ExprKind::Call {
                call_type: CallTypeNode::Function {
                    component_info: Some(key),
                    instance_identifier: Some(ii_node),
                    function_name: dpfn,
                },
                generic_type_parameter: None,
                args: args.to_vec(),
            };

            Ok((call_kind, inferred))
        }
        _ => {
            // Reconstruct the lhs display name from the arena for a precise error message
            let lhs_name = {
                use crate::expr_arena::ExprKind;
                match &_arena.expr(lhs_id).kind {
                    ExprKind::Identifier { variable_id } => variable_id.name(),
                    _ => "lhs".to_string(),
                }
            };
            Err(crate::CustomError {
                source_span: source_span.clone(),
                help_message: vec![],
                message: format!(
                    "invalid method invocation `{lhs_name}.{method}`. make sure `{lhs_name}` is defined and is a valid instance type (i.e, resource or worker)"
                ),
            }
            .into())
        }
    }
}

fn get_inferred_type_of_selected_field_arena(
    record_type: &InferredType,
    source_span: &crate::rib_source_span::SourceSpan,
    field: &str,
    _arena: &ExprArena,
    _inner_id: ExprId,
) -> Result<InferredType, RibTypeErrorInternal> {
    let refined = RecordType::refine(record_type).ok_or_else(|| TypeMismatchError {
        source_span: source_span.clone(),
        expected_type: ExpectedType::Hint(TypeHint::Record(None)),
        actual_type: ActualType::Inferred(record_type.clone()),
        field_path: Path::default(),
        additional_error_detail: vec![
            format!("cannot select `{field}` from this expression"),
            format!("if `{field}` is a function, pass arguments"),
        ],
    })?;
    Ok(refined.inner_type_by_name(field))
}

fn get_inferred_type_of_selection_dynamic_arena(
    list_type: &InferredType,
    source_span: &crate::rib_source_span::SourceSpan,
    index_type: &InferredType,
    _arena: &ExprArena,
    _inner_id: ExprId,
    _index_id: ExprId,
) -> Result<InferredType, RibTypeErrorInternal> {
    let refined = ListType::refine(list_type).ok_or_else(|| TypeMismatchError {
        source_span: source_span.clone(),
        expected_type: ExpectedType::Hint(TypeHint::List(None)),
        actual_type: ActualType::Inferred(list_type.clone()),
        field_path: Default::default(),
        additional_error_detail: vec![format!(
            "cannot index into this expression since it is not a list type. Found: {}",
            list_type.get_type_hint()
        )],
    })?;

    let elem_type = refined.inner_type();

    if index_type.contains_only_number() {
        Ok(elem_type)
    } else {
        Ok(InferredType::list(elem_type))
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

#[cfg(test)]
mod type_pull_up_tests {
    use bigdecimal::BigDecimal;

    use test_r::test;

    use crate::call_type::CallType;
    use crate::function_name::DynamicParsedFunctionName;
    use crate::DynamicParsedFunctionReference::Function;
    use crate::ParsedFunctionSite::PackagedInterface;
    use crate::{ArmPattern, ComponentDependencies, Expr, InferredType, MatchArm, VariableId};

    #[test]
    pub fn test_pull_up_identifier() {
        let expr = "foo";
        let mut expr = Expr::from_text(expr).unwrap();
        expr.add_infer_type_mut(InferredType::string());
        expr.pull_types_up(&ComponentDependencies::default())
            .unwrap();
        assert_eq!(expr.inferred_type(), InferredType::string());
    }

    #[test]
    pub fn test_pull_up_for_select_field() {
        let record_identifier =
            Expr::identifier_global("foo", None).merge_inferred_type(InferredType::record(vec![(
                "foo".to_string(),
                InferredType::record(vec![("bar".to_string(), InferredType::u64())]),
            )]));
        let select_expr = Expr::select_field(record_identifier, "foo", None);
        let mut expr = Expr::select_field(select_expr, "bar", None);
        expr.pull_types_up(&ComponentDependencies::default())
            .unwrap();
        assert_eq!(expr.inferred_type(), InferredType::u64());
    }

    #[test]
    pub fn test_pull_up_for_select_index() {
        let identifier = Expr::identifier_global("foo", None)
            .merge_inferred_type(InferredType::list(InferredType::u64()));
        let mut expr = Expr::select_index(identifier.clone(), Expr::number(BigDecimal::from(0)));
        expr.pull_types_up(&ComponentDependencies::default())
            .unwrap();
        let expected = Expr::select_index(identifier, Expr::number(BigDecimal::from(0)))
            .merge_inferred_type(InferredType::u64());
        assert_eq!(expr, expected);
    }

    #[test]
    pub fn test_pull_up_for_sequence() {
        let elems = vec![
            Expr::number_inferred(BigDecimal::from(1), None, InferredType::u64()),
            Expr::number_inferred(BigDecimal::from(2), None, InferredType::u64()),
        ];

        let mut expr =
            Expr::sequence(elems.clone(), None).with_inferred_type(InferredType::unknown());
        expr.pull_types_up(&ComponentDependencies::default())
            .unwrap();

        assert_eq!(
            expr,
            Expr::sequence(elems, None).with_inferred_type(InferredType::list(InferredType::u64()))
        );
    }

    #[test]
    pub fn test_pull_up_for_tuple() {
        let mut expr = Expr::tuple(vec![
            Expr::literal("foo"),
            Expr::number_inferred(BigDecimal::from(1), None, InferredType::u64()),
        ]);

        expr.pull_types_up(&ComponentDependencies::default())
            .unwrap();

        assert_eq!(
            expr.inferred_type(),
            InferredType::tuple(vec![InferredType::string(), InferredType::u64()])
        );
    }

    #[test]
    pub fn test_pull_up_for_record() {
        let elems = vec![
            (
                "foo".to_string(),
                Expr::number_inferred(BigDecimal::from(1), None, InferredType::u64()),
            ),
            (
                "bar".to_string(),
                Expr::number_inferred(BigDecimal::from(2), None, InferredType::u32()),
            ),
        ];
        let mut expr = Expr::record(elems.clone()).with_inferred_type(InferredType::record(vec![
            ("foo".to_string(), InferredType::unknown()),
            ("bar".to_string(), InferredType::unknown()),
        ]));

        expr.pull_types_up(&ComponentDependencies::default())
            .unwrap();

        assert_eq!(
            expr,
            Expr::record(elems).with_inferred_type(InferredType::all_of(vec![
                InferredType::record(vec![
                    ("foo".to_string(), InferredType::u64()),
                    ("bar".to_string(), InferredType::u32())
                ]),
                InferredType::record(vec![
                    ("foo".to_string(), InferredType::unknown()),
                    ("bar".to_string(), InferredType::unknown())
                ])
            ]))
        );
    }

    #[test]
    pub fn test_pull_up_for_concat() {
        let mut expr = Expr::concat(vec![Expr::literal("foo"), Expr::literal("bar")]);
        expr.pull_types_up(&ComponentDependencies::default())
            .unwrap();
        let expected = Expr::concat(vec![Expr::literal("foo"), Expr::literal("bar")])
            .with_inferred_type(InferredType::string());
        assert_eq!(expr, expected);
    }

    #[test]
    pub fn test_pull_up_for_not() {
        let mut expr = Expr::not(Expr::boolean(true));
        expr.pull_types_up(&ComponentDependencies::default())
            .unwrap();
        assert_eq!(expr.inferred_type(), InferredType::bool());
    }

    #[test]
    pub fn test_pull_up_if_else() {
        let inner1 = Expr::identifier_global("foo", None)
            .merge_inferred_type(InferredType::list(InferredType::u64()));

        let select_index1 = Expr::select_index(inner1.clone(), Expr::number(BigDecimal::from(0)));
        let select_index2 = Expr::select_index(inner1, Expr::number(BigDecimal::from(1)));

        let inner2 = Expr::identifier_global("bar", None)
            .merge_inferred_type(InferredType::list(InferredType::u64()));

        let select_index3 = Expr::select_index(inner2.clone(), Expr::number(BigDecimal::from(0)));
        let select_index4 = Expr::select_index(inner2, Expr::number(BigDecimal::from(1)));

        let mut expr = Expr::cond(
            Expr::greater_than(select_index1.clone(), select_index2.clone()),
            select_index3.clone(),
            select_index4.clone(),
        );

        expr.pull_types_up(&ComponentDependencies::default())
            .unwrap();
        let expected = Expr::cond(
            Expr::greater_than(
                Expr::select_index(
                    Expr::identifier_global("foo", None)
                        .with_inferred_type(InferredType::list(InferredType::u64())),
                    Expr::number(BigDecimal::from(0)),
                )
                .with_inferred_type(InferredType::u64()),
                Expr::select_index(
                    Expr::identifier_global("foo", None)
                        .with_inferred_type(InferredType::list(InferredType::u64())),
                    Expr::number(BigDecimal::from(1)),
                )
                .with_inferred_type(InferredType::u64()),
            )
            .with_inferred_type(InferredType::bool()),
            Expr::select_index(
                Expr::identifier_global("bar", None)
                    .with_inferred_type(InferredType::list(InferredType::u64())),
                Expr::number(BigDecimal::from(0)),
            )
            .with_inferred_type(InferredType::u64()),
            Expr::select_index(
                Expr::identifier_global("bar", None)
                    .with_inferred_type(InferredType::list(InferredType::u64())),
                Expr::number(BigDecimal::from(1)),
            )
            .with_inferred_type(InferredType::u64()),
        )
        .with_inferred_type(InferredType::u64());
        assert_eq!(expr, expected);
    }

    #[test]
    pub fn test_pull_up_for_greater_than() {
        let inner =
            Expr::identifier_global("foo", None).merge_inferred_type(InferredType::record(vec![
                ("bar".to_string(), InferredType::string()),
                ("baz".to_string(), InferredType::u64()),
            ]));

        let select_field1 = Expr::select_field(inner.clone(), "bar", None);
        let select_field2 = Expr::select_field(inner, "baz", None);
        let mut expr = Expr::greater_than(select_field1.clone(), select_field2.clone());

        expr.pull_types_up(&ComponentDependencies::default())
            .unwrap();

        let expected = Expr::greater_than(
            select_field1.merge_inferred_type(InferredType::string()),
            select_field2.merge_inferred_type(InferredType::u64()),
        )
        .merge_inferred_type(InferredType::bool());
        assert_eq!(expr, expected);
    }

    #[test]
    pub fn test_pull_up_for_greater_than_or_equal_to() {
        let inner = Expr::identifier_global("foo", None)
            .merge_inferred_type(InferredType::list(InferredType::u64()));

        let select_index1 = Expr::select_index(inner.clone(), Expr::number(BigDecimal::from(0)));
        let select_index2 = Expr::select_index(inner, Expr::number(BigDecimal::from(1)));
        let mut expr = Expr::greater_than_or_equal_to(select_index1.clone(), select_index2.clone());

        expr.pull_types_up(&ComponentDependencies::default())
            .unwrap();

        let expected = Expr::greater_than_or_equal_to(
            select_index1.merge_inferred_type(InferredType::u64()),
            select_index2.merge_inferred_type(InferredType::u64()),
        )
        .merge_inferred_type(InferredType::bool());
        assert_eq!(expr, expected);
    }

    #[test]
    pub fn test_pull_up_for_less_than_or_equal_to() {
        let record_type = InferredType::record(vec![
            ("bar".to_string(), InferredType::string()),
            ("baz".to_string(), InferredType::u64()),
        ]);

        let inner = Expr::identifier_global("foo", None)
            .merge_inferred_type(InferredType::list(record_type.clone()));

        let select_field_from_first = Expr::select_field(
            Expr::select_index(inner.clone(), Expr::number(BigDecimal::from(0))),
            "bar",
            None,
        );
        let select_field_from_second = Expr::select_field(
            Expr::select_index(inner.clone(), Expr::number(BigDecimal::from(1))),
            "baz",
            None,
        );
        let mut expr = Expr::less_than_or_equal_to(
            select_field_from_first.clone(),
            select_field_from_second.clone(),
        );

        expr.pull_types_up(&ComponentDependencies::default())
            .unwrap();

        let new_select_field_from_first = Expr::select_field(
            Expr::select_index(inner.clone(), Expr::number(BigDecimal::from(0)))
                .merge_inferred_type(record_type.clone()),
            "bar",
            None,
        )
        .merge_inferred_type(InferredType::string());

        let new_select_field_from_second = Expr::select_field(
            Expr::select_index(inner.clone(), Expr::number(BigDecimal::from(1)))
                .merge_inferred_type(record_type),
            "baz",
            None,
        )
        .merge_inferred_type(InferredType::u64());

        let expected =
            Expr::less_than_or_equal_to(new_select_field_from_first, new_select_field_from_second)
                .merge_inferred_type(InferredType::bool());

        assert_eq!(expr, expected);
    }

    #[test]
    pub fn test_pull_up_for_equal_to() {
        let mut expr = Expr::equal_to(
            Expr::number(BigDecimal::from(1)),
            Expr::number(BigDecimal::from(2)),
        );
        expr.pull_types_up(&ComponentDependencies::default())
            .unwrap();
        assert_eq!(expr.inferred_type(), InferredType::bool());
    }

    #[test]
    pub fn test_pull_up_for_less_than() {
        let mut expr = Expr::less_than(
            Expr::number(BigDecimal::from(1)),
            Expr::number(BigDecimal::from(2)),
        );

        expr.pull_types_up(&ComponentDependencies::default())
            .unwrap();

        assert_eq!(expr.inferred_type(), InferredType::bool());
    }

    #[test]
    pub fn test_pull_up_for_call() {
        let mut expr = Expr::call_worker_function(
            DynamicParsedFunctionName::parse("global_fn").unwrap(),
            None,
            None,
            vec![Expr::number(BigDecimal::from(1))],
            None,
        );

        expr.pull_types_up(&ComponentDependencies::default())
            .unwrap();

        assert_eq!(expr.inferred_type(), InferredType::unknown());
    }

    #[test]
    pub fn test_pull_up_for_dynamic_call() {
        let rib = r#"
           let input = { foo: "afs", bar: "al" };
           golem:it/api.{cart-checkout}(input.foo)
        "#;

        let mut expr = Expr::from_text(rib).unwrap();
        let component_dependencies = ComponentDependencies::default();

        expr.infer_types_initial_phase(&component_dependencies, &vec![], &[])
            .unwrap();
        expr.infer_all_identifiers();
        expr.pull_types_up(&ComponentDependencies::default())
            .unwrap();

        let expected = Expr::expr_block(vec![
            Expr::let_binding_with_variable_id(
                VariableId::local("input", 0),
                Expr::record(vec![
                    (
                        "foo".to_string(),
                        Expr::literal("afs").with_inferred_type(InferredType::string()),
                    ),
                    (
                        "bar".to_string(),
                        Expr::literal("al").with_inferred_type(InferredType::string()),
                    ),
                ])
                .with_inferred_type(InferredType::record(vec![
                    ("foo".to_string(), InferredType::string()),
                    ("bar".to_string(), InferredType::string()),
                ])),
                None,
            ),
            Expr::call(
                CallType::function_call(
                    DynamicParsedFunctionName {
                        site: PackagedInterface {
                            namespace: "golem".to_string(),
                            package: "it".to_string(),
                            interface: "api".to_string(),
                            version: None,
                        },
                        function: Function {
                            function: "cart-checkout".to_string(),
                        },
                    },
                    None,
                ),
                None,
                vec![Expr::select_field(
                    Expr::identifier_local("input", 0, None).with_inferred_type(
                        InferredType::record(vec![
                            ("foo".to_string(), InferredType::string()),
                            ("bar".to_string(), InferredType::string()),
                        ]),
                    ),
                    "foo",
                    None,
                )
                .with_inferred_type(InferredType::string())],
            ),
        ]);

        assert_eq!(expr, expected);
    }

    #[test]
    pub fn test_pull_up_for_unwrap() {
        let mut number = Expr::number(BigDecimal::from(1));
        number.with_inferred_type_mut(InferredType::f64());
        let mut expr = Expr::option(Some(number)).unwrap();
        expr.pull_types_up(&ComponentDependencies::default())
            .unwrap();
        assert_eq!(
            expr.inferred_type(),
            InferredType::option(InferredType::f64())
        );
    }

    #[test]
    pub fn test_pull_up_for_tag() {
        let mut number = Expr::number(BigDecimal::from(1));
        number.with_inferred_type_mut(InferredType::f64());
        let mut expr = Expr::get_tag(Expr::option(Some(number)));
        expr.pull_types_up(&ComponentDependencies::default())
            .unwrap();
        assert_eq!(
            expr.inferred_type(),
            InferredType::option(InferredType::f64())
        );
    }

    #[test]
    pub fn test_pull_up_for_pattern_match() {
        let mut expr = Expr::pattern_match(
            Expr::select_field(
                Expr::identifier_global("foo", None).merge_inferred_type(InferredType::record(
                    vec![("bar".to_string(), InferredType::string())],
                )),
                "bar",
                None,
            ),
            vec![
                MatchArm {
                    arm_pattern: ArmPattern::Constructor(
                        "cons1".to_string(),
                        vec![ArmPattern::Literal(Box::new(Expr::select_field(
                            Expr::identifier_global("foo", None).merge_inferred_type(
                                InferredType::record(vec![(
                                    "bar".to_string(),
                                    InferredType::string(),
                                )]),
                            ),
                            "bar",
                            None,
                        )))],
                    ),
                    arm_resolution_expr: Box::new(Expr::select_field(
                        Expr::identifier_global("baz", None).merge_inferred_type(
                            InferredType::record(vec![("qux".to_string(), InferredType::string())]),
                        ),
                        "qux",
                        None,
                    )),
                },
                MatchArm {
                    arm_pattern: ArmPattern::Constructor(
                        "cons2".to_string(),
                        vec![ArmPattern::Literal(Box::new(Expr::select_field(
                            Expr::identifier_global("quux", None).merge_inferred_type(
                                InferredType::record(vec![(
                                    "corge".to_string(),
                                    InferredType::string(),
                                )]),
                            ),
                            "corge",
                            None,
                        )))],
                    ),
                    arm_resolution_expr: Box::new(Expr::select_field(
                        Expr::identifier_global("grault", None).merge_inferred_type(
                            InferredType::record(vec![(
                                "garply".to_string(),
                                InferredType::string(),
                            )]),
                        ),
                        "garply",
                        None,
                    )),
                },
            ],
        );

        expr.pull_types_up(&ComponentDependencies::default())
            .unwrap();

        let expected = expected_pattern_match();

        assert_eq!(expr, expected);
    }

    fn expected_pattern_match() -> Expr {
        Expr::pattern_match(
            Expr::select_field(
                Expr::identifier_global("foo", None).with_inferred_type(InferredType::record(
                    vec![("bar".to_string(), InferredType::string())],
                )),
                "bar",
                None,
            )
            .with_inferred_type(InferredType::string()),
            vec![
                MatchArm {
                    arm_pattern: ArmPattern::Constructor(
                        "cons1".to_string(),
                        vec![ArmPattern::Literal(Box::new(
                            Expr::select_field(
                                Expr::identifier_global("foo", None).with_inferred_type(
                                    InferredType::record(vec![(
                                        "bar".to_string(),
                                        InferredType::string(),
                                    )]),
                                ),
                                "bar",
                                None,
                            )
                            .with_inferred_type(InferredType::string()),
                        ))],
                    ),
                    arm_resolution_expr: Box::new(
                        Expr::select_field(
                            Expr::identifier_global("baz", None).with_inferred_type(
                                InferredType::record(vec![(
                                    "qux".to_string(),
                                    InferredType::string(),
                                )]),
                            ),
                            "qux",
                            None,
                        )
                        .with_inferred_type(InferredType::string()),
                    ),
                },
                MatchArm {
                    arm_pattern: ArmPattern::Constructor(
                        "cons2".to_string(),
                        vec![ArmPattern::Literal(Box::new(
                            Expr::select_field(
                                Expr::identifier_global("quux", None).with_inferred_type(
                                    InferredType::record(vec![(
                                        "corge".to_string(),
                                        InferredType::string(),
                                    )]),
                                ),
                                "corge",
                                None,
                            )
                            .with_inferred_type(InferredType::string()),
                        ))],
                    ),
                    arm_resolution_expr: Box::new(
                        Expr::select_field(
                            Expr::identifier_global("grault", None).with_inferred_type(
                                InferredType::record(vec![(
                                    "garply".to_string(),
                                    InferredType::string(),
                                )]),
                            ),
                            "garply",
                            None,
                        )
                        .with_inferred_type(InferredType::string()),
                    ),
                },
            ],
        )
        .with_inferred_type(InferredType::string())
    }
}
