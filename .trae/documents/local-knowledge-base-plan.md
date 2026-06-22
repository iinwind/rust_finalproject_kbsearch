# 本地知识库搜索系统 - 需求分析与实施计划

## 一、选题分析

**选题**：本地知识库搜索系统

**核心价值**：对本地目录中的文档（Markdown/TXT等）建立索引，支持快速全文检索，类似于本地版的"Everything"或简易版 Elasticsearch。

**与 Rust 特性的契合度**：
- 所有权/借用：索引构建、查询处理中的数据共享与转移
- struct/enum：文档模型、查询类型、搜索结果
- trait：搜索引擎抽象、文件解析器抽象
- 泛型：不同存储后端、不同分词器
- 错误处理：文件IO、解析失败、查询语法错误
- 并发：多线程索引构建、并行查询

---

## 二、功能点全览与分级

### P0 - 核心功能（必须实现，构成项目骨架）

| # | 功能 | 说明 | Rust难度 |
|---|------|------|----------|
| 1 | 文件扫描与收集 | 递归扫描指定目录，收集支持的文件 | ⭐ 低 - 主要是 `std::fs` 和路径操作 |
| 2 | 文档解析 | 解析 TXT/Markdown 文件，提取纯文本内容 | ⭐⭐ 中低 - Markdown 解析需引入 crate |
| 3 | 分词与倒排索引 | 对文本分词，构建倒排索引（token → 文档列表） | ⭐⭐⭐ 中 - 核心数据结构，需理解 HashMap/BTreeMap |
| 4 | 关键词搜索 | 支持输入关键词，返回匹配文档列表 | ⭐⭐ 中 - 倒排索引查询 |
| 5 | TF-IDF 排序 | 基于词频-逆文档频率对结果排序 | ⭐⭐ 中 - 算法本身简单，Rust实现无特殊难度 |
| 6 | CLI 交互界面 | 命令行交互式搜索（add/search/list等子命令） | ⭐⭐ 中低 - 使用 `clap` crate |
| 7 | 索引持久化 | 将索引序列化到磁盘，启动时加载 | ⭐⭐⭐ 中高 - 需要 serde 序列化，理解 Rust 类型与 trait |

### P1 - 增强功能（提升项目完整度与亮点）

| # | 功能 | 说明 | Rust难度 |
|---|------|------|----------|
| 8 | 多格式支持 | 支持 PDF/DOCX 等格式解析 | ⭐⭐⭐ 中 - 需引入对应解析 crate |
| 9 | 布尔查询 | 支持 AND/OR/NOT 组合查询 | ⭐⭐⭐ 中高 - 需设计查询表达式 AST，涉及 enum 递归类型 |
| 10 | 搜索结果高亮 | 在结果中高亮匹配词 | ⭐⭐ 中 - 字符串处理 |
| 11 | 增量索引更新 | 检测文件变更，只更新变化部分 | ⭐⭐⭐⭐ 高 - 需文件元信息追踪，增量更新逻辑复杂 |
| 12 | 配置文件 | TOML 配置文件支持 | ⭐⭐ 中低 - serde + toml crate |
| 13 | 多线程索引构建 | 并行处理多个文件的索引构建 | ⭐⭐⭐⭐ 高 - 需理解 Arc/Mutex/channel，Rust并发模型对新手挑战大 |

### P2 - 锦上添花（时间充裕可做）

| # | 功能 | 说明 | Rust难度 |
|---|------|------|----------|
| 14 | 中文分词 | 使用 jieba 分词支持中文搜索 | ⭐⭐⭐ 中 - 引入 jieba crate 即可，但中文处理细节多 |
| 15 | 模糊搜索 | 支持拼写容错/模糊匹配 | ⭐⭐⭐⭐ 高 - 编辑距离算法 + Rust 字符串处理 |
| 16 | Web UI | 提供简单 Web 界面搜索 | ⭐⭐⭐⭐ 高 - 需要 tokio + HTTP 框架，异步编程对新手不友好 |
| 17 | 文件监控 | Watch 目录变化自动更新索引 | ⭐⭐⭐⭐ 高 - 异步/事件驱动 |
| 18 | 搜索历史 | 记录搜索历史 | ⭐⭐ 低 - 简单文件读写 |
| 19 | 标签系统 | 为文档打标签分类 | ⭐⭐⭐ 中 - 数据模型设计 |

