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
        "recursive-compact-key-staged-run.json",
        "generator-log byte count",
        "execution-report SHA-256",
        "runner-report binding",
        "elapsed-seconds sidecar",
        "runner's exact six-fractional-digit",
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
        "readiness-summary Android matrix lists",
        "per-slot `signed_evidence` map",
        "trusted-signer digest lists",
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
        "MiB evidence JSON cap",
        "ABI-6 manifest is likewise capped at 1 MiB",
        "readiness-summary",
        "release-bundle JSON writers also reject non-finite",
        "release-bundle CLI and writer now also reject control-character",
        "Direct Android device-lab scanner path preflights now reject",
        "collapse to the same redacted signed-evidence",
        "redacted report-key collisions",
        "malformed direct report statuses",
        "normalize non-string direct report keys",
        "redact non-finite direct report numbers",
        "normalize unsupported direct report values",
        "normalize malformed direct report error lists",
        "normalize malformed direct Kagemusha report sections",
        "require canonical device-family strings before matrix",
        "Signed-evidence helper path preflights now also reject control-character",
        "Signed-slot assembler source-copy preflights now reject control-character",
        "Signed-slot assembler source digest preflights now reject blank or noncanonical",
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
        "verify-existing manifest JSON inputs are capped at 16 MiB",
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
        "ADB `getprop`",
        "exact one-LF values",
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
        "recursive-compact-key-staged-run.json",
        "generator-log byte count",
        "runner-report binding",
        "captured `record-archive-proof.log`",
        "lineage-proof-staged-run.json",
        "proof-log byte count",
        "lineage-key-artifact log byte counts",
        "execution-report SHA-256",
        "runner-report binding",
        "exact positive decimal line with six fractional digits",
        "missing-vs-unreadable state",
        "Path.is_file()",
        "hashes and parses the local proof log",
        "single expected `test ... ok` line",
        "Marker-stuffed proof logs with extra passing tests",
        "recorded command is the production",
        "run exactly as the canonical command string",
        "no quoted-token aliases, newlines, or appended shell commands",
        "set -o pipefail",
        "so `tee` cannot mask a terminated prover or key-generation",
        "runtime lineage keygen unset",
        "`cargo test -p iroha_core",
        "kagemusha_recursive_spend_lineage_init_append_from_record_archives_proves_reserved_lineage_output",
        "tee artifacts/kagemusha/record-archive-proof.log",
        "python3 scripts/kagemusha_run_lineage_proof_staged.py",
        "python3 scripts/kagemusha_run_recursive_compact_keygen_staged.py",
        "python3 scripts/kagemusha_lineage_proof_evidence.py",
        "python3 scripts/kagemusha_recursive_compact_key_evidence.py",
        "python3 scripts/kagemusha_finalize_lineage_proof_staged_run.py",
        "python3 scripts/kagemusha_finalize_recursive_compact_key_staged_run.py",
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
        "ADB `getprop`",
        "LF-terminated value",
        "rejects summary drift",
        "extra release claims",
        "readiness-summary Android matrix lists",
        "per-slot `signed_evidence` map",
        "trusted-signer digest lists",
        "are",
        "rejected instead of ignored",
        "duplicate JSON object keys are also invalid",
        "last-key-wins evidence packets",
        "ABI-6 manifest JSON capped at 1 MiB",
        "capped at 16 MiB before parsing",
        "readiness summary writer",
        "serialize with strict JSON",
        "non-finite values such as `NaN`",
        "`Infinity` fail before any temporary release output",
        "control-character paths, secret-looking paths",
        "stops before loading any readiness JSON",
        "Control-character `--out` values",
        "`--out` cannot",
        "already hash-bound into the manifest",
        "--verify-existing dist/kagemusha-production-release-bundle.json",
        "stable manifest comparison",
        "capped at 16 MiB from the opened file metadata",
        "future-dated beyond the release validator",
        "clock-skew allowance, remains blocked",
        "timestamp must use canonical UTC",
        "helper rejects noncanonical `--generated-at-utc`",
        "normalizing them into",
        "symlink-ancestor output aliases",
        "checked-in ABI-6 manifest plus ABI-7",
        "symlink-free ancestors",
        "reading compact key artifacts",
        "Direct Android device-lab scanner path preflights reject control-character",
        "collapse to the same redacted signed-evidence",
        "redacted report-key collisions",
        "malformed direct report statuses",
        "normalize non-string direct report keys",
        "redact non-finite direct report numbers",
        "normalize unsupported direct report values",
        "normalize malformed direct report error lists",
        "normalize malformed direct Kagemusha report sections",
        "require canonical device-family strings before matrix",
        "signed-evidence helper also rejects control-character slot",
        "signed-slot assembler source-copy preflight rejects control-character",
        "signed-slot assembler source digest preflights reject blank or noncanonical",
        "recorded proof",
        "commands with surrounding whitespace, control characters, or secret-looking",
        "echoing unsafe command bytes",
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
        "harness-result source path uses the shared guarded JSON",
        "paths fail before metadata reads or parsing",
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
        "Last updated: 2026-06-13",
        "ABI 6 recursive spend JNI probes pass on every required device family.",
        "ABI 7 recursive compact-token JNI probes prove and verify the packaged",
        "one-hop LEN=4 path on every required device family.",
        "Slot probe-state fields (`abi6_recursive_spend_jni_probe`,",
        "must be exactly `passed`; `ok` is not accepted as a production alias.",
        "must be exact lowercase strings with no",
        "reports `verification.status` as exact `ok`",
        "aliased or secret-looking harness-result source paths",
        "ABI 7 recursive compact prover calls that require multi-hop append-batch",
        "composition produce package-backed compact tokens when the key package is",
        "supplied, while empty, malformed, or dummy-proof local archives remain",
        "caller-input errors or soft-invalid verifier results",
        "Lab reports include raw test commands, device fingerprints, OS build IDs, and",
        ":offline-wallet-android:connectedDebugAndroidTest",
        ":offline-wallet-lab-app:assembleRelease",
        ":offline-wallet-lab-app:installRelease",
        ":offline-wallet-lab-app:installReleaseAndroidTest",
        "adb shell am instrument",
        "org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProverTest",
        "org.hyperledger.iroha.android.offline.OfflineNoteTransferHandoffTest",
        "org.hyperledger.iroha.android.offline.KagemushaDeviceLabArtifactExportTest",
        "kotlin/offline-wallet-lab-app",
        "kotlin/offline-wallet-android/src/androidTest/java/org/hyperledger/iroha/android/offline/",
        "kagemusha-device-lab/<slot-id>",
        "kagemusha-device-lab/latest-slot.txt",
        "python3 scripts/kagemusha_pull_android_device_lab_raw_slot.py",
        "--run-as-package org.hyperledger.iroha.sdk.offline.wallet.lab",
        "The puller rejects empty, surrounding-whitespace-normalized,",
        "control-character, or secret-looking ADB executable, serial, run-as package,",
        "response must be exactly one LF-terminated value",
        "on trimming surrounding whitespace",
        "rejects symlink, hardlink, special-file, traversal, duplicate,",
        "over-entry-limit, directory-colliding, unreviewed extra-artifact, and",
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
        "D2D and wallet transcript string fields must match slot metadata",
        "Telemetry",
        "must use the exact `kagemusha-device-lab`",
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
        "attestation/harness-result.json",
        "attestation/result.json",
        "attestation/report.json",
        "attestation verifier report",
        "python3 scripts/kagemusha_android_attestation_report.py",
        "--harness-result <android_keystore_attestation_result.json>",
        "--attestation-harness-result <harness-result.json>",
        "--physical-device-attestation --out <report.json>",
        "unexpected verifier-result fields",
        "explicit physical-device assertion",
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
    "kotlin/offline-wallet-android/src/androidTest/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProverTest.java": (
        "@RunWith(AndroidJUnit4.class)",
        "productionHarnessResolvesKagemushaRecursiveSpendSurface",
        "recursiveSpendWitnesslessPolicyFailsClosedAtBounds",
        "recursiveSpendKeyArtifactsRejectInvalidPackagesBeforeNativeDispatch",
        "recursiveCompactProjectionRejectsInvalidInputsBeforeNativeDispatch",
        "lineageKeyArtifactsForInit",
        "verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight",
        "18446744073709551615",
    ),
    "kotlin/offline-wallet-android/src/androidTest/java/org/hyperledger/iroha/android/offline/OfflineNoteTransferHandoffTest.java": (
        "@RunWith(AndroidJUnit4.class)",
        "productionHarnessResolvesOfflineNoteTransferHandoffFixture",
        "nearbyQrAndNfcTokenHandoffRoundTripFixtureBytes",
        "receiptAckHandoffRoundTripsFixtureRecipient",
        "InstrumentationRegistry.getInstrumentation()",
        "interop_contract.json",
        "OfflineNoteTransferCapabilities.current(false, true)",
        "OfflineNoteTransferHandoff.qrStreamingFrameBytes",
        "OfflineNoteTransferHandoff.nfcFrameBytes",
        "OfflineNoteReceiptAck.fromPaymentToken",
    ),
    "kotlin/offline-wallet-android/src/androidTest/java/org/hyperledger/iroha/android/offline/KagemushaDeviceLabArtifactExportTest.java": (
        "@RunWith(AndroidJUnit4.class)",
        "exportsKagemushaDeviceLabArtifactsFromPhysicalStrongBoxDevice",
        "kagemusha-device-lab",
        "latest-slot.txt",
        "KeyGenParameterSpec.Builder",
        "KeyProperties.KEY_ALGORITHM_EC",
        "setIsStrongBoxBacked(true)",
        "setAttestationChallenge(challenge)",
        "attestation/harness-result.json",
        "attestation/result.json",
        "attestation/keymint-certificate-chain.pem",
        "chain_length",
        "handoff/d2d-payment.json",
        "wallet/integrity.json",
        "sha256File(new File(context.getPackageCodePath()))",
        "getPackageCodePath",
        "kagemusha device-lab run complete",
        "strongbox_attestation",
        "physical_device_attestation",
    ),
    "kotlin/offline-wallet-lab-app/build.gradle.kts": (
        "alias(libs.plugins.android.application)",
        "applicationId = \"org.hyperledger.iroha.sdk.offline.wallet.lab\"",
        "testBuildType = \"release\"",
        "isDebuggable = true",
        "java.srcDir(\"../offline-wallet-android/src/androidTest/java\")",
        "implementation(project(\":offline-wallet-android\"))",
    ),
    "kotlin/settings.gradle.kts": (
        "include(\":offline-wallet-lab-app\")",
    ),
    "scripts/check_android_device_lab_slot.py": (
        "KAGEMUSHA_STANDARD_DEVICE_FAMILIES",
        "KAGEMUSHA_STANDARD_DEVICE_MINIMUM_OS",
        "DEVICE_LAB_ROOT_SUMMARY_LABEL",
        "\"root\": DEVICE_LAB_ROOT_SUMMARY_LABEL",
        "RAW_TEST_COMMAND_REQUIRED_MARKERS",
        'RAW_TEST_COMMAND_REQUIRED_MARKERS: tuple[str, ...] = (\n    ":client-android:assembleRelease",\n    ":offline-wallet-android:assembleRelease",\n    ":offline-wallet-android:connectedDebugAndroidTest",\n    ":offline-wallet-lab-app:assembleRelease",\n    ":offline-wallet-lab-app:installRelease",\n    ":offline-wallet-lab-app:installReleaseAndroidTest",\n    "adb shell am instrument",\n    "org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProverTest",\n    "org.hyperledger.iroha.android.offline.OfflineNoteTransferHandoffTest",\n    "org.hyperledger.iroha.android.offline.KagemushaDeviceLabArtifactExportTest",\n)',
        "KAGEMUSHA_ANDROID_PRODUCTION_RAW_TEST_COMMAND",
        "KAGEMUSHA_ANDROID_PRODUCTION_RAW_TEST_COMMANDS",
        "must exactly match the Kagemusha Android production raw test command",
        "org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProverTest",
        "org.hyperledger.iroha.android.offline.OfflineNoteTransferHandoffTest",
        "SIGNED_EVIDENCE_SCHEMA",
        "D2D_PAYMENT_TRANSCRIPT_SCHEMA",
        "D2D_PAYMENT_PAYLOAD_SCHEMA",
        "WALLET_INTEGRITY_TRANSCRIPT_SCHEMA",
        "d2d payment transcript {key} must not contain surrounding whitespace",
        "d2d payment transcript {key} must not contain control characters",
        "wallet integrity transcript {key} must not contain surrounding whitespace",
        "wallet integrity transcript {key} must not contain control characters",
        "ED25519_SIGNATURE_BYTES = 64",
        "REQUIRED_KAGEMUSHA_SLOT_ARTIFACT_PATHS",
        "KAGEMUSHA_SIGNED_EVIDENCE_ARTIFACT_PATH",
        "attestation/harness-result.json",
        "telemetry/status.ndjson",
        "logs/runtime.log",
        "MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES",
        "MAX_KAGEMUSHA_OFFLINE_WALLET_APK_BYTES",
        "KAGEMUSHA_OFFLINE_WALLET_APK_PATH",
        "def _slot_artifact_max_bytes",
        "MAX_ANDROID_DEVICE_LAB_JSON_BYTES",
        "MAX_ANDROID_DEVICE_LAB_SHA256_MANIFEST_BYTES",
        "validate_required_kagemusha_slot_artifact_shapes",
        "KAGEMUSHA_RUNTIME_LOG_COMPLETE_MARKER",
        "KAGEMUSHA_TELEMETRY_SUITE",
        "KAGEMUSHA_RUNTIME_LOG_FAILURE_MARKERS",
        "KAGEMUSHA_STATUS_FAILURE_VALUES",
        "PENDING_QUEUE_FIELDS",
        "_validate_required_pending_queue_artifact",
        "_validate_required_pending_queue_artifact(slot_path, errors)",
        "queue/pending_queue.json contains unexpected field",
        "queue/pending_queue.json slot_id must be a non-empty string",
        "queue/pending_queue.json slot_id must not contain surrounding whitespace",
        "queue/pending_queue.json slot_id must not contain control characters",
        "queue/pending_queue.json slot_id must match slot id",
        "queue/pending_queue.json pending_transactions must be an array",
        "queue/pending_queue.json pending_transactions must be empty after D2D handoff",
        "TELEMETRY_FIELDS",
        "TELEMETRY_STRING_FIELDS",
        "telemetry/telemetry.json contains unexpected field",
        "_validate_telemetry_string",
        "_validate_required_telemetry_artifact",
        "expected_app_package_name",
        "expected_app_package_label",
        "expected_device_model",
        "expected_device_codename",
        "telemetry/telemetry.json app_package_name must match",
        "telemetry/telemetry.json {key} must match slot.json {key}",
        "slot_id != slot_path.name",
        "telemetry/telemetry.json slot_id must be a non-empty string",
        "telemetry/telemetry.json slot_id must not contain surrounding whitespace",
        "telemetry/telemetry.json slot_id must not contain control characters",
        "telemetry/telemetry.json suite must be a non-empty string",
        "telemetry/telemetry.json suite must not contain surrounding whitespace",
        "telemetry/telemetry.json suite must not contain control characters",
        "suite != KAGEMUSHA_TELEMETRY_SUITE",
        'label = f"telemetry/telemetry.json {key}"',
        "f\"{label} must be a non-empty string\"",
        "f\"{label} must not contain surrounding whitespace\"",
        "f\"{label} must not contain control characters\"",
        "f\"{label} must not contain secret-looking material\"",
        "slot.json {key} must be a non-empty string",
        "slot.json {key} must not contain surrounding whitespace",
        "slot.json {key} must not contain control characters",
        "slot.json {key} must be lowercase",
        "STATUS_EVENT_FIELDS",
        "_validate_required_status_artifact",
        "_validate_required_runtime_log_artifact",
        "kagemusha device-lab run complete",
        "telemetry/status.ndjson must use LF line endings",
        "telemetry/status.ndjson must end with a trailing newline",
        "telemetry/status.ndjson line {line_no} contains unexpected field",
        "telemetry/status.ndjson line {line_no} must not contain surrounding whitespace",
        "telemetry/status.ndjson line {line_no} status must not contain surrounding whitespace",
        "telemetry/status.ndjson line {line_no} status must not contain control characters",
        "telemetry/status.ndjson line {line_no} status must be lowercase",
        "telemetry/status.ndjson line {line_no} status must be ok",
        "telemetry/status.ndjson line {line_no} slot_id must be a non-empty string",
        "telemetry/status.ndjson line {line_no} slot_id must be a string",
        "telemetry/status.ndjson line {line_no} slot_id must not contain surrounding whitespace",
        "telemetry/status.ndjson line {line_no} slot_id must not contain control characters",
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
        "KAGEMUSHA_DEVICE_FAMILY_MODEL_RULES",
        "infer_kagemusha_device_family",
        "_match_kagemusha_device_model_family",
        "_match_kagemusha_device_codename_family",
        "model_family is None or codename_family is None",
        "model_family != codename_family",
        '"dm1q", "dm2q", "dm3q"',
        '"e1q", "e2q", "e3q"',
        "slot.json device_family must match device_model/device_codename",
        "slot.json device_model/device_codename must identify a standard Kagemusha family",
        "SIGNED_EVIDENCE_SLOT_SHA256_FIELDS: tuple[str, ...]",
        "SIGNED_EVIDENCE_SLOT_INT_FIELDS: tuple[str, ...]",
        "SIGNED_EVIDENCE_SLOT_TRUE_FIELDS: tuple[str, ...]",
        "SLOT_METADATA_FIELDS",
        "ATTESTATION_RESULT_FIELDS",
        "ATTESTATION_REPORT_SCHEMA",
        "ATTESTATION_REPORT_FIELDS",
        "ATTESTATION_REPORT_VERIFICATION_FIELDS",
        "ATTESTATION_HARNESS_RESULT_FIELDS",
        "validate_attestation_report",
        "validate_attestation_report(slot_path, metadata, errors)",
        "validate_attestation_report_result_level_binding",
        "validate_attestation_report_result_level_binding(",
        'for level_key in (\n        "keymint_security_level",\n        "attestation_security_level",\n        "keymaster_security_level",\n    ):\n        value = _attestation_report_verification_string(verification, level_key, errors)',
        'if status is not None and status != "ok":',
        "attestation/result.json status must be ok",
        "attestation/report.json verification.status must be ok",
        "and result_status != report_status",
        "attestation/report.json verification.status must match",
        "and result_level != report_level",
        "attestation/report.json verification.{level_key} must be STRONGBOX",
        "attestation/report.json verification.{level_key} must match",
        "validate_attestation_harness_result",
        "validate_attestation_harness_result(",
        "security_level is not None and security_level not in STRONGBOX_LEVELS",
        "slot.json keymint_security_level must be STRONGBOX",
        "attestation/result.json keymint_security_level must match",
        '_require_status(metadata, "abi6_recursive_spend_jni_probe", {"passed"}, errors)',
        "attestation/harness-result.json {key} must not have surrounding whitespace",
        "attestation/harness-result.json {key} must not contain control characters",
        "if level is not None and level not in STRONGBOX_LEVELS:",
        "attestation/harness-result.json challenge_hex must be lowercase hexadecimal without whitespace",
        "challenge_hex != challenge_hex.lower()",
        "attestation/harness-result.json challenge_hex digest must match slot.json attestation_challenge_sha256",
        "attestation/harness-result.json chain_length must match",
        "set(result) - ATTESTATION_RESULT_FIELDS",
        "SECRET_PATH_REDACTION",
        "_summary_safe_string",
        "_summary_safe_value",
        "_summary_safe_report",
        "SUMMARY_REDACTION_KEY_COLLISION_FIELD",
        "SUMMARY_STATUS_NORMALIZED_FIELD",
        "SUMMARY_NON_STRING_KEY_NORMALIZED_FIELD",
        "SUMMARY_NON_STRING_KEY_REDACTION",
        "SUMMARY_NONFINITE_NUMBER_NORMALIZED_FIELD",
        "SUMMARY_NONFINITE_NUMBER_REDACTION",
        "SUMMARY_UNSUPPORTED_VALUE_NORMALIZED_FIELD",
        "SUMMARY_UNSUPPORTED_VALUE_REDACTION",
        "SUMMARY_KAGEMUSHA_SHAPE_NORMALIZED_FIELD",
        "SUMMARY_ERRORS_NORMALIZED_FIELD",
        "SUMMARY_ERROR_REDACTION",
        "def _summary_kagemusha(report: dict) -> dict[str, Any]:",
        "def _summary_device_family(report: dict) -> str | None:",
        "def _summary_safe_errors(value: Any) -> tuple[list[str], bool]:",
        "import math",
        "def _summary_safe_value(value: Any) -> tuple[Any, bool, bool, bool, bool]:",
        "math.isfinite(value)",
        "return SUMMARY_UNSUPPORTED_VALUE_REDACTION, False, False, False, True",
        "if safe_key in safe:",
        'summary_report["status"] = "error"',
        'summary_report["errors"] = errors',
        "summary_report[SUMMARY_NON_STRING_KEY_NORMALIZED_FIELD] = True",
        "summary_report[SUMMARY_NONFINITE_NUMBER_NORMALIZED_FIELD] = True",
        "summary_report[SUMMARY_UNSUPPORTED_VALUE_NORMALIZED_FIELD] = True",
        "summary_report[SUMMARY_KAGEMUSHA_SHAPE_NORMALIZED_FIELD] = True",
        "summary_report[SUMMARY_ERRORS_NORMALIZED_FIELD] = True",
        "summary_report[SUMMARY_REDACTION_KEY_COLLISION_FIELD] = True",
        "SHA256_HEX_RE.fullmatch(value)",
        "SHA256_HEX_RE.fullmatch(digest)",
        "unsafe path contains secret-looking material",
        "unsafe path contains control characters",
        "unsafe path contains surrounding whitespace",
        "--require-kagemusha-production-evidence",
        "--require-kagemusha-standard-matrix",
        "--trusted-signer-public-key",
        "--root must not contain secret-looking material",
        "--json-out must not contain secret-looking material",
        'for index, key_path in enumerate(args.trusted_signer_public_keys or []):',
        'label = f"--trusted-signer-public-key[{index}]"',
        'path_arg_errors.append(f"{label} must not contain control characters")',
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
        "slot_ids, slot_id_errors = validate_slot_ids(args.slots)\n    if slot_id_errors:\n        for error in slot_id_errors:\n            print(f\"[device-lab] {error}\", file=sys.stderr)\n        return 1\n\n    root = Path(args.root)",
        "root_exists, root_errors = classify_device_lab_root_path(root)",
        "if not root_exists:",
        '    try:\n        root_mode = root.lstat().st_mode\n    except FileNotFoundError:\n        root_mode = None\n    except OSError:\n        return False, ["device-lab root metadata could not be read"]\n',
        "root_mode is not None and stat.S_ISLNK(root_mode)",
        "root_mode is not None and not stat.S_ISDIR(root_mode)",
        "validate_no_symlink_ancestors",
        "slot_ids, slot_id_errors = validate_slot_ids(args.slots)",
        "slot_paths, discovery_errors = discover_slots(root, slot_ids)",
        "validated_slot_ids, slot_id_errors = validate_slot_ids(slot_ids)",
        "return [], slot_id_errors",
        "device-lab root could not be listed",
        "entries = sorted(root.iterdir(), key=lambda entry: entry.name)",
        "entry_mode = entry.lstat().st_mode",
        "device-lab slot directory metadata could not be read",
        "if stat.S_ISDIR(entry_mode) or stat.S_ISLNK(entry_mode):",
        "validate_summary_output_path",
        "--root must not contain control characters",
        "--json-out must not contain control characters",
        'f"{label} path must not contain control characters"',
        '    parent_exists, parent_errors = _validate_summary_output_parent(path, label)\n    if parent_errors:\n        return parent_errors\n    ancestor_errors = validate_no_symlink_ancestors(\n',
        '        f"{label} ancestor directory",\n    )\n    if ancestor_errors:\n        return ancestor_errors\n    if not parent_exists:\n',
        '    parent_exists, parent_errors = _validate_summary_output_parent(\n        path,\n        label,\n        missing_error=f"{label} parent must be a directory",\n    )\n    if parent_errors:\n        return parent_errors\n    if not parent_exists:\n        return [f"{label} parent must be a directory"]\n',
        'def _validate_summary_output_parent(\n    path: Path,\n    label: str,\n    *,\n    missing_error: str | None = None,\n) -> tuple[bool, list[str]]:\n    """Classify a scanner summary output parent without following aliases."""\n\n    parent = path.parent\n    try:\n        parent_mode = parent.lstat().st_mode\n    except FileNotFoundError:\n        if missing_error is None:\n            return False, []\n        return False, [missing_error]\n    except OSError:\n        return False, [f"{label} parent directory metadata could not be read"]\n',
        '    if stat.S_ISLNK(parent_mode):\n        return True, [f"{label} parent directory must not be a symlink"]\n    if not stat.S_ISDIR(parent_mode):\n        return True, [f"{label} parent must be a directory"]\n    return True, []\n',
        "_slot_tree_entries",
        "pending = [dir_path]",
        "scanned = sorted(os.scandir(current), key=lambda entry: entry.name)",
        "entry.stat(follow_symlinks=False)",
        'f"{label} could not be listed"',
        "slot id {_display_path(slot_id)!r} must be a single safe directory name",
        "slot id {_display_path(slot_id)!r} must be a canonical single directory name",
        "unsafe path is not canonical",
        "slot directory name must not contain backslashes",
        "slot id {index} must not contain whitespace",
        "must not duplicate slot id",
        'if SECRET_RE.search(root_text):\n        return False, ["device-lab root path must not contain secret-looking material"]',
        'if _contains_control_character(root_text):\n        return False, ["device-lab root path must not contain control characters"]',
        'if "\\\\" in root_text:\n        return False, ["device-lab root path must not contain backslashes"]',
        'if ".." in root.parts:\n        return False, ["device-lab root path must be canonical"]',
        "DuplicateJsonKeyError",
        "NonFiniteJsonConstantError",
        "_reject_nonfinite_json_constant",
        "_reject_duplicate_json_object_pairs",
        "object_pairs_hook=_reject_duplicate_json_object_pairs",
        "parse_constant=_reject_nonfinite_json_constant",
        "contains duplicate JSON object key",
        "contains non-finite constant",
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
        "slot directory name must not contain whitespace",
        "slot directory name must not contain control characters",
        "slot id {index} must not contain control characters",
        "slot.json {key} must not contain surrounding whitespace",
        "slot.json {key} must not contain control characters",
        "signed evidence artifact {key} must not contain surrounding whitespace",
        "signed evidence artifact {key} must not contain control characters",
        "attestation/result.json {key} must not contain surrounding whitespace",
        "attestation/result.json {key} must not contain control characters",
        "attestation/report.json {key} must not contain surrounding whitespace",
        "attestation/report.json {key} must not contain control characters",
        "attestation/report.json verification.{key} must not contain surrounding whitespace",
        "attestation/report.json verification.{key} must not contain control characters",
        "def _slot_path_boundary_errors(slot_path: Path) -> list[str]:",
        "path_text = str(slot_path)",
        "slot path must not contain secret-looking material",
        "_reject_secret_slot_path(slot_path, errors)",
        "slot path must not contain control characters",
        "slot path must not contain backslashes",
        "slot path must be canonical",
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
        'def _validate_manifest_slot_path(slot_path: Path) -> list[str]:\n    path_errors = _slot_path_boundary_errors(slot_path)\n    if path_errors:\n        return path_errors\n',
        'root_errors = _validate_manifest_slot_path(slot_path)\n    if root_errors:\n        return entries, root_errors\n',
        'root_errors = _validate_manifest_slot_path(slot_path)\n    if root_errors:\n        return root_errors\n',
        '        try:\n            candidate = Path.cwd() / path\n        except OSError:\n            return [f"{label} metadata could not be read"]\n',
        "ancestor_mode = ancestor.lstat().st_mode",
        '        except OSError:\n            errors.append(f"{label} metadata could not be read")\n            break\n',
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
        "open_stat.st_size > MAX_ANDROID_DEVICE_LAB_SHA256_MANIFEST_BYTES",
        "size > MAX_ANDROID_DEVICE_LAB_SHA256_MANIFEST_BYTES",
        'lines = b"".join(chunks).decode("utf-8").splitlines()',
        "sha256sum.txt line {line_no}: must not contain surrounding whitespace",
        'or path_text.startswith("*")',
        '    except (OSError, UnicodeDecodeError):\n        return entries, ["sha256sum.txt could not be read"]\n',
        "sha256sum.txt could not be read",
        "def _has_manifest_file_shape_error(errors: list[str]) -> bool:",
        "if _has_manifest_file_shape_error(errors):",
        'def _slot_root_entries(slot_path: Path, errors: list[str]) -> list[Path] | None:\n    try:\n        return sorted(slot_path.iterdir(), key=lambda entry: entry.name)\n    except OSError:\n        _append_error_once(errors, "slot directory could not be listed")\n        return None\n',
        "def _record_manifest_inventory_entry(",
        '    try:\n        mode = entry.lstat().st_mode\n    except OSError:\n        _append_error_once(\n            errors,\n            f"slot artifact {_display_path(relative)} file metadata could not be read",\n        )\n        return\n    if stat.S_ISREG(mode) or stat.S_ISLNK(mode):\n        files.add(relative)\n',
        'def _slot_files(slot_path: Path, errors: list[str] | None = None) -> set[str]:\n    slot_errors = errors if errors is not None else []\n    path_errors = _slot_path_boundary_errors(slot_path)\n    if path_errors:\n        slot_errors.extend(path_errors)\n        return set()\n    try:\n        slot_mode = slot_path.lstat().st_mode\n',
        "slot directory could not be listed",
        "path_errors = _slot_path_boundary_errors(slot_path)",
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
        "open_stat.st_size > max_bytes",
        "size > max_bytes",
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
        "            if open_stat.st_nlink > 1:\n                return None, [verification_error]\n",
        "readback, readback_errors = _read_staged_bytes(",
        'def _verify_ed25519_signature(\n    *,\n    public_key_path: Path,\n    payload: bytes,\n    signature: bytes,\n    errors: list[str],\n    label: str = "trusted signer public key",\n) -> None:\n    if not _validate_public_key_path_shape(public_key_path, errors=errors, label=label):\n        return\n    openssl = _require_openssl(errors)\n',
        "signature verification staging files could not be written",
        "signature verification staged payload did not match input",
        "signature verification staged signature did not match input",
        "signature verification temporary directory could not be created",
        "covered_device_families",
        "missing_device_families",
        "trusted_signer_public_key_sha256",
        "KAGEMUSHA_SUMMARY_RELEASE_ARTIFACTS",
        "KAGEMUSHA_SUMMARY_RELEASE_SHA256_FIELDS",
        "KAGEMUSHA_SUMMARY_RELEASE_REDACTED_SLOT_IDS",
        "def _summary_release_kagemusha(",
        "def _summary_release_device_family(",
        "require_complete_signed_evidence=require_complete_kagemusha",
        "require_complete_signed_evidence=True",
        "trusted_signer_public_key_sha256 is not None",
        "signer_public_key_sha256 not in trusted_signer_public_key_sha256",
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
        'f"{label} references artifact {display} must be no more than "',
        "    except OSError:\n        return None, [unreadable_error]\n",
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
        "signed evidence artifact signature payload is not strict JSON",
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
        'signed evidence artifact digest references artifact "\n                    f"{display} must be no more than "',
        "f\"{MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES} bytes\"",
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
        ":offline-wallet-android:connectedDebugAndroidTest",
        ":offline-wallet-lab-app:installRelease",
        ":offline-wallet-lab-app:installReleaseAndroidTest",
        "adb shell am instrument",
        "signed evidence artifact signed_at_utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
        "_require_evidence_raw_string",
        '_require_evidence_raw_string(evidence, "signed_at_utc", errors)',
        "slot.json raw_test_commands",
        "_validate_public_key_path_shape",
        "from collections.abc import Mapping",
        "def _trusted_signer_public_key_sha256_set(",
        "def validate_trusted_signer_public_key_map(",
        "def _valid_trusted_signer_public_key_sha256(",
        'def _valid_trusted_signer_public_key_sha256(value: Any) -> bool:\n'
        '    return (\n'
        '        isinstance(value, str)\n'
        '        and SHA256_HEX_RE.fullmatch(value) is not None\n'
        '        and value != "0" * 64\n'
        '    )\n',
        "trusted signer public key digest must be non-zero lowercase sha256 hex",
        "trusted signer public key map must be a mapping",
        "if not isinstance(public_key_path, Path):",
        "trusted signer public key path must be a pathlib Path",
        "def _trusted_signer_digest_sort_key(",
        "key=_trusted_signer_digest_sort_key",
        "def kagemusha_duplicate_matrix_bindings(",
        '                or value == "0" * 64\n',
        "trusted signer public key path must not contain control characters",
        "trusted signer public key path must not contain backslashes",
        "trusted signer public key path must be canonical",
        "signer_map_errors = validate_trusted_signer_public_key_map(",
        "if signer_map_errors:\n        return signer_map_errors, details",
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
        "_cleanup_summary_output",
        "--json-out temporary file could not be removed",
        "tmp_identity = _file_identity(os.fstat(handle.fileno()))",
        "write_errors.extend(_cleanup_summary_output(tmp_path, tmp_identity))",
        "--json-out temporary file changed before cleanup",
        "os.stat(\n                path.name,\n                dir_fd=parent_fd,\n                follow_symlinks=False",
        "_file_identity(temp_stat) != expected_identity",
        "os.unlink(path.name, dir_fd=parent_fd)",
        "--json-out parent directory could not be synced",
        "parent directory changed before sync",
        "def _file_identity",
        "def _directory_open_flags",
        "O_NOFOLLOW",
        "def _sync_summary_output_parent",
        "os.fstat(parent_fd)",
        "expected_identity=parent_identity",
        "_read_summary_output_text",
        "summary_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        "summary_open_identity = (open_stat.st_dev, open_stat.st_ino)",
        "--json-out changed while being read",
        '            if open_stat.st_size > MAX_ANDROID_DEVICE_LAB_JSON_BYTES:\n                return None, [\n                    "--json-out must be no more than "\n                    f"{MAX_ANDROID_DEVICE_LAB_JSON_BYTES} bytes"\n                ]\n',
        '                if size > MAX_ANDROID_DEVICE_LAB_JSON_BYTES:\n                    return None, [\n                        "--json-out must be no more than "\n                        f"{MAX_ANDROID_DEVICE_LAB_JSON_BYTES} bytes"\n                    ]\n',
        "readback_text, readback_errors = _read_summary_output_text(path, expected_stat)",
        "readback_text != summary_text",
        "--json-out write verification failed",
        "json.dumps(summary, indent=2, allow_nan=False)",
        "--json-out summary is not strict JSON",
        "len(summary_text.encode(\"utf-8\")) > MAX_ANDROID_DEVICE_LAB_JSON_BYTES",
        '    except OSError:\n        return None, ["--json-out write verification failed"]\n',
        '    errors = validate_summary_output_path(path, "--json-out")\n    if errors:\n        return errors\n    try:\n        expected_stat = path.lstat()\n',
        '    try:\n        output_mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return []\n    except OSError:\n        return [f"{label} file metadata could not be read"]\n',
        "if stat.S_ISLNK(output_mode):",
        "if not stat.S_ISREG(output_mode):",
        '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        return [f"{label} hardlink metadata could not be read"]\n',
        'if SECRET_RE.search(path_text):\n        return [f"{label} must not contain secret-looking material"]',
        'if _contains_control_character(path_text):\n        return [f"{label} must not contain control characters"]',
        'if "\\\\" in path_text:\n        return [f"{label} must not contain backslashes"]',
        'if ".." in path.parts:\n        return [f"{label} must be canonical"]',
        'return [f"{label} must not be a symlink"]',
        'return [f"{label} must not be hardlinked"]',
        'def _load_json(path: Path, label: str, errors: list[str]) -> dict[str, Any] | None:\n    path_text = str(path)\n    if SECRET_RE.search(path_text):\n        errors.append(f"{label} path must not contain secret-looking material")\n        return None\n',
        'if _contains_control_character(path_text):\n        errors.append(f"{label} path must not contain control characters")\n        return None',
        'if "\\\\" in path_text:\n        errors.append(f"{label} path must not contain backslashes")\n        return None',
        'if ".." in path.parts:\n        errors.append(f"{label} path must be canonical")\n        return None',
        "json_ancestor_errors = validate_no_symlink_ancestors(",
        'f"{label} ancestor directory"',
        '    try:\n        expected_stat = path.lstat()\n    except FileNotFoundError:\n        errors.append(f"missing {label}")\n        return None\n    except OSError:\n        errors.append(f"{label} file metadata could not be read")\n        return None\n',
        "json_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        "json_open_identity = (open_stat.st_dev, open_stat.st_ino)",
        "json_path_stat = path.lstat()",
        "open_stat.st_size > MAX_ANDROID_DEVICE_LAB_JSON_BYTES",
        "size > MAX_ANDROID_DEVICE_LAB_JSON_BYTES",
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
        "device_lab.validate_attestation_report(slot_path, metadata, errors)",
        "device_lab.SIGNED_EVIDENCE_SLOT_INT_FIELDS",
        "slot.json native_bridge_abi_version must be an integer",
        "private key did not produce a signature accepted by the signer public key",
        "signed evidence payload is not strict JSON",
        "_secret_key_path_error",
        "if device_lab.SECRET_RE.search(path_text):",
        "path must not contain secret-looking material",
        "path must not contain control characters",
        'return f"{label} path must not contain backslashes"',
        'return f"{label} path must be canonical"',
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
        "            if open_stat.st_nlink > 1:\n                errors.append(\"signature output could not be read\")\n                return None\n",
        "read_limit = device_lab.ED25519_SIGNATURE_BYTES + 1",
        "chunk = handle.read(read_limit - size)",
        "signature output must be 64 bytes",
        "len(signature) != device_lab.ED25519_SIGNATURE_BYTES",
        'if verify_errors == ["signed evidence artifact signature verification failed"]:\n        errors.append(\n            "private key did not produce a signature accepted by the signer public key"\n        )\n    elif verify_errors:\n        errors.extend(verify_errors)\n',
        "private key must not be a symlink",
        "private key ancestor directory",
        '    try:\n        link_count = private_key_path.stat().st_nlink\n    except OSError:\n        errors.append("private key hardlink metadata could not be read")\n        return None\n',
        "private key must not be hardlinked",
        '    if private_key_mode is None:\n        errors.append("private key must point to an existing file")\n        return None\n    if not stat.S_ISREG(private_key_mode):\n        errors.append("private key must be a regular file")\n        return None\n',
        "signed evidence output path must not contain secret-looking material",
        "signed evidence output path must not contain control characters",
        "signed evidence output path must not contain backslashes",
        "signed evidence output path must be canonical",
        "signer key id must be non-empty and must not contain secret-looking material",
        'device_lab.validate_no_symlink_ancestors(\n            candidate,\n            "signed evidence output path ancestor directory",\n        )',
        "signed evidence output path file metadata could not be read",
        "candidate_resolved = candidate.resolve()",
        "slot_resolved = slot_path.resolve()",
        "signed evidence output path could not be resolved",
        '        except OSError:\n            errors.append("signed evidence output path could not be resolved")\n            return None\n',
        "signed evidence output path must stay under evidence/",
        "signed evidence output path must be",
        "_validate_json_output_path",
        "json.dumps(payload, indent=2, sort_keys=True, allow_nan=False)",
        'return [f"{label} is not strict JSON"]',
        'if len(text.encode("utf-8")) > device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES:',
        'f"{label} must be no more than "',
        'def _validate_json_output_path(path: Path, label: str) -> list[str]:\n    """Validate a signer-controlled output immediately before writing."""\n\n    path_text = str(path)\n    if device_lab.SECRET_RE.search(path_text):\n        return [f"{label} must not contain secret-looking material"]\n',
        'if "\\\\" in path_text:\n        return [f"{label} must not contain backslashes"]',
        'if ".." in path.parts:\n        return [f"{label} must be canonical"]',
        '    parent_exists, parent_errors = _validate_json_output_parent(path, label)\n    errors.extend(parent_errors)\n    if errors:\n        return errors\n',
        '    errors.extend(\n        device_lab.validate_no_symlink_ancestors(\n            path,\n            f"{label} ancestor directory",\n        )\n    )\n    if errors:\n        return errors\n    if not parent_exists:\n',
        '    if not parent_exists:\n        try:\n            parent.mkdir(mode=0o700, parents=True, exist_ok=True)\n        except OSError:\n            errors.append(f"{label} parent directory could not be created")\n',
        '    parent_exists, parent_errors = _validate_json_output_parent(\n        path,\n        label,\n        missing_error=f"{label} parent must be a directory",\n    )\n    errors.extend(parent_errors)\n    if not parent_exists and not errors:\n        errors.append(f"{label} parent must be a directory")\n',
        "def _set_private_directory_permissions",
        "os.fchmod(dir_fd, 0o700)",
        "stat.S_IMODE(directory_stat.st_mode) != 0o700",
        "os.fchmod(handle.fileno(), 0o600)",
        'if stat.S_IMODE(mode) != 0o600:',
        '        try:\n            parent.mkdir(mode=0o700, parents=True, exist_ok=True)\n        except OSError:\n            errors.append(f"{label} parent directory could not be created")\n',
        'def _validate_json_output_parent(\n    path: Path,\n    label: str,\n    *,\n    missing_error: str | None = None,\n) -> tuple[bool, list[str]]:\n    """Classify a signer-controlled output parent without following aliases."""\n\n    parent = path.parent\n    try:\n        parent_mode = parent.lstat().st_mode\n    except FileNotFoundError:\n        if missing_error is None:\n            return False, []\n        return False, [missing_error]\n    except OSError:\n        return False, [f"{label} parent directory metadata could not be read"]\n',
        '    if stat.S_ISLNK(parent_mode):\n        return True, [f"{label} parent directory must not be a symlink"]\n    if not stat.S_ISDIR(parent_mode):\n        return True, [f"{label} parent must be a directory"]\n    return True, []\n',
        '    try:\n        mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return errors\n    except OSError:\n        errors.append(f"{label} file metadata could not be read")\n        return errors\n    if stat.S_ISLNK(mode):\n        errors.append(f"{label} must not be a symlink")\n',
        '        try:\n            link_count = path.stat().st_nlink\n        except OSError:\n            errors.append(f"{label} hardlink metadata could not be read")\n        else:\n            if link_count > 1:\n                errors.append(f"{label} must not be hardlinked")\n',
        "_validate_existing_json_output_path",
        'def _validate_existing_json_output_path(path: Path, label: str) -> list[str]:\n    """Validate a signer-controlled output immediately before reading it back."""\n\n    path_text = str(path)\n    if device_lab.SECRET_RE.search(path_text):\n        return [f"{label} must not contain secret-looking material"]\n',
        '    _, parent_errors = _validate_json_output_parent(\n        path,\n        label,\n        missing_error=f"{label} parent directory is missing",\n    )\n    if parent_errors:\n        return parent_errors\n',
        '    try:\n        mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return [f"{label} must exist before digest"]\n    except OSError:\n        return [f"{label} file metadata could not be read"]\n    if stat.S_ISLNK(mode):\n        return [f"{label} must not be a symlink"]\n',
        '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        return [f"{label} hardlink metadata could not be read"]\n    if link_count > 1:\n        return [f"{label} must not be hardlinked"]\n',
        "_output_file_sha256",
        'errors = _validate_existing_json_output_path(path, label)\n    if errors:\n        return None, errors\n',
        "_read_existing_output_bytes",
        "max_bytes: int | None = None",
        "byte_limit = (",
        "payload, read_errors = _read_existing_output_bytes(",
        'with path.open("rb") as handle:',
        "signer_output_expected_identity = (",
        "signer_output_expected_identity = (\n                expected_stat.st_dev,\n                expected_stat.st_ino,\n            )",
        "signer_output_open_identity = (open_stat.st_dev, open_stat.st_ino)",
        'return None, [f"{label} changed while being read"]',
        "open_stat.st_size > byte_limit",
        "size > byte_limit",
        'except OSError:\n        return None, [f"{label} could not be read"]',
        'artifact_digest, digest_errors = _output_file_sha256(\n        output_path,\n        "signed evidence output path",\n    )',
        "_write_json(output_path, evidence, \"signed evidence output path\")",
        "_write_text",
        'slot_path / "sha256sum.txt"',
        'manifest_text = "\\n".join(lines) + "\\n"',
        "max_bytes=device_lab.MAX_ANDROID_DEVICE_LAB_SHA256_MANIFEST_BYTES",
        "_write_text_atomic",
        'if len(text.encode("utf-8")) > byte_limit:',
        "tempfile.NamedTemporaryFile(",
        "handle.flush()",
        "os.fsync(handle.fileno())",
        "os.replace(tmp_path, path)",
        "_cleanup_temp_output",
        'f"{label} temporary file could not be removed"',
        "tmp_identity = _file_identity(os.fstat(handle.fileno()))",
        "write_errors.extend(_cleanup_temp_output(tmp_path, label, tmp_identity))",
        'f"{label} temporary file changed before cleanup"',
        "_file_identity(temp_stat) != expected_identity",
        "os.unlink(path.name, dir_fd=parent_fd)",
        'f"{label} parent directory could not be synced"',
        "def _file_identity",
        "def _directory_open_flags",
        "O_NOFOLLOW",
        "def _sync_output_parent",
        "expected_identity=parent_identity",
        "parent directory changed before sync",
        "_read_existing_output_text",
        '        if read_errors == [f"{label} could not be read"]:\n            return None, [f"{label} write verification failed"]\n',
        "readback_text != text",
        "write verification failed",
        '    errors = _validate_existing_json_output_path(path, label)\n    if errors:\n        return errors\n    try:\n        expected_stat = path.lstat()\n',
        '    readback_text, readback_errors = _read_existing_output_text(\n        path,\n        expected_stat,\n        label,\n        max_bytes=max_bytes,\n    )\n    if readback_errors:\n        return readback_errors\n    if readback_text != text:',
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
        "artifact_max_bytes =",
        "open_stat.st_size > artifact_max_bytes",
        "size > artifact_max_bytes",
        '    try:\n        artifact_stat = artifact_path.lstat()\n    except FileNotFoundError:\n        return None, None, [f"slot artifact {display} is missing"]\n    except OSError:\n        return None, None, [f"slot artifact {display} file metadata could not be read"]\n    if stat.S_ISLNK(artifact_stat.st_mode):\n        return None, None, [f"slot artifact {display} must not be a symlink"]\n',
        '    try:\n        link_count = artifact_path.stat().st_nlink\n    except OSError:\n        return None, None, [\n            f"slot artifact {display} hardlink metadata could not be read"\n        ]\n',
        "slot artifact {display} could not be read",
        'artifact_path, artifact_stat, errors = _validate_slot_artifact_for_digest(\n        slot_path,\n        relative,\n    )',
        "digest, digest_errors = _slot_artifact_sha256(slot_path, relative)",
        "validate_no_slot_symlink_artifacts",
        "validate_slot_regular_file_artifacts",
        "validate_no_slot_hardlink_artifacts",
        'def _validate_slot_path_boundary(slot_path: Path) -> list[str]:\n    """Validate signer slot paths before reading mutable slot artifacts."""\n\n    path_errors = device_lab._slot_path_boundary_errors(slot_path)',
        "device_lab._slot_path_boundary_errors(slot_path)",
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
    "scripts/kagemusha_android_device_lab_slot.py": (
        "Assemble a signed Kagemusha Android device-lab slot from lab artifacts",
        "DEFAULT_ATTESTATION_HARNESS_RESULT_PATH",
        "DEFAULT_ATTESTATION_CHAIN_PATH",
        "DEFAULT_OFFLINE_WALLET_APK_PATH",
        "DEFAULT_D2D_TRANSCRIPT_PATH",
        "DEFAULT_WALLET_TRANSCRIPT_PATH",
        "DEVICE_FAMILY_MODEL_RULES",
        "exact_models, _codenames, model_prefixes",
        "_exact_models, codenames, _model_prefixes",
        "_match_device_model_family",
        "_match_device_codename_family",
        "model_family is None or codename_family is None",
        "model_family != codename_family",
        '"dm1q", "dm2q", "dm3q"',
        '"e1q", "e2q", "e3q"',
        "model_text.startswith(prefix)",
        "has_device_identity = bool",
        "if has_device_identity and inferred != family:",
        "device family must match attached device model/codename",
        '"device_model": facts["device_model"]',
        '"device_codename": facts["device_codename"]',
        "expected_device_model=facts[\"device_model\"]",
        "expected_device_codename=facts[\"device_codename\"]",
        "device_lab.validate_no_symlink_ancestors(",
        "def _set_private_directory_permissions",
        "os.fchmod(dir_fd, 0o700)",
        "stat.S_IMODE(directory_stat.st_mode) != 0o700",
        'f"{label} path must not contain control characters"',
        'f"{label} path must not contain backslashes"',
        'f"{label} path must be canonical"',
        "expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        "open_identity = (open_stat.st_dev, open_stat.st_ino)",
        "open_identity != expected_identity or path_identity != expected_identity",
        "changed while being read",
        "destination_parent_identity = _file_identity(destination_parent_stat)",
        "expected_identity=destination_parent_identity",
        "def _verify_copied_file(",
        "verify_errors = _verify_copied_file(",
        "if verify_errors:",
        "def _write_json(",
        "path.parent.mkdir(mode=0o700",
        "def _cleanup_temp_output(",
        "tmp_identity = _file_identity(os.fstat(handle.fileno()))",
        "os.fchmod(handle.fileno(), 0o600)",
        "_file_identity(temp_stat) != expected_identity",
        "os.unlink(path.name, dir_fd=parent_fd)",
        "temporary output changed before cleanup",
        "cleanup_errors = _cleanup_temp_output(tmp_path, label, tmp_identity)",
        "expected_identity=json_parent_identity",
        "parent directory changed before sync",
        "def _verify_written_bytes(",
        "return _verify_written_bytes(path, encoded, label)",
        "changed after write",
        "permissions must be 0600",
        "def _single_safe_slot_id(slot_id: str) -> str | None:",
        "candidate.as_posix() != slot_id",
        'or "\\\\" in slot_id',
        'if device_lab._contains_control_character(root_text):\n        return 1, None, ["device-lab root path must not contain control characters"]',
        'if "\\\\" in root_text:\n        return 1, None, ["device-lab root path must not contain backslashes"]',
        'if ".." in root.parts:\n        return 1, None, ["device-lab root path must be canonical"]',
        "device_lab.classify_device_lab_root_path(root)",
        "def _publish_stage_slot(",
        "_file_identity(root_stat) != expected_root_identity",
        "_file_identity(temp_parent_stat) != expected_temp_parent_identity",
        "_file_identity(stage_stat) != expected_stage_identity",
        "src_dir_fd=temp_parent_fd",
        "dst_dir_fd=root_fd",
        "os.fsync(root_fd)",
        "root.mkdir(mode=0o700)",
        "stage_slot.mkdir(mode=0o700)",
        "destination.parent.mkdir(mode=0o700",
        "os.fchmod(out.fileno(), 0o600)",
        "slot root directory changed before publish",
        "staged slot directory changed before publish",
        "def _cleanup_temp_parent(",
        "_file_identity(temp_parent_stat) != expected_identity",
        "shutil.rmtree(temp_parent.name, dir_fd=parent_fd)",
        "staged slot temporary directory could not be removed",
        "cleanup_errors = _cleanup_temp_parent(",
        "if stage_errors or cleanup_errors:",
        "device_lab.validate_required_kagemusha_slot_artifact_shapes(",
        "device_lab.validate_d2d_payment_transcript(",
        "device_lab.validate_wallet_integrity_transcript(",
        "slot directory already exists; refuse to overwrite evidence",
        "signing inputs are required unless --allow-unsigned is set",
        "--private-key, --public-key, and --signer-key-id must be supplied together",
        "def _run_adb_getprop",
        "stdout.count(\"\\n\") != 1",
        "adb getprop output must be exactly one LF-terminated value",
        "adb getprop {prop} failed",
        'if override == "":\n        errors.append(f"{key} must be a non-empty string")\n        return None',
        "def build_device_identity_hints(",
        "hint_sources: dict[str, str] = {}",
        "if hints[key] != value:",
        "must match {hint_sources[key]} {key}",
        'if value == "":\n        errors.append(f"{label} {key} must be a non-empty string")\n        return None',
        "identity_hints: dict[str, str] | None = None",
        "Return device identity from overrides, captured artifacts, or ADB.",
        "if value is not None and hint_value is not None and value != hint_value:",
        "override must match captured source identity",
        "telemetry/telemetry.json",
        "identity_hints=identity_hints",
        "value != value.strip()",
        "device family could not be inferred; pass --device-family",
        "normalise_attestation_payloads",
        "validate_attestation_harness_source_claims",
        "--attestation-harness-result",
        "attestation harness result source",
        "set(attestation_result) - device_lab.ATTESTATION_RESULT_FIELDS",
        "attestation/result.json contains unexpected field",
        "set(attestation_report) - device_lab.ATTESTATION_REPORT_FIELDS",
        "attestation/report.json contains unexpected field",
        "if report_schema != device_lab.ATTESTATION_REPORT_SCHEMA:",
        "attestation/report.json schema must be",
        '_require_source_string(attestation_report, "verifier", "attestation/report.json", errors)',
        "set(d2d_payment_transcript) - device_lab.D2D_PAYMENT_TRANSCRIPT_FIELDS",
        "d2d payment transcript contains unexpected field",
        "if d2d_schema != device_lab.D2D_PAYMENT_TRANSCRIPT_SCHEMA:",
        "d2d payment transcript schema must be",
        "device_lab.D2D_PAYMENT_TRANSCRIPT_SCHEMA",
        "set(wallet_integrity_transcript) - device_lab.WALLET_INTEGRITY_TRANSCRIPT_FIELDS",
        "wallet integrity transcript contains unexpected field",
        "if wallet_schema != device_lab.WALLET_INTEGRITY_TRANSCRIPT_SCHEMA:",
        "wallet integrity transcript schema must be",
        "device_lab.WALLET_INTEGRITY_TRANSCRIPT_SCHEMA",
        "set(verification) - device_lab.ATTESTATION_REPORT_VERIFICATION_FIELDS",
        "attestation/report.json verification contains unexpected field",
        "f\"{label} {key} must not have surrounding whitespace\"",
        "f\"{label} {key} must not contain control characters\"",
        "def _require_source_sha256(",
        "device_lab.SHA256_HEX_RE.fullmatch(value)",
        'f"{label} {key} must be non-zero lowercase sha256 hex"',
        "def _require_metadata_sha256(",
        'raise ValueError(f"{label} must be non-zero lowercase sha256 hex")',
        "source_digests: dict[str, str] = {}",
        "and result_app_package != report_app_package",
        "attestation/report.json app_package_name must match",
        "attestation/report.json attestation_challenge_sha256 must match",
        'if result_status is not None and result_status != "ok":',
        'if report_status is not None and report_status != "ok":',
        "attestation/result.json status must be ok",
        "attestation/report.json verification.status must be ok",
        "and result_status != report_status",
        "attestation/report.json verification.status must match",
        "and result_level != report_level",
        "attestation/report.json verification.{level_key} must match",
        '_require_source_sha256(\n        attestation_result,\n        "offline_wallet_policy_sha256"',
        "if level is not None and level not in device_lab.STRONGBOX_LEVELS:",
        "attestation harness result challenge_hex must be lowercase hexadecimal without whitespace",
        "challenge_hex != challenge_hex.lower()",
        "attestation harness result challenge_hex digest must match",
        "attestation/report.json device_fingerprint must match device identity",
        "attestation/report.json os_build_id must match device identity",
        "wallet integrity transcript rollback_rejection_passed must be true",
        "evidence_signer.rewrite_sha256_manifest(stage_slot)",
        "evidence_signer.sign_slot_evidence",
        "--allow-unsigned",
        "The production readiness rollup will reject it.",
    ),
    "scripts/kagemusha_pull_android_device_lab_raw_slot.py": (
        "Pull raw Kagemusha Android device-lab artifacts from an attached device",
        "DEFAULT_RUN_AS_PACKAGE = \"org.hyperledger.iroha.sdk.offline.wallet.lab\"",
        "DEFAULT_DEVICE_LAB_DEVICE_ROOT = \"files/kagemusha-device-lab\"",
        "ADB_LATEST_SLOT_COMMAND_HELP",
        "ADB_PULL_TAR_COMMAND_HELP",
        "MAX_RAW_SLOT_ENTRIES = 256",
        "def _validate_non_secret_adb_string",
        "if args.serial is not None:",
        "{label} must be a non-empty string",
        "{label} must not contain surrounding whitespace",
        "{label} must not contain control characters",
        "{label} must not contain secret-looking material",
        "def _path_shape_errors",
        "raw output root path must not contain backslashes",
        "raw output root path must be canonical",
        "errors.extend(_path_shape_errors(args.out_root, \"raw output root path\"))",
        "errors.extend(_path_shape_errors(args.summary_out, \"raw pull summary output\"))",
        "KagemushaDeviceLabArtifactExportTest",
        "RAW_SLOT_REQUIRED_PATHS",
        "RAW_SLOT_ALLOWED_PATHS",
        "RAW_SLOT_ALLOWED_DIRECTORIES",
        "raw slot artifact {relative} is not an allowed path",
        "raw slot artifact paths must not contain control characters",
        "entry_count += 1",
        "raw slot tar must not contain more than {MAX_RAW_SLOT_ENTRIES} entries",
        "def _set_private_directory_permissions",
        "raw output root directory",
        "os.fchmod(dir_fd, 0o700)",
        "stat.S_IMODE(directory_stat.st_mode) != 0o700",
        "destination.parent.mkdir(mode=0o700",
        "destination.parent.chmod(0o700)",
        "os.fchmod(output.fileno(), 0o600)",
        "directory.mkdir(mode=0o700",
        "directory.chmod(0o700)",
        "attestation/harness-result.json",
        "HARNESS_RESULT_ALLOWED_FIELDS",
        "_validate_harness_result",
        "attestation/harness-result.json strongbox_attestation must be true",
        "attestation/harness-result.json {key} must not have surrounding whitespace",
        "attestation/harness-result.json {key} must not contain control characters",
        "attestation/harness-result.json {key} must not contain secret-looking material",
        "attestation/harness-result.json challenge_hex must be lowercase hexadecimal without whitespace",
        "attestation/harness-result.json chain_length must match",
        "attestation/harness-result.json challenge_hex must match attestation/challenge.hex",
        "_validate_challenge_hex_file",
        "attestation/challenge.hex must be canonical lowercase hexadecimal plus trailing newline",
        "challenge_text.count(\"\\n\") != 1",
        "latest-slot.txt must be canonical and contain exactly one slot id",
        "latest_text.count(\"\\n\") != 1",
        "extract_raw_slot_tar",
        "mode=\"r:\"",
        "allow_trailing_slash=member.isdir()",
        "raw slot tar member path must not contain control characters",
        "raw slot tar member has noncanonical path",
        "slot directory already exists; refuse to overwrite raw evidence",
        'latest_text != f"{slot_id}\\n"',
        "latest-slot.txt must be canonical and match slot id",
        "raw latest-slot output must not be a symlink after writing",
        "raw latest-slot output must not be hardlinked after writing",
        "raw latest-slot output changed while being read back",
        "raw latest-slot output permissions must be 0600",
        "expected_identity=root_identity",
        "def _cleanup_temp_output(",
        "temp_identity = _file_identity(os.fstat(output.fileno()))",
        "_file_identity(temp_stat) != expected_identity",
        "os.unlink(path.name, dir_fd=parent_fd)",
        "temporary output changed before cleanup",
        "cleanup_errors = _cleanup_temp_output(",
        "raw slot tar directory {relative} could not be created",
        "_validate_sha256_hex",
        "must be a lowercase SHA-256 hex digest",
        "must be a non-zero lowercase SHA-256 hex digest",
        "RAW_RESULT_ALLOWED_FIELDS",
        "RAW_RESULT_STRING_FIELDS",
        'RAW_RESULT_APP_SIGNING_DIGEST_FIELD = "app_signing_certificate_sha256"',
        'RAW_RESULT_CHALLENGE_DIGEST_FIELD = "attestation_challenge_sha256"',
        'RAW_RESULT_CHAIN_DIGEST_FIELD = "attestation_certificate_chain_sha256"',
        'RAW_RESULT_POLICY_DIGEST_FIELD = "offline_wallet_policy_sha256"',
        "RAW_RESULT_SHA256_FIELDS",
        "RAW_RESULT_STRONGBOX_FIELDS",
        "PENDING_QUEUE_FIELDS",
        "def _validate_raw_result_string",
        "_validate_raw_result_string",
        "device_lab._contains_control_character(value)",
        "_validate_raw_json_artifacts",
        "def _validate_raw_json_slot_id",
        "_validate_raw_json_slot_id",
        "slot_id must be a non-empty string",
        "slot_id must not contain surrounding whitespace",
        "slot_id must not contain control characters",
        "_validate_raw_json_true",
        "device_lab.D2D_PAYMENT_TRANSCRIPT_SCHEMA",
        "device_lab.D2D_PAYMENT_PAYLOAD_SCHEMA",
        "device_lab.WALLET_INTEGRITY_TRANSCRIPT_SCHEMA",
        "queue/pending_queue.json contains unexpected field",
        "queue/pending_queue.json pending_transactions must be an array",
        "queue/pending_queue.json pending_transactions must be empty after D2D handoff",
        "TELEMETRY_FIELDS",
        "TELEMETRY_STRING_FIELDS",
        "telemetry/telemetry.json contains unexpected field",
        "telemetry/telemetry.json app_package_name must match",
        "telemetry/telemetry.json suite must identify a Kagemusha device-lab run",
        "telemetry/telemetry.json suite must be a non-empty string",
        "telemetry/telemetry.json suite must not contain surrounding whitespace",
        "telemetry/telemetry.json suite must not contain control characters",
        "suite != device_lab.KAGEMUSHA_TELEMETRY_SUITE",
        "\"transport_offline\"",
        "\"rollback_rejection_passed\"",
        "_validate_raw_status_ndjson",
        "device_lab._loads_json_without_duplicate_keys",
        "device_lab.STATUS_EVENT_FIELDS",
        "device_lab.KAGEMUSHA_STATUS_FAILURE_VALUES",
        "device_lab.KAGEMUSHA_RUNTIME_LOG_FAILURE_MARKERS",
        "telemetry/status.ndjson must use LF line endings",
        "telemetry/status.ndjson must end with a trailing newline",
        "telemetry/status.ndjson line {line_no} contains unexpected field",
        "telemetry/status.ndjson line {line_no} must not contain surrounding whitespace",
        "telemetry/status.ndjson line {line_no} status must not contain surrounding whitespace",
        "telemetry/status.ndjson line {line_no} status must not contain control characters",
        "telemetry/status.ndjson line {line_no} status must be lowercase",
        "telemetry/status.ndjson line {line_no} status must be ok",
        "telemetry/status.ndjson line {line_no} slot_id must be a non-empty string",
        "telemetry/status.ndjson line {line_no} slot_id must be a string",
        "telemetry/status.ndjson line {line_no} slot_id must not contain surrounding whitespace",
        "telemetry/status.ndjson line {line_no} slot_id must not contain control characters",
        "telemetry/status.ndjson line {line_no} slot_id must match slot id",
        "telemetry/status.ndjson line {line_no} status must not be",
        "logs/runtime.log must not contain failure marker {marker}",
        "for field in RAW_RESULT_SHA256_FIELDS:",
        "for field in RAW_RESULT_STRONGBOX_FIELDS:",
        "attestation/result.json {field} must be STRONGBOX",
        "must not have surrounding whitespace",
        "attestation/result.json contains unexpected field",
        'result.get("slot") != slot_id',
        "attestation/result.json slot must match slot id",
        "must not be a symlink or hardlink",
        "must not exceed",
        "slot-mismatched tar members",
        "device_lab.validate_summary_output_path",
        "raw pull summary output is not strict JSON",
        "if len(encoded) > device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES:",
        "raw pull summary output must be no more than",
        "device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES",
        "raw pull summary output parent directory could not be synced",
        "raw pull summary output parent directory metadata could not be read",
        "parent_identity = _file_identity(parent_stat)",
        "expected_identity=parent_identity",
        "raw pull summary output must not be a symlink after writing",
        "raw pull summary output must not be hardlinked after writing",
        "raw pull summary output changed while being read back",
        "raw pull summary output permissions must be 0600",
        "expected_identity = _file_identity(expected_stat)",
        "with path.open(\"rb\") as readback_handle:",
        "_file_identity(final_stat) != expected_identity",
        "def _raw_artifact_digest",
        "os.fstat(handle.fileno())",
        "open_identity != expected_identity",
        "path_identity != expected_identity",
        "raw artifact digest inventory must include every required artifact",
        "def _install_validated_slot",
        "def _file_identity",
        "def _directory_open_flags",
        "O_NOFOLLOW",
        "os.fstat(dir_fd)",
        "if _file_identity(open_stat) != expected_identity:",
        "def _slot_entry_identity",
        "os.stat(path.name, dir_fd=parent_fd, follow_symlinks=False)",
        "def _created_slot_identity_errors",
        "raw slot directory changed during install",
        "def _remove_created_slot",
        "parent_fd = os.open(parent_path, _directory_open_flags())",
        "os.stat(path.name, dir_fd=parent_fd, follow_symlinks=False)",
        "_file_identity(path_stat) == expected_identity",
        "shutil.rmtree(path.name, dir_fd=parent_fd)",
        "raw slot partial install could not be removed",
        "cleanup_errors = _remove_created_slot(",
        "return [*install_errors, *cleanup_errors]",
        "def _cleanup_temp_parent",
        "_file_identity(temp_parent_stat) != expected_identity",
        "shutil.rmtree(temp_parent.name, dir_fd=parent_fd)",
        "raw pull temporary directory could not be removed",
        "cleanup_errors = _cleanup_temp_parent(",
        "if pull_errors or cleanup_errors:",
        "def _open_verified_directory",
        "os.listdir(stage_fd)",
        "os.rename(",
        "src_dir_fd=stage_fd",
        "dst_dir_fd=final_fd",
        "def _remove_empty_stage_slot",
        "os.rmdir(stage_slot.name, dir_fd=parent_fd)",
        "raw pull temporary directory metadata could not be read",
        "raw output root directory metadata could not be read",
        "raw output root directory changed during install",
        "final_slot.mkdir(mode=0o700)",
        "raw slot install source contains unexpected top-level entry",
        "device_lab._display_path(child_name)",
        "_slot_entry_identity(\n        final_slot,\n        output_root,\n        output_root_identity",
        "expected_identity=final_slot_identity",
        "expected_identity=output_root_identity",
        "raw slot directory could not be synced",
        "raw slot directory parent could not be synced",
    ),
    "scripts/kagemusha_android_attestation_report.py": (
        "Render a slot-bound Kagemusha Android attestation verifier report",
        "DEFAULT_VERIFIER = \"android-keystore-attestation-harness\"",
        "HARNESS_RESULT_FIELDS",
        "PHYSICAL_DEVICE_ASSERTION_REQUIRED",
        "--physical-device-attestation",
        "physical device attestation must be explicitly asserted",
        "def _reject_whitespace",
        "def _reject_control",
        "f\"{label} must not contain whitespace\"",
        "f\"{label} must not contain control characters\"",
        "if _reject_whitespace(value, label, errors):\n        return None\n    if _reject_control(value, label, errors):\n        return None\n    candidate = PurePosixPath(value)",
        "f\"{label} must be a canonical single directory name\"",
        'or "\\\\" in value',
        "if _reject_control(value, label, errors):\n        return None\n    if device_lab.SECRET_RE.search(value):",
        "if _reject_control(value, label, errors):\n        return None\n    if value not in device_lab.STRONGBOX_LEVELS:",
        "_string_value(result.get(\"alias\"), \"attestation harness result alias\", errors)",
        "attestation harness result strongbox_attestation must be true",
        "attestation harness result keymaster_security_level must be STRONGBOX",
        "_pem_certificate_count",
        "attestation certificate chain PEM must contain at least two certificates",
        "elif chain_length != certificate_count:",
        "attestation harness result chain_length must match",
        "attestation certificate-chain certificate count",
        "f\"{label} must be lowercase hexadecimal without whitespace\"",
        "\"--expected-challenge-hex\",",
        '    if value != value.strip() or any(ch.isspace() for ch in value):\n        errors.append(f"{label} must be lowercase hexadecimal without whitespace")\n        return None\n',
        'if device_lab._contains_control_character(value):\n        errors.append(f"{label} must not contain control characters")\n        return None',
        'if any(ch not in "0123456789abcdef" for ch in value):',
        'result = device_lab._load_json(path, "attestation harness result", errors)',
        "attestation certificate chain path must not contain whitespace",
        "attestation certificate chain path must not contain control characters",
        "attestation certificate chain path must not contain backslashes",
        "elif raw != raw.strip() or any(ch.isspace() for ch in raw):",
        "if device_lab._contains_control_character(raw):",
        'if device_lab._contains_control_character(path_text):\n        errors.append(f"{label} path must not contain control characters")\n        return None, None',
        'if "\\\\" in path_text:\n        errors.append(f"{label} path must not contain backslashes")\n        return None, None',
        'if ".." in path.parts:\n        errors.append(f"{label} path must be canonical")\n        return None, None',
        "attestation certificate chain path must stay under attestation/",
        "attestation certificate chain path must be canonical",
        "device_lab.ATTESTATION_REPORT_SCHEMA",
        "device_lab.validate_summary_output_path(path, label)",
        "def _set_private_directory_permissions",
        "os.fchmod(dir_fd, 0o700)",
        "stat.S_IMODE(directory_stat.st_mode) != 0o700",
        "tmp_identity = device_lab._file_identity(os.fstat(handle.fileno()))",
        "os.fchmod(handle.fileno(), 0o600)",
        "stat.S_IMODE(expected_stat.st_mode) != 0o600",
        "write_errors.extend(_cleanup_temp_output(tmp_path, label, tmp_identity))",
        "temporary file changed before cleanup",
        "device_lab._file_identity(temp_stat) != expected_identity",
        "os.unlink(path.name, dir_fd=parent_fd)",
        "parent_identity = device_lab._file_identity(parent_stat)",
        "sync_errors = device_lab._sync_summary_output_parent(",
        "expected_identity=parent_identity",
    ),
    "scripts/android_keystore_attestation.sh": (
        "Verify an Android Keystore attestation bundle using the Iroha Android attestation harness",
        "Files named trust_root_*.pem in the bundle directory are detected",
        "The script compiles the attestation harness and its direct verifier dependencies",
        "MAIN_SOURCES=(",
        "org/hyperledger/iroha/android/crypto/keystore/KeyAttestation.java",
        "org/hyperledger/iroha/android/crypto/keystore/attestation/AttestationVerifier.java",
        "org/hyperledger/iroha/android/tools/AndroidKeystoreAttestationHarness.java",
        'javac "${JAVAC_FLAGS[@]}" "${MAIN_SOURCES[@]}"',
        'if [[ -z "$BUNDLE_DIR" && ${#TRUST_ROOTS[@]} -eq 0',
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
        "EVIDENCE_CONTROL_STRING_REDACTION",
        "def _display_evidence_field(field: str) -> str:",
        "device_lab._contains_control_character(field)",
        "MAX_ABI6_MANIFEST_JSON_BYTES",
        "MAX_REPO_SOURCE_MARKER_BYTES = 8 * 1024 * 1024",
        "MAX_LINEAGE_PROOF_EVIDENCE_JSON_BYTES",
        "MAX_COMPACT_KEY_EVIDENCE_JSON_BYTES",
        "LINEAGE_PROOF_EVIDENCE_FIELDS",
        "LINEAGE_PROOF_TEST_FIELDS",
        "COMPACT_KEY_EVIDENCE_FIELDS",
        "\"artifact_size_bytes\"",
        "def _secret_looking_path_blocker(",
        "device_lab._contains_control_character(value)",
        "def _require_compact_key_artifact_size(",
        "compact_key_evidence_artifact_sizes",
        "compact_key_evidence_artifact_sizes_unexpected_field",
        "ABI-7 recursive compact key evidence artifact size does not match local artifact bytes",
        "\"root\": ANDROID_DEVICE_LAB_ROOT_SUMMARY_LABEL",
        "ABI6_MANIFEST_SCHEMA",
        "ABI6_OPERATION_SYMBOLS",
        "check_abi6_reserved_lineage",
        "KagemushaCommand::RecursiveCompactKeyArtifacts",
        "KagemushaRecursiveCompactKeyArtifactsArgs",
        "derive_halo2_ipa_kagemusha_recursive_compact_payment_token_proving_key_bytes",
        "kagemusha_recursive_compact_payment_token_vk_record_from_box",
        "validate_release_local_json_file",
        "_validate_release_local_json_file_for_read",
        "def _read_release_json_text(",
        "release_json_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        "release_json_open_identity = (open_stat.st_dev, open_stat.st_ino)",
        "release_json_path_stat = path.lstat()",
        "release_json_final_path_stat = path.lstat()",
        "if open_stat.st_size > MAX_ABI6_MANIFEST_JSON_BYTES:",
        "if size > MAX_ABI6_MANIFEST_JSON_BYTES:",
        'f"{label} must be no more than {MAX_ABI6_MANIFEST_JSON_BYTES} bytes"',
        'except OSError:\n        return None, [blocker(unreadable_code, f"{label} could not be read")]',
        'except UnicodeDecodeError:\n        return None, [blocker(unreadable_code, f"{label} could not be read")]',
        '_file_stat, errors = _validate_release_local_json_file_for_read(path, label)',
        'def _validate_release_local_json_file_for_read(\n    path: Path,\n    label: str,\n) -> tuple[os.stat_result | None, list[str]]:\n    """Reject local release JSON files and return the read identity."""\n\n    path_text = str(path)\n    if device_lab.SECRET_RE.search(path_text):\n        return None, [f"{label} path must not contain secret-looking material"]\n',
        'if device_lab._contains_control_character(path_text):\n        return None, [f"{label} path must not contain control characters"]',
        'if "\\\\" in path_text:\n        return None, [f"{label} path must not contain backslashes"]',
        'if ".." in path.parts:\n        return None, [f"{label} path must be canonical"]',
        "release_json_ancestor_errors = device_lab.validate_no_symlink_ancestors(",
        '    release_json_ancestor_errors = device_lab.validate_no_symlink_ancestors(\n        path,\n        f"{label} ancestor directory",\n    )\n    if release_json_ancestor_errors:\n        return None, release_json_ancestor_errors\n    try:\n        file_stat = path.lstat()\n    except FileNotFoundError:\n        return None, [f"{label} is missing"]\n    except OSError:\n        return None, [f"{label} file metadata could not be read"]\n    if stat.S_ISLNK(file_stat.st_mode):\n        return None, [f"{label} must not be a symlink"]\n    if not stat.S_ISREG(file_stat.st_mode):\n        return None, [f"{label} must be a regular file"]\n',
        '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        return None, [f"{label} hardlink metadata could not be read"]\n    if link_count > 1:\n        return None, [f"{label} must not be hardlinked"]\n    return file_stat, []\n\n\ndef _validate_repo_source_marker_file_for_read(\n',
        "validate_repo_source_marker_file",
        "def _validate_repo_source_marker_file_for_read(",
        'def _validate_repo_source_marker_file_for_read(\n    path: Path,\n    label: str,\n) -> tuple[os.stat_result | None, list[str]]:\n    """Reject checked-in marker files that could alias external bytes."""\n\n    path_text = str(path)\n    if device_lab.SECRET_RE.search(path_text):\n        return None, [f"{label} path must not contain secret-looking material"]\n',
        'if device_lab._contains_control_character(path_text):\n        return None, [f"{label} path must not contain control characters"]',
        'if "\\\\" in path_text:\n        return None, [f"{label} path must not contain backslashes"]',
        'if ".." in path.parts:\n        return None, [f"{label} path must be canonical"]',
        '    try:\n        file_stat = path.lstat()\n    except FileNotFoundError:\n        errors.append(f"{label} is missing")\n        return None, errors\n    except OSError:\n        errors.append(f"{label} file metadata could not be read")\n        return None, errors\n    if stat.S_ISLNK(file_stat.st_mode):\n        errors.append(f"{label} must not be a symlink")\n        return None, errors\n    if not stat.S_ISREG(file_stat.st_mode):\n        errors.append(f"{label} must be a regular file")\n        return None, errors\n',
        '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        errors.append(f"{label} hardlink metadata could not be read")\n        return None, errors\n    if link_count > 1:\n        errors.append(f"{label} must not be hardlinked")\n    if errors:\n        return None, errors\n    return file_stat, []\n\n\ndef validate_repo_source_marker_file(path: Path, label: str) -> list[str]:',
        "_file_stat, errors = _validate_repo_source_marker_file_for_read(path, label)",
        "expected_marker_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        "open_marker_identity = (open_stat.st_dev, open_stat.st_ino)",
        "def _repo_source_marker_text(",
        "marker_path_stat = path.lstat()",
        "marker_final_path_stat = path.lstat()",
        'f"{label} changed while being read"',
        "if open_stat.st_size > MAX_REPO_SOURCE_MARKER_BYTES:",
        "if size > MAX_REPO_SOURCE_MARKER_BYTES:",
        'f"{label} must be no more than {MAX_REPO_SOURCE_MARKER_BYTES} bytes"',
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
        "--command must not contain surrounding whitespace",
        "--command must not contain control characters",
        "--command must not contain secret-looking material",
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
        "def _validate_lineage_local_file_for_read(",
        '    path_text = str(path)\n    if device_lab.SECRET_RE.search(path_text):\n        return None, [f"{label} path must not contain secret-looking material"]',
        'if device_lab._contains_control_character(path_text):\n        return None, [f"{label} path must not contain control characters"]',
        'if "\\\\" in path_text:\n        return None, [f"{label} path must not contain backslashes"]',
        'if ".." in path.parts:\n        return None, [f"{label} path must be canonical"]',
        '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        return None, [f"{label} hardlink metadata could not be read"]\n    if link_count > 1:\n        return None, [f"{label} must not be hardlinked"]\n    return file_stat, []\n\n\ndef validate_lineage_local_file(path: Path, label: str) -> list[str]:\n',
        'except OSError:\n        return None, [f"{label} could not be read"]',
        'size_error = f"production proof log must be no more than {MAX_LINEAGE_PROOF_LOG_BYTES} bytes"',
        "def _lineage_local_text(",
        "def _sha256_text_file(",
        "chunks: list[bytes] = []",
        'text = b"".join(chunks).decode("utf-8", errors=decode_errors)',
        'digest, text, read_errors = _sha256_text_file(\n        path,\n        "production proof log",\n        "production proof log could not be read",',
        "max_bytes=MAX_LINEAGE_PROOF_LOG_BYTES",
        'digest, text, read_errors = _sha256_text_file(\n        path,\n        "ABI-7 recursive compact key generator log",\n        "ABI-7 recursive compact key generator log could not be read",',
        "max_bytes=MAX_COMPACT_KEY_GENERATOR_LOG_BYTES",
        "DuplicateJsonKeyError",
        "NonFiniteJsonConstantError",
        "_reject_duplicate_json_object_pairs",
        "_reject_nonfinite_json_constant",
        "shape_code: str",
        "max_bytes: int | None = None",
        'size_error = (\n        f"{label} must be no more than {max_bytes} bytes"\n        if max_bytes is not None\n        else None\n    )',
        'digest, text, read_errors = _sha256_text_file(\n        path,\n        label,\n        f"{label} could not be read",\n        max_bytes=max_bytes,\n        too_large_error=size_error,\n    )',
        "too_large_error=size_error",
        "shape_code=\"lineage_proof_evidence_file_shape\"",
        "max_bytes=MAX_LINEAGE_PROOF_EVIDENCE_JSON_BYTES",
        "shape_code=\"compact_key_evidence_file_shape\"",
        "max_bytes=MAX_COMPACT_KEY_EVIDENCE_JSON_BYTES",
        "object_pairs_hook=_reject_duplicate_json_object_pairs",
        "parse_constant=_reject_nonfinite_json_constant",
        "contains duplicate JSON object key",
        "non-finite constant",
        "def _display_evidence_value(value: Any) -> Any:",
        "_append_evidence_timestamp_string_blockers",
        "timestamp_surrounding_whitespace",
        "timestamp_control_character",
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
        '_android_report_kagemusha(report).get("signed_at_utc")',
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
        'device_lab.SECRET_RE.search(path_text)',
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
        "_android_report_kagemusha",
        "_android_report_device_family",
        "android_device_lab_duplicate_device_fingerprint",
        "android_device_lab_duplicate_attestation_challenge",
        "android_device_lab_binding_digest_invalid",
        "Android device-lab production slots must not reuse a device fingerprint",
        "Android device-lab production binding digests must be non-zero lowercase sha256 hex",
        "device_lab.SHA256_HEX_RE.fullmatch(value)",
        'or value == "0" * 64',
        "safe_slot = _display_evidence_value(slot)",
        "value_sha256",
        "_redact_secret_strings",
        "_sanitize_android_reports",
        "android_device_lab_report_unsafe_material",
        "android_device_lab_report_redacted_key_collision",
        "android_device_lab_report_non_string_key",
        "android_device_lab_report_unsupported_value",
        "key_collision = False",
        "if redacted_key in redacted:",
        "EVIDENCE_CONTROL_STRING_REDACTION",
        "EVIDENCE_NONFINITE_NUMBER_REDACTION",
        "EVIDENCE_NON_STRING_KEY_REDACTION",
        "EVIDENCE_UNSUPPORTED_VALUE_REDACTION",
        "EVIDENCE_ERRORS_NORMALIZED_FIELD",
        "EVIDENCE_ERROR_REDACTION",
        "def _android_report_errors_value(value: Any) -> tuple[list[str], bool]:",
        "def _android_report_errors(report: dict[str, Any]) -> list[str]:",
        "android_device_lab_report_errors_malformed",
        'sanitized_report["errors"] = safe_errors',
        'errors=_android_report_errors(report)',
        "math.isfinite(value)",
        "ANDROID_SIGNED_EVIDENCE_SUMMARY_FIELDS",
        "ANDROID_SIGNED_EVIDENCE_SUMMARY_TARGET_FIELDS",
        "ANDROID_SIGNED_EVIDENCE_SUMMARY_IDENTITY_FIELDS",
        "ANDROID_SIGNED_EVIDENCE_SUMMARY_ARTIFACT_PAIRS",
        "ANDROID_SIGNED_EVIDENCE_SUMMARY_CORE_FIELDS",
        "ANDROID_SLOT_RELEASE_KAGEMUSHA_FIELDS",
        '("signed_at_utc", "artifact_sha256", "signer_public_key_sha256")',
        '("offline_wallet_apk_path", "offline_wallet_apk_sha256")',
        '("d2d_payment_transcript_path", "d2d_payment_transcript_sha256")',
        '("wallet_integrity_transcript_path", "wallet_integrity_transcript_sha256")',
        '("attestation_certificate_chain_path", "attestation_certificate_chain_sha256")',
        '("device_family", "device_family")',
        '("device_model", "device_model")',
        '("device_codename", "device_codename")',
        "_check_android_signed_evidence_summary_values",
        "_valid_android_signed_evidence_summary_value",
        "device_lab.infer_kagemusha_device_family",
        "validated Android device-lab report model/codename must match its device family",
        "for pair in ANDROID_SIGNED_EVIDENCE_SUMMARY_ARTIFACT_PAIRS:",
        "artifact_fields = expected & set(entry)",
        "if artifact_fields and artifact_fields != expected:",
        "core_fields = ANDROID_SIGNED_EVIDENCE_SUMMARY_CORE_FIELDS & set(entry)",
        "if core_fields and core_fields != ANDROID_SIGNED_EVIDENCE_SUMMARY_CORE_FIELDS:",
        "identity_fields = ANDROID_SIGNED_EVIDENCE_SUMMARY_IDENTITY_FIELDS & set(entry)",
        "identity_fields and identity_fields != ANDROID_SIGNED_EVIDENCE_SUMMARY_IDENTITY_FIELDS",
        "for field in ANDROID_SIGNED_EVIDENCE_SUMMARY_IDENTITY_FIELDS:",
        "entry.pop(field, None)",
        "if set(entry) != ANDROID_SIGNED_EVIDENCE_SUMMARY_TARGET_FIELDS:",
        "continue",
        "android_signed_evidence_summary_invalid",
        "android_signed_evidence_summary_missing",
        "android_signed_evidence_summary_slot_invalid",
        "android_signed_evidence_summary_slot_collision",
        "seen_slots: set[str] = set()",
        "if safe_slot is None:",
        "slot not in signed_evidence",
        "def _android_safe_slot_id(report: dict[str, Any]) -> str | None:",
        "def _android_report_has_complete_signed_evidence(",
        "def _android_slot_reports_summary(",
        "def _android_duplicate_matrix_bindings_summary(",
        "and _android_report_has_complete_signed_evidence(report, signed_evidence)",
        "if not _android_report_has_complete_signed_evidence(report, signed_evidence):",
        '"slots": _android_slot_reports_summary(reports, signed_evidence),',
        '"duplicate_bindings": _android_duplicate_matrix_bindings_summary(',
        "blockers.extend(_check_android_signed_evidence_summary_values(reports))",
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
        "def _repo_root_shape_blocker(root: Path)",
        "shape_blocker = _repo_root_shape_blocker(root)",
        "--repo-root must not contain backslashes",
        "--repo-root must be a canonical directory path",
        '    try:\n        root_mode = root.lstat().st_mode\n    except FileNotFoundError:\n        root_mode = None\n    except OSError:\n        errors.append("--repo-root metadata could not be read")\n        return [\n            blocker("kagemusha_repo_root_path_invalid", error)\n            for error in errors\n        ]\n',
        "root_mode is not None and stat.S_ISLNK(root_mode)",
        "root_mode is not None and not stat.S_ISDIR(root_mode)",
        "repo_root_blockers = validate_repo_root_path(repo_root)",
        "repo_root_errors = validate_repo_root_path(Path(args.repo_root))",
        "--repo-root must not be a symlink",
        "--repo-root ancestor directory",
        "def _cli_path_shape_blocker(",
        "f\"{label} must be a canonical path\"",
        "f\"{label} must not contain backslashes\"",
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
        "from collections.abc import Mapping",
        "def _safe_trusted_signer_public_key_sha256(",
        "device_lab._trusted_signer_public_key_sha256_set(",
        "signer_map_blockers = [",
        "device_lab.validate_trusted_signer_public_key_map(",
        "trusted_signer_public_key_sha256 = _safe_trusted_signer_public_key_sha256(",
        '"trusted_signer_public_key_sha256": trusted_signer_public_key_sha256',
        "MAX_READINESS_SUMMARY_JSON_BYTES",
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
        "allow_nan=False",
        "--summary-out summary is not strict JSON",
        'if len(summary_text.encode("utf-8")) > MAX_READINESS_SUMMARY_JSON_BYTES:',
        "--summary-out could not be written",
        "_cleanup_summary_output",
        "--summary-out temporary file could not be removed",
        "tmp_identity = _file_identity(os.fstat(handle.fileno()))",
        "write_blockers.extend(_cleanup_summary_output(tmp_path, tmp_identity))",
        "--summary-out temporary file changed before cleanup",
        "_file_identity(temp_stat) != expected_identity",
        "os.unlink(path.name, dir_fd=parent_fd)",
        "tempfile.NamedTemporaryFile(",
        "os.fchmod(handle.fileno(), 0o600)",
        "handle.flush()",
        "os.fsync(handle.fileno())",
        "os.replace(tmp_path, path)",
        "_read_summary_output_text",
        "summary_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        "summary_open_identity = (open_stat.st_dev, open_stat.st_ino)",
        "--summary-out changed while being read",
        "stat.S_IMODE(open_stat.st_mode) != 0o600",
        "--summary-out permissions must be 0600",
        "if open_stat.st_size > MAX_READINESS_SUMMARY_JSON_BYTES:",
        "if size > MAX_READINESS_SUMMARY_JSON_BYTES:",
        'f"--summary-out must be no more than {MAX_READINESS_SUMMARY_JSON_BYTES} bytes"',
        "readback_text, readback_errors = _read_summary_output_text(path, expected_stat)",
        "readback_text != summary_text",
        "--summary-out write verification failed",
        "--summary-out parent directory could not be synced",
        "--summary-out parent directory changed before sync",
        "def _file_identity",
        "def _directory_open_flags",
        "O_NOFOLLOW",
        "def _sync_summary_output_parent",
        "os.fstat(parent_fd)",
        "expected_identity=parent_identity",
        '    except OSError:\n        return None, [\n            _summary_out_blocker("--summary-out write verification failed")\n        ]\n',
        '    errors = validate_summary_output_path(path)\n    if errors:\n        return errors\n    try:\n        expected_stat = path.lstat()\n',
        "--summary-out must not be a symlink",
        '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        return [\n            blocker(\n                SUMMARY_OUT_PATH_INVALID_CODE,\n                "--summary-out hardlink metadata could not be read",\n            )\n        ]\n',
        "--summary-out must not be hardlinked",
        "--summary-out ancestor directory",
        "trusted_signer_public_key_sha256",
        'signed_evidence = _android_signed_evidence_summary(reports)',
        '"signed_evidence": signed_evidence',
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
        "_validate_generated_at_future_skew",
        "_validate_generated_at_future_skew(\n            generated_at,\n            max_generated_at_future_skew_seconds,",
        "--generated-at-utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
        "--generated-at-utc must not be ahead of the helper clock skew allowance",
        "--max-generated-at-future-skew-seconds",
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
        "must not contain control characters",
        "must not contain backslashes",
        "must be canonical",
        "Path(path).parts",
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
        'out_secret_error = _secret_path_error(str(out_path), "--out")',
        'artifact_dir_secret_error = _secret_path_error(str(artifact_dir), "--artifact-dir")',
        "path_errors.extend(validate_output_corridor(out_path, artifact_dir))",
        "validate_lineage_input_paths",
        'proof_log_secret_error = _secret_path_error(str(proof_log), "--proof-log")\n    if proof_log_secret_error is not None:\n        return [proof_log_secret_error]\n    errors = validate_artifact_dir_path(artifact_dir)\n    if errors:\n        return errors\n',
        "errors = validate_lineage_input_paths(artifact_dir, proof_log)",
        "path_errors.extend(validate_lineage_input_paths(artifact_dir, proof_log))",
        "preflight_output_path",
        'def preflight_output_path(path: Path, label: str) -> list[str]:\n    """Reject aliased output paths before evidence inputs are read."""\n\n    secret_error = _secret_path_error(str(path), label)\n    if secret_error is not None:\n        return [secret_error]\n',
        '    parent_exists, parent_errors = _validate_output_parent(path, label)\n    if parent_errors:\n        return parent_errors\n',
        '    output_ancestor_errors = device_lab.validate_no_symlink_ancestors(\n        path,\n        f"{label} ancestor directory",\n    )\n    if output_ancestor_errors:\n        return output_ancestor_errors\n    if not parent_exists:\n        try:\n            parent.mkdir(mode=0o700, parents=True, exist_ok=True)\n        except OSError:\n            return [f"{label} parent directory could not be created"]\n',
        '    parent_exists, parent_errors = _validate_output_parent(\n        path,\n        label,\n        missing_error=f"{label} parent must be a directory",\n    )\n    if parent_errors:\n        return parent_errors\n    if not parent_exists:\n        return [f"{label} parent must be a directory"]\n',
        '    try:\n        output_mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return []\n    except OSError:\n        return [f"{label} file metadata could not be read"]\n',
        "if stat.S_ISLNK(output_mode):",
        "if not stat.S_ISREG(output_mode):",
        '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        return [f"{label} hardlink metadata could not be read"]\n    if link_count > 1:\n        return [f"{label} must not be hardlinked"]\n',
        'def _validate_output_parent(\n    path: Path,\n    label: str,\n    *,\n    missing_error: str | None = None,\n) -> tuple[bool, list[str]]:\n    """Classify an output parent without following symlink aliases."""\n\n    parent = path.parent\n    try:\n        parent_mode = parent.lstat().st_mode\n    except FileNotFoundError:\n        if missing_error is None:\n            return False, []\n        return False, [missing_error]\n    except OSError:\n        return False, [f"{label} parent directory metadata could not be read"]\n',
        '    if stat.S_ISLNK(parent_mode):\n        return True, [f"{label} parent directory must not be a symlink"]\n    if not stat.S_ISDIR(parent_mode):\n        return True, [f"{label} parent must be a directory"]\n    return True, []\n',
        "validate_output_path",
        '    errors = preflight_output_path(path, label)\n    if errors:\n        return errors\n    parent = path.parent\n    parent_exists, parent_errors = _validate_output_parent(path, label)\n    if parent_errors:\n        return parent_errors\n    if not parent_exists:\n        try:\n            parent.mkdir(mode=0o700, parents=True, exist_ok=True)\n        except OSError:\n            return [f"{label} parent directory could not be created"]\n    permission_errors = _set_private_directory_permissions(parent, f"{label} parent")\n    if permission_errors:\n        return permission_errors\n    return preflight_output_path(path, label)\n',
        'early_output_errors = preflight_output_path(out_path, "--out")',
        "--artifact-dir must not be a symlink",
        'f"{label} ancestor directory"',
        "write_errors = write_evidence(out_path, evidence)",
        """            allow_nan=False,
        ) + "\\n"
    except ValueError:
        return ["--out evidence is not strict JSON"]
""",
        "--out could not be written",
        "tempfile.NamedTemporaryFile(",
        "handle.flush()",
        "os.fsync(handle.fileno())",
        "os.replace(tmp_path, path)",
        "_cleanup_temp_output",
        "--out temporary file could not be removed",
        "tmp_identity = _file_identity(os.fstat(handle.fileno()))",
        "write_errors.extend(_cleanup_temp_output(tmp_path, tmp_identity))",
        "--out temporary file changed before cleanup",
        "_file_identity(temp_stat) != expected_identity",
        "os.unlink(path.name, dir_fd=parent_fd)",
        "--out parent directory could not be synced",
        "parent directory changed before sync",
        "def _file_identity",
        "def _directory_open_flags",
        "O_NOFOLLOW",
        "def _sync_output_parent",
        "os.fstat(parent_fd)",
        "expected_identity=parent_identity",
        "def _read_output_text(",
        "output_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        "output_open_identity = (open_stat.st_dev, open_stat.st_ino)",
        'f"{label} changed while being read"',
        'except OSError:\n        return None, [f"{label} write verification failed"]',
        'except UnicodeDecodeError:\n        return None, [f"{label} write verification failed"]',
        '    try:\n        expected_stat = path.lstat()\n    except (FileNotFoundError, OSError):\n        return ["--out write verification failed"]\n',
        """readback_text, readback_errors = _read_output_text(
        path,
        expected_stat,
        "--out",
        max_bytes=readiness.MAX_LINEAGE_PROOF_EVIDENCE_JSON_BYTES,
    )""",
        "readback_text != evidence_text",
        "--out write verification failed",
        '    errors = validate_output_path(path, "--out")\n    if errors:\n        return errors\n    try:\n        expected_stat = path.lstat()',
        "missing lineage artifact",
        "wrote evidence",
        "pre_create_dir_errors = validate_artifact_dir_path(artifact_dir)",
        '    try:\n        artifact_dir.mkdir(mode=0o700, parents=True, exist_ok=True)\n    except OSError:\n        return ["--artifact-dir could not be created for evidence validation"]\n',
        "def _set_private_directory_permissions",
        "os.fchmod(dir_fd, 0o700)",
        "stat.S_IMODE(directory_stat.st_mode) != 0o700",
        "permission_errors = _set_private_directory_permissions(",
        "os.fchmod(handle.fileno(), 0o600)",
        "stat.S_IMODE(expected_stat.st_mode) != 0o600",
        "--out permissions must be 0600",
        '    except ValueError:\n        return ["lineage proof evidence validation file is not strict JSON"]\n',
        "len(evidence_text.encode(\"utf-8\")) > readiness.MAX_LINEAGE_PROOF_EVIDENCE_JSON_BYTES",
        "max_bytes=readiness.MAX_LINEAGE_PROOF_EVIDENCE_JSON_BYTES",
        "lineage proof evidence validation file could not be written",
        'errors = ["lineage proof evidence validation file could not be written"]',
        "def _cleanup_validation_temp_output(",
        "validation_temp_stat = os.stat(",
        "_file_identity(validation_temp_stat) != expected_identity",
        "os.unlink(path.name, dir_fd=parent_fd)",
        "lineage proof evidence validation file changed before cleanup",
        "tmp_identity = _file_identity(os.fstat(handle.fileno()))",
        "errors.extend(_cleanup_validation_temp_output(path, tmp_identity))",
        "cleanup_errors = _cleanup_validation_temp_output(path, tmp_identity)",
    ),
    "scripts/kagemusha_recursive_compact_key_evidence.py": (
        "Build ABI-7 recursive compact key-artifact release evidence JSON",
        "DEFAULT_COMPACT_KEY_COMMAND",
        "COMPACT_KEY_REQUIRED_ARTIFACTS",
        "validate_compact_key_command",
        "validate_lineage_local_file",
        "_validate_generated_at_utc",
        "errors.extend(_validate_generated_at_utc(generated_at_utc))",
        "_validate_generated_at_future_skew",
        "_validate_generated_at_future_skew(\n            generated_at,\n            max_generated_at_future_skew_seconds,",
        "--generated-at-utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
        "--generated-at-utc must not be ahead of the helper clock skew allowance",
        "--max-generated-at-future-skew-seconds",
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
        "def validate_generator_log_path(",
        '_secret_path_error(str(generator_log_path), "--generator-log")',
        'artifact_dir_secret_error = _secret_path_error(str(artifact_dir), "--artifact-dir")',
        "generator_log_parent = generator_log_path.parent.resolve()",
        'readiness._validate_lineage_local_file_for_read(\n        generator_log_path,\n        "recursive compact key generator log",\n    )',
        "--generator-log must live directly under --artifact-dir",
        "artifact_size_bytes",
        'return None, None, None, [f"{label} must be non-empty"]',
        "readiness.validate_compact_key_artifact_prefix(artifact_prefix, artifact)",
        "def _sha256_text_file_with_size(",
        "chunks: list[bytes] = []",
        'except UnicodeDecodeError:\n        return None, None, None, [f"{label} could not be read"]',
        'text = b"".join(chunks).decode("utf-8")',
        "generator_log_path_was_explicit = generator_log_path is not None",
        'generator_log_secret_error = (\n        _secret_path_error(str(generator_log_path), "--generator-log")',
        "if generator_log_secret_error is not None:\n        errors.append(generator_log_secret_error)\n    else:\n        errors.extend(validate_artifact_dir_path(artifact_dir))",
        ") = _sha256_text_file_with_size(",
        "readiness.parse_compact_key_generator_log(generator_log_text)",
        "generator_log_sha256",
        "not a placeholder fixture",
        "validate_evidence_document",
        "check_compact_key_evidence",
        "require_canonical_filename=False",
        "must not contain control characters",
        "must not contain backslashes",
        "must be canonical",
        "Path(path).parts",
        'out_secret_error = _secret_path_error(str(out_path), "--out")',
        'artifact_dir_secret_error = _secret_path_error(str(artifact_dir), "--artifact-dir")',
        "pre_create_dir_errors = validate_artifact_dir_path(artifact_dir)",
        "artifact_dir.mkdir(mode=0o700, parents=True, exist_ok=True)",
        "post_create_dir_errors = validate_artifact_dir_path(artifact_dir)",
        "--artifact-dir could not be created for evidence validation",
        '    except ValueError:\n        return ["recursive compact key evidence validation file is not strict JSON"]\n',
        "len(evidence_text.encode(\"utf-8\")) > readiness.MAX_COMPACT_KEY_EVIDENCE_JSON_BYTES",
        "max_bytes=readiness.MAX_COMPACT_KEY_EVIDENCE_JSON_BYTES",
        'def validate_artifact_dir_path(artifact_dir: Path) -> list[str]:\n    """Reject artifact directories that could alias external release bytes."""\n\n    secret_error = _secret_path_error(str(artifact_dir), "--artifact-dir")\n    if secret_error is not None:\n        return [secret_error]\n',
        '    try:\n        artifact_dir_mode = artifact_dir.lstat().st_mode\n    except FileNotFoundError:\n        artifact_dir_mode = None\n    except OSError:\n        return ["--artifact-dir metadata could not be read"]\n',
        'secret_error = _secret_path_error(str(artifact_dir), "--artifact-dir")',
        'secret_error = _secret_path_error(str(path), label)',
        "validate_artifact_dir_path",
        "preflight_output_path",
        "validate_output_corridor",
        'path_errors.extend(preflight_output_path(out_path, "--out"))',
        '    if not parent_exists:\n        try:\n            parent.mkdir(mode=0o700, parents=True, exist_ok=True)\n        except OSError:\n            return [f"{label} parent directory could not be created"]\n',
        "def _set_private_directory_permissions",
        "os.fchmod(dir_fd, 0o700)",
        "stat.S_IMODE(directory_stat.st_mode) != 0o700",
        "permission_errors = _set_private_directory_permissions(",
        "os.fchmod(handle.fileno(), 0o600)",
        "stat.S_IMODE(expected_stat.st_mode) != 0o600",
        "--out permissions must be 0600",
        '    try:\n        output_mode = path.lstat().st_mode\n    except FileNotFoundError:\n        return []\n    except OSError:\n        return [f"{label} file metadata could not be read"]\n',
        '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        return [f"{label} hardlink metadata could not be read"]\n    if link_count > 1:\n        return [f"{label} must not be hardlinked"]\n',
        "validate_output_path",
        "write_errors = write_evidence(out_path, evidence)",
        """            allow_nan=False,
        ) + "\\n"
    except ValueError:
        return ["--out evidence is not strict JSON"]
""",
        "--out could not be written",
        "tempfile.NamedTemporaryFile(",
        "handle.flush()",
        "os.fsync(handle.fileno())",
        "os.replace(tmp_path, path)",
        "_cleanup_temp_output",
        "--out temporary file could not be removed",
        "tmp_identity = _file_identity(os.fstat(handle.fileno()))",
        "write_errors.extend(_cleanup_temp_output(tmp_path, tmp_identity))",
        "--out temporary file changed before cleanup",
        "_file_identity(temp_stat) != expected_identity",
        "os.unlink(path.name, dir_fd=parent_fd)",
        "--out parent directory could not be synced",
        "parent directory changed before sync",
        "def _file_identity",
        "def _directory_open_flags",
        "O_NOFOLLOW",
        "def _sync_output_parent",
        "os.fstat(parent_fd)",
        "expected_identity=parent_identity",
        "def _read_output_text(",
        "output_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        "output_open_identity = (open_stat.st_dev, open_stat.st_ino)",
        'f"{label} changed while being read"',
        'except OSError:\n        return None, [f"{label} write verification failed"]',
        'except UnicodeDecodeError:\n        return None, [f"{label} write verification failed"]',
        '    try:\n        expected_stat = path.lstat()\n    except (FileNotFoundError, OSError):\n        return ["--out write verification failed"]\n',
        """readback_text, readback_errors = _read_output_text(
        path,
        expected_stat,
        "--out",
        max_bytes=readiness.MAX_COMPACT_KEY_EVIDENCE_JSON_BYTES,
    )""",
        "readback_text != evidence_text",
        "--out write verification failed",
        '    errors = validate_output_path(path, "--out")\n    if errors:\n        return errors\n    try:\n        expected_stat = path.lstat()',
        "recursive compact key evidence validation file could not be written",
        "recursive compact key evidence validation file could not be removed",
        'errors = ["recursive compact key evidence validation file could not be written"]',
        "def _cleanup_validation_temp_output(",
        "validation_temp_stat = os.stat(",
        "_file_identity(validation_temp_stat) != expected_identity",
        "os.unlink(path.name, dir_fd=parent_fd)",
        "recursive compact key evidence validation file changed before cleanup",
        "tmp_identity = _file_identity(os.fstat(handle.fileno()))",
        "errors.extend(_cleanup_validation_temp_output(path, tmp_identity))",
        "cleanup_errors = _cleanup_validation_temp_output(path, tmp_identity)",
        "wrote evidence",
    ),
    "scripts/kagemusha_finalize_recursive_compact_key_staged_run.py": (
        "Finalize a completed staged ABI-7 recursive compact keygen run",
        "DEFAULT_TEMP_ROOT",
        "DEFAULT_EXIT_FILE",
        "DEFAULT_STAGED_ARTIFACT_DIR",
        "Path(\"/tmp\").resolve()",
        "RUN_REPORT_FILENAME",
        "STAGED_RUN_REPORT_SCHEMA",
        "EXECUTION_REPORT_FILENAME",
        "EXECUTION_REPORT_SCHEMA",
        "MAX_STAGED_RUN_REPORT_BYTES",
        "MAX_EXECUTION_REPORT_BYTES",
        "CONTROL_EXIT_MARKER_REDACTION",
        "SECRET_EXIT_MARKER_REDACTION",
        "def _secret_path_error",
        "def _set_private_directory_permissions",
        "os.fchmod(dir_fd, 0o700)",
        "stat.S_IMODE(directory_stat.st_mode) != 0o700",
        "destination.parent.mkdir(mode=0o700",
        "os.fchmod(dst.fileno(), 0o600)",
        "stat.S_IMODE(copied_stat.st_mode) != 0o600",
        "artifact_dir.mkdir(mode=0o700",
        "stage_dir.mkdir(mode=0o700",
        'if device_lab._contains_control_character(path_text):\n        return f"{label} must not contain control characters"',
        'if "\\\\" in path_text:\n        return f"{label} must not contain backslashes"',
        'if ".." in path.parts:\n        return f"{label} must be canonical"',
        "def _display_exit_marker",
        "device_lab._contains_control_character(marker)",
        "device_lab._display_path(exc.key)",
        "device_lab._display_path(extra_keys[0])",
        "def _validate_report_command",
        "command must be a non-empty string",
        "command must not contain surrounding whitespace",
        "command must not contain control characters",
        "command must not contain secret-looking material",
        "validate_exit_marker",
        "validate_staged_run_report",
        "validate_staged_execution_report",
        "expected_elapsed_seconds",
        "elapsed_seconds must match staged run report",
        "must be a non-zero SHA-256 hex digest",
        "generator_log_sha256 must match staged generator log SHA-256",
        "compact_evidence._validate_generated_at_utc(args.generated_at_utc)",
        "generated_at, timestamp_error = readiness.parse_utc_timestamp(",
        "compact_evidence._validate_generated_at_future_skew(\n            generated_at,\n            args.max_generated_at_future_skew_seconds,",
        "--max-generated-at-future-skew-seconds",
        "max_generated_at_future_skew_seconds: int",
        "max_generated_at_future_skew_seconds=max_generated_at_future_skew_seconds,",
        "text != \"0\\n\" and marker == \"0\"",
        "staged keygen exit marker must be exactly 0 followed by newline",
        "staged keygen exit code must be 0",
        "if exit_errors:",
        "return 1, None, errors",
        "exit_code must match staged keygen exit marker",
        "generator_log_size_bytes must match staged generator log",
        "stage_compact_key_evidence",
        "publish_stage",
        "already exists; refuse to overwrite without --replace",
        "_verify_published_file",
        "verify_errors = _verify_published_file(",
        "published {destination.name} does not match staged bytes",
        "def _unlink_file_if_identity(",
        "os.unlink(path.name, dir_fd=parent_fd)",
        "_file_identity(path_stat) == expected_identity",
        "def _cleanup_published_files(",
        "_cleanup_published_files(installed)",
        "rollback cleanup could not remove file",
        "cleanup_errors.extend(_cleanup_published_files(installed))",
        "return [*copy_errors, *cleanup_errors]",
        "destination_identity = _regular_file_identity(destination)",
        "def _sync_artifact_dir",
        "expected_identity=artifact_dir_identity",
        "artifact directory changed before sync",
        "def _cleanup_temp_parent(",
        "_file_identity(temp_parent_stat) != expected_identity",
        "shutil.rmtree(temp_parent.name, dir_fd=parent_fd)",
        "staged finalizer temporary directory could not be removed",
        "cleanup_errors = _cleanup_temp_parent(",
        "if finalizer_errors or cleanup_errors:",
        "check_compact_key_evidence(final_evidence_path)",
        "compact_evidence.build_evidence",
        "compact_evidence.validate_evidence_document",
        "compact_evidence.write_evidence",
    ),
    "scripts/kagemusha_finalize_lineage_proof_staged_run.py": (
        "Finalize a completed staged Reserved-lineage proof run",
        "DEFAULT_TEMP_ROOT",
        "DEFAULT_EXIT_FILE",
        "DEFAULT_STAGED_ARTIFACT_DIR",
        "Path(\"/tmp\").resolve()",
        "DEFAULT_ELAPSED_SECONDS_FILE",
        "RUN_REPORT_FILENAME",
        "STAGED_RUN_REPORT_SCHEMA",
        "EXECUTION_REPORT_SCHEMA",
        "LINEAGE_EXECUTION_REPORT_FILENAMES",
        "LINEAGE_KEY_ARTIFACT_LOG_FILENAMES",
        "LINEAGE_KEY_ARTIFACT_COMMANDS",
        "MAX_STAGED_RUN_REPORT_BYTES",
        "MAX_EXECUTION_REPORT_BYTES",
        "CONTROL_EXIT_MARKER_REDACTION",
        "SECRET_EXIT_MARKER_REDACTION",
        "CANONICAL_ELAPSED_SECONDS_RE",
        "def _secret_path_error",
        "def _set_private_directory_permissions",
        "os.fchmod(dir_fd, 0o700)",
        "stat.S_IMODE(directory_stat.st_mode) != 0o700",
        "destination.parent.mkdir(mode=0o700",
        "os.fchmod(dst.fileno(), 0o600)",
        "stat.S_IMODE(copied_stat.st_mode) != 0o600",
        "artifact_dir.mkdir(mode=0o700",
        "stage_dir.mkdir(mode=0o700",
        'if device_lab._contains_control_character(path_text):\n        return f"{label} must not contain control characters"',
        'if "\\\\" in path_text:\n        return f"{label} must not contain backslashes"',
        'if ".." in path.parts:\n        return f"{label} must be canonical"',
        "def _display_exit_marker",
        "def _parse_canonical_elapsed_seconds_file_text",
        "device_lab._contains_control_character(marker)",
        "device_lab._display_path(exc.key)",
        "device_lab._display_path(extra_keys[0])",
        "device_lab._display_path(unexpected_profiles[0])",
        "device_lab._display_path(entry_extra[0])",
        "def _validate_report_command",
        "command must be a non-empty string",
        "command must not contain surrounding whitespace",
        "command must not contain control characters",
        "command must not contain secret-looking material",
        "validate_exit_marker",
        "validate_staged_run_report",
        "validate_staged_execution_reports",
        "must be a non-zero SHA-256 hex digest",
        "log_sha256 must match staged log SHA-256",
        "lineage_evidence._validate_generated_at_utc(args.generated_at_utc)",
        "generated_at, timestamp_error = readiness.parse_utc_timestamp(",
        "lineage_evidence._validate_generated_at_future_skew(\n            generated_at,\n            args.max_generated_at_future_skew_seconds,",
        "--max-generated-at-future-skew-seconds",
        "max_generated_at_future_skew_seconds: int",
        "max_generated_at_future_skew_seconds=max_generated_at_future_skew_seconds,",
        "text != \"0\\n\" and marker == \"0\"",
        "staged lineage proof exit marker must be exactly 0 followed by newline",
        "staged lineage proof exit code must be 0",
        "positive finite decimal with six fractional digits followed by newline",
        "f\"{value:.6f}\\n\" != text",
        "if exit_errors:",
        "return 1, None, errors",
        "exit_code must match staged lineage proof exit marker",
        "elapsed_seconds must match staged elapsed seconds",
        "proof_log_size_bytes must match staged proof log",
        "lineage_key_artifact_logs",
        "lineage key artifact log size_bytes must match",
        "staged log size",
        "resolve_elapsed_seconds",
        "--elapsed-seconds-file",
        "--elapsed-seconds must match --elapsed-seconds-file",
        "stage_lineage_proof_evidence",
        "publish_stage",
        "already exists; refuse to overwrite without --replace",
        "_verify_published_file",
        "verify_errors = _verify_published_file(",
        "published {destination.name} does not match staged bytes",
        "def _unlink_file_if_identity(",
        "os.unlink(path.name, dir_fd=parent_fd)",
        "_file_identity(path_stat) == expected_identity",
        "def _cleanup_published_files(",
        "_cleanup_published_files(installed)",
        "rollback cleanup could not remove file",
        "cleanup_errors.extend(_cleanup_published_files(installed))",
        "return [*copy_errors, *cleanup_errors]",
        "destination_identity = _regular_file_identity(destination)",
        "def _sync_artifact_dir",
        "expected_identity=artifact_dir_identity",
        "artifact directory changed before sync",
        "def _cleanup_temp_parent(",
        "_file_identity(temp_parent_stat) != expected_identity",
        "shutil.rmtree(temp_parent.name, dir_fd=parent_fd)",
        "staged finalizer temporary directory could not be removed",
        "cleanup_errors = _cleanup_temp_parent(",
        "if finalizer_errors or cleanup_errors:",
        "check_lineage_proof_evidence(final_evidence_path)",
        "lineage_evidence.build_evidence",
        "lineage_evidence.validate_evidence_document",
        "lineage_evidence.write_evidence",
    ),
    "scripts/kagemusha_run_lineage_proof_staged.py": (
        "Run the Kagemusha Reserved-lineage production proof into a staging directory",
        "DEFAULT_TEMP_ROOT",
        "Path(\"/tmp\").resolve()",
        "LINEAGE_KEY_ARTIFACT_LOG_FILENAMES",
        "LINEAGE_KEY_ARTIFACT_COMMANDS",
        "iroha app zk kagemusha lineage-key-artifacts",
        "--profile init",
        "--profile append",
        "_staged_root_from_artifact_dir",
        "--staged-artifact-dir must end with artifacts/kagemusha",
        "DEFAULT_RECORD_ARCHIVE_PROOF_COMMAND",
        "PROOF_LOG_FILENAME",
        "RUN_REPORT_FILENAME",
        "STAGED_RUN_REPORT_SCHEMA",
        "EXECUTION_REPORT_SCHEMA",
        "LINEAGE_EXECUTION_REPORT_FILENAMES",
        "lineage-init-key-artifacts-execution.json",
        "lineage-append-key-artifacts-execution.json",
        "lineage-proof-execution.json",
        "LINEAGE_KEY_ARTIFACTS_BY_PROFILE",
        "MAX_EXECUTION_REPORT_BYTES",
        "STAGED_COMMAND_HEARTBEAT_SECONDS = 300.0",
        "def _secret_path_error",
        'if device_lab._contains_control_character(path_text):\n        return f"{label} must not contain control characters"',
        'if "\\\\" in path_text:\n        return f"{label} must not contain backslashes"',
        'if ".." in path.parts:\n        return f"{label} must be canonical"',
        "device_lab._display_path(exc.key)",
        "device_lab._display_path(extra_keys[0])",
        "def _validate_report_command",
        "command must be a non-empty string",
        "command must not contain surrounding whitespace",
        "command must not contain control characters",
        "command must not contain secret-looking material",
        "--resume-key-artifacts",
        "--replace and --resume-key-artifacts cannot be combined",
        "_validate_reusable_key_artifact_phase",
        "_try_resume_key_artifact_phase",
        "_cleanup_profile_for_resume",
        "validate_output_file_path",
        "staged lineage proof artifact",
        "staged lineage proof execution report",
        "already exists; refuse to overwrite without --replace",
        "def _file_identity",
        "def _directory_open_flags",
        "def _file_open_flags",
        "def _set_private_directory_permissions",
        "def _set_private_file_permissions",
        "O_NOFOLLOW",
        "os.fchmod(dir_fd, 0o700)",
        "stat.S_IMODE(directory_stat.st_mode) != 0o700",
        "os.fchmod(file_fd, 0o600)",
        "stat.S_IMODE(file_stat.st_mode) != 0o600",
        "def _sync_output_parent",
        "expected_identity=parent_identity",
        "log_parent_identity = _file_identity(log_parent_stat)",
        "expected_identity=log_parent_identity",
        "parent directory changed before sync",
        "def _regular_file_identity_for_unlink(",
        "def _unlink_file_if_identity(",
        "def _unlink_output_for_replace(",
        "def _cleanup_temp_output(",
        "_file_identity(path_stat) != expected_identity",
        "os.unlink(path.name, dir_fd=parent_fd)",
        "temporary output changed before cleanup",
        "tmp_identity = _file_identity(os.fstat(handle.fileno()))",
        "os.fchmod(handle.fileno(), 0o600)",
        "_unlink_output_for_replace(path, label)",
        "_verify_written_text_file",
        "return _verify_written_text_file(path, expected_bytes, label)",
        "stat.S_IMODE(expected_stat.st_mode) != 0o600",
        "changed after write",
        "data = handle.read(len(expected_bytes) + 1)",
        "_run_lineage_key_artifact_command",
        "_write_execution_report",
        "_write_run_report",
        "_wrapper_exit_status",
        "staged run exited with",
        "return _wrapper_exit_status(status)",
        "proof_log_size_bytes",
        "log_sha256",
        "lineage_evidence._sha256_file(",
        "log_sha256 must match staged {profile} lineage key artifact log SHA-256",
        "lineage_key_artifact_logs",
        "subprocess.Popen(",
        "stdout=log_handle",
        "stderr=subprocess.STDOUT",
        "process.wait(timeout=heartbeat_interval_seconds)",
        "except subprocess.TimeoutExpired:",
        "[kagemusha-staged-runner] lineage-proof heartbeat ",
        "os.fchmod(log_handle.fileno(), 0o600)",
        "os.fsync(log_handle.fileno())",
        "shlex.split(DEFAULT_RECORD_ARCHIVE_PROOF_COMMAND)",
        "readiness.validate_lineage_proof_command(",
        "args.staged_artifact_dir.mkdir(mode=0o700",
        "_write_exit_marker",
        "f\"{exit_code}\\n\"",
        "staged lineage proof exit marker",
        "elapsed_seconds",
        "_install_log_temp",
    ),
    "scripts/kagemusha_run_recursive_compact_keygen_staged.py": (
        "Run ABI-7 recursive compact key generation into a staging directory",
        "DEFAULT_TEMP_ROOT",
        "Path(\"/tmp\").resolve()",
        "DEFAULT_COMPACT_KEY_COMMAND",
        "GENERATOR_LOG_FILENAME",
        "RUN_REPORT_FILENAME",
        "STAGED_RUN_REPORT_SCHEMA",
        "EXECUTION_REPORT_FILENAME",
        "EXECUTION_REPORT_SCHEMA",
        "recursive-compact-key-execution.json",
        "MAX_EXECUTION_REPORT_BYTES",
        "MAX_RUN_REPORT_BYTES",
        "STAGED_COMMAND_HEARTBEAT_SECONDS = 300.0",
        "CONTROL_EXIT_MARKER_REDACTION",
        "SECRET_EXIT_MARKER_REDACTION",
        "def _secret_path_error",
        'if device_lab._contains_control_character(path_text):\n        return f"{label} must not contain control characters"',
        'if "\\\\" in path_text:\n        return f"{label} must not contain backslashes"',
        'if ".." in path.parts:\n        return f"{label} must be canonical"',
        "def _display_exit_marker",
        "device_lab._contains_control_character(marker)",
        "device_lab._display_path(exc.key)",
        "device_lab._display_path(extra_keys[0])",
        "def _validate_report_command",
        "command must be a non-empty string",
        "command must not contain surrounding whitespace",
        "command must not contain control characters",
        "command must not contain secret-looking material",
        "--resume-keygen",
        "--replace and --resume-keygen cannot be combined",
        "_validate_reusable_staged_keygen",
        "_validate_reusable_execution_report",
        "_validate_reusable_run_report",
        "_unlink_resume_outputs",
        "validate_output_file_path",
        "already exists; refuse to overwrite without --replace",
        "staged recursive compact key execution report",
        "def _file_identity",
        "def _directory_open_flags",
        "def _file_open_flags",
        "def _set_private_directory_permissions",
        "def _set_private_file_permissions",
        "O_NOFOLLOW",
        "os.fchmod(dir_fd, 0o700)",
        "stat.S_IMODE(directory_stat.st_mode) != 0o700",
        "os.fchmod(file_fd, 0o600)",
        "stat.S_IMODE(file_stat.st_mode) != 0o600",
        "def _sync_output_parent",
        "expected_identity=parent_identity",
        "log_parent_identity = _file_identity(log_parent_stat)",
        "expected_identity=log_parent_identity",
        "parent directory changed before sync",
        "def _regular_file_identity_for_unlink(",
        "def _unlink_file_if_identity(",
        "def _unlink_output_for_replace(",
        "def _cleanup_temp_output(",
        "_file_identity(path_stat) != expected_identity",
        "os.unlink(path.name, dir_fd=parent_fd)",
        "temporary output changed before cleanup",
        "tmp_identity = _file_identity(os.fstat(handle.fileno()))",
        "os.fchmod(handle.fileno(), 0o600)",
        "_unlink_output_for_replace(path, label)",
        "_verify_written_text_file",
        "return _verify_written_text_file(path, expected_bytes, label)",
        "stat.S_IMODE(expected_stat.st_mode) != 0o600",
        "changed after write",
        "data = handle.read(len(expected_bytes) + 1)",
        "_write_execution_report",
        "_write_run_report",
        "_wrapper_exit_status",
        "staged keygen exited with",
        "return _wrapper_exit_status(status)",
        "generator_log_size_bytes",
        "generator_log_sha256",
        "compact_evidence._sha256_file(",
        "generator_log_sha256 must match staged generator log SHA-256",
        "text != \"0\\n\" and stripped == \"0\"",
        "staged keygen exit marker must be exactly 0 followed by newline for resume",
        "subprocess.Popen(",
        "stdout=log_handle",
        "stderr=subprocess.STDOUT",
        "process.wait(timeout=heartbeat_interval_seconds)",
        "except subprocess.TimeoutExpired:",
        "[kagemusha-staged-runner] compact-keygen heartbeat ",
        "os.fchmod(log_handle.fileno(), 0o600)",
        "os.fsync(log_handle.fileno())",
        "shlex.split(DEFAULT_COMPACT_KEY_COMMAND)",
        "readiness.validate_compact_key_command(DEFAULT_COMPACT_KEY_COMMAND)",
        "args.staged_artifact_dir.mkdir(mode=0o700",
        'f"{exit_code}\\n"',
        "staged keygen exit marker",
        "--staged-artifact-dir must end with artifacts/kagemusha",
        "_install_log_temp",
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
        "kagemusha_release_summary_timestamp",
        "kagemusha_release_summary_future_dated",
        "kagemusha_release_summary_section_timestamp",
        "kagemusha_release_summary_section_future_dated",
        "ANDROID_SIGNED_EVIDENCE_SUMMARY_REQUIRED_FIELDS",
        "ANDROID_SIGNED_EVIDENCE_SUMMARY_PATH_FIELDS",
        "ANDROID_SIGNED_EVIDENCE_SUMMARY_SHA256_FIELDS",
        "ANDROID_SIGNED_EVIDENCE_SUMMARY_IDENTITY_FIELDS",
        "device_lab.infer_kagemusha_device_family",
        "kagemusha_release_summary_android_signed_evidence_identity",
        "Android signed-evidence summary model/codename must match device family",
        "and device_model\n            and device_codename",
        "kagemusha_release_summary_android_slots_device_identity",
        "Android readiness summary Kagemusha slot model/codename must match device family",
        "or not value\n                        or value != value.strip()",
        "--repo-root",
        "--verify-existing",
        "package_aware_multi_hop_composed",
        "production_width_proof_passed",
        "compact_key_artifacts_validated",
        "from collections.abc import Mapping",
        "def build_release_bundle(",
        "def verify_release_bundle(",
        "def _safe_trusted_signer_public_key_sha256(",
        "device_lab._trusted_signer_public_key_sha256_set(",
        "def _blocked_release_bundle_manifest(",
        "signer_map_blockers = [",
        "device_lab.validate_trusted_signer_public_key_map(",
        "repo_root_blockers = readiness.validate_repo_root_path(repo_root)",
        "_blocked_release_bundle_manifest(",
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
        "_secret_path_error",
        'return _blocker(code, f"{label} must not contain control characters")',
        "_contains_secret_string",
        "_contains_control_string",
        "_check_ready_summary_shape",
        "_check_android_ready_summary_shape",
        "_check_android_signed_evidence_summary_shape",
        "_check_android_trusted_signer_binding",
        "_compare_android_slots_summary",
        "kagemusha_release_summary_secret_material",
        "kagemusha_release_summary_control_character",
        "kagemusha_release_summary_unexpected_field",
        "kagemusha_release_summary_missing_field",
        "kagemusha_release_summary_schema_shape",
        "kagemusha_release_summary_schema",
        "kagemusha_release_summary_section_shape",
        "kagemusha_release_summary_unexpected_section_field",
        "kagemusha_release_summary_section_missing_field",
        "kagemusha_release_summary_ready_shape",
        "kagemusha_release_summary_status_shape",
        "kagemusha_release_summary_blockers_shape",
        "kagemusha_release_summary_section_ok_shape",
        "kagemusha_release_summary_section_state_shape",
        "kagemusha_release_summary_section_string",
        "kagemusha_release_summary_section_boolean",
        "kagemusha_release_summary_section_object",
        "kagemusha_release_summary_section_list",
        "kagemusha_release_summary_section_inventory",
        "kagemusha_release_summary_section_sha256",
        "kagemusha_release_summary_section_size",
        "kagemusha_release_summary_section_integer_map",
        "kagemusha_release_summary_section_string_map",
        "kagemusha_release_summary_section_integer",
        "kagemusha_release_summary_section_blockers_shape",
        "kagemusha_release_summary_section_blockers_present",
        "kagemusha_release_summary_android_signed_evidence_shape",
        "kagemusha_release_summary_android_signed_evidence_slot",
        "kagemusha_release_summary_android_signed_evidence_unexpected_field",
        "kagemusha_release_summary_android_signed_evidence_missing_field",
        "kagemusha_release_summary_android_slots_shape",
        "kagemusha_release_summary_android_slots_slot",
        "kagemusha_release_summary_android_slots_status",
        "kagemusha_release_summary_android_slots_inventory",
        "kagemusha_release_summary_android_slots_unexpected_field",
        "kagemusha_release_summary_android_slots_kagemusha_unexpected_field",
        "kagemusha_release_summary_android_slots_errors",
        "kagemusha_release_summary_android_slots_present",
        "kagemusha_release_summary_android_slots_file_counts",
        "kagemusha_release_summary_android_slots_missing_field",
        "kagemusha_release_summary_android_slots_value",
        "kagemusha_release_summary_android_slots_device_family",
        "kagemusha_release_summary_android_slots_device_family_inventory",
        "kagemusha_release_summary_android_slots_abi",
        "kagemusha_release_summary_android_slots_timestamp",
        "kagemusha_release_summary_android_slots_future_dated",
        "kagemusha_release_summary_android_slots_sha256",
        "kagemusha_release_summary_android_slots_path",
        "kagemusha_release_summary_android_slots_binding",
        "kagemusha_release_summary_android_slots_drift",
        "kagemusha_release_summary_android_signed_evidence_value",
        "kagemusha_release_summary_android_signed_evidence_timestamp",
        "kagemusha_release_summary_android_signed_evidence_future_dated",
        "kagemusha_release_summary_android_signed_evidence_sha256",
        "kagemusha_release_summary_android_signed_evidence_path",
        "kagemusha_release_summary_android_root",
        "kagemusha_release_summary_android_list_shape",
        "kagemusha_release_summary_android_device_families",
        "Android readiness summary covered_device_families must exactly match the standard matrix",
        "kagemusha_release_summary_android_signer_sha256",
        "kagemusha_release_summary_android_signer_binding",
        "Android readiness summary trusted signer digests must be unique sorted non-zero lowercase sha256 hex strings",
        "_compare_android_signed_evidence_summary",
        "kagemusha_release_summary_android_signed_evidence_inventory_drift",
        "kagemusha_release_summary_android_signed_evidence_identity_drift",
        "kagemusha_release_summary_android_signed_evidence_drift",
        "kagemusha_release_summary_android_signed_bounds_drift",
        "kagemusha_release_summary_android_slots_identity_drift",
        "_compare_validated_sections",
        "blockers.extend(\n            _compare_validated_sections(\n                summary,\n                abi6,\n                abi7,",
        "kagemusha_release_summary_drift",
        "_compare_section_evidence_fields",
        "kagemusha_release_summary_section_evidence_drift",
        "_compare_section_value_fields",
        "kagemusha_release_summary_section_value_drift",
        '"circuit_ids",\n                "artifact_count",\n                "tests",',
        '"record_namespace",\n                "record_version",\n                "command_validated",',
        "_compare_android_summary_binding",
        "kagemusha_release_summary_android_device_families_drift",
        "kagemusha_release_summary_android_duplicate_bindings_drift",
        "kagemusha_release_summary_android_trusted_signer_drift",
        "_read_local_json_text",
        "_validate_local_file_for_read",
        "MAX_RELEASE_BUNDLE_LOCAL_JSON_BYTES",
        "MAX_RELEASE_BUNDLE_OUTPUT_JSON_BYTES",
        'text, read_blockers = _read_local_json_text(',
        "release_json_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        "release_json_open_identity = (open_stat.st_dev, open_stat.st_ino)",
        "if open_stat.st_size > MAX_RELEASE_BUNDLE_LOCAL_JSON_BYTES:",
        "if size > MAX_RELEASE_BUNDLE_LOCAL_JSON_BYTES:",
        'f"{label} must be no more than {MAX_RELEASE_BUNDLE_LOCAL_JSON_BYTES} bytes"',
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
        "kagemusha_release_bundle_manifest_control_character",
        "kagemusha_release_bundle_manifest_future_dated",
        "RELEASE_BUNDLE_ALLOWED_EVIDENCE_KEYS",
        "RELEASE_BUNDLE_SINGLE_EVIDENCE_KEYS",
        "RELEASE_BUNDLE_MAP_EVIDENCE_KEYS",
        "RELEASE_BUNDLE_EVIDENCE_ENTRY_FIELDS",
        "RELEASE_BUNDLE_ALLOWED_SECTION_KEYS",
        "RELEASE_BUNDLE_ALLOWED_ANDROID_SECTION_KEYS",
        "ANDROID_DUPLICATE_BINDING_SUMMARY_FIELDS",
        "ANDROID_DUPLICATE_BINDING_ENTRY_FIELDS",
        "_check_release_bundle_evidence_inventory_shape",
        "_check_release_bundle_evidence_entry_shape",
        "_check_release_bundle_evidence_paths",
        "_expected_release_bundle_section_map_keys",
        "expected_checked_files = list(",
        "expected_abi6_limits = {",
        "expected_abi6_modes = {",
        "kagemusha_release_bundle_manifest_section_value",
        "_check_release_bundle_section_shapes",
        "_check_release_bundle_android_section_shape",
        "_check_release_bundle_cross_section_shape",
        "evidence = bundle.get(\"evidence\")",
        "blockers.extend(_check_release_bundle_evidence_inventory_shape(evidence))",
        "blockers.extend(_check_release_bundle_evidence_paths(evidence))",
        "blockers.extend(_check_release_bundle_section_shapes(bundle))",
        "blockers.extend(_check_release_bundle_android_section_shape(bundle))",
        "blockers.extend(_check_release_bundle_cross_section_shape(bundle))",
        "kagemusha_release_bundle_manifest_evidence_shape",
        "kagemusha_release_bundle_manifest_evidence_entry_shape",
        "kagemusha_release_bundle_manifest_evidence_unexpected_field",
        "kagemusha_release_bundle_manifest_evidence_missing_field",
        "kagemusha_release_bundle_manifest_evidence_inventory_shape",
        "kagemusha_release_bundle_manifest_evidence_artifact_kind",
        "kagemusha_release_bundle_manifest_evidence_inventory_keys",
        "kagemusha_release_bundle_manifest_evidence_inventory_item",
        "kagemusha_release_bundle_manifest_evidence_slot",
        "kagemusha_release_bundle_manifest_evidence_path",
        "kagemusha_release_bundle_manifest_evidence_sha256",
        "kagemusha_release_bundle_manifest_evidence_size",
        "kagemusha_release_bundle_manifest_top_level_evidence_path",
        "_check_release_bundle_expected_top_level_evidence_binding",
        '"compact_key_evidence",\n        "compact_key_generator_log",',
        "kagemusha_release_bundle_manifest_top_level_evidence_binding",
        "_check_release_bundle_expected_android_summary_binding",
        '"signed_evidence",\n        "trusted_signer_public_key_sha256",',
        "kagemusha_release_bundle_manifest_android_summary_binding",
        "kagemusha_release_bundle_manifest_android_signed_evidence_identity_binding",
        "_check_release_bundle_expected_android_evidence_binding",
        "kagemusha_release_bundle_manifest_android_signed_evidence_binding",
        "kagemusha_release_bundle_manifest_android_slot_artifact_binding",
        "_check_release_bundle_expected_section_value_binding",
        "kagemusha_release_bundle_manifest_section_value_binding",
        "_check_release_bundle_single_section_evidence_binding",
        "_check_release_bundle_section_evidence_map_binding",
        "_check_release_bundle_section_log_binding",
        "kagemusha_release_bundle_manifest_section_evidence_binding",
        "_check_release_bundle_expected_compact_generator_log_artifact_binding",
        "kagemusha_release_bundle_manifest_compact_generator_log_artifact_binding",
        "kagemusha_release_bundle_manifest_missing_field",
        "kagemusha_release_bundle_manifest_schema_shape",
        "kagemusha_release_bundle_manifest_schema",
        "kagemusha_release_bundle_manifest_ready_shape",
        "kagemusha_release_bundle_manifest_not_ready",
        "kagemusha_release_bundle_manifest_blockers_shape",
        "kagemusha_release_bundle_manifest_blockers_present",
        "kagemusha_release_bundle_manifest_section_shape",
        "kagemusha_release_bundle_manifest_section_unexpected_field",
        "kagemusha_release_bundle_manifest_section_missing_field",
        "kagemusha_release_bundle_manifest_section_state_shape",
        "kagemusha_release_bundle_manifest_section_state",
        "kagemusha_release_bundle_manifest_section_timestamp",
        "kagemusha_release_bundle_manifest_section_future_dated",
        "kagemusha_release_bundle_manifest_section_sha256",
        "kagemusha_release_bundle_manifest_section_size",
        "kagemusha_release_bundle_manifest_android_device_families",
        "kagemusha_release_bundle_manifest_android_signer_sha256",
        "kagemusha_release_bundle_manifest_android_root",
        "kagemusha_release_bundle_manifest_android_signer_binding",
        "kagemusha_release_summary_android_duplicate_bindings_slots",
        "kagemusha_release_summary_android_duplicate_bindings_slot_binding",
        "kagemusha_release_summary_android_duplicate_bindings_value_binding",
        "kagemusha_release_summary_android_duplicate_bindings_value_inventory",
        "duplicate_value_sha256_by_value",
        "valid_value_sha256s != sorted(set(valid_value_sha256s))",
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
        'path_secret = _secret_path_error(',
        'root_secret = _secret_path_error(',
        "def _bundle_path_shape_error(path: Path, label: str)",
        "def _bundle_root_shape_error(root: Path)",
        "must be a canonical path under --bundle-root",
        "--bundle-root must be a canonical directory path",
        "must not contain backslashes",
        "--bundle-root must not contain backslashes",
        "root_shape = _bundle_root_shape_error(bundle_root)",
        "relative, relative_blockers = _relative_to_bundle(path, bundle_root, label)\n    if relative_blockers:\n        return None, relative_blockers\n    assert relative is not None\n    digest, digest_blockers = _sha256_file(",
        "relative, relative_blockers = _relative_to_bundle(path, bundle_root, label)\n    if relative_blockers:\n        return None, relative_blockers\n    assert relative is not None\n    digest, size, digest_blockers = _sha256_file_with_size(",
        "kagemusha_release_bundle_path_invalid",
        "must stay under --bundle-root",
        "_validate_output_parent_path",
        "_validate_output_path",
        "--bundle-root must not be a symlink",
        "--out parent directory must not be a symlink",
        "--out must not be a symlink",
        "--out must not be hardlinked",
        "tempfile.NamedTemporaryFile",
        "os.fchmod(handle.fileno(), 0o600)",
        "os.fsync(handle.fileno())",
        "os.replace(tmp_path, path)",
        "_cleanup_temp_output",
        "--out temporary file could not be removed",
        "tmp_identity = _file_identity(os.fstat(handle.fileno()))",
        "write_blockers.extend(_cleanup_temp_output(tmp_path, tmp_identity))",
        "--out temporary file changed before cleanup",
        "_file_identity(temp_stat) != expected_identity",
        "os.unlink(path.name, dir_fd=parent_fd)",
        "_read_output_text",
        "--out parent directory could not be synced",
        "--out parent directory changed before sync",
        "def _file_identity",
        "def _directory_open_flags",
        "O_NOFOLLOW",
        "def _sync_output_parent",
        "os.fstat(parent_fd)",
        "expected_identity=parent_identity",
        "output_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
        "output_open_identity = (open_stat.st_dev, open_stat.st_ino)",
        "--out changed while being read",
        "stat.S_IMODE(open_stat.st_mode) != 0o600",
        "--out permissions must be 0600",
        "if open_stat.st_size > MAX_RELEASE_BUNDLE_OUTPUT_JSON_BYTES:",
        "if size > MAX_RELEASE_BUNDLE_OUTPUT_JSON_BYTES:",
        'f"--out must be no more than {MAX_RELEASE_BUNDLE_OUTPUT_JSON_BYTES} bytes"',
        "readback, readback_blockers = _read_output_text(path, expected_stat)",
        '    try:\n        expected_stat = path.lstat()\n    except (FileNotFoundError, OSError):\n        return [\n            _release_bundle_out_blocker("--out could not be read back after writing")\n        ]\n',
        "_sync_output_parent(path.parent, expected_identity=parent_identity)",
        "os.fsync(parent_fd)",
        '    except OSError:\n        return None, [\n            _release_bundle_out_blocker("--out could not be read back after writing")\n        ]\n',
        "--out readback did not match the generated manifest",
        "allow_nan=False",
        "release bundle manifest is not strict JSON",
        'if len(manifest_text.encode("utf-8")) > MAX_RELEASE_BUNDLE_OUTPUT_JSON_BYTES:',
        "check_lineage_proof_evidence",
        "check_compact_key_evidence",
        "check_android_device_lab",
        "write_release_bundle",
        "[kagemusha-release-bundle] ready",
    ),
    "scripts/tests/check_android_device_lab_slot_test.py": (
        "test_checked_in_sample_slot_passes_default_validation",
        "test_kagemusha_slot_assembler_builds_signed_production_slot",
        "test_kagemusha_slot_assembler_installs_private_permissions",
        "test_kagemusha_slot_metadata_rejects_missing_source_policy_digest",
        "test_kagemusha_slot_assembler_requires_signing_by_default",
        "test_kagemusha_slot_assembler_rejects_control_root_before_classify",
        "test_kagemusha_slot_assembler_rejects_control_source_metadata_string",
        "test_kagemusha_slot_assembler_rejects_noncanonical_slot_id_before_path_join",
        "test_kagemusha_slot_assembler_rejects_backslash_slot_id_before_path_join",
        "test_kagemusha_slot_assembler_rejects_report_device_mismatch_before_install",
        "test_kagemusha_slot_assembler_rejects_report_app_package_mismatch_before_publish",
        "test_kagemusha_slot_assembler_rejects_unexpected_attestation_source_fields_before_publish",
        "test_kagemusha_slot_assembler_rejects_bad_attestation_report_metadata_before_publish",
        "test_kagemusha_slot_assembler_rejects_unexpected_transcript_source_fields_before_publish",
        "test_kagemusha_slot_assembler_rejects_transcript_schema_mismatch_before_publish",
        "test_kagemusha_slot_assembler_rejects_d2d_transcript_semantic_mismatch_before_publish",
        "test_kagemusha_slot_assembler_rejects_wallet_transcript_semantic_mismatch_before_publish",
        "test_kagemusha_slot_assembler_rejects_malformed_required_runtime_artifacts_before_publish",
        "test_kagemusha_slot_assembler_rejects_symlinked_source_ancestor",
        "test_kagemusha_slot_assembler_rejects_source_swap_after_preflight",
        "test_kagemusha_slot_assembler_copy_rejects_control_source_path_before_copy",
        "test_kagemusha_slot_assembler_requires_attestation_harness_result",
        "test_kagemusha_slot_assembler_rejects_harness_challenge_mismatch",
        "test_kagemusha_slot_assembler_rejects_blank_source_challenge_before_unsigned_publish",
        "test_kagemusha_slot_assembler_rejects_noncanonical_source_policy_before_unsigned_publish",
        "test_kagemusha_slot_assembler_rejects_report_level_mismatch_before_publish",
        "test_kagemusha_slot_assembler_rejects_report_status_mismatch_before_publish",
        "test_kagemusha_slot_assembler_rejects_passed_attestation_status_before_publish",
        "test_kagemusha_slot_assembler_rejects_blank_identity_override_without_adb",
        "test_kagemusha_slot_assembler_rejects_padded_adb_identity",
        "test_kagemusha_slot_assembler_rejects_noncanonical_adb_identity_output",
        "test_kagemusha_slot_assembler_uses_source_identity_without_adb",
        "test_kagemusha_slot_assembler_rejects_bad_source_identity_without_adb",
        "test_kagemusha_slot_assembler_rejects_blank_source_identity_without_adb",
        "test_kagemusha_slot_assembler_rejects_conflicting_source_identity_without_adb",
        "test_kagemusha_slot_assembler_rejects_override_source_identity_mismatch_without_adb",
        "test_kagemusha_slot_assembler_json_write_rejects_parent_identity_swap",
        "test_kagemusha_slot_assembler_json_write_reports_temp_cleanup_failure",
        "test_kagemusha_slot_assembler_json_temp_cleanup_preserves_swapped_file",
        "test_kagemusha_slot_assembler_json_write_verifies_installed_bytes",
        "test_kagemusha_slot_assembler_cleanup_reports_temp_parent_failure",
        "test_kagemusha_slot_assembler_reports_temp_parent_cleanup_failure",
        "test_kagemusha_slot_assembler_rejects_alias_root_before_classify",
        "test_kagemusha_slot_assembler_source_path_validators_reject_aliases_before_metadata",
        "test_kagemusha_attestation_report_writer_temp_cleanup_rejects_swap",
        "test_kagemusha_attestation_report_writer_installs_private_permissions",
        "test_kagemusha_attestation_report_writer_rejects_control_chain_source_path_before_ancestor_check",
        "test_kagemusha_attestation_report_writer_rejects_alias_chain_source_path_before_metadata",
        "test_kagemusha_attestation_report_writer_rejects_alias_harness_result_path_before_metadata",
        "test_kagemusha_attestation_report_writer_rejects_secret_harness_result_path_without_leak",
        "test_kagemusha_attestation_report_writer_rejects_noncanonical_slot_id",
        "test_kagemusha_attestation_report_writer_rejects_backslash_slot_id",
        "test_kagemusha_attestation_report_writer_rejects_noncanonical_chain_path",
        "test_kagemusha_attestation_report_writer_rejects_backslash_chain_path",
        "test_scan_slot_rejects_padded_sha256sum_line",
        "test_scan_slot_rejects_zero_sha256sum_digest",
        "test_scan_slot_rejects_star_normalized_sha256sum_path",
        "test_scan_slot_rejects_noncanonical_sha256sum_path",
        "test_normalise_safe_relative_path_rejects_control_before_strip",
        "test_normalise_safe_relative_path_rejects_surrounding_whitespace",
        "test_normalise_safe_relative_path_rejects_noncanonical_aliases",
        "test_production_metadata_rejects_star_normalized_signed_evidence_path",
        "test_production_metadata_rejects_noncanonical_signed_evidence_path",
        "test_kagemusha_android_raw_puller_reads_latest_and_installs_slot",
        "test_kagemusha_android_raw_puller_rejects_noncanonical_slot_id_before_adb",
        "test_kagemusha_android_raw_puller_rejects_control_out_root_before_adb",
        "test_kagemusha_android_raw_puller_rejects_alias_cli_paths_before_adb",
        "test_kagemusha_android_raw_puller_rejects_control_summary_out_before_adb",
        "test_kagemusha_android_raw_puller_rejects_control_raw_slot_path_before_stat",
        "test_kagemusha_android_raw_puller_rejects_alias_raw_slot_path_before_stat",
        "test_kagemusha_android_raw_puller_redacts_control_raw_artifact_path",
        "test_kagemusha_android_raw_puller_rejects_control_tar_member_path_before_normalise",
        "test_kagemusha_android_raw_puller_install_refuses_late_existing_slot",
        "test_kagemusha_android_raw_puller_install_rejects_unexpected_top_level_entry",
        "test_kagemusha_android_raw_puller_install_syncs_directories_and_cleans_failure",
        "test_kagemusha_android_raw_puller_install_rejects_destination_identity_swap",
        "test_kagemusha_android_raw_puller_install_rejects_output_root_identity_swap",
        "test_kagemusha_android_raw_puller_install_rejects_parent_identity_before_slot_stat",
        "test_kagemusha_android_raw_puller_install_cleanup_preserves_swapped_destination",
        "test_kagemusha_android_raw_puller_install_cleanup_uses_parent_dir_fd",
        "test_kagemusha_android_raw_puller_install_cleanup_reports_failure",
        "test_kagemusha_android_raw_puller_install_moves_with_directory_fds",
        "test_kagemusha_android_raw_puller_temp_cleanup_removes_original_parent",
        "test_kagemusha_android_raw_puller_temp_cleanup_reports_failure",
        "test_kagemusha_android_raw_puller_temp_cleanup_preserves_swapped_parent",
        "test_kagemusha_android_raw_puller_reports_temp_cleanup_failure",
        "test_kagemusha_android_raw_puller_install_sync_rejects_identity_mismatch",
        "test_kagemusha_android_raw_puller_latest_writer_syncs_parent_identity",
        "test_kagemusha_android_raw_puller_latest_writer_installs_private_permissions",
        "test_kagemusha_android_raw_puller_latest_writer_rejects_symlink_after_replace",
        "test_kagemusha_android_raw_puller_latest_writer_rejects_hardlink_after_replace",
        "test_kagemusha_android_raw_puller_latest_writer_rejects_permissive_mode_after_replace",
        "test_kagemusha_android_raw_puller_latest_writer_rejects_readback_path_swap",
        "test_kagemusha_android_raw_puller_latest_writer_reports_temp_cleanup_failure",
        "test_kagemusha_android_raw_puller_latest_writer_temp_cleanup_rejects_swap",
        "test_kagemusha_android_raw_puller_summary_rejects_nonfinite_json_before_tempfile",
        "test_kagemusha_android_raw_puller_summary_rejects_oversized_json_before_tempfile",
        "test_kagemusha_android_raw_puller_summary_installs_private_permissions",
        "test_kagemusha_android_raw_puller_summary_rejects_symlink_after_replace",
        "test_kagemusha_android_raw_puller_summary_rejects_hardlink_after_replace",
        "test_kagemusha_android_raw_puller_summary_rejects_permissive_mode_after_replace",
        "test_kagemusha_android_raw_puller_summary_rejects_readback_path_swap",
        "test_kagemusha_android_raw_puller_summary_reports_temp_cleanup_failure",
        "test_kagemusha_android_raw_puller_summary_temp_cleanup_rejects_swap",
        "test_kagemusha_android_raw_puller_summary_sync_rejects_parent_identity_swap",
        "test_kagemusha_android_raw_puller_summary_digest_rejects_symlinked_artifact",
        "test_kagemusha_android_raw_puller_summary_digest_rejects_hardlinked_artifact",
        "test_kagemusha_android_raw_puller_requires_harness_result",
        "test_kagemusha_android_raw_puller_rejects_harness_challenge_mismatch",
        "test_kagemusha_android_raw_puller_refuses_existing_slot_before_adb_tar",
        "test_kagemusha_android_raw_puller_rejects_tar_path_traversal",
        "test_kagemusha_android_raw_puller_rejects_compressed_tar_stream",
        "test_kagemusha_android_raw_puller_rejects_noncanonical_tar_member_path",
        "test_kagemusha_android_raw_puller_allows_trailing_slash_directory_members",
        "test_kagemusha_android_raw_puller_rejects_tar_symlink_member",
        "test_kagemusha_android_raw_puller_rejects_tar_hardlink_member",
        "test_kagemusha_android_raw_puller_rejects_unexpected_raw_artifact",
        "test_kagemusha_android_raw_puller_rejects_oversized_tar_member",
        "test_kagemusha_android_raw_puller_rejects_latest_slot_mismatch",
        "test_kagemusha_android_raw_puller_rejects_noncanonical_latest_slot_query",
        "test_kagemusha_android_raw_puller_rejects_noncanonical_latest_slot",
        "test_kagemusha_android_raw_puller_rejects_noncanonical_challenge_file",
        "test_kagemusha_android_raw_puller_requires_challenge_file_newline",
        "test_kagemusha_android_raw_puller_rejects_tar_file_parent_collision",
        "test_kagemusha_android_raw_puller_rejects_tar_directory_collision",
        "test_kagemusha_android_raw_puller_requires_result_slot_field",
        "test_kagemusha_android_raw_puller_requires_result_chain_digest",
        "test_kagemusha_android_raw_puller_requires_result_challenge_digest",
        "test_kagemusha_android_raw_puller_rejects_result_extra_field",
        "test_kagemusha_android_raw_puller_requires_result_identity_strings",
        "test_kagemusha_android_raw_puller_rejects_control_result_identity_strings",
        "test_kagemusha_android_raw_puller_requires_result_sdk_digests",
        "test_kagemusha_android_raw_puller_rejects_zero_result_digests",
        "test_kagemusha_android_raw_puller_requires_result_strongbox_levels",
        "test_kagemusha_android_raw_puller_rejects_queue_slot_mismatch",
        "test_kagemusha_android_raw_puller_rejects_queue_extra_field",
        "test_kagemusha_android_raw_puller_rejects_nonempty_pending_queue",
        "test_kagemusha_android_raw_puller_rejects_telemetry_slot_mismatch",
        "test_kagemusha_android_raw_puller_rejects_whitespace_normalized_telemetry_slot",
        "test_kagemusha_android_raw_puller_rejects_telemetry_extra_field",
        "test_kagemusha_android_raw_puller_rejects_noncanonical_telemetry_identity_strings",
        "test_kagemusha_android_raw_puller_rejects_telemetry_app_package_mismatch",
        "test_kagemusha_android_raw_puller_rejects_noncanonical_json_slot_bindings",
        "test_kagemusha_android_raw_puller_rejects_noncanonical_telemetry_suite",
        "test_kagemusha_android_raw_puller_rejects_d2d_online_handoff",
        "test_kagemusha_android_raw_puller_rejects_wallet_rollback_failure",
        "test_kagemusha_android_raw_puller_rejects_failed_status_ndjson",
        "test_kagemusha_android_raw_puller_rejects_status_ndjson_unexpected_field",
        "test_kagemusha_android_raw_puller_rejects_unknown_status_ndjson",
        "test_kagemusha_android_raw_puller_requires_status_slot_id",
        "test_kagemusha_android_raw_puller_rejects_noncanonical_status_ndjson",
        "test_kagemusha_android_raw_puller_rejects_status_slot_mismatch",
        "test_kagemusha_android_raw_puller_rejects_noncanonical_status_slot_binding",
        "test_kagemusha_android_raw_puller_rejects_runtime_failure_marker",
        "test_kagemusha_android_raw_puller_rejects_noncanonical_harness_strings",
        "test_kagemusha_android_raw_puller_rejects_control_harness_strings",
        "test_kagemusha_android_raw_puller_rejects_harness_chain_length_mismatch",
        "test_kagemusha_android_raw_puller_rejects_malformed_harness_result",
        "test_scan_slot_rejects_missing_attestation_harness_result",
        "test_scan_slot_rejects_attestation_harness_challenge_mismatch",
        "test_scan_slot_rejects_control_attestation_harness_strings",
        "test_scan_slot_rejects_sha256_drift",
        "test_scan_slot_rejects_noncanonical_sha256sum_path",
        "test_production_metadata_rejects_whitespace_normalized_signed_evidence_path",
        "test_production_metadata_rejects_noncanonical_signed_evidence_path",
        "test_production_metadata_rejects_control_signed_evidence_path",
        "test_production_metadata_rejects_whitespace_normalized_signed_evidence_digest",
        "test_explicit_missing_slot_returns_structured_error",
        "test_validate_slot_ids_rejects_duplicate_explicit_slots",
        "test_validate_slot_ids_rejects_noncanonical_slot_aliases",
        "test_explicit_duplicate_slot_id_rejected_before_scan",
        "test_discover_slots_returns_structured_error_on_root_list_failure",
        "test_discover_slots_uses_lstat_before_is_dir_preflight",
        "test_discover_slots_reports_slot_metadata_failure_before_is_dir_preflight",
        "test_discover_slots_preserves_symlinked_slot_for_scan_slot_rejection",
        "test_discover_slots_returns_stable_sorted_order",
        "test_discover_slots_revalidates_explicit_slot_ids_directly",
        "test_main_rejects_device_lab_root_list_failure_without_traceback",
        "test_explicit_unsafe_slot_id_rejected_before_path_join",
        "test_explicit_noncanonical_slot_id_rejected_before_path_join",
        "test_explicit_slot_id_rejects_surrounding_whitespace_before_path_join",
        "test_explicit_slot_id_rejects_newline_before_path_join",
        "test_explicit_slot_id_rejects_internal_whitespace_before_path_join",
        "test_explicit_slot_id_rejects_control_character_before_path_join",
        "test_explicit_secret_looking_slot_id_is_not_echoed",
        "test_discovered_secret_looking_slot_directory_is_not_echoed",
        "test_discovered_whitespace_slot_directory_is_rejected_before_metadata",
        "test_discovered_control_slot_directory_is_rejected_without_echo",
        "test_discovered_backslash_slot_directory_is_rejected_before_metadata",
        "test_scan_slot_rejects_control_slot_directory_before_metadata",
        "test_scan_slot_rejects_backslash_slot_directory_before_metadata",
        "test_scan_slot_rejects_newline_slot_directory_before_metadata",
        "test_scan_slot_redacts_secret_looking_manifest_paths",
        "test_scan_slot_rejects_slot_directory_metadata_failure",
        "test_scan_slot_rejects_slot_parent_metadata_failure",
        "test_slot_files_missing_slot_returns_empty_without_traceback",
        "test_slot_root_entries_returns_stable_sorted_order",
        "test_slot_files_non_directory_root_returns_empty_without_traceback",
        "test_slot_files_reports_slot_metadata_failure_without_omission",
        "test_slot_files_secret_slot_path_returns_empty_without_traversal",
        "test_slot_files_rejects_alias_slot_path_before_metadata",
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
        "test_load_json_rejects_nonfinite_json_constant",
        "test_load_json_rejects_oversized_json_before_parse",
        "test_load_json_rejects_non_utf8_bytes_without_traceback",
        "test_parse_sha256_manifest_rejects_secret_slot_path_directly_before_parse",
        "test_parse_sha256_manifest_rejects_control_slot_path_directly_before_parse",
        "test_parse_sha256_manifest_rejects_alias_slot_path_before_metadata",
        "test_parse_sha256_manifest_rejects_symlinked_slot_root_directly_before_parse",
        "test_parse_sha256_manifest_rejects_slot_metadata_failure_before_parse",
        "test_parse_sha256_manifest_rejects_symlinked_slot_ancestor_before_parse",
        "test_parse_sha256_manifest_rejects_hardlinked_manifest_before_read",
        "test_parse_sha256_manifest_rejects_file_metadata_failure_before_read",
        "test_parse_sha256_manifest_rejects_hardlink_metadata_failure_before_read",
        "test_parse_sha256_manifest_rejects_non_utf8_bytes_without_traceback",
        "test_parse_sha256_manifest_rejects_oversized_manifest_before_parse",
        "test_parse_sha256_manifest_rejects_regular_file_swap_after_preflight",
        "test_verify_sha256_manifest_rejects_secret_slot_path_directly_before_traversal",
        "test_verify_sha256_manifest_rejects_symlinked_slot_root_directly_before_parse",
        "test_verify_sha256_manifest_rejects_slot_metadata_failure_before_parse",
        "test_verify_sha256_manifest_rejects_symlinked_slot_ancestor_before_discovery",
        "test_verify_sha256_manifest_missing_slot_returns_missing_manifest_without_traceback",
        "test_verify_sha256_manifest_rejects_hardlinked_manifest_before_discovery",
        "test_verify_sha256_manifest_rejects_symlinked_artifact_directory_before_digest_read",
        "test_manifest_artifact_digest_rejects_secret_relative_path_directly",
        "test_manifest_artifact_digest_rejects_control_relative_path_directly",
        "test_manifest_artifact_digest_rejects_symlink_directly",
        "test_manifest_artifact_digest_rejects_hardlink_directly",
        "test_manifest_artifact_digest_rejects_oversized_artifact_directly",
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
        "test_metadata_artifact_digest_rejects_oversized_artifact_after_preflight",
        "test_metadata_artifact_digest_rejects_read_failure_after_preflight",
        "test_metadata_artifact_digest_rejects_symlink_swap_after_preflight",
        "test_metadata_artifact_digest_rejects_regular_file_swap_after_preflight",
        "test_d2d_transcript_binding_rejects_symlink_path_before_digest_read",
        "test_wallet_transcript_binding_rejects_hardlink_path_before_digest_read",
        "test_d2d_transcript_rejects_symlinked_queue_before_digest_read",
        "test_d2d_transcript_uses_lstat_before_queue_is_file_preflight",
        "test_required_artifact_shapes_rejects_secret_slot_path_directly_before_stat",
        "test_required_artifact_shapes_rejects_oversized_artifact_directly",
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
        "test_production_metadata_rejects_abi6_probe_ok_status_alias",
        "test_production_metadata_rejects_noncanonical_probe_states",
        "test_production_metadata_rejects_noncanonical_slot_keymint_level",
        "test_production_metadata_rejects_signed_evidence_digest_drift",
        "test_production_metadata_rejects_zero_sha256_placeholders",
        "test_kagemusha_slot_metadata_rejects_zero_sha256_placeholders",
        "test_production_metadata_uses_lstat_before_signed_evidence_is_file_preflight",
        "test_metadata_artifact_digest_rejects_secret_relative_path_directly",
        "test_metadata_artifact_digest_rejects_control_relative_path_directly",
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
        "device_lab.os.scandir = failing_scandir",
        "test_load_json_rejects_symlinked_ancestor_before_read",
        "test_load_json_rejects_symlink_swap_after_preflight",
        "test_load_json_rejects_regular_file_swap_after_preflight",
        "test_validate_no_symlink_ancestors_rejects_cwd_failure",
        "test_validate_no_symlink_ancestors_rejects_ancestor_metadata_failure",
        "test_validate_no_symlink_ancestors_uses_lstat_before_is_symlink_preflight",
        "test_validate_no_symlink_ancestors_uses_lstat_before_exists_preflight",
        "test_load_json_rejects_secret_path_directly_before_parse",
        "test_load_json_rejects_control_path_directly_before_parse",
        "test_load_json_rejects_alias_path_directly_before_metadata",
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
        "test_d2d_payment_transcript_rejects_zero_sha256_placeholders",
        "test_production_metadata_rejects_d2d_payment_transcript_outside_handoff",
        "test_production_metadata_rejects_missing_wallet_integrity_transcript_binding",
        "test_production_metadata_rejects_wallet_integrity_transcript_digest_drift",
        "test_wallet_integrity_transcript_rejects_zero_sha256_placeholders",
        "test_production_metadata_rejects_wallet_integrity_false_rollback_claim",
        "test_production_metadata_rejects_noncanonical_transcript_strings",
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
        "test_production_metadata_rejects_whitespace_normalized_attestation_slot_alias",
        "test_production_metadata_rejects_noncanonical_attestation_status",
        "test_production_metadata_rejects_attestation_passed_status_alias",
        "test_production_metadata_rejects_attestation_result_without_strongbox",
        "test_production_metadata_rejects_whitespace_normalized_attestation_strongbox_level",
        "test_production_metadata_rejects_attestation_result_slot_keymint_mismatch",
        "test_production_metadata_rejects_whitespace_normalized_attestation_report_binding",
        "test_production_metadata_rejects_attestation_report_without_strongbox",
        "test_production_metadata_rejects_whitespace_normalized_attestation_report_strongbox",
        "test_production_metadata_rejects_missing_attestation_report_level_fields",
        "test_production_metadata_rejects_attestation_report_result_level_mismatch",
        "test_production_metadata_rejects_attestation_report_result_status_mismatch",
        "test_production_metadata_rejects_noncanonical_attestation_report_status",
        "test_production_metadata_rejects_zero_attestation_sha256_bindings",
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
        "test_production_metadata_rejects_signed_evidence_device_model_mismatch",
        "test_production_metadata_rejects_slot_family_model_codename_mismatch",
        "test_production_metadata_rejects_conflicting_model_codename",
        "test_production_metadata_rejects_unknown_model_with_known_codename",
        "test_production_metadata_rejects_whitespace_normalized_signed_evidence_slot_field",
        "test_production_metadata_rejects_control_signed_evidence_slot_field",
        "test_production_metadata_rejects_whitespace_normalized_signed_evidence_algorithm",
        "test_production_metadata_rejects_signed_evidence_digest_map_drift",
        "test_production_metadata_rejects_zero_signed_evidence_artifact_digest",
        "test_signed_evidence_artifact_digest_rejects_secret_relative_path_directly",
        "test_signed_evidence_artifact_digest_rejects_control_relative_path_directly",
        "test_signed_evidence_artifact_digest_rejects_symlink_directly",
        "test_signed_evidence_artifact_digest_rejects_hardlink_directly",
        "test_signed_evidence_artifact_digest_rejects_oversized_artifact_directly",
        "test_signed_evidence_artifact_digest_rejects_file_metadata_failure",
        "test_signed_evidence_artifact_digest_rejects_read_failure_after_preflight",
        "test_signed_evidence_artifact_digest_rejects_regular_file_swap_after_preflight",
        "test_signed_evidence_artifact_revalidates_required_digest_before_read",
        "test_production_metadata_rejects_signed_evidence_missing_required_digest",
        "test_production_metadata_rejects_missing_required_slot_artifact",
        "test_production_metadata_rejects_empty_required_slot_artifact",
        "test_production_metadata_rejects_oversized_required_slot_artifact",
        "test_production_metadata_rejects_telemetry_slot_mismatch",
        "test_production_metadata_rejects_whitespace_normalized_telemetry_slot",
        "test_production_metadata_rejects_telemetry_extra_field",
        "test_production_metadata_rejects_noncanonical_telemetry_identity_strings",
        "test_production_metadata_rejects_telemetry_model_slot_mismatch",
        "test_production_metadata_rejects_telemetry_app_package_mismatch",
        "test_production_metadata_rejects_noncanonical_telemetry_slot_binding",
        "test_production_metadata_rejects_noncanonical_telemetry_suite",
        "test_production_metadata_rejects_pending_queue_shape",
        "test_production_metadata_rejects_failed_status_ndjson",
        "test_production_metadata_rejects_status_ndjson_unexpected_field",
        "test_production_metadata_rejects_unknown_status_ndjson",
        "test_production_metadata_requires_status_ndjson_slot_id",
        "test_production_metadata_rejects_noncanonical_status_ndjson",
        "test_production_metadata_rejects_status_ndjson_slot_mismatch",
        "test_production_metadata_rejects_runtime_log_without_completion_marker",
        "test_production_metadata_rejects_runtime_log_failure_marker",
        "test_production_metadata_rejects_signed_evidence_missing_handoff_digest",
        "test_production_metadata_rejects_missing_trusted_signer_public_key",
        "test_production_metadata_rejects_untrusted_signed_evidence_key",
        "test_production_metadata_rejects_trusted_signer_public_key_symlinked_ancestor_from_direct_map",
        "test_production_metadata_rejects_alias_trusted_signer_map_before_metadata_read",
        "test_production_metadata_rejects_control_trusted_signer_map_before_metadata_read",
        "test_production_metadata_rejects_signed_evidence_payload_hash_drift",
        "test_production_metadata_rejects_zero_signed_evidence_sha256_placeholders",
        "test_kagemusha_slot_assembler_rejects_zero_source_sha256_placeholders_before_publish",
        "test_signed_evidence_canonical_payload_rejects_nonfinite_json",
        "test_production_metadata_rejects_signed_evidence_signature_drift",
        "test_duplicate_matrix_bindings_redacts_unsafe_direct_report_slots",
        "test_duplicate_matrix_bindings_ignores_non_sha256_direct_values",
        "test_duplicate_matrix_bindings_ignores_zero_direct_values",
        "test_build_summary_redacts_unsafe_direct_report_strings",
        "test_build_summary_marks_redacted_key_collision_without_overwrite",
        "test_build_summary_normalizes_malformed_direct_report_status",
        "test_build_summary_normalizes_malformed_direct_report_errors",
        "test_build_summary_normalizes_non_string_direct_report_keys",
        "test_build_summary_redacts_nonfinite_direct_report_values",
        "test_build_summary_normalizes_finite_float_direct_report_values",
        "test_build_summary_normalizes_unsupported_direct_report_values",
        "test_build_summary_normalizes_malformed_kagemusha_report_shape",
        "test_build_summary_ignores_malformed_direct_device_family_values",
        "test_duplicate_matrix_bindings_can_require_complete_signed_evidence",
        "test_build_summary_requires_complete_signed_evidence_for_kagemusha_rollup",
        "test_build_summary_preserves_complete_signed_evidence_for_kagemusha_rollup",
        "test_build_summary_requires_trusted_signer_for_kagemusha_rollup",
        "test_build_summary_rejects_malformed_complete_signed_evidence_rollup_fields",
        "test_build_summary_ignores_non_sha256_direct_trusted_signer_keys",
        "test_build_summary_ignores_zero_direct_trusted_signer_keys",
        "test_production_metadata_rejects_zero_trusted_signer_digest_before_metadata_read",
        "test_production_metadata_rejects_non_path_trusted_signer_map_before_metadata_read",
        "test_production_metadata_rejects_non_mapping_trusted_signer_map_before_metadata_read",
        "test_production_metadata_rejects_mixed_trusted_signer_digest_keys_without_crash",
        "test_production_metadata_rejects_unrepresentable_trusted_signer_digest_without_crash",
        "test_build_summary_ignores_non_mapping_direct_trusted_signer_keys",
        "test_build_summary_ignores_mixed_direct_trusted_signer_key_types",
        "test_json_summary_reports_kagemusha_matrix_and_signer_pins",
        "test_json_summary_does_not_leak_trusted_signer_key_paths",
        "test_json_summary_does_not_leak_device_lab_root_or_summary_output_path",
        "test_root_validator_rejects_secret_path_directly_without_leak",
        "test_root_validator_rejects_control_path_directly_without_leak",
        "test_root_validator_rejects_alias_path_directly_before_metadata",
        "test_root_validator_rejects_metadata_failure_directly_without_leak",
        "test_main_uses_lstat_before_missing_root_exists_preflight",
        "test_main_rejects_secret_looking_root_without_leak",
        "test_main_rejects_control_root_without_leak",
        "test_main_rejects_control_slot_before_root_classify_without_leak",
        "test_main_rejects_control_trusted_signer_before_slot_discovery_without_leak",
        "test_json_summary_rejects_secret_looking_output_without_leak",
        "test_json_summary_rejects_control_output_without_leak",
        "test_write_summary_rejects_secret_output_path_directly_without_leak",
        "test_write_summary_rejects_control_output_path_directly_without_leak",
        "test_validate_summary_output_path_uses_lstat_before_parent_is_dir_preflight",
        "test_validate_summary_output_path_rejects_parent_metadata_failure",
        "test_validate_summary_output_path_rejects_aliases_before_parent_metadata",
        "test_write_summary_uses_lstat_before_parent_is_dir_preflight",
        "test_write_summary_rejects_parent_metadata_failure_before_write",
        "test_write_summary_rejects_parent_create_failure_before_write",
        "test_write_summary_rejects_file_metadata_failure_before_write",
        "test_write_summary_rejects_hardlink_metadata_failure_before_write",
        "test_write_summary_rejects_nonfinite_json_before_write",
        "test_write_summary_rejects_oversized_json_before_write",
        "test_write_summary_rejects_write_failure_after_preflight",
        "test_write_summary_preserves_existing_output_on_replace_failure",
        "test_write_summary_reports_temp_cleanup_failure_after_write_failure",
        "test_write_summary_reports_temp_cleanup_failure_after_post_stage_validation_failure",
        "test_write_summary_temp_cleanup_rejects_swapped_temp_file",
        "test_write_summary_rejects_parent_directory_sync_failure_after_replace",
        "test_write_summary_rejects_parent_directory_identity_swap_before_sync",
        "test_write_summary_rejects_symlink_swap_before_replace",
        "test_write_summary_rejects_readback_mismatch",
        "test_write_summary_rejects_readback_failure",
        "test_read_summary_output_rejects_oversized_readback",
        "test_write_summary_rejects_regular_file_swap_before_readback",
        "test_write_summary_rejects_symlink_swap_after_replace",
        "test_write_summary_rechecks_parent_after_create_before_write",
        "test_json_summary_rejects_symlinked_output_without_following_alias",
        "test_json_summary_rejects_hardlinked_output_without_overwriting_alias",
        "test_standard_matrix_requires_every_kagemusha_device_family",
        "test_standard_matrix_accepts_all_kagemusha_device_families",
        "test_signer_helper_generates_validator_accepted_evidence",
        "test_signer_helper_rejects_nonfinite_canonical_payload_before_signing",
        "test_signer_helper_rejects_mismatched_private_and_public_keys",
        "test_trusted_signer_public_key_rejects_symlink_without_path_leak",
        "test_trusted_signer_public_key_rejects_secret_looking_path_without_leak",
        "test_trusted_signer_public_key_rejects_secret_path_before_openssl_lookup",
        "test_trusted_signer_public_key_rejects_control_path_before_openssl_lookup",
        "test_trusted_signer_public_key_rejects_aliases_before_openssl_lookup",
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
        "test_write_staged_bytes_rejects_hardlink_created_before_readback",
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
        "test_signer_helper_rejects_control_public_key_path_before_write",
        "test_signer_helper_rejects_key_aliases_before_metadata_read",
        "test_signer_helper_rejects_secret_looking_slot_path_before_metadata_read",
        "test_signer_helper_rejects_control_slot_path_before_metadata_read",
        "test_signer_helper_rejects_alias_slot_path_before_metadata_read",
        "test_signer_helper_rejects_slot_directory_metadata_failure_before_read",
        "test_signer_helper_rejects_slot_parent_metadata_failure_before_read",
        "test_signer_helper_rejects_secret_looking_output_before_metadata_read",
        "test_signer_helper_rejects_control_output_before_metadata_read",
        "test_signer_json_output_validators_reject_alias_paths_before_metadata",
        "test_signer_helper_rejects_secret_looking_signer_key_id_before_metadata_read",
        "private key path must not contain secret-looking material",
        "signer public key path must not contain secret-looking material",
        "private key path must be canonical",
        "private key path must not contain backslashes",
        "signer public key path must be canonical",
        "signer public key path must not contain backslashes",
        "test_sign_ed25519_rejects_private_key_aliases_before_metadata_or_openssl",
        "test_signer_helper_rejects_output_outside_evidence_before_write",
        "test_signer_helper_rejects_noncanonical_output_filename_before_write",
        "test_signer_helper_rejects_backslash_output_path_before_write",
        "test_signer_helper_rejects_absolute_parent_segment_output_path_before_write",
        "test_signer_output_normalise_rejects_output_resolve_failure",
        "test_signer_output_normalise_rejects_slot_resolve_failure",
        "test_signer_output_normalise_rejects_absolute_symlinked_output_ancestor",
        "test_signer_output_normalise_rejects_absolute_symlinked_output_leaf",
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
        "test_signer_write_json_rejects_control_output_path_directly_without_write",
        "test_signer_write_json_rejects_nonfinite_json_before_write",
        "test_signer_write_json_rejects_oversized_json_before_write",
        "test_signer_write_json_rejects_write_failure_after_preflight",
        "test_signer_write_json_preserves_existing_output_on_replace_failure",
        "test_signer_write_json_reports_temp_cleanup_failure_after_write_failure",
        "test_signer_write_json_reports_temp_cleanup_failure_after_post_stage_validation_failure",
        "test_signer_write_json_temp_cleanup_rejects_swapped_temp_file",
        "test_signer_write_json_rejects_parent_directory_sync_failure_after_replace",
        "test_signer_write_json_rejects_symlink_swap_before_replace",
        "test_signer_write_json_rejects_readback_mismatch",
        "test_signer_write_json_rejects_readback_failure",
        "test_signer_write_json_rejects_oversized_readback_after_replace",
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
        "test_signer_output_digest_rejects_oversized_output_after_write",
        "test_signer_output_digest_rejects_hardlink_metadata_failure_after_write",
        "test_signer_output_digest_rejects_file_metadata_failure_after_write",
        "test_signer_output_digest_rejects_read_failure_after_preflight",
        "test_signer_output_digest_rejects_regular_file_swap_after_preflight",
        "test_signer_helper_revalidates_output_digest_before_slot_json_update",
        "test_signer_write_text_rejects_symlinked_manifest_leaf_before_write",
        "test_signer_write_text_rejects_dangling_symlinked_manifest_leaf_before_write",
        "test_signer_write_text_rejects_hardlinked_manifest_leaf_before_write",
        "test_signer_write_text_rejects_secret_manifest_path_directly_without_write",
        "test_signer_write_text_rejects_oversized_manifest_before_write",
        "test_signer_write_text_rejects_write_failure_after_preflight",
        "test_signer_write_text_preserves_existing_output_on_replace_failure",
        "test_signer_write_text_rejects_symlink_swap_before_replace",
        "test_signer_write_text_rejects_readback_mismatch",
        "test_signer_write_text_rejects_readback_failure",
        "test_signer_write_text_rejects_oversized_readback_after_replace",
        "test_signer_write_text_rejects_symlink_swap_after_replace",
        "test_rewrite_sha256_manifest_rejects_oversized_manifest_before_write",
        "test_rewrite_sha256_manifest_rejects_symlinked_artifact_when_called_directly",
        "test_rewrite_sha256_manifest_rejects_hardlinked_manifest_when_called_directly",
        "test_rewrite_sha256_manifest_rejects_secret_looking_artifact_when_called_directly",
        "test_rewrite_sha256_manifest_rejects_secret_slot_path_directly_without_write",
        "test_rewrite_sha256_manifest_rejects_slot_directory_metadata_failure_without_write",
        "test_rewrite_sha256_manifest_rejects_slot_parent_metadata_failure_without_write",
        "test_signer_slot_artifact_digest_rejects_secret_relative_path_directly",
        "test_signer_slot_artifact_digest_rejects_control_relative_path_directly",
        "test_signer_slot_artifact_digest_rejects_symlink_directly",
        "test_signer_slot_artifact_digest_rejects_hardlink_directly",
        "test_signer_slot_artifact_digest_rejects_oversized_artifact_directly",
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
        "test_signer_helper_rejects_missing_attestation_report_level_before_write",
        "test_signer_helper_rejects_d2d_transcript_mismatch_before_write",
        "test_signer_helper_rejects_wallet_integrity_transcript_mismatch_before_write",
        "test_signer_helper_rejects_secret_looking_artifact_paths_before_write",
        "test_signer_helper_rejects_missing_required_slot_artifact_before_write",
        "test_signer_helper_rejects_empty_required_slot_artifact_before_write",
        "test_signer_helper_rejects_failed_status_ndjson_before_write",
        "test_signer_helper_does_not_leak_secret_looking_private_key_path",
        "test_sign_ed25519_rejects_secret_private_key_path_before_openssl_lookup",
        "test_sign_ed25519_rejects_control_private_key_path_before_openssl_lookup",
        "test_sign_ed25519_rejects_missing_private_key_before_openssl_lookup",
        "test_sign_ed25519_rejects_non_regular_private_key_before_openssl_lookup",
        "test_sign_ed25519_rejects_private_key_file_metadata_failure_before_openssl",
        "test_sign_ed25519_rejects_private_key_hardlink_metadata_failure_before_openssl",
        "test_sign_ed25519_rejects_payload_staging_write_failure_before_openssl",
        "test_sign_ed25519_rejects_payload_staging_readback_mismatch_before_openssl",
        "test_sign_ed25519_rejects_signature_read_failure_after_openssl",
        "test_sign_ed25519_rejects_signature_output_swap_after_openssl",
        "test_sign_ed25519_rejects_signature_output_hardlink_after_openssl",
        "test_sign_ed25519_reads_only_shape_bound_signature_output_after_openssl",
        "test_sign_ed25519_rejects_short_signature_output_after_openssl",
        "test_sign_ed25519_rejects_tempdir_failure_before_payload_staging",
        "test_sign_ed25519_rejects_spawn_failure_after_payload_staging",
        "test_sign_ed25519_rejects_invalid_private_key_after_openssl_failure",
        "test_sign_ed25519_rejects_signature_read_failure_after_openssl",
        "test_sign_ed25519_rejects_short_signature_output_after_openssl",
    ),
    "scripts/tests/kagemusha_production_readiness_test.py": (
        "test_complete_signed_android_matrix_passes_rollup",
        "test_staged_path_validators_reject_control_directory_paths_before_metadata",
        "test_staged_path_validators_reject_alias_directory_paths_before_metadata",
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
        "test_android_rollup_rejects_duplicate_slot_ids_before_root_classify",
        "test_android_report_control_material_is_redacted_before_summary",
        "test_android_report_nonfinite_numbers_are_redacted_before_summary",
        "test_android_report_redacted_key_collision_blocks_without_overwrite",
        "test_android_report_non_string_keys_are_normalized_before_summary",
        "test_android_report_unsupported_values_are_normalized_before_summary",
        "test_android_report_malformed_errors_are_normalized_before_summary",
        "test_android_report_malformed_kagemusha_shape_blocks_without_traceback",
        "test_android_report_malformed_device_family_does_not_cover_matrix",
        "test_android_slot_summary_omits_incomplete_release_kagemusha_fields",
        "test_android_slot_summary_preserves_complete_release_kagemusha_fields",
        "test_android_slot_summary_requires_report_match_for_duplicate_slot_admission",
        "test_android_duplicate_bindings_summary_omits_incomplete_release_slots",
        "test_android_duplicate_bindings_summary_preserves_complete_release_slots",
        "test_android_matrix_rejects_noncanonical_direct_binding_digest",
        "test_android_matrix_rejects_zero_direct_binding_digest",
        "test_android_matrix_redacts_secret_direct_binding_digest",
        "test_android_matrix_redacts_unsafe_direct_duplicate_slots",
        "test_android_matrix_redacts_control_direct_binding_digest_slot",
        "test_android_signed_evidence_summary_rejects_malformed_direct_values",
        "test_android_signed_evidence_summary_rejects_missing_direct_values",
        "test_android_signed_evidence_summary_rejects_single_missing_core_binding_without_partial_reflection",
        "test_android_signed_evidence_summary_rejects_single_missing_artifact_binding_without_partial_reflection",
        "test_android_signed_evidence_summary_rejects_unsafe_direct_slot_keys",
        "test_android_signed_evidence_summary_rejects_duplicate_safe_slot_without_overwrite",
        "test_android_signed_evidence_summary_redacts_secret_direct_values",
        "test_android_signed_evidence_summary_includes_device_identity",
        "test_android_signed_evidence_summary_rejects_family_model_mismatch",
        "test_android_signed_evidence_summary_rejects_one_sided_identity_match",
        "test_android_signed_evidence_summary_rejects_unknown_codename_identity_match",
        "test_android_signed_evidence_summary_rejects_malformed_identity_values",
        "test_android_signed_evidence_summary_rejects_missing_identity_values",
        "test_android_signed_evidence_summary_rejects_single_missing_identity_without_partial_reflection",
        "test_untrusted_signed_evidence_blocks_rollup",
        "summary[\"android_device_lab\"][\"signed_evidence\"]",
        "expected_android_signed_evidence",
        "test_abi6_manifest_drift_blocks_rollup_section",
        "test_abi6_manifest_rejects_oversized_manifest_json",
        "test_abi6_manifest_rejects_symlinked_manifest_file",
        "test_abi6_manifest_rejects_symlink_swap_after_preflight",
        "test_abi6_manifest_rejects_regular_file_swap_after_preflight",
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
        "test_repo_source_marker_text_rejects_oversized_marker_before_decode",
        "test_repo_source_marker_text_accepts_large_checked_in_marker",
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
        "test_compact_key_evidence_rejects_oversized_evidence_json",
        "test_compact_key_evidence_rejects_duplicate_json_keys",
        "test_compact_key_evidence_rejects_secret_duplicate_json_key",
        "test_compact_key_evidence_rejects_nonfinite_json_constant",
        "test_stale_compact_key_evidence_blocks_rollup_section",
        "test_compact_key_evidence_rejects_noncanonical_timestamp",
        "test_compact_key_evidence_rejects_timestamp_string_shape",
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
        "test_compact_key_evidence_rejects_oversized_generator_log",
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
        "test_compact_key_staged_finalizer_publishes_validator_accepted_evidence",
        "test_compact_key_staged_finalizer_rejects_missing_execution_report",
        "test_compact_key_staged_finalizer_rejects_zero_execution_log_digest",
        "test_compact_key_staged_finalizer_rejects_execution_log_digest_drift",
        "test_compact_key_staged_finalizer_rejects_execution_report_command_exactness",
        "test_compact_key_staged_finalizer_rejects_run_report_zero_elapsed",
        "test_compact_key_staged_finalizer_rejects_execution_report_elapsed_drift",
        "test_compact_key_staged_finalizer_rejects_missing_exit_marker",
        "test_compact_key_staged_finalizer_rejects_nonzero_exit_marker",
        "test_compact_key_staged_finalizer_rejects_nonzero_marker_before_success_inputs",
        "test_compact_key_staged_finalizer_requires_run_report_on_success_marker",
        "test_compact_key_staged_finalizer_rejects_run_report_exit_mismatch",
        "test_compact_key_staged_finalizer_rejects_run_report_command_mismatch",
        "test_compact_key_staged_finalizer_rejects_run_report_log_size_drift",
        "test_compact_key_staged_finalizer_rejects_symlinked_run_report",
        "test_compact_key_staged_finalizer_rejects_hardlinked_run_report",
        "test_compact_key_staged_finalizer_redacts_secret_duplicate_run_report_key",
        "test_compact_key_staged_finalizer_refuses_destination_overwrite",
        "test_compact_key_staged_finalizer_rejects_symlinked_staged_artifact",
        "test_compact_key_staged_finalizer_rejects_generator_log_digest_drift",
        "test_compact_key_staged_finalizer_defaults_out_under_artifact_dir",
        "test_compact_key_runner_and_finalizer_defaults_share_staging_paths",
        "test_compact_key_staged_finalizer_cleans_partial_publish_on_copy_error",
        "test_compact_key_staged_finalizer_reports_partial_publish_cleanup_failure",
        "test_compact_key_staged_finalizer_unlink_preserves_swapped_published_file",
        "test_compact_key_staged_finalizer_verifies_published_stage_bytes",
        "test_compact_key_staged_finalizer_publish_outputs_private_permissions",
        "test_compact_key_staged_finalizer_reports_temp_parent_cleanup_failure",
        "test_compact_key_staged_runner_outputs_finalize_successfully",
        "test_compact_key_staged_runner_resume_reuses_complete_keygen",
        "test_compact_key_staged_runner_rejects_replace_with_resume_keygen",
        "test_compact_key_staged_runner_resume_replaces_failed_keygen",
        "test_compact_key_staged_runner_resume_rejects_symlinked_artifact",
        "test_compact_key_staged_runner_refuses_existing_artifact_before_run",
        "test_compact_key_staged_runner_refuses_existing_run_report_before_run",
        "test_compact_key_staged_runner_preserves_nonzero_exit_marker",
        "test_compact_key_staged_runner_main_reports_nonzero_conventionally",
        "test_compact_key_staged_runner_main_errors_exit_conventionally",
        "test_compact_key_staged_runner_atomic_write_verifies_installed_bytes",
        "test_compact_key_staged_runner_atomic_write_installs_private_file",
        "test_compact_key_staged_runner_resume_cleanup_preserves_swapped_output",
        "test_compact_key_staged_runner_temp_cleanup_preserves_swapped_output",
        "test_compact_key_staged_runner_log_install_installs_private_file",
        "test_compact_key_staged_runner_creates_private_staging_outputs",
        "test_compact_key_staged_runner_rejects_symlinked_exit_marker",
        "test_compact_key_staged_runner_writes_child_output_directly_to_log_file",
        "fsync_fds",
        "stdout_fds",
        "test_compact_key_staged_runner_removes_temp_log_on_spawn_failure",
        "test_lineage_proof_staged_finalizer_publishes_validator_accepted_evidence",
        "test_lineage_proof_staged_finalizer_rejects_missing_execution_report",
        "test_lineage_proof_staged_finalizer_rejects_zero_execution_log_digest",
        "test_lineage_proof_staged_finalizer_rejects_execution_log_digest_drift",
        "test_lineage_proof_staged_finalizer_rejects_execution_report_command_exactness",
        "test_lineage_proof_staged_finalizer_rejects_missing_exit_marker",
        "test_lineage_proof_staged_finalizer_rejects_missing_marker_before_elapsed",
        "test_lineage_proof_staged_finalizer_rejects_nonzero_exit_marker",
        "test_lineage_proof_staged_finalizer_rejects_partial_nonzero_stage",
        "test_lineage_proof_staged_finalizer_rejects_nonzero_marker_before_success_inputs",
        "test_lineage_proof_staged_finalizer_requires_run_report_on_success_marker",
        "test_lineage_proof_staged_finalizer_rejects_run_report_exit_mismatch",
        "test_lineage_proof_staged_finalizer_rejects_run_report_command_mismatch",
        "test_lineage_proof_staged_finalizer_rejects_run_report_elapsed_mismatch",
        "test_lineage_proof_staged_finalizer_rejects_run_report_log_size_drift",
        "test_lineage_proof_staged_finalizer_rejects_run_report_missing_key_log",
        "test_lineage_proof_staged_finalizer_rejects_run_report_key_log_size_drift",
        "test_lineage_proof_staged_finalizer_rejects_symlinked_run_report",
        "test_lineage_proof_staged_finalizer_rejects_hardlinked_run_report",
        "test_lineage_proof_staged_finalizer_redacts_secret_duplicate_run_report_key",
        "test_lineage_proof_staged_finalizer_refuses_destination_overwrite",
        "test_lineage_proof_staged_finalizer_rejects_symlinked_staged_artifact",
        "test_lineage_proof_staged_finalizer_rejects_bad_proof_log",
        "test_lineage_proof_staged_finalizer_defaults_out_under_artifact_dir",
        "test_lineage_runner_and_finalizer_defaults_share_staging_paths",
        "test_lineage_proof_staged_finalizer_cleans_partial_publish_on_copy_error",
        "test_lineage_proof_staged_finalizer_reports_partial_publish_cleanup_failure",
        "test_lineage_proof_staged_finalizer_unlink_preserves_swapped_published_file",
        "test_lineage_proof_staged_finalizer_verifies_published_stage_bytes",
        "test_lineage_proof_staged_finalizer_publish_outputs_private_permissions",
        "test_lineage_proof_staged_finalizer_reports_temp_parent_cleanup_failure",
        "test_lineage_proof_staged_runner_outputs_finalize_successfully",
        "test_lineage_proof_staged_runner_resume_reuses_completed_init_phase",
        "test_lineage_proof_staged_runner_rejects_replace_with_resume_key_artifacts",
        "test_lineage_proof_staged_runner_resume_replaces_failed_append_phase",
        "test_lineage_proof_staged_runner_resume_rejects_symlinked_phase_output",
        "test_lineage_proof_staged_runner_refuses_existing_log_before_run",
        "test_lineage_proof_staged_runner_refuses_existing_run_report_before_run",
        "test_lineage_proof_staged_runner_preserves_nonzero_exit_marker",
        "test_lineage_proof_staged_runner_reports_nonzero_init_keygen_phase",
        "test_lineage_proof_staged_runner_main_reports_nonzero_without_success_paths",
        "test_lineage_proof_staged_runner_main_errors_exit_conventionally",
        "test_lineage_proof_staged_runner_atomic_write_verifies_installed_bytes",
        "test_lineage_proof_staged_runner_atomic_write_installs_private_file",
        "test_lineage_proof_staged_runner_resume_cleanup_preserves_swapped_output",
        "test_lineage_proof_staged_runner_temp_cleanup_preserves_swapped_output",
        "test_lineage_proof_staged_runner_log_install_installs_private_file",
        "test_lineage_proof_staged_runner_creates_private_staging_outputs",
        "test_lineage_proof_staged_runner_rejects_symlinked_exit_marker",
        "test_lineage_proof_staged_runner_writes_child_output_directly_to_log_file",
        "test_lineage_proof_staged_runner_removes_temp_log_on_spawn_failure",
        "test_lineage_proof_staged_finalizer_rejects_elapsed_seconds_file_conflict",
        "test_lineage_proof_staged_finalizer_rejects_bad_elapsed_seconds_file",
        "test_lineage_proof_staged_finalizer_rejects_integer_elapsed_seconds_file",
        "test_lineage_proof_staged_finalizer_rejects_padded_elapsed_seconds_file",
        "test_lineage_proof_staged_finalizer_rejects_zero_elapsed_seconds_file",
        "test_lineage_proof_staged_finalizer_redacts_control_elapsed_seconds_file",
        "test_compact_key_evidence_helper_rejects_missing_artifact",
        "test_compact_key_evidence_helper_rejects_empty_artifact",
        "test_compact_key_evidence_helper_rejects_artifact_symlink_swap_after_preflight",
        "test_compact_key_evidence_helper_rejects_artifact_regular_file_swap_after_preflight",
        "test_compact_key_evidence_helper_rejects_placeholder_artifact",
        "test_compact_key_evidence_helper_placeholder_check_uses_hashed_prefix",
        "test_compact_key_evidence_helper_rejects_all_placeholder_prefixes",
        "test_compact_key_evidence_helper_rejects_all_zero_artifact",
        "test_compact_key_evidence_helper_rejects_missing_generator_log",
        "test_compact_key_evidence_helper_rejects_symlinked_generator_log_before_artifact_reads",
        "test_compact_key_evidence_helper_rejects_generator_log_size_drift",
        "test_compact_key_evidence_helper_rejects_generator_log_digest_drift",
        "test_compact_key_evidence_helper_rejects_generator_log_trailing_whitespace",
        "test_compact_key_evidence_helper_rejects_generator_log_crlf_line_endings",
        "test_compact_key_evidence_helper_rejects_generator_log_without_final_lf",
        "test_compact_key_evidence_helper_rejects_generator_log_invalid_utf8_bytes",
        "test_compact_key_evidence_helper_rejects_noncanonical_generated_at_utc",
        "test_compact_key_evidence_helper_rejects_appended_shell_command",
        "test_compact_key_evidence_helper_rejects_control_generator_log_before_artifact_reads",
        "test_compact_key_build_evidence_rejects_control_generator_log_before_artifact_dir_metadata",
        "test_compact_key_evidence_helper_rejects_secret_generator_log_before_artifact_reads",
        "test_compact_key_evidence_helper_rejects_outside_artifact_dir",
        "test_evidence_output_corridors_reject_control_paths_before_resolve",
        "test_evidence_artifact_dir_validators_reject_aliases_before_metadata",
        "test_evidence_output_corridors_reject_alias_paths_before_resolve",
        "test_compact_key_evidence_helper_rejects_symlinked_output_leaf",
        "test_compact_key_evidence_helper_rejects_dangling_symlinked_output_leaf",
        "test_compact_key_output_preflight_rejects_parent_create_failure_before_write",
        "test_compact_key_output_preflight_rejects_file_metadata_failure_before_write",
        "test_compact_key_output_preflight_rejects_hardlink_metadata_failure_before_write",
        "test_compact_key_write_evidence_rejects_write_failure_after_preflight",
        "test_compact_key_write_evidence_rejects_nonfinite_json_before_write",
        "test_compact_key_write_evidence_rejects_oversized_json_before_write",
        "test_compact_key_write_evidence_installs_private_permissions",
        "test_compact_key_validate_evidence_document_installs_private_scratch_permissions",
        "test_compact_key_write_evidence_preserves_existing_output_on_replace_failure",
        "test_compact_key_write_evidence_reports_temp_cleanup_failure_after_write_failure",
        "test_compact_key_write_evidence_reports_temp_cleanup_failure_after_post_stage_validation_failure",
        "test_compact_key_write_evidence_temp_cleanup_rejects_swapped_temp_file",
        "test_compact_key_write_evidence_rejects_parent_directory_sync_failure_after_replace",
        "test_compact_key_write_evidence_rejects_parent_directory_identity_swap_before_sync",
        "test_compact_key_write_evidence_rejects_readback_mismatch",
        "test_compact_key_write_evidence_rejects_readback_failure",
        "test_compact_key_write_evidence_rejects_oversized_readback_after_replace",
        "test_compact_key_write_evidence_rejects_regular_file_swap_before_readback",
        "test_compact_key_write_evidence_rejects_symlink_swap_before_replace",
        "test_compact_key_write_evidence_rejects_symlink_swap_after_replace",
        "test_compact_key_evidence_document_validator_rejects_artifact_dir_create_failure_after_preflight",
        "test_compact_key_evidence_document_validator_rejects_nonfinite_json_before_write",
        "test_compact_key_evidence_document_validator_rejects_temp_write_failure_after_preflight",
        "test_compact_key_evidence_document_validator_reports_temp_cleanup_failure_after_write_failure",
        "test_compact_key_evidence_document_validator_rejects_temp_cleanup_failure",
        "test_compact_key_evidence_document_validator_temp_cleanup_rejects_swap",
        "test_compact_key_artifact_dir_validator_rejects_secret_path_directly",
        "test_compact_key_artifact_dir_validator_rejects_metadata_failure_directly",
        "test_compact_key_sha256_file_rejects_secret_path_directly",
        "test_compact_key_sha256_file_rejects_symlink_directly",
        "test_compact_key_sha256_file_rejects_hardlink_directly",
        "test_compact_key_sha256_file_rejects_read_failure_without_traceback",
        "test_compact_key_generator_log_path_rejects_control_artifact_dir_before_resolve",
        "test_compact_key_generator_log_path_rejects_alias_before_metadata",
        "test_compact_key_generator_log_path_rejects_symlink_without_resolving_final_log",
        "test_missing_lineage_proof_evidence_blocks_rollup_section",
        "test_lineage_proof_evidence_rejects_noncanonical_filename",
        "test_lineage_proof_evidence_rejects_symlinked_evidence_file",
        "test_lineage_proof_evidence_rejects_json_symlink_swap_after_preflight",
        "test_lineage_proof_evidence_rejects_symlinked_evidence_ancestor",
        "test_lineage_proof_evidence_rejects_secret_path_before_json_parse",
        "test_lineage_proof_evidence_rejects_non_utf8_without_traceback",
        "test_lineage_proof_evidence_rejects_oversized_evidence_json",
        "test_lineage_proof_evidence_rejects_duplicate_json_keys",
        "test_lineage_proof_evidence_redacts_secret_duplicate_json_key",
        "test_lineage_proof_evidence_rejects_nonfinite_json_constant",
        "test_stale_lineage_proof_evidence_blocks_rollup_section",
        "test_lineage_proof_evidence_rejects_noncanonical_timestamp",
        "test_lineage_proof_evidence_rejects_timestamp_string_shape",
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
        "test_kagemusha_release_bundle_verify_existing_rejects_future_dated_manifest_timestamp",
        "test_kagemusha_release_bundle_verify_existing_rejects_compact_generated_at_section_binding_drift",
        "test_kagemusha_release_bundle_rejects_noncanonical_summary_generated_at",
        "test_kagemusha_release_bundle_rejects_noncanonical_summary_section_generated_at",
        "test_kagemusha_release_bundle_rejects_noncanonical_summary_section_max_generated_at",
        "test_kagemusha_release_bundle_rejects_future_dated_summary_generated_at",
        "test_kagemusha_release_bundle_rejects_future_dated_summary_section_generated_at",
        "test_kagemusha_release_bundle_rejects_future_dated_android_summary_max_signed_at",
        "test_kagemusha_release_bundle_rejects_future_dated_android_signed_evidence_summary_slot",
        "test_kagemusha_release_bundle_rejects_future_dated_android_slot_kagemusha_timestamp",
        "test_kagemusha_release_bundle_rejects_android_min_signed_at_summary_drift",
        "test_release_bundle_relative_path_rejects_control_paths_before_resolve",
        "test_release_bundle_relative_path_rejects_aliases_before_resolve",
        "test_release_bundle_relative_path_rejects_bundle_root_aliases_before_resolve",
        "test_kagemusha_release_bundle_rejects_noncanonical_summary_path_before_json_load",
        "test_kagemusha_release_bundle_evidence_entries_reject_outside_root_before_hash",
        "test_release_bundle_build_rejects_unsafe_trusted_signer_map_before_bundle_root_metadata",
        "test_release_bundle_root_rejects_aliases_before_metadata",
        "test_kagemusha_release_bundle_rejects_noncanonical_bundle_root_before_load",
        "test_release_bundle_build_rejects_repo_root_aliases_before_bundle_root_metadata",
        "test_release_bundle_verify_rejects_unsafe_trusted_signer_map_before_manifest_load",
        "test_release_bundle_build_rejects_unsafe_repo_root_before_bundle_root_metadata",
        "test_release_bundle_verify_rejects_unsafe_repo_root_before_manifest_load",
        "test_kagemusha_release_bundle_verify_existing_rejects_positive_evidence_size_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_generator_log_artifact_digest_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_generator_log_artifact_size_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_compact_generator_log_evidence_size_drift",
        "test_kagemusha_release_bundle_rejects_generator_log_artifact_digest_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_digest_matched_invalid_utf8_proof_log",
        "test_kagemusha_release_bundle_verify_existing_rejects_digest_matched_invalid_utf8_generator_log",
        "test_kagemusha_release_bundle_verify_existing_rejects_unexpected_field",
        "test_kagemusha_release_bundle_verify_existing_rejects_missing_top_level_field",
        "test_kagemusha_release_bundle_verify_existing_rejects_nonstring_manifest_schema",
        "test_kagemusha_release_bundle_verify_existing_rejects_blocked_manifest",
        "test_kagemusha_release_bundle_verify_existing_rejects_nonboolean_ready_field",
        "test_kagemusha_release_bundle_verify_existing_rejects_nonarray_blockers_field",
        "test_kagemusha_release_bundle_verify_existing_rejects_secret_material",
        "test_kagemusha_release_bundle_verify_existing_rejects_unsafe_evidence_path",
        "test_kagemusha_release_bundle_verify_existing_rejects_nonstring_evidence_path",
        "test_kagemusha_release_bundle_verify_existing_rejects_noncanonical_evidence_path",
        "test_kagemusha_release_bundle_verify_existing_rejects_malformed_evidence_sha256",
        "test_kagemusha_release_bundle_verify_existing_rejects_noninteger_evidence_size",
        "test_kagemusha_release_bundle_verify_existing_rejects_boolean_evidence_size",
        "test_kagemusha_release_bundle_verify_existing_rejects_zero_evidence_size",
        "test_kagemusha_release_bundle_verify_existing_rejects_missing_evidence_size",
        "test_kagemusha_release_bundle_verify_existing_rejects_unexpected_evidence_group",
        "test_kagemusha_release_bundle_verify_existing_rejects_missing_evidence_entry_field",
        "test_kagemusha_release_bundle_verify_existing_rejects_unexpected_evidence_entry_field",
        "test_kagemusha_release_bundle_verify_existing_rejects_android_slot_artifact_kind",
        "test_kagemusha_release_bundle_verify_existing_rejects_missing_android_slot_artifact_kind",
        "test_kagemusha_release_bundle_verify_existing_rejects_unsafe_android_slot_artifact_kind_without_leak",
        "test_kagemusha_release_bundle_verify_existing_rejects_unsafe_android_signed_evidence_slot_without_leak",
        "test_kagemusha_release_bundle_verify_existing_rejects_unsafe_android_slot_artifact_slot_without_leak",
        "test_kagemusha_release_bundle_verify_existing_rejects_missing_lineage_artifact_inventory_key",
        "test_kagemusha_release_bundle_verify_existing_rejects_missing_lineage_proof_log_inventory_key",
        "test_kagemusha_release_bundle_verify_existing_rejects_missing_compact_key_artifact_inventory_key",
        "test_kagemusha_release_bundle_verify_existing_rejects_unexpected_lineage_artifact_inventory_key_without_leak",
        "test_kagemusha_release_bundle_verify_existing_rejects_unexpected_lineage_proof_log_inventory_key_without_leak",
        "test_kagemusha_release_bundle_verify_existing_rejects_unexpected_compact_key_artifact_inventory_key_without_leak",
        "test_kagemusha_release_bundle_verify_existing_rejects_readiness_summary_top_level_path_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_readiness_summary_top_level_digest_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_lineage_evidence_top_level_path_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_lineage_evidence_top_level_size_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_compact_evidence_top_level_path_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_compact_evidence_top_level_digest_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_android_evidence_slot_inventory_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_android_signed_evidence_digest_binding_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_android_signed_evidence_path_binding_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_android_signed_evidence_size_binding_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_android_signed_evidence_summary_timestamp_binding_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_android_signed_evidence_identity_binding_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_android_slot_artifact_digest_binding_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_android_slot_artifact_path_binding_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_android_slot_artifact_size_binding_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_lineage_artifact_section_binding_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_lineage_artifact_section_path_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_lineage_proof_log_section_binding_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_lineage_proof_log_section_path_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_compact_artifact_section_binding_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_compact_artifact_section_path_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_compact_generator_log_section_binding_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_compact_generator_log_section_path_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_missing_section",
        "test_kagemusha_release_bundle_verify_existing_rejects_missing_section_field",
        "test_kagemusha_release_bundle_verify_existing_rejects_unexpected_section_field",
        "test_kagemusha_release_bundle_verify_existing_rejects_section_state_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_nonstring_section_state",
        "test_kagemusha_release_bundle_verify_existing_rejects_abi6_section_value_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_abi6_limit_value_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_abi6_mode_value_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_abi7_circuit_value_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_section_timestamp",
        "test_kagemusha_release_bundle_verify_existing_rejects_future_dated_section_timestamp",
        "test_kagemusha_release_bundle_verify_existing_rejects_lineage_generated_at_section_binding_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_section_sha256",
        "test_kagemusha_release_bundle_verify_existing_rejects_section_size",
        "test_kagemusha_release_bundle_verify_existing_rejects_section_map_inventory",
        "test_kagemusha_release_bundle_verify_existing_rejects_checked_files_inventory",
        "test_kagemusha_release_bundle_verify_existing_rejects_section_list",
        "test_kagemusha_release_bundle_verify_existing_rejects_unexpected_android_field",
        "test_kagemusha_release_bundle_verify_existing_rejects_missing_android_duplicate_bindings",
        "test_kagemusha_release_bundle_verify_existing_rejects_malformed_android_duplicate_bindings",
        "test_kagemusha_release_bundle_verify_existing_rejects_forged_android_root",
        "test_kagemusha_release_bundle_verify_existing_rejects_noncanonical_android_duplicate_binding_slots",
        "test_kagemusha_release_bundle_verify_existing_rejects_repeated_android_duplicate_binding_value",
        "test_kagemusha_release_bundle_verify_existing_rejects_noncanonical_android_duplicate_binding_values",
        "test_kagemusha_release_bundle_verify_existing_rejects_unbound_android_duplicate_binding_slot",
        "test_kagemusha_release_bundle_verify_existing_rejects_android_duplicate_binding_summary_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_android_duplicate_binding_value_mismatch",
        "test_kagemusha_release_bundle_verify_existing_rejects_future_dated_android_signed_evidence_summary_slot",
        "test_kagemusha_release_bundle_verify_existing_rejects_bad_android_signer_digest",
        "test_kagemusha_release_bundle_verify_existing_rejects_android_signer_summary_drift",
        "test_kagemusha_release_bundle_verify_existing_rejects_android_untrusted_signer",
        "test_kagemusha_release_bundle_verify_existing_rejects_empty_android_signers",
        "test_kagemusha_release_bundle_verify_existing_rejects_incomplete_android_families",
        "test_kagemusha_release_bundle_verify_existing_rejects_unknown_android_family",
        "test_kagemusha_release_bundle_verify_existing_rejects_nonempty_android_missing_families",
        "test_kagemusha_release_bundle_rejects_empty_compact_generator_log_inventory",
        "test_kagemusha_release_bundle_verify_existing_rejects_duplicate_manifest_json_key",
        "test_kagemusha_release_bundle_verify_existing_rejects_nonfinite_manifest_json_constant",
        "test_kagemusha_release_bundle_verify_existing_rejects_oversized_manifest_json",
        "test_kagemusha_release_bundle_verify_existing_rejects_noncanonical_manifest_timestamp",
        "test_kagemusha_release_bundle_load_local_json_rejects_symlink_swap_after_preflight",
        "test_kagemusha_release_bundle_load_local_json_rejects_oversized_input",
        "test_kagemusha_release_bundle_verify_existing_rejects_bundle_root_symlink_before_manifest_load",
        "test_kagemusha_release_bundle_verify_existing_rejects_outside_manifest_before_scanners",
        "lineage_artifacts",
        "compact_key_artifacts",
        "lineage_proof_logs",
        "android_slot_artifacts",
        "test_kagemusha_release_bundle_rejects_missing_android_slot_apk_after_validation",
        "test_kagemusha_release_bundle_rejects_malformed_android_ready_summary_lists",
        "test_kagemusha_release_bundle_rejects_forged_android_summary_root",
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
        "test_kagemusha_release_bundle_rejects_nonboolean_ready_summary_field",
        "test_kagemusha_release_bundle_rejects_nonstring_status_summary_field",
        "test_kagemusha_release_bundle_rejects_nonarray_summary_blockers_field",
        "test_kagemusha_release_bundle_rejects_nonarray_summary_section_blockers_field",
        "test_kagemusha_release_bundle_rejects_nonboolean_summary_section_ok_field",
        "test_kagemusha_release_bundle_rejects_nonstring_summary_section_state_field",
        "test_kagemusha_release_bundle_rejects_unexpected_android_signed_evidence_summary_field",
        "test_kagemusha_release_bundle_rejects_nonlist_android_summary_slots",
        "test_kagemusha_release_bundle_rejects_unsafe_android_summary_slot_without_leak",
        "test_kagemusha_release_bundle_rejects_android_summary_slots_inventory_drift",
        "test_kagemusha_release_bundle_rejects_unexpected_android_summary_slot_field",
        "test_kagemusha_release_bundle_rejects_unexpected_android_summary_slot_kagemusha_field_without_leak",
        "test_kagemusha_release_bundle_rejects_android_summary_slot_missing_kagemusha_field",
        "test_kagemusha_release_bundle_rejects_android_summary_slot_malformed_kagemusha_digest",
        "test_kagemusha_release_bundle_rejects_android_summary_slot_kagemusha_binding_drift",
        "test_kagemusha_release_bundle_rejects_android_summary_slot_device_identity",
        "test_kagemusha_release_bundle_rejects_android_summary_slot_one_sided_identity",
        "test_kagemusha_release_bundle_rejects_android_summary_slot_unknown_codename_identity",
        "test_kagemusha_release_bundle_rejects_blank_android_summary_slot_device_identity",
        "test_kagemusha_release_bundle_rejects_android_summary_slot_device_family_inventory_drift",
        "test_kagemusha_release_bundle_rejects_android_summary_slot_errors",
        "test_kagemusha_release_bundle_rejects_android_summary_slot_missing_present_group",
        "test_kagemusha_release_bundle_rejects_android_summary_slot_invalid_file_count",
        "test_kagemusha_release_bundle_rejects_android_summary_slot_metadata_drift",
        "test_kagemusha_release_bundle_rejects_android_summary_identity_drift",
        "test_kagemusha_release_bundle_rejects_android_summary_untrusted_signer",
        "test_kagemusha_release_bundle_rejects_missing_android_signed_evidence_summary_field",
        "test_kagemusha_release_bundle_rejects_android_signed_evidence_identity_mismatch",
        "test_kagemusha_release_bundle_rejects_android_signed_evidence_one_sided_identity",
        "test_kagemusha_release_bundle_rejects_android_signed_evidence_unknown_codename_identity",
        "test_kagemusha_release_bundle_rejects_blank_android_signed_evidence_identity",
        "test_kagemusha_release_bundle_rejects_nonobject_android_signed_evidence_summary_entry",
        "test_kagemusha_release_bundle_rejects_unsafe_android_signed_evidence_summary_slot_without_leak",
        "test_kagemusha_release_bundle_rejects_malformed_android_signed_evidence_summary_sha256",
        "test_kagemusha_release_bundle_rejects_unsafe_android_signed_evidence_summary_path_without_leak",
        "test_kagemusha_release_bundle_rejects_android_signed_evidence_summary_path_drift",
        "test_kagemusha_release_bundle_rejects_noncanonical_android_signed_evidence_summary_timestamp",
        "test_kagemusha_release_bundle_rejects_missing_android_duplicate_bindings_summary",
        "test_kagemusha_release_bundle_rejects_unexpected_android_duplicate_binding_field",
        "test_kagemusha_release_bundle_rejects_malformed_android_duplicate_binding_digest",
        "test_kagemusha_release_bundle_rejects_singleton_android_duplicate_binding_slots",
        "test_kagemusha_release_bundle_rejects_repeated_android_duplicate_binding_slot",
        "test_kagemusha_release_bundle_rejects_noncanonical_android_duplicate_binding_slots",
        "test_kagemusha_release_bundle_rejects_repeated_android_duplicate_binding_value",
        "test_kagemusha_release_bundle_rejects_noncanonical_android_duplicate_binding_values",
        "test_kagemusha_release_bundle_rejects_unbound_android_duplicate_binding_slot",
        "test_kagemusha_release_bundle_rejects_secret_android_duplicate_binding_slot_without_leak",
        "test_kagemusha_release_bundle_rejects_android_duplicate_binding_summary_drift",
        "test_kagemusha_release_bundle_rejects_android_duplicate_binding_value_mismatch",
        "test_kagemusha_release_bundle_rejects_android_trusted_signer_summary_drift",
        "test_kagemusha_release_bundle_rejects_android_covered_families_summary_drift",
        "test_kagemusha_release_bundle_rejects_lineage_tests_summary_drift",
        "test_kagemusha_release_bundle_rejects_compact_record_namespace_summary_drift",
        "test_kagemusha_release_bundle_rejects_missing_android_signed_evidence_summary_slot",
        "test_kagemusha_release_bundle_rejects_extra_android_signed_evidence_summary_slot",
        "test_kagemusha_release_bundle_rejects_all_zero_lineage_artifact",
        "test_kagemusha_release_bundle_rejects_placeholder_compact_artifact",
        "test_kagemusha_release_bundle_rejects_all_placeholder_compact_prefixes",
        "test_kagemusha_release_bundle_rejects_all_zero_compact_artifact",
        "kagemusha_release_lineage_artifact_placeholder",
        "kagemusha_release_compact_artifact_placeholder",
        "compact_key_generator_log",
        "kagemusha_release_compact_generator_log_digest_drift",
        "test_kagemusha_release_bundle_rejects_summary_digest_drift",
        "test_kagemusha_release_bundle_rejects_malformed_summary_section_sha256_fields",
        "test_kagemusha_release_bundle_rejects_summary_section_inventory_drift",
        "test_kagemusha_release_bundle_rejects_generator_log_artifact_size_drift",
        "test_kagemusha_release_bundle_rejects_digest_matched_invalid_utf8_proof_log",
        "test_kagemusha_release_bundle_rejects_digest_matched_invalid_utf8_generator_log",
        "test_kagemusha_release_bundle_rejects_lineage_size_drift",
        "test_kagemusha_release_bundle_rejects_malformed_summary_section_size_fields",
        "test_kagemusha_release_bundle_rejects_lineage_timestamp_summary_drift",
        "test_kagemusha_release_bundle_rejects_lineage_runtime_keygen_summary_drift",
        "test_kagemusha_release_bundle_rejects_malformed_lineage_tests_summary",
        "test_kagemusha_release_bundle_rejects_compact_timestamp_summary_drift",
        "test_kagemusha_release_bundle_rejects_malformed_summary_section_object_fields",
        "test_kagemusha_release_bundle_rejects_malformed_summary_section_integer_map_fields",
        "test_kagemusha_release_bundle_rejects_malformed_summary_section_string_map_fields",
        "test_kagemusha_release_bundle_rejects_summary_section_semantic_map_inventory_drift",
        "test_kagemusha_release_bundle_rejects_nonstring_summary_section_string_field",
        "test_kagemusha_release_bundle_rejects_compact_command_validated_summary_drift",
        "test_kagemusha_release_bundle_rejects_nonboolean_summary_section_boolean_field",
        "test_kagemusha_release_bundle_rejects_android_summary_drift",
        "test_kagemusha_release_bundle_rejects_abi6_summary_drift",
        "test_kagemusha_release_bundle_rejects_abi7_summary_drift",
        "test_kagemusha_release_bundle_rejects_lineage_tooling_summary_drift",
        "test_kagemusha_release_bundle_rejects_malformed_lineage_tooling_checked_files_summary",
        "test_kagemusha_release_bundle_rejects_malformed_summary_section_integer_fields",
        "test_kagemusha_release_bundle_rejects_wrong_repo_root",
        "test_kagemusha_release_bundle_rejects_unexpected_summary_field",
        "test_kagemusha_release_bundle_rejects_missing_summary_field",
        "test_kagemusha_release_bundle_rejects_nonstring_summary_schema",
        "test_kagemusha_release_bundle_rejects_unexpected_summary_section_field",
        "test_kagemusha_release_bundle_rejects_missing_summary_section_field",
        "test_kagemusha_release_bundle_rejects_nonobject_summary_section",
        "test_kagemusha_release_bundle_rejects_nonobject_android_summary_section",
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
        "test_write_release_bundle_installs_private_file_permissions",
        "test_write_release_bundle_reports_temp_cleanup_failure_after_write_failure",
        "test_write_release_bundle_reports_temp_cleanup_failure_after_post_stage_validation_failure",
        "test_write_release_bundle_temp_cleanup_rejects_swapped_temp_file",
        "test_write_release_bundle_rejects_parent_directory_sync_failure_after_replace",
        "test_write_release_bundle_rejects_parent_directory_identity_swap_before_sync",
        "test_write_release_bundle_rejects_nonfinite_manifest_before_write",
        "test_write_release_bundle_rejects_oversized_manifest_before_write",
        "test_write_release_bundle_rejects_oversized_readback_after_replace",
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
        "test_kagemusha_release_bundle_rejects_noncanonical_output_path",
        "test_kagemusha_release_bundle_rejects_output_parent_symlink_after_create",
        "test_write_release_bundle_rejects_control_output_path_before_parent_create",
        "test_kagemusha_release_bundle_rejects_bundle_root_symlink",
        "test_kagemusha_release_bundle_rejects_bundle_root_symlink_ancestor_without_leak",
        "test_kagemusha_release_bundle_rejects_control_bundle_root_without_leak",
        "test_kagemusha_release_bundle_rejects_secret_summary_path_without_leak",
        "test_kagemusha_release_bundle_rejects_control_summary_path_without_leak",
        "test_kagemusha_release_bundle_rejects_secret_repo_root_without_leak",
        "test_release_bundle_build_redacts_zero_trusted_signer_digest_in_blocked_manifest",
        "test_release_bundle_build_rejects_non_mapping_trusted_signer_map_without_crash",
        "test_release_bundle_verify_redacts_zero_trusted_signer_digest_in_blocked_manifest",
        "test_release_bundle_verify_rejects_unrepresentable_trusted_signer_digest_without_crash",
        "test_kagemusha_release_bundle_rejects_missing_trusted_signer",
        "test_kagemusha_release_bundle_rejects_secret_signer_path_before_load",
        "test_kagemusha_release_bundle_rejects_control_signer_path_before_load",
        "test_kagemusha_release_bundle_rejects_control_verify_existing_path_without_leak",
        "test_kagemusha_release_bundle_rejects_noncanonical_verify_existing_path_before_load",
        "release_bundle.RELEASE_BUNDLE_SCHEMA",
        "test_lineage_proof_evidence_rejects_missing_local_proof_log_file",
        "test_lineage_proof_evidence_uses_log_validation_before_is_file_preflight",
        "test_lineage_proof_evidence_rejects_symlinked_local_proof_log_file",
        "test_lineage_proof_evidence_rejects_hardlinked_local_proof_log_file",
        "test_lineage_proof_log_rejects_secret_path_before_digest",
        "test_lineage_proof_log_rejects_oversized_open_file",
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
        "test_lineage_proof_evidence_document_validator_rejects_nonfinite_json_before_write",
        "test_lineage_proof_evidence_document_validator_rejects_temp_write_failure_after_preflight",
        "test_lineage_proof_evidence_document_validator_reports_temp_cleanup_failure_after_write_failure",
        "test_lineage_proof_evidence_document_validator_rejects_temp_cleanup_failure",
        "test_lineage_proof_evidence_document_validator_temp_cleanup_rejects_swap",
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
        "test_lineage_proof_write_evidence_installs_private_permissions",
        "test_lineage_proof_validate_evidence_document_installs_private_scratch_permissions",
        "test_lineage_proof_write_evidence_rejects_write_failure_after_preflight",
        "test_lineage_proof_write_evidence_rejects_nonfinite_json_before_write",
        "test_lineage_proof_write_evidence_rejects_oversized_json_before_write",
        "test_lineage_proof_write_evidence_preserves_existing_output_on_replace_failure",
        "test_lineage_proof_write_evidence_reports_temp_cleanup_failure_after_write_failure",
        "test_lineage_proof_write_evidence_reports_temp_cleanup_failure_after_post_stage_validation_failure",
        "test_lineage_proof_write_evidence_temp_cleanup_rejects_swapped_temp_file",
        "test_lineage_proof_write_evidence_rejects_parent_directory_sync_failure_after_replace",
        "test_lineage_proof_write_evidence_rejects_parent_directory_identity_swap_before_sync",
        "test_lineage_proof_write_evidence_rejects_readback_mismatch",
        "test_lineage_proof_write_evidence_rejects_readback_failure",
        "test_lineage_proof_write_evidence_rejects_oversized_readback_after_replace",
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
        "test_lineage_proof_input_validator_rejects_control_proof_log_before_artifact_dir_metadata",
        "test_lineage_proof_input_validator_rejects_alias_proof_log_before_metadata",
        "test_lineage_proof_input_validator_rejects_parent_resolve_failure",
        "test_lineage_proof_evidence_helper_rejects_log_without_test_name",
        "test_lineage_proof_evidence_helper_rejects_marker_stuffed_proof_log",
        "test_lineage_proof_evidence_helper_rejects_failed_proof_log",
        "test_summary_does_not_leak_trusted_signer_key_paths",
        "test_summary_does_not_leak_device_lab_root_path",
        "test_secret_looking_device_lab_root_blocks_without_leak",
        "test_readiness_cli_rejects_read_input_aliases_before_rollup",
        "test_trusted_signer_path_aliases_block_before_key_loading",
        "test_android_root_discovery_failure_blocks_rollup_without_traceback",
        "test_validate_repo_root_rejects_secret_path_directly_without_leak",
        "test_validate_repo_root_rejects_aliases_before_metadata",
        "test_validate_repo_root_rejects_metadata_failure_directly_without_leak",
        "test_main_rejects_repo_root_aliases_before_resolve_without_leak",
        "test_main_rejects_repo_root_resolve_failure_without_traceback",
        "test_trust_root_sections_reject_secret_repo_root_before_reads",
        "test_symlinked_repo_root_blocks_before_rollup_without_path_leak",
        "test_symlinked_repo_root_ancestor_blocks_before_rollup_without_path_leak",
        "test_symlinked_android_root_blocks_rollup_without_path_leak",
        "test_symlinked_android_root_ancestor_blocks_rollup_without_path_leak",
        "test_android_report_secret_material_is_redacted_before_summary",
        "test_android_report_control_material_is_redacted_before_summary",
        "test_android_report_nonfinite_numbers_are_redacted_before_summary",
        "test_android_report_redacted_key_collision_blocks_without_overwrite",
        "test_android_report_non_string_keys_are_normalized_before_summary",
        "test_android_report_unsupported_values_are_normalized_before_summary",
        "test_android_report_malformed_kagemusha_shape_blocks_without_traceback",
        "test_android_report_malformed_device_family_does_not_cover_matrix",
        "test_android_slot_summary_omits_incomplete_release_kagemusha_fields",
        "test_android_slot_summary_preserves_complete_release_kagemusha_fields",
        "test_android_slot_summary_requires_report_match_for_duplicate_slot_admission",
        "test_android_duplicate_bindings_summary_omits_incomplete_release_slots",
        "test_android_duplicate_bindings_summary_preserves_complete_release_slots",
        "test_android_matrix_rejects_noncanonical_direct_binding_digest",
        "test_android_matrix_rejects_zero_direct_binding_digest",
        "test_android_matrix_redacts_secret_direct_binding_digest",
        "test_android_matrix_redacts_unsafe_direct_duplicate_slots",
        "test_android_matrix_redacts_control_direct_binding_digest_slot",
        "test_android_signed_evidence_summary_rejects_malformed_direct_values",
        "test_android_signed_evidence_summary_rejects_missing_direct_values",
        "test_android_signed_evidence_summary_rejects_unsafe_direct_slot_keys",
        "test_android_signed_evidence_summary_rejects_duplicate_safe_slot_without_overwrite",
        "test_android_signed_evidence_summary_redacts_secret_direct_values",
        "test_android_rollup_rejects_unsafe_trusted_signer_map_before_root_classify",
        "test_android_rollup_redacts_zero_trusted_signer_digest_before_root_classify",
        "test_android_rollup_rejects_non_mapping_trusted_signer_map_without_crash",
        "test_android_rollup_rejects_unrepresentable_trusted_signer_digest_without_crash",
        "test_secret_looking_summary_out_blocks_before_write_without_leak",
        "test_write_summary_rejects_secret_path_before_direct_write",
        "test_write_summary_rejects_non_regular_output_leaf_before_write",
        "test_validate_summary_output_path_uses_lstat_before_parent_is_dir_preflight",
        "test_validate_summary_output_path_rejects_parent_metadata_failure",
        "test_validate_summary_output_path_rejects_aliases_before_parent_metadata",
        "test_write_summary_uses_lstat_before_parent_is_dir_preflight",
        "test_write_summary_rejects_parent_metadata_failure_before_write",
        "test_write_summary_rejects_file_metadata_failure_before_write",
        "test_write_summary_rejects_hardlink_metadata_failure_before_write",
        "test_write_summary_rejects_write_failure_after_preflight",
        "test_write_summary_rejects_nonfinite_json_before_write",
        "test_write_summary_rejects_oversized_json_before_write",
        "test_write_summary_rejects_oversized_readback_after_replace",
        "test_write_summary_preserves_existing_output_on_replace_failure",
        "test_write_summary_reports_temp_cleanup_failure_after_write_failure",
        "test_write_summary_reports_temp_cleanup_failure_after_post_stage_validation_failure",
        "test_write_summary_temp_cleanup_rejects_swapped_temp_file",
        "test_write_summary_rejects_parent_directory_sync_failure_after_replace",
        "test_write_summary_rejects_parent_directory_identity_swap_before_sync",
        "test_write_summary_rejects_symlink_swap_before_replace",
        "test_write_summary_installs_private_file_permissions",
        "test_write_summary_rejects_readback_mismatch",
        "test_write_summary_rejects_permissive_mode_after_replace",
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
        "Generating {} Reserved-lineage verifier key for `{}` opening_len={}",
        "Writing {} Reserved-lineage verifier key to {}",
        "Writing {} Reserved-lineage verifier record to {}",
        "Deriving {} Reserved-lineage proving key archive for `{}` opening_len={}",
        "Writing {} Reserved-lineage proving key archive to {}",
        "Writing {} Reserved-lineage key package to {}",
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
    "scripts/kagemusha_run_lineage_proof_staged.py": (
        "stdout=subprocess.PIPE",
        "process.stdout.read",
        "sys.stdout.buffer",
    ),
    "scripts/kagemusha_run_recursive_compact_keygen_staged.py": (
        "stdout=subprocess.PIPE",
        "process.stdout.read",
        "sys.stdout.buffer",
    ),
}

WORKFLOW_PATH = ".github/workflows/pr_kagemusha_payload_bench.yml"
WORKFLOW_REQUIREMENTS = (
    '"ci/check_kagemusha_production_readiness.sh"',
    '"scripts/check_android_device_lab_slot.py"',
    '"scripts/sign_android_device_lab_evidence.py"',
    '"scripts/kagemusha_android_device_lab_slot.py"',
    '"scripts/kagemusha_pull_android_device_lab_raw_slot.py"',
    '"scripts/kagemusha_android_attestation_report.py"',
    '"scripts/android_keystore_attestation.sh"',
    '"scripts/kagemusha_production_readiness.py"',
    '"scripts/kagemusha_lineage_proof_evidence.py"',
    '"scripts/kagemusha_recursive_compact_key_evidence.py"',
    '"scripts/kagemusha_run_lineage_proof_staged.py"',
    '"scripts/kagemusha_run_recursive_compact_keygen_staged.py"',
    '"scripts/kagemusha_finalize_lineage_proof_staged_run.py"',
    '"scripts/kagemusha_finalize_recursive_compact_key_staged_run.py"',
    '"scripts/kagemusha_release_bundle.py"',
    '"scripts/tests/check_android_device_lab_slot_test.py"',
    '"kotlin/offline-wallet-lab-app/**"',
    '"kotlin/settings.gradle.kts"',
    '"kotlin/gradle/libs.versions.toml"',
    '"kotlin/offline-wallet-android/src/androidTest/java/org/hyperledger/iroha/android/offline/KagemushaDeviceLabArtifactExportTest.java"',
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
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-instrumentation-harness",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-command-marker-specificity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-artifact-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-summary-strict-json",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-summary-size-limit",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-summary-parent-sync",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-summary-parent-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-summary-readback-symlink",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-summary-readback-hardlink",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-summary-readback-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-summary-private-permissions",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-summary-temp-cleanup-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-summary-digest-open-path",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-summary-digest-inventory",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-harness-result",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signed-harness-result",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signed-evidence-path-root",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signed-evidence-path-canonical",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-release-apk-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-minimum-os",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signed-device-identity-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-signed-evidence-summary-identity-fields",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-signed-evidence-summary-partial-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-signed-evidence-summary-partial-artifact-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-signed-evidence-summary-partial-core-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-signed-evidence-summary-incomplete-entry",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-signed-evidence-summary-slot-id",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-incomplete-slot-coverage",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-slot-summary-incomplete-kagemusha",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-duplicate-bindings-incomplete-slot-summary",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-abi6-probe-status-exactness",
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
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-summary-complete-evidence",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-summary-trusted-signer-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-summary-zero-trusted-signer-digest",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-duplicate-binding-zero-digest",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-zero-sha256-placeholders",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-source-zero-sha256-placeholders",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-trusted-signer-map-path-type",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-trusted-signer-map-container",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-trusted-signer-map-mixed-key-sort",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-parent-is-dir-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-parent-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-parent-create-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-post-create-parent-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-parent-sync-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-file-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-hardlink-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-write-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-temp-cleanup-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-temp-cleanup-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-strict-json-write",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-size-limit",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-readback-verification",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-readback-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-readback-size-limit",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-readback-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-post-write-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-direct-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-direct-control-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-output-direct-path-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-scan-slot-expected-dir-is-dir-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-scan-slot-artifact-count-is-file-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-scan-slot-sha-is-file-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-secret-redaction",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-root-direct-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-root-direct-control-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-root-direct-path-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-root-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-main-root-exists-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-rollup-root-exists-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-duplicate-json-keys",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-nonfinite-json-constants",
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
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-load-direct-control-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-load-direct-path-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-load-file-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-json-load-size-limit",
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
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-size-limit",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-artifact-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-relative-ancestor-is-symlink-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-file-shape-terminal",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-direct-helper-slot-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-direct-helper-slot-path-aliases",
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
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-pending-queue-shape",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-pending-queue-closed-schema",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-pending-queue-empty-after-handoff",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-telemetry-closed-schema",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-telemetry-identity-exactness",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-telemetry-app-package-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-id-safety",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-name-safety",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signed-artifact-schema",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signed-evidence-artifact-digest-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signed-evidence-artifact-size-limit",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signed-evidence-artifact-is-file-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signed-evidence-artifact-read-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signed-evidence-artifact-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-metadata-artifact-digest-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-metadata-artifact-size-limit",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-metadata-artifact-read-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-metadata-artifact-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-transcript-artifact-digest-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-required-text-artifact-read-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signature-verify",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-public-key-openssl-spawn-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-public-key-openssl-invalid-key",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signature-verify-staging-write-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-staged-bytes-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-staged-bytes-hardlink-readback",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signature-verify-tempdir-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signature-verify-spawn-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signed-evidence-canonical-payload-strict-json",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-canonical-payload-strict-json",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-signature-read-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-signature-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-signature-output-hardlink",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-signature-output-read-limit",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-signature-shape",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-signature-staging-write-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-signature-tempdir-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-signature-spawn-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-signature-invalid-private-key",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-write",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-strict-json-write",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-size-limit",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-json-write-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-ancestor",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-parent-is-dir-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-parent-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-parent-create-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-post-create-parent-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-parent-sync-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-resolve-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-dangling-output-alias",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-hardlink-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-file-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-digest-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-digest-parent-missing",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-digest-leaf-missing",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-digest-hardlink-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-digest-file-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-digest-size-limit",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-digest-read-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-output-digest-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-direct-output-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-manifest-write",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-manifest-size-limit",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-text-size-limit",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-text-write-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-temp-cleanup-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-temp-cleanup-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-readback-verification",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-direct-manifest-shape",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-slot-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-slot-parent-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-slot-artifact-digest-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-slot-artifact-size-limit",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-slot-artifact-hardlink-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-slot-artifact-file-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-slot-artifact-read-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-slot-artifact-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-artifact-digest-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-artifact-size-limit",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-digest-artifact-file-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-manifest-artifact-read-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-manifest-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-metadata-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-artifact-digests-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-direct-slot-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-direct-slot-path-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-signing-helper-json-output-path-aliases",
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
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-attestation-report",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-attestation-report-level-fields",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-attestation-report-result-level-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-attestation-report-result-status-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-attestation-status-exactness",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-attestation-result-slot-keymint-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-attestation-report-writer-physical-device",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-attestation-report-writer-parent-sync-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-attestation-report-writer-temp-cleanup-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-attestation-report-writer-private-permissions",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-overwrite",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-install-no-overwrite",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-install-top-level",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-install-parent-sync",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-install-directory-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-install-sync-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-install-cleanup-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-install-cleanup-report",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-temp-cleanup-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-temp-cleanup-report",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-install-rename-dir-fd",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-install-output-root-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-install-cleanup-dir-fd",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-install-slot-entry-dir-fd",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-path-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-allowed-artifacts",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-json-slot-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-d2d-offline",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-wallet-rollback",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-status-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-runtime-failure-marker",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-harness-challenge",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-harness-strongbox",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-harness-chain-length",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-harness-canonical",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-scanner-harness-canonical",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-attestation-report-challenge-canonical",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-attestation-report-chain-path-canonical",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-attestation-report-chain-source-path-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-attestation-report-harness-source-path-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-attestation-report-slot-id-canonical",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-attestation-report-identity-canonical",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-attestation-report-strongbox-level-canonical",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-attestation-report-chain-length-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-challenge-file-canonical",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-latest-slot-canonical",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-latest-query-canonical",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-latest-write-parent-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-latest-write-readback-symlink",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-latest-write-readback-hardlink",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-latest-write-readback-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-latest-write-private-permissions",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-latest-write-temp-cleanup-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-directory-collision",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-entry-cap",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-private-permissions",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-private-permissions",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-result-slot-required",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-result-chain-digest-required",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-result-challenge-digest-required",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-result-closed-schema",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-result-identity-strings",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-result-sdk-digests",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-result-strongbox-levels",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-raw-puller-blank-serial",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-signature-required",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-family-override-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-device-identity-fields",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-source-identity-fallback",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-source-open-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-root-path-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-source-path-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-copy-parent-sync-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-copy-readback",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-json-parent-sync-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-json-readback",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-json-temp-cleanup-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-publish-root-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-publish-stage-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-temp-cleanup-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-temp-cleanup-report",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-harness-canonical",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-report-app-package-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-result-closed-schema",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-report-closed-schema",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-report-verification-closed-schema",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-report-schema",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-report-verifier",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-d2d-closed-schema",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-wallet-closed-schema",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-d2d-schema",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-wallet-schema",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-d2d-semantic-validation",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-wallet-semantic-validation",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-required-artifact-validation",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-report-level-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-report-status-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-device-lab-slot-assembler-attestation-status-exactness",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-signed-evidence-freshness-report",
    "ci/check_kagemusha_production_readiness.sh --negative-control-android-signed-evidence-timestamp-raw",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-rollup",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-rollup-path-safety",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-trusted-signer-sanitization",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-android-report-secret-redaction",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-android-zero-binding-digest",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-repo-root-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-repo-root-direct-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-repo-root-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-repo-root-resolve-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-trust-root-section-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-android-root-discovery-read-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-release-json-direct-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-release-json-direct-path-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-release-json-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-json-read-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-json-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-source-marker-direct-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-source-marker-direct-path-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-source-marker-read-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-source-marker-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-source-marker-non-utf8-read",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-source-marker-size-limit",
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
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-temp-cleanup-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-temp-cleanup-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-strict-json-write",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-size-limit",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-readback-verification",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-readback-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-readback-size-limit",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-readback-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-private-permissions",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-parent-sync-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-summary-output-post-write-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-release-json-hardlink-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-release-json-file-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-kagemusha-readiness-release-json-size-limit",
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
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-local-path-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-evidence-helper-path-aliases",
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
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-temp-cleanup-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-temp-cleanup-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-strict-json-write",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-readback-verification",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-readback-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-readback-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-post-write-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-private-permissions",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-output-parent-create-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-output-file-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-output-hardlink-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-output-early-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-output-write-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-output-temp-cleanup-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-output-temp-cleanup-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-strict-json-write",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-output-readback-verification",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-output-readback-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-output-readback-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-output-parent-sync-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-output-post-write-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-output-private-permissions",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-validation-dir-create-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-validation-strict-json-write",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-validation-temp-write-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-validation-temp-cleanup-after-write-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-validation-temp-cleanup-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-validation-temp-cleanup-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-direct-artifact-dir-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-direct-artifact-dir-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-direct-hash-shape",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-direct-hash-read-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-generator-log-strict-read",
    "ci/check_kagemusha_production_readiness.sh --negative-control-staged-path-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-staged-runner-exit-marker",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-staged-runner-exit-marker",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-staged-runner-readback",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-staged-runner-readback",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-staged-runner-parent-sync-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-staged-runner-parent-sync-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-staged-runner-log-install-parent-sync-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-staged-runner-log-install-parent-sync-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-staged-runner-cleanup-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-staged-runner-cleanup-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-staged-runner-private-permissions",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-staged-runner-private-permissions",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-staged-runner-child-log-file",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-staged-runner-child-log-file",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-staged-runner-supervisor-output-pipe",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-staged-runner-supervisor-output-pipe",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-staged-runner-heartbeat",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-staged-runner-heartbeat",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-staged-runner-execution-log-sha256",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-staged-runner-execution-log-sha256",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-finalizer-execution-log-sha256",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-finalizer-execution-log-sha256",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-finalizer-execution-elapsed-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-staged-runner-resume-replace-conflict",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-staged-runner-resume-replace-conflict",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-finalizer-exit-marker",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-finalizer-exit-marker",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-finalizer-timestamp-raw",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-finalizer-timestamp-raw",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-finalizer-future-skew",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-finalizer-future-skew",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-finalizer-private-permissions",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-finalizer-private-permissions",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-finalizer-publish-readback",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-finalizer-publish-readback",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-finalizer-publish-rollback-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-finalizer-publish-rollback-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-finalizer-publish-rollback-cleanup-report",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-finalizer-publish-rollback-cleanup-report",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-finalizer-publish-dir-sync-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-finalizer-publish-dir-sync-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-finalizer-temp-cleanup-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-finalizer-temp-cleanup-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-finalizer-temp-cleanup-report",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-finalizer-temp-cleanup-report",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-direct-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-direct-hash-shape",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-direct-hash-read-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-direct-artifact-dir-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-direct-artifact-dir-metadata-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-direct-proof-log-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-direct-output-preflight-secret-paths",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-validation-dir-aliases",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-validation-dir-create-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-validation-strict-json-write",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-validation-temp-write-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-validation-temp-cleanup-after-write-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-validation-temp-cleanup-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-validation-temp-cleanup-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-input-corridor",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-input-corridor-resolve-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-output-corridor-resolve-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-command-canonical",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-command-canonical",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-scalar-types",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-scalar-types",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-artifact-size-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-evidence-json-size-limit",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-readiness-artifact-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-artifact-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-artifact-prefix-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-artifact-size-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-evidence-json-size-limit",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-readiness-artifact-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-artifact-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-artifact-prefix-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-placeholder-artifacts",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-generator-log-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-generator-log-digest-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-generator-log-size-limit",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-generator-log-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-timestamp-raw",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-timestamp-raw",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-summary-drift",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-summary-section-schema",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-android-signed-evidence-summary-schema",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-android-duplicate-binding-value-inventory",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-blocked-manifest-trusted-signer-sanitization",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-android-signed-evidence-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-android-slot-summary-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-android-signed-evidence-identity-drift",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-android-slot-identity-drift",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-manifest-android-signed-evidence-identity-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-evidence-inventory-schema",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-evidence-inventory-keysets",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-section-schema",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-android-manifest-schema",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-artifact-inventory",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-android-slot-artifact-inventory",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-compact-placeholder-inventory",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-compact-generator-log-inventory",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-evidence-entry-nonempty",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-evidence-entry-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-json-input-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-local-json-size-limit",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-digest-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-atomic-output",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-temp-cleanup-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-temp-cleanup-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-strict-json-write",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-output-size-limit",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-output-readback-failure",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-output-readback-size-limit",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-output-readback-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-output-private-permissions",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-output-parent-sync-identity",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-output-post-write-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-control-path-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-input-path-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-scan-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-output-overwrite",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-verify-existing",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-verify-existing-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-release-bundle-verify-existing-evidence-path-shape",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-timestamp-raw",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-helper-future-skew",
    "ci/check_kagemusha_production_readiness.sh --negative-control-compact-key-helper-future-skew",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-log-exact",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-log-size-limit",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-log-is-file-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-log-text-preflight",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-log-open-path-binding",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-evidence-filename",
    "ci/check_kagemusha_production_readiness.sh --negative-control-lineage-proof-evidence-output-parent-sync-identity",
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
    if manifest.get("native_bridge_abi_version") != 6:
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
            'def _validate_release_local_json_file_for_read(\n    path: Path,\n    label: str,\n) -> tuple[os.stat_result | None, list[str]]:\n    """Reject local release JSON files and return the read identity."""\n\n    path_text = str(path)\n    if device_lab.SECRET_RE.search(path_text):\n        return None, [f"{label} path must not contain secret-looking material"]\n',
            'def _validate_release_local_json_file_for_read(\n    path: Path,\n    label: str,\n) -> tuple[os.stat_result | None, list[str]]:\n    """Reject local release JSON files and return the read identity."""\n\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-release-json-direct-path-aliases":
    run_negative_control(
        "Kagemusha readiness release JSON direct path-alias gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    if "\\\\" in path_text:\n        return None, [f"{label} path must not contain backslashes"]\n    if ".." in path.parts:\n        return None, [f"{label} path must be canonical"]\n    release_json_ancestor_errors = device_lab.validate_no_symlink_ancestors(\n',
            '    release_json_ancestor_errors = device_lab.validate_no_symlink_ancestors(\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-release-json-hardlink-metadata-failure":
    run_negative_control(
        "Kagemusha readiness release JSON hardlink metadata failure gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        return None, [f"{label} hardlink metadata could not be read"]\n    if link_count > 1:\n        return None, [f"{label} must not be hardlinked"]\n',
            '    link_count = path.stat().st_nlink\n    if link_count > 1:\n        return None, [f"{label} must not be hardlinked"]\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-release-json-file-metadata-failure":
    run_negative_control(
        "Kagemusha readiness release JSON file metadata failure gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    release_json_ancestor_errors = device_lab.validate_no_symlink_ancestors(\n        path,\n        f"{label} ancestor directory",\n    )\n    if release_json_ancestor_errors:\n        return None, release_json_ancestor_errors\n    try:\n        file_stat = path.lstat()\n    except FileNotFoundError:\n        return None, [f"{label} is missing"]\n    except OSError:\n        return None, [f"{label} file metadata could not be read"]\n',
            '    release_json_ancestor_errors = device_lab.validate_no_symlink_ancestors(\n        path,\n        f"{label} ancestor directory",\n    )\n    if release_json_ancestor_errors:\n        return None, release_json_ancestor_errors\n    try:\n        file_stat = path.lstat()\n    except FileNotFoundError:\n        return None, [f"{label} is missing"]\n    except OSError:\n        return None, [f"{label} is missing"]\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-release-json-size-limit":
    run_negative_control(
        "Kagemusha readiness release JSON size limit",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "if open_stat.st_size > MAX_ABI6_MANIFEST_JSON_BYTES:",
            "if False and open_stat.st_size > MAX_ABI6_MANIFEST_JSON_BYTES:",
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
            "release_json_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)",
            "release_json_expected_identity = (open_stat.st_dev, open_stat.st_ino)",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-json-open-path-binding":
    run_negative_control(
        "Kagemusha readiness JSON open-path binding",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            'digest, text, read_errors = _sha256_text_file(\n        path,\n        label,\n        f"{label} could not be read",\n        max_bytes=max_bytes,\n        too_large_error=size_error,\n    )',
            'digest, text, read_errors = _sha256_text_file_unbound(\n        path,\n        label,\n        f"{label} could not be read",\n        max_bytes=max_bytes,\n        too_large_error=size_error,\n    )',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-source-marker-direct-secret-paths":
    run_negative_control(
        "Kagemusha readiness source marker direct secret-path gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            'def _validate_repo_source_marker_file_for_read(\n    path: Path,\n    label: str,\n) -> tuple[os.stat_result | None, list[str]]:\n    """Reject checked-in marker files that could alias external bytes."""\n\n    path_text = str(path)\n    if device_lab.SECRET_RE.search(path_text):\n        return None, [f"{label} path must not contain secret-looking material"]\n',
            'def _validate_repo_source_marker_file_for_read(\n    path: Path,\n    label: str,\n) -> tuple[os.stat_result | None, list[str]]:\n    """Reject checked-in marker files that could alias external bytes."""\n\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-source-marker-direct-path-aliases":
    run_negative_control(
        "Kagemusha readiness source marker direct path-alias gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    if "\\\\" in path_text:\n        return None, [f"{label} path must not contain backslashes"]\n    if ".." in path.parts:\n        return None, [f"{label} path must be canonical"]\n    errors = [\n',
            '    errors = [\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-source-marker-hardlink-metadata-failure":
    run_negative_control(
        "Kagemusha readiness source marker hardlink metadata failure gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    try:\n        link_count = path.stat().st_nlink\n    except OSError:\n        errors.append(f"{label} hardlink metadata could not be read")\n        return None, errors\n    if link_count > 1:\n        errors.append(f"{label} must not be hardlinked")\n    if errors:\n        return None, errors\n    return file_stat, []\n\n\ndef validate_repo_source_marker_file(path: Path, label: str) -> list[str]:',
            '    link_count = path.stat().st_nlink\n    if link_count > 1:\n        errors.append(f"{label} must not be hardlinked")\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-source-marker-file-metadata-failure":
    run_negative_control(
        "Kagemusha readiness source marker file metadata failure gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    try:\n        file_stat = path.lstat()\n    except FileNotFoundError:\n        errors.append(f"{label} is missing")\n        return None, errors\n    except OSError:\n        errors.append(f"{label} file metadata could not be read")\n        return None, errors\n    if stat.S_ISLNK(file_stat.st_mode):\n        errors.append(f"{label} must not be a symlink")\n        return None, errors\n    if not stat.S_ISREG(file_stat.st_mode):\n        errors.append(f"{label} must be a regular file")\n        return None, errors\n',
            '    try:\n        file_stat = path.lstat()\n    except FileNotFoundError:\n        errors.append(f"{label} is missing")\n        return None, errors\n    if stat.S_ISLNK(file_stat.st_mode):\n        errors.append(f"{label} must not be a symlink")\n        return None, errors\n    if not stat.S_ISREG(file_stat.st_mode):\n        errors.append(f"{label} must be a regular file")\n        return None, errors\n',
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

if mode == "--negative-control-kagemusha-readiness-source-marker-size-limit":
    run_negative_control(
        "Kagemusha readiness source marker size-limit gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "if open_stat.st_size > MAX_REPO_SOURCE_MARKER_BYTES:",
            "if False and open_stat.st_size > MAX_REPO_SOURCE_MARKER_BYTES:",
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

if mode == "--negative-control-android-device-lab-instrumentation-harness":
    run_negative_control(
        "Android device-lab instrumentation harness",
        lambda: override_text(
            "kotlin/offline-wallet-android/src/androidTest/java/org/hyperledger/iroha/android/offline/OfflineNoteTransferHandoffTest.java",
            "nearbyQrAndNfcTokenHandoffRoundTripFixtureBytes",
            "qrAndNfcTokenHandoffRoundTripDisabled",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-command-marker-specificity":
    run_negative_control(
        "Android device-lab raw command marker specificity",
        lambda: (
            override_text(
                "scripts/check_android_device_lab_slot.py",
                '    "org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProverTest",\n',
                '    "KagemushaRecursiveSpendProverTest",\n',
            ),
            override_text(
                "scripts/check_android_device_lab_slot.py",
                '    "org.hyperledger.iroha.android.offline.OfflineNoteTransferHandoffTest",\n',
                '    "OfflineNoteTransferHandoffTest",\n',
            ),
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

if mode == "--negative-control-android-device-lab-raw-puller-blank-serial":
    run_negative_control(
        "Android raw puller blank serial gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "if args.serial is not None:",
            "if args.serial:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-summary-strict-json":
    run_negative_control(
        "Android raw puller summary strict-JSON gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "raw pull summary output is not strict JSON",
            "raw pull summary output may contain non-finite JSON",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-summary-size-limit":
    run_negative_control(
        "Android raw puller summary size-limit gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "len(encoded) > device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-summary-parent-sync":
    run_negative_control(
        "Android raw puller summary parent-sync gate",
        lambda: override_text_all(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "raw pull summary output parent directory could not be synced",
            "raw pull summary output parent sync is optional",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-summary-parent-identity":
    run_negative_control(
        "Android raw puller summary parent identity gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "expected_identity=parent_identity",
            "expected_identity=None",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-summary-readback-symlink":
    run_negative_control(
        "Android raw puller summary readback symlink gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "raw pull summary output must not be a symlink after writing",
            "raw pull summary output symlink readback is accepted",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-summary-readback-hardlink":
    run_negative_control(
        "Android raw puller summary readback hardlink gate",
        lambda: override_text_all(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "raw pull summary output must not be hardlinked after writing",
            "raw pull summary output hardlink readback is accepted",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-summary-readback-identity":
    run_negative_control(
        "Android raw puller summary readback identity gate",
        lambda: override_text_all(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "raw pull summary output changed while being read back",
            "raw pull summary output path swaps are accepted during readback",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-summary-private-permissions":
    run_negative_control(
        "Android raw puller summary private permissions gate",
        lambda: override_text_all(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "raw pull summary output permissions must be 0600",
            "raw pull summary output may be world-readable",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-summary-temp-cleanup-identity":
    run_negative_control(
        "Android raw puller summary temp cleanup identity gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "_file_identity(temp_stat) != expected_identity",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-summary-digest-open-path":
    run_negative_control(
        "Android raw puller summary digest open-path gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "open_identity != expected_identity",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-summary-digest-inventory":
    run_negative_control(
        "Android raw puller summary digest inventory gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "raw artifact digest inventory must include every required artifact",
            "raw artifact digest inventory may omit artifacts",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-harness-result":
    run_negative_control(
        "Android device-lab raw harness-result contract",
        lambda: override_text_all(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "_validate_harness_result",
            "_trust_harness_result",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signed-harness-result":
    run_negative_control(
        "Android device-lab signed harness-result contract",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "attestation/harness-result.json challenge_hex digest must match slot.json attestation_challenge_sha256",
            "attestation/harness-result.json challenge_hex digest may differ from slot.json attestation_challenge_sha256",
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

if mode == "--negative-control-android-device-lab-signed-device-identity-binding":
    run_negative_control(
        "Android device-lab signed device identity binding",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "slot.json device_family must match device_model/device_codename",
            "slot.json device_family may differ from device_model/device_codename",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-abi6-probe-status-exactness":
    run_negative_control(
        "Android device-lab ABI-6 probe exact passed status gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '_require_status(metadata, "abi6_recursive_spend_jni_probe", {"passed"}, errors)',
            '_require_status(metadata, "abi6_recursive_spend_jni_probe", {"passed", "ok"}, errors)',
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

if mode == "--negative-control-android-device-lab-summary-complete-evidence":
    run_negative_control(
        "Android device-lab complete signed-evidence summary gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "require_complete_signed_evidence=require_complete_kagemusha",
            "require_complete_signed_evidence=False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-summary-trusted-signer-binding":
    run_negative_control(
        "Android device-lab summary trusted-signer binding",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "and signer_public_key_sha256 not in trusted_signer_public_key_sha256",
            "and False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-summary-zero-trusted-signer-digest":
    run_negative_control(
        "Android device-lab summary zero trusted-signer digest",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            'def _valid_trusted_signer_public_key_sha256(value: Any) -> bool:\n'
            '    return (\n'
            '        isinstance(value, str)\n'
            '        and SHA256_HEX_RE.fullmatch(value) is not None\n'
            '        and value != "0" * 64\n'
            '    )\n',
            'def _valid_trusted_signer_public_key_sha256(value: Any) -> bool:\n'
            '    return (\n'
            '        isinstance(value, str)\n'
            '        and SHA256_HEX_RE.fullmatch(value) is not None\n'
            '    )\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-duplicate-binding-zero-digest":
    run_negative_control(
        "Android device-lab duplicate-binding zero digest",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '                or value == "0" * 64\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-zero-sha256-placeholders":
    run_negative_control(
        "Android device-lab zero SHA-256 placeholder evidence",
        lambda: override_text_all(
            "scripts/check_android_device_lab_slot.py",
            '== "0" * 64',
            '== "__disabled_zero_sha256_placeholder_gate__"',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-source-zero-sha256-placeholders":
    def disable_android_source_zero_sha256_placeholder_gate() -> None:
        override_text_all(
            "scripts/kagemusha_android_device_lab_slot.py",
            '== "0" * 64',
            '== "__disabled_zero_sha256_placeholder_gate__"',
        )
        override_text_all(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            '== "0" * 64',
            '== "__disabled_zero_sha256_placeholder_gate__"',
        )

    run_negative_control(
        "Android device-lab source zero SHA-256 placeholder evidence",
        disable_android_source_zero_sha256_placeholder_gate,
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-trusted-signer-map-path-type":
    run_negative_control(
        "Android device-lab trusted-signer direct-map path type",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '        if not isinstance(public_key_path, Path):\n'
            '            errors.append("trusted signer public key path must be a pathlib Path")\n'
            '            continue\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-trusted-signer-map-container":
    run_negative_control(
        "Android device-lab trusted-signer direct-map container",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    if not isinstance(trusted_signer_public_keys, Mapping):\n'
            '        return ["trusted signer public key map must be a mapping"]\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-trusted-signer-map-mixed-key-sort":
    run_negative_control(
        "Android device-lab trusted-signer direct-map mixed-key sorting",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "key=_trusted_signer_digest_sort_key",
            "key=None",
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

if mode == "--negative-control-android-device-lab-json-output-parent-sync-identity":
    run_negative_control(
        "Android device-lab JSON summary output parent sync identity gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "expected_identity=parent_identity",
            "expected_identity=None",
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
            "            os.replace(tmp_path, path)\n",
            '            path.write_text(summary_text, encoding="utf-8")\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-json-output-temp-cleanup-failure":
    run_negative_control(
        "Android device-lab JSON summary output temp cleanup failure gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    except OSError:\n        return ["--json-out temporary file could not be removed"]\n',
            "    except OSError:\n        return []\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-json-output-temp-cleanup-identity":
    run_negative_control(
        "Android device-lab JSON summary output temp cleanup identity gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "_file_identity(temp_stat) != expected_identity",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-json-output-strict-json-write":
    run_negative_control(
        "Android device-lab JSON summary strict JSON write gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "json.dumps(summary, indent=2, allow_nan=False)",
            "json.dumps(summary, indent=2)",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-json-output-size-limit":
    run_negative_control(
        "Android device-lab JSON summary output size-limit gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    if len(summary_text.encode("utf-8")) > MAX_ANDROID_DEVICE_LAB_JSON_BYTES:\n        return [\n            "--json-out must be no more than "\n            f"{MAX_ANDROID_DEVICE_LAB_JSON_BYTES} bytes"\n        ]\n',
            "",
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

if mode == "--negative-control-android-device-lab-json-output-readback-size-limit":
    run_negative_control(
        "Android device-lab JSON summary output readback size-limit gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '            if open_stat.st_size > MAX_ANDROID_DEVICE_LAB_JSON_BYTES:\n                return None, [\n                    "--json-out must be no more than "\n                    f"{MAX_ANDROID_DEVICE_LAB_JSON_BYTES} bytes"\n                ]\n',
            "",
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
            '    if SECRET_RE.search(path_text):\n        return [f"{label} must not contain secret-looking material"]\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-json-output-direct-control-paths":
    run_negative_control(
        "Android device-lab direct JSON summary output control-path gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    if _contains_control_character(path_text):\n        return [f"{label} must not contain control characters"]\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-json-output-direct-path-aliases":
    run_negative_control(
        "Android device-lab direct JSON summary output path-alias gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    if "\\\\" in path_text:\n        return [f"{label} must not contain backslashes"]\n',
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
            '    if SECRET_RE.search(root_text):\n        return False, ["device-lab root path must not contain secret-looking material"]\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-root-direct-control-paths":
    run_negative_control(
        "Android device-lab direct root control-path gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    if _contains_control_character(root_text):\n        return False, ["device-lab root path must not contain control characters"]\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-root-direct-path-aliases":
    run_negative_control(
        "Android device-lab direct root path-alias gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    if "\\\\" in root_text:\n        return False, ["device-lab root path must not contain backslashes"]\n',
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

if mode == "--negative-control-android-device-lab-nonfinite-json-constants":
    run_negative_control(
        "Android device-lab non-finite JSON constant gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "parse_constant=_reject_nonfinite_json_constant",
            "parse_constant=float",
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
            "        if not ancestor.exists():\n            continue\n        try:\n            ancestor_mode = ancestor.stat().st_mode\n",
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
            '    if SECRET_RE.search(path_text):\n        errors.append(f"{label} path must not contain secret-looking material")\n        return None\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-json-load-direct-control-paths":
    run_negative_control(
        "Android device-lab JSON loader direct control-path gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    if _contains_control_character(path_text):\n        errors.append(f"{label} path must not contain control characters")\n        return None\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-json-load-direct-path-aliases":
    run_negative_control(
        "Android device-lab JSON loader direct path-alias gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    if "\\\\" in path_text:\n        errors.append(f"{label} path must not contain backslashes")\n        return None\n',
            "",
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

if mode == "--negative-control-android-device-lab-json-load-size-limit":
    run_negative_control(
        "Android device-lab JSON loader size-limit gate",
        lambda: override_text_all(
            "scripts/check_android_device_lab_slot.py",
            "open_stat.st_size > MAX_ANDROID_DEVICE_LAB_JSON_BYTES",
            "False",
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
            '    try:\n        manifest_stat = manifest_path.lstat()\n    except FileNotFoundError:\n        return entries, ["missing sha256sum.txt"]\n    except OSError:\n        return entries, ["sha256sum.txt file metadata could not be read"]\n',
            '    try:\n        manifest_stat = manifest_path.lstat()\n    except FileNotFoundError:\n        return entries, ["missing sha256sum.txt"]\n',
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

if mode == "--negative-control-android-device-lab-manifest-size-limit":
    run_negative_control(
        "Android device-lab manifest size-limit gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "open_stat.st_size > MAX_ANDROID_DEVICE_LAB_SHA256_MANIFEST_BYTES",
            "False",
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
            '    if SECRET_RE.search(path_text):\n        return ["slot path must not contain secret-looking material"]\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-direct-helper-slot-path-aliases":
    run_negative_control(
        "Android device-lab direct helper slot path-alias gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            '    if "\\\\" in path_text:\n        return ["slot path must not contain backslashes"]\n',
            "",
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
            'def _slot_root_entries(slot_path: Path, errors: list[str]) -> list[Path] | None:\n    try:\n        return sorted(slot_path.iterdir(), key=lambda entry: entry.name)\n    except OSError:\n        _append_error_once(errors, "slot directory could not be listed")\n        return None\n',
            'def _slot_root_entries(slot_path: Path, errors: list[str]) -> list[Path] | None:\n    return sorted(slot_path.iterdir(), key=lambda entry: entry.name)\n',
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
            '    display = _display_path(safe_relative)\n    artifact_path = slot_path / safe_relative\n    if _slot_relative_symlink_ancestor(slot_path, safe_relative) is not None:\n        return None, None, [\n            "sha256sum.txt references artifact under symlink directory "\n            f"{display}"\n        ]\n',
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
        lambda: override_text_all(
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
        lambda: override_text_all(
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
        lambda: override_text_all(
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

if mode == "--negative-control-android-device-lab-pending-queue-shape":
    run_negative_control(
        "Android device-lab pending queue shape gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "_validate_required_pending_queue_artifact(slot_path, errors)",
            "# unchecked pending queue shape",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-pending-queue-closed-schema":
    run_negative_control(
        "Android device-lab pending queue closed schema gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "queue/pending_queue.json contains unexpected field",
            "queue/pending_queue.json ignores unexpected field",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-pending-queue-empty-after-handoff":
    run_negative_control(
        "Android device-lab pending queue empty-after-handoff gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "queue/pending_queue.json pending_transactions must be empty after D2D handoff",
            "queue/pending_queue.json pending_transactions may remain queued after D2D handoff",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-telemetry-closed-schema":
    run_negative_control(
        "Android device-lab telemetry closed schema gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "telemetry/telemetry.json contains unexpected field",
            "telemetry/telemetry.json ignores unexpected field",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-telemetry-identity-exactness":
    run_negative_control(
        "Android device-lab telemetry identity exactness gate",
        lambda: override_text_all(
            "scripts/check_android_device_lab_slot.py",
            "_validate_telemetry_string",
            "_unchecked_telemetry_string",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-telemetry-app-package-binding":
    run_negative_control(
        "Android device-lab telemetry app-package binding gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "telemetry/telemetry.json app_package_name must match ",
            "telemetry/telemetry.json app_package_name may differ from ",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-status-event-closed-schema":
    run_negative_control(
        "Android device-lab status event closed schema gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "telemetry/status.ndjson line {line_no} contains unexpected field",
            "telemetry/status.ndjson line {line_no} ignores unexpected field",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-status-value-closed-schema":
    run_negative_control(
        "Android device-lab status value closed schema gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "telemetry/status.ndjson line {line_no} status must be ok",
            "telemetry/status.ndjson line {line_no} status may be advisory",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-status-slot-binding-required":
    run_negative_control(
        "Android device-lab status slot binding required gate",
        lambda: override_text_all(
            "scripts/check_android_device_lab_slot.py",
            "telemetry/status.ndjson line {line_no} slot_id must be a non-empty string",
            "telemetry/status.ndjson line {line_no} slot_id may be omitted",
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
        lambda: (
            override_text(
                "scripts/check_android_device_lab_slot.py",
                "slot directory name must not contain secret-looking material",
                "slot directory name may contain secret-looking material",
            ),
            override_text(
                "scripts/check_android_device_lab_slot.py",
                "slot directory name must not contain whitespace",
                "slot directory name may contain whitespace",
            ),
            override_text(
                "scripts/check_android_device_lab_slot.py",
                "slot directory name must not contain control characters",
                "slot directory name may contain control characters",
            ),
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

if mode == "--negative-control-android-device-lab-signed-evidence-artifact-size-limit":
    run_negative_control(
        "Android device-lab signed evidence artifact digest size-limit gate",
        lambda: (
            override_text_all(
                "scripts/check_android_device_lab_slot.py",
                "open_stat.st_size > max_bytes",
                "False",
            ),
            override_text_all(
                "scripts/check_android_device_lab_slot.py",
                "size > max_bytes",
                "False",
            ),
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

if mode == "--negative-control-android-device-lab-metadata-artifact-size-limit":
    run_negative_control(
        "Android device-lab metadata artifact digest size-limit gate",
        lambda: (
            override_text_all(
                "scripts/check_android_device_lab_slot.py",
                "open_stat.st_size > max_bytes",
                "False",
            ),
            override_text_all(
                "scripts/check_android_device_lab_slot.py",
                "size > max_bytes",
                "False",
            ),
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
        lambda: override_text_all(
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

if mode == "--negative-control-android-device-lab-staged-bytes-hardlink-readback":
    run_negative_control(
        "Android device-lab staged bytes hardlink readback gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "            if open_stat.st_nlink > 1:\n                return None, [verification_error]\n",
            "",
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

if mode == "--negative-control-android-device-lab-signed-evidence-canonical-payload-strict-json":
    run_negative_control(
        "Android device-lab signed evidence canonical payload strict JSON gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "signed evidence artifact signature payload is not strict JSON",
            "signed evidence artifact signature payload allows non-strict JSON",
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

if mode == "--negative-control-android-device-lab-signing-helper-canonical-payload-strict-json":
    run_negative_control(
        "Android device-lab signing helper canonical payload strict JSON gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "signed evidence payload is not strict JSON",
            "signed evidence payload allows non-strict JSON",
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

if mode == "--negative-control-android-device-lab-signing-helper-signature-output-hardlink":
    run_negative_control(
        "Android device-lab signed evidence helper signature output hardlink gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "            if open_stat.st_nlink > 1:\n                errors.append(\"signature output could not be read\")\n                return None\n",
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-signature-output-read-limit":
    run_negative_control(
        "Android device-lab signed evidence helper signature output read-limit gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "read_limit = device_lab.ED25519_SIGNATURE_BYTES + 1",
            "read_limit = 1024 * 1024",
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

if mode == "--negative-control-android-device-lab-signing-helper-output-strict-json-write":
    run_negative_control(
        "Android device-lab signed evidence helper strict JSON write gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "json.dumps(payload, indent=2, sort_keys=True, allow_nan=False)",
            "json.dumps(payload, indent=2, sort_keys=True)",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-output-size-limit":
    run_negative_control(
        "Android device-lab signed evidence helper output size-limit gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            'if len(text.encode("utf-8")) > device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES:',
            'if False and len(text.encode("utf-8")) > device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES:',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-json-write-failure":
    run_negative_control(
        "Android device-lab signed evidence helper JSON write-failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "            os.replace(tmp_path, path)\n",
            '            path.write_text(text, encoding="utf-8")\n',
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

if mode == "--negative-control-android-device-lab-signing-helper-output-parent-sync-identity":
    run_negative_control(
        "Android device-lab signed evidence helper output parent sync identity gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "expected_identity=parent_identity",
            "expected_identity=None",
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
        lambda: override_text_all(
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

if mode == "--negative-control-android-device-lab-signing-helper-output-digest-size-limit":
    run_negative_control(
        "Android device-lab signed evidence helper output digest size-limit gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "open_stat.st_size > byte_limit",
            "False",
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
            '    if device_lab.SECRET_RE.search(path_text):\n        return [f"{label} must not contain secret-looking material"]\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-json-output-path-aliases":
    run_negative_control(
        "Android device-lab signed evidence helper JSON output path-alias gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    if "\\\\" in path_text:\n        return [f"{label} must not contain backslashes"]\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-manifest-write":
    run_negative_control(
        "Android device-lab signed evidence helper manifest write gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            'return _write_text(\n        slot_path / "sha256sum.txt",',
            'return _write_text(\n        slot_path / "sha256sum.unchecked",',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-manifest-size-limit":
    run_negative_control(
        "Android device-lab signed evidence helper manifest size-limit gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "max_bytes=device_lab.MAX_ANDROID_DEVICE_LAB_SHA256_MANIFEST_BYTES,",
            "max_bytes=None,",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-text-size-limit":
    run_negative_control(
        "Android device-lab signed evidence helper text size-limit gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            'if len(text.encode("utf-8")) > byte_limit:',
            'if False and len(text.encode("utf-8")) > byte_limit:',
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

if mode == "--negative-control-android-device-lab-signing-helper-temp-cleanup-failure":
    run_negative_control(
        "Android device-lab signed evidence helper temp cleanup failure gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    except OSError:\n        return [f"{label} temporary file could not be removed"]\n',
            "    except OSError:\n        return []\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-temp-cleanup-identity":
    run_negative_control(
        "Android device-lab signed evidence helper temp cleanup identity gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            "_file_identity(temp_stat) != expected_identity",
            "False",
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

if mode == "--negative-control-android-device-lab-signing-helper-slot-artifact-size-limit":
    run_negative_control(
        "Android device-lab signed evidence helper slot artifact size-limit gate",
        lambda: (
            override_text_all(
                "scripts/sign_android_device_lab_evidence.py",
                "open_stat.st_size > artifact_max_bytes",
                "False",
            ),
            override_text_all(
                "scripts/sign_android_device_lab_evidence.py",
                "size > artifact_max_bytes",
                "False",
            ),
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
            '    artifact_path, artifact_stat, errors = _validate_manifest_artifact_for_digest(\n        slot_path,\n        relative,\n    )\n    if errors:\n        return None, errors\n    assert artifact_path is not None and artifact_stat is not None\n',
            "    artifact_path = slot_path / relative\n    artifact_stat = artifact_path.lstat()\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-manifest-artifact-size-limit":
    run_negative_control(
        "Android device-lab manifest artifact size-limit gate",
        lambda: (
            override_text_all(
                "scripts/check_android_device_lab_slot.py",
                "open_stat.st_size > max_bytes",
                "False",
            ),
            override_text_all(
                "scripts/check_android_device_lab_slot.py",
                "size > max_bytes",
                "False",
            ),
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
            '    except OSError:\n        return None, [\n            "sha256sum.txt references artifact that could not be read "\n            f"{display}"\n        ]\n',
            "    except OSError:\n        return None, []\n",
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
            '    path_errors = device_lab._slot_path_boundary_errors(slot_path)  # type: ignore[attr-defined]\n    if path_errors:\n        return path_errors\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-signing-helper-direct-slot-path-aliases":
    run_negative_control(
        "Android device-lab signed evidence helper direct metadata slot path-alias gate",
        lambda: override_text(
            "scripts/sign_android_device_lab_evidence.py",
            '    path_errors = device_lab._slot_path_boundary_errors(slot_path)  # type: ignore[attr-defined]\n    if path_errors:\n        return path_errors\n',
            "",
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

if mode == "--negative-control-android-device-lab-attestation-report":
    run_negative_control(
        "Android device-lab attestation verifier report gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "validate_attestation_report(slot_path, metadata, errors)",
            "validate_attestation_result(slot_path, metadata, errors)",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-attestation-report-level-fields":
    run_negative_control(
        "Android device-lab attestation verifier report level-field gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            'for level_key in (\n        "keymint_security_level",\n        "attestation_security_level",\n        "keymaster_security_level",\n    ):\n        value = _attestation_report_verification_string(verification, level_key, errors)',
            'for level_key in (\n        "keymint_security_level",\n    ):\n        value = _attestation_report_verification_string(verification, level_key, errors)',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-attestation-report-result-level-binding":
    run_negative_control(
        "Android device-lab attestation verifier report/result level binding",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "and result_level != report_level",
            "and False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-attestation-report-result-status-binding":
    run_negative_control(
        "Android device-lab attestation verifier report/result status binding",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "and result_status != report_status",
            "and False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-attestation-status-exactness":
    run_negative_control(
        "Android device-lab attestation exact ok status gate",
        lambda: override_text_all(
            "scripts/check_android_device_lab_slot.py",
            'if status is not None and status != "ok":',
            'if status is not None and status not in {"ok", "passed"}:',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-attestation-result-slot-keymint-binding":
    run_negative_control(
        "Android device-lab attestation result slot KeyMint binding",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "attestation/result.json keymint_security_level must match",
            "attestation/result.json keymint_security_level may differ from",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-attestation-report-writer-physical-device":
    run_negative_control(
        "Android attestation report writer physical-device assertion gate",
        lambda: override_text(
            "scripts/kagemusha_android_attestation_report.py",
            "physical device attestation must be explicitly asserted with",
            "physical device attestation is optional for local reports",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-attestation-report-writer-parent-sync-identity":
    run_negative_control(
        "Android attestation report writer parent sync identity gate",
        lambda: override_text(
            "scripts/kagemusha_android_attestation_report.py",
            "expected_identity=parent_identity",
            "expected_identity=None",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-attestation-report-writer-temp-cleanup-identity":
    run_negative_control(
        "Android attestation report writer temp cleanup identity gate",
        lambda: override_text(
            "scripts/kagemusha_android_attestation_report.py",
            "device_lab._file_identity(temp_stat) != expected_identity",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-attestation-report-writer-private-permissions":
    run_negative_control(
        "Android attestation report writer private permissions gate",
        lambda: (
            override_text(
                "scripts/kagemusha_android_attestation_report.py",
                "os.fchmod(dir_fd, 0o700)",
                "os.fstat(dir_fd)",
            ),
            override_text(
                "scripts/kagemusha_android_attestation_report.py",
                "os.fchmod(handle.fileno(), 0o600)",
                "handle.fileno()",
            ),
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-overwrite":
    run_negative_control(
        "Android raw puller overwrite refusal gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "slot directory already exists; refuse to overwrite raw evidence",
            "slot directory already exists; replacing raw evidence",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-install-no-overwrite":
    run_negative_control(
        "Android raw puller install-time overwrite refusal gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "final_slot.mkdir(mode=0o700)",
            "final_slot.mkdir(mode=0o700, exist_ok=True)",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-install-top-level":
    run_negative_control(
        "Android raw puller install top-level allowlist gate",
        lambda: override_text_all(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "raw slot install source contains unexpected top-level entry",
            "raw slot install source accepts unexpected top-level entry",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-install-parent-sync":
    run_negative_control(
        "Android raw puller install parent-sync gate",
        lambda: override_text_all(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "raw slot directory parent could not be synced",
            "raw slot directory parent sync is optional",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-install-directory-identity":
    run_negative_control(
        "Android raw puller install directory-identity gate",
        lambda: override_text_all(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "raw slot directory changed during install",
            "raw slot directory identity drift is accepted",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-install-sync-identity":
    run_negative_control(
        "Android raw puller install sync identity gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "if _file_identity(open_stat) != expected_identity:",
            "if False:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-install-cleanup-identity":
    run_negative_control(
        "Android raw puller install cleanup identity gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "and _file_identity(path_stat) == expected_identity",
            "and True",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-install-cleanup-report":
    run_negative_control(
        "Android raw puller install cleanup report gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "return [*install_errors, *cleanup_errors]",
            "return install_errors",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-temp-cleanup-identity":
    run_negative_control(
        "Android raw puller temp cleanup identity gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "_file_identity(temp_parent_stat) != expected_identity",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-install-rename-dir-fd":
    run_negative_control(
        "Android raw puller install rename dir-fd gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "src_dir_fd=stage_fd,\n                            dst_dir_fd=final_fd,",
            "src_dir_fd=None,\n                            dst_dir_fd=None,",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-install-output-root-identity":
    run_negative_control(
        "Android raw puller install output-root identity gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "expected_identity=output_root_identity",
            "expected_identity=None",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-install-cleanup-dir-fd":
    run_negative_control(
        "Android raw puller install cleanup dir-fd gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "shutil.rmtree(path.name, dir_fd=parent_fd)",
            "shutil.rmtree(path)",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-install-slot-entry-dir-fd":
    run_negative_control(
        "Android raw puller install slot-entry dir-fd gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "_slot_entry_identity(\n        final_slot,\n        output_root,\n        output_root_identity",
            "_created_slot_identity_errors(\n        final_slot,\n        final_slot_identity",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-allowed-artifacts":
    run_negative_control(
        "Android raw puller closed artifact set gate",
        lambda: override_text_all(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "raw slot artifact {relative} is not an allowed path",
            "raw slot artifact {relative} may be an unreviewed debug path",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-json-slot-binding":
    run_negative_control(
        "Android raw puller JSON slot-binding gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "def _validate_raw_json_slot_id",
            "def _normalise_raw_json_slot_id",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-d2d-offline":
    run_negative_control(
        "Android raw puller D2D offline-offline gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            '"transport_offline"',
            '"transport_online_optional"',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-wallet-rollback":
    run_negative_control(
        "Android raw puller wallet rollback-rejection gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            '"rollback_rejection_passed"',
            '"rollback_rejection_optional"',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-status-failure":
    run_negative_control(
        "Android raw puller status failure gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "device_lab.KAGEMUSHA_STATUS_FAILURE_VALUES",
            "set()",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-runtime-failure-marker":
    run_negative_control(
        "Android raw puller runtime failure-marker gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "device_lab.KAGEMUSHA_RUNTIME_LOG_FAILURE_MARKERS",
            "()",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-harness-challenge":
    run_negative_control(
        "Android raw puller harness challenge binding gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "attestation/harness-result.json challenge_hex must match attestation/challenge.hex",
            "attestation/harness-result.json challenge_hex may differ from attestation/challenge.hex",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-harness-strongbox":
    run_negative_control(
        "Android raw puller harness StrongBox claim gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "attestation/harness-result.json strongbox_attestation must be true",
            "attestation/harness-result.json strongbox_attestation may be false",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-harness-chain-length":
    run_negative_control(
        "Android raw puller harness certificate-chain length binding gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "attestation/harness-result.json chain_length must match",
            "attestation/harness-result.json chain_length may differ from",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-harness-canonical":
    run_negative_control(
        "Android raw puller harness canonical string gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "attestation/harness-result.json challenge_hex must be lowercase hexadecimal without whitespace",
            "attestation/harness-result.json challenge_hex may be normalized",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-scanner-harness-canonical":
    run_negative_control(
        "Android scanner harness canonical string gate",
        lambda: override_text(
            "scripts/check_android_device_lab_slot.py",
            "if level is not None and level not in STRONGBOX_LEVELS:",
            "if level is not None and level.upper() not in STRONGBOX_LEVELS:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-attestation-report-challenge-canonical":
    run_negative_control(
        "Android attestation report canonical challenge gate",
        lambda: override_text(
            "scripts/kagemusha_android_attestation_report.py",
            'if any(ch not in "0123456789abcdef" for ch in value):',
            "if False:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-attestation-report-chain-path-canonical":
    run_negative_control(
        "Android attestation report canonical chain path gate",
        lambda: override_text(
            "scripts/kagemusha_android_attestation_report.py",
            "elif raw != raw.strip() or any(ch.isspace() for ch in raw):",
            "elif False:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-attestation-report-chain-source-path-aliases":
    run_negative_control(
        "Android attestation report chain source path alias gate",
        lambda: override_text(
            "scripts/kagemusha_android_attestation_report.py",
            '    if "\\\\" in path_text:\n'
            '        errors.append(f"{label} path must not contain backslashes")\n'
            '        return None, None\n'
            '    if ".." in path.parts:\n'
            '        errors.append(f"{label} path must be canonical")\n'
            '        return None, None\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-attestation-report-harness-source-path-aliases":
    run_negative_control(
        "Android attestation report harness-result source path alias gate",
        lambda: override_text(
            "scripts/kagemusha_android_attestation_report.py",
            'result = device_lab._load_json(path, "attestation harness result", errors)',
            'result = json.loads(path.read_text(encoding="utf-8"))',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-path-aliases":
    run_negative_control(
        "Android raw puller path-alias gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "raw output root path must not contain backslashes",
            "raw output root path may contain backslashes",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-attestation-report-slot-id-canonical":
    run_negative_control(
        "Android attestation report slot id canonical gate",
        lambda: override_text(
            "scripts/kagemusha_android_attestation_report.py",
            "if _reject_whitespace(value, label, errors):\n        return None\n    if _reject_control(value, label, errors):\n        return None\n    candidate = PurePosixPath(value)",
            "if False:\n        return None\n    if False:\n        return None\n    candidate = PurePosixPath(value.strip())",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-attestation-report-identity-canonical":
    run_negative_control(
        "Android attestation report identity string canonical gate",
        lambda: override_text(
            "scripts/kagemusha_android_attestation_report.py",
            "if _reject_whitespace(value, label, errors):\n        return None\n    if _reject_control(value, label, errors):\n        return None\n    if device_lab.SECRET_RE.search(value):",
            "if False:\n        return None\n    if False:\n        return None\n    if device_lab.SECRET_RE.search(value):",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-attestation-report-strongbox-level-canonical":
    run_negative_control(
        "Android attestation report StrongBox level canonical gate",
        lambda: override_text(
            "scripts/kagemusha_android_attestation_report.py",
            "if _reject_whitespace(value, label, errors):\n        return None\n    if _reject_control(value, label, errors):\n        return None\n    if value not in device_lab.STRONGBOX_LEVELS:",
            "if False:\n        return None\n    if False:\n        return None\n    if value.strip().upper() not in device_lab.STRONGBOX_LEVELS:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-attestation-report-chain-length-binding":
    run_negative_control(
        "Android attestation report chain-length binding gate",
        lambda: override_text(
            "scripts/kagemusha_android_attestation_report.py",
            "elif chain_length != certificate_count:",
            "elif False:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-challenge-file-canonical":
    run_negative_control(
        "Android raw puller challenge file canonical gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            'challenge_text.count("\\n") != 1',
            'False',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-latest-slot-canonical":
    run_negative_control(
        "Android raw puller latest-slot canonical binding gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            'latest_text != f"{slot_id}\\n"',
            "latest_text.strip() != slot_id",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-latest-query-canonical":
    run_negative_control(
        "Android raw puller latest-slot query canonical gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            'latest_text.count("\\n") != 1',
            'False',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-latest-write-parent-identity":
    run_negative_control(
        "Android raw puller latest-slot writer parent identity gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "expected_identity=root_identity",
            "expected_identity=None",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-latest-write-readback-symlink":
    run_negative_control(
        "Android raw puller latest-slot writer symlink readback gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "raw latest-slot output must not be a symlink after writing",
            "raw latest-slot output symlink readback is accepted",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-latest-write-readback-hardlink":
    run_negative_control(
        "Android raw puller latest-slot writer hardlink readback gate",
        lambda: override_text_all(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "raw latest-slot output must not be hardlinked after writing",
            "raw latest-slot output hardlink readback is accepted",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-latest-write-readback-identity":
    run_negative_control(
        "Android raw puller latest-slot writer identity readback gate",
        lambda: override_text_all(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "raw latest-slot output changed while being read back",
            "raw latest-slot output path swaps are accepted during readback",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-latest-write-private-permissions":
    run_negative_control(
        "Android raw puller latest-slot writer private permissions gate",
        lambda: override_text_all(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "raw latest-slot output permissions must be 0600",
            "raw latest-slot output may be world-readable",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-latest-write-temp-cleanup-identity":
    run_negative_control(
        "Android raw puller latest-slot writer temp cleanup identity gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "_file_identity(temp_stat) != expected_identity",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-directory-collision":
    run_negative_control(
        "Android raw puller tar directory collision gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "raw slot tar directory {relative} could not be created",
            "raw slot tar directory collisions are ignored",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-entry-cap":
    run_negative_control(
        "Android raw puller tar entry-cap gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "entry_count += 1",
            "entry_count += 0",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-private-permissions":
    run_negative_control(
        "Android raw puller private extracted-artifact permissions gate",
        lambda: (
            override_text(
                "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
                "os.fchmod(dir_fd, 0o700)",
                "os.fstat(dir_fd)",
            ),
            override_text(
                "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
                "os.fchmod(output.fileno(), 0o600)",
                "output.fileno()",
            ),
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-private-permissions":
    run_negative_control(
        "Android slot assembler private published-artifact permissions gate",
        lambda: (
            override_text(
                "scripts/kagemusha_android_device_lab_slot.py",
                "os.fchmod(dir_fd, 0o700)",
                "os.fstat(dir_fd)",
            ),
            override_text(
                "scripts/kagemusha_android_device_lab_slot.py",
                "os.fchmod(out.fileno(), 0o600)",
                "out.fileno()",
            ),
            override_text(
                "scripts/sign_android_device_lab_evidence.py",
                "os.fchmod(handle.fileno(), 0o600)",
                "handle.fileno()",
            ),
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-result-slot-required":
    run_negative_control(
        "Android raw puller attestation result slot-required gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            'result.get("slot") != slot_id',
            'result.get("slot") not in (None, slot_id)',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-result-chain-digest-required":
    run_negative_control(
        "Android raw puller attestation result chain digest-required gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            'RAW_RESULT_CHAIN_DIGEST_FIELD = "attestation_certificate_chain_sha256"',
            'RAW_RESULT_CHAIN_DIGEST_FIELD = "attestation_certificate_chain_digest_optional"',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-result-challenge-digest-required":
    run_negative_control(
        "Android raw puller attestation result challenge digest-required gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            'RAW_RESULT_CHALLENGE_DIGEST_FIELD = "attestation_challenge_sha256"',
            'RAW_RESULT_CHALLENGE_DIGEST_FIELD = "attestation_challenge_digest_optional"',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-result-closed-schema":
    run_negative_control(
        "Android raw puller attestation result closed-schema gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "attestation/result.json contains unexpected field",
            "attestation/result.json may contain debug fields",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-result-identity-strings":
    run_negative_control(
        "Android raw puller attestation result identity string gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "def _validate_raw_result_string",
            "_normalise_raw_result_string",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-result-sdk-digests":
    run_negative_control(
        "Android raw puller attestation result SDK digest gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "for field in RAW_RESULT_SHA256_FIELDS:",
            "for field in ():",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-result-strongbox-levels":
    run_negative_control(
        "Android raw puller attestation result StrongBox-level gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "for field in RAW_RESULT_STRONGBOX_FIELDS:",
            "for field in ():",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-signature-required":
    run_negative_control(
        "Android device-lab slot assembler signing-required gate",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "signing inputs are required unless --allow-unsigned is set",
            "signing inputs are optional by default",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-family-override-binding":
    run_negative_control(
        "Android device-lab slot assembler requested-family binding gate",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            '        if has_device_identity and inferred != family:\n            errors.append("device family must match attached device model/codename")\n            return None\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-device-identity-fields":
    run_negative_control(
        "Android device-lab slot assembler device identity fields",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            '        "device_model": facts["device_model"],\n        "device_codename": facts["device_codename"],\n',
            '        "device_model": family,\n        "device_codename": family,\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-source-identity-fallback":
    run_negative_control(
        "Android device-lab slot assembler source identity fallback",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "identity_hints=identity_hints",
            "identity_hints={}",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-source-identity-conflict":
    run_negative_control(
        "Android device-lab slot assembler source identity conflict",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "if hints[key] != value:",
            "if False and hints[key] != value:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-blank-source-identity":
    run_negative_control(
        "Android device-lab slot assembler blank source identity",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            'if value == "":\n        errors.append(f"{label} {key} must be a non-empty string")\n        return None',
            'if value == "":\n        return None',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-blank-identity-override":
    run_negative_control(
        "Android device-lab slot assembler blank identity override",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            'if override == "":\n        errors.append(f"{key} must be a non-empty string")\n        return None',
            'if override == "":\n        return None',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-override-source-identity-binding":
    run_negative_control(
        "Android device-lab slot assembler override source identity binding",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "if value is not None and hint_value is not None and value != hint_value:",
            "if False and value is not None and hint_value is not None and value != hint_value:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-source-open-binding":
    run_negative_control(
        "Android device-lab slot assembler source open-path binding",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "open_identity != expected_identity or path_identity != expected_identity",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-root-path-aliases":
    run_negative_control(
        "Android device-lab slot assembler root path-alias gate",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            '    if "\\\\" in root_text:\n        return 1, None, ["device-lab root path must not contain backslashes"]\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-source-path-aliases":
    run_negative_control(
        "Android device-lab slot assembler source path-alias gate",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            '    if "\\\\" in path_text:\n        errors.append(f"{label} path must not contain backslashes")\n        return None\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-copy-parent-sync-identity":
    run_negative_control(
        "Android device-lab slot assembler copy parent sync identity",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "expected_identity=destination_parent_identity",
            "expected_identity=None",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-copy-readback":
    run_negative_control(
        "Android device-lab slot assembler copy readback",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "if verify_errors:",
            "if False:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-json-parent-sync-identity":
    run_negative_control(
        "Android device-lab slot assembler JSON parent sync identity",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "expected_identity=json_parent_identity",
            "expected_identity=None",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-json-readback":
    run_negative_control(
        "Android device-lab slot assembler JSON readback",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "return _verify_written_bytes(path, encoded, label)",
            "return []",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-json-temp-cleanup-identity":
    run_negative_control(
        "Android device-lab slot assembler JSON temp cleanup identity",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "_file_identity(temp_stat) != expected_identity",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-publish-root-identity":
    run_negative_control(
        "Android device-lab slot assembler publish root identity",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "_file_identity(root_stat) != expected_root_identity",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-raw-puller-temp-cleanup-report":
    run_negative_control(
        "Android raw puller temp cleanup report gate",
        lambda: override_text(
            "scripts/kagemusha_pull_android_device_lab_raw_slot.py",
            "if pull_errors or cleanup_errors:",
            "if pull_errors:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-publish-stage-identity":
    run_negative_control(
        "Android device-lab slot assembler publish staged-slot identity",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "_file_identity(stage_stat) != expected_stage_identity",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-temp-cleanup-identity":
    run_negative_control(
        "Android device-lab slot assembler temporary cleanup identity",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "_file_identity(temp_parent_stat) != expected_identity",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-temp-cleanup-report":
    run_negative_control(
        "Android device-lab slot assembler temporary cleanup report",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "if stage_errors or cleanup_errors:",
            "if stage_errors:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-harness-canonical":
    run_negative_control(
        "Android device-lab slot assembler harness canonical string gate",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "if level is not None and level not in device_lab.STRONGBOX_LEVELS:",
            "if level is not None and level.upper() not in device_lab.STRONGBOX_LEVELS:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-report-app-package-binding":
    run_negative_control(
        "Android device-lab slot assembler report app-package binding",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "and result_app_package != report_app_package",
            "and False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-result-closed-schema":
    run_negative_control(
        "Android device-lab slot assembler attestation result closed schema",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "set(attestation_result) - device_lab.ATTESTATION_RESULT_FIELDS",
            "set()",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-report-closed-schema":
    run_negative_control(
        "Android device-lab slot assembler attestation report closed schema",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "set(attestation_report) - device_lab.ATTESTATION_REPORT_FIELDS",
            "set()",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-report-verification-closed-schema":
    run_negative_control(
        "Android device-lab slot assembler attestation report verification closed schema",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "set(verification) - device_lab.ATTESTATION_REPORT_VERIFICATION_FIELDS",
            "set()",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-report-schema":
    run_negative_control(
        "Android device-lab slot assembler attestation report schema",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "if report_schema != device_lab.ATTESTATION_REPORT_SCHEMA:",
            "if False:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-report-verifier":
    run_negative_control(
        "Android device-lab slot assembler attestation report verifier",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            '_require_source_string(attestation_report, "verifier", "attestation/report.json", errors)',
            '# unchecked attestation report verifier',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-d2d-closed-schema":
    run_negative_control(
        "Android device-lab slot assembler D2D transcript closed schema",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "set(d2d_payment_transcript) - device_lab.D2D_PAYMENT_TRANSCRIPT_FIELDS",
            "set()",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-wallet-closed-schema":
    run_negative_control(
        "Android device-lab slot assembler wallet transcript closed schema",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "set(wallet_integrity_transcript) - device_lab.WALLET_INTEGRITY_TRANSCRIPT_FIELDS",
            "set()",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-d2d-schema":
    run_negative_control(
        "Android device-lab slot assembler D2D transcript schema",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "if d2d_schema != device_lab.D2D_PAYMENT_TRANSCRIPT_SCHEMA:",
            "if False:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-wallet-schema":
    run_negative_control(
        "Android device-lab slot assembler wallet transcript schema",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "if wallet_schema != device_lab.WALLET_INTEGRITY_TRANSCRIPT_SCHEMA:",
            "if False:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-d2d-semantic-validation":
    run_negative_control(
        "Android device-lab slot assembler D2D transcript semantic validation",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "device_lab.validate_d2d_payment_transcript(",
            "device_lab.unchecked_d2d_payment_transcript(",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-wallet-semantic-validation":
    run_negative_control(
        "Android device-lab slot assembler wallet transcript semantic validation",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "device_lab.validate_wallet_integrity_transcript(",
            "device_lab.unchecked_wallet_integrity_transcript(",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-required-artifact-validation":
    run_negative_control(
        "Android device-lab slot assembler required artifact validation",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "device_lab.validate_required_kagemusha_slot_artifact_shapes(",
            "device_lab.unchecked_required_kagemusha_slot_artifact_shapes(",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-report-level-binding":
    run_negative_control(
        "Android device-lab slot assembler report level binding",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "and result_level != report_level",
            "and False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-report-status-binding":
    run_negative_control(
        "Android device-lab slot assembler report status binding",
        lambda: override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            "and result_status != report_status",
            "and False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-slot-assembler-attestation-status-exactness":
    def weaken_slot_assembler_attestation_status_exactness() -> None:
        override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            'if result_status is not None and result_status != "ok":',
            'if result_status is not None and result_status not in {"ok", "passed"}:',
        )
        override_text(
            "scripts/kagemusha_android_device_lab_slot.py",
            'if report_status is not None and report_status != "ok":',
            'if report_status is not None and report_status not in {"ok", "passed"}:',
        )

    run_negative_control(
        "Android device-lab slot assembler exact ok status gate",
        weaken_slot_assembler_attestation_status_exactness,
    )
    raise SystemExit(0)

if mode == "--negative-control-android-signed-evidence-freshness-report":
    run_negative_control(
        "Android signed-evidence freshness report binding",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '_android_report_kagemusha(report).get("signed_at_utc")',
            '_android_report_kagemusha(report).get("unchecked_signed_at_utc")',
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

if mode == "--negative-control-kagemusha-readiness-trusted-signer-sanitization":
    run_negative_control(
        "Kagemusha readiness trusted-signer summary sanitization",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "device_lab._trusted_signer_public_key_sha256_set(",
            "set(",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-android-report-secret-redaction":
    run_negative_control(
        "Kagemusha readiness Android report unsafe-string redaction",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "android_device_lab_report_unsafe_material",
            "android_device_lab_report_redaction_disabled",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-android-zero-binding-digest":
    run_negative_control(
        "Kagemusha readiness Android zero binding digest",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            'or value == "0" * 64',
            "or False",
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
        lambda: override_text_all(
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

if mode == "--negative-control-kagemusha-readiness-summary-output-temp-cleanup-failure":
    run_negative_control(
        "Kagemusha readiness summary output temp cleanup-failure gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    except OSError:\n        return [\n            _summary_out_blocker("--summary-out temporary file could not be removed")\n        ]\n',
            "    except OSError:\n        return []\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-summary-output-temp-cleanup-identity":
    run_negative_control(
        "Kagemusha readiness summary output temp cleanup identity gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "_file_identity(temp_stat) != expected_identity",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-summary-output-strict-json-write":
    run_negative_control(
        "Kagemusha readiness summary output strict JSON writer",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "allow_nan=False",
            "allow_nan=True",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-summary-output-size-limit":
    run_negative_control(
        "Kagemusha readiness summary output size-limit gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            'if len(summary_text.encode("utf-8")) > MAX_READINESS_SUMMARY_JSON_BYTES:',
            'if False and len(summary_text.encode("utf-8")) > MAX_READINESS_SUMMARY_JSON_BYTES:',
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

if mode == "--negative-control-kagemusha-readiness-summary-output-readback-size-limit":
    run_negative_control(
        "Kagemusha readiness summary output readback size-limit gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "if open_stat.st_size > MAX_READINESS_SUMMARY_JSON_BYTES:",
            "if False and open_stat.st_size > MAX_READINESS_SUMMARY_JSON_BYTES:",
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

if mode == "--negative-control-kagemusha-readiness-summary-output-private-permissions":
    run_negative_control(
        "Kagemusha readiness summary output private permissions",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "os.fchmod(handle.fileno(), 0o600)",
            "handle.fileno()",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-kagemusha-readiness-summary-output-parent-sync-identity":
    run_negative_control(
        "Kagemusha readiness summary output parent sync identity gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "expected_identity=parent_identity",
            "expected_identity=None",
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
        lambda: override_text_all(
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

if mode == "--negative-control-release-bundle-android-duplicate-binding-value-inventory":
    run_negative_control(
        "Kagemusha release bundle Android duplicate-binding value inventory",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "valid_value_sha256s != sorted(set(valid_value_sha256s))",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-blocked-manifest-trusted-signer-sanitization":
    run_negative_control(
        "Kagemusha release bundle blocked-manifest trusted-signer sanitization",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "device_lab._trusted_signer_public_key_sha256_set(",
            "set(",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-android-signed-evidence-identity":
    run_negative_control(
        "Kagemusha release bundle Android signed-evidence identity binding",
        lambda: override_text_all(
            "scripts/kagemusha_release_bundle.py",
            "device_lab.infer_kagemusha_device_family",
            "device_lab.accept_any_kagemusha_device_family",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-android-slot-summary-identity":
    run_negative_control(
        "Kagemusha release bundle Android slot summary identity binding",
        lambda: override_text_all(
            "scripts/kagemusha_release_bundle.py",
            "kagemusha_release_summary_android_slots_device_identity",
            "android_slots_device_identity_disabled",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-android-signed-evidence-identity-drift":
    run_negative_control(
        "Kagemusha release bundle Android signed-evidence identity drift",
        lambda: override_text_all(
            "scripts/kagemusha_release_bundle.py",
            "kagemusha_release_summary_android_signed_evidence_identity_drift",
            "android_signed_evidence_identity_drift_disabled",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-android-slot-identity-drift":
    run_negative_control(
        "Kagemusha release bundle Android slot identity drift",
        lambda: override_text_all(
            "scripts/kagemusha_release_bundle.py",
            "kagemusha_release_summary_android_slots_identity_drift",
            "android_slots_identity_drift_disabled",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-manifest-android-signed-evidence-identity-binding":
    run_negative_control(
        "Kagemusha release bundle manifest Android signed-evidence identity binding",
        lambda: override_text_all(
            "scripts/kagemusha_release_bundle.py",
            "kagemusha_release_bundle_manifest_android_signed_evidence_identity_binding",
            "android_manifest_signed_evidence_identity_binding_disabled",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-evidence-inventory-schema":
    run_negative_control(
        "Kagemusha release bundle evidence inventory schema gate",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "blockers.extend(_check_release_bundle_evidence_inventory_shape(evidence))",
            "blockers.extend([])",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-evidence-inventory-keysets":
    run_negative_control(
        "Kagemusha release bundle evidence inventory key-set gate",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "blockers.extend(_check_release_bundle_cross_section_shape(bundle))",
            "blockers.extend([])",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-section-schema":
    run_negative_control(
        "Kagemusha release bundle section schema gate",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "blockers.extend(_check_release_bundle_section_shapes(bundle))",
            "blockers.extend([])",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-android-manifest-schema":
    run_negative_control(
        "Kagemusha release bundle Android manifest schema gate",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "blockers.extend(_check_release_bundle_android_section_shape(bundle))",
            "blockers.extend([])",
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

if mode == "--negative-control-release-bundle-local-json-size-limit":
    run_negative_control(
        "Kagemusha release bundle local JSON size limit",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "if open_stat.st_size > MAX_RELEASE_BUNDLE_LOCAL_JSON_BYTES:",
            "if False and open_stat.st_size > MAX_RELEASE_BUNDLE_LOCAL_JSON_BYTES:",
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

if mode == "--negative-control-release-bundle-temp-cleanup-failure":
    run_negative_control(
        "Kagemusha release bundle temp cleanup failure gate",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            '    except OSError:\n        return [\n            _blocker(\n                "kagemusha_release_bundle_out_invalid",\n                "--out temporary file could not be removed",\n            )\n        ]\n',
            "    except OSError:\n        return []\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-temp-cleanup-identity":
    run_negative_control(
        "Kagemusha release bundle temp cleanup identity gate",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "_file_identity(temp_stat) != expected_identity",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-strict-json-write":
    run_negative_control(
        "Kagemusha release bundle strict JSON writer",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "allow_nan=False",
            "allow_nan=True",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-output-size-limit":
    run_negative_control(
        "Kagemusha release bundle output size limit",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            'if len(manifest_text.encode("utf-8")) > MAX_RELEASE_BUNDLE_OUTPUT_JSON_BYTES:',
            'if False and len(manifest_text.encode("utf-8")) > MAX_RELEASE_BUNDLE_OUTPUT_JSON_BYTES:',
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

if mode == "--negative-control-release-bundle-output-readback-size-limit":
    run_negative_control(
        "Kagemusha release bundle output readback size limit",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "if open_stat.st_size > MAX_RELEASE_BUNDLE_OUTPUT_JSON_BYTES:",
            "if False and open_stat.st_size > MAX_RELEASE_BUNDLE_OUTPUT_JSON_BYTES:",
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

if mode == "--negative-control-release-bundle-output-private-permissions":
    run_negative_control(
        "Kagemusha release bundle output private permissions",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "os.fchmod(handle.fileno(), 0o600)",
            "handle.fileno()",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-output-parent-sync-identity":
    run_negative_control(
        "Kagemusha release bundle output parent sync identity gate",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "expected_identity=parent_identity",
            "expected_identity=None",
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

if mode == "--negative-control-release-bundle-control-path-preflight":
    run_negative_control(
        "Kagemusha release bundle control path preflight",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            '    if device_lab._contains_control_character(path):\n        return _blocker(code, f"{label} must not contain control characters")\n',
            "",
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
            'existing_bundle_path,\n            bundle_root,\n            "Kagemusha release bundle manifest",',
            'existing_bundle_path,\n            bundle_root,\n            "Kagemusha release bundle manifest disabled",',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-release-bundle-verify-existing-evidence-path-shape":
    run_negative_control(
        "Kagemusha release bundle verify-existing evidence path-shape gate",
        lambda: override_text(
            "scripts/kagemusha_release_bundle.py",
            "blockers.extend(_check_release_bundle_evidence_paths(evidence))",
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
            '    path_text = str(path)\n    if device_lab.SECRET_RE.search(path_text):\n        return None, [f"{label} path must not contain secret-looking material"]\n',
            "",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-local-path-aliases":
    run_negative_control(
        "Reserved-lineage proof evidence local path-alias gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '    if "\\\\" in path_text:\n        return None, [f"{label} path must not contain backslashes"]\n    if ".." in path.parts:\n        return None, [f"{label} path must be canonical"]\n    ancestor_errors = device_lab.validate_no_symlink_ancestors(\n',
            '    ancestor_errors = device_lab.validate_no_symlink_ancestors(\n',
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
            '    if not parent_exists:\n        try:\n            parent.mkdir(mode=0o700, parents=True, exist_ok=True)\n        except OSError:\n            return [f"{label} parent directory could not be created"]\n',
            "    if not parent_exists:\n        parent.mkdir(mode=0o700, parents=True, exist_ok=True)\n",
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
            '    errors = preflight_output_path(path, label)\n    if errors:\n        return errors\n    parent = path.parent\n    parent_exists, parent_errors = _validate_output_parent(path, label)\n    if parent_errors:\n        return parent_errors\n    if not parent_exists:\n        try:\n            parent.mkdir(mode=0o700, parents=True, exist_ok=True)\n        except OSError:\n            return [f"{label} parent directory could not be created"]\n    permission_errors = _set_private_directory_permissions(parent, f"{label} parent")\n    if permission_errors:\n        return permission_errors\n    return preflight_output_path(path, label)\n',
            '    errors = preflight_output_path(path, label)\n    if errors:\n        return errors\n    parent = path.parent\n    parent_exists, parent_errors = _validate_output_parent(path, label)\n    if parent_errors:\n        return parent_errors\n    if not parent_exists:\n        parent.mkdir(mode=0o700, parents=True, exist_ok=True)\n    permission_errors = _set_private_directory_permissions(parent, f"{label} parent")\n    if permission_errors:\n        return permission_errors\n    return preflight_output_path(path, label)\n',
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

if mode == "--negative-control-lineage-proof-helper-output-temp-cleanup-failure":
    run_negative_control(
        "Reserved-lineage proof evidence helper output temp cleanup-failure gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            '    except OSError:\n        return ["--out temporary file could not be removed"]\n',
            "    except OSError:\n        return []\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-output-temp-cleanup-identity":
    run_negative_control(
        "Reserved-lineage proof evidence helper output temp cleanup identity gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            "_file_identity(temp_stat) != expected_identity",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-strict-json-write":
    run_negative_control(
        "Reserved-lineage proof evidence helper strict JSON writer",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            '            allow_nan=False,\n        ) + "\\n"\n    except ValueError:\n        return ["--out evidence is not strict JSON"]\n',
            '            allow_nan=True,\n        ) + "\\n"\n    except ValueError:\n        return ["--out evidence is not strict JSON"]\n',
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

if mode == "--negative-control-lineage-proof-helper-output-private-permissions":
    run_negative_control(
        "Reserved-lineage proof evidence helper private output permissions",
        lambda: (
            override_text(
                "scripts/kagemusha_lineage_proof_evidence.py",
                "os.fchmod(dir_fd, 0o700)",
                "os.fstat(dir_fd)",
            ),
            override_text_all(
                "scripts/kagemusha_lineage_proof_evidence.py",
                "os.fchmod(handle.fileno(), 0o600)",
                "handle.fileno()",
            ),
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
        old = '    if not parent_exists:\n        try:\n            parent.mkdir(mode=0o700, parents=True, exist_ok=True)\n        except OSError:\n            return [f"{label} parent directory could not be created"]\n'
        new = '    if not parent_exists:\n        parent.mkdir(mode=0o700, parents=True, exist_ok=True)\n'
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

if mode == "--negative-control-compact-key-helper-output-temp-cleanup-failure":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper output temp cleanup-failure gate",
        lambda: override_text(
            "scripts/kagemusha_recursive_compact_key_evidence.py",
            '    except OSError:\n        return ["--out temporary file could not be removed"]\n',
            "    except OSError:\n        return []\n",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-helper-output-temp-cleanup-identity":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper output temp cleanup identity gate",
        lambda: override_text(
            "scripts/kagemusha_recursive_compact_key_evidence.py",
            "_file_identity(temp_stat) != expected_identity",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-helper-strict-json-write":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper strict JSON writer",
        lambda: override_text(
            "scripts/kagemusha_recursive_compact_key_evidence.py",
            '            allow_nan=False,\n        ) + "\\n"\n    except ValueError:\n        return ["--out evidence is not strict JSON"]\n',
            '            allow_nan=True,\n        ) + "\\n"\n    except ValueError:\n        return ["--out evidence is not strict JSON"]\n',
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

if mode == "--negative-control-compact-key-helper-output-parent-sync-identity":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper output parent sync identity gate",
        lambda: override_text(
            "scripts/kagemusha_recursive_compact_key_evidence.py",
            "expected_identity=parent_identity",
            "expected_identity=None",
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

if mode == "--negative-control-compact-key-helper-output-private-permissions":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper private output permissions",
        lambda: (
            override_text(
                "scripts/kagemusha_recursive_compact_key_evidence.py",
                "os.fchmod(dir_fd, 0o700)",
                "os.fstat(dir_fd)",
            ),
            override_text_all(
                "scripts/kagemusha_recursive_compact_key_evidence.py",
                "os.fchmod(handle.fileno(), 0o600)",
                "handle.fileno()",
            ),
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
            '    try:\n        artifact_dir.mkdir(mode=0o700, parents=True, exist_ok=True)\n    except OSError:\n        return ["--artifact-dir could not be created for evidence validation"]\n',
            '    artifact_dir.mkdir(mode=0o700, parents=True, exist_ok=True)\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-helper-validation-strict-json-write":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper validation strict JSON gate",
        lambda: override_text(
            "scripts/kagemusha_recursive_compact_key_evidence.py",
            "recursive compact key evidence validation file is not strict JSON",
            "recursive compact key evidence validation file allows non-strict JSON",
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

if mode == "--negative-control-compact-key-helper-validation-temp-cleanup-after-write-failure":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper validation temp cleanup after write-failure gate",
        lambda: override_text(
            "scripts/kagemusha_recursive_compact_key_evidence.py",
            '                errors.append(\n                    "recursive compact key evidence validation file could not be removed"\n                )\n',
            "                pass\n",
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

if mode == "--negative-control-compact-key-helper-validation-temp-cleanup-identity":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper validation temp cleanup identity",
        lambda: override_text(
            "scripts/kagemusha_recursive_compact_key_evidence.py",
            "_file_identity(validation_temp_stat) != expected_identity",
            "False",
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

if mode == "--negative-control-evidence-helper-path-aliases":
    evidence_helper_alias_checks = (
        '    if "\\\\" in path:\n'
        '        return f"{label} must not contain backslashes"\n'
        '    if ".." in Path(path).parts:\n'
        '        return f"{label} must be canonical"\n'
    )
    run_negative_control(
        "Kagemusha evidence helper path alias gate",
        lambda: (
            override_text_all(
                "scripts/kagemusha_lineage_proof_evidence.py",
                evidence_helper_alias_checks,
                "",
            ),
            override_text_all(
                "scripts/kagemusha_recursive_compact_key_evidence.py",
                evidence_helper_alias_checks,
                "",
            ),
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-staged-path-aliases":
    staged_alias_checks = (
        '    if "\\\\" in path_text:\n'
        '        return f"{label} must not contain backslashes"\n'
        '    if ".." in path.parts:\n'
        '        return f"{label} must be canonical"\n'
    )
    run_negative_control(
        "Kagemusha staged path alias gate",
        lambda: (
            override_text_all(
                "scripts/kagemusha_run_lineage_proof_staged.py",
                staged_alias_checks,
                "",
            ),
            override_text_all(
                "scripts/kagemusha_run_recursive_compact_keygen_staged.py",
                staged_alias_checks,
                "",
            ),
            override_text_all(
                "scripts/kagemusha_finalize_lineage_proof_staged_run.py",
                staged_alias_checks,
                "",
            ),
            override_text_all(
                "scripts/kagemusha_finalize_recursive_compact_key_staged_run.py",
                staged_alias_checks,
                "",
            ),
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
            '    try:\n        artifact_dir.mkdir(mode=0o700, parents=True, exist_ok=True)\n    except OSError:\n        return ["--artifact-dir could not be created for evidence validation"]\n',
            '    artifact_dir.mkdir(mode=0o700, parents=True, exist_ok=True)\n',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-helper-validation-strict-json-write":
    run_negative_control(
        "Reserved-lineage proof evidence helper validation strict JSON gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            "lineage proof evidence validation file is not strict JSON",
            "lineage proof evidence validation file allows non-strict JSON",
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

if mode == "--negative-control-lineage-proof-helper-validation-temp-cleanup-after-write-failure":
    run_negative_control(
        "Reserved-lineage proof evidence helper validation temp cleanup after write-failure gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            '                errors.append("lineage proof evidence validation file could not be removed")\n',
            "                pass\n",
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

if mode == "--negative-control-lineage-proof-helper-validation-temp-cleanup-identity":
    run_negative_control(
        "Reserved-lineage proof evidence helper validation temp cleanup identity",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            "_file_identity(validation_temp_stat) != expected_identity",
            "False",
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

if mode == "--negative-control-compact-key-finalizer-exit-marker":
    run_negative_control(
        "ABI-7 recursive compact key staged finalizer exit-marker gate",
        lambda: override_text(
            "scripts/kagemusha_finalize_recursive_compact_key_staged_run.py",
            "staged keygen exit code must be 0",
            "staged keygen exit code is advisory",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-finalizer-exit-marker":
    run_negative_control(
        "Reserved-lineage proof staged finalizer exit-marker gate",
        lambda: override_text(
            "scripts/kagemusha_finalize_lineage_proof_staged_run.py",
            "staged lineage proof exit code must be 0",
            "staged lineage proof exit code is advisory",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-finalizer-timestamp-raw":
    run_negative_control(
        "ABI-7 recursive compact key staged finalizer raw timestamp gate",
        lambda: override_text(
            "scripts/kagemusha_finalize_recursive_compact_key_staged_run.py",
            "compact_evidence._validate_generated_at_utc(args.generated_at_utc)",
            "[]",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-finalizer-timestamp-raw":
    run_negative_control(
        "Reserved-lineage proof staged finalizer raw timestamp gate",
        lambda: override_text(
            "scripts/kagemusha_finalize_lineage_proof_staged_run.py",
            "lineage_evidence._validate_generated_at_utc(args.generated_at_utc)",
            "[]",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-finalizer-future-skew":
    run_negative_control(
        "ABI-7 recursive compact key staged finalizer future-skew preflight",
        lambda: override_text(
            "scripts/kagemusha_finalize_recursive_compact_key_staged_run.py",
            "compact_evidence._validate_generated_at_future_skew(\n            generated_at,\n            args.max_generated_at_future_skew_seconds,",
            "compact_evidence._skip_generated_at_future_skew(\n            generated_at,\n            args.max_generated_at_future_skew_seconds,",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-finalizer-future-skew":
    run_negative_control(
        "Reserved-lineage proof staged finalizer future-skew preflight",
        lambda: override_text(
            "scripts/kagemusha_finalize_lineage_proof_staged_run.py",
            "lineage_evidence._validate_generated_at_future_skew(\n            generated_at,\n            args.max_generated_at_future_skew_seconds,",
            "lineage_evidence._skip_generated_at_future_skew(\n            generated_at,\n            args.max_generated_at_future_skew_seconds,",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-finalizer-publish-readback":
    run_negative_control(
        "ABI-7 recursive compact key staged finalizer publish readback",
        lambda: override_text(
            "scripts/kagemusha_finalize_recursive_compact_key_staged_run.py",
            "verify_errors = _verify_published_file(",
            "verify_errors = _trust_published_file(",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-finalizer-publish-readback":
    run_negative_control(
        "Reserved-lineage proof staged finalizer publish readback",
        lambda: override_text(
            "scripts/kagemusha_finalize_lineage_proof_staged_run.py",
            "verify_errors = _verify_published_file(",
            "verify_errors = _trust_published_file(",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-finalizer-private-permissions":
    run_negative_control(
        "ABI-7 recursive compact key staged finalizer private output permissions",
        lambda: (
            override_text(
                "scripts/kagemusha_finalize_recursive_compact_key_staged_run.py",
                "os.fchmod(dir_fd, 0o700)",
                "os.fstat(dir_fd)",
            ),
            override_text(
                "scripts/kagemusha_finalize_recursive_compact_key_staged_run.py",
                "os.fchmod(dst.fileno(), 0o600)",
                "dst.fileno()",
            ),
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-finalizer-private-permissions":
    run_negative_control(
        "Reserved-lineage proof staged finalizer private output permissions",
        lambda: (
            override_text(
                "scripts/kagemusha_finalize_lineage_proof_staged_run.py",
                "os.fchmod(dir_fd, 0o700)",
                "os.fstat(dir_fd)",
            ),
            override_text(
                "scripts/kagemusha_finalize_lineage_proof_staged_run.py",
                "os.fchmod(dst.fileno(), 0o600)",
                "dst.fileno()",
            ),
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-finalizer-publish-rollback-identity":
    run_negative_control(
        "ABI-7 recursive compact key staged finalizer publish rollback identity",
        lambda: override_text(
            "scripts/kagemusha_finalize_recursive_compact_key_staged_run.py",
            "_file_identity(path_stat) == expected_identity",
            "True",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-finalizer-publish-rollback-identity":
    run_negative_control(
        "Reserved-lineage proof staged finalizer publish rollback identity",
        lambda: override_text(
            "scripts/kagemusha_finalize_lineage_proof_staged_run.py",
            "_file_identity(path_stat) == expected_identity",
            "True",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-finalizer-publish-rollback-cleanup-report":
    run_negative_control(
        "ABI-7 recursive compact key staged finalizer publish rollback cleanup report",
        lambda: override_text(
            "scripts/kagemusha_finalize_recursive_compact_key_staged_run.py",
            'return [f"{label} rollback cleanup could not remove file"]',
            "return []",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-finalizer-publish-rollback-cleanup-report":
    run_negative_control(
        "Reserved-lineage proof staged finalizer publish rollback cleanup report",
        lambda: override_text(
            "scripts/kagemusha_finalize_lineage_proof_staged_run.py",
            'return [f"{label} rollback cleanup could not remove file"]',
            "return []",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-finalizer-publish-dir-sync-identity":
    run_negative_control(
        "ABI-7 recursive compact key staged finalizer publish directory sync identity",
        lambda: override_text(
            "scripts/kagemusha_finalize_recursive_compact_key_staged_run.py",
            "expected_identity=artifact_dir_identity",
            "expected_identity=None",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-finalizer-publish-dir-sync-identity":
    run_negative_control(
        "Reserved-lineage proof staged finalizer publish directory sync identity",
        lambda: override_text(
            "scripts/kagemusha_finalize_lineage_proof_staged_run.py",
            "expected_identity=artifact_dir_identity",
            "expected_identity=None",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-finalizer-temp-cleanup-identity":
    run_negative_control(
        "ABI-7 recursive compact staged finalizer temporary cleanup identity",
        lambda: override_text(
            "scripts/kagemusha_finalize_recursive_compact_key_staged_run.py",
            "_file_identity(temp_parent_stat) != expected_identity",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-finalizer-temp-cleanup-identity":
    run_negative_control(
        "Reserved-lineage proof staged finalizer temporary cleanup identity",
        lambda: override_text(
            "scripts/kagemusha_finalize_lineage_proof_staged_run.py",
            "_file_identity(temp_parent_stat) != expected_identity",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-finalizer-temp-cleanup-report":
    run_negative_control(
        "ABI-7 recursive compact staged finalizer temporary cleanup report",
        lambda: override_text(
            "scripts/kagemusha_finalize_recursive_compact_key_staged_run.py",
            "if finalizer_errors or cleanup_errors:",
            "if finalizer_errors:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-finalizer-temp-cleanup-report":
    run_negative_control(
        "Reserved-lineage proof staged finalizer temporary cleanup report",
        lambda: override_text(
            "scripts/kagemusha_finalize_lineage_proof_staged_run.py",
            "if finalizer_errors or cleanup_errors:",
            "if finalizer_errors:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-staged-runner-exit-marker":
    run_negative_control(
        "Reserved-lineage proof staged runner exit-marker preservation",
        lambda: override_text(
            "scripts/kagemusha_run_lineage_proof_staged.py",
            'f"{exit_code}\\n"',
            '"0\\n"',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-staged-runner-readback":
    run_negative_control(
        "Reserved-lineage proof staged runner metadata readback",
        lambda: override_text(
            "scripts/kagemusha_run_lineage_proof_staged.py",
            "return _verify_written_text_file(path, expected_bytes, label)",
            "return []",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-staged-runner-parent-sync-identity":
    run_negative_control(
        "Reserved-lineage proof staged runner parent sync identity",
        lambda: override_text(
            "scripts/kagemusha_run_lineage_proof_staged.py",
            "expected_identity=parent_identity",
            "expected_identity=None",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-staged-runner-log-install-parent-sync-identity":
    run_negative_control(
        "Reserved-lineage proof staged runner log-install parent sync identity",
        lambda: override_text(
            "scripts/kagemusha_run_lineage_proof_staged.py",
            "expected_identity=log_parent_identity",
            "expected_identity=None",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-staged-runner-cleanup-identity":
    run_negative_control(
        "Reserved-lineage proof staged runner cleanup identity",
        lambda: override_text(
            "scripts/kagemusha_run_lineage_proof_staged.py",
            "_file_identity(path_stat) != expected_identity",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-staged-runner-private-permissions":
    run_negative_control(
        "Reserved-lineage proof staged runner private output permissions",
        lambda: (
            override_text(
                "scripts/kagemusha_run_lineage_proof_staged.py",
                "os.fchmod(dir_fd, 0o700)",
                "os.fstat(dir_fd)",
            ),
            override_text(
                "scripts/kagemusha_run_lineage_proof_staged.py",
                "os.fchmod(file_fd, 0o600)",
                "os.fstat(file_fd)",
            ),
            override_text(
                "scripts/kagemusha_run_lineage_proof_staged.py",
                "os.fchmod(handle.fileno(), 0o600)",
                "handle.fileno()",
            ),
            override_text(
                "scripts/kagemusha_run_lineage_proof_staged.py",
                "os.fchmod(log_handle.fileno(), 0o600)",
                "log_handle.fileno()",
            ),
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-staged-runner-private-permissions":
    run_negative_control(
        "ABI-7 recursive compact key staged runner private output permissions",
        lambda: (
            override_text(
                "scripts/kagemusha_run_recursive_compact_keygen_staged.py",
                "os.fchmod(dir_fd, 0o700)",
                "os.fstat(dir_fd)",
            ),
            override_text(
                "scripts/kagemusha_run_recursive_compact_keygen_staged.py",
                "os.fchmod(file_fd, 0o600)",
                "os.fstat(file_fd)",
            ),
            override_text(
                "scripts/kagemusha_run_recursive_compact_keygen_staged.py",
                "os.fchmod(handle.fileno(), 0o600)",
                "handle.fileno()",
            ),
            override_text(
                "scripts/kagemusha_run_recursive_compact_keygen_staged.py",
                "os.fchmod(log_handle.fileno(), 0o600)",
                "log_handle.fileno()",
            ),
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-staged-runner-exit-marker":
    run_negative_control(
        "ABI-7 recursive compact key staged runner exit-marker preservation",
        lambda: override_text(
            "scripts/kagemusha_run_recursive_compact_keygen_staged.py",
            'f"{exit_code}\\n"',
            '"0\\n"',
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-staged-runner-readback":
    run_negative_control(
        "ABI-7 recursive compact key staged runner metadata readback",
        lambda: override_text(
            "scripts/kagemusha_run_recursive_compact_keygen_staged.py",
            "return _verify_written_text_file(path, expected_bytes, label)",
            "return []",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-staged-runner-parent-sync-identity":
    run_negative_control(
        "ABI-7 recursive compact key staged runner parent sync identity",
        lambda: override_text(
            "scripts/kagemusha_run_recursive_compact_keygen_staged.py",
            "expected_identity=parent_identity",
            "expected_identity=None",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-staged-runner-log-install-parent-sync-identity":
    run_negative_control(
        "ABI-7 recursive compact key staged runner log-install parent sync identity",
        lambda: override_text(
            "scripts/kagemusha_run_recursive_compact_keygen_staged.py",
            "expected_identity=log_parent_identity",
            "expected_identity=None",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-staged-runner-cleanup-identity":
    run_negative_control(
        "ABI-7 recursive compact key staged runner cleanup identity",
        lambda: override_text(
            "scripts/kagemusha_run_recursive_compact_keygen_staged.py",
            "_file_identity(path_stat) != expected_identity",
            "False",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-staged-runner-child-log-file":
    run_negative_control(
        "Reserved-lineage proof staged runner child log-file binding",
        lambda: override_text(
            "scripts/kagemusha_run_lineage_proof_staged.py",
            "stdout=log_handle",
            "stdout=subprocess.PIPE",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-staged-runner-child-log-file":
    run_negative_control(
        "ABI-7 recursive compact key staged runner child log-file binding",
        lambda: override_text(
            "scripts/kagemusha_run_recursive_compact_keygen_staged.py",
            "stdout=log_handle",
            "stdout=subprocess.PIPE",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-staged-runner-supervisor-output-pipe":
    run_negative_control(
        "Reserved-lineage proof staged runner supervisor output pipe",
        lambda: override_text(
            "scripts/kagemusha_run_lineage_proof_staged.py",
            "                break\n            except subprocess.TimeoutExpired:",
            "                sys.stdout.buffer.write(b\"\")\n                break\n            except subprocess.TimeoutExpired:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-staged-runner-supervisor-output-pipe":
    run_negative_control(
        "ABI-7 recursive compact key staged runner supervisor output pipe",
        lambda: override_text(
            "scripts/kagemusha_run_recursive_compact_keygen_staged.py",
            "                break\n            except subprocess.TimeoutExpired:",
            "                sys.stdout.buffer.write(b\"\")\n                break\n            except subprocess.TimeoutExpired:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-staged-runner-heartbeat":
    run_negative_control(
        "Reserved-lineage proof staged runner heartbeat observability",
        lambda: (
            override_text(
                "scripts/kagemusha_run_lineage_proof_staged.py",
                "STAGED_COMMAND_HEARTBEAT_SECONDS = 300.0",
                "STAGED_COMMAND_HEARTBEAT_SECONDS = 0.0",
            ),
            override_text(
                "scripts/kagemusha_run_lineage_proof_staged.py",
                "[kagemusha-staged-runner] lineage-proof heartbeat ",
                "[kagemusha-staged-runner] lineage-proof quiet ",
            ),
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-staged-runner-heartbeat":
    run_negative_control(
        "ABI-7 recursive compact key staged runner heartbeat observability",
        lambda: (
            override_text(
                "scripts/kagemusha_run_recursive_compact_keygen_staged.py",
                "STAGED_COMMAND_HEARTBEAT_SECONDS = 300.0",
                "STAGED_COMMAND_HEARTBEAT_SECONDS = 0.0",
            ),
            override_text(
                "scripts/kagemusha_run_recursive_compact_keygen_staged.py",
                "[kagemusha-staged-runner] compact-keygen heartbeat ",
                "[kagemusha-staged-runner] compact-keygen quiet ",
            ),
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-staged-runner-execution-log-sha256":
    run_negative_control(
        "Reserved-lineage proof staged runner execution-log SHA-256 binding",
        lambda: override_text(
            "scripts/kagemusha_run_lineage_proof_staged.py",
            "log_sha256 must match staged {profile} lineage key artifact log SHA-256",
            "log_sha256 may drift from staged {profile} lineage key artifact log",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-staged-runner-execution-log-sha256":
    run_negative_control(
        "ABI-7 recursive compact key staged runner execution-log SHA-256 binding",
        lambda: override_text(
            "scripts/kagemusha_run_recursive_compact_keygen_staged.py",
            "generator_log_sha256 must match staged generator log SHA-256",
            "generator_log_sha256 may drift from staged generator log SHA-256",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-finalizer-execution-log-sha256":
    run_negative_control(
        "Reserved-lineage proof staged finalizer execution-log SHA-256 binding",
        lambda: override_text(
            "scripts/kagemusha_finalize_lineage_proof_staged_run.py",
            "log_sha256 must match staged log SHA-256",
            "log_sha256 may drift from staged log SHA-256",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-finalizer-execution-log-sha256":
    run_negative_control(
        "ABI-7 recursive compact key staged finalizer execution-log SHA-256 binding",
        lambda: override_text(
            "scripts/kagemusha_finalize_recursive_compact_key_staged_run.py",
            "generator_log_sha256 must match staged generator log SHA-256",
            "generator_log_sha256 may drift from staged generator log SHA-256",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-finalizer-execution-elapsed-binding":
    run_negative_control(
        "ABI-7 recursive compact key staged finalizer execution elapsed-time binding",
        lambda: override_text(
            "scripts/kagemusha_finalize_recursive_compact_key_staged_run.py",
            "elapsed_seconds must match staged run report",
            "elapsed_seconds may drift from staged run report",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-lineage-proof-staged-runner-resume-replace-conflict":
    run_negative_control(
        "Reserved-lineage proof staged runner resume/replace conflict gate",
        lambda: override_text(
            "scripts/kagemusha_run_lineage_proof_staged.py",
            "--replace and --resume-key-artifacts cannot be combined",
            "--replace and --resume-key-artifacts may be combined",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-staged-runner-resume-replace-conflict":
    run_negative_control(
        "ABI-7 recursive compact key staged runner resume/replace conflict gate",
        lambda: override_text(
            "scripts/kagemusha_run_recursive_compact_keygen_staged.py",
            "--replace and --resume-keygen cannot be combined",
            "--replace and --resume-keygen may be combined",
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

if mode == "--negative-control-lineage-proof-evidence-json-size-limit":
    run_negative_control(
        "Reserved-lineage proof evidence JSON size limit",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "max_bytes=MAX_LINEAGE_PROOF_EVIDENCE_JSON_BYTES",
            "max_bytes=None",
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

if mode == "--negative-control-compact-key-evidence-json-size-limit":
    run_negative_control(
        "ABI-7 recursive compact key evidence JSON size limit",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "max_bytes=MAX_COMPACT_KEY_EVIDENCE_JSON_BYTES",
            "max_bytes=None",
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

if mode == "--negative-control-compact-key-generator-log-size-limit":
    run_negative_control(
        "ABI-7 recursive compact key generator log size limit",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "max_bytes=MAX_COMPACT_KEY_GENERATOR_LOG_BYTES",
            "max_bytes=None",
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

if mode == "--negative-control-android-signed-evidence-summary-identity-fields":
    run_negative_control(
        "Android signed-evidence readiness summary identity binding",
        lambda: override_text_all(
            "scripts/kagemusha_production_readiness.py",
            "device_lab.infer_kagemusha_device_family",
            "device_lab.accept_any_kagemusha_device_family",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-signed-evidence-summary-partial-identity":
    run_negative_control(
        "Android signed-evidence readiness summary partial identity omission",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "if identity_fields and identity_fields != ANDROID_SIGNED_EVIDENCE_SUMMARY_IDENTITY_FIELDS:",
            "if False and identity_fields != ANDROID_SIGNED_EVIDENCE_SUMMARY_IDENTITY_FIELDS:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-signed-evidence-summary-partial-artifact-binding":
    run_negative_control(
        "Android signed-evidence readiness summary partial artifact binding omission",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "if artifact_fields and artifact_fields != expected:",
            "if False and artifact_fields != expected:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-signed-evidence-summary-partial-core-binding":
    run_negative_control(
        "Android signed-evidence readiness summary partial core binding omission",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "if core_fields and core_fields != ANDROID_SIGNED_EVIDENCE_SUMMARY_CORE_FIELDS:",
            "if False and core_fields != ANDROID_SIGNED_EVIDENCE_SUMMARY_CORE_FIELDS:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-signed-evidence-summary-incomplete-entry":
    run_negative_control(
        "Android signed-evidence readiness summary incomplete entry omission",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "if set(entry) != ANDROID_SIGNED_EVIDENCE_SUMMARY_TARGET_FIELDS:",
            "if False and set(entry) != ANDROID_SIGNED_EVIDENCE_SUMMARY_TARGET_FIELDS:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-signed-evidence-summary-slot-id":
    run_negative_control(
        "Android signed-evidence readiness summary safe slot id gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "if safe_slot is None:",
            "if False and safe_slot is None:",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-device-lab-incomplete-slot-coverage":
    run_negative_control(
        "Android device-lab incomplete slot matrix coverage",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "and _android_report_has_complete_signed_evidence(report, signed_evidence)",
            "and True",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-slot-summary-incomplete-kagemusha":
    run_negative_control(
        "Android device-lab incomplete slot Kagemusha summary omission",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "if not _android_report_has_complete_signed_evidence(report, signed_evidence):",
            "if False and not _android_report_has_complete_signed_evidence(report, signed_evidence):",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-android-duplicate-bindings-incomplete-slot-summary":
    run_negative_control(
        "Android duplicate-bindings summary complete-slot gate",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            '"duplicate_bindings": _android_duplicate_matrix_bindings_summary(\n            reports,\n            signed_evidence,\n        ),',
            '"duplicate_bindings": device_lab.kagemusha_duplicate_matrix_bindings(reports),',
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

if mode == "--negative-control-lineage-proof-helper-future-skew":
    run_negative_control(
        "Reserved-lineage proof evidence helper future-skew gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            "_validate_generated_at_future_skew(\n            generated_at,\n            max_generated_at_future_skew_seconds,",
            "_skip_generated_at_future_skew(\n            generated_at,\n            max_generated_at_future_skew_seconds,",
        ),
    )
    raise SystemExit(0)

if mode == "--negative-control-compact-key-helper-future-skew":
    run_negative_control(
        "ABI-7 recursive compact key evidence helper future-skew gate",
        lambda: override_text(
            "scripts/kagemusha_recursive_compact_key_evidence.py",
            "_validate_generated_at_future_skew(\n            generated_at,\n            max_generated_at_future_skew_seconds,",
            "_skip_generated_at_future_skew(\n            generated_at,\n            max_generated_at_future_skew_seconds,",
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

if mode == "--negative-control-lineage-proof-log-size-limit":
    run_negative_control(
        "Reserved-lineage proof evidence proof-log size limit",
        lambda: override_text(
            "scripts/kagemusha_production_readiness.py",
            "max_bytes=MAX_LINEAGE_PROOF_LOG_BYTES",
            "max_bytes=None",
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

if mode == "--negative-control-lineage-proof-evidence-output-parent-sync-identity":
    run_negative_control(
        "Reserved-lineage proof evidence output parent sync identity gate",
        lambda: override_text(
            "scripts/kagemusha_lineage_proof_evidence.py",
            "expected_identity=parent_identity",
            "expected_identity=None",
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
        lambda: override_text_all(
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
