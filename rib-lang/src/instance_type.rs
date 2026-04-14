use crate::parser::PackageName;
use crate::type_parameter::InterfaceName;
use crate::FunctionName;
use crate::{
    ComponentDependency, ComponentDependencyKey, Expr, FullyQualifiedResourceConstructor,
    FunctionDictionary, FunctionType, ResourceMethodDictionary,
};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use std::fmt::Debug;
use std::ops::Deref;

// `InstanceType` will be the type (`InferredType`) of the variable associated with creation of an instance
// `InstanceType` is structured to help with compilation logic better. Example: a random `instance()` call
// is of type `Global` to begin with and as soon as method invocations becomes a real function call,
// the type of instance becomes more and more precise.
//
// Please look at `InstanceCreationType`
// for a tangible view on the fact that an instance can be either worker or a resource.
#[derive(Debug, Hash, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub enum InstanceType {
    // Worker instance: one component's exports (all packages/interfaces merged in the dictionary)
    Global {
        worker_name: Option<Box<Expr>>,
        /// Shared across all worker-instance types for one compile — avoids cloning [`FunctionDictionary`] per node.
        component: Arc<ComponentDependency>,
    },

    // Holds the resource creation and the functions in the resource
    // that may or may not be addressed
    Resource {
        analysed_resource_id: u64,
        analysed_resource_mode: u8,
        worker_name: Option<Box<Expr>>,
        package_name: Option<PackageName>,
        interface_name: Option<InterfaceName>,
        resource_constructor: String,
        resource_args: Vec<Expr>,
        component_dependency_key: ComponentDependencyKey,
        resource_method_dictionary: ResourceMethodDictionary,
    },
}

impl InstanceType {
    pub fn narrow_to_single_component(
        &mut self,
        component_dependency_key: &ComponentDependencyKey,
    ) {
        match self {
            InstanceType::Global { component, .. } => {
                Arc::make_mut(component).narrow_to_component(component_dependency_key);
            }
            // A resource is already narrowed down to a component
            InstanceType::Resource { .. } => {}
        }
    }

    pub fn set_worker_name(&mut self, worker_name: Expr) {
        match self {
            InstanceType::Global {
                worker_name: wn, ..
            } => {
                *wn = Some(Box::new(worker_name));
            }
            InstanceType::Resource {
                worker_name: wn, ..
            } => {
                *wn = Some(Box::new(worker_name));
            }
        }
    }

    pub fn worker_mut(&mut self) -> Option<&mut Box<Expr>> {
        match self {
            InstanceType::Global { worker_name, .. } => worker_name.as_mut(),
            InstanceType::Resource { worker_name, .. } => worker_name.as_mut(),
        }
    }

    pub fn worker(&self) -> Option<&Expr> {
        match self {
            InstanceType::Global { worker_name, .. } => worker_name.as_ref().map(|v| v.deref()),
            InstanceType::Resource { worker_name, .. } => worker_name.as_ref().map(|v| v.deref()),
        }
    }

    pub fn get_resource_instance_type(
        &self,
        fully_qualified_resource_constructor: FullyQualifiedResourceConstructor,
        resource_args: Vec<Expr>,
        worker_name: Option<Box<Expr>>,
        analysed_resource_id: u64,
        analysed_resource_mode: u8,
    ) -> Result<InstanceType, String> {
        let interface_name = fully_qualified_resource_constructor.interface_name.clone();
        let package_name = fully_qualified_resource_constructor.package_name.clone();
        let resource_constructor_name = fully_qualified_resource_constructor.resource_name.clone();

        let dependencies = self.component_dependency();
        let mut resource_method_dict = BTreeMap::new();

        for (name, typ) in dependencies.function_dictionary.name_and_types.iter() {
            if let FunctionName::ResourceMethod(resource_method) = name {
                if resource_method.resource_name == resource_constructor_name
                    && resource_method.interface_name == interface_name
                    && resource_method.package_name == package_name
                {
                    resource_method_dict.insert(resource_method.clone(), typ.clone());
                }
            }
        }

        let component_dependency_key = dependencies.key.clone();
        let resource_methods = resource_method_dict;

        if !resource_methods.is_empty() {
            let resource_method_dictionary = ResourceMethodDictionary {
                map: resource_methods,
            };

            Ok(InstanceType::Resource {
                worker_name,
                package_name,
                interface_name,
                resource_constructor: resource_constructor_name,
                resource_args,
                component_dependency_key,
                resource_method_dictionary,
                analysed_resource_id,
                analysed_resource_mode,
            })
        } else {
            Err(format!(
                "No components found have the resource constructor '{resource_constructor_name}'"
            ))
        }
    }

    pub fn interface_name(&self) -> Option<InterfaceName> {
        match self {
            InstanceType::Global { .. } => None,
            InstanceType::Resource { interface_name, .. } => interface_name.clone(),
        }
    }

    pub fn package_name(&self) -> Option<PackageName> {
        match self {
            InstanceType::Global { .. } => None,
            InstanceType::Resource { package_name, .. } => package_name.clone(),
        }
    }

