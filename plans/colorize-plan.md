# Plan: Migrate Colorize Logic to `jt-consoleutils`

## Overview

Extract the rainbow-colorize logic that currently lives as a local module in
`hello-cli`, `learning-rust/hello-cli`, `vr`, and `filebydaterust` into the
shared `jt-consoleutils` library crate, then update each consumer to use the
library.

`filebydaterust` also has a `colors.rs` module containing basic ANSI escape
constants (`RESET`, `BOLD`, `DIM`, `RED`, `GREEN`, `YELLOW`, `CYAN`) that is
used heavily across that project. This is a natural fit for the library as well
and is included in this plan.

`jt-consoleutils` will also take ownership of terminal-size detection. All four
consumer projects currently query terminal width directly via either `term_size`
(0.3.2) or `terminal_size` (0.4). The library will wrap this with a simple
`terminal_width() -> usize` function so that no consumer needs a direct
dependency on either crate.

---

## Reference Implementations

### `colorize` — canonical source: `vr/src/cli/colorize.rs`

Where copies differ, the `vr` version wins.

| Detail | `vr` (canonical) | `hello-cli` (discard) | `learning-rust/hello-cli` (discard) | `filebydaterust` (discard) |
|---|---|---|---|---|
| Range check | `(0.0..1.0).contains(&h_prime)` | `h_prime >= 0.0 && h_prime < 1.0` | `h_prime >= 0.0 && h_prime < 1.0` | `(0.0..1.0).contains(&h_prime)` ✓ |
| Line-to-string | `str::to_owned` | `\|s\| s.to_string()` | `\|s\| s.to_string()` | `\|s\| s.to_string()` |
| `RESET` source | `const RESET` defined locally | inline `"\x1b[0m"` literal | inline `"\x1b[0m"` literal | imported via `use crate::cli::colors::*` |
| `colorize_text` wrapper | absent | commented out | commented out | absent |

### `colors` — canonical source: `filebydaterust/src/cli/colors.rs`

This is the only copy; it will be moved verbatim into the library.

| Constant | Value |
|---|---|
| `RESET` | `"\x1b[0m"` |
| `BOLD` | `"\x1b[1m"` |
| `DIM` | `"\x1b[2m"` |
| `RED` | `"\x1b[31m"` |
| `GREEN` | `"\x1b[32m"` |
| `YELLOW` | `"\x1b[33m"` |
| `CYAN` | `"\x1b[36m"` |

### `terminal` — new module, no existing canonical source

All four projects use the same one-liner pattern to detect terminal width,
always falling back to `80`, and never needing height. The library will
consolidate this into a single function.

| Project | Crate used | Call site |
|---|---|---|
| `hello-cli` | `term_size` 0.3.2 | `term_size::dimensions().unwrap_or((80, 24)).0` |
| `learning-rust/hello-cli` | `term_size` 0.3.2 | `term_size::dimensions().unwrap_or((80, 24)).0` |
| `filebydaterust` | `terminal_size` 0.4 | `terminal_size::terminal_size().map(\|(w, _)\| w.0 as usize).unwrap_or(80)` |
| `vr/src/cli/help.rs` | `terminal_size` 0.4 | `terminal_size::terminal_size().map(\|(w, _)\| w.0 as usize).unwrap_or(80)` |
| `vr/src/shell/overlay.rs` | `terminal_size` 0.4 | `terminal_size().map(\|(Width(w), _)\| w as usize).unwrap_or(80)` |
| `vr/examples/test_shell.rs` | `terminal_size` 0.4 | `terminal_size().map(\|(Width(w), _)\| w as usize).unwrap_or(80)` |

All callers need **width only**; none need height. The library will use the
`terminal_size` 0.4 crate (already the majority choice) and expose:

```rust
/// Returns the current terminal width in columns, or 80 if it cannot be determined.
pub fn terminal_width() -> usize
```

---

## Open Question to Resolve Before Implementing

**`colorize_text` convenience wrapper** — absent in `vr` and `filebydaterust`,
commented out in both `hello-cli` copies. Before starting implementation, decide:

- [ ] **Include it** — expose `pub fn colorize_text(text: &str) -> String` as a
  zero-config convenience that spans the widest line.
- [ ] **Omit it** — callers always pass an explicit `Option<usize>`.

> Action: confirm before Step 4 below.

---

## Steps

