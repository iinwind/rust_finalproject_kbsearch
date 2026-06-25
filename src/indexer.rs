use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::parser::{DocId, Document};
pub use crate::tokenizer::{SimpleTokenizer, Tokenizer};

/// 词项在文档中的位置信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Posting {
    pub doc_id: DocId,
    pub term_freq: usize,
    /// 词在分词结果中的位置下标（可用于后续高亮）
    pub positions: Vec<usize>,
}

/// 文档元信息（不存储全文，节省序列化空间）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMeta {
    pub id: DocId,
    pub path: String,
    pub title: String,
    /// 文档总词数，用于 TF 归一化
    pub doc_length: usize,
}

/// 索引统计信息
#[derive(Debug, Clone)]
pub struct IndexStats {
    pub doc_count: usize,
    pub vocab_size: usize,
    pub total_postings: usize,
}

/// 倒排索引
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvertedIndex {
    /// token → 倒排列表
    pub postings: HashMap<String, Vec<Posting>>,
    /// doc_id → 文档元信息
    pub documents: HashMap<DocId, DocumentMeta>,
    /// 文档总数
    pub doc_count: usize,
}

impl InvertedIndex {
    /// 创建空索引
    pub fn new() -> Self {
        Self {
            postings: HashMap::new(),
            documents: HashMap::new(),
            doc_count: 0,
        }
    }

    /// 将一个文档加入索引
    pub fn add_document(&mut self, doc: &Document, tokenizer: &dyn Tokenizer) {
        let tokens = tokenizer.tokenize(&doc.content);
        let doc_length = tokens.len();

        // 记录文档元信息
        self.documents.insert(
            doc.id,
            DocumentMeta {
                id: doc.id,
                path: doc.path.to_string_lossy().to_string(),
                title: doc.title.clone(),
                doc_length,
            },
        );
        self.doc_count = self.documents.len();

        // 构建 token → (位置列表) 的映射
        let mut token_positions: HashMap<String, Vec<usize>> = HashMap::new();
        for (pos, token) in tokens.iter().enumerate() {
            token_positions.entry(token.clone()).or_default().push(pos);
        }

        // 更新倒排列表
        for (token, positions) in token_positions {
            let posting = Posting {
                doc_id: doc.id,
                term_freq: positions.len(),
                positions,
            };

            self.postings.entry(token).or_default().push(posting);
        }
    }

    /// 批量构建索引
    pub fn build_from_documents(docs: &[Document], tokenizer: &dyn Tokenizer) -> Self {
        let mut index = Self::new();
        for doc in docs {
            index.add_document(doc, tokenizer);
        }
        index
    }

    /// 计算 TF-IDF 分数
    ///
    /// TF = term_freq / doc_length （归一化词频）
    /// IDF = 1 + ln(doc_count / (1 + doc_freq)) （平滑 IDF）
    pub fn tfidf(&self, token: &str, doc_id: DocId) -> f64 {
        let posting_list = match self.postings.get(token) {
            Some(list) => list,
            None => return 0.0,
        };

        let posting = match posting_list.iter().find(|p| p.doc_id == doc_id) {
            Some(p) => p,
            None => return 0.0,
        };

        let doc_meta = match self.documents.get(&doc_id) {
            Some(m) => m,
            None => return 0.0,
        };

        // TF: 归一化词频
        let tf = posting.term_freq as f64 / doc_meta.doc_length.max(1) as f64;

        // IDF: 平滑逆文档频率
        let doc_freq = posting_list.len() as f64;
        let idf = 1.0 + (self.doc_count as f64 / (1.0 + doc_freq)).ln();

        tf * idf
    }

    /// 获取索引统计信息
    pub fn stats(&self) -> IndexStats {
        let total_postings: usize = self.postings.values().map(|list| list.len()).sum();

        IndexStats {
            doc_count: self.doc_count,
            vocab_size: self.postings.len(),
            total_postings,
        }
    }
}