    pub fn worker_name(&self) -> Option<Box<Expr>> {
        match self {
            InstanceType::Global { worker_name, .. } => worker_name.clone(),
            InstanceType::Resource { worker_name, .. } => worker_name.clone(),
        }
    }

    pub fn get_function(
        &self,
        method_name: &str,
    ) -> Result<(ComponentDependencyKey, Function), String> {
        search_function_in_instance(self, method_name)
    }

    // A flattened list of all resource methods
    pub fn resource_method_dictionary(&self) -> FunctionDictionary {
        let name_and_types = self
            .component_dependency()
            .function_dictionary
            .name_and_types
            .iter()
            .filter(|(f, _)| matches!(f, FunctionName::ResourceMethod(_)))
            .map(|(f, t)| (f.clone(), t.clone()))
            .collect();

        FunctionDictionary { name_and_types }
    }

    pub fn function_dict_without_resource_methods(&self) -> FunctionDictionary {
        let name_and_types = self
            .component_dependency()
            .function_dictionary
            .name_and_types
            .iter()
            .filter(|(f, _)| {
                !matches!(f, FunctionName::ResourceMethod(_))
                    && !matches!(f, FunctionName::Variant(_))
                    && !matches!(f, FunctionName::Enum(_))
            })
            .map(|(f, t)| (f.clone(), t.clone()))
            .collect();

        FunctionDictionary { name_and_types }
    }

    pub fn component_dependency(&self) -> ComponentDependency {
        match self {
            InstanceType::Global { component, .. } => (**component).clone(),
            InstanceType::Resource {
                resource_method_dictionary,
                component_dependency_key,
                ..
            } => {
                let function_dictionary = FunctionDictionary::from(resource_method_dictionary);

                ComponentDependency {
                    key: component_dependency_key.clone(),
                    function_dictionary,
                }
            }
        }
    }

    pub fn from(
        dependency: Arc<ComponentDependency>,
        worker_name: Option<&Expr>,
    ) -> Result<InstanceType, String> {
        Ok(InstanceType::Global {
            worker_name: worker_name.cloned().map(Box::new),
            component: dependency,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Function {
    pub function_name: FunctionName,
    pub function_type: FunctionType,
}

fn search_function_in_instance(
    instance: &InstanceType,
    function_name: &str,
) -> Result<(ComponentDependencyKey, Function), String> {
    let dependencies = instance.component_dependency();
    let function_dictionary = &dependencies.function_dictionary;
    let key = dependencies.key.clone();

    let functions: Vec<&(FunctionName, FunctionType)> = function_dictionary
        .name_and_types
        .iter()
        .filter(|(f, _)| f.name() == function_name)
        .collect();

    if functions.is_empty() {
        return Err(format!("function '{function_name}' not found"));
    }

    let mut package_map: HashMap<Option<PackageName>, HashSet<Option<InterfaceName>>> =
        HashMap::new();

    for (fqfn, _) in &functions {
        package_map
            .entry(fqfn.package_name())
            .or_default()
            .insert(fqfn.interface_name());
    }

    match package_map.len() {
        1 => {
            let interfaces = package_map.values().flatten().cloned().collect();
            let function = search_function_in_single_package(interfaces, functions, function_name)?;
            Ok((key, function))
        }
        _ => {
            let function = search_function_in_multiple_packages(function_name, package_map)?;
            Ok((key, function))
        }
    }
}

fn search_function_in_single_package(
    interfaces: HashSet<Option<InterfaceName>>,
    functions: Vec<&(FunctionName, FunctionType)>,
    function_name: &str,
) -> Result<Function, String> {
    if interfaces.len() == 1 {
        let (fqfn, ftype) = &functions[0];
        Ok(Function {
            function_name: fqfn.clone(),
            function_type: ftype.clone(),
        })
    } else {
        let mut interfaces = interfaces
            .into_iter()
            .filter_map(|iface| iface.map(|i| i.name))
            .collect::<Vec<_>>();

        interfaces.sort();

        Err(format!(
            "multiple interfaces contain function '{function_name}'. Rib does not currently support disambiguating worker method names across interfaces. interfaces: {}",
            interfaces.join(", ")
        ))
    }
}

fn search_function_in_multiple_packages(
    function_name: &str,
    package_map: HashMap<Option<PackageName>, HashSet<Option<InterfaceName>>>,
) -> Result<Function, String> {
    let mut error_msg = format!(
        "function '{function_name}' exists in multiple packages. Rib does not currently support disambiguating worker method names in this case. Conflicting exports: "
    );

    let mut package_interface_list = package_map
        .into_iter()
        .filter_map(|(pkg, interfaces)| {
            pkg.map(|p| {
                let mut interface_list = interfaces
                    .into_iter()
                    .filter_map(|iface| iface.map(|i| i.name))
                    .collect::<Vec<_>>();

                interface_list.sort();

                if interface_list.is_empty() {
                    format!("{p}")
                } else {
                    format!("{} (interfaces: {})", p, interface_list.join(", "))
                }
            })
        })
        .collect::<Vec<_>>();

    package_interface_list.sort();

    error_msg.push_str(&package_interface_list.join(", "));
    Err(error_msg)
}
