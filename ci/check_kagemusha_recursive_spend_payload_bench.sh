#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_PAYLOAD_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"

if [[ $# -ne 0 ]]; then
  echo "error: synthetic and negative-control payload modes are not part of the first release" >&2
  exit 2
fi

python3 - "${ROOT_DIR}" <<'PY'
from pathlib import Path
import re
import sys

root = Path(sys.argv[1])
model_path = root / "crates/iroha_data_model/src/offline/mod.rs"
model_fragment_path = root / "crates/iroha_data_model/src/offline/kagemusha_model.rs"
model_include = 'include!("kagemusha_model.rs");'
verifier_path = root / "crates/iroha_data_model/src/offline/kagemusha_release_verifier.rs"
verifier_module = "mod kagemusha_release_verifier;"
model_parent = model_path.read_text(encoding="utf-8")
if model_parent.count(model_include) != 1:
    raise SystemExit(
        f"{model_path}: expected exactly one reviewed {model_fragment_path.name} include"
    )
rust = model_parent.replace(
    model_include,
    model_fragment_path.read_text(encoding="utf-8"),
    1,
)
if rust.count(verifier_module) != 1:
    raise SystemExit(
        f"{model_path}: expected exactly one reviewed {verifier_path.name} module"
    )
verifier = verifier_path.read_text(encoding="utf-8")
for marker in (
    "const VERIFIER_IDENTITY_SCHEMA_V4",
    "pub fn kagemusha_recursive_spend_verifier_key_id_v4",
):
    if verifier.count(marker) != 1:
        raise SystemExit(f"{verifier_path}: expected exactly one {marker!r}")
rust = rust.replace(
    verifier_module,
    "mod kagemusha_release_verifier {\n" + verifier + "\n}",
    1,
)
swift = (root / "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2.swift").read_text(encoding="utf-8")
transport = (root / "IrohaSwift/Sources/IrohaSwift/KagemushaPeerTransport.swift").read_text(encoding="utf-8")
swift_qr_tests = (root / "IrohaSwift/Tests/IrohaSwiftTests/KagemushaQRStreamTests.swift").read_text(encoding="utf-8")
swift_nfc_tests = (root / "IrohaSwift/Tests/IrohaSwiftTests/KagemushaNFCTests.swift").read_text(encoding="utf-8")
kotlin_prover = (root / "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt").read_text(encoding="utf-8")
kotlin_transport = (root / "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaPeerTransport.kt").read_text(encoding="utf-8")
java_prover = (root / "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java").read_text(encoding="utf-8")
java_transport = (root / "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaPeerTransport.java").read_text(encoding="utf-8")

def rust_integer(name: str) -> int:
    match = re.search(rf"\b{name}\s*:\s*[^=]+\s*=\s*([0-9_* ]+)\s*;", rust)
    if match is None:
        raise SystemExit(f"missing Rust constant {name}")
    factors = [part.strip().replace("_", "") for part in match.group(1).split("*")]
    if not factors or any(not factor.isdigit() for factor in factors):
        raise SystemExit(f"noncanonical Rust integer expression {name}")
    result = 1
    for factor in factors:
        result *= int(factor)
    return result

raw_limit = rust_integer("KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2")
raw_limit_v4 = rust_integer("KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V4")
branch_depth = rust_integer("KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2")
peer_hop_limit = rust_integer("KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2")
tag_bytes = rust_integer("KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_TAG_BYTES_V2")
if (raw_limit, raw_limit_v4, branch_depth, peer_hop_limit, tag_bytes) != (
    32_768,
    32 * 1024 * 1024,
    64,
    8,
    24,
):
    raise SystemExit(
        "unexpected Kagemusha peer bounds: "
        f"v2_raw={raw_limit}, v4_raw={raw_limit_v4}, branch_depth={branch_depth}, "
        f"peer_hops={peer_hop_limit}, transition_tag={tag_bytes}"
    )

backend = re.search(
    r"pub const KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE:\s*bool\s*=\s*"
    r'cfg!\(feature\s*=\s*"kagemusha-production-enabled"\)\s*;',
    rust,
)
if backend is None:
    raise SystemExit(
        "Kagemusha proof-backend availability must remain compile-time promotion gated"
    )

text_limit = 12 * 1_024
prefix_bytes = 6
text_archive_limit = ((text_limit - prefix_bytes) * 3) // 4
encoded_bytes = (text_archive_limit * 4 + 2) // 3
next_encoded_bytes = ((text_archive_limit + 1) * 4 + 2) // 3
if text_archive_limit != 9_211 or encoded_bytes + prefix_bytes != text_limit:
    raise SystemExit(
        "derived text-transport archive limit does not map exactly to 12 KiB: "
        f"raw={text_archive_limit}, text={encoded_bytes + prefix_bytes}"
    )
if next_encoded_bytes + prefix_bytes <= text_limit:
    raise SystemExit("derived text-transport archive limit is not maximal")
if raw_limit <= text_archive_limit:
    raise SystemExit("protocol raw archive cap must remain independent of the text sub-cap")

for needle in (
    "public static let maximumPeerArchiveBytesV2 = 32 * 1024",
    "public static let maximumPeerArchiveBytesV4 = 32 * 1024 * 1024",
    "public static let maximumPeerArchiveBytes = maximumPeerArchiveBytesV4",
    "public static let maximumPeerTextEnvelopeBytes = 12 * 1024",
):
    if needle not in swift:
        raise SystemExit(f"Swift transport bound missing: {needle}")
for prefix in ("PKK2R.", "PKK2P.", "PKK2A."):
    if f'= "{prefix}"' not in transport or len(prefix.encode("ascii")) != 6:
        raise SystemExit(f"missing canonical six-byte peer prefix: {prefix}")

mobile_text_contracts = (
    (
        "Kotlin",
        kotlin_prover,
        kotlin_transport,
        "const val MAX_PEER_TEXT_ENVELOPE_BYTES: Int = 12 * 1024",
        "(MAX_PEER_TEXT_ENVELOPE_BYTES - 6) * 3 / 4",
        "archive.size <= KagemushaRecursiveSpendProver.MAX_PEER_TEXT_ARCHIVE_BYTES",
    ),
    (
        "Java",
        java_prover,
        java_transport,
        "public static final int MAX_PEER_TEXT_ENVELOPE_BYTES = 12 * 1024;",
        "(MAX_PEER_TEXT_ENVELOPE_BYTES - 6) * 3 / 4;",
        "archive.length > KagemushaRecursiveSpendProver.MAX_PEER_TEXT_ARCHIVE_BYTES",
    ),
)
for sdk, prover_source, transport_source, envelope, archive, enforcement in mobile_text_contracts:
    if envelope not in prover_source or archive not in prover_source:
        raise SystemExit(f"{sdk} direct-text transport bounds differ from 12 KiB / 9,211 bytes")
    if transport_source.count(enforcement) != 2:
        raise SystemExit(f"{sdk} direct-text encode/decode archive enforcement is incomplete")

for source, transport_name, needle in (
    (swift_qr_tests, "QR", '("payment-v4-peer-hop-1", 12_896, 65),'),
    (swift_nfc_tests, "NFC", '("payment-v4-peer-hop-1", 12_896, 59, 61),'),
):
    if needle not in source:
        raise SystemExit(f"Swift {transport_name} measurement drifted: {needle}")
for retired_hop in (16, 32, 64):
    needle = f"hop-{retired_hop}"
    if needle in swift_qr_tests or needle in swift_nfc_tests:
        raise SystemExit(f"Swift transport measurements exceed the protocol hop cap: {needle}")

print(
    "Kagemusha peer transport bounds are internally consistent: "
    "the ABI-21/V4 archive permits 32 MiB, the canonical 12,896-byte ABI-21 peer-payment "
    "fixture remains pinned in the QR and NFC transport tests, and the "
    "12 KiB text envelope derives an independent 9,211-byte raw sub-cap; runtime proof use "
    "still requires the authenticated installed ABI-21/V4 artifact set and promotion evidence."
)
PY
