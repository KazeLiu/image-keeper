use crate::db::models::Image;

/// pHash 匹配引擎 - Phase 3 候选筛选
pub struct PhashMatcher {
    max_distance: i32,
    top_k: usize,
}

impl PhashMatcher {
    pub fn new(max_distance: i32, top_k: usize) -> Self {
        Self {
            max_distance,
            top_k,
        }
    }

    /// 计算两个 pHash 的汉明距离
    pub fn hamming_distance(hash1: &str, hash2: &str) -> Option<i32> {
        if hash1.len() != 16 || hash2.len() != 16 {
            return None;
        }

        let h1 = u64::from_str_radix(hash1, 16).ok()?;
        let h2 = u64::from_str_radix(hash2, 16).ok()?;

        Some((h1 ^ h2).count_ones() as i32)
    }

    /// 查找候选匹配（返回候选列表和是否被截断）
    pub fn find_candidates(
        &self,
        comparison_phash: &str,
        baseline_images: &[Image],
    ) -> (Vec<CandidateMatch>, bool) {
        let mut candidates = Vec::new();

        // 遍历所有基准图片，计算汉明距离
        for baseline_img in baseline_images {
            let Some(ref baseline_phash) = baseline_img.phash else {
                continue;
            };

            let Some(distance) = Self::hamming_distance(comparison_phash, baseline_phash) else {
                continue;
            };

            // 只保留距离 <= max_distance 的候选
            if distance <= self.max_distance {
                candidates.push(CandidateMatch {
                    baseline_image: baseline_img.clone(),
                    phash_distance: distance,
                });
            }
        }

        // 稳定排序：距离升序，然后按路径排序
        candidates.sort_by(|a, b| match a.phash_distance.cmp(&b.phash_distance) {
            std::cmp::Ordering::Equal => {
                a.baseline_image.file_path.cmp(&b.baseline_image.file_path)
            }
            other => other,
        });

        // Top-K 截断
        let truncated = candidates.len() > self.top_k;
        candidates.truncate(self.top_k);

        (candidates, truncated)
    }
}

/// 候选匹配
#[derive(Debug, Clone)]
pub struct CandidateMatch {
    pub baseline_image: Image,
    pub phash_distance: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hamming_distance() {
        // 相同哈希距离为 0
        assert_eq!(
            PhashMatcher::hamming_distance("0000000000000000", "0000000000000000"),
            Some(0)
        );

        // 1 位差异
        assert_eq!(
            PhashMatcher::hamming_distance("0000000000000000", "0000000000000001"),
            Some(1)
        );

        // 全部不同（64 位）
        assert_eq!(
            PhashMatcher::hamming_distance("0000000000000000", "ffffffffffffffff"),
            Some(64)
        );
    }

    #[test]
    fn test_find_candidates_top_k() {
        use crate::db::models::{FolderRole, ScanStatus};

        let matcher = PhashMatcher::new(10, 3);

        // 创建测试基准图片
        let baseline_images = vec![Image {
            id: 1,
            run_id: "test".to_string(),
            folder_id: 1,
            source_role: FolderRole::Baseline,
            file_path: "a.jpg".to_string(),
            relative_path: "a.jpg".to_string(),
            file_size: 1000,
            file_modified_at: 0,
            width: 1920,
            height: 1080,
            format: "jpg".to_string(),
            aspect_ratio: 1.777,
            frame_count: 1,
            frame_strategy: "first".to_string(),
            blake3_hash: Some("hash1".to_string()),
            phash: Some("0000000000000001".to_string()), // 距离 1
            phash_algorithm_version: Some("v1".to_string()),
            scan_status: ScanStatus::Completed,
            error_message: None,
            scanned_at: 0,
            hash_computed_at: Some(0),
        }];

        let (candidates, truncated) = matcher.find_candidates("0000000000000000", &baseline_images);

        // 应该返回 1 个候选
        assert_eq!(candidates.len(), 1);
        assert!(!truncated);

        // 验证距离
        assert_eq!(candidates[0].phash_distance, 1);
    }
}
