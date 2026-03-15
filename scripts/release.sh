#!/usr/bin/env bash
set -euo pipefail

# Usage: ./scripts/release.sh [patch|minor|major]
# Defaults to "patch" if no argument given.

BUMP="${1:-patch}"

# --- Parse current version ---
CURRENT=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"

case "$BUMP" in
  patch) PATCH=$((PATCH + 1)) ;;
  minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
  major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
  *) echo "Usage: $0 [patch|minor|major]"; exit 1 ;;
esac

NEW="${MAJOR}.${MINOR}.${PATCH}"
TAG="v${NEW}"

echo "${CURRENT} → ${NEW}"

# --- Preflight checks ---
if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "Error: working tree is dirty. Commit or stash changes first." >&2
  exit 1
fi

if git tag -l "$TAG" | grep -q .; then
  echo "Error: tag ${TAG} already exists." >&2
  exit 1
fi

# --- Bump version ---
sed -i.bak "s/^version = \"${CURRENT}\"/version = \"${NEW}\"/" Cargo.toml && rm -f Cargo.toml.bak

# --- Commit, tag, push ---
git add Cargo.toml
git commit -m "Bump version to ${NEW}"
git tag "$TAG"
git push
git push origin "$TAG"

# --- Create GitHub release ---
# Generate changelog from commits since last tag
PREV_TAG=$(git tag --sort=-v:refname | grep -v "^${TAG}$" | head -1)
if [ -n "$PREV_TAG" ]; then
  NOTES=$(git log --pretty=format:"- %s" "${PREV_TAG}..HEAD~1")
else
  NOTES="Initial release"
fi

gh release create "$TAG" --title "$TAG" --notes "### Changes

${NOTES}
"

echo ""
echo "Released ${TAG}"
echo "CI will build binaries and update the Homebrew tap."
echo ""
echo "To upgrade locally:"
echo "  brew unpin kubo 2>/dev/null; brew update && brew upgrade kubo"
