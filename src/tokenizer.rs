use std::collections::HashSet;
use std::sync::OnceLock;

use jieba_rs::Jieba;

// ========== Tokenizer Trait ==========

/// 分词器 trait
pub trait Tokenizer: Send + Sync {
    /// 将文本分词，返回 token 列表
    fn tokenize(&self, text: &str) -> Vec<String>;
}

// ========== CJK 工具函数 ==========

/// 判断字符是否为 CJK 统一表意文字及相关字符
pub fn is_cjk(ch: char) -> bool {
    matches!(
        ch,
        '\u{4E00}'..='\u{9FFF}'
        | '\u{3400}'..='\u{4DBF}'
        | '\u{20000}'..='\u{2A6DF}'
        | '\u{2A700}'..='\u{2B73F}'
        | '\u{2B740}'..='\u{2B81F}'
        | '\u{F900}'..='\u{FAFF}'
        | '\u{2F800}'..='\u{2FA1F}'
        | '\u{3000}'..='\u{303F}'
        | '\u{FF00}'..='\u{FFEF}'
        | '\u{3040}'..='\u{309F}'
        | '\u{30A0}'..='\u{30FF}'
    )
}

// ========== CJK 边界切分 ==========

/// 文本段，标记为中文或非中文
#[derive(Debug, Clone, PartialEq)]
pub enum TextSegment {
    Cjk(String),
    NonCjk(String),
}

/// 按 CJK 边界将文本切分为交替的段
///
/// 示例: "Rust语言编程guide" -> [NonCjk("Rust"), Cjk("语言编程"), NonCjk("guide")]
pub fn segment_by_cjk(text: &str) -> Vec<TextSegment> {
    let mut segments = Vec::new();
    let mut current_cjk = String::new();
    let mut current_non_cjk = String::new();
    let mut in_cjk = false;

    for ch in text.chars() {
        let ch_is_cjk = is_cjk(ch);
        if ch_is_cjk {
            if !in_cjk && !current_non_cjk.is_empty() {
                segments.push(TextSegment::NonCjk(std::mem::take(&mut current_non_cjk)));
            }
            in_cjk = true;
            current_cjk.push(ch);
        } else {
            if in_cjk && !current_cjk.is_empty() {
                segments.push(TextSegment::Cjk(std::mem::take(&mut current_cjk)));
            }
            in_cjk = false;
            current_non_cjk.push(ch);
        }
    }

    if !current_cjk.is_empty() {
        segments.push(TextSegment::Cjk(current_cjk));
    }
    if !current_non_cjk.is_empty() {
        segments.push(TextSegment::NonCjk(current_non_cjk));
    }

    segments
}

// ========== 停用词 ==========

/// 英文停用词列表
const ENGLISH_STOP_WORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with", "by",
    "from", "is", "it", "that", "this", "was", "are", "be", "been", "being", "have", "has", "had",
    "do", "does", "did", "will", "would", "could", "should", "may", "might", "can", "shall", "not",
    "no", "nor", "so", "if", "then", "than", "too", "very", "just", "about", "above", "after",
    "again", "all", "also", "am", "as", "because", "before", "between", "both", "each", "few",
    "more", "most", "other", "own", "same", "some", "such", "up", "out", "only", "into", "over",
    "down", "here", "there", "when", "where", "why", "how", "what", "which", "who", "whom", "he",
    "she", "we", "they", "i", "me", "my", "you", "your",
];

/// 中文停用词列表
const CHINESE_STOP_WORDS: &[&str] = &[
    "的",
    "了",
    "在",
    "是",
    "我",
    "有",
    "和",
    "就",
    "不",
    "人",
    "都",
    "一",
    "一个",
    "上",
    "也",
    "很",
    "到",
    "说",
    "要",
    "去",
    "你",
    "会",
    "着",
    "没有",
    "看",
    "好",
    "自己",
    "这",
    "他",
    "她",
    "它",
    "们",
    "那",
    "被",
    "从",
    "把",
    "对",
    "与",
    "之",
    "以",
    "而",
    "但",
    "或",
    "所",
    "等",
    "吗",
    "吧",
    "呢",
    "啊",
    "呀",
    "哦",
    "哈",
    "嗯",
    "个",
    "中",
    "为",
    "年",
    "月",
    "日",
    "时",
    "分",
    "秒",
    "其",
    "可以",
    "没",
    "这个",
    "那个",
    "什么",
    "怎么",
    "如何",
    "为什么",
    "因为",
    "所以",
    "如果",
    "虽然",
    "不过",
    "然而",
];

