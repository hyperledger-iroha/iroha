#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_PRODUCTION_READINESS_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
MODE="candidate"
SELF_TEST="false"

for argument in "$@"; do
  case "${argument}" in
    candidate|promotion) MODE="${argument}" ;;
    --self-test) SELF_TEST="true" ;;
    *)
      echo "usage: ci/check_kagemusha_production_readiness.sh [candidate|promotion] [--self-test]" >&2
      exit 2
      ;;
  esac
done

python3 - "${ROOT_DIR}" "${MODE}" "${SELF_TEST}" <<'PY'
from __future__ import annotations

import hashlib
import json
import os
import re
import subprocess
import sys
from pathlib import Path

root = Path(sys.argv[1])
mode = sys.argv[2]
self_test = sys.argv[3] == "true"

MODEL = "crates/iroha_data_model/src/offline/mod.rs"
MODEL_COMPONENT = "crates/iroha_data_model/src/offline/kagemusha_model.rs"
PRIVACY = "crates/iroha_data_model/src/privacy.rs"
PRIVACY_PROTOCOL = "crates/iroha_data_model/src/privacy/protocol.rs"
BRIDGE = "crates/connect_norito_bridge/src/lib.rs"
HEADER = "crates/connect_norito_bridge/include/connect_norito_bridge.h"
CATALOG = "crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_terminal_registry_v4.rs"
CORE = "crates/iroha_core/src/smartcontracts/isi/offline.rs"
STEP_TRANSITION = "crates/iroha_core/src/zk/kagemusha_step_transition.rs"
RECURSIVE_BACKEND = "crates/iroha_core/src/zk/kagemusha_v2.rs"
RECURSION_ADAPTER = "crates/iroha_core/src/zk/kagemusha_recursion_adapter.rs"
VALUE_CONTRACT = "crates/iroha_data_model/tests/kagemusha_value_contract.rs"
SCHEMA_GOLDEN = "crates/iroha_data_model/tests/offline_public_schema_golden.rs"
CONFIG = "crates/iroha_config/src/parameters/user.rs"
NODE = "crates/irohad/src/main.rs"
KAGAMI = "crates/iroha_kagami/src/kagemusha.rs"
ROUTES = "crates/iroha_torii_shared/src/route_catalog.rs"
WORKFLOW = ".github/workflows/pr_kagemusha_payload_bench.yml"
IOS_EVIDENCE_CHECKER = "scripts/check_kagemusha_candidate_ios_evidence.py"

ARTIFACTS = (
    "step-eq.params-ipa.krv4",
    "step-eq.proving-key.krv4",
    "step-eq.verifying-key.krv4",
    "step-eq.bootstrap-witness.krv4",
    "step-ep.params-ipa.krv4",
    "step-ep.proving-key.krv4",
    "step-ep.verifying-key.krv4",
    "step-ep.bootstrap-witness.krv4",
)
FINAL_METADATA = (
    "topup-finality-roster-v4.norito",
    "manifest.norito",
    "manifest.norito.sha256",
    "manifest.json",
    "release-attestation-v4.norito",
    "physical-device-benchmark.evidence",
    "cryptographic-review.evidence",
    "promotion-record-v4.norito",
)
MAX_RELEASE_DIRECTORIES = 16
MAX_RELEASE_INVENTORY_ENTRIES = len(ARTIFACTS + FINAL_METADATA)
MAX_MANIFEST_BYTES = 32 * 1024 * 1024
MAX_DIGEST_SIDECAR_BYTES = 65
MAX_RELEASE_ATTESTATION_BYTES = 1024 * 1024
MAX_BENCHMARK_EVIDENCE_BYTES = 16 * 1024 * 1024
MAX_CRYPTOGRAPHIC_REVIEW_BYTES = 1024 * 1024
MAX_PROMOTION_RECORD_BYTES = 1024 * 1024
MAX_DECLARED_ARTIFACT_FILE_BYTES = 5 * 1024 * 1024 * 1024
MAX_DECLARED_ARTIFACT_AGGREGATE_BYTES = 10 * 1024 * 1024 * 1024
READ_CHUNK_BYTES = 1024 * 1024
ROUTE_LITERALS = (
    "/v1/offline/readiness",
    "/v1/offline/top-up",
    "/v1/offline/redeem",
    "/v1/offline/operations/{operation_id}",
)
RETIRED_RECURSIVE_LIFECYCLE_TYPES = (
    "KagemushaRecursiveSpendInitRequestV2",
    "KagemushaRecursiveSpendInitResultV2",
    "KagemushaRecursiveSpendTopUpUnsignedV2",
    "KagemushaRecursiveSpendTopUpRequestV2",
    "KagemushaRecursiveSpendTopUpAnchorV2",
    "KagemushaRecursiveSpendAppendInputV2",
    "KagemushaRecursiveSpendSplitIntentBuildRequestV2",
    "KagemushaRecursiveSpendSplitIntentV2",
    "KagemushaRecursiveSpendAppendRequestV2",
    "KagemushaRecursiveSpendRedeemBuildRequestV2",
    "KagemushaRecursiveSpendRedeemBuildResultV2",
    "KagemushaRecursiveSpendRedemptionIntentV2",
    "KagemushaRecursiveSpendRedemptionIntentBuildRequestV2",
    "KagemushaRecursiveSpendPeerSplitTransitionV2",
    "KagemushaRecursiveSpendRedemptionChangeTransitionV2",
    "KagemushaRecursiveSpendPublicStatementV2",
    "KagemushaRecursiveSpendProofV2",
    "KagemushaRecursiveSpendBundleV2",
    "KagemushaRecursiveSpendRedeemChangeBranchV2",
    "KagemushaRecursiveSpendSplitResultV2",
    "KagemushaRecursiveSpendPeerPaymentV2",
    "KagemushaRecursiveSpendTopUpFinalityEvidenceV2",
    "KagemushaRecursiveSpendVerifyRequestV2",
    "KagemushaRecursiveSpendBundleSummaryV2",
    "KagemushaRecursiveSpendVerifyResultV2",
    "KagemushaRecursiveSpendRedeemResultV2",
    "KagemushaRecursiveSpendRedeemUnsignedV2",
    "KagemushaRecursiveSpendRedeemRequestV2",
    "KagemushaRecursiveSpendTransitionV2",
    "KagemushaRecursiveSpendTransitionValuesV2",
    "KagemushaRecursiveSpendTransitionConfigV2",
    "KagemushaRecursiveSpendTransitionCircuitV2",
    "KagemushaRecursiveSpendTransitionEqCircuitV2",
    "KagemushaRecursiveSpendTransitionEpCircuitV2",
    "kagemusha_recursive_spend_transition_instance_columns_v2",
    "KAGEMUSHA_RECURSIVE_SPEND_V2_PUBLIC_INPUTS_SCHEMA",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V2",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V2",
    "KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V2",
    "KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1",
    "KagemushaRecursiveSpendArtifactManifestV3",
    "KagemushaRecursiveSpendPromotedReleaseV3",
    "KagemushaRecursiveSpendArtifactBindingV3",
)
RETIRED_RECURSIVE_V3_MARKERS = (
    "KAGEMUSHA_VERIFIER_ROLE_STEP_EQ_V3",
    "KAGEMUSHA_VERIFIER_ROLE_STEP_EP_V3",
    "KAGEMUSHA_VERIFIER_PURPOSE_STEP_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFIER_CURVE_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFIER_CURVE_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_PROMOTED_RELEASE_SCHEMA_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_KEY_MAGIC_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMETERS_FILE_NAME_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMETERS_FILE_NAME_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V3",
    "is_kagemusha_v3_",
    "V3 artifact release",
)


