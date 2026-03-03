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

---

## Strong candidates

### 1. `plural(n)` — from `vr::str_utils`

```rust
pub fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}
```

Zero domain coupling. `filebydaterust/processor/stats.rs` already reinvents
this inline (`if count == 1 { "" } else { "s" }`). Moving it here would let
both crates share one implementation and eliminate the duplication.

**Proposed location:** `jt-consoleutils::str_utils::plural`

---

### 2. `path_to_string` — from `vr::str_utils`

```rust
pub fn path_to_string(path: &Path) -> String {
    path.display().to_string()
}
```

Thin but universally useful. Belongs in a utils library rather than a domain
crate. Low priority since the one-liner is not hard to write inline, but
consolidating it here keeps things consistent.

**Proposed location:** `jt-consoleutils::str_utils::path_to_string`

---

### 3. `same_file` and `same_content` — from `vr::fs_utils`

```rust
pub fn same_file(a: &Path, b: &Path) -> bool { ... }
pub fn same_content(a: &Path, b: &Path) -> bool { ... }
```

General-purpose filesystem comparison helpers with no domain coupling and no
external dependencies. `filebydaterust` does essentially the same checks in
its own `copy.rs` and `dupe.rs` paths. Moving them here would give both crates
a shared, tested home for this logic.

**Proposed location:** `jt-consoleutils::fs_utils`

---

### 4. `format_bytes` — from `filebydaterust::processor::stats`

```rust
fn format_bytes(bytes: u64) -> String {
    // Returns "1.2 GB", "512 B", etc.
}
```

Human-readable byte-size formatting is a classic utility function. Currently
`pub(crate)` and used only for one summary line in `filebydaterust`, but any
CLI tool that reports file sizes or progress would benefit from it.

**Proposed location:** `jt-consoleutils::str_utils::format_bytes`

---

## Weak candidates (probably leave in place for now)

### 5. `expand_env_vars` — from `vr::env`

Expands `${ENV_VAR}` patterns from the host environment. Generic enough
conceptually but only consumed by `vr`'s script metadata parsing. No second
use case yet justifies the move.

---

### 6. `print_help` / colorized help pattern

Both crates share the same pattern: build a help string → `terminal_width()`
→ `colorize_text_with_width` → `process::exit(0)`. A `print_help(text: &str) -> !`
convenience function here would eliminate that repetition, but it is currently
a one-liner wrapper around two calls that already live in this crate. Worth
doing if a third tool follows the same pattern.

---

### 7. Spinner overlay rendering — from `vr::shell::overlay` + `vr::shell::exec`

The `render_frame` / `clear_lines` / `truncate_visible` / `render_overlay_lines`
functions in `overlay.rs` and `exec.rs` are pure terminal-rendering code with
no `vr`-specific concepts. `filebydaterust`'s `processor/progress.rs` is a
parallel, self-rolled approach to the same problem.

The display layer itself is a straightforward extraction: these four functions
work only on `mpsc::Receiver<Line>` and raw terminal I/O, and already depend on
`jt_consoleutils::terminal::terminal_width()`. Moving them to
`jt-consoleutils::spinner` would require making the `Line` enum public here,
and `vr::shell::exec` would call into the library instead of a sibling module —
no other call-site changes needed.

The blocker is the `Shell` trait and its implementations, which are harder to
separate cleanly (see "Leave in their crates" below). The spinner extraction
alone is low-risk and can be done independently; the full `Shell` abstraction
belongs in a future dedicated crate.

**Deferred** until there is a clear second consumer, at which point the spinner
display layer should be extracted first, separately from the `Shell` trait.

---

## Leave in their crates

| Code | Reason |
|---|---|
| `vr::shell` (`Shell` trait, `ProcessShell`, `DryRunShell`, `MockShell`) | Execution abstraction, not a display abstraction — see detailed note below |
| `vr::fs_utils::make_executable` / `remove_symlink_dir_like` | `vr`-specific installation mechanics |
| `vr::paths` | `vr`-specific path resolution (script discovery, `~/.vr/`, etc.) |
| `vr::test_utils` (`TestEnv`, `MetadataBuilder`, …) | `vr`-specific test scaffolding |
| `filebydaterust::processor::*` | Fully domain-specific (date-path parsing, CRC deduplication, etc.) |
| `filebydaterust::processor::progress::Progress` | Works against the `Output` trait but is tied to `filebydaterust`'s terminology and progress model |

