use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::secrets::Backend;
use crate::store::ProviderEntry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
}

/// Live-enumerate models from an OpenAI-compatible `/v1/models` endpoint.
/// Anthropic-style providers have no list endpoint, so enumeration returns
/// an empty vec and models must be pinned by hand.
pub fn fetch_models(entry: &ProviderEntry, backend: &Backend) -> Result<Vec<ModelInfo>> {
    if entry.api != "openai-compatible" {
        return Ok(Vec::new());
    }
    let key = backend.get_optional(&entry.key_env).unwrap_or_default();
    let url = format!("{}/v1/models", entry.base_url.trim_end_matches('/'));
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    let mut req = client.get(&url);
    if !key.is_empty() {
        req = req.bearer_auth(&key);
    }
    let resp = req.send().with_context(|| format!("GET {url}"))?;
    let status = resp.status();
    if !status.is_success() {
        anyhow::bail!("GET {url} -> {status}");
    }
    let v: serde_json::Value = resp.json().context("parse /v1/models")?;
    let mut out = Vec::new();
    if let Some(data) = v.get("data").and_then(|d| d.as_array()) {
        for m in data {
            if let Some(id) = m.get("id").and_then(|i| i.as_str()) {
                out.push(ModelInfo { id: id.to_string() });
            }
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}
