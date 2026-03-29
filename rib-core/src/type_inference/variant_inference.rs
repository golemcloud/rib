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

use crate::{ComponentDependencies, Expr};

pub fn infer_variants(expr: &mut Expr, component_dependency: &ComponentDependencies) {
    let variants = internal::get_variants_info(expr, component_dependency);

    internal::convert_identifiers_to_no_arg_variant_calls(expr, &variants);

    // Initially every call type is dynamic-parsed function name
    internal::convert_function_calls_to_variant_calls(expr, &variants);
}

mod internal {
    use crate::call_type::CallType;
    use crate::{ComponentDependencies, Expr, InferredType};
    use std::collections::VecDeque;

    pub(crate) fn convert_function_calls_to_variant_calls(
        expr: &mut Expr,
        variant_info: &VariantInfo,
    ) {
        let variants = variant_info.variants_with_args.clone();
        let mut queue = VecDeque::new();
        queue.push_back(expr);

        while let Some(expr) = queue.pop_back() {
            match expr {
                Expr::Call {
                    call_type: CallType::Function { function_name, .. },
                    args,
                    inferred_type,
                    ..
                } => {
                    if variants.contains(&function_name.to_string()) {
                        *expr = Expr::call(
                            CallType::VariantConstructor(function_name.to_string()),
                            None,
                            args.clone(),
                        )
                        .with_inferred_type(inferred_type.clone());
                    }
                }
                _ => expr.visit_expr_nodes_lazy(&mut queue),
            }
        }
    }

    pub(crate) fn convert_identifiers_to_no_arg_variant_calls(
        expr: &mut Expr,
        variant_info: &VariantInfo,
    ) {
        let variants = variant_info.no_arg_variants.clone();

        let mut queue = VecDeque::new();
        queue.push_back(expr);

        while let Some(expr) = queue.pop_back() {
            match expr {
                Expr::Identifier {
                    variable_id,
                    inferred_type,
                    ..
                } => {
                    if !variable_id.is_local() && variants.contains(&variable_id.name()) {
                        *expr = Expr::call(
                            CallType::VariantConstructor(variable_id.name()),
                            None,
                            vec![],
                        )
                        .with_inferred_type(inferred_type.clone());
                    }
                }
                _ => expr.visit_expr_nodes_lazy(&mut queue),
            }
        }
    }

    pub(crate) fn get_variants_info(
        expr: &mut Expr,
        component_dependency: &ComponentDependencies,
    ) -> VariantInfo {
        let mut no_arg_variants = vec![];
        let mut variant_with_args = vec![];
        let mut queue = VecDeque::new();
        queue.push_back(expr);

        while let Some(expr) = queue.pop_back() {
            match expr {
                Expr::Identifier {
                    variable_id,
                    inferred_type,
                    ..
                } => {
                    if !variable_id.is_local() {
                        let type_variants_opt = component_dependency
                            .function_dictionary()
                            .iter()
                            .find_map(|x| {
                                let result = x.get_variant_info(variable_id.name().as_str());

                                if result.is_empty() {
                                    None
                                } else {
                                    Some(result)
                                }
                            });

                        if let Some(type_variants) = type_variants_opt {
                            no_arg_variants.push(variable_id.name());

                            let inferred_types = type_variants
                                .iter()
                                .map(InferredType::from_type_variant)
                                .collect::<Vec<_>>();

                            let new_inferred_type = if inferred_types.len() == 1 {
                                inferred_types[0].clone()
                            } else {
                                InferredType::all_of(inferred_types)
                            };

                            *inferred_type = inferred_type.merge(new_inferred_type);
                        }
                    }
                }

                Expr::Call {
                    call_type: CallType::Function { function_name, .. },
                    args,
                    inferred_type,
                    ..
                } => {
                    // Conflicts of having the same variant names across multiple components is not handled
                    let result = component_dependency
                        .function_dictionary()
                        .iter()
                        .find_map(|x| {
                            let type_variants =
                                x.get_variant_info(function_name.to_string().as_str());
                            if type_variants.is_empty() {
                                None
                            } else {
                                Some(type_variants)
                            }
                        });

                    if let Some(result) = result {
                        variant_with_args.push(function_name.to_string());

                        let inferred_types = result
                            .iter()
                            .map(InferredType::from_type_variant)
                            .collect::<Vec<_>>();

                        let new_inferred_type = if inferred_types.len() == 1 {
                            inferred_types[0].clone()
                        } else {
                            InferredType::all_of(inferred_types)
                        };

                        *inferred_type = inferred_type.merge(new_inferred_type);
                    }

                    for expr in args {
                        queue.push_back(expr);
                    }
                }

                _ => expr.visit_expr_nodes_lazy(&mut queue),
            }
        }

        VariantInfo {
            no_arg_variants,
            variants_with_args: variant_with_args,
        }
    }

    #[derive(Debug, Clone)]
    pub(crate) struct VariantInfo {
        no_arg_variants: Vec<String>,
        variants_with_args: Vec<String>,
    }
}

pub mod arena {
    use crate::expr_arena::{CallTypeNode, ExprArena, ExprId, ExprKind, TypeTable};
    use crate::type_inference::expr_visitor::arena::children_of;
    use crate::{ComponentDependencies, InferredType};

