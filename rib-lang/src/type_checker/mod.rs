pub(crate) use exhaustive_pattern_match::*;

pub use path::*;
/// Type checking on lowered `(ExprId, ExprArena, TypeTable)`; see [`checker::type_check`].
pub(crate) mod checker;
mod exhaustive_pattern_match;
mod path;
mod unresolved_types;
