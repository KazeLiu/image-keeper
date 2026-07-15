use crate::error::{AppError, Result};
use image::{DynamicImage, GrayImage};
use std::collections::{HashMap, HashSet};

const WINDOW_RADIUS: i32 = 5;
const SIGMA: f64 = 1.5;
const C1: f64 = 6.5025;
const C2: f64 = 58.5225;

pub struct StandardSsim;

impl StandardSsim {
    /// 使用 11×11、sigma=1.5 的高斯窗口计算灰度 SSIM。
    ///
    /// 边界像素采用镜像延拓；返回值不裁剪，因此标准公式产生的负值会被保留。
    pub fn compute(left: &DynamicImage, right: &DynamicImage) -> Result<f64> {
        Self::validate_dimensions(left, right)?;
        Self::compute_gray(left.to_luma8(), right.to_luma8())
    }

    /// 消费已解码图片计算 SSIM，允许调用方在转灰度时立即释放彩色原图。
    pub fn compute_owned(left: DynamicImage, right: DynamicImage) -> Result<f64> {
        Self::validate_dimensions(&left, &right)?;
        Self::compute_gray(left.into_luma8(), right.into_luma8())
    }

    fn validate_dimensions(left: &DynamicImage, right: &DynamicImage) -> Result<()> {
        if left.width() != right.width() || left.height() != right.height() {
            return Err(AppError::SsimComputation("图片尺寸不匹配".to_string()));
        }
        if left.width() == 0 || left.height() == 0 {
            return Err(AppError::SsimComputation("图片像素为空".to_string()));
        }
        Ok(())
    }

    fn compute_gray(left: GrayImage, right: GrayImage) -> Result<f64> {
        let kernel = gaussian_kernel();
        let mut horizontal_rows: HashMap<u32, Vec<[f64; 5]>> = HashMap::new();
        let mut score_sum = 0.0;

        for y in 0..left.height() {
            let required_rows = required_source_rows(y, left.height());
            for source_y in &required_rows {
                horizontal_rows
                    .entry(*source_y)
                    .or_insert_with(|| horizontal_stats_row(&left, &right, *source_y, &kernel));
            }

            for x in 0..left.width() as usize {
                let mut stats = [0.0; 5];
                for offset in -WINDOW_RADIUS..=WINDOW_RADIUS {
                    let source_y = reflect(y as i32 + offset, left.height()) as u32;
                    let weight = kernel[(offset + WINDOW_RADIUS) as usize];
                    let horizontal = horizontal_rows
                        .get(&source_y)
                        .expect("所需的 SSIM 横向统计行必须已缓存")[x];
                    for index in 0..stats.len() {
                        stats[index] += weight * horizontal[index];
                    }
                }

                let [mu_left, mu_right, second_left, second_right, cross] = stats;
                let variance_left = (second_left - mu_left * mu_left).max(0.0);
                let variance_right = (second_right - mu_right * mu_right).max(0.0);
                let covariance = cross - mu_left * mu_right;
                let numerator = (2.0 * mu_left * mu_right + C1) * (2.0 * covariance + C2);
                let denominator = (mu_left * mu_left + mu_right * mu_right + C1)
                    * (variance_left + variance_right + C2);
                score_sum += numerator / denominator;
            }

            let next_rows = if y + 1 < left.height() {
                required_source_rows(y + 1, left.height())
            } else {
                HashSet::new()
            };
            horizontal_rows.retain(|source_y, _| next_rows.contains(source_y));
        }

        Ok(score_sum / (left.width() as f64 * left.height() as f64))
    }
}

