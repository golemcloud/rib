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

use crate::compiler::worker_functions_in_rib::WorkerFunctionsInRib;
use crate::{RibByteCode, RibInputTypeInfo, RibOutputTypeInfo};

#[derive(Debug, Clone)]
pub struct CompilerOutput {
    pub worker_invoke_calls: Option<WorkerFunctionsInRib>,
    pub byte_code: RibByteCode,
    pub rib_input_type_info: RibInputTypeInfo,
    // Optional to keep backward compatible as compiler output information
    // for some existing Rib in persistence store doesn't have this info.
    // This is optional mainly to support the proto conversions.
    // At the API level, if we have access to expr, whenever this field is optional
    // we can compile the expression again and get the output type info
    pub rib_output_type_info: Option<RibOutputTypeInfo>,
}
