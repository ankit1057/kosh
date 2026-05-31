#!/usr/bin/env python3
"""
Kosh Token Savings Agent — exhaustive test harness.

Runs Qwen2.5-Coder-3B-Instruct via mlx-lm as a tool-calling agent.
Compares NAIVE mode (serial read_file only) vs KOSH mode (batch/packet/lease).
Measures and reports token savings per scenario.
"""

import json
import os
import re
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

# ── mlx-lm imports ────────────────────────────────────────────────────────────
from mlx_lm import load, generate
from mlx_lm.sample_utils import make_sampler
from mlx_lm.tokenizer_utils import TokenizerWrapper

# ── Config ────────────────────────────────────────────────────────────────────
MODEL_ID = "mlx-community/Qwen2.5-Coder-3B-Instruct-4bit"
REPO_ROOT = Path(__file__).parent.parent
KOSH_BIN  = REPO_ROOT / "target" / "debug" / "kosh"
MAX_TOKENS = 1024
TEMP       = 0.1   # near-deterministic for reproducible tests

# ── Token counting ─────────────────────────────────────────────────────────────
@dataclass
class TurnStats:
    prompt_tokens: int = 0
    gen_tokens: int    = 0
    tool_calls: int    = 0
    tool_result_tokens: int = 0

    @property
    def total(self):
        return self.prompt_tokens + self.gen_tokens + self.tool_result_tokens


@dataclass
class ScenarioResult:
    name: str
    mode: str            # "naive" or "kosh"
    turns: list[TurnStats] = field(default_factory=list)
    tool_call_log: list[dict] = field(default_factory=list)
    answer: str = ""
    elapsed_s: float = 0.0

    @property
    def total_tokens(self):
        return sum(t.total for t in self.turns)

    @property
    def total_tool_calls(self):
        return sum(t.tool_calls for t in self.turns)


# ── Kosh tool execution ────────────────────────────────────────────────────────
def run_kosh(*args) -> str:
    env = {**os.environ, "KOSH_REPO": "agent-kosh", "KOSH_FEATURE": "agent-test"}
    result = subprocess.run(
        [str(KOSH_BIN), *args],
        capture_output=True, text=True, env=env,
        cwd=str(REPO_ROOT)
    )
    return (result.stdout + result.stderr).strip()


def execute_tool(name: str, arguments: dict, mode: str) -> str:
    """Dispatch a tool call. Returns the result as a string."""
    if name == "read_file":
        path = arguments.get("path", "")
        full = REPO_ROOT / path
        if full.exists():
            content = full.read_text(errors="replace")
            if len(content) > 4096:
                content = content[:4096] + "...[truncated]"
            return content
        return f"error: file not found: {path}"

    if name == "kosh_batch":
        calls = arguments.get("calls", [])
        payload = json.dumps(calls)
        return run_kosh("batch", payload)

    if name == "kosh_packet_load":
        packet_name = arguments.get("name", "")
        return run_kosh("packet", "load", packet_name)

    if name == "kosh_lease_touch":
        lease_id = arguments.get("id", "")
        return run_kosh("lease", "touch", lease_id)

    if name == "kosh_packet_list":
        return run_kosh("packet", "list")

    if name == "kosh_lease_list":
        return run_kosh("lease", "list")

    return f"error: unknown tool: {name}"


# ── Qwen2.5 tool-call format ──────────────────────────────────────────────────
# Qwen2.5-Coder may output tool calls in either format:
#   <tool_call>{...}</tool_call>
#   ```json\n{...}\n```
TOOL_CALL_XML_RE  = re.compile(r"<tool_call>\s*(\{.*?\})\s*</tool_call>", re.DOTALL)
TOOL_CALL_JSON_RE = re.compile(r"```(?:json)?\s*(\{[^`]*\"name\"\s*:[^`]*\})\s*```", re.DOTALL)


def parse_tool_calls(text: str) -> list[dict]:
    calls = []
    for pattern in (TOOL_CALL_XML_RE, TOOL_CALL_JSON_RE):
        for m in pattern.finditer(text):
            try:
                obj = json.loads(m.group(1))
                if "name" in obj:
                    calls.append(obj)
            except json.JSONDecodeError:
                pass
    return calls


