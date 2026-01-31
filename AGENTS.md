# AGENTS.md

## Project Overview
Ralph Desktop is a Tauri 2.x + Svelte 5 desktop app for orchestrating CLI coding agents (Claude Code/Codex/OpenCode).

## Repo Layout
- `src/`: Svelte frontend (components, stores, services, routes)
- `src-tauri/`: Rust backend (commands, engine, adapters, storage)

## Dev Commands
- Frontend dev: `pnpm dev`
- Tauri dev: `pnpm tauri dev`
- Frontend checks: `pnpm check`
- Rust tests: `cargo test` (run inside `src-tauri`)

## Conventions
- Keep UI aligned to the VS Code Dark Modern style (use the existing `bg-vscode-*`, `text-vscode-*`, `border-vscode` utility classes).
- All user-facing strings should use `svelte-i18n` keys from `src/lib/i18n/locales`.
- Brainstorm flow is AI-driven only; avoid re‑introducing preset/questionnaire flows unless explicitly requested.

## Data & Storage
- App data lives under `~/.ralph-desktop/` (config, projects, logs).

## Versioning
- Keep versions in sync across `package.json` and `src-tauri/tauri.conf.json`.
- Use `scripts/bump-version.mjs` when updating release versions.

## Version Alignment Checklist
- `package.json`: name = ralph-desktop, version = 0.1.1
- `src-tauri/tauri.conf.json`: version = 0.1.1
- `src-tauri/Cargo.toml`: name = ralph-desktop, version = 0.1.1, authors = 刘小排
- `src-tauri/Cargo.lock`: sync via cargo build/check after Cargo.toml changes

## Version Review Notes
- `src-tauri/src/main.rs` must reference `ralph_desktop_lib` after renaming the crate.
- `src-tauri/Cargo.lock` should be regenerated after Cargo.toml changes (run cargo check/build).
- Release requirement docs mention v0.1.0 intentionally; update only when bumping release docs.

## E2E Testing
- Headless webview E2E runs via `pnpm test:e2e` (uses Playwright).
- E2E mode is enabled with `VITE_E2E=1` and uses mocked tauri services.
- CLI coverage runs sequentially: Codex then Claude (override with `--cli`).
- E2E temp paths use `/tmp/ralph-e2e-*` and artifacts go to `./artifacts/e2e/`.
- Set `E2E_HOST`/`E2E_PORT` if needed; default host is `localhost`.
- `./artifacts` is gitignored; keep temporary outputs out of version control.

