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

use crate::call_type::{CallType, InstanceCreationType, InstanceIdentifier};
use crate::rib_type_error::RibTypeErrorInternal;
use crate::type_parameter::TypeParameter;
use crate::{
    CustomError, DynamicParsedFunctionName, Expr, FullyQualifiedResourceConstructor,
    FunctionCallError, InferredType, TypeInternal, TypeOrigin,
};
use crate::{FunctionName, InstanceType};
use std::collections::VecDeque;

pub mod arena {
    use crate::call_type::{CallType, InstanceCreationType, InstanceIdentifier};
    use crate::expr_arena::{
        CallTypeNode, ExprArena, ExprId, ExprKind, ExprNode, InstanceCreationNode,
        InstanceIdentifierNode, TypeTable,
    };
    use crate::rib_type_error::RibTypeErrorInternal;
    use crate::type_inference::expr_visitor::arena::children_of;
    use crate::type_parameter::TypeParameter;
    use crate::InstanceType;
    use crate::{
        CustomError, DynamicParsedFunctionName, FullyQualifiedResourceConstructor,
        FunctionCallError, FunctionName, InferredType, TypeInternal, TypeOrigin, VariableId,
    };

    /// Arena version of `infer_worker_function_invokes`.
    ///
    /// Converts `SelectField` on an `Instance` type into a resolved `Call`,
    /// and `InvokeMethodLazy` on an `Instance` type into a resolved `Call`.
    pub fn infer_worker_function_invokes(
        root: ExprId,
        arena: &mut ExprArena,
        types: &mut TypeTable,
        component_dependencies: &crate::ComponentDependencies,
    ) -> Result<(), RibTypeErrorInternal> {
        let mut stack = vec![root];

        while let Some(id) = stack.pop() {
            let kind = arena.expr(id).kind.clone();
            let span = arena.expr(id).source_span.clone();

            match kind {
                ExprKind::SelectField {
                    expr: lhs_id,
                    ref field,
                } => {
                    let field = field.clone();
                    let lhs_type = types.get(lhs_id).clone();

                    if let TypeInternal::Instance { instance_type } = lhs_type.internal_type() {
                        let (component, function) = instance_type
                            .get_function(&field, None)
                            .map_err(|err| {
                                FunctionCallError::invalid_function_call(&field, span.clone(), err)
                            })
                            .map_err(RibTypeErrorInternal::from)?;

                        let module =
                            get_instance_identifier_from_arena(&instance_type, lhs_id, arena);

                        // Narrow the instance type on the lhs
                        let lhs_type_narrowed = {
                            let mut t = lhs_type.clone();
                            t.internal_type_mut().narrow_to_single_component(&component);
                            t
                        };
                        types.set(lhs_id, lhs_type_narrowed);

                        if let FunctionName::ResourceConstructor(
                            fully_qualified_resource_constructor,
                        ) = function.function_name
                        {
                            let (resource_id, resource_mode) = match function
                                .function_type
                                .return_type
                            {
                                Some(rt) => match rt.internal_type() {
                                    TypeInternal::Resource {
                                        resource_id,
                                        resource_mode,
                                        ..
                                    } => (*resource_id, *resource_mode),
                                    _ => {
                                        return Err(RibTypeErrorInternal::from(
                                            CustomError::new(
                                                span.clone(),
                                                "expected resource type as return type of resource constructor",
                                            ),
                                        ))
                                    }
                                },
                                None => {
                                    return Err(RibTypeErrorInternal::from(CustomError::new(
                                        span.clone(),
                                        "resource constructor must have a return type",
                                    )));
                                }
                            };

                            let resource_instance_type = instance_type
                                .get_resource_instance_type(
                                    fully_qualified_resource_constructor.clone(),
                                    vec![],
                                    instance_type.worker_name(),
                                    resource_id,
                                    resource_mode,
                                )
                                .map_err(|err| {
                                    RibTypeErrorInternal::from(CustomError::new(
                                        span.clone(),
                                        format!("Failed to get resource instance type: {err}"),
                                    ))
                                })?;

                            let new_type = InferredType::new(
                                TypeInternal::Instance {
                                    instance_type: Box::new(resource_instance_type),
                                },
                                TypeOrigin::NoOrigin,
                            );

                            let module_node =
                                to_instance_identifier_node_with_arena(module, arena, types);
                            let node_mut = arena.expr_mut(id);
                            node_mut.kind = ExprKind::Call {
                                call_type: CallTypeNode::InstanceCreation(
                                    InstanceCreationNode::WitResource {
                                        component_info: Some(component),
                                        module: Some(module_node),
                                        resource_name: fully_qualified_resource_constructor,
                                    },
                                ),
                                generic_type_parameter: None,
                                args: vec![],
                            };
                            types.set(id, new_type);
                        }
                        // For other function types, push children
                    }
                    for child in children_of(id, arena).into_iter().rev() {
                        stack.push(child);
                    }
                }

                ExprKind::InvokeMethodLazy {
                    lhs: lhs_id,
                    ref method,
                    ref generic_type_parameter,
                    ref args,
                } => {
                    let method = method.clone();
                    let gtp = generic_type_parameter.clone();
                    let args_ids: Vec<ExprId> = args.clone();
                    let lhs_type = types.get(lhs_id).clone();

                    match lhs_type.internal_type() {
                        TypeInternal::Instance { instance_type } => {
                            let type_parameter = gtp
                                .as_ref()
                                .map(|g| {
                                    TypeParameter::from_text(&g.value).map_err(|err| {
                                        FunctionCallError::invalid_generic_type_parameter(
                                            &g.value,
                                            err,
                                            span.clone(),
                                        )
                                    })
                                })
                                .transpose()
                                .map_err(RibTypeErrorInternal::from)?;

                            let (component, function) = instance_type
                                .get_function(&method, type_parameter)
                                .map_err(|err| {
                                    FunctionCallError::invalid_function_call(
                                        &method,
                                        span.clone(),
                                        err,
                                    )
                                })
                                .map_err(RibTypeErrorInternal::from)?;

                            let module =
                                get_instance_identifier_from_arena(&instance_type, lhs_id, arena);

                            // Narrow the lhs type
                            let lhs_narrowed = {
                                let mut t = lhs_type.clone();
                                t.internal_type_mut().narrow_to_single_component(&component);
                                t
                            };
                            types.set(lhs_id, lhs_narrowed);

                            match function.function_name {
                                FunctionName::Variant(_) | FunctionName::Enum(_) => {}

                                FunctionName::Function(fn_name) => {
                                    let dpfn_str = fn_name.to_string();
                                    let dpfn = DynamicParsedFunctionName::parse(&dpfn_str)
                                        .map_err(|err| {
                                            RibTypeErrorInternal::from(
                                                FunctionCallError::invalid_function_call(
                                                    &dpfn_str,
                                                    span.clone(),
                                                    format!("invalid function name: {err}"),
                                                ),
                                            )
                                        })?;

                                    let ii_node = to_instance_identifier_node_with_arena(
                                        module, arena, types,
                                    );
                                    let node_mut = arena.expr_mut(id);
                                    node_mut.kind = ExprKind::Call {
                                        call_type: CallTypeNode::Function {
                                            component_info: Some(component),
                                            instance_identifier: Some(ii_node),
                                            function_name: dpfn,
                                        },
                                        generic_type_parameter: None,
                                        args: args_ids,
                                    };
                                    node_mut.source_span = span;
                                }

                                FunctionName::ResourceConstructor(fqrc) => {
                                    let (resource_id, resource_mode) =
                                        match function.function_type.return_type {
                                            Some(rt) => match rt.internal_type() {
                                                TypeInternal::Resource {
                                                    resource_id,
                                                    resource_mode,
                                                    ..
                                                } => (*resource_id, *resource_mode),
                                                _ => {
                                                    return Err(RibTypeErrorInternal::from(
                                                        CustomError::new(
                                                            span.clone(),
                                                            "expected resource type",
                                                        ),
                                                    ))
                                                }
                                            },
                                            None => return Err(RibTypeErrorInternal::from(
                                                CustomError::new(
                                                    span.clone(),
                                                    "resource constructor must have a return type",
                                                ),
                                            )),
                                        };

                                    // Need to pass args as Expr for get_resource_instance_type
                                    let args_exprs: Vec<crate::Expr> = vec![];

                                    let resource_instance_type = instance_type
                                        .get_resource_instance_type(
                                            fqrc.clone(),
                                            args_exprs,
                                            instance_type.worker_name(),
                                            resource_id,
                                            resource_mode,
                                        )
                                        .map_err(|err| {
                                            RibTypeErrorInternal::from(CustomError::new(
                                                span.clone(),
                                                format!(
                                                    "Failed to get resource instance type: {err}"
                                                ),
                                            ))
                                        })?;

                                    let new_type = InferredType::new(
                                        TypeInternal::Instance {
                                            instance_type: Box::new(resource_instance_type),
                                        },
                                        TypeOrigin::NoOrigin,
                                    );

                                    let module_node = to_instance_identifier_node_with_arena(
                                        module, arena, types,
                                    );
                                    let node_mut = arena.expr_mut(id);
                                    node_mut.kind = ExprKind::Call {
                                        call_type: CallTypeNode::InstanceCreation(
                                            InstanceCreationNode::WitResource {
                                                component_info: Some(component),
                                                module: Some(module_node),
                                                resource_name: fqrc,
                                            },
                                        ),
                                        generic_type_parameter: None,
                                        args: args_ids,
                                    };
                                    types.set(id, new_type);
                                }

                                FunctionName::ResourceMethod(resource_method) => {
                                    let resource_method_dictionary =
                                        instance_type.resource_method_dictionary();

                                    let resource_method_info = resource_method_dictionary
                                        .get(&FunctionName::ResourceMethod(resource_method.clone()))
                                        .ok_or_else(|| {
                                            RibTypeErrorInternal::from(
                                                FunctionCallError::invalid_function_call(
                                                    resource_method.method_name(),
                                                    span.clone(),
                                                    format!(
                                                        "Resource method {} not found",
                                                        resource_method.method_name()
                                                    ),
                                                ),
                                            )
                                        })?;

                                    let return_type = resource_method_info
                                        .return_type
                                        .clone()
                                        .unwrap_or_else(InferredType::unknown);

                                    let new_inferred_type = match return_type.internal_type() {
                                        TypeInternal::Resource {
                                            resource_id,
                                            resource_mode,
                                            ..
                                        } => {
                                            let args_exprs: Vec<crate::Expr> = vec![];
                                            let resource_instance_type = instance_type
                                                .get_resource_instance_type(
                                                    FullyQualifiedResourceConstructor {
                                                        package_name: resource_method
                                                            .package_name
                                                            .clone(),
                                                        interface_name: resource_method
                                                            .interface_name
                                                            .clone(),
                                                        resource_name: "cart".to_string(),
                                                    },
                                                    args_exprs,
                                                    instance_type.worker_name(),
                                                    *resource_id,
                                                    *resource_mode,
                                                )
                                                .map_err(|err| {
                                                    RibTypeErrorInternal::from(CustomError::new(
                                                        span.clone(),
                                                        format!(
                                                            "Failed to get resource instance type: {err}"
                                                        ),
                                                    ))
                                                })?;
                                            InferredType::new(
                                                TypeInternal::Instance {
                                                    instance_type: Box::new(resource_instance_type),
                                                },
                                                TypeOrigin::NoOrigin,
                                            )
                                        }
                                        _ => InferredType::unknown(),
                                    };

                                    let dpfn = resource_method
                                        .dynamic_parsed_function_name()
                                        .map_err(|err| {
                                            RibTypeErrorInternal::from(
                                                FunctionCallError::invalid_function_call(
                                                    resource_method.method_name(),
                                                    span.clone(),
                                                    format!("Invalid resource method name: {err}"),
                                                ),
                                            )
                                        })?;

                                    let ii_node = to_instance_identifier_node_with_arena(
                                        module, arena, types,
                                    );
                                    let node_mut = arena.expr_mut(id);
                                    node_mut.kind = ExprKind::Call {
                                        call_type: CallTypeNode::Function {
                                            component_info: Some(component),
                                            instance_identifier: Some(ii_node),
                                            function_name: dpfn,
                                        },
                                        generic_type_parameter: None,
                                        args: args_ids,
                                    };
                                    types.set(id, new_inferred_type);
                                    node_mut.source_span = span;
                                }
                            }
                        }
                        TypeInternal::Unknown => {
                            // Not yet identified — re-run will handle
                        }
                        _ => {
                            return Err(RibTypeErrorInternal::from(
                                FunctionCallError::invalid_function_call(
                                    &method,
                                    span.clone(),
                                    "invalid worker function invoke. Expected to be an instance type",
                                ),
                            ));
                        }
                    }

                    for child in children_of(id, arena).into_iter().rev() {
                        stack.push(child);
                    }
                }

                _ => {
                    for child in children_of(id, arena).into_iter().rev() {
                        stack.push(child);
                    }
                }
            }
        }

        Ok(())
    }

