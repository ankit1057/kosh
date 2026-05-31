# Kosh - Project Context & Instructions

Kosh (formerly RTK) is a **token elimination and context efficiency platform** for agentic development environments. 

## Core Mission & KPIs
Kosh's primary goal is NOT code intelligence, graph completeness, or search. **Kosh = Context Representation + Context Reuse + Context Elimination.** 

The primary KPIs are strictly economic:
- Tokens avoided
- Tool calls avoided
- File reads avoided
- Context retransmission avoided
- Latency avoided
- Cost avoided

## Strict Constraints
**DO NOT build anything that requires an LLM** until Kosh can prove substantial token savings using aliases, caching, leasing, batching, and symbolic references alone. If Kosh can save 60-80% of tokens before introducing models, the core thesis is validated. Everything else is an accelerator, not a dependency.

## Project Overview

- **Architecture**: A Rust workspace with a central CLI app and modular crates. *(Note: The codebase currently still uses the name `rtk` and `.rtk/` directories. A rename to `kosh` is planned but not fully executed).*
- **Primary Technology**: Rust (2021 edition), emphasizing a minimal dependency footprint and no unsafe code.
- **Token Focus**: Every feature must answer: *"What measurable token expenditure does this eliminate?"*

### Workspace Structure

- `apps/cli`: The main command-line interface.
- `crates/indexer`: File inventory and hashing.
- `crates/mcp_router`: MCP alias expansion and symbol resolution.
- `crates/tool_registry`: Registry for command-line shorthand aliases.
- `crates/cache_engine`: Context fingerprinting.
- `crates/cost_estimator`: Heuristic-based token and cost savings estimation.

## Building and Running

### Prerequisites
- Rust and Cargo (latest stable)

### Key Commands
- **Build**: `cargo build`
- **Run CLI**: `cargo run -p kosh-cli -- <subcommand>` 
- **Test**: `cargo test`
- **Format**: `cargo fmt --all`
- **Check**: `cargo check`

*(Note: The binary and commands currently still use `rtk`.)*
- `kosh gain`: Show token/cost savings analytics.
- `kosh index`: Scan repository and manage file inventory.
- `kosh mcp expand "<alias> <arg>"`: Expand an MCP shorthand.

## Development Conventions

- **Safety**: Unsafe code is strictly forbidden (`#![forbid(unsafe_code)]`).
- **Configuration**: Local configuration is stored in `.rtk/` using simple custom text formats (not JSON/TOML).
- **State Management**: Runtime state is stored in TSV files within `.rtk/`. These files are ignored by version control.
- **Token Estimation**: Uses a conservative heuristic of 4 characters per token.
- **Testing**: Prioritize unit tests within each crate.

## Project Documentation
- **Token Economics**: Refer to `docs/kosh-token-economy-report-v1.md` for the primary architectural drivers.
- **Roadmap**: Refer to `docs/master-plan.md` for the Token Economics Roadmap (prioritizing MCP Batching and Context Leasing).
- **State**: Refer to `STATE.md` for the current "honest state" of the project.

## Credits & Acknowledgments
Kosh is built upon the foundational work and vision of:
- **[RTK (Rust Token Killer)](https://github.com/rtk-ai/rtk)** by the **RTK AI team**.
- **[MadCat](https://gitlab.com/cabalbl4/madcat)** by **cabalbl4**.
