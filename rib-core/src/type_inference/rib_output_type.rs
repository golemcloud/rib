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