### Step 1 — Add `terminal_size` dependency to `jt-consoleutils`

In `jt-consoleutils/Cargo.toml`, add:

```toml
[dependencies]
terminal_size = "0.4"
```

### Step 2 — Add `terminal` module to `jt-consoleutils`

1. Create `jt-consoleutils/src/terminal.rs`.
2. Implement `terminal_width()` using `terminal_size` 0.4:

```rust
use terminal_size::terminal_size;

/// Returns the current terminal width in columns, or 80 if it cannot be determined.
pub fn terminal_width() -> usize {
    terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(80)
}
```

### Step 3 — Add `colors` module to `jt-consoleutils`

1. Create `jt-consoleutils/src/colors.rs`.
2. Copy the contents of `filebydaterust/src/cli/colors.rs` verbatim.
3. All constants are `pub` — no visibility changes needed.

### Step 4 — Add `colorize` module to `jt-consoleutils`

1. Create `jt-consoleutils/src/colorize.rs`.
2. Copy the canonical implementation from `vr/src/cli/colorize.rs`.
3. Replace the local `const RESET` definition with an import from the sibling
   module: `use crate::colors::RESET;`
4. Keep `hsv_to_rgb` and `ansi_rgb_escape` as private `fn`s.
5. Conditionally add `pub fn colorize_text` depending on the open question above.

### Step 5 — Wire all modules into `jt-consoleutils/src/lib.rs`

Replace the placeholder content of `lib.rs` with:

```rust
pub mod colors;
pub mod colorize;
pub mod terminal;
```

Full public API surface of the crate after this step:

| Symbol | Signature |
|---|---|
| `jt_consoleutils::colors::RESET` | `&str` (and `BOLD`, `DIM`, `RED`, `GREEN`, `YELLOW`, `CYAN`) |
| `jt_consoleutils::colorize::colorize_text_with_width` | `fn(text: &str, rainbow_width: Option<usize>) -> String` |
| `jt_consoleutils::colorize::colorize_text` *(optional)* | `fn(text: &str) -> String` |
| `jt_consoleutils::terminal::terminal_width` | `fn() -> usize` |

### Step 6 — Add tests to `jt-consoleutils`

**`src/colorize.rs`** — add a `#[cfg(test)]` block covering:

| Test name | What it checks |
|---|---|
| `empty_text_returns_unchanged` | empty `""` → `""` |
| `single_line_contains_ansi_escape` | output contains `\x1b[38;2;` |
| `output_ends_with_reset` | output ends with `\x1b[0m` |
| `explicit_width_repeats_gradient` | a line longer than `rainbow_width` colorizes without panic |
| `none_width_uses_max_line_width` | `None` produces same result as passing the widest line length explicitly |

**`src/terminal.rs`** — add a `#[cfg(test)]` block covering:

| Test name | What it checks |
|---|---|
| `terminal_width_returns_positive` | returned value is > 0 (either detected or fallback 80) |

Run `cargo test -p jt-consoleutils` — confirm green.

### Step 7 — Migrate `vr`

**Colorize:**

1. Delete `vr/src/cli/colorize.rs`.
2. In `vr/src/cli/mod.rs` — remove `pub mod colorize;`.
3. In `vr/src/cli/help.rs` — replace:
   `use crate::cli::colorize::colorize_text_with_width;`
   with:
   `use jt_consoleutils::colorize::colorize_text_with_width;`

**Terminal width — `vr/src/cli/help.rs`:**

4. Replace the inline `terminal_size` call in `print_colorized`:
   ```rust
   // before
   let width = terminal_size::terminal_size()
       .map(|(w, _)| w.0 as usize)
       .unwrap_or(80);
   ```
   with:
   ```rust
   // after
   let width = jt_consoleutils::terminal::terminal_width();
   ```

**Terminal width — `vr/src/shell/overlay.rs`:**

5. Remove `use terminal_size::{Width, terminal_size};` and replace the local
   `fn term_width()` body with `jt_consoleutils::terminal::terminal_width()`.

**Terminal width — `vr/examples/test_shell.rs`:**

6. Remove `use terminal_size::{Width, terminal_size};` and replace the local
   `fn term_width()` body with `jt_consoleutils::terminal::terminal_width()`.

**Cargo:**

7. In `vr/Cargo.toml` — remove `terminal_size = "0.4"`.
   (`jt-consoleutils` dependency already present — no addition needed.)

