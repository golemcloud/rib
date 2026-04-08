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

use std::fmt::Display;

use crate::analysis::AnalysedType;
use crate::call_type::CallType;
use crate::expr_arena::{
    CallTypeNode, ExprArena, ExprId, ExprKind, InstanceCreationNode, TypeTable,
};
use crate::inferred_type::TypeOrigin;
use crate::type_inference::expr_visitor::arena::children_of;
use crate::{
    ComponentDependency, CustomInstanceSpec, DynamicParsedFunctionName,
    FullyQualifiedResourceMethod, FunctionCallError, FunctionName, InferredType, TypeInternal,
};

#[derive(Clone)]
pub enum FunctionDetails {
    ResourceConstructorName {
        resource_constructor_name: String,
    },
    ResourceMethodName {
        resource_name: String,
        resource_method_name: String,
    },
    Fqn(DynamicParsedFunctionName),
    VariantName(String),
}

impl FunctionDetails {
    pub fn name(&self) -> String {
        match self {
            FunctionDetails::ResourceConstructorName {
                resource_constructor_name,
            } => resource_constructor_name.replace("[constructor]", ""),
            FunctionDetails::ResourceMethodName {
                resource_name,
                resource_method_name,
            } => {
                let resource_constructor_prefix = format!("[method]{resource_name}.");
                resource_method_name.replace(&resource_constructor_prefix, "")
            }
            FunctionDetails::Fqn(fqn) => fqn.function.name_pretty(),
            FunctionDetails::VariantName(name) => name.clone(),
        }
    }
}

impl Display for FunctionDetails {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FunctionDetails::ResourceConstructorName {
                resource_constructor_name,
                ..
            } => {
                write!(f, "{resource_constructor_name}")
            }
            FunctionDetails::ResourceMethodName {
                resource_method_name,
                ..
            } => {
                write!(f, "{resource_method_name}")
            }
            FunctionDetails::Fqn(fqn) => {
                write!(f, "{fqn}")
            }
            FunctionDetails::VariantName(name) => {
                write!(f, "{name}")
            }
        }
    }
}

