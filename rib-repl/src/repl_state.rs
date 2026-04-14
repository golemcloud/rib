use crate::instance_name_gen::ReplInstanceNameGen;
use crate::{ComponentFunctionInvoke, RawRibScript};
use rib::ValueAndType;
use rib::{InstructionId, RibCompiler};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock, RwLockReadGuard};

pub struct ReplState {
    rib_script: RwLock<RawRibScript>,
    worker_function_invoke: Arc<dyn ComponentFunctionInvoke + Sync + Send>,
    invocation_results: InvocationResultCache,
    last_executed_instruction: RwLock<Option<InstructionId>>,
    rib_compiler: RwLock<RibCompiler>,
    history_file_path: PathBuf,
    worker_name_gen: RwLock<ReplInstanceNameGen>,
}

impl ReplState {
    pub fn worker_function_invoke(&self) -> &Arc<dyn ComponentFunctionInvoke + Sync + Send> {
        &self.worker_function_invoke
    }

    pub fn invocation_results(&self) -> &InvocationResultCache {
        &self.invocation_results
    }

    pub fn update_cache(&self, instruction_id: InstructionId, result: Option<ValueAndType>) {
        self.invocation_results
            .results
            .write()
            .unwrap()
            .insert(instruction_id, result);
    }

    pub fn last_executed_instruction(&self) -> InstructionId {
        self.last_executed_instruction
            .read()
            .unwrap()
            .clone()
            .unwrap_or(InstructionId { index: 0 })
    }

    pub fn history_file_path(&self) -> &PathBuf {
        &self.history_file_path
    }

    // This reset is to ensure the rib compiler the REPL can reuse the previous
    // compilations (within the same session) instance names generated. i.e, before every compilation we reset the instance count,
    // and thereby, for the new script, the instance creation will end up reusing already generated instance names.
    pub fn reset_instance_count(&self) {
        self.worker_name_gen.write().unwrap().reset_instance_count();
    }

    pub fn generate_worker_name(&self) -> String {
        self.worker_name_gen.write().unwrap().generate_worker_name()
    }

    pub fn update_last_executed_instruction(&self, instruction_id: InstructionId) {
        *self.last_executed_instruction.write().unwrap() = Some(instruction_id);
    }

    pub fn clear(&self) {
        *self.rib_script.write().unwrap() = RawRibScript::default();
        *self.invocation_results.results.write().unwrap() = HashMap::new();
        *self.last_executed_instruction.write().unwrap() = None;
    }

    pub fn rib_script(&self) -> RwLockReadGuard<'_, RawRibScript> {
        self.rib_script.read().unwrap()
    }

    pub fn rib_compiler(&self) -> RwLockReadGuard<'_, RibCompiler> {
        self.rib_compiler.read().unwrap()
    }

    pub fn current_rib_program(&self) -> String {
        self.rib_script.read().unwrap().as_text()
    }

    pub fn update_rib(&self, rib: &str) {
        self.rib_script.write().unwrap().push(rib);
    }

    pub fn remove_last_rib_expression(&self) {
        self.rib_script.write().unwrap().pop();
    }

    pub fn new(
        worker_function_invoke: Arc<dyn ComponentFunctionInvoke + Sync + Send>,
        rib_compiler: RibCompiler,
        history_file: PathBuf,
    ) -> Self {
        Self {
            rib_script: RwLock::new(RawRibScript::default()),
            worker_function_invoke,
            invocation_results: InvocationResultCache {
                results: RwLock::new(HashMap::new()),
            },
            last_executed_instruction: RwLock::new(None),
            rib_compiler: RwLock::new(rib_compiler),
            history_file_path: history_file,
            worker_name_gen: RwLock::new(ReplInstanceNameGen::new()),
        }
    }
}

#[derive(Debug)]
pub struct InvocationResultCache {
    pub results: RwLock<HashMap<InstructionId, Option<ValueAndType>>>,
}

impl InvocationResultCache {
    pub fn get(&self, script_id: &InstructionId) -> Option<Option<ValueAndType>> {
        self.results.read().unwrap().get(script_id).cloned()
    }
}
