use crate::core::algorithm_profile::algorithm_pool;
use crate::core::image_features::{extract_image_features, phash_distance, ImageFeatures};
use crate::core::ssim::compute::SsimComputer;
use crate::error::{AppError, Result};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use walkdir::WalkDir;

const MAX_PHASH_DISTANCE: u32 = 16;
const STRONG_PHASH_DISTANCE: u32 = 10;
const VARIANT_SIMILARITY: f64 = 0.75;
const COMPRESSED_SIMILARITY: f64 = 0.995;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DifferenceReferenceInput {
    pub id: String,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DifferenceSearchRequest {
    pub session_id: String,
    pub references: Vec<DifferenceReferenceInput>,
    pub target_roots: Vec<String>,
    pub recursive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchClassification {
    Exact,
    CompressedOrReencoded,
    Variant,
    RelatedGroup,
    WeakCandidate,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceRelation {
    pub reference_id: String,
    pub reference_path: String,
    pub classification: MatchClassification,
    pub phash_distance: u32,
    pub similarity: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DifferenceMatchItem {
    pub file_path: String,
    pub file_name: String,
    pub source_root: String,
    pub relative_path: String,
    pub file_size: u64,
    pub modified_at: i64,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub blake3_hash: String,
    pub classification: MatchClassification,
    pub best_reference_id: String,
    pub relations: Vec<ReferenceRelation>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchFileError {
    pub file_path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DifferenceSearchResponse {
    pub session_id: String,
    pub scanned_file_count: usize,
    pub valid_reference_count: usize,
    pub matches: Vec<DifferenceMatchItem>,
    pub errors: Vec<SearchFileError>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DifferenceSearchPhase {
    Scanning,
    Extracting,
    Matching,
    Aggregating,
    Completed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DifferenceSearchProgress {
    pub session_id: String,
    pub phase: DifferenceSearchPhase,
    pub processed: usize,
    pub total: usize,
    pub current_file: Option<String>,
}

#[derive(Debug, Clone)]
struct CandidateRelation {
    target: ImageFeatures,
    source_root: String,
    reference_id: String,
    reference_path: String,
    classification: MatchClassification,
    phash_distance: u32,
    similarity: f64,
}

pub fn search_difference_images<P, C>(
    request: DifferenceSearchRequest,
    progress: P,
    is_cancelled: C,
) -> Result<DifferenceSearchResponse>
where
    P: Fn(DifferenceSearchProgress) + Sync,
    C: Fn() -> bool + Sync,
{
    if request.references.is_empty() {
        return Err(AppError::ValidationError("至少添加一张参考图".to_string()));
    }
    if request.target_roots.is_empty() {
        return Err(AppError::ValidationError(
            "至少添加一个搜索目录".to_string(),
        ));
    }

    let session_id = request.session_id.clone();
    let mut errors = Vec::new();
    let mut references = Vec::new();
    let reference_results = algorithm_pool().install(|| {
        request
            .references
            .par_iter()
            .map(|reference| {
                (
                    reference.clone(),
                    extract_image_features(Path::new(&reference.path)),
                )
            })
            .collect::<Vec<_>>()
    });
    for (reference, result) in reference_results {
        check_cancelled(&is_cancelled)?;
        match result {
            Ok(features) => references.push((reference, features)),
            Err(error) => errors.push(SearchFileError {
                file_path: reference.path.clone(),
                message: error.to_string(),
            }),
        }
    }
    if references.is_empty() {
        return Err(AppError::ValidationError("参考图均无法读取".to_string()));
    }

    progress(DifferenceSearchProgress {
        session_id: session_id.clone(),
        phase: DifferenceSearchPhase::Scanning,
        processed: 0,
        total: request.target_roots.len(),
        current_file: None,
    });
    let (target_files, discovery_errors) =
        discover_target_files(&request.target_roots, request.recursive)?;
    errors.extend(discovery_errors);
    check_cancelled(&is_cancelled)?;

    let extracted = AtomicUsize::new(0);
    let total_files = target_files.len();
    let feature_results: Vec<_> = algorithm_pool().install(|| {
        target_files
            .par_iter()
            .map(|(root, path)| {
                if is_cancelled() {
                    return (root.clone(), path.clone(), Err("搜索已取消".to_string()));
                }
                let result = extract_image_features(path).map_err(|error| error.to_string());
                let processed = extracted.fetch_add(1, Ordering::Relaxed) + 1;
                progress(DifferenceSearchProgress {
                    session_id: session_id.clone(),
                    phase: DifferenceSearchPhase::Extracting,
                    processed,
                    total: total_files,
                    current_file: Some(path.to_string_lossy().to_string()),
                });
                (root.clone(), path.clone(), result)
            })
            .collect()
    });
    check_cancelled(&is_cancelled)?;

    let mut targets = Vec::new();
    for (root, path, result) in feature_results {
        match result {
            Ok(features) => targets.push((root, features)),
            Err(message) if message != "搜索已取消" => errors.push(SearchFileError {
                file_path: path.to_string_lossy().to_string(),
                message,
            }),
            Err(_) => return Err(AppError::Other("搜索已取消".to_string())),
        }
    }

    let total_pairs = references.len() * targets.len();
    let processed_pairs = AtomicUsize::new(0);
    let relation_results = algorithm_pool().install(|| {
        targets
            .par_iter()
            .flat_map_iter(|(source_root, target)| {
                references.iter().map(|(reference_input, reference)| {
                    check_cancelled(&is_cancelled)?;
                    let distance =
                        phash_distance(&reference.phash, &target.phash).unwrap_or(u32::MAX);
                    let exact = reference.blake3_hash == target.blake3_hash;
                    let similarity = if exact {
                        Some(1.0)
                    } else if distance <= MAX_PHASH_DISTANCE {
                        compute_similarity(reference, target).ok()
                    } else {
                        None
                    };
                    let relation = classify_relation(reference, target, distance, similarity).map(
                        |classification| CandidateRelation {
                            target: target.clone(),
                            source_root: source_root.to_string_lossy().to_string(),
                            reference_id: reference_input.id.clone(),
                            reference_path: reference_input.path.clone(),
                            classification,
                            phash_distance: distance,
                            similarity: similarity.unwrap_or_default(),
                        },
                    );
                    let processed = processed_pairs.fetch_add(1, Ordering::Relaxed) + 1;
                    progress(DifferenceSearchProgress {
                        session_id: session_id.clone(),
                        phase: DifferenceSearchPhase::Matching,
                        processed,
                        total: total_pairs,
                        current_file: Some(target.file_path.clone()),
                    });
                    Ok(relation)
                })
            })
            .collect::<Vec<Result<Option<CandidateRelation>>>>()
    });
    let mut relations = Vec::new();
    for result in relation_results {
        if let Some(relation) = result? {
            relations.push(relation);
        }
    }

    progress(DifferenceSearchProgress {
        session_id: session_id.clone(),
        phase: DifferenceSearchPhase::Aggregating,
        processed: 0,
        total: relations.len(),
        current_file: None,
    });
    let matches = aggregate_relations(relations);
    progress(DifferenceSearchProgress {
        session_id: session_id.clone(),
        phase: DifferenceSearchPhase::Completed,
        processed: matches.len(),
        total: matches.len(),
        current_file: None,
    });

    Ok(DifferenceSearchResponse {
        session_id,
        scanned_file_count: targets.len(),
        valid_reference_count: references.len(),
        matches,
        errors,
    })
}

fn classify_relation(
    reference: &ImageFeatures,
    target: &ImageFeatures,
    distance: u32,
    similarity: Option<f64>,
) -> Option<MatchClassification> {
    if reference.blake3_hash == target.blake3_hash {
        return Some(MatchClassification::Exact);
    }
    if distance > MAX_PHASH_DISTANCE {
        return None;
    }

    let similarity = similarity.unwrap_or_default();
    let reference_pixels = reference.width as f64 * reference.height as f64;
    let target_pixels = target.width as f64 * target.height as f64;
    let resolution_ratio = target_pixels / reference_pixels.max(1.0);

    if similarity >= COMPRESSED_SIMILARITY {
        return Some(MatchClassification::CompressedOrReencoded);
    }
    if similarity >= VARIANT_SIMILARITY && (0.95..=1.05).contains(&resolution_ratio) {
        return Some(MatchClassification::Variant);
    }
    if similarity >= VARIANT_SIMILARITY || distance <= STRONG_PHASH_DISTANCE {
        return Some(MatchClassification::RelatedGroup);
    }
    Some(MatchClassification::WeakCandidate)
}

fn aggregate_relations(relations: Vec<CandidateRelation>) -> Vec<DifferenceMatchItem> {
    let mut grouped: HashMap<String, Vec<CandidateRelation>> = HashMap::new();
    for relation in relations {
        grouped
            .entry(normalize_path_key(&relation.target.file_path))
            .or_default()
            .push(relation);
    }

    let mut items: Vec<_> = grouped
        .into_values()
        .filter_map(|mut relations| {
            relations.sort_by(compare_candidate_relations);
            let best = relations.first()?.clone();
            let source_root = PathBuf::from(&best.source_root);
            let relative_path = Path::new(&best.target.file_path)
                .strip_prefix(&source_root)
                .unwrap_or_else(|_| Path::new(&best.target.file_path))
                .to_string_lossy()
                .to_string();
            let reference_relations = relations
                .into_iter()
                .map(|relation| ReferenceRelation {
                    reference_id: relation.reference_id,
                    reference_path: relation.reference_path,
                    classification: relation.classification,
                    phash_distance: relation.phash_distance,
                    similarity: Some(relation.similarity),
                })
                .collect();

            Some(DifferenceMatchItem {
                file_name: Path::new(&best.target.file_path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                file_path: best.target.file_path.clone(),
                source_root: best.source_root,
                relative_path,
                file_size: best.target.file_size,
                modified_at: best.target.modified_at,
                width: best.target.width,
                height: best.target.height,
                format: best.target.format,
                blake3_hash: best.target.blake3_hash,
                classification: best.classification,
                best_reference_id: best.reference_id,
                relations: reference_relations,
            })
        })
        .collect();
    items.sort_by(|left, right| {
        classification_rank(left.classification)
            .cmp(&classification_rank(right.classification))
            .then_with(|| left.file_path.cmp(&right.file_path))
    });
    items
}

fn compare_candidate_relations(
    left: &CandidateRelation,
    right: &CandidateRelation,
) -> std::cmp::Ordering {
    classification_rank(left.classification)
        .cmp(&classification_rank(right.classification))
        .then_with(|| right.similarity.total_cmp(&left.similarity))
        .then_with(|| left.phash_distance.cmp(&right.phash_distance))
        .then_with(|| left.reference_id.cmp(&right.reference_id))
}

fn classification_rank(value: MatchClassification) -> u8 {
    match value {
        MatchClassification::Exact => 0,
        MatchClassification::CompressedOrReencoded => 1,
        MatchClassification::Variant => 2,
        MatchClassification::RelatedGroup => 3,
        MatchClassification::WeakCandidate => 4,
    }
}

fn compute_similarity(reference: &ImageFeatures, target: &ImageFeatures) -> Result<f64> {
    let reference_pixels = reference.width as u64 * reference.height as u64;
    let target_pixels = target.width as u64 * target.height as u64;
    let (large, small) = if reference_pixels >= target_pixels {
        (
            Path::new(&reference.file_path),
            Path::new(&target.file_path),
        )
    } else {
        (
            Path::new(&target.file_path),
            Path::new(&reference.file_path),
        )
    };
    SsimComputer::compute_from_files(large, small)
}

fn discover_target_files(
    roots: &[String],
    recursive: bool,
) -> Result<(Vec<(PathBuf, PathBuf)>, Vec<SearchFileError>)> {
    let mut valid_roots = Vec::new();
    let mut errors = Vec::new();
    for root in roots {
        match std::fs::canonicalize(root) {
            Ok(path) if path.is_dir() => valid_roots.push(path),
            Ok(_) => errors.push(SearchFileError {
                file_path: root.clone(),
                message: "搜索路径不是目录".to_string(),
            }),
            Err(error) => errors.push(SearchFileError {
                file_path: root.clone(),
                message: format!("无法访问搜索目录: {error}"),
            }),
        }
    }
    if valid_roots.is_empty() {
        return Err(AppError::ValidationError(
            "所有搜索目录都无法访问".to_string(),
        ));
    }
    valid_roots.sort_by_key(|path| path.components().count());
    let mut unique_roots = Vec::new();
    for path in valid_roots {
        if !unique_roots
            .iter()
            .any(|root: &PathBuf| path.starts_with(root))
        {
            unique_roots.push(path);
        }
    }

    let mut files = Vec::new();
    for root in unique_roots {
        let mut walker = WalkDir::new(&root).follow_links(false);
        if !recursive {
            walker = walker.max_depth(1);
        }
        for entry in walker {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    errors.push(SearchFileError {
                        file_path: error.path().unwrap_or(&root).to_string_lossy().to_string(),
                        message: format!("遍历目录失败: {error}"),
                    });
                    continue;
                }
            };
            if entry.file_type().is_file() && is_supported_image(entry.path()) {
                files.push((root.clone(), entry.into_path()));
            }
        }
    }
    files.sort_by(|left, right| left.1.cmp(&right.1));
    Ok((files, errors))
}

fn is_supported_image(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .as_deref(),
        Some("jpg" | "jpeg" | "png" | "webp" | "bmp" | "gif")
    )
}

fn normalize_path_key(path: &str) -> String {
    let normalized = path.replace('/', "\\");
    if cfg!(windows) {
        normalized.to_lowercase()
    } else {
        normalized
    }
}

fn check_cancelled<C>(is_cancelled: &C) -> Result<()>
where
    C: Fn() -> bool,
{
    if is_cancelled() {
        Err(AppError::Other("搜索已取消".to_string()))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::image_features::ImageFeatures;

    fn feature(path: &str, hash: &str, phash: &str, width: u32, height: u32) -> ImageFeatures {
        ImageFeatures {
            file_path: path.to_string(),
            file_size: 1024,
            modified_at: 1,
            width,
            height,
            format: "png".to_string(),
            color_type: "Rgb8".to_string(),
            blake3_hash: hash.to_string(),
            phash: phash.to_string(),
        }
    }

    #[test]
    fn aggregates_one_candidate_across_multiple_references() {
        let target = feature(
            "C:/images/candidate.png",
            "target",
            "0000000000000001",
            100,
            100,
        );
        let relations = vec![
            CandidateRelation {
                target: target.clone(),
                source_root: "C:/images".to_string(),
                reference_id: "ref-a".to_string(),
                reference_path: "C:/refs/a.png".to_string(),
                classification: MatchClassification::Variant,
                phash_distance: 1,
                similarity: 0.98,
            },
            CandidateRelation {
                target,
                source_root: "C:/images".to_string(),
                reference_id: "ref-b".to_string(),
                reference_path: "C:/refs/b.png".to_string(),
                classification: MatchClassification::RelatedGroup,
                phash_distance: 2,
                similarity: 0.90,
            },
        ];

        let items = aggregate_relations(relations);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].relations.len(), 2);
        assert_eq!(items[0].best_reference_id, "ref-a");
        assert_eq!(items[0].classification, MatchClassification::Variant);
    }

    #[test]
    fn classifies_exact_and_rejects_far_candidates() {
        let reference = feature("reference.png", "same", "0000000000000000", 100, 100);
        let exact = feature("exact.png", "same", "ffffffffffffffff", 100, 100);
        let far = feature("far.png", "other", "ffffffffffffffff", 100, 100);

        assert_eq!(
            classify_relation(&reference, &exact, 64, None),
            Some(MatchClassification::Exact)
        );
        assert_eq!(classify_relation(&reference, &far, 64, Some(0.99)), None);
    }

    #[test]
    fn classifies_compressed_variant_group_and_weak_ranges() {
        let reference = feature("reference.png", "reference", "0", 1000, 1000);
        let compressed = feature("compressed.png", "a", "0", 500, 500);
        let variant = feature("variant.png", "b", "0", 1000, 1000);
        let group = feature("group.png", "c", "0", 700, 1000);
        let weak = feature("weak.png", "d", "0", 700, 1000);

        assert_eq!(
            classify_relation(&reference, &compressed, 2, Some(0.999)),
            Some(MatchClassification::CompressedOrReencoded)
        );
        assert_eq!(
            classify_relation(&reference, &variant, 3, Some(0.90)),
            Some(MatchClassification::Variant)
        );
        assert_eq!(
            classify_relation(&reference, &group, 8, Some(0.80)),
            Some(MatchClassification::RelatedGroup)
        );
        assert_eq!(
            classify_relation(&reference, &weak, 14, Some(0.60)),
            Some(MatchClassification::WeakCandidate)
        );
    }

    #[test]
    fn keeps_scanning_when_one_root_is_unavailable() {
        let valid = tempfile::tempdir().unwrap();
        std::fs::write(valid.path().join("candidate.png"), b"not decoded here").unwrap();
        let missing = valid.path().join("missing");

        let (files, errors) = discover_target_files(
            &[
                valid.path().to_string_lossy().to_string(),
                missing.to_string_lossy().to_string(),
            ],
            true,
        )
        .unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].file_path.contains("missing"));
    }
}
