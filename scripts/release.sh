#!/usr/bin/env bash
set -euo pipefail

# ── Usage ────────────────────────────────────────────────────────────────────
#   scripts/release.sh <version>
#   Example: scripts/release.sh 0.2.0
# ─────────────────────────────────────────────────────────────────────────────

if [[ $# -ne 1 ]]; then
  echo "Usage: scripts/release.sh <version>  (e.g. 0.2.0)"
  exit 1
fi

VERSION="$1"
TAG="v${VERSION}"

echo "==> Releasing ${TAG}"
echo ""

# ── 1. Preflight checks ─────────────────────────────────────────────────────

echo "--- Preflight checks ---"

# Clean working tree
if [[ -n "$(git status --porcelain)" ]]; then
  echo "ERROR: working tree is not clean. Commit or stash changes first."
  exit 1
fi
echo "  Working tree is clean"

# Tests
echo "  Running tests..."
cargo test --all-features --quiet
echo "  Tests passed"

# Clippy
echo "  Running clippy..."
cargo clippy --all-features --quiet -- -D warnings
echo "  Clippy passed"

# Docs
echo "  Checking docs..."
RUSTDOCFLAGS="-D missing_docs" cargo doc --all-features --no-deps --quiet
echo "  Docs passed"

# Dry-run publish
echo "  Running cargo publish --dry-run..."
cargo publish --dry-run --quiet
echo "  Publish dry-run passed"

echo ""

# ── 2. Update Cargo.toml version ────────────────────────────────────────────

echo "--- Updating Cargo.toml version to ${VERSION} ---"

CURRENT_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
if [[ "${CURRENT_VERSION}" == "${VERSION}" ]]; then
  echo "ERROR: Cargo.toml already at version ${VERSION}"
  exit 1
fi

awk -v old="$CURRENT_VERSION" -v new="$VERSION" '
  !done && $0 ~ "^version = \"" old "\"" {
    sub("\"" old "\"", "\"" new "\"")
    done=1
  }
  {print}
' Cargo.toml > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
echo "  Cargo.toml updated: ${CURRENT_VERSION} -> ${VERSION}"

# Regenerate Cargo.lock to match the new version
cargo check --quiet
echo "  Cargo.lock updated"

echo ""

# ── 3. Draft changelog ──────────────────────────────────────────────────────

echo "--- Drafting changelog ---"

PREV_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "")
TODAY=$(date +%Y-%m-%d)

# Collect commits since last tag
if [[ -n "${PREV_TAG}" ]]; then
  COMMITS=$(git log "${PREV_TAG}..HEAD" --pretty=format:"- %s" --reverse)
else
  COMMITS=$(git log --pretty=format:"- %s" --reverse)
fi

# Build the new changelog section
NEW_SECTION="## [${VERSION}] — ${TODAY}

### Added

### Changed

### Fixed

### Commits since ${PREV_TAG:-initial}

${COMMITS}
"

# Insert after the "---" separator line (line after the header block)
# Find the line number of the first "## [" section or end of file
INSERT_LINE=$(grep -n "^## \[" CHANGELOG.md | head -1 | cut -d: -f1)

if [[ -n "${INSERT_LINE}" ]]; then
  # Insert before the first existing version section
  head -n $((INSERT_LINE - 1)) CHANGELOG.md > CHANGELOG.tmp
  printf '%s\n\n' "${NEW_SECTION}" >> CHANGELOG.tmp
  tail -n +"${INSERT_LINE}" CHANGELOG.md >> CHANGELOG.tmp
else
  # No existing sections — append to end
  cp CHANGELOG.md CHANGELOG.tmp
  printf '\n%s\n' "${NEW_SECTION}" >> CHANGELOG.tmp
fi

mv CHANGELOG.tmp CHANGELOG.md

# Append the link reference for this version
REPO_URL=$(grep '^repository = ' Cargo.toml | sed 's/repository = "\(.*\)"/\1/')
if [[ -n "${PREV_TAG}" ]]; then
  COMPARE_LINK="[${VERSION}]: ${REPO_URL}/compare/${PREV_TAG}...${TAG}"
else
  COMPARE_LINK="[${VERSION}]: ${REPO_URL}/releases/tag/${TAG}"
fi

# Add link ref before existing link refs or at end
if grep -q '^\[' CHANGELOG.md; then
  FIRST_LINK_LINE=$(grep -n '^\[' CHANGELOG.md | head -1 | cut -d: -f1)
  { head -n $((FIRST_LINK_LINE - 1)) CHANGELOG.md; echo "${COMPARE_LINK}"; tail -n +"${FIRST_LINK_LINE}" CHANGELOG.md; } > CHANGELOG.tmp && mv CHANGELOG.tmp CHANGELOG.md
else
  echo "" >> CHANGELOG.md
  echo "${COMPARE_LINK}" >> CHANGELOG.md
fi

echo "  Changelog drafted with $(echo "${COMMITS}" | wc -l | tr -d ' ') commit(s)"
echo ""

# ── 4. Open editor for changelog polish ─────────────────────────────────────

EDITOR="${EDITOR:-vi}"
echo "--- Opening CHANGELOG.md in ${EDITOR} ---"
echo "  Edit the Added/Changed/Fixed sections, then save and close."
echo "  Delete the 'Commits since ...' section once you've moved items up."
echo ""

${EDITOR} CHANGELOG.md

# Confirm
echo ""
read -rp "Changelog looks good? [y/N] " CONFIRM
if [[ "${CONFIRM}" != "y" && "${CONFIRM}" != "Y" ]]; then
  echo "Aborting. Changes left in working tree for you to clean up."
  exit 1
fi

echo ""

# ── 5. Commit ────────────────────────────────────────────────────────────────

echo "--- Committing release ---"
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "release v${VERSION}"
echo "  Committed"

echo ""

# ── 6. Tag ───────────────────────────────────────────────────────────────────

echo "--- Tagging ${TAG} ---"
HEAD_SHA=$(git rev-parse HEAD)
if EXISTING_SHA=$(git rev-parse --verify --quiet "refs/tags/${TAG}"); then
  if [[ "${EXISTING_SHA}" == "${HEAD_SHA}" ]]; then
    echo "  Tag ${TAG} already exists at HEAD; skipping"
  else
    echo "ERROR: tag ${TAG} already exists at ${EXISTING_SHA} (HEAD is ${HEAD_SHA})."
    echo "       Delete or move the tag manually before re-running."
    exit 1
  fi
else
  git tag "${TAG}"
  echo "  Tagged"
fi

echo ""

# ── 7. Push ──────────────────────────────────────────────────────────────────

echo "--- Pushing to remote ---"
git push
# `git push --tags` rejects when any tag already exists remotely with a different
# SHA. For our tag we've verified above that local matches HEAD, so push it
# explicitly; if the remote already has it at the same SHA the push is a no-op.
REMOTE_TAG_SHA=$(git ls-remote --tags origin "refs/tags/${TAG}" | awk '{print $1}')
if [[ -z "${REMOTE_TAG_SHA}" ]]; then
  git push origin "refs/tags/${TAG}"
elif [[ "${REMOTE_TAG_SHA}" == "${HEAD_SHA}" ]]; then
  echo "  Remote already has ${TAG} at HEAD; skipping tag push"
else
  echo "ERROR: remote tag ${TAG} points to ${REMOTE_TAG_SHA}, not HEAD (${HEAD_SHA})."
  exit 1
fi
echo "  Pushed"

echo ""

# ── 8. Publish ───────────────────────────────────────────────────────────────

echo "--- Publishing to crates.io ---"
cargo publish
echo "  Published"

echo ""
echo "==> ${TAG} released successfully!"
