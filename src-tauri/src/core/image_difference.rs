use std::collections::VecDeque;
use std::io::Cursor;
use std::path::Path;

use base64::{engine::general_purpose, Engine as _};
use image::{io::Reader as ImageReader, DynamicImage, ImageOutputFormat, Rgba, RgbaImage};

use crate::core::image_metrics::normalized_pair;
use crate::error::{AppError, Result};

const PREVIEW_MAX_EDGE: u32 = 1600;
const MAX_SOURCE_PIXELS: u64 = 25_000_000;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DifferencePreview {
    pub baseline_data_url: String,
    pub candidate_data_url: String,
    pub highlight_data_url: String,
    pub width: u32,
    pub height: u32,
    pub changed_pixel_ratio: f64,
    pub region_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Bounds {
    min_x: u32,
    min_y: u32,
    max_x: u32,
    max_y: u32,
}

impl Bounds {
    fn new(min_x: u32, min_y: u32, max_x: u32, max_y: u32) -> Self {
        Self {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }

    fn include(&mut self, x: u32, y: u32) {
        self.min_x = self.min_x.min(x);
        self.min_y = self.min_y.min(y);
        self.max_x = self.max_x.max(x);
        self.max_y = self.max_y.max(y);
    }
}

#[derive(Debug)]
struct Region {
    bounds: Bounds,
    pixels: Vec<usize>,
}

fn detect_regions(mask: &[bool], width: u32, height: u32, min_area: usize) -> Vec<Region> {
    let width_usize = width as usize;
    let mut visited = vec![false; mask.len()];
    let mut regions = Vec::new();

    for start in 0..mask.len() {
        if !mask[start] || visited[start] {
            continue;
        }

        visited[start] = true;
        let mut queue = VecDeque::from([start]);
        let start_x = (start % width_usize) as u32;
        let start_y = (start / width_usize) as u32;
        let mut bounds = Bounds::new(start_x, start_y, start_x, start_y);
        let mut pixels = Vec::new();

        while let Some(index) = queue.pop_front() {
            pixels.push(index);
            let x = (index % width_usize) as i32;
            let y = (index / width_usize) as i32;
            bounds.include(x as u32, y as u32);

            for offset_y in -1..=1 {
                for offset_x in -1..=1 {
                    if offset_x == 0 && offset_y == 0 {
                        continue;
                    }
                    let next_x = x + offset_x;
                    let next_y = y + offset_y;
                    if next_x < 0 || next_y < 0 || next_x >= width as i32 || next_y >= height as i32
                    {
                        continue;
                    }
                    let next = next_y as usize * width_usize + next_x as usize;
                    if mask[next] && !visited[next] {
                        visited[next] = true;
                        queue.push_back(next);
                    }
                }
            }
        }

        if pixels.len() >= min_area {
            regions.push(Region { bounds, pixels });
        }
    }

    regions
}

pub fn compute_difference_preview(
    baseline_path: &Path,
    candidate_path: &Path,
    sensitivity: u8,
) -> Result<DifferencePreview> {
    let (baseline_width, baseline_height) = source_dimensions(baseline_path)?;
    let (candidate_width, candidate_height) = source_dimensions(candidate_path)?;
    validate_preview_dimensions(baseline_width, baseline_height)?;
    validate_preview_dimensions(candidate_width, candidate_height)?;
    let (baseline, candidate) =
        normalized_pair(baseline_path, candidate_path, Some(PREVIEW_MAX_EDGE))?;
    let baseline = baseline.to_rgba8();
    let candidate = candidate.to_rgba8();
    let (width, height) = baseline.dimensions();
    let threshold = sensitivity_threshold(sensitivity);
    let raw_mask = baseline
        .pixels()
        .zip(candidate.pixels())
        .map(|(left, right)| {
            left.0
                .iter()
                .zip(right.0.iter())
                .map(|(left, right)| left.abs_diff(*right))
                .max()
                .unwrap_or(0)
                > threshold
        })
        .collect::<Vec<_>>();
    let total_pixels = width as usize * height as usize;
    let min_area = 9usize.max(total_pixels / 100_000);
    let regions = detect_regions(&raw_mask, width, height, min_area);
    let changed_pixels = regions
        .iter()
        .map(|region| region.pixels.len())
        .sum::<usize>();
    let mut highlighted = candidate.clone();

    for region in &regions {
        overlay_region(&mut highlighted, &region.pixels);
        draw_region_bounds(&mut highlighted, region.bounds);
    }

    Ok(DifferencePreview {
        baseline_data_url: encode_png(DynamicImage::ImageRgba8(baseline))?,
        candidate_data_url: encode_png(DynamicImage::ImageRgba8(candidate))?,
        highlight_data_url: encode_png(DynamicImage::ImageRgba8(highlighted))?,
        width,
        height,
        changed_pixel_ratio: if total_pixels == 0 {
            0.0
        } else {
            changed_pixels as f64 / total_pixels as f64
        },
        region_count: regions.len(),
    })
}

fn source_dimensions(path: &Path) -> Result<(u32, u32)> {
    Ok(ImageReader::open(path)?
        .with_guessed_format()?
        .into_dimensions()?)
}

fn validate_preview_dimensions(width: u32, height: u32) -> Result<()> {
    let pixels = u64::from(width) * u64::from(height);
    if width == 0 || height == 0 || pixels > MAX_SOURCE_PIXELS {
        return Err(AppError::ValidationError(
            "图片像素数量超过差异预览上限，请先缩小图片".to_string(),
        ));
    }
    Ok(())
}

fn sensitivity_threshold(sensitivity: u8) -> u8 {
    let sensitivity = sensitivity.min(100) as u16;
    (56 - (48 * sensitivity + 50) / 100) as u8
}

fn overlay_region(image: &mut RgbaImage, pixels: &[usize]) {
    let width = image.width() as usize;
    for index in pixels {
        let x = (*index % width) as u32;
        let y = (*index / width) as u32;
        let pixel = image.get_pixel_mut(x, y);
        pixel.0[0] = ((pixel.0[0] as u16 * 45 + 255 * 55) / 100) as u8;
        pixel.0[1] = (pixel.0[1] as u16 * 45 / 100) as u8;
        pixel.0[2] = (pixel.0[2] as u16 * 45 / 100) as u8;
        pixel.0[3] = pixel.0[3].max(180);
    }
}

fn draw_region_bounds(image: &mut RgbaImage, bounds: Bounds) {
    let max_image_x = image.width().saturating_sub(1);
    let max_image_y = image.height().saturating_sub(1);
    let left = bounds.min_x.saturating_sub(2);
    let top = bounds.min_y.saturating_sub(2);
    let right = bounds.max_x.saturating_add(2).min(max_image_x);
    let bottom = bounds.max_y.saturating_add(2).min(max_image_y);
    let outline = Rgba([255, 196, 0, 255]);

    for inset in 0..2 {
        let x1 = left.saturating_add(inset).min(right);
        let x2 = right.saturating_sub(inset).max(x1);
        let y1 = top.saturating_add(inset).min(bottom);
        let y2 = bottom.saturating_sub(inset).max(y1);
        for x in x1..=x2 {
            image.put_pixel(x, y1, outline);
            image.put_pixel(x, y2, outline);
        }
        for y in y1..=y2 {
            image.put_pixel(x1, y, outline);
            image.put_pixel(x2, y, outline);
        }
    }
}

fn encode_png(image: DynamicImage) -> Result<String> {
    let mut png = Cursor::new(Vec::new());
    image.write_to(&mut png, ImageOutputFormat::Png)?;
    Ok(format!(
        "data:image/png;base64,{}",
        general_purpose::STANDARD.encode(png.into_inner())
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb, Rgba};
    use tempfile::tempdir;

    fn write_image(path: &std::path::Path, image: ImageBuffer<Rgb<u8>, Vec<u8>>) {
        image.save(path).unwrap();
    }

    #[test]
    fn detects_one_local_difference_region() {
        let width = 24;
        let height = 18;
        let mut mask = vec![false; width * height];
        for y in 6..11 {
            for x in 8..15 {
                mask[y * width + x] = true;
            }
        }

        let regions = detect_regions(&mask, width as u32, height as u32, 9);

        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].bounds, Bounds::new(8, 6, 14, 10));
        assert_eq!(regions[0].pixels.len(), 35);
    }

    #[test]
    fn creates_a_highlight_preview_for_a_local_change() {
        let dir = tempdir().unwrap();
        let baseline = dir.path().join("baseline.png");
        let candidate = dir.path().join("candidate.png");
        write_image(
            &baseline,
            ImageBuffer::from_pixel(24, 18, Rgb([255, 255, 255])),
        );
        write_image(
            &candidate,
            ImageBuffer::from_fn(24, 18, |x, y| {
                if (8..15).contains(&x) && (6..11).contains(&y) {
                    Rgb([0, 0, 0])
                } else {
                    Rgb([255, 255, 255])
                }
            }),
        );

        let preview = compute_difference_preview(&baseline, &candidate, 50).unwrap();

        assert_eq!(preview.region_count, 1);
        assert!(preview.changed_pixel_ratio > 0.0);
        assert_eq!((preview.width, preview.height), (24, 18));
        assert!(preview
            .highlight_data_url
            .starts_with("data:image/png;base64,"));
    }

    #[test]
    fn identical_images_have_no_highlight_regions() {
        let dir = tempdir().unwrap();
        let baseline = dir.path().join("baseline.png");
        let candidate = dir.path().join("candidate.png");
        let image = ImageBuffer::from_pixel(24, 18, Rgb([120, 80, 40]));
        write_image(&baseline, image.clone());
        write_image(&candidate, image);

        let preview = compute_difference_preview(&baseline, &candidate, 50).unwrap();

        assert_eq!(preview.region_count, 0);
        assert_eq!(preview.changed_pixel_ratio, 0.0);
    }

    #[test]
    fn filters_a_single_changed_pixel_as_noise() {
        let dir = tempdir().unwrap();
        let baseline = dir.path().join("baseline.png");
        let candidate = dir.path().join("candidate.png");
        write_image(
            &baseline,
            ImageBuffer::from_pixel(24, 18, Rgb([255, 255, 255])),
        );
        write_image(
            &candidate,
            ImageBuffer::from_fn(24, 18, |x, y| {
                if (x, y) == (12, 9) {
                    Rgb([0, 0, 0])
                } else {
                    Rgb([255, 255, 255])
                }
            }),
        );

        let preview = compute_difference_preview(&baseline, &candidate, 50).unwrap();

        assert_eq!(preview.region_count, 0);
        assert_eq!(preview.changed_pixel_ratio, 0.0);
    }

    #[test]
    fn detects_a_visible_alpha_channel_change() {
        let dir = tempdir().unwrap();
        let baseline = dir.path().join("baseline.png");
        let candidate = dir.path().join("candidate.png");
        ImageBuffer::from_pixel(24, 18, Rgba([80u8, 120, 160, 0]))
            .save(&baseline)
            .unwrap();
        ImageBuffer::from_fn(24, 18, |x, y| {
            let alpha = if (8..15).contains(&x) && (6..11).contains(&y) {
                255
            } else {
                0
            };
            Rgba([80u8, 120, 160, alpha])
        })
        .save(&candidate)
        .unwrap();

        let preview = compute_difference_preview(&baseline, &candidate, 50).unwrap();

        assert_eq!(preview.region_count, 1);
        assert!(preview.changed_pixel_ratio > 0.0);
    }

    #[test]
    fn highlight_overlay_stays_visible_on_a_transparent_candidate_pixel() {
        let mut image = RgbaImage::from_pixel(3, 3, Rgba([80, 120, 160, 0]));

        overlay_region(&mut image, &[4]);

        assert!(image.get_pixel(1, 1).0[3] >= 180);
    }

    #[test]
    fn rejects_source_images_over_the_preview_pixel_budget() {
        assert!(validate_preview_dimensions(6000, 6000).is_err());
        assert!(validate_preview_dimensions(4000, 4000).is_ok());
    }
}
