use crate::error::{AppError, Result};
use image::DynamicImage;

/// SSIM 计算器
///
/// 注意：这是一个简化的 SSIM 实现
/// 完整的生产级实现建议使用 OpenCV 或专门的 SSIM 库
pub struct SsimComputer;

impl SsimComputer {
    const MAX_SSIM_EDGE: u32 = 512;

    /// 计算两张图片的 SSIM 相似度
    ///
    /// 返回值范围: 0.0 ~ 1.0, 1.0 表示完全相同
    pub fn compute(img1: &DynamicImage, img2: &DynamicImage) -> Result<f64> {
        // 确保两张图片尺寸相同
        if img1.width() != img2.width() || img1.height() != img2.height() {
            return Err(AppError::SsimComputation("图片尺寸不匹配".to_string()));
        }

        // 转换为灰度图
        let gray1 = img1.to_luma8();
        let gray2 = img2.to_luma8();

        // 简化的 SSIM 计算
        // 这里使用均方误差的简化版本
        // 生产环境建议使用完整的 SSIM 算法或 OpenCV
        let pixels1 = gray1.as_raw();
        let pixels2 = gray2.as_raw();

        let mut sum_diff_sq = 0.0;

        for (p1, p2) in pixels1.iter().zip(pixels2.iter()) {
            let diff = (*p1 as f64) - (*p2 as f64);
            sum_diff_sq += diff * diff;
        }

        let n = pixels1.len() as f64;
        let mse = sum_diff_sq / n;

        // 转换 MSE 为相似度 (0-1)
        // MSE 越小，相似度越高
        let max_value = 255.0 * 255.0;
        let similarity = 1.0 - (mse / max_value).min(1.0);

        Ok(similarity)
    }

    /// 计算两个文件的 SSIM
    ///
    /// large_path: 大图路径
    /// small_path: 小图路径
    ///
    /// 会自动将大图缩放到小图尺寸后再计算
    pub fn compute_from_files(
        large_path: &std::path::Path,
        small_path: &std::path::Path,
    ) -> Result<f64> {
        use super::resize::ImageResizer;

        // 加载小图
        let small_img = image::open(small_path)?;
        let (target_width, target_height) =
            Self::target_dimensions(small_img.width(), small_img.height(), Self::MAX_SSIM_EDGE);

        let small_img_resized =
            if small_img.width() == target_width && small_img.height() == target_height {
                small_img
            } else {
                ImageResizer::resize_to_target(&small_img, target_width, target_height)?
            };

        // 加载大图并缩放
        let large_img_resized =
            ImageResizer::load_and_resize(large_path, target_width, target_height)?;

        // 计算 SSIM
        Self::compute(&large_img_resized, &small_img_resized)
    }

    /// 根据较小图片尺寸计算 SSIM 归一化目标尺寸，并限制最大边长。
    pub fn target_dimensions(width: u32, height: u32, max_edge: u32) -> (u32, u32) {
        if width == 0 || height == 0 || max_edge == 0 {
            return (width.max(1), height.max(1));
        }

        let longest_edge = width.max(height);
        if longest_edge <= max_edge {
            return (width, height);
        }

        let scale = max_edge as f64 / longest_edge as f64;
        let target_width = ((width as f64 * scale).round() as u32).max(1);
        let target_height = ((height as f64 * scale).round() as u32).max(1);

        (target_width, target_height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_identical_images() {
        // 两张完全相同的图片，SSIM 应该接近 1.0
        let img1 = DynamicImage::new_rgb8(100, 100);
        let img2 = DynamicImage::new_rgb8(100, 100);

        let ssim = SsimComputer::compute(&img1, &img2).unwrap();
        assert!(ssim > 0.99);
    }

    #[test]
    fn test_target_dimensions_limit_large_images() {
        let (width, height) = SsimComputer::target_dimensions(1280, 1051, 512);

        assert_eq!(width, 512);
        assert_eq!(height, 420);
    }
}
