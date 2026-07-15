pub mod engine;

use crate::error::AppError;
use crate::error::Result;
use std::path::Path;

/// pHash 计算器
pub struct PHashComputer;

impl PHashComputer {
    /// 使用正式扫描流程的 DCT-based 算法计算文件 pHash。
    pub fn compute_phash(path: &Path) -> Result<String> {
        Self::compute_from_image(&image::open(path)?)
    }

    /// 使用正式扫描流程的 DCT-based 算法计算已解码图片 pHash。
    pub fn compute_from_image(image: &image::DynamicImage) -> Result<String> {
        let resized = image.resize_exact(32, 32, image::imageops::FilterType::Lanczos3);
        let gray = resized.to_luma8();
        let dct = compute_low_frequency_dct(gray.as_raw());

        let mut low_frequency = Vec::with_capacity(63);
        for v in 0..8 {
            for u in 0..8 {
                if u != 0 || v != 0 {
                    low_frequency.push(dct[v * 8 + u]);
                }
            }
        }
        let mut sorted = low_frequency.clone();
        sorted.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
        let median = sorted[sorted.len() / 2];
        let mut hash = 0u64;
        for (index, value) in low_frequency.iter().enumerate().take(63) {
            if *value > median {
                hash |= 1u64 << index;
            }
        }

        Ok(format!("{hash:016x}"))
    }

    /// 计算两个 pHash 之间的汉明距离
    pub fn hamming_distance(hash1: &str, hash2: &str) -> Result<u32> {
        if hash1.len() != 16 || hash2.len() != 16 {
            return Err(AppError::HashComputation(
                "pHash 必须是 16 位十六进制字符串".to_string(),
            ));
        }
        let left = u64::from_str_radix(hash1, 16)
            .map_err(|error| AppError::HashComputation(format!("解析左侧 pHash 失败: {error}")))?;
        let right = u64::from_str_radix(hash2, 16)
            .map_err(|error| AppError::HashComputation(format!("解析右侧 pHash 失败: {error}")))?;
        Ok((left ^ right).count_ones())
    }
}

fn compute_low_frequency_dct(pixels: &[u8]) -> Vec<f64> {
    let mut dct = vec![0.0; 8 * 8];
    for v in 0..8 {
        for u in 0..8 {
            let mut sum = 0.0;
            for y in 0..32 {
                for x in 0..32 {
                    let pixel = pixels[y * 32 + x] as f64;
                    let cos_x =
                        ((2.0 * x as f64 + 1.0) * u as f64 * std::f64::consts::PI / 64.0).cos();
                    let cos_y =
                        ((2.0 * y as f64 + 1.0) * v as f64 * std::f64::consts::PI / 64.0).cos();
                    sum += pixel * cos_x * cos_y;
                }
            }
            let cu = if u == 0 { 1.0 / (2.0_f64).sqrt() } else { 1.0 };
            let cv = if v == 0 { 1.0 / (2.0_f64).sqrt() } else { 1.0 };
            dct[v * 8 + u] = 0.25 * cu * cv * sum;
        }
    }
    dct
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phash_matches_the_formal_scanner_hex_format() {
        let image = image::DynamicImage::new_rgb8(64, 64);
        let hash = PHashComputer::compute_from_image(&image).unwrap();

        assert_eq!(hash.len(), 16);
        assert!(u64::from_str_radix(&hash, 16).is_ok());
    }

    #[test]
    fn hamming_distance_uses_all_64_formal_phash_bits() {
        assert_eq!(
            PHashComputer::hamming_distance("0000000000000000", "ffffffffffffffff").unwrap(),
            64
        );
    }

    #[test]
    fn gradient_hash_stays_compatible_with_the_formal_scanner() {
        let image =
            image::GrayImage::from_fn(32, 32, |x, y| image::Luma([((x * 7 + y * 11) % 256) as u8]));

        let hash =
            PHashComputer::compute_from_image(&image::DynamicImage::ImageLuma8(image)).unwrap();

        assert_eq!(hash, "472d78037e54c9d4");
    }
}
