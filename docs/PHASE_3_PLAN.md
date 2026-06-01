# Phase 3: Rust-Native Context Operating System (Revised v2)

## Mission: From Optimization to Context Awareness & Decision Making
Eliminate context retransmission by enabling Kosh to determine that "context is already known" and recommending the optimal minimal context before a tool call happens.

---

## Phase 3.1: Context Awareness Foundation (Week 1 - Completed)
*   **Context Fingerprint V2**: Branch/Commit/Symbol awareness for state detection.
*   **Lease Intelligence**: SQLite-backed tracking of `hits`, `last_used`, and `tokens_saved`.

---

## Phase 3.5: Context Resolver (Week 1.5 - IN PROGRESS)
*   **Goal**: Move from "context storage" to "context decision making."
*   **Objectives**:
    1. Resolve active environment fingerprints into available leases.
    2. Resolve requested symbols into existing packets.
    3. Rank candidates by estimated token/read savings.
    4. Provide deterministic recommendations to the agent.
*   **Commands**:
    - `kosh context resolve <query>`: Find the best lease/packet match.
    - `kosh context suggest`: Proactively suggest context based on current repo state.
    - `kosh context explain`: Justify a context recommendation based on ROI.
*   **KPI**: Elimination of "Exploratory Turns" before they even begin.

---

## Phase 3.6: Symbol Extraction (Week 2)
*   **Goal**: Extract concepts (classes, functions) into an SQLite `symbol_table`.
*   **Tech**: `tree-sitter`.
*   **Economics**: Provide a map of the repository without reading source code.

---

## Phase 3.7: Relationship Extraction (Week 3)
*   **Goal**: Build an SQLite `relations` table (`USES`, `CALLS`).
*   **Tech**: `petgraph`.
*   **Outcome**: High-speed resolution of dependencies to minimize lease injection.

---

## Phase 3.8: Context Diff & Delta Engine (Week 4)
*   **Goal**: Send only deltas (+/- lines) when a leased context changes slightly.
*   **KPI**: 95% reduction in re-transmission for "edit-verify" loops.

---

## Roadmap Summary
1. Fingerprint -> 2. Lease Intelligence -> **3. Context Resolver** -> 4. Symbol Extraction -> 5. Symbol Relations -> 6. Context Planning -> 7. MadCat Facts -> 8. Proxy
