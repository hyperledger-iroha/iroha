#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${SORAFS_PIN_REGISTER_SWIFT_SDK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
PYTHON_BIN="${SORAFS_PIN_REGISTER_SWIFT_PYTHON_BIN:-python3}"
SWIFTC_BIN="${SORAFS_PIN_REGISTER_SWIFTC_BIN:-swiftc}"

"${PYTHON_BIN}" - "${ROOT_DIR}" <<'PY'
import sys
from pathlib import Path

root = Path(sys.argv[1])


def read(path):
    return (root / path).read_text(encoding="utf-8")


def require(text, needle, label):
    if needle not in text:
        raise SystemExit(f"error: missing Swift SoraFS pin-register contract: {label}")


source = read("IrohaSwift/Sources/IrohaSwift/ToriiClient.swift")
tests = read("IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift")

for needle, label in (
    (
        "public func registerSoraFsPinManifest(_ requestBody: ToriiSoraFsPinRegisterRequest) async throws -> ToriiSoraFsPinRegisterResponse",
        "async API",
    ),
    ("requestBody.normalized()", "request normalization before send"),
    ('path: "/v1/sorafs/pin/register"', "paid-pin endpoint"),
    (".normalized()", "typed response normalization"),
    ("ToriiSoraFsPinRegisterRequest", "request model"),
    ("ToriiSoraFsPinRegisterResponse", "response model"),
):
    require(source, needle, label)

for needle, label in (
    (
        "testRegisterSoraFsPinManifestPostsNormalizedPayloadAndDecodesResponse",
        "positive request/response test",
    ),
    (
        "testRegisterSoraFsPinManifestRejectsMalformedInputsBeforeRequest",
        "preflight malformed-input test",
    ),
    (
        "testRegisterSoraFsPinManifestRejectsMalformedResponse",
        "malformed response test",
    ),
    ("XCTAssertFalse(didSendRequest)", "fail-closed no-request assertion"),
    ('ToriiSoraFsStorageClass(type: "lava")', "unsupported storage-class rejection"),
    ('proofBase64 = "not base64!"', "malformed alias proof rejection"),
    ('proofBase64 = Data().base64EncodedString()', "empty alias proof rejection"),
):
    require(tests, needle, label)

print("Swift SoraFS pin-register SDK contract: ok")
PY

command -v "${SWIFTC_BIN}" >/dev/null 2>&1
cd "${ROOT_DIR}"
"${SWIFTC_BIN}" --version
"${SWIFTC_BIN}" -parse -parse-as-library \
  IrohaSwift/Sources/IrohaSwift/ToriiClient.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift
