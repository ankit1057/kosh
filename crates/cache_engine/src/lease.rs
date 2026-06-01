use std::fs;
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseRecord {
    pub id: String,
    pub repo: String,
    pub feature: String,
    pub fingerprint: String,
    pub summary: String,
    pub byte_size: u64,
    pub created_at: u64,
    pub access_count: u64,
}

impl LeaseRecord {
    pub fn new(
        id: impl Into<String>,
        repo: impl Into<String>,
        feature: impl Into<String>,
        fingerprint: impl Into<String>,
        summary: impl Into<String>,
        byte_size: u64,
        created_at: u64,
    ) -> Self {
        Self {
            id: id.into(),
            repo: repo.into(),
            feature: feature.into(),
            fingerprint: fingerprint.into(),
            summary: summary.into(),
            byte_size,
            created_at,
            access_count: 0,
        }
    }

    pub fn to_compact_json(&self) -> String {
        format!(
            "{{\"id\":\"{}\",\"repo\":\"{}\",\"feature\":\"{}\",\"fingerprint\":\"{}\",\"summary\":\"{}\",\"byte_size\":{},\"created_at\":{},\"access_count\":{}}}",
            escape_json(&self.id),
            escape_json(&self.repo),
            escape_json(&self.feature),
            escape_json(&self.fingerprint),
            escape_json(&self.summary),
            self.byte_size,
            self.created_at,
            self.access_count
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextLeaseManager {
    records: Vec<LeaseRecord>,
    next_sequence: std::collections::HashMap<String, u64>,
}

impl ContextLeaseManager {
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

    pub fn create_lease(
        &mut self,
        repo: &str,
        feature: &str,
        fingerprint: &str,
        summary: &str,
        byte_size: u64,
    ) -> LeaseRecord {
        let seq_key = format!("{}:{}", repo, feature);
        let seq = self.next_sequence.entry(seq_key).or_insert(1);
        let id = format!("lease:{}:{:03}", feature, seq);
        *seq += 1;

        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let record = LeaseRecord::new(
            id,
            repo,
            feature,
            fingerprint,
            summary,
            byte_size,
            created_at,
        );
        self.records.push(record.clone());
        record
    }

    pub fn get(&self, id: &str) -> Option<&LeaseRecord> {
        self.records.iter().find(|candidate| candidate.id == id)
    }

    pub fn touch(&mut self, id: &str) -> Option<&LeaseRecord> {
        if let Some(record) = self.records.iter_mut().find(|candidate| candidate.id == id) {
            record.access_count += 1;
            Some(record)
        } else {
            None
        }
    }

    pub fn records(&self) -> &[LeaseRecord] {
        &self.records
    }

    pub fn from_tsv(input: &str) -> Result<Self, String> {
        let mut manager = Self::new();

        for (index, line) in input.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }

            let fields = line.split('\t').collect::<Vec<_>>();
            if fields.len() != 8 {
                return Err(format!(
                    "line {}: expected 8 tab-separated fields",
                    index + 1
                ));
            }

            let id = unescape_field(fields[0])?;
            let repo = unescape_field(fields[1])?;
            let feature = unescape_field(fields[2])?;
            let fingerprint = unescape_field(fields[3])?;
            let summary = unescape_field(fields[4])?;
            let byte_size = fields[5]
                .parse::<u64>()
                .map_err(|error| format!("line {}: invalid byte_size: {error}", index + 1))?;
            let created_at = fields[6]
                .parse::<u64>()
                .map_err(|error| format!("line {}: invalid created_at: {error}", index + 1))?;
            let access_count = fields[7]
                .parse::<u64>()
                .map_err(|error| format!("line {}: invalid access_count: {error}", index + 1))?;

            let mut record = LeaseRecord::new(
                id.clone(),
                repo.clone(),
                feature.clone(),
                fingerprint,
                summary,
                byte_size,
                created_at,
            );
            record.access_count = access_count;
            manager.records.push(record);

            // Update sequence logic
            if id.starts_with("lease:") {
                let parts: Vec<&str> = id.split(':').collect();
                if parts.len() == 3 {
                    if let Ok(seq_num) = parts[2].parse::<u64>() {
                        let seq_key = format!("{}:{}", repo, feature);
                        let current_seq = manager.next_sequence.entry(seq_key).or_insert(1);
                        if seq_num >= *current_seq {
                            *current_seq = seq_num + 1;
                        }
                    }
                }
            }
        }

        Ok(manager)
    }

    pub fn to_tsv(&self) -> String {
        let mut output = String::new();

        for record in &self.records {
            output.push_str(&escape_field(&record.id));
            output.push('\t');
            output.push_str(&escape_field(&record.repo));
            output.push('\t');
            output.push_str(&escape_field(&record.feature));
            output.push('\t');
            output.push_str(&escape_field(&record.fingerprint));
            output.push('\t');
            output.push_str(&escape_field(&record.summary));
            output.push('\t');
            output.push_str(&record.byte_size.to_string());
            output.push('\t');
            output.push_str(&record.created_at.to_string());
            output.push('\t');
            output.push_str(&record.access_count.to_string());
            output.push('\n');
        }

        output
    }
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
    fn creates_sequential_leases() {
        let mut manager = ContextLeaseManager::new();
        let l1 = manager.create_lease("repo", "auth", "hash1", "summary1", 100);
        assert_eq!(l1.id, "lease:auth:001");
        assert_eq!(l1.byte_size, 100);
        let l2 = manager.create_lease("repo", "auth", "hash2", "summary2", 200);
        assert_eq!(l2.id, "lease:auth:002");
        assert_eq!(l2.byte_size, 200);
    }

    #[test]
    fn tracks_access_count() {
        let mut manager = ContextLeaseManager::new();
        manager.create_lease("repo", "auth", "hash1", "summary1", 100);
        manager.touch("lease:auth:001");
        manager.touch("lease:auth:001");

        let l1 = manager.get("lease:auth:001").unwrap();
        assert_eq!(l1.access_count, 2);
    }

    #[test]
    fn round_trips_tsv_leases() {
        let mut manager = ContextLeaseManager::new();
        manager.create_lease("repo", "auth", "hash1", "sum\tmary\n1", 100);
        manager.touch("lease:auth:001");

        let tsv = manager.to_tsv();
        let parsed = ContextLeaseManager::from_tsv(&tsv).unwrap();

        assert_eq!(parsed.records(), manager.records());
        let l2 = parsed
            .clone()
            .create_lease("repo", "auth", "hash2", "summary2", 200);
        assert_eq!(l2.id, "lease:auth:002");
    }
}
