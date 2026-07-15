# 图片指标测试工具 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在首页两张主任务卡片下加入紧凑入口，打开无持久化的独立图片指标测试窗口，自动计算相对底图的 pHash 距离和当前低精度相似度，并按单卡片请求计算标准窗口式 SSIM。

**Architecture:** Rust 新增无数据库依赖的 `image_metrics` Tauri 命令层，并把标准 SSIM 放在可被正式工作流复用的核心模块；前端新增独立 API、会话状态和 `/image-metrics-test` 路由视图，不进入正式对比 Pinia store。主窗口用固定 label 创建或聚焦 Tauri WebviewWindow；测试窗口拦截系统关闭请求。大图只在后端生成最长边 500px 的 PNG 缩略图，原图只交给 `el-image` 预览。

**Tech Stack:** Vue 3、TypeScript、Element Plus、Vitest、Vue Test Utils、Tauri 2、Rust 2021、`image`、`fast_image_resize`、64 位 pHash。

---

## 文件边界

- Create `src-tauri/src/core/ssim/standard.rs`：标准 11×11 Gaussian-window SSIM 纯算法。
- Modify `src-tauri/src/core/ssim/mod.rs`：导出标准 SSIM 模块。
- Create `src-tauri/src/commands/image_metrics.rs`：元数据/缩略图、低精度配对、标准 SSIM 配对三个无数据库命令。
- Modify `src-tauri/src/commands/mod.rs`：导出命令模块。
- Modify `src-tauri/src/main.rs`：注册三个 Tauri 命令。
- Modify `package.json`、`package-lock.json`、`vite.config.ts`：增加前端测试设施。
- Create `src/api/imageMetrics.ts`：测试工具专用 Tauri API 和返回类型。
- Create `src/features/imageMetrics/session.ts`：可独立测试的导入、底图、队列、过期结果和关闭判断状态。
- Create `src/features/imageMetrics/session.spec.ts`：状态行为测试。
- Create `src/views/ImageMetricsTestView.vue`：独立窗口页面、拖拽、图库卡片、原图预览和关闭确认。
- Create `src/views/ImageMetricsTestView.spec.ts`：关键窗口交互测试。
- Modify `src/router/index.ts`：增加独立窗口专用路由。
- Create `src/features/imageMetrics/window.ts`：创建、查找、显示和聚焦固定 label 的 WebviewWindow。
- Create `src/features/imageMetrics/window.spec.ts`：单实例窗口行为测试。
- Modify `src/views/MainView.vue`：在两张主卡片下加入紧凑入口并调用窗口服务。

另一个对话当前正在修改 `src-tauri/src/commands/comparison.rs`、`src/api/comparison.ts` 和 `src/components/ComparisonGroupDetail.vue`。本计划不修改这三个文件。若其标准 SSIM 核心在执行前已提交，Task 2 先比较接口并直接复用，不创建重复算法。

### Task 1: 建立前端测试运行器

**Files:**
- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `vite.config.ts`
- Create: `src/test/smoke.spec.ts`

- [ ] **Step 1: 添加测试依赖和脚本**

Run:

```powershell
npm install --save-dev vitest@^2.1.9 @vue/test-utils@^2.4.6 happy-dom@^15.11.7
```

在 `package.json` 的 `scripts` 中加入：

```json
"test": "vitest run"
```

- [ ] **Step 2: 配置 Vitest 并写冒烟测试**

把 `vite.config.ts` 的导入和配置扩展为：

```ts
/// <reference types="vitest/config" />
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { resolve } from 'path'

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src')
    }
  },
  test: {
    environment: 'happy-dom',
    clearMocks: true
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ['**/src-tauri/**']
    }
  }
})
```

创建 `src/test/smoke.spec.ts`：

```ts
import { describe, expect, it } from 'vitest'

describe('frontend test runner', () => {
  it('runs TypeScript tests', () => {
    expect(1 + 1).toBe(2)
  })
})
```

- [ ] **Step 3: 运行测试和构建**

Run:

```powershell
npm test -- --run src/test/smoke.spec.ts
npm run build
```

Expected: 冒烟测试 `1 passed`，Vue TypeScript 检查和 Vite 构建成功。

- [ ] **Step 4: 提交测试设施**

```powershell
git add package.json package-lock.json vite.config.ts src/test/smoke.spec.ts
git commit -m "test: add frontend test runner"
```

### Task 2: 用 TDD 实现可复用的标准 SSIM

**Files:**
- Create: `src-tauri/src/core/ssim/standard.rs`
- Modify: `src-tauri/src/core/ssim/mod.rs`

- [ ] **Step 1: 先写失败的标准 SSIM 测试**

创建 `standard.rs`，先只放测试与期望 API：

```rust
use crate::error::Result;
use image::DynamicImage;

pub struct StandardSsim;

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, GrayImage, Luma};

    fn solid(value: u8) -> DynamicImage {
        DynamicImage::ImageLuma8(GrayImage::from_pixel(16, 16, Luma([value])))
    }

    #[test]
    fn identical_images_score_one() {
        let image = solid(127);
        let score = StandardSsim::compute(&image, &image).unwrap();
        assert!((score - 1.0).abs() < 1e-12);
    }

    #[test]
    fn black_and_white_match_analytical_reference() {
        let score = StandardSsim::compute(&solid(0), &solid(255)).unwrap();
        let c1 = (0.01_f64 * 255.0).powi(2);
        let expected = c1 / (255.0_f64.powi(2) + c1);
        assert!((score - expected).abs() < 1e-10, "{score} != {expected}");
    }

    #[test]
    fn local_structure_change_reduces_score() {
        let left = GrayImage::from_fn(32, 32, |x, y| Luma([((x + y) % 256) as u8]));
        let right = GrayImage::from_fn(32, 32, |x, y| Luma([((x * 3 + y) % 256) as u8]));
        let score = StandardSsim::compute(
            &DynamicImage::ImageLuma8(left),
            &DynamicImage::ImageLuma8(right),
        ).unwrap();
        assert!(score < 0.95);
    }
}
```

