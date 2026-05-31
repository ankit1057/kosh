pub mod lease;
pub use lease::*;

use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextFingerprint {
    pub repo: String,
    pub feature: String,
    pub hash: String,
}

impl ContextFingerprint {
    pub fn new(
        repo: impl Into<String>,
        feature: impl Into<String>,
        hash: impl Into<String>,
    ) -> Self {
        Self {
            repo: repo.into(),
            feature: feature.into(),
            hash: hash.into(),
        }
    }

    pub fn key(&self) -> String {
        format!(
            "{}:{}:{}",
            normalize(&self.repo),
            normalize(&self.feature),
            self.hash
        )
    }

    pub fn to_compact_json(&self) -> String {
        format!(
            "{{\"repo\":\"{}\",\"feature\":\"{}\",\"hash\":\"{}\"}}",
            escape_json(&self.repo),
            escape_json(&self.feature),
            escape_json(&self.hash)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheRecord {
    pub fingerprint: ContextFingerprint,
    pub summary: String,
}

impl CacheRecord {
    pub fn new(fingerprint: ContextFingerprint, summary: impl Into<String>) -> Self {
        Self {
            fingerprint,
            summary: summary.into(),
        }
    }

    pub fn key(&self) -> String {
        self.fingerprint.key()
    }

    pub fn to_compact_json(&self) -> String {
        format!(
            "{{\"key\":\"{}\",\"fingerprint\":{},\"summary\":\"{}\"}}",
            escape_json(&self.key()),
            self.fingerprint.to_compact_json(),
            escape_json(&self.summary)
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextCache {
    records: Vec<CacheRecord>,
}

impl ContextCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::new());
        }

        let contents = fs::read_to_string(path)?;
        Self::from_tsv(&contents).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    pub fn save(&self, path: impl AsRef<Path>) -> io::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(path, self.to_tsv())
    }

    pub fn upsert(&mut self, record: CacheRecord) {
        let key = record.key();
        if let Some(existing) = self
            .records
            .iter_mut()
            .find(|candidate| candidate.key() == key)
        {
            *existing = record;
        } else {
            self.records.push(record);
        }
    }

    pub fn get(&self, key: &str) -> Option<&CacheRecord> {
        self.records.iter().find(|candidate| candidate.key() == key)
    }

    pub fn records(&self) -> &[CacheRecord] {
        &self.records
    }

    pub fn from_tsv(input: &str) -> Result<Self, String> {
        let mut cache = Self::new();

        for (index, line) in input.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }

            let fields = line.split('\t').collect::<Vec<_>>();
            if fields.len() != 5 {
                return Err(format!(
                    "line {}: expected 5 tab-separated fields",
                    index + 1
                ));
            }

            let fingerprint = ContextFingerprint::new(
                unescape_field(fields[1])?,
                unescape_field(fields[2])?,
                unescape_field(fields[3])?,
            );
            let summary = unescape_field(fields[4])?;
            let record = CacheRecord::new(fingerprint, summary);

            if record.key() != fields[0] {
                return Err(format!(
                    "line {}: cache key does not match fingerprint",
                    index + 1
                ));
            }

            cache.upsert(record);
        }

        Ok(cache)
    }

    pub fn to_tsv(&self) -> String {
        let mut output = String::new();

        for record in &self.records {
            output.push_str(&record.key());
            output.push('\t');
            output.push_str(&escape_field(&record.fingerprint.repo));
            output.push('\t');
            output.push_str(&escape_field(&record.fingerprint.feature));
            output.push('\t');
            output.push_str(&escape_field(&record.fingerprint.hash));
            output.push('\t');
            output.push_str(&escape_field(&record.summary));
            output.push('\n');
        }

        output
    }
}

fn normalize(input: &str) -> String {
    input.trim().to_ascii_lowercase().replace(' ', "-")
}

fn escape_json(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
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

    #[test]
    fn builds_stable_key() {
        let fingerprint = ContextFingerprint::new("Veil", "Auth Flow", "xyz");
        assert_eq!(fingerprint.key(), "veil:auth-flow:xyz");
    }

    #[test]
    fn emits_compact_json() {
        let fingerprint = ContextFingerprint::new("veil", "auth", "xyz");
        assert_eq!(
            fingerprint.to_compact_json(),
            "{\"repo\":\"veil\",\"feature\":\"auth\",\"hash\":\"xyz\"}"
        );
    }

    #[test]
    fn upserts_and_reads_cache_records() {
        let mut cache = ContextCache::new();
        let fingerprint = ContextFingerprint::new("veil", "auth", "xyz");
        cache.upsert(CacheRecord::new(fingerprint, "Auth repository summary"));

        let record = cache.get("veil:auth:xyz").unwrap();
        assert_eq!(record.summary, "Auth repository summary");
    }

    #[test]
    fn round_trips_tsv_cache() {
        let mut cache = ContextCache::new();
        cache.upsert(CacheRecord::new(
            ContextFingerprint::new("Veil", "Auth Flow", "xyz"),
            "line one\nline two",
        ));

        let tsv = cache.to_tsv();
        let parsed = ContextCache::from_tsv(&tsv).unwrap();

        assert_eq!(parsed.records(), cache.records());
    }
}
