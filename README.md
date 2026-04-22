# mdv

Markdown desktop viewer scaffolded around Tauri 2, `comrak`, `notify`, and Mermaid JS.

## Quick start

```powershell
npm.cmd install
cd src-tauri
cargo build
cd ..
npm.cmd run tauri dev
```

The initial app loads [sample/example.md](/C:/work/mdv/sample/example.md:1) and watches the current file for changes after you open it.

## Notes

- Mermaid blocks are rewritten to raw HTML containers on the Rust side, then rendered in the webview.
- Raw HTML is enabled in `comrak` so Mermaid containers survive the Markdown render step.
- This is a local preview tool scaffold, not a sanitized renderer for untrusted Markdown.

