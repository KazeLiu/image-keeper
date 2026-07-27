# ImageKeeper 项目总结文档

## 项目概述

**ImageKeeper** 是一个基于 Tauri + Vue 3 的桌面应用,用于智能管理和去重图片集合,特别适用于处理大量图片的场景。

### 核心功能

1. **多文件夹对比去重**
   - 支持设定基准文件夹(Baseline)和对比文件夹(Compare)
   - 识别完全重复、压缩版本、差分图三种类型
   - 基于 BLAKE3 哈希 + pHash + SSIM 三重算法

2. **智能相似度分析**
   - pHash (感知哈希) 快速筛选候选图片对
   - SSIM (结构相似度) 精确计算相似程度
   - 自动分类:压缩版本 (SSIM ≥ 0.995)、差分图 (0.75-0.995)

3. **安全删除管理**
   - 回收站机制,支持误删恢复
   - 批量操作与导出报告

---

## 技术栈

### 后端 (Rust + Tauri)

| 组件 | 技术 | 版本 |
|------|------|------|
| 应用框架 | Tauri | 2.x |
| 数据库 | rusqlite | - |
| 图像处理 | image, fast_image_resize | 0.24, 3.0 |
| 哈希计算 | blake3 | 1.5 |
| 感知哈希 | img_hash | 3.2 |
| 并行处理 | rayon | 1.8 |
| 文件遍历 | walkdir | 2.4 |

### 前端 (Vue 3)

| 组件 | 技术 | 版本 |
|------|------|------|
| 框架 | Vue 3 | (latest) |
| 构建工具 | Vite | 5.4.21 |
| UI 组件 | Element Plus | - |
| 图标 | @element-plus/icons-vue | - |

---

## 项目架构

### 后端模块结构

```
src-tauri/src/
├── main.rs                 # 应用入口
├── error.rs                # 统一错误处理
├── db/                     # 数据库层
│   ├── mod.rs              # 数据库连接初始化
│   ├── schema.rs           # 数据库表结构定义
│   ├── models.rs           # 数据模型
│   └── repository.rs       # 数据访问层
├── core/                   # 核心算法
│   ├── scanner/            # 文件扫描
│   │   ├── walker.rs       # 目录遍历
│   │   └── metadata.rs     # 元数据提取
│   ├── hash/               # 哈希计算
│   │   ├── mod.rs          # HashEngine
│   │   └── blake3.rs       # BLAKE3 实现
│   ├── phash/              # 感知哈希
│   │   ├── mod.rs          # pHash 计算
│   │   └── engine.rs       # PHashEngine
│   ├── ssim/               # SSIM 相似度
│   │   ├── mod.rs          # SsimEngine
│   │   ├── resize.rs       # 图片缩放
│   │   └── compute.rs      # SSIM 计算
│   ├── matching/           # 匹配算法
│   │   ├── index.rs        # 索引构建
│   │   └── filter.rs       # 候选对筛选
│   ├── comparison/         # 对比引擎
│   │   └── mod.rs          # ComparisonEngine
│   └── delete/             # 删除管理
│       ├── mod.rs          # DeleteManager
│       └── export.rs       # ExportManager
└── commands/               # Tauri 命令
    ├── scan.rs             # 扫描相关命令
    ├── comparison.rs       # 对比相关命令
    ├── settings.rs         # 设置管理
    └── directory.rs        # 目录树加载
```

### 前端组件结构

```
src/
├── main.ts                 # 应用入口
├── App.vue                 # 根组件
├── components/             # UI 组件
│   ├── DirectoryTree.vue   # 目录树选择器
│   ├── ComparisonPanel.vue # 对比面板
│   └── ResultView.vue      # 结果展示
└── assets/                 # 静态资源
```

---

## 数据库设计

### 核心表结构

#### 1. images - 图片元数据表

```sql
CREATE TABLE images (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT NOT NULL UNIQUE,
    relative_path TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    file_modified_at INTEGER NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    format TEXT NOT NULL,
    aspect_ratio REAL NOT NULL,
    blake3_hash TEXT,           -- BLAKE3 哈希值
    phash TEXT,                 -- 感知哈希值
    hash_computed_at INTEGER,
    scan_id INTEGER NOT NULL,
    folder_id INTEGER,
    scanned_at INTEGER NOT NULL,
    FOREIGN KEY (scan_id) REFERENCES scans(id) ON DELETE CASCADE,
    FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE CASCADE
);
```

