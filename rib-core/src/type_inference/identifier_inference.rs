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

use crate::{
    visit_post_order_rev_mut, visit_pre_order_mut, ArmPattern, Expr, InferredType, MatchArm,
    VariableId,
};
use std::collections::HashMap;

pub fn infer_all_identifiers(expr: &mut Expr) {
    // We scan top-down and bottom-up to inform the type between the identifiers
    // It doesn't matter which order we do it in (i.e, which identifier expression has the right type isn't a problem),
    // as we accumulate all the types in both directions
    infer_all_identifiers_bottom_up(expr);
    infer_all_identifiers_top_down(expr);
    infer_match_binding_variables(expr);
}

fn infer_all_identifiers_bottom_up(expr: &mut Expr) {
    let mut identifier_lookup = IdentifierTypeState::new();

    // Given
    //   `Expr::Block(Expr::Let(x, Expr::Num(1)), Expr::Call(func, x))`
    // Expr::Num(1)
    // Expr::Let(Variable(x), Expr::Num(1))
    // Expr::Identifier(x)
    // Expr::Call(func, Expr::Identifier(x))
    // Expr::Block(Expr::Let(x, Expr::Num(1)), Expr::Call(func, x))

    // Popping it from the back results in `Expr::Identifier(x)` to be processed first
    // in the above example.
    visit_post_order_rev_mut(expr, &mut |expr| {
        match expr {
            // If identifier is inferred (probably because it was part of a function call befre),
            // make sure to update the identifier inference lookup table.
            // If lookup table is already updated, merge the inferred type
            Expr::Identifier {
                variable_id,
                inferred_type,
                ..
            } => {
                if let Some(new_inferred_type) = identifier_lookup.lookup(variable_id) {
                    *inferred_type = inferred_type.merge(new_inferred_type)
                }

                identifier_lookup.update(variable_id.clone(), inferred_type.clone());
            }

            // In the above example `let x = 1`,
            // since `x` is already inferred before, we propagate the type to the expression to `1`.
            // Also if `1` is already inferred we update the identifier lookup table with x's type as 1's type
            Expr::Let {
                variable_id, expr, ..
            } => {
                if let Some(inferred_type) = identifier_lookup.lookup(variable_id) {
                    expr.add_infer_type_mut(inferred_type);
                }
                identifier_lookup.update(variable_id.clone(), expr.inferred_type());
            }

            _ => {}
        }
    });
}

// This is more of an optional stage, as bottom-up type propagation would be enough
// but helps with reaching early fix point later down the line of compilation phases
fn infer_all_identifiers_top_down(expr: &mut Expr) {
    let mut identifier_lookup = IdentifierTypeState::new();
    visit_pre_order_mut(expr, &mut |expr| match expr {
        Expr::Let {
            variable_id, expr, ..
        } => {
            if let Some(inferred_type) = identifier_lookup.lookup(variable_id) {
                expr.add_infer_type_mut(inferred_type);
            }

            identifier_lookup.update(variable_id.clone(), expr.inferred_type());
        }
        Expr::Identifier {
            variable_id,
            inferred_type,
            ..
        } => {
            if let Some(new_inferred_type) = identifier_lookup.lookup(variable_id) {
                *inferred_type = inferred_type.merge(new_inferred_type)
            }

            identifier_lookup.update(variable_id.clone(), inferred_type.clone());
        }

        _ => {}
    });
}

fn infer_match_binding_variables(expr: &mut Expr) {
    visit_post_order_rev_mut(expr, &mut |expr| {
        if let Expr::PatternMatch { match_arms, .. } = expr {
            for arm in match_arms {
                process_arm(arm)
            }
        }
    });
}

// A state that maps from the identifiers to the types inferred
#[derive(Debug, Clone)]
struct IdentifierTypeState(HashMap<VariableId, InferredType>);

impl IdentifierTypeState {
    fn new() -> Self {
        IdentifierTypeState(HashMap::new())
    }

    fn update(&mut self, id: VariableId, ty: InferredType) {
        self.0
            .entry(id)
            .and_modify(|e| *e = e.merge(ty.clone()))
            .or_insert(ty);
    }

    pub fn lookup(&self, id: &VariableId) -> Option<InferredType> {
        self.0.get(id).cloned()
    }
}

fn process_arm(arm: &mut MatchArm) {
    let arm_pattern = &mut arm.arm_pattern;
    let mut initial_set = IdentifierTypeState::new();
    collect_all_identifiers(arm_pattern, &mut initial_set);
    let arm_resolution = &mut arm.arm_resolution_expr;

    update_arm_resolution_expr_with_identifiers(arm_resolution, &initial_set);
}

