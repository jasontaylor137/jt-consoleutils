# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added

- `terminal::enable_ansi()` prepares the Windows console for the ANSI escapes
  and non-ASCII text this crate emits: it enables virtual-terminal processing
  on stdout and sets the console output codepage to UTF-8 (65001). A no-op on
  every other platform, so callers invoke it unconditionally at startup.
  Previously every consumer had to hand-roll the `GetConsoleMode` /
  `SetConsoleMode` / `SetConsoleOutputCP` dance — and take a `windows-sys`
  `Win32_System_Console` dependency to do it.

- `fs_utils::TempFile`, a uniquely-named file that is deleted when dropped:
  created exclusively (never opening or truncating an existing name) and `0600`
  on Unix, with `persist` to rename it over a destination. Implemented on `std`
  alone — no new runtime dependency for downstream consumers.

- `fs_utils::atomic_write` and `fs_utils::atomic_write_keep_perms` stage a
  sibling `.<name>.tmp.<unique>` file and rename it over the destination, so a
  reader never sees a half-written file. The `keep_perms` form carries the
  original's mode over to the replacement and returns a `PermsCopy` saying
  whether that succeeded — a rewrite of a credentials file must never widen its
  mode. A symlinked destination is written *through* rather than replaced, and
  a rename that Windows refuses because a scanner or indexer still holds the
  destination open is retried over ~170 ms before giving up.

### Changed

- **Behavioral:** `fs_utils::write_if_changed` (and `dry::dry_write_if_changed`
  through it) now writes atomically via `atomic_write_keep_perms` rather than
  `std::fs::write`. Three consequences: writes need permission on the
  containing directory; a destination that is a symlink is followed rather than
  replaced; and a read-only destination is overwritten rather than refused,
  since a rename answers to the directory's permissions rather than the file's.

### Fixed

## [0.7.1] — 2026-06-27

### Added

- `MockShell` gained a `run_command_results` queue and a `push_run` method, so
  tests can script a sequence of `run_command` outcomes — including captured
  stderr, which the `run_success` flag alone cannot express.

## [0.7.0] — 2026-06-17

A consolidation release that trims unused public surface and tightens the
output traits. Several `pub` items were removed or moved; see the breaking
notes below.

### Added

- `shell::format_command` is now public, for rendering a command line the same
  way the shell runner displays it.

### Changed

- **Breaking:** the `Output` trait was narrowed to terminal/quiet/verbose
  capability queries. The action-emitting methods — `state`, `hint`, `section`,
  `item`, `warn`, and `error` — now live on the `OutputAction` trait. Callers
  that invoked these on an `Output` value must bring `OutputAction` into scope.
  Quiet-mode suppression is now applied at a single seam rather than per method.
- **Breaking:** `signals` no longer re-exports its items at the module root.
  Use the now-public submodules instead: `signals::interrupt::{install_interrupt_handler,
  is_interrupted, reset_interrupt}` and `signals::parent::{install_parent_handlers,
  SigintDefaultGuard}`.
- **Breaking:** `fs_utils::make_executable` now returns `Result<(), FsError>`
  with path context on failure, matching `restrict_permissions`.
- **Breaking:** `cli::extract_global_flags` now returns
  `Result<(OutputMode, Vec<_>), CliError>` — it validates conflicting flags and
  builds the `OutputMode` for you instead of returning raw flags.
- **Breaking:** the cfg-gated `Dim` enum was replaced with named dim-prefix
  functions.
- The `JsonMap` object backing is now an insertion-ordered `Vec` instead of a
  `BTreeMap`, so object keys serialize in insertion order. Also trims ~7.7 KB
  off the built crate.
- Internal: size scanning swaps `std::sync::mpsc` for a minimal line queue, and
  the JSON / JSONC file-read paths share one parse core.

### Removed

- **Breaking:** the unused public `Spinner` type from `terminal`.
- **Breaking:** `shell::run_passthrough` (superseded by the standard run path;
  use `format_command` for display).
- **Breaking:** `Progress::finish` — it was an exact alias of `Progress::clear`;
  call `clear` instead.
