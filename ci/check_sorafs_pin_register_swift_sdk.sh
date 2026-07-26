#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${SORAFS_PIN_REGISTER_SWIFT_SDK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
PYTHON_BIN="${SORAFS_PIN_REGISTER_SWIFT_PYTHON_BIN:-python3}"
SWIFTC_BIN="${SORAFS_PIN_REGISTER_SWIFTC_BIN:-swiftc}"

"${PYTHON_BIN}" - "${ROOT_DIR}" <<'PY'
import os
import stat
import sys
from pathlib import Path

root = Path(sys.argv[1])


def read_open_flags() -> int:
    return os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)


def fail(path: Path, message: str) -> None:
    raise SystemExit(f"error: Swift SoraFS pin-register source {message}: {path}")


def read_text_no_follow(path: Path) -> str:
    if path.is_symlink():
        fail(path, "must not be a symlink")
    for parent in (path.parent, *path.parent.parents):
        if parent.is_symlink():
            fail(parent, "parent must not be a symlink")
        if parent.exists() and not parent.is_dir():
            fail(parent, "parent must be a directory")
    try:
        path_stat = path.lstat()
    except FileNotFoundError:
        fail(path, "is missing")
    if not stat.S_ISREG(path_stat.st_mode):
        fail(path, "must be a regular file")
    fd = os.open(path, read_open_flags())
    try:
        descriptor_stat = os.fstat(fd)
        if not stat.S_ISREG(descriptor_stat.st_mode):
            fail(path, "must be a regular file")
        with os.fdopen(fd, "r", encoding="utf-8") as handle:
            fd = -1
            return handle.read()
    finally:
        if fd >= 0:
            os.close(fd)


def read(path):
    return read_text_no_follow(root / path)


def require(text, needle, label):
    if needle not in text:
        raise SystemExit(f"error: missing Swift SoraFS pin-register contract: {label}")


source = read("IrohaSwift/Sources/IrohaSwift/ToriiClient.swift")
tests = read("IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift")

if "ToriiSoraFsPinRegisterRequest" in source:
    raise SystemExit(
        "error: Swift SoraFS pin-register must not expose a secret-bearing request DTO"
    )

for needle, label in (
    (
        "public func registerSoraFsPinManifest(_ transaction: SignedTransactionEnvelope) async throws -> ToriiSoraFsPinRegisterResponse",
        "async API",
    ),
    ('path: "/v1/sorafs/pin/register"', "paid-pin endpoint"),
    ("body: transaction.norito", "caller-signed Norito body"),
    ('"Content-Type": "application/x-norito"', "Norito content type"),
    ('"Accept": "application/json"', "JSON response negotiation"),
    ("acceptedStatus: 202..<203", "HTTP 202 admission contract"),
    ('Set(["status", "tx_hash_hex", "manifest_digest_hex"])', "closed admission response"),
    ("ToriiSoraFsPinRegisterResponse", "response model"),
):
    require(source, needle, label)

for needle, label in (
    (
        "testRegisterSoraFsPinManifestPostsOnlySignedNoritoAndReturnsAdmission",
        "signed request/admission response test",
    ),
    (
        "testRegisterSoraFsPinManifestRejectsPreFinalityFeeClaims",
        "pre-finality response guard",
    ),
    ('"application/x-norito"', "Norito transport assertion"),
    ("statusCode: 202", "HTTP 202 fixture"),
    ('"pin_fee":"1"', "non-admission field rejection fixture"),
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
