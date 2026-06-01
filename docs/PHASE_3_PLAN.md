# Phase 3: Rust-Native Context Operating System (Revised v5)

## Mission: Context DNA via Signatures
Kosh identifies and recommends context by comparing **Context Signatures** (sets of code symbols), enabling deterministic selection based on code concepts.

---

## Phase 3.1 - 3.5b: Awareness & Decision Foundation (Completed)
*   **Context Fingerprint V2**: State detection (Commit/Branch/Symbols).
*   **Lease Intelligence**: SQLite ROI tracking (Hits/Savings).
*   **Scoring & Explainability**: Multi-factor decision engine and `kosh context explain`.

---

## Phase 3.6: Symbol Extraction & Signatures (Week 2 - IN PROGRESS)
*   **Goal**: Extract symbols and group them into **Context Signatures**.
*   **Language Priority**: **Dart** (End-to-End implementation first).
*   **New Schema**: `lease_symbols` table to map leases to their contained symbols.
*   **Commands**: `kosh context signature <lease_id>`.
*   **KPI**: Ability to see the "DNA" (symbol set) of any leased context.

---

## Phase 3.6b: Signature Overlap Scoring (Week 2.5)
*   **Goal**: Compare task signatures vs. lease signatures.
*   **Formula**: 
    `score = 0.30*recency + 0.25*savings + 0.20*frequency + 0.25*signature_overlap`
*   **KPI**: Recommending `lease:auth` because it contains 90% of the requested symbols.

---

## Phase 3.7: Context Composition (Week 3)
*   **Goal**: Compose the minimal bundle of leases/packets to satisfy a signature.

---

## Phase 3.8: Context Identity & Versioning (Week 4)
*   **Goal**: Track `symbol_versions` and `lease_versions` to handle repo evolution.

---

## Roadmap Summary
1. Fingerprint -> 2. Lease Intel -> 3. Resolver -> 4. Scoring/Why -> **5. Signatures (Dart First)** -> 6. Overlap Scoring -> 7. Context Composition -> 8. Versioning/Deltas -> 9. Deterministic Planner -> 10. MadCat Facts -> 11. Proxy
