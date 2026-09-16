use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::secrets::Backend;
use crate::store::{config_home, Config, ProviderEntry};

/// A harness is one AI tool whose config Swisst keeps in sync.
/// New harnesses plug in here: implement the trait, add to `all()`.
/// Later `swisst harness new` will scaffold file-backed templates in
/// `~/.config/swisst/harnesses/` instead of code.
pub trait Harness {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn config_path(&self) -> PathBuf;
    fn sync(&self, cfg: &Config, backend: &Backend, dry_run: bool) -> Result<String>;
}

pub fn all() -> Vec<Box<dyn Harness>> {
    vec![
        Box::new(Opencode),
        Box::new(ClaudeCode),
        Box::new(CodexCli),
        Box::new(GeminiCli),
    ]
}

pub fn find(id: &str) -> Option<Box<dyn Harness>> {
    all().into_iter().find(|h| h.id() == id)
}

fn read_json(path: &PathBuf) -> serde_json::Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|r| serde_json::from_str(&r).ok())
        .unwrap_or(serde_json::Value::Object(Default::default()))
}

fn write_json(path: &PathBuf, v: &serde_json::Value, dry_run: bool) -> Result<()> {
    if dry_run {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("mkdir {}", parent.display()))?;
    }
    let raw = serde_json::to_string_pretty(v)? + "\n";
    std::fs::write(path, raw).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn resolve_key(p: &ProviderEntry, backend: &Backend) -> String {
    backend.get_optional(&p.key_env).unwrap_or_default()
}

// --- opencode ---------------------------------------------------------------
/// Merges `provider.<name>` blocks into opencode.json, preserving every
/// other provider, agent, plugin and mcp section.
pub struct Opencode;

impl Harness for Opencode {
    fn id(&self) -> &'static str {
        "opencode"
    }
    fn name(&self) -> &'static str {
        "OpenCode CLI"
    }
    fn config_path(&self) -> PathBuf {
        config_home().join("opencode").join("opencode.json")
    }
    fn sync(&self, cfg: &Config, backend: &Backend, dry_run: bool) -> Result<String> {
        let path = self.config_path();
        let mut doc = read_json(&path);
        let map = doc.as_object_mut().unwrap();
        let providers = map
            .entry("provider".to_string())
            .or_insert(serde_json::Value::Object(Default::default()));
        let pobj = providers.as_object_mut().unwrap();
        let mut touched = Vec::new();
        for p in &cfg.providers {
            let mut models = serde_json::Map::new();
            for m in &p.models {
                models.insert(m.clone(), serde_json::json!({}));
            }
            pobj.insert(
                p.name.clone(),
                serde_json::json!({
                    "name": p.name,
                    "api": "openaicompatible",
                    "options": {
                        "baseURL": format!("{}/v1", p.base_url.trim_end_matches('/')),
                        "apiKey": resolve_key(p, backend),
                    },
                    "models": models,
                }),
            );
            touched.push(p.name.clone());
        }
        write_json(&path, &doc, dry_run)?;
        Ok(format!("{}: providers [{}]", path.display(), touched.join(", ")))
    }
}

// --- env-style harnesses ----------------------------------------------------
/// Claude Code / Gemini CLI style: merge `<PREFIX>_BASE_URL` and
/// `<PREFIX>_API_KEY` into the `env` object of their settings.json.
/// Only providers with `env_prefix` set take part.
fn sync_env_json(id: &str, path: PathBuf, cfg: &Config, backend: &Backend, dry_run: bool) -> Result<String> {
    let mut doc = read_json(&path);
    {
        let map = doc.as_object_mut().unwrap();
        map.entry("env".to_string())
            .or_insert(serde_json::Value::Object(Default::default()));
    }
    let env = doc.pointer_mut("/env").expect("env object");
    let mut touched = Vec::new();
    for p in &cfg.providers {
        if let Some(prefix) = &p.env_prefix {
            let base = format!("{}/v1", p.base_url.trim_end_matches('/'));
            env[format!("{prefix}_BASE_URL")] = serde_json::Value::String(base);
            let key = resolve_key(p, backend);
            if !key.is_empty() {
                env[format!("{prefix}_API_KEY")] = serde_json::Value::String(key);
            }
            touched.push(p.name.clone());
        }
    }
    write_json(&path, &doc, dry_run)?;
    Ok(format!("{id} {}: env [{}]", path.display(), touched.join(", ")))
}

pub struct ClaudeCode;

impl Harness for ClaudeCode {
    fn id(&self) -> &'static str {
        "claude"
    }
    fn name(&self) -> &'static str {
        "Claude Code"
    }
    fn config_path(&self) -> PathBuf {
        dirs_home().join(".claude").join("settings.json")
    }
    fn sync(&self, cfg: &Config, backend: &Backend, dry_run: bool) -> Result<String> {
        sync_env_json("claude", self.config_path(), cfg, backend, dry_run)
    }
}

pub struct GeminiCli;

impl Harness for GeminiCli {
    fn id(&self) -> &'static str {
        "gemini"
    }
    fn name(&self) -> &'static str {
        "Gemini CLI"
    }
    fn config_path(&self) -> PathBuf {
        dirs_home().join(".gemini").join("settings.json")
    }
    fn sync(&self, cfg: &Config, backend: &Backend, dry_run: bool) -> Result<String> {
        sync_env_json("gemini", self.config_path(), cfg, backend, dry_run)
    }
}

fn dirs_home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"))
}

// --- codex ------------------------------------------------------------------
/// Codex CLI TOML: `[model_providers.<name>]` with base_url + env_key.
/// The key itself stays in the environment; only its name is written.
pub struct CodexCli;

impl Harness for CodexCli {
    fn id(&self) -> &'static str {
        "codex"
    }
    fn name(&self) -> &'static str {
        "OpenAI Codex CLI"
    }
    fn config_path(&self) -> PathBuf {
        dirs_home().join(".codex").join("config.toml")
    }
    fn sync(&self, cfg: &Config, backend: &Backend, dry_run: bool) -> Result<String> {
        let _ = backend;
        let path = self.config_path();
        let mut doc: toml::Value = std::fs::read_to_string(&path)
            .ok()
            .and_then(|r| r.parse().ok())
            .unwrap_or(toml::Value::Table(Default::default()));
        let root = doc.as_table_mut().unwrap();
        let prov = root
            .entry("model_providers".to_string())
            .or_insert(toml::Value::Table(Default::default()));
        let table = prov.as_table_mut().unwrap();
        let mut touched = Vec::new();
        for p in &cfg.providers {
            let mut t = toml::map::Map::new();
            t.insert("name".to_string(), toml::Value::String(p.name.clone()));
            t.insert(
                "base_url".to_string(),
                toml::Value::String(format!("{}/v1", p.base_url.trim_end_matches('/'))),
            );
            t.insert("env_key".to_string(), toml::Value::String(p.key_env.clone()));
            table.insert(p.name.clone(), toml::Value::Table(t));
            touched.push(p.name.clone());
        }
        if !dry_run {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("mkdir {}", parent.display()))?;
            }
            let raw = toml::to_string_pretty(&doc)?;
            std::fs::write(&path, raw).with_context(|| format!("write {}", path.display()))?;
        }
        Ok(format!("codex {}: model_providers [{}]", path.display(), touched.join(", ")))
    }
}
