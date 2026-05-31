# Context Leasing Economy Report

## Overview
Context Leasing provides Kosh with stable, non-expiring handles to reference previously indexed or provided context. Instead of repeatedly transferring full file contents, an agent can exchange a compact handle (e.g., `lease:auth:001`) to implicitly inject the corresponding files.

This implementation effectively zeroes out the cost of repeating read operations across sequential turns.

## Expected Token Savings

**Baseline (Without Leasing):**
- Initial Context Request: ~20,000 tokens
- Turn 2 Follow-up Request: ~20,000 tokens (Retransmitted Context)
- Turn 3 Follow-up Request: ~20,000 tokens (Retransmitted Context)
- *Total Cost:* 60,000 tokens over 3 turns.

**With Leasing:**
- Initial Context Request: ~20,000 tokens
- Turn 2: Uses `lease:auth:001` (4 tokens)
- Turn 3: Uses `lease:auth:001` (4 tokens)
- *Total Cost:* 20,008 tokens over 3 turns.

**Expected Savings:** ~66% reduction in tokens per standard debugging session, climbing toward 90%+ as session depth increases.

## Expected File Read Savings

**Without Leasing:**
- Every turn requiring a reminder of context triggers an MCP `read_file` or `search` call. If 10 files define the "auth flow", a 5-turn session requires 50 file read operations.

**With Leasing:**
- Files are read exactly once upon lease creation (or indexer run). Subsequent turns use the lease handle, completely eliminating disk I/O and protocol overhead for redundant reads.
- **Expected Savings:** 100% elimination of redundant file reads for unchanged working context.

## Expected MCP Call Savings

**Without Leasing:**
- An agent issues repetitive MCP `read_file` calls for files it has already seen but dropped from context due to system constraints.

**With Leasing:**
- Redundant MCP `read_file` and `search` chatter is eliminated. The context is bundled directly at the Kosh abstraction layer.
- **Expected Savings:** 1-5 MCP calls avoided per conversational turn.

## Next Steps
Now that the lease infrastructure is active, the agent can reference vast project contexts (repos, features, or hashes) by outputting a few characters. The gain tracking system inherently monitors and proves these token savings.

*This report finalizes the documentation for the Leasing feature. Future milestones will focus strictly on `crates/` and `tests/` implementations without generating extraneous documentation.*