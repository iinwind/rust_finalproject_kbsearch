use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use crate::error::{KbError, Result};

/// 文档 ID 类型
pub type DocId = u64;

/// 解析后的文档
#[derive(Debug, Clone)]
pub struct Document {
    pub id: DocId,
    pub path: PathBuf,
    pub title: String,
    pub content: String,
}

/// 文档解析器 trait
///
/// 不同文件格式实现此 trait，提供统一的解析接口
pub trait Parser: Send + Sync {
    /// 解析指定路径的文件，返回 Document
    fn parse(&self, path: &Path, id: DocId) -> Result<Document>;

    /// 返回此解析器支持的文件扩展名列表
    fn supported_extensions(&self) -> &[&str];
}

/// 根据文件路径生成文档 ID
///
/// 使用路径的哈希值作为 ID，保证同一文件路径始终生成相同 ID
pub fn generate_doc_id(path: &Path) -> DocId {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    hasher.finish()
}

/// 从文件路径提取标题（文件名，不含扩展名）
fn extract_title(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled")
        .to_string()
}

// ========== TxtParser ==========

/// 纯文本文件解析器
pub struct TxtParser;

impl TxtParser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TxtParser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser for TxtParser {
    fn parse(&self, path: &Path, id: DocId) -> Result<Document> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            KbError::Parse(format!("Failed to read file {}: {}", path.display(), e))
        })?;

        let title = extract_title(path);

        Ok(Document {
            id,
            path: path.to_path_buf(),
            title,
            content,
        })
    }

    fn supported_extensions(&self) -> &[&str] {
        &["txt"]
    }
}

// ========== MarkdownParser ==========

/// Markdown 文件解析器
///
/// 使用 pulldown-cmark 提取纯文本内容，去除 Markdown 格式标记
pub struct MarkdownParser;

impl MarkdownParser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MarkdownParser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser for MarkdownParser {
    fn parse(&self, path: &Path, id: DocId) -> Result<Document> {
        let raw = std::fs::read_to_string(path).map_err(|e| {
            KbError::Parse(format!("Failed to read file {}: {}", path.display(), e))
        })?;

        let content = extract_markdown_text(&raw);
        let title = extract_title(path);

        Ok(Document {
            id,
            path: path.to_path_buf(),
            title,
            content,
        })
    }

    fn supported_extensions(&self) -> &[&str] {
        &["md", "markdown"]
    }
}

/// 从 Markdown 文本中提取纯文本内容
///
/// 遍历 pulldown-cmark 的事件流，只保留文本内容
fn extract_markdown_text(markdown: &str) -> String {
    use pulldown_cmark::{Event, Parser};

    let parser = Parser::new(markdown);
    let mut result = String::new();

    for event in parser {
        match event {
            Event::Text(text) => {
                if !result.is_empty() {
                    result.push(' ');
                }
                result.push_str(&text);
            }
            Event::Code(code) => {
                if !result.is_empty() {
                    result.push(' ');
                }
                result.push_str(&code);
            }
            Event::SoftBreak | Event::HardBreak => {
                result.push(' ');
            }
            _ => {}
        }
    }

    result
}

// ========== PdfParser ==========

/// PDF 文件解析器
///
/// 使用 pdf-extract 从 PDF 中提取纯文本内容
pub struct PdfParser;

impl PdfParser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PdfParser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser for PdfParser {
    fn parse(&self, path: &Path, id: DocId) -> Result<Document> {
        let content = pdf_extract::extract_text(path).map_err(|e| {
            KbError::Parse(format!("Failed to parse PDF {}: {}", path.display(), e))
        })?;

        let title = extract_title(path);

        Ok(Document {
            id,
            path: path.to_path_buf(),
            title,
            content,
        })
    }

    fn supported_extensions(&self) -> &[&str] {
        &["pdf"]
    }
}

// ========== DocxParser ==========

/// DOCX 文件解析器
///
/// 使用 docx-lite 从 Word 文档中提取纯文本内容
pub struct DocxParser;

impl DocxParser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DocxParser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser for DocxParser {
    fn parse(&self, path: &Path, id: DocId) -> Result<Document> {
        let content = docx_lite::extract_text(path).map_err(|e| {
            KbError::Parse(format!("Failed to parse DOCX {}: {}", path.display(), e))
        })?;

        let title = extract_title(path);

        Ok(Document {
            id,
            path: path.to_path_buf(),
            title,
            content,
        })
    }

    fn supported_extensions(&self) -> &[&str] {
        &["docx"]
    }
}

// ========== ParserRegistry ==========

/// 解析器注册表
///
/// 根据文件扩展名自动选择合适的解析器
pub struct ParserRegistry {
    parsers: Vec<Box<dyn Parser>>,
}

