#!/usr/bin/env bash
set -euo pipefail

PATCH=/tmp/iroha_block_legacy_hash.patch

for repo in /Users/administrator/dev/iroha /Users/administrator/dev/iroha-build-taira-latest; do
  printf '\nAPPLY %s\n' "$repo"
  cd "$repo"
  patch -p1 -N < "$PATCH"
done
