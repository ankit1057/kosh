# Milestone 2 Implementation Plan
# Kosh — MCP Batching, Context Packets, Identity Rename

Date: 2026-06-01
Status: PENDING

## Context

Context Leasing is complete and verified. The binary is `rtk`, config dir is `.rtk/`, the project
identity is Kosh. Milestone 2 completes the First Milestone scope from the master plan.

## Constraints

- No LLMs, no embeddings, no graph intelligence
- Every feature measured by tokens / file reads / MCP calls avoided
- Rust workspace at `/Users/mobile/Desktop/Dev/AI_ML/agent-kosh`
- All storage in `.rtk/*.tsv` format (same pattern as leases, cache, history)
- Tests with `cargo test`; check with `cargo check`; format with `cargo fmt --all`

## Existing crates

- `apps/cli` — binary `rtk`
- `crates/tool_registry` — command alias expansion
- `crates/mcp_router` — MCP alias expansion and symbol resolution
- `crates/cache_engine` — context cache + context leasing (LeaseRecord, ContextLeaseManager)
- `crates/cost_estimator` — compression history / gain tracking
- `crates/indexer` — file index snapshot

## Tasks

---

### Task 1: MCP Batching

**Goal:** Collapse N serial MCP calls into a single batch execution, recording the savings.

**Design:**
- New CLI subcommand: `rtk batch <json-array-of-mcp-calls>`
- Also support: `rtk batch --file <path>` (reads JSON array from file)
- Input format: `[{"tool":"read_file","path":"..."}, {"tool":"search_files","query":"..."}]`
- Output format: one JSON object per line `{"tool":"...","result":"...","status":"ok|err"}`
- After execution, call `maybe_record_compression("mcp_batch", compact, expanded, status)`
  where compact = the short batch spec string, expanded = all expanded MCP payloads joined
- No new crate needed; implement entirely in `apps/cli/src/main.rs`

**Success criteria:**
- `rtk batch '[{"tool":"read_file","path":"Cargo.toml"},{"tool":"list_directory","path":"."}]'`
  executes each call in sequence and prints results
- `rtk gain` shows mcp_batch records with saved_chars > 0
- Unit tests for batch JSON parsing and result formatting
- `cargo test` green, `cargo check` clean

**Files to touch:**
- `apps/cli/src/main.rs` — add `handle_batch`, wire into `run()` match, add `BATCH_KIND`

---

### Task 2: Context Packets

**Goal:** Bundle related files and symbols into a single loadable handle to eliminate discovery calls.

**Design:**
- Storage: `.rtk/packets.tsv` — TSV with fields: `name\tfiles\tsymbols\tcreated_at`
  - `files` is a pipe-delimited list of file paths (escaped as per existing TSV conventions)
  - `symbols` is a pipe-delimited list of `@symbol` names
- New CLI subcommands:
  - `rtk packet create --name <name> [--file <path>]... [--symbol <@sym>]...`
  - `rtk packet load <name>` — emits all files and symbols as compact MCP calls (JSON lines)
  - `rtk packet list` — lists all packets with file/symbol counts
  - `rtk packet delete <name>` — removes packet from store
- New crate: `crates/packet_engine` with `PacketRecord` and `PacketStore` (same pattern as
  `cache_engine`)
- `apps/cli/src/main.rs` adds `handle_packet`, imports `packet_engine`
- `Cargo.toml` workspace adds `crates/packet_engine`
- `apps/cli/Cargo.toml` adds `packet_engine` dependency

**Success criteria:**
- `rtk packet create --name auth --file lib/auth/repo.dart --symbol @authrepo`
- `rtk packet load auth` emits MCP read_file calls for each file and symbol
- `rtk packet list` shows packet name, file count, symbol count
- Gain tracking: `packet_load` kind recorded in history
- Unit tests for PacketRecord serialization, PacketStore load/save, round-trip TSV
- `cargo test` green, `cargo check` clean

**Files to touch:**
- `crates/packet_engine/src/lib.rs` — new crate
- `crates/packet_engine/Cargo.toml` — new crate manifest
- `Cargo.toml` — add workspace member
- `apps/cli/Cargo.toml` — add dependency
- `apps/cli/src/main.rs` — add `handle_packet`, wire into `run()` match

---

### Task 3: Identity Rename (rtk → kosh)

**Goal:** Rename the binary and config dir from `rtk` to `kosh` while maintaining backward
compatibility for existing `.rtk/` data.

**Design:**
- Rename binary: in `apps/cli/Cargo.toml`, change `[[bin]] name = "rtk"` to `name = "kosh"`
- Config dir: support both `.kosh/` (new default) and `.rtk/` (fallback for existing data)
  - Logic: if `.kosh/` exists use it; else if `.rtk/` exists use it; else default to `.kosh/`
  - Implement as `fn config_dir() -> &'static str` or a runtime function
- Environment variables: support both `KOSH_REPO`/`KOSH_FEATURE` (preferred) and
  `RTK_REPO`/`RTK_FEATURE` (fallback)
- Update all constant strings in `apps/cli/src/main.rs`:
  - `RTK_DIR` → use `config_dir()` result
  - All file path constants derive from `config_dir()`
- Update `print_help()` to say `kosh` instead of `rtk`
- Do NOT rename the Cargo package name or crate names — only the binary name and config paths

**Success criteria:**
- `cargo build` produces binary named `kosh` in `target/debug/kosh`
- `kosh lease list`, `kosh gain`, etc. all work
- If `.rtk/` exists and `.kosh/` does not, data is read from `.rtk/`
- If `.kosh/` exists, data is read from `.kosh/`
- `cargo test` green, `cargo check` clean
- Help text shows `kosh` not `rtk`

**Files to touch:**
- `apps/cli/Cargo.toml` — rename binary
- `apps/cli/src/main.rs` — update constants and help text, add config_dir() logic

---

## Execution Order

1 → 2 → 3 (sequential; each task is self-contained and can be reviewed independently)

## Definition of Done

- All three tasks complete with green tests
- `kosh gain` shows mcp_batch and packet_load records in history
- `docs/STATE.md` updated to reflect new capabilities
- All changes committed
