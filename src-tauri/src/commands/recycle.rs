use crate::core::recycle::RecycleEngine;
use crate::db::repository::Repository;
use crate::error::{AppError, Result};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::State;

/// 回收文件请求
#[derive(Debug, serde::Deserialize)]
pub struct RecycleRequest {
    pub result_id: i64,
    pub run_id: String,
}

/// 恢复文件请求
#[derive(Debug, serde::Deserialize)]
pub struct RestoreRequest {
    pub result_id: i64,
}

/// 永久删除请求
#[derive(Debug, serde::Deserialize)]
pub struct PermanentDeleteRequest {
    pub result_id: i64,
}

/// 按图片回收的单项结果
#[derive(Debug, serde::Serialize)]
pub struct ImageRecycleOutcome {
    pub image_id: i64,
    pub result_id: Option<i64>,
    pub success: bool,
    pub error_message: Option<String>,
}

fn read_recyclable_result_ids_by_image_ids(
    repo: &Repository,
    run_id: &str,
    image_ids: &[i64],
) -> Result<HashMap<i64, i64>> {
    if image_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let placeholders = std::iter::repeat("?")
        .take(image_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        r#"SELECT comparison_image_id, id
           FROM analysis_results
           WHERE run_id = ?
             AND comparison_image_id IN ({placeholders})
             AND COALESCE((
                 SELECT ol.operation_type
                 FROM operation_logs ol
                 WHERE ol.analysis_result_id = analysis_results.id
                 ORDER BY ol.created_at DESC, ol.id DESC
                 LIMIT 1
             ), 'none') NOT IN ('recycled', 'permanently_deleted')
           ORDER BY id"#
    );

    let mut params: Vec<rusqlite::types::Value> = Vec::with_capacity(image_ids.len() + 1);
    params.push(run_id.to_string().into());
    params.extend(image_ids.iter().copied().map(Into::into));

    let mut stmt = repo.conn().prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
    })?;

    let mut result_ids = HashMap::new();
    for row in rows {
        let (image_id, result_id) = row?;
        result_ids.entry(image_id).or_insert(result_id);
    }

    Ok(result_ids)
}

