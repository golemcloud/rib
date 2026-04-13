use crate::wit_type::TypeEnum;
use crate::wit_type::{TypeVariant, WitExport};
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
        exports: &[WitExport],
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
