use crate::db::models::{FolderRole, RunStatus, ScanProgressEvent};
use crate::db::repository::Repository;
use crate::error::{AppError, Result};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// 工作流引擎 - 编排 Phase 0-5 流程
pub struct WorkflowEngine {
    repository: Arc<Mutex<Repository>>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WorkflowOptions {
    pub allow_internal_same_root: bool,
}

impl WorkflowEngine {
    /// 创建新的工作流引擎
    pub fn new(repository: Arc<Mutex<Repository>>) -> Self {
        Self { repository }
    }

    /// 执行完整的对比工作流
    pub async fn execute_comparison(
        &self,
        run_id: &str,
        baseline_path: PathBuf,
        comparison_paths: Vec<PathBuf>,
        progress_tx: mpsc::Sender<ScanProgressEvent>,
    ) -> Result<()> {
        self.execute_comparison_with_options(
            run_id,
            baseline_path,
            comparison_paths,
            progress_tx,
            WorkflowOptions::default(),
        )
        .await
    }

    pub async fn execute_comparison_with_options(
        &self,
        run_id: &str,
        baseline_path: PathBuf,
        comparison_paths: Vec<PathBuf>,
        progress_tx: mpsc::Sender<ScanProgressEvent>,
        options: WorkflowOptions,
    ) -> Result<()> {
        // Phase 0: 预检查
        self.phase0_preflight(
            &run_id,
            &baseline_path,
            &comparison_paths,
            &progress_tx,
            options,
        )
        .await?;

        // Phase 1: 扫描与特征提取
        self.phase1_scan_and_extract(
            &run_id,
            &baseline_path,
            &comparison_paths,
            &progress_tx,
            options,
        )
        .await?;

        // Phase 2: 精确匹配
        self.phase2_exact_match(&run_id, &progress_tx, options)
            .await?;

        // Phase 3: 候选筛选
        self.phase3_candidate_search(&run_id, &progress_tx, options)
            .await?;

        // Phase 4: 规范化与相似度计算
        self.phase4_similarity(&run_id, &progress_tx).await?;

        // Phase 5: 配对分类与多候选仲裁
        self.phase5_classification(&run_id, &progress_tx).await?;

        // 完成
        {
            let repo = self.repository.lock().unwrap();
            repo.update_run_status(run_id, RunStatus::AnalysisComplete)?;
        }

        let _ = progress_tx
            .send(ScanProgressEvent {
                run_id: run_id.to_string(),
                phase: "complete".to_string(),
                total_files: 0,
                processed_files: 0,
                current_file: None,
            })
            .await;

        Ok(())
    }

    fn filter_self_matches(
        comparison_image: &crate::db::models::Image,
        baseline_images: Vec<crate::db::models::Image>,
        options: WorkflowOptions,
    ) -> Vec<crate::db::models::Image> {
        if !options.allow_internal_same_root {
            return baseline_images;
        }

        baseline_images
            .into_iter()
            .filter(|baseline_image| baseline_image.file_path != comparison_image.file_path)
            .collect()
    }

    fn paths_refer_to_same_location(left: &Path, right: &Path) -> bool {
        match (left.canonicalize(), right.canonicalize()) {
            (Ok(left), Ok(right)) => left == right,
            _ => {
                let left = left.to_string_lossy().replace('/', "\\").to_lowercase();
                let right = right.to_string_lossy().replace('/', "\\").to_lowercase();
                left == right
            }
        }
    }

    fn is_internal_same_root_path(
        baseline_path: &Path,
        comparison_path: &Path,
        options: WorkflowOptions,
    ) -> bool {
        options.allow_internal_same_root
            && Self::paths_refer_to_same_location(baseline_path, comparison_path)
    }

    fn get_images_to_compare(
        &self,
        run_id: &str,
        options: WorkflowOptions,
    ) -> Result<Vec<crate::db::models::Image>> {
        let repo = self.repository.lock().unwrap();
        let mut images = Vec::new();

        if options.allow_internal_same_root {
            images.extend(repo.get_baseline_images(run_id)?);
        }

        images.extend(repo.get_comparison_images(run_id)?);
        Ok(images)
    }

