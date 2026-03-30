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

//! Runs the former “initial phase” passes on an [`crate::expr_arena::ExprArena`] after [`crate::expr_arena::lower`],
//! in the same order as the historical `Expr` pipeline.

use crate::expr_arena::{ExprArena, ExprId, TypeTable};
use crate::rib_type_error::RibTypeErrorInternal;
use crate::type_inference as ti;
use crate::{ComponentDependencies, CustomInstanceSpec, GlobalVariableTypeSpec};

pub(crate) fn run_initial_binding_and_instance_phases(
    root: ExprId,
    arena: &mut ExprArena,
    types: &mut TypeTable,
    component_dependency: &ComponentDependencies,
    global_variable_type_spec: &[GlobalVariableTypeSpec],
    custom_instance_spec: &[CustomInstanceSpec],
) -> Result<(), RibTypeErrorInternal> {
    ti::global_variable_type_binding::arena::bind_global_variable_types(
        root,
        arena,
        types,
        global_variable_type_spec,
    );
    ti::type_annotation_binding::arena::bind_type_annotations(root, arena, types);
    ti::variable_binding::arena::bind_variables_of_list_comprehension(root, arena, types);
    ti::variable_binding::arena::bind_variables_of_list_reduce(root, arena, types);
    ti::variable_binding::arena::bind_variables_of_pattern_match(root, arena, types);
    ti::variable_binding::arena::bind_variables_of_let_assignment(root, arena, types);
    ti::identify_instance_creation::arena::identify_instance_creation(
        root,
        arena,
        types,
        component_dependency,
        custom_instance_spec,
    )?;
    ti::stateful_instance::arena::ensure_stateful_instance(root, arena, types);
    ti::type_annotation_binding::arena::set_origin(root, arena, types);
    Ok(())
}