---

## 三、Rust 新手需特别注意的难点

1. **生命周期标注**：倒排索引中文档引用、搜索结果中引用索引数据时，可能需要显式生命周期
2. **字符串处理**：Rust 的 String/&str/Vec<char 区别是新手常见坑，分词时尤甚
3. **并发编程**：Arc<Mutex<>>、channel、Send/Sync trait 对新手概念负担重
4. **错误处理范式**：从大量 unwrap 迁移到 Result + ? 操作符需要刻意练习
5. **trait 设计**：如何用 trait 抽象搜索引擎/分词器/存储后端，需要一定设计经验

---

## 四、推荐 P0 功能清单

基于以下原则筛选：
- 功能完整可运行
- 代码量可控（P0 预计 ~1500-2000 行）
- Rust 特性覆盖充分
- 避免新手高难度点

### 推荐的 P0 清单

| # | 功能 | 模块归属 | 关键 Rust 特性 |
|---|------|----------|---------------|
| 1 | 文件扫描与收集 | `scanner` | fs, 路径处理, 迭代器, Result 错误处理 |
| 2 | 文档解析（TXT + Markdown） | `parser` | trait（Parser 抽象）, enum（文件类型）, String 处理 |
| 3 | 分词 + 倒排索引构建 | `indexer` | HashMap/BTreeMap, struct, 泛型, 所有权转移 |
| 4 | 关键词搜索 + TF-IDF 排序 | `searcher` | trait（SearchEngine）, 生命周期, 借用 |
| 5 | CLI 交互界面 | `cli` | clap, 模式匹配 |
| 6 | 索引持久化（序列化到磁盘） | `storage` | serde 序列化, trait, 错误处理 |
| 7 | 单元测试 | 各模块 | #[test], 测试组织 |

### 模块结构预览

```
src/
├── main.rs          # 入口，CLI 解析
├── scanner.rs       # 文件扫描模块
├── parser.rs        # 文档解析模块（trait Parser + 实现）
├── indexer.rs       # 索引构建模块（倒排索引 + TF-IDF）
├── searcher.rs      # 搜索模块（trait SearchEngine + 实现）
├── storage.rs       # 索引持久化模块
└── error.rs         # 统一错误类型（thiserror）
```

### 依赖 crate 预估

| crate | 用途 |
|-------|------|
| `clap` | CLI 参数解析 |
| `serde` + `serde_json` | 索引序列化 |
| `pulldown-cmark` | Markdown 解析 |
| `thiserror` | 错误类型定义 |
| `walkdir` | 递归目录遍历 |

---

## 五、详细实施步骤

### 步骤 1：项目初始化

**目标**：配置 Cargo.toml 依赖，创建模块骨架文件

**Cargo.toml 依赖**：
```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
pulldown-cmark = "0.11"
thiserror = "2"
walkdir = "2"
```

**创建文件**：
- `src/error.rs` — 空文件占位
- `src/scanner.rs` — 空文件占位
- `src/parser.rs` — 空文件占位
- `src/indexer.rs` — 空文件占位
- `src/searcher.rs` — 空文件占位
- `src/storage.rs` — 空文件占位

**main.rs 模块声明**：
```rust
mod error;
mod scanner;
mod parser;
mod indexer;
mod searcher;
mod storage;
```

---

### 步骤 2：error.rs — 统一错误类型

**目标**：定义项目全局错误类型，所有模块统一使用

**数据结构**：
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum KbError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Index error: {0}")]
    Index(String),

    #[error("Search error: {0}")]
    Search(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("No index found at {0}, please run `index` command first")]
    NoIndex(String),
}

pub type Result<T> = std::result::Result<T, KbError>;
```

**体现的 Rust 特性**：enum、thiserror derive 宏、`#[from]` 自动转换、类型别名

---

### 步骤 3：scanner.rs — 文件扫描模块

**目标**：递归扫描指定目录，收集支持的文件路径

