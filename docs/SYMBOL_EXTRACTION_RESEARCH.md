# Kosh Multi-Language Symbol Extraction Research Report

## Executive Summary
Kosh aims to provide high-performance, deterministic Context Signatures for agentic workflows. To achieve this, we need robust symbol extraction across multiple programming languages. This report analyzes existing Open Source Software (OSS) tools and proposes an implementation strategy for Kosh's `symbol_extractor` crate.

## 1. Analyzed Technologies

### Tree-sitter (Primary Recommendation)
Tree-sitter is a parser generator tool and an incremental parsing library. It builds a concrete syntax tree (CST) for a source file and efficiently updates the syntax tree as the source file is edited.

- **Pros**:
    - **Performance**: Extremely fast, suitable for real-time analysis.
    - **Robustness**: Designed for IDEs, handles syntax errors gracefully.
    - **Ecosystem**: Support for 100+ languages with mature grammars.
    - **Native Rust**: Excellent Rust bindings (`tree-sitter` crate).
    - **Deterministic**: Produces the same tree for the same input.
- **Cons**:
    - **Local Scope**: Primarily single-file analysis; lacks cross-file semantic resolution (e.g., doesn't know where a type is defined if it's in another file).

### SCIP (Symbolic Code Intelligence Protocol)
SCIP is a successor to LSIF (Language Server Index Format) developed by Sourcegraph. It provides a standard format for semantic code indexing.

- **Pros**:
    - **Precision**: Full semantic accuracy (handles cross-file references).
    - **Standardized**: Works across many languages (Java, Go, Python, etc.).
- **Cons**:
    - **Weight**: Typically requires running a separate indexer binary per language and access to a build system (e.g., Maven, Go modules).
    - **Complexity**: Harder to integrate into a minimal-dependency Rust binary compared to Tree-sitter.

### Recommendation
**Kosh should prioritize Tree-sitter** for its initial multi-language expansion. Tree-sitter's performance and zero-config nature align perfectly with Kosh's goal of fast context representation and elimination. SCIP can be explored later for "Deep Semantic Signatures" if cross-file resolution becomes a bottleneck.

## 2. Language-Specific OSS Selection

| Language | Grammar Project | Query Source (tags.scm) |
| :--- | :--- | :--- |
| **Java** | `tree-sitter-java` | GitHub / Neovim |
| **Kotlin** | `tree-sitter-kotlin` | fwcd / community |
| **JS/TS** | `tree-sitter-typescript` | Tree-sitter authors |
| **Python** | `tree-sitter-python` | Max Brunsfeld |
| **Go** | `tree-sitter-go` | Max Brunsfeld |
| **C++** | `tree-sitter-cpp` | Max Brunsfeld |
| **Rust** | `tree-sitter-rust` | Tree-sitter authors |
| **Dart** | `tree-sitter-dart` | User G (already in Kosh) |

## 3. Implementation Strategy

### Unified Extractor Architecture
Instead of per-language extractor structs (e.g., `DartExtractor`, `RustExtractor`), we will implement a `GenericTreeSitterExtractor`.

```rust
pub struct LanguageConfig {
    pub language: tree_sitter::Language,
    pub query: &'static str,
    pub kind_mapping: HashMap<&'static str, &'static str>,
}

pub struct GenericTreeSitterExtractor;

impl GenericTreeSitterExtractor {
    pub fn extract_symbols(source: &str, config: &LanguageConfig) -> Vec<(String, String)> {
        // ... implementation using tree-sitter queries ...
    }
}
```

### Steps:
1.  **Refactor**: Migrate existing `RustExtractor` to the generic pattern.
2.  **Add Queries**: Incorporate `tags.scm` queries for Java, Kotlin, Python, JS, TS, Go, and C++.
3.  **Kind Normalization**: Map diverse tree-sitter captures (e.g., `@definition.method`, `@definition.function`) to a unified set of Kosh kinds (`function`, `type`, `field`).
4.  **Dependency Management**: Add the required `tree-sitter-<lang>` crates to `Cargo.toml`.

## 4. Prior Art and Credit
Kosh stands on the shoulders of giants. We will maintain an `OSS_CREDITS.md` file to acknowledge the contributions of:
- **Max Brunsfeld** and the Tree-sitter community for the core engine.
- **GitHub** for the standardized `tags.scm` queries that power code navigation.
- **Helix** and **Neovim** (nvim-treesitter) for community-maintained query improvements.
- Language-specific grammar maintainers (e.g., `fwcd` for Kotlin).

## 5. Next Steps
1.  Initialize `OSS_CREDITS.md`.
2.  Update `symbol_extractor` to support Python and TypeScript (already in `Cargo.toml`).
3.  Implement the `GenericTreeSitterExtractor` refactor.
