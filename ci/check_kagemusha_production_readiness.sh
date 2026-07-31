#!/usr/bin/env bash
set -euo pipefail

SCRIPT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT_DIR="${KAGEMUSHA_PRODUCTION_READINESS_ROOT:-${SCRIPT_ROOT}}"
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

if [[ "${MODE}" == "promotion" && "${ROOT_DIR}" != "${SCRIPT_ROOT}" ]]; then
  echo "promotion must run from the admitted closure source; root override is forbidden" >&2
  exit 2
fi

run_readiness_python() {
  "$@" <<'PY'
from __future__ import annotations

import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import types
from pathlib import Path

sys.dont_write_bytecode = True
root = Path(sys.argv[1])
mode = sys.argv[2]
self_test = sys.argv[3] == "true"
if sys.flags.isolated != 1:
    raise SystemExit("readiness corridor requires Python isolated mode")
if mode == "promotion":
    admitted_python = os.environ.get("KAGEMUSHA_BUILD_PYTHON_EXECUTABLE", "")
    if (
        not admitted_python
        or not Path(admitted_python).is_absolute()
        or Path(sys.executable).resolve(strict=True)
        != Path(admitted_python).resolve(strict=True)
    ):
        raise SystemExit(
            "promotion must run directly under the admitted closure Python"
        )

MODEL = "crates/iroha_data_model/src/offline/mod.rs"
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
TORII = "crates/iroha_torii/src/lib.rs"
KAGAMI = "crates/iroha_kagami/src/kagemusha.rs"
ROUTES = "crates/iroha_torii_shared/src/route_catalog.rs"
WORKFLOW = ".github/workflows/pr_kagemusha_payload_bench.yml"

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
MAX_DECLARED_ARTIFACT_BYTES = 256 * 1024 * 1024
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
            TORII,
            KAGAMI,
            ROUTES,
            WORKFLOW,
        )
    }
    model = texts[MODEL]
    require(
        model,
        MODEL,
        errors,
        "KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4: u32 = 21",
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
        texts[BRIDGE],
        BRIDGE,
        errors,
        "CONNECT_NORITO_BRIDGE_ABI_VERSION: u32 = 21",
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
        "CONNECT_NORITO_BRIDGE_ABI_VERSION 21",
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
            r"let\s+(?P<descriptors>[A-Za-z_][A-Za-z0-9_]*)\s*=\s*manifest\s*"
            r"\.profiles\s*\.iter\(\)\s*"
            r"\.flat_map\(\|profile\|\s*profile\.artifacts\.iter\(\)\)\s*"
            r"\.collect::<Vec<_>>\(\)\s*;\s*"
            r"if\s+(?P=descriptors)\.len\(\)\s*!=\s*"
            r"KAGEMUSHA_CATALOG_ARTIFACT_COUNT_V4\s*\{"
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
        texts[CONFIG] + texts[NODE] + texts[TORII],
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
    def object_pairs(pairs: list[tuple[str, object]]) -> dict[str, object]:
        result: dict[str, object] = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"duplicate JSON key {key!r}")
            result[key] = value
        return result

    value = json.loads(
        read_regular_bounded(path, MAX_MANIFEST_BYTES, "manifest JSON").decode("utf-8"),
        object_pairs_hook=object_pairs,
    )
    if not isinstance(value, dict):
        raise ValueError("manifest JSON must be an object")
    return value


def source_only_module(
    module_name: str,
    path: Path,
    source_bytes: bytes | None = None,
) -> types.ModuleType:
    """Execute stable source bytes directly without consulting any repo pyc."""

    if source_bytes is None:
        source_bytes = read_regular_bounded(
            path,
            16 * 1024 * 1024,
            f"{module_name} source",
        )
    module = types.ModuleType(module_name)
    module.__file__ = str(path)
    module.__package__ = module_name.rpartition(".")[0]
    sys.modules[module_name] = module
    try:
        exec(compile(source_bytes, str(path), "exec"), module.__dict__)
    except BaseException:
        sys.modules.pop(module_name, None)
        raise
    return module


