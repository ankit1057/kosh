# Current Honest State

Date: 2026-06-01

This repository is **Kosh** — token elimination and context efficiency infrastructure for AI agents.

## Milestone 2: COMPLETED
- **Context Virtualization**: Proved 91.4% payload reduction (leasing 89.2%, packets 98.4%, batching 93.8%, compression 33.3%).
- Binary: `kosh`, config: `.kosh/`, fallback: `.rtk/`
- Primitive benchmark: `agent_test/kosh_benchmarks.py` — no LLM, raw payload measurement.

## Research findings (2026-06-01)
External validation confirms Kosh's thesis:
- 30,400 of 48,400 SWE-bench tokens came from tool results; 39.9–59.7% removable with no performance loss.
- 10-step naive agent loop costs 43.3× more than a single call (triangular history accumulation).
- Deterministic retrieval (SQLite FTS5, BM25, RRF) is competitive with embeddings for code context.
- mcpwall proves transparent MCP stdio proxy is feasible, deterministic, production-ready.
- SWE-Pruner: 23-54% token reduction while *improving* task success (uses 0.6B skimmer — not Kosh's path).
- Poorly chosen context actively hurts, not neutrally — validates minimum-context thesis.

## Roadmap
```
v0.1  ✅  Aliases, symbols, cache, gain tracking, indexer
v0.2  ✅  Context leasing, MCP batching, context packets, kosh rename
v0.3  🔲  kosh benchmark CLI, packet symbol resolution, lease size accuracy
v0.4  🔲  Context signatures + composition (kosh signature match/compose — crate DONE, CLI pending)
v0.5  🔲  Real session replay benchmark — actual API, prompt caching both arms, savings + task-success
v0.6  🔲  MCP proxy — transparent stdio interception
v0.7  🔲  Deterministic context planner — task → minimum required files, no repo exploration
v0.8  🔲  Fact engine — TSV + SQLite FTS5, confidence scores, architecture decisions
v0.9  🔲  Kosh OS — 0 redundant reads, context fingerprint → lease → fact → answer
```

## Active work
- `crates/context_signatures` — DONE (8 tests passing). Jaccard overlap, subsumption, composition, TSV round-trip.
- `kosh signature` CLI subcommand — IN PROGRESS (Gemini agent).
- `agent_test/session_replay.py` — IN PROGRESS (Gemini agent).

## Constraint
No LLMs, no embeddings, no graph intelligence until Kosh proves substantial token savings using deterministic systems alone.

## Push credential
`GITHUB_TOKEN="" git push` (ankit1057 is active gh account).
