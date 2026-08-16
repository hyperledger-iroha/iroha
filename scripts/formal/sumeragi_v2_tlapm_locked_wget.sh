#!/bin/sh
set -eu

: "${TLAPM_LOCKED_WGET_PYTHON:?missing locked-wget Python}"
: "${TLAPM_LOCKED_WGET_HELPER:?missing locked-wget helper}"
: "${TLAPM_LOCKED_WGET_LOCK:?missing locked-wget lock}"
: "${TLAPM_LOCKED_WGET_PLATFORM:?missing locked-wget platform}"
: "${TLAPM_LOCKED_WGET_CACHE:?missing locked-wget cache}"
: "${TLAPM_LOCKED_WGET_OUTPUT_ROOT:?missing locked-wget output root}"
: "${TLAPM_LOCKED_WGET_RECEIPTS:?missing locked-wget receipt directory}"

exec "$TLAPM_LOCKED_WGET_PYTHON" -I -S "$TLAPM_LOCKED_WGET_HELPER" \
  --lock "$TLAPM_LOCKED_WGET_LOCK" \
  --platform "$TLAPM_LOCKED_WGET_PLATFORM" \
  serve-wget \
  --cache-dir "$TLAPM_LOCKED_WGET_CACHE" \
  --output-root "$TLAPM_LOCKED_WGET_OUTPUT_ROOT" \
  --receipt-dir "$TLAPM_LOCKED_WGET_RECEIPTS" \
  -- "$@"
