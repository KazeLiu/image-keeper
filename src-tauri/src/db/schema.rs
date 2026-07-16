use crate::core::algorithm_profile::CURRENT_ALGORITHM_PROFILE_ID;
use crate::error::Result;
use rusqlite::Connection;

/// 数据库初始化 SQL - 符合 IMAGE_COMPARISON_WORKFLOW.md 规范
const SCHEMA_SQL: &str = r#"
-- ============================================================================
-- 运行快照表 (runs)
-- 每次分析创建唯一 runId，保存不可变运行配置
-- ============================================================================
CREATE TABLE IF NOT EXISTS runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL UNIQUE,
    application_version TEXT NOT NULL,
    algorithm_profile_id TEXT NOT NULL,
    baseline_root_path TEXT NOT NULL,
    baseline_root_alias TEXT NOT NULL DEFAULT 'A',
    comparison_root_paths TEXT NOT NULL, -- JSON array
    comparison_root_aliases TEXT NOT NULL, -- JSON array
    phash_max_distance INTEGER NOT NULL DEFAULT 10,
    compressed_ssim_threshold REAL NOT NULL DEFAULT 0.995,
    variant_review_lower_bound REAL NOT NULL DEFAULT 0.75,
    aspect_ratio_tolerance REAL NOT NULL DEFAULT 0.005,
    primary_match_tie_threshold REAL NOT NULL DEFAULT 0.001,
    supported_formats TEXT NOT NULL, -- JSON array
    follow_symlinks INTEGER NOT NULL DEFAULT 0,
    exclude_patterns TEXT, -- JSON array
    max_workers INTEGER NOT NULL DEFAULT 4,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN (
        'pending', 'preflight', 'indexing', 'matching', 'scoring',
        'resolving', 'review_pending', 'analysis_complete',
        'action_in_progress', 'action_complete', 'completed_with_errors',
        'paused', 'canceled', 'failed'
    )),
    total_baseline_files INTEGER DEFAULT 0,
    total_comparison_files INTEGER DEFAULT 0,
    error_count INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL,
    started_at INTEGER,
    completed_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_runs_run_id ON runs(run_id);
CREATE INDEX IF NOT EXISTS idx_runs_status ON runs(status);
CREATE INDEX IF NOT EXISTS idx_runs_created_at ON runs(created_at);

-- ============================================================================
-- 算法配置表 (algorithm_profiles)
-- 版本化的算法配置，用于结果可追溯
-- ============================================================================
CREATE TABLE IF NOT EXISTS algorithm_profiles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id TEXT NOT NULL UNIQUE,
    hash_algorithm TEXT NOT NULL DEFAULT 'blake3',
    phash_algorithm TEXT NOT NULL DEFAULT 'gradient',
    phash_hash_size INTEGER NOT NULL DEFAULT 8,
    ssim_window_size INTEGER NOT NULL DEFAULT 11,
    normalization_version INTEGER NOT NULL DEFAULT 1,
    resize_algorithm TEXT NOT NULL DEFAULT 'lanczos3',
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_algorithm_profiles_profile_id ON algorithm_profiles(profile_id);

-- 插入默认算法配置
INSERT OR IGNORE INTO algorithm_profiles
    (profile_id, hash_algorithm, phash_algorithm, phash_hash_size, ssim_window_size,
     normalization_version, resize_algorithm, created_at)
VALUES
    ('imagekeeper-v1-ssim', 'blake3', 'gradient', 8, 11, 1, 'lanczos3', strftime('%s', 'now'));

-- ============================================================================
-- 文件夹角色表 (folders)
-- 记录每次运行的目录角色和别名
-- ============================================================================
CREATE TABLE IF NOT EXISTS folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL,
    path TEXT NOT NULL,
    alias TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('baseline', 'comparison')),
    file_count INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (run_id) REFERENCES runs(run_id) ON DELETE CASCADE,
    UNIQUE(run_id, path)
);

CREATE INDEX IF NOT EXISTS idx_folders_run_id ON folders(run_id);
CREATE INDEX IF NOT EXISTS idx_folders_role ON folders(role);
CREATE INDEX IF NOT EXISTS idx_folders_path ON folders(path);

