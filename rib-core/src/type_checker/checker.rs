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

use crate::analysis::AnalysedType;
use crate::call_type::CallType;
use crate::expr_arena::{
    rebuild_arm_pattern, rebuild_call_type, ArmPatternNode, CallTypeNode, ExprArena, ExprId,
    ExprKind, InstanceCreationNode, InstanceIdentifierNode, MatchArmNode, RangeKind,
    ResultExprKind, TypeTable,
};
use crate::rib_source_span::SourceSpan;
use crate::rib_type_error::RibTypeErrorInternal;
use crate::type_checker::exhaustive_pattern_match::{
    check_exhaustive_pattern_match_with_arms, ExhaustivePatternMatchError,
};
use crate::type_checker::{Path, PathElem};
use crate::type_inference::arena::children_of;
use crate::{
    ComponentDependency, FunctionCallError, FunctionName, InvalidWorkerName, UnResolvedTypesError,
};
use std::collections::VecDeque;

pub fn type_check(
    root: ExprId,
    arena: &ExprArena,
    types: &mut TypeTable,
    component_dependency: &ComponentDependency,
) -> Result<(), RibTypeErrorInternal> {
    check_unresolved_types_lowered(root, arena, types)?;
    check_invalid_function_args_lowered(root, arena, types, component_dependency)?;
    check_invalid_worker_name_lowered(root, arena, types)?;
    check_exhaustive_pattern_match_lowered(root, arena, types, component_dependency)?;
    check_invalid_function_calls_lowered(root, arena, types)?;
    Ok(())
}

// --- missing fields -----------------------------------------------------------

fn find_missing_fields_in_record_lowered(
    expr_id: ExprId,
    arena: &ExprArena,
    expected: &AnalysedType,
) -> Vec<Path> {
    let mut missing_paths = Vec::new();

    if let AnalysedType::Record(expected_record) = expected {
        for (field_name, expected_type_of_field) in expected_record
            .fields
            .iter()
            .map(|name_typ| (name_typ.name.clone(), name_typ.typ.clone()))
        {
            if let ExprKind::Record { fields } = &arena.expr(expr_id).kind {
                let actual_value_opt = fields
                    .iter()
                    .find(|(name, _)| *name == field_name)
                    .map(|(_, id)| *id);

                if let Some(actual_id) = actual_value_opt {
                    if let AnalysedType::Record(record) = expected_type_of_field {
                        let nested_paths = find_missing_fields_in_record_lowered(
                            actual_id,
                            arena,
                            &AnalysedType::Record(record.clone()),
                        );
                        for mut nested_path in nested_paths {
                            nested_path.push_front(PathElem::Field(field_name.clone()));
                            missing_paths.push(nested_path);
                        }
                    }
                } else {
                    missing_paths.push(Path::from_elem(PathElem::Field(field_name.clone())));
                }
            }
        }
    }

    missing_paths
}

// --- invalid function args --------------------------------------------------

fn check_invalid_function_args_lowered(
    root: ExprId,
    arena: &ExprArena,
    types: &TypeTable,
    component_dependency: &ComponentDependency,
) -> Result<(), RibTypeErrorInternal> {
    let mut order = Vec::new();
    post_order_collect(root, arena, &mut order);

    for id in order {
        let node = arena.expr(id);
        let ExprKind::Call {
            call_type, args, ..
        } = &node.kind
        else {
            continue;
        };

        if matches!(call_type, CallTypeNode::InstanceCreation(_)) {
            continue;
        }

        let call_ty = rebuild_call_type(call_type, arena, types);
        get_missing_record_keys_lowered(
            &call_ty,
            args,
            component_dependency,
            arena,
            node.source_span.clone(),
        )?;
    }

    Ok(())
}

