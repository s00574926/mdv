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
