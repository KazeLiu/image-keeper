use crate::core::algorithm_profile::algorithm_pool;
use crate::core::image_difference;
use crate::core::image_metrics as shared;
use crate::error::{AppError, Result};

pub type TestImageInfo = shared::LoadedTestImage;
pub type TestImagePhashResult = shared::TestImagePhashResult;
pub type TestDifferencePreviewResult = image_difference::DifferencePreview;
pub type TestSsimResult = shared::TestSsimResult;

fn thumbnail_dimensions(width: u32, height: u32) -> (u32, u32) {
    shared::thumbnail_dimensions(width, height)
}

fn pair_target_dimensions(left: (u32, u32), right: (u32, u32)) -> (u32, u32) {
    shared::pair_target_dimensions(left, right)
}

fn load_test_image_sync(path: String) -> Result<TestImageInfo> {
    shared::load_test_image_sync(path)
}

fn compute_test_phash_sync(
    path: String,
    file_size: u64,
    modified_at_ms: u64,
) -> Result<TestImagePhashResult> {
    shared::compute_test_phash_sync(path, file_size, modified_at_ms)
}

fn compute_ssim_sync(
    baseline_path: String,
    candidate_path: String,
    baseline_file_size: u64,
    baseline_modified_at_ms: u64,
    candidate_file_size: u64,
    candidate_modified_at_ms: u64,
) -> Result<TestSsimResult> {
    shared::compute_test_ssim_sync(
        baseline_path,
        candidate_path,
        baseline_file_size,
        baseline_modified_at_ms,
        candidate_file_size,
        candidate_modified_at_ms,
    )
}

fn compute_difference_preview_sync(
    baseline_path: String,
    candidate_path: String,
    baseline_file_size: u64,
    baseline_modified_at_ms: u64,
    candidate_file_size: u64,
    candidate_modified_at_ms: u64,
    sensitivity: u8,
) -> Result<TestDifferencePreviewResult> {
    if sensitivity > 100 {
        return Err(AppError::ValidationError(
            "差异灵敏度必须在 0 到 100 之间".to_string(),
        ));
    }
    shared::validate_file_fingerprint(
        std::path::Path::new(&baseline_path),
        baseline_file_size,
        baseline_modified_at_ms,
    )?;
    shared::validate_file_fingerprint(
        std::path::Path::new(&candidate_path),
        candidate_file_size,
        candidate_modified_at_ms,
    )?;
    let preview = image_difference::compute_difference_preview(
        std::path::Path::new(&baseline_path),
        std::path::Path::new(&candidate_path),
        sensitivity,
    )?;
    shared::validate_file_fingerprint(
        std::path::Path::new(&baseline_path),
        baseline_file_size,
        baseline_modified_at_ms,
    )?;
    shared::validate_file_fingerprint(
        std::path::Path::new(&candidate_path),
        candidate_file_size,
        candidate_modified_at_ms,
    )?;
    Ok(preview)
}

fn join_error(error: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("图片指标任务执行失败: {error}"))
}

#[tauri::command]
pub async fn load_test_image(path: String) -> Result<TestImageInfo> {
    tauri::async_runtime::spawn_blocking(move || {
        algorithm_pool().install(|| load_test_image_sync(path))
    })
    .await
    .map_err(join_error)?
}

#[tauri::command]
pub async fn compute_test_phash(
    path: String,
    file_size: u64,
    modified_at_ms: u64,
) -> Result<TestImagePhashResult> {
    tauri::async_runtime::spawn_blocking(move || {
        algorithm_pool().install(|| compute_test_phash_sync(path, file_size, modified_at_ms))
    })
    .await
    .map_err(join_error)?
}

#[tauri::command]
pub async fn compute_test_ssim(
    baseline_path: String,
    candidate_path: String,
    baseline_file_size: u64,
    baseline_modified_at_ms: u64,
    candidate_file_size: u64,
    candidate_modified_at_ms: u64,
) -> Result<TestSsimResult> {
    tauri::async_runtime::spawn_blocking(move || {
        algorithm_pool().install(|| {
            compute_ssim_sync(
                baseline_path,
                candidate_path,
                baseline_file_size,
                baseline_modified_at_ms,
                candidate_file_size,
                candidate_modified_at_ms,
            )
        })
    })
    .await
    .map_err(join_error)?
}

