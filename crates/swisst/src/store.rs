use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Where user-level config lives. Mirrors the `~/.config/<tool>` layout
/// used across Zoidot-managed machines (XDG first, `~/.config` fallback).
pub fn config_home() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg);
        }
    }
    if let Some(home) = dirs::home_dir() {
        let dot = home.join(".config");
        if dot.is_dir() {
            return dot;
        }
        if let Some(dir) = dirs::config_dir() {
            return dir;
        }
        return dot;
    }
    PathBuf::from(".config")
}

pub fn store_path() -> PathBuf {
    config_home().join("swisst").join("swisst.json")
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub secrets: SecretsConfig,
    #[serde(default)]
    pub providers: Vec<ProviderEntry>,
}

fn default_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SecretsConfig {
    /// "env" (default) or "infisical". New backends plug in here later.
    #[serde(default = "default_backend")]
    pub backend: String,
    /// Optional Infisical scoping (also readable from INFISICAL_PROJECT /
    /// INFISICAL_ENV at resolve time).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<String>,
}

fn default_backend() -> String {
    "env".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderEntry {
    pub name: String,
    pub base_url: String,
    /// "openai-compatible" (default) or "anthropic"
    #[serde(default = "default_api")]
    pub api: String,
    /// Env var / secret name holding the API key. Never the key itself.
    pub key_env: String,
    /// Optional prefix for VAR_BASE_URL / VAR_API_KEY env exports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env_prefix: Option<String>,
    /// Model ids pinned for sync. Managed via `models <p> --pick`.
    #[serde(default)]
    pub models: Vec<String>,
}

fn default_api() -> String {
    "openai-compatible".to_string()
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = store_path();
        if !path.is_file() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))
    }

    pub fn save(&self) -> Result<()> {
        let path = store_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("mkdir {}", parent.display()))?;
        }
        let raw = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, raw).with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    pub fn find(&self, name: &str) -> Option<&ProviderEntry> {
        self.providers.iter().find(|p| p.name == name)
    }

    pub fn find_mut(&mut self, name: &str) -> Option<&mut ProviderEntry> {
        self.providers.iter_mut().find(|p| p.name == name)
    }

    /// Extra template search dirs, earliest wins. Reserved for
    /// `swisst harness new` scaffolding later.
    #[allow(dead_code)]
    pub fn template_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = vec![config_home().join("swisst").join("harnesses")];
        if let Ok(extra) = std::env::var("SWISST_HARNESS_DIR") {
            for part in std::env::split_paths(&extra) {
                dirs.push(part);
            }
        }
        dirs
    }
}
