/// smc — Search My Claude.
///
/// Programmatic access to Claude Code conversation logs.
/// All subcommands emit JSON Lines — zero ANSI, zero pagination, machine-parseable.
///
/// Module layout:
///   util/    — token counting, JSONL discovery
///   output/  — `Emitter<W>`, shared record types
///   models/  — Claude Code JSONL record types (deserialization)
///   cmd/     — one module per subcommand, each exposing XxxOpts + run(opts, &mut Emitter)

pub mod util;
pub mod output;
pub mod models;
pub mod cmd;
