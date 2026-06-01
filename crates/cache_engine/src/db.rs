use rusqlite::{params, Connection};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextFingerprintV2 {
    pub repo: String,
    pub branch: String,
    pub commit: String,
    pub symbols: Vec<String>,
    pub files: Vec<String>,
}

impl ContextFingerprintV2 {
    pub fn new(repo: &str, branch: &str, commit: &str) -> Self {
        Self {
            repo: repo.to_string(),
            branch: branch.to_string(),
            commit: commit.to_string(),
            symbols: Vec::new(),
            files: Vec::new(),
        }
    }

    pub fn add_symbol(&mut self, symbol: &str) {
        self.symbols.push(symbol.to_string());
        self.symbols.sort();
    }

    pub fn add_file(&mut self, file: &str) {
        self.files.push(file.to_string());
        self.files.sort();
    }

    pub fn deterministic_hash(&self) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        self.repo.hash(&mut hasher);
        self.branch.hash(&mut hasher);
        self.commit.hash(&mut hasher);
        self.symbols.hash(&mut hasher);
        self.files.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
}

#[derive(Debug, Clone)]
pub struct DbLeaseRecord {
    pub id: String,
    pub repo: String,
    pub feature: String,
    pub fingerprint_hash: String,
    pub summary: String,
    pub byte_size: u64,
    pub created_at: u64,
    pub access_count: u64,
    pub last_used: Option<u64>,
    pub tokens_saved: u64,
    pub file_list: Vec<String>,
}

pub struct DbLeaseStore {
    pub conn: Connection,
}

impl DbLeaseStore {
    pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;

        // Initialize schema
        conn.execute(
            "CREATE TABLE IF NOT EXISTS leases (
                id TEXT PRIMARY KEY,
                repo TEXT NOT NULL,
                feature TEXT NOT NULL,
                fingerprint_hash TEXT NOT NULL,
                summary TEXT,
                byte_size INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                access_count INTEGER DEFAULT 0,
                last_used INTEGER,
                tokens_saved INTEGER DEFAULT 0,
                file_list TEXT
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_leases_fingerprint ON leases (fingerprint_hash)",
            [],
        )?;

        Ok(Self { conn })
    }

    pub fn find_by_fingerprint(&self, hash: &str) -> rusqlite::Result<Option<DbLeaseRecord>> {
        let mut stmt = self.conn.prepare("SELECT id, repo, feature, fingerprint_hash, summary, byte_size, created_at, access_count, last_used, tokens_saved, file_list FROM leases WHERE fingerprint_hash = ? LIMIT 1")?;
        let mut rows = stmt.query(params![hash])?;
        if let Some(row) = rows.next()? {
            let file_list_raw: String = row.get(10).unwrap_or_default();
            Ok(Some(DbLeaseRecord {
                id: row.get(0)?,
                repo: row.get(1)?,
                feature: row.get(2)?,
                fingerprint_hash: row.get(3)?,
                summary: row.get(4).unwrap_or_default(),
                byte_size: row.get(5)?,
                created_at: row.get(6)?,
                access_count: row.get(7)?,
                last_used: row.get(8)?,
                tokens_saved: row.get(9)?,
                file_list: file_list_raw.split('|').map(|s| s.to_string()).filter(|s| !s.is_empty()).collect(),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_all(&self) -> rusqlite::Result<Vec<DbLeaseRecord>> {
        let mut stmt = self.conn.prepare("SELECT id, repo, feature, fingerprint_hash, summary, byte_size, created_at, access_count, last_used, tokens_saved, file_list FROM leases")?;
        let rows = stmt.query_map([], |row| {
            let file_list_raw: String = row.get(10).unwrap_or_default();
            Ok(DbLeaseRecord {
                id: row.get(0)?,
                repo: row.get(1)?,
                feature: row.get(2)?,
                fingerprint_hash: row.get(3)?,
                summary: row.get(4).unwrap_or_default(),
                byte_size: row.get(5)?,
                created_at: row.get(6)?,
                access_count: row.get(7)?,
                last_used: row.get(8)?,
                tokens_saved: row.get(9)?,
                file_list: file_list_raw.split('|').map(|s| s.to_string()).filter(|s| !s.is_empty()).collect(),
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn insert_lease(&self, record: &DbLeaseRecord) -> rusqlite::Result<()> {
        let file_list_raw = record.file_list.join("|");
        self.conn.execute(
            "INSERT INTO leases (id, repo, feature, fingerprint_hash, summary, byte_size, created_at, file_list) 
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![record.id, record.repo, record.feature, record.fingerprint_hash, record.summary, record.byte_size, record.created_at, file_list_raw],
        )?;
        Ok(())
    }

    pub fn record_hit(&self, id: &str, tokens: u64) -> rusqlite::Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.conn.execute(
            "UPDATE leases SET 
                access_count = access_count + 1,
                last_used = ?,
                tokens_saved = tokens_saved + ?
             WHERE id = ?",
            params![now, tokens, id],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_v2_is_deterministic() {
        let mut f1 = ContextFingerprintV2::new("kosh", "main", "abc");
        f1.add_symbol("LeaseRecord");
        f1.add_file("lease.rs");

        let mut f2 = ContextFingerprintV2::new("kosh", "main", "abc");
        f2.add_file("lease.rs");
        f2.add_symbol("LeaseRecord");

        assert_eq!(f1.deterministic_hash(), f2.deterministic_hash());
    }

    #[test]
    fn db_store_records_hits() -> rusqlite::Result<()> {
        let store = DbLeaseStore::open(":memory:")?;
        store.conn.execute(
            "INSERT INTO leases (id, repo, feature, fingerprint_hash, byte_size, created_at, file_list) 
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            params!["lease:auth:001", "kosh", "auth", "hash123", 1000, 0, "src/auth.rs"],
        )?;

        store.record_hit("lease:auth:001", 250)?;

        let saved: u64 = store.conn.query_row(
            "SELECT tokens_saved FROM leases WHERE id = ?",
            params!["lease:auth:001"],
            |r| r.get(0)
        )?;

        assert_eq!(saved, 250);
        Ok(())
    }
}
