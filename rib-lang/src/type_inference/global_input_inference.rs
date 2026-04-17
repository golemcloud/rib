//! After [`crate::type_inference::type_pull_up`], pin `env.*` to [`crate::InferredType::string`]
//! and validate that no other global identifiers appear.

use crate::expr_arena::{ExprArena, ExprId, TypeTable};
use crate::rib_type_error::RibTypeErrorInternal;
use crate::type_inference::env_namespace;

pub fn infer_global_inputs(
    root: ExprId,
    arena: &ExprArena,
    types: &mut TypeTable,
) -> Result<(), RibTypeErrorInternal> {
    env_namespace::apply_env_namespace_types(root, arena, types);
    env_namespace::validate_global_identifiers(root, arena)?;
    Ok(())
}
