use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::workspace_payload::{ExplorerNode, ExplorerNodeKind, ExplorerRoot};

const SKIPPED_DIRECTORY_NAMES: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    "coverage",
];

pub struct ScannedExplorerRoot {
    pub root: ExplorerRoot,
    pub first_markdown: Option<PathBuf>,
}

pub fn placeholder_root(path: &Path) -> ExplorerRoot {
    ExplorerRoot {
        name: path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("Folder")
            .to_owned(),
        path: path.display().to_string(),
        children: Vec::new(),
    }
}

pub fn scan_root(path: &Path) -> Result<ScannedExplorerRoot> {
    let (children, first_markdown) = build_nodes(path)?;

    Ok(ScannedExplorerRoot {
        root: ExplorerRoot {
            children,
            ..placeholder_root(path)
        },
        first_markdown,
    })
}

pub fn build_root(path: &Path) -> Result<ExplorerRoot> {
    Ok(scan_root(path)?.root)
}

fn build_nodes(path: &Path) -> Result<(Vec<ExplorerNode>, Option<PathBuf>)> {
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
    let mut first_markdown = None;
    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("Failed to inspect {}", path.display()))?;
        let entry_name = entry.file_name().to_string_lossy().into_owned();

        if should_skip_entry(&entry_name, &file_type) {
            continue;
        }

        if file_type.is_dir() {
            let (children, first_markdown_in_directory) = build_nodes(&path)?;
            if first_markdown.is_none() {
                first_markdown = first_markdown_in_directory.clone();
            }
            if children.is_empty() && first_markdown_in_directory.is_none() {
                continue;
            }
            nodes.push(ExplorerNode {
                name: entry_name,
                path: path.display().to_string(),
                kind: ExplorerNodeKind::Directory,
                children,
            });
            continue;
        }

        if file_type.is_file()
            && path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("md"))
        {
            if first_markdown.is_none() {
                first_markdown = Some(path.clone());
            }
            nodes.push(ExplorerNode {
                name: entry_name,
                path: path.display().to_string(),
                kind: ExplorerNodeKind::File,
                children: Vec::new(),
            });
        }
    }

    Ok((nodes, first_markdown))
}

fn should_skip_entry(name: &str, file_type: &fs::FileType) -> bool {
    if file_type.is_symlink() {
        return true;
    }

    file_type.is_dir() && (name.starts_with('.') || SKIPPED_DIRECTORY_NAMES.contains(&name))
}

#[cfg(test)]
mod tests {
    use super::{build_root, scan_root};
    use crate::test_support::filesystem_test_lock;
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
        let _filesystem_test_lock = filesystem_test_lock();
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
        assert!(
            explorer
                .children
                .iter()
                .any(|node| node.name == "readme.md")
        );

        cleanup_test_dir(&root);
    }

    #[test]
    fn find_first_markdown_file_prefers_sorted_directory_order() {
        let _filesystem_test_lock = filesystem_test_lock();
        let root = unique_test_dir("explorer-first");
        fs::create_dir_all(&root).expect("failed to create root dir");
        fs::create_dir_all(root.join("a-dir")).expect("failed to create first dir");
        fs::create_dir_all(root.join("z-dir")).expect("failed to create second dir");
        fs::write(root.join("z-dir").join("later.md"), "# later").expect("failed to write later");
        fs::write(root.join("a-dir").join("first.md"), "# first").expect("failed to write first");

        let first = scan_root(&root).expect("failed to scan dir").first_markdown;
        assert_eq!(
            first.expect("expected markdown file"),
            root.join("a-dir").join("first.md")
        );

        cleanup_test_dir(&root);
    }

    #[test]
    fn find_first_markdown_file_returns_none_when_absent() {
        let _filesystem_test_lock = filesystem_test_lock();
        let root = unique_test_dir("explorer-empty");
        fs::create_dir_all(&root).expect("failed to create root");
        fs::write(root.join("notes.txt"), "ignore").expect("failed to write non-markdown");

        let first = scan_root(&root).expect("failed to scan dir").first_markdown;
        assert!(first.is_none());

        cleanup_test_dir(&root);
    }

    #[test]
    fn scan_root_returns_tree_and_first_markdown_together() {
        let _filesystem_test_lock = filesystem_test_lock();
        let root = unique_test_dir("explorer-scan-root");
        fs::create_dir_all(root.join("a-dir")).expect("failed to create first dir");
        fs::write(root.join("a-dir").join("first.md"), "# first").expect("failed to write first");
        fs::write(root.join("readme.md"), "# readme").expect("failed to write readme");

        let scanned = scan_root(&root).expect("failed to scan root");

        assert_eq!(scanned.root.children.len(), 2);
        assert_eq!(
            scanned.first_markdown,
            Some(root.join("a-dir").join("first.md"))
        );

        cleanup_test_dir(&root);
    }

    #[test]
    fn skips_generated_and_hidden_directories() {
        let _filesystem_test_lock = filesystem_test_lock();
        let root = unique_test_dir("explorer-skip-heavy-dirs");
        fs::create_dir_all(root.join("docs")).expect("failed to create docs dir");
        fs::create_dir_all(root.join("node_modules").join("pkg"))
            .expect("failed to create node_modules dir");
        fs::create_dir_all(root.join(".git").join("hooks")).expect("failed to create hidden dir");
        fs::write(root.join("docs").join("guide.md"), "# Guide")
            .expect("failed to write docs markdown");
        fs::write(
            root.join("node_modules").join("pkg").join("README.md"),
            "# Dependency",
        )
        .expect("failed to write dependency markdown");
        fs::write(root.join(".git").join("hooks").join("notes.md"), "# Hidden")
            .expect("failed to write hidden markdown");

        let scanned = scan_root(&root).expect("failed to scan root");

        assert_eq!(scanned.root.children.len(), 1);
        assert_eq!(scanned.root.children[0].name, "docs");
        assert_eq!(
            scanned.first_markdown,
            Some(root.join("docs").join("guide.md"))
        );

        cleanup_test_dir(&root);
    }

    #[test]
    fn omits_directories_without_markdown_descendants() {
        let _filesystem_test_lock = filesystem_test_lock();
        let root = unique_test_dir("explorer-skip-empty-dirs");
        fs::create_dir_all(root.join("empty-dir").join("nested"))
            .expect("failed to create empty dir");
        fs::create_dir_all(root.join("docs")).expect("failed to create docs dir");
        fs::write(root.join("docs").join("guide.md"), "# Guide")
            .expect("failed to write docs markdown");
        fs::write(
            root.join("empty-dir").join("nested").join("notes.txt"),
            "ignore",
        )
        .expect("failed to write text file");

        let explorer = build_root(&root).expect("failed to build root");

        assert_eq!(explorer.children.len(), 1);
        assert_eq!(explorer.children[0].name, "docs");

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
