# Current Honest State

Date: 2026-06-01

This repository is **Kosh** — a token elimination and context efficiency platform for agentic development.

## Identity

- **Project:** Kosh (The Context Virtualization Platform)
- **Technology:** RTK (Rust Token Killer) — The high-performance token elimination engine
- **Binary:** `rtk` (The command-line tool)
- **Config dir:** `.rtk/`
- **Env vars:** `RTK_REPO`, `RTK_FEATURE`

## Context Virtualization USP
Kosh's USP is not just "token killing" (which RTK does), but **Context Virtualization**. By using stable leases and packets, Kosh enables agents to work in massive repositories with minimal token overhead.


## Constraint

No LLMs, no embeddings, no graph intelligence until Kosh proves substantial token savings using deterministic systems alone.

## What Exists (Milestone 2 Complete)

Rust workspace with these crates:
- `apps/cli` — binary `rtk`
- `crates/tool_registry` — command alias expansion
- `crates/mcp_router` — MCP alias expansion and symbol resolution
- `crates/cache_engine` — context cache + context leasing (`lease:auth:001` style handles)
- `crates/cost_estimator` — compression history and gain tracking
- `crates/indexer` — file index snapshot
- `crates/packet_engine` — context packet bundles
- `crates/skill_engine` — executable skill workflows

### Capabilities

- **Command aliases:** `rtk gs`, `rtk gd` etc.
- **MCP aliases:** `rtk mcp expand "rf @authrepo"` expands to structured tool calls
- **Symbol aliases:** `@authrepo => lib/features/auth/...`
- **Context Cache:** fingerprint-keyed cache in `.rtk/cache.tsv`
- **Context Leasing:** stable handles (`lease:auth:001`) with high-accuracy `byte_size` tracking.
- **MCP Batching:** `rtk batch '[{"tool":"read_file","path":"..."}]'` — collapses N serial calls.
- **Context Packets:** Grouped file and symbol bundles. `rtk packet load` resolves symbols and outputs a single MCP batch.
- **Skill References:** Named executable workflows. `rtk skill run` executes a bundle of actions and records `skill_run` in gain history.
- **MCP Server Stub:** `rtk serve` provides a minimal stdio JSON-RPC server for agent discovery and tool routing.
- **Repository Indexing:** `rtk index`, `rtk index diff`
- **Gain Tracking:** `rtk gain`, `rtk gain --by-kind`, `rtk gain --history` — tracks all savings

## What Does Not Exist Yet

- No real MCP transport beyond stdio stub.
- No daemon.
- No automatic packet generation (manual creation only).

## Next Sensible Implementation Steps

1. **Automatic Packets:** Use the indexer or basic regex import tracing to suggest or create packets automatically.
2. **Persistence Refinement:** Move from TSV to a more robust local storage if needed (e.g. SQLite if complexity grows, though TSV is currently prioritized for "zero dependency").
