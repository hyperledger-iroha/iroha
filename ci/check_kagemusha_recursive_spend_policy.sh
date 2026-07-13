#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_POLICY_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
MODE="${1:-}"

if [[ -n "${MODE}" && "${MODE}" != "--self-test" ]]; then
  echo "usage: ci/check_kagemusha_recursive_spend_policy.sh [--self-test]" >&2
  exit 2
fi

bash "${ROOT_DIR}/ci/check_kagemusha_v3_release_contract.sh"
bash "${ROOT_DIR}/ci/check_kagemusha_recursive_spend_sdk_parity.sh"
bash "${ROOT_DIR}/ci/check_kagemusha_recursive_spend_payload_bench.sh"

python3 - "${ROOT_DIR}" "${MODE}" <<'PY'
from pathlib import Path
import json
import re
import sys

root = Path(sys.argv[1])
self_test = sys.argv[2] == "--self-test"

JAVA_SOURCE = "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java"
KOTLIN_SOURCE = "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt"
JVM_TRANSPORT_SOURCES = {
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaNfcProtocol.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaNearby.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaPeerTransport.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaQrStream.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaScaledAmount.java",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaNearby.kt",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaNfcProtocol.kt",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaPeerTransport.kt",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaQrStream.kt",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaScaledAmount.kt",
}
MODEL_SOURCE = "crates/iroha_data_model/src/offline/mod.rs"
READINESS_SOURCE = "crates/iroha_torii_shared/src/offline_api.rs"
SWIFT_SOURCE = "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2.swift"
SWIFT_TEST_SOURCE = "IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift"
PYTHON_SOURCE = "python/iroha_torii_client/client.py"
PYTHON_TEST_SOURCE = "python/iroha_torii_client/tests/test_client.py"
TORII_SOURCE = "crates/iroha_torii/src/lib.rs"
OPENAPI_SOURCE = "crates/iroha_torii/src/openapi.rs"
RUST_CLIENT_SOURCE = "crates/iroha/src/client.rs"
CORE_TRANSITION_SOURCE = "crates/iroha_core/src/zk/kagemusha_v2.rs"
MODEL_TEST_SOURCE = "crates/iroha_data_model/tests/kagemusha_value_contract.rs"
RECURSION_DOC_SOURCE = "docs/source/offline_kagemusha_recursion_adapter.md"
SWIFT_SDK_DOC_SOURCE = "docs/source/sdk/swift/index.md"
JS_SDK_README_SOURCE = "javascript/iroha_js/README.md"

MANIFEST_FIELDS = {
    "schema",
    "version",
    "bridge_abi_version",
    "proof_backend",
    "transcript_profile",
    "generation",
    "source_commit",
    "chain_id",
    "asset",
    "asset_scale",
    "activation_height",
    "withdrawal_height",
    "max_proof_bytes",
    "profiles",
    "topup_finality_roster_artifact",
    "benchmark_evidence_sha256",
    "cryptographic_review_sha256",
    "release_attestation_sha256",
}

CAPABILITY_FIELDS = {
    "bridge_abi_version",
    "artifact_manifest_schema",
    "proof_backend",
    "transcript_profile",
    "proof_envelope_version",
    "state_boundary_version",
    "step_eq_circuit_id",
    "step_ep_circuit_id",
    "max_proof_bytes",
    "proof_backend_available",
    "missing_gates",
}

SWIFT_CAPABILITY_FIELDS = {
    "bridgeABIVersion",
    "artifactManifestSchema",
    "proofBackend",
    "transcriptProfile",
    "proofEnvelopeVersion",
    "stateBoundaryVersion",
    "stepEqCircuitID",
    "stepEpCircuitID",
    "maxProofBytes",
    "proofBackendAvailable",
    "missingGates",
}

READINESS_FIELDS = {
    "required_bridge_abi_version",
    "max_hops",
    "asset_definition_id",
    "asset_scale",
    "evaluated_block_height",
    "evaluated_block_hash",
    "active_transfer_verifier",
    "active_topup_shield_verifier",
    "active_unshield_verifier",
    "active_recursive_step_eq_verifier",
    "active_recursive_step_ep_verifier",
    "proof_backend_available",
    "recursive_lineage_supported",
    "ready",
    "blockers",
}

EXPECTED_ROUTES = {
    "READINESS": "/v1/offline/readiness",
    "TOP_UP": "/v1/offline/top-up",
    "REDEEM": "/v1/offline/redeem",
    "OPERATION": "/v1/offline/operations/{operation_id}",
}

