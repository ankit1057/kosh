# Phase 3: Rust-Native Context Operating System (Revised v3)

## Mission: From Optimization to Data-Driven Context Decision Making
Eliminate context retransmission by enabling Kosh to deterministically prove and recommend the optimal minimal context based on historical ROI and state matching.

---

## Phase 3.1: Context Awareness Foundation (Completed)
*   **Context Fingerprint V2**: State detection (Commit/Branch/Symbols).
*   **Lease Intelligence**: SQLite tracking of hits, usage, and token ROI.

---

## Phase 3.5: Context Resolver (Completed)
*   **Engine**: `crates/context_resolver`.
*   **Capabilities**: Fingerprint-to-Lease mapping and Query-to-Packet resolution.

---

## Phase 3.5b: Recommendation Scoring & Explainability (IN PROGRESS)
*   **Goal**: Replace heuristics with data-driven decision making.
*   **Scoring Engine**: Implement a weighted formula for `ContextScore`:
    - `0.40 * recency` (Last used)
    - `0.30 * historical_savings` (Tokens saved)
    - `0.30 * frequency` (Access count)
*   **Explainability**: Implement `kosh context explain` to justify recommendations with real metrics.
*   **KPI**: 100% transparency in context selection logic.

---

## Phase 3.6: Symbol Extraction (Week 2)
*   **Goal**: Extract high-level symbols into an SQLite `symbol_table`.
*   **Optimization**: Enable **Symbol Overlap** scoring (calculating % match between query and lease content).

---

## Phase 3.7: Relationship Extraction & Graph (Week 3)
*   **Goal**: Map `relations` (USES, CALLS) to compose minimal dependency sets.

---

## Phase 4: Deterministic Context Planner
*   **Goal**: Kosh *proves* the minimal context required for a task and composes it on-the-fly.

---

## Roadmap Summary
1. Fingerprint -> 2. Lease Intelligence -> 3. Context Resolver -> **4. Scoring & Explainability** -> 5. Symbol Extraction -> 6. Relations -> 7. Deterministic Planner -> 8. MadCat Facts
