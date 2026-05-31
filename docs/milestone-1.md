# Milestone 1: CLI, Indexer, MCP Aliases, Context Cache

## Scope

This milestone proves context virtualization before adding memory, embeddings, local models, or graph intelligence. Token reduction is a side effect of making context representable and reusable.

## RTK CLI

The CLI expands compact aliases into common shell commands:

```text
gs -> git status --short
gd -> git diff
fpg -> flutter pub get
dart files -> find . -name "*.dart"
```

Unknown commands pass through unchanged.

Custom aliases can be added after running:

```bash
rtk config init
```

The command alias format is:

```text
<alias> => <expansion>
```

## Repository Indexer

The indexer inventories repository files and records:

- path
- language
- byte size
- content hash

```bash
rtk index
rtk index --json
rtk index write
rtk index diff
```

This is the base layer for context packets, symbol graphs, cache invalidation, and future context leases.

## MCP Aliases

The MCP alias layer expands terse calls into structured tool payloads:

```text
rf @authrepo
```

expands to a structured call. If `@authrepo` is registered as a symbol alias, the emitted path is the resolved value:

```json
{"tool":"read_file","path":"lib/features/auth/data/repositories/auth_repository_impl.dart"}
```

Symbol aliases are now supported as the first version of that bridge:

```bash
rtk symbols put @authrepo lib/features/auth/data/repositories/auth_repository_impl.dart
rtk mcp expand "rf @authrepo"
```

The MCP output resolves the symbol before emitting the tool payload.

MCP aliases can be extended in `.rtk/mcp.aliases`:

```text
rf => read_file path
sf => search_files query
```

## Context Cache

Context fingerprints identify reusable context without retransmitting it:

```json
{"repo":"veil","feature":"auth","hash":"xyz"}
```

The cache crate provides stable key generation, compact JSON output, and bootstrap persistence.

The CLI now supports persistent cache records:

```bash
rtk cache put --repo veil --feature auth --hash xyz --summary "Auth flow context"
rtk cache get veil:auth:xyz
rtk cache list
```

Records are stored in `.rtk/cache.tsv`. This is a bootstrap storage format; the design leaves room for a database-backed cache later.

## Telemetry

The first estimator tracks both model-style token buckets in the library and real CLI compression history:

- input tokens
- output tokens
- tool tokens
- memory tokens

For CLI history it reports record count, compact characters, expanded characters, saved characters, estimated saved tokens, and estimated cost saved.

The CLI now records actual shorthand usage:

```bash
rtk expand gs
rtk mcp expand "rf @authrepo"
rtk gain
rtk gain --history
rtk gain --by-kind
rtk gain --by-repo
rtk gain --by-feature
rtk gain --by-context
```

History is stored in `.rtk/history.tsv`, which is ignored as runtime state. New rows include timestamp, repo, feature, event kind, compact form, expanded form, and status; older three- and five-column rows remain readable. Repo is inferred from the current directory, and attribution can be overridden with `RTK_REPO` and `RTK_FEATURE`.

## Explicitly Deferred

Early summarization, embeddings, vector search, and MadCat memory are intentionally deferred. The near-term path is:

```text
Alias
-> Symbol
-> Index
-> Graph
-> Cache
-> Lease
```