    fn get_instance_identifier_from_arena(
        instance_type: &InstanceType,
        lhs_id: ExprId,
        arena: &ExprArena,
    ) -> InstanceIdentifier {
        let variable_id = match &arena.expr(lhs_id).kind {
            ExprKind::Identifier { variable_id } => Some(variable_id.clone()),
            _ => None,
        };

        match instance_type {
            InstanceType::Resource {
                worker_name,
                resource_constructor,
                ..
            } => InstanceIdentifier::WitResource {
                variable_id,
                worker_name: worker_name.clone(),
                resource_name: resource_constructor.clone(),
            },
            other => InstanceIdentifier::WitWorker {
                variable_id,
                worker_name: other.worker_name(),
            },
        }
    }

    fn to_instance_identifier_node_with_arena(
        ii: InstanceIdentifier,
        arena: &mut ExprArena,
        types: &mut TypeTable,
    ) -> InstanceIdentifierNode {
        match ii {
            InstanceIdentifier::WitWorker {
                variable_id,
                worker_name,
            } => InstanceIdentifierNode::WitWorker {
                variable_id,
                worker_name: worker_name.map(|wn| {
                    let (mut sub_arena, mut sub_types, sub_root) = crate::expr_arena::lower(&wn);
                    // Migrate nodes into the target arena
                    let mut id_map = std::collections::HashMap::new();
                    for (old_id, node) in sub_arena.exprs.iter() {
                        let new_id = arena.alloc_expr(node.clone());
                        id_map.insert(old_id, new_id);
                        if let Some(ty) = sub_types.get_opt(old_id) {
                            types.set(new_id, ty.clone());
                        }
                    }
                    *id_map.get(&sub_root).unwrap()
                }),
            },
            InstanceIdentifier::WitResource {
                variable_id,
                worker_name,
                resource_name,
            } => InstanceIdentifierNode::WitResource {
                variable_id,
                worker_name: worker_name.map(|wn| {
                    let (mut sub_arena, mut sub_types, sub_root) = crate::expr_arena::lower(&wn);
                    let mut id_map = std::collections::HashMap::new();
                    for (old_id, node) in sub_arena.exprs.iter() {
                        let new_id = arena.alloc_expr(node.clone());
                        id_map.insert(old_id, new_id);
                        if let Some(ty) = sub_types.get_opt(old_id) {
                            types.set(new_id, ty.clone());
                        }
                    }
                    *id_map.get(&sub_root).unwrap()
                }),
                resource_name,
            },
        }
    }
}

