# mdv Code Overhaul Review

Date: 2026-04-22
Mode: FULL AUDIT

## Step 0

Stacks detected: Web/JS/CSS plus Rust/Tauri.

### Repo health

- `cargo test -q` passes: 4 Rust tests, 0 failures, about 0.5s.
- `npm run build` passes once run outside the sandbox; Vite builds in 4.46s.
- `npm audit --json` reports 0 vulnerabilities.
- There are no `TODO`/`FIXME`/`HACK` markers in tracked source.
- There is no frontend lint, typecheck, or test script in [package.json](/C:/work/mdv/package.json:1).
- CI is currently broken: [rust.yml](/C:/work/mdv/.github/workflows/rust.yml:19) runs `cargo build` and `cargo test` from repo root, but the manifest is in `src-tauri/`. Reproducing `cargo build` at `C:\work\mdv` fails immediately.
- Production build already warns about oversized Mermaid chunks; `mermaid.core` is 601.5 kB minified, 145.8 kB gzip.
- Current built payload is about 2.99 MB under `dist`; the Windows offline installer is about 201.6 MB, mostly because [tauri.conf.json](/C:/work/mdv/src-tauri/tauri.conf.json:28) uses `offlineInstaller`.

### Dependency landscape

- JS direct deps: `@tauri-apps/api 2.10.1`, `@tauri-apps/cli 2.10.1`, `mermaid 11.14.0` are current from live npm queries; `vite` is installed at `7.3.2` while npm reports `8.0.9`.
- Rust direct deps appear current from `cargo info`: `anyhow 1.0.102`, `comrak 0.52.0`, `serde 1.0.228`, `serde_json 1.0.149`, `tauri 2.10.3`, `tauri-build 2.5.6`, `tauri-plugin-dialog 2.7.0`. `notify 8.2.0` is current stable; only a `9.0.0-rc.3` prerelease exists.

### Platform / language floor

- Frontend is plain ESM JavaScript on Vite; no TS, no browserslist, no lint config.
- Rust is edition 2024 in [Cargo.toml](/C:/work/mdv/src-tauri/Cargo.toml:1), but there is no explicit `rust-version` floor declared.

### Tech debt concentration

- [workspace.rs](/C:/work/mdv/src-tauri/src/workspace.rs:1): biggest module and central state/orchestration path.
- [markdown.rs](/C:/work/mdv/src-tauri/src/markdown.rs:1): rendering, HTML trust boundary, and most existing tests.
- [main.js](/C:/work/mdv/src/main.js:1) plus [watcher.rs](/C:/work/mdv/src-tauri/src/watcher.rs:1): UI/event model and refresh behavior.
- Git history is too short to use churn meaningfully: only 4 commits.

### Early high-risk observations

- Do fix CI first. [rust.yml](/C:/work/mdv/.github/workflows/rust.yml:19) is a hard reliability failure.
- Do review the trust boundary next. [markdown.rs](/C:/work/mdv/src-tauri/src/markdown.rs:77) enables unsafe HTML, [main.js](/C:/work/mdv/src/main.js:59) injects it with `innerHTML`, Mermaid runs in `"loose"` mode at [main.js](/C:/work/mdv/src/main.js:19), and [tauri.conf.json](/C:/work/mdv/src-tauri/tauri.conf.json:24) sets `csp: null`.
- Do simplify the workspace model before adding features. [workspace.rs](/C:/work/mdv/src-tauri/src/workspace.rs:46) contains an unused startup path, and [watcher.rs](/C:/work/mdv/src-tauri/src/watcher.rs:8) only watches the selected file’s parent, so explorer contents can go stale.

### Impact / effort matrix

```text
                LOW EFFORT                 HIGH EFFORT
           ┌─────────────────┬────────────────────────┐
HIGH       │ Fix broken CI   │ Redefine trust boundary│
IMPACT     │ Add core tests  │ Rework explorer/watch  │
           ├─────────────────┼────────────────────────┤
LOW        │ Remove dead API │ JS->TS migration       │
IMPACT     │ Split modules   │ Broad UI redesign      │
           └─────────────────┴────────────────────────┘
```

### What already exists

- Rust side is already separated into sensible modules.
- Mermaid loading is lazy on the frontend.
- The markdown renderer has better fixture coverage than the rest of the app.
- Direct dependency count is small.

### Recommendation

Do `SYSTEMATIC`. Here’s why: the repo is small, but the issues cut across CI, security boundary, architecture, tests, and packaging; a section-by-section audit will stay concrete without turning into rewrite theater.

## Architecture

Current dependency shape:

```text
menu / commands
      │
      v
  workspace.rs
   ├─ state.rs      session + recents + watcher handle
   ├─ markdown.rs   file IO + markdown->HTML
   ├─ watcher.rs    fs events -> emit refresh
   └─ app_menu.rs   menu refresh + open/recent actions

frontend main.js
   └─ listens for `workspace://updated`
      and directly injects returned HTML
```

Primary source of truth today:

```text
AppState.session
  current_path
  current_directory
  watcher
  recent_paths
