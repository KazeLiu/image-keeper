use crate::db::models::{ActionStatus, FolderRole};
use crate::db::repository::Repository;
use crate::error::{AppError, Result};
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};

/// 回收站引擎 - Phase 7: 执行前复验与安全回收
pub struct RecycleEngine<'a> {
    repository: &'a Repository,
}

impl<'a> RecycleEngine<'a> {
    pub fn new(repository: &'a Repository) -> Self {
        Self { repository }
    }

    /// Phase 7.1: 执行前复验
    ///
    /// 验证文件和依赖的 baseline 匹配未变化
    pub fn verify_before_recycle(&self, result_id: i64, _run_id: &str) -> Result<()> {
        // 1. 获取分析结果
        let result = self
            .repository
            .get_analysis_result(result_id)?
            .ok_or_else(|| AppError::NotFound("分析结果不存在".to_string()))?;

        // 2. 获取对比图片
        let comparison_image = self
            .repository
            .get_image_by_id(result.comparison_image_id)?
            .ok_or_else(|| AppError::NotFound("对比图片不存在".to_string()))?;

        // 2.1 检查图片角色：禁止回收 baseline 图片
        if comparison_image.source_role == FolderRole::Baseline {
            return Err(AppError::ValidationError(
                "A 永远只读：不能回收 baseline 中的文件".to_string(),
            ));
        }

        // 3. 检查文件仍在原位置
        let file_path = PathBuf::from(&comparison_image.file_path);
        if !file_path.exists() {
            return Err(AppError::ValidationError(format!(
                "文件不存在: {:?}",
                file_path
            )));
        }

        // 4. 验证文件元数据未变化
        let metadata = fs::metadata(&file_path)
            .map_err(|e| AppError::FileSystem(format!("读取文件元数据失败: {}", e)))?;

        let current_size = metadata.len() as i64;
        let current_modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        if current_size != comparison_image.file_size {
            return Err(AppError::ValidationError(format!(
                "文件大小已变化: 期望 {}, 实际 {}",
                comparison_image.file_size, current_size
            )));
        }

        if current_modified != comparison_image.file_modified_at {
            return Err(AppError::ValidationError(format!("文件修改时间已变化")));
        }

        // 5. 重新计算 BLAKE3 验证
        let current_hash = self.compute_blake3(&file_path)?;
        if let Some(ref stored_hash) = comparison_image.blake3_hash {
            if &current_hash != stored_hash {
                return Err(AppError::ValidationError(format!(
                    "文件内容已变化: BLAKE3 不匹配"
                )));
            }
        }

        // 6. 如果有 baseline 匹配，验证匹配文件仍存在且未变化
        if let Some(primary_match_id) = result.primary_match_image_id {
            let match_image = self
                .repository
                .get_image_by_id(primary_match_id)?
                .ok_or_else(|| AppError::NotFound("baseline 匹配图片不存在".to_string()))?;

            let match_path = PathBuf::from(&match_image.file_path);
            if !match_path.exists() {
                return Err(AppError::ValidationError(format!(
                    "baseline 匹配文件已不存在: {:?}",
                    match_path
                )));
            }

            // 验证 baseline 文件 BLAKE3 未变化
            let match_hash = self.compute_blake3(&match_path)?;
            if let Some(ref stored_match_hash) = match_image.blake3_hash {
                if &match_hash != stored_match_hash {
                    return Err(AppError::ValidationError(format!(
                        "baseline 匹配文件内容已变化"
                    )));
                }
            }
        }

        Ok(())
    }

