# Kosh Master Plan

Kosh (formerly RTK) is a token elimination and context efficiency platform for agentic development environments. It minimizes repeated context transfer by turning files, symbols, graph facts, cache entries, and active working context into reusable handles.

**MISSION: Kosh is a token elimination and context efficiency platform.**

The primary KPI is NOT indexing quality, graph completeness, or code intelligence coverage.
**The primary KPI IS:**
* Tokens avoided
* Tool calls avoided
* File reads avoided
* Context retransmission avoided
* Latency avoided
* Cost avoided

Every feature must answer: *"What measurable token expenditure does this eliminate?"*

## Product Layers

```text
Kosh CLI
        |
        v
Kosh MCP Hub
        |
 +------+---------+
 |      |         |
 v      v         v
Skills Hooks   Plugins

        |
        v
Context Engine
        |
 +------+----------+
 |      |          |
 v      v          v
Alias  Lease      Cache

        |
        v
Context Packets

        |
        v
Symbol Index / Graph (Token-optimized)
```

## First Milestone

Start with:
- Kosh CLI
- Tool Alias Compression
- Symbol References
- Context Caching
- **MCP Batching** (Priority shift)
- **Context Leasing** (Priority shift)

*Constraint: Do not build anything that requires an LLM until Kosh can prove substantial token savings using aliases, caching, leasing, batching, and symbolic references alone.*

## Token Economics Roadmap (Re-anchored)

For every planned feature, we measure success strictly through token economics.

### 1. MCP Batching
* **Estimated tokens saved:** Prevents full conversation history retransmission (e.g., 50k tokens) per avoided turn.
* **Estimated MCP calls saved:** Collapses N serial calls into 1.
* **Estimated file reads avoided:** N/A (makes reads concurrent instead of serial).
* **Estimated latency reduction:** Saves N-1 LLM inference roundtrips (huge latency win).
* **How success is measured:** `rtk gain` tracks "turns avoided" and calculates the associated history token savings.

### 2. Context Leasing (Handles)
* **Estimated tokens saved:** 10k-100k+ tokens per turn by referencing `ctx:auth:14` instead of re-reading a file.
* **Estimated MCP calls saved:** Eliminates redundant `read_file` calls entirely.
* **Estimated file reads avoided:** 100% of redundant reads for active context.
* **Estimated latency reduction:** Bypasses output generation time for large files.
* **How success is measured:** Tracking the byte size of leased contexts vs actual characters sent.

### 3. Context Packets
* **Estimated tokens saved:** Prevents exploratory searches (grep/ls) by providing a pre-defined bundle.
* **Estimated MCP calls saved:** Replaces 5-10 discovery calls with 1 packet load.
* **Estimated file reads avoided:** Eliminates reading wrong/irrelevant files.
* **Estimated latency reduction:** Saves the entire exploratory phase of an agent session.
* **How success is measured:** Reduced sequence of `search -> read -> search` patterns in agent logs.

### 4. Symbol Graph (Deferred)
* **Estimated tokens saved:** Prevents missing context errors by automatically appending precise symbol dependencies.
* **Estimated MCP calls saved:** Eliminates manual dependency tracing tool calls.
* **Estimated file reads avoided:** Only injects the *symbol* (50 lines) instead of the *full file* (1000 lines).
* **Estimated latency reduction:** Faster resolution of compiler errors.
* **How success is measured:** Average file payload size reduced by isolating symbols.

*Note: The Symbol Graph is deferred until features 1-3 prove out the token elimination KPIs.*
