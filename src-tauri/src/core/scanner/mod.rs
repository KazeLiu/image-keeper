pub mod metadata;
pub mod walker;

use std::path::Path;
use crate::error::Result;
use crate::db::repository::Repository;
use crate::db::models::ScanProgressEvent;

/// 扫描引擎
pub struct ScanEngine;

impl ScanEngine {
    /// 开始扫描目录
    pub fn start_scan<F>(
        root_path: &Path,
        repository: &Repository,
        progress_callback: F,
    ) -> Result<i64>
    where
        F: Fn(ScanProgressEvent) + Send + Sync,
    {
        // 创建扫描任务
        let scan = repository.create_scan(&root_path.to_string_lossy())?;
        let scan_id = scan.id;

        // 开始扫描
        walker::DirectoryWalker::scan_directory(
            root_path,
            scan_id,
            repository,
            progress_callback,
        )?;

        Ok(scan_id)
    }
}
