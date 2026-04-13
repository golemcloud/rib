//! Global-input typing runs on lowered IR; use [`infer_global_inputs`] from the same
//! `lower` / `rebuild_expr` boundary as [`crate::Expr::infer_types`].

use crate::expr_arena::{ExprArena, ExprId, ExprKind, TypeTable};
use crate::type_inference::expr_visitor::arena::children_of;
use crate::InferredType;
use std::collections::HashMap;

pub fn infer_global_inputs(root: ExprId, arena: &ExprArena, types: &mut TypeTable) {
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