    fn get_unmatched_images_to_compare(
        &self,
        run_id: &str,
        options: WorkflowOptions,
    ) -> Result<Vec<crate::db::models::Image>> {
        let repo = self.repository.lock().unwrap();
        let mut images = Vec::new();

        if options.allow_internal_same_root {
            images.extend(repo.get_unmatched_baseline_images(run_id)?);
        }

        images.extend(repo.get_unmatched_comparison_images(run_id)?);
        Ok(images)
    }

    fn filter_self_match_ids(
        &self,
        comparison_image: &crate::db::models::Image,
        baseline_ids: Vec<i64>,
        options: WorkflowOptions,
    ) -> Result<Vec<i64>> {
        if !options.allow_internal_same_root {
            return Ok(baseline_ids);
        }

        let repo = self.repository.lock().unwrap();
        let mut filtered_ids = Vec::new();
        for baseline_id in baseline_ids {
            if let Some(baseline_image) = repo.get_image_by_id(baseline_id)? {
                if baseline_image.file_path != comparison_image.file_path {
                    filtered_ids.push(baseline_id);
                }
            }
        }

        Ok(filtered_ids)
    }

    /// Phase 0: 目录预检
    async fn phase0_preflight(
        &self,
        run_id: &str,
        baseline_path: &Path,
        comparison_paths: &[PathBuf],
        progress_tx: &mpsc::Sender<ScanProgressEvent>,
        options: WorkflowOptions,
    ) -> Result<()> {
        {
            let repo = self.repository.lock().unwrap();
            repo.update_run_status(run_id, RunStatus::Preflight)?;
        }

        let _ = progress_tx
            .send(ScanProgressEvent {
                run_id: run_id.to_string(),
                phase: "preflight".to_string(),
                total_files: 0,
                processed_files: 0,
                current_file: Some("检查目录...".to_string()),
            })
            .await;

        // 调用 PreflightChecker 执行检查
        use crate::core::preflight::PreflightChecker;

        let checker = if options.allow_internal_same_root {
            PreflightChecker::new_allowing_same_root(baseline_path, comparison_paths.to_vec())
        } else {
            PreflightChecker::new(baseline_path, comparison_paths.to_vec())
        };
        checker
            .can_proceed()
            .map_err(|e| AppError::ValidationError(format!("预检失败: {}", e)))?;

        Ok(())
    }