def read(relative: str, errors: list[str]) -> str:
    path = root / relative
    if not path.is_file():
        errors.append(f"missing corridor file: {relative}")
        return ""
    return path.read_text(encoding="utf-8")


def read_regular_bounded(path: Path, maximum_bytes: int, label: str) -> bytes:
    """Read one pinned regular file without trusting path metadata as an allocation size."""

    before = path.lstat()
    if path.is_symlink() or not path.is_file() or before.st_nlink != 1:
        raise ValueError(f"{label} must be a singly-linked non-symlink regular file")
    if before.st_size <= 0 or before.st_size > maximum_bytes:
        raise ValueError(f"{label} violates its {maximum_bytes}-byte size limit")
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    chunks: list[bytes] = []
    try:
        opened = os.fstat(descriptor)
        identity = (before.st_dev, before.st_ino)
        if (
            not os.path.samestat(before, opened)
            or opened.st_nlink != 1
            or opened.st_size != before.st_size
        ):
            raise ValueError(f"{label} changed while it was opened")
        size = 0
        while True:
            chunk = os.read(descriptor, min(READ_CHUNK_BYTES, maximum_bytes + 1 - size))
            if not chunk:
                break
            size += len(chunk)
            if size > maximum_bytes:
                raise ValueError(f"{label} exceeds its size limit")
            chunks.append(chunk)
        after_open = os.fstat(descriptor)
        after_path = path.lstat()
        if (
            (after_path.st_dev, after_path.st_ino) != identity
            or not os.path.samestat(before, after_open)
            or size != before.st_size
            or after_path.st_size != size
            or after_path.st_mtime_ns != before.st_mtime_ns
            or after_path.st_ctime_ns != before.st_ctime_ns
        ):
            raise ValueError(f"{label} changed while it was read")
    finally:
        os.close(descriptor)
    return b"".join(chunks)


def inspect_regular_prefix(
    path: Path,
    expected_bytes: int,
    maximum_bytes: int,
    prefix_bytes: int,
    label: str,
) -> bytes:
    """Inspect only a bounded prefix while pinning the complete file's identity and size."""

    before = path.lstat()
    if path.is_symlink() or not path.is_file() or before.st_nlink != 1:
        raise ValueError(f"{label} must be a singly-linked non-symlink regular file")
    if expected_bytes <= 0 or expected_bytes > maximum_bytes or before.st_size != expected_bytes:
        raise ValueError(f"{label} does not match its bounded declared size")
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        if not os.path.samestat(before, opened) or opened.st_nlink != 1:
            raise ValueError(f"{label} changed while it was opened")
        prefix = os.read(descriptor, prefix_bytes)
        after_open = os.fstat(descriptor)
        after_path = path.lstat()
        if (
            len(prefix) != prefix_bytes
            or not os.path.samestat(before, after_open)
            or not os.path.samestat(before, after_path)
            or after_path.st_size != before.st_size
            or after_path.st_mtime_ns != before.st_mtime_ns
            or after_path.st_ctime_ns != before.st_ctime_ns
        ):
            raise ValueError(f"{label} changed while it was inspected")
        return prefix
    finally:
        os.close(descriptor)


def checked_declared_artifact_total(declared_artifacts: dict[str, int]) -> int:
    """Validate each exact artifact size and its aggregate release inventory."""

    total = 0
    for name in ARTIFACTS:
        size_bytes = declared_artifacts[name]
        if size_bytes <= 0 or size_bytes > MAX_DECLARED_ARTIFACT_FILE_BYTES:
            raise ValueError(
                f"artifact {name} violates its "
                f"{MAX_DECLARED_ARTIFACT_FILE_BYTES}-byte size limit"
            )
        total += size_bytes
        if total > MAX_DECLARED_ARTIFACT_AGGREGATE_BYTES:
            raise ValueError(
                "declared artifacts exceed the "
                f"{MAX_DECLARED_ARTIFACT_AGGREGATE_BYTES}-byte aggregate limit"
            )
    return total


