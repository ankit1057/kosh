use rusqlite::{params, Connection};
use std::path::Path;
use tree_sitter::{Parser, Query, QueryCursor};
use streaming_iterator::StreamingIterator;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolRecord {
    pub id: Option<i64>,
    pub name: String,
    pub kind: String,
    pub file_path: String,
    pub repo: String,
    pub content_hash: String,
}

pub struct SymbolTable {
    conn: Connection,
}

impl SymbolTable {
    pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS symbols (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                kind TEXT NOT NULL,
                file_path TEXT NOT NULL,
                repo TEXT NOT NULL,
                content_hash TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS lease_symbols (
                lease_id TEXT NOT NULL,
                symbol_id INTEGER NOT NULL,
                PRIMARY KEY (lease_id, symbol_id),
                FOREIGN KEY (symbol_id) REFERENCES symbols (id)
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS context_signatures (
                lease_id TEXT PRIMARY KEY,
                signature_hash TEXT NOT NULL,
                symbol_count INTEGER NOT NULL,
                version TEXT NOT NULL DEFAULT 'v1'
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols (name)",
            [],
        )?;

        Ok(Self { conn })
    }

    pub fn insert_symbol(&self, symbol: &SymbolRecord) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO symbols (name, kind, file_path, repo, content_hash) VALUES (?, ?, ?, ?, ?)",
            params![symbol.name, symbol.kind, symbol.file_path, symbol.repo, symbol.content_hash],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn associate_lease(&self, lease_id: &str, symbol_id: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO lease_symbols (lease_id, symbol_id) VALUES (?, ?)",
            params![lease_id, symbol_id],
        )?;
        Ok(())
    }

    pub fn upsert_signature(&self, lease_id: &str, hash: &str, count: usize) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO context_signatures (lease_id, signature_hash, symbol_count) 
             VALUES (?, ?, ?) 
             ON CONFLICT(lease_id) DO UPDATE SET 
                signature_hash = excluded.signature_hash,
                symbol_count = excluded.symbol_count",
            params![lease_id, hash, count as i64],
        )?;
        Ok(())
    }

    pub fn get_lease_signature(&self, lease_id: &str) -> rusqlite::Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.name FROM symbols s 
             JOIN lease_symbols ls ON s.id = ls.symbol_id 
             WHERE ls.lease_id = ?
             ORDER BY s.name ASC"
        )?;
        let rows = stmt.query_map(params![lease_id], |row| row.get(0))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn get_signature_metadata(&self, lease_id: &str) -> rusqlite::Result<Option<(String, String, usize)>> {
        let mut stmt = self.conn.prepare(
            "SELECT signature_hash, version, symbol_count FROM context_signatures WHERE lease_id = ?"
        )?;
        let mut rows = stmt.query(params![lease_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some((row.get(0)?, row.get(1)?, row.get::<_, i64>(2)? as usize)))
        } else {
            Ok(None)
        }
    }
}

pub fn compute_signature_hash(symbols: &[String]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    let mut sorted = symbols.to_vec();
    sorted.sort();
    for sym in sorted {
        sym.hash(&mut hasher);
    }
    format!("{:x}", hasher.finish())
}

pub struct DartExtractor;

impl DartExtractor {
    pub fn extract_symbols(source: &str) -> Vec<(String, String)> {
        let mut parser = Parser::new();
        let language = tree_sitter::Language::from(tree_sitter_dart::LANGUAGE);
        parser.set_language(&language).expect("Error loading Dart grammar");
        let tree = parser.parse(source, None).expect("Error parsing source");

        let mut symbols = Vec::new();
        let mut cursor = tree.walk();
        Self::traverse_recursive(tree.root_node(), &mut cursor, source, &mut symbols);
        symbols
    }

    fn traverse_recursive(node: tree_sitter::Node, cursor: &mut tree_sitter::TreeCursor, source: &str, out: &mut Vec<(String, String)>) {
        match node.kind() {
            "class_declaration" | "mixin_declaration" | "enum_declaration" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    out.push((name_node.utf8_text(source.as_bytes()).unwrap_or("").to_string(), "type".to_string()));
                }
            }
            "method_declaration" | "function_signature" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    out.push((name_node.utf8_text(source.as_bytes()).unwrap_or("").to_string(), "function".to_string()));
                }
            }
            _ => {}
        }

        if cursor.goto_first_child() {
            loop {
                Self::traverse_recursive(cursor.node(), cursor, source, out);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
        }
    }
}

pub struct RustExtractor;

impl RustExtractor {
    pub fn extract_symbols(source: &str) -> Vec<(String, String)> {
        let mut parser = Parser::new();
        let language = tree_sitter::Language::from(tree_sitter_rust::LANGUAGE);
        parser.set_language(&language).expect("Error loading Rust grammar");
        let tree = parser.parse(source, None).expect("Error parsing source");

        // Query for structs, impls, and functions
        let query_str = r#"
            (function_item name: (identifier) @function.name)
            (struct_item name: (type_identifier) @struct.name)
            (impl_item type: (type_identifier) @impl.type)
        "#;
        let query = Query::new(&language, query_str).expect("Error creating query");
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

        let mut symbols = Vec::new();
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let name = capture.node.utf8_text(source.as_bytes()).unwrap_or("").to_string();
                let kind = query.capture_names()[capture.index as usize].to_string();
                symbols.push((name, kind));
            }
        }
        symbols
    }
}
