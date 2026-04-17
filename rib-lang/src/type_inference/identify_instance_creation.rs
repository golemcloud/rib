use crate::call_type::InstanceCreationType;
use crate::instance_type::InstanceType;
use crate::rib_type_error::RibTypeErrorInternal;
use crate::wit_type::WitType;
use crate::{ComponentDependency, CustomInstanceSpec, Expr};
use crate::{CustomError, InferredType, ParsedFunctionReference, TypeInternal, TypeOrigin};
use std::sync::Arc;

use crate::expr_arena::{
    rebuild_expr, CallTypeNode, ExprArena, ExprId, ExprKind, InstanceCreationNode, TypeTable,
};
use crate::type_inference::expr_visitor::arena::children_of;

pub fn identify_instance_creation(
    root: ExprId,
    arena: &mut ExprArena,
    types: &mut TypeTable,
    component: Arc<ComponentDependency>,
    custom_instance_spec: &[CustomInstanceSpec],
) -> Result<(), RibTypeErrorInternal> {
    search_for_invalid_instance_declarations_arena(root, arena, types)?;
    identify_instance_creation_with_worker_arena(
        root,
        arena,
        types,
        component,
        custom_instance_spec,
    )
}

fn search_for_invalid_instance_declarations_arena(
    root: ExprId,
    arena: &ExprArena,
    _types: &TypeTable,
) -> Result<(), RibTypeErrorInternal> {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let node = arena.expr(id);
        let span = node.source_span.clone();
        match &node.kind.clone() {
            ExprKind::Let { variable_id, .. } => {
                if variable_id.name() == "instance" || variable_id.name() == "env" {
                    return Err(CustomError::new(
                        span,
                        format!(
                            "`{}` is a reserved keyword and cannot be used as a variable.",
                            variable_id.name()
                        ),
                    )
                    .into());
                }
            }
            ExprKind::Identifier { variable_id } => {
                if variable_id.name() == "instance" && variable_id.is_global() {
                    return Err(CustomError::new(span, "`instance` is a reserved keyword")
                        .with_help_message(
                            "use `instance()` instead of `instance` to create an ephemeral instance.",
                        )
                        .with_help_message(
                            "for a named instance, use `instance(\"foo\")` where `\"foo\"` is the instance name",
                        )
                        .into());
                }
            }
            _ => {}
        }
        for child in children_of(id, arena).into_iter().rev() {
            stack.push(child);
        }
    }
    Ok(())
}

fn identify_instance_creation_with_worker_arena(
    root: ExprId,
    arena: &mut ExprArena,
    types: &mut TypeTable,
    component: Arc<ComponentDependency>,
    custom_instance_spec: &[CustomInstanceSpec],
) -> Result<(), RibTypeErrorInternal> {
    // Collect Call nodes bottom-up (post-order)
    let mut order = Vec::new();
    collect_post_order(root, arena, &mut order);

    for id in order {
        let kind = arena.expr(id).kind.clone();
        let span = arena.expr(id).source_span.clone();

        if let ExprKind::Call {
            ref call_type,
            ref args,
            ..
        } = kind
        {
            let args_ids: Vec<ExprId> = args.clone();

            let result = get_instance_creation_details_arena(
                call_type,
                &args_ids,
                arena,
                types,
                component.as_ref(),
                custom_instance_spec,
            )
            .map_err(|err| {
                RibTypeErrorInternal::from(CustomError::new(
                    span.clone(),
                    format!("failed to create instance: {err}"),
                ))
            })?;

            if let Some(instance_creation_type) = result {
                let worker_name = instance_creation_type.worker_name();

                let new_instance_type = InstanceType::from(component.clone(), worker_name.as_ref())
                    .map_err(|err| {
                        RibTypeErrorInternal::from(CustomError::new(
                            span.clone(),
                            format!("failed to create instance: {err}"),
                        ))
                    })?;

                let new_type = InferredType::new(
                    TypeInternal::Instance {
                        instance_type: Box::new(new_instance_type),
                    },
                    TypeOrigin::NoOrigin,
                );

                // Convert old InstanceCreationType to arena CallTypeNode
                let new_call_type =
                    convert_instance_creation_type(instance_creation_type, arena, types);

                let node_mut = arena.expr_mut(id);
                if let ExprKind::Call {
                    call_type: ref mut ct,
                    ..
                } = node_mut.kind
                {
                    *ct = new_call_type;
                }
                types.set(id, new_type);
            }
        }
    }

    Ok(())
}

