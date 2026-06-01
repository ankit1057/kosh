//! Deterministic MCP stdio proxy.
//! Intercepts JSON-RPC messages between an MCP host (Claude Code, Cursor)
//! and an upstream MCP server. Applies caching and deduplication rules.
//! No LLMs. Same input + same rules = same output, every time.

use std::collections::HashMap;

// ── FNV-1a hash (copied from crates/context_signatures) ──────────────────────

fn fnv1a(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325; // FNV offset basis
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3); // FNV prime
    }
    hash
}

// ── CacheKey ──────────────────────────────────────────────────────────────────

/// Stable identity for a JSON-RPC request: method + FNV-1a hash of params.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub method: String,
    pub params_hash: u64,
}

impl CacheKey {
    /// Build a cache key from a method name and the raw params JSON string.
    pub fn from_message(method: &str, params: &str) -> Self {
        Self {
            method: method.to_owned(),
            params_hash: fnv1a(params),
        }
    }
}

// ── CachedResponse ────────────────────────────────────────────────────────────

/// A stored JSON-RPC response with metadata.
#[derive(Debug, Clone)]
pub struct CachedResponse {
    /// Raw JSON-RPC response string.
    pub response: String,
    /// How many times this entry has been served from cache.
    pub hit_count: u64,
    /// Unix timestamp (seconds) when the entry was first inserted.
    pub created_at: u64,
    /// Byte length of `response`.
    pub byte_size: usize,
}

// ── ProxyCache ────────────────────────────────────────────────────────────────

/// In-memory LRU-free cache for MCP responses.
pub struct ProxyCache {
    entries: HashMap<CacheKey, CachedResponse>,
    hits: u64,
    misses: u64,
}

impl Default for ProxyCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ProxyCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            hits: 0,
            misses: 0,
        }
    }

    /// Look up a cached response. Increments both the global hit counter and
    /// the entry's `hit_count`. Returns `None` (and increments miss counter)
    /// if the key is absent.
    pub fn get(&mut self, key: &CacheKey) -> Option<&CachedResponse> {
        if let Some(entry) = self.entries.get_mut(key) {
            entry.hit_count += 1;
            self.hits += 1;
            // Re-borrow immutably to satisfy the borrow checker.
            Some(self.entries.get(key).unwrap())
        } else {
            self.misses += 1;
            None
        }
    }

    /// Insert a new response. `created_at` should be a Unix timestamp in
    /// seconds; callers are responsible for supplying the current time so
    /// this struct stays deterministic and test-friendly.
    pub fn insert(&mut self, key: CacheKey, response: String, created_at: u64) {
        let byte_size = response.len();
        self.entries.insert(
            key,
            CachedResponse {
                response,
                hit_count: 0,
                created_at,
                byte_size,
            },
        );
    }

    /// Total cache hits across all lookups.
    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Total cache misses across all lookups.
    pub fn misses(&self) -> u64 {
        self.misses
    }

    /// Number of distinct entries stored.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when the cache contains no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Fraction of lookups served from cache.
    /// Returns `0.0` when no lookups have been made yet.
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

// ── JSON-RPC helpers ──────────────────────────────────────────────────────────

/// Extract the `method` field from a JSON-RPC request string.
///
/// Uses a minimal hand-rolled scan to stay dependency-free.
/// Returns `None` if the string is not parseable as a JSON-RPC message.
pub fn extract_method(json: &str) -> Option<String> {
    extract_string_field(json, "method")
}

/// Extract the `id` field from a JSON-RPC request string.
///
/// Returns the value as a string regardless of whether it was a JSON string
/// or a JSON number in the original message.
pub fn extract_id(json: &str) -> Option<String> {
    // Try as quoted string first, then as bare number.
    if let Some(v) = extract_string_field(json, "id") {
        return Some(v);
    }
    extract_number_field(json, "id")
}

/// Returns `true` if the JSON-RPC message has no `id` field (i.e. it is a
/// notification and should be forwarded as-is without caching).
pub fn is_notification(json: &str) -> bool {
    extract_id(json).is_none()
}

