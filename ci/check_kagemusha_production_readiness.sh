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
        "production proof-log artifact",
        "single expected proof",
        "one-test",
        "cargo result",
        "canonical `lineage-proof-evidence.json` filename",
        "duplicate JSON object keys rejected",
        "Android device-lab scanner also rejects duplicate JSON",
        "reuse the scanner-validated signed-evidence timestamp",
        "telemetry/status/runtime completion markers",
        "symlink-free ancestors before its",
        "symlink-ancestor `--repo-root` aliases",
        "symlink-free key-path ancestors",
        "secret-looking key path strings",
        "reject symlinked rollup summary output ancestors",
        "leaves and ancestors before creating missing `--out` parents",
        "release artifact/proof-log inputs",
    ),
    "docs/source/offline_kagemusha.md": (
        "The reserved `kagemusha-recursive-spend-lineage-v1` profile is the enabled",
        "witnessless chain-admission path for constant-size lineage proofs inside the",
        "64-hop cap",
        "The routine offline-offline production path",
        "uses the ABI-6 reserved-lineage recursive spend verifier and redemption surface",
        "ABI-7 recursive compact-token symbols remain fail-closed until that proof",
        "uses the composed private-hop verifier-slice circuit",
        "--record-out artifacts/kagemusha/lineage-init-len128.record.norito",
        "--record-out artifacts/kagemusha/lineage-append-len128.record.norito",
        "iroha app zk kagemusha lineage-record",
        "--vk artifacts/kagemusha/lineage-init-len128.vk",
        "--vk artifacts/kagemusha/lineage-append-len128.vk",
        "governance/WSV `VerifyingKeyRecord` bound to `offline_kagemusha`",
        "`--record-namespace` and `--record-version`",
        "lineage-proof-evidence.json` to sit beside these",
        "canonical `lineage-proof-evidence.json` filename is part of the release packet",
        "renamed, copied, symlinked, or symlink-ancestor evidence JSON files",
        "recomputes each digest",
        "captured `record-archive-proof.log`",
        "re-checks that the local proof log",
        "single expected `test ... ok` line",
        "Marker-stuffed proof logs with extra passing tests",
        "recorded command is the production",
        "run exactly as the canonical command string",
        "no quoted-token aliases, newlines, or appended shell commands",
        "runtime lineage keygen unset",
        "`cargo test -p iroha_core",
        "kagemusha_recursive_spend_lineage_init_append_from_record_archives_proves_reserved_lineage_output",
        "tee artifacts/kagemusha/record-archive-proof.log",
        "python3 scripts/kagemusha_lineage_proof_evidence.py",
        "--proof-log artifacts/kagemusha/record-archive-proof.log",
        "extra release claims",
        "are",
        "rejected instead of ignored",
        "duplicate JSON object keys are also invalid",
        "last-key-wins evidence packets",
        "future-dated beyond the release validator",
        "clock-skew allowance, remains blocked",
        "timestamp must use canonical UTC",
        "helper rejects noncanonical `--generated-at-utc`",
        "normalizing them into",
        "symlink-ancestor output aliases",
        "checked-in ABI-6 manifest plus ABI-7",
        "symlink-free ancestors",
        "reading release artifact and proof-log inputs",
        "recorded proof",
        "commands with secret-looking material",
        "echoing the secret value",
        "Android freshness checks consume the",
        "scanner-validated signed-evidence timestamp",
        "readiness summary writer also rejects symlinked `--summary-out` ancestors",
        "shared Android device-lab JSON loader",
        "aliased directories",
        "symlinked `--repo-root` directories",
        "repo-root ancestors",
    ),
    "docs/source/sdk/android/readiness/android_strongbox_device_matrix.md": (
        "Android StrongBox Offline Payments Device Matrix",
        "Last updated: 2026-06-07",
        "ABI 6 recursive spend JNI probes pass on every required device family.",
        "ABI 7 recursive compact-token JNI probes fail closed with the unavailable",
        "ABI 7 recursive compact prover calls that reach the proof-composition",
        "reservation are reported as unavailable state, while empty or malformed local",
        "archives remain caller-input errors.",
        "Lab reports include raw test commands, device fingerprints, OS build IDs, and",
        "connectedAndroidTest",
        "org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProverTest",
        "OfflineNoteTransferHandoff",
        "python3 scripts/check_android_device_lab_slot.py --root artifacts/android/device_lab --require-slot --require-kagemusha-production-evidence --require-kagemusha-standard-matrix --trusted-signer-public-key",
        "single safe slot",
        "python3 scripts/sign_android_device_lab_evidence.py --slot artifacts/android/device_lab/<slot-id> --private-key",
        "python3 scripts/kagemusha_production_readiness.py --device-lab-root artifacts/android/device_lab --trusted-signer-public-key",
        "Reserved-lineage proof evidence",
        "canonical",
        "`lineage-proof-evidence.json` filename",
        "renamed or copied evidence",
        "The rollup recomputes their SHA-256 digests from",
        "local bytes",
        "non-symlink",
        "non-hardlinked files",
        "symlink-free ancestors",
        "symlinked output ancestors plus symlinked, hardlinked, or non-regular",
        "record-archive-proof.log",
        "re-checks the proof log's passing cargo",
        "one expected `test ... ok` line",
        "Marker-stuffed proof logs with extra passing tests",
        "Duplicate JSON",
        "last-key-wins parser behavior",
        "device-lab scanner applies the same rule",
        "D2D handoff",
        "wallet-integrity transcripts",
        "Telemetry",
        "status NDJSON must include an `ok` status",
        "`logs/runtime.log` must carry the Kagemusha device-lab",
        "artifact files must be ordinary",
        "directories or regular files",
        "must not",
        "hardlinks",
        "operator-supplied root ancestors",
        "slot path ancestors",
        "special-file slot",
        "linked or",
        "shared device-lab JSON",
        "cannot read through aliased directories",
        "exact production `cargo test -p iroha_core` command",
        "appended shell commands",
        "--max-lineage-proof-evidence-future-skew-seconds 300",
        "future-dated",
        "beyond the validator clock-skew allowance is also blocked",
        "`generated_at_utc` must use canonical UTC",
        "`generated_at_utc` must use",
        "`YYYY-MM-DDTHH:MM:SSZ` form",
        "lineage evidence helper rejects",
        "instead of normalizing it",
        "output ancestors before creating missing `--out` parent directories",
        "reading release artifact and proof-log inputs",
        "`--repo-root` must also be an existing non-symlink directory",
        "--max-signed-at-future-skew-seconds 300",
        "slot.json",
        "family-specific minimum OS",
        "app package name",
        "app signing certificate",
        "attestation challenge",
        "attestation certificate chain path and SHA-256",
        "physical device attestation",
        "offline wallet policy",
        "release APK path and SHA-256",
        "D2D payment transcript path and SHA-256",
        "that path must stay under `handoff/`",
        "wallet integrity transcript path and SHA-256",
        "native bridge ABI version",
        ":client-android:assembleRelease",
        ":offline-wallet-android:assembleRelease",
        "attestation/result.json",
        "StrongBox/KeyMint security level",
        "closed schema",
        "signed evidence artifact schema",
        "signed evidence artifact path",
        "trusted signer public key",
        "signer_public_key_sha256",
        "signature_payload_sha256",
        "Runtime private-key",
        "symlink-free ancestors",
        "Secret-looking key path strings are rejected before OpenSSL",
        "YYYY-MM-DDTHH:MM:SSZ",
        "artifact digests for the required telemetry, attestation, queue, log, wallet integrity, and D2D handoff files",
        "d2d payment transcript",
        "SHA-256 of the trusted public key DER",
        "The hash must match the referenced artifact bytes",
        "future-dated beyond the release validator clock-skew allowance",
        "Freshness checks",
        "scanner-validated signed-evidence timestamp",
        "The signed evidence artifact path must be the canonical",
        "`evidence/signed-evidence.json` path",
        "renamed or copied signed evidence",
        "summary output path",
    ),
    "scripts/check_android_device_lab_slot.py": (
        "KAGEMUSHA_STANDARD_DEVICE_FAMILIES",
        "KAGEMUSHA_STANDARD_DEVICE_MINIMUM_OS",
        "DEVICE_LAB_ROOT_SUMMARY_LABEL",
        "\"root\": DEVICE_LAB_ROOT_SUMMARY_LABEL",
        "RAW_TEST_COMMAND_REQUIRED_MARKERS",
        "KAGEMUSHA_ANDROID_PRODUCTION_RAW_TEST_COMMAND",
        "KAGEMUSHA_ANDROID_PRODUCTION_RAW_TEST_COMMANDS",
        "must exactly match the Kagemusha Android production raw test command",
        "SIGNED_EVIDENCE_SCHEMA",
        "D2D_PAYMENT_TRANSCRIPT_SCHEMA",
        "D2D_PAYMENT_PAYLOAD_SCHEMA",
        "WALLET_INTEGRITY_TRANSCRIPT_SCHEMA",
        "REQUIRED_KAGEMUSHA_SLOT_ARTIFACT_PATHS",
        "KAGEMUSHA_SIGNED_EVIDENCE_ARTIFACT_PATH",
        "telemetry/status.ndjson",
        "logs/runtime.log",
        "MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES",
        "validate_required_kagemusha_slot_artifact_shapes",
        "KAGEMUSHA_RUNTIME_LOG_COMPLETE_MARKER",
        "KAGEMUSHA_RUNTIME_LOG_FAILURE_MARKERS",
        "KAGEMUSHA_STATUS_FAILURE_VALUES",
        "_validate_required_telemetry_artifact",
        "_validate_required_status_artifact",
        "_validate_required_runtime_log_artifact",
        "kagemusha device-lab run complete",
        "telemetry/status.ndjson must contain at least one ok status",
        "logs/runtime.log must contain Kagemusha device-lab completion marker",
        "artifact_size == 0",
        "must be non-empty",
        "must be no more than",
        "MAX_D2D_PAYMENT_PAYLOAD_BYTES",
        "ATTESTATION_CERTIFICATE_CHAIN_SUFFIXES",
        "MAX_ATTESTATION_CERTIFICATE_CHAIN_BYTES",
        "transport_session_id_sha256",
        "payer_wallet_state_before_sha256",
        "payer_wallet_state_after_sha256",
        "payee_wallet_state_before_sha256",
        "payee_wallet_state_after_sha256",
        "device_fingerprint_sha256",
        "physical_device_attestation",
        "in set(EXPECTED_DIRS) | {\"handoff\", \"wallet\"}",
        "SIGNED_AT_UTC_RE",
        "SIGNED_EVIDENCE_SIGNATURE_ALGORITHMS",
        "REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION",
        "SIGNED_EVIDENCE_SLOT_STRING_FIELDS: tuple[str, ...]",
        "SIGNED_EVIDENCE_SLOT_SHA256_FIELDS: tuple[str, ...]",
        "SIGNED_EVIDENCE_SLOT_INT_FIELDS: tuple[str, ...]",
        "SIGNED_EVIDENCE_SLOT_TRUE_FIELDS: tuple[str, ...]",
        "SLOT_METADATA_FIELDS",
        "ATTESTATION_RESULT_FIELDS",
        "set(result) - ATTESTATION_RESULT_FIELDS",
        "SECRET_PATH_REDACTION",
        "unsafe path contains secret-looking material",
        "--require-kagemusha-production-evidence",
        "--require-kagemusha-standard-matrix",
        "--trusted-signer-public-key",
        "--root must not contain secret-looking material",
        "--json-out must not contain secret-looking material",
        "root does not exist",
        "no slots found under root",
        "load_trusted_signer_public_keys",
        "validate_slot_metadata_fields",
        "validate_attestation_result",
        "validate_d2d_payment_transcript",
        "validate_d2d_payment_transcript_binding",
        "validate_wallet_integrity_transcript",
        "validate_wallet_integrity_transcript_binding",
        "validate_slot_ids",
        "validate_device_lab_root_path",
        "validate_no_symlink_ancestors",
        "slot_ids, slot_id_errors = validate_slot_ids(args.slots)",
        "slot id {_display_path(slot_id)!r} must be a single safe directory name",
        'if SECRET_RE.search(str(root)):\n        return ["device-lab root path must not contain secret-looking material"]',
        "DuplicateJsonKeyError",
        "_reject_duplicate_json_object_pairs",
        "object_pairs_hook=_reject_duplicate_json_object_pairs",
        "contains duplicate JSON object key",
        "validate_no_slot_symlink_artifacts",
        'def validate_no_slot_symlink_artifacts(slot_path: Path, errors: list[str]) -> None:\n    """Reject symlinked slot metadata, directories, and evidence artifacts."""\n\n    if _reject_secret_slot_path(slot_path, errors):\n        return\n',
        "validate_slot_regular_file_artifacts",
        'def validate_slot_regular_file_artifacts(slot_path: Path, errors: list[str]) -> None:\n    """Reject special-file slot metadata and evidence artifacts."""\n\n    if _reject_secret_slot_path(slot_path, errors):\n        return\n',
        "validate_no_slot_hardlink_artifacts",
        'def validate_no_slot_hardlink_artifacts(slot_path: Path, errors: list[str]) -> None:\n    """Reject hardlinked slot metadata and evidence artifacts."""\n\n    if _reject_secret_slot_path(slot_path, errors):\n        return\n',
        "slot directory name must not contain secret-looking material",
        'def _reject_secret_slot_path(slot_path: Path, errors: list[str]) -> bool:\n    """Reject direct helper calls that receive secret-looking slot paths."""\n\n    if SECRET_RE.search(str(slot_path)):\n        errors.append("slot path must not contain secret-looking material")\n        return True\n    return False\n',
        "_reject_secret_slot_path(slot_path, errors)",
        'def _validate_manifest_slot_path(slot_path: Path) -> list[str]:\n    if SECRET_RE.search(str(slot_path)):\n        return ["slot path must not contain secret-looking material"]\n    if slot_path.is_symlink():\n        return ["slot directory must not be a symlink"]\n    return validate_no_symlink_ancestors(slot_path, "slot ancestor directory")\n',
        'root_errors = _validate_manifest_slot_path(slot_path)\n    if root_errors:\n        return entries, root_errors\n',
        'root_errors = _validate_manifest_slot_path(slot_path)\n    if root_errors:\n        return root_errors\n',
        'def _slot_files(slot_path: Path) -> set[str]:\n    if slot_path.is_symlink() or not slot_path.is_dir():\n        return set()\n',
        'if SECRET_RE.search(str(slot_path)):\n        return set()\n',
        'if validate_no_symlink_ancestors(slot_path, "slot ancestor directory"):\n        return set()\n',
        "if dir_path.is_symlink() or not dir_path.is_dir():",
        'skipped_roots = {"sha256sum.txt", *EXPECTED_DIRS, *OPTIONAL_EVIDENCE_DIRS}',
        "def _slot_relative_symlink_ancestor(slot_path: Path, relative: str) -> str | None:",
        "sha256sum.txt references artifact under symlink directory",
        "sha256sum.txt references symlink artifact",
        "sha256sum.txt references non-regular artifact",
        "sha256sum.txt references hardlinked artifact",
        "device-lab root must not be a symlink",
        "device-lab root ancestor directory",
        "slot directory must not be a symlink",
        "slot parent directory must not be a symlink",
        "slot ancestor directory",
        "must not be a symlink",
        "must be a regular file",
        "must not be hardlinked",
        "slot.json contains unexpected field",
        "_verify_ed25519_signature",
        "covered_device_families",
        "missing_device_families",
        "trusted_signer_public_key_sha256",
        "sha256sum.txt digest mismatch",
        '"abi7_recursive_compact_jni_probe"',
        "slot.json minimum_os",
        "app_signing_certificate_sha256",
        "attestation_challenge_sha256",
        "attestation_certificate_chain_path",
        "attestation_certificate_chain_sha256",
        "offline_wallet_policy_sha256",
        "offline_wallet_apk_path",
        "offline_wallet_apk_sha256",
        "d2d_payment_transcript_path",
        "d2d_payment_transcript_sha256",
        "wallet_integrity_transcript_path",
        "wallet_integrity_transcript_sha256",
        "native_bridge_abi_version",
        "slot.json offline_wallet_apk_sha256 does not match offline_wallet_apk_path",
        "slot.json attestation_certificate_chain_sha256 does not match attestation_certificate_chain_path",
        "slot.json attestation_certificate_chain_path must end in .pem or .der",
        "attestation certificate chain PEM must contain certificate boundaries",
        "attestation certificate chain must be no more than",
        "slot.json d2d_payment_transcript_sha256 does not match d2d_payment_transcript_path",
        "slot.json d2d_payment_transcript_path must stay under handoff/",
        "slot.json wallet_integrity_transcript_sha256 does not match wallet_integrity_transcript_path",
        "wallet integrity transcript key_id_before_sha256 must differ from key_id_after_sha256",
        "wallet integrity transcript stale_snapshot_rejected must be true",
        "d2d payment transcript contains unexpected field",
        "d2d payment transcript {before_key} must differ from {after_key}",
        "d2d payment transcript queue_after_sha256 must match queue/pending_queue.json",
        "d2d payment transcript payload_bytes must be no more than",
        "slot.json native_bridge_abi_version must be",
        "attestation/result.json must report STRONGBOX security level",
        "attestation/result.json contains unexpected field",
        "attestation/result.json physical_device_attestation must be true",
        "attestation/result.json {key} must be lowercase sha256 hex",
        "attestation/result.json {key} must match slot.json {key}",
        "attestation/result.json {slot_key} must match the slot directory name",
        "attestation/result.json slot and slot_id must match",
        "signed_evidence_artifact_path",
        "slot.json signed_evidence_artifact_path must stay under evidence/",
        "slot.json signed_evidence_artifact_path must be",
        "signed_evidence_artifact_sha256 does not match signed_evidence_artifact_path",
        "signed evidence artifact contains unexpected field {_display_path(field)}",
        "signer_public_key_sha256",
        "signature_payload_sha256",
        "trusted signer public key required for Kagemusha production evidence",
        "signed evidence artifact signature verification failed",
        "signed evidence artifact digest mismatch for",
        "signed evidence artifact artifact_digests",
        "signed evidence artifact required slot artifact is missing",
        "f\"[{_display_path(relative)}] must be lowercase sha256 hex\"",
        "signed evidence artifact raw_test_commands must match slot.json raw_test_commands",
        "_validate_raw_test_command_markers",
        "must include {marker}",
        ":client-android:assembleRelease",
        ":offline-wallet-android:assembleRelease",
        "KagemushaRecursiveSpendProverTest",
        "connectedAndroidTest",
        "signed evidence artifact signed_at_utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
        "_require_evidence_raw_string",
        '_require_evidence_raw_string(evidence, "signed_at_utc", errors)',
        "slot.json raw_test_commands",
        "public_key_path.is_symlink()",
        "_validate_public_key_path_shape",
        'label: str = "trusted signer public key"',
        'f"{label} ancestor directory"',
        "slot directory missing",
        "validate_summary_output_path",
        "write_errors = write_summary",
        'if SECRET_RE.search(str(path)):\n        return [f"{label} must not contain secret-looking material"]',
        'return [f"{label} must not be a symlink"]',
        'return [f"{label} must not be hardlinked"]',
        'def _load_json(path: Path, label: str, errors: list[str]) -> dict[str, Any] | None:\n    if SECRET_RE.search(str(path)):\n        errors.append(f"{label} path must not contain secret-looking material")\n        return None\n',
        "json_ancestor_errors = validate_no_symlink_ancestors(",
        'f"{label} ancestor directory"',
    ),
    "scripts/sign_android_device_lab_evidence.py": (
        "Build and sign Kagemusha Android device-lab evidence artifacts",
        "DEFAULT_SIGNED_EVIDENCE_PATH",
        "device_lab.KAGEMUSHA_SIGNED_EVIDENCE_ARTIFACT_PATH",
        "device_lab._load_json",
        "device_lab._canonical_signed_evidence_payload",
        "device_lab.SIGNED_EVIDENCE_SLOT_INT_FIELDS",
        "slot.json native_bridge_abi_version must be an integer",
        "private key did not produce a signature accepted by the signer public key",
        "_secret_key_path_error",
        "if device_lab.SECRET_RE.search(str(path)):",
        "private_key_path.is_symlink()",
        "path must not contain secret-looking material",
        "slot path must not contain secret-looking material",
        "private key must not be a symlink",
        "private key ancestor directory",
        "private key must not be hardlinked",
        "signed evidence output path must not contain secret-looking material",
        "signer key id must be non-empty and must not contain secret-looking material",
        "signed evidence output path must stay under evidence/",
        "signed evidence output path must be",
        "_validate_json_output_path",
        'def _validate_json_output_path(path: Path, label: str) -> list[str]:\n    """Validate a signer-controlled output immediately before writing."""\n\n    if device_lab.SECRET_RE.search(str(path)):\n        return [f"{label} must not contain secret-looking material"]\n',
        "_write_json(output_path, evidence, \"signed evidence output path\")",
        "_write_text",
        "_write_text(slot_path / \"sha256sum.txt\"",
        "_preflight_slot_metadata_reads",
        "Validate slot paths before any signer-controlled metadata is parsed",
        "errors = _preflight_slot_metadata_reads(slot_path)",
        'f"{label} parent directory must not be a symlink"',
        'f"{label} ancestor directory"',
        'f"{label} must not be hardlinked"',
        "slot artifacts must not contain secret-looking material",
        "slot artifact {relative} is missing",
        "validate_no_slot_symlink_artifacts",
        "validate_slot_regular_file_artifacts",
        "validate_no_slot_hardlink_artifacts",
        'def _preflight_slot_metadata_reads(slot_path: Path) -> list[str]:\n    """Validate slot paths before any signer-controlled metadata is parsed."""\n\n    if device_lab.SECRET_RE.search(str(slot_path)):\n        return ["slot path must not contain secret-looking material"]\n',
        "slot directory must not be a symlink",
        "validate_no_symlink_ancestors",
        "slot ancestor directory",
        "validate_required_kagemusha_slot_artifact_shapes",
        "_validate_slot_for_manifest_rewrite",
        'def _validate_slot_for_manifest_rewrite(slot_path: Path) -> list[str]:\n    """Validate a slot immediately before rewriting its SHA-256 manifest."""\n\n    if device_lab.SECRET_RE.search(str(slot_path)):\n        return ["slot path must not contain secret-looking material"]\n',
        "errors = _validate_slot_for_manifest_rewrite(slot_path)",
        "for relative in device_lab._slot_files(slot_path):",
        "rewrite_sha256_manifest",
        "validate_slot_metadata_fields",
        "validate_attestation_result",
        "validate_d2d_payment_transcript_binding",
        "validate_wallet_integrity_transcript_binding",
        "validate_kagemusha_production_metadata",
        "--no-update-slot-json",
        "--no-update-sha256sum",
    ),
    "scripts/kagemusha_production_readiness.py": (
        "Roll up Kagemusha production-readiness evidence into a strict summary",
        "SUMMARY_SCHEMA = \"iroha.kagemusha.production_readiness.v1\"",
        "LINEAGE_PROOF_EVIDENCE_SCHEMA = \"iroha.kagemusha.lineage_proof_evidence.v1\"",
        "LINEAGE_PROOF_EVIDENCE_FILENAME = \"lineage-proof-evidence.json\"",
        "DEFAULT_LINEAGE_PROOF_EVIDENCE_PATH",
        "DEFAULT_MIN_SIGNED_AT_UTC = \"2026-06-06T00:00:00Z\"",
        "DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS = 300",
        "ANDROID_DEVICE_LAB_ROOT_SUMMARY_LABEL",
        "LINEAGE_PROOF_EVIDENCE_SUMMARY_LABEL",
        "LINEAGE_PROOF_EVIDENCE_FIELDS",
        "LINEAGE_PROOF_TEST_FIELDS",
        "\"root\": ANDROID_DEVICE_LAB_ROOT_SUMMARY_LABEL",
        "ABI6_OPERATION_SYMBOLS",
        "check_abi6_reserved_lineage",
        "validate_release_local_json_file",
        'def validate_release_local_json_file(path: Path, label: str) -> list[str]:\n    """Reject local release JSON files that could alias external bytes."""\n\n    if device_lab.SECRET_RE.search(str(path)):\n        return [f"{label} path must not contain secret-looking material"]\n',
        "release_json_ancestor_errors = device_lab.validate_no_symlink_ancestors(",
        "validate_repo_source_marker_file",
        'def validate_repo_source_marker_file(path: Path, label: str) -> list[str]:\n    """Reject checked-in marker files that could alias external bytes."""\n\n    if device_lab.SECRET_RE.search(str(path)):\n        return [f"{label} path must not contain secret-looking material"]\n',
        "abi6_manifest_file_shape",
        "check_abi7_fail_closed",
        "abi7_source_marker_file_shape",
        "LINEAGE_KEY_RELEASE_TOOLING_REQUIREMENTS",
        "check_lineage_key_release_tooling",
        "lineage_key_release_file_shape",
        "lineage_key_release_marker_missing",
        '"lineage_key_release_tooling": lineage',
        "LINEAGE_PROOF_REQUIRED_ARTIFACTS",
        "LINEAGE_PROOF_REQUIRED_TESTS",
        "LINEAGE_PROOF_REQUIRED_TEST_LOGS",
        "EXPECTED_LINEAGE_PROOF_RESULT_PREFIX",
        "MAX_LINEAGE_PROOF_LOG_BYTES",
        "shlex.split(command)",
        "expected_tokens = (",
        "expected_lineage_proof_command",
        "validate_lineage_proof_log",
        "validate_lineage_proof_command",
        "DuplicateJsonKeyError",
        "_reject_duplicate_json_object_pairs",
        "object_pairs_hook=_reject_duplicate_json_object_pairs",
        "contains duplicate JSON object key",
        "details[\"generated_at_utc\"]",
        "details[\"artifact_sha256\"]",
        "details[\"test_log_sha256\"]",
        "max_generated_at_utc",
        "check_lineage_proof_evidence",
        "lineage_proof_evidence_filename",
        "Reserved-lineage proof evidence file must be named",
        "require_canonical_filename: bool = True",
        "lineage_proof_evidence_missing",
        "lineage_proof_evidence_file_shape",
        "Reserved-lineage proof evidence file",
        'report.get("kagemusha", {}).get("signed_at_utc")',
        "validated Android device-lab report is missing signed evidence timestamp",
        "lineage_proof_evidence_stale",
        "lineage_proof_evidence_future_dated",
        "lineage_proof_evidence_timestamp_noncanonical",
        "generated_at_utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
        "generated_at_raw = generated_at_text",
        "device_lab.SIGNED_AT_UTC_RE.fullmatch(generated_at_raw)",
        "not isinstance(scalar_value, int)",
        "isinstance(scalar_value, bool)",
        "must be integer",
        "lineage_proof_evidence_artifact_missing",
        "validate_lineage_local_file",
        'device_lab.SECRET_RE.search(str(path))',
        "path must not contain secret-looking material",
        "ancestor_errors = device_lab.validate_no_symlink_ancestors(",
        '    ancestor_errors = device_lab.validate_no_symlink_ancestors(\n        path,\n        f"{label} ancestor directory",\n    )\n    if ancestor_errors:\n        return ancestor_errors\n',
        'f"{label} ancestor directory"',
        "lineage_proof_evidence_artifact_file_shape",
        "lineage_proof_evidence_artifact_file_digest",
        "lineage_proof_evidence_test_log_path",
        "lineage_proof_evidence_test_log_missing",
        "lineage_proof_evidence_test_log_unreadable",
        "lineage_proof_evidence_test_log_file_digest",
        "lineage_proof_evidence_test_log_content",
        "test_lines != [expected_test_line]",
        "not line.startswith(\"test result:\")",
        "result_lines = [line.rstrip() for line in lines if line.startswith(\"test result:\")]",
        "must contain only the single production proof test line",
        "must contain exactly one cargo test result for one passed production test",
        '"cargo"',
        '"test"',
        '"iroha_core"',
        "must exactly match the production Reserved-lineage proof command",
        "must exactly match the canonical production Reserved-lineage proof command string",
        "command != expected_command",
        "must not set runtime lineage keygen",
        "must not contain secret-looking material",
        "isinstance(elapsed_seconds, bool)",
        "math.isfinite(float(elapsed_seconds))",
        "lineage_proof_evidence_unexpected_field",
        "lineage_proof_evidence_circuit_ids_unexpected_field",
        "lineage_proof_evidence_artifacts_unexpected_field",
        "lineage_proof_evidence_tests_unexpected_field",
        "lineage_proof_evidence_test_unexpected_field",
        "Reserved-lineage proof evidence artifact digest does not match local artifact bytes",
        "log digest does not match local log bytes",
        "record_archive_proof_runtime_keygen_env",
        '"lineage_proof_evidence": lineage_proof',
        "--lineage-proof-evidence",
        "--min-lineage-proof-evidence-at-utc",
        "--max-lineage-proof-evidence-future-skew-seconds",
        "max_lineage_proof_evidence_at",
        "lineage_proof_evidence_max_timestamp_invalid",
        "check_android_device_lab",
        "_check_android_matrix_unique_bindings",
        "android_device_lab_duplicate_device_fingerprint",
        "android_device_lab_duplicate_attestation_challenge",
        "Android device-lab production slots must not reuse a device fingerprint",
        "value_sha256",
        "_redact_secret_strings",
        "_sanitize_android_reports",
        "android_device_lab_report_secret_material",
        "reports, report_secret_blockers = _sanitize_android_reports(",
        "validate_cli_path_arguments",
        "path_blockers = validate_cli_path_arguments(args)",
        "validate_repo_root_path",
        'secret_blocker = _secret_looking_path_blocker(\n        str(root),\n        label="--repo-root",\n        code="kagemusha_repo_root_path_invalid",\n    )\n    if secret_blocker is not None:\n        return [secret_blocker]\n',
        "repo_root_blockers = validate_repo_root_path(repo_root)",
        "repo_root_errors = validate_repo_root_path(Path(args.repo_root))",
        "--repo-root must not be a symlink",
        "--repo-root ancestor directory",
        "SUMMARY_OUT_PATH_INVALID_CODE",
        "must not contain secret-looking material",
        "android_device_lab_root_path_invalid",
        "android_device_lab_root_invalid",
        "android_trusted_signer_path_invalid",
        "android_device_lab_standard_matrix_missing",
        "android_device_lab_slot_id_invalid",
        "android_signed_evidence_stale",
        "android_signed_evidence_future_dated",
        "--min-signed-at-utc",
        "--max-signed-at-future-skew-seconds",
        "min_signed_at_utc",
        "max_signed_at_utc",
        "validate_summary_output_path",
        'secret_blocker = _secret_looking_path_blocker(\n        str(path),\n        label="--summary-out",\n        code=SUMMARY_OUT_PATH_INVALID_CODE,\n    )\n    if secret_blocker is not None:\n        return [secret_blocker]\n',
        "write_blockers = write_summary",
        "--summary-out must not be a symlink",
        "--summary-out must not be hardlinked",
        "--summary-out ancestor directory",
        "trusted_signer_public_key_sha256",
        '"signed_evidence": _android_signed_evidence_summary(reports)',
        "_android_signed_evidence_summary",
        "lineage_proof_evidence_path=lineage_proof_evidence_path,",
        'print("[kagemusha-readiness] wrote summary")',
        "ready\" if not all_blockers else \"blocked",
    ),
    "scripts/kagemusha_lineage_proof_evidence.py": (
        "Build Reserved-lineage production proof evidence JSON for Kagemusha",
        "DEFAULT_RECORD_ARCHIVE_PROOF_COMMAND",
        "LINEAGE_PROOF_REQUIRED_ARTIFACTS",
        "LINEAGE_PROOF_REQUIRED_TESTS",
        "validate_lineage_proof_command",
        "validate_lineage_local_file",
        "_validate_generated_at_utc",
        "errors.extend(_validate_generated_at_utc(generated_at_utc))",
        "--generated-at-utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
        "--artifact-dir",
        "--proof-log",
        "--elapsed-seconds",
        "--generated-at-utc",
        "validate_lineage_proof_log",
        "--out must be named",
        "--out must be written directly under --artifact-dir",
        "--proof-log must be written directly under --artifact-dir",
        "validate_evidence_document",
        "check_lineage_proof_evidence",
        "require_canonical_filename=False",
        'secret_error = _secret_path_error(str(artifact_dir), "--artifact-dir")',
        'secret_error = _secret_path_error(str(path), label)',
        "validate_artifact_dir_path",
        'def validate_artifact_dir_path(artifact_dir: Path) -> list[str]:\n    """Reject artifact directories that could alias external release bytes."""\n\n    secret_error = _secret_path_error(str(artifact_dir), "--artifact-dir")\n    if secret_error is not None:\n        return [secret_error]\n',
        "validate_lineage_input_paths",
        'proof_log_secret_error = _secret_path_error(str(proof_log), "--proof-log")\n    if proof_log_secret_error is not None:\n        errors.append(proof_log_secret_error)\n    if errors:\n        return errors\n',
        "errors = validate_lineage_input_paths(artifact_dir, proof_log)",
        "path_errors.extend(validate_lineage_input_paths(artifact_dir, proof_log))",
        "preflight_output_path",
        'def preflight_output_path(path: Path, label: str) -> list[str]:\n    """Reject aliased output paths before evidence inputs are read."""\n\n    secret_error = _secret_path_error(str(path), label)\n    if secret_error is not None:\n        return [secret_error]\n',
        "validate_output_path",
        'early_output_errors = preflight_output_path(out_path, "--out")',
        "--artifact-dir must not be a symlink",
        "output_ancestor_errors = device_lab.validate_no_symlink_ancestors(",
        'f"{label} ancestor directory"',
        "write_errors = write_evidence(out_path, evidence)",
        "missing lineage artifact",
        "wrote evidence",
        "pre_create_dir_errors = validate_artifact_dir_path(artifact_dir)",
    ),
    "scripts/tests/check_android_device_lab_slot_test.py": (
        "test_checked_in_sample_slot_passes_default_validation",
        "test_scan_slot_rejects_sha256_drift",
        "test_explicit_missing_slot_returns_structured_error",
        "test_explicit_unsafe_slot_id_rejected_before_path_join",
        "test_explicit_secret_looking_slot_id_is_not_echoed",
        "test_discovered_secret_looking_slot_directory_is_not_echoed",
        "test_scan_slot_redacts_secret_looking_manifest_paths",
        "test_slot_files_missing_slot_returns_empty_without_traceback",
        "test_slot_files_non_directory_root_returns_empty_without_traceback",
        "test_slot_files_secret_slot_path_returns_empty_without_traversal",
        "test_slot_files_rejects_symlinked_slot_root_directly_without_traversal",
        "test_slot_files_rejects_symlinked_slot_ancestor_directly_without_traversal",
        "test_slot_files_skips_symlinked_artifact_directory_directly_without_traversal",
        "test_parse_sha256_manifest_rejects_secret_slot_path_directly_before_parse",
        "test_parse_sha256_manifest_rejects_symlinked_slot_root_directly_before_parse",
        "test_parse_sha256_manifest_rejects_symlinked_slot_ancestor_before_parse",
        "test_verify_sha256_manifest_rejects_secret_slot_path_directly_before_traversal",
        "test_verify_sha256_manifest_rejects_symlinked_slot_root_directly_before_parse",
        "test_verify_sha256_manifest_rejects_symlinked_slot_ancestor_before_discovery",
        "test_verify_sha256_manifest_missing_slot_returns_missing_manifest_without_traceback",
        "test_verify_sha256_manifest_rejects_symlinked_artifact_directory_before_digest_read",
        "test_attestation_result_rejects_secret_slot_path_directly_before_parse",
        "test_d2d_transcript_rejects_secret_slot_path_directly_before_parse",
        "test_d2d_transcript_binding_rejects_secret_slot_path_directly_before_artifact_read",
        "test_wallet_transcript_binding_rejects_secret_slot_path_directly_before_artifact_read",
        "test_required_artifact_shapes_rejects_secret_slot_path_directly_before_stat",
        "test_slot_symlink_artifact_validator_rejects_secret_slot_path_directly_before_traversal",
        "test_slot_hardlink_artifact_validator_rejects_secret_slot_path_directly_before_stat",
        "test_slot_regular_artifact_validator_rejects_secret_slot_path_directly_before_shape",
        "test_signed_evidence_artifact_rejects_secret_slot_path_directly_before_parse",
        "test_kagemusha_production_metadata_rejects_secret_slot_path_directly_before_parse",
        "test_json_summary_redacts_secret_looking_unlisted_artifact_paths",
        "test_production_json_summary_redacts_secret_looking_required_artifact_paths",
        "test_production_metadata_rejects_duplicate_slot_json_key",
        "test_production_metadata_redacts_secret_duplicate_json_key",
        "test_production_metadata_rejects_duplicate_attestation_json_key",
        "test_production_metadata_rejects_duplicate_signed_evidence_json_key",
        "test_production_metadata_rejects_duplicate_d2d_transcript_json_key",
        "test_production_metadata_rejects_duplicate_wallet_integrity_json_key",
        "test_production_metadata_rejects_available_recursive_compact_probe",
        "test_production_metadata_rejects_signed_evidence_digest_drift",
        "test_production_metadata_rejects_unsafe_signed_evidence_path",
        "test_production_metadata_rejects_signed_evidence_artifact_outside_evidence",
        "test_production_metadata_rejects_noncanonical_signed_evidence_filename",
        "test_scan_slot_rejects_symlinked_slot_directory",
        "test_main_rejects_symlinked_device_lab_root_before_discovery",
        "test_main_rejects_symlinked_device_lab_root_ancestor_before_discovery",
        "test_scan_slot_rejects_symlinked_slot_parent_directory",
        "test_scan_slot_rejects_symlinked_slot_ancestor_directory",
        "test_load_json_rejects_symlinked_ancestor_before_read",
        "test_load_json_rejects_secret_path_directly_before_parse",
        "test_scan_slot_rejects_symlinked_required_artifact",
        "test_scan_slot_rejects_hardlinked_required_artifact",
        "test_scan_slot_rejects_non_regular_required_artifact",
        "test_production_metadata_rejects_symlinked_signed_evidence_artifact",
        "test_production_metadata_rejects_hardlinked_signed_evidence_artifact",
        "test_production_metadata_rejects_non_regular_signed_evidence_artifact",
        "test_production_metadata_rejects_unexpected_slot_fields_with_redaction",
        "test_production_metadata_rejects_unexpected_attestation_fields_with_redaction",
        "test_production_metadata_rejects_noncanonical_attestation_sha",
        "test_production_metadata_rejects_virtual_device_attestation",
        "test_production_metadata_rejects_missing_attestation_chain_binding",
        "test_production_metadata_rejects_attestation_chain_digest_drift",
        "test_production_metadata_rejects_attestation_chain_summary_file_substitution",
        "test_production_metadata_rejects_malformed_attestation_chain_pem",
        "test_production_metadata_rejects_oversized_attestation_chain",
        "test_production_metadata_rejects_wrong_minimum_os_for_device_family",
        "test_production_metadata_rejects_missing_attestation_challenge_binding",
        "test_production_metadata_rejects_missing_release_apk_binding",
        "test_production_metadata_rejects_release_apk_digest_drift",
        "test_production_metadata_rejects_missing_d2d_payment_transcript_binding",
        "test_production_metadata_rejects_d2d_payment_transcript_digest_drift",
        "test_production_metadata_rejects_d2d_payment_transcript_outside_handoff",
        "test_production_metadata_rejects_missing_wallet_integrity_transcript_binding",
        "test_production_metadata_rejects_wallet_integrity_transcript_digest_drift",
        "test_production_metadata_rejects_wallet_integrity_false_rollback_claim",
        "test_production_metadata_rejects_wallet_integrity_unchanged_rotation_key",
        "test_production_metadata_rejects_d2d_payment_transcript_secret_field_with_redaction",
        "test_production_metadata_rejects_d2d_payment_transcript_queue_splice",
        "test_production_metadata_rejects_d2d_payment_transcript_online_wallets",
        "test_production_metadata_rejects_d2d_payment_transcript_attestation_challenge_splice",
        "test_production_metadata_rejects_d2d_payment_transcript_unchanged_payer_wallet_state",
        "test_production_metadata_rejects_oversized_d2d_payment_payload",
        "test_production_metadata_rejects_missing_d2d_handoff_raw_command_marker",
        "test_production_metadata_rejects_stale_native_bridge_abi_version",
        "test_production_metadata_rejects_attestation_result_challenge_mismatch",
        "test_production_metadata_rejects_attestation_result_chain_digest_mismatch",
        "test_production_metadata_rejects_attestation_slot_alias_mismatch",
        "test_production_metadata_rejects_attestation_result_without_strongbox",
        "test_production_metadata_rejects_signed_evidence_challenge_mismatch",
        "test_production_metadata_rejects_signed_evidence_attestation_chain_mismatch",
        "test_production_metadata_rejects_signed_evidence_apk_digest_mismatch",
        "test_production_metadata_rejects_signed_evidence_d2d_transcript_digest_mismatch",
        "test_production_metadata_rejects_signed_evidence_wallet_integrity_digest_mismatch",
        "test_production_metadata_rejects_unexpected_signed_evidence_fields_with_redaction",
        "test_production_metadata_rejects_signed_evidence_probe_state_mismatch",
        "test_production_metadata_rejects_signed_evidence_raw_command_mismatch",
        "test_production_metadata_rejects_irrelevant_raw_test_commands",
        "test_production_metadata_rejects_marker_stuffed_raw_test_commands",
        "test_production_metadata_rejects_noncanonical_signed_evidence_timestamp",
        " 2026-06-06T00:00:00Z ",
        "test_production_metadata_rejects_signed_evidence_schema_drift",
        "test_production_metadata_rejects_signed_evidence_slot_mismatch",
        "test_production_metadata_rejects_signed_evidence_digest_map_drift",
        "test_production_metadata_rejects_signed_evidence_missing_required_digest",
        "test_production_metadata_rejects_missing_required_slot_artifact",
        "test_production_metadata_rejects_empty_required_slot_artifact",
        "test_production_metadata_rejects_oversized_required_slot_artifact",
        "test_production_metadata_rejects_telemetry_slot_mismatch",
        "test_production_metadata_rejects_failed_status_ndjson",
        "test_production_metadata_rejects_runtime_log_without_completion_marker",
        "test_production_metadata_rejects_runtime_log_failure_marker",
        "test_production_metadata_rejects_signed_evidence_missing_handoff_digest",
        "test_production_metadata_rejects_missing_trusted_signer_public_key",
        "test_production_metadata_rejects_untrusted_signed_evidence_key",
        "test_production_metadata_rejects_trusted_signer_public_key_symlinked_ancestor_from_direct_map",
        "test_production_metadata_rejects_signed_evidence_payload_hash_drift",
        "test_production_metadata_rejects_signed_evidence_signature_drift",
        "test_json_summary_reports_kagemusha_matrix_and_signer_pins",
        "test_json_summary_does_not_leak_trusted_signer_key_paths",
        "test_json_summary_does_not_leak_device_lab_root_or_summary_output_path",
        "test_root_validator_rejects_secret_path_directly_without_leak",
        "test_main_rejects_secret_looking_root_without_leak",
        "test_json_summary_rejects_secret_looking_output_without_leak",
        "test_write_summary_rejects_secret_output_path_directly_without_leak",
        "test_json_summary_rejects_symlinked_output_without_following_alias",
        "test_json_summary_rejects_hardlinked_output_without_overwriting_alias",
        "test_standard_matrix_requires_every_kagemusha_device_family",
        "test_standard_matrix_accepts_all_kagemusha_device_families",
        "test_signer_helper_generates_validator_accepted_evidence",
        "test_signer_helper_rejects_mismatched_private_and_public_keys",
        "test_trusted_signer_public_key_rejects_symlink_without_path_leak",
        "test_trusted_signer_public_key_rejects_secret_looking_path_without_leak",
        "test_trusted_signer_public_key_rejects_symlinked_ancestor_without_path_leak",
        "test_trusted_signer_public_key_rejects_hardlink_without_path_leak",
        "test_signer_helper_rejects_symlinked_private_key_before_write",
        "test_signer_helper_rejects_symlinked_private_key_ancestor_before_write",
        "test_signer_helper_rejects_symlinked_public_key_ancestor_before_write",
        "trusted signer public key ancestor directory must not be a symlink",
        "private key ancestor directory must not be a symlink",
        "signer public key ancestor directory must not be a symlink",
        "test_signer_helper_rejects_hardlinked_public_key_before_write",
        "test_signer_helper_rejects_secret_looking_public_key_path_before_write",
        "test_signer_helper_rejects_secret_looking_slot_path_before_metadata_read",
        "test_signer_helper_rejects_secret_looking_output_before_metadata_read",
        "test_signer_helper_rejects_secret_looking_signer_key_id_before_metadata_read",
        "private key path must not contain secret-looking material",
        "signer public key path must not contain secret-looking material",
        "test_signer_helper_rejects_output_outside_evidence_before_write",
        "test_signer_helper_rejects_noncanonical_output_filename_before_write",
        "test_signer_write_json_rejects_symlinked_output_parent_before_write",
        "test_signer_write_json_rejects_symlinked_output_ancestor_before_write",
        "test_signer_write_json_rejects_symlinked_output_ancestor_before_creating_parent",
        "test_signer_write_json_rejects_symlinked_output_leaf_before_write",
        "test_signer_write_json_rejects_hardlinked_output_leaf_before_write",
        "test_signer_write_json_rejects_secret_output_path_directly_without_write",
        "test_signer_write_text_rejects_symlinked_manifest_leaf_before_write",
        "test_signer_write_text_rejects_hardlinked_manifest_leaf_before_write",
        "test_signer_write_text_rejects_secret_manifest_path_directly_without_write",
        "test_rewrite_sha256_manifest_rejects_symlinked_artifact_when_called_directly",
        "test_rewrite_sha256_manifest_rejects_hardlinked_manifest_when_called_directly",
        "test_rewrite_sha256_manifest_rejects_secret_looking_artifact_when_called_directly",
        "test_rewrite_sha256_manifest_rejects_secret_slot_path_directly_without_write",
        "test_signer_metadata_loader_rejects_secret_slot_path_directly_without_parse",
        "test_signer_helper_rejects_symlinked_slot_json_before_write",
        "test_signer_helper_rejects_hardlinked_slot_json_before_write",
        "test_signer_metadata_loader_preflights_symlinked_artifacts",
        "test_signer_metadata_loader_preflights_hardlinked_artifacts",
        "test_signer_helper_rejects_symlinked_required_artifact_before_write",
        "test_signer_helper_rejects_symlinked_slot_directory_before_write",
        "test_signer_helper_rejects_symlinked_slot_parent_before_write",
        "test_signer_helper_rejects_symlinked_slot_ancestor_before_write",
        "test_signer_helper_rejects_hardlinked_required_artifact_before_write",
        "test_signer_helper_rejects_non_regular_required_artifact_before_write",
        "test_signer_helper_rejects_noncanonical_signed_at_utc",
        "test_signer_helper_rejects_unexpected_slot_metadata_field",
        "test_signer_helper_rejects_duplicate_slot_json_key_before_write",
        "test_signer_helper_rejects_duplicate_attestation_json_key_before_write",
        "test_signer_helper_rejects_duplicate_d2d_transcript_json_key_before_write",
        "test_signer_helper_rejects_duplicate_wallet_integrity_json_key_before_write",
        "test_signer_helper_rejects_irrelevant_raw_test_commands",
        "test_signer_helper_rejects_marker_stuffed_raw_test_commands",
        "test_signer_helper_rejects_missing_native_bridge_abi_before_write",
        "test_signer_helper_rejects_attestation_result_mismatch_before_write",
        "test_signer_helper_rejects_d2d_transcript_mismatch_before_write",
        "test_signer_helper_rejects_wallet_integrity_transcript_mismatch_before_write",
        "test_signer_helper_rejects_secret_looking_artifact_paths_before_write",
        "test_signer_helper_rejects_missing_required_slot_artifact_before_write",
        "test_signer_helper_rejects_empty_required_slot_artifact_before_write",
        "test_signer_helper_rejects_failed_status_ndjson_before_write",
        "test_signer_helper_does_not_leak_secret_looking_private_key_path",
    ),
    "scripts/tests/kagemusha_production_readiness_test.py": (
        "test_complete_signed_android_matrix_passes_rollup",
        "summary[\"lineage_proof_evidence\"][\"generated_at_utc\"]",
        "test_missing_android_root_blocks_rollup",
        "test_missing_standard_family_blocks_rollup",
        "test_duplicate_device_fingerprint_blocks_rollup",
        "test_duplicate_attestation_challenge_blocks_rollup",
        "test_stale_signed_evidence_blocks_rollup",
        "test_future_signed_evidence_blocks_rollup",
        "test_signed_evidence_freshness_uses_validated_report_timestamp",
        "test_signed_evidence_freshness_requires_report_timestamp",
        "test_duplicate_signed_evidence_json_key_blocks_rollup",
        "test_explicit_missing_slot_blocks_without_traceback",
        "test_unsafe_slot_id_blocks_rollup_without_path_escape",
        "test_untrusted_signed_evidence_blocks_rollup",
        "summary[\"android_device_lab\"][\"signed_evidence\"]",
        "expected_android_signed_evidence",
        "test_abi6_manifest_drift_blocks_rollup_section",
        "test_abi6_manifest_rejects_symlinked_manifest_file",
        "test_abi6_manifest_rejects_symlinked_manifest_ancestor",
        "test_abi6_manifest_rejects_hardlinked_manifest_file",
        "test_release_local_json_validator_rejects_secret_path_directly_without_parse",
        "test_repo_source_marker_validator_rejects_secret_path_directly_without_metadata",
        "test_abi7_fail_closed_rejects_symlinked_source_marker_file",
        "test_abi7_fail_closed_rejects_hardlinked_source_marker_file",
        "test_lineage_key_release_tooling_drift_blocks_rollup_section",
        "test_lineage_key_release_tooling_rejects_symlinked_marker_file",
        "test_lineage_key_release_tooling_rejects_hardlinked_marker_file",
        "test_missing_lineage_proof_evidence_blocks_rollup_section",
        "test_lineage_proof_evidence_rejects_noncanonical_filename",
        "test_lineage_proof_evidence_rejects_symlinked_evidence_file",
        "test_lineage_proof_evidence_rejects_symlinked_evidence_ancestor",
        "test_lineage_proof_evidence_rejects_secret_path_before_json_parse",
        "test_lineage_proof_evidence_rejects_duplicate_json_keys",
        "test_lineage_proof_evidence_redacts_secret_duplicate_json_key",
        "test_stale_lineage_proof_evidence_blocks_rollup_section",
        "test_lineage_proof_evidence_rejects_noncanonical_timestamp",
        "test_future_lineage_proof_evidence_blocks_rollup_section",
        "test_lineage_proof_evidence_drift_blocks_rollup_section",
        "test_lineage_proof_evidence_rejects_float_scalar_claims",
        "test_lineage_proof_evidence_rejects_runtime_keygen_command",
        "test_lineage_proof_evidence_rejects_fake_runner_command",
        "test_lineage_proof_evidence_rejects_appended_shell_command",
        "test_lineage_proof_evidence_rejects_shell_equivalent_noncanonical_command",
        "test_lineage_proof_evidence_rejects_secret_looking_command_without_leak",
        "test_lineage_proof_evidence_rejects_missing_local_artifact_file",
        "test_lineage_proof_evidence_rejects_symlinked_local_artifact_file",
        "test_lineage_proof_evidence_rejects_hardlinked_local_artifact_file",
        "test_lineage_proof_evidence_rejects_local_artifact_digest_mismatch",
        "summary[\"lineage_proof_evidence\"][\"artifact_sha256\"]",
        "summary[\"lineage_proof_evidence\"][\"test_log_sha256\"]",
        "test_lineage_proof_evidence_rejects_missing_local_proof_log_file",
        "test_lineage_proof_evidence_rejects_symlinked_local_proof_log_file",
        "test_lineage_proof_evidence_rejects_hardlinked_local_proof_log_file",
        "test_lineage_proof_log_rejects_secret_path_before_digest",
        "test_lineage_proof_log_rejects_symlinked_ancestor_before_digest",
        "test_lineage_proof_evidence_rejects_oversized_local_proof_log",
        "test_lineage_proof_evidence_rejects_local_proof_log_digest_mismatch",
        "test_lineage_proof_evidence_rejects_bad_local_proof_log_content",
        "test_lineage_proof_evidence_rejects_marker_stuffed_local_proof_log",
        "test_lineage_proof_evidence_rejects_boolean_or_nonfinite_elapsed",
        "test_lineage_proof_evidence_rejects_unexpected_top_level_field_with_redaction",
        "test_lineage_proof_evidence_rejects_unexpected_nested_fields_with_redaction",
        "test_lineage_proof_evidence_helper_generates_validator_accepted_json",
        "test_lineage_proof_evidence_document_validator_rejects_symlinked_artifact_dir",
        "test_lineage_proof_evidence_document_validator_rejects_secret_artifact_dir",
        "test_lineage_proof_artifact_dir_validator_rejects_secret_path_directly",
        "test_lineage_proof_evidence_helper_rejects_missing_artifact",
        "test_lineage_proof_evidence_helper_rejects_symlinked_artifact",
        "test_lineage_proof_evidence_helper_rejects_hardlinked_artifact",
        "test_lineage_proof_evidence_helper_rejects_noncanonical_generated_at_utc",
        "test_lineage_proof_evidence_helper_rejects_runtime_keygen_command",
        "test_lineage_proof_evidence_helper_rejects_fake_runner_command",
        "test_lineage_proof_evidence_helper_rejects_appended_shell_command",
        "test_lineage_proof_evidence_helper_rejects_shell_equivalent_noncanonical_command",
        "test_lineage_proof_evidence_helper_rejects_secret_looking_command_without_leak",
        "test_lineage_proof_evidence_helper_rejects_nonfinite_elapsed",
        "test_lineage_proof_evidence_helper_rejects_outside_artifact_dir",
        "test_lineage_proof_evidence_helper_rejects_noncanonical_output_filename",
        "test_lineage_proof_evidence_helper_rejects_symlinked_artifact_dir",
        "test_lineage_proof_evidence_helper_preflights_output_ancestor_before_artifact_reads",
        "test_lineage_proof_evidence_helper_rejects_symlinked_output_ancestor",
        "test_lineage_proof_output_validator_rejects_symlinked_ancestor_before_creating_parent",
        "test_lineage_proof_output_preflight_rejects_secret_path_directly_before_creating_parent",
        "test_lineage_proof_write_evidence_rejects_secret_output_path_before_write",
        "test_lineage_proof_evidence_helper_rejects_symlinked_output_leaf",
        "test_lineage_proof_evidence_helper_rejects_hardlinked_output_leaf",
        "test_lineage_proof_evidence_helper_rejects_detached_proof_log",
        "test_lineage_proof_build_evidence_rejects_detached_proof_log_directly",
        "test_lineage_proof_build_evidence_rejects_secret_looking_proof_log_before_reads",
        "test_lineage_proof_input_validator_rejects_secret_proof_log_directly_before_resolve",
        "test_lineage_proof_evidence_helper_rejects_log_without_test_name",
        "test_lineage_proof_evidence_helper_rejects_marker_stuffed_proof_log",
        "test_lineage_proof_evidence_helper_rejects_failed_proof_log",
        "test_summary_does_not_leak_trusted_signer_key_paths",
        "test_summary_does_not_leak_device_lab_root_path",
        "test_secret_looking_device_lab_root_blocks_without_leak",
        "test_validate_repo_root_rejects_secret_path_directly_without_leak",
        "test_trust_root_sections_reject_secret_repo_root_before_reads",
        "test_symlinked_repo_root_blocks_before_rollup_without_path_leak",
        "test_symlinked_repo_root_ancestor_blocks_before_rollup_without_path_leak",
        "test_symlinked_android_root_blocks_rollup_without_path_leak",
        "test_symlinked_android_root_ancestor_blocks_rollup_without_path_leak",
        "test_android_report_secret_material_is_redacted_before_summary",
        "test_secret_looking_summary_out_blocks_before_write_without_leak",
        "test_write_summary_rejects_secret_path_before_direct_write",
        "test_symlinked_summary_out_blocks_without_following_alias",
        "test_symlinked_summary_out_ancestor_blocks_before_creating_parent",
        "test_hardlinked_summary_out_blocks_without_overwriting_alias",
        "test_secret_looking_trusted_signer_path_blocks_without_leak",
        "test_negative_lineage_proof_future_skew_blocks_before_rollup",
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
        "KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_PROOF_UNAVAILABLE",
        "semantic ABI-7 compact tokens are disabled for production",
        "composed private-hop verifier batch to be proved in-circuit",
        "decode_kagemusha_recursive_compact_pallas_open_envelopes",
        "failed to decode Kagemusha recursive compact Pallas open-envelope archive",
        "invalid Kagemusha recursive compact Pallas open-envelope archive",
        "invalid Kagemusha recursive compact record-backed Pallas preflight",
        "fn kagemusha_recursive_compact_record_prover_preflights_pallas_archive_before_unavailable",
        "record-bound multi-hop compact Pallas archive must reject before the unavailable gate",
        "Err(KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE.to_owned())",
        "returns the production-unavailable diagnostic until ABI-7 compact proofs",
        "compose the private-hop verifier slice in-circuit",
        "pub fn verify_kagemusha_recursive_compact_payment_token(",
        "false",
        "preverify_kagemusha_recursive_compact_payment_token_with_record",
        "kagemusha_recursive_spend_lineage_vk_record_from_box_for_circuit",
        "pub fn kagemusha_recursive_spend_lineage_vk_record_from_box(",
        "pub fn kagemusha_recursive_spend_lineage_append_vk_record_from_box(",
        "does not generate a verifier key at runtime",
        "lineage_vk_record_from_box_canonicalizes_profiles_without_keygen",
    ),
    "crates/iroha_cli/src/zk.rs": (
        "KagemushaCommand::LineageKeyArtifacts",
        "KagemushaCommand::LineageRecord",
        "KagemushaLineageRecordArgs",
        "record_out: Option<std::path::PathBuf>",
        "record_namespace: String",
        "record_version: u32",
        "kagemusha_lineage_vk_record_from_bytes",
        "std::fs::read(&self.vk)",
        "kagemusha_recursive_spend_lineage_vk_record_from_box(",
        "kagemusha_recursive_spend_lineage_append_vk_record_from_box(",
        "kagemusha_lineage_record_run_writes_norito_record_from_existing_vk_file",
        'record_summary = format!(", record={} bytes", record_bytes.len())',
    ),
    "crates/connect_norito_bridge/src/lib.rs": (
        "CONNECT_NORITO_BRIDGE_ABI_VERSION: u32 = 7;",
        "KagemushaRecursiveCompactUnavailable",
        "prove_verified_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelope_archive",
        "is_kagemusha_recursive_compact_unavailable_error",
        "Vec<iroha_zkp_halo2::OpenVerifyEnvelope>",
        "valid recursive compact Pallas envelope fixture must decode",
        "detached valid Pallas opening archives before the unavailable gate",
        "valid multi-hop recursive compact Pallas archives must map to unavailable",
        "shape-valid ABI-7 compact tokens must return a soft invalid result",
        "shape-valid envelopes with stale folded-token bindings must hard-fail before soft invalid",
        "preverify_kagemusha_recursive_compact_payment_token",
        "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE",
        "*out_valid = 0",
        "connect_norito_kagemusha_recursive_spend_redeem",
    ),
    "crates/iroha_js_host/src/lib.rs": (
        "connect_norito_bridge_abi_version() -> u32",
        "prove_verified_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelope_archive",
        "failed to decode Kagemusha recursive compact Pallas open-envelope archive",
        "invalid Kagemusha recursive compact Pallas open-envelope archive",
        "invalid Kagemusha recursive compact record-backed Pallas preflight",
        "detached valid recursive compact Pallas archive must reject",
        "valid multi-hop recursive compact archive must remain unavailable",
        "KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_PROOF_UNAVAILABLE",
        "preverify_kagemusha_recursive_compact_payment_token",
        "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE",
        "return Ok(false);",
        "kagemusha_recursive_spend_redeem_instruction_from_request",
    ),
    "python/iroha_python/iroha_python_rs/src/lib.rs": (
        "kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes_py",
        "prove_verified_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelope_archive",
        "failed to decode Kagemusha recursive compact Pallas open-envelope archive",
        "invalid Kagemusha recursive compact Pallas open-envelope archive",
        "invalid Kagemusha recursive compact record-backed Pallas preflight",
        "detached valid Pallas archive",
        "valid multi-hop recursive compact archive must remain unavailable",
        "KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_PROOF_UNAVAILABLE",
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
    '"scripts/check_android_device_lab_slot.py"',
    '"scripts/sign_android_device_lab_evidence.py"',
    '"scripts/kagemusha_production_readiness.py"',
    '"scripts/kagemusha_lineage_proof_evidence.py"',
    '"scripts/tests/check_android_device_lab_slot_test.py"',
    '"scripts/tests/kagemusha_production_readiness_test.py"',
    '"fixtures/android/device_lab/**"',
    "python3 -m unittest discover -s scripts/tests -p check_android_device_lab_slot_test.py",
    "python3 -m unittest discover -s scripts/tests -p kagemusha_production_readiness_test.py",
    "ci/check_kagemusha_production_readiness.sh --negative-control-doc-route",
    "ci/check_kagemusha_production_readiness.sh --negative-control-abi6-manifest",
    "ci/check_kagemusha_production_readiness.sh --negative-control-abi6-manifest-file-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-abi6-manifest-ancestor-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-sdk-default",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-open",
    "ci/check_kagemusha_production_readiness.sh --negative-control-abi7-source-marker-file-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-pallas-envelope-type",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-matrix",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-test-workflow",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-artifact-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signed-evidence-path-root",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signed-evidence-path-canonical",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-release-apk-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-minimum-os",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-attestation-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-attestation-chain-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-attestation-chain-shape",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-attestation-slot-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-attestation-schema",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-physical-device",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-d2d-transcript",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-d2d-path-root",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-wallet-integrity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-unique-bindings",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-production-claim-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-summary",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-direct-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-secret-redaction",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-root-direct-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-duplicate-json-keys",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-root-symlink",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-root-ancestor-symlink",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-load-ancestor",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-load-direct-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-parse-direct-slot-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-verify-direct-slot-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-slot-root-symlink",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-slot-ancestor-symlink",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-direct-helper-slot-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-direct-symlink-artifact-slot-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-direct-hardlink-artifact-slot-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-direct-regular-artifact-slot-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-files-direct-root-shape",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-files-direct-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-files-direct-ancestor-symlink",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-files-direct-symlink-directory",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-verify-symlink-directory",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-dir-symlink",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-parent-symlink",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-ancestor-symlink",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-symlink-artifacts",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-regular-file-artifacts",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-hardlink-artifacts",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-required-artifacts",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-required-artifact-shape",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-required-artifact-content",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-id-safety",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-name-safety",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signed-artifact-schema",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signature-verify",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-write",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-ancestor",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-direct-output-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-manifest-write",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-direct-manifest-shape",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-manifest-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-metadata-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-direct-slot-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-direct-manifest-slot-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signer-key-files",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signer-key-ancestors",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-private-key-ancestors",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signer-key-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-cli-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-cli-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-command-exact",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-signed-evidence-freshness-report",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-rollup",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-rollup-path-safety",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-android-report-secret-redaction",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-repo-root-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-repo-root-direct-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-trust-root-section-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-release-json-direct-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-source-marker-direct-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-ancestor",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-direct-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-key-release-tooling",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-key-release-source-marker-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-evidence",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-evidence-path-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-local-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-local-ancestor-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-artifact-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-file-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-future-skew",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-ancestor",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-early-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-direct-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-direct-artifact-dir-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-direct-proof-log-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-direct-output-preflight-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-validation-dir-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-input-corridor",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-command-canonical",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-scalar-types",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-timestamp-raw",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-timestamp-raw",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-log-exact",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-evidence-filename",
    "ci/check_kagemusha_production_readiness.sh --negative-control-json-duplicate-keys",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-closed-schema",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-evidence-helper",
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

if mode == "--negative-control-abi6-manifest-file-aliases":
    run_negative_control(
        "ABI-6 manifest file alias gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "abi6_manifest_file_shape",
            "abi6_manifest_file_alias_allowed",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-abi6-manifest-ancestor-aliases":
    run_negative_control(
        "ABI-6 manifest ancestor alias gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "release_json_ancestor_errors = device_lab.validate_no_symlink_ancestors(",
            "release_json_ancestor_errors = _skip_release_json_ancestor_validation(",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-release-json-direct-secret-paths":
    run_negative_control(
        "Kagemusha readiness release JSON direct secret-path gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            'def validate_release_local_json_file(path: Path, label: str) -> list[str]:\n    """Reject local release JSON files that could alias external bytes."""\n\n    if device_lab.SECRET_RE.search(str(path)):\n        return [f"{label} path must not contain secret-looking material"]\n',
            'def validate_release_local_json_file(path: Path, label: str) -> list[str]:\n    """Reject local release JSON files that could alias external bytes."""\n\n',
        ),
    )
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

if mode == "--negative-control-abi7-source-marker-file-aliases":
    run_negative_control(
        "ABI-7 source marker file alias gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "abi7_source_marker_file_shape",
            "abi7_source_marker_file_alias_allowed",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-source-marker-direct-secret-paths":
    run_negative_control(
        "Kagemusha readiness source marker direct secret-path gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            'def validate_repo_source_marker_file(path: Path, label: str) -> list[str]:\n    """Reject checked-in marker files that could alias external bytes."""\n\n    if device_lab.SECRET_RE.search(str(path)):\n        return [f"{label} path must not contain secret-looking material"]\n',
            'def validate_repo_source_marker_file(path: Path, label: str) -> list[str]:\n    """Reject checked-in marker files that could alias external bytes."""\n\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-pallas-envelope-type":
    run_negative_control(
        "ABI-7 compact Pallas envelope preflight type",
        lambda: override_text(
            "crates/iroha_core/src/zk.rs",
            "fn kagemusha_recursive_compact_record_prover_preflights_pallas_archive_before_unavailable",
            "fn kagemusha_recursive_compact_record_prover_skips_pallas_archive_before_unavailable",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-matrix":
    run_negative_control(
        "Android device-matrix compact unavailable boundary",
        lambda: override_text(
            "docs/source/sdk/android/readiness/android_strongbox_device_matrix.md",
            "ABI 7 recursive compact prover calls that reach the proof-composition",
            "ABI 7 recursive compact prover calls may be accepted as production state",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-test-workflow":
    run_negative_control(
        "Android device-lab validator workflow",
        lambda: override_text(
            WORKFLOW_PATH,
            "python3 -m unittest discover -s scripts/tests -p check_android_device_lab_slot_test.py",
            "python3 -m unittest discover -s scripts/tests -p disabled_check_android_device_lab_slot_test.py",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-artifact-binding":
    run_negative_control(
        "Android device-lab signed-evidence artifact binding",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "signed_evidence_artifact_sha256 does not match signed_evidence_artifact_path",
            "signed_evidence_artifact_sha256 is accepted without matching signed_evidence_artifact_path",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signed-evidence-path-root":
    run_negative_control(
        "Android device-lab signed evidence artifact path root",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "slot.json signed_evidence_artifact_path must stay under evidence/",
            "slot.json signed_evidence_artifact_path may point outside evidence/",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signed-evidence-path-canonical":
    run_negative_control(
        "Android device-lab signed evidence canonical artifact path",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "slot.json signed_evidence_artifact_path must be",
            "slot.json signed_evidence_artifact_path may be",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-release-apk-binding":
    run_negative_control(
        "Android device-lab release APK and native ABI binding",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "slot.json native_bridge_abi_version must be",
            "slot.json native_bridge_abi_version may be",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-minimum-os":
    run_negative_control(
        "Android device-lab family minimum OS binding",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "slot.json minimum_os for {family} must be",
            "slot.json unsupported_os for {family} must be",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-attestation-binding":
    run_negative_control(
        "Android device-lab attestation challenge binding",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "SIGNED_EVIDENCE_SLOT_SHA256_FIELDS: tuple[str, ...]",
            "SIGNED_EVIDENCE_SLOT_OPTIONAL_SHA256_FIELDS: tuple[str, ...]",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-attestation-chain-binding":
    run_negative_control(
        "Android device-lab attestation certificate-chain binding",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "slot.json attestation_certificate_chain_sha256 does not match attestation_certificate_chain_path",
            "slot.json attestation_certificate_chain_sha256 may ignore attestation_certificate_chain_path",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-attestation-chain-shape":
    run_negative_control(
        "Android device-lab attestation certificate-chain artifact shape",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "attestation certificate chain PEM must contain certificate boundaries",
            "attestation certificate chain PEM may omit certificate boundaries",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-attestation-slot-binding":
    run_negative_control(
        "Android device-lab attestation slot binding",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "attestation/result.json {slot_key} must match the slot directory name",
            "attestation/result.json {slot_key} may differ from the slot directory name",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-attestation-schema":
    run_negative_control(
        "Android device-lab attestation result schema",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "set(result) - ATTESTATION_RESULT_FIELDS",
            "set()",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-physical-device":
    run_negative_control(
        "Android device-lab physical-device attestation",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "attestation/result.json physical_device_attestation must be true",
            "attestation/result.json physical_device_attestation may be false",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-d2d-transcript":
    run_negative_control(
        "Android device-lab D2D payment transcript binding",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "d2d payment transcript queue_after_sha256 must match queue/pending_queue.json",
            "d2d payment transcript queue_after_sha256 may ignore queue/pending_queue.json",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-d2d-path-root":
    run_negative_control(
        "Android device-lab D2D payment transcript handoff path root",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "slot.json d2d_payment_transcript_path must stay under handoff/",
            "slot.json d2d_payment_transcript_path may point outside handoff/",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-wallet-integrity":
    run_negative_control(
        "Android device-lab wallet integrity transcript binding",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "wallet integrity transcript stale_snapshot_rejected must be true",
            "wallet integrity transcript stale_snapshot_rejected may be false",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-unique-bindings":
    run_negative_control(
        "Android device-lab unique matrix bindings",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "Android device-lab production slots must not reuse a device fingerprint",
            "Android device-lab production slots may reuse a device fingerprint",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-production-claim-binding":
    run_negative_control(
        "Android device-lab signed production-claim binding",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "SIGNED_EVIDENCE_SLOT_TRUE_FIELDS: tuple[str, ...]",
            "SIGNED_EVIDENCE_SLOT_OPTIONAL_TRUE_FIELDS: tuple[str, ...]",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-summary":
    run_negative_control(
        "Android device-lab Kagemusha summary binding",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "trusted_signer_public_key_sha256",
            "trusted_signer_public_key_paths",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-json-output-aliases":
    run_negative_control(
        "Android device-lab JSON summary output alias gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            'return [f"{label} must not be a symlink"]',
            'return [f"{label} may be a symlink"]',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-json-output-direct-secret-paths":
    run_negative_control(
        "Android device-lab direct JSON summary output secret-path gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    if SECRET_RE.search(str(path)):\n        return [f"{label} must not contain secret-looking material"]\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-secret-redaction":
    run_negative_control(
        "Android device-lab secret-looking path redaction",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "f\"[{_display_path(relative)}] must be lowercase sha256 hex\"",
            "f\"[{relative}] must be lowercase sha256 hex\"",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-root-direct-secret-paths":
    run_negative_control(
        "Android device-lab direct root secret-path gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    if SECRET_RE.search(str(root)):\n        return ["device-lab root path must not contain secret-looking material"]\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-duplicate-json-keys":
    run_negative_control(
        "Android device-lab duplicate JSON key gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "object_pairs_hook=_reject_duplicate_json_object_pairs",
            "object_pairs_hook=dict",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-root-symlink":
    run_negative_control(
        "Android device-lab root symlink gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "device-lab root must not be a symlink",
            "device-lab root may be a symlink",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-root-ancestor-symlink":
    run_negative_control(
        "Android device-lab root ancestor symlink gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "device-lab root ancestor directory",
            "device-lab root ancestor path",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-json-load-ancestor":
    run_negative_control(
        "Android device-lab JSON loader ancestor symlink gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "json_ancestor_errors = validate_no_symlink_ancestors(",
            "json_ancestor_errors = _skip_json_ancestor_validation(",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-json-load-direct-secret-paths":
    run_negative_control(
        "Android device-lab JSON loader direct secret-path gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            'def _load_json(path: Path, label: str, errors: list[str]) -> dict[str, Any] | None:\n    if SECRET_RE.search(str(path)):\n        errors.append(f"{label} path must not contain secret-looking material")\n        return None\n',
            'def _load_json(path: Path, label: str, errors: list[str]) -> dict[str, Any] | None:\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-manifest-parse-direct-slot-secret-paths":
    run_negative_control(
        "Android device-lab manifest parser direct slot secret-path gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            'root_errors = _validate_manifest_slot_path(slot_path)\n    if root_errors:\n        return entries, root_errors\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-manifest-verify-direct-slot-secret-paths":
    run_negative_control(
        "Android device-lab manifest verifier direct slot secret-path gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            'root_errors = _validate_manifest_slot_path(slot_path)\n    if root_errors:\n        return root_errors\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-manifest-slot-root-symlink":
    run_negative_control(
        "Android device-lab manifest slot-root symlink gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    if slot_path.is_symlink():\n        return ["slot directory must not be a symlink"]\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-manifest-slot-ancestor-symlink":
    run_negative_control(
        "Android device-lab manifest slot-ancestor symlink gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    return validate_no_symlink_ancestors(slot_path, "slot ancestor directory")\n',
            "    return []\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-direct-helper-slot-secret-paths":
    run_negative_control(
        "Android device-lab direct helper slot secret-path gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            'def _reject_secret_slot_path(slot_path: Path, errors: list[str]) -> bool:\n    """Reject direct helper calls that receive secret-looking slot paths."""\n\n    if SECRET_RE.search(str(slot_path)):\n        errors.append("slot path must not contain secret-looking material")\n        return True\n    return False\n',
            'def _reject_secret_slot_path(slot_path: Path, errors: list[str]) -> bool:\n    """Reject direct helper calls that receive secret-looking slot paths."""\n\n    return False\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-direct-symlink-artifact-slot-secret-paths":
    run_negative_control(
        "Android device-lab direct symlink-artifact slot secret-path gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            'def validate_no_slot_symlink_artifacts(slot_path: Path, errors: list[str]) -> None:\n    """Reject symlinked slot metadata, directories, and evidence artifacts."""\n\n    if _reject_secret_slot_path(slot_path, errors):\n        return\n',
            'def validate_no_slot_symlink_artifacts(slot_path: Path, errors: list[str]) -> None:\n    """Reject symlinked slot metadata, directories, and evidence artifacts."""\n\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-direct-hardlink-artifact-slot-secret-paths":
    run_negative_control(
        "Android device-lab direct hardlink-artifact slot secret-path gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            'def validate_no_slot_hardlink_artifacts(slot_path: Path, errors: list[str]) -> None:\n    """Reject hardlinked slot metadata and evidence artifacts."""\n\n    if _reject_secret_slot_path(slot_path, errors):\n        return\n',
            'def validate_no_slot_hardlink_artifacts(slot_path: Path, errors: list[str]) -> None:\n    """Reject hardlinked slot metadata and evidence artifacts."""\n\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-direct-regular-artifact-slot-secret-paths":
    run_negative_control(
        "Android device-lab direct regular-artifact slot secret-path gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            'def validate_slot_regular_file_artifacts(slot_path: Path, errors: list[str]) -> None:\n    """Reject special-file slot metadata and evidence artifacts."""\n\n    if _reject_secret_slot_path(slot_path, errors):\n        return\n',
            'def validate_slot_regular_file_artifacts(slot_path: Path, errors: list[str]) -> None:\n    """Reject special-file slot metadata and evidence artifacts."""\n\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-files-direct-root-shape":
    run_negative_control(
        "Android device-lab slot file discovery direct root-shape gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            'def _slot_files(slot_path: Path) -> set[str]:\n    if slot_path.is_symlink() or not slot_path.is_dir():\n        return set()\n',
            'def _slot_files(slot_path: Path) -> set[str]:\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-files-direct-secret-paths":
    run_negative_control(
        "Android device-lab slot file discovery direct secret-path gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    if SECRET_RE.search(str(slot_path)):\n        return set()\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-files-direct-ancestor-symlink":
    run_negative_control(
        "Android device-lab slot file discovery ancestor symlink gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    if validate_no_symlink_ancestors(slot_path, "slot ancestor directory"):\n        return set()\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-files-direct-symlink-directory":
    run_negative_control(
        "Android device-lab slot file discovery symlink directory gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "if dir_path.is_symlink() or not dir_path.is_dir():",
            "if not dir_path.is_dir():",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-manifest-verify-symlink-directory":
    run_negative_control(
        "Android device-lab manifest verifier symlink directory gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '        if _slot_relative_symlink_ancestor(slot_path, relative) is not None:\n            errors.append(\n                "sha256sum.txt references artifact under symlink directory "\n                f"{_display_path(relative)}"\n            )\n            continue\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-dir-symlink":
    run_negative_control(
        "Android device-lab slot directory symlink gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "slot directory must not be a symlink",
            "slot directory may be a symlink",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-parent-symlink":
    run_negative_control(
        "Android device-lab slot parent symlink gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "slot parent directory must not be a symlink",
            "slot parent directory may be a symlink",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-ancestor-symlink":
    run_negative_control(
        "Android device-lab slot ancestor symlink gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "slot ancestor directory",
            "slot ancestor path",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-symlink-artifacts":
    run_negative_control(
        "Android device-lab symlink artifact gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "sha256sum.txt references symlink artifact",
            "sha256sum.txt accepts symlink artifact",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-regular-file-artifacts":
    run_negative_control(
        "Android device-lab regular-file artifact gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "sha256sum.txt references non-regular artifact",
            "sha256sum.txt accepts non-regular artifact",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-hardlink-artifacts":
    run_negative_control(
        "Android device-lab hardlink artifact gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "sha256sum.txt references hardlinked artifact",
            "sha256sum.txt accepts hardlinked artifact",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-required-artifacts":
    run_negative_control(
        "Android device-lab required artifact gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "signed evidence artifact required slot artifact is missing",
            "signed evidence artifact required slot artifact may be omitted",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-required-artifact-shape":
    run_negative_control(
        "Android device-lab required artifact shape gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "artifact_size == 0",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-required-artifact-content":
    run_negative_control(
        "Android device-lab required artifact content gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "logs/runtime.log must contain Kagemusha device-lab completion marker",
            "logs/runtime.log may omit Kagemusha device-lab completion marker",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-id-safety":
    run_negative_control(
        "Android device-lab explicit slot id safety",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "slot_ids, slot_id_errors = validate_slot_ids(args.slots)",
            "slot_ids, slot_id_errors = args.slots, []",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-name-safety":
    run_negative_control(
        "Android device-lab discovered slot name safety",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "slot directory name must not contain secret-looking material",
            "slot directory name may contain secret-looking material",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signed-artifact-schema":
    run_negative_control(
        "Android device-lab signed evidence artifact schema",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "signed evidence artifact digest mismatch for",
            "signed evidence artifact accepts digest drift for",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signature-verify":
    run_negative_control(
        "Android device-lab signed evidence signature verification",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "signed evidence artifact signature verification failed",
            "signed evidence artifact signature verification skipped",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper":
    run_negative_control(
        "Android device-lab signed evidence helper",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "device_lab._canonical_signed_evidence_payload",
            "json.dumps",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-output-write":
    run_negative_control(
        "Android device-lab signed evidence helper output write gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            'f"{label} parent directory must not be a symlink"',
            'f"{label} parent directory may be a symlink"',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-output-ancestor":
    run_negative_control(
        "Android device-lab signed evidence helper output ancestor gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            'f"{label} ancestor directory"',
            'f"{label} unchecked ancestor directory"',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-direct-output-secret-paths":
    run_negative_control(
        "Android device-lab signed evidence helper direct output secret-path gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            'def _validate_json_output_path(path: Path, label: str) -> list[str]:\n    """Validate a signer-controlled output immediately before writing."""\n\n    if device_lab.SECRET_RE.search(str(path)):\n        return [f"{label} must not contain secret-looking material"]\n',
            'def _validate_json_output_path(path: Path, label: str) -> list[str]:\n    """Validate a signer-controlled output immediately before writing."""\n\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-manifest-write":
    run_negative_control(
        "Android device-lab signed evidence helper manifest write gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '_write_text(slot_path / "sha256sum.txt"',
            '_write_text(slot_path / "sha256sum.unchecked"',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-direct-manifest-shape":
    run_negative_control(
        "Android device-lab signed evidence helper direct manifest shape gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "errors = _validate_slot_for_manifest_rewrite(slot_path)",
            "errors = []",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-manifest-secret-paths":
    run_negative_control(
        "Android device-lab signed evidence helper manifest secret-path gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "for relative in device_lab._slot_files(slot_path):",
            "for relative in ():",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-metadata-preflight":
    run_negative_control(
        "Android device-lab signed evidence helper metadata preflight gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "errors = _preflight_slot_metadata_reads(slot_path)",
            "errors = []",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-direct-slot-secret-paths":
    run_negative_control(
        "Android device-lab signed evidence helper direct metadata slot secret-path gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            'def _preflight_slot_metadata_reads(slot_path: Path) -> list[str]:\n    """Validate slot paths before any signer-controlled metadata is parsed."""\n\n    if device_lab.SECRET_RE.search(str(slot_path)):\n        return ["slot path must not contain secret-looking material"]\n',
            'def _preflight_slot_metadata_reads(slot_path: Path) -> list[str]:\n    """Validate slot paths before any signer-controlled metadata is parsed."""\n\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-direct-manifest-slot-secret-paths":
    run_negative_control(
        "Android device-lab signed evidence helper direct manifest slot secret-path gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            'def _validate_slot_for_manifest_rewrite(slot_path: Path) -> list[str]:\n    """Validate a slot immediately before rewriting its SHA-256 manifest."""\n\n    if device_lab.SECRET_RE.search(str(slot_path)):\n        return ["slot path must not contain secret-looking material"]\n',
            'def _validate_slot_for_manifest_rewrite(slot_path: Path) -> list[str]:\n    """Validate a slot immediately before rewriting its SHA-256 manifest."""\n\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signer-key-files":
    run_negative_control(
        "Android device-lab signer key-file alias gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "private key must not be a symlink",
            "private key may be a symlink",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signer-key-ancestors":
    run_negative_control(
        "Android device-lab trusted signer key ancestor gate",
        lambda: override_text(
            "scripts/tests/check_android_device_lab_slot_test.py",
            "test_trusted_signer_public_key_rejects_symlinked_ancestor_without_path_leak",
            "test_trusted_signer_public_key_allows_symlinked_ancestor_without_path_leak",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-private-key-ancestors":
    run_negative_control(
        "Android device-lab private key ancestor gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "private key ancestor directory",
            "private key ancestor path",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signer-key-secret-paths":
    run_negative_control(
        "Android device-lab signer key secret-path gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "if device_lab.SECRET_RE.search(str(path)):",
            "if False:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-cli-secret-paths":
    run_negative_control(
        "Android device-lab CLI secret-path gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "--root must not contain secret-looking material",
            "--root may contain secret-looking material",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-cli-secret-paths":
    run_negative_control(
        "Android device-lab signing helper CLI secret-path gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "slot path must not contain secret-looking material",
            "slot path may contain secret-looking material",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-command-exact":
    run_negative_control(
        "Android device-lab exact raw command gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "must exactly match the Kagemusha Android production raw test command",
            "may contain Kagemusha Android production raw test command markers",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-signed-evidence-freshness-report":
    run_negative_control(
        "Android signed-evidence freshness report binding",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            'report.get("kagemusha", {}).get("signed_at_utc")',
            'report.get("kagemusha", {}).get("unchecked_signed_at_utc")',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-rollup":
    run_negative_control(
        "Kagemusha production readiness evidence rollup",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "android_device_lab_standard_matrix_missing",
            "android_device_lab_matrix_optional",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-rollup-path-safety":
    run_negative_control(
        "Kagemusha readiness rollup path safety",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "path_blockers = validate_cli_path_arguments(args)",
            "path_blockers = []",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-android-report-secret-redaction":
    run_negative_control(
        "Kagemusha readiness Android report secret redaction",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "android_device_lab_report_secret_material",
            "android_device_lab_report_redaction_disabled",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-repo-root-aliases":
    run_negative_control(
        "Kagemusha readiness repo-root alias gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "repo_root_errors = validate_repo_root_path(Path(args.repo_root))",
            "repo_root_errors = []",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-repo-root-direct-secret-paths":
    run_negative_control(
        "Kagemusha readiness direct repo-root secret-path gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    secret_blocker = _secret_looking_path_blocker(\n        str(root),\n        label="--repo-root",\n        code="kagemusha_repo_root_path_invalid",\n    )\n    if secret_blocker is not None:\n        return [secret_blocker]\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-trust-root-section-preflight":
    run_negative_control(
        "Kagemusha readiness trust-root section repo-root preflight",
        lambda: (
            override_text(
                "scripts/kagemusha_production_readiness.py",
                "    repo_root_blockers = validate_repo_root_path(repo_root)\n    if repo_root_blockers:\n        details[\"ok\"] = False\n        details[\"blockers\"] = repo_root_blockers\n        return details\n\n",
                "",
            ),
            override_text(
                "scripts/kagemusha_production_readiness.py",
                "    repo_root_blockers = validate_repo_root_path(repo_root)\n    if repo_root_blockers:\n        return {\n            \"ok\": False,\n            \"state\": \"unknown\",\n            \"circuit_id\": \"kagemusha-recursive-compact-v1\",\n            \"blockers\": repo_root_blockers,\n        }\n\n",
                "",
            ),
            override_text(
                "scripts/kagemusha_production_readiness.py",
                "    repo_root_blockers = validate_repo_root_path(repo_root)\n    if repo_root_blockers:\n        return {\n            \"ok\": False,\n            \"state\": \"unknown\",\n            \"checked_files\": [],\n            \"blockers\": repo_root_blockers,\n        }\n\n",
                "",
            ),
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-summary-output-aliases":
    run_negative_control(
        "Kagemusha readiness summary output alias gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "--summary-out must not be a symlink",
            "--summary-out may be a symlink",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-summary-output-ancestor":
    run_negative_control(
        "Kagemusha readiness summary output ancestor alias gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "--summary-out ancestor directory",
            "--summary-out unchecked ancestor directory",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-summary-output-direct-secret-paths":
    run_negative_control(
        "Kagemusha readiness direct summary output secret-path gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    secret_blocker = _secret_looking_path_blocker(\n        str(path),\n        label="--summary-out",\n        code=SUMMARY_OUT_PATH_INVALID_CODE,\n    )\n    if secret_blocker is not None:\n        return [secret_blocker]\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-key-release-tooling":
    run_negative_control(
        "Reserved-lineage key release tooling",
        lambda: override_text(
            "crates/iroha_cli/src/zk.rs",
            "record_out: Option<std::path::PathBuf>",
            "record_archive_out: Option<std::path::PathBuf>",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-key-release-source-marker-aliases":
    run_negative_control(
        "Reserved-lineage key release source marker alias gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "lineage_key_release_file_shape",
            "lineage_key_release_file_alias_allowed",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-evidence":
    run_negative_control(
        "Reserved-lineage production proof evidence",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "lineage_proof_evidence_missing",
            "lineage_proof_evidence_optional",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-evidence-path-aliases":
    run_negative_control(
        "Reserved-lineage proof evidence path alias gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "lineage_proof_evidence_path=lineage_proof_evidence_path,",
            "lineage_proof_evidence_path=lineage_proof_evidence_path.resolve(),",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-local-secret-paths":
    run_negative_control(
        "Reserved-lineage proof evidence local secret-path gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    if device_lab.SECRET_RE.search(str(path)):\n        return [f"{label} path must not contain secret-looking material"]\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-local-ancestor-aliases":
    run_negative_control(
        "Reserved-lineage proof evidence local ancestor alias gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    ancestor_errors = device_lab.validate_no_symlink_ancestors(\n        path,\n        f"{label} ancestor directory",\n    )\n    if ancestor_errors:\n        return ancestor_errors\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-artifact-binding":
    run_negative_control(
        "Reserved-lineage proof evidence artifact byte binding",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "lineage_proof_evidence_artifact_file_digest",
            "lineage_proof_evidence_artifact_self_report_only",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-file-aliases":
    run_negative_control(
        "Reserved-lineage proof evidence file alias gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "lineage_proof_evidence_artifact_file_shape",
            "lineage_proof_evidence_artifact_file_alias_allowed",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-future-skew":
    run_negative_control(
        "Reserved-lineage proof evidence future-skew gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "lineage_proof_evidence_future_dated",
            "lineage_proof_evidence_allows_future_dated",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-output-aliases":
    run_negative_control(
        "Reserved-lineage proof evidence helper output alias gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            "--artifact-dir must not be a symlink",
            "--artifact-dir may be a symlink",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-output-ancestor":
    run_negative_control(
        "Reserved-lineage proof evidence helper output ancestor gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            "output_ancestor_errors = device_lab.validate_no_symlink_ancestors(",
            "output_ancestor_errors = _skip_output_ancestor_validation(",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-output-early-preflight":
    run_negative_control(
        "Reserved-lineage proof evidence helper early output preflight gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            'early_output_errors = preflight_output_path(out_path, "--out")',
            "early_output_errors = []",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-direct-secret-paths":
    run_negative_control(
        "Reserved-lineage proof evidence helper direct secret-path gates",
        lambda: (
            override_text(
                "scripts/kagemusha_lineage_proof_evidence.py",
                'secret_error = _secret_path_error(str(artifact_dir), "--artifact-dir")',
                "secret_error = None",
            ),
            override_text(
                "scripts/kagemusha_lineage_proof_evidence.py",
                "secret_error = _secret_path_error(str(path), label)",
                "secret_error = None",
            ),
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-direct-artifact-dir-secret-paths":
    run_negative_control(
        "Reserved-lineage proof evidence helper direct artifact-dir secret-path gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            'def validate_artifact_dir_path(artifact_dir: Path) -> list[str]:\n    """Reject artifact directories that could alias external release bytes."""\n\n    secret_error = _secret_path_error(str(artifact_dir), "--artifact-dir")\n    if secret_error is not None:\n        return [secret_error]\n',
            'def validate_artifact_dir_path(artifact_dir: Path) -> list[str]:\n    """Reject artifact directories that could alias external release bytes."""\n\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-direct-proof-log-secret-paths":
    run_negative_control(
        "Reserved-lineage proof evidence helper direct proof-log secret-path gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            '    proof_log_secret_error = _secret_path_error(str(proof_log), "--proof-log")\n    if proof_log_secret_error is not None:\n        errors.append(proof_log_secret_error)\n    if errors:\n        return errors\n',
            '    if errors:\n        return errors\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-direct-output-preflight-secret-paths":
    run_negative_control(
        "Reserved-lineage proof evidence helper direct output-preflight secret-path gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            'def preflight_output_path(path: Path, label: str) -> list[str]:\n    """Reject aliased output paths before evidence inputs are read."""\n\n    secret_error = _secret_path_error(str(path), label)\n    if secret_error is not None:\n        return [secret_error]\n',
            'def preflight_output_path(path: Path, label: str) -> list[str]:\n    """Reject aliased output paths before evidence inputs are read."""\n\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-validation-dir-aliases":
    run_negative_control(
        "Reserved-lineage proof evidence helper validation dir alias gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            "pre_create_dir_errors = validate_artifact_dir_path(artifact_dir)",
            "pre_create_dir_errors = []",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-input-corridor":
    run_negative_control(
        "Reserved-lineage proof evidence helper input corridor",
        lambda: (
            override_text(
                "scripts/kagemusha_lineage_proof_evidence.py",
                "errors = validate_lineage_input_paths(artifact_dir, proof_log)",
                "errors = []",
            ),
            override_text(
                "scripts/kagemusha_lineage_proof_evidence.py",
                "path_errors.extend(validate_lineage_input_paths(artifact_dir, proof_log))",
                "path_errors.extend([])",
            ),
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-command-canonical":
    run_negative_control(
        "Reserved-lineage proof evidence canonical command gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "must exactly match the canonical production Reserved-lineage proof command string",
            "canonical command spelling accepted",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-scalar-types":
    run_negative_control(
        "Reserved-lineage proof evidence scalar type gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "not isinstance(scalar_value, int)",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-timestamp-raw":
    run_negative_control(
        "Reserved-lineage proof evidence raw timestamp gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "generated_at_raw = generated_at_text",
            "generated_at_stripped = generated_at_text.strip()\n        generated_at_raw = generated_at_stripped",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-timestamp-raw":
    run_negative_control(
        "Reserved-lineage proof evidence helper raw timestamp gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            "errors.extend(_validate_generated_at_utc(generated_at_utc))",
            "errors.extend([])",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-log-exact":
    run_negative_control(
        "Reserved-lineage proof evidence exact proof-log gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "test_lines != [expected_test_line]",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-evidence-filename":
    run_negative_control(
        "Reserved-lineage proof evidence filename gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "lineage_proof_evidence_filename",
            "lineage_proof_evidence_any_filename",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-json-duplicate-keys":
    run_negative_control(
        "Kagemusha readiness duplicate JSON key gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "object_pairs_hook=_reject_duplicate_json_object_pairs",
            "object_pairs_hook=dict",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-closed-schema":
    run_negative_control(
        "Reserved-lineage proof evidence closed schema",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "lineage_proof_evidence_unexpected_field",
            "lineage_proof_evidence_allows_extra_fields",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-evidence-helper":
    run_negative_control(
        "Reserved-lineage proof evidence helper runtime-keygen guard",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            "validate_lineage_proof_command",
            "lineage_proof_command_disabled",
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
