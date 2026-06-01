# Kosh Long-Term Roadmap: The Path to a Context OS

## Phase 4: Deterministic Context Planning (Composition Engine)
*   **Goal**: Kosh *proves* the minimal context required for a task and composes it on-the-fly.
*   **Commands**: `kosh suggest`, `kosh context resolve`, `kosh why`.
*   **KPI**: 100% provable context selection based on symbol overlap and ROI.

## Phase 5: Fact Memory (MadCat Fact Engine)
*   **Goal**: Persistent "Institutional Memory" for architectural decisions and agent discoveries.
*   **Mechanism**: Store facts (not code) in a MadCat-inspired research layer *after* context planning is active.
*   **KPI**: Zero re-discovery turns for established project patterns.

## Kosh + MadCat Execution Pipeline
1. Question -> 2. Fingerprint Match -> 3. Lease Match -> 4. Symbol Graph -> 5. **MadCat Facts** -> 6. Answer

## Phase 6: Kosh Proxy (Transparent Optimization)
*   **Goal**: Transparently intercept standard tools (git, find, read_file) and optimize them without agent modification.
*   **Mechanism**: A protocol-level shim between the agent and the environment.
*   **KPI**: 100% "Zero-Integration" token elimination.

## Phase 7: Context Economy Platform (Enterprise ROI)
*   **Goal**: Aggregate ROI analytics across organizations, repositories, and developers.
*   **KPI**: Clear visibility into cost and latency avoided ($ saved, hours saved).

## Phase 8: Rust Superpowers (Scaling to Millions of LOC)
*   **Goal**: Use high-performance Rust primitives for enterprise-scale.
*   **Tech**: `memmap2` (zero-copy), `postcard` (binary packets), `rayon` (parallel indexing).
*   **KPI**: Sub-millisecond context resolution in repositories with 10M+ lines of code.

## Phase 9: Kosh OS (The Endgame)
*   **Goal**: Sub-1000 token overhead for complex engineering tasks.
*   **Workflow**: Question -> [Fingerprint + Lease + Fact + Graph Match] -> Answer.
*   **KPI**: Zero file reads, zero packet loads, zero searches in the ideal turn.