    /// Phase 1: 扫描与特征提取
    async fn phase1_scan_and_extract(
        &self,
        run_id: &str,
        baseline_path: &Path,
        comparison_paths: &[PathBuf],
        progress_tx: &mpsc::Sender<ScanProgressEvent>,
        options: WorkflowOptions,
    ) -> Result<()> {
        {
            let repo = self.repository.lock().unwrap();
            repo.update_run_status(run_id, RunStatus::Indexing)?;
        }

        let _ = progress_tx
            .send(ScanProgressEvent {
                run_id: run_id.to_string(),
                phase: "indexing".to_string(),
                total_files: 0,
                processed_files: 0,
                current_file: Some("扫描文件...".to_string()),
            })
            .await;

        use crate::core::scanner::ScanEngine;

        // 获取配置
        let (exclude_patterns, supported_formats) = {
            let repo = self.repository.lock().unwrap();
            let run = repo
                .get_run(run_id)?
                .ok_or_else(|| AppError::Internal("运行记录不存在".to_string()))?;

            let exclude: Vec<String> = run
                .exclude_patterns
                .map(|s| serde_json::from_str(&s).unwrap_or_default())
                .unwrap_or_default();

            let formats: Vec<String> =
                serde_json::from_str(&run.supported_formats).unwrap_or_default();

            (exclude, formats)
        };

        // 扫描 baseline
        {
            let _ = progress_tx
                .send(ScanProgressEvent {
                    run_id: run_id.to_string(),
                    phase: "indexing".to_string(),
                    total_files: 0,
                    processed_files: 0,
                    current_file: Some(format!("正在统计基准目录文件...")),
                })
                .await;

            let folder_id = {
                let repo = self.repository.lock().unwrap();
                repo.create_folder(
                    run_id,
                    baseline_path.to_string_lossy().as_ref(),
                    "A",
                    FolderRole::Baseline,
                )?
            };

            let scanner = ScanEngine::new(exclude_patterns.clone(), supported_formats.clone());

            // 使用进度回调
            let progress_tx_clone = progress_tx.clone();
            let run_id_clone = run_id.to_string();

            let image_ids = {
                let repo = self.repository.lock().unwrap();
                scanner.scan_directory(
                    &repo,
                    run_id,
                    folder_id,
                    baseline_path,
                    FolderRole::Baseline,
                    move |processed, total| {
                        let _ = progress_tx_clone.try_send(ScanProgressEvent {
                            run_id: run_id_clone.clone(),
                            phase: "indexing".to_string(),
                            total_files: total as i64,
                            processed_files: processed as i64,
                            current_file: Some(format!("扫描基准目录: {}/{}", processed, total)),
                        });
                    },
                )?
            };

            let repo = self.repository.lock().unwrap();
            repo.update_folder_file_count(folder_id, image_ids.len() as i64)?;
        }

        // 扫描每个对比目录
        for (idx, comp_path) in comparison_paths.iter().enumerate() {
            let alias = format!("{}", (b'B' + idx as u8) as char);

            if Self::is_internal_same_root_path(baseline_path, comp_path, options) {
                let _ = progress_tx
                    .send(ScanProgressEvent {
                        run_id: run_id.to_string(),
                        phase: "indexing".to_string(),
                        total_files: 0,
                        processed_files: 0,
                        current_file: Some("复用基准目录作为内部对比，跳过重复扫描".to_string()),
                    })
                    .await;
                continue;
            }

            let _ = progress_tx
                .send(ScanProgressEvent {
                    run_id: run_id.to_string(),
                    phase: "indexing".to_string(),
                    total_files: 0,
                    processed_files: 0,
                    current_file: Some(format!("正在统计对比目录 {} 文件...", alias)),
                })
                .await;

            let folder_id = {
                let repo = self.repository.lock().unwrap();
                repo.create_folder(
                    run_id,
                    comp_path.to_string_lossy().as_ref(),
                    &alias,
                    FolderRole::Comparison,
                )?
            };

            let scanner = ScanEngine::new(exclude_patterns.clone(), supported_formats.clone());

            let progress_tx_clone = progress_tx.clone();
            let run_id_clone = run_id.to_string();
            let alias_clone = alias.clone();

            let image_ids = {
                let repo = self.repository.lock().unwrap();
                scanner.scan_directory(
                    &repo,
                    run_id,
                    folder_id,
                    comp_path,
                    FolderRole::Comparison,
                    move |processed, total| {
                        let _ = progress_tx_clone.try_send(ScanProgressEvent {
                            run_id: run_id_clone.clone(),
                            phase: "indexing".to_string(),
                            total_files: total as i64,
                            processed_files: processed as i64,
                            current_file: Some(format!(
                                "扫描对比目录 {}: {}/{}",
                                alias_clone, processed, total
                            )),
                        });
                    },
                )?
            };

            let repo = self.repository.lock().unwrap();
            repo.update_folder_file_count(folder_id, image_ids.len() as i64)?;
        }

        Ok(())
    }

