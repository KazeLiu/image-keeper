use serde::{Deserialize, Serialize};

// ============================================================================
// 运行快照相关模型
// ============================================================================

/// 运行快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: i64,
    pub run_id: String,
    pub application_version: String,
    pub algorithm_profile_id: String,
    pub baseline_root_path: String,
    pub baseline_root_alias: String,
    pub comparison_root_paths: String,   // JSON array
    pub comparison_root_aliases: String, // JSON array
    pub phash_max_distance: i32,
    pub compressed_ssim_threshold: f64,
    pub variant_review_lower_bound: f64,
    pub aspect_ratio_tolerance: f64,
    pub primary_match_tie_threshold: f64,
    pub supported_formats: String, // JSON array
    pub follow_symlinks: bool,
    pub exclude_patterns: Option<String>, // JSON array
    pub max_workers: i32,
    pub status: RunStatus,
    pub total_baseline_files: i64,
    pub total_comparison_files: i64,
    pub error_count: i64,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
}

/// 运行状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Pending,
    Preflight,
    Indexing,
    Matching,
    Scoring,
    Resolving,
    ReviewPending,
    AnalysisComplete,
    ActionInProgress,
    ActionComplete,
    CompletedWithErrors,
    Paused,
    Canceled,
    Failed,
}

impl RunStatus {
    pub fn as_str(&self) -> &str {
        match self {
            RunStatus::Pending => "pending",
            RunStatus::Preflight => "preflight",
            RunStatus::Indexing => "indexing",
            RunStatus::Matching => "matching",
            RunStatus::Scoring => "scoring",
            RunStatus::Resolving => "resolving",
            RunStatus::ReviewPending => "review_pending",
            RunStatus::AnalysisComplete => "analysis_complete",
            RunStatus::ActionInProgress => "action_in_progress",
            RunStatus::ActionComplete => "action_complete",
            RunStatus::CompletedWithErrors => "completed_with_errors",
            RunStatus::Paused => "paused",
            RunStatus::Canceled => "canceled",
            RunStatus::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(RunStatus::Pending),
            "preflight" => Some(RunStatus::Preflight),
            "indexing" => Some(RunStatus::Indexing),
            "matching" => Some(RunStatus::Matching),
            "scoring" => Some(RunStatus::Scoring),
            "resolving" => Some(RunStatus::Resolving),
            "review_pending" => Some(RunStatus::ReviewPending),
            "analysis_complete" => Some(RunStatus::AnalysisComplete),
            "action_in_progress" => Some(RunStatus::ActionInProgress),
            "action_complete" => Some(RunStatus::ActionComplete),
            "completed_with_errors" => Some(RunStatus::CompletedWithErrors),
            "paused" => Some(RunStatus::Paused),
            "canceled" => Some(RunStatus::Canceled),
            "failed" => Some(RunStatus::Failed),
            _ => None,
        }
    }
}

/// 算法配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlgorithmProfile {
    pub id: i64,
    pub profile_id: String,
    pub hash_algorithm: String,
    pub phash_algorithm: String,
    pub phash_hash_size: i32,
    pub ssim_window_size: i32,
    pub normalization_version: i32,
    pub resize_algorithm: String,
    pub created_at: i64,
}

// ============================================================================
// 文件夹和图片相关模型
// ============================================================================

/// 文件夹角色
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: i64,
    pub run_id: String,
    pub path: String,
    pub alias: String,
    pub role: FolderRole,
    pub file_count: i64,
    pub created_at: i64,
}

/// 文件夹角色枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FolderRole {
    Baseline,
    Comparison,
}

impl FolderRole {
    pub fn as_str(&self) -> &str {
        match self {
            FolderRole::Baseline => "baseline",
            FolderRole::Comparison => "comparison",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "baseline" => Some(FolderRole::Baseline),
            "comparison" => Some(FolderRole::Comparison),
            _ => None,
        }
    }
}

/// 图片元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    pub id: i64,
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
    pub blake3_hash: Option<String>,
    pub phash: Option<String>,
    pub phash_algorithm_version: Option<String>,
    pub scan_status: ScanStatus,
    pub error_message: Option<String>,
    pub scanned_at: i64,
    pub hash_computed_at: Option<i64>,
}

/// 扫描状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ScanStatus {
    Pending,
    Scanning,
    Decoded,
    HashComputed,
    PhashComputed,
    Completed,
    DecodeFailed,
    HashFailed,
    PhashFailed,
    Error,
}

