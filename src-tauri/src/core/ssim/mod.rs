pub mod compute;
pub mod resize;
pub mod standard;

use crate::core::algorithm_profile::algorithm_pool;
use crate::db::repository::Repository;
use crate::error::Result;
use rayon::prelude::*;
use std::path::Path;

/// 结构相似性引擎
pub struct SsimEngine;

impl SsimEngine {
    /// 计算两张图片的结构相似性
    pub fn compute_ssim(path1: &Path, path2: &Path) -> Result<f64> {
        compute::SsimComputer::compute_from_files(path1, path2)
    }

    /// 为候选配对计算结构相似性
    pub fn compute_ssim_for_pairs(
        scan_id: i64,
        repository: &Repository,
        ssim_threshold: f64,
    ) -> Result<()> {
        // 获取所有待计算结构相似性的配对
        let pairs = Self::get_pending_pairs(scan_id, repository)?;

        // 算法在线程池中并行执行，数据库连接仍只在当前线程串行使用。
        let scores = algorithm_pool().install(|| {
            pairs
                .par_iter()
                .map(|(_, left_path, right_path)| {
                    compute::SsimComputer::compute_from_files(
                        Path::new(left_path),
                        Path::new(right_path),
                    )
                })
                .collect::<Vec<_>>()
        });
        for ((pair_id, _, _), score) in pairs.iter().zip(scores) {
            let ssim_score = score?;

            // 判断是否为压缩版本
            let is_compressed = ssim_score >= ssim_threshold;

            // 更新数据库
            let now = chrono::Utc::now().timestamp();
            repository.conn().execute(
                "UPDATE similar_pairs
                 SET ssim_score = ?1, is_compressed_version = ?2, ssim_threshold = ?3, computed_at = ?4
                 WHERE id = ?5",
                rusqlite::params![ssim_score, if is_compressed { 1 } else { 0 }, ssim_threshold, now, pair_id],
            )?;
        }

        Ok(())
    }

    /// 获取待计算结构相似性的配对
    fn get_pending_pairs(
        scan_id: i64,
        repository: &Repository,
    ) -> Result<Vec<(i64, String, String)>> {
        let mut stmt = repository.conn().prepare(
            "SELECT sp.id, i1.file_path as larger_path, i2.file_path as smaller_path
             FROM similar_pairs sp
             JOIN images i1 ON sp.larger_image_id = i1.id
             JOIN images i2 ON sp.smaller_image_id = i2.id
             WHERE i1.scan_id = ?1 AND sp.ssim_score IS NULL",
        )?;

        let pairs = stmt
            .query_map(rusqlite::params![scan_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(pairs)
    }
}
