# Kosh Token Economy Report v1

## Token Flow Analysis: Typical Agentic Session (Claude Code / Codex)

In a typical multi-turn agentic session (e.g., debugging a failing test or implementing a new feature), token consumption scales linearly—and often quadratically—due to context retransmission and repetitive tool use. 

### Top 20 Token Sinks (Categorized)

**System & Schema Overhead**
1. **Tool Schemas in System Prompt**: JSON Schema definitions for every available tool re-transmitted on every turn.
2. **Standard System Instructions**: Lengthy behavioral constraints re-transmitted on every turn.

**Context Retransmission**
3. **Repeated File Reads**: Re-reading the same, unmodified file across multiple turns.
4. **Context Window Bloat**: The entire conversation history (including previous full-file reads) being resent with each new prompt.
5. **Irrelevant Surrounding Code**: Fetching full files when only a specific symbol/function is needed.

**Tool & Protocol Overhead (MCP)**
6. **MCP Request/Response JSON**: Protocol chatter and wrapping overhead for every tool call.
7. **Sequential Tool Chaining**: Making 5 separate tool calls (e.g., 5 `read_file` calls) instead of 1 batched call, causing 5 extra LLM turns (and thus 5x context retransmissions).
8. **File Path Verbosity**: Repeatedly typing/outputting deeply nested absolute or relative paths.

**Exploration & Navigation**
9. **Directory Listings (`ls` / `tree`)**: Repeatedly listing directories to remember file locations.
10. **Broad Grep Searches**: Returning hundreds of lines of irrelevant match context.
11. **Project Structure Discovery**: Blindly searching for configuration or architecture files.

**Action Execution**
12. **Full File Writes for Small Edits**: Using `write_file` for an entire 1000-line file to change 2 lines.
13. **Diff/Patch Verification**: Repeatedly running `git diff` to verify changes, printing the same diffs.
14. **Compilation/Test Output Noise**: Flooding the context with verbose, irrelevant build warnings or un-truncated stack traces.

**Cognitive & Memory Repetition**
15. **Redundant Planning**: Re-summarizing the master plan on every step.
16. **Fact Re-discovery**: Forgetting the port number or DB schema and running commands to find it again.
17. **Routine Workflows**: "Find the auth tests and run them" requires 3-4 turns every time.
18. **Dependency Tracing**: Manually opening imported files one by one.
19. **Unused Imports/Dead Code**: Fetching bloated files full of unused legacy code.
20. **Format Inefficiencies**: Outputting dense JSON/XML instead of compact representations (like TSV or custom DSLs).

---

## Estimated Savings Opportunities & Highest ROI Features

Based on the token sinks, Kosh's immediate roadmap must target mechanisms that prevent retransmission and compress overhead.

| Feature | Targets Sinks | Estimated Savings | Mechanism |
| :--- | :--- | :--- | :--- |
| **Tool Alias Compression** | 6, 8 | 80% character reduction on tool calls | `rf @auth` instead of `{"name": "read_file", "path": "..."}` |
| **Context Leasing** | 3, 4, 15 | 20k+ tokens / lease | Agent outputs `ctx:auth:14`; Kosh injects the file content locally, bypassing LLM generation. |
| **MCP Batching** | 7 | 4x context retransmissions | Collapse 5 serial reads into 1 prompt, avoiding 4 full-history roundtrips. |
| **Symbol References** | 8, 11 | 90% char reduction on paths | Map `@authrepo` to `lib/features/auth/...` |
| **Context Packets** | 9, 10, 18 | 50% exploration overhead | Pre-computed bundles of related files injected at once via lease. |

---

## Recommended Implementation Order

To adhere strictly to the thesis that **Kosh = Token Elimination**, we must prioritize features that reduce tokens *now*, before introducing complex code intelligence.

1. **Tool Alias Compression** (WIP - validates compression)
2. **Symbol References** (WIP - eliminates path verbosity)
3. **MCP Batching** (Next - eliminates serial context roundtrips)
4. **Context Leasing** (Next - the holy grail of avoiding redundant file context)
5. **Context Packets** (Next - bundles context behind a single lease)
6. **Symbol Graph** (Pushed Back - only valuable once packets/leases need automatic discovery)

*Mandate: No LLM-based features (summarization, embeddings) will be implemented until Kosh proves 60-80% token savings using aliases, caching, leasing, batching, and symbolic references alone.*
