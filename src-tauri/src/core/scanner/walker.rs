use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use crate::error::Result;
use super::metadata::{is_supported_image, MetadataExtractor};
use crate::db::repository::Repository;
use crate::db::models::{ScanStatus, ScanProgressEvent};

/// 目录遍历器
pub struct DirectoryWalker;

impl DirectoryWalker {
    /// 遍历目录并收集所有支持的图片文件
    pub fn collect_image_files(root_path: &Path) -> Result<Vec<PathBuf>> {
        let mut image_files = Vec::new();

        for entry in WalkDir::new(root_path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() && is_supported_image(path) {
                image_files.push(path.to_path_buf());
            }
        }

        Ok(image_files)
    }

    /// 扫描目录并提取所有图片元数据
    pub fn scan_directory<F>(
        root_path: &Path,
        scan_id: i64,
        repository: &Repository,
        progress_callback: F,
    ) -> Result<()>
    where
        F: Fn(ScanProgressEvent) + Send + Sync,
    {
        // 收集所有图片文件
        let image_files = Self::collect_image_files(root_path)?;
        let total_files = image_files.len() as u64;

        // 更新扫描任务总文件数
        repository.conn().execute(
            "UPDATE scans SET total_files = ?1 WHERE id = ?2",
            rusqlite::params![total_files, scan_id],
        )?;

        // 更新状态为运行中
        repository.update_scan_status(scan_id, ScanStatus::Running)?;

        let start_time = std::time::Instant::now();
        let mut scanned_count = 0u64;

        // 顺序处理图片文件（rusqlite Connection 不支持跨线程共享）
        for file_path in &image_files {
            // 提取元数据
            let mut image = MetadataExtractor::extract(file_path, root_path, scan_id)?;

            // 插入数据库
            let image_id = repository.insert_image(&image)?;
            image.id = image_id;

            // 更新进度
            scanned_count += 1;
            let scanned = scanned_count;

            // 计算预计剩余时间
            let elapsed = start_time.elapsed().as_secs();
            let estimated_time_remaining = if scanned > 0 {
                let avg_time_per_file = elapsed / scanned;
                Some(avg_time_per_file * (total_files - scanned))
            } else {
                None
            };

            // 发送进度事件
            progress_callback(ScanProgressEvent {
                scan_id,
                total_files,
                scanned_files: scanned,
                current_file: file_path.to_string_lossy().to_string(),
                estimated_time_remaining,
            });

            // 定期更新数据库进度
            if scanned % 100 == 0 {
                repository.update_scan_progress(
                    scan_id,
                    scanned,
                    Some(&file_path.to_string_lossy()),
                )?;
            }

        }

        // 更新最终进度
        repository.update_scan_progress(scan_id, total_files, None)?;
        repository.update_scan_status(scan_id, ScanStatus::Completed)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_image_files() {
        // 测试需要实际的测试图片目录
        // 这里仅作为示例
    }
}
