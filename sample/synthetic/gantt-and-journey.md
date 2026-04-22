# Gantt And Journey

> A synthetic scheduling fixture for timeline-heavy diagrams.

```mermaid
gantt
    title Release Cutover
    dateFormat  YYYY-MM-DD
    section Build
    Compile           :done,    build, 2026-04-20, 1d
    Smoke test        :active,  smoke, 2026-04-21, 2d
```

- [x] Build
- [ ] Ship

```mermaid
journey
    title Preview flow
    section Authoring
      Open file: 5: Writer
      Change diagram: 4: Writer
    section Viewer
      Hot reload: 5: Viewer
```

