#!/usr/bin/env bash
# Packages one built `meridian` executable as a release artifact
# (`meridian-rust-migration-program-plan.md` §5.24.4).
#
#   scripts/release/package.sh <target-triple> <built-executable> <out-dir>
#
# The artifact is `meridian-<VERSION>-<target>.zip` for Windows and
# `meridian-<VERSION>-<target>.tar.gz` otherwise, holding one directory
# `meridian-<VERSION>-<target>/` with exactly the executable and LICENSE —
# no runtime, library or interpreter. The version is read from the root
# `VERSION`, never passed in, so the artifact name cannot drift from it.
# File times are fixed and the archive carries no owner, so the same
# executable yields the same archive bytes on the same platform.
# Prints the artifact path.

set -euo pipefail

if [ "$#" -ne 3 ]; then
  echo "usage: $0 <target-triple> <built-executable> <out-dir>" >&2
  exit 2
fi
target=$1
binary=$2
out=$3

root=$(cd "$(dirname "$0")/../.." && pwd)
version=$(tr -d '[:space:]' < "$root/VERSION")
case "$version" in
  [0-9]*.[0-9]*.[0-9]*) ;;
  *) echo "package: VERSION \"$version\" is not X.Y.Z" >&2; exit 3 ;;
esac
[ -f "$binary" ] || { echo "package: no executable at $binary" >&2; exit 3; }

name="meridian-$version-$target"
case "$target" in
  *-windows-*) exe=meridian.exe; archive="$name.zip" ;;
  *) exe=meridian; archive="$name.tar.gz" ;;
esac

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
mkdir "$stage/$name"
cp "$binary" "$stage/$name/$exe"
cp "$root/LICENSE" "$stage/$name/LICENSE"
chmod 0755 "$stage/$name/$exe"
chmod 0644 "$stage/$name/LICENSE"
# A fixed instant (2026-01-01T00:00:00Z) for every entry.
TZ=UTC touch -t 202601010000 "$stage/$name/$exe" "$stage/$name/LICENSE" "$stage/$name"

mkdir -p "$out"
out=$(cd "$out" && pwd)
rm -f "$out/$archive"
case "$archive" in
  *.zip)
    if command -v 7z >/dev/null 2>&1; then
      (cd "$stage" && 7z a -tzip -bso0 -bsp0 "$out/$archive" "$name/$exe" "$name/LICENSE")
    else
      (cd "$stage" && zip -q -X "$out/$archive" "$name/$exe" "$name/LICENSE")
    fi
    ;;
  *)
    # Owner-free entries: GNU tar (Linux) and bsdtar (macOS) spell it apart.
    if tar --version 2>/dev/null | grep -q 'GNU tar'; then
      owner=(--owner=0 --group=0 --numeric-owner --format=ustar)
    else
      owner=(--uid 0 --gid 0 --uname '' --gname '' --format ustar)
    fi
    (cd "$stage" && tar "${owner[@]}" -cf - "$name/$exe" "$name/LICENSE" | gzip -n -9 > "$out/$archive")
    ;;
esac
echo "$out/$archive"