impl ScanStatus {
    pub fn as_str(&self) -> &str {
        match self {
            ScanStatus::Pending => "pending",
            ScanStatus::Scanning => "scanning",
            ScanStatus::Decoded => "decoded",
            ScanStatus::HashComputed => "hash_computed",
            ScanStatus::PhashComputed => "phash_computed",
            ScanStatus::Completed => "completed",
            ScanStatus::DecodeFailed => "decode_failed",
            ScanStatus::HashFailed => "hash_failed",
            ScanStatus::PhashFailed => "phash_failed",
            ScanStatus::Error => "error",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(ScanStatus::Pending),
            "scanning" => Some(ScanStatus::Scanning),
            "decoded" => Some(ScanStatus::Decoded),
            "hash_computed" => Some(ScanStatus::HashComputed),
            "phash_computed" => Some(ScanStatus::PhashComputed),
            "completed" => Some(ScanStatus::Completed),
            "decode_failed" => Some(ScanStatus::DecodeFailed),
            "hash_failed" => Some(ScanStatus::HashFailed),
            "phash_failed" => Some(ScanStatus::PhashFailed),
            "error" => Some(ScanStatus::Error),
            _ => None,
        }
    }
}

// ============================================================================
// 分析结果相关模型
// ============================================================================

/// 分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub id: i64,
    pub run_id: String,
    pub comparison_image_id: i64,
    pub analysis_type: AnalysisType,
    pub primary_match_image_id: Option<i64>,
    pub all_candidate_ids: Option<String>, // JSON array
    pub candidate_truncated: bool,
    pub phash_distance: Option<i32>,
    pub ssim_score: Option<f64>,
    pub size_ratio: Option<f64>,
    pub resolution_ratio: Option<f64>,
    pub aspect_diff: Option<f64>,
    pub direction_smaller_resolution: bool,
    pub direction_smaller_filesize: bool,
    pub review_status: ReviewStatusType,
    pub action_status: ActionStatus,
    pub reviewed_at: Option<i64>,
    pub reviewer_note: Option<String>,
    pub algorithm_profile_id: String,
    pub analysis_metadata: Option<String>, // JSON object
    pub analyzed_at: i64,
}

/// 分析分类（8种）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisType {
    ExactDuplicate,
    LikelyCompressed,
    Variant,
    SimilarKeep,
    NoBaselineMatch,
    Inconclusive,
    NotEvaluated,
    Error,
}

impl AnalysisType {
    pub fn as_str(&self) -> &str {
        match self {
            AnalysisType::ExactDuplicate => "exact_duplicate",
            AnalysisType::LikelyCompressed => "likely_compressed",
            AnalysisType::Variant => "variant",
            AnalysisType::SimilarKeep => "similar_keep",
            AnalysisType::NoBaselineMatch => "no_baseline_match",
            AnalysisType::Inconclusive => "inconclusive",
            AnalysisType::NotEvaluated => "not_evaluated",
            AnalysisType::Error => "error",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "exact_duplicate" => Some(AnalysisType::ExactDuplicate),
            "likely_compressed" => Some(AnalysisType::LikelyCompressed),
            "variant" => Some(AnalysisType::Variant),
            "similar_keep" => Some(AnalysisType::SimilarKeep),
            "no_baseline_match" => Some(AnalysisType::NoBaselineMatch),
            "inconclusive" => Some(AnalysisType::Inconclusive),
            "not_evaluated" => Some(AnalysisType::NotEvaluated),
            "error" => Some(AnalysisType::Error),
            _ => None,
        }
    }
}

/// 审核状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewStatus {
    pub id: i64,
    pub analysis_result_id: i64,
    pub review_status: ReviewStatusType,
    pub reviewed_by: Option<String>,
    pub review_reason: Option<String>,
    pub review_notes: Option<String>,
    pub reviewed_at: Option<i64>,
}

/// 审核状态类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatusType {
    NotRequired,
    Pending,
    ApprovedForRecycle,
    RejectedKeep,
}

impl ReviewStatusType {
    pub fn as_str(&self) -> &str {
        match self {
            ReviewStatusType::NotRequired => "not_required",
            ReviewStatusType::Pending => "pending",
            ReviewStatusType::ApprovedForRecycle => "approved_for_recycle",
            ReviewStatusType::RejectedKeep => "rejected_keep",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "not_required" => Some(ReviewStatusType::NotRequired),
            "pending" => Some(ReviewStatusType::Pending),
            "approved_for_recycle" => Some(ReviewStatusType::ApprovedForRecycle),
            "rejected_keep" => Some(ReviewStatusType::RejectedKeep),
            _ => None,
        }
    }
}

// ============================================================================
// 操作日志和回收站模型
// ============================================================================

/// 操作状态（action_status 字段）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
    None,
    Validating,
    Prepared,
    Recycled,
    Restored,
    PermanentlyDeleted,
    Stale,
    Failed,
    ReconciliationRequired,
}