def evidence_is_non_placeholder(path: Path, maximum_bytes: int, label: str) -> bool:
    """Scan bounded evidence without retaining the complete file in memory."""

    before = path.lstat()
    if path.is_symlink() or not path.is_file() or before.st_nlink != 1:
        raise ValueError(f"{label} must be a singly-linked non-symlink regular file")
    if before.st_size < 64 or before.st_size > maximum_bytes:
        return False
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    placeholder = re.compile(rb"(?:placeholder|synthetic|dummy|todo|not[ -]?reviewed)", re.I)
    tail = b""
    size = 0
    found = False
    try:
        opened = os.fstat(descriptor)
        if not os.path.samestat(before, opened) or opened.st_nlink != 1:
            raise ValueError(f"{label} changed while it was opened")
        while True:
            chunk = os.read(descriptor, min(READ_CHUNK_BYTES, maximum_bytes + 1 - size))
            if not chunk:
                break
            size += len(chunk)
            if size > maximum_bytes:
                return False
            scan = tail + chunk
            found = found or placeholder.search(scan) is not None
            tail = scan[-64:]
        after_open = os.fstat(descriptor)
        after_path = path.lstat()
        if (
            not os.path.samestat(before, after_open)
            or not os.path.samestat(before, after_path)
            or size != before.st_size
            or after_path.st_size != size
            or after_path.st_mtime_ns != before.st_mtime_ns
            or after_path.st_ctime_ns != before.st_ctime_ns
        ):
            raise ValueError(f"{label} changed while it was scanned")
    finally:
        os.close(descriptor)
    return not found


def require(text: str, relative: str, errors: list[str], *needles: str) -> None:
    for needle in needles:
        if needle not in text:
            errors.append(f"{relative}: missing {needle!r}")


def require_pattern(
    text: str,
    relative: str,
    errors: list[str],
    pattern: str,
    description: str,
) -> None:
    if re.search(pattern, text, flags=re.DOTALL) is None:
        errors.append(f"{relative}: missing {description}")


def forbid(text: str, relative: str, errors: list[str], *needles: str) -> None:
    for needle in needles:
        if needle in text:
            errors.append(f"{relative}: retired corridor remains: {needle!r}")


