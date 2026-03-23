use reqwest::Client;
use serde::{Deserialize, Serialize};

/// HTTP client for the Node sidecar service.
/// Handles markdown↔JSON conversion and document diffing.
pub struct SidecarClient {
    client: Client,
    base_url: String,
}

#[derive(Serialize)]
struct SerializeRequest {
    doc: serde_json::Value,
    schema_version: u32,
}

#[derive(Deserialize)]
struct SerializeResponse {
    markdown: String,
}

#[derive(Serialize)]
struct ParseRequest {
    markdown: String,
    schema_version: u32,
}

#[derive(Deserialize)]
struct ParseResponse {
    doc: serde_json::Value,
}

#[derive(Serialize)]
struct DiffRequest {
    old_doc: serde_json::Value,
    new_doc: serde_json::Value,
}

#[derive(Deserialize)]
struct DiffResponse {
    steps: Vec<serde_json::Value>,
}

impl SidecarClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
        }
    }

    /// Serialize ProseMirror JSON to canonical markdown.
    pub async fn serialize(
        &self,
        doc: serde_json::Value,
        schema_version: u32,
    ) -> Result<String, reqwest::Error> {
        let resp: SerializeResponse = self
            .client
            .post(format!("{}/serialize", self.base_url))
            .json(&SerializeRequest {
                doc,
                schema_version,
            })
            .send()
            .await?
            .json()
            .await?;
        Ok(resp.markdown)
    }

    /// Parse markdown to ProseMirror JSON.
    pub async fn parse(
        &self,
        markdown: String,
        schema_version: u32,
    ) -> Result<serde_json::Value, reqwest::Error> {
        let resp: ParseResponse = self
            .client
            .post(format!("{}/parse", self.base_url))
            .json(&ParseRequest {
                markdown,
                schema_version,
            })
            .send()
            .await?
            .json()
            .await?;
        Ok(resp.doc)
    }

    /// Compute ProseMirror Steps between two document states.
    pub async fn diff(
        &self,
        old_doc: serde_json::Value,
        new_doc: serde_json::Value,
    ) -> Result<Vec<serde_json::Value>, reqwest::Error> {
        let resp: DiffResponse = self
            .client
            .post(format!("{}/diff", self.base_url))
            .json(&DiffRequest { old_doc, new_doc })
            .send()
            .await?
            .json()
            .await?;
        Ok(resp.steps)
    }

    /// Health check — also verifies schema version compatibility.
    pub async fn health(&self) -> Result<u32, reqwest::Error> {
        #[derive(Deserialize)]
        struct HealthResponse {
            schema_version: u32,
        }
        let resp: HealthResponse = self
            .client
            .get(format!("{}/health", self.base_url))
            .send()
            .await?
            .json()
            .await?;
        Ok(resp.schema_version)
    }
}
