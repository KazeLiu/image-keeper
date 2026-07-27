use crate::error::{AppError, Result};
use fast_image_resize as fr;
use image::DynamicImage;
use std::num::NonZeroU32;

/// 图片缩放器
pub struct ImageResizer;

impl ImageResizer {
    /// 将大图缩放到小图尺寸
    pub fn resize_to_target(
        large_img: &DynamicImage,
        target_width: u32,
        target_height: u32,
    ) -> Result<DynamicImage> {
        // 转换为 RGBA8
        let src_image = large_img.to_rgba8();
        let width = src_image.width();
        let height = src_image.height();

        // 创建源图片
        let src = fr::Image::from_vec_u8(
            NonZeroU32::new(width).ok_or_else(|| AppError::Other("图片宽度为0".to_string()))?,
            NonZeroU32::new(height).ok_or_else(|| AppError::Other("图片高度为0".to_string()))?,
            src_image.into_raw(),
            fr::PixelType::U8x4,
        )
        .map_err(|e| {
            AppError::Image(image::ImageError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )))
        })?;

        // 创建目标图片
        let mut dst = fr::Image::new(
            NonZeroU32::new(target_width)
                .ok_or_else(|| AppError::Other("目标宽度为0".to_string()))?,
            NonZeroU32::new(target_height)
                .ok_or_else(|| AppError::Other("目标高度为0".to_string()))?,
            fr::PixelType::U8x4,
        );

        // 创建缩放器
        let mut resizer = fr::Resizer::new(fr::ResizeAlg::Convolution(fr::FilterType::Lanczos3));

        // 执行缩放
        resizer
            .resize(&src.view(), &mut dst.view_mut())
            .map_err(|e| {
                AppError::Image(image::ImageError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )))
            })?;

        // 转换回 DynamicImage
        let buffer = image::RgbaImage::from_raw(target_width, target_height, dst.into_vec())
            .ok_or_else(|| AppError::Other("图片缩放失败".to_string()))?;

        Ok(DynamicImage::ImageRgba8(buffer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resize_to_target() {
        // 创建一个测试图片
        let img = DynamicImage::new_rgb8(1920, 1080);

        // 缩放到 960x540
        let resized = ImageResizer::resize_to_target(&img, 960, 540).unwrap();

        assert_eq!(resized.width(), 960);
        assert_eq!(resized.height(), 540);
    }
}
