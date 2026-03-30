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
use crate::expr_arena::{CallTypeNode, ExprArena, ExprId, ExprKind, TypeTable};
use crate::type_inference::expr_visitor::arena::children_of;
use crate::{ComponentDependencies, Expr};

pub fn infer_enums(expr: &mut Expr, component_dependencies: &ComponentDependencies) {
    let (mut expr_arena, mut types, root) = crate::expr_arena::lower(expr);
    infer_enums_lowered(
        root,
        &mut expr_arena,
        &mut types,
        component_dependencies,
    );
    *expr = crate::expr_arena::rebuild_expr(root, &expr_arena, &types);
}

pub fn infer_enums_lowered(
    root: ExprId,
    arena: &mut ExprArena,
    types: &mut TypeTable,
    component_dependencies: &ComponentDependencies,
) {
    let enum_ids = collect_enum_identifiers(root, arena, types, component_dependencies);

    for id in enum_ids {
        let node = arena.expr(id);
        if let ExprKind::Identifier { variable_id } = &node.kind {
            let name = variable_id.name();
            let annotation = node.type_annotation.clone();
            let span = node.source_span.clone();
            let node_mut = arena.expr_mut(id);
            node_mut.kind = ExprKind::Call {
                call_type: CallTypeNode::EnumConstructor(name),
                generic_type_parameter: None,
                args: vec![],
            };
            node_mut.type_annotation = annotation;
            node_mut.source_span = span;
        }
    }
}

fn collect_enum_identifiers(
    root: ExprId,
    arena: &ExprArena,
    types: &mut TypeTable,
    component_dependencies: &ComponentDependencies,
) -> Vec<ExprId> {
    let mut enum_ids = Vec::new();
    let mut stack = vec![root];

    while let Some(id) = stack.pop() {
        let node = arena.expr(id);
        if let ExprKind::Identifier { variable_id } = &node.kind {
            if !variable_id.is_local() {
                let result = component_dependencies
                    .function_dictionary()
                    .iter()
                    .find_map(|x| x.get_enum_info(variable_id.name().as_str()));

                if let Some(typed_enum) = result {
                    let new_type: crate::InferredType =
                        (&AnalysedType::Enum(typed_enum.clone())).into();
                    let current = types.get(id).clone();
                    types.set(id, current.merge(new_type));
                    enum_ids.push(id);
                }
            }
        }
        for child in children_of(id, arena).into_iter().rev() {
            stack.push(child);
        }
    }

    enum_ids
}
