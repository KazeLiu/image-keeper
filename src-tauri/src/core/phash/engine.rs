use super::PHashComputer;
use crate::core::algorithm_profile::algorithm_pool;
use crate::db::models::ScanProgressEvent;
use crate::db::repository::Repository;
use crate::error::Result;
use rayon::prelude::*;
use std::path::Path;

/// 感知哈希引擎
pub struct PHashEngine;

impl PHashEngine {
    /// 为扫描任务中的所有图片计算感知哈希
    pub fn compute_phashes<F>(
        scan_id: i64,
        repository: &Repository,
        progress_callback: F,
    ) -> Result<()>
    where
        F: Fn(ScanProgressEvent) + Send + Sync,
    {
        // 获取所有未计算感知哈希的图片
        let images = Self::get_images_without_phash(scan_id, repository)?;
        let total_files = images.len() as u64;

        let mut phashed_count = 0u64;

        let hashes = algorithm_pool().install(|| {
            images
                .par_iter()
                .map(|(_, file_path)| PHashComputer::compute_phash(Path::new(file_path)))
                .collect::<Vec<_>>()
        });
        for ((id, file_path), hash) in images.iter().zip(hashes) {
            let phash = hash?;

            // 更新数据库
            repository.conn().execute(
                "UPDATE images SET phash = ?1 WHERE id = ?2",
                rusqlite::params![phash, id],
            )?;

            // 更新进度
            phashed_count += 1;
            let phashed = phashed_count;

            // 发送进度事件
            progress_callback(ScanProgressEvent {
                run_id: "".to_string(), // TODO: 传入 run_id
                phase: "phash_computation".to_string(),
                total_files: total_files as i64,
                processed_files: phashed as i64,
                current_file: Some(file_path.clone()),
            });
        }

        // 更新扫描任务的感知哈希计算完成数
        repository.conn().execute(
            "UPDATE scans SET phash_computed = ?1 WHERE id = ?2",
            rusqlite::params![total_files, scan_id],
        )?;

        Ok(())
    }

    /// 获取未计算感知哈希的图片列表
    fn get_images_without_phash(
        scan_id: i64,
        repository: &Repository,
    ) -> Result<Vec<(i64, String)>> {
        let mut stmt = repository
            .conn()
            .prepare("SELECT id, file_path FROM images WHERE scan_id = ?1 AND phash IS NULL")?;

        let images = stmt
            .query_map(rusqlite::params![scan_id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(images)
    }

    /// 使用感知哈希快速筛选候选相似对
    /// 汉明距离阈值推荐 10
    pub fn filter_candidates_by_phash(
        scan_id: i64,
        repository: &Repository,
        hamming_threshold: u32,
    ) -> Result<Vec<(i64, i64, u32)>> {
        // 获取所有已计算感知哈希的图片
        let mut stmt = repository
            .conn()
            .prepare("SELECT id, phash FROM images WHERE scan_id = ?1 AND phash IS NOT NULL")?;

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
