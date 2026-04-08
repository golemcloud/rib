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

use crate::expr_arena::{ArmPatternId, ExprArena, TypeTable};
use crate::rib_source_span::SourceSpan;
use crate::{ArmPattern, ComponentDependency, InferredType};

pub(crate) use internal::ConstructorDetail;

/// Precomputed list used when exhaustive checking must consider every WIT variant
/// (option, result, and all component variants). Built once per [`type_check`](crate::type_checker::checker::type_check)
/// and shared across all `match` expressions and recursive inner checks.
pub(crate) fn build_fallback_constructor_details(
    component_dependency: &ComponentDependency,
) -> Vec<ConstructorDetail> {
    internal::fallback_full(component_dependency)
}

pub(crate) fn check_exhaustive_pattern_match_with_arms(
    match_source_span: SourceSpan,
    scrutinee_type: &InferredType,
    arm_pattern_ids: &[ArmPatternId],
    arena: &ExprArena,
    types: &TypeTable,
    fallback_cached: &[ConstructorDetail],
) -> Result<(), ExhaustivePatternMatchError> {
    internal::check_exhaustive_pattern_match_from_ids(
        match_source_span,
        Some(scrutinee_type),
        arm_pattern_ids,
        arena,
        types,
        fallback_cached,
    )
}

#[derive(Debug, Clone)]
pub enum ExhaustivePatternMatchError {
    MissingConstructors {
        predicate_source_span: SourceSpan,
        missing_constructors: Vec<String>,
    },
    DeadCode {
        predicate_source_span: SourceSpan,
        cause: ArmPattern,
        dead_pattern: ArmPattern,
    },
}

mod internal {
    use crate::expr_arena::{
        rebuild_arm_pattern, rebuild_expr, ArmPatternId, ArmPatternNode, ExprArena, ExprId,
        ExprKind, TypeTable,
    };
    use crate::rib_source_span::SourceSpan;
    use crate::type_checker::exhaustive_pattern_match::ExhaustivePatternMatchError;
    use crate::wit_type::TypeVariant;
    use crate::{ArmPattern, ComponentDependency, InferredType, TypeInternal};
    use std::borrow::Cow;
    use std::collections::HashMap;
    use std::ops::Deref;

    /// Arena-based pattern reference: avoids [`rebuild_arm_pattern`] on the hot path.
    #[derive(Clone, Debug)]
    pub(super) enum PatternView {
        Node(ArmPatternId),
        /// Argument sub-expression in a pattern like `some(x)` (not a standalone pattern node).
        ArgLiteral(ExprId),
    }

    pub(crate) fn check_exhaustive_pattern_match_from_ids(
        match_source_span: SourceSpan,
        scrutinee_type: Option<&InferredType>,
        arm_pattern_ids: &[ArmPatternId],
        arena: &ExprArena,
        types: &TypeTable,
        fallback_cached: &[ConstructorDetail],
    ) -> Result<(), ExhaustivePatternMatchError> {
        let arms: Vec<PatternView> = arm_pattern_ids
            .iter()
            .map(|&id| PatternView::Node(id))
            .collect();
        check_exhaustive_pattern_match(
            match_source_span,
            scrutinee_type,
            &arms,
            arena,
            types,
            fallback_cached,
        )
    }

    fn pattern_view_to_arm_pattern(
        pv: &PatternView,
        arena: &ExprArena,
        types: &TypeTable,
    ) -> ArmPattern {
        match pv {
            PatternView::Node(id) => rebuild_arm_pattern(*id, arena, types),
            PatternView::ArgLiteral(eid) => {
                ArmPattern::Literal(Box::new(rebuild_expr(*eid, arena, types)))
            }
        }
    }

    fn expr_is_identifier(eid: ExprId, arena: &ExprArena) -> bool {
        matches!(arena.expr(eid).kind, ExprKind::Identifier { .. })
    }

    fn pattern_view_is_wildcard(pv: &PatternView, arena: &ExprArena) -> bool {
        matches!(
            pv,
            PatternView::Node(id) if matches!(arena.pattern(*id), ArmPatternNode::WildCard)
        )
    }

    fn pattern_view_is_literal_identifier(pv: &PatternView, arena: &ExprArena) -> bool {
        match pv {
            PatternView::Node(id) => match arena.pattern(*id) {
                ArmPatternNode::Literal(eid) => expr_is_identifier(*eid, arena),
                _ => false,
            },
            PatternView::ArgLiteral(eid) => expr_is_identifier(*eid, arena),
        }
    }

