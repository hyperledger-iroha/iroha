#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_POLICY_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
MODE="${1:-}"

if [[ -n "${MODE}" && "${MODE}" != "--self-test" ]]; then
  echo "usage: ci/check_kagemusha_recursive_spend_policy.sh [--self-test]" >&2
  exit 2
fi

if [[ "${MODE}" == "--self-test" ]]; then
  bash "${ROOT_DIR}/ci/check_kagemusha_v3_release_contract.sh" --self-test
  bash "${ROOT_DIR}/ci/check_kagemusha_recursive_spend_sdk_parity.sh" --self-test
  bash "${ROOT_DIR}/ci/check_kagemusha_recursive_spend_payload_bench.sh" --self-test
else
  bash "${ROOT_DIR}/ci/check_kagemusha_v3_release_contract.sh"
  bash "${ROOT_DIR}/ci/check_kagemusha_recursive_spend_sdk_parity.sh"
  bash "${ROOT_DIR}/ci/check_kagemusha_recursive_spend_payload_bench.sh"
fi

python3 - "${ROOT_DIR}" "${MODE}" <<'PY'
from __future__ import annotations

from pathlib import Path
import json
import re
import sys

root = Path(sys.argv[1])
self_test = sys.argv[2] == "--self-test"

JAVA_SOURCE = "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java"
KOTLIN_SOURCE = "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt"
JVM_TRANSPORT_SOURCES = {
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaDevicePublicKeyV2.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaDeviceSignatureV2.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaNfcProtocol.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaNearby.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaP256Codec.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaPeerTransport.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaQrStream.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaScaledAmount.java",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaNearby.kt",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaDeviceAuthority.kt",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaNfcProtocol.kt",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaPeerTransport.kt",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaQrStream.kt",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaScaledAmount.kt",
}
MODEL_SOURCE = "crates/iroha_data_model/src/offline/mod.rs"
READINESS_SOURCE = "crates/iroha_torii_shared/src/offline_api.rs"
ROUTE_CATALOG_SOURCE = "crates/iroha_torii_shared/src/route_catalog.rs"
SWIFT_SOURCE = "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2.swift"
SWIFT_TEST_SOURCE = "IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift"
PYTHON_SOURCE = "python/iroha_torii_client/client.py"
PYTHON_TEST_SOURCE = "python/iroha_torii_client/tests/test_client.py"
TORII_SOURCE = "crates/iroha_torii/src/lib.rs"
TORII_SMOKE_SOURCE = "crates/iroha_torii/tests/offline_redeem_contract.rs"
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
    "source_tree_sha256",
    "source_repo_dirty",
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

TOP_UP_REQUEST_FIELDS = {
    "asset",
    "amount",
    "current_note",
    "shield_evidence",
    "artifact_binding",
    "operation_id",
    "authorization",
}

REDEEM_REQUEST_FIELDS = {
    "bundle",
    "recipient",
    "amount",
    "redeem_proof",
    "redemption",
    "offline_change",
    "block_height",
    "operation_id",
    "authorization",
}

AUXILIARY_STRUCT_FIELDS = {
    "OfflineReadinessBlocker": {"code", "message"},
    "OfflineVerifierId": {"backend", "name"},
    "OfflineOperationReference": {
        "operation_id",
        "kind",
        "state",
        "transaction_hash",
        "status_uri",
        "submitted_at_ms",
    },
    "OfflineTopUpResult": {
        "transaction_hash",
        "finalized_block_height",
        "server_time_ms",
        "anchor",
        "finality_proof",
    },
    "OfflineRedeemResult": {
        "transaction_hash",
        "finalized_block_height",
        "server_time_ms",
    },
}

OPERATION_STATUS_VARIANT_FIELDS = {
    "Pending": {"operation_id", "kind", "transaction_hash", "submitted_at_ms"},
    "Applied": {"operation_id", "result"},
    "Rejected": {"operation_id", "kind", "transaction_hash", "error"},
}

