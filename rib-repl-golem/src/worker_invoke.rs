use async_trait::async_trait;
use golem_wasm::analysis::AnalysedType;
use golem_wasm::json::ValueAndTypeJsonExtensions;
use golem_wasm::ValueAndType;
use reqwest::Client;
use rib_repl::WorkerFunctionInvoke;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub struct GolemWorkerFunctionInvoke {
    client: Client,
    base_url: String,
    app_name: String,
    env_name: String,
    token: Option<String>,
}

const LOCAL_WELL_KNOWN_TOKEN: &str = "5c832d93-ff85-4a8f-9803-513950fdfdb1";

impl GolemWorkerFunctionInvoke {
    pub fn new(
        base_url: String,
        app_name: String,
        env_name: String,
        token: Option<String>,
    ) -> Self {
        Self {
            client: Client::new(),
            base_url,
            app_name,
            env_name,
            token,
        }
    }

    fn effective_token(&self) -> &str {
        self.token.as_deref().unwrap_or(LOCAL_WELL_KNOWN_TOKEN)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentInvocationRequest {
    app_name: String,
    env_name: String,
    agent_type_name: String,
    parameters: UntypedJsonDataValue,
    method_name: String,
    method_parameters: UntypedJsonDataValue,
    mode: String,
    idempotency_key: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentInvocationResult {
    result: Option<UntypedJsonDataValue>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "PascalCase")]
enum UntypedJsonDataValue {
    Tuple(UntypedJsonElementValues),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UntypedJsonElementValues {
    elements: Vec<UntypedJsonElementValue>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "PascalCase")]
enum UntypedJsonElementValue {
    ComponentModel(JsonComponentModelValue),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonComponentModelValue {
    value: serde_json::Value,
}

impl UntypedJsonDataValue {
    fn tuple(values: Vec<serde_json::Value>) -> Self {
        UntypedJsonDataValue::Tuple(UntypedJsonElementValues {
            elements: values
                .into_iter()
                .map(|v| {
                    UntypedJsonElementValue::ComponentModel(JsonComponentModelValue { value: v })
                })
                .collect(),
        })
    }

    fn into_first_value(self) -> Option<serde_json::Value> {
        match self {
            UntypedJsonDataValue::Tuple(t) => t.elements.into_iter().next().map(|e| match e {
                UntypedJsonElementValue::ComponentModel(v) => v.value,
            }),
        }
    }
}

/// Parses a Rib worker name (the instance() argument) which is an agent ID
/// in the format `"agent-type(param1, param2)"` into the agent type name
/// and constructor parameters.
fn parse_agent_id(worker_name: &str) -> anyhow::Result<(String, UntypedJsonDataValue)> {
    let worker_name = worker_name.trim();

    if let Some(paren_pos) = worker_name.find('(') {
        let agent_type = worker_name[..paren_pos].trim().to_string();
        let params_str = worker_name[paren_pos + 1..].trim_end_matches(')').trim();

        let params = if params_str.is_empty() {
            vec![]
        } else {
            params_str
                .split(',')
                .map(|s| {
                    let s = s.trim();
                    serde_json::from_str(s).unwrap_or(serde_json::Value::String(s.to_string()))
                })
                .collect()
        };

        Ok((agent_type, UntypedJsonDataValue::tuple(params)))
    } else {
        Ok((worker_name.to_string(), UntypedJsonDataValue::tuple(vec![])))
    }
}

/// Extracts the agent type name from the function name.
/// e.g. `golem:agent/HttpAgent.{string-path-var}` → `HttpAgent`
fn extract_agent_type(function_name: &str) -> Option<String> {
    let after_slash = function_name.rsplit_once('/')?.1;
    let interface = after_slash.split_once(".{")?.0;
    Some(interface.to_string())
}

/// Extracts the method name from a fully qualified function name and converts
/// from kebab-case (Rib/WIT convention) to snake_case (invoke-agent API convention).
/// e.g. `HttpAgent.{string-path-var}` → `string_path_var`
fn extract_method_name(function_name: &str) -> String {
    let raw = if let Some(start) = function_name.rfind(".{") {
        let method = &function_name[start + 2..];
        method.trim_end_matches('}')
    } else {
        function_name
    };
    raw.replace('-', "_")
}

#[async_trait]
impl WorkerFunctionInvoke for GolemWorkerFunctionInvoke {
    async fn invoke(
        &self,
        _component_id: Uuid,
        _component_name: &str,
        worker_name: &str,
        function_name: &str,
        args: Vec<ValueAndType>,
        return_type: Option<AnalysedType>,
    ) -> anyhow::Result<Option<ValueAndType>> {
        let (agent_type_name, constructor_params) = parse_agent_id(worker_name)?;
        let method_name = extract_method_name(function_name);

        if let Some(fn_agent_type) = extract_agent_type(function_name) {
            if fn_agent_type != agent_type_name {
                anyhow::bail!(
                    "Method '{}' belongs to agent type '{}', but this instance is '{}'",
                    method_name,
                    fn_agent_type,
                    agent_type_name
                );
            }
        }

        let method_params: Vec<serde_json::Value> = args
            .iter()
            .map(|vat| vat.to_json_value().map_err(|e| anyhow::anyhow!(e)))
            .collect::<anyhow::Result<Vec<_>>>()?;

        let request = AgentInvocationRequest {
            app_name: self.app_name.clone(),
            env_name: self.env_name.clone(),
            agent_type_name,
            parameters: constructor_params,
            method_name,
            method_parameters: UntypedJsonDataValue::tuple(method_params),
            mode: "await".to_string(),
            idempotency_key: Uuid::new_v4().to_string(),
        };

        let url = format!("{}/v1/agents/invoke-agent", self.base_url);

        let response = self
            .client
            .post(&url)
            .json(&request)
            .bearer_auth(self.effective_token())
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            if status == reqwest::StatusCode::UNAUTHORIZED {
                if self.token.is_none() {
                    anyhow::bail!(
                        "Authorization failed. The default local token may have changed in your Golem build. \
                         Pass an explicit token via --token or GOLEM_TOKEN."
                    );
                } else {
                    anyhow::bail!("Authorization failed. The provided token was rejected by the Golem server.");
                }
            }

            anyhow::bail!("Golem API error ({}): {}", status, body);
        }

        let result: AgentInvocationResult = response.json().await?;

        match (
            result.result.and_then(|r| r.into_first_value()),
            return_type,
        ) {
            (Some(json_val), Some(typ)) => {
                let vat = ValueAndType::parse_with_type(&json_val, &typ).map_err(|errs| {
                    anyhow::anyhow!("Failed to parse result: {}", errs.join(", "))
                })?;
                Ok(Some(vat))
            }
            _ => Ok(None),
        }
    }
}