def require_root_controlled_bootstrap(path: Path, source_root: Path) -> None:
    """Fail before executing a bootstrap that the promotion UID could replace."""

    metadata = path.lstat()
    source_metadata = source_root.lstat()
    if (
        path.resolve(strict=True) != path
        or source_root.resolve(strict=True) != source_root
        or not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != 0
        or metadata.st_nlink != 1
        or metadata.st_mode & 0o222 != 0
        or not stat.S_ISDIR(source_metadata.st_mode)
        or source_metadata.st_uid != 0
        or source_metadata.st_mode & 0o222 != 0
    ):
        raise ValueError("promotion bootstrap is not root-published and immutable")
    ancestor = source_root.parent
    while True:
        ancestor_metadata = ancestor.lstat()
        if (
            ancestor.resolve(strict=True) != ancestor
            or not stat.S_ISDIR(ancestor_metadata.st_mode)
            or ancestor_metadata.st_uid != 0
            or ancestor_metadata.st_mode & 0o022 != 0
        ):
            raise ValueError(
                "promotion bootstrap parent chain is not root-controlled"
            )
        if ancestor == ancestor.parent:
            break
        ancestor = ancestor.parent


def require_root_controlled_storage(
    path: Path,
    *,
    directory: bool,
) -> None:
    """Require immutable promotion input storage outside the operator UID."""

    metadata = path.lstat()
    if (
        path.resolve(strict=True) != path
        or metadata.st_uid != 0
        or metadata.st_mode & 0o222 != 0
        or metadata.st_mode & 0o7000 != 0
        or (directory and not stat.S_ISDIR(metadata.st_mode))
        or (
            not directory
            and (
                not stat.S_ISREG(metadata.st_mode)
                or metadata.st_nlink != 1
            )
        )
    ):
        raise ValueError("promotion input is not root-published and immutable")
    ancestor = path.parent
    while True:
        ancestor_metadata = ancestor.lstat()
        if (
            ancestor.resolve(strict=True) != ancestor
            or not stat.S_ISDIR(ancestor_metadata.st_mode)
            or ancestor_metadata.st_uid != 0
            or ancestor_metadata.st_mode & 0o022 != 0
        ):
            raise ValueError(
                "promotion input parent chain is not root-controlled"
            )
        if ancestor == ancestor.parent:
            break
        ancestor = ancestor.parent


