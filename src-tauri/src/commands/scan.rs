use tauri::{Emitter, State, Window};
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use crate::error::Result;
use crate::db::repository::Repository;
use crate::db::models::{Scan, ScanProgressEvent};
use crate::core::scanner::ScanEngine;
use crate::core::hash::HashEngine;
use crate::core::matching::MatchingEngine;
use crate::core::ssim::SsimEngine;

/// 开始扫描目录
#[tauri::command]
pub async fn start_scan(
    root_path: String,
    window: Window,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<Scan> {
    let root = PathBuf::from(&root_path);
    let repo_clone = repo.inner().clone();

    // 在后台线程执行扫描
    let result = tokio::task::spawn_blocking(move || {
        let repository = repo_clone.lock().unwrap();

        // 进度回调
        let progress_callback = |event: ScanProgressEvent| {
            window.emit("scan_progress", &event).ok();
        };

        // 开始扫描
        let scan_id = ScanEngine::start_scan(&root, &repository, progress_callback)?;

        // 计算哈希
        let hash_callback = |event| {
            window.emit("hash_progress", &event).ok();
        };
        HashEngine::compute_hashes(scan_id, &repository, hash_callback)?;

        // 识别完全重复文件
        HashEngine::identify_duplicates(scan_id, &repository)?;

        // 构建候选配对
        let pairs = MatchingEngine::build_candidate_pairs(scan_id, &repository)?;
        MatchingEngine::save_candidate_pairs(&pairs, &repository)?;

        // 计算 SSIM
        let settings = repository.load_settings()?;
        SsimEngine::compute_ssim_for_pairs(scan_id, &repository, settings.ssim_threshold)?;

        repository.get_scan(scan_id)
    }).await;

    result.map_err(|e| crate::error::AppError::Other(e.to_string()))?
}

/// 暂停扫描
#[tauri::command]
pub async fn pause_scan(
    scan_id: i64,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<()> {
    let repo = repo.lock().unwrap();
    repo.update_scan_status(scan_id, crate::db::models::ScanStatus::Paused)
}

/// 恢复扫描
#[tauri::command]
pub async fn resume_scan(
    scan_id: i64,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<()> {
    let repo = repo.lock().unwrap();
    repo.update_scan_status(scan_id, crate::db::models::ScanStatus::Running)
}

/// 取消扫描
#[tauri::command]
pub async fn cancel_scan(
    scan_id: i64,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<()> {
    let repo = repo.lock().unwrap();
    repo.update_scan_status(scan_id, crate::db::models::ScanStatus::Cancelled)
}
