# Phase 3: Rust-Native Context Operating System - Technical Specification

## 1. Honest State Findings (End of Phase 2)

### What Works
- **Token Elimination Thesis**: Proved 91.4% payload reduction without LLMs.
- **Deterministic Virtualization**: Aliases, Leasing, and Packets are functional.
- **Gain Tracking**: Heuristic-based tracking is reliable and proves ROI.
- **Zero-Dependency Core**: Fast, minimal overhead, easily portable.

### What is Fragile/Limited
- **Concurrency**: TSV files lack locking; concurrent agent turns risk data corruption.
- **Query Performance**: Indexer and Symbol lookups are $O(N)$ string scans.
- **Exploration Overhead**: Agents still need to "search" before they can "lease".
- **Memory Footprint**: Full deserialization of packets/indexes is inefficient for large repos.

---

## 2. Phase 3 Architectural Specifications

### Priority 1: SQLite Migration (`kosh.db`)
- **Engine**: `rusqlite`.
- **Rationale**: Provides ACID compliance, atomic writes, and relational querying for gain analytics.
- **Schema**: Consolidate `cache`, `leases`, `history`, `packets`, and `index` into a single local database.
- **KPI**: 100% elimination of state corruption during parallel tool calls.

### Priority 2: Memory-Mapped Context Store (`memmap2`)
- **Implementation**: Map `.kpkt` (binary packets) directly into memory.
- **Rationale**: Zero-copy access. Instead of loading a 5MB packet into a `Vec<String>`, the agent reads only the specific byte offsets required.
- **KPI**: >90% reduction in RAM usage and sub-millisecond context retrieval.

### Priority 3: Symbol Graph for Elimination
- **Stack**: `tree-sitter` (parsers) + `petgraph` (graph engine).
- **Goal**: Resolve "What does `AuthService` depend on?" instantly.
- **Economics**: Allow Kosh to suggest the *exact* minimum lease required for a task, eliminating "just-in-case" file reads.
- **KPI**: 50% reduction in "exploratory" tool calls.

### Priority 4: Binary Context Packets (`.kpkt`)
- **Serialization**: `postcard` (efficient binary format).
- **Content**: Pre-indexed symbol offsets, fingerprints, and dependency pointers.
- **KPI**: 70% reduction in disk storage vs JSON packets.

### Priority 5: Context Diff Engine
- **Stack**: `similar`, `diffy`.
- **Logic**: If an agent has `lease:auth:001` but the file changed, send only the `edits` (delta) to update to `v2`.
- **KPI**: 95% reduction in re-transmission tokens when files undergo minor changes.

---

## 3. Week-by-Week Roadmap

### Week 1: SQLite & Fingerprint v2
- Migrate TSV loaders to `rusqlite`.
- Implement `ContextFingerprintV2` (Repo + Branch + Commit + Symbol List).
- Update `kosh gain` to use SQL queries for multi-dimensional ROI reporting.

### Week 2: Tree-Sitter & Petgraph
- Integrate `tree-sitter` for Rust/TypeScript/Python.
- Build the initial symbol dependency graph in `crates/indexer`.
- Add `kosh graph find <symbol>` command.

### Week 3: Binary Packets & Diff Engine
- Implement `.kpkt` serialization via `postcard`.
- Integrate `memmap2` for zero-copy reading.
- Implement delta-only context injection for leased handles.

### Week 4: MadCat Fact Memory
- Implement `crates/fact_engine` inspired by MadCat.
- Store architectural decisions and "agent discoveries" as persistent facts.
- Define memory policies (expire vs. pin).

---

## 4. Final Goal Architecture

```text
Question 
  ↓
[KOSH]
  1. Symbol Graph Lookup (Find dependencies)
  2. Lease/Cache Check (Already seen?)
  3. Fact Memory Check (Architecture rules?)
  ↓
Context Delta (Minimal bits)
  ↓
Answer
```

**Result**: Deep engineering work with sub-1000 token overhead per turn.
