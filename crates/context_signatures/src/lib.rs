/// Context Signatures — deterministic fingerprints for context composition.
///
/// A ContextSignature captures the minimal identity of a context bundle:
/// which files, which symbols, which repo+feature. Two signatures can be
/// overlap-scored to find reusable context without reading any files.
///
/// Storage: TSV, one record per line, tab-separated fields.
/// No LLMs, no embeddings, no external dependencies.
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;

// ── Record ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextSignature {
    /// Stable identifier: `sig:<repo>:<feature>:<short_hash>`
    pub id: String,
    pub repo: String,
    pub feature: String,
    /// Sorted, deduplicated file paths included in this context bundle.
    pub files: Vec<String>,
    /// Sorted, deduplicated symbol handles (e.g. `@authrepo`, `AuthRepository`).
    pub symbols: Vec<String>,
    /// Unix timestamp (seconds) when this signature was created.
    pub created_at: u64,
    /// How many times this signature has been matched against a query.
    pub hit_count: u64,
}

impl ContextSignature {
    pub fn new(
        repo: impl Into<String>,
        feature: impl Into<String>,
        mut files: Vec<String>,
        mut symbols: Vec<String>,
        created_at: u64,
    ) -> Self {
        files.sort();
        files.dedup();
        symbols.sort();
        symbols.dedup();

        let repo = repo.into();
        let feature = feature.into();
        let hash = short_hash(&repo, &feature, &files, &symbols);
        let id = format!("sig:{}:{}:{}", repo, feature, hash);

        Self { id, repo, feature, files, symbols, created_at, hit_count: 0 }
    }

    /// Jaccard overlap over the union of files and symbols.
    /// Returns 0.0 (no overlap) to 1.0 (identical).
    pub fn overlap(&self, other: &ContextSignature) -> f32 {
        let a_files: HashSet<&str> = self.files.iter().map(String::as_str).collect();
        let b_files: HashSet<&str> = other.files.iter().map(String::as_str).collect();
        let a_syms: HashSet<&str> = self.symbols.iter().map(String::as_str).collect();
        let b_syms: HashSet<&str> = other.symbols.iter().map(String::as_str).collect();

        let intersection = a_files.intersection(&b_files).count()
            + a_syms.intersection(&b_syms).count();
        let union = a_files.union(&b_files).count() + a_syms.union(&b_syms).count();

        if union == 0 { 1.0 } else { intersection as f32 / union as f32 }
    }

    /// True if this signature subsumes `other` (every file+symbol in other is in self).
    pub fn subsumes(&self, other: &ContextSignature) -> bool {
        let self_files: HashSet<&str> = self.files.iter().map(String::as_str).collect();
        let self_syms: HashSet<&str> = self.symbols.iter().map(String::as_str).collect();
        other.files.iter().all(|f| self_files.contains(f.as_str()))
            && other.symbols.iter().all(|s| self_syms.contains(s.as_str()))
    }

    /// Merge two signatures into a union signature (for context composition).
    pub fn compose(a: &ContextSignature, b: &ContextSignature, created_at: u64) -> Self {
        let mut files = a.files.clone();
        files.extend(b.files.iter().cloned());
        let mut symbols = a.symbols.clone();
        symbols.extend(b.symbols.iter().cloned());
        let feature = if a.feature == b.feature {
            a.feature.clone()
        } else {
            format!("{}+{}", a.feature, b.feature)
        };
        ContextSignature::new(a.repo.clone(), feature, files, symbols, created_at)
    }

