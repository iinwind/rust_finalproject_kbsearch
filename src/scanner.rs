use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::error::{KbError, Result};

/// 支持的文件类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileType {
    Markdown,
    PlainText,
    Pdf,
    Docx,
}

/// 扫描到的文件条目
#[derive(Debug, Clone)]
pub struct ScannedFile {
    pub path: PathBuf,
    pub file_type: FileType,
}

/// 根据文件扩展名判断文件类型
///
/// 支持的扩展名：
/// - `.md` → Markdown
/// - `.txt` → PlainText
/// - `.pdf` → Pdf
/// - `.docx` → Docx
/// - 其他 → None
pub fn detect_file_type(path: &Path) -> Option<FileType> {
    match path.extension()?.to_str()?.to_lowercase().as_str() {
        "md" | "markdown" => Some(FileType::Markdown),
        "txt" => Some(FileType::PlainText),
        "pdf" => Some(FileType::Pdf),
        "docx" => Some(FileType::Docx),
        _ => None,
    }
}

/// 判断路径是否为隐藏文件或隐藏目录中的文件
///
/// 以 `.` 开头的文件或目录视为隐藏
fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with('.'))
}

/// 递归扫描目录，收集所有支持的文件
///
/// 会自动跳过隐藏文件和隐藏目录
pub fn scan_directory(dir: &Path) -> Result<Vec<ScannedFile>> {
    if !dir.exists() {
        return Err(KbError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Directory not found: {}", dir.display()),
        )));
    }

    if !dir.is_dir() {
        return Err(KbError::Io(std::io::Error::new(
            std::io::ErrorKind::NotADirectory,
            format!("Path is not a directory: {}", dir.display()),
        )));
    }

    let mut files = Vec::new();

    let root = dunce::canonicalize(dir)?;

    for entry in WalkDir::new(&root).into_iter().filter_entry(|e| {
        // 根目录本身不过滤，其余跳过隐藏文件/目录
        e.path() == root || !is_hidden(e.path())
    }) {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue, // 跳过无权限访问的条目
        };

        let path = entry.path();

        // 只处理文件，跳过目录
        if !entry.file_type().is_file() {
            continue;
        }

        // 检测文件类型
        if let Some(file_type) = detect_file_type(path) {
            files.push(ScannedFile {
                path: path.to_path_buf(),
                file_type,
            });
        }
    }

    // 按路径排序，保证输出稳定
    files.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// 创建临时测试目录结构
    fn create_test_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();

        fs::write(base.join("readme.md"), "# Hello").unwrap();
        fs::write(base.join("notes.txt"), "Some notes").unwrap();
        fs::write(base.join("data.csv"), "a,b,c").unwrap();

        // 隐藏文件（应被跳过）
        fs::write(base.join(".hidden.md"), "Hidden").unwrap();

        // 子目录
        fs::create_dir_all(base.join("sub")).unwrap();
        fs::write(base.join("sub/doc.md"), "Sub doc").unwrap();

        // 隐藏子目录（应被跳过）
        fs::create_dir_all(base.join(".hidden_dir")).unwrap();
        fs::write(base.join(".hidden_dir/secret.md"), "Secret").unwrap();

        dir
    }

    #[test]
    fn test_detect_file_type() {
        assert_eq!(
            detect_file_type(Path::new("test.md")),
            Some(FileType::Markdown)
        );
        assert_eq!(
            detect_file_type(Path::new("test.MD")),
            Some(FileType::Markdown)
        );
        assert_eq!(
            detect_file_type(Path::new("test.markdown")),
            Some(FileType::Markdown)
        );
        assert_eq!(
            detect_file_type(Path::new("test.txt")),
            Some(FileType::PlainText)
        );
        assert_eq!(
            detect_file_type(Path::new("report.pdf")),
            Some(FileType::Pdf)
        );
        assert_eq!(
            detect_file_type(Path::new("document.docx")),
            Some(FileType::Docx)
        );
        assert_eq!(
            detect_file_type(Path::new("report.PDF")),
            Some(FileType::Pdf)
        );
        assert_eq!(
            detect_file_type(Path::new("document.DOCX")),
            Some(FileType::Docx)
        );
        assert_eq!(detect_file_type(Path::new("test.csv")), None);
        assert_eq!(detect_file_type(Path::new("test")), None);
    }

    #[test]
    fn test_scan_directory() {
        let dir = create_test_dir();
        let files = scan_directory(dir.path()).unwrap();

        // 应找到 3 个文件：readme.md, notes.txt, sub/doc.md
        assert_eq!(files.len(), 3);

        let paths: Vec<String> = files
            .iter()
            .map(|f| f.path.file_name().unwrap().to_str().unwrap().to_string())
            .collect();
        assert!(paths.contains(&"readme.md".to_string()));
        assert!(paths.contains(&"notes.txt".to_string()));
    }

    #[test]
    fn test_scan_skips_hidden() {
        let dir = create_test_dir();
        let files = scan_directory(dir.path()).unwrap();

        // 不应包含隐藏文件和隐藏目录中的文件
        let paths: Vec<String> = files
            .iter()
            .map(|f| f.path.to_str().unwrap().to_string())
            .collect();
        assert!(!paths.iter().any(|p| p.contains(".hidden")));
    }

    #[test]
    fn test_scan_nonexistent_directory() {
        let result = scan_directory(Path::new("/nonexistent/path"));
        assert!(result.is_err());
    }
}
