use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

/// Phase 0: 目录预检
/// 在读取图片前完成所有安全检查
pub struct PreflightChecker {
    baseline_root: PathBuf,
    comparison_roots: Vec<PathBuf>,
    exclude_patterns: Vec<String>,
    allow_same_root_comparison: bool,
}

impl PreflightChecker {
    pub fn new(baseline_root: impl AsRef<Path>, comparison_roots: Vec<impl AsRef<Path>>) -> Self {
        Self {
            baseline_root: baseline_root.as_ref().to_path_buf(),
            comparison_roots: comparison_roots
                .iter()
                .map(|p| p.as_ref().to_path_buf())
                .collect(),
            exclude_patterns: vec![".recycle".to_string(), ".git".to_string()],
            allow_same_root_comparison: false,
        }
    }

    pub fn new_allowing_same_root(
        baseline_root: impl AsRef<Path>,
        comparison_roots: Vec<impl AsRef<Path>>,
    ) -> Self {
        Self {
            baseline_root: baseline_root.as_ref().to_path_buf(),
            comparison_roots: comparison_roots
                .iter()
                .map(|p| p.as_ref().to_path_buf())
                .collect(),
            exclude_patterns: vec![".recycle".to_string(), ".git".to_string()],
            allow_same_root_comparison: true,
        }
    }

    /// 执行完整的预检流程
    pub fn check_all(&self) -> Result<PreflightReport> {
        let mut report = PreflightReport {
            baseline_exists: false,
            baseline_readable: false,
            comparison_roots_ok: vec![],
            has_path_conflicts: false,
            has_nesting_conflicts: false,
            warnings: vec![],
            errors: vec![],
        };

        // 1. 检查基准目录存在且可读
        if !self.baseline_root.exists() {
            report
                .errors
                .push(format!("基准目录不存在: {:?}", self.baseline_root));
        } else {
            report.baseline_exists = true;

            if !self.baseline_root.is_dir() {
                report
                    .errors
                    .push(format!("基准路径不是目录: {:?}", self.baseline_root));
            } else if !is_readable(&self.baseline_root) {
                report
                    .errors
                    .push(format!("基准目录不可读: {:?}", self.baseline_root));
            } else {
                report.baseline_readable = true;
            }
        }

        // 2. 检查至少有一个对比目录
        if self.comparison_roots.is_empty() {
            report.errors.push("至少需要选择一个对比目录".to_string());
        }

        // 3. 检查每个对比目录存在且可读
        for root in &self.comparison_roots {
            let mut root_ok = true;

            if !root.exists() {
                report.errors.push(format!("对比目录不存在: {:?}", root));
                root_ok = false;
            } else if !root.is_dir() {
                report.errors.push(format!("对比路径不是目录: {:?}", root));
                root_ok = false;
            } else if !is_readable(root) {
                report.errors.push(format!("对比目录不可读: {:?}", root));
                root_ok = false;
            }

            report.comparison_roots_ok.push(root_ok);
        }

        // 4. 规范化路径并检查冲突
        let canonical_baseline = self
            .baseline_root
            .canonicalize()
            .context("无法规范化基准目录路径")?;

        let mut canonical_comparisons = Vec::new();
        for root in &self.comparison_roots {
            match root.canonicalize() {
                Ok(canonical) => canonical_comparisons.push(canonical),
                Err(e) => {
                    report
                        .errors
                        .push(format!("无法规范化对比目录路径 {:?}: {}", root, e));
                }
            }
        }

        // 5. 检查基准目录与对比目录是否相同或嵌套
        for (idx, comp_root) in canonical_comparisons.iter().enumerate() {
            if comp_root == &canonical_baseline {
                if self.allow_same_root_comparison {
                    report.warnings.push(format!(
                        "对比目录 {} 与基准目录相同，将作为目录内部对比处理",
                        idx + 1
                    ));
                    continue;
                } else {
                    report.has_path_conflicts = true;
                    report
                        .errors
                        .push(format!("对比目录 {} 与基准目录相同", idx + 1));
                }
            }

            if is_nested(&canonical_baseline, comp_root) {
                report.has_nesting_conflicts = true;
                report
                    .errors
                    .push(format!("对比目录 {} 嵌套在基准目录内", idx + 1));
            }

            if is_nested(comp_root, &canonical_baseline) {
                report.has_nesting_conflicts = true;
                report
                    .errors
                    .push(format!("基准目录嵌套在对比目录 {} 内", idx + 1));
            }
        }

        // 6. 检查对比目录之间是否相同或嵌套
        for i in 0..canonical_comparisons.len() {
            for j in (i + 1)..canonical_comparisons.len() {
                let root_i = &canonical_comparisons[i];
                let root_j = &canonical_comparisons[j];

                if root_i == root_j {
                    report.has_path_conflicts = true;
                    report
                        .errors
                        .push(format!("对比目录 {} 和 {} 相同", i + 1, j + 1));
                }

                if is_nested(root_i, root_j) {
                    report.has_nesting_conflicts = true;
                    report
                        .errors
                        .push(format!("对比目录 {} 嵌套在对比目录 {} 内", j + 1, i + 1));
                }

                if is_nested(root_j, root_i) {
                    report.has_nesting_conflicts = true;
                    report
                        .errors
                        .push(format!("对比目录 {} 嵌套在对比目录 {} 内", i + 1, j + 1));
                }
            }
        }

        // 7. 检查符号链接策略（默认不跟随）
        report
            .warnings
            .push("符号链接将被忽略（不跟随目录符号链接）".to_string());

        // 8. 检查排除目录
        for pattern in &self.exclude_patterns {
            report
                .warnings
                .push(format!("将排除包含 '{}' 的目录", pattern));
        }

        Ok(report)
    }

