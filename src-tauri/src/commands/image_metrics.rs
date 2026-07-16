use crate::core::image_metrics as shared;
use crate::error::{AppError, Result};

pub type TestImageInfo = shared::LoadedTestImage;
pub type TestImagePhashResult = shared::TestImagePhashResult;
pub type TestLowPrecisionResult = shared::TestLowPrecisionResult;
pub type TestStandardSsimResult = shared::TestStandardSsimResult;

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

fn compute_low_precision_sync(
    baseline_path: String,
    candidate_path: String,
) -> Result<TestLowPrecisionResult> {
    shared::compute_low_precision_sync(baseline_path, candidate_path)
}

fn compute_standard_ssim_sync(
    baseline_path: String,
    candidate_path: String,
    baseline_file_size: u64,
    baseline_modified_at_ms: u64,
    candidate_file_size: u64,
    candidate_modified_at_ms: u64,
) -> Result<TestStandardSsimResult> {
    shared::compute_standard_ssim_sync(
        baseline_path,
        candidate_path,
        baseline_file_size,
        baseline_modified_at_ms,
        candidate_file_size,
        candidate_modified_at_ms,
    )
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
pub async fn compute_test_phash(
    path: String,
    file_size: u64,
    modified_at_ms: u64,
) -> Result<TestImagePhashResult> {
    tauri::async_runtime::spawn_blocking(move || {
        compute_test_phash_sync(path, file_size, modified_at_ms)
    })
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
    use crate::core::image_features::extract_image_features;
    use crate::core::ssim::compute::SsimComputer;
    use crate::core::ssim::resize::ImageResizer;
    use crate::core::ssim::standard::StandardSsim;
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
        let phash = compute_test_phash_sync(info.path.clone(), info.file_size, info.modified_at_ms)
            .unwrap()
            .phash;
        assert!(!phash.is_empty());

        let formal_features = extract_image_features(std::path::Path::new(&info.path)).unwrap();
        assert_eq!(phash, formal_features.phash);
    }

    #[test]
    fn standard_similarity_uses_the_shared_standard_algorithm_and_normalization() {
        let dir = tempdir().unwrap();
        let baseline = dir.path().join("baseline.png");
        let candidate = dir.path().join("candidate.png");
        ImageBuffer::from_fn(48, 32, |x, y| {
            Rgb([((x * 7 + y * 3) % 256) as u8, (x * 5) as u8, (y * 9) as u8])
        })
        .save(&baseline)
        .unwrap();
        ImageBuffer::from_fn(96, 64, |x, y| {
            Rgb([
                ((x * 11 + y * 13) % 256) as u8,
                (x * 3) as u8,
                (y * 5) as u8,
            ])
        })
        .save(&candidate)
        .unwrap();

        let baseline_info = load_test_image_sync(baseline.to_string_lossy().into_owned()).unwrap();
        let candidate_info =
            load_test_image_sync(candidate.to_string_lossy().into_owned()).unwrap();
        let baseline_image = image::open(&baseline_info.path).unwrap();
        let candidate_image = image::open(&candidate_info.path).unwrap();
        let (target_width, target_height) = pair_target_dimensions(
            (baseline_image.width(), baseline_image.height()),
            (candidate_image.width(), candidate_image.height()),
        );
        let normalize = |image: image::DynamicImage| {
            if image.width() == target_width && image.height() == target_height {
                image
            } else {
                ImageResizer::resize_to_target(&image, target_width, target_height).unwrap()
            }
        };
        let expected =
            StandardSsim::compute_owned(normalize(baseline_image), normalize(candidate_image))
                .unwrap();

        let actual = compute_standard_ssim_sync(
            baseline_info.path,
            candidate_info.path,
            baseline_info.file_size,
            baseline_info.modified_at_ms,
            candidate_info.file_size,
            candidate_info.modified_at_ms,
        )
        .unwrap()
        .score;

        assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
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
