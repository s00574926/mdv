use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::workspace_payload::{ExplorerNode, ExplorerNodeKind, ExplorerRoot};

pub fn build_root(path: &Path) -> Result<ExplorerRoot> {
    Ok(ExplorerRoot {
        name: path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("Folder")
            .to_owned(),
        path: path.display().to_string(),
        children: build_nodes(path)?,
    })
}

pub fn find_first_markdown_file(path: &Path) -> Result<Option<PathBuf>> {
    for node in build_nodes(path)? {
        if let Some(first_markdown) = first_markdown_from_node(&node) {
            return Ok(Some(first_markdown));
        }
    }

    Ok(None)
}

fn build_nodes(path: &Path) -> Result<Vec<ExplorerNode>> {
    let mut entries = fs::read_dir(path)
        .with_context(|| format!("Failed to read {}", path.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("Failed to read {}", path.display()))?;

    entries.sort_by(|left, right| {
        let left_type = left.file_type().ok();
        let right_type = right.file_type().ok();
        let left_is_dir = left_type.is_some_and(|file_type| file_type.is_dir());
        let right_is_dir = right_type.is_some_and(|file_type| file_type.is_dir());

        match right_is_dir.cmp(&left_is_dir) {
            std::cmp::Ordering::Equal => left
                .file_name()
                .to_string_lossy()
                .to_lowercase()
                .cmp(&right.file_name().to_string_lossy().to_lowercase()),
            ordering => ordering,
        }
    });

    let mut nodes = Vec::new();
    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("Failed to inspect {}", path.display()))?;

        if file_type.is_dir() {
            nodes.push(ExplorerNode {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: path.display().to_string(),
                kind: ExplorerNodeKind::Directory,
                children: build_nodes(&path)?,
            });
            continue;
        }

        if file_type.is_file()
            && path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("md"))
        {
            nodes.push(ExplorerNode {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: path.display().to_string(),
                kind: ExplorerNodeKind::File,
                children: Vec::new(),
            });
        }
    }

    Ok(nodes)
}

fn first_markdown_from_node(node: &ExplorerNode) -> Option<PathBuf> {
    match node.kind {
        ExplorerNodeKind::File => Some(PathBuf::from(&node.path)),
        ExplorerNodeKind::Directory => node.children.iter().find_map(first_markdown_from_node),
    }
}

#[cfg(test)]
mod tests {
    use super::{build_root, find_first_markdown_file};
    use crate::workspace_payload::ExplorerNodeKind;
    use std::{
        env, fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn build_root_includes_only_markdown_files() {
        let root = unique_test_dir("explorer-tree");
        fs::create_dir_all(&root).expect("failed to create root dir");
        fs::create_dir_all(root.join("docs")).expect("failed to create docs dir");
        fs::write(root.join("docs").join("guide.md"), "# Guide").expect("failed to write markdown");
        fs::write(root.join("docs").join("notes.txt"), "ignore").expect("failed to write text");
        fs::write(root.join("readme.md"), "# Readme").expect("failed to write root markdown");

        let explorer = build_root(&root).expect("failed to build root");
        assert_eq!(explorer.children.len(), 2);
        let docs = explorer
            .children
            .iter()
            .find(|node| node.name == "docs")
            .expect("missing docs dir");
        assert!(matches!(docs.kind, ExplorerNodeKind::Directory));
        assert!(explorer.children.iter().any(|node| node.name == "readme.md"));

        cleanup_test_dir(&root);
    }

    #[test]
    fn find_first_markdown_file_prefers_sorted_directory_order() {
        let root = unique_test_dir("explorer-first");
        fs::create_dir_all(&root).expect("failed to create root dir");
        fs::create_dir_all(root.join("a-dir")).expect("failed to create first dir");
        fs::create_dir_all(root.join("z-dir")).expect("failed to create second dir");
        fs::write(root.join("z-dir").join("later.md"), "# later").expect("failed to write later");
        fs::write(root.join("a-dir").join("first.md"), "# first").expect("failed to write first");

        let first = find_first_markdown_file(&root).expect("failed to scan dir");
        assert_eq!(
            first.expect("expected markdown file"),
            root.join("a-dir").join("first.md")
        );

        cleanup_test_dir(&root);
    }

    #[test]
    fn find_first_markdown_file_returns_none_when_absent() {
        let root = unique_test_dir("explorer-empty");
        fs::create_dir_all(&root).expect("failed to create root");
        fs::write(root.join("notes.txt"), "ignore").expect("failed to write non-markdown");

        let first = find_first_markdown_file(&root).expect("failed to scan dir");
        assert!(first.is_none());

        cleanup_test_dir(&root);
    }

    fn unique_test_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let sequence = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
        env::temp_dir()
            .join("mdv-tests")
            .join(format!("{nonce}-{sequence}-{name}"))
    }

    fn cleanup_test_dir(path: &Path) {
        let _ = fs::remove_dir_all(path);
    }
}