/// Arena-based function-call argument typing (used from [`crate::expr_arena`] inference).
pub fn infer_function_call_types_lowered(
    root: ExprId,
    arena: &ExprArena,
    types: &mut TypeTable,
    component_dependency: &ComponentDependency,
    custom_instance_spec: &[CustomInstanceSpec],
) -> Result<(), FunctionCallError> {
    let mut stack = vec![root];

    while let Some(id) = stack.pop() {
        let node = arena.expr(id);
        let kind = node.kind.clone();
        let span = node.source_span.clone();

        if let ExprKind::Call {
            ref call_type,
            ref args,
            ..
        } = kind
        {
            let args_ids: Vec<ExprId> = args.clone();
            resolve_call_argument_types_arena(
                id,
                call_type,
                &args_ids,
                &span,
                arena,
                types,
                component_dependency,
                custom_instance_spec,
            )?;
        } else {
            for child in children_of(id, arena).into_iter().rev() {
                stack.push(child);
            }
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn resolve_call_argument_types_arena(
    call_id: ExprId,
    call_type: &CallTypeNode,
    args: &[ExprId],
    span: &crate::rib_source_span::SourceSpan,
    arena: &ExprArena,
    types: &mut TypeTable,
    component_dependency: &ComponentDependency,
    custom_instance_spec: &[CustomInstanceSpec],
) -> Result<(), FunctionCallError> {
    match call_type {
        CallTypeNode::InstanceCreation(creation) => match creation {
            InstanceCreationNode::WitWorker { .. } => {
                if custom_instance_spec.is_empty() {
                    for &arg_id in args {
                        if types.get(arg_id).is_unknown() {
                            merge_into(arg_id, InferredType::string(), types);
                        }
                    }
                }
                Ok(())
            }
            InstanceCreationNode::WitResource { resource_name, .. } => {
                let fn_name = FunctionName::ResourceConstructor(resource_name.clone());
                infer_args_and_result_type_arena(
                    span,
                    &FunctionDetails::ResourceConstructorName {
                        resource_constructor_name: resource_name.resource_name.clone(),
                    },
                    component_dependency,
                    &fn_name,
                    args,
                    None,
                    arena,
                    types,
                )
            }
        },

        CallTypeNode::Function { function_name, .. } => {
            use crate::FunctionName as FN;
            let fn_name0 = FN::from_dynamic_parsed_function_name(function_name);
            match fn_name0 {
                FN::ResourceMethod(fqrm) => infer_resource_method_args_arena(
                    span,
                    &fqrm,
                    function_name,
                    component_dependency,
                    args,
                    call_id,
                    arena,
                    types,
                ),
                _ => {
                    // Reconstruct the CallType for key lookup
                    let key = {
                        let ct = CallType::Function {
                            component_info: None,
                            instance_identifier: None,
                            function_name: function_name.clone(),
                        };
                        FunctionName::from_call_type(&ct).ok_or(
                            FunctionCallError::InvalidFunctionCall {
                                function_name: function_name.to_string(),
                                source_span: span.clone(),
                                message: "unknown function".to_string(),
                            },
                        )?
                    };
                    infer_args_and_result_type_arena(
                        span,
                        &FunctionDetails::Fqn(function_name.clone()),
                        component_dependency,
                        &key,
                        args,
                        Some(call_id),
                        arena,
                        types,
                    )
                }
            }
        }

        CallTypeNode::EnumConstructor(name) => {
            if args.is_empty() {
                Ok(())
            } else {
                Err(FunctionCallError::ArgumentSizeMisMatch {
                    function_name: name.clone(),
                    source_span: span.clone(),
                    expected: 0,
                    provided: args.len(),
                })
            }
        }

        CallTypeNode::VariantConstructor(variant_name) => {
            let fn_name = FunctionName::Variant(variant_name.clone());
            infer_args_and_result_type_arena(
                span,
                &FunctionDetails::VariantName(variant_name.clone()),
                component_dependency,
                &fn_name,
                args,
                Some(call_id),
                arena,
                types,
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn infer_resource_method_args_arena(
    span: &crate::rib_source_span::SourceSpan,
    fqrm: &FullyQualifiedResourceMethod,
    function_name: &crate::DynamicParsedFunctionName,
    component_dependency: &ComponentDependency,
    args: &[ExprId],
    call_id: ExprId,
    arena: &ExprArena,
    types: &mut TypeTable,
) -> Result<(), FunctionCallError> {
    let resource_constructor_name = function_name.resource_name_simplified().unwrap_or_default();
    let resource_method = fqrm.method_name.clone();
    infer_args_and_result_type_arena(
        span,
        &FunctionDetails::ResourceMethodName {
            resource_name: resource_constructor_name,
            resource_method_name: resource_method,
        },
        component_dependency,
        &FunctionName::ResourceMethod(fqrm.clone()),
        args,
        Some(call_id),
        arena,
        types,
    )
}

#[allow(clippy::too_many_arguments)]
fn infer_args_and_result_type_arena(
    span: &crate::rib_source_span::SourceSpan,
    function_details: &FunctionDetails,
    component_dependency: &ComponentDependency,
    key: &FunctionName,
    args: &[ExprId],
    result_id: Option<ExprId>,
    arena: &ExprArena,
    types: &mut TypeTable,
) -> Result<(), FunctionCallError> {
    let (_, function_type) = component_dependency
        .get_function_type(key)
        .map_err(|err| FunctionCallError::InvalidFunctionCall {
            function_name: function_details.to_string(),
            source_span: span.clone(),
            message: err.to_string(),
        })?;

    let mut parameter_types: Vec<AnalysedType> = function_type
        .parameter_types
        .iter()
        .map(|t| AnalysedType::try_from(t).unwrap())
        .collect();

    match key {
        FunctionName::Variant(_) => {
            let result_type =
                function_type
                    .as_type_variant()
                    .ok_or(FunctionCallError::InvalidFunctionCall {
                        function_name: function_details.to_string(),
                        source_span: span.clone(),
                        message: "expected a variant type".to_string(),
                    })?;

            if parameter_types.len() == args.len() {
                tag_argument_types_arena(function_details, args, &parameter_types, arena, types)?;
                if let Some(rid) = result_id {
                    types.set(rid, InferredType::from_type_variant(&result_type));
                }
                Ok(())
            } else {
                Err(FunctionCallError::ArgumentSizeMisMatch {
                    function_name: function_details.name(),
                    source_span: span.clone(),
                    expected: parameter_types.len(),
                    provided: args.len(),
                })
            }
        }

        FunctionName::Enum(_) => Ok(()),

        FunctionName::ResourceConstructor(_) | FunctionName::Function(_) => {
            if parameter_types.len() == args.len() {
                let result_type = function_type.return_type.clone();
                tag_argument_types_arena(function_details, args, &parameter_types, arena, types)?;
                if let Some(rid) = result_id {
                    let rt = result_type.unwrap_or_else(|| InferredType::sequence(vec![]));
                    types.set(rid, rt);
                }
                Ok(())
            } else {
                Err(FunctionCallError::ArgumentSizeMisMatch {
                    function_name: function_details.name(),
                    source_span: span.clone(),
                    expected: parameter_types.len(),
                    provided: args.len(),
                })
            }
        }

        FunctionName::ResourceMethod(_) => {
            if let Some(AnalysedType::Handle(_)) = parameter_types.first() {
                parameter_types.remove(0);
            }
            let return_type = function_type.return_type.clone();
            if parameter_types.len() == args.len() {
                tag_argument_types_arena(function_details, args, &parameter_types, arena, types)?;
                if let Some(rid) = result_id {
                    let rt = return_type.unwrap_or_else(|| InferredType::sequence(vec![]));
                    types.set(rid, rt);
                }
                Ok(())
            } else {
                Err(FunctionCallError::ArgumentSizeMisMatch {
                    function_name: function_details.name(),
                    source_span: span.clone(),
                    expected: parameter_types.len(),
                    provided: args.len(),
                })
            }
        }
    }
}

fn tag_argument_types_arena(
    function_details: &FunctionDetails,
    args: &[ExprId],
    parameter_types: &[AnalysedType],
    arena: &ExprArena,
    types: &mut TypeTable,
) -> Result<(), FunctionCallError> {
    for (&arg_id, param_type) in args.iter().zip(parameter_types) {
        // Variant conflict workaround: update Variant TypeInternal in place
        if let AnalysedType::Variant(type_variant) = param_type {
            let current = types.get(arg_id).clone();
            let mut current_inner = current.inner.as_ref().clone();
            if let TypeInternal::Variant(ref mut collections) = current_inner {
                *collections = type_variant
                    .cases
                    .iter()
                    .map(|case| (case.name.clone(), case.typ.as_ref().map(InferredType::from)))
                    .collect();
                let updated = InferredType::new(current_inner, current.origin.clone());
                types.set(arg_id, updated);
            }
        }

        // Check argument validity
        let arg_type = types.get(arg_id).clone();
        let arg_span = arena.expr(arg_id).source_span.clone();
        let is_valid = if arg_type.is_unknown() || arg_type.is_all_of() {
            true
        } else {
            use crate::type_inference::GetTypeHint;
            arg_type.get_type_hint().get_type_kind() == param_type.get_type_hint().get_type_kind()
        };

        if !is_valid {
            use crate::{ActualType, ExpectedType, TypeMismatchError};
            return Err(FunctionCallError::TypeMisMatch {
                function_name: function_details.name(),
                argument_source_span: arg_span.clone(),
                error: TypeMismatchError {
                    source_span: arg_span,
                    expected_type: ExpectedType::AnalysedType(param_type.clone()),
                    actual_type: ActualType::Inferred(arg_type),
                    field_path: Default::default(),
                    additional_error_detail: vec![],
                },
            });
        }

        // Tag the argument with the declared parameter type
        let current = types.get(arg_id).clone();
        let param_inferred = InferredType::from(param_type)
            .add_origin(TypeOrigin::Declared(arena.expr(arg_id).source_span.clone()));
        types.set(arg_id, current.merge(param_inferred));
    }
    Ok(())
}

fn merge_into(id: ExprId, ty: InferredType, types: &mut TypeTable) {
    let current = types.get(id).clone();
    types.set(id, current.merge(ty));
}

#[cfg(test)]
mod function_parameters_inference_tests {
    use test_r::test;

    use crate::analysis::{
        AnalysedExport, AnalysedFunction, AnalysedFunctionParameter, AnalysedType, TypeU32, TypeU64,
    };
    use crate::function_name::{DynamicParsedFunctionName, DynamicParsedFunctionReference};
    use crate::rib_source_span::SourceSpan;
    use crate::{
        ComponentDependency, ComponentDependencyKey, CustomInstanceSpec, Expr, FunctionCallError,
        InferredType, ParsedFunctionSite,
    };
    use bigdecimal::BigDecimal;
    use uuid::Uuid;

    fn infer_function_call_types_via_arena(
        expr: &mut Expr,
        component_dependency: &ComponentDependency,
        custom_instance_spec: &[CustomInstanceSpec],
    ) -> Result<(), FunctionCallError> {
        let (arena, mut types, root) = crate::expr_arena::lower(expr);
        super::infer_function_call_types_lowered(
            root,
            &arena,
            &mut types,
            component_dependency,
            custom_instance_spec,
        )?;
        *expr = crate::expr_arena::rebuild_expr(root, &arena, &types);
        Ok(())
    }

    fn get_component_dependency() -> ComponentDependency {
        let metadata = vec![
            AnalysedExport::Function(AnalysedFunction {
                name: "foo".to_string(),
                parameters: vec![AnalysedFunctionParameter {
                    name: "my_parameter".to_string(),
                    typ: AnalysedType::U64(TypeU64),
                }],
                result: None,
            }),
            AnalysedExport::Function(AnalysedFunction {
                name: "baz".to_string(),
                parameters: vec![AnalysedFunctionParameter {
                    name: "my_parameter".to_string(),
                    typ: AnalysedType::U32(TypeU32),
                }],
                result: None,
            }),
        ];

        let component_info = ComponentDependencyKey {
            component_name: "foo".to_string(),
            component_id: Uuid::new_v4(),
            component_revision: 0,
            root_package_name: None,
            root_package_version: None,
        };

        ComponentDependency::from_wit_metadata(component_info, metadata.as_slice())
            .expect("Failed to create component dependencies")
    }

    #[test]
    fn test_infer_function_types() {
        let rib_expr = r#"
          let x = 1;
          foo(x)
        "#;

        let function_type_registry = get_component_dependency();

        let mut expr = Expr::from_text(rib_expr).unwrap();
        infer_function_call_types_via_arena(&mut expr, &function_type_registry, &[]).unwrap();

        let let_binding = Expr::let_binding("x", Expr::number(BigDecimal::from(1)), None);

        let call_expr = Expr::call_worker_function(
            DynamicParsedFunctionName {
                site: ParsedFunctionSite::Global,
                function: DynamicParsedFunctionReference::Function {
                    function: "foo".to_string(),
                },
            },
            None,
            vec![Expr::identifier_global("x", None).with_inferred_type(InferredType::u64())],
            None,
        )
        .with_inferred_type(InferredType::sequence(vec![]));

        let expected = Expr::ExprBlock {
            exprs: vec![let_binding, call_expr],
            inferred_type: InferredType::unknown(),
            source_span: SourceSpan::default(),
            type_annotation: None,
        };

        assert_eq!(expr, expected);
    }
}
