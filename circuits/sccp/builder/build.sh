#!/bin/sh
set -eu

if [ "$#" -ne 1 ]; then
  echo "usage: builder/build.sh OUTPUT_DIRECTORY" >&2
  exit 64
fi

output_directory=$1
case "$output_directory" in
  ""|/|.|..)
    echo "refusing unsafe output directory" >&2
    exit 64
    ;;
esac
if [ -e "$output_directory" ] || [ -L "$output_directory" ]; then
  echo "output directory must not already exist" >&2
  exit 73
fi

exec docker buildx build \
  --file builder/Dockerfile \
  --network none \
  --no-cache \
  --platform linux/amd64 \
  --provenance=false \
  --sbom=false \
  --output "type=local,dest=$output_directory" \
  .
