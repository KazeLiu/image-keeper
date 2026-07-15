use crate::core::difference_finder::{
    search_difference_images, DifferenceSearchRequest, DifferenceSearchResponse,
};
use crate::core::file_operations::{
    execute_copy, execute_move, execute_rename, preview_explicit_names, preview_rename,
    preview_transfer, undo_operation_batch, OperationBatchResult, RenameExecutionItem, RenameInput,
    RenamePreviewItem, RenameRule, TransferInput, TransferPreview,
};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::{Component, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, State, Window};

#[derive(Default)]
pub struct DifferenceFinderState {
    cancelled_sessions: Mutex<HashSet<String>>,
    latest_reversible_batch: Mutex<Option<OperationBatchResult>>,
    file_operation: Mutex<()>,
}

impl DifferenceFinderState {
    pub fn begin_session(&self, session_id: &str) {
        if let Ok(mut sessions) = self.cancelled_sessions.lock() {
            sessions.remove(session_id);
        }
    }

    pub fn cancel(&self, session_id: &str) {
        if let Ok(mut sessions) = self.cancelled_sessions.lock() {
            sessions.insert(session_id.to_string());
        }
    }

    pub fn is_cancelled(&self, session_id: &str) -> bool {
        self.cancelled_sessions
            .lock()
            .map(|sessions| sessions.contains(session_id))
            .unwrap_or(true)
    }

    fn store_batch(&self, batch: OperationBatchResult) -> Result<(), String> {
        if batch.reversible {
            *self
                .latest_reversible_batch
                .lock()
                .map_err(|_| "文件操作状态锁已损坏".to_string())? = Some(batch);
        }
        Ok(())
    }

    fn get_batch(&self, batch_id: &str) -> Result<OperationBatchResult, String> {
        self.latest_reversible_batch
            .lock()
            .map_err(|_| "文件操作状态锁已损坏".to_string())?
            .as_ref()
            .filter(|batch| batch.batch_id == batch_id)
            .cloned()
            .ok_or_else(|| "找不到可撤销的操作批次".to_string())
    }

