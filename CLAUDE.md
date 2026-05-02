# CLAUDE.md — Project Rules for jt-consoleutils

## Overview

Public crate, edition 2024. General-purpose CLI utility library on
[crates.io](https://crates.io/crates/jt-consoleutils). Currently consumed by
`sr` and `filebydaterust`, but designed for arbitrary downstream use.

For module/API reference, see
[docs.rs](https://docs.rs/jt-consoleutils) — that is the source of truth.
Don't restate the public API here; doing so creates drift.

### Public API contract

Every `pub` item is part of the public API:

- Prefer additive changes; removing or renaming `pub` items requires a semver
  major bump.
- New modules must be deliberately `pub` — don't expose internal helpers by
  accident.
- Every public item needs a doc comment (`#![warn(missing_docs)]` is on).
- `cargo doc --no-deps --all-features` must produce zero warnings.

## Build & Test

- `cargo build` / `cargo test`
- `cargo run --example test_shell` — visual demo of all shell output modes
  and spinner behaviors
- `cargo doc --no-deps --all-features` — must be warning-free
- `cargo publish --dry-run` — verify the crate packages cleanly

### Feature flags

| Flag | Effect |
|---|---|
| `verbose` | `LogLevel::Verbose`, the `verbose!` macro, `-v`/`--verbose` CLI flag, and verbose-only methods |
| `trace` | `LogLevel::Trace`, the `trace!` macro, `-t`/`--trace` CLI flag, and `format_trace_block` |
| `dotenv` | The `dotenv` module (depends on `dotenvy`) |
| `build-support` | `build_support::emit_build_info()` for use in `build.rs` |

## Releasing

`scripts/release.sh <version>` runs all checks, bumps the version, drafts a
changelog from commit messages, opens `$EDITOR` for polish, then commits,
tags, pushes, and publishes to crates.io.

## Code Style

- Small, single-responsibility modules; short, focused functions.
- **Imports over qualified paths.** If a path prefix appears more than once
  in a file, add a `use` statement.
- **No bare boolean parameters.** Avoid passing `bool` arguments unless the
  function name makes intent unambiguous; prefer a named enum
  (e.g., `Force::Yes`/`Force::No`).
- Avoid over-engineering — only build what's needed now.

## Testing

- Use the **Given / When / Then** pattern:

  ```rust
  #[test]
  fn descriptive_name() {
      // Given
      let mut out = StringOutput::new();

      // When
      out.writeln("hello");

      // Then
      assert_eq!(out.log(), "hello\n");
  }
  ```

- Cover happy paths, edge cases, and error conditions.
- Use `rstest` for parameterized cases.
- Use `StringOutput` to assert on output without touching stdout.

## Error Handling

- Use `thiserror` for error types.
- Each module that can fail defines its own error enum with
  `#[derive(Debug, thiserror::Error)]`.
- Use `#[error("...")]` for human-readable display messages and `#[from]`
  for automatic conversion from underlying errors.

## Dependencies

Current runtime deps: `terminal_size`, `thiserror`, `pico-args`, plus
platform-specific `libc` (Unix) and `windows-sys` (Windows). Optional:
`dotenvy` (`dotenv` feature). Dev: `rstest`, `tempfile`.

### Dependency policy

- Prefer `std` over a new dependency for small utilities.
- Every new runtime dependency increases compile time and supply-chain
  surface for all downstream consumers; justify additions in the commit.
- Dev-only dependencies are fine to add freely.

## What Belongs Here vs. Elsewhere

This library owns **display and execution abstractions** with no domain
coupling. The boundary rules:

| Belongs here | Belongs in consuming crate |
|---|---|
| `Output` / `Shell` traits and implementations | Domain-specific output formatting and command sequences |
| Terminal/color/spinner primitives | App-specific path resolution, config loading |
| General `str_utils` / `fs_utils` helpers (e.g. `plural`, `format_bytes`, `same_file`) | Domain models and business logic |

If you find yourself adding a default value or fallback string that hints at
a specific consuming project (e.g. a TS file extension, a tool name),
that's a sign the helper has leaked too far up — keep the helper neutral
and let the consumer supply the domain term.

## Clarifying Questions

Before starting a non-trivial task, ask one or two focused questions if the
request is ambiguous, multiple approaches have meaningfully different
trade-offs, or a destructive action is implied. Prefer a short question
over an assumption that leads to rework.
