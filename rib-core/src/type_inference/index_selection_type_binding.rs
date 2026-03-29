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

use crate::{visit_post_order_rev_mut, Expr, InferredType};
use std::ops::DerefMut;

pub mod arena {
    use crate::expr_arena::{ExprArena, ExprId, ExprKind, RangeKind, TypeTable};
    use crate::type_inference::expr_visitor::arena::children_of;
    use crate::InferredType;

    /// Arena version: sets `u64` on `Number` nodes that appear directly
    /// as the index of a `SelectIndex` or as bounds of a `Range`.
    pub fn bind_default_types_to_index_expressions(
        root: ExprId,
        arena: &ExprArena,
        types: &mut TypeTable,
    ) {
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let kind = arena.expr(id).kind.clone();
            match kind {
                ExprKind::SelectIndex { index: idx_id, .. } => {
                    set_number_u64(idx_id, arena, types);
                    // Also handle Range index
                    if let ExprKind::Range { range } = &arena.expr(idx_id).kind {
                        match range {
                            RangeKind::Range { from, to } => {
                                set_number_u64(*from, arena, types);
                                set_number_u64(*to, arena, types);
                            }
                            RangeKind::RangeInclusive { from, to } => {
                                set_number_u64(*from, arena, types);
                                set_number_u64(*to, arena, types);
                            }
                            RangeKind::RangeFrom { from } => {
                                set_number_u64(*from, arena, types);
                            }
                        }
                    }
                }
                ExprKind::Range { ref range } => match range {
                    RangeKind::Range { from, to } => {
                        set_number_u64(*from, arena, types);
                        set_number_u64(*to, arena, types);
                    }
                    RangeKind::RangeInclusive { from, to } => {
                        set_number_u64(*from, arena, types);
                        set_number_u64(*to, arena, types);
                    }
                    RangeKind::RangeFrom { from } => {
                        set_number_u64(*from, arena, types);
                    }
                },
                _ => {}
            }
            for child in children_of(id, arena).into_iter().rev() {
                stack.push(child);
            }
        }
    }

    fn set_number_u64(id: ExprId, arena: &ExprArena, types: &mut TypeTable) {
        if let ExprKind::Number { .. } = arena.expr(id).kind {
            types.set(id, InferredType::u64());
        }
    }
}

// All select indices with literal numbers don't need to explicit
// type annotation to get better developer experience,
// and all literal numbers will be automatically inferred as u64
pub fn bind_default_types_to_index_expressions(expr: &mut Expr) {
    visit_post_order_rev_mut(expr, &mut |expr| match expr {
        Expr::SelectIndex { index, .. } => {
            if let Expr::Number { inferred_type, .. } = index.deref_mut() {
                *inferred_type = InferredType::u64()
            }

            if let Expr::Range { range, .. } = index.deref_mut() {
                let exprs = range.get_exprs_mut();

                for expr in exprs {
                    if let Expr::Number { inferred_type, .. } = expr.deref_mut() {
                        *inferred_type = InferredType::u64()
                    }
                }
            }
        }

        Expr::Range { range, .. } => {
            let exprs = range.get_exprs_mut();

            for expr in exprs {
                if let Expr::Number { inferred_type, .. } = expr.deref_mut() {
                    *inferred_type = InferredType::u64()
                }
            }
        }

        _ => {}
    });
}
