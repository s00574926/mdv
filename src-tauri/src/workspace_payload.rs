use serde::Serialize;

use crate::markdown;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePayload {
    pub document: markdown::RenderedDocument,
    pub editor_text: Option<String>,
    pub current_file_path: Option<String>,
    pub explorer: Option<ExplorerRoot>,
    pub explorer_updated: bool,
    pub recent_paths: Vec<String>,
    pub document_tabs: Vec<DocumentTabPayload>,
    pub active_document_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentTabPayload {
    pub label: String,
    pub is_untitled: bool,
    pub has_unsaved_content: bool,
    pub is_active: bool,
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