#[allow(clippy::result_large_err)]
fn get_missing_record_keys_lowered(
    call_type: &CallType,
    args: &[ExprId],
    component_dependency: &ComponentDependency,
    arena: &ExprArena,
    call_source_span: SourceSpan,
) -> Result<(), FunctionCallError> {
    let function_name =
        FunctionName::from_call_type(call_type).ok_or(FunctionCallError::InvalidFunctionCall {
            function_name: call_type.to_string(),
            source_span: call_source_span.clone(),
            message: "invalid function call type".to_string(),
        })?;

    let (_, function_type) = component_dependency
        .get_function_type(&function_name)
        .map_err(|err| FunctionCallError::InvalidFunctionCall {
            function_name: call_type.to_string(),
            source_span: call_source_span,
            message: err.to_string(),
        })?;

    let expected_arg_types = function_type.parameter_types;

    let mut filtered_expected_types = expected_arg_types
        .iter()
        .map(|x| AnalysedType::try_from(x).unwrap())
        .collect::<Vec<_>>();

    if call_type.is_resource_method() {
        filtered_expected_types.remove(0);
    }

    for (actual_arg_id, expected_arg_type) in args.iter().zip(filtered_expected_types) {
        let missing_fields =
            find_missing_fields_in_record_lowered(*actual_arg_id, arena, &expected_arg_type);

        if !missing_fields.is_empty() {
            return Err(FunctionCallError::MissingRecordFields {
                function_name: call_type.to_string(),
                argument_source_span: arena.expr(*actual_arg_id).source_span.clone(),
                missing_fields,
            });
        }
    }

    Ok(())
}

// --- invalid worker name ----------------------------------------------------

fn check_invalid_worker_name_lowered(
    root: ExprId,
    arena: &ExprArena,
    types: &TypeTable,
) -> Result<(), RibTypeErrorInternal> {
    let mut order = Vec::new();
    post_order_collect(root, arena, &mut order);

    for id in order {
        let node = arena.expr(id);
        let ExprKind::Call { call_type, .. } = &node.kind else {
            continue;
        };

        match call_type {
            CallTypeNode::InstanceCreation(InstanceCreationNode::WitWorker {
                worker_name, ..
            }) => {
                check_worker_name_opt(*worker_name, arena, types)?;
            }
            CallTypeNode::Function {
                instance_identifier: Some(ii),
                ..
            } => {
                check_worker_name_from_instance_id(ii, arena, types)?;
            }
            CallTypeNode::InstanceCreation(InstanceCreationNode::WitResource {
                module: Some(m),
                ..
            }) => {
                check_worker_name_from_instance_id(m, arena, types)?;
            }
            _ => {}
        }
    }

    Ok(())
}

fn check_worker_name_from_instance_id(
    ii: &InstanceIdentifierNode,
    arena: &ExprArena,
    types: &TypeTable,
) -> Result<(), RibTypeErrorInternal> {
    let wid = match ii {
        InstanceIdentifierNode::WitWorker { worker_name, .. }
        | InstanceIdentifierNode::WitResource { worker_name, .. } => *worker_name,
    };
    check_worker_name_opt(wid, arena, types).map_err(RibTypeErrorInternal::from)
}

fn check_worker_name_opt(
    worker_name_opt: Option<ExprId>,
    arena: &ExprArena,
    types: &TypeTable,
) -> Result<(), InvalidWorkerName> {
    use crate::type_refinement::precise_types::StringType;
    use crate::type_refinement::TypeRefinement;
    use crate::TypeName;

    let Some(wid) = worker_name_opt else {
        return Ok(());
    };

    let inferred_type = types.get(wid).clone();
    let string_type = StringType::refine(&inferred_type);

    match string_type {
        Some(_) => Ok(()),
        None => {
            let node = arena.expr(wid);
            let type_name = TypeName::try_from(inferred_type.clone())
                .map(|t| t.to_string())
                .unwrap_or_else(|_| "unknown".to_string());
            Err(InvalidWorkerName {
                worker_name_source_span: node.source_span.clone(),
                message: format!("expected string, found {type_name}"),
            })
        }
    }
}

