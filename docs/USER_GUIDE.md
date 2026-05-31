# Kosh User Guide: Expectations & Installation

Welcome to the **Kosh v0.1.0-beta** release. Kosh is designed to be a high-performance, zero-latency "Context Virtualization" proxy that drastically reduces the token costs of using AI agents in your terminal.

---

## What to Expect

1.  **Immediate Token Savings**: You can expect **60-90% reduction** in token retransmission costs for deep conversational turns (debugging, refactoring) through **Context Leasing**.
2.  **Zero-Latency Proxy**: Kosh is written in pure Rust with zero external runtime dependencies. It adds negligible overhead to your commands.
3.  **Command Parity**: Your agents (and you) can use short aliases like `gs` (git status) or `gd` (git diff), and Kosh will handle the expansion and "gain" tracking.
4.  **No Model Dependencies**: This version of Kosh is **deterministic**. It does not require an LLM, API keys, or embeddings to work. It saves tokens through architecture, not more AI.

---

## Prerequisites

To install and run Kosh, you need:

1.  **Rust & Cargo**: Latest stable version. [Install here](https://rustup.rs/).
2.  **Git**: For version control integration.
3.  **Unix-like environment**: macOS or Linux (see Platform Support below).
4.  **Standard Utilities**: `find` (used by the `search_files` batch tool).

---

## Installation

### The One-Liner (Recommended)
This script installs the CLI, initializes configuration, and sets up shell aliases for you and your agents.

```bash
curl -sSL https://raw.githubusercontent.com/ankit1057/kosh/main/scripts/install.sh | bash
```

### Manual Installation
If you prefer to install manually:

```bash
# Clone the repo
git clone https://github.com/ankit1057/kosh.git
cd kosh

# Build and install
cargo install --path apps/cli --force

# Initialize config
kosh config init
```

---

## Supported Platforms

| Platform | Support Level | Notes |
| :--- | :--- | :--- |
| **macOS (Intel/Apple Silicon)** | **Primary** | Fully tested and verified. |
| **Linux (Ubuntu/Debian/Arch)** | **Stable** | Supports standard shell hooks and utilities. |
| **Windows (WSL2)** | **Stable** | Works perfectly within a WSL2 environment. |
| **Windows (Native PowerShell/CMD)** | **Experimental** | Requires `find` to be in PATH. Shell hooks must be added manually to `$PROFILE`. |

---

## First Steps for a New User

Once installed, try the following flow to see Kosh in action:

1.  **Check your baseline**: `kosh gain` (should be empty).
2.  **Create a context lease**: 
    ```bash
    kosh lease create --repo my-app --feature auth --fingerprint initial --summary "Auth logic"
    ```
3.  **Reference the lease**:
    ```bash
    kosh lease touch lease:auth:001
    ```
4.  **Verify savings**:
    ```bash
    kosh gain --history
    ```
    You will see that Kosh just saved you **~5,000 tokens** by virtualizing that context reference.

---

## Help & Community
- **GitHub**: [ankit1057/kosh](https://github.com/ankit1057/kosh)
- **Issues**: Report bugs or request features on our [GitHub Issues](https://github.com/ankit1057/kosh/issues).
- **Whitepaper**: Read the [Token Economics Whitepaper](https://github.com/ankit1057/kosh/blob/main/docs/whitepaper.md) for the deep dive on how this works.
