use crate::error::{AppError, Result};
use blake3::Hasher;
use image::{DynamicImage, GenericImageView};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageFeatures {
    pub file_path: String,
    pub file_size: u64,
    pub modified_at: i64,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub blake3_hash: String,
    pub phash: String,
}

pub fn extract_image_features(path: &Path) -> Result<ImageFeatures> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| AppError::FileSystem(format!("读取文件元数据失败: {error}")))?;
    let modified_at = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|value| value.as_secs() as i64)
        .unwrap_or_default();
    let image = image::open(path)?;
    let (width, height) = image.dimensions();

    Ok(ImageFeatures {
        file_path: path.to_string_lossy().to_string(),
        file_size: metadata.len(),
        modified_at,
        width,
        height,
        format: path
            .extension()
            .map(|value| value.to_string_lossy().to_lowercase())
            .unwrap_or_default(),
        blake3_hash: compute_blake3(path)?,
        phash: compute_phash(&image),
    })
}

pub fn compute_blake3(path: &Path) -> Result<String> {
    let file =
        File::open(path).map_err(|error| AppError::FileSystem(format!("打开文件失败: {error}")))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Hasher::new();
    let mut buffer = [0_u8; 8192];

    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|error| AppError::FileSystem(format!("读取文件失败: {error}")))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }

    Ok(hasher.finalize().to_hex().to_string())
}

pub fn compute_phash(image: &DynamicImage) -> String {
    let resized = image.resize_exact(32, 32, image::imageops::FilterType::Lanczos3);
    let gray = resized.to_luma8();
    let dct_matrix = compute_dct(gray.as_raw(), 32, 32);
    let mut low_frequency = Vec::with_capacity(63);

    for y in 0..8 {
        for x in 0..8 {
            if x != 0 || y != 0 {
                low_frequency.push(dct_matrix[y * 32 + x]);
            }
        }
    }

    let mut sorted = low_frequency.clone();
    sorted.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let median = sorted[sorted.len() / 2];
    let mut hash = 0_u64;
    for (index, value) in low_frequency.iter().take(63).enumerate() {
        if *value > median {
            hash |= 1_u64 << index;
        }
    }

    format!("{hash:016x}")
}

pub fn phash_distance(left: &str, right: &str) -> Option<u32> {
    let left = u64::from_str_radix(left, 16).ok()?;
    let right = u64::from_str_radix(right, 16).ok()?;
    Some((left ^ right).count_ones())
}

fn compute_dct(pixels: &[u8], width: usize, height: usize) -> Vec<f64> {
    let mut dct = vec![0.0; width * height];
    for v in 0..height {
        for u in 0..width {
            let mut sum = 0.0;
            for y in 0..height {
                for x in 0..width {
                    let pixel = pixels[y * width + x] as f64;
                    let cos_x = ((2.0 * x as f64 + 1.0) * u as f64 * std::f64::consts::PI
                        / (2.0 * width as f64))
                        .cos();
                    let cos_y = ((2.0 * y as f64 + 1.0) * v as f64 * std::f64::consts::PI
                        / (2.0 * height as f64))
                        .cos();
                    sum += pixel * cos_x * cos_y;
                }
            }
            let cu = if u == 0 { 1.0 / 2.0_f64.sqrt() } else { 1.0 };
            let cv = if v == 0 { 1.0 / 2.0_f64.sqrt() } else { 1.0 };
            dct[v * width + u] = 0.25 * cu * cv * sum;
        }
    }
    dct
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, Rgb, RgbImage};

    #[test]
    fn extracts_stable_features_for_identical_files() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.png");
        let second = dir.path().join("second.png");
        let image = DynamicImage::ImageRgb8(RgbImage::from_fn(48, 32, |x, y| {
            Rgb([(x * 5) as u8, (y * 7) as u8, ((x + y) * 3) as u8])
        }));
        image.save(&first).unwrap();
        std::fs::copy(&first, &second).unwrap();

        let left = extract_image_features(&first).unwrap();
        let right = extract_image_features(&second).unwrap();

        assert_eq!(left.blake3_hash, right.blake3_hash);
        assert_eq!(left.phash, right.phash);
        assert_eq!((left.width, left.height), (48, 32));
        assert_eq!(left.format, "png");
    }

    #[test]
    fn computes_hex_phash_hamming_distance() {
        assert_eq!(
            phash_distance("0000000000000000", "0000000000000003"),
            Some(2)
        );
        assert_eq!(phash_distance("invalid", "0000000000000000"), None);
    }
}