#### 2. scans - 扫描任务表

```sql
CREATE TABLE scans (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    root_path TEXT NOT NULL,
    status TEXT NOT NULL,       -- 'pending', 'running', 'completed'
    compare_mode TEXT NOT NULL, -- 'within', 'between'
    total_files INTEGER DEFAULT 0,
    scanned_files INTEGER DEFAULT 0,
    hash_computed INTEGER DEFAULT 0,
    phash_computed INTEGER DEFAULT 0,
    last_scanned_path TEXT,
    created_at INTEGER NOT NULL,
    started_at INTEGER,
    completed_at INTEGER
);
```

#### 3. folders - 文件夹表

```sql
CREATE TABLE folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scan_id INTEGER NOT NULL,
    path TEXT NOT NULL,
    role TEXT NOT NULL,         -- 'baseline', 'compare'
    file_count INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (scan_id) REFERENCES scans(id) ON DELETE CASCADE,
    UNIQUE(scan_id, path)
);
```

#### 4. duplicates - 完全重复表

```sql
CREATE TABLE duplicates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hash_group TEXT NOT NULL,
    original_image_id INTEGER NOT NULL,
    duplicate_image_id INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    marked_at INTEGER NOT NULL,
    FOREIGN KEY (original_image_id) REFERENCES images(id) ON DELETE CASCADE,
    FOREIGN KEY (duplicate_image_id) REFERENCES images(id) ON DELETE CASCADE
);
```

#### 5. similar_pairs - 相似图片对表

```sql
CREATE TABLE similar_pairs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    larger_image_id INTEGER NOT NULL,
    smaller_image_id INTEGER NOT NULL,
    phash_distance INTEGER,
    ssim_score REAL,
    size_ratio REAL NOT NULL,
    resolution_ratio REAL NOT NULL,
    similarity_type TEXT,       -- 'compressed', 'diff', 'similar'
    is_compressed_version INTEGER NOT NULL DEFAULT 0,
    ssim_threshold REAL,
    status TEXT NOT NULL DEFAULT 'pending',
    marked_at INTEGER NOT NULL,
    computed_at INTEGER,
    FOREIGN KEY (larger_image_id) REFERENCES images(id) ON DELETE CASCADE,
    FOREIGN KEY (smaller_image_id) REFERENCES images(id) ON DELETE CASCADE
);
```

#### 6. recycle_bin - 回收站表

```sql
CREATE TABLE recycle_bin (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    original_path TEXT NOT NULL,
    recycled_path TEXT NOT NULL,
    delete_reason TEXT NOT NULL, -- 'exact_duplicate', 'lower_resolution'
    related_image_id INTEGER,
    duplicate_id INTEGER,
    similar_pair_id INTEGER,
    file_size INTEGER NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    blake3_hash TEXT,
    ssim_score REAL,
    recycled_at INTEGER NOT NULL
);
```

---

## 核心算法流程

### Phase 1: 文件扫描与特征提取

```rust
DirectoryWalker::scan_directory(root_path) {
    遍历所有文件 (walkdir)
    ├─ 过滤支持的图片格式 (jpg, png, webp, bmp, gif)
    ├─ 提取元数据 (分辨率、大小、修改时间)
    ├─ 计算 BLAKE3 哈希 (精确去重)
    └─ 计算 pHash (快速筛选)
}
```

### Phase 2: 精确哈希匹配

```rust
HashEngine::find_exact_duplicates() {
    遍历 Baseline 文件夹图片
    ├─ 建立 HashMap<blake3_hash, image_id>
    └─ 对 Compare 文件夹图片查询哈希
        └─ 匹配成功 → 标记为 "完全重复"
}
```

### Phase 3: pHash 主题分组

```rust
PHashEngine::filter_candidates() {
    对于每个 Compare 图片:
        遍历 Baseline 图片:
            计算汉明距离 = hamming_distance(phash1, phash2)
            if 汉明距离 <= 10:
                加入候选相似对
}
```

