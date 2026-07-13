use rusqlite::{Connection, params};
use chrono::Utc;
use crate::error::{Result};
use super::models::*;

/// 数据库访问层
pub struct Repository {
    conn: Connection,
}

impl Repository {
    /// 创建新的 Repository 实例
    pub fn new(conn: Connection) -> Self {
        Self { conn }
    }

    /// 获取数据库连接的引用
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// 创建扫描任务
    pub fn create_scan(&self, root_path: &str) -> Result<Scan> {
        let now = Utc::now().timestamp();

        self.conn.execute(
            "INSERT INTO scans (root_path, status, compare_mode, created_at) VALUES (?1, ?2, 'within', ?3)",
            params![root_path, ScanStatus::Pending.as_str(), now],
        )?;

        let id = self.conn.last_insert_rowid();

        Ok(Scan {
            id,
            root_path: root_path.to_string(),
            status: ScanStatus::Pending,
            compare_mode: CompareMode::Within,
            total_files: 0,
            scanned_files: 0,
            hash_computed: 0,
            phash_computed: 0,
            last_scanned_path: None,
            created_at: now,
            started_at: None,
            completed_at: None,
        })
    }

    /// 更新扫描状态
    pub fn update_scan_status(&self, scan_id: i64, status: ScanStatus) -> Result<()> {
        let status_str = status.as_str();
        let now = Utc::now().timestamp();

        let mut sql = "UPDATE scans SET status = ?1".to_string();

        if status == ScanStatus::Running {
            sql.push_str(", started_at = ?2 WHERE id = ?3");
            self.conn.execute(&sql, params![status_str, now, scan_id])?;
        } else if status == ScanStatus::Completed {
            sql.push_str(", completed_at = ?2 WHERE id = ?3");
            self.conn.execute(&sql, params![status_str, now, scan_id])?;
        } else {
            sql.push_str(" WHERE id = ?2");
            self.conn.execute(&sql, params![status_str, scan_id])?;
        }

        Ok(())
    }

