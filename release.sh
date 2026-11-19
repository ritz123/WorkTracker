#!/usr/bin/env bash
# Set the app version, tag that commit, and push both to origin.
#
# Usage: ./release.sh 0.2.0
#        ./release.sh v0.2.0
#
# With no argument it prints the latest vMAJOR.MINOR.PATCH tag and stops.
# A release prints that same number before tagging the new one.
#
# The running app reports the git tag. Cargo.toml is the fallback for a
# checkout with no tag, so this script writes the same number into
# Cargo.toml and Cargo.lock, commits that change when it is not already
# the current version, tags the commit vMAJOR.MINOR.PATCH, and pushes the
# branch and the tag. Pushing the tag starts the GitHub Release workflow.
#
# The working tree must be clean. Other uncommitted work is left untouched
# and the script stops, so a version bump never includes unrelated files.

set -euo pipefail

cd "$(dirname "$0")"
git rev-parse --is-inside-work-tree >/dev/null

# The latest release is the highest vMAJOR.MINOR.PATCH tag. Cargo.toml is
# only used when this checkout has never been tagged, so the number shown
# is the last release rather than an untagged edit.
latest_version() {
  local tag
  tag="$(git tag -l 'v*' --sort=-v:refname | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' | head -1 || true)"
  if [[ -n "$tag" ]]; then
    echo "${tag#v}"
    return
  fi
  sed -n 's/^version = "\([0-9][0-9.]*\)"$/\1/p' Cargo.toml | head -1
}

latest="$(latest_version)"

if [[ $# -ne 1 ]]; then
  if [[ -n "$latest" ]]; then
    echo "Latest version is $latest." >&2
  fi
  echo "Usage: ./release.sh MAJOR.MINOR.PATCH" >&2
  exit 2
fi

version="${1#v}"
if ! [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Version must be MAJOR.MINOR.PATCH (got '$1')." >&2
  if [[ -n "$latest" ]]; then
    echo "Latest version is $latest." >&2
  fi
  exit 1
fi

if [[ -n "$latest" ]]; then
  echo "Latest version was $latest."
fi

branch="$(git rev-parse --abbrev-ref HEAD)"
if [[ "$branch" == "HEAD" ]]; then
  echo "Checkout a branch before releasing." >&2
  exit 1
fi

if [[ -n "$(git status --porcelain)" ]]; then
  echo "Working tree is not clean. Commit or stash other changes first." >&2
  git status --short >&2
  exit 1
fi

if git rev-parse -q --verify "refs/tags/v$version" >/dev/null; then
  echo "Tag v$version already exists locally." >&2
  exit 1
fi

if git ls-remote --tags origin "refs/tags/v$version" | grep -q .; then
  echo "Tag v$version already exists on origin." >&2
  exit 1
fi

stamp_version() {
  local file="$1"
  local mode="$2"
  awk -v ver="$version" -v mode="$mode" '
    mode == "toml" && /^version = / && !done {
      print "version = \"" ver "\""
      done = 1
      next
    }
    mode == "lock" && $0 == "name = \"weekly_work_tracker\"" { hit = 1 }
    mode == "lock" && hit && /^version = / {
      print "version = \"" ver "\""
      hit = 0
      next
    }
    { print }
  ' "$file" > "$file.stamped"
  mv "$file.stamped" "$file"
}

stamp_version Cargo.toml toml
stamp_version Cargo.lock lock
grep -q "^version = \"$version\"$" Cargo.toml
grep -q "^version = \"$version\"$" Cargo.lock

# The advert page names the release files, WorkTracker-<version>-x86_64.AppImage
# and WorkTracker-<version>-setup.exe. That number has to be the one just
# written into Cargo.toml. A page left on the previous version offers a
# download from the previous tag.
page="advert/index.html"
current="$(sed -n 's/.*name="release-version" content="\([0-9][0-9.]*\)".*/\1/p' "$page" | head -1)"
if [[ -z "$current" ]]; then
  echo "$page has no release-version, so the download links cannot follow $version." >&2
  exit 1
fi
if [[ "$current" != "$version" ]]; then
  python3 - "$page" "$current" "$version" <<'PY'
import pathlib, re, sys
path, old, new = sys.argv[1:]
file = pathlib.Path(path)
text = file.read_text()
updated, count = re.subn(r"(?<![0-9.])" + re.escape(old) + r"(?![0-9.])", new, text)
if count == 0:
    raise SystemExit(f"{path} did not contain version {old}")
file.write_text(updated)
PY
fi
grep -q "name=\"release-version\" content=\"$version\"" "$page"

if git diff --quiet -- Cargo.toml Cargo.lock advert/index.html; then
  echo "Version is already $version. Tagging the current commit."
else
  git add Cargo.toml Cargo.lock advert/index.html
  git commit -m "Set version to $version"
fi

git tag -a "v$version" -m "Work Tracker $version"
git push origin "$branch" "v$version"

echo "Pushed $branch and v$version. GitHub will build and publish the release."
