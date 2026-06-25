use std::collections::HashMap;

use crate::error::Result;
use crate::indexer::{DocumentMeta, InvertedIndex};
use crate::parser::{DocId, ParserRegistry};
use crate::tokenizer::Tokenizer;

/// 搜索结果条目
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub doc: DocumentMeta,
    pub score: f64,
    /// 匹配词附近的文本片段
    pub snippet: String,
}

/// 搜索引擎 trait
pub trait SearchEngine {
    /// 搜索查询词，返回按相关度排序的结果列表
    fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>>;
}

/// 基于倒排索引的搜索引擎
///
/// 借用 InvertedIndex、Tokenizer 和 ParserRegistry 的引用，避免数据拷贝
pub struct IndexSearcher<'a> {
    index: &'a InvertedIndex,
    tokenizer: &'a dyn Tokenizer,
    registry: &'a ParserRegistry,
}

impl<'a> IndexSearcher<'a> {
    pub fn new(
        index: &'a InvertedIndex,
        tokenizer: &'a dyn Tokenizer,
        registry: &'a ParserRegistry,
    ) -> Self {
        Self {
            index,
            tokenizer,
            registry,
        }
    }

    /// 生成匹配词附近的文本片段
    ///
    /// 从文档内容中截取匹配词前后约 50 个字符的上下文，
    /// 统一换行为空格、压缩连续空格，且不会在单词中间截断
    fn generate_snippet(&self, content: &str, query_tokens: &[String]) -> String {
        // 将换行统一为空格，压缩连续空格
        let normalized: String = content
            .split(['\n', '\r'])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        let normalized_lower = normalized.to_lowercase();

        // 找到第一个匹配词在规范化文本中的位置
        let mut best_pos = 0;
        for token in query_tokens {
            if let Some(pos) = normalized_lower.find(token.as_str()) {
                best_pos = pos;
                break;
            }
        }

        let before_len = 50;
        let after_len = 100;

        // 计算截取范围
        let mut start = best_pos.saturating_sub(before_len);
        let mut end = (best_pos + after_len).min(normalized.len());

        // 确保起止位置在合法的 UTF-8 字符边界上
        while start < normalized.len() && !normalized.is_char_boundary(start) {
            start += 1;
        }
        while end > 0 && !normalized.is_char_boundary(end) {
            end -= 1;
        }

        // 在有限范围内查找词边界（空格），避免在无空格的中文文本中跳太远
        let max_boundary_search = 10;

        if start > 0 {
            if let Some(space_pos) = normalized[start..]
                .char_indices()
                .take_while(|(i, _)| *i <= max_boundary_search)
                .find(|(_, c)| *c == ' ')
                .map(|(i, _)| i)
            {
                start += space_pos;
            }
        }

        if end < normalized.len() {
            let mut search_start = end.saturating_sub(max_boundary_search);
            while search_start > start && !normalized.is_char_boundary(search_start) {
                search_start -= 1;
            }
            if search_start >= start {
                if let Some(space_pos) = normalized[search_start..end]
                    .char_indices()
                    .find(|(_, c)| *c == ' ')
                    .map(|(i, _)| search_start + i)
                {
                    end = space_pos;
                }
            }
        }

        let snippet = normalized[start..end].trim();

        let prefix = if start > 0 { "... " } else { "" };
        let suffix = if end < normalized.len() { "..." } else { "" };

        format!("{}{}{}", prefix, snippet, suffix)
    }
}

impl<'a> SearchEngine for IndexSearcher<'a> {
    fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let query_tokens = self.tokenizer.tokenize(query);

        if query_tokens.is_empty() {
            return Ok(Vec::new());
        }

        // 对每个查询 token，收集匹配文档并累加 TF-IDF 分数
        let mut doc_scores: HashMap<DocId, f64> = HashMap::new();

        for token in &query_tokens {
            if let Some(posting_list) = self.index.postings.get(token) {
                for posting in posting_list {
                    let score = self.index.tfidf(token, posting.doc_id);
                    *doc_scores.entry(posting.doc_id).or_default() += score;
                }
            }
        }