def static_errors(overrides: dict[str, str] | None = None) -> list[str]:
    errors: list[str] = []
    overrides = overrides or {}
    texts = {
        path: overrides.get(path, read(path, errors))
        for path in (
            MODEL,
            PRIVACY,
            PRIVACY_PROTOCOL,
            BRIDGE,
            HEADER,
            CATALOG,
            CORE,
            STEP_TRANSITION,
            RECURSIVE_BACKEND,
            RECURSION_ADAPTER,
            VALUE_CONTRACT,
            SCHEMA_GOLDEN,
            CONFIG,
            NODE,
            KAGAMI,
            ROUTES,
            WORKFLOW,
        )
    }
    if MODEL not in overrides:
        texts[MODEL] += "\n" + read(MODEL_COMPONENT, errors)
    model = texts[MODEL]
    require(
        model,
        MODEL,
        errors,
        "KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4: u32 = 22",
        '"kagemusha.offline.recursive_spend.artifact_manifest.v4"',
        '"iroha.reviewed-source-closure.v1"',
        "reviewed_source_closure_descriptor_sha256",
        "KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4: [&str; 8]",
        "KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROMOTION_BYTES_V4",
        "ParamsIpa",
        "BootstrapWitness",
        "KagemushaRecursiveSpendReleaseActivationV4",
        "kagemusha_recursive_spend_verifier_key_id_v4",
    )
    forbid(
        model,
        MODEL,
        errors,
        *RETIRED_RECURSIVE_LIFECYCLE_TYPES,
        *RETIRED_RECURSIVE_V3_MARKERS,
    )
    forbid(
        "\n".join(
            texts[path]
            for path in (
                BRIDGE,
                CORE,
                STEP_TRANSITION,
                RECURSIVE_BACKEND,
                VALUE_CONTRACT,
                SCHEMA_GOLDEN,
            )
        ),
        "Rust ABI-21/V4 corridor",
        errors,
        *RETIRED_RECURSIVE_LIFECYCLE_TYPES,
        *RETIRED_RECURSIVE_V3_MARKERS,
    )
    for artifact in ARTIFACTS:
        if model.count(f'"{artifact}"') != 1:
            errors.append(f"{MODEL}: exact-eight artifact {artifact!r} must be declared once")
    availability = re.search(
        r"pub const KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE:\s*bool\s*=\s*"
        r'cfg!\(feature\s*=\s*"kagemusha-production-enabled"\)\s*;',
        model,
    )
    if availability is None:
        errors.append(
            f"{MODEL}: production availability must be controlled only by the "
            "kagemusha-production-enabled feature"
        )

    require(
        texts[PRIVACY],
        PRIVACY,
        errors,
        'include!("privacy/protocol.rs");',
    )
    require(
        texts[PRIVACY_PROTOCOL],
        PRIVACY_PROTOCOL,
        errors,
        "pub const PRIVACY_BRIDGE_ABI_VERSION_V1: u32 = 22;",
    )
    require(
        texts[BRIDGE],
        BRIDGE,
        errors,
        "CONNECT_NORITO_BRIDGE_ABI_VERSION: u32 = PRIVACY_BRIDGE_ABI_VERSION_V1",
        "connect_norito_kagemusha_recursive_spend_artifact_begin_v4",
        "connect_norito_kagemusha_recursive_spend_artifact_set_install_v4",
        "promotion_record_norito_ptr",
        "KagemushaRecursiveSpendReleaseRecordV4",
        ".authenticate(&trusted_policy)",
        "self.promotion_record",
        "validate_against_authenticated_release",
        "require_kagemusha_recursive_spend_production_promotion_v4()?",
        "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v4",
        "connect_norito_kagemusha_recursive_spend_installed_manifest_sha256_v4",
        "installed.validate_live_inventory()?",
        "KagemushaQualifiedArtifactSourceV4",
        "qualify_kagemusha_authenticated_artifact_source_v4(",
        "KagemushaPastaCycleOpaqueVerifierV4::from_qualified_artifact_source(",
        "KagemushaPastaCycleOpaqueProverV4::from_qualified_artifact_source(",
        "from_candidate_artifact_spool_loader(",
        "fn candidate_proving_key_spool(",
        "fn runtime_verifier(",
        "fn runtime_prover(",
        "recursive_spend_v4_prover_and_terminal_verifier_lifetimes_do_not_overlap",
        '"authenticated-v4-artifact-installation"',
        "connect_norito_kagemusha_recursive_spend_init_v4",
        "connect_norito_kagemusha_recursive_spend_append_v4",
        "connect_norito_kagemusha_recursive_spend_verify_v4",
        "connect_norito_kagemusha_recursive_spend_redeem_v4",
        "connect_norito_kagemusha_recursive_spend_redemption_change_prepare_v4",
        "connect_norito_kagemusha_secret_free_buffer",
        "KagemushaRecursiveSpendRedemptionChangePrepareRequestV4",
        "KagemushaRecursiveSpendRedemptionChangePrepareResultV4",
    )
    require(
        texts[HEADER],
        HEADER,
        errors,
        "CONNECT_NORITO_BRIDGE_ABI_VERSION 22",
        "connect_norito_kagemusha_recursive_spend_artifact_begin_v4",
        "connect_norito_kagemusha_recursive_spend_artifact_set_install_v4",
        "connect_norito_kagemusha_recursive_spend_redemption_change_prepare_v4",
        "connect_norito_kagemusha_secret_free_buffer",
        "promotion_record_norito_ptr",
    )
    forbid(
        texts[BRIDGE] + texts[HEADER],
        f"{BRIDGE} / {HEADER}",
        errors,
        "kagemusha_recursive_spend_artifact_begin_v3",
        "kagemusha_recursive_spend_artifact_set_install_v3",
        "kagemusha_recursive_spend_init_v3",
        "kagemusha_recursive_spend_append_v3",
    )

    require(
        texts[CATALOG],
        CATALOG,
        errors,
        "pub struct KagemushaReleaseCatalogV4",
        "pub fn load(policy_path: &Path, artifact_dir: &Path)",
        "exactly eight artifacts",
        "KagemushaPastaCycleOpaqueVerifierV4::from_qualified_artifact_source",
        "DEFAULT_KAGEMUSHA_CATALOG_MAX_DECODED_BYTES_V4",
        "KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROMOTION_BYTES_V4",
    )
    runtime_profile_validation = texts[RECURSION_ADAPTER].split(
        "fn validate_kagemusha_profile_protocol_v4<C>(", 1
    )[-1].split("fn terminal_validate_kagemusha_eq_bootstrap_v4(", 1)[0]
    forbid(
        runtime_profile_validation,
        "runtime Kagemusha protocol validation",
        errors,
        "keygen_vk",
        "kagemusha_bootstrap_verifying_key_v1",
        "validate_bootstrap_protocol",
    )
    require(
        runtime_profile_validation,
        "runtime Kagemusha protocol validation",
        errors,
        "kagemusha_compiled_protocol_structure_sha256",
        "KagemushaStepBootstrapV4::decode_authenticated",
    )
    require_pattern(
        texts[CATALOG],
        CATALOG,
        errors,
        (
            r"const\s+KAGEMUSHA_CATALOG_ARTIFACT_COUNT_V4:\s*usize\s*=\s*"
            r"KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4\.len\(\)\s*;\s*"
            r"[\s\S]*?"
            r"if\s+manifest\s*"
            r"\.profiles\s*\.iter\(\)\s*"
            r"\.map\(\|profile\|\s*profile\.artifacts\.len\(\)\)\s*"
            r"\.sum::<usize>\(\)\s*"
            r"!=\s*KAGEMUSHA_CATALOG_ARTIFACT_COUNT_V4\s*\{"
        ),
        "exact-eight manifest inventory check",
    )
    forbid(
        texts[CATALOG] + texts[CORE] + texts[NODE] + texts[KAGAMI],
        "configured V4 runtime",
        errors,
        "IROHA_KAGEMUSHA_RELEASE_TRUST_ROOT_NORITO_HEX",
        "kagemusha_enabled",
    )
    require(
        texts[KAGAMI],
        KAGAMI,
        errors,
        "fn configured_policy_bytes(path: &Path)",
        'decode_canonical_norito(&configured, "configured Kagemusha V4 release policy")',
        "KagemushaAuthenticatedReleaseV4::verify",
        "ActivateKagemushaRecursiveReleaseV4::new(activation, policy)",
        r'instruction_count\":1',
    )
    require(
        texts[CONFIG] + texts[NODE] + texts[CORE],
        "configured V4 runtime",
        errors,
        "kagemusha_release_policy_path",
        "kagemusha_artifact_dir",
        "KagemushaReleaseCatalogV4::load",
        "ensure_kagemusha_active_release_material_v4",
    )
    require(
        texts[CORE],
        CORE,
        errors,
        "impl Execute for ActivateKagemushaRecursiveReleaseV4",
        "CanActivateKagemushaRecursiveReleaseV4",
        "CanManageOfflineDeviceAttestationPolicy",
        "validate_offline_attestation_policy_for_release_activation",
        "self.device_attestation_policy",
        "impl Execute for TopUpKagemushaRecursiveV4",
        "impl Execute for RedeemKagemushaRecursiveV4",
        "issuance_active_at",
    )
    require_pattern(
        texts[CORE],
        CORE,
        errors,
        (
            r"let\s+change_release\s*=\s*request\s*\.offline_change\s*\.as_ref\(\)"
            r".*?\.transpose\(\)\?\s*;\s*"
            r"if\s+change_release\.as_ref\(\)\.is_some_and\(\|release\|\s*\{\s*"
            r"!\s*release\s*\.cached\s*"
            r"\.issuance_active_at\(state_transaction\.block_height\(\)\)"
        ),
        "offline-change withdrawal-height issuance check",
    )
    for route in ROUTE_LITERALS:
        if route not in texts[ROUTES]:
            errors.append(f"{ROUTES}: stable route changed or disappeared: {route}")
    require(
        texts[WORKFLOW],
        WORKFLOW,
        errors,
        "check_kagemusha_production_readiness.sh candidate",
        "check_kagemusha_production_readiness.sh candidate --self-test",
        "check_kagemusha_recursive_spend_v4_sdk_contract.sh",
        '"crates/iroha_core/src/smartcontracts/isi/offline/**"',
        "cargo test -p iroha_core kagemusha_v4 --lib",
        "cargo test -p iroha_core sparse_confidential_subtree_roots_match_dense_reference --lib",
        "cargo test -p iroha_core next_zero_confidential_path_matches_padded_tree_path --lib",
        "cargo test -p iroha_core sequential_append_paths --lib",
        "cargo test -p iroha_core recursive_state_vector_is_exact_and_zero_padded --lib",
        "cargo test -p iroha_core output_membership --lib",
        "cargo test -p iroha_core v4_eq_frontier_copy_constraints --lib",
        "cargo test -p iroha_core v4_manifest_preserves_exact_little_endian_state_limbs --lib",
        "cargo test -p iroha_core v4_eq_and_ep_public_columns_share_the_v2_result_frontier_limb --lib",
        "cargo test -p iroha_core kagemusha_terminal_registry_v4 --lib",
        "cargo test -p iroha_kagami --bin kagami harden_private_tree",
        "cargo test -p iroha_kagami --bin kagami private_custody_readme_invokes_non_executable_scripts_through_bash",
        "cargo test -p iroha_kagami --bin kagami raw_npos_genesis_receives_the_chain_bound_localnet_epoch_seed",
        "cargo test -p iroha_torii readiness_authenticates_exact_release_without_global_backend_flag",
        "cargo test -p iroha_torii v4_snapshot_admission_authenticates_exact_release_without_global_backend_flag",
        "cargo test -p connect_norito_bridge recursive_spend_v4",
        "cargo test -p connect_norito_bridge output_membership_local_carrier --lib",
    )
    return errors


