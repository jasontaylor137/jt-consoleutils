# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.1.0] — 2025-07-11

### Added

- `Output` trait and `ConsoleOutput` implementation for abstracting stdout/stderr
  writes, with `OutputMode` enum (`Normal` / `Quiet`) to suppress output in
  non-interactive contexts.
- `Shell` trait and `ProcessShell` implementation for running external commands,
  with `ShellConfig` (working directory, environment overrides) and
  `CommandResult` (exit code, stdout, stderr capture).
- `ShellError` error type (via `thiserror`) covering command-not-found, non-zero
  exit, I/O failure, and UTF-8 decode errors.
- `version` module with `version_string` helper for formatting build-time
  `BUILD_DATE` / `GIT_HASH` env vars into a human-readable version string.
- `format_bytes` utility for rendering byte counts as human-readable strings
  (B, KB, MB, GB, TB).
- `build-support` feature flag that exposes a `build.rs` helper for injecting
  `BUILD_DATE` and `GIT_HASH` at compile time.
- Full `///` doc comments on all public items; `#![warn(missing_docs)]` enforced
  in `lib.rs`.
- MIT OR Apache-2.0 dual license.

[0.1.0]: https://github.com/jasontaylor137/jt-consoleutils/releases/tag/v0.1.0