use crate::db::repository::Repository;
use crate::error::Result;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// 导出管理器
pub struct ExportManager;

impl ExportManager {
    /// 导出删除列表 (delete-list.txt)
    pub fn export_delete_list(
        output_path: &Path,
        root_path: &Path,
        repository: &Repository,
    ) -> Result<()> {
        let file = File::create(output_path)?;
        let mut writer = BufWriter::new(file);

        // 查询所有回收站中的文件
        let mut stmt = repository
            .conn()
            .prepare("SELECT original_path FROM recycle_bin ORDER BY recycled_at")?;

        let paths: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // 写入相对路径
        for original_path in paths {
            let path = Path::new(&original_path);
            if let Ok(relative) = path.strip_prefix(root_path) {
                writeln!(writer, "{}", relative.to_string_lossy())?;
            }
        }

        writer.flush()?;
        Ok(())
    }

    /// 导出详细报告 (report.csv)
    pub fn export_report(
        output_path: &Path,
        root_path: &Path,
        repository: &Repository,
    ) -> Result<()> {
        let file = File::create(output_path)?;
        let mut writer = BufWriter::new(file);

        // CSV 表头
        writeln!(
            writer,
            "保留文件,删除文件,删除原因,Hash,结构相似性,宽度,高度,文件大小"
        )?;

        // 查询回收站数据
        let mut stmt = repository.conn().prepare(
            "SELECT rb.original_path, rb.delete_reason, rb.blake3_hash, rb.ssim_score,
                    rb.width, rb.height, rb.file_size, rb.related_image_id
             FROM recycle_bin rb
             ORDER BY rb.recycled_at",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<f64>>(3)?,
                row.get::<_, u32>(4)?,
                row.get::<_, u32>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, Option<i64>>(7)?,
            ))
        })?;

        for row in rows {
            let (original_path, delete_reason, hash, ssim, width, height, file_size, related_id) =
                row?;

            // 获取保留文件路径
            let kept_file = if let Some(img_id) = related_id {
                repository
                    .conn()
                    .query_row(
                        "SELECT file_path FROM images WHERE id = ?1",
                        rusqlite::params![img_id],
                        |row| row.get::<_, String>(0),
                    )
                    .ok()
            } else {
                None
            };

            // 转换为相对路径
            let deleted_relative = Path::new(&original_path)
                .strip_prefix(root_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or(original_path.clone());

            let kept_relative = kept_file
                .and_then(|p| {
                    Path::new(&p)
                        .strip_prefix(root_path)
                        .ok()
                        .map(|p| p.to_string_lossy().to_string())
                })
                .unwrap_or_default();

            // 删除原因中文化
            let reason_text = match delete_reason.as_str() {
                "exact_duplicate" => "完全重复",
                "lower_resolution" => "低分辨率版本",
                _ => "未知",
            };

            writeln!(
                writer,
                "\"{}\",\"{}\",{},{},{},{},{},{}",
                kept_relative,
                deleted_relative,
                reason_text,
                hash.unwrap_or_default(),
                ssim.map(|s| format!("{:.4}", s)).unwrap_or_default(),
                width,
                height,
                file_size
            )?;
        }

        writer.flush()?;
        Ok(())
    }
}