RETIRED_MODEL_FIELDS = {
    "topup_request_norito_base64",
    "redeem_request_norito_base64",
    "request_norito_base64",
    "transaction_norito_base64",
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

def rust_enum_variant_fields(source: str, enum_name: str, variant: str) -> set[str]:
    enum_match = re.search(
        rf"pub enum {re.escape(enum_name)} \{{(?P<body>[\s\S]*?)^\}}",
        source,
        re.MULTILINE,
    )
    if enum_match is None:
        return set()
    variant_match = re.search(
        rf"^    {re.escape(variant)} \{{(?P<body>[\s\S]*?)^    \}},?$",
        enum_match.group("body"),
        re.MULTILINE,
    )
    if variant_match is None:
        return set()
    return set(re.findall(r"^        ([a-z][a-z0-9_]*)\s*:", variant_match.group("body"), re.MULTILINE))

def check(overrides: dict[str, str] | None = None) -> list[str]:
    errors: list[str] = []

    def read(source: str) -> str:
        if overrides is not None and source in overrides:
            return overrides[source]
        return (root / source).read_text(encoding="utf-8")

    model = read(MODEL_SOURCE)

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
        text = read(source)
        for literal in (
            "REQUIRED_NATIVE_BRIDGE_ABI_VERSION",
            "V4_REQUIRED_NATIVE_BRIDGE_ABI_VERSION",
            "20",
            "kagemusha.offline.recursive_spend.artifact_manifest.v4",
            "decodeRecipientPaymentRequest",
            "KagemushaRecipientPaymentRequestV2",
            "decodePeerPayment",
            "KagemushaRecursiveSpendPeerPaymentV2",
            "decodeReceiverAcknowledgement",
            "KagemushaReceiverAcknowledgementV2",
            "decodeNoteMembershipWitness",
            "KagemushaNoteMembershipWitnessV2",
            "decodeSplitResult",
            "KagemushaRecursiveSpendSplitResultV4",
            "decodeVerifyResult",
            "KagemushaRecursiveSpendVerifyResultV4",
            "decodeRedeemBuildResult",
            "KagemushaRecursiveSpendRedeemBuildResultV4",
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
        ("KagemushaRecursiveSpendTopUpRequestV2", TOP_UP_REQUEST_FIELDS),
        ("KagemushaRecursiveSpendRedeemRequestV2", REDEEM_REQUEST_FIELDS),
    ):
        actual = rust_struct_fields(model, struct_name)
        if actual != expected:
            errors.append(
                f"{struct_name} field inventory mismatch: "
                f"missing={sorted(expected - actual)}, extra={sorted(actual - expected)}"
            )

    for retired_field in sorted(RETIRED_MODEL_FIELDS):
        if re.search(rf"\bpub\s+{re.escape(retired_field)}\s*:", model):
            errors.append(f"data model retains retired transport field {retired_field!r}")

    swift = read(SWIFT_SOURCE)
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
    core_transition = read(CORE_TRANSITION_SOURCE)
    if re.search(
        r"const\s+PEER_HOP_SELECTOR_COUNT:\s*usize\s*=\s*"
        r"KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2\s+as\s+usize\s*\+\s*1\s*;",
        core_transition,
    ) is None:
        errors.append("Eq transition circuit must constrain the exact 8-peer-hop domain")
    model_tests = read(MODEL_TEST_SOURCE)
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
    python_client = read(PYTHON_SOURCE)
    if re.search(r"^_KAGEMUSHA_MAX_HOPS\s*=\s*8\s*$", python_client, re.MULTILINE) is None:
        errors.append("Python peer-hop bound must be exactly 8")
    python_tests = read(PYTHON_TEST_SOURCE)
    if '"max_hops": 8' not in python_tests:
        errors.append("Python readiness fixtures must advertise exactly 8 peer hops")
    torii = read(TORII_SOURCE)
    redeem_handler_start = torii.find("async fn handler_offline_redeem(")
    redeem_handler_end = torii.find(
        '\n#[cfg(feature = "app_api")]', redeem_handler_start + 1
    )
    if redeem_handler_start < 0 or redeem_handler_end < 0:
        errors.append("Torii direct typed offline redeem handler is missing")
    else:
        redeem_handler = torii[redeem_handler_start:redeem_handler_end]
        for marker in (
            "crate::utils::extractors::NoritoOnly(request)",
            "iroha_torii_shared::offline_api::OfflineRedeemRequest",
            "offline_commands::handle_redeem(app, &headers, request).await",
        ):
            if marker not in redeem_handler:
                errors.append(f"Torii direct typed offline redeem handler is missing {marker!r}")
        if re.search(r"base64|Json\s*\(", redeem_handler, re.IGNORECASE):
            errors.append("Torii direct typed offline redeem handler restored a wrapper transport")
    if re.search(
        r"&route_catalog::offline::REDEEM,[\s\S]{0,240}"
        r"catalog_post\(handler_offline_redeem\)[\s\S]{0,240}"
        r"DefaultBodyLimit::max\(offline_command_body_limit\)",
        torii,
    ) is None:
        errors.append("Torii redeem route must bind the typed handler and bounded body directly")
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
    rust_client = read(RUST_CLIENT_SOURCE)
    if rust_client.count(
        "iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2"
    ) < 2:
        errors.append("Rust client readiness validation must use the exact 8-peer-hop ceiling")

    readiness = read(READINESS_SOURCE)
    actual_readiness = rust_struct_fields(readiness, "OfflineReadiness")
    if actual_readiness != READINESS_FIELDS:
        errors.append(
            "Torii readiness field inventory mismatch: "
            f"missing={sorted(READINESS_FIELDS - actual_readiness)}, "
            f"extra={sorted(actual_readiness - READINESS_FIELDS)}"
        )

    expected_redeem_alias = (
        "KagemushaRecursiveSpendRedeemRequestV2 as OfflineRedeemRequest"
    )
    if readiness.count(expected_redeem_alias) != 1:
        errors.append("Torii offline API must expose exactly the direct V2 redeem request alias")
    if re.search(r"\bas\s+OfflineRedeemRequestV[0-9]+\b", readiness):
        errors.append("Torii offline API retains a version-suffixed redeem request alias")

    for struct_name, expected in AUXILIARY_STRUCT_FIELDS.items():
        actual = rust_struct_fields(readiness, struct_name)
        if actual != expected:
            errors.append(
                f"{struct_name} auxiliary field inventory mismatch: "
                f"missing={sorted(expected - actual)}, extra={sorted(actual - expected)}"
            )
    for variant, expected in OPERATION_STATUS_VARIANT_FIELDS.items():
        actual = rust_enum_variant_fields(readiness, "OfflineOperationStatus", variant)
        if actual != expected:
            errors.append(
                f"OfflineOperationStatus::{variant} archive field shape mismatch: "
                f"missing={sorted(expected - actual)}, extra={sorted(actual - expected)}"
            )

    smoke = read(TORII_SMOKE_SOURCE)
    for marker in (
        "fn production_source(source: &str) -> &str",
        "fn typed_offline_redeem_route_accepts_only_the_direct_v2_request()",
        "fn offline_operation_polling_preserves_redeem_identity_and_finality_integrity()",
    ):
        if marker not in smoke:
            errors.append(f"Torii offline redeem smoke contract is missing {marker!r}")

    catalog = read(ROUTE_CATALOG_SOURCE)
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

    openapi = json.loads(read("docs/portal/static/openapi/torii.json"))
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
    swift_sdk_doc = read(SWIFT_SDK_DOC_SOURCE)
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

POLICY_SELF_TEST_CASES = {
    "torii_redeem_surface_mutation",
    "torii_route_inventory_mutation",
    "model_exact_field_inventory_mutation",
    "archive_field_shape_mutation",
    "retired_model_fields_mutation",
    "retired_redeem_aliases_mutation",
    "auxiliary_field_inventory_mutation",
    "openapi_contract_mutation",
    "smoke_contract_mutation",
}

def mutate_once(source: str, before: str, after: str, self_test_name: str) -> str:
    if source.count(before) != 1:
        raise SystemExit(
            f"policy self-test {self_test_name} expected one mutation target, "
            f"found {source.count(before)}"
        )
    return source.replace(before, after, 1)

def require_mutation_rejected(
    self_test_name: str,
    overrides: dict[str, str],
    expected_fragments: tuple[str, ...],
) -> None:
    errors = check(overrides)
    if not errors:
        raise SystemExit(f"policy self-test {self_test_name} was not rejected")
    if not any(all(fragment in error for fragment in expected_fragments) for error in errors):
        rendered = "\n - ".join(errors)
        raise SystemExit(
            f"policy self-test {self_test_name} was rejected for the wrong reason; "
            f"expected {expected_fragments!r}, got:\n - {rendered}"
        )
    print(f"Kagemusha policy self-test rejected {self_test_name}")

def run_policy_self_test(self_test_name: str) -> None:
    if self_test_name == "torii_redeem_surface_mutation":
        source = (root / TORII_SOURCE).read_text(encoding="utf-8")
        mutated = mutate_once(
            source,
            "offline_commands::handle_redeem(app, &headers, request).await",
            "offline_commands::handle_top_up(app, &headers, request).await",
            self_test_name,
        )
        require_mutation_rejected(
            self_test_name,
            {TORII_SOURCE: mutated},
            ("Torii direct typed offline redeem handler", "handle_redeem"),
        )
        return

    if self_test_name == "torii_route_inventory_mutation":
        source = (root / ROUTE_CATALOG_SOURCE).read_text(encoding="utf-8")
        mutated = mutate_once(
            source,
            'pub const REDEEM_PATH: &str = "/v1/offline/redeem";',
            'pub const REDEEM_PATH: &str = "/v1/offline/redeem-v2";',
            self_test_name,
        )
        require_mutation_rejected(
            self_test_name,
            {ROUTE_CATALOG_SOURCE: mutated},
            ("Torii route inventory mismatch",),
        )
        return

    if self_test_name == "model_exact_field_inventory_mutation":
        source = (root / MODEL_SOURCE).read_text(encoding="utf-8")
        marker = "    pub struct KagemushaRecursiveSpendArtifactManifestV3 {"
        mutated = mutate_once(
            source,
            marker,
            marker + "\n        pub unexpected_field: String,",
            self_test_name,
        )
        require_mutation_rejected(
            self_test_name,
            {MODEL_SOURCE: mutated},
            ("field inventory mismatch", "unexpected_field"),
        )
        return

    if self_test_name == "archive_field_shape_mutation":
        source = (root / READINESS_SOURCE).read_text(encoding="utf-8")
        marker = "        submitted_at_ms: u64,\n    },\n    /// The transaction was applied"
        mutated = mutate_once(
            source,
            marker,
            "        submitted_at_ms: u64,\n        legacy_archive: Vec<u8>,\n    },\n    /// The transaction was applied",
            self_test_name,
        )
        require_mutation_rejected(
            self_test_name,
            {READINESS_SOURCE: mutated},
            ("OfflineOperationStatus::Pending archive field shape mismatch", "legacy_archive"),
        )
        return

    if self_test_name == "retired_model_fields_mutation":
        source = (root / MODEL_SOURCE).read_text(encoding="utf-8")
        marker = "    pub struct KagemushaRecursiveSpendRedeemRequestV2 {"
        mutated = mutate_once(
            source,
            marker,
            marker + "\n        pub redeem_request_norito_base64: String,",
            self_test_name,
        )
        require_mutation_rejected(
            self_test_name,
            {MODEL_SOURCE: mutated},
            ("retired transport field", "redeem_request_norito_base64"),
        )
        return

    if self_test_name == "retired_redeem_aliases_mutation":
        source = (root / READINESS_SOURCE).read_text(encoding="utf-8")
        mutated = mutate_once(
            source,
            "KagemushaRecursiveSpendRedeemRequestV2 as OfflineRedeemRequest",
            "KagemushaRecursiveSpendRedeemRequestV2 as OfflineRedeemRequestV2",
            self_test_name,
        )
        require_mutation_rejected(
            self_test_name,
            {READINESS_SOURCE: mutated},
            ("version-suffixed redeem request alias",),
        )
        return

    if self_test_name == "auxiliary_field_inventory_mutation":
        source = (root / READINESS_SOURCE).read_text(encoding="utf-8")
        marker = "pub struct OfflineRedeemResult {"
        mutated = mutate_once(
            source,
            marker,
            marker + "\n    pub unexpected_receipt: Vec<u8>,",
            self_test_name,
        )
        require_mutation_rejected(
            self_test_name,
            {READINESS_SOURCE: mutated},
            ("OfflineRedeemResult auxiliary field inventory mismatch", "unexpected_receipt"),
        )
        return

    if self_test_name == "openapi_contract_mutation":
        openapi_source = "docs/portal/static/openapi/torii.json"
        document = json.loads((root / openapi_source).read_text(encoding="utf-8"))
        if document.get("paths", {}).pop("/v1/offline/redeem", None) is None:
            raise SystemExit(
                f"policy self-test {self_test_name} could not remove the redeem path"
            )
        mutated = json.dumps(document, sort_keys=True)
        require_mutation_rejected(
            self_test_name,
            {openapi_source: mutated},
            ("OpenAPI route inventory mismatch", "/v1/offline/redeem"),
        )
        return

    if self_test_name == "smoke_contract_mutation":
        source = (root / TORII_SMOKE_SOURCE).read_text(encoding="utf-8")
        marker = "fn typed_offline_redeem_route_accepts_only_the_direct_v2_request()"
        mutated = mutate_once(
            source,
            marker,
            "fn typed_offline_redeem_route_accepts_legacy_wrappers()",
            self_test_name,
        )
        require_mutation_rejected(
            self_test_name,
            {TORII_SMOKE_SOURCE: mutated},
            ("Torii offline redeem smoke contract", marker),
        )
        return

    raise SystemExit(f"unknown policy self-test: {self_test_name}")

if self_test:
    for self_test_name in sorted(POLICY_SELF_TEST_CASES):
        run_policy_self_test(self_test_name)
    print(
        "Kagemusha first-release policy self-test passed: "
        f"{len(POLICY_SELF_TEST_CASES)} destructive contract mutations rejected"
    )
else:
    print("Kagemusha first-release policy passed: exact sources, fields, routes, and artifact contract")
PY
