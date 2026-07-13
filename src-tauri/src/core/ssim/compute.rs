use image::DynamicImage;
use crate::error::{Result, AppError};

/// SSIM 计算器
///
/// 注意：这是一个简化的 SSIM 实现
/// 完整的生产级实现建议使用 OpenCV 或专门的 SSIM 库
pub struct SsimComputer;

impl SsimComputer {
    /// 计算两张图片的 SSIM 相似度
    ///
    /// 返回值范围: 0.0 ~ 1.0, 1.0 表示完全相同
    pub fn compute(img1: &DynamicImage, img2: &DynamicImage) -> Result<f64> {
        // 确保两张图片尺寸相同
        if img1.width() != img2.width() || img1.height() != img2.height() {
            return Err(AppError::SsimComputation(
                "图片尺寸不匹配".to_string()
            ));
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
        let target_width = small_img.width();
        let target_height = small_img.height();

        // 加载大图并缩放
        let large_img_resized = ImageResizer::load_and_resize(
            large_path,
            target_width,
            target_height,
        )?;

        // 计算 SSIM
        Self::compute(&large_img_resized, &small_img)
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
}
