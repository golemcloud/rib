use crate::repl_state::ReplState;
use crate::rib_val::{try_rib_val_to_value_and_type, try_value_and_type_to_rib_val, RibVal};
use async_trait::async_trait;
use rib::wit_type::WitType;
use rib::ValueAndType;
use rib::{
    ComponentDependencyKey, EvaluatedFnArgs, EvaluatedFqFn, EvaluatedWorkerName, InstructionId,
    RibComponentFunctionInvoke, RibFunctionInvokeResult,
};
use std::sync::Arc;
use uuid::Uuid;

fn io_other_box(err: impl std::fmt::Display) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(std::io::Error::new(
        std::io::ErrorKind::Other,
        err.to_string(),
    ))
}

#[async_trait]
pub trait ComponentFunctionInvoke {
    async fn invoke(
        &self,
        component_id: Uuid,
        component_name: &str,
        worker_name: &str,
        function_name: &str,
        args: Vec<RibVal>,
        return_type: Option<WitType>,
    ) -> anyhow::Result<Option<RibVal>>;
}

// Note: Currently, the Rib interpreter supports only one component, so the
// `RibFunctionInvoke` trait in the `golem-rib` module does not include `component_id` in
// the `invoke` arguments. It only requires the optional instance name, function name, and arguments.
// Once multi-component support is added, the trait will be updated to include `component_id`,
// and we can use it directly instead of `WorkerFunctionInvoke` in the `golem-rib-repl` module.
pub(crate) struct ReplRibFunctionInvoke {
    repl_state: Arc<ReplState>,
}

impl ReplRibFunctionInvoke {
    pub fn new(repl_state: Arc<ReplState>) -> Self {
        Self { repl_state }
    }

    fn get_cached_result(&self, instruction_id: &InstructionId) -> Option<Option<ValueAndType>> {
        // If the current instruction index is greater than the last played index result,
        // then we shouldn't use the cache result no matter what.
        // This check is important because without this, loops end up reusing the cached invocation result
        if instruction_id.index > self.repl_state.last_executed_instruction().index {
            None
        } else {
            self.repl_state.invocation_results().get(instruction_id)
        }
    }
}

#[async_trait]
impl RibComponentFunctionInvoke for ReplRibFunctionInvoke {
    async fn invoke(
        &self,
        component_dependency: ComponentDependencyKey,
        instruction_id: &InstructionId,
        worker_name: EvaluatedWorkerName,
        function_name: EvaluatedFqFn,
        args: EvaluatedFnArgs,
        return_type: Option<WitType>,
    ) -> RibFunctionInvokeResult {
        match self.get_cached_result(instruction_id) {
            Some(result) => Ok(result),
            None => {
                let return_ty = return_type.clone();
                let mut args_rt = Vec::with_capacity(args.0.len());
                for a in &args.0 {
                    args_rt.push(try_value_and_type_to_rib_val(a).map_err(|e| io_other_box(&e))?);
                }

                let rib_invocation_result = self
                    .repl_state
                    .worker_function_invoke()
                    .invoke(
                        component_dependency.component_id,
                        &component_dependency.component_name,
                        &worker_name.0,
                        &function_name.0,
                        args_rt,
                        return_type,
                    )
                    .await;

                match rib_invocation_result {
                    Ok(result) => {
                        let mapped: Option<ValueAndType> = match (result, return_ty) {
                            (None, _) => None,
                            (Some(rv), Some(ty)) => Some(
                                try_rib_val_to_value_and_type(&rv, &ty)
                                    .map_err(|e| io_other_box(&e))?,
                            ),
                            (Some(_), None) => {
                                return Err(io_other_box(
                                    "host returned a value but call has no return type",
                                ));
                            }
                        };

                        self.repl_state
                            .update_cache(instruction_id.clone(), mapped.clone());

                        Ok(mapped)
                    }
                    Err(err) => Err(err.into()),
                }
            }
        }
    }
}