- **Breaking:** `StructSerializer::field_array` and `field_array_str`.
- **Breaking:** the `Trailing::PrepCustom` variant.

### Fixed

- Build warning when the `trace` feature is disabled.
- Broken intra-doc links left by the `Output` trait narrowing, so
  `cargo doc --all-features` is warning-free again.

## [0.6.0] — 2026-06-06

### Added

- Span-aware, comment-preserving JSONC editor in the `json` module: `jsonc_get`, `jsonc_set`, `jsonc_unset`, and the `EditError` type. The editor navigates only the addressed path over the raw source bytes and splices the single addressed span, so comments, key order, and all formatting outside that span are preserved byte-for-byte.
  - `jsonc_get(src, path)` returns the raw value slice for a path (scalars come back with their quotes), or `None` if absent.
  - `jsonc_set(src, path, value)` replaces an existing value, appends a new member, or synthesizes the missing parent-object chain — detecting the file's indentation unit (spaces/tabs) and re-indenting inserted `JsonValue` fragments to match. An empty or blank source starts from `{}`.
  - `jsonc_unset(src, path)` removes a member surgically: it never prunes a parent left empty, keeps comment lines above the removed key, and correctly fixes up commas (including the hard case where the previous member's comma sits after a comment).
  - Schema-awareness, value typing, and validation are left to the caller: paths arrive pre-split into object-key segments and values arrive as `JsonValue`.

### Changed

- Extracted the low-level JSONC byte primitives (`scan_string`, `comment_len`, `skip_trivia`) into a shared `scan` module. The `strip_jsonc` comment-stripping pre-pass in the parser now reuses these instead of carrying its own duplicated string/comment scanning logic.
- **Breaking:** the `output::action::Trailing` enum no longer has a `Count` variant, and `output::render::render_action` takes an additional `count: Option<&str>` argument. Count phrases are now tracked independently of the trailing preposition (see Fixed). Downstream code that constructed `Trailing::Count` or called `render_action` directly must be updated; consumers using `ActionBuilder` (`.count(...)`, `.to(...)`, `.from(...)`) are unaffected at the source level.

### Fixed

- Action summaries now compose a count with a `to`/`from` target instead of one clobbering the other: `Removed 2 deps from script.hs` renders correctly, where previously the count and the preposition could not both appear. The count is held separately from the trailing preposition and rendered between the verb and the trailing target.
- Silenced unused-import warnings in `shell` and `signals` tests on Windows by gating Unix-only test imports behind `#[cfg(unix)]`.

## [0.5.3] — 2026-05-25

### Added

- `MockShell::mark_missing(program)` — marks a single program as absent so `command_exists(program)` returns `false` even when the global `command_exists_result` flag is `true`. Lets a test model a partial PATH (e.g. "ruby is on PATH but bundle is not") without flipping the all-or-nothing flag. Backed by a new public `missing_commands` field.

### Commits since v0.5.2

- MockShell::mark_missing

## [0.5.2] — 2026-05-25

### Added

### Changed

- Updated dependencies.

### Fixed

- `<cmd> <sub> --help` now resolves nested command help (e.g. `app config show --help`), matching the `help config show` form. Previously the flag form discarded the path between the subcommand and the flag and fell back to the parent command's help.
- The parent-process signal path now installs a real Ctrl+C/SIGINT handler instead of `SIG_IGN`. Because `SIG_IGN` is inherited across `exec`, spawned children (and their descendants) would otherwise have become immune to Ctrl+C; a handler is reset to the default in exec'd children, so they still terminate on Ctrl+C while the parent survives.

### Commits since v0.5.1

- update dependencies
- fix(signals): install survive-Ctrl+C handler, not inherited SIG_IGN
- fix(cli): resolve nested help for the '<cmd> <sub> --help' form
- update version to 0.5.2

## [0.5.1] — 2026-05-11

### Added

### Changed

### Fixed

### Commits since v0.5.0

- docs: bump README install pin to 0.5

## [0.5.0] — 2026-05-11

### Changed

- **BREAKING:** `cli::parse_cli` / `parse_cli_from` no longer call `std::process::exit`. Help and version requests now surface as `CliError::ShowHelp` and the new `CliError::ShowVersion` variant; the application owns its exit codes. This makes the parser embeddable in TUIs, tests, and tools that wrap other CLIs.
- **BREAKING:** `cli::help::print_help` and `cli::help::print_version` no longer return `!`. They print to stdout and return `()`; callers decide whether (and how) to exit afterwards.
- **BREAKING (error-string only):** `CommandResult::require_success` no longer appends `" — run with --verbose to see details"` to the error message. The library now returns just `"{cmd} failed"`, leaving consumer-specific recovery advice to the application. Consumers that want to embed a hint should use the new `require_success_with_hint(cmd, hint)` method or wrap `require_success` in their own extension trait.
- **BREAKING:** `output::file_stats` is now gated behind the default-off `file-stats` feature. Per-run file-operation telemetry is opt-in scope for this crate; consumers that use `FileStats` / `ShowBytes` must enable `features = ["file-stats"]`. Consumers that don't need file-op summaries get a slightly leaner build.
- **BREAKING:** Renamed `paths::script_dir` → `paths::parent_dir_or_dot` and `paths::script_filename` → `paths::file_name_str`. The old names embedded one downstream consumer's "script" concept; the new names describe the operation. Behavior is unchanged.
- **BREAKING:** `shell::shell_exec` (free function) now takes explicit `program: &str` and `flag: &str` arguments instead of hardcoding `bash`/`powershell`. Callers that previously relied on the platform default should call `ShellConfig::effective_shell_program()` to resolve the pair, or use the `Shell::shell_exec` trait method which now uses `ShellConfig` automatically.
- **BREAKING (default behaviour):** `ProcessShell::shell_exec`, `exec_capture`, and `exec_interactive` no longer hardcode `bash -c` / `powershell -Command`. They now consult `ShellConfig::effective_shell_program()`, which on Unix prefers `$SHELL` (falling back to `bash`) and on Windows prefers `pwsh` → `powershell` → `cmd /c`. Pin the program explicitly via `ShellConfig { shell_program: Some((..., ...)), .. }` if you need the old behavior.
- **BREAKING:** Help and version are no longer modeled as `CliError` variants. `cli::parse_cli` / `parse_cli_from` now return `Result<CliOutcome<C>, CliError>`, where the new `CliOutcome` enum carries the three success shapes (`Parsed(ParsedCli<C>)`, `Help(String)`, `Version(String)`). `CliError` shrinks to genuine errors only (`Usage`, `Conflict`); the `ShowHelp` and `ShowVersion` variants and their `show_help` / `show_version` constructors are gone. Sub-parsers that previously returned `Err(CliError::ShowHelp(...))` to surface help on unknown sub-subcommands should now return `Err(CliError::Usage(...))` pointing the user at `help <cmd>`, since `Result<C, CliError>` no longer has a non-error path.
- **BREAKING:** `shell::scripted::ScriptedShell` renamed to `OverlayScriptedShell`. The new name reflects the type's narrow scope — it only scripts `Shell::run_command` to drive the spinner overlay. Every other `Shell` method (`shell_exec`, `command_exists`, `command_output`, `exec_capture`, `exec_interactive`) now **panics** with an explanatory message instead of silently returning fake success. Tests that previously relied on the silent stubs were masking misuse; compose `OverlayScriptedShell` with `MockShell` (or your own `Shell` impl) when you need both overlay-rendered `run_command` calls and other shell behaviour.

### Added

- `CliError::ShowVersion(String)` variant + `CliError::show_version` constructor.
- `CommandResult::require_success_with_hint(cmd, hint)` — builds a `ShellError::Failed` of the form `"{cmd} failed — {hint}"` for callers that have concrete recovery advice to surface.
- `ShellConfig::shell_program: Option<(String, String)>` — optional `(program, flag)` override for shell-script execution; works around minimal containers without `bash`, locked-down `powershell.exe`, or users who prefer `pwsh`/`zsh`/`fish`.
- `ShellConfig::effective_shell_program()` — resolves the configured override or auto-detects the platform shell.

### Migration

```rust
// Before (jt-consoleutils 0.4.x): parse_cli would exit on --help/--version.
let cli = parse_cli::<Cmd>()?;

// After (0.5.0): handle ShowHelp / ShowVersion explicitly.
let cli = match parse_cli::<Cmd>() {
    Ok(cli) => cli,
    Err(CliError::ShowHelp(text)) => { print_help(&text); std::process::exit(0); }
    Err(CliError::ShowVersion(text)) => { print_version(&text); std::process::exit(0); }
    Err(e) => { eprintln!("Error: {e}"); std::process::exit(1); }
};

// Before
match parse_cli::<Cmd>() {
    Ok(parsed) => run(parsed),
    Err(CliError::ShowHelp(t))    => { print_help(&t);    exit(0); }
    Err(CliError::ShowVersion(t)) => { print_version(&t); exit(0); }
    Err(e) => { eprintln!("Error: {e}"); exit(1); }
}

// After
match parse_cli::<Cmd>() {
    Ok(CliOutcome::Parsed(parsed)) => run(parsed),
    Ok(CliOutcome::Help(t))        => { print_help(&t);    exit(0); }
    Ok(CliOutcome::Version(t))     => { print_version(&t); exit(0); }
    Err(e) => { eprintln!("Error: {e}"); exit(1); }
}
```

---

## [0.4.0] — 2026-05-03

### Added

### Changed

### Fixed

### Commits since v0.3.0

- version bump
- add parse_cli_from, also minor formatting fixes
- perf: store JSON numbers as String to drop f64 parse path
- refactor(output): typed-vocabulary abstraction with Verb/Noun enums and ActionBuilder
- perf(colorize): single-alloc rainbow render — palette as RGB triples
- rearranged modules for better cohesion
- additional functionality from SR
- finish (for now) move of functionality from SR to jt-consoleutils
- various progress & file stat capabilities
- add read_jsonc_file symmetric to read_json_file
- broad audit pass: docs, color routing, API expansion, bug fixes
- feat: add CliError::ShowHelp variant for help-text responses
- Add Output::summary(verb) for subject-less action lines
- add MockShell::push_capture
- fix windows build, update rstest
- cargo:rerun-if-changed


## [0.3.0] — 2026-04-13

### Added

### Changed

### Fixed

### Commits since v0.2.0

- version bump
- remove reliance on float formatting
- CLI argument processing trait template
- add JSONC support
- u+x on scripts/release.sh
- clippy fix
- sed portability fix
- minor release.sh fix - add cargo.lock to commit


## [0.2.0] — 2026-04-10

### Added

### Changed

### Fixed

### Commits since v0.1.0

- add CHANGELOG and GitHub Actions CI workflow
- corrected changelog date
- resolve clippy issues
- additional clippy fix
- propagate exit code from shell
- add require_success and check on CommandResult
- switched to LogLevel
- verbose and trace macros
- verbose and trace fully conditionally compiled
- improve feature flagging of verbose and trace
- command_parts() function to reduce duplication
- support word wrapping in output
- search path directly rather than running where/which for performance
- added release script, refreshed claude.md
- fix clippy and publish flags in release script
- fix broken intra-doc link for LogLevel in output module docs


## [0.1.0] — 2026-03-04

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

[0.7.1]: https://github.com/jasontaylor137/jt-consoleutils/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/jasontaylor137/jt-consoleutils/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/jasontaylor137/jt-consoleutils/compare/v0.5.3...v0.6.0
[0.5.3]: https://github.com/jasontaylor137/jt-consoleutils/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/jasontaylor137/jt-consoleutils/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/jasontaylor137/jt-consoleutils/compare/v0.5.0...v0.5.1
[0.4.0]: https://github.com/jasontaylor137/jt-consoleutils/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/jasontaylor137/jt-consoleutils/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/jasontaylor137/jt-consoleutils/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/jasontaylor137/jt-consoleutils/releases/tag/v0.1.0
