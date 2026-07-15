use crate::db::models::{AnalysisType, ComparisonStats};
use crate::db::repository::Repository;
use crate::error::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};

/// Phase 8: 报告生成器
/// 生成 JSON、CSV、HTML 格式的分析报告
pub struct ReportGenerator<'a> {
    repo: &'a Repository,
    run_id: String,
    output_dir: PathBuf,
}

impl<'a> ReportGenerator<'a> {
    pub fn new(repo: &'a Repository, run_id: String, output_dir: PathBuf) -> Self {
        Self {
            repo,
            run_id,
            output_dir,
        }
    }

    /// 生成完整报告（JSON + CSV + HTML）
    pub fn generate_all(&self) -> Result<GeneratedReports> {
        let json_path = self.generate_json_report()?;
        let csv_path = self.generate_csv_report()?;
        let html_path = self.generate_html_report()?;

        Ok(GeneratedReports {
            json_report: json_path,
            csv_export: csv_path,
            html_report: html_path,
        })
    }

    /// Phase 8.2: 生成 JSON 主报告
    pub fn generate_json_report(&self) -> Result<PathBuf> {
        let stats = self.repo.get_analysis_stats(&self.run_id)?;
        let results = self.repo.get_analysis_results_for_report(&self.run_id)?;
        let run = self.repo.get_run(&self.run_id)?.ok_or_else(|| {
            crate::error::AppError::Other(format!("Run not found: {}", self.run_id))
        })?;

        // 构建 JSON 报告
        let report = JsonReport {
            schema_version: "1".to_string(),
            run_id: self.run_id.clone(),
            generated_at: Utc::now().timestamp(),
            application_version: run.application_version,
            algorithm_profile_id: run.algorithm_profile_id,
            baseline_root: run.baseline_root_path,
            baseline_alias: run.baseline_root_alias,
            comparison_roots: serde_json::from_str(&run.comparison_root_paths)?,
            comparison_aliases: serde_json::from_str(&run.comparison_root_aliases)?,
            algorithm_config: AlgorithmConfig {
                phash_max_distance: run.phash_max_distance,
                compressed_ssim_threshold: run.compressed_ssim_threshold,
                variant_review_lower_bound: run.variant_review_lower_bound,
                aspect_ratio_tolerance: run.aspect_ratio_tolerance,
                primary_match_tie_threshold: run.primary_match_tie_threshold,
            },
            statistics: stats,
            results,
        };

        // 验证统计守恒
        self.verify_statistics(&report.statistics)?;

        // 写入文件
        let filename = format!("report_{}.json", self.run_id);
        let output_path = self.output_dir.join(&filename);
        let temp_path = self.output_dir.join(format!(".{}.tmp", filename));

        // 原子写入
        let json_content = serde_json::to_string_pretty(&report)?;
        std::fs::write(&temp_path, json_content)?;
        std::fs::rename(&temp_path, &output_path)?;

        Ok(output_path)
    }

