# smc — Search My Claude

[![Crates.io](https://img.shields.io/crates/v/smc-cli-cc.svg)](https://crates.io/crates/smc-cli-cc)
[![Downloads](https://img.shields.io/crates/d/smc-cli-cc.svg)](https://crates.io/crates/smc-cli-cc)
[![docs.rs](https://docs.rs/smc-cli-cc/badge.svg)](https://docs.rs/smc-cli-cc)
[![MSRV](https://img.shields.io/badge/MSRV-1.70-blue.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

> Surgical search through Claude Code conversation logs. Built in Rust for speed.

Claude Code stores every conversation as JSONL files — messages, tool calls, thinking blocks, timestamps, git context — but provides no way to search through them after context compaction. **smc** fixes that.

Search 2.8GB+ of conversation history in under a second.

---

## Quick Start

```bash
# Install from crates.io
cargo install smc-cli-cc

# Or build from source
git clone https://github.com/scalecode-solutions/smc_cli.git
cd smc_cli
cargo install --path .
```

```bash
smc --version       # Verify install
smc stats           # See what you've got
smc search "bug"    # Find something
```

---

## Commands at a Glance

| Command | Alias | Description |
|---------|-------|-------------|
| `smc search <query>` | `s` | Full-text search across all conversations |
| `smc sessions` | `ls` | List sessions with previews |
| `smc show <id>` | — | Pretty-print a conversation |
| `smc tools <id>` | `t` | List tool calls in a session |
| `smc export <id>` | `e` | Export session as markdown |
| `smc context <id> <line>` | `ctx` | Show messages around a line number |
| `smc projects` | `p` | List projects with stats |
| `smc recent` | `r` | Latest messages across all sessions |
| `smc freq [mode]` | `f` | Frequency analysis (chars, words, tools, roles) |
| `smc stats` | — | Aggregate statistics |

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
| `--count` | `-c` | Show match counts per project |
| `--summary` | | Condensed overview: projects, roles, dates, topics |
| `--json` | | Output as JSON lines |
| `--output` | `-o` | Markdown to stdout |
| `--md <FILE>` | | Save results to markdown file |
| `--include-smc` | `-i` | Include previous smc output (excluded by default) |
| `--exclude-session <ID>` | | Skip a specific session |

### AI-Friendly Features

smc is designed to work well when used by AI assistants inside Claude Code sessions:

```bash
smc search "bug" --exclude-session 394af           # Skip a specific session
smc search "bug" -i                                # Include previous smc output (excluded by default)
```

All smc output is wrapped in `<smc-cc-cli>` tags. By default, search excludes records containing these tags — preventing the recursion problem where an AI searching for "X" finds its own previous search results for "X". Use `-i`/`--include-smc` to opt back in.

### Output Modes

```bash
smc search "auth" --count                          # Match counts per project
smc search "auth" --summary                        # Condensed overview with topics
smc search "auth" --json                           # JSON lines (pipe to jq)
smc search "auth" -o                               # Markdown to stdout (pipeable)
smc search "auth" --md report.md                   # Save to markdown file
smc search "auth" -o --md report.md                # Both
```

Summary mode gives a condensed overview — projects, roles, date range, and auto-extracted topics — without flooding context:

```
Summary for 'deploy'

  Projects:
    myapp                                     36 matches
    backend-api                               14 matches

  Roles:
    assistant                                 39
    user                                      11

  Dates:    2026-01-15 → 2026-02-10
  Sessions: 4

  Topics:   docker, nginx, config, staging, migration, rollback, endpoint

50 total matches
```

Count mode is more compact — just per-project totals:

```
Match counts for 'auth'

  -Users-travis-GitHub-misc-myapp           12
  -Users-travis-GitHub-misc-relayterm        3
  -Users-travis-GitHub-misc-smc_cli          1

16 total matches across 3 projects
```

---

## Browse & Inspect

```bash
# List sessions (most recent first)
smc sessions                           # Default: 20 most recent
smc sessions -n 50                     # Show more
smc sessions -p MyProject              # Filter by project
smc sessions --after 2026-02-01        # After a date
smc sessions --before 2026-02-14       # Before a date

# View a conversation
smc show 394afc                        # Pretty-print with colors
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
smc export 394afc -o                   # Pipe to stdout

# Recent messages
smc recent                             # Last 10 across all sessions
smc recent -p MyProject                # Filter by project
smc recent --role user                 # Only user messages
```

---

## Analytics

### Stats & Projects

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

The default `freq chars` parses JSONL and counts only message content, producing accurate English letter distributions. Use `--raw` to count all bytes in the JSONL files (faster, but includes JSON structure and encoded data).

```
Character Frequency (a-z, case-insensitive, parsed content)
════════════════════════════════════════════════════════════
  e    16,489,526  (12.03%)  ████████████████████████████████████████
  t    12,567,821  ( 9.17%)  ██████████████████████████████
  a    10,824,395  ( 7.89%)  ██████████████████████████
  o    10,156,782  ( 7.41%)  █████████████████████████
  i     9,438,210  ( 6.89%)  ███████████████████████
  ...
────────────────────────────────────────────────────────────
  Total: 137,128,394  across 244 files (2.85GB)
```

```
Tool Usage Frequency
════════════════════════════════════════════════════════════
  Bash                     16,052  ( 28.5%)  ██████████████████████████████
  Read                     13,526  ( 24.1%)  █████████████████████████
  Edit                     11,794  ( 21.0%)  ██████████████████████
  Grep                      5,492  (  9.8%)  ██████████
  Write                     2,950  (  5.2%)  █████
  ...
────────────────────────────────────────────────────────────
  56,241 total tool calls
```

---

## Global Options

```bash
--path <PATH>    # Override Claude projects directory (default: ~/.claude/projects)
```

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

smc uses [Rayon](https://github.com/rayon-rs/rayon) for parallel file processing — all CPU cores scan simultaneously, which is why it searches gigabytes in under a second.

## Performance

Benchmarked on Apple Silicon, 244 sessions, 2.85GB total:

| Operation | Time |
|-----------|------|
| `search` (full scan) | ~0.5s |
| `freq chars` (parsed, 137M chars) | ~7s |
| `freq chars --raw` (2B+ chars) | ~80s |
| `stats` | instant |
| `show` | instant to ~1s |

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
make patch    # 0.5.0 → 0.5.1
make minor    # 0.5.0 → 0.6.0
make major    # 0.5.0 → 1.0.0
make current  # Show current version
```

---

## Why?

Claude Code logs everything — every message, tool call, thinking block, timestamp, git branch — as structured JSONL. But after context compaction, that history is gone. The only way to recover it was manually grepping through files that can be hundreds of megabytes.

smc gives Claude (and you) instant access to all of it.

---

## License

MIT
