use crate::core::phash::PHashComputer;
use crate::core::ssim::{compute::SsimComputer, resize::ImageResizer};
use crate::error::{AppError, Result};
use base64::{engine::general_purpose, Engine as _};
use image::ImageOutputFormat;
use serde::Serialize;
use std::io::Cursor;
use std::path::Path;
use std::time::{Instant, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedTestImage {
    pub path: String,
    pub file_name: String,
    pub file_size: u64,
    pub width: u32,
    pub height: u32,
    pub modified_at_ms: u64,
    pub thumbnail_data_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestImagePhashResult {
    pub phash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestSsimResult {
    pub score: f64,
    pub duration_ms: u64,
}

pub fn thumbnail_dimensions(width: u32, height: u32) -> (u32, u32) {
    SsimComputer::target_dimensions(width, height, 200)
}

pub fn pair_target_dimensions(left: (u32, u32), right: (u32, u32)) -> (u32, u32) {
    SsimComputer::pair_target_dimensions(left, right)
}

pub fn load_test_image_sync(path: String) -> Result<LoadedTestImage> {
    let canonical = std::fs::canonicalize(&path).map_err(|_| AppError::InvalidPath)?;
    let metadata = std::fs::metadata(&canonical)?;
    if !metadata.is_file() {
        return Err(AppError::InvalidPath);
    }

    let image = image::open(&canonical)?;
    let (thumb_width, thumb_height) = thumbnail_dimensions(image.width(), image.height());
    let thumbnail = if (image.width(), image.height()) == (thumb_width, thumb_height) {
        image.clone()
    } else {
        ImageResizer::resize_to_target(&image, thumb_width, thumb_height)?
    };
    let mut png = Cursor::new(Vec::new());
    thumbnail.write_to(&mut png, ImageOutputFormat::Png)?;
    let modified_at_ms = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);

    Ok(LoadedTestImage {
        path: canonical.to_string_lossy().into_owned(),
        file_name: canonical
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("未命名图片")
            .to_string(),
        file_size: metadata.len(),
        width: image.width(),
        height: image.height(),
        modified_at_ms,
        thumbnail_data_url: format!(
            "data:image/png;base64,{}",
            general_purpose::STANDARD.encode(png.into_inner())
        ),
    })
}

pub fn compute_test_phash_sync(
    path: String,
    file_size: u64,
    modified_at_ms: u64,
) -> Result<TestImagePhashResult> {
    let canonical = std::fs::canonicalize(&path).map_err(|_| AppError::InvalidPath)?;
    validate_file_fingerprint(&canonical, file_size, modified_at_ms)?;
    let image = image::open(&canonical)?;
    Ok(TestImagePhashResult {
        phash: PHashComputer::compute_from_image(&image)?,
    })
}

pub(crate) fn normalized_pair(
    left_path: &Path,
    right_path: &Path,
    max_edge: Option<u32>,
) -> Result<(image::DynamicImage, image::DynamicImage)> {
    let left = image::open(left_path)?;
    let right = image::open(right_path)?;
    let (mut target_width, mut target_height) = pair_target_dimensions(
        (left.width(), left.height()),
        (right.width(), right.height()),
    );
    if let Some(max_edge) = max_edge {
        (target_width, target_height) =
            SsimComputer::target_dimensions(target_width, target_height, max_edge);
    }

    let normalize = |image: image::DynamicImage| -> Result<image::DynamicImage> {
        if (image.width(), image.height()) == (target_width, target_height) {
            Ok(image)
        } else {
            ImageResizer::resize_to_target(&image, target_width, target_height)
        }
    };

    Ok((normalize(left)?, normalize(right)?))
}

pub fn compute_test_ssim_sync(
    baseline_path: String,
    candidate_path: String,
    baseline_file_size: u64,
    baseline_modified_at_ms: u64,
    candidate_file_size: u64,
    candidate_modified_at_ms: u64,
) -> Result<TestSsimResult> {
    let started = Instant::now();
    validate_file_fingerprint(
        Path::new(&baseline_path),
        baseline_file_size,
        baseline_modified_at_ms,
    )?;
    validate_file_fingerprint(
        Path::new(&candidate_path),
        candidate_file_size,
        candidate_modified_at_ms,
    )?;
    Ok(TestSsimResult {
        score: SsimComputer::compute_from_files(
            Path::new(&baseline_path),
            Path::new(&candidate_path),
        )?,
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

pub(crate) fn validate_file_fingerprint(
    path: &Path,
    expected_size: u64,
    expected_modified_at_ms: u64,
) -> Result<()> {
    let metadata = std::fs::metadata(path)?;
    let modified_at_ms = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    if metadata.len() != expected_size || modified_at_ms != expected_modified_at_ms {
        return Err(AppError::ValidationError(
            "图片已发生变化，请移除后重新导入".to_string(),
        ));
    }
    Ok(())
}
