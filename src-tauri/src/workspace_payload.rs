use serde::Serialize;

use crate::markdown;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePayload {
    pub document: markdown::RenderedDocument,
    pub current_file_path: Option<String>,
    pub explorer: Option<ExplorerRoot>,
    pub recent_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerRoot {
    pub name: String,
    pub path: String,
    pub children: Vec<ExplorerNode>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerNode {
    pub name: String,
    pub path: String,
    pub kind: ExplorerNodeKind,
    pub children: Vec<ExplorerNode>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ExplorerNodeKind {
    Directory,
    File,
}
