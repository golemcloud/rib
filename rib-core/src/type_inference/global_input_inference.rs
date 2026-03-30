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
use crate::type_inference::expr_visitor::arena::children_of;
use crate::{Expr, InferredType};
use std::collections::HashMap;

pub(crate) fn infer_global_inputs_lowered(root: ExprId, arena: &ExprArena, types: &mut TypeTable) {
    let global_vars = collect_global_variable_types(root, arena, types);

    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let kind = arena.expr(id).kind.clone();
        if let ExprKind::Identifier { variable_id } = kind {
            if variable_id.is_global() {
                if let Some(type_list) = global_vars.get(&variable_id.name()) {
                    types.set(id, InferredType::all_of(type_list.clone()));
                }
            }
        } else {
            for child in children_of(id, arena).into_iter().rev() {
                stack.push(child);
            }
        }
    }
}

fn collect_global_variable_types(
    root: ExprId,
    arena: &ExprArena,
    types: &TypeTable,
) -> HashMap<String, Vec<InferredType>> {
    let mut all_types: HashMap<String, Vec<InferredType>> = HashMap::new();

    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let kind = arena.expr(id).kind.clone();
        if let ExprKind::Identifier { variable_id } = kind {
            if variable_id.is_global() {
                let ty = types.get(id).clone();
                let entry = all_types.entry(variable_id.name()).or_default();
                if !entry.contains(&ty) {
                    entry.push(ty);
                }
            }
        } else {
            for child in children_of(id, arena).into_iter().rev() {
                stack.push(child);
            }
        }
    }

    all_types
}

pub fn infer_global_inputs(expr: &mut Expr) {
    let (expr_arena, mut types, root) = crate::expr_arena::lower(expr);
    infer_global_inputs_lowered(root, &expr_arena, &mut types);
    *expr = crate::expr_arena::rebuild_expr(root, &expr_arena, &types);
}