// ========== Jieba 全局单例 ==========

/// Jieba 全局单例，首次调用时加载词典，线程安全
static JIEBA: OnceLock<Jieba> = OnceLock::new();

fn get_jieba() -> &'static Jieba {
    JIEBA.get_or_init(Jieba::new)
}

// ========== SimpleTokenizer ==========

/// 简单英文分词器
///
/// 处理流程：小写化 → 按非字母数字字符分割 → 过滤空串和停用词
pub struct SimpleTokenizer {
    stop_words: HashSet<String>,
}

impl SimpleTokenizer {
    pub fn new() -> Self {
        let stop_words: HashSet<String> =
            ENGLISH_STOP_WORDS.iter().map(|s| s.to_string()).collect();
        Self { stop_words }
    }
}

impl Default for SimpleTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Tokenizer for SimpleTokenizer {
    fn tokenize(&self, text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty() && s.len() > 1 && !self.stop_words.contains(*s))
            .map(|s| s.to_string())
            .collect()
    }
}

// ========== JiebaTokenizer (Phase 1) ==========

/// 基于 jieba 的纯中文分词器
///
/// 使用 jieba 的 search 模式进行分词，对长词进一步切分，提升搜索召回率
pub struct JiebaTokenizer {
    stop_words: HashSet<String>,
}

impl JiebaTokenizer {
    pub fn new() -> Self {
        let stop_words: HashSet<String> =
            CHINESE_STOP_WORDS.iter().map(|s| s.to_string()).collect();
        Self { stop_words }
    }
}

impl Default for JiebaTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Tokenizer for JiebaTokenizer {
    fn tokenize(&self, text: &str) -> Vec<String> {
        let jieba = get_jieba();
        jieba
            .cut_for_search(text, true)
            .into_iter()
            .map(|word| word.to_string())
            .filter(|word| {
                !word.is_empty()
                    && !self.stop_words.contains(word)
                    && word.chars().any(|c| c.is_alphanumeric() || is_cjk(c))
            })
            .collect()
    }
}

// ========== MixedTokenizer (Phase 2) ==========

/// 中英文混合分词器
///
/// 处理流程:
/// 1. 按 CJK 边界将文本切分为交替的中/英文段
/// 2. 对中文段: jieba search 模式分词 + 中文停用词过滤
/// 3. 对英文段: 小写化 + 非字母数字分割 + 英文停用词过滤 + 单字过滤
/// 4. 合并所有 token
pub struct MixedTokenizer {
    en_stop_words: HashSet<String>,
    zh_stop_words: HashSet<String>,
}

impl MixedTokenizer {
    pub fn new() -> Self {
        let en_stop_words: HashSet<String> =
            ENGLISH_STOP_WORDS.iter().map(|s| s.to_string()).collect();
        let zh_stop_words: HashSet<String> =
            CHINESE_STOP_WORDS.iter().map(|s| s.to_string()).collect();
        Self {
            en_stop_words,
            zh_stop_words,
        }
    }

    fn tokenize_cjk(&self, text: &str) -> Vec<String> {
        let jieba = get_jieba();
        jieba
            .cut_for_search(text, true)
            .into_iter()
            .map(|word| word.to_string())
            .filter(|word| {
                !word.is_empty()
                    && !self.zh_stop_words.contains(word)
                    && word.chars().any(|c| c.is_alphanumeric() || is_cjk(c))
            })
            .collect()
    }

    fn tokenize_non_cjk(&self, text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty() && s.len() > 1 && !self.en_stop_words.contains(*s))
            .map(|s| s.to_string())
            .collect()
    }
}