/// Returns `true` if the given JSON-RPC method should be cached.
///
/// Cacheable methods:
/// - `tools/call`
/// - `resources/read`
///
/// Not cacheable:
/// - `tools/list`
/// - `initialize`
/// - `notifications/*` (any method starting with `notifications/`)
pub fn is_cacheable(method: &str) -> bool {
    matches!(method, "tools/call" | "resources/read")
}

/// Build a JSON-RPC 2.0 error response for the given `id`.
///
/// ```text
/// {"jsonrpc":"2.0","id":<id>,"error":{"code":<code>,"message":"<message>"}}
/// ```
pub fn error_response(id: &str, code: i32, message: &str) -> String {
    // Escape the id: if it looks like a number emit it bare, otherwise quote it.
    let id_json = if id.chars().all(|c| c.is_ascii_digit() || c == '-') && !id.is_empty() {
        id.to_owned()
    } else {
        format!("\"{}\"", id.replace('\\', "\\\\").replace('"', "\\\""))
    };
    let msg_escaped = message.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{id_json},\"error\":{{\"code\":{code},\"message\":\"{msg_escaped}\"}}}}",
    )
}

// ── ProxyStats ────────────────────────────────────────────────────────────────

/// Aggregate statistics for a running proxy session.
#[derive(Debug, Clone, Default)]
pub struct ProxyStats {
    pub requests_intercepted: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    /// Total bytes of upstream responses avoided by cache hits.
    pub bytes_avoided: u64,
    /// Number of upstream calls avoided by cache hits.
    pub calls_avoided: u64,
}

impl ProxyStats {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a cache hit that saved `bytes_saved` bytes of upstream traffic.
    pub fn record_hit(&mut self, bytes_saved: usize) {
        self.requests_intercepted += 1;
        self.cache_hits += 1;
        self.bytes_avoided += bytes_saved as u64;
        self.calls_avoided += 1;
    }

    /// Record a cache miss (upstream was called).
    pub fn record_miss(&mut self) {
        self.requests_intercepted += 1;
        self.cache_misses += 1;
    }

    /// Fraction of intercepted requests served from cache.
    /// Returns `0.0` when no requests have been recorded.
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            self.cache_hits as f64 / total as f64
        }
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Scan `json` for `"<field>": "<value>"` and return the value string.
///
/// This is intentionally minimal — it handles the well-formed JSON-RPC
/// messages produced by standard MCP hosts. It is NOT a general JSON parser.
fn extract_string_field(json: &str, field: &str) -> Option<String> {
    let needle = format!("\"{}\"", field);
    let start = json.find(&needle)?;
    let after_key = &json[start + needle.len()..];
    // Skip whitespace and the colon.
    let after_colon = after_key.trim_start().strip_prefix(':')?.trim_start();
    if !after_colon.starts_with('"') {
        return None;
    }
    // Collect until the closing unescaped quote.
    let inner = &after_colon[1..];
    let mut value = String::new();
    let mut chars = inner.chars();
    loop {
        match chars.next()? {
            '\\' => {
                // consume the escaped character
                let escaped = chars.next()?;
                value.push('\\');
                value.push(escaped);
            }
            '"' => break,
            c => value.push(c),
        }
    }
    Some(value)
}