    /// 更新扫描进度
    pub fn update_scan_progress(
        &self,
        scan_id: i64,
        scanned_files: u64,
        last_scanned_path: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE scans SET scanned_files = ?1, last_scanned_path = ?2 WHERE id = ?3",
            params![scanned_files, last_scanned_path, scan_id],
        )?;
        Ok(())
    }

    /// 获取扫描任务
    pub fn get_scan(&self, scan_id: i64) -> Result<Scan> {
        let scan = self.conn.query_row(
            "SELECT id, root_path, status, compare_mode, total_files, scanned_files, hash_computed,
                    phash_computed, last_scanned_path, created_at, started_at, completed_at
             FROM scans WHERE id = ?1",
            params![scan_id],
            |row| {
                Ok(Scan {
                    id: row.get(0)?,
                    root_path: row.get(1)?,
                    status: ScanStatus::from_str(&row.get::<_, String>(2)?)
                        .unwrap_or(ScanStatus::Pending),
                    compare_mode: CompareMode::from_str(&row.get::<_, String>(3)?)
                        .unwrap_or(CompareMode::Within),
                    total_files: row.get(4)?,
                    scanned_files: row.get(5)?,
                    hash_computed: row.get(6)?,
                    phash_computed: row.get(7)?,
                    last_scanned_path: row.get(8)?,
                    created_at: row.get(9)?,
                    started_at: row.get(10)?,
                    completed_at: row.get(11)?,
                })
            },
        )?;

        Ok(scan)
    }

    /// 插入图片记录
    pub fn insert_image(&self, image: &Image) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO images (file_path, relative_path, file_size, file_modified_at,
                                width, height, format, aspect_ratio, blake3_hash, phash,
                                hash_computed_at, scan_id, folder_id, scanned_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                image.file_path,
                image.relative_path,
                image.file_size,
                image.file_modified_at,
                image.width,
                image.height,
                image.format,
                image.aspect_ratio,
                image.blake3_hash,
                image.phash,
                image.hash_computed_at,
                image.scan_id,
                image.folder_id,
                image.scanned_at,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// 更新图片哈希
    pub fn update_image_hash(&self, image_id: i64, hash: &str) -> Result<()> {
        let now = Utc::now().timestamp();
        self.conn.execute(
            "UPDATE images SET blake3_hash = ?1, hash_computed_at = ?2 WHERE id = ?3",
            params![hash, now, image_id],
        )?;
        Ok(())
    }

    /// 根据哈希查找图片
    pub fn find_images_by_hash(&self, hash: &str) -> Result<Vec<Image>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, file_path, relative_path, file_size, file_modified_at,
                    width, height, format, aspect_ratio, blake3_hash, phash,
                    hash_computed_at, scan_id, folder_id, scanned_at
             FROM images WHERE blake3_hash = ?1",
        )?;

        let images = stmt.query_map(params![hash], |row| {
            Ok(Image {
                id: row.get(0)?,
                file_path: row.get(1)?,
                relative_path: row.get(2)?,
                file_size: row.get(3)?,
                file_modified_at: row.get(4)?,
                width: row.get(5)?,
                height: row.get(6)?,
                format: row.get(7)?,
                aspect_ratio: row.get(8)?,
                blake3_hash: row.get(9)?,
                phash: row.get(10)?,
                hash_computed_at: row.get(11)?,
                scan_id: row.get(12)?,
                folder_id: row.get(13)?,
                scanned_at: row.get(14)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(images)
    }

    /// 查找比指定尺寸更大的图片
    pub fn find_larger_images(
        &self,
        width: u32,
        height: u32,
        aspect_ratio_min: f64,
        aspect_ratio_max: f64,
    ) -> Result<Vec<Image>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, file_path, relative_path, file_size, file_modified_at,
                    width, height, format, aspect_ratio, blake3_hash, phash,
                    hash_computed_at, scan_id, folder_id, scanned_at
             FROM images
             WHERE width > ?1 AND height > ?2
               AND aspect_ratio BETWEEN ?3 AND ?4
             ORDER BY width ASC, height ASC",
        )?;

        let images = stmt.query_map(
            params![width, height, aspect_ratio_min, aspect_ratio_max],
            |row| {
                Ok(Image {
                    id: row.get(0)?,
                    file_path: row.get(1)?,
                    relative_path: row.get(2)?,
                    file_size: row.get(3)?,
                    file_modified_at: row.get(4)?,
                    width: row.get(5)?,
                    height: row.get(6)?,
                    format: row.get(7)?,
                    aspect_ratio: row.get(8)?,
                    blake3_hash: row.get(9)?,
                    phash: row.get(10)?,
                    hash_computed_at: row.get(11)?,
                    scan_id: row.get(12)?,
                    folder_id: row.get(13)?,
                    scanned_at: row.get(14)?,
                })
            },
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(images)
    }

    /// 加载设置
    pub fn load_settings(&self) -> Result<Settings> {
        let ssim_threshold: String = self.conn.query_row(
            "SELECT value FROM settings WHERE key = 'ssim_threshold'",
            [],
            |row| row.get(0),
        )?;

        let duplicate_keep_strategy: String = self.conn.query_row(
            "SELECT value FROM settings WHERE key = 'duplicate_keep_strategy'",
            [],
            |row| row.get(0),
        )?;

        let preferred_directory: String = self.conn.query_row(
            "SELECT value FROM settings WHERE key = 'preferred_directory'",
            [],
            |row| row.get(0),
        )?;

        let auto_recycle_duplicates: String = self.conn.query_row(
            "SELECT value FROM settings WHERE key = 'auto_recycle_duplicates'",
            [],
            |row| row.get(0),
        )?;

        let auto_recycle_compressed: String = self.conn.query_row(
            "SELECT value FROM settings WHERE key = 'auto_recycle_compressed'",
            [],
            |row| row.get(0),
        )?;

        Ok(Settings {
            ssim_threshold: ssim_threshold.parse().unwrap_or(0.995),
            duplicate_keep_strategy,
            preferred_directory,
            auto_recycle_duplicates: auto_recycle_duplicates == "1",
            auto_recycle_compressed: auto_recycle_compressed == "1",
        })
    }

    /// 保存设置
    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        let now = Utc::now().timestamp();

        self.conn.execute(
            "UPDATE settings SET value = ?1, updated_at = ?2 WHERE key = 'ssim_threshold'",
            params![settings.ssim_threshold.to_string(), now],
        )?;

        self.conn.execute(
            "UPDATE settings SET value = ?1, updated_at = ?2 WHERE key = 'duplicate_keep_strategy'",
            params![settings.duplicate_keep_strategy, now],
        )?;

        self.conn.execute(
            "UPDATE settings SET value = ?1, updated_at = ?2 WHERE key = 'preferred_directory'",
            params![settings.preferred_directory, now],
        )?;

        self.conn.execute(
            "UPDATE settings SET value = ?1, updated_at = ?2 WHERE key = 'auto_recycle_duplicates'",
            params![if settings.auto_recycle_duplicates { "1" } else { "0" }, now],
        )?;

        self.conn.execute(
            "UPDATE settings SET value = ?1, updated_at = ?2 WHERE key = 'auto_recycle_compressed'",
            params![if settings.auto_recycle_compressed { "1" } else { "0" }, now],
        )?;

        Ok(())
    }
}
