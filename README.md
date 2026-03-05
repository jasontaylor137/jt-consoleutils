# jt-consoleutils

[![Crates.io](https://img.shields.io/crates/v/jt-consoleutils.svg)](https://crates.io/crates/jt-consoleutils)
[![docs.rs](https://docs.rs/jt-consoleutils/badge.svg)](https://docs.rs/jt-consoleutils)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/jasontaylor137/jt-consoleutils#license)

Scaffolding and helpers for CLI tools — output abstractions, shell execution,
terminal utilities, colorization, and optional build-time metadata support.

---

## Overview

`jt-consoleutils` provides a small set of focused building blocks for writing
Rust CLI tools:

| Module | What it provides |
|---|---|
| `output` | `Output` trait + `ConsoleOutput`, `StringOutput`, `OutputMode` |
| `shell` | `Shell` trait + `ProcessShell`, `DryRunShell`, `MockShell`, `create()` |
| `colorize` | Rainbow ANSI colorization helper |
| `colors` | Common ANSI color/reset escape constants |
| `terminal` | Terminal width detection |
| `str_utils` | String wrapping and padding helpers |
| `fs_utils` | Filesystem helpers (recursive copy, etc.) |
| `help` | Formatted help-text rendering |
| `version` | `version_string(build_date, git_hash)` formatter |
| `build_support` | *(feature-gated)* `emit_build_info()` for `build.rs` |

---

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
jt-consoleutils = "0.1"
```

---

## Output

`Output` is a trait that abstracts where CLI output goes. `ConsoleOutput` writes
to stdout/stderr; `StringOutput` captures output in memory (useful in tests).
Both respect `OutputMode`, which carries `verbose`, `quiet`, and `dry_run` flags.

```rust
use jt_consoleutils::output::{ConsoleOutput, Output, OutputMode};

fn main() {
    let mode = OutputMode { verbose: true, ..Default::default() };
    let mut out = ConsoleOutput::new(mode);

    out.writeln("Starting deployment…");
    out.verbose(Box::new(|| "Loaded config from ~/.config/app.toml".to_string()));
    out.step_result("Build", true, 1_420, &[]);
}
```

Use `StringOutput` in tests to assert on exactly what was written:

```rust
use jt_consoleutils::output::{Output, OutputMode, StringOutput};

let mut out = StringOutput::new();
out.writeln("hello");
out.writeln("world");
assert_eq!(out.log(), "hello\nworld\n");
```

### OutputMode

```rust
use jt_consoleutils::output::OutputMode;

// Verbose mode: extra diagnostic lines are printed.
let verbose = OutputMode { verbose: true, ..Default::default() };

// Quiet mode: only errors are shown.
let quiet = OutputMode { quiet: true, ..Default::default() };

// Dry-run mode: commands are announced but not executed.
let dry_run = OutputMode { dry_run: true, ..Default::default() };
```

---

## Shell

`Shell` is a trait that abstracts process execution. `ProcessShell` runs real
commands; `DryRunShell` announces what it would run without executing anything;
`MockShell` records calls and returns configurable results — ideal for unit tests.

Use `create(dry_run)` for the common case:

```rust
use jt_consoleutils::{
    output::{ConsoleOutput, OutputMode},
    shell::create,
};

fn main() {
    let mode = OutputMode::default();
    let mut out = ConsoleOutput::new(mode);
    let shell = create(false); // true => DryRunShell

    let result = shell
        .run_command("Install deps", "npm", &["install"], &mut out, mode)
        .expect("shell error");

    if !result.success {
        eprintln!("npm install failed:\n{}", result.stderr);
    }
}
```

Construct a `ProcessShell` directly if you need custom config:

```rust
use jt_consoleutils::shell::{ProcessShell, Shell, ShellConfig};
use jt_consoleutils::output::{ConsoleOutput, OutputMode};

let shell = ProcessShell { config: ShellConfig { viewport_size: 10 } };
let mode  = OutputMode::default();
let mut out = ConsoleOutput::new(mode);

let exists = shell.command_exists("git");
let tag    = shell.command_output("git", &["describe", "--tags"]);
```

Use `MockShell` in tests:

```rust
use jt_consoleutils::shell::{MockShell, Shell};
use jt_consoleutils::output::{OutputMode, StringOutput};

let mut shell = MockShell::new(true, true, "v1.2.3".to_string());
let mode = OutputMode::default();
let mut out = StringOutput::new();

let result = shell
    .run_command("Deploy", "deploy.sh", &[], &mut out, mode)
    .unwrap();

assert!(result.success);
assert_eq!(shell.calls(), &["run_command: deploy.sh"]);
```

---

## Feature Flags

| Feature | Default | Description |
|---|---|---|
| `build-support` | off | Enables `build_support::emit_build_info()` for use in `build.rs` |

### `build-support`

Inject a build date and git commit hash into your binary at compile time:

```toml
# In your project's Cargo.toml
[build-dependencies]
jt-consoleutils = { version = "0.1", features = ["build-support"] }
```

```rust
// In your project's build.rs
fn main() {
    jt_consoleutils::build_support::emit_build_info();
}
```

```rust
// In your application code
const BUILD_DATE: &str = env!("BUILD_DATE");
const GIT_HASH:   &str = env!("GIT_HASH");

fn version() -> String {
    jt_consoleutils::version::version_string(BUILD_DATE, GIT_HASH)
    // => "2026-03-05 (a1b2c3d)"
}
```

`BUILD_DATE` is computed from the system clock (no external crates) and
`GIT_HASH` is the short commit hash from `git rev-parse --short HEAD`, or
`"unknown"` if git is unavailable.

---

## MSRV

The minimum supported Rust version is **1.85** (Rust 2024 edition).

---

## License

Licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.