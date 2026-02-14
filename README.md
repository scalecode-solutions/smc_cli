# RustyScalpel

Surgical search through Claude Code JSONL conversation logs.

Built in Rust for speed — searches 2.8GB+ of conversation history in under a second using parallel processing.

## Install

```bash
# From the project directory
cargo install --path .

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

```
RustyScalpel Stats
══════════════════════════════════════════════════
  Total sessions:  207
  Total size:      2.80GB
  Projects:        57

Top Projects by Size
──────────────────────────────────────────────────
  Clingy                           48 sessions   719.4MB
  ClingyX                           5 sessions   440.4MB
  mvServer                         40 sessions   409.6MB
  ...
```

### `smc sessions`

List sessions sorted by most recent, with the first user message as a preview.

```bash
# Show 20 most recent sessions (default)
smc sessions

# Show more
smc sessions -n 50

# Filter by project name
smc sessions -p Clingy
```

### `smc search <query> [query2] [query3] ...`

Full-text search across all conversations. Searches user messages, assistant responses, tool calls, tool results, and thinking blocks. Multiple queries are OR'd together. All searches are case-insensitive and substring-based (e.g., `"shel"` matches "shelby").

```bash
# Basic search
smc search "swiftui"

# Multiple terms (OR) — finds messages containing any of them
smc search "shelby" "upstream" "donut"

# Search only user messages
smc search "bug" --role user

# Search only assistant messages
smc search "error" --role assistant

# Filter by project
smc search "animation" -p Clingy

# Filter by date range
smc search "refactor" --after 2026-02-01
smc search "deploy" --before 2026-01-15
smc search "fix" --after 2026-02-01 --before 2026-02-14

# Filter by tool name (find conversations where a specific tool was used)
smc search "swift" --tool Bash
smc search "model" --tool Write

# Filter by git branch
smc search "merge" --branch main

# Regex search
smc search "func\s+\w+View" -e

# Limit results (default: 50)
smc search "todo" -n 10

# Combine filters
smc search "crash" --role user --after 2026-02-01 -p Clingy -n 20

# Print markdown to stdout (pipeable)
smc search "swiftui" --role user -o

# Save results to a markdown file
smc search "auth" -p mvServer --after 2026-02-01 --md auth_research.md

# Both: pipe markdown to stdout AND save to file
smc search "crash" -o --md crash_report.md
```

**Output flags:**
- `-o` prints clean markdown to stdout (suppresses colored output, great for piping)
- `--md <file>` saves results to a markdown file (terminal output stays normal)
- `-o --md <file>` does both

Each markdown result includes the query, active filters, session ID, timestamp, role, and a content preview.

### `smc show <session-id>`

Pretty-print an entire conversation with colored roles, timestamps, tool calls, and thinking blocks.

```bash
# Full session ID
smc show 394afc57-5feb-4d44-af8c-23b273bd4c56

# Prefix match (just enough to be unique)
smc show 394afc

# Include thinking blocks (shown by default but truncated)
smc show 394afc --thinking
```

### `smc tools <session-id>`

List all tool calls made during a session — useful for understanding what actions were taken.

```bash
smc tools 394afc
```

```
Tool calls in session: 394afc57-... (Geralds/Game)

  2026-01-25T08:37:20 assistant Glob
  2026-01-25T08:37:20 assistant Glob
  2026-01-25T08:37:25 assistant Bash
  ...

12 tool-calling messages
```

## Global Options

```
--path <PATH>    Override the Claude projects directory
                 Default: ~/.claude/projects
```

Use `--path` if your logs are in a non-standard location:

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

RustyScalpel uses [Rayon](https://github.com/rayon-rs/rayon) for parallel file processing — all CPU cores work simultaneously to scan files, which is why it can search gigabytes of data in under a second.

## Performance

Benchmarked on an Apple Silicon Mac with 207 sessions / 2.8GB of logs:

| Operation | Time |
|-----------|------|
| `stats` | instant |
| `sessions` | ~2s (reads first few lines of each file) |
| `search` (full scan) | ~0.5s |
| `show` (single session) | instant to ~1s depending on file size |

## Build from Source

Requires Rust 1.70+.

```bash
git clone <repo>
cd jsonxplorer

# Debug build (faster compile, slower runtime)
cargo build

# Release build (slower compile, faster runtime)
cargo build --release

# Install globally
cargo install --path .
```
