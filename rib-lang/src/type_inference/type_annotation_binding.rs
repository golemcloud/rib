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

use crate::expr_arena::{ExprArena, ExprId, ExprKind, TypeTable};
use crate::inferred_type::TypeOrigin;
use crate::type_inference::expr_visitor::arena::visit_pre_order_mut as visit_expr_ids_pre_order_mut;
use crate::InferredType;

/// For every node, tags its `InferredType` with `TypeOrigin::OriginatedAt`
/// using the node's source span. This mirrors `Expr::set_origin`.
pub fn set_origin(root: ExprId, arena: &ExprArena, types: &mut TypeTable) {
    visit_expr_ids_pre_order_mut(root, arena, types, &mut |id, arena, types| {
        let current = types.get(id);
        // Only stamp OriginatedAt on nodes that don't already carry a
        // Declared origin from bind_type_annotations. This mirrors the
        // Expr pipeline where set_origin runs first (setting OriginatedAt
        // on all nodes), then bind_type_annotations overwrites annotated
        // nodes with Declared.
        if current.origin.is_declared().is_none() {
            let span = arena.expr(id).source_span.clone();
            let origin = TypeOrigin::OriginatedAt(span);
            let updated = current.clone().add_origin(origin);
            types.set(id, updated);
        }
    });
}

/// For every node that has a `type_annotation`, derives an `InferredType`
/// from that annotation and writes it into `TypeTable`.  For `Let` nodes the
/// type annotation applies to the *rhs* child, not the `Let` node itself
/// (mirrors the existing behaviour exactly).
pub fn bind_type_annotations(root: ExprId, arena: &ExprArena, types: &mut TypeTable) {
    visit_expr_ids_pre_order_mut(root, arena, types, &mut |id, arena, types| {
        let node = arena.expr(id);
        match &node.kind {
            ExprKind::Let { expr: rhs_id, .. } => {
                if let Some(annotation) = &node.type_annotation {
                    let new_type =
                        InferredType::from(annotation).declared_at(node.source_span.clone());
                    let rhs_id = *rhs_id;
                    types.set(rhs_id, new_type);
                }
            }
            _ => {
                if let Some(annotation) = &node.type_annotation {
                    let new_type =
                        InferredType::from(annotation).declared_at(node.source_span.clone());
                    types.set(id, new_type);
                }
            }
        }
    });
}
