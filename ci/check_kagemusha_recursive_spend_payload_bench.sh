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

def rust_integer(name: str) -> int:
    match = re.search(rf"\b{name}\s*:\s*[^=]+\s*=\s*([0-9_]+)\s*;", rust)
    if match is None:
        raise SystemExit(f"missing Rust constant {name}")
    return int(match.group(1).replace("_", ""))

raw_limit = rust_integer("KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2")
hop_limit = rust_integer("KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2")
tag_bytes = rust_integer("KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_TAG_BYTES_V2")
if (raw_limit, hop_limit, tag_bytes) != (9_211, 64, 24):
    raise SystemExit(
        "unexpected Kagemusha peer bounds: "
        f"raw={raw_limit}, hops={hop_limit}, transition_tag={tag_bytes}"
    )

backend = re.search(
    r"KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE\s*:\s*bool\s*=\s*(true|false)\s*;",
    rust,
)
if backend is None or backend.group(1) != "false":
    raise SystemExit(
        "production Kagemusha must remain unavailable until an exact full-archive "
        "hop-64 benchmark fits the peer limit"
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

print(
    "Kagemusha peer transport bounds are internally consistent: "
    "9,211 raw bytes -> 12 KiB text; production remains unavailable pending "
    "an exact full-archive hop-64 benchmark."
)
PY
