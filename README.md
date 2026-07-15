# ImageKeeper

二次元原图库整理工具 - 专注于原图保护的智能图片管理软件

## 项目简介

ImageKeeper 不是传统的图片查重软件,而是一款专门用于整理二次元原图图库的企业级桌面应用。

**核心原则**: 宁可漏删一万张图片,也绝不误删任何一张原图。

## 技术栈

### 前端
- Vue 3 (Composition API + `<script setup>`)
- TypeScript (严格模式)
- Element Plus
- Pinia
- Vue Router

### 后端
- Tauri V2
- Rust
- SQLite
- BLAKE3 哈希
- 图片处理 (image, fast_image_resize)

## 支持格式

- JPG/JPEG
- PNG
- WebP
- AVIF
- BMP
- GIF (静态第一帧)

## 核心功能

### 1. 智能扫描
- 递归扫描指定目录
- 提取图片元数据 (尺寸、格式、修改时间等)
- 计算 BLAKE3 哈希
- 断点续扫支持

### 2. 精准去重
- **完全重复文件**: BLAKE3 哈希完全一致
- **压缩版本识别**: 
  - 长宽比一致
  - 小图尺寸严格小于大图
  - 小图文件大小更小
  - SSIM 相似度超过阈值 (默认 0.995)

### 3. 安全删除
- 文件先移动到 `.recycle` 回收站
- 支持恢复误删文件
- 用户最终确认后永久删除

### 4. 导出功能
- `delete-list.txt`: 相对路径列表,方便同步删除云端文件
- `report.csv`: 详细报告 (保留文件、删除文件、删除原因、Hash、SSIM等)

## 项目结构

```
ImageKeeper/
├── src/                      # Vue 前端源码
│   ├── components/           # Vue 组件
│   │   ├── DirectoryTree.vue      # 目录树
│   │   ├── ComparisonDirectorySelector.vue # 多目录选择
│   │   ├── ComparisonProgress.vue # 对比进度与统计
│   │   ├── ComparisonResults.vue  # 对比结果
│   │   ├── ImageGrid.vue          # 图片网格
│   │   └── ImagePreview.vue       # 图片预览
│   ├── stores/               # Pinia 状态管理
│   │   ├── scanStore.ts          # 扫描状态
│   │   ├── imageStore.ts         # 图片数据
│   │   ├── settingsStore.ts      # 用户设置
│   │   └── deleteStore.ts        # 删除管理
│   ├── views/                # 页面视图
│   │   ├── MainView.vue          # 标准首页（多目录对比工作台）
│   │   ├── SettingsView.vue      # 设置页面
│   │   └── ExportView.vue        # 导出页面
│   ├── types/                # TypeScript 类型定义
│   ├── router/               # Vue Router 配置
│   ├── App.vue               # 根组件
│   └── main.ts               # 入口文件
├── src-tauri/                # Rust 后端源码
│   ├── src/
│   │   ├── db/                   # 数据库层
│   │   │   ├── models.rs         # 数据模型
│   │   │   ├── schema.rs         # 数据库模式
│   │   │   ├── repository.rs     # 数据访问层
│   │   │   └── mod.rs
│   │   ├── core/                 # 核心业务逻辑
│   │   │   ├── scanner/          # 扫描模块
│   │   │   │   ├── metadata.rs   # 元数据提取
│   │   │   │   ├── walker.rs     # 目录遍历
│   │   │   │   └── mod.rs
│   │   │   ├── hash/             # 哈希模块
│   │   │   │   ├── blake3.rs     # BLAKE3 计算
│   │   │   │   └── mod.rs
│   │   │   ├── matching/         # 匹配模块
│   │   │   │   ├── index.rs      # 尺寸索引
│   │   │   │   └── mod.rs
│   │   │   ├── ssim/             # SSIM 模块
│   │   │   │   ├── resize.rs     # 图片缩放
│   │   │   │   ├── compute.rs    # SSIM 计算
│   │   │   │   └── mod.rs
│   │   │   ├── delete/           # 删除模块
│   │   │   │   ├── export.rs     # 导出功能
│   │   │   │   └── mod.rs
│   │   │   └── mod.rs
│   │   ├── commands/             # Tauri 命令
│   │   │   ├── scan.rs           # 扫描命令
│   │   │   ├── settings.rs       # 设置命令
│   │   │   └── mod.rs
│   │   ├── error.rs              # 错误类型
│   │   └── main.rs               # 入口文件
│   ├── Cargo.toml
│   └── tauri.conf.json
├── package.json
├── tsconfig.json
├── vite.config.ts
└── index.html
```

## 开发指南

### 安装依赖

```bash
npm install
```

### 开发模式

```bash
npm run tauri:dev
```

### 构建

```bash
npm run tauri:build
```

## 数据库设计

### 核心表

- `images`: 图片元数据
- `scans`: 扫描任务
- `duplicates`: 完全重复文件
- `similar_pairs`: 相似图片配对
- `recycle_bin`: 回收站
- `settings`: 用户设置

## 性能特性

- ✅ 支持 50 万张以上图片
- ✅ 多线程并行处理
- ✅ 断点续扫
- ✅ UI 非阻塞
- ✅ 增量扫描 (仅处理新增/变更文件)

## 安全保障

1. **双重确认**: 文件先进回收站,用户确认后才永久删除
2. **严格匹配**: 只有同时满足多个条件才判定为可删除
3. **相同尺寸禁删**: 分辨率完全一致的图片禁止自动删除
4. **完整日志**: 详细的删除报告,可追溯每个决策

## 开发状态

当前进度: ✅ 基础项目结构创建完成

- [x] 前端框架搭建
- [x] Rust 后端模块设计
- [x] 数据库设计
- [x] 扫描引擎
- [x] 哈希计算
- [x] SSIM 引擎
- [x] 删除管理
- [ ] Tauri 命令完善
- [ ] 前端组件功能实现
- [ ] 测试与优化

## License

MIT
