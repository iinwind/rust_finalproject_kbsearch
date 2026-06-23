use std::path::Path;

use crate::error::{KbError, Result};
use crate::indexer::InvertedIndex;

/// 索引存储 trait
///
/// 抽象索引的序列化/反序列化，便于后续扩展其他存储格式
pub trait IndexStorage {
    /// 将索引保存到指定路径
    fn save(&self, index: &InvertedIndex, path: &Path) -> Result<()>;

    /// 从指定路径加载索引
    fn load(&self, path: &Path) -> Result<InvertedIndex>;
}

/// JSON 格式的索引存储
pub struct JsonStorage;

impl JsonStorage {
    pub fn new() -> Self {
        Self
    }
}

impl Default for JsonStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl IndexStorage for JsonStorage {
    fn save(&self, index: &InvertedIndex, path: &Path) -> Result<()> {
        // 确保父目录存在
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(index)
            .map_err(|e| KbError::Storage(format!("Failed to serialize index: {}", e)))?;

        std::fs::write(path, json)?;
        Ok(())
    }

    fn load(&self, path: &Path) -> Result<InvertedIndex> {
        if !path.exists() {
            return Err(KbError::NoIndex(path.to_string_lossy().to_string()));
        }

        let json = std::fs::read_to_string(path)?;
        let index: InvertedIndex = serde_json::from_str(&json)
            .map_err(|e| KbError::Storage(format!("Failed to deserialize index: {}", e)))?;

        Ok(index)
    }
}

/// 获取默认索引文件路径
///
/// 默认位置：`~/.kbsearch/index.json`
pub fn default_index_path() -> std::path::PathBuf {
    let home = dirs_home();
    home.join(".kbsearch").join("index.json")
}

/// 获取用户主目录
fn dirs_home() -> std::path::PathBuf {
    // 优先使用 HOME 环境变量（Unix 风格）
    if let Ok(home) = std::env::var("HOME") {
        return std::path::PathBuf::from(home);
    }
    // Windows: 使用 USERPROFILE
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        return std::path::PathBuf::from(userprofile);
    }
    // 兜底：当前目录
    std::path::PathBuf::from(".")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexer::SimpleTokenizer;
    use crate::parser::Document;
    use std::path::PathBuf;

    fn make_test_index() -> InvertedIndex {
        let tokenizer = SimpleTokenizer::new();
        let docs = vec![
            Document {
                id: 1,
                path: PathBuf::from("/test/doc1.txt"),
                title: "doc1".to_string(),
                content: "Rust programming language".to_string(),
            },
            Document {
                id: 2,
                path: PathBuf::from("/test/doc2.txt"),
                title: "doc2".to_string(),
                content: "Python programming language".to_string(),
            },
        ];

        InvertedIndex::build_from_documents(&docs, &tokenizer)
    }

    #[test]
    fn test_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let index_path = dir.path().join("test_index.json");

        let original_index = make_test_index();
        let storage = JsonStorage::new();

        // 保存
        storage.save(&original_index, &index_path).unwrap();
        assert!(index_path.exists());

        // 加载
        let loaded_index = storage.load(&index_path).unwrap();

        // 验证数据一致性
        assert_eq!(loaded_index.doc_count, original_index.doc_count);
        assert_eq!(loaded_index.postings.len(), original_index.postings.len());
        assert_eq!(loaded_index.documents.len(), original_index.documents.len());
    }

    #[test]
    fn test_load_nonexistent() {
        let storage = JsonStorage::new();
        let result = storage.load(Path::new("/nonexistent/index.json"));

        assert!(result.is_err());
        match result.unwrap_err() {
            KbError::NoIndex(_) => {}
            other => panic!("Expected NoIndex error, got: {:?}", other),
        }
    }

    #[test]
    fn test_save_creates_parent_dir() {
        let dir = tempfile::tempdir().unwrap();
        let index_path = dir.path().join("nested").join("dir").join("index.json");

        let index = make_test_index();
        let storage = JsonStorage::new();

        storage.save(&index, &index_path).unwrap();
        assert!(index_path.exists());
    }

    #[test]
    fn test_default_index_path() {
        let path = default_index_path();
        assert!(path.to_string_lossy().contains(".kbsearch"));
        assert!(path.to_string_lossy().contains("index.json"));
    }
}
