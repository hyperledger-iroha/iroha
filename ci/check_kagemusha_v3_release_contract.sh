#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_V3_RELEASE_CONTRACT_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"

python3 - "${ROOT_DIR}" <<'PY'
from pathlib import Path
import re
import sys

root = Path(sys.argv[1])
errors: list[str] = []

def read(relative: str) -> str:
    path = root / relative
    if not path.is_file():
        errors.append(f"missing required file: {relative}")
        return ""
    return path.read_text(encoding="utf-8")

def require(relative: str, text: str, *needles: str) -> None:
    for needle in needles:
        if needle not in text:
            errors.append(f"{relative}: missing {needle!r}")

model_path = "crates/iroha_data_model/src/offline/mod.rs"
packager_path = "crates/iroha_core/src/bin/kagemusha_recursive_spend_v3_bundle.rs"
bridge_path = "crates/connect_norito_bridge/src/lib.rs"
header_path = "crates/connect_norito_bridge/include/connect_norito_bridge.h"
build_path = "scripts/build_norito_xcframework.sh"
model = read(model_path)
packager = read(packager_path)
bridge = read(bridge_path)
header = read(header_path)
build = read(build_path)

require(
    model_path,
    model,
    "KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3: u32 = 19",
    'KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3: &str =\n    "kagemusha.offline.recursive_spend.artifact_manifest.v3"',
    'KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V2: &str = "topup-finality-roster.norito"',
)

artifact_constants = {
    "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PARAMETERS_FILE_NAME_V3": "transition-eq.parameters.krv3",
    "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROVING_KEY_FILE_NAME_V3": "transition-eq.proving-key.krv3",
    "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_VERIFYING_KEY_FILE_NAME_V3": "transition-eq.verifying-key.krv3",
    "KAGEMUSHA_RECURSIVE_SPEND_STATE_PARAMETERS_FILE_NAME_V3": "state-ep.parameters.krv3",
    "KAGEMUSHA_RECURSIVE_SPEND_STATE_PROVING_KEY_FILE_NAME_V3": "state-ep.proving-key.krv3",
    "KAGEMUSHA_RECURSIVE_SPEND_STATE_VERIFYING_KEY_FILE_NAME_V3": "state-ep.verifying-key.krv3",
}
declared_artifact_constants = {
    name: value
    for name, value in re.findall(
        r'pub const (KAGEMUSHA_RECURSIVE_SPEND_[A-Z_]+FILE_NAME_V3):\s*&str\s*=\s*\n?\s*"([^"]+)";',
        model,
    )
}
if declared_artifact_constants != artifact_constants:
    errors.append(
        f"{model_path}: V3 proof-key inventory must be exactly six files; "
        f"found {declared_artifact_constants}"
    )

inputs = re.search(
    r"const INPUTS:\s*&\[InputSpec\]\s*=\s*&\[(?P<body>[\s\S]*?)\n\];",
    packager,
)
if inputs is None:
    errors.append(f"{packager_path}: missing exact INPUTS inventory")
else:
    body = inputs.group("body")
    if body.count("InputSpec {") != 6:
        errors.append(f"{packager_path}: INPUTS must contain exactly six proof-key artifacts")
    for name in artifact_constants:
        if body.count(name) != 1:
            errors.append(f"{packager_path}: INPUTS must contain {name} exactly once")
require(
    packager_path,
    packager,
    'required(options, "topup-finality-roster")',
    "KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V2",
    "KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2",
)

require(bridge_path, bridge, "CONNECT_NORITO_BRIDGE_ABI_VERSION: u32 = 19")
release_symbols = (
    "connect_norito_bridge_abi_version",
    "connect_norito_kagemusha_recursive_spend_capabilities_v1",
    "connect_norito_kagemusha_recursive_spend_artifact_begin_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_write_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_finalize_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_cancel_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_set_install_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v3",
)
for symbol in release_symbols:
    require(bridge_path, bridge, symbol)
    require(header_path, header, symbol)

require(
    build_path,
    build,
    "--privacy-production-enabled",
    '"ios-arm64"',
    '"ios-arm64_x86_64-simulator"',
    '"macos-arm64"',
    '"connect_norito_bridge_abi_version"',
    '"connect_norito_kagemusha_recursive_spend_artifact_set_install_v3"',
    '"source_commit"',
    '"bridge_header_sha256"',
)

for relative in (
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt",
):
    text = read(relative)
    require(
        relative,
        text,
        "REQUIRED_NATIVE_BRIDGE_ABI_VERSION",
        "19",
        '"kagemusha.offline.recursive_spend.artifact_manifest.v3"',
    )
    for file_name in artifact_constants.values():
        require(relative, text, file_name)

if errors:
    print("Kagemusha V3 release contract failed:", file=sys.stderr)
    for error in errors:
        print(f" - {error}", file=sys.stderr)
    raise SystemExit(1)
print("Kagemusha V3 release contract passed: ABI 19, six proof keys, one roster.")
PY