fn collect_all_identifiers(pattern: &mut ArmPattern, state: &mut IdentifierTypeState) {
    match pattern {
        ArmPattern::WildCard => {}
        ArmPattern::As(_, arm_pattern) => collect_all_identifiers(arm_pattern, state),
        ArmPattern::Constructor(_, patterns) => {
            for pattern in patterns {
                collect_all_identifiers(pattern, state)
            }
        }
        ArmPattern::TupleConstructor(patterns) => {
            for pattern in patterns {
                collect_all_identifiers(pattern, state)
            }
        }
        ArmPattern::ListConstructor(patterns) => {
            for pattern in patterns {
                collect_all_identifiers(pattern, state)
            }
        }
        ArmPattern::RecordConstructor(fields) => {
            for (_, pattern) in fields {
                collect_all_identifiers(pattern, state)
            }
        }
        ArmPattern::Literal(expr) => accumulate_types_of_identifiers(&mut *expr, state),
    }
}

fn accumulate_types_of_identifiers(expr: &mut Expr, state: &mut IdentifierTypeState) {
    visit_post_order_rev_mut(expr, &mut |expr| {
        if let Expr::Identifier {
            variable_id,
            inferred_type,
            ..
        } = expr
        {
            if !inferred_type.is_unknown() {
                state.update(variable_id.clone(), inferred_type.clone())
            }
        }
    });
}

fn update_arm_resolution_expr_with_identifiers(
    arm_resolution: &mut Expr,
    state: &IdentifierTypeState,
) {
    visit_post_order_rev_mut(arm_resolution, &mut |expr| match expr {
        Expr::Identifier {
            variable_id,
            inferred_type,
            ..
        } if variable_id.is_match_binding() => {
            if let Some(new_inferred_type) = state.lookup(variable_id) {
                *inferred_type = inferred_type.merge(new_inferred_type)
            }
        }
        _ => {}
    });
}

pub mod arena {
    use super::IdentifierTypeState;
    use crate::expr_arena::{
        ArmPatternId, ArmPatternNode, ExprArena, ExprId, ExprKind, MatchArmNode, TypeTable,
    };
    use crate::type_inference::expr_visitor::arena::children_of;

    /// Arena version of `infer_all_identifiers`.
    pub fn infer_all_identifiers(root: ExprId, arena: &ExprArena, types: &mut TypeTable) {
        infer_all_identifiers_bottom_up(root, arena, types);
        infer_all_identifiers_top_down(root, arena, types);
        infer_match_binding_variables(root, arena, types);
    }

    fn infer_all_identifiers_bottom_up(root: ExprId, arena: &ExprArena, types: &mut TypeTable) {
        // Post-order-rev: process deepest nodes first (right-to-left children).
        // We approximate this with post-order here — the semantics of bottom-up
        // identifier propagation are preserved because we accumulate into a
        // lookup table that gets merged on every encounter.
        let mut lookup = IdentifierTypeState::new();

        // Collect traversal order first (post-order), then apply.
        // This avoids the borrow conflict of reading arena while mutating types.
        let mut order = Vec::new();
        collect_post_order(root, arena, &mut order);

        for id in order.into_iter().rev() {
            let node = arena.expr(id);
            match &node.kind {
                ExprKind::Identifier { variable_id } => {
                    let current = types.get(id).clone();
                    let merged = if let Some(looked_up) = lookup.lookup(variable_id) {
                        current.merge(looked_up)
                    } else {
                        current.clone()
                    };
                    types.set(id, merged.clone());
                    lookup.update(variable_id.clone(), merged);
                }
                ExprKind::Let {
                    variable_id,
                    expr: rhs_id,
                } => {
                    let rhs_id = *rhs_id;
                    if let Some(looked_up) = lookup.lookup(variable_id) {
                        let rhs_type = types.get(rhs_id).clone();
                        types.set(rhs_id, rhs_type.merge(looked_up));
                    }
                    let rhs_type = types.get(rhs_id).clone();
                    lookup.update(variable_id.clone(), rhs_type);
                }
                _ => {}
            }
        }
    }

