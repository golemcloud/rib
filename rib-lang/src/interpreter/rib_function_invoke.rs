use crate::wit_type::WitType;
use crate::ValueAndType;
use crate::{ComponentDependencyKey, InstructionId};
use async_trait::async_trait;

#[async_trait]
pub trait RibComponentFunctionInvoke {
    async fn invoke(
        &self,
        component_dependency_key: ComponentDependencyKey,
        instruction_id: &InstructionId,
        worker_name: EvaluatedWorkerName,
        function_name: EvaluatedFqFn,
        args: EvaluatedFnArgs,
        return_type: Option<WitType>,
    ) -> RibFunctionInvokeResult;
}

pub type RibFunctionInvokeResult =
    Result<Option<ValueAndType>, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug)]
pub struct EvaluatedFqFn(pub String);

#[derive(Clone)]
pub struct EvaluatedWorkerName(pub String);

pub struct EvaluatedFnArgs(pub Vec<ValueAndType>);
