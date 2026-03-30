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

use crate::rib_type_error::RibTypeErrorInternal;
use crate::Expr;

/// Runs [`arena::push_types_down`] on a lowered copy of `expr` and writes back
/// the rebuilt tree.
pub fn push_types_down(expr: &mut Expr) -> Result<(), RibTypeErrorInternal> {
    let (expr_arena, mut types, root) = crate::expr_arena::lower(expr);
    arena::push_types_down(root, &expr_arena, &mut types)?;
    *expr = crate::expr_arena::rebuild_expr(root, &expr_arena, &types);
    Ok(())
}

mod internal {
    use crate::analysis::AnalysedType;
    use crate::rib_source_span::SourceSpan;
    use crate::rib_type_error::RibTypeErrorInternal;
    use crate::type_inference::type_hint::{GetTypeHint, TypeHint};
    use crate::{
        ActualType, AmbiguousTypeError, ExpectedType, InferredType, Path, TypeMismatchError,
    };

    // actual_inferred_type: InferredType found in the outer structure
    // expr: The expr corresponding to the outer inferred type. Example: yield expr in a list comprehension
    // push_down_kind: The expected kind of the outer expression before pushing down
    pub fn get_compilation_error_for_ambiguity(
        actual_inferred_type: &InferredType,
        source_span: &SourceSpan,
        push_down_kind: &TypeHint,
    ) -> RibTypeErrorInternal {
        // First check if the inferred type is a fully valid WIT type
        // If so, we trust this as this may handle majority of the cases
        // in compiler's best effort to create precise error message
        match AnalysedType::try_from(actual_inferred_type) {
            Ok(analysed_type) => {
                let type_mismatch_error = TypeMismatchError {
                    source_span: source_span.clone(),
                    expected_type: ExpectedType::AnalysedType(analysed_type),
                    actual_type: ActualType::Hint(push_down_kind.clone()),
                    field_path: Path::default(),
                    additional_error_detail: Vec::new(),
                };
                type_mismatch_error.into()
            }

            Err(_) => {
                // InferredType is not a fully valid WIT type yet
                // however it has enough information for compiler to trust it over the expected `type_kind`
                let actual_kind = actual_inferred_type.get_type_hint();
                match actual_kind {
                    TypeHint::Number | TypeHint::Str | TypeHint::Boolean | TypeHint::Char => {
                        TypeMismatchError {
                            source_span: source_span.clone(),
                            expected_type: ExpectedType::Hint(actual_kind.clone()),
                            actual_type: ActualType::Hint(push_down_kind.clone()),
                            field_path: Default::default(),
                            additional_error_detail: vec![],
                        }
                        .into()
                    }

                    _ => AmbiguousTypeError::new(actual_inferred_type, source_span, push_down_kind)
                        .into(),
                }
            }
        }
    }
}

pub mod arena {
    use crate::expr_arena::{
        ArmPatternId, ArmPatternNode, ExprArena, ExprId, ExprKind, MatchArmNode, ResultExprKind,
        TypeTable,
    };
    use crate::rib_type_error::RibTypeErrorInternal;
    use crate::type_inference::expr_visitor::arena::children_of;
    use crate::type_inference::type_hint::TypeHint;
    use crate::type_inference::type_push_down::internal::get_compilation_error_for_ambiguity;
    use crate::type_refinement::precise_types::*;
    use crate::type_refinement::TypeRefinement;
    use crate::{InferredType, InvalidPatternMatchError, TypeInternal, VariableId};
    use std::ops::Deref;

