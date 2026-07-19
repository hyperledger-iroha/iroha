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
BRIDGE = "crates/connect_norito_bridge/src/lib.rs"
HEADER = "crates/connect_norito_bridge/include/connect_norito_bridge.h"
CATALOG = "crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_terminal_registry_v4.rs"
CORE = "crates/iroha_core/src/smartcontracts/isi/offline.rs"
STEP_TRANSITION = "crates/iroha_core/src/zk/kagemusha_step_transition.rs"
RECURSIVE_BACKEND = "crates/iroha_core/src/zk/kagemusha_v2.rs"
VALUE_CONTRACT = "crates/iroha_data_model/tests/kagemusha_value_contract.rs"
SCHEMA_GOLDEN = "crates/iroha_data_model/tests/offline_public_schema_golden.rs"
CONFIG = "crates/iroha_config/src/parameters/user.rs"
NODE = "crates/irohad/src/main.rs"
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
            VALUE_CONTRACT,
            SCHEMA_GOLDEN,
            CONFIG,
            NODE,
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
        r"pub const KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE:\s*bool\s*=\s*(true|false)\s*;",
        model,
    )
    expected_availability = "true" if mode == "promotion" else "false"
    if availability is None or availability.group(1) != expected_availability:
        errors.append(
            f"{MODEL}: {mode} mode requires production availability "
            f"{expected_availability}"
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
        "installed.authenticated_verifier_artifacts()?",
        "installed.authenticated_prover_artifacts()?",
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
        "KagemushaPastaCycleVerifierArtifactsV4::new",
        "KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROMOTION_BYTES_V4",
    )
    require_pattern(
        texts[CATALOG],
        CATALOG,
        errors,
        (
            r"let\s+(?P<descriptors>[A-Za-z_][A-Za-z0-9_]*)\s*=\s*manifest\s*"
            r"\.profiles\s*\.iter\(\)\s*"
            r"\.flat_map\(\|profile\|\s*profile\.artifacts\.iter\(\)\)\s*"
            r"\.collect::<Vec<_>>\(\)\s*;\s*"
            r"if\s+(?P=descriptors)\.len\(\)\s*!=\s*8\s*\{"
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
        texts[CONFIG] + texts[NODE],
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

    value = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=object_pairs)
    if not isinstance(value, dict):
        raise ValueError("manifest JSON must be an object")
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
    if not policy.is_file() or policy.is_symlink() or policy.stat().st_size == 0:
        errors.append("promotion policy must be a nonempty regular file")
    if not artifact_root.is_dir() or artifact_root.is_symlink():
        errors.append("promotion artifact root must be a real directory")
        return errors

    status = subprocess.run(
        ["git", "status", "--porcelain=v1", "--untracked-files=all"],
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    if status.returncode != 0 or status.stdout:
        errors.append("promotion requires a clean source tree, including untracked files")
    signature = subprocess.run(
        ["git", "verify-commit", "HEAD"], cwd=root, check=False, capture_output=True
    )
    if signature.returncode != 0:
        errors.append("promotion requires a locally verifiable signature on HEAD")
    authenticated_verification_allowed = not errors

    directories = sorted(path for path in artifact_root.iterdir())
    if not directories:
        errors.append("promotion artifact root contains no manifest-digest releases")
        return errors
    expected_inventory = set(ARTIFACTS + FINAL_METADATA)
    placeholder = re.compile(rb"(?:placeholder|synthetic|dummy|todo|not[ -]?reviewed)", re.I)
    for directory in directories:
        directory_error_count = len(errors)
        if not directory.is_dir() or directory.is_symlink() or not re.fullmatch(r"[0-9a-f]{64}", directory.name):
            errors.append(f"noncanonical release entry: {directory.name}")
            continue
        actual = {path.name for path in directory.iterdir()}
        if actual != expected_inventory:
            errors.append(f"{directory.name}: final release inventory is not exact")
            continue
        manifest_bytes = (directory / "manifest.norito").read_bytes()
        digest = hashlib.sha256(manifest_bytes).hexdigest()
        if digest != directory.name:
            errors.append(f"{directory.name}: directory does not equal manifest SHA-256")
        if (directory / "manifest.norito.sha256").read_text(encoding="ascii") != f"{digest}\n":
            errors.append(f"{directory.name}: manifest digest sidecar is not canonical")
        try:
            manifest = strict_json(directory / "manifest.json")
        except (OSError, UnicodeError, ValueError, json.JSONDecodeError) as error:
            errors.append(f"{directory.name}: invalid manifest JSON: {error}")
            continue
        if manifest.get("schema") != "kagemusha.offline.recursive_spend.artifact_manifest.v4":
            errors.append(f"{directory.name}: manifest schema is not V4")
        if manifest.get("bridge_abi_version") != 21 or manifest.get("source_repo_dirty") is not False:
            errors.append(f"{directory.name}: ABI/source-tree promotion binding is invalid")
        profiles = manifest.get("profiles")
        roles = []
        if isinstance(profiles, list):
            for profile in profiles:
                if isinstance(profile, dict) and isinstance(profile.get("artifacts"), list):
                    roles.extend(profile["artifacts"])
        if len(roles) != 8:
            errors.append(f"{directory.name}: manifest does not bind exactly eight artifacts")
        for name in ARTIFACTS:
            payload = (directory / name).read_bytes()
            if len(payload) <= 16 or not payload.startswith(b"KRV4KEY\0"):
                errors.append(f"{directory.name}/{name}: invalid KRV4 framing")
        for name in (
            "release-attestation-v4.norito",
            "physical-device-benchmark.evidence",
            "cryptographic-review.evidence",
            "promotion-record-v4.norito",
        ):
            payload = (directory / name).read_bytes()
            if len(payload) < 64 or placeholder.search(payload):
                errors.append(f"{directory.name}/{name}: missing non-placeholder evidence bytes")
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
    flipped_availability = re.sub(
        r"(KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE:\s*bool\s*=\s*)"
        r"(?:true|false)",
        lambda match: match.group(1)
        + ("false" if mode == "promotion" else "true"),
        baseline[MODEL],
        count=1,
    )
    if not static_errors({MODEL: flipped_availability}):
        errors.append("self-test failed to reject an invalid availability state")
    seven_artifacts = baseline[CATALOG].replace(
        "descriptors.len() != 8",
        "descriptors.len() != 7",
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
