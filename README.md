# smc — Search My Claude

Surgical search through Claude Code JSONL conversation logs.

Built in Rust for speed — searches 2.8GB+ of conversation history in under a second using parallel processing.

## Install

```bash
# From the project directory
cargo install --path .

# Or use the Makefile
make install

# Verify
smc --version
```

The binary installs to `~/.cargo/bin/smc`.

## Commands

### `smc stats`

Overview of all your conversation data — total sessions, total size, and top projects by size.

```bash
smc stats
```

### `smc sessions` (alias: `smc ls`)

List sessions sorted by most recent, with the first user message as a preview.

```bash
# Show 20 most recent sessions (default)
smc sessions

# Show more
smc sessions -n 50

# Filter by project name
smc sessions -p Clingy

# Filter by date range
smc sessions --after 2026-02-01
smc sessions --before 2026-02-10
smc sessions --after 2026-02-01 --before 2026-02-14
```

### `smc projects` (alias: `smc p`)

List all projects with session counts, total sizes, and date ranges.

```bash
smc projects
```

```
57 projects

  Clingy                           48 sessions   720.7MB  2026-01-28 → 2026-02-14
  mvServer                         40 sessions   410.5MB  2026-01-28 → 2026-02-14
  ...
```

### `smc search <query> [query2] ...` (alias: `smc s`)

Full-text search across all conversations. Searches user messages, assistant responses, tool calls, tool results, and thinking blocks. Multiple queries are OR'd together. All searches are case-insensitive and substring-based.

```bash
# Basic search
smc search "swiftui"

# Multiple terms (OR)
smc search "shelby" "upstream" "donut"

# Filter by role
smc search "bug" --role user
smc search "error" --role assistant

# Filter by project
smc search "animation" -p Clingy

# Filter by date range
smc search "refactor" --after 2026-02-01
smc search "deploy" --before 2026-01-15

# Filter by tool name
smc search "swift" --tool Bash

# Filter by git branch
smc search "merge" --branch main

# Regex search
smc search "func\s+\w+View" -e

# Limit results (default: 50)
smc search "todo" -n 10

# Count mode — show match counts per project
smc search "swiftui" --count

# JSON output — one JSON object per line (pipeable to jq)
smc search "shelby" --json -n 5

# Print markdown to stdout (pipeable)
smc search "swiftui" -o

# Save results to a markdown file
smc search "auth" --md auth_research.md

# Both: pipe to stdout AND save to file
smc search "crash" -o --md crash_report.md
```

**Output flags:**
- `-o` prints clean markdown to stdout (suppresses colored output, great for piping)
- `--md <file>` saves results to a markdown file
- `--count` / `-c` shows aggregate match counts per project instead of results
- `--json` outputs one JSON object per line for piping to `jq`

### `smc show <session-id>`

Pretty-print a conversation with colored roles, timestamps, tool calls, and thinking blocks.

```bash
# Full session ID
smc show 394afc57-5feb-4d44-af8c-23b273bd4c56

# Prefix match (just enough to be unique)
smc show 394afc

# Include thinking blocks
smc show 394afc --thinking

# Show specific message range
smc show 394afc --from 5 --to 15
```

### `smc tools <session-id>` (alias: `smc t`)

List all tool calls made during a session.

```bash
smc tools 394afc
```

### `smc export <session-id>` (alias: `smc e`)

Export a session as clean markdown with collapsible thinking blocks, formatted tool calls, and result previews.

```bash
# Export to file (default: <session-id>.md)
smc export 394afc

# Export to a specific file
smc export 394afc --md my_session.md

# Export to stdout (pipeable)
smc export 394afc -o
```

### `smc context <session-id> <line>` (alias: `smc ctx`)

Show messages around a specific line number in a session. Useful when search gives you a line number and you want to see the surrounding conversation.

```bash
# Show 3 messages before and after line 50
smc context 394afc 50

# Adjust context window
smc context 394afc 50 -C 5
```

### `smc freq [mode]` (alias: `smc f`)

Frequency analysis across all conversation data. Runs in parallel across all files.

```bash
# Character frequency (a-z, case-insensitive) — default
smc freq
smc freq chars

# Word frequency (top words, 3+ chars)
smc freq words
smc freq words -n 50

# Tool usage frequency
smc freq tools

# Message role frequency
smc freq roles
```

Modes can be abbreviated: `c`, `w`, `t`, `r`.

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

### `smc recent` (alias: `smc r`)

Show the most recent messages across all sessions, sorted by timestamp.

```bash
# Show 10 most recent messages (default)
smc recent

# Show more
smc recent -n 25

# Filter by role
smc recent --role user
smc recent --role assistant
```

## Global Options

```
--path <PATH>    Override the Claude projects directory
                 Default: ~/.claude/projects
```

```bash
smc stats --path /custom/path/to/projects
```

## How It Works

Claude Code stores conversation logs as JSONL files in `~/.claude/projects/`. Each project gets a directory named after its filesystem path (e.g., `-Users-travis-GitHub-Clingy`), containing one `.jsonl` file per session.

Each line in a JSONL file is a record with a `type` field:

| Type | Description |
|------|-------------|
| `user` | User messages |
| `assistant` | Claude's responses (text, thinking, tool calls) |
| `system` | System messages |
| `file-history-snapshot` | File state snapshots |
| `progress` | Progress indicators |

smc uses [Rayon](https://github.com/rayon-rs/rayon) for parallel file processing — all CPU cores work simultaneously to scan files, which is why it can search gigabytes of data in under a second.

## Performance

Benchmarked on an Apple Silicon Mac with 207 sessions / 2.8GB of logs:

| Operation | Time |
|-----------|------|
| `stats` | instant |
| `sessions` | ~2s |
| `search` (full scan) | ~0.5s |
| `show` (single session) | instant to ~1s |

## Version Management

```bash
make patch    # 0.3.0 → 0.3.1
make minor    # 0.3.0 → 0.4.0
make major    # 0.3.0 → 1.0.0
make current  # Show current version
```

## Build from Source

Requires Rust 1.70+.

```bash
git clone <repo>
cd smc_cli

# Debug build
cargo build

# Release build
cargo build --release

# Install globally
cargo install --path .
```