**汉明距离阈值:**
- `< 5`: 几乎相同
- `5-10`: 同主题可能性高
- `> 15`: 不同图片,跳过

### Phase 4: SSIM 精确计算

```rust
SsimEngine::compute_similarity(img1, img2) {
    // 预检查
    if 分辨率差异 > 20% && 大小差异 > 50%:
        return 不相似
    
    // 缩放到统一尺寸 (fast_image_resize)
    resize_both_to(target_size)
    
    // 计算 SSIM
    ssim_score = calculate_ssim_score()
    
    // 分类判定
    if ssim_score >= 0.995:
        if 分辨率不同 || 大小比例 < 0.9:
            return "压缩版本"
        else:
            return "极度相似"
    else if 0.75 <= ssim_score < 0.995:
        if 分辨率相同 && 大小比例 > 0.95:
            return "差分图"
        else:
            return "相似但不同"
    else:
        return "不相似"
}
```

### Phase 5: 结果汇总与报告

```rust
ComparisonEngine::generate_report() {
    统计结果:
        ├─ 完全重复 (哈希相同)
        ├─ 压缩版本 (SSIM ≥ 0.995)
        ├─ 差分图 (0.75 ≤ SSIM < 0.995)
        └─ 唯一图片
    
    导出格式:
        ├─ JSON (完整数据)
        ├─ CSV (结果清单)
        └─ HTML (可视化报告)
}
```

---

## Tauri 命令接口

### 扫描相关

```rust
#[tauri::command]
async fn start_scan(
    folder_path: String,
    repository: State<Arc<Mutex<Repository>>>
) -> Result<Scan, String>

#[tauri::command]
async fn pause_scan(scan_id: i64) -> Result<(), String>

#[tauri::command]
async fn resume_scan(scan_id: i64) -> Result<(), String>

#[tauri::command]
async fn cancel_scan(scan_id: i64) -> Result<(), String>
```

### 对比相关

```rust
#[tauri::command]
async fn start_multi_compare(
    baseline_path: String,
    compare_paths: Vec<String>,
    repository: State<Arc<Mutex<Repository>>>,
    app_handle: tauri::AppHandle
) -> Result<i64, String>

#[tauri::command]
async fn get_comparison_stats(
    scan_id: i64,
    repository: State<Arc<Mutex<Repository>>>
) -> Result<ComparisonStats, String>
```

### 设置相关

```rust
#[tauri::command]
async fn load_settings(
    repository: State<Arc<Mutex<Repository>>>
) -> Result<Settings, String>

#[tauri::command]
async fn save_settings(
    settings: Settings,
    repository: State<Arc<Mutex<Repository>>>
) -> Result<(), String>
```

### 目录树加载

```rust
#[tauri::command]
async fn load_directory_tree(
    root_path: String
) -> Result<Vec<DirectoryNode>, String>
```

---

## 性能优化策略

### 1. 分阶段过滤

避免全量对比 (10000 × 10000 = 1 亿次),采用分层筛选:

```
Phase 2 (哈希):   10000 次查询 (< 1 秒)
Phase 3 (pHash):  1000 万次汉明距离 (≈ 1 秒)
Phase 4 (SSIM):   仅对 500 对候选计算 (≈ 50 秒)
```

### 2. 并行处理

使用 `rayon` 并行计算哈希和 SSIM:

```rust
use rayon::prelude::*;

file_paths.par_iter()
    .map(|path| compute_hash(path))
    .collect()
```

### 3. 预检查快速跳过

```rust
if resolution_diff > 2x || size_diff > 3x {
    skip_ssim_computation();
}
```

### 4. 索引优化

数据库索引覆盖高频查询:

```sql
CREATE INDEX idx_images_hash ON images(blake3_hash);
CREATE INDEX idx_images_phash ON images(phash);
CREATE INDEX idx_images_size ON images(width, height);
```

---

## 已完成的功能

### 后端核心

- ✅ 数据库表结构设计与初始化
- ✅ 文件扫描与元数据提取
- ✅ BLAKE3 哈希计算
- ✅ pHash 感知哈希计算
- ✅ SSIM 相似度计算
- ✅ 多文件夹对比引擎
- ✅ 匹配算法与候选对筛选
- ✅ 删除管理与回收站机制
- ✅ Tauri 命令接口实现