    struct PatternCollectState<'a> {
        arena: &'a ExprArena,
        with_arg_constructors: &'a [String],
        no_arg_constructors: &'a [String],
        constructor_map_result: &'a mut HashMap<String, Vec<PatternView>>,
        constructors_with_arg: &'a mut ConstructorsWithArgTracker,
        constructors_with_no_arg: &'a mut NoArgConstructorsTracker,
        detected_wild_card_or_identifier: &'a mut Vec<PatternView>,
    }

    fn handle_literal_expr(
        eid: ExprId,
        literal_pattern_id: Option<ArmPatternId>,
        state: &mut PatternCollectState<'_>,
    ) {
        if let ExprKind::Call { call_type, args } = &state.arena.expr(eid).kind {
            let ctor_name = call_type.to_string();
            let arm_patterns: Vec<PatternView> =
                args.iter().map(|&a| PatternView::ArgLiteral(a)).collect();
            if state.with_arg_constructors.contains(&ctor_name) {
                state
                    .constructor_map_result
                    .entry(ctor_name.clone())
                    .or_default()
                    .extend(arm_patterns);
                state.constructors_with_arg.register(ctor_name.as_str());
            } else if state.no_arg_constructors.contains(&ctor_name) {
                state.constructors_with_no_arg.register(ctor_name.as_str());
            }
        } else if expr_is_identifier(eid, state.arena) {
            let pv = if let Some(pid) = literal_pattern_id {
                PatternView::Node(pid)
            } else {
                PatternView::ArgLiteral(eid)
            };
            state.detected_wild_card_or_identifier.push(pv);
        }
    }

    fn process_pattern_view(pv: &PatternView, state: &mut PatternCollectState<'_>) {
        match pv {
            PatternView::ArgLiteral(eid) => {
                handle_literal_expr(*eid, None, state);
            }
            PatternView::Node(id) => match state.arena.pattern(*id) {
                ArmPatternNode::WildCard => {
                    state
                        .detected_wild_card_or_identifier
                        .push(PatternView::Node(*id));
                }
                ArmPatternNode::As(_, inner) => {
                    process_pattern_view(&PatternView::Node(*inner), state);
                }
                ArmPatternNode::Constructor(ctor_name, arm_patterns) => {
                    let mapped: Vec<PatternView> =
                        arm_patterns.iter().map(|&c| PatternView::Node(c)).collect();
                    if state.with_arg_constructors.contains(ctor_name) {
                        state
                            .constructor_map_result
                            .entry(ctor_name.clone())
                            .or_default()
                            .extend(mapped);
                        state.constructors_with_arg.register(ctor_name);
                    } else if state.no_arg_constructors.contains(ctor_name) {
                        state.constructors_with_no_arg.register(ctor_name);
                    }
                }
                ArmPatternNode::Literal(eid) => {
                    handle_literal_expr(*eid, Some(*id), state);
                }
                ArmPatternNode::TupleConstructor(_)
                | ArmPatternNode::RecordConstructor(_)
                | ArmPatternNode::ListConstructor(_) => {}
            },
        }
    }

    pub(crate) fn check_exhaustive_pattern_match(
        match_source_span: SourceSpan,
        scrutinee_type: Option<&InferredType>,
        arms: &[PatternView],
        arena: &ExprArena,
        types: &TypeTable,
        fallback_cached: &[ConstructorDetail],
    ) -> Result<(), ExhaustivePatternMatchError> {
        let constructor_details =
            constructor_details_for_scrutinee(scrutinee_type, fallback_cached);

        let mut exhaustive_check_result =
            ExhaustiveCheckResult(Ok(ConstructorPatterns(HashMap::new())));

        for detail in constructor_details.iter() {
            exhaustive_check_result = exhaustive_check_result.unwrap_or_run_with(
                match_source_span.clone(),
                arms,
                detail.clone(),
                arena,
                types,
            );
        }

        let inner_constructors = exhaustive_check_result.value()?;

        for (field, patterns) in inner_constructors.inner() {
            check_exhaustive_pattern_match(
                match_source_span.clone(),
                None,
                patterns,
                arena,
                types,
                fallback_cached,
            )
            .map_err(|e| match e {
                ExhaustivePatternMatchError::MissingConstructors {
                    missing_constructors,
                    ..
                } => {
                    let mut new_missing_constructors = vec![];
                    missing_constructors.iter().for_each(|missing_constructor| {
                        new_missing_constructors.push(format!("{field}({missing_constructor})"));
                    });
                    ExhaustivePatternMatchError::MissingConstructors {
                        predicate_source_span: match_source_span.clone(),
                        missing_constructors: new_missing_constructors,
                    }
                }
                other_errors => other_errors,
            })?;
        }

        Ok(())
    }

    fn constructor_details_for_scrutinee<'a>(
        scrutinee: Option<&InferredType>,
        fallback_cached: &'a [ConstructorDetail],
    ) -> Cow<'a, [ConstructorDetail]> {
        let Some(ty) = scrutinee else {
            return Cow::Borrowed(fallback_cached);
        };
        if ty.is_unknown() {
            return Cow::Borrowed(fallback_cached);
        }
        match ty.inner.deref() {
            TypeInternal::Option(_) => Cow::Owned(vec![ConstructorDetail::option()]),
            TypeInternal::Result { .. } => Cow::Owned(vec![ConstructorDetail::result()]),
            TypeInternal::Variant(cases) => {
                Cow::Owned(vec![ConstructorDetail::from_inferred_variant_cases(cases)])
            }
            TypeInternal::Enum(cases) => {
                Cow::Owned(vec![ConstructorDetail::from_enum_cases(cases)])
            }
            TypeInternal::AllOf(_) | TypeInternal::Instance { .. } => {
                Cow::Borrowed(fallback_cached)
            }
            _ => Cow::Borrowed(fallback_cached),
        }
    }

    pub(super) fn fallback_full(
        component_dependency: &ComponentDependency,
    ) -> Vec<ConstructorDetail> {
        let mut constructor_details = vec![ConstructorDetail::option()];
        for variant in component_dependency.get_variants() {
            constructor_details.push(ConstructorDetail::from_variant(variant));
        }
        constructor_details.push(ConstructorDetail::option());
        constructor_details.push(ConstructorDetail::result());
        constructor_details
    }

    #[derive(Clone, Debug)]
    pub struct ConstructorPatterns(HashMap<String, Vec<PatternView>>);

    impl ConstructorPatterns {
        pub fn inner(&self) -> &HashMap<String, Vec<PatternView>> {
            &self.0
        }

        fn is_empty(&self) -> bool {
            self.0.is_empty()
        }
    }

    #[derive(Debug, Clone)]
    pub(crate) struct ExhaustiveCheckResult(
        pub(crate) Result<ConstructorPatterns, ExhaustivePatternMatchError>,
    );

    impl ExhaustiveCheckResult {
        fn unwrap_or_run_with(
            &self,
            match_source_span: SourceSpan,
            patterns: &[PatternView],
            constructor_details: ConstructorDetail,
            arena: &ExprArena,
            types: &TypeTable,
        ) -> ExhaustiveCheckResult {
            match self {
                ExhaustiveCheckResult(Ok(result)) if result.is_empty() => check_exhaustive(
                    match_source_span,
                    patterns,
                    constructor_details,
                    arena,
                    types,
                ),
                ExhaustiveCheckResult(Ok(_)) => self.clone(),
                ExhaustiveCheckResult(Err(e)) => ExhaustiveCheckResult(Err(e.clone())),
            }
        }

        fn value(&self) -> Result<ConstructorPatterns, ExhaustivePatternMatchError> {
            self.0.clone()
        }

        fn missing_constructors(
            match_source_span: SourceSpan,
            missing_constructors: Vec<String>,
        ) -> Self {
            ExhaustiveCheckResult(Err(ExhaustivePatternMatchError::MissingConstructors {
                predicate_source_span: match_source_span,
                missing_constructors,
            }))
        }

        fn dead_code(
            match_source_span: SourceSpan,
            cause: ArmPattern,
            dead_pattern: ArmPattern,
        ) -> Self {
            ExhaustiveCheckResult(Err(ExhaustivePatternMatchError::DeadCode {
                predicate_source_span: match_source_span,
                cause,
                dead_pattern,
            }))
        }

        fn succeed(constructor_patterns: ConstructorPatterns) -> Self {
            ExhaustiveCheckResult(Ok(constructor_patterns))
        }
    }

    fn check_exhaustive(
        match_source_span: SourceSpan,
        patterns: &[PatternView],
        pattern_mach_args: ConstructorDetail,
        arena: &ExprArena,
        types: &TypeTable,
    ) -> ExhaustiveCheckResult {
        let with_arg_constructors = pattern_mach_args.with_arg_constructors;
        let no_arg_constructors = pattern_mach_args.no_arg_constructors;

        let mut constructors_with_arg: ConstructorsWithArgTracker =
            ConstructorsWithArgTracker::new();
        let mut constructors_with_no_arg: NoArgConstructorsTracker =
            NoArgConstructorsTracker::new();
        let mut detected_wild_card_or_identifier: Vec<PatternView> = vec![];
        let mut constructor_map_result: HashMap<String, Vec<PatternView>> = HashMap::new();

        constructors_with_arg.initialise(with_arg_constructors.clone());
        constructors_with_no_arg.initialise(no_arg_constructors.clone());

        let mut collect_state = PatternCollectState {
            arena,
            with_arg_constructors: &with_arg_constructors,
            no_arg_constructors: &no_arg_constructors,
            constructor_map_result: &mut constructor_map_result,
            constructors_with_arg: &mut constructors_with_arg,
            constructors_with_no_arg: &mut constructors_with_no_arg,
            detected_wild_card_or_identifier: &mut detected_wild_card_or_identifier,
        };

        for pattern in patterns {
            if !collect_state.detected_wild_card_or_identifier.is_empty() {
                let cause = collect_state
                    .detected_wild_card_or_identifier
                    .last()
                    .map(|pv| pattern_view_to_arm_pattern(pv, arena, types))
                    .unwrap_or(ArmPattern::WildCard);
                let dead = pattern_view_to_arm_pattern(pattern, arena, types);
                return ExhaustiveCheckResult::dead_code(match_source_span.clone(), cause, dead);
            }
            process_pattern_view(pattern, &mut collect_state);
        }

        if constructors_with_arg.registered_any() || constructors_with_no_arg.registered_any() {
            let all_with_arg_covered = constructors_with_arg.registered_all();
            let all_no_arg_covered = constructors_with_no_arg.registered_all();

            if (!all_with_arg_covered || !all_no_arg_covered)
                && detected_wild_card_or_identifier.is_empty()
            {
                let mut missing_constructors: Vec<_> =
                    constructors_with_arg.unregistered_constructors();
                let missing_no_arg_constructors: Vec<_> =
                    constructors_with_no_arg.unregistered_constructors();

                missing_constructors.extend(missing_no_arg_constructors.clone());

                return ExhaustiveCheckResult::missing_constructors(
                    match_source_span.clone(),
                    missing_constructors,
                );
            }
        }

        if !constructor_map_result.is_empty() {
            if !detected_wild_card_or_identifier.is_empty() {
                constructor_map_result.values_mut().for_each(|patterns| {
                    if !patterns.iter().any(|pv| {
                        pattern_view_is_literal_identifier(pv, arena)
                            || pattern_view_is_wildcard(pv, arena)
                    }) {
                        patterns.extend(detected_wild_card_or_identifier.clone());
                    }
                });
            }

            return ExhaustiveCheckResult::succeed(ConstructorPatterns(constructor_map_result));
        }

        ExhaustiveCheckResult::succeed(ConstructorPatterns(constructor_map_result))
    }

    struct ConstructorsWithArgTracker {
        status: HashMap<String, bool>,
    }

    impl ConstructorsWithArgTracker {
        fn new() -> Self {
            ConstructorsWithArgTracker {
                status: HashMap::new(),
            }
        }

        fn initialise(&mut self, with_arg_constructors: Vec<String>) {
            for constructor in with_arg_constructors {
                self.status.insert(constructor.to_string(), false);
            }
        }

        fn register(&mut self, constructor: &str) {
            self.status.insert(constructor.to_string(), true);
        }

        fn registered_any(&self) -> bool {
            self.status.values().any(|&v| v)
        }

        fn registered_all(&self) -> bool {
            self.status.values().all(|&v| v)
        }

        fn unregistered_constructors(&self) -> Vec<String> {
            get_false_entries(&self.status)
        }
    }

    struct NoArgConstructorsTracker {
        status: HashMap<String, bool>,
    }

    impl NoArgConstructorsTracker {
        fn new() -> Self {
            NoArgConstructorsTracker {
                status: HashMap::new(),
            }
        }

        fn initialise(&mut self, no_arg_constructors: Vec<String>) {
            for constructor in no_arg_constructors {
                self.status.insert(constructor.to_string(), false);
            }
        }

        fn register(&mut self, constructor: &str) {
            self.status.insert(constructor.to_string(), true);
        }

        fn registered_any(&self) -> bool {
            self.status.values().any(|&v| v)
        }

        fn registered_all(&self) -> bool {
            self.status.values().all(|&v| v)
        }

        fn unregistered_constructors(&self) -> Vec<String> {
            get_false_entries(&self.status)
        }
    }

    fn get_false_entries(map: &HashMap<String, bool>) -> Vec<String> {
        map.iter()
            .filter(|(_, &v)| !v)
            .map(|(k, _)| k.clone())
            .collect()
    }

    #[derive(Clone, Debug)]
    pub(crate) struct ConstructorDetail {
        no_arg_constructors: Vec<String>,
        with_arg_constructors: Vec<String>,
    }

    impl ConstructorDetail {
        fn from_variant(variant: TypeVariant) -> ConstructorDetail {
            let cases = variant.cases;

            let (no_arg_constructors, with_arg_constructors): (Vec<_>, Vec<_>) =
                cases.into_iter().partition(|c| c.typ.is_none());

            ConstructorDetail {
                no_arg_constructors: no_arg_constructors.iter().map(|c| c.name.clone()).collect(),
                with_arg_constructors: with_arg_constructors
                    .iter()
                    .map(|c| c.name.clone())
                    .collect(),
            }
        }

        fn option() -> Self {
            ConstructorDetail {
                no_arg_constructors: vec!["none".to_string()],
                with_arg_constructors: vec!["some".to_string()],
            }
        }

        fn result() -> Self {
            ConstructorDetail {
                no_arg_constructors: vec![],
                with_arg_constructors: vec!["ok".to_string(), "err".to_string()],
            }
        }

        fn from_inferred_variant_cases(
            cases: &[(String, Option<InferredType>)],
        ) -> ConstructorDetail {
            let (no_arg_constructors, with_arg_constructors): (Vec<_>, Vec<_>) =
                cases.iter().partition(|(_, typ)| typ.is_none());

            ConstructorDetail {
                no_arg_constructors: no_arg_constructors
                    .iter()
                    .map(|(name, _)| name.clone())
                    .collect(),
                with_arg_constructors: with_arg_constructors
                    .iter()
                    .map(|(name, _)| name.clone())
                    .collect(),
            }
        }

        fn from_enum_cases(cases: &[String]) -> ConstructorDetail {
            ConstructorDetail {
                no_arg_constructors: cases.to_vec(),
                with_arg_constructors: vec![],
            }
        }
    }
}
#[cfg(test)]
mod pattern_match_exhaustive_tests {
    use crate::type_checker::exhaustive_pattern_match::pattern_match_exhaustive_tests::internal::strip_spaces;
    use crate::{Expr, RibCompiler};
    use test_r::test;

