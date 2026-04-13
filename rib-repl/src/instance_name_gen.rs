use crate::repl_state::ReplState;
use rib::GenerateWorkerName;
use std::collections::HashMap;
use std::sync::Arc;

// When it comes to REPL, unlike the regular Rib execution,
// it recompiles from the start anytime to figure out the types
// however it shouldn't result in a variable having a different instance of worker,
// meaning different worker name. Rib internally generates a worker name at compile time
// for instances without worker-name, i.e, `instance()` compared to `instance("my-worker")`.
pub struct ReplInstanceNameGen {
    pub instance_count: u64,
    pub worker_name_cache: HashMap<u64, String>,
}

impl ReplInstanceNameGen {
    pub fn new() -> Self {
        ReplInstanceNameGen {
            instance_count: 0,
            worker_name_cache: HashMap::new(),
        }
    }

    // A reset prior to any compilation will only reset the instance count,
    // holding on to the cache.
    // The cache is active throughout a REPL session.
    pub fn reset_instance_count(&mut self) {
        self.instance_count = 0;
    }

    pub fn generate_worker_name(&mut self) -> String {
        self.instance_count += 1;

        if let Some(name) = self.worker_name_cache.get(&self.instance_count) {
            return name.clone();
        }
        let uuid = uuid::Uuid::new_v4();
        let name = format!("worker-{}-{}", self.instance_count, uuid);
        self.worker_name_cache
            .insert(self.instance_count, name.clone());
        name
    }
}

pub struct DynamicWorkerGen {
    repl_state: Arc<ReplState>,
}

impl DynamicWorkerGen {
    pub fn new(repl_state: Arc<ReplState>) -> Self {
        DynamicWorkerGen { repl_state }
    }
}

impl GenerateWorkerName for DynamicWorkerGen {
    fn generate_worker_name(&self) -> String {
        self.repl_state.generate_worker_name()
    }
}