// --- invalid function calls (component bound) -------------------------------

fn check_invalid_function_calls_lowered(
    root: ExprId,
    arena: &ExprArena,
    _types: &TypeTable,
) -> Result<(), RibTypeErrorInternal> {
    let mut order = Vec::new();
    post_order_collect(root, arena, &mut order);

    for id in order {
        let node = arena.expr(id);
        if let ExprKind::Call {
            call_type:
                CallTypeNode::Function {
                    component_info,
                    function_name,
                    ..
                },
            ..
        } = &node.kind
        {
            if component_info.is_none() {
                return Err(
                    FunctionCallError::InvalidFunctionCall {
                        function_name: function_name.function.name_pretty().to_string(),
                        source_span: node.source_span.clone(),
                        message: "function call is not associated with a wasm component. make sure component functions are called after creating an instance using `instance(<optional-worker-name>)`".to_string(),
                    }
                    .into(),
                );
            }
        }
    }

    Ok(())
}

// --- exhaustive pattern match -----------------------------------------------

fn check_exhaustive_pattern_match_lowered(
    root: ExprId,
    arena: &ExprArena,
    types: &TypeTable,
    component_dependency: &ComponentDependency,
) -> Result<(), ExhaustivePatternMatchError> {
    let mut order = Vec::new();
    post_order_collect(root, arena, &mut order);

    for id in order {
        let node = arena.expr(id);
        if let ExprKind::PatternMatch {
            predicate,
            match_arms,
        } = &node.kind
        {
            let scrutinee_type = types.get(*predicate);
            let arm_patterns: Vec<_> = match_arms
                .iter()
                .map(|arm| rebuild_arm_pattern(arm.arm_pattern, arena, types))
                .collect();
            check_exhaustive_pattern_match_with_arms(
                node.source_span.clone(),
                scrutinee_type,
                &arm_patterns,
                component_dependency,
            )?;
        }
    }

    Ok(())
}

// --- unresolved types ---------------------------------------------------------

fn call_type_node_additional_detail(ct: &CallTypeNode) -> String {
    match ct {
        CallTypeNode::Function { function_name, .. } => {
            format!("cannot determine the return type of the function `{function_name}`")
        }
        CallTypeNode::VariantConstructor(name) => {
            format!("cannot determine the type of the variant constructor `{name}`")
        }
        CallTypeNode::EnumConstructor(name) => {
            format!("cannot determine the type of the enum constructor `{name}`")
        }
        CallTypeNode::InstanceCreation(ic) => match ic {
            InstanceCreationNode::WitWorker { worker_name, .. } => {
                let wn = worker_name
                    .map(|_| "(worker expr)")
                    .map_or(String::new(), |s| format!(", with worker `{s}`"));
                format!("cannot determine the type of instance creation `{wn}`")
            }
            InstanceCreationNode::WitResource {
                module,
                resource_name,
                ..
            } => {
                let _ = module;
                format!(
                    "cannot determine the type of the resource creation `{}`",
                    resource_name.resource_name
                )
            }
        },
    }
}

fn call_type_worker_queue_ids(ct: &CallTypeNode) -> Vec<ExprId> {
    let mut out = Vec::new();
    match ct {
        CallTypeNode::Function {
            instance_identifier: Some(ii),
            ..
        } => match ii {
            InstanceIdentifierNode::WitWorker { worker_name, .. }
            | InstanceIdentifierNode::WitResource { worker_name, .. } => {
                if let Some(w) = worker_name {
                    out.push(*w);
                }
            }
        },
        CallTypeNode::InstanceCreation(InstanceCreationNode::WitWorker {
            worker_name: Some(w),
            ..
        }) => {
            out.push(*w);
        }
        CallTypeNode::InstanceCreation(InstanceCreationNode::WitResource {
            module: Some(m),
            ..
        }) => match m {
            InstanceIdentifierNode::WitWorker { worker_name, .. }
            | InstanceIdentifierNode::WitResource { worker_name, .. } => {
                if let Some(w) = worker_name {
                    out.push(*w);
                }
            }
        },
        _ => {}
    }
    out
}