    fn to_tsv_line(&self) -> String {
        let files = self.files.join("|");
        let symbols = self.symbols.join("|");
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            tsv_escape(&self.id),
            tsv_escape(&self.repo),
            tsv_escape(&self.feature),
            tsv_escape(&files),
            tsv_escape(&symbols),
            self.created_at,
            self.hit_count,
        )
    }

    fn from_tsv_line(line: &str) -> Option<Self> {
        let cols: Vec<&str> = line.splitn(7, '\t').collect();
        if cols.len() < 7 { return None; }
        let files = if cols[3].is_empty() {
            vec![]
        } else {
            cols[3].split('|').map(|s| tsv_unescape(s)).collect()
        };
        let symbols = if cols[4].is_empty() {
            vec![]
        } else {
            cols[4].split('|').map(|s| tsv_unescape(s)).collect()
        };
        Some(Self {
            id:         tsv_unescape(cols[0]),
            repo:       tsv_unescape(cols[1]),
            feature:    tsv_unescape(cols[2]),
            files,
            symbols,
            created_at: cols[5].parse().unwrap_or(0),
            hit_count:  cols[6].parse().unwrap_or(0),
        })
    }
}

// ── Store ─────────────────────────────────────────────────────────────────────

pub struct SignatureStore {
    records: Vec<ContextSignature>,
}

impl SignatureStore {
    pub fn new() -> Self {
        Self { records: Vec::new() }
    }

    pub fn load(path: &Path) -> io::Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let content = fs::read_to_string(path)?;
        let records = content
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .filter_map(ContextSignature::from_tsv_line)
            .collect();
        Ok(Self { records })
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = self.records.iter().map(|r| r.to_tsv_line()).collect::<Vec<_>>().join("\n");
        fs::write(path, if content.is_empty() { content } else { content + "\n" })
    }

    pub fn upsert(&mut self, sig: ContextSignature) {
        if let Some(existing) = self.records.iter_mut().find(|r| r.id == sig.id) {
            *existing = sig;
        } else {
            self.records.push(sig);
        }
    }

    pub fn get(&self, id: &str) -> Option<&ContextSignature> {
        self.records.iter().find(|r| r.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut ContextSignature> {
        self.records.iter_mut().find(|r| r.id == id)
    }

    pub fn records(&self) -> &[ContextSignature] {
        &self.records
    }

    /// Find signatures whose Jaccard overlap with `query` exceeds `threshold`.
    /// Returns matches sorted by descending overlap score.
    pub fn find_overlapping(
        &self,
        query: &ContextSignature,
        threshold: f32,
    ) -> Vec<(&ContextSignature, f32)> {
        let mut matches: Vec<(&ContextSignature, f32)> = self
            .records
            .iter()
            .map(|r| (r, r.overlap(query)))
            .filter(|(_, score)| *score >= threshold)
            .collect();
        matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        matches
    }

    /// Find the single best-matching signature for `query` (highest overlap).
    pub fn best_match(
        &self,
        query: &ContextSignature,
    ) -> Option<(&ContextSignature, f32)> {
        self.records
            .iter()
            .map(|r| (r, r.overlap(query)))
            .filter(|(_, s)| *s > 0.0)
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    }

    pub fn delete(&mut self, id: &str) -> bool {
        let before = self.records.len();
        self.records.retain(|r| r.id != id);
        self.records.len() < before
    }

    pub fn touch(&mut self, id: &str) -> bool {
        if let Some(r) = self.get_mut(id) {
            r.hit_count += 1;
            true
        } else {
            false
        }
    }
}

impl Default for SignatureStore {
    fn default() -> Self { Self::new() }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Cheap non-cryptographic hash for stable signature IDs.
/// Uses FNV-1a over the sorted, pipe-joined inputs.
fn short_hash(repo: &str, feature: &str, files: &[String], symbols: &[String]) -> String {
    let input = format!(
        "{}|{}|{}|{}",
        repo,
        feature,
        files.join("|"),
        symbols.join("|")
    );
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325; // FNV offset basis
    for byte in input.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3); // FNV prime
    }
    format!("{:016x}", hash)[..8].to_string()
}

fn tsv_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\t', "\\t").replace('\n', "\\n")
}

