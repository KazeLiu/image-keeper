use crate::core::image_features::extract_image_features;
use crate::core::phash::PHASH_ALGORITHM_VERSION;
use crate::db::models::FolderRole;
use crate::db::repository::Repository;
use crate::error::{AppError, Result};
use std::path::Path;
use walkdir::WalkDir;

/// 进度回调函数类型
pub type ProgressCallback = Box<dyn Fn(usize, usize, &str) + Send>;

/// 扫描引擎 - Phase 1: 扫描与特征提取
pub struct ScanEngine {
    exclude_patterns: Vec<String>,
    supported_formats: Vec<String>,
}

impl ScanEngine {
    pub fn new(exclude_patterns: Vec<String>, supported_formats: Vec<String>) -> Self {
        Self {
            exclude_patterns,
            supported_formats,
        }
    }

    /// 扫描目录并提取特征(带进度回调)
    pub fn scan_directory<F>(
        &self,
        repository: &Repository,
        run_id: &str,
        folder_id: i64,
        root_path: &Path,
        role: FolderRole,
        progress_callback: F,
    ) -> Result<Vec<i64>>
    where
        F: Fn(usize, usize) + Send,
    {
        // 方案 A: 先统计文件数量
        let total_files = self.count_files(root_path)?;
        let mut processed_files = 0;
        let mut image_ids = Vec::new();

        for entry in WalkDir::new(root_path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| self.should_include(e.path()))
        {
            let entry = entry.map_err(|e| AppError::FileSystem(format!("遍历目录失败: {}", e)))?;

            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();
            if !self.is_supported_format(path) {
                continue;
            }

            // 扫描单个文件
            match self.scan_file(repository, run_id, folder_id, root_path, path, role.clone()) {
                Ok(image_id) => {
                    image_ids.push(image_id);
                    processed_files += 1;
                    progress_callback(processed_files, total_files);
                }
                Err(e) => {
                    eprintln!("扫描文件失败 {:?}: {}", path, e);
                    // 单文件失败不影响整体进度
                    processed_files += 1;
                    progress_callback(processed_files, total_files);
                }
            }
        }

        Ok(image_ids)
    }

    /// 统计目录下的图片文件数量
    fn count_files(&self, root_path: &Path) -> Result<usize> {
        let mut count = 0;

        for entry in WalkDir::new(root_path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| self.should_include(e.path()))
        {
            let entry = entry.map_err(|e| AppError::FileSystem(format!("遍历目录失败: {}", e)))?;

            if entry.file_type().is_file() && self.is_supported_format(entry.path()) {
                count += 1;
            }
        }

        Ok(count)
    }

    /// 扫描单个文件
    fn scan_file(
        &self,
        repository: &Repository,
        run_id: &str,
        folder_id: i64,
        root_path: &Path,
        file_path: &Path,
        role: FolderRole,
    ) -> Result<i64> {
        let relative_path = file_path
            .strip_prefix(root_path)
            .map_err(|_| AppError::InvalidPath)?
            .to_string_lossy()
            .to_string();

        let features = extract_image_features(file_path)?;
        let aspect_ratio = features.width as f64 / features.height as f64;

        // 创建图片记录
        let image_id = {
            use crate::db::models::ScanStatus;

            // 先插入基本信息
            let _image_id = repository
                .conn()
                .execute(
                    r#"INSERT INTO images (
                    run_id, folder_id, source_role, file_path, relative_path,
                    file_size, file_modified_at, width, height, format,
                    aspect_ratio, frame_count, frame_strategy, scan_status, scanned_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"#,
                    rusqlite::params![
                        run_id,
                        folder_id,
                        role.as_str(),
                        file_path.to_string_lossy().as_ref(),
                        relative_path,
                        features.file_size as i64,
                        features.modified_at,
                        features.width,
                        features.height,
                        features.color_type,
                        aspect_ratio,
                        1,       // frame_count
                        "first", // frame_strategy
                        ScanStatus::Decoded.as_str(),
                        chrono::Utc::now().timestamp(),
                    ],
                )
                .map_err(|e| AppError::Database(e))?;

            let image_id = repository.conn().last_insert_rowid();

            // 更新哈希
            repository.update_image_hash(
                image_id,
                &features.blake3_hash,
                &features.phash,
                PHASH_ALGORITHM_VERSION,
            )?;

            image_id
        };

        Ok(image_id)
    }

    /// 检查路径是否应该包含
    fn should_include(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        for pattern in &self.exclude_patterns {
            if path_str.contains(pattern) {
                return false;
            }
        }

        true
    }

    /// 检查是否支持的格式
    fn is_supported_format(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            let ext_lower = ext.to_string_lossy().to_lowercase();
            self.supported_formats.contains(&ext_lower)
        } else {
            false
        }
    }
}
