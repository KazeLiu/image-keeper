use crate::db::models::FolderRole;
use crate::db::repository::Repository;
use crate::error::{AppError, Result};
use blake3::Hasher;
use image::GenericImageView;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use walkdir::WalkDir;

/// 进度回调函数类型
pub type ProgressCallback = Box<dyn Fn(usize, usize, &str) + Send>;

/// 扫描引擎 - Phase 1: 扫描与特征提取
pub struct ScanEngine {
    exclude_patterns: Vec<String>,
    supported_formats: Vec<String>,
}

impl ScanEngine {
    pub fn new(exclude_patterns: Vec<String>, supported_formats: Vec<String>) -> Self {
        Self {
            exclude_patterns,
            supported_formats,
        }
    }

    /// 扫描目录并提取特征(带进度回调)
    pub fn scan_directory<F>(
        &self,
        repository: &Repository,
        run_id: &str,
        folder_id: i64,
        root_path: &Path,
        role: FolderRole,
        progress_callback: F,
    ) -> Result<Vec<i64>>
    where
        F: Fn(usize, usize) + Send,
    {
        // 方案 A: 先统计文件数量
        let total_files = self.count_files(root_path)?;
        let mut processed_files = 0;
        let mut image_ids = Vec::new();

        for entry in WalkDir::new(root_path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| self.should_include(e.path()))
        {
            let entry = entry.map_err(|e| AppError::FileSystem(format!("遍历目录失败: {}", e)))?;

            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();
            if !self.is_supported_format(path) {
                continue;
            }

            // 扫描单个文件
            match self.scan_file(repository, run_id, folder_id, root_path, path, role.clone()) {
                Ok(image_id) => {
                    image_ids.push(image_id);
                    processed_files += 1;
                    progress_callback(processed_files, total_files);
                }
                Err(e) => {
                    eprintln!("扫描文件失败 {:?}: {}", path, e);
                    // 单文件失败不影响整体进度
                    processed_files += 1;
                    progress_callback(processed_files, total_files);
                }
            }
        }

        Ok(image_ids)
    }

    /// 统计目录下的图片文件数量
    fn count_files(&self, root_path: &Path) -> Result<usize> {
        let mut count = 0;

        for entry in WalkDir::new(root_path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| self.should_include(e.path()))
        {
            let entry = entry.map_err(|e| AppError::FileSystem(format!("遍历目录失败: {}", e)))?;

            if entry.file_type().is_file() && self.is_supported_format(entry.path()) {
                count += 1;
            }
        }

        Ok(count)
    }

    /// 扫描单个文件
    fn scan_file(
        &self,
        repository: &Repository,
        run_id: &str,
        folder_id: i64,
        root_path: &Path,
        file_path: &Path,
        role: FolderRole,
    ) -> Result<i64> {
        // 获取文件元数据
        let metadata = std::fs::metadata(file_path)
            .map_err(|e| AppError::FileSystem(format!("读取文件元数据失败: {}", e)))?;

        let file_size = metadata.len() as i64;
        let file_modified_at = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let relative_path = file_path
            .strip_prefix(root_path)
            .map_err(|_| AppError::InvalidPath)?
            .to_string_lossy()
            .to_string();

        // 解码图片
        let img = image::open(file_path).map_err(|e| AppError::Image(e))?;

        let (width, height) = img.dimensions();
        let format = format!("{:?}", img.color());
        let aspect_ratio = width as f64 / height as f64;

        // 计算 BLAKE3 哈希
        let blake3_hash = self.compute_blake3(file_path)?;

        // 计算 pHash (暂时使用占位符)
        let phash = self.compute_phash(&img)?;

        // 创建图片记录
        let image_id = {
            use crate::db::models::ScanStatus;

            // 先插入基本信息
            let _image_id = repository
                .conn()
                .execute(
                    r#"INSERT INTO images (
                    run_id, folder_id, source_role, file_path, relative_path,
                    file_size, file_modified_at, width, height, format,
                    aspect_ratio, frame_count, frame_strategy, scan_status, scanned_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"#,
                    rusqlite::params![
                        run_id,
                        folder_id,
                        role.as_str(),
                        file_path.to_string_lossy().as_ref(),
                        relative_path,
                        file_size,
                        file_modified_at,
                        width,
                        height,
                        format,
                        aspect_ratio,
                        1,       // frame_count
                        "first", // frame_strategy
                        ScanStatus::Decoded.as_str(),
                        chrono::Utc::now().timestamp(),
                    ],
                )
                .map_err(|e| AppError::Database(e))?;

            let image_id = repository.conn().last_insert_rowid();

            // 更新哈希
            repository.update_image_hash(image_id, &blake3_hash, &phash, "phash-v1")?;

            image_id
        };

        Ok(image_id)
    }

