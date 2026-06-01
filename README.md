# Kosh

**Context virtualization for AI agents. Measure and eliminate token waste.**

Kosh treats repository context as a referable, reusable, and compressible resource.
Instead of retransmitting the same files across every agent turn, Kosh lets agents
reference context by handle — and tracks exactly how many tokens were avoided.

---

## Benchmark Results

Independent payload benchmarks measuring Kosh primitives against naive alternatives.
No LLM involved — raw bytes in, raw bytes out.

```
Suite                        Naive tok   Kosh tok     Saved      %   Calls↓
─────────────────────────────────────────────────────────────────────────────
Leasing (10-turn session)      20,480      2,221   +18,258   89.2%      +9
Packet loading (7 files)        7,398        117    +7,280   98.4%      +7
Batching (5 serial reads)       1,321         82    +1,239   93.8%      +4
Symbol compression (×10 refs)     157        105       +52   33.3%      +0
─────────────────────────────────────────────────────────────────────────────
TOTAL                          29,357      2,526   +26,831   91.4%     +20
```

> Kosh primitives reduced benchmarked context-transfer payloads by up to 98.4%,
> with an aggregate reduction of 91.4% across the benchmark suite.

Run the benchmarks yourself:

```bash
cargo build
python agent_test/kosh_benchmarks.py
```

---

## How It Works

### Leasing — read once, reference many times

```bash
kosh lease create --repo myapp --feature auth --fingerprint abc123 --summary "Auth module"
# => {"id":"lease:auth:001", ...}

# Every subsequent turn: reference the lease, not the file
kosh lease touch lease:auth:001
```

A 10-turn session reading the same 8 KB of context costs **20,480 tokens** naively.
With leasing: **2,221 tokens**. The file is read once; subsequent turns send a 30-char handle.

### Packets — one call instead of N reads

```bash
kosh packet create --name auth \
  --file lib/auth/repo.dart \
  --file lib/auth/models.dart \
  --symbol @authrepo

kosh packet load auth
# => {"tool":"read_file","path":"lib/auth/repo.dart"}
#    {"tool":"read_file","path":"lib/auth/models.dart"}
#    {"tool":"read_file","path":"..."}  (resolved from @authrepo)
```

Seven individual reads (ls + 7 read_file calls = 7,398 tokens) collapse to one
packet load call (117 tokens). **98.4% reduction.**

### Batching — collapse serial tool calls

```bash
kosh batch '[
  {"tool":"read_file","path":"Cargo.toml"},
  {"tool":"read_file","path":"apps/cli/Cargo.toml"},
  {"tool":"read_file","path":"crates/packet_engine/Cargo.toml"}
]'
```

Five serial reads accumulate conversation-history overhead with each round trip.
One batch call eliminates 4 of 5 round trips. **93.8% reduction.**

### Gain Tracking — see exactly what was saved

```bash
kosh gain --by-kind
# lease_hit     54,945 estimated tokens saved   $0.55
# mcp_batch     22,147 estimated tokens saved   $0.22
# packet_load      245 estimated tokens saved   $0.002
```

Every Kosh operation records its savings in `.kosh/history.tsv`.
Gain tracking is the most important part — it makes token elimination measurable
and reproducible across sessions and releases.

---

## Quick Start

```bash
git clone https://github.com/ankit1057/agent-kosh
cd agent-kosh
cargo build
./target/debug/kosh --help
```

### Initialize a project

```bash
kosh config init
# creates .kosh/commands.aliases, mcp.aliases, symbols.aliases
```

### Symbol aliases

```bash
kosh symbols put @authrepo lib/features/auth/data/repositories/auth_repository_impl.dart
kosh mcp expand "rf @authrepo"
# => {"tool":"read_file","path":"lib/features/auth/data/repositories/auth_repository_impl.dart"}
```

### Context cache

```bash
kosh cache put --repo myapp --feature auth --hash abc123 --summary "Auth flow context"
kosh cache get myapp:auth:abc123
```

### Indexer

```bash
kosh index          # scan working directory
kosh index write    # persist to .kosh/index.tsv
kosh index diff     # compare to last saved snapshot
```

### Gain report

```bash
kosh gain
kosh gain --by-kind
kosh gain --by-repo
kosh gain --history
```

---

## Workspace

```
apps/cli                Binary: kosh
crates/tool_registry    Command alias expansion
crates/mcp_router       MCP alias parser and symbol resolution
crates/cache_engine     Context fingerprinting and leasing
crates/packet_engine    Context packet bundles
crates/cost_estimator   Token savings tracking and reporting
crates/indexer          File inventory and change detection
agent_test/             Benchmark harnesses
docs/                   Architecture notes and economy reports
```

Config dir: `.kosh/` (falls back to `.rtk/` for existing data).
Env vars: `KOSH_REPO`, `KOSH_FEATURE` (fall back to `RTK_REPO`, `RTK_FEATURE`).

---

## Contributing

Kosh is early-stage infrastructure. The primitives work; the gaps are known.

**High-value contributions:**

- **Real session replay benchmark** — extract `read_file`/`grep`/`ls` calls from a Claude Code
  or Codex transcript and replay with/without Kosh. This is the missing "real world" benchmark.
- **`kosh benchmark` subcommand** — run the four primitive suites as a single CLI command,
  output a machine-readable report so every release can track regression/improvement.
- **Packet symbol resolution** — `kosh packet load` should resolve `@symbol` refs through
  `.kosh/symbols.aliases` before emitting MCP calls (currently stores the symbol as-is).
- **Lease size accuracy** — link lease fingerprints to indexer data so `lease touch` records
  actual avoided bytes instead of a fixed heuristic.
- **MCP server stub** — minimal stdio JSON-RPC server wrapping the CLI for native agent integration.
- **Automatic packet generation** — use the indexer or import tracing to suggest packets
  based on co-accessed files.

**Design constraints (please read before opening a PR):**

- No LLMs, no embeddings, no graph intelligence until token savings are proven by measurement
- All storage in TSV format — zero external dependencies, human-readable, diffable
- Every new feature must answer: *what measurable token expenditure does this eliminate?*
- One document equals many code changes, not the reverse

**Run the tests:**

```bash
cargo test       # 46 unit tests across all crates
cargo check      # zero warnings policy
cargo fmt --all  # enforced formatting
```

---

## Roadmap

```
v0.1  ✅  Aliases, symbols, cache, gain tracking, indexer
v0.2  ✅  Context leasing, MCP batching, context packets, kosh rename
v0.3  🔲  kosh benchmark CLI, packet symbol resolution, lease size accuracy
v0.4  🔲  MCP server, real session replay benchmark
v0.5  🔲  Automatic packet generation, session telemetry export
```

---

## Credits

- **[RTK (Rust Token Killer)](https://github.com/rtk-ai/rtk)** — core token elimination mission
  and proxy architecture
- **[MadCat](https://gitlab.com/cabalbl4/madcat)** — long-term memory and research fact layer inspiration

---

<!-- hashtags for discoverability -->
<!--
#ai #llm #agents #tokenoptimization #claudecode #mcp #rust #contextwindow
#aiengineering #llmops #agentic #tokensaving #rustlang #openhermes #mlx
#aiinfrastructure #contextcompression #aitools #codingagents #aiproductivity
-->
