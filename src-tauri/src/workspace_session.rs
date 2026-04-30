use anyhow::{Result, anyhow, bail};
use notify::RecommendedWatcher;
use std::path::PathBuf;
use tauri::State;

use crate::{
    state::{AppSession, AppState, OpenDocumentSession},
    workspace_payload::DocumentTabPayload,
};

#[derive(Clone)]
pub struct WorkspaceSnapshot {
    pub active_document: Option<OpenDocumentSession>,
    pub active_document_index: Option<usize>,
    pub watching: bool,
    pub recent_paths: Vec<PathBuf>,
    pub document_tabs: Vec<DocumentTabSnapshot>,
}

#[derive(Clone)]
pub struct DocumentTabSnapshot {
    pub label: String,
    pub is_untitled: bool,
    pub has_unsaved_content: bool,
}

pub fn create_untitled_document(state: &State<'_, AppState>) -> Result<()> {
    let mut session = lock_session(state)?;
    let untitled_number = session.next_untitled_number;
    session.next_untitled_number += 1;
    session.documents.push(OpenDocumentSession {
        path: None,
        directory: None,
        untitled_number: Some(untitled_number),
        content: String::new(),
    });
    session.active_document_index = Some(session.documents.len() - 1);
    session.current_document_watcher = None;
    session.explorer_watcher = None;
    session.watched_document_path = None;
    session.watched_explorer_root = None;
    session.rendered_document = None;
    Ok(())
}

pub fn add_document(
    state: &State<'_, AppState>,
    path: Option<PathBuf>,
    directory: Option<PathBuf>,
) -> Result<()> {
    let mut session = lock_session(state)?;
    session.documents.push(OpenDocumentSession {
        path,
        directory,
        untitled_number: None,
        content: String::new(),
    });
    session.active_document_index = Some(session.documents.len() - 1);
    session.current_document_watcher = None;
    session.explorer_watcher = None;
    Ok(())
}

pub fn replace_active_document(
    state: &State<'_, AppState>,
    path: Option<PathBuf>,
    directory: Option<PathBuf>,
) -> Result<()> {
    let mut session = lock_session(state)?;
    let document = OpenDocumentSession {
        path,
        directory,
        untitled_number: None,
        content: String::new(),
    };

    match session.active_document_index {
        Some(index) if index < session.documents.len() => session.documents[index] = document,
        _ => {
            session.documents.push(document);
            session.active_document_index = Some(session.documents.len() - 1);
        }
    }

    session.current_document_watcher = None;
    session.explorer_watcher = None;
    Ok(())
}

pub fn set_active_document_index(state: &State<'_, AppState>, index: usize) -> Result<()> {
    let mut session = lock_session(state)?;
    if index >= session.documents.len() {
        bail!("That document tab no longer exists.");
    }

    session.active_document_index = Some(index);
    session.current_document_watcher = None;
    session.explorer_watcher = None;
    Ok(())
}

pub fn close_document(state: &State<'_, AppState>, index: usize) -> Result<()> {
    let mut session = lock_session(state)?;
    close_document_in_session(&mut session, index)
}

pub fn update_document_content(
    state: &State<'_, AppState>,
    index: usize,
    content: String,
) -> Result<()> {
    let mut session = lock_session(state)?;
    update_document_content_in_session(&mut session, index, content)
}

pub fn set_active_watchers(
    state: &State<'_, AppState>,
    current_document_watcher: Option<RecommendedWatcher>,
    explorer_watcher: Option<RecommendedWatcher>,
    watched_document_path: Option<PathBuf>,
    watched_explorer_root: Option<PathBuf>,
) -> Result<()> {
    let mut session = lock_session(state)?;
    session.current_document_watcher = current_document_watcher;
    session.explorer_watcher = explorer_watcher;
    session.watched_document_path = watched_document_path;
    session.watched_explorer_root = watched_explorer_root;
    Ok(())
}

pub fn watched_document_path(state: &State<'_, AppState>) -> Result<Option<PathBuf>> {
    let session = lock_session(state)?;
    Ok(session.watched_document_path.clone())
}

pub fn watched_explorer_root(state: &State<'_, AppState>) -> Result<Option<PathBuf>> {
    let session = lock_session(state)?;
    Ok(session.watched_explorer_root.clone())
}

pub fn current_directory(state: &State<'_, AppState>) -> Result<Option<PathBuf>> {
    let session = lock_session(state)?;
    let Some(index) = session.active_document_index else {
        return Ok(None);
    };

    Ok(session
        .documents
        .get(index)
        .and_then(|document| document.directory.clone()))
}

pub fn active_document_is_untitled(state: &State<'_, AppState>) -> Result<bool> {
    let session = lock_session(state)?;
    let Some(index) = session.active_document_index else {
        return Ok(false);
    };

    Ok(session
        .documents
        .get(index)
        .is_some_and(|document| document.path.is_none() && document.directory.is_none()))
}

