#!/usr/bin/env bash
set -euo pipefail

# ── Usage ────────────────────────────────────────────────────────────────────
#   scripts/prepare-release.sh <version>
#   Example: scripts/prepare-release.sh 0.6.0
#
# Does everything up to a local `release vX.Y.Z` commit. No tag, no push, no
# publish. Run scripts/publish-release.sh afterwards. To back out:
#   git reset --hard HEAD~1
# ─────────────────────────────────────────────────────────────────────────────

if [[ $# -ne 1 ]]; then
  echo "Usage: scripts/prepare-release.sh <version>  (e.g. 0.6.0)"
  exit 1
fi

VERSION="$1"
TAG="v${VERSION}"

echo "==> Preparing release ${TAG}"
echo ""

# ── 1. Preflight checks ─────────────────────────────────────────────────────

echo "--- Preflight checks ---"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "ERROR: working tree is not clean. Commit or stash changes first."
  exit 1
fi
echo "  Working tree is clean"

echo "  Running tests..."
cargo test --all-features --quiet
echo "  Tests passed"

echo "  Running clippy..."
cargo clippy --all-features --quiet -- -D warnings
echo "  Clippy passed"

echo "  Checking docs..."
RUSTDOCFLAGS="-D missing_docs" cargo doc --all-features --no-deps --quiet
echo "  Docs passed"

echo "  Running cargo publish --dry-run..."
cargo publish --dry-run --quiet
echo "  Publish dry-run passed"

echo ""

# ── 2. crates.io token validation ───────────────────────────────────────────
# `cargo publish --dry-run` does not authenticate, so expired tokens only
# surface at real-publish time — after commit and push. Hit the crates.io API
# now to fail fast while everything is still reversible.

echo "--- Validating crates.io token ---"
CREDS_FILE="${CARGO_HOME:-$HOME/.cargo}/credentials.toml"
if [[ ! -f "$CREDS_FILE" ]]; then
  echo "ERROR: no credentials file at $CREDS_FILE. Run: cargo login"
  exit 1
fi
TOKEN=$(awk -F'"' '/^token = / {print $2; exit}' "$CREDS_FILE")
if [[ -z "$TOKEN" ]]; then
  echo "ERROR: no token found in $CREDS_FILE. Run: cargo login"
  exit 1
fi
HTTP=$(curl -s -o /dev/null -w "%{http_code}" \
  -H "Authorization: $TOKEN" \
  -H "User-Agent: prepare-release.sh (jt-consoleutils)" \
  https://crates.io/api/v1/me)
# 200 = unscoped token; 403 = scoped token (recognized but /me is outside its
# scopes — token is still valid for publish). 401 = expired/invalid token,
# which is what we actually want to fail fast on.
case "$HTTP" in
  200) echo "  Token valid (crates.io HTTP 200, unscoped)" ;;
  403) echo "  Token valid (crates.io HTTP 403, scoped — /me out of scope)" ;;
  *)
    echo "ERROR: crates.io rejected token (HTTP $HTTP)."
    echo "       Refresh at https://crates.io/me/ and run: cargo login"
    exit 1
    ;;
esac
echo ""

# ── 3. State-drift detection ────────────────────────────────────────────────
# Be idempotent: if Cargo.toml is already at the target version (e.g. a prior
# run got partway), skip the bump and just verify everything is consistent.

echo "--- Checking repo state ---"

CURRENT_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
LOWER=$(printf '%s\n%s\n' "$CURRENT_VERSION" "$VERSION" | sort -V | head -1)
HAS_CHANGELOG_SECTION=false
if grep -qE "^## \[${VERSION//./\\.}\]" CHANGELOG.md; then
  HAS_CHANGELOG_SECTION=true
fi

# Check README pin too — it's easy to commit a Cargo.toml bump and CHANGELOG
# promotion while leaving the `crate = "X.Y"` install snippet stale, and the
# published crates.io page surfaces that snippet prominently.
CRATE_NAME=$(grep '^name = ' Cargo.toml | head -1 | sed 's/name = "\(.*\)"/\1/')
MAJOR_MINOR="${VERSION%.*}"
README_PIN_OK=true
if grep -qE "^${CRATE_NAME} = \"[0-9]+\.[0-9]+\"" README.md; then
  README_MM=$(grep -E "^${CRATE_NAME} = \"[0-9]+\.[0-9]+\"" README.md | head -1 | sed -E "s/^${CRATE_NAME} = \"([0-9]+\.[0-9]+)\"/\1/")
  if [[ "$README_MM" != "$MAJOR_MINOR" ]]; then
    README_PIN_OK=false
  fi
fi

