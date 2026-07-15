use crate::db::models::{AnalysisType, Image};

/// Phase 5: 配对分类与多候选仲裁
/// 根据匹配证据确定唯一的分析分类
pub struct ResultClassifier {
    compressed_ssim_threshold: f64,
    variant_review_lower_bound: f64,
    aspect_ratio_tolerance: f64,
    primary_match_tie_threshold: f64,
}

impl ResultClassifier {
    pub fn new(
        compressed_ssim_threshold: f64,
        variant_review_lower_bound: f64,
        aspect_ratio_tolerance: f64,
        primary_match_tie_threshold: f64,
    ) -> Self {
        Self {
            compressed_ssim_threshold,
            variant_review_lower_bound,
            aspect_ratio_tolerance,
            primary_match_tie_threshold,
        }
    }

    /// 分类单个对比图片的结果
    pub fn classify(
        &self,
        comparison_img: &Image,
        candidates: Vec<CandidateMatch>,
        candidate_truncated: bool,
    ) -> ClassificationResult {
        // 1. 候选查询不完整或被截断且无法可靠仲裁
        if candidate_truncated && candidates.len() >= 50 {
            return ClassificationResult {
                analysis_type: AnalysisType::Inconclusive,
                primary_match: None,
                all_candidates: candidates,
                reason: "候选被截断，无法可靠仲裁".to_string(),
            };
        }

        // 2. 没有候选
        if candidates.is_empty() {
            return ClassificationResult {
                analysis_type: AnalysisType::NoBaselineMatch,
                primary_match: None,
                all_candidates: vec![],
                reason: "未匹配到基准图片".to_string(),
            };
        }

        // 3. 精确匹配优先（BLAKE3 相同）
        let exact_matches: Vec<_> = candidates
            .iter()
            .filter(|c| c.match_type == MatchType::ExactHash)
            .cloned()
            .collect();

        if !exact_matches.is_empty() {
            // 选择路径稳定排序后的第一个作为主匹配
            let mut sorted = exact_matches.clone();
            sorted.sort_by(|a, b| a.baseline_image.file_path.cmp(&b.baseline_image.file_path));

            return ClassificationResult {
                analysis_type: AnalysisType::ExactDuplicate,
                primary_match: Some(sorted[0].clone()),
                all_candidates: exact_matches,
                reason: "BLAKE3 哈希完全相同".to_string(),
            };
        }

        // 4. 相似度匹配（pHash + SSIM）
        let similar_matches: Vec<_> = candidates
            .iter()
            .filter(|c| c.match_type == MatchType::Similar && c.ssim_score.is_some())
            .cloned()
            .collect();

        if similar_matches.is_empty() {
            // 所有候选未完成 SSIM 计算或失败
            return ClassificationResult {
                analysis_type: AnalysisType::NotEvaluated,
                primary_match: None,
                all_candidates: candidates,
                reason: "相似度计算未完成或失败".to_string(),
            };
        }

        // 5. 检查是否所有候选都低于变体审核下限
        let max_ssim = similar_matches
            .iter()
            .filter_map(|c| c.ssim_score)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        if max_ssim < self.variant_review_lower_bound {
            return ClassificationResult {
                analysis_type: AnalysisType::NoBaselineMatch,
                primary_match: None,
                all_candidates: candidates,
                reason: format!("所有候选 SSIM < {}", self.variant_review_lower_bound),
            };
        }

        // 6. 按可比性、SSIM、pHash 距离和基准图片分辨率稳定排序
        let mut sorted_candidates = similar_matches.clone();
        sorted_candidates.sort_by(|a, b| {
            // 优先：SSIM 降序
            let ssim_cmp = b
                .ssim_score
                .unwrap_or(0.0)
                .partial_cmp(&a.ssim_score.unwrap_or(0.0))
                .unwrap();
            if ssim_cmp != std::cmp::Ordering::Equal {
                return ssim_cmp;
            }

            // 次优：pHash 距离升序
            let phash_cmp = a.phash_distance.cmp(&b.phash_distance);
            if phash_cmp != std::cmp::Ordering::Equal {
                return phash_cmp;
            }

            // 再次：基准图片分辨率降序
            let res_a = a.baseline_image.width as u64 * a.baseline_image.height as u64;
            let res_b = b.baseline_image.width as u64 * b.baseline_image.height as u64;
            res_b.cmp(&res_a)
        });

        let primary = &sorted_candidates[0];

        // 7. 检查多候选冲突（前两项分数接近）
        if sorted_candidates.len() >= 2 {
            let second = &sorted_candidates[1];
            let ssim_diff =
                (primary.ssim_score.unwrap_or(0.0) - second.ssim_score.unwrap_or(0.0)).abs();

            if ssim_diff <= self.primary_match_tie_threshold {
                return ClassificationResult {
                    analysis_type: AnalysisType::Inconclusive,
                    primary_match: Some(primary.clone()),
                    all_candidates: similar_matches,
                    reason: format!(
                        "多个候选 SSIM 接近（差值 {} <= {}）",
                        ssim_diff, self.primary_match_tie_threshold
                    ),
                };
            }
        }

        // 8. 检查是否满足有方向性的压缩条件
        let ssim = primary.ssim_score.unwrap_or(0.0);
        if ssim >= self.compressed_ssim_threshold {
            if self.is_directional_compressed(comparison_img, &primary.baseline_image) {
                return ClassificationResult {
                    analysis_type: AnalysisType::LikelyCompressed,
                    primary_match: Some(primary.clone()),
                    all_candidates: similar_matches,
                    reason: format!("满足有方向性压缩条件（SSIM = {:.4}）", ssim),
                };
            }
        }

        // 9. 检查是否为变体图
        if ssim >= self.variant_review_lower_bound {
            // 同尺寸或近似尺寸且主题高度相似
            let same_resolution = comparison_img.width == primary.baseline_image.width
                && comparison_img.height == primary.baseline_image.height;

            let resolution_ratio = (comparison_img.width as f64 * comparison_img.height as f64)
                / (primary.baseline_image.width as f64 * primary.baseline_image.height as f64);

            if same_resolution || (0.95..=1.05).contains(&resolution_ratio) {
                if ssim < self.compressed_ssim_threshold {
                    return ClassificationResult {
                        analysis_type: AnalysisType::Variant,
                        primary_match: Some(primary.clone()),
                        all_candidates: similar_matches,
                        reason: format!("同尺寸但存在内容差异（SSIM = {:.4}）", ssim),
                    };
                }
            }
        }

        // 10. 其他达到审核下限但不满足压缩或变体条件
        ClassificationResult {
            analysis_type: AnalysisType::SimilarKeep,
            primary_match: Some(primary.clone()),
            all_candidates: similar_matches,
            reason: format!("相似但不满足压缩或变体条件（SSIM = {:.4}）", ssim),
        }
    }

