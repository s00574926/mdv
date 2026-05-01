use anyhow::{Context, Result};
use comrak::{
    Arena, format_html,
    nodes::{AstNode, NodeValue},
    parse_document,
};
use serde::Serialize;
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use crate::trusted_preview;

const LOCAL_ASSET_URL_PREFIX: &str = "mdv-local-asset:";
const LOCAL_MARKDOWN_URL_PREFIX: &str = "mdv-local-markdown:";

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
        path.parent(),
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
    render_markdown(title, "", "", markdown, false, None)
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
    base_dir: Option<&Path>,
) -> RenderedDocument {
    let markdown = markdown.strip_prefix('\u{feff}').unwrap_or(markdown);
    let transformed = rewrite_mermaid_content(markdown);
    let options = trusted_preview::markdown_options();
    let arena = Arena::new();
    let root = parse_document(&arena, &transformed, &options);
    rewrite_relative_references(root, base_dir);

    let mut html = String::new();
    format_html(root, &options, &mut html).expect("formatting HTML into a String should not fail");

    RenderedDocument {
        title: title.to_owned(),
        html,
        source_name: source_name.to_owned(),
        source_path: source_path.to_owned(),
        watching,
        trust_model: trusted_preview::TRUST_MODEL,
    }
}

fn rewrite_relative_references<'a>(root: &'a AstNode<'a>, base_dir: Option<&Path>) {
    let Some(base_dir) = base_dir else {
        return;
    };

    for node in root.descendants() {
        let mut data = node.data_mut();
        match data.value {
            NodeValue::Image(ref mut link) => {
                if let Some(reference) = resolve_local_reference(base_dir, &link.url) {
                    link.url = format!(
                        "{}{}{}",
                        LOCAL_ASSET_URL_PREFIX,
                        percent_encode(&reference.path.display().to_string()),
                        reference.suffix
                    );
                }
            }
            NodeValue::Link(ref mut link) => {
                if let Some(reference) = resolve_local_reference(base_dir, &link.url)
                    && is_markdown_path(&reference.path)
                {
                    link.url = format!(
                        "{}{}{}",
                        LOCAL_MARKDOWN_URL_PREFIX,
                        percent_encode(&reference.path.display().to_string()),
                        reference.suffix
                    );
                }
            }
            _ => {}
        }
    }
}

struct LocalReference {
    path: PathBuf,
    suffix: String,
}

fn resolve_local_reference(base_dir: &Path, url: &str) -> Option<LocalReference> {
    if url.is_empty() || url.starts_with('#') || url.starts_with("//") || url_has_scheme(url) {
        return None;
    }

    let (path_part, suffix) = split_reference_suffix(url);
    if path_part.is_empty() {
        return None;
    }

    let decoded_path = percent_decode(path_part)?;
    let candidate = PathBuf::from(decoded_path);
    let path = if candidate.is_absolute() {
        candidate
    } else {
        base_dir.join(candidate)
    };

    Some(LocalReference {
        path: normalize_path_components(&path),
        suffix: suffix.to_owned(),
    })
}

fn split_reference_suffix(url: &str) -> (&str, &str) {
    let suffix_start = url
        .char_indices()
        .find_map(|(index, ch)| (ch == '?' || ch == '#').then_some(index))
        .unwrap_or(url.len());

    (&url[..suffix_start], &url[suffix_start..])
}

fn url_has_scheme(url: &str) -> bool {
    let Some(colon_index) = url.find(':') else {
        return false;
    };

    let scheme = &url[..colon_index];
    if scheme.len() == 1 && url.as_bytes()[0].is_ascii_alphabetic() {
        return false;
    }

    scheme
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.'))
}

fn is_markdown_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("md"))
}

fn normalize_path_components(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() && !normalized.has_root() {
                    normalized.push(component.as_os_str());
                }
            }
            _ => normalized.push(component.as_os_str()),
        }
    }

    normalized
}

fn percent_encode(value: &str) -> String {
    let mut output = String::new();

    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            output.push(char::from(byte));
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }

    output
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0usize;

    while index < bytes.len() {
        if bytes[index] != b'%' {
            output.push(bytes[index]);
            index += 1;
            continue;
        }

        let high = hex_value(*bytes.get(index + 1)?)?;
        let low = hex_value(*bytes.get(index + 2)?)?;
        output.push((high << 4) | low);
        index += 3;
    }

    String::from_utf8(output).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn rewrite_mermaid_content(markdown: &str) -> String {
    let (rewritten, rewrote_mermaid_block) = rewrite_mermaid_blocks(markdown);
    if rewrote_mermaid_block {
        return rewritten;
    }

    if let Some(raw_mermaid) = render_raw_mermaid_document(markdown) {
        return raw_mermaid;
    }

    rewritten
}

