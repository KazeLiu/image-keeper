use std::path::{Path};
use std::fs;
use chrono::Utc;
use crate::error::{Result, AppError};
use crate::db::models::Image;

/// 支持的图片格式
const SUPPORTED_FORMATS: &[&str] = &["jpg", "jpeg", "png", "webp", "avif", "bmp", "gif"];

/// 检查文件是否为支持的图片格式
pub fn is_supported_image(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        let ext_lower = ext.to_string_lossy().to_lowercase();
        return SUPPORTED_FORMATS.contains(&ext_lower.as_str());
    }
    false
}

/// 图片元数据提取器
pub struct MetadataExtractor;

impl MetadataExtractor {
    /// 提取图片元数据
    pub fn extract(
        file_path: &Path,
        root_path: &Path,
        scan_id: i64,
    ) -> Result<Image> {
        // 检查文件是否存在
        if !file_path.exists() {
            return Err(AppError::InvalidPath);
        }

        // 获取文件元数据
        let metadata = fs::metadata(file_path)?;
        let file_size = metadata.len() as i64;
        let file_modified_at = metadata
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| AppError::Other("无法获取文件修改时间".to_string()))?
            .as_secs() as i64;

        // 获取相对路径
        let relative_path = file_path
            .strip_prefix(root_path)
            .map_err(|_| AppError::InvalidPath)?
            .to_string_lossy()
            .to_string();

        // 读取图片尺寸和格式
        let img_reader = image::io::Reader::open(file_path)?;
        let img_format = img_reader
            .format()
            .ok_or_else(|| AppError::UnsupportedFormat("无法识别图片格式".to_string()))?;

        let img = img_reader.decode()?;
        let width = img.width();
        let height = img.height();

        // 计算宽高比
        let aspect_ratio = width as f64 / height as f64;

        // 格式转换为字符串
        let format = match img_format {
            image::ImageFormat::Jpeg => "jpg",
            image::ImageFormat::Png => "png",
            image::ImageFormat::WebP => "webp",
            image::ImageFormat::Bmp => "bmp",
            image::ImageFormat::Gif => "gif",
            _ => {
                return Err(AppError::UnsupportedFormat(
                    format!("{:?}", img_format)
                ))
            }
        };

        let now = Utc::now().timestamp();

        Ok(Image {
            id: 0, // 插入数据库后会被替换
            file_path: file_path.to_string_lossy().to_string(),
            relative_path,
            file_size,
            file_modified_at,
            width,
            height,
            format: format.to_string(),
            aspect_ratio,
            blake3_hash: None,
            phash: None,
            hash_computed_at: None,
            scan_id,
            folder_id: None,
            scanned_at: now,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_supported_image() {
        assert!(is_supported_image(Path::new("test.jpg")));
        assert!(is_supported_image(Path::new("test.PNG")));
        assert!(is_supported_image(Path::new("test.webp")));
        assert!(!is_supported_image(Path::new("test.txt")));
        assert!(!is_supported_image(Path::new("test")));
    }
}
