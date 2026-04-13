use crate::wit_type::WitType;

#[derive(Clone, Debug)]
pub struct CustomInstanceSpec {
    pub instance_name: String,
    pub parameter_types: Vec<WitType>,
}

impl CustomInstanceSpec {
    /// Allows instance creation under a custom name (not only `instance`) with typed parameters.
    pub fn new(instance_name: String, parameter_types: Vec<WitType>) -> Self {
        CustomInstanceSpec {
            instance_name,
            parameter_types,
        }
    }
}
