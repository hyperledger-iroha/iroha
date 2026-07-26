#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_JS_SDK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
NODE_BIN="${KAGEMUSHA_RECURSIVE_SPEND_JS_SDK_NODE_BIN:-node}"

version="$(${NODE_BIN} --version)"
if [[ ! "${version}" =~ ^v20\. ]]; then
  echo "error: JavaScript SDK checks require Node 20; got ${version}" >&2
  exit 1
fi

cd "${ROOT_DIR}/javascript/iroha_js"
"${NODE_BIN}" ./scripts/build-dist.mjs
"${NODE_BIN}" --test \
  test/package_dist.test.js \
  test/crypto.browser.test.js \
  test/toriiBrowserClient.test.js
npx tsc --noEmit --skipLibCheck index.d.ts
npx eslint --max-warnings=0 \
  src/index.js \
  src/crypto.js \
  src/crypto.browser.js \
  src/toriiClient.js \
  src/toriiBrowserClient.js \
  src/transaction.js \
  test/package_dist.test.js \
  test/crypto.browser.test.js \
  test/toriiBrowserClient.test.js

echo "Kagemusha JavaScript boundary passed: no offline lifecycle is published."