pub fn active_document_suggested_name(state: &State<'_, AppState>) -> Result<Option<String>> {
    let session = lock_session(state)?;
    let Some(index) = session.active_document_index else {
        return Ok(None);
    };

    let Some(document) = session.documents.get(index) else {
        return Ok(None);
    };

    if document.path.is_some() || document.directory.is_some() {
        return Ok(None);
    }

    Ok(Some(format!(
        "{}.md",
        untitled_label(document.untitled_number)
    )))
}

pub fn active_document_content(state: &State<'_, AppState>) -> Result<String> {
    let session = lock_session(state)?;
    let Some(index) = session.active_document_index else {
        bail!("No document is selected.");
    };
    let Some(document) = session.documents.get(index) else {
        bail!("That document tab no longer exists.");
    };

    Ok(document.content.clone())
}

pub fn snapshot(state: &AppState) -> Result<WorkspaceSnapshot> {
    let session = lock_session(state)?;
    let active_document = session
        .active_document_index
        .and_then(|index| session.documents.get(index).cloned());

    Ok(WorkspaceSnapshot {
        active_document,
        active_document_index: session.active_document_index,
        watching: session
            .active_document_index
            .and_then(|index| session.documents.get(index))
            .and_then(|document| document.path.as_ref())
            .is_some()
            && session.current_document_watcher.is_some(),
        recent_paths: session.recent_paths.clone(),
        document_tabs: session
            .documents
            .iter()
            .map(|document| DocumentTabSnapshot {
                label: document_label(document),
                is_untitled: document.path.is_none() && document.directory.is_none(),
                has_unsaved_content: document_has_unsaved_content(document),
            })
            .collect(),
    })
}

pub fn build_document_tabs(
    snapshot: &[DocumentTabSnapshot],
    active_index: Option<usize>,
) -> Vec<DocumentTabPayload> {
    snapshot
        .iter()
        .enumerate()
        .map(|(index, document)| DocumentTabPayload {
            label: document.label.clone(),
            is_untitled: document.is_untitled,
            has_unsaved_content: document.has_unsaved_content,
            is_active: active_index == Some(index),
        })
        .collect()
}

fn document_has_unsaved_content(document: &OpenDocumentSession) -> bool {
    document.path.is_none() && document.directory.is_none() && !document.content.trim().is_empty()
}

fn document_label(document: &OpenDocumentSession) -> String {
    if let Some(path) = document.path.as_ref() {
        return path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Untitled")
            .to_owned();
    }

    if let Some(directory) = document.directory.as_ref() {
        return directory
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("Folder")
            .to_owned();
    }

    untitled_label(document.untitled_number)
}

fn update_document_content_in_session(
    session: &mut AppSession,
    index: usize,
    content: String,
) -> Result<()> {
    let Some(document) = session.documents.get_mut(index) else {
        bail!("That document tab no longer exists.");
    };
    if document.path.is_some() || document.directory.is_some() {
        bail!("Only untitled documents can be edited.");
    }

    document.content = content;
    Ok(())
}

fn close_document_in_session(session: &mut AppSession, index: usize) -> Result<()> {
    if index >= session.documents.len() {
        bail!("That document tab no longer exists.");
    }

    session.documents.remove(index);
    session.active_document_index = match session.active_document_index {
        None => None,
        Some(active_index) if active_index == index => {
            if session.documents.is_empty() {
                None
            } else if index < session.documents.len() {
                Some(index)
            } else {
                Some(session.documents.len() - 1)
            }
        }
        Some(active_index) if index < active_index => Some(active_index - 1),
        Some(active_index) => Some(active_index),
    };
    session.current_document_watcher = None;
    session.explorer_watcher = None;
    Ok(())
}

fn untitled_label(untitled_number: Option<usize>) -> String {
    match untitled_number.unwrap_or(1) {
        1 => "Untitled".to_owned(),
        number => format!("Untitled {number}"),
    }
}

fn lock_session<'a>(
    state: &'a AppState,
) -> Result<std::sync::MutexGuard<'a, crate::state::AppSession>> {
    state
        .session
        .lock()
        .map_err(|_| anyhow!("The preview state is unavailable."))
}

#[cfg(test)]
mod tests {
    use super::{
        DocumentTabSnapshot, build_document_tabs, close_document_in_session, document_label,
    };
    use crate::state::{AppSession, OpenDocumentSession};
    use std::path::PathBuf;

    #[test]
    fn labels_multiple_untitled_documents_stably() {
        let first = OpenDocumentSession {
            path: None,
            directory: None,
            untitled_number: Some(1),
            content: String::new(),
        };
        let second = OpenDocumentSession {
            path: None,
            directory: None,
            untitled_number: Some(2),
            content: String::new(),
        };

        assert_eq!(document_label(&first), "Untitled");
        assert_eq!(document_label(&second), "Untitled 2");
    }