    /// Phase 7.2: 安全回收
    ///
    /// 移动文件到 .recycle/<runId>/... 并记录操作日志
    pub fn recycle_file(&self, result_id: i64, run_id: &str) -> Result<PathBuf> {
        // 1. 执行前复验
        self.verify_before_recycle(result_id, run_id)?;

        // 2. 获取文件信息
        let result = self
            .repository
            .get_analysis_result(result_id)?
            .ok_or_else(|| AppError::NotFound("分析结果不存在".to_string()))?;

        let comparison_image = self
            .repository
            .get_image_by_id(result.comparison_image_id)?
            .ok_or_else(|| AppError::NotFound("对比图片不存在".to_string()))?;

        let source_path = PathBuf::from(&comparison_image.file_path);

        // 3. 获取文件夹信息，找到对比目录根路径
        let folder = self
            .repository
            .get_folder(comparison_image.folder_id)?
            .ok_or_else(|| AppError::NotFound("文件夹不存在".to_string()))?;

        let comparison_root = PathBuf::from(&folder.path);

        // 4. 计算相对路径
        let relative_path = source_path
            .strip_prefix(&comparison_root)
            .map_err(|_| AppError::InvalidPath)?;

        // 5. 构建回收站目标路径
        let recycle_root = comparison_root.join(".recycle").join(run_id);
        let mut target_path = recycle_root.join(relative_path);

        // 6. 检查目标路径冲突，生成唯一名
        if target_path.exists() {
            target_path = self.generate_unique_path(&target_path)?;
        }

        // 7. 创建目标目录
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| AppError::FileSystem(format!("创建回收站目录失败: {}", e)))?;
        }

        // 8. 记录 prepared 状态
        self.repository
            .update_result_action_status(result_id, ActionStatus::Prepared)?;
        self.repository.create_action_log(
            result_id,
            "prepared",
            Some(&source_path.to_string_lossy()),
            Some(&target_path.to_string_lossy()),
            None,
        )?;

        // 9. 移动文件
        fs::rename(&source_path, &target_path).map_err(|e| {
            // 移动失败，记录错误
            let _ = self
                .repository
                .update_result_action_status(result_id, ActionStatus::Failed);
            let _ = self.repository.create_action_log(
                result_id,
                "failed",
                None,
                None,
                Some(&format!("移动文件失败: {}", e)),
            );
            AppError::FileSystem(format!("移动文件失败: {}", e))
        })?;

        // 10. 验证目标文件
        if !target_path.exists() {
            let _ = self
                .repository
                .update_result_action_status(result_id, ActionStatus::Failed);
            return Err(AppError::FileSystem("移动后目标文件不存在".to_string()));
        }

        // 11. 记录 recycled 状态
        self.repository
            .update_result_action_status(result_id, ActionStatus::Recycled)?;
        self.repository.create_action_log(
            result_id,
            "recycled",
            Some(&source_path.to_string_lossy()),
            Some(&target_path.to_string_lossy()),
            None,
        )?;

        Ok(target_path)
    }

    /// Phase 7.3: 恢复文件
    ///
    /// 从回收站恢复文件到原位置
    pub fn restore_file(&self, result_id: i64) -> Result<PathBuf> {
        // 1. 获取分析结果
        let result = self
            .repository
            .get_analysis_result(result_id)?
            .ok_or_else(|| AppError::NotFound("分析结果不存在".to_string()))?;

        // 2. 检查当前状态
        if result.action_status != ActionStatus::Recycled {
            return Err(AppError::ValidationError(format!(
                "只能恢复已回收的文件，当前状态: {:?}",
                result.action_status
            )));
        }

        // 3. 获取最后一次 recycled 操作日志
        let log = self
            .repository
            .get_last_action_log(result_id, "recycled")?
            .ok_or_else(|| AppError::NotFound("找不到回收操作日志".to_string()))?;

        let source_path = PathBuf::from(
            log.source_path
                .as_ref()
                .ok_or_else(|| AppError::Internal("日志缺少源路径".to_string()))?,
        );
        let recycled_path = PathBuf::from(
            log.target_path
                .as_ref()
                .ok_or_else(|| AppError::Internal("日志缺少目标路径".to_string()))?,
        );

        // 4. 检查回收文件存在
        if !recycled_path.exists() {
            return Err(AppError::NotFound(format!(
                "回收文件不存在: {:?}",
                recycled_path
            )));
        }

        // 5. 检查原路径冲突
        if source_path.exists() {
            return Err(AppError::ValidationError(format!(
                "原路径已存在文件，无法恢复: {:?}",
                source_path
            )));
        }

        // 6. 创建原目录
        if let Some(parent) = source_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| AppError::FileSystem(format!("创建原目录失败: {}", e)))?;
        }

        // 7. 移动文件回原位置
        fs::rename(&recycled_path, &source_path)
            .map_err(|e| AppError::FileSystem(format!("恢复文件失败: {}", e)))?;

        // 8. 更新状态
        self.repository
            .update_result_action_status(result_id, ActionStatus::Restored)?;
        self.repository.create_action_log(
            result_id,
            "restored",
            Some(&recycled_path.to_string_lossy()),
            Some(&source_path.to_string_lossy()),
            None,
        )?;

        Ok(source_path)
    }

    /// Phase 7.3: 永久删除
    ///
    /// 从回收站永久删除文件（需要独立确认）
    pub fn permanently_delete(&self, result_id: i64) -> Result<()> {
        // 1. 获取分析结果
        let result = self
            .repository
            .get_analysis_result(result_id)?
            .ok_or_else(|| AppError::NotFound("分析结果不存在".to_string()))?;

        // 2. 检查当前状态
        if result.action_status != ActionStatus::Recycled {
            return Err(AppError::ValidationError(format!(
                "只能永久删除已回收的文件，当前状态: {:?}",
                result.action_status
            )));
        }

        // 3. 获取最后一次 recycled 操作日志
        let log = self
            .repository
            .get_last_action_log(result_id, "recycled")?
            .ok_or_else(|| AppError::NotFound("找不到回收操作日志".to_string()))?;

        let recycled_path = PathBuf::from(
            log.target_path
                .as_ref()
                .ok_or_else(|| AppError::Internal("日志缺少目标路径".to_string()))?,
        );

        // 4. 检查回收文件存在
        if !recycled_path.exists() {
            return Err(AppError::NotFound(format!(
                "回收文件不存在: {:?}",
                recycled_path
            )));
        }

        // 5. 删除文件
        fs::remove_file(&recycled_path)
            .map_err(|e| AppError::FileSystem(format!("永久删除文件失败: {}", e)))?;

        // 6. 更新状态
        self.repository
            .update_result_action_status(result_id, ActionStatus::PermanentlyDeleted)?;
        self.repository.create_action_log(
            result_id,
            "permanently_deleted",
            Some(&recycled_path.to_string_lossy()),
            None,
            None,
        )?;

        Ok(())
    }

    /// 生成唯一文件路径（避免覆盖）
    fn generate_unique_path(&self, path: &Path) -> Result<PathBuf> {
        let parent = path.parent().ok_or_else(|| AppError::InvalidPath)?;

        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| AppError::InvalidPath)?;

        let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");

        let timestamp = Utc::now().format("%Y%m%d-%H%M%S").to_string();

        let new_name = if extension.is_empty() {
            format!("{}_{}", stem, timestamp)
        } else {
            format!("{}_{}.{}", stem, timestamp, extension)
        };

        Ok(parent.join(new_name))
    }

    /// 计算 BLAKE3 哈希
    fn compute_blake3(&self, path: &Path) -> Result<String> {
        use blake3::Hasher;
        use std::fs::File;
        use std::io::{BufReader, Read};

        let file =
            File::open(path).map_err(|e| AppError::FileSystem(format!("打开文件失败: {}", e)))?;

        let mut reader = BufReader::new(file);
        let mut hasher = Hasher::new();
        let mut buffer = [0; 8192];

        loop {
            let count = reader
                .read(&mut buffer)
                .map_err(|e| AppError::FileSystem(format!("读取文件失败: {}", e)))?;

            if count == 0 {
                break;
            }

            hasher.update(&buffer[..count]);
        }

        Ok(hasher.finalize().to_hex().to_string())
    }
}