if [[ "$CURRENT_VERSION" == "$VERSION" ]]; then
  if $HAS_CHANGELOG_SECTION && $README_PIN_OK; then
    echo "  Cargo.toml at ${VERSION}, CHANGELOG.md has [${VERSION}], README pin matches."
    echo "  Release is fully prepared — nothing to do."
    echo ""
    echo "==> Already prepared. Next steps:"
    echo "    Inspect:      git show HEAD"
    echo "    Publish:      scripts/publish-release.sh"
    exit 0
  fi
  STATE="already-bumped"
  echo "  Cargo.toml already at ${VERSION} (skip bump, verify consistency)"
  $HAS_CHANGELOG_SECTION || echo "  CHANGELOG.md missing [${VERSION}] section — will add"
  $README_PIN_OK         || echo "  README.md pin is \"${README_MM}\", expected \"${MAJOR_MINOR}\" — will fix"
elif [[ "$LOWER" == "$CURRENT_VERSION" ]]; then
  if $HAS_CHANGELOG_SECTION; then
    echo "ERROR: CHANGELOG.md has [${VERSION}] section but Cargo.toml is still at ${CURRENT_VERSION}."
    echo "       Half-prepped state — fix manually."
    exit 1
  fi
  STATE="needs-bump"
  echo "  Cargo.toml at ${CURRENT_VERSION}; will bump to ${VERSION}"
else
  echo "ERROR: Cargo.toml is at ${CURRENT_VERSION}, ahead of target ${VERSION}."
  exit 1
fi

echo ""

# ── 4. Update Cargo.toml / Cargo.lock / README.md ───────────────────────────

echo "--- Applying version updates ---"

if [[ "$STATE" == "needs-bump" ]]; then
  awk -v old="$CURRENT_VERSION" -v new="$VERSION" '
    !done && $0 ~ "^version = \"" old "\"" {
      sub("\"" old "\"", "\"" new "\"")
      done=1
    }
    {print}
  ' Cargo.toml > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
  echo "  Cargo.toml: ${CURRENT_VERSION} -> ${VERSION}"
fi

cargo check --quiet
echo "  Cargo.lock in sync with Cargo.toml"

CRATE_NAME=$(grep '^name = ' Cargo.toml | head -1 | sed 's/name = "\(.*\)"/\1/')
MAJOR_MINOR="${VERSION%.*}"
if grep -qE "^${CRATE_NAME} = \"[0-9]+\.[0-9]+\"" README.md; then
  CURRENT_MM=$(grep -E "^${CRATE_NAME} = \"[0-9]+\.[0-9]+\"" README.md | head -1 | sed -E "s/^${CRATE_NAME} = \"([0-9]+\.[0-9]+)\"/\1/")
  if [[ "$CURRENT_MM" != "$MAJOR_MINOR" ]]; then
    awk -v name="$CRATE_NAME" -v mm="$MAJOR_MINOR" '
      !done && $0 ~ "^" name " = \"[0-9]+\\.[0-9]+\"" {
        sub("\"[0-9]+\\.[0-9]+\"", "\"" mm "\"")
        done=1
      }
      {print}
    ' README.md > README.tmp && mv README.tmp README.md
    echo "  README.md: ${CRATE_NAME} = \"${CURRENT_MM}\" -> \"${MAJOR_MINOR}\""
  else
    echo "  README.md already shows ${CRATE_NAME} = \"${MAJOR_MINOR}\""
  fi
else
  echo "  README.md has no \`${CRATE_NAME} = \"X.Y\"\` line; skipping"
fi

echo ""

# ── 5. Stray-version sweep ──────────────────────────────────────────────────
# Catch references the script doesn't otherwise touch (doc snippets, examples,
# stray version pins). Sweep for PREV_TAG's version — that's the one stale
# refs would carry — rather than CURRENT_VERSION, which may already match the
# target in drifted-state runs.

PREV_VERSION=""
if [[ -n "${PREV_TAG:-}" ]]; then
  PREV_VERSION="${PREV_TAG#v}"
else
  # PREV_TAG not yet set this early in some runs; compute now.
  LAST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "")
  PREV_VERSION="${LAST_TAG#v}"
fi

if [[ -n "$PREV_VERSION" && "$PREV_VERSION" != "$VERSION" ]]; then
  echo "--- Sweeping for stray references to ${PREV_VERSION} ---"
  STRAYS=$(git grep -nF "$PREV_VERSION" -- \
    ':!CHANGELOG.md' \
    ':!Cargo.lock' \
    || true)
  if [[ -n "$STRAYS" ]]; then
    echo "  Found references to ${PREV_VERSION}:"
    echo "$STRAYS" | sed 's/^/    /'
    echo ""
    read -rp "  Continue? Each may be intentional or a missed bump. [y/N] " ACK
    if [[ "$ACK" != "y" && "$ACK" != "Y" ]]; then
      echo "Aborting. Fix the references and re-run."
      exit 1
    fi
  else
    echo "  No stray references found"
  fi
else
  echo "--- Sweeping for stray references ---"
  echo "  (skipped — no previous tag to sweep against)"
fi

echo ""

# ── 6. Changelog: append commit list to [Unreleased] ────────────────────────