    fn infer_all_identifiers_top_down(root: ExprId, arena: &ExprArena, types: &mut TypeTable) {
        let mut lookup = IdentifierTypeState::new();

        let mut order = Vec::new();
        collect_pre_order(root, arena, &mut order);

        for id in order {
            let node = arena.expr(id);
            match &node.kind {
                ExprKind::Let {
                    variable_id,
                    expr: rhs_id,
                } => {
                    let rhs_id = *rhs_id;
                    if let Some(looked_up) = lookup.lookup(variable_id) {
                        let rhs_type = types.get(rhs_id).clone();
                        types.set(rhs_id, rhs_type.merge(looked_up));
                    }
                    let rhs_type = types.get(rhs_id).clone();
                    lookup.update(variable_id.clone(), rhs_type);
                }
                ExprKind::Identifier { variable_id } => {
                    let current = types.get(id).clone();
                    let merged = if let Some(looked_up) = lookup.lookup(variable_id) {
                        current.merge(looked_up)
                    } else {
                        current.clone()
                    };
                    types.set(id, merged.clone());
                    lookup.update(variable_id.clone(), merged);
                }
                _ => {}
            }
        }
    }

    fn infer_match_binding_variables(root: ExprId, arena: &ExprArena, types: &mut TypeTable) {
        let mut order = Vec::new();
        collect_post_order(root, arena, &mut order);
        // process in reverse post-order (parents before children in original tree)
        for id in order.into_iter().rev() {
            let node = arena.expr(id);
            if let ExprKind::PatternMatch { match_arms, .. } = &node.kind {
                let arms: Vec<MatchArmNode> = match_arms.clone();
                for arm in arms {
                    process_arm_arena(&arm, arena, types);
                }
            }
        }
    }

    fn process_arm_arena(arm: &MatchArmNode, arena: &ExprArena, types: &mut TypeTable) {
        let mut state = IdentifierTypeState::new();
        collect_identifiers_from_pattern(arm.arm_pattern, arena, types, &mut state);
        update_arm_resolution_with_identifiers(arm.arm_resolution_expr, arena, types, &state);
    }

    fn collect_identifiers_from_pattern(
        pat_id: ArmPatternId,
        arena: &ExprArena,
        types: &TypeTable,
        state: &mut IdentifierTypeState,
    ) {
        match arena.pattern(pat_id) {
            ArmPatternNode::Literal(expr_id) => {
                let expr_id = *expr_id;
                // Walk all identifier nodes under this literal expression
                let mut order = Vec::new();
                collect_post_order(expr_id, arena, &mut order);
                for id in order {
                    let node = arena.expr(id);
                    if let ExprKind::Identifier { variable_id } = &node.kind {
                        let ty = types.get(id).clone();
                        if !ty.is_unknown() {
                            state.update(variable_id.clone(), ty);
                        }
                    }
                }
            }
            ArmPatternNode::WildCard => {}
            ArmPatternNode::As(_, inner) => {
                let inner = *inner;
                collect_identifiers_from_pattern(inner, arena, types, state);
            }
            ArmPatternNode::Constructor(_, children)
            | ArmPatternNode::TupleConstructor(children)
            | ArmPatternNode::ListConstructor(children) => {
                let children: Vec<_> = children.clone();
                for child in children {
                    collect_identifiers_from_pattern(child, arena, types, state);
                }
            }
            ArmPatternNode::RecordConstructor(fields) => {
                let fields: Vec<_> = fields.clone();
                for (_, child) in fields {
                    collect_identifiers_from_pattern(child, arena, types, state);
                }
            }
        }
    }

    fn update_arm_resolution_with_identifiers(
        resolution_id: ExprId,
        arena: &ExprArena,
        types: &mut TypeTable,
        state: &IdentifierTypeState,
    ) {
        let mut order = Vec::new();
        collect_post_order(resolution_id, arena, &mut order);
        for id in order.into_iter().rev() {
            let node = arena.expr(id);
            if let ExprKind::Identifier { variable_id } = &node.kind {
                if variable_id.is_match_binding() {
                    if let Some(new_type) = state.lookup(variable_id) {
                        let current = types.get(id).clone();
                        types.set(id, current.merge(new_type));
                    }
                }
            }
        }
    }

    // Simple iterative post-order collector (left-to-right children).
    fn collect_post_order(root: ExprId, arena: &ExprArena, out: &mut Vec<ExprId>) {
        let mut stack = vec![(root, false)];
        while let Some((id, visited)) = stack.pop() {
            if visited {
                out.push(id);
            } else {
                stack.push((id, true));
                for child in children_of(id, arena).into_iter().rev() {
                    stack.push((child, false));
                }
            }
        }
    }

    fn collect_pre_order(root: ExprId, arena: &ExprArena, out: &mut Vec<ExprId>) {
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            out.push(id);
            for child in children_of(id, arena).into_iter().rev() {
                stack.push(child);
            }
        }
    }
}
