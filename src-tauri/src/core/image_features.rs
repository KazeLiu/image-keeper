use crate::core::phash::PHashComputer;
use crate::error::{AppError, Result};
use blake3::Hasher;
use image::GenericImageView;
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
    pub color_type: String,
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
    let color_type = format!("{:?}", image.color());

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
        color_type,
        blake3_hash: compute_blake3(path)?,
        phash: PHashComputer::compute_from_image(&image)?,
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

pub fn phash_distance(left: &str, right: &str) -> Option<u32> {
    PHashComputer::hamming_distance(left, right).ok()
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
        assert_eq!(left.color_type, "Rgb8");
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