    /// Phase 2: 精确匹配
    async fn phase2_exact_match(
        &self,
        run_id: &str,
        progress_tx: &mpsc::Sender<ScanProgressEvent>,
        options: WorkflowOptions,
    ) -> Result<()> {
        {
            let repo = self.repository.lock().unwrap();
            repo.update_run_status(run_id, RunStatus::Matching)?;
        }

        let _ = progress_tx
            .send(ScanProgressEvent {
                run_id: run_id.to_string(),
                phase: "matching".to_string(),
                total_files: 0,
                processed_files: 0,
                current_file: Some("精确匹配 (BLAKE3)".to_string()),
            })
            .await;

        // 1. 查询所有待比较图片。内部同目录模式复用 baseline 图片作为待比较集合。
        let comparison_images = { self.get_images_to_compare(run_id, options)? };

        // 2. 对每个对比图片查找相同 BLAKE3
        let total = comparison_images.len();
        let mut processed = 0;

        let _ = progress_tx
            .send(ScanProgressEvent {
                run_id: run_id.to_string(),
                phase: "matching".to_string(),
                total_files: total as i64,
                processed_files: 0,
                current_file: Some(format!("准备精确匹配: 0/{}", total)),
            })
            .await;

        for comp_img in comparison_images {
            // 跳过没有哈希的图片
            let Some(ref blake3_hash) = comp_img.blake3_hash else {
                processed += 1;
                continue;
            };

            // 在 baseline 中查找相同哈希
            let baseline_ids = {
                let repo = self.repository.lock().unwrap();
                repo.find_baseline_by_hash(run_id, blake3_hash)?
            };
            let baseline_ids = self.filter_self_match_ids(&comp_img, baseline_ids, options)?;

            // 如果找到精确匹配，创建分析结果
            if !baseline_ids.is_empty() {
                use crate::db::models::AnalysisType;
                use crate::db::repository::AnalysisResultInsert;

                // 选择第一个基准图片作为主匹配（稳定排序）
                let primary_match_id = baseline_ids[0];

                let result = AnalysisResultInsert {
                    run_id: run_id.to_string(),
                    comparison_image_id: comp_img.id,
                    analysis_type: AnalysisType::ExactDuplicate,
                    primary_match_image_id: Some(primary_match_id),
                    all_candidate_ids: Some(baseline_ids.clone()),
                    candidate_truncated: false,
                    phash_distance: Some(0),
                    ssim_score: None,
                    size_ratio: None,
                    resolution_ratio: None,
                    aspect_diff: None,
                    direction_smaller_resolution: false,
                    direction_smaller_filesize: false,
                    algorithm_profile_id: "v1".to_string(),
                    analysis_metadata: None,
                };

                let repo = self.repository.lock().unwrap();
                repo.insert_analysis_result(&result)?;
            }

            processed += 1;

            // 定期发送进度
            if processed % 10 == 0 || processed == total {
                let _ = progress_tx
                    .send(ScanProgressEvent {
                        run_id: run_id.to_string(),
                        phase: "matching".to_string(),
                        total_files: total as i64,
                        processed_files: processed as i64,
                        current_file: Some(format!("精确匹配: {}/{}", processed, total)),
                    })
                    .await;
            }
        }

        Ok(())
    }

