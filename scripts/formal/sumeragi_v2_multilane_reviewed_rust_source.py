#!/usr/bin/env python3
"""Authenticate and expand reviewed Rust include closures for formal gates."""

from __future__ import annotations

import ast
import hashlib
import json
import os
import re
import stat
import subprocess
import unicodedata
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Iterator


DEFAULT_ROOT = Path(__file__).resolve().parents[2]
REVIEWED_RUST_SOURCE_HELPER_RELATIVE = Path(
    "scripts/formal/sumeragi_v2_multilane_reviewed_rust_source.py"
)
REVIEWED_RUST_INCLUDE_MANIFEST_RELATIVE = Path(
    "scripts/formal/sumeragi_v2_proof_ledger_source_seal_contracts.py"
)
REVIEWED_RUST_INCLUDE_MANIFEST_SHA256 = (
    "6830478f0523f8e320378200b67894bf9a6a3c09574a99741a3a04b76a457990"
)
API_AUTHORITY_SEPARATION_SOURCE_CHECKS = (
    (
        "pytests/scripts/native_amx_v2_grouped_fixture_test.py",
        ("test_sumeragi_status_and_diagnostics_openapi_surfaces_are_disjoint",),
    ),
    (
        "crates/iroha/src/client.rs",
        (
            "pub fn get_sumeragi_status(&self) -> Result<SumeragiV2Status>",
            "pub fn get_sumeragi_diagnostics(&self) -> Result<SumeragiDiagnosticsStatus>",
        ),
    ),
    (
        "crates/iroha/src/client/sumeragi_api_separation_tests.rs",
        (
            "get_sumeragi_status_rejects_unknown_json_fields",
            "status endpoint must reject a diagnostics-shaped payload",
            "get_sumeragi_diagnostics_rejects_json_payload_missing_required_fields",
            "diagnostics endpoint must reject a status-shaped payload",
        ),
    ),
    (
        "python/iroha_torii_client/tests/test_client.py",
        ("test_sumeragi_endpoint_methods_reject_swapped_payload_contracts",),
    ),
    (
        "python/iroha_python/tests/client_sumeragi_v2_status_test.py",
        ("test_typed_endpoint_methods_reject_swapped_sumeragi_payloads",),
    ),
    (
        "javascript/iroha_js/test/toriiClient.test.js",
        (
            "typed Sumeragi endpoints reject swapped status and diagnostics payloads",
            "sumeragi status payload contains unknown field pipeline_execution",
            "sumeragi diagnostics contains unknown field protocol_version",
        ),
    ),
    (
        "IrohaSwift/Tests/IrohaSwiftTests/NativeAmxV2GroupedFixtureTests.swift",
        (
            "func testRustOwnedGroupedNativeAmxV2EndpointSeparation()",
            "status endpoint must reject a diagnostics-shaped payload",
            "diagnostics endpoint must reject a status-shaped payload",
        ),
    ),
    (
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/client/"
        "SumeragiHttpTransportContractTest.kt",
        (
            "fun `status and diagnostics reject missing parameterized or ambiguous JSON content types`()",
            "status endpoint must reject a diagnostics-shaped payload",
            "diagnostics endpoint must reject a status-shaped payload",
        ),
    ),
    (
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/client/"
        "SumeragiHttpTransportTests.java",
        (
            "public void responsesRequireExactContentTypeCanonicalLengthAndBoundedBody()",
            "status endpoint must reject a diagnostics-shaped payload",
            "diagnostics endpoint must reject a status-shaped payload",
        ),
    ),
)
API_AUTHORITY_SEPARATION_SOURCE_PATHS = tuple(
    path for path, _tokens in API_AUTHORITY_SEPARATION_SOURCE_CHECKS
)
FIXTURE_CANONICAL_OWNER_SOURCE_CHECKS = (
    (
        "crates/iroha_data_model/src/bin/sumeragi_v2_wire_fixtures.rs",
        (
            "add `--check`",
            '"--check" if !check_only',
            "native_amx_grouped::write_fixture(",
            "options.check_only",
        ),
    ),
    (
        "crates/iroha_data_model/src/bin/native_amx_grouped.rs",
        (
            '"rust_owner": "iroha_data_model::block::consensus"',
            "pub fn write_fixture(path: &Path, check_only: bool)",
        ),
    ),
    (
        "ci/run_native_amx_v2_grouped_sdk_parity.sh",
        (
            "openapi_require_signed=0",
            'if [[ "${IROHA_RELEASE_SEALED_WORKTREE:-0}" == 1 ]]; then',
            "openapi_require_signed=1",
            'expected_marker="openapi-two-mirror-replay status=success '
            "candidate_oid=${candidate_oid} candidate_tree=${candidate_tree} "
            'mirrors=2 artifacts=5 require_signed=${require_signed}"',
            "grouped Native AMX V2 OpenAPI parity lacks one exact path-free "
            "two-mirror replay marker",
            "observed_test_count=7",
            'OPENAPI_NODE_BIN="$sdk_openapi_node_bin"',
            'OPENAPI_NODE_MODULES_ROOT="$sdk_openapi_node_modules_root"',
            'OPENAPI_REQUIRE_SIGNED="$openapi_require_signed"',
            'bash "${repo_root}/ci/check_openapi_spec.sh"',
            "assert_openapi_replay_marker",
        ),
    ),
    (
        "ci/sumeragi_v2_sdk_source_closure.json",
        (
            '"native-amx-v2-grouped-suite": [',
            '"artifacts/openapi/allowed_signers.json",',
            '"ci/check_openapi_spec.sh",',
        ),
    ),
    (
        "scripts/run_sumeragi_v2_release_gates.sh",
        (
            'readonly native_amx_grouped_parity_harness="ci/'
            'run_native_amx_v2_grouped_sdk_parity.sh"',
            'bash "$native_amx_grouped_parity_harness" '
            "--suite-source-manifest-sha256",
            "grouped Native AMX V2 parity harness returned an invalid source digest",
            '"native-amx-grouped-${native_amx_grouped_parity_surface}"',
            "native_amx_grouped_suite_source_manifest_sha256 \\",
        ),
    ),
    (
        "scripts/write_sumeragi_v2_release_receipt.py",
        (
            '"write_sumeragi_v2_release_receipt_gate_evidence.py": (',
            "0654dc5ac1f8235bc66df852947003054d4d17658703ffe72a38be3be352441b",
            '_SDK_SOURCE_CLOSURE_RESOLVER = "ci/'
            'resolve_sumeragi_v2_sdk_source_closure.py"',
            '_SDK_SOURCE_CLOSURE_MANIFEST = "ci/'
            'sumeragi_v2_sdk_source_closure.json"',
            '_NATIVE_AMX_GROUPED_SOURCE_CLOSURE_SUITE = "native-amx-v2-grouped"',
            "hashlib.sha256(payload).hexdigest()",
            "!= _RELEASE_RECEIPT_COMPONENT_SHA256[filename]",
            '"_sdk_suite_source_manifest",',
        ),
    ),
    (
        "scripts/write_sumeragi_v2_release_receipt_gate_evidence.py",
        (
            "expected_suite_manifest = _sdk_suite_source_manifest(",
            "repo_root, _NATIVE_AMX_GROUPED_SOURCE_CLOSURE_SUITE",
            'fields["native_amx_grouped_suite_source_manifest_sha256"]',
            'replay_marker_prefix = "openapi-two-mirror-replay "',
            'f"candidate_oid={sealed[\'head_commit\']} "',
            'f"candidate_tree={sealed[\'head_tree\']} "',
            '"mirrors=2 artifacts=5 require_signed=1"',
            "replay_markers != [expected_replay_marker]",
            "or lines.index(expected_replay_marker)",
            ">= lines.index(expected_marker)",
            "exact path-free two-mirror replay binding",
        ),
    ),
    (
        "ci/check_sumeragi_v2_multilane_release_inventory.sh",
        (
            "native-amx-rust-fixture-check command 0",
            "regenerate Native AMX Rust fixture authority twice into disjoint "
            "private roots and byte-authenticate both outputs",
            'readonly grouped_parity_harness="ci/'
            'run_native_amx_v2_grouped_sdk_parity.sh"',
            'bash "$grouped_parity_harness" --suite-source-manifest-sha256',
            'symbols["_sdk_suite_source_manifest"](',
            'symbols["_NATIVE_AMX_GROUPED_SOURCE_CLOSURE_SUITE"]',
            '!= "$grouped_suite_source_manifest_sha256"',
            "grouped Native AMX V2 fixture/suite source binding is invalid",
            "grouped Native AMX V2 suite-source manifest SHA-256",
        ),
    ),
)
FIXTURE_CANONICAL_OWNER_SOURCE_PATHS = tuple(
    path for path, _tokens in FIXTURE_CANONICAL_OWNER_SOURCE_CHECKS
)
WIRE_RELEASE_INVARIANT_SOURCE_CHECKS = (
    (
        "scripts/check_no_legacy_codec.sh",
        (
            "retired_native_amx_v1_pattern=",
            "retired_lane_handoff_pattern=",
            "No retired Native AMX V1 consensus codecs found.",
            "No retired lane executable payload handoff codecs found.",
        ),
    ),
    (
        "fixtures/sumeragi_v2/wire_v2.tsv",
        (
            "# kind\tname\thex\texpectation",
            "message\tquorum_certificate_merge_carrier\t",
            "negative_message\texecution_commitment_merge_carrier_wrong_version\t",
            "negative_message\texecution_commitment_missing_merge_carrier_field\t",
        ),
    ),
    (
        "crates/iroha_data_model/src/bin/sumeragi_v2_wire_fixtures.rs",
        (
            'const WIRE_FIXTURE_BASENAME: &str = "wire_v2.tsv";',
            'name: "quorum_certificate_merge_carrier",',
            '"execution_commitment_merge_carrier_wrong_version",',
            '"execution_commitment_missing_merge_carrier_field",',
            "&options.output_dir.join(WIRE_FIXTURE_BASENAME),",
        ),
    ),
    (
        "crates/iroha_data_model/tests/sumeragi_v2_cross_sdk_fixtures.rs",
        (
            'const FIXTURES: &str = include_str!("../../../fixtures/sumeragi_v2/wire_v2.tsv");',
            "fn shared_sdk_accept_fixtures_are_exact_current_rust_encodings()",
            "fn shared_sdk_negative_fixtures_fail_rust_structure_or_protocol_validation()",
            '"quorum_certificate_merge_carrier",',
            '"execution_commitment_merge_carrier_wrong_version",',
            '"execution_commitment_missing_merge_carrier_field",',
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        (
            "fn merge_share_transport_rejects_omission_nonleader_body_and_legacy_version()",
            "legacy.version = MERGE_COMMITTEE_SIGNATURE_VERSION_V2.saturating_sub(1);",
            'expect("reject legacy merge-share version")',
            'expect("read untouched signing guard")',
        ),
    ),
    (
        "IrohaSwift/Tests/IrohaSwiftTests/SumeragiV2WireFixtureTests.swift",
        (
            "final class SumeragiV2WireFixtureTests: XCTestCase",
            'private let fixtureRelativePath = "fixtures/sumeragi_v2/wire_v2.tsv"',
            "func testRustCanonicalMessageFixturesRoundtrip() throws",
            "func testMalformedAndSemanticallyNoncanonicalFixturesFailClosed() throws",
        ),
    ),
    (
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/consensus/"
        "SumeragiV2WireFixtureTest.kt",
        (
            "class SumeragiV2WireFixtureTest",
            'private const val FIXTURE_RELATIVE_PATH = "fixtures/sumeragi_v2/wire_v2.tsv"',
            "fun `rust canonical message fixtures roundtrip`()",
            "fun `malformed and semantically noncanonical fixtures fail closed`()",
        ),
    ),
    (
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/consensus/"
        "SumeragiV2WireFixtureTests.java",
        (
            "public final class SumeragiV2WireFixtureTests",
            'private static final String FIXTURE_RELATIVE_PATH = "fixtures/sumeragi_v2/wire_v2.tsv";',
            "public void rustCanonicalMessageFixturesRoundtrip() throws Exception",
            "public void malformedAndSemanticallyNoncanonicalFixturesFailClosed() throws Exception",
        ),
    ),
    (
        "ci/run_sumeragi_v2_sdk_diagnostics.sh",
        (
            "observed_test_count=34",
            "ExactCertificateCardinalityTests",
            "SumeragiV2WireFixtureTests'",
            "observed_test_count=43",
            "--tests org.hyperledger.iroha.sdk.consensus.SumeragiV2WireFixtureTest",
            "observed_test_count=42",
            "--tests org.hyperledger.iroha.android.consensus.SumeragiV2WireFixtureTests",
        ),
    ),
    (
        "ci/sumeragi_v2_sdk_source_closure.json",
        (
            '"IrohaSwift/Tests/IrohaSwiftTests/ExactCertificateCardinalityTests.swift"',
            '"IrohaSwift/Tests/IrohaSwiftTests/SumeragiV2WireFixtureTests.swift"',
            '"kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/consensus/SumeragiV2WireFixtureTest.kt"',
            '"java/iroha_android/src/test/java/org/hyperledger/iroha/android/consensus/SumeragiV2WireFixtureTests.java"',
            '"python/iroha_torii_client/tests/exact_certificate_cardinality_test.py"',
        ),
    ),
    (
        "scripts/run_sumeragi_v2_release_gates.sh",
        (
            "sumeragi::v2_lane_work::tests::merge_share_transport_rejects_omission_nonleader_body_and_legacy_version",
            "sumeragi_v2_cross_sdk_fixtures::shared_sdk_accept_fixtures_are_exact_current_rust_encodings",
            "sumeragi_v2_cross_sdk_fixtures::shared_sdk_negative_fixtures_fail_rust_structure_or_protocol_validation",
            "cross-sdk-rust cargo-exact 2",
            "sumeragi_v2_sdk_diagnostics_test_counts=(",
            '"sumeragi-diagnostics-${sumeragi_v2_sdk_diagnostics_surface}"',
        ),
    ),
    (
        "scripts/write_sumeragi_v2_release_receipt.py",
        (
            '("swift", 34)',
            '("kotlin", 43)',
            '("java", 42)',
        ),
    ),
    (
        "ci/check_sumeragi_v2_multilane_release_inventory.sh",
        (
            "source-sealed-legacy-codec-guard command 0",
            "bash scripts/check_no_legacy_codec.sh",
            "native-amx-rust-fixture-check command 0",
            "regenerate Native AMX Rust fixture authority twice into disjoint "
            "private roots and byte-authenticate both outputs",
            "for sdk_diagnostics_test_count in 129 88 34 43 42; do",
            "SumeragiV2WireFixtureTest",
        ),
    ),
)
WIRE_RELEASE_INVARIANT_SOURCE_PATHS = tuple(
    path for path, _tokens in WIRE_RELEASE_INVARIANT_SOURCE_CHECKS
)


def _validate_exact_release_invariant_source_checks(
    mutation_id: str, source_checks: object, errors: list[str]
) -> None:
    """Require the exact reviewed API and wire release-source contracts."""

    expected = {
        "ML-MUT-API-02": API_AUTHORITY_SEPARATION_SOURCE_CHECKS,
        "ML-MUT-API-04": FIXTURE_CANONICAL_OWNER_SOURCE_CHECKS,
        "ML-MUT-WIRE-01": WIRE_RELEASE_INVARIANT_SOURCE_CHECKS,
    }.get(mutation_id)
    if expected is None:
        return
    actual = (
        tuple(
            (
                check.get("path"),
                tuple(check.get("required_tokens", ()))
                if isinstance(check.get("required_tokens"), list)
                else (),
            )
            for check in source_checks
            if isinstance(check, dict)
        )
        if isinstance(source_checks, list)
        else ()
    )
    if actual != expected:
        errors.append(
            f"{mutation_id}: semantic source checks differ from the "
            "exact reviewed contract"
        )


NATIVE_PREPUBLICATION_REVIEWED_BINDINGS = (
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "preflight_native_amx_participant_application_route_under_publication_guard",
        (
            "receipt.manifest_artifact_hash != HashOf::new(manifest)",
            "!incoming.matches_manifest(manifest)",
            "NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_TEMP_FILE",
            "inventory_native_amx_evidence_files_locked(&namespace, true)",
            "preflight_native_amx_incoming_artifacts_locked",
            "decode_bound_native_amx_participant_receipt_latest_index_locked",
            "validate_native_amx_prepublication_transition_locked",
            "NativeAmxParticipantApplicationRoutePreflight { incoming, current }",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "preflight_native_amx_incoming_artifacts_locked",
        (
            "validate_native_amx_retained_history_continuity",
            "native_amx_participant_application_pair_framed_bytes",
            "let mut additional_bytes = 0_u64;",
            "read_native_amx_evidence_file_bytes_locked",
            "!= expected_bytes.as_slice()",
            "conflicts with the incoming same-height plan before publication",
            "temporary conflicts with the incoming plan before publication",
            "!inventory.stable(*kind).contains_key(&participant_height)",
            "inventory.temporary(*kind).is_none()",
            "additional_bytes = additional_bytes",
            "native_amx_participant_evidence_startup_bytes",
            "native_amx_evidence_total_payload_bytes(inventory)",
            "bytes.checked_add(additional_bytes)",
            "single bounded transient publication window",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "validate_native_amx_prepublication_transition_locked",
        (
            "if current == incoming {",
            "let durable_manifest = self",
            "read_native_amx_participant_application_manifest_from_paths_locked",
            "let durable_receipt = self",
            "read_native_amx_participant_application_receipt_from_paths_locked",
            ".is_some_and(|durable| durable != manifest)",
            ".is_some_and(|durable| durable != receipt)",
            "native_amx_participant_application_manifest_matches_available_finality_under_prune_and_canonical_guards",
            "Native AMX exact retry does not match authenticated durable evidence",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "persist_native_amx_participant_application_repair_targets_under_publication_guard",
        (
            "preflight_native_amx_participant_application_repair_targets_under_publication_guard",
            "write_native_amx_participant_application_manifest_artifact_with_retention_policy_under_publication_guard",
            "read_back_native_amx_repair_target_manifests_under_publication_guard",
            "write_native_amx_participant_application_receipt_artifact_only_with_retention_policy_under_publication_guard",
            "write_native_amx_participant_receipt_latest_index_for_prepublication_under_publication_guard",
            "authenticate_native_amx_participant_application_prepublication_under_publication_guard",
            "cleanup_native_amx_participant_application_evidence_under_publication_guard",
        ),
    ),
)
NATIVE_PREPUBLICATION_REVIEWED_ORDERED_SOURCE_CHECKS = (
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "preflight_native_amx_participant_application_route_under_publication_guard",
        (
            "inventory_native_amx_evidence_files_locked(&namespace, true)",
            "preflight_native_amx_incoming_artifacts_locked",
            "decode_bound_native_amx_participant_receipt_latest_index_locked",
            "validate_native_amx_prepublication_transition_locked",
            "NativeAmxParticipantApplicationRoutePreflight { incoming, current }",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "preflight_native_amx_incoming_artifacts_locked",
        (
            "validate_native_amx_retained_history_continuity",
            "native_amx_participant_application_pair_framed_bytes",
            "let mut additional_bytes = 0_u64;",
            "!inventory.stable(*kind).contains_key(&participant_height)",
            "inventory.temporary(*kind).is_none()",
            "additional_bytes = additional_bytes",
            "native_amx_participant_evidence_startup_bytes",
            "native_amx_evidence_total_payload_bytes(inventory)",
            "bytes.checked_add(additional_bytes)",
            "single bounded transient publication window",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "validate_native_amx_prepublication_transition_locked",
        (
            "if current == incoming {",
            "let manifest_path =",
            "let receipt_path =",
            "let durable_manifest = self",
            "let durable_receipt = self",
            ".is_some_and(|durable| durable != manifest)",
            ".is_some_and(|durable| durable != receipt)",
            "native_amx_participant_application_manifest_matches_available_finality_under_prune_and_canonical_guards",
            "Native AMX exact retry does not match authenticated durable evidence",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "persist_native_amx_participant_application_repair_targets_under_publication_guard",
        (
            "preflight_native_amx_participant_application_repair_targets_under_publication_guard",
            "write_native_amx_participant_application_manifest_artifact_with_retention_policy_under_publication_guard",
            "read_back_native_amx_repair_target_manifests_under_publication_guard",
            "write_native_amx_participant_application_receipt_artifact_only_with_retention_policy_under_publication_guard",
            "write_native_amx_participant_receipt_latest_index_for_prepublication_under_publication_guard",
            "authenticate_native_amx_participant_application_prepublication_under_publication_guard",
            "cleanup_native_amx_participant_application_evidence_under_publication_guard",
        ),
    ),
)


def _validate_native_prepublication_reviewed_kura_checks(
    binding_items: dict[tuple[str, str, str], str], errors: list[str]
) -> None:
    """Validate semantic relations inside the reviewed Kura bindings."""

    incoming_artifact_preflight = binding_items.get(
        (
            "crates/iroha_core/src/kura.rs",
            "fn",
            "preflight_native_amx_incoming_artifacts_locked",
        )
    )
    if incoming_artifact_preflight is not None:
        compact_incoming_preflight = " ".join(incoming_artifact_preflight.split())
        expected_missing_member_reservation = (
            "if !inventory.stable(*kind).contains_key(&participant_height) "
            "&& inventory.temporary(*kind).is_none() { additional_bytes = "
            "additional_bytes"
        )
        if expected_missing_member_reservation not in compact_incoming_preflight:
            errors.append(
                "Native incoming-artifact preflight must reserve bytes exactly "
                "when both the stable and temporary member are absent"
            )

    exact_retry_transition = binding_items.get(
        (
            "crates/iroha_core/src/kura.rs",
            "fn",
            "validate_native_amx_prepublication_transition_locked",
        )
    )
    if exact_retry_transition is None:
        return
    branch_start = exact_retry_transition.find("if current == incoming {")
    branch_end = exact_retry_transition.find("return Ok(());", branch_start)
    if branch_start < 0 or branch_end < 0:
        errors.append(
            "Native exact-retry prepublication transition must expose its "
            "bounded repair branch"
        )
        return
    exact_retry_branch = exact_retry_transition[branch_start:branch_end]
    compact_exact_retry_branch = " ".join(exact_retry_branch.split())
    expected_present_member_relation = (
        "if durable_manifest .as_ref() "
        ".is_some_and(|durable| durable != manifest) || "
        "durable_receipt .as_ref() "
        ".is_some_and(|durable| durable != receipt) || !self "
        ".native_amx_participant_application_manifest_matches_available_"
        "finality_under_prune_and_canonical_guards( manifest, ) {"
    )
    if expected_present_member_relation not in compact_exact_retry_branch:
        errors.append(
            "Native exact-retry prepublication must reject either present "
            "stable-member mismatch and independently reauthenticate the "
            "incoming manifest finality"
        )
    if ".ok_or_else" in exact_retry_branch:
        errors.append(
            "Native exact-retry prepublication must permit either stable "
            "member to be absent for bounded reconstruction"
        )


def _regular_file(path: Path, label: str, errors: list[str]) -> bool:
    if not path.is_file() or path.is_symlink():
        errors.append(f"{label} must be a regular non-symlink file: {path}")
        return False
    return True


def _manifest_assignment(
    tree: ast.Module, name: str, path: Path, errors: list[str]
) -> ast.expr | None:
    """Return one exact top-level assignment from the reviewed manifest source."""

    values: list[ast.expr] = []
    for node in tree.body:
        if not isinstance(node, ast.Assign) or len(node.targets) != 1:
            continue
        target = node.targets[0]
        if isinstance(target, ast.Name) and target.id == name:
            values.append(node.value)
    if len(values) != 1:
        errors.append(
            f"{path}: reviewed Rust include manifest must define exactly one "
            f"{name} assignment; found {len(values)}"
        )
        return None
    return values[0]


def _safe_manifest_path(raw: str, *, parent: bool) -> bool:
    path = Path(raw)
    return (
        not path.is_absolute()
        and ".." not in path.parts
        and path.suffix == ".rs"
        and path.as_posix() == raw
        and (not parent or len(path.parts) > 1)
    )


def _decode_reviewed_rust_include_manifest(
    path: Path, errors: list[str]
) -> dict[str, tuple[str, ...]]:
    """Safely decode and authenticate the proof-ledger include allowlist."""

    if not _regular_file(path, "reviewed Rust include manifest source", errors):
        return {}
    try:
        source = path.read_text(encoding="utf-8")
        tree = ast.parse(source, filename=str(path))
    except (OSError, UnicodeDecodeError, SyntaxError) as error:
        errors.append(f"{path}: cannot parse reviewed Rust include manifest: {error}")
        return {}

    kura_node = _manifest_assignment(
        tree, "_KURA_PRODUCTION_COMPONENT_FILES", path, errors
    )
    manifest_node = _manifest_assignment(
        tree, "_REVIEWED_RUST_INCLUDE_MANIFESTS", path, errors
    )
    if kura_node is None or manifest_node is None:
        return {}
    try:
        kura_components = ast.literal_eval(kura_node)
    except (ValueError, TypeError) as error:
        errors.append(
            f"{path}: Kura production include tuple is not a literal: {error}"
        )
        return {}
    if (
        not isinstance(kura_components, tuple)
        or not kura_components
        or not all(isinstance(component, str) for component in kura_components)
        or len(kura_components) != len(set(kura_components))
    ):
        errors.append(
            f"{path}: Kura production include tuple must contain unique paths"
        )
        return {}
    if not isinstance(manifest_node, ast.Dict):
        errors.append(f"{path}: reviewed Rust include manifest must be a dict literal")
        return {}

    manifest: dict[str, tuple[str, ...]] = {}
    for key_node, value_node in zip(manifest_node.keys, manifest_node.values):
        try:
            parent = ast.literal_eval(key_node)
        except (ValueError, TypeError) as error:
            errors.append(
                f"{path}: reviewed Rust include parent is not a literal: {error}"
            )
            continue
        if not isinstance(parent, str) or not isinstance(value_node, ast.Tuple):
            errors.append(
                f"{path}: reviewed Rust include entries must map strings to tuples"
            )
            continue
        if parent in manifest:
            errors.append(f"{path}: duplicate reviewed Rust include parent {parent!r}")
            continue
        components: list[str] = []
        malformed = False
        for element in value_node.elts:
            if isinstance(element, ast.Starred):
                if (
                    not isinstance(element.value, ast.Name)
                    or element.value.id != "_KURA_PRODUCTION_COMPONENT_FILES"
                ):
                    errors.append(
                        f"{path}: {parent!r} contains an unreviewed starred include"
                    )
                    malformed = True
                    continue
                components.extend(kura_components)
                continue
            try:
                component = ast.literal_eval(element)
            except (ValueError, TypeError) as error:
                errors.append(
                    f"{path}: {parent!r} include is not a literal: {error}"
                )
                malformed = True
                continue
            if not isinstance(component, str):
                errors.append(f"{path}: {parent!r} include path must be a string")
                malformed = True
                continue
            components.append(component)
        if malformed:
            continue
        if (
            not _safe_manifest_path(parent, parent=True)
            or not components
            or len(components) != len(set(components))
            or any(
                not _safe_manifest_path(component, parent=False)
                for component in components
            )
        ):
            errors.append(
                f"{path}: reviewed Rust include entry {parent!r} has an unsafe "
                "or noncanonical path inventory"
            )
            continue
        manifest[parent] = tuple(components)

    payload = json.dumps(
        manifest, sort_keys=True, separators=(",", ":"), ensure_ascii=True
    ).encode("ascii")
    digest = hashlib.sha256(payload).hexdigest()
    if digest != REVIEWED_RUST_INCLUDE_MANIFEST_SHA256:
        errors.append(
            f"{path}: reviewed Rust include manifest digest must equal "
            f"{REVIEWED_RUST_INCLUDE_MANIFEST_SHA256}; found {digest}"
        )
    return manifest


_CANONICAL_REVIEWED_RUST_INCLUDE_MANIFEST_ERRORS: list[str] = []
_REVIEWED_RUST_INCLUDE_MANIFESTS = _decode_reviewed_rust_include_manifest(
    DEFAULT_ROOT / REVIEWED_RUST_INCLUDE_MANIFEST_RELATIVE,
    _CANONICAL_REVIEWED_RUST_INCLUDE_MANIFEST_ERRORS,
)


def _validate_reviewed_rust_include_manifest(
    root: Path, errors: list[str]
) -> None:
    """Require the target tree to retain the checker-pinned include allowlist."""

    errors.extend(_CANONICAL_REVIEWED_RUST_INCLUDE_MANIFEST_ERRORS)
    target_errors: list[str] = []
    observed = _decode_reviewed_rust_include_manifest(
        root / REVIEWED_RUST_INCLUDE_MANIFEST_RELATIVE, target_errors
    )
    errors.extend(target_errors)
    if not target_errors and observed != _REVIEWED_RUST_INCLUDE_MANIFESTS:
        errors.append(
            f"{root / REVIEWED_RUST_INCLUDE_MANIFEST_RELATIVE}: reviewed Rust "
            "include manifest differs from the checker-pinned allowlist"
        )


def _mask_rust_comments(source: str) -> str:
    """Mask Rust comments and literals while preserving byte offsets and lines."""

    output = list(source)

    def mask(start: int, end: int) -> None:
        for offset in range(start, end):
            if output[offset] != "\n":
                output[offset] = " "

    index = 0
    length = len(source)
    state = "code"
    raw_hashes = 0
    literal_start = 0
    while index < length:
        char = source[index]
        pair = source[index : index + 2]
        if state == "string":
            if char == "\\":
                index += 2
            else:
                if char == '"':
                    index += 1
                    mask(literal_start, index)
                    state = "code"
                else:
                    index += 1
            continue
        if state == "char":
            if char == "\\":
                index += 2
            else:
                if char == "'":
                    index += 1
                    mask(literal_start, index)
                    state = "code"
                else:
                    index += 1
            continue
        if state == "raw-string":
            terminator = '"' + ("#" * raw_hashes)
            if source.startswith(terminator, index):
                index += len(terminator)
                mask(literal_start, index)
                state = "code"
            else:
                index += 1
            continue

        if pair == "//":
            end = source.find("\n", index + 2)
            end = length if end < 0 else end
            mask(index, end)
            index = end
            continue
        if pair == "/*":
            depth = 1
            end = index + 2
            while end < length and depth:
                if source.startswith("/*", end):
                    depth += 1
                    end += 2
                elif source.startswith("*/", end):
                    depth -= 1
                    end += 2
                else:
                    end += 1
            mask(index, end)
            index = end
            continue
        raw_prefix = None
        for prefix in ("br", "cr", "r"):
            if source.startswith(prefix, index):
                cursor = index + len(prefix)
                while cursor < length and source[cursor] == "#":
                    cursor += 1
                if cursor < length and source[cursor] == '"':
                    raw_prefix = (cursor - index - len(prefix), cursor + 1)
                    break
        if raw_prefix is not None:
            literal_start = index
            raw_hashes, index = raw_prefix
            state = "raw-string"
            continue
        if source.startswith(('b"', 'c"'), index):
            literal_start = index
            state = "string"
            index += 2
            continue
        if char == '"':
            literal_start = index
            state = "string"
            index += 1
            continue
        char_quote = index + 1 if source.startswith("b'", index) else index
        if source[char_quote : char_quote + 1] == "'":
            value = char_quote + 1
            if value < length and source[value] == "\\":
                value += 1
                if source[value : value + 2] == "u{":
                    closing_brace = source.find("}", value + 2)
                    value = length if closing_brace < 0 else closing_brace + 1
                elif source[value : value + 1] == "x":
                    value += 3
                else:
                    value += 1
            else:
                value += 1
            is_char_literal = value < length and source[value] == "'"
        else:
            is_char_literal = False
        if is_char_literal:
            literal_start = index
            state = "char"
            index = char_quote + 1
            continue
        index += 1
    if state in {"string", "char", "raw-string"}:
        mask(literal_start, length)
    return "".join(output)


@dataclass(frozen=True)
class ReviewedRustIncludeProvenance:
    """One authenticated include edge in a recursively reviewed source closure."""

    root_parent: Path
    parent: Path
    provider: Path
    line: int
    chain: tuple[Path, ...]


@dataclass(frozen=True)
class ReviewedRustSourceClosure:
    """Expanded source plus the exact providers and include-edge provenance."""

    path: Path
    source: str
    providers: tuple[Path, ...]
    provenance: tuple[ReviewedRustIncludeProvenance, ...]


@dataclass(frozen=True)
class _RustIncludeInvocation:
    relative: str
    start: int
    end: int
    line: int
    binding: str


_ACTIVE_REVIEWED_RUST_SOURCE_CACHE: (
    dict[tuple[Path, str], ReviewedRustSourceClosure] | None
) = None
_ACTIVE_REVIEWED_RUST_GIT_INDEX_CACHE: (
    dict[Path, dict[str, tuple[tuple[str, str, int], ...]]] | None
) = None


@contextmanager
def _reviewed_rust_source_cache() -> Iterator[None]:
    """Cache immutable reviewed expansions for one complete validation run."""

    global _ACTIVE_REVIEWED_RUST_GIT_INDEX_CACHE
    global _ACTIVE_REVIEWED_RUST_SOURCE_CACHE
    if _ACTIVE_REVIEWED_RUST_SOURCE_CACHE is not None:
        yield
        return
    _ACTIVE_REVIEWED_RUST_SOURCE_CACHE = {}
    _ACTIVE_REVIEWED_RUST_GIT_INDEX_CACHE = {}
    try:
        yield
    finally:
        _ACTIVE_REVIEWED_RUST_SOURCE_CACHE = None
        _ACTIVE_REVIEWED_RUST_GIT_INDEX_CACHE = None


def _canonical_provider_relative(raw: str) -> Path | None:
    """Return a portable canonical Rust provider path, or ``None``."""

    if raw != unicodedata.normalize("NFC", raw) or "\\" in raw:
        return None
    posix = PurePosixPath(raw)
    if (
        not raw
        or posix.is_absolute()
        or posix.suffix != ".rs"
        or posix.as_posix() != raw
        or any(part in {"", ".", ".."} for part in posix.parts)
    ):
        return None
    return Path(*posix.parts)


def _normalized_provider_spelling(raw: str) -> str:
    """Normalize separators and dot segments only for alias detection."""

    portable = unicodedata.normalize("NFC", raw).replace("\\", "/")
    absolute = portable.startswith("/")
    parts: list[str] = []
    for part in portable.split("/"):
        if part in {"", "."}:
            continue
        if part == ".." and parts and parts[-1] != "..":
            parts.pop()
        else:
            parts.append(part)
    normalized = "/".join(parts)
    return f"/{normalized}" if absolute else normalized


def _portable_provider_key(relative: Path) -> str:
    return unicodedata.normalize("NFC", relative.as_posix()).casefold()


def _rust_include_invocations(
    source: str,
    provider: Path,
    errors: list[str],
    masked_source: str | None = None,
) -> tuple[_RustIncludeInvocation, ...]:
    """Parse every code-level ``include!`` as one standalone string literal."""

    masked = (
        _mask_rust_comments(source) if masked_source is None else masked_source
    )
    invocations: list[_RustIncludeInvocation] = []
    include_start = re.compile(r"\binclude\s*!\s*\(")
    cursor = 0
    while True:
        match = include_start.search(masked, cursor)
        if match is None:
            break
        line = source.count("\n", 0, match.start()) + 1
        open_parenthesis = match.end() - 1
        depth = 0
        close_parenthesis = None
        for offset in range(open_parenthesis, len(masked)):
            char = masked[offset]
            if char == "(":
                depth += 1
            elif char == ")":
                depth -= 1
                if depth == 0:
                    close_parenthesis = offset
                    break
        if close_parenthesis is None:
            errors.append(
                f"{provider}:{line}: reviewed Rust include! invocation is "
                "unterminated"
            )
            break

        semicolon = close_parenthesis + 1
        while semicolon < len(masked) and masked[semicolon].isspace():
            semicolon += 1
        if semicolon >= len(masked) or masked[semicolon] != ";":
            errors.append(
                f"{provider}:{line}: reviewed Rust include! invocation must end "
                "with a semicolon"
            )
            cursor = close_parenthesis + 1
            continue

        invocation_source = source[match.start() : close_parenthesis + 1]
        literal = re.fullmatch(
            r'include\s*!\s*\(\s*"(?P<relative>[^"\\\r\n]+)"\s*\)',
            invocation_source,
        )
        line_end = source.find("\n", semicolon + 1)
        if line_end < 0:
            line_end = len(source)
            line_ending = line_end
        else:
            line_ending = line_end + 1
        # Preserve trailing comments on the include line. If more code follows
        # the semicolon, insert immediately so that code remains after the
        # recursively expanded provider.
        suffix = masked[semicolon + 1 : line_end]
        insertion_end = line_ending if not suffix.strip() else semicolon + 1
        if literal is None:
            errors.append(
                f"{provider}:{line}: reviewed Rust include! path must be one "
                "literal canonical .rs string"
            )
        else:
            relative = literal.group("relative")
            if _canonical_provider_relative(relative) is None:
                errors.append(
                    f"{provider}:{line}: reviewed Rust include! path is unsafe "
                    f"or noncanonical: {relative!r}"
                )
            else:
                invocations.append(
                    _RustIncludeInvocation(
                        relative=relative,
                        start=match.start(),
                        end=insertion_end,
                        line=line,
                        binding="include!",
                    )
                )
        cursor = insertion_end if insertion_end > match.start() else semicolon + 1
    return tuple(invocations)


def _rust_path_module_invocations(
    source: str,
    provider: Path,
    expected: tuple[str, ...] | None,
    errors: list[str],
    masked_source: str | None = None,
) -> tuple[_RustIncludeInvocation, ...]:
    """Parse manifest-declared literal ``#[path] mod`` provider bindings."""

    masked = (
        _mask_rust_comments(source) if masked_source is None else masked_source
    )
    expected_paths = frozenset(expected or ())
    if not expected_paths:
        return ()
    expected_module_names = frozenset(
        PurePosixPath(relative).stem for relative in expected_paths
    )
    invocations: list[_RustIncludeInvocation] = []
    attribute_start = re.compile(r"#\s*\[\s*path\b")
    module_binding = re.compile(
        r"\s*(?:(?:pub(?:\s*\([^\r\n)]*\))?)\s+)?"
        r"mod\s+(?P<module>(?:r#)?[A-Za-z_][A-Za-z0-9_]*)\s*;"
    )
    cursor = 0
    while True:
        match = attribute_start.search(masked, cursor)
        if match is None:
            break
        line = source.count("\n", 0, match.start()) + 1
        open_bracket = masked.find("[", match.start(), match.end())
        depth = 0
        close_bracket = None
        for offset in range(open_bracket, len(masked)):
            char = masked[offset]
            if char == "[":
                depth += 1
            elif char == "]":
                depth -= 1
                if depth == 0:
                    close_bracket = offset
                    break
        if close_bracket is None:
            errors.append(
                f"{provider}:{line}: reviewed Rust #[path] attribute is "
                "unterminated"
            )
            break

        attribute_source = source[match.start() : close_bracket + 1]
        literal = re.fullmatch(
            r'#\s*\[\s*path\s*=\s*"(?P<relative>[^"\\\r\n]+)"\s*\]',
            attribute_source,
        )
        binding_start = close_bracket + 1
        while True:
            outer_start = binding_start
            while outer_start < len(masked) and masked[outer_start].isspace():
                outer_start += 1
            outer_match = re.match(r"#\s*\[", masked[outer_start:])
            if outer_match is None or attribute_start.match(masked, outer_start):
                break
            outer_bracket = masked.find(
                "[", outer_start, outer_start + outer_match.end()
            )
            outer_depth = 0
            outer_end = None
            for offset in range(outer_bracket, len(masked)):
                if masked[offset] == "[":
                    outer_depth += 1
                elif masked[offset] == "]":
                    outer_depth -= 1
                    if outer_depth == 0:
                        outer_end = offset + 1
                        break
            if outer_end is None:
                break
            binding_start = outer_end
        binding_match = module_binding.match(masked, binding_start)
        if binding_match is None:
            literal_relative = (
                None if literal is None else literal.group("relative")
            )
            if (
                literal_relative in expected_paths
                or (
                    literal_relative is not None
                    and _normalized_provider_spelling(literal_relative)
                    in expected_paths
                )
                or (
                    literal is None
                    and any(
                        f'"{relative}"' in attribute_source
                        for relative in expected_paths
                    )
                )
            ):
                errors.append(
                    f"{provider}:{line}: reviewed Rust #[path] attribute must "
                    "bind one out-of-line mod item"
                )
            cursor = close_bracket + 1
            continue
        semicolon = binding_match.end() - 1
        line_end = source.find("\n", semicolon + 1)
        if line_end < 0:
            line_end = len(source)
            line_ending = line_end
        else:
            line_ending = line_end + 1
        suffix = masked[semicolon + 1 : line_end]
        insertion_end = line_ending if not suffix.strip() else semicolon + 1

        module_name = binding_match.group("module").removeprefix("r#")
        if literal is None:
            if module_name in expected_module_names:
                errors.append(
                    f"{provider}:{line}: reviewed Rust #[path] path must be one "
                    "literal canonical .rs string"
                )
        else:
            relative = literal.group("relative")
            canonical = _canonical_provider_relative(relative)
            if relative in expected_paths:
                if canonical is None:
                    errors.append(
                        f"{provider}:{line}: reviewed Rust #[path] path is "
                        f"unsafe or noncanonical: {relative!r}"
                    )
                else:
                    invocations.append(
                        _RustIncludeInvocation(
                            relative=relative,
                            start=match.start(),
                            end=insertion_end,
                            line=line,
                            binding="#[path] mod",
                        )
                    )
            elif canonical is None and (
                _normalized_provider_spelling(relative) in expected_paths
                or module_name in expected_module_names
            ):
                errors.append(
                    f"{provider}:{line}: reviewed Rust #[path] path is unsafe "
                    f"or noncanonical: {relative!r}"
                )
        cursor = insertion_end
    return tuple(invocations)


def _rust_provider_invocations(
    source: str,
    provider: Path,
    expected: tuple[str, ...] | None,
    errors: list[str],
) -> tuple[_RustIncludeInvocation, ...]:
    """Return exact include and manifest-declared path bindings in source order."""

    masked = _mask_rust_comments(source)
    invocations = sorted(
        (
            *_rust_include_invocations(source, provider, errors, masked),
            *_rust_path_module_invocations(
                source, provider, expected, errors, masked
            ),
        ),
        key=lambda invocation: invocation.start,
    )
    first_bindings: dict[str, _RustIncludeInvocation] = {}
    for invocation in invocations:
        first = first_bindings.setdefault(invocation.relative, invocation)
        if first is invocation:
            continue
        errors.append(
            f"{provider}:{invocation.line}: duplicate reviewed Rust include provider "
            f"binding {invocation.relative!r} via {invocation.binding}; first "
            f"bound at line {first.line} via {first.binding}"
        )
    return tuple(invocations)


def _load_git_index(
    root: Path, errors: list[str]
) -> dict[str, tuple[tuple[str, str, int], ...]] | None:
    """Load the stage-aware Git index used to authenticate provider tracking."""

    canonical_root = root.resolve()
    if (
        _ACTIVE_REVIEWED_RUST_GIT_INDEX_CACHE is not None
        and canonical_root in _ACTIVE_REVIEWED_RUST_GIT_INDEX_CACHE
    ):
        return _ACTIVE_REVIEWED_RUST_GIT_INDEX_CACHE[canonical_root]
    try:
        top_level = subprocess.run(
            ["git", "-C", str(canonical_root), "rev-parse", "--show-toplevel"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
    except OSError as error:
        errors.append(f"{canonical_root}: cannot inspect Git worktree: {error}")
        return None
    if top_level.returncode != 0:
        detail = top_level.stderr.decode("utf-8", errors="replace").strip()
        errors.append(
            f"{canonical_root}: reviewed Rust closure root must be a Git "
            f"worktree root: {detail or 'git rev-parse failed'}"
        )
        return None
    discovered_root = Path(
        top_level.stdout.decode("utf-8", errors="surrogateescape").strip()
    ).resolve()
    if discovered_root != canonical_root:
        errors.append(
            f"{canonical_root}: reviewed Rust closure root must equal Git "
            f"worktree root {discovered_root}"
        )
        return None
    index_result = subprocess.run(
        ["git", "-C", str(canonical_root), "ls-files", "--stage", "-z"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if index_result.returncode != 0:
        detail = index_result.stderr.decode("utf-8", errors="replace").strip()
        errors.append(
            f"{canonical_root}: cannot read Git index for reviewed Rust "
            f"closure: {detail or 'git ls-files failed'}"
        )
        return None

    mutable: dict[str, list[tuple[str, str, int]]] = {}
    for record in index_result.stdout.split(b"\0"):
        if not record:
            continue
        try:
            metadata, encoded_path = record.split(b"\t", 1)
            encoded_mode, encoded_object_id, encoded_stage = metadata.split(b" ", 2)
            relative = os.fsdecode(encoded_path)
            mode = encoded_mode.decode("ascii")
            object_id = encoded_object_id.decode("ascii")
            stage = int(encoded_stage)
        except (ValueError, UnicodeDecodeError) as error:
            errors.append(
                f"{canonical_root}: malformed Git index entry in reviewed Rust "
                f"closure: {error}"
            )
            return None
        mutable.setdefault(relative, []).append((mode, object_id, stage))
    index = {relative: tuple(entries) for relative, entries in mutable.items()}
    if _ACTIVE_REVIEWED_RUST_GIT_INDEX_CACHE is not None:
        _ACTIVE_REVIEWED_RUST_GIT_INDEX_CACHE[canonical_root] = index
    return index


def _strict_provider_stat(
    root: Path,
    relative: Path,
    index: dict[str, tuple[tuple[str, str, int], ...]],
    errors: list[str],
) -> os.stat_result | None:
    """Require a provider to be regular, symlink-free, and stage-zero tracked."""

    provider = root / relative
    current = root
    for position, part in enumerate(relative.parts):
        current /= part
        try:
            metadata = os.lstat(current)
        except OSError as error:
            errors.append(
                f"{provider}: reviewed Rust include provider is missing or "
                f"unreadable: {error}"
            )
            return None
        if stat.S_ISLNK(metadata.st_mode):
            errors.append(
                f"{provider}: reviewed Rust include provider must be a regular "
                "non-symlink file"
            )
            return None
        if position < len(relative.parts) - 1 and not stat.S_ISDIR(metadata.st_mode):
            errors.append(
                f"{provider}: reviewed Rust include provider has a non-directory "
                f"ancestor {current}"
            )
            return None
    if not stat.S_ISREG(metadata.st_mode):
        errors.append(
            f"{provider}: reviewed Rust include provider must be a regular "
            "non-symlink file"
        )
        return None
    entries = index.get(relative.as_posix(), ())
    if len(entries) != 1 or entries[0][2] != 0:
        errors.append(
            f"{provider}: reviewed Rust include provider must have exactly one "
            "stage-zero Git index entry"
        )
        return None
    mode, object_id, _stage = entries[0]
    if mode not in {"100644", "100755"}:
        errors.append(
            f"{provider}: reviewed Rust include provider has non-regular Git "
            f"mode {mode}"
        )
        return None
    if object_id and set(object_id) == {"0"}:
        errors.append(
            f"{provider}: reviewed Rust include provider has an intent-to-add "
            "Git index entry without a bound blob"
        )
        return None
    return metadata


class _ReviewedRustClosureResolver:
    def __init__(
        self,
        root: Path,
        root_parent: Path,
        index: dict[str, tuple[tuple[str, str, int], ...]],
        errors: list[str],
    ) -> None:
        self.root = root
        self.root_parent = root_parent
        self.index = index
        self.errors = errors
        self.providers: list[Path] = []
        self.provenance: list[ReviewedRustIncludeProvenance] = []
        self._claimed: dict[Path, ReviewedRustIncludeProvenance | None] = {}
        self._portable_claims: dict[str, Path] = {}
        self._inode_claims: dict[tuple[int, int], Path] = {}

    def _claim(
        self,
        relative: Path,
        edge: ReviewedRustIncludeProvenance | None,
    ) -> bool:
        if relative in self._claimed:
            first = self._claimed[relative]
            first_site = (
                "closure root"
                if first is None
                else f"{first.parent}:{first.line}"
            )
            site = "closure root" if edge is None else f"{edge.parent}:{edge.line}"
            self.errors.append(
                f"{self.root / relative}: duplicate reviewed Rust include "
                f"provider at {site}; first claimed at {first_site}"
            )
            return False
        portable_key = _portable_provider_key(relative)
        portable_first = self._portable_claims.get(portable_key)
        if portable_first is not None and portable_first != relative:
            self.errors.append(
                f"{self.root / relative}: reviewed Rust include path aliases "
                f"previous provider {portable_first}"
            )
            return False
        metadata = _strict_provider_stat(self.root, relative, self.index, self.errors)
        if metadata is None:
            return False
        inode_key = (metadata.st_dev, metadata.st_ino)
        inode_first = self._inode_claims.get(inode_key)
        if inode_first is not None and inode_first != relative:
            self.errors.append(
                f"{self.root / relative}: reviewed Rust include provider aliases "
                f"the same filesystem object as {inode_first}"
            )
            return False
        self._claimed[relative] = edge
        self._portable_claims[portable_key] = relative
        self._inode_claims[inode_key] = relative
        self.providers.append(relative)
        if edge is not None:
            self.provenance.append(edge)
        return True

    def _expand(self, relative: Path, stack: tuple[Path, ...]) -> str | None:
        provider = self.root / relative
        try:
            source = provider.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            self.errors.append(
                f"{provider}: cannot read reviewed Rust include provider: {error}"
            )
            return None
        initial_error_count = len(self.errors)
        expected = _REVIEWED_RUST_INCLUDE_MANIFESTS.get(relative.as_posix())
        invocations = _rust_provider_invocations(
            source, provider, expected, self.errors
        )
        if len(self.errors) != initial_error_count:
            return None
        observed = tuple(invocation.relative for invocation in invocations)
        if expected is not None and observed != expected:
            self.errors.append(
                f"{provider}: reviewed Rust include inventory must equal "
                f"{expected!r}; found {observed!r} across {len(invocations)} "
                "code-level binding(s)"
            )
            return None

        expanded: list[str] = []
        cursor = 0
        for invocation in invocations:
            literal = _canonical_provider_relative(invocation.relative)
            if literal is None:
                # The parser has already rejected this path. This is defensive.
                return None
            child = relative.parent.joinpath(*literal.parts)
            edge = ReviewedRustIncludeProvenance(
                root_parent=self.root_parent,
                parent=relative,
                provider=child,
                line=invocation.line,
                chain=(*stack, child),
            )
            if child in stack:
                cycle_start = stack.index(child)
                cycle = (*stack[cycle_start:], child)
                self.errors.append(
                    f"{provider}:{invocation.line}: reviewed Rust include cycle: "
                    + " -> ".join(path.as_posix() for path in cycle)
                )
                return None
            if not self._claim(child, edge):
                return None
            child_source = self._expand(child, edge.chain)
            if child_source is None:
                return None
            expanded.append(source[cursor : invocation.end])
            if expanded[-1] and not expanded[-1].endswith("\n"):
                expanded.append("\n")
            expanded.append(
                "/* reviewed-rust-include begin "
                f"root={self.root_parent.as_posix()} "
                f"parent={relative.as_posix()} "
                f"provider={child.as_posix()} line={invocation.line} */\n"
            )
            expanded.append(child_source)
            if child_source and not child_source.endswith("\n"):
                expanded.append("\n")
            expanded.append(
                "/* reviewed-rust-include end "
                f"provider={child.as_posix()} */\n"
            )
            cursor = invocation.end
        expanded.append(source[cursor:])
        return "".join(expanded)

    def resolve(self) -> ReviewedRustSourceClosure | None:
        if not self._claim(self.root_parent, None):
            return None
        source = self._expand(self.root_parent, (self.root_parent,))
        if source is None:
            return None
        return ReviewedRustSourceClosure(
            path=self.root / self.root_parent,
            source=source,
            providers=tuple(self.providers),
            provenance=tuple(self.provenance),
        )


def _resolve_reviewed_rust_source(
    root: Path,
    relative: str,
    label: str,
    errors: list[str],
) -> ReviewedRustSourceClosure | None:
    """Authenticate and recursively expand one reviewed Rust source closure."""

    canonical_root = root.resolve()
    cache_key = (canonical_root, relative)
    if (
        _ACTIVE_REVIEWED_RUST_SOURCE_CACHE is not None
        and cache_key in _ACTIVE_REVIEWED_RUST_SOURCE_CACHE
    ):
        return _ACTIVE_REVIEWED_RUST_SOURCE_CACHE[cache_key]
    root_parent = _canonical_provider_relative(relative)
    if root_parent is None:
        errors.append(
            f"{root / relative}: {label} path must be a portable canonical .rs path"
        )
        return None
    index = _load_git_index(canonical_root, errors)
    if index is None:
        return None
    initial_error_count = len(errors)
    closure = _ReviewedRustClosureResolver(
        canonical_root, root_parent, index, errors
    ).resolve()
    if closure is None or len(errors) != initial_error_count:
        return None
    if _ACTIVE_REVIEWED_RUST_SOURCE_CACHE is not None:
        _ACTIVE_REVIEWED_RUST_SOURCE_CACHE[cache_key] = closure
    return closure


def _read_reviewed_rust_source(
    root: Path,
    relative: str,
    label: str,
    errors: list[str],
) -> tuple[Path, str | None]:
    """Read a Rust source after validating and expanding its reviewed includes."""

    path = root / relative
    if relative not in _REVIEWED_RUST_INCLUDE_MANIFESTS:
        if not _regular_file(path, label, errors):
            return path, None
        try:
            return path, path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            errors.append(f"{path}: cannot read {label}: {error}")
            return path, None
    closure = _resolve_reviewed_rust_source(root, relative, label, errors)
    if closure is None:
        return path, None
    return closure.path, closure.source


def _expanded_source_manifest_paths(
    relative_paths: set[Path],
    root: Path = DEFAULT_ROOT,
    errors: list[str] | None = None,
) -> set[Path]:
    """Add every authenticated include component consumed by a source binding."""

    expanded = set(relative_paths)
    expanded.add(REVIEWED_RUST_SOURCE_HELPER_RELATIVE)
    expanded.add(REVIEWED_RUST_INCLUDE_MANIFEST_RELATIVE)
    closure_errors: list[str] = []
    with _reviewed_rust_source_cache():
        for parent in sorted(relative_paths):
            if parent.as_posix() not in _REVIEWED_RUST_INCLUDE_MANIFESTS:
                continue
            closure = _resolve_reviewed_rust_source(
                root,
                parent.as_posix(),
                f"reviewed Rust source manifest parent {parent}",
                closure_errors,
            )
            if closure is not None:
                expanded.update(closure.providers)
    if errors is not None:
        errors.extend(closure_errors)
    elif closure_errors:
        raise ValueError("\n".join(closure_errors))
    return expanded
