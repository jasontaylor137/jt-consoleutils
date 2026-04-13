# CLAUDE.md — Project Rules for jt-consoleutils

## Overview

**Public crate.** General-purpose CLI utility library written in Rust (edition 2024). Published to [crates.io](https://crates.io) and intended for use by any Rust CLI project, not just internal tools.

Provides terminal output abstractions, ANSI color helpers, a rainbow colorizer, terminal width detection, a full shell execution abstraction with spinner overlay rendering, cross-platform filesystem helpers, and general string/path utilities. Currently consumed by `sr` and `filebydaterust`.

### Public API contract

Because this is a published crate, **all `pub` items are part of the public API**:

- Prefer additive changes; avoid removing or renaming `pub` items without a semver major bump
- New modules must be deliberately `pub` — do not expose internal helpers by accident
- Keep doc comments on all public items; they appear on docs.rs

## Build & Test

- `cargo build` / `cargo test`
- `cargo run --example test_shell` — visual demo of all shell output modes and spinner behaviors
- `cargo publish --dry-run` — verify the crate packages cleanly before a real publish

### Feature flags

| Flag | Effect |
|---|---|
| `verbose` | Enables `LogLevel::Verbose`, `Output::is_verbose()`, `Output::emit_verbose()`, `Output::shell_command()`, `Output::shell_line()`, `Output::log_exec()`, and the `verbose!` macro |
| `trace` | Enables `LogLevel::Trace`, `Output::is_trace()`, `Output::emit_trace()`, the `trace!` macro, and `str_utils::format_trace_block` |
| `build-support` | Exposes `build_support::emit_build_info()` for use in `build.rs` |

## Releasing

Run `scripts/release.sh <version>` (e.g. `scripts/release.sh 0.2.0`). The script runs all checks, bumps the version, drafts a changelog from commit messages, opens `$EDITOR` for you to polish it, then commits, tags, pushes, and publishes to crates.io.

## Architecture

### Modules

- `colors.rs` — ANSI escape constants (`RESET`, `BOLD`, `DIM`, `RED`, `GREEN`, `YELLOW`, `CYAN`)
- `colorize.rs` — left-to-right rainbow colorizer (`colorize_text_with_width(text, Option<usize>) -> String`); emits 24-bit ANSI foreground escapes
- `terminal.rs` — terminal width detection (`terminal_width() -> usize`; returns 80 if undetermined)
- `output.rs` — `Output` trait, `OutputMode`, `LogLevel`, `ConsoleOutput` (production), `StringOutput` (test helper); `verbose!` and `trace!` macros
- `help.rs` — help text formatting: `wrap_help_text(text, width) -> String`, `print_help(text) -> !`, `print_version(version_str) -> !`
- `shell/` — shell execution abstraction: `Shell` trait, `ProcessShell`, `DryRunShell`, `MockShell`, `CommandResult`, `ShellError`, `ShellConfig`; internal `exec.rs` (three output modes), `overlay.rs` (spinner frame rendering), `scripted.rs` (`ScriptedShell` for visual regression tests)
- `str_utils.rs` — general string/path utilities: `path_to_string`, `plural`, `format_bytes`, `format_trace_block` (feature-gated on `trace`)
- `fs_utils.rs` — cross-platform filesystem helpers: `same_file`, `same_content`, `make_executable`, `remove_symlink_dir_like`
- `version.rs` — shared version string formatting: `version_string(build_date, git_hash) -> String`
- `json/` — zero-dependency JSON/JSONC parser, serializer, and value type: `JsonValue`, `parse_json`, `parse_jsonc`, `to_json_pretty`, `FromJsonValue` trait with deserialization helpers, `ToJson` trait with `StructSerializer`, `json_deep_merge`, `json_remove_paths`
- `cli/` — CLI parsing framework: `CommandParser` trait, `parse_cli`, global flag extraction (`-v`/`-q`/`-d`/`-t`), `--help`/`--version` handling, subcommand dispatch; uses `pico-args` for subcommand flag parsing

### `Output` trait (`output.rs`)

The primary output abstraction. Implementations:

| Type | Purpose |
|---|---|
| `ConsoleOutput` | Production — writes to stdout, respects quiet/verbose/dry-run modes |
| `StringOutput` | Tests — captures all output in an in-memory buffer; `log() -> &str` |

Key methods:

| Method | Description |
|---|---|
| `writeln(&str)` | Print a line (suppressed in quiet mode) |
| `write(&str)` | Print without newline, flushing stdout |
| `is_verbose() -> bool` | Whether verbose output is active (feature: `verbose`) |
| `emit_verbose(String)` | Emit a verbose-level message (feature: `verbose`) |
| `is_trace() -> bool` | Whether trace output is active (feature: `trace`) |
| `emit_trace(String)` | Emit a trace-level message (feature: `trace`) |
| `shell_command(&str)` | Log a shell command with `> ` prefix (feature: `verbose`) |
| `shell_line(&str)` | Log a line of shell output with `> ` prefix (feature: `verbose`) |
| `step_result(label, success, elapsed_ms, viewport)` | Print `✓`/`✗` step summary with elapsed time |
| `dry_run_shell(&str)` | Announce a command that would run (dry-run mode) |
| `dry_run_write(&str)` | Announce a file that would be written |
| `dry_run_delete(&str)` | Announce a file/dir that would be deleted |
| `log_exec(cmd)` | Format and emit a `std::process::Command` via verbose (feature: `verbose`) |

Convenience macros (defined in `lib.rs`):

| Macro | Description |
|---|---|
| `verbose!(output, ...)` | Emit verbose message when feature and mode are active |
| `trace!(output, ...)` | Emit trace message when feature and mode are active |

### `OutputMode` and `LogLevel` (`output.rs`)

```rust
pub enum LogLevel { Quiet, Normal, Verbose, Trace }

pub struct OutputMode {
    pub level: LogLevel,
    pub dry_run: bool,
}
```

`Verbose` and `Trace` variants are conditionally compiled behind their respective feature flags. `OutputMode` provides convenience methods: `is_verbose()`, `is_quiet()`, `is_trace()`, `is_dry_run()`.

Passed through the call stack so each layer can adapt its behavior without global state.

### `Shell` trait (`shell/mod.rs`)

Execution abstraction for running external commands. Implementations:

| Type | Purpose |
|---|---|
| `ProcessShell` | Production — spawns real processes |
| `DryRunShell` | Logs what would run via `output.dry_run_shell()`; delegates `command_exists`/`command_output` to the real OS |
| `MockShell` | Tests — records calls for assertion; configurable return values and a per-call `exec_capture` result queue |
| `ScriptedShell` | Visual regression tests — drives the real spinner with scripted stdout/stderr events |

Key trait methods:

| Method | Description |
|---|---|
| `run_command(label, program, args, output, mode)` | Run a command with mode-appropriate output (spinner / verbose / quiet) |
| `shell_exec(script, output, mode)` | Run a shell string via `bash -c` (Unix) or `powershell -Command` (Windows) |
| `command_exists(name)` | Return whether the named binary is on `PATH` |
| `command_output(program, args)` | Capture stdout of a command as a `String` |
| `exec_capture(cmd, output, mode)` | Run a command and return `CommandResult` |
| `exec_interactive(cmd, output, mode)` | Run a command with inherited stdio (interactive prompts) |

`CommandResult` has fields `success: bool`, `code: Option<i32>`, `stderr: String` and methods `require_success(cmd) -> Result<(), ShellError>` and `check(err_fn) -> Result<(), E>` for ergonomic error handling.

`create(dry_run: bool) -> Box<dyn Shell>` is a factory that returns `DryRunShell` when dry-run and `ProcessShell` otherwise.

### Spinner overlay (`shell/overlay.rs`, `shell/exec.rs`)

The default (non-verbose, non-quiet) output mode renders an animated braille spinner with a scrolling viewport of the last N lines of output. Key internals:

- `render_frame` — erase previous frame, draw spinner + viewport lines truncated to terminal width
- `clear_lines` — move cursor up and clear each line
- `truncate_visible` — truncate a string to a visible column count (ANSI-escape-aware)
- `render_overlay_lines` — expand viewport slots containing embedded `\r` into multiple visual rows
- `Line` enum — `Stdout(String)`, `StdoutCr(String)` (overwrites current slot in place), `Stderr(String)`

`ShellConfig.viewport_size` controls how many lines are visible in the spinner overlay (default: 5).

### Free functions (`shell/mod.rs`)

These delegate to real OS behavior regardless of which `Shell` implementation is in use:

- `command_exists(name: &str) -> bool`
- `command_output(program: &str, args: &[&str]) -> Result<String, ShellError>`
- `shell_exec(script: &str, output, mode, viewport_size) -> Result<CommandResult, ShellError>`
- `run_command(label, program, args, output, mode, viewport_size) -> Result<CommandResult, ShellError>`
- `run_passthrough(program, args, output, mode) -> Result<CommandResult, ShellError>`
- `command_parts(cmd: &Command) -> (String, Vec<String>)`

### JSON module (`json/`)

Zero-dependency JSON/JSONC handling. Key components:

| File | Purpose |
|---|---|
| `value.rs` | `JsonValue` enum — `Object(BTreeMap)`, `Array`, `String`, `Number(f64)`, `Bool`, `Null`; index operators, `PartialEq` with primitives |
| `parser.rs` | `parse_json`, `parse_jsonc` (with `//`, `/* */` comments and trailing commas); line/column error positions; UTF-16 surrogate pair support |
| `serialize.rs` | `to_json_pretty` — 2-space-indented pretty-printer; integers without decimal point; non-finite → `null` |
| `deserialize.rs` | `FromJsonValue` trait and helpers (`require_string`, `optional_bool`, `optional_nested`, `deny_unknown_fields`, etc.) |
| `to_json.rs` | `ToJson` trait and `StructSerializer` builder for direct struct → JSON string serialization (skips intermediate `JsonValue`) |
| `ops.rs` | `json_deep_merge`, `json_remove_paths` (with empty-parent pruning) |
| `escape.rs` | Shared JSON string escaping used by both `serialize.rs` and `to_json.rs` |
| `error.rs` | `JsonError` — `Parse { line, col, msg }`, `Value(String)`, `Io(io::Error)` |

### CLI module (`cli/`)

Trait-based CLI argument parsing with global flag handling and subcommand dispatch.

Apps implement `CommandParser` on their `Command` enum:

| Method | Description |
|---|---|
| `subcommands()` | List of recognized subcommand names |
| `parse(name, args)` | Parse a recognized subcommand with its args |
| `default_command(first_arg, rest)` | Optional fallback for unknown first args (e.g. treat as file path) |
| `version()` | Version string for `--version` |
| `help_text()` | Main help text for `--help` |
| `command_help(cmd, args)` | Per-subcommand help text |

`parse_cli<C>()` extracts global flags (`-v`/`--verbose`, `-q`/`--quiet`, `-d`/`--dry-run`, `-t`/`--trace`), validates mutual exclusivity, handles `--help`/`--version`, and dispatches to the app's parser. Returns `ParsedCli<C>` with the resolved `OutputMode` and parsed command.

`to_pargs(args)` converts `&[String]` to `pico_args::Arguments` for subcommand-level flag parsing.

## Code Style

- Prefer small, single-responsibility modules — each module should do one thing well
- Keep functions short and focused; extract helpers when a function grows beyond a single concern
- Avoid over-engineering — only build what's needed now
- **Imports over qualified paths**: if a path prefix appears more than once in a file, add a `use` statement; a single usage can stay qualified
- **No bare boolean parameters**: avoid passing `bool` values as function arguments unless the function name makes the intent unambiguous. Use a named enum instead (e.g. `Force::Yes`/`Force::No`)

## Testing

- Use the **Given / When / Then** pattern for all tests:
  ```rust
  #[test]
  fn descriptive_test_name() {
      // Given
      let mut out = StringOutput::new();

      // When
      out.writeln("hello");

      // Then
      assert_eq!(out.log(), "hello\n");
  }
  ```
- Aim for thorough test coverage — test happy paths, edge cases, and error conditions
- Use `rstest` for parameterized tests when testing multiple inputs/outputs
- Use `StringOutput` for unit tests that exercise output behavior without touching stdout

## Error Handling

- Use the `thiserror` crate for defining error types
- Each module that can fail should define its own error enum with `#[derive(Debug, thiserror::Error)]`
- Use `#[error("...")]` for human-readable display messages
- Use `#[from]` to enable automatic conversion from underlying errors

## Dependencies

- `terminal_size` for terminal width detection
- `thiserror` for error types
- `pico-args` for subcommand flag parsing in the `cli` module
- Dev: `rstest`, `tempfile`

### Dependency policy

This is a public crate — keep the dependency footprint small and deliberate:

- Prefer `std` over adding a new dependency for small utilities
- Every new dependency increases compile time and supply-chain surface for all downstream consumers; justify additions in the PR/commit
- Dev-only dependencies (behind `[dev-dependencies]`) are fine to add freely

## What Belongs Here vs. Elsewhere

This library owns **display and execution abstractions** with no domain coupling. The boundary rules are:

| Belongs here | Belongs in consuming crate |
|---|---|
| `Output` trait and implementations | Domain-specific output formatting |
| `Shell` trait and implementations | Domain-specific command sequences |
| Terminal/color/spinner primitives | App-specific path resolution, config loading |
| General `str_utils` / `fs_utils` helpers (e.g. `plural`, `format_bytes`, `same_file`) | Domain models and business logic |

See `plans/to-move-here.md` for a prioritized list of candidates to migrate from `sr` and `filebydaterust`.

## Planning

When implementing work tracked in a plan file (e.g., `plans/to-move-here.md`), remove each item from the file immediately after completing it. Do not leave completed items in the plan.

## Clarifying Questions

Before starting any non-trivial task, ask clarifying questions if the request is ambiguous or underspecified. Prefer a short, focused question over making assumptions that could lead to rework. Examples of when to ask:

- The scope is unclear (e.g., "add tests" — which module? all modules?)
- Multiple valid approaches exist with meaningfully different trade-offs
- A destructive or hard-to-reverse action is implied

Keep questions concise — one or two targeted questions, not an exhaustive checklist.