    /// Phase 3: 候选筛选
    async fn phase3_candidate_search(
        &self,
        run_id: &str,
        progress_tx: &mpsc::Sender<ScanProgressEvent>,
        options: WorkflowOptions,
    ) -> Result<()> {
        let _ = progress_tx
            .send(ScanProgressEvent {
                run_id: run_id.to_string(),
                phase: "candidate_search".to_string(),
                total_files: 0,
                processed_files: 0,
                current_file: Some("候选筛选 (pHash)".to_string()),
            })
            .await;

        // 获取配置参数
        let (phash_max_distance, aspect_tolerance, top_k) = {
            let repo = self.repository.lock().unwrap();
            let run = repo
                .get_run(run_id)?
                .ok_or_else(|| AppError::Internal("运行记录不存在".to_string()))?;
            (run.phash_max_distance, run.aspect_ratio_tolerance, 50) // Top-K 默认 50
        };

        // 1. 查询未精确匹配的待比较图片。内部同目录模式复用 baseline 图片。
        let unmatched_images = { self.get_unmatched_images_to_compare(run_id, options)? };

        let total = unmatched_images.len();
        let mut processed = 0;

        let _ = progress_tx
            .send(ScanProgressEvent {
                run_id: run_id.to_string(),
                phase: "candidate_search".to_string(),
                total_files: total as i64,
                processed_files: 0,
                current_file: Some(format!("准备候选筛选: 0/{}", total)),
            })
            .await;

        use crate::core::matching::PhashMatcher;

        let matcher = PhashMatcher::new(phash_max_distance, top_k);

        // 2. 对每个未匹配的对比图片查找候选
        for comp_img in unmatched_images {
            // 跳过没有 pHash 的图片
            let Some(ref comp_phash) = comp_img.phash else {
                processed += 1;
                continue;
            };

            // 2.1 获取符合宽高比的基准图片
            let baseline_candidates = {
                let repo = self.repository.lock().unwrap();
                repo.find_baseline_by_phash(run_id, comp_img.aspect_ratio, aspect_tolerance)?
            };
            let baseline_candidates =
                Self::filter_self_matches(&comp_img, baseline_candidates, options);

            // 2.2 使用 pHash 匹配找到候选
            let (candidates, truncated) = matcher.find_candidates(comp_phash, &baseline_candidates);

            // 2.3 如果没有候选，创建 no_baseline_match 结果
            if candidates.is_empty() {
                use crate::db::models::AnalysisType;
                use crate::db::repository::AnalysisResultInsert;

                let result = AnalysisResultInsert {
                    run_id: run_id.to_string(),
                    comparison_image_id: comp_img.id,
                    analysis_type: AnalysisType::NoBaselineMatch,
                    primary_match_image_id: None,
                    all_candidate_ids: None,
                    candidate_truncated: false,
                    phash_distance: None,
                    ssim_score: None,
                    size_ratio: None,
                    resolution_ratio: None,
                    aspect_diff: None,
                    direction_smaller_resolution: false,
                    direction_smaller_filesize: false,
                    algorithm_profile_id: "v1".to_string(),
                    analysis_metadata: None,
                };

                let repo = self.repository.lock().unwrap();
                repo.insert_analysis_result(&result)?;
            } else {
                // 2.4 保存候选信息到临时表（为 Phase 4 准备）
                // 暂时不创建分析结果，等 Phase 5 分类后再创建
                // 这里我们将候选信息存储到内存或临时表
                // 简化实现：直接标记为 NotEvaluated，等 Phase 4 计算 SSIM 后更新
                use crate::db::models::AnalysisType;
                use crate::db::repository::AnalysisResultInsert;

                let candidate_ids: Vec<i64> =
                    candidates.iter().map(|c| c.baseline_image.id).collect();
                let primary_match_id = candidate_ids.first().copied();

                let result = AnalysisResultInsert {
                    run_id: run_id.to_string(),
                    comparison_image_id: comp_img.id,
                    analysis_type: AnalysisType::NotEvaluated, // 临时状态
                    primary_match_image_id: primary_match_id,
                    all_candidate_ids: Some(candidate_ids),
                    candidate_truncated: truncated,
                    phash_distance: candidates.first().map(|c| c.phash_distance),
                    ssim_score: None,
                    size_ratio: None,
                    resolution_ratio: None,
                    aspect_diff: None,
                    direction_smaller_resolution: false,
                    direction_smaller_filesize: false,
                    algorithm_profile_id: "v1".to_string(),
                    analysis_metadata: None,
                };

                let repo = self.repository.lock().unwrap();
                repo.insert_analysis_result(&result)?;
            }

            processed += 1;

            // 定期发送进度
            if processed % 10 == 0 || processed == total {
                let _ = progress_tx
                    .send(ScanProgressEvent {
                        run_id: run_id.to_string(),
                        phase: "candidate_search".to_string(),
                        total_files: total as i64,
                        processed_files: processed as i64,
                        current_file: Some(format!("候选筛选: {}/{}", processed, total)),
                    })
                    .await;
            }
        }

        Ok(())
    }