/// 回收文件到回收站
#[tauri::command]
pub async fn recycle_file(
    request: RecycleRequest,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<String> {
    let repo_lock = repo.lock().unwrap();
    let engine = RecycleEngine::new(&repo_lock);

    let target_path = engine.recycle_file(request.result_id, &request.run_id)?;

    Ok(target_path.to_string_lossy().to_string())
}

/// 从回收站恢复文件
#[tauri::command]
pub async fn restore_file(
    request: RestoreRequest,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<String> {
    let repo_lock = repo.lock().unwrap();
    let engine = RecycleEngine::new(&repo_lock);

    let restored_path = engine.restore_file(request.result_id)?;

    Ok(restored_path.to_string_lossy().to_string())
}

/// 永久删除回收站中的文件
#[tauri::command]
pub async fn permanently_delete_file(
    request: PermanentDeleteRequest,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<()> {
    let repo_lock = repo.lock().unwrap();
    let engine = RecycleEngine::new(&repo_lock);

    engine.permanently_delete(request.result_id)?;

    Ok(())
}

/// 批量回收文件
#[tauri::command]
pub async fn batch_recycle_files(
    result_ids: Vec<i64>,
    run_id: String,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<Vec<(i64, bool, Option<String>)>> {
    let repo_lock = repo.lock().unwrap();
    let engine = RecycleEngine::new(&repo_lock);

    let mut results = Vec::new();

    for result_id in result_ids {
        match engine.recycle_file(result_id, &run_id) {
            Ok(_) => {
                results.push((result_id, true, None));
            }
            Err(e) => {
                results.push((result_id, false, Some(format!("{}", e))));
            }
        }
    }

    Ok(results)
}

/// 按图片 ID 批量回收文件
#[tauri::command]
pub async fn batch_recycle_images(
    run_id: String,
    image_ids: Vec<i64>,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<Vec<ImageRecycleOutcome>> {
    if image_ids.is_empty() {
        return Err(AppError::ValidationError("请选择要删除的图片".to_string()));
    }

    let repo_lock = repo.lock().unwrap();
    let result_ids_by_image_id =
        read_recyclable_result_ids_by_image_ids(&repo_lock, &run_id, &image_ids)?;
    let engine = RecycleEngine::new(&repo_lock);

    let mut outcomes = Vec::with_capacity(image_ids.len());
    for image_id in image_ids {
        let Some(result_id) = result_ids_by_image_id.get(&image_id).copied() else {
            outcomes.push(ImageRecycleOutcome {
                image_id,
                result_id: None,
                success: false,
                error_message: Some("没有找到这张图片对应的可回收分析结果".to_string()),
            });
            continue;
        };

        match engine.recycle_file(result_id, &run_id) {
            Ok(_) => outcomes.push(ImageRecycleOutcome {
                image_id,
                result_id: Some(result_id),
                success: true,
                error_message: None,
            }),
            Err(error) => outcomes.push(ImageRecycleOutcome {
                image_id,
                result_id: Some(result_id),
                success: false,
                error_message: Some(error.to_string()),
            }),
        }
    }

    Ok(outcomes)
}

/// 批量恢复文件
#[tauri::command]
pub async fn batch_restore_files(
    result_ids: Vec<i64>,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<Vec<(i64, bool, Option<String>)>> {
    let repo_lock = repo.lock().unwrap();
    let engine = RecycleEngine::new(&repo_lock);

    let mut results = Vec::new();

    for result_id in result_ids {
        match engine.restore_file(result_id) {
            Ok(_) => {
                results.push((result_id, true, None));
            }
            Err(e) => {
                results.push((result_id, false, Some(format!("{}", e))));
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{AnalysisType, FolderRole};
    use crate::db::repository::{AnalysisResultInsert, ImageInsert};
    use crate::db::{repository::RunConfig, schema};
    use rusqlite::Connection;

    fn create_test_repo() -> Repository {
        let conn = Connection::open_in_memory().unwrap();
        schema::initialize_database(&conn).unwrap();
        let repo = Repository::new(conn);
        repo.create_run(
            "run-1",
            "0.1.0",
            "test-profile",
            "D:/baseline",
            "A",
            &["D:/comparison".to_string()],
            &["B".to_string()],
            &RunConfig::default(),
        )
        .unwrap();
        repo
    }

    #[test]
    fn read_recyclable_result_ids_by_image_ids_returns_only_active_results() {
        let repo = create_test_repo();
        let folder_id = repo
            .create_folder("run-1", "D:/comparison", "B", FolderRole::Comparison)
            .unwrap();
        let active_image_id = repo
            .insert_image(&ImageInsert {
                run_id: "run-1".to_string(),
                folder_id,
                source_role: FolderRole::Comparison,
                file_path: "D:/comparison/active.png".to_string(),
                relative_path: "active.png".to_string(),
                file_size: 100,
                file_modified_at: 0,
                width: 10,
                height: 10,
                format: "png".to_string(),
                aspect_ratio: 1.0,
                frame_count: 1,
                frame_strategy: "first".to_string(),
            })
            .unwrap();
        let recycled_image_id = repo
            .insert_image(&ImageInsert {
                run_id: "run-1".to_string(),
                folder_id,
                source_role: FolderRole::Comparison,
                file_path: "D:/comparison/recycled.png".to_string(),
                relative_path: "recycled.png".to_string(),
                file_size: 100,
                file_modified_at: 0,
                width: 10,
                height: 10,
                format: "png".to_string(),
                aspect_ratio: 1.0,
                frame_count: 1,
                frame_strategy: "first".to_string(),
            })
            .unwrap();

        let active_result_id = repo
            .insert_analysis_result(&AnalysisResultInsert {
                run_id: "run-1".to_string(),
                comparison_image_id: active_image_id,
                analysis_type: AnalysisType::LikelyCompressed,
                primary_match_image_id: None,
                all_candidate_ids: None,
                candidate_truncated: false,
                phash_distance: Some(1),
                ssim_score: Some(0.999),
                size_ratio: Some(0.5),
                resolution_ratio: Some(0.5),
                aspect_diff: Some(0.0),
                direction_smaller_resolution: true,
                direction_smaller_filesize: true,
                algorithm_profile_id: "test-profile".to_string(),
                analysis_metadata: None,
            })
            .unwrap();
        let recycled_result_id = repo
            .insert_analysis_result(&AnalysisResultInsert {
                run_id: "run-1".to_string(),
                comparison_image_id: recycled_image_id,
                analysis_type: AnalysisType::LikelyCompressed,
                primary_match_image_id: None,
                all_candidate_ids: None,
                candidate_truncated: false,
                phash_distance: Some(1),
                ssim_score: Some(0.999),
                size_ratio: Some(0.5),
                resolution_ratio: Some(0.5),
                aspect_diff: Some(0.0),
                direction_smaller_resolution: true,
                direction_smaller_filesize: true,
                algorithm_profile_id: "test-profile".to_string(),
                analysis_metadata: None,
            })
            .unwrap();
        repo.create_action_log(
            recycled_result_id,
            "recycled",
            Some("D:/comparison/recycled.png"),
            Some("D:/comparison/.recycle/run-1/recycled.png"),
            None,
        )
        .unwrap();

        let result_ids = read_recyclable_result_ids_by_image_ids(
            &repo,
            "run-1",
            &[active_image_id, recycled_image_id],
        )
        .unwrap();

        assert_eq!(result_ids.get(&active_image_id), Some(&active_result_id));
        assert!(!result_ids.contains_key(&recycled_image_id));
    }
}

/// 批量永久删除文件
#[tauri::command]
pub async fn batch_permanently_delete_files(
    result_ids: Vec<i64>,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<Vec<(i64, bool, Option<String>)>> {
    let repo_lock = repo.lock().unwrap();
    let engine = RecycleEngine::new(&repo_lock);

    let mut results = Vec::new();

    for result_id in result_ids {
        match engine.permanently_delete(result_id) {
            Ok(_) => {
                results.push((result_id, true, None));
            }
            Err(e) => {
                results.push((result_id, false, Some(format!("{}", e))));
            }
        }
    }

    Ok(results)
}
