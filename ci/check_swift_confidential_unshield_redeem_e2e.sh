#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${SWIFT_CONFIDENTIAL_UNSHIELD_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/iroha-swift-unshield.XXXXXX")"
BRIDGE_PATH="${ROOT_DIR}/dist/NoritoBridge.xcframework"
BRIDGE_BACKUP="${TMP_DIR}/NoritoBridge.xcframework.previous"
# CI defaults to a disposable target directory. Local retries may point this at
# a persistent cache without changing any generated or tracked artifact.
BRIDGE_TARGET_DIR="${SWIFT_CONFIDENTIAL_UNSHIELD_CARGO_TARGET_DIR:-${TMP_DIR}/cargo-target}"
BRIDGE_HEADERS="${TMP_DIR}/bridge-headers"
SWIFT_SCRATCH_PATH="${TMP_DIR}/swift-build"
BRIDGE_REPLACED=0

case "$(uname -m)" in
  arm64)
    RUST_HOST_TARGET="aarch64-apple-darwin"
    ;;
  x86_64)
    RUST_HOST_TARGET="x86_64-apple-darwin"
    ;;
  *)
    echo "unsupported macOS host architecture: $(uname -m)" >&2
    exit 1
    ;;
esac

cleanup() {
  rm -rf "${BRIDGE_PATH}"
  if [[ "${BRIDGE_REPLACED}" == "1" && -d "${BRIDGE_BACKUP}" ]]; then
    mv "${BRIDGE_BACKUP}" "${BRIDGE_PATH}"
  fi
  rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

# Make every relative Cargo/Swift path use the requested checkout. This also
# lets local validation point at a clean overlay while another worktree is
# active in the caller's current directory.
cd "${ROOT_DIR}"

# This lane deliberately enables the existing real prover/verifier feature only
# for its temporary host bridge. Production SDK artifacts remain fail-closed.
if [[ -d "${BRIDGE_PATH}" ]]; then
  mv "${BRIDGE_PATH}" "${BRIDGE_BACKUP}"
  BRIDGE_REPLACED=1
fi
mkdir -p "${ROOT_DIR}/dist" "${BRIDGE_HEADERS}"
cp "${ROOT_DIR}/crates/connect_norito_bridge/include/connect_norito_bridge.h" "${BRIDGE_HEADERS}/"
cp "${ROOT_DIR}/crates/connect_norito_bridge/include/NoritoBridge.h" "${BRIDGE_HEADERS}/"
cp "${ROOT_DIR}/crates/connect_norito_bridge/module.modulemap.template" \
  "${BRIDGE_HEADERS}/module.modulemap"

rustup target add "${RUST_HOST_TARGET}"
env \
  CARGO_TARGET_DIR="${BRIDGE_TARGET_DIR}" \
  MACOSX_DEPLOYMENT_TARGET=12.0 \
  NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo build --locked --offline -p connect_norito_bridge --lib --release \
    --target "${RUST_HOST_TARGET}" \
    --features privacy-production-enabled
xcodebuild -create-xcframework \
  -library "${BRIDGE_TARGET_DIR}/${RUST_HOST_TARGET}/release/libconnect_norito_bridge.a" \
  -headers "${BRIDGE_HEADERS}" \
  -output "${BRIDGE_PATH}"
touch "${BRIDGE_PATH}/.privacy-production-enabled"

env CARGO_TARGET_DIR="${BRIDGE_TARGET_DIR}" cargo run --locked --offline -q --release \
  --target "${RUST_HOST_TARGET}" \
  -p iroha_core --example confidential_v2_vk_json -- unshield-v3 1 \
  >"${TMP_DIR}/verifier-record.json"
python3 - "${TMP_DIR}/verifier-record.json" "${TMP_DIR}/verifier-record.norito" <<'PY'
import base64
import json
import pathlib
import sys

source = pathlib.Path(sys.argv[1])
target = pathlib.Path(sys.argv[2])
payload = json.loads(source.read_text(encoding="utf-8"))
encoded = payload["record_norito_base64"]
decoded = base64.b64decode(encoded, validate=True)
if base64.b64encode(decoded).decode("ascii") != encoded:
    raise SystemExit("record_norito_base64 is not canonical Standard Base64")
target.write_bytes(decoded)
PY

export IROHA_SWIFT_UNSHIELD_V3_RECORD_PATH="${TMP_DIR}/verifier-record.norito"
export IROHA_SWIFT_UNSHIELD_ATTACHMENT_OUT="${TMP_DIR}/redeem-attachment.norito"
(
  cd IrohaSwift
  swift test --scratch-path "${SWIFT_SCRATCH_PATH}" \
    --filter KagemushaTopUpParityTests/testRealNativeUnshieldProofBuildsRustDecodableRedeemAttachment
)

test -s "${IROHA_SWIFT_UNSHIELD_ATTACHMENT_OUT}"
export IROHA_SWIFT_UNSHIELD_ATTACHMENT_PATH="${IROHA_SWIFT_UNSHIELD_ATTACHMENT_OUT}"
env CARGO_TARGET_DIR="${BRIDGE_TARGET_DIR}" cargo test --locked --offline --release \
  --target "${RUST_HOST_TARGET}" \
  -p iroha_core --test swift_confidential_unshield_redeem \
  swift_confidential_unshield_redeem_attachment_is_canonical_and_verifies \
  -- --nocapture

export KAGEMUSHA_RECURSIVE_SPEND_SWIFT_SCRATCH_PATH="${SWIFT_SCRATCH_PATH}"
bash "${ROOT_DIR}/ci/check_kagemusha_recursive_spend_swift_sdk.sh"
