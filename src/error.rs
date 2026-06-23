use thiserror::Error;

/// 项目统一错误类型
#[derive(Error, Debug)]
pub enum KbError {
    /// IO 错误（文件读写、目录访问等）
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// 文档解析错误
    #[error("Parse error: {0}")]
    Parse(String),

    /// 索引构建错误
    #[error("Index error: {0}")]
    Index(String),

    /// 搜索错误
    #[error("Search error: {0}")]
    Search(String),

    /// 存储错误（序列化/反序列化失败等）
    #[error("Storage error: {0}")]
    Storage(String),

    /// 索引文件不存在
    #[error("No index found at {0}, please run `index` command first")]
    NoIndex(String),
}

/// 项目统一 Result 类型别名
pub type Result<T> = std::result::Result<T, KbError>;