    /// 检查是否可以开始运行（无阻塞性错误）
    pub fn can_proceed(&self) -> Result<()> {
        let report = self.check_all()?;

        if !report.errors.is_empty() {
            bail!("预检失败:\n{}", report.errors.join("\n"));
        }

        Ok(())
    }
}

/// 预检报告
#[derive(Debug, Clone)]
pub struct PreflightReport {
    pub baseline_exists: bool,
    pub baseline_readable: bool,
    pub comparison_roots_ok: Vec<bool>,
    pub has_path_conflicts: bool,
    pub has_nesting_conflicts: bool,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

impl PreflightReport {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// 检查目录是否可读
fn is_readable(path: &Path) -> bool {
    path.read_dir().is_ok()
}

/// 检查 child 是否嵌套在 parent 内
fn is_nested(parent: &Path, child: &Path) -> bool {
    child.starts_with(parent) && child != parent
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_preflight_valid_setup() {
        let temp_dir = TempDir::new().unwrap();
        let baseline = temp_dir.path().join("baseline");
        let comp1 = temp_dir.path().join("comp1");
        let comp2 = temp_dir.path().join("comp2");

        fs::create_dir_all(&baseline).unwrap();
        fs::create_dir_all(&comp1).unwrap();
        fs::create_dir_all(&comp2).unwrap();

        let checker = PreflightChecker::new(&baseline, vec![&comp1, &comp2]);
        let report = checker.check_all().unwrap();

        assert!(report.is_ok());
        assert_eq!(report.comparison_roots_ok.len(), 2);
    }

    #[test]
    fn test_preflight_same_directory() {
        let temp_dir = TempDir::new().unwrap();
        let baseline = temp_dir.path().join("baseline");
        fs::create_dir_all(&baseline).unwrap();

        let checker = PreflightChecker::new(&baseline, vec![&baseline]);
        let report = checker.check_all().unwrap();

        assert!(!report.is_ok());
        assert!(report.has_path_conflicts);
    }

    #[test]
    fn test_preflight_allows_same_directory_for_internal_compare() {
        let temp_dir = TempDir::new().unwrap();
        let baseline = temp_dir.path().join("baseline");
        fs::create_dir_all(&baseline).unwrap();

        let checker = PreflightChecker::new_allowing_same_root(&baseline, vec![&baseline]);
        let report = checker.check_all().unwrap();

        assert!(report.is_ok());
        assert!(!report.has_path_conflicts);
    }

    #[test]
    fn test_preflight_nested_directory() {
        let temp_dir = TempDir::new().unwrap();
        let baseline = temp_dir.path().join("baseline");
        let nested = baseline.join("nested");

        fs::create_dir_all(&baseline).unwrap();
        fs::create_dir_all(&nested).unwrap();

        let checker = PreflightChecker::new(&baseline, vec![&nested]);
        let report = checker.check_all().unwrap();

        assert!(!report.is_ok());
        assert!(report.has_nesting_conflicts);
    }
}
