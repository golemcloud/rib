//! `env.<name>` is the only supported global input: **exactly one** field after `env` (never `env.a.b`),
//! always [`crate::InferredType::string`]. Nested paths are rejected to keep resolution and typing simple.

use crate::expr_arena::{ExprArena, ExprId, ExprKind, TypeTable};
use crate::inferred_type::InferredType;
use crate::rib_type_error::RibTypeErrorInternal;
use crate::type_inference::expr_visitor::arena::children_of;
use crate::CustomError;
use std::collections::HashMap;

/// Path segments after `env` when `start` is the **outermost** [`ExprKind::SelectField`] (`env.a.b` → `["a","b"]`).
pub fn env_path_from_outer_select(start: ExprId, arena: &ExprArena) -> Option<Vec<String>> {
    let mut fields = vec![];
    let mut current = start;
    loop {
        match &arena.expr(current).kind {
            ExprKind::SelectField { expr, field } => {
                fields.push(field.clone());
                current = *expr;
            }
            ExprKind::Identifier { variable_id } => {
                if variable_id.is_global() && variable_id.name() == "env" {
                    fields.reverse();
                    return Some(fields);
                }
                return None;
            }
            _ => return None,
        }
    }
}

pub fn env_path_key(path: &[String]) -> String {
    path.join(".")
}

/// Only `env.<field>` (one segment) is typed; inner `env` placeholder is not given a full record type.
pub fn apply_env_namespace_types(root: ExprId, arena: &ExprArena, types: &mut TypeTable) {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if let ExprKind::SelectField { expr, .. } = &arena.expr(id).kind {
            if let ExprKind::Identifier { variable_id } = &arena.expr(*expr).kind {
                if variable_id.is_global() && variable_id.name() == "env" {
                    types.set(id, InferredType::string());
                }
            }
        }
        for child in children_of(id, arena).into_iter().rev() {
            stack.push(child);
        }
    }
}

fn build_parent_map(root: ExprId, arena: &ExprArena) -> HashMap<ExprId, ExprId> {
    let mut map = HashMap::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        for ch in children_of(id, arena) {
            map.insert(ch, id);
            stack.push(ch);
        }
    }
    map
}

pub fn validate_global_identifiers(
    root: ExprId,
    arena: &ExprArena,
) -> Result<(), RibTypeErrorInternal> {
    let parent = build_parent_map(root, arena);
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let span = arena.expr(id).source_span.clone();
        if let ExprKind::SelectField { .. } = &arena.expr(id).kind {
            if let Some(path) = env_path_from_outer_select(id, arena) {
                if path.len() > 1 {
                    return Err(CustomError::new(
                        span,
                        "only a single field is allowed after `env` (e.g. `env.TOKEN_ID`). nested paths like `env.a.b` are not supported.",
                    )
                    .into());
                }
            }
        }
        if let ExprKind::Identifier { variable_id } = &arena.expr(id).kind {
            if variable_id.is_global() {
                let name = variable_id.name();
                if name == "env" {
                    let allowed = parent
                        .get(&id)
                        .map(|p| {
                            matches!(
                                &arena.expr(*p).kind,
                                ExprKind::SelectField { expr, .. } if *expr == id
                            )
                        })
                        .unwrap_or(false);
                    if !allowed {
                        return Err(CustomError::new(
                            span,
                            "`env` is reserved: use `env.<name>` (same spelling as the environment variable key, e.g. `env.TOKEN_ID`).",
                        )
                        .into());
                    }
                } else {
                    return Err(CustomError::new(
                        span,
                        format!(
                            "unknown global `{name}`. Rib only supports `env.<name>` for inputs (strings from environment variables)."
                        ),
                    )
                    .into());
                }
            }
        }
        for child in children_of(id, arena).into_iter().rev() {
            stack.push(child);
        }
    }
    Ok(())
}
