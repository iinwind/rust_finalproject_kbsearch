use kbsearch::error::Result;
use kbsearch::indexer::InvertedIndex;
use kbsearch::parser::ParserRegistry;
use kbsearch::scanner::scan_directory;
use kbsearch::searcher::{IndexSearcher, SearchEngine};
use kbsearch::storage::{IndexStorage, JsonStorage};
use kbsearch::tokenizer::MixedTokenizer;
use std::fs;
use std::io::Write;

// 辅助函数
/// 构造最小合法 DOCX 文件
fn create_minimal_docx(path: &std::path::Path, text: &str) {
    let content_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r>
        <w:t>{text}</w:t>
      </w:r>
    </w:p>
  </w:body>
</w:document>"#
    );

    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("word/document.xml", options).unwrap();
    zip.write_all(content_xml.as_bytes()).unwrap();

    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#,
    ).unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
    ).unwrap();

    zip.start_file("word/_rels/document.xml.rels", options)
        .unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
</Relationships>"#,
    )
    .unwrap();

    zip.finish().unwrap();
}
/// 构造最小合法 PDF 文件
fn create_minimal_pdf(path: &std::path::Path, text: &str) {
    let mut buf = Vec::new();
    writeln!(buf, "%PDF-1.0").unwrap();

    let off1 = buf.len();
    writeln!(buf, "1 0 obj").unwrap();
    writeln!(buf, "<< /Type /Catalog /Pages 2 0 R >>").unwrap();
    writeln!(buf, "endobj").unwrap();

    let off2 = buf.len();
    writeln!(buf, "2 0 obj").unwrap();
    writeln!(buf, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>").unwrap();
    writeln!(buf, "endobj").unwrap();

    let off3 = buf.len();
    writeln!(buf, "3 0 obj").unwrap();
    writeln!(buf, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>").unwrap();
    writeln!(buf, "endobj").unwrap();

    let stream = format!("BT /F1 12 Tf 100 700 Td ({text}) Tj ET");
    let off4 = buf.len();
    writeln!(buf, "4 0 obj").unwrap();
    writeln!(buf, "<< /Length {} >>", stream.len()).unwrap();
    writeln!(buf, "stream").unwrap();
    buf.extend_from_slice(stream.as_bytes());
    writeln!(buf).unwrap();
    writeln!(buf, "endstream").unwrap();
    writeln!(buf, "endobj").unwrap();

    let off5 = buf.len();
    writeln!(buf, "5 0 obj").unwrap();
    writeln!(
        buf,
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"
    )
    .unwrap();
    writeln!(buf, "endobj").unwrap();

    let xref_off = buf.len();
    writeln!(buf, "xref").unwrap();
    writeln!(buf, "0 6").unwrap();
    writeln!(buf, "0000000000 65535 f ").unwrap();
    writeln!(buf, "{:010} 00000 n ", off1).unwrap();
    writeln!(buf, "{:010} 00000 n ", off2).unwrap();
    writeln!(buf, "{:010} 00000 n ", off3).unwrap();
    writeln!(buf, "{:010} 00000 n ", off4).unwrap();
    writeln!(buf, "{:010} 00000 n ", off5).unwrap();

    writeln!(buf, "trailer").unwrap();
    writeln!(buf, "<< /Size 6 /Root 1 0 R >>").unwrap();
    writeln!(buf, "startxref").unwrap();
    writeln!(buf, "{}", xref_off).unwrap();
    writeln!(buf, "%%EOF").unwrap();

    std::fs::write(path, &buf).unwrap();
}

// 1.测试基础搜索功能

/// 测试中英文混合文档的端到端搜索
#[test]
fn test_mixed_language_search() -> Result<()> {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();

    fs::write(
        base.join("rust_intro.txt"),
        "Rust语言编程入门，Rust是一门安全的系统编程语言",
    )
    .unwrap();
    fs::write(
        base.join("python_intro.txt"),
        "Python编程指南，Python是易学的编程语言",
    )
    .unwrap();
    fs::write(
        base.join("seo.md"),
        "# 搜索引擎优化\n\n搜索引擎优化是提高网站排名的技术，与Web性能密切相关",
    )
    .unwrap();

    let registry = ParserRegistry::with_defaults();
    let files = scan_directory(base)?;
    let documents: Vec<_> = files
        .iter()
        .filter_map(|f| registry.parse_file(&f.path).ok())
        .collect();

    let tokenizer = MixedTokenizer::new();
    let index = InvertedIndex::build_from_documents(&documents, &tokenizer);
    let searcher = IndexSearcher::new(&index, &tokenizer, &registry);

    // 搜索中文词
    let results = searcher.search("语言", 10)?;
    assert!(!results.is_empty());

    // 搜索英文词（在含中文的文档中）
    let results = searcher.search("rust", 10)?;
    assert!(!results.is_empty());
    assert!(results[0].doc.title.contains("rust_intro"));

    // 搜索 jieba search 模式产生的子词（"搜索引擎" → "搜索" + "引擎" + "搜索引擎"）
    let results = searcher.search("搜索", 10)?;
    assert!(!results.is_empty());
    assert!(results[0].doc.title.contains("seo"));

    // 搜索不存在的中文词
    let results = searcher.search("量子力学", 10)?;
    assert!(results.is_empty());

    Ok(())
}

/// 测试 PDF 和 DOCX 文件的端到端搜索
#[test]
fn test_pdf_docx_search() -> Result<()> {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();

    create_minimal_pdf(&base.join("paper.pdf"), "Machine learning algorithms");
    create_minimal_docx(&base.join("report.docx"), "Database system optimization");

    let registry = ParserRegistry::with_defaults();
    let files = scan_directory(base)?;
    assert_eq!(files.len(), 2);

    let documents: Vec<_> = files
        .iter()
        .filter_map(|f| registry.parse_file(&f.path).ok())
        .collect();

    let tokenizer = MixedTokenizer::new();
    let index = InvertedIndex::build_from_documents(&documents, &tokenizer);
    let searcher = IndexSearcher::new(&index, &tokenizer, &registry);

    // PDF 中的词
    let results = searcher.search("machine", 10)?;
    assert!(!results.is_empty());
    assert!(results[0].doc.title.contains("paper"));

    // DOCX 中的词
    let results = searcher.search("database", 10)?;
    assert!(!results.is_empty());
    assert!(results[0].doc.title.contains("report"));

    Ok(())
}

/// 测试空目录搜索
#[test]
fn test_empty_directory_search() -> Result<()> {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();

    let registry = ParserRegistry::with_defaults();
    let files = scan_directory(base)?;
    assert!(files.is_empty());

    let documents: Vec<_> = files
        .iter()
        .filter_map(|f| registry.parse_file(&f.path).ok())
        .collect();

    let tokenizer = MixedTokenizer::new();
    let index = InvertedIndex::build_from_documents(&documents, &tokenizer);
    let searcher = IndexSearcher::new(&index, &tokenizer, &registry);

    let results = searcher.search("anything", 10)?;
    assert!(results.is_empty());

    Ok(())
}

// 2.测试排序与限制

/// 测试 TF-IDF 排序正确性（中英文）
#[test]
fn test_tfidf_ranking() -> Result<()> {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();

    // 英文：doc1 大量提到 rust，doc2 只提一次
    fs::write(
        base.join("rust_heavy.txt"),
        "rust rust rust rust rust programming",
    )
    .unwrap();
    fs::write(base.join("rust_light.txt"), "python programming rust").unwrap();

    // 中文：doc3 大量提到 "搜索"，doc4 只提一次
    fs::write(
        base.join("search_heavy.txt"),
        "搜索引擎 搜索引擎 搜索引擎 搜索引擎 搜索引擎技术",
    )
    .unwrap();
    fs::write(base.join("search_light.txt"), "编程语言 搜索引擎 开发工具").unwrap();

    let registry = ParserRegistry::with_defaults();
    let files = scan_directory(base)?;
    let documents: Vec<_> = files
        .iter()
        .filter_map(|f| registry.parse_file(&f.path).ok())
        .collect();

    let tokenizer = MixedTokenizer::new();
    let index = InvertedIndex::build_from_documents(&documents, &tokenizer);
    let searcher = IndexSearcher::new(&index, &tokenizer, &registry);

    // 英文：rust_heavy 应排第一
    let results = searcher.search("rust", 10)?;
    assert!(!results.is_empty());
    assert!(results[0].doc.title.contains("rust_heavy"));

    // 中文：search_heavy 应排第一（"搜索" 出现更多次）
    let results = searcher.search("搜索", 10)?;
    assert!(!results.is_empty());
    assert!(results[0].doc.title.contains("search_heavy"));

    Ok(())
}

/// 测试搜索结果数量限制
#[test]
fn test_search_limit() -> Result<()> {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();

    // 创建 5 个都包含 "rust" 的文档
    for i in 1..=5 {
        fs::write(
            base.join(format!("doc{i}.txt")),
            format!("rust programming doc number {i}"),
        )
        .unwrap();
    }

    let registry = ParserRegistry::with_defaults();
    let files = scan_directory(base)?;
    let documents: Vec<_> = files
        .iter()
        .filter_map(|f| registry.parse_file(&f.path).ok())
        .collect();

    let tokenizer = MixedTokenizer::new();
    let index = InvertedIndex::build_from_documents(&documents, &tokenizer);
    let searcher = IndexSearcher::new(&index, &tokenizer, &registry);

    // limit=2 时只返回 2 条
    let results = searcher.search("rust", 2)?;
    assert_eq!(results.len(), 2);

    // limit=10 时返回全部 5 条
    let results = searcher.search("rust", 10)?;
    assert_eq!(results.len(), 5);

    Ok(())
}

/// 测试搜索 snippet 生成（含中文 UTF-8 截断）
#[test]
fn test_snippet_generation() -> Result<()> {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();

    // 写一个足够长的中文文档，确保 snippet 需要截断
    let long_text = "这是一段很长的中文文本，".repeat(20)
        + "搜索引擎是信息检索的核心工具，"
        + &"后面还有很多内容用于测试截断。".repeat(20);
    fs::write(base.join("long_doc.txt"), &long_text)?;

    let registry = ParserRegistry::with_defaults();
    let files = scan_directory(base)?;
    let documents: Vec<_> = files
        .iter()
        .filter_map(|f| registry.parse_file(&f.path).ok())
        .collect();

    let tokenizer = MixedTokenizer::new();
    let index = InvertedIndex::build_from_documents(&documents, &tokenizer);
    let searcher = IndexSearcher::new(&index, &tokenizer, &registry);

    let results = searcher.search("搜索引擎", 10)?;
    assert_eq!(results.len(), 1);

    let snippet = &results[0].snippet;
    // snippet 应包含匹配词
    assert!(snippet.contains("搜索引擎"));
    // snippet 长度应远小于原文（被截断了）
    assert!(snippet.len() < long_text.len());

    Ok(())
}

// 3.测试索引持久化与重建

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

    let tokenizer = MixedTokenizer::new();
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

/// 测试索引重建覆盖旧索引
#[test]
fn test_reindex_overwrites() -> Result<()> {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();

    // 第一轮：索引 2 个文档
    fs::write(base.join("doc1.txt"), "Rust programming").unwrap();
    fs::write(base.join("doc2.txt"), "Python scripting").unwrap();

    let registry = ParserRegistry::with_defaults();
    let tokenizer = MixedTokenizer::new();
    let storage = JsonStorage::new();
    let index_path = dir.path().join("index.json");

    let files = scan_directory(base)?;
    let documents: Vec<_> = files
        .iter()
        .filter_map(|f| registry.parse_file(&f.path).ok())
        .collect();
    let index = InvertedIndex::build_from_documents(&documents, &tokenizer);
    storage.save(&index, &index_path)?;

    // 验证第一轮
    let loaded = storage.load(&index_path)?;
    let searcher = IndexSearcher::new(&loaded, &tokenizer, &registry);
    assert_eq!(searcher.search("rust", 10)?.len(), 1);
    assert_eq!(searcher.search("python", 10)?.len(), 1);

    // 第二轮：删除 doc2，新增 doc3
    fs::remove_file(base.join("doc2.txt"))?;
    fs::write(base.join("doc3.txt"), "Golang concurrency")?;

    let files = scan_directory(base)?;
    let documents: Vec<_> = files
        .iter()
        .filter_map(|f| registry.parse_file(&f.path).ok())
        .collect();
    let new_index = InvertedIndex::build_from_documents(&documents, &tokenizer);
    storage.save(&new_index, &index_path)?;

    // 验证第二轮：python 消失，golang 出现
    let loaded = storage.load(&index_path)?;
    let searcher = IndexSearcher::new(&loaded, &tokenizer, &registry);
    assert_eq!(searcher.search("rust", 10)?.len(), 1);
    assert!(searcher.search("python", 10)?.is_empty());
    assert_eq!(searcher.search("golang", 10)?.len(), 1);

    Ok(())
}