    /// Phase 8.3: 生成 CSV 导出
    pub fn generate_csv_report(&self) -> Result<PathBuf> {
        let results = self.repo.get_analysis_results_for_report(&self.run_id)?;

        let filename = format!("export_{}.csv", self.run_id);
        let output_path = self.output_dir.join(&filename);
        let temp_path = self.output_dir.join(format!(".{}.tmp", filename));

        let mut wtr = csv::Writer::from_path(&temp_path)?;

        // CSV 头部
        wtr.write_record(&[
            "runId",
            "comparisonImageId",
            "comparisonPath",
            "analysisType",
            "primaryMatchImageId",
            "primaryMatchPath",
            "phashDistance",
            "ssimScore",
            "sizeRatio",
            "resolutionRatio",
            "aspectDiff",
            "directionSmallerResolution",
            "directionSmallerFilesize",
            "reviewStatus",
            "actionStatus",
            "algorithmProfileId",
        ])?;

        // 写入数据行
        for result in results {
            wtr.write_record(&[
                &self.run_id,
                &result.comparison_image_id.to_string(),
                &result.comparison_path,
                result.analysis_type.as_str(),
                &result
                    .primary_match_image_id
                    .map_or_else(|| String::new(), |id| id.to_string()),
                &result.primary_match_path.unwrap_or_default(),
                &result
                    .phash_distance
                    .map_or_else(|| String::new(), |v| v.to_string()),
                &result
                    .ssim_score
                    .map_or_else(|| String::new(), |v| format!("{:.6}", v)),
                &result
                    .size_ratio
                    .map_or_else(|| String::new(), |v| format!("{:.6}", v)),
                &result
                    .resolution_ratio
                    .map_or_else(|| String::new(), |v| format!("{:.6}", v)),
                &result
                    .aspect_diff
                    .map_or_else(|| String::new(), |v| format!("{:.6}", v)),
                &result.direction_smaller_resolution.to_string(),
                &result.direction_smaller_filesize.to_string(),
                &result.review_status.unwrap_or_else(|| "N/A".to_string()),
                &result.action_status.unwrap_or_else(|| "N/A".to_string()),
                &result.algorithm_profile_id,
            ])?;
        }

        wtr.flush()?;
        drop(wtr);

        // 原子替换
        std::fs::rename(&temp_path, &output_path)?;

        Ok(output_path)
    }

    /// Phase 8.3: 生成 HTML 报告
    pub fn generate_html_report(&self) -> Result<PathBuf> {
        let stats = self.repo.get_analysis_stats(&self.run_id)?;
        let results = self.repo.get_analysis_results_for_report(&self.run_id)?;
        let run = self.repo.get_run(&self.run_id)?.ok_or_else(|| {
            crate::error::AppError::Other(format!("Run not found: {}", self.run_id))
        })?;

        let filename = format!("report_{}.html", self.run_id);
        let output_path = self.output_dir.join(&filename);
        let temp_path = self.output_dir.join(format!(".{}.tmp", filename));

        let mut file = File::create(&temp_path)?;

        // 生成 HTML 内容
        self.write_html_header(&mut file, &run)?;
        self.write_html_statistics(&mut file, &stats)?;
        self.write_html_results_table(&mut file, &results)?;
        self.write_html_footer(&mut file)?;

        drop(file);

        // 原子替换
        std::fs::rename(&temp_path, &output_path)?;

        Ok(output_path)
    }

    /// 验证统计守恒公式
    fn verify_statistics(&self, stats: &ComparisonStats) -> Result<()> {
        let sum = stats.exact_duplicate
            + stats.likely_compressed
            + stats.variant
            + stats.similar_keep
            + stats.no_baseline_match
            + stats.inconclusive
            + stats.not_evaluated
            + stats.error;

        if sum != stats.comparison_total {
            return Err(crate::error::AppError::Other(format!(
                "统计守恒失败: sum={}, comparison_total={}",
                sum, stats.comparison_total
            ))
            .into());
        }

        Ok(())
    }

    // ========================================================================
    // HTML 生成辅助方法
    // ========================================================================

    fn write_html_header(&self, file: &mut File, run: &crate::db::models::Run) -> Result<()> {
        writeln!(file, "<!DOCTYPE html>")?;
        writeln!(file, "<html lang=\"zh-CN\">")?;
        writeln!(file, "<head>")?;
        writeln!(file, "  <meta charset=\"UTF-8\">")?;
        writeln!(
            file,
            "  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">"
        )?;
        writeln!(
            file,
            "  <title>ImageKeeper 分析报告 - {}</title>",
            self.run_id
        )?;
        writeln!(file, "  <style>")?;
        writeln!(file, "{}", include_str!("report_style.css"))?;
        writeln!(file, "  </style>")?;
        writeln!(file, "</head>")?;
        writeln!(file, "<body>")?;
        writeln!(file, "  <div class=\"container\">")?;
        writeln!(file, "    <h1>ImageKeeper 分析报告</h1>")?;
        writeln!(file, "    <div class=\"meta\">")?;
        writeln!(
            file,
            "      <p><strong>Run ID:</strong> {}</p>",
            self.run_id
        )?;
        writeln!(
            file,
            "      <p><strong>生成时间:</strong> {}</p>",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        )?;
        writeln!(
            file,
            "      <p><strong>应用版本:</strong> {}</p>",
            run.application_version
        )?;
        writeln!(
            file,
            "      <p><strong>算法配置:</strong> {}</p>",
            run.algorithm_profile_id
        )?;
        writeln!(
            file,
            "      <p><strong>基准目录:</strong> {} ({})</p>",
            run.baseline_root_alias, run.baseline_root_path
        )?;
        writeln!(file, "    </div>")?;
        Ok(())
    }

