// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod core;
mod db;
mod error;

use std::sync::{Arc, Mutex};
use tauri::Manager;

const MAIN_WINDOW_LABEL: &str = "main";

fn should_exit_after_close_request(window_label: &str) -> bool {
    window_label == MAIN_WINDOW_LABEL
}

fn main() {
    // 初始化数据库连接
    let conn = db::init_connection().expect("数据库初始化失败");
    let repository = db::repository::Repository::new(conn);

    tauri::Builder::default()
        .manage(Arc::new(Mutex::new(repository)))
        .manage(Arc::new(
            commands::comparison::GroupSimilarityBackfillState::default(),
        ))
        .manage(Arc::new(
            commands::difference_finder::DifferenceFinderState::default(),
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. })
                && should_exit_after_close_request(window.label())
            {
                window.app_handle().exit(0);
            }
        })
        .invoke_handler(tauri::generate_handler![
            // TODO: 重新实现命令
            // commands::scan::start_scan,
            // commands::scan::pause_scan,
            // commands::scan::resume_scan,
            // commands::scan::cancel_scan,
            commands::settings::load_settings,
            commands::settings::save_settings,
            commands::directory::load_directory_tree,
            commands::directory::open_folder,
            commands::difference_finder::start_difference_search,
            commands::difference_finder::cancel_difference_search,
            commands::difference_finder::preview_difference_rename,
            commands::difference_finder::preview_difference_explicit_rename,
            commands::difference_finder::preview_difference_transfer,
            commands::difference_finder::execute_difference_rename,
            commands::difference_finder::move_difference_files,
            commands::difference_finder::copy_difference_files,
            commands::difference_finder::undo_difference_batch,
            commands::image_metrics::load_test_image,
            commands::image_metrics::compute_test_phash,
            commands::image_metrics::compute_test_ssim,
            commands::image_metrics::compute_test_difference_preview,
            commands::comparison::start_multi_compare,
            commands::comparison::get_comparison_stats,
            commands::comparison::get_comparison_results,
            commands::comparison::get_comparison_groups,
            commands::comparison::get_group_similarity_scores,
            commands::comparison::cancel_group_similarity_request,
            commands::comparison::get_group_similarity_statuses,
            commands::comparison::start_group_similarity_backfill,
            commands::comparison::get_run_status,
            commands::comparison::list_comparison_runs,
            commands::comparison::delete_comparison_run,
            commands::report::generate_reports,
            commands::report::generate_json_report,
            commands::report::generate_csv_report,
            commands::report::generate_html_report,
            commands::recycle::recycle_file,
            commands::recycle::restore_file,
            commands::recycle::permanently_delete_file,
            commands::recycle::batch_recycle_files,
            commands::recycle::batch_recycle_images,
            commands::recycle::batch_restore_files,
            commands::recycle::batch_permanently_delete_files,
        ])
        .run(tauri::generate_context!())
        .expect("启动 Tauri 应用失败");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_main_window_close_exits_the_entire_application() {
        assert!(should_exit_after_close_request("main"));
        assert!(!should_exit_after_close_request("difference-finder"));
        assert!(!should_exit_after_close_request("image-metrics-test"));
    }
}
