pub mod index;
pub mod matcher;

use crate::error::Result;
use crate::db::repository::Repository;
use crate::db::models::Image;
use self::index::SizeIndex;

/// 匹配引擎
pub struct MatchingEngine;

impl MatchingEngine {
    /// 为扫描任务构建小图-大图候选配对
    pub fn build_candidate_pairs(scan_id: i64, repository: &Repository) -> Result<Vec<(Image, Image)>> {
        // 获取所有已计算哈希且哈希不同的图片
        let mut stmt = repository.conn().prepare(
            "SELECT id, file_path, relative_path, file_size, file_modified_at,
                    width, height, format, aspect_ratio, blake3_hash, phash,
                    hash_computed_at, scan_id, folder_id, scanned_at
             FROM images
             WHERE scan_id = ?1 AND blake3_hash IS NOT NULL
             ORDER BY width ASC, height ASC",
        )?;

        let images: Vec<Image> = stmt
            .query_map(rusqlite::params![scan_id], |row| {
                Ok(Image {
                    id: row.get(0)?,
                    file_path: row.get(1)?,
                    relative_path: row.get(2)?,
                    file_size: row.get(3)?,
                    file_modified_at: row.get(4)?,
                    width: row.get(5)?,
                    height: row.get(6)?,
                    format: row.get(7)?,
                    aspect_ratio: row.get(8)?,
                    blake3_hash: row.get(9)?,
                    phash: row.get(10)?,
                    hash_computed_at: row.get(11)?,
                    scan_id: row.get(12)?,
                    folder_id: row.get(13)?,
                    scanned_at: row.get(14)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut candidate_pairs = Vec::new();
        let aspect_ratio_tolerance = 0.01; // 1% 容差

        // 对于每张小图，查找可能的大图
        for (i, small_img) in images.iter().enumerate() {
            // 只查找比当前图片更大的图片
            for large_img in &images[i + 1..] {
                // 检查哈希是否不同（完全相同的已经被识别为重复文件）
                if small_img.blake3_hash == large_img.blake3_hash {
                    continue;
                }

                // 检查是否满足配对条件
                if SizeIndex::has_same_aspect_ratio(small_img, large_img, aspect_ratio_tolerance)
                    && SizeIndex::is_strictly_smaller(small_img, large_img)
                {
                    candidate_pairs.push((small_img.clone(), large_img.clone()));
                }
            }
        }

        Ok(candidate_pairs)
    }

    /// 保存候选配对到数据库
    pub fn save_candidate_pairs(
        pairs: &[(Image, Image)],
        repository: &Repository,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp();

        for (smaller, larger) in pairs {
            let size_ratio = smaller.file_size as f64 / larger.file_size as f64;
            let resolution_ratio = (smaller.width * smaller.height) as f64
                / (larger.width * larger.height) as f64;

            repository.conn().execute(
                "INSERT OR IGNORE INTO similar_pairs
                 (larger_image_id, smaller_image_id, size_ratio, resolution_ratio, status, marked_at)
                 VALUES (?1, ?2, ?3, ?4, 'pending', ?5)",
                rusqlite::params![larger.id, smaller.id, size_ratio, resolution_ratio, now],
            )?;
        }

        Ok(())
    }
}