    fn write_html_statistics(&self, file: &mut File, stats: &ComparisonStats) -> Result<()> {
        writeln!(file, "    <h2>统计摘要</h2>")?;
        writeln!(file, "    <table>")?;
        writeln!(file, "      <tr><th>项目</th><th>数量</th></tr>")?;
        writeln!(
            file,
            "      <tr><td>基准图片总数</td><td>{}</td></tr>",
            stats.baseline_total
        )?;
        writeln!(
            file,
            "      <tr><td>对比图片总数</td><td>{}</td></tr>",
            stats.comparison_total
        )?;
        writeln!(
            file,
            "      <tr class=\"category\"><td>完全重复</td><td>{}</td></tr>",
            stats.exact_duplicate
        )?;
        writeln!(
            file,
            "      <tr class=\"category\"><td>疑似压缩</td><td>{}</td></tr>",
            stats.likely_compressed
        )?;
        writeln!(
            file,
            "      <tr class=\"category\"><td>变体</td><td>{}</td></tr>",
            stats.variant
        )?;
        writeln!(
            file,
            "      <tr class=\"category\"><td>相似但保留</td><td>{}</td></tr>",
            stats.similar_keep
        )?;
        writeln!(
            file,
            "      <tr class=\"category\"><td>无匹配</td><td>{}</td></tr>",
            stats.no_baseline_match
        )?;
        writeln!(
            file,
            "      <tr class=\"category\"><td>无法确定</td><td>{}</td></tr>",
            stats.inconclusive
        )?;
        writeln!(
            file,
            "      <tr class=\"category\"><td>未评估</td><td>{}</td></tr>",
            stats.not_evaluated
        )?;
        writeln!(
            file,
            "      <tr class=\"category\"><td>错误</td><td>{}</td></tr>",
            stats.error
        )?;
        writeln!(file, "      <tr><td colspan=\"2\"><hr></td></tr>")?;
        writeln!(
            file,
            "      <tr><td>待审核</td><td>{}</td></tr>",
            stats.pending_review
        )?;
        writeln!(
            file,
            "      <tr><td>已批准回收</td><td>{}</td></tr>",
            stats.approved_for_recycle
        )?;
        writeln!(
            file,
            "      <tr><td>拒绝保留</td><td>{}</td></tr>",
            stats.rejected_keep
        )?;
        writeln!(
            file,
            "      <tr><td>已回收</td><td>{}</td></tr>",
            stats.recycled
        )?;
        writeln!(
            file,
            "      <tr><td>已恢复</td><td>{}</td></tr>",
            stats.restored
        )?;
        writeln!(
            file,
            "      <tr><td>永久删除</td><td>{}</td></tr>",
            stats.permanently_deleted
        )?;
        writeln!(file, "    </table>")?;
        Ok(())
    }

