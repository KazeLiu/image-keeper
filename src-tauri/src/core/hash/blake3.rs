use crate::error::Result;
use blake3::Hasher;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// BLAKE3 哈希计算器
pub struct Blake3Computer;

impl Blake3Computer {
    /// 计算文件的 BLAKE3 哈希
    pub fn compute_file_hash(file_path: &Path) -> Result<String> {
        let mut file = File::open(file_path)?;
        let mut hasher = Hasher::new();

        // 使用缓冲区读取文件
        let mut buffer = vec![0u8; 1024 * 1024]; // 1MB 缓冲区

        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        let hash = hasher.finalize();
        Ok(hash.to_hex().to_string())
    }

    /// 批量计算图片哈希
    pub fn compute_hashes_batch(file_paths: &[&Path]) -> Vec<Result<String>> {
        file_paths
            .iter()
            .map(|path| Self::compute_file_hash(path))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_compute_file_hash() {
        // 创建临时测试文件
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_hash.txt");

        let mut file = File::create(&test_file).unwrap();
        file.write_all(b"Hello, ImageKeeper!").unwrap();

        let hash = Blake3Computer::compute_file_hash(&test_file).unwrap();

        // BLAKE3 哈希应该是64个字符的十六进制字符串
        assert_eq!(hash.len(), 64);

        // 清理测试文件
        std::fs::remove_file(test_file).ok();
    }
}
