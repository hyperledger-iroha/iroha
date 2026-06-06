#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_PRODUCTION_READINESS_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
MODE="${1:-}"

python3 - "$ROOT_DIR" "$MODE" <<'PY'
import json
import sys
from pathlib import Path

root = Path(sys.argv[1])
mode = sys.argv[2]
text_overrides: dict[str, str] = {}

ABI6_SYMBOLS = (
    "connect_norito_kagemusha_recursive_spend_init",
    "connect_norito_kagemusha_recursive_spend_append",
    "connect_norito_kagemusha_recursive_spend_transition_profile_init",
    "connect_norito_kagemusha_recursive_spend_transition_profile_append",
    "connect_norito_kagemusha_recursive_spend_lineage_append_boundary",
    "connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result",
    "connect_norito_kagemusha_recursive_spend_lineage_witness_append_result",
    "connect_norito_kagemusha_recursive_spend_verify",
    "connect_norito_kagemusha_recursive_spend_redeem",
)

TEXT_REQUIREMENTS = {
    "roadmap.md": (
        "Reserved-lineage recursive spend path",
        "ABI-6 verify",
        "request archives now fail closed at the C bridge",
        "ABI-7",
        "and fail closed while core projection tests bind folded public-input hash",
        "Remaining compact-token release work is to replace the semantic aggregation",
        "proof with a composed private-hop verifier-slice proof before enabling",
        "receiver admission or SDK default selection",
    ),
    "docs/source/offline_kagemusha.md": (
        "The reserved `kagemusha-recursive-spend-lineage-v1` profile is the enabled",
        "witnessless chain-admission path for constant-size lineage proofs inside the",
        "64-hop cap",
        "The routine offline-offline production path",
        "uses the ABI-6 reserved-lineage recursive spend verifier and redemption surface",
        "ABI-7 recursive compact-token symbols remain fail-closed until that proof",
        "uses the composed private-hop verifier-slice circuit",
    ),
    "crates/iroha_data_model/src/offline/mod.rs": (
        "pub const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1: u32 = 64;",
        "KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1",
        "This mode is intentionally not selected by production defaults",
        "preferred_kagemusha_offline_spend_mode_for_capabilities(false, recursive_spend_available)",
        "_recursive_compact_available: bool",
        "if recursive_spend_available",
        "KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1",
        "KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1",
        "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1",
        "hop_count <= KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1",
    ),
    "crates/iroha_core/src/zk.rs": (
        "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE",
        "semantic ABI-7 compact tokens are disabled for production",
        "Err(KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE.to_owned())",
        "returns the production-unavailable diagnostic until ABI-7 compact proofs",
        "compose the private-hop verifier slice in-circuit",
        "pub fn verify_kagemusha_recursive_compact_payment_token(",
        "false",
        "preverify_kagemusha_recursive_compact_payment_token_with_record",
    ),
    "crates/connect_norito_bridge/src/lib.rs": (
        "CONNECT_NORITO_BRIDGE_ABI_VERSION: u32 = 7;",
        "KagemushaRecursiveCompactUnavailable",
        "preverify_kagemusha_recursive_compact_payment_token",
        "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE",
        "*out_valid = 0",
        "connect_norito_kagemusha_recursive_spend_redeem",
    ),
    "crates/iroha_js_host/src/lib.rs": (
        "connect_norito_bridge_abi_version() -> u32",
        "preverify_kagemusha_recursive_compact_payment_token",
        "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE",
        "return Ok(false);",
        "kagemusha_recursive_spend_redeem_instruction_from_request",
    ),
    "python/iroha_python/iroha_python_rs/src/lib.rs": (
        "kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes_py",
        "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE",
        "return Ok(false);",
        "kagemusha_recursive_spend_redeem_py",
    ),
}

SDK_SELECTOR_REQUIREMENTS = {
    "javascript/iroha_js/src/crypto.js": (
        "void recursiveCompactAvailable;",
        "if (recursiveSpendAvailable)",
        "return KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1;",
        "return KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1;",
    ),
    "javascript/iroha_js/dist/crypto.js": (
        "void recursiveCompactAvailable;",
        "if (recursiveSpendAvailable)",
        "return KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1;",
        "return KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1;",
    ),
    "python/iroha_python/src/iroha_python/kagemusha.py": (
        "_ = recursive_compact_available",
        "if recursive_spend_available:",
        "return KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1",
        "return KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1",
    ),
    "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift": (
        "_ = recursiveCompactAvailable",
        "return recursiveSpendAvailable ? .recursiveSpendV1 : .checkedPrefoldV1",
    ),
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt": (
        '@Suppress("UNUSED_PARAMETER")',
        "recursiveCompactAvailable: Boolean",
        "if (recursiveSpendAvailable)",
        "Mode.RECURSIVE_SPEND_V1",
        "Mode.CHECKED_PREFOLD_V1",
    ),
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java": (
        "compact mode is not a production default yet",
        "return recursiveSpendAvailable ? Mode.RECURSIVE_SPEND_V1 : Mode.CHECKED_PREFOLD_V1;",
    ),
    "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs": (
        "_ = recursiveCompactAvailable;",
        "return recursiveSpendAvailable",
        "KagemushaOfflineSpendMode.RecursiveSpendV1",
        "KagemushaOfflineSpendMode.CheckedPrefoldV1",
    ),
}

