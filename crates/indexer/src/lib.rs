use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub path: String,
    pub language: String,
    pub bytes: u64,
    pub hash: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexSnapshot {
    pub files: Vec<FileEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexSummary {
    pub files: usize,
    pub bytes: u64,
    pub by_language: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexDiff {
    pub added: Vec<FileEntry>,
    pub modified: Vec<FileEntry>,
    pub deleted: Vec<FileEntry>,
}

impl FileEntry {
    pub fn to_compact_json(&self) -> String {
        format!(
            "{{\"path\":\"{}\",\"language\":\"{}\",\"bytes\":{},\"hash\":{}}}",
            escape_json(&self.path),
            escape_json(&self.language),
            self.bytes,
            self.hash
        )
    }
}

impl IndexSnapshot {
    pub fn scan(root: impl AsRef<Path>) -> io::Result<Self> {
        let root = root.as_ref();
        let mut files = Vec::new();
        scan_dir(root, root, &mut files)?;
        files.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(Self { files })
    }

    pub fn summary(&self) -> IndexSummary {
        let mut summary = IndexSummary::default();
        summary.files = self.files.len();

        for file in &self.files {
            summary.bytes += file.bytes;
            *summary
                .by_language
                .entry(file.language.clone())
                .or_default() += 1;
        }

        summary
    }

    pub fn diff(&self, previous: &IndexSnapshot) -> IndexDiff {
        let current_by_path = self
            .files
            .iter()
            .map(|file| (file.path.as_str(), file))
            .collect::<BTreeMap<_, _>>();
        let previous_by_path = previous
            .files
            .iter()
            .map(|file| (file.path.as_str(), file))
            .collect::<BTreeMap<_, _>>();
        let all_paths = current_by_path
            .keys()
            .chain(previous_by_path.keys())
            .copied()
            .collect::<BTreeSet<_>>();

        let mut diff = IndexDiff::default();

        for path in all_paths {
            match (current_by_path.get(path), previous_by_path.get(path)) {
                (Some(current), None) => diff.added.push((*current).clone()),
                (Some(current), Some(previous)) if current.hash != previous.hash => {
                    diff.modified.push((*current).clone());
                }
                (None, Some(previous)) => diff.deleted.push((*previous).clone()),
                _ => {}
            }
        }

        diff
    }

    pub fn to_tsv(&self) -> String {
        let mut output = String::new();
        for file in &self.files {
            output.push_str(&escape_field(&file.path));
            output.push('\t');
            output.push_str(&escape_field(&file.language));
            output.push('\t');
            output.push_str(&file.bytes.to_string());
            output.push('\t');
            output.push_str(&file.hash.to_string());
            output.push('\n');
        }
        output
    }

    pub fn from_tsv(input: &str) -> Result<Self, String> {
        let mut files = Vec::new();

        for (index, line) in input.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }

            let fields = line.split('\t').collect::<Vec<_>>();
            if fields.len() != 4 {
                return Err(format!(
                    "line {}: expected 4 tab-separated fields",
                    index + 1
                ));
            }

            files.push(FileEntry {
                path: unescape_field(fields[0])?,
                language: unescape_field(fields[1])?,
                bytes: fields[2]
                    .parse::<u64>()
                    .map_err(|error| format!("line {}: invalid byte count: {error}", index + 1))?,
                hash: fields[3]
                    .parse::<u64>()
                    .map_err(|error| format!("line {}: invalid hash: {error}", index + 1))?,
            });
        }

        Ok(Self { files })
    }

    pub fn to_compact_json(&self) -> String {
        let files = self
            .files
            .iter()
            .map(FileEntry::to_compact_json)
            .collect::<Vec<_>>()
            .join(",");
        format!("[{files}]")
    }
}

impl IndexSummary {
    pub fn to_compact_json(&self) -> String {
        let by_language = self
            .by_language
            .iter()
            .map(|(language, count)| format!("\"{}\":{}", escape_json(language), count))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"files\":{},\"bytes\":{},\"by_language\":{{{}}}}}",
            self.files, self.bytes, by_language
        )
    }
}

impl IndexDiff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.modified.is_empty() && self.deleted.is_empty()
    }

    pub fn to_compact_json(&self) -> String {
        format!(
            "{{\"added\":{},\"modified\":{},\"deleted\":{}}}",
            entries_json(&self.added),
            entries_json(&self.modified),
            entries_json(&self.deleted)
        )
    }
}

