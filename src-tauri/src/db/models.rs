use serde::{Deserialize, Serialize};

/// 图片元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    pub id: i64,
    pub file_path: String,
    pub relative_path: String,
    pub file_size: i64,
    pub file_modified_at: i64,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub aspect_ratio: f64,
    pub blake3_hash: Option<String>,
    pub phash: Option<String>,
    pub hash_computed_at: Option<i64>,
    pub scan_id: i64,
    pub folder_id: Option<i64>,
    pub scanned_at: i64,
}

/// 扫描任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scan {
    pub id: i64,
    pub root_path: String,
    pub status: ScanStatus,
    pub compare_mode: CompareMode,
    pub total_files: u64,
    pub scanned_files: u64,
    pub hash_computed: u64,
    pub phash_computed: u64,
    pub last_scanned_path: Option<String>,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
}

/// 文件夹（多目录支持）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: i64,
    pub scan_id: i64,
    pub path: String,
    pub role: FolderRole,
    pub file_count: u64,
    pub created_at: i64,
}

/// 文件夹角色
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FolderRole {
    Baseline,
    Compare,
}

impl FolderRole {
    pub fn as_str(&self) -> &str {
        match self {
            FolderRole::Baseline => "baseline",
            FolderRole::Compare => "compare",
        }
    }
}

/// 对比模式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CompareMode {
    Within,
    Between,
}

impl CompareMode {
    pub fn as_str(&self) -> &str {
        match self {
            CompareMode::Within => "within",
            CompareMode::Between => "between",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "within" => Some(CompareMode::Within),
            "between" => Some(CompareMode::Between),
            _ => None,
        }
    }
}

/// 扫描状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ScanStatus {
    Pending,
    Running,
    Paused,
    Completed,
    Cancelled,
}

impl ScanStatus {
    pub fn as_str(&self) -> &str {
        match self {
            ScanStatus::Pending => "pending",
            ScanStatus::Running => "running",
            ScanStatus::Paused => "paused",
            ScanStatus::Completed => "completed",
            ScanStatus::Cancelled => "cancelled",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(ScanStatus::Pending),
            "running" => Some(ScanStatus::Running),
            "paused" => Some(ScanStatus::Paused),
            "completed" => Some(ScanStatus::Completed),
            "cancelled" => Some(ScanStatus::Cancelled),
            _ => None,
        }
    }
}

/// 完全重复文件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Duplicate {
    pub id: i64,
    pub hash_group: String,
    pub original_image_id: i64,
    pub duplicate_image_id: i64,
    pub status: DeleteStatus,
    pub marked_at: i64,
}

/// 相似图片配对
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarPair {
    pub id: i64,
    pub larger_image_id: i64,
    pub smaller_image_id: i64,
    pub phash_distance: Option<u32>,
    pub ssim_score: Option<f64>,
    pub size_ratio: f64,
    pub resolution_ratio: f64,
    pub similarity_type: Option<SimilarityType>,
    pub is_compressed_version: bool,
    pub ssim_threshold: Option<f64>,
    pub status: DeleteStatus,
    pub marked_at: i64,
    pub computed_at: Option<i64>,
}

/// 相似度类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SimilarityType {
    Compressed,
    Diff,
    Similar,
}

impl SimilarityType {
    pub fn as_str(&self) -> &str {
        match self {
            SimilarityType::Compressed => "compressed",
            SimilarityType::Diff => "diff",
            SimilarityType::Similar => "similar",
        }
    }
}

/// 删除状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DeleteStatus {
    Pending,
    Recycled,
    Deleted,
    Kept,
    Skipped,
}

impl DeleteStatus {
    pub fn as_str(&self) -> &str {
        match self {
            DeleteStatus::Pending => "pending",
            DeleteStatus::Recycled => "recycled",
            DeleteStatus::Deleted => "deleted",
            DeleteStatus::Kept => "kept",
            DeleteStatus::Skipped => "skipped",
        }
    }
}

/// 回收站记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecycleBinEntry {
    pub id: i64,
    pub original_path: String,
    pub recycled_path: String,
    pub delete_reason: DeleteReason,
    pub related_image_id: Option<i64>,
    pub duplicate_id: Option<i64>,
    pub similar_pair_id: Option<i64>,
    pub file_size: i64,
    pub width: u32,
    pub height: u32,
    pub blake3_hash: Option<String>,
    pub ssim_score: Option<f64>,
    pub recycled_at: i64,
}

/// 删除原因
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeleteReason {
    ExactDuplicate,
    LowerResolution,
}

impl DeleteReason {
    pub fn as_str(&self) -> &str {
        match self {
            DeleteReason::ExactDuplicate => "exact_duplicate",
            DeleteReason::LowerResolution => "lower_resolution",
        }
    }
}

/// 用户设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub ssim_threshold: f64,
    pub duplicate_keep_strategy: String,
    pub preferred_directory: String,
    pub auto_recycle_duplicates: bool,
    pub auto_recycle_compressed: bool,
}

/// 扫描进度事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgressEvent {
    pub scan_id: i64,
    pub total_files: u64,
    pub scanned_files: u64,
    pub current_file: String,
    pub estimated_time_remaining: Option<u64>,
}

/// 哈希进度事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashProgressEvent {
    pub scan_id: i64,
    pub total_files: u64,
    pub hashed_files: u64,
    pub current_file: String,
}

/// pHash 进度事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PHashProgressEvent {
    pub scan_id: i64,
    pub total_files: u64,
    pub phashed_files: u64,
    pub current_file: String,
}

/// 匹配进度事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchProgressEvent {
    pub scan_id: i64,
    pub total_pairs: u64,
    pub processed_pairs: u64,
    pub current_phase: String,
}

/// SSIM 进度事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsimProgressEvent {
    pub scan_id: i64,
    pub total_pairs: u64,
    pub computed_pairs: u64,
    pub current_pair: String,
}
