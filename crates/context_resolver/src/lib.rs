use cache_engine::db::{ContextFingerprintV2, DbLeaseStore};
use packet_engine::PacketStore;
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ContextRecommendation {
    pub lease_id: Option<String>,
    pub packet_name: Option<String>,
    pub confidence: f64,
    pub estimated_tokens_saved: u64,
    pub reason: String,
}

pub struct ContextResolver {
    lease_store: DbLeaseStore,
    packet_store: PacketStore,
}

impl ContextResolver {
    pub fn open(db_path: impl AsRef<Path>, packet_tsv_path: impl AsRef<Path>) -> Result<Self, String> {
        let lease_store = DbLeaseStore::open(db_path).map_err(|e| e.to_string())?;
        let packet_store = PacketStore::load(packet_tsv_path).map_err(|e| e.to_string())?;
        Ok(Self {
            lease_store,
            packet_store,
        })
    }

    pub fn resolve_from_fingerprint(&self, fingerprint: &ContextFingerprintV2) -> Option<ContextRecommendation> {
        let hash = fingerprint.deterministic_hash();
        match self.lease_store.find_by_fingerprint(&hash) {
            Ok(Some(id)) => {
                // If we found an exact fingerprint match, confidence is high.
                Some(ContextRecommendation {
                    lease_id: Some(id),
                    packet_name: None,
                    confidence: 1.0,
                    estimated_tokens_saved: 5000, // Heuristic for now
                    reason: "Exact fingerprint match found in lease store.".to_string(),
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
                estimated_tokens_saved: 2000, // Heuristic
                reason: format!("Direct match for packet: {}", packet.name),
            });
        }

        // 2. Fuzzy/Keyword match (very simple for now)
        for packet in self.packet_store.records() {
            if packet.name.contains(query) && packet.name != query {
                results.push(ContextRecommendation {
                    lease_id: None,
                    packet_name: Some(packet.name.clone()),
                    confidence: 0.7,
                    estimated_tokens_saved: 2000,
                    reason: format!("Partial match for packet: {}", packet.name),
                });
            }
        }

        results.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        results
    }
}