### 前端基础

- ✅ Vue 3 + Vite 项目搭建
- ✅ Element Plus UI 集成
- ✅ 目录树选择组件
- ✅ Tauri API 调用封装

---

## 待完成功能

### 前端 UI

- ⬜ 对比结果展示面板
- ⬜ 图片预览与对比视图
- ⬜ 批量操作界面 (删除、移动、导出)
- ⬜ 进度条与实时事件推送
- ⬜ 设置页面 (SSIM 阈值、保留策略等)
- ⬜ 报告导出 (JSON/CSV/HTML)

### 后端增强

- ⬜ 差分图组管理
- ⬜ 智能保留策略 (最短路径、首选目录、最高分辨率)
- ⬜ 增量扫描 (仅处理新增/修改文件)
- ✅ pHash 与标准 SSIM 统一使用 4 线程有界算法池
- ⬜ 进度事件推送优化

### 高级特性

- ⬜ 批量操作撤销功能
- ⬜ 自定义筛选规则
- ⬜ 扫描历史管理
- ⬜ 统计图表与报表

---

## 项目配置文件

### package.json

```json
{
  "name": "imagekeeper",
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "tauri": "tauri"
  },
  "dependencies": {
    "@element-plus/icons-vue": "^2.3.1",
    "@tauri-apps/api": "^2",
    "@tauri-apps/plugin-dialog": "^2",
    "@tauri-apps/plugin-fs": "^2",
    "element-plus": "^2.9.1",
    "vue": "^3.5.13"
  },
  "devDependencies": {
    "@vitejs/plugin-vue": "^5.2.1",
    "vite": "^5.4.21"
  }
}
```

### Cargo.toml

```toml
[package]
name = "imagekeeper"
version = "0.1.0"
edition = "2021"

[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-dialog = "2"
tauri-plugin-fs = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rusqlite = { version = "0.32", features = ["bundled"] }
rayon = "1.8"
image = "0.24"
fast_image_resize = "3.0"
blake3 = "1.5"
chrono = { version = "0.4", features = ["serde"] }
anyhow = "1.0"
thiserror = "1.0"
walkdir = "2.4"
dirs = "5.0"
img_hash = { version = "3.2", features = ["image"] }

[features]
default = ["custom-protocol"]
custom-protocol = ["tauri/custom-protocol"]
```

---

## 构建与运行

### 开发模式

```bash
# 安装前端依赖
npm install

# 启动开发服务器
npm run tauri dev
```

### 生产构建

```bash
# 构建前端
npm run build

# 构建 Tauri 应用
npm run tauri build
```

### 数据库位置

Windows: `%LOCALAPPDATA%\ImageKeeper\imagekeeper.db`
macOS: `~/Library/Application Support/ImageKeeper/imagekeeper.db`
Linux: `~/.local/share/ImageKeeper/imagekeeper.db`

---

## 已修复的问题

### 编译错误修复

1. ✅ 修复缺失的 `Emitter` trait 导入
2. ✅ 添加缺失的 `dirs` crate 依赖
3. ✅ 修复 `Repository.conn` 私有字段访问
4. ✅ 修复 `NonZeroU32` 类型转换问题
5. ✅ 清理未使用的导入
6. ✅ 构建前端生成 `dist` 目录
7. ✅ 修复 `rusqlite::Connection` 线程安全问题

### 数据库问题修复

8. ✅ 清理旧数据库文件(缺少 `phash` 列)
9. ✅ 重新初始化数据库表结构

---

## 参考文档

详细设计请参考:
- `IMAGE_COMPARISON_WORKFLOW.md` - 完整的对比去重流程设计

---

## 项目状态

**当前状态:** 后端核心功能已完成,应用可正常启动,等待前端 UI 开发

**构建状态:** ✅ 编译成功 (有 25 个警告,均为未使用代码,不影响运行)

**下一步:** 开发前端对比结果展示界面

---

*文档生成时间: 2026-07-13*
*项目作者: [Your Name]*