```

### 1

Do split the workspace orchestration boundary before adding features. Here’s why: [workspace.rs](/C:/work/mdv/src-tauri/src/workspace.rs:46) owns startup state, folder traversal, file opening, watcher wiring, payload assembly, and event emission, so every change to selection/rendering/watch behavior lands in one choke point.

- Failure mode: opening a large folder forces synchronous recursive tree rebuilds in [workspace.rs](/C:/work/mdv/src-tauri/src/workspace.rs:189) and [workspace.rs](/C:/work/mdv/src-tauri/src/workspace.rs:304), which can stall UI refresh and make racey watcher/menu bugs harder to isolate. Current code has no caching, no incremental refresh, and no boundary that isolates explorer state from document state.
- Recommendation maps to preference: explicit over clever and minimal diff; one small boundary split is cheaper than continuing to pile behavior into a god-module.
- A. Recommended: split `workspace.rs` into `workspace_session`, `explorer`, and `workspace_payload`; effort medium, risk low, blast radius moderate, maintenance burden lower.
- B. Keep one file but add internal structs/functions around selection/render/watch responsibilities; effort low, risk low, blast radius small, maintenance burden medium.
- C. Defer and keep shipping through `workspace.rs`; effort none now, risk rising, blast radius future-wide, maintenance burden highest.

### 2

Do pick one startup architecture and delete the dead branch. Here’s why: the backend exposes `load_initial_workspace` at [commands.rs](/C:/work/mdv/src-tauri/src/commands.rs:8) and implements sample bootstrap in [workspace.rs](/C:/work/mdv/src-tauri/src/workspace.rs:46), but the frontend never invokes it in [main.js](/C:/work/mdv/src/main.js:1), so the app has two conflicting startup models and one is dead code.

- Failure mode: future work updates only one path, so startup behavior, recent-file restore, and initial watcher setup drift silently. Current code already shows drift: README describes blank startup while backend still tries to preload `sample/example.md`.
- Recommendation maps to preference: DRY and engineered enough; one startup path is the only sane source of truth.
- A. Recommended: remove `load_initial_workspace` and sample boot logic, formalize blank-start architecture; effort low, risk low, blast radius small, maintenance burden lower.
- B. Wire the frontend to call `load_initial_workspace` on boot and make sample/recent restore real product behavior; effort low, risk low, blast radius small, maintenance burden medium.
- C. Defer and leave both paths present; effort none now, risk medium, blast radius startup-only, maintenance burden high.

### 3

Do separate “current document watching” from “workspace explorer watching.” Here’s why: [watcher.rs](/C:/work/mdv/src-tauri/src/watcher.rs:8) only watches the selected file’s parent directory and only refreshes if the changed path matches the current file, while explorer contents come from one-off scans in [workspace.rs](/C:/work/mdv/src-tauri/src/workspace.rs:304).

- Failure mode: user opens a folder, adds or renames another `.md` file externally, and the explorer goes stale until they reopen the folder or switch files. Current code does not handle that production case at all.
- Recommendation maps to preference: more edge cases, not fewer; a file explorer that ignores external folder changes is fragile.
- A. Recommended: add a directory watcher when a folder is open and invalidate/rebuild the explorer on relevant `.md` changes; effort medium, risk medium, blast radius moderate, maintenance burden lower.
- B. Drop live explorer freshness and document that only the current file auto-refreshes; effort low, risk low, blast radius UX-only, maintenance burden medium.
- C. Defer and keep mixed semantics; effort none now, risk medium, blast radius user-visible, maintenance burden high.

### 4

Do formalize the trust boundary instead of leaving every layer permissive. Here’s why: [markdown.rs](/C:/work/mdv/src-tauri/src/markdown.rs:77) enables unsafe HTML, [main.js](/C:/work/mdv/src/main.js:59) injects returned HTML with `innerHTML`, Mermaid runs with `securityLevel: "loose"` at [main.js](/C:/work/mdv/src/main.js:19), and [tauri.conf.json](/C:/work/mdv/src-tauri/tauri.conf.json:24) disables CSP entirely.

- Failure mode: opening a malicious Markdown file can execute arbitrary markup/script in the webview. The README notes the tool is for trusted local content, but the architecture has no enforcement layer, no “safe mode,” and no boundary comment where this decision lives.
- Recommendation maps to preference: explicit over clever; if the app is trusted-only, say so in code and isolate the unsafe path, otherwise add sanitization and a stricter CSP.
- A. Recommended: keep trusted-local mode but centralize it as an explicit `trusted_preview` boundary with comments/tests/config naming, and tighten Mermaid/CSP as far as compatibility allows; effort medium, risk medium, blast radius moderate, maintenance burden lower.
- B. Build a sanitized mode for untrusted Markdown and default to it; effort high, risk medium, blast radius broad, maintenance burden medium.
- C. Defer and keep implicit permissive behavior; effort none now, risk high, blast radius security-wide, maintenance burden high.

### Files that need inline diagrams

- [workspace.rs](/C:/work/mdv/src-tauri/src/workspace.rs:46) for open/select/reload/watch state flow.
- [watcher.rs](/C:/work/mdv/src-tauri/src/watcher.rs:8) for event flow and refresh conditions.
- [main.js](/C:/work/mdv/src/main.js:111) for frontend render/update lifecycle.

### Not in scope yet

- Exact refactor plan for module extraction.
- Bundle-size work beyond noting architecture impact.
- Filing beads; ask before creating any.

### Unresolved decisions that may bite later

- Is this app intentionally trusted-content-only forever, or just for now?
- Should folder mode promise live explorer updates, or only current-file refresh?
- Should startup be blank, sample-based, or recent-file restore?

Recommended architecture choices: `1A`, `2A`, `3A`, `4A`.

## Pending

Awaiting user decisions for Architecture items `1` through `4` before continuing to:

1. Code Quality
2. Tests
3. Performance
4. Dependencies and modernization