impl ParserRegistry {
    /// 创建空的解析器注册表
    pub fn new() -> Self {
        Self {
            parsers: Vec::new(),
        }
    }

    /// 创建包含默认解析器的注册表（TxtParser + MarkdownParser + PdfParser + DocxParser）
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(TxtParser::new()));
        registry.register(Box::new(MarkdownParser::new()));
        registry.register(Box::new(PdfParser::new()));
        registry.register(Box::new(DocxParser::new()));
        registry
    }

    /// 注册一个解析器
    pub fn register(&mut self, parser: Box<dyn Parser>) {
        self.parsers.push(parser);
    }

    /// 解析指定文件
    ///
    /// 根据文件扩展名自动选择匹配的解析器
    pub fn parse_file(&self, path: &Path) -> Result<Document> {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| KbError::Parse(format!("File has no extension: {}", path.display())))?
            .to_lowercase();

        for parser in &self.parsers {
            if parser
                .supported_extensions()
                .iter()
                .any(|ext| ext.to_lowercase() == extension)
            {
                let id = generate_doc_id(path);
                return parser.parse(path, id);
            }
        }

        Err(KbError::Parse(format!(
            "No parser found for extension: .{}",
            extension
        )))
    }
}

impl Default for ParserRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_generate_doc_id_consistency() {
        let path = Path::new("/test/file.md");
        let id1 = generate_doc_id(path);
        let id2 = generate_doc_id(path);
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_generate_doc_id_uniqueness() {
        let id1 = generate_doc_id(Path::new("/test/file1.md"));
        let id2 = generate_doc_id(Path::new("/test/file2.md"));
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_extract_title() {
        assert_eq!(extract_title(Path::new("/docs/my_notes.md")), "my_notes");
        assert_eq!(extract_title(Path::new("/docs/readme.txt")), "readme");
    }

    #[test]
    fn test_txt_parser() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "Hello, world!").unwrap();

        let parser = TxtParser::new();
        let id = generate_doc_id(&file_path);
        let doc = parser.parse(&file_path, id).unwrap();