在 `src-tauri/src/core/ssim/mod.rs` 增加 `pub mod standard;`。

- [ ] **Step 2: 运行测试确认 RED**

Run:

```powershell
cargo test core::ssim::standard --manifest-path src-tauri/Cargo.toml
```

Expected: FAIL，原因是 `StandardSsim::compute` 尚未定义。

- [ ] **Step 3: 实现最小标准窗口式 SSIM**

在 `standard.rs` 的测试上方实现：

```rust
use crate::error::{AppError, Result};
use image::DynamicImage;

const WINDOW_RADIUS: i32 = 5;
const SIGMA: f64 = 1.5;
const C1: f64 = 6.5025;
const C2: f64 = 58.5225;

pub struct StandardSsim;

impl StandardSsim {
    pub fn compute(left: &DynamicImage, right: &DynamicImage) -> Result<f64> {
        if left.width() != right.width() || left.height() != right.height() {
            return Err(AppError::SsimComputation("图片尺寸不匹配".to_string()));
        }
        if left.width() == 0 || left.height() == 0 {
            return Err(AppError::SsimComputation("图片像素为空".to_string()));
        }

        let left = left.to_luma8();
        let right = right.to_luma8();
        let kernel = gaussian_kernel();
        let mut sum = 0.0;

        for y in 0..left.height() {
            for x in 0..left.width() {
                let mut mu_left = 0.0;
                let mut mu_right = 0.0;
                let mut second_left = 0.0;
                let mut second_right = 0.0;
                let mut cross = 0.0;

                for ky in -WINDOW_RADIUS..=WINDOW_RADIUS {
                    for kx in -WINDOW_RADIUS..=WINDOW_RADIUS {
                        let weight = kernel[(ky + WINDOW_RADIUS) as usize]
                            * kernel[(kx + WINDOW_RADIUS) as usize];
                        let px = reflect(x as i32 + kx, left.width()) as u32;
                        let py = reflect(y as i32 + ky, left.height()) as u32;
                        let a = left.get_pixel(px, py)[0] as f64;
                        let b = right.get_pixel(px, py)[0] as f64;
                        mu_left += weight * a;
                        mu_right += weight * b;
                        second_left += weight * a * a;
                        second_right += weight * b * b;
                        cross += weight * a * b;
                    }
                }

                let variance_left = (second_left - mu_left * mu_left).max(0.0);
                let variance_right = (second_right - mu_right * mu_right).max(0.0);
                let covariance = cross - mu_left * mu_right;
                let numerator = (2.0 * mu_left * mu_right + C1) * (2.0 * covariance + C2);
                let denominator = (mu_left * mu_left + mu_right * mu_right + C1)
                    * (variance_left + variance_right + C2);
                sum += numerator / denominator;
            }
        }

        Ok(sum / (left.width() as f64 * left.height() as f64))
    }
}

fn gaussian_kernel() -> [f64; 11] {
    let mut kernel = [0.0; 11];
    let mut sum = 0.0;
    for offset in -WINDOW_RADIUS..=WINDOW_RADIUS {
        let value = (-((offset * offset) as f64) / (2.0 * SIGMA * SIGMA)).exp();
        kernel[(offset + WINDOW_RADIUS) as usize] = value;
        sum += value;
    }
    for value in &mut kernel {
        *value /= sum;
    }
    kernel
}

fn reflect(mut index: i32, length: u32) -> i32 {
    let length = length as i32;
    if length == 1 {
        return 0;
    }
    while index < 0 || index >= length {
        index = if index < 0 { -index - 1 } else { 2 * length - index - 1 };
    }
    index
}
```

- [ ] **Step 4: 验证 GREEN 并格式化**

Run:

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test core::ssim::standard --manifest-path src-tauri/Cargo.toml
```

Expected: 3 个标准 SSIM 测试通过。若 `cargo fmt --check` 失败，先运行 `cargo fmt --manifest-path src-tauri/Cargo.toml` 再重跑。

- [ ] **Step 5: 提交算法核心**

```powershell
git add src-tauri/src/core/ssim/standard.rs src-tauri/src/core/ssim/mod.rs
git commit -m "feat: add standard windowed ssim"
```

### Task 3: 用 TDD 实现无数据库图片指标命令

**Files:**
- Create: `src-tauri/src/commands/image_metrics.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: 写元数据、缩略图和尺寸规则的失败测试**

在新文件 `image_metrics.rs` 中先定义序列化结构和测试期望：

```rust
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestImageInfo {
    pub path: String,
    pub file_name: String,
    pub file_size: u64,
    pub width: u32,
    pub height: u32,
    pub modified_at_ms: u64,
    pub phash: String,
    pub thumbnail_data_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestLowPrecisionResult {
    pub phash_distance: u32,
    pub similarity: f64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestStandardSsimResult {
    pub score: f64,
    pub duration_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};
    use tempfile::tempdir;

    fn write_fixture(path: &std::path::Path, width: u32, height: u32, value: u8) {
        ImageBuffer::from_pixel(width, height, Rgb([value, value, value]))
            .save(path)
            .unwrap();
    }

    #[test]
    fn thumbnail_longest_edge_is_at_most_500() {
        assert_eq!(thumbnail_dimensions(2000, 1000), (500, 250));
        assert_eq!(thumbnail_dimensions(320, 240), (320, 240));
    }

    #[test]
    fn pair_target_uses_smaller_pixel_image_without_512_cap() {
        assert_eq!(pair_target_dimensions((4000, 3000), (1600, 900)), (1600, 900));
        assert_eq!(pair_target_dimensions((100, 200), (200, 100)), (100, 200));
    }

    #[test]
    fn loaded_info_contains_real_thumbnail_and_no_database_state() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("large.png");
        write_fixture(&path, 1000, 600, 80);
        let info = load_test_image_sync(path.to_string_lossy().into_owned()).unwrap();
        assert_eq!((info.width, info.height), (1000, 600));
        assert!(info.thumbnail_data_url.starts_with("data:image/png;base64,"));
        assert!(!info.phash.is_empty());
    }
}
```

