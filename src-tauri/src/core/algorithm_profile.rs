use rayon::{ThreadPool, ThreadPoolBuilder};
use std::sync::OnceLock;

/// 所有图片解码、pHash 和 SSIM 任务共用的最大并行度下限与上限。
///
/// 实际取 CPU 逻辑核数减一，留一个核给 webview 与 Tauri 主线程；
/// 上限避免超多核机器上 rayon 调度与全局缓存锁的争抢反而成为瓶颈。
pub const MIN_ALGORITHM_WORKER_COUNT: usize = 2;
pub const MAX_ALGORITHM_WORKER_COUNT: usize = 16;
pub const BACKGROUND_ALGORITHM_WORKER_COUNT: usize = 1;

/// 按当前机器的可用并行度推导算法线程池大小。
pub fn algorithm_worker_count() -> usize {
    std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4)
        .saturating_sub(1)
        .clamp(MIN_ALGORITHM_WORKER_COUNT, MAX_ALGORITHM_WORKER_COUNT)
}

/// 当前分析流程使用的算法配置 ID。
///
/// 只要感知哈希、标准结构相似性或归一化口径任一发生变化，
/// 新创建的 run 和 analysis_result 都应写入新的 profile id，历史记录则保留旧 id。
///
/// v4 起正式任务的 SSIM 归一化在「取较小图片尺寸」之后再按 TASK_SSIM_MAX_EDGE 封顶，
/// 与 v3 的全分辨率口径数值不可混用；小工具测试台仍保留全分辨率口径。
pub const CURRENT_ALGORITHM_PROFILE_ID: &str = "imagekeeper-v4-standard-ssim-capped";

static ALGORITHM_POOL: OnceLock<ThreadPool> = OnceLock::new();
static BACKGROUND_ALGORITHM_POOL: OnceLock<ThreadPool> = OnceLock::new();

/// 返回全程序唯一的算法线程池，避免不同入口各自使用不同并行度。
pub fn algorithm_pool() -> &'static ThreadPool {
    ALGORITHM_POOL.get_or_init(|| {
        ThreadPoolBuilder::new()
            .num_threads(algorithm_worker_count())
            .thread_name(|index| format!("imagekeeper-algorithm-{index}"))
            .build()
            .expect("创建图片算法线程池失败")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn algorithm_pool_scales_with_available_parallelism_within_bounds() {
        let worker_count = algorithm_worker_count();
        assert!((MIN_ALGORITHM_WORKER_COUNT..=MAX_ALGORITHM_WORKER_COUNT).contains(&worker_count));
        assert_eq!(algorithm_pool().current_num_threads(), worker_count);
    }

    #[test]
    fn algorithm_pool_leaves_one_core_for_the_interface() {
        // 只在核数足够时才校验「留一核」，双核及以下会被下限兜住。
        let available = std::thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(4);
        if available > MIN_ALGORITHM_WORKER_COUNT + 1 && available <= MAX_ALGORITHM_WORKER_COUNT {
            assert_eq!(algorithm_worker_count(), available - 1);
        }
    }

    #[test]
    fn background_algorithm_pool_uses_one_worker() {
        assert_eq!(BACKGROUND_ALGORITHM_WORKER_COUNT, 1);
        assert_eq!(background_algorithm_pool().current_num_threads(), 1);
    }
}

/// 正式任务历史结果的低优先级补算池，不占用小工具的 4 路实时计算池。
pub fn background_algorithm_pool() -> &'static ThreadPool {
    BACKGROUND_ALGORITHM_POOL.get_or_init(|| {
        ThreadPoolBuilder::new()
            .num_threads(BACKGROUND_ALGORITHM_WORKER_COUNT)
            .thread_name(|index| format!("imagekeeper-background-{index}"))
            .build()
            .expect("创建后台图片算法线程池失败")
    })
}