fn rewrite_mermaid_blocks(markdown: &str) -> (String, bool) {
    let mut output = String::new();
    let mut mermaid_buffer = Vec::new();
    let mut fence_marker = None;
    let mut fence_length = 0usize;
    let mut opening_fence = String::new();
    let mut rewrote_mermaid_block = false;

    for line in markdown.lines() {
        if let Some(marker) = fence_marker {
            if is_fence_close(line, marker, fence_length) {
                output.push_str("<pre class=\"mermaid\">");
                output.push_str(&escape_html(&mermaid_buffer.join("\n")));
                output.push_str("</pre>\n");
                rewrote_mermaid_block = true;
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

    (output, rewrote_mermaid_block)
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
    if markdown.trim().is_empty() || contains_markdown_fence(markdown) {
        return false;
    }

    for line in mermaid_detection_body(markdown).lines() {
        let (indent, content) = split_indentation(line);
        let line = content.trim();
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }

        return indent <= 3 && is_mermaid_root_line(line);
    }

    false
}

fn contains_markdown_fence(markdown: &str) -> bool {
    markdown.lines().any(|line| {
        let (indent, content) = split_indentation(line);
        if indent > 3 {
            return false;
        }

        let Some(marker) = content.chars().next() else {
            return false;
        };
        if marker != '`' && marker != '~' {
            return false;
        }

        content.chars().take_while(|ch| *ch == marker).count() >= 3
    })
}

fn mermaid_detection_body(markdown: &str) -> &str {
    let mut frontmatter_start = 0usize;

    for line in markdown.split_inclusive('\n') {
        let trimmed_line = line.trim_end_matches(&['\r', '\n'][..]).trim();
        if trimmed_line.is_empty() {
            frontmatter_start += line.len();
            continue;
        }

        if trimmed_line != "---" {
            return markdown;
        }

        let mut next_line_start = frontmatter_start + line.len();
        for frontmatter_line in markdown[next_line_start..].split_inclusive('\n') {
            let trimmed_frontmatter_line =
                frontmatter_line.trim_end_matches(&['\r', '\n'][..]).trim();
            next_line_start += frontmatter_line.len();
            if trimmed_frontmatter_line == "---" {
                return &markdown[next_line_start..];
            }
        }

        return markdown;
    }

    markdown
}

fn is_mermaid_root_line(line: &str) -> bool {
    if is_mermaid_flow_root_line(line, "graph") {
        return true;
    }

    if is_mermaid_flow_root_line(line, "flowchart") {
        return true;
    }

    if is_mermaid_flow_root_line(line, "flowchart-elk") {
        return true;
    }

    if is_mermaid_pie_root_line(line) {
        return true;
    }

    if is_mermaid_git_graph_root_line(line) {
        return true;
    }

    if is_mermaid_xychart_root_line(line) {
        return true;
    }

    const MERMAID_EXACT_ROOTS: &[&str] = &[
        "sequenceDiagram",
        "classDiagram",
        "classDiagram-v2",
        "stateDiagram",
        "stateDiagram-v2",
        "erDiagram",
        "journey",
        "gantt",
        "info",
        "mindmap",
        "timeline",
        "zenuml",
        "quadrantChart",
        "requirementDiagram",
        "sankey",
        "sankey-beta",
        "architecture-beta",
        "block",
        "block-beta",
        "packet",
        "kanban",
        "packet-beta",
        "ishikawa",
        "ishikawa-beta",
        "radar-beta",
        "treemap",
        "treemap-beta",
        "treeView-beta",
        "venn-beta",
        "wardley-beta",
        "C4Context",
        "C4Container",
        "C4Component",
        "C4Dynamic",
        "C4Deployment",
    ];

    MERMAID_EXACT_ROOTS.contains(&line)
}

fn is_mermaid_xychart_root_line(line: &str) -> bool {
    is_mermaid_optional_token_root_line(line, "xychart-beta", &["horizontal", "vertical"])
        || is_mermaid_optional_token_root_line(line, "xychart", &["horizontal", "vertical"])
}

fn is_mermaid_git_graph_root_line(line: &str) -> bool {
    let Some(suffix) = line.strip_prefix("gitGraph") else {
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
        .map(|token| token.trim_end_matches(':'))
        .is_some_and(is_mermaid_flow_direction)
}

fn is_mermaid_pie_root_line(line: &str) -> bool {
    let Some(suffix) = line.strip_prefix("pie") else {
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
        .is_some_and(|token| matches!(token, "title" | "showData"))
}

fn is_mermaid_optional_token_root_line(line: &str, root: &str, tokens: &[&str]) -> bool {
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
        .is_some_and(|token| tokens.contains(&token.trim_end_matches(';')))
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
        LOCAL_ASSET_URL_PREFIX, LOCAL_MARKDOWN_URL_PREFIX, looks_like_raw_mermaid_document,
        new_document, percent_decode, render_file, render_markdown, rewrite_mermaid_blocks,
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

        let (rewritten, rewrote_mermaid_block) = rewrite_mermaid_blocks(input);

        assert!(rewrote_mermaid_block);
        assert_eq!(rewritten.matches("<pre class=\"mermaid\">").count(), 2);
        assert!(rewritten.contains("A[&quot;quoted &amp; linked&quot;] --&gt; B"));
        assert!(rewritten.contains("Alice-&gt;&gt;Bob: Hello &lt;world&gt;"));
        assert!(!rewritten.contains("```mermaid"));
        assert!(!rewritten.contains("~~~mermaid"));
    }

    #[test]
    fn rewrites_mermaid_fences_after_utf8_bom() {
        let rendered = untitled_document(
            "Untitled",
            "\u{feff}```mermaid\nflowchart LR\n  A --> B\n```",
        );

        assert!(rendered.html.contains("<pre class=\"mermaid\">"));
        assert!(rendered.html.contains("flowchart LR"));
        assert!(!rendered.html.contains("```mermaid"));
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

        let (rewritten, rewrote_mermaid_block) = rewrite_mermaid_blocks(input);

        assert!(!rewrote_mermaid_block);
        assert!(rewritten.contains("```rust"));
        assert!(!rewritten.contains("<pre class=\"mermaid\">"));
    }

    #[test]
    fn rewrites_relative_images_and_markdown_links_to_local_preview_references() {
        let base_dir = PathBuf::from(r"C:\docs\guide");
        let rendered = render_markdown(
            "Guide",
            "guide.md",
            r"C:\docs\guide\guide.md",
            r#"
![Diagram](assets/diagram%201.png)

[Next](../next.md#intro)
[Website](https://example.com)
[Anchor](#local)
"#,
            true,
            Some(&base_dir),
        );

        let asset_path =
            extract_prefixed_attribute(&rendered.html, "src=\"", LOCAL_ASSET_URL_PREFIX);
        assert_eq!(
            percent_decode(asset_path).expect("expected valid encoded asset path"),
            r"C:\docs\guide\assets\diagram 1.png"
        );

        let link_path =
            extract_prefixed_attribute(&rendered.html, "href=\"", LOCAL_MARKDOWN_URL_PREFIX);
        let (encoded_path, suffix) = link_path
            .split_once("#intro")
            .expect("expected markdown link fragment");
        assert_eq!(suffix, "");
        assert_eq!(
            percent_decode(encoded_path).expect("expected valid encoded markdown path"),
            r"C:\docs\next.md"
        );

        assert!(rendered.html.contains("href=\"https://example.com\""));
        assert!(rendered.html.contains("href=\"#local\""));
    }

    #[test]
    fn leaves_relative_non_markdown_links_as_normal_links() {
        let base_dir = PathBuf::from(r"C:\docs");
        let rendered = render_markdown(
            "Guide",
            "guide.md",
            r"C:\docs\guide.md",
            "[PDF](assets/spec.pdf)",
            true,
            Some(&base_dir),
        );

        assert!(rendered.html.contains("href=\"assets/spec.pdf\""));
        assert!(!rendered.html.contains(LOCAL_MARKDOWN_URL_PREFIX));
    }

    #[test]
    fn leaves_indented_mermaid_fences_as_code_blocks() {
        let input = r#"
    ```mermaid
    flowchart TD
      A --> B
    ```
"#;

        let (rewritten, rewrote_mermaid_block) = rewrite_mermaid_blocks(input);

        assert!(!rewrote_mermaid_block);
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
    fn renders_raw_mermaid_documents_with_utf8_bom() {
        let rendered = untitled_document("Untitled", "\u{feff}flowchart TD\n  Start --> Finish");

        assert!(rendered.html.contains("<pre class=\"mermaid\">"));
        assert!(rendered.html.contains("flowchart TD"));
        assert!(!rendered.html.contains("<p>"));
    }

    #[test]
    fn renders_raw_mermaid_documents_with_yaml_frontmatter() {
        let rendered = untitled_document(
            "Untitled",
            r#"---
title: Release graph
---
flowchart TD
  Start --> Finish
"#,
        );

        assert!(rendered.html.contains("<pre class=\"mermaid\">"));
        assert!(rendered.html.contains("title: Release graph"));
        assert!(rendered.html.contains("flowchart TD"));
        assert!(!rendered.html.contains("<hr />"));
        assert!(!rendered.html.contains("<p>flowchart TD"));
    }

    #[test]
    fn renders_raw_mermaid_pie_documents_with_title() {
        let rendered = untitled_document(
            "Untitled",
            r#"
pie title Release split
  "Done" : 8
  "Todo" : 2
"#,
        );

        assert!(rendered.html.contains("<pre class=\"mermaid\">"));
        assert!(rendered.html.contains("pie title Release split"));
        assert!(!rendered.html.contains("<p>pie title Release split"));
    }

    #[test]
    fn renders_raw_mermaid_git_graph_documents_with_direction() {
        let rendered = untitled_document(
            "Untitled",
            r#"
gitGraph TB:
  commit id: "init"
"#,
        );

        assert!(rendered.html.contains("<pre class=\"mermaid\">"));
        assert!(rendered.html.contains("gitGraph TB:"));
        assert!(!rendered.html.contains("<p>gitGraph TB:"));
    }

    #[test]
    fn renders_raw_mermaid_info_documents() {
        let rendered = untitled_document("Untitled", "info");

        assert!(rendered.html.contains("<pre class=\"mermaid\">"));
        assert!(rendered.html.contains("info"));
        assert!(!rendered.html.contains("<p>info</p>"));
    }

    #[test]
    fn renders_raw_mermaid_venn_documents() {
        let rendered = untitled_document("Untitled", "venn-beta\nset Bugs");

        assert!(rendered.html.contains("<pre class=\"mermaid\">"));
        assert!(rendered.html.contains("venn-beta"));
        assert!(!rendered.html.contains("<p>venn-beta"));
    }

    #[test]
    fn renders_raw_mermaid_flowchart_elk_documents() {
        let rendered = untitled_document("Untitled", "flowchart-elk TD\n  A --> B");

        assert!(rendered.html.contains("<pre class=\"mermaid\">"));
        assert!(rendered.html.contains("flowchart-elk TD"));
        assert!(!rendered.html.contains("<p>flowchart-elk TD"));
    }

    #[test]
    fn renders_raw_mermaid_xychart_documents_with_orientation() {
        let rendered = untitled_document(
            "Untitled",
            r#"xychart-beta horizontal
  title "Q1"
  x-axis [Jan, Feb]
  bar [1, 2]"#,
        );

        assert!(rendered.html.contains("<pre class=\"mermaid\">"));
        assert!(rendered.html.contains("xychart-beta horizontal"));
        assert!(!rendered.html.contains("<p>xychart-beta horizontal"));
    }

    #[test]
    fn renders_raw_mermaid_documents_with_literal_mermaid_pre_labels() {
        let rendered = untitled_document(
            "Untitled",
            r#"flowchart TD
  A["<pre class="mermaid">"] --> B"#,
        );

        assert!(
            rendered
                .html
                .contains("<pre class=\"mermaid\">flowchart TD")
        );
        assert!(
            rendered
                .html
                .contains("A[&quot;&lt;pre class=&quot;mermaid&quot;&gt;&quot;] --&gt; B")
        );
        assert!(!rendered.html.contains("<p>flowchart TD"));
    }

    #[test]
    fn renders_raw_mermaid_documents_with_literal_fence_markers_in_labels() {
        let rendered = untitled_document(
            "Untitled",
            r#"flowchart TD
  A["```"] --> B"#,
        );

        assert!(
            rendered
                .html
                .contains("<pre class=\"mermaid\">flowchart TD")
        );
        assert!(rendered.html.contains("A[&quot;```&quot;] --&gt; B"));
        assert!(!rendered.html.contains("<p>flowchart TD"));
    }

    #[test]
    fn leaves_indented_raw_mermaid_documents_as_code_blocks() {
        let rendered = untitled_document(
            "Untitled",
            r#"    flowchart TD
      A --> B"#,
        );

        assert!(!rendered.html.contains("<pre class=\"mermaid\">"));
        assert!(rendered.html.contains("<pre><code>flowchart TD"));
        assert!(rendered.html.contains("A --&gt; B"));
    }

    #[test]
    fn renders_raw_mermaid_architecture_beta_documents() {
        let rendered = untitled_document(
            "Untitled",
            r#"architecture-beta
  service api(server)[API]"#,
        );

        assert!(rendered.html.contains("<pre class=\"mermaid\">"));
        assert!(rendered.html.contains("architecture-beta"));
        assert!(!rendered.html.contains("<p>architecture-beta"));
    }

    #[test]
    fn leaves_stable_architecture_text_as_markdown() {
        let rendered = untitled_document(
            "Untitled",
            r#"architecture
  service api(server)[API]"#,
        );

        assert!(!rendered.html.contains("<pre class=\"mermaid\">"));
        assert!(rendered.html.contains("<p>architecture"));
    }

    #[test]
    fn renders_raw_mermaid_tree_treemap_and_wardley_documents() {
        for (source, root) in [
            ("treeView-beta", "treeView-beta"),
            ("treemap", "treemap"),
            ("wardley-beta", "wardley-beta"),
        ] {
            let rendered = untitled_document("Untitled", source);

            assert!(rendered.html.contains("<pre class=\"mermaid\">"));
            assert!(rendered.html.contains(root));
            assert!(!rendered.html.contains(&format!("<p>{root}</p>")));
        }
    }

    #[test]
    fn renders_raw_mermaid_stable_block_documents() {
        let rendered = untitled_document("Untitled", "block\n  columns 1\n  A");

        assert!(rendered.html.contains("<pre class=\"mermaid\">"));
        assert!(rendered.html.contains("block"));
        assert!(!rendered.html.contains("<p>block"));
    }

    #[test]
    fn renders_raw_mermaid_stable_packet_documents() {
        let rendered = untitled_document("Untitled", "packet");

        assert!(rendered.html.contains("<pre class=\"mermaid\">"));
        assert!(rendered.html.contains("packet"));
        assert!(!rendered.html.contains("<p>packet</p>"));
    }

    #[test]
    fn renders_raw_mermaid_stable_sankey_documents() {
        let rendered = untitled_document("Untitled", "sankey\nSource,Target,1");

        assert!(rendered.html.contains("<pre class=\"mermaid\">"));
        assert!(rendered.html.contains("sankey"));
        assert!(!rendered.html.contains("<p>sankey"));
    }

    #[test]
    fn renders_raw_mermaid_ishikawa_documents() {
        let rendered = untitled_document(
            "Untitled",
            r#"ishikawa-beta
  root((Release delay))
    cause((Build))
      issue(Compiler mismatch)"#,
        );

        assert!(rendered.html.contains("<pre class=\"mermaid\">"));
        assert!(rendered.html.contains("ishikawa-beta"));
        assert!(!rendered.html.contains("<p>ishikawa-beta"));
    }

    #[test]
    fn does_not_treat_plain_text_as_raw_mermaid() {
        assert!(!looks_like_raw_mermaid_document("graph theory is fun"));
        assert!(!looks_like_raw_mermaid_document("graphTD"));
        assert!(!looks_like_raw_mermaid_document("flowchartLR"));
        assert!(!looks_like_raw_mermaid_document("gitGraph is fun"));
        assert!(!looks_like_raw_mermaid_document(
            "pie is better with coffee"
        ));

        let rendered = untitled_document("Untitled", "graph theory is fun");
        assert!(rendered.html.contains("<p>graph theory is fun</p>"));
        assert!(!rendered.html.contains("<pre class=\"mermaid\">"));

        let plain_pie_sentence = untitled_document("Untitled", "pie is better with coffee");
        assert!(
            plain_pie_sentence
                .html
                .contains("<p>pie is better with coffee</p>")
        );
        assert!(!plain_pie_sentence.html.contains("<pre class=\"mermaid\">"));

        let compact_graph_word = untitled_document("Untitled", "graphTD");
        assert!(compact_graph_word.html.contains("<p>graphTD</p>"));
        assert!(!compact_graph_word.html.contains("<pre class=\"mermaid\">"));

        let plain_git_graph_sentence = untitled_document("Untitled", "gitGraph is fun");
        assert!(
            plain_git_graph_sentence
                .html
                .contains("<p>gitGraph is fun</p>")
        );
        assert!(
            !plain_git_graph_sentence
                .html
                .contains("<pre class=\"mermaid\">")
        );
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

    fn extract_prefixed_attribute<'a>(html: &'a str, attribute: &str, prefix: &str) -> &'a str {
        let start = html
            .find(&format!("{attribute}{prefix}"))
            .unwrap_or_else(|| panic!("missing {attribute}{prefix} in {html}"))
            + attribute.len()
            + prefix.len();
        let end = html[start..]
            .find('"')
            .unwrap_or_else(|| panic!("unterminated attribute in {html}"));

        &html[start..start + end]
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