**数据结构**：
```rust
use std::path::{Path, PathBuf};

/// 支持的文件类型
#[derive(Debug, Clone, PartialEq)]
pub enum FileType {
    Markdown,
    PlainText,
}

/// 扫描到的文件条目
#[derive(Debug, Clone)]
pub struct ScannedFile {
    pub path: PathBuf,
    pub file_type: FileType,
}
```

**核心函数**：
```rust
/// 根据扩展名判断文件类型
pub fn detect_file_type(path: &Path) -> Option<FileType>

/// 递归扫描目录，返回所有支持的文件
pub fn scan_directory(dir: &Path) -> Result<Vec<ScannedFile>>
```

**实现要点**：
- 使用 `walkdir::WalkDir` 递归遍历
- 通过 `Path::extension()` 判断文件类型（.md → Markdown, .txt → PlainText）
- 跳过隐藏文件/目录（以 `.` 开头）
- 错误处理：目录不存在 → `KbError::Io`，权限不足 → 跳过并记录

**体现的 Rust 特性**：Path/PathBuf、迭代器、模式匹配、Result 传播

---

### 步骤 4：parser.rs — 文档解析模块

**目标**：将文件解析为统一的文档结构，用 trait 抽象不同格式的解析器

**数据结构**：
```rust
use std::path::Path;
use crate::error::Result;

/// 文档ID类型
pub type DocId = u64;

/// 解析后的文档
#[derive(Debug, Clone)]
pub struct Document {
    pub id: DocId,
    pub path: PathBuf,
    pub title: String,       // 文件名（不含扩展名）作为标题
    pub content: String,     // 纯文本内容
}

/// 文档解析器 trait
pub trait Parser: Send + Sync {
    fn parse(&self, path: &Path, id: DocId) -> Result<Document>;
    fn supported_extensions(&self) -> &[&str];
}
```

**实现**：
```rust
/// 纯文本解析器
pub struct TxtParser;

impl Parser for TxtParser { ... }

/// Markdown 解析器（使用 pulldown-cmark 提取纯文本）
pub struct MarkdownParser;

impl Parser for MarkdownParser { ... }

/// 解析器调度：根据文件类型选择对应解析器
pub struct ParserRegistry {
    parsers: Vec<Box<dyn Parser>>,
}

impl ParserRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, parser: Box<dyn Parser>);
    pub fn parse_file(&self, path: &Path, id: DocId) -> Result<Document>;
}
```

**实现要点**：
- `TxtParser`：直接 `std::fs::read_to_string`
- `MarkdownParser`：用 `pulldown-cmark` 的 `Parser` 遍历事件，提取 `Event::Text` 内容
- `ParserRegistry`：遍历注册的解析器，找到支持该扩展名的解析器执行
- `DocId` 生成：基于文件路径的哈希（`std::hash::Hasher`）

**体现的 Rust 特性**：trait 定义与实现、`dyn Trait` 动态分发、`Box<dyn Parser>`、`Send + Sync` 约束、泛型注册

---

### 步骤 5：indexer.rs — 倒排索引 + TF-IDF

**目标**：构建倒排索引数据结构，实现分词和 TF-IDF 计算

**数据结构**：
```rust
use std::collections::HashMap;

/// 词项在文档中的位置信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Posting {
    pub doc_id: DocId,
    pub term_freq: usize,
    pub positions: Vec<usize>,  // 词在文档中的位置（用于后续高亮）
}

/// 文档元信息（不存储全文，节省空间）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMeta {
    pub id: DocId,
    pub path: PathBuf,
    pub title: String,
    pub doc_length: usize,  // 总词数，用于 TF 归一化
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
```

**分词器**：
```rust
/// 分词器 trait
pub trait Tokenizer: Send + Sync {
    fn tokenize(&self, text: &str) -> Vec<String>;
}

/// 简单英文分词器：小写化 + 按非字母数字分割 + 去停用词
pub struct SimpleTokenizer {
    stop_words: HashSet<String>,
}

impl SimpleTokenizer {
    pub fn new() -> Self;
}

impl Tokenizer for SimpleTokenizer { ... }
```

