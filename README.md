# KOSH OS

KOSH OS is a context virtualization layer for AI agents. The first milestone focuses on making repository context representable, reusable, and referable through compact handles.

This repository is intentionally starting with the useful core:

- `KOSH CLI`: compact commands for common agent workflows.
- `Indexer`: file inventory, language detection, hashing, and change detection.
- `MCP Compression`: terse aliases that expand into structured MCP tool calls.
- `Context Cache`: stable fingerprints for repository and task context.
- `Cost Engine`: lightweight estimates for token and cost savings.

Later phases add symbol graphs, context leases, MCP batching, executable skills, a research fact layer, MadCat memory, and universal agent adapters.

## Quick Start

```bash
cargo test
cargo run -p kosh-cli -- git status
cargo run -p kosh-cli -- expand gs
cargo run -p kosh-cli -- config init
cargo run -p kosh-cli -- symbols put @authrepo lib/features/auth/data/repositories/auth_repository_impl.dart
cargo run -p kosh-cli -- mcp expand "rf @authrepo"
cargo run -p kosh-cli -- cache fingerprint --repo veil --feature auth --hash xyz
cargo run -p kosh-cli -- cache put --repo veil --feature auth --hash xyz --summary "Auth flow context"
cargo run -p kosh-cli -- cache get veil:auth:xyz
cargo run -p kosh-cli -- index
cargo run -p kosh-cli -- index write
cargo run -p kosh-cli -- index diff
cargo run -p kosh-cli -- gain
cargo run -p kosh-cli -- gain --json
cargo run -p kosh-cli -- gain --history
cargo run -p kosh-cli -- gain --history-json
cargo run -p kosh-cli -- gain --by-kind
cargo run -p kosh-cli -- gain --by-context
```

## Workspace

```text
apps/cli              KOSH command-line interface
crates/tool_registry  Command alias registry
crates/mcp_router     MCP alias parser and expander
crates/cache_engine   Context fingerprint primitives
crates/cost_estimator Token and cost savings estimates
crates/indexer        File inventory and change detection
docs/                 Product and architecture notes
```

## Local Config

`kosh config init` creates:

- `.kosh/commands.aliases`
- `.kosh/mcp.aliases`
- `.kosh/symbols.aliases`

Alias file format:

```text
gs => git status --short
rf => read_file path
@authrepo => lib/features/auth/data/repositories/auth_repository_impl.dart
```

The context cache is stored at `.kosh/cache.tsv`.

## Indexing

The indexer is the base for context virtualization.

```bash
kosh index
kosh index --json
kosh index write
kosh index diff
```

It records file path, language, byte size, and content hash. The saved index lives at `.kosh/index.tsv`.

## Gain Tracking

KOSH records shorthand expansions that save characters into `.kosh/history.tsv`. Each new row includes timestamp, repo, feature, event kind, compact form, expanded form, and status.

```bash
kosh gain
kosh gain --json
kosh gain --history
kosh gain --history-json
kosh gain --by-kind
kosh gain --by-repo
kosh gain --by-feature
kosh gain --by-context
```

The token estimate currently uses a simple 4 characters per token heuristic. This is deliberately conservative and dependency-free for the bootstrap implementation.

Repo names are inferred from the current directory. Override attribution with `KOSH_REPO` and `KOSH_FEATURE`.

## Credits & Acknowledgments

Kosh is built upon the foundational work and vision of several pioneer projects in the AI agent space:

- **[RTK (Rust Token Killer)](https://github.com/rtk-ai/rtk)**: Kosh inherits its core token elimination mission and "Proxy" architecture from the original RTK project by the **RTK AI team**.
- **[MadCat](https://gitlab.com/cabalbl4/madcat)**: Kosh's long-term memory vision and future research fact layer are inspired by the MadCat project by **cabalbl4**.
