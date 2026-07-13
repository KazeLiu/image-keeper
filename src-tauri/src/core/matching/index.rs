use crate::db::models::Image;

/// 尺寸索引 - 用于快速查找比指定图片更大的候选项
pub struct SizeIndex;

impl SizeIndex {
    /// 检查两张图片是否具有相同的宽高比
    pub fn has_same_aspect_ratio(img1: &Image, img2: &Image, tolerance: f64) -> bool {
        (img1.aspect_ratio - img2.aspect_ratio).abs() < tolerance
    }

    /// 检查 small 是否严格小于 large
    pub fn is_strictly_smaller(small: &Image, large: &Image) -> bool {
        small.width < large.width
            && small.height < large.height
            && small.file_size < large.file_size
    }

    /// 计算宽高比容差范围
    pub fn calculate_aspect_ratio_range(aspect_ratio: f64, tolerance: f64) -> (f64, f64) {
        (aspect_ratio - tolerance, aspect_ratio + tolerance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_image(width: u32, height: u32, file_size: i64) -> Image {
        Image {
            id: 0,
            file_path: String::new(),
            relative_path: String::new(),
            file_size,
            file_modified_at: 0,
            width,
            height,
            format: "jpg".to_string(),
            aspect_ratio: width as f64 / height as f64,
            blake3_hash: None,
            hash_computed_at: None,
            scan_id: 0,
            scanned_at: 0,
        }
    }

    #[test]
    fn test_has_same_aspect_ratio() {
        let img1 = create_test_image(1920, 1080, 1000000);
        let img2 = create_test_image(3840, 2160, 2000000);

        // 相同宽高比 (16:9)
        assert!(SizeIndex::has_same_aspect_ratio(&img1, &img2, 0.01));
    }

    #[test]
    fn test_is_strictly_smaller() {
        let small = create_test_image(1920, 1080, 1000000);
        let large = create_test_image(3840, 2160, 2000000);

        assert!(SizeIndex::is_strictly_smaller(&small, &large));
        assert!(!SizeIndex::is_strictly_smaller(&large, &small));
    }
}