impl Default for InvertedIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_doc(id: DocId, content: &str) -> Document {
        Document {
            id,
            path: PathBuf::from(format!("/test/doc_{}.txt", id)),
            title: format!("doc_{}", id),
            content: content.to_string(),
        }
    }

    #[test]
    fn test_simple_tokenizer() {
        let tokenizer = SimpleTokenizer::new();

        let tokens = tokenizer.tokenize("Hello, World! This is a test.");
        assert!(tokens.contains(&"hello".to_string()));
        assert!(tokens.contains(&"world".to_string()));
        assert!(tokens.contains(&"test".to_string()));
        // 停用词应被过滤
        assert!(!tokens.contains(&"this".to_string()));
        assert!(!tokens.contains(&"is".to_string()));
        assert!(!tokens.contains(&"a".to_string()));
    }

    #[test]
    fn test_tokenizer_single_char_filtered() {
        let tokenizer = SimpleTokenizer::new();
        let tokens = tokenizer.tokenize("I am a person");
        // 单字符词应被过滤
        assert!(!tokens.iter().any(|t| t.len() == 1));
    }

    #[test]
    fn test_add_document() {
        let tokenizer = SimpleTokenizer::new();
        let mut index = InvertedIndex::new();

        let doc = make_doc(1, "Rust programming language");
        index.add_document(&doc, &tokenizer);

        assert_eq!(index.doc_count, 1);
        assert!(index.postings.contains_key("rust"));
        assert!(index.postings.contains_key("programming"));
        assert!(index.postings.contains_key("language"));
    }

    #[test]
    fn test_build_from_documents() {
        let tokenizer = SimpleTokenizer::new();
        let docs = vec![
            make_doc(1, "Rust is a programming language"),
            make_doc(2, "Python is another programming language"),
        ];

        let index = InvertedIndex::build_from_documents(&docs, &tokenizer);

        assert_eq!(index.doc_count, 2);
        // "programming" 出现在两个文档中
        assert_eq!(index.postings.get("programming").unwrap().len(), 2);
    }

    #[test]
    fn test_tfidf_calculation() {
        let tokenizer = SimpleTokenizer::new();
        let docs = vec![
            make_doc(1, "rust rust rust programming"),
            make_doc(2, "python programming language"),
            make_doc(3, "cooking recipes food"),
        ];

        let index = InvertedIndex::build_from_documents(&docs, &tokenizer);

        // "rust" 只在 doc1 出现，TF 较高，IDF 也较高
        let score_rust_doc1 = index.tfidf("rust", 1);
        assert!(score_rust_doc1 > 0.0);

        // "programming" 在 doc1 和 doc2 出现，IDF 较低
        let score_prog_doc1 = index.tfidf("programming", 1);
        assert!(score_prog_doc1 > 0.0);
        assert!(score_prog_doc1 < score_rust_doc1);

        // 不存在的词
        let score_missing = index.tfidf("nonexistent", 1);
        assert_eq!(score_missing, 0.0);
    }

    #[test]
    fn test_index_stats() {
        let tokenizer = SimpleTokenizer::new();
        let docs = vec![make_doc(1, "hello world"), make_doc(2, "hello rust")];

        let index = InvertedIndex::build_from_documents(&docs, &tokenizer);
        let stats = index.stats();

        assert_eq!(stats.doc_count, 2);
        assert!(stats.vocab_size > 0);
    }

    #[test]
    fn test_posting_positions() {
        let tokenizer = SimpleTokenizer::new();
        let mut index = InvertedIndex::new();

        let doc = make_doc(1, "rust programming rust language");
        index.add_document(&doc, &tokenizer);

        let posting = index.postings.get("rust").unwrap().first().unwrap();
        assert_eq!(posting.term_freq, 2);
        // "rust" 出现在位置 0 和 2
        assert_eq!(posting.positions, vec![0, 2]);
    }
}
