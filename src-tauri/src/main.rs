// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod error;
mod db;
mod core;
mod commands;

use std::sync::{Arc, Mutex};

fn main() {
    // 初始化数据库连接
    let conn = db::init_connection().expect("数据库初始化失败");
    let repository = db::repository::Repository::new(conn);

    tauri::Builder::default()
        .manage(Arc::new(Mutex::new(repository)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            commands::scan::start_scan,
            commands::scan::pause_scan,
            commands::scan::resume_scan,
            commands::scan::cancel_scan,
            commands::settings::load_settings,
            commands::settings::save_settings,
            commands::directory::load_directory_tree,
            commands::comparison::start_multi_compare,
            commands::comparison::get_comparison_stats,
        ])
        .run(tauri::generate_context!())
        .expect("启动 Tauri 应用失败");
}
