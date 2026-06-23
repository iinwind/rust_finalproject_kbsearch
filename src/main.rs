use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

use kbsearch::error::Result;
use kbsearch::indexer::{InvertedIndex, SimpleTokenizer};
use kbsearch::parser::ParserRegistry;
use kbsearch::scanner::scan_directory;
use kbsearch::searcher::{IndexSearcher, SearchEngine};
use kbsearch::storage::{default_index_path, IndexStorage, JsonStorage};

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
    let index_path = cli.index_path.unwrap_or_else(default_index_path);

    match cli.command {
        Commands::Index { dir } => handle_index(&dir, &index_path),
        Commands::Search { query, limit } => handle_search(&query, limit, &index_path),
        Commands::List => handle_list(&index_path),
        Commands::Info => handle_info(&index_path),
        Commands::Reindex { dir } => handle_index(&dir, &index_path),
    }
}

/// 处理 index 子命令：扫描目录 → 解析文档 → 构建索引 → 保存
fn handle_index(dir: &Path, index_path: &Path) -> Result<()> {
    println!("扫描目录: {}", dir.display());

    // 1. 扫描文件
    let files = scan_directory(dir)?;
    println!("发现 {} 个支持的文件", files.len());

    if files.is_empty() {
        println!("未找到可索引的文件（支持 .md 和 .txt 格式）");
        return Ok(());
    }

    // 2. 解析文档
    let registry = ParserRegistry::with_defaults();
    let mut documents = Vec::new();
    let mut failed = 0;

    for file in &files {
        match registry.parse_file(&file.path) {
            Ok(doc) => documents.push(doc),
            Err(e) => {
                eprintln!("  跳过 {}: {}", file.path.display(), e);
                failed += 1;
            }
        }
    }

    println!(
        "成功解析 {} 个文档{}",
        documents.len(),
        if failed > 0 {
            format!("，{} 个失败", failed)
        } else {
            String::new()
        }
    );

    if documents.is_empty() {
        println!("没有可索引的文档");
        return Ok(());
    }

    // 3. 构建索引
    let tokenizer = SimpleTokenizer::new();
    let index = InvertedIndex::build_from_documents(&documents, &tokenizer);
    let stats = index.stats();

    // 4. 保存索引
    let storage = JsonStorage::new();
    storage.save(&index, index_path)?;

    println!("\n索引构建完成！");
    println!("  文档数: {}", stats.doc_count);
    println!("  词汇量: {}", stats.vocab_size);
    println!("  倒排记录数: {}", stats.total_postings);
    println!("  索引保存至: {}", index_path.display());

    Ok(())
}

/// 处理 search 子命令：加载索引 → 搜索 → 显示结果
fn handle_search(query: &str, limit: usize, index_path: &Path) -> Result<()> {
    let storage = JsonStorage::new();
    let index = storage.load(index_path)?;

    let tokenizer = SimpleTokenizer::new();
    let searcher = IndexSearcher::new(&index, &tokenizer);

    let results = searcher.search(query, limit)?;

    if results.is_empty() {
        println!("未找到与 \"{}\" 相关的文档", query);
        return Ok(());
    }

    println!("搜索 \"{}\" 找到 {} 个结果：\n", query, results.len());

    for (i, result) in results.iter().enumerate() {
        println!(
            "[{}] score={:.4}  {}",
            i + 1,
            result.score,
            result.doc.title
        );
        println!("    路径: {}", result.doc.path);
        println!("    摘要: {}", result.snippet);
        println!();
    }

    Ok(())
}

/// 处理 list 子命令：列出所有已索引文档
fn handle_list(index_path: &Path) -> Result<()> {
    let storage = JsonStorage::new();
    let index = storage.load(index_path)?;

    let mut docs: Vec<_> = index.documents.values().collect();
    docs.sort_by_key(|d| d.title.clone());

    println!("已索引文档（共 {} 个）：\n", docs.len());

    for doc in &docs {
        println!("  {} ({})", doc.title, doc.path);
    }

    Ok(())
}

/// 处理 info 子命令：显示索引统计信息
fn handle_info(index_path: &Path) -> Result<()> {
    let storage = JsonStorage::new();
    let index = storage.load(index_path)?;

    let stats = index.stats();

    println!("索引统计信息：");
    println!("  文档数: {}", stats.doc_count);
    println!("  词汇量: {}", stats.vocab_size);
    println!("  倒排记录数: {}", stats.total_postings);
    println!("  索引文件: {}", index_path.display());

    // 计算平均文档长度
    let avg_doc_length = if stats.doc_count > 0 {
        let total: usize = index.documents.values().map(|d| d.doc_length).sum();
        total as f64 / stats.doc_count as f64
    } else {
        0.0
    };
    println!("  平均文档词数: {:.1}", avg_doc_length);

    Ok(())
}
