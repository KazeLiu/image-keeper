use crate::error::{AppError, Result};
use image::{DynamicImage, GrayImage};
use std::path::Path;

use super::{resize::ImageResizer, standard::StandardSsim};

/// 全程序唯一的标准结构相似性计算入口。
pub struct SsimComputer;

impl SsimComputer {
    /// 使用 11×11、sigma=1.5 的高斯窗口计算标准灰度 SSIM。
    ///
    /// 1.0 表示完全相同；标准公式对反相关结构可能返回负值。
    pub fn compute(img1: &DynamicImage, img2: &DynamicImage) -> Result<f64> {
        StandardSsim::compute(img1, img2)
    }

    /// 对已归一化的灰度图计算同一套标准 SSIM，供缓存路径复用。
    pub fn compute_gray(left: &GrayImage, right: &GrayImage) -> Result<f64> {
        StandardSsim::compute_gray(left, right)
    }

    /// 计算两个文件的标准 SSIM。参数顺序不影响归一化结果。
    ///
    /// 两张图片统一到像素数较少图片的完整宽高，不再使用 512px 降采样。
    pub fn compute_from_files(left_path: &Path, right_path: &Path) -> Result<f64> {
        let left = image::open(left_path)?;
        let right = image::open(right_path)?;
        let target = Self::pair_target_dimensions(
            (left.width(), left.height()),
            (right.width(), right.height()),
        );
        let left = Self::prepare_image(&left, target.0, target.1)?;
        let right = Self::prepare_image(&right, target.0, target.1)?;
        Self::compute_gray(&left, &right)
    }

    /// 为同一图片对选择唯一目标尺寸：像素数较少者优先，平局时按宽、高排序。
    pub fn pair_target_dimensions(left: (u32, u32), right: (u32, u32)) -> (u32, u32) {
        let left_key = (left.0 as u64 * left.1 as u64, left.0, left.1);
        let right_key = (right.0 as u64 * right.1 as u64, right.0, right.1);
        if left_key <= right_key {
            left
        } else {
            right
        }
    }

    /// 使用统一的 Lanczos3 路径把图片准备为目标尺寸的灰度数据。
    pub fn prepare_image(
        image: &DynamicImage,
        target_width: u32,
        target_height: u32,
    ) -> Result<GrayImage> {
        if target_width == 0 || target_height == 0 {
            return Err(AppError::SsimComputation("目标图片像素为空".to_string()));
        }
        if image.width() == target_width && image.height() == target_height {
            Ok(image.to_luma8())
        } else {
            Ok(ImageResizer::resize_to_target(image, target_width, target_height)?.into_luma8())
        }
    }

    /// 根据较小图片尺寸计算结构相似性归一化目标尺寸，并限制最大边长。
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
    use crate::core::ssim::standard::StandardSsim;
    use image::{GrayImage, Luma};

    #[test]
    fn test_compute_identical_images() {
        // 两张完全相同的图片，结构相似性应该接近 1.0
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

    #[test]
    fn canonical_compute_uses_the_standard_windowed_formula() {
        let left = DynamicImage::ImageLuma8(GrayImage::from_fn(32, 24, |x, y| {
            Luma([((x * 7 + y * 11) % 256) as u8])
        }));
        let right = DynamicImage::ImageLuma8(GrayImage::from_fn(32, 24, |x, y| {
            Luma([((x * 13 + y * 3 + 17) % 256) as u8])
        }));

        let expected = StandardSsim::compute(&left, &right).unwrap();
        let actual = SsimComputer::compute(&left, &right).unwrap();

        assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
    }
}