    #[test]
    fn uses_active_index_when_building_tab_payloads() {
        let payload = build_document_tabs(
            &[
                DocumentTabSnapshot {
                    label: String::from("Untitled"),
                    is_untitled: true,
                    has_unsaved_content: true,
                },
                DocumentTabSnapshot {
                    label: String::from("notes"),
                    is_untitled: false,
                    has_unsaved_content: false,
                },
            ],
            Some(1),
        );

        assert_eq!(payload.len(), 2);
        assert!(payload[0].is_untitled);
        assert!(payload[0].has_unsaved_content);
        assert!(!payload[0].is_active);
        assert_eq!(payload[1].label, "notes");
        assert!(!payload[1].has_unsaved_content);
        assert!(payload[1].is_active);
    }

    #[test]
    fn treats_whitespace_only_untitled_content_as_empty() {
        let document = OpenDocumentSession {
            path: None,
            directory: None,
            untitled_number: Some(1),
            content: String::from("  \n\t  "),
        };

        assert!(!super::document_has_unsaved_content(&document));
    }

    #[test]
    fn marks_non_empty_untitled_content_as_unsaved() {
        let document = OpenDocumentSession {
            path: None,
            directory: None,
            untitled_number: Some(1),
            content: String::from("# Draft"),
        };

        assert!(super::document_has_unsaved_content(&document));
    }

    #[test]
    fn labels_file_backed_documents_from_their_stem() {
        let document = OpenDocumentSession {
            path: Some(PathBuf::from(r"C:\docs\plan.md")),
            directory: None,
            untitled_number: None,
            content: String::new(),
        };

        assert_eq!(document_label(&document), "plan");
    }

    #[test]
    fn updates_untitled_document_content_by_index() {
        let mut session = AppSession {
            documents: vec![OpenDocumentSession {
                path: None,
                directory: None,
                untitled_number: Some(1),
                content: String::new(),
            }],
            ..Default::default()
        };

        super::update_document_content_in_session(&mut session, 0, String::from("# Draft"))
            .expect("expected untitled document update");

        assert_eq!(session.documents[0].content, "# Draft");
    }

    #[test]
    fn rejects_editing_file_backed_documents() {
        let mut session = AppSession {
            documents: vec![OpenDocumentSession {
                path: Some(PathBuf::from(r"C:\docs\plan.md")),
                directory: None,
                untitled_number: None,
                content: String::new(),
            }],
            ..Default::default()
        };

        let error =
            super::update_document_content_in_session(&mut session, 0, String::from("# Draft"))
                .expect_err("expected file-backed document rejection");

        assert_eq!(error.to_string(), "Only untitled documents can be edited.");
    }

    #[test]
    fn closes_active_tab_and_keeps_the_next_tab_selected() {
        let mut session = AppSession {
            documents: vec![
                OpenDocumentSession {
                    path: Some(PathBuf::from(r"C:\docs\first.md")),
                    directory: None,
                    untitled_number: None,
                    content: String::new(),
                },
                OpenDocumentSession {
                    path: Some(PathBuf::from(r"C:\docs\second.md")),
                    directory: None,
                    untitled_number: None,
                    content: String::new(),
                },
                OpenDocumentSession {
                    path: Some(PathBuf::from(r"C:\docs\third.md")),
                    directory: None,
                    untitled_number: None,
                    content: String::new(),
                },
            ],
            active_document_index: Some(1),
            ..Default::default()
        };

        close_document_in_session(&mut session, 1).expect("expected active tab to close");

        assert_eq!(session.documents.len(), 2);
        assert_eq!(session.active_document_index, Some(1));
        assert_eq!(
            session.documents[1]
                .path
                .as_ref()
                .and_then(|path| path.file_name())
                .and_then(|value| value.to_str()),
            Some("third.md")
        );
    }

    #[test]
    fn closes_tab_before_active_and_shifts_the_active_index() {
        let mut session = AppSession {
            documents: vec![
                OpenDocumentSession {
                    path: Some(PathBuf::from(r"C:\docs\first.md")),
                    directory: None,
                    untitled_number: None,
                    content: String::new(),
                },
                OpenDocumentSession {
                    path: Some(PathBuf::from(r"C:\docs\second.md")),
                    directory: None,
                    untitled_number: None,
                    content: String::new(),
                },
            ],
            active_document_index: Some(1),
            ..Default::default()
        };

        close_document_in_session(&mut session, 0).expect("expected tab close");

        assert_eq!(session.documents.len(), 1);
        assert_eq!(session.active_document_index, Some(0));
        assert_eq!(
            session.documents[0]
                .path
                .as_ref()
                .and_then(|path| path.file_name())
                .and_then(|value| value.to_str()),
            Some("second.md")
        );
    }

    #[test]
    fn closing_the_last_tab_clears_the_active_index() {
        let mut session = AppSession {
            documents: vec![OpenDocumentSession {
                path: None,
                directory: None,
                untitled_number: Some(1),
                content: String::new(),
            }],
            active_document_index: Some(0),
            ..Default::default()
        };

        close_document_in_session(&mut session, 0).expect("expected tab close");

        assert!(session.documents.is_empty());
        assert_eq!(session.active_document_index, None);
    }
}