    fn write_html_results_table(&self, file: &mut File, results: &[ReportResultRow]) -> Result<()> {
        writeln!(file, "    <h2>分析结果明细（前 100 条）</h2>")?;
        writeln!(file, "    <table>")?;
        writeln!(file, "      <tr>")?;
        writeln!(file, "        <th>对比图片</th>")?;
        writeln!(file, "        <th>分类</th>")?;
        writeln!(file, "        <th>主匹配</th>")?;
        writeln!(file, "        <th>pHash</th>")?;
        writeln!(file, "        <th>SSIM</th>")?;
        writeln!(file, "        <th>审核状态</th>")?;
        writeln!(file, "      </tr>")?;

        for result in results.iter().take(100) {
            writeln!(file, "      <tr>")?;
            writeln!(
                file,
                "        <td title=\"{}\">{}</td>",
                result.comparison_path,
                Path::new(&result.comparison_path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            )?;
            writeln!(
                file,
                "        <td>{}</td>",
                self.analysis_type_display(&result.analysis_type)
            )?;
            writeln!(
                file,
                "        <td>{}</td>",
                result
                    .primary_match_path
                    .as_ref()
                    .and_then(|p| Path::new(p).file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "N/A".to_string())
            )?;
            writeln!(
                file,
                "        <td>{}</td>",
                result
                    .phash_distance
                    .map_or_else(|| "N/A".to_string(), |v| v.to_string())
            )?;
            writeln!(
                file,
                "        <td>{}</td>",
                result
                    .ssim_score
                    .map_or_else(|| "N/A".to_string(), |v| format!("{:.4}", v))
            )?;
            writeln!(
                file,
                "        <td>{}</td>",
                result.review_status.as_deref().unwrap_or("N/A")
            )?;
            writeln!(file, "      </tr>")?;
        }

        writeln!(file, "    </table>")?;

        if results.len() > 100 {
            writeln!(file, "    <p class=\"note\">注：仅显示前 100 条结果，完整数据请查看 JSON 或 CSV 报告。</p>")?;
        }

        Ok(())
    }

    fn write_html_footer(&self, file: &mut File) -> Result<()> {
        writeln!(file, "  </div>")?;
        writeln!(file, "</body>")?;
        writeln!(file, "</html>")?;
        Ok(())
    }

    fn analysis_type_display(&self, analysis_type: &AnalysisType) -> &str {
        match analysis_type {
            AnalysisType::ExactDuplicate => "完全重复",
            AnalysisType::LikelyCompressed => "疑似压缩",
            AnalysisType::Variant => "变体",
            AnalysisType::SimilarKeep => "相似但保留",
            AnalysisType::NoBaselineMatch => "无匹配",
            AnalysisType::Inconclusive => "无法确定",
            AnalysisType::NotEvaluated => "未评估",
            AnalysisType::Error => "错误",
        }
    }
}

// ========================================================================
// 数据结构定义
// ========================================================================

/// 生成的报告路径
#[derive(Debug, Serialize)]
pub struct GeneratedReports {
    pub json_report: PathBuf,
    pub csv_export: PathBuf,
    pub html_report: PathBuf,
}

/// JSON 主报告结构
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonReport {
    pub schema_version: String,
    pub run_id: String,
    pub generated_at: i64,
    pub application_version: String,
    pub algorithm_profile_id: String,
    pub baseline_root: String,
    pub baseline_alias: String,
    pub comparison_roots: Vec<String>,
    pub comparison_aliases: Vec<String>,
    pub algorithm_config: AlgorithmConfig,
    pub statistics: ComparisonStats,
    pub results: Vec<ReportResultRow>,
}

/// 算法配置
#[derive(Debug, Serialize, Deserialize)]
pub struct AlgorithmConfig {
    pub phash_max_distance: i32,
    pub compressed_ssim_threshold: f64,
    pub variant_review_lower_bound: f64,
    pub aspect_ratio_tolerance: f64,
    pub primary_match_tie_threshold: f64,
}

/// 报告结果行（扩展字段用于报告展示）
#[derive(Debug, Serialize, Deserialize)]
pub struct ReportResultRow {
    pub comparison_image_id: i64,
    pub comparison_path: String,
    pub analysis_type: AnalysisType,
    pub primary_match_image_id: Option<i64>,
    pub primary_match_path: Option<String>,
    pub phash_distance: Option<i32>,
    pub ssim_score: Option<f64>,
    pub size_ratio: Option<f64>,
    pub resolution_ratio: Option<f64>,
    pub aspect_diff: Option<f64>,
    pub direction_smaller_resolution: bool,
    pub direction_smaller_filesize: bool,
    pub algorithm_profile_id: String,
    pub review_status: Option<String>,
    pub action_status: Option<String>,
}
