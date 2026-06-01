use cache_engine::db::{ContextFingerprintV2, DbLeaseStore, DbLeaseRecord};
use packet_engine::PacketStore;
use symbol_extractor::SymbolTable;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, serde::Serialize)]
pub struct ContextRecommendation {
    pub lease_id: Option<String>,
    pub packet_name: Option<String>,
    pub confidence: f64,
    pub estimated_tokens_saved: u64,
    pub reason: String,
    pub score: f64,
}

pub struct ContextResolver {
    lease_store: DbLeaseStore,
    packet_store: PacketStore,
    symbol_table: SymbolTable,
}

impl ContextResolver {
    pub fn open(db_path: impl AsRef<Path>, packet_tsv_path: impl AsRef<Path>) -> Result<Self, String> {
        let lease_store = DbLeaseStore::open(&db_path).map_err(|e| e.to_string())?;
        let packet_store = PacketStore::load(packet_tsv_path).map_err(|e| e.to_string())?;
        let symbol_table = SymbolTable::open(db_path).map_err(|e| e.to_string())?;
        Ok(Self {
            lease_store,
            packet_store,
            symbol_table,
        })
    }

    pub fn resolve_from_fingerprint(&self, fingerprint: &ContextFingerprintV2) -> Option<ContextRecommendation> {
        let hash = fingerprint.deterministic_hash();
        match self.lease_store.find_by_fingerprint(&hash) {
            Ok(Some(record)) => {
                let score = self.calculate_lease_score(&record);
                Some(ContextRecommendation {
                    lease_id: Some(record.id),
                    packet_name: None,
                    confidence: 1.0,
                    estimated_tokens_saved: record.tokens_saved / (record.access_count.max(1)),
                    reason: "Exact fingerprint match found in lease store.".to_string(),
                    score,
                })
            }
            _ => None,
        }
    }

    pub fn resolve_query(&self, query: &str) -> Vec<ContextRecommendation> {
        let mut results = Vec::new();

        // 1. Check for packet name matches
        if let Some(packet) = self.packet_store.get(query) {
            results.push(ContextRecommendation {
                lease_id: None,
                packet_name: Some(packet.name.clone()),
                confidence: 0.95,
                estimated_tokens_saved: 2000, 
                reason: format!("Direct match for packet: {}", packet.name),
                score: 0.95,
            });
        }

        // 2. Scan leases and score them
        if let Ok(leases) = self.lease_store.list_all() {
            for lease in leases {
                if lease.id.contains(query) || lease.feature.contains(query) || lease.summary.contains(query) {
                    let score = self.calculate_lease_score(&lease);
                    results.push(ContextRecommendation {
                        lease_id: Some(lease.id.clone()),
                        packet_name: None,
                        confidence: 0.8, // Basic keyword match confidence
                        estimated_tokens_saved: lease.tokens_saved / (lease.access_count.max(1)),
                        reason: format!("Keyword match in lease: {}", lease.id),
                        score,
                    });
                }
            }
        }

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        results
    }

    pub fn resolve_from_signature(&self, symbols: &[String]) -> Vec<ContextRecommendation> {
        let mut results = Vec::new();
        if let Ok(leases) = self.lease_store.list_all() {
            for lease in leases {
                if let Ok(lease_symbols) = self.symbol_table.get_lease_signature(&lease.id) {
                    let overlap = calculate_overlap(symbols, &lease_symbols);
                    if overlap > 0.0 {
                        let base_score = self.calculate_lease_score(&lease);
                        let final_score = (0.75 * base_score) + (0.25 * overlap);
                        results.push(ContextRecommendation {
                            lease_id: Some(lease.id.clone()),
                            packet_name: None,
                            confidence: overlap,
                            estimated_tokens_saved: lease.tokens_saved / (lease.access_count.max(1)),
                            reason: format!("{:.0}% symbol overlap with lease: {}", overlap * 100.0, lease.id),
                            score: final_score,
                        });
                    }
                }
            }
        }
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        results
    }

    fn calculate_lease_score(&self, record: &DbLeaseRecord) -> f64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // 1. Recency Score (0.40)
        let last_used = record.last_used.unwrap_or(record.created_at);
        let seconds_since_use = now.saturating_sub(last_used);
        let recency_score = if seconds_since_use < 3600 { 1.0 } 
                           else if seconds_since_use < 86400 { 0.5 }
                           else { 0.1 };

        // 2. Historical Savings Score (0.30)
        // Normalize against a "high" savings bar of 1M tokens
        let savings_score = (record.tokens_saved as f64 / 1_000_000.0).min(1.0);

        // 3. Frequency Score (0.30)
        // Normalize against a "high" frequency of 100 hits
        let frequency_score = (record.access_count as f64 / 100.0).min(1.0);

        (0.40 * recency_score) + (0.30 * savings_score) + (0.30 * frequency_score)
    }
}

fn calculate_overlap(requested: &[String], actual: &[String]) -> f64 {
    if requested.is_empty() { return 0.0; }
    let mut matches = 0;
    for req in requested {
        if actual.contains(req) {
            matches += 1;
        }
    }
    matches as f64 / requested.len() as f64
}