WORKFLOW_PATH = ".github/workflows/pr_kagemusha_payload_bench.yml"
WORKFLOW_REQUIREMENTS = (
    '"ci/check_kagemusha_production_readiness.sh"',
    "ci/check_kagemusha_production_readiness.sh --negative-control-doc-route",
    "ci/check_kagemusha_production_readiness.sh --negative-control-abi6-manifest",
    "ci/check_kagemusha_production_readiness.sh --negative-control-sdk-default",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-open",
    "ci/check_kagemusha_production_readiness.sh --negative-control-workflow",
    "ci/check_kagemusha_production_readiness.sh",
)


def read_text(relative: str) -> str:
    if relative in text_overrides:
        return text_overrides[relative]
    return (root / relative).read_text(encoding="utf-8")


def override_text(relative: str, old: str, new: str) -> None:
    text = read_text(relative)
    if old not in text:
        raise SystemExit(f"negative control setup failed: `{old}` not found in {relative}")
    text_overrides[relative] = text.replace(old, new, 1)


def require_contains(relative: str, snippets: tuple[str, ...], errors: list[str]) -> None:
    text = read_text(relative)
    for snippet in snippets:
        if snippet not in text:
            errors.append(f"{relative}: missing `{snippet}`")


def require_manifest(errors: list[str]) -> None:
    manifest = json.loads(read_text("fixtures/kagemusha_recursive_spend_abi6/manifest.json"))
    if manifest.get("schema") != "iroha.kagemusha.recursive_spend.abi6.fixture_manifest.v1":
        errors.append("ABI-6 fixture manifest schema mismatch")
    if manifest.get("bridge_abi_version") != 6:
        errors.append("ABI-6 fixture manifest must advertise bridge ABI 6")
    if manifest.get("operation_count") != len(ABI6_SYMBOLS):
        errors.append("ABI-6 fixture manifest operation_count must remain 9")
    operation_symbols = tuple(item.get("symbol") for item in manifest.get("operations", []))
    if operation_symbols != ABI6_SYMBOLS:
        errors.append("ABI-6 fixture manifest operation symbols drifted")
    limits = manifest.get("limits", {})
    expected_limits = {
        "compact_token_max_hops": 64,
        "reserved_lineage_witnessless_max_hops": 64,
        "previous_proof_open_envelopes_required_count": 1,
        "native_archive_max_bytes": 64 * 1024 * 1024,
    }
    for key, expected in expected_limits.items():
        if limits.get(key) != expected:
            errors.append(f"ABI-6 fixture manifest limit {key} must be {expected}")
    modes = manifest.get("modes", {})
    if modes.get("preferred_when_recursive_available") != "recursive_spend_v1":
        errors.append("ABI-6 fixture manifest must prefer recursive_spend_v1")
    if modes.get("fallback_when_recursive_unavailable") != "checked_prefold_v1":
        errors.append("ABI-6 fixture manifest must fall back to checked_prefold_v1")


def check_readiness() -> list[str]:
    errors: list[str] = []
    for relative, snippets in TEXT_REQUIREMENTS.items():
        require_contains(relative, snippets, errors)
    for relative, snippets in SDK_SELECTOR_REQUIREMENTS.items():
        require_contains(relative, snippets, errors)
    require_contains(WORKFLOW_PATH, WORKFLOW_REQUIREMENTS, errors)
    require_manifest(errors)
    return errors


def run_negative_control(label: str, mutator) -> None:
    text_overrides.clear()
    mutator()
    errors = check_readiness()
    if errors:
        print(f"negative control rejected Kagemusha production-readiness drift: {label}")
        return
    raise SystemExit(
        f"negative control failed: Kagemusha production-readiness drift was not detected for {label}"
    )


if mode == "--negative-control-doc-route":
    run_negative_control(
        "production route docs",
        lambda: override_text("roadmap.md", "Reserved-lineage recursive spend path", "semantic aggregation compact path"),
    )
    raise SystemExit(0)

if mode == "--negative-control-abi6-manifest":
    def mutate_manifest() -> None:
        manifest = json.loads(read_text("fixtures/kagemusha_recursive_spend_abi6/manifest.json"))
        manifest["operation_count"] = 8
        text_overrides["fixtures/kagemusha_recursive_spend_abi6/manifest.json"] = json.dumps(
            manifest, indent=2, sort_keys=True
        )

    run_negative_control("ABI-6 manifest operation count", mutate_manifest)
    raise SystemExit(0)

if mode == "--negative-control-sdk-default":
    run_negative_control(
        "SDK default selector",
        lambda: override_text(
            "crates/iroha_data_model/src/offline/mod.rs",
            "if recursive_spend_available",
            "if _recursive_compact_available",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-open":
    run_negative_control(
        "ABI-7 compact fail-closed gate",
        lambda: override_text(
            "crates/iroha_core/src/zk.rs",
            "semantic ABI-7 compact tokens are disabled for production",
            "semantic ABI-7 compact tokens are enabled for production",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-workflow":
    run_negative_control(
        "workflow readiness guard",
        lambda: override_text(
            WORKFLOW_PATH,
            "ci/check_kagemusha_production_readiness.sh --negative-control-doc-route",
            "ci/disabled_kagemusha_production_readiness.sh --negative-control-doc-route",
        ),
    )
    raise SystemExit(0)

if mode:
    raise SystemExit(f"unknown mode: {mode}")

errors = check_readiness()
if errors:
    for error in errors:
        print(f"error: {error}", file=sys.stderr)
    raise SystemExit(1)

print("Kagemusha production readiness is routed through ABI-6 Reserved-lineage recursive spend; ABI-7 recursive compact remains fail-closed")
PY
