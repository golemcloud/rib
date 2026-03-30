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
    CallTypeNode, ExprArena, ExprId, ExprKind, InstanceCreationNode, TypeTable,
};
use crate::type_inference::expr_visitor::arena::children_of;
use crate::{visit_post_order_rev_mut, CallType, Expr, InstanceCreationType, InferredType, TypeInternal,
            TypeOrigin};

pub fn ensure_stateful_instance_lowered(
    root: ExprId,
    arena: &mut ExprArena,
    types: &mut TypeTable,
) {
    let mut ids_to_patch = Vec::new();

    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let kind = arena.expr(id).kind.clone();
        if let ExprKind::Call {
            call_type:
                CallTypeNode::InstanceCreation(InstanceCreationNode::WitWorker {
                    worker_name: None,
                    ..
                }),
            ..
        } = kind
        {
            ids_to_patch.push(id);
        } else {
            for child in children_of(id, arena).into_iter().rev() {
                stack.push(child);
            }
        }
    }

    for id in ids_to_patch {
        let gen_worker_node = crate::expr_arena::ExprNode {
            kind: ExprKind::GenerateWorkerName { variable_id: None },
            source_span: crate::rib_source_span::SourceSpan::default(),
            type_annotation: None,
        };
        let gen_id = arena.alloc_expr(gen_worker_node);
        types.set(gen_id, InferredType::string());

        let call_kind = arena.expr(id).kind.clone();
        if let ExprKind::Call {
            call_type:
                CallTypeNode::InstanceCreation(InstanceCreationNode::WitWorker {
                    ref component_info,
                    ..
                }),
            args: _,
            ..
        } = call_kind
        {
            let ci = component_info.clone();
            let new_args = vec![gen_id];

            let node_mut = arena.expr_mut(id);
            node_mut.kind = ExprKind::Call {
                call_type: CallTypeNode::InstanceCreation(InstanceCreationNode::WitWorker {
                    component_info: ci,
                    worker_name: Some(gen_id),
                }),
                generic_type_parameter: None,
                args: new_args,
            };

            let current_type = types.get(id).clone();
            if let TypeInternal::Instance { mut instance_type } = current_type.internal_type().clone()
            {
                instance_type.set_worker_name(crate::Expr::generate_worker_name(None));
                let new_type = InferredType::new(
                    TypeInternal::Instance {
                        instance_type: Box::new(*instance_type),
                    },
                    TypeOrigin::NoOrigin,
                );
                types.set(id, new_type);
            }
        }
    }
}

pub fn ensure_stateful_instance(expr: &mut Expr) {
    visit_post_order_rev_mut(expr, &mut |expr| {
        if let Expr::Call {
            call_type:
                CallType::InstanceCreation(InstanceCreationType::WitWorker { worker_name, .. }),
            inferred_type,
            args,
            ..
        } = expr
        {
            if worker_name.is_none() {
                *worker_name = Some(Box::new(Expr::generate_worker_name(None)));

                let type_internal = &mut *inferred_type.inner;

                *args = vec![Expr::generate_worker_name(None)];

                if let TypeInternal::Instance { instance_type } = type_internal {
                    instance_type.set_worker_name(Expr::generate_worker_name(None))
                }
            }
        }
    });
}
