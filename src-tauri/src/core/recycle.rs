use crate::db::models::{ActionStatus, FolderRole};
use crate::db::repository::Repository;
use crate::error::{AppError, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// 将确认后的文件交给系统回收站实现，并返回原路径作为日志引用。
fn dispatch_system_recycle<F>(source_path: &Path, recycle: F) -> Result<PathBuf>
where
    F: FnOnce(&Path) -> Result<()>,
{
    recycle(source_path)?;
    Ok(source_path.to_path_buf())
}

#[cfg(target_os = "windows")]
/// 在专用 STA 线程中将文件移入 Windows 系统回收站。
fn send_file_to_system_recycle_bin(source_path: &Path) -> Result<()> {
    let source_path = source_path.to_path_buf();
    let result = std::thread::spawn(move || recycle_on_sta_thread(&source_path))
        .join()
        .map_err(|_| AppError::FileSystem("Windows 回收站线程异常退出".to_string()))?;

    result.map_err(|error| AppError::FileSystem(format!("移入 Windows 回收站失败: {error}")))
}

#[cfg(target_os = "windows")]
/// 返回要求系统回收而不是静默永久删除的 Shell 操作 flags。
fn system_recycle_operation_flags() -> windows::Win32::UI::Shell::FILEOPERATION_FLAGS {
    use windows::Win32::UI::Shell::{
        FOFX_RECYCLEONDELETE, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT,
    };

    FOFX_RECYCLEONDELETE | FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT
}

#[cfg(target_os = "windows")]
/// 在独立 STA 线程中执行 Windows Shell 回收站操作。
fn recycle_on_sta_thread(source_path: &Path) -> std::result::Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        FileOperation, IFileOperation, IShellItem, SHCreateItemFromParsingName,
    };

    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
        .ok()
        .map_err(|error| format!("初始化 COM 失败: {error}"))?;
    let _com_apartment = ComApartment;

    let wide_path = source_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let operation: IFileOperation =
        unsafe { CoCreateInstance(&FileOperation, None, CLSCTX_INPROC_SERVER) }
            .map_err(|error| format!("创建 Windows 文件操作失败: {error}"))?;
    unsafe { operation.SetOperationFlags(system_recycle_operation_flags()) }
        .map_err(|error| format!("设置 Windows 回收站操作标志失败: {error}"))?;
    let item: IShellItem = unsafe { SHCreateItemFromParsingName(PCWSTR(wide_path.as_ptr()), None) }
        .map_err(|error| format!("读取待回收文件失败: {error}"))?;
    unsafe { operation.DeleteItem(&item, None) }
        .map_err(|error| format!("加入 Windows 回收队列失败: {error}"))?;
    unsafe { operation.PerformOperations() }
        .map_err(|error| format!("执行 Windows 回收操作失败: {error}"))?;

    let aborted = unsafe { operation.GetAnyOperationsAborted() }
        .map_err(|error| format!("读取 Windows 回收操作状态失败: {error}"))?;
    if aborted.as_bool() {
        return Err("Windows 回收站操作被中止".to_string());
    }

    Ok(())
}

#[cfg(target_os = "windows")]
/// 在线程退出时释放 COM apartment。
struct ComApartment;

#[cfg(target_os = "windows")]
impl Drop for ComApartment {
    /// 释放当前线程的 COM apartment。
    fn drop(&mut self) {
        unsafe { windows::Win32::System::Com::CoUninitialize() };
    }
}

#[cfg(not(target_os = "windows"))]
/// 非 Windows 平台拒绝执行只适用于 Windows 的系统回收操作。
fn send_file_to_system_recycle_bin(_source_path: &Path) -> Result<()> {
    Err(AppError::ValidationError(
        "当前平台不支持 Windows 系统回收站".to_string(),
    ))
}

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

    /// 将文件移入操作系统回收站并记录操作日志
    pub fn recycle_file(&self, result_id: i64, run_id: &str) -> Result<PathBuf> {
        self.recycle_file_with(result_id, run_id, send_file_to_system_recycle_bin)
    }

    /// 通过可替换的系统回收实现执行删除，供边界测试验证日志行为。
    fn recycle_file_with<F>(&self, result_id: i64, run_id: &str, recycle: F) -> Result<PathBuf>
    where
        F: FnOnce(&Path) -> Result<()>,
    {
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

        // 3. 系统回收站没有稳定的普通文件路径，因此只记录原路径。
        self.repository
            .update_result_action_status(result_id, ActionStatus::Prepared)?;
        self.repository.create_action_log(
            result_id,
            "prepared",
            Some(&source_path.to_string_lossy()),
            None,
            None,
        )?;

        // 4. 通过系统 Shell 执行移入回收站，失败时保留原文件。
        dispatch_system_recycle(&source_path, recycle).map_err(|error| {
            // 移动失败，记录错误
            let _ = self
                .repository
                .update_result_action_status(result_id, ActionStatus::Failed);
            let _ = self.repository.create_action_log(
                result_id,
                "failed",
                None,
                None,
                Some(&error.to_string()),
            );
            error
        })?;

        // 5. Shell 返回成功后，源路径必须已经不存在。
        if source_path.exists() {
            let _ = self
                .repository
                .update_result_action_status(result_id, ActionStatus::Failed);
            return Err(AppError::FileSystem(
                "移入 Windows 回收站后源文件仍然存在".to_string(),
            ));
        }

        // 6. 记录已移入系统回收站；target_path 留空表示由 Windows 管理位置。
        self.repository
            .update_result_action_status(result_id, ActionStatus::Recycled)?;
        self.repository.create_action_log(
            result_id,
            "recycled",
            Some(&source_path.to_string_lossy()),
            None,
            None,
        )?;

        Ok(source_path)
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
        let Some(recycled_path_value) = log.target_path.as_ref() else {
            return Err(AppError::ValidationError(
                "该文件已移入 Windows 系统回收站，请在 Windows 回收站中恢复".to_string(),
            ));
        };
        let recycled_path = PathBuf::from(recycled_path_value);

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

        let Some(recycled_path_value) = log.target_path.as_ref() else {
            return Err(AppError::ValidationError(
                "该文件已移入 Windows 系统回收站，请在 Windows 回收站中永久删除".to_string(),
            ));
        };
        let recycled_path = PathBuf::from(recycled_path_value);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "windows")]
    #[test]
    /// Windows 删除 flags 必须要求无法回收时失败。
    fn windows_recycle_operation_requires_recycle_on_delete() {
        use windows::Win32::UI::Shell::FOFX_RECYCLEONDELETE;

        assert!(system_recycle_operation_flags().contains(FOFX_RECYCLEONDELETE));
    }

    #[test]
    /// 系统回收调度不得在原目录创建程序私有副本。
    fn system_recycle_dispatch_does_not_create_an_internal_recycle_copy() {
        let temp = tempfile::tempdir().unwrap();
        let source_path = temp.path().join("confirmed.png");
        fs::write(&source_path, b"test image").unwrap();
        let mut dispatched_path = None;

        let returned_path = dispatch_system_recycle(&source_path, |path| {
            dispatched_path = Some(path.to_path_buf());
            Ok(())
        })
        .unwrap();

        assert_eq!(returned_path, source_path);
        assert_eq!(dispatched_path, Some(source_path.clone()));
        assert!(source_path.exists());
        assert!(!temp.path().join(".recycle").exists());
    }
}
