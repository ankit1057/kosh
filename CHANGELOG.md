# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-06-01

### Added
- **Core Identity**: Project renamed to **Kosh**, focusing on Token Economics and Context Virtualization.
- **Context Leasing**: Introduced stable, non-expiring context handles (e.g., `lease:auth:001`) to eliminate redundant retransmissions.
- **Context Packets**: Added grouping for related files and symbols into named bundles.
- **MCP Batching**: Implemented batch execution for multiple tool calls in a single roundtrip.
- **Command Aliases**: Shorthand for common shell commands (e.g., `gs` -> `git status`).
- **Symbol Aliases**: Compact handles for file paths (e.g., `@authrepo` -> `src/auth/repo.rs`).
- **Indexer**: File inventory, language detection, hashing, and change detection.
- **Gain Tracking**: Heuristic-based tracking of token and cost savings, supporting kind, repo, feature, and context breakdowns.
- **TSV Storage**: Fast, human-readable state management for leases, history, packets, and indexing.

### Security
- Enforced `#![forbid(unsafe_code)]` across all crates.