def strict_json(path: Path) -> dict[str, object]:
    return strict_json_bytes(
        read_regular_bounded(path, MAX_MANIFEST_BYTES, "manifest JSON"),
        "manifest JSON",
    )


def strict_json_bytes(payload: bytes, label: str) -> dict[str, object]:
    def object_pairs(pairs: list[tuple[str, object]]) -> dict[str, object]:
        result: dict[str, object] = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"duplicate JSON key {key!r}")
            result[key] = value
        return result

    value = json.loads(
        payload.decode("utf-8"),
        object_pairs_hook=object_pairs,
        parse_constant=lambda value: (_ for _ in ()).throw(
            ValueError(f"{label} contains non-finite value {value!r}")
        ),
    )
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be an object")
    return value


def release_verifier_command(directory: Path, policy: Path) -> list[str]:
    """Use only the maintained in-tree verifier for promotion decisions."""
    return [
        "cargo",
        "run",
        "--locked",
        "--quiet",
        "-p",
        "iroha_kagami",
        "--",
        "kagemusha",
        "verify-release-v4",
        "--bundle-dir",
        str(directory),
        "--release-policy",
        str(policy),
        "--benchmark-evidence",
        str(directory / "physical-device-benchmark.evidence"),
        "--cryptographic-review",
        str(directory / "cryptographic-review.evidence"),
    ]


def ios_evidence_configuration(
    errors: list[str],
) -> tuple[Path, str, Path] | None:
    """Return the complete opt-in physical-iOS evidence configuration."""

    root_text = os.environ.get("KAGEMUSHA_IOS_DEVICE_EVIDENCE_ROOT", "")
    key_id = os.environ.get("KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_KEY_ID", "")
    public_key_text = os.environ.get(
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_PUBLIC_KEY", ""
    )
    present = tuple(bool(value) for value in (root_text, key_id, public_key_text))
    if not any(present):
        return None
    if not all(present):
        errors.append(
            "physical-iOS evidence requires KAGEMUSHA_IOS_DEVICE_EVIDENCE_ROOT, "
            "KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_KEY_ID, and "
            "KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_PUBLIC_KEY together"
        )
        return None
    ios_root = Path(root_text)
    if not ios_root.is_dir() or ios_root.is_symlink():
        errors.append("physical-iOS evidence root must be a real directory")
        return None
    return ios_root, key_id, Path(public_key_text)


