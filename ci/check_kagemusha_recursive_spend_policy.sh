#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_POLICY_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
MODE="${1:-}"

if [[ $# -gt 1 || ( -n "${MODE}" && "${MODE}" != "--self-test" ) ]]; then
  echo "usage: ci/check_kagemusha_recursive_spend_policy.sh [--self-test]" >&2
  exit 2
fi

if [[ "${MODE}" == "--self-test" ]]; then
  bash "${ROOT_DIR}/ci/check_kagemusha_production_readiness.sh" candidate --self-test
  bash "${ROOT_DIR}/ci/check_kagemusha_recursive_spend_v4_sdk_contract.sh" --self-test
  bash "${ROOT_DIR}/ci/check_kagemusha_recursive_spend_payload_bench.sh" --self-test
else
  bash "${ROOT_DIR}/ci/check_kagemusha_production_readiness.sh" candidate
  bash "${ROOT_DIR}/ci/check_kagemusha_recursive_spend_v4_sdk_contract.sh"
  bash "${ROOT_DIR}/ci/check_kagemusha_recursive_spend_payload_bench.sh"
fi

python3 - "${ROOT_DIR}" "${MODE}" <<'PY'
from __future__ import annotations

import json
from pathlib import Path
import re
import sys

root = Path(sys.argv[1])
self_test = sys.argv[2] == "--self-test"

MODEL_SOURCE = "crates/iroha_data_model/src/offline/mod.rs"
READINESS_SOURCE = "crates/iroha_torii_shared/src/offline_api.rs"
ROUTE_CATALOG_SOURCE = "crates/iroha_torii_shared/src/route_catalog.rs"
TORII_SOURCE = "crates/iroha_torii/src/lib.rs"
TORII_COMMAND_SOURCE = "crates/iroha_torii/src/offline_commands.rs"
TORII_SMOKE_SOURCE = "crates/iroha_torii/tests/offline_redeem_contract.rs"
OPENAPI_SOURCE = "docs/portal/static/openapi/torii.json"
CORE_SOURCE = "crates/iroha_core/src/smartcontracts/isi/offline.rs"
CONTRACT_DOC_SOURCE = "docs/source/offline_kagemusha_v2_contract.md"
READINESS_DOC_SOURCE = "docs/source/offline_kagemusha.md"
SWIFT_SOURCE = "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2.swift"
SWIFT_TEST_SOURCE = "IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift"

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
    "reviewed_source_closure",
    "reviewed_source_closure_descriptor_sha256",
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
    "step_eq_circuit_id",
    "step_ep_circuit_id",
    "artifact_roles",
    "max_proof_bytes",
    "proof_backend_available",
    "missing_gates",
}

TOP_UP_REQUEST_FIELDS = {
    "version",
    "asset",
    "amount",
    "current_note",
    "shield_evidence",
    "artifact_binding",
    "operation_id",
    "authorization",
}

REDEEM_REQUEST_FIELDS = {
    "version",
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
    "artifact_set",
    "proof_backend_available",
    "recursive_lineage_supported",
    "ready",
    "blockers",
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

RETIRED_LIFECYCLE_TYPES = {
    "KagemushaRecursiveSpendArtifactManifestV3",
    "KagemushaRecursiveSpendNativeCapabilitiesV1",
    "KagemushaRecursiveSpendTopUpRequestV2",
    "KagemushaRecursiveSpendTopUpRequestV3",
    "KagemushaRecursiveSpendRedeemRequestV2",
    "KagemushaRecursiveSpendRedeemRequestV3",
}

EXPECTED_ROUTES = {
    "READINESS": "/v1/offline/readiness",
    "RECIPIENT_LINEAGE": "/v1/offline/receiver-lineage",
    "TOP_UP": "/v1/offline/top-up",
    "REDEEM": "/v1/offline/redeem",
    "OPERATION": "/v1/offline/operations/{operation_id}",
}


def rust_struct_fields(source: str, name: str) -> set[str]:
    match = re.search(
        rf"pub struct {re.escape(name)} \{{(?P<body>[\s\S]*?)\n(?:    )?\}}",
        source,
    )
    if match is None:
        return set()
    return set(re.findall(r"\bpub\s+([a-z][a-z0-9_]*)\s*:", match.group("body")))


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
    return set(
        re.findall(
            r"^        ([a-z][a-z0-9_]*)\s*:",
            variant_match.group("body"),
            re.MULTILINE,
        )
    )


def check(overrides: dict[str, str] | None = None) -> list[str]:
    errors: list[str] = []

    def read(source: str) -> str:
        if overrides is not None and source in overrides:
            return overrides[source]
        return (root / source).read_text(encoding="utf-8")

    model = read(MODEL_SOURCE)
    if "KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4: u32 = 21" not in model:
        errors.append("data model must pin the current native bridge ABI to 21")
    if re.search(
        r"KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE\s*:\s*bool\s*=\s*"
        r"cfg!\(feature\s*=\s*\"kagemusha-production-enabled\"\)\s*;",
        model,
    ) is None:
        errors.append("proof backend availability must remain compile-time promotion gated")
    for struct_name, expected in (
        ("KagemushaRecursiveSpendArtifactManifestV4", MANIFEST_FIELDS),
        ("KagemushaRecursiveSpendNativeCapabilitiesV4", CAPABILITY_FIELDS),
        ("KagemushaRecursiveSpendTopUpRequestV4", TOP_UP_REQUEST_FIELDS),
        ("KagemushaRecursiveSpendRedeemRequestV4", REDEEM_REQUEST_FIELDS),
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
    for retired_type in sorted(RETIRED_LIFECYCLE_TYPES):
        if re.search(
            rf"\b(?:pub\s+)?(?:struct|enum|type)\s+{re.escape(retired_type)}\b",
            model,
        ):
            errors.append(f"data model retains retired lifecycle type {retired_type}")

    for name, expected in (
        ("KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2", 64),
        ("KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2", 8),
        ("KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2", 2),
    ):
        match = re.search(rf"\b{name}\s*:\s*[^=]+\s*=\s*([0-9_]+)\s*;", model)
        if match is None or int(match.group(1).replace("_", "")) != expected:
            errors.append(f"data-model bound {name} must remain exactly {expected}")

    readiness = read(READINESS_SOURCE)
    if readiness.count(
        "KagemushaRecursiveSpendRedeemRequestV4 as OfflineRedeemRequest"
    ) != 1:
        errors.append("Torii offline API must expose exactly the direct V4 redeem alias")
    if readiness.count(
        "KagemushaRecursiveSpendTopUpRequestV4 as OfflineTopUpRequest"
    ) != 1:
        errors.append("Torii offline API must expose exactly the direct V4 top-up alias")
    if re.search(r"\bas\s+Offline(?:Redeem|TopUp)RequestV[0-9]+\b", readiness):
        errors.append("Torii offline API retains a version-suffixed lifecycle alias")

    actual_readiness = rust_struct_fields(readiness, "OfflineReadiness")
    if actual_readiness != READINESS_FIELDS:
        errors.append(
            "Torii readiness field inventory mismatch: "
            f"missing={sorted(READINESS_FIELDS - actual_readiness)}, "
            f"extra={sorted(actual_readiness - READINESS_FIELDS)}"
        )
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

    catalog = read(ROUTE_CATALOG_SOURCE)
    module_match = re.search(r"pub mod offline \{(?P<body>[\s\S]*?)\n\}", catalog)
    if module_match is None:
        errors.append("Torii route catalog has no offline module")
    else:
        body = module_match.group("body")
        actual_routes = dict(
            re.findall(r'pub const ([A-Z_]+)_PATH: &str = "([^"]+)";', body)
        )
        if actual_routes != EXPECTED_ROUTES:
            errors.append(
                f"Torii route inventory mismatch: expected={EXPECTED_ROUTES}, "
                f"actual={actual_routes}"
            )
        if re.search(
            r"pub const ROUTES: &\[RouteDescriptor\] =\s*&\["
            r"READINESS, RECIPIENT_LINEAGE, TOP_UP, REDEEM, OPERATION\];",
            body,
        ) is None:
            errors.append("Torii descriptor inventory must contain the exact five routes")

    torii = read(TORII_SOURCE)
    handler_start = torii.find("async fn handler_offline_redeem(")
    handler_end = torii.find('\n#[cfg(feature = "app_api")]', handler_start + 1)
    if handler_start < 0 or handler_end < 0:
        errors.append("Torii direct typed offline redeem handler is missing")
    else:
        handler = torii[handler_start:handler_end]
        for marker in (
            "crate::utils::extractors::NoritoOnly(request)",
            "iroha_torii_shared::offline_api::OfflineRedeemRequest",
            "offline_commands::handle_redeem(app, &headers, request).await",
        ):
            if marker not in handler:
                errors.append(
                    f"Torii direct typed offline redeem handler is missing {marker!r}"
                )
        if re.search(r"base64|Json\s*\(", handler, re.IGNORECASE):
            errors.append("Torii direct typed redeem handler restored a wrapper transport")
    if re.search(
        r"&route_catalog::offline::REDEEM,[\s\S]{0,180}"
        r"catalog_post\(handler_offline_redeem\)[\s\S]{0,180}"
        r"DefaultBodyLimit::max\(offline_redeem_body_limit_bytes\)",
        torii,
    ) is None:
        errors.append("Torii redeem route must bind its typed handler and body limit")
    if re.search(
        r"&route_catalog::offline::RECIPIENT_LINEAGE,[\s\S]{0,180}"
        r"catalog_post\(handler_offline_recipient_lineage\)",
        torii,
    ) is None:
        errors.append("Torii receiver-lineage route must bind its typed handler")

    smoke = read(TORII_SMOKE_SOURCE)
    for marker in (
        "fn production_source(source: &str) -> &str",
        "fn typed_offline_redeem_route_accepts_only_the_direct_v2_request()",
        "fn offline_operation_polling_preserves_redeem_identity_and_finality_integrity()",
    ):
        if marker not in smoke:
            errors.append(f"Torii offline redeem smoke contract is missing {marker!r}")

    commands = read(TORII_COMMAND_SOURCE)
    if "fn unavailable_abi21_release_fails_closed_with_stable_service_error()" not in commands:
        errors.append("Torii ABI-21 unavailable-release fail-closed regression is missing")

    core = read(CORE_SOURCE)
    for marker in (
        "fn kagemusha_v4_operation_marker_is_global_while_other_markers_are_authority_scoped()",
        "fn kagemusha_v4_cross_authority_operation_conflict_preserves_all_state()",
        "fn kagemusha_v4_unavailable_release_fails_closed_without_state_mutation()",
        "authorization_replay",
    ):
        if marker not in core:
            errors.append(f"core replay/rollback contract is missing {marker!r}")

    swift = read(SWIFT_SOURCE)
    if "public static let requiredNativeBridgeAbiVersion: UInt32 = 21" not in swift:
        errors.append("Swift Kagemusha contract must require native bridge ABI 21")
    swift_tests = read(SWIFT_TEST_SOURCE)
    for marker in (
        "getKagemushaReadiness",
        "submitKagemushaTopUp",
        "submitKagemushaRedeem",
        "getKagemushaOperationStatus",
    ):
        if marker not in swift_tests:
            errors.append(f"Swift Torii contract tests are missing {marker}")

    openapi = json.loads(read(OPENAPI_SOURCE))
    actual_openapi = {
        path for path in openapi.get("paths", {}) if path.startswith("/v1/offline/")
    }
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
        errors.append("OpenAPI readiness max_hops must remain exactly 8")

    contract_doc = read(CONTRACT_DOC_SOURCE)
    readiness_doc = read(READINESS_DOC_SOURCE)
    for source_name, source in (
        (CONTRACT_DOC_SOURCE, contract_doc),
        (READINESS_DOC_SOURCE, readiness_doc),
    ):
        for marker in (
            "global namespace across authorities",
            "remain authority-scoped",
            "/v1/offline/receiver-lineage",
        ):
            if marker not in source:
                errors.append(f"{source_name}: missing policy statement {marker!r}")
    for marker in (
        "kagemusha-production-enabled",
        "safety gate",
        "peer_transport_measurements_v1.json",
    ):
        if marker not in readiness_doc:
            errors.append(f"{READINESS_DOC_SOURCE}: missing release statement {marker!r}")

    return errors


baseline_errors = check()
if baseline_errors:
    print("Kagemusha ABI-21 policy failed:", file=sys.stderr)
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


def mutate_once(source: str, before: str, after: str, name: str) -> str:
    if source.count(before) != 1:
        raise SystemExit(
            f"policy self-test {name} expected one mutation target, "
            f"found {source.count(before)}"
        )
    return source.replace(before, after, 1)


def require_mutation_rejected(
    name: str,
    overrides: dict[str, str],
    expected_fragments: tuple[str, ...],
) -> None:
    errors = check(overrides)
    if not errors:
        raise SystemExit(f"policy self-test {name} was not rejected")
    if not any(all(fragment in error for fragment in expected_fragments) for error in errors):
        rendered = "\n - ".join(errors)
        raise SystemExit(
            f"policy self-test {name} was rejected for the wrong reason; "
            f"expected {expected_fragments!r}, got:\n - {rendered}"
        )
    print(f"Kagemusha policy self-test rejected {name}")


def run_policy_self_test(name: str) -> None:
    if name == "torii_redeem_surface_mutation":
        source = (root / TORII_SOURCE).read_text(encoding="utf-8")
        mutated = mutate_once(
            source,
            "offline_commands::handle_redeem(app, &headers, request).await",
            "offline_commands::handle_top_up(app, &headers, request).await",
            name,
        )
        require_mutation_rejected(
            name,
            {TORII_SOURCE: mutated},
            ("Torii direct typed offline redeem handler", "handle_redeem"),
        )
        return
    if name == "torii_route_inventory_mutation":
        source = (root / ROUTE_CATALOG_SOURCE).read_text(encoding="utf-8")
        mutated = mutate_once(
            source,
            'pub const REDEEM_PATH: &str = "/v1/offline/redeem";',
            'pub const REDEEM_PATH: &str = "/v1/offline/redeem-v4";',
            name,
        )
        require_mutation_rejected(
            name, {ROUTE_CATALOG_SOURCE: mutated}, ("Torii route inventory mismatch",)
        )
        return
    if name == "model_exact_field_inventory_mutation":
        source = (root / MODEL_SOURCE).read_text(encoding="utf-8")
        marker = "    pub struct KagemushaRecursiveSpendArtifactManifestV4 {"
        mutated = mutate_once(
            source, marker, marker + "\n        pub unexpected_field: String,", name
        )
        require_mutation_rejected(
            name,
            {MODEL_SOURCE: mutated},
            ("field inventory mismatch", "unexpected_field"),
        )
        return
    if name == "archive_field_shape_mutation":
        source = (root / READINESS_SOURCE).read_text(encoding="utf-8")
        marker = "        submitted_at_ms: u64,\n    },\n    /// The transaction was applied"
        mutated = mutate_once(
            source,
            marker,
            "        submitted_at_ms: u64,\n"
            "        legacy_archive: Vec<u8>,\n"
            "    },\n"
            "    /// The transaction was applied",
            name,
        )
        require_mutation_rejected(
            name,
            {READINESS_SOURCE: mutated},
            ("OfflineOperationStatus::Pending archive field shape mismatch", "legacy_archive"),
        )
        return
    if name == "retired_model_fields_mutation":
        source = (root / MODEL_SOURCE).read_text(encoding="utf-8")
        marker = "    pub struct KagemushaRecursiveSpendRedeemRequestV4 {"
        mutated = mutate_once(
            source,
            marker,
            marker + "\n        pub redeem_request_norito_base64: String,",
            name,
        )
        require_mutation_rejected(
            name,
            {MODEL_SOURCE: mutated},
            ("retired transport field", "redeem_request_norito_base64"),
        )
        return
    if name == "retired_redeem_aliases_mutation":
        source = (root / READINESS_SOURCE).read_text(encoding="utf-8")
        mutated = mutate_once(
            source,
            "KagemushaRecursiveSpendRedeemRequestV4 as OfflineRedeemRequest",
            "KagemushaRecursiveSpendRedeemRequestV4 as OfflineRedeemRequestV4",
            name,
        )
        require_mutation_rejected(
            name,
            {READINESS_SOURCE: mutated},
            ("version-suffixed lifecycle alias",),
        )
        return
    if name == "auxiliary_field_inventory_mutation":
        source = (root / READINESS_SOURCE).read_text(encoding="utf-8")
        marker = "pub struct OfflineRedeemResult {"
        mutated = mutate_once(
            source, marker, marker + "\n    pub unexpected_receipt: Vec<u8>,", name
        )
        require_mutation_rejected(
            name,
            {READINESS_SOURCE: mutated},
            ("OfflineRedeemResult auxiliary field inventory mismatch", "unexpected_receipt"),
        )
        return
    if name == "openapi_contract_mutation":
        document = json.loads((root / OPENAPI_SOURCE).read_text(encoding="utf-8"))
        if document.get("paths", {}).pop("/v1/offline/receiver-lineage", None) is None:
            raise SystemExit(f"policy self-test {name} could not remove receiver-lineage")
        require_mutation_rejected(
            name,
            {OPENAPI_SOURCE: json.dumps(document, sort_keys=True)},
            ("OpenAPI route inventory mismatch", "/v1/offline/receiver-lineage"),
        )
        return
    if name == "smoke_contract_mutation":
        source = (root / TORII_SMOKE_SOURCE).read_text(encoding="utf-8")
        marker = "fn typed_offline_redeem_route_accepts_only_the_direct_v2_request()"
        mutated = mutate_once(
            source,
            marker,
            "fn typed_offline_redeem_route_accepts_legacy_wrappers()",
            name,
        )
        require_mutation_rejected(
            name,
            {TORII_SMOKE_SOURCE: mutated},
            ("Torii offline redeem smoke contract", marker),
        )
        return
    raise SystemExit(f"unknown policy self-test: {name}")


if self_test:
    for self_test_name in sorted(POLICY_SELF_TEST_CASES):
        run_policy_self_test(self_test_name)
    print(
        "Kagemusha ABI-21 policy self-test passed: "
        f"{len(POLICY_SELF_TEST_CASES)} destructive contract mutations rejected"
    )
else:
    print(
        "Kagemusha ABI-21 policy passed: exact lifecycle fields, five routes, "
        "fail-closed release admission, replay namespaces, and payload evidence"
    )
PY
