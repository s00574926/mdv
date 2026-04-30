use anyhow::{Context, Result};
use comrak::markdown_to_html;
use serde::Serialize;
use std::{fs, path::Path};

use crate::trusted_preview;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderedDocument {
    pub title: String,
    pub html: String,
    pub source_name: String,
    pub source_path: String,
    pub watching: bool,
    pub trust_model: &'static str,
}

pub fn render_file(path: &Path, watching: bool) -> Result<RenderedDocument> {
    let markdown = fs::read_to_string(path)
        .with_context(|| format!("Failed to read Markdown file at {}", path.display()))?;

    Ok(render_markdown(
        &display_name(path),
        &file_name(path),
        &path.display().to_string(),
        &markdown,
        watching,
    ))
}

pub fn render_error(path: &Path, error: &anyhow::Error, watching: bool) -> RenderedDocument {
    let body = format!(
        "<section class=\"preview-error\">\
            <h1>Preview error</h1>\
            <p>{}</p>\
            <pre><code>{}</code></pre>\
        </section>",
        escape_html(&format!("Could not render {}", path.display())),
        escape_html(&format!("{error:#}"))
    );

    RenderedDocument {
        title: display_name(path),
        html: body,
        source_name: file_name(path),
        source_path: path.display().to_string(),
        watching,
        trust_model: trusted_preview::TRUST_MODEL,
    }
}

pub fn new_document() -> RenderedDocument {
    RenderedDocument {
        title: String::new(),
        html: String::new(),
        source_name: String::new(),
        source_path: String::new(),
        watching: false,
        trust_model: trusted_preview::TRUST_MODEL,
    }
}

pub fn untitled_document(title: &str, markdown: &str) -> RenderedDocument {
    render_markdown(title, "", "", markdown, false)
}

pub fn folder_placeholder_document(path: &Path) -> RenderedDocument {
    let _ = path;
    new_document()
}

fn render_markdown(
    title: &str,
    source_name: &str,
    source_path: &str,
    markdown: &str,
    watching: bool,
) -> RenderedDocument {
    let transformed = rewrite_mermaid_content(markdown);
    let html = markdown_to_html(&transformed, &trusted_preview::markdown_options());

    RenderedDocument {
        title: title.to_owned(),
        html,
        source_name: source_name.to_owned(),
        source_path: source_path.to_owned(),
        watching,
        trust_model: trusted_preview::TRUST_MODEL,
    }
}

fn rewrite_mermaid_content(markdown: &str) -> String {
    let rewritten = rewrite_mermaid_blocks(markdown);
    if rewritten.contains("<pre class=\"mermaid\">") {
        return rewritten;
    }

    if let Some(raw_mermaid) = render_raw_mermaid_document(markdown) {
        return raw_mermaid;
    }

    rewritten
}

fn rewrite_mermaid_blocks(markdown: &str) -> String {
    let mut output = String::new();
    let mut mermaid_buffer = Vec::new();
    let mut fence_marker = None;
    let mut fence_length = 0usize;
    let mut opening_fence = String::new();

    for line in markdown.lines() {
        if let Some(marker) = fence_marker {
            if is_fence_close(line, marker, fence_length) {
                output.push_str("<pre class=\"mermaid\">");
                output.push_str(&escape_html(&mermaid_buffer.join("\n")));
                output.push_str("</pre>\n");
                mermaid_buffer.clear();
                fence_marker = None;
                fence_length = 0;
                opening_fence.clear();
            } else {
                mermaid_buffer.push(line.to_owned());
            }

            continue;
        }

        if let Some((marker, length)) = mermaid_fence_start(line) {
            fence_marker = Some(marker);
            fence_length = length;
            opening_fence.clear();
            opening_fence.push_str(line);
            continue;
        }

        output.push_str(line);
        output.push('\n');
    }

    if fence_marker.is_some() {
        output.push_str(&opening_fence);
        output.push('\n');

        for line in mermaid_buffer {
            output.push_str(&line);
            output.push('\n');
        }
    }

    output
}

fn render_raw_mermaid_document(markdown: &str) -> Option<String> {
    if !looks_like_raw_mermaid_document(markdown) {
        return None;
    }

    let trimmed = markdown.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(format!(
        "<pre class=\"mermaid\">{}</pre>\n",
        escape_html(trimmed)
    ))
}

fn looks_like_raw_mermaid_document(markdown: &str) -> bool {
    if markdown.trim().is_empty() || markdown.contains("```") || markdown.contains("~~~") {
        return false;
    }

    markdown
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("%%"))
        .is_some_and(is_mermaid_root_line)
}

