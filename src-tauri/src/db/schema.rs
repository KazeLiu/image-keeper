use rusqlite::Connection;
use crate::error::Result;

/// 数据库初始化 SQL
const SCHEMA_SQL: &str = r#"
-- 图片元数据表
CREATE TABLE IF NOT EXISTS images (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT NOT NULL UNIQUE,
    relative_path TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    file_modified_at INTEGER NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    format TEXT NOT NULL,
    aspect_ratio REAL NOT NULL,
    blake3_hash TEXT,
    phash TEXT,
    hash_computed_at INTEGER,
    scan_id INTEGER NOT NULL,
    folder_id INTEGER,
    scanned_at INTEGER NOT NULL,
    FOREIGN KEY (scan_id) REFERENCES scans(id) ON DELETE CASCADE,
    FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_images_hash ON images(blake3_hash) WHERE blake3_hash IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_images_phash ON images(phash) WHERE phash IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_images_size ON images(width, height);
CREATE INDEX IF NOT EXISTS idx_images_aspect_ratio ON images(aspect_ratio);
CREATE INDEX IF NOT EXISTS idx_images_scan_id ON images(scan_id);
CREATE INDEX IF NOT EXISTS idx_images_folder_id ON images(folder_id);
CREATE INDEX IF NOT EXISTS idx_images_file_modified ON images(file_modified_at);

-- 扫描任务表
CREATE TABLE IF NOT EXISTS scans (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    root_path TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'paused', 'completed', 'cancelled')),
    compare_mode TEXT NOT NULL DEFAULT 'within' CHECK (compare_mode IN ('within', 'between')),
    total_files INTEGER DEFAULT 0,
    scanned_files INTEGER DEFAULT 0,
    hash_computed INTEGER DEFAULT 0,
    phash_computed INTEGER DEFAULT 0,
    last_scanned_path TEXT,
    created_at INTEGER NOT NULL,
    started_at INTEGER,
    completed_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_scans_status ON scans(status);
CREATE INDEX IF NOT EXISTS idx_scans_root_path ON scans(root_path);

-- 文件夹表
CREATE TABLE IF NOT EXISTS folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scan_id INTEGER NOT NULL,
    path TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('baseline', 'compare')),
    file_count INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (scan_id) REFERENCES scans(id) ON DELETE CASCADE,
    UNIQUE(scan_id, path)
);

CREATE INDEX IF NOT EXISTS idx_folders_scan_id ON folders(scan_id);
CREATE INDEX IF NOT EXISTS idx_folders_role ON folders(role);

-- 完全重复文件表
CREATE TABLE IF NOT EXISTS duplicates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hash_group TEXT NOT NULL,
    original_image_id INTEGER NOT NULL,
    duplicate_image_id INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'recycled', 'deleted', 'kept')),
    marked_at INTEGER NOT NULL,
    FOREIGN KEY (original_image_id) REFERENCES images(id) ON DELETE CASCADE,
    FOREIGN KEY (duplicate_image_id) REFERENCES images(id) ON DELETE CASCADE,
    UNIQUE(original_image_id, duplicate_image_id)
);

CREATE INDEX IF NOT EXISTS idx_duplicates_hash_group ON duplicates(hash_group);
CREATE INDEX IF NOT EXISTS idx_duplicates_status ON duplicates(status);

-- 相似图片配对表
CREATE TABLE IF NOT EXISTS similar_pairs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    larger_image_id INTEGER NOT NULL,
    smaller_image_id INTEGER NOT NULL,
    phash_distance INTEGER,
    ssim_score REAL,
    size_ratio REAL NOT NULL,
    resolution_ratio REAL NOT NULL,
    similarity_type TEXT CHECK (similarity_type IN ('compressed', 'diff', 'similar')),
    is_compressed_version INTEGER NOT NULL DEFAULT 0,
    ssim_threshold REAL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'recycled', 'deleted', 'kept', 'skipped')),
    marked_at INTEGER NOT NULL,
    computed_at INTEGER,
    FOREIGN KEY (larger_image_id) REFERENCES images(id) ON DELETE CASCADE,
    FOREIGN KEY (smaller_image_id) REFERENCES images(id) ON DELETE CASCADE,
    UNIQUE(larger_image_id, smaller_image_id)
);

CREATE INDEX IF NOT EXISTS idx_similar_pairs_larger ON similar_pairs(larger_image_id);
CREATE INDEX IF NOT EXISTS idx_similar_pairs_smaller ON similar_pairs(smaller_image_id);
CREATE INDEX IF NOT EXISTS idx_similar_pairs_status ON similar_pairs(status);
CREATE INDEX IF NOT EXISTS idx_similar_pairs_is_compressed ON similar_pairs(is_compressed_version);
CREATE INDEX IF NOT EXISTS idx_similar_pairs_similarity_type ON similar_pairs(similarity_type);

-- 回收站表
CREATE TABLE IF NOT EXISTS recycle_bin (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    original_path TEXT NOT NULL,
    recycled_path TEXT NOT NULL,
    delete_reason TEXT NOT NULL CHECK (delete_reason IN ('exact_duplicate', 'lower_resolution')),
    related_image_id INTEGER,
    duplicate_id INTEGER,
    similar_pair_id INTEGER,
    file_size INTEGER NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    blake3_hash TEXT,
    ssim_score REAL,
    recycled_at INTEGER NOT NULL,
    FOREIGN KEY (related_image_id) REFERENCES images(id) ON DELETE SET NULL,
    FOREIGN KEY (duplicate_id) REFERENCES duplicates(id) ON DELETE SET NULL,
    FOREIGN KEY (similar_pair_id) REFERENCES similar_pairs(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_recycle_bin_reason ON recycle_bin(delete_reason);
CREATE INDEX IF NOT EXISTS idx_recycle_bin_recycled_at ON recycle_bin(recycled_at);

-- 设置表
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

-- 插入默认设置
INSERT OR IGNORE INTO settings (key, value, updated_at) VALUES
    ('ssim_threshold', '0.995', strftime('%s', 'now')),
    ('duplicate_keep_strategy', 'shortest_path', strftime('%s', 'now')),
    ('preferred_directory', '', strftime('%s', 'now')),
    ('auto_recycle_duplicates', '1', strftime('%s', 'now')),
    ('auto_recycle_compressed', '1', strftime('%s', 'now'));
"#;

/// 初始化数据库
pub fn initialize_database(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA_SQL)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize_database() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_database(&conn).unwrap();

        // 验证表是否创建成功
        let table_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert!(table_count >= 6);
    }
}