def rust_struct_fields(source: str, name: str) -> set[str]:
    match = re.search(rf"pub struct {re.escape(name)} \{{(?P<body>[\s\S]*?)\n(?:    )?\}}", source)
    if match is None:
        return set()
    return set(re.findall(r"\bpub\s+([a-z][a-z0-9_]*)\s*:", match.group("body")))

def swift_struct_fields(source: str, name: str) -> set[str]:
    match = re.search(rf"public struct {re.escape(name)}[^{{]*\{{(?P<body>[\s\S]*?)\n\}}", source)
    if match is None:
        return set()
    return set(re.findall(r"\bpublic let ([A-Za-z][A-Za-z0-9]*)\s*:", match.group("body")))

def check(model_override: str | None = None) -> list[str]:
    errors: list[str] = []
    model = model_override if model_override is not None else (root / MODEL_SOURCE).read_text(encoding="utf-8")

    java_dir = root / Path(JAVA_SOURCE).parent
    kotlin_dir = root / Path(KOTLIN_SOURCE).parent
    actual_sources = {
        path.relative_to(root).as_posix()
        for directory in (java_dir, kotlin_dir)
        for path in directory.iterdir()
        if path.is_file() and path.name.startswith("Kagemusha")
    }
    prover_sources = {JAVA_SOURCE, KOTLIN_SOURCE}
    expected_sources = prover_sources | JVM_TRANSPORT_SOURCES
    if actual_sources != expected_sources:
        errors.append(
            "JVM Kagemusha source inventory mismatch: "
            f"missing={sorted(expected_sources - actual_sources)}, "
            f"extra={sorted(actual_sources - expected_sources)}"
        )

    for source in prover_sources:
        text = (root / source).read_text(encoding="utf-8")
        for literal in (
            "REQUIRED_NATIVE_BRIDGE_ABI_VERSION",
            "19",
            "kagemusha.offline.recursive_spend.artifact_manifest.v3",
            "decodeRecipientPaymentRequest",
            "KagemushaRecipientPaymentRequestV2",
            "decodePeerPayment",
            "KagemushaRecursiveSpendPeerPaymentV2",
            "decodeReceiverAcknowledgement",
            "KagemushaReceiverAcknowledgementV2",
            "decodeNoteMembershipWitness",
            "KagemushaNoteMembershipWitnessV2",
            "decodeSplitResult",
            "KagemushaRecursiveSpendSplitResultV2",
            "decodeVerifyResult",
            "KagemushaRecursiveSpendVerifyResultV2",
            "decodeRedeemBuildResult",
            "KagemushaRecursiveSpendRedeemBuildResultV2",
            "newToriiClient",
            "getReadiness",
            "submitTopUp",
            "submitRedeem",
            "getOperation",
            "/v1/offline/readiness",
            "/v1/offline/top-up",
            "/v1/offline/redeem",
            "/v1/offline/operations",
            "application/x-norito",
            "Idempotency-Key",
        ):
            if literal not in text:
                errors.append(f"{source}: missing contract literal {literal!r}")

    for struct_name, expected in (
        ("KagemushaRecursiveSpendArtifactManifestV3", MANIFEST_FIELDS),
        ("KagemushaRecursiveSpendNativeCapabilitiesV1", CAPABILITY_FIELDS),
    ):
        actual = rust_struct_fields(model, struct_name)
        if actual != expected:
            errors.append(
                f"{struct_name} field inventory mismatch: "
                f"missing={sorted(expected - actual)}, extra={sorted(actual - expected)}"
            )

    swift = (root / SWIFT_SOURCE).read_text(encoding="utf-8")
    actual_swift = swift_struct_fields(swift, "KagemushaRecursiveSpendNativeCapabilities")
    if actual_swift != SWIFT_CAPABILITY_FIELDS:
        errors.append(
            "Swift native capability field inventory mismatch: "
            f"missing={sorted(SWIFT_CAPABILITY_FIELDS - actual_swift)}, "
            f"extra={sorted(actual_swift - SWIFT_CAPABILITY_FIELDS)}"
        )

    branch_depth_bound = re.search(
        r"KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2:\s*u8\s*=\s*(\d+)\s*;",
        model,
    )
    if branch_depth_bound is None or int(branch_depth_bound.group(1)) != 64:
        errors.append("data-model branch-path capacity must remain exactly 64")
    peer_hop_bound = re.search(
        r"KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2:\s*u32\s*=\s*(\d+)\s*;",
        model,
    )
    if peer_hop_bound is None or int(peer_hop_bound.group(1)) != 8:
        errors.append("data-model peer-hop ceiling must remain exactly 8")
    input_bound = re.search(
        r"KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2:\s*usize\s*=\s*(\d+)\s*;",
        model,
    )
    if input_bound is None or int(input_bound.group(1)) != 2:
        errors.append("data-model transition input ceiling must remain exactly 2")
    if model.count("KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2") < 5:
        errors.append("data-model peer-hop validations must use the exact 8-hop constant")
    core_transition = (root / CORE_TRANSITION_SOURCE).read_text(encoding="utf-8")
    if re.search(
        r"const\s+PEER_HOP_SELECTOR_COUNT:\s*usize\s*=\s*"
        r"KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2\s+as\s+usize\s*\+\s*1\s*;",
        core_transition,
    ) is None:
        errors.append("Eq transition circuit must constrain the exact 8-peer-hop domain")
    model_tests = (root / MODEL_TEST_SOURCE).read_text(encoding="utf-8")
    if (
        "fn peer_hop_limit_is_eight_and_independent_of_branch_depth()"
        not in model_tests
    ):
        errors.append("data-model peer-hop boundary regression is missing")
    if re.search(r"maximumInputsPerTransition\s*=\s*2\b", swift) is None:
        errors.append("Swift transition input ceiling must be exactly 2")
    if re.search(r"maximumPeerHops:\s*UInt32\s*=\s*8\b", swift) is None:
        errors.append("Swift peer-hop bound must be exactly 8")
    for binding in (
        "peerHopCount <= KagemushaRecursiveSpend.maximumPeerHops",
        "$0.peerHopCount < KagemushaRecursiveSpend.maximumPeerHops",
        "parentPeerHopCount <= KagemushaRecursiveSpend.maximumPeerHops",
        "maximumHops == KagemushaRecursiveSpend.maximumPeerHops",
    ):
        if binding not in swift:
            errors.append(f"Swift peer-hop validation drifted: missing {binding!r}")
    python_client = (root / PYTHON_SOURCE).read_text(encoding="utf-8")
    if re.search(r"^_KAGEMUSHA_MAX_HOPS\s*=\s*8\s*$", python_client, re.MULTILINE) is None:
        errors.append("Python peer-hop bound must be exactly 8")
    python_tests = (root / PYTHON_TEST_SOURCE).read_text(encoding="utf-8")
    if '"max_hops": 8' not in python_tests:
        errors.append("Python readiness fixtures must advertise exactly 8 peer hops")
    torii = (root / TORII_SOURCE).read_text(encoding="utf-8")
    readiness_constructor = re.search(
        r"let payload = iroha_torii_shared::offline_api::OfflineReadiness \{(?P<body>[\s\S]*?)\n    \};",
        torii,
    )
    if readiness_constructor is None or (
        "iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2"
        not in readiness_constructor.group("body")
    ):
        errors.append("Torii readiness must advertise the exact 8-peer-hop ceiling")
    if torii.count(
        "iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2"
    ) < 2:
        errors.append("Torii readiness fixtures must use the exact 8-peer-hop ceiling")
    rust_client = (root / RUST_CLIENT_SOURCE).read_text(encoding="utf-8")
    if rust_client.count(
        "iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2"
    ) < 2:
        errors.append("Rust client readiness validation must use the exact 8-peer-hop ceiling")

    readiness = (root / READINESS_SOURCE).read_text(encoding="utf-8")
    actual_readiness = rust_struct_fields(readiness, "OfflineReadiness")
    if actual_readiness != READINESS_FIELDS:
        errors.append(
            "Torii readiness field inventory mismatch: "
            f"missing={sorted(READINESS_FIELDS - actual_readiness)}, "
            f"extra={sorted(actual_readiness - READINESS_FIELDS)}"
        )

    catalog = (root / "crates/iroha_torii_shared/src/route_catalog.rs").read_text(encoding="utf-8")
    match = re.search(r"pub mod offline \{(?P<body>[\s\S]*?)\n\}", catalog)
    if match is None:
        errors.append("Torii route catalog has no offline module")
    else:
        body = match.group("body")
        actual_routes = dict(re.findall(r'pub const ([A-Z_]+)_PATH: &str = "([^"]+)";', body))
        if actual_routes != EXPECTED_ROUTES:
            errors.append(f"Torii route inventory mismatch: expected={EXPECTED_ROUTES}, actual={actual_routes}")
        expected_descriptor = "pub const ROUTES: &[RouteDescriptor] = &[READINESS, TOP_UP, REDEEM, OPERATION];"
        if expected_descriptor not in body:
            errors.append("Torii descriptor inventory mismatch")

    openapi = json.loads((root / "docs/portal/static/openapi/torii.json").read_text(encoding="utf-8"))
    actual_openapi = {path for path in openapi.get("paths", {}) if path.startswith("/v1/offline/")}
    expected_openapi = set(EXPECTED_ROUTES.values())
    if actual_openapi != expected_openapi:
        errors.append(
            "OpenAPI route inventory mismatch: "
            f"missing={sorted(expected_openapi - actual_openapi)}, "
            f"extra={sorted(actual_openapi - expected_openapi)}"
        )
    max_hops_schema = (
        openapi.get("components", {})
        .get("schemas", {})
        .get("OfflineReadiness", {})
        .get("properties", {})
        .get("max_hops", {})
    )
    if max_hops_schema.get("minimum") != 8 or max_hops_schema.get("maximum") != 8:
        errors.append("OpenAPI readiness max_hops must be the exact first-release value 8")
    schemas = openapi.get("components", {}).get("schemas", {})
    recursive_bounds = {
        ("OfflinePeerSplitTransition", "parent_max_proof_step_count"): (1, 127),
        ("OfflinePeerSplitTransition", "parent_max_peer_hop_count"): (0, 7),
        ("OfflineRedemptionChangeTransition", "parent_proof_step_count"): (1, 127),
        ("OfflineRedemptionChangeTransition", "parent_peer_hop_count"): (0, 8),
        ("OfflineSpendStatement", "proof_step_count"): (1, 128),
        ("OfflineSpendStatement", "peer_hop_count"): (0, 8),
        ("OfflineRedemptionIntent", "parent_proof_step_count"): (1, 128),
        ("OfflineRedemptionIntent", "parent_peer_hop_count"): (0, 8),
    }
    for (owner, field), expected in recursive_bounds.items():
        schema = schemas.get(owner, {}).get("properties", {}).get(field, {})
        actual = (schema.get("minimum"), schema.get("maximum"))
        if actual != expected:
            errors.append(
                f"OpenAPI {owner}.{field} bounds must be {expected}, got {actual}"
            )

    retired_js_offline_methods = (
        "getOfflineReadiness",
        "submitOfflineTopUp",
        "submitOfflineRedeem",
        "getOfflineOperationStatus",
    )
    current_docs = list((root / "docs").rglob("*.md")) + [root / JS_SDK_README_SOURCE]
    for path in current_docs:
        if "version-2025-q2" in path.parts:
            continue
        text = path.read_text(encoding="utf-8")
        stale = [method for method in retired_js_offline_methods if method in text]
        if stale:
            relative = path.relative_to(root).as_posix()
            errors.append(f"{relative}: retired JavaScript Kagemusha methods remain: {stale}")
    swift_sdk_doc = (root / SWIFT_SDK_DOC_SOURCE).read_text(encoding="utf-8")
    for method in (
        "getKagemushaReadiness",
        "submitKagemushaTopUp",
        "submitKagemushaRedeem",
        "getKagemushaOperationStatus",
    ):
        if method not in swift_sdk_doc:
            errors.append(f"Swift SDK guide is missing current method {method}")

    for literal in (
        "KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3: u32 = 19",
        "kagemusha.offline.recursive_spend.artifact_manifest.v3",
    ):
        if literal not in model:
            errors.append(f"data model is missing {literal!r}")
    return errors

baseline_errors = check()
if baseline_errors:
    print("Kagemusha first-release policy failed:", file=sys.stderr)
    for error in baseline_errors:
        print(f" - {error}", file=sys.stderr)
    raise SystemExit(1)

if self_test:
    model = (root / MODEL_SOURCE).read_text(encoding="utf-8")
    marker = "        pub schema: String,"
    if model.count(marker) != 1:
        raise SystemExit("policy self-test could not identify the manifest field insertion point")
    mutated = model.replace(marker, marker + "\n        pub unexpected_field: String,", 1)
    mutation_errors = check(mutated)
    if not any("field inventory mismatch" in error and "unexpected_field" in error for error in mutation_errors):
        raise SystemExit("policy self-test failed to reject an unknown manifest field")
    print("Kagemusha first-release policy self-test rejected an unknown artifact field")
else:
    print("Kagemusha first-release policy passed: exact sources, fields, routes, and artifact contract")
PY
