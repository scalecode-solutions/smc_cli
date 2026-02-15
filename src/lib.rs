//! # smc — Search My Claude
//!
//! Programmatic access to Claude Code conversation logs.
//!
//! Claude Code stores every conversation as JSONL files — messages, tool calls,
//! thinking blocks, timestamps, git context. This library provides fast, parallel
//! search and analysis across all of them.
//!
//! ## Quick Start
//!
//! ```no_run
//! use smc_cli_cc::{config::Config, search::{self, SearchOpts}};
//!
//! let cfg = Config::new(None).unwrap();
//! let files = cfg.discover_jsonl_files().unwrap();
//!
//! let opts = SearchOpts {
//!     queries: vec!["authentication".to_string()],
//!     is_regex: false,
//!     and_mode: false,
//!     role: None,
//!     tool: None,
//!     project: Some("myapp".to_string()),
//!     after: None,
//!     before: None,
//!     branch: None,
//!     max_results: 10,
//!     stdout_md: false,
//!     md_file: None,
//!     count_mode: false,
//!     summary_mode: false,
//!     json_mode: false,
//!     include_smc: false,
//!     exclude_session: None,
//! };
//!
//! search::search(&files, &opts).unwrap();
//! ```

pub mod analytics;
pub mod config;
pub mod display;
pub mod models;
pub mod search;
pub mod session;
