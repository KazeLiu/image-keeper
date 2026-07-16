// TypeScript 类型定义

/** 图片元数据 */
export interface Image {
  id: number
  filePath: string
  relativePath: string
  fileSize: number
  fileModifiedAt: number
  width: number
  height: number
  format: string
  aspectRatio: number
  blake3Hash?: string
  hashComputedAt?: number
  scanId: number
  scannedAt: number
}

/** 扫描任务 */
export interface Scan {
  id: number
  rootPath: string
  status: ScanStatus
  totalFiles: number
  scannedFiles: number
  hashComputed: number
  lastScannedPath?: string
  createdAt: number
  startedAt?: number
  completedAt?: number
}

/** 扫描状态 */
export enum ScanStatus {
  Pending = 'pending',
  Running = 'running',
  Paused = 'paused',
  Completed = 'completed',
  Cancelled = 'cancelled'
}

/** 完全重复文件 */
export interface Duplicate {
  id: number
  hashGroup: string
  originalImageId: number
  duplicateImageId: number
  status: DeleteStatus
  markedAt: number
}

/** 相似图片配对 */
export interface SimilarPair {
  id: number
  largerImageId: number
  smallerImageId: number
  ssimScore?: number
  sizeRatio: number
  resolutionRatio: number
  isCompressedVersion: boolean
  ssimThreshold?: number
  status: DeleteStatus
  markedAt: number
  computedAt?: number
}

/** 删除状态 */
export enum DeleteStatus {
  Pending = 'pending',
  Recycled = 'recycled',
  Deleted = 'deleted',
  Kept = 'kept',
  Skipped = 'skipped'
}

/** 回收站记录 */
export interface RecycleBinEntry {
  id: number
  originalPath: string
  recycledPath: string
  deleteReason: DeleteReason
  relatedImageId?: number
  duplicateId?: number
  similarPairId?: number
  fileSize: number
  width: number
  height: number
  blake3Hash?: string
  ssimScore?: number
  recycledAt: number
}

/** 删除原因 */
export enum DeleteReason {
  ExactDuplicate = 'exact_duplicate',
  LowerResolution = 'lower_resolution'
}

/** 扫描进度事件 */
export interface ScanProgressEvent {
  scanId: number
  totalFiles: number
  scannedFiles: number
  currentFile: string
  estimatedTimeRemaining?: number
}

/** 哈希进度事件 */
export interface HashProgressEvent {
  scanId: number
  totalFiles: number
  hashedFiles: number
  currentFile: string
}

/** 用户设置 */
export interface Settings {
  ssimThreshold: number
  duplicateKeepStrategy: 'shortest_path' | 'earliest_time' | 'preferred_dir'
  preferredDirectory: string
  autoRecycleDuplicates: boolean
  autoRecycleCompressed: boolean
}

// ============================================================================
// 多目录对比工作流相关类型
// ============================================================================

/** 多目录对比请求 */
export interface MultiCompareRequest {
  baseline_path: string
  comparison_paths: string[]
  directory_options?: DirectoryCompareOption[]
}

/** 目录级对比选项 */
export interface DirectoryCompareOption {
  path: string
  compare_within: boolean
}

/** 文件夹角色 */
export enum FolderRole {
  Baseline = 'baseline',
  Comparison = 'comparison'
}

/** 文件夹信息 */
export interface Folder {
  id: number
  run_id: string
  path: string
  alias: string
  role: FolderRole
  file_count: number
  created_at: number
}

/** 运行状态 */
export enum RunStatus {
  Pending = 'pending',
  Preflight = 'preflight',
  Indexing = 'indexing',
  Matching = 'matching',
  Scoring = 'scoring',
  Resolving = 'resolving',
  ReviewPending = 'review_pending',
  AnalysisComplete = 'analysis_complete',
  ActionInProgress = 'action_in_progress',
  ActionComplete = 'action_complete',
  CompletedWithErrors = 'completed_with_errors',
  Paused = 'paused',
  Canceled = 'canceled',
  Failed = 'failed'
}

/** 运行状态快照 */
export interface RunStatusResponse {
  run_id: string
  status: RunStatus
  completed_at?: number | null
}

/** 对比历史记录行 */
export interface ComparisonRunHistoryItem {
  run_id: string
  status: RunStatus
  baseline_root_path: string
  comparison_root_paths: string[]
  baseline_total: number
  comparison_total: number
  result_count: number
  error_count: number
  created_at: number
  started_at?: number | null
  completed_at?: number | null
}

/** 分析分类（8种） */
export enum AnalysisType {
  ExactDuplicate = 'exact_duplicate',
  LikelyCompressed = 'likely_compressed',
  Variant = 'variant',
  SimilarKeep = 'similar_keep',
  NoBaselineMatch = 'no_baseline_match',
  Inconclusive = 'inconclusive',
  NotEvaluated = 'not_evaluated',
  Error = 'error'
}