fn check_unresolved_types_lowered(
    root: ExprId,
    arena: &ExprArena,
    types: &TypeTable,
) -> Result<(), UnResolvedTypesError> {
    let mut queue = VecDeque::new();
    queue.push_back(root);

    while let Some(id) = queue.pop_back() {
        let node = arena.expr(id);
        let span = node.source_span.clone();
        let inferred = types.get(id);

        match &node.kind {
            ExprKind::Let { expr, .. } => {
                queue.push_back(*expr);
            }

            ExprKind::Range { range } => {
                match range {
                    RangeKind::Range { from, to } | RangeKind::RangeInclusive { from, to } => {
                        queue.push_back(*from);
                        queue.push_back(*to);
                    }
                    RangeKind::RangeFrom { from } => {
                        queue.push_back(*from);
                    }
                }
                if inferred.is_unknown() {
                    return Err(UnResolvedTypesError::from(span));
                }
            }

            ExprKind::InvokeMethodLazy { lhs, args, .. } => {
                queue.push_back(*lhs);
                for a in args {
                    queue.push_back(*a);
                }
                if inferred.is_unknown() {
                    return Err(UnResolvedTypesError::from(span));
                }
            }

            ExprKind::SelectField { expr, field, .. } => {
                queue.push_back(*expr);
                if inferred.is_unknown() {
                    return Err(UnResolvedTypesError::from(span).at_field(field.clone()));
                }
            }

            ExprKind::SelectIndex { expr, index, .. } => {
                queue.push_back(*expr);
                queue.push_back(*index);
                if inferred.is_unknown() {
                    return Err(UnResolvedTypesError::from(span));
                }
            }

            ExprKind::Sequence { exprs, .. } => {
                for (index, eid) in exprs.iter().enumerate() {
                    check_unresolved_types_lowered(*eid, arena, types)
                        .map_err(|e| e.at_index(index))?;
                }
                if inferred.is_unknown() {
                    return Err(UnResolvedTypesError::from(span));
                }
            }

            ExprKind::Record { fields, .. } => {
                for (field_name, fid) in fields {
                    check_unresolved_types_lowered(*fid, arena, types)
                        .map_err(|e| e.at_field(field_name.clone()))?;
                }
                if inferred.is_unknown() {
                    return Err(UnResolvedTypesError::from(span));
                }
            }

            ExprKind::Tuple { exprs, .. } => {
                for (index, eid) in exprs.iter().enumerate() {
                    check_unresolved_types_lowered(*eid, arena, types)
                        .map_err(|e| e.at_index(index))?;
                }
            }

            ExprKind::Literal { .. } => {
                if inferred.is_unknown() {
                    return Err(UnResolvedTypesError::from(span));
                }
            }
            ExprKind::Number { .. } => {
                if inferred.is_unknown() {
                    return Err(UnResolvedTypesError::from(span));
                }
            }
            ExprKind::Flags { .. } => {
                if inferred.is_unknown() {
                    return Err(UnResolvedTypesError::from(span));
                }
            }
            ExprKind::Identifier { variable_id } => {
                if inferred.is_unknown() {
                    return Err(UnResolvedTypesError::from(span).with_help_message(
                        format!("make sure `{variable_id}` is a valid identifier").as_str(),
                    ));
                }
            }
            ExprKind::Boolean { .. } => {
                if inferred.is_unknown() {
                    return Err(UnResolvedTypesError::from(span));
                }
            }

            ExprKind::Concat { exprs, .. } => {
                for (index, eid) in exprs.iter().enumerate() {
                    let field_type = types.get(*eid);
                    if field_type.is_unknown() {
                        let ch = arena.expr(*eid);
                        return Err(
                            UnResolvedTypesError::from(ch.source_span.clone()).at_index(index)
                        );
                    }
                    check_unresolved_types_lowered(*eid, arena, types)
                        .map_err(|e| e.at_index(index))?;
                }
                if inferred.is_unknown() {
                    return Err(UnResolvedTypesError::from(span));
                }
            }

            ExprKind::ExprBlock { exprs, .. } => {
                for e in exprs {
                    queue.push_back(*e);
                }
            }

            ExprKind::Not { expr, .. } => {
                queue.push_back(*expr);
                if inferred.is_unknown() {
                    return Err(UnResolvedTypesError::from(span));
                }
            }

            ExprKind::GreaterThan { lhs, rhs, .. }
            | ExprKind::And { lhs, rhs, .. }
            | ExprKind::Plus { lhs, rhs, .. }
            | ExprKind::Minus { lhs, rhs, .. }
            | ExprKind::Multiply { lhs, rhs, .. }
            | ExprKind::Divide { lhs, rhs, .. }
            | ExprKind::Or { lhs, rhs, .. }
            | ExprKind::GreaterThanOrEqualTo { lhs, rhs, .. }
            | ExprKind::LessThanOrEqualTo { lhs, rhs, .. }
            | ExprKind::EqualTo { lhs, rhs, .. }
            | ExprKind::LessThan { lhs, rhs, .. } => {
                unresolved_type_for_binary_op_lowered(*lhs, *rhs, arena, types)?;
                if inferred.is_unknown() {
                    return Err(UnResolvedTypesError::from(span));
                }
            }

            ExprKind::Cond { cond, lhs, rhs, .. } => {
                unresolved_type_for_if_lowered(*cond, *lhs, *rhs, arena, types)?;
                if inferred.is_unknown() {
                    return Err(UnResolvedTypesError::from(span));
                }
            }

            ExprKind::PatternMatch {
                predicate,
                match_arms,
                ..
            } => {
                unresolved_type_for_pattern_match_lowered(*predicate, match_arms, arena, types)?;
            }

            ExprKind::Option { expr: opt, .. } => {
                if let Some(e) = opt {
                    queue.push_back(*e);
                }
                if inferred.is_unknown() {
                    return Err(UnResolvedTypesError::from(span));
                }
            }

            ExprKind::Result { expr: re } => match re {
                ResultExprKind::Ok(e) => {
                    check_unresolved_types_lowered(*e, arena, types)?;
                }
                ResultExprKind::Err(e) => {
                    check_unresolved_types_lowered(*e, arena, types)?;
                }
            },

            ExprKind::Call {
                call_type, args, ..
            } => {
                for a in args {
                    queue.push_back(*a);
                }
                for w in call_type_worker_queue_ids(call_type) {
                    queue.push_back(w);
                }
                if inferred.is_unknown() {
                    return Err(
                        UnResolvedTypesError::from(span).with_additional_error_detail(
                            call_type_node_additional_detail(call_type),
                        ),
                    );
                }
            }

            ExprKind::Unwrap { .. }
            | ExprKind::Throw { .. }
            | ExprKind::GenerateWorkerName { .. }
            | ExprKind::GetTag { .. } => {}

            ExprKind::ListComprehension {
                iterable_expr,
                yield_expr,
                ..
            } => {
                unresolved_type_for_list_comprehension_lowered(
                    *iterable_expr,
                    *yield_expr,
                    arena,
                    types,
                )?;
                if inferred.is_unknown() {
                    return Err(UnResolvedTypesError::from(span));
                }
            }

            ExprKind::Length { expr, .. } => {
                queue.push_back(*expr);
                if inferred.is_unknown() {
                    return Err(UnResolvedTypesError::from(span));
                }
            }

            ExprKind::ListReduce {
                iterable_expr,
                init_value_expr,
                yield_expr,
                ..
            } => {
                unresolved_type_for_list_reduce_lowered(
                    *iterable_expr,
                    *init_value_expr,
                    *yield_expr,
                    arena,
                    types,
                )?;
                if inferred.is_unknown() {
                    return Err(UnResolvedTypesError::from(span));
                }
            }
        }
    }

    Ok(())
}

