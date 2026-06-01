#!/usr/bin/env python3
"""
Kosh Primitive Benchmarks — measures Kosh primitives directly.

No LLM. No agent reasoning. No model behavior noise.

Each suite answers one question:
  "What does Kosh eliminate compared to the naive payload?"

Metrics: chars, estimated tokens (chars/4), tool calls, round trips.
"""

import json
import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent
KOSH_BIN  = REPO_ROOT / "target" / "debug" / "kosh"
CHARS_PER_TOKEN = 4

os.environ.setdefault("TOKENIZERS_PARALLELISM", "false")
os.environ["KOSH_REPO"]    = "agent-kosh"
os.environ["KOSH_FEATURE"] = "benchmark"


# ── Helpers ───────────────────────────────────────────────────────────────────

def kosh(*args) -> str:
    r = subprocess.run([str(KOSH_BIN), *args], capture_output=True, text=True, cwd=str(REPO_ROOT))
    return (r.stdout + r.stderr).strip()


def tokens(chars: int) -> int:
    return chars // CHARS_PER_TOKEN


def read_file_bytes(path: str) -> int:
    p = REPO_ROOT / path
    return p.stat().st_size if p.exists() else 0


def read_file_content(path: str) -> str:
    p = REPO_ROOT / path
    return p.read_text(errors="replace") if p.exists() else ""


def mcp_read_payload(path: str) -> str:
    """Bytes an agent would send for a single read_file call + its full response."""
    call  = json.dumps({"tool": "read_file", "path": path})
    response = read_file_content(path)[:4096]
    return call + response


@dataclass
class Suite:
    name: str
    naive_chars:  int
    kosh_chars:   int
    naive_calls:  int
    kosh_calls:   int
    note: str = ""

    @property
    def saved_chars(self):  return self.naive_chars - self.kosh_chars
    @property
    def saved_pct(self):    return (self.saved_chars / self.naive_chars * 100) if self.naive_chars else 0
    @property
    def saved_tokens(self): return tokens(self.saved_chars)
    @property
    def saved_calls(self):  return self.naive_calls - self.kosh_calls


# ── Suite 1: Compression (symbol aliases) ─────────────────────────────────────

def bench_compression() -> Suite:
    """
    Measures chars saved when an agent references a long path via a @symbol
    vs transmitting the full path every turn.

    Naive:  agent sends the full path in every read_file call.
    Kosh:   agent sends @symbol; kosh mcp expand resolves it to the path.

    We measure the MCP payload size for 10 repeated references (a realistic
    conversation where the agent reads the same file across several turns).
    """
    symbol  = "@authrepo"
    value   = "crates/cache_engine/src/lib.rs"
    repeats = 10

    # Register the symbol
    kosh("symbols", "put", symbol, value)

    # Naive: full path in every call
    naive_call   = json.dumps({"tool": "read_file", "path": value})
    naive_chars  = len(naive_call) * repeats

    # Kosh: symbol reference resolved once via mcp expand
    kosh_call    = json.dumps({"tool": "read_file", "path": symbol})
    expand_out   = kosh("mcp", "expand", f"rf {symbol}")
    # The expand output is what the agent sends; the path lookup is local, no retransmit
    kosh_chars   = len(kosh_call) * repeats  # agent sends the short form each time

    return Suite(
        name="compression-symbol",
        naive_chars=naive_chars,
        kosh_chars=kosh_chars,
        naive_calls=repeats,
        kosh_calls=repeats,
        note=f"@{symbol}='{value}' × {repeats} refs | expand: {expand_out[:60]}",
    )


# ── Suite 2: Leasing ──────────────────────────────────────────────────────────

