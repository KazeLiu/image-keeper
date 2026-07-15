use crate::core::image_features::compute_blake3;
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum RenameRule {
    Simple {
        template: String,
    },
    Advanced {
        old_pattern: String,
        new_template: String,
    },
    Quick {
        first_name: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameInput {
    pub source_path: String,
    pub reference_name: String,
    pub group_index: usize,
    pub order: usize,
    pub expected_fingerprint: Option<FileFingerprint>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameExecutionItem {
    pub source_path: String,
    pub new_name: String,
    pub expected_fingerprint: Option<FileFingerprint>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferInput {
    pub source_path: String,
    pub expected_fingerprint: Option<FileFingerprint>,
}

impl TransferInput {
    pub fn new(path: &Path) -> Self {
        Self {
            source_path: path.to_string_lossy().to_string(),
            expected_fingerprint: fingerprint(path).ok(),
        }
    }
}

impl RenameExecutionItem {
    pub fn new(path: &Path, new_name: &str) -> Self {
        Self {
            source_path: path.to_string_lossy().to_string(),
            new_name: new_name.to_string(),
            expected_fingerprint: fingerprint(path).ok(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FilePlanIssueKind {
    InvalidName,
    BatchDuplicate,
    TargetExists,
    SameContentExists,
    RuleUnmatched,
    SourceMissing,
    SourceChanged,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePlanIssue {
    pub kind: FilePlanIssueKind,
    pub message: String,
    pub blocking: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenamePreviewItem {
    pub source_path: String,
    pub original_name: String,
    pub proposed_name: String,
    pub target_path: String,
    pub issues: Vec<FilePlanIssue>,
    pub blocking: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferPreviewItem {
    pub source_path: String,
    pub target_path: String,
    pub issues: Vec<FilePlanIssue>,
    pub conflict: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferPreview {
    pub destination: String,
    pub items: Vec<TransferPreviewItem>,
    pub conflict_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    Rename,
    Move,
    Copy,
    Undo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationEntryStatus {
    Succeeded,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileFingerprint {
    pub blake3_hash: String,
    pub file_size: u64,
    pub modified_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationEntry {
    pub source_path: String,
    pub target_path: String,
    pub status: OperationEntryStatus,
    pub message: Option<String>,
    pub original_fingerprint: Option<FileFingerprint>,
    pub target_fingerprint: Option<FileFingerprint>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationBatchResult {
    pub batch_id: String,
    pub kind: OperationKind,
    pub entries: Vec<OperationEntry>,
    pub succeeded: usize,
    pub skipped: usize,
    pub failed: usize,
    pub reversible: bool,
}

pub fn preview_rename(items: &[RenameInput], rule: &RenameRule) -> Vec<RenamePreviewItem> {
    let quick_extensions: Vec<String> = items
        .iter()
        .map(|item| extension_of(Path::new(&item.source_path)))
        .collect();
    let quick_names = match rule {
        RenameRule::Quick { first_name } => quick_rename_names(
            first_name,
            &quick_extensions
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
        ),
        _ => Vec::new(),
    };

    let mut explicit = Vec::with_capacity(items.len());
    let mut unmatched = HashSet::new();
    let mut template_errors = HashMap::new();
    for (index, item) in items.iter().enumerate() {
        let source = Path::new(&item.source_path);
        let original_name = source
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let context = RenameContext {
            original_name: &original_name,
            reference_name: &item.reference_name,
            group_index: item.group_index,
            order: item.order,
        };
        let proposed_name = match rule {
            RenameRule::Simple { template } => match render_template(template, &context, &[]) {
                Ok(value) => value,
                Err(error) => {
                    template_errors
                        .insert(normalize_path_key(&item.source_path), error.to_string());
                    original_name.clone()
                }
            },
            RenameRule::Advanced {
                old_pattern,
                new_template,
            } => match capture_wildcards(old_pattern, &original_name) {
                Some(captures) => match render_template(new_template, &context, &captures) {
                    Ok(value) => value,
                    Err(error) => {
                        template_errors
                            .insert(normalize_path_key(&item.source_path), error.to_string());
                        original_name.clone()
                    }
                },
                None => {
                    unmatched.insert(normalize_path_key(&item.source_path));
                    original_name.clone()
                }
            },
            RenameRule::Quick { .. } => quick_names[index].clone(),
        };
        explicit.push(RenameExecutionItem {
            source_path: item.source_path.clone(),
            new_name: proposed_name,
            expected_fingerprint: item.expected_fingerprint.clone(),
        });
    }

    let mut preview = preview_explicit_names(&explicit);
    for item in &mut preview {
        if unmatched.contains(&normalize_path_key(&item.source_path)) {
            item.issues.push(FilePlanIssue {
                kind: FilePlanIssueKind::RuleUnmatched,
                message: "原文件名不匹配高级规则，已保留原名称".to_string(),
                blocking: false,
            });
        }
        if let Some(message) = template_errors.get(&normalize_path_key(&item.source_path)) {
            item.issues
                .push(blocking_issue(FilePlanIssueKind::InvalidName, message));
            item.blocking = true;
        }
    }
    preview
}

pub fn preview_explicit_names(items: &[RenameExecutionItem]) -> Vec<RenamePreviewItem> {
    let source_keys: HashSet<_> = items
        .iter()
        .map(|item| normalize_path_key(&item.source_path))
        .collect();
    let mut target_counts: HashMap<String, usize> = HashMap::new();
    let mut target_paths = Vec::with_capacity(items.len());
    for item in items {
        let source = Path::new(&item.source_path);
        let target = source
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(&item.new_name);
        let key = normalize_path_key(&target.to_string_lossy());
        *target_counts.entry(key).or_default() += 1;
        target_paths.push(target);
    }

    items
        .iter()
        .zip(target_paths)
        .map(|(item, target)| {
            let source = Path::new(&item.source_path);
            let original_name = source
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let mut issues = Vec::new();
            if !source.is_file() {
                issues.push(blocking_issue(
                    FilePlanIssueKind::SourceMissing,
                    "源文件不存在或已被移动",
                ));
            } else if let Some(expected) = &item.expected_fingerprint {
                match fingerprint(source) {
                    Ok(actual) if actual != *expected => issues.push(blocking_issue(
                        FilePlanIssueKind::SourceChanged,
                        "文件在搜索后发生变化，请重新搜索后再操作",
                    )),
                    Err(error) => issues.push(blocking_issue(
                        FilePlanIssueKind::SourceChanged,
                        &format!("无法复核文件状态: {error}"),
                    )),
                    Ok(_) => {}
                }
            } else {
                issues.push(blocking_issue(
                    FilePlanIssueKind::SourceChanged,
                    "缺少搜索时的文件指纹，请重新搜索后再操作",
                ));
            }
            if let Some(message) = invalid_file_name_reason(&item.new_name) {
                issues.push(blocking_issue(FilePlanIssueKind::InvalidName, &message));
            }
            let target_key = normalize_path_key(&target.to_string_lossy());
            if target_counts.get(&target_key).copied().unwrap_or_default() > 1 {
                issues.push(blocking_issue(
                    FilePlanIssueKind::BatchDuplicate,
                    "批次内生成了重复文件名",
                ));
            }
            let source_key = normalize_path_key(&item.source_path);
            if target.exists() && target_key != source_key && !source_keys.contains(&target_key) {
                let same_content =
                    source.is_file() && compute_blake3(source).ok() == compute_blake3(&target).ok();
                issues.push(if same_content {
                    FilePlanIssue {
                        kind: FilePlanIssueKind::SameContentExists,
                        message: "目标位置已有相同内容".to_string(),
                        blocking: true,
                    }
                } else {
                    blocking_issue(FilePlanIssueKind::TargetExists, "目标位置已有同名文件")
                });
            }
            let blocking = issues.iter().any(|issue| issue.blocking);
            RenamePreviewItem {
                source_path: item.source_path.clone(),
                original_name,
                proposed_name: item.new_name.clone(),
                target_path: target.to_string_lossy().to_string(),
                issues,
                blocking,
            }
        })
        .collect()
}

pub fn quick_rename_names(first_name: &str, extensions: &[&str]) -> Vec<String> {
    let stem = Path::new(first_name)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    extensions
        .iter()
        .enumerate()
        .map(|(index, extension)| {
            if extension.is_empty() {
                format!("{stem}_{}", index + 1)
            } else {
                format!("{stem}_{}.{}", index + 1, extension)
            }
        })
        .collect()
}

pub fn preview_transfer(items: &[TransferInput], target_directory: &Path) -> TransferPreview {
    let preview_items = items
        .iter()
        .map(|item| {
            let source = Path::new(&item.source_path);
            let target = target_directory.join(source.file_name().unwrap_or_default());
            let mut issues = Vec::new();
            if !source.is_file() {
                issues.push(blocking_issue(
                    FilePlanIssueKind::SourceMissing,
                    "源文件不存在或已被移动",
                ));
            } else {
                match (&item.expected_fingerprint, fingerprint(source)) {
                    (Some(expected), Ok(actual)) if *expected == actual => {}
                    (Some(_), Ok(_)) => issues.push(blocking_issue(
                        FilePlanIssueKind::SourceChanged,
                        "文件在搜索后发生变化，请重新搜索后再操作",
                    )),
                    (_, Err(error)) => issues.push(blocking_issue(
                        FilePlanIssueKind::SourceChanged,
                        &format!("无法复核文件状态: {error}"),
                    )),
                    (None, Ok(_)) => issues.push(blocking_issue(
                        FilePlanIssueKind::SourceChanged,
                        "缺少搜索时的文件指纹，请重新搜索后再操作",
                    )),
                }
            }
            if target.exists() {
                let same_content = source.is_file()
                    && target.is_file()
                    && compute_blake3(source).ok() == compute_blake3(&target).ok();
                issues.push(blocking_issue(
                    if same_content {
                        FilePlanIssueKind::SameContentExists
                    } else {
                        FilePlanIssueKind::TargetExists
                    },
                    if same_content {
                        "目标位置已有相同内容"
                    } else {
                        "目标位置已有同名文件"
                    },
                ));
            }
            TransferPreviewItem {
                source_path: item.source_path.clone(),
                target_path: target.to_string_lossy().to_string(),
                conflict: !issues.is_empty(),
                issues,
            }
        })
        .collect::<Vec<_>>();
    let conflict_count = preview_items.iter().filter(|item| item.conflict).count();
    TransferPreview {
        destination: target_directory.to_string_lossy().to_string(),
        items: preview_items,
        conflict_count,
    }
}

pub fn execute_rename(items: &[RenameExecutionItem]) -> Result<OperationBatchResult> {
    let preview = preview_explicit_names(items);
    if preview.iter().any(|item| item.blocking) {
        return Err(AppError::ValidationError(
            "重命名计划存在未解决的冲突".to_string(),
        ));
    }
    let mappings: Vec<_> = preview
        .iter()
        .filter(|item| item.source_path != item.target_path)
        .map(|item| {
            (
                PathBuf::from(&item.source_path),
                PathBuf::from(&item.target_path),
            )
        })
        .collect();
    let fingerprints: HashMap<_, _> = mappings
        .iter()
        .map(|(source, _)| {
            Ok((
                normalize_path_key(&source.to_string_lossy()),
                fingerprint(source)?,
            ))
        })
        .collect::<Result<_>>()?;
    execute_path_mapping(&mappings)?;

    let entries = mappings
        .into_iter()
        .map(|(source, target)| OperationEntry {
            original_fingerprint: fingerprints
                .get(&normalize_path_key(&source.to_string_lossy()))
                .cloned(),
            source_path: source.to_string_lossy().to_string(),
            target_path: target.to_string_lossy().to_string(),
            status: OperationEntryStatus::Succeeded,
            message: None,
            target_fingerprint: fingerprint(&target).ok(),
        })
        .collect::<Vec<_>>();
    Ok(batch_result(OperationKind::Rename, entries, true))
}

pub fn execute_move(
    items: &[TransferInput],
    target_directory: &Path,
) -> Result<OperationBatchResult> {
    std::fs::create_dir_all(target_directory)?;
    execute_transfer(items, target_directory, OperationKind::Move)
}

pub fn execute_copy(
    items: &[TransferInput],
    target_directory: &Path,
) -> Result<OperationBatchResult> {
    std::fs::create_dir_all(target_directory)?;
    execute_transfer(items, target_directory, OperationKind::Copy)
}

pub fn undo_operation_batch(batch: &OperationBatchResult) -> Result<OperationBatchResult> {
    if !batch.reversible || !matches!(batch.kind, OperationKind::Rename | OperationKind::Move) {
        return Err(AppError::ValidationError("该批次不支持撤销".to_string()));
    }
    let mappings: Vec<_> = batch
        .entries
        .iter()
        .filter(|entry| entry.status == OperationEntryStatus::Succeeded)
        .map(|entry| {
            let current = PathBuf::from(&entry.target_path);
            let expected = entry
                .target_fingerprint
                .as_ref()
                .ok_or_else(|| AppError::ValidationError("撤销记录缺少文件指纹".to_string()))?;
            let actual = fingerprint(&current)?;
            if actual != *expected {
                return Err(AppError::ValidationError(format!(
                    "文件内容已变化，无法撤销: {}",
                    entry.target_path
                )));
            }
            Ok((current, PathBuf::from(&entry.source_path)))
        })
        .collect::<Result<_>>()?;
    execute_path_mapping(&mappings)?;
    let entries = mappings
        .into_iter()
        .map(|(source, target)| OperationEntry {
            source_path: source.to_string_lossy().to_string(),
            target_path: target.to_string_lossy().to_string(),
            status: OperationEntryStatus::Succeeded,
            message: None,
            original_fingerprint: fingerprint(&target).ok(),
            target_fingerprint: fingerprint(&target).ok(),
        })
        .collect();
    Ok(batch_result(OperationKind::Undo, entries, false))
}

fn execute_transfer(
    items: &[TransferInput],
    target_directory: &Path,
    kind: OperationKind,
) -> Result<OperationBatchResult> {
    let mut entries = Vec::with_capacity(items.len());
    for item in items {
        let source = Path::new(&item.source_path);
        let target = target_directory.join(source.file_name().unwrap_or_default());
        if !source.is_file() {
            entries.push(outcome(
                source,
                &target,
                OperationEntryStatus::Failed,
                "源文件不存在",
                None,
            ));
            continue;
        }
        let before = fingerprint(source)?;
        if item.expected_fingerprint.as_ref() != Some(&before) {
            entries.push(outcome(
                source,
                &target,
                OperationEntryStatus::Failed,
                "文件在搜索后发生变化，请重新搜索后再操作",
                Some(before),
            ));
            continue;
        }
        if target.exists() {
            let message = if compute_blake3(source).ok() == compute_blake3(&target).ok() {
                "目标位置已有相同内容"
            } else {
                "目标位置已有同名文件"
            };
            entries.push(outcome(
                source,
                &target,
                OperationEntryStatus::Skipped,
                message,
                Some(before),
            ));
            continue;
        }

        let result = match kind {
            OperationKind::Move => move_file_safely(source, &target),
            OperationKind::Copy => copy_file_verified(source, &target),
            _ => unreachable!(),
        };
        match result {
            Ok(()) => entries.push(outcome(
                source,
                &target,
                OperationEntryStatus::Succeeded,
                "",
                Some(before),
            )),
            Err(error) => entries.push(outcome(
                source,
                &target,
                OperationEntryStatus::Failed,
                &error.to_string(),
                Some(before),
            )),
        }
    }
    Ok(batch_result(kind, entries, kind == OperationKind::Move))
}

fn execute_path_mapping(mappings: &[(PathBuf, PathBuf)]) -> Result<()> {
    if mappings.is_empty() {
        return Ok(());
    }

    match perform_two_phase_mapping(mappings) {
        Ok(()) => Ok(()),
        Err(failure) => {
            let original_error = failure.error.to_string();
            let recovery: Vec<_> = failure
                .current_locations
                .into_iter()
                .zip(mappings.iter())
                .filter_map(|(current, (original, _))| {
                    (current != *original).then(|| (current, original.clone()))
                })
                .collect();

            if recovery.is_empty() {
                return Err(failure.error);
            }

            match perform_two_phase_mapping(&recovery) {
                Ok(()) => Err(failure.error),
                Err(recovery_failure) => {
                    let manual_recovery = recovery_failure
                        .current_locations
                        .iter()
                        .zip(recovery.iter())
                        .filter(|(current, (_, original))| *current != original)
                        .map(|(current, (_, original))| {
                            format!("{} -> {}", current.display(), original.display())
                        })
                        .collect::<Vec<_>>()
                        .join("; ");
                    Err(AppError::FileSystem(format!(
                        "文件映射失败: {original_error}；自动恢复失败: {}。请按以下路径手工恢复: {manual_recovery}",
                        recovery_failure.error
                    )))
                }
            }
        }
    }
}

struct MappingFailure {
    error: AppError,
    current_locations: Vec<PathBuf>,
}

fn perform_two_phase_mapping(
    mappings: &[(PathBuf, PathBuf)],
) -> std::result::Result<(), MappingFailure> {
    let mut current_locations = mappings
        .iter()
        .map(|(source, _)| source.clone())
        .collect::<Vec<_>>();

    for (source, target) in mappings {
        if !source.is_file() {
            return Err(MappingFailure {
                error: AppError::ValidationError(format!("源文件不存在: {}", source.display())),
                current_locations,
            });
        }
        if let Some(parent) = target.parent() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                return Err(MappingFailure {
                    error: error.into(),
                    current_locations,
                });
            }
        }
    }

    let mut staged = Vec::with_capacity(mappings.len());
    for (index, (source, target)) in mappings.iter().enumerate() {
        let temporary =
            source.with_file_name(format!(".imagekeeper-{}.tmp", Uuid::new_v4().simple()));
        if let Err(error) = move_file_safely(source, &temporary) {
            return Err(MappingFailure {
                error,
                current_locations,
            });
        }
        current_locations[index] = temporary.clone();
        staged.push((temporary, target.clone()));
    }

    for (index, (temporary, target)) in staged.iter().enumerate() {
        if let Err(error) = move_file_safely(temporary, target) {
            return Err(MappingFailure {
                error,
                current_locations,
            });
        }
        current_locations[index] = target.clone();
    }
    Ok(())
}

fn move_file_safely(source: &Path, target: &Path) -> Result<()> {
    if source == target {
        return Ok(());
    }
    match std::fs::hard_link(source, target) {
        Ok(()) => {
            if let Err(error) = std::fs::remove_file(source) {
                let cleanup_error = std::fs::remove_file(target).err();
                return Err(AppError::FileSystem(match cleanup_error {
                    Some(cleanup) => {
                        format!("删除移动源文件失败: {error}；清理目标文件也失败: {cleanup}")
                    }
                    None => format!("删除移动源文件失败: {error}"),
                }));
            }
            return Ok(());
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(AppError::ValidationError(format!(
                "目标文件已存在: {}",
                target.display()
            )));
        }
        Err(_) => {}
    }

    copy_file_verified(source, target)?;
    if let Err(error) = std::fs::remove_file(source) {
        let cleanup_error = std::fs::remove_file(target).err();
        return Err(AppError::FileSystem(match cleanup_error {
            Some(cleanup) => {
                format!("删除移动源文件失败: {error}；清理目标文件也失败: {cleanup}")
            }
            None => format!("删除移动源文件失败: {error}"),
        }));
    }
    Ok(())
}

fn copy_file_verified(source: &Path, target: &Path) -> Result<()> {
    let before = fingerprint(source)?;
    let mut source_file = std::fs::File::open(source)?;
    let mut target_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                AppError::ValidationError(format!("目标文件已存在: {}", target.display()))
            } else {
                error.into()
            }
        })?;

    let copy_result = (|| -> Result<()> {
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = source_file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            target_file.write_all(&buffer[..read])?;
        }
        target_file.sync_all()?;

        let after = fingerprint(source)?;
        let copied = fingerprint(target)?;
        if before.blake3_hash != after.blake3_hash
            || before.file_size != after.file_size
            || copied.blake3_hash != before.blake3_hash
            || copied.file_size != before.file_size
        {
            return Err(AppError::FileSystem(
                "复制期间源文件发生变化，或复制后校验失败".to_string(),
            ));
        }
        Ok(())
    })();
    drop(target_file);

    if let Err(error) = copy_result {
        let cleanup_error = std::fs::remove_file(target).err();
        return Err(match cleanup_error {
            Some(cleanup) => {
                AppError::FileSystem(format!("{error}；清理未完成的目标文件失败: {cleanup}"))
            }
            None => error,
        });
    }
    Ok(())
}

struct RenameContext<'a> {
    original_name: &'a str,
    reference_name: &'a str,
    group_index: usize,
    order: usize,
}

fn render_template(
    template: &str,
    context: &RenameContext<'_>,
    captures: &[String],
) -> Result<String> {
    let original_path = Path::new(context.original_name);
    let name = original_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let extension = extension_of(original_path);
    let mut output = template.replace("$$", "\u{0}");
    output = output.replace("$n:03", &format!("{:03}", context.order));
    output = output.replace("$n:02", &format!("{:02}", context.order));
    output = output.replace("$name", &name);
    output = output.replace("$ext", &extension);
    output = output.replace("$ref", context.reference_name);
    output = output.replace("$group", &context.group_index.to_string());
    output = output.replace("$n", &context.order.to_string());
    for index in (1..=9).rev() {
        let value = captures.get(index - 1).map(String::as_str).unwrap_or("");
        output = output.replace(&format!("${index}"), value);
    }
    output = output.replace('\u{0}', "$");
    if output.contains('$') {
        return Err(AppError::ValidationError(
            "名称模板包含未知变量".to_string(),
        ));
    }
    Ok(output)
}

fn capture_wildcards(pattern: &str, value: &str) -> Option<Vec<String>> {
    let parts: Vec<_> = pattern.split('*').collect();
    if parts.len() == 1 {
        return (pattern == value).then(Vec::new);
    }
    if !value.starts_with(parts[0]) {
        return None;
    }
    let mut cursor = parts[0].len();
    let mut captures = Vec::with_capacity(parts.len() - 1);
    for index in 0..parts.len() - 1 {
        let next_literal = parts[index + 1];
        if index == parts.len() - 2 {
            if !value[cursor..].ends_with(next_literal) {
                return None;
            }
            let end = value.len() - next_literal.len();
            captures.push(value[cursor..end].to_string());
            cursor = value.len();
        } else {
            let offset = value[cursor..].find(next_literal)?;
            let end = cursor + offset;
            captures.push(value[cursor..end].to_string());
            cursor = end + next_literal.len();
        }
    }
    (cursor == value.len()).then_some(captures)
}

fn invalid_file_name_reason(name: &str) -> Option<String> {
    if name.trim().is_empty() || matches!(name, "." | "..") {
        return Some("文件名不能为空".to_string());
    }
    if name.ends_with(' ') || name.ends_with('.') {
        return Some("Windows 文件名不能以空格或句点结尾".to_string());
    }
    if name
        .chars()
        .any(|character| character < ' ' || "<>:\"/\\|?*".contains(character))
    {
        return Some("文件名包含 Windows 不允许的字符".to_string());
    }
    let stem = Path::new(name)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem[3..]
                .parse::<u8>()
                .is_ok_and(|number| (1..=9).contains(&number)));
    reserved.then(|| "文件名使用了 Windows 保留名称".to_string())
}

fn fingerprint(path: &Path) -> Result<FileFingerprint> {
    let metadata = std::fs::metadata(path)?;
    let modified_at = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|value| value.as_secs() as i64)
        .unwrap_or_default();
    Ok(FileFingerprint {
        blake3_hash: compute_blake3(path)?,
        file_size: metadata.len(),
        modified_at,
    })
}

fn extension_of(path: &Path) -> String {
    path.extension()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn blocking_issue(kind: FilePlanIssueKind, message: &str) -> FilePlanIssue {
    FilePlanIssue {
        kind,
        message: message.to_string(),
        blocking: true,
    }
}

fn normalize_path_key(path: &str) -> String {
    path.replace('/', "\\").to_lowercase()
}

fn batch_result(
    kind: OperationKind,
    entries: Vec<OperationEntry>,
    reversible: bool,
) -> OperationBatchResult {
    let succeeded = entries
        .iter()
        .filter(|entry| entry.status == OperationEntryStatus::Succeeded)
        .count();
    let skipped = entries
        .iter()
        .filter(|entry| entry.status == OperationEntryStatus::Skipped)
        .count();
    let failed = entries
        .iter()
        .filter(|entry| entry.status == OperationEntryStatus::Failed)
        .count();
    OperationBatchResult {
        batch_id: Uuid::new_v4().to_string(),
        kind,
        entries,
        succeeded,
        skipped,
        failed,
        reversible: reversible && succeeded > 0,
    }
}

fn outcome(
    source: &Path,
    target: &Path,
    status: OperationEntryStatus,
    message: &str,
    original_fingerprint: Option<FileFingerprint>,
) -> OperationEntry {
    OperationEntry {
        source_path: source.to_string_lossy().to_string(),
        target_path: target.to_string_lossy().to_string(),
        status,
        message: (!message.is_empty()).then(|| message.to_string()),
        original_fingerprint,
        target_fingerprint: (status == OperationEntryStatus::Succeeded)
            .then(|| fingerprint(target).ok())
            .flatten(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(path: &std::path::Path, order: usize) -> RenameInput {
        RenameInput {
            source_path: path.to_string_lossy().to_string(),
            reference_name: "三月七".to_string(),
            group_index: 1,
            order,
            expected_fingerprint: fingerprint(path).ok(),
        }
    }

    #[test]
    fn renders_simple_template_with_zero_padded_order() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("立绘.png");
        std::fs::write(&source, b"image").unwrap();

        let preview = preview_rename(
            &[input(&source, 2)],
            &RenameRule::Simple {
                template: "$ref-$n:02.$ext".to_string(),
            },
        );

        assert_eq!(preview[0].proposed_name, "三月七-02.png");
        assert!(!preview[0].blocking);
    }

    #[test]
    fn captures_wildcards_for_advanced_rename() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("表情_角色.png");
        std::fs::write(&source, b"image").unwrap();

        let preview = preview_rename(
            &[input(&source, 1)],
            &RenameRule::Advanced {
                old_pattern: "*_*.png".to_string(),
                new_template: "$2-$1.png".to_string(),
            },
        );

        assert_eq!(preview[0].proposed_name, "角色-表情.png");
        assert!(!preview[0].blocking);
    }

    #[test]
    fn quick_rename_uses_first_stem_and_each_original_extension() {
        let names = quick_rename_names("三月七.png", &["png", "jpg", "webp"]);
        assert_eq!(names, ["三月七_1.png", "三月七_2.jpg", "三月七_3.webp"]);
    }

    #[test]
    fn swaps_two_names_without_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.png");
        let b = dir.path().join("b.png");
        std::fs::write(&a, b"A").unwrap();
        std::fs::write(&b, b"B").unwrap();

        let batch = execute_rename(&[
            RenameExecutionItem::new(&a, "b.png"),
            RenameExecutionItem::new(&b, "a.png"),
        ])
        .unwrap();

        assert_eq!(std::fs::read(&a).unwrap(), b"B");
        assert_eq!(std::fs::read(&b).unwrap(), b"A");
        assert_eq!(batch.succeeded, 2);
    }

    #[cfg(windows)]
    #[test]
    fn supports_case_only_rename_on_windows() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("lower.png");
        std::fs::write(&source, b"A").unwrap();

        let batch = execute_rename(&[RenameExecutionItem::new(&source, "LOWER.png")]).unwrap();
        let actual_name = std::fs::read_dir(dir.path())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .file_name()
            .to_string_lossy()
            .to_string();

        assert_eq!(batch.succeeded, 1);
        assert_eq!(actual_name, "LOWER.png");
    }

    #[test]
    fn restores_every_source_when_final_mapping_fails() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.png");
        let b = dir.path().join("b.png");
        let c = dir.path().join("c.png");
        let blocked_target = dir.path().join("blocked.png");
        std::fs::write(&a, b"A").unwrap();
        std::fs::write(&b, b"B").unwrap();
        std::fs::write(&c, b"C").unwrap();
        std::fs::create_dir(&blocked_target).unwrap();

        let result = execute_path_mapping(&[
            (a.clone(), b.clone()),
            (b.clone(), c.clone()),
            (c.clone(), blocked_target),
        ]);

        assert!(result.is_err());
        assert_eq!(std::fs::read(&a).unwrap(), b"A");
        assert_eq!(std::fs::read(&b).unwrap(), b"B");
        assert_eq!(std::fs::read(&c).unwrap(), b"C");
        assert_eq!(
            std::fs::read_dir(dir.path())
                .unwrap()
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".imagekeeper-"))
                .count(),
            0
        );
    }

    #[test]
    fn blocks_rename_when_source_changed_after_search() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("changed.png");
        std::fs::write(&source, b"before").unwrap();
        let expected = fingerprint(&source).unwrap();
        std::fs::write(&source, b"after").unwrap();

        let preview = preview_explicit_names(&[RenameExecutionItem {
            source_path: source.to_string_lossy().to_string(),
            new_name: "renamed.png".to_string(),
            expected_fingerprint: Some(expected),
        }]);

        assert!(preview[0].blocking);
        assert!(preview[0]
            .issues
            .iter()
            .any(|issue| issue.kind == FilePlanIssueKind::SourceChanged));
    }

    #[test]
    fn blocks_copy_when_source_changed_after_search() {
        let source_dir = tempfile::tempdir().unwrap();
        let target_dir = tempfile::tempdir().unwrap();
        let source = source_dir.path().join("changed.png");
        std::fs::write(&source, b"before").unwrap();
        let expected = fingerprint(&source).unwrap();
        std::fs::write(&source, b"after").unwrap();

        let batch = execute_copy(
            &[TransferInput {
                source_path: source.to_string_lossy().to_string(),
                expected_fingerprint: Some(expected),
            }],
            target_dir.path(),
        )
        .unwrap();

        assert_eq!(batch.failed, 1);
        assert!(!target_dir.path().join("changed.png").exists());
    }

    #[test]
    fn transfer_preview_lists_targets_and_conflicts() {
        let source_dir = tempfile::tempdir().unwrap();
        let target_dir = tempfile::tempdir().unwrap();
        let source = source_dir.path().join("same.png");
        std::fs::write(&source, b"source").unwrap();
        std::fs::write(target_dir.path().join("same.png"), b"existing").unwrap();

        let preview = preview_transfer(&[TransferInput::new(&source)], target_dir.path());

        assert_eq!(preview.items.len(), 1);
        assert_eq!(preview.conflict_count, 1);
        assert_eq!(
            Path::new(&preview.items[0].target_path),
            target_dir.path().join("same.png")
        );
    }

    #[test]
    fn blocks_duplicate_generated_names_case_insensitively() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.png");
        let b = dir.path().join("b.png");
        std::fs::write(&a, b"A").unwrap();
        std::fs::write(&b, b"B").unwrap();

        let preview = preview_explicit_names(&[
            RenameExecutionItem::new(&a, "same.png"),
            RenameExecutionItem::new(&b, "SAME.png"),
        ]);

        assert!(preview.iter().all(|item| item.blocking));
        assert!(preview.iter().all(|item| item
            .issues
            .iter()
            .any(|issue| { issue.kind == FilePlanIssueKind::BatchDuplicate })));
    }

    #[test]
    fn blocks_unknown_template_variables() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("a.png");
        std::fs::write(&source, b"A").unwrap();

        let preview = preview_rename(
            &[input(&source, 1)],
            &RenameRule::Simple {
                template: "$unknown.png".to_string(),
            },
        );

        assert!(preview[0].blocking);
        assert!(preview[0]
            .issues
            .iter()
            .any(|issue| issue.kind == FilePlanIssueKind::InvalidName));
    }

    #[test]
    fn moves_and_undoes_a_batch() {
        let dir = tempfile::tempdir().unwrap();
        let source_dir = dir.path().join("source");
        let target_dir = dir.path().join("target");
        std::fs::create_dir_all(&source_dir).unwrap();
        let source = source_dir.join("a.png");
        std::fs::write(&source, b"A").unwrap();

        let moved = execute_move(&[TransferInput::new(&source)], &target_dir).unwrap();
        assert!(target_dir.join("a.png").exists());

        let undone = undo_operation_batch(&moved).unwrap();
        assert_eq!(undone.succeeded, 1);
        assert!(source.exists());
        assert!(!target_dir.join("a.png").exists());
    }

    #[test]
    fn skips_copy_when_same_content_already_exists() {
        let dir = tempfile::tempdir().unwrap();
        let source_dir = dir.path().join("source");
        let target_dir = dir.path().join("target");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::create_dir_all(&target_dir).unwrap();
        let source = source_dir.join("a.png");
        let target = target_dir.join("a.png");
        std::fs::write(&source, b"A").unwrap();
        std::fs::write(&target, b"A").unwrap();

        let copied = execute_copy(&[TransferInput::new(&source)], &target_dir).unwrap();

        assert_eq!(copied.skipped, 1);
        assert_eq!(copied.failed, 0);
    }
}