def release_verifier_command(
    verifier: Path,
    directory: Path,
    policy: Path,
) -> list[str]:
    """Use only the admitted root-published Kagami verifier."""
    return [
        str(verifier),
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
    generated_receipt_sha256 = os.environ.get(
        "KAGEMUSHA_ROOT_PUBLISHED_GENERATED_CANDIDATE_RECEIPT_SHA256",
        "",
    )
    launch_receipt_sha256 = os.environ.get(
        "KAGEMUSHA_GENERATION_WORKER_LAUNCH_RECEIPT_SHA256",
        "",
    )
    if (
        re.fullmatch(r"[0-9a-f]{64}", generated_receipt_sha256) is None
        or re.fullmatch(r"[0-9a-f]{64}", launch_receipt_sha256) is None
        or generated_receipt_sha256 == "0" * 64
        or launch_receipt_sha256 == "0" * 64
        or generated_receipt_sha256 == launch_receipt_sha256
    ):
        errors.append(
            "promotion requires distinct nonzero generated-candidate and "
            "generation-worker launch receipt SHA-256 pins"
        )
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
    try:
        policy = policy.resolve(strict=True)
        artifact_root = artifact_root.resolve(strict=True)
        require_root_controlled_storage(policy, directory=False)
        require_root_controlled_storage(artifact_root, directory=True)
    except OSError:
        errors.append("promotion policy/artifact paths must resolve exactly")
        return errors
    except ValueError as error:
        errors.append(f"promotion policy/artifact storage is mutable: {error}")
        return errors

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
    captured_commit: str | None = None

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
    private_materialization = None
    materialized_source_root: Path | None = None
    promotion_toolchain = None
    promotion_verifier = None
    source_seal = None
    candidate_builder = None
    published_build = None
    if not errors:
        try:
            source_seal_path = root / "scripts/kagemusha_source_tree_seal.py"
            require_root_controlled_bootstrap(source_seal_path, root)
            resource_guard_path = (
                root / "scripts/formal/run_sumeragi_v2_tlapm_guard.py"
            )
            require_root_controlled_bootstrap(resource_guard_path, root)
            provenance_text = os.environ.get(
                "KAGEMUSHA_BUILD_TOOLCHAIN_PROVENANCE",
                "",
            )
            provenance_sha256 = os.environ.get(
                "KAGEMUSHA_BUILD_TOOLCHAIN_PROVENANCE_SHA256",
                "",
            )
            if not provenance_text:
                raise ValueError(
                    "promotion requires pinned production-closure provenance"
                )
            candidate_builder_path = (
                root / "scripts/build_kagemusha_v4_candidate_bundle.py"
            )
            require_root_controlled_bootstrap(candidate_builder_path, root)
            candidate_builder = source_only_module(
                "_kagemusha_ci_candidate_builder",
                candidate_builder_path,
            )
            scripts_package = sys.modules.get("scripts")
            if scripts_package is None:
                scripts_package = types.ModuleType("scripts")
                scripts_package.__path__ = []
                sys.modules["scripts"] = scripts_package
            sys.modules["scripts.kagemusha_source_tree_seal"] = (
                candidate_builder.source_seal
            )
            scripts_package.kagemusha_source_tree_seal = (
                candidate_builder.source_seal
            )
            published_build_path = (
                root / "scripts/kagemusha_root_published_build.py"
            )
            require_root_controlled_bootstrap(published_build_path, root)
            published_build = source_only_module(
                "_kagemusha_ci_root_published_build",
                published_build_path,
            )
            source_seal = candidate_builder.source_seal
            promotion_toolchain = (
                candidate_builder._admitted_production_rust_toolchain(
                    "cargo",
                    Path(provenance_text),
                    provenance_sha256,
                )
            )
            reviewed_closure_path = Path(reviewed_closure_text).resolve(strict=True)
            candidate_builder._admit_production_closure_binding(
                root,
                reviewed_closure_path,
                promotion_toolchain,
            )
            source_seal.configure_production_git(
                promotion_toolchain.git,
                promotion_toolchain.git_exec_path,
            )
            expected_identity = source_seal.compute_identity(
                root,
                reviewed_closure_text,
                reviewed_closure_sha256,
            )
            source_identity = {
                "reviewed_source_closure": (
                    expected_identity.reviewed_source_closure
                ),
                "reviewed_source_closure_descriptor_sha256": (
                    expected_identity.reviewed_source_closure_descriptor_sha256
                ),
                "schema": source_seal.SOURCE_IDENTITY_SCHEMA,
                "source_commit": expected_identity.source_commit,
                "source_repo_dirty": expected_identity.source_repo_dirty,
                "source_tree_sha256": expected_identity.source_tree_sha256,
            }
            captured_commit = expected_identity.source_commit
            private_materialization = source_seal.SourceMaterialization(
                root=root,
                reviewed_source_closure=reviewed_closure_path,
                identity=expected_identity,
            )
            materialized_source_root = root
            candidate_builder._reject_ambient_cargo_configs(
                materialized_source_root
            )
            verifier_receipt_text = os.environ.get(
                "KAGEMUSHA_PROMOTION_VERIFIER_RECEIPT",
                "",
            )
            verifier_receipt_sha256 = os.environ.get(
                "KAGEMUSHA_PROMOTION_VERIFIER_RECEIPT_SHA256",
                "",
            )
            if not verifier_receipt_text:
                raise ValueError(
                    "promotion requires a pinned root-published Kagami verifier"
                )
            promotion_verifier = published_build.admit_promotion_verifier(
                Path(verifier_receipt_text),
                verifier_receipt_sha256,
            )
            candidate_builder._admit_macos_dynamic_tool_closure(
                promotion_verifier.artifact_root,
                (promotion_verifier.executable,),
                otool=promotion_toolchain.linker.parent / "otool",
            )
            materialized_identity = private_materialization.identity
            if (
                materialized_identity.source_commit
                != source_identity.get("source_commit")
                or materialized_identity.source_tree_sha256
                != source_identity.get("source_tree_sha256")
                or materialized_identity.source_repo_dirty
                is not source_identity.get("source_repo_dirty")
                or materialized_identity.reviewed_source_closure
                != source_identity.get("reviewed_source_closure")
                or materialized_identity.reviewed_source_closure_descriptor_sha256
                != source_identity.get(
                    "reviewed_source_closure_descriptor_sha256"
                )
            ):
                raise ValueError(
                    "private promotion source differs from reviewed identity"
                )
            if not isinstance(captured_commit, str):
                raise ValueError(
                    "promotion reviewed source identity has no exact commit"
                )
            candidate_builder._verify_exact_signed_commit(
                materialized_source_root,
                captured_commit,
                promotion_toolchain,
            )
            if (
                promotion_verifier.production_closure_tree_sha256
                != promotion_toolchain.closure_tree_sha256
                or promotion_verifier.toolchain_provenance_sha256
                != promotion_toolchain.provenance_sha256
                or promotion_verifier.reviewed_source_closure_descriptor_sha256
                != expected_identity.reviewed_source_closure_descriptor_sha256
                or promotion_verifier.source_commit
                != expected_identity.source_commit
                or promotion_verifier.source_tree_sha256
                != expected_identity.source_tree_sha256
            ):
                raise ValueError(
                    "root-published Kagami verifier differs from the admitted "
                    "source/build closure"
                )
        except (ImportError, OSError, ValueError, RuntimeError) as error:
            errors.append(
                f"promotion could not admit the root-published build closure: {error}"
            )
    authenticated_verification_allowed = (
        not errors
        and private_materialization is not None
        and materialized_source_root is not None
        and promotion_toolchain is not None
        and promotion_verifier is not None
        and source_seal is not None
        and candidate_builder is not None
        and published_build is not None
    )
    expected_inventory = set(ARTIFACTS + FINAL_METADATA)
    for directory in directories:
        directory_error_count = len(errors)
        if not directory.is_dir() or directory.is_symlink() or not re.fullmatch(r"[0-9a-f]{64}", directory.name):
            errors.append(f"noncanonical release entry: {directory.name}")
            continue
        try:
            require_root_controlled_storage(directory, directory=True)
        except (OSError, ValueError) as error:
            errors.append(
                f"{directory.name}: release directory is mutable: {error}"
            )
            continue
        actual = set()
        for path in directory.iterdir():
            actual.add(path.name)
            try:
                require_root_controlled_storage(path, directory=False)
            except (OSError, ValueError) as error:
                errors.append(
                    f"{directory.name}/{path.name}: release file is mutable: {error}"
                )
                break
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
        if (
            manifest.get("bridge_abi_version") != 21
            or not isinstance(manifest.get("source_repo_dirty"), bool)
            or manifest.get(
                "root_published_generated_candidate_receipt_sha256"
            )
            != generated_receipt_sha256
            or manifest.get("generation_worker_launch_receipt_sha256")
            != launch_receipt_sha256
        ):
            errors.append(
                f"{directory.name}: ABI/source/receipt promotion binding is invalid"
            )
        if source_identity is not None and (
            manifest.get("source_commit") != source_identity.get("source_commit")
            or manifest.get("source_tree_sha256")
            != source_identity.get("source_tree_sha256")
            or manifest.get("source_repo_dirty")
            != source_identity.get("source_repo_dirty")
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
        elif sum(declared_artifacts.values()) > MAX_DECLARED_ARTIFACT_BYTES:
            errors.append(
                f"{directory.name}: declared artifacts exceed {MAX_DECLARED_ARTIFACT_BYTES} bytes"
            )
        else:
            for name in ARTIFACTS:
                try:
                    prefix = inspect_regular_prefix(
                        directory / name,
                        declared_artifacts[name],
                        MAX_DECLARED_ARTIFACT_BYTES,
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
        if authenticated_verification_allowed and len(errors) == directory_error_count:
            assert materialized_source_root is not None
            assert private_materialization is not None
            assert source_seal is not None
            assert candidate_builder is not None
            assert promotion_toolchain is not None
            assert promotion_verifier is not None
            verifier_command = release_verifier_command(
                promotion_verifier.executable,
                directory,
                policy,
            )
            verified = subprocess.run(
                verifier_command,
                cwd=promotion_verifier.artifact_root,
                env={
                    "HOME": "/var/empty",
                    "LANG": "C",
                    "LC_ALL": "C",
                    "PATH": "/usr/bin:/bin",
                    "TMPDIR": "/private/tmp",
                    "TZ": "UTC",
                },
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
            elif source_seal is not None and private_materialization is not None:
                try:
                    candidate_builder._recheck_admitted_toolchain(
                        promotion_toolchain
                    )
                    current_materialized_identity = source_seal.compute_identity(
                        materialized_source_root,
                        str(private_materialization.reviewed_source_closure),
                        reviewed_closure_sha256,
                    )
                    if current_materialized_identity != private_materialization.identity:
                        raise ValueError(
                            "private promotion source changed during verification"
                        )
                except (OSError, RuntimeError, ValueError) as error:
                    errors.append(
                        f"{directory.name}: private authenticated verifier source "
                        f"changed: {error}"
                    )
    return errors


errors = static_errors()
if mode == "promotion":
    errors.extend(promotion_errors())

if self_test:
    baseline = {
        MODEL: read(MODEL, []),
        CATALOG: read(CATALOG, []),
        CORE: read(CORE, []),
        WORKFLOW: read(WORKFLOW, []),
    }
    mutated = baseline[MODEL].replace(
        "KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4: u32 = 21",
        "KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4: u32 = 19",
    )
    if not static_errors({MODEL: mutated}):
        errors.append("self-test failed to reject ABI-19 substitution")
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
    verifier_override_name = "KAGEMUSHA_V4_RELEASE_" + "VERIFIER_BIN"
    readiness_source = (root / "ci/check_kagemusha_production_readiness.sh").read_text(
        encoding="utf-8"
    )
    if verifier_override_name in readiness_source:
        errors.append("self-test found a production release-verifier override hook")
    previous_verifier_override = os.environ.get(verifier_override_name)
    os.environ[verifier_override_name] = "/usr/bin/true"
    try:
        verifier_command = release_verifier_command(
            Path("/root/published-verifier/kagami"),
            Path("release"),
            Path("policy.norito"),
        )
    finally:
        if previous_verifier_override is None:
            del os.environ[verifier_override_name]
        else:
            os.environ[verifier_override_name] = previous_verifier_override
    if verifier_command[:3] != [
        "/root/published-verifier/kagami",
        "kagemusha",
        "verify-release-v4",
    ]:
        errors.append("self-test failed to reject a substituted release verifier")
    if (
        "candidate_builder._verify_exact_signed_commit(" not in readiness_source
        or "captured_commit," not in readiness_source
        or "_admit_production_closure_binding(" not in readiness_source
        or "require_root_controlled_bootstrap(" not in readiness_source
        or "published_build.admit_promotion_verifier(" not in readiness_source
        or "promotion_verifier.executable" not in readiness_source
        or "root_published_generated_candidate_receipt_sha256"
        not in readiness_source
        or "generation_worker_launch_receipt_sha256" not in readiness_source
        or "source_only_module(" not in readiness_source
        or "/usr/bin/env -i" not in readiness_source
        or '"${KAGEMUSHA_BUILD_PYTHON_EXECUTABLE}" -I -' not in readiness_source
        or "source_seal.configure_production_git(" not in readiness_source
        or ("tempfile." + "TemporaryDirectory") in readiness_source
        or (
            "cargo_" + "command = release_verifier_command"
        )
        in readiness_source
        or ("source_identity_" + "command") in readiness_source
        or (
            "from scripts import "
            + "kagemusha_source_tree_seal"
        )
        in readiness_source
    ):
        errors.append(
            "self-test found an unbound mutable promotion verifier source"
        )

if errors:
    print(f"Kagemusha ABI-21/V4 {mode} corridor failed:", file=sys.stderr)
    for error in errors:
        print(f" - {error}", file=sys.stderr)
    raise SystemExit(1)
print(f"Kagemusha ABI-21/V4 {mode} corridor passed.")
PY
}

if [[ "${MODE}" == "promotion" ]]; then
  required_promotion_variables=(
    KAGEMUSHA_BUILD_PYTHON_EXECUTABLE
    KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE
    KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256
    KAGEMUSHA_BUILD_TOOLCHAIN_PROVENANCE
    KAGEMUSHA_BUILD_TOOLCHAIN_PROVENANCE_SHA256
    KAGEMUSHA_PROMOTION_VERIFIER_RECEIPT
    KAGEMUSHA_PROMOTION_VERIFIER_RECEIPT_SHA256
    KAGEMUSHA_ROOT_PUBLISHED_GENERATED_CANDIDATE_RECEIPT_SHA256
    KAGEMUSHA_GENERATION_WORKER_LAUNCH_RECEIPT_SHA256
    KAGEMUSHA_V4_ARTIFACT_ROOT
    KAGEMUSHA_V4_RELEASE_POLICY_PATH
  )
  for variable_name in "${required_promotion_variables[@]}"; do
    if [[ -z "${!variable_name:-}" ]]; then
      echo "promotion requires ${variable_name}" >&2
      exit 2
    fi
  done
  run_readiness_python \
    /usr/bin/env -i \
    HOME=/var/empty \
    LANG=C \
    LC_ALL=C \
    PATH=/usr/bin:/bin \
    TMPDIR=/private/tmp \
    TZ=UTC \
    KAGEMUSHA_BUILD_PYTHON_EXECUTABLE="${KAGEMUSHA_BUILD_PYTHON_EXECUTABLE}" \
    KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE="${KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE}" \
    KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256="${KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256}" \
    KAGEMUSHA_BUILD_TOOLCHAIN_PROVENANCE="${KAGEMUSHA_BUILD_TOOLCHAIN_PROVENANCE}" \
    KAGEMUSHA_BUILD_TOOLCHAIN_PROVENANCE_SHA256="${KAGEMUSHA_BUILD_TOOLCHAIN_PROVENANCE_SHA256}" \
    KAGEMUSHA_PROMOTION_VERIFIER_RECEIPT="${KAGEMUSHA_PROMOTION_VERIFIER_RECEIPT}" \
    KAGEMUSHA_PROMOTION_VERIFIER_RECEIPT_SHA256="${KAGEMUSHA_PROMOTION_VERIFIER_RECEIPT_SHA256}" \
    KAGEMUSHA_ROOT_PUBLISHED_GENERATED_CANDIDATE_RECEIPT_SHA256="${KAGEMUSHA_ROOT_PUBLISHED_GENERATED_CANDIDATE_RECEIPT_SHA256}" \
    KAGEMUSHA_GENERATION_WORKER_LAUNCH_RECEIPT_SHA256="${KAGEMUSHA_GENERATION_WORKER_LAUNCH_RECEIPT_SHA256}" \
    KAGEMUSHA_V4_ARTIFACT_ROOT="${KAGEMUSHA_V4_ARTIFACT_ROOT}" \
    KAGEMUSHA_V4_RELEASE_POLICY_PATH="${KAGEMUSHA_V4_RELEASE_POLICY_PATH}" \
    "${KAGEMUSHA_BUILD_PYTHON_EXECUTABLE}" -I - \
    "${ROOT_DIR}" "${MODE}" "${SELF_TEST}"
else
  run_readiness_python \
    python3 -I - "${ROOT_DIR}" "${MODE}" "${SELF_TEST}"
fi
