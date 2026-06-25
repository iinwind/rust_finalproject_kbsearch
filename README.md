# kbsearch - 本地知识库搜索系统

一个基于 Rust 的本地知识库全文搜索工具，支持中英文混合搜索，采用倒排索引与 TF-IDF 排序算法。

## 功能特性

- **多格式文档解析**：支持 `.txt`、`.md`、`.pdf`、`.docx` 四种格式
- **中英文混合分词**：基于 jieba 的中文分词 + 英文空格分词，自动识别 CJK 边界
- **TF-IDF 排序**：使用归一化词频 + 平滑逆文档频率进行相关性排序
- **索引批量构建**：扫描目录后批量解析文档并构建倒排索引
- **索引持久化**：索引序列化为 JSON 文件，支持保存与加载
- **搜索结果高亮**：终端 ANSI 高亮匹配关键词
- **自动跳过隐藏文件**：递归扫描时跳过 `.` 开头的文件和目录

## 编译运行方式

### 环境要求

- Rust 1.70+

### 编译

```bash
# debug 模式
cargo build

# release 模式
cargo build --release
```

编译产物位于 `target/debug/kbsearch` 或 `target/release/kbsearch`。

### 运行

通过 cargo 运行时，`--` 用于分隔 cargo 参数和程序参数：

```bash
cargo run -- <子命令> [选项]
```

直接运行编译产物则不需要 `--`：

```bash
./target/release/kbsearch <子命令> [选项]
```

## 使用方法

### 子命令一览

| 子命令            | 说明            |
| -------------- | ------------- |
| `index <目录>`   | 扫描目录并构建索引     |
| `search <查询词>` | 搜索关键词         |
| `list`         | 列出所有已索引文档     |
| `info`         | 显示索引统计信息      |
| `reindex <目录>` | 重新构建索引（覆盖旧索引） |

> 使用 `help` 可查看帮助信息

### 使用示例

**1. 构建索引**

```bash
cargo run -- index ./my-docs
```

**2. 搜索关键词**

```bash
cargo run -- search "rust"
```

**3. 限制结果数量**

```bash
cargo run -- search "rust" --limit 3
```

**4. 列出已索引文档**

```bash
cargo run -- list
```

**5. 查看索引统计**

```bash
cargo run -- info
```

**6. 重新构建索引**

```bash
cargo run -- reindex ./my-docs
```

## 依赖说明

### 运行时依赖

| 依赖                     | 版本   | 用途                 |
| ---------------------- | ---- | ------------------ |
| `clap`                 | 4    | 命令行参数解析（derive 模式） |
| `serde` / `serde_json` | 1    | 索引序列化与反序列化         |
| `pulldown-cmark`       | 0.11 | Markdown 文档纯文本提取   |
| `pdf-extract`          | 0.10 | PDF 文档文本提取         |
| `docx-lite`            | 0.2  | DOCX 文档文本提取        |
| `thiserror`            | 2    | 错误类型派生宏            |
| `walkdir`              | 2    | 目录递归遍历             |
| `dunce`                | 1    | Windows 路径规范化      |
| `jieba-rs`             | 0.7  | 中文分词               |

### 开发依赖

| 依赖         | 版本  | 用途            |
| ---------- | --- | ------------- |
| `tempfile` | 3   | 测试临时目录        |
| `zip`      | 0.6 | 构造测试用 DOCX 文件 |

## 项目结构

```
src/
├── main.rs        # CLI 入口，子命令分发
├── lib.rs         # 模块声明
├── error.rs       # 统一错误类型 KbError（thiserror 派生）
├── scanner.rs     # 文件扫描与类型检测
├── parser.rs      # 多格式文档解析器（Parser trait + 四种实现）
├── tokenizer.rs   # 中英文混合分词（Tokenizer trait + 三种实现）
├── indexer.rs     # 倒排索引构建与 TF-IDF 计算
├── storage.rs     # 索引持久化（IndexStorage trait + JSON 实现）
└── searcher.rs    # 搜索引擎（SearchEngine trait + TF-IDF 排序）
tests/
└── integration_test.rs  # 集成测试
```

## 代码测试

```bash
# 运行全部测试（50 个单元测试 + 8 个集成测试）
cargo test

# 仅运行单元测试
cargo test --lib

# 仅运行集成测试
cargo test --test integration_test
```

## 代码规范

```bash
cargo fmt      # 格式化代码
cargo clippy   # 静态检查
```

