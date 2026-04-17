//! Resolve `env.<name>` at run time: the `<name>` segment is the **exact** key used in
//! [`RibInput::input`] and in [`std::env::var`] (no alternate spellings or normalizations).

use crate::value::Value;
use crate::wit_type::builders::str;
use crate::ValueAndType;

use super::RibInput;

/// Read a string: [`RibInput`] overrides (tests / hosts), then the process environment, same key as in Rib.
pub fn resolve_env_string(field: &str, rib_input: &RibInput) -> String {
    if let Some(vnt) = rib_input.input.get(field) {
        if let Value::String(s) = &vnt.value {
            return s.clone();
        }
    }
    std::env::var(field).unwrap_or_default()
}

pub fn resolve_env_value_and_type(field: &str, rib_input: &RibInput) -> ValueAndType {
    ValueAndType::new(Value::String(resolve_env_string(field, rib_input)), str())
}
