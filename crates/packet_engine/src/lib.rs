use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketRecord {
    pub name: String,
    pub files: Vec<String>,
    pub symbols: Vec<String>,
    pub created_at: u64,
}

impl PacketRecord {
    pub fn new(
        name: impl Into<String>,
        files: Vec<String>,
        symbols: Vec<String>,
        created_at: u64,
    ) -> Self {
        Self {
            name: name.into(),
            files,
            symbols,
            created_at,
        }
    }

    pub fn to_compact_json(&self) -> String {
        let files_json = self
            .files
            .iter()
            .map(|f| format!("\"{}\"", escape_json(f)))
            .collect::<Vec<_>>()
            .join(",");
        let symbols_json = self
            .symbols
            .iter()
            .map(|s| format!("\"{}\"", escape_json(s)))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"name\":\"{}\",\"files\":[{}],\"symbols\":[{}],\"created_at\":{}}}",
            escape_json(&self.name),
            files_json,
            symbols_json,
            self.created_at
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PacketStore {
    records: Vec<PacketRecord>,
}

impl PacketStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::new());
        }
        let contents = fs::read_to_string(path)?;
        Self::from_tsv(&contents).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn save(&self, path: impl AsRef<Path>) -> io::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.to_tsv())
    }

    pub fn upsert(&mut self, record: PacketRecord) {
        if let Some(existing) = self.records.iter_mut().find(|r| r.name == record.name) {
            *existing = record;
        } else {
            self.records.push(record);
        }
    }

    pub fn get(&self, name: &str) -> Option<&PacketRecord> {
        self.records.iter().find(|r| r.name == name)
    }

    pub fn delete(&mut self, name: &str) -> bool {
        let before = self.records.len();
        self.records.retain(|r| r.name != name);
        self.records.len() < before
    }

    pub fn records(&self) -> &[PacketRecord] {
        &self.records
    }

    pub fn from_tsv(input: &str) -> Result<Self, String> {
        let mut store = Self::new();

        for (index, line) in input.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }

            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() != 4 {
                return Err(format!(
                    "line {}: expected 4 tab-separated fields, got {}",
                    index + 1,
                    fields.len()
                ));
            }

            let name = unescape_field(fields[0])?;
            let files = parse_pipe_list(fields[1])?;
            let symbols = parse_pipe_list(fields[2])?;
            let created_at: u64 = fields[3]
                .parse()
                .map_err(|_| format!("line {}: invalid created_at", index + 1))?;

            store.upsert(PacketRecord::new(name, files, symbols, created_at));
        }

        Ok(store)
    }

    pub fn to_tsv(&self) -> String {
        let mut output = String::new();

        for record in &self.records {
            output.push_str(&escape_field(&record.name));
            output.push('\t');
            output.push_str(&format_pipe_list(&record.files));
            output.push('\t');
            output.push_str(&format_pipe_list(&record.symbols));
            output.push('\t');
            output.push_str(&record.created_at.to_string());
            output.push('\n');
        }

        output
    }
}

fn format_pipe_list(items: &[String]) -> String {
    items
        .iter()
        .map(|item| escape_pipe_item(item))
        .collect::<Vec<_>>()
        .join("|")
}

fn parse_pipe_list(input: &str) -> Result<Vec<String>, String> {
    if input.is_empty() {
        return Ok(vec![]);
    }
    // Split on unescaped '|'
    let mut items = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('\\') => current.push('\\'),
                Some('|') => current.push('|'),
                Some('t') => current.push('\t'),
                Some('n') => current.push('\n'),
                Some(other) => return Err(format!("unsupported escape: \\{other}")),
                None => return Err("trailing escape in pipe list".to_string()),
            }
        } else if ch == '|' {
            items.push(current.clone());
            current.clear();
        } else {
            current.push(ch);
        }
    }
    items.push(current);

    Ok(items)
}

fn escape_pipe_item(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
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

fn escape_json(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_packet_with_fields() {
        let record = PacketRecord::new(
            "auth",
            vec!["src/auth.rs".to_string()],
            vec!["@authrepo".to_string()],
            1_000_000,
        );
        assert_eq!(record.name, "auth");
        assert_eq!(record.files, vec!["src/auth.rs"]);
        assert_eq!(record.symbols, vec!["@authrepo"]);
        assert_eq!(record.created_at, 1_000_000);
    }

    #[test]
    fn round_trips_tsv() {
        let mut store = PacketStore::new();
        store.upsert(PacketRecord::new(
            "auth",
            vec!["src/auth.rs".to_string(), "src/token.rs".to_string()],
            vec!["@authrepo".to_string()],
            1_000_000,
        ));
        store.upsert(PacketRecord::new("empty", vec![], vec![], 2_000_000));

        let tsv = store.to_tsv();
        let restored = PacketStore::from_tsv(&tsv).expect("round-trip failed");
        assert_eq!(store, restored);
    }

    #[test]
    fn upsert_replaces_existing() {
        let mut store = PacketStore::new();
        store.upsert(PacketRecord::new(
            "auth",
            vec!["old.rs".to_string()],
            vec![],
            1,
        ));
        store.upsert(PacketRecord::new(
            "auth",
            vec!["new.rs".to_string()],
            vec![],
            2,
        ));

        assert_eq!(store.records().len(), 1);
        assert_eq!(store.get("auth").unwrap().files, vec!["new.rs"]);
    }

    #[test]
    fn delete_removes_record() {
        let mut store = PacketStore::new();
        store.upsert(PacketRecord::new("auth", vec![], vec![], 1));

        assert!(store.delete("auth"));
        assert!(store.get("auth").is_none());
        assert!(!store.delete("auth")); // already gone
    }

    #[test]
    fn compact_json_is_valid_shape() {
        let record = PacketRecord::new(
            "auth",
            vec!["src/auth.rs".to_string()],
            vec!["@repo".to_string()],
            999,
        );
        let json = record.to_compact_json();
        assert!(json.contains("\"name\":\"auth\""));
        assert!(json.contains("\"files\":[\"src/auth.rs\"]"));
        assert!(json.contains("\"symbols\":[\"@repo\"]"));
        assert!(json.contains("\"created_at\":999"));
    }
}
