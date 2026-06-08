# Implementation Plan: Multi-Language Symbol Extraction

This document outlines the step-by-step implementation of the multi-language symbol extraction expansion for Kosh.

## Phase 1: Infrastructure Refactoring
Goal: Replace specialized extractors with a generic, data-driven approach.

1.  **Define `LanguageConfig`**:
    - Create a registry of supported languages in `crates/symbol_extractor/src/lib.rs`.
    - Each entry includes the `tree_sitter::Language`, the query string, and a kind-mapping.

2.  **Implement `GenericTreeSitterExtractor`**:
    - A single struct that takes `LanguageConfig` and source code.
    - Uses `tree_sitter::Query` to extract captures.
    - Normalizes capture names to Kosh's standard kinds (`type`, `function`, `field`).

3.  **Refactor `RustExtractor` and `DartExtractor`**:
    - Migrate them to use the generic implementation.

## Phase 2: Immediate Language Support
Goal: Enable languages already present in `Cargo.toml`.

1.  **Python Support**:
    - Add `tags.scm` query for Python.
    - Update `SymbolExtractor` factory to handle `.py` files.
2.  **TypeScript Support**:
    - Add `tags.scm` query for TypeScript.
    - Update `SymbolExtractor` factory to handle `.ts` and `.tsx` files.

## Phase 3: Expansion (New Dependencies)
Goal: Add Java, Kotlin, Go, C++, and JavaScript.

1.  **Update `Cargo.toml`**:
    - Add `tree-sitter-java`, `tree-sitter-kotlin`, `tree-sitter-go`, `tree-sitter-cpp`, `tree-sitter-javascript`.
2.  **Add Queries**:
    - Source and normalize `tags.scm` for each language.
3.  **Registration**:
    - Add them to the `LanguageConfig` registry.

## Phase 4: Validation and Benchmarking
Goal: Ensure the Kosh "Deterministic" mandate is met.

1.  **Test Suite**:
    - Create a test file for each language with representative code.
    - Verify extracted symbols match expectations.
2.  **Performance Check**:
    - Ensure extraction time remains sub-millisecond for typical source files.
3.  **Token Savings Audit**:
    - Verify that Context Signatures for these languages effectively eliminate redundant context in agent turns.

## Normalized Kosh Kinds
To ensure consistency across languages, we map diverse grammar captures to these three primary kinds:
- `type`: Classes, Interfaces, Structs, Enums, Records.
- `function`: Methods, Functions, Constructors, Lambdas.
- `field`: Properties, Fields, Enum Constants, Global Variables.