def tool_definitions(mode: str) -> list[dict]:
    base = [
        {
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read a single file from the repository.",
                "parameters": {
                    "type": "object",
                    "properties": {"path": {"type": "string", "description": "Repo-relative path"}},
                    "required": ["path"]
                }
            }
        }
    ]
    if mode == "kosh":
        base += [
            {
                "type": "function",
                "function": {
                    "name": "kosh_batch",
                    "description": "Read multiple files in ONE call — far more efficient than serial read_file calls. Use this whenever you need to read 2+ files.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "calls": {
                                "type": "array",
                                "items": {"type": "object"},
                                "description": 'Array of MCP calls e.g. [{"tool":"read_file","path":"..."}]'
                            }
                        },
                        "required": ["calls"]
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "kosh_packet_load",
                    "description": "Load a pre-defined context packet (a named bundle of files+symbols). One call replaces reading many files individually.",
                    "parameters": {
                        "type": "object",
                        "properties": {"name": {"type": "string", "description": "Packet name"}},
                        "required": ["name"]
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "kosh_lease_touch",
                    "description": "Reference a previously cached context lease by ID. The context is already in memory — no file reads needed.",
                    "parameters": {
                        "type": "object",
                        "properties": {"id": {"type": "string", "description": "Lease ID e.g. lease:auth:001"}},
                        "required": ["id"]
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "kosh_packet_list",
                    "description": "List available context packets.",
                    "parameters": {"type": "object", "properties": {}}
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "kosh_lease_list",
                    "description": "List available context leases.",
                    "parameters": {"type": "object", "properties": {}}
                }
            }
        ]
    return base


SYSTEM_PROMPT = """You are an expert Rust code analyst. You have access to tools to read files in the repository.

{tool_hint}

When you need to read multiple files, prefer batch/packet tools over individual reads.
Always think step by step. When done, provide a concise but complete answer."""


def system_for_mode(mode: str) -> str:
    hint = (
        "EFFICIENCY RULE: Never call read_file more than once per turn. "
        "Use kosh_batch to read 2+ files in a single call, or kosh_packet_load for pre-bundled contexts."
        if mode == "kosh"
        else "Read files one at a time as needed."
    )
    return SYSTEM_PROMPT.format(tool_hint=hint)


# ── Chat / generation loop ─────────────────────────────────────────────────────
def build_prompt(tokenizer, messages: list[dict], tools: list[dict]) -> str:
    """Build prompt using tokenizer's native chat template (handles tool-call format correctly)."""
    try:
        return tokenizer.apply_chat_template(
            messages,
            tools=tools if tools else None,
            tokenize=False,
            add_generation_prompt=True,
        )
    except Exception:
        # Fallback: manual ChatML
        tools_json = json.dumps(tools, indent=2)
        parts = []
        for msg in messages:
            role = msg["role"]
            content = msg["content"]
            if role == "system" and tools:
                content = content + f"\n\nAvailable tools:\n```json\n{tools_json}\n```"
            parts.append(f"<|im_start|>{role}\n{content}<|im_end|>")
        parts.append("<|im_start|>assistant\n")
        return "\n".join(parts)


def count_tokens(tokenizer, text: str) -> int:
    return len(tokenizer.encode(text))


def run_agent(
    model,
    tokenizer,
    scenario_prompt: str,
    mode: str,
    max_turns: int = 6,
) -> ScenarioResult:
    tools  = tool_definitions(mode)
    system = system_for_mode(mode)
    messages = [
        {"role": "system",    "content": system},
        {"role": "user",      "content": scenario_prompt},
    ]
    result = ScenarioResult(name="", mode=mode)
    t0 = time.time()

    for _turn in range(max_turns):
        turn_stat = TurnStats()
        prompt = build_prompt(tokenizer, messages, tools)
        turn_stat.prompt_tokens = count_tokens(tokenizer, prompt)

        sampler = make_sampler(temp=TEMP)
        raw = generate(model, tokenizer, prompt=prompt, max_tokens=MAX_TOKENS, sampler=sampler, verbose=False)
        turn_stat.gen_tokens = count_tokens(tokenizer, raw)

        tool_calls = parse_tool_calls(raw)

        if not tool_calls:
            # No tool call — agent produced final answer
            result.answer = raw.strip()
            result.turns.append(turn_stat)
            break

        # Execute each tool call
        messages.append({"role": "assistant", "content": raw})
        for call in tool_calls:
            fn_name = call.get("name", "")
            fn_args = call.get("arguments", {})
            if isinstance(fn_args, str):
                try:
                    fn_args = json.loads(fn_args)
                except Exception:
                    fn_args = {}
            turn_stat.tool_calls += 1
            result.tool_call_log.append({"tool": fn_name, "args": fn_args})
            tool_output = execute_tool(fn_name, fn_args, mode)[:3000]
            # Qwen2.5 expects one tool message per call with name field
            messages.append({
                "role": "tool",
                "name": fn_name,
                "content": tool_output,
            })
            turn_stat.tool_result_tokens += count_tokens(tokenizer, tool_output)
        result.turns.append(turn_stat)

    result.elapsed_s = time.time() - t0
    return result


