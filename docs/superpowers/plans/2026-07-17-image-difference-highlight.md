# 图片差异高亮 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在“图片指标测试”中，为标准图与任意对比图生成可调灵敏度的差异预览，以半透明蒙层和轮廓框标出显著差异区域。

**Architecture:** Rust 后端复用现有图片归一化逻辑，将图片限制在适合预览的尺寸后计算 RGB 通道差异、过滤细碎噪点并输出三张 PNG data URL。前端通过一个独立预览状态模块调用 Tauri 命令，在图片卡片中提供“差异高亮”入口，并用弹窗同时展示标准图、对比图和高亮结果。计算按需触发，灵敏度变化后重新计算，错误保留在弹窗内并提供重试。

**Tech Stack:** Rust 2021、`image`、`base64`、Tauri 2、Vue 3、TypeScript、Element Plus、Vitest。

---

## 文件边界

- 新建 `src-tauri/src/core/image_difference.rs`：纯 Rust 差异检测、连通区域过滤、轮廓框绘制与 PNG 编码。
- 修改 `src-tauri/src/core/mod.rs`：注册差异检测模块。
- 修改 `src-tauri/src/core/image_metrics.rs`：向同 crate 的差异模块开放现有归一化函数。
- 修改 `src-tauri/src/commands/image_metrics.rs`：增加异步 Tauri 命令和命令层测试。
- 修改 `src-tauri/src/main.rs`：注册新命令。
- 修改 `src/api/imageMetrics.ts` 与 `src/api/imageMetrics.spec.ts`：定义结果契约并封装 `invoke`。
- 新建 `src/features/imageMetrics/differencePreview.ts` 与 `.spec.ts`：管理弹窗、加载、错误、灵敏度和过期请求。
- 新建 `src/components/image-metrics/DifferenceHighlightDialog.vue`：专注展示预览和交互。
- 修改 `src/views/ImageMetricsTestView.vue` 与 `.spec.ts`：在候选卡片接入入口和弹窗。
- 修改 `README.md`：补充图片指标测试支持差异高亮的说明。

### Task 1: Rust 差异检测核心

**Files:**
- Create: `src-tauri/src/core/image_difference.rs`
- Modify: `src-tauri/src/core/mod.rs`
- Modify: `src-tauri/src/core/image_metrics.rs`

- [ ] **Step 1: 写差异区域测试**

在新模块中先写测试，构造两张小图：一张全白，另一张只在矩形区域填黑。断言默认灵敏度只返回一个区域，区域边界覆盖被修改矩形，并且差异像素比例大于零。

- [ ] **Step 2: 运行测试确认 RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml core::image_difference::tests::detects_one_local_difference_region -- --exact`

Expected: FAIL，原因是 `compute_difference_preview` 或对应结果类型尚不存在。

- [ ] **Step 3: 实现最小差异算法**

实现以下契约：

```rust
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DifferencePreview {
    pub baseline_data_url: String,
    pub candidate_data_url: String,
    pub highlight_data_url: String,
    pub width: u32,
    pub height: u32,
    pub changed_pixel_ratio: f64,
    pub region_count: usize,
}

pub fn compute_difference_preview(
    baseline_path: &Path,
    candidate_path: &Path,
    sensitivity: u8,
) -> Result<DifferencePreview>
```

将灵敏度 `0..=100` 映射到 RGB 最大通道差阈值 `56..=8`。使用 `normalized_pair(..., Some(1600))` 获取同尺寸图片，生成二值差异掩码；8 邻域连通区域小于 `max(9, width * height / 100_000)` 时丢弃。保留区域在候选图上覆盖半透明红色，并绘制黄色矩形轮廓。标准图、候选图和高亮图都编码为 PNG data URL。

- [ ] **Step 4: 增加相同图片和噪点过滤测试**

相同图片断言 `region_count == 0`、`changed_pixel_ratio == 0.0`；单个噪点断言被最小区域规则过滤。

- [ ] **Step 5: 运行核心测试确认 GREEN**

Run: `cargo test --manifest-path src-tauri/Cargo.toml core::image_difference::tests`

Expected: PASS。

### Task 2: Tauri 命令与前端 API

**Files:**
- Modify: `src-tauri/src/commands/image_metrics.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src/api/imageMetrics.ts`
- Modify: `src/api/imageMetrics.spec.ts`

- [ ] **Step 1: 写命令层文件指纹测试**

构造测试图片，导入后修改候选图，再调用同步命令实现，断言返回“图片已发生变化，请移除后重新导入”。

- [ ] **Step 2: 运行命令测试确认 RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml commands::image_metrics::tests::difference_preview_rejects_changed_files -- --exact`

Expected: FAIL，原因是差异预览命令尚不存在。

- [ ] **Step 3: 实现命令并注册**

增加 `compute_test_difference_preview`，参数包含两张图片路径、各自文件大小与修改时间、灵敏度。先复用文件指纹校验，再通过 `spawn_blocking` 调用核心算法；灵敏度超出 `0..=100` 返回验证错误。在 `generate_handler!` 中注册命令。

- [ ] **Step 4: 写 TypeScript invoke 契约测试**