    #[test]
    fn test_option_pattern_match1() {
        let expr = r#"
        let x = some("foo");
        match x {
            some(a) => a,
            none => "none"
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();

        let compiler = RibCompiler::default();
        let result = compiler.compile(expr);
        assert!(result.is_ok())
    }

    #[test]
    fn test_option_pattern_match2() {
        let expr = r#"
        let x = some("foo");
        match x {
            none => "none",
            some(a) => a
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();

        let compiler = RibCompiler::default();
        let result = compiler.compile(expr);
        assert!(result.is_ok())
    }

    #[test]
    fn test_option_pattern_match_wild_card1() {
        let expr = r#"
        let x = some("foo");
        match x {
            some(_) => a,
            none => "none"
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();
        let compiler = RibCompiler::default();
        let result = compiler.compile(expr);
        assert!(result.is_ok())
    }
    #[test]
    fn test_option_pattern_match_wild_card2() {
        let expr = r#"
        let x = some("foo");
        match x {
            none => "none",
            some(_) => a

        }
        "#;

        let expr = Expr::from_text(expr).unwrap();
        let compiler = RibCompiler::default();
        let result = compiler.compile(expr);
        assert!(result.is_ok())
    }

    #[test]
    fn test_option_pattern_match_wild_card3() {
        let expr = r#"
        let x = some("foo");
        match x {
            some(a) => a,
            _ => "none"
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();
        let compiler = RibCompiler::default();
        let result = compiler.compile(expr);
        assert!(result.is_ok())
    }

    #[test]
    fn test_option_pattern_match_wild_card4() {
        let expr = r#"
        let x = some("foo");
        match x {
            none => "none",
            _ => "none"
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();
        let compiler = RibCompiler::default();
        let result = compiler.compile(expr);
        assert!(result.is_ok())
    }

    #[test]
    fn test_option_pattern_match_wild_card5() {
        let expr = r#"
        let x = some("foo");
        match x {
            some(_) => a,
            _ => "none"
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();
        let compiler = RibCompiler::default();
        let result = compiler.compile(expr);
        assert!(result.is_ok())
    }

    #[test]
    fn test_option_pattern_match_wild_card_invalid1() {
        let expr = r#"
        let x = some("foo");
        match x {
            _ => "none",
            some(_) => a
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();

        let compiler = RibCompiler::default();
        let error_msg = compiler.compile(expr).unwrap_err().to_string();

        let expected = r#"
        error in the following rib found at line 3, column 9
        `match x {  _ => "none", some(_) => a } `
        cause: dead code detected, pattern `some(_)` is unreachable due to the existence of the pattern `_` prior to it
        help: to ensure a complete match, add missing patterns or use wildcard (`_`)
        "#;

        assert_eq!(error_msg, strip_spaces(expected))
    }

    #[test]
    fn test_option_pattern_match_wild_card_invalid2() {
        let expr = r#"
        let x = some("foo");
        match x {
            _ => "none",
            none => "a"
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();

        let compiler = RibCompiler::default();
        let error_msg = compiler.compile(expr).unwrap_err().to_string();

        let expected = r#"
        error in the following rib found at line 3, column 9
        `match x {  _ => "none", none => "a" } `
        cause: dead code detected, pattern `none` is unreachable due to the existence of the pattern `_` prior to it
        help: to ensure a complete match, add missing patterns or use wildcard (`_`)
        "#;

        assert_eq!(error_msg, strip_spaces(expected))
    }

    #[test]
    fn test_option_pattern_match_identifier_invalid1() {
        let expr = r#"
        let x = some("foo");
        match x {
            something => "none",
            some(_) => a
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();

        let compiler = RibCompiler::default();
        let error_msg = compiler.compile(expr).unwrap_err().to_string();

        let expected = r#"
        error in the following rib found at line 3, column 9
        `match x {  something => "none", some(_) => a } `
        cause: dead code detected, pattern `some(_)` is unreachable due to the existence of the pattern `something` prior to it
        help: to ensure a complete match, add missing patterns or use wildcard (`_`)
        "#;

        assert_eq!(error_msg, strip_spaces(expected))
    }

    #[test]
    fn test_option_pattern_match_identifier_invalid2() {
        let expr = r#"
        let x = some("foo");
        match x {
            something => "none",
            none => "a"
        }
        "#;
        let expr = Expr::from_text(expr).unwrap();

        let compiler = RibCompiler::default();
        let error_msg = compiler.compile(expr).unwrap_err().to_string();

        let expected = r#"
        error in the following rib found at line 3, column 9
        `match x {  something => "none", none => "a" } `
        cause: dead code detected, pattern `none` is unreachable due to the existence of the pattern `something` prior to it
        help: to ensure a complete match, add missing patterns or use wildcard (`_`)
        "#;

        assert_eq!(error_msg, strip_spaces(expected))
    }

    #[test]
    fn test_option_none_absent() {
        let expr = r#"
        let x = some("foo");
        match x {
            some(a) => a
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();

        let compiler = RibCompiler::default();
        let error_msg = compiler.compile(expr).unwrap_err().to_string();

        let expected = r#"
        error in the following rib found at line 3, column 9
        `match x {  some(a) => a } `
        cause: non-exhaustive pattern match: the following patterns are not covered: `none`
        help: to ensure a complete match, add missing patterns or use wildcard (`_`)
        "#;

        assert_eq!(error_msg, strip_spaces(expected));
    }

    #[test]
    fn test_option_some_absent() {
        let expr = r#"
        let x = some("foo");
        match x {
           none => "none"
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();

        let compiler = RibCompiler::default();
        let error_msg = compiler.compile(expr).unwrap_err().to_string();

        let expected = r#"
        error in the following rib found at line 3, column 9
        `match x {  none => "none" } `
        cause: non-exhaustive pattern match: the following patterns are not covered: `some`
        help: to ensure a complete match, add missing patterns or use wildcard (`_`)
        "#;

        assert_eq!(error_msg, strip_spaces(expected));
    }

    #[test]
    fn test_option_nested_invalid1() {
        let expr = r#"
        let x = some(some("foo"));
        match x {
            some(some(a)) => a,
            none => "bar"
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();

        let compiler = RibCompiler::default();
        let error_msg = compiler.compile(expr).unwrap_err().to_string();

        let expected = r#"
        error in the following rib found at line 3, column 9
        `match x {  some(some(a)) => a, none => "bar" } `
        cause: non-exhaustive pattern match: the following patterns are not covered: `some(none)`
        help: to ensure a complete match, add missing patterns or use wildcard (`_`)
        "#;

        assert_eq!(error_msg, strip_spaces(expected));
    }

    #[test]
    fn test_result_pattern_match1() {
        let expr = r#"
        let x: result<string, string> = ok("foo");
        match x {
            ok(a) => a,
            err(msg) =>  msg
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();

        let compiler = RibCompiler::default();
        let result = compiler.compile(expr);
        assert!(result.is_ok())
    }

    #[test]
    fn test_result_pattern_match2() {
        let expr = r#"
        let x: result<string, string> = ok("foo");
        match x {
            err(a) => a,
            ok(a) => a
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();

        let compiler = RibCompiler::default();
        let result = compiler.compile(expr);
        assert!(result.is_ok())
    }

    #[test]
    fn test_result_pattern_match_wild_card1() {
        let expr = r#"
        let x = ok("foo");
        match x {
            err(_) => "error",
            ok(msg) => msg
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();
        let compiler = RibCompiler::default();
        let result = compiler.compile(expr);
        assert!(result.is_ok())
    }
    #[test]
    fn test_result_pattern_match_wild_card2() {
        let expr = r#"
        let x: result<string, string> = ok("foo");
        match x {
            err(msg) => msg,
            ok(_) => a

        }
        "#;

        let expr = Expr::from_text(expr).unwrap();
        let compiler = RibCompiler::default();
        let result = compiler.compile(expr);
        assert!(result.is_ok())
    }

    #[test]
    fn test_result_pattern_match_wild_card3() {
        let expr = r#"
        let x = ok("foo");
        match x {
            ok(a) => a,
            _ => "none"
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();
        let compiler = RibCompiler::default();
        let result = compiler.compile(expr);
        assert!(result.is_ok())
    }

    #[test]
    fn test_result_pattern_match_wild_card4() {
        let expr = r#"
        let x = err("foo");
        match x {
            err(msg) => "none",
            _ => "none"
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();
        let compiler = RibCompiler::default();
        let result = compiler.compile(expr);
        assert!(result.is_ok())
    }

    #[test]
    fn test_result_pattern_match_wild_card5() {
        let expr = r#"
        let x = ok("foo");
        match x {
            ok(_) => a,
            _ => "none"
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();
        let compiler = RibCompiler::default();
        let result = compiler.compile(expr);
        assert!(result.is_ok())
    }

    #[test]
    fn test_result_pattern_match_wild_card_invalid1() {
        let expr = r#"
        let x = ok("foo");
        match x {
            _ => "none",
            ok(_) => a
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();

        let compiler = RibCompiler::default();
        let error_msg = compiler.compile(expr).unwrap_err().to_string();

        let expected = r#"
        error in the following rib found at line 3, column 9
        `match x {  _ => "none", ok(_) => a } `
        cause: dead code detected, pattern `ok(_)` is unreachable due to the existence of the pattern `_` prior to it
        help: to ensure a complete match, add missing patterns or use wildcard (`_`)
        "#;

        assert_eq!(error_msg, strip_spaces(expected));
    }

    #[test]
    fn test_result_pattern_match_wild_card_invalid2() {
        let expr = r#"
        let x = err("foo");
        match x {
            _ => "none",
            err(msg) => "a"
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();

        let compiler = RibCompiler::default();
        let error_msg = compiler.compile(expr).unwrap_err().to_string();

        let expected = r#"
        error in the following rib found at line 3, column 9
        `match x {  _ => "none", err(msg) => "a" } `
        cause: dead code detected, pattern `err(msg)` is unreachable due to the existence of the pattern `_` prior to it
        help: to ensure a complete match, add missing patterns or use wildcard (`_`)
        "#;

        assert_eq!(error_msg, strip_spaces(expected));
    }

    #[test]
    fn test_result_pattern_match_identifier_invalid1() {
        let expr = r#"
        let x = ok("foo");
        match x {
            something => "none",
            ok(_) => "a"
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();

        let compiler = RibCompiler::default();
        let error_msg = compiler.compile(expr).unwrap_err().to_string();

        let expected = r#"
        error in the following rib found at line 3, column 9
        `match x {  something => "none", ok(_) => "a" } `
        cause: dead code detected, pattern `ok(_)` is unreachable due to the existence of the pattern `something` prior to it
        help: to ensure a complete match, add missing patterns or use wildcard (`_`)
        "#;

        assert_eq!(error_msg, strip_spaces(expected));
    }

    #[test]
    fn test_result_pattern_match_identifier_invalid2() {
        let expr = r#"
        let x = err("foo");
        match x {
            something => "none",
            err(msg) => "a"
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();

        let compiler = RibCompiler::default();
        let error_msg = compiler.compile(expr).unwrap_err().to_string();

        let expected = r#"
        error in the following rib found at line 3, column 9
        `match x {  something => "none", err(msg) => "a" } `
        cause: dead code detected, pattern `err(msg)` is unreachable due to the existence of the pattern `something` prior to it
        help: to ensure a complete match, add missing patterns or use wildcard (`_`)
        "#;

        assert_eq!(error_msg, strip_spaces(expected));
    }

    #[test]
    fn test_result_err_absent() {
        let expr = r#"
        let x = ok("foo");
        match x {
            ok(a) => a
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();

        let compiler = RibCompiler::default();
        let error_msg = compiler.compile(expr).unwrap_err().to_string();

        let expected = r#"
        error in the following rib found at line 3, column 9
        `match x {  ok(a) => a } `
        cause: non-exhaustive pattern match: the following patterns are not covered: `err`
        help: to ensure a complete match, add missing patterns or use wildcard (`_`)
        "#;

        assert_eq!(error_msg, strip_spaces(expected));
    }

    #[test]
    fn test_result_ok_absent() {
        // Explicit type annotation is required here otherwise `str` in `err` cannot be inferred
        let expr = r#"
        let x: result<string, string> = ok("foo");
        match x {
           err(str) => str
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();

        let compiler = RibCompiler::default();
        let error_msg = compiler.compile(expr).unwrap_err().to_string();

        let expected = r#"
        error in the following rib found at line 3, column 9
        `match x {  err(str) => str } `
        cause: non-exhaustive pattern match: the following patterns are not covered: `ok`
        help: to ensure a complete match, add missing patterns or use wildcard (`_`)
        "#;

        assert_eq!(error_msg, strip_spaces(expected));
    }

    #[test]
    fn test_result_nested_invalid1() {
        let expr = r#"
        let x = ok(err("foo"));
        match x {
            ok(err(a)) => a,
            err(_) => "bar"
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();

        let compiler = RibCompiler::default();
        let error_msg = compiler.compile(expr).unwrap_err().to_string();

        let expected = r#"
        error in the following rib found at line 3, column 9
        `match x {  ok(err(a)) => a, err(_) => "bar" } `
        cause: non-exhaustive pattern match: the following patterns are not covered: `ok(ok)`
        help: to ensure a complete match, add missing patterns or use wildcard (`_`)
        "#;

        assert_eq!(error_msg, strip_spaces(expected));
    }

    #[test]
    fn test_result_nested_invalid2() {
        let expr = r#"
        let x = ok(ok("foo"));
        match x {
            ok(ok(a)) => a,
            err(_) => "bar"
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();

        let compiler = RibCompiler::default();
        let error_msg = compiler.compile(expr).unwrap_err().to_string();

        let expected = r#"
        error in the following rib found at line 3, column 9
        `match x {  ok(ok(a)) => a, err(_) => "bar" } `
        cause: non-exhaustive pattern match: the following patterns are not covered: `ok(err)`
        help: to ensure a complete match, add missing patterns or use wildcard (`_`)
        "#;

        assert_eq!(error_msg, strip_spaces(expected));
    }

    #[test]
    fn test_result_wild_card_with_nested1() {
        let expr = r#"
        let x = ok(ok("foo"));
        match x {
            ok(ok(a)) => a,
            _ => "bar"
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();
        let compiler = RibCompiler::default();
        let result = compiler.compile(expr);

        assert!(result.is_ok());
    }

    #[test]
    fn test_result_wild_card_with_nested2() {
        let expr = r#"
        let x = err(err("foo"));
        match x {
            err(err(a)) => a,
            _ => "bar"
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();
        let compiler = RibCompiler::default();
        let result = compiler.compile(expr);

        assert!(result.is_ok());
    }

    #[test]
    fn test_option_wild_card_with_nested1() {
        let expr = r#"
        let x = some(some("foo"));
        match x {
            some(some(a)) => a,
            _ => "bar"
        }
        "#;

        let expr = Expr::from_text(expr).unwrap();
        let compiler = RibCompiler::default();
        let result = compiler.compile(expr);

        assert!(result.is_ok());
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
