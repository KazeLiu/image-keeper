use crate::core::report::{GeneratedReports, ReportGenerator};
use crate::db::init_connection;
use crate::db::repository::Repository;
use std::path::PathBuf;

/// 生成完整报告（JSON + CSV + HTML）
#[tauri::command]
pub async fn generate_reports(
    run_id: String,
    output_dir: Option<String>,
) -> Result<GeneratedReports, String> {
    let conn = init_connection().map_err(|e| e.to_string())?;
    let repo = Repository::new(conn);

    // 确定输出目录
    let output_dir = if let Some(dir) = output_dir {
        PathBuf::from(dir)
    } else {
        // 默认使用用户数据目录的 reports 子目录
        let app_data_dir = dirs::data_dir()
            .ok_or_else(|| "无法获取应用数据目录".to_string())?
            .join("ImageKeeper")
            .join("reports");
        std::fs::create_dir_all(&app_data_dir).map_err(|e| e.to_string())?;
        app_data_dir
    };

    let generator = ReportGenerator::new(&repo, run_id, output_dir);
    generator
        .generate_all()
        .map_err(|e| format!("报告生成失败: {}", e))
}

/// 仅生成 JSON 报告
#[tauri::command]
pub async fn generate_json_report(
    run_id: String,
    output_dir: Option<String>,
) -> Result<String, String> {
    let conn = init_connection().map_err(|e| e.to_string())?;
    let repo = Repository::new(conn);

    let output_dir = if let Some(dir) = output_dir {
        PathBuf::from(dir)
    } else {
        let app_data_dir = dirs::data_dir()
            .ok_or_else(|| "无法获取应用数据目录".to_string())?
            .join("ImageKeeper")
            .join("reports");
        std::fs::create_dir_all(&app_data_dir).map_err(|e| e.to_string())?;
        app_data_dir
    };

    let generator = ReportGenerator::new(&repo, run_id, output_dir);
    let path = generator
        .generate_json_report()
        .map_err(|e| format!("JSON 报告生成失败: {}", e))?;

    Ok(path.to_string_lossy().to_string())
}

/// 仅生成 CSV 报告
#[tauri::command]
pub async fn generate_csv_report(
    run_id: String,
    output_dir: Option<String>,
) -> Result<String, String> {
    let conn = init_connection().map_err(|e| e.to_string())?;
    let repo = Repository::new(conn);

    let output_dir = if let Some(dir) = output_dir {
        PathBuf::from(dir)
    } else {
        let app_data_dir = dirs::data_dir()
            .ok_or_else(|| "无法获取应用数据目录".to_string())?
            .join("ImageKeeper")
            .join("reports");
        std::fs::create_dir_all(&app_data_dir).map_err(|e| e.to_string())?;
        app_data_dir
    };

    let generator = ReportGenerator::new(&repo, run_id, output_dir);
    let path = generator
        .generate_csv_report()
        .map_err(|e| format!("CSV 报告生成失败: {}", e))?;

    Ok(path.to_string_lossy().to_string())
}

/// 仅生成 HTML 报告
#[tauri::command]
pub async fn generate_html_report(
    run_id: String,
    output_dir: Option<String>,
) -> Result<String, String> {
    let conn = init_connection().map_err(|e| e.to_string())?;
    let repo = Repository::new(conn);

    let output_dir = if let Some(dir) = output_dir {
        PathBuf::from(dir)
    } else {
        let app_data_dir = dirs::data_dir()
            .ok_or_else(|| "无法获取应用数据目录".to_string())?
            .join("ImageKeeper")
            .join("reports");
        std::fs::create_dir_all(&app_data_dir).map_err(|e| e.to_string())?;
        app_data_dir
    };

    let generator = ReportGenerator::new(&repo, run_id, output_dir);
    let path = generator
        .generate_html_report()
        .map_err(|e| format!("HTML 报告生成失败: {}", e))?;

    Ok(path.to_string_lossy().to_string())
}
