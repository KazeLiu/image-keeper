use crate::core::difference_finder::{
    search_difference_images, DifferenceSearchRequest, DifferenceSearchResponse,
};
use crate::core::file_operations::{
    execute_copy, execute_move, execute_rename, preview_rename, undo_operation_batch,
    OperationBatchResult, RenameExecutionItem, RenameInput, RenamePreviewItem, RenameRule,
};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::{Component, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, State, Window};

#[derive(Default)]
pub struct DifferenceFinderState {
    cancelled_sessions: Mutex<HashSet<String>>,
    batches: Mutex<HashMap<String, OperationBatchResult>>,
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
        self.batches
            .lock()
            .map_err(|_| "文件操作状态锁已损坏".to_string())?
            .insert(batch.batch_id.clone(), batch);
        Ok(())
    }

    fn get_batch(&self, batch_id: &str) -> Result<OperationBatchResult, String> {
        self.batches
            .lock()
            .map_err(|_| "文件操作状态锁已损坏".to_string())?
            .get(batch_id)
            .cloned()
            .ok_or_else(|| "找不到可撤销的操作批次".to_string())
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
    pub paths: Vec<String>,
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
pub fn execute_difference_rename(
    request: RenameExecuteRequest,
    state: State<'_, Arc<DifferenceFinderState>>,
) -> Result<OperationBatchResult, String> {
    let batch = execute_rename(&request.items).map_err(|error| error.to_string())?;
    state.store_batch(batch.clone())?;
    Ok(batch)
}

#[tauri::command]
pub fn move_difference_files(
    request: TransferFilesRequest,
    state: State<'_, Arc<DifferenceFinderState>>,
) -> Result<OperationBatchResult, String> {
    let destination = resolve_destination(&request)?;
    let batch = execute_move(&request.paths, &destination).map_err(|error| error.to_string())?;
    state.store_batch(batch.clone())?;
    Ok(batch)
}

#[tauri::command]
pub fn copy_difference_files(
    request: TransferFilesRequest,
    state: State<'_, Arc<DifferenceFinderState>>,
) -> Result<OperationBatchResult, String> {
    let destination = resolve_destination(&request)?;
    let batch = execute_copy(&request.paths, &destination).map_err(|error| error.to_string())?;
    state.store_batch(batch.clone())?;
    Ok(batch)
}

#[tauri::command]
pub fn undo_difference_batch(
    batch_id: String,
    state: State<'_, Arc<DifferenceFinderState>>,
) -> Result<OperationBatchResult, String> {
    let original = state.get_batch(&batch_id)?;
    undo_operation_batch(&original).map_err(|error| error.to_string())
}

fn resolve_destination(request: &TransferFilesRequest) -> Result<PathBuf, String> {
    let mut destination = PathBuf::from(&request.target_directory);
    if let Some(name) = request
        .new_folder_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_is_scoped_to_one_session() {
        let state = DifferenceFinderState::default();
        state.cancel("session-a");

        assert!(state.is_cancelled("session-a"));
        assert!(!state.is_cancelled("session-b"));

        state.begin_session("session-a");
        assert!(!state.is_cancelled("session-a"));
    }
}