// This phase is responsible for identifying the worker function invocations
// such as `worker.foo("x, y, z")` or `cart-resource.add-item(..)` etc
// lazy method invocations are converted to actual Expr::Call
pub fn infer_worker_function_invokes(expr: &mut Expr) -> Result<(), RibTypeErrorInternal> {
    let mut queue = VecDeque::new();
    queue.push_back(expr);

    while let Some(expr) = queue.pop_back() {
        if let Expr::SelectField {
            expr: lhs,
            field,
            source_span,
            ..
        } = expr
        {
            let lhs_inferred_type = lhs.inferred_type().internal_type().clone();

            if let TypeInternal::Instance { instance_type } = lhs_inferred_type {
                let (component, function) =
                    instance_type.get_function(field, None).map_err(|err| {
                        FunctionCallError::invalid_function_call(field, source_span.clone(), err)
                    })?;

                let module = get_instance_identifier(&instance_type, lhs);

                lhs.inferred_type_mut()
                    .internal_type_mut()
                    .narrow_to_single_component(&component);

                if let FunctionName::ResourceConstructor(fully_qualified_resource_constructor) =
                    function.function_name
                {
                    let (resource_id, resource_mode) =
                        match function.function_type.return_type {
                            Some(return_type) => match return_type.internal_type() {
                                TypeInternal::Resource {
                                    resource_id,
                                    resource_mode,
                                    ..
                                } => (*resource_id, *resource_mode),
                                _ => return Err(RibTypeErrorInternal::from(CustomError::new(
                                    expr.source_span(),
                                    "expected resource type as return type of resource constructor",
                                ))),
                            },

                            None => {
                                return Err(RibTypeErrorInternal::from(CustomError::new(
                                    expr.source_span(),
                                    "resource constructor must have a return type",
                                )));
                            }
                        };

                    let resource_instance_type = instance_type
                        .get_resource_instance_type(
                            fully_qualified_resource_constructor.clone(),
                            vec![],
                            instance_type.worker_name(),
                            resource_id,
                            resource_mode,
                        )
                        .map_err(|err| {
                            RibTypeErrorInternal::from(CustomError::new(
                                lhs.source_span(),
                                format!("Failed to get resource instance type: {err}"),
                            ))
                        })?;

                    let new_inferred_type = InferredType::new(
                        TypeInternal::Instance {
                            instance_type: Box::new(resource_instance_type),
                        },
                        TypeOrigin::NoOrigin,
                    );

                    let new_call_type =
                        CallType::InstanceCreation(InstanceCreationType::WitResource {
                            component_info: Some(component.clone()),
                            module: Some(module),
                            resource_name: fully_qualified_resource_constructor.clone(),
                        });

                    *expr = Expr::call(new_call_type, None, vec![])
                        .with_inferred_type(new_inferred_type)
                        .with_source_span(source_span.clone());
                }
            }
        } else if let Expr::InvokeMethodLazy {
            lhs,
            method,
            generic_type_parameter,
            args,
            source_span,
            ..
        } = expr
        {
            match lhs.inferred_type().internal_type() {
                TypeInternal::Instance { instance_type } => {
                    let type_parameter = generic_type_parameter
                        .as_ref()
                        .map(|gtp| {
                            TypeParameter::from_text(&gtp.value).map_err(|err| {
                                FunctionCallError::invalid_generic_type_parameter(
                                    &gtp.value,
                                    err,
                                    source_span.clone(),
                                )
                            })
                        })
                        .transpose()?;

                    // This can be made optional component info to improve type inference
                    // with multiple possibilities of functions but complicates quite a bit
                    let (component, function) = instance_type
                        .get_function(method, type_parameter)
                        .map_err(|err| {
                            FunctionCallError::invalid_function_call(
                                method,
                                source_span.clone(),
                                err,
                            )
                        })?;

                    let module = get_instance_identifier(instance_type, lhs);

                    // We can narrow down the instance type to a single component
                    lhs.inferred_type_mut()
                        .internal_type_mut()
                        .narrow_to_single_component(&component);

                    match function.function_name {
                        // TODO; verify if this assumption is true
                        // that user never need to call a variant function from an instance
                        // If we need to support instance.variant-name(),
                        // this needs to be implemented
                        FunctionName::Variant(_) => {}
                        FunctionName::Enum(_) => {}

                        FunctionName::Function(function_name) => {
                            let dynamic_parsed_function_name = function_name.to_string();
                            let dynamic_parsed_function_name = DynamicParsedFunctionName::parse(
                                dynamic_parsed_function_name.as_str(),
                            )
                            .map_err(|err| {
                                FunctionCallError::invalid_function_call(
                                    &dynamic_parsed_function_name,
                                    source_span.clone(),
                                    format!("invalid function name: {dynamic_parsed_function_name} {err}"),
                                )
                            })?;

                            let new_call = Expr::call_worker_function(
                                dynamic_parsed_function_name,
                                None,
                                Some(module),
                                args.clone(),
                                Some(component),
                            )
                            .with_source_span(source_span.clone());
                            *expr = new_call;
                        }

                        FunctionName::ResourceConstructor(fully_qualified_resource_constructor) => {
                            let (resource_id, resource_mode) = match function.function_type.return_type {
                                Some(return_type) => match return_type.internal_type()  {
                                    TypeInternal::Resource {resource_id, resource_mode, ..} =>  {
                                        (*resource_id, *resource_mode)
                                    }
                                    _ => return Err(RibTypeErrorInternal::from(CustomError::new(expr.source_span(), "expected resource type as return type of resource constructor"))),
                                }

                                None => {
                                    return Err(RibTypeErrorInternal::from(CustomError::new(
                                        expr.source_span(),
                                        "resource constructor must have a return type",
                                    )));
                                }
                            };

                            let resource_instance_type = instance_type
                                .get_resource_instance_type(
                                    fully_qualified_resource_constructor.clone(),
                                    args.clone(),
                                    instance_type.worker_name(),
                                    resource_id,
                                    resource_mode,
                                )
                                .map_err(|err| {
                                    RibTypeErrorInternal::from(CustomError::new(
                                        lhs.source_span(),
                                        format!("Failed to get resource instance type: {err}"),
                                    ))
                                })?;

                            let new_inferred_type = InferredType::new(
                                TypeInternal::Instance {
                                    instance_type: Box::new(resource_instance_type),
                                },
                                TypeOrigin::NoOrigin,
                            );

                            let new_call_type =
                                CallType::InstanceCreation(InstanceCreationType::WitResource {
                                    component_info: Some(component.clone()),
                                    module: Some(module),
                                    resource_name: fully_qualified_resource_constructor.clone(),
                                });

                            *expr = Expr::call(new_call_type, None, args.clone())
                                .with_inferred_type(new_inferred_type)
                                .with_source_span(source_span.clone());
                        }

                        // If resource method is called, we could convert to strict call
                        // however it can only be possible if the instance type of LHS is
                        // a resource type
                        FunctionName::ResourceMethod(resource_method) => {
                            let resource_method_dictionary =
                                instance_type.resource_method_dictionary();

                            let resource_method_info =
                                resource_method_dictionary.get(&FunctionName::ResourceMethod(resource_method.clone()))
                                    .ok_or(FunctionCallError::invalid_function_call(
                                        resource_method.method_name(),
                                        source_span.clone(),
                                    format!(
                                        "Resource method {} not found in resource method dictionary",
                                        resource_method.method_name()
                                    ),
                                ))?;

                            let resource_method_return_type = resource_method_info
                                .return_type
                                .clone()
                                .unwrap_or(InferredType::unknown());

                            let new_inferred_type = match resource_method_return_type
                                .internal_type()
                            {
                                TypeInternal::Resource {
                                    resource_id,
                                    resource_mode,
                                    ..
                                } => {
                                    let resource_instance_type = instance_type
                                        .get_resource_instance_type(
                                            FullyQualifiedResourceConstructor {
                                                package_name: resource_method.package_name.clone(),
                                                interface_name: resource_method
                                                    .interface_name
                                                    .clone(),
                                                resource_name: "cart".to_string(),
                                            },
                                            args.clone(),
                                            instance_type.worker_name(),
                                            *resource_id,
                                            *resource_mode,
                                        )
                                        .map_err(|err| {
                                            RibTypeErrorInternal::from(CustomError::new(
                                                lhs.source_span(),
                                                format!(
                                                    "Failed to get resource instance type: {err}"
                                                ),
                                            ))
                                        })?;

                                    InferredType::new(
                                        TypeInternal::Instance {
                                            instance_type: Box::new(resource_instance_type),
                                        },
                                        TypeOrigin::NoOrigin,
                                    )
                                }

                                _ => InferredType::unknown(),
                            };

                            let dynamic_parsed_function_name = resource_method
                                .dynamic_parsed_function_name()
                                .map_err(|err| {
                                    FunctionCallError::invalid_function_call(
                                        resource_method.method_name(),
                                        source_span.clone(),
                                        format!("Invalid resource method name:  {err}"),
                                    )
                                })?;

                            let new_call = Expr::call_worker_function(
                                dynamic_parsed_function_name,
                                None,
                                Some(module),
                                args.clone(),
                                Some(component),
                            )
                            .with_inferred_type(new_inferred_type)
                            .with_source_span(source_span.clone());

                            *expr = new_call;
                        }
                    }
                }
                // This implies, none of the phase identified `lhs` to be an instance-type yet.
                // Re-running (fix point) the same phase will help identify the instance type of `lhs`.
                // Hence, this phase is part of computing the fix-point of compiler type inference.
                TypeInternal::Unknown => {}
                _ => {
                    return Err(FunctionCallError::invalid_function_call(
                        method,
                        source_span.clone(),
                        "invalid worker function invoke. Expected to be an instance type",
                    )
                    .into());
                }
            }
        }

        expr.visit_expr_nodes_lazy(&mut queue);
    }

    Ok(())
}

fn get_instance_identifier(instance_type: &InstanceType, lhs: &mut Expr) -> InstanceIdentifier {
    let variable_id = match lhs {
        Expr::Identifier { variable_id, .. } => Some(variable_id),
        _ => None,
    };

    match instance_type {
        InstanceType::Resource {
            worker_name,
            resource_constructor,
            ..
        } => InstanceIdentifier::WitResource {
            variable_id: variable_id.cloned(),
            worker_name: worker_name.clone(),
            resource_name: resource_constructor.clone(),
        },
        instance_type => InstanceIdentifier::WitWorker {
            variable_id: variable_id.cloned(),
            worker_name: instance_type.worker_name(),
        },
    }
}