- [ ] **Step 2: 运行命令测试确认 RED**

Run:

```powershell
cargo test commands::image_metrics --manifest-path src-tauri/Cargo.toml
```

Expected: FAIL，缺少 `thumbnail_dimensions`、`pair_target_dimensions` 和 `load_test_image_sync`。

- [ ] **Step 3: 实现元数据和真实缩略图**

在结构体之后实现这些同步函数：

```rust
use crate::core::phash::PHashComputer;
use crate::core::ssim::{compute::SsimComputer, resize::ImageResizer, standard::StandardSsim};
use crate::error::{AppError, Result};
use base64::{engine::general_purpose, Engine as _};
use image::ImageOutputFormat;
use std::{io::Cursor, path::Path, time::{Instant, UNIX_EPOCH}};

fn thumbnail_dimensions(width: u32, height: u32) -> (u32, u32) {
    SsimComputer::target_dimensions(width, height, 500)
}

fn pair_target_dimensions(left: (u32, u32), right: (u32, u32)) -> (u32, u32) {
    let left_key = (left.0 as u64 * left.1 as u64, left.0, left.1);
    let right_key = (right.0 as u64 * right.1 as u64, right.0, right.1);
    if left_key <= right_key { left } else { right }
}

fn load_test_image_sync(path: String) -> Result<TestImageInfo> {
    let canonical = std::fs::canonicalize(&path).map_err(|_| AppError::InvalidPath)?;
    let metadata = std::fs::metadata(&canonical)?;
    if !metadata.is_file() {
        return Err(AppError::InvalidPath);
    }
    let image = image::open(&canonical)?;
    let (thumb_width, thumb_height) = thumbnail_dimensions(image.width(), image.height());
    let thumbnail = if (image.width(), image.height()) == (thumb_width, thumb_height) {
        image.clone()
    } else {
        ImageResizer::resize_to_target(&image, thumb_width, thumb_height)?
    };
    let mut png = Cursor::new(Vec::new());
    thumbnail.write_to(&mut png, ImageOutputFormat::Png)?;
    let modified_at_ms = metadata.modified().ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);

    Ok(TestImageInfo {
        path: canonical.to_string_lossy().into_owned(),
        file_name: canonical.file_name().and_then(|value| value.to_str())
            .unwrap_or("未命名图片").to_string(),
        file_size: metadata.len(),
        width: image.width(),
        height: image.height(),
        modified_at_ms,
        phash: PHashComputer::compute_phash(&canonical)?,
        thumbnail_data_url: format!(
            "data:image/png;base64,{}",
            general_purpose::STANDARD.encode(png.into_inner())
        ),
    })
}
```

- [ ] **Step 4: 先写低精度与标准 SSIM 配对失败测试**

在同一测试模块追加：

```rust
#[test]
fn identical_pair_has_zero_phash_distance_and_scores_one() {
    let dir = tempdir().unwrap();
    let left = dir.path().join("left.png");
    let right = dir.path().join("right.png");
    write_fixture(&left, 640, 480, 100);
    write_fixture(&right, 640, 480, 100);
    let left_info = load_test_image_sync(left.to_string_lossy().into_owned()).unwrap();
    let right_info = load_test_image_sync(right.to_string_lossy().into_owned()).unwrap();
    let low = compute_low_precision_sync(
        left_info.path.clone(), right_info.path.clone(),
        left_info.phash, right_info.phash,
    ).unwrap();
    let high = compute_standard_ssim_sync(
        left_info.path, right_info.path,
        left_info.file_size, left_info.modified_at_ms,
        right_info.file_size, right_info.modified_at_ms,
    ).unwrap();
    assert_eq!(low.phash_distance, 0);
    assert!((low.similarity - 1.0).abs() < 1e-12);
    assert!((high.score - 1.0).abs() < 1e-12);
}

#[test]
fn standard_ssim_rejects_a_file_changed_after_import() {
    let dir = tempdir().unwrap();
    let left = dir.path().join("left.png");
    let right = dir.path().join("right.png");
    write_fixture(&left, 32, 32, 100);
    write_fixture(&right, 32, 32, 100);
    let left_info = load_test_image_sync(left.to_string_lossy().into_owned()).unwrap();
    let right_info = load_test_image_sync(right.to_string_lossy().into_owned()).unwrap();
    write_fixture(&right, 64, 64, 80);
    let error = compute_standard_ssim_sync(
        left_info.path, right_info.path,
        left_info.file_size, left_info.modified_at_ms,
        right_info.file_size, right_info.modified_at_ms,
    ).unwrap_err();
    assert!(error.to_string().contains("重新导入"));
}
```

- [ ] **Step 5: 运行测试确认第二次 RED**

Run:

```powershell
cargo test commands::image_metrics --manifest-path src-tauri/Cargo.toml
```

Expected: FAIL，缺少两个配对计算函数。

- [ ] **Step 6: 实现配对归一化与三个异步 Tauri 命令**

