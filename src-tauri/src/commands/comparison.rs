use tauri::{Emitter, State, Window};
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use crate::error::Result;
use crate::db::repository::Repository;
use crate::db::models::{Scan, CompareMode, FolderRole, Folder, ScanProgressEvent, HashProgressEvent, PHashProgressEvent, MatchProgressEvent};
use crate::core::scanner::walker::DirectoryWalker;
use crate::core::hash::HashEngine;
use crate::core::phash::engine::PHashEngine;
use crate::core::comparison::ComparisonEngine;
use crate::core::scanner::metadata::MetadataExtractor;

/// 多文件夹对比请求
#[derive(Debug, serde::Deserialize)]
pub struct MultiCompareRequest {
    pub folders: Vec<FolderConfig>,
    pub compare_mode: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct FolderConfig {
    pub path: String,
    pub role: String,
}

/// 开始多文件夹对比扫描
#[tauri::command]
pub async fn start_multi_compare(
    request: MultiCompareRequest,
    window: Window,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<Scan> {
    let repo_clone = repo.inner().clone();

    let result = tokio::task::spawn_blocking(move || {
        let repository = repo_clone.lock().unwrap();

        // 解析对比模式
        let compare_mode = match request.compare_mode.as_str() {
            "within" => CompareMode::Within,
            "between" => CompareMode::Between,
            _ => return Err(crate::error::AppError::ValidationError(
                "无效的对比模式".to_string()
            )),
        };

        // 创建扫描任务
        let now = chrono::Utc::now().timestamp();
        let root_path = request.folders.first()
            .map(|f| f.path.clone())
            .unwrap_or_default();

        repository.conn().execute(
            "INSERT INTO scans (root_path, status, compare_mode, created_at)
             VALUES (?1, 'pending', ?2, ?3)",
            rusqlite::params![root_path, compare_mode.as_str(), now],
        )?;

        let scan_id = repository.conn().last_insert_rowid();

        // 创建文件夹记录
        for folder_config in &request.folders {
            let role = match folder_config.role.as_str() {
                "baseline" => FolderRole::Baseline,
                "compare" => FolderRole::Compare,
                _ => return Err(crate::error::AppError::ValidationError(
                    "无效的文件夹角色".to_string()
                )),
            };

            repository.conn().execute(
                "INSERT INTO folders (scan_id, path, role, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![scan_id, folder_config.path, role.as_str(), now],
            )?;
        }

        // Phase 1: 扫描所有文件夹
        let progress_callback = |event: ScanProgressEvent| {
            window.emit("scan_progress", &event).ok();
        };

        scan_all_folders(scan_id, &request.folders, &repository, progress_callback)?;

        // Phase 2: 计算哈希
        let hash_callback = |event: HashProgressEvent| {
            window.emit("hash_progress", &event).ok();
        };
        HashEngine::compute_hashes(scan_id, &repository, hash_callback)?;

        // Phase 2: 识别完全重复
        ComparisonEngine::identify_exact_duplicates(scan_id, &repository, compare_mode.clone())?;

        // Phase 3: 计算 pHash
        let phash_callback = |event: PHashProgressEvent| {
            window.emit("phash_progress", &event).ok();
        };
        PHashEngine::compute_phashes(scan_id, &repository, phash_callback)?;

        // Phase 3: pHash 快速筛选
        let match_callback = |event: MatchProgressEvent| {
            window.emit("match_progress", &event).ok();
        };

        let hamming_threshold = 10;
        let candidates = ComparisonEngine::build_phash_candidates(
            scan_id,
            &repository,
            compare_mode.clone(),
            hamming_threshold,
        )?;

        // 保存候选配对
        let now = chrono::Utc::now().timestamp();
        for (id1, id2, distance) in &candidates {
            // 获取图片信息计算大小比例
            let (size1, size2, w1, h1, w2, h2): (i64, i64, u32, u32, u32, u32) = repository.conn().query_row(
                "SELECT
                    (SELECT file_size FROM images WHERE id = ?1),
                    (SELECT file_size FROM images WHERE id = ?2),
                    (SELECT width FROM images WHERE id = ?1),
                    (SELECT height FROM images WHERE id = ?1),
                    (SELECT width FROM images WHERE id = ?2),
                    (SELECT height FROM images WHERE id = ?2)",
                rusqlite::params![id1, id2],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
            )?;

            let size_ratio = size2 as f64 / size1 as f64;
            let resolution_ratio = (w2 * h2) as f64 / (w1 * h1) as f64;

            repository.conn().execute(
                "INSERT OR IGNORE INTO similar_pairs
                 (larger_image_id, smaller_image_id, phash_distance, size_ratio, resolution_ratio, status, marked_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6)",
                rusqlite::params![id1, id2, distance, size_ratio, resolution_ratio, now],
            )?;
        }

        match_callback(MatchProgressEvent {
            scan_id,
            total_pairs: candidates.len() as u64,
            processed_pairs: candidates.len() as u64,
            current_phase: "pHash筛选完成".to_string(),
        });

        // Phase 4: 计算 SSIM 并分类
        let settings = repository.load_settings()?;
        ComparisonEngine::compute_ssim_and_classify(scan_id, &repository, settings.ssim_threshold)?;

        // 更新状态为完成
        repository.update_scan_status(scan_id, crate::db::models::ScanStatus::Completed)?;

        repository.get_scan(scan_id)
    }).await;

    result.map_err(|e| crate::error::AppError::Other(e.to_string()))?
}

/// 扫描所有文件夹
fn scan_all_folders<F>(
    scan_id: i64,
    folders: &[FolderConfig],
    repository: &Repository,
    progress_callback: F,
) -> Result<()>
where
    F: Fn(ScanProgressEvent) + Send + Sync,
{
    let mut total_scanned = 0u64;

    for folder_config in folders {
        let folder_path = PathBuf::from(&folder_config.path);

        // 获取 folder_id
        let folder_id: i64 = repository.conn().query_row(
            "SELECT id FROM folders WHERE scan_id = ?1 AND path = ?2",
            rusqlite::params![scan_id, folder_config.path],
            |row| row.get(0),
        )?;

        // 收集图片文件
        let image_files = DirectoryWalker::collect_image_files(&folder_path)?;
        let folder_file_count = image_files.len() as u64;

        // 更新文件夹文件数
        repository.conn().execute(
            "UPDATE folders SET file_count = ?1 WHERE id = ?2",
            rusqlite::params![folder_file_count, folder_id],
        )?;

        // 扫描并提取元数据
        for file_path in &image_files {
            let mut image = MetadataExtractor::extract(file_path, &folder_path, scan_id)?;
            image.folder_id = Some(folder_id);

            let image_id = repository.insert_image(&image)?;
            total_scanned += 1;

            progress_callback(ScanProgressEvent {
                scan_id,
                total_files: folder_file_count,
                scanned_files: total_scanned,
                current_file: file_path.to_string_lossy().to_string(),
                estimated_time_remaining: None,
            });
        }
    }

    // 更新扫描任务总文件数
    repository.conn().execute(
        "UPDATE scans SET total_files = ?1, scanned_files = ?1, status = 'running' WHERE id = ?2",
        rusqlite::params![total_scanned, scan_id],
    )?;

    Ok(())
}

/// 获取对比结果统计
#[tauri::command]
pub async fn get_comparison_stats(
    scan_id: i64,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<ComparisonStats> {
    let repo = repo.lock().unwrap();

    // 完全重复数量
    let exact_duplicates: i64 = repo.conn().query_row(
        "SELECT COUNT(*) FROM duplicates WHERE original_image_id IN (SELECT id FROM images WHERE scan_id = ?1)",
        rusqlite::params![scan_id],
        |row| row.get(0),
    )?;

    // 压缩版本数量
    let compressed: i64 = repo.conn().query_row(
        "SELECT COUNT(*) FROM similar_pairs WHERE larger_image_id IN (SELECT id FROM images WHERE scan_id = ?1)
         AND similarity_type = 'compressed'",
        rusqlite::params![scan_id],
        |row| row.get(0),
    )?;

    // 差分图数量
    let diff: i64 = repo.conn().query_row(
        "SELECT COUNT(*) FROM similar_pairs WHERE larger_image_id IN (SELECT id FROM images WHERE scan_id = ?1)
         AND similarity_type = 'diff'",
        rusqlite::params![scan_id],
        |row| row.get(0),
    )?;

    // 总图片数
    let total_images: i64 = repo.conn().query_row(
        "SELECT COUNT(*) FROM images WHERE scan_id = ?1",
        rusqlite::params![scan_id],
        |row| row.get(0),
    )?;

    Ok(ComparisonStats {
        total_images: total_images as u64,
        exact_duplicates: exact_duplicates as u64,
        compressed_versions: compressed as u64,
        diff_images: diff as u64,
        unique_images: (total_images - exact_duplicates - compressed - diff) as u64,
    })
}

#[derive(Debug, serde::Serialize)]
pub struct ComparisonStats {
    pub total_images: u64,
    pub exact_duplicates: u64,
    pub compressed_versions: u64,
    pub diff_images: u64,
    pub unique_images: u64,
}
