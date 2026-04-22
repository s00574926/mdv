# Mermaid Showcase

This sample document is meant to stress the default viewer with several Mermaid diagram families in one file.

## Flowchart

```mermaid
flowchart LR
    Draft[Draft markdown] --> Rust[Render with comrak]
    Rust --> Webview[Tauri webview]
    Webview --> Mermaid[Mermaid JS]
    Mermaid --> Preview[Live preview]
```

## Sequence Diagram

```mermaid
sequenceDiagram
    participant User
    participant Menu
    participant Renderer
    User->>Menu: Open Folder
    Menu->>Renderer: Load first .md file
    Renderer-->>User: Refresh preview
```

## Class Diagram

```mermaid
classDiagram
    class AppState {
      +current_path
      +recent_paths
    }
    class Workspace {
      +document
      +explorer
    }
    AppState --> Workspace : builds
```

## ER Diagram

```mermaid
erDiagram
    FOLDER ||--o{ DOCUMENT : contains
    DOCUMENT ||--o{ DIAGRAM : renders
    FOLDER {
      string path
    }
    DOCUMENT {
      string title
      string source
    }
    DIAGRAM {
      string kind
    }
```

## Journey

```mermaid
journey
    title Preview workflow
    section Startup
      Open app: 5: User
      Load sample: 4: Viewer
    section Editing
      Save markdown: 5: User
      Hot reload preview: 5: Viewer
```

## Gantt

```mermaid
gantt
    title Viewer milestones
    dateFormat  YYYY-MM-DD
    section Foundation
    Tauri shell      :done, shell, 2026-04-18, 1d
    Markdown render  :done, render, 2026-04-19, 1d
    section UX
    File menu        :done, menu, 2026-04-20, 1d
    Folder explorer  :active, tree, 2026-04-21, 1d
```

## Pie

```mermaid
pie title Surface area
    "Rust backend" : 40
    "Frontend shell" : 25
    "Mermaid diagrams" : 20
    "Testing" : 15
```

## Mindmap

```mermaid
mindmap
  root((mdv))
    Backend
      comrak
      notify
      menu wiring
    Frontend
      preview
      folder tree
      hot reload
    Content
      markdown
      mermaid
```

## Notes

- The viewer only accepts `.md` files.
- Opening a folder shows directories plus Markdown files in the sidebar.
- Recent files are persisted and capped at ten entries.
