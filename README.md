# jt-consoleutils

[![Crates.io](https://img.shields.io/crates/v/jt-consoleutils.svg)](https://crates.io/crates/jt-consoleutils)
[![docs.rs](https://docs.rs/jt-consoleutils/badge.svg)](https://docs.rs/jt-consoleutils)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/jasontaylor137/jt-consoleutils#license)

**An opinionated toolkit for building polished Rust CLI tools — with a small
binary footprint.**

`jt-consoleutils` gives you everything a professional CLI needs out of the box:
structured output with verbosity levels, shell execution with animated spinners,
zero-dependency JSON/JSONC parsing, argument handling, rainbow help text, and
dry-run support — all wired together so the pieces work as a cohesive whole.

Built by [Original JT](https://originaljt.com) and used in production by
[Script Runner (sr)](https://originaljt.com), a developer-centric task
automation tool.

---

## Why jt-consoleutils?

Most CLI crates solve one problem well but leave you gluing five or six
dependencies together yourself.  `jt-consoleutils` is the glue _and_ the
pieces:

- **Tiny binaries.** The JSON module is zero-dependency — no proc macros, no
  `serde`, no code generation. Your release builds stay lean.
- **Testable by default.** Every I/O abstraction (`Output`, `Shell`) ships with
  mock/test implementations. Swap `ConsoleOutput` for `StringOutput` and assert
  on exactly what your CLI printed — no stdout capture hacks.
- **Batteries included.** Output levels, dry-run mode, shell spinners, JSONC
  comments, help-text colorization, CLI argument parsing — one `cargo add` and
  you're building features, not infrastructure.
- **Opinionated, not rigid.** The framework handles global flags (`-v`, `-q`,
  `-d`, `--help`, `--version`) so every tool in your ecosystem behaves
  consistently. Your code only deals with subcommands and domain logic.

---

## Quick start

```toml
[dependencies]
jt-consoleutils = "0.2"
```

```rust
use jt_consoleutils::output::{ConsoleOutput, Output, OutputMode};

fn main() {
    let mode = OutputMode::default();
    let mut out = ConsoleOutput::new(mode);
    out.writeln("Hello from jt-consoleutils!");
}
```

---

## Output

`Output` is a trait that abstracts where CLI output goes.  `ConsoleOutput`
writes to stdout; `StringOutput` captures output in memory for tests.  Both
respect `OutputMode`, which carries a `LogLevel` and a `dry_run` flag.

```rust
use jt_consoleutils::output::{ConsoleOutput, Output, OutputMode};

let mode = OutputMode::default();
let mut out = ConsoleOutput::new(mode);

out.writeln("Deploying to production...");
out.step_result("Build", true, 1_420, &[]);
// => ✓ Build (1s)
```

### OutputMode and LogLevel

`OutputMode` pairs a verbosity level with the dry-run flag.  The four levels
are `Quiet`, `Normal` (default), `Verbose`, and `Trace` — each successive
level shows more detail.

```rust
use jt_consoleutils::output::{LogLevel, OutputMode};

// Normal — progress messages only
let normal = OutputMode::default();

// Quiet — suppress everything except errors
let quiet = OutputMode { level: LogLevel::Quiet, ..OutputMode::default() };

// Dry-run — announce what would happen without doing it
let dry_run = OutputMode { dry_run: true, ..OutputMode::default() };
```

> `Verbose` and `Trace` are compile-time feature-gated (see
> [Feature flags](#feature-flags)) so they add zero overhead when unused.

### verbose! and trace! macros

The `verbose!` and `trace!` macros emit level-gated messages without allocating
the format string when the level is inactive:

```rust
use jt_consoleutils::{verbose, output::{Output, StringOutput}};

let mut out = StringOutput::new();
verbose!(out, "cache hit for {}", "deploy.tar.gz");
```

### Testing with StringOutput

`StringOutput` captures everything in an in-memory buffer — ideal for
assertions:

```rust
use jt_consoleutils::output::{Output, StringOutput};

let mut out = StringOutput::new();
out.writeln("step 1");
out.writeln("step 2");
assert_eq!(out.log(), "step 1\nstep 2\n");
```

---

## Shell

`Shell` abstracts process execution.  `ProcessShell` runs real commands with an
animated braille spinner overlay; `DryRunShell` announces what it would run;
`MockShell` records calls for unit tests.

Use `shell::create(dry_run)` for the common case:

```rust
use jt_consoleutils::output::{ConsoleOutput, OutputMode};
use jt_consoleutils::shell;

let mode = OutputMode::default();
let mut out = ConsoleOutput::new(mode);
let sh = shell::create(mode.is_dry_run());

let result = sh
    .run_command("Install deps", "npm", &["install"], &mut out, mode)
    .expect("spawn failed");

if !result.success {
    eprintln!("npm install failed:\n{}", result.stderr);
}
```

### Spinner overlay

In the default (non-verbose, non-quiet) mode, `run_command` renders an animated
spinner with a scrolling viewport of the last few lines of output — the same
polish you see in tools like `npm` or `cargo`.  In verbose mode the spinner is
replaced with line-by-line output prefixed with `>`.  In quiet mode, output is
collected silently.

### Convenience functions

Free functions delegate to real OS behavior regardless of which `Shell` is in
scope:

```rust
use jt_consoleutils::shell;

if shell::command_exists("git") {
    let tag = shell::command_output("git", &["describe", "--tags"])
        .unwrap_or_default();
    println!("latest tag: {tag}");
}
```

### MockShell for tests

```rust
use jt_consoleutils::shell::{MockShell, Shell};
use jt_consoleutils::output::{OutputMode, StringOutput};

let shell = MockShell::new(true, true, "v1.2.3".to_string());
let mode = OutputMode::default();
let mut out = StringOutput::new();

let result = shell
    .run_command("Deploy", "deploy.sh", &[], &mut out, mode)
    .unwrap();

assert!(result.success);
assert_eq!(shell.calls(), &["run_command: deploy.sh"]);
```

---

## JSON — zero dependencies

A complete JSON and JSONC parser, serializer, and value type with **no external
dependencies**.  No `serde`, no proc macros, no code generation — just
straightforward Rust.  This keeps compile times fast and binaries small.

### Parsing

```rust
use jt_consoleutils::json::{parse_json, parse_jsonc};

// Standard JSON
let val = parse_json(r#"{"name": "sr", "version": 2}"#).unwrap();
assert_eq!(val["name"], "sr");
assert_eq!(val["version"], 2);

// JSONC — comments and trailing commas
let config = parse_jsonc(r#"{
    // database settings
    "host": "localhost",
    "port": 5432,
    "tags": ["fast", "reliable",],  // trailing comma OK
}"#).unwrap();
assert_eq!(config["host"], "localhost");
assert_eq!(config["tags"][0], "fast");
```

Errors include line and column numbers for quick debugging:

```rust
use jt_consoleutils::json::parse_json;

let err = parse_json("{ \"key\": }").unwrap_err();
assert!(err.to_string().contains("line 1"));
```

### Serialization

```rust
use jt_consoleutils::json::{parse_json, to_json_pretty};

let val = parse_json(r#"{"b": 2, "a": 1}"#).unwrap();
let out = to_json_pretty(&val);
// Keys are sorted, 2-space indent:
// {
//   "a": 1,
//   "b": 2
// }
```

### JsonValue

`JsonValue` is the central type — a lightweight enum you can index, compare,
and destructure:

```rust
use jt_consoleutils::json::{parse_json, JsonValue};

let val = parse_json(r#"{"users": [{"name": "Alice"}]}"#).unwrap();

// Index chaining — missing keys return Null instead of panicking
assert_eq!(val["users"][0]["name"], "Alice");
assert_eq!(val["users"][99]["name"], JsonValue::Null);

// Convenience accessors
if let Some(name) = val["users"][0]["name"].as_str() {
    println!("First user: {name}");
}
```

### Typed deserialization

Implement `FromJsonValue` on your structs for type-safe extraction with clear
error messages:

```rust
use jt_consoleutils::json::*;

struct ServerConfig {
    host: String,
    port: Option<String>,
}

impl FromJsonValue for ServerConfig {
    fn from_json_value(value: &JsonValue) -> Result<Self, JsonError> {
        let map = expect_object(value, "ServerConfig")?;
        deny_unknown_fields(map, &["host", "port"], "ServerConfig")?;
        Ok(ServerConfig {
            host: require_string(map, "host", "ServerConfig")?,
            port: optional_string(map, "port", "ServerConfig")?,
        })
    }
}
```

### Typed serialization

Implement `ToJson` with `StructSerializer` to go directly from struct to JSON
string — no intermediate `JsonValue` needed:

```rust
use jt_consoleutils::json::{ToJson, StructSerializer};

struct Package { name: String, version: String }

impl ToJson for Package {
    fn to_json_pretty(&self) -> String {
        let mut s = StructSerializer::new();
        s.field_str("name", &self.name);
        s.field_str("version", &self.version);
        s.finish()
    }
}
```

### Deep merge and path removal

Utilities for working with configuration overlays:

```rust
use jt_consoleutils::json::{parse_json, json_deep_merge, json_remove_paths, to_json_pretty};

let mut base = parse_json(r#"{"a": 1, "nested": {"x": true}}"#).unwrap();
let overlay = parse_json(r#"{"nested": {"y": false}}"#).unwrap();

json_deep_merge(&mut base, &overlay);
// base is now {"a": 1, "nested": {"x": true, "y": false}}

json_remove_paths(&mut base, &[&["nested", "x"]]);
// base is now {"a": 1, "nested": {"y": false}}
```

---

## CLI parsing

A trait-based argument parser that handles the global flags every CLI needs —
`-v`/`--verbose`, `-q`/`--quiet`, `-d`/`--dry-run`, `-t`/`--trace`, `--help`,
and `--version` — then dispatches to your subcommand parser.

Implement `CommandParser` on your command enum:

```rust
use jt_consoleutils::cli::{CommandParser, CliError, ParsedCli, parse_cli, to_pargs};
use jt_consoleutils::output::OutputMode;

#[derive(Debug)]
enum Cmd {
    Build { release: bool },
    Test,
}

impl CommandParser for Cmd {
    fn subcommands() -> &'static [&'static str] { &["build", "test"] }

    fn parse(name: &str, args: &[String]) -> Result<Self, CliError> {
        match name {
            "build" => {
                let mut pargs = to_pargs(args);
                let release = pargs.contains("--release");
                Ok(Cmd::Build { release })
            }
            "test" => Ok(Cmd::Test),
            _ => unreachable!(),
        }
    }

    fn version() -> String { "myapp 1.0.0".into() }
    fn help_text() -> String { "Usage: myapp <command>".into() }
}

// In main():
// let parsed = parse_cli::<Cmd>().unwrap();
// parsed.mode is the resolved OutputMode (verbose/quiet/dry-run)
// parsed.command is your Cmd enum variant
```

The framework gives you:

- **Consistent flag behavior** across every tool built with it
- **Mutual exclusivity validation** (`--verbose` + `--quiet` is an error)
- **Rainbow-colorized help text** via `print_help`
- **`--` separator support** — flags after `--` pass through to subcommands
- **Default command fallback** — route unknown first args (e.g. file paths)
  through `default_command` instead of erroring

---

## Terminal and color utilities

### Rainbow colorization

Help text is automatically colorized with a left-to-right rainbow when printed
through `print_help`:

```rust
use jt_consoleutils::help::print_help;

// Wraps to terminal width, applies rainbow ANSI colors, exits 0
// print_help("Usage: myapp ...");
```

### ANSI color constants

```rust
use jt_consoleutils::colors::*;

println!("{GREEN}{BOLD}Success!{RESET}");
println!("{DIM}(completed in 1.2s){RESET}");
```

Available: `RESET`, `BOLD`, `DIM`, `RED`, `GREEN`, `YELLOW`, `CYAN`.

### Terminal width

```rust
use jt_consoleutils::terminal::terminal_width;

let width = terminal_width(); // returns 80 if undetermined
println!("Terminal is {width} columns wide");
```

---

## Utility helpers

### String utilities

```rust
use jt_consoleutils::str_utils::{format_bytes, plural, path_to_string};
use std::path::Path;

assert_eq!(format_bytes(1_572_864), "1.5 MB");
assert_eq!(format!("3 file{}", plural(3)), "3 files");
assert_eq!(path_to_string(Path::new("/tmp/out")), "/tmp/out");
```

### Filesystem utilities

```rust
use jt_consoleutils::fs_utils::{same_file, same_content, make_executable};
use std::path::Path;

// Compare paths (resolves symlinks)
let equal = same_file(Path::new("a.txt"), Path::new("./a.txt"));

// Compare content (short-circuits on size mismatch)
let identical = same_content(Path::new("a.txt"), Path::new("b.txt"));

// Set +x on Unix, no-op on Windows
make_executable(Path::new("script.sh")).unwrap();
```

---

## Feature flags

| Feature | Default | Description |
|---|---|---|
| `verbose` | off | Enables `LogLevel::Verbose`, verbose output methods, the `verbose!` macro, and `-v`/`--verbose` CLI flag |
| `trace` | off | Enables `LogLevel::Trace`, trace output methods, the `trace!` macro, `-t`/`--trace` CLI flag, and `format_trace_block` |
| `build-support` | off | Exposes `build_support::emit_build_info()` for use in `build.rs` |

### build-support

Inject a build date and git commit hash into your binary at compile time:

```toml
[build-dependencies]
jt-consoleutils = { version = "0.2", features = ["build-support"] }
```

```rust
// build.rs
fn main() {
    jt_consoleutils::build_support::emit_build_info();
}
```

```rust
// main.rs
const BUILD_DATE: &str = env!("BUILD_DATE");
const GIT_HASH:   &str = env!("GIT_HASH");

fn version() -> String {
    jt_consoleutils::version::version_string(BUILD_DATE, GIT_HASH)
    // => "2026-04-12 (a1b2c3d)"
}
```

---

## Modules at a glance

| Module | What it provides |
|---|---|
| `output` | `Output` trait, `ConsoleOutput`, `StringOutput`, `OutputMode`, `LogLevel` |
| `shell` | `Shell` trait, `ProcessShell`, `DryRunShell`, `MockShell`, `CommandResult`, animated spinner |
| `json` | `JsonValue`, `parse_json`, `parse_jsonc`, `to_json_pretty`, `FromJsonValue`, `ToJson`, `StructSerializer`, merge/remove ops |
| `cli` | `CommandParser` trait, `parse_cli`, `ParsedCli`, `CliError`, global flag handling |
| `colorize` | Rainbow ANSI colorizer for terminal output |
| `colors` | ANSI escape constants (`RESET`, `BOLD`, `DIM`, `RED`, `GREEN`, `YELLOW`, `CYAN`) |
| `terminal` | Terminal width detection |
| `help` | Rainbow help-text printing, word-wrapping, version printing |
| `str_utils` | `format_bytes`, `plural`, `path_to_string`, `format_trace_block` |
| `fs_utils` | `same_file`, `same_content`, `make_executable`, `remove_symlink_dir_like` |
| `version` | `version_string(build_date, git_hash)` formatter |
| `build_support` | *(feature-gated)* `emit_build_info()` for `build.rs` |

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
