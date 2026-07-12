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

def require_absent(path: str) -> None:
    if (root / path).exists():
        errors.append(f"retired path still exists: {path}")

for path in (
    "fixtures/kagemusha_recursive_spend_abi6",
    "fixtures/kagemusha_recursive_spend_abi7",
    "fixtures/offline",
    "kotlin/offline-wallet-android",
    "kotlin/offline-wallet-lab-app",
    "javascript/iroha_js/src/offlineApi.js",
    "javascript/iroha_js/src/offlineCashLifecycle.js",
    "javascript/iroha_js/src/offlineQrStream.js",
    "python/iroha_python/src/iroha_python/kagemusha.py",
    "python/iroha_python/src/iroha_python/offline_cash.py",
    "ci/check_kagemusha_production_readiness.sh",
    "scripts/kagemusha_production_readiness.py",
    "scripts/kagemusha_recursive_compact_key_evidence.py",
    "scripts/kagemusha_run_recursive_compact_keygen_staged.py",
    "scripts/kagemusha_finalize_recursive_compact_key_staged_run.py",
    "scripts/kagemusha_lineage_proof_evidence.py",
    "scripts/kagemusha_run_lineage_proof_staged.py",
    "scripts/kagemusha_finalize_lineage_proof_staged_run.py",
):
    require_absent(path)

java_main = root / "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline"
kotlin_main = root / "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline"
allowed_jvm = {
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
if actual_jvm != allowed_jvm:
    errors.append(
        "JVM offline source inventory must contain only ABI-18 artifact bridges; "
        f"found {sorted(actual_jvm)}"
    )

for path in (root / "csharp/src").rglob("*"):
    if path.is_file() and "Offline" in path.parts:
        errors.append(f"C# offline lifecycle file is forbidden: {relative(path)}")

text_scopes = [
    root / "crates/iroha_data_model/src/offline",
    root / "crates/iroha_core/src/zk",
    root / "crates/iroha_torii/src",
    root / "crates/iroha_torii_shared/src",
    root / "IrohaSwift/Sources/IrohaSwift",
    root / "crates/iroha_js_host/src",
    root / "javascript/iroha_js/src",
    root / "javascript/iroha_js/index.d.ts",
    root / "python/iroha_python/src/iroha_python",
    root / "python/iroha_python/iroha_python_rs/src",
    root / "csharp/src",
]
source_files: list[Path] = []
for scope in text_scopes:
    if scope.is_file():
        source_files.append(scope)
    elif scope.exists():
        source_files.extend(
            path
            for path in scope.rglob("*")
            if path.is_file() and path.suffix in {".rs", ".swift", ".js", ".ts", ".py", ".cs"}
        )

retired_patterns = {
    "Offline Note protocol": re.compile(r"\bOfflineNote\b|\bBearerOffline|offline[_ -]note", re.I),
    "compact-payment prototype": re.compile(r"KagemushaCompactPayment|recursive[_ -]compact|compact[_ -]payment", re.I),
    "recursive-aggregation prototype": re.compile(r"KagemushaRecursiveAggregation|recursive[_ -]aggregation", re.I),
    "semantic-lineage fallback": re.compile(r"semantic[_ -]lineage", re.I),
    "V1 recursive-spend contract": re.compile(r"KagemushaRecursiveSpend(?:Bundle|Accumulator|InitRequest|AppendRequest|VerifyRequest|RedeemRequest)V1"),
}
for path in source_files:
    # Generic queued transaction envelopes are not an offline-cash protocol.
    if "/tx/offline/" in f"/{relative(path)}":
        continue
    text = path.read_text(encoding="utf-8", errors="replace")
    for label, pattern in retired_patterns.items():
        match = pattern.search(text)
        if match is not None:
            line = text.count("\n", 0, match.start()) + 1
            errors.append(f"{relative(path)}:{line}: retired {label}: {match.group(0)!r}")

torii_files = (
    root / "crates/iroha_torii/src/lib.rs",
    root / "crates/iroha_torii/src/openapi.rs",
    root / "crates/iroha_torii/src/app_auth.rs",
)
torii_text = "\n".join(path.read_text(encoding="utf-8") for path in torii_files)
for route in (
    "/v1/offline/readiness",
    "/v1/offline/top-up",
    "/v1/offline/redeem",
    "/v1/offline/operations/{operation_id}",
):
    if route not in torii_text:
        errors.append(f"missing first-release Torii route: {route}")
for retired_route in (
    "/v1/offline/v2",
    "/v1/offline/notes",
    "/v1/offline/issuer",
    "/v1/offline/audit",
):
    if retired_route in torii_text:
        errors.append(f"retired Torii route remains: {retired_route}")
for wrapper in ("topup_request_norito_base64", "redeem_request_norito_base64"):
    if wrapper in torii_text:
        errors.append(f"retired base64 request wrapper remains: {wrapper}")

package_json = (root / "javascript/iroha_js/package.json").read_text(encoding="utf-8")
if '"./offline-cash"' in package_json:
    errors.append("JavaScript package still exports ./offline-cash")
python_init = (root / "python/iroha_python/src/iroha_python/__init__.py").read_text(encoding="utf-8")
if re.search(r"kagemusha|offline_cash", python_init, re.I):
    errors.append("Python package root still exports an offline lifecycle")

workflow = (root / ".github/workflows/pr_kagemusha_payload_bench.yml").read_text(encoding="utf-8")
if re.search(r"negative-control|production_readiness|recursive_compact|semantic[_ -]lineage", workflow, re.I):
    errors.append("Kagemusha workflow still invokes legacy compatibility/negative-control machinery")

if errors:
    print("Kagemusha first-release policy failed:", file=sys.stderr)
    for error in errors:
        print(f" - {error}", file=sys.stderr)
    raise SystemExit(1)
print("Kagemusha first-release policy passed: one protocol, one route set, one artifact set.")
PY
