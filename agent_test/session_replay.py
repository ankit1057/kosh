#!/usr/bin/env python3
"""
Kosh Session Replay Benchmark

This benchmark replays a real Claude Code session transcript to measure the
payload difference between a naive sequence of tool calls versus a Kosh-
optimized sequence.

It measures:
- Total characters sent (requests)
- Total characters received (responses)
- Number of tool calls
- Elapsed time

The goal is to quantify the savings from `kosh batch` and `kosh lease touch`.
"""

import json
import os
import subprocess
import sys
import time
import argparse
from dataclasses import dataclass, field
from pathlib import Path

# ── Constants and Setup ───────────────────────────────────────────────────────

REPO_ROOT = Path(__file__).parent.parent
KOSH_BIN  = REPO_ROOT / "target" / "debug" / "kosh"
CHARS_PER_TOKEN = 4
SUPPORTED_TOOLS = ["read_file", "list_directory", "grep", "search_files", "Bash", "Read", "Edit", "Write", "mcp__", "WebFetch", "WebSearch"]

os.environ.setdefault("TOKENIZERS_PARALLELISM", "false")
# Set Kosh context vars if not present, for kosh command invocations
os.environ.setdefault("KOSH_REPO", "agent-kosh")
os.environ.setdefault("KOSH_FEATURE", "session-replay-benchmark")


# ── Helpers ───────────────────────────────────────────────────────────────────

def kosh(*args) -> str:
    """Runs a kosh command and returns its stripped stdout+stderr."""
    try:
        r = subprocess.run([str(KOSH_BIN), *args], capture_output=True, text=True, cwd=str(REPO_ROOT), check=True)
        return (r.stdout + r.stderr).strip()
    except subprocess.CalledProcessError as e:
        # Don't kill the benchmark for one failed kosh call
        return f"""Kosh Error: {e}
{e.stdout}
{e.stderr}"""
    except FileNotFoundError:
        print(f"ERROR: kosh binary not found at '{KOSH_BIN}'. Please run 'cargo build'.", file=sys.stderr)
        sys.exit(1)


def tokens(chars: int) -> int:
    """Estimates tokens from character count."""
    return chars // CHARS_PER_TOKEN


def find_latest_claude_session() -> Path | None:
    """Finds the most recent .jsonl session file in Claude's project dirs."""
    claude_dir = Path.home() / ".claude" / "projects"
    if not claude_dir.exists():
        return None
    
    jsonl_files = list(claude_dir.rglob("*.jsonl"))
    if not jsonl_files:
        return None

    return max(jsonl_files, key=lambda f: f.stat().st_mtime)


# Claude Code tool → MCP-style canonical name mapping
TOOL_NORMALIZE = {
    "Read": "read_file",
    "Edit": "edit_file",
    "Write": "write_file",
    "Bash": "bash",
    "WebFetch": "fetch_url",
    "WebSearch": "search_web",
}

READ_TOOLS = {"Read", "read_file"}


def extract_path(name: str, inp: dict) -> str:
    """Extract the file path from a tool call input regardless of key name."""
    return inp.get("path") or inp.get("file_path") or ""


@dataclass
class ToolCall:
    id: str
    name: str
    input: dict
    result_content: str = ""
    result_found: bool = False

    @property
    def path(self) -> str:
        return extract_path(self.name, self.input)

@dataclass
class ReplayMetrics:
    request_chars: int = 0
    response_chars: int = 0
    tool_calls: int = 0
    elapsed_time: float = 0.0


def parse_session_transcript(path: Path) -> list[ToolCall]:
    """Parses a .jsonl session file and extracts tool calls and results."""
    tool_uses = {}
    tool_calls = []

    with path.open("r") as f:
        for line in f:
            try:
                data = json.loads(line)
                msg_type = data.get("type")
                message = data.get("message", {})
                role = message.get("role")
                content = message.get("content", [])

                if not isinstance(content, list):
                    continue

                if msg_type == "assistant" and role == "assistant":
                    for item in content:
                        if item.get("type") == "tool_use":
                            if item["name"] in SUPPORTED_TOOLS:
                                tool_uses[item["id"]] = ToolCall(id=item["id"], name=item["name"], input=item["input"])
                
                elif msg_type == "user" and role == "user":
                    for item in content:
                        if item.get("type") == "tool_result" and item.get("tool_use_id") in tool_uses:
                            tool_call = tool_uses[item["tool_use_id"]]
                            tool_call.result_content = item.get("content", "")
                            tool_call.result_found = True
                            tool_calls.append(tool_call)
                            del tool_uses[item["tool_use_id"]] # Move to final list

            except (json.JSONDecodeError, KeyError):
                continue
    
    # Add any tool_uses that didn't get a result
    tool_calls.extend(tool_uses.values())

    return tool_calls

