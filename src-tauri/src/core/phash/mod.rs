pub mod engine;

use crate::error::Result;
use base64::{engine::general_purpose, Engine as _};
use std::path::Path;

/// pHash 计算器
pub struct PHashComputer;

impl PHashComputer {
    /// 计算图片的感知哈希 (简化的 DCT-based pHash)
    pub fn compute_phash(path: &Path) -> Result<String> {
        let img = image::open(path)?;

        // 1. 转为灰度并缩放到 32x32
        let gray = img.to_luma8();
        let resized = image::imageops::resize(&gray, 32, 32, image::imageops::FilterType::Lanczos3);

        // 2. 计算 DCT (简化版：使用均值哈希代替)
        let pixels: Vec<u8> = resized.into_raw();
        let sum: u32 = pixels.iter().map(|&p| p as u32).sum();
        let avg = sum / (32 * 32);

        // 3. 生成 64 位哈希 (8x8)
        let mut hash_bits = Vec::new();
        for chunk in pixels.chunks(16) {
            let chunk_avg: u32 = chunk.iter().map(|&p| p as u32).sum::<u32>() / chunk.len() as u32;
            hash_bits.push(if chunk_avg > avg { 1u8 } else { 0u8 });
        }

        // 4. 转换为 base64 字符串
        let hash_bytes: Vec<u8> = hash_bits
            .chunks(8)
            .map(|bits| {
                bits.iter()
                    .enumerate()
                    .fold(0u8, |acc, (i, &bit)| acc | (bit << i))
            })
            .collect();

        Ok(general_purpose::STANDARD.encode(&hash_bytes))
    }

    /// 计算两个 pHash 之间的汉明距离
    pub fn hamming_distance(hash1: &str, hash2: &str) -> Result<u32> {
        let bytes1 = general_purpose::STANDARD
            .decode(hash1)
            .map_err(|e| crate::error::AppError::Internal(format!("解析 hash1 失败: {:?}", e)))?;
        let bytes2 = general_purpose::STANDARD
            .decode(hash2)
            .map_err(|e| crate::error::AppError::Internal(format!("解析 hash2 失败: {:?}", e)))?;

        if bytes1.len() != bytes2.len() {
            return Err(crate::error::AppError::Internal(
                "哈希长度不一致".to_string(),
            ));
        }

        let mut distance = 0u32;
        for (b1, b2) in bytes1.iter().zip(bytes2.iter()) {
            let xor = b1 ^ b2;
            distance += xor.count_ones();
        }

        Ok(distance)
    }
}