def bench_leasing() -> Suite:
    """
    Measures bytes avoided when an agent re-references a large file context
    via a lease instead of re-reading the file each turn.

    Naive:  10 turns, each sending the full file content as tool result.
    Kosh:   1 read (turn 1) + 9 lease touches (turns 2-10).
            Lease touch payload = the terse lease ID, not the file content.

    Files chosen: the two largest source files in the project.
    """
    files = [
        "apps/cli/src/main.rs",
        "crates/cache_engine/src/lib.rs",
    ]
    turns = 10

    # Create leases for each file
    kosh("lease", "create", "--repo", "agent-kosh", "--feature", "bench-lease",
         "--fingerprint", "bench-001",
         "--summary", f"Context for {files[0]} and {files[1]}")

    file_contents = [read_file_content(f)[:4096] for f in files]
    total_file_chars = sum(len(c) for c in file_contents)

    # Naive: agent retransmits full file content every turn
    naive_chars = total_file_chars * turns
    naive_calls = turns * len(files)

    # Kosh: 1 full read on turn 1 + lease touch (tiny payload) for turns 2-10
    lease_id    = kosh("lease", "list").split('"id":"')[1].split('"')[0] if '"id":"' in kosh("lease", "list") else "lease:bench-lease:001"
    touch_out   = kosh("lease", "touch", lease_id)
    touch_chars = len(f"kosh lease touch {lease_id}") + len(touch_out)

    kosh_chars  = total_file_chars + touch_chars * (turns - 1)
    kosh_calls  = len(files) + (turns - 1)  # reads + touches

    return Suite(
        name="leasing-repeated-reads",
        naive_chars=naive_chars,
        kosh_chars=kosh_chars,
        naive_calls=naive_calls,
        kosh_calls=kosh_calls,
        note=f"files={[f.split('/')[-1] for f in files]} total_file_chars={total_file_chars} turns={turns}",
    )


# ── Suite 3: Packet Loading ───────────────────────────────────────────────────

def bench_packet_loading() -> Suite:
    """
    Measures tool calls avoided when an agent loads a packet (one call)
    vs discovering and reading files individually.

    Naive:  agent runs ls to find files (1 call) + reads each file (N calls).
    Kosh:   agent calls kosh packet load (1 call) → gets all file refs at once.

    We use the 'arch' packet which bundles all 7 crate lib.rs files.
    """
    packet_name = "arch"
    files = [
        "crates/cache_engine/src/lib.rs",
        "crates/cache_engine/src/lease.rs",
        "crates/packet_engine/src/lib.rs",
        "crates/cost_estimator/src/lib.rs",
        "crates/tool_registry/src/lib.rs",
        "crates/mcp_router/src/lib.rs",
        "crates/indexer/src/lib.rs",
    ]

    # Ensure packet exists
    args = ["packet", "create", "--name", packet_name]
    for f in files:
        args += ["--file", f]
    kosh(*args)

    # Naive: 1 list_directory call + N read_file calls
    ls_payload     = json.dumps({"tool": "list_directory", "path": "crates"})
    ls_response    = "\n".join(str(REPO_ROOT / "crates" / d) for d in os.listdir(REPO_ROOT / "crates"))
    naive_discovery = len(ls_payload) + len(ls_response)
    naive_reads    = sum(len(mcp_read_payload(f)) for f in files)
    naive_chars    = naive_discovery + naive_reads
    naive_calls    = 1 + len(files)  # ls + N reads

    # Kosh: 1 packet load call → returns all file refs as MCP calls
    packet_out   = kosh("packet", "load", packet_name)
    kosh_payload = json.dumps({"tool": "kosh_packet_load", "name": packet_name})
    kosh_chars   = len(kosh_payload) + len(packet_out)
    kosh_calls   = 1

    return Suite(
        name="packet-loading",
        naive_chars=naive_chars,
        kosh_chars=kosh_chars,
        naive_calls=naive_calls,
        kosh_calls=kosh_calls,
        note=f"packet '{packet_name}' bundles {len(files)} files | load output: {len(packet_out)} chars",
    )


# ── Suite 4: Batching ─────────────────────────────────────────────────────────

