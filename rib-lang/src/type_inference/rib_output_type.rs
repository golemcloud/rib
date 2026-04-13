use crate::wit_type::WitType;
use crate::{InferredExpr, RibCompilationError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RibOutputTypeInfo {
    pub analysed_type: WitType,
}

impl RibOutputTypeInfo {
    pub fn from_expr(
        inferred_expr: &InferredExpr,
    ) -> Result<RibOutputTypeInfo, RibCompilationError> {
        let inferred_type = inferred_expr.get_expr().inferred_type();
        let analysed_type = WitType::try_from(&inferred_type).map_err(|e| {
            RibCompilationError::RibStaticAnalysisError(format!(
                "failed to convert inferred type to wit type: {e}"
            ))
        })?;

        Ok(RibOutputTypeInfo { analysed_type })
    }
}
