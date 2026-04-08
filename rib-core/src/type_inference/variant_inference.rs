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

use crate::expr_arena::{CallTypeNode, ExprArena, ExprId, ExprKind, TypeTable};
use crate::type_inference::expr_visitor::arena::children_of;
use crate::{ComponentDependency, InferredType};

pub fn infer_variants_lowered(
    root: ExprId,
    arena: &mut ExprArena,
    types: &mut TypeTable,
    component_dependencies: &ComponentDependency,
) {
    let info = collect_variant_info(root, arena, types, component_dependencies);

    convert_identifier_nodes(root, arena, types, &info.no_arg_variants);

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
    component_dependencies: &ComponentDependency,
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
    _types: &TypeTable,
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
            args,
        };
    }
}

fn merge_variant_types(type_variants: &[crate::wit_type::TypeVariant]) -> InferredType {
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
