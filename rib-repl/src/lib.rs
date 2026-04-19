#![allow(clippy::large_enum_variant)]

// Re-exported for embedders (e.g. Wasmtime CLI) that implement Rib traits without
// adding their own `anyhow` / `uuid` dependency edges.
pub use anyhow;
pub use uuid;

/// Core Rib compiler crate (`rib-lang`); pulled in transitively so embedders only add `rib-repl`.
pub use rib;

/// [`rib::wit_type`] at the crate root so embedders avoid a direct `rib` / `rib-lang` dependency path.
pub use rib::wit_type;

pub use rib::{
    ComponentDependency, ComponentDependencyKey, ParsedFunctionName, ParsedFunctionSite,
};

pub use command::*;
pub use dependency_manager::*;
pub use invoke::*;
pub use raw::*;
pub use repl_bootstrap_error::*;
pub use repl_printer::*;
pub use rib_context::*;
pub use rib_execution_error::*;
pub use rib_repl::*;
pub use runtime_value::{
    try_runtime_to_value_and_type, try_value_and_type_to_runtime, tuple_element_runtime_types,
    RuntimeValue,
};

mod command;
mod compiler;
mod dependency_manager;
mod eval;
mod instance_name_gen;
mod invoke;
mod raw;
mod repl_bootstrap_error;
mod repl_printer;
mod repl_state;
mod rib_context;
mod rib_edit;
mod rib_execution_error;
mod rib_repl;
mod runtime_value;
mod value_generator;

#[cfg(test)]
test_r::enable!();
