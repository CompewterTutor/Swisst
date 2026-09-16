use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "swisst", about = "Swiss army knife for providers, models and harnesses")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Manage providers
    Provider {
        #[command(subcommand)]
        command: ProviderCommands,
    },
    /// List models served by a provider (live /v1/models query)
    Models {
        /// Provider name from the store
        name: String,
        /// Interactive ratatui picker; selection is saved to the store
        #[arg(long)]
        pick: bool,
        /// Output raw JSON instead of a table
        #[arg(long)]
        json: bool,
    },
    /// Manage pinned models for a provider
    Model {
        #[command(subcommand)]
        command: ModelCommands,
    },
    /// Render the store into every (or one) harness config
    Sync {
        /// Only sync this harness id (see `harness list`)
        #[arg(long)]
        harness: Option<String>,
        /// Print what would change without writing
        #[arg(long)]
        dry_run: bool,
    },
    /// List known harness templates
    Harness {
        #[command(subcommand)]
        command: HarnessCommands,
    },
    /// Resolve one secret through the configured backend (debug)
    Secret {
        /// Environment variable / secret name to resolve
        var: String,
    },
    /// Print the store file path
    ConfigPath,
}

#[derive(Subcommand)]
pub enum ProviderCommands {
    /// Add (or update) a provider
    Add {
        #[arg(long)]
        name: String,
        #[arg(long)]
        base_url: String,
        /// openai-compatible (default) or anthropic
        #[arg(long, default_value = "openai-compatible")]
        api: String,
        /// Env var / secret name holding the API key (default: <NAME>_API_KEY)
        #[arg(long)]
        key_env: Option<String>,
        /// Prefix for VAR_BASE_URL / VAR_API_KEY env exports (env-style harnesses)
        #[arg(long)]
        env_prefix: Option<String>,
    },
    /// List providers in the store
    List,
    /// Remove a provider from the store
    Remove {
        name: String,
    },
}

#[derive(Subcommand)]
pub enum ModelCommands {
    /// Pin a model id to a provider (used by sync)
    Add {
        provider: String,
        model_id: String,
    },
    /// Unpin a model id
    Remove {
        provider: String,
        model_id: String,
    },
}

#[derive(Subcommand)]
pub enum HarnessCommands {
    /// List harness templates, target paths and status
    List,
}
