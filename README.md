# Kosh

Kosh is a context virtualization layer for AI agents. It treats repository context as a referable, reusable, and compressible resource, significantly reducing token costs in autonomous engineering workflows.

## Documentation

- **[Technical Whitepaper](docs/whitepaper.md)**: The core thesis on Token Economics and Context Virtualization.
- **[User Guide](docs/USER_GUIDE.md)**: Installation, configuration, and first steps.
- **[Lease Economy Report](docs/LEASE_ECONOMY_REPORT.md)**: Projections on token and read savings.

## Key Features

- `Kosh CLI`: Compact commands for common agent workflows.
- `Context Leasing`: Stable handles (e.g., `lease:auth:001`) to eliminate redundant retransmissions.
- `Context Packets`: Group related files/symbols into loadable bundles.
- `MCP Batching`: Collapse multiple tool calls into one roundtrip.
- `Indexer`: File inventory, language detection, and change tracking.
- `Gain Tracking`: Real-time monitoring of token and cost savings.

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
