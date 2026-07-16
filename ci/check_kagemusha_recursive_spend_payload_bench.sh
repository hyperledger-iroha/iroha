#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_PAYLOAD_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
MODE="${1:-}"

if [[ -n "${MODE}" && "${MODE}" != "--self-test" ]] || [[ $# -gt 1 ]]; then
  echo "usage: ci/check_kagemusha_recursive_spend_payload_bench.sh [--self-test]" >&2
  exit 2
fi

# Compare a live regeneration with the checked-in bytes. Source and fixture
# digests alone cannot detect a stale fixture after a Norito serializer change.
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
generator_relative = "crates/iroha_data_model/src/bin/kagemusha_peer_transport_fixtures.rs"
generator_path = root / generator_relative
factory_relative = "crates/iroha_data_model/src/offline/peer_transport_fixtures.rs"
factory_path = root / factory_relative
rust_path = root / "crates/iroha_data_model/src/offline/mod.rs"
swift_path = root / "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2.swift"
transport_path = root / "IrohaSwift/Sources/IrohaSwift/KagemushaPeerTransport.swift"
swift_qr_path = root / "IrohaSwift/Sources/IrohaSwift/KagemushaQRStream.swift"
swift_nfc_path = root / "IrohaSwift/Sources/IrohaSwift/KagemushaNFC.swift"
swift_qr_tests_path = root / "IrohaSwift/Tests/IrohaSwiftTests/KagemushaQRStreamTests.swift"
swift_nfc_tests_path = root / "IrohaSwift/Tests/IrohaSwiftTests/KagemushaNFCTests.swift"

SCHEMA = "iroha.kagemusha.peer_transport_measurements.v1"
ROOT_KEYS = {
    "schema",
    "generator",
    "generator_dependency",
    "generator_sha256",
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

# These reviewed digests make a serializer or canonical-sample change an explicit
# release-contract update. They are populated from the deterministic Rust generator,
# never from transport-test arithmetic.
EXPECTED_ARCHIVES = {
    "request": (
        614,
        "03c308b70ca7d7eb0016249f404ecb5aca5effdf92b66e00bb97aac688d8fcc9",
    ),
    "acknowledgement": (
        370,
        "dc52b0dc2ee798325ead5a72c43eb33b8b0eccb0a208ee39e760247fa245a1da",
    ),
    "payment-depth-1-hop-1": (
        7_241,
        "f9fce3291826a0073061b4e5f20082cea9f92c453f9470145875f0c5b418faa6",
    ),
    "payment-depth-8-hop-8": (
        7_412,
        "e5969b0ab4a71de94ff6225a2b00829173bff57be2cb01fb7b35fc1138ea52fd",
    ),
    "payment-depth-16-hop-8": (
        7_604,
        "22bf01df2bc65bd832f3af4a24ba27afccb70af4724fe50aa83be4c3e7d506b5",
    ),
    "payment-depth-32-hop-8": (
        7_988,
        "aaad8879c031b09e44e24112e916699a7391d30bf56744c7bfb50df62866cfb3",
    ),
    "payment-depth-64-hop-8": (
        8_756,
        "d35ec967e990c6ba0e6ca1cb9d3c042a79d8b9eed47f92f9825a395df7638bca",
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
) -> tuple[dict[str, Any], tuple[tuple[str, int, int], ...], tuple[tuple[str, int, int, int], ...]]:
    document = load_fixture(raw)
    exact_keys(document, ROOT_KEYS, "fixture")
    if document["schema"] != SCHEMA:
        fail("measurement fixture schema drifted")
    if document["generator"] != generator_relative:
        fail("measurement fixture generator path drifted")
    if document["generator_dependency"] != factory_relative:
        fail("measurement fixture generator dependency drifted")
    effective_factory_source = factory_source if factory_source_override is None else factory_source_override
    if document["generator_sha256"] != generator_digest(generator_source, effective_factory_source):
        fail("measurement fixture generator digest drifted")
    if type(document["proof_bytes"]) is not int or document["proof_bytes"] != 4_096:
        fail("canonical payment proof payload must remain exactly 4096 bytes")
    records = document["records"]
    if not isinstance(records, list) or len(records) != len(EXPECTED_PROFILE):
        fail("measurement fixture record inventory has the wrong cardinality")
    if set(expected_archives) != {profile[0] for profile in EXPECTED_PROFILE}:
        fail("gate has no exact reviewed digest inventory for every canonical archive")

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
        digest = canonical_hex(record["archive_sha256"], f"{label}.archive_sha256", 32).hex()
        if digest != sha256_hex(archive):
            fail(f"{label} archive digest does not match its serialized bytes")
        if label not in expected_archives:
            fail(f"unreviewed canonical archive label {label!r}")
        reviewed_bytes, reviewed_digest = expected_archives[label]
        if (archive_bytes, digest) != (reviewed_bytes, reviewed_digest):
            fail(f"{label} canonical serializer output drifted from the reviewed release archive")
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
    match = re.search(rf"\b{name}\s*:\s*[^=]+\s*=\s*([0-9_]+)\s*;", source)
    if match is None:
        fail(f"missing Rust constant {name}")
    return int(match.group(1).replace("_", ""))


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

raw_limit = rust_integer(rust, "KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2")
branch_depth = rust_integer(rust, "KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2")
peer_hop_limit = rust_integer(rust, "KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2")
tag_bytes = rust_integer(rust, "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_TAG_BYTES_V2")
if (raw_limit, branch_depth, peer_hop_limit, tag_bytes) != (32_768, 64, 8, 24):
    fail(
        "unexpected Kagemusha peer bounds: "
        f"raw={raw_limit}, branch_depth={branch_depth}, "
        f"peer_hops={peer_hop_limit}, transition_tag={tag_bytes}"
    )

backend = re.search(
    r"KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE\s*:\s*bool\s*=\s*(true|false)\s*;",
    rust,
)
if backend is None or backend.group(1) != "false":
    fail(
        "production Kagemusha must remain unavailable until the exact full-archive "
        "depth-64/eight-peer-hop relation is backed by the production proof implementation"
    )

text_limit = 12 * 1_024
prefix_bytes = 6
text_archive_limit = ((text_limit - prefix_bytes) * 3) // 4
encoded_bytes = (text_archive_limit * 4 + 2) // 3
next_encoded_bytes = ((text_archive_limit + 1) * 4 + 2) // 3
if text_archive_limit != 9_211 or encoded_bytes + prefix_bytes != text_limit:
    fail("derived text-transport archive limit does not map exactly to 12 KiB")
if next_encoded_bytes + prefix_bytes <= text_limit:
    fail("derived text-transport archive limit is not maximal")
if raw_limit <= text_archive_limit:
    fail("protocol raw archive cap must remain independent of the text sub-cap")

for needle in (
    "public static let maximumPeerArchiveBytes = 32_768",
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
    fail("Swift standard QR geometry must remain exactly 256 bytes with parity group 4")
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
for label, size, _ in qr_rows:
    if size > raw_limit or size > text_archive_limit:
        fail(f"canonical archive exceeds a first-release transport cap: {label}")

for source, label in ((swift_qr_tests, "QR"), (swift_nfc_tests, "NFC")):
    for needle in (
        "peer_transport_measurements_v1.json",
        "archive_hex",
        "archive.count",
    ):
        if needle not in source:
            fail(f"Swift {label} test does not derive measurements from canonical archives: {needle}")

if self_test:
    cases: list[tuple[str, str, bytes, dict[str, tuple[int, str]]]] = []

    def rendered(value: dict[str, Any]) -> str:
        return json.dumps(value, indent=2, separators=(",", ": ")) + "\n"

    mutated = copy.deepcopy(document)
    archive_hex = mutated["records"][0]["archive_hex"]
    mutated["records"][0]["archive_hex"] = archive_hex[:-2] + ("00" if archive_hex[-2:] != "00" else "01")
    cases.append(("archive-byte-tamper", rendered(mutated), generator_source, EXPECTED_ARCHIVES))

    mutated = copy.deepcopy(document)
    mutated["records"][0]["archive_hex"] = "0"
    cases.append(("malformed-odd-hex", rendered(mutated), generator_source, EXPECTED_ARCHIVES))

    mutated = copy.deepcopy(document)
    archive = bytes.fromhex(mutated["records"][0]["archive_hex"])
    tampered = archive[:-1] + bytes([archive[-1] ^ 1])
    mutated["records"][0]["archive_hex"] = tampered.hex()
    mutated["records"][0]["archive_sha256"] = sha256_hex(tampered)
    cases.append(("serializer-drift-with-rebound-digest", rendered(mutated), generator_source, EXPECTED_ARCHIVES))

    mutated = copy.deepcopy(document)
    mutated["records"][0]["archive_bytes"] += 1
    cases.append(("dishonest-size", rendered(mutated), generator_source, EXPECTED_ARCHIVES))

    mutated = copy.deepcopy(document)
    mutated["records"][1]["label"] = mutated["records"][0]["label"]
    cases.append(("duplicate-record", rendered(mutated), generator_source, EXPECTED_ARCHIVES))

    mutated = copy.deepcopy(document)
    for field in ("archive_bytes", "archive_sha256", "archive_hex"):
        mutated["records"][1][field] = mutated["records"][0][field]
    cases.append(("duplicate-archive", rendered(mutated), generator_source, EXPECTED_ARCHIVES))

    mutated = copy.deepcopy(document)
    mutated["records"].pop()
    cases.append(("missing-release-profile", rendered(mutated), generator_source, EXPECTED_ARCHIVES))

    mutated = copy.deepcopy(document)
    mutated["generator_sha256"] = "00" * 32
    cases.append(("generator-digest-drift", rendered(mutated), generator_source, EXPECTED_ARCHIVES))

    mutated = copy.deepcopy(document)
    mutated["unexpected"] = True
    cases.append(("extra-root-field", rendered(mutated), generator_source, EXPECTED_ARCHIVES))

    duplicate_key = fixture_raw.replace(
        '  "schema":',
        f'  "schema": "{SCHEMA}",\n  "schema":',
        1,
    )
    cases.append(("duplicate-json-key", duplicate_key, generator_source, EXPECTED_ARCHIVES))

    cases.append(("generator-source-drift", fixture_raw, generator_source + b"\n", EXPECTED_ARCHIVES))

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
        fail("adversarial self-test unexpectedly passed: generator-dependency-source-drift")

    with tempfile.TemporaryDirectory(prefix="kagemusha-payload-self-test-") as directory:
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
    "Kagemusha peer transport measurements passed: seven canonical Rust-serialized "
    "archives are generator- and SHA-pinned; QR/NFC counts are derived from their bytes; "
    "the protocol permits 32,768 raw bytes and the 12 KiB text envelope permits 9,211."
)
PY