```rust
fn normalized_pair(left_path: &Path, right_path: &Path, max_edge: Option<u32>) -> Result<(image::DynamicImage, image::DynamicImage)> {
    let left = image::open(left_path)?;
    let right = image::open(right_path)?;
    let (mut target_width, mut target_height) = pair_target_dimensions(
        (left.width(), left.height()),
        (right.width(), right.height()),
    );
    if let Some(max_edge) = max_edge {
        (target_width, target_height) = SsimComputer::target_dimensions(target_width, target_height, max_edge);
    }
    let normalize = |image: &image::DynamicImage| {
        if (image.width(), image.height()) == (target_width, target_height) {
            Ok(image.clone())
        } else {
            ImageResizer::resize_to_target(image, target_width, target_height)
        }
    };
    Ok((normalize(&left)?, normalize(&right)?))
}

fn compute_low_precision_sync(
    baseline_path: String,
    candidate_path: String,
    baseline_phash: String,
    candidate_phash: String,
) -> Result<TestLowPrecisionResult> {
    let started = Instant::now();
    let (baseline, candidate) = normalized_pair(
        Path::new(&baseline_path), Path::new(&candidate_path), Some(512)
    )?;
    Ok(TestLowPrecisionResult {
        phash_distance: PHashComputer::hamming_distance(&baseline_phash, &candidate_phash)?,
        similarity: SsimComputer::compute(&baseline, &candidate)?,
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

fn compute_standard_ssim_sync(
    baseline_path: String,
    candidate_path: String,
    baseline_file_size: u64,
    baseline_modified_at_ms: u64,
    candidate_file_size: u64,
    candidate_modified_at_ms: u64,
) -> Result<TestStandardSsimResult> {
    let started = Instant::now();
    validate_file_fingerprint(Path::new(&baseline_path), baseline_file_size, baseline_modified_at_ms)?;
    validate_file_fingerprint(Path::new(&candidate_path), candidate_file_size, candidate_modified_at_ms)?;
    let (baseline, candidate) = normalized_pair(
        Path::new(&baseline_path), Path::new(&candidate_path), None
    )?;
    Ok(TestStandardSsimResult {
        score: StandardSsim::compute(&baseline, &candidate)?,
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

fn validate_file_fingerprint(path: &Path, expected_size: u64, expected_modified_at_ms: u64) -> Result<()> {
    let metadata = std::fs::metadata(path)?;
    let modified_at_ms = metadata.modified().ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    if metadata.len() != expected_size || modified_at_ms != expected_modified_at_ms {
        return Err(AppError::ValidationError("图片已发生变化，请移除后重新导入".to_string()));
    }
    Ok(())
}

fn join_error(error: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("图片指标任务执行失败: {error}"))
}

#[tauri::command]
pub async fn load_test_image(path: String) -> Result<TestImageInfo> {
    tauri::async_runtime::spawn_blocking(move || load_test_image_sync(path))
        .await.map_err(join_error)?
}

#[tauri::command]
pub async fn compute_test_low_precision(
    baseline_path: String,
    candidate_path: String,
    baseline_phash: String,
    candidate_phash: String,
) -> Result<TestLowPrecisionResult> {
    tauri::async_runtime::spawn_blocking(move || compute_low_precision_sync(
        baseline_path, candidate_path, baseline_phash, candidate_phash
    )).await.map_err(join_error)?
}

#[tauri::command]
pub async fn compute_test_standard_ssim(
    baseline_path: String,
    candidate_path: String,
    baseline_file_size: u64,
    baseline_modified_at_ms: u64,
    candidate_file_size: u64,
    candidate_modified_at_ms: u64,
) -> Result<TestStandardSsimResult> {
    tauri::async_runtime::spawn_blocking(move || compute_standard_ssim_sync(
        baseline_path, candidate_path,
        baseline_file_size, baseline_modified_at_ms,
        candidate_file_size, candidate_modified_at_ms,
    )).await.map_err(join_error)?
}
```

在 `commands/mod.rs` 增加 `pub mod image_metrics;`，在 `main.rs` 的 handler 注册三个命令。

- [ ] **Step 7: 验证命令测试与全部 Rust 测试**

Run:

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo test commands::image_metrics --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: 图片指标测试通过，全部 Rust 测试无回归。

- [ ] **Step 8: 提交命令层**

```powershell
git add src-tauri/src/commands/image_metrics.rs src-tauri/src/commands/mod.rs src-tauri/src/main.rs
git commit -m "feat: add transient image metrics commands"
```

### Task 4: 用 TDD 实现前端 API 和会话状态

**Files:**
- Create: `src/api/imageMetrics.ts`
- Create: `src/features/imageMetrics/session.ts`
- Create: `src/features/imageMetrics/session.spec.ts`

- [ ] **Step 1: 写会话行为失败测试**

创建 `session.spec.ts`，用可控 Promise 覆盖底图切换和高精度按需行为：

