use super::models::*;
use crate::error::Result;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

/// 数据库访问层 - 符合 IMAGE_COMPARISON_WORKFLOW.md 规范
pub struct Repository {
    conn: Connection,
}

impl Repository {
    /// 创建新的 Repository 实例
    pub fn new(conn: Connection) -> Self {
        Self { conn }
    }

    /// 获取数据库连接的引用
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    // ========================================================================
    // 运行快照 (Runs) 相关操作
    // ========================================================================

    /// 创建新的运行快照
    pub fn create_run(
        &self,
        run_id: &str,
        app_version: &str,
        algorithm_profile_id: &str,
        baseline_root: &str,
        baseline_alias: &str,
        comparison_roots: &[String],
        comparison_aliases: &[String],
        config: &RunConfig,
    ) -> Result<i64> {
        let now = Utc::now().timestamp();
        let comparison_roots_json = serde_json::to_string(comparison_roots)?;
        let comparison_aliases_json = serde_json::to_string(comparison_aliases)?;
        let supported_formats_json = serde_json::to_string(&config.supported_formats)?;
        let exclude_patterns_json = config
            .exclude_patterns
            .as_ref()
            .map(|p| serde_json::to_string(p).ok())
            .flatten();

        self.conn.execute(
            r#"INSERT INTO runs (
                run_id, application_version, algorithm_profile_id,
                baseline_root_path, baseline_root_alias,
                comparison_root_paths, comparison_root_aliases,
                phash_max_distance, compressed_ssim_threshold,
                variant_review_lower_bound, aspect_ratio_tolerance,
                primary_match_tie_threshold, supported_formats,
                follow_symlinks, exclude_patterns, max_workers,
                status, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)"#,
            params![
                run_id,
                app_version,
                algorithm_profile_id,
                baseline_root,
                baseline_alias,
                comparison_roots_json,
                comparison_aliases_json,
                config.phash_max_distance,
                config.compressed_ssim_threshold,
                config.variant_review_lower_bound,
                config.aspect_ratio_tolerance,
                config.primary_match_tie_threshold,
                supported_formats_json,
                config.follow_symlinks,
                exclude_patterns_json,
                config.max_workers,
                RunStatus::Pending.as_str(),
                now,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// 更新运行状态
    pub fn update_run_status(&self, run_id: &str, status: RunStatus) -> Result<()> {
        let now = Utc::now().timestamp();
        let status_str = status.as_str();

        match status {
            RunStatus::Preflight | RunStatus::Indexing => {
                self.conn.execute(
                    "UPDATE runs SET status = ?1, started_at = ?2 WHERE run_id = ?3",
                    params![status_str, now, run_id],
                )?;
            }
            RunStatus::AnalysisComplete
            | RunStatus::ActionComplete
            | RunStatus::CompletedWithErrors
            | RunStatus::Failed => {
                self.conn.execute(
                    "UPDATE runs SET status = ?1, completed_at = ?2 WHERE run_id = ?3",
                    params![status_str, now, run_id],
                )?;
            }
            _ => {
                self.conn.execute(
                    "UPDATE runs SET status = ?1 WHERE run_id = ?2",
                    params![status_str, run_id],
                )?;
            }
        }

        Ok(())
    }

    /// 更新运行文件统计
    pub fn update_run_file_counts(
        &self,
        run_id: &str,
        baseline_count: i64,
        comparison_count: i64,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE runs SET total_baseline_files = ?1, total_comparison_files = ?2 WHERE run_id = ?3",
            params![baseline_count, comparison_count, run_id],
        )?;
        Ok(())
    }

    /// 增加运行错误计数
    pub fn increment_run_error_count(&self, run_id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE runs SET error_count = error_count + 1 WHERE run_id = ?1",
            params![run_id],
        )?;
        Ok(())
    }