    /// 检查是否满足有方向性的压缩条件
    fn is_directional_compressed(&self, comparison: &Image, baseline: &Image) -> bool {
        // 1. 宽度和高度都必须小于基准
        if comparison.width >= baseline.width || comparison.height >= baseline.height {
            return false;
        }

        // 2. 像素总数小于基准
        let comp_pixels = comparison.width as u64 * comparison.height as u64;
        let base_pixels = baseline.width as u64 * baseline.height as u64;
        if comp_pixels >= base_pixels {
            return false;
        }

        // 3. 文件大小小于基准
        if comparison.file_size >= baseline.file_size {
            return false;
        }

        // 4. 宽高比差异在容差范围内
        let aspect_diff = (comparison.aspect_ratio - baseline.aspect_ratio).abs()
            / comparison.aspect_ratio.max(baseline.aspect_ratio);

        if aspect_diff > self.aspect_ratio_tolerance {
            return false;
        }

        true
    }
}

/// 候选匹配
#[derive(Debug, Clone)]
pub struct CandidateMatch {
    pub baseline_image: Image,
    pub match_type: MatchType,
    pub phash_distance: i32,
    pub ssim_score: Option<f64>,
}

/// 匹配类型
#[derive(Debug, Clone, PartialEq)]
pub enum MatchType {
    ExactHash,
    Similar,
}

/// 分类结果
#[derive(Debug, Clone)]
pub struct ClassificationResult {
    pub analysis_type: AnalysisType,
    pub primary_match: Option<CandidateMatch>,
    pub all_candidates: Vec<CandidateMatch>,
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::FolderRole;

    fn create_test_image(width: u32, height: u32, file_size: i64, path: &str) -> Image {
        Image {
            id: 1,
            run_id: "test".to_string(),
            folder_id: 1,
            source_role: FolderRole::Comparison,
            file_path: path.to_string(),
            relative_path: path.to_string(),
            file_size,
            file_modified_at: 0,
            width,
            height,
            format: "jpg".to_string(),
            aspect_ratio: width as f64 / height as f64,
            frame_count: 1,
            frame_strategy: "first_frame".to_string(),
            blake3_hash: None,
            phash: None,
            phash_algorithm_version: None,
            scan_status: crate::db::models::ScanStatus::Completed,
            error_message: None,
            scanned_at: 0,
            hash_computed_at: None,
        }
    }

    #[test]
    fn test_classify_exact_duplicate() {
        let classifier = ResultClassifier::new(0.995, 0.75, 0.005, 0.001);
        let comparison = create_test_image(1920, 1080, 500000, "comp.jpg");
        let baseline = create_test_image(1920, 1080, 500000, "base.jpg");

        let candidates = vec![CandidateMatch {
            baseline_image: baseline,
            match_type: MatchType::ExactHash,
            phash_distance: 0,
            ssim_score: None,
        }];

        let result = classifier.classify(&comparison, candidates, false);
        assert_eq!(result.analysis_type, AnalysisType::ExactDuplicate);
    }

    #[test]
    fn test_classify_likely_compressed() {
        let classifier = ResultClassifier::new(0.995, 0.75, 0.005, 0.001);
        let comparison = create_test_image(1920, 1080, 200000, "comp.jpg");
        let baseline = create_test_image(3840, 2160, 800000, "base.jpg");

        let candidates = vec![CandidateMatch {
            baseline_image: baseline,
            match_type: MatchType::Similar,
            phash_distance: 2,
            ssim_score: Some(0.996),
        }];

        let result = classifier.classify(&comparison, candidates, false);
        assert_eq!(result.analysis_type, AnalysisType::LikelyCompressed);
    }

    #[test]
    fn test_classify_variant() {
        let classifier = ResultClassifier::new(0.995, 0.75, 0.005, 0.001);
        let comparison = create_test_image(1920, 1080, 500000, "comp.jpg");
        let baseline = create_test_image(1920, 1080, 550000, "base.jpg");

        let candidates = vec![CandidateMatch {
            baseline_image: baseline,
            match_type: MatchType::Similar,
            phash_distance: 5,
            ssim_score: Some(0.85),
        }];

        let result = classifier.classify(&comparison, candidates, false);
        assert_eq!(result.analysis_type, AnalysisType::Variant);
    }

    #[test]
    fn test_classify_no_match() {
        let classifier = ResultClassifier::new(0.995, 0.75, 0.005, 0.001);
        let comparison = create_test_image(1920, 1080, 500000, "comp.jpg");

        let result = classifier.classify(&comparison, vec![], false);
        assert_eq!(result.analysis_type, AnalysisType::NoBaselineMatch);
    }
}
