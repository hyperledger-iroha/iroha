#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_PAYLOAD_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
MODE="${1:-}"

if [[ $# -gt 1 || ( -n "${MODE}" && "${MODE}" != "--self-test" ) ]]; then
  echo "usage: ci/check_kagemusha_recursive_spend_payload_bench.sh [--self-test]" >&2
  exit 2
fi

bash "${ROOT_DIR}/ci/check_kagemusha_reviewed_cargo_lock.sh" --verify

# A source digest cannot detect a Norito dependency change. Always compare a
# live serialization with the reviewed fixture before validating its pins.
cargo run --quiet --locked --manifest-path "${ROOT_DIR}/Cargo.toml" \
  -p iroha_data_model --features test-fixtures \
  --bin kagemusha_peer_transport_fixtures -- --check

python3 - "${ROOT_DIR}" "${MODE}" <<'PY'
from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile
from typing import Any

root = Path(sys.argv[1])
self_test = sys.argv[2] == "--self-test"
fixture_path = root / "fixtures/kagemusha/peer_transport_measurements_v1.json"
cargo_lock_path = root / "Cargo.lock"
reviewed_cargo_lock_relative = "fixtures/kagemusha/cargo-lock.reviewed.v1"
reviewed_cargo_lock_path = root / reviewed_cargo_lock_relative
generator_relative = "crates/iroha_data_model/src/bin/kagemusha_peer_transport_fixtures.rs"
factory_relative = "crates/iroha_data_model/src/offline/peer_transport_fixtures.rs"
generator_path = root / generator_relative
factory_path = root / factory_relative
rust_path = root / "crates/iroha_data_model/src/offline/mod.rs"
swift_path = root / "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2.swift"
transport_path = root / "IrohaSwift/Sources/IrohaSwift/KagemushaPeerTransport.swift"
swift_qr_path = root / "IrohaSwift/Sources/IrohaSwift/KagemushaQRStream.swift"
swift_nfc_path = root / "IrohaSwift/Sources/IrohaSwift/KagemushaNFC.swift"
swift_qr_tests_path = root / "IrohaSwift/Tests/IrohaSwiftTests/KagemushaQRStreamTests.swift"
swift_nfc_tests_path = root / "IrohaSwift/Tests/IrohaSwiftTests/KagemushaNFCTests.swift"

SCHEMA = "iroha.kagemusha.peer_transport_measurements.v1"
CARGO_LOCK_SHA256 = "ff773ee12a07de45d0e9df9ed29620142d884f365adb5e83d372e15dbedcd409"
ROOT_KEYS = {
    "schema",
    "generator",
    "generator_dependency",
    "generator_sha256",
    "cargo_lock",
    "cargo_lock_sha256",
    "proof_bytes",
    "records",
}
RECORD_KEYS = {
    "label",
    "kind",
    "branch_depth",
    "peer_hops",
    "archive_bytes",
    "archive_sha256",
    "archive_hex",
}
EXPECTED_PROFILE = (
    ("request", "request", 0, 0),
    ("acknowledgement", "acknowledgement", 0, 0),
    ("payment-depth-1-hop-1", "payment", 1, 1),
    ("payment-depth-8-hop-8", "payment", 8, 8),
    ("payment-depth-16-hop-8", "payment", 16, 8),
    ("payment-depth-32-hop-8", "payment", 32, 8),
    ("payment-depth-64-hop-8", "payment", 64, 8),
)

# Populated only from a reviewed run of the deterministic ABI-21/V4 factory.
# Any change is a release-contract change, even if a new digest is internally
# self-consistent.
EXPECTED_ARCHIVES = {
    "request": (
        612,
        "2faefadfb5f25dfb838e300bb0cb64c36ff7ce845dd5428d4a602d5991cb3eb1",
    ),
    "acknowledgement": (
        370,
        "4ee22699384122cf7cd8d5e45aeda7d71201d7cfefa56d1742b85a719c7e9f37",
    ),
    "payment-depth-1-hop-1": (
        15_920,
        "e976c0ea36f684174d1b1adc4c88a16c3165c8546bcdb3aa48b2e33fd663cce3",
    ),
    "payment-depth-8-hop-8": (
        16_091,
        "31be978f0ea4ca8122f7680ba4977774edd57007b44eca2586ad11e921a95958",
    ),
    "payment-depth-16-hop-8": (
        16_283,
        "cf49098bc6b2ab7c767c9a81004c627ee08c5838e32455c2f1c54243c31c0c0d",
    ),
    "payment-depth-32-hop-8": (
        16_667,
        "2fb5d09c84458307aa555633685cd9fdf273806e41d884f84cac411601f4dcff",
    ),
    "payment-depth-64-hop-8": (
        17_435,
        "c1a063495f07e66161f27dd5f484bb2914d11da9c967b49aa34a32eba56890b5",
    ),
}


class GateError(ValueError):
    pass


def fail(message: str) -> None:
    raise GateError(message)


def reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            fail(f"duplicate JSON key {key!r}")
        value[key] = item
    return value


def load_fixture(raw: str) -> dict[str, Any]:
    try:
        value = json.loads(raw, object_pairs_hook=reject_duplicate_keys)
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        fail(f"malformed measurement fixture: {error}")
    if not isinstance(value, dict):
        fail("measurement fixture root must be an object")
    return value


def exact_keys(value: dict[str, Any], expected: set[str], label: str) -> None:
    actual = set(value)
    if actual != expected:
        fail(
            f"{label} key inventory mismatch: "
            f"missing={sorted(expected - actual)}, extra={sorted(actual - expected)}"
        )


def canonical_hex(value: Any, field: str, expected_bytes: int | None = None) -> bytes:
    if not isinstance(value, str) or re.fullmatch(r"[0-9a-f]+", value) is None:
        fail(f"{field} must be non-empty canonical lowercase hex")
    if len(value) % 2:
        fail(f"{field} must contain complete bytes")
    decoded = bytes.fromhex(value)
    if expected_bytes is not None and len(decoded) != expected_bytes:
        fail(f"{field} must encode exactly {expected_bytes} bytes")
    return decoded


def sha256_hex(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def validate_cargo_locks(ambient: bytes, reviewed: bytes) -> None:
    if sha256_hex(reviewed) != CARGO_LOCK_SHA256:
        fail("reviewed Cargo lock artifact digest drifted")
    if sha256_hex(ambient) != CARGO_LOCK_SHA256 or ambient != reviewed:
        fail("Cargo.lock drifted from the reviewed serializer dependency closure")


def generator_digest(generator_source: bytes, factory_source: bytes) -> str:
    digest = hashlib.sha256()
    digest.update(generator_source)
    digest.update(b"\0")
    digest.update(factory_source)
    return digest.hexdigest()


def validate_fixture(
    raw: str,
    generator_source: bytes,
    expected_archives: dict[str, tuple[int, str]],
    factory_source_override: bytes | None = None,
) -> tuple[
    dict[str, Any],
    tuple[tuple[str, int, int], ...],
    tuple[tuple[str, int, int, int], ...],
]:
    document = load_fixture(raw)
    exact_keys(document, ROOT_KEYS, "fixture")
    if document["schema"] != SCHEMA:
        fail("measurement fixture schema drifted")
    if document["generator"] != generator_relative:
        fail("measurement fixture generator path drifted")
    if document["generator_dependency"] != factory_relative:
        fail("measurement fixture generator dependency drifted")
    if document["cargo_lock"] != reviewed_cargo_lock_relative:
        fail("measurement fixture reviewed Cargo lock path drifted")
    if document["cargo_lock_sha256"] != CARGO_LOCK_SHA256:
        fail("measurement fixture reviewed Cargo lock digest drifted")
    effective_factory_source = (
        factory_source if factory_source_override is None else factory_source_override
    )
    if document["generator_sha256"] != generator_digest(
        generator_source, effective_factory_source
    ):
        fail("measurement fixture generator digest drifted")
    if type(document["proof_bytes"]) is not int or document["proof_bytes"] != 4_096:
        fail("canonical payment proof payload must remain exactly 4096 bytes")
    records = document["records"]
    if not isinstance(records, list) or len(records) != len(EXPECTED_PROFILE):
        fail("measurement fixture record inventory has the wrong cardinality")
    if set(expected_archives) != {profile[0] for profile in EXPECTED_PROFILE}:
        fail("gate lacks a reviewed pin for every canonical archive")

    labels: set[str] = set()
    archive_digests: set[str] = set()
    archive_values: set[bytes] = set()
    derived: list[tuple[str, int]] = []
    actual_profile: list[tuple[str, str, int, int]] = []
    for index, record in enumerate(records):
        if not isinstance(record, dict):
            fail(f"record {index} must be an object")
        exact_keys(record, RECORD_KEYS, f"record {index}")
        label = record["label"]
        kind = record["kind"]
        branch_depth = record["branch_depth"]
        peer_hops = record["peer_hops"]
        archive_bytes = record["archive_bytes"]
        for value, field in (
            (branch_depth, "branch_depth"),
            (peer_hops, "peer_hops"),
            (archive_bytes, "archive_bytes"),
        ):
            if type(value) is not int or value < 0:
                fail(f"record {index} {field} must be a non-negative integer")
        if not isinstance(label, str) or not isinstance(kind, str):
            fail(f"record {index} labels must be strings")
        if label in labels:
            fail(f"duplicate measurement label {label!r}")
        labels.add(label)
        archive = canonical_hex(record["archive_hex"], f"{label}.archive_hex")
        if not archive.startswith(b"NRT0"):
            fail(f"{label} is not a canonical Norito archive")
        if len(archive) != archive_bytes:
            fail(f"{label} archive_bytes is not derived from archive_hex")
        digest = canonical_hex(
            record["archive_sha256"], f"{label}.archive_sha256", 32
        ).hex()
        if digest != sha256_hex(archive):
            fail(f"{label} archive digest does not match its serialized bytes")
        if label not in expected_archives:
            fail(f"unreviewed canonical archive label {label!r}")
        reviewed_bytes, reviewed_digest = expected_archives[label]
        if (archive_bytes, digest) != (reviewed_bytes, reviewed_digest):
            fail(
                f"{label} canonical serializer output drifted from the reviewed "
                "ABI-21 release archive"
            )
        if digest in archive_digests or archive in archive_values:
            fail(f"{label} duplicates another canonical archive")
        archive_digests.add(digest)
        archive_values.add(archive)
        actual_profile.append((label, kind, branch_depth, peer_hops))
        derived.append((label, len(archive)))

    if tuple(actual_profile) != EXPECTED_PROFILE:
        fail(f"measurement profile drifted: {tuple(actual_profile)!r}")

    qr_rows = tuple(
        (label, size, 1 + (size + 255) // 256 + (((size + 255) // 256) + 3) // 4)
        for label, size in derived
    )
    nfc_rows = tuple(
        (label, size, (size + 219) // 220, (size + 219) // 220 + 2)
        for label, size in derived
    )
    return document, qr_rows, nfc_rows


def rust_integer(source: str, name: str) -> int:
    match = re.search(
        rf"\b{name}\s*:\s*[^=]+\s*=\s*([0-9_* ]+)\s*;",
        source,
    )
    if match is None:
        fail(f"missing Rust constant {name}")
    factors = [part.strip().replace("_", "") for part in match.group(1).split("*")]
    if not factors or any(not factor.isdigit() for factor in factors):
        fail(f"noncanonical Rust integer expression {name}")
    result = 1
    for factor in factors:
        result *= int(factor)
    return result


rust = rust_path.read_text(encoding="utf-8")
swift = swift_path.read_text(encoding="utf-8")
transport = transport_path.read_text(encoding="utf-8")
swift_qr = swift_qr_path.read_text(encoding="utf-8")
swift_nfc = swift_nfc_path.read_text(encoding="utf-8")
swift_qr_tests = swift_qr_tests_path.read_text(encoding="utf-8")
swift_nfc_tests = swift_nfc_tests_path.read_text(encoding="utf-8")
generator_source = generator_path.read_bytes()
factory_source = factory_path.read_bytes()
fixture_raw = fixture_path.read_text(encoding="utf-8")
ambient_cargo_lock = cargo_lock_path.read_bytes()
reviewed_cargo_lock = reviewed_cargo_lock_path.read_bytes()
validate_cargo_locks(ambient_cargo_lock, reviewed_cargo_lock)

raw_limit_v2 = rust_integer(
    rust, "KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2"
)
raw_limit_v4 = rust_integer(
    rust, "KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V4"
)
branch_depth = rust_integer(rust, "KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2")
peer_hop_limit = rust_integer(rust, "KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2")
tag_bytes = rust_integer(rust, "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_TAG_BYTES_V2")
bridge_abi = rust_integer(rust, "KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4")
if (
    raw_limit_v2,
    raw_limit_v4,
    branch_depth,
    peer_hop_limit,
    tag_bytes,
    bridge_abi,
) != (32_768, 32 * 1024 * 1024, 64, 8, 24, 21):
    fail(
        "unexpected ABI-21 Kagemusha bounds: "
        f"v2_raw={raw_limit_v2}, v4_raw={raw_limit_v4}, "
        f"branch_depth={branch_depth}, peer_hops={peer_hop_limit}, "
        f"transition_tag={tag_bytes}, bridge_abi={bridge_abi}"
    )

if re.search(
    r"KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE\s*:\s*bool\s*=\s*"
    r"cfg!\(feature\s*=\s*\"kagemusha-production-enabled\"\)\s*;",
    rust,
) is None:
    fail("ABI-21 backend availability must remain compile-time promotion gated")

text_limit = 12 * 1_024
prefix_bytes = 6
text_archive_limit = ((text_limit - prefix_bytes) * 3) // 4
encoded_bytes = (text_archive_limit * 4 + 2) // 3
next_encoded_bytes = ((text_archive_limit + 1) * 4 + 2) // 3
if text_archive_limit != 9_211 or encoded_bytes + prefix_bytes != text_limit:
    fail("derived single-text-envelope limit does not map exactly to 12 KiB")
if next_encoded_bytes + prefix_bytes <= text_limit:
    fail("derived single-text-envelope limit is not maximal")

for needle in (
    "public static let maximumPeerArchiveBytesV2 = 32 * 1024",
    "public static let maximumPeerArchiveBytesV4 = 32 * 1024 * 1024",
    "public static let maximumPeerArchiveBytes = maximumPeerArchiveBytesV4",
    "public static let maximumPeerTextEnvelopeBytes = 12 * 1024",
):
    if needle not in swift:
        fail(f"Swift transport bound missing: {needle}")
for prefix in ("PKK2R.", "PKK2P.", "PKK2A."):
    if f'= "{prefix}"' not in transport or len(prefix.encode("ascii")) != prefix_bytes:
        fail(f"missing canonical six-byte peer prefix: {prefix}")
if re.search(
    r"^\s*Self\(uncheckedChunkSize:\s*256,\s*parityGroup:\s*4\)\s*$",
    swift_qr,
    re.MULTILINE,
) is None:
    fail("Swift standard QR geometry must remain 256 bytes with parity group 4")
if re.search(
    r"^\s*public static let safeChunkBytes\s*=\s*220\s*$",
    swift_nfc,
    re.MULTILINE,
) is None:
    fail("Swift safe NFC chunk size must remain exactly 220 bytes")

document, qr_rows, nfc_rows = validate_fixture(
    fixture_raw,
    generator_source,
    EXPECTED_ARCHIVES,
)
for record in document["records"]:
    limit = raw_limit_v4 if record["kind"] == "payment" else raw_limit_v2
    if record["archive_bytes"] > limit:
        fail(f"{record['label']} exceeds its authoritative raw archive limit")
    if record["branch_depth"] > branch_depth or record["peer_hops"] > peer_hop_limit:
        fail(f"{record['label']} exceeds the recursive release profile")

for source, label in ((swift_qr_tests, "QR"), (swift_nfc_tests, "NFC")):
    for needle in (
        "peer_transport_measurements_v1.json",
        "archive_hex",
        "archive.count",
    ):
        if needle not in source:
            fail(
                f"Swift {label} test does not derive measurements from canonical "
                f"archives: {needle}"
            )

if self_test:
    cases: list[tuple[str, str, bytes, dict[str, tuple[int, str]]]] = []

    def rendered(value: dict[str, Any]) -> str:
        return json.dumps(value, indent=2, separators=(",", ": ")) + "\n"

    mutated = copy.deepcopy(document)
    archive_hex = mutated["records"][0]["archive_hex"]
    mutated["records"][0]["archive_hex"] = archive_hex[:-2] + (
        "00" if archive_hex[-2:] != "00" else "01"
    )
    cases.append(
        ("archive-byte-tamper", rendered(mutated), generator_source, EXPECTED_ARCHIVES)
    )

    mutated = copy.deepcopy(document)
    mutated["records"][0]["archive_hex"] = "0"
    cases.append(
        ("malformed-odd-hex", rendered(mutated), generator_source, EXPECTED_ARCHIVES)
    )

    mutated = copy.deepcopy(document)
    archive = bytes.fromhex(mutated["records"][0]["archive_hex"])
    tampered = archive[:-1] + bytes([archive[-1] ^ 1])
    mutated["records"][0]["archive_hex"] = tampered.hex()
    mutated["records"][0]["archive_sha256"] = sha256_hex(tampered)
    cases.append(
        (
            "serializer-drift-with-rebound-digest",
            rendered(mutated),
            generator_source,
            EXPECTED_ARCHIVES,
        )
    )

    mutated = copy.deepcopy(document)
    mutated["records"][0]["archive_bytes"] += 1
    cases.append(
        ("dishonest-size", rendered(mutated), generator_source, EXPECTED_ARCHIVES)
    )

    mutated = copy.deepcopy(document)
    mutated["records"][1]["label"] = mutated["records"][0]["label"]
    cases.append(
        ("duplicate-record", rendered(mutated), generator_source, EXPECTED_ARCHIVES)
    )

    mutated = copy.deepcopy(document)
    for field in ("archive_bytes", "archive_sha256", "archive_hex"):
        mutated["records"][1][field] = mutated["records"][0][field]
    cases.append(
        ("duplicate-archive", rendered(mutated), generator_source, EXPECTED_ARCHIVES)
    )

    mutated = copy.deepcopy(document)
    mutated["records"].pop()
    cases.append(
        (
            "missing-release-profile",
            rendered(mutated),
            generator_source,
            EXPECTED_ARCHIVES,
        )
    )

    mutated = copy.deepcopy(document)
    mutated["generator_sha256"] = "00" * 32
    cases.append(
        (
            "generator-digest-drift",
            rendered(mutated),
            generator_source,
            EXPECTED_ARCHIVES,
        )
    )

    mutated = copy.deepcopy(document)
    mutated["unexpected"] = True
    cases.append(
        ("extra-root-field", rendered(mutated), generator_source, EXPECTED_ARCHIVES)
    )

    duplicate_key = fixture_raw.replace(
        '  "schema":',
        f'  "schema": "{SCHEMA}",\n  "schema":',
        1,
    )
    cases.append(
        ("duplicate-json-key", duplicate_key, generator_source, EXPECTED_ARCHIVES)
    )
    cases.append(
        (
            "generator-source-drift",
            fixture_raw,
            generator_source + b"\n",
            EXPECTED_ARCHIVES,
        )
    )

    for name, raw, source, pinned in cases:
        try:
            validate_fixture(raw, source, pinned)
        except GateError:
            print(f"self-test passed: {name}")
        else:
            fail(f"adversarial self-test unexpectedly passed: {name}")

    try:
        validate_fixture(
            fixture_raw,
            generator_source,
            EXPECTED_ARCHIVES,
            factory_source + b"\n",
        )
    except GateError:
        print("self-test passed: generator-dependency-source-drift")
    else:
        fail(
            "adversarial self-test unexpectedly passed: "
            "generator-dependency-source-drift"
        )

    try:
        validate_cargo_locks(
            ambient_cargo_lock,
            reviewed_cargo_lock[:-1] + bytes([reviewed_cargo_lock[-1] ^ 1]),
        )
    except GateError:
        print("self-test passed: reviewed-cargo-lock-byte-drift")
    else:
        fail(
            "adversarial self-test unexpectedly passed: "
            "reviewed-cargo-lock-byte-drift"
        )

    try:
        validate_cargo_locks(
            ambient_cargo_lock[:-1] + bytes([ambient_cargo_lock[-1] ^ 1]),
            reviewed_cargo_lock,
        )
    except GateError:
        print("self-test passed: ambient-cargo-lock-mismatch")
    else:
        fail("adversarial self-test unexpectedly passed: ambient-cargo-lock-mismatch")

    with tempfile.TemporaryDirectory(
        prefix="kagemusha-payload-self-test-"
    ) as directory:
        stale_path = Path(directory) / "stale.json"
        stale_path.write_text(
            fixture_raw.replace('"proof_bytes": 4096', '"proof_bytes": 4095', 1),
            encoding="utf-8",
        )
        stale = subprocess.run(
            [
                "cargo",
                "run",
                "--quiet",
                "--locked",
                "--manifest-path",
                str(root / "Cargo.toml"),
                "-p",
                "iroha_data_model",
                "--features",
                "test-fixtures",
                "--bin",
                "kagemusha_peer_transport_fixtures",
                "--",
                "--check",
                str(stale_path),
            ],
            cwd=root,
            capture_output=True,
            text=True,
        )
        if stale.returncode == 0 or "is stale" not in stale.stderr:
            fail("live serializer check accepted a stale canonical fixture")
        print("self-test passed: live-serializer-rejects-stale-fixture")

print(
    "Kagemusha ABI-21 peer measurements passed: seven canonical "
    "Rust-serialized archives are source-, size-, and SHA-pinned; QR/NFC "
    "counts derive from their bytes; V2 leaf and V4 payment caps remain distinct."
)
PY