#[tauri::command]
pub async fn compute_test_difference_preview(
    baseline_path: String,
    candidate_path: String,
    baseline_file_size: u64,
    baseline_modified_at_ms: u64,
    candidate_file_size: u64,
    candidate_modified_at_ms: u64,
    sensitivity: u8,
) -> Result<TestDifferencePreviewResult> {
    tauri::async_runtime::spawn_blocking(move || {
        algorithm_pool().install(|| {
            compute_difference_preview_sync(
                baseline_path,
                candidate_path,
                baseline_file_size,
                baseline_modified_at_ms,
                candidate_file_size,
                candidate_modified_at_ms,
                sensitivity,
            )
        })
    })
    .await
    .map_err(join_error)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::image_features::extract_image_features;
    use crate::core::ssim::compute::SsimComputer;
    use image::{ImageBuffer, Rgb};
    use tempfile::tempdir;

    fn write_fixture(path: &std::path::Path, width: u32, height: u32, value: u8) {
        ImageBuffer::from_pixel(width, height, Rgb([value, value, value]))
            .save(path)
            .unwrap();
    }

    fn compute_pair(left: &TestImageInfo, right: &TestImageInfo) -> TestSsimResult {
        compute_ssim_sync(
            left.path.clone(),
            right.path.clone(),
            left.file_size,
            left.modified_at_ms,
            right.file_size,
            right.modified_at_ms,
        )
        .unwrap()
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
    fn loaded_phash_matches_the_formal_scanner_algorithm() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("large.png");
        write_fixture(&path, 1000, 600, 80);
        let info = load_test_image_sync(path.to_string_lossy().into_owned()).unwrap();

        let phash = compute_test_phash_sync(info.path.clone(), info.file_size, info.modified_at_ms)
            .unwrap()
            .phash;
        let formal_features = extract_image_features(std::path::Path::new(&info.path)).unwrap();

        assert_eq!((info.width, info.height), (1000, 600));
        assert!(info
            .thumbnail_data_url
            .starts_with("data:image/png;base64,"));
        assert_eq!(phash, formal_features.phash);
    }

    #[test]
    fn test_tool_and_main_program_return_the_exact_same_standard_ssim() {
        let dir = tempdir().unwrap();
        let left_path = dir.path().join("left.png");
        let right_path = dir.path().join("right.png");
        ImageBuffer::from_fn(80, 60, |x, y| {
            Rgb([((x * 7 + y * 3) % 256) as u8, (x * 5) as u8, (y * 9) as u8])
        })
        .save(&left_path)
        .unwrap();
        ImageBuffer::from_fn(160, 120, |x, y| {
            Rgb([
                ((x * 11 + y * 13) % 256) as u8,
                (x * 3) as u8,
                (y * 5) as u8,
            ])
        })
        .save(&right_path)
        .unwrap();
        let left = load_test_image_sync(left_path.to_string_lossy().into_owned()).unwrap();
        let right = load_test_image_sync(right_path.to_string_lossy().into_owned()).unwrap();

        let expected = SsimComputer::compute_from_files(&left_path, &right_path).unwrap();
        let actual = compute_pair(&left, &right).score;

        assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
    }

    #[test]
    fn identical_pair_scores_one() {
        let dir = tempdir().unwrap();
        let left_path = dir.path().join("left.png");
        let right_path = dir.path().join("right.png");
        write_fixture(&left_path, 640, 480, 100);
        write_fixture(&right_path, 640, 480, 100);
        let left = load_test_image_sync(left_path.to_string_lossy().into_owned()).unwrap();
        let right = load_test_image_sync(right_path.to_string_lossy().into_owned()).unwrap();

        assert!((compute_pair(&left, &right).score - 1.0).abs() < 1e-12);
    }

    #[test]
    fn standard_ssim_rejects_a_file_changed_after_import() {
        let dir = tempdir().unwrap();
        let left_path = dir.path().join("left.png");
        let right_path = dir.path().join("right.png");
        write_fixture(&left_path, 32, 32, 100);
        write_fixture(&right_path, 32, 32, 100);
        let left = load_test_image_sync(left_path.to_string_lossy().into_owned()).unwrap();
        let right = load_test_image_sync(right_path.to_string_lossy().into_owned()).unwrap();
        write_fixture(&right_path, 64, 64, 80);

        let error = compute_ssim_sync(
            left.path,
            right.path,
            left.file_size,
            left.modified_at_ms,
            right.file_size,
            right.modified_at_ms,
        )
        .unwrap_err();

        assert!(error.to_string().contains("重新导入"));
    }

    #[test]
    fn difference_preview_rejects_changed_files() {
        let dir = tempdir().unwrap();
        let baseline = dir.path().join("baseline.png");
        let candidate = dir.path().join("candidate.png");
        write_fixture(&baseline, 32, 32, 100);
        write_fixture(&candidate, 32, 32, 100);
        let baseline_info = load_test_image_sync(baseline.to_string_lossy().into_owned()).unwrap();
        let candidate_info =
            load_test_image_sync(candidate.to_string_lossy().into_owned()).unwrap();
        write_fixture(&candidate, 64, 64, 80);

        let error = compute_difference_preview_sync(
            baseline_info.path,
            candidate_info.path,
            baseline_info.file_size,
            baseline_info.modified_at_ms,
            candidate_info.file_size,
            candidate_info.modified_at_ms,
            50,
        )
        .unwrap_err();

        assert!(error.to_string().contains("重新导入"));
    }
}
