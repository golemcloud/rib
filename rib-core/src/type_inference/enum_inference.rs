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

use crate::{ComponentDependencies, Expr};

pub fn infer_enums(expr: &mut Expr, component_dependencies: &ComponentDependencies) {
    let eum_info = internal::get_enum_info(expr, component_dependencies);

    internal::convert_identifiers_to_enum_function_calls(expr, &eum_info);
}

mod internal {
    use crate::analysis::AnalysedType;
    use crate::call_type::CallType;
    use crate::{visit_post_order_rev_mut, ComponentDependencies, Expr};

    pub(crate) fn convert_identifiers_to_enum_function_calls(
        expr: &mut Expr,
        enum_info: &EnumInfo,
    ) {
        let enum_cases = enum_info.clone();

        visit_post_order_rev_mut(expr, &mut |expr| {
            if let Expr::Identifier {
                variable_id,
                inferred_type,
                source_span,
                type_annotation,
            } = expr
            {
                if enum_cases.cases.contains(&variable_id.name()) && !variable_id.is_local() {
                    *expr = Expr::Call {
                        call_type: CallType::EnumConstructor(variable_id.name()),
                        generic_type_parameter: None,
                        args: vec![],
                        inferred_type: inferred_type.clone(),
                        source_span: source_span.clone(),
                        type_annotation: type_annotation.clone(),
                    };
                }
            }
        });
    }

    pub(crate) fn get_enum_info(
        expr: &mut Expr,
        component_dependency: &ComponentDependencies,
    ) -> EnumInfo {
        let mut enum_cases = vec![];
        visit_post_order_rev_mut(expr, &mut |expr| {
            if let Expr::Identifier {
                variable_id,
                inferred_type,
                ..
            } = expr
            {
                // If variable is local, it takes priority over being a global enum
                if !variable_id.is_local() {
                    // Retrieve the possible no-arg variant from the registry
                    let result = component_dependency
                        .function_dictionary()
                        .iter()
                        .find_map(|x| x.get_enum_info(variable_id.name().as_str()));

                    if let Some(typed_enum) = result {
                        enum_cases.push(variable_id.name());
                        *inferred_type =
                            inferred_type.merge((&AnalysedType::Enum(typed_enum.clone())).into());
                    }
                }
            }
        });

        EnumInfo { cases: enum_cases }
    }

    #[derive(Debug, Clone)]
    pub(crate) struct EnumInfo {
        cases: Vec<String>,
    }
}

pub mod arena {
    use crate::analysis::AnalysedType;
    use crate::expr_arena::{CallTypeNode, ExprArena, ExprId, ExprKind, TypeTable};
    use crate::type_inference::expr_visitor::arena::children_of;
    use crate::ComponentDependencies;

    /// Arena version of `infer_enums`.
    ///
    /// Two passes:
    /// 1. Walk all `Identifier` nodes; if the name is a known enum case, update
    ///    its type in `TypeTable` and record its id.
    /// 2. Replace those nodes' `ExprKind` with `Call { EnumConstructor }`.
    ///    This is an in-place structural mutation — the `ExprId` stays the same.
    pub fn infer_enums(
        root: ExprId,
        arena: &mut ExprArena,
        types: &mut TypeTable,
        component_dependencies: &ComponentDependencies,
    ) {
        // Collect enum cases and update types.
        let enum_ids = collect_enum_identifiers(root, arena, types, component_dependencies);

        // Structurally replace Identifier nodes with Call { EnumConstructor }.
        for id in enum_ids {
            let node = arena.expr(id);
            if let ExprKind::Identifier { variable_id } = &node.kind {
                let name = variable_id.name();
                let annotation = node.type_annotation.clone();
                let span = node.source_span.clone();
                let node_mut = arena.expr_mut(id);
                node_mut.kind = ExprKind::Call {
                    call_type: CallTypeNode::EnumConstructor(name),
                    generic_type_parameter: None,
                    args: vec![],
                };
                node_mut.type_annotation = annotation;
                node_mut.source_span = span;
            }
        }
    }

    fn collect_enum_identifiers(
        root: ExprId,
        arena: &ExprArena,
        types: &mut TypeTable,
        component_dependencies: &ComponentDependencies,
    ) -> Vec<ExprId> {
        let mut enum_ids = Vec::new();
        let mut stack = vec![root];

        while let Some(id) = stack.pop() {
            let node = arena.expr(id);
            if let ExprKind::Identifier { variable_id } = &node.kind {
                if !variable_id.is_local() {
                    let result = component_dependencies
                        .function_dictionary()
                        .iter()
                        .find_map(|x| x.get_enum_info(variable_id.name().as_str()));

                    if let Some(typed_enum) = result {
                        let new_type: crate::InferredType =
                            (&AnalysedType::Enum(typed_enum.clone())).into();
                        let current = types.get(id).clone();
                        types.set(id, current.merge(new_type));
                        enum_ids.push(id);
                    }
                }
            }
            for child in children_of(id, arena).into_iter().rev() {
                stack.push(child);
            }
        }

        enum_ids
    }
}