fn convert_instance_creation_type(
    ict: InstanceCreationType,
    arena: &mut ExprArena,
    types: &mut TypeTable,
) -> CallTypeNode {
    match ict {
        InstanceCreationType::WitWorker {
            component_info,
            worker_name,
        } => {
            let worker_id = worker_name.map(|wn| lower_expr_into_arena(*wn, arena, types));
            CallTypeNode::InstanceCreation(InstanceCreationNode::WitWorker {
                component_info,
                worker_name: worker_id,
            })
        }
        InstanceCreationType::WitResource {
            component_info,
            module,
            resource_name,
        } => {
            use crate::expr_arena::InstanceIdentifierNode;
            let module_node = module.map(|m| match m {
                crate::call_type::InstanceIdentifier::WitWorker {
                    variable_id,
                    worker_name,
                } => InstanceIdentifierNode::WitWorker {
                    variable_id,
                    worker_name: worker_name.map(|wn| lower_expr_into_arena(*wn, arena, types)),
                },
                crate::call_type::InstanceIdentifier::WitResource {
                    variable_id,
                    worker_name,
                    resource_name,
                } => InstanceIdentifierNode::WitResource {
                    variable_id,
                    worker_name: worker_name.map(|wn| lower_expr_into_arena(*wn, arena, types)),
                    resource_name,
                },
            });
            CallTypeNode::InstanceCreation(InstanceCreationNode::WitResource {
                component_info,
                module: module_node,
                resource_name,
            })
        }
    }
}

/// Lower an `Expr` value into the arena, returning its new `ExprId`.
/// Used when converting `InstanceCreationType` worker name expressions.
fn lower_expr_into_arena(expr: Expr, arena: &mut ExprArena, types: &mut TypeTable) -> ExprId {
    crate::expr_arena::lower_into(arena, types, &expr)
}

fn get_instance_creation_details_arena(
    call_type: &CallTypeNode,
    args: &[ExprId],
    arena: &mut ExprArena,
    types: &mut TypeTable,
    component_dependency: &ComponentDependency,
    custom_instance_spec: &[CustomInstanceSpec],
) -> Result<Option<InstanceCreationType>, String> {
    match call_type {
        CallTypeNode::Function { function_name, .. } => {
            let fn_ref = function_name.to_parsed_function_name().function;
            match fn_ref {
                ParsedFunctionReference::Function { function } if function == "instance" => {
                    // Get the first arg as an optional worker name
                    let worker_name_expr = args.first().map(|&id| rebuild_expr(id, arena, types));
                    let instance_creation =
                        component_dependency.get_worker_instance_type(worker_name_expr)?;
                    Ok(Some(instance_creation))
                }
                ParsedFunctionReference::Function { function } => {
                    let spec = custom_instance_spec
                        .iter()
                        .find(|s| s.instance_name == function)
                        .cloned();
                    match spec {
                        None => Ok(None),
                        Some(spec) => {
                            let prefix = format!("{}(", spec.instance_name);
                            let mut concat_parts: Vec<Expr> = vec![Expr::literal(prefix)];

                            if args.len() != spec.parameter_types.len() {
                                return Err(format!(
                                    "expected {} arguments, found {}",
                                    spec.parameter_types.len(),
                                    args.len()
                                ));
                            }

                            let mut args_iter =
                                args.iter().zip(spec.parameter_types.iter()).peekable();

                            while let Some((&arg_id, analysed_type)) = args_iter.next() {
                                let inferred = InferredType::from(analysed_type);
                                let current = types.get(arg_id).clone();
                                types.set(arg_id, current.merge(inferred));

                                let arg_expr = rebuild_expr(arg_id, arena, types);
                                match analysed_type {
                                    WitType::Str(_) => {
                                        concat_parts.push(Expr::literal("\""));
                                        concat_parts.push(arg_expr);
                                        concat_parts.push(Expr::literal("\""));
                                    }
                                    _ => {
                                        concat_parts.push(arg_expr);
                                    }
                                }
                                if args_iter.peek().is_some() {
                                    concat_parts.push(Expr::literal(","));
                                }
                            }
                            concat_parts.push(Expr::literal(")"));
                            let worker_name_expr = Expr::concat(concat_parts);
                            let instance_creation = component_dependency
                                .get_worker_instance_type(Some(worker_name_expr))?;
                            Ok(Some(instance_creation))
                        }
                    }
                }
                _ => Ok(None),
            }
        }
        CallTypeNode::InstanceCreation(creation) => {
            // Already identified — convert back to old type for InstanceType::from
            let ict = match creation {
                InstanceCreationNode::WitWorker {
                    component_info,
                    worker_name,
                } => {
                    let wn = worker_name.map(|wn_id| Box::new(rebuild_expr(wn_id, arena, types)));
                    InstanceCreationType::WitWorker {
                        component_info: component_info.clone(),
                        worker_name: wn,
                    }
                }
                InstanceCreationNode::WitResource {
                    component_info,
                    module: _,
                    resource_name,
                } => {
                    InstanceCreationType::WitResource {
                        component_info: component_info.clone(),
                        module: None, // simplified
                        resource_name: resource_name.clone(),
                    }
                }
            };
            Ok(Some(ict))
        }
        CallTypeNode::VariantConstructor(_) | CallTypeNode::EnumConstructor(_) => Ok(None),
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
