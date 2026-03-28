use anyhow::Result;
use clap::Parser;
use golem_wasm::analysis::AnalysedExport;
use rib::{ComponentDependency, ComponentDependencyKey};
use rib_repl::{ReplComponentDependencies, RibDependencyManager, RibRepl, RibReplConfig};
use rib_repl_golem::{fetch_components, GolemWorkerFunctionInvoke};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "rib-repl", about = "Rib REPL with Golem integration")]
struct Cli {
    /// Path to the WASM component file (used to load type information).
    /// If not provided, component metadata is fetched from the Golem API.
    #[arg(short, long)]
    component: Option<PathBuf>,

    /// Component name (defaults to filename without extension when using --component)
    #[arg(short = 'n', long)]
    name: Option<String>,

    /// Golem API base URL
    #[arg(long, default_value = "http://localhost:9881")]
    golem_url: String,

    /// Application name in Golem
    #[arg(long)]
    app_name: String,

    /// Environment name in Golem
    #[arg(long, default_value = "default")]
    env_name: String,

    /// Agent ID, e.g. "HttpAgent(test)". Loads only this agent type's methods.
    #[arg(long)]
    agent_id: Option<String>,

    /// Auth token for the Golem API
    #[arg(long, env = "GOLEM_TOKEN")]
    token: Option<String>,
}

struct WasmFileDependencyManager {
    component_path: PathBuf,
    component_name: String,
}

#[async_trait::async_trait]
impl RibDependencyManager for WasmFileDependencyManager {
    async fn get_dependencies(&self) -> anyhow::Result<ReplComponentDependencies> {
        let exports = load_component_exports(&self.component_path)?;

        let key = ComponentDependencyKey {
            component_name: self.component_name.clone(),
            component_id: uuid::Uuid::new_v4(),
            component_revision: 0,
            root_package_name: None,
            root_package_version: None,
        };

        Ok(ReplComponentDependencies {
            component_dependencies: vec![ComponentDependency::new(key, exports)],
            custom_instance_spec: vec![],
        })
    }

    async fn add_component(
        &self,
        _source_path: &Path,
        _component_name: String,
    ) -> anyhow::Result<ComponentDependency> {
        anyhow::bail!("Dynamic component loading not supported in standalone mode")
    }
}

struct GolemApiDependencyManager {
    base_url: String,
    app_name: String,
    env_name: String,
    token: Option<String>,
    agent_id: Option<String>,
}

#[async_trait::async_trait]
impl RibDependencyManager for GolemApiDependencyManager {
    async fn get_dependencies(&self) -> anyhow::Result<ReplComponentDependencies> {
        let components =
            fetch_components(&self.base_url, &self.app_name, &self.env_name, self.token.as_deref())
                .await?;

        let deps = components
            .into_iter()
            .map(|c| {
                let key = ComponentDependencyKey {
                    component_name: c.component_name,
                    component_id: c.id,
                    component_revision: c.revision,
                    root_package_name: c.metadata.root_package_name,
                    root_package_version: c.metadata.root_package_version,
                };
                let agent_type_filter = self.agent_id.as_ref().map(|id| {
                    let id = id.trim();
                    id.find('(')
                        .map(|pos| id[..pos].trim().to_string())
                        .unwrap_or_else(|| id.to_string())
                });
                let agent_types_iter = c.metadata.agent_types.iter();
                let exports: Vec<golem_wasm::analysis::AnalysedExport> =
                    if let Some(ref filter) = agent_type_filter {
                        agent_types_iter
                            .filter(|at| at.type_name == *filter)
                            .map(|at| at.to_analysed_export())
                            .collect()
                    } else {
                        agent_types_iter
                            .map(|at| at.to_analysed_export())
                            .collect()
                    };
                ComponentDependency::new(key, exports)
            })
            .collect();

        Ok(ReplComponentDependencies {
            component_dependencies: deps,
            custom_instance_spec: vec![],
        })
    }

    async fn add_component(
        &self,
        _source_path: &Path,
        _component_name: String,
    ) -> anyhow::Result<ComponentDependency> {
        anyhow::bail!("Dynamic component loading not supported in API mode")
    }
}

fn load_component_exports(path: &Path) -> Result<Vec<AnalysedExport>> {
    let bytes = std::fs::read(path)?;
    let ctx = golem_wasm::analysis::wit_parser::WitAnalysisContext::new(&bytes)
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let exports = ctx
        .get_top_level_exports()
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    Ok(exports)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let dep_manager: Arc<dyn RibDependencyManager + Send + Sync> = if let Some(ref component_path) = cli.component
    {
        let component_name = cli.name.unwrap_or_else(|| {
            component_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        });
        Arc::new(WasmFileDependencyManager {
            component_path: component_path.clone(),
            component_name,
        })
    } else {
        Arc::new(GolemApiDependencyManager {
            base_url: cli.golem_url.clone(),
            app_name: cli.app_name.clone(),
            env_name: cli.env_name.clone(),
            token: cli.token.clone(),
            agent_id: cli.agent_id.clone(),
        })
    };

    let invoke = Arc::new(GolemWorkerFunctionInvoke::new(
        cli.golem_url,
        cli.app_name,
        cli.env_name,
        cli.token,
    ));

    let mut repl = RibRepl::bootstrap(RibReplConfig {
        history_file: None,
        dependency_manager: dep_manager,
        worker_function_invoke: invoke,
        printer: None,
        component_source: None,
        prompt: None,
        command_registry: None,
    })
    .await
    .map_err(|e| anyhow::anyhow!("{e:?}"))?;

    repl.run().await;

    Ok(())
}
