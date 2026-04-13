use crate::ValueAndType;
use std::collections::HashMap;

// Acts as the structure to hold the global input values to the Rib script
#[derive(Debug, Default, Clone)]
pub struct RibInput {
    pub input: HashMap<String, ValueAndType>,
}

impl RibInput {
    pub fn new(input: HashMap<String, ValueAndType>) -> RibInput {
        RibInput { input }
    }

    pub fn merge(&self, other: RibInput) -> RibInput {
        let mut cloned = self.clone();
        cloned.input.extend(other.input);
        cloned
    }
}
