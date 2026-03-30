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

use crate::expr_arena::{
    rebuild_expr, CallTypeNode, ExprArena, ExprId, ExprKind, InstanceCreationNode,
    InstanceIdentifierNode, TypeTable,
};
use crate::instance_type::InstanceType;
use crate::type_inference::expr_visitor::arena::children_of;
use crate::{InferredType, TypeInternal, TypeOrigin, VariableId};
use std::collections::HashMap;

/// Copies the worker-name expression from the arena-backed [`CallTypeNode`]
/// into [`InstanceType::worker_name`] for every `Call` whose type is
/// [`TypeInternal::Instance`].
///
/// [`InstanceType`] stores `Option<Box<Expr>>` for worker names; during
/// inference those can diverge from the canonical subtree referenced by
/// [`ExprId`] in the call node (for example after
/// [`super::stateful_instance::ensure_stateful_instance`]).
/// This pass aligns `TypeTable` with what [`rebuild_expr`] will produce.
pub fn sync_embedded_worker_exprs_from_calls(
    root: ExprId,
    arena: &ExprArena,
    types: &mut TypeTable,
) {
    let mut order = Vec::new();
    collect_pre_order_instance(root, arena, &mut order);

    for id in order {
        if !matches!(types.get(id).internal_type(), TypeInternal::Instance { .. }) {
            continue;
        }
        let kind = arena.expr(id).kind.clone();
        let ExprKind::Call { call_type, .. } = kind else {
            continue;
        };
        let Some(wn_id) = worker_name_expr_id_from_call_node(&call_type) else {
            continue;
        };
        let worker_expr = rebuild_expr(wn_id, arena, types);
        let mut updated = types.get(id).clone();
        if let TypeInternal::Instance { instance_type } = updated.internal_type_mut() {
            instance_type.set_worker_name(worker_expr);
        }
        types.set(id, updated);
    }
}

fn worker_name_expr_id_from_call_node(ct: &CallTypeNode) -> Option<ExprId> {
    match ct {
        CallTypeNode::InstanceCreation(InstanceCreationNode::WitWorker {
            worker_name: Some(id),
            ..
        }) => Some(*id),
        CallTypeNode::Function {
            instance_identifier:
                Some(InstanceIdentifierNode::WitWorker {
                    worker_name: Some(id),
                    ..
                }),
            ..
        }
        | CallTypeNode::Function {
            instance_identifier:
                Some(InstanceIdentifierNode::WitResource {
                    worker_name: Some(id),
                    ..
                }),
            ..
        } => Some(*id),
        CallTypeNode::Function {
            instance_identifier: Some(_),
            ..
        } => None,
        _ => None,
    }
}

/// Propagates instance types from `let` rhs to bound identifiers. Use from the same
/// `lower` / `rebuild_expr` boundary as [`crate::Expr::infer_types`].
pub fn bind_instance_types_lowered(root: ExprId, arena: &ExprArena, types: &mut TypeTable) {
    let mut instance_variables: HashMap<VariableId, Box<InstanceType>> = HashMap::new();

    let mut order = Vec::new();
    collect_pre_order_instance(root, arena, &mut order);

    for id in order {
        let kind = arena.expr(id).kind.clone();
        match kind {
            ExprKind::Let {
                variable_id,
                expr: rhs_id,
            } => {
                let rhs_type = types.get(rhs_id).clone();
                if let TypeInternal::Instance { instance_type } = rhs_type.internal_type() {
                    instance_variables.insert(variable_id, instance_type.clone());
                }
            }
            ExprKind::Identifier { variable_id } => {
                if let Some(instance_type) = instance_variables.get(&variable_id) {
                    types.set(
                        id,
                        InferredType::new(
                            TypeInternal::Instance {
                                instance_type: instance_type.clone(),
                            },
                            TypeOrigin::NoOrigin,
                        ),
                    );
                }
            }
            _ => {}
        }
    }
}

fn collect_pre_order_instance(root: ExprId, arena: &ExprArena, out: &mut Vec<ExprId>) {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        out.push(id);
        for child in children_of(id, arena).into_iter().rev() {
            stack.push(child);
        }
    }
}
