# mdv

mdv is a local Markdown desktop viewer built with Tauri 2, TypeScript, `comrak`, `notify`, and Mermaid JS. It is tuned for opening trusted local Markdown, browsing Markdown workspaces, previewing diagrams, and moving Mermaid output into slides.

## Quick Start

```powershell
npm.cmd install
npm.cmd run tauri dev
```

Use the titlebar menu to create a draft, open a `.md` file, or open a folder. [sample/example.md](/C:/work/mdv/sample/example.md:1) is included as a simple demo document, and [sample/synthetic](/C:/work/mdv/sample/synthetic/flow-and-sequence.md:1) contains Mermaid coverage samples.

## Features

### Markdown Viewing

- Opens local `.md` and `.MD` files in separate tabs.
- Renders CommonMark-style Markdown through `comrak`.
- Supports raw HTML inside trusted local Markdown previews.
- Rewrites fenced Mermaid blocks to previewable Mermaid containers.
- Detects raw Mermaid documents without fences for supported Mermaid roots such as flowcharts, sequence diagrams, class diagrams, ER diagrams, state diagrams, Gantt, journey, Git graph, pie, mindmap, timeline, C4, xychart, architecture, block, packet, sankey, radar, treemap, Venn, Wardley, and related beta diagram types.
- Renders Mermaid with a theme that follows the app theme.
- Shows render errors inside the preview instead of failing silently.
- Refreshes the active file preview when the file changes on disk.
- Supports preview zoom from `50%` to `250%` with `Ctrl` + mouse wheel on Windows/Linux or `Cmd` + mouse wheel on macOS.

### Editing And Tabs

- `New` creates an untitled Markdown draft in its own tab.
- Multiple untitled drafts are labeled stably as `Untitled`, `Untitled 2`, and so on.
- Untitled drafts are editable in the built-in textarea.
- Untitled Mermaid drafts show a live preview beside/below the editor.
- `Save` and `Save As...` persist untitled drafts as `.md` files and add the extension when needed.
- Tabs can be selected, closed, and cycled with keyboard shortcuts.
- Closing a dirty untitled tab, quitting, or closing the window prompts to save, discard, or cancel.

### Workspace Browsing

- `Open Folder...` opens a Markdown workspace with a sidebar explorer.
- The explorer shows Markdown files and directories that contain Markdown descendants.
- The explorer skips hidden/generated/heavy directories such as `.git`, `node_modules`, `target`, `dist`, `build`, `out`, and `coverage`.
- Opening a folder scans asynchronously and opens the first Markdown file in sorted order when one exists.
- Selecting a file from the explorer replaces the active folder placeholder/tab with that document.
- The explorer refreshes when Markdown files or directories are created, removed, renamed, or otherwise changed inside the workspace.

### Local Navigation

- Relative local image references are resolved from the opened Markdown file's folder and rendered through Tauri's asset protocol.
- Relative links to other `.md` files open inside mdv.
- External links and non-Markdown relative links remain regular links.
- The `Open Recent` submenu persists and deduplicates up to 10 recent Markdown files.
- Recent-file paths are normalized across Windows path casing, slash style, dot components, and verbatim prefixes.
- Dragging a Markdown file or folder over the window shows a drop target, and dropping it opens the file or workspace.

### Document Navigation

- The document navigation panel lists headings from the rendered preview.
- Headings get stable generated IDs when needed so outline jumps work.
- `Find` searches rendered preview text, highlights matches, shows match counts, and supports next/previous navigation.
- The active outline item follows preview scrolling.

### Diagram Export

- Right-click a Mermaid SVG to copy it as a PNG.
- Right-click a Mermaid SVG to copy it as a PowerPoint slide.
- PowerPoint slide copy validates the SVG payload, scales it into a 16:9 slide with margins, and uses local PowerPoint automation. Microsoft PowerPoint must be installed for that path.

### Window And Appearance

- Custom titlebar with app menu, tab strip, document navigation toggle, theme toggle, and minimize/maximize/close controls.
- Light/dark theme follows the system initially and persists the user's choice in local storage.
- Window size, position, monitor placement, and maximized state are persisted in the app local data directory.
- The titlebar supports dragging and double-click maximize/restore.

### Security Boundary