fn tsv_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('t')  => out.push('\t'),
                Some('n')  => out.push('\n'),
                Some('\\') => out.push('\\'),
                Some(c)    => { out.push('\\'); out.push(c); }
                None       => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sig(feature: &str, files: &[&str], syms: &[&str]) -> ContextSignature {
        ContextSignature::new(
            "myapp",
            feature,
            files.iter().map(|s| s.to_string()).collect(),
            syms.iter().map(|s| s.to_string()).collect(),
            1_000_000,
        )
    }

    #[test]
    fn test_identical_signatures_overlap_1() {
        let a = make_sig("auth", &["auth.rs", "models.rs"], &["@authrepo"]);
        let b = make_sig("auth", &["auth.rs", "models.rs"], &["@authrepo"]);
        assert_eq!(a.id, b.id, "same inputs → same id");
        assert!((a.overlap(&b) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_disjoint_signatures_overlap_0() {
        let a = make_sig("auth", &["auth.rs"], &[]);
        let b = make_sig("payment", &["payment.rs"], &[]);
        assert!((a.overlap(&b)).abs() < 1e-6);
    }

    #[test]
    fn test_partial_overlap() {
        let a = make_sig("auth", &["a.rs", "b.rs"], &[]);
        let b = make_sig("auth", &["b.rs", "c.rs"], &[]);
        // intersection = {b.rs}, union = {a.rs, b.rs, c.rs}  → 1/3
        let score = a.overlap(&b);
        assert!((score - 1.0 / 3.0).abs() < 1e-5, "got {score}");
    }

    #[test]
    fn test_subsumes() {
        let big = make_sig("auth", &["a.rs", "b.rs", "c.rs"], &["@x"]);
        let small = make_sig("auth", &["a.rs", "b.rs"], &["@x"]);
        assert!(big.subsumes(&small));
        assert!(!small.subsumes(&big));
    }

    #[test]
    fn test_compose() {
        let a = make_sig("auth", &["auth.rs"], &["@authrepo"]);
        let b = make_sig("payment", &["pay.rs"], &["@payrepo"]);
        let composed = ContextSignature::compose(&a, &b, 1_000_001);
        assert!(composed.files.contains(&"auth.rs".to_string()));
        assert!(composed.files.contains(&"pay.rs".to_string()));
        assert_eq!(composed.repo, "myapp");
    }

    #[test]
    fn test_tsv_round_trip() {
        let sig = make_sig("auth", &["lib/auth.rs", "lib/mod\tels.rs"], &["@authrepo"]);
        let line = sig.to_tsv_line();
        let recovered = ContextSignature::from_tsv_line(&line).expect("parse");
        assert_eq!(sig.id, recovered.id);
        assert_eq!(sig.files, recovered.files);
        assert_eq!(sig.symbols, recovered.symbols);
    }

    #[test]
    fn test_store_find_overlapping() {
        let mut store = SignatureStore::new();
        store.upsert(make_sig("auth", &["a.rs", "b.rs"], &[]));
        store.upsert(make_sig("payment", &["c.rs", "d.rs"], &[]));
        store.upsert(make_sig("shared", &["a.rs", "c.rs"], &[]));

        let query = make_sig("query", &["a.rs"], &[]);
        let matches = store.find_overlapping(&query, 0.1);
        // "auth" and "shared" share a.rs; "payment" does not
        assert!(matches.len() >= 2);
        assert!(matches.iter().any(|(s, _)| s.feature == "auth"));
        assert!(matches.iter().any(|(s, _)| s.feature == "shared"));
    }

    #[test]
    fn test_store_touch_increments_hit_count() {
        let mut store = SignatureStore::new();
        let sig = make_sig("auth", &["a.rs"], &[]);
        let id = sig.id.clone();
        store.upsert(sig);
        store.touch(&id);
        store.touch(&id);
        assert_eq!(store.get(&id).unwrap().hit_count, 2);
    }
}
