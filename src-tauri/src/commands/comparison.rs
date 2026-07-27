use crate::core::algorithm_profile::{
    algorithm_pool, ALGORITHM_WORKER_COUNT, CURRENT_ALGORITHM_PROFILE_ID,
};
use crate::db::models::{AnalysisType, ComparisonStats, RunStatus};
use crate::db::repository::{Repository, RunConfig};
use crate::error::{AppError, Result};
use chrono::Utc;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicUsize, Ordering as AtomicOrdering},
    Arc, Mutex, OnceLock,
};
use tauri::{Emitter, State, Window};

/// 多文件夹对比请求
#[derive(Debug, serde::Deserialize)]
pub struct MultiCompareRequest {
    pub baseline_path: String,
    pub comparison_paths: Vec<String>,
    pub directory_options: Option<Vec<DirectoryCompareOption>>,
}

/// 目录级对比选项
#[derive(Debug, serde::Deserialize)]
pub struct DirectoryCompareOption {
    pub path: String,
    pub compare_within: bool,
}

/// 运行状态快照
#[derive(Debug, serde::Serialize)]
pub struct RunStatusResponse {
    pub run_id: String,
    pub status: RunStatus,
    pub completed_at: Option<i64>,
}

/// 历史运行列表行
#[derive(Debug, serde::Serialize)]
pub struct ComparisonRunHistoryItem {
    pub run_id: String,
    pub status: RunStatus,
    pub baseline_root_path: String,
    pub comparison_root_paths: Vec<String>,
    pub baseline_total: i64,
    pub comparison_total: i64,
    pub result_count: i64,
    pub error_count: i64,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
}

/// 前端分类结果列表行
#[derive(Debug, serde::Serialize)]
pub struct ComparisonResultRow {
    pub id: i64,
    pub run_id: String,
    pub comparison_image_id: i64,
    pub comparison_path: String,
    pub comparison_relative_path: String,
    pub comparison_file_size: i64,
    pub comparison_width: u32,
    pub comparison_height: u32,
    pub analysis_type: AnalysisType,
    pub primary_match_image_id: Option<i64>,
    pub primary_match_path: Option<String>,
    pub primary_match_relative_path: Option<String>,
    pub all_candidate_ids: Option<Vec<i64>>,
    pub candidate_truncated: bool,
    pub phash_distance: Option<i32>,
    pub ssim_score: Option<f64>,
    pub size_ratio: Option<f64>,
    pub resolution_ratio: Option<f64>,
    pub aspect_diff: Option<f64>,
    pub direction_smaller_resolution: bool,
    pub direction_smaller_filesize: bool,
    pub algorithm_profile_id: String,
    pub analysis_metadata: Option<String>,
    pub computed_at: i64,
}

/// 感知哈希粗分组
#[derive(Debug, serde::Serialize)]
pub struct ComparisonGroup {
    pub group_index: usize,
    pub representative_image_id: i64,
    pub representative_file_name: String,
    pub member_count: usize,
    pub has_low_quality_suggestion: bool,
    pub members: Vec<ComparisonGroupMember>,
}

/// 分组内图片行
#[derive(Debug, serde::Serialize, Clone)]
pub struct ComparisonGroupMember {
    pub image_id: i64,
    pub file_path: String,
    pub relative_path: String,
    pub file_size: i64,
    pub width: u32,
    pub height: u32,
    pub phash: Option<String>,
    pub phash_distance_to_reference: Option<i32>,
    pub role: String,
    pub role_label: String,
    pub reference_image_id: Option<i64>,
    pub reference_relative_path: Option<String>,
    pub ssim_score: Option<f64>,
    pub ssim_cluster_key: String,
    pub is_low_quality_suggestion: bool,
}

/// 当前分组内两两图片相似度。用于前端把缩略图挂到最像的原图下面。
#[derive(Debug, Clone, serde::Serialize)]
pub struct GroupSimilarityScore {
    pub left_image_id: i64,
    pub right_image_id: i64,
    pub ssim_score: Option<f64>,
    pub error_message: Option<String>,
}

/// 当前分组交叉验证进度。只统计真实需要处理的组合，不做假进度。
#[derive(Debug, Clone, serde::Serialize)]
pub struct GroupSimilarityProgress {
    pub request_id: String,
    pub status: String,
    pub phase: String,
    pub total_pairs: usize,
    pub processed_pairs: usize,
    pub total_images: usize,
    pub processed_images: usize,
    pub current_left_image_id: Option<i64>,
    pub current_right_image_id: Option<i64>,
    pub current_left_file_name: Option<String>,
    pub current_right_file_name: Option<String>,
    pub current_image_id: Option<i64>,
    pub current_image_file_name: Option<String>,
    pub cache_hits: usize,
    pub image_cache_hits: usize,
    pub computed_pairs: usize,
    pub skipped_pairs: usize,
}

#[derive(Debug, Clone)]
struct GroupSimilarityPlanPair {
    left_index: usize,
    right_index: usize,
}

