//! Runs the former “initial phase” passes on an [`crate::expr_arena::ExprArena`] after [`crate::expr_arena::lower`],
//! in the same order as the historical `Expr` pipeline.

use crate::expr_arena::{ExprArena, ExprId, TypeTable};
use crate::rib_type_error::RibTypeErrorInternal;
use crate::type_inference as ti;
use crate::{ComponentDependency, CustomInstanceSpec};
use std::sync::Arc;

pub fn run_initial_binding_and_instance_phases(
    root: ExprId,
    arena: &mut ExprArena,
    types: &mut TypeTable,
    component: Arc<ComponentDependency>,
    custom_instance_spec: &[CustomInstanceSpec],
) -> Result<(), RibTypeErrorInternal> {
    ti::type_annotation_binding::bind_type_annotations(root, arena, types);
    ti::variable_binding::bind_variables_of_list_comprehension(root, arena, types);
    ti::variable_binding::bind_variables_of_list_reduce(root, arena, types);
    ti::variable_binding::bind_variables_of_pattern_match(root, arena, types);
    ti::variable_binding::bind_variables_of_let_assignment(root, arena, types);
    ti::identify_instance_creation::identify_instance_creation(
        root,
        arena,
        types,
        component,
        custom_instance_spec,
    )?;
    ti::stateful_instance::ensure_stateful_instance(root, arena, types);
    ti::type_annotation_binding::set_origin(root, arena, types);
    Ok(())
}
