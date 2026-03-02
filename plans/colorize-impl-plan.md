# Implementation Plan: Migrate Colorize Logic to `jt-consoleutils`

## Pre-Work: Resolved

**`colorize_text` convenience wrapper: Omit.** No caller uses it — `vr` and `filebydaterust` never had it, and both `hello-cli` copies have it commented out. Every real call site already fetches terminal width and passes `Some(width)` explicitly.

---

## Step 1 — Add `terminal_size` dependency to `jt-consoleutils`

- [ ] Open `jt-consoleutils/Cargo.toml`
- [ ] Under `[dependencies]`, add:
  ```toml
  terminal_size = "0.4"
  ```
- [ ] Verify `cargo check -p jt-consoleutils` passes

---

## Step 2 — Create `terminal` module in `jt-consoleutils`

- [ ] Create file `jt-consoleutils/src/terminal.rs`
- [ ] Implement `terminal_width()`:
  ```rust
  use terminal_size::{Width, terminal_size};

  /// Returns the current terminal width in columns, or 80 if it cannot be determined.
  pub fn terminal_width() -> usize {
      terminal_size()
          .map(|(Width(w), _)| w as usize)
          .unwrap_or(80)
  }
  ```
- [ ] Add `#[cfg(test)]` block with:
  - [ ] `terminal_width_returns_positive` — asserts the return value is `> 0`

---

## Step 3 — Create `colors` module in `jt-consoleutils`

- [ ] Create file `jt-consoleutils/src/colors.rs`
- [ ] Copy contents of `filebydaterust/src/cli/colors.rs` verbatim (all constants are already `pub`):
  - `RESET = "\x1b[0m"`
  - `BOLD  = "\x1b[1m"`
  - `DIM   = "\x1b[2m"`
  - `RED   = "\x1b[31m"`
  - `GREEN = "\x1b[32m"`
  - `YELLOW= "\x1b[33m"`
  - `CYAN  = "\x1b[36m"`
- [ ] No visibility changes required

---

## Step 4 — Create `colorize` module in `jt-consoleutils`

- [ ] Create file `jt-consoleutils/src/colorize.rs`
- [ ] Copy the canonical implementation from `vr/src/cli/colorize.rs`
- [ ] Replace the local `const RESET` definition with: `use crate::colors::RESET;`
- [ ] Keep `hsv_to_rgb` and `ansi_rgb_escape` as private `fn`s
- [ ] Confirm `(0.0..1.0).contains(&h_prime)` range-check form is used (canonical `vr` style)
- [ ] Confirm `str::to_owned` is used for line-to-string conversion (canonical `vr` style)
- [ ] Ensure only `colorize_text_with_width` is public — do not add `colorize_text`
- [ ] Add `#[cfg(test)]` block with:
  - [ ] `empty_text_returns_unchanged` — `""` input yields `""` output
  - [ ] `single_line_contains_ansi_escape` — output contains `"\x1b[38;2;"`
  - [ ] `output_ends_with_reset` — output ends with `"\x1b[0m"`
  - [ ] `explicit_width_repeats_gradient` — a line longer than `rainbow_width` colorizes without panic
  - [ ] `none_width_uses_max_line_width` — `None` produces same result as passing the widest line length explicitly

---

## Step 5 — Wire modules into `jt-consoleutils/src/lib.rs`

- [ ] Replace the placeholder content of `lib.rs` with:
  ```rust
  pub mod colors;
  pub mod colorize;
  pub mod terminal;
  ```
- [ ] Confirm the public API surface looks correct:

  | Symbol | Signature |
  |---|---|
  | `jt_consoleutils::colors::RESET` | `&str` (and `BOLD`, `DIM`, `RED`, `GREEN`, `YELLOW`, `CYAN`) |
  | `jt_consoleutils::colorize::colorize_text_with_width` | `fn(text: &str, rainbow_width: Option<usize>) -> String` |
  | `jt_consoleutils::terminal::terminal_width` | `fn() -> usize` |

---

## Step 6 — Run `jt-consoleutils` tests

- [ ] Run `cargo test -p jt-consoleutils`
- [ ] All 6 tests must be green before proceeding

---

## Step 7 — Migrate `vr`

### Remove local colorize module
- [ ] Delete `vr/src/cli/colorize.rs`
- [ ] In `vr/src/cli/mod.rs` — remove `pub mod colorize;`
- [ ] In `vr/src/cli/help.rs` — update import:
  - Remove: `use crate::cli::colorize::colorize_text_with_width;`
  - Add: `use jt_consoleutils::colorize::colorize_text_with_width;`

### Replace terminal width calls
- [ ] In `vr/src/cli/help.rs` — inside `print_colorized`, replace:
  ```rust
  let width = terminal_size::terminal_size()
      .map(|(w, _)| w.0 as usize)
      .unwrap_or(80);
  ```
  with:
  ```rust
  let width = jt_consoleutils::terminal::terminal_width();
  ```
- [ ] In `vr/src/shell/overlay.rs` — remove `use terminal_size::{Width, terminal_size};` and replace the `term_width()` body with:
  ```rust
  jt_consoleutils::terminal::terminal_width()
  ```
- [ ] In `vr/examples/test_shell.rs` — remove `use terminal_size::{Width, terminal_size};` and replace the `term_width()` body with:
  ```rust
  jt_consoleutils::terminal::terminal_width()
  ```

### Update Cargo.toml
- [ ] In `vr/Cargo.toml` — remove `terminal_size = "0.4"` (the `jt-consoleutils` dep is already present)

### Verify
- [ ] `cargo build` inside `vr/` — green
- [ ] `cargo test` inside `vr/` — green

---

## Step 8 — Migrate `filebydaterust`

