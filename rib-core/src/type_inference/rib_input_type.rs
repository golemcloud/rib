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
use crate::{try_visit_post_order_rev_mut, Expr, InferredExpr, RibCompilationError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// RibInputTypeInfo refers to the required global inputs to a RibScript
// with its type information. Example: `request` variable which should be of the type `Record`.
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

        let mut global_variables = HashMap::new();

        try_visit_post_order_rev_mut(&mut expr, &mut |expr| {
            if let Expr::Identifier {
                variable_id,
                inferred_type,
                ..
            } = &*expr
            {
                if variable_id.is_global() {
                    let analysed_type = WitType::try_from(inferred_type).map_err(|e| {
                        RibCompilationError::RibStaticAnalysisError(format!(
                            "failed to convert inferred type to analysed type: {e}"
                        ))
                    })?;

                    global_variables.insert(variable_id.name(), analysed_type);
                }
            }
            Ok::<(), RibCompilationError>(())
        })?;

        Ok(RibInputTypeInfo {
            types: global_variables,
        })
    }
}
