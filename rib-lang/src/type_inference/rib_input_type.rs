use crate::wit_type::WitType;
use crate::{try_visit_post_order_rev_mut, Expr, InferredExpr, RibCompilationError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Declared environment bindings used by the script: each `env.<name>` is [`WitType::Str`].
/// The `<name>` matches the host / OS / [`crate::RibInput`] key exactly (e.g. `env.TOKEN_ID` → key `TOKEN_ID`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RibInputTypeInfo {
    pub types: HashMap<String, WitType>,
}
impl RibInputTypeInfo {
    pub fn get(&self, key: &str) -> Option<&WitType> {
        self.types.get(key)
    }

    pub fn empty() -> Self {
        RibInputTypeInfo {
            types: HashMap::new(),
        }
    }

    pub fn from_expr(
        inferred_expr: &InferredExpr,
    ) -> Result<RibInputTypeInfo, RibCompilationError> {
        let mut expr = inferred_expr.get_expr().clone();

        let mut types = HashMap::new();

        try_visit_post_order_rev_mut(&mut expr, &mut |expr| {
            if let Expr::SelectField {
                expr: inner,
                field,
                inferred_type,
                ..
            } = &*expr
            {
                if let Expr::Identifier {
                    variable_id,
                    inferred_type: _,
                    ..
                } = inner.as_ref()
                {
                    if variable_id.is_global() && variable_id.name() == "env" {
                        let analysed_type = WitType::try_from(inferred_type).map_err(|e| {
                            RibCompilationError::RibStaticAnalysisError(format!(
                                "failed to convert inferred type to wit type: {e}"
                            ))
                        })?;
                        types.insert(format!("env.{field}"), analysed_type);
                    }
                }
            }
            Ok::<(), RibCompilationError>(())
        })?;

        Ok(RibInputTypeInfo { types })
    }
}