    /// Arena version of `push_types_down`.
    ///
    /// Pre-order traversal: for each parent node, read its type from
    /// `TypeTable` and push derived types down into child nodes.
    pub fn push_types_down(
        root: ExprId,
        arena: &ExprArena,
        types: &mut TypeTable,
    ) -> Result<(), RibTypeErrorInternal> {
        let mut order = Vec::new();
        collect_pre_order(root, arena, &mut order);

        for id in order {
            let node = arena.expr(id);
            let kind = node.kind.clone();
            let span = node.source_span.clone();
            let self_type = types.get(id).clone();

            match kind {
                ExprKind::SelectField {
                    expr: inner_id,
                    ref field,
                } => {
                    let field = field.clone();
                    let record_type = InferredType::record(vec![(field, self_type)]);
                    merge_into(inner_id, record_type, types);
                }

                ExprKind::SelectIndex {
                    expr: inner_id,
                    index: index_id,
                } => {
                    let index_type = types.get(index_id).clone();
                    match index_type.inner.deref() {
                        TypeInternal::Range { .. } => merge_into(inner_id, self_type, types),
                        _ => merge_into(inner_id, InferredType::list(self_type), types),
                    }
                }

                ExprKind::Cond {
                    cond: cond_id,
                    lhs: lhs_id,
                    rhs: rhs_id,
                } => {
                    merge_into(lhs_id, self_type.clone(), types);
                    merge_into(rhs_id, self_type, types);
                    merge_into(cond_id, InferredType::bool(), types);
                }

                ExprKind::Not { expr: inner_id } => {
                    merge_into(inner_id, self_type, types);
                }

                ExprKind::Option {
                    expr: Some(inner_id),
                } => {
                    handle_option_arena(inner_id, &span, &self_type, types)?;
                }

                ExprKind::Result {
                    expr: ResultExprKind::Ok(inner_id),
                } => {
                    handle_ok_arena(inner_id, &span, &self_type, types)?;
                }

                ExprKind::Result {
                    expr: ResultExprKind::Err(inner_id),
                } => {
                    handle_err_arena(inner_id, &span, &self_type, types)?;
                }

                ExprKind::PatternMatch {
                    predicate: pred_id,
                    ref match_arms,
                } => {
                    let pred_type = types.get(pred_id).clone();
                    let arms: Vec<MatchArmNode> = match_arms.clone();
                    for arm in &arms {
                        update_arm_pattern_type_arena(
                            arm.arm_pattern,
                            arena,
                            types,
                            &span,
                            &pred_type,
                        )?;
                        merge_into(arm.arm_resolution_expr, self_type.clone(), types);
                    }
                }

                ExprKind::Tuple { ref exprs } => {
                    let exprs: Vec<ExprId> = exprs.clone();
                    handle_tuple_arena(&exprs, &span, &self_type, types)?;
                }

                ExprKind::Sequence { ref exprs } => {
                    let exprs: Vec<ExprId> = exprs.clone();
                    handle_sequence_arena(&exprs, &span, &self_type, types)?;
                }

                ExprKind::Record { ref fields } => {
                    let fields: Vec<(String, ExprId)> = fields.clone();
                    handle_record_arena(&fields, &span, &self_type, types)?;
                }

                ExprKind::Call {
                    ref call_type,
                    ref args,
                    ..
                } => {
                    use crate::expr_arena::CallTypeNode;
                    use crate::TypeInternal;
                    if let CallTypeNode::VariantConstructor(name) = call_type {
                        if let TypeInternal::Variant(variant) = self_type.inner.deref() {
                            let variant = variant.clone();
                            let name = name.clone();
                            let args: Vec<ExprId> = args.clone();
                            if let Some((_n, Some(inner_type))) =
                                variant.iter().find(|(vn, _)| vn == &name)
                            {
                                let inner_type = inner_type.clone();
                                for arg_id in args {
                                    merge_into(arg_id, inner_type.clone(), types);
                                }
                            }
                        }
                    }
                }

                ExprKind::ListComprehension {
                    ref iterated_variable,
                    iterable_expr: iterable_id,
                    yield_expr: yield_id,
                } => {
                    let iterated_var = iterated_variable.clone();
                    handle_list_comprehension_arena(
                        iterated_var,
                        iterable_id,
                        yield_id,
                        &self_type,
                        &span,
                        arena,
                        types,
                    )?;
                }

                ExprKind::ListReduce {
                    ref reduce_variable,
                    ref iterated_variable,
                    iterable_expr: iterable_id,
                    init_value_expr: init_id,
                    yield_expr: yield_id,
                } => {
                    let reduce_var = reduce_variable.clone();
                    let iter_var = iterated_variable.clone();
                    handle_list_reduce_arena(
                        reduce_var,
                        iter_var,
                        iterable_id,
                        init_id,
                        yield_id,
                        &self_type,
                        &span,
                        arena,
                        types,
                    )?;
                }

                ExprKind::Plus { lhs, rhs }
                | ExprKind::Minus { lhs, rhs }
                | ExprKind::Multiply { lhs, rhs }
                | ExprKind::Divide { lhs, rhs } => {
                    let lhs_t = types.get(lhs).clone();
                    let rhs_t = types.get(rhs).clone();
                    let current_lhs = types.get(lhs).clone();
                    types.set(lhs, current_lhs.merge(rhs_t).merge(self_type.clone()));
                    let current_rhs = types.get(rhs).clone();
                    types.set(rhs, current_rhs.merge(lhs_t).merge(self_type));
                }

                _ => {}
            }
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn merge_into(id: ExprId, ty: InferredType, types: &mut TypeTable) {
        let current = types.get(id).clone();
        types.set(id, current.merge(ty));
    }

    fn handle_option_arena(
        inner_id: ExprId,
        span: &crate::rib_source_span::SourceSpan,
        outer_type: &InferredType,
        types: &mut TypeTable,
    ) -> Result<(), RibTypeErrorInternal> {
        let refined = OptionalType::refine(outer_type).ok_or_else(|| {
            get_compilation_error_for_ambiguity(outer_type, span, &TypeHint::Option(None))
        })?;
        merge_into(inner_id, refined.inner_type().clone(), types);
        Ok(())
    }

    fn handle_ok_arena(
        inner_id: ExprId,
        span: &crate::rib_source_span::SourceSpan,
        outer_type: &InferredType,
        types: &mut TypeTable,
    ) -> Result<(), RibTypeErrorInternal> {
        let refined = OkType::refine(outer_type).ok_or_else(|| {
            get_compilation_error_for_ambiguity(
                outer_type,
                span,
                &TypeHint::Result {
                    ok: None,
                    err: None,
                },
            )
        })?;
        merge_into(inner_id, refined.inner_type().clone(), types);
        Ok(())
    }

    fn handle_err_arena(
        inner_id: ExprId,
        span: &crate::rib_source_span::SourceSpan,
        outer_type: &InferredType,
        types: &mut TypeTable,
    ) -> Result<(), RibTypeErrorInternal> {
        let refined = ErrType::refine(outer_type).ok_or_else(|| {
            get_compilation_error_for_ambiguity(
                outer_type,
                span,
                &TypeHint::Result {
                    ok: None,
                    err: None,
                },
            )
        })?;
        merge_into(inner_id, refined.inner_type().clone(), types);
        Ok(())
    }

    fn handle_sequence_arena(
        elems: &[ExprId],
        span: &crate::rib_source_span::SourceSpan,
        outer_type: &InferredType,
        types: &mut TypeTable,
    ) -> Result<(), RibTypeErrorInternal> {
        let refined = ListType::refine(outer_type).ok_or_else(|| {
            get_compilation_error_for_ambiguity(outer_type, span, &TypeHint::List(None))
        })?;
        let inner = refined.inner_type();
        for &id in elems {
            merge_into(id, inner.clone(), types);
        }
        Ok(())
    }

    fn handle_tuple_arena(
        elems: &[ExprId],
        span: &crate::rib_source_span::SourceSpan,
        outer_type: &InferredType,
        types: &mut TypeTable,
    ) -> Result<(), RibTypeErrorInternal> {
        let refined = TupleType::refine(outer_type).ok_or_else(|| {
            get_compilation_error_for_ambiguity(outer_type, span, &TypeHint::Tuple(None))
        })?;
        for (&id, ty) in elems.iter().zip(refined.inner_types()) {
            merge_into(id, ty.clone(), types);
        }
        Ok(())
    }

    fn handle_record_arena(
        fields: &[(String, ExprId)],
        span: &crate::rib_source_span::SourceSpan,
        outer_type: &InferredType,
        types: &mut TypeTable,
    ) -> Result<(), RibTypeErrorInternal> {
        let refined = RecordType::refine(outer_type).ok_or_else(|| {
            get_compilation_error_for_ambiguity(outer_type, span, &TypeHint::Record(None))
        })?;
        for (field, id) in fields {
            merge_into(*id, refined.inner_type_by_name(field).clone(), types);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_list_comprehension_arena(
        variable: VariableId,
        iterable_id: ExprId,
        yield_id: ExprId,
        comprehension_type: &InferredType,
        _span: &crate::rib_source_span::SourceSpan,
        arena: &ExprArena,
        types: &mut TypeTable,
    ) -> Result<(), RibTypeErrorInternal> {
        let iterable_type = types.get(iterable_id).clone();

        if !iterable_type.is_unknown() {
            let elem_type = match ListType::refine(&iterable_type) {
                Some(l) => l.inner_type(),
                None => {
                    let r = RangeType::refine(&iterable_type).ok_or_else(|| {
                        let iterable_span = arena.expr(iterable_id).source_span.clone();
                        get_compilation_error_for_ambiguity(
                            &iterable_type,
                            &iterable_span,
                            &TypeHint::List(None),
                        )
                        .with_additional_error_detail(
                            "the iterable expression in list comprehension should be of type list or a range",
                        )
                    })?;
                    r.inner_type()
                }
            };

            patch_comprehension_identifiers(yield_id, arena, types, &variable, &elem_type);
        }

        let refined_result = ListType::refine(comprehension_type).ok_or_else(|| {
            let yield_span = arena.expr(yield_id).source_span.clone();
            get_compilation_error_for_ambiguity(
                comprehension_type,
                &yield_span,
                &TypeHint::List(None),
            )
            .with_additional_error_detail("the result of a comprehension should be of type list")
        })?;

        merge_into(yield_id, refined_result.inner_type().clone(), types);
        Ok(())
    }

    fn patch_comprehension_identifiers(
        root: ExprId,
        arena: &ExprArena,
        types: &mut TypeTable,
        variable: &VariableId,
        elem_type: &InferredType,
    ) {
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let kind = arena.expr(id).kind.clone();
            if let ExprKind::Identifier { variable_id } = kind {
                if let VariableId::ListComprehension(l) = &variable_id {
                    if l.name == variable.name() {
                        merge_into(id, elem_type.clone(), types);
                    }
                }
            } else {
                for child in children_of(id, arena).into_iter().rev() {
                    stack.push(child);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_list_reduce_arena(
        reduce_variable: VariableId,
        iterated_variable: VariableId,
        iterable_id: ExprId,
        init_id: ExprId,
        yield_id: ExprId,
        aggregation_type: &InferredType,
        _span: &crate::rib_source_span::SourceSpan,
        arena: &ExprArena,
        types: &mut TypeTable,
    ) -> Result<(), RibTypeErrorInternal> {
        let iterable_type = types.get(iterable_id).clone();
        let init_type = types.get(init_id).clone();

        if !iterable_type.is_unknown() {
            let elem_type = match ListType::refine(&iterable_type) {
                Some(l) => l.inner_type(),
                None => {
                    let iterable_span = arena.expr(iterable_id).source_span.clone();
                    let r = RangeType::refine(&iterable_type).ok_or_else(|| {
                        get_compilation_error_for_ambiguity(
                            &iterable_type,
                            &iterable_span,
                            &TypeHint::List(None),
                        )
                        .with_additional_error_detail(
                            "the iterable expression in list comprehension should be of type list or a range",
                        )
                    })?;
                    r.inner_type()
                }
            };

            patch_reduce_identifiers(
                yield_id,
                arena,
                types,
                &iterated_variable,
                &elem_type,
                &reduce_variable,
                &init_type,
            );
        }

        merge_into(yield_id, aggregation_type.clone(), types);
        merge_into(init_id, aggregation_type.clone(), types);
        Ok(())
    }

    fn patch_reduce_identifiers(
        root: ExprId,
        arena: &ExprArena,
        types: &mut TypeTable,
        iter_var: &VariableId,
        elem_type: &InferredType,
        reduce_var: &VariableId,
        init_type: &InferredType,
    ) {
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let kind = arena.expr(id).kind.clone();
            if let ExprKind::Identifier { variable_id } = kind {
                if let VariableId::ListComprehension(l) = &variable_id {
                    if l.name == iter_var.name() {
                        merge_into(id, elem_type.clone(), types);
                    }
                } else if let VariableId::ListReduce(l) = &variable_id {
                    if l.name == reduce_var.name() {
                        merge_into(id, init_type.clone(), types);
                    }
                }
            } else {
                for child in children_of(id, arena).into_iter().rev() {
                    stack.push(child);
                }
            }
        }
    }

    fn update_arm_pattern_type_arena(
        pat_id: ArmPatternId,
        arena: &ExprArena,
        types: &mut TypeTable,
        span: &crate::rib_source_span::SourceSpan,
        predicate_type: &InferredType,
    ) -> Result<(), RibTypeErrorInternal> {
        match arena.pattern(pat_id).clone() {
            ArmPatternNode::Literal(expr_id) => {
                merge_into(expr_id, predicate_type.clone(), types);
            }
            ArmPatternNode::As(_, inner) => {
                update_arm_pattern_type_arena(inner, arena, types, span, predicate_type)?;
            }
            ArmPatternNode::Constructor(name, children) => {
                let inner_type = if name == "some" || name == "none" {
                    let opt = OptionalType::refine(predicate_type).ok_or_else(|| {
                        InvalidPatternMatchError::constructor_type_mismatch(span.clone(), &name)
                    })?;
                    opt.inner_type()
                } else if name == "ok" {
                    match OkType::refine(predicate_type) {
                        Some(ok) => ok.inner_type(),
                        None => {
                            ErrType::refine(predicate_type).ok_or_else(|| {
                                InvalidPatternMatchError::constructor_type_mismatch(
                                    span.clone(),
                                    "ok",
                                )
                            })?;
                            InferredType::unknown()
                        }
                    }
                } else if name == "err" {
                    match ErrType::refine(predicate_type) {
                        Some(err) => err.inner_type(),
                        None => {
                            OkType::refine(predicate_type).ok_or_else(|| {
                                InvalidPatternMatchError::constructor_type_mismatch(
                                    span.clone(),
                                    "err",
                                )
                            })?;
                            InferredType::unknown()
                        }
                    }
                } else if let Some(vt) = VariantType::refine(predicate_type) {
                    vt.inner_type_by_name(&name)
                } else {
                    InferredType::unknown()
                };
                for child in children {
                    update_arm_pattern_type_arena(child, arena, types, span, &inner_type)?;
                }
            }
            ArmPatternNode::TupleConstructor(children) => {
                let tuple = TupleType::refine(predicate_type).ok_or_else(|| {
                    InvalidPatternMatchError::constructor_type_mismatch(span.clone(), "tuple")
                })?;
                let inner_types = tuple.inner_types();
                if children.len() == inner_types.len() {
                    for (child, inner) in children.iter().zip(inner_types) {
                        update_arm_pattern_type_arena(*child, arena, types, span, &inner)?;
                    }
                } else {
                    return Err(InvalidPatternMatchError::arg_size_mismatch(
                        span.clone(),
                        "tuple",
                        inner_types.len(),
                        children.len(),
                    )
                    .into());
                }
            }
            ArmPatternNode::ListConstructor(children) => {
                let list = ListType::refine(predicate_type).ok_or_else(|| {
                    InvalidPatternMatchError::constructor_type_mismatch(span.clone(), "list")
                })?;
                let elem_type = list.inner_type();
                for child in children {
                    update_arm_pattern_type_arena(child, arena, types, span, &elem_type)?;
                }
            }
            ArmPatternNode::RecordConstructor(fields) => {
                let record = RecordType::refine(predicate_type).ok_or_else(|| {
                    InvalidPatternMatchError::constructor_type_mismatch(span.clone(), "record")
                })?;
                for (field, child) in fields {
                    let ft = record.inner_type_by_name(&field);
                    update_arm_pattern_type_arena(child, arena, types, span, &ft)?;
                }
            }
            ArmPatternNode::WildCard => {}
        }
        Ok(())
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

#[cfg(test)]
mod type_push_down_tests {
    use test_r::test;

    use crate::type_inference::type_push_down::type_push_down_tests::internal::strip_spaces;
    use crate::{Expr, InferredType, RibCompiler};

    #[test]
    fn test_push_down_for_record() {
        let mut expr = Expr::record(vec![(
            "titles".to_string(),
            Expr::identifier_global("x", None),
        )])
        .with_inferred_type(InferredType::all_of(vec![
            InferredType::record(vec![("titles".to_string(), InferredType::unknown())]),
            InferredType::record(vec![("titles".to_string(), InferredType::u64())]),
        ]));

        expr.push_types_down().unwrap();
        let expected = Expr::record(vec![(
            "titles".to_string(),
            Expr::identifier_global("x", None).with_inferred_type(InferredType::u64()),
        )])
        .with_inferred_type(InferredType::all_of(vec![
            InferredType::record(vec![("titles".to_string(), InferredType::unknown())]),
            InferredType::record(vec![("titles".to_string(), InferredType::u64())]),
        ]));
        assert_eq!(expr, expected);
    }

    #[test]
    fn test_push_down_for_sequence() {
        let mut expr = Expr::sequence(
            vec![
                Expr::identifier_global("x", None),
                Expr::identifier_global("y", None),
            ],
            None,
        )
        .with_inferred_type(InferredType::all_of(vec![
            InferredType::list(InferredType::u32()),
            InferredType::list(InferredType::u64()),
        ]));

        expr.push_types_down().unwrap();
        let expected =
            Expr::sequence(
                vec![
                    Expr::identifier_global("x", None).with_inferred_type(InferredType::all_of(
                        vec![InferredType::u32(), InferredType::u64()],
                    )),
                    Expr::identifier_global("y", None).with_inferred_type(InferredType::all_of(
                        vec![InferredType::u32(), InferredType::u64()],
                    )),
                ],
                None,
            )
            .with_inferred_type(InferredType::all_of(vec![
                InferredType::list(InferredType::u32()),
                InferredType::list(InferredType::u64()),
            ]));
        assert_eq!(expr, expected);
    }

    #[test]
    fn invalid_push_down() {
        let expr = r#"
          let x: tuple<u32, u16> = [1, 2];
          x
        "#;

        let expr = Expr::from_text(expr).unwrap();

        let compiler = RibCompiler::default();

        let error_message = compiler.compile(expr).unwrap_err().to_string();

        let expected = r#"
        error in the following rib found at line 2, column 36
        `[1, 2]`
        cause: type mismatch. expected tuple<u32, u16>, found list
        "#;

        assert_eq!(error_message, strip_spaces(expected));
    }

    mod internal {
        pub(crate) fn strip_spaces(input: &str) -> String {
            let lines = input.lines();

            let first_line = lines
                .clone()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("");
            let margin_width = first_line.chars().take_while(|c| c.is_whitespace()).count();

            let result = lines
                .map(|line| {
                    if line.trim().is_empty() {
                        String::new()
                    } else {
                        line[margin_width..].to_string()
                    }
                })
                .collect::<Vec<String>>()
                .join("\n");

            result.strip_prefix("\n").unwrap_or(&result).to_string()
        }
    }
}
