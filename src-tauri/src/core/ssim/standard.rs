use crate::error::{AppError, Result};
use image::{DynamicImage, GrayImage};

const WINDOW_RADIUS: i32 = 5;
const WINDOW_SPAN: usize = (WINDOW_RADIUS * 2 + 1) as usize;
const SIGMA: f64 = 1.5;
const C1: f64 = 6.5025;
const C2: f64 = 58.5225;

pub struct StandardSsim;

impl StandardSsim {
    /// 使用 11×11、sigma=1.5 的高斯窗口计算灰度结构相似性。
    ///
    /// 边界像素采用镜像延拓；返回值不裁剪，因此标准公式产生的负值会被保留。
    pub fn compute(left: &DynamicImage, right: &DynamicImage) -> Result<f64> {
        Self::validate_dimensions(left, right)?;
        let left = left.to_luma8();
        let right = right.to_luma8();
        Self::compute_gray(&left, &right)
    }

    /// 消费已解码图片计算结构相似性，允许调用方在转灰度时立即释放彩色原图。
    pub fn compute_owned(left: DynamicImage, right: DynamicImage) -> Result<f64> {
        Self::validate_dimensions(&left, &right)?;
        let left = left.into_luma8();
        let right = right.into_luma8();
        Self::compute_gray(&left, &right)
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

    pub(crate) fn compute_gray(left: &GrayImage, right: &GrayImage) -> Result<f64> {
        Self::compute_gray_cancellable(left, right, || false)
    }

    pub(crate) fn compute_gray_cancellable<F>(
        left: &GrayImage,
        right: &GrayImage,
        mut is_cancelled: F,
    ) -> Result<f64>
    where
        F: FnMut() -> bool,
    {
        if left.dimensions() != right.dimensions() {
            return Err(AppError::SsimComputation("图片尺寸不匹配".to_string()));
        }
        if left.width() == 0 || left.height() == 0 {
            return Err(AppError::SsimComputation("图片像素为空".to_string()));
        }
        let kernel = gaussian_kernel();
        let width = left.width() as usize;
        let height = left.height() as usize;
        // 预先展开镜像延拓索引，避免在双层像素循环里反复执行折叠计算。
        let vertical_offsets = reflect_offset_table(left.height());
        let horizontal_offsets = reflect_offset_table(left.width());
        let left_pixels = left.as_raw();
        let right_pixels = right.as_raw();
        // 按 source_y % WINDOW_SPAN 寻址的行缓存：镜像延拓后的源行落在跨度不超过
        // WINDOW_SPAN 的区间内，因此同一输出行内两个不同源行不会争用同一槽位，
        // 由 ring_buffer_slots_never_alias_two_different_source_rows 穷举验证。
        let mut ring = vec![vec![[0.0f64; 5]; width]; WINDOW_SPAN];
        let mut ring_owner = [usize::MAX; WINDOW_SPAN];
        let mut score_sum = 0.0;

        for y in 0..height {
            if is_cancelled() {
                return Err(AppError::SsimComputation("SSIM 计算已取消".to_string()));
            }
            let row_sources = &vertical_offsets[y * WINDOW_SPAN..(y + 1) * WINDOW_SPAN];
            for &source_y in row_sources {
                let slot = source_y % WINDOW_SPAN;
                if ring_owner[slot] == source_y {
                    continue;
                }
                if is_cancelled() {
                    return Err(AppError::SsimComputation("SSIM 计算已取消".to_string()));
                }
                fill_horizontal_stats_row(
                    left_pixels,
                    right_pixels,
                    width,
                    source_y,
                    &horizontal_offsets,
                    &kernel,
                    &mut ring[slot],
                );
                ring_owner[slot] = source_y;
            }

            for x in 0..width {
                let mut stats = [0.0; 5];
                for offset in 0..WINDOW_SPAN {
                    let source_y = row_sources[offset];
                    let weight = kernel[offset];
                    let horizontal = ring[source_y % WINDOW_SPAN][x];
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
        }

        Ok(score_sum / (width as f64 * height as f64))
    }
}

/// 就地填充一行横向高斯统计量，直接索引底层灰度缓冲避免逐像素边界检查。
fn fill_horizontal_stats_row(
    left_pixels: &[u8],
    right_pixels: &[u8],
    width: usize,
    y: usize,
    horizontal_offsets: &[usize],
    kernel: &[f64; WINDOW_SPAN],
    row: &mut [[f64; 5]],
) {
    let row_base = y * width;
    let left_row = &left_pixels[row_base..row_base + width];
    let right_row = &right_pixels[row_base..row_base + width];

    for x in 0..width {
        let sources = &horizontal_offsets[x * WINDOW_SPAN..(x + 1) * WINDOW_SPAN];
        let mut stats = [0.0; 5];
        for offset in 0..WINDOW_SPAN {
            let source_x = sources[offset];
            let weight = kernel[offset];
            let left_value = left_row[source_x] as f64;
            let right_value = right_row[source_x] as f64;
            stats[0] += weight * left_value;
            stats[1] += weight * right_value;
            stats[2] += weight * left_value * left_value;
            stats[3] += weight * right_value * right_value;
            stats[4] += weight * left_value * right_value;
        }
        row[x] = stats;
    }
}

/// 为每个坐标预展开 11 个镜像延拓后的源索引，布局为 index * WINDOW_SPAN + offset。
fn reflect_offset_table(length: u32) -> Vec<usize> {
    let mut table = Vec::with_capacity(length as usize * WINDOW_SPAN);
    for index in 0..length as i32 {
        for offset in -WINDOW_RADIUS..=WINDOW_RADIUS {
            table.push(reflect(index + offset, length) as usize);
        }
    }
    table
}

fn gaussian_kernel() -> [f64; WINDOW_SPAN] {
    let mut kernel = [0.0; WINDOW_SPAN];
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

    #[test]
    fn reflect_offset_table_matches_direct_reflection_including_degenerate_sizes() {
        // 覆盖 length=1 的早退分支与小于窗口半径的退化尺寸，确保预计算表与逐点折叠一致。
        for length in [1u32, 2, 3, 5, 6, 11, 12, 37] {
            let table = reflect_offset_table(length);
            assert_eq!(table.len(), length as usize * WINDOW_SPAN);
            for index in 0..length as i32 {
                for offset in -WINDOW_RADIUS..=WINDOW_RADIUS {
                    let slot = index as usize * WINDOW_SPAN + (offset + WINDOW_RADIUS) as usize;
                    assert_eq!(
                        table[slot],
                        reflect(index + offset, length) as usize,
                        "length={length} index={index} offset={offset}"
                    );
                }
            }
        }
    }

    #[test]
    fn ring_buffer_slots_never_alias_two_different_source_rows() {
        // 环形缓冲的正确性前提：同一输出行内，两个不同的源行不得落进同一槽位。
        // 边界行经镜像延拓后会重复引用同一源行（如 height=11、y=0 得到 5,4,3,2,1,0,1,2,3,4,5），
        // 这种重复共享槽位是安全的，ring_owner 会跳过重复填充；真正的风险只有不同源行互相覆盖。
        for height in 1u32..=200 {
            let table = reflect_offset_table(height);
            for y in 0..height as usize {
                let sources = &table[y * WINDOW_SPAN..(y + 1) * WINDOW_SPAN];
                let mut occupied = [usize::MAX; WINDOW_SPAN];
                for &source_y in sources {
                    let slot = source_y % WINDOW_SPAN;
                    if occupied[slot] != usize::MAX {
                        assert_eq!(
                            occupied[slot], source_y,
                            "height={height} y={y} slot={slot} 被两个不同源行占用"
                        );
                    }
                    occupied[slot] = source_y;
                }
            }
        }
    }

    #[test]
    fn odd_and_degenerate_dimensions_keep_producing_finite_scores() {
        // 非方形、极窄以及单像素图都要走通镜像延拓路径而不 panic。
        for (width, height) in [(1u32, 1u32), (1, 9), (9, 1), (3, 17), (17, 3), (13, 13)] {
            let left = GrayImage::from_fn(width, height, |x, y| {
                Luma([((x * 7 + y * 13) % 256) as u8])
            });
            let right = GrayImage::from_fn(width, height, |x, y| {
                Luma([((x * 3 + y * 29 + 11) % 256) as u8])
            });

            let score = StandardSsim::compute_gray(&left, &right).unwrap();
            assert!(score.is_finite(), "{width}x{height} 得到非有限分数 {score}");
            assert!(score <= 1.0 + 1e-12, "{width}x{height} 分数超出上界 {score}");
        }
    }

    #[test]
    fn cancellable_compute_stops_during_row_processing() {
        use std::cell::Cell;

        let left = GrayImage::from_pixel(64, 64, Luma([120]));
        let right = GrayImage::from_pixel(64, 64, Luma([121]));
        let checks = Cell::new(0);

        let error = StandardSsim::compute_gray_cancellable(&left, &right, || {
            let next = checks.get() + 1;
            checks.set(next);
            next >= 3
        })
        .unwrap_err();

        assert!(error.to_string().contains("已取消"));
        assert!(checks.get() >= 3);
    }
}
