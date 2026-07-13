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
rust = (root / "crates/iroha_data_model/src/offline/mod.rs").read_text(encoding="utf-8")
swift = (root / "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2.swift").read_text(encoding="utf-8")
transport = (root / "IrohaSwift/Sources/IrohaSwift/KagemushaPeerTransport.swift").read_text(encoding="utf-8")
core_transition = (root / "crates/iroha_core/src/zk/kagemusha_v2.rs").read_text(encoding="utf-8")
swift_qr_tests = (root / "IrohaSwift/Tests/IrohaSwiftTests/KagemushaQRStreamTests.swift").read_text(encoding="utf-8")
swift_nfc_tests = (root / "IrohaSwift/Tests/IrohaSwiftTests/KagemushaNFCTests.swift").read_text(encoding="utf-8")

def rust_integer(name: str) -> int:
    match = re.search(rf"\b{name}\s*:\s*[^=]+\s*=\s*([0-9_]+)\s*;", rust)
    if match is None:
        raise SystemExit(f"missing Rust constant {name}")
    return int(match.group(1).replace("_", ""))

raw_limit = rust_integer("KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2")
branch_depth = rust_integer("KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2")
peer_hop_limit = rust_integer("KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2")
tag_bytes = rust_integer("KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_TAG_BYTES_V2")
if (raw_limit, branch_depth, peer_hop_limit, tag_bytes) != (9_211, 64, 64, 24):
    raise SystemExit(
        "unexpected Kagemusha peer bounds: "
        f"raw={raw_limit}, branch_depth={branch_depth}, "
        f"peer_hops={peer_hop_limit}, transition_tag={tag_bytes}"
    )

backend = re.search(
    r"KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE\s*:\s*bool\s*=\s*(true|false)\s*;",
    rust,
)
if backend is None or backend.group(1) != "false":
    raise SystemExit(
        "production Kagemusha must remain unavailable until an exact full-archive "
        "depth-64/64-peer-hop archive is backed by the production proof implementation"
    )

encoded_bytes = (raw_limit * 4 + 2) // 3
if encoded_bytes + 6 != 12 * 1_024:
    raise SystemExit(
        f"raw peer archive limit does not map exactly to 12 KiB: {encoded_bytes + 6}"
    )

for needle in (
    "public static let maximumPeerArchiveBytes = 9_211",
    "public static let maximumPeerTextEnvelopeBytes = 12 * 1024",
):
    if needle not in swift:
        raise SystemExit(f"Swift transport bound missing: {needle}")
for prefix in ("PKK2R.", "PKK2P.", "PKK2A."):
    if f'= "{prefix}"' not in transport or len(prefix.encode("ascii")) != 6:
        raise SystemExit(f"missing canonical six-byte peer prefix: {prefix}")

for needle in (
    "fn complete_peer_archives_fit_real_text_qr_and_nfc_limits_through_branch_depth_64()",
    "(request_archive.len(), request_text.len()), (824, 1_105)",
    "(64, 64, 8_192, 10_929, 41, 50, 52)",
    "(acknowledgement_archive.len(), acknowledgement_text.len()),",
    "(471, 634)",
):
    if needle not in core_transition:
        raise SystemExit(f"authoritative typed peer-archive size pin missing: {needle}")

for depth, peer_hops, raw_bytes, text_bytes in (
    (1, 1, "6_677", "8_909"),
    (8, 8, "6_848", "9_137"),
    (16, 16, "7_040", "9_393"),
    (32, 32, "7_424", "9_905"),
    (64, 64, "8_192", "10_929"),
):
    qr_needle = f'(\"payment-depth-{depth}-hop-{peer_hops}\", {raw_bytes},'
    nfc_needle = f'(\"payment-depth-{depth}-hop-{peer_hops}\", {text_bytes},'
    if qr_needle not in swift_qr_tests:
        raise SystemExit(f"Swift QR measurement drifted: {qr_needle}")
    if nfc_needle not in swift_nfc_tests:
        raise SystemExit(f"Swift NFC measurement drifted: {nfc_needle}")

print(
    "Kagemusha peer transport bounds are internally consistent: "
    "validated depth-64/64-peer-hop payment is 8,192 raw / 10,929 text bytes and "
    "9,211 raw bytes maps to the exact 12 KiB ceiling; production remains "
    "unavailable pending the recursive proof backend."
)
PY