impl Default for MixedTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Tokenizer for MixedTokenizer {
    fn tokenize(&self, text: &str) -> Vec<String> {
        let segments = segment_by_cjk(text);
        let mut tokens = Vec::new();

        for segment in segments {
            match segment {
                TextSegment::Cjk(s) => {
                    tokens.extend(self.tokenize_cjk(&s));
                }
                TextSegment::NonCjk(s) => {
                    tokens.extend(self.tokenize_non_cjk(&s));
                }
            }
        }

        tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_tokenizer() {
        let tokenizer = SimpleTokenizer::new();
        let tokens = tokenizer.tokenize("Hello, World! This is a test.");
        assert!(tokens.contains(&"hello".to_string()));
        assert!(tokens.contains(&"world".to_string()));
        assert!(tokens.contains(&"test".to_string()));
        assert!(!tokens.contains(&"this".to_string()));
        assert!(!tokens.contains(&"is".to_string()));
        assert!(!tokens.contains(&"a".to_string()));
    }

    #[test]
    fn test_simple_tokenizer_single_char_filtered() {
        let tokenizer = SimpleTokenizer::new();
        let tokens = tokenizer.tokenize("I am a person");
        assert!(!tokens.iter().any(|t| t.len() == 1));
    }

    #[test]
    fn test_mixed_tokenizer_chinese() {
        let tokenizer = MixedTokenizer::new();
        let tokens = tokenizer.tokenize("Rust语言编程");
        assert!(tokens.contains(&"rust".to_string()));
        assert!(tokens.contains(&"语言".to_string()));
        assert!(tokens.contains(&"编程".to_string()));
    }

    #[test]
    fn test_mixed_tokenizer_mixed() {
        let tokenizer = MixedTokenizer::new();
        let tokens = tokenizer.tokenize("使用Rust开发Web应用");
        assert!(tokens.contains(&"使用".to_string()));
        assert!(tokens.contains(&"rust".to_string()));
        assert!(tokens.contains(&"开发".to_string()));
        assert!(tokens.contains(&"web".to_string()));
        assert!(tokens.contains(&"应用".to_string()));
    }

    #[test]
    fn test_mixed_tokenizer_chinese_single_char_not_filtered() {
        let tokenizer = MixedTokenizer::new();
        // jieba 的 cut_for_search 对长词会产出子词，单字 token 不会被 len>1 规则过滤
        // 测试非停用词的单字 token 如果被 jieba 产出，会保留在结果中
        let tokens = tokenizer.tokenize("山水");
        assert!(tokens.contains(&"山水".to_string()));
    }

    #[test]
    fn test_mixed_tokenizer_english_single_char_filtered() {
        let tokenizer = MixedTokenizer::new();
        let tokens = tokenizer.tokenize("I am a person");
        assert!(!tokens
            .iter()
            .any(|t| t.chars().all(|c| c.is_ascii_alphabetic()) && t.len() == 1));
    }

    #[test]
    fn test_mixed_tokenizer_english_only() {
        let tokenizer = MixedTokenizer::new();
        let tokens = tokenizer.tokenize("Hello, World! This is a test.");
        assert!(tokens.contains(&"hello".to_string()));
        assert!(tokens.contains(&"world".to_string()));
        assert!(tokens.contains(&"test".to_string()));
        assert!(!tokens.contains(&"this".to_string()));
        assert!(!tokens.contains(&"is".to_string()));
    }

    #[test]
    fn test_mixed_tokenizer_chinese_stop_words() {
        let tokenizer = MixedTokenizer::new();
        let tokens = tokenizer.tokenize("这是一个测试");
        assert!(!tokens.contains(&"的".to_string()));
        assert!(!tokens.contains(&"是".to_string()));
        assert!(!tokens.contains(&"一个".to_string()));
        assert!(tokens.contains(&"测试".to_string()));
    }

    #[test]
    fn test_mixed_tokenizer_search_mode() {
        let tokenizer = MixedTokenizer::new();
        let tokens = tokenizer.tokenize("搜索引擎优化");
        assert!(tokens.contains(&"搜索".to_string()));
        assert!(tokens.contains(&"引擎".to_string()));
        assert!(tokens.contains(&"搜索引擎".to_string()));
        assert!(tokens.contains(&"优化".to_string()));
    }

    #[test]
    fn test_segment_by_cjk_mixed() {
        let segments = segment_by_cjk("Rust语言编程guide");
        assert_eq!(
            segments,
            vec![
                TextSegment::NonCjk("Rust".to_string()),
                TextSegment::Cjk("语言编程".to_string()),
                TextSegment::NonCjk("guide".to_string()),
            ]
        );
    }

    #[test]
    fn test_segment_by_cjk_pure_chinese() {
        let segments = segment_by_cjk("中文文本");
        assert_eq!(segments, vec![TextSegment::Cjk("中文文本".to_string())]);
    }

    #[test]
    fn test_segment_by_cjk_pure_english() {
        let segments = segment_by_cjk("Hello World");
        assert_eq!(
            segments,
            vec![TextSegment::NonCjk("Hello World".to_string())]
        );
    }

    #[test]
    fn test_jieba_tokenizer() {
        let tokenizer = JiebaTokenizer::new();
        let tokens = tokenizer.tokenize("搜索引擎优化");
        assert!(tokens.contains(&"搜索引擎".to_string()));
        assert!(tokens.contains(&"优化".to_string()));
    }
}
