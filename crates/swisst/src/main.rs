mod cli;
mod harnesses;
mod providers;
mod secrets;
mod store;
mod tui;

use anyhow::{bail, Context, Result};
use clap::Parser;

use cli::{Cli, Commands, HarnessCommands, ModelCommands, ProviderCommands};
use secrets::Backend;
use store::Config;

fn main() -> Result<()> {
    let args = Cli::parse();
    match args.command {
        Commands::Provider { command } => match command {
            ProviderCommands::Add { name, base_url, api, key_env, env_prefix } => {
                if api != "openai-compatible" && api != "anthropic" {
                    bail!("--api must be openai-compatible or anthropic");
                }
                let mut cfg = Config::load()?;
                let key = key_env.unwrap_or_else(|| format!("{}_API_KEY", name.to_uppercase().replace('-', "_")));
                match cfg.find_mut(&name) {
                    Some(p) => {
                        p.base_url = base_url;
                        p.api = api;
                        p.key_env = key;
                        p.env_prefix = env_prefix;
                        println!("updated provider {name}");
                    }
                    None => {
                        cfg.providers.push(store::ProviderEntry {
                            name: name.clone(),
                            base_url,
                            api,
                            key_env: key,
                            env_prefix,
                            models: Vec::new(),
                        });
                        println!("added provider {name}");
                    }
                }
                cfg.save()?;
            }
            ProviderCommands::List => {
                let cfg = Config::load()?;
                if cfg.providers.is_empty() {
                    println!("no providers (swisst provider add ...)");
                    return Ok(());
                }
                for p in &cfg.providers {
                    println!(
                        "{name} [{api}] {url} key={key} models={n}",
                        name = p.name,
                        api = p.api,
                        url = p.base_url,
                        key = p.key_env,
                        n = p.models.len()
                    );
                }
            }
            ProviderCommands::Remove { name } => {
                let mut cfg = Config::load()?;
                let before = cfg.providers.len();
                cfg.providers.retain(|p| p.name != name);
                if cfg.providers.len() == before {
                    bail!("no provider named {name}");
                }
                cfg.save()?;
                println!("removed provider {name}");
            }
        },
        Commands::Models { name, pick, json } => {
            let mut cfg = Config::load()?;
            let backend = Backend::from_config(&cfg.secrets);
            let entry = cfg
                .find(&name)
                .with_context(|| format!("no provider named {name}"))?
                .clone();
            let models = providers::fetch_models(&entry, &backend)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&models)?);
                return Ok(());
            }
            if models.is_empty() {
                println!("{name}: no listable models (non-openai api? pin with `model add`)");
                return Ok(());
            }
            if pick {
                let ids: Vec<String> = models.iter().map(|m| m.id.clone()).collect();
                let current = entry.models.clone();
                let sel = tui::pick(&ids, &current)?;
                if let Some(p) = cfg.find_mut(&name) {
                    p.models = sel.clone();
                }
                cfg.save()?;
                println!("{name}: pinned {} model(s)", sel.len());
            } else {
                for m in &models {
                    let pinned = if entry.models.contains(&m.id) { " *" } else { "" };
                    println!("{}{pinned}", m.id);
                }
            }
        }
        Commands::Model { command } => match command {
            ModelCommands::Add { provider, model_id } => {
                let mut cfg = Config::load()?;
                let p = cfg
                    .find_mut(&provider)
                    .with_context(|| format!("no provider named {provider}"))?;
                if !p.models.contains(&model_id) {
                    p.models.push(model_id.clone());
                }
                cfg.save()?;
                println!("pinned {model_id} on {provider}");
            }
            ModelCommands::Remove { provider, model_id } => {
                let mut cfg = Config::load()?;
                let p = cfg
                    .find_mut(&provider)
                    .with_context(|| format!("no provider named {provider}"))?;
                p.models.retain(|m| m != &model_id);
                cfg.save()?;
                println!("unpinned {model_id} from {provider}");
            }
        },
        Commands::Sync { harness, dry_run } => {
            let cfg = Config::load()?;
            let backend = Backend::from_config(&cfg.secrets);
            let targets: Vec<Box<dyn harnesses::Harness>> = match harness {
                Some(id) => vec![harnesses::find(&id)
                    .with_context(|| format!("unknown harness {id} (see `harness list`)"))?],
                None => harnesses::all(),
            };
            for h in &targets {
                let line = h.sync(&cfg, &backend, dry_run)?;
                println!("{}{}", if dry_run { "[dry-run] " } else { "" }, line);
            }
        }
        Commands::Harness { command } => match command {
            HarnessCommands::List => {
                for h in harnesses::all() {
                    let path = h.config_path();
                    let status = if path.is_file() { "present" } else { "missing" };
                    println!("{id:10} {name:16} {path} [{status}]", id = h.id(), name = h.name(), path = path.display());
                }
            }
        },
        Commands::Secret { var } => {
            let cfg = Config::load()?;
            let backend = Backend::from_config(&cfg.secrets);
            let val = backend.get(&var)?;
            println!("{var} resolves ({} chars)", val.len());
        }
        Commands::ConfigPath => {
            println!("{}", store::store_path().display());
        }
    }
    Ok(())
}