```ts
import { describe, expect, it, vi } from 'vitest'
import { createImageMetricsSession, type ImageMetricsDependencies } from './session'

const image = (path: string) => ({
  path,
  fileName: `${path}.png`,
  fileSize: 100,
  width: 100,
  height: 100,
  modifiedAtMs: 1,
  phash: path,
  thumbnailDataUrl: `data:image/png;base64,${path}`
})

describe('image metrics session', () => {
  it('deduplicates imported canonical paths', async () => {
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => image(path.toLowerCase())),
      computeLow: vi.fn(),
      computeHigh: vi.fn()
    }
    const session = createImageMetricsSession(deps)
    await session.addPaths(['A', 'a'])
    expect(session.items.value).toHaveLength(1)
  })

  it('automatically computes low precision for every non-baseline image', async () => {
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => image(path)),
      computeLow: vi.fn(async () => ({ phashDistance: 2, similarity: 0.9, durationMs: 3 })),
      computeHigh: vi.fn()
    }
    const session = createImageMetricsSession(deps)
    await session.addPaths(['base', 'a', 'b'])
    await session.setBaseline('base')
    expect(deps.computeLow).toHaveBeenCalledTimes(2)
    expect(session.items.value.find((item) => item.path === 'a')?.low.status).toBe('done')
  })

  it('does not start standard ssim until one card requests it', async () => {
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => image(path)),
      computeLow: vi.fn(async () => ({ phashDistance: 0, similarity: 1, durationMs: 1 })),
      computeHigh: vi.fn(async () => ({ score: 0.98, durationMs: 20 }))
    }
    const session = createImageMetricsSession(deps)
    await session.addPaths(['base', 'candidate'])
    await session.setBaseline('base')
    expect(deps.computeHigh).not.toHaveBeenCalled()
    await session.computeHighPrecision('candidate')
    expect(deps.computeHigh).toHaveBeenCalledTimes(1)
  })

  it('reuses a standard ssim result for the same unordered unchanged pair', async () => {
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => image(path)),
      computeLow: vi.fn(async () => ({ phashDistance: 0, similarity: 1, durationMs: 1 })),
      computeHigh: vi.fn(async () => ({ score: 0.98, durationMs: 20 }))
    }
    const session = createImageMetricsSession(deps)
    await session.addPaths(['a', 'b'])
    await session.setBaseline('a')
    await session.computeHighPrecision('b')
    await session.setBaseline('b')
    await session.computeHighPrecision('a')
    expect(deps.computeHigh).toHaveBeenCalledTimes(1)
  })

  it('discards low precision results from an old baseline generation', async () => {
    let resolveFirst!: (value: { phashDistance: number; similarity: number; durationMs: number }) => void
    const first = new Promise<{ phashDistance: number; similarity: number; durationMs: number }>((resolve) => { resolveFirst = resolve })
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => image(path)),
      computeLow: vi.fn().mockReturnValueOnce(first).mockResolvedValue({ phashDistance: 1, similarity: 0.8, durationMs: 2 }),
      computeHigh: vi.fn()
    }
    const session = createImageMetricsSession(deps)
    await session.addPaths(['a', 'b'])
    const oldRun = session.setBaseline('a')
    await session.setBaseline('b')
    resolveFirst({ phashDistance: 9, similarity: 0.1, durationMs: 9 })
    await oldRun
    expect(session.baselinePath.value).toBe('b')
    expect(session.items.value.find((item) => item.path === 'b')?.low.status).toBe('baseline')
  })

  it('requires close confirmation only when the session has content', async () => {
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => image(path)), computeLow: vi.fn(), computeHigh: vi.fn()
    }
    const session = createImageMetricsSession(deps)
    expect(session.hasContent.value).toBe(false)
    await session.addPaths(['a'])
    expect(session.hasContent.value).toBe(true)
    session.reset()
    expect(session.hasContent.value).toBe(false)
  })
})
```

- [ ] **Step 2: 运行测试确认 RED**

Run:

```powershell
npm test -- --run src/features/imageMetrics/session.spec.ts
```

Expected: FAIL，`session` 模块不存在。

- [ ] **Step 3: 实现专用 API**

创建 `src/api/imageMetrics.ts`：

```ts
import { invoke } from '@tauri-apps/api/core'

export interface TestImageInfo {
  path: string
  fileName: string
  fileSize: number
  width: number
  height: number
  modifiedAtMs: number
  phash: string
  thumbnailDataUrl: string
}

export interface TestLowPrecisionResult {
  phashDistance: number
  similarity: number
  durationMs: number
}

export interface TestStandardSsimResult {
  score: number
  durationMs: number
}

export const loadTestImage = (path: string) =>
  invoke<TestImageInfo>('load_test_image', { path })

export const computeTestLowPrecision = (
  baseline: TestImageInfo,
  candidate: TestImageInfo
) => invoke<TestLowPrecisionResult>('compute_test_low_precision', {
  baselinePath: baseline.path,
  candidatePath: candidate.path,
  baselinePhash: baseline.phash,
  candidatePhash: candidate.phash
})

export const computeTestStandardSsim = (
  baseline: TestImageInfo,
  candidate: TestImageInfo
) => invoke<TestStandardSsimResult>('compute_test_standard_ssim', {
  baselinePath: baseline.path,
  candidatePath: candidate.path,
  baselineFileSize: baseline.fileSize,
  baselineModifiedAtMs: baseline.modifiedAtMs,
  candidateFileSize: candidate.fileSize,
  candidateModifiedAtMs: candidate.modifiedAtMs
})
```

- [ ] **Step 4: 实现最小会话状态使测试转绿**

创建 `src/features/imageMetrics/session.ts`。状态必须使用以下公开接口，组件不得绕过它直接写任务状态：

