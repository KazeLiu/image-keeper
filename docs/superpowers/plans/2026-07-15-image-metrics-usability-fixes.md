# 图片指标测试易用性修复 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 消除批量导入的假死感，修复 Windows 标题栏关闭，并把图片卡片压缩为 200px 缩略图和单行指标。

**Architecture:** 前端会话层增加全会话三并发信号量和稳定导入序号，选择完成后一次性增加全部加载占位；独立窗口关闭回调同步阻止第一次系统关闭，再异步执行现有确认流程，避免关闭事件等待自身。Rust 只把真实缩略图上限改为 200px；标准 SSIM 继续使用较小原图完整分辨率，不读取卡片缩略图。

**Tech Stack:** Vue 3、TypeScript、Element Plus、Vitest、Vue Test Utils、Tauri 2、Rust 2021、`image`。

---

### Task 1: 有界并发导入与即时占位

**Files:**
- Modify: `src/features/imageMetrics/session.spec.ts`
- Modify: `src/features/imageMetrics/session.ts`
- Modify: `src/views/ImageMetricsTestView.spec.ts`

- [ ] **Step 1: 写失败测试**

在 `session.spec.ts` 增加测试：调用 `addPaths(['a', 'b', 'c', 'd'])` 后同步观察 `loadingCount === 4`；用受控 Promise 断言同时执行数最多为 `3`；乱序完成后 `items` 仍为 `a,b,c,d`。

```ts
const importing = session.addPaths(['a', 'b', 'c', 'd'])
expect(session.loadingCount.value).toBe(4)
await vi.waitFor(() => expect(deps.loadImage).toHaveBeenCalledTimes(3))
resolve('c'); await vi.waitFor(() => expect(deps.loadImage).toHaveBeenCalledTimes(4))
resolve('d'); resolve('b'); resolve('a')
await importing
expect(maxActive).toBe(3)
expect(session.items.value.map((item) => item.path)).toEqual(['a', 'b', 'c', 'd'])
```

- [ ] **Step 2: 运行测试确认 RED**

Run: `npm test -- src/features/imageMetrics/session.spec.ts`

Expected: FAIL，串行实现只启动一张，初始 `loadingCount` 为 `1`。

- [ ] **Step 3: 实现全会话三并发信号量**

在 `session.ts` 增加 `IMPORT_CONCURRENCY = 3`、等待队列和稳定序号。`addPaths` 先一次性增加 `paths.length`，每条路径通过 permit 执行，完成后按 `importOrder` 排序。

```ts
const IMPORT_CONCURRENCY = 3
let activeImports = 0
const importWaiters: Array<() => void> = []

async function acquireImportPermit() {
  if (activeImports < IMPORT_CONCURRENCY) { activeImports += 1; return }
  await new Promise<void>((resolve) => importWaiters.push(resolve))
}

function releaseImportPermit() {
  const next = importWaiters.shift()
  if (next) next()
  else activeImports = Math.max(0, activeImports - 1)
}
```

生命周期代次不匹配时丢弃结果，但仍在 `finally` 中释放 permit 并减少加载数。

- [ ] **Step 4: 运行测试确认 GREEN**

Run: `npm test -- src/features/imageMetrics/session.spec.ts src/views/ImageMetricsTestView.spec.ts`

Expected: 会话和窗口组件测试通过，选择完成后全部骨架卡片立即出现。

- [ ] **Step 5: 提交**

```powershell
git add src/features/imageMetrics/session.ts src/features/imageMetrics/session.spec.ts src/views/ImageMetricsTestView.spec.ts
git commit -m "fix: parallelize image metrics imports"
```

### Task 2: 修复 Windows 标题栏关闭流程

**Files:**
- Modify: `src/views/ImageMetricsTestView.spec.ts`
- Modify: `src/views/ImageMetricsTestView.vue`

- [ ] **Step 1: 写失败测试**

