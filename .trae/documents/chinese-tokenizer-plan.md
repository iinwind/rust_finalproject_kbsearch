# 中文及中英混合分词支持实现计划

## Context

当前 `kbsearch` 的 `SimpleTokenizer` 完全基于英文思维设计：按非字母数字字符分割、过滤单字符词、仅含英文停用词。对中文文档，连续中文文本会被当成一个巨大 token，单字查询被丢弃，导致中文搜索几乎不可用。

目标：先实现 jieba 中文分词器（方案一），再迭代到中英混合分词器（方案二），使系统正确支持中文、英文及混合文档。

## 实现步骤

### 1. Cargo.toml 添加依赖

添加 `jieba-rs = "0.7"`。不需要 `once_cell`，使用标准库 `std::sync::OnceLock` 管理 Jieba 全局单例。

### 2. 新建 `src/tokenizer.rs` — 核心工作

从 `indexer.rs` 迁移 `Tokenizer` trait 和 `SimpleTokenizer`（逻辑不变），并新增：

- **`is_cjk(ch)`**：判断字符是否为 CJK 统一表意文字及相关符号（覆盖 U+4E00..U+9FFF、扩展区、全角字符等）
- **`TextSegment` 枚举**：`Cjk(String)` / `NonCjk(String)`
- **`segment_by_cjk(text)`**：按 CJK 边界将文本切分为交替段，如 `"Rust语言编程guide"` → `[NonCjk("Rust"), Cjk("语言编程"), NonCjk("guide")]`
- **`CHINESE_STOP_WORDS`**：约 80 个中文虚词常量（的、了、在、是、...）
- **`JiebaTokenizer`**：实现 `Tokenizer` trait，调用 `jieba.cut_for_search(text, true)`，过滤停用词但**保留单字**（中文单字有意义）
- **`MixedTokenizer`**：最终推荐分词器。先 `segment_by_cjk` 切段，中文段走 jieba + 中文停用词，英文段走 `SimpleTokenizer` 逻辑（小写+分割+英文停用词+过滤单字），合并所有 token
- **`JIEBA: OnceLock<Jieba>`**：全局单例，首次调用时加载词典，线程安全

分词效果示例：
| 输入 | 输出 tokens |
|---|---|
| `"Rust语言编程"` | `["rust", "语", "言", "语言", "编", "程", "编程"]` |
| `"搜索引擎优化SEO"` | `["搜索", "引擎", "搜索引擎", "优化", "seo"]` |
| `"Hello World"` | `["hello", "world"]` |
| `"这是一个测试"` | `["测试"]` (停用词被过滤) |

### 3. 修改 `src/indexer.rs`

- 删除 `Tokenizer` trait 和 `SimpleTokenizer` 定义（约 50 行）
- 添加 `pub use crate::tokenizer::{SimpleTokenizer, Tokenizer};` 保持向后兼容
- 测试中的 `use super::SimpleTokenizer` 改为从新模块导入

### 4. 修改 `src/lib.rs`

添加 `pub mod tokenizer;`

### 5. 修改 `src/main.rs`

- 导入：`SimpleTokenizer` → `MixedTokenizer`
- `handle_index` 和 `handle_search` 中：`SimpleTokenizer::new()` → `MixedTokenizer::new()`

### 6. 修改 `src/searcher.rs`

- 导入：添加 `use crate::tokenizer::Tokenizer;`
- `generate_snippet`：添加 UTF-8 字符边界检查（`is_char_boundary`），防止多字节中文被截断；限制空格查找范围为 10 字节，避免中文无空格文本中跳太远

### 7. 修改 `src/storage.rs` 测试

`make_test_index` 中 `SimpleTokenizer::new()` → `MixedTokenizer::new()`，导入相应更新。

### 8. 新增单元测试

在 `tokenizer.rs` 中添加测试：
- `test_mixed_tokenizer_chinese`：纯中文分词
- `test_mixed_tokenizer_mixed`：中英混合
- `test_mixed_tokenizer_chinese_single_char_kept`：中文单字保留
- `test_mixed_tokenizer_english_single_char_filtered`：英文单字过滤
- `test_mixed_tokenizer_english_only`：纯英文不变
- `test_mixed_tokenizer_chinese_stop_words`：中文停用词
- `test_mixed_tokenizer_search_mode`：jieba search 模式产生子词
- `test_segment_by_cjk`：CJK 边界切分
- `test_jieba_tokenizer`：JiebaTokenizer 基本功能

在 `searcher.rs` 测试中添加中文搜索测试。

## 关键设计决策

1. **MixedTokenizer 作为唯一入口**：无需文档级语言检测，`segment_by_cjk` 自动处理混合文本
2. **cut_for_search 而非 cut**：search 模式同时产生子词和完整词（如 "搜索引擎" → ["搜索", "引擎", "搜索引擎"]），显著提升召回率
3. **索引和搜索用同一个分词器**：结构性保证一致性
4. **中文单字保留、英文单字过滤**：两种语言过滤规则不同
5. **已有索引需重建**：旧索引的中文 token 粒度错误，更换分词器后须重新执行 `index` 命令

## 关键文件

| 文件 | 操作 |
|---|---|
| `src/tokenizer.rs` | 新建 |
| `src/indexer.rs` | 修改（迁移分词器定义） |
| `src/lib.rs` | 修改（注册模块） |
| `src/main.rs` | 修改（换分词器） |
| `src/searcher.rs` | 修改（UTF-8 边界 + 导入） |
| `src/storage.rs` | 修改（测试导入） |
| `Cargo.toml` | 修改（添加 jieba-rs） |

## 验证方式

1. `cargo test` — 全部单元测试通过
2. `cargo run -- index <含中文文档的目录>` — 构建索引，观察词汇量增加
3. `cargo run -- search 语言` — 验证中文搜索命中
4. `cargo run -- search rust` — 验证英文搜索不退化
5. `cargo run -- search "Rust语言"` — 验证混合搜索
