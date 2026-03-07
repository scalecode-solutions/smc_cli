<div align="center">

# smc — Search My Claude

**Surgical search through Claude Code conversation logs — structured JSONL output**

[![crates.io](https://img.shields.io/crates/v/smc-cli-cc.svg?style=flat-square&color=fc8d62&logo=rust)](https://crates.io/crates/smc-cli-cc)
[![crates.io downloads](https://img.shields.io/crates/d/smc-cli-cc?style=flat-square&color=2ecc71)](https://crates.io/crates/smc-cli-cc)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)

</div>

---

Claude Code stores every conversation as JSONL files — messages, tool calls, thinking blocks, timestamps, git context — but provides no way to search through them after context compaction. **smc** fixes that.

Every record is a JSON Line. Every command respects a token budget. Every result is parseable, composable, and consistent. Search 3GB+ of conversation history in milliseconds.

---

## Install

```bash
cargo install smc-cli-cc
```

---

## Commands

| Command | Alias | What it does |
|---------|-------|-------------|
| `smc search <query>` | `s` | Parallel full-text search across all conversations |
| `smc sessions` | `ls` | List sessions with previews, dates, and sizes |
| `smc show <id>` | — | Emit a conversation as JSONL message records |
| `smc tools <id>` | `t` | List every tool call in a session with timestamps |
| `smc stats` | — | Aggregate statistics: sessions, sizes, top projects |
| `smc export <id>` | `e` | Export a session as markdown (file or stdout) |
| `smc context <id> <line>` | `ctx` | Show messages around a specific JSONL line number |
| `smc projects` | `p` | List projects with session counts, sizes, and date ranges |
| `smc freq [mode]` | `f` | Frequency analysis: chars, words, tools, or roles |
| `smc recent` | `r` | Most recent messages across all sessions |

Session IDs support prefix matching — type just enough to be unique (e.g., `smc show 394af`).

---

## Search

The core feature. Parallel full-text search across every message, tool call, tool result, and thinking block.

```bash
smc search "authentication"                        # Basic search
smc search "bug" "error" "crash"                   # Multiple terms (OR)
smc search "bug" "deploy" -a                       # Multiple terms (AND)
smc search "refactor" --role user                  # Only user messages
smc search "deploy" -p myapp                       # Filter by project
smc search "migration" --after 2026-01-01          # After a date
smc search "hotfix" --before 2026-02-01            # Before a date
smc search "config" --tool Bash                    # Filter by tool name
smc search "merge" --branch main                   # Filter by git branch
smc search "fn\s+\w+_test" -e                      # Regex mode
smc search "todo" -n 10                            # Limit results
smc search "git push" --tool-input                 # Search tool commands/arguments only
smc search --file src/main.rs "refactor"           # Messages that touched a file
smc search "architecture" --thinking               # Search only thinking blocks
smc search "deploy" --no-thinking                  # Exclude thinking blocks
```

### Search Flags

| Flag | Short | Description |
|------|-------|-------------|
| `--role <ROLE>` | | Filter by role: `user`, `assistant`, `system` |
| `--tool <TOOL>` | | Filter by tool name (substring match) |
| `--project <NAME>` | `-p` | Filter by project name (substring match) |
| `--after <DATE>` | | Only results after date (YYYY-MM-DD) |
| `--before <DATE>` | | Only results before date (YYYY-MM-DD) |
| `--branch <BRANCH>` | | Filter by git branch |
| `--and` | `-a` | Require ALL terms to match (default is OR) |
| `--regex` | `-e` | Treat query as regex |
| `--max <N>` | `-n` | Maximum results (default: 50) |
| `--file <PATH>` | | Filter to messages that touch a file path |
| `--tool-input` | | Search only within tool input content |
| `--thinking` | | Search only within thinking blocks |
| `--no-thinking` | | Exclude thinking blocks from search |
| `--include-smc` | `-i` | Include previous smc output (excluded by default) |
| `--exclude-session <ID>` | | Skip a specific session |

### AI-Friendly Features

smc is designed to work well when used by AI assistants inside Claude Code sessions:

```bash
smc search "bug" --exclude-session 394af           # Skip the current session
smc search "bug" -i                                # Include previous smc output
```

By default, search excludes records containing `<smc-cc-cli>` tags — preventing the recursion problem where an AI searching for "X" finds its own previous search results for "X". Use `-i`/`--include-smc` to opt back in.

---

## Output Format

All output is JSON Lines — one record per line, zero ANSI, zero pagination:

```jsonl
{"type":"match","project":"myapp","session_id":"394afc...","line":42,"role":"user","timestamp":"2026-02-10T15:30:00Z","matched_query":"deploy","text":"..."}
{"type":"match","project":"myapp","session_id":"394afc...","line":87,"role":"assistant","timestamp":"2026-02-10T15:30:05Z","matched_query":"deploy","text":"..."}
{"type":"summary","query":"deploy","count":2,"files_scanned":293,"elapsed_ms":3}
```

Every command emits typed records with a `type` field. Pipe through `jq` for formatting:

```bash
smc search "auth" | jq 'select(.type == "match") | {project, role, line}'
smc stats | jq '.projects[] | {name, sessions}'
smc sessions -n 5 | jq 'select(.type == "session") | {session_id, project, preview}'
```

---

## Browse & Inspect

```bash
# List sessions (most recent first)
smc sessions                           # Default: 20 most recent
smc sessions -n 50                     # Show more
smc sessions -p MyProject              # Filter by project
smc sessions --after 2026-02-01        # After a date

# View a conversation
smc show 394afc                        # Emit as JSONL message records
smc show 394afc --thinking             # Include thinking blocks
smc show 394afc --from 5 --to 15       # Specific message range

# Drill into search results
smc context 394afc 50                  # Messages around line 50
smc context 394afc 50 -C 5            # Wider context window

# See what tools were used
smc tools 394afc

# Export for sharing
smc export 394afc                      # Save as <session-id>.md
smc export 394afc --md report.md       # Custom output path
smc export 394afc -o                   # Markdown to stdout

# Recent messages
smc recent                             # Last 10 across all sessions
smc recent -p MyProject                # Filter by project
smc recent --role user                 # Only user messages
```

---

## Analytics

```bash
smc stats        # Total sessions, size, top projects
smc projects     # All projects with session counts and date ranges
```

### Frequency Analysis

```bash
smc freq              # Character frequency (parsed message content) — default
smc freq --raw        # Character frequency (raw JSONL bytes)
smc freq words        # Most common words
smc freq tools        # Tool usage breakdown
smc freq roles        # Message counts by role
smc freq words -n 50  # Top 50 words
```

Modes can be abbreviated: `chars`/`c`, `words`/`w`, `tools`/`t`, `roles`/`r`.

---

## Global Options

```bash
--path <PATH>        # Override Claude projects directory (default: ~/.claude/projects)
--max-tokens <N>     # Hard cap on output tokens (0 = unlimited)
```

---

## Library Usage

smc is also a Rust library crate. Add it to your project:

```bash
cargo add smc-cli-cc
```

```rust
use smc::{cmd, output::Emitter, util::discover};

// Discover all conversation files
let dir = discover::claude_dir(None)?;
let files = discover::discover_jsonl_files(&dir)?;

// Search programmatically
let opts = cmd::search::SearchOpts {
    queries: vec!["authentication".into()],
    max_results: 10,
    // ...all other fields
};

// Emit to stdout
let mut em = Emitter::stdout(0);
cmd::search::run(&opts, &files, &mut em)?;

// Or capture in memory (for tests / programmatic use)
let mut em = Emitter::capturing(0);
cmd::search::run(&opts, &files, &mut em)?;
let records = em.into_records(); // Vec<serde_json::Value>
```

Available modules: `cmd` (search, sessions, show, tools, export, context, stats, projects, freq, recent), `models`, `output`, `util`.

---

## How It Works

Claude Code stores conversation logs as JSONL files in `~/.claude/projects/`. Each project gets a directory, and each session is a `.jsonl` file containing one JSON record per line:

| Record Type | Contents |
|-------------|----------|
| `user` | Your messages |
| `assistant` | Claude's responses — text, thinking blocks, tool calls |
| `system` | System prompts and context |
| `file-history-snapshot` | File state snapshots |
| `progress` | Progress indicators |

smc uses [Rayon](https://github.com/rayon-rs/rayon) for parallel file processing — all CPU cores scan simultaneously, which is why it searches gigabytes in milliseconds.

---

## Development

Requires Rust 1.70+.

```bash
git clone https://github.com/scalecode-solutions/smc_cli.git
cd smc_cli
cargo build --release
cargo install --path .
```

### Version Management

```bash
make patch    # 0.8.0 → 0.8.1
make minor    # 0.8.0 → 0.9.0
make major    # 0.8.0 → 1.0.0
make current  # Show current version
```

---

## Why?

Claude Code logs everything — every message, tool call, thinking block, timestamp, git branch — as structured JSONL. But after context compaction, that history is gone. The only way to recover it was manually grepping through files that can be hundreds of megabytes.

smc gives Claude (and you) instant access to all of it — as machine-parseable JSONL, composable with `jq`, respecting token budgets, and designed to sit naturally alongside tools like [mvtk](https://crates.io/crates/mvtk).

---

## License

MIT
