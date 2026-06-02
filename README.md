# Kosh

Kosh is a **Context Operating System** for AI agents. It treats repository context as a referable, reusable, and virtualized resource, eliminating the "Quadratic Token Sink" common in autonomous engineering workflows.

## 🚀 The Core Thesis
**Same Input + Same Repository + Same History = Same Output.** 
Kosh achieves deterministic token elimination (60-90%+ savings) using pure Rust infrastructure before a single LLM call is made.

## 📚 Documentation

- **[Technical Whitepaper](docs/whitepaper.md)**: The core thesis on Token Economics and Context Virtualization.
- **[User Guide](docs/USER_GUIDE.md)**: Installation, configuration, and first steps.
- **[Lease Economy Report](docs/LEASE_ECONOMY_REPORT.md)**: Empirical proof of token savings.
- **[Phase 3 Plan](docs/PHASE_3_PLAN.md)**: Detailed roadmap for the Rust-Native Context OS.
- **[Long Term Vision](docs/LONG_TERM_VISION.md)**: The path to a sub-1000 token engineering turn.

## ✨ Key Features

- **Context Leasing**: Stable handles (e.g., `lease:auth:001`) that replace thousands of tokens with a single 4-token reference.
- **Context Signatures (DNA)**: Deterministic code-level signatures (classes, methods, functions) used to recognize and reuse context.
- **MCP Batching**: Collapses multiple tool calls into a single roundtrip, saving history retransmission.
- **Context Packets**: Group related files and symbols into loadable, high-ROI bundles.
- **Gain Tracking**: Real-time SQLite-backed monitoring of tokens, reads, and cost avoided.

## 🛠️ Architecture: The PRECC Stack
Kosh organizes context into a predictable hierarchy:
**Alias → Symbol → Index → Graph → Cache → Lease**

- **RTK (Rust Token Killer)**: The underlying engine handling command and protocol-level compression.
- **Kosh**: The high-level platform for context virtualization and deterministic planning.

## 🚀 Quick Start

### The One-Liner
```bash
curl -sSL https://raw.githubusercontent.com/ankit1057/kosh/main/scripts/install.sh | bash
```

### Manual
```bash
cargo test
cargo run -p kosh-cli -- config init
cargo run -p kosh-cli -- lease create --repo demo --feature auth --file src/auth.rs
cargo run -p kosh-cli -- context extract lease:auth:001
cargo run -p kosh-cli -- gain --history
```

## 📈 Roadmap (Current: Phase 3.6)
1. **Context Awareness** (V2 Fingerprints + SQLite) - ✅ **Done**
2. **Context Resolver** (Scoring & Explainability) - ✅ **Done**
3. **Context Signatures** (DNA Extraction - Dart/Rust) - ✅ **Done**
4. **Signature Overlap Scoring** - 🏗️ **In Progress**
5. **Context Composition & Planning** - 📅 **Next**

## 🤝 Credits & Acknowledgments
Kosh is built upon the foundational work of:
- **[RTK (Rust Token Killer)](https://github.com/rtk-ai/rtk)** by the **RTK AI team**.
- **[MadCat](https://gitlab.com/cabalbl4/madcat)** by **cabalbl4**.
