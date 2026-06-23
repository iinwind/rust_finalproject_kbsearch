use std::collections::HashMap;

use crate::error::Result;
use crate::indexer::{DocumentMeta, InvertedIndex, Tokenizer};
use crate::parser::{DocId, ParserRegistry};

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

        // 计算截取范围，避免截断单词
        let mut start = best_pos.saturating_sub(before_len);
        let mut end = (best_pos + after_len).min(normalized.len());

        // 如果 start 不在边界，向右推到最近的空格处，避免截断左侧单词
        if start > 0 {
            if let Some(space_pos) = normalized[start..].find(' ') {
                start += space_pos;
            }
        }

        // 如果 end 不在边界，向左退到最近的空格处，避免截断右侧单词
        if end < normalized.len() {
            if let Some(space_pos) = normalized[..end].rfind(' ') {
                end = space_pos;
            }
        }

        let snippet = normalized[start..end].trim();

        // 在摘要中高亮标记匹配的关键词
        let highlighted = self.highlight_keywords(snippet, query_tokens);

        let prefix = if start > 0 { "..." } else { "" };
        let suffix = if end < normalized.len() { "..." } else { "" };

        format!(
            "{}{}{}{}",
            prefix,
            if start > 0 { " " } else { "" },
            highlighted,
            suffix
        )
    }

    /// 在摘要文本中用 ANSI 粗体+黄色标记匹配的关键词
    fn highlight_keywords(&self, text: &str, query_tokens: &[String]) -> String {
        let mut result = text.to_string();
        for token in query_tokens {
            // 大小写不敏感替换，保留原文大小写
            let mut new = String::with_capacity(result.len());
            let mut last_end = 0;
            let lower = result.to_lowercase();
            while let Some(pos) = lower[last_end..].find(token.as_str()) {
                let abs_pos = last_end + pos;
                new.push_str(&result[last_end..abs_pos]);
                // ANSI: \x1b[1;33m 粗体黄色, \x1b[0m 重置
                new.push_str("\x1b[1;33m");
                new.push_str(&result[abs_pos..abs_pos + token.len()]);
                new.push_str("\x1b[0m");
                last_end = abs_pos + token.len();
            }
            new.push_str(&result[last_end..]);
            result = new;
        }
        result
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
    use crate::indexer::SimpleTokenizer;
    use crate::parser::Document;
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
        let tokenizer = SimpleTokenizer::new();
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
        let tokenizer = SimpleTokenizer::new();
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
        let tokenizer = SimpleTokenizer::new();
        let docs = vec![make_doc(1, "Hello world")];

        let index = InvertedIndex::build_from_documents(&docs, &tokenizer);
        let registry = ParserRegistry::with_defaults();
        let searcher = IndexSearcher::new(&index, &tokenizer, &registry);

        let results = searcher.search("quantum", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_empty_query() {
        let tokenizer = SimpleTokenizer::new();
        let docs = vec![make_doc(1, "Hello world")];

        let index = InvertedIndex::build_from_documents(&docs, &tokenizer);
        let registry = ParserRegistry::with_defaults();
        let searcher = IndexSearcher::new(&index, &tokenizer, &registry);

        let results = searcher.search("", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_limit() {
        let tokenizer = SimpleTokenizer::new();
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
        let tokenizer = SimpleTokenizer::new();
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
}