### Why `vr::shell` stays out of `jt-consoleutils`

There are two distinct reasons the `Shell` trait and its implementations do not
belong here:

**The trait surface area is shaped by `vr`'s execution needs.** The six trait
methods (`run_command`, `shell_exec`, `exec_capture`, `exec_interactive`,
`command_exists`, `command_output`) exist because `vr` needs to run npm scripts,
check for installed tools like `git`/`node`, capture their output for metadata
parsing, and drop into interactive flows like `aws sso login`. That is execution
orchestration, not display. A display/utils library that acquires a `Shell`
trait with six methods and three concrete implementations is operating well
outside its mandate.

**The `Shell` trait is coupled to `Output` throughout, not the other way
around.** Every trait method signature takes `output: &mut dyn Output` and
`mode: OutputMode`. Moving `Shell` here would make the display primitives and
the process-spawning abstraction live in the same crate, inverting the
appropriate dependency direction. The spinner *rendering* layer depends on
`Output`; the `Shell` *execution* layer depends on the spinner rendering and
on `Output`. Those are two separate levels.

**`MockShell` and `ScriptedShell` are `vr`-shaped test infrastructure.**
`MockShell` records calls for assertion against `vr` command sequences;
`ScriptedShell` drives the real spinner with scripted stdout/stderr events for
`vr`'s visual regression tests. Extracting them gains nothing until a second
crate needs the same execution abstraction with the same testing pattern.

### Future path: a dedicated `jt-shell` crate

When a second tool needs the same "spinner + dry-run + mock" execution pattern,
the right move is a new `jt-shell` crate that owns the `Shell` trait,
`CommandResult`, `ShellError`, `ProcessShell`, `DryRunShell`, `MockShell`, and
`ScriptedShell`. That crate would depend on `jt-consoleutils` for
`Output`/`OutputMode`/`terminal_width` — keeping the dependency direction
correct — and `vr` would depend on `jt-shell`. The spinner rendering could be
extracted into `jt-consoleutils::spinner` at that same time, or earlier as a
standalone step.

#### Making the `Shell` trait more general

The current `run_command` signature takes `program: &str, args: &[&str]`,
which bakes in a specific calling convention. A more general design would
accept a `std::process::Command` directly — callers construct the command
however they like, and the `Shell` implementation only decides *how* to run it
(live, dry-run, captured, interactive). This would decouple the trait from the
string-pair convention and make it usable for tools that need environment
variables, working directories, or other `Command` builder options that the
current API cannot express without adding more parameters.

The free functions `command_exists` and `command_output` are already general
and would move unchanged. `shell_exec` (which delegates to `bash -c` / 
`powershell -Command`) is also fully general and would move unchanged.

`MockShell` and `ScriptedShell` would need updating to accept `Command` rather
than `(program, args)`, but their core logic (recording calls / replaying
scripted events) is unaffected by the signature change.

---

## Priority order

| Priority | Item | Source |
|---|---|---|
| High | `plural(n)` | `vr::str_utils` — already duplicated inline in `filebydaterust` |
| High | `same_file` / `same_content` | `vr::fs_utils` — general, no deps, reusable |
| Medium | `path_to_string` | `vr::str_utils` — trivial but consistent with the above |
| Medium | `format_bytes` | `filebydaterust::processor::stats` — useful if more tools need it |
| Low | `expand_env_vars` | `vr::env` — generic but single consumer today |
| Low | `print_help` wrapper | both crates — worth doing when a third tool appears |
| Deferred | Spinner overlay display layer | `vr::shell::overlay` + `vr::shell::exec` — extract when second consumer appears |
| Future crate | `Shell` trait + implementations | `vr::shell` — belongs in `jt-shell`, not here |