/// Scan `json` for `"<field>": <number>` and return the number as a string.
fn extract_number_field(json: &str, field: &str) -> Option<String> {
    let needle = format!("\"{}\"", field);
    let start = json.find(&needle)?;
    let after_key = &json[start + needle.len()..];
    let after_colon = after_key.trim_start().strip_prefix(':')?.trim_start();
    // Collect digits (and possible leading minus).
    let num: String = after_colon
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    if num.is_empty() {
        None
    } else {
        Some(num)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // 1. Same params → same hash
    #[test]
    fn cache_key_same_params_same_hash() {
        let k1 = CacheKey::from_message("tools/call", r#"{"name":"grep","input":{}}"#);
        let k2 = CacheKey::from_message("tools/call", r#"{"name":"grep","input":{}}"#);
        assert_eq!(k1, k2);
    }

    // 2. Different params → different hash
    #[test]
    fn cache_key_different_params_different_hash() {
        let k1 = CacheKey::from_message("tools/call", r#"{"name":"grep","input":{}}"#);
        let k2 = CacheKey::from_message("tools/call", r#"{"name":"find","input":{}}"#);
        assert_ne!(k1.params_hash, k2.params_hash);
    }

    // 3. extract_method happy path
    #[test]
    fn extract_method_valid_request() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{}}"#;
        assert_eq!(extract_method(json).as_deref(), Some("tools/call"));
    }

    // 4. extract_method on garbage
    #[test]
    fn extract_method_invalid_json_returns_none() {
        assert_eq!(extract_method("not json at all"), None);
        assert_eq!(extract_method("{}"), None);
    }

    // 5. tools/call is cacheable
    #[test]
    fn is_cacheable_tools_call_true() {
        assert!(is_cacheable("tools/call"));
        assert!(is_cacheable("resources/read"));
    }

    // 6. tools/list is NOT cacheable
    #[test]
    fn is_cacheable_tools_list_false() {
        assert!(!is_cacheable("tools/list"));
        assert!(!is_cacheable("initialize"));
    }

    // 7. notifications/* are NOT cacheable
    #[test]
    fn is_cacheable_notification_false() {
        assert!(!is_cacheable("notifications/progress"));
        assert!(!is_cacheable("notifications/message"));
    }

    // 8. Cache hit increments hit_count on the entry
    #[test]
    fn proxy_cache_hit_increments_count() {
        let mut cache = ProxyCache::new();
        let key = CacheKey::from_message("tools/call", "{}");
        cache.insert(key.clone(), r#"{"result":{}}"#.to_owned(), 1000);

        cache.get(&key);
        cache.get(&key);

        let entry = cache.entries.get(&key).unwrap();
        assert_eq!(entry.hit_count, 2);
        assert_eq!(cache.hits(), 2);
    }

    // 9. Miss then hit — counters correct
    #[test]
    fn proxy_cache_miss_then_hit() {
        let mut cache = ProxyCache::new();
        let key = CacheKey::from_message("resources/read", r#"{"uri":"file:///foo"}"#);

        // First lookup → miss
        assert!(cache.get(&key).is_none());
        assert_eq!(cache.misses(), 1);

        // Insert then lookup → hit
        cache.insert(key.clone(), r#"{"result":"content"}"#.to_owned(), 42);
        assert!(cache.get(&key).is_some());
        assert_eq!(cache.hits(), 1);
        assert_eq!(cache.misses(), 1);
        assert!((cache.hit_rate() - 0.5).abs() < 1e-10);
    }

    // 10. error_response produces valid-looking JSON
    #[test]
    fn error_response_valid_json() {
        let resp = error_response("42", -32600, "Invalid Request");
        assert!(resp.contains("\"jsonrpc\":\"2.0\""));
        assert!(resp.contains("\"id\":42"));
        assert!(resp.contains("\"code\":-32600"));
        assert!(resp.contains("\"message\":\"Invalid Request\""));

        // String id
        let resp2 = error_response("req-1", -32601, "Method not found");
        assert!(resp2.contains("\"id\":\"req-1\""));
    }

    // 11. is_notification — no id field means notification
    #[test]
    fn is_notification_no_id_true() {
        let notif = r#"{"jsonrpc":"2.0","method":"notifications/progress","params":{}}"#;
        assert!(is_notification(notif));

        let request = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{}}"#;
        assert!(!is_notification(request));
    }

    // 12. ProxyStats hit_rate
    #[test]
    fn stats_hit_rate_correct() {
        let mut stats = ProxyStats::new();
        assert_eq!(stats.hit_rate(), 0.0);

        stats.record_miss();
        stats.record_miss();
        stats.record_hit(512);
        stats.record_hit(1024);

        assert_eq!(stats.cache_hits, 2);
        assert_eq!(stats.cache_misses, 2);
        assert_eq!(stats.bytes_avoided, 1536);
        assert_eq!(stats.calls_avoided, 2);
        assert!((stats.hit_rate() - 0.5).abs() < 1e-10);
    }
}