fn unresolved_type_for_binary_op_lowered(
    left: ExprId,
    right: ExprId,
    arena: &ExprArena,
    types: &TypeTable,
) -> Result<(), UnResolvedTypesError> {
    check_unresolved_types_lowered(left, arena, types)?;
    check_unresolved_types_lowered(right, arena, types)?;
    Ok(())
}

fn unresolved_type_for_if_lowered(
    cond: ExprId,
    if_e: ExprId,
    else_e: ExprId,
    arena: &ExprArena,
    types: &TypeTable,
) -> Result<(), UnResolvedTypesError> {
    check_unresolved_types_lowered(cond, arena, types)?;
    check_unresolved_types_lowered(if_e, arena, types)?;
    check_unresolved_types_lowered(else_e, arena, types)?;
    Ok(())
}

fn unresolved_type_for_list_comprehension_lowered(
    iterable_expr: ExprId,
    yield_expr: ExprId,
    arena: &ExprArena,
    types: &TypeTable,
) -> Result<(), UnResolvedTypesError> {
    check_unresolved_types_lowered(iterable_expr, arena, types)?;
    check_unresolved_types_lowered(yield_expr, arena, types)?;
    Ok(())
}

fn unresolved_type_for_list_reduce_lowered(
    iterable_expr: ExprId,
    init_value_expr: ExprId,
    yield_expr: ExprId,
    arena: &ExprArena,
    types: &TypeTable,
) -> Result<(), UnResolvedTypesError> {
    check_unresolved_types_lowered(iterable_expr, arena, types)?;
    check_unresolved_types_lowered(init_value_expr, arena, types)?;
    check_unresolved_types_lowered(yield_expr, arena, types)?;
    Ok(())
}

