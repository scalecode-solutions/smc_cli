# smc — Search My Claude

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
smc search "swiftui"                              # Basic search
smc search "bug" "error" "crash"             # Multiple terms (OR)
smc search "bug" --role user                       # Only user messages
smc search "animation" -p myapp                   # Filter by project
smc search "refactor" --after 2026-02-01           # Date filter
smc search "swift" --tool Bash                     # Filter by tool
smc search "merge" --branch main                   # Filter by git branch
smc search "func\s+\w+View" -e                     # Regex
smc search "todo" -n 10                            # Limit results
```

### Output Modes

```bash
smc search "auth" --count                          # Match counts per project
smc search "auth" --json                           # JSON lines (pipe to jq)
smc search "auth" -o                               # Markdown to stdout
smc search "auth" --md report.md                   # Save to markdown file
smc search "auth" -o --md report.md                # Both
```

---

## Browse & Inspect

```bash
# List sessions (most recent first)
smc sessions                           # Default: 20 most recent
smc sessions -p MyProject              # Filter by project
smc sessions --after 2026-02-01        # Date range

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
smc export 394afc                      # Save as markdown
smc export 394afc -o                   # Pipe to stdout
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
smc freq              # Character frequency (a-z) — default
smc freq words        # Most common words
smc freq tools        # Tool usage breakdown
smc freq roles        # Message counts by role
smc freq words -n 50  # Top 50 words
```

Modes can be abbreviated: `chars`/`c`, `words`/`w`, `tools`/`t`, `roles`/`r`.

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

Benchmarked on Apple Silicon, 207 sessions, 2.8GB total:

| Operation | Time |
|-----------|------|
| `search` (full scan) | ~0.5s |
| `freq chars` (2B+ characters) | ~2s |
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
make patch    # 0.3.3 → 0.3.4
make minor    # 0.3.3 → 0.4.0
make major    # 0.3.3 → 1.0.0
make current  # Show current version
```

---

## Why?

Claude Code logs everything — every message, tool call, thinking block, timestamp, git branch — as structured JSONL. But after context compaction, that history is gone. The only way to recover it was manually grepping through files that can be hundreds of megabytes.

smc gives Claude (and you) instant access to all of it.

---

## License

MIT