8. Run `cargo build` and `cargo test` inside `vr/` — confirm green.

### Step 8 — Migrate `filebydaterust`

**Colorize:**

1. Delete `filebydaterust/src/cli/colorize.rs`.
2. In `filebydaterust/src/cli.rs` — remove `pub mod colorize;`.
3. In `filebydaterust/src/cli/args.rs` — replace:
   `use crate::cli::colorize::colorize_text_with_width;`
   with:
   `use jt_consoleutils::colorize::colorize_text_with_width;`

**Colors:**

4. Delete `filebydaterust/src/cli/colors.rs`.
5. In `filebydaterust/src/cli.rs` — remove `pub mod colors;`.
6. In every file that contains `use crate::cli::colors::*;` — replace with
   `use jt_consoleutils::colors::*;`

   Files requiring this import update (9 files):
   - `src/main.rs`
   - `src/processor/copy.rs`
   - `src/processor/date_path.rs`
   - `src/processor/dupe.rs`
   - `src/processor/length_checker.rs`
   - `src/processor/progress.rs`
   - `src/processor/purge.rs`
   - `src/processor/stats.rs`
   - `src/processor.rs` (inline `use crate::cli::colors::*` inside a fn body)

   Note: `src/cli/colorize.rs` also had this import but is deleted in step 1.

**Terminal width — `filebydaterust/src/cli/args.rs`:**

7. Replace the inline `terminal_size` call in `print_help`:
   ```rust
   // before
   let width = terminal_size::terminal_size()
       .map(|(w, _)| w.0 as usize)
       .unwrap_or(80);
   ```
   with:
   ```rust
   // after
   let width = jt_consoleutils::terminal::terminal_width();
   ```

**Cargo:**

8. In `filebydaterust/Cargo.toml`:
   - Add: `jt-consoleutils = { path = "../jt-consoleutils" }`
   - Remove: `terminal_size = "0.4"`

9. Run `cargo build` and `cargo test` inside `filebydaterust/` — confirm green.

### Step 9 — Migrate `hello-cli`

**Colorize:**

1. Delete `hello-cli/src/cli_colorize.rs`.
2. In `hello-cli/src/main.rs` — remove `mod cli_colorize;`.
3. In `hello-cli/src/cli_args.rs` — replace:
   `use crate::cli_colorize::colorize_text_with_width;`
   with:
   `use jt_consoleutils::colorize::colorize_text_with_width;`

**Terminal width — `hello-cli/src/cli_args.rs`:**

4. Replace:
   `let width = term_size::dimensions().unwrap_or((80, 24)).0;`
   with:
   `let width = jt_consoleutils::terminal::terminal_width();`

**Cargo:**

5. In `hello-cli/Cargo.toml`:
   - Add: `jt-consoleutils = { path = "../jt-consoleutils" }`
   - Remove: `term_size = "0.3.2"`

6. `hello-cli` has no `colors.rs` equivalent — nothing further to migrate.

7. Run `cargo build` and `cargo test` inside `hello-cli/` — confirm green.

### Step 10 — Migrate `learning-rust/hello-cli`

Note: this project uses `clap` instead of `gumdrop`. The colorize and
terminal-width logic is otherwise identical to `hello-cli`.

**Colorize:**

1. Delete `learning-rust/hello-cli/src/cli_colorize.rs`.
2. In `learning-rust/hello-cli/src/main.rs` — remove `mod cli_colorize;`.
3. In `learning-rust/hello-cli/src/cli_args.rs` — replace:
   `use crate::cli_colorize::colorize_text_with_width;`
   with:
   `use jt_consoleutils::colorize::colorize_text_with_width;`

**Terminal width — `learning-rust/hello-cli/src/cli_args.rs`:**

4. Replace:
   `let width = term_size::dimensions().unwrap_or((80, 24)).0;`
   with:
   `let width = jt_consoleutils::terminal::terminal_width();`

**Cargo:**

5. In `learning-rust/hello-cli/Cargo.toml`:
   - Add: `jt-consoleutils = { path = "../../jt-consoleutils" }`
   - Remove: `term_size = "0.3.2"`

6. `learning-rust/hello-cli` has no `colors.rs` equivalent — nothing further
   to migrate.

7. Run `cargo build` and `cargo test` inside `learning-rust/hello-cli/` —
   confirm green.