fn is_mermaid_root_line(line: &str) -> bool {
    if is_mermaid_flow_root_line(line, "graph") {
        return true;
    }

    if is_mermaid_flow_root_line(line, "flowchart") {
        return true;
    }

    const MERMAID_ROOTS: &[&str] = &[
        "sequenceDiagram",
        "classDiagram",
        "classDiagram-v2",
        "stateDiagram",
        "stateDiagram-v2",
        "erDiagram",
        "journey",
        "gantt",
        "pie",
        "gitGraph",
        "mindmap",
        "timeline",
        "zenuml",
        "quadrantChart",
        "requirementDiagram",
        "sankey-beta",
        "architecture-beta",
        "block-beta",
        "kanban",
        "packet-beta",
        "xychart",
        "xychart-beta",
        "radar-beta",
        "treemap-beta",
        "C4Context",
        "C4Container",
        "C4Component",
        "C4Dynamic",
        "C4Deployment",
    ];

    MERMAID_ROOTS.iter().any(|root| {
        line == *root
            || line
                .strip_prefix(root)
                .and_then(|suffix| suffix.chars().next())
                .is_some_and(char::is_whitespace)
    })
}

fn is_mermaid_flow_root_line(line: &str, root: &str) -> bool {
    let Some(suffix) = line.strip_prefix(root) else {
        return false;
    };

    if suffix.is_empty() {
        return true;
    }

    if !suffix.chars().next().is_some_and(char::is_whitespace) {
        return false;
    }

    suffix
        .split_whitespace()
        .next()
        .is_some_and(is_mermaid_flow_direction)
}

fn is_mermaid_flow_direction(token: &str) -> bool {
    matches!(
        token.trim_end_matches(';'),
        "TB" | "TD" | "BT" | "RL" | "LR"
    )
}

fn mermaid_fence_start(line: &str) -> Option<(char, usize)> {
    let (indent, content) = split_indentation(line);
    if indent > 3 {
        return None;
    }

    let marker = content.chars().next()?;

    if marker != '`' && marker != '~' {
        return None;
    }

    let fence_length = content.chars().take_while(|ch| *ch == marker).count();
    if fence_length < 3 {
        return None;
    }

    let info = content[fence_length..].trim();
    let language = info.split_whitespace().next()?;

    if language.eq_ignore_ascii_case("mermaid") {
        Some((marker, fence_length))
    } else {
        None
    }
}

fn is_fence_close(line: &str, marker: char, fence_length: usize) -> bool {
    let (indent, content) = split_indentation(line);
    if indent > 3 {
        return false;
    }

    let run_length = content.chars().take_while(|ch| *ch == marker).count();

    run_length >= fence_length && content[run_length..].trim().is_empty()
}