    /// 计算 BLAKE3 哈希
    fn compute_blake3(&self, path: &Path) -> Result<String> {
        let file =
            File::open(path).map_err(|e| AppError::FileSystem(format!("打开文件失败: {}", e)))?;

        let mut reader = BufReader::new(file);
        let mut hasher = Hasher::new();
        let mut buffer = [0; 8192];

        loop {
            let count = reader
                .read(&mut buffer)
                .map_err(|e| AppError::FileSystem(format!("读取文件失败: {}", e)))?;

            if count == 0 {
                break;
            }

            hasher.update(&buffer[..count]);
        }

        Ok(hasher.finalize().to_hex().to_string())
    }

    /// 计算 pHash (DCT-based 实现)
    fn compute_phash(&self, img: &image::DynamicImage) -> Result<String> {
        // 1. 缩放到 32x32
        let resized = img.resize_exact(32, 32, image::imageops::FilterType::Lanczos3);

        // 2. 转换为灰度
        let gray = resized.to_luma8();
        let pixels = gray.as_raw();

        // 3. 执行 DCT 变换
        let dct_matrix = self.compute_dct(pixels, 32, 32);

        // 4. 提取左上角 8x8 低频分量（跳过 DC 分量 [0,0]）
        let mut low_freq = Vec::new();
        for y in 0..8 {
            for x in 0..8 {
                if x == 0 && y == 0 {
                    continue; // 跳过 DC 分量
                }
                low_freq.push(dct_matrix[y * 32 + x]);
            }
        }

        // 5. 计算中值
        let mut sorted = low_freq.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = sorted[sorted.len() / 2];

        // 6. 生成 64 位哈希（8x8 - 1 = 63 位，补齐到 64）
        let mut hash = 0u64;
        for (i, &value) in low_freq.iter().enumerate().take(63) {
            if value > median {
                hash |= 1u64 << i;
            }
        }

        Ok(format!("{:016x}", hash))
    }

    /// 计算 DCT (简化版 2D DCT)
    fn compute_dct(&self, pixels: &[u8], width: usize, height: usize) -> Vec<f64> {
        let mut dct = vec![0.0; width * height];

        for v in 0..height {
            for u in 0..width {
                let mut sum = 0.0;

                for y in 0..height {
                    for x in 0..width {
                        let pixel_value = pixels[y * width + x] as f64;
                        let cos_x = ((2.0 * x as f64 + 1.0) * u as f64 * std::f64::consts::PI
                            / (2.0 * width as f64))
                            .cos();
                        let cos_y = ((2.0 * y as f64 + 1.0) * v as f64 * std::f64::consts::PI
                            / (2.0 * height as f64))
                            .cos();
                        sum += pixel_value * cos_x * cos_y;
                    }
                }

                // 归一化系数
                let cu = if u == 0 { 1.0 / (2.0_f64).sqrt() } else { 1.0 };
                let cv = if v == 0 { 1.0 / (2.0_f64).sqrt() } else { 1.0 };

                dct[v * width + u] = 0.25 * cu * cv * sum;
            }
        }

        dct
    }

    /// 检查路径是否应该包含
    fn should_include(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        for pattern in &self.exclude_patterns {
            if path_str.contains(pattern) {
                return false;
            }
        }

        true
    }

    /// 检查是否支持的格式
    fn is_supported_format(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            let ext_lower = ext.to_string_lossy().to_lowercase();
            self.supported_formats.contains(&ext_lower)
        } else {
            false
        }
    }
}
