/// 当前分析流程使用的算法配置 ID。
///
/// 只要感知哈希、低精度结构相似性、标准结构相似性任一核心口径发生变化，
/// 新创建的 run 和 analysis_result 都应写入新的 profile id，历史记录则保留旧 id。
pub const CURRENT_ALGORITHM_PROFILE_ID: &str = "imagekeeper-v2-phash-ssim";