pub fn detect_language(path: &Path) -> String {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("rs") => "rust",
        Some("dart") => "dart",
        Some("ts") | Some("tsx") => "typescript",
        Some("js") | Some("jsx") => "javascript",
        Some("json") => "json",
        Some("toml") => "toml",
        Some("yaml") | Some("yml") => "yaml",
        Some("md") => "markdown",
        Some("py") => "python",
        Some("kt") | Some("kts") => "kotlin",
        Some("java") => "java",
        Some("swift") => "swift",
        Some("html") => "html",
        Some("css") => "css",
        Some("sh") => "shell",
        _ => "unknown",
    }
    .to_string()
}

fn scan_dir(root: &Path, dir: &Path, files: &mut Vec<FileEntry>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();

        if entry.file_type()?.is_dir() {
            if should_skip_dir(&file_name) {
                continue;
            }
            scan_dir(root, &path, files)?;
            continue;
        }

        if !entry.file_type()?.is_file() {
            continue;
        }

        let bytes = fs::read(&path)?;
        let relative_path = relative_path(root, &path);
        files.push(FileEntry {
            language: detect_language(&path),
            bytes: bytes.len() as u64,
            hash: stable_hash(&bytes),
            path: relative_path,
        });
    }

    Ok(())
}

fn should_skip_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | ".kosh" | "target" | "node_modules" | ".dart_tool" | "build" | ".gradle"
    )
}

fn stable_hash(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn entries_json(entries: &[FileEntry]) -> String {
    let entries = entries
        .iter()
        .map(FileEntry::to_compact_json)
        .collect::<Vec<_>>()
        .join(",");
    format!("[{entries}]")
}

fn escape_json(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

fn escape_field(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
}

fn unescape_field(input: &str) -> Result<String, String> {
    let mut output = String::new();
    let mut chars = input.chars();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            output.push(ch);
            continue;
        }

        match chars.next() {
            Some('\\') => output.push('\\'),
            Some('t') => output.push('\t'),
            Some('n') => output.push('\n'),
            Some(other) => return Err(format!("unsupported escape sequence: \\{other}")),
            None => return Err("trailing escape in field".to_string()),
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_common_languages() {
        assert_eq!(detect_language(Path::new("main.rs")), "rust");
        assert_eq!(detect_language(Path::new("app.dart")), "dart");
        assert_eq!(detect_language(Path::new("README.md")), "markdown");
    }

    #[test]
    fn summarizes_snapshot() {
        let snapshot = IndexSnapshot {
            files: vec![
                FileEntry {
                    path: "src/lib.rs".to_string(),
                    language: "rust".to_string(),
                    bytes: 10,
                    hash: 1,
                },
                FileEntry {
                    path: "README.md".to_string(),
                    language: "markdown".to_string(),
                    bytes: 5,
                    hash: 2,
                },
            ],
        };

        let summary = snapshot.summary();
        assert_eq!(summary.files, 2);
        assert_eq!(summary.bytes, 15);
        assert_eq!(summary.by_language["rust"], 1);
    }

    #[test]
    fn diffs_snapshots() {
        let previous = IndexSnapshot {
            files: vec![
                FileEntry {
                    path: "a.rs".to_string(),
                    language: "rust".to_string(),
                    bytes: 1,
                    hash: 1,
                },
                FileEntry {
                    path: "gone.rs".to_string(),
                    language: "rust".to_string(),
                    bytes: 1,
                    hash: 1,
                },
            ],
        };
        let current = IndexSnapshot {
            files: vec![
                FileEntry {
                    path: "a.rs".to_string(),
                    language: "rust".to_string(),
                    bytes: 2,
                    hash: 2,
                },
                FileEntry {
                    path: "new.rs".to_string(),
                    language: "rust".to_string(),
                    bytes: 1,
                    hash: 3,
                },
            ],
        };

        let diff = current.diff(&previous);
        assert_eq!(diff.added[0].path, "new.rs");
        assert_eq!(diff.modified[0].path, "a.rs");
        assert_eq!(diff.deleted[0].path, "gone.rs");
    }

    #[test]
    fn round_trips_tsv() {
        let snapshot = IndexSnapshot {
            files: vec![FileEntry {
                path: "src/lib.rs".to_string(),
                language: "rust".to_string(),
                bytes: 10,
                hash: 42,
            }],
        };

        let parsed = IndexSnapshot::from_tsv(&snapshot.to_tsv()).unwrap();
        assert_eq!(parsed, snapshot);
    }
}
