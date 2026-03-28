use golem_wasm::analysis::{
    AnalysedExport, AnalysedFunction, AnalysedFunctionParameter, AnalysedFunctionResult,
    AnalysedInstance, AnalysedType,
};
use reqwest::Client;
use serde::Deserialize;
use uuid::Uuid;

const LOCAL_WELL_KNOWN_TOKEN: &str = "5c832d93-ff85-4a8f-9803-513950fdfdb1";
const LOCAL_ACCOUNT_ID: &str = "51de7d7d-f286-49aa-b79a-96022f7e2df9";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Application {
    id: Uuid,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Environment {
    id: Uuid,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageComponentDto {
    values: Vec<ComponentDto>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentDto {
    pub id: Uuid,
    pub revision: u64,
    pub component_name: String,
    pub metadata: ComponentMetadata,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentMetadata {
    pub exports: Vec<AnalysedExport>,
    pub root_package_name: Option<String>,
    pub root_package_version: Option<String>,
    #[serde(default)]
    pub agent_types: Vec<AgentTypeDto>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTypeDto {
    pub type_name: String,
    pub constructor: AgentConstructorDto,
    pub methods: Vec<AgentMethodDto>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentConstructorDto {
    pub input_schema: DataSchema,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMethodDto {
    pub name: String,
    pub input_schema: DataSchema,
    pub output_schema: DataSchema,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataSchema {
    pub elements: Vec<NamedElementSchema>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedElementSchema {
    pub name: String,
    pub schema: ElementSchema,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum ElementSchema {
    ComponentModel(ComponentModelElementSchema),
    UnstructuredBinary(serde_json::Value),
    UnstructuredText(serde_json::Value),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentModelElementSchema {
    pub element_type: AnalysedType,
}

impl AgentTypeDto {
    pub fn to_analysed_export(&self) -> AnalysedExport {
        let functions = self
            .methods
            .iter()
            .map(|method| {
                let parameters = method
                    .input_schema
                    .elements
                    .iter()
                    .filter_map(|elem| match &elem.schema {
                        ElementSchema::ComponentModel(cm) => Some(AnalysedFunctionParameter {
                            name: elem.name.clone(),
                            typ: cm.element_type.clone(),
                        }),
                        _ => None,
                    })
                    .collect();

                let result = method
                    .output_schema
                    .elements
                    .first()
                    .and_then(|elem| match &elem.schema {
                        ElementSchema::ComponentModel(cm) => {
                            Some(AnalysedFunctionResult { typ: cm.element_type.clone() })
                        }
                        _ => None,
                    });

                AnalysedFunction {
                    name: method.name.replace('_', "-"),
                    parameters,
                    result,
                }
            })
            .collect();

        AnalysedExport::Instance(AnalysedInstance {
            name: format!("golem:agent/{}", self.type_name),
            functions,
        })
    }
}

pub async fn fetch_components(
    base_url: &str,
    app_name: &str,
    env_name: &str,
    token: Option<&str>,
) -> anyhow::Result<Vec<ComponentDto>> {
    let client = Client::new();
    let effective_token = token.unwrap_or(LOCAL_WELL_KNOWN_TOKEN);

    // Step 1: GET /v1/accounts/{account_id}/apps/{app_name}
    let app_url = format!(
        "{}/v1/accounts/{}/apps/{}",
        base_url, LOCAL_ACCOUNT_ID, app_name
    );
    let app: Application = client
        .get(&app_url)
        .bearer_auth(effective_token)
        .send()
        .await?
        .error_for_status()
        .map_err(|e| anyhow::anyhow!("Failed to fetch application '{}': {}", app_name, e))?
        .json()
        .await?;

    // Step 2: GET /v1/apps/{application_id}/envs/{env_name}
    let env_url = format!("{}/v1/apps/{}/envs/{}", base_url, app.id, env_name);
    let env: Environment = client
        .get(&env_url)
        .bearer_auth(effective_token)
        .send()
        .await?
        .error_for_status()
        .map_err(|e| anyhow::anyhow!("Failed to fetch environment '{}': {}", env_name, e))?
        .json()
        .await?;

    // Step 3: GET /v1/envs/{environment_id}/components
    let components_url = format!("{}/v1/envs/{}/components", base_url, env.id);
    let page: PageComponentDto = client
        .get(&components_url)
        .bearer_auth(effective_token)
        .send()
        .await?
        .error_for_status()
        .map_err(|e| anyhow::anyhow!("Failed to fetch components: {}", e))?
        .json()
        .await?;

    Ok(page.values)
}
