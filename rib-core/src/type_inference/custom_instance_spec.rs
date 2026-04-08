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

use crate::analysis::AnalysedType;

#[derive(Clone, Debug)]
pub struct CustomInstanceSpec {
    pub instance_name: String,
    pub parameter_types: Vec<AnalysedType>,
}

impl CustomInstanceSpec {
    /// Allows instance creation under a custom name (not only `instance`) with typed parameters.
    pub fn new(instance_name: String, parameter_types: Vec<AnalysedType>) -> Self {
        CustomInstanceSpec {
            instance_name,
            parameter_types,
        }
    }
}
