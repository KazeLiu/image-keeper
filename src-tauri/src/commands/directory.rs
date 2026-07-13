use std::path::Path;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct TreeNode {
    label: String,
    path: String,
    #[serde(rename = "fileCount")]
    file_count: usize,
    children: Option<Vec<TreeNode>>,
}

/// 加载目录树
#[tauri::command]
pub fn load_directory_tree(path: String) -> Result<Vec<TreeNode>, String> {
    let root_path = Path::new(&path);

    if !root_path.exists() {
        return Err("目录不存在".to_string());
    }

    if !root_path.is_dir() {
        return Err("路径不是目录".to_string());
    }

    // 收集所有目录和图片数量
    let mut dir_info: HashMap<String, (String, usize)> = HashMap::new();

    // 支持的图片格式
    let image_extensions = ["jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff", "svg"];

    for entry in WalkDir::new(root_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let entry_path = entry.path();

        // 统计图片文件
        if entry_path.is_file() {
            if let Some(ext) = entry_path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                if image_extensions.contains(&ext_str.as_str()) {
                    // 找到父目录并增加计数
                    if let Some(parent) = entry_path.parent() {
                        let parent_str = parent.to_string_lossy().to_string();
                        let parent_name = parent
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();

                        dir_info
                            .entry(parent_str.clone())
                            .and_modify(|(_, count)| *count += 1)
                            .or_insert((parent_name, 1));
                    }
                }
            }
        }
    }

    // 构建树结构
    let tree = build_tree(root_path, &dir_info);

    Ok(vec![tree])
}

fn build_tree(path: &Path, dir_info: &HashMap<String, (String, usize)>) -> TreeNode {
    let path_str = path.to_string_lossy().to_string();
    let label = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let file_count = dir_info.get(&path_str).map(|(_, count)| *count).unwrap_or(0);

    // 获取直接子目录
    let mut children = Vec::new();
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                let child_str = entry_path.to_string_lossy().to_string();
                // 只包含有图片的目录
                if dir_info.contains_key(&child_str) {
                    children.push(build_tree(&entry_path, dir_info));
                }
            }
        }
    }

    // 按名称排序
    children.sort_by(|a, b| a.label.cmp(&b.label));

    TreeNode {
        label,
        path: path_str,
        file_count,
        children: if children.is_empty() {
            None
        } else {
            Some(children)
        },
    }
}