### Remove local colorize module
- [ ] Delete `filebydaterust/src/cli/colorize.rs`
- [ ] In `filebydaterust/src/cli.rs` — remove `pub mod colorize;`
- [ ] In `filebydaterust/src/cli/args.rs` — update import:
  - Remove: `use crate::cli::colorize::colorize_text_with_width;`
  - Add: `use jt_consoleutils::colorize::colorize_text_with_width;`

### Remove local colors module
- [ ] Delete `filebydaterust/src/cli/colors.rs`
- [ ] In `filebydaterust/src/cli.rs` — remove `pub mod colors;`
- [ ] In each of the following 9 files, replace `use crate::cli::colors::*;` with `use jt_consoleutils::colors::*;`:
  - [ ] `src/main.rs`
  - [ ] `src/processor/copy.rs`
  - [ ] `src/processor/date_path.rs`
  - [ ] `src/processor/dupe.rs`
  - [ ] `src/processor/length_checker.rs`
  - [ ] `src/processor/progress.rs`
  - [ ] `src/processor/purge.rs`
  - [ ] `src/processor/stats.rs`
  - [ ] `src/processor.rs` (inline import inside a fn body)

### Replace terminal width call
- [ ] In `filebydaterust/src/cli/args.rs` — inside `print_help`, replace:
  ```rust
  let width = terminal_size::terminal_size()
      .map(|(w, _)| w.0 as usize)
      .unwrap_or(80);
  ```
  with:
  ```rust
  let width = jt_consoleutils::terminal::terminal_width();
  ```

### Update Cargo.toml
- [ ] In `filebydaterust/Cargo.toml`:
  - Add: `jt-consoleutils = { path = "../jt-consoleutils" }`
  - Remove: `terminal_size = "0.4"`

### Verify
- [ ] `cargo build` inside `filebydaterust/` — green
- [ ] `cargo test` inside `filebydaterust/` — green

---

## Step 9 — Migrate `hello-cli`

### Remove local colorize module
- [ ] Delete `hello-cli/src/cli_colorize.rs`
- [ ] In `hello-cli/src/main.rs` — remove `mod cli_colorize;`
- [ ] In `hello-cli/src/cli_args.rs` — update import:
  - Remove: `use crate::cli_colorize::colorize_text_with_width;`
  - Add: `use jt_consoleutils::colorize::colorize_text_with_width;`

### Replace terminal width call
- [ ] In `hello-cli/src/cli_args.rs` — replace:
  ```rust
  let width = term_size::dimensions().unwrap_or((80, 24)).0;
  ```
  with:
  ```rust
  let width = jt_consoleutils::terminal::terminal_width();
  ```

### Update Cargo.toml
- [ ] In `hello-cli/Cargo.toml`:
  - Add: `jt-consoleutils = { path = "../jt-consoleutils" }`
  - Remove: `term_size = "0.3.2"`

### Verify
- [ ] `cargo build` inside `hello-cli/` — green
- [ ] `cargo test` inside `hello-cli/` — green

---

## Step 10 — Migrate `learning-rust/hello-cli`

> Note: this project uses `clap` instead of `gumdrop`; the colorize and terminal-width logic is otherwise identical to `hello-cli`.

### Remove local colorize module
- [ ] Delete `learning-rust/hello-cli/src/cli_colorize.rs`
- [ ] In `learning-rust/hello-cli/src/main.rs` — remove `mod cli_colorize;`
- [ ] In `learning-rust/hello-cli/src/cli_args.rs` — update import:
  - Remove: `use crate::cli_colorize::colorize_text_with_width;`
  - Add: `use jt_consoleutils::colorize::colorize_text_with_width;`

### Replace terminal width call
- [ ] In `learning-rust/hello-cli/src/cli_args.rs` — replace:
  ```rust
  let width = term_size::dimensions().unwrap_or((80, 24)).0;
  ```
  with:
  ```rust
  let width = jt_consoleutils::terminal::terminal_width();
  ```

### Update Cargo.toml
- [ ] In `learning-rust/hello-cli/Cargo.toml`:
  - Add: `jt-consoleutils = { path = "../../jt-consoleutils" }`
  - Remove: `term_size = "0.3.2"`

### Verify
- [ ] `cargo build` inside `learning-rust/hello-cli/` — green
- [ ] `cargo test` inside `learning-rust/hello-cli/` — green

---

## Final Checklist

- [ ] ~~Open question resolved: `colorize_text` wrapper omitted~~ ✓
- [ ] `jt-consoleutils/Cargo.toml` — `terminal_size = "0.4"` added
- [ ] `jt-consoleutils/src/terminal.rs` — created; `terminal_width()` implemented and tested
- [ ] `jt-consoleutils/src/colors.rs` — created; all 7 constants present and `pub`
- [ ] `jt-consoleutils/src/colorize.rs` — created; uses `crate::colors::RESET`; canonical `vr` style applied; all tests pass
- [ ] `jt-consoleutils/src/lib.rs` — exports `colors`, `colorize`, and `terminal`
- [ ] `cargo test -p jt-consoleutils` — all green
- [ ] `vr` — local colorize deleted; `terminal_size` dep removed; builds and tests pass
- [ ] `filebydaterust` — local colorize and colors deleted; all 9 import sites updated; `terminal_size` dep removed; builds and tests pass
- [ ] `hello-cli` — local colorize deleted; `term_size` dep removed; builds and tests pass
- [ ] `learning-rust/hello-cli` — local colorize deleted; `term_size` dep removed; builds and tests pass
- [ ] No consumer project retains a direct dependency on `terminal_size` or `term_size`
- [ ] No duplicate copies of colorize or colors logic remain anywhere in the repo