    /// 获取运行快照
    pub fn get_run(&self, run_id: &str) -> Result<Option<Run>> {
        self.conn
            .query_row(
                r#"SELECT id, run_id, application_version, algorithm_profile_id,
                          baseline_root_path, baseline_root_alias,
                          comparison_root_paths, comparison_root_aliases,
                          phash_max_distance, compressed_ssim_threshold,
                          variant_review_lower_bound, aspect_ratio_tolerance,
                          primary_match_tie_threshold, supported_formats,
                          follow_symlinks, exclude_patterns, max_workers,
                          status, total_baseline_files, total_comparison_files,
                          error_count, created_at, started_at, completed_at
                   FROM runs WHERE run_id = ?1"#,
                params![run_id],
                |row| {
                    Ok(Run {
                        id: row.get(0)?,
                        run_id: row.get(1)?,
                        application_version: row.get(2)?,
                        algorithm_profile_id: row.get(3)?,
                        baseline_root_path: row.get(4)?,
                        baseline_root_alias: row.get(5)?,
                        comparison_root_paths: row.get(6)?,
                        comparison_root_aliases: row.get(7)?,
                        phash_max_distance: row.get(8)?,
                        compressed_ssim_threshold: row.get(9)?,
                        variant_review_lower_bound: row.get(10)?,
                        aspect_ratio_tolerance: row.get(11)?,
                        primary_match_tie_threshold: row.get(12)?,
                        supported_formats: row.get(13)?,
                        follow_symlinks: row.get(14)?,
                        exclude_patterns: row.get(15)?,
                        max_workers: row.get(16)?,
                        status: RunStatus::from_str(&row.get::<_, String>(17)?)
                            .unwrap_or(RunStatus::Pending),
                        total_baseline_files: row.get(18)?,
                        total_comparison_files: row.get(19)?,
                        error_count: row.get(20)?,
                        created_at: row.get(21)?,
                        started_at: row.get(22)?,
                        completed_at: row.get(23)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    // ========================================================================
    // 文件夹 (Folders) 相关操作
    // ========================================================================

    /// 创建文件夹记录
    pub fn create_folder(
        &self,
        run_id: &str,
        path: &str,
        alias: &str,
        role: FolderRole,
    ) -> Result<i64> {
        let now = Utc::now().timestamp();

        self.conn.execute(
            "INSERT INTO folders (run_id, path, alias, role, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![run_id, path, alias, role.as_str(), now],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// 更新文件夹文件计数
    pub fn update_folder_file_count(&self, folder_id: i64, count: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE folders SET file_count = ?1 WHERE id = ?2",
            params![count, folder_id],
        )?;
        Ok(())
    }

    /// 获取运行的所有文件夹
    pub fn get_folders_by_run(&self, run_id: &str) -> Result<Vec<Folder>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, run_id, path, alias, role, file_count, created_at FROM folders WHERE run_id = ?1"
        )?;

        let folders = stmt
            .query_map(params![run_id], |row| {
                Ok(Folder {
                    id: row.get(0)?,
                    run_id: row.get(1)?,
                    path: row.get(2)?,
                    alias: row.get(3)?,
                    role: FolderRole::from_str(&row.get::<_, String>(4)?)
                        .unwrap_or(FolderRole::Baseline),
                    file_count: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(folders)
    }

    // ========================================================================
    // 图片 (Images) 相关操作
    // ========================================================================

    /// 插入图片记录
    pub fn insert_image(&self, img: &ImageInsert) -> Result<i64> {
        let now = Utc::now().timestamp();

        self.conn.execute(
            r#"INSERT INTO images (
                run_id, folder_id, source_role, file_path, relative_path,
                file_size, file_modified_at, width, height, format,
                aspect_ratio, frame_count, frame_strategy, scan_status, scanned_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"#,
            params![
                img.run_id,
                img.folder_id,
                img.source_role.as_str(),
                img.file_path,
                img.relative_path,
                img.file_size,
                img.file_modified_at,
                img.width,
                img.height,
                img.format,
                img.aspect_ratio,
                img.frame_count,
                img.frame_strategy,
                ScanStatus::Decoded.as_str(),
                now,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// 更新图片哈希
    pub fn update_image_hash(
        &self,
        image_id: i64,
        blake3_hash: &str,
        phash: &str,
        phash_version: &str,
    ) -> Result<()> {
        let now = Utc::now().timestamp();

        self.conn.execute(
            r#"UPDATE images SET
                blake3_hash = ?1,
                phash = ?2,
                phash_algorithm_version = ?3,
                scan_status = ?4,
                hash_computed_at = ?5
            WHERE id = ?6"#,
            params![
                blake3_hash,
                phash,
                phash_version,
                ScanStatus::Completed.as_str(),
                now,
                image_id
            ],
        )?;

        Ok(())
    }

    /// 更新图片扫描状态
    pub fn update_image_scan_status(
        &self,
        image_id: i64,
        status: ScanStatus,
        error_msg: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE images SET scan_status = ?1, error_message = ?2 WHERE id = ?3",
            params![status.as_str(), error_msg, image_id],
        )?;
        Ok(())
    }

    /// 获取运行的所有基准图片
    pub fn get_baseline_images(&self, run_id: &str) -> Result<Vec<Image>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, run_id, folder_id, source_role, file_path, relative_path,
                      file_size, file_modified_at, width, height, format,
                      aspect_ratio, frame_count, frame_strategy,
                      blake3_hash, phash, phash_algorithm_version,
                      scan_status, error_message, scanned_at, hash_computed_at
               FROM images
               WHERE run_id = ?1 AND source_role = 'baseline' AND scan_status = 'completed'"#,
        )?;

        let images = stmt
            .query_map(params![run_id], |row| {
                Ok(Image {
                    id: row.get(0)?,
                    run_id: row.get(1)?,
                    folder_id: row.get(2)?,
                    source_role: FolderRole::from_str(&row.get::<_, String>(3)?)
                        .unwrap_or(FolderRole::Baseline),
                    file_path: row.get(4)?,
                    relative_path: row.get(5)?,
                    file_size: row.get(6)?,
                    file_modified_at: row.get(7)?,
                    width: row.get(8)?,
                    height: row.get(9)?,
                    format: row.get(10)?,
                    aspect_ratio: row.get(11)?,
                    frame_count: row.get(12)?,
                    frame_strategy: row.get(13)?,
                    blake3_hash: row.get(14)?,
                    phash: row.get(15)?,
                    phash_algorithm_version: row.get(16)?,
                    scan_status: ScanStatus::from_str(&row.get::<_, String>(17)?)
                        .unwrap_or(ScanStatus::Pending),
                    error_message: row.get(18)?,
                    scanned_at: row.get(19)?,
                    hash_computed_at: row.get(20)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(images)
    }

    /// 获取运行的所有对比图片
    pub fn get_comparison_images(&self, run_id: &str) -> Result<Vec<Image>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, run_id, folder_id, source_role, file_path, relative_path,
                      file_size, file_modified_at, width, height, format,
                      aspect_ratio, frame_count, frame_strategy,
                      blake3_hash, phash, phash_algorithm_version,
                      scan_status, error_message, scanned_at, hash_computed_at
               FROM images
               WHERE run_id = ?1 AND source_role = 'comparison' AND scan_status = 'completed'"#,
        )?;

        let images = stmt
            .query_map(params![run_id], |row| {
                Ok(Image {
                    id: row.get(0)?,
                    run_id: row.get(1)?,
                    folder_id: row.get(2)?,
                    source_role: FolderRole::from_str(&row.get::<_, String>(3)?)
                        .unwrap_or(FolderRole::Comparison),
                    file_path: row.get(4)?,
                    relative_path: row.get(5)?,
                    file_size: row.get(6)?,
                    file_modified_at: row.get(7)?,
                    width: row.get(8)?,
                    height: row.get(9)?,
                    format: row.get(10)?,
                    aspect_ratio: row.get(11)?,
                    frame_count: row.get(12)?,
                    frame_strategy: row.get(13)?,
                    blake3_hash: row.get(14)?,
                    phash: row.get(15)?,
                    phash_algorithm_version: row.get(16)?,
                    scan_status: ScanStatus::from_str(&row.get::<_, String>(17)?)
                        .unwrap_or(ScanStatus::Pending),
                    error_message: row.get(18)?,
                    scanned_at: row.get(19)?,
                    hash_computed_at: row.get(20)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(images)
    }

    /// 根据 BLAKE3 哈希查找基准图片
    pub fn find_baseline_by_hash(&self, run_id: &str, blake3_hash: &str) -> Result<Vec<i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT id FROM images WHERE run_id = ?1 AND source_role = 'baseline' AND blake3_hash = ?2"
        )?;

        let ids = stmt
            .query_map(params![run_id, blake3_hash], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(ids)
    }

    /// 获取未精确匹配的对比图片（Phase 3 使用）
    pub fn get_unmatched_comparison_images(&self, run_id: &str) -> Result<Vec<Image>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, run_id, folder_id, source_role, file_path, relative_path,
                      file_size, file_modified_at, width, height, format,
                      aspect_ratio, frame_count, frame_strategy,
                      blake3_hash, phash, phash_algorithm_version,
                      scan_status, error_message, scanned_at, hash_computed_at
               FROM images
               WHERE run_id = ?1
                 AND source_role = 'comparison'
                 AND scan_status = 'completed'
                 AND id NOT IN (
                     SELECT comparison_image_id FROM analysis_results WHERE run_id = ?1
                 )"#,
        )?;

        let images = stmt
            .query_map(params![run_id], |row| {
                Ok(Image {
                    id: row.get(0)?,
                    run_id: row.get(1)?,
                    folder_id: row.get(2)?,
                    source_role: FolderRole::from_str(&row.get::<_, String>(3)?)
                        .unwrap_or(FolderRole::Comparison),
                    file_path: row.get(4)?,
                    relative_path: row.get(5)?,
                    file_size: row.get(6)?,
                    file_modified_at: row.get(7)?,
                    width: row.get(8)?,
                    height: row.get(9)?,
                    format: row.get(10)?,
                    aspect_ratio: row.get(11)?,
                    frame_count: row.get(12)?,
                    frame_strategy: row.get(13)?,
                    blake3_hash: row.get(14)?,
                    phash: row.get(15)?,
                    phash_algorithm_version: row.get(16)?,
                    scan_status: ScanStatus::from_str(&row.get::<_, String>(17)?)
                        .unwrap_or(ScanStatus::Pending),
                    error_message: row.get(18)?,
                    scanned_at: row.get(19)?,
                    hash_computed_at: row.get(20)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(images)
    }

    /// 获取内部同目录模式下尚未产生分析结果的基准图片（Phase 3 使用）
    pub fn get_unmatched_baseline_images(&self, run_id: &str) -> Result<Vec<Image>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, run_id, folder_id, source_role, file_path, relative_path,
                      file_size, file_modified_at, width, height, format,
                      aspect_ratio, frame_count, frame_strategy,
                      blake3_hash, phash, phash_algorithm_version,
                      scan_status, error_message, scanned_at, hash_computed_at
               FROM images
               WHERE run_id = ?1
                 AND source_role = 'baseline'
                 AND scan_status = 'completed'
                 AND id NOT IN (
                     SELECT comparison_image_id FROM analysis_results WHERE run_id = ?1
                 )"#,
        )?;

        let images = stmt
            .query_map(params![run_id], |row| {
                Ok(Image {
                    id: row.get(0)?,
                    run_id: row.get(1)?,
                    folder_id: row.get(2)?,
                    source_role: FolderRole::from_str(&row.get::<_, String>(3)?)
                        .unwrap_or(FolderRole::Baseline),
                    file_path: row.get(4)?,
                    relative_path: row.get(5)?,
                    file_size: row.get(6)?,
                    file_modified_at: row.get(7)?,
                    width: row.get(8)?,
                    height: row.get(9)?,
                    format: row.get(10)?,
                    aspect_ratio: row.get(11)?,
                    frame_count: row.get(12)?,
                    frame_strategy: row.get(13)?,
                    blake3_hash: row.get(14)?,
                    phash: row.get(15)?,
                    phash_algorithm_version: row.get(16)?,
                    scan_status: ScanStatus::from_str(&row.get::<_, String>(17)?)
                        .unwrap_or(ScanStatus::Pending),
                    error_message: row.get(18)?,
                    scanned_at: row.get(19)?,
                    hash_computed_at: row.get(20)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(images)
    }

    /// 获取需要计算结构相似性的候选对（Phase 4 使用）
    pub fn get_pending_ssim_results(&self, run_id: &str) -> Result<Vec<PendingSsimPair>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT
                   ar.id as result_id,
                   ar.comparison_image_id,
                   ar.primary_match_image_id,
                   ar.all_candidate_ids,
                   ci.file_path as comparison_path,
                   bi.file_path as baseline_path,
                   ci.file_size as comparison_size,
                   bi.file_size as baseline_size,
                   ci.width as comparison_width,
                   ci.height as comparison_height,
                   bi.width as baseline_width,
                   bi.height as baseline_height
               FROM analysis_results ar
               JOIN images ci ON ar.comparison_image_id = ci.id
               LEFT JOIN images bi ON ar.primary_match_image_id = bi.id
               WHERE ar.run_id = ?1
                 AND ar.analysis_type = 'not_evaluated'
                 AND ar.ssim_score IS NULL
                 AND ar.primary_match_image_id IS NOT NULL"#,
        )?;

        let pairs = stmt
            .query_map(params![run_id], |row| {
                Ok(PendingSsimPair {
                    result_id: row.get(0)?,
                    comparison_image_id: row.get(1)?,
                    baseline_image_id: row.get(2)?,
                    all_candidate_ids: row
                        .get::<_, Option<String>>(3)?
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default(),
                    comparison_path: row.get(4)?,
                    baseline_path: row.get(5)?,
                    comparison_size: row.get(6)?,
                    baseline_size: row.get(7)?,
                    comparison_width: row.get(8)?,
                    comparison_height: row.get(9)?,
                    baseline_width: row.get(10)?,
                    baseline_height: row.get(11)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(pairs)
    }

    /// 更新分析结果的结构相似性分数和方向性（Phase 4 使用）
    pub fn update_analysis_ssim(
        &self,
        result_id: i64,
        ssim_score: f64,
        size_ratio: f64,
        resolution_ratio: f64,
        aspect_diff: f64,
        direction_smaller_resolution: bool,
        direction_smaller_filesize: bool,
    ) -> Result<()> {
        self.conn.execute(
            r#"UPDATE analysis_results
               SET ssim_score = ?1,
                   size_ratio = ?2,
                   resolution_ratio = ?3,
                   aspect_diff = ?4,
                   direction_smaller_resolution = ?5,
                   direction_smaller_filesize = ?6
               WHERE id = ?7"#,
            params![
                ssim_score,
                size_ratio,
                resolution_ratio,
                aspect_diff,
                direction_smaller_resolution,
                direction_smaller_filesize,
                result_id
            ],
        )?;
        Ok(())
    }

    /// 更新分析结果类型（Phase 5 使用）
    pub fn update_analysis_type(&self, result_id: i64, analysis_type: AnalysisType) -> Result<()> {
        self.conn.execute(
            "UPDATE analysis_results SET analysis_type = ?1 WHERE id = ?2",
            params![analysis_type.as_str(), result_id],
        )?;
        Ok(())
    }

    /// 获取需要分类的分析结果（Phase 5 使用）
    pub fn get_results_for_classification(&self, run_id: &str) -> Result<Vec<AnalysisResult>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, run_id, comparison_image_id, analysis_type,
                      primary_match_image_id, all_candidate_ids, candidate_truncated,
                      phash_distance, ssim_score, size_ratio, resolution_ratio,
                      aspect_diff, direction_smaller_resolution, direction_smaller_filesize,
                      algorithm_profile_id, analysis_metadata, computed_at
               FROM analysis_results
               WHERE run_id = ?1 AND analysis_type = 'not_evaluated' AND ssim_score IS NOT NULL"#,
        )?;

        let results = stmt
            .query_map(params![run_id], |row| {
                Ok(AnalysisResult {
                    id: row.get(0)?,
                    run_id: row.get(1)?,
                    comparison_image_id: row.get(2)?,
                    analysis_type: AnalysisType::from_str(&row.get::<_, String>(3)?)
                        .unwrap_or(AnalysisType::NotEvaluated),
                    primary_match_image_id: row.get(4)?,
                    all_candidate_ids: row.get(5)?,
                    candidate_truncated: row.get(6)?,
                    phash_distance: row.get(7)?,
                    ssim_score: row.get(8)?,
                    size_ratio: row.get(9)?,
                    resolution_ratio: row.get(10)?,
                    aspect_diff: row.get(11)?,
                    direction_smaller_resolution: row.get(12)?,
                    direction_smaller_filesize: row.get(13)?,
                    review_status: ReviewStatusType::NotRequired,
                    action_status: ActionStatus::None,
                    reviewed_at: None,
                    reviewer_note: None,
                    algorithm_profile_id: row.get(14)?,
                    analysis_metadata: row.get(15)?,
                    analyzed_at: row.get(16)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(results)
    }

    /// 根据 ID 获取图片
    pub fn get_image_by_id(&self, image_id: i64) -> Result<Option<Image>> {
        self.conn
            .query_row(
                r#"SELECT id, run_id, folder_id, source_role, file_path, relative_path,
                          file_size, file_modified_at, width, height, format,
                          aspect_ratio, frame_count, frame_strategy,
                          blake3_hash, phash, phash_algorithm_version,
                          scan_status, error_message, scanned_at, hash_computed_at
                   FROM images WHERE id = ?1"#,
                params![image_id],
                |row| {
                    Ok(Image {
                        id: row.get(0)?,
                        run_id: row.get(1)?,
                        folder_id: row.get(2)?,
                        source_role: FolderRole::from_str(&row.get::<_, String>(3)?)
                            .unwrap_or(FolderRole::Baseline),
                        file_path: row.get(4)?,
                        relative_path: row.get(5)?,
                        file_size: row.get(6)?,
                        file_modified_at: row.get(7)?,
                        width: row.get(8)?,
                        height: row.get(9)?,
                        format: row.get(10)?,
                        aspect_ratio: row.get(11)?,
                        frame_count: row.get(12)?,
                        frame_strategy: row.get(13)?,
                        blake3_hash: row.get(14)?,
                        phash: row.get(15)?,
                        phash_algorithm_version: row.get(16)?,
                        scan_status: ScanStatus::from_str(&row.get::<_, String>(17)?)
                            .unwrap_or(ScanStatus::Pending),
                        error_message: row.get(18)?,
                        scanned_at: row.get(19)?,
                        hash_computed_at: row.get(20)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }
    pub fn find_baseline_by_phash(
        &self,
        run_id: &str,
        comparison_aspect_ratio: f64,
        aspect_tolerance: f64,
    ) -> Result<Vec<Image>> {
        // 计算宽高比范围
        let min_aspect = comparison_aspect_ratio * (1.0 - aspect_tolerance);
        let max_aspect = comparison_aspect_ratio * (1.0 + aspect_tolerance);

        let mut stmt = self.conn.prepare(
            r#"SELECT id, run_id, folder_id, source_role, file_path, relative_path,
                      file_size, file_modified_at, width, height, format,
                      aspect_ratio, frame_count, frame_strategy,
                      blake3_hash, phash, phash_algorithm_version,
                      scan_status, error_message, scanned_at, hash_computed_at
               FROM images
               WHERE run_id = ?1
                 AND source_role = 'baseline'
                 AND scan_status = 'completed'
                 AND phash IS NOT NULL
                 AND aspect_ratio >= ?2
                 AND aspect_ratio <= ?3"#,
        )?;

        let images = stmt
            .query_map(params![run_id, min_aspect, max_aspect], |row| {
                Ok(Image {
                    id: row.get(0)?,
                    run_id: row.get(1)?,
                    folder_id: row.get(2)?,
                    source_role: FolderRole::from_str(&row.get::<_, String>(3)?)
                        .unwrap_or(FolderRole::Baseline),
                    file_path: row.get(4)?,
                    relative_path: row.get(5)?,
                    file_size: row.get(6)?,
                    file_modified_at: row.get(7)?,
                    width: row.get(8)?,
                    height: row.get(9)?,
                    format: row.get(10)?,
                    aspect_ratio: row.get(11)?,
                    frame_count: row.get(12)?,
                    frame_strategy: row.get(13)?,
                    blake3_hash: row.get(14)?,
                    phash: row.get(15)?,
                    phash_algorithm_version: row.get(16)?,
                    scan_status: ScanStatus::from_str(&row.get::<_, String>(17)?)
                        .unwrap_or(ScanStatus::Pending),
                    error_message: row.get(18)?,
                    scanned_at: row.get(19)?,
                    hash_computed_at: row.get(20)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(images)
    }

    // ========================================================================
    // 分析结果 (AnalysisResults) 相关操作
    // ========================================================================

    /// 插入分析结果
    pub fn insert_analysis_result(&self, result: &AnalysisResultInsert) -> Result<i64> {
        let now = Utc::now().timestamp();
        let candidate_ids_json = result
            .all_candidate_ids
            .as_ref()
            .map(|ids| serde_json::to_string(ids).ok())
            .flatten();

        self.conn.execute(
            r#"INSERT INTO analysis_results (
                run_id, comparison_image_id, analysis_type,
                primary_match_image_id, all_candidate_ids, candidate_truncated,
                phash_distance, ssim_score, size_ratio, resolution_ratio,
                aspect_diff, direction_smaller_resolution, direction_smaller_filesize,
                algorithm_profile_id, analysis_metadata, computed_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)"#,
            params![
                result.run_id,
                result.comparison_image_id,
                result.analysis_type.as_str(),
                result.primary_match_image_id,
                candidate_ids_json,
                result.candidate_truncated,
                result.phash_distance,
                result.ssim_score,
                result.size_ratio,
                result.resolution_ratio,
                result.aspect_diff,
                result.direction_smaller_resolution,
                result.direction_smaller_filesize,
                result.algorithm_profile_id,
                result.analysis_metadata,
                now,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// 获取运行的分析统计
    pub fn get_analysis_stats(&self, run_id: &str) -> Result<ComparisonStats> {
        // 获取基准和对比总数
        let baseline_total: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM images WHERE run_id = ?1 AND source_role = 'baseline'",
            params![run_id],
            |row| row.get(0),
        )?;

        let mut comparison_total: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM images WHERE run_id = ?1 AND source_role = 'comparison'",
            params![run_id],
            |row| row.get(0),
        )?;

        // 内部同目录对比不会创建 source_role='comparison' 的重复图片记录；
        // 此时 analysis_results.comparison_image_id 会引用 baseline 图片。
        // 统计展示应按实际待分析图片数量，而不是按物理 comparison 记录数显示 0。
        let analyzed_image_total: i64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT comparison_image_id) FROM analysis_results WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )?;
        comparison_total = comparison_total.max(analyzed_image_total);

        // 获取各分类统计
        let mut stmt = self.conn.prepare(
            "SELECT analysis_type, COUNT(*) FROM analysis_results WHERE run_id = ?1 GROUP BY analysis_type"
        )?;

        let mut type_counts = std::collections::HashMap::new();
        let rows = stmt.query_map(params![run_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;

        for row in rows {
            let (analysis_type, count) = row?;
            type_counts.insert(analysis_type, count);
        }

        // 获取审核状态统计
        let pending_review: i64 = self
            .conn
            .query_row(
                r#"SELECT COUNT(*) FROM review_status rs
               JOIN analysis_results ar ON rs.analysis_result_id = ar.id
               WHERE ar.run_id = ?1 AND rs.review_status = 'pending'"#,
                params![run_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let approved: i64 = self
            .conn
            .query_row(
                r#"SELECT COUNT(*) FROM review_status rs
               JOIN analysis_results ar ON rs.analysis_result_id = ar.id
               WHERE ar.run_id = ?1 AND rs.review_status = 'approved_for_recycle'"#,
                params![run_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let rejected: i64 = self
            .conn
            .query_row(
                r#"SELECT COUNT(*) FROM review_status rs
               JOIN analysis_results ar ON rs.analysis_result_id = ar.id
               WHERE ar.run_id = ?1 AND rs.review_status = 'rejected_keep'"#,
                params![run_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // 获取操作统计
        let recycled: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM recycle_bin WHERE run_id = ?1 AND restored_at IS NULL AND permanently_deleted_at IS NULL",
            params![run_id],
            |row| row.get(0),
        ).unwrap_or(0);

        let restored: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM recycle_bin WHERE run_id = ?1 AND restored_at IS NOT NULL",
                params![run_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let permanently_deleted: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM recycle_bin WHERE run_id = ?1 AND permanently_deleted_at IS NOT NULL",
            params![run_id],
            |row| row.get(0),
        ).unwrap_or(0);

        Ok(ComparisonStats {
            run_id: run_id.to_string(),
            baseline_total,
            comparison_total,
            exact_duplicate: *type_counts.get("exact_duplicate").unwrap_or(&0),
            likely_compressed: *type_counts.get("likely_compressed").unwrap_or(&0),
            variant: *type_counts.get("variant").unwrap_or(&0),
            similar_keep: *type_counts.get("similar_keep").unwrap_or(&0),
            no_baseline_match: *type_counts.get("no_baseline_match").unwrap_or(&0),
            inconclusive: *type_counts.get("inconclusive").unwrap_or(&0),
            not_evaluated: *type_counts.get("not_evaluated").unwrap_or(&0),
            error: *type_counts.get("error").unwrap_or(&0),
            pending_review,
            approved_for_recycle: approved,
            rejected_keep: rejected,
            recycled,
            restored,
            permanently_deleted,
        })
    }

    /// 获取用于报告的分析结果（带图片路径和审核状态）
    pub fn get_analysis_results_for_report(
        &self,
        run_id: &str,
    ) -> Result<Vec<crate::core::report::ReportResultRow>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT
                ar.comparison_image_id,
                ci.relative_path as comparison_path,
                ar.analysis_type,
                ar.primary_match_image_id,
                bi.relative_path as primary_match_path,
                ar.phash_distance,
                ar.ssim_score,
                ar.size_ratio,
                ar.resolution_ratio,
                ar.aspect_diff,
                ar.direction_smaller_resolution,
                ar.direction_smaller_filesize,
                ar.algorithm_profile_id,
                COALESCE(rs.review_status, 'not_required') AS review_status,
                COALESCE((
                    SELECT ol.operation_type
                    FROM operation_logs ol
                    WHERE ol.analysis_result_id = ar.id
                    ORDER BY ol.created_at DESC, ol.id DESC
                    LIMIT 1
                ), 'none') AS action_status
            FROM analysis_results ar
            JOIN images ci ON ar.comparison_image_id = ci.id
            LEFT JOIN images bi ON ar.primary_match_image_id = bi.id
            LEFT JOIN review_status rs ON ar.id = rs.analysis_result_id
            WHERE ar.run_id = ?1
            ORDER BY ar.id"#,
        )?;

        let rows = stmt.query_map(params![run_id], |row| {
            Ok(crate::core::report::ReportResultRow {
                comparison_image_id: row.get(0)?,
                comparison_path: row.get(1)?,
                analysis_type: crate::db::models::AnalysisType::from_str(&row.get::<_, String>(2)?)
                    .unwrap_or(crate::db::models::AnalysisType::Error),
                primary_match_image_id: row.get(3)?,
                primary_match_path: row.get(4)?,
                phash_distance: row.get(5)?,
                ssim_score: row.get(6)?,
                size_ratio: row.get(7)?,
                resolution_ratio: row.get(8)?,
                aspect_diff: row.get(9)?,
                direction_smaller_resolution: row.get(10)?,
                direction_smaller_filesize: row.get(11)?,
                algorithm_profile_id: row.get(12)?,
                review_status: row.get(13)?,
                action_status: row.get(14)?,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }

        Ok(results)
    }

    // ========================================================================
    // 设置相关操作
    // ========================================================================

    /// 加载设置
    pub fn load_settings(&self) -> Result<Settings> {
        let get_setting = |key: &str, default: &str| -> String {
            self.conn
                .query_row(
                    "SELECT value FROM settings WHERE key = ?1",
                    params![key],
                    |row| row.get(0),
                )
                .unwrap_or_else(|_| default.to_string())
        };

        Ok(Settings {
            default_compressed_ssim_threshold: get_setting(
                "default_compressed_ssim_threshold",
                "0.995",
            )
            .parse()
            .unwrap_or(0.995),
            default_variant_review_lower_bound: get_setting(
                "default_variant_review_lower_bound",
                "0.75",
            )
            .parse()
            .unwrap_or(0.75),
            default_phash_max_distance: get_setting("default_phash_max_distance", "10")
                .parse()
                .unwrap_or(10),
            default_aspect_ratio_tolerance: get_setting("default_aspect_ratio_tolerance", "0.005")
                .parse()
                .unwrap_or(0.005),
            auto_preselect_exact_duplicates: get_setting("auto_preselect_exact_duplicates", "0")
                == "1",
            max_candidate_per_image: get_setting("max_candidate_per_image", "50")
                .parse()
                .unwrap_or(50),
        })
    }

    /// 保存设置
    pub fn save_setting(&self, key: &str, value: &str) -> Result<()> {
        let now = Utc::now().timestamp();
        self.conn.execute(
            "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
            params![key, value, now],
        )?;
        Ok(())
    }

    /// 保存完整设置
    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        self.save_setting(
            "default_compressed_ssim_threshold",
            &settings.default_compressed_ssim_threshold.to_string(),
        )?;
        self.save_setting(
            "default_variant_review_lower_bound",
            &settings.default_variant_review_lower_bound.to_string(),
        )?;
        self.save_setting(
            "default_phash_max_distance",
            &settings.default_phash_max_distance.to_string(),
        )?;
        self.save_setting(
            "default_aspect_ratio_tolerance",
            &settings.default_aspect_ratio_tolerance.to_string(),
        )?;
        self.save_setting(
            "auto_preselect_exact_duplicates",
            if settings.auto_preselect_exact_duplicates {
                "1"
            } else {
                "0"
            },
        )?;
        self.save_setting(
            "max_candidate_per_image",
            &settings.max_candidate_per_image.to_string(),
        )?;
        Ok(())
    }
}

// ============================================================================
// 辅助结构体
// ============================================================================

/// 运行配置（用于创建运行时传入）
pub struct RunConfig {
    pub phash_max_distance: i32,
    pub compressed_ssim_threshold: f64,
    pub variant_review_lower_bound: f64,
    pub aspect_ratio_tolerance: f64,
    pub primary_match_tie_threshold: f64,
    pub supported_formats: Vec<String>,
    pub follow_symlinks: bool,
    pub exclude_patterns: Option<Vec<String>>,
    pub max_workers: i32,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            phash_max_distance: 10,
            compressed_ssim_threshold: 0.995,
            variant_review_lower_bound: 0.75,
            aspect_ratio_tolerance: 0.005,
            primary_match_tie_threshold: 0.001,
            supported_formats: vec![
                "jpg".to_string(),
                "jpeg".to_string(),
                "png".to_string(),
                "webp".to_string(),
                "bmp".to_string(),
                "gif".to_string(),
            ],
            follow_symlinks: false,
            exclude_patterns: Some(vec![".recycle".to_string()]),
            max_workers: 4,
        }
    }
}

impl Repository {
    // ========================================================================
    // 分析结果相关操作（回收站需要）
    // ========================================================================

    /// 获取单个分析结果
    pub fn get_analysis_result(&self, result_id: i64) -> Result<Option<AnalysisResult>> {
        self.conn
            .query_row(
                r#"SELECT
                          ar.id, ar.run_id, ar.comparison_image_id, ar.analysis_type,
                          ar.primary_match_image_id, ar.all_candidate_ids, ar.phash_distance,
                          ar.ssim_score, ar.size_ratio, ar.resolution_ratio, ar.aspect_diff,
                          ar.direction_smaller_resolution, ar.direction_smaller_filesize,
                          ar.candidate_truncated,
                          COALESCE(rs.review_status, 'not_required') AS review_status,
                          COALESCE((
                              SELECT ol.operation_type
                              FROM operation_logs ol
                              WHERE ol.analysis_result_id = ar.id
                              ORDER BY ol.created_at DESC, ol.id DESC
                              LIMIT 1
                          ), 'none') AS action_status,
                          rs.reviewed_at,
                          rs.review_notes,
                          ar.computed_at,
                          ar.algorithm_profile_id,
                          ar.analysis_metadata
                   FROM analysis_results ar
                   LEFT JOIN review_status rs ON rs.analysis_result_id = ar.id
                   WHERE ar.id = ?1"#,
                params![result_id],
                |row| {
                    let action_status_raw: String = row.get(15)?;
                    Ok(AnalysisResult {
                        id: row.get(0)?,
                        run_id: row.get(1)?,
                        comparison_image_id: row.get(2)?,
                        analysis_type: AnalysisType::from_str(&row.get::<_, String>(3)?)
                            .unwrap_or(AnalysisType::Error),
                        primary_match_image_id: row.get(4)?,
                        all_candidate_ids: row.get(5)?,
                        phash_distance: row.get(6)?,
                        ssim_score: row.get(7)?,
                        size_ratio: row.get(8)?,
                        resolution_ratio: row.get(9)?,
                        aspect_diff: row.get(10)?,
                        direction_smaller_resolution: row.get(11)?,
                        direction_smaller_filesize: row.get(12)?,
                        candidate_truncated: row.get(13)?,
                        review_status: ReviewStatusType::from_str(&row.get::<_, String>(14)?)
                            .unwrap_or(ReviewStatusType::NotRequired),
                        action_status: parse_action_status_from_operation(&action_status_raw),
                        reviewed_at: row.get(16)?,
                        reviewer_note: row.get(17)?,
                        analyzed_at: row.get(18)?,
                        algorithm_profile_id: row.get(19)?,
                        analysis_metadata: row.get(20)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// 更新分析结果的操作状态
    pub fn update_result_action_status(
        &self,
        _result_id: i64,
        _status: ActionStatus,
    ) -> Result<()> {
        // 当前 schema 将操作状态记录在 operation_logs 中，而不是 analysis_results 列。
        // 保留此方法作为回收引擎的兼容入口，实际状态由 create_action_log 写入后推导。
        Ok(())
    }

    /// 获取文件夹信息
    pub fn get_folder(&self, folder_id: i64) -> Result<Option<Folder>> {
        self.conn
            .query_row(
                "SELECT id, run_id, path, alias, role, file_count, created_at FROM folders WHERE id = ?1",
                params![folder_id],
                |row| {
                    Ok(Folder {
                        id: row.get(0)?,
                        run_id: row.get(1)?,
                        path: row.get(2)?,
                        alias: row.get(3)?,
                        role: FolderRole::from_str(&row.get::<_, String>(4)?)
                            .unwrap_or(FolderRole::Comparison),
                        file_count: row.get(5)?,
                        created_at: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    // ========================================================================
    // 操作日志相关操作
    // ========================================================================

    /// 创建操作日志
    pub fn create_action_log(
        &self,
        result_id: i64,
        action_type: &str,
        source_path: Option<&str>,
        target_path: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<i64> {
        let now = Utc::now().timestamp();
        let run_id: String = self.conn.query_row(
            "SELECT run_id FROM analysis_results WHERE id = ?1",
            params![result_id],
            |row| row.get(0),
        )?;
        let operation_type = match action_type {
            "failed" => "operation_failed",
            other => other,
        };
        let source_path_value = source_path.unwrap_or("");

        self.conn.execute(
            r#"INSERT INTO operation_logs (
                run_id, analysis_result_id, operation_type, source_path, target_path,
                error_message, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
            params![
                run_id,
                result_id,
                operation_type,
                source_path_value,
                target_path,
                error_message,
                now,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// 获取最后一次指定类型的操作日志
    pub fn get_last_action_log(
        &self,
        result_id: i64,
        action_type: &str,
    ) -> Result<Option<ActionLog>> {
        self.conn
            .query_row(
                r#"SELECT id, analysis_result_id, operation_type, source_path, target_path,
                          error_message, created_at
                   FROM operation_logs
                   WHERE analysis_result_id = ?1 AND operation_type = ?2
                   ORDER BY created_at DESC
                   LIMIT 1"#,
                params![result_id, action_type],
                |row| {
                    Ok(ActionLog {
                        id: row.get(0)?,
                        result_id: row.get(1)?,
                        action_type: row.get(2)?,
                        source_path: row.get(3)?,
                        target_path: row.get(4)?,
                        error_message: row.get(5)?,
                        created_at: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }
}

fn parse_action_status_from_operation(operation_type: &str) -> ActionStatus {
    match operation_type {
        "operation_failed" | "validation_failed" => ActionStatus::Failed,
        other => ActionStatus::from_str(other).unwrap_or(ActionStatus::None),
    }
}

/// 图片插入结构体
pub struct ImageInsert {
    pub run_id: String,
    pub folder_id: i64,
    pub source_role: FolderRole,
    pub file_path: String,
    pub relative_path: String,
    pub file_size: i64,
    pub file_modified_at: i64,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub aspect_ratio: f64,
    pub frame_count: i32,
    pub frame_strategy: String,
}

/// 分析结果插入结构体
pub struct AnalysisResultInsert {
    pub run_id: String,
    pub comparison_image_id: i64,
    pub analysis_type: AnalysisType,
    pub primary_match_image_id: Option<i64>,
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
}

/// 待计算结构相似性的配对
pub struct PendingSsimPair {
    pub result_id: i64,
    pub comparison_image_id: i64,
    pub baseline_image_id: i64,
    pub all_candidate_ids: Vec<i64>,
    pub comparison_path: String,
    pub baseline_path: String,
    pub comparison_size: i64,
    pub baseline_size: i64,
    pub comparison_width: u32,
    pub comparison_height: u32,
    pub baseline_width: u32,
    pub baseline_height: u32,
}