# ── Test scenarios ─────────────────────────────────────────────────────────────
SCENARIOS = [
    {
        "name": "multi-file-architecture",
        "prompt": (
            "Explain the architecture of the Kosh codebase. "
            "Read these files: apps/cli/src/main.rs, crates/cache_engine/src/lib.rs, "
            "crates/packet_engine/src/lib.rs, crates/cost_estimator/src/lib.rs, "
            "crates/tool_registry/src/lib.rs. "
            "Summarise what each crate does in 2-3 sentences."
        ),
    },
    {
        "name": "lease-vs-reread",
        "prompt": (
            "I have a context lease lease:auth:001 for the auth module. "
            "Using it (or the cache/lease system), tell me what the LeaseRecord struct fields are "
            "and what the to_compact_json method outputs."
        ),
    },
    {
        "name": "packet-discovery",
        "prompt": (
            "List available context packets and leases. "
            "Then load any relevant packet and tell me which files it bundles."
        ),
    },
    {
        "name": "coding-task-batch",
        "prompt": (
            "Read Cargo.toml, apps/cli/Cargo.toml, and crates/packet_engine/Cargo.toml. "
            "List all workspace members and their direct dependencies."
        ),
    },
    {
        "name": "deep-code-review",
        "prompt": (
            "Read crates/cache_engine/src/lease.rs and crates/packet_engine/src/lib.rs. "
            "Compare the TSV serialisation approaches. Are they consistent? "
            "What fields do each store and how do they escape special characters?"
        ),
    },
]


# ── Reporting ─────────────────────────────────────────────────────────────────
def print_scenario_result(r: ScenarioResult):
    print(f"\n  mode={r.mode}  turns={len(r.turns)}  tool_calls={r.total_tool_calls}  "
          f"tokens={r.total_tokens}  elapsed={r.elapsed_s:.1f}s")
    for i, t in enumerate(r.turns):
        print(f"    turn {i+1}: prompt={t.prompt_tokens} gen={t.gen_tokens} "
              f"tool_result={t.tool_result_tokens} calls={t.tool_calls}")
    if r.tool_call_log:
        print("  tools used:", ", ".join(c["tool"] for c in r.tool_call_log))
    if r.answer:
        preview = r.answer[:300].replace("\n", " ")
        print(f"  answer: {preview}{'...' if len(r.answer) > 300 else ''}")


def print_comparison(naive: ScenarioResult, kosh: ScenarioResult):
    token_saved = naive.total_tokens - kosh.total_tokens
    call_saved  = naive.total_tool_calls - kosh.total_tool_calls
    pct = (token_saved / naive.total_tokens * 100) if naive.total_tokens else 0
    print(f"\n  DELTA  tokens_saved={token_saved} ({pct:.1f}%)  tool_calls_saved={call_saved}")


def run_kosh_gain():
    result = subprocess.run(
        [str(KOSH_BIN), "gain", "--by-kind"],
        capture_output=True, text=True,
        env={**os.environ, "KOSH_REPO": "agent-kosh"},
        cwd=str(REPO_ROOT)
    )
    return result.stdout.strip()


