#!/usr/bin/env bash
set -euo pipefail

# ── Usage ────────────────────────────────────────────────────────────────────
#   scripts/publish-release.sh
#
# Run after scripts/prepare-release.sh. Tags HEAD, pushes commit + tag, and
# publishes to crates.io. Re-validates the crates.io token before doing
# anything irreversible.
# ─────────────────────────────────────────────────────────────────────────────

echo "==> Publishing"
echo ""

# ── 1. Sanity checks ────────────────────────────────────────────────────────

echo "--- Sanity checks ---"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "ERROR: working tree is not clean."
  exit 1
fi
echo "  Working tree is clean"

HEAD_MSG=$(git log -1 --pretty=%s)
if [[ ! "$HEAD_MSG" =~ ^release[[:space:]]+v[0-9]+\.[0-9]+\.[0-9]+ ]]; then
  echo "ERROR: HEAD message is '${HEAD_MSG}'; expected 'release vX.Y.Z'."
  echo "       Run scripts/prepare-release.sh first."
  exit 1
fi

CARGO_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
COMMIT_VERSION=$(echo "$HEAD_MSG" | sed -E 's/^release[[:space:]]+v([0-9]+\.[0-9]+\.[0-9]+).*$/\1/')
if [[ "$CARGO_VERSION" != "$COMMIT_VERSION" ]]; then
  echo "ERROR: Cargo.toml (${CARGO_VERSION}) and commit message (${COMMIT_VERSION}) disagree."
  exit 1
fi
VERSION="$CARGO_VERSION"
TAG="v${VERSION}"
HEAD_SHA=$(git rev-parse HEAD)
echo "  HEAD is 'release ${TAG}' at ${HEAD_SHA:0:10}"
echo ""

# ── 2. Re-validate crates.io token ──────────────────────────────────────────
# Tokens can expire between prepare and publish. Recheck before tag/push so
# we don't end up with a pushed tag and a failed publish.

echo "--- Re-validating crates.io token ---"
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
  -H "User-Agent: publish-release.sh (jt-consoleutils)" \
  https://crates.io/api/v1/me)
if [[ "$HTTP" != "200" ]]; then
  echo "ERROR: crates.io rejected token (HTTP $HTTP)."
  echo "       Refresh at https://crates.io/me/ and run: cargo login"
  exit 1
fi
echo "  Token valid (crates.io HTTP 200)"
echo ""

# ── 3. Tag ──────────────────────────────────────────────────────────────────

echo "--- Tagging ${TAG} ---"
if EXISTING_SHA=$(git rev-parse --verify --quiet "refs/tags/${TAG}"); then
  if [[ "${EXISTING_SHA}" == "${HEAD_SHA}" ]]; then
    echo "  Tag ${TAG} already at HEAD; skipping"
  else
    echo "ERROR: tag ${TAG} exists at ${EXISTING_SHA} (HEAD is ${HEAD_SHA})."
    exit 1
  fi
else
  git tag "${TAG}"
  echo "  Tagged"
fi
echo ""

# ── 4. Push ─────────────────────────────────────────────────────────────────

echo "--- Pushing to remote ---"
git push
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

# ── 5. Publish ──────────────────────────────────────────────────────────────

echo "--- Publishing to crates.io ---"
cargo publish
echo "  Published"

echo ""
echo "==> ${TAG} released successfully!"
