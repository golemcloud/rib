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

use crate::{ComponentDependencyKey, DynamicParsedFunctionName, Expr};
use crate::{FullyQualifiedResourceConstructor, VariableId};
use std::fmt::Display;

#[derive(Debug, Hash, PartialEq, Eq, Clone, Ord, PartialOrd)]
pub enum CallType {
    Function {
        component_info: Option<ComponentDependencyKey>,
        // as compilation progress the function call is expected to a instance_identifier
        // and will be always `Some`.
        instance_identifier: Option<Box<InstanceIdentifier>>,
        // TODO; a dynamic-parsed-function-name can be replaced by ParsedFunctionName
        // after the introduction of non-lazy resource constructor.
        function_name: DynamicParsedFunctionName,
    },
    VariantConstructor(String),
    EnumConstructor(String),
    InstanceCreation(InstanceCreationType),
}

// InstanceIdentifier holds the variables that are used to identify a worker or resource instance.
// Unlike InstanceCreationType, this type can be formed only after the instance is inferred
#[derive(Debug, Hash, PartialEq, Eq, Clone, Ord, PartialOrd)]
pub enum InstanceIdentifier {
    WitWorker {
        variable_id: Option<VariableId>,
        worker_name: Option<Box<Expr>>,
    },

    WitResource {
        variable_id: Option<VariableId>,
        worker_name: Option<Box<Expr>>,
        resource_name: String,
    },
}

impl InstanceIdentifier {
    pub fn worker_name_mut(&mut self) -> Option<&mut Box<Expr>> {
        match self {
            InstanceIdentifier::WitWorker { worker_name, .. } => worker_name.as_mut(),
            InstanceIdentifier::WitResource { worker_name, .. } => worker_name.as_mut(),
        }
    }
    pub fn worker_name(&self) -> Option<&Expr> {
        match self {
            InstanceIdentifier::WitWorker { worker_name, .. } => worker_name.as_deref(),
            InstanceIdentifier::WitResource { worker_name, .. } => worker_name.as_deref(),
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Ord, PartialOrd)]
pub enum InstanceCreationType {
    // A wit worker instance can be created without another module
    WitWorker {
        component_info: Option<ComponentDependencyKey>,
        worker_name: Option<Box<Expr>>,
    },
    // an instance type of the type wit-resource can only be part of
    // another instance (we call it module), which can be theoretically only be
    // a worker, but we don't restrict this in types, such that it will easily
    // handle nested wit resources
    WitResource {
        component_info: Option<ComponentDependencyKey>,
        // this module identifier during resource creation will be always a worker module, but we don't necessarily restrict
        // i.e, we do allow nested resource construction
        module: Option<InstanceIdentifier>,
        resource_name: FullyQualifiedResourceConstructor,
    },
}

impl InstanceCreationType {
    pub fn worker_name(&self) -> Option<Expr> {
        match self {
            InstanceCreationType::WitWorker { worker_name, .. } => worker_name.as_deref().cloned(),
            InstanceCreationType::WitResource { module, .. } => {
                let r = module.as_ref().and_then(|m| m.worker_name());
                r.cloned()
            }
        }
    }
}

impl CallType {
    pub fn function_name(&self) -> Option<DynamicParsedFunctionName> {
        match self {
            CallType::Function { function_name, .. } => Some(function_name.clone()),
            _ => None,
        }
    }
    pub fn worker_expr(&self) -> Option<&Expr> {
        match self {
            CallType::Function {
                instance_identifier,
                ..
            } => {
                let module = instance_identifier.as_ref()?;
                module.worker_name()
            }
            _ => None,
        }
    }

    pub fn function_call(
        function: DynamicParsedFunctionName,
        component_info: Option<ComponentDependencyKey>,
    ) -> CallType {
        CallType::Function {
            instance_identifier: None,
            function_name: function,
            component_info,
        }
    }

    pub fn function_call_with_worker(
        module: InstanceIdentifier,
        function: DynamicParsedFunctionName,
        component_info: Option<ComponentDependencyKey>,
    ) -> CallType {
        CallType::Function {
            instance_identifier: Some(Box::new(module)),
            function_name: function,
            component_info,
        }
    }

    pub fn is_resource_method(&self) -> bool {
        match self {
            CallType::Function { function_name, .. } => function_name
                .to_parsed_function_name()
                .function
                .resource_method_name()
                .is_some(),
            _ => false,
        }
    }
}

impl Display for CallType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CallType::Function { function_name, .. } => write!(f, "{function_name}"),
            CallType::VariantConstructor(name) => write!(f, "{name}"),
            CallType::EnumConstructor(name) => write!(f, "{name}"),
            CallType::InstanceCreation(instance_creation_type) => match instance_creation_type {
                InstanceCreationType::WitWorker { .. } => {
                    write!(f, "instance")
                }
                InstanceCreationType::WitResource { resource_name, .. } => {
                    write!(f, "{}", resource_name.resource_name)
                }
            },
        }
    }
}
