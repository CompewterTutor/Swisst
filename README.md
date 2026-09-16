# Swisst
A swiss army knife for providers, models, harnesses, agent definitions, skills, tools, plugins, integrations, secret managers, quants, workflows, loras, etc. The ultimate sync tool for your AI Psychosis and LLMADHD.

## What works today: provider manager CLI

One store (`~/.config/swisst/swisst.json`), every harness in sync.

```sh
# register a provider (key by reference — never stored)
swisst provider add --name mtplx --base-url http://192.168.1.200:8066
swisst provider list

# enumerate live models, pick which to keep (ratatui TUI)
swisst models mtplx
swisst models mtplx --pick
swisst model add mtplx <model-id>

# render into harness configs (opencode, claude, codex, gemini)
swisst sync --dry-run
swisst sync
swisst sync --harness opencode
```

Secrets: `env` backend by default (`MTPLX_KEY=... swisst sync`);
`infisical` backend shells `infisical export` (set
`"backend": "infisical"` plus optional `project`/`env` in the store).
New secret managers plug into `crates/swisst/src/secrets.rs`;
new harnesses implement the `Harness` trait in `harnesses.rs`
(file-backed templates via `swisst harness new` are planned).

Build: `cargo build` (set `CARGO_TARGET_DIR` to a roomy drive on
cramped Windows system disks).

Docs: `docs/swisst.1` (man page), `docs/harness-template-spec.md`
(template contract), `docs/zoid-shell.md` (planned GUI shell).

## GUI shell (planned)

A zoid `tray` (taskbar) + `menubar` shell around this CLI is planned —
see `docs/zoid-shell.md`. Needs the `zoid` CLI (`zoid create` /
`zoid companion`); until then this crate is the whole app.
