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
        Box::new(OhMyPi),
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

// --- omp --------------------------------------------------------------------
/// Oh My Pi YAML: `providers.<name>` in `~/.omp/agent/models.yml` (or
/// `$PI_CODING_AGENT_DIR/models.yml`) with `baseUrl` + `apiKey` (env var
/// NAME — OMP resolves env-var-name-or-literal, so the secret itself never
/// touches disk) + `api` + pinned `models`. Merge-style: foreign providers
/// and root keys are preserved.
fn omp_agent_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("PI_CODING_AGENT_DIR") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    dirs_home().join(".omp").join("agent")
}

fn omp_models_path() -> PathBuf {
    let dir = omp_agent_dir();
    let yml = dir.join("models.yml");
    if yml.is_file() {
        return yml;
    }
    let yaml = dir.join("models.yaml");
    if yaml.is_file() {
        return yaml;
    }
    yml
}

fn yaml_mapping(v: &mut serde_yaml::Value) -> &mut serde_yaml::Mapping {
    if !v.is_mapping() {
        *v = serde_yaml::Value::Mapping(serde_yaml::Mapping::new());
    }
    v.as_mapping_mut().expect("yaml mapping")
}

fn read_yaml(path: &PathBuf) -> serde_yaml::Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|r| serde_yaml::from_str(&r).ok())
        .unwrap_or(serde_yaml::Value::Mapping(serde_yaml::Mapping::new()))
}

fn write_yaml(path: &PathBuf, v: &serde_yaml::Value, dry_run: bool) -> Result<()> {
    if dry_run {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("mkdir {}", parent.display()))?;
    }
    let raw = serde_yaml::to_string(v)?;
    std::fs::write(path, raw).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub struct OhMyPi;

impl Harness for OhMyPi {
    fn id(&self) -> &'static str {
        "omp"
    }
    fn name(&self) -> &'static str {
        "Oh My Pi"
    }
    fn config_path(&self) -> PathBuf {
        omp_models_path()
    }
    fn sync(&self, cfg: &Config, backend: &Backend, dry_run: bool) -> Result<String> {
        let _ = backend;
        let path = self.config_path();
        let mut doc = read_yaml(&path);
        let touched = {
            let root = yaml_mapping(&mut doc);
            let providers = root
                .entry(serde_yaml::Value::String("providers".to_string()))
                .or_insert(serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));
            let pmap = yaml_mapping(providers);
            let mut touched = Vec::new();
            for p in &cfg.providers {
                let wire = if p.api == "anthropic" {
                    "anthropic-messages"
                } else {
                    "openai-completions"
                };
                let mut entry = serde_yaml::Mapping::new();
                entry.insert(
                    serde_yaml::Value::String("baseUrl".to_string()),
                    serde_yaml::Value::String(format!("{}/v1", p.base_url.trim_end_matches('/'))),
                );
                // Env var NAME, never the secret (OMP resolves name-or-literal).
                entry.insert(
                    serde_yaml::Value::String("apiKey".to_string()),
                    serde_yaml::Value::String(p.key_env.clone()),
                );
                entry.insert(
                    serde_yaml::Value::String("api".to_string()),
                    serde_yaml::Value::String(wire.to_string()),
                );
                if p.models.is_empty() {
                    // No pins: let OMP enumerate live for OpenAI-compatible
                    // endpoints. Anthropic has no list endpoint — models get
                    // pinned later via `swisst model add`.
                    if p.api != "anthropic" {
                        let mut discovery = serde_yaml::Mapping::new();
                        discovery.insert(
                            serde_yaml::Value::String("type".to_string()),
                            serde_yaml::Value::String("openai-models-list".to_string()),
                        );
                        entry.insert(
                            serde_yaml::Value::String("discovery".to_string()),
                            serde_yaml::Value::Mapping(discovery),
                        );
                    }
                } else {
                    let models: Vec<serde_yaml::Value> = p.models
                        .iter()
                        .map(|m| {
                            let mut mm = serde_yaml::Mapping::new();
                            mm.insert(
                                serde_yaml::Value::String("id".to_string()),
                                serde_yaml::Value::String(m.clone()),
                            );
                            serde_yaml::Value::Mapping(mm)
                        })
                        .collect();
                    entry.insert(
                        serde_yaml::Value::String("models".to_string()),
                        serde_yaml::Value::Sequence(models),
                    );
                }
                pmap.insert(
                    serde_yaml::Value::String(p.name.clone()),
                    serde_yaml::Value::Mapping(entry),
                );
                touched.push(p.name.clone());
            }
            touched
        };
        write_yaml(&path, &doc, dry_run)?;
        Ok(format!("{}: providers [{}]", path.display(), touched.join(", ")))
    }
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