在 `src/api/imageMetrics.spec.ts` 中断言：

```ts
expect(invoke).toHaveBeenCalledWith('compute_test_difference_preview', {
  baselinePath: 'base.png',
  candidatePath: 'candidate.png',
  baselineFileSize: 10,
  baselineModifiedAtMs: 11,
  candidateFileSize: 20,
  candidateModifiedAtMs: 21,
  sensitivity: 50
})
```

- [ ] **Step 5: 运行 API 测试确认 RED**

Run: `npm test -- src/api/imageMetrics.spec.ts`

Expected: FAIL，原因是 `computeTestDifferencePreview` 尚未导出。

- [ ] **Step 6: 实现 TypeScript 类型和 API 封装**

增加 `TestDifferencePreviewResult` 接口，与 Rust camelCase 序列化字段一致；增加 `computeTestDifferencePreview(baseline, candidate, sensitivity)`。

- [ ] **Step 7: 运行命令与 API 测试确认 GREEN**

Run: `cargo test --manifest-path src-tauri/Cargo.toml commands::image_metrics::tests`

Run: `npm test -- src/api/imageMetrics.spec.ts`

Expected: 两条命令均 PASS。

### Task 3: 差异预览状态

**Files:**
- Create: `src/features/imageMetrics/differencePreview.ts`
- Create: `src/features/imageMetrics/differencePreview.spec.ts`

- [ ] **Step 1: 写状态测试**

覆盖三个行为：打开时用默认灵敏度 `50` 请求；调整灵敏度后重新请求当前图片对；较早请求晚返回时不能覆盖新结果。

- [ ] **Step 2: 运行测试确认 RED**

Run: `npm test -- src/features/imageMetrics/differencePreview.spec.ts`

Expected: FAIL，原因是状态模块不存在。

- [ ] **Step 3: 实现最小状态模块**

导出 `createDifferencePreview(loadPreview)`，包含 `visible`、`loading`、`error`、`result`、`baseline`、`candidate`、`sensitivity`，以及 `open`、`refresh`、`retry`、`close`。使用递增 generation 忽略过期结果；关闭不取消后端任务，但清除界面状态。

- [ ] **Step 4: 运行状态测试确认 GREEN**

Run: `npm test -- src/features/imageMetrics/differencePreview.spec.ts`

Expected: PASS。

### Task 4: 图片指标测试交互

**Files:**
- Create: `src/components/image-metrics/DifferenceHighlightDialog.vue`
- Modify: `src/views/ImageMetricsTestView.vue`
- Modify: `src/views/ImageMetricsTestView.spec.ts`

- [ ] **Step 1: 写视图失败测试**

选择标准图后，断言候选卡出现 `data-test="difference-1"`；点击后断言调用差异 API 并出现标题“差异高亮”；标准图卡不显示该按钮。另写错误响应测试，断言弹窗显示错误与“重试”。

- [ ] **Step 2: 运行视图测试确认 RED**

Run: `npm test -- src/views/ImageMetricsTestView.spec.ts`

Expected: FAIL，原因是候选卡没有差异入口。

- [ ] **Step 3: 实现弹窗组件**

弹窗宽度 `92%`、最大宽度 `1500px`。顶部展示“检测到 N 个区域 / 显著差异像素 X%”文本；中部三列分别为标准图、对比图、差异高亮；小窗口下改为单列。滑块明确标注“忽略细碎差异”到“捕捉细微差异”，只在 change 事件触发重算。加载超过 300ms 时使用 Element Plus loading 状态；错误区提供重试按钮。红色不是唯一提示，黄色轮廓框和统计文本同时表达差异。

- [ ] **Step 4: 接入图片卡片**

候选卡的指标区增加带 `View` 图标的“差异高亮”文本按钮；未选择标准图时不显示，正在加载的卡片不可用。点击使用 `.stop`，避免误将候选图切换为标准图。

- [ ] **Step 5: 运行视图测试确认 GREEN**

Run: `npm test -- src/views/ImageMetricsTestView.spec.ts`

Expected: PASS。

### Task 5: 文档与完整验证

**Files:**
- Modify: `README.md`

- [ ] **Step 1: 更新使用说明**

在“临时图片指标测试”章节说明：选定标准图后可对候选图打开差异高亮，结果使用归一化预览、可调灵敏度，并且该功能只用于人工观察，不改变正式分类或删除建议。

- [ ] **Step 2: 运行格式与完整测试**

Run: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`

Run: `npm test`

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Run: `npm run build`

Expected: 全部退出码为 0；允许保留仓库既有编译 warning，但不得新增错误。

- [ ] **Step 3: 检查变更边界**

Run: `git status --short`

Run: `git diff --check`

确认只包含计划列出的源文件、测试、文档，以及 Tauri 构建自动更新的 schema；构建产物和 `node_modules` 不进入提交。

- [ ] **Step 4: 提交功能分支**

```powershell
git add docs/superpowers/plans/2026-07-17-image-difference-highlight.md README.md src src-tauri/src
git commit -m "feat: add image difference highlights"
```

提交后报告分支、工作树绝对路径、commit id，以及合并前需要先将分支 rebase 到包含当前未提交指标统一改动的最新 `main`。
