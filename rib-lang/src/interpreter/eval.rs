use crate::{
    DefaultWorkerNameGenerator, Expr, GenerateInstanceName, RibCompilationError, RibCompiler,
    RibCompilerConfig, RibComponentFunctionInvoke, RibInput, RibResult, RibRuntimeError,
};
use std::sync::Arc;

pub struct RibEvalConfig {
    compiler_config: RibCompilerConfig,
    rib_input: RibInput,
    function_invoke: Arc<dyn RibComponentFunctionInvoke + Sync + Send>,
    generate_instance_name: Arc<dyn GenerateInstanceName + Sync + Send>,
}

impl RibEvalConfig {
    pub fn new(
        compiler_config: RibCompilerConfig,
        rib_input: RibInput,
        function_invoke: Arc<dyn RibComponentFunctionInvoke + Sync + Send>,
        generate_worker_name: Option<Arc<dyn GenerateInstanceName + Sync + Send>>,
    ) -> Self {
        RibEvalConfig {
            compiler_config,
            rib_input,
            function_invoke,
            generate_instance_name: generate_worker_name
                .unwrap_or_else(|| Arc::new(DefaultWorkerNameGenerator)),
        }
    }
}

pub struct RibEvaluator {
    pub config: RibEvalConfig,
}

impl RibEvaluator {
    pub fn new(config: RibEvalConfig) -> Self {
        RibEvaluator { config }
    }

    pub async fn eval(self, rib: &str) -> Result<RibResult, RibEvaluationError> {
        let expr = Expr::from_text(rib).map_err(RibEvaluationError::ParseError)?;
        let config = self.config.compiler_config;
        let compiler = RibCompiler::new(config);
        let compiled = compiler.compile(expr.clone())?;

        let result = crate::interpret(
            compiled.byte_code,
            self.config.rib_input,
            self.config.function_invoke,
            Some(self.config.generate_instance_name.clone()),
        )
        .await?;

        Ok(result)
    }
}

#[derive(Debug)]
pub enum RibEvaluationError {
    ParseError(String),
    CompileError(RibCompilationError),
    RuntimeError(RibRuntimeError),
}

impl From<RibCompilationError> for RibEvaluationError {
    fn from(error: RibCompilationError) -> Self {
        RibEvaluationError::CompileError(error)
    }
}

impl From<RibRuntimeError> for RibEvaluationError {
    fn from(error: RibRuntimeError) -> Self {
        RibEvaluationError::RuntimeError(error)
    }
}
