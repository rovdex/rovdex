use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceMapOptions {
    pub max_files: usize,
    pub max_file_bytes: u64,
    pub max_symbols_per_file: usize,
}

impl Default for WorkspaceMapOptions {
    fn default() -> Self {
        Self {
            max_files: 250,
            max_file_bytes: 256 * 1024,
            max_symbols_per_file: 8,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceMap {
    pub root: String,
    pub scanned_files: usize,
    pub total_lines: usize,
    pub languages: BTreeMap<String, usize>,
    pub directories: Vec<DirectorySummary>,
    pub key_files: Vec<FileSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectorySummary {
    pub path: String,
    pub file_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSummary {
    pub path: String,
    pub language: String,
    pub lines: usize,
    pub symbols: Vec<String>,
}

impl WorkspaceMap {
    pub fn scan(root: impl Into<PathBuf>) -> Result<Self> {
        Self::scan_with_options(root, &WorkspaceMapOptions::default())
    }

    pub fn scan_with_options(root: impl Into<PathBuf>, options: &WorkspaceMapOptions) -> Result<Self> {
        let root = root.into();
        let mut languages = BTreeMap::new();
        let mut directories = HashMap::<String, usize>::new();
        let mut key_files = Vec::new();
        let mut scanned_files = 0usize;
        let mut total_lines = 0usize;

        for entry in WalkDir::new(&root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| should_visit(entry))
            .filter_map(std::result::Result::ok)
        {
            if !entry.file_type().is_file() || scanned_files >= options.max_files {
                continue;
            }

            let Some(relative_path) = relative_display_path(entry.path(), &root) else {
                continue;
            };
            let Some(language) = detect_language(entry.path()) else {
                continue;
            };

            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };
            if metadata.len() > options.max_file_bytes {
                continue;
            }

            let content = match fs::read_to_string(entry.path()) {
                Ok(content) => content,
                Err(_) => continue,
            };

            scanned_files += 1;
            let line_count = content.lines().count();
            total_lines += line_count;
            *languages.entry(language.clone()).or_insert(0) += 1;

            let directory = relative_path
                .rsplit_once('/')
                .map(|(dir, _)| dir.to_string())
                .unwrap_or_else(|| ".".to_string());
            *directories.entry(directory).or_insert(0) += 1;

            if is_key_file(&relative_path) || key_files.len() < 16 {
                key_files.push(FileSummary {
                    path: relative_path,
                    language,
                    lines: line_count,
                    symbols: extract_symbols(&content, options.max_symbols_per_file),
                });
            }
        }

        key_files.sort_by(|a, b| {
            is_key_file(&b.path)
                .cmp(&is_key_file(&a.path))
                .then_with(|| a.path.cmp(&b.path))
        });
        key_files.truncate(16);

        let mut directories = directories
            .into_iter()
            .map(|(path, file_count)| DirectorySummary { path, file_count })
            .collect::<Vec<_>>();
        directories.sort_by(|a, b| b.file_count.cmp(&a.file_count).then_with(|| a.path.cmp(&b.path)));
        directories.truncate(8);

        Ok(Self {
            root: root.display().to_string(),
            scanned_files,
            total_lines,
            languages,
            directories,
            key_files,
        })
    }

    pub fn render_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("Workspace map:\n");
        out.push_str(&format!(
            "- root: {}\n- scanned_files: {}\n- total_lines: {}\n",
            self.root, self.scanned_files, self.total_lines
        ));

        if !self.languages.is_empty() {
            let languages = self
                .languages
                .iter()
                .map(|(language, count)| format!("{language}({count})"))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("- languages: {languages}\n"));
        }

        if !self.directories.is_empty() {
            out.push_str("- top_directories:\n");
            for directory in &self.directories {
                out.push_str(&format!("  - {} ({})\n", directory.path, directory.file_count));
            }
        }

        if !self.key_files.is_empty() {
            out.push_str("- key_files:\n");
            for file in &self.key_files {
                out.push_str(&format!(
                    "  - {} [{}] lines={}",
                    file.path, file.language, file.lines
                ));
                if !file.symbols.is_empty() {
                    out.push_str(&format!(" symbols={}", file.symbols.join(", ")));
                }
                out.push('\n');
            }
        }

        out
    }
}

fn should_visit(entry: &DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    if entry.depth() == 0 {
        return true;
    }

    if entry.file_type().is_dir() {
        return !matches!(
            name.as_ref(),
            ".git" | "target" | "node_modules" | "dist" | "build" | ".next" | ".turbo" | ".idea" | ".rovdex"
        );
    }

    true
}

fn relative_display_path(path: &Path, root: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
}

fn detect_language(path: &Path) -> Option<String> {
    let file_name = path.file_name()?.to_string_lossy();
    if file_name == "Cargo.toml" {
        return Some("toml".to_string());
    }
    if file_name == "package.json" {
        return Some("json".to_string());
    }
    if file_name.eq_ignore_ascii_case("readme.md") {
        return Some("markdown".to_string());
    }

    let extension = path.extension()?.to_string_lossy().to_ascii_lowercase();
    let language = match extension.as_str() {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "go" => "go",
        "py" => "python",
        "java" => "java",
        "kt" => "kotlin",
        "swift" => "swift",
        "toml" => "toml",
        "json" => "json",
        "yml" | "yaml" => "yaml",
        "md" => "markdown",
        "sql" => "sql",
        "sh" => "shell",
        "php" => "php",
        _ => return None,
    };

    Some(language.to_string())
}

fn is_key_file(path: &str) -> bool {
    matches!(
        path,
        "Cargo.toml" | "Cargo.lock" | "package.json" | "README.md" | "src/main.rs" | "src/lib.rs"
    ) || path.ends_with("/Cargo.toml")
        || path.ends_with("/main.rs")
        || path.ends_with("/lib.rs")
        || path.ends_with("/mod.rs")
}

fn extract_symbols(content: &str, max_symbols: usize) -> Vec<String> {
    let patterns = [
        Regex::new(r"^\s*pub\s+struct\s+([A-Za-z_][A-Za-z0-9_]*)").expect("struct regex"),
        Regex::new(r"^\s*struct\s+([A-Za-z_][A-Za-z0-9_]*)").expect("struct regex"),
        Regex::new(r"^\s*pub\s+enum\s+([A-Za-z_][A-Za-z0-9_]*)").expect("enum regex"),
        Regex::new(r"^\s*enum\s+([A-Za-z_][A-Za-z0-9_]*)").expect("enum regex"),
        Regex::new(r"^\s*pub\s+trait\s+([A-Za-z_][A-Za-z0-9_]*)").expect("trait regex"),
        Regex::new(r"^\s*trait\s+([A-Za-z_][A-Za-z0-9_]*)").expect("trait regex"),
        Regex::new(r"^\s*(?:pub\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)").expect("fn regex"),
        Regex::new(r"^\s*(?:export\s+)?(?:async\s+)?function\s+([A-Za-z_][A-Za-z0-9_]*)")
            .expect("function regex"),
        Regex::new(r"^\s*(?:export\s+)?class\s+([A-Za-z_][A-Za-z0-9_]*)").expect("class regex"),
    ];

    let mut symbols = Vec::new();
    for line in content.lines() {
        for pattern in &patterns {
            if let Some(captures) = pattern.captures(line) {
                symbols.push(captures[1].to_string());
                break;
            }
        }
        if symbols.len() >= max_symbols {
            break;
        }
    }
    symbols
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("rovdex-workspace-map-{unique}"));
        fs::create_dir_all(path.join("src")).expect("temp dir");
        path
    }

    #[test]
    fn scans_workspace_and_extracts_symbols() {
        let root = temp_dir();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
        )
        .expect("cargo");
        fs::write(
            root.join("src/main.rs"),
            "pub struct App;\nfn main() {}\n",
        )
        .expect("main");

        let map = WorkspaceMap::scan(&root).expect("workspace map");

        assert_eq!(map.scanned_files, 2);
        assert!(map.languages.contains_key("rust"));
        assert!(map
            .key_files
            .iter()
            .any(|file| file.path == "src/main.rs" && file.symbols.iter().any(|symbol| symbol == "App")));

        let _ = fs::remove_dir_all(root);
    }
}
