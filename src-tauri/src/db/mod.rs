pub mod models;
pub mod repository;
pub mod schema;

use crate::error::Result;
use rusqlite::Connection;
use std::path::PathBuf;

/// 获取数据库路径
pub fn get_database_path() -> Result<PathBuf> {
    let app_data_dir = dirs::data_local_dir()
        .ok_or_else(|| crate::error::AppError::Other("无法获取应用数据目录".to_string()))?;

    let db_dir = app_data_dir.join("ImageKeeper");
    std::fs::create_dir_all(&db_dir)?;

    Ok(db_dir.join("imagekeeper.db"))
}

/// 初始化数据库连接
pub fn init_connection() -> Result<Connection> {
    let db_path = get_database_path()?;
    let conn = Connection::open(db_path)?;

    // 启用外键约束
    conn.execute("PRAGMA foreign_keys = ON", [])?;

    // 初始化数据库表结构
    schema::initialize_database(&conn)?;

    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_connection() {
        let conn = Connection::open_in_memory().unwrap();
        schema::initialize_database(&conn).unwrap();
    }
}