---

## Completion Checklist

- [ ] Open question resolved: include `colorize_text` convenience wrapper?
- [ ] `jt-consoleutils/Cargo.toml` adds `terminal_size = "0.4"`
- [ ] `jt-consoleutils/src/terminal.rs` created; `terminal_width()` implemented
- [ ] `jt-consoleutils/src/colors.rs` created
- [ ] `jt-consoleutils/src/colorize.rs` created (uses `crate::colors::RESET`)
- [ ] `jt-consoleutils/src/lib.rs` exports all three modules
- [ ] All tests added and passing (`cargo test -p jt-consoleutils`)
- [ ] `vr` local colorize module deleted; `terminal_size` dep removed; builds and tests pass
- [ ] `filebydaterust` local colorize and colors modules deleted; `terminal_size` dep removed; all import sites updated; builds and tests pass
- [ ] `hello-cli` local colorize module deleted; `term_size` dep removed; builds and tests pass
- [ ] `learning-rust/hello-cli` local colorize module deleted; `term_size` dep removed; builds and tests pass
- [ ] No consumer project retains a direct dependency on `terminal_size` or `term_size`
- [ ] No duplicate copies of colorize or colors logic remain in the repo

---

## Files Affected

| File | Action |
|---|---|
| `jt-consoleutils/Cargo.toml` | **Modify** (add `terminal_size = "0.4"`) |
| `jt-consoleutils/src/terminal.rs` | **Create** |
| `jt-consoleutils/src/colors.rs` | **Create** |
| `jt-consoleutils/src/colorize.rs` | **Create** |
| `jt-consoleutils/src/lib.rs` | **Modify** (declare all three modules) |
| `vr/src/cli/colorize.rs` | **Delete** |
| `vr/src/cli/mod.rs` | **Modify** (remove `pub mod colorize`) |
| `vr/src/cli/help.rs` | **Modify** (update colorize import; replace terminal width call) |
| `vr/src/shell/overlay.rs` | **Modify** (remove `terminal_size` import; replace `term_width()` body) |
| `vr/examples/test_shell.rs` | **Modify** (remove `terminal_size` import; replace `term_width()` body) |
| `vr/Cargo.toml` | **Modify** (remove `terminal_size = "0.4"`) |
| `filebydaterust/src/cli/colorize.rs` | **Delete** |
| `filebydaterust/src/cli/colors.rs` | **Delete** |
| `filebydaterust/src/cli.rs` | **Modify** (remove `pub mod colorize` and `pub mod colors`) |
| `filebydaterust/src/cli/args.rs` | **Modify** (update colorize import; replace terminal width call) |
| `filebydaterust/src/main.rs` | **Modify** (update colors import) |
| `filebydaterust/src/processor/copy.rs` | **Modify** (update colors import) |
| `filebydaterust/src/processor/date_path.rs` | **Modify** (update colors import) |
| `filebydaterust/src/processor/dupe.rs` | **Modify** (update colors import) |
| `filebydaterust/src/processor/length_checker.rs` | **Modify** (update colors import) |
| `filebydaterust/src/processor/progress.rs` | **Modify** (update colors import) |
| `filebydaterust/src/processor/purge.rs` | **Modify** (update colors import) |
| `filebydaterust/src/processor/stats.rs` | **Modify** (update colors import) |
| `filebydaterust/src/processor.rs` | **Modify** (update inline colors import) |
| `filebydaterust/Cargo.toml` | **Modify** (add `jt-consoleutils`; remove `terminal_size`) |
| `hello-cli/src/cli_colorize.rs` | **Delete** |
| `hello-cli/src/main.rs` | **Modify** (remove `mod cli_colorize`) |
| `hello-cli/src/cli_args.rs` | **Modify** (update colorize import; replace terminal width call) |
| `hello-cli/Cargo.toml` | **Modify** (add `jt-consoleutils`; remove `term_size`) |
| `learning-rust/hello-cli/src/cli_colorize.rs` | **Delete** |
| `learning-rust/hello-cli/src/main.rs` | **Modify** (remove `mod cli_colorize`) |
| `learning-rust/hello-cli/src/cli_args.rs` | **Modify** (update colorize import; replace terminal width call) |
| `learning-rust/hello-cli/Cargo.toml` | **Modify** (add `jt-consoleutils`; remove `term_size`) |