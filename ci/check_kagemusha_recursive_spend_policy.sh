#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_POLICY_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"

"${ROOT_DIR}/ci/check_kagemusha_v3_release_contract.sh"
"${ROOT_DIR}/ci/check_kagemusha_recursive_spend_sdk_parity.sh"
"${ROOT_DIR}/ci/check_kagemusha_recursive_spend_payload_bench.sh"

python3 - "${ROOT_DIR}" <<'PY'
from pathlib import Path
import re
import sys

root = Path(sys.argv[1])
errors: list[str] = []


def relative(path: Path) -> str:
    return path.relative_to(root).as_posix()


# The JVM boundary has one exact, artifact-only Kagemusha source per language.
java_main = root / "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline"
kotlin_main = root / "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline"
expected_jvm = {
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt",
}
actual_jvm = {
    relative(path)
    for directory in (java_main, kotlin_main)
    if directory.exists()
    for path in directory.iterdir()
    if path.is_file()
}
if actual_jvm != expected_jvm:
    errors.append(
        "JVM Kagemusha source inventory mismatch: "
        f"missing={sorted(expected_jvm - actual_jvm)}, "
        f"extra={sorted(actual_jvm - expected_jvm)}"
    )

for source in sorted(expected_jvm):
    text = (root / source).read_text(encoding="utf-8")
    for literal in (
        "REQUIRED_NATIVE_BRIDGE_ABI_VERSION",
        "19",
        'ARTIFACT_MANIFEST_MODE',
        'recursive_spend_v1',
        'kagemusha.offline.recursive_spend.artifact_manifest.v3',
    ):
        if literal not in text:
            errors.append(f"{source}: missing exact Kagemusha contract literal {literal!r}")

# The route catalog is the authoritative public inventory. It contains exactly
# four Kagemusha routes and exactly one descriptor for each route.
catalog_path = root / "crates/iroha_torii_shared/src/route_catalog.rs"
catalog = catalog_path.read_text(encoding="utf-8")
try:
    offline_catalog = catalog.split("pub mod offline {", 1)[1].split("\n}\n", 1)[0]
except IndexError:
    errors.append("Torii route catalog has no offline module")
    offline_catalog = ""

expected_paths = {
    "READINESS": "/v1/offline/readiness",
    "TOP_UP": "/v1/offline/top-up",
    "REDEEM": "/v1/offline/redeem",
    "OPERATION": "/v1/offline/operations/{operation_id}",
}
actual_paths = dict(
    re.findall(r'pub const ([A-Z_]+)_PATH: &str = "([^"]+)";', offline_catalog)
)
if actual_paths != expected_paths:
    errors.append(
        "Torii Kagemusha route inventory mismatch: "
        f"expected={expected_paths}, actual={actual_paths}"
    )
if "pub const ROUTES: &[RouteDescriptor] = &[READINESS, TOP_UP, REDEEM, OPERATION];" not in offline_catalog:
    errors.append("Torii Kagemusha descriptor inventory is not exact")

torii_path = root / "crates/iroha_torii/src/lib.rs"
torii = torii_path.read_text(encoding="utf-8")
expected_registrations = {
    "READINESS": "catalog_get(handler_offline_readiness)",
    "TOP_UP": "catalog_post(handler_offline_top_up)",
    "REDEEM": "catalog_post(handler_offline_redeem)",
    "OPERATION": "catalog_get(handler_offline_operation_status)",
}
for descriptor, handler in expected_registrations.items():
    marker = f"&route_catalog::offline::{descriptor}"
    if torii.count(marker) != 1:
        errors.append(f"Torii must register {marker} exactly once")
    if handler not in torii:
        errors.append(f"Torii is missing exact Kagemusha handler {handler}")

# Generated OpenAPI must expose the same exact path set.
openapi_path = root / "docs/portal/static/openapi/torii.json"
import json

openapi = json.loads(openapi_path.read_text(encoding="utf-8"))
actual_openapi_paths = {
    path for path in openapi.get("paths", {}) if path.startswith("/v1/offline/")
}
expected_openapi_paths = set(expected_paths.values())
if actual_openapi_paths != expected_openapi_paths:
    errors.append(
        "OpenAPI Kagemusha route inventory mismatch: "
        f"missing={sorted(expected_openapi_paths - actual_openapi_paths)}, "
        f"extra={sorted(actual_openapi_paths - expected_openapi_paths)}"
    )

# Native release scripts own the exact exported-symbol inventory. Pin the
# shared capability identity here so route, SDK, and artifact checks cannot
# silently select different Kagemusha contracts.
model_path = root / "crates/iroha_data_model/src/offline/mod.rs"
model = model_path.read_text(encoding="utf-8")
for literal in (
    'KAGEMUSHA_RECURSIVE_SPEND_MODE: &str = "recursive_spend_v1"',
    'KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3: u32 = 19',
    '"kagemusha.offline.recursive_spend.artifact_manifest.v3"',
):
    if literal not in model:
        errors.append(f"data-model Kagemusha contract is missing {literal!r}")

if errors:
    print("Kagemusha first-release policy failed:", file=sys.stderr)
    for error in errors:
        print(f" - {error}", file=sys.stderr)
    raise SystemExit(1)
print("Kagemusha first-release policy passed: one protocol, one route set, one artifact set.")
PY
