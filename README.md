# jt-consoleutils

[![Crates.io](https://img.shields.io/crates/v/jt-consoleutils.svg)](https://crates.io/crates/jt-consoleutils)
[![docs.rs](https://docs.rs/jt-consoleutils/badge.svg)](https://docs.rs/jt-consoleutils)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

**Composable building blocks for polished Rust CLI tools.** Output and shell
abstractions with mock implementations, a zero-dependency JSON/JSONC parser,
a small CLI parsing framework, and the small utilities (paths, env vars,
filesystem, signals) that every CLI ends up reinventing.

```toml
[dependencies]
jt-consoleutils = "0.5"
```

```rust
use jt_consoleutils::output::{ConsoleOutput, Output, OutputMode};

let mut out = ConsoleOutput::new(OutputMode::default());
out.writeln("Hello from jt-consoleutils!");
```

For full API reference, see [docs.rs](https://docs.rs/jt-consoleutils).

---

## Why

- **Testable I/O.** `Output` and `Shell` are traits with `StringOutput` /
  `MockShell` test doubles in the box — no stdout-capture hacks needed.
- **Tiny binaries.** The JSON module is zero-dependency; no `serde`, no proc
  macros, no code generation.
- **Consistent CLI behavior.** A `CommandParser` trait handles `-v`, `-q`,
  `-d`, `-t`, `--help`, and `--version` so every tool in your ecosystem
  behaves the same way.
- **Opt-in features.** `verbose`, `trace`, `dotenv`, and `build-support` are
  feature-gated so you only pay for what you use.

---

## What's in the box

| Module | Purpose |
|---|---|
| [`output`](https://docs.rs/jt-consoleutils/latest/jt_consoleutils/output/) | `Output` trait with `ConsoleOutput` (production) and `StringOutput` (tests); `OutputMode` / `LogLevel` for verbosity; progress bars and file-stat summaries |
| [`shell`](https://docs.rs/jt-consoleutils/latest/jt_consoleutils/shell/) | `Shell` trait with `ProcessShell`, `DryRunShell`, and `MockShell`; animated spinner with scrolling viewport in default mode |
| [`json`](https://docs.rs/jt-consoleutils/latest/jt_consoleutils/json/) | Zero-dependency JSON/JSONC parser, serializer, and `JsonValue`; `FromJsonValue` / `ToJson` traits for typed (de)serialization; deep-merge and path-removal helpers |
| [`cli`](https://docs.rs/jt-consoleutils/latest/jt_consoleutils/cli/) | `CommandParser` trait with global flag handling, subcommand dispatch, rainbow-colorized help text, and `--version` printing |
| [`fs_utils`](https://docs.rs/jt-consoleutils/latest/jt_consoleutils/fs_utils/) | `FsError` with path context; `read_json_file` / `write_json_file_pretty` / `write_if_changed`; dry-run-aware `dry_write` / `dry_remove_file` |
| [`paths`](https://docs.rs/jt-consoleutils/latest/jt_consoleutils/paths/) | Home dir, `.`/`..` normalization, PATH membership, canonicalization with UNC stripping |
| [`terminal`](https://docs.rs/jt-consoleutils/latest/jt_consoleutils/terminal/) | ANSI color constants, rainbow colorizer, terminal width detection |
| [`str_utils`](https://docs.rs/jt-consoleutils/latest/jt_consoleutils/str_utils/) | `format_bytes`, `plural`, `path_to_string` |
| [`envvars`](https://docs.rs/jt-consoleutils/latest/jt_consoleutils/envvars/) | `${VAR}` expansion from the host environment |
| [`signals`](https://docs.rs/jt-consoleutils/latest/jt_consoleutils/signals/) | SIGINT / Ctrl+C handling for graceful shutdown and cooperative cancellation |
| [`vocab`](https://docs.rs/jt-consoleutils/latest/jt_consoleutils/vocab/) | `AsVerb` / `AsNoun` traits for typed action-line vocabulary |
| [`dotenv`](https://docs.rs/jt-consoleutils/latest/jt_consoleutils/dotenv/) *(feature: `dotenv`)* | `.env` file loader with rich error context |
| [`build_support`](https://docs.rs/jt-consoleutils/latest/jt_consoleutils/build_support/) *(feature: `build-support`)* | `build.rs` helper that emits `BUILD_DATE` and `GIT_HASH` |

---

## Scope and non-goals

`jt-consoleutils` is **synchronous and `std`-only** by design. It targets
hosted command-line tools that run a handful of child processes, parse a few
config files, and print to a terminal — the workload where blocking I/O on
the main thread is the right shape.

- **No `async` / `tokio` support, and none planned.** The `Shell` trait's
  methods are all blocking. If you're building an async tool or embedding
  shell calls into a `tokio` runtime, you'll want a different abstraction —
  either [`tokio::process`](https://docs.rs/tokio/latest/tokio/process/)
  directly, or your own `Shell`-shaped trait whose methods return futures.
- **No `no_std` support, and none planned.** The crate uses `std::process`,
  `std::fs`, `std::io`, threads, and `std::time` throughout; an embedded
  port would be a near-total rewrite.

Both are deliberate choices, not oversights. File an issue if your use case
is close enough to the current shape that a small extension would help, but
don't expect either to land in 0.x.

---

## Two examples

### Output with verbosity, captured in tests

```rust
use jt_consoleutils::output::{Output, StringOutput};

let mut out = StringOutput::new();
out.writeln("step 1");
out.writeln("step 2");
assert_eq!(out.log(), "step 1\nstep 2\n");
```

### Shell with dry-run support

```rust
use jt_consoleutils::output::{ConsoleOutput, OutputMode};
use jt_consoleutils::shell;

let mode = OutputMode { dry_run: true, ..OutputMode::default() };
let mut out = ConsoleOutput::new(mode);
let sh = shell::create(mode.is_dry_run());

sh.run_command("Install deps", "npm", &["install"], &mut out, mode)
    .expect("spawn failed");
// stdout: "[dry-run] would run: npm install"
```

---

## Feature flags

| Feature | Default | Description |
|---|---|---|
| `verbose` | off | Enables `LogLevel::Verbose`, the `verbose!` macro, `-v`/`--verbose` CLI flag, and verbose-only methods on `Output` and `Shell` |
| `trace` | off | Enables `LogLevel::Trace`, the `trace!` macro, `-t`/`--trace` CLI flag, and `format_trace_block` |
| `dotenv` | off | Enables the `dotenv` module (depends on `dotenvy`) |
| `build-support` | off | Exposes `build_support::emit_build_info()` for use in `build.rs` |

---

## MSRV

The minimum supported Rust version is **1.85** (Rust 2024 edition).

---

## License

Licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.

---

Built with care by [Original JT](https://originaljt.com).
