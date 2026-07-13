pub mod recycle;
pub mod export;

use std::path::{Path, PathBuf};
use std::fs;
use chrono::Utc;
use crate::error::{Result, AppError};
use crate::db::repository::Repository;
use crate::db::models::{DeleteReason, RecycleBinEntry};

/// 删除管理器
pub struct DeleteManager;

impl DeleteManager {
    /// 获取回收站目录
    pub fn get_recycle_bin_path(root_path: &Path) -> PathBuf {
        root_path.join(".recycle")
    }

    /// 移动文件到回收站
    pub fn move_to_recycle_bin(
        file_path: &Path,
        root_path: &Path,
        delete_reason: DeleteReason,
        related_image_id: Option<i64>,
        duplicate_id: Option<i64>,
        similar_pair_id: Option<i64>,
        repository: &Repository,
    ) -> Result<RecycleBinEntry> {
        // 确保回收站目录存在
        let recycle_bin = Self::get_recycle_bin_path(root_path);
        fs::create_dir_all(&recycle_bin)?;

        // 生成回收站中的文件名 (保持相对路径结构)
        let relative_path = file_path
            .strip_prefix(root_path)
            .map_err(|_| AppError::InvalidPath)?;

        let recycled_path = recycle_bin.join(relative_path);

        // 确保目标目录存在
        if let Some(parent) = recycled_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // 移动文件
        fs::rename(file_path, &recycled_path)?;

        // 获取文件信息
        let metadata = fs::metadata(&recycled_path)?;
        let file_size = metadata.len() as i64;

        // 从数据库获取图片信息
        let image_info = if let Some(img_id) = related_image_id {
            repository.conn().query_row(
                "SELECT width, height, blake3_hash FROM images WHERE id = ?1",
                rusqlite::params![img_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            ).ok()
        } else {
            None
        };

        let (width, height, blake3_hash) = image_info.unwrap_or((0, 0, None));

        // 获取 SSIM 分数
        let ssim_score = if let Some(pair_id) = similar_pair_id {
            repository.conn().query_row(
                "SELECT ssim_score FROM similar_pairs WHERE id = ?1",
                rusqlite::params![pair_id],
                |row| row.get(0),
            ).ok().flatten()
        } else {
            None
        };

        let now = Utc::now().timestamp();

        // 插入回收站记录
        repository.conn().execute(
            "INSERT INTO recycle_bin
             (original_path, recycled_path, delete_reason, related_image_id,
              duplicate_id, similar_pair_id, file_size, width, height,
              blake3_hash, ssim_score, recycled_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                file_path.to_string_lossy().as_ref(),
                recycled_path.to_string_lossy().as_ref(),
                delete_reason.as_str(),
                related_image_id,
                duplicate_id,
                similar_pair_id,
                file_size,
                width,
                height,
                blake3_hash,
                ssim_score,
                now,
            ],
        )?;

        let id = repository.conn().last_insert_rowid();

        Ok(RecycleBinEntry {
            id,
            original_path: file_path.to_string_lossy().to_string(),
            recycled_path: recycled_path.to_string_lossy().to_string(),
            delete_reason,
            related_image_id,
            duplicate_id,
            similar_pair_id,
            file_size,
            width: width as u32,
            height: height as u32,
            blake3_hash,
            ssim_score,
            recycled_at: now,
        })
    }

    /// 从回收站恢复文件
    pub fn restore_from_recycle_bin(
        entry_id: i64,
        repository: &Repository,
    ) -> Result<()> {
        // 获取回收站记录
        let (original_path, recycled_path): (String, String) = repository.conn().query_row(
            "SELECT original_path, recycled_path FROM recycle_bin WHERE id = ?1",
            rusqlite::params![entry_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        let original = Path::new(&original_path);
        let recycled = Path::new(&recycled_path);

        // 确保原始目录存在
        if let Some(parent) = original.parent() {
            fs::create_dir_all(parent)?;
        }

        // 恢复文件
        fs::rename(recycled, original)?;

        // 删除回收站记录
        repository.conn().execute(
            "DELETE FROM recycle_bin WHERE id = ?1",
            rusqlite::params![entry_id],
        )?;

        Ok(())
    }

    /// 永久删除回收站中的文件
    pub fn permanent_delete(entry_ids: &[i64], repository: &Repository) -> Result<()> {
        for &entry_id in entry_ids {
            // 获取回收站文件路径
            let recycled_path: String = repository.conn().query_row(
                "SELECT recycled_path FROM recycle_bin WHERE id = ?1",
                rusqlite::params![entry_id],
                |row| row.get(0),
            )?;

            // 删除文件
            let path = Path::new(&recycled_path);
            if path.exists() {
                fs::remove_file(path)?;
            }

            // 删除数据库记录
            repository.conn().execute(
                "DELETE FROM recycle_bin WHERE id = ?1",
                rusqlite::params![entry_id],
            )?;
        }

        Ok(())
    }
}
