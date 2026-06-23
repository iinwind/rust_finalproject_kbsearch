use kbsearch::error::Result;
use kbsearch::indexer::{InvertedIndex, SimpleTokenizer};
use kbsearch::parser::{generate_doc_id, ParserRegistry};
use kbsearch::scanner::scan_directory;
use kbsearch::searcher::{IndexSearcher, SearchEngine};
use kbsearch::storage::{IndexStorage, JsonStorage};
use std::fs;

/// 端到端集成测试：扫描 → 解析 → 索引 → 搜索
#[test]
fn test_end_to_end_workflow() -> Result<()> {
    // 1. 创建临时目录和测试文件
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();

    fs::write(base.join("rust_intro.md"), "# Rust Introduction\n\nRust is a systems programming language focused on safety, speed, and concurrency.").unwrap();
    fs::write(
        base.join("python_notes.txt"),
        "Python is a high-level programming language known for its readability and simplicity.",
    )
    .unwrap();
    fs::write(
        base.join("rust_advanced.md"),
        "# Advanced Rust\n\nOwnership and borrowing are core concepts in Rust programming.",
    )
    .unwrap();
    fs::write(base.join("ignore_me.csv"), "this,should,be,ignored").unwrap();

    // 2. 扫描
    let files = scan_directory(base)?;
    assert_eq!(files.len(), 3); // 2 md + 1 txt, csv 被忽略

    // 3. 解析
    let registry = ParserRegistry::with_defaults();
    let mut documents = Vec::new();
    for file in &files {
        let doc = registry.parse_file(&file.path)?;
        documents.push(doc);
    }
    assert_eq!(documents.len(), 3);

    // 4. 构建索引
    let tokenizer = SimpleTokenizer::new();
    let index = InvertedIndex::build_from_documents(&documents, &tokenizer);
    assert_eq!(index.doc_count, 3);

    // 5. 搜索 "rust"
    let searcher = IndexSearcher::new(&index, &tokenizer, &registry);
    let results = searcher.search("rust", 10)?;

    assert!(!results.is_empty());
    // "rust" 应在 rust_intro.md 和 rust_advanced.md 中
    assert!(results.len() >= 2);

    // 6. 搜索 "programming"
    let results = searcher.search("programming", 10)?;
    assert!(!results.is_empty());
    // "programming" 出现在所有三个文档中
    assert!(results.len() >= 2);

    // 7. 搜索不存在的词
    let results = searcher.search("quantum", 10)?;
    assert!(results.is_empty());

    Ok(())
}

/// 测试索引持久化：保存 → 加载 → 搜索
#[test]
fn test_index_persistence() -> Result<()> {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();

    fs::write(base.join("doc1.txt"), "Rust programming language").unwrap();
    fs::write(base.join("doc2.txt"), "Python programming language").unwrap();

    // 构建索引
    let registry = ParserRegistry::with_defaults();
    let files = scan_directory(base)?;
    let documents: Vec<_> = files
        .iter()
        .filter_map(|f| registry.parse_file(&f.path).ok())
        .collect();

    let tokenizer = SimpleTokenizer::new();
    let index = InvertedIndex::build_from_documents(&documents, &tokenizer);

    // 保存
    let storage = JsonStorage::new();
    let index_path = dir.path().join("saved_index.json");
    storage.save(&index, &index_path)?;

    // 加载
    let loaded_index = storage.load(&index_path)?;

    // 用加载的索引搜索
    let registry = ParserRegistry::with_defaults();
    let searcher = IndexSearcher::new(&loaded_index, &tokenizer, &registry);
    let results = searcher.search("rust", 10)?;

    assert!(!results.is_empty());
    assert!(results[0].doc.title.contains("doc1"));

    Ok(())
}

/// 测试 Markdown 解析正确性
#[test]
fn test_markdown_parsing_in_workflow() -> Result<()> {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("test.md");

    fs::write(
        &file_path,
        "# Title\n\nParagraph with **bold** and *italic*.\n\n- Item 1\n- Item 2\n\nCode: `hello`",
    )
    .unwrap();

    let registry = ParserRegistry::with_defaults();
    let doc = registry.parse_file(&file_path)?;

    // 纯文本应包含所有内容但不含 Markdown 标记
    assert!(doc.content.contains("Title"));
    assert!(doc.content.contains("bold"));
    assert!(doc.content.contains("italic"));
    assert!(doc.content.contains("Item 1"));
    assert!(doc.content.contains("hello"));
    assert!(!doc.content.contains("**"));
    assert!(!doc.content.contains("*"));

    Ok(())
}

/// 测试 TF-IDF 排序正确性
#[test]
fn test_tfidf_ranking() -> Result<()> {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();

    // doc1 大量提到 rust，doc2 只提一次
    fs::write(
        base.join("rust_heavy.txt"),
        "rust rust rust rust rust programming",
    )
    .unwrap();
    fs::write(base.join("rust_light.txt"), "python programming rust").unwrap();

    let registry = ParserRegistry::with_defaults();
    let files = scan_directory(base)?;
    let documents: Vec<_> = files
        .iter()
        .filter_map(|f| registry.parse_file(&f.path).ok())
        .collect();

    let tokenizer = SimpleTokenizer::new();
    let index = InvertedIndex::build_from_documents(&documents, &tokenizer);
    let searcher = IndexSearcher::new(&index, &tokenizer, &registry);

    let results = searcher.search("rust", 10)?;
    assert!(!results.is_empty());

    // rust_heavy 应排第一（TF 更高）
    assert!(results[0].doc.title.contains("rust_heavy"));

    Ok(())
}

/// 测试 doc_id 稳定性
#[test]
fn test_doc_id_stability() {
    let path1 = std::path::Path::new("/test/readme.md");
    let path2 = std::path::Path::new("/test/notes.txt");

    let id1a = generate_doc_id(path1);
    let id1b = generate_doc_id(path1);
    let id2 = generate_doc_id(path2);

    // 同一路径生成相同 ID
    assert_eq!(id1a, id1b);
    // 不同路径生成不同 ID
    assert_ne!(id1a, id2);
}
