use rayon::{ThreadPool, ThreadPoolBuilder};
use std::sync::OnceLock;

/// 所有图片解码、pHash 和 SSIM 任务共用的最大并行度。
pub const ALGORITHM_WORKER_COUNT: usize = 4;

/// 当前分析流程使用的算法配置 ID。
///
/// 只要感知哈希、标准结构相似性或归一化口径任一发生变化，
/// 新创建的 run 和 analysis_result 都应写入新的 profile id，历史记录则保留旧 id。
pub const CURRENT_ALGORITHM_PROFILE_ID: &str = "imagekeeper-v3-standard-ssim-fullres";

static ALGORITHM_POOL: OnceLock<ThreadPool> = OnceLock::new();

/// 返回全程序唯一的算法线程池，避免不同入口各自使用不同并行度。
pub fn algorithm_pool() -> &'static ThreadPool {
    ALGORITHM_POOL.get_or_init(|| {
        ThreadPoolBuilder::new()
            .num_threads(ALGORITHM_WORKER_COUNT)
            .thread_name(|index| format!("imagekeeper-algorithm-{index}"))
            .build()
            .expect("创建图片算法线程池失败")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn algorithm_pool_uses_the_shared_four_worker_limit() {
        assert_eq!(ALGORITHM_WORKER_COUNT, 4);
        assert_eq!(algorithm_pool().current_num_threads(), 4);
    }
}
