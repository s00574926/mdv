# Class And ER

The renderer should keep list and table HTML around Mermaid blocks.

```mermaid
classDiagram
    class Renderer {
      +render(markdown)
      +watch(path)
    }
    class Webview {
      +hydrate(html)
    }
    Renderer --> Webview : emits html
```

| Piece | Responsibility |
| --- | --- |
| Renderer | Transform Markdown |
| Webview | Run Mermaid |

- One
- Two

```mermaid
erDiagram
    DOCUMENT ||--o{ DIAGRAM : contains
    DOCUMENT {
        string path
        string title
    }
    DIAGRAM {
        string kind
        int order
    }
```

