use tauri::State;
use std::sync::{Arc, Mutex};
use crate::error::Result;
use crate::db::repository::Repository;
use crate::db::models::Settings;

/// 加载用户设置
#[tauri::command]
pub async fn load_settings(
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<Settings> {
    let repo = repo.lock().unwrap();
    repo.load_settings()
}

/// 保存用户设置
#[tauri::command]
pub async fn save_settings(
    settings: Settings,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<()> {
    let repo = repo.lock().unwrap();
    repo.save_settings(&settings)
}