**核心函数**：
```rust
impl InvertedIndex {
    /// 从空索引开始
    pub fn new() -> Self;

    /// 将一个文档加入索引
    pub fn add_document(&mut self, doc: &Document, tokenizer: &dyn Tokenizer);

    /// 批量构建索引
    pub fn build_from_documents(docs: Vec<Document>, tokenizer: &dyn Tokenizer) -> Self;

    /// 计算 TF-IDF 分数
    /// TF = term_freq / doc_length
    /// IDF = 1 + ln(doc_count / (1 + doc_freq))
    pub fn tfidf(&self, token: &str, doc_id: DocId) -> f64;

    /// 获取索引统计信息
    pub fn stats(&self) -> IndexStats;
}
```

**实现要点**：
- 分词：`text.to_lowercase()` → `split(|c: char| !c.is_alphanumeric())` → 过滤空串和停用词
- 倒排列表构建：遍历 token，更新 `postings` 和 `positions`
- TF-IDF：标准公式，`f64` 计算
- `positions` 记录词在分词结果中的下标（非字符偏移，简化实现）

**体现的 Rust 特性**：HashMap 操作、trait + 泛型、所有权转移（Document 消费）、迭代器链、Serialize/Deserialize derive

---

### 步骤 6：searcher.rs — 搜索引擎

**目标**：基于倒排索引执行搜索，返回 TF-IDF 排序的结果

**数据结构**：
```rust
/// 搜索结果条目
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub doc: DocumentMeta,
    pub score: f64,
    pub snippet: String,  // 匹配词附近的文本片段
}

/// 搜索引擎 trait
pub trait SearchEngine {
    fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>>;
}
```

**实现**：
```rust
/// 基于 InvertedIndex 的搜索引擎
pub struct IndexSearcher<'a> {
    index: &'a InvertedIndex,
    tokenizer: &'a dyn Tokenizer,
}

impl<'a> IndexSearcher<'a> {
    pub fn new(index: &'a InvertedIndex, tokenizer: &'a dyn Tokenizer) -> Self;
}

impl<'a> SearchEngine for IndexSearcher<'a> {
    fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>>;
}
```

**搜索流程**：
1. 对 query 分词得到 tokens
2. 对每个 token，查找倒排列表
3. 对每个匹配文档，累加各 token 的 TF-IDF 分数
4. 按分数降序排序，取 top N
5. 生成 snippet：从原始文档内容中截取匹配词附近 ~100 字符

**体现的 Rust 特性**：生命周期 `'a`（ searcher 借用 index）、trait 实现、借用规则

---

### 步骤 7：storage.rs — 索引持久化

**目标**：将索引序列化到磁盘 / 从磁盘反序列化

**数据结构**：
```rust
use std::path::Path;
use crate::error::Result;

/// 索引存储 trait
pub trait IndexStorage {
    fn save(&self, index: &InvertedIndex, path: &Path) -> Result<()>;
    fn load(&self, path: &Path) -> Result<InvertedIndex>;
}

/// JSON 存储实现
pub struct JsonStorage;

impl IndexStorage for JsonStorage { ... }
```

**实现要点**：
- `save`：`serde_json::to_string_pretty()` → `std::fs::write()`
- `load`：`std::fs::read_to_string()` → `serde_json::from_str()`
- 默认索引文件路径：`~/.kbsearch/index.json` 或用户指定路径
- 错误处理：文件不存在 → `KbError::NoIndex`，格式错误 → `KbError::Storage`

**体现的 Rust 特性**：trait 抽象存储后端、serde 序列化、错误处理

---

### 步骤 8：CLI + main.rs — 命令行交互

**目标**：使用 clap 定义子命令，串联所有模块

**CLI 命令设计**：
```
kbsearch index   <dir>        # 扫描目录，构建索引并保存
kbsearch search  <query>      # 加载索引，搜索关键词
kbsearch list                 # 列出所有已索引文档
kbsearch info                 # 显示索引统计信息
kbsearch reindex <dir>        # 重新构建索引
```

