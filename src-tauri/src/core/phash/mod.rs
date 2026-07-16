pub mod engine;

use crate::error::AppError;
use crate::error::Result;
use std::path::Path;

pub const PHASH_ALGORITHM_VERSION: &str = "phash-v2";

/// 感知哈希计算器
pub struct PHashComputer;

impl PHashComputer {
    /// 使用正式扫描流程的 DCT-based 算法计算文件感知哈希。
    pub fn compute_phash(path: &Path) -> Result<String> {
        Self::compute_from_image(&image::open(path)?)
    }

    /// 使用正式扫描流程的 DCT-based 算法计算已解码图片感知哈希。
    pub fn compute_from_image(image: &image::DynamicImage) -> Result<String> {
        let resized = image.resize_exact(32, 32, image::imageops::FilterType::Lanczos3);
        let gray = resized.to_luma8();
        let dct = compute_dct_coefficients(gray.as_raw(), 16);
        let dct_selected = select_zigzag_coefficients(&dct, 16, 32);
        let dct_hash = fold_coefficients_to_bits(&dct_selected);
        let difference_hash = compute_difference_hash(image);
        let combined = (dct_hash << 32) | difference_hash;

        Ok(format!("{combined:016x}"))
    }

    /// 计算两个感知哈希之间的汉明距离
    pub fn hamming_distance(hash1: &str, hash2: &str) -> Result<u32> {
        if hash1.len() != 16 || hash2.len() != 16 {
            return Err(AppError::HashComputation(
                "感知哈希必须是 16 位十六进制字符串".to_string(),
            ));
        }
        let left = u64::from_str_radix(hash1, 16)
            .map_err(|error| AppError::HashComputation(format!("解析左侧感知哈希失败: {error}")))?;
        let right = u64::from_str_radix(hash2, 16)
            .map_err(|error| AppError::HashComputation(format!("解析右侧感知哈希失败: {error}")))?;
        Ok((left ^ right).count_ones())
    }
}

fn fold_coefficients_to_bits(coefficients: &[f64]) -> u64 {
    let mut sorted = coefficients.to_vec();
    sorted.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let median = sorted[sorted.len() / 2];
    let mut hash = 0u64;
    for (index, value) in coefficients.iter().enumerate() {
        if *value > median {
            hash |= 1u64 << index;
        }
    }
    hash
}

fn compute_difference_hash(image: &image::DynamicImage) -> u64 {
    let resized = image.resize_exact(17, 16, image::imageops::FilterType::Lanczos3);
    let gray = resized.to_luma8();
    let mut hash = 0u64;
    for y in 0..16 {
        for x in 0..16 {
            let left = gray.get_pixel(x, y).0[0];
            let right = gray.get_pixel(x + 1, y).0[0];
            if left > right {
                hash ^= 1u64 << ((y * 16 + x) % 32);
            }
        }
    }
    hash
}

fn compute_dct_coefficients(pixels: &[u8], coefficient_grid: usize) -> Vec<f64> {
    let mut dct = vec![0.0; coefficient_grid * coefficient_grid];
    for v in 0..coefficient_grid {
        for u in 0..coefficient_grid {
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
            dct[v * coefficient_grid + u] = 0.25 * cu * cv * sum;
        }
    }
    dct
}

fn select_zigzag_coefficients(dct: &[f64], coefficient_grid: usize, count: usize) -> Vec<f64> {
    let mut selected = Vec::with_capacity(count);
    for diagonal in 0..=(coefficient_grid * 2 - 2) {
        for v in 0..=diagonal {
            let u = diagonal - v;
            if u >= coefficient_grid || v >= coefficient_grid || (u == 0 && v == 0) {
                continue;
            }
            selected.push(dct[v * coefficient_grid + u]);
            if selected.len() == count {
                return selected;
            }
        }
    }
    selected
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

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
    fn gradient_hash_stays_stable_for_the_current_algorithm() {
        let image =
            image::GrayImage::from_fn(32, 32, |x, y| image::Luma([((x * 7 + y * 11) % 256) as u8]));

        let hash =
            PHashComputer::compute_from_image(&image::DynamicImage::ImageLuma8(image)).unwrap();

        assert_eq!(hash, "7987c03a8fff3bff");
    }

    #[test]
    fn similar_compositions_with_different_details_do_not_collapse_to_zero_distance() {
        let make_fixture = |mouth_y: u32| {
            let mut image = RgbImage::from_pixel(360, 512, Rgb([250, 250, 250]));
            for y in 30..470 {
                for x in 170..300 {
                    image.put_pixel(x, y, Rgb([220, 224, 228]));
                }
            }
            for y in 185..430 {
                for x in 70..265 {
                    image.put_pixel(x, y, Rgb([252, 226, 224]));
                }
            }
            for y in 360..455 {
                for x in 60..310 {
                    image.put_pixel(x, y, Rgb([60, 76, 100]));
                }
            }
            for y in 150..154 {
                for x in 145..190 {
                    image.put_pixel(x, y, Rgb([40, 40, 48]));
                }
                for x in 220..265 {
                    image.put_pixel(x, y, Rgb([40, 40, 48]));
                }
            }
            for y in mouth_y..mouth_y + 4 {
                for x in 182..228 {
                    image.put_pixel(x, y, Rgb([90, 34, 40]));
                }
            }
            image::DynamicImage::ImageRgb8(image)
        };
        let left = PHashComputer::compute_from_image(&make_fixture(205)).unwrap();
        let right = PHashComputer::compute_from_image(&make_fixture(230)).unwrap();

        assert_ne!(left, right);
        assert!(PHashComputer::hamming_distance(&left, &right).unwrap() > 0);
    }
}