# ── Setup: seed packets and leases for kosh mode ──────────────────────────────
def seed_kosh_context():
    print("Seeding Kosh context (packets + leases)...")
    # Create an 'arch' packet bundling all crate lib.rs files
    run_kosh("packet", "create",
        "--name", "arch",
        "--file", "crates/cache_engine/src/lib.rs",
        "--file", "crates/cache_engine/src/lease.rs",
        "--file", "crates/packet_engine/src/lib.rs",
        "--file", "crates/cost_estimator/src/lib.rs",
        "--file", "crates/tool_registry/src/lib.rs",
        "--file", "crates/mcp_router/src/lib.rs",
        "--file", "crates/indexer/src/lib.rs",
    )
    # Create an 'auth' packet for the lease/cache engine
    run_kosh("packet", "create",
        "--name", "auth",
        "--file", "crates/cache_engine/src/lib.rs",
        "--file", "crates/cache_engine/src/lease.rs",
        "--symbol", "@authrepo",
    )
    # Create a lease for the auth module
    run_kosh("lease", "create",
        "--repo", "agent-kosh",
        "--feature", "auth",
        "--fingerprint", "lease-test-001",
        "--summary", "LeaseRecord struct with fields: id, repo, feature, fingerprint, summary, created_at (u64 unix seconds), access_count (u64). to_compact_json() emits: {\"id\":\"...\",\"repo\":\"...\",\"feature\":\"...\",\"fingerprint\":\"...\",\"summary\":\"...\",\"created_at\":N,\"access_count\":N}",
    )
    print("  created: packet:arch, packet:auth, lease:auth:001")
    print("  packets:", run_kosh("packet", "list"))
    print("  leases: ", run_kosh("lease", "list"))


# ── Main ──────────────────────────────────────────────────────────────────────
def main():
    print("=" * 70)
    print(f"Kosh Token Savings Agent Test")
    print(f"Model: {MODEL_ID}")
    print(f"Repo:  {REPO_ROOT}")
    print("=" * 70)

    # Seed Kosh context
    seed_kosh_context()

    # Load model
    print(f"\nLoading {MODEL_ID} ...")
    model, tokenizer = load(MODEL_ID)
    print("Model loaded.\n")

    summary_rows = []

    for scenario in SCENARIOS:
        print("\n" + "─" * 70)
        print(f"SCENARIO: {scenario['name']}")
        print(f"PROMPT:   {scenario['prompt'][:120]}...")

        # Run naive mode
        print("\n[NAIVE mode — serial read_file only]")
        r_naive = run_agent(model, tokenizer, scenario["prompt"], mode="naive")
        r_naive.name = scenario["name"]
        print_scenario_result(r_naive)

        # Run kosh mode
        print("\n[KOSH mode — batch / packet / lease]")
        r_kosh = run_agent(model, tokenizer, scenario["prompt"], mode="kosh")
        r_kosh.name = scenario["name"]
        print_scenario_result(r_kosh)

        print_comparison(r_naive, r_kosh)

        summary_rows.append({
            "scenario": scenario["name"],
            "naive_tokens": r_naive.total_tokens,
            "kosh_tokens":  r_kosh.total_tokens,
            "naive_calls":  r_naive.total_tool_calls,
            "kosh_calls":   r_kosh.total_tool_calls,
        })

    # Final summary
    print("\n" + "=" * 70)
    print("SUMMARY")
    print("=" * 70)
    print(f"{'Scenario':<28} {'Naive tok':>10} {'Kosh tok':>10} {'Saved':>8} {'Saved%':>7} {'Calls↓':>7}")
    print("-" * 70)
    total_naive = total_kosh = total_calls_saved = 0
    for row in summary_rows:
        saved  = row["naive_tokens"] - row["kosh_tokens"]
        pct    = (saved / row["naive_tokens"] * 100) if row["naive_tokens"] else 0
        cdiff  = row["naive_calls"] - row["kosh_calls"]
        total_naive       += row["naive_tokens"]
        total_kosh        += row["kosh_tokens"]
        total_calls_saved += cdiff
        print(f"{row['scenario']:<28} {row['naive_tokens']:>10} {row['kosh_tokens']:>10} "
              f"{saved:>8} {pct:>6.1f}% {cdiff:>7}")
    total_saved = total_naive - total_kosh
    total_pct   = (total_saved / total_naive * 100) if total_naive else 0
    print("-" * 70)
    print(f"{'TOTAL':<28} {total_naive:>10} {total_kosh:>10} {total_saved:>8} {total_pct:>6.1f}% {total_calls_saved:>7}")

    # Kosh gain report
    print("\n" + "=" * 70)
    print("KOSH GAIN (gain --by-kind):")
    print(run_kosh_gain())
    print("=" * 70)


if __name__ == "__main__":
    main()