**main.rs 结构**：
```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "kbsearch", about = "本地知识库搜索系统")]
struct Cli {
    /// 索引文件路径（默认 ~/.kbsearch/index.json）
    #[arg(long, global = true)]
    index_path: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 扫描目录并构建索引
    Index {
        /// 要索引的目录路径
        dir: PathBuf,
    },
    /// 搜索关键词
    Search {
        /// 搜索查询词
        query: String,
        /// 返回结果数量
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// 列出所有已索引文档
    List,
    /// 显示索引统计信息
    Info,
    /// 重新构建索引
    Reindex {
        /// 要索引的目录路径
        dir: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    // 根据子命令分发到对应处理函数
    match cli.command {
        Commands::Index { dir } => handle_index(&dir, &index_path)?,
        Commands::Search { query, limit } => handle_search(&query, limit, &index_path)?,
        Commands::List => handle_list(&index_path)?,
        Commands::Info => handle_info(&index_path)?,
        Commands::Reindex { dir } => handle_reindex(&dir, &index_path)?,
    }
    Ok(())
}
```

**各命令处理流程**：

- **index**：`scan_directory` → `ParserRegistry::parse_file`（逐文件）→ `InvertedIndex::build_from_documents` → `JsonStorage::save`
- **search**：`JsonStorage::load` → `IndexSearcher::new` → `search` → 打印结果
- **list**：`JsonStorage::load` → 遍历 `index.documents` → 打印文档列表
- **info**：`JsonStorage::load` → `index.stats()` → 打印统计
- **reindex**：等同 index（覆盖旧索引）

**搜索结果输出格式**：
```
[1] score=2.345  my_notes.md
    标题: My Notes
    摘要: ...这是匹配词附近的文本片段...

[2] score=1.892  readme.txt
    标题: Readme
    摘要: ...另一个匹配片段...
```

**体现的 Rust 特性**：clap derive 宏、enum 模式匹配、模块组合

---

### 步骤 9：单元测试

**目标**：为各模块编写关键功能测试

**测试分布**：

| 模块 | 测试内容 |
|------|----------|
| `scanner` | 扫描临时目录、过滤文件类型、跳过隐藏文件 |
| `parser` | TXT 解析、Markdown 解析（提取纯文本）、ParserRegistry 调度 |
| `indexer` | 分词器正确性、倒排索引构建、TF-IDF 计算正确性 |
| `searcher` | 单词搜索、多词搜索、排序正确性、空结果处理 |
| `storage` | 索引序列化/反序列化一致性、文件不存在错误 |

**测试策略**：
- 使用 `std::env::temp_dir()` 创建临时测试文件
- 每个模块的 `#[cfg(test)] mod tests` 内编写
- 关键：TF-IDF 计算结果与手工计算对比

---

### 步骤 10：cargo fmt + clippy 修正

**目标**：确保代码通过格式化和 lint 检查

- `cargo fmt` — 自动格式化
- `cargo clippy` — 修复所有 warning
- 确认 `cargo build` 无 error/warning
- 确认 `cargo test` 全部通过

---

### 数据流总览

```
用户执行 `kbsearch index ./docs`
  │
  ▼
scanner::scan_directory("./docs")
  → Vec<ScannedFile>
  │
  ▼
parser::ParserRegistry::parse_file()  (逐文件)
  → Vec<Document>
  │
  ▼
indexer::InvertedIndex::build_from_documents()
  → InvertedIndex
  │
  ▼
storage::JsonStorage::save()
  → 写入 index.json


用户执行 `kbsearch search "rust ownership"`
  │
  ▼
storage::JsonStorage::load()
  → InvertedIndex
  │
  ▼
searcher::IndexSearcher::search("rust ownership")
  → Vec<SearchResult>
  │
  ▼
格式化输出到终端
```

### 预估代码量

| 模块 | 预估行数 |
|------|----------|
| error.rs | ~30 |
| scanner.rs | ~80 |
| parser.rs | ~150 |
| indexer.rs | ~250 |
| searcher.rs | ~180 |
| storage.rs | ~80 |
| main.rs (CLI) | ~200 |
| 测试代码 | ~300 |
| **合计** | **~1270** |

加上注释和空行，预计总代码量 1500-2000 行，满足课程要求。