    /// Phase 4: 规范化与相似度计算
    async fn phase4_similarity(
        &self,
        run_id: &str,
        progress_tx: &mpsc::Sender<ScanProgressEvent>,
    ) -> Result<()> {
        {
            let repo = self.repository.lock().unwrap();
            repo.update_run_status(run_id, RunStatus::Scoring)?;
        }

        let _ = progress_tx
            .send(ScanProgressEvent {
                run_id: run_id.to_string(),
                phase: "scoring".to_string(),
                total_files: 0,
                processed_files: 0,
                current_file: Some("计算相似度 (SSIM)".to_string()),
            })
            .await;

        // 1. 获取待计算 SSIM 的候选对
        let pending_pairs = {
            let repo = self.repository.lock().unwrap();
            repo.get_pending_ssim_results(run_id)?
        };

        let total = pending_pairs.len();
        let mut processed = 0;

        let _ = progress_tx
            .send(ScanProgressEvent {
                run_id: run_id.to_string(),
                phase: "scoring".to_string(),
                total_files: total as i64,
                processed_files: 0,
                current_file: Some(format!("准备计算相似度: 0/{}", total)),
            })
            .await;

        use crate::core::ssim::SsimEngine;
        use std::path::Path;

        // 2. 计算每个候选对的 SSIM
        for pair in pending_pairs {
            let _ = progress_tx
                .send(ScanProgressEvent {
                    run_id: run_id.to_string(),
                    phase: "scoring".to_string(),
                    total_files: total as i64,
                    processed_files: processed as i64,
                    current_file: Some(format!(
                        "计算相似度 {}/{}: {}",
                        processed + 1,
                        total,
                        Path::new(&pair.comparison_path)
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("当前图片")
                    )),
                })
                .await;

            match SsimEngine::compute_ssim(
                Path::new(&pair.baseline_path),
                Path::new(&pair.comparison_path),
            ) {
                Ok(ssim_score) => {
                    // 计算尺寸和分辨率比率
                    let size_ratio = pair.comparison_size as f64 / pair.baseline_size as f64;
                    let resolution_ratio = (pair.comparison_width as f64
                        * pair.comparison_height as f64)
                        / (pair.baseline_width as f64 * pair.baseline_height as f64);

                    // 计算宽高比差异
                    let comp_aspect = pair.comparison_width as f64 / pair.comparison_height as f64;
                    let base_aspect = pair.baseline_width as f64 / pair.baseline_height as f64;
                    let aspect_diff =
                        (comp_aspect - base_aspect).abs() / comp_aspect.max(base_aspect);

                    // 方向性：文件大小
                    let direction_smaller_filesize = pair.comparison_size < pair.baseline_size;

                    // 方向性：分辨率
                    let direction_smaller_resolution = pair.comparison_width < pair.baseline_width
                        && pair.comparison_height < pair.baseline_height;

                    // 更新数据库
                    let repo = self.repository.lock().unwrap();
                    repo.update_analysis_ssim(
                        pair.result_id,
                        ssim_score,
                        size_ratio,
                        resolution_ratio,
                        aspect_diff,
                        direction_smaller_resolution,
                        direction_smaller_filesize,
                    )?;
                }
                Err(e) => {
                    eprintln!(
                        "计算 SSIM 失败: {} <-> {}: {}",
                        pair.comparison_path, pair.baseline_path, e
                    );
                    // 失败时保持 not_evaluated 状态
                }
            }

            processed += 1;

            // 定期发送进度
            let _ = progress_tx
                .send(ScanProgressEvent {
                    run_id: run_id.to_string(),
                    phase: "scoring".to_string(),
                    total_files: total as i64,
                    processed_files: processed as i64,
                    current_file: Some(format!("计算相似度: {}/{}", processed, total)),
                })
                .await;
        }

        Ok(())
    }

