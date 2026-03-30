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

use crate::expr_arena::{rebuild_expr, ExprArena, ExprId, ExprKind, TypeTable};
use crate::inferred_type::UnificationFailureInternal;
use crate::rib_source_span::SourceSpan;
use crate::type_inference::expr_visitor::arena::children_of;
use crate::{Expr, InferredType, TypeUnificationError};

fn get_type_unification_error_from_mismatch(
    rib: &Expr,
    expr_unified: &Expr,
    left: InferredType,
    right: InferredType,
) -> TypeUnificationError {
    let left_default = left.origin.is_default();
    let right_default = right.origin.is_default();

    let left_declared = left.origin.is_declared();
    let right_declared = right.origin.is_declared();

    let left_expr = left
        .source_span()
        .and_then(|span| rib.lookup(&span).map(|expr| (span, expr)));

    let right_expr = right
        .source_span()
        .and_then(|span| rib.lookup(&span).map(|expr| (span, expr)));

    match (left_expr, right_expr) {
        (Some((_, left_expr)), Some((right_span, right_expr))) => {
            let mut additional_error_detail = vec![format!(
                "expected type {} based on expression `{}` found at line {} column {}",
                right.printable(),
                right_expr,
                right_span.start_line(),
                right_span.start_column()
            )];

            additional_error_detail.extend(get_error_detail(
                &right_expr,
                &right,
                right_declared,
                right_default,
            ));
            additional_error_detail.extend(get_error_detail(
                &left_expr,
                &left,
                left_declared,
                left_default,
            ));

            TypeUnificationError::type_mismatch_error(
                left_expr.source_span(),
                right,
                left,
                additional_error_detail,
            )
        }

        (Some((_, left_expr)), None) => {
            let additional_error_detail =
                get_error_detail(&left_expr, &left, left_declared, left_default);

            TypeUnificationError::type_mismatch_error(
                left_expr.source_span(),
                right,
                left,
                additional_error_detail,
            )
        }

        (None, Some((_, right_expr))) => {
            let additional_error_detail =
                get_error_detail(&right_expr, &right, right_declared, right_default);

            TypeUnificationError::type_mismatch_error(
                right_expr.source_span(),
                left,
                right,
                additional_error_detail,
            )
        }

        (None, None) => {
            let additional_messages = vec![format!(
                "conflicting types: {}, {}",
                left.printable(),
                right.printable()
            )];

            TypeUnificationError::unresolved_types_error(
                expr_unified.source_span(),
                additional_messages,
            )
        }
    }
}

fn get_error_detail(
    expr: &Expr,
    inferred_type: &InferredType,
    declared: Option<&SourceSpan>,
    is_default: bool,
) -> Vec<String> {
    let mut details = vec![];

    if let Some(span) = declared {
        details.push(format!(
            "the type of `{}` is declared as `{}` at line {} column {}",
            expr,
            inferred_type.printable(),
            span.start_line(),
            span.start_column()
        ));
    } else if is_default {
        details.push(format!(
            "the expression `{}` is inferred as `{}` by default",
            expr,
            inferred_type.printable()
        ));
    }

    details
}

/// Same semantics as [`unify_types`], but updates a [`TypeTable`] in place. On failure, rebuilds
/// from `root` for error context (cold path).
pub fn unify_types_lowered(
    root: ExprId,
    arena: &ExprArena,
    types: &mut TypeTable,
) -> Result<(), TypeUnificationError> {
    let mut order = Vec::new();
    fn post_order(id: ExprId, arena: &ExprArena, out: &mut Vec<ExprId>) {
        for c in children_of(id, arena) {
            post_order(c, arena, out);
        }
        out.push(id);
    }
    post_order(root, arena, &mut order);

    for id in order {
        let kind = &arena.expr(id).kind;
        let skip = matches!(
            kind,
            ExprKind::Let { .. }
                | ExprKind::Boolean { .. }
                | ExprKind::Concat { .. }
                | ExprKind::GreaterThan { .. }
                | ExprKind::And { .. }
                | ExprKind::Or { .. }
                | ExprKind::GreaterThanOrEqualTo { .. }
                | ExprKind::LessThanOrEqualTo { .. }
                | ExprKind::EqualTo { .. }
                | ExprKind::LessThan { .. }
                | ExprKind::InvokeMethodLazy { .. }
        );
        if skip {
            continue;
        }

        let ty = types.get(id).clone();
        let unification_result = ty.unify();
        match unification_result {
            Ok(unified_type) => {
                types.set(id, unified_type);
            }
            Err(e) => {
                let original_expr = rebuild_expr(root, arena, types);
                let expr_unified = rebuild_expr(id, arena, types);
                return Err(match e {
                    UnificationFailureInternal::TypeMisMatch { left, right } => {
                        get_type_unification_error_from_mismatch(
                            &original_expr,
                            &expr_unified,
                            left,
                            right,
                        )
                    }

                    UnificationFailureInternal::ConflictingTypes {
                        conflicting_types,
                        additional_error_detail,
                    } => {
                        let mut additional_messages = vec![format!(
                            "conflicting types: {}",
                            conflicting_types
                                .iter()
                                .map(|t| t.printable())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )];

                        additional_messages.extend(additional_error_detail);

                        TypeUnificationError::unresolved_types_error(
                            expr_unified.source_span(),
                            additional_messages,
                        )
                    }

                    UnificationFailureInternal::UnknownType => {
                        TypeUnificationError::unresolved_types_error(
                            expr_unified.source_span(),
                            vec!["cannot determine the type".to_string()],
                        )
                    }
                });
            }
        }
    }
    Ok(())
}
