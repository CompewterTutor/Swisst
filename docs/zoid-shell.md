# Zoid GUI shell (planned)

Swisst core is the Rust CLI in `crates/swisst/` (clap + ratatui).
The desktop shell — taskbar tray + macOS menubar — will be scaffolded
with the zoid CLI once available:

```sh
# from a scratch dir (zoid create owns the tree, so scaffold outside
# the submodule, then merge the GUI crates back in)
zoid create swisst-shell --template tray --workspace
zoid companion --project swisst-shell   # wires CLI actions into crates/cli-companion
```

Findings from the zoid repo (`git_repos/clawffice/zoid`):

- `zoid create` takes exactly ONE `--template`; `tray` and `menubar`
  cannot combine in one manifest. Scaffold `tray` (covers Windows
  taskbar + macOS/Linux) and hand-merge `templates/menubar/src/*.tmpl`
  modules for the macOS menubar.
- `cli_companion` is not a create template — it attaches to an existing
  project via `zoid companion`. Our `crates/swisst` CLI already plays
  that role; point the companion generator at the tray project's
  `zoid-project.json` menu actions, or keep `swisst` standalone and
  have the tray shell shell out to it.
- zoid has no ratatui wrapper crate; add `ratatui`/`crossterm`
  (already in our workspace deps) to any generated companion crate.
- Extra features (settings, menus, theme) live in
  `templates/features/` and compose via `--include-features`.
