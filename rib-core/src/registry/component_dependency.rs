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

use crate::analysis::TypeEnum;
use crate::analysis::{AnalysedExport, TypeVariant};
use crate::{
    ComponentDependencyKey, Expr, FunctionDictionary, FunctionName, FunctionType,
    FunctionTypeRegistry, InstanceCreationType,
};

/// Single Wasm component: identity plus resolved export surface as a function dictionary.
#[derive(Debug, Default, Hash, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct ComponentDependency {
    pub key: ComponentDependencyKey,
    pub function_dictionary: FunctionDictionary,
}

impl ComponentDependency {
    pub fn from_wit_metadata(
        key: ComponentDependencyKey,
        exports: &[AnalysedExport],
    ) -> Result<Self, String> {
        let function_type_registry = FunctionTypeRegistry::from_export_metadata(exports);
        let function_dictionary =
            FunctionDictionary::from_function_type_registry(&function_type_registry)?;
        Ok(ComponentDependency {
            key,
            function_dictionary,
        })
    }

    pub fn get_variants(&self) -> Vec<TypeVariant> {
        self.function_dictionary.get_all_variants()
    }

    pub fn get_enums(&self) -> Vec<TypeEnum> {
        self.function_dictionary.get_all_enums()
    }

    pub fn get_function_type(
        &self,
        function_name: &FunctionName,
    ) -> Result<(ComponentDependencyKey, FunctionType), String> {
        let types: Vec<&FunctionType> = self
            .function_dictionary
            .name_and_types
            .iter()
            .filter_map(|(f_name, function_type)| {
                if f_name == function_name {
                    Some(function_type)
                } else {
                    None
                }
            })
            .collect();

        if types.is_empty() {
            Err("unknown function".to_string())
        } else {
            Ok((self.key.clone(), types[0].clone()))
        }
    }

    /// No-op; kept so narrowing call sites stay stable.
    pub fn narrow_to_component(&mut self, _component_dependency_key: &ComponentDependencyKey) {}

    pub fn function_dictionary(&self) -> Vec<&FunctionDictionary> {
        vec![&self.function_dictionary]
    }

    pub fn get_worker_instance_type(
        &self,
        worker_name: Option<Expr>,
    ) -> Result<InstanceCreationType, String> {
        Ok(InstanceCreationType::WitWorker {
            component_info: None,
            worker_name: worker_name.map(Box::new),
        })
    }
}