- mdv is for Markdown files you trust and open yourself.
- Rendered HTML must carry the explicit `trusted-local-markdown-preview` trust model before the frontend injects it.
- Mermaid runs in `antiscript` security mode.
- Production CSP is set in [src-tauri/tauri.conf.json](/C:/work/mdv/src-tauri/tauri.conf.json:1); development keeps `devCsp: null` for Vite compatibility.
- The local asset protocol is enabled so local preview images can render.

## Keyboard Shortcuts

| Shortcut | Action |
| --- | --- |
| `Ctrl+N` / `Cmd+N` | New untitled draft |
| `Ctrl+O` / `Cmd+O` | Open Markdown file |
| `Ctrl+Shift+O` / `Cmd+Shift+O` | Open folder |
| `Ctrl+S` / `Cmd+S` | Save active untitled draft |
| `Ctrl+Shift+S` / `Cmd+Shift+S` | Save active untitled draft as |
| `Ctrl+F` / `Cmd+F` | Open document find/navigation |
| `Ctrl+W` / `Cmd+W` | Close active tab |
| `Ctrl+Tab` | Next tab |
| `Ctrl+Shift+Tab` | Previous tab |
| `Escape` | Close open menus, context menus, drop overlay, or focused document navigation |
| `Alt+F4` | Quit on Windows |

## Commands

| Command | Description |
| --- | --- |
| `npm.cmd run dev` | Start the Vite dev server on `127.0.0.1:1420` |
| `npm.cmd run tauri dev` | Start the Tauri desktop app in development |
| `npm.cmd run typecheck` | Run TypeScript without emitting files |
| `npm.cmd run test:frontend` | Run frontend unit-style checks |
| `cargo test -q --manifest-path src-tauri/Cargo.toml` | Run Rust tests |
| `npm.cmd test` | Run frontend checks and Rust tests |
| `npm.cmd run e2e` | Run Playwright end-to-end tests |
| `npm.cmd run build` | Build the frontend bundle |
| `npm.cmd run tauri build -- -b nsis --ci --no-sign` | Build an unsigned Windows NSIS installer |

## Build A Self-Contained Windows App

This repo is configured to build a self-contained Windows installer with Tauri's `offlineInstaller` WebView2 mode in [src-tauri/tauri.conf.json](/C:/work/mdv/src-tauri/tauri.conf.json:1).

```powershell
npm.cmd install
npm.cmd run tauri build -- -b nsis --ci --no-sign
```

Build outputs:

- [mdv.exe](/C:/work/mdv/src-tauri/target/release/mdv.exe)
- [mdv_0.1.0_x64-setup.exe](/C:/work/mdv/src-tauri/target/release/bundle/nsis/mdv_0.1.0_x64-setup.exe)

Notes:

- `--no-sign` skips Windows code signing.
- The NSIS installer embeds the offline WebView2 installer, so install does not depend on a network download.

## Architecture

- [src/main.ts](/C:/work/mdv/src/main.ts:1) owns browser-side UI wiring, shortcuts, drag/drop, Mermaid hydration, copy actions, tabs, navigation, theme, and window controls.
- [src/view.ts](/C:/work/mdv/src/view.ts:1) contains deterministic render helpers for tabs, explorer, editor, preview, heading IDs, local references, busy state, and zoom state.
- [src-tauri/src/workspace.rs](/C:/work/mdv/src-tauri/src/workspace.rs:1) coordinates document sessions, open/save commands, folder scans, recent files, file watching, and workspace payloads.
- [src-tauri/src/markdown.rs](/C:/work/mdv/src-tauri/src/markdown.rs:1) renders Markdown, rewrites Mermaid, and rewrites local images/Markdown links.
- [src-tauri/src/explorer.rs](/C:/work/mdv/src-tauri/src/explorer.rs:1) builds the Markdown-only workspace tree.
- [src-tauri/src/watcher.rs](/C:/work/mdv/src-tauri/src/watcher.rs:1) handles active-file and workspace-directory refresh events.
- [src-tauri/src/state.rs](/C:/work/mdv/src-tauri/src/state.rs:1) stores session caches, recent files, and persisted window state.
- [src-tauri/src/powerpoint_clipboard.rs](/C:/work/mdv/src-tauri/src/powerpoint_clipboard.rs:1) implements Mermaid-to-PowerPoint clipboard automation.

## Project Docs

- [README.md](/C:/work/mdv/README.md:1): setup, usage, build, features, and architecture overview.
- [audit-review.md](/C:/work/mdv/audit-review.md:1): point-in-time engineering audit with a current-status addendum.