/** 审核状态 */
export enum ReviewStatusType {
  NotRequired = 'not_required',
  Pending = 'pending',
  ApprovedForRecycle = 'approved_for_recycle',
  RejectedKeep = 'rejected_keep'
}

/** 对比统计结果 */
export interface ComparisonStats {
  run_id: string
  baseline_total: number
  comparison_total: number
  exact_duplicate: number
  likely_compressed: number
  variant: number
  similar_keep: number
  no_baseline_match: number
  inconclusive: number
  not_evaluated: number
  error: number
  pending_review: number
  approved_for_recycle: number
  rejected_keep: number
  recycled: number
  restored: number
  permanently_deleted: number
}

/** 扫描进度事件（新版多阶段） */
export interface MultiCompareProgressEvent {
  run_id: string
  phase: string // preflight | indexing | matching | scoring | resolving | complete
  total_files: number
  processed_files: number
  current_file?: string
}

/** 分析结果 */
export interface AnalysisResult {
  id: number
  run_id: string
  comparison_image_id: number
  analysis_type: AnalysisType
  primary_match_image_id?: number
  all_candidate_ids?: number[]
  candidate_truncated: boolean
  phash_distance?: number
  ssim_score?: number
  size_ratio?: number
  resolution_ratio?: number
  aspect_diff?: number
  direction_smaller_resolution: boolean
  direction_smaller_filesize: boolean
  algorithm_profile_id: string
  analysis_metadata?: any
  computed_at: number
}

/** 前端展示用分类结果行 */
export interface ComparisonResultRow {
  id: number
  run_id: string
  comparison_image_id: number
  comparison_path: string
  comparison_relative_path: string
  comparison_file_size: number
  comparison_width: number
  comparison_height: number
  analysis_type: AnalysisType
  primary_match_image_id?: number | null
  primary_match_path?: string | null
  primary_match_relative_path?: string | null
  all_candidate_ids?: number[] | null
  candidate_truncated: boolean
  phash_distance?: number | null
  ssim_score?: number | null
  size_ratio?: number | null
  resolution_ratio?: number | null
  aspect_diff?: number | null
  direction_smaller_resolution: boolean
  direction_smaller_filesize: boolean
  algorithm_profile_id: string
  analysis_metadata?: string | null
  computed_at: number
}

/** 感知哈希粗分组 */
export interface ComparisonGroup {
  group_index: number
  representative_image_id: number
  representative_file_name: string
  member_count: number
  has_low_quality_suggestion: boolean
  members: ComparisonGroupMember[]
  manual_merged?: boolean
  manual_group_id?: string
  source_group_indices?: number[]
}

/** 分组内图片行 */
export interface ComparisonGroupMember {
  image_id: number
  file_path: string
  relative_path: string
  file_size: number
  width: number
  height: number
  phash?: string | null
  phash_distance_to_reference?: number | null
  role: string
  role_label: string
  reference_image_id?: number | null
  reference_relative_path?: string | null
  ssim_score?: number | null
  ssim_cluster_key: string
  is_low_quality_suggestion: boolean
}

/** 当前分组内两两图片相似度 */
export interface GroupSimilarityScore {
  left_image_id: number
  right_image_id: number
  ssim_score?: number | null
  error_message?: string | null
}

/** 当前分组交叉验证进度 */
export interface GroupSimilarityProgress {
  request_id: string
  status: 'started' | 'running' | 'completed'
  phase: 'planning' | 'caching' | 'comparing' | 'completed'
  total_pairs: number
  processed_pairs: number
  total_images: number
  processed_images: number
  current_left_image_id?: number | null
  current_right_image_id?: number | null
  current_left_file_name?: string | null
  current_right_file_name?: string | null
  current_image_id?: number | null
  current_image_file_name?: string | null
  cache_hits: number
  image_cache_hits: number
  computed_pairs: number
  skipped_pairs: number
}

/** 按图片回收的单项结果 */
export interface ImageRecycleOutcome {
  image_id: number
  result_id?: number | null
  success: boolean
  error_message?: string | null
}

/** 图片元数据（对比工作流版本） */
export interface ComparisonImage {
  id: number
  run_id: string
  folder_id: number
  source_role: FolderRole
  file_path: string
  relative_path: string
  file_size: number
  file_modified_at: number
  width: number
  height: number
  format: string
  aspect_ratio: number
  frame_count: number
  frame_strategy: string
  blake3_hash?: string
  phash?: string
  phash_algorithm_version?: string
  scan_status: string
  error_message?: string
  scanned_at: number
  hash_computed_at?: number
}
