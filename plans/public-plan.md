# Plan: Publish jt-consoleutils as a Public crates.io Crate

This plan tracks all work needed to take `jt-consoleutils` from its current
private/local state to a properly published, open-source crate on crates.io.

---

## Step 1 — Confirm crate name availability

- [x] Search [crates.io](https://crates.io/search?q=jt-consoleutils) to confirm
      `jt-consoleutils` is not already taken.
- The `jt-` prefix is a personal namespace convention; there is no formal
  namespace reservation on crates.io, so first-come-first-served applies.

---

## Step 2 — Flesh out `Cargo.toml`

The current `Cargo.toml` is missing all the metadata fields crates.io requires
or strongly expects. Add the following to `[package]`:

| Field | Notes |
|---|---|
| `description` | One-line summary — required by crates.io |
| `license` | e.g. `"MIT"` or `"MIT OR Apache-2.0"` — required |
| `repository` | URL of the GitHub/GitLab repo |
| `homepage` | Optional; can be the same as `repository` |
| `documentation` | Optional; defaults to docs.rs automatically |
| `keywords` | Up to 5 searchable tags |
| `categories` | From the [crates.io category slugs](https://crates.io/category_slugs) list |
| `readme` | `"README.md"` (see Step 3) |
| `authors` | Optional but good to include |

Suggested keywords: `["cli", "terminal", "console", "shell"]`

Suggested categories: `["command-line-interface", "command-line-utilities"]`

---

## Step 3 — Write `README.md`

There is currently no README. crates.io and docs.rs both display it as the
crate's front page. The README should cover:

- [x] Elevator-pitch description of what the crate provides
- [x] Quick-start example showing `Output` / `ConsoleOutput` / `OutputMode`
- [x] Example showing `Shell` / `ProcessShell` / `create()`
- [x] Feature flags section documenting `build-support`
- [x] MSRV declaration (minimum supported Rust version) if desired
- [x] License badge and link

---

## Step 4 — Choose and add a license

There is currently no `LICENSE` file. Pick one and add it:

- **MIT OR Apache-2.0** — the Rust ecosystem standard dual-license (recommended)
- **MIT** — permissive, simple
- **Apache-2.0** — permissive with patent protection clause

- [x] Create `LICENSE-MIT` and/or `LICENSE-APACHE` (or a single `LICENSE` file)
- [x] Set the matching `license` field in `Cargo.toml`

---

## Step 5 — Audit and complete doc comments

All `pub` items must have `///` doc comments — they appear on docs.rs and are
part of the public API contract already stated in `CLAUDE.md`.

Run the missing-docs lint to find gaps:

```
RUSTDOCFLAGS="-D missing_docs" cargo doc
```

Items most likely to need attention:

- [x] Trait methods on `Output` (most have no `///` today)
- [x] Trait methods on `Shell`
- [x] Fields on `OutputMode`, `ShellConfig`, `CommandResult`
- [x] `ShellError` variants
- [x] Top-level `lib.rs` module-level doc comment (`//!`)
- [x] Each module's top-level `//!` doc comment

Also add a `#![warn(missing_docs)]` attribute to `lib.rs` so the lint is
enforced going forward. ✅ Added; `RUSTDOCFLAGS="-D missing_docs" cargo doc`
now passes with zero warnings and zero errors.

---

## Step 6 — Resolve the ignored doctest in `version.rs`

The test run shows:

```
test src/version.rs - version::version_string (line 8) ... ignored
```

The example uses `rust,ignore` because it requires build-time env vars. This
is acceptable, but:

- [x] Add a comment in the doc block explaining *why* the example is `ignore`d
      (e.g. `// requires BUILD_DATE and GIT_HASH env vars set by build.rs`)
      so it doesn't look like an oversight to contributors.

---

## Step 7 — Establish a public Git repository

crates.io expects a `repository` URL in `Cargo.toml`. Decide on the hosting
approach:

**Option A — Dedicated repo (recommended for a standalone public crate)**
- [x] Create `github.com/<yourname>/jt-consoleutils`
- [x] Push the crate's directory as the repo root
- [x] Set `repository = "https://github.com/<yourname>/jt-consoleutils"`

**Option B — Publish from the existing monorepo**
- [x] Ensure the monorepo is public (or make it public)
- [x] Set `repository` to the monorepo URL with a subdirectory note in the
      README, e.g. `https://github.com/<yourname>/rust/tree/main/jt-consoleutils`

Either option works with `cargo publish`; Option A gives the crate a cleaner
standalone presence.

---

## Step 8 — Verify `cargo publish --dry-run` passes

```
cargo publish --dry-run
```

This checks that everything packages correctly: all referenced files exist,
the crate compiles in its packaged form, no banned file patterns are included,
etc.

Common issues to watch for:

- [x] `readme = "README.md"` in `Cargo.toml` must point to a file that exists
- [x] `license-file` (if used) must point to a file that exists
- [x] `.gitignore` already excludes `target/` — confirmed OK
- [x] `Cargo.lock` is intentionally included (this is a library; it can be
      omitted or kept — crates.io ignores it on publish either way)

---

## Step 9 — Create a crates.io account and API token

- [x] Sign in at [crates.io](https://crates.io) with a GitHub account
- [x] Go to **Account Settings → API Tokens** and create a publish token
- [x] Run `cargo login <token>` once on the local machine

---

## Step 10 — Publish

```
cargo publish
```

The crate will be live within minutes. docs.rs will automatically build and
host the documentation.

---

## Step 11 — Post-publish housekeeping

- [ ] Tag the release in git: `git tag v0.1.0 && git push --tags`
- [ ] Update `vr` and `filebydaterust` to reference the published version from
      crates.io rather than a local path dependency — or keep path dependencies
      during active co-development and switch when the API stabilises
- [ ] Create `CHANGELOG.md` to track changes across future versions
- [ ] Consider adding a GitHub Actions CI workflow (or equivalent) to run
      `cargo test` on push so regressions are caught before the next publish

---

## Summary checklist

| # | Task | Status |
|---|---|---|
| 1 | Confirm `jt-consoleutils` name is available on crates.io | ✅ |
| 2 | Add `description`, `license`, `repository`, `readme`, `keywords`, `categories` to `Cargo.toml` | ✅ |
| 3 | Write `README.md` | ✅ |
| 4 | Add `LICENSE` file(s) | ✅ |
| 5 | Audit doc comments; add `#![warn(missing_docs)]` to `lib.rs` | ✅ |
| 6 | Annotate the ignored doctest in `version.rs` | ✅ |
| 7 | Create public Git repository and set `repository` URL | ✅ |
| 8 | `cargo publish --dry-run` passes cleanly | ✅ |
| 9 | crates.io account created and `cargo login` done | ✅ |
| 10 | `cargo publish` | ❌ |
| 11 | Tag release, update consumers, add CHANGELOG, set up CI | ❌ |

The code itself is in excellent shape — 115 passing tests, clean architecture,
no compiler warnings. All remaining work is packaging and metadata, not code.