    /// Phase 5: 配对分类与多候选仲裁
    async fn phase5_classification(
        &self,
        run_id: &str,
        progress_tx: &mpsc::Sender<ScanProgressEvent>,
    ) -> Result<()> {
        {
            let repo = self.repository.lock().unwrap();
            repo.update_run_status(run_id, RunStatus::Resolving)?;
        }

        let _ = progress_tx
            .send(ScanProgressEvent {
                run_id: run_id.to_string(),
                phase: "resolving".to_string(),
                total_files: 0,
                processed_files: 0,
                current_file: Some("分类结果".to_string()),
            })
            .await;

        // 1. 获取需要分类的分析结果
        let results_to_classify = {
            let repo = self.repository.lock().unwrap();
            repo.get_results_for_classification(run_id)?
        };

        let total = results_to_classify.len();
        let mut processed = 0;

        let _ = progress_tx
            .send(ScanProgressEvent {
                run_id: run_id.to_string(),
                phase: "resolving".to_string(),
                total_files: total as i64,
                processed_files: 0,
                current_file: Some(format!("准备分类结果: 0/{}", total)),
            })
            .await;

        // 2. 获取分类器配置
        let (compressed_threshold, variant_lower_bound, aspect_tolerance, tie_threshold) = {
            let repo = self.repository.lock().unwrap();
            let run = repo
                .get_run(run_id)?
                .ok_or_else(|| AppError::Internal("运行记录不存在".to_string()))?;
            (
                run.compressed_ssim_threshold,
                run.variant_review_lower_bound,
                run.aspect_ratio_tolerance,
                run.primary_match_tie_threshold,
            )
        };

        use crate::core::classifier::{CandidateMatch, MatchType, ResultClassifier};

        let classifier = ResultClassifier::new(
            compressed_threshold,
            variant_lower_bound,
            aspect_tolerance,
            tie_threshold,
        );

        // 3. 对每个结果进行分类
        for result in results_to_classify {
            // 获取对比图片
            let comparison_img = {
                let repo = self.repository.lock().unwrap();
                repo.get_image_by_id(result.comparison_image_id)?
                    .ok_or_else(|| AppError::Internal("对比图片不存在".to_string()))?
            };

            // 获取所有候选图片
            let candidate_ids: Vec<i64> = result
                .all_candidate_ids
                .as_ref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default();

            let mut candidates = Vec::new();
            for candidate_id in candidate_ids {
                let repo = self.repository.lock().unwrap();
                if let Some(baseline_img) = repo.get_image_by_id(candidate_id)? {
                    candidates.push(CandidateMatch {
                        baseline_image: baseline_img,
                        match_type: MatchType::Similar,
                        phash_distance: result.phash_distance.unwrap_or(0),
                        ssim_score: result.ssim_score,
                    });
                }
            }

            // 调用分类器
            let classification =
                classifier.classify(&comparison_img, candidates, result.candidate_truncated);

            // 更新分析类型
            {
                let repo = self.repository.lock().unwrap();
                repo.update_analysis_type(result.id, classification.analysis_type)?;
            }

            processed += 1;

            // 定期发送进度
            if processed % 10 == 0 || processed == total {
                let _ = progress_tx
                    .send(ScanProgressEvent {
                        run_id: run_id.to_string(),
                        phase: "resolving".to_string(),
                        total_files: total as i64,
                        processed_files: processed as i64,
                        current_file: Some(format!("分类结果: {}/{}", processed, total)),
                    })
                    .await;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repository::RunConfig;
    use crate::db::schema::initialize_database;
    use image::{ImageBuffer, Rgba};
    use rusqlite::Connection;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    fn write_test_png(path: &Path, color: [u8; 4]) {
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(16, 16, Rgba(color));
        img.save(path).unwrap();
    }

    #[tokio::test]
    async fn internal_same_root_workflow_scans_directory_once() {
        let temp = tempdir().unwrap();
        let image_dir = temp.path().join("images");
        std::fs::create_dir(&image_dir).unwrap();
        write_test_png(&image_dir.join("a.png"), [255, 0, 0, 255]);
        write_test_png(&image_dir.join("b.png"), [0, 255, 0, 255]);

        let conn = Connection::open_in_memory().unwrap();
        initialize_database(&conn).unwrap();
        let repo = Arc::new(Mutex::new(Repository::new(conn)));
        let run_id = "internal-same-root";

        {
            let repo = repo.lock().unwrap();
            repo.create_run(
                run_id,
                "test",
                "imagekeeper-v1-ssim",
                image_dir.to_string_lossy().as_ref(),
                "A",
                &[image_dir.to_string_lossy().to_string()],
                &["B".to_string()],
                &RunConfig::default(),
            )
            .unwrap();
        }

        let engine = WorkflowEngine::new(repo.clone());
        let (progress_tx, _progress_rx) = mpsc::channel(32);

        engine
            .execute_comparison_with_options(
                run_id,
                image_dir.clone(),
                vec![image_dir.clone()],
                progress_tx,
                WorkflowOptions {
                    allow_internal_same_root: true,
                },
            )
            .await
            .unwrap();

        let repo = repo.lock().unwrap();
        let folder_count: i64 = repo
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM folders WHERE run_id = ?1",
                [run_id],
                |row| row.get(0),
            )
            .unwrap();
        let image_count: i64 = repo
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM images WHERE run_id = ?1",
                [run_id],
                |row| row.get(0),
            )
            .unwrap();
        let analysis_count: i64 = repo
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM analysis_results WHERE run_id = ?1",
                [run_id],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(folder_count, 1, "内部同目录对比不应创建重复 folder");
        assert_eq!(image_count, 2, "内部同目录对比不应重复插入同一路径图片");
        assert_eq!(
            analysis_count, 2,
            "内部同目录对比应把基准图片作为待比较图片参与分析"
        );

        let stats = repo.get_analysis_stats(run_id).unwrap();
        assert_eq!(
            stats.comparison_total, 2,
            "内部同目录对比的统计总数应按待分析图片数展示"
        );
    }
}
