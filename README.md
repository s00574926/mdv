# mdv

Markdown desktop viewer scaffolded around Tauri 2, `comrak`, `notify`, and Mermaid JS.

## Quick start

```powershell
npm.cmd install
npm.cmd run tauri dev
```

Use the native `File` menu to open a Markdown file or folder. The app starts with a blank window and renders content only after you open something. [sample/example.md](/C:/work/mdv/sample/example.md:1) remains available as a bundled demo document.

## What the app does

- `File -> Open...` opens a single `.md` file.
- `File -> Open Folder...` opens a workspace folder, shows a Markdown-only explorer, and lets you switch files from the sidebar.
- `File -> Open Recent` reopens previously viewed Markdown files.
- The current document refreshes automatically when the file changes on disk.
- When a folder is open, the explorer refreshes for Markdown file and directory changes inside that workspace.
- In the preview pane, use `Ctrl` + mouse wheel on Windows/Linux or `Cmd` + mouse wheel on macOS to zoom in and out without changing normal scroll behavior.

## Developer commands

```powershell
npm.cmd run test:frontend
npm.cmd run typecheck
npm.cmd test
npm.cmd run build
```

`npm.cmd test` runs both the frontend checks and the Rust test suite.

## Build self-contained Windows app

This repo is configured to build a self-contained Windows installer with Tauri's `offlineInstaller` WebView2 mode in [tauri.conf.json](/C:/work/mdv/src-tauri/tauri.conf.json:1).

Install dependencies:

```powershell
npm.cmd install
```

Build the unsigned NSIS installer:

```powershell
npm.cmd run tauri build -- -b nsis --ci --no-sign
```

Build outputs:

- [mdv.exe](/C:/work/mdv/src-tauri/target/release/mdv.exe)
- [mdv_0.1.0_x64-setup.exe](/C:/work/mdv/src-tauri/target/release/bundle/nsis/mdv_0.1.0_x64-setup.exe)

Notes:

- `--no-sign` skips Windows code signing.
- The NSIS installer embeds the offline WebView2 installer, so it does not depend on a network download at install time.

## Project docs

- [README.md](/C:/work/mdv/README.md:1): setup, usage, build, and packaging commands.
- [audit-review.md](/C:/work/mdv/audit-review.md:1): point-in-time engineering audit with a current-status addendum.

## Notes

- Mermaid blocks are rewritten to raw HTML containers on the Rust side, then rendered in the webview.
- Raw HTML is intentionally enabled in `comrak` for the explicit `trusted-local-markdown-preview` boundary so Mermaid containers survive the Markdown render step.
- The frontend only injects HTML tagged with that trusted preview model, and Mermaid runs in a stricter `antiscript` mode instead of `loose`.
- Production CSP is explicitly set in [tauri.conf.json](/C:/work/mdv/src-tauri/tauri.conf.json:1) instead of leaving it `null`, but development still uses `devCsp: null` to avoid breaking the Vite dev server.
- This is still a local preview tool for trusted Markdown you open yourself, not a sanitized renderer for untrusted content.
