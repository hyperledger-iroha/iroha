#!/usr/bin/env bash
set -euo pipefail

# Prepare corpora parity tests by copying JSON files into the test corpora dir
# and generating .tape fixtures for them using Norito's dump_tape example.
#
# Usage: scripts/prepare_corpora.sh <corpora_dir>
#   Where <corpora_dir> contains files like citm_catalog.json, twitter.json.

SRC=${1:-}
if [[ -z "$SRC" ]]; then
  echo "Usage: $0 <corpora_dir>" >&2
  exit 1
fi
if [[ ! -d "$SRC" ]]; then
  echo "Not a directory: $SRC" >&2
  exit 1
fi

DEST="crates/norito/tests/corpora"
mkdir -p "$DEST"
echo "Copying JSON corpora from $SRC to $DEST ..."
shopt -s nullglob
for f in "$SRC"/*.json; do
  bn=$(basename "$f")
  cp -f "$f" "$DEST/$bn"
done
echo "Generating .tape files ..."
scripts/generate_tapes.sh "$DEST"
echo "Done."

