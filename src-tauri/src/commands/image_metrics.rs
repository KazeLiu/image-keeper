use crate::core::phash::PHashComputer;
use crate::core::ssim::{compute::SsimComputer, resize::ImageResizer, standard::StandardSsim};
use crate::error::{AppError, Result};
use base64::{engine::general_purpose, Engine as _};
use image::ImageOutputFormat;
use serde::Serialize;
use std::io::Cursor;
use std::path::Path;
use std::time::{Instant, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestImageInfo {
    pub path: String,
    pub file_name: String,
    pub file_size: u64,
    pub width: u32,
    pub height: u32,
    pub modified_at_ms: u64,
    pub phash: String,
    pub thumbnail_data_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestLowPrecisionResult {
    pub similarity: f64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestStandardSsimResult {
    pub score: f64,
    pub duration_ms: u64,
}

fn thumbnail_dimensions(width: u32, height: u32) -> (u32, u32) {
    SsimComputer::target_dimensions(width, height, 200)
}

fn pair_target_dimensions(left: (u32, u32), right: (u32, u32)) -> (u32, u32) {
    let left_key = (left.0 as u64 * left.1 as u64, left.0, left.1);
    let right_key = (right.0 as u64 * right.1 as u64, right.0, right.1);
    if left_key <= right_key {
        left
    } else {
        right
    }
}

fn load_test_image_sync(path: String) -> Result<TestImageInfo> {
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

    Ok(TestImageInfo {
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
        phash: PHashComputer::compute_from_image(&image)?,
        thumbnail_data_url: format!(
            "data:image/png;base64,{}",
            general_purpose::STANDARD.encode(png.into_inner())
        ),
    })
}

fn normalized_pair(
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

fn compute_low_precision_sync(
    baseline_path: String,
    candidate_path: String,
) -> Result<TestLowPrecisionResult> {
    let started = Instant::now();

    Ok(TestLowPrecisionResult {
        similarity: SsimComputer::compute_from_files(
            Path::new(&baseline_path),
            Path::new(&candidate_path),
        )?,
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

fn compute_standard_ssim_sync(
    baseline_path: String,
    candidate_path: String,
    baseline_file_size: u64,
    baseline_modified_at_ms: u64,
    candidate_file_size: u64,
    candidate_modified_at_ms: u64,
) -> Result<TestStandardSsimResult> {
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
    let (baseline, candidate) =
        normalized_pair(Path::new(&baseline_path), Path::new(&candidate_path), None)?;

    Ok(TestStandardSsimResult {
        score: StandardSsim::compute_owned(baseline, candidate)?,
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

fn validate_file_fingerprint(
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

fn join_error(error: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("图片指标任务执行失败: {error}"))
}

#[tauri::command]
pub async fn load_test_image(path: String) -> Result<TestImageInfo> {
    tauri::async_runtime::spawn_blocking(move || load_test_image_sync(path))
        .await
        .map_err(join_error)?
}

#[tauri::command]
pub async fn compute_test_low_precision(
    baseline_path: String,
    candidate_path: String,
) -> Result<TestLowPrecisionResult> {
    tauri::async_runtime::spawn_blocking(move || {
        compute_low_precision_sync(baseline_path, candidate_path)
    })
    .await
    .map_err(join_error)?
}

#[tauri::command]
pub async fn compute_test_standard_ssim(
    baseline_path: String,
    candidate_path: String,
    baseline_file_size: u64,
    baseline_modified_at_ms: u64,
    candidate_file_size: u64,
    candidate_modified_at_ms: u64,
) -> Result<TestStandardSsimResult> {
    tauri::async_runtime::spawn_blocking(move || {
        compute_standard_ssim_sync(
            baseline_path,
            candidate_path,
            baseline_file_size,
            baseline_modified_at_ms,
            candidate_file_size,
            candidate_modified_at_ms,
        )
    })
    .await
    .map_err(join_error)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};
    use tempfile::tempdir;

    fn write_fixture(path: &std::path::Path, width: u32, height: u32, value: u8) {
        ImageBuffer::from_pixel(width, height, Rgb([value, value, value]))
            .save(path)
            .unwrap();
    }

    #[test]
    fn thumbnail_longest_edge_is_at_most_200() {
        assert_eq!(thumbnail_dimensions(2000, 1000), (200, 100));
        assert_eq!(thumbnail_dimensions(160, 120), (160, 120));
    }

    #[test]
    fn pair_target_uses_smaller_pixel_image_without_512_cap() {
        assert_eq!(
            pair_target_dimensions((4000, 3000), (1600, 900)),
            (1600, 900)
        );
        assert_eq!(pair_target_dimensions((100, 200), (200, 100)), (100, 200));
    }

    #[test]
    fn low_precision_uses_the_same_directional_resize_as_the_main_program() {
        let dir = tempdir().unwrap();
        let baseline = dir.path().join("baseline.png");
        let candidate = dir.path().join("candidate.png");
        ImageBuffer::from_fn(32, 24, |x, y| {
            Rgb([((x * 7 + y * 3) % 256) as u8, (x * 5) as u8, (y * 9) as u8])
        })
        .save(&baseline)
        .unwrap();
        ImageBuffer::from_fn(64, 48, |x, y| {
            Rgb([
                ((x * 11 + y * 13) % 256) as u8,
                (x * 3) as u8,
                (y * 5) as u8,
            ])
        })
        .save(&candidate)
        .unwrap();

        let expected = SsimComputer::compute_from_files(&baseline, &candidate).unwrap();
        let actual = compute_low_precision_sync(
            baseline.to_string_lossy().into_owned(),
            candidate.to_string_lossy().into_owned(),
        )
        .unwrap()
        .similarity;

        assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
    }

    #[test]
    fn loaded_info_contains_real_thumbnail_and_no_database_state() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("large.png");
        write_fixture(&path, 1000, 600, 80);

        let info = load_test_image_sync(path.to_string_lossy().into_owned()).unwrap();

        assert_eq!((info.width, info.height), (1000, 600));
        assert!(info
            .thumbnail_data_url
            .starts_with("data:image/png;base64,"));
        assert!(!info.phash.is_empty());
    }

    #[test]
    fn identical_pair_scores_one_for_both_similarity_algorithms() {
        let dir = tempdir().unwrap();
        let left = dir.path().join("left.png");
        let right = dir.path().join("right.png");
        write_fixture(&left, 640, 480, 100);
        write_fixture(&right, 640, 480, 100);
        let left_info = load_test_image_sync(left.to_string_lossy().into_owned()).unwrap();
        let right_info = load_test_image_sync(right.to_string_lossy().into_owned()).unwrap();

        let low =
            compute_low_precision_sync(left_info.path.clone(), right_info.path.clone()).unwrap();
        let high = compute_standard_ssim_sync(
            left_info.path,
            right_info.path,
            left_info.file_size,
            left_info.modified_at_ms,
            right_info.file_size,
            right_info.modified_at_ms,
        )
        .unwrap();

        assert!((low.similarity - 1.0).abs() < 1e-12);
        assert!((high.score - 1.0).abs() < 1e-12);
    }

    #[test]
    fn standard_ssim_rejects_a_file_changed_after_import() {
        let dir = tempdir().unwrap();
        let left = dir.path().join("left.png");
        let right = dir.path().join("right.png");
        write_fixture(&left, 32, 32, 100);
        write_fixture(&right, 32, 32, 100);
        let left_info = load_test_image_sync(left.to_string_lossy().into_owned()).unwrap();
        let right_info = load_test_image_sync(right.to_string_lossy().into_owned()).unwrap();
        write_fixture(&right, 64, 64, 80);

        let error = compute_standard_ssim_sync(
            left_info.path,
            right_info.path,
            left_info.file_size,
            left_info.modified_at_ms,
            right_info.file_size,
            right_info.modified_at_ms,
        )
        .unwrap_err();

        assert!(error.to_string().contains("重新导入"));
    }
}