#[derive(Debug, Clone)]
struct GroupSimilarityImageCacheJob {
    image_index: usize,
    target_width: u32,
    target_height: u32,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct GroupSimilarityCacheKey {
    run_id: String,
    algorithm_profile_id: String,
    left: SimilaritySourceFingerprint,
    right: SimilaritySourceFingerprint,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct GroupSimilarityResultCacheKey {
    run_id: String,
    algorithm_profile_id: String,
    members: Vec<GroupSimilarityResultCacheMember>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct GroupSimilarityResultCacheMember {
    source: SimilaritySourceFingerprint,
    phash: Option<String>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct SimilaritySourceFingerprint {
    image_id: i64,
    canonical_file_path: String,
    current_file_size: u64,
    current_modified_ns: u128,
    source_width: u32,
    source_height: u32,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct SimilarityImageCacheKey {
    run_id: String,
    algorithm_profile_id: String,
    source: SimilaritySourceFingerprint,
    target_width: u32,
    target_height: u32,
}

#[derive(Debug, Clone)]
struct CachedSimilarityImage {
    gray: image::GrayImage,
}

#[derive(Debug, Clone)]
struct ImageSummary {
    id: i64,
    file_path: String,
    canonical_file_path: String,
    relative_path: String,
    file_size: i64,
    file_modified_at: i64,
    current_file_size: u64,
    current_modified_ns: u128,
    width: u32,
    height: u32,
    phash: Option<String>,
}

#[derive(Debug, Clone)]
struct AnalysisSummary {
    analysis_type: AnalysisType,
    primary_match_image_id: Option<i64>,
    phash_distance: Option<i32>,
    ssim_score: Option<f64>,
}

const GROUP_SIMILARITY_MAX_CANDIDATES_PER_IMAGE: usize = 8;
const GROUP_SIMILARITY_PAIR_PHASH_MAX_DISTANCE: i32 = 24;
const GROUP_SIMILARITY_ORIGINAL_PIXEL_RATIO: f64 = 0.9;
const GROUP_SIMILARITY_ORIGINAL_FILE_RATIO: f64 = 0.75;
const GROUP_SIMILARITY_LOWER_PIXEL_RATIO: f64 = 0.98;
const GROUP_SIMILARITY_LOWER_FILE_RATIO: f64 = 0.98;
const GROUP_SIMILARITY_RESULT_CACHE_LIMIT: usize = 50;
const GROUP_SIMILARITY_IMAGE_CACHE_BYTE_BUDGET: usize = 256 * 1024 * 1024;

static GROUP_SIMILARITY_CACHE: OnceLock<
    Mutex<HashMap<GroupSimilarityCacheKey, GroupSimilarityScore>>,
> = OnceLock::new();
static GROUP_SIMILARITY_RESULT_CACHE: OnceLock<
    Mutex<HashMap<GroupSimilarityResultCacheKey, Vec<GroupSimilarityScore>>>,
> = OnceLock::new();
static GROUP_SIMILARITY_IMAGE_CACHE: OnceLock<
    Mutex<HashMap<SimilarityImageCacheKey, Arc<CachedSimilarityImage>>>,
> = OnceLock::new();

fn read_run_status(repo: &Repository, run_id: &str) -> Result<RunStatusResponse> {
    let run = repo
        .get_run(run_id)?
        .ok_or_else(|| AppError::NotFound(format!("运行不存在: {}", run_id)))?;

    Ok(RunStatusResponse {
        run_id: run.run_id,
        status: run.status,
        completed_at: run.completed_at,
    })
}

fn read_comparison_run_history(
    repo: &Repository,
    limit: i64,
) -> Result<Vec<ComparisonRunHistoryItem>> {
    let active_limit = limit.clamp(1, 50);
    let mut stmt = repo.conn().prepare(
        r#"SELECT
               r.run_id,
               r.status,
               r.baseline_root_path,
               r.comparison_root_paths,
               r.total_baseline_files,
               r.total_comparison_files,
               r.error_count,
               r.created_at,
               r.started_at,
               r.completed_at,
               (SELECT COUNT(*) FROM analysis_results ar WHERE ar.run_id = r.run_id) AS result_count,
               (SELECT COUNT(*) FROM images i WHERE i.run_id = r.run_id AND i.source_role = 'baseline') AS baseline_image_count,
               (SELECT COUNT(*) FROM images i WHERE i.run_id = r.run_id AND i.source_role = 'comparison') AS comparison_image_count
           FROM runs r
           ORDER BY r.created_at DESC, r.id DESC
           LIMIT ?1"#,
    )?;

    let rows = stmt.query_map([active_limit], |row| {
        let status_raw: String = row.get(1)?;
        let comparison_paths_raw: String = row.get(3)?;
        let comparison_root_paths =
            serde_json::from_str::<Vec<String>>(&comparison_paths_raw).unwrap_or_default();

        let total_baseline_files = row.get::<_, i64>(4)?;
        let total_comparison_files = row.get::<_, i64>(5)?;
        let result_count = row.get::<_, i64>(10)?;
        let baseline_image_count = row.get::<_, i64>(11)?;
        let comparison_image_count = row.get::<_, i64>(12)?;

        Ok(ComparisonRunHistoryItem {
            run_id: row.get(0)?,
            status: RunStatus::from_str(&status_raw).unwrap_or(RunStatus::Failed),
            baseline_root_path: row.get(2)?,
            comparison_root_paths,
            baseline_total: total_baseline_files.max(baseline_image_count),
            comparison_total: total_comparison_files.max(comparison_image_count.max(result_count)),
            error_count: row.get(6)?,
            created_at: row.get(7)?,
            started_at: row.get(8)?,
            completed_at: row.get(9)?,
            result_count,
        })
    })?;

    let mut history = Vec::new();
    for row in rows {
        history.push(row?);
    }

    Ok(history)
}

fn delete_comparison_run_records(repo: &Repository, run_id: &str) -> Result<()> {
    if repo.get_run(run_id)?.is_none() {
        return Err(AppError::NotFound(format!("历史任务不存在: {}", run_id)));
    }

    repo.conn().execute(
        r#"DELETE FROM review_status
           WHERE analysis_result_id IN (
               SELECT id FROM analysis_results WHERE run_id = ?1
           )"#,
        [run_id],
    )?;
    repo.conn()
        .execute("DELETE FROM recycle_bin WHERE run_id = ?1", [run_id])?;
    repo.conn()
        .execute("DELETE FROM operation_logs WHERE run_id = ?1", [run_id])?;
    repo.conn()
        .execute("DELETE FROM analysis_results WHERE run_id = ?1", [run_id])?;
    repo.conn()
        .execute("DELETE FROM images WHERE run_id = ?1", [run_id])?;
    repo.conn()
        .execute("DELETE FROM folders WHERE run_id = ?1", [run_id])?;
    repo.conn()
        .execute("DELETE FROM runs WHERE run_id = ?1", [run_id])?;

    Ok(())
}

fn read_comparison_results(repo: &Repository, run_id: &str) -> Result<Vec<ComparisonResultRow>> {
    let mut stmt = repo.conn().prepare(
        r#"SELECT
               ar.id,
               ar.run_id,
               ar.comparison_image_id,
               ci.file_path,
               ci.relative_path,
               ci.file_size,
               ci.width,
               ci.height,
               ar.analysis_type,
               ar.primary_match_image_id,
               bi.file_path,
               bi.relative_path,
               ar.all_candidate_ids,
               ar.candidate_truncated,
               ar.phash_distance,
               ar.ssim_score,
               ar.size_ratio,
               ar.resolution_ratio,
               ar.aspect_diff,
               ar.direction_smaller_resolution,
               ar.direction_smaller_filesize,
               ar.algorithm_profile_id,
               ar.analysis_metadata,
               ar.computed_at
           FROM analysis_results ar
           JOIN images ci ON ar.comparison_image_id = ci.id
           LEFT JOIN images bi ON ar.primary_match_image_id = bi.id
           WHERE ar.run_id = ?1
           ORDER BY
               CASE ar.analysis_type
                   WHEN 'exact_duplicate' THEN 1
                   WHEN 'likely_compressed' THEN 2
                   WHEN 'variant' THEN 3
                   WHEN 'inconclusive' THEN 4
                   WHEN 'similar_keep' THEN 5
                   WHEN 'no_baseline_match' THEN 6
                   WHEN 'not_evaluated' THEN 7
                   ELSE 8
               END,
               ar.id"#,
    )?;

    let rows = stmt
        .query_map([run_id], |row| {
            let analysis_type_raw: String = row.get(8)?;
            let all_candidate_ids = row
                .get::<_, Option<String>>(12)?
                .and_then(|raw| serde_json::from_str(&raw).ok());

            Ok(ComparisonResultRow {
                id: row.get(0)?,
                run_id: row.get(1)?,
                comparison_image_id: row.get(2)?,
                comparison_path: row.get(3)?,
                comparison_relative_path: row.get(4)?,
                comparison_file_size: row.get(5)?,
                comparison_width: row.get(6)?,
                comparison_height: row.get(7)?,
                analysis_type: AnalysisType::from_str(&analysis_type_raw)
                    .unwrap_or(AnalysisType::Error),
                primary_match_image_id: row.get(9)?,
                primary_match_path: row.get(10)?,
                primary_match_relative_path: row.get(11)?,
                all_candidate_ids,
                candidate_truncated: row.get(13)?,
                phash_distance: row.get(14)?,
                ssim_score: row.get(15)?,
                size_ratio: row.get(16)?,
                resolution_ratio: row.get(17)?,
                aspect_diff: row.get(18)?,
                direction_smaller_resolution: row.get(19)?,
                direction_smaller_filesize: row.get(20)?,
                algorithm_profile_id: row.get(21)?,
                analysis_metadata: row.get(22)?,
                computed_at: row.get(23)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(rows)
}

fn read_completed_images(repo: &Repository, run_id: &str) -> Result<Vec<ImageSummary>> {
    let mut stmt = repo.conn().prepare(
        r#"SELECT id, file_path, relative_path, file_size, width, height, phash, file_modified_at
           FROM images
           WHERE run_id = ?1
             AND scan_status = 'completed'
             AND NOT EXISTS (
                 SELECT 1
                 FROM analysis_results ar
                 WHERE ar.run_id = images.run_id
                   AND ar.comparison_image_id = images.id
                   AND COALESCE((
                       SELECT ol.operation_type
                       FROM operation_logs ol
                       WHERE ol.analysis_result_id = ar.id
                       ORDER BY ol.created_at DESC, ol.id DESC
                       LIMIT 1
                   ), 'none') IN ('recycled', 'permanently_deleted')
             )
           ORDER BY relative_path, id"#,
    )?;

    let images = stmt
        .query_map([run_id], |row| {
            let file_path: String = row.get(1)?;
            let file_size: i64 = row.get(3)?;
            let file_modified_at: i64 = row.get(7)?;
            Ok(ImageSummary {
                id: row.get(0)?,
                canonical_file_path: file_path.clone(),
                file_path,
                relative_path: row.get(2)?,
                file_size,
                file_modified_at,
                current_file_size: file_size.max(0) as u64,
                current_modified_ns: file_modified_at.max(0) as u128 * 1_000_000_000,
                width: row.get(4)?,
                height: row.get(5)?,
                phash: row.get(6)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(images)
}

fn read_images_by_ids(
    repo: &Repository,
    run_id: &str,
    image_ids: &[i64],
) -> Result<Vec<ImageSummary>> {
    if image_ids.is_empty() {
        return Ok(Vec::new());
    }

    if image_ids.len() > 800 {
        return Err(AppError::ValidationError(
            "单个分组图片过多，请先调严格分组范围后再查看详情".to_string(),
        ));
    }

    let placeholders = std::iter::repeat("?")
        .take(image_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        r#"SELECT id, file_path, relative_path, file_size, width, height, phash, file_modified_at
           FROM images
           WHERE run_id = ? AND scan_status = 'completed' AND id IN ({placeholders})
           ORDER BY relative_path, id"#
    );

    let mut params: Vec<rusqlite::types::Value> = Vec::with_capacity(image_ids.len() + 1);
    params.push(run_id.to_string().into());
    params.extend(image_ids.iter().copied().map(Into::into));

    let mut stmt = repo.conn().prepare(&sql)?;
    let images = stmt
        .query_map(rusqlite::params_from_iter(params), |row| {
            let file_path: String = row.get(1)?;
            let file_size: i64 = row.get(3)?;
            let file_modified_at: i64 = row.get(7)?;
            Ok(ImageSummary {
                id: row.get(0)?,
                canonical_file_path: file_path.clone(),
                file_path,
                relative_path: row.get(2)?,
                file_size,
                file_modified_at,
                current_file_size: file_size.max(0) as u64,
                current_modified_ns: file_modified_at.max(0) as u128 * 1_000_000_000,
                width: row.get(4)?,
                height: row.get(5)?,
                phash: row.get(6)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(images)
}

fn read_analysis_summaries(
    repo: &Repository,
    run_id: &str,
) -> Result<HashMap<i64, AnalysisSummary>> {
    let mut stmt = repo.conn().prepare(
        r#"SELECT comparison_image_id, analysis_type, primary_match_image_id, phash_distance, ssim_score
           FROM analysis_results
           WHERE run_id = ?1"#,
    )?;

    let rows = stmt.query_map([run_id], |row| {
        let analysis_type_raw: String = row.get(1)?;
        Ok((
            row.get::<_, i64>(0)?,
            AnalysisSummary {
                analysis_type: AnalysisType::from_str(&analysis_type_raw)
                    .unwrap_or(AnalysisType::Error),
                primary_match_image_id: row.get(2)?,
                phash_distance: row.get(3)?,
                ssim_score: row.get(4)?,
            },
        ))
    })?;

    let mut summaries = HashMap::new();
    for row in rows {
        let (image_id, summary) = row?;
        summaries.insert(image_id, summary);
    }

    Ok(summaries)
}

fn find_parent(parent: &mut [usize], index: usize) -> usize {
    if parent[index] != index {
        parent[index] = find_parent(parent, parent[index]);
    }
    parent[index]
}

fn union_parent(parent: &mut [usize], left: usize, right: usize) {
    let left_root = find_parent(parent, left);
    let right_root = find_parent(parent, right);

    if left_root != right_root {
        parent[right_root] = left_root;
    }
}

fn pixel_count(image: &ImageSummary) -> u64 {
    image.width as u64 * image.height as u64
}

fn file_name_from_path(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

fn ensure_current_algorithm_profile(algorithm_profile_id: &str) -> Result<()> {
    if algorithm_profile_id == CURRENT_ALGORITHM_PROFILE_ID {
        return Ok(());
    }

    Err(AppError::ValidationError(format!(
        "该历史任务使用旧算法配置 {algorithm_profile_id}，不能与当前标准 SSIM/pHash 数值混用，请重新运行任务"
    )))
}

fn refresh_current_file_fingerprints(images: &mut [ImageSummary]) -> Result<()> {
    for image in images {
        let canonical = std::fs::canonicalize(Path::new(&image.file_path)).map_err(|error| {
            AppError::FileSystem(format!("无法读取图片 {}: {error}", image.relative_path))
        })?;
        let metadata = std::fs::metadata(&canonical)?;
        if !metadata.is_file() {
            return Err(AppError::ValidationError(format!(
                "图片已不再是普通文件，请重新运行任务: {}",
                image.relative_path
            )));
        }
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok());
        let current_modified_secs = modified
            .as_ref()
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or_default();

        if metadata.len() != image.file_size.max(0) as u64
            || current_modified_secs != image.file_modified_at
        {
            return Err(AppError::ValidationError(format!(
                "图片在任务完成后已发生变化，请重新运行任务: {}",
                image.relative_path
            )));
        }

        image.canonical_file_path = canonical.to_string_lossy().into_owned();
        image.current_file_size = metadata.len();
        image.current_modified_ns = modified
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
    }

    Ok(())
}

fn validate_current_file_fingerprints(images: &[ImageSummary]) -> Result<()> {
    for image in images {
        let canonical = std::fs::canonicalize(Path::new(&image.file_path)).map_err(|error| {
            AppError::FileSystem(format!("无法读取图片 {}: {error}", image.relative_path))
        })?;
        let metadata = std::fs::metadata(&canonical)?;
        let modified_ns = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();

        if canonical.to_string_lossy() != image.canonical_file_path
            || metadata.len() != image.current_file_size
            || modified_ns != image.current_modified_ns
        {
            return Err(AppError::ValidationError(format!(
                "图片在相似度计算期间已发生变化，请重试: {}",
                image.relative_path
            )));
        }
    }

    Ok(())
}

fn choose_group_reference<'a>(images: &'a [&ImageSummary]) -> &'a ImageSummary {
    images
        .iter()
        .copied()
        .max_by(|left, right| {
            pixel_count(left)
                .cmp(&pixel_count(right))
                .then(left.file_size.cmp(&right.file_size))
                .then(right.relative_path.cmp(&left.relative_path))
        })
        .expect("group contains at least one image")
}

fn build_group_member(
    image: &ImageSummary,
    reference: &ImageSummary,
    image_by_id: &HashMap<i64, ImageSummary>,
    analysis_by_image_id: &HashMap<i64, AnalysisSummary>,
) -> ComparisonGroupMember {
    use crate::core::matching::PhashMatcher;

    let analysis = analysis_by_image_id.get(&image.id);
    let relation_reference = analysis
        .and_then(|summary| summary.primary_match_image_id)
        .and_then(|id| image_by_id.get(&id));

    let effective_reference = relation_reference.unwrap_or(reference);
    let phash_distance_to_reference = match (&image.phash, &effective_reference.phash) {
        (Some(left), Some(right)) if image.id != effective_reference.id => {
            PhashMatcher::hamming_distance(left, right)
        }
        _ => None,
    };

    let image_pixels = pixel_count(image);
    let reference_pixels = pixel_count(effective_reference);
    let primary_is_better = reference_pixels > image_pixels
        || (reference_pixels == image_pixels && effective_reference.file_size > image.file_size);

    let ssim_score = analysis.and_then(|summary| summary.ssim_score);
    let is_low_quality_suggestion = analysis.is_some_and(|summary| {
        summary.analysis_type == AnalysisType::LikelyCompressed
            || (summary.ssim_score.unwrap_or(0.0) >= 0.995 && primary_is_better)
    });

    let (role, role_label) = if image.id == reference.id {
        ("reference", "组内参考图")
    } else if is_low_quality_suggestion {
        ("lower_quality", "疑似低质量")
    } else if analysis.is_some_and(|summary| summary.analysis_type == AnalysisType::Inconclusive) {
        ("needs_review", "需确认")
    } else if analysis.is_some_and(|summary| summary.analysis_type == AnalysisType::NoBaselineMatch)
    {
        ("standalone", "无相似对象")
    } else {
        ("candidate", "相似候选")
    };

    let ssim_cluster_key = analysis
        .and_then(|summary| summary.primary_match_image_id)
        .unwrap_or(image.id)
        .to_string();

    ComparisonGroupMember {
        image_id: image.id,
        file_path: image.file_path.clone(),
        relative_path: image.relative_path.clone(),
        file_size: image.file_size,
        width: image.width,
        height: image.height,
        phash: image.phash.clone(),
        phash_distance_to_reference: analysis
            .and_then(|summary| summary.phash_distance)
            .or(phash_distance_to_reference),
        role: role.to_string(),
        role_label: role_label.to_string(),
        reference_image_id: if image.id == effective_reference.id {
            None
        } else {
            Some(effective_reference.id)
        },
        reference_relative_path: if image.id == effective_reference.id {
            None
        } else {
            Some(effective_reference.relative_path.clone())
        },
        ssim_score,
        ssim_cluster_key,
        is_low_quality_suggestion,
    }
}

fn read_comparison_groups(
    repo: &Repository,
    run_id: &str,
    grouping_distance: Option<i32>,
) -> Result<Vec<ComparisonGroup>> {
    use crate::core::matching::PhashMatcher;

    let run = repo
        .get_run(run_id)?
        .ok_or_else(|| AppError::NotFound(format!("运行不存在: {}", run_id)))?;
    ensure_current_algorithm_profile(&run.algorithm_profile_id)?;
    let active_grouping_distance = grouping_distance
        .unwrap_or(run.phash_max_distance)
        .clamp(0, 24);
    let images = read_completed_images(repo, run_id)?;
    let analysis_by_image_id = read_analysis_summaries(repo, run_id)?;
    let image_by_id: HashMap<i64, ImageSummary> = images
        .iter()
        .map(|image| (image.id, image.clone()))
        .collect();

    if images.is_empty() {
        return Ok(Vec::new());
    }

    let mut parent: Vec<usize> = (0..images.len()).collect();
    for left_index in 0..images.len() {
        for right_index in (left_index + 1)..images.len() {
            let (Some(left_phash), Some(right_phash)) =
                (&images[left_index].phash, &images[right_index].phash)
            else {
                continue;
            };

            let Some(distance) = PhashMatcher::hamming_distance(left_phash, right_phash) else {
                continue;
            };

            if distance <= active_grouping_distance {
                union_parent(&mut parent, left_index, right_index);
            }
        }
    }

    let mut grouped_indices: HashMap<usize, Vec<usize>> = HashMap::new();
    for index in 0..images.len() {
        let root = find_parent(&mut parent, index);
        grouped_indices.entry(root).or_default().push(index);
    }

    let mut raw_groups: Vec<Vec<&ImageSummary>> = grouped_indices
        .values()
        .map(|indices| {
            let mut group_images: Vec<&ImageSummary> =
                indices.iter().map(|index| &images[*index]).collect();
            group_images.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
            group_images
        })
        .collect();

    raw_groups.sort_by(|left, right| {
        right
            .len()
            .cmp(&left.len())
            .then(left[0].relative_path.cmp(&right[0].relative_path))
    });

    let groups = raw_groups
        .into_iter()
        .enumerate()
        .map(|(group_index, group_images)| {
            let reference = choose_group_reference(&group_images);
            let mut members: Vec<ComparisonGroupMember> = group_images
                .iter()
                .map(|image| {
                    build_group_member(image, reference, &image_by_id, &analysis_by_image_id)
                })
                .collect();

            members.sort_by(|left, right| {
                left.ssim_cluster_key
                    .cmp(&right.ssim_cluster_key)
                    .then(left.role.cmp(&right.role))
                    .then(left.relative_path.cmp(&right.relative_path))
            });

            ComparisonGroup {
                group_index: group_index + 1,
                representative_image_id: reference.id,
                representative_file_name: file_name_from_path(&reference.relative_path),
                member_count: members.len(),
                has_low_quality_suggestion: members
                    .iter()
                    .any(|member| member.is_low_quality_suggestion),
                members,
            }
        })
        .collect();

    Ok(groups)
}

fn group_similarity_cache() -> &'static Mutex<HashMap<GroupSimilarityCacheKey, GroupSimilarityScore>>
{
    GROUP_SIMILARITY_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn group_similarity_result_cache(
) -> &'static Mutex<HashMap<GroupSimilarityResultCacheKey, Vec<GroupSimilarityScore>>> {
    GROUP_SIMILARITY_RESULT_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn group_similarity_image_cache(
) -> &'static Mutex<HashMap<SimilarityImageCacheKey, Arc<CachedSimilarityImage>>> {
    GROUP_SIMILARITY_IMAGE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn similarity_image_cache_bytes(
    cache: &HashMap<SimilarityImageCacheKey, Arc<CachedSimilarityImage>>,
) -> usize {
    cache.values().map(|image| image.gray.as_raw().len()).sum()
}

fn insert_similarity_image_cache_entry(
    cache: &mut HashMap<SimilarityImageCacheKey, Arc<CachedSimilarityImage>>,
    cache_key: SimilarityImageCacheKey,
    cached_image: Arc<CachedSimilarityImage>,
    byte_budget: usize,
) {
    cache.insert(cache_key.clone(), cached_image);

    while cache.len() > 1 && similarity_image_cache_bytes(cache) > byte_budget {
        let Some(eviction_key) = cache.keys().find(|key| *key != &cache_key).cloned() else {
            break;
        };
        cache.remove(&eviction_key);
    }
}

fn similarity_source_fingerprint(image: &ImageSummary) -> SimilaritySourceFingerprint {
    SimilaritySourceFingerprint {
        image_id: image.id,
        canonical_file_path: image.canonical_file_path.clone(),
        current_file_size: image.current_file_size,
        current_modified_ns: image.current_modified_ns,
        source_width: image.width,
        source_height: image.height,
    }
}

fn group_similarity_cache_key(
    run_id: &str,
    left: &ImageSummary,
    right: &ImageSummary,
) -> GroupSimilarityCacheKey {
    let left = similarity_source_fingerprint(left);
    let right = similarity_source_fingerprint(right);
    let (left, right) = if left <= right {
        (left, right)
    } else {
        (right, left)
    };

    GroupSimilarityCacheKey {
        run_id: run_id.to_string(),
        algorithm_profile_id: CURRENT_ALGORITHM_PROFILE_ID.to_string(),
        left,
        right,
    }
}

fn group_similarity_result_cache_key(
    run_id: &str,
    images: &[ImageSummary],
) -> GroupSimilarityResultCacheKey {
    let mut members = images
        .iter()
        .map(|image| GroupSimilarityResultCacheMember {
            source: similarity_source_fingerprint(image),
            phash: image.phash.clone(),
        })
        .collect::<Vec<_>>();
    members.sort();

    GroupSimilarityResultCacheKey {
        run_id: run_id.to_string(),
        algorithm_profile_id: CURRENT_ALGORITHM_PROFILE_ID.to_string(),
        members,
    }
}

fn similarity_image_cache_key(
    run_id: &str,
    image: &ImageSummary,
    target_width: u32,
    target_height: u32,
) -> SimilarityImageCacheKey {
    SimilarityImageCacheKey {
        run_id: run_id.to_string(),
        algorithm_profile_id: CURRENT_ALGORITHM_PROFILE_ID.to_string(),
        source: similarity_source_fingerprint(image),
        target_width,
        target_height,
    }
}

fn phash_distance_between(left: &ImageSummary, right: &ImageSummary) -> Option<i32> {
    use crate::core::matching::PhashMatcher;

    match (&left.phash, &right.phash) {
        (Some(left_phash), Some(right_phash)) => {
            PhashMatcher::hamming_distance(left_phash, right_phash)
        }
        _ => None,
    }
}

fn passes_group_similarity_phash_pruning(left: &ImageSummary, right: &ImageSummary) -> bool {
    phash_distance_between(left, right)
        .is_none_or(|distance| distance <= GROUP_SIMILARITY_PAIR_PHASH_MAX_DISTANCE)
}

fn is_probable_original_anchor(image: &ImageSummary, max_pixels: u64, max_file_size: i64) -> bool {
    let high_resolution = if max_pixels == 0 {
        true
    } else {
        pixel_count(image) as f64 >= max_pixels as f64 * GROUP_SIMILARITY_ORIGINAL_PIXEL_RATIO
    };
    let large_file = if max_file_size <= 0 {
        true
    } else {
        image.file_size as f64 >= max_file_size as f64 * GROUP_SIMILARITY_ORIGINAL_FILE_RATIO
    };

    high_resolution || large_file
}

fn is_possible_lower_quality_candidate(candidate: &ImageSummary, anchor: &ImageSummary) -> bool {
    let lower_resolution = (pixel_count(candidate) as f64)
        < pixel_count(anchor) as f64 * GROUP_SIMILARITY_LOWER_PIXEL_RATIO;
    let smaller_file =
        (candidate.file_size as f64) < anchor.file_size as f64 * GROUP_SIMILARITY_LOWER_FILE_RATIO;

    lower_resolution || smaller_file
}

fn build_group_similarity_plan(images: &[ImageSummary]) -> Vec<GroupSimilarityPlanPair> {
    if images.len() < 2 {
        return Vec::new();
    }

    let max_pixels = images.iter().map(pixel_count).max().unwrap_or(0);
    let max_file_size = images
        .iter()
        .map(|image| image.file_size)
        .max()
        .unwrap_or(0);
    let anchor_indices = images
        .iter()
        .enumerate()
        .filter_map(|(index, image)| {
            is_probable_original_anchor(image, max_pixels, max_file_size).then_some(index)
        })
        .collect::<Vec<_>>();

    let mut planned_pairs = Vec::new();
    let mut planned_pair_keys = HashSet::new();

    for (candidate_index, candidate) in images.iter().enumerate() {
        let mut anchors = anchor_indices
            .iter()
            .copied()
            .filter(|anchor_index| *anchor_index != candidate_index)
            .filter(|anchor_index| {
                let anchor = &images[*anchor_index];
                is_possible_lower_quality_candidate(candidate, anchor)
                    && passes_group_similarity_phash_pruning(candidate, anchor)
            })
            .map(|anchor_index| {
                let anchor = &images[anchor_index];
                let phash_distance = phash_distance_between(candidate, anchor).unwrap_or(i32::MAX);
                (anchor_index, phash_distance)
            })
            .collect::<Vec<_>>();

        anchors.sort_by(
            |(left_index, left_distance), (right_index, right_distance)| {
                left_distance
                    .cmp(right_distance)
                    .then_with(|| {
                        compare_image_quality(&images[*right_index], &images[*left_index])
                    })
                    .then(
                        images[*left_index]
                            .relative_path
                            .cmp(&images[*right_index].relative_path),
                    )
            },
        );

        for (anchor_index, _) in anchors
            .into_iter()
            .take(GROUP_SIMILARITY_MAX_CANDIDATES_PER_IMAGE)
        {
            let (left_index, right_index) = if candidate_index <= anchor_index {
                (candidate_index, anchor_index)
            } else {
                (anchor_index, candidate_index)
            };

            if planned_pair_keys.insert((left_index, right_index)) {
                planned_pairs.push(GroupSimilarityPlanPair {
                    left_index,
                    right_index,
                });
            }
        }
    }

    planned_pairs
}

fn group_similarity_progress(
    request_id: &str,
    status: &str,
    phase: &str,
    total_pairs: usize,
    processed_pairs: usize,
    total_images: usize,
    processed_images: usize,
    current_left: Option<&ImageSummary>,
    current_right: Option<&ImageSummary>,
    current_image: Option<&ImageSummary>,
    cache_hits: usize,
    image_cache_hits: usize,
    computed_pairs: usize,
    skipped_pairs: usize,
) -> GroupSimilarityProgress {
    GroupSimilarityProgress {
        request_id: request_id.to_string(),
        status: status.to_string(),
        phase: phase.to_string(),
        total_pairs,
        processed_pairs,
        total_images,
        processed_images,
        current_left_image_id: current_left.map(|image| image.id),
        current_right_image_id: current_right.map(|image| image.id),
        current_left_file_name: current_left.map(|image| file_name_from_path(&image.relative_path)),
        current_right_file_name: current_right
            .map(|image| file_name_from_path(&image.relative_path)),
        current_image_id: current_image.map(|image| image.id),
        current_image_file_name: current_image
            .map(|image| file_name_from_path(&image.relative_path)),
        cache_hits,
        image_cache_hits,
        computed_pairs,
        skipped_pairs,
    }
}

fn emit_group_similarity_progress(window: &Window, progress: GroupSimilarityProgress) {
    let _ = window.emit("group-similarity-progress", &progress);
}

fn build_group_similarity_image_cache_jobs(
    run_id: &str,
    images: &[ImageSummary],
    plan: &[GroupSimilarityPlanPair],
) -> Vec<GroupSimilarityImageCacheJob> {
    use crate::core::ssim::compute::SsimComputer;

    let mut jobs = Vec::new();
    let mut seen_keys = HashSet::new();

    for pair in plan {
        let left = &images[pair.left_index];
        let right = &images[pair.right_index];
        let (target_width, target_height) = SsimComputer::pair_target_dimensions(
            (left.width, left.height),
            (right.width, right.height),
        );

        for image_index in [pair.left_index, pair.right_index] {
            let image = &images[image_index];
            let cache_key = similarity_image_cache_key(run_id, image, target_width, target_height);
            if seen_keys.insert(cache_key) {
                jobs.push(GroupSimilarityImageCacheJob {
                    image_index,
                    target_width,
                    target_height,
                });
            }
        }
    }

    jobs
}

fn has_cached_similarity_image(
    run_id: &str,
    image: &ImageSummary,
    target_width: u32,
    target_height: u32,
) -> bool {
    let cache_key = similarity_image_cache_key(run_id, image, target_width, target_height);
    group_similarity_image_cache()
        .lock()
        .unwrap()
        .contains_key(&cache_key)
}

fn prepare_similarity_image(
    run_id: &str,
    image: &ImageSummary,
    target_width: u32,
    target_height: u32,
) -> Result<Arc<CachedSimilarityImage>> {
    use crate::core::ssim::compute::SsimComputer;

    let cache_key = similarity_image_cache_key(run_id, image, target_width, target_height);
    if let Some(cached_image) = group_similarity_image_cache()
        .lock()
        .unwrap()
        .get(&cache_key)
        .cloned()
    {
        return Ok(cached_image);
    }

    let source_image = image::open(Path::new(&image.file_path))?;
    let gray_image = SsimComputer::prepare_image(&source_image, target_width, target_height)?;
    let cached_image = Arc::new(CachedSimilarityImage { gray: gray_image });

    let mut cache = group_similarity_image_cache().lock().unwrap();
    insert_similarity_image_cache_entry(
        &mut cache,
        cache_key,
        Arc::clone(&cached_image),
        GROUP_SIMILARITY_IMAGE_CACHE_BYTE_BUDGET,
    );

    Ok(cached_image)
}

fn compute_similarity_from_cached_images(
    left: &CachedSimilarityImage,
    right: &CachedSimilarityImage,
) -> Result<f64> {
    if left.gray.dimensions() != right.gray.dimensions() {
        return Err(AppError::SsimComputation("图片尺寸不匹配".to_string()));
    }

    use crate::core::ssim::compute::SsimComputer;

    SsimComputer::compute_gray(&left.gray, &right.gray)
}

fn compute_group_similarity_pair(
    run_id: &str,
    left: &ImageSummary,
    right: &ImageSummary,
) -> GroupSimilarityScore {
    use crate::core::ssim::compute::SsimComputer;

    let (target_width, target_height) = SsimComputer::pair_target_dimensions(
        (left.width, left.height),
        (right.width, right.height),
    );

    match (
        prepare_similarity_image(run_id, left, target_width, target_height),
        prepare_similarity_image(run_id, right, target_width, target_height),
    ) {
        (Ok(large_image), Ok(small_image)) => {
            match compute_similarity_from_cached_images(&large_image, &small_image) {
                Ok(ssim_score) => GroupSimilarityScore {
                    left_image_id: left.id,
                    right_image_id: right.id,
                    ssim_score: Some(ssim_score),
                    error_message: None,
                },
                Err(error) => GroupSimilarityScore {
                    left_image_id: left.id,
                    right_image_id: right.id,
                    ssim_score: None,
                    error_message: Some(error.to_string()),
                },
            }
        }
        (Err(error), _) | (_, Err(error)) => GroupSimilarityScore {
            left_image_id: left.id,
            right_image_id: right.id,
            ssim_score: None,
            error_message: Some(error.to_string()),
        },
    }
}

fn compute_group_similarity_scores(
    run_id: &str,
    images: &[ImageSummary],
    request_id: &str,
    window: Option<&Window>,
) -> Result<Vec<GroupSimilarityScore>> {
    let mut current_images = images.to_vec();
    refresh_current_file_fingerprints(&mut current_images)?;
    let images = current_images.as_slice();
    let full_pair_count = images.len().saturating_mul(images.len().saturating_sub(1)) / 2;
    let plan = build_group_similarity_plan(images);
    let total_pairs = plan.len();
    let skipped_pairs = full_pair_count.saturating_sub(total_pairs);
    let result_cache_key = group_similarity_result_cache_key(run_id, images);

    if let Some(window) = window {
        emit_group_similarity_progress(
            window,
            group_similarity_progress(
                request_id,
                "started",
                "planning",
                total_pairs,
                0,
                0,
                0,
                None,
                None,
                None,
                0,
                0,
                0,
                skipped_pairs,
            ),
        );
    }

    if let Some(cached_scores) = group_similarity_result_cache()
        .lock()
        .unwrap()
        .get(&result_cache_key)
        .cloned()
    {
        if let Some(window) = window {
            emit_group_similarity_progress(
                window,
                group_similarity_progress(
                    request_id,
                    "completed",
                    "completed",
                    total_pairs,
                    total_pairs,
                    0,
                    0,
                    None,
                    None,
                    None,
                    cached_scores.len(),
                    0,
                    0,
                    skipped_pairs,
                ),
            );
        }
        return Ok(cached_scores);
    }

    let image_cache_jobs = build_group_similarity_image_cache_jobs(run_id, images, &plan);
    let total_cache_images = image_cache_jobs.len();

    if let Some(window) = window {
        emit_group_similarity_progress(
            window,
            group_similarity_progress(
                request_id,
                "running",
                "caching",
                total_pairs,
                0,
                total_cache_images,
                0,
                None,
                None,
                None,
                0,
                0,
                0,
                skipped_pairs,
            ),
        );
    }

    let processed_cache_images = AtomicUsize::new(0);
    let image_cache_hits = AtomicUsize::new(0);
    algorithm_pool().install(|| {
        image_cache_jobs.par_iter().for_each(|job| {
            let image = &images[job.image_index];
            let was_cached =
                has_cached_similarity_image(run_id, image, job.target_width, job.target_height);
            let _ = prepare_similarity_image(run_id, image, job.target_width, job.target_height);
            let processed = processed_cache_images.fetch_add(1, AtomicOrdering::SeqCst) + 1;
            let hits = if was_cached {
                image_cache_hits.fetch_add(1, AtomicOrdering::SeqCst) + 1
            } else {
                image_cache_hits.load(AtomicOrdering::SeqCst)
            };
            if let Some(window) = window {
                emit_group_similarity_progress(
                    window,
                    group_similarity_progress(
                        request_id,
                        "running",
                        "caching",
                        total_pairs,
                        0,
                        total_cache_images,
                        processed,
                        None,
                        None,
                        Some(image),
                        0,
                        hits,
                        0,
                        skipped_pairs,
                    ),
                );
            }
        });
    });
    let processed_cache_images = processed_cache_images.load(AtomicOrdering::SeqCst);
    let image_cache_hits = image_cache_hits.load(AtomicOrdering::SeqCst);

    let processed_pairs = AtomicUsize::new(0);
    let cache_hits = AtomicUsize::new(0);
    let computed_pairs = AtomicUsize::new(0);
    let progress_window = window.cloned();

    let scores = algorithm_pool().install(|| {
        plan.par_iter()
            .map(|pair| {
                let left = &images[pair.left_index];
                let right = &images[pair.right_index];
                let cache_key = group_similarity_cache_key(run_id, left, right);

                if let Some(cached_score) = group_similarity_cache()
                    .lock()
                    .unwrap()
                    .get(&cache_key)
                    .cloned()
                {
                    let processed = processed_pairs.fetch_add(1, AtomicOrdering::SeqCst) + 1;
                    let hits = cache_hits.fetch_add(1, AtomicOrdering::SeqCst) + 1;
                    if let Some(window) = &progress_window {
                        emit_group_similarity_progress(
                            window,
                            group_similarity_progress(
                                request_id,
                                "running",
                                "comparing",
                                total_pairs,
                                processed,
                                total_cache_images,
                                processed_cache_images,
                                Some(left),
                                Some(right),
                                None,
                                hits,
                                image_cache_hits,
                                computed_pairs.load(AtomicOrdering::SeqCst),
                                skipped_pairs,
                            ),
                        );
                    }
                    return cached_score;
                }

                if let Some(window) = &progress_window {
                    emit_group_similarity_progress(
                        window,
                        group_similarity_progress(
                            request_id,
                            "running",
                            "comparing",
                            total_pairs,
                            processed_pairs.load(AtomicOrdering::SeqCst),
                            total_cache_images,
                            processed_cache_images,
                            Some(left),
                            Some(right),
                            None,
                            cache_hits.load(AtomicOrdering::SeqCst),
                            image_cache_hits,
                            computed_pairs.load(AtomicOrdering::SeqCst),
                            skipped_pairs,
                        ),
                    );
                }

                let score = compute_group_similarity_pair(run_id, left, right);

                let processed = processed_pairs.fetch_add(1, AtomicOrdering::SeqCst) + 1;
                let computed = computed_pairs.fetch_add(1, AtomicOrdering::SeqCst) + 1;

                if let Some(window) = &progress_window {
                    emit_group_similarity_progress(
                        window,
                        group_similarity_progress(
                            request_id,
                            "running",
                            "comparing",
                            total_pairs,
                            processed,
                            total_cache_images,
                            processed_cache_images,
                            Some(left),
                            Some(right),
                            None,
                            cache_hits.load(AtomicOrdering::SeqCst),
                            image_cache_hits,
                            computed,
                            skipped_pairs,
                        ),
                    );
                }

                score
            })
            .collect::<Vec<_>>()
    });

    validate_current_file_fingerprints(images)?;
    {
        let mut pair_cache = group_similarity_cache().lock().unwrap();
        for (pair, score) in plan.iter().zip(&scores) {
            let left = &images[pair.left_index];
            let right = &images[pair.right_index];
            pair_cache.insert(
                group_similarity_cache_key(run_id, left, right),
                score.clone(),
            );
        }
    }

    let mut result_cache = group_similarity_result_cache().lock().unwrap();
    result_cache.insert(result_cache_key.clone(), scores.clone());
    if result_cache.len() > GROUP_SIMILARITY_RESULT_CACHE_LIMIT {
        if let Some(oldest_key) = result_cache
            .keys()
            .find(|key| *key != &result_cache_key)
            .cloned()
        {
            result_cache.remove(&oldest_key);
        }
    }

    if let Some(window) = window {
        emit_group_similarity_progress(
            window,
            group_similarity_progress(
                request_id,
                "completed",
                "completed",
                total_pairs,
                processed_pairs.load(AtomicOrdering::SeqCst),
                total_cache_images,
                processed_cache_images,
                None,
                None,
                None,
                cache_hits.load(AtomicOrdering::SeqCst),
                image_cache_hits,
                computed_pairs.load(AtomicOrdering::SeqCst),
                skipped_pairs,
            ),
        );
    }

    Ok(scores)
}

fn compare_image_quality(left: &ImageSummary, right: &ImageSummary) -> std::cmp::Ordering {
    pixel_count(left)
        .cmp(&pixel_count(right))
        .then(left.file_size.cmp(&right.file_size))
        .then(right.relative_path.cmp(&left.relative_path))
}

fn run_config_from_settings(settings: &crate::db::models::Settings) -> RunConfig {
    RunConfig {
        phash_max_distance: settings.default_phash_max_distance,
        compressed_ssim_threshold: settings.default_compressed_ssim_threshold,
        variant_review_lower_bound: settings.default_variant_review_lower_bound,
        aspect_ratio_tolerance: settings.default_aspect_ratio_tolerance,
        primary_match_tie_threshold: 0.001,
        supported_formats: vec![
            "jpg".to_string(),
            "jpeg".to_string(),
            "png".to_string(),
            "gif".to_string(),
            "bmp".to_string(),
            "webp".to_string(),
        ],
        follow_symlinks: false,
        exclude_patterns: Some(vec![
            ".recycle".to_string(),
            ".git".to_string(),
            "node_modules".to_string(),
        ]),
        max_workers: ALGORITHM_WORKER_COUNT as i32,
    }
}

/// 开始多文件夹对比扫描
#[tauri::command]
pub async fn start_multi_compare(
    request: MultiCompareRequest,
    window: Window,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<String> {
    // 1. 生成 run_id
    let timestamp = Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let run_id = format!(
        "{}-{}",
        timestamp,
        uuid::Uuid::new_v4().to_string()[..8].to_string()
    );

    // 2. 准备配置
    let app_version = env!("CARGO_PKG_VERSION").to_string();
    let algorithm_profile_id = CURRENT_ALGORITHM_PROFILE_ID.to_string();

    let baseline_alias = "A".to_string();
    let comparison_aliases: Vec<String> = (0..request.comparison_paths.len())
        .map(|i| format!("{}", (b'B' + i as u8) as char))
        .collect();

    let internal_compare_paths: Vec<String> = request
        .directory_options
        .as_ref()
        .map(|options| {
            options
                .iter()
                .filter(|option| option.compare_within)
                .map(|option| option.path.clone())
                .collect()
        })
        .unwrap_or_default();
    let allow_internal_same_root = internal_compare_paths
        .iter()
        .any(|path| path == &request.baseline_path);

    // 新任务保存完整配置快照，后续设置变化不影响历史结果。
    let config = {
        let repo_lock = repo.lock().unwrap();
        run_config_from_settings(&repo_lock.load_settings()?)
    };

    // 3. 创建运行记录
    {
        let repo_lock = repo.lock().unwrap();
        repo_lock.create_run(
            &run_id,
            &app_version,
            &algorithm_profile_id,
            &request.baseline_path,
            &baseline_alias,
            &request.comparison_paths,
            &comparison_aliases,
            &config,
        )?;
    }

    // 4. 启动后台任务执行工作流
    let run_id_clone = run_id.clone();
    let baseline_path = PathBuf::from(request.baseline_path);
    let comparison_paths: Vec<PathBuf> = request
        .comparison_paths
        .into_iter()
        .map(PathBuf::from)
        .collect();

    let window_clone = window.clone();
    let repo_clone_outer = Arc::clone(&repo);

    tauri::async_runtime::spawn(async move {
        use crate::core::workflow::{WorkflowEngine, WorkflowOptions};

        let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel(100);

        // 转发进度事件到前端
        let window_clone_inner = window_clone.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(event) = progress_rx.recv().await {
                let _ = window_clone_inner.emit("scan-progress", &event);
            }
        });

        // 执行工作流
        let engine = WorkflowEngine::new(repo_clone_outer.clone());
        let result = engine
            .execute_comparison_with_options(
                &run_id_clone,
                baseline_path,
                comparison_paths,
                progress_tx,
                WorkflowOptions {
                    allow_internal_same_root,
                },
            )
            .await;

        // 处理结果
        if let Err(e) = result {
            eprintln!("工作流执行失败: {:?}", e);
            let _ = {
                let repo_lock = repo_clone_outer.lock().unwrap();
                repo_lock.update_run_status(&run_id_clone, RunStatus::Failed)
            };
        }

        // 发送完成事件
        let _ = window_clone.emit("comparison-complete", &run_id_clone);
    });

    Ok(run_id)
}

/// 获取对比统计
#[tauri::command]
pub async fn get_comparison_stats(
    run_id: String,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<ComparisonStats> {
    let repo_lock = repo.lock().unwrap();
    repo_lock.get_analysis_stats(&run_id)
}

/// 获取分类结果列表
#[tauri::command]
pub async fn get_comparison_results(
    run_id: String,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<Vec<ComparisonResultRow>> {
    let repo_lock = repo.lock().unwrap();
    let run = repo_lock
        .get_run(&run_id)?
        .ok_or_else(|| AppError::NotFound(format!("运行不存在: {}", run_id)))?;
    ensure_current_algorithm_profile(&run.algorithm_profile_id)?;
    read_comparison_results(&repo_lock, &run_id)
}

/// 获取感知哈希粗分组和组内结构相似性决策信息
#[tauri::command]
pub async fn get_comparison_groups(
    run_id: String,
    grouping_distance: Option<i32>,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<Vec<ComparisonGroup>> {
    let repo_lock = repo.lock().unwrap();
    read_comparison_groups(&repo_lock, &run_id, grouping_distance)
}

/// 获取当前组内图片两两相似度，用于前端做“缩略图挂到最像原图”的交叉验证
#[tauri::command]
pub async fn get_group_similarity_scores(
    run_id: String,
    image_ids: Vec<i64>,
    request_id: String,
    window: Window,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<Vec<GroupSimilarityScore>> {
    let images = {
        let repo_lock = repo.lock().unwrap();
        let run = repo_lock
            .get_run(&run_id)?
            .ok_or_else(|| AppError::NotFound(format!("运行不存在: {}", run_id)))?;
        ensure_current_algorithm_profile(&run.algorithm_profile_id)?;
        read_images_by_ids(&repo_lock, &run_id, &image_ids)?
    };

    tauri::async_runtime::spawn_blocking(move || {
        compute_group_similarity_scores(&run_id, &images, &request_id, Some(&window))
    })
    .await
    .map_err(|error| AppError::Internal(format!("组内 SSIM 任务执行失败: {error}")))?
}

/// 获取运行状态
#[tauri::command]
pub async fn get_run_status(
    run_id: String,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<RunStatusResponse> {
    let repo_lock = repo.lock().unwrap();
    read_run_status(&repo_lock, &run_id)
}

/// 获取最近的对比历史
#[tauri::command]
pub async fn list_comparison_runs(
    limit: Option<i64>,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<Vec<ComparisonRunHistoryItem>> {
    let repo_lock = repo.lock().unwrap();
    read_comparison_run_history(&repo_lock, limit.unwrap_or(20))
}

/// 删除历史任务数据库记录，不删除任何图片文件
#[tauri::command]
pub async fn delete_comparison_run(
    run_id: String,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<()> {
    let repo_lock = repo.lock().unwrap();
    delete_comparison_run_records(&repo_lock, &run_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::phash::PHASH_ALGORITHM_VERSION;
    use crate::core::ssim::standard::StandardSsim;
    use crate::db::{repository::RunConfig, schema};
    use image::{DynamicImage, GrayImage, Luma};
    use rusqlite::Connection;

    fn create_test_repo_with_profile(algorithm_profile_id: &str) -> Repository {
        let conn = Connection::open_in_memory().unwrap();
        schema::initialize_database(&conn).unwrap();
        let repo = Repository::new(conn);

        repo.create_run(
            "run-1",
            "0.1.0",
            algorithm_profile_id,
            "D:/baseline",
            "A",
            &["D:/comparison".to_string()],
            &["B".to_string()],
            &RunConfig::default(),
        )
        .unwrap();

        repo
    }

    fn create_test_repo() -> Repository {
        create_test_repo_with_profile(CURRENT_ALGORITHM_PROFILE_ID)
    }

    #[test]
    fn comparison_groups_reject_legacy_algorithm_profiles() {
        let repo = create_test_repo_with_profile("imagekeeper-v2-mse-ssim-512");

        let error = read_comparison_groups(&repo, "run-1", None).unwrap_err();

        assert!(
            error.to_string().contains("旧算法") && error.to_string().contains("重新运行"),
            "旧任务不能把历史 pHash/SSIM 当作当前标准值使用: {error}"
        );
    }

    #[test]
    fn read_run_status_returns_terminal_status_for_existing_run() {
        let repo = create_test_repo();
        repo.update_run_status("run-1", RunStatus::AnalysisComplete)
            .unwrap();

        let response = read_run_status(&repo, "run-1").unwrap();

        assert_eq!(response.run_id, "run-1");
        assert_eq!(response.status, RunStatus::AnalysisComplete);
        assert!(response.completed_at.is_some());
    }

    #[test]
    fn read_run_status_errors_for_missing_run() {
        let repo = create_test_repo();

        let result = read_run_status(&repo, "missing-run");

        assert!(result.is_err());
    }

    #[test]
    fn read_comparison_run_history_returns_recent_runs_with_summary() {
        use crate::db::models::{AnalysisType, FolderRole};
        use crate::db::repository::{AnalysisResultInsert, ImageInsert};

        let repo = create_test_repo();
        let baseline_folder = repo
            .create_folder("run-1", "D:/baseline", "A", FolderRole::Baseline)
            .unwrap();
        let comparison_folder = repo
            .create_folder("run-1", "D:/comparison", "B", FolderRole::Comparison)
            .unwrap();

        let baseline_id = repo
            .insert_image(&ImageInsert {
                run_id: "run-1".to_string(),
                folder_id: baseline_folder,
                source_role: FolderRole::Baseline,
                file_path: "D:/baseline/a.png".to_string(),
                relative_path: "a.png".to_string(),
                file_size: 100,
                file_modified_at: 0,
                width: 10,
                height: 10,
                format: "png".to_string(),
                aspect_ratio: 1.0,
                frame_count: 1,
                frame_strategy: "first".to_string(),
            })
            .unwrap();
        let comparison_id = repo
            .insert_image(&ImageInsert {
                run_id: "run-1".to_string(),
                folder_id: comparison_folder,
                source_role: FolderRole::Comparison,
                file_path: "D:/comparison/b.png".to_string(),
                relative_path: "b.png".to_string(),
                file_size: 90,
                file_modified_at: 0,
                width: 10,
                height: 10,
                format: "png".to_string(),
                aspect_ratio: 1.0,
                frame_count: 1,
                frame_strategy: "first".to_string(),
            })
            .unwrap();

        repo.update_image_hash(baseline_id, "hash-a", "phash-a", PHASH_ALGORITHM_VERSION)
            .unwrap();
        repo.update_image_hash(comparison_id, "hash-b", "phash-b", PHASH_ALGORITHM_VERSION)
            .unwrap();
        repo.insert_analysis_result(&AnalysisResultInsert {
            run_id: "run-1".to_string(),
            comparison_image_id: comparison_id,
            analysis_type: AnalysisType::LikelyCompressed,
            primary_match_image_id: Some(baseline_id),
            all_candidate_ids: Some(vec![baseline_id]),
            candidate_truncated: false,
            phash_distance: Some(1),
            ssim_score: Some(0.998),
            size_ratio: Some(0.9),
            resolution_ratio: Some(1.0),
            aspect_diff: Some(0.0),
            direction_smaller_resolution: false,
            direction_smaller_filesize: true,
            algorithm_profile_id: "test-profile".to_string(),
            analysis_metadata: None,
        })
        .unwrap();
        repo.update_run_status("run-1", RunStatus::AnalysisComplete)
            .unwrap();

        let history = read_comparison_run_history(&repo, 10).unwrap();

        assert_eq!(history.len(), 1);
        assert_eq!(history[0].run_id, "run-1");
        assert_eq!(history[0].status, RunStatus::AnalysisComplete);
        assert_eq!(history[0].baseline_root_path, "D:/baseline");
        assert_eq!(
            history[0].comparison_root_paths,
            vec!["D:/comparison".to_string()]
        );
        assert_eq!(history[0].baseline_total, 1);
        assert_eq!(history[0].comparison_total, 1);
        assert_eq!(history[0].result_count, 1);
        assert!(history[0].completed_at.is_some());
    }

    #[test]
    fn delete_comparison_run_removes_database_rows_without_touching_file_path() {
        use crate::db::models::{AnalysisType, FolderRole};
        use crate::db::repository::{AnalysisResultInsert, ImageInsert};

        let repo = create_test_repo();
        let baseline_folder = repo
            .create_folder("run-1", "D:/baseline", "A", FolderRole::Baseline)
            .unwrap();
        let comparison_folder = repo
            .create_folder("run-1", "D:/comparison", "B", FolderRole::Comparison)
            .unwrap();
        let untouched_file = std::env::temp_dir().join(format!(
            "imagekeeper-history-delete-{}.png",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&untouched_file, b"keep me").unwrap();

        let baseline_id = repo
            .insert_image(&ImageInsert {
                run_id: "run-1".to_string(),
                folder_id: baseline_folder,
                source_role: FolderRole::Baseline,
                file_path: "D:/baseline/a.png".to_string(),
                relative_path: "a.png".to_string(),
                file_size: 100,
                file_modified_at: 0,
                width: 10,
                height: 10,
                format: "png".to_string(),
                aspect_ratio: 1.0,
                frame_count: 1,
                frame_strategy: "first".to_string(),
            })
            .unwrap();
        let comparison_id = repo
            .insert_image(&ImageInsert {
                run_id: "run-1".to_string(),
                folder_id: comparison_folder,
                source_role: FolderRole::Comparison,
                file_path: untouched_file.to_string_lossy().to_string(),
                relative_path: "b.png".to_string(),
                file_size: 90,
                file_modified_at: 0,
                width: 10,
                height: 10,
                format: "png".to_string(),
                aspect_ratio: 1.0,
                frame_count: 1,
                frame_strategy: "first".to_string(),
            })
            .unwrap();

        repo.update_image_hash(baseline_id, "hash-a", "phash-a", PHASH_ALGORITHM_VERSION)
            .unwrap();
        repo.update_image_hash(comparison_id, "hash-b", "phash-b", PHASH_ALGORITHM_VERSION)
            .unwrap();
        repo.insert_analysis_result(&AnalysisResultInsert {
            run_id: "run-1".to_string(),
            comparison_image_id: comparison_id,
            analysis_type: AnalysisType::LikelyCompressed,
            primary_match_image_id: Some(baseline_id),
            all_candidate_ids: Some(vec![baseline_id]),
            candidate_truncated: false,
            phash_distance: Some(1),
            ssim_score: Some(0.998),
            size_ratio: Some(0.9),
            resolution_ratio: Some(1.0),
            aspect_diff: Some(0.0),
            direction_smaller_resolution: false,
            direction_smaller_filesize: true,
            algorithm_profile_id: "test-profile".to_string(),
            analysis_metadata: None,
        })
        .unwrap();

        delete_comparison_run_records(&repo, "run-1").unwrap();

        for table in ["runs", "folders", "images", "analysis_results"] {
            let count: i64 = repo
                .conn()
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE run_id = ?1"),
                    ["run-1"],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 0, "{table} 应删除 run-1 的数据库记录");
        }
        assert!(untouched_file.exists(), "删除历史任务不能删除真实图片文件");
        std::fs::remove_file(untouched_file).unwrap();
    }

    #[test]
    fn read_comparison_results_returns_rows_with_image_paths() {
        use crate::db::models::{AnalysisType, FolderRole};
        use crate::db::repository::{AnalysisResultInsert, ImageInsert};

        let repo = create_test_repo();
        let baseline_folder = repo
            .create_folder("run-1", "D:/baseline", "A", FolderRole::Baseline)
            .unwrap();
        let comparison_folder = repo
            .create_folder("run-1", "D:/comparison", "B", FolderRole::Comparison)
            .unwrap();

        let baseline_id = repo
            .insert_image(&ImageInsert {
                run_id: "run-1".to_string(),
                folder_id: baseline_folder,
                source_role: FolderRole::Baseline,
                file_path: "D:/baseline/a.png".to_string(),
                relative_path: "a.png".to_string(),
                file_size: 100,
                file_modified_at: 0,
                width: 10,
                height: 10,
                format: "png".to_string(),
                aspect_ratio: 1.0,
                frame_count: 1,
                frame_strategy: "first".to_string(),
            })
            .unwrap();
        let comparison_id = repo
            .insert_image(&ImageInsert {
                run_id: "run-1".to_string(),
                folder_id: comparison_folder,
                source_role: FolderRole::Comparison,
                file_path: "D:/comparison/b.png".to_string(),
                relative_path: "b.png".to_string(),
                file_size: 90,
                file_modified_at: 0,
                width: 10,
                height: 10,
                format: "png".to_string(),
                aspect_ratio: 1.0,
                frame_count: 1,
                frame_strategy: "first".to_string(),
            })
            .unwrap();

        repo.update_image_hash(baseline_id, "hash-a", "phash-a", PHASH_ALGORITHM_VERSION)
            .unwrap();
        repo.update_image_hash(comparison_id, "hash-b", "phash-b", PHASH_ALGORITHM_VERSION)
            .unwrap();

        repo.insert_analysis_result(&AnalysisResultInsert {
            run_id: "run-1".to_string(),
            comparison_image_id: comparison_id,
            analysis_type: AnalysisType::Inconclusive,
            primary_match_image_id: Some(baseline_id),
            all_candidate_ids: Some(vec![baseline_id]),
            candidate_truncated: false,
            phash_distance: Some(2),
            ssim_score: Some(0.99),
            size_ratio: Some(0.9),
            resolution_ratio: Some(1.0),
            aspect_diff: Some(0.0),
            direction_smaller_resolution: false,
            direction_smaller_filesize: true,
            algorithm_profile_id: "test-profile".to_string(),
            analysis_metadata: None,
        })
        .unwrap();

        let rows = read_comparison_results(&repo, "run-1").unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].analysis_type, AnalysisType::Inconclusive);
        assert_eq!(rows[0].comparison_path, "D:/comparison/b.png");
        assert_eq!(
            rows[0].primary_match_path.as_deref(),
            Some("D:/baseline/a.png")
        );
        assert_eq!(rows[0].comparison_relative_path, "b.png");
    }

    #[test]
    fn read_comparison_groups_connects_transitive_phash_matches() {
        use crate::db::models::FolderRole;
        use crate::db::repository::ImageInsert;

        let repo = create_test_repo();
        let folder_id = repo
            .create_folder("run-1", "D:/baseline", "A", FolderRole::Baseline)
            .unwrap();

        let image_specs = [
            ("D:/baseline/a.png", "a.png", "0000000000000000"),
            ("D:/baseline/b.png", "b.png", "0000000000000001"),
            ("D:/baseline/c.png", "c.png", "0000000000000003"),
            ("D:/baseline/d.png", "d.png", "ffffffffffffffff"),
        ];

        for (idx, (file_path, relative_path, phash)) in image_specs.iter().enumerate() {
            let image_id = repo
                .insert_image(&ImageInsert {
                    run_id: "run-1".to_string(),
                    folder_id,
                    source_role: FolderRole::Baseline,
                    file_path: file_path.to_string(),
                    relative_path: relative_path.to_string(),
                    file_size: 100 + idx as i64,
                    file_modified_at: 0,
                    width: 10,
                    height: 10,
                    format: "png".to_string(),
                    aspect_ratio: 1.0,
                    frame_count: 1,
                    frame_strategy: "first".to_string(),
                })
                .unwrap();

            repo.update_image_hash(
                image_id,
                &format!("hash-{idx}"),
                phash,
                PHASH_ALGORITHM_VERSION,
            )
            .unwrap();
        }

        let groups = read_comparison_groups(&repo, "run-1", None).unwrap();

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].member_count, 3);
        assert_eq!(
            groups[0]
                .members
                .iter()
                .map(|member| member.relative_path.as_str())
                .collect::<Vec<_>>(),
            vec!["a.png", "b.png", "c.png"]
        );
        assert_eq!(groups[1].member_count, 1);
        assert_eq!(groups[1].members[0].relative_path, "d.png");
    }

    #[test]
    fn read_comparison_groups_uses_optional_grouping_distance() {
        use crate::db::models::FolderRole;
        use crate::db::repository::ImageInsert;

        let repo = create_test_repo();
        let folder_id = repo
            .create_folder("run-1", "D:/baseline", "A", FolderRole::Baseline)
            .unwrap();

        let image_specs = [
            ("D:/baseline/a.png", "a.png", "0000000000000000"),
            ("D:/baseline/b.png", "b.png", "0000000000000001"),
        ];

        for (idx, (file_path, relative_path, phash)) in image_specs.iter().enumerate() {
            let image_id = repo
                .insert_image(&ImageInsert {
                    run_id: "run-1".to_string(),
                    folder_id,
                    source_role: FolderRole::Baseline,
                    file_path: file_path.to_string(),
                    relative_path: relative_path.to_string(),
                    file_size: 100 + idx as i64,
                    file_modified_at: 0,
                    width: 10,
                    height: 10,
                    format: "png".to_string(),
                    aspect_ratio: 1.0,
                    frame_count: 1,
                    frame_strategy: "first".to_string(),
                })
                .unwrap();

            repo.update_image_hash(
                image_id,
                &format!("hash-{idx}"),
                phash,
                PHASH_ALGORITHM_VERSION,
            )
            .unwrap();
        }

        let default_groups = read_comparison_groups(&repo, "run-1", None).unwrap();
        let strict_groups = read_comparison_groups(&repo, "run-1", Some(0)).unwrap();

        assert_eq!(default_groups.len(), 1);
        assert_eq!(default_groups[0].member_count, 2);
        assert_eq!(strict_groups.len(), 2);
        assert_eq!(strict_groups[0].member_count, 1);
        assert_eq!(strict_groups[1].member_count, 1);
    }

    fn image_summary_for_plan(
        id: i64,
        relative_path: &str,
        width: u32,
        height: u32,
        file_size: i64,
        phash: &str,
    ) -> ImageSummary {
        let file_path = format!("D:/images/{relative_path}");
        ImageSummary {
            id,
            canonical_file_path: file_path.clone(),
            file_path,
            relative_path: relative_path.to_string(),
            file_size,
            file_modified_at: 0,
            current_file_size: file_size.max(0) as u64,
            current_modified_ns: 0,
            width,
            height,
            phash: Some(phash.to_string()),
        }
    }

    #[test]
    fn build_group_similarity_plan_compares_thumbnails_with_probable_originals_only() {
        let images = vec![
            image_summary_for_plan(
                1,
                "142585056_p0.png",
                4000,
                3000,
                9_000_000,
                "0000000000000000",
            ),
            image_summary_for_plan(
                2,
                "142585056_p1.png",
                4000,
                3000,
                9_000_000,
                "0000000000000001",
            ),
            image_summary_for_plan(
                3,
                "142585056_p2.png",
                4000,
                3000,
                9_000_000,
                "0000000000000003",
            ),
            image_summary_for_plan(4, "photo_0.jpg", 1000, 750, 400_000, "0000000000000000"),
            image_summary_for_plan(5, "photo_1.jpg", 1000, 750, 400_000, "0000000000000001"),
            image_summary_for_plan(6, "photo_2.jpg", 1000, 750, 400_000, "0000000000000003"),
        ];

        let plan = build_group_similarity_plan(&images);
        let planned_pairs = plan
            .iter()
            .map(|pair| {
                let mut ids = [images[pair.left_index].id, images[pair.right_index].id];
                ids.sort();
                (ids[0], ids[1])
            })
            .collect::<Vec<_>>();

        assert_eq!(planned_pairs.len(), 9);
        for original_id in 1..=3 {
            for thumbnail_id in 4..=6 {
                assert!(
                    planned_pairs.contains(&(original_id, thumbnail_id)),
                    "缩略图 {thumbnail_id} 应该和可能的原图 {original_id} 做交叉验证"
                );
            }
        }
        assert!(!planned_pairs.contains(&(4, 5)), "缩略图之间不应互相比对");
    }

    #[test]
    fn build_group_similarity_plan_skips_far_phash_pairs() {
        let images = vec![
            image_summary_for_plan(1, "original.png", 4000, 3000, 9_000_000, "0000000000000000"),
            image_summary_for_plan(
                2,
                "unrelated_thumb.jpg",
                1000,
                750,
                400_000,
                "ffffffffffffffff",
            ),
        ];

        let plan = build_group_similarity_plan(&images);

        assert!(plan.is_empty(), "明显不是同一张图的组合应该被剪枝");
    }

    #[test]
    fn group_similarity_result_cache_key_is_stable_for_same_group_members() {
        let first_order = vec![
            image_summary_for_plan(3, "c.png", 3000, 2000, 3_000_000, "0000000000000003"),
            image_summary_for_plan(1, "a.png", 3000, 2000, 3_000_000, "0000000000000001"),
            image_summary_for_plan(2, "b.png", 1000, 667, 300_000, "0000000000000002"),
        ];
        let second_order = vec![
            first_order[1].clone(),
            first_order[2].clone(),
            first_order[0].clone(),
        ];

        assert_eq!(
            group_similarity_result_cache_key("run-1", &first_order),
            group_similarity_result_cache_key("run-1", &second_order)
        );
    }

    #[test]
    fn all_similarity_cache_layers_change_with_the_current_file_fingerprint() {
        let left = image_summary_for_plan(1, "a.png", 3000, 2000, 3_000_000, "0000000000000001");
        let right = image_summary_for_plan(2, "b.png", 1000, 667, 300_000, "0000000000000002");
        let first_pair_key = group_similarity_cache_key("run-1", &left, &right);
        let first_result_key =
            group_similarity_result_cache_key("run-1", &[left.clone(), right.clone()]);
        let first_image_key = similarity_image_cache_key("run-1", &left, 1000, 667);

        let mut changed_left = left.clone();
        changed_left.current_modified_ns = left.current_modified_ns.saturating_add(1);
        let second_pair_key = group_similarity_cache_key("run-1", &changed_left, &right);
        let second_result_key =
            group_similarity_result_cache_key("run-1", &[changed_left.clone(), right]);
        let second_image_key = similarity_image_cache_key("run-1", &changed_left, 1000, 667);

        assert_ne!(first_pair_key, second_pair_key);
        assert_ne!(first_result_key, second_result_key);
        assert_ne!(first_image_key, second_image_key);
    }

    #[test]
    fn group_similarity_rejects_a_file_changed_after_the_run() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("changed.png");
        std::fs::write(&path, b"before").unwrap();
        let original_metadata = std::fs::metadata(&path).unwrap();
        let original_modified_at = original_metadata
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let mut image = image_summary_for_plan(
            1,
            "changed.png",
            1,
            1,
            original_metadata.len() as i64,
            "0000000000000001",
        );
        image.file_path = path.to_string_lossy().into_owned();
        image.canonical_file_path = image.file_path.clone();
        image.file_modified_at = original_modified_at;

        std::fs::write(&path, b"after-with-a-different-size").unwrap();
        let error =
            refresh_current_file_fingerprints(std::slice::from_mut(&mut image)).unwrap_err();

        assert!(error.to_string().contains("重新运行任务"));
    }

    #[test]
    fn current_file_fingerprint_validation_rejects_late_changes() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("changed-during-comparison.png");
        std::fs::write(&path, b"before").unwrap();
        let metadata = std::fs::metadata(&path).unwrap();
        let modified_at = metadata
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let mut image = image_summary_for_plan(
            1,
            "changed-during-comparison.png",
            10,
            10,
            metadata.len() as i64,
            "0000000000000001",
        );
        image.file_path = path.to_string_lossy().into_owned();
        image.canonical_file_path = image.file_path.clone();
        image.file_modified_at = modified_at;
        refresh_current_file_fingerprints(std::slice::from_mut(&mut image)).unwrap();

        std::fs::write(&path, b"after-with-a-different-size").unwrap();

        let error = validate_current_file_fingerprints(std::slice::from_ref(&image)).unwrap_err();
        assert!(error.to_string().contains("计算期间已发生变化"));
    }

    #[test]
    fn saved_settings_are_snapshotted_into_new_run_config() {
        let settings = crate::db::models::Settings {
            default_compressed_ssim_threshold: 0.9825,
            default_variant_review_lower_bound: 0.72,
            default_phash_max_distance: 8,
            default_aspect_ratio_tolerance: 0.004,
            auto_preselect_exact_duplicates: false,
            max_candidate_per_image: 50,
        };

        let config = run_config_from_settings(&settings);

        assert_eq!(config.compressed_ssim_threshold, 0.9825);
        assert_eq!(config.variant_review_lower_bound, 0.72);
        assert_eq!(config.phash_max_distance, 8);
        assert_eq!(config.aspect_ratio_tolerance, 0.004);
        assert_eq!(config.max_workers, ALGORITHM_WORKER_COUNT as i32);
    }

    #[test]
    fn similarity_image_cache_key_includes_target_size() {
        let image = image_summary_for_plan(1, "a.png", 3000, 2000, 3_000_000, "0000000000000001");

        assert_ne!(
            similarity_image_cache_key("run-1", &image, 512, 341),
            similarity_image_cache_key("run-1", &image, 256, 171)
        );
    }

    #[test]
    fn similarity_image_cache_is_bounded_by_gray_pixel_bytes() {
        let first_key = similarity_image_cache_key(
            "run-1",
            &image_summary_for_plan(1, "a.png", 3, 2, 6, "0000000000000001"),
            3,
            2,
        );
        let second_key = similarity_image_cache_key(
            "run-1",
            &image_summary_for_plan(2, "b.png", 3, 2, 6, "0000000000000002"),
            3,
            2,
        );
        let mut cache = HashMap::new();
        let first = Arc::new(CachedSimilarityImage {
            gray: GrayImage::from_pixel(3, 2, Luma([10])),
        });
        let second = Arc::new(CachedSimilarityImage {
            gray: GrayImage::from_pixel(3, 2, Luma([20])),
        });

        insert_similarity_image_cache_entry(&mut cache, first_key.clone(), first, 10);
        insert_similarity_image_cache_entry(&mut cache, second_key.clone(), second, 10);

        assert!(!cache.contains_key(&first_key));
        assert!(cache.contains_key(&second_key));
        assert!(similarity_image_cache_bytes(&cache) <= 10);
    }

    #[test]
    fn group_similarity_matches_standard_similarity_algorithm() {
        let red = CachedSimilarityImage {
            gray: GrayImage::from_raw(2, 1, vec![76, 76]).unwrap(),
        };
        let green = CachedSimilarityImage {
            gray: GrayImage::from_raw(2, 1, vec![150, 150]).unwrap(),
        };
        let red_image = DynamicImage::ImageLuma8(red.gray.clone());
        let green_image = DynamicImage::ImageLuma8(green.gray.clone());
        let expected = StandardSsim::compute_owned(red_image, green_image).unwrap();

        let score = compute_similarity_from_cached_images(&red, &green).unwrap();

        assert!(
            (score - expected).abs() < 1e-12,
            "组内标准结构相似性应该复用统一算法，实际 {score}，期望 {expected}"
        );
    }

    #[test]
    fn cached_group_similarity_matches_the_standard_similarity_algorithm() {
        let left = CachedSimilarityImage {
            gray: GrayImage::from_raw(3, 2, vec![0, 32, 64, 96, 128, 160]).unwrap(),
        };
        let right = CachedSimilarityImage {
            gray: GrayImage::from_raw(3, 2, vec![12, 40, 70, 90, 140, 180]).unwrap(),
        };
        let expected = StandardSsim::compute_owned(
            DynamicImage::ImageLuma8(left.gray.clone()),
            DynamicImage::ImageLuma8(right.gray.clone()),
        )
        .unwrap();

        let score = compute_similarity_from_cached_images(&left, &right).unwrap();

        assert!((score - expected).abs() < 1e-12, "{score} != {expected}");
    }

    #[test]
    fn group_similarity_uses_the_smaller_images_full_resolution() {
        let images = vec![
            image_summary_for_plan(1, "large.png", 4000, 3000, 4_000_000, "0000000000000001"),
            image_summary_for_plan(2, "small.png", 1600, 1200, 1_000_000, "0000000000000002"),
        ];
        let plan = vec![GroupSimilarityPlanPair {
            left_index: 0,
            right_index: 1,
        }];

        let jobs = build_group_similarity_image_cache_jobs("run-1", &images, &plan);

        assert!(jobs
            .iter()
            .all(|job| { job.target_width == 1600 && job.target_height == 1200 }));
    }
}