    /// Arena version of `infer_variants`.
    pub fn infer_variants(
        root: ExprId,
        arena: &mut ExprArena,
        types: &mut TypeTable,
        component_dependencies: &ComponentDependencies,
    ) {
        let info = collect_variant_info(root, arena, types, component_dependencies);

        // Convert no-arg Identifier nodes -> Call { VariantConstructor }
        convert_identifier_nodes(root, arena, types, &info.no_arg_variants);

        // Convert Call { Function } nodes whose name matches a variant -> Call { VariantConstructor }
        convert_call_nodes(root, arena, &info.variants_with_args);
    }

    struct VariantInfo {
        no_arg_variants: Vec<String>,
        variants_with_args: Vec<String>,
    }

    fn collect_variant_info(
        root: ExprId,
        arena: &ExprArena,
        types: &mut TypeTable,
        component_dependencies: &ComponentDependencies,
    ) -> VariantInfo {
        let mut no_arg_variants = Vec::new();
        let mut variants_with_args = Vec::new();
        let mut stack = vec![root];

        while let Some(id) = stack.pop() {
            let node = arena.expr(id);
            match &node.kind {
                ExprKind::Identifier { variable_id } if !variable_id.is_local() => {
                    let type_variants_opt = component_dependencies
                        .function_dictionary()
                        .iter()
                        .find_map(|x| {
                            let r = x.get_variant_info(variable_id.name().as_str());
                            if r.is_empty() {
                                None
                            } else {
                                Some(r)
                            }
                        });

                    if let Some(type_variants) = type_variants_opt {
                        no_arg_variants.push(variable_id.name());
                        let new_type = merge_variant_types(&type_variants);
                        let current = types.get(id).clone();
                        types.set(id, current.merge(new_type));
                    }
                    for child in children_of(id, arena).into_iter().rev() {
                        stack.push(child);
                    }
                }
                ExprKind::Call {
                    call_type: CallTypeNode::Function { function_name, .. },
                    args,
                    ..
                } => {
                    let fn_str = function_name.to_string();
                    let result = component_dependencies
                        .function_dictionary()
                        .iter()
                        .find_map(|x| {
                            let r = x.get_variant_info(fn_str.as_str());
                            if r.is_empty() {
                                None
                            } else {
                                Some(r)
                            }
                        });

                    if let Some(type_variants) = result {
                        variants_with_args.push(fn_str);
                        let new_type = merge_variant_types(&type_variants);
                        let current = types.get(id).clone();
                        types.set(id, current.merge(new_type));
                    }
                    // push args to continue traversal
                    let args: Vec<ExprId> = args.clone();
                    for arg in args.into_iter().rev() {
                        stack.push(arg);
                    }
                }
                _ => {
                    for child in children_of(id, arena).into_iter().rev() {
                        stack.push(child);
                    }
                }
            }
        }

        VariantInfo {
            no_arg_variants,
            variants_with_args,
        }
    }

    fn convert_identifier_nodes(
        root: ExprId,
        arena: &mut ExprArena,
        types: &TypeTable,
        no_arg_variants: &[String],
    ) {
        let mut ids_to_convert = Vec::new();
        let mut stack = vec![root];

        while let Some(id) = stack.pop() {
            {
                let node = arena.expr(id);
                if let ExprKind::Identifier { variable_id } = &node.kind {
                    if !variable_id.is_local() && no_arg_variants.contains(&variable_id.name()) {
                        ids_to_convert.push((id, variable_id.name()));
                    }
                }
            }
            for child in children_of(id, arena).into_iter().rev() {
                stack.push(child);
            }
        }

        for (id, name) in ids_to_convert {
            let node = arena.expr_mut(id);
            node.kind = ExprKind::Call {
                call_type: CallTypeNode::VariantConstructor(name),
                generic_type_parameter: None,
                args: vec![],
            };
        }
    }

    fn convert_call_nodes(root: ExprId, arena: &mut ExprArena, variants_with_args: &[String]) {
        let mut ids_to_convert = Vec::new();
        let mut stack = vec![root];

        while let Some(id) = stack.pop() {
            {
                let node = arena.expr(id);
                if let ExprKind::Call {
                    call_type: CallTypeNode::Function { function_name, .. },
                    args,
                    ..
                } = &node.kind
                {
                    let fn_str = function_name.to_string();
                    if variants_with_args.contains(&fn_str) {
                        let args_clone: Vec<ExprId> = args.clone();
                        ids_to_convert.push((id, fn_str, args_clone));
                    }
                }
            }
            for child in children_of(id, arena).into_iter().rev() {
                stack.push(child);
            }
        }

        for (id, name, args) in ids_to_convert {
            let node = arena.expr_mut(id);
            node.kind = ExprKind::Call {
                call_type: CallTypeNode::VariantConstructor(name),
                generic_type_parameter: None,
                args,
            };
        }
    }

    fn merge_variant_types(type_variants: &[crate::analysis::TypeVariant]) -> InferredType {
        let inferred: Vec<InferredType> = type_variants
            .iter()
            .map(InferredType::from_type_variant)
            .collect();
        if inferred.len() == 1 {
            inferred.into_iter().next().unwrap()
        } else {
            InferredType::all_of(inferred)
        }
    }
}