fn horizontal_stats_row(
    left: &GrayImage,
    right: &GrayImage,
    y: u32,
    kernel: &[f64; 11],
) -> Vec<[f64; 5]> {
    (0..left.width())
        .map(|x| {
            let mut stats = [0.0; 5];
            for offset in -WINDOW_RADIUS..=WINDOW_RADIUS {
                let source_x = reflect(x as i32 + offset, left.width()) as u32;
                let weight = kernel[(offset + WINDOW_RADIUS) as usize];
                let left_value = left.get_pixel(source_x, y)[0] as f64;
                let right_value = right.get_pixel(source_x, y)[0] as f64;
                stats[0] += weight * left_value;
                stats[1] += weight * right_value;
                stats[2] += weight * left_value * left_value;
                stats[3] += weight * right_value * right_value;
                stats[4] += weight * left_value * right_value;
            }
            stats
        })
        .collect()
}

fn required_source_rows(y: u32, height: u32) -> HashSet<u32> {
    (-WINDOW_RADIUS..=WINDOW_RADIUS)
        .map(|offset| reflect(y as i32 + offset, height) as u32)
        .collect()
}

fn gaussian_kernel() -> [f64; 11] {
    let mut kernel = [0.0; 11];
    let mut sum = 0.0;
    for offset in -WINDOW_RADIUS..=WINDOW_RADIUS {
        let value = (-((offset * offset) as f64) / (2.0 * SIGMA * SIGMA)).exp();
        kernel[(offset + WINDOW_RADIUS) as usize] = value;
        sum += value;
    }
    for value in &mut kernel {
        *value /= sum;
    }
    kernel
}

fn reflect(mut index: i32, length: u32) -> i32 {
    let length = length as i32;
    if length == 1 {
        return 0;
    }
    while index < 0 || index >= length {
        index = if index < 0 {
            -index - 1
        } else {
            2 * length - index - 1
        };
    }
    index
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GrayImage, Luma};

    fn solid(value: u8) -> DynamicImage {
        DynamicImage::ImageLuma8(GrayImage::from_pixel(16, 16, Luma([value])))
    }

    #[test]
    fn identical_images_score_one() {
        let image = solid(127);
        let score = StandardSsim::compute(&image, &image).unwrap();

        assert!((score - 1.0).abs() < 1e-12);
    }

    #[test]
    fn owned_images_score_the_same_without_requiring_caller_clones() {
        let score = StandardSsim::compute_owned(solid(127), solid(127)).unwrap();

        assert!((score - 1.0).abs() < 1e-12);
    }

    #[test]
    fn black_and_white_match_analytical_reference() {
        let score = StandardSsim::compute(&solid(0), &solid(255)).unwrap();
        let c1 = (0.01_f64 * 255.0).powi(2);
        let expected = c1 / (255.0_f64.powi(2) + c1);

        assert!((score - expected).abs() < 1e-10, "{score} != {expected}");
    }

    #[test]
    fn local_structure_change_reduces_score() {
        let left = GrayImage::from_fn(32, 32, |x, y| Luma([((x + y) % 256) as u8]));
        let right = GrayImage::from_fn(32, 32, |x, y| Luma([((x * 3 + y) % 256) as u8]));
        let score = StandardSsim::compute(
            &DynamicImage::ImageLuma8(left),
            &DynamicImage::ImageLuma8(right),
        )
        .unwrap();

        assert!(score < 0.95);
    }

    #[test]
    fn patterned_images_match_independent_reference_vector() {
        let left = GrayImage::from_fn(17, 13, |x, y| {
            Luma([((x * 17 + y * 29 + (x * y) % 31) % 256) as u8])
        });
        let right = GrayImage::from_fn(17, 13, |x, y| {
            Luma([((x * 11 + y * 37 + ((x + 3) * (y + 5)) % 43) % 256) as u8])
        });

        let score = StandardSsim::compute(
            &DynamicImage::ImageLuma8(left),
            &DynamicImage::ImageLuma8(right),
        )
        .unwrap();

        // 由独立的二维 11×11 高斯卷积参考实现生成。
        assert!((score - 0.383_446_772_514_431).abs() < 1e-12, "{score}");
    }
}
