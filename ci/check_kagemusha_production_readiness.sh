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
        "compact-token symbols now route one-hop",
        "Remaining compact-token release work is to attach signed device-lab evidence",
        "packaged one-hop and append proving-key artifacts",
        "receiver admission can trust that metadata",
        "SDK default selection",
        "production proof-log artifact",
        "ABI-7 recursive compact key evidence",
        "compact key evidence hash-binds the ABI-7 LEN=4",
        "artifact sizes",
        "compact key artifact byte sizes",
        "plain-text or all-zero placeholder compact key artifacts",
        "hashes and parses `recursive-compact-key-artifacts.log`",
        "canonical CLI summary line",
        "compact key generator log",
        "Reserved-lineage and compact key artifact size maps",
        "packaged lineage artifacts",
        "production proof logs",
        "release APKs",
        "D2D handoff transcripts",
        "wallet-integrity transcripts",
        "attestation certificate-chain files",
        "bundle-relative paths, SHA-256 digests, and byte sizes",
        "recursive-compact-key-evidence.json",
        "iroha.kagemusha.production_release_bundle.v1",
        "hash-bind the ready",
        "rejecting summary drift",
        "single expected proof",
        "one-test",
        "cargo result",
        "missing-vs-unreadable state",
        "Path.is_file()",
        "canonical `lineage-proof-evidence.json` filename",
        "duplicate JSON object keys rejected",
        "Android device-lab scanner also rejects duplicate JSON",
        "reuse the scanner-validated signed-evidence timestamp",
        "telemetry/status/runtime completion markers",
        "symlink-free ancestors before its",
        "symlink-ancestor `--repo-root` aliases",
        "symlink-free key-path ancestors",
        "secret-looking key path strings",
        "classifies slot directory",
        "unreadable signer slot/parent metadata",
        "scanner and rollup missing-root decisions consume",
        "`lstat()`-classified root presence",
        "classifies signer-controlled",
        "output parents with `lstat()`",
        "stops all readiness/evidence/device-lab",
        "`--out` cannot overwrite any",
        "hash-bound readiness summary",
        "verify existing manifests",
        "stable manifest comparison",
        "Path.is_dir()",
        "summary output parents",
        "scanner slot inventory classify",
        "recursive file-count entries",
        "automatic slot discovery classify",
        "preserve symlinked slot entries",
        "unreadable slot-entry metadata",
        "shared Android ancestor validation classify",
        "Path.is_symlink()` or `Path.exists()` preflight",
        "manifest artifact digest validation classify",
        "slot-relative",
        "Path.is_symlink()` preflight",
        "unreadable slot-root metadata",
        "scanner rejection of unreadable slot directory",
        "symlink artifact validator report unreadable",
        "slot-metadata, artifact-directory, and nested-artifact metadata",
        "regular-file artifact validator classify leaves",
        "before any `exists()` preflight",
        "hardlink and regular-file validators classify artifact",
        "regular-file validator classify nested artifacts before any",
        "`is_symlink()` preflight",
        "required-artifact shape checks, required status/runtime text reads",
        "the D2D queue digest binding, and",
        "signed-evidence artifact binding",
        "classify artifacts with `lstat()`",
        "before any `is_file()` preflight",
        "Direct slot-file discovery reports unreadable slot-root and",
        "artifact-directory metadata",
        "reject symlinked rollup summary output ancestors",
        "leaves and ancestors before creating missing `--out` parents",
        "classify `--out`",
        "release artifact/proof-log inputs",
    ),
    "docs/source/offline_kagemusha.md": (
        "The reserved `kagemusha-recursive-spend-lineage-v1` profile is the enabled",
        "witnessless chain-admission path for constant-size lineage proofs inside the",
        "64-hop cap",
        "The routine offline-offline production path",
        "uses the ABI-6 reserved-lineage recursive spend verifier and redemption surface",
        "ABI-7 recursive compact-token symbols now route one-hop",
        "LEN=4 compact-token proof path",
        "packaged compact one-hop and append proving-key archives",
        "default selection remain reserved ABI-7 state",
        "iroha app zk kagemusha recursive-compact-key-artifacts",
        "--record-out artifacts/kagemusha/recursive-compact-len4.record.norito",
        "--pk-out artifacts/kagemusha/recursive-compact-len4.pk",
        "--key-artifacts-out artifacts/kagemusha/recursive-compact-key-artifacts.norito",
        "--verifier-keys-out artifacts/kagemusha/recursive-compact-verifier-keys.norito",
        "--record-out artifacts/kagemusha/lineage-init-len128.record.norito",
        "--record-out artifacts/kagemusha/lineage-append-len128.record.norito",
        "iroha app zk kagemusha lineage-record",
        "--vk artifacts/kagemusha/lineage-init-len128.vk",
        "--vk artifacts/kagemusha/lineage-append-len128.vk",
        "governance/WSV `VerifyingKeyRecord` bound to `offline_kagemusha`",
        "`--record-namespace` and `--record-version`",
        "lineage-proof-evidence.json` and",
        "recursive-compact-key-evidence.json` to sit beside these",
        "canonical `lineage-proof-evidence.json` and",
        "`recursive-compact-key-evidence.json` filenames are part of the release packet",
        "renamed, copied, symlinked, or symlink-ancestor evidence JSON files",
        "byte sizes",
        "recomputes each digest",
        "artifact size",
        "requires lineage and compact key artifacts to be",
        "All-zero Reserved-lineage artifacts and plain-text or all-zero placeholder",
        "recursive-compact-key-artifacts.log",
        "canonical CLI summary line",
        "compact key generator log",
        "captured `record-archive-proof.log`",
        "missing-vs-unreadable state",
        "Path.is_file()",
        "hashes and parses the local proof log",
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
        "python3 scripts/kagemusha_recursive_compact_key_evidence.py",
        "tee artifacts/kagemusha/recursive-compact-key-artifacts.log",
        "--generator-log artifacts/kagemusha/recursive-compact-key-artifacts.log",
        "python3 scripts/kagemusha_release_bundle.py",
        "--repo-root .",
        "--proof-log artifacts/kagemusha/record-archive-proof.log",
        "--out artifacts/kagemusha/recursive-compact-key-evidence.json",
        "dist/kagemusha-production-release-bundle.json",
        "iroha.kagemusha.production_release_bundle.v1",
        "recomputes the checked-in ABI-6",
        "per-slot Android signed-evidence artifact paths",
        "every packaged lineage artifact",
        "compact key artifact",
        "production proof",
        "release APK",
        "D2D handoff transcript",
        "wallet-integrity transcript",
        "attestation certificate-chain",
        "missing or digest-drifted Android release APK, D2D handoff",
        "wallet-integrity, and attestation-chain artifacts",
        "rejects summary drift",
        "extra release claims",
        "are",
        "rejected instead of ignored",
        "duplicate JSON object keys are also invalid",
        "last-key-wins evidence packets",
        "stops before loading any readiness JSON",
        "`--out` cannot",
        "already hash-bound into the manifest",
        "--verify-existing dist/kagemusha-production-release-bundle.json",
        "stable manifest comparison",
        "future-dated beyond the release validator",
        "clock-skew allowance, remains blocked",
        "timestamp must use canonical UTC",
        "helper rejects noncanonical `--generated-at-utc`",
        "normalizing them into",
        "symlink-ancestor output aliases",
        "checked-in ABI-6 manifest plus ABI-7",
        "symlink-free ancestors",
        "reading compact key artifacts",
        "recorded proof",
        "commands with secret-looking material",
        "echoing the secret value",
        "unreadable slot or parent metadata",
        "Scanner and rollup missing-root decisions",
        "`lstat()`-classified root presence",
        "before metadata-derived output paths",
        "classifies signer-controlled output parents",
        "Path.is_dir()",
        "summary output parents",
        "Scanner slot inventory",
        "presence and file-count fields",
        "Automatic slot discovery",
        "fail-closed `scan_slot(...)`",
        "unreadable slot-entry metadata",
        "classify `--out`",
        "unreadable slot-root metadata",
        "scan_slot(...) rejects unreadable slot directory or parent metadata",
        "symlink validator now reports unreadable",
        "slot-metadata, artifact-directory, and nested-artifact metadata",
        "regular-file validator classifies leaves",
        "before any `exists()` preflight",
        "Hardlink and",
        "regular-file validators also classify artifact directories",
        "nested artifacts before any `is_symlink()` preflight",
        "Required-artifact shape",
        "required status/runtime text reads",
        "signed-evidence artifact binding",
        "also classify artifacts with `lstat()`",
        "before any `is_file()` preflight",
        "Direct slot-file discovery reports unreadable slot-root and",
        "artifact-directory metadata",
        "Android freshness checks consume the",
        "scanner-validated signed-evidence timestamp",
        "readiness summary writer also rejects symlinked `--summary-out` ancestors",
        "shared Android device-lab JSON loader",
        "aliased directories",
        "Shared Android ancestor validation",
        "Path.is_symlink()` or `Path.exists()` preflights",
        "Manifest artifact digest validation",
        "slot-relative ancestor directories",
        "symlinked `--repo-root` directories",
        "repo-root ancestors",
    ),
    "docs/source/sdk/android/readiness/android_strongbox_device_matrix.md": (
        "Android StrongBox Offline Payments Device Matrix",
        "Last updated: 2026-06-07",
        "ABI 6 recursive spend JNI probes pass on every required device family.",
        "ABI 7 recursive compact-token JNI probes prove and verify the packaged",
        "one-hop LEN=4 path on every required device family.",
        "ABI 7 recursive compact prover calls that require multi-hop append-batch",
        "composition produce package-backed compact tokens when the key package is",
        "supplied, while empty, malformed, or dummy-proof local archives remain",
        "caller-input errors or soft-invalid verifier results",
        "Lab reports include raw test commands, device fingerprints, OS build IDs, and",
        "connectedAndroidTest",
        "org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProverTest",
        "OfflineNoteTransferHandoff",
        "python3 scripts/check_android_device_lab_slot.py --root artifacts/android/device_lab --require-slot --require-kagemusha-production-evidence --require-kagemusha-standard-matrix --trusted-signer-public-key",
        "single safe slot",
        "python3 scripts/sign_android_device_lab_evidence.py --slot artifacts/android/device_lab/<slot-id> --private-key",
        "python3 scripts/kagemusha_production_readiness.py --device-lab-root artifacts/android/device_lab --trusted-signer-public-key",
        "python3 scripts/kagemusha_release_bundle.py",
        "--repo-root .",
        "dist/kagemusha-production-release-bundle.json",
        "iroha.kagemusha.production_release_bundle.v1",
        "recomputes the checked-in",
        "per-slot Android signed-evidence artifact paths",
        "rejects summary drift",
        "--max-compact-key-evidence-future-skew-seconds 300",
        "Reserved-lineage proof evidence",
        "ABI-7 recursive compact key evidence",
        "canonical",
        "`lineage-proof-evidence.json` filename",
        "`recursive-compact-key-evidence.json` filename",
        "recomputes the compact key",
        "recursive-compact-key-artifacts.log",
        "canonical CLI summary line",
        "byte sizes",
        "size-mismatched",
        "placeholder compact key evidence",
        "same placeholder-artifact and generator-log rejection",
        "renamed or copied evidence",
        "The rollup recomputes their SHA-256 digests and",
        "local bytes",
        "non-symlink",
        "non-hardlinked files",
        "missing-vs-unreadable state",
        "Path.is_file()",
        "symlink-free ancestors",
        "symlinked output ancestors plus symlinked, hardlinked, or non-regular",
        "record-archive-proof.log",
        "re-checks the proof log's passing cargo",
        "one expected `test ... ok` line",
        "signer slot preflight also classifies the slot directory and parent",
        "unreadable slot or parent metadata fails closed",
        "output parents with `lstat()`",
        "Path.is_dir()",
        "summary output parents",
        "Scanner slot inventory",
        "summary presence/count fields",
        "Automatic slot discovery",
        "fail-closed",
        "unreadable slot-entry metadata",
        "classifies `--out`",
        "unreadable slot-root metadata",
        "scanner also rejects unreadable slot directory or parent metadata",
        "symlink validator now reports unreadable",
        "slot-metadata, artifact-directory, and nested-artifact metadata",
        "regular-file validator classifies leaves",
        "before any `exists()` preflight",
        "Hardlink and regular-file validators also classify artifact directories",
        "nested artifacts before any `is_symlink()` preflight",
        "Manifest artifact digest validation",
        "slot-relative ancestor",
        "Path.is_symlink()",
        "Required-artifact shape checks, required status/runtime text reads, the",
        "D2D queue digest binding, and the",
        "signed-evidence artifact binding also",
        "before any `is_file()` preflight",
        "Direct slot-file discovery reports unreadable slot-root and",
        "artifact-directory metadata",
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
        "Scanner and rollup missing-root decisions",
        "`lstat()`-classified root presence",
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
        "`--repo-root` must",
        "existing non-symlink directory",
        "Shared ancestor validation",
        "Path.is_symlink()` or `Path.exists()` preflights",
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
        "ED25519_SIGNATURE_BYTES = 64",
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
        "ABI7_RECURSIVE_COMPACT_ONE_HOP_JNI_PROBE_STATES",
        '"one_hop_verified"',
        "ABI7_RECURSIVE_COMPACT_MULTI_HOP_PROVER_STATES",
        '"multi_hop_proof_composed"',
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
        "classify_device_lab_root_path",
        "root_exists, root_errors = classify_device_lab_root_path(root)",
        "if not root_exists:",
        '    try:\n        root_mode = root.lstat().st_mode\n    except FileNotFoundError:\n        root_mode = None\n    except OSError:\n        return False, ["device-lab root metadata could not be read"]\n',
        "root_mode is not None and stat.S_ISLNK(root_mode)",
        "root_mode is not None and not stat.S_ISDIR(root_mode)",
        "validate_no_symlink_ancestors",
        "slot_ids, slot_id_errors = validate_slot_ids(args.slots)",
        "slot_paths, discovery_errors = discover_slots(root, slot_ids)",
        "device-lab root could not be listed",
        "entries = list(root.iterdir())",
        "entry_mode = entry.lstat().st_mode",
        "device-lab slot directory metadata could not be read",
        "if stat.S_ISDIR(entry_mode) or stat.S_ISLNK(entry_mode):",
        "validate_summary_output_path",
        '    parent_exists, parent_errors = _validate_summary_output_parent(path, label)\n    if parent_errors:\n        return parent_errors\n    ancestor_errors = validate_no_symlink_ancestors(\n',
        '        f"{label} ancestor directory",\n    )\n    if ancestor_errors:\n        return ancestor_errors\n    if not parent_exists:\n',
        '    parent_exists, parent_errors = _validate_summary_output_parent(\n        path,\n        label,\n        missing_error=f"{label} parent must be a directory",\n    )\n    if parent_errors:\n        return parent_errors\n    if not parent_exists:\n        return [f"{label} parent must be a directory"]\n',
        'def _validate_summary_output_parent(\n    path: Path,\n    label: str,\n    *,\n    missing_error: str | None = None,\n) -> tuple[bool, list[str]]:\n    """Classify a scanner summary output parent without following aliases."""\n\n    parent = path.parent\n    try:\n        parent_mode = parent.lstat().st_mode\n    except FileNotFoundError:\n        if missing_error is None:\n            return False, []\n        return False, [missing_error]\n    except OSError:\n        return False, [f"{label} parent directory metadata could not be read"]\n',
        '    if stat.S_ISLNK(parent_mode):\n        return True, [f"{label} parent directory must not be a symlink"]\n    if not stat.S_ISDIR(parent_mode):\n        return True, [f"{label} parent must be a directory"]\n    return True, []\n',
        "_slot_tree_entries",
        'f"{label} could not be listed"',
        "slot id {_display_path(slot_id)!r} must be a single safe directory name",
        'if SECRET_RE.search(str(root)):\n        return False, ["device-lab root path must not contain secret-looking material"]',
        "DuplicateJsonKeyError",
        "_reject_duplicate_json_object_pairs",
        "object_pairs_hook=_reject_duplicate_json_object_pairs",
        "contains duplicate JSON object key",
        "validate_no_slot_symlink_artifacts",
        'def validate_no_slot_symlink_artifacts(slot_path: Path, errors: list[str]) -> None:\n    """Reject symlinked slot metadata, directories, and evidence artifacts."""\n\n    if _reject_secret_slot_path(slot_path, errors):\n        return\n',
        '        try:\n            mode = path.lstat().st_mode\n        except FileNotFoundError:\n            continue\n        except OSError:\n            _append_error_once(errors, f"{relative} file metadata could not be read")\n            continue\n        if stat.S_ISLNK(mode):\n            errors.append(f"{relative} must not be a symlink")\n',
        '        try:\n            dir_mode = dir_path.lstat().st_mode\n        except FileNotFoundError:\n            continue\n        except OSError:\n            _append_error_once(errors, f"{dirname}/ metadata could not be read")\n            continue\n        if stat.S_ISLNK(dir_mode):\n            errors.append(f"{dirname}/ must not be a symlink")\n',
        '            try:\n                entry_mode = entry.lstat().st_mode\n            except OSError:\n                _append_error_once(\n                    errors,\n                    f"slot artifact {_display_path(relative)} file metadata could not be read",\n                )\n                continue\n            if stat.S_ISLNK(entry_mode):\n',
        "validate_slot_regular_file_artifacts",
        'def validate_slot_regular_file_artifacts(slot_path: Path, errors: list[str]) -> None:\n    """Reject special-file slot metadata and evidence artifacts."""\n\n    if _reject_secret_slot_path(slot_path, errors):\n        return\n',
        "validate_no_slot_hardlink_artifacts",
        'def validate_no_slot_hardlink_artifacts(slot_path: Path, errors: list[str]) -> None:\n    """Reject hardlinked slot metadata and evidence artifacts."""\n\n    if _reject_secret_slot_path(slot_path, errors):\n        return\n',
        'def _reject_hardlinked_file(path: Path, label: str, errors: list[str]) -> None:\n    try:\n        mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return\n    except OSError:\n        errors.append(f"{label} file metadata could not be read")\n        return\n    if stat.S_ISLNK(mode) or not stat.S_ISREG(mode):\n        return\n',
        'def _reject_non_regular_file(path: Path, label: str, errors: list[str]) -> None:\n    try:\n        mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return\n    except OSError:\n        errors.append(f"{label} file metadata could not be read")\n        return\n    if stat.S_ISLNK(mode):\n        return\n    if not stat.S_ISREG(mode):\n        errors.append(f"{label} must be a regular file")\n',
        "slot directory name must not contain secret-looking material",
        'def _reject_secret_slot_path(slot_path: Path, errors: list[str]) -> bool:\n    """Reject direct helper calls that receive secret-looking slot paths."""\n\n    if SECRET_RE.search(str(slot_path)):\n        errors.append("slot path must not contain secret-looking material")\n        return True\n    return False\n',
        "_reject_secret_slot_path(slot_path, errors)",
        '    try:\n        slot_mode = slot_path.lstat().st_mode\n    except FileNotFoundError:\n        slot_mode = None\n    except OSError:\n        return {\n            "slot": slot_label,\n            "status": "error",\n            "errors": ["slot directory metadata could not be read"],\n            "present": present,\n            "file_counts": file_counts,\n            "kagemusha": {"required": require_kagemusha_production_evidence},\n        }\n',
        "directory_present, directory_missing = _slot_expected_directory_present(",
        "count = _slot_regular_file_count(slot_path, entries, errors)",
        'present["sha256sum.txt"] = _slot_regular_file_present(',
        "def _slot_expected_directory_present(",
        '    try:\n        dir_mode = dir_path.lstat().st_mode\n    except FileNotFoundError:\n        return False, True\n    except OSError:\n        _append_error_once(errors, f"{dirname}/ metadata could not be read")\n        return False, False\n    if stat.S_ISLNK(dir_mode) or not stat.S_ISDIR(dir_mode):\n        return False, False\n    return True, False\n',
        "def _slot_regular_file_count(",
        '        try:\n            entry_mode = entry.lstat().st_mode\n        except OSError:\n            _append_error_once(\n                errors,\n                f"slot artifact {_display_path(relative)} file metadata could not be read",\n            )\n            continue\n        if stat.S_ISREG(entry_mode):\n            count += 1\n',
        "def _slot_regular_file_present(",
        '    try:\n        mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return False\n    except OSError:\n        _append_error_once(errors, f"{label} file metadata could not be read")\n        return False\n    return stat.S_ISREG(mode)\n',
        'def _validate_manifest_slot_path(slot_path: Path) -> list[str]:\n    if SECRET_RE.search(str(slot_path)):\n        return ["slot path must not contain secret-looking material"]\n    try:\n        slot_mode = slot_path.lstat().st_mode\n    except FileNotFoundError:\n        slot_mode = None\n    except OSError:\n        return ["slot directory metadata could not be read"]\n    if slot_mode is not None and stat.S_ISLNK(slot_mode):\n        return ["slot directory must not be a symlink"]\n    return validate_no_symlink_ancestors(slot_path, "slot ancestor directory")\n',
        'root_errors = _validate_manifest_slot_path(slot_path)\n    if root_errors:\n        return entries, root_errors\n',
        'root_errors = _validate_manifest_slot_path(slot_path)\n    if root_errors:\n        return root_errors\n',
        '        try:\n            candidate = Path.cwd() / path\n        except OSError:\n            return [f"{label} metadata could not be read"]\n',
        "ancestor_mode = ancestor.lstat().st_mode",
        "except FileNotFoundError:\n            continue",
        "if stat.S_ISLNK(ancestor_mode):",
        '    try:\n        manifest_stat = manifest_path.lstat()\n    except FileNotFoundError:\n        return entries, ["missing sha256sum.txt"]\n    except OSError:\n        return entries, ["sha256sum.txt file metadata could not be read"]\n',
        "if stat.S_ISLNK(manifest_stat.st_mode):",
        "if not stat.S_ISREG(manifest_stat.st_mode):",
        'if manifest_path.stat().st_nlink > 1:\n            return entries, ["sha256sum.txt must not be hardlinked"]\n',
        'with manifest_path.open("rb") as handle:',
        "open_stat = os.fstat(handle.fileno())",
        "expected_identity = (manifest_stat.st_dev, manifest_stat.st_ino)",
        "open_identity = (open_stat.st_dev, open_stat.st_ino)",
        "sha256sum.txt changed while being read",
        'lines = payload.decode("utf-8").splitlines()',
        '    except (OSError, UnicodeDecodeError):\n        return entries, ["sha256sum.txt could not be read"]\n',
        "sha256sum.txt could not be read",
        "def _has_manifest_file_shape_error(errors: list[str]) -> bool:",
        "if _has_manifest_file_shape_error(errors):",
        'def _slot_root_entries(slot_path: Path, errors: list[str]) -> list[Path] | None:\n    try:\n        return list(slot_path.iterdir())\n    except OSError:\n        _append_error_once(errors, "slot directory could not be listed")\n        return None\n',
        "def _record_manifest_inventory_entry(",
        '    try:\n        mode = entry.lstat().st_mode\n    except OSError:\n        _append_error_once(\n            errors,\n            f"slot artifact {_display_path(relative)} file metadata could not be read",\n        )\n        return\n    if stat.S_ISREG(mode) or stat.S_ISLNK(mode):\n        files.add(relative)\n',
        'def _slot_files(slot_path: Path, errors: list[str] | None = None) -> set[str]:\n    slot_errors = errors if errors is not None else []\n    try:\n        slot_mode = slot_path.lstat().st_mode\n    except FileNotFoundError:\n        return set()\n    except OSError:\n        _append_error_once(slot_errors, "slot directory metadata could not be read")\n        return set()\n    if stat.S_ISLNK(slot_mode) or not stat.S_ISDIR(slot_mode):\n        return set()\n',
        "slot directory could not be listed",
        'if SECRET_RE.search(str(slot_path)):\n        return set()\n',
        'if validate_no_symlink_ancestors(slot_path, "slot ancestor directory"):\n        return set()\n',
        "slot_errors = errors if errors is not None else []",
        '        try:\n            dir_mode = dir_path.lstat().st_mode\n        except FileNotFoundError:\n            continue\n        except OSError:\n            _append_error_once(slot_errors, f"{dirname}/ metadata could not be read")\n            continue\n        if stat.S_ISLNK(dir_mode) or not stat.S_ISDIR(dir_mode):\n            continue\n        entries = _slot_tree_entries(dir_path, f"{dirname}/", slot_errors)',
        'entries = _slot_tree_entries(dir_path, f"{dirname}/", slot_errors)',
        "root_entries = _slot_root_entries(slot_path, slot_errors)",
        'skipped_roots = {"sha256sum.txt", *EXPECTED_DIRS, *OPTIONAL_EVIDENCE_DIRS}',
        '    try:\n        mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return\n    except OSError:\n        errors.append(f"{label} file metadata could not be read")\n        return\n',
        '        try:\n            dir_mode = dir_path.lstat().st_mode\n        except FileNotFoundError:\n            continue\n        except OSError:\n            _append_error_once(errors, f"{dirname}/ metadata could not be read")\n            continue\n        if stat.S_ISLNK(dir_mode) or not stat.S_ISDIR(dir_mode):\n            continue\n',
        '        try:\n            mode = dir_path.lstat().st_mode\n        except FileNotFoundError:\n            continue\n        except OSError:\n            errors.append(f"{dirname}/ metadata could not be read")\n            continue\n        if stat.S_ISLNK(mode):\n            continue\n',
        '        for entry in entries:\n            relative = entry.relative_to(slot_path).as_posix()\n            try:\n                entry_mode = entry.lstat().st_mode\n            except OSError:\n                errors.append(\n                    f"slot artifact {_display_path(relative)} file metadata could not be read"\n                )\n                continue\n            if stat.S_ISLNK(entry_mode):\n                continue\n            if stat.S_ISDIR(entry_mode):\n',
        "def _slot_relative_symlink_ancestor(slot_path: Path, relative: str) -> str | None:",
        "current_mode = current.lstat().st_mode",
        "if stat.S_ISLNK(current_mode):",
        "def _slot_artifact_lstat_mode(",
        '    try:\n        return artifact_path.lstat().st_mode, []\n    except FileNotFoundError:\n        return None, []\n    except OSError:\n        return None, [metadata_error]\n',
        "def _validate_manifest_artifact_for_digest(",
        "def _manifest_artifact_sha256(",
        "def _read_validated_manifest_artifact_bytes(",
        "manifest_expected_identity = (\n                expected_stat.st_dev,\n                expected_stat.st_ino,\n            )",
        "sha256sum.txt references artifact changed while being read",
        'artifact_path, artifact_stat, errors = _validate_manifest_artifact_for_digest(\n        slot_path,\n        relative,\n    )',
        "sha256sum.txt references artifact that could not be read",
        "sha256sum.txt references artifact file metadata could not be read",
        '    display = _display_path(safe_relative)\n    artifact_path = slot_path / safe_relative\n    if _slot_relative_symlink_ancestor(slot_path, safe_relative) is not None:\n        return None, None, [\n            "sha256sum.txt references artifact under symlink directory "\n            f"{display}"\n        ]\n',
        "actual_files = _slot_files(slot_path, errors)",
        "actual_digest, digest_errors = _manifest_artifact_sha256(slot_path, relative)",
        "sha256sum.txt references artifact under symlink directory",
        "sha256sum.txt references symlink artifact",
        "sha256sum.txt references non-regular artifact",
        "sha256sum.txt references hardlinked artifact",
        "device-lab root must not be a symlink",
        "device-lab root ancestor directory",
        "slot directory metadata could not be read",
        "slot directory must not be a symlink",
        "slot parent directory metadata could not be read",
        "slot parent directory must not be a symlink",
        "slot ancestor directory",
        "must not be a symlink",
        "must be a regular file",
        "must not be hardlinked",
        "slot.json contains unexpected field",
        "_verify_ed25519_signature",
        "_write_staged_bytes",
        'with path.open("xb") as handle:',
        '        with path.open("xb") as handle:\n            handle.write(payload)\n            handle.flush()\n            os.fsync(handle.fileno())\n',
        "handle.flush()",
        "os.fsync(handle.fileno())",
        "staged_stat = os.fstat(handle.fileno())",
        "_read_staged_bytes",
        'with path.open("rb") as handle:',
        "staged_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        "staged_open_identity = (open_stat.st_dev, open_stat.st_ino)",
        "readback, readback_errors = _read_staged_bytes(",
        'def _verify_ed25519_signature(\n    *,\n    public_key_path: Path,\n    payload: bytes,\n    signature: bytes,\n    errors: list[str],\n    label: str = "trusted signer public key",\n) -> None:\n    if not _validate_public_key_path_shape(public_key_path, errors=errors, label=label):\n        return\n    openssl = _require_openssl(errors)\n',
        "signature verification staging files could not be written",
        "signature verification staged payload did not match input",
        "signature verification staged signature did not match input",
        "signature verification temporary directory could not be created",
        "covered_device_families",
        "missing_device_families",
        "trusted_signer_public_key_sha256",
        'if not _validate_public_key_path_shape(public_key_path, errors=errors, label=label):\n        return None\n    openssl = _require_openssl(errors)\n',
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
        "def _validate_metadata_artifact_for_read(",
        "def _metadata_artifact_bytes_and_sha256(",
        "def _metadata_artifact_text(",
        "def _read_validated_metadata_artifact_bytes(",
        "expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        'f"{label} references artifact changed while being read {display}"',
        'artifact_path, artifact_stat, errors = _validate_metadata_artifact_for_read(\n        slot_path,\n        relative,\n        label,\n        missing_error,\n    )',
        'artifact_bytes, read_errors = _read_validated_metadata_artifact_bytes(',
        "def _should_read_optional_text_artifact(",
        '    mode, mode_errors = _slot_artifact_lstat_mode(\n        slot_path / relative,\n        f"{label} file metadata could not be read",\n    )\n    if mode_errors:\n        errors.extend(mode_errors)\n        return False\n    if mode is None:\n        return False\n    return stat.S_ISLNK(mode) or stat.S_ISREG(mode)\n',
        '    if not _should_read_optional_text_artifact(\n        slot_path,\n        "telemetry/status.ndjson",\n        "telemetry/status.ndjson",\n        errors,\n    ):\n        return\n',
        'text, read_errors = _metadata_artifact_text(\n        slot_path,\n        "telemetry/status.ndjson",\n        "telemetry/status.ndjson",\n        "telemetry/status.ndjson required artifact is missing",\n        "telemetry/status.ndjson could not be read",\n    )',
        '    if not _should_read_optional_text_artifact(\n        slot_path,\n        "logs/runtime.log",\n        "logs/runtime.log",\n        errors,\n    ):\n        return\n',
        'text, read_errors = _metadata_artifact_text(\n        slot_path,\n        "logs/runtime.log",\n        "logs/runtime.log",\n        "logs/runtime.log required artifact is missing",\n        "logs/runtime.log could not be read",\n        decode_errors="replace",\n    )',
        "chain_bytes, actual_chain_digest, digest_errors =",
        "_, actual_apk_digest, digest_errors = _metadata_artifact_bytes_and_sha256(",
        "_, actual_digest, digest_errors = _metadata_artifact_bytes_and_sha256(",
        '    if stat.S_ISLNK(artifact_stat.st_mode):\n        return None, None, [f"{label} references symlink artifact {display}"]\n    if not stat.S_ISREG(artifact_stat.st_mode):\n        return None, None, [f"{label} references non-regular artifact {display}"]\n',
        "references hardlinked artifact",
        "d2d_payment_transcript_path",
        "d2d_payment_transcript_sha256",
        '"slot.json d2d_payment_transcript_path",',
        '"slot.json d2d_payment_transcript_path must point to an existing file",',
        '"d2d payment transcript queue_after_sha256",',
        '"d2d payment transcript queue_after_sha256 requires queue/pending_queue.json",',
        "wallet_integrity_transcript_path",
        "wallet_integrity_transcript_sha256",
        '"slot.json wallet_integrity_transcript_path",',
        '"slot.json wallet_integrity_transcript_path must point to an existing file",',
        '_, actual_digest, digest_errors = _metadata_artifact_bytes_and_sha256(\n        slot_path,\n        relative,\n        "slot.json d2d_payment_transcript_path",\n        "slot.json d2d_payment_transcript_path must point to an existing file",\n    )',
        '_, actual_digest, digest_errors = _metadata_artifact_bytes_and_sha256(\n        slot_path,\n        relative,\n        "slot.json wallet_integrity_transcript_path",\n        "slot.json wallet_integrity_transcript_path must point to an existing file",\n    )',
        '    _, actual_queue_digest, digest_errors = _metadata_artifact_bytes_and_sha256(\n        slot_path,\n        "queue/pending_queue.json",\n        "d2d payment transcript queue_after_sha256",\n        "d2d payment transcript queue_after_sha256 requires queue/pending_queue.json",\n    )\n    if digest_errors:\n        errors.extend(digest_errors)\n    elif (\n        actual_queue_digest is not None\n        and queue_after_sha256 is not None\n',
        "native_bridge_abi_version",
        "slot.json offline_wallet_apk_sha256 does not match offline_wallet_apk_path",
        '        mode, mode_errors = _slot_artifact_lstat_mode(\n            artifact_path,\n            f"required slot artifact metadata could not be read {relative}",\n        )\n        if mode_errors:\n            errors.extend(mode_errors)\n            continue\n        if mode is None or stat.S_ISLNK(mode) or not stat.S_ISREG(mode):\n            continue\n',
        '        try:\n            artifact_size = artifact_path.stat().st_size\n        except OSError:\n            errors.append(f"required slot artifact metadata could not be read {relative}")\n            continue\n',
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
        '        _, actual_digest, digest_errors = _metadata_artifact_bytes_and_sha256(\n            slot_path,\n            artifact_relative,\n            "slot.json signed_evidence_artifact_path",\n            "slot.json signed_evidence_artifact_path must point to an existing file",\n        )\n        if digest_errors:\n            errors.extend(digest_errors)\n        elif (\n            actual_digest is not None\n            and digest is not None\n',
        "signed_evidence_artifact_sha256 does not match signed_evidence_artifact_path",
        "signed evidence artifact contains unexpected field {_display_path(field)}",
        "signer_public_key_sha256",
        "signature_payload_sha256",
        "trusted signer public key required for Kagemusha production evidence",
        "signed evidence artifact signature verification failed",
        "must be a valid OpenSSL public key",
        '    except subprocess.CalledProcessError:\n        errors.append(f"{label} must be a valid OpenSSL public key")\n        return None\n',
        "OpenSSL public key command could not be run",
        "signature verification command could not be run",
        "signed evidence artifact digest mismatch for",
        "signed evidence artifact artifact_digests",
        "def _validate_signed_evidence_artifact_for_digest(",
        "def _signed_evidence_artifact_sha256(",
        "def _read_validated_signed_evidence_artifact_bytes(",
        "signed evidence artifact digest references artifact that could not be read",
        "signed evidence artifact digest references artifact changed",
        "signed_evidence_expected_identity = (\n                expected_stat.st_dev,\n                expected_stat.st_ino,\n            )",
        'artifact_path, artifact_stat, errors = _validate_signed_evidence_artifact_for_digest(\n        slot_path,\n        relative,\n    )\n    if errors:\n        return None, errors\n',
        'payload, read_errors = _read_validated_signed_evidence_artifact_bytes(',
        "actual_digest, digest_errors = _signed_evidence_artifact_sha256(",
        "signed evidence artifact digest references symlink artifact",
        "signed evidence artifact digest references hardlinked artifact",
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
        "_validate_public_key_path_shape",
        'label: str = "trusted signer public key"',
        'f"{label} ancestor directory"',
        '    try:\n        public_key_mode = public_key_path.lstat().st_mode\n    except FileNotFoundError:\n        public_key_mode = None\n    except OSError:\n        errors.append(f"{label} file metadata could not be read")\n        return False\n    if public_key_mode is not None and stat.S_ISLNK(public_key_mode):\n        errors.append(f"{label} must not be a symlink")\n        return False\n',
        '    if public_key_mode is None:\n        errors.append(f"{label} must point to an existing public key file")\n        return False\n    if not stat.S_ISREG(public_key_mode):\n        errors.append(f"{label} must be a regular file")\n        return False\n',
        '    try:\n        link_count = public_key_path.stat().st_nlink\n    except OSError:\n        errors.append(f"{label} hardlink metadata could not be read")\n        return False\n',
        "slot directory missing",
        "validate_summary_output_path",
        "write_errors = write_summary",
        '        try:\n            parent.mkdir(parents=True, exist_ok=True)\n        except OSError:\n            return [f"{label} parent directory could not be created"]\n',
        "tempfile.NamedTemporaryFile(",
        "handle.flush()",
        "os.fsync(handle.fileno())",
        "os.replace(tmp_path, path)",
        "_read_summary_output_text",
        "summary_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        "summary_open_identity = (open_stat.st_dev, open_stat.st_ino)",
        "--json-out changed while being read",
        "readback_text, readback_errors = _read_summary_output_text(path, expected_stat)",
        "readback_text != summary_text",
        "--json-out write verification failed",
        '    except OSError:\n        return None, ["--json-out write verification failed"]\n',
        '    errors = validate_summary_output_path(path, "--json-out")\n    if errors:\n        return errors\n    try:\n        expected_stat = path.lstat()\n',
        '    try:\n        output_mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return []\n    except OSError:\n        return [f"{label} file metadata could not be read"]\n',
        "if stat.S_ISLNK(output_mode):",
        "if not stat.S_ISREG(output_mode):",
        '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        return [f"{label} hardlink metadata could not be read"]\n',
        'if SECRET_RE.search(str(path)):\n        return [f"{label} must not contain secret-looking material"]',
        'return [f"{label} must not be a symlink"]',
        'return [f"{label} must not be hardlinked"]',
        'def _load_json(path: Path, label: str, errors: list[str]) -> dict[str, Any] | None:\n    if SECRET_RE.search(str(path)):\n        errors.append(f"{label} path must not contain secret-looking material")\n        return None\n',
        "json_ancestor_errors = validate_no_symlink_ancestors(",
        'f"{label} ancestor directory"',
        '    try:\n        expected_stat = path.lstat()\n    except FileNotFoundError:\n        errors.append(f"missing {label}")\n        return None\n    except OSError:\n        errors.append(f"{label} file metadata could not be read")\n        return None\n',
        "json_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        "json_open_identity = (open_stat.st_dev, open_stat.st_ino)",
        "json_path_stat = path.lstat()",
        'errors.append(f"{label} changed while being read")',
        'data = _loads_json_without_duplicate_keys(b"".join(chunks).decode("utf-8"))',
        'except (OSError, UnicodeDecodeError):\n        errors.append(f"{label} could not be read")\n        return None',
    ),
    "scripts/sign_android_device_lab_evidence.py": (
        "Build and sign Kagemusha Android device-lab evidence artifacts",
        "import stat",
        "DEFAULT_SIGNED_EVIDENCE_PATH",
        "device_lab.KAGEMUSHA_SIGNED_EVIDENCE_ARTIFACT_PATH",
        "device_lab._load_json",
        "device_lab._canonical_signed_evidence_payload",
        "device_lab.SIGNED_EVIDENCE_SLOT_INT_FIELDS",
        "slot.json native_bridge_abi_version must be an integer",
        "private key did not produce a signature accepted by the signer public key",
        "_secret_key_path_error",
        "if device_lab.SECRET_RE.search(str(path)):",
        "path must not contain secret-looking material",
        "slot path must not contain secret-looking material",
        'def _sign_ed25519(private_key_path: Path, payload: bytes, errors: list[str]) -> bytes | None:\n    secret_error = _secret_key_path_error(private_key_path, "private key")\n',
        '    try:\n        private_key_mode = private_key_path.lstat().st_mode\n    except FileNotFoundError:\n        private_key_mode = None\n    except OSError:\n        errors.append("private key file metadata could not be read")\n        return None\n    if private_key_mode is not None and stat.S_ISLNK(private_key_mode):\n        errors.append("private key must not be a symlink")\n        return None\n',
        "signature payload could not be staged",
        "device_lab._write_staged_bytes",
        "signature payload staging verification failed",
        '            except subprocess.CalledProcessError:\n                errors.append("private key must be a valid OpenSSL Ed25519 private key")\n                return None\n',
        "signature command could not be run",
        '            except OSError:\n                errors.append("signature command could not be run")\n                return None\n',
        "signature temporary directory could not be created",
        "_read_signature_output",
        "signature output could not be read",
        '    except OSError:\n        errors.append("signature output could not be read")\n        return None\n    return b"".join(chunks)\n',
        "signature_output_expected_identity = (",
        "signature_output_expected_identity = (\n        expected_stat.st_dev,\n        expected_stat.st_ino,\n    )",
        "signature_output_open_identity = (open_stat.st_dev, open_stat.st_ino)",
        "signature output must be 64 bytes",
        "len(signature) != device_lab.ED25519_SIGNATURE_BYTES",
        'if verify_errors == ["signed evidence artifact signature verification failed"]:\n        errors.append(\n            "private key did not produce a signature accepted by the signer public key"\n        )\n    elif verify_errors:\n        errors.extend(verify_errors)\n',
        "private key must not be a symlink",
        "private key ancestor directory",
        '    try:\n        link_count = private_key_path.stat().st_nlink\n    except OSError:\n        errors.append("private key hardlink metadata could not be read")\n        return None\n',
        "private key must not be hardlinked",
        '    if private_key_mode is None:\n        errors.append("private key must point to an existing file")\n        return None\n    if not stat.S_ISREG(private_key_mode):\n        errors.append("private key must be a regular file")\n        return None\n',
        "signed evidence output path must not contain secret-looking material",
        "signer key id must be non-empty and must not contain secret-looking material",
        "candidate_resolved = candidate.resolve()",
        "slot_resolved = slot_path.resolve()",
        "signed evidence output path could not be resolved",
        '        except OSError:\n            errors.append("signed evidence output path could not be resolved")\n            return None\n',
        "signed evidence output path must stay under evidence/",
        "signed evidence output path must be",
        "_validate_json_output_path",
        'def _validate_json_output_path(path: Path, label: str) -> list[str]:\n    """Validate a signer-controlled output immediately before writing."""\n\n    if device_lab.SECRET_RE.search(str(path)):\n        return [f"{label} must not contain secret-looking material"]\n',
        '    parent_exists, parent_errors = _validate_json_output_parent(path, label)\n    errors.extend(parent_errors)\n    if errors:\n        return errors\n',
        '    errors.extend(\n        device_lab.validate_no_symlink_ancestors(\n            path,\n            f"{label} ancestor directory",\n        )\n    )\n    if errors:\n        return errors\n    if not parent_exists:\n',
        '    if not parent_exists:\n        try:\n            parent.mkdir(parents=True, exist_ok=True)\n        except OSError:\n            errors.append(f"{label} parent directory could not be created")\n',
        '    parent_exists, parent_errors = _validate_json_output_parent(\n        path,\n        label,\n        missing_error=f"{label} parent must be a directory",\n    )\n    errors.extend(parent_errors)\n    if not parent_exists and not errors:\n        errors.append(f"{label} parent must be a directory")\n',
        '        try:\n            parent.mkdir(parents=True, exist_ok=True)\n        except OSError:\n            errors.append(f"{label} parent directory could not be created")\n',
        'def _validate_json_output_parent(\n    path: Path,\n    label: str,\n    *,\n    missing_error: str | None = None,\n) -> tuple[bool, list[str]]:\n    """Classify a signer-controlled output parent without following aliases."""\n\n    parent = path.parent\n    try:\n        parent_mode = parent.lstat().st_mode\n    except FileNotFoundError:\n        if missing_error is None:\n            return False, []\n        return False, [missing_error]\n    except OSError:\n        return False, [f"{label} parent directory metadata could not be read"]\n',
        '    if stat.S_ISLNK(parent_mode):\n        return True, [f"{label} parent directory must not be a symlink"]\n    if not stat.S_ISDIR(parent_mode):\n        return True, [f"{label} parent must be a directory"]\n    return True, []\n',
        '    try:\n        mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return errors\n    except OSError:\n        errors.append(f"{label} file metadata could not be read")\n        return errors\n    if stat.S_ISLNK(mode):\n        errors.append(f"{label} must not be a symlink")\n',
        '        try:\n            link_count = path.stat().st_nlink\n        except OSError:\n            errors.append(f"{label} hardlink metadata could not be read")\n        else:\n            if link_count > 1:\n                errors.append(f"{label} must not be hardlinked")\n',
        "_validate_existing_json_output_path",
        'def _validate_existing_json_output_path(path: Path, label: str) -> list[str]:\n    """Validate a signer-controlled output immediately before reading it back."""\n\n    if device_lab.SECRET_RE.search(str(path)):\n        return [f"{label} must not contain secret-looking material"]\n',
        '    _, parent_errors = _validate_json_output_parent(\n        path,\n        label,\n        missing_error=f"{label} parent directory is missing",\n    )\n    if parent_errors:\n        return parent_errors\n',
        '    try:\n        mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return [f"{label} must exist before digest"]\n    except OSError:\n        return [f"{label} file metadata could not be read"]\n    if stat.S_ISLNK(mode):\n        return [f"{label} must not be a symlink"]\n',
        '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        return [f"{label} hardlink metadata could not be read"]\n    if link_count > 1:\n        return [f"{label} must not be hardlinked"]\n',
        "_output_file_sha256",
        'errors = _validate_existing_json_output_path(path, label)\n    if errors:\n        return None, errors\n',
        "_read_existing_output_bytes",
        "payload, read_errors = _read_existing_output_bytes(path, expected_stat, label)",
        'with path.open("rb") as handle:',
        "signer_output_expected_identity = (",
        "signer_output_open_identity = (open_stat.st_dev, open_stat.st_ino)",
        'return None, [f"{label} changed while being read"]',
        'except OSError:\n        return None, [f"{label} could not be read"]',
        'artifact_digest, digest_errors = _output_file_sha256(\n        output_path,\n        "signed evidence output path",\n    )',
        "_write_json(output_path, evidence, \"signed evidence output path\")",
        "_write_text",
        "_write_text(slot_path / \"sha256sum.txt\"",
        "_write_text_atomic",
        "tempfile.NamedTemporaryFile(",
        "handle.flush()",
        "os.fsync(handle.fileno())",
        "os.replace(tmp_path, path)",
        "_read_existing_output_text",
        '        if read_errors == [f"{label} could not be read"]:\n            return None, [f"{label} write verification failed"]\n',
        "readback_text != text",
        "write verification failed",
        '    errors = _validate_existing_json_output_path(path, label)\n    if errors:\n        return errors\n    try:\n        expected_stat = path.lstat()\n',
        '    readback_text, readback_errors = _read_existing_output_text(\n        path,\n        expected_stat,\n        label,\n    )\n    if readback_errors:\n        return readback_errors\n    if readback_text != text:',
        "_preflight_slot_metadata_reads",
        "_validate_slot_path_boundary",
        "Validate slot paths before any signer-controlled metadata is parsed",
        "Validate signer slot paths before reading mutable slot artifacts",
        "slot directory metadata could not be read",
        "slot parent directory metadata could not be read",
        "slot_mode = slot_path.lstat().st_mode",
        "parent_mode = slot_path.parent.lstat().st_mode",
        "slot_mode is not None and stat.S_ISLNK(slot_mode)",
        "parent_mode is not None and stat.S_ISLNK(parent_mode)",
        "slot_mode is None or not stat.S_ISDIR(slot_mode)",
        '    errors = _preflight_slot_metadata_reads(slot_path)\n    if errors:\n        return None, errors\n',
        '            _secret_key_path_error(private_key_path, "private key"),\n            _secret_key_path_error(public_key_path, "signer public key"),\n',
        'f"{label} parent directory must not be a symlink"',
        'f"{label} ancestor directory"',
        'f"{label} must not be hardlinked"',
        "slot artifacts must not contain secret-looking material",
        "slot artifact {display} is missing",
        "_validate_slot_artifact_for_digest",
        "_slot_artifact_sha256",
        "_read_validated_slot_artifact_bytes",
        "signer_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        "slot artifact {display} changed while being read",
        '    try:\n        artifact_stat = artifact_path.lstat()\n    except FileNotFoundError:\n        return None, None, [f"slot artifact {display} is missing"]\n    except OSError:\n        return None, None, [f"slot artifact {display} file metadata could not be read"]\n    if stat.S_ISLNK(artifact_stat.st_mode):\n        return None, None, [f"slot artifact {display} must not be a symlink"]\n',
        '    try:\n        link_count = artifact_path.stat().st_nlink\n    except OSError:\n        return None, None, [\n            f"slot artifact {display} hardlink metadata could not be read"\n        ]\n',
        "slot artifact {display} could not be read",
        'artifact_path, artifact_stat, errors = _validate_slot_artifact_for_digest(\n        slot_path,\n        relative,\n    )',
        "digest, digest_errors = _slot_artifact_sha256(slot_path, relative)",
        "validate_no_slot_symlink_artifacts",
        "validate_slot_regular_file_artifacts",
        "validate_no_slot_hardlink_artifacts",
        'def _validate_slot_path_boundary(slot_path: Path) -> list[str]:\n    """Validate signer slot paths before reading mutable slot artifacts."""\n\n    if device_lab.SECRET_RE.search(str(slot_path)):\n        return ["slot path must not contain secret-looking material"]\n    try:\n        slot_mode = slot_path.lstat().st_mode\n    except FileNotFoundError:\n        slot_mode = None\n    except OSError:\n        return ["slot directory metadata could not be read"]\n    if slot_mode is not None and stat.S_ISLNK(slot_mode):\n        return ["slot directory must not be a symlink"]\n    try:\n        parent_mode = slot_path.parent.lstat().st_mode\n    except FileNotFoundError:\n        parent_mode = None\n    except OSError:\n        return ["slot parent directory metadata could not be read"]\n    if parent_mode is not None and stat.S_ISLNK(parent_mode):\n        return ["slot parent directory must not be a symlink"]\n    ancestor_errors = device_lab.validate_no_symlink_ancestors(\n        slot_path,\n        "slot ancestor directory",\n    )\n    if ancestor_errors:\n        return ancestor_errors\n',
        "slot directory must not be a symlink",
        "validate_no_symlink_ancestors",
        '    ancestor_errors = device_lab.validate_no_symlink_ancestors(\n        slot_path,\n        "slot ancestor directory",\n    )\n    if ancestor_errors:\n        return ancestor_errors\n',
        "validate_required_kagemusha_slot_artifact_shapes",
        'preflight_errors = _preflight_slot_metadata_reads(slot_path)\n    if preflight_errors:\n        errors.extend(preflight_errors)\n        return None\n',
        "_validate_slot_for_manifest_rewrite",
        'def _validate_slot_for_manifest_rewrite(slot_path: Path) -> list[str]:\n    """Validate a slot immediately before rewriting its SHA-256 manifest."""\n\n    path_errors = _validate_slot_path_boundary(slot_path)\n    if path_errors:\n        return path_errors\n\n    errors: list[str] = []\n',
        "errors = _validate_slot_for_manifest_rewrite(slot_path)",
        "slot_files = device_lab._slot_files(slot_path, errors)",
        "for relative in slot_files:",
        '    slot_files = device_lab._slot_files(slot_path, errors)\n    if errors:\n        return errors\n    for relative in slot_files:\n        if device_lab.SECRET_RE.search(relative):\n            errors.append("slot artifacts must not contain secret-looking material")\n            return errors\n',
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
        "import stat",
        "SUMMARY_SCHEMA = \"iroha.kagemusha.production_readiness.v1\"",
        "LINEAGE_PROOF_EVIDENCE_SCHEMA = \"iroha.kagemusha.lineage_proof_evidence.v1\"",
        "LINEAGE_PROOF_EVIDENCE_FILENAME = \"lineage-proof-evidence.json\"",
        "DEFAULT_LINEAGE_PROOF_EVIDENCE_PATH",
        "COMPACT_KEY_EVIDENCE_SCHEMA = \"iroha.kagemusha.recursive_compact_key_evidence.v1\"",
        "COMPACT_KEY_EVIDENCE_FILENAME = \"recursive-compact-key-evidence.json\"",
        "DEFAULT_COMPACT_KEY_EVIDENCE_PATH",
        "DEFAULT_MIN_SIGNED_AT_UTC = \"2026-06-06T00:00:00Z\"",
        "DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS = 300",
        "ANDROID_DEVICE_LAB_ROOT_SUMMARY_LABEL",
        "LINEAGE_PROOF_EVIDENCE_SUMMARY_LABEL",
        "COMPACT_KEY_EVIDENCE_SUMMARY_LABEL",
        "LINEAGE_PROOF_EVIDENCE_FIELDS",
        "LINEAGE_PROOF_TEST_FIELDS",
        "COMPACT_KEY_EVIDENCE_FIELDS",
        "\"artifact_size_bytes\"",
        "def _require_compact_key_artifact_size(",
        "compact_key_evidence_artifact_sizes",
        "compact_key_evidence_artifact_sizes_unexpected_field",
        "ABI-7 recursive compact key evidence artifact size does not match local artifact bytes",
        "\"root\": ANDROID_DEVICE_LAB_ROOT_SUMMARY_LABEL",
        "ABI6_OPERATION_SYMBOLS",
        "check_abi6_reserved_lineage",
        "KagemushaCommand::RecursiveCompactKeyArtifacts",
        "KagemushaRecursiveCompactKeyArtifactsArgs",
        "derive_halo2_ipa_kagemusha_recursive_compact_payment_token_proving_key_bytes",
        "kagemusha_recursive_compact_payment_token_vk_record_from_box",
        "validate_release_local_json_file",
        "def _read_release_json_text(",
        "release_json_path_stat = path.lstat()",
        "release_json_final_path_stat = path.lstat()",
        'except OSError:\n        return None, [blocker(unreadable_code, f"{label} could not be read")]',
        'except UnicodeDecodeError:\n        return None, [blocker(unreadable_code, f"{label} could not be read")]',
        'def validate_release_local_json_file(path: Path, label: str) -> list[str]:\n    """Reject local release JSON files that could alias external bytes."""\n\n    if device_lab.SECRET_RE.search(str(path)):\n        return [f"{label} path must not contain secret-looking material"]\n',
        "release_json_ancestor_errors = device_lab.validate_no_symlink_ancestors(",
        '    release_json_ancestor_errors = device_lab.validate_no_symlink_ancestors(\n        path,\n        f"{label} ancestor directory",\n    )\n    if release_json_ancestor_errors:\n        return release_json_ancestor_errors\n    try:\n        mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return [f"{label} is missing"]\n    except OSError:\n        return [f"{label} file metadata could not be read"]\n    if stat.S_ISLNK(mode):\n        return [f"{label} must not be a symlink"]\n    if not stat.S_ISREG(mode):\n        return [f"{label} must be a regular file"]\n',
        '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        return [f"{label} hardlink metadata could not be read"]\n    if link_count > 1:\n        return [f"{label} must not be hardlinked"]\n    return []\n\n\ndef _validate_repo_source_marker_file_for_read(\n',
        "validate_repo_source_marker_file",
        "def _validate_repo_source_marker_file_for_read(",
        '    if device_lab.SECRET_RE.search(str(path)):\n        return None, [f"{label} path must not contain secret-looking material"]\n',
        '    try:\n        file_stat = path.lstat()\n    except FileNotFoundError:\n        errors.append(f"{label} is missing")\n        return None, errors\n    except OSError:\n        errors.append(f"{label} file metadata could not be read")\n        return None, errors\n    if stat.S_ISLNK(file_stat.st_mode):\n        errors.append(f"{label} must not be a symlink")\n        return None, errors\n    if not stat.S_ISREG(file_stat.st_mode):\n        errors.append(f"{label} must be a regular file")\n        return None, errors\n',
        '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        errors.append(f"{label} hardlink metadata could not be read")\n        return None, errors\n    if link_count > 1:\n        errors.append(f"{label} must not be hardlinked")\n    if errors:\n        return None, errors\n    return file_stat, []\n\n\ndef validate_repo_source_marker_file(path: Path, label: str) -> list[str]:',
        "_file_stat, errors = _validate_repo_source_marker_file_for_read(path, label)",
        "expected_marker_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        "open_marker_identity = (open_stat.st_dev, open_stat.st_ino)",
        "def _repo_source_marker_text(",
        "marker_path_stat = path.lstat()",
        "marker_final_path_stat = path.lstat()",
        'f"{label} changed while being read"',
        'return b"".join(chunks).decode("utf-8"), []',
        'except UnicodeDecodeError:\n        return None, [unreadable_error]',
        'unreadable_error = "ABI-7 source marker file could not be read"\n        text, file_errors = _repo_source_marker_text(\n            path,\n            label,\n            unreadable_error,\n        )',
        'unreadable_error = "Reserved-lineage release-tooling file could not be read"\n        text, file_errors = _repo_source_marker_text(\n            path,\n            "Reserved-lineage release-tooling marker file",\n            unreadable_error,\n        )',
        "abi6_manifest_unreadable",
        "lineage_proof_evidence_unreadable",
        'elif error == unreadable_error:\n                blockers.append(blocker(unreadable_code, error))',
        "abi6_manifest_file_shape",
        "def _rust_function_body(source: str, signature: str) -> str | None:",
        "def _require_rust_function_contract(",
        "check_abi7_fail_closed",
        "package_aware_multi_hop_composed",
        "abi7_source_marker_file_shape",
        "abi7_fail_closed_contract_missing",
        "abi7_bridge_unavailable_contract_missing",
        "preverify_kagemusha_recursive_compact_payment_token_with_expected_circuit_id(",
        "BridgeError::KagemushaRecursiveCompactUnavailable",
        "LINEAGE_KEY_RELEASE_TOOLING_REQUIREMENTS",
        "check_lineage_key_release_tooling",
        "lineage_key_release_file_shape",
        "lineage_key_release_marker_missing",
        '"lineage_key_release_tooling": lineage',
        "LINEAGE_PROOF_REQUIRED_ARTIFACTS",
        "LINEAGE_PROOF_REQUIRED_TESTS",
        "LINEAGE_PROOF_REQUIRED_TEST_LOGS",
        "EXPECTED_LINEAGE_PROOF_RESULT_PREFIX",
        "LINEAGE_ARTIFACT_ALL_ZERO_ERROR",
        "COMPACT_KEY_REQUIRED_ARTIFACTS",
        "\"recursive-compact-key-artifacts.norito\"",
        "\"recursive-compact-verifier-keys.norito\"",
        "COMPACT_KEY_PLACEHOLDER_PREFIXES",
        "COMPACT_KEY_ALL_ZERO_ERROR",
        "COMPACT_KEY_GENERATOR_LOG_FILENAME",
        "\"recursive-compact-key-artifacts.norito\": \"key_artifacts\"",
        "\"recursive-compact-verifier-keys.norito\": \"verifier_keys\"",
        "COMPACT_KEY_GENERATOR_LOG_DIGEST_FIELDS",
        "COMPACT_KEY_GENERATOR_LOG_RE",
        "sha256=(?P<vk_sha256>",
        "MAX_COMPACT_KEY_GENERATOR_LOG_BYTES",
        "EXPECTED_COMPACT_KEY_OPENING_LEN = 4",
        "EXPECTED_COMPACT_KEY_IPA_K = 8",
        "EXPECTED_COMPACT_KEY_BACKEND = \"halo2/ipa\"",
        "EXPECTED_COMPACT_KEY_CIRCUIT_ID = \"kagemusha-recursive-compact-v1\"",
        "EXPECTED_COMPACT_KEY_RECORD_NAMESPACE = \"offline_kagemusha\"",
        "EXPECTED_COMPACT_KEY_RECORD_VERSION = 1",
        "MAX_LINEAGE_PROOF_LOG_BYTES",
        "shlex.split(command)",
        "expected_tokens = (",
        "expected_lineage_proof_command",
        "expected_compact_key_command",
        "expected_compact_key_generator_log_line",
        "validate_lineage_proof_log",
        "validate_lineage_proof_command",
        "validate_lineage_artifact_content",
        "content_errors = validate_lineage_artifact_prefix(artifact_prefix, artifact)",
        "validate_compact_key_command",
        "validate_compact_key_artifact_content",
        "parse_compact_key_generator_log",
        "compact key generator log must use canonical LF line endings",
        "compact key generator log must end with a canonical LF line terminator",
        "validate_compact_key_generator_log",
        "lineage_proof_evidence_artifact_placeholder",
        "compact_key_evidence_artifact_placeholder",
        "compact_key_evidence_generator_log_path",
        "compact_key_evidence_generator_log_sha256",
        "compact_key_evidence_generator_log_digest",
        "compact_key_evidence_generator_log_format",
        "compact_key_evidence_generator_log_artifact_size",
        "compact_key_evidence_generator_log_artifact_digest",
        "generator_log_artifact_sha256",
        "must be generated lineage material, not all-zero placeholder bytes",
        "must be generated key material, not a placeholder fixture",
        "must be generated key material, not all-zero placeholder bytes",
        'def _sha256_file(path: Path, label: str) -> tuple[str | None, list[str]]:\n    expected_stat, file_errors = _validate_lineage_local_file_for_read(path, label)\n    if file_errors:\n        return None, file_errors\n    digest = hashlib.sha256()\n',
        "def _sha256_file_with_size(",
        "def _sha256_file_with_size_and_prefix(",
        "_validate_lineage_local_file_for_read",
        "prefix_parts: list[bytes] = []",
        "prefix_remaining = prefix_len",
        "prefix_parts.append(chunk[:prefix_remaining])",
        "expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        "open_identity = (open_stat.st_dev, open_stat.st_ino)",
        "open_stat = os.fstat(handle.fileno())",
        "            path_stat = path.lstat()",
        "final_path_stat = path.lstat()",
        'f"{label} changed while being read"',
        "size += len(chunk)",
        'return None, None, None, [f"{label} must be non-empty"]',
        "validate_compact_key_artifact_prefix",
        "validate_lineage_artifact_prefix",
        "content_errors = validate_lineage_artifact_prefix(artifact_prefix, artifact)",
        "content_errors = validate_compact_key_artifact_prefix(artifact_prefix, artifact)",
        'def _validate_lineage_local_file_for_read(\n    path: Path,\n    label: str,\n) -> tuple[os.stat_result | None, list[str]]:\n    """Reject local lineage evidence files that could alias external bytes."""\n\n    if device_lab.SECRET_RE.search(str(path)):\n        return None, [f"{label} path must not contain secret-looking material"]\n    ancestor_errors = device_lab.validate_no_symlink_ancestors(\n        path,\n        f"{label} ancestor directory",\n    )\n    if ancestor_errors:\n        return None, ancestor_errors\n    try:\n        file_stat = path.lstat()\n    except FileNotFoundError:\n        return None, [f"{label} is missing"]\n    except OSError:\n        return None, [f"{label} file metadata could not be read"]\n',
        '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        return None, [f"{label} hardlink metadata could not be read"]\n    if link_count > 1:\n        return None, [f"{label} must not be hardlinked"]\n    return file_stat, []\n\n\ndef validate_lineage_local_file(path: Path, label: str) -> list[str]:\n',
        'except OSError:\n        return None, [f"{label} could not be read"]',
        '    try:\n        if path.stat().st_size > MAX_LINEAGE_PROOF_LOG_BYTES:\n            return None, [\n                f"production proof log must be no more than {MAX_LINEAGE_PROOF_LOG_BYTES} bytes"\n            ]\n    except OSError:\n        return None, ["production proof log metadata could not be read"]\n',
        "def _lineage_local_text(",
        "def _sha256_text_file(",
        "chunks: list[bytes] = []",
        'text = b"".join(chunks).decode("utf-8", errors=decode_errors)',
        'digest, text, read_errors = _sha256_text_file(\n        path,\n        "production proof log",\n        "production proof log could not be read",',
        'digest, text, read_errors = _sha256_text_file(\n        path,\n        "ABI-7 recursive compact key generator log",\n        "ABI-7 recursive compact key generator log could not be read",',
        "DuplicateJsonKeyError",
        "NonFiniteJsonConstantError",
        "_reject_duplicate_json_object_pairs",
        "_reject_nonfinite_json_constant",
        "shape_code: str",
        'digest, text, read_errors = _sha256_text_file(\n        path,\n        label,\n        f"{label} could not be read",\n    )',
        "shape_code=\"lineage_proof_evidence_file_shape\"",
        "shape_code=\"compact_key_evidence_file_shape\"",
        "object_pairs_hook=_reject_duplicate_json_object_pairs",
        "parse_constant=_reject_nonfinite_json_constant",
        "contains duplicate JSON object key",
        "non-finite constant",
        "def _display_evidence_value(value: Any) -> Any:",
        "details[\"generated_at_utc\"]",
        "details[\"artifact_sha256\"]",
        "details[\"test_log_sha256\"]",
        "max_generated_at_utc",
        "check_lineage_proof_evidence",
        "check_compact_key_evidence",
        "lineage_proof_evidence_filename",
        "Reserved-lineage proof evidence file must be named",
        "compact_key_evidence_filename",
        "ABI-7 recursive compact key evidence file must be named",
        "require_canonical_filename: bool = True",
        "lineage_proof_evidence_missing",
        "compact_key_evidence_missing",
        "lineage_proof_evidence_file_shape",
        "compact_key_evidence_file_shape",
        "Reserved-lineage proof evidence file",
        "ABI-7 recursive compact key evidence file",
        'report.get("kagemusha", {}).get("signed_at_utc")',
        "validated Android device-lab report is missing signed evidence timestamp",
        "lineage_proof_evidence_stale",
        "lineage_proof_evidence_future_dated",
        "lineage_proof_evidence_timestamp_noncanonical",
        "compact_key_evidence_stale",
        "compact_key_evidence_future_dated",
        "compact_key_evidence_timestamp_noncanonical",
        "generated_at_utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
        "generated_at_raw = generated_at_text",
        "compact_generated_at_raw = generated_at_text",
        "device_lab.SIGNED_AT_UTC_RE.fullmatch(generated_at_raw)",
        "device_lab.SIGNED_AT_UTC_RE.fullmatch(compact_generated_at_raw)",
        "device_lab.SIGNED_AT_UTC_RE.fullmatch(signed_at_text)",
        "android_signed_evidence_timestamp_noncanonical",
        "not isinstance(scalar_value, int)",
        "not isinstance(compact_scalar_value, int)",
        "isinstance(scalar_value, bool)",
        "isinstance(compact_scalar_value, bool)",
        "must be integer",
        "def _require_lineage_artifact_size(",
        "lineage_proof_evidence_artifact_sizes",
        "lineage_proof_evidence_artifact_sizes_unexpected_field",
        "lineage_proof_evidence_artifact_missing",
        "lineage_proof_evidence_artifact_empty",
        "compact_key_evidence_artifact_missing",
        "compact_key_evidence_artifact_empty",
        "validate_lineage_local_file",
        'device_lab.SECRET_RE.search(str(path))',
        "path must not contain secret-looking material",
        "ancestor_errors = device_lab.validate_no_symlink_ancestors(",
        'def validate_lineage_local_file(path: Path, label: str) -> list[str]:\n    """Reject local lineage evidence files that could alias external bytes."""\n\n    _file_stat, errors = _validate_lineage_local_file_for_read(path, label)\n    return errors\n',
        'f"{label} ancestor directory"',
        '            artifact_file_errors = validate_lineage_local_file(\n                artifact_path,\n                "Reserved-lineage proof evidence artifact file",\n            )\n            if artifact_file_errors:\n                if artifact_file_errors == [\n                    "Reserved-lineage proof evidence artifact file is missing"\n                ]:\n                    blockers.append(\n                        blocker(\n                            "lineage_proof_evidence_artifact_missing",\n                            "Reserved-lineage proof evidence artifact file is missing",\n                            artifact=artifact,\n                        )\n                    )\n                else:\n                    for error in artifact_file_errors:\n                        blockers.append(\n                            blocker(\n                                "lineage_proof_evidence_artifact_file_shape",\n                                error,\n                                artifact=artifact,\n                            )\n                        )\n                continue\n',
        '            (\n                actual_digest,\n                artifact_size,\n                artifact_prefix,\n                digest_errors,\n            ) = _sha256_file_with_size_and_prefix(\n                artifact_path,\n                "Reserved-lineage proof evidence artifact file",\n                allow_empty=True,\n            )',
        '            (\n                actual_digest,\n                artifact_size,\n                artifact_prefix,\n                digest_errors,\n            ) = _sha256_file_with_size_and_prefix(\n                artifact_path,\n                "ABI-7 recursive compact key evidence artifact file",\n                allow_empty=True,\n            )',
        '            actual_log_digest, log_errors = validate_lineage_proof_log(\n                log_artifact_path, expected_name\n            )\n            log_file_missing = log_errors == ["missing production proof log"]\n',
        "lineage_proof_evidence_artifact_file_digest",
        "compact_key_evidence_artifact_file_digest",
        "compact_key_evidence_command",
        "must exactly match the production ABI-7 recursive compact keygen command",
        "must exactly match the canonical ABI-7 recursive compact keygen command string",
        "lineage_proof_evidence_test_log_path",
        "lineage_proof_evidence_test_log_missing",
        "lineage_proof_evidence_test_log_unreadable",
        "lineage_proof_evidence_test_log_file_digest",
        "lineage_proof_evidence_test_log_content",
        "--proof-log must use canonical LF line endings",
        "--proof-log must end with a canonical LF line terminator",
        "test_lines != [expected_test_line]",
        "not line.startswith(\"test result:\")",
        "LINEAGE_PROOF_RESULT_RE.fullmatch",
        "result_lines = [line for line in lines if line.startswith(\"test result:\")]",
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
        "compact_key_evidence_unexpected_field",
        "lineage_proof_evidence_circuit_ids_unexpected_field",
        "lineage_proof_evidence_artifacts_unexpected_field",
        "lineage_proof_evidence_artifact_sizes_unexpected_field",
        "compact_key_evidence_artifacts_unexpected_field",
        "lineage_proof_evidence_tests_unexpected_field",
        "lineage_proof_evidence_test_unexpected_field",
        "Reserved-lineage proof evidence artifact digest does not match local artifact bytes",
        "Reserved-lineage proof evidence artifact size does not match local artifact bytes",
        "log digest does not match local log bytes",
        "record_archive_proof_runtime_keygen_env",
        '"lineage_proof_evidence": lineage_proof',
        '"compact_key_evidence": compact_key',
        "--lineage-proof-evidence",
        "--compact-key-evidence",
        "compact_key_evidence_path=compact_key_evidence_path,",
        "--min-lineage-proof-evidence-at-utc",
        "--max-lineage-proof-evidence-future-skew-seconds",
        "--min-compact-key-evidence-at-utc",
        "--max-compact-key-evidence-future-skew-seconds",
        "max_lineage_proof_evidence_at",
        "max_compact_key_evidence_at",
        "lineage_proof_evidence_max_timestamp_invalid",
        "compact_key_evidence_max_timestamp_invalid",
        "check_android_device_lab",
        "root_exists, root_errors = device_lab.classify_device_lab_root_path(root)",
        "if not root_exists:",
        "_check_android_matrix_unique_bindings",
        "android_device_lab_duplicate_device_fingerprint",
        "android_device_lab_duplicate_attestation_challenge",
        "Android device-lab production slots must not reuse a device fingerprint",
        "value_sha256",
        "_redact_secret_strings",
        "_sanitize_android_reports",
        "android_device_lab_report_secret_material",
        "android_device_lab_root_unreadable",
        "raw_reports, discovery_blockers = _slot_reports(",
        "blockers.extend(discovery_blockers)",
        "reports, report_secret_blockers = _sanitize_android_reports(",
        "validate_cli_path_arguments",
        "path_blockers = validate_cli_path_arguments(args)",
        "validate_repo_root_path",
        "repo_root: Path | None = None",
        "--repo-root could not be resolved",
        '        try:\n            repo_root = Path(args.repo_root).resolve()\n        except OSError:\n            path_blockers.append(\n                blocker(\n                    "kagemusha_repo_root_path_invalid",\n                    "--repo-root could not be resolved",\n                )\n            )\n',
        'secret_blocker = _secret_looking_path_blocker(\n        str(root),\n        label="--repo-root",\n        code="kagemusha_repo_root_path_invalid",\n    )\n    if secret_blocker is not None:\n        return [secret_blocker]\n',
        '    try:\n        root_mode = root.lstat().st_mode\n    except FileNotFoundError:\n        root_mode = None\n    except OSError:\n        errors.append("--repo-root metadata could not be read")\n        return [\n            blocker("kagemusha_repo_root_path_invalid", error)\n            for error in errors\n        ]\n',
        "root_mode is not None and stat.S_ISLNK(root_mode)",
        "root_mode is not None and not stat.S_ISDIR(root_mode)",
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
        '    parent_exists, parent_blockers = _validate_summary_output_parent(path)\n    if parent_blockers:\n        return parent_blockers\n',
        '    ancestor_errors = device_lab.validate_no_symlink_ancestors(\n        path,\n        "--summary-out ancestor directory",\n    )\n    if ancestor_errors:\n        return [\n            blocker(SUMMARY_OUT_PATH_INVALID_CODE, error)\n            for error in ancestor_errors\n        ]\n    if not parent_exists:\n',
        '        try:\n            parent.mkdir(parents=True, exist_ok=True)\n        except OSError:\n            return [\n                blocker(\n                    SUMMARY_OUT_PATH_INVALID_CODE,\n                    "--summary-out parent directory could not be created",\n                )\n            ]\n',
        '    parent_exists, parent_blockers = _validate_summary_output_parent(\n        path,\n        missing_message="--summary-out parent must be a directory",\n    )\n    if parent_blockers:\n        return parent_blockers\n    if not parent_exists:\n        return [\n            blocker(\n                SUMMARY_OUT_PATH_INVALID_CODE,\n                "--summary-out parent must be a directory",\n            )\n        ]\n',
        'def _validate_summary_output_parent(\n    path: Path,\n    *,\n    missing_message: str | None = None,\n) -> tuple[bool, list[dict[str, Any]]]:\n    """Classify the readiness summary output parent without following aliases."""\n\n    parent = path.parent\n    try:\n        parent_mode = parent.lstat().st_mode\n    except FileNotFoundError:\n        if missing_message is None:\n            return False, []\n        return False, [blocker(SUMMARY_OUT_PATH_INVALID_CODE, missing_message)]\n    except OSError:\n        return False, [\n            blocker(\n                SUMMARY_OUT_PATH_INVALID_CODE,\n                "--summary-out parent directory metadata could not be read",\n            )\n        ]\n',
        '    if stat.S_ISLNK(parent_mode):\n        return True, [\n            blocker(\n                SUMMARY_OUT_PATH_INVALID_CODE,\n                "--summary-out parent directory must not be a symlink",\n            )\n        ]\n    if not stat.S_ISDIR(parent_mode):\n        return True, [\n            blocker(\n                SUMMARY_OUT_PATH_INVALID_CODE,\n                "--summary-out parent must be a directory",\n            )\n        ]\n    return True, []\n',
        '    try:\n        summary_output_mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return []\n    except OSError:\n        return [\n            blocker(\n                SUMMARY_OUT_PATH_INVALID_CODE,\n                "--summary-out file metadata could not be read",\n            )\n        ]\n',
        "if stat.S_ISLNK(summary_output_mode):",
        "if not stat.S_ISREG(summary_output_mode):",
        "write_blockers = write_summary",
        "--summary-out could not be written",
        "tempfile.NamedTemporaryFile(",
        "handle.flush()",
        "os.fsync(handle.fileno())",
        "os.replace(tmp_path, path)",
        "_read_summary_output_text",
        "summary_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        "summary_open_identity = (open_stat.st_dev, open_stat.st_ino)",
        "--summary-out changed while being read",
        "readback_text, readback_errors = _read_summary_output_text(path, expected_stat)",
        "readback_text != summary_text",
        "--summary-out write verification failed",
        '    except OSError:\n        return None, [\n            _summary_out_blocker("--summary-out write verification failed")\n        ]\n',
        '    errors = validate_summary_output_path(path)\n    if errors:\n        return errors\n    try:\n        expected_stat = path.lstat()\n',
        "--summary-out must not be a symlink",
        '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        return [\n            blocker(\n                SUMMARY_OUT_PATH_INVALID_CODE,\n                "--summary-out hardlink metadata could not be read",\n            )\n        ]\n',
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
        "def _sha256_file(path: Path, label: str) -> tuple[str | None, list[str]]:",
        "def _sha256_file_with_size(",
        "expected_stat, file_errors = readiness._validate_lineage_local_file_for_read(",
        "prefix_parts: list[bytes] = []",
        "prefix_remaining = 4096",
        "prefix_parts.append(chunk[:prefix_remaining])",
        "expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        "open_identity = (open_stat.st_dev, open_stat.st_ino)",
        "return None, file_errors",
        "open_stat = os.fstat(handle.fileno())",
        "            path_stat = path.lstat()",
        "final_path_stat = path.lstat()",
        'f"{label} changed while being read"',
        "size += len(chunk)",
        'except OSError:\n        return None, [f"{label} could not be read"]',
        "digest, artifact_size, artifact_prefix, file_errors = _sha256_file_with_size(",
        "artifact_size_bytes",
        'return None, None, None, [f"{label} must be non-empty"]',
        "content_errors = readiness.validate_lineage_artifact_prefix(artifact_prefix, artifact)",
        "validate_evidence_document",
        "check_lineage_proof_evidence",
        "require_canonical_filename=False",
        'secret_error = _secret_path_error(str(artifact_dir), "--artifact-dir")',
        'secret_error = _secret_path_error(str(path), label)',
        "validate_artifact_dir_path",
        'def validate_artifact_dir_path(artifact_dir: Path) -> list[str]:\n    """Reject artifact directories that could alias external release bytes."""\n\n    secret_error = _secret_path_error(str(artifact_dir), "--artifact-dir")\n    if secret_error is not None:\n        return [secret_error]\n',
        '    try:\n        artifact_dir_mode = artifact_dir.lstat().st_mode\n    except FileNotFoundError:\n        artifact_dir_mode = None\n    except OSError:\n        return ["--artifact-dir metadata could not be read"]\n',
        "artifact_dir_mode is not None and stat.S_ISLNK(artifact_dir_mode)",
        "if artifact_dir_mode is None:",
        "if not stat.S_ISDIR(artifact_dir_mode):",
        "_resolve_corridor_path",
        'except OSError:\n        return None, [f"{label} could not be resolved"]',
        "_same_resolved_parent",
        "same_parent, corridor_errors = _same_resolved_parent(proof_log, artifact_dir)",
        "validate_output_corridor",
        "path_errors.extend(validate_output_corridor(out_path, artifact_dir))",
        "validate_lineage_input_paths",
        'proof_log_secret_error = _secret_path_error(str(proof_log), "--proof-log")\n    if proof_log_secret_error is not None:\n        errors.append(proof_log_secret_error)\n    if errors:\n        return errors\n',
        "errors = validate_lineage_input_paths(artifact_dir, proof_log)",
        "path_errors.extend(validate_lineage_input_paths(artifact_dir, proof_log))",
        "preflight_output_path",
        'def preflight_output_path(path: Path, label: str) -> list[str]:\n    """Reject aliased output paths before evidence inputs are read."""\n\n    secret_error = _secret_path_error(str(path), label)\n    if secret_error is not None:\n        return [secret_error]\n',
        '    parent_exists, parent_errors = _validate_output_parent(path, label)\n    if parent_errors:\n        return parent_errors\n',
        '    output_ancestor_errors = device_lab.validate_no_symlink_ancestors(\n        path,\n        f"{label} ancestor directory",\n    )\n    if output_ancestor_errors:\n        return output_ancestor_errors\n    if not parent_exists:\n        try:\n            parent.mkdir(parents=True, exist_ok=True)\n        except OSError:\n            return [f"{label} parent directory could not be created"]\n',
        '    parent_exists, parent_errors = _validate_output_parent(\n        path,\n        label,\n        missing_error=f"{label} parent must be a directory",\n    )\n    if parent_errors:\n        return parent_errors\n    if not parent_exists:\n        return [f"{label} parent must be a directory"]\n',
        '    try:\n        output_mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return []\n    except OSError:\n        return [f"{label} file metadata could not be read"]\n',
        "if stat.S_ISLNK(output_mode):",
        "if not stat.S_ISREG(output_mode):",
        '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        return [f"{label} hardlink metadata could not be read"]\n    if link_count > 1:\n        return [f"{label} must not be hardlinked"]\n',
        'def _validate_output_parent(\n    path: Path,\n    label: str,\n    *,\n    missing_error: str | None = None,\n) -> tuple[bool, list[str]]:\n    """Classify an output parent without following symlink aliases."""\n\n    parent = path.parent\n    try:\n        parent_mode = parent.lstat().st_mode\n    except FileNotFoundError:\n        if missing_error is None:\n            return False, []\n        return False, [missing_error]\n    except OSError:\n        return False, [f"{label} parent directory metadata could not be read"]\n',
        '    if stat.S_ISLNK(parent_mode):\n        return True, [f"{label} parent directory must not be a symlink"]\n    if not stat.S_ISDIR(parent_mode):\n        return True, [f"{label} parent must be a directory"]\n    return True, []\n',
        "validate_output_path",
        '    errors = preflight_output_path(path, label)\n    if errors:\n        return errors\n    parent = path.parent\n    parent_exists, parent_errors = _validate_output_parent(path, label)\n    if parent_errors:\n        return parent_errors\n    if not parent_exists:\n        try:\n            parent.mkdir(parents=True, exist_ok=True)\n        except OSError:\n            return [f"{label} parent directory could not be created"]\n    return preflight_output_path(path, label)\n',
        'early_output_errors = preflight_output_path(out_path, "--out")',
        "--artifact-dir must not be a symlink",
        'f"{label} ancestor directory"',
        "write_errors = write_evidence(out_path, evidence)",
        "--out could not be written",
        "tempfile.NamedTemporaryFile(",
        "handle.flush()",
        "os.fsync(handle.fileno())",
        "os.replace(tmp_path, path)",
        "def _read_output_text(",
        "output_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        "output_open_identity = (open_stat.st_dev, open_stat.st_ino)",
        'f"{label} changed while being read"',
        'except OSError:\n        return None, [f"{label} write verification failed"]',
        'except UnicodeDecodeError:\n        return None, [f"{label} write verification failed"]',
        '    try:\n        expected_stat = path.lstat()\n    except (FileNotFoundError, OSError):\n        return ["--out write verification failed"]\n',
        'readback_text, readback_errors = _read_output_text(path, expected_stat, "--out")',
        "readback_text != evidence_text",
        "--out write verification failed",
        '    errors = validate_output_path(path, "--out")\n    if errors:\n        return errors\n    try:\n        expected_stat = path.lstat()',
        "missing lineage artifact",
        "wrote evidence",
        "pre_create_dir_errors = validate_artifact_dir_path(artifact_dir)",
        '    try:\n        artifact_dir.mkdir(parents=True, exist_ok=True)\n    except OSError:\n        return ["--artifact-dir could not be created for evidence validation"]\n',
        "lineage proof evidence validation file could not be written",
        '    except OSError:\n        if path is not None:\n            try:\n                path.unlink(missing_ok=True)\n            except OSError:\n                pass\n        return ["lineage proof evidence validation file could not be written"]\n',
        '    try:\n        path.unlink(missing_ok=True)\n    except OSError:\n        return ["lineage proof evidence validation file could not be removed"]\n',
    ),
    "scripts/kagemusha_recursive_compact_key_evidence.py": (
        "Build ABI-7 recursive compact key-artifact release evidence JSON",
        "DEFAULT_COMPACT_KEY_COMMAND",
        "COMPACT_KEY_REQUIRED_ARTIFACTS",
        "validate_compact_key_command",
        "validate_lineage_local_file",
        "_validate_generated_at_utc",
        "errors.extend(_validate_generated_at_utc(generated_at_utc))",
        "--generated-at-utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
        "--artifact-dir",
        "--generated-at-utc",
        "--out must be named",
        "--out must be written directly under --artifact-dir",
        "def _sha256_file(path: Path, label: str) -> tuple[str | None, list[str]]:",
        "def _sha256_file_with_size(",
        "expected_stat, file_errors = readiness._validate_lineage_local_file_for_read(",
        "prefix_parts: list[bytes] = []",
        "prefix_remaining = 4096",
        "prefix_parts.append(chunk[:prefix_remaining])",
        "expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        "open_identity = (open_stat.st_dev, open_stat.st_ino)",
        "return None, file_errors",
        "open_stat = os.fstat(handle.fileno())",
        "            path_stat = path.lstat()",
        "final_path_stat = path.lstat()",
        'f"{label} changed while being read"',
        "size += len(chunk)",
        'except OSError:\n        return None, [f"{label} could not be read"]',
        "missing recursive compact key artifact",
        "missing recursive compact key generator log",
        "recursive compact key generator log size does not match local artifact",
        "--generator-log must live directly under --artifact-dir",
        "artifact_size_bytes",
        'return None, None, None, [f"{label} must be non-empty"]',
        "readiness.validate_compact_key_artifact_prefix(artifact_prefix, artifact)",
        "def _sha256_text_file_with_size(",
        "chunks: list[bytes] = []",
        'except UnicodeDecodeError:\n        return None, None, None, [f"{label} could not be read"]',
        'text = b"".join(chunks).decode("utf-8")',
        ") = _sha256_text_file_with_size(",
        "readiness.parse_compact_key_generator_log(generator_log_text)",
        "generator_log_sha256",
        "not a placeholder fixture",
        "validate_evidence_document",
        "check_compact_key_evidence",
        "require_canonical_filename=False",
        "pre_create_dir_errors = validate_artifact_dir_path(artifact_dir)",
        "artifact_dir.mkdir(parents=True, exist_ok=True)",
        "post_create_dir_errors = validate_artifact_dir_path(artifact_dir)",
        "--artifact-dir could not be created for evidence validation",
        'def validate_artifact_dir_path(artifact_dir: Path) -> list[str]:\n    """Reject artifact directories that could alias external release bytes."""\n\n    secret_error = _secret_path_error(str(artifact_dir), "--artifact-dir")\n    if secret_error is not None:\n        return [secret_error]\n',
        '    try:\n        artifact_dir_mode = artifact_dir.lstat().st_mode\n    except FileNotFoundError:\n        artifact_dir_mode = None\n    except OSError:\n        return ["--artifact-dir metadata could not be read"]\n',
        'secret_error = _secret_path_error(str(artifact_dir), "--artifact-dir")',
        'secret_error = _secret_path_error(str(path), label)',
        "validate_artifact_dir_path",
        "preflight_output_path",
        "validate_output_corridor",
        'path_errors.extend(preflight_output_path(out_path, "--out"))',
        '    if not parent_exists:\n        try:\n            parent.mkdir(parents=True, exist_ok=True)\n        except OSError:\n            return [f"{label} parent directory could not be created"]\n',
        '    try:\n        output_mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return []\n    except OSError:\n        return [f"{label} file metadata could not be read"]\n',
        '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        return [f"{label} hardlink metadata could not be read"]\n    if link_count > 1:\n        return [f"{label} must not be hardlinked"]\n',
        "validate_output_path",
        "write_errors = write_evidence(out_path, evidence)",
        "--out could not be written",
        "tempfile.NamedTemporaryFile(",
        "handle.flush()",
        "os.fsync(handle.fileno())",
        "os.replace(tmp_path, path)",
        "def _read_output_text(",
        "output_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        "output_open_identity = (open_stat.st_dev, open_stat.st_ino)",
        'f"{label} changed while being read"',
        'except OSError:\n        return None, [f"{label} write verification failed"]',
        'except UnicodeDecodeError:\n        return None, [f"{label} write verification failed"]',
        '    try:\n        expected_stat = path.lstat()\n    except (FileNotFoundError, OSError):\n        return ["--out write verification failed"]\n',
        'readback_text, readback_errors = _read_output_text(path, expected_stat, "--out")',
        "readback_text != evidence_text",
        "--out write verification failed",
        '    errors = validate_output_path(path, "--out")\n    if errors:\n        return errors\n    try:\n        expected_stat = path.lstat()',
        "recursive compact key evidence validation file could not be written",
        "recursive compact key evidence validation file could not be removed",
        "wrote evidence",
    ),
    "scripts/kagemusha_release_bundle.py": (
        "Validate and manifest a Kagemusha production release evidence bundle",
        "RELEASE_BUNDLE_SCHEMA = \"iroha.kagemusha.production_release_bundle.v1\"",
        "DEFAULT_READINESS_SUMMARY_PATH = \"dist/kagemusha-production-readiness.json\"",
        "DEFAULT_RELEASE_BUNDLE_OUT = \"dist/kagemusha-production-release-bundle.json\"",
        "RELEASE_BUNDLE_ALLOWED_TOP_LEVEL_KEYS",
        "SUMMARY_REQUIRED_SECTION_STATES",
        "SUMMARY_ALLOWED_TOP_LEVEL_KEYS",
        "SUMMARY_ALLOWED_SECTION_KEYS",
        "ANDROID_SIGNED_EVIDENCE_SUMMARY_REQUIRED_FIELDS",
        "ANDROID_SIGNED_EVIDENCE_SUMMARY_PATH_FIELDS",
        "ANDROID_SIGNED_EVIDENCE_SUMMARY_SHA256_FIELDS",
        "--repo-root",
        "--verify-existing",
        "package_aware_multi_hop_composed",
        "production_width_proof_passed",
        "compact_key_artifacts_validated",
        "def build_release_bundle(",
        "def verify_release_bundle(",
        "root_blockers = _validate_bundle_root(bundle_root)",
        'if not root_blockers:\n        path_ok, path_blockers = _preflight_bundle_input_path(',
        'existing_bundle_path,\n            bundle_root,\n            "Kagemusha release bundle manifest",',
        "check_abi6_reserved_lineage",
        "check_abi7_fail_closed",
        "check_lineage_key_release_tooling",
        "_preflight_bundle_input_path",
        "summary_path_ok",
        "lineage_path_ok",
        "compact_path_ok",
        "android_path_ok",
        "input_paths_ok",
        "if input_paths_ok and summary_path_ok:",
        "if input_paths_ok and lineage_path_ok:",
        "artifact_content_validator=readiness.validate_lineage_artifact_content",
        "if input_paths_ok and compact_path_ok:",
        "if input_paths_ok and android_path_ok:",
        "        and input_paths_ok",
        "_contains_secret_string",
        "_check_ready_summary_shape",
        "_check_android_signed_evidence_summary_shape",
        "kagemusha_release_summary_secret_material",
        "kagemusha_release_summary_unexpected_field",
        "kagemusha_release_summary_unexpected_section_field",
        "kagemusha_release_summary_section_blockers_present",
        "kagemusha_release_summary_android_signed_evidence_shape",
        "kagemusha_release_summary_android_signed_evidence_slot",
        "kagemusha_release_summary_android_signed_evidence_unexpected_field",
        "kagemusha_release_summary_android_signed_evidence_missing_field",
        "kagemusha_release_summary_android_signed_evidence_value",
        "kagemusha_release_summary_android_signed_evidence_timestamp",
        "kagemusha_release_summary_android_signed_evidence_sha256",
        "kagemusha_release_summary_android_signed_evidence_path",
        "_compare_validated_sections",
        "blockers.extend(\n            _compare_validated_sections(\n                summary,\n                abi6,\n                abi7,",
        "kagemusha_release_summary_drift",
        "_read_local_json_text",
        "_validate_local_file_for_read",
        'text, read_blockers = _read_local_json_text(',
        "release_json_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        "release_json_open_identity = (open_stat.st_dev, open_stat.st_ino)",
        "object_pairs_hook=readiness._reject_duplicate_json_object_pairs",
        "parse_constant=readiness._reject_nonfinite_json_constant",
        "_evidence_entry_with_size",
        "_sha256_file",
        "_sha256_file_with_size",
        "    digest_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        "            digest_open_identity = (open_stat.st_dev, open_stat.st_ino)",
        "sized_digest_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        "sized_digest_open_identity = (open_stat.st_dev, open_stat.st_ino)",
        "open_stat = os.fstat(handle.fileno())",
        "            path_stat = path.lstat()",
        "final_path_stat = path.lstat()",
        'f"{label} changed while being read"',
        "size += len(chunk)",
        "if size <= 0:",
        "f\"{label} must be non-empty\"",
        "generator_log_artifact_sha256",
        "_artifact_inventory_entries",
        "artifact_content_validator",
        "Callable[[Path, str], list[str]]",
        'f"{code_prefix}_placeholder"',
        "readiness.validate_compact_key_artifact_content",
        "_lineage_proof_log_entries",
        "_stable_release_bundle",
        "_check_release_bundle_manifest_shape",
        "_check_release_bundle_evidence_paths",
        'blockers.extend(_check_release_bundle_evidence_paths(bundle.get("evidence")))',
        "kagemusha_release_bundle_manifest_evidence_shape",
        "kagemusha_release_bundle_manifest_evidence_path",
        "kagemusha_release_bundle_manifest_evidence_sha256",
        "kagemusha_release_bundle_manifest_evidence_size",
        '"size_bytes" not in item',
        "kagemusha_release_bundle_manifest_drift",
        "[kagemusha-release-bundle] verified",
        "lineage_artifacts",
        "compact_key_artifacts",
        "compact_key_generator_log",
        "lineage_proof_logs",
        "ANDROID_SLOT_RELEASE_ARTIFACTS",
        "_android_slot_artifact_entries",
        "android_slot_artifacts",
        "offline_wallet_apk",
        "d2d_payment_transcript",
        "wallet_integrity_transcript",
        "attestation_certificate_chain",
        "kagemusha_release_android_slot_artifact_summary_drift",
        "kagemusha_release_android_slot_artifact_digest_drift",
        "kagemusha_release_android_slot_artifact_inventory",
        "device_lab._normalise_safe_relative_path",
        'code_prefix="kagemusha_release_lineage_artifact"',
        'code_prefix="kagemusha_release_compact_artifact"',
        'f"{code_prefix}_digest_drift"',
        'f"{code_prefix}_size_drift"',
        'f"{code_prefix}_inventory"',
        "kagemusha_release_lineage_proof_log_digest_drift",
        "kagemusha_release_lineage_proof_log_inventory",
        "_android_signed_evidence_entries",
        "_validate_android_manifest_slot",
        "KAGEMUSHA_SIGNED_EVIDENCE_ARTIFACT_PATH",
        "kagemusha_release_android_signed_evidence_slot",
        "kagemusha_release_android_signed_evidence_summary_drift",
        "android_signed_evidence",
        "_bundle_evidence_paths",
        "--out must not overwrite bundled evidence input",
        "_relative_to_bundle",
        "must stay under --bundle-root",
        "_validate_output_parent_path",
        "_validate_output_path",
        "--bundle-root must not be a symlink",
        "--out parent directory must not be a symlink",
        "--out must not be a symlink",
        "--out must not be hardlinked",
        "tempfile.NamedTemporaryFile",
        "os.fsync(handle.fileno())",
        "os.replace(tmp_path, path)",
        "_read_output_text",
        "output_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        "output_open_identity = (open_stat.st_dev, open_stat.st_ino)",
        "--out changed while being read",
        "readback, readback_blockers = _read_output_text(path, expected_stat)",
        '    try:\n        expected_stat = path.lstat()\n    except (FileNotFoundError, OSError):\n        return [\n            _release_bundle_out_blocker("--out could not be read back after writing")\n        ]\n',
        "os.open(path.parent, os.O_RDONLY)",
        "os.fsync(parent_fd)",
        '    except OSError:\n        return None, [\n            _release_bundle_out_blocker("--out could not be read back after writing")\n        ]\n',
        "--out readback did not match the generated manifest",
        "check_lineage_proof_evidence",
        "check_compact_key_evidence",
        "check_android_device_lab",
        "write_release_bundle",
        "[kagemusha-release-bundle] ready",
    ),
    "scripts/tests/check_android_device_lab_slot_test.py": (
        "test_checked_in_sample_slot_passes_default_validation",
        "test_scan_slot_rejects_sha256_drift",
        "test_explicit_missing_slot_returns_structured_error",
        "test_discover_slots_returns_structured_error_on_root_list_failure",
        "test_discover_slots_uses_lstat_before_is_dir_preflight",
        "test_discover_slots_reports_slot_metadata_failure_before_is_dir_preflight",
        "test_discover_slots_preserves_symlinked_slot_for_scan_slot_rejection",
        "test_main_rejects_device_lab_root_list_failure_without_traceback",
        "test_explicit_unsafe_slot_id_rejected_before_path_join",
        "test_explicit_secret_looking_slot_id_is_not_echoed",
        "test_discovered_secret_looking_slot_directory_is_not_echoed",
        "test_scan_slot_redacts_secret_looking_manifest_paths",
        "test_scan_slot_rejects_slot_directory_metadata_failure",
        "test_scan_slot_rejects_slot_parent_metadata_failure",
        "test_slot_files_missing_slot_returns_empty_without_traceback",
        "test_slot_files_non_directory_root_returns_empty_without_traceback",
        "test_slot_files_reports_slot_metadata_failure_without_omission",
        "test_slot_files_secret_slot_path_returns_empty_without_traversal",
        "test_slot_files_rejects_symlinked_slot_root_directly_without_traversal",
        "test_slot_files_rejects_symlinked_slot_ancestor_directly_without_traversal",
        "test_slot_files_skips_symlinked_artifact_directory_directly_without_traversal",
        "test_slot_files_reports_artifact_directory_metadata_failure_without_omission",
        "test_slot_files_reports_top_level_listing_failure_without_traceback",
        "test_slot_files_reports_artifact_metadata_failure_without_omission",
        "test_artifact_shape_validators_report_top_level_listing_failure",
        "test_verify_sha256_manifest_reports_top_level_listing_failure",
        "test_required_signed_evidence_digest_paths_reports_top_level_listing_failure",
        "test_signer_manifest_rewrite_rejects_top_level_listing_failure",
        "test_load_json_rejects_non_utf8_bytes_without_traceback",
        "test_parse_sha256_manifest_rejects_secret_slot_path_directly_before_parse",
        "test_parse_sha256_manifest_rejects_symlinked_slot_root_directly_before_parse",
        "test_parse_sha256_manifest_rejects_slot_metadata_failure_before_parse",
        "test_parse_sha256_manifest_rejects_symlinked_slot_ancestor_before_parse",
        "test_parse_sha256_manifest_rejects_hardlinked_manifest_before_read",
        "test_parse_sha256_manifest_rejects_file_metadata_failure_before_read",
        "test_parse_sha256_manifest_rejects_hardlink_metadata_failure_before_read",
        "test_parse_sha256_manifest_rejects_non_utf8_bytes_without_traceback",
        "test_parse_sha256_manifest_rejects_regular_file_swap_after_preflight",
        "test_verify_sha256_manifest_rejects_secret_slot_path_directly_before_traversal",
        "test_verify_sha256_manifest_rejects_symlinked_slot_root_directly_before_parse",
        "test_verify_sha256_manifest_rejects_slot_metadata_failure_before_parse",
        "test_verify_sha256_manifest_rejects_symlinked_slot_ancestor_before_discovery",
        "test_verify_sha256_manifest_missing_slot_returns_missing_manifest_without_traceback",
        "test_verify_sha256_manifest_rejects_hardlinked_manifest_before_discovery",
        "test_verify_sha256_manifest_rejects_symlinked_artifact_directory_before_digest_read",
        "test_manifest_artifact_digest_rejects_secret_relative_path_directly",
        "test_manifest_artifact_digest_rejects_symlink_directly",
        "test_manifest_artifact_digest_rejects_hardlink_directly",
        "test_manifest_artifact_digest_rejects_file_metadata_failure",
        "test_manifest_artifact_digest_uses_lstat_before_relative_ancestor_is_symlink_preflight",
        "test_manifest_artifact_digest_rejects_read_failure_after_preflight",
        "test_manifest_artifact_digest_rejects_regular_file_swap_after_preflight",
        "test_verify_sha256_manifest_revalidates_artifact_before_digest",
        "test_attestation_result_rejects_secret_slot_path_directly_before_parse",
        "test_d2d_transcript_rejects_secret_slot_path_directly_before_parse",
        "test_d2d_transcript_binding_rejects_secret_slot_path_directly_before_artifact_read",
        "test_wallet_transcript_binding_rejects_secret_slot_path_directly_before_artifact_read",
        "test_metadata_artifact_digest_rejects_file_metadata_failure",
        "test_metadata_artifact_digest_rejects_read_failure_after_preflight",
        "test_metadata_artifact_digest_rejects_symlink_swap_after_preflight",
        "test_metadata_artifact_digest_rejects_regular_file_swap_after_preflight",
        "test_d2d_transcript_binding_rejects_symlink_path_before_digest_read",
        "test_wallet_transcript_binding_rejects_hardlink_path_before_digest_read",
        "test_d2d_transcript_rejects_symlinked_queue_before_digest_read",
        "test_d2d_transcript_uses_lstat_before_queue_is_file_preflight",
        "test_required_artifact_shapes_rejects_secret_slot_path_directly_before_stat",
        "test_required_artifact_shapes_reports_required_artifact_metadata_failure",
        "test_required_artifact_shapes_uses_lstat_before_is_file_preflight",
        "test_required_status_artifact_rejects_symlink_before_text_read",
        "test_required_runtime_log_rejects_hardlink_before_text_read",
        "test_required_runtime_log_rejects_symlink_swap_after_preflight",
        "test_required_status_artifact_uses_lstat_before_is_file_preflight",
        "test_required_runtime_log_uses_lstat_before_is_file_preflight",
        "telemetry/status.ndjson references symlink artifact",
        "logs/runtime.log references hardlinked artifact",
        "test_slot_symlink_artifact_validator_rejects_secret_slot_path_directly_before_traversal",
        "test_slot_symlink_artifact_validator_reports_slot_metadata_file_metadata_failure",
        "test_slot_symlink_artifact_validator_reports_directory_metadata_failure",
        "test_slot_symlink_artifact_validator_reports_nested_artifact_file_metadata_failure",
        "test_slot_hardlink_artifact_validator_rejects_secret_slot_path_directly_before_stat",
        "test_slot_hardlink_artifact_validator_reports_file_metadata_failure",
        "test_slot_hardlink_artifact_validator_uses_lstat_before_directory_exists_preflight",
        "test_slot_regular_artifact_validator_rejects_secret_slot_path_directly_before_shape",
        "test_slot_regular_artifact_validator_reports_slot_metadata_file_metadata_failure",
        "test_slot_regular_artifact_validator_uses_lstat_before_exists_preflight",
        "test_slot_regular_artifact_validator_reports_directory_metadata_failure",
        "test_slot_regular_artifact_validator_uses_lstat_before_directory_exists_preflight",
        "test_slot_regular_artifact_validator_reports_nested_artifact_file_metadata_failure",
        "test_slot_regular_artifact_validator_uses_lstat_before_nested_symlink_preflight",
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
        "test_production_metadata_rejects_unavailable_recursive_compact_one_hop_probe",
        "test_production_metadata_rejects_generic_recursive_compact_prover_state",
        "test_production_metadata_rejects_signed_evidence_digest_drift",
        "test_production_metadata_uses_lstat_before_signed_evidence_is_file_preflight",
        "test_metadata_artifact_digest_rejects_secret_relative_path_directly",
        "test_production_metadata_rejects_symlinked_signed_evidence_digest_path",
        "test_production_metadata_rejects_hardlinked_release_apk_digest_path",
        "test_production_metadata_rejects_unsafe_signed_evidence_path",
        "test_production_metadata_rejects_signed_evidence_artifact_outside_evidence",
        "test_production_metadata_rejects_noncanonical_signed_evidence_filename",
        "test_scan_slot_rejects_symlinked_slot_directory",
        "test_main_rejects_symlinked_device_lab_root_before_discovery",
        "test_main_rejects_symlinked_device_lab_root_ancestor_before_discovery",
        "test_scan_slot_rejects_symlinked_slot_parent_directory",
        "test_scan_slot_uses_lstat_before_expected_directory_is_dir_preflight",
        "test_scan_slot_reports_expected_directory_metadata_failure_before_is_dir_preflight",
        "test_scan_slot_counts_artifacts_with_lstat_before_is_file_preflight",
        "test_scan_slot_sha_presence_uses_lstat_before_is_file_preflight",
        "test_scan_slot_rejects_symlinked_slot_ancestor_directory",
        "test_scan_slot_rejects_directory_traversal_failure_without_traceback",
        "test_load_json_rejects_symlinked_ancestor_before_read",
        "test_load_json_rejects_symlink_swap_after_preflight",
        "test_load_json_rejects_regular_file_swap_after_preflight",
        "test_validate_no_symlink_ancestors_rejects_cwd_failure",
        "test_validate_no_symlink_ancestors_rejects_ancestor_metadata_failure",
        "test_validate_no_symlink_ancestors_uses_lstat_before_is_symlink_preflight",
        "test_validate_no_symlink_ancestors_uses_lstat_before_exists_preflight",
        "test_load_json_rejects_secret_path_directly_before_parse",
        "test_load_json_rejects_file_metadata_failure_before_missing",
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
        "test_signed_evidence_artifact_digest_rejects_secret_relative_path_directly",
        "test_signed_evidence_artifact_digest_rejects_symlink_directly",
        "test_signed_evidence_artifact_digest_rejects_hardlink_directly",
        "test_signed_evidence_artifact_digest_rejects_file_metadata_failure",
        "test_signed_evidence_artifact_digest_rejects_read_failure_after_preflight",
        "test_signed_evidence_artifact_digest_rejects_regular_file_swap_after_preflight",
        "test_signed_evidence_artifact_revalidates_required_digest_before_read",
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
        "test_root_validator_rejects_metadata_failure_directly_without_leak",
        "test_main_uses_lstat_before_missing_root_exists_preflight",
        "test_main_rejects_secret_looking_root_without_leak",
        "test_json_summary_rejects_secret_looking_output_without_leak",
        "test_write_summary_rejects_secret_output_path_directly_without_leak",
        "test_validate_summary_output_path_uses_lstat_before_parent_is_dir_preflight",
        "test_validate_summary_output_path_rejects_parent_metadata_failure",
        "test_write_summary_uses_lstat_before_parent_is_dir_preflight",
        "test_write_summary_rejects_parent_metadata_failure_before_write",
        "test_write_summary_rejects_parent_create_failure_before_write",
        "test_write_summary_rejects_file_metadata_failure_before_write",
        "test_write_summary_rejects_hardlink_metadata_failure_before_write",
        "test_write_summary_rejects_write_failure_after_preflight",
        "test_write_summary_preserves_existing_output_on_replace_failure",
        "test_write_summary_rejects_symlink_swap_before_replace",
        "test_write_summary_rejects_readback_mismatch",
        "test_write_summary_rejects_readback_failure",
        "test_write_summary_rejects_regular_file_swap_before_readback",
        "test_write_summary_rejects_symlink_swap_after_replace",
        "test_write_summary_rechecks_parent_after_create_before_write",
        "test_json_summary_rejects_symlinked_output_without_following_alias",
        "test_json_summary_rejects_hardlinked_output_without_overwriting_alias",
        "test_standard_matrix_requires_every_kagemusha_device_family",
        "test_standard_matrix_accepts_all_kagemusha_device_families",
        "test_signer_helper_generates_validator_accepted_evidence",
        "test_signer_helper_rejects_mismatched_private_and_public_keys",
        "test_trusted_signer_public_key_rejects_symlink_without_path_leak",
        "test_trusted_signer_public_key_rejects_secret_looking_path_without_leak",
        "test_trusted_signer_public_key_rejects_secret_path_before_openssl_lookup",
        "test_verify_signature_rejects_secret_public_key_path_before_openssl_lookup",
        "test_openssl_public_key_der_rejects_spawn_failure_after_path_shape",
        "test_openssl_public_key_der_rejects_invalid_public_key_after_openssl_failure",
        "test_openssl_public_key_der_rejects_missing_public_key_before_openssl_lookup",
        "test_openssl_public_key_der_rejects_non_regular_public_key_before_openssl_lookup",
        "test_openssl_public_key_der_rejects_file_metadata_failure_before_openssl_lookup",
        "test_verify_signature_rejects_staging_write_failure_before_openssl",
        "test_verify_signature_rejects_payload_staging_readback_mismatch_before_openssl",
        "test_verify_signature_rejects_signature_staging_readback_mismatch_before_openssl",
        "test_write_staged_bytes_rejects_regular_file_swap_before_readback",
        "test_verify_signature_rejects_tempdir_failure_before_staging",
        "test_verify_signature_rejects_spawn_failure_after_staging",
        "test_private_public_pair_preserves_public_key_path_error_before_mismatch",
        "test_trusted_signer_public_key_rejects_symlinked_ancestor_without_path_leak",
        "test_trusted_signer_public_key_rejects_hardlink_without_path_leak",
        "test_trusted_signer_public_key_rejects_hardlink_metadata_failure_before_openssl",
        "test_signer_helper_rejects_symlinked_private_key_before_write",
        "test_signer_helper_rejects_symlinked_private_key_ancestor_before_write",
        "test_signer_helper_rejects_symlinked_public_key_ancestor_before_write",
        "trusted signer public key ancestor directory must not be a symlink",
        "private key ancestor directory must not be a symlink",
        "signer public key ancestor directory must not be a symlink",
        "test_signer_helper_rejects_hardlinked_public_key_before_write",
        "test_signer_helper_rejects_secret_looking_public_key_path_before_write",
        "test_signer_helper_rejects_secret_looking_slot_path_before_metadata_read",
        "test_signer_helper_rejects_slot_directory_metadata_failure_before_read",
        "test_signer_helper_rejects_slot_parent_metadata_failure_before_read",
        "test_signer_helper_rejects_secret_looking_output_before_metadata_read",
        "test_signer_helper_rejects_secret_looking_signer_key_id_before_metadata_read",
        "private key path must not contain secret-looking material",
        "signer public key path must not contain secret-looking material",
        "test_signer_helper_rejects_output_outside_evidence_before_write",
        "test_signer_helper_rejects_noncanonical_output_filename_before_write",
        "test_signer_output_normalise_rejects_output_resolve_failure",
        "test_signer_output_normalise_rejects_slot_resolve_failure",
        "test_signer_write_json_rejects_symlinked_output_parent_before_write",
        "test_signer_write_json_uses_lstat_before_parent_is_dir_preflight",
        "test_signer_write_json_rejects_parent_metadata_failure_before_write",
        "test_signer_write_json_rejects_symlinked_output_ancestor_before_write",
        "test_signer_write_json_rejects_symlinked_output_ancestor_before_creating_parent",
        "test_signer_write_json_rejects_symlinked_output_leaf_before_write",
        "test_signer_write_json_rejects_hardlinked_output_leaf_before_write",
        "test_signer_write_json_rejects_hardlink_metadata_failure_before_write",
        "test_signer_write_json_rejects_file_metadata_failure_before_write",
        "test_signer_write_json_rejects_secret_output_path_directly_without_write",
        "test_signer_write_json_rejects_write_failure_after_preflight",
        "test_signer_write_json_preserves_existing_output_on_replace_failure",
        "test_signer_write_json_rejects_symlink_swap_before_replace",
        "test_signer_write_json_rejects_readback_mismatch",
        "test_signer_write_json_rejects_readback_failure",
        "test_signer_write_json_rejects_regular_file_swap_before_readback",
        "test_signer_write_json_rejects_symlink_swap_after_replace",
        "test_signer_write_json_rejects_parent_create_failure_before_write",
        "test_signer_write_json_rechecks_parent_after_create_before_write",
        "test_signer_output_digest_rejects_secret_path_directly_without_read",
        "test_signer_output_digest_rejects_missing_parent_before_read",
        "test_signer_output_digest_uses_lstat_before_parent_is_dir_preflight",
        "test_signer_output_digest_rejects_parent_metadata_failure_before_read",
        "test_signer_output_digest_rejects_missing_leaf_before_read",
        "test_signer_output_digest_rejects_symlinked_leaf_after_write",
        "test_signer_output_digest_rejects_hardlinked_leaf_after_write",
        "test_signer_output_digest_rejects_hardlink_metadata_failure_after_write",
        "test_signer_output_digest_rejects_file_metadata_failure_after_write",
        "test_signer_output_digest_rejects_read_failure_after_preflight",
        "test_signer_output_digest_rejects_regular_file_swap_after_preflight",
        "test_signer_helper_revalidates_output_digest_before_slot_json_update",
        "test_signer_write_text_rejects_symlinked_manifest_leaf_before_write",
        "test_signer_write_text_rejects_dangling_symlinked_manifest_leaf_before_write",
        "test_signer_write_text_rejects_hardlinked_manifest_leaf_before_write",
        "test_signer_write_text_rejects_secret_manifest_path_directly_without_write",
        "test_signer_write_text_rejects_write_failure_after_preflight",
        "test_signer_write_text_preserves_existing_output_on_replace_failure",
        "test_signer_write_text_rejects_symlink_swap_before_replace",
        "test_signer_write_text_rejects_readback_mismatch",
        "test_signer_write_text_rejects_readback_failure",
        "test_signer_write_text_rejects_symlink_swap_after_replace",
        "test_rewrite_sha256_manifest_rejects_symlinked_artifact_when_called_directly",
        "test_rewrite_sha256_manifest_rejects_hardlinked_manifest_when_called_directly",
        "test_rewrite_sha256_manifest_rejects_secret_looking_artifact_when_called_directly",
        "test_rewrite_sha256_manifest_rejects_secret_slot_path_directly_without_write",
        "test_rewrite_sha256_manifest_rejects_slot_directory_metadata_failure_without_write",
        "test_rewrite_sha256_manifest_rejects_slot_parent_metadata_failure_without_write",
        "test_signer_slot_artifact_digest_rejects_secret_relative_path_directly",
        "test_signer_slot_artifact_digest_rejects_symlink_directly",
        "test_signer_slot_artifact_digest_rejects_hardlink_directly",
        "test_signer_slot_artifact_digest_rejects_hardlink_metadata_failure_after_preflight",
        "test_signer_slot_artifact_digest_rejects_file_metadata_failure_after_preflight",
        "test_signer_slot_artifact_digest_rejects_read_failure_after_preflight",
        "test_signer_slot_artifact_digest_rejects_regular_file_swap_after_preflight",
        "test_rewrite_sha256_manifest_revalidates_artifact_before_digest",
        "test_signer_metadata_loader_rejects_secret_slot_path_directly_without_parse",
        "test_signer_artifact_digests_rejects_secret_slot_path_directly_before_hash",
        "test_signer_artifact_digests_rejects_symlinked_slot_ancestor_before_hash",
        "test_signer_artifact_digests_rejects_symlinked_artifact_directory_before_hash",
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
        "test_sign_ed25519_rejects_secret_private_key_path_before_openssl_lookup",
        "test_sign_ed25519_rejects_missing_private_key_before_openssl_lookup",
        "test_sign_ed25519_rejects_non_regular_private_key_before_openssl_lookup",
        "test_sign_ed25519_rejects_private_key_file_metadata_failure_before_openssl",
        "test_sign_ed25519_rejects_private_key_hardlink_metadata_failure_before_openssl",
        "test_sign_ed25519_rejects_payload_staging_write_failure_before_openssl",
        "test_sign_ed25519_rejects_payload_staging_readback_mismatch_before_openssl",
        "test_sign_ed25519_rejects_signature_read_failure_after_openssl",
        "test_sign_ed25519_rejects_signature_output_swap_after_openssl",
        "test_sign_ed25519_rejects_short_signature_output_after_openssl",
        "test_sign_ed25519_rejects_tempdir_failure_before_payload_staging",
        "test_sign_ed25519_rejects_spawn_failure_after_payload_staging",
        "test_sign_ed25519_rejects_invalid_private_key_after_openssl_failure",
        "test_sign_ed25519_rejects_signature_read_failure_after_openssl",
        "test_sign_ed25519_rejects_short_signature_output_after_openssl",
    ),
    "scripts/tests/kagemusha_production_readiness_test.py": (
        "test_complete_signed_android_matrix_passes_rollup",
        "summary[\"lineage_proof_evidence\"][\"generated_at_utc\"]",
        "test_missing_android_root_blocks_rollup",
        "test_missing_android_root_uses_lstat_before_exists_preflight",
        "test_missing_standard_family_blocks_rollup",
        "test_duplicate_device_fingerprint_blocks_rollup",
        "test_duplicate_attestation_challenge_blocks_rollup",
        "test_stale_signed_evidence_blocks_rollup",
        "test_future_signed_evidence_blocks_rollup",
        "test_signed_evidence_freshness_uses_validated_report_timestamp",
        "test_signed_evidence_freshness_requires_report_timestamp",
        "test_signed_evidence_freshness_rejects_noncanonical_report_timestamp",
        "test_signed_evidence_freshness_redacts_noncanonical_secret_timestamp",
        "test_duplicate_signed_evidence_json_key_blocks_rollup",
        "test_explicit_missing_slot_blocks_without_traceback",
        "test_unsafe_slot_id_blocks_rollup_without_path_escape",
        "test_untrusted_signed_evidence_blocks_rollup",
        "summary[\"android_device_lab\"][\"signed_evidence\"]",
        "expected_android_signed_evidence",
        "test_abi6_manifest_drift_blocks_rollup_section",
        "test_abi6_manifest_rejects_symlinked_manifest_file",
        "test_abi6_manifest_rejects_symlink_swap_after_preflight",
        "test_abi6_manifest_rejects_symlinked_manifest_ancestor",
        "test_abi6_manifest_rejects_hardlinked_manifest_file",
        "test_abi6_manifest_rejects_non_utf8_without_traceback",
        "test_abi6_manifest_rejects_nonfinite_json_constant",
        "test_release_local_json_validator_rejects_secret_path_directly_without_parse",
        "test_release_local_json_validator_rejects_hardlink_metadata_failure_before_parse",
        "test_release_local_json_validator_rejects_file_metadata_failure_before_parse",
        "test_repo_source_marker_validator_rejects_secret_path_directly_without_metadata",
        "test_repo_source_marker_text_rejects_symlink_directly_before_read",
        "test_repo_source_marker_text_rejects_symlink_swap_after_preflight",
        "test_repo_source_marker_text_rejects_regular_file_swap_after_preflight",
        "test_repo_source_marker_text_rejects_hardlink_directly_before_read",
        "test_repo_source_marker_text_rejects_hardlink_metadata_failure_before_read",
        "test_repo_source_marker_text_rejects_file_metadata_failure_before_read",
        "test_repo_source_marker_text_rejects_non_utf8_without_traceback",
        "test_abi7_fail_closed_rejects_symlinked_source_marker_file",
        "test_abi7_fail_closed_rejects_hardlinked_source_marker_file",
        "test_abi7_fail_closed_rejects_non_utf8_source_marker_without_traceback",
        "test_rust_function_body_ignores_braces_inside_strings_and_comments",
        "test_abi7_fail_closed_accepts_strict_function_contracts",
        "test_abi7_fail_closed_rejects_one_hop_runtime_keygen_fallback",
        "test_abi7_fail_closed_rejects_append_runtime_keygen_fallback",
        "test_abi7_fail_closed_rejects_preverify_contract_without_unavailable_error",
        "test_abi7_fail_closed_rejects_verify_contract_without_backend_call",
        "test_abi7_fail_closed_rejects_bridge_contract_without_unavailable_mapping",
        "test_lineage_key_release_tooling_drift_blocks_rollup_section",
        "test_lineage_key_release_tooling_rejects_symlinked_marker_file",
        "test_lineage_key_release_tooling_rejects_hardlinked_marker_file",
        "test_lineage_key_release_tooling_rejects_marker_regular_file_swap_after_preflight",
        "test_lineage_key_release_tooling_rejects_non_utf8_marker_without_traceback",
        "test_missing_compact_key_evidence_blocks_rollup_section",
        "test_compact_key_evidence_rejects_noncanonical_filename",
        "test_compact_key_evidence_rejects_symlinked_evidence_file",
        "test_compact_key_evidence_rejects_json_symlink_swap_after_preflight",
        "test_compact_key_evidence_rejects_duplicate_json_keys",
        "test_compact_key_evidence_rejects_secret_duplicate_json_key",
        "test_compact_key_evidence_rejects_nonfinite_json_constant",
        "test_stale_compact_key_evidence_blocks_rollup_section",
        "test_compact_key_evidence_rejects_noncanonical_timestamp",
        "test_future_compact_key_evidence_blocks_rollup_section",
        "test_compact_key_evidence_drift_blocks_rollup_section",
        "test_compact_key_evidence_rejects_float_scalar_claims",
        "test_compact_key_evidence_rejects_missing_artifact_size_map",
        "test_compact_key_evidence_rejects_artifact_size_drift",
        "test_compact_key_evidence_rejects_missing_generator_log",
        "test_compact_key_evidence_rejects_generator_log_digest_drift",
        "test_compact_key_evidence_rejects_generator_log_artifact_digest_drift",
        "test_compact_key_evidence_rejects_generator_log_extra_lines",
        "test_compact_key_evidence_rejects_generator_log_trailing_whitespace",
        "test_compact_key_evidence_rejects_generator_log_crlf_line_endings",
        "test_compact_key_evidence_rejects_generator_log_without_final_lf",
        "test_compact_key_evidence_rejects_generator_log_invalid_utf8_bytes",
        "test_compact_key_evidence_rejects_generator_log_symlink_swap_after_preflight",
        "test_compact_key_evidence_rejects_noncanonical_generator_log_path",
        "test_compact_key_evidence_rejects_secret_size_field_without_leak",
        "test_compact_key_evidence_rejects_appended_shell_command",
        "test_compact_key_evidence_rejects_shell_equivalent_noncanonical_command",
        "test_compact_key_evidence_rejects_secret_looking_command_without_leak",
        "test_compact_key_evidence_rejects_unexpected_secret_field_without_leak",
        "test_compact_key_evidence_redacts_secret_required_scalars_in_full_result",
        "test_compact_key_evidence_rejects_missing_local_artifact_file",
        "test_compact_key_evidence_rejects_symlinked_local_artifact_file",
        "test_compact_key_evidence_rejects_hardlinked_local_artifact_file",
        "test_compact_key_evidence_rejects_artifact_symlink_swap_after_preflight",
        "test_compact_key_evidence_rejects_local_artifact_digest_mismatch",
        "test_compact_key_evidence_rejects_empty_local_artifact_file",
        "test_compact_key_evidence_rejects_placeholder_local_artifact_file",
        "test_compact_key_evidence_placeholder_check_uses_hashed_prefix",
        "test_compact_key_evidence_rejects_all_placeholder_prefixes",
        "test_compact_key_evidence_rejects_all_zero_local_artifact_file",
        "test_compact_key_evidence_helper_generates_validator_accepted_json",
        "test_compact_key_evidence_helper_rejects_missing_artifact",
        "test_compact_key_evidence_helper_rejects_empty_artifact",
        "test_compact_key_evidence_helper_rejects_artifact_symlink_swap_after_preflight",
        "test_compact_key_evidence_helper_rejects_artifact_regular_file_swap_after_preflight",
        "test_compact_key_evidence_helper_rejects_placeholder_artifact",
        "test_compact_key_evidence_helper_placeholder_check_uses_hashed_prefix",
        "test_compact_key_evidence_helper_rejects_all_placeholder_prefixes",
        "test_compact_key_evidence_helper_rejects_all_zero_artifact",
        "test_compact_key_evidence_helper_rejects_missing_generator_log",
        "test_compact_key_evidence_helper_rejects_generator_log_size_drift",
        "test_compact_key_evidence_helper_rejects_generator_log_digest_drift",
        "test_compact_key_evidence_helper_rejects_generator_log_trailing_whitespace",
        "test_compact_key_evidence_helper_rejects_generator_log_crlf_line_endings",
        "test_compact_key_evidence_helper_rejects_generator_log_without_final_lf",
        "test_compact_key_evidence_helper_rejects_generator_log_invalid_utf8_bytes",
        "test_compact_key_evidence_helper_rejects_noncanonical_generated_at_utc",
        "test_compact_key_evidence_helper_rejects_appended_shell_command",
        "test_compact_key_evidence_helper_rejects_outside_artifact_dir",
        "test_compact_key_evidence_helper_rejects_symlinked_output_leaf",
        "test_compact_key_evidence_helper_rejects_dangling_symlinked_output_leaf",
        "test_compact_key_output_preflight_rejects_parent_create_failure_before_write",
        "test_compact_key_output_preflight_rejects_file_metadata_failure_before_write",
        "test_compact_key_output_preflight_rejects_hardlink_metadata_failure_before_write",
        "test_compact_key_write_evidence_rejects_write_failure_after_preflight",
        "test_compact_key_write_evidence_preserves_existing_output_on_replace_failure",
        "test_compact_key_write_evidence_rejects_readback_mismatch",
        "test_compact_key_write_evidence_rejects_readback_failure",
        "test_compact_key_write_evidence_rejects_regular_file_swap_before_readback",
        "test_compact_key_write_evidence_rejects_symlink_swap_before_replace",
        "test_compact_key_write_evidence_rejects_symlink_swap_after_replace",
        "test_compact_key_evidence_document_validator_rejects_artifact_dir_create_failure_after_preflight",
        "test_compact_key_evidence_document_validator_rejects_temp_write_failure_after_preflight",
        "test_compact_key_evidence_document_validator_rejects_temp_cleanup_failure",
        "test_compact_key_artifact_dir_validator_rejects_secret_path_directly",
        "test_compact_key_artifact_dir_validator_rejects_metadata_failure_directly",
        "test_compact_key_sha256_file_rejects_secret_path_directly",
        "test_compact_key_sha256_file_rejects_symlink_directly",
        "test_compact_key_sha256_file_rejects_hardlink_directly",
        "test_compact_key_sha256_file_rejects_read_failure_without_traceback",
        "test_missing_lineage_proof_evidence_blocks_rollup_section",
        "test_lineage_proof_evidence_rejects_noncanonical_filename",
        "test_lineage_proof_evidence_rejects_symlinked_evidence_file",
        "test_lineage_proof_evidence_rejects_json_symlink_swap_after_preflight",
        "test_lineage_proof_evidence_rejects_symlinked_evidence_ancestor",
        "test_lineage_proof_evidence_rejects_secret_path_before_json_parse",
        "test_lineage_proof_evidence_rejects_non_utf8_without_traceback",
        "test_lineage_proof_evidence_rejects_duplicate_json_keys",
        "test_lineage_proof_evidence_redacts_secret_duplicate_json_key",
        "test_lineage_proof_evidence_rejects_nonfinite_json_constant",
        "test_stale_lineage_proof_evidence_blocks_rollup_section",
        "test_lineage_proof_evidence_rejects_noncanonical_timestamp",
        "test_future_lineage_proof_evidence_blocks_rollup_section",
        "test_lineage_proof_evidence_drift_blocks_rollup_section",
        "test_lineage_proof_evidence_rejects_float_scalar_claims",
        "test_lineage_proof_evidence_rejects_missing_artifact_size_map",
        "test_lineage_proof_evidence_rejects_artifact_size_drift",
        "test_lineage_proof_evidence_rejects_secret_size_field_without_leak",
        "test_lineage_proof_evidence_rejects_runtime_keygen_command",
        "test_lineage_proof_evidence_rejects_fake_runner_command",
        "test_lineage_proof_evidence_rejects_appended_shell_command",
        "test_lineage_proof_evidence_rejects_shell_equivalent_noncanonical_command",
        "test_lineage_proof_evidence_rejects_secret_looking_command_without_leak",
        "test_lineage_proof_evidence_redacts_secret_required_scalars_in_full_result",
        "test_lineage_proof_evidence_rejects_missing_local_artifact_file",
        "test_lineage_proof_evidence_rejects_symlinked_local_artifact_file",
        "test_lineage_proof_evidence_rejects_hardlinked_local_artifact_file",
        "test_lineage_proof_evidence_rejects_local_artifact_digest_mismatch",
        "test_lineage_proof_evidence_rejects_empty_local_artifact_file",
        "test_lineage_proof_evidence_rejects_all_zero_local_artifact_file",
        "test_lineage_proof_evidence_placeholder_check_uses_hashed_prefix",
        "test_lineage_proof_evidence_uses_local_file_validation_before_artifact_is_file_preflight",
        "summary[\"lineage_proof_evidence\"][\"artifact_sha256\"]",
        "summary[\"lineage_proof_evidence\"][\"artifact_size_bytes\"]",
        "summary[\"lineage_proof_evidence\"][\"test_log_sha256\"]",
        "summary[\"compact_key_evidence\"][\"artifact_sha256\"]",
        "summary[\"compact_key_evidence\"][\"artifact_size_bytes\"]",
        "summary[\"compact_key_evidence\"][\"generator_log_artifact_sha256\"]",
        "summary[\"compact_key_evidence\"][\"command_validated\"]",
        "test_kagemusha_release_bundle_manifest_passes_ready_fixture",
        "test_kagemusha_release_bundle_verify_existing_passes_ready_fixture",
        "test_kagemusha_release_bundle_verify_existing_allows_timestamp_refresh",
        "test_kagemusha_release_bundle_verify_existing_rejects_manifest_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_positive_evidence_size_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_generator_log_artifact_digest_drift",
        "test_kagemusha_release_bundle_rejects_generator_log_artifact_digest_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_digest_matched_invalid_utf8_proof_log",
        "test_kagemusha_release_bundle_verify_existing_rejects_digest_matched_invalid_utf8_generator_log",
        "test_kagemusha_release_bundle_verify_existing_rejects_unexpected_field",
        "test_kagemusha_release_bundle_verify_existing_rejects_secret_material",
        "test_kagemusha_release_bundle_verify_existing_rejects_unsafe_evidence_path",
        "test_kagemusha_release_bundle_verify_existing_rejects_nonstring_evidence_path",
        "test_kagemusha_release_bundle_verify_existing_rejects_noncanonical_evidence_path",
        "test_kagemusha_release_bundle_verify_existing_rejects_malformed_evidence_sha256",
        "test_kagemusha_release_bundle_verify_existing_rejects_noninteger_evidence_size",
        "test_kagemusha_release_bundle_verify_existing_rejects_boolean_evidence_size",
        "test_kagemusha_release_bundle_verify_existing_rejects_zero_evidence_size",
        "test_kagemusha_release_bundle_verify_existing_rejects_missing_evidence_size",
        "test_kagemusha_release_bundle_rejects_empty_compact_generator_log_inventory",
        "test_kagemusha_release_bundle_verify_existing_rejects_duplicate_manifest_json_key",
        "test_kagemusha_release_bundle_verify_existing_rejects_nonfinite_manifest_json_constant",
        "test_kagemusha_release_bundle_verify_existing_rejects_noncanonical_manifest_timestamp",
        "test_kagemusha_release_bundle_load_local_json_rejects_symlink_swap_after_preflight",
        "test_kagemusha_release_bundle_verify_existing_rejects_bundle_root_symlink_before_manifest_load",
        "test_kagemusha_release_bundle_verify_existing_rejects_outside_manifest_before_scanners",
        "lineage_artifacts",
        "compact_key_artifacts",
        "lineage_proof_logs",
        "android_slot_artifacts",
        "test_kagemusha_release_bundle_rejects_missing_android_slot_apk_after_validation",
        "test_kagemusha_release_bundle_rejects_android_slot_attestation_digest_drift",
        "test_kagemusha_release_bundle_rejects_android_slot_d2d_transcript_digest_drift",
        "test_kagemusha_release_bundle_rejects_android_slot_wallet_transcript_digest_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_missing_android_slot_artifacts",
        "test_kagemusha_release_bundle_artifact_inventory_rejects_digest_drift",
        "test_kagemusha_release_bundle_artifact_inventory_rejects_size_drift",
        "test_kagemusha_release_bundle_artifact_inventory_rejects_outside_bundle_root",
        "test_kagemusha_release_bundle_evidence_entry_rejects_symlink_swap_after_preflight",
        "test_kagemusha_release_bundle_json_input_rejects_regular_file_swap_after_preflight",
        "test_kagemusha_release_bundle_digest_rejects_regular_file_swap_after_preflight",
        "test_kagemusha_release_bundle_evidence_entry_rejects_regular_file_swap_after_preflight",
        "test_kagemusha_release_bundle_rejects_blocked_summary",
        "test_kagemusha_release_bundle_rejects_unexpected_android_signed_evidence_summary_field",
        "test_kagemusha_release_bundle_rejects_missing_android_signed_evidence_summary_field",
        "test_kagemusha_release_bundle_rejects_nonobject_android_signed_evidence_summary_entry",
        "test_kagemusha_release_bundle_rejects_unsafe_android_signed_evidence_summary_slot_without_leak",
        "test_kagemusha_release_bundle_rejects_malformed_android_signed_evidence_summary_sha256",
        "test_kagemusha_release_bundle_rejects_unsafe_android_signed_evidence_summary_path_without_leak",
        "test_kagemusha_release_bundle_rejects_noncanonical_android_signed_evidence_summary_timestamp",
        "test_kagemusha_release_bundle_rejects_all_zero_lineage_artifact",
        "test_kagemusha_release_bundle_rejects_placeholder_compact_artifact",
        "test_kagemusha_release_bundle_rejects_all_placeholder_compact_prefixes",
        "test_kagemusha_release_bundle_rejects_all_zero_compact_artifact",
        "kagemusha_release_lineage_artifact_placeholder",
        "kagemusha_release_compact_artifact_placeholder",
        "compact_key_generator_log",
        "kagemusha_release_compact_generator_log_digest_drift",
        "test_kagemusha_release_bundle_rejects_summary_digest_drift",
        "test_kagemusha_release_bundle_rejects_digest_matched_invalid_utf8_proof_log",
        "test_kagemusha_release_bundle_rejects_digest_matched_invalid_utf8_generator_log",
        "test_kagemusha_release_bundle_rejects_lineage_size_drift",
        "test_kagemusha_release_bundle_rejects_android_summary_drift",
        "test_kagemusha_release_bundle_rejects_abi6_summary_drift",
        "test_kagemusha_release_bundle_rejects_abi7_summary_drift",
        "test_kagemusha_release_bundle_rejects_lineage_tooling_summary_drift",
        "test_kagemusha_release_bundle_rejects_wrong_repo_root",
        "test_kagemusha_release_bundle_rejects_unexpected_summary_field",
        "test_kagemusha_release_bundle_rejects_unexpected_summary_section_field",
        "test_kagemusha_release_bundle_rejects_ready_summary_section_blockers",
        "test_kagemusha_release_bundle_rejects_secret_summary_material_without_leak",
        "test_kagemusha_release_bundle_rejects_duplicate_summary_json_key",
        "test_kagemusha_release_bundle_rejects_nonfinite_summary_json_constant",
        "test_abi6_manifest_rejects_nonfinite_json_constant",
        "test_kagemusha_release_bundle_rejects_evidence_outside_bundle_root",
        "test_kagemusha_release_bundle_rejects_outside_summary_before_json_load",
        "test_kagemusha_release_bundle_rejects_outside_evidence_before_scanners",
        "test_kagemusha_release_bundle_rejects_output_overwriting_evidence",
        "test_write_release_bundle_preserves_existing_output_on_replace_failure",
        "test_write_release_bundle_rejects_readback_mismatch",
        "test_write_release_bundle_rejects_readback_failure",
        "test_write_release_bundle_rejects_regular_file_swap_before_readback",
        "test_write_release_bundle_rejects_symlink_swap_after_replace",
        "assert_not_called",
        "lineage evidence must not be scanned",
        "compact evidence must not be scanned",
        "device lab must not be scanned",
        "test_kagemusha_release_bundle_rejects_android_evidence_outside_bundle_root",
        "test_kagemusha_release_bundle_rejects_forged_android_slot_escape",
        "test_kagemusha_release_bundle_rejects_secret_android_slot_without_leak",
        "test_kagemusha_release_bundle_rejects_output_symlink",
        "test_kagemusha_release_bundle_rejects_output_hardlink",
        "test_kagemusha_release_bundle_rejects_output_parent_symlink_after_create",
        "test_kagemusha_release_bundle_rejects_bundle_root_symlink",
        "test_kagemusha_release_bundle_rejects_bundle_root_symlink_ancestor_without_leak",
        "test_kagemusha_release_bundle_rejects_secret_summary_path_without_leak",
        "test_kagemusha_release_bundle_rejects_secret_repo_root_without_leak",
        "test_kagemusha_release_bundle_rejects_missing_trusted_signer",
        "test_kagemusha_release_bundle_rejects_secret_signer_path_before_load",
        "release_bundle.RELEASE_BUNDLE_SCHEMA",
        "test_lineage_proof_evidence_rejects_missing_local_proof_log_file",
        "test_lineage_proof_evidence_uses_log_validation_before_is_file_preflight",
        "test_lineage_proof_evidence_rejects_symlinked_local_proof_log_file",
        "test_lineage_proof_evidence_rejects_hardlinked_local_proof_log_file",
        "test_lineage_proof_log_rejects_secret_path_before_digest",
        "test_lineage_proof_log_rejects_metadata_read_failure_after_preflight",
        "test_lineage_proof_log_rejects_symlink_swap_after_preflight",
        "test_lineage_proof_log_rejects_trailing_whitespace_on_required_lines",
        "test_lineage_proof_log_rejects_crlf_line_endings",
        "test_lineage_proof_log_rejects_missing_final_lf",
        "test_lineage_proof_log_rejects_invalid_utf8_bytes",
        "test_lineage_proof_evidence_rejects_digest_matched_crlf_proof_log",
        "test_lineage_proof_evidence_rejects_digest_matched_invalid_utf8_proof_log",
        "test_lineage_proof_evidence_rejects_digest_matched_missing_final_lf",
        "test_lineage_local_text_rejects_symlink_directly_before_read",
        "test_lineage_local_text_rejects_hardlink_directly_before_read",
        "test_lineage_local_text_rejects_regular_file_swap_after_preflight",
        "test_lineage_readiness_sha256_file_rejects_secret_path_directly",
        "test_lineage_readiness_sha256_file_rejects_symlink_directly",
        "test_lineage_readiness_sha256_file_rejects_hardlink_directly",
        "test_lineage_readiness_sha256_file_rejects_regular_file_swap_after_preflight",
        "test_lineage_readiness_sha256_file_rejects_hardlink_metadata_failure_directly",
        "test_lineage_readiness_sha256_file_rejects_file_metadata_failure_directly",
        "test_lineage_readiness_sha256_file_rejects_read_failure_without_traceback",
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
        "test_lineage_proof_evidence_document_validator_rejects_artifact_dir_create_failure_after_preflight",
        "test_lineage_proof_evidence_document_validator_rejects_temp_write_failure_after_preflight",
        "test_lineage_proof_evidence_document_validator_rejects_temp_cleanup_failure",
        "test_lineage_proof_artifact_dir_validator_rejects_secret_path_directly",
        "test_lineage_proof_artifact_dir_validator_rejects_metadata_failure_directly",
        "test_lineage_proof_sha256_file_rejects_secret_path_directly",
        "test_lineage_proof_sha256_file_rejects_symlink_directly",
        "test_lineage_proof_sha256_file_rejects_hardlink_directly",
        "test_lineage_proof_sha256_file_rejects_read_failure_without_traceback",
        "test_lineage_proof_evidence_helper_rejects_artifact_symlink_swap_after_preflight",
        "test_lineage_proof_evidence_helper_rejects_artifact_regular_file_swap_after_preflight",
        "test_lineage_proof_evidence_rejects_artifact_symlink_swap_after_preflight",
        "test_lineage_proof_evidence_helper_rejects_missing_artifact",
        "test_lineage_proof_evidence_helper_rejects_empty_artifact",
        "test_lineage_proof_evidence_helper_rejects_all_zero_artifact",
        "test_lineage_proof_evidence_helper_placeholder_check_uses_hashed_prefix",
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
        "test_lineage_proof_output_corridor_rejects_parent_resolve_failure",
        "test_lineage_proof_output_corridor_rejects_artifact_dir_resolve_failure",
        "test_lineage_proof_evidence_helper_rejects_noncanonical_output_filename",
        "test_lineage_proof_evidence_helper_rejects_symlinked_artifact_dir",
        "test_lineage_proof_evidence_helper_preflights_output_ancestor_before_artifact_reads",
        "test_lineage_proof_evidence_helper_rejects_symlinked_output_ancestor",
        "test_lineage_proof_output_validator_rejects_symlinked_ancestor_before_creating_parent",
        "test_lineage_proof_output_preflight_rejects_secret_path_directly_before_creating_parent",
        "test_lineage_proof_output_preflight_uses_lstat_before_parent_is_dir_preflight",
        "test_lineage_proof_output_preflight_rejects_parent_metadata_failure_before_write",
        "test_lineage_proof_output_validator_uses_lstat_before_parent_is_dir_preflight",
        "test_lineage_proof_output_validator_rejects_parent_metadata_failure_before_write",
        "test_lineage_proof_output_preflight_rejects_parent_create_failure_before_write",
        "test_lineage_proof_output_preflight_rechecks_parent_after_create",
        "test_lineage_proof_output_validator_rejects_parent_create_failure_after_preflight",
        "test_lineage_proof_output_preflight_rejects_file_metadata_failure_before_write",
        "test_lineage_proof_output_preflight_rejects_hardlink_metadata_failure_before_write",
        "test_lineage_proof_write_evidence_rejects_secret_output_path_before_write",
        "test_lineage_proof_write_evidence_rejects_write_failure_after_preflight",
        "test_lineage_proof_write_evidence_preserves_existing_output_on_replace_failure",
        "test_lineage_proof_write_evidence_rejects_readback_mismatch",
        "test_lineage_proof_write_evidence_rejects_readback_failure",
        "test_lineage_proof_write_evidence_rejects_regular_file_swap_before_readback",
        "test_lineage_proof_write_evidence_rejects_symlink_swap_before_replace",
        "test_lineage_proof_write_evidence_rejects_symlink_swap_after_replace",
        "test_lineage_proof_evidence_helper_rejects_symlinked_output_leaf",
        "test_lineage_proof_evidence_helper_rejects_dangling_symlinked_output_leaf",
        "test_lineage_proof_evidence_helper_rejects_hardlinked_output_leaf",
        "test_lineage_proof_evidence_helper_rejects_detached_proof_log",
        "test_lineage_proof_build_evidence_rejects_detached_proof_log_directly",
        "test_lineage_proof_build_evidence_rejects_secret_looking_proof_log_before_reads",
        "test_lineage_proof_input_validator_rejects_secret_proof_log_directly_before_resolve",
        "test_lineage_proof_input_validator_rejects_parent_resolve_failure",
        "test_lineage_proof_evidence_helper_rejects_log_without_test_name",
        "test_lineage_proof_evidence_helper_rejects_marker_stuffed_proof_log",
        "test_lineage_proof_evidence_helper_rejects_failed_proof_log",
        "test_summary_does_not_leak_trusted_signer_key_paths",
        "test_summary_does_not_leak_device_lab_root_path",
        "test_secret_looking_device_lab_root_blocks_without_leak",
        "test_android_root_discovery_failure_blocks_rollup_without_traceback",
        "test_validate_repo_root_rejects_secret_path_directly_without_leak",
        "test_validate_repo_root_rejects_metadata_failure_directly_without_leak",
        "test_main_rejects_repo_root_resolve_failure_without_traceback",
        "test_trust_root_sections_reject_secret_repo_root_before_reads",
        "test_symlinked_repo_root_blocks_before_rollup_without_path_leak",
        "test_symlinked_repo_root_ancestor_blocks_before_rollup_without_path_leak",
        "test_symlinked_android_root_blocks_rollup_without_path_leak",
        "test_symlinked_android_root_ancestor_blocks_rollup_without_path_leak",
        "test_android_report_secret_material_is_redacted_before_summary",
        "test_secret_looking_summary_out_blocks_before_write_without_leak",
        "test_write_summary_rejects_secret_path_before_direct_write",
        "test_write_summary_rejects_non_regular_output_leaf_before_write",
        "test_validate_summary_output_path_uses_lstat_before_parent_is_dir_preflight",
        "test_validate_summary_output_path_rejects_parent_metadata_failure",
        "test_write_summary_uses_lstat_before_parent_is_dir_preflight",
        "test_write_summary_rejects_parent_metadata_failure_before_write",
        "test_write_summary_rejects_file_metadata_failure_before_write",
        "test_write_summary_rejects_hardlink_metadata_failure_before_write",
        "test_write_summary_rejects_write_failure_after_preflight",
        "test_write_summary_preserves_existing_output_on_replace_failure",
        "test_write_summary_rejects_symlink_swap_before_replace",
        "test_write_summary_rejects_readback_mismatch",
        "test_write_summary_rejects_readback_failure",
        "test_write_summary_rejects_regular_file_swap_before_readback",
        "test_write_summary_rejects_symlink_swap_after_replace",
        "test_write_summary_rejects_parent_create_failure_before_write",
        "test_write_summary_rechecks_parent_after_create_before_write",
        "test_symlinked_summary_out_blocks_without_following_alias",
        "test_dangling_symlinked_summary_out_blocks_without_following_alias",
        "test_symlinked_summary_out_ancestor_blocks_before_creating_parent",
        "test_hardlinked_summary_out_blocks_without_overwriting_alias",
        "test_secret_looking_trusted_signer_path_blocks_without_leak",
        "test_negative_lineage_proof_future_skew_blocks_before_rollup",
        "test_negative_compact_key_future_skew_blocks_before_rollup",
        "test_secret_looking_compact_key_evidence_path_blocks_without_leak",
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
            "multi-hop proving requires the append verifier batch to be composed into the compact proof",
            "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_OPENING_LEN",
            "KAGEMUSHA_RECURSIVE_COMPACT_MIN_PROOF_BYTES",
            "prove_halo2_ipa_kagemusha_recursive_compact_payment_token_one_hop_envelope",
            "prove_halo2_ipa_kagemusha_recursive_compact_payment_token_one_hop_envelope_dispatch",
            "kagemusha_recursive_spend_lineage_runtime_keygen_enabled()",
            "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_KEY_ARTIFACTS_REQUIRED",
            "missing compact one-hop proving key archive",
            "missing compact append proving key archive",
            "height-aware detached compact Pallas archive must reject before proving",
            "height-aware extra compact Pallas opening must reject before proving",
            "height-aware missing compact Pallas opening must reject before proving",
            "duplicated multi-hop compact Pallas archive must reject before proving",
            "height-aware duplicated multi-hop compact Pallas archive must reject before proving",
            "forged multi-hop compact Pallas metadata must reject before proving",
            "height-aware forged multi-hop compact Pallas metadata must reject before proving",
            "reordered multi-hop compact Pallas archive must reject before proving",
            "height-aware reordered multi-hop compact Pallas archive must reject before proving",
            "attach_recursive_compact_one_hop_zk1_instance_envelope",
            "public ABI-7 compact token one-hop shape preverification",
            "dummy ABI-7 compact proof body must fail the compact proof-size floor before backend verification",
            "decode_kagemusha_recursive_compact_pallas_open_envelopes",
            "failed to decode Kagemusha recursive compact Pallas open-envelope archive",
            "invalid Kagemusha recursive compact Pallas open-envelope archive",
            "invalid Kagemusha recursive compact record-backed Pallas preflight",
            "fn kagemusha_recursive_compact_record_prover_preflights_pallas_archive_before_unavailable",
        "record-bound multi-hop compact Pallas archive must produce a token",
        "record-backed LEN=4 evidence binds to one-hop verifier-slice metadata",
        "one-hop verifier-slice evidence binding must reject proof-count splice",
        "one-hop verifier-slice evidence binding must reject witness-profile splice",
        "one-hop verifier-slice evidence binding must reject params fingerprint splice",
        "one-hop verifier-slice evidence binding must reject schedule digest splice",
        "one-hop verifier-slice evidence binding must reject shared-table manifest splice",
            "one-hop verifier-slice open-envelope evidence must reject params splice",
            "verifier parameter fingerprint mismatch",
            "fixed-window schedule digest mismatch",
            "shared-table manifest digest mismatch",
            "pub fn verify_kagemusha_recursive_compact_payment_token(",
            "verify_backend(",
            "preverify_kagemusha_recursive_compact_payment_token_with_record",
            "kagemusha_recursive_spend_lineage_vk_record_from_box_for_circuit",
            "pub fn kagemusha_recursive_spend_lineage_vk_record_from_box(",
        "pub fn kagemusha_recursive_spend_lineage_append_vk_record_from_box(",
        "does not generate a verifier key at runtime",
        "lineage_vk_record_from_box_canonicalizes_profiles_without_keygen",
    ),
    "crates/iroha_cli/src/zk.rs": (
        "KagemushaCommand::LineageKeyArtifacts",
        "KagemushaCommand::RecursiveCompactKeyArtifacts",
        "KagemushaCommand::LineageRecord",
        "KagemushaRecursiveCompactKeyArtifactsArgs",
        "KagemushaLineageRecordArgs",
        "derive_halo2_ipa_kagemusha_recursive_compact_payment_token_proving_key_bytes",
        "derive_halo2_ipa_kagemusha_recursive_compact_payment_token_append_proving_key_bytes",
        "KagemushaRecursiveCompactKeyArtifactsV1::new",
        'arg(long, value_name = "PATH", required = true)',
        "--key-artifacts-out and --verifier-keys-out must both be provided for ABI-7 recursive compact production key packages",
        "recursive_compact_key_artifacts_rejects_one_sided_package_outputs_before_keygen",
        "recursive_compact_key_artifacts_rejects_missing_package_outputs_before_keygen",
        "kagemusha_recursive_compact_payment_token_vk_record_from_box",
        "record_out: Option<std::path::PathBuf>",
        "record_namespace: String",
        "record_version: u32",
        "kagemusha_recursive_compact_vk_record_from_bytes",
        "kagemusha_lineage_vk_record_from_bytes",
        "std::fs::read(&self.vk)",
        "write_kagemusha_lineage_key_artifact_file",
        "std::fs::OpenOptions::new()",
        ".create_new(true)",
        "file.sync_all()",
        "std::fs::rename(&temp_path, path)",
        "parent_dir.sync_all()?",
        "failed to allocate temporary artifact output path",
        "kagemusha_key_artifact_writer_creates_nested_parent_and_replaces_target",
        "kagemusha_key_artifact_writer_rejects_directory_output_path",
        "kagemusha_recursive_compact_record_from_existing_vk_bytes_rejects_adversarial_inputs",
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
        "detached valid Pallas opening archives before proving",
        "valid multi-hop recursive compact Pallas archives must produce a package-backed token",
        "shape-valid ABI-7 compact tokens with invalid proof bodies must return a soft invalid result",
        "shape-valid envelopes with stale folded-token bindings must hard-fail before soft invalid",
        "preverify_kagemusha_recursive_compact_payment_token",
        "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE",
        "if is_kagemusha_recursive_compact_unavailable_error(&err) {\n                    BridgeError::KagemushaRecursiveCompactUnavailable\n                } else {",
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
        "valid multi-hop recursive compact archive must produce a token",
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
        "valid multi-hop recursive compact archive must produce a token",
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

FORBIDDEN_SNIPPETS = {
    "scripts/kagemusha_production_readiness.py": (
        '            if not artifact_path.is_file():\n                blockers.append(\n                    blocker(\n                        "lineage_proof_evidence_artifact_missing",\n',
    ),
}

WORKFLOW_PATH = ".github/workflows/pr_kagemusha_payload_bench.yml"
WORKFLOW_REQUIREMENTS = (
    '"ci/check_kagemusha_production_readiness.sh"',
    '"scripts/check_android_device_lab_slot.py"',
    '"scripts/sign_android_device_lab_evidence.py"',
    '"scripts/kagemusha_production_readiness.py"',
    '"scripts/kagemusha_lineage_proof_evidence.py"',
    '"scripts/kagemusha_recursive_compact_key_evidence.py"',
    '"scripts/kagemusha_release_bundle.py"',
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
    "ci/check_kagemusha_production_readiness.sh --negative-control-abi7-core-contract-open",
    "ci/check_kagemusha_production_readiness.sh --negative-control-abi7-one-hop-runtime-keygen-fallback",
    "ci/check_kagemusha_production_readiness.sh --negative-control-abi7-append-runtime-keygen-fallback",
    "ci/check_kagemusha_production_readiness.sh --negative-control-abi7-bridge-unavailable-mapping",
    "ci/check_kagemusha_production_readiness.sh --negative-control-abi7-offline-doc-one-hop-boundary",
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
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-parent-is-dir-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-parent-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-parent-create-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-post-create-parent-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-file-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-hardlink-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-write-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-readback-verification",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-readback-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-readback-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-post-write-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-direct-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-scan-slot-expected-dir-is-dir-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-scan-slot-artifact-count-is-file-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-scan-slot-sha-is-file-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-secret-redaction",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-root-direct-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-root-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-main-root-exists-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-rollup-root-exists-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-duplicate-json-keys",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-root-symlink",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-root-ancestor-symlink",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-ancestor-cwd-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-ancestor-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-ancestor-is-symlink-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-ancestor-exists-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-root-discovery-read-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-discover-slots-is-dir-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-discover-slots-entry-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-load-ancestor",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-load-direct-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-load-file-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-load-read-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-load-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-parse-direct-slot-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-verify-direct-slot-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-slot-root-symlink",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-slot-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-slot-ancestor-symlink",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-hardlink",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-file-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-hardlink-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-read-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-artifact-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-relative-ancestor-is-symlink-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-file-shape-terminal",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-direct-helper-slot-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-direct-symlink-artifact-slot-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-direct-hardlink-artifact-slot-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-direct-regular-artifact-slot-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-files-direct-root-shape",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-files-root-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-files-direct-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-files-direct-ancestor-symlink",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-files-direct-symlink-directory",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-files-directory-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-top-level-listing-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-files-artifact-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-slot-listing-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-verify-symlink-directory",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-dir-symlink",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-parent-symlink",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-parent-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-ancestor-symlink",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-directory-traversal-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-symlink-artifacts",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-symlink-artifact-leaf-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-symlink-artifact-directory-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-symlink-artifact-nested-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-regular-file-artifacts",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-regular-file-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-regular-file-exists-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-directory-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-directory-exists-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-artifact-file-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-artifact-symlink-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-hardlink-artifacts",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-hardlink-artifact-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-hardlink-artifact-directory-exists-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-required-artifacts",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-d2d-queue-is-file-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-required-artifact-is-file-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-required-status-is-file-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-required-runtime-log-is-file-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-required-artifact-shape",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-required-artifact-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-required-artifact-content",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-id-safety",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-name-safety",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signed-artifact-schema",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signed-evidence-artifact-digest-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signed-evidence-artifact-is-file-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signed-evidence-artifact-read-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signed-evidence-artifact-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-metadata-artifact-digest-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-metadata-artifact-read-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-metadata-artifact-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-transcript-artifact-digest-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-required-text-artifact-read-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signature-verify",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-public-key-openssl-spawn-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-public-key-openssl-invalid-key",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signature-verify-staging-write-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-staged-bytes-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signature-verify-tempdir-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signature-verify-spawn-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-signature-read-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-signature-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-signature-shape",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-signature-staging-write-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-signature-tempdir-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-signature-spawn-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-signature-invalid-private-key",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-write",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-json-write-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-ancestor",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-parent-is-dir-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-parent-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-parent-create-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-post-create-parent-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-resolve-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-dangling-output-alias",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-hardlink-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-file-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-digest-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-digest-parent-missing",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-digest-leaf-missing",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-digest-hardlink-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-digest-file-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-digest-read-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-digest-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-direct-output-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-manifest-write",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-text-write-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-readback-verification",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-direct-manifest-shape",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-slot-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-slot-parent-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-slot-artifact-digest-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-slot-artifact-hardlink-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-slot-artifact-file-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-slot-artifact-read-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-slot-artifact-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-artifact-digest-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-digest-artifact-file-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-artifact-read-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-manifest-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-metadata-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-artifact-digests-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-direct-slot-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-direct-manifest-slot-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signer-key-files",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signer-key-ancestors",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-private-key-ancestors",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-public-key-hardlink-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-public-key-file-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-public-key-regular-file-before-openssl",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-public-key-missing-before-openssl",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-private-key-hardlink-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-private-key-file-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-private-key-regular-file-before-openssl",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-private-key-missing-before-openssl",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-public-key-path-before-openssl",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-private-key-path-before-openssl",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signature-verify-key-path-before-openssl",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-private-public-pair-preserves-key-path-errors",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signer-key-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-cli-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-cli-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-command-exact",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-signed-evidence-freshness-report",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-signed-evidence-timestamp-raw",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-rollup",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-rollup-path-safety",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-android-report-secret-redaction",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-repo-root-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-repo-root-direct-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-repo-root-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-repo-root-resolve-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-trust-root-section-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-android-root-discovery-read-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-release-json-direct-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-release-json-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-json-read-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-json-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-source-marker-direct-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-source-marker-read-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-source-marker-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-source-marker-non-utf8-read",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-dangling-alias",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-ancestor",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-parent-is-dir-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-parent-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-parent-create-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-post-create-parent-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-regular-file",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-file-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-hardlink-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-direct-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-write-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-readback-verification",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-readback-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-readback-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-post-write-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-release-json-hardlink-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-release-json-file-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-key-release-tooling",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-release-tooling",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-key-release-source-marker-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-key-release-source-marker-non-utf8-read",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-evidence",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-evidence",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-evidence-path-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-evidence-path-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-source-marker-hardlink-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-source-marker-file-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-readiness-direct-hash-shape",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-readiness-direct-hash-read-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-local-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-local-ancestor-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-local-hardlink-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-local-file-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-artifact-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-artifact-is-file-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-file-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-future-skew",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-dangling-alias",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-ancestor",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-parent-is-dir-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-parent-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-parent-create-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-post-create-parent-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-validate-parent-create-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-file-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-hardlink-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-early-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-write-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-readback-verification",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-readback-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-readback-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-post-write-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-output-parent-create-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-output-file-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-output-hardlink-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-output-early-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-output-write-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-output-readback-verification",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-output-readback-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-output-readback-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-output-post-write-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-validation-dir-create-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-validation-temp-write-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-validation-temp-cleanup-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-direct-artifact-dir-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-direct-artifact-dir-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-direct-hash-shape",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-direct-hash-read-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-generator-log-strict-read",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-direct-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-direct-hash-shape",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-direct-hash-read-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-direct-artifact-dir-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-direct-artifact-dir-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-direct-proof-log-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-direct-output-preflight-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-validation-dir-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-validation-dir-create-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-validation-temp-write-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-validation-temp-cleanup-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-input-corridor",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-input-corridor-resolve-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-corridor-resolve-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-command-canonical",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-command-canonical",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-scalar-types",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-scalar-types",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-artifact-size-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-readiness-artifact-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-artifact-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-artifact-prefix-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-artifact-size-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-readiness-artifact-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-artifact-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-artifact-prefix-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-placeholder-artifacts",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-generator-log-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-generator-log-digest-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-generator-log-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-timestamp-raw",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-timestamp-raw",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-summary-drift",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-summary-section-schema",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-android-signed-evidence-summary-schema",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-artifact-inventory",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-android-slot-artifact-inventory",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-compact-placeholder-inventory",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-compact-generator-log-inventory",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-evidence-entry-nonempty",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-evidence-entry-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-json-input-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-digest-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-atomic-output",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-output-readback-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-output-readback-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-output-post-write-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-input-path-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-scan-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-output-overwrite",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-verify-existing",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-verify-existing-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-verify-existing-evidence-path-shape",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-timestamp-raw",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-log-exact",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-log-metadata-read-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-log-is-file-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-log-text-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-log-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-evidence-filename",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-evidence-filename",
    "ci/check_kagemusha_production_readiness.sh --negative-control-json-duplicate-keys",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-closed-schema",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-closed-schema",
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


def override_text_all(relative: str, old: str, new: str) -> None:
    text = read_text(relative)
    if old not in text:
        raise SystemExit(f"negative control setup failed: `{old}` not found in {relative}")
    text_overrides[relative] = text.replace(old, new)


def require_contains(relative: str, snippets: tuple[str, ...], errors: list[str]) -> None:
    text = read_text(relative)
    for snippet in snippets:
        if snippet not in text:
            errors.append(f"{relative}: missing `{snippet}`")


def require_absent(relative: str, snippets: tuple[str, ...], errors: list[str]) -> None:
    text = read_text(relative)
    for snippet in snippets:
        if snippet in text:
            errors.append(f"{relative}: forbidden `{snippet}`")


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
    for relative, snippets in FORBIDDEN_SNIPPETS.items():
        require_absent(relative, snippets, errors)
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

if mode == "--negative-control-kagemusha-readiness-release-json-hardlink-metadata-failure":
    run_negative_control(
        "Kagemusha readiness release JSON hardlink metadata failure gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        return [f"{label} hardlink metadata could not be read"]\n    if link_count > 1:\n        return [f"{label} must not be hardlinked"]\n',
            '    link_count = path.stat().st_nlink\n    if link_count > 1:\n        return [f"{label} must not be hardlinked"]\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-release-json-file-metadata-failure":
    run_negative_control(
        "Kagemusha readiness release JSON file metadata failure gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    release_json_ancestor_errors = device_lab.validate_no_symlink_ancestors(\n        path,\n        f"{label} ancestor directory",\n    )\n    if release_json_ancestor_errors:\n        return release_json_ancestor_errors\n    try:\n        mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return [f"{label} is missing"]\n    except OSError:\n        return [f"{label} file metadata could not be read"]\n',
            '    release_json_ancestor_errors = device_lab.validate_no_symlink_ancestors(\n        path,\n        f"{label} ancestor directory",\n    )\n    if release_json_ancestor_errors:\n        return release_json_ancestor_errors\n    try:\n        mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return [f"{label} is missing"]\n    except OSError:\n        return [f"{label} is missing"]\n',
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
        "ABI-7 compact multi-hop fail-closed gate",
        lambda: override_text(
            "crates/iroha_core/src/zk.rs",
            "multi-hop proving requires the append verifier batch to be composed into the compact proof",
            "multi-hop proving is enabled without the append verifier batch",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-abi7-core-contract-open":
    run_negative_control(
        "ABI-7 one-hop compact core function contract",
        lambda: override_text(
            "crates/iroha_core/src/zk.rs",
            "public ABI-7 compact token one-hop shape preverification",
            "public ABI-7 compact token disabled shape preverification",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-abi7-one-hop-runtime-keygen-fallback":
    run_negative_control(
        "ABI-7 one-hop compact runtime keygen fallback",
        lambda: override_text_all(
            "crates/iroha_core/src/zk.rs",
            "missing compact one-hop proving key archive",
            "runtime-generated compact one-hop proving key archive accepted",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-abi7-append-runtime-keygen-fallback":
    run_negative_control(
        "ABI-7 append compact runtime keygen fallback",
        lambda: override_text_all(
            "crates/iroha_core/src/zk.rs",
            "missing compact append proving key archive",
            "runtime-generated compact append proving key archive accepted",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-abi7-bridge-unavailable-mapping":
    run_negative_control(
        "ABI-7 bridge unavailable mapping",
        lambda: override_text(
            "crates/connect_norito_bridge/src/lib.rs",
            "if is_kagemusha_recursive_compact_unavailable_error(&err) {\n                    BridgeError::KagemushaRecursiveCompactUnavailable\n                } else {",
            "if is_kagemusha_recursive_compact_unavailable_error(&err) {\n                    BridgeError::KagemushaProve\n                } else {",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-abi7-offline-doc-one-hop-boundary":
    run_negative_control(
        "ABI-7 offline doc one-hop compact boundary",
        lambda: override_text(
            "docs/source/offline_kagemusha.md",
            "ABI-7 recursive compact-token symbols now route one-hop",
            "ABI-7 recursive compact-token symbols are globally disabled",
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

if mode == "--negative-control-kagemusha-readiness-json-read-failure":
    def mutate_rollup_json_read_failure() -> None:
        override_text(
            "scripts/kagemusha_production_readiness.py",
            '    except OSError:\n        return None, [blocker(unreadable_code, f"{label} could not be read")]\n',
            "",
        )
        override_text(
            "scripts/kagemusha_production_readiness.py",
            '            elif error == unreadable_error:\n                blockers.append(blocker(unreadable_code, error))\n',
            "",
        )

    run_negative_control(
        "Kagemusha readiness JSON read/decode failure gate",
        mutate_rollup_json_read_failure,
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-release-json-open-path-binding":
    run_negative_control(
        "Kagemusha readiness release JSON open-path binding",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "            release_json_path_stat = path.lstat()",
            "            release_json_path_stat = open_stat",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-json-open-path-binding":
    run_negative_control(
        "Kagemusha readiness JSON open-path binding",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            'digest, text, read_errors = _sha256_text_file(\n        path,\n        label,\n        f"{label} could not be read",\n    )',
            'digest, text, read_errors = _sha256_text_file_unbound(\n        path,\n        label,\n        f"{label} could not be read",\n    )',
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

if mode == "--negative-control-kagemusha-readiness-source-marker-hardlink-metadata-failure":
    run_negative_control(
        "Kagemusha readiness source marker hardlink metadata failure gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        errors.append(f"{label} hardlink metadata could not be read")\n        return errors\n    if link_count > 1:\n        errors.append(f"{label} must not be hardlinked")\n',
            '    link_count = path.stat().st_nlink\n    if link_count > 1:\n        errors.append(f"{label} must not be hardlinked")\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-source-marker-file-metadata-failure":
    run_negative_control(
        "Kagemusha readiness source marker file metadata failure gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    except OSError:\n        errors.append(f"{label} file metadata could not be read")\n        return errors\n',
            '    except OSError:\n        errors.append(f"{label} is missing")\n        return errors\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-source-marker-read-preflight":
    run_negative_control(
        "Kagemusha readiness source marker read preflight gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            'unreadable_error = "ABI-7 source marker file could not be read"\n        text, file_errors = _repo_source_marker_text(\n            path,\n            label,\n            unreadable_error,\n        )',
            'unreadable_error = "ABI-7 source marker file could not be read"\n        try:\n            text = path.read_text(encoding="utf-8")\n            file_errors = []\n        except OSError:\n            text = None\n            file_errors = [unreadable_error]',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-source-marker-open-path-binding":
    run_negative_control(
        "Kagemusha readiness source marker open-path binding",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "expected_marker_identity = (expected_stat.st_dev, expected_stat.st_ino)",
            "expected_marker_identity = (open_stat.st_dev, open_stat.st_ino)",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-source-marker-non-utf8-read":
    run_negative_control(
        "Kagemusha readiness source marker non-UTF-8 read gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    except UnicodeDecodeError:\n        return None, [unreadable_error]\n',
            '    except UnicodeDecodeError:\n        return "", []\n',
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
        "Android device-matrix compact one-hop/multi-hop boundary",
        lambda: override_text(
            "docs/source/sdk/android/readiness/android_strongbox_device_matrix.md",
            "ABI 7 recursive compact prover calls that require multi-hop append-batch",
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

if mode == "--negative-control-android-device-lab-json-output-parent-is-dir-preflight":
    run_negative_control(
        "Android device-lab JSON summary output parent is_dir preflight gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "    if not stat.S_ISDIR(parent_mode):\n",
            "    if not parent.is_dir():\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-json-output-parent-metadata-failure":
    run_negative_control(
        "Android device-lab JSON summary output parent metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    except OSError:\n        return False, [f"{label} parent directory metadata could not be read"]\n',
            "    except OSError:\n        return False, []\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-json-output-parent-create-failure":
    run_negative_control(
        "Android device-lab JSON summary output parent-create failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '        try:\n            parent.mkdir(parents=True, exist_ok=True)\n        except OSError:\n            return [f"{label} parent directory could not be created"]\n',
            "        parent.mkdir(parents=True, exist_ok=True)\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-json-output-post-create-parent-preflight":
    run_negative_control(
        "Android device-lab JSON summary output post-create parent preflight gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    parent_exists, parent_errors = _validate_summary_output_parent(\n        path,\n        label,\n        missing_error=f"{label} parent must be a directory",\n    )\n    if parent_errors:\n        return parent_errors\n    if not parent_exists:\n        return [f"{label} parent must be a directory"]\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-json-output-file-metadata-failure":
    run_negative_control(
        "Android device-lab JSON summary output file metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    try:\n        output_mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return []\n    except OSError:\n        return [f"{label} file metadata could not be read"]\n',
            '    try:\n        output_mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return []\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-json-output-hardlink-metadata-failure":
    run_negative_control(
        "Android device-lab JSON summary output hardlink metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        return [f"{label} hardlink metadata could not be read"]\n',
            "    link_count = path.stat().st_nlink\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-json-output-write-failure":
    run_negative_control(
        "Android device-lab JSON summary output write-failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "        os.replace(tmp_path, path)\n",
            '        path.write_text(summary_text, encoding="utf-8")\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-json-output-readback-verification":
    run_negative_control(
        "Android device-lab JSON summary output readback gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "readback_text != summary_text",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-json-output-readback-failure":
    run_negative_control(
        "Android device-lab JSON summary output readback failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    except OSError:\n        return None, ["--json-out write verification failed"]\n',
            "    except OSError:\n        return None, []\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-json-output-readback-open-path-binding":
    run_negative_control(
        "Android device-lab JSON summary output readback open-path binding gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "summary_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
            "summary_expected_identity = (open_stat.st_dev, open_stat.st_ino)",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-json-output-post-write-preflight":
    run_negative_control(
        "Android device-lab JSON summary output post-write preflight gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    errors = validate_summary_output_path(path, "--json-out")\n    if errors:\n        return errors\n    try:\n        expected_stat = path.lstat()\n',
            '    try:\n        expected_stat = path.lstat()\n',
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

if mode == "--negative-control-android-device-lab-scan-slot-expected-dir-is-dir-preflight":
    run_negative_control(
        "Android device-lab scan_slot expected directory is_dir preflight gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    if stat.S_ISLNK(dir_mode) or not stat.S_ISDIR(dir_mode):\n        return False, False\n',
            '    if stat.S_ISLNK(dir_mode) or not dir_path.is_dir():\n        return False, False\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-scan-slot-artifact-count-is-file-preflight":
    run_negative_control(
        "Android device-lab scan_slot artifact count is_file preflight gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '        if stat.S_ISREG(entry_mode):\n            count += 1\n',
            '        if entry.is_file():\n            count += 1\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-scan-slot-sha-is-file-preflight":
    run_negative_control(
        "Android device-lab scan_slot sha256sum is_file preflight gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "    return stat.S_ISREG(mode)\n",
            "    return path.is_file()\n",
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
            '    if SECRET_RE.search(str(root)):\n        return False, ["device-lab root path must not contain secret-looking material"]\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-root-metadata-failure":
    run_negative_control(
        "Android device-lab direct root metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    try:\n        root_mode = root.lstat().st_mode\n    except FileNotFoundError:\n        root_mode = None\n    except OSError:\n        return False, ["device-lab root metadata could not be read"]\n',
            '    try:\n        root_mode = root.lstat().st_mode\n    except FileNotFoundError:\n        root_mode = None\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-main-root-exists-preflight":
    run_negative_control(
        "Android device-lab scanner root exists preflight gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "    if not root_exists:\n        if args.allow_missing_root:\n",
            "    if not root.exists():\n        if args.allow_missing_root:\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-rollup-root-exists-preflight":
    run_negative_control(
        "Android device-lab rollup root exists preflight gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    if not root_exists:\n        return {\n            "ok": False,\n',
            '    if not root.exists():\n        return {\n            "ok": False,\n',
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

if mode == "--negative-control-android-device-lab-ancestor-cwd-failure":
    run_negative_control(
        "Android device-lab ancestor cwd failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    if path.is_absolute():\n        candidate = path\n    else:\n        try:\n            candidate = Path.cwd() / path\n        except OSError:\n            return [f"{label} metadata could not be read"]\n',
            "    candidate = path if path.is_absolute() else Path.cwd() / path\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-ancestor-metadata-failure":
    run_negative_control(
        "Android device-lab ancestor metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '        except OSError:\n            errors.append(f"{label} metadata could not be read")\n            break\n',
            "        except OSError:\n            continue\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-ancestor-is-symlink-preflight":
    run_negative_control(
        "Android device-lab ancestor is_symlink preflight gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "        if stat.S_ISLNK(ancestor_mode):\n            errors.append(f\"{label} must not be a symlink\")\n            break\n",
            "        if ancestor.is_symlink():\n            errors.append(f\"{label} must not be a symlink\")\n            break\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-ancestor-exists-preflight":
    run_negative_control(
        "Android device-lab ancestor exists preflight gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "        try:\n            ancestor_mode = ancestor.lstat().st_mode\n",
            "        if not ancestor.exists():\n            continue\n        try:\n            ancestor_mode = ancestor.lstat().st_mode\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-root-discovery-read-failure":
    run_negative_control(
        "Android device-lab root discovery read-failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "device-lab root could not be listed",
            "device-lab root listing failures ignored",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-discover-slots-is-dir-preflight":
    run_negative_control(
        "Android device-lab discover_slots is_dir preflight gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "        if stat.S_ISDIR(entry_mode) or stat.S_ISLNK(entry_mode):\n            slots.append(entry)\n",
            "        if entry.is_dir():\n            slots.append(entry)\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-discover-slots-entry-metadata-failure":
    run_negative_control(
        "Android device-lab discover_slots entry metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '        except OSError:\n            _append_error_once(\n                errors,\n                "device-lab slot directory metadata could not be read",\n            )\n            continue\n',
            "        except OSError:\n            continue\n",
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

if mode == "--negative-control-android-device-lab-json-load-file-metadata-failure":
    run_negative_control(
        "Android device-lab JSON loader file metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    try:\n        expected_stat = path.lstat()\n    except FileNotFoundError:\n        errors.append(f"missing {label}")\n        return None\n    except OSError:\n        errors.append(f"{label} file metadata could not be read")\n        return None\n',
            '    expected_stat = path.lstat()\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-json-load-read-failure":
    run_negative_control(
        "Android device-lab JSON loader read/decode failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    except (OSError, UnicodeDecodeError):\n        errors.append(f"{label} could not be read")\n        return None\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-json-load-open-path-binding":
    run_negative_control(
        "Android device-lab JSON loader open-path binding gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "json_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
            "json_expected_identity = (open_stat.st_dev, open_stat.st_ino)",
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
            '    if slot_mode is not None and stat.S_ISLNK(slot_mode):\n        return ["slot directory must not be a symlink"]\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-manifest-slot-metadata-failure":
    run_negative_control(
        "Android device-lab manifest slot metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    except OSError:\n        return ["slot directory metadata could not be read"]\n',
            '    except OSError:\n        slot_mode = None\n',
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

if mode == "--negative-control-android-device-lab-manifest-hardlink":
    run_negative_control(
        "Android device-lab manifest hardlink gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    try:\n        if manifest_path.stat().st_nlink > 1:\n            return entries, ["sha256sum.txt must not be hardlinked"]\n    except OSError:\n        return entries, ["sha256sum.txt hardlink metadata could not be read"]\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-manifest-file-metadata-failure":
    run_negative_control(
        "Android device-lab manifest file metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    try:\n        manifest_mode = manifest_path.lstat().st_mode\n    except FileNotFoundError:\n        return entries, ["missing sha256sum.txt"]\n    except OSError:\n        return entries, ["sha256sum.txt file metadata could not be read"]\n',
            '    try:\n        manifest_mode = manifest_path.lstat().st_mode\n    except FileNotFoundError:\n        return entries, ["missing sha256sum.txt"]\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-manifest-hardlink-metadata-failure":
    run_negative_control(
        "Android device-lab manifest hardlink metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    try:\n        if manifest_path.stat().st_nlink > 1:\n            return entries, ["sha256sum.txt must not be hardlinked"]\n    except OSError:\n        return entries, ["sha256sum.txt hardlink metadata could not be read"]\n',
            '    if manifest_path.stat().st_nlink > 1:\n        return entries, ["sha256sum.txt must not be hardlinked"]\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-manifest-read-failure":
    run_negative_control(
        "Android device-lab manifest read/decode failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    except (OSError, UnicodeDecodeError):\n        return entries, ["sha256sum.txt could not be read"]\n',
            '    except UnicodeDecodeError:\n        return entries, ["sha256sum.txt could not be read"]\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-manifest-open-path-binding":
    run_negative_control(
        "Android device-lab manifest open-path binding gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "expected_identity = (manifest_stat.st_dev, manifest_stat.st_ino)",
            "expected_identity = (open_stat.st_dev, open_stat.st_ino)",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-manifest-artifact-open-path-binding":
    run_negative_control(
        "Android device-lab manifest artifact open-path binding gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "manifest_expected_identity = (\n                expected_stat.st_dev,\n                expected_stat.st_ino,\n            )",
            "manifest_expected_identity = (\n                open_stat.st_dev,\n                open_stat.st_ino,\n            )",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-manifest-file-shape-terminal":
    run_negative_control(
        "Android device-lab manifest file-shape terminal gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    if _has_manifest_file_shape_error(errors):\n        return errors\n',
            "",
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
            'def _slot_files(slot_path: Path, errors: list[str] | None = None) -> set[str]:\n    slot_errors = errors if errors is not None else []\n    try:\n        slot_mode = slot_path.lstat().st_mode\n    except FileNotFoundError:\n        return set()\n    except OSError:\n        _append_error_once(slot_errors, "slot directory metadata could not be read")\n        return set()\n    if stat.S_ISLNK(slot_mode) or not stat.S_ISDIR(slot_mode):\n        return set()\n',
            'def _slot_files(slot_path: Path, errors: list[str] | None = None) -> set[str]:\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-files-root-metadata-failure":
    run_negative_control(
        "Android device-lab slot file discovery root metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    except OSError:\n        _append_error_once(slot_errors, "slot directory metadata could not be read")\n        return set()\n',
            '    except OSError:\n        return set()\n',
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
            '        try:\n            dir_mode = dir_path.lstat().st_mode\n        except FileNotFoundError:\n            continue\n        except OSError:\n            _append_error_once(slot_errors, f"{dirname}/ metadata could not be read")\n            continue\n        if stat.S_ISLNK(dir_mode) or not stat.S_ISDIR(dir_mode):\n            continue\n',
            '        if not dir_path.is_dir():\n            continue\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-files-directory-metadata-failure":
    run_negative_control(
        "Android device-lab slot file discovery directory metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '        except OSError:\n            _append_error_once(slot_errors, f"{dirname}/ metadata could not be read")\n            continue\n',
            '        except OSError:\n            continue\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-top-level-listing-failure":
    run_negative_control(
        "Android device-lab slot top-level listing failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            'def _slot_root_entries(slot_path: Path, errors: list[str]) -> list[Path] | None:\n    try:\n        return list(slot_path.iterdir())\n    except OSError:\n        _append_error_once(errors, "slot directory could not be listed")\n        return None\n',
            'def _slot_root_entries(slot_path: Path, errors: list[str]) -> list[Path] | None:\n    return list(slot_path.iterdir())\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-files-artifact-metadata-failure":
    run_negative_control(
        "Android device-lab slot file discovery artifact metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    try:\n        mode = entry.lstat().st_mode\n    except OSError:\n        _append_error_once(\n            errors,\n            f"slot artifact {_display_path(relative)} file metadata could not be read",\n        )\n        return\n    if stat.S_ISREG(mode) or stat.S_ISLNK(mode):\n        files.add(relative)\n',
            '    if entry.is_file() or entry.is_symlink():\n        files.add(relative)\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-slot-listing-failure":
    run_negative_control(
        "Android device-lab signing helper slot listing failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "slot_files = device_lab._slot_files(slot_path, errors)",
            "slot_files = device_lab._slot_files(slot_path)",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-manifest-verify-symlink-directory":
    run_negative_control(
        "Android device-lab manifest verifier symlink directory gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    display = _display_path(safe_relative)\n    artifact_path = slot_path / safe_relative\n    if _slot_relative_symlink_ancestor(slot_path, safe_relative) is not None:\n        return None, [\n            "sha256sum.txt references artifact under symlink directory "\n            f"{display}"\n        ]\n',
            '    display = _display_path(safe_relative)\n    artifact_path = slot_path / safe_relative\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-relative-ancestor-is-symlink-preflight":
    run_negative_control(
        "Android device-lab relative ancestor is_symlink preflight gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "        if stat.S_ISLNK(current_mode):\n            return current.relative_to(slot_path).as_posix()\n",
            "        if current.is_symlink():\n            return current.relative_to(slot_path).as_posix()\n",
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

if mode == "--negative-control-android-device-lab-slot-metadata-failure":
    run_negative_control(
        "Android device-lab slot metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    except OSError:\n        return {\n            "slot": slot_label,\n            "status": "error",\n            "errors": ["slot directory metadata could not be read"],\n            "present": present,\n            "file_counts": file_counts,\n            "kagemusha": {"required": require_kagemusha_production_evidence},\n        }\n\n',
            '    except OSError:\n        slot_mode = None\n\n',
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

if mode == "--negative-control-android-device-lab-slot-parent-metadata-failure":
    run_negative_control(
        "Android device-lab slot parent metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    except OSError:\n        return {\n            "slot": slot_label,\n            "status": "error",\n            "errors": ["slot parent directory metadata could not be read"],\n            "present": present,\n            "file_counts": file_counts,\n            "kagemusha": {"required": require_kagemusha_production_evidence},\n        }\n\n',
            '    except OSError:\n        parent_mode = None\n\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-ancestor-symlink":
    run_negative_control(
        "Android device-lab slot ancestor symlink gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    ancestor_errors = device_lab.validate_no_symlink_ancestors(\n        slot_path,\n        "slot ancestor directory",\n    )\n    if ancestor_errors:\n        return ancestor_errors\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-directory-traversal-failure":
    run_negative_control(
        "Android device-lab slot directory traversal-failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            'f"{label} could not be listed"',
            'f"{label} listing failures ignored"',
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

if mode == "--negative-control-android-device-lab-symlink-artifact-leaf-metadata-failure":
    run_negative_control(
        "Android device-lab symlink artifact leaf metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '        except OSError:\n            _append_error_once(errors, f"{relative} file metadata could not be read")\n            continue\n',
            '        except OSError:\n            continue\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-symlink-artifact-directory-metadata-failure":
    run_negative_control(
        "Android device-lab symlink artifact directory metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '        except OSError:\n            _append_error_once(errors, f"{dirname}/ metadata could not be read")\n            continue\n        if stat.S_ISLNK(dir_mode):\n            errors.append(f"{dirname}/ must not be a symlink")\n',
            '        except OSError:\n            continue\n        if stat.S_ISLNK(dir_mode):\n            errors.append(f"{dirname}/ must not be a symlink")\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-symlink-artifact-nested-metadata-failure":
    run_negative_control(
        "Android device-lab symlink artifact nested metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '            except OSError:\n                _append_error_once(\n                    errors,\n                    f"slot artifact {_display_path(relative)} file metadata could not be read",\n                )\n                continue\n            if stat.S_ISLNK(entry_mode):\n',
            '            except OSError:\n                continue\n            if stat.S_ISLNK(entry_mode):\n',
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

if mode == "--negative-control-android-device-lab-slot-regular-file-metadata-failure":
    run_negative_control(
        "Android device-lab slot regular-file metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    try:\n        mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return\n    except OSError:\n        errors.append(f"{label} file metadata could not be read")\n        return\n',
            "    mode = path.lstat().st_mode\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-regular-file-exists-preflight":
    run_negative_control(
        "Android device-lab slot regular-file exists preflight gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            'def _reject_non_regular_file(path: Path, label: str, errors: list[str]) -> None:\n    try:\n',
            'def _reject_non_regular_file(path: Path, label: str, errors: list[str]) -> None:\n    if path.is_symlink() or not path.exists():\n        return\n    try:\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-directory-metadata-failure":
    run_negative_control(
        "Android device-lab slot directory metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '        try:\n            mode = dir_path.lstat().st_mode\n        except FileNotFoundError:\n            continue\n        except OSError:\n            errors.append(f"{dirname}/ metadata could not be read")\n            continue\n',
            "        mode = dir_path.lstat().st_mode\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-directory-exists-preflight":
    run_negative_control(
        "Android device-lab slot directory exists preflight gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '        try:\n            mode = dir_path.lstat().st_mode\n        except FileNotFoundError:\n            continue\n        except OSError:\n            errors.append(f"{dirname}/ metadata could not be read")\n            continue\n        if stat.S_ISLNK(mode):\n            continue\n',
            '        if dir_path.is_symlink() or not dir_path.exists():\n            continue\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-artifact-file-metadata-failure":
    run_negative_control(
        "Android device-lab slot artifact file metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '            try:\n                entry_mode = entry.lstat().st_mode\n            except OSError:\n                errors.append(\n                    f"slot artifact {_display_path(relative)} file metadata could not be read"\n                )\n                continue\n',
            "            entry_mode = entry.lstat().st_mode\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-artifact-symlink-preflight":
    run_negative_control(
        "Android device-lab slot artifact symlink preflight gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '        for entry in entries:\n            relative = entry.relative_to(slot_path).as_posix()\n            try:\n                entry_mode = entry.lstat().st_mode\n            except OSError:\n                errors.append(\n                    f"slot artifact {_display_path(relative)} file metadata could not be read"\n                )\n                continue\n            if stat.S_ISLNK(entry_mode):\n                continue\n            if stat.S_ISDIR(entry_mode):\n',
            '        for entry in entries:\n            if entry.is_symlink():\n                continue\n            relative = entry.relative_to(slot_path).as_posix()\n            try:\n                entry_mode = entry.lstat().st_mode\n            except OSError:\n                errors.append(\n                    f"slot artifact {_display_path(relative)} file metadata could not be read"\n                )\n                continue\n            if stat.S_ISLNK(entry_mode):\n                continue\n            if stat.S_ISDIR(entry_mode):\n',
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

if mode == "--negative-control-android-device-lab-hardlink-artifact-metadata-failure":
    run_negative_control(
        "Android device-lab hardlink artifact metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            'def _reject_hardlinked_file(path: Path, label: str, errors: list[str]) -> None:\n    try:\n        mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return\n    except OSError:\n        errors.append(f"{label} file metadata could not be read")\n        return\n    if stat.S_ISLNK(mode) or not stat.S_ISREG(mode):\n        return\n',
            'def _reject_hardlinked_file(path: Path, label: str, errors: list[str]) -> None:\n    if path.is_symlink() or not path.is_file():\n        return\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-hardlink-artifact-directory-exists-preflight":
    run_negative_control(
        "Android device-lab hardlink artifact directory exists preflight gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '        try:\n            dir_mode = dir_path.lstat().st_mode\n        except FileNotFoundError:\n            continue\n        except OSError:\n            _append_error_once(errors, f"{dirname}/ metadata could not be read")\n            continue\n        if stat.S_ISLNK(dir_mode) or not stat.S_ISDIR(dir_mode):\n            continue\n',
            '        if dir_path.is_symlink() or not dir_path.exists():\n            continue\n',
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

if mode == "--negative-control-android-device-lab-d2d-queue-is-file-preflight":
    run_negative_control(
        "Android device-lab D2D queue is_file preflight gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    _, actual_queue_digest, digest_errors = _metadata_artifact_bytes_and_sha256(\n        slot_path,\n        "queue/pending_queue.json",\n        "d2d payment transcript queue_after_sha256",\n        "d2d payment transcript queue_after_sha256 requires queue/pending_queue.json",\n    )\n    if digest_errors:\n        errors.extend(digest_errors)\n    elif (\n        actual_queue_digest is not None\n        and queue_after_sha256 is not None\n',
            '    queue_path = slot_path / "queue" / "pending_queue.json"\n    if not queue_path.is_file():\n        errors.append("d2d payment transcript queue_after_sha256 requires queue/pending_queue.json")\n    elif queue_after_sha256 is not None:\n        _, actual_queue_digest, digest_errors = _metadata_artifact_bytes_and_sha256(\n            slot_path,\n            "queue/pending_queue.json",\n            "d2d payment transcript queue_after_sha256",\n            "d2d payment transcript queue_after_sha256 requires queue/pending_queue.json",\n        )\n        if digest_errors:\n            errors.extend(digest_errors)\n        elif (\n            actual_queue_digest is not None\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-required-artifact-is-file-preflight":
    run_negative_control(
        "Android device-lab required artifact is_file preflight gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '        mode, mode_errors = _slot_artifact_lstat_mode(\n            artifact_path,\n            f"required slot artifact metadata could not be read {relative}",\n        )\n        if mode_errors:\n            errors.extend(mode_errors)\n            continue\n        if mode is None or stat.S_ISLNK(mode) or not stat.S_ISREG(mode):\n            continue\n',
            '        if not artifact_path.is_file():\n            continue\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-required-status-is-file-preflight":
    run_negative_control(
        "Android device-lab required status is_file preflight gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    if not _should_read_optional_text_artifact(\n        slot_path,\n        "telemetry/status.ndjson",\n        "telemetry/status.ndjson",\n        errors,\n    ):\n        return\n',
            '    if not (slot_path / "telemetry" / "status.ndjson").is_file():\n        return\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-required-runtime-log-is-file-preflight":
    run_negative_control(
        "Android device-lab required runtime log is_file preflight gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    if not _should_read_optional_text_artifact(\n        slot_path,\n        "logs/runtime.log",\n        "logs/runtime.log",\n        errors,\n    ):\n        return\n',
            '    if not (slot_path / "logs" / "runtime.log").is_file():\n        return\n',
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

if mode == "--negative-control-android-device-lab-required-artifact-metadata-failure":
    run_negative_control(
        "Android device-lab required artifact metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '        try:\n            artifact_size = artifact_path.stat().st_size\n        except OSError:\n            errors.append(f"required slot artifact metadata could not be read {relative}")\n            continue\n',
            "        artifact_size = artifact_path.stat().st_size\n",
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

if mode == "--negative-control-android-device-lab-signed-evidence-artifact-digest-preflight":
    run_negative_control(
        "Android device-lab signed evidence artifact digest preflight gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    artifact_path, artifact_stat, errors = _validate_signed_evidence_artifact_for_digest(\n        slot_path,\n        relative,\n    )\n    if errors:\n        return None, errors\n    assert artifact_path is not None and artifact_stat is not None\n',
            "    artifact_path = slot_path / relative\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signed-evidence-artifact-is-file-preflight":
    run_negative_control(
        "Android device-lab signed evidence artifact is_file preflight gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '        _, actual_digest, digest_errors = _metadata_artifact_bytes_and_sha256(\n            slot_path,\n            artifact_relative,\n            "slot.json signed_evidence_artifact_path",\n            "slot.json signed_evidence_artifact_path must point to an existing file",\n        )\n        if digest_errors:\n            errors.extend(digest_errors)\n        elif (\n            actual_digest is not None\n            and digest is not None\n',
            '        if not artifact_path.is_file():\n            errors.append(\n                "slot.json signed_evidence_artifact_path must point to an existing file"\n            )\n        elif digest is not None and SHA256_HEX_RE.fullmatch(digest):\n            _, actual_digest, digest_errors = _metadata_artifact_bytes_and_sha256(\n                slot_path,\n                artifact_relative,\n                "slot.json signed_evidence_artifact_path",\n                "slot.json signed_evidence_artifact_path must point to an existing file",\n            )\n            if digest_errors:\n                errors.extend(digest_errors)\n            elif actual_digest is not None\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signed-evidence-artifact-read-failure":
    run_negative_control(
        "Android device-lab signed evidence artifact digest read-failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    except OSError:\n        return None, [\n            "signed evidence artifact digest references artifact that could not be read "\n            f"{display}"\n        ]\n',
            "    except OSError:\n        return None, []\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signed-evidence-artifact-open-path-binding":
    run_negative_control(
        "Android device-lab signed evidence artifact open-path binding gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "signed_evidence_expected_identity = (\n                expected_stat.st_dev,\n                expected_stat.st_ino,\n            )",
            "signed_evidence_expected_identity = (\n                open_stat.st_dev,\n                open_stat.st_ino,\n            )",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-metadata-artifact-digest-preflight":
    run_negative_control(
        "Android device-lab metadata artifact digest preflight gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    if stat.S_ISLNK(artifact_stat.st_mode):\n        return None, None, [f"{label} references symlink artifact {display}"]\n    if not stat.S_ISREG(artifact_stat.st_mode):\n        return None, None, [f"{label} references non-regular artifact {display}"]\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-metadata-artifact-read-failure":
    run_negative_control(
        "Android device-lab metadata artifact digest read-failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "    except OSError:\n        return None, [unreadable_error]\n",
            "    except OSError:\n        return None, []\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-metadata-artifact-open-path-binding":
    run_negative_control(
        "Android device-lab metadata artifact open-path binding gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
            "expected_identity = (open_stat.st_dev, open_stat.st_ino)",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-transcript-artifact-digest-preflight":
    run_negative_control(
        "Android device-lab transcript artifact digest preflight gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '        "slot.json d2d_payment_transcript_path",\n        "slot.json d2d_payment_transcript_path must point to an existing file",\n',
            '        "slot.json unchecked_d2d_payment_transcript_path",\n        "slot.json unchecked_d2d_payment_transcript_path must point to an existing file",\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-required-text-artifact-read-preflight":
    run_negative_control(
        "Android device-lab required text artifact read preflight gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            'text, read_errors = _metadata_artifact_text(\n        slot_path,\n        "telemetry/status.ndjson",\n        "telemetry/status.ndjson",\n        "telemetry/status.ndjson required artifact is missing",\n        "telemetry/status.ndjson could not be read",\n    )',
            'text = (slot_path / "telemetry" / "status.ndjson").read_text(encoding="utf-8")\n    read_errors = []',
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

if mode == "--negative-control-android-device-lab-public-key-openssl-spawn-failure":
    run_negative_control(
        "Android device-lab public key OpenSSL spawn-failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "OpenSSL public key command could not be run",
            "OpenSSL public key command spawn failures ignored",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-public-key-openssl-invalid-key":
    run_negative_control(
        "Android device-lab public key OpenSSL invalid-key gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    except subprocess.CalledProcessError:\n        errors.append(f"{label} must be a valid OpenSSL public key")\n        return None\n',
            '    except subprocess.CalledProcessError:\n        errors.append(f"{label} OpenSSL public key command could not be run")\n        return None\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signature-verify-staging-write-failure":
    run_negative_control(
        "Android device-lab signature verification staging write-failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '        with path.open("xb") as handle:\n            handle.write(payload)\n            handle.flush()\n            os.fsync(handle.fileno())\n',
            '        with path.open("xb") as handle:\n            handle.write(payload)\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-staged-bytes-open-path-binding":
    run_negative_control(
        "Android device-lab staged bytes open-path binding gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "staged_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
            "staged_expected_identity = (open_stat.st_dev, open_stat.st_ino)",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signature-verify-tempdir-failure":
    run_negative_control(
        "Android device-lab signature verification tempdir failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "signature verification temporary directory could not be created",
            "signature verification temporary directory failures ignored",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signature-verify-spawn-failure":
    run_negative_control(
        "Android device-lab signature verification spawn-failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "signature verification command could not be run",
            "signature verification command spawn failures ignored",
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

if mode == "--negative-control-android-device-lab-signing-helper-signature-read-failure":
    run_negative_control(
        "Android device-lab signed evidence helper signature read-failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    except OSError:\n        errors.append("signature output could not be read")\n        return None\n    return b"".join(chunks)\n',
            '    except OSError:\n        return None\n    return b"".join(chunks)\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-signature-open-path-binding":
    run_negative_control(
        "Android device-lab signed evidence helper signature open-path binding gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "signature_output_expected_identity = (\n        expected_stat.st_dev,\n        expected_stat.st_ino,\n    )",
            "signature_output_expected_identity = (\n        open_stat.st_dev,\n        open_stat.st_ino,\n    )",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-signature-shape":
    run_negative_control(
        "Android device-lab signed evidence helper signature shape gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "len(signature) != device_lab.ED25519_SIGNATURE_BYTES",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-signature-staging-write-failure":
    run_negative_control(
        "Android device-lab signed evidence helper signature staging write-failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '            stage_errors = device_lab._write_staged_bytes(\n                payload_path,\n                payload,\n                write_error="signature payload could not be staged",\n                verification_error="signature payload staging verification failed",\n            )\n            if stage_errors:\n                errors.extend(stage_errors)\n                return None\n',
            "            payload_path.write_bytes(payload)\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-signature-tempdir-failure":
    run_negative_control(
        "Android device-lab signed evidence helper signature tempdir failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "signature temporary directory could not be created",
            "signature temporary directory failures ignored",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-signature-spawn-failure":
    run_negative_control(
        "Android device-lab signed evidence helper signature spawn-failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "signature command could not be run",
            "signature command spawn failures ignored",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-signature-invalid-private-key":
    run_negative_control(
        "Android device-lab signed evidence helper signature invalid-private-key gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '            except subprocess.CalledProcessError:\n                errors.append("private key must be a valid OpenSSL Ed25519 private key")\n                return None\n',
            '            except subprocess.CalledProcessError:\n                errors.append("signature command could not be run")\n                return None\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-output-write":
    run_negative_control(
        "Android device-lab signed evidence helper output write gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '_write_json(output_path, evidence, "signed evidence output path")',
            '_write_json(output_path, evidence, "unchecked signed evidence output path")',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-json-write-failure":
    run_negative_control(
        "Android device-lab signed evidence helper JSON write-failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "        os.replace(tmp_path, path)\n",
            '        path.write_text(text, encoding="utf-8")\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-output-ancestor":
    run_negative_control(
        "Android device-lab signed evidence helper output ancestor gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    errors.extend(\n        device_lab.validate_no_symlink_ancestors(\n            path,\n            f"{label} ancestor directory",\n        )\n    )\n    if errors:\n        return errors\n    if not parent_exists:\n',
            '    if not parent_exists:\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-output-parent-is-dir-preflight":
    run_negative_control(
        "Android device-lab signed evidence helper output parent is_dir preflight gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "    if not stat.S_ISDIR(parent_mode):\n",
            "    if not parent.is_dir():\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-output-parent-metadata-failure":
    run_negative_control(
        "Android device-lab signed evidence helper output parent metadata failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    except OSError:\n        return False, [f"{label} parent directory metadata could not be read"]\n',
            "    except OSError:\n        return False, []\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-output-parent-create-failure":
    run_negative_control(
        "Android device-lab signed evidence helper output parent-create failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '        try:\n            parent.mkdir(parents=True, exist_ok=True)\n        except OSError:\n            errors.append(f"{label} parent directory could not be created")\n',
            "        parent.mkdir(parents=True, exist_ok=True)\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-output-post-create-parent-preflight":
    run_negative_control(
        "Android device-lab signed evidence helper output post-create parent preflight gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    parent_exists, parent_errors = _validate_json_output_parent(\n        path,\n        label,\n        missing_error=f"{label} parent must be a directory",\n    )\n    errors.extend(parent_errors)\n    if not parent_exists and not errors:\n        errors.append(f"{label} parent must be a directory")\n    if errors:\n        return errors\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-output-resolve-failure":
    run_negative_control(
        "Android device-lab signed evidence helper output resolve-failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '        except OSError:\n            errors.append("signed evidence output path could not be resolved")\n            return None\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-dangling-output-alias":
    run_negative_control(
        "Android device-lab signed evidence helper dangling output alias gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "if stat.S_ISLNK(mode):",
            "if False and stat.S_ISLNK(mode):",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-output-file-metadata-failure":
    run_negative_control(
        "Android device-lab signed evidence helper output file metadata failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    try:\n        mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return errors\n    except OSError:\n        errors.append(f"{label} file metadata could not be read")\n        return errors\n',
            '    try:\n        mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return errors\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-output-hardlink-metadata-failure":
    run_negative_control(
        "Android device-lab signed evidence helper output hardlink metadata failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '        try:\n            link_count = path.stat().st_nlink\n        except OSError:\n            errors.append(f"{label} hardlink metadata could not be read")\n        else:\n            if link_count > 1:\n                errors.append(f"{label} must not be hardlinked")\n',
            '        link_count = path.stat().st_nlink\n        if link_count > 1:\n            errors.append(f"{label} must not be hardlinked")\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-output-file-metadata-failure":
    run_negative_control(
        "Android device-lab signed evidence helper output file metadata failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    except OSError:\n        errors.append(f"{label} file metadata could not be read")\n        return errors\n',
            '    except OSError:\n        return errors\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-output-digest-preflight":
    run_negative_control(
        "Android device-lab signed evidence helper output digest preflight gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    errors = _validate_existing_json_output_path(path, label)\n    if errors:\n        return None, errors\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-output-digest-parent-missing":
    run_negative_control(
        "Android device-lab signed evidence helper output digest parent-missing gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '        missing_error=f"{label} parent directory is missing",\n',
            "        missing_error=None,\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-output-digest-leaf-missing":
    run_negative_control(
        "Android device-lab signed evidence helper output digest leaf-missing gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    except FileNotFoundError:\n        return [f"{label} must exist before digest"]\n',
            "    except FileNotFoundError:\n        return []\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-output-digest-file-metadata-failure":
    run_negative_control(
        "Android device-lab signed evidence helper output digest file metadata failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    try:\n        mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return [f"{label} must exist before digest"]\n    except OSError:\n        return [f"{label} file metadata could not be read"]\n',
            '    try:\n        mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return [f"{label} must exist before digest"]\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-output-digest-hardlink-metadata-failure":
    run_negative_control(
        "Android device-lab signed evidence helper output digest hardlink metadata failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        return [f"{label} hardlink metadata could not be read"]\n    if link_count > 1:\n        return [f"{label} must not be hardlinked"]\n',
            '    link_count = path.stat().st_nlink\n    if link_count > 1:\n        return [f"{label} must not be hardlinked"]\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-output-digest-file-metadata-failure":
    run_negative_control(
        "Android device-lab signed evidence helper output digest file metadata failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    except OSError:\n        return [f"{label} file metadata could not be read"]\n',
            "    except OSError:\n        return []\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-output-digest-read-failure":
    run_negative_control(
        "Android device-lab signed evidence helper output digest read-failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    except OSError:\n        return None, [f"{label} could not be read"]\n',
            "    except OSError:\n        return None, []\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-output-digest-open-path-binding":
    run_negative_control(
        "Android device-lab signed evidence helper output digest open-path binding gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "signer_output_expected_identity = (\n                expected_stat.st_dev,\n                expected_stat.st_ino,\n            )",
            "signer_output_expected_identity = (\n                open_stat.st_dev,\n                open_stat.st_ino,\n            )",
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

if mode == "--negative-control-android-device-lab-signing-helper-text-write-failure":
    run_negative_control(
        "Android device-lab signed evidence helper text write-failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "            os.fsync(handle.fileno())\n",
            "            handle.fileno()\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-readback-verification":
    run_negative_control(
        "Android device-lab signed evidence helper readback gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "readback_text != text",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-readback-failure":
    run_negative_control(
        "Android device-lab signed evidence helper readback failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '        if read_errors == [f"{label} could not be read"]:\n            return None, [f"{label} write verification failed"]\n',
            '        if False:\n            return None, [f"{label} write verification failed"]\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-post-write-preflight":
    run_negative_control(
        "Android device-lab signed evidence helper post-write preflight gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    errors = _validate_existing_json_output_path(path, label)\n    if errors:\n        return errors\n    try:\n        expected_stat = path.lstat()\n',
            '    try:\n        expected_stat = path.lstat()\n',
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

if mode == "--negative-control-android-device-lab-signing-helper-slot-metadata-failure":
    run_negative_control(
        "Android device-lab signed evidence helper slot metadata failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    except OSError:\n        return ["slot directory metadata could not be read"]\n',
            '    except OSError:\n        slot_mode = None\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-slot-parent-metadata-failure":
    run_negative_control(
        "Android device-lab signed evidence helper slot parent metadata failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    except OSError:\n        return ["slot parent directory metadata could not be read"]\n',
            '    except OSError:\n        parent_mode = None\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-slot-artifact-digest-preflight":
    run_negative_control(
        "Android device-lab signed evidence helper slot artifact digest preflight gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    artifact_path, artifact_stat, errors = _validate_slot_artifact_for_digest(\n        slot_path,\n        relative,\n    )\n    if errors:\n        return None, errors\n    assert artifact_path is not None and artifact_stat is not None\n',
            "    artifact_path = slot_path / relative\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-slot-artifact-hardlink-metadata-failure":
    run_negative_control(
        "Android device-lab signed evidence helper slot artifact hardlink metadata failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    try:\n        link_count = artifact_path.stat().st_nlink\n    except OSError:\n        return None, None, [\n            f"slot artifact {display} hardlink metadata could not be read"\n        ]\n',
            "    link_count = artifact_path.stat().st_nlink\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-slot-artifact-file-metadata-failure":
    run_negative_control(
        "Android device-lab signed evidence helper slot artifact file metadata failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    except OSError:\n        return None, None, [f"slot artifact {display} file metadata could not be read"]\n',
            "    except OSError:\n        return artifact_path, artifact_stat, []\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-slot-artifact-read-failure":
    run_negative_control(
        "Android device-lab signed evidence helper slot artifact read-failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    except OSError:\n        return None, [f"slot artifact {display} could not be read"]\n',
            "    except OSError:\n        return None, []\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-slot-artifact-open-path-binding":
    run_negative_control(
        "Android device-lab signed evidence helper slot artifact open-path binding gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "signer_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
            "signer_expected_identity = (open_stat.st_dev, open_stat.st_ino)",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-manifest-artifact-digest-preflight":
    run_negative_control(
        "Android device-lab manifest artifact digest preflight gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    artifact_path, errors = _validate_manifest_artifact_for_digest(slot_path, relative)\n    if errors:\n        return None, errors\n    assert artifact_path is not None\n',
            "    artifact_path = slot_path / relative\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-digest-artifact-file-metadata-failure":
    run_negative_control(
        "Android device-lab digest artifact file metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            'def _slot_artifact_lstat_mode(\n    artifact_path: Path,\n    metadata_error: str,\n) -> tuple[int | None, list[str]]:\n    try:\n        return artifact_path.lstat().st_mode, []\n    except FileNotFoundError:\n        return None, []\n    except OSError:\n        return None, [metadata_error]\n',
            'def _slot_artifact_lstat_mode(\n    artifact_path: Path,\n    metadata_error: str,\n) -> tuple[int | None, list[str]]:\n    return artifact_path.lstat().st_mode, []\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-manifest-artifact-read-failure":
    run_negative_control(
        "Android device-lab manifest artifact digest read-failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    try:\n        payload = artifact_path.read_bytes()\n    except OSError:\n        return None, [\n            "sha256sum.txt references artifact that could not be read "\n            f"{_display_path(relative)}"\n        ]\n    return hashlib.sha256(payload).hexdigest(), []\n',
            "    return hashlib.sha256(artifact_path.read_bytes()).hexdigest(), []\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-manifest-secret-paths":
    run_negative_control(
        "Android device-lab signed evidence helper manifest secret-path gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    slot_files = device_lab._slot_files(slot_path, errors)\n    if errors:\n        return errors\n    for relative in slot_files:\n        if device_lab.SECRET_RE.search(relative):\n            errors.append("slot artifacts must not contain secret-looking material")\n            return errors\n',
            '    slot_files = device_lab._slot_files(slot_path, errors)\n    if errors:\n        return errors\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-metadata-preflight":
    run_negative_control(
        "Android device-lab signed evidence helper metadata preflight gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    errors = _preflight_slot_metadata_reads(slot_path)\n    if errors:\n        return None, errors\n',
            "    errors = []\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-artifact-digests-preflight":
    run_negative_control(
        "Android device-lab signed evidence helper artifact digest preflight gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    preflight_errors = _preflight_slot_metadata_reads(slot_path)\n    if preflight_errors:\n        errors.extend(preflight_errors)\n        return None\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-direct-slot-secret-paths":
    run_negative_control(
        "Android device-lab signed evidence helper direct metadata slot secret-path gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            'def _validate_slot_path_boundary(slot_path: Path) -> list[str]:\n    """Validate signer slot paths before reading mutable slot artifacts."""\n\n    if device_lab.SECRET_RE.search(str(slot_path)):\n        return ["slot path must not contain secret-looking material"]\n',
            'def _validate_slot_path_boundary(slot_path: Path) -> list[str]:\n    """Validate signer slot paths before reading mutable slot artifacts."""\n\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-direct-manifest-slot-secret-paths":
    run_negative_control(
        "Android device-lab signed evidence helper direct manifest slot secret-path gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            'def _validate_slot_for_manifest_rewrite(slot_path: Path) -> list[str]:\n    """Validate a slot immediately before rewriting its SHA-256 manifest."""\n\n    path_errors = _validate_slot_path_boundary(slot_path)\n    if path_errors:\n        return path_errors\n\n    errors: list[str] = []\n',
            'def _validate_slot_for_manifest_rewrite(slot_path: Path) -> list[str]:\n    """Validate a slot immediately before rewriting its SHA-256 manifest."""\n\n    errors: list[str] = []\n',
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

if mode == "--negative-control-android-device-lab-public-key-hardlink-metadata-failure":
    run_negative_control(
        "Android device-lab public key hardlink metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    try:\n        link_count = public_key_path.stat().st_nlink\n    except OSError:\n        errors.append(f"{label} hardlink metadata could not be read")\n        return False\n',
            "    link_count = public_key_path.stat().st_nlink\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-public-key-file-metadata-failure":
    run_negative_control(
        "Android device-lab public key file metadata failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    except OSError:\n        errors.append(f"{label} file metadata could not be read")\n        return False\n',
            "    except OSError:\n        public_key_mode = None\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-public-key-regular-file-before-openssl":
    run_negative_control(
        "Android device-lab public key regular-file-before-OpenSSL gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    if not stat.S_ISREG(public_key_mode):\n        errors.append(f"{label} must be a regular file")\n        return False\n',
            '    if False and not stat.S_ISREG(public_key_mode):\n        errors.append(f"{label} must be a regular file")\n        return False\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-public-key-missing-before-openssl":
    run_negative_control(
        "Android device-lab public key missing-before-OpenSSL gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    if public_key_mode is None:\n        errors.append(f"{label} must point to an existing public key file")\n        return False\n',
            '    if public_key_mode is None:\n        pass\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-private-key-hardlink-metadata-failure":
    run_negative_control(
        "Android device-lab private key hardlink metadata failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    try:\n        link_count = private_key_path.stat().st_nlink\n    except OSError:\n        errors.append("private key hardlink metadata could not be read")\n        return None\n',
            "    link_count = private_key_path.stat().st_nlink\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-private-key-file-metadata-failure":
    run_negative_control(
        "Android device-lab private key file metadata failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    except OSError:\n        errors.append("private key file metadata could not be read")\n        return None\n',
            "    except OSError:\n        private_key_mode = None\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-private-key-regular-file-before-openssl":
    run_negative_control(
        "Android device-lab private key regular-file-before-OpenSSL gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    if not stat.S_ISREG(private_key_mode):\n        errors.append("private key must be a regular file")\n        return None\n',
            '    if False and not stat.S_ISREG(private_key_mode):\n        errors.append("private key must be a regular file")\n        return None\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-private-key-missing-before-openssl":
    run_negative_control(
        "Android device-lab private key missing-before-OpenSSL gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    if private_key_mode is None:\n        errors.append("private key must point to an existing file")\n        return None\n',
            '    if private_key_mode is None:\n        pass\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-public-key-path-before-openssl":
    run_negative_control(
        "Android device-lab public key path-before-OpenSSL gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    if not _validate_public_key_path_shape(public_key_path, errors=errors, label=label):\n        return None\n    openssl = _require_openssl(errors)\n',
            '    openssl = _require_openssl(errors)\n    if not _validate_public_key_path_shape(public_key_path, errors=errors, label=label):\n        return None\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-private-key-path-before-openssl":
    run_negative_control(
        "Android device-lab private key path-before-OpenSSL gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            'def _sign_ed25519(private_key_path: Path, payload: bytes, errors: list[str]) -> bytes | None:\n    secret_error = _secret_key_path_error(private_key_path, "private key")\n',
            'def _sign_ed25519(private_key_path: Path, payload: bytes, errors: list[str]) -> bytes | None:\n    openssl = device_lab._require_openssl(errors)\n    if openssl is None:\n        return None\n    secret_error = _secret_key_path_error(private_key_path, "private key")\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signature-verify-key-path-before-openssl":
    run_negative_control(
        "Android device-lab signature verifier key path-before-OpenSSL gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            'def _verify_ed25519_signature(\n    *,\n    public_key_path: Path,\n    payload: bytes,\n    signature: bytes,\n    errors: list[str],\n    label: str = "trusted signer public key",\n) -> None:\n    if not _validate_public_key_path_shape(public_key_path, errors=errors, label=label):\n        return\n    openssl = _require_openssl(errors)\n',
            'def _verify_ed25519_signature(\n    *,\n    public_key_path: Path,\n    payload: bytes,\n    signature: bytes,\n    errors: list[str],\n    label: str = "trusted signer public key",\n) -> None:\n    openssl = _require_openssl(errors)\n    if not _validate_public_key_path_shape(public_key_path, errors=errors, label=label):\n        return\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-private-public-pair-preserves-key-path-errors":
    run_negative_control(
        "Android device-lab private/public pair key-path error preservation gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    if verify_errors == ["signed evidence artifact signature verification failed"]:\n        errors.append(\n            "private key did not produce a signature accepted by the signer public key"\n        )\n    elif verify_errors:\n        errors.extend(verify_errors)\n',
            '    if verify_errors:\n        errors.append(\n            "private key did not produce a signature accepted by the signer public key"\n        )\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signer-key-secret-paths":
    run_negative_control(
        "Android device-lab signer key secret-path gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '            _secret_key_path_error(private_key_path, "private key"),\n            _secret_key_path_error(public_key_path, "signer public key"),\n',
            "",
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

if mode == "--negative-control-kagemusha-readiness-repo-root-metadata-failure":
    run_negative_control(
        "Kagemusha readiness direct repo-root metadata failure gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    try:\n        root_mode = root.lstat().st_mode\n    except FileNotFoundError:\n        root_mode = None\n    except OSError:\n        errors.append("--repo-root metadata could not be read")\n        return [\n            blocker("kagemusha_repo_root_path_invalid", error)\n            for error in errors\n        ]\n',
            '    try:\n        root_mode = root.lstat().st_mode\n    except FileNotFoundError:\n        root_mode = None\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-repo-root-resolve-failure":
    run_negative_control(
        "Kagemusha readiness repo-root resolve-failure gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '        try:\n            repo_root = Path(args.repo_root).resolve()\n        except OSError:\n            path_blockers.append(\n                blocker(\n                    "kagemusha_repo_root_path_invalid",\n                    "--repo-root could not be resolved",\n                )\n            )\n',
            "        repo_root = Path(args.repo_root).resolve()\n",
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

if mode == "--negative-control-kagemusha-readiness-android-root-discovery-read-failure":
    run_negative_control(
        "Kagemusha readiness Android root discovery read-failure gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "android_device_lab_root_unreadable",
            "android_device_lab_root_listing_failures_ignored",
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

if mode == "--negative-control-kagemusha-readiness-summary-output-dangling-alias":
    run_negative_control(
        "Kagemusha readiness summary output dangling alias gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "if stat.S_ISLNK(summary_output_mode):",
            "if stat.S_ISLNK(summary_output_mode) and path.exists():",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-summary-output-ancestor":
    run_negative_control(
        "Kagemusha readiness summary output ancestor alias gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    ancestor_errors = device_lab.validate_no_symlink_ancestors(\n        path,\n        "--summary-out ancestor directory",\n    )\n    if ancestor_errors:\n        return [\n            blocker(SUMMARY_OUT_PATH_INVALID_CODE, error)\n            for error in ancestor_errors\n        ]\n    if not parent_exists:\n',
            '    if not parent_exists:\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-summary-output-parent-is-dir-preflight":
    run_negative_control(
        "Kagemusha readiness summary output parent is_dir preflight gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "    if not stat.S_ISDIR(parent_mode):\n",
            "    if not parent.is_dir():\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-summary-output-parent-metadata-failure":
    run_negative_control(
        "Kagemusha readiness summary output parent metadata failure gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    except OSError:\n        return False, [\n            blocker(\n                SUMMARY_OUT_PATH_INVALID_CODE,\n                "--summary-out parent directory metadata could not be read",\n            )\n        ]\n',
            "    except OSError:\n        return False, []\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-summary-output-parent-create-failure":
    run_negative_control(
        "Kagemusha readiness summary output parent-create failure gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '        try:\n            parent.mkdir(parents=True, exist_ok=True)\n        except OSError:\n            return [\n                blocker(\n                    SUMMARY_OUT_PATH_INVALID_CODE,\n                    "--summary-out parent directory could not be created",\n                )\n            ]\n',
            "        parent.mkdir(parents=True, exist_ok=True)\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-summary-output-post-create-parent-preflight":
    run_negative_control(
        "Kagemusha readiness summary output post-create parent preflight gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    parent_exists, parent_blockers = _validate_summary_output_parent(\n        path,\n        missing_message="--summary-out parent must be a directory",\n    )\n    if parent_blockers:\n        return parent_blockers\n    if not parent_exists:\n        return [\n            blocker(\n                SUMMARY_OUT_PATH_INVALID_CODE,\n                "--summary-out parent must be a directory",\n            )\n        ]\n    ancestor_errors = device_lab.validate_no_symlink_ancestors(\n        path,\n        "--summary-out ancestor directory",\n    )\n    if ancestor_errors:\n        return [\n            blocker(SUMMARY_OUT_PATH_INVALID_CODE, error)\n            for error in ancestor_errors\n        ]\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-summary-output-regular-file":
    run_negative_control(
        "Kagemusha readiness summary output regular-file gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "if not stat.S_ISREG(summary_output_mode):",
            "if False and not stat.S_ISREG(summary_output_mode):",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-summary-output-file-metadata-failure":
    run_negative_control(
        "Kagemusha readiness summary output file metadata failure gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    try:\n        summary_output_mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return []\n    except OSError:\n        return [\n            blocker(\n                SUMMARY_OUT_PATH_INVALID_CODE,\n                "--summary-out file metadata could not be read",\n            )\n        ]\n',
            '    try:\n        summary_output_mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return []\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-summary-output-hardlink-metadata-failure":
    run_negative_control(
        "Kagemusha readiness summary output hardlink metadata failure gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        return [\n            blocker(\n                SUMMARY_OUT_PATH_INVALID_CODE,\n                "--summary-out hardlink metadata could not be read",\n            )\n        ]\n',
            "    link_count = path.stat().st_nlink\n",
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

if mode == "--negative-control-kagemusha-readiness-summary-output-write-failure":
    run_negative_control(
        "Kagemusha readiness summary output write-failure gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "os.replace(tmp_path, path)",
            'path.write_text(summary_text, encoding="utf-8")',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-summary-output-readback-verification":
    run_negative_control(
        "Kagemusha readiness summary output readback gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "readback_text != summary_text",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-summary-output-readback-failure":
    run_negative_control(
        "Kagemusha readiness summary output readback failure gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    except OSError:\n        return None, [\n            _summary_out_blocker("--summary-out write verification failed")\n        ]\n',
            "    except OSError:\n        return None, []\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-summary-output-readback-open-path-binding":
    run_negative_control(
        "Kagemusha readiness summary output readback open-path binding gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "summary_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
            "summary_expected_identity = (open_stat.st_dev, open_stat.st_ino)",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-summary-output-post-write-preflight":
    run_negative_control(
        "Kagemusha readiness summary output post-write preflight gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    errors = validate_summary_output_path(path)\n    if errors:\n        return errors\n    try:\n        expected_stat = path.lstat()\n',
            '    try:\n        expected_stat = path.lstat()\n',
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

if mode == "--negative-control-compact-key-release-tooling":
    run_negative_control(
        "ABI-7 compact key release tooling",
        lambda: override_text(
            "crates/iroha_cli/src/zk.rs",
            "derive_halo2_ipa_kagemusha_recursive_compact_payment_token_proving_key_bytes",
            "derive_halo2_ipa_kagemusha_recursive_compact_payment_token_disabled",
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

if mode == "--negative-control-lineage-key-release-source-marker-non-utf8-read":
    run_negative_control(
        "Reserved-lineage key release source marker non-UTF-8 read gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    except UnicodeDecodeError:\n        return None, [unreadable_error]\n',
            '    except UnicodeDecodeError:\n        return "", []\n',
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

if mode == "--negative-control-compact-key-evidence":
    run_negative_control(
        "ABI-7 recursive compact key evidence",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "compact_key_evidence_missing",
            "compact_key_evidence_optional",
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

if mode == "--negative-control-compact-key-evidence-path-aliases":
    run_negative_control(
        "ABI-7 recursive compact key evidence path alias gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "compact_key_evidence_path=compact_key_evidence_path,",
            "compact_key_evidence_path=compact_key_evidence_path.resolve(),",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-summary-drift":
    run_negative_control(
        "Kagemusha release bundle summary drift gate",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            """blockers.extend(
            _compare_validated_sections(
                summary,
                abi6,
                abi7,
                lineage_tooling,
                lineage,
                compact,
                android,
            )
        )""",
            "pass",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-summary-section-schema":
    run_negative_control(
        "Kagemusha release bundle summary section schema gate",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "kagemusha_release_summary_section_blockers_present",
            "kagemusha_release_summary_section_blockers_disabled",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-android-signed-evidence-summary-schema":
    run_negative_control(
        "Kagemusha release bundle Android signed-evidence summary schema gate",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "kagemusha_release_summary_android_signed_evidence_missing_field",
            "android_signed_evidence_missing_field_disabled",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-artifact-inventory":
    run_negative_control(
        "Kagemusha release bundle artifact inventory",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            '"lineage_artifacts"',
            '"lineageArtifactInventoryDisabled"',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-android-slot-artifact-inventory":
    run_negative_control(
        "Kagemusha release bundle Android slot artifact inventory",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            '"android_slot_artifacts"',
            '"androidSlotArtifactsDisabled"',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-compact-placeholder-inventory":
    run_negative_control(
        "Kagemusha release bundle compact placeholder inventory",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "artifact_content_validator=readiness.validate_compact_key_artifact_content,",
            "artifact_content_validator=None,",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-compact-generator-log-inventory":
    run_negative_control(
        "Kagemusha release bundle compact generator log inventory",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            '"compact_key_generator_log"',
            '"compactKeyGeneratorLogDisabled"',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-evidence-entry-nonempty":
    run_negative_control(
        "Kagemusha release bundle evidence entry non-empty gate",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "if size <= 0:",
            "if size < 0:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-evidence-entry-open-path-binding":
    run_negative_control(
        "Kagemusha release bundle evidence entry open path binding",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "sized_digest_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
            "sized_digest_expected_identity = (open_stat.st_dev, open_stat.st_ino)",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-json-input-open-path-binding":
    run_negative_control(
        "Kagemusha release bundle JSON input open path binding",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "release_json_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
            "release_json_expected_identity = (open_stat.st_dev, open_stat.st_ino)",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-digest-open-path-binding":
    run_negative_control(
        "Kagemusha release bundle digest open path binding",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "digest_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
            "digest_expected_identity = (open_stat.st_dev, open_stat.st_ino)",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-atomic-output":
    run_negative_control(
        "Kagemusha release bundle atomic output",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "os.replace(tmp_path, path)",
            'path.write_text(manifest_text, encoding="utf-8")',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-output-readback-failure":
    run_negative_control(
        "Kagemusha release bundle output readback failure gate",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            '    except OSError:\n        return None, [\n            _release_bundle_out_blocker("--out could not be read back after writing")\n        ]\n',
            "    except OSError:\n        return None, []\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-output-readback-open-path-binding":
    run_negative_control(
        "Kagemusha release bundle output readback open-path binding gate",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "output_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
            "output_expected_identity = (open_stat.st_dev, open_stat.st_ino)",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-output-post-write-preflight":
    run_negative_control(
        "Kagemusha release bundle output post-write preflight",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            '    try:\n        expected_stat = path.lstat()\n    except (FileNotFoundError, OSError):\n        return [\n            _release_bundle_out_blocker("--out could not be read back after writing")\n        ]\n',
            '    expected_stat = path.stat()\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-verify-existing":
    run_negative_control(
        "Kagemusha release bundle verify-existing gate",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "def verify_release_bundle(",
            "def verify_release_bundle_disabled(",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-verify-existing-preflight":
    run_negative_control(
        "Kagemusha release bundle verify-existing path preflight",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            'existing_bundle_path,\n        bundle_root,\n        "Kagemusha release bundle manifest",',
            'existing_bundle_path,\n        bundle_root,\n        "Kagemusha release bundle manifest disabled",',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-verify-existing-evidence-path-shape":
    run_negative_control(
        "Kagemusha release bundle verify-existing evidence path-shape gate",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            'blockers.extend(_check_release_bundle_evidence_paths(bundle.get("evidence")))',
            "blockers.extend([])",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-input-path-preflight":
    run_negative_control(
        "Kagemusha release bundle input path preflight",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "        and input_paths_ok\n",
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-scan-preflight":
    run_negative_control(
        "Kagemusha release bundle scanner preflight",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "if input_paths_ok and summary_path_ok:",
            "if summary_path_ok:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-output-overwrite":
    run_negative_control(
        "Kagemusha release bundle output overwrite gate",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "--out must not overwrite bundled evidence input",
            "--out may overwrite bundled evidence input",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-readiness-direct-hash-shape":
    run_negative_control(
        "Reserved-lineage proof readiness direct hash path-shape gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    expected_stat, file_errors = _validate_lineage_local_file_for_read(path, label)\n    if file_errors:\n        return None, file_errors\n',
            '    file_errors = []\n    expected_stat = path.stat()\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-readiness-direct-hash-read-failure":
    run_negative_control(
        "Reserved-lineage proof readiness direct hash read-failure gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            'except OSError:\n        return None, [f"{label} could not be read"]',
            'except OSError:\n        raise',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-local-secret-paths":
    run_negative_control(
        "Reserved-lineage proof evidence local secret-path gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    if device_lab.SECRET_RE.search(str(path)):\n        return None, [f"{label} path must not contain secret-looking material"]\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-local-ancestor-aliases":
    run_negative_control(
        "Reserved-lineage proof evidence local ancestor alias gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    ancestor_errors = device_lab.validate_no_symlink_ancestors(\n        path,\n        f"{label} ancestor directory",\n    )\n    if ancestor_errors:\n        return None, ancestor_errors\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-local-hardlink-metadata-failure":
    run_negative_control(
        "Reserved-lineage proof evidence local hardlink metadata failure gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        return None, [f"{label} hardlink metadata could not be read"]\n    if link_count > 1:\n        return None, [f"{label} must not be hardlinked"]\n',
            '    link_count = path.stat().st_nlink\n    if link_count > 1:\n        return None, [f"{label} must not be hardlinked"]\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-local-file-metadata-failure":
    run_negative_control(
        "Reserved-lineage proof evidence local file metadata failure gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    try:\n        file_stat = path.lstat()\n    except FileNotFoundError:\n        return None, [f"{label} is missing"]\n    except OSError:\n        return None, [f"{label} file metadata could not be read"]\n',
            '    try:\n        file_stat = path.lstat()\n    except FileNotFoundError:\n        return None, [f"{label} is missing"]\n    except OSError:\n        return None, [f"{label} is missing"]\n',
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

if mode == "--negative-control-lineage-proof-artifact-is-file-preflight":
    run_negative_control(
        "Reserved-lineage proof evidence artifact is_file preflight gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '                continue\n            (\n                actual_digest,\n                artifact_size,\n                artifact_prefix,\n                digest_errors,\n            ) = _sha256_file_with_size_and_prefix(\n                artifact_path,\n                "Reserved-lineage proof evidence artifact file",\n                allow_empty=True,\n            )\n',
            '                continue\n            if not artifact_path.is_file():\n                blockers.append(\n                    blocker(\n                        "lineage_proof_evidence_artifact_missing",\n                        "Reserved-lineage proof evidence artifact file is missing",\n                        artifact=artifact,\n                    )\n                )\n                continue\n            (\n                actual_digest,\n                artifact_size,\n                artifact_prefix,\n                digest_errors,\n            ) = _sha256_file_with_size_and_prefix(\n                artifact_path,\n                "Reserved-lineage proof evidence artifact file",\n                allow_empty=True,\n            )\n',
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

if mode == "--negative-control-lineage-proof-helper-output-dangling-alias":
    run_negative_control(
        "Reserved-lineage proof evidence helper dangling output alias gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            "if stat.S_ISLNK(output_mode):",
            "if stat.S_ISLNK(output_mode) and path.exists():",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-output-ancestor":
    run_negative_control(
        "Reserved-lineage proof evidence helper output ancestor gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            '    output_ancestor_errors = device_lab.validate_no_symlink_ancestors(\n        path,\n        f"{label} ancestor directory",\n    )\n    if output_ancestor_errors:\n        return output_ancestor_errors\n    if not parent_exists:\n',
            '    if not parent_exists:\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-output-parent-is-dir-preflight":
    run_negative_control(
        "Reserved-lineage proof evidence helper output parent is_dir preflight gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            "    if not stat.S_ISDIR(parent_mode):\n",
            "    if not parent.is_dir():\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-output-parent-metadata-failure":
    run_negative_control(
        "Reserved-lineage proof evidence helper output parent metadata failure gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            '    except OSError:\n        return False, [f"{label} parent directory metadata could not be read"]\n',
            "    except OSError:\n        return False, []\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-output-parent-create-failure":
    run_negative_control(
        "Reserved-lineage proof evidence helper output parent-create failure gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            '    if not parent_exists:\n        try:\n            parent.mkdir(parents=True, exist_ok=True)\n        except OSError:\n            return [f"{label} parent directory could not be created"]\n',
            "    if not parent_exists:\n        parent.mkdir(parents=True, exist_ok=True)\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-output-post-create-parent-preflight":
    run_negative_control(
        "Reserved-lineage proof evidence helper output post-create parent preflight gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            '    parent_exists, parent_errors = _validate_output_parent(\n        path,\n        label,\n        missing_error=f"{label} parent must be a directory",\n    )\n    if parent_errors:\n        return parent_errors\n    if not parent_exists:\n        return [f"{label} parent must be a directory"]\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-output-validate-parent-create-failure":
    run_negative_control(
        "Reserved-lineage proof evidence helper output validator parent-create failure gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            '    errors = preflight_output_path(path, label)\n    if errors:\n        return errors\n    parent = path.parent\n    parent_exists, parent_errors = _validate_output_parent(path, label)\n    if parent_errors:\n        return parent_errors\n    if not parent_exists:\n        try:\n            parent.mkdir(parents=True, exist_ok=True)\n        except OSError:\n            return [f"{label} parent directory could not be created"]\n    return preflight_output_path(path, label)\n',
            '    errors = preflight_output_path(path, label)\n    if errors:\n        return errors\n    parent = path.parent\n    parent_exists, parent_errors = _validate_output_parent(path, label)\n    if parent_errors:\n        return parent_errors\n    if not parent_exists:\n        parent.mkdir(parents=True, exist_ok=True)\n    return preflight_output_path(path, label)\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-output-hardlink-metadata-failure":
    run_negative_control(
        "Reserved-lineage proof evidence helper output hardlink metadata failure gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        return [f"{label} hardlink metadata could not be read"]\n    if link_count > 1:\n        return [f"{label} must not be hardlinked"]\n',
            '    link_count = path.stat().st_nlink\n    if link_count > 1:\n        return [f"{label} must not be hardlinked"]\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-output-file-metadata-failure":
    run_negative_control(
        "Reserved-lineage proof evidence helper output file metadata failure gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            '    try:\n        output_mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return []\n    except OSError:\n        return [f"{label} file metadata could not be read"]\n',
            '    try:\n        output_mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return []\n',
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

if mode == "--negative-control-lineage-proof-helper-output-write-failure":
    run_negative_control(
        "Reserved-lineage proof evidence helper output write-failure gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            "os.replace(tmp_path, path)",
            'path.write_text(evidence_text, encoding="utf-8")',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-output-readback-verification":
    run_negative_control(
        "Reserved-lineage proof evidence helper output readback gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            "readback_text != evidence_text",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-output-readback-failure":
    run_negative_control(
        "Reserved-lineage proof evidence helper output readback failure gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            '    except OSError:\n        return None, [f"{label} write verification failed"]',
            "    except OSError:\n        return None, []",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-output-readback-open-path-binding":
    run_negative_control(
        "Reserved-lineage proof evidence helper output readback open-path binding gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            "output_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
            "output_expected_identity = (open_stat.st_dev, open_stat.st_ino)",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-output-post-write-preflight":
    run_negative_control(
        "Reserved-lineage proof evidence helper output post-write preflight gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            '    errors = validate_output_path(path, "--out")\n    if errors:\n        return errors\n    try:\n        expected_stat = path.lstat()',
            '    try:\n        expected_stat = path.lstat()',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-artifact-open-path-binding":
    run_negative_control(
        "Reserved-lineage proof evidence helper artifact open path binding",
        lambda: override_text_all(
            "scripts/kagemusha_lineage_proof_evidence.py",
            "expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
            "expected_identity = (open_stat.st_dev, open_stat.st_ino)",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-artifact-prefix-binding":
    run_negative_control(
        "Reserved-lineage proof evidence artifact prefix binding",
        lambda: (
            override_text(
                "scripts/kagemusha_production_readiness.py",
                "content_errors = validate_lineage_artifact_prefix(artifact_prefix, artifact)",
                "content_errors = validate_lineage_artifact_content(artifact_path, artifact)",
            ),
            override_text(
                "scripts/kagemusha_lineage_proof_evidence.py",
                "content_errors = readiness.validate_lineage_artifact_prefix(artifact_prefix, artifact)",
                "content_errors = readiness.validate_lineage_artifact_content(path, artifact)",
            ),
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-helper-output-early-preflight":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper early output preflight gate",
        lambda: override_text(
            "scripts/kagemusha_recursive_compact_key_evidence.py",
            'path_errors.extend(preflight_output_path(out_path, "--out"))',
            "path_errors.extend([])",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-helper-output-parent-create-failure":
    def mutate_compact_key_output_parent_create_failure() -> None:
        target = "scripts/kagemusha_recursive_compact_key_evidence.py"
        old = '    if not parent_exists:\n        try:\n            parent.mkdir(parents=True, exist_ok=True)\n        except OSError:\n            return [f"{label} parent directory could not be created"]\n'
        new = '    if not parent_exists:\n        parent.mkdir(parents=True, exist_ok=True)\n'
        text = read_text(target)
        if old not in text:
            raise SystemExit(f"negative control setup failed: `{old}` not found in {target}")
        text_overrides[target] = text.replace(old, new)

    run_negative_control(
        "ABI-7 recursive compact key evidence helper output parent-create failure gate",
        mutate_compact_key_output_parent_create_failure,
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-helper-output-file-metadata-failure":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper output file metadata failure gate",
        lambda: override_text(
            "scripts/kagemusha_recursive_compact_key_evidence.py",
            '    try:\n        output_mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return []\n    except OSError:\n        return [f"{label} file metadata could not be read"]\n',
            '    try:\n        output_mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return []\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-helper-output-hardlink-metadata-failure":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper output hardlink metadata failure gate",
        lambda: override_text(
            "scripts/kagemusha_recursive_compact_key_evidence.py",
            '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        return [f"{label} hardlink metadata could not be read"]\n    if link_count > 1:\n        return [f"{label} must not be hardlinked"]\n',
            '    link_count = path.stat().st_nlink\n    if link_count > 1:\n        return [f"{label} must not be hardlinked"]\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-helper-output-write-failure":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper output write-failure gate",
        lambda: override_text(
            "scripts/kagemusha_recursive_compact_key_evidence.py",
            "os.replace(tmp_path, path)",
            'path.write_text(evidence_text, encoding="utf-8")',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-helper-output-readback-verification":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper output readback gate",
        lambda: override_text(
            "scripts/kagemusha_recursive_compact_key_evidence.py",
            "readback_text != evidence_text",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-helper-output-readback-failure":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper output readback failure gate",
        lambda: override_text(
            "scripts/kagemusha_recursive_compact_key_evidence.py",
            '    except OSError:\n        return None, [f"{label} write verification failed"]',
            "    except OSError:\n        return None, []",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-helper-output-readback-open-path-binding":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper output readback open-path binding gate",
        lambda: override_text(
            "scripts/kagemusha_recursive_compact_key_evidence.py",
            "output_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
            "output_expected_identity = (open_stat.st_dev, open_stat.st_ino)",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-helper-output-post-write-preflight":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper output post-write preflight gate",
        lambda: override_text(
            "scripts/kagemusha_recursive_compact_key_evidence.py",
            '    errors = validate_output_path(path, "--out")\n    if errors:\n        return errors\n    try:\n        expected_stat = path.lstat()',
            '    try:\n        expected_stat = path.lstat()',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-helper-artifact-open-path-binding":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper artifact open path binding",
        lambda: override_text_all(
            "scripts/kagemusha_recursive_compact_key_evidence.py",
            "expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
            "expected_identity = (open_stat.st_dev, open_stat.st_ino)",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-artifact-prefix-binding":
    run_negative_control(
        "ABI-7 recursive compact key evidence artifact prefix binding",
        lambda: (
            override_text(
                "scripts/kagemusha_production_readiness.py",
                "content_errors = validate_compact_key_artifact_prefix(artifact_prefix, artifact)",
                "content_errors = validate_compact_key_artifact_content(artifact_path, artifact)",
            ),
            override_text(
                "scripts/kagemusha_recursive_compact_key_evidence.py",
                "readiness.validate_compact_key_artifact_prefix(artifact_prefix, artifact)",
                "readiness.validate_compact_key_artifact_content(path, artifact)",
            ),
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-helper-validation-dir-create-failure":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper validation dir create-failure gate",
        lambda: override_text(
            "scripts/kagemusha_recursive_compact_key_evidence.py",
            '    try:\n        artifact_dir.mkdir(parents=True, exist_ok=True)\n    except OSError:\n        return ["--artifact-dir could not be created for evidence validation"]\n',
            '    artifact_dir.mkdir(parents=True, exist_ok=True)\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-helper-validation-temp-write-failure":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper validation temp write-failure gate",
        lambda: override_text(
            "scripts/kagemusha_recursive_compact_key_evidence.py",
            "recursive compact key evidence validation file could not be written",
            "recursive compact key evidence validation file write failures ignored",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-helper-validation-temp-cleanup-failure":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper validation temp cleanup-failure gate",
        lambda: override_text(
            "scripts/kagemusha_recursive_compact_key_evidence.py",
            "recursive compact key evidence validation file could not be removed",
            "recursive compact key evidence validation file cleanup failures ignored",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-helper-direct-artifact-dir-secret-paths":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper direct artifact-dir secret-path gate",
        lambda: override_text(
            "scripts/kagemusha_recursive_compact_key_evidence.py",
            'def validate_artifact_dir_path(artifact_dir: Path) -> list[str]:\n    """Reject artifact directories that could alias external release bytes."""\n\n    secret_error = _secret_path_error(str(artifact_dir), "--artifact-dir")\n    if secret_error is not None:\n        return [secret_error]\n',
            'def validate_artifact_dir_path(artifact_dir: Path) -> list[str]:\n    """Reject artifact directories that could alias external release bytes."""\n\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-helper-direct-artifact-dir-metadata-failure":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper direct artifact-dir metadata failure gate",
        lambda: override_text(
            "scripts/kagemusha_recursive_compact_key_evidence.py",
            '    try:\n        artifact_dir_mode = artifact_dir.lstat().st_mode\n    except FileNotFoundError:\n        artifact_dir_mode = None\n    except OSError:\n        return ["--artifact-dir metadata could not be read"]\n',
            '    try:\n        artifact_dir_mode = artifact_dir.lstat().st_mode\n    except FileNotFoundError:\n        artifact_dir_mode = None\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-helper-direct-hash-shape":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper direct hash-shape gate",
        lambda: override_text(
            "scripts/kagemusha_recursive_compact_key_evidence.py",
            '    expected_stat, file_errors = readiness._validate_lineage_local_file_for_read(\n        path,\n        label,\n    )\n    if file_errors:\n        return None, file_errors\n',
            '    file_errors = []\n    expected_stat = path.stat()\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-helper-direct-hash-read-failure":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper direct hash read-failure gate",
        lambda: override_text(
            "scripts/kagemusha_recursive_compact_key_evidence.py",
            'except OSError:\n        return None, [f"{label} could not be read"]',
            'except OSError:\n        raise',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-helper-generator-log-strict-read":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper generator-log strict-read gate",
        lambda: override_text(
            "scripts/kagemusha_recursive_compact_key_evidence.py",
            'except UnicodeDecodeError:\n        return None, None, None, [f"{label} could not be read"]',
            'except UnicodeDecodeError:\n        raise',
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

if mode == "--negative-control-lineage-proof-helper-direct-hash-shape":
    run_negative_control(
        "Reserved-lineage proof evidence helper direct hash path-shape gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            '    expected_stat, file_errors = readiness._validate_lineage_local_file_for_read(\n        path,\n        label,\n    )\n    if file_errors:\n        return None, file_errors\n',
            '    file_errors = []\n    expected_stat = path.stat()\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-direct-hash-read-failure":
    run_negative_control(
        "Reserved-lineage proof evidence helper direct hash read-failure gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            'except OSError:\n        return None, [f"{label} could not be read"]',
            'except OSError:\n        raise',
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

if mode == "--negative-control-lineage-proof-helper-direct-artifact-dir-metadata-failure":
    run_negative_control(
        "Reserved-lineage proof evidence helper direct artifact-dir metadata failure gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            '    try:\n        artifact_dir_mode = artifact_dir.lstat().st_mode\n    except FileNotFoundError:\n        artifact_dir_mode = None\n    except OSError:\n        return ["--artifact-dir metadata could not be read"]\n',
            '    try:\n        artifact_dir_mode = artifact_dir.lstat().st_mode\n    except FileNotFoundError:\n        artifact_dir_mode = None\n',
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

if mode == "--negative-control-lineage-proof-helper-validation-dir-create-failure":
    run_negative_control(
        "Reserved-lineage proof evidence helper validation dir create-failure gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            '    try:\n        artifact_dir.mkdir(parents=True, exist_ok=True)\n    except OSError:\n        return ["--artifact-dir could not be created for evidence validation"]\n',
            '    artifact_dir.mkdir(parents=True, exist_ok=True)\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-validation-temp-write-failure":
    run_negative_control(
        "Reserved-lineage proof evidence helper validation temp write-failure gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            "lineage proof evidence validation file could not be written",
            "lineage proof evidence validation file write failures ignored",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-validation-temp-cleanup-failure":
    run_negative_control(
        "Reserved-lineage proof evidence helper validation temp cleanup-failure gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            "lineage proof evidence validation file could not be removed",
            "lineage proof evidence validation file cleanup failures ignored",
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

if mode == "--negative-control-lineage-proof-helper-input-corridor-resolve-failure":
    run_negative_control(
        "Reserved-lineage proof evidence helper input corridor resolve-failure gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            '    same_parent, corridor_errors = _same_resolved_parent(proof_log, artifact_dir)\n    if corridor_errors:\n        return corridor_errors\n    if proof_log.name != expected_proof_log_name or not same_parent:\n',
            '    same_parent = proof_log.parent.resolve() == artifact_dir.resolve()\n    if proof_log.name != expected_proof_log_name or not same_parent:\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-output-corridor-resolve-failure":
    run_negative_control(
        "Reserved-lineage proof evidence helper output corridor resolve-failure gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            "path_errors.extend(validate_output_corridor(out_path, artifact_dir))",
            'if out_path.resolve().parent != artifact_dir.resolve():\n        path_errors.append("--out must be written directly under --artifact-dir")',
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

if mode == "--negative-control-compact-key-command-canonical":
    run_negative_control(
        "ABI-7 recursive compact key evidence canonical command gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "must exactly match the canonical ABI-7 recursive compact keygen command string",
            "canonical compact key command spelling accepted",
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

if mode == "--negative-control-compact-key-scalar-types":
    run_negative_control(
        "ABI-7 recursive compact key evidence scalar type gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "not isinstance(compact_scalar_value, int)",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-artifact-size-binding":
    run_negative_control(
        "Reserved-lineage proof evidence artifact size binding",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "_require_lineage_artifact_size",
            "_lineage_artifact_size_disabled",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-readiness-artifact-open-path-binding":
    run_negative_control(
        "Reserved-lineage proof readiness artifact open-path binding",
        lambda: override_text_all(
            "scripts/kagemusha_production_readiness.py",
            "expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
            "expected_identity = (open_stat.st_dev, open_stat.st_ino)",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-artifact-size-binding":
    run_negative_control(
        "ABI-7 recursive compact key artifact size binding",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "_require_compact_key_artifact_size",
            "_compact_key_artifact_size_disabled",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-readiness-artifact-open-path-binding":
    run_negative_control(
        "ABI-7 recursive compact key readiness artifact open-path binding",
        lambda: override_text_all(
            "scripts/kagemusha_production_readiness.py",
            "expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
            "expected_identity = (open_stat.st_dev, open_stat.st_ino)",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-placeholder-artifacts":
    run_negative_control(
        "ABI-7 recursive compact key placeholder artifact gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "must be generated key material, not a placeholder fixture",
            "may use placeholder fixture material",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-generator-log-binding":
    run_negative_control(
        "ABI-7 recursive compact key generator log binding",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "compact_key_evidence_generator_log_artifact_size",
            "compact_key_evidence_generator_log_unchecked_size",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-generator-log-digest-binding":
    run_negative_control(
        "ABI-7 recursive compact key generator log digest binding",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "compact_key_evidence_generator_log_artifact_digest",
            "compact_key_evidence_generator_log_unchecked_digest",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-generator-log-open-path-binding":
    run_negative_control(
        "ABI-7 recursive compact key generator log open-path binding",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            'digest, text, read_errors = _sha256_text_file(\n        path,\n        "ABI-7 recursive compact key generator log",\n        "ABI-7 recursive compact key generator log could not be read",',
            'digest, text, read_errors = _sha256_text_file_unbound(\n        path,\n        "ABI-7 recursive compact key generator log",\n        "ABI-7 recursive compact key generator log could not be read",',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-timestamp-raw":
    run_negative_control(
        "Reserved-lineage proof evidence raw timestamp gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "device_lab.SIGNED_AT_UTC_RE.fullmatch(generated_at_raw)",
            "device_lab.SIGNED_AT_UTC_RE.fullmatch(generated_at_raw.strip())",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-timestamp-raw":
    run_negative_control(
        "ABI-7 recursive compact key evidence raw timestamp gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "compact_generated_at_raw = generated_at_text",
            "compact_generated_at_stripped = generated_at_text.strip()\n        compact_generated_at_raw = compact_generated_at_stripped",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-signed-evidence-timestamp-raw":
    run_negative_control(
        "Android signed-evidence report raw timestamp gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "if device_lab.SIGNED_AT_UTC_RE.fullmatch(signed_at_text) is None:",
            "if device_lab.SIGNED_AT_UTC_RE.fullmatch(signed_at_text.strip()) is None:",
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

if mode == "--negative-control-lineage-proof-log-metadata-read-failure":
    run_negative_control(
        "Reserved-lineage proof evidence proof-log metadata read-failure gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    try:\n        if path.stat().st_size > MAX_LINEAGE_PROOF_LOG_BYTES:\n            return None, [\n                f"production proof log must be no more than {MAX_LINEAGE_PROOF_LOG_BYTES} bytes"\n            ]\n    except OSError:\n        return None, ["production proof log metadata could not be read"]\n',
            '    if path.stat().st_size > MAX_LINEAGE_PROOF_LOG_BYTES:\n        return None, [\n            f"production proof log must be no more than {MAX_LINEAGE_PROOF_LOG_BYTES} bytes"\n        ]\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-log-is-file-preflight":
    run_negative_control(
        "Reserved-lineage proof evidence proof-log is_file preflight gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '            actual_log_digest, log_errors = validate_lineage_proof_log(\n                log_artifact_path, expected_name\n            )\n            log_file_missing = log_errors == ["missing production proof log"]\n',
            '            log_file_exists = log_artifact_path.is_file()\n            actual_log_digest, log_errors = validate_lineage_proof_log(\n                log_artifact_path, expected_name\n            )\n            log_file_missing = not log_file_exists\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-log-text-preflight":
    run_negative_control(
        "Reserved-lineage proof evidence proof-log text preflight gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            'text = b"".join(chunks).decode("utf-8", errors=decode_errors)',
            'text = path.read_text(encoding="utf-8", errors=decode_errors)',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-log-open-path-binding":
    run_negative_control(
        "Reserved-lineage proof evidence proof-log open-path binding",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            'digest, text, read_errors = _sha256_text_file(\n        path,\n        "production proof log",\n        "production proof log could not be read",',
            'digest, text, read_errors = _sha256_text_file_unbound(\n        path,\n        "production proof log",\n        "production proof log could not be read",',
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

if mode == "--negative-control-compact-key-evidence-filename":
    run_negative_control(
        "ABI-7 recursive compact key evidence filename gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "compact_key_evidence_filename",
            "compact_key_evidence_any_filename",
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

if mode == "--negative-control-compact-key-closed-schema":
    run_negative_control(
        "ABI-7 recursive compact key evidence closed schema",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "compact_key_evidence_unexpected_field",
            "compact_key_evidence_allows_extra_fields",
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

print("Kagemusha production readiness is routed through ABI-6 Reserved-lineage recursive spend; ABI-7 recursive compact has package-aware one-hop/append proof wiring while production default selection remains blocked")
PY
