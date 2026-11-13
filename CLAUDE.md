# Magpie — Claude / Agent Notes

Local-first, cross-platform clipboard manager built with Tauri 2. The Rust
backend lives in `src-tauri/` (hexagonal layout: `domain/`, `ports/`,
`adapters/`, `services/`, `shell/`), the React + TypeScript frontend in
`src/`. Setup, run, build and test commands are in `README.md`; non-obvious
choices are recorded in `DECISIONS.md`.

## Running locally

```bash
npm run tauri dev
```

Use a throwaway data folder so test runs don't touch real history (debug
builds only):

```bash
MAGPIE_DATA_DIR=/tmp/magpie-dev npm run tauri dev
```

Rust tests:

```bash
cd src-tauri && cargo test
```

## Inspecting the database

History is stored in SQLite under the app's local data folder
(`~/Library/Application Support/app.magpie.desktop/` on macOS,
`%LOCALAPPDATA%\app.magpie.desktop\` on Windows), or under
`MAGPIE_DATA_DIR` when set. Open it with `sqlite3 <path-to-db>`.

## Pushing code

**Every branch must bump the app version once** — one bump per branch, not
per commit or per push. Follow-up commits on a branch that has already
bumped keep the same number. The version lives in three places that must stay
in sync: `package.json`, `src-tauri/tauri.conf.json` and
`src-tauri/Cargo.toml`. Nothing bumps it automatically, and no CI check will
catch a forgotten bump. Bumps made on branches are provisional; the number is
settled at merge, so on a merge conflict keep the higher value.

**No Claude / AI attribution anywhere.** Commit messages and PR descriptions
must not contain `Co-Authored-By: Claude`, "Generated with Claude Code", or any
similar attribution line.

## Frontend components

When creating new frontend components, keep them easy to maintain and
understand, and keep each file under 250 lines.

## Merging branches

When merging a branch it should be squashed and merged into one commit.

## BMAD method

BMAD v6.12.4 (module `bmm`) is installed. Skills live in
`.claude/skills/bmad-*`; shared config and scripts are in `_bmad/`, outputs
go to `_bmad-output/` (specs and stories under `_bmad-output/specs/`).

- Not sure where to start: invoke `bmad-help`.
- Typical flow for a change: `bmad-spec` → `bmad-build` → `bmad-code-review`.
  Agents: `bmad-agent-dev` (Amelia), `bmad-agent-architect` (Winston),
  `bmad-agent-pm` (John).
- Repo-specific behaviour lives in `_bmad/custom/<skill>.toml` (committed) and
  `_bmad/custom/<skill>.user.toml` (personal, gitignored). Never edit a
  skill's `customize.toml` or `_bmad/config*.toml`; the installer regenerates
  them.
- Personal config (`_bmad/config.user.toml`, `_bmad/core/config.yaml`,
  `_bmad/bmm/config.yaml`) is gitignored. To regenerate it after cloning:
  `npx bmad-method@6.12.4 install --directory . --modules bmm --tools claude-code --yes`
  (needs Node 20.12+, Python 3.10+ and `uv`).