impl ActionStatus {
    pub fn as_str(&self) -> &str {
        match self {
            ActionStatus::None => "none",
            ActionStatus::Validating => "validating",
            ActionStatus::Prepared => "prepared",
            ActionStatus::Recycled => "recycled",
            ActionStatus::Restored => "restored",
            ActionStatus::PermanentlyDeleted => "permanently_deleted",
            ActionStatus::Stale => "stale",
            ActionStatus::Failed => "failed",
            ActionStatus::ReconciliationRequired => "reconciliation_required",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "none" => Some(ActionStatus::None),
            "validating" => Some(ActionStatus::Validating),
            "prepared" => Some(ActionStatus::Prepared),
            "recycled" => Some(ActionStatus::Recycled),
            "restored" => Some(ActionStatus::Restored),
            "permanently_deleted" => Some(ActionStatus::PermanentlyDeleted),
            "stale" => Some(ActionStatus::Stale),
            "failed" => Some(ActionStatus::Failed),
            "reconciliation_required" => Some(ActionStatus::ReconciliationRequired),
            _ => None,
        }
    }
}

/// 操作日志
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionLog {
    pub id: i64,
    pub result_id: i64,
    pub action_type: String,
    pub source_path: Option<String>,
    pub target_path: Option<String>,
    pub error_message: Option<String>,
    pub created_at: i64,
}

/// 旧的操作日志结构（保持兼容）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationLog {
    pub id: i64,
    pub run_id: String,
    pub analysis_result_id: i64,
    pub operation_type: OperationType,
    pub source_path: String,
    pub target_path: Option<String>,
    pub verification_blake3: Option<String>,
    pub verification_size: Option<i64>,
    pub verification_mtime: Option<i64>,
    pub error_message: Option<String>,
    pub created_at: i64,
}

/// 操作类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OperationType {
    Validating,
    Prepared,
    Recycled,
    Restored,
    PermanentlyDeleted,
    ValidationFailed,
    OperationFailed,
    Stale,
    ReconciliationRequired,
}

impl OperationType {
    pub fn as_str(&self) -> &str {
        match self {
            OperationType::Validating => "validating",
            OperationType::Prepared => "prepared",
            OperationType::Recycled => "recycled",
            OperationType::Restored => "restored",
            OperationType::PermanentlyDeleted => "permanently_deleted",
            OperationType::ValidationFailed => "validation_failed",
            OperationType::OperationFailed => "operation_failed",
            OperationType::Stale => "stale",
            OperationType::ReconciliationRequired => "reconciliation_required",
        }
    }
}

/// 回收站记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecycleBinEntry {
    pub id: i64,
    pub run_id: String,
    pub analysis_result_id: i64,
    pub original_path: String,
    pub original_relative_path: String,
    pub recycled_path: String,
    pub file_size: i64,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub blake3_hash: String,
    pub related_baseline_image_id: Option<i64>,
    pub analysis_type: AnalysisType,
    pub ssim_score: Option<f64>,
    pub phash_distance: Option<i32>,
    pub can_restore: bool,
    pub restore_conflict_checked: bool,
    pub recycled_at: i64,
    pub restored_at: Option<i64>,
    pub permanently_deleted_at: Option<i64>,
}

// ============================================================================
// 设置和进度事件
// ============================================================================

/// 用户设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub default_compressed_ssim_threshold: f64,
    pub default_variant_review_lower_bound: f64,
    pub default_phash_max_distance: i32,
    pub default_aspect_ratio_tolerance: f64,
    pub auto_preselect_exact_duplicates: bool,
    pub max_candidate_per_image: i32,
}

/// 扫描进度事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgressEvent {
    pub run_id: String,
    pub phase: String,
    pub total_files: i64,
    pub processed_files: i64,
    pub current_file: Option<String>,
}

/// 匹配进度事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchProgressEvent {
    pub run_id: String,
    pub phase: String,
    pub total_pairs: u64,
    pub processed_pairs: u64,
    pub current_info: String,
}

// ============================================================================
// 统计汇总模型
// ============================================================================

/// 对比统计结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonStats {
    pub run_id: String,
    pub baseline_total: i64,
    pub comparison_total: i64,
    pub exact_duplicate: i64,
    pub likely_compressed: i64,
    pub variant: i64,
    pub similar_keep: i64,
    pub no_baseline_match: i64,
    pub inconclusive: i64,
    pub not_evaluated: i64,
    pub error: i64,
    pub pending_review: i64,
    pub approved_for_recycle: i64,
    pub rejected_keep: i64,
    pub recycled: i64,
    pub restored: i64,
    pub permanently_deleted: i64,
}
