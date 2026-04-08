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

#![allow(clippy::large_enum_variant)]
#![allow(clippy::result_large_err)]

pub use call_type::*;
pub use compiler::*;
pub use expr::*;
pub use function_name::*;
pub use inferred_type::*;
pub use instance_type::*;
pub use interpreter::*;
pub use parser::type_name::TypeName;
pub use registry::*;
pub use text::*;
pub use type_checker::*;
pub use type_inference::*;
pub use type_parameter::*;
pub use value::Value;
pub use value_and_type::{IntoValue, IntoValueAndType, ValueAndType};
pub use variable_id::*;
pub use wasm_wave_text::*;

mod call_type;

mod profile;

pub mod wit_type;
mod compiler;
mod expr;
pub mod expr_arena;
mod function_name;
mod inferred_type;
mod instance_type;
mod interpreter;
mod parser;
mod registry;
mod rib_source_span;
mod rib_type_error;
mod text;
mod type_checker;
mod type_inference;
mod type_parameter;
mod type_parameter_parser;
mod type_refinement;
pub mod value;
pub mod value_and_type;
mod variable_id;
mod wasm_wave_text;
mod wave;

#[cfg(test)]
test_r::enable!();
