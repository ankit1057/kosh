use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct FactRecord {
    pub id: String,
    pub repo: String,
    pub feature: String,
    pub fact: String,
    pub confidence: f32,
    pub source: String,
    pub symbols: Vec<String>,
    pub created_at: u64,
    pub access_count: u64,
}

#[derive(Debug, Clone, Default)]
pub struct FactStore {
    records: Vec<FactRecord>,
}

impl FactStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(path: &Path) -> io::Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let contents = fs::read_to_string(path)?;
        Self::from_tsv(&contents).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.to_tsv())
    }

    pub fn upsert(&mut self, fact: FactRecord) {
        if let Some(existing) = self.records.iter_mut().find(|r| r.id == fact.id) {
            *existing = fact;
        } else {
            self.records.push(fact);
        }
    }

    pub fn get(&self, id: &str) -> Option<&FactRecord> {
        self.records.iter().find(|r| r.id == id)
    }

    pub fn records(&self) -> &[FactRecord] {
        &self.records
    }

    pub fn delete(&mut self, id: &str) -> bool {
        let before = self.records.len();
        self.records.retain(|r| r.id != id);
        self.records.len() < before
    }

    pub fn touch(&mut self, id: &str) -> bool {
        if let Some(record) = self.records.iter_mut().find(|r| r.id == id) {
            record.access_count += 1;
            true
        } else {
            false
        }
    }

    pub fn search(&self, query: &str) -> Vec<&FactRecord> {
        let q = query.to_lowercase();
        self.records
            .iter()
            .filter(|r| {
                r.fact.to_lowercase().contains(&q)
                    || r.symbols.iter().any(|s| s.to_lowercase().contains(&q))
            })
            .collect()
    }

    pub fn by_repo(&self, repo: &str) -> Vec<&FactRecord> {
        self.records.iter().filter(|r| r.repo == repo).collect()
    }

    pub fn make_id(repo: &str, feature: &str, counter: usize) -> String {
        format!("fact:{}:{}:{:03}", repo, feature, counter)
    }

    fn from_tsv(input: &str) -> Result<Self, String> {
        let mut store = Self::new();

        for (index, line) in input.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }

            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() != 9 {
                return Err(format!(
                    "line {}: expected 9 tab-separated fields, got {}",
                    index + 1,
                    fields.len()
                ));
            }

            let id = unescape_field(fields[0])?;
            let repo = unescape_field(fields[1])?;
            let feature = unescape_field(fields[2])?;
            let fact = unescape_field(fields[3])?;
            let confidence: f32 = fields[4]
                .parse()
                .map_err(|_| format!("line {}: invalid confidence", index + 1))?;
            let source = unescape_field(fields[5])?;
            let symbols = parse_pipe_list(fields[6])?;
            let created_at: u64 = fields[7]
                .parse()
                .map_err(|_| format!("line {}: invalid created_at", index + 1))?;
            let access_count: u64 = fields[8]
                .parse()
                .map_err(|_| format!("line {}: invalid access_count", index + 1))?;

            store.upsert(FactRecord {
                id,
                repo,
                feature,
                fact,
                confidence,
                source,
                symbols,
                created_at,
                access_count,
            });
        }

        Ok(store)
    }

    fn to_tsv(&self) -> String {
        let mut output = String::new();

        for r in &self.records {
            output.push_str(&escape_field(&r.id));
            output.push('\t');
            output.push_str(&escape_field(&r.repo));
            output.push('\t');
            output.push_str(&escape_field(&r.feature));
            output.push('\t');
            output.push_str(&escape_field(&r.fact));
            output.push('\t');
            output.push_str(&r.confidence.to_string());
            output.push('\t');
            output.push_str(&escape_field(&r.source));
            output.push('\t');
            output.push_str(&format_pipe_list(&r.symbols));
            output.push('\t');
            output.push_str(&r.created_at.to_string());
            output.push('\t');
            output.push_str(&r.access_count.to_string());
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_fact(repo: &str, feature: &str, counter: usize, fact_text: &str) -> FactRecord {
        FactRecord {
            id: FactStore::make_id(repo, feature, counter),
            repo: repo.to_string(),
            feature: feature.to_string(),
            fact: fact_text.to_string(),
            confidence: 0.9,
            source: "manual".to_string(),
            symbols: vec!["@auth".to_string(), "src/auth.rs".to_string()],
            created_at: 1_000_000,
            access_count: 0,
        }
    }

    #[test]
    fn id_format_is_correct() {
        let id = FactStore::make_id("kosh", "auth", 1);
        assert_eq!(id, "fact:kosh:auth:001");

        let id2 = FactStore::make_id("myrepo", "payments", 42);
        assert_eq!(id2, "fact:myrepo:payments:042");
    }

    #[test]
    fn tsv_round_trip_preserves_all_fields() {
        let mut store = FactStore::new();
        let fact = FactRecord {
            id: "fact:kosh:auth:001".to_string(),
            repo: "kosh".to_string(),
            feature: "auth".to_string(),
            fact: "Auth uses JWT tokens\twith tabs\nand newlines".to_string(),
            confidence: 0.85,
            source: "PR #41".to_string(),
            symbols: vec!["@auth".to_string(), "src/auth.rs".to_string(), "pipe|sym".to_string()],
            created_at: 1_700_000_000,
            access_count: 5,
        };
        store.upsert(fact.clone());

        let tsv = store.to_tsv();
        let restored = FactStore::from_tsv(&tsv).expect("round-trip failed");

        let r = restored.get("fact:kosh:auth:001").unwrap();
        assert_eq!(r.id, fact.id);
        assert_eq!(r.repo, fact.repo);
        assert_eq!(r.feature, fact.feature);
        assert_eq!(r.fact, fact.fact);
        assert!((r.confidence - fact.confidence).abs() < 1e-6);
        assert_eq!(r.source, fact.source);
        assert_eq!(r.symbols, fact.symbols);
        assert_eq!(r.created_at, fact.created_at);
        assert_eq!(r.access_count, fact.access_count);
    }

    #[test]
    fn upsert_overwrites_existing_record() {
        let mut store = FactStore::new();
        store.upsert(make_fact("kosh", "auth", 1, "original fact"));
        store.upsert(FactRecord {
            id: "fact:kosh:auth:001".to_string(),
            repo: "kosh".to_string(),
            feature: "auth".to_string(),
            fact: "updated fact".to_string(),
            confidence: 0.5,
            source: "review".to_string(),
            symbols: vec![],
            created_at: 2_000_000,
            access_count: 0,
        });

        assert_eq!(store.records().len(), 1);
        assert_eq!(store.get("fact:kosh:auth:001").unwrap().fact, "updated fact");
    }

    #[test]
    fn delete_returns_true_on_hit_false_on_miss() {
        let mut store = FactStore::new();
        store.upsert(make_fact("kosh", "auth", 1, "some fact"));

        assert!(store.delete("fact:kosh:auth:001"));
        assert!(store.get("fact:kosh:auth:001").is_none());
        assert!(!store.delete("fact:kosh:auth:001"));
    }

    #[test]
    fn search_finds_by_fact_text_case_insensitive() {
        let mut store = FactStore::new();
        store.upsert(make_fact("kosh", "auth", 1, "Auth uses JWT tokens"));
        store.upsert(make_fact("kosh", "db", 2, "Database uses PostgreSQL"));

        let results = store.search("jwt");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "fact:kosh:auth:001");

        let results2 = store.search("JWT");
        assert_eq!(results2.len(), 1);

        let empty = store.search("redis");
        assert!(empty.is_empty());
    }

    #[test]
    fn search_finds_by_symbol_substring() {
        let mut store = FactStore::new();
        let mut fact = make_fact("kosh", "auth", 1, "some fact");
        fact.symbols = vec!["@AuthService".to_string(), "src/auth/mod.rs".to_string()];
        store.upsert(fact);

        let results = store.search("authservice");
        assert_eq!(results.len(), 1);

        let results2 = store.search("mod.rs");
        assert_eq!(results2.len(), 1);
    }

    #[test]
    fn touch_increments_access_count() {
        let mut store = FactStore::new();
        store.upsert(make_fact("kosh", "auth", 1, "some fact"));

        assert_eq!(store.get("fact:kosh:auth:001").unwrap().access_count, 0);
        assert!(store.touch("fact:kosh:auth:001"));
        assert_eq!(store.get("fact:kosh:auth:001").unwrap().access_count, 1);
        store.touch("fact:kosh:auth:001");
        assert_eq!(store.get("fact:kosh:auth:001").unwrap().access_count, 2);

        assert!(!store.touch("fact:nonexistent:x:000"));
    }

    #[test]
    fn by_repo_filters_correctly() {
        let mut store = FactStore::new();
        store.upsert(make_fact("kosh", "auth", 1, "kosh auth fact"));
        store.upsert(make_fact("kosh", "db", 2, "kosh db fact"));
        store.upsert(make_fact("other-repo", "feature", 1, "other fact"));

        let kosh_facts = store.by_repo("kosh");
        assert_eq!(kosh_facts.len(), 2);

        let other_facts = store.by_repo("other-repo");
        assert_eq!(other_facts.len(), 1);
        assert_eq!(other_facts[0].fact, "other fact");

        let empty = store.by_repo("nonexistent");
        assert!(empty.is_empty());
    }

    #[test]
    fn load_returns_empty_for_missing_file() {
        let path = PathBuf::from("/tmp/nonexistent_fact_store_test.tsv");
        let store = FactStore::load(&path).expect("should succeed for missing file");
        assert!(store.records().is_empty());
    }
}