```ts
import { computed, ref } from 'vue'
import type { TestImageInfo, TestLowPrecisionResult, TestStandardSsimResult } from '@/api/imageMetrics'

type MetricState<T> =
  | { status: 'idle' | 'queued' | 'loading'; value?: undefined; error?: undefined }
  | { status: 'done'; value: T; error?: undefined }
  | { status: 'error'; value?: undefined; error: string }
  | { status: 'baseline'; value?: undefined; error?: undefined }

export interface TestImageItem extends TestImageInfo {
  low: MetricState<TestLowPrecisionResult>
  high: MetricState<TestStandardSsimResult>
}

export interface ImageMetricsDependencies {
  loadImage(path: string): Promise<TestImageInfo>
  computeLow(baseline: TestImageInfo, candidate: TestImageInfo): Promise<TestLowPrecisionResult>
  computeHigh(baseline: TestImageInfo, candidate: TestImageInfo): Promise<TestStandardSsimResult>
}

const idle = (): MetricState<never> => ({ status: 'idle' })
const message = (error: unknown) => error instanceof Error ? error.message : String(error)

export function createImageMetricsSession(deps: ImageMetricsDependencies) {
  const items = ref<TestImageItem[]>([])
  const baselinePath = ref<string | null>(null)
  const loadingCount = ref(0)
  const highPrecisionBusy = ref(false)
  const highCache = new Map<string, TestStandardSsimResult>()
  let generation = 0

  const hasContent = computed(() => items.value.length > 0 || loadingCount.value > 0)

  async function addPaths(paths: string[]) {
    for (const path of paths) {
      loadingCount.value += 1
      try {
        const loaded = await deps.loadImage(path)
        if (items.value.some((item) => item.path.toLocaleLowerCase() === loaded.path.toLocaleLowerCase())) continue
        items.value.push({ ...loaded, low: idle(), high: idle() })
      } finally {
        loadingCount.value -= 1
      }
    }
  }

  async function setBaseline(path: string) {
    const baseline = items.value.find((item) => item.path === path)
    if (!baseline) return
    generation += 1
    const run = generation
    baselinePath.value = path
    for (const item of items.value) {
      item.low = item.path === path ? { status: 'baseline' } : { status: 'queued' }
      item.high = item.path === path ? { status: 'baseline' } : idle()
    }
    for (const item of items.value) {
      if (item.path === path || run !== generation) continue
      item.low = { status: 'loading' }
      try {
        const value = await deps.computeLow(baseline, item)
        if (run === generation && baselinePath.value === path) item.low = { status: 'done', value }
      } catch (error) {
        if (run === generation && baselinePath.value === path) item.low = { status: 'error', error: message(error) }
      }
    }
  }

  async function computeHighPrecision(path: string) {
    const baseline = items.value.find((item) => item.path === baselinePath.value)
    const candidate = items.value.find((item) => item.path === path)
    if (!baseline || !candidate || baseline.path === candidate.path || highPrecisionBusy.value) return false
    const cacheKey = [baseline, candidate]
      .map((item) => `${item.path}|${item.fileSize}|${item.modifiedAtMs}`)
      .sort()
      .join('::')
    const cached = highCache.get(cacheKey)
    if (cached) {
      candidate.high = { status: 'done', value: cached }
      return true
    }
    const run = generation
    highPrecisionBusy.value = true
    candidate.high = { status: 'loading' }
    try {
      const value = await deps.computeHigh(baseline, candidate)
      if (run === generation && baselinePath.value === baseline.path) {
        highCache.set(cacheKey, value)
        candidate.high = { status: 'done', value }
      }
    } catch (error) {
      if (run === generation && baselinePath.value === baseline.path) candidate.high = { status: 'error', error: message(error) }
    } finally {
      highPrecisionBusy.value = false
    }
    return true
  }

  function remove(path: string) {
    generation += 1
    items.value = items.value.filter((item) => item.path !== path)
    if (baselinePath.value === path) {
      baselinePath.value = null
      for (const item of items.value) {
        item.low = idle()
        item.high = idle()
      }
    }
  }

  function reset() {
    generation += 1
    items.value = []
    baselinePath.value = null
    loadingCount.value = 0
    highPrecisionBusy.value = false
    highCache.clear()
  }

  return { items, baselinePath, loadingCount, highPrecisionBusy, hasContent, addPaths, setBaseline, computeHighPrecision, remove, reset }
}
```

- [ ] **Step 5: 验证会话测试和构建**

Run:

```powershell
npm test -- --run src/features/imageMetrics/session.spec.ts
npm run build
```

Expected: 6 个会话测试通过，TypeScript 无错误。

- [ ] **Step 6: 提交 API 和状态**

```powershell
git add src/api/imageMetrics.ts src/features/imageMetrics/session.ts src/features/imageMetrics/session.spec.ts
git commit -m "feat: add transient image metrics session"
```

### Task 5: 用 TDD 实现独立窗口页面与关闭拦截

**Files:**
- Create: `src/views/ImageMetricsTestView.vue`
- Create: `src/views/ImageMetricsTestView.spec.ts`
- Modify: `src/router/index.ts`

- [ ] **Step 1: 写关闭确认和高精度按需的失败组件测试**

`ImageMetricsTestView.spec.ts` 使用 `vi.mock('@/api/imageMetrics')` 返回两张固定图片，并 mock 当前 Tauri 窗口。测试至少包含：

```ts
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import ImageMetricsTestView from './ImageMetricsTestView.vue'
import { ElMessageBox } from 'element-plus'

vi.mock('@/api/imageMetrics', () => ({
  loadTestImage: vi.fn(async (path: string) => ({
    path, fileName: `${path}.png`, fileSize: 100, width: 100, height: 100,
    modifiedAtMs: 1, phash: path, thumbnailDataUrl: `data:image/png;base64,${path}`
  })),
  computeTestLowPrecision: vi.fn(async () => ({ phashDistance: 1, similarity: 0.9, durationMs: 2 })),
  computeTestStandardSsim: vi.fn(async () => ({ score: 0.95, durationMs: 10 }))
}))

vi.mock('@tauri-apps/api/core', () => ({ convertFileSrc: (path: string) => `asset://${path}` }))
const closeWindow = vi.fn(async () => undefined)
const onCloseRequested = vi.fn(async () => () => undefined)
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    close: closeWindow,
    onCloseRequested,
    onDragDropEvent: vi.fn(async () => () => undefined)
  })
}))

