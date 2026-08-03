# ImageKeeper 开发约定与经验沉淀

本文件记录长期有效、跨任务可复用的关键决策与经验。一次性需求和临时方案不写在这里。

## 关键约束

### SSIM 归一化口径分两套，不能混用

**触发信号**：改动 SSIM 目标尺寸、缩放策略、灰度转换顺序，或看到分数与历史记录不一致。

**约束**：

| 路径 | 口径 | 入口 |
|---|---|---|
| 正式任务（扫描评分、分组复核） | 取较小图片尺寸，再按 `TASK_SSIM_MAX_EDGE = 1600` 封顶 | `SsimComputer::task_target_dimensions` |
| SSIM 指标测试台、找差分图 | 取较小图片尺寸，不封顶 | `SsimComputer::pair_target_dimensions` |
| 差分预览图 | 1600 上限 | `image_difference.rs` 的 `PREVIEW_MAX_EDGE` |

**原因**：降采样是低通滤波，会抹掉 JPEG 伪影、噪点、锐化差异这类高频信息，使 SSIM 分数**系统性升高**。而原图识别阈值在 0.99 以上是万分之一步进，判别区间极窄，口径变化足以吞掉整个区间。小工具保留全分辨率是为了人工核对真实数值。

**正确做法**：任何影响归一化结果的改动都必须同步升级 `CURRENT_ALGORITHM_PROFILE_ID`，否则 `ensure_current_algorithm_profile` 无法拦住新旧数值混用。缓存 job 与实际计算必须调用同一个口径函数，否则缓存键与计算尺寸错位。

**验证方式**：`cargo test -- ssim`，重点看 `task_and_tool_dimensions_agree_below_the_cap` 与 `task_target_dimensions_cap_large_pairs_while_tool_keeps_full_resolution`。

### pHash 对输入分辨率免疫

**触发信号**：担心"跑原图和跑压缩图 pHash 会不会有差别"。

**结论**：不会有实质差别。`compute_from_image` 第一步就无条件 `resize_exact` 到 32×32（DCT 路径）和 17×16（dHash 路径），原图高频信息在这一步全部丢弃。差异只剩重采样误差累积，落在低频 DCT 上通常 0-2 bit，远小于 `GROUP_SIMILARITY_PAIR_PHASH_MAX_DISTANCE = 24` 的容差。

**已知缺陷（未修）**：`compute_difference_hash` 把 16×16 = 256 个比较结果用 `% 32` 折叠进 32 bit，每 bit 承载 8 个比较的 XOR，判别力大幅损失；且 `y*16+x` 与 `(y+2)*16+x` 相差正好 32 落在同一 bit，导致垂直相隔 2 行的同列差异互相抵消。标准 dHash 应是 9×8 → 64 bit 一一对应。修复需升 `PHASH_ALGORITHM_VERSION` 并重算全部历史 phash。

## 性能经验

### SSIM 内核的热点在数据结构，不在算法

**触发信号**：分组复核慢、状态灯长时间停在"比对中"。

**根因**：内核是逐像素标量浮点循环，热点集中在**内层循环的每像素查找开销**，不是浮点运算本身。

**归因数据**（4000×3000，独立 release 基准实测）：

| 变体 | 耗时 | 说明 |
|---|---:|---|
| HashMap 行缓存 + `get_pixel` | 1.7s | 优化前 |
| 只把 reflect 提到循环外 | 1.47s | reflect 的 while 循环**不是**瓶颈 |
| 环形缓冲 + 切片索引 + 预计算反射表 | 506ms | **3.43x** |

关键是消除每像素 11 次 HashMap 哈希（占约 3/4 耗时），其次是用切片索引替掉 `get_pixel` 的边界检查与地址重算。

**正确做法**：横向统计行按 `source_y % WINDOW_SPAN` 寻址 11 槽环形缓冲，配 `ring_owner` 判重；反射索引预展开成查表。**累加顺序（offset 顺序 + 5 个 stats 分量顺序）一字不能改**，否则破坏 1e-12 精度断言。

**验证方式**：`ring_buffer_slots_never_alias_two_different_source_rows` 穷举 1..200 全部高度验证槽位不别名 —— 这是环形缓冲成立的前提。镜像延拓在边界会**重复**引用同一源行（`height=11, y=0` 得到 `5,4,3,2,1,0,1,2,3,4,5`），重复共享槽位是安全的，真正的风险只有不同源行互相覆盖。改动窗口半径必须重跑这个测试。

### 缓存淘汰必须是 LRU，因为组间从不回收

**触发信号**：处理多个高分辨率分组时反复重新解码同一批图。

**根因**：灰度图缓存只在 run 被删除时按 `run_id` 清理，组间没有任何回收。预算填满后淘汰成为常态，而任意顺序淘汰完全可能踢掉当前活跃组正在用的图、却留着早已结束的组的死条目。

**正确做法**：`SimilarityImageCache` 维护访问序 tick 与增量 `total_bytes`，按最近最少使用淘汰。不要用 `HashMap::keys().next()` 或 `find()` 挑受害者。

### 状态查询的错误必须按组隔离

**触发信号**：改动 `resolve_group_similarity_statuses` 的指纹刷新逻辑。

**陷阱**：为了去重而把全部图片合并成一批刷新时，很容易写成"任一张失败就全部退回 pending"。这会让一张图片被改动导致所有分组的状态灯集体倒退。

**正确做法**：逐张记录 `Result` 存进 `HashMap<i64, Result<ImageSummary>>`，逐组读取时只有包含失败图片的组受影响。

**验证方式**：`similarity_status_keeps_untouched_groups_completed_when_another_group_changes` —— 这个测试就是为了抓这个回归而写的。

## 构建

`[profile.release]` 配了 `lto = "fat"` + `codegen-units = 1`，release 构建约 8 分钟，这是 SSIM 浮点热路径拿跨 crate 内联的代价。**不要加 `panic = "abort"`** —— Tauri command 依赖 unwind 把单个分组的 panic 转成错误返回，改了会让一个组的失败杀掉整个进程。

`[profile.dev.package."*"] opt-level = 2` 是为了让开发态跑真实图片时依赖库不退化到无优化，否则调试 SSIM 基本不可用。