-- ============================================================================
-- 图片清单表 (images)
-- 每个文件记录完整元数据和特征
-- ============================================================================
CREATE TABLE IF NOT EXISTS images (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL,
    folder_id INTEGER NOT NULL,
    source_role TEXT NOT NULL CHECK (source_role IN ('baseline', 'comparison')),
    file_path TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    file_modified_at INTEGER NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    format TEXT NOT NULL,
    aspect_ratio REAL NOT NULL,
    frame_count INTEGER DEFAULT 1,
    frame_strategy TEXT DEFAULT 'first_frame',
    blake3_hash TEXT,
    phash TEXT,
    phash_algorithm_version TEXT,
    scan_status TEXT NOT NULL DEFAULT 'pending' CHECK (scan_status IN (
        'pending', 'scanning', 'decoded', 'hash_computed', 'phash_computed',
        'completed', 'decode_failed', 'hash_failed', 'phash_failed', 'error'
    )),
    error_message TEXT,
    scanned_at INTEGER NOT NULL,
    hash_computed_at INTEGER,
    FOREIGN KEY (run_id) REFERENCES runs(run_id) ON DELETE CASCADE,
    FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE CASCADE,
    UNIQUE(run_id, file_path)
);

CREATE INDEX IF NOT EXISTS idx_images_run_id ON images(run_id);
CREATE INDEX IF NOT EXISTS idx_images_folder_id ON images(folder_id);
CREATE INDEX IF NOT EXISTS idx_images_source_role ON images(source_role);
CREATE INDEX IF NOT EXISTS idx_images_blake3_hash ON images(blake3_hash) WHERE blake3_hash IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_images_phash ON images(phash) WHERE phash IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_images_aspect_ratio ON images(aspect_ratio);
CREATE INDEX IF NOT EXISTS idx_images_scan_status ON images(scan_status);

-- ============================================================================
-- 分析结果表 (analysis_results)
-- 每个对比目录文件的唯一分析分类
-- ============================================================================
CREATE TABLE IF NOT EXISTS analysis_results (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL,
    comparison_image_id INTEGER NOT NULL,
    analysis_type TEXT NOT NULL CHECK (analysis_type IN (
        'exact_duplicate', 'likely_compressed', 'variant', 'similar_keep',
        'no_baseline_match', 'inconclusive', 'not_evaluated', 'error'
    )),
    primary_match_image_id INTEGER,
    all_candidate_ids TEXT, -- JSON array of candidate image IDs
    candidate_truncated INTEGER NOT NULL DEFAULT 0,
    phash_distance INTEGER,
    ssim_score REAL,
    size_ratio REAL,
    resolution_ratio REAL,
    aspect_diff REAL,
    direction_smaller_resolution INTEGER DEFAULT 0,
    direction_smaller_filesize INTEGER DEFAULT 0,
    algorithm_profile_id TEXT NOT NULL,
    analysis_metadata TEXT, -- JSON object for extra evidence
    computed_at INTEGER NOT NULL,
    FOREIGN KEY (run_id) REFERENCES runs(run_id) ON DELETE CASCADE,
    FOREIGN KEY (comparison_image_id) REFERENCES images(id) ON DELETE CASCADE,
    FOREIGN KEY (primary_match_image_id) REFERENCES images(id) ON DELETE SET NULL,
    UNIQUE(run_id, comparison_image_id)
);

CREATE INDEX IF NOT EXISTS idx_analysis_results_run_id ON analysis_results(run_id);
CREATE INDEX IF NOT EXISTS idx_analysis_results_comparison_image_id ON analysis_results(comparison_image_id);
CREATE INDEX IF NOT EXISTS idx_analysis_results_analysis_type ON analysis_results(analysis_type);
CREATE INDEX IF NOT EXISTS idx_analysis_results_primary_match ON analysis_results(primary_match_image_id);

-- ============================================================================
-- 审核状态表 (review_status)
-- 人工审核决定，与分析结果分离
-- ============================================================================
CREATE TABLE IF NOT EXISTS review_status (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    analysis_result_id INTEGER NOT NULL,
    review_status TEXT NOT NULL DEFAULT 'not_required' CHECK (review_status IN (
        'not_required', 'pending', 'approved_for_recycle', 'rejected_keep'
    )),
    reviewed_by TEXT,
    review_reason TEXT,
    review_notes TEXT,
    reviewed_at INTEGER,
    FOREIGN KEY (analysis_result_id) REFERENCES analysis_results(id) ON DELETE CASCADE,
    UNIQUE(analysis_result_id)
);

CREATE INDEX IF NOT EXISTS idx_review_status_analysis_result_id ON review_status(analysis_result_id);
CREATE INDEX IF NOT EXISTS idx_review_status_review_status ON review_status(review_status);

