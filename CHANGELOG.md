# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

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

[0.4.0]: https://github.com/jasontaylor137/jt-consoleutils/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/jasontaylor137/jt-consoleutils/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/jasontaylor137/jt-consoleutils/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/jasontaylor137/jt-consoleutils/releases/tag/v0.1.0
