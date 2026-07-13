pub mod blake3;

use std::path::Path;
use crate::error::Result;
use crate::db::repository::Repository;
use crate::db::models::HashProgressEvent;

/// 哈希引擎
pub struct HashEngine;

impl HashEngine {
    /// 为扫描任务中的所有图片计算哈希
    pub fn compute_hashes<F>(
        scan_id: i64,
        repository: &Repository,
        progress_callback: F,
    ) -> Result<()>
    where
        F: Fn(HashProgressEvent) + Send + Sync,
    {
        // 获取所有未计算哈希的图片
        let images = Self::get_images_without_hash(scan_id, repository)?;
        let total_files = images.len() as u64;

        let mut hashed_count = 0u64;

        // 顺序计算哈希（rusqlite Connection 不支持跨线程共享）
        for (id, file_path) in &images {
            // 计算 BLAKE3 哈希
            let hash = blake3::Blake3Computer::compute_file_hash(Path::new(file_path))?;

            // 更新数据库
            repository.update_image_hash(*id, &hash)?;

            // 更新进度
            hashed_count += 1;
            let hashed = hashed_count;

            // 发送进度事件
            progress_callback(HashProgressEvent {
                scan_id,
                total_files,
                hashed_files: hashed,
                current_file: file_path.clone(),
            });

        }

        // 更新扫描任务的哈希计算完成数
        repository.conn().execute(
            "UPDATE scans SET hash_computed = ?1 WHERE id = ?2",
            rusqlite::params![total_files, scan_id],
        )?;

        Ok(())
    }

    /// 获取未计算哈希的图片列表
    fn get_images_without_hash(
        scan_id: i64,
        repository: &Repository,
    ) -> Result<Vec<(i64, String)>> {
        let mut stmt = repository.conn().prepare(
            "SELECT id, file_path FROM images WHERE scan_id = ?1 AND blake3_hash IS NULL",
        )?;

        let images = stmt
            .query_map(rusqlite::params![scan_id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(images)
    }

    /// 识别完全重复的文件
    pub fn identify_duplicates(scan_id: i64, repository: &Repository) -> Result<()> {
        // 查找所有有相同哈希的图片组
        let mut stmt = repository.conn().prepare(
            "SELECT blake3_hash, GROUP_CONCAT(id) as ids
             FROM images
             WHERE scan_id = ?1 AND blake3_hash IS NOT NULL
             GROUP BY blake3_hash
             HAVING COUNT(*) > 1",
        )?;

        let duplicate_groups: Vec<(String, String)> = stmt
            .query_map(rusqlite::params![scan_id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let now = chrono::Utc::now().timestamp();

        // 为每组重复文件创建记录
        for (hash, ids_str) in duplicate_groups {
            let ids: Vec<i64> = ids_str
                .split(',')
                .filter_map(|s| s.parse().ok())
                .collect();

            if ids.len() < 2 {
                continue;
            }

            // 第一个作为原始文件，其他作为重复文件
            let original_id = ids[0];
            for &duplicate_id in &ids[1..] {
                repository.conn().execute(
                    "INSERT OR IGNORE INTO duplicates (hash_group, original_image_id, duplicate_image_id, status, marked_at)
                     VALUES (?1, ?2, ?3, 'pending', ?4)",
                    rusqlite::params![hash, original_id, duplicate_id, now],
                )?;
            }
        }

        Ok(())
    }
}