-- ============================================================================
-- 操作日志表 (operation_logs)
-- 记录所有文件操作事件，支持 reconcile
-- ============================================================================
CREATE TABLE IF NOT EXISTS operation_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL,
    analysis_result_id INTEGER NOT NULL,
    operation_type TEXT NOT NULL CHECK (operation_type IN (
        'validating', 'prepared', 'recycled', 'restored', 'permanently_deleted',
        'validation_failed', 'operation_failed', 'stale', 'reconciliation_required'
    )),
    source_path TEXT NOT NULL,
    target_path TEXT,
    verification_blake3 TEXT,
    verification_size INTEGER,
    verification_mtime INTEGER,
    error_message TEXT,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (run_id) REFERENCES runs(run_id) ON DELETE CASCADE,
    FOREIGN KEY (analysis_result_id) REFERENCES analysis_results(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_operation_logs_run_id ON operation_logs(run_id);
CREATE INDEX IF NOT EXISTS idx_operation_logs_analysis_result_id ON operation_logs(analysis_result_id);
CREATE INDEX IF NOT EXISTS idx_operation_logs_operation_type ON operation_logs(operation_type);
CREATE INDEX IF NOT EXISTS idx_operation_logs_created_at ON operation_logs(created_at);

-- ============================================================================
-- 回收站表 (recycle_bin)
-- 安全回收机制，按 runId 隔离
-- ============================================================================
CREATE TABLE IF NOT EXISTS recycle_bin (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL,
    analysis_result_id INTEGER NOT NULL,
    original_path TEXT NOT NULL,
    original_relative_path TEXT NOT NULL,
    recycled_path TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    format TEXT NOT NULL,
    blake3_hash TEXT NOT NULL,
    related_baseline_image_id INTEGER,
    analysis_type TEXT NOT NULL,
    ssim_score REAL,
    phash_distance INTEGER,
    can_restore INTEGER NOT NULL DEFAULT 1,
    restore_conflict_checked INTEGER NOT NULL DEFAULT 0,
    recycled_at INTEGER NOT NULL,
    restored_at INTEGER,
    permanently_deleted_at INTEGER,
    FOREIGN KEY (run_id) REFERENCES runs(run_id) ON DELETE CASCADE,
    FOREIGN KEY (analysis_result_id) REFERENCES analysis_results(id) ON DELETE CASCADE,
    FOREIGN KEY (related_baseline_image_id) REFERENCES images(id) ON DELETE SET NULL,
    UNIQUE(run_id, original_path)
);

CREATE INDEX IF NOT EXISTS idx_recycle_bin_run_id ON recycle_bin(run_id);
CREATE INDEX IF NOT EXISTS idx_recycle_bin_analysis_result_id ON recycle_bin(analysis_result_id);
CREATE INDEX IF NOT EXISTS idx_recycle_bin_analysis_type ON recycle_bin(analysis_type);
CREATE INDEX IF NOT EXISTS idx_recycle_bin_recycled_at ON recycle_bin(recycled_at);
CREATE INDEX IF NOT EXISTS idx_recycle_bin_can_restore ON recycle_bin(can_restore);

-- ============================================================================
-- 设置表 (settings)
-- 保留用户配置
-- ============================================================================
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

-- 插入默认设置
INSERT OR IGNORE INTO settings (key, value, updated_at) VALUES
    ('default_compressed_ssim_threshold', '0.995', strftime('%s', 'now')),
    ('default_variant_review_lower_bound', '0.75', strftime('%s', 'now')),
    ('default_phash_max_distance', '10', strftime('%s', 'now')),
    ('default_aspect_ratio_tolerance', '0.005', strftime('%s', 'now')),
    ('auto_preselect_exact_duplicates', '0', strftime('%s', 'now')),
    ('max_candidate_per_image', '50', strftime('%s', 'now'));
"#;

/// 初始化数据库
pub fn initialize_database(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA_SQL)?;
    conn.execute(
        r#"INSERT OR IGNORE INTO algorithm_profiles
            (profile_id, hash_algorithm, phash_algorithm, phash_hash_size, ssim_window_size,
             normalization_version, resize_algorithm, created_at)
           VALUES
            (?1, 'blake3', 'dct-gradient-combined', 8, 11, 1, 'lanczos3', strftime('%s', 'now'))"#,
        [CURRENT_ALGORITHM_PROFILE_ID],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize_database() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_database(&conn).unwrap();

        // 验证核心表是否创建成功
        let table_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN (
                    'runs', 'algorithm_profiles', 'folders', 'images',
                    'analysis_results', 'review_status', 'operation_logs',
                    'recycle_bin', 'settings'
                )",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(table_count, 9, "应该创建 9 个核心表");

        let current_profile_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM algorithm_profiles WHERE profile_id = ?1",
                [CURRENT_ALGORITHM_PROFILE_ID],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(current_profile_count, 1, "应该写入当前算法配置");
    }
}
