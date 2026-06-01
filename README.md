# Kosh

**Context virtualization for AI agents. Measure and eliminate token waste.**

Kosh treats repository context as a referable, reusable, and compressible resource.
Instead of retransmitting the same files across every agent turn, Kosh lets agents
reference context by handle — and tracks exactly how many tokens were avoided.

> **Kosh vs prompt caching:** Anthropic/OpenAI prompt caching reduces the *cost* of
> retransmitting a cached prefix (~90% discount on cache reads). Kosh reduces the *need*
> to transmit that prefix in the first place — and eliminates the round trips that caching
> never touches: repeated `read_file`, `grep`, and `find` calls that accumulate as growing,
> un-cacheable conversation history. Use both together.

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

**What these numbers measure:** `chars // 4` token estimation, no API call, no model in the loop.
These are *payload-reduction* benchmarks that isolate Kosh's primitives from model behavior.
The next benchmark (v0.3) will replay a real agent session against the actual API with prompt
caching enabled in both arms and report billed tokens, latency, and task-success side by side.

Run the benchmarks yourself:

```bash
cargo build
python agent_test/kosh_benchmarks.py
```

---

## Why Tool Outputs, Not Prompt Compression

SWE-bench measurements show that **30,400 of 48,400 total agent tokens come from tool results**,
and 39.9–59.7% of those are removable with no performance loss. Prompt compression targets the
wrong layer.

The second finding that shapes Kosh's architecture: in a naive 10-step agent loop, token cost
follows a triangular number series — each step re-bills all prior context. A 10-step loop costs
**43.3× more than a single call**. Prompt caching reduces the cost per prefix read (~90%
discount) but doesn't touch this growth. Kosh's leasing and batching attack the accumulation
itself — eliminating turns, not discounting them.

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

### Pipeline — what the model actually receives

```
Agent turn N:
  "investigate login bug"
        │
        ▼
  kosh packet load auth          ← 1 call, 117 chars
        │
        ▼
  [lib/auth/repo.dart             ← materialized locally,
   lib/auth/models.dart            not re-transmitted
   lib/auth/login_usecase.dart]    from model context
        │
        ▼
  Model receives: minimal context only
  Tokens avoided: file content that would have been
                  re-read and re-appended to history
```

The key question is not "can I replace 500 tokens with 5?"
It is "can I avoid materializing 49,500 of the original 50,000 tokens entirely?"

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

## Install

```bash
curl -sSf https://raw.githubusercontent.com/ankit1057/kosh/main/scripts/install.sh | bash
```

Requires Rust. Installs the `kosh` binary via `cargo install` and runs `kosh config init`.

Or build from source:

```bash
git clone https://github.com/ankit1057/kosh
cd kosh
cargo build --release
./target/release/kosh --help
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
apps/cli                    Binary: kosh
crates/tool_registry        Command alias expansion
crates/mcp_router           MCP alias parser and symbol resolution
crates/cache_engine         Context fingerprinting and leasing
crates/packet_engine        Context packet bundles
crates/context_signatures   Jaccard overlap scoring, context composition
crates/cost_estimator       Token savings tracking and reporting
crates/indexer              File inventory and change detection
agent_test/                 Benchmark harnesses (primitive + session replay)
docs/                       Architecture notes and economy reports
```

Config dir: `.kosh/` (falls back to `.rtk/` for existing data).
Env vars: `KOSH_REPO`, `KOSH_FEATURE` (fall back to `RTK_REPO`, `RTK_FEATURE`).

---

## Contributing

Kosh is early-stage infrastructure. The primitives work; the gaps are known.

**High-value contributions:**

- **Real session replay benchmark** — extract `read_file`/`grep`/`ls` calls from a Claude Code
  or Codex transcript and replay with/without Kosh against the actual API, prompt caching on in
  both arms, reporting billed tokens + latency + task-success. This is the missing proof point.
- **Quality-preservation evaluation** — pair every savings number with a task-success rate.
  A system that saves 90% of tokens but degrades outcomes by 20% is not a win. We need a
  benchmark that reports both together.
- **`kosh benchmark` subcommand** — run the four primitive suites as a single CLI command,
  output a machine-readable report so every release can track regression/improvement.
- **Packet symbol resolution** — `kosh packet load` should resolve `@symbol` refs through
  `.kosh/symbols.aliases` before emitting MCP calls (currently stores the symbol as-is).
- **Lease size accuracy** — link lease fingerprints to indexer data so `lease touch` records
  actual avoided bytes instead of a fixed heuristic.
- **MCP server stub** — minimal stdio JSON-RPC server wrapping the CLI for native agent integration.
- **Automatic packet generation** — use the indexer or import tracing to suggest packets
  based on co-accessed files.

**Pressure-testing contributions are equally valuable:**

A contributor who can demonstrate *where Kosh hurts task quality*, *where prompt caching already
solves the problem sufficiently*, or *where deterministic planning breaks down on large repos*
is as valuable as someone adding features. The project is early enough that strong criticism can
change the architecture.

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
v0.4  🔲  Context signatures + composition (Jaccard overlap scoring, kosh signature match)
v0.5  🔲  Real session replay benchmark — actual API, prompt caching on, savings + task-success paired
v0.6  🔲  MCP proxy — transparent stdio interception (proven pattern: mcpwall, mcp-audit)
v0.7  🔲  Deterministic context planner — task description → minimum required files, no repo exploration
v0.8  🔲  Fact engine — architecture decisions, known bugs, TSV + SQLite FTS5, confidence scores
v0.9  🔲  Kosh OS — context fingerprint → lease match → fact match → answer, 0 redundant reads
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