fn unresolved_type_for_pattern_match_lowered(
    predicate: ExprId,
    match_arms: &[MatchArmNode],
    arena: &ExprArena,
    types: &TypeTable,
) -> Result<(), UnResolvedTypesError> {
    check_unresolved_types_lowered(predicate, arena, types)?;

    for arm in match_arms {
        for lit_id in arm_pattern_literal_expr_ids(arm.arm_pattern, arena) {
            check_unresolved_types_lowered(lit_id, arena, types)?;
        }
        check_unresolved_types_lowered(arm.arm_resolution_expr, arena, types)?;
    }

    Ok(())
}

fn arm_pattern_literal_expr_ids(
    pat: crate::expr_arena::ArmPatternId,
    arena: &ExprArena,
) -> Vec<ExprId> {
    let mut out = Vec::new();
    fn walk(pat: crate::expr_arena::ArmPatternId, arena: &ExprArena, out: &mut Vec<ExprId>) {
        match arena.pattern(pat) {
            ArmPatternNode::WildCard => {}
            ArmPatternNode::As(_, inner) => walk(*inner, arena, out),
            ArmPatternNode::Literal(eid) => out.push(*eid),
            ArmPatternNode::Constructor(_, children) => {
                for c in children {
                    walk(*c, arena, out);
                }
            }
            ArmPatternNode::TupleConstructor(children)
            | ArmPatternNode::ListConstructor(children) => {
                for c in children {
                    walk(*c, arena, out);
                }
            }
            ArmPatternNode::RecordConstructor(fields) => {
                for (_, c) in fields {
                    walk(*c, arena, out);
                }
            }
        }
    }
    walk(pat, arena, &mut out);
    out
}

fn post_order_collect(root: ExprId, arena: &ExprArena, out: &mut Vec<ExprId>) {
    for c in children_of(root, arena) {
        post_order_collect(c, arena, out);
    }
    out.push(root);
}