新增断言：非空窗口的系统关闭 handler 同步调用 `preventDefault()` 并立即返回 `undefined`，确认 Promise 解决后才调用 `appWindow.close()`。

```ts
const result = windowMocks.closeHandler?.({ preventDefault })
expect(result).toBeUndefined()
expect(preventDefault).toHaveBeenCalledTimes(1)
expect(windowMocks.close).not.toHaveBeenCalled()
resolveConfirm('confirm')
await flushPromises()
expect(windowMocks.close).toHaveBeenCalledTimes(1)
```

- [ ] **Step 2: 运行测试确认 RED**

Run: `npm test -- src/views/ImageMetricsTestView.spec.ts`

Expected: FAIL，当前 `async` 关闭 handler 返回 Promise，并在回调内等待第二次关闭。

- [ ] **Step 3: 实现非阻塞系统关闭回调**

```ts
unlistenClose = await appWindow.onCloseRequested((event) => {
  if (allowNativeClose) return
  event.preventDefault()
  void requestClose()
})
```

- [ ] **Step 4: 运行测试确认 GREEN 并提交**

Run: `npm test -- src/views/ImageMetricsTestView.spec.ts`

```powershell
git add src/views/ImageMetricsTestView.vue src/views/ImageMetricsTestView.spec.ts
git commit -m "fix: restore native image metrics close"
```

### Task 3: 200px 缩略图与单行指标卡片

**Files:**
- Modify: `src-tauri/src/commands/image_metrics.rs`
- Modify: `src/views/ImageMetricsTestView.vue`
- Modify: `src/views/ImageMetricsTestView.spec.ts`

- [ ] **Step 1: 写失败测试**

Rust 将缩略图期望改为 `(200, 100)`；Vue 断言非底图只有一个 `.metrics-inline`，其中同时包含三个指标，并且 pHash 显示整数而非 `/ 64`。

```rust
assert_eq!(thumbnail_dimensions(2000, 1000), (200, 100));
```

```ts
const metrics = wrapper.get('[data-test="card-1"] .metrics-inline')
expect(metrics.text()).toContain('pHash 距离：1')
expect(metrics.text()).not.toContain('/ 64')
expect(metrics.text()).toContain('低精度 SSIM：0.900000')
expect(metrics.text()).toContain('标准 SSIM：点击计算')
```

- [ ] **Step 2: 运行测试确认 RED**

Run: `cargo test thumbnail_longest_edge_is_at_most_200 --manifest-path src-tauri/Cargo.toml` 和 `npm test -- src/views/ImageMetricsTestView.spec.ts`

Expected: Rust 仍返回 500px；Vue 仍渲染三行和 `/ 64`。

- [ ] **Step 3: 实现紧凑卡片**

把 `thumbnail_dimensions` 上限改为 `200`。模板将三个指标合并到 `.metrics-inline`，低精度失败及标准 SSIM 未计算仍保留按钮。CSS 默认两列并允许窄窗口换行：

```scss
.metrics-grid { grid-template-columns: repeat(auto-fill, minmax(480px, 1fr)); gap: 12px; }
.image-wrap { min-height: 120px; max-height: 200px; }
.metrics-image,
.metrics-image :deep(.el-image__inner) { max-height: 200px; }
.metrics-inline { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; font-size: 12px; }
```

顶部文案明确：`标准 SSIM 不使用 200px 缩略图或 512px 限制；仅将较大原图缩小到较小原图的完整分辨率。`

- [ ] **Step 4: 完整验证**

```powershell
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
git diff --check
```

Expected: 所有前端/Rust 测试通过，生产构建成功，无格式或空白错误。

- [ ] **Step 5: 提交**

```powershell
git add src-tauri/src/commands/image_metrics.rs src/views/ImageMetricsTestView.vue src/views/ImageMetricsTestView.spec.ts
git commit -m "refactor: compact image metrics cards"
```