PREV_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "")
TODAY=$(date +%Y-%m-%d)

if $HAS_CHANGELOG_SECTION; then
  echo "--- CHANGELOG.md already has [${VERSION}] section — skipping changelog steps ---"
  SKIP_CHANGELOG_STEPS=true
else
  SKIP_CHANGELOG_STEPS=false
fi

if ! $SKIP_CHANGELOG_STEPS; then

echo "--- Updating CHANGELOG.md ---"

if [[ -n "${PREV_TAG}" ]]; then
  COMMITS=$(git log "${PREV_TAG}..HEAD" --pretty=format:"- %s" --reverse)
else
  COMMITS=$(git log --pretty=format:"- %s" --reverse)
fi

python3 - "$VERSION" "$TODAY" "${PREV_TAG:-initial}" "$COMMITS" <<'PY'
import sys, re, pathlib

version, today, prev_tag, commits = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
path = pathlib.Path("CHANGELOG.md")
text = path.read_text()

commits_block = f"### Commits since {prev_tag}\n\n{commits}\n"

m = re.search(r"^## \[Unreleased\].*?(?=^## \[|\Z)", text, re.M | re.S)
if m:
    body = m.group(0)
    if "### Commits since" in body:
        print("  [Unreleased] already has a 'Commits since' block; not re-adding")
    else:
        new_body = body.rstrip() + "\n\n" + commits_block + "\n"
        text = text[:m.start()] + new_body + text[m.end():]
        path.write_text(text)
        print("  Appended commit list to [Unreleased]")
else:
    new_section = (
        "## [Unreleased]\n\n"
        "### Added\n\n### Changed\n\n### Fixed\n\n"
        f"{commits_block}\n"
    )
    m2 = re.search(r"^## \[", text, re.M)
    if m2:
        text = text[:m2.start()] + new_section + text[m2.start():]
    else:
        text = text.rstrip() + "\n\n" + new_section
    path.write_text(text)
    print("  Synthesized [Unreleased] section with commit list")
PY

echo ""

# ── 7. Editor ───────────────────────────────────────────────────────────────

EDITOR="${EDITOR:-vi}"
echo "--- Opening CHANGELOG.md in ${EDITOR} ---"
echo "  Fold items from the 'Commits since' block into Added/Changed/Fixed."
echo "  Delete the 'Commits since ...' block when done."
echo "  Leave the header as '## [Unreleased]' — it gets renamed to [${VERSION}] after save."
echo ""

${EDITOR} CHANGELOG.md

echo ""
read -rp "Changelog looks good? [y/N] " CONFIRM
if [[ "${CONFIRM}" != "y" && "${CONFIRM}" != "Y" ]]; then
  echo "Aborting. Edits left in working tree."
  exit 1
fi
echo ""

# ── 8. Promote [Unreleased] -> [X.Y.Z], seed fresh [Unreleased] ─────────────

echo "--- Promoting [Unreleased] to [${VERSION}] ---"

REPO_URL=$(grep '^repository = ' Cargo.toml | sed 's/repository = "\(.*\)"/\1/')
if [[ -n "${PREV_TAG}" ]]; then
  COMPARE_LINK="[${VERSION}]: ${REPO_URL}/compare/${PREV_TAG}...${TAG}"
else
  COMPARE_LINK="[${VERSION}]: ${REPO_URL}/releases/tag/${TAG}"
fi

python3 - "$VERSION" "$TODAY" "$COMPARE_LINK" <<'PY'
import sys, re, pathlib

version, today, compare_link = sys.argv[1], sys.argv[2], sys.argv[3]
path = pathlib.Path("CHANGELOG.md")
text = path.read_text()

new_unreleased = "## [Unreleased]\n\n### Added\n\n### Changed\n\n### Fixed\n\n"
text, n = re.subn(
    r"^## \[Unreleased\][^\n]*\n",
    new_unreleased + f"## [{version}] — {today}\n",
    text, count=1, flags=re.M)
if n == 0:
    sys.exit("ERROR: [Unreleased] header not found — did you rename it manually?")

m = re.search(r"^\[[^\]]+\]:\s", text, re.M)
if m:
    text = text[:m.start()] + compare_link + "\n" + text[m.start():]
else:
    text = text.rstrip() + "\n\n" + compare_link + "\n"

path.write_text(text)
PY
echo "  Promoted"
echo ""

fi  # end SKIP_CHANGELOG_STEPS guard

# ── 9. Commit (no tag, no push) ─────────────────────────────────────────────

echo "--- Committing ---"
git add Cargo.toml Cargo.lock CHANGELOG.md README.md
git commit -m "release v${VERSION}"
echo "  Committed locally as 'release v${VERSION}'"

echo ""
echo "==> Prepared. Next steps:"
echo "    Inspect:      git show HEAD"
echo "    Publish:      scripts/publish-release.sh"
echo "    Back out:     git reset --hard HEAD~1"
