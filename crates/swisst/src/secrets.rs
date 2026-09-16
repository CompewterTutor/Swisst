use anyhow::{bail, Context, Result};
use std::process::Command;

use crate::store::SecretsConfig;

/// Secret backends. Only key *references* live in the store; resolution
/// happens here at sync/query time. Add new managers as variants.
pub enum Backend {
    Env,
    Infisical { project: Option<String>, env: Option<String> },
}

impl Backend {
    pub fn from_config(cfg: &SecretsConfig) -> Self {
        match cfg.backend.as_str() {
            "infisical" => Backend::Infisical {
                project: cfg.project.clone().or_else(|| std::env::var("INFISICAL_PROJECT").ok()),
                env: cfg.env.clone().or_else(|| std::env::var("INFISICAL_ENV").ok()),
            },
            _ => Backend::Env,
        }
    }

    /// Resolve one secret name to its value.
    pub fn get(&self, name: &str) -> Result<String> {
        match self {
            Backend::Env => std::env::var(name)
                .with_context(|| format!("env var {name} is not set")),
            Backend::Infisical { .. } => self.infisical_get(name),
        }
    }

    /// Optional resolution: process env first, then backend, else None.
    /// Lets plain env exports override the manager.
    pub fn get_optional(&self, name: &str) -> Option<String> {
        if let Ok(v) = std::env::var(name) {
            if !v.is_empty() {
                return Some(v);
            }
        }
        self.get(name).ok()
    }

    fn infisical_get(&self, name: &str) -> Result<String> {
        let Backend::Infisical { project, env } = self else {
            bail!("not the infisical backend");
        };
        let mut cmd = Command::new("infisical");
        cmd.arg("export").arg("--format").arg("json");
        if let Some(p) = project {
            cmd.arg("--projectId").arg(p);
        }
        if let Some(e) = env {
            cmd.arg("--env").arg(e);
        }
        let out = cmd.output().context("run `infisical export` (is the CLI installed/authed?)")?;
        if !out.status.success() {
            bail!("infisical export failed: {}", String::from_utf8_lossy(&out.stderr));
        }
        let stdout = String::from_utf8_lossy(&out.stdout);
        // Accept {"KEY":"value"} or {"secrets":[{"secretKey","secretValue"}]}.
        let v: serde_json::Value = serde_json::from_str(&stdout).context("parse infisical export")?;
        if let Some(map) = v.as_object() {
            if let Some(val) = map.get(name).and_then(|x| x.as_str()) {
                return Ok(val.to_string());
            }
            if let Some(secrets) = map.get("secrets").and_then(|x| x.as_array()) {
                for s in secrets {
                    let k = s.get("secretKey").and_then(|x| x.as_str()).unwrap_or("");
                    if k == name {
                        if let Some(val) = s.get("secretValue").and_then(|x| x.as_str()) {
                            return Ok(val.to_string());
                        }
                    }
                }
            }
        }
        // Last resort: dotenv-shaped lines.
        for line in stdout.lines() {
            if let Some(rest) = line.strip_prefix(&format!("{name}=")) {
                return Ok(rest.trim_matches('"').to_string());
            }
        }
        bail!("secret {name} not found in infisical export");
    }
}