        // 按分数降序排序
        let mut scored_docs: Vec<(DocId, f64)> = doc_scores.into_iter().collect();
        scored_docs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // 取 top N 结果
        let results: Vec<SearchResult> = scored_docs
            .into_iter()
            .take(limit)
            .filter_map(|(doc_id, score)| {
                self.index.documents.get(&doc_id).map(|doc_meta| {
                    // 通过解析器读取文档内容生成 snippet，支持所有文件格式
                    let snippet = match self
                        .registry
                        .parse_file(std::path::Path::new(&doc_meta.path))
                    {
                        Ok(doc) => self.generate_snippet(&doc.content, &query_tokens),
                        Err(_) => doc_meta.title.clone(),
                    };

                    SearchResult {
                        doc: doc_meta.clone(),
                        score,
                        snippet,
                    }
                })
            })
            .collect();

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Document;
    use crate::tokenizer::MixedTokenizer;
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
    fn test_search_single_term() {
        let tokenizer = MixedTokenizer::new();
        let docs = vec![
            make_doc(1, "Rust programming language"),
            make_doc(2, "Python programming language"),
            make_doc(3, "Cooking recipes"),
        ];

        let index = InvertedIndex::build_from_documents(&docs, &tokenizer);
        let registry = ParserRegistry::with_defaults();
        let searcher = IndexSearcher::new(&index, &tokenizer, &registry);

        let results = searcher.search("rust", 10).unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].doc.id, 1);
        assert!(results[0].score > 0.0);
    }

    #[test]
    fn test_search_multiple_terms() {
        let tokenizer = MixedTokenizer::new();
        let docs = vec![
            make_doc(1, "Rust programming language"),
            make_doc(2, "Rust is safe and fast"),
            make_doc(3, "Python programming"),
        ];

        let index = InvertedIndex::build_from_documents(&docs, &tokenizer);
        let registry = ParserRegistry::with_defaults();
        let searcher = IndexSearcher::new(&index, &tokenizer, &registry);

        let results = searcher.search("rust programming", 10).unwrap();

        // doc1 同时包含 rust 和 programming，分数应最高
        assert!(!results.is_empty());
        assert_eq!(results[0].doc.id, 1);
    }

    #[test]
    fn test_search_no_results() {
        let tokenizer = MixedTokenizer::new();
        let docs = vec![make_doc(1, "Hello world")];

        let index = InvertedIndex::build_from_documents(&docs, &tokenizer);
        let registry = ParserRegistry::with_defaults();
        let searcher = IndexSearcher::new(&index, &tokenizer, &registry);

        let results = searcher.search("quantum", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_empty_query() {
        let tokenizer = MixedTokenizer::new();
        let docs = vec![make_doc(1, "Hello world")];

        let index = InvertedIndex::build_from_documents(&docs, &tokenizer);
        let registry = ParserRegistry::with_defaults();
        let searcher = IndexSearcher::new(&index, &tokenizer, &registry);

        let results = searcher.search("", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_limit() {
        let tokenizer = MixedTokenizer::new();
        let docs: Vec<Document> = (1..=5)
            .map(|i| make_doc(i, &format!("rust programming doc number {}", i)))
            .collect();

        let index = InvertedIndex::build_from_documents(&docs, &tokenizer);
        let registry = ParserRegistry::with_defaults();
        let searcher = IndexSearcher::new(&index, &tokenizer, &registry);

        let results = searcher.search("rust", 2).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_ranking() {
        let tokenizer = MixedTokenizer::new();
        let docs = vec![
            make_doc(1, "rust rust rust programming"),
            make_doc(2, "rust programming"),
            make_doc(3, "programming language"),
        ];

        let index = InvertedIndex::build_from_documents(&docs, &tokenizer);
        let registry = ParserRegistry::with_defaults();
        let searcher = IndexSearcher::new(&index, &tokenizer, &registry);

        let results = searcher.search("rust", 10).unwrap();

        // doc1 的 "rust" 词频最高，应排第一
        assert_eq!(results[0].doc.id, 1);
        // doc3 不包含 "rust"，不应出现
        assert!(!results.iter().any(|r| r.doc.id == 3));
    }

    #[test]
    fn test_search_chinese() {
        let tokenizer = MixedTokenizer::new();
        let docs = vec![
            make_doc(1, "Rust语言编程入门"),
            make_doc(2, "Python编程指南"),
            make_doc(3, "搜索引擎优化"),
        ];

        let index = InvertedIndex::build_from_documents(&docs, &tokenizer);
        let registry = ParserRegistry::with_defaults();
        let searcher = IndexSearcher::new(&index, &tokenizer, &registry);

        let results = searcher.search("语言", 10).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].doc.id, 1);
    }
}