describe('ImageMetricsTestView', () => {
  beforeEach(() => vi.restoreAllMocks())

  it('closes an empty window without confirmation', async () => {
    const confirm = vi.spyOn(ElMessageBox, 'confirm')
    const wrapper = mount(ImageMetricsTestView)
    await wrapper.get('[data-test="close-window"]').trigger('click')
    expect(confirm).not.toHaveBeenCalled()
    expect(closeWindow).toHaveBeenCalledTimes(1)
  })

  it('keeps a non-empty window when discard confirmation is canceled', async () => {
    vi.spyOn(ElMessageBox, 'confirm').mockRejectedValue('cancel')
    const wrapper = mount(ImageMetricsTestView)
    await wrapper.vm.addImagePathsForTest(['a'])
    await wrapper.get('[data-test="close-window"]').trigger('click')
    expect(closeWindow).not.toHaveBeenCalled()
  })

  it('computes standard ssim only after the card action is clicked', async () => {
    const api = await import('@/api/imageMetrics')
    const wrapper = mount(ImageMetricsTestView)
    await wrapper.vm.addImagePathsForTest(['base', 'candidate'])
    await wrapper.get('[data-test="card-base"]').trigger('click')
    await flushPromises()
    expect(api.computeTestStandardSsim).not.toHaveBeenCalled()
    await wrapper.get('[data-test="high-candidate"]').trigger('click')
    await flushPromises()
    expect(api.computeTestStandardSsim).toHaveBeenCalledTimes(1)
  })
})
```

暴露给测试的 `addImagePathsForTest` 只是在 `<script setup>` 中通过 `defineExpose({ addImagePathsForTest: session.addPaths })` 暴露已有用户行为入口，不新增测试专用生产分支。

- [ ] **Step 2: 运行组件测试确认 RED**

Run:

```powershell
npm test -- --run src/views/ImageMetricsTestView.spec.ts
```

Expected: FAIL，独立窗口视图不存在。

- [ ] **Step 3: 实现窗口页面、系统关闭拦截和路由**

视图顶部包含窗口标题与关闭按钮、四条指标说明，操作栏包含选择图片、清空全部、数量和拖拽提示。它使用当前 Tauri 窗口并拦截系统标题栏关闭请求：

```ts
const appWindow = getCurrentWindow()
const session = createImageMetricsSession({
  loadImage: loadTestImage,
  computeLow: computeTestLowPrecision,
  computeHigh: computeTestStandardSsim
})
const previewUrls = computed(() => session.items.value.map((item) => convertFileSrc(item.path)))

async function chooseImages() {
  const selected = await open({ multiple: true, directory: false, filters: [{ name: '图片', extensions: ['png', 'jpg', 'jpeg', 'webp', 'bmp', 'gif', 'tif', 'tiff'] }] })
  if (Array.isArray(selected)) await session.addPaths(selected)
}

async function confirmDiscard() {
  if (!session.hasContent.value) return true
  try {
    await ElMessageBox.confirm(
      '本次测试内容不会保存，关闭后将全部清空。确定关闭吗？',
      '关闭图片指标测试',
      { confirmButtonText: '确定关闭', cancelButtonText: '继续测试', type: 'warning' }
    )
    return true
  } catch (error) {
    if (error === 'cancel' || error === 'close') return false
    throw error
  }
}

let allowNativeClose = false

async function closeWindow() {
  if (!await confirmDiscard()) return
  session.reset()
  allowNativeClose = true
  await appWindow.close()
}

let unlistenClose: (() => void) | undefined
let unlistenDrop: (() => void) | undefined

onMounted(async () => {
  unlistenClose = await appWindow.onCloseRequested(async (event) => {
    if (allowNativeClose) return
    event.preventDefault()
    await closeWindow()
  })
  unlistenDrop = await appWindow.onDragDropEvent(async (event) => {
    if (event.payload.type === 'drop') await session.addPaths(event.payload.paths)
  })
})