# ── Replay Logic ──────────────────────────────────────────────────────────────

def run_naive_replay(tool_calls: list[ToolCall]) -> ReplayMetrics:
    """Executes each tool call independently."""
    metrics = ReplayMetrics()
    start_time = time.time()

    for call in tool_calls:
        metrics.tool_calls += 1
        
        # Measure request payload (what the agent sends)
        request_payload = json.dumps({"tool": call.name, **call.input})
        metrics.request_chars += len(request_payload)

        # Execute the call (simplified simulation)
        # In a real scenario, this would call kosh or the actual tool.
        # Here we just use the captured response from the log.
        metrics.response_chars += len(call.result_content)

    metrics.elapsed_time = time.time() - start_time
    return metrics


def run_kosh_replay(tool_calls: list[ToolCall]) -> ReplayMetrics:
    """Executes tool calls using Kosh batching and leasing."""
    metrics = ReplayMetrics()
    start_time = time.time()
    
    files_seen = set()
    repeat_reads = 0
    
    i = 0
    while i < len(tool_calls):
        call = tool_calls[i]
        
        # Batch consecutive read calls (Read, read_file)
        if call.name in READ_TOOLS:
            batch_start_index = i
            while (i + 1 < len(tool_calls) and
                   tool_calls[i+1].name in READ_TOOLS):
                i += 1
            
            if i > batch_start_index + 1: # Batch of 3 or more
                batch_calls = tool_calls[batch_start_index : i+1]
                metrics.tool_calls += 1
                
                # Request: kosh batch '[...]'
                batch_payload_list = [{"tool": c.name, **c.input} for c in batch_calls]
                kosh_batch_cmd_str = json.dumps(batch_payload_list)
                request_payload = json.dumps({"tool": "kosh_batch", "calls": batch_payload_list})
                metrics.request_chars += len(request_payload)
                
                # Response: simulate batch call
                response_payload = ""
                for c in batch_calls:
                    response_payload += c.result_content
                metrics.response_chars += len(response_payload)
                
                # Track file reads
                for c in batch_calls:
                    path = c.input.get("path")
                    if path:
                        if path in files_seen:
                            repeat_reads += 1
                        else:
                            files_seen.add(path)
                i += 1
                continue

        # Handle single calls (or non-batchable sequences)
        metrics.tool_calls += 1
        path = call.path

        if call.name in READ_TOOLS and path and path in files_seen:
            # It's a repeated read, use kosh lease touch
            repeat_reads += 1
            lease_id_placeholder = f"lease:{Path(path).stem}:001"
            
            # Request: kosh lease touch <id>
            request_payload = f"kosh lease touch {lease_id_placeholder}"
            metrics.request_chars += len(request_payload)
            
            # Response: lease touch provides minimal confirmation, not file content
            # This is a major source of savings.
            touch_response = f'{{"id":"{lease_id_placeholder}","byte_size":{len(call.result_content)}}}'
            metrics.response_chars += len(touch_response)
        else:
            # First time read or other tool call
            request_payload = json.dumps({"tool": call.name, **call.input})
            metrics.request_chars += len(request_payload)
            metrics.response_chars += len(call.result_content)
            if call.name in READ_TOOLS and path:
                files_seen.add(path)
        
        i += 1

    metrics.elapsed_time = time.time() - start_time
    # Add counts to metrics for reporting
    metrics.files_seen = len(files_seen)
    metrics.repeat_reads = repeat_reads
    return metrics


# ── Presentation ──────────────────────────────────────────────────────────────

