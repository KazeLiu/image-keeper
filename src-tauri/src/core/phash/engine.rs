use std::path::Path;
use crate::error::Result;
use crate::db::repository::Repository;
use crate::db::models::PHashProgressEvent;
use super::PHashComputer;

/// pHash 引擎
pub struct PHashEngine;

impl PHashEngine {
    /// 为扫描任务中的所有图片计算 pHash
    pub fn compute_phashes<F>(
        scan_id: i64,
        repository: &Repository,
        progress_callback: F,
    ) -> Result<()>
    where
        F: Fn(PHashProgressEvent) + Send + Sync,
    {
        // 获取所有未计算 pHash 的图片
        let images = Self::get_images_without_phash(scan_id, repository)?;
        let total_files = images.len() as u64;

        let mut phashed_count = 0u64;

        // 顺序计算 pHash
        for (id, file_path) in &images {
            // 计算 pHash
            let phash = PHashComputer::compute_phash(Path::new(file_path))?;

            // 更新数据库
            repository.conn().execute(
                "UPDATE images SET phash = ?1 WHERE id = ?2",
                rusqlite::params![phash, id],
            )?;

            // 更新进度
            phashed_count += 1;
            let phashed = phashed_count;

            // 发送进度事件
            progress_callback(PHashProgressEvent {
                scan_id,
                total_files,
                phashed_files: phashed,
                current_file: file_path.clone(),
            });
        }

        // 更新扫描任务的 pHash 计算完成数
        repository.conn().execute(
            "UPDATE scans SET phash_computed = ?1 WHERE id = ?2",
            rusqlite::params![total_files, scan_id],
        )?;

        Ok(())
    }

    /// 获取未计算 pHash 的图片列表
    fn get_images_without_phash(
        scan_id: i64,
        repository: &Repository,
    ) -> Result<Vec<(i64, String)>> {
        let mut stmt = repository.conn().prepare(
            "SELECT id, file_path FROM images WHERE scan_id = ?1 AND phash IS NULL",
        )?;

        let images = stmt
            .query_map(rusqlite::params![scan_id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(images)
    }

    /// 使用 pHash 快速筛选候选相似对
    /// 汉明距离阈值推荐 10
    pub fn filter_candidates_by_phash(
        scan_id: i64,
        repository: &Repository,
        hamming_threshold: u32,
    ) -> Result<Vec<(i64, i64, u32)>> {
        // 获取所有已计算 pHash 的图片
        let mut stmt = repository.conn().prepare(
            "SELECT id, phash FROM images WHERE scan_id = ?1 AND phash IS NOT NULL",
        )?;

        let images: Vec<(i64, String)> = stmt
            .query_map(rusqlite::params![scan_id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut candidates = Vec::new();

        // 计算所有图片对的汉明距离
        for i in 0..images.len() {
            for j in (i + 1)..images.len() {
                let (id1, hash1) = &images[i];
                let (id2, hash2) = &images[j];

                let distance = PHashComputer::hamming_distance(hash1, hash2)?;

                if distance < hamming_threshold {
                    candidates.push((*id1, *id2, distance));
                }
            }
        }

        Ok(candidates)
    }
}
