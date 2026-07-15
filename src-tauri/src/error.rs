use thiserror::Error;

/// 应用错误类型
#[derive(Error, Debug)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("IO错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("图片处理错误: {0}")]
    Image(#[from] image::ImageError),

    #[error("序列化错误: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("路径错误: 无效的路径")]
    InvalidPath,

    #[error("扫描任务不存在: {0}")]
    ScanNotFound(i64),

    #[error("图片不存在: {0}")]
    ImageNotFound(i64),

    #[error("资源不存在: {0}")]
    NotFound(String),

    #[error("不支持的图片格式: {0}")]
    UnsupportedFormat(String),

    #[error("哈希计算失败: {0}")]
    HashComputation(String),

    #[error("SSIM计算失败: {0}")]
    SsimComputation(String),

    #[error("文件系统操作失败: {0}")]
    FileSystem(String),

    #[error("验证错误: {0}")]
    ValidationError(String),

    #[error("内部错误: {0}")]
    Internal(String),

    #[error("CSV 错误: {0}")]
    Csv(#[from] csv::Error),

    #[error("其他错误: {0}")]
    Other(String),
}

/// 统一结果类型
pub type Result<T> = std::result::Result<T, AppError>;

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}
