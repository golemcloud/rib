use crate::wit_type::WitType;
use crate::{ComponentDependency, FunctionName, InferredExpr, RibCompilationError};

// An easier data type that focus just on the side effecting function calls in Rib script.
// These will not include variant or enum calls, that were originally
// tagged as functions before compilation.
// This is why we need a fully inferred Rib (fully compiled rib),
// which has specific details, along with original type registry to construct this data.
// These function calls are indeed worker invoke calls and nothing else.
// If Rib has inbuilt function support, those will not be included here either.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideEffectFunctions {
    pub function_calls: Vec<SideEffectFunctionSignature>,
}

impl SideEffectFunctions {
    pub fn from_inferred_expr(
        inferred_expr: &InferredExpr,
        component_dependency: &ComponentDependency,
    ) -> Result<Option<SideEffectFunctions>, RibCompilationError> {
        let worker_invoke_registry_keys = inferred_expr.worker_invoke_registry_keys();

        let mut function_calls = vec![];

        for key in worker_invoke_registry_keys {
            let (_, function_type) = component_dependency
                .get_function_type(&key)
                .map_err(|e| RibCompilationError::RibStaticAnalysisError(e.to_string()))?;

            let function_call_in_rib = SideEffectFunctionSignature {
                function_name: key,
                parameter_types: function_type
                    .parameter_types
                    .iter()
                    .map(|param| WitType::try_from(param).unwrap())
                    .collect(),
                return_type: function_type
                    .return_type
                    .as_ref()
                    .map(|return_type| WitType::try_from(return_type).unwrap()),
            };

            function_calls.push(function_call_in_rib)
        }

        if function_calls.is_empty() {
            Ok(None)
        } else {
            Ok(Some(SideEffectFunctions { function_calls }))
        }
    }
}

// The type of a function call with worker (ephmeral or durable) in Rib script
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideEffectFunctionSignature {
    pub function_name: FunctionName,
    pub parameter_types: Vec<WitType>,
    pub return_type: Option<WitType>,
}
