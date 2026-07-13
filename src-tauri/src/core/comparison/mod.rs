use crate::error::Result;
use crate::db::repository::Repository;
use crate::db::models::{CompareMode, SimilarityType};

/// 对比引擎 - 实现 IMAGE_COMPARISON_WORKFLOW.md 的完整流程
pub struct ComparisonEngine;

impl ComparisonEngine {
    /// Phase 2: 精确匹配（哈希去重）
    /// 在 between 模式下，只查找 compare 文件夹中与 baseline 文件夹重复的图片
    pub fn identify_exact_duplicates(
        scan_id: i64,
        repository: &Repository,
        compare_mode: CompareMode,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp();

        match compare_mode {
            CompareMode::Within => {
                // 文件夹内比对：查找所有哈希相同的图片组
                Self::identify_duplicates_within(scan_id, repository, now)?;
            }
            CompareMode::Between => {
                // 文件夹间比对：只查找 compare 文件夹中与 baseline 重复的图片
                Self::identify_duplicates_between(scan_id, repository, now)?;
            }
        }

        Ok(())
    }

    /// 文件夹内比对模式
    fn identify_duplicates_within(
        scan_id: i64,
        repository: &Repository,
        now: i64,
    ) -> Result<()> {
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

        for (hash, ids_str) in duplicate_groups {
            let ids: Vec<i64> = ids_str
                .split(',')
                .filter_map(|s| s.parse().ok())
                .collect();

            if ids.len() < 2 {
                continue;
            }

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

    /// 文件夹间比对模式
    fn identify_duplicates_between(
        scan_id: i64,
        repository: &Repository,
        now: i64,
    ) -> Result<()> {
        // 获取 baseline 文件夹 ID
        let baseline_folders: Vec<i64> = repository.conn()
            .prepare("SELECT id FROM folders WHERE scan_id = ?1 AND role = 'baseline'")?
            .query_map(rusqlite::params![scan_id], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // 获取 compare 文件夹 ID
        let compare_folders: Vec<i64> = repository.conn()
            .prepare("SELECT id FROM folders WHERE scan_id = ?1 AND role = 'compare'")?
            .query_map(rusqlite::params![scan_id], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        if baseline_folders.is_empty() {
            return Ok(());
        }

        // 对于 compare 文件夹中的每张图片，查找 baseline 中的重复
        for compare_folder_id in compare_folders {
            let mut stmt = repository.conn().prepare(
                "SELECT c.id, c.blake3_hash, b.id
                 FROM images c
                 JOIN images b ON c.blake3_hash = b.blake3_hash
                 WHERE c.scan_id = ?1
                   AND c.folder_id = ?2
                   AND c.blake3_hash IS NOT NULL
                   AND b.folder_id IN (SELECT id FROM folders WHERE scan_id = ?1 AND role = 'baseline')",
            )?;

            let duplicates: Vec<(i64, String, i64)> = stmt
                .query_map(rusqlite::params![scan_id, compare_folder_id], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;

            for (compare_id, hash, baseline_id) in duplicates {
                repository.conn().execute(
                    "INSERT OR IGNORE INTO duplicates (hash_group, original_image_id, duplicate_image_id, status, marked_at)
                     VALUES (?1, ?2, ?3, 'pending', ?4)",
                    rusqlite::params![hash, baseline_id, compare_id, now],
                )?;
            }
        }

        Ok(())
    }

    /// Phase 3: 主题分组（pHash 快速筛选）
    /// 根据 compare_mode 决定比对逻辑
    pub fn build_phash_candidates(
        scan_id: i64,
        repository: &Repository,
        compare_mode: CompareMode,
        hamming_threshold: u32,
    ) -> Result<Vec<(i64, i64, u32)>> {
        match compare_mode {
            CompareMode::Within => {
                Self::build_phash_candidates_within(scan_id, repository, hamming_threshold)
            }
            CompareMode::Between => {
                Self::build_phash_candidates_between(scan_id, repository, hamming_threshold)
            }
        }
    }

    /// 文件夹内 pHash 候选
    fn build_phash_candidates_within(
        scan_id: i64,
        repository: &Repository,
        hamming_threshold: u32,
    ) -> Result<Vec<(i64, i64, u32)>> {
        use crate::core::phash::PHashComputer;

        let mut stmt = repository.conn().prepare(
            "SELECT id, phash, blake3_hash FROM images WHERE scan_id = ?1 AND phash IS NOT NULL",
        )?;

        let images: Vec<(i64, String, String)> = stmt
            .query_map(rusqlite::params![scan_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut candidates = Vec::new();

        for i in 0..images.len() {
            for j in (i + 1)..images.len() {
                let (id1, hash1, blake3_1) = &images[i];
                let (id2, hash2, blake3_2) = &images[j];

                // 跳过完全相同的图片（已在 Phase 2 处理）
                if blake3_1 == blake3_2 {
                    continue;
                }

                let distance = PHashComputer::hamming_distance(hash1, hash2)?;

                if distance < hamming_threshold {
                    candidates.push((*id1, *id2, distance));
                }
            }
        }

        Ok(candidates)
    }

    /// 文件夹间 pHash 候选
    fn build_phash_candidates_between(
        scan_id: i64,
        repository: &Repository,
        hamming_threshold: u32,
    ) -> Result<Vec<(i64, i64, u32)>> {
        use crate::core::phash::PHashComputer;

        // 获取 baseline 图片
        let mut stmt = repository.conn().prepare(
            "SELECT i.id, i.phash, i.blake3_hash
             FROM images i
             JOIN folders f ON i.folder_id = f.id
             WHERE i.scan_id = ?1 AND f.role = 'baseline' AND i.phash IS NOT NULL",
        )?;

        let baseline_images: Vec<(i64, String, String)> = stmt
            .query_map(rusqlite::params![scan_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // 获取 compare 图片
        let mut stmt = repository.conn().prepare(
            "SELECT i.id, i.phash, i.blake3_hash
             FROM images i
             JOIN folders f ON i.folder_id = f.id
             WHERE i.scan_id = ?1 AND f.role = 'compare' AND i.phash IS NOT NULL",
        )?;

        let compare_images: Vec<(i64, String, String)> = stmt
            .query_map(rusqlite::params![scan_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut candidates = Vec::new();

        // 只比较 compare 与 baseline，不比较 compare 之间
        for (compare_id, compare_hash, compare_blake3) in &compare_images {
            for (baseline_id, baseline_hash, baseline_blake3) in &baseline_images {
                // 跳过完全相同的图片
                if compare_blake3 == baseline_blake3 {
                    continue;
                }

                let distance = PHashComputer::hamming_distance(compare_hash, baseline_hash)?;

                if distance < hamming_threshold {
                    candidates.push((*compare_id, *baseline_id, distance));
                }
            }
        }

        Ok(candidates)
    }

    /// Phase 4: 精确相似度计算（SSIM + 分类判定）
    pub fn compute_ssim_and_classify(
        scan_id: i64,
        repository: &Repository,
        ssim_threshold: f64,
    ) -> Result<()> {
        use crate::core::ssim::compute::SsimComputer;
        use std::path::Path;

        // 获取所有待计算 SSIM 的配对
        let mut stmt = repository.conn().prepare(
            "SELECT sp.id, i1.file_path, i2.file_path,
                    i1.width as w1, i1.height as h1, i1.file_size as s1,
                    i2.width as w2, i2.height as h2, i2.file_size as s2,
                    sp.size_ratio, sp.resolution_ratio
             FROM similar_pairs sp
             JOIN images i1 ON sp.larger_image_id = i1.id
             JOIN images i2 ON sp.smaller_image_id = i2.id
             WHERE i1.scan_id = ?1 AND sp.ssim_score IS NULL",
        )?;

        let pairs: Vec<(i64, String, String, u32, u32, i64, u32, u32, i64, f64, f64)> = stmt
            .query_map(rusqlite::params![scan_id], |row| {
                Ok((
                    row.get(0)?, row.get(1)?, row.get(2)?,
                    row.get(3)?, row.get(4)?, row.get(5)?,
                    row.get(6)?, row.get(7)?, row.get(8)?,
                    row.get(9)?, row.get(10)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let now = chrono::Utc::now().timestamp();

        for (pair_id, path1, path2, w1, h1, s1, w2, h2, s2, size_ratio, resolution_ratio) in pairs {
            // 预检查 - 快速跳过明显不同的图
            let resolution_diff = ((w1 as f64 * h1 as f64) - (w2 as f64 * h2 as f64)).abs()
                / (w1 as f64 * h1 as f64);
            let size_diff = ((s1 - s2).abs() as f64) / (s1 as f64);

            if resolution_diff > 0.2 && size_diff > 0.5 {
                // 标记为不相似，跳过 SSIM 计算
                repository.conn().execute(
                    "UPDATE similar_pairs
                     SET ssim_score = 0.0, similarity_type = 'similar', computed_at = ?1
                     WHERE id = ?2",
                    rusqlite::params![now, pair_id],
                )?;
                continue;
            }

            // 计算 SSIM
            let ssim = SsimComputer::compute_from_files(
                Path::new(&path1),
                Path::new(&path2),
            )?;

            // 判定相似度类型
            let resolution_same = w1 == w2 && h1 == h2;
            let (similarity_type, is_compressed) = Self::classify_similarity(
                ssim,
                size_ratio,
                resolution_same,
                ssim_threshold,
            );

            repository.conn().execute(
                "UPDATE similar_pairs
                 SET ssim_score = ?1, similarity_type = ?2, is_compressed_version = ?3,
                     ssim_threshold = ?4, computed_at = ?5
                 WHERE id = ?6",
                rusqlite::params![
                    ssim,
                    similarity_type.as_str(),
                    if is_compressed { 1 } else { 0 },
                    ssim_threshold,
                    now,
                    pair_id
                ],
            )?;
        }

        Ok(())
    }

    /// 分类判定逻辑（根据 IMAGE_COMPARISON_WORKFLOW.md）
    fn classify_similarity(
        ssim: f64,
        size_ratio: f64,
        resolution_same: bool,
        ssim_threshold: f64,
    ) -> (SimilarityType, bool) {
        if ssim >= ssim_threshold {
            // 极高相似度
            if !resolution_same || size_ratio < 0.9 {
                // 压缩版本
                (SimilarityType::Compressed, true)
            } else {
                // 极度相似（需人工确认）
                (SimilarityType::Similar, false)
            }
        } else if ssim >= 0.75 && ssim < ssim_threshold {
            // 中高相似度
            if resolution_same && size_ratio > 0.95 {
                // 差分图
                (SimilarityType::Diff, false)
            } else {
                // 相似但不同
                (SimilarityType::Similar, false)
            }
        } else {
            // 低相似度
            (SimilarityType::Similar, false)
        }
    }
}