onBeforeUnmount(() => {
  unlistenClose?.()
  unlistenDrop?.()
  session.reset()
})
```

模板的每张卡片必须满足：图片区域 `@click.stop`，卡片自身支持 click、Enter 和 Space 设底图；底图显示文字标签；非底图显示 `pHash 距离`、`低精度`、`高精度` 三行；pHash 距离 tooltip 同时显示底图和当前图片的完整 pHash；高精度行的按钮调用 `session.computeHighPrecision(item.path)`；移除按钮 `@click.stop`。`el-image` 使用缩略图作为 `src`，原图 `previewUrls` 作为 `preview-src-list`，设置 `preview-teleported`。

样式固定使用：

```scss
.metrics-window { height: 100vh; min-height: 0; padding: 16px; display: flex; flex-direction: column; gap: 12px; background: #f5f7fa; }
.metrics-grid { min-height: 0; overflow-y: auto; display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 16px; align-content: start; }
.metrics-card { border: 1px solid #dcdfe6; border-radius: 8px; background: #fff; overflow: hidden; cursor: pointer; }
.metrics-card.is-baseline { border: 2px solid #409eff; background: #ecf5ff; }
.metrics-image { width: 100%; max-height: 500px; object-fit: contain; background: #f5f7fa; display: block; }
.metric-value { font-variant-numeric: tabular-nums; }
```

路由表增加 `/image-metrics-test`，懒加载 `ImageMetricsTestView.vue`。若高精度繁忙时点击另一张卡片，显示 `ElMessage.info('请等待当前高精度计算完成')`。

- [ ] **Step 4: 验证组件测试、全量前端测试与构建**

Run:

```powershell
npm test -- --run src/views/ImageMetricsTestView.spec.ts
npm test
npm run build
```

Expected: 组件测试和全部前端测试通过，构建成功。

- [ ] **Step 5: 提交独立窗口页面**

```powershell
git add src/views/ImageMetricsTestView.vue src/views/ImageMetricsTestView.spec.ts src/router/index.ts
git commit -m "feat: add image metrics test window view"
```

### Task 6: 接入首页并完成回归验证

**Files:**
- Create: `src/features/imageMetrics/window.ts`
- Create: `src/features/imageMetrics/window.spec.ts`
- Modify: `src/views/MainView.vue`
- Create: `src/views/MainView.spec.ts`
- Modify: `README.md`

- [ ] **Step 1: 先写首页入口失败测试**

先创建 `window.spec.ts`，mock `WebviewWindow.getByLabel` 与构造器，断言不存在时创建固定 label，已存在时只 show、unminimize、setFocus。再创建 `MainView.spec.ts`，stub 现有工作台子组件和 Pinia，断言紧凑入口位于两张主任务卡片下方，点击后调用窗口服务：

```ts
import { createPinia } from 'pinia'
import { openImageMetricsWindow } from '@/features/imageMetrics/window'

vi.mock('@/features/imageMetrics/window', () => ({ openImageMetricsWindow: vi.fn() }))

it('opens the independent metrics window from a compact card below the main cards', async () => {
  const wrapper = mount(MainView, {
    global: {
      plugins: [createPinia()],
      stubs: {
        ComparisonDirectorySelector: true,
        ComparisonProgress: true,
        ComparisonResults: true,
        ComparisonGroupDetail: true
      }
    }
  })
  expect(wrapper.findAll('.task-cards .task-card')).toHaveLength(2)
  expect(wrapper.get('[data-test="open-image-metrics"]').classes()).toContain('compact-task-card')
  await wrapper.get('[data-test="open-image-metrics"]').trigger('click')
  expect(openImageMetricsWindow).toHaveBeenCalledTimes(1)
})
```

测试使用真实 `createPinia()` 和组件 stub，不为这个入口断言增加 `@pinia/testing` 依赖。

- [ ] **Step 2: 运行测试确认 RED**

Run:

```powershell
npm test -- --run src/views/MainView.spec.ts
```

Expected: FAIL，窗口服务与紧凑入口不存在。

- [ ] **Step 3: 实现单实例窗口服务和首页紧凑入口**

创建 `src/features/imageMetrics/window.ts`：

```ts
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'

const LABEL = 'image-metrics-test'

export async function openImageMetricsWindow() {
  const existing = await WebviewWindow.getByLabel(LABEL)
  if (existing) {
    await existing.show()
    await existing.unminimize()
    await existing.setFocus()
    return existing
  }
  return new WebviewWindow(LABEL, {
    url: '/image-metrics-test',
    title: 'ImageKeeper - 图片指标测试',
    width: 1180,
    height: 820,
    minWidth: 840,
    minHeight: 600,
    resizable: true,
    center: true
  })
}
```

在 `.task-cards` 结束标签之后增加：

```vue
<button
  type="button"
  class="compact-task-card"
  data-test="open-image-metrics"
  @click="openMetricsTest"
>
  <span class="task-icon"><el-icon><DataAnalysis /></el-icon></span>
  <span class="task-title">图片指标测试</span>
  <span class="task-copy">临时比较多张图片的 pHash、当前相似度与标准 SSIM，不保存记录</span>
</button>
```

脚本引入 `DataAnalysis` 和 `openImageMetricsWindow`，实现 `openMetricsTest()`；创建失败时用 `ElMessage.error` 提示。上方 `.task-cards` 始终保持现有两列，紧凑卡高度约 `96px`、上边距 `16px`，采用横向图标/标题/说明布局；现有 `760px` 断点下改为更紧凑的单列文字布局。

- [ ] **Step 4: 更新 README 的非持久化工具说明**

在使用说明加入一段：图片指标测试是临时诊断工具，pHash/低精度自动，高精度标准 SSIM 按单图计算，关闭后不保存。不要把它描述为正式任务结果或删除依据。

- [ ] **Step 5: 运行最终验证**

Run:

```powershell
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
git diff --check
```

Expected: 所有前端和 Rust 测试通过，生产构建成功，无格式或空白错误。

- [ ] **Step 6: 手工验收大图路径**

Run:

```powershell
npm run tauri:dev
```

依次验证：首页两张大卡下方的紧凑卡创建独立窗口；重复点击只聚焦同一窗口；拖入/选择多图；卡片缩略图不超过 500px；点击图片打开原图预览；选底图后低精度逐张完成；只有点击单卡才计算标准 SSIM；空窗口直接关闭；非空窗口通过系统标题栏关闭时取消可保留状态、确认后重新打开为空。

- [ ] **Step 7: 提交入口和文档**

```powershell
git add src/features/imageMetrics/window.ts src/features/imageMetrics/window.spec.ts src/views/MainView.vue src/views/MainView.spec.ts README.md
git commit -m "feat: expose image metrics test tool"
```

## 完成前检查

- [ ] 对照设计规格逐项确认：无业务数据库写入、真实 500px 缩略图、原图按需预览、唯一底图、低精度自动、高精度单对按需、非空关闭确认。
- [ ] 检查 `git status --short`，确认没有合入另一个对话对 `comparison.rs`、`comparison.ts` 或 `ComparisonGroupDetail.vue` 的未提交改动。
- [ ] 若另一个分支已提供标准 SSIM，确认正式工作流与测试工具最终只保留一套核心实现。
