# Candidates to Move into jt-consoleutils

Analysis of code in `filebydaterust` and `vr` that could reasonably be
extracted into this general-purpose CLI utility library.

---

## Current contents of jt-consoleutils

| Module | What it provides |
|---|---|
| `colors` | ANSI color/style constants (`RED`, `GREEN`, `BOLD`, `RESET`, …) |
| `colorize` | Left-to-right rainbow colorizer (`colorize_text_with_width`) |
| `terminal` | Terminal width detection (`terminal_width()`) |
| `output` | `Output` trait, `ConsoleOutput`, `StringOutput`, `OutputMode` |
| `str_utils` | `plural`, `path_to_string`, `format_bytes` |
| `fs_utils` | `same_file`, `same_content`, `make_executable`, `remove_symlink_dir_like` |
| `shell` | `Shell` trait, `ProcessShell`, `DryRunShell`, `MockShell`, `ScriptedShell`, spinner overlay rendering |

---

## Weak candidates (probably leave in place for now)

### `expand_env_vars` — from `vr::env`

Expands `${ENV_VAR}` patterns from the host environment. Generic enough
conceptually but only consumed by `vr`'s registry and run logic. No second
use case yet justifies the move.

---

### `print_help` / colorized help pattern

Both `vr` and `filebydaterust` share the same pattern: build a help string →
`terminal_width()` → `colorize_text_with_width` → `process::exit(0)`. A
`print_help(text: &str) -> !` convenience function here would eliminate that
repetition, but it is currently a one-liner wrapper around two calls that
already live in this crate. Worth doing if a third tool follows the same
pattern.

---

## Leave in their crates

| Code | Reason |
|---|---|
| `vr::env::expand_env_vars` | Generic but single consumer today |
| `vr::paths` | `vr`-specific path resolution (script discovery, `~/.vr/`, etc.) |
| `vr::test_utils` (`TestEnv`, `MetadataBuilder`, …) | `vr`-specific test scaffolding |
| `filebydaterust::processor::*` | Fully domain-specific (date-path parsing, CRC deduplication, etc.) |
| `filebydaterust::processor::progress::Progress` | Works against the `Output` trait but is tied to `filebydaterust`'s terminology and progress model |

### Note on `vr::shell` landing in this crate

The `Shell` trait and its implementations were originally flagged as a future
`jt-shell` crate, but were extracted here directly. The `(program, args)`
calling convention is still baked into the trait's `run_command` signature.
A more general design would accept a `std::process::Command` directly —
callers construct the command however they like, and the `Shell` implementation
only decides *how* to run it (live, dry-run, captured, interactive). This
would decouple the trait from the string-pair convention and make it usable for
tools that need environment variables, working directories, or other `Command`
builder options that the current API cannot express without adding more
parameters.

This is worth revisiting when a third tool adopts this crate.

---

## Priority order

| Priority | Item | Source |
|---|---|---|
| Low | `expand_env_vars` | `vr::env` — generic but single consumer today |
| Low | `print_help` wrapper | both crates — worth doing when a third tool appears |
| Consider | `Shell` trait — `Command`-based API | trait still takes `(program, args)`; revisit when a third tool needs env vars or working dirs |