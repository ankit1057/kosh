# Current Honest State

Date: 2026-06-01

This repository is a bootstrap implementation of **Kosh** (formerly known as KOSH OS).

## Pivot Acknowledgment
We have recently pivoted away from building a code-intelligence platform (Sourcegraph-lite) and firmly re-anchored on **Token Economics**. Kosh is a token elimination infrastructure. Our primary KPIs are tokens, tool calls, file reads, and latency avoided.

*Constraint:* We will NOT build anything requiring an LLM until Kosh proves substantial token savings using deterministic systems (aliases, caching, leasing, batching, and symbolic references).

## What Exists

- A Rust workspace builds successfully with these crates:
  - `apps/cli` (currently named `kosh-cli`, execution binary is `rtk`)
  - `crates/tool_registry`
  - `crates/mcp_router`
  - `crates/cache_engine`
  - `crates/cost_estimator`
  - `crates/indexer`
- Command aliases are supported.
- MCP aliases are supported.
- Symbol aliases are supported (e.g., `@authrepo => lib/...`).
- **Context Leasing:** Stable handles (e.g., `lease:auth:001`) allow referencing cached context with minimal tokens.
- Gain tracking exists (`.kosh/history.tsv`), which tracks shorthand and lease savings.
- Repository indexing exists (inventories files, detects languages, hashes content).
- Token savings are estimated with a simple 4 characters per token heuristic.

## What Does Not Exist Yet

- **MCP Batching:** No capability to batch multiple tool calls into one execution to save roundtrips.
- **Context Packets:** Bundles of files/symbols behind a single handle.
- No real MCP server or transport implementation exists.
- No daemon exists.

## Next Sensible Implementation Steps (Token-Optimized)

Based on the **Kosh Token Economy Report v1**:

1. **Context Packets:** Group related files/symbols into single loadable packets. This eliminates the "exploratory" turns where an agent lists directories and reads files one by one.
2. **MCP Batching:** Implement the ability to parse and execute a batch of MCP commands to collapse serial turns and save context history retransmission.
3. (Completed): Renamed `rtk` binary, `.rtk` config folder, and environment variables to `kosh` to complete the project identity pivot.