fn split_indentation(line: &str) -> (usize, &str) {
    let mut columns = 0usize;

    for (index, ch) in line.char_indices() {
        match ch {
            ' ' => columns += 1,
            '\t' => columns += 4 - (columns % 4),
            _ => return (columns, &line[index..]),
        }
    }

    (columns, "")
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn display_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Untitled")
        .to_owned()
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("document.md")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::{
        looks_like_raw_mermaid_document, new_document, render_file, rewrite_mermaid_blocks,
        untitled_document,
    };
    use std::{fs, path::PathBuf};

    use crate::trusted_preview::TRUST_MODEL;

    struct FixtureExpectation {
        file_name: &'static str,
        mermaid_blocks: usize,
        contains: &'static [&'static str],
        excludes: &'static [&'static str],
    }

    #[test]
    fn rewrites_backtick_and_tilde_mermaid_fences() {
        let input = r#"
```mermaid
flowchart LR
  A["quoted & linked"] --> B
```

~~~mermaid
sequenceDiagram
  Alice->>Bob: Hello <world>
~~~
"#;

        let rewritten = rewrite_mermaid_blocks(input);

        assert_eq!(rewritten.matches("<pre class=\"mermaid\">").count(), 2);
        assert!(rewritten.contains("A[&quot;quoted &amp; linked&quot;] --&gt; B"));
        assert!(rewritten.contains("Alice-&gt;&gt;Bob: Hello &lt;world&gt;"));
        assert!(!rewritten.contains("```mermaid"));
        assert!(!rewritten.contains("~~~mermaid"));
    }

    #[test]
    fn leaves_non_mermaid_code_fences_untouched() {
        let input = r#"
```rust
fn main() {
    println!("hello");
}
```
"#;

        let rewritten = rewrite_mermaid_blocks(input);

        assert!(rewritten.contains("```rust"));
        assert!(!rewritten.contains("<pre class=\"mermaid\">"));
    }

    #[test]
    fn leaves_indented_mermaid_fences_as_code_blocks() {
        let input = r#"
    ```mermaid
    flowchart TD
      A --> B
    ```
"#;

        let rewritten = rewrite_mermaid_blocks(input);

        assert!(rewritten.contains("```mermaid"));
        assert!(rewritten.contains("flowchart TD"));
        assert!(!rewritten.contains("<pre class=\"mermaid\">"));
    }

    #[test]
    fn renders_raw_mermaid_documents_without_fences() {
        let rendered = untitled_document(
            "Untitled",
            r#"
flowchart TD
  Start --> Finish
"#,
        );

        assert!(rendered.html.contains("<pre class=\"mermaid\">"));
        assert!(rendered.html.contains("flowchart TD"));
        assert!(rendered.html.contains("Start --&gt; Finish"));
        assert!(!rendered.html.contains("<p>flowchart TD"));
    }

    #[test]
    fn does_not_treat_plain_text_as_raw_mermaid() {
        assert!(!looks_like_raw_mermaid_document("graph theory is fun"));
        assert!(!looks_like_raw_mermaid_document("graphTD"));
        assert!(!looks_like_raw_mermaid_document("flowchartLR"));

        let rendered = untitled_document("Untitled", "graph theory is fun");
        assert!(rendered.html.contains("<p>graph theory is fun</p>"));
        assert!(!rendered.html.contains("<pre class=\"mermaid\">"));

        let compact_graph_word = untitled_document("Untitled", "graphTD");
        assert!(compact_graph_word.html.contains("<p>graphTD</p>"));
        assert!(!compact_graph_word.html.contains("<pre class=\"mermaid\">"));
    }

    #[test]
    fn renders_all_synthetic_mermaid_fixtures() {
        for expectation in fixture_expectations() {
            let rendered =
                render_file(&fixture_path(expectation.file_name), false).unwrap_or_else(|error| {
                    panic!("failed to render {}: {error:#}", expectation.file_name)
                });

            assert_eq!(
                rendered.html.matches("<pre class=\"mermaid\">").count(),
                expectation.mermaid_blocks,
                "unexpected Mermaid block count in {}",
                expectation.file_name
            );

            for expected in expectation.contains {
                assert!(
                    rendered.html.contains(expected),
                    "expected {:?} in {}",
                    expected,
                    expectation.file_name
                );
            }

            for excluded in expectation.excludes {
                assert!(
                    !rendered.html.contains(excluded),
                    "did not expect {:?} in {}",
                    excluded,
                    expectation.file_name
                );
            }
        }
    }

    #[test]
    fn synthetic_fixture_inventory_is_stable() {
        let mut actual = fs::read_dir(fixtures_dir())
            .expect("failed to list synthetic fixtures")
            .map(|entry| entry.expect("invalid fixture entry").path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("md"))
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        actual.sort();

        let mut expected = fixture_expectations()
            .iter()
            .map(|fixture| fixture.file_name.to_owned())
            .collect::<Vec<_>>();
        expected.sort();

        assert_eq!(actual, expected, "fixture list and test catalog drifted");
    }

    #[test]
    fn trusted_preview_documents_are_explicitly_marked() {
        let empty = new_document();
        assert_eq!(empty.trust_model, TRUST_MODEL);

        let untitled = untitled_document("Untitled", "# Draft");
        assert_eq!(untitled.title, "Untitled");
        assert_eq!(untitled.trust_model, TRUST_MODEL);
        assert!(untitled.html.contains("<h1>Draft</h1>"));

        let rendered = render_file(&fixture_path("flow-and-sequence.md"), false)
            .expect("failed to render trusted preview fixture");
        assert_eq!(rendered.trust_model, TRUST_MODEL);
    }

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("sample")
            .join("synthetic")
    }

    fn fixture_path(file_name: &str) -> PathBuf {
        fixtures_dir().join(file_name)
    }

    fn fixture_expectations() -> &'static [FixtureExpectation] {
        &[
            FixtureExpectation {
                file_name: "flow-and-sequence.md",
                mermaid_blocks: 2,
                contains: &[
                    "<h1>Flow And Sequence</h1>",
                    "flowchart TD",
                    "sequenceDiagram",
                    "<strong>bold</strong>",
                    "language-json",
                ],
                excludes: &["```mermaid", "~~~mermaid"],
            },
            FixtureExpectation {
                file_name: "class-and-er.md",
                mermaid_blocks: 2,
                contains: &["classDiagram", "erDiagram", "<table>", "<li>One</li>"],
                excludes: &["```mermaid"],
            },
            FixtureExpectation {
                file_name: "gantt-and-journey.md",
                mermaid_blocks: 2,
                contains: &[
                    "gantt",
                    "journey",
                    "<blockquote>",
                    "<input type=\"checkbox\"",
                ],
                excludes: &["```mermaid"],
            },
            FixtureExpectation {
                file_name: "gitgraph-and-pie.md",
                mermaid_blocks: 2,
                contains: &[
                    "gitGraph",
                    "pie title Release split",
                    "<code>inline code</code>",
                ],
                excludes: &["```mermaid"],
            },
            FixtureExpectation {
                file_name: "mindmap-and-timeline.md",
                mermaid_blocks: 2,
                contains: &[
                    "mindmap",
                    "timeline",
                    "<a href=\"https://example.com\">https://example.com</a>",
                ],
                excludes: &["```mermaid"],
            },
            FixtureExpectation {
                file_name: "state-and-architecture.md",
                mermaid_blocks: 2,
                contains: &["stateDiagram-v2", "architecture-beta", "language-rust"],
                excludes: &["```mermaid"],
            },
        ]
    }
}
