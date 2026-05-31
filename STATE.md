# Current Honest State

Date: 2026-06-01

This repository is **Kosh** — a token elimination and context efficiency platform for agentic development.

## Identity

- **Project:** Kosh
- **Technology:** RTK (Rust Token Killer) — the underlying binary and engine
- **Binary:** `kosh`
- **Config dir:** `.kosh/` (falls back to `.rtk/` for existing data)
- **Env vars:** `KOSH_REPO`, `KOSH_FEATURE` (fall back to `RTK_REPO`, `RTK_FEATURE`)

## Constraint

No LLMs, no embeddings, no graph intelligence until Kosh proves substantial token savings using deterministic systems alone.

## What Exists (Milestone 2 Complete)

Rust workspace with these crates:
- `apps/cli` — binary `kosh`
- `crates/tool_registry` — command alias expansion
- `crates/mcp_router` — MCP alias expansion and symbol resolution
- `crates/cache_engine` — context cache + context leasing (`lease:auth:001` style handles)
- `crates/cost_estimator` — compression history and gain tracking
- `crates/indexer` — file index snapshot
- `crates/packet_engine` — context packet bundles

### Capabilities

- **Command aliases:** `kosh gs`, `kosh gd` etc.
- **MCP aliases:** `kosh mcp expand "rf @authrepo"` expands to structured tool calls
- **Symbol aliases:** `@authrepo => lib/features/auth/...`
- **Context Cache:** fingerprint-keyed cache in `.kosh/cache.tsv`
- **Context Leasing:** stable handles (`lease:auth:001`) — create/get/touch/list/stats
- **MCP Batching:** `kosh batch '[{"tool":"read_file","path":"..."}]'` — collapses N serial calls, records `mcp_batch` in gain history
- **Context Packets:** `kosh packet create|load|list|delete` — bundles files+symbols into a single loadable handle, records `packet_create`/`packet_load` in gain history
- **Repository Indexing:** `kosh index`, `kosh index diff`
- **Gain Tracking:** `kosh gain`, `kosh gain --by-kind`, `kosh gain --history` — tracks all savings

## What Does Not Exist Yet

- No real MCP server or transport (all current functionality is CLI-only)
- No daemon
- Context Packets do not yet resolve `@symbol` references through the symbol alias table (they store the symbol string as-is)
- Lease fingerprints are not yet linked to actual file byte sizes in the indexer (token savings use a 20k-char heuristic)

## Next Sensible Implementation Steps

1. **Packet → Symbol resolution:** When `kosh packet load` sees an `@symbol`, resolve it through `.kosh/symbols.aliases` before emitting the MCP call
2. **Lease size accuracy:** Link lease fingerprints to indexer data so `lease touch` records actual avoided bytes instead of the 20k heuristic
3. **MCP server stub:** A minimal stdio MCP server that wraps the CLI, enabling agent integration without shell invocation