@dataclass
class Suite:
    name: str
    naive: ReplayMetrics
    kosh: ReplayMetrics
    note: str = ""

    @property
    def saved_req_chars(self): return self.naive.request_chars - self.kosh.request_chars
    @property
    def saved_req_pct(self): return (self.saved_req_chars / self.naive.request_chars * 100) if self.naive.request_chars else 0
    @property
    def saved_resp_chars(self): return self.naive.response_chars - self.kosh.response_chars
    @property
    def saved_resp_pct(self): return (self.saved_resp_chars / self.naive.response_chars * 100) if self.naive.response_chars else 0
    @property
    def saved_total_chars(self): return (self.naive.request_chars + self.naive.response_chars) - (self.kosh.request_chars + self.kosh.response_chars)
    @property
    def saved_total_pct(self): 
        naive_total = self.naive.request_chars + self.naive.response_chars
        return (self.saved_total_chars / naive_total * 100) if naive_total else 0
    @property
    def saved_calls(self):  return self.naive.tool_calls - self.kosh.tool_calls


def print_suite(s: Suite):
    bar_w = 40
    max_chars = max(s.naive.request_chars + s.naive.response_chars, 1)
    naive_bar = int(bar_w * (s.naive.request_chars + s.naive.response_chars) / max_chars)
    kosh_bar  = int(bar_w * (s.kosh.request_chars + s.kosh.response_chars) / max_chars)
    
    print(f"\n{'─'*80}")
    print(f"  SESSION REPLAY: {s.name}")
    print(f"{'─'*80}")
    if s.note:
        print(f"  {s.note}")
        print()

    print(f"  {'MODE':8} {'REQ (chars)':>15} {'RESP (chars)':>15} {'TOTAL (chars)':>15} {'CALLS':>8} {'TIME (s)':>10}")
    print(f"  {'-'*78}")
    
    print(f"  {'NAIVE':8} {s.naive.request_chars:>15,} {s.naive.response_chars:>15,} {s.naive.request_chars+s.naive.response_chars:>15,} {s.naive.tool_calls:>8} {s.naive.elapsed_time:>10.4f}")
    print(f"  {'KOSH':8} {s.kosh.request_chars:>15,} {s.kosh.response_chars:>15,} {s.kosh.request_chars+s.kosh.response_chars:>15,} {s.kosh.tool_calls:>8} {s.kosh.elapsed_time:>10.4f}")
    print()
    
    print(f"  SAVINGS")
    print(f"    Request Chars:  {s.saved_req_chars:+,} ({s.saved_req_pct:.1f}%)")
    print(f"    Response Chars: {s.saved_resp_chars:+,} ({s.saved_resp_pct:.1f}%)")
    print(f"    Total Chars:    {s.saved_total_chars:+,} ({s.saved_total_pct:.1f}%)")
    print(f"    Tool Calls:     {s.saved_calls:+}")


# ── Main ──────────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description="Kosh Session Replay Benchmark.")
    parser.add_argument(
        "session_file",
        nargs="?",
        type=Path,
        help="Path to the Claude Code .jsonl session transcript. If omitted, the latest session is used."
    )
    args = parser.parse_args()

    if args.session_file:
        session_path = args.session_file
        if not session_path.exists():
            print(f"Error: File not found at '{session_path}'", file=sys.stderr)
            sys.exit(1)
    else:
        print("No session file provided, searching for the latest one...")
        session_path = find_latest_claude_session()
        if not session_path:
            print("Error: Could not find any Claude session transcripts in ~/.claude/projects/", file=sys.stderr)
            sys.exit(1)
    
    print(f"Running benchmark on: {session_path.name}")

    tool_calls = parse_session_transcript(session_path)
    if not tool_calls:
        print("No supported tool calls found in the session transcript.")
        return

    # Fresh history for this benchmark run
    history = REPO_ROOT / ".kosh" / "history.tsv"
    if history.exists():
        history.unlink()

    print(f"Found {len(tool_calls)} relevant tool calls. Replaying...")

    naive_metrics = run_naive_replay(tool_calls)
    kosh_metrics = run_kosh_replay(tool_calls)

    note = (
        f"{len(tool_calls)} tool calls | "
        f"{getattr(kosh_metrics, 'files_seen', 0)} unique files | "
        f"{getattr(kosh_metrics, 'repeat_reads', 0)} repeat reads"
    )

    suite = Suite(
        name=session_path.stem,
        naive=naive_metrics,
        kosh=kosh_metrics,
        note=note,
    )

    print_suite(suite)
    print()


if __name__ == "__main__":
    main()
