# Harness template spec

How a harness plugs into Swisst — today as Rust code, tomorrow as a
TOML file `swisst harness new` scaffolds. File-backed templates are
not implemented yet; this spec is the contract they will follow.

## Today: the `Harness` trait

`crates/swisst/src/harnesses.rs`:

```rust
pub trait Harness {
    fn id(&self) -> &'static str;      // e.g. "opencode"
    fn name(&self) -> &'static str;    // e.g. "OpenCode CLI"
    fn config_path(&self) -> PathBuf;  // target file
    fn sync(&self, cfg: &Config, backend: &Backend, dry_run: bool) -> Result<String>;
}
```

To add one: implement the trait, push into `all()`. Sync must be
merge-style (preserve foreign keys), honor `dry_run` (no writes),
and return a one-line summary. Register the id in this doc's table.

## Tomorrow: file template (`~/.config/swisst/harnesses/<id>.toml`)

```toml
version = 1
id = "myharness"          # must match filename; [a-z0-9-]
name = "My Harness"
kind = "opencode-providers"  # see kinds below
path = "~/.config/myharness/config.json"  # ~ and $XDG_CONFIG_HOME expand
```

### Kinds

| kind | target | behavior |
|------|--------|----------|
| `opencode-providers` | JSON | set `provider.<name> = {name, api, options:{baseURL, apiKey}, models:{id:{}}}`, keep rest |
| `env-json` | JSON | merge `<PREFIX>_BASE_URL` (+ `_API_KEY` when resolvable) into top-level `env` object; only providers with `env_prefix` |
| `codex-providers` | TOML | set `[model_providers.<name>] = {name, base_url, env_key}` (key by name only, never the value) |

### Common rules

- Missing target file starts from `{}` / empty table; parents created.
- Unknown `kind` is an error naming the file.
- Key resolution uses the store's secret backend; unresolvable keys
  write empty string (JSON kinds) or are skipped (env kinds skip only
  the `_API_KEY`, still write `_BASE_URL`).
- `SWISST_HARNESS_DIR` prepends extra search dirs; first `<id>.toml`
  wins over built-ins.

## Built-in registry

| id | harness | kind today |
|----|---------|------------|
| `opencode` | OpenCode CLI (`~/.config/opencode/opencode.json`) | opencode-providers |
| `claude` | Claude Code (`~/.claude/settings.json`) | env-json |
| `codex` | OpenAI Codex CLI (`~/.codex/config.toml`) | codex-providers |
| `gemini` | Gemini CLI (`~/.gemini/settings.json`) | env-json |
