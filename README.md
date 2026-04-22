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

Use the native `File` menu to open a Markdown file or folder. The app starts with a blank window and renders content only after you open something. [sample/example.md](/C:/work/mdv/sample/example.md:1) remains available as a bundled demo document.

## Notes

- Mermaid blocks are rewritten to raw HTML containers on the Rust side, then rendered in the webview.
- Raw HTML is enabled in `comrak` so Mermaid containers survive the Markdown render step.
- This is a local preview tool scaffold, not a sanitized renderer for untrusted Markdown.