    fn consume_batch(&self, batch_id: &str) -> Result<(), String> {
        let mut latest = self
            .latest_reversible_batch
            .lock()
            .map_err(|_| "文件操作状态锁已损坏".to_string())?;
        if latest.as_ref().map(|batch| batch.batch_id.as_str()) != Some(batch_id) {
            return Err("找不到可撤销的操作批次".to_string());
        }
        latest.take();
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenamePreviewRequest {
    pub items: Vec<RenameInput>,
    pub rule: RenameRule,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameExecuteRequest {
    pub items: Vec<RenameExecutionItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferFilesRequest {
    pub files: Vec<TransferInput>,
    pub target_directory: String,
    pub new_folder_name: Option<String>,
}

#[tauri::command]
pub async fn start_difference_search(
    request: DifferenceSearchRequest,
    window: Window,
    state: State<'_, Arc<DifferenceFinderState>>,
) -> Result<DifferenceSearchResponse, String> {
    let state = state.inner().clone();
    let session_id = request.session_id.clone();
    state.begin_session(&session_id);
    tauri::async_runtime::spawn_blocking(move || {
        let progress_window = window.clone();
        let cancellation_state = state.clone();
        search_difference_images(
            request,
            move |progress| {
                let _ = progress_window.emit("difference-search-progress", progress);
            },
            move || cancellation_state.is_cancelled(&session_id),
        )
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("搜索任务异常结束: {error}"))?
}

#[tauri::command]
pub fn cancel_difference_search(
    session_id: String,
    state: State<'_, Arc<DifferenceFinderState>>,
) -> Result<(), String> {
    state.cancel(&session_id);
    Ok(())
}

#[tauri::command]
pub fn preview_difference_rename(
    request: RenamePreviewRequest,
) -> Result<Vec<RenamePreviewItem>, String> {
    Ok(preview_rename(&request.items, &request.rule))
}

#[tauri::command]
pub fn preview_difference_explicit_rename(
    request: RenameExecuteRequest,
) -> Result<Vec<RenamePreviewItem>, String> {
    Ok(preview_explicit_names(&request.items))
}

#[tauri::command]
pub fn preview_difference_transfer(
    request: TransferFilesRequest,
) -> Result<TransferPreview, String> {
    let destination = resolve_destination(&request)?;
    Ok(preview_transfer(&request.files, &destination))
}

#[tauri::command]
pub fn execute_difference_rename(
    request: RenameExecuteRequest,
    state: State<'_, Arc<DifferenceFinderState>>,
) -> Result<OperationBatchResult, String> {
    let _operation_guard = state
        .file_operation
        .lock()
        .map_err(|_| "文件操作状态锁已损坏".to_string())?;
    let batch = execute_rename(&request.items).map_err(|error| error.to_string())?;
    state.store_batch(batch.clone())?;
    Ok(batch)
}

#[tauri::command]
pub fn move_difference_files(
    request: TransferFilesRequest,
    state: State<'_, Arc<DifferenceFinderState>>,
) -> Result<OperationBatchResult, String> {
    let _operation_guard = state
        .file_operation
        .lock()
        .map_err(|_| "文件操作状态锁已损坏".to_string())?;
    let destination = resolve_destination(&request)?;
    let batch = execute_move(&request.files, &destination).map_err(|error| error.to_string())?;
    state.store_batch(batch.clone())?;
    Ok(batch)
}

#[tauri::command]
pub fn copy_difference_files(
    request: TransferFilesRequest,
    state: State<'_, Arc<DifferenceFinderState>>,
) -> Result<OperationBatchResult, String> {
    let _operation_guard = state
        .file_operation
        .lock()
        .map_err(|_| "文件操作状态锁已损坏".to_string())?;
    let destination = resolve_destination(&request)?;
    let batch = execute_copy(&request.files, &destination).map_err(|error| error.to_string())?;
    state.store_batch(batch.clone())?;
    Ok(batch)
}

#[tauri::command]
pub fn undo_difference_batch(
    batch_id: String,
    state: State<'_, Arc<DifferenceFinderState>>,
) -> Result<OperationBatchResult, String> {
    let _operation_guard = state
        .file_operation
        .lock()
        .map_err(|_| "文件操作状态锁已损坏".to_string())?;
    let original = state.get_batch(&batch_id)?;
    let result = undo_operation_batch(&original).map_err(|error| error.to_string())?;
    state.consume_batch(&batch_id)?;
    Ok(result)
}

fn resolve_destination(request: &TransferFilesRequest) -> Result<PathBuf, String> {
    let mut destination = PathBuf::from(&request.target_directory);
    if let Some(name) = request
        .new_folder_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        if invalid_folder_name(name) {
            return Err("新文件夹名称包含 Windows 不允许的字符或保留名称".to_string());
        }
        let relative = PathBuf::from(name);
        if relative.components().count() != 1
            || !matches!(relative.components().next(), Some(Component::Normal(_)))
        {
            return Err("新文件夹名称不能包含路径分隔符".to_string());
        }
        destination.push(relative);
    }
    Ok(destination)
}

fn invalid_folder_name(name: &str) -> bool {
    if name.is_empty()
        || name.ends_with([' ', '.'])
        || name
            .chars()
            .any(|ch| ch < ' ' || r#"<>:"/\|?*"#.contains(ch))
    {
        return true;
    }
    let base = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (base.len() == 4
            && (base.starts_with("COM") || base.starts_with("LPT"))
            && matches!(base.as_bytes()[3], b'1'..=b'9'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::file_operations::OperationKind;

    fn reversible_batch(id: &str) -> OperationBatchResult {
        OperationBatchResult {
            batch_id: id.to_string(),
            kind: OperationKind::Rename,
            entries: Vec::new(),
            succeeded: 1,
            skipped: 0,
            failed: 0,
            reversible: true,
        }
    }

    #[test]
    fn cancellation_is_scoped_to_one_session() {
        let state = DifferenceFinderState::default();
        state.cancel("session-a");

        assert!(state.is_cancelled("session-a"));
        assert!(!state.is_cancelled("session-b"));

        state.begin_session("session-a");
        assert!(!state.is_cancelled("session-a"));
    }

    #[test]
    fn only_latest_reversible_batch_can_be_consumed_once() {
        let state = DifferenceFinderState::default();
        state.store_batch(reversible_batch("older")).unwrap();
        state.store_batch(reversible_batch("latest")).unwrap();

        assert!(state.get_batch("older").is_err());
        assert_eq!(state.get_batch("latest").unwrap().batch_id, "latest");
        state.consume_batch("latest").unwrap();
        assert!(state.get_batch("latest").is_err());
    }

    #[test]
    fn rejects_invalid_windows_folder_names() {
        for name in ["CON", "folder.", "bad:name"] {
            let request = TransferFilesRequest {
                files: Vec::new(),
                target_directory: "C:\\target".to_string(),
                new_folder_name: Some(name.to_string()),
            };
            assert!(resolve_destination(&request).is_err(), "accepted {name}");
        }
    }
}