        assert_eq!(doc.id, id);
        assert_eq!(doc.title, "test");
        assert_eq!(doc.content, "Hello, world!");
    }

    #[test]
    fn test_markdown_parser() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "# Title\n\nHello **bold** and `code`").unwrap();

        let parser = MarkdownParser::new();
        let id = generate_doc_id(&file_path);
        let doc = parser.parse(&file_path, id).unwrap();

        assert_eq!(doc.title, "test");
        // 应包含纯文本，不含 Markdown 标记
        assert!(doc.content.contains("Title"));
        assert!(doc.content.contains("bold"));
        assert!(doc.content.contains("code"));
        assert!(!doc.content.contains("**"));
    }

    #[test]
    fn test_parser_registry_txt() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("notes.txt");
        fs::write(&file_path, "Some notes").unwrap();

        let registry = ParserRegistry::with_defaults();
        let doc = registry.parse_file(&file_path).unwrap();

        assert_eq!(doc.content, "Some notes");
    }

    #[test]
    fn test_parser_registry_markdown() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("readme.md");
        fs::write(&file_path, "# Hello\nWorld").unwrap();

        let registry = ParserRegistry::with_defaults();
        let doc = registry.parse_file(&file_path).unwrap();

        assert!(doc.content.contains("Hello"));
        assert!(doc.content.contains("World"));
    }

    #[test]
    fn test_parser_registry_unsupported() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.csv");
        fs::write(&file_path, "a,b,c").unwrap();

        let registry = ParserRegistry::with_defaults();
        let result = registry.parse_file(&file_path);

        assert!(result.is_err());
    }

    #[test]
    fn test_extract_markdown_text() {
        let md = "# Heading\n\nParagraph with **bold** and *italic*.\n\n- Item 1\n- Item 2";
        let text = extract_markdown_text(md);

        assert!(text.contains("Heading"));
        assert!(text.contains("bold"));
        assert!(text.contains("italic"));
        assert!(text.contains("Item 1"));
        assert!(text.contains("Item 2"));
        // 不应包含 Markdown 标记
        assert!(!text.contains("**"));
        assert!(!text.contains("*"));
    }

    /// 构造最小合法 DOCX 文件（DOCX 本质是 ZIP 包含 XML）
    fn create_minimal_docx(path: &Path, text: &str) {
        use std::io::Write;

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

    #[test]
    fn test_docx_parser() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.docx");
        create_minimal_docx(&file_path, "Hello from DOCX");

        let parser = DocxParser::new();
        let id = generate_doc_id(&file_path);
        let doc = parser.parse(&file_path, id).unwrap();

        assert_eq!(doc.id, id);
        assert_eq!(doc.title, "test");
        assert!(
            doc.content.contains("Hello from DOCX"),
            "DOCX content should contain extracted text, got: {:?}",
            doc.content
        );
    }

    #[test]
    fn test_parser_registry_docx() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("report.docx");
        create_minimal_docx(&file_path, "Quarterly report");

        let registry = ParserRegistry::with_defaults();
        let doc = registry.parse_file(&file_path).unwrap();

        assert!(doc.content.contains("Quarterly report"));
    }

    /// 构造最小合法 PDF 文件（含正确的交叉引用表偏移）
    fn create_minimal_pdf(path: &Path, text: &str) {
        use std::io::Write;

        let mut buf = Vec::new();
        writeln!(buf, "%PDF-1.0").unwrap();

        // 对象 1: Catalog
        let off1 = buf.len();
        writeln!(buf, "1 0 obj").unwrap();
        writeln!(buf, "<< /Type /Catalog /Pages 2 0 R >>").unwrap();
        writeln!(buf, "endobj").unwrap();

        // 对象 2: Pages
        let off2 = buf.len();
        writeln!(buf, "2 0 obj").unwrap();
        writeln!(buf, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>").unwrap();
        writeln!(buf, "endobj").unwrap();

        // 对象 3: Page
        let off3 = buf.len();
        writeln!(buf, "3 0 obj").unwrap();
        writeln!(buf, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>").unwrap();
        writeln!(buf, "endobj").unwrap();

        // 对象 4: Content stream
        let stream = format!("BT /F1 12 Tf 100 700 Td ({text}) Tj ET");
        let off4 = buf.len();
        writeln!(buf, "4 0 obj").unwrap();
        writeln!(buf, "<< /Length {} >>", stream.len()).unwrap();
        writeln!(buf, "stream").unwrap();
        buf.extend_from_slice(stream.as_bytes());
        writeln!(buf).unwrap();
        writeln!(buf, "endstream").unwrap();
        writeln!(buf, "endobj").unwrap();

        // 对象 5: Font
        let off5 = buf.len();
        writeln!(buf, "5 0 obj").unwrap();
        writeln!(
            buf,
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"
        )
        .unwrap();
        writeln!(buf, "endobj").unwrap();

        // Cross-reference table
        let xref_off = buf.len();
        writeln!(buf, "xref").unwrap();
        writeln!(buf, "0 6").unwrap();
        writeln!(buf, "0000000000 65535 f ").unwrap();
        writeln!(buf, "{:010} 00000 n ", off1).unwrap();
        writeln!(buf, "{:010} 00000 n ", off2).unwrap();
        writeln!(buf, "{:010} 00000 n ", off3).unwrap();
        writeln!(buf, "{:010} 00000 n ", off4).unwrap();
        writeln!(buf, "{:010} 00000 n ", off5).unwrap();

        // Trailer
        writeln!(buf, "trailer").unwrap();
        writeln!(buf, "<< /Size 6 /Root 1 0 R >>").unwrap();
        writeln!(buf, "startxref").unwrap();
        writeln!(buf, "{}", xref_off).unwrap();
        writeln!(buf, "%%EOF").unwrap();

        std::fs::write(path, &buf).unwrap();
    }

    #[test]
    fn test_pdf_parser() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.pdf");
        create_minimal_pdf(&file_path, "Hello from PDF");

        let parser = PdfParser::new();
        let id = generate_doc_id(&file_path);
        let doc = parser.parse(&file_path, id).unwrap();

        assert_eq!(doc.id, id);
        assert_eq!(doc.title, "test");
        assert!(
            doc.content.contains("Hello from PDF"),
            "PDF content should contain extracted text, got: {:?}",
            doc.content
        );
    }

    #[test]
    fn test_parser_registry_pdf() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("paper.pdf");
        create_minimal_pdf(&file_path, "Research paper abstract");

        let registry = ParserRegistry::with_defaults();
        let doc = registry.parse_file(&file_path).unwrap();

        assert!(doc.content.contains("Research paper abstract"));
    }

    #[test]
    fn test_pdf_parser_invalid_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("bad.pdf");
        fs::write(&file_path, "this is not a pdf").unwrap();

        let parser = PdfParser::new();
        let id = generate_doc_id(&file_path);
        let result = parser.parse(&file_path, id);

        assert!(result.is_err());
    }

    #[test]
    fn test_docx_parser_invalid_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("bad.docx");
        fs::write(&file_path, "this is not a docx").unwrap();

        let parser = DocxParser::new();
        let id = generate_doc_id(&file_path);
        let result = parser.parse(&file_path, id);

        assert!(result.is_err());
    }
}