def verify_ios_evidence(
    directory: Path,
    ios_configuration: tuple[Path, str, Path],
) -> tuple[str | None, str | None]:
    """Verify one signed raw physical-iOS slot and return its candidate digest."""

    ios_root, key_id, public_key = ios_configuration
    release_root = ios_root / directory.name
    raw_root = release_root / "raw"
    if (
        not release_root.is_dir()
        or release_root.is_symlink()
        or not raw_root.is_dir()
        or raw_root.is_symlink()
    ):
        return None, (
            f"{directory.name}: physical-iOS evidence must use "
            f"{ios_root}/<manifest-sha256>/raw"
        )
    evidence_path = directory / "physical-device-benchmark.evidence"
    command = [
        sys.executable,
        "-I",
        str(root / IOS_EVIDENCE_CHECKER),
        "--evidence",
        str(evidence_path),
        "--artifact-root",
        str(raw_root),
        "--trusted-key-id",
        key_id,
        "--trusted-public-key",
        str(public_key),
    ]
    checked = subprocess.run(
        command,
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    if checked.returncode != 0:
        detail = (checked.stderr or checked.stdout).strip().splitlines()
        suffix = f": {detail[-1]}" if detail else ""
        return None, f"{directory.name}: physical-iOS evidence verification failed{suffix}"
    try:
        evidence = strict_json_bytes(
            read_regular_bounded(
                evidence_path,
                MAX_BENCHMARK_EVIDENCE_BYTES,
                "signed physical-iOS evidence",
            ),
            "signed physical-iOS evidence",
        )
        artifact_digests = evidence.get("artifact_digests")
        if not isinstance(artifact_digests, dict):
            raise ValueError("artifact_digests is not an object")
        candidate = artifact_digests.get("input/candidate-v4.norito")
        if not isinstance(candidate, dict):
            raise ValueError("candidate artifact binding is missing")
        candidate_sha256 = candidate.get("sha256")
        if (
            not isinstance(candidate_sha256, str)
            or re.fullmatch(r"[0-9a-f]{64}", candidate_sha256) is None
            or candidate_sha256 == "0" * 64
        ):
            raise ValueError("candidate artifact digest is not canonical")
    except (OSError, UnicodeError, ValueError, json.JSONDecodeError) as error:
        return None, f"{directory.name}: invalid signed physical-iOS evidence: {error}"
    return candidate_sha256, None


def promotion_errors() -> list[str]:
    errors: list[str] = []
    policy_text = os.environ.get("KAGEMUSHA_V4_RELEASE_POLICY_PATH", "")
    artifact_text = os.environ.get("KAGEMUSHA_V4_ARTIFACT_ROOT", "")
    if not policy_text or not artifact_text:
        return [
            "promotion requires KAGEMUSHA_V4_RELEASE_POLICY_PATH and KAGEMUSHA_V4_ARTIFACT_ROOT"
        ]
    policy = Path(policy_text)
    artifact_root = Path(artifact_text)
    if (
        not policy.is_file()
        or policy.is_symlink()
        or policy.stat().st_size == 0
        or policy.stat().st_size > 64 * 1024
    ):
        errors.append("promotion policy must be a nonempty regular file")
    if not artifact_root.is_dir() or artifact_root.is_symlink():
        errors.append("promotion artifact root must be a real directory")
        return errors
    ios_configuration = ios_evidence_configuration(errors)

    source_identity: dict[str, object] | None = None
    reviewed_closure_text = os.environ.get(
        "KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE", ""
    )
    reviewed_closure_sha256 = os.environ.get(
        "KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256", ""
    )
    if (
        not reviewed_closure_text
        or re.fullmatch(r"[0-9a-f]{64}", reviewed_closure_sha256) is None
        or reviewed_closure_sha256 == "0" * 64
    ):
        errors.append(
            "promotion requires the independently pinned reviewed source-closure path and SHA-256"
        )
    else:
        source_identity_result = subprocess.run(
            [
                sys.executable,
                "-I",
                str(root / "scripts/kagemusha_source_tree_seal.py"),
                "identity",
                "--root",
                str(root),
                "--reviewed-source-closure",
                reviewed_closure_text,
                "--reviewed-source-closure-sha256",
                reviewed_closure_sha256,
            ],
            cwd=root,
            check=False,
            capture_output=True,
        )
        if source_identity_result.returncode != 0:
            errors.append(
                "promotion source differs from the independently pinned reviewed closure"
            )
        else:
            try:
                parsed_identity = json.loads(source_identity_result.stdout)
                if (
                    not isinstance(parsed_identity, dict)
                    or parsed_identity.get("schema")
                    != "iroha.kagemusha.reviewed_source_tree_identity.v1"
                    or parsed_identity.get("source_repo_dirty") is not False
                    or parsed_identity.get(
                        "reviewed_source_closure_descriptor_sha256"
                    )
                    != reviewed_closure_sha256
                    or not isinstance(
                        parsed_identity.get("reviewed_source_closure"), dict
                    )
                ):
                    raise ValueError("reviewed source identity is not exact")
                source_identity = parsed_identity
            except (UnicodeError, ValueError, json.JSONDecodeError):
                errors.append("promotion reviewed source identity is malformed")
    authenticated_verification_allowed = not errors

    directories = []
    for path in artifact_root.iterdir():
        directories.append(path)
        if len(directories) > MAX_RELEASE_DIRECTORIES:
            errors.append(
                f"promotion artifact root exceeds {MAX_RELEASE_DIRECTORIES} releases"
            )
            return errors
    directories.sort()
    if not directories:
        errors.append("promotion artifact root contains no manifest-digest releases")
        return errors
    if ios_configuration is not None:
        ios_root = ios_configuration[0]
        ios_directories = []
        for path in ios_root.iterdir():
            ios_directories.append(path)
            if len(ios_directories) > MAX_RELEASE_DIRECTORIES:
                errors.append(
                    "physical-iOS evidence root exceeds "
                    f"{MAX_RELEASE_DIRECTORIES} releases"
                )
                return errors
        if {path.name for path in ios_directories} != {
            path.name for path in directories
        }:
            errors.append(
                "physical-iOS evidence root must contain exactly one "
                "manifest-digest directory for every promoted release"
            )
            return errors
    expected_inventory = set(ARTIFACTS + FINAL_METADATA)
    for directory in directories:
        directory_error_count = len(errors)
        ios_candidate_sha256: str | None = None
        if not directory.is_dir() or directory.is_symlink() or not re.fullmatch(r"[0-9a-f]{64}", directory.name):
            errors.append(f"noncanonical release entry: {directory.name}")
            continue
        actual = set()
        for path in directory.iterdir():
            actual.add(path.name)
            if len(actual) > MAX_RELEASE_INVENTORY_ENTRIES:
                errors.append(f"{directory.name}: final release inventory is oversized")
                break
        if actual != expected_inventory:
            errors.append(f"{directory.name}: final release inventory is not exact")
            continue
        try:
            manifest_bytes = read_regular_bounded(
                directory / "manifest.norito", MAX_MANIFEST_BYTES, "manifest.norito"
            )
        except (OSError, ValueError) as error:
            errors.append(f"{directory.name}: invalid manifest.norito: {error}")
            continue
        digest = hashlib.sha256(manifest_bytes).hexdigest()
        if digest != directory.name:
            errors.append(f"{directory.name}: directory does not equal manifest SHA-256")
        try:
            sidecar = read_regular_bounded(
                directory / "manifest.norito.sha256",
                MAX_DIGEST_SIDECAR_BYTES,
                "manifest digest sidecar",
            )
            if sidecar != f"{digest}\n".encode("ascii"):
                errors.append(f"{directory.name}: manifest digest sidecar is not canonical")
        except (OSError, ValueError) as error:
            errors.append(f"{directory.name}: invalid manifest digest sidecar: {error}")
        try:
            manifest = strict_json(directory / "manifest.json")
        except (OSError, UnicodeError, ValueError, json.JSONDecodeError) as error:
            errors.append(f"{directory.name}: invalid manifest JSON: {error}")
            continue
        if manifest.get("schema") != "kagemusha.offline.recursive_spend.artifact_manifest.v4":
            errors.append(f"{directory.name}: manifest schema is not V4")
        if manifest.get("bridge_abi_version") != 22 or manifest.get("source_repo_dirty") is not False:
            errors.append(f"{directory.name}: ABI/source-tree promotion binding is invalid")
        if source_identity is not None and (
            manifest.get("source_commit") != source_identity.get("source_commit")
            or manifest.get("source_tree_sha256")
            != source_identity.get("source_tree_sha256")
            or manifest.get("reviewed_source_closure")
            != source_identity.get("reviewed_source_closure")
            or manifest.get("reviewed_source_closure_descriptor_sha256")
            != source_identity.get("reviewed_source_closure_descriptor_sha256")
        ):
            errors.append(
                f"{directory.name}: manifest differs from the pinned reviewed source closure"
            )
        profiles = manifest.get("profiles")
        roles = []
        if isinstance(profiles, list):
            for profile in profiles:
                if isinstance(profile, dict) and isinstance(profile.get("artifacts"), list):
                    roles.extend(profile["artifacts"])
        if len(roles) != 8:
            errors.append(f"{directory.name}: manifest does not bind exactly eight artifacts")
        declared_artifacts: dict[str, int] = {}
        for role in roles:
            if not isinstance(role, dict):
                continue
            name = role.get("file_name")
            size_bytes = role.get("size_bytes")
            if isinstance(name, str) and isinstance(size_bytes, int) and not isinstance(size_bytes, bool):
                declared_artifacts[name] = size_bytes
        if set(declared_artifacts) != set(ARTIFACTS):
            errors.append(f"{directory.name}: manifest artifact names are not exact")
        else:
            try:
                checked_declared_artifact_total(declared_artifacts)
            except ValueError as error:
                errors.append(f"{directory.name}: {error}")
            else:
                for name in ARTIFACTS:
                    try:
                        prefix = inspect_regular_prefix(
                            directory / name,
                            declared_artifacts[name],
                            MAX_DECLARED_ARTIFACT_FILE_BYTES,
                            8,
                            f"artifact {name}",
                        )
                        if prefix != b"KRV4KEY\0":
                            errors.append(f"{directory.name}/{name}: invalid KRV4 framing")
                    except (OSError, ValueError) as error:
                        errors.append(f"{directory.name}/{name}: invalid artifact: {error}")
        evidence_limits = (
            ("release-attestation-v4.norito", MAX_RELEASE_ATTESTATION_BYTES),
            ("physical-device-benchmark.evidence", MAX_BENCHMARK_EVIDENCE_BYTES),
            ("cryptographic-review.evidence", MAX_CRYPTOGRAPHIC_REVIEW_BYTES),
            ("promotion-record-v4.norito", MAX_PROMOTION_RECORD_BYTES),
        )
        for name, maximum in evidence_limits:
            try:
                if not evidence_is_non_placeholder(directory / name, maximum, name):
                    errors.append(
                        f"{directory.name}/{name}: missing non-placeholder evidence bytes"
                    )
            except (OSError, ValueError) as error:
                errors.append(f"{directory.name}/{name}: invalid evidence: {error}")
        if ios_configuration is not None and len(errors) == directory_error_count:
            ios_candidate_sha256, ios_error = verify_ios_evidence(
                directory, ios_configuration
            )
            if ios_error is not None:
                errors.append(ios_error)
        if authenticated_verification_allowed and len(errors) == directory_error_count:
            command = release_verifier_command(directory, policy)
            verified = subprocess.run(
                command,
                cwd=root,
                check=False,
                capture_output=True,
                text=True,
            )
            if verified.returncode != 0:
                detail = (verified.stderr or verified.stdout).strip().splitlines()
                suffix = f": {detail[-1]}" if detail else ""
                errors.append(
                    f"{directory.name}: authenticated V4 release verification failed{suffix}"
                )
            elif ios_candidate_sha256 is not None:
                try:
                    report = strict_json_bytes(
                        verified.stdout.encode("utf-8"),
                        "Kagami V4 verification report",
                    )
                    reconstructed_candidate = report.get("candidate_sha256")
                    if reconstructed_candidate != ios_candidate_sha256:
                        raise ValueError(
                            "signed physical-iOS candidate differs from "
                            "Kagami's reconstructed immutable release candidate"
                        )
                except (UnicodeError, ValueError, json.JSONDecodeError) as error:
                    errors.append(
                        f"{directory.name}: physical-iOS release binding failed: {error}"
                    )
    return errors


errors = static_errors()
if mode == "promotion":
    errors.extend(promotion_errors())

if self_test:
    baseline = {
        MODEL: read(MODEL, []) + "\n" + read(MODEL_COMPONENT, []),
        PRIVACY: read(PRIVACY, []),
        PRIVACY_PROTOCOL: read(PRIVACY_PROTOCOL, []),
        CATALOG: read(CATALOG, []),
        CORE: read(CORE, []),
        WORKFLOW: read(WORKFLOW, []),
    }
    mutated = baseline[MODEL].replace(
        "KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4: u32 = 22",
        "KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4: u32 = 21",
    )
    if not static_errors({MODEL: mutated}):
        errors.append("self-test failed to reject ABI-21 substitution")
    shared_bridge_abi_drift = baseline[PRIVACY_PROTOCOL].replace(
        "pub const PRIVACY_BRIDGE_ABI_VERSION_V1: u32 = 22;",
        "pub const PRIVACY_BRIDGE_ABI_VERSION_V1: u32 = 21;",
        1,
    )
    if not static_errors({PRIVACY_PROTOCOL: shared_bridge_abi_drift}):
        errors.append("self-test failed to reject shared bridge ABI-21 substitution")
    detached_protocol_surface = baseline[PRIVACY].replace(
        'include!("privacy/protocol.rs");',
        "// protocol include removed",
        1,
    )
    if not static_errors({PRIVACY: detached_protocol_surface}):
        errors.append("self-test failed to reject detached privacy protocol surface")
    flipped_availability = baseline[MODEL].replace(
        'cfg!(feature = "kagemusha-production-enabled")',
        "true",
        1,
    )
    if not static_errors({MODEL: flipped_availability}):
        errors.append("self-test failed to reject an invalid availability state")
    seven_artifacts = baseline[CATALOG].replace(
        "KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4.len();",
        "7;",
        1,
    )
    seven_artifact_errors = static_errors({CATALOG: seven_artifacts})
    if not any("exact-eight manifest inventory check" in error for error in seven_artifact_errors):
        errors.append("self-test failed to reject a seven-artifact manifest check")
    unguarded_change = baseline[CORE].replace(
        "change_release.as_ref().is_some_and(|release|",
        "change_release.as_ref().is_none_or(|release|",
        1,
    )
    unguarded_change_errors = static_errors({CORE: unguarded_change})
    if not any(
        "offline-change withdrawal-height issuance check" in error
        for error in unguarded_change_errors
    ):
        errors.append("self-test failed to reject an unguarded offline-change issuance path")
    missing_frontier_filter = baseline[WORKFLOW].replace(
        "cargo test -p iroha_core output_membership --lib",
        "cargo test -p iroha_core retired_output_membership_filter --lib",
        1,
    )
    missing_frontier_filter_errors = static_errors({WORKFLOW: missing_frontier_filter})
    if not any(
        "cargo test -p iroha_core output_membership --lib" in error
        for error in missing_frontier_filter_errors
    ):
        errors.append("self-test failed to reject a missing frontier-test workflow filter")
    boundary_artifacts = {
        name: MAX_DECLARED_ARTIFACT_AGGREGATE_BYTES // len(ARTIFACTS)
        for name in ARTIFACTS
    }
    if (
        checked_declared_artifact_total(boundary_artifacts)
        != MAX_DECLARED_ARTIFACT_AGGREGATE_BYTES
    ):
        errors.append("self-test failed to accept the exact artifact aggregate limit")
    exact_file_artifacts = {name: 1 for name in ARTIFACTS}
    exact_file_artifacts[ARTIFACTS[0]] = MAX_DECLARED_ARTIFACT_FILE_BYTES
    if (
        checked_declared_artifact_total(exact_file_artifacts)
        != MAX_DECLARED_ARTIFACT_FILE_BYTES + len(ARTIFACTS) - 1
    ):
        errors.append("self-test failed to accept the exact artifact file limit")
    oversized_file_artifacts = dict(boundary_artifacts)
    oversized_file_artifacts[ARTIFACTS[0]] = MAX_DECLARED_ARTIFACT_FILE_BYTES + 1
    try:
        checked_declared_artifact_total(oversized_file_artifacts)
    except ValueError:
        pass
    else:
        errors.append("self-test failed to reject an oversized artifact file")
    oversized_aggregate_artifacts = dict(boundary_artifacts)
    oversized_aggregate_artifacts[ARTIFACTS[0]] += 1
    try:
        checked_declared_artifact_total(oversized_aggregate_artifacts)
    except ValueError:
        pass
    else:
        errors.append("self-test failed to reject an oversized artifact aggregate")
    verifier_override_name = "KAGEMUSHA_V4_RELEASE_" + "VERIFIER_BIN"
    readiness_source = (root / "ci/check_kagemusha_production_readiness.sh").read_text(
        encoding="utf-8"
    )
    if verifier_override_name in readiness_source:
        errors.append("self-test found a production release-verifier override hook")
    previous_verifier_override = os.environ.get(verifier_override_name)
    os.environ[verifier_override_name] = "/usr/bin/true"
    try:
        verifier_command = release_verifier_command(Path("release"), Path("policy.norito"))
    finally:
        if previous_verifier_override is None:
            del os.environ[verifier_override_name]
        else:
            os.environ[verifier_override_name] = previous_verifier_override
    if verifier_command[:7] != [
        "cargo",
        "run",
        "--locked",
        "--quiet",
        "-p",
        "iroha_kagami",
        "--",
    ]:
        errors.append("self-test failed to reject a substituted release verifier")

if errors:
    print(f"Kagemusha ABI-21/V4 {mode} corridor failed:", file=sys.stderr)
    for error in errors:
        print(f" - {error}", file=sys.stderr)
    raise SystemExit(1)
print(f"Kagemusha ABI-21/V4 {mode} corridor passed.")
PY