def bench_batching() -> Suite:
    """
    Measures round trips avoided when N file reads are collapsed into one batch.

    Naive:  N separate read_file calls, each a separate MCP round trip.
    Kosh:   1 kosh batch call with the same N files.

    Measures the request payload (what the agent sends) — the response content
    is identical in both cases, so we only count the request overhead.
    Each serial call also carries the growing conversation history prefix,
    which we model as +500 chars per additional turn.
    """
    files = [
        "Cargo.toml",
        "apps/cli/Cargo.toml",
        "crates/packet_engine/Cargo.toml",
        "crates/cache_engine/Cargo.toml",
        "crates/cost_estimator/Cargo.toml",
    ]
    history_overhead_per_turn = 500  # chars of accumulated conversation history per extra turn

    # Naive: N serial calls, each in its own round trip
    # Each call re-sends the conversation context prefix (growing each turn)
    naive_request_chars = 0
    for i, f in enumerate(files):
        call_payload       = json.dumps({"tool": "read_file", "path": f})
        history_prefix     = history_overhead_per_turn * i
        naive_request_chars += len(call_payload) + history_prefix
    naive_calls = len(files)

    # Kosh: 1 batch call, 1 round trip
    batch_calls = [{"tool": "read_file", "path": f} for f in files]
    batch_cmd   = json.dumps({"tool": "kosh_batch", "calls": batch_calls})
    batch_out   = kosh("batch", json.dumps(batch_calls))
    kosh_request_chars = len(batch_cmd)
    kosh_calls  = 1

    return Suite(
        name="batching-serial-vs-batch",
        naive_chars=naive_request_chars,
        kosh_chars=kosh_request_chars,
        naive_calls=naive_calls,
        kosh_calls=kosh_calls,
        note=f"{len(files)} files | history_overhead={history_overhead_per_turn}chars/turn | batch_out={len(batch_out)} chars",
    )


# ── Print ─────────────────────────────────────────────────────────────────────

def print_suite(s: Suite):
    bar_w = 40
    naive_bar = int(bar_w * min(s.naive_chars, 60000) / 60000)
    kosh_bar  = int(bar_w * min(s.kosh_chars,  60000) / 60000)
    saved_sign = "+" if s.saved_chars >= 0 else ""

    print(f"\n{'─'*70}")
    print(f"  {s.name}")
    print(f"{'─'*70}")
    print(f"  {s.note}")
    print()
    print(f"  {'NAIVE':8}  {'█'*naive_bar}  {s.naive_chars:>8} chars  {tokens(s.naive_chars):>6} tok  {s.naive_calls:>3} calls")
    print(f"  {'KOSH':8}  {'█'*kosh_bar}  {s.kosh_chars:>8} chars  {tokens(s.kosh_chars):>6} tok  {s.kosh_calls:>3} calls")
    print()
    print(f"  SAVED    {saved_sign}{s.saved_chars:,} chars  {saved_sign}{s.saved_tokens:,} tokens  {s.saved_calls:+} calls  ({s.saved_pct:.1f}%)")


def print_summary(suites: list[Suite]):
    print(f"\n{'='*70}")
    print("  SUMMARY")
    print(f"{'='*70}")
    print(f"  {'Suite':<32} {'Naive tok':>10} {'Kosh tok':>9} {'Saved':>8} {'%':>7} {'Calls↓':>7}")
    print(f"  {'-'*68}")

    total_naive = total_kosh = total_calls = 0
    for s in suites:
        sign = "+" if s.saved_tokens >= 0 else ""
        print(f"  {s.name:<32} {tokens(s.naive_chars):>10,} {tokens(s.kosh_chars):>9,} "
              f"{sign}{s.saved_tokens:>7,} {s.saved_pct:>6.1f}% {s.saved_calls:>+7}")
        total_naive += s.naive_chars
        total_kosh  += s.kosh_chars
        total_calls += s.saved_calls

    total_saved = total_naive - total_kosh
    total_pct   = (total_saved / total_naive * 100) if total_naive else 0
    sign = "+" if total_saved >= 0 else ""
    print(f"  {'-'*68}")
    print(f"  {'TOTAL':<32} {tokens(total_naive):>10,} {tokens(total_kosh):>9,} "
          f"{sign}{tokens(total_saved):>7,} {total_pct:>6.1f}% {total_calls:>+7}")


def print_gain():
    out = kosh("gain", "--by-kind")
    if not out.strip():
        return
    print(f"\n{'='*70}")
    print("  KOSH GAIN TRACKING (cumulative session)")
    print(f"{'='*70}")
    for line in out.splitlines():
        print(f"  {line}")


# ── Main ──────────────────────────────────────────────────────────────────────

def main():
    # Fresh history for this benchmark run
    history = REPO_ROOT / ".kosh" / "history.tsv"
    if history.exists():
        history.unlink()

    print("=" * 70)
    print("  Kosh Primitive Benchmarks")
    print("  Measuring primitives directly — no LLM, no agent noise")
    print("=" * 70)

    suites = [
        bench_compression(),
        bench_leasing(),
        bench_packet_loading(),
        bench_batching(),
    ]

    for s in suites:
        print_suite(s)

    print_summary(suites)
    print_gain()
    print()


if __name__ == "__main__":
    main()
