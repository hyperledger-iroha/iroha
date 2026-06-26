"""Roll up Kagemusha production-readiness evidence into a strict summary."""

from __future__ import annotations

import argparse
import base64
import binascii
from collections.abc import Mapping
import datetime as dt
import hashlib
import json
import math
import os
from pathlib import Path
import re
import shlex
import stat
import sys
import tempfile
from typing import Any, Iterable

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import check_android_device_lab_slot as device_lab  # noqa: E402


SUMMARY_SCHEMA = "iroha.kagemusha.production_readiness.v1"
ABI6_MANIFEST_PATH = "fixtures/kagemusha_recursive_spend_abi6/manifest.json"
ABI6_MANIFEST_SCHEMA = "iroha.kagemusha.recursive_spend.abi6.fixture_manifest.v1"
ABI7_FIXTURE_MANIFEST_PATH = "fixtures/kagemusha_recursive_spend_abi7/manifest.json"
ABI7_FIXTURE_MANIFEST_SCHEMA = "iroha.kagemusha.recursive_spend.abi7.fixture_manifest.v1"
ABI7_ARCHIVE_FIXTURE_PATH = "fixtures/kagemusha_recursive_spend_abi7/archives.json"
ABI7_ARCHIVE_FIXTURE_SCHEMA = "iroha.kagemusha.recursive_spend.abi7.archive_fixtures.v1"
LINEAGE_PROOF_EVIDENCE_SCHEMA = "iroha.kagemusha.lineage_proof_evidence.v1"
LINEAGE_PROOF_EVIDENCE_FILENAME = "lineage-proof-evidence.json"
DEFAULT_LINEAGE_PROOF_EVIDENCE_PATH = f"artifacts/kagemusha/{LINEAGE_PROOF_EVIDENCE_FILENAME}"
COMPACT_KEY_EVIDENCE_SCHEMA = "iroha.kagemusha.recursive_compact_key_evidence.v1"
COMPACT_KEY_EVIDENCE_FILENAME = "recursive-compact-key-evidence.json"
DEFAULT_COMPACT_KEY_EVIDENCE_PATH = f"artifacts/kagemusha/{COMPACT_KEY_EVIDENCE_FILENAME}"
LOCALNET_LIFECYCLE_EVIDENCE_SCHEMA = "iroha.kagemusha.localnet_lifecycle_evidence.v1"
LOCALNET_LIFECYCLE_EVIDENCE_FILENAME = "kagemusha-localnet-lifecycle-evidence.json"
DEFAULT_LOCALNET_LIFECYCLE_EVIDENCE_PATH = (
    f"artifacts/kagemusha/{LOCALNET_LIFECYCLE_EVIDENCE_FILENAME}"
)
COMPACT_KEY_GENERATOR_LOG_FILENAME = "recursive-compact-key-artifacts.log"
DEFAULT_ANDROID_DEVICE_LAB_ROOT_PATH = "artifacts/android/device_lab"
DEFAULT_MIN_SIGNED_AT_UTC = "2026-06-06T00:00:00Z"
DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS = 300
ANDROID_DEVICE_LAB_ROOT_SUMMARY_LABEL = "<local-device-lab-root>"
LINEAGE_PROOF_EVIDENCE_SUMMARY_LABEL = "<lineage-proof-evidence>"
COMPACT_KEY_EVIDENCE_SUMMARY_LABEL = "<recursive-compact-key-evidence>"
LOCALNET_LIFECYCLE_EVIDENCE_SUMMARY_LABEL = "<kagemusha-localnet-lifecycle-evidence>"
EVIDENCE_CONTROL_STRING_REDACTION = "<redacted-control-string>"
EVIDENCE_NONFINITE_NUMBER_REDACTION = "<redacted-non-finite-number>"
EVIDENCE_NON_STRING_KEY_REDACTION = "<redacted-non-string-key>"
EVIDENCE_UNSUPPORTED_VALUE_REDACTION = "<redacted-unsupported-value>"
EVIDENCE_ERRORS_NORMALIZED_FIELD = "android_report_errors_normalized"
EVIDENCE_ERROR_REDACTION = "<malformed-android-report-error>"
ANDROID_SIGNED_EVIDENCE_SUMMARY_FIELDS: tuple[tuple[str, str], ...] = (
    ("signed_at_utc", "signed_at_utc"),
    ("device_family", "device_family"),
    ("device_model", "device_model"),
    ("device_codename", "device_codename"),
    ("signed_evidence_artifact_sha256", "artifact_sha256"),
    ("signed_evidence_signer_public_key_sha256", "signer_public_key_sha256"),
    ("offline_wallet_apk_path", "offline_wallet_apk_path"),
    ("offline_wallet_apk_sha256", "offline_wallet_apk_sha256"),
    ("d2d_payment_transcript_path", "d2d_payment_transcript_path"),
    ("d2d_payment_transcript_sha256", "d2d_payment_transcript_sha256"),
    ("wallet_integrity_transcript_path", "wallet_integrity_transcript_path"),
    ("wallet_integrity_transcript_sha256", "wallet_integrity_transcript_sha256"),
    ("attestation_certificate_chain_path", "attestation_certificate_chain_path"),
    ("attestation_certificate_chain_sha256", "attestation_certificate_chain_sha256"),
)
ANDROID_SIGNED_EVIDENCE_SUMMARY_TARGET_FIELDS = frozenset(
    target_key for _, target_key in ANDROID_SIGNED_EVIDENCE_SUMMARY_FIELDS
)
ANDROID_SIGNED_EVIDENCE_SUMMARY_SHA256_FIELDS = frozenset(
    (
        "artifact_sha256",
        "signer_public_key_sha256",
        "offline_wallet_apk_sha256",
        "d2d_payment_transcript_sha256",
        "wallet_integrity_transcript_sha256",
        "attestation_certificate_chain_sha256",
    )
)
ANDROID_SIGNED_EVIDENCE_SUMMARY_PATH_FIELDS = frozenset(
    (
        "offline_wallet_apk_path",
        "d2d_payment_transcript_path",
        "wallet_integrity_transcript_path",
        "attestation_certificate_chain_path",
    )
)
ANDROID_SIGNED_EVIDENCE_SUMMARY_PATH_ROOTS = {
    "offline_wallet_apk_path": "evidence",
    "d2d_payment_transcript_path": "handoff",
    "wallet_integrity_transcript_path": "wallet",
    "attestation_certificate_chain_path": "attestation",
}
ANDROID_SIGNED_EVIDENCE_SUMMARY_ARTIFACT_PAIRS: tuple[tuple[str, str], ...] = (
    ("offline_wallet_apk_path", "offline_wallet_apk_sha256"),
    ("d2d_payment_transcript_path", "d2d_payment_transcript_sha256"),
    ("wallet_integrity_transcript_path", "wallet_integrity_transcript_sha256"),
    ("attestation_certificate_chain_path", "attestation_certificate_chain_sha256"),
)
ANDROID_SIGNED_EVIDENCE_SUMMARY_CORE_FIELDS = frozenset(
    ("signed_at_utc", "artifact_sha256", "signer_public_key_sha256")
)
ANDROID_SIGNED_EVIDENCE_SUMMARY_IDENTITY_FIELDS = frozenset(
    ("device_family", "device_model", "device_codename")
)
ANDROID_SLOT_RELEASE_KAGEMUSHA_FIELDS = frozenset(
    (
        "required",
        "native_bridge_abi_version",
        "device_fingerprint_sha256",
        "attestation_challenge_sha256",
        "d2d_payment_transport",
        "d2d_payment_transports",
        "d2d_payment_transcripts",
        *(source_key for source_key, _ in ANDROID_SIGNED_EVIDENCE_SUMMARY_FIELDS),
    )
)
ANDROID_REQUIRED_D2D_PAYMENT_TRANSPORTS = tuple(sorted(device_lab.D2D_PAYMENT_TRANSPORTS))
MAX_ABI6_MANIFEST_JSON_BYTES = 1024 * 1024
MAX_ABI7_FIXTURE_JSON_BYTES = 1024 * 1024
MAX_REPO_SOURCE_MARKER_BYTES = 8 * 1024 * 1024
MAX_LINEAGE_PROOF_EVIDENCE_JSON_BYTES = 16 * 1024 * 1024
MAX_COMPACT_KEY_EVIDENCE_JSON_BYTES = 16 * 1024 * 1024
MAX_LOCALNET_LIFECYCLE_EVIDENCE_JSON_BYTES = 16 * 1024 * 1024
MAX_READINESS_SUMMARY_JSON_BYTES = 16 * 1024 * 1024
EXPECTED_LINEAGE_PROOF_OPENING_LEN = 128
EXPECTED_LINEAGE_PROOF_IPA_K = 8
EXPECTED_LINEAGE_PROOF_BACKEND = "halo2/ipa"
EXPECTED_COMPACT_KEY_OPENING_LEN = 4
EXPECTED_COMPACT_KEY_IPA_K = 8
EXPECTED_COMPACT_KEY_BACKEND = "halo2/ipa"
EXPECTED_COMPACT_KEY_CIRCUIT_ID = "kagemusha-recursive-compact-v1"
EXPECTED_COMPACT_KEY_RECORD_NAMESPACE = "offline_kagemusha"
EXPECTED_COMPACT_KEY_RECORD_VERSION = 1
KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_RUNTIME_KEYGEN_ENV = (
    "IROHA_KAGEMUSHA_ALLOW_RUNTIME_LINEAGE_KEYGEN"
)
EXPECTED_LINEAGE_VERIFIER_WITNESS_PROFILE = (
    "pallas-ipa-transparent-v1/vesta-recursive-fixed-window-255x1"
)
EXPECTED_LINEAGE_CIRCUIT_IDS = {
    "one_hop": "kagemusha-recursive-spend-lineage-onehop-v1",
    "append": "kagemusha-recursive-spend-lineage-append-v1",
}
LINEAGE_PROOF_REQUIRED_ARTIFACTS = (
    "lineage-init-len128.norito",
    "lineage-init-len128.record.norito",
    "lineage-init-len128.vk",
    "lineage-init-len128.pk",
    "lineage-append-len128.norito",
    "lineage-append-len128.record.norito",
    "lineage-append-len128.vk",
    "lineage-append-len128.pk",
)
LINEAGE_PROOF_REQUIRED_TESTS = {
    "record_archive_proof": (
        "kagemusha_recursive_spend_lineage_init_append_from_record_archives_proves_reserved_lineage_output"
    ),
}
LINEAGE_PROOF_REQUIRED_TEST_LOGS = {
    "record_archive_proof": "record-archive-proof.log",
}
EXPECTED_LINEAGE_PROOF_RESULT_PREFIX = (
    "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;"
)
LINEAGE_PROOF_RESULT_RE = re.compile(
    r"^test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; "
    r"0 filtered out; finished in [0-9]+(?:\.[0-9]+)?s$"
)
LINEAGE_ARTIFACT_ALL_ZERO_ERROR = (
    "must be generated lineage material, not all-zero placeholder bytes"
)
COMPACT_KEY_REQUIRED_ARTIFACTS = (
    "recursive-compact-len4.vk",
    "recursive-compact-len4.pk",
    "recursive-compact-key-artifacts.norito",
    "recursive-compact-verifier-keys.norito",
    "recursive-compact-len4.record.norito",
)
COMPACT_KEY_PLACEHOLDER_PREFIXES = (
    b"recursive compact key artifact ",
    b"dummy recursive compact key ",
    b"placeholder recursive compact key ",
    b"test recursive compact key ",
)
COMPACT_KEY_PLACEHOLDER_ERROR = "must be generated key material, not a placeholder fixture"
COMPACT_KEY_ALL_ZERO_ERROR = "must be generated key material, not all-zero placeholder bytes"
MAX_COMPACT_KEY_GENERATOR_LOG_BYTES = 1024 * 1024
COMPACT_KEY_GENERATOR_LOG_SIZE_FIELDS = {
    "recursive-compact-len4.vk": "vk",
    "recursive-compact-len4.pk": "pk",
    "recursive-compact-key-artifacts.norito": "key_artifacts",
    "recursive-compact-verifier-keys.norito": "verifier_keys",
    "recursive-compact-len4.record.norito": "record",
}
COMPACT_KEY_GENERATOR_LOG_DIGEST_FIELDS = {
    artifact: f"{field}_sha256"
    for artifact, field in COMPACT_KEY_GENERATOR_LOG_SIZE_FIELDS.items()
}
COMPACT_KEY_GENERATOR_LOG_RE = re.compile(
    r"^Wrote ABI-7 recursive compact key artifacts for "
    r"`kagemusha-recursive-compact-v1` opening_len=4 to "
    r"artifacts/kagemusha/recursive-compact-len4\.vk and "
    r"artifacts/kagemusha/recursive-compact-len4\.pk "
    r"\(vk=(?P<vk>[1-9][0-9]*) bytes sha256=(?P<vk_sha256>[0-9a-f]{64}), "
    r"pk=(?P<pk>[1-9][0-9]*) bytes sha256=(?P<pk_sha256>[0-9a-f]{64}), "
    r"record=(?P<record>[1-9][0-9]*) bytes sha256=(?P<record_sha256>[0-9a-f]{64}), "
    r"key_artifacts=(?P<key_artifacts>[1-9][0-9]*) bytes sha256=(?P<key_artifacts_sha256>[0-9a-f]{64}), "
    r"verifier_keys=(?P<verifier_keys>[1-9][0-9]*) bytes sha256=(?P<verifier_keys_sha256>[0-9a-f]{64})\)$"
)


def expected_lineage_proof_command(expected_name: str) -> str:
    """Return the canonical production Reserved-lineage proof command string."""

    return (
        "cargo test -p iroha_core "
        f"{expected_name} "
        "--lib -- --ignored --test-threads=1 --nocapture"
    )


def expected_compact_key_command() -> str:
    """Return the canonical ABI-7 recursive compact key-artifact command."""

    return (
        "iroha app zk kagemusha recursive-compact-key-artifacts "
        "--vk-out artifacts/kagemusha/recursive-compact-len4.vk "
        "--pk-out artifacts/kagemusha/recursive-compact-len4.pk "
        "--key-artifacts-out artifacts/kagemusha/recursive-compact-key-artifacts.norito "
        "--verifier-keys-out artifacts/kagemusha/recursive-compact-verifier-keys.norito "
        "--record-out artifacts/kagemusha/recursive-compact-len4.record.norito "
        "--record-namespace offline_kagemusha "
        "--record-version 1"
    )


def expected_compact_key_generator_log_line(
    artifact_size_bytes: dict[str, int],
    artifact_sha256: dict[str, str],
) -> str:
    """Return the canonical ABI-7 recursive compact key generator summary line."""

    return (
        "Wrote ABI-7 recursive compact key artifacts for "
        "`kagemusha-recursive-compact-v1` opening_len=4 to "
        "artifacts/kagemusha/recursive-compact-len4.vk and "
        "artifacts/kagemusha/recursive-compact-len4.pk "
        f"(vk={artifact_size_bytes['recursive-compact-len4.vk']} bytes "
        f"sha256={artifact_sha256['recursive-compact-len4.vk']}, "
        f"pk={artifact_size_bytes['recursive-compact-len4.pk']} bytes "
        f"sha256={artifact_sha256['recursive-compact-len4.pk']}, "
        f"record={artifact_size_bytes['recursive-compact-len4.record.norito']} bytes "
        f"sha256={artifact_sha256['recursive-compact-len4.record.norito']}, "
        f"key_artifacts={artifact_size_bytes['recursive-compact-key-artifacts.norito']} bytes "
        f"sha256={artifact_sha256['recursive-compact-key-artifacts.norito']}, "
        f"verifier_keys={artifact_size_bytes['recursive-compact-verifier-keys.norito']} bytes "
        f"sha256={artifact_sha256['recursive-compact-verifier-keys.norito']})"
    )


MAX_LINEAGE_PROOF_LOG_BYTES = 64 * 1024 * 1024
LINEAGE_PROOF_EVIDENCE_FIELDS: frozenset[str] = frozenset(
    {
        "schema",
        "generated_at_utc",
        "opening_len",
        "ipa_k",
        "verifier_backend",
        "verifier_witness_profile",
        "record_archive_proof_runtime_keygen_env",
        "circuit_ids",
        "artifacts",
        "artifact_size_bytes",
        "tests",
    }
)
LINEAGE_PROOF_TEST_FIELDS: frozenset[str] = frozenset(
    {
        "name",
        "status",
        "ignored",
        "command",
        "elapsed_seconds",
        "log_path",
        "log_sha256",
    }
)
COMPACT_KEY_EVIDENCE_FIELDS: frozenset[str] = frozenset(
    {
        "schema",
        "generated_at_utc",
        "opening_len",
        "ipa_k",
        "verifier_backend",
        "circuit_id",
        "record_namespace",
        "record_version",
        "command",
        "generator_log_path",
        "generator_log_sha256",
        "artifacts",
        "artifact_size_bytes",
    }
)
LOCALNET_LIFECYCLE_EVIDENCE_FIELDS: frozenset[str] = frozenset(
    {
        "schema",
        "generated_at_utc",
        "localnet_run_id",
        "chain_id",
        "localnet_acceptance",
    }
)
LOCALNET_LIFECYCLE_ACCEPTANCE_FIELDS: frozenset[str] = frozenset(
    {
        "run_id",
        "target",
        "peer_count",
        "peer_ids",
        "chain_id",
        "smoke_passed",
        "smoke_tx_hash",
        "replay_rejected",
        "replay_rejection_hash",
        "restart_persistence_checked",
        "restart_replay_rejected",
        "restart_replay_rejection_hash",
        "state_recovery_passed",
        "state_recovery_hash",
        "lifecycle_passed",
        "lifecycle_shield_tx_hash",
        "lifecycle_hop_proof_hash",
        "lifecycle_recursive_init_hash",
        "lifecycle_recursive_init_verify_hash",
        "lifecycle_recursive_append_hash",
        "lifecycle_recursive_append_verify_hash",
        "lifecycle_unshield_proof_hash",
        "lifecycle_redeem_tx_hash",
    }
)
LOCALNET_LIFECYCLE_HASH_FIELDS: tuple[str, ...] = (
    "smoke_tx_hash",
    "replay_rejection_hash",
    "restart_replay_rejection_hash",
    "state_recovery_hash",
    "lifecycle_shield_tx_hash",
    "lifecycle_hop_proof_hash",
    "lifecycle_recursive_init_hash",
    "lifecycle_recursive_init_verify_hash",
    "lifecycle_recursive_append_hash",
    "lifecycle_recursive_append_verify_hash",
    "lifecycle_unshield_proof_hash",
    "lifecycle_redeem_tx_hash",
)
LOCALNET_LIFECYCLE_TRUE_FIELDS: tuple[str, ...] = (
    "smoke_passed",
    "replay_rejected",
    "restart_persistence_checked",
    "restart_replay_rejected",
    "state_recovery_passed",
    "lifecycle_passed",
)
EXPECTED_LOCALNET_TARGET = "localnet"
EXPECTED_LOCALNET_PEER_COUNT = 4
ABI6_OPERATION_SYMBOLS = (
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
EXPECTED_ABI6_OPERATIONS = [
    {
        "name": "init",
        "symbol": "connect_norito_kagemusha_recursive_spend_init",
        "input_archives": ["KagemushaRecursiveSpendInitRequestV1"],
        "output_archive": "KagemushaRecursiveSpendBundleV1",
        "output_kind": "bundle",
    },
    {
        "name": "append",
        "symbol": "connect_norito_kagemusha_recursive_spend_append",
        "input_archives": ["KagemushaRecursiveSpendAppendRequestV1"],
        "output_archive": "KagemushaRecursiveSpendBundleV1",
        "output_kind": "bundle",
    },
    {
        "name": "transition_profile_init",
        "symbol": (
            "connect_norito_kagemusha_recursive_spend_transition_profile_init"
        ),
        "input_archives": ["KagemushaRecursiveSpendInitRequestV1"],
        "output_archive": "KagemushaRecursiveSpendTransitionProfileV1",
        "output_kind": "transition_profile",
    },
    {
        "name": "transition_profile_append",
        "symbol": (
            "connect_norito_kagemusha_recursive_spend_transition_profile_append"
        ),
        "input_archives": ["KagemushaRecursiveSpendAppendRequestV1"],
        "output_archive": "KagemushaRecursiveSpendTransitionProfileV1",
        "output_kind": "transition_profile",
    },
    {
        "name": "lineage_append_boundary",
        "symbol": (
            "connect_norito_kagemusha_recursive_spend_lineage_append_boundary"
        ),
        "input_archives": ["KagemushaRecursiveSpendTransitionProfileV1"],
        "output_archive": "KagemushaRecursiveSpendLineageAppendBoundaryV1",
        "output_kind": "append_boundary",
    },
    {
        "name": "lineage_witness_from_init_result",
        "symbol": (
            "connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result"
        ),
        "input_archives": [
            "KagemushaRecursiveSpendInitRequestV1",
            "KagemushaRecursiveSpendBundleV1",
        ],
        "output_archive": "KagemushaRecursiveSpendLineageWitnessV1",
        "output_kind": "lineage_witness",
    },
    {
        "name": "lineage_witness_append_result",
        "symbol": (
            "connect_norito_kagemusha_recursive_spend_lineage_witness_append_result"
        ),
        "input_archives": [
            "KagemushaRecursiveSpendLineageWitnessV1",
            "KagemushaRecursiveSpendAppendRequestV1",
            "KagemushaRecursiveSpendBundleV1",
        ],
        "output_archive": "KagemushaRecursiveSpendLineageWitnessV1",
        "output_kind": "lineage_witness",
    },
    {
        "name": "verify",
        "symbol": "connect_norito_kagemusha_recursive_spend_verify",
        "input_archives": ["KagemushaRecursiveSpendVerifyRequestV1"],
        "output_archive": "KagemushaRecursiveSpendVerifyResultV1",
        "output_kind": "verify_result",
    },
    {
        "name": "redeem",
        "symbol": "connect_norito_kagemusha_recursive_spend_redeem",
        "input_archives": ["KagemushaRecursiveSpendRedeemRequestV1"],
        "output_archive": "RedeemKagemushaRecursive",
        "output_kind": "instruction",
    },
]
ABI7_FIXTURE_OPERATIONS = (
    {
        "name": "append_bundle",
        "operation": "append",
        "norito_type": "KagemushaRecursiveSpendBundleV1",
        "archive_kind": "bundle",
    },
    {
        "name": "verify_request",
        "operation": "verify",
        "norito_type": "KagemushaRecursiveSpendVerifyRequestV1",
        "archive_kind": "request",
    },
    {
        "name": "verify_result",
        "operation": "verify",
        "norito_type": "KagemushaRecursiveSpendVerifyResultV1",
        "archive_kind": "result",
    },
    {
        "name": "redeem_request",
        "operation": "redeem",
        "norito_type": "KagemushaRecursiveSpendRedeemRequestV1",
        "archive_kind": "request",
    },
    {
        "name": "redeem_instruction",
        "operation": "redeem",
        "norito_type": "RedeemKagemushaRecursive",
        "archive_kind": "instruction",
    },
)
ABI7_FIXTURE_GENERATOR = {
    "crate": "iroha_python_rs",
    "test": "kagemusha_recursive_spend_abi7_archive_fixture_matches_python_native_bridge",
    "print_env": "KAGEMUSHA_RECURSIVE_SPEND_PRINT_ABI7_ARCHIVES",
}
ABI7_FIXTURE_DOMAINS = {
    "lineage_accumulator": "iroha:kagemusha:v1:recursive-spend-accumulator",
    "fixture_label": "kagemusha-recursive-spend-python-real",
}
ABI7_FIXTURE_GENERATOR_FIELDS = frozenset(("crate", "test", "print_env"))
ABI7_FIXTURE_DOMAINS_FIELDS = frozenset(("lineage_accumulator", "fixture_label"))
ABI7_FIXTURE_MANIFEST_FIELDS = frozenset(
    (
        "schema",
        "fixture_kind",
        "archive_fixture",
        "native_bridge_abi_version",
        "operation_count",
        "generator",
        "domains",
        "operations",
    )
)
ABI7_FIXTURE_OPERATION_FIELDS = frozenset(("name", "operation", "norito_type", "archive_kind"))
ABI7_FIXTURE_ARCHIVE_REF_FIELDS = frozenset(("path", "schema"))
ABI7_ARCHIVE_FIXTURE_FIELDS = frozenset(
    ("schema", "fixture_kind", "native_bridge_abi_version", "archives")
)
ABI7_ARCHIVE_FIXTURE_ENTRY_FIELDS = frozenset(
    ("name", "operation", "norito_type", "byte_len", "sha256_hex", "bytes_base64")
)
EXPECTED_ABI6_LIMITS = {
    "compact_token_max_hops": 64,
    "reserved_lineage_witnessless_max_hops": 64,
    "previous_proof_open_envelopes_required_count": 1,
    "native_archive_max_bytes": 64 * 1024 * 1024,
}
EXPECTED_ABI6_LIMIT_VALUES = {
    **EXPECTED_ABI6_LIMITS,
    "previous_proof_open_envelopes_max_bytes": 8 * 1024 * 1024,
    "pallas_open_envelope_max_transcript_label_bytes": 128,
}
ABI6_MANIFEST_FIELDS = frozenset(
    (
        "schema",
        "fixture_kind",
        "archive_fixture",
        "native_bridge_abi_version",
        "operation_count",
        "proof_circuit_ids",
        "limits",
        "domains",
        "modes",
        "operations",
        "hop_policy",
        "payload_benchmarks",
    )
)
ABI6_OPERATION_FIELDS = frozenset(
    ("name", "symbol", "input_archives", "output_archive", "output_kind")
)
ABI6_LIMIT_FIELDS = frozenset(EXPECTED_ABI6_LIMIT_VALUES)
ABI6_MODE_FIELDS = frozenset(
    (
        "preferred_when_recursive_available",
        "fallback_when_recursive_unavailable",
    )
)
EXPECTED_ABI6_MODES = {
    "preferred_when_recursive_available": "recursive_spend_v1",
    "fallback_when_recursive_unavailable": "checked_prefold_v1",
}
ABI6_ARCHIVE_FIXTURE_REF_FIELDS = frozenset(("path", "schema"))
ABI6_PROOF_CIRCUIT_ID_FIELDS = frozenset(
    (
        "recursive_aggregation",
        "reserved_lineage",
        "reserved_lineage_one_hop",
        "reserved_lineage_append",
    )
)
ABI6_DOMAIN_FIELDS = frozenset(
    (
        "transition_profile",
        "transition_profile_digest",
        "transition_profile_binding_digest",
        "lineage_append_openings_preflight",
        "lineage_append_boundary",
        "lineage_append_boundary_chain_asset_binding",
        "lineage_append_boundary_final_note_binding",
    )
)
ABI6_HOP_POLICY_FIELDS = frozenset(
    (
        "preferred_append_output",
        "append_witnessless",
        "redeem_witnessless",
    )
)
ABI6_HOP_POLICY_ENTRY_FIELDS = {
    "preferred_append_output": frozenset(("previous_hop_count", "circuit_id")),
    "append_witnessless": frozenset(("previous_hop_count", "allowed")),
    "redeem_witnessless": frozenset(
        ("circuit_id", "hop_count", "allowed", "requires_lineage_witness")
    ),
}
ABI6_PAYLOAD_BENCHMARK_FIELDS = frozenset(
    (
        "hops",
        "semantic_payload_bytes",
        "semantic_payload_max_bytes",
        "semantic_transition_profile_bytes",
        "semantic_transition_profile_max_bytes",
        "reserved_lineage_payload_bytes",
        "reserved_lineage_payload_max_bytes",
        "reserved_lineage_transition_profile_bytes",
        "reserved_lineage_transition_profile_max_bytes",
    )
)
EXPECTED_ABI6_ARCHIVE_FIXTURE = {
    "path": "fixtures/kagemusha_recursive_spend_abi6/archives.json",
    "schema": "iroha.kagemusha.recursive_spend.abi6.archive_fixtures.v1",
}
EXPECTED_ABI6_PROOF_CIRCUIT_IDS = {
    "recursive_aggregation": "kagemusha-recursive-aggregation-v1",
    "reserved_lineage": "kagemusha-recursive-spend-lineage-v1",
    "reserved_lineage_one_hop": "kagemusha-recursive-spend-lineage-onehop-v1",
    "reserved_lineage_append": "kagemusha-recursive-spend-lineage-append-v1",
}
EXPECTED_ABI6_DOMAINS = {
    "transition_profile": "iroha:kagemusha:v1:recursive-spend-transition-profile",
    "transition_profile_digest": (
        "iroha:kagemusha:v1:recursive-spend-transition-profile-digest"
    ),
    "transition_profile_binding_digest": (
        "iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest"
    ),
    "lineage_append_openings_preflight": (
        "iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1"
    ),
    "lineage_append_boundary": (
        "iroha:kagemusha:recursive-spend-lineage-append-boundary:v1"
    ),
    "lineage_append_boundary_chain_asset_binding": (
        "iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1"
    ),
    "lineage_append_boundary_final_note_binding": (
        "iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1"
    ),
}
EXPECTED_ABI6_HOP_POLICY = {
    "preferred_append_output": [
        {
            "previous_hop_count": 1,
            "circuit_id": "kagemusha-recursive-spend-lineage-append-v1",
        },
        {
            "previous_hop_count": 63,
            "circuit_id": "kagemusha-recursive-spend-lineage-append-v1",
        },
        {
            "previous_hop_count": 64,
            "circuit_id": "kagemusha-recursive-aggregation-v1",
        },
        {
            "previous_hop_count": 0,
            "circuit_id": "kagemusha-recursive-aggregation-v1",
        },
    ],
    "append_witnessless": [
        {"previous_hop_count": 0, "allowed": False},
        {"previous_hop_count": 1, "allowed": True},
        {"previous_hop_count": 63, "allowed": True},
        {"previous_hop_count": 64, "allowed": False},
    ],
    "redeem_witnessless": [
        {
            "circuit_id": "kagemusha-recursive-spend-lineage-v1",
            "hop_count": 1,
            "allowed": True,
            "requires_lineage_witness": False,
        },
        {
            "circuit_id": "kagemusha-recursive-spend-lineage-onehop-v1",
            "hop_count": 1,
            "allowed": True,
            "requires_lineage_witness": False,
        },
        {
            "circuit_id": "kagemusha-recursive-spend-lineage-append-v1",
            "hop_count": 2,
            "allowed": True,
            "requires_lineage_witness": False,
        },
        {
            "circuit_id": "kagemusha-recursive-spend-lineage-v1",
            "hop_count": 64,
            "allowed": True,
            "requires_lineage_witness": False,
        },
        {
            "circuit_id": "kagemusha-recursive-aggregation-v1",
            "hop_count": 1,
            "allowed": False,
            "requires_lineage_witness": True,
        },
        {
            "circuit_id": "kagemusha-recursive-spend-lineage-v1",
            "hop_count": 0,
            "allowed": False,
            "requires_lineage_witness": True,
        },
        {
            "circuit_id": "kagemusha-recursive-spend-lineage-v1",
            "hop_count": 65,
            "allowed": False,
            "requires_lineage_witness": True,
        },
    ],
}
EXPECTED_ABI6_PAYLOAD_BENCHMARKS = {
    "hops": [1, 2, 3, 5, 8, 13, 21, 34, 55, 64],
    "semantic_payload_bytes": 1751,
    "semantic_payload_max_bytes": 2048,
    "semantic_transition_profile_bytes": 2094,
    "semantic_transition_profile_max_bytes": 3072,
    "reserved_lineage_payload_bytes": 3847,
    "reserved_lineage_payload_max_bytes": 8192,
    "reserved_lineage_transition_profile_bytes": 2817,
    "reserved_lineage_transition_profile_max_bytes": 4096,
}
LINEAGE_KEY_RELEASE_TOOLING_REQUIREMENTS = {
    "crates/iroha_cli/src/zk.rs": (
        "KagemushaCommand::LineageKeyArtifacts",
        "KagemushaCommand::RecursiveCompactKeyArtifacts",
        "KagemushaCommand::LineageRecord",
        "KagemushaRecursiveCompactKeyArtifactsArgs",
        "KagemushaLineageRecordArgs",
        "derive_halo2_ipa_kagemusha_recursive_compact_payment_token_proving_key_bytes",
        "kagemusha_recursive_compact_payment_token_vk_record_from_box",
        "record_out: Option<std::path::PathBuf>",
        "record_namespace: String",
        "record_version: u32",
        "kagemusha_lineage_vk_record_from_bytes",
        "std::fs::read(&self.vk)",
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
    "crates/iroha_core/src/zk.rs": (
        "kagemusha_recursive_spend_lineage_vk_record_from_box_for_circuit",
        "pub fn kagemusha_recursive_spend_lineage_vk_record_from_box(",
        "pub fn kagemusha_recursive_spend_lineage_append_vk_record_from_box(",
        "does not generate a verifier key at runtime",
        "lineage_vk_record_from_box_canonicalizes_profiles_without_keygen",
    ),
    "docs/source/offline_kagemusha.md": (
        "--record-out artifacts/kagemusha/lineage-init-len128.record.norito",
        "--record-out artifacts/kagemusha/lineage-append-len128.record.norito",
        "iroha app zk kagemusha recursive-compact-key-artifacts",
        "--record-out artifacts/kagemusha/recursive-compact-len4.record.norito",
        "--pk-out artifacts/kagemusha/recursive-compact-len4.pk",
        "iroha app zk kagemusha lineage-record",
        "--vk artifacts/kagemusha/lineage-init-len128.vk",
        "--vk artifacts/kagemusha/lineage-append-len128.vk",
        "governance/WSV `VerifyingKeyRecord` bound to `offline_kagemusha`",
        "`--record-namespace` and `--record-version`",
    ),
}
SUMMARY_OUT_PATH_INVALID_CODE = "kagemusha_summary_out_path_invalid"


def utc_now() -> str:
    """Return a canonical current UTC timestamp."""

    return dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat().replace(
        "+00:00",
        "Z",
    )


def blocker(code: str, message: str, **extra: Any) -> dict[str, Any]:
    """Build a normalized readiness blocker."""

    item: dict[str, Any] = {"code": code, "message": message}
    item.update(extra)
    return item


def _secret_looking_path_blocker(
    value: str | None,
    *,
    label: str,
    code: str,
) -> dict[str, Any] | None:
    if value is not None and device_lab.SECRET_RE.search(value):
        return blocker(code, f"{label} must not contain secret-looking material")
    if value is not None and device_lab._contains_control_character(value):
        return blocker(code, f"{label} must not contain control characters")
    return None


def _repo_root_shape_blocker(root: Path) -> dict[str, Any] | None:
    """Reject repo-root aliases before filesystem metadata reads."""

    root_text = str(root)
    if (
        root_text != root_text.strip()
        or device_lab._path_has_surrounding_whitespace_component(root)
    ):
        return blocker(
            "kagemusha_repo_root_path_invalid",
            "--repo-root must not contain surrounding whitespace",
        )
    if "\\" in root_text:
        return blocker(
            "kagemusha_repo_root_path_invalid",
            "--repo-root must not contain backslashes",
        )
    if ".." in root.parts:
        return blocker(
            "kagemusha_repo_root_path_invalid",
            "--repo-root must be a canonical directory path",
        )
    return None


def _cli_path_shape_blocker(
    value: str | None,
    *,
    label: str,
    code: str,
) -> dict[str, Any] | None:
    """Reject local CLI path aliases before resolver normalization."""

    if value is None:
        return None
    candidate = Path(value)
    if (
        value != value.strip()
        or device_lab._path_has_surrounding_whitespace_component(candidate)
    ):
        return blocker(code, f"{label} must not contain surrounding whitespace")
    if "\\" in value:
        return blocker(code, f"{label} must not contain backslashes")
    if ".." in candidate.parts:
        return blocker(code, f"{label} must be a canonical path")
    return None


def validate_repo_root_path(root: Path) -> list[dict[str, Any]]:
    """Reject repo roots that could alias checked-in readiness trust roots."""

    secret_blocker = _secret_looking_path_blocker(
        str(root),
        label="--repo-root",
        code="kagemusha_repo_root_path_invalid",
    )
    if secret_blocker is not None:
        return [secret_blocker]
    shape_blocker = _repo_root_shape_blocker(root)
    if shape_blocker is not None:
        return [shape_blocker]
    errors: list[str] = []
    try:
        root_mode = root.lstat().st_mode
    except FileNotFoundError:
        root_mode = None
    except OSError:
        errors.append("--repo-root metadata could not be read")
        return [
            blocker("kagemusha_repo_root_path_invalid", error)
            for error in errors
        ]
    if root_mode is not None and stat.S_ISLNK(root_mode):
        errors.append("--repo-root must not be a symlink")
    errors.extend(
        device_lab.validate_no_symlink_ancestors(
            root,
            "--repo-root ancestor directory",
        )
    )
    if root_mode is not None and not stat.S_ISDIR(root_mode):
        errors.append("--repo-root must be a directory")
    if root_mode is None:
        errors.append("--repo-root must be an existing directory")
    return [
        blocker("kagemusha_repo_root_path_invalid", error)
        for error in errors
    ]


def _device_lab_root_arg_values(args: argparse.Namespace) -> list[str]:
    """Return CLI device-lab root values, preserving the legacy default."""

    raw = getattr(args, "device_lab_root", None)
    if raw is None:
        return [DEFAULT_ANDROID_DEVICE_LAB_ROOT_PATH]
    if isinstance(raw, str):
        return [raw]
    return list(raw)


def validate_cli_path_arguments(args: argparse.Namespace) -> list[dict[str, Any]]:
    """Reject unsafe local path arguments before summaries are built."""

    blockers: list[dict[str, Any]] = []
    for value, label, code in (
        (args.repo_root, "--repo-root", "kagemusha_repo_root_path_invalid"),
        (args.summary_out, "--summary-out", SUMMARY_OUT_PATH_INVALID_CODE),
        (
            args.lineage_proof_evidence,
            "--lineage-proof-evidence",
            "lineage_proof_evidence_path_invalid",
        ),
        (
            args.compact_key_evidence,
            "--compact-key-evidence",
            "compact_key_evidence_path_invalid",
        ),
        (
            args.localnet_lifecycle_evidence,
            "--localnet-lifecycle-evidence",
            "localnet_lifecycle_evidence_path_invalid",
        ),
    ):
        item = _secret_looking_path_blocker(value, label=label, code=code)
        if item is not None:
            blockers.append(item)
            continue
        if label != "--repo-root":
            item = _cli_path_shape_blocker(value, label=label, code=code)
            if item is not None:
                blockers.append(item)
    if not any(item["code"] == "kagemusha_repo_root_path_invalid" for item in blockers):
        repo_root_errors = validate_repo_root_path(Path(args.repo_root))
        blockers.extend(repo_root_errors)
    root_values = _device_lab_root_arg_values(args)
    for index, value in enumerate(root_values):
        label = (
            "--device-lab-root"
            if len(root_values) == 1
            else f"--device-lab-root[{index}]"
        )
        item = _secret_looking_path_blocker(
            value,
            label=label,
            code="android_device_lab_root_path_invalid",
        )
        if item is not None:
            blockers.append(item)
            continue
        item = _cli_path_shape_blocker(
            value,
            label=label,
            code="android_device_lab_root_path_invalid",
        )
        if item is not None:
            blockers.append(item)
    for index, value in enumerate(args.trusted_signer_public_keys or []):
        item = _secret_looking_path_blocker(
            value,
            label=f"--trusted-signer-public-key[{index}]",
            code="android_trusted_signer_path_invalid",
        )
        if item is not None:
            blockers.append(item)
            continue
        item = _cli_path_shape_blocker(
            value,
            label=f"--trusted-signer-public-key[{index}]",
            code="android_trusted_signer_path_invalid",
        )
        if item is not None:
            blockers.append(item)
    return blockers


def parse_utc_timestamp(value: str, label: str) -> tuple[dt.datetime | None, dict[str, Any] | None]:
    """Parse an ISO-8601 timestamp and normalize it to UTC."""

    try:
        parsed = dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return None, blocker(
            "timestamp_invalid",
            f"{label} must be an ISO-8601 timestamp",
        )
    if parsed.tzinfo is None:
        return None, blocker(
            "timestamp_timezone_missing",
            f"{label} must include a timezone",
        )
    return parsed.astimezone(dt.timezone.utc), None


def _append_evidence_timestamp_string_blockers(
    blockers: list[dict[str, Any]],
    *,
    code_prefix: str,
    label: str,
    raw: str,
) -> bool:
    """Append string-shape blockers and return whether parsing must be skipped."""

    if raw != raw.strip():
        blockers.append(
            blocker(
                f"{code_prefix}_timestamp_surrounding_whitespace",
                f"{label} generated_at_utc must not contain surrounding whitespace",
                generated_at_utc=_display_evidence_value(raw),
            )
        )
    if device_lab._contains_control_character(raw):
        blockers.append(
            blocker(
                f"{code_prefix}_timestamp_control_character",
                f"{label} generated_at_utc must not contain control characters",
            )
        )
        return True
    return False


class DuplicateJsonKeyError(ValueError):
    """Raised when a JSON object contains a duplicate key."""

    def __init__(self, key: str) -> None:
        self.key = key
        super().__init__(key)


class NonFiniteJsonConstantError(ValueError):
    """Raised when release evidence JSON uses non-standard numeric constants."""

    def __init__(self, constant: str) -> None:
        self.constant = constant
        super().__init__(constant)


def _display_json_key(key: str) -> str:
    if device_lab.SECRET_RE.search(key):
        return device_lab.SECRET_PATH_REDACTION
    if device_lab._contains_control_character(key):
        return device_lab.CONTROL_PATH_REDACTION
    return key


def _reject_duplicate_json_object_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    item: dict[str, Any] = {}
    for key, value in pairs:
        if key in item:
            raise DuplicateJsonKeyError(key)
        item[key] = value
    return item


def _reject_nonfinite_json_constant(constant: str) -> None:
    raise NonFiniteJsonConstantError(constant)


def _read_release_json_text(
    path: Path,
    label: str,
    *,
    missing_code: str,
    shape_code: str,
    unreadable_code: str,
    max_bytes: int | None = None,
) -> tuple[str | None, list[dict[str, Any]]]:
    if max_bytes is None:
        max_bytes = MAX_ABI6_MANIFEST_JSON_BYTES
    expected_stat, shape_errors = _validate_release_local_json_file_for_read(path, label)
    if shape_errors:
        missing_error = f"{label} is missing"
        if shape_errors == [missing_error]:
            return None, [blocker(missing_code, f"missing {label}")]
        return None, [blocker(shape_code, error) for error in shape_errors]
    assert expected_stat is not None
    chunks: list[bytes] = []
    size = 0
    release_json_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            release_json_path_stat = path.lstat()
            if stat.S_ISLNK(release_json_path_stat.st_mode):
                return None, [blocker(shape_code, f"{label} must not be a symlink")]
            if not stat.S_ISREG(release_json_path_stat.st_mode) or not stat.S_ISREG(
                open_stat.st_mode
            ):
                return None, [blocker(shape_code, f"{label} must be a regular file")]
            release_json_open_identity = (open_stat.st_dev, open_stat.st_ino)
            if release_json_open_identity != release_json_expected_identity or (
                release_json_path_stat.st_dev,
                release_json_path_stat.st_ino,
            ) != release_json_expected_identity:
                return None, [blocker(shape_code, f"{label} changed while being read")]
            if open_stat.st_nlink > 1:
                return None, [blocker(shape_code, f"{label} must not be hardlinked")]
            if open_stat.st_size > max_bytes:
                return None, [
                    blocker(
                        shape_code,
                        f"{label} must be no more than {max_bytes} bytes",
                    )
                ]
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if size > max_bytes:
                    return None, [
                        blocker(
                            shape_code,
                            f"{label} must be no more than {max_bytes} bytes",
                        )
                    ]
                chunks.append(chunk)
            release_json_final_path_stat = path.lstat()
            if (
                release_json_final_path_stat.st_dev,
                release_json_final_path_stat.st_ino,
            ) != release_json_expected_identity:
                return None, [blocker(shape_code, f"{label} changed while being read")]
    except OSError:
        return None, [blocker(unreadable_code, f"{label} could not be read")]
    try:
        return b"".join(chunks).decode("utf-8"), []
    except UnicodeDecodeError:
        return None, [blocker(unreadable_code, f"{label} could not be read")]


def _load_release_json_data(
    path: Path,
    label: str,
    *,
    missing_code: str,
    shape_code: str,
    unreadable_code: str,
    invalid_code: str,
    not_object_code: str,
    max_bytes: int | None = None,
) -> tuple[dict[str, Any] | None, str | None, list[dict[str, Any]]]:
    text, read_blockers = _read_release_json_text(
        path,
        label,
        missing_code=missing_code,
        shape_code=shape_code,
        unreadable_code=unreadable_code,
        max_bytes=max_bytes,
    )
    if read_blockers:
        return None, None, read_blockers
    assert text is not None
    try:
        data = json.loads(
            text,
            object_pairs_hook=_reject_duplicate_json_object_pairs,
            parse_constant=_reject_nonfinite_json_constant,
        )
    except json.JSONDecodeError as exc:
        return None, text, [blocker(invalid_code, f"{label} is not valid JSON: {exc}")]
    except DuplicateJsonKeyError as exc:
        return None, text, [blocker(invalid_code, _duplicate_json_key_message(label, exc))]
    except NonFiniteJsonConstantError:
        return None, text, [
            blocker(
                invalid_code,
                f"{label} is not strict JSON: non-finite constant "
                f"{EVIDENCE_NONFINITE_NUMBER_REDACTION} is not allowed",
            )
        ]
    if not isinstance(data, dict):
        return None, text, [blocker(not_object_code, f"{label} must be a JSON object")]
    return data, text, []


def _duplicate_json_key_message(label: str, exc: DuplicateJsonKeyError) -> str:
    return f"{label} contains duplicate JSON object key {_display_json_key(exc.key)}"


def _load_json(path: Path) -> tuple[dict[str, Any] | None, list[dict[str, Any]]]:
    data, _text, blockers = _load_release_json_data(
        path,
        "ABI-6 manifest",
        missing_code="abi6_manifest_missing",
        shape_code="abi6_manifest_file_shape",
        unreadable_code="abi6_manifest_unreadable",
        invalid_code="abi6_manifest_invalid_json",
        not_object_code="abi6_manifest_not_object",
    )
    return data, blockers


def validate_release_local_json_file(path: Path, label: str) -> list[str]:
    """Reject local release JSON files that could alias external bytes."""

    _file_stat, errors = _validate_release_local_json_file_for_read(path, label)
    return errors


def _validate_release_local_json_file_for_read(
    path: Path,
    label: str,
) -> tuple[os.stat_result | None, list[str]]:
    """Reject local release JSON files and return the read identity."""

    path_text = str(path)
    if device_lab.SECRET_RE.search(path_text):
        return None, [f"{label} path must not contain secret-looking material"]
    if device_lab._contains_control_character(path_text):
        return None, [f"{label} path must not contain control characters"]
    if path_text != path_text.strip() or device_lab._path_has_surrounding_whitespace_component(  # type: ignore[attr-defined]
        path
    ):
        return None, [f"{label} path must not contain surrounding whitespace"]
    if "\\" in path_text:
        return None, [f"{label} path must not contain backslashes"]
    if ".." in path.parts:
        return None, [f"{label} path must be canonical"]
    release_json_ancestor_errors = device_lab.validate_no_symlink_ancestors(
        path,
        f"{label} ancestor directory",
    )
    if release_json_ancestor_errors:
        return None, release_json_ancestor_errors
    try:
        file_stat = path.lstat()
    except FileNotFoundError:
        return None, [f"{label} is missing"]
    except OSError:
        return None, [f"{label} file metadata could not be read"]
    if stat.S_ISLNK(file_stat.st_mode):
        return None, [f"{label} must not be a symlink"]
    if not stat.S_ISREG(file_stat.st_mode):
        return None, [f"{label} must be a regular file"]
    try:
        link_count = path.stat().st_nlink
    except OSError:
        return None, [f"{label} hardlink metadata could not be read"]
    if link_count > 1:
        return None, [f"{label} must not be hardlinked"]
    return file_stat, []


def _validate_repo_source_marker_file_for_read(
    path: Path,
    label: str,
) -> tuple[os.stat_result | None, list[str]]:
    """Reject checked-in marker files that could alias external bytes."""

    path_text = str(path)
    if device_lab.SECRET_RE.search(path_text):
        return None, [f"{label} path must not contain secret-looking material"]
    if device_lab._contains_control_character(path_text):
        return None, [f"{label} path must not contain control characters"]
    if path_text != path_text.strip() or device_lab._path_has_surrounding_whitespace_component(  # type: ignore[attr-defined]
        path
    ):
        return None, [f"{label} path must not contain surrounding whitespace"]
    if "\\" in path_text:
        return None, [f"{label} path must not contain backslashes"]
    if ".." in path.parts:
        return None, [f"{label} path must be canonical"]
    errors = [
        *device_lab.validate_no_symlink_ancestors(
            path,
            f"{label} ancestor directory",
        )
    ]
    try:
        file_stat = path.lstat()
    except FileNotFoundError:
        errors.append(f"{label} is missing")
        return None, errors
    except OSError:
        errors.append(f"{label} file metadata could not be read")
        return None, errors
    if stat.S_ISLNK(file_stat.st_mode):
        errors.append(f"{label} must not be a symlink")
        return None, errors
    if not stat.S_ISREG(file_stat.st_mode):
        errors.append(f"{label} must be a regular file")
        return None, errors
    try:
        link_count = path.stat().st_nlink
    except OSError:
        errors.append(f"{label} hardlink metadata could not be read")
        return None, errors
    if link_count > 1:
        errors.append(f"{label} must not be hardlinked")
    if errors:
        return None, errors
    return file_stat, []


def validate_repo_source_marker_file(path: Path, label: str) -> list[str]:
    """Reject checked-in marker files that could alias external bytes."""

    _file_stat, errors = _validate_repo_source_marker_file_for_read(path, label)
    return errors


def _repo_source_marker_text(
    path: Path,
    label: str,
    unreadable_error: str,
) -> tuple[str | None, list[str]]:
    """Validate a checked-in source marker immediately before reading text."""

    expected_stat, file_errors = _validate_repo_source_marker_file_for_read(path, label)
    if file_errors:
        return None, file_errors
    assert expected_stat is not None
    expected_marker_identity = (expected_stat.st_dev, expected_stat.st_ino)
    chunks: list[bytes] = []
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            marker_path_stat = path.lstat()
            if stat.S_ISLNK(marker_path_stat.st_mode):
                return None, [f"{label} must not be a symlink"]
            if not stat.S_ISREG(marker_path_stat.st_mode) or not stat.S_ISREG(
                open_stat.st_mode
            ):
                return None, [f"{label} must be a regular file"]
            open_marker_identity = (open_stat.st_dev, open_stat.st_ino)
            if open_marker_identity != expected_marker_identity or (
                marker_path_stat.st_dev,
                marker_path_stat.st_ino,
            ) != expected_marker_identity:
                return None, [f"{label} changed while being read"]
            if open_stat.st_nlink > 1:
                return None, [f"{label} must not be hardlinked"]
            if open_stat.st_size > MAX_REPO_SOURCE_MARKER_BYTES:
                return None, [
                    f"{label} must be no more than {MAX_REPO_SOURCE_MARKER_BYTES} bytes"
                ]
            size = 0
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if size > MAX_REPO_SOURCE_MARKER_BYTES:
                    return None, [
                        f"{label} must be no more than {MAX_REPO_SOURCE_MARKER_BYTES} bytes"
                    ]
                chunks.append(chunk)
            marker_final_path_stat = path.lstat()
            if (marker_final_path_stat.st_dev, marker_final_path_stat.st_ino) != (
                expected_marker_identity
            ):
                return None, [f"{label} changed while being read"]
    except OSError:
        return None, [unreadable_error]
    try:
        return b"".join(chunks).decode("utf-8"), []
    except UnicodeDecodeError:
        return None, [unreadable_error]


def _load_json_artifact(
    path: Path,
    *,
    missing_code: str,
    invalid_code: str,
    unreadable_code: str,
    shape_code: str,
    not_object_code: str,
    label: str,
    max_bytes: int | None = None,
) -> tuple[dict[str, Any] | None, list[dict[str, Any]]]:
    size_error = (
        f"{label} must be no more than {max_bytes} bytes"
        if max_bytes is not None
        else None
    )
    digest, text, read_errors = _sha256_text_file(
        path,
        label,
        f"{label} could not be read",
        max_bytes=max_bytes,
        too_large_error=size_error,
    )
    if read_errors:
        blockers: list[dict[str, Any]] = []
        missing_error = f"{label} is missing"
        unreadable_error = f"{label} could not be read"
        for error in read_errors:
            if error == missing_error:
                blockers.append(blocker(missing_code, f"missing {label}"))
            elif error == unreadable_error:
                blockers.append(blocker(unreadable_code, error))
            else:
                blockers.append(blocker(shape_code, error))
        return None, blockers
    assert digest is not None and text is not None
    try:
        data = json.loads(
            text,
            object_pairs_hook=_reject_duplicate_json_object_pairs,
            parse_constant=_reject_nonfinite_json_constant,
        )
    except json.JSONDecodeError as exc:
        return None, [blocker(invalid_code, f"{label} is not valid JSON: {exc}")]
    except DuplicateJsonKeyError as exc:
        return None, [blocker(invalid_code, _duplicate_json_key_message(label, exc))]
    except NonFiniteJsonConstantError:
        return None, [
            blocker(
                invalid_code,
                f"{label} is not strict JSON: non-finite constant "
                f"{EVIDENCE_NONFINITE_NUMBER_REDACTION} is not allowed",
            )
        ]
    if not isinstance(data, dict):
        return None, [blocker(not_object_code, f"{label} must be a JSON object")]
    return data, []


def _is_expected_json_int(value: Any, expected: int) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value == expected


def _json_exactly_matches(value: Any, expected: Any) -> bool:
    if isinstance(expected, bool):
        return isinstance(value, bool) and value is expected
    if isinstance(expected, int):
        return _is_expected_json_int(value, expected)
    if isinstance(expected, str):
        return isinstance(value, str) and value == expected
    if isinstance(expected, list):
        return (
            isinstance(value, list)
            and len(value) == len(expected)
            and all(
                _json_exactly_matches(actual_item, expected_item)
                for actual_item, expected_item in zip(value, expected)
            )
        )
    if isinstance(expected, dict):
        return (
            isinstance(value, dict)
            and set(value) == set(expected)
            and all(
                _json_exactly_matches(value[key], expected_value)
                for key, expected_value in expected.items()
            )
        )
    return value == expected


def check_abi6_reserved_lineage(repo_root: Path) -> dict[str, Any]:
    """Check the checked-in ABI-6 Reserved-lineage manifest."""

    details: dict[str, Any] = {"manifest_path": ABI6_MANIFEST_PATH}
    repo_root_blockers = validate_repo_root_path(repo_root)
    if repo_root_blockers:
        details["ok"] = False
        details["blockers"] = repo_root_blockers
        return details

    manifest_path = repo_root / ABI6_MANIFEST_PATH
    manifest, blockers = _load_json(manifest_path)
    if manifest is not None:
        details["schema"] = manifest.get("schema")
        details["native_bridge_abi_version"] = manifest.get("native_bridge_abi_version")
        details["operation_count"] = manifest.get("operation_count")
        _append_unexpected_json_field_blockers(
            blockers,
            manifest,
            ABI6_MANIFEST_FIELDS,
            code="abi6_manifest_unexpected_field",
            message="ABI-6 manifest contains an unexpected field",
        )
        if manifest.get("schema") != ABI6_MANIFEST_SCHEMA:
            blockers.append(blocker("abi6_manifest_schema", "ABI-6 manifest schema mismatch"))
        if manifest.get("fixture_kind") != "metadata_contract":
            blockers.append(
                blocker(
                    "abi6_manifest_fixture_kind",
                    "ABI-6 manifest fixture_kind must be metadata_contract",
                )
            )
        if not _is_expected_json_int(manifest.get("native_bridge_abi_version"), 6):
            blockers.append(
                blocker("abi6_manifest_bridge_version", "ABI-6 manifest must advertise bridge ABI 6")
            )
        archive_ref = manifest.get("archive_fixture")
        if not isinstance(archive_ref, dict):
            blockers.append(
                blocker(
                    "abi6_manifest_archive_fixture_shape",
                    "ABI-6 manifest archive_fixture must be an object",
                )
            )
        else:
            _append_unexpected_json_field_blockers(
                blockers,
                archive_ref,
                ABI6_ARCHIVE_FIXTURE_REF_FIELDS,
                code="abi6_manifest_archive_fixture_unexpected_field",
                message="ABI-6 manifest archive_fixture contains an unexpected field",
            )
            if not _json_exactly_matches(archive_ref, EXPECTED_ABI6_ARCHIVE_FIXTURE):
                blockers.append(
                    blocker(
                        "abi6_manifest_archive_fixture",
                        "ABI-6 manifest archive_fixture metadata drifted",
                    )
                )
        proof_circuit_ids = manifest.get("proof_circuit_ids")
        if not isinstance(proof_circuit_ids, dict):
            blockers.append(
                blocker(
                    "abi6_manifest_proof_circuit_ids_shape",
                    "ABI-6 manifest proof_circuit_ids must be an object",
                )
            )
        else:
            _append_unexpected_json_field_blockers(
                blockers,
                proof_circuit_ids,
                ABI6_PROOF_CIRCUIT_ID_FIELDS,
                code="abi6_manifest_proof_circuit_ids_unexpected_field",
                message="ABI-6 manifest proof_circuit_ids contain an unexpected field",
            )
            if not _json_exactly_matches(
                proof_circuit_ids,
                EXPECTED_ABI6_PROOF_CIRCUIT_IDS,
            ):
                blockers.append(
                    blocker(
                        "abi6_manifest_proof_circuit_ids",
                        "ABI-6 manifest proof_circuit_ids drifted",
                    )
                )
        domains = manifest.get("domains")
        if not isinstance(domains, dict):
            blockers.append(
                blocker(
                    "abi6_manifest_domains_shape",
                    "ABI-6 manifest domains must be an object",
                )
            )
        else:
            _append_unexpected_json_field_blockers(
                blockers,
                domains,
                ABI6_DOMAIN_FIELDS,
                code="abi6_manifest_domains_unexpected_field",
                message="ABI-6 manifest domains contain an unexpected field",
            )
            if not _json_exactly_matches(domains, EXPECTED_ABI6_DOMAINS):
                blockers.append(
                    blocker(
                        "abi6_manifest_domains",
                        "ABI-6 manifest domains drifted",
                    )
                )
        hop_policy = manifest.get("hop_policy")
        if not isinstance(hop_policy, dict):
            blockers.append(
                blocker(
                    "abi6_manifest_hop_policy_shape",
                    "ABI-6 manifest hop_policy must be an object",
                )
            )
        else:
            _append_unexpected_json_field_blockers(
                blockers,
                hop_policy,
                ABI6_HOP_POLICY_FIELDS,
                code="abi6_manifest_hop_policy_unexpected_field",
                message="ABI-6 manifest hop_policy contains an unexpected field",
            )
            for section, allowed_fields in ABI6_HOP_POLICY_ENTRY_FIELDS.items():
                entries = hop_policy.get(section)
                if not isinstance(entries, list):
                    blockers.append(
                        blocker(
                            "abi6_manifest_hop_policy_shape",
                            "ABI-6 manifest hop_policy sections must be arrays",
                            section=section,
                        )
                    )
                    continue
                for entry in entries:
                    if not isinstance(entry, dict):
                        blockers.append(
                            blocker(
                                "abi6_manifest_hop_policy_entry_shape",
                                "ABI-6 manifest hop_policy entries must be objects",
                                section=section,
                            )
                        )
                        continue
                    _append_unexpected_json_field_blockers(
                        blockers,
                        entry,
                        allowed_fields,
                        code="abi6_manifest_hop_policy_entry_unexpected_field",
                        message="ABI-6 manifest hop_policy entry contains an unexpected field",
                    )
            if not _json_exactly_matches(hop_policy, EXPECTED_ABI6_HOP_POLICY):
                blockers.append(
                    blocker(
                        "abi6_manifest_hop_policy",
                        "ABI-6 manifest hop_policy drifted",
                    )
                )
        payload_benchmarks = manifest.get("payload_benchmarks")
        if not isinstance(payload_benchmarks, dict):
            blockers.append(
                blocker(
                    "abi6_manifest_payload_benchmarks_shape",
                    "ABI-6 manifest payload_benchmarks must be an object",
                )
            )
        else:
            _append_unexpected_json_field_blockers(
                blockers,
                payload_benchmarks,
                ABI6_PAYLOAD_BENCHMARK_FIELDS,
                code="abi6_manifest_payload_benchmarks_unexpected_field",
                message="ABI-6 manifest payload_benchmarks contain an unexpected field",
            )
            if not _json_exactly_matches(
                payload_benchmarks,
                EXPECTED_ABI6_PAYLOAD_BENCHMARKS,
            ):
                blockers.append(
                    blocker(
                        "abi6_manifest_payload_benchmarks",
                        "ABI-6 manifest payload_benchmarks drifted",
                    )
                )
        raw_operations = manifest.get("operations")
        operation_symbols: list[str] = []
        if not isinstance(raw_operations, list):
            blockers.append(
                blocker(
                    "abi6_manifest_operation_shape",
                    "ABI-6 manifest operations must be an array",
                )
            )
        else:
            for operation in raw_operations:
                if isinstance(operation, dict):
                    _append_unexpected_json_field_blockers(
                        blockers,
                        operation,
                        ABI6_OPERATION_FIELDS,
                        code="abi6_manifest_operation_unexpected_field",
                        message="ABI-6 manifest operation contains an unexpected field",
                    )
                if not isinstance(operation, dict) or not isinstance(
                    operation.get("symbol"),
                    str,
                ):
                    blockers.append(
                        blocker(
                            "abi6_manifest_operation_shape",
                            "ABI-6 manifest operations must be objects with string symbols",
                        )
                    )
                    continue
                operation_symbols.append(operation["symbol"])
        operations = tuple(operation_symbols)
        if not _is_expected_json_int(
            manifest.get("operation_count"),
            len(ABI6_OPERATION_SYMBOLS),
        ):
            blockers.append(
                blocker("abi6_manifest_operation_count", "ABI-6 manifest operation count drifted")
            )
        if operations != ABI6_OPERATION_SYMBOLS:
            blockers.append(
                blocker("abi6_manifest_operations", "ABI-6 manifest operation symbols drifted")
            )
        if not _json_exactly_matches(raw_operations, EXPECTED_ABI6_OPERATIONS):
            blockers.append(
                blocker(
                    "abi6_manifest_operation_inventory",
                    "ABI-6 manifest operation inventory drifted",
                )
            )
        limits = manifest.get("limits", {})
        if not isinstance(limits, dict):
            blockers.append(blocker("abi6_manifest_limits", "ABI-6 manifest limits must be an object"))
        else:
            _append_unexpected_json_field_blockers(
                blockers,
                limits,
                ABI6_LIMIT_FIELDS,
                code="abi6_manifest_limit_unexpected_field",
                message="ABI-6 manifest limits contain an unexpected field",
            )
            details["limits"] = {
                key: limits.get(key) for key in sorted(EXPECTED_ABI6_LIMIT_VALUES)
            }
            for key, expected in EXPECTED_ABI6_LIMIT_VALUES.items():
                if not _is_expected_json_int(limits.get(key), expected):
                    blockers.append(
                        blocker(
                            "abi6_manifest_limit",
                            f"ABI-6 manifest limit {key} must be {expected}",
                            limit=key,
                        )
                    )
        modes = manifest.get("modes", {})
        if not isinstance(modes, dict):
            blockers.append(blocker("abi6_manifest_modes", "ABI-6 manifest modes must be an object"))
        else:
            _append_unexpected_json_field_blockers(
                blockers,
                modes,
                ABI6_MODE_FIELDS,
                code="abi6_manifest_mode_unexpected_field",
                message="ABI-6 manifest modes contain an unexpected field",
            )
            details["modes"] = {
                "preferred_when_recursive_available": modes.get(
                    "preferred_when_recursive_available"
                ),
                "fallback_when_recursive_unavailable": modes.get(
                    "fallback_when_recursive_unavailable"
                ),
            }
            if modes.get("preferred_when_recursive_available") != "recursive_spend_v1":
                blockers.append(
                    blocker(
                        "abi6_manifest_preferred_mode",
                        "ABI-6 manifest must prefer recursive_spend_v1",
                    )
                )
            if modes.get("fallback_when_recursive_unavailable") != "checked_prefold_v1":
                blockers.append(
                    blocker(
                        "abi6_manifest_fallback_mode",
                        "ABI-6 manifest must fall back to checked_prefold_v1",
                    )
                )
            if not _json_exactly_matches(modes, EXPECTED_ABI6_MODES):
                blockers.append(
                    blocker("abi6_manifest_modes", "ABI-6 manifest modes drifted")
                )

    details["ok"] = not blockers
    details["blockers"] = blockers
    return details


def _abi7_fixture_operation_names() -> tuple[str, ...]:
    return tuple(operation["name"] for operation in ABI7_FIXTURE_OPERATIONS)


def _decode_abi7_fixture_archive_bytes(
    value: Any,
    name: str,
) -> tuple[bytes | None, list[dict[str, Any]]]:
    if not isinstance(value, str) or not value:
        return None, [
            blocker(
                "abi7_archive_fixture_base64",
                "ABI-7 archive fixture bytes_base64 must be a non-empty base64 string",
                archive=name,
            )
        ]
    try:
        decoded = base64.b64decode(value, validate=True)
    except (binascii.Error, ValueError):
        return None, [
            blocker(
                "abi7_archive_fixture_base64",
                "ABI-7 archive fixture bytes_base64 is not valid base64",
                archive=name,
            )
        ]
    if value != base64.b64encode(decoded).decode("ascii"):
        return None, [
            blocker(
                "abi7_archive_fixture_base64",
                "ABI-7 archive fixture bytes_base64 must be canonical standard base64",
                archive=name,
            )
        ]
    return decoded, []


def _append_unexpected_json_field_blockers(
    blockers: list[dict[str, Any]],
    value: Mapping[str, Any],
    allowed_fields: frozenset[str],
    *,
    code: str,
    message: str,
    archive: str | None = None,
) -> None:
    for field in sorted(set(value) - allowed_fields, key=_display_json_key):
        extra: dict[str, Any] = {"field": _display_json_key(field)}
        if archive is not None:
            extra["archive"] = archive
        blockers.append(blocker(code, message, **extra))


def check_abi7_fixture_manifest(repo_root: Path) -> dict[str, Any]:
    """Check the shared ABI-7 recursive-spend fixture manifest and archives."""

    details: dict[str, Any] = {
        "fixture_manifest_path": ABI7_FIXTURE_MANIFEST_PATH,
        "archive_fixture_path": ABI7_ARCHIVE_FIXTURE_PATH,
    }
    repo_root_blockers = validate_repo_root_path(repo_root)
    if repo_root_blockers:
        details["ok"] = False
        details["blockers"] = repo_root_blockers
        return details

    blockers: list[dict[str, Any]] = []
    manifest_path = repo_root / ABI7_FIXTURE_MANIFEST_PATH
    manifest, manifest_text, manifest_blockers = _load_release_json_data(
        manifest_path,
        "ABI-7 fixture manifest",
        missing_code="abi7_fixture_manifest_missing",
        shape_code="abi7_fixture_manifest_file_shape",
        unreadable_code="abi7_fixture_manifest_unreadable",
        invalid_code="abi7_fixture_manifest_invalid_json",
        not_object_code="abi7_fixture_manifest_not_object",
        max_bytes=MAX_ABI7_FIXTURE_JSON_BYTES,
    )
    blockers.extend(manifest_blockers)
    archive_path = repo_root / ABI7_ARCHIVE_FIXTURE_PATH
    archive_fixture, archive_text, archive_blockers = _load_release_json_data(
        archive_path,
        "ABI-7 archive fixture",
        missing_code="abi7_archive_fixture_missing",
        shape_code="abi7_archive_fixture_file_shape",
        unreadable_code="abi7_archive_fixture_unreadable",
        invalid_code="abi7_archive_fixture_invalid_json",
        not_object_code="abi7_archive_fixture_not_object",
        max_bytes=MAX_ABI7_FIXTURE_JSON_BYTES,
    )
    blockers.extend(archive_blockers)

    if manifest_text is not None:
        details["fixture_manifest_sha256"] = hashlib.sha256(
            manifest_text.encode("utf-8")
        ).hexdigest()
    if archive_text is not None:
        details["archive_fixture_sha256"] = hashlib.sha256(
            archive_text.encode("utf-8")
        ).hexdigest()

    expected_operations = list(ABI7_FIXTURE_OPERATIONS)
    if manifest is not None:
        details["fixture_manifest_schema"] = manifest.get("schema")
        details["native_bridge_abi_version"] = manifest.get("native_bridge_abi_version")
        details["operation_count"] = manifest.get("operation_count")
        _append_unexpected_json_field_blockers(
            blockers,
            manifest,
            ABI7_FIXTURE_MANIFEST_FIELDS,
            code="abi7_fixture_manifest_unexpected_field",
            message="ABI-7 fixture manifest contains an unexpected field",
        )
        archive_ref = manifest.get("archive_fixture")
        if manifest.get("schema") != ABI7_FIXTURE_MANIFEST_SCHEMA:
            blockers.append(
                blocker("abi7_fixture_manifest_schema", "ABI-7 fixture manifest schema mismatch")
            )
        if manifest.get("fixture_kind") != "native_bridge_norito_archives":
            blockers.append(
                blocker(
                    "abi7_fixture_manifest_kind",
                    "ABI-7 fixture manifest kind must be native_bridge_norito_archives",
                )
            )
        if not _is_expected_json_int(manifest.get("native_bridge_abi_version"), 7):
            blockers.append(
                blocker(
                    "abi7_fixture_manifest_bridge_version",
                    "ABI-7 fixture manifest must advertise bridge ABI 7",
                )
            )
        if not _is_expected_json_int(
            manifest.get("operation_count"),
            len(expected_operations),
        ):
            blockers.append(
                blocker(
                    "abi7_fixture_manifest_operation_count",
                    "ABI-7 fixture manifest operation count drifted",
                )
            )
        if not isinstance(archive_ref, dict):
            blockers.append(
                blocker(
                    "abi7_fixture_manifest_archive_fixture_shape",
                    "ABI-7 fixture manifest archive_fixture must be an object",
                )
            )
        else:
            _append_unexpected_json_field_blockers(
                blockers,
                archive_ref,
                ABI7_FIXTURE_ARCHIVE_REF_FIELDS,
                code="abi7_fixture_manifest_archive_fixture_unexpected_field",
                message="ABI-7 fixture manifest archive_fixture contains an unexpected field",
            )
            if archive_ref.get("path") != ABI7_ARCHIVE_FIXTURE_PATH:
                blockers.append(
                    blocker(
                        "abi7_fixture_manifest_archive_fixture",
                        "ABI-7 fixture manifest archive path drifted",
                    )
                )
            if archive_ref.get("schema") != ABI7_ARCHIVE_FIXTURE_SCHEMA:
                blockers.append(
                    blocker(
                        "abi7_fixture_manifest_archive_fixture",
                        "ABI-7 fixture manifest archive schema drifted",
                    )
                )
        generator = manifest.get("generator")
        if not isinstance(generator, dict):
            blockers.append(
                blocker(
                    "abi7_fixture_manifest_generator_shape",
                    "ABI-7 fixture manifest generator must be an object",
                )
            )
        else:
            _append_unexpected_json_field_blockers(
                blockers,
                generator,
                ABI7_FIXTURE_GENERATOR_FIELDS,
                code="abi7_fixture_manifest_generator_unexpected_field",
                message="ABI-7 fixture manifest generator contains an unexpected field",
            )
        if generator != ABI7_FIXTURE_GENERATOR:
            blockers.append(
                blocker(
                    "abi7_fixture_manifest_generator",
                    "ABI-7 fixture manifest generator provenance drifted",
                )
            )
        domains = manifest.get("domains")
        if not isinstance(domains, dict):
            blockers.append(
                blocker(
                    "abi7_fixture_manifest_domains_shape",
                    "ABI-7 fixture manifest domains must be an object",
                )
            )
        else:
            _append_unexpected_json_field_blockers(
                blockers,
                domains,
                ABI7_FIXTURE_DOMAINS_FIELDS,
                code="abi7_fixture_manifest_domains_unexpected_field",
                message="ABI-7 fixture manifest domains object contains an unexpected field",
            )
        if domains != ABI7_FIXTURE_DOMAINS:
            blockers.append(
                blocker(
                    "abi7_fixture_manifest_domains",
                    "ABI-7 fixture manifest domains drifted",
                )
            )
        operations = manifest.get("operations")
        if isinstance(operations, list):
            for operation_entry in operations:
                if isinstance(operation_entry, dict):
                    _append_unexpected_json_field_blockers(
                        blockers,
                        operation_entry,
                        ABI7_FIXTURE_OPERATION_FIELDS,
                        code="abi7_fixture_manifest_operation_unexpected_field",
                        message="ABI-7 fixture manifest operation contains an unexpected field",
                    )
                else:
                    blockers.append(
                        blocker(
                            "abi7_fixture_manifest_operation_shape",
                            "ABI-7 fixture manifest operations must be objects",
                        )
                    )
        else:
            blockers.append(
                blocker(
                    "abi7_fixture_manifest_operations_shape",
                    "ABI-7 fixture manifest operations must be an array",
                )
            )
        if manifest.get("operations") != expected_operations:
            blockers.append(
                blocker(
                    "abi7_fixture_manifest_operations",
                    "ABI-7 fixture manifest operation inventory drifted",
                )
            )

    if archive_fixture is not None:
        details["archive_fixture_schema"] = archive_fixture.get("schema")
        _append_unexpected_json_field_blockers(
            blockers,
            archive_fixture,
            ABI7_ARCHIVE_FIXTURE_FIELDS,
            code="abi7_archive_fixture_unexpected_field",
            message="ABI-7 archive fixture contains an unexpected field",
        )
        if archive_fixture.get("schema") != ABI7_ARCHIVE_FIXTURE_SCHEMA:
            blockers.append(
                blocker("abi7_archive_fixture_schema", "ABI-7 archive fixture schema mismatch")
            )
        if archive_fixture.get("fixture_kind") != "native_bridge_norito_archives":
            blockers.append(
                blocker(
                    "abi7_archive_fixture_kind",
                    "ABI-7 archive fixture kind must be native_bridge_norito_archives",
                )
            )
        if not _is_expected_json_int(
            archive_fixture.get("native_bridge_abi_version"),
            7,
        ):
            blockers.append(
                blocker(
                    "abi7_archive_fixture_bridge_version",
                    "ABI-7 archive fixture must advertise bridge ABI 7",
                )
            )
        archives = archive_fixture.get("archives")
        if not isinstance(archives, list):
            blockers.append(
                blocker(
                    "abi7_archive_fixture_archives",
                    "ABI-7 archive fixture archives must be an array",
                )
            )
            archives = []
        if len(archives) != len(expected_operations):
            blockers.append(
                blocker(
                    "abi7_archive_fixture_operation_count",
                    "ABI-7 archive fixture operation count drifted",
                )
            )
        archive_by_name: dict[str, Any] = {}
        for entry in archives:
            if isinstance(entry, dict) and isinstance(entry.get("name"), str):
                name = entry["name"]
                if name in archive_by_name:
                    blockers.append(
                        blocker(
                            "abi7_archive_fixture_duplicate_archive",
                            "ABI-7 archive fixture archive names must be unique",
                        )
                    )
                    continue
                archive_by_name[name] = entry
            else:
                blockers.append(
                    blocker(
                        "abi7_archive_fixture_archive_shape",
                        "ABI-7 archive fixture entries must be objects with names",
                    )
                )
        if tuple(archive_by_name) != _abi7_fixture_operation_names():
            blockers.append(
                blocker(
                    "abi7_archive_fixture_operations",
                    "ABI-7 archive fixture operation names drifted",
                )
            )
        for expected in expected_operations:
            name = expected["name"]
            entry = archive_by_name.get(name)
            if not isinstance(entry, dict):
                blockers.append(
                    blocker(
                        "abi7_archive_fixture_missing_archive",
                        "ABI-7 archive fixture is missing an expected archive",
                        archive=name,
                    )
                )
                continue
            _append_unexpected_json_field_blockers(
                blockers,
                entry,
                ABI7_ARCHIVE_FIXTURE_ENTRY_FIELDS,
                code="abi7_archive_fixture_archive_unexpected_field",
                message="ABI-7 archive fixture entry contains an unexpected field",
                archive=name,
            )
            for field in ("operation", "norito_type"):
                if entry.get(field) != expected[field]:
                    blockers.append(
                        blocker(
                            "abi7_archive_fixture_archive_metadata",
                            "ABI-7 archive fixture metadata drifted",
                            archive=name,
                            field=field,
                        )
                    )
            byte_len = entry.get("byte_len")
            sha256_hex = entry.get("sha256_hex")
            decoded, decode_blockers = _decode_abi7_fixture_archive_bytes(
                entry.get("bytes_base64"),
                name,
            )
            blockers.extend(decode_blockers)
            if isinstance(byte_len, bool) or not isinstance(byte_len, int) or byte_len <= 0:
                blockers.append(
                    blocker(
                        "abi7_archive_fixture_byte_len",
                        "ABI-7 archive fixture byte_len must be a positive integer",
                        archive=name,
                    )
                )
            elif decoded is not None and byte_len != len(decoded):
                blockers.append(
                    blocker(
                        "abi7_archive_fixture_byte_len",
                        "ABI-7 archive fixture byte_len does not match decoded bytes",
                        archive=name,
                    )
                )
            if (
                not isinstance(sha256_hex, str)
                or not re.fullmatch(r"[0-9a-f]{64}", sha256_hex)
                or sha256_hex == "0" * 64
            ):
                blockers.append(
                    blocker(
                        "abi7_archive_fixture_sha256",
                        "ABI-7 archive fixture sha256_hex must be a non-zero lowercase SHA-256 digest",
                        archive=name,
                    )
                )
            elif decoded is not None and hashlib.sha256(decoded).hexdigest() != sha256_hex:
                blockers.append(
                    blocker(
                        "abi7_archive_fixture_sha256",
                        "ABI-7 archive fixture sha256_hex does not match decoded bytes",
                        archive=name,
                    )
                )

    details["ok"] = not blockers
    details["blockers"] = blockers
    return details


def _is_lower_sha256_hex(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and value == value.lower()
        and value != "0" * 64
        and all(character in "0123456789abcdef" for character in value)
    )


def _sha256_file(path: Path, label: str) -> tuple[str | None, list[str]]:
    expected_stat, file_errors = _validate_lineage_local_file_for_read(path, label)
    if file_errors:
        return None, file_errors
    digest = hashlib.sha256()
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
            open_identity = (open_stat.st_dev, open_stat.st_ino)
            if stat.S_ISLNK(path_stat.st_mode):
                return None, [f"{label} must not be a symlink"]
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(open_stat.st_mode):
                return None, [f"{label} must be a regular file"]
            if open_identity != expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != expected_identity:
                return None, [f"{label} changed while being read"]
            if open_stat.st_nlink > 1:
                return None, [f"{label} must not be hardlinked"]
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
            final_path_stat = path.lstat()
            if (final_path_stat.st_dev, final_path_stat.st_ino) != expected_identity:
                return None, [f"{label} changed while being read"]
    except OSError:
        return None, [f"{label} could not be read"]
    return digest.hexdigest(), []


def _sha256_file_with_size(
    path: Path,
    label: str,
    *,
    allow_empty: bool = False,
) -> tuple[str | None, int | None, list[str]]:
    digest, size, _prefix, errors = _sha256_file_with_size_and_prefix(
        path,
        label,
        allow_empty=allow_empty,
    )
    return digest, size, errors


def _sha256_file_with_size_and_prefix(
    path: Path,
    label: str,
    *,
    allow_empty: bool = False,
    prefix_len: int = 4096,
) -> tuple[str | None, int | None, bytes | None, list[str]]:
    expected_stat, file_errors = _validate_lineage_local_file_for_read(path, label)
    if file_errors:
        return None, None, None, file_errors
    digest = hashlib.sha256()
    prefix_parts: list[bytes] = []
    prefix_remaining = prefix_len
    size = 0
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
            open_identity = (open_stat.st_dev, open_stat.st_ino)
            if stat.S_ISLNK(path_stat.st_mode):
                return None, None, None, [f"{label} must not be a symlink"]
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(open_stat.st_mode):
                return None, None, None, [f"{label} must be a regular file"]
            if open_identity != expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != expected_identity:
                return None, None, None, [f"{label} changed while being read"]
            if open_stat.st_nlink > 1:
                return None, None, None, [f"{label} must not be hardlinked"]
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if prefix_remaining > 0:
                    prefix_parts.append(chunk[:prefix_remaining])
                    prefix_remaining -= min(prefix_remaining, len(chunk))
                digest.update(chunk)
            final_path_stat = path.lstat()
            if (final_path_stat.st_dev, final_path_stat.st_ino) != expected_identity:
                return None, None, None, [f"{label} changed while being read"]
    except OSError:
        return None, None, None, [f"{label} could not be read"]
    if size <= 0 and not allow_empty:
        return None, None, None, [f"{label} must be non-empty"]
    return digest.hexdigest(), size, b"".join(prefix_parts), []


def _validate_lineage_local_file_for_read(
    path: Path,
    label: str,
) -> tuple[os.stat_result | None, list[str]]:
    """Reject local lineage evidence files that could alias external bytes."""

    path_text = str(path)
    if device_lab.SECRET_RE.search(path_text):
        return None, [f"{label} path must not contain secret-looking material"]
    if device_lab._contains_control_character(path_text):
        return None, [f"{label} path must not contain control characters"]
    if path_text != path_text.strip() or device_lab._path_has_surrounding_whitespace_component(  # type: ignore[attr-defined]
        path
    ):
        return None, [f"{label} path must not contain surrounding whitespace"]
    if "\\" in path_text:
        return None, [f"{label} path must not contain backslashes"]
    if ".." in path.parts:
        return None, [f"{label} path must be canonical"]
    ancestor_errors = device_lab.validate_no_symlink_ancestors(
        path,
        f"{label} ancestor directory",
    )
    if ancestor_errors:
        return None, ancestor_errors
    try:
        file_stat = path.lstat()
    except FileNotFoundError:
        return None, [f"{label} is missing"]
    except OSError:
        return None, [f"{label} file metadata could not be read"]
    if stat.S_ISLNK(file_stat.st_mode):
        return None, [f"{label} must not be a symlink"]
    if not stat.S_ISREG(file_stat.st_mode):
        return None, [f"{label} must be a regular file"]
    try:
        link_count = path.stat().st_nlink
    except OSError:
        return None, [f"{label} hardlink metadata could not be read"]
    if link_count > 1:
        return None, [f"{label} must not be hardlinked"]
    return file_stat, []


def validate_lineage_local_file(path: Path, label: str) -> list[str]:
    """Reject local lineage evidence files that could alias external bytes."""

    _file_stat, errors = _validate_lineage_local_file_for_read(path, label)
    return errors


def _lineage_local_text(
    path: Path,
    label: str,
    unreadable_error: str,
    *,
    decode_errors: str = "strict",
) -> tuple[str | None, list[str]]:
    """Validate a local lineage file immediately before reading text."""

    _digest, text, errors = _sha256_text_file(
        path,
        label,
        unreadable_error,
        decode_errors=decode_errors,
    )
    return text, errors


def _sha256_text_file(
    path: Path,
    label: str,
    unreadable_error: str,
    *,
    max_bytes: int | None = None,
    too_large_error: str | None = None,
    decode_errors: str = "strict",
) -> tuple[str | None, str | None, list[str]]:
    """Return a digest and decoded text from one opened, path-bound file."""

    expected_stat, file_errors = _validate_lineage_local_file_for_read(path, label)
    if file_errors:
        return None, None, file_errors
    digest = hashlib.sha256()
    chunks: list[bytes] = []
    size = 0
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
            open_identity = (open_stat.st_dev, open_stat.st_ino)
            if stat.S_ISLNK(path_stat.st_mode):
                return None, None, [f"{label} must not be a symlink"]
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(open_stat.st_mode):
                return None, None, [f"{label} must be a regular file"]
            if open_identity != expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != expected_identity:
                return None, None, [f"{label} changed while being read"]
            if open_stat.st_nlink > 1:
                return None, None, [f"{label} must not be hardlinked"]
            if max_bytes is not None and open_stat.st_size > max_bytes:
                return None, None, [
                    too_large_error
                    if too_large_error is not None
                    else f"{label} must be no more than {max_bytes} bytes"
                ]
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if max_bytes is not None and size > max_bytes:
                    return None, None, [
                        too_large_error
                        if too_large_error is not None
                        else f"{label} must be no more than {max_bytes} bytes"
                    ]
                chunks.append(chunk)
                digest.update(chunk)
            final_path_stat = path.lstat()
            if (final_path_stat.st_dev, final_path_stat.st_ino) != expected_identity:
                return None, None, [f"{label} changed while being read"]
    except OSError:
        return None, None, [unreadable_error]
    try:
        text = b"".join(chunks).decode("utf-8", errors=decode_errors)
    except UnicodeDecodeError:
        return None, None, [unreadable_error]
    return digest.hexdigest(), text, []


def validate_lineage_proof_log(path: Path, expected_name: str) -> tuple[str | None, list[str]]:
    """Return the SHA-256 and content errors for a captured Reserved-lineage proof log."""

    file_errors = validate_lineage_local_file(path, "production proof log")
    if file_errors:
        if file_errors == ["production proof log is missing"]:
            return None, ["missing production proof log"]
        return None, file_errors

    size_error = f"production proof log must be no more than {MAX_LINEAGE_PROOF_LOG_BYTES} bytes"
    digest, text, read_errors = _sha256_text_file(
        path,
        "production proof log",
        "production proof log could not be read",
        max_bytes=MAX_LINEAGE_PROOF_LOG_BYTES,
        too_large_error=size_error,
    )
    if read_errors:
        return None, read_errors
    assert digest is not None and text is not None
    errors: list[str] = []
    if "\r" in text:
        errors.append("--proof-log must use canonical LF line endings")
    if not text.endswith("\n"):
        errors.append("--proof-log must end with a canonical LF line terminator")
    lines = text.splitlines()
    expected_test_line = f"test {expected_name} ... ok"
    test_lines = [
        line
        for line in lines
        if line.startswith("test ") and not line.startswith("test result:")
    ]
    has_expected_test_line = expected_test_line in test_lines
    if not has_expected_test_line:
        errors.append("--proof-log must contain the passing production proof test line")
    if test_lines != [expected_test_line]:
        errors.append("--proof-log must contain only the single production proof test line")

    result_lines = [line for line in lines if line.startswith("test result:")]
    has_expected_result_line = any(
        LINEAGE_PROOF_RESULT_RE.fullmatch(line) for line in result_lines
    )
    if not has_expected_result_line:
        errors.append("--proof-log must contain a passing cargo test result")
    if (
        len(result_lines) != 1
        or LINEAGE_PROOF_RESULT_RE.fullmatch(result_lines[0]) is None
    ):
        errors.append(
            "--proof-log must contain exactly one cargo test result for one passed production test"
        )
    if any(
        marker in text
        for marker in (
            "test result: FAILED",
            "FAILED",
            "\nfailures:",
            "panicked at",
            "error: test failed",
        )
    ):
        errors.append("--proof-log must not contain cargo failure markers")
    return digest, errors


def _rust_function_body(source: str, signature: str) -> str | None:
    """Return the Rust function body following `signature`, ignoring braces in strings."""

    start = source.find(signature)
    if start < 0:
        return None
    brace_start = source.find("{", start)
    if brace_start < 0:
        return None

    depth = 0
    index = brace_start
    in_line_comment = False
    in_block_comment = False
    in_string = False
    raw_string_hashes: int | None = None
    in_char = False
    escaped = False
    while index < len(source):
        char = source[index]
        next_char = source[index + 1] if index + 1 < len(source) else ""

        if in_line_comment:
            if char == "\n":
                in_line_comment = False
            index += 1
            continue
        if in_block_comment:
            if char == "*" and next_char == "/":
                in_block_comment = False
                index += 2
            else:
                index += 1
            continue
        if raw_string_hashes is not None:
            if char == '"' and source.startswith("#" * raw_string_hashes, index + 1):
                index += 1 + raw_string_hashes
                raw_string_hashes = None
            else:
                index += 1
            continue
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if in_char:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == "'":
                in_char = False
            index += 1
            continue

        if char == "/" and next_char == "/":
            in_line_comment = True
            index += 2
            continue
        if char == "/" and next_char == "*":
            in_block_comment = True
            index += 2
            continue
        raw_prefix_len = 0
        if char == "r":
            raw_prefix_len = 1
        elif char == "b" and next_char == "r":
            raw_prefix_len = 2
        if raw_prefix_len:
            raw_index = index + raw_prefix_len
            raw_hashes = 0
            while raw_index < len(source) and source[raw_index] == "#":
                raw_hashes += 1
                raw_index += 1
            if raw_index < len(source) and source[raw_index] == '"':
                raw_string_hashes = raw_hashes
                index = raw_index + 1
                continue
        if char == '"':
            in_string = True
            index += 1
            continue
        if char == "'" and not (next_char.isalpha() or next_char == "_"):
            in_char = True
            index += 1
            continue
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return source[brace_start : index + 1]
        index += 1
    return None


def _require_rust_function_contract(
    source: str, signature: str, snippets: Iterable[str]
) -> list[str]:
    """Return missing snippets from a Rust function contract."""

    body = _rust_function_body(source, signature)
    if body is None:
        return [signature]
    return [snippet for snippet in snippets if snippet not in body]


def validate_lineage_proof_command(command: Any, expected_name: str) -> list[str]:
    """Return validation errors for the production Reserved-lineage proof command."""

    if not isinstance(command, str) or not command:
        return ["--command must be a non-empty string"]
    errors: list[str] = []
    if command != command.strip():
        errors.append("--command must not contain surrounding whitespace")
    if device_lab._contains_control_character(command):
        errors.append("--command must not contain control characters")
    expected_command = expected_lineage_proof_command(expected_name)
    expected_tokens = (
        "cargo",
        "test",
        "-p",
        "iroha_core",
        expected_name,
        "--lib",
        "--",
        "--ignored",
        "--test-threads=1",
        "--nocapture",
    )
    try:
        tokens = tuple(shlex.split(command))
    except ValueError:
        tokens = ()
        errors.append("--command must be shell-tokenizable without quoting errors")
    if tokens != expected_tokens:
        errors.append(
            "--command must exactly match the production Reserved-lineage proof command"
        )
    if command != expected_command:
        errors.append(
            "--command must exactly match the canonical production Reserved-lineage proof command string"
        )
    if KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_RUNTIME_KEYGEN_ENV in command:
        errors.append(
            "--command must not set runtime lineage keygen for the production proof run"
        )
    if device_lab.SECRET_RE.search(command):
        errors.append("--command must not contain secret-looking material")
    return errors


def validate_compact_key_command(command: Any) -> list[str]:
    """Return validation errors for the ABI-7 recursive compact keygen command."""

    if not isinstance(command, str) or not command:
        return ["--command must be a non-empty string"]
    errors: list[str] = []
    if command != command.strip():
        errors.append("--command must not contain surrounding whitespace")
    if device_lab._contains_control_character(command):
        errors.append("--command must not contain control characters")
    expected_command = expected_compact_key_command()
    expected_tokens = (
        "iroha",
        "app",
        "zk",
        "kagemusha",
        "recursive-compact-key-artifacts",
        "--vk-out",
        "artifacts/kagemusha/recursive-compact-len4.vk",
        "--pk-out",
        "artifacts/kagemusha/recursive-compact-len4.pk",
        "--key-artifacts-out",
        "artifacts/kagemusha/recursive-compact-key-artifacts.norito",
        "--verifier-keys-out",
        "artifacts/kagemusha/recursive-compact-verifier-keys.norito",
        "--record-out",
        "artifacts/kagemusha/recursive-compact-len4.record.norito",
        "--record-namespace",
        EXPECTED_COMPACT_KEY_RECORD_NAMESPACE,
        "--record-version",
        str(EXPECTED_COMPACT_KEY_RECORD_VERSION),
    )
    try:
        tokens = tuple(shlex.split(command))
    except ValueError:
        tokens = ()
        errors.append("--command must be shell-tokenizable without quoting errors")
    if tokens != expected_tokens:
        errors.append(
            "--command must exactly match the production ABI-7 recursive compact keygen command"
        )
    if command != expected_command:
        errors.append(
            "--command must exactly match the canonical ABI-7 recursive compact keygen command string"
        )
    if device_lab.SECRET_RE.search(command):
        errors.append("--command must not contain secret-looking material")
    return errors


def validate_compact_key_artifact_prefix(prefix: bytes, artifact: str) -> list[str]:
    """Reject obvious development placeholders for ABI-7 compact key artifacts."""

    stripped = prefix.strip().lower()
    if prefix and all(byte == 0 for byte in prefix):
        return [
            (
                f"recursive compact key artifact {artifact} "
                f"{COMPACT_KEY_ALL_ZERO_ERROR}"
            )
        ]
    if any(stripped.startswith(marker) for marker in COMPACT_KEY_PLACEHOLDER_PREFIXES):
        return [
            (
                f"recursive compact key artifact {artifact} "
                f"{COMPACT_KEY_PLACEHOLDER_ERROR}"
            )
        ]
    return []


def validate_compact_key_artifact_content(path: Path, artifact: str) -> list[str]:
    """Reject obvious development placeholders for ABI-7 compact key artifacts."""

    _digest, _size, prefix, errors = _sha256_file_with_size_and_prefix(
        path,
        f"recursive compact key artifact {artifact}",
        allow_empty=True,
    )
    if errors:
        return errors
    assert prefix is not None
    return validate_compact_key_artifact_prefix(prefix, artifact)


def validate_lineage_artifact_prefix(prefix: bytes, artifact: str) -> list[str]:
    """Reject obvious development placeholders for Reserved-lineage artifacts."""

    if prefix and all(byte == 0 for byte in prefix):
        return [f"lineage artifact {artifact} {LINEAGE_ARTIFACT_ALL_ZERO_ERROR}"]
    return []


def validate_lineage_artifact_content(path: Path, artifact: str) -> list[str]:
    """Reject obvious development placeholders for Reserved-lineage artifacts."""

    _digest, _size, prefix, errors = _sha256_file_with_size_and_prefix(
        path,
        f"lineage artifact {artifact}",
        allow_empty=True,
    )
    if errors:
        return errors
    assert prefix is not None
    return validate_lineage_artifact_prefix(prefix, artifact)


def parse_compact_key_generator_log(
    text: str,
) -> tuple[dict[str, int], dict[str, str], list[str]]:
    """Parse the canonical ABI-7 recursive compact key generator summary log."""

    errors: list[str] = []
    if "\r" in text:
        errors.append("compact key generator log must use canonical LF line endings")
    if not text.endswith("\n"):
        errors.append("compact key generator log must end with a canonical LF line terminator")
    lines = text.splitlines()
    if len(lines) != 1:
        errors.append("compact key generator log must contain exactly one summary line")
    if errors:
        return {}, {}, errors
    line = lines[0]
    match = COMPACT_KEY_GENERATOR_LOG_RE.fullmatch(line)
    if match is None:
        return {}, {}, ["compact key generator log must match the canonical CLI summary"]
    sizes = {
        artifact: int(match.group(field))
        for artifact, field in COMPACT_KEY_GENERATOR_LOG_SIZE_FIELDS.items()
    }
    digests = {
        artifact: match.group(field)
        for artifact, field in COMPACT_KEY_GENERATOR_LOG_DIGEST_FIELDS.items()
    }
    if any(digest == "0" * 64 for digest in digests.values()):
        return (
            {},
            {},
            ["compact key generator log must contain non-zero SHA-256 artifact digests"],
        )
    return sizes, digests, []


def validate_compact_key_generator_log(
    path: Path,
    expected_sha256: Any,
    artifact_size_bytes: dict[str, int],
    artifact_sha256: dict[str, str],
) -> tuple[str | None, dict[str, int], dict[str, str], list[dict[str, Any]]]:
    """Validate the ABI-7 compact-key generator log against local artifacts."""

    blockers: list[dict[str, Any]] = []
    _require_compact_key_sha256(
        blockers,
        value=expected_sha256,
        field="generator_log_sha256",
        code="compact_key_evidence_generator_log_sha256",
    )
    file_errors = validate_lineage_local_file(
        path,
        "ABI-7 recursive compact key generator log",
    )
    if file_errors:
        for error in file_errors:
            blockers.append(
                blocker(
                    "compact_key_evidence_generator_log_file_shape",
                    error,
                )
            )
        return None, {}, {}, blockers
    size_error = (
        "ABI-7 recursive compact key generator log must be no more than "
        f"{MAX_COMPACT_KEY_GENERATOR_LOG_BYTES} bytes"
    )
    digest, text, read_errors = _sha256_text_file(
        path,
        "ABI-7 recursive compact key generator log",
        "ABI-7 recursive compact key generator log could not be read",
        max_bytes=MAX_COMPACT_KEY_GENERATOR_LOG_BYTES,
        too_large_error=size_error,
    )
    if read_errors:
        for error in read_errors:
            blockers.append(
                blocker(
                    (
                        "compact_key_evidence_generator_log_size"
                        if error == size_error
                        else "compact_key_evidence_generator_log_file_shape"
                    ),
                    error,
                )
            )
        return None, {}, {}, blockers
    assert digest is not None and text is not None
    if _is_lower_sha256_hex(expected_sha256) and digest != expected_sha256:
        blockers.append(
            blocker(
                "compact_key_evidence_generator_log_digest",
                "ABI-7 recursive compact key generator log digest does not match local bytes",
            )
        )
    parsed_sizes, parsed_digests, parse_errors = parse_compact_key_generator_log(text)
    for error in parse_errors:
        blockers.append(
            blocker(
                "compact_key_evidence_generator_log_format",
                error,
            )
        )
    for artifact, actual_size in artifact_size_bytes.items():
        logged_size = parsed_sizes.get(artifact)
        if logged_size is not None and logged_size != actual_size:
            blockers.append(
                blocker(
                    "compact_key_evidence_generator_log_artifact_size",
                    "ABI-7 recursive compact key generator log size does not match local artifact bytes",
                    artifact=artifact,
                )
            )
    for artifact, actual_digest in artifact_sha256.items():
        logged_digest = parsed_digests.get(artifact)
        if logged_digest is not None and logged_digest != actual_digest:
            blockers.append(
                blocker(
                    "compact_key_evidence_generator_log_artifact_digest",
                    "ABI-7 recursive compact key generator log digest does not match local artifact bytes",
                    artifact=artifact,
                )
            )
    return digest, parsed_sizes, parsed_digests, blockers


def _require_lineage_sha256(
    blockers: list[dict[str, Any]],
    *,
    value: Any,
    field: str,
    code: str,
) -> None:
    if not _is_lower_sha256_hex(value):
        blockers.append(
            blocker(
                code,
                f"lineage proof evidence {field} must be a non-zero lowercase sha256 hex digest",
                field=field,
            )
        )


def _require_lineage_artifact_size(
    blockers: list[dict[str, Any]],
    *,
    value: Any,
    artifact: str,
    actual_size: int | None = None,
) -> bool:
    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        blockers.append(
            blocker(
                "lineage_proof_evidence_artifact_size",
                "Reserved-lineage proof evidence artifact size must be a positive integer",
                artifact=artifact,
            )
        )
        return False
    if actual_size is not None and value != actual_size:
        blockers.append(
            blocker(
                "lineage_proof_evidence_artifact_size",
                "Reserved-lineage proof evidence artifact size does not match local artifact bytes",
                artifact=artifact,
            )
        )
        return False
    return True


def _require_compact_key_sha256(
    blockers: list[dict[str, Any]],
    *,
    value: Any,
    field: str,
    code: str,
) -> None:
    if not _is_lower_sha256_hex(value):
        blockers.append(
            blocker(
                code,
                f"recursive compact key evidence {field} must be a non-zero lowercase sha256 hex digest",
                field=field,
            )
        )


def _require_compact_key_artifact_size(
    blockers: list[dict[str, Any]],
    *,
    value: Any,
    artifact: str,
    actual_size: int | None = None,
) -> bool:
    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        blockers.append(
            blocker(
                "compact_key_evidence_artifact_size",
                "ABI-7 recursive compact key evidence artifact size must be a positive integer",
                artifact=artifact,
            )
        )
        return False
    if actual_size is not None and value != actual_size:
        blockers.append(
            blocker(
                "compact_key_evidence_artifact_size",
                "ABI-7 recursive compact key evidence artifact size does not match local artifact bytes",
                artifact=artifact,
            )
        )
        return False
    return True


def _display_evidence_field(field: str) -> str:
    if device_lab.SECRET_RE.search(field):
        return device_lab.SECRET_PATH_REDACTION
    if device_lab._contains_control_character(field):
        return EVIDENCE_CONTROL_STRING_REDACTION
    return field


def _display_evidence_value(value: Any) -> Any:
    if isinstance(value, str) and device_lab.SECRET_RE.search(value):
        return device_lab.SECRET_PATH_REDACTION
    if isinstance(value, str) and device_lab._contains_control_character(value):
        return EVIDENCE_CONTROL_STRING_REDACTION
    if isinstance(value, float) and not math.isfinite(value):
        return EVIDENCE_NONFINITE_NUMBER_REDACTION
    return value


NON_PRODUCTION_TEXT_MARKERS = frozenset(
    (
        "demo",
        "dev",
        "development",
        "devnet",
        "dummy",
        "example",
        "fake",
        "fixture",
        "mock",
        "placeholder",
        "preprod",
        "preproduction",
        "preview",
        "qa",
        "sample",
        "sandbox",
        "stage",
        "staging",
        "test",
        "testing",
        "testnet",
        "uat",
        "zero",
    )
)
NON_PRODUCTION_COMPACT_MARKERS = frozenset(
    (
        "demochain",
        "demolocalnet",
        "devfixture",
        "devlocalnet",
        "devnet",
        "devnetlocalnet",
        "devprooffixture",
        "developmentlocalnet",
        "examplechain",
        "examplelocalnet",
        "fakechain",
        "fakelocalnet",
        "dummylocalnet",
        "fixture",
        "fixturelocalnet",
        "localnetdemo",
        "localnetdev",
        "localnetdevnet",
        "localnetdevelopment",
        "localnetdummy",
        "localnetexample",
        "localnetfake",
        "localnetfixture",
        "localnetmock",
        "localnetplaceholder",
        "localnetpreprod",
        "localnetpreproduction",
        "localnetpreview",
        "localnetqa",
        "localnetsample",
        "localnetsandbox",
        "localnetstage",
        "localnetstaging",
        "localnettest",
        "localnettesting",
        "localnetuat",
        "localnetzero",
        "mock",
        "mocklocalnet",
        "placeholder",
        "placeholderlocalnet",
        "preprod",
        "preprodlocalnet",
        "preproduction",
        "preproductionlocalnet",
        "preview",
        "previewlocalnet",
        "qa",
        "qalocalnet",
        "samplechain",
        "samplelocalnet",
        "sandboxchain",
        "sandboxlocalnet",
        "stagelocalnet",
        "staginglocalnet",
        "testlocalnet",
        "testnet",
        "testinglocalnet",
        "uat",
        "zeroproduction",
        "zerochain",
        "zerolocalnet",
        "zeronet",
    )
)
NON_PRODUCTION_COMPACT_CONTEXT_MARKERS = frozenset(("prod", "production"))
NON_PRODUCTION_CONTEXTUAL_COMPACT_MARKERS = frozenset(
    f"{marker}{context}"
    for marker in NON_PRODUCTION_TEXT_MARKERS
    for context in NON_PRODUCTION_COMPACT_CONTEXT_MARKERS
) | frozenset(
    f"{context}{marker}"
    for context in NON_PRODUCTION_COMPACT_CONTEXT_MARKERS
    for marker in NON_PRODUCTION_TEXT_MARKERS
)
CONTRADICTORY_LOCALNET_TEXT_MARKERS = frozenset(("mainnet",))
CONTRADICTORY_LOCALNET_COMPACT_MARKERS = frozenset(
    (
        "localnetmainnet",
        "mainnetlocalnet",
    )
)
PRODUCTION_TEXT_MARKERS = frozenset(("prod", "production"))
LOCALNET_TEXT_MARKERS = frozenset(("localnet",))


def _evidence_text_token_sequence(value: str) -> tuple[str, ...]:
    return tuple(token for token in re.split(r"[^a-z0-9]+", value.lower()) if token)


def _evidence_text_tokens(value: str) -> frozenset[str]:
    return frozenset(_evidence_text_token_sequence(value))


def _evidence_text_compact_token_candidates(value: str) -> frozenset[str]:
    tokens = _evidence_text_token_sequence(value)
    return frozenset(tokens) | frozenset(
        f"{left}{right}" for left, right in zip(tokens, tokens[1:])
    )


def _evidence_token_has_non_production_prefix(token: str) -> bool:
    if token in NON_PRODUCTION_TEXT_MARKERS:
        return True
    return any(
        len(marker) >= 3 and token.startswith(marker)
        for marker in NON_PRODUCTION_TEXT_MARKERS
    )


def _evidence_text_has_joined_non_production_context(value: str) -> bool:
    tokens = _evidence_text_token_sequence(value)
    contexts = LOCALNET_TEXT_MARKERS | NON_PRODUCTION_COMPACT_CONTEXT_MARKERS
    for token in tokens:
        pending = [token]
        seen: set[str] = set()
        while pending:
            current = pending.pop()
            if current in seen:
                continue
            seen.add(current)
            for context in contexts:
                if current.startswith(context):
                    suffix = current[len(context) :]
                    if _evidence_token_has_non_production_prefix(suffix):
                        return True
                    if suffix:
                        pending.append(suffix)
                if current.endswith(context):
                    prefix = current[: -len(context)]
                    if _evidence_token_has_non_production_prefix(prefix):
                        return True
                    if prefix:
                        pending.append(prefix)
    for left, right in zip(tokens, tokens[1:]):
        if left in contexts and _evidence_token_has_non_production_prefix(right):
            return True
        if right in contexts and _evidence_token_has_non_production_prefix(left):
            return True
    return False


def _evidence_text_has_non_production_marker(value: str) -> bool:
    normalized = value.replace("-", "_").lower()
    tokens = _evidence_text_tokens(value)
    compact_candidates = _evidence_text_compact_token_candidates(value)
    return (
        not tokens.isdisjoint(NON_PRODUCTION_TEXT_MARKERS)
        or any(marker in normalized for marker in ("dev_fixture", "dev_proof_fixture"))
        or not compact_candidates.isdisjoint(NON_PRODUCTION_COMPACT_MARKERS)
        or not compact_candidates.isdisjoint(NON_PRODUCTION_CONTEXTUAL_COMPACT_MARKERS)
        or _evidence_text_has_joined_non_production_context(value)
    )


def _evidence_text_has_contradictory_localnet_marker(value: str) -> bool:
    tokens = _evidence_text_tokens(value)
    compact_candidates = _evidence_text_compact_token_candidates(value)
    return (
        not tokens.isdisjoint(CONTRADICTORY_LOCALNET_TEXT_MARKERS)
        or not compact_candidates.isdisjoint(CONTRADICTORY_LOCALNET_COMPACT_MARKERS)
    )


def _evidence_text_has_production_marker(value: str) -> bool:
    return not _evidence_text_tokens(value).isdisjoint(PRODUCTION_TEXT_MARKERS)


def _evidence_text_has_localnet_marker(value: str) -> bool:
    return not _evidence_text_tokens(value).isdisjoint(LOCALNET_TEXT_MARKERS)


def _localnet_public_text_is_valid(
    value: Any,
    *,
    limit: int,
    allowed_re: re.Pattern[str],
    require_production_marker: bool = True,
    require_localnet_marker: bool = True,
) -> bool:
    return (
        isinstance(value, str)
        and 0 < len(value) <= limit
        and not device_lab.SECRET_RE.search(value)
        and not device_lab._contains_control_character(value)
        and ".." not in value
        and not _evidence_text_has_non_production_marker(value)
        and not _evidence_text_has_contradictory_localnet_marker(value)
        and (
            not require_production_marker
            or _evidence_text_has_production_marker(value)
        )
        and (
            not require_localnet_marker
            or _evidence_text_has_localnet_marker(value)
        )
        and allowed_re.fullmatch(value) is not None
    )


LOCALNET_RUN_ID_RE = re.compile(r"[A-Za-z0-9_.:-]+")
LOCALNET_PEER_ID_RE = re.compile(r"[A-Za-z0-9_.:@-]+")
LOCALNET_CHAIN_ID_RE = re.compile(r"[A-Za-z0-9_.:-]+")


def _localnet_run_id_is_valid(value: Any) -> bool:
    if not _localnet_public_text_is_valid(
        value,
        limit=160,
        allowed_re=LOCALNET_RUN_ID_RE,
    ):
        return False
    compact = value.replace("_", "-").lower()
    return "4-peer" in compact or "4peer" in compact


def _localnet_peer_id_is_valid(value: Any) -> bool:
    return _localnet_public_text_is_valid(
        value,
        limit=160,
        allowed_re=LOCALNET_PEER_ID_RE,
    )


def _localnet_chain_id_is_valid(value: Any) -> bool:
    return _localnet_public_text_is_valid(
        value,
        limit=256,
        allowed_re=LOCALNET_CHAIN_ID_RE,
    )


def _localnet_hash_uri_digest(value: Any) -> str | None:
    if not isinstance(value, str) or not value:
        return None
    prefixes = ("sha256:", "urn:sha256:", "hash://sha256/")
    digest = None
    for prefix in prefixes:
        if value.startswith(prefix):
            digest = value[len(prefix):]
            break
    if digest is None:
        return None
    if (
        len(digest) != 64
        or digest != digest.lower()
        or not all(character in "0123456789abcdef" for character in digest)
        or digest == "0" * 64
        or len(set(digest)) == 1
    ):
        return None
    return digest


def check_localnet_lifecycle_evidence(
    path: Path,
    *,
    min_generated_at: dt.datetime | None = None,
    max_generated_at: dt.datetime | None = None,
    require_canonical_filename: bool = True,
) -> dict[str, Any]:
    """Check Kagemusha 4-peer localnet lifecycle evidence."""

    blockers: list[dict[str, Any]] = []
    if require_canonical_filename and path.name != LOCALNET_LIFECYCLE_EVIDENCE_FILENAME:
        blockers.append(
            blocker(
                "localnet_lifecycle_evidence_filename",
                (
                    "Kagemusha localnet lifecycle evidence file must be named "
                    f"{LOCALNET_LIFECYCLE_EVIDENCE_FILENAME}"
                ),
                expected=LOCALNET_LIFECYCLE_EVIDENCE_FILENAME,
            )
        )
        return {
            "path": LOCALNET_LIFECYCLE_EVIDENCE_SUMMARY_LABEL,
            "schema": None,
            "artifact_sha256": {},
            "min_generated_at_utc": (
                min_generated_at.isoformat().replace("+00:00", "Z")
                if min_generated_at is not None
                else None
            ),
            "max_generated_at_utc": (
                max_generated_at.isoformat().replace("+00:00", "Z")
                if max_generated_at is not None
                else None
            ),
            "ok": False,
            "blockers": blockers,
        }
    evidence_file_errors = [
        *device_lab.validate_no_symlink_ancestors(
            path,
            "Kagemusha localnet lifecycle evidence ancestor directory",
        )
    ]
    for error in validate_lineage_local_file(
        path,
        "Kagemusha localnet lifecycle evidence file",
    ):
        if error != "Kagemusha localnet lifecycle evidence file is missing":
            evidence_file_errors.append(error)
    if evidence_file_errors:
        for error in evidence_file_errors:
            blockers.append(blocker("localnet_lifecycle_evidence_file_shape", error))
        return {
            "path": LOCALNET_LIFECYCLE_EVIDENCE_SUMMARY_LABEL,
            "schema": None,
            "artifact_sha256": {},
            "min_generated_at_utc": (
                min_generated_at.isoformat().replace("+00:00", "Z")
                if min_generated_at is not None
                else None
            ),
            "max_generated_at_utc": (
                max_generated_at.isoformat().replace("+00:00", "Z")
                if max_generated_at is not None
                else None
            ),
            "ok": False,
            "blockers": blockers,
        }
    evidence, load_blockers = _load_json_artifact(
        path,
        missing_code="localnet_lifecycle_evidence_missing",
        invalid_code="localnet_lifecycle_evidence_invalid_json",
        unreadable_code="localnet_lifecycle_evidence_unreadable",
        shape_code="localnet_lifecycle_evidence_file_shape",
        not_object_code="localnet_lifecycle_evidence_not_object",
        label="Kagemusha localnet lifecycle evidence",
        max_bytes=MAX_LOCALNET_LIFECYCLE_EVIDENCE_JSON_BYTES,
    )
    blockers.extend(load_blockers)
    details: dict[str, Any] = {
        "path": LOCALNET_LIFECYCLE_EVIDENCE_SUMMARY_LABEL,
        "schema": None,
        "artifact_sha256": {},
        "min_generated_at_utc": (
            min_generated_at.isoformat().replace("+00:00", "Z")
            if min_generated_at is not None
            else None
        ),
        "max_generated_at_utc": (
            max_generated_at.isoformat().replace("+00:00", "Z")
            if max_generated_at is not None
            else None
        ),
    }
    if evidence is None:
        details["ok"] = False
        details["blockers"] = blockers
        return details

    for field in sorted(set(evidence) - LOCALNET_LIFECYCLE_EVIDENCE_FIELDS):
        blockers.append(
            blocker(
                "localnet_lifecycle_evidence_unexpected_field",
                "Kagemusha localnet lifecycle evidence contains unexpected field",
                field=_display_evidence_field(field),
            )
        )

    details["schema"] = _display_evidence_value(evidence.get("schema"))
    details["generated_at_utc"] = None
    if evidence.get("schema") != LOCALNET_LIFECYCLE_EVIDENCE_SCHEMA:
        blockers.append(
            blocker(
                "localnet_lifecycle_evidence_schema",
                "Kagemusha localnet lifecycle evidence schema mismatch",
            )
        )

    generated_at_text = evidence.get("generated_at_utc")
    if not isinstance(generated_at_text, str) or not generated_at_text.strip():
        blockers.append(
            blocker(
                "localnet_lifecycle_evidence_timestamp_missing",
                "Kagemusha localnet lifecycle evidence generated_at_utc is required",
            )
        )
    else:
        generated_at_raw = generated_at_text
        details["generated_at_utc"] = _display_evidence_value(generated_at_raw)
        skip_timestamp_parse = _append_evidence_timestamp_string_blockers(
            blockers,
            code_prefix="localnet_lifecycle_evidence",
            label="Kagemusha localnet lifecycle evidence",
            raw=generated_at_raw,
        )
        if (
            not skip_timestamp_parse
            and device_lab.SIGNED_AT_UTC_RE.fullmatch(generated_at_raw) is None
        ):
            blockers.append(
                blocker(
                    "localnet_lifecycle_evidence_timestamp_noncanonical",
                    "Kagemusha localnet lifecycle evidence generated_at_utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
                    generated_at_utc=_display_evidence_value(generated_at_raw),
                )
            )
        if not skip_timestamp_parse:
            generated_at, parse_blocker = parse_utc_timestamp(
                generated_at_raw,
                "Kagemusha localnet lifecycle evidence generated_at_utc",
            )
            if parse_blocker is not None:
                parse_blocker["code"] = "localnet_lifecycle_evidence_timestamp_invalid"
                blockers.append(parse_blocker)
            elif min_generated_at is not None and generated_at is not None and generated_at < min_generated_at:
                blockers.append(
                    blocker(
                        "localnet_lifecycle_evidence_stale",
                        "Kagemusha localnet lifecycle evidence predates the required release evidence cutoff",
                        generated_at_utc=_display_evidence_value(generated_at_raw),
                        min_generated_at_utc=min_generated_at.isoformat().replace("+00:00", "Z"),
                    )
                )
            elif max_generated_at is not None and generated_at is not None and generated_at > max_generated_at:
                blockers.append(
                    blocker(
                        "localnet_lifecycle_evidence_future_dated",
                        "Kagemusha localnet lifecycle evidence is ahead of the release validator clock skew",
                        generated_at_utc=_display_evidence_value(generated_at_raw),
                        max_generated_at_utc=max_generated_at.isoformat().replace("+00:00", "Z"),
                    )
                )

    localnet_run_id = evidence.get("localnet_run_id")
    chain_id = evidence.get("chain_id")
    if not _localnet_run_id_is_valid(localnet_run_id):
        blockers.append(
            blocker(
                "localnet_lifecycle_evidence_run_id",
                "Kagemusha localnet lifecycle evidence localnet_run_id must identify a production 4-peer run",
            )
        )
    if not _localnet_chain_id_is_valid(chain_id):
        blockers.append(
            blocker(
                "localnet_lifecycle_evidence_chain_id",
                "Kagemusha localnet lifecycle evidence chain_id must be a production chain id",
            )
        )
    details["localnet_run_id"] = _display_evidence_value(localnet_run_id)
    details["chain_id"] = _display_evidence_value(chain_id)

    acceptance = evidence.get("localnet_acceptance")
    normalized_hashes: dict[str, str] = {}
    if not isinstance(acceptance, dict):
        blockers.append(
            blocker(
                "localnet_lifecycle_evidence_acceptance",
                "Kagemusha localnet lifecycle evidence localnet_acceptance must be an object",
            )
        )
    else:
        for field in sorted(set(acceptance) - LOCALNET_LIFECYCLE_ACCEPTANCE_FIELDS):
            blockers.append(
                blocker(
                    "localnet_lifecycle_evidence_acceptance_unexpected_field",
                    "Kagemusha localnet lifecycle evidence localnet_acceptance contains unexpected field",
                    field=_display_evidence_field(field),
                )
            )
        for field in sorted(LOCALNET_LIFECYCLE_ACCEPTANCE_FIELDS - set(acceptance)):
            blockers.append(
                blocker(
                    "localnet_lifecycle_evidence_acceptance_missing_field",
                    "Kagemusha localnet lifecycle evidence localnet_acceptance is missing a required field",
                    field=field,
                )
            )
        if acceptance.get("run_id") != localnet_run_id:
            blockers.append(
                blocker(
                    "localnet_lifecycle_evidence_run_id_binding",
                    "Kagemusha localnet lifecycle evidence localnet_acceptance.run_id must match localnet_run_id",
                )
            )
        if acceptance.get("chain_id") != chain_id:
            blockers.append(
                blocker(
                    "localnet_lifecycle_evidence_chain_id_binding",
                    "Kagemusha localnet lifecycle evidence localnet_acceptance.chain_id must match chain_id",
                )
            )
        if acceptance.get("target") != EXPECTED_LOCALNET_TARGET:
            blockers.append(
                blocker(
                    "localnet_lifecycle_evidence_target",
                    "Kagemusha localnet lifecycle evidence target must be localnet",
                )
            )
        if acceptance.get("peer_count") != EXPECTED_LOCALNET_PEER_COUNT:
            blockers.append(
                blocker(
                    "localnet_lifecycle_evidence_peer_count",
                    "Kagemusha localnet lifecycle evidence peer_count must be 4",
                )
            )
        peer_ids = acceptance.get("peer_ids")
        if (
            not isinstance(peer_ids, list)
            or len(peer_ids) != EXPECTED_LOCALNET_PEER_COUNT
            or any(not _localnet_peer_id_is_valid(peer_id) for peer_id in peer_ids)
            or len(set(peer_ids)) != EXPECTED_LOCALNET_PEER_COUNT
            or peer_ids != sorted(peer_ids)
        ):
            blockers.append(
                blocker(
                    "localnet_lifecycle_evidence_peer_ids",
                    "Kagemusha localnet lifecycle evidence peer_ids must contain four distinct sorted production peer ids",
                )
            )
        for field in LOCALNET_LIFECYCLE_TRUE_FIELDS:
            if acceptance.get(field) is not True:
                blockers.append(
                    blocker(
                        "localnet_lifecycle_evidence_flag",
                        "Kagemusha localnet lifecycle evidence required lifecycle flag must be true",
                        field=field,
                    )
                )
        seen_digests: dict[str, str] = {}
        for field in LOCALNET_LIFECYCLE_HASH_FIELDS:
            digest = _localnet_hash_uri_digest(acceptance.get(field))
            if digest is None:
                blockers.append(
                    blocker(
                        "localnet_lifecycle_evidence_artifact_hash",
                        "Kagemusha localnet lifecycle evidence hash must be a non-placeholder SHA-256 URI",
                        field=field,
                    )
                )
                continue
            if digest in seen_digests:
                blockers.append(
                    blocker(
                        "localnet_lifecycle_evidence_artifact_hash_distinct",
                        "Kagemusha localnet lifecycle evidence artifact hashes must be distinct after URI normalization",
                        field=field,
                        first_field=seen_digests[digest],
                    )
                )
                continue
            seen_digests[digest] = field
            normalized_hashes[field] = digest

    details["target"] = (
        _display_evidence_value(acceptance.get("target"))
        if isinstance(acceptance, dict)
        else None
    )
    details["peer_count"] = (
        acceptance.get("peer_count") if isinstance(acceptance, dict) else None
    )
    details["peer_ids"] = (
        [_display_evidence_value(peer_id) for peer_id in acceptance.get("peer_ids", [])]
        if isinstance(acceptance, dict) and isinstance(acceptance.get("peer_ids"), list)
        else []
    )
    details["artifact_sha256"] = normalized_hashes
    details["artifact_count"] = len(normalized_hashes)
    details["ok"] = not blockers
    details["state"] = "localnet_lifecycle_validated" if not blockers else "blocked"
    details["blockers"] = blockers
    return details


def check_lineage_proof_evidence(
    path: Path,
    *,
    min_generated_at: dt.datetime | None = None,
    max_generated_at: dt.datetime | None = None,
    require_canonical_filename: bool = True,
) -> dict[str, Any]:
    """Check production-width Reserved-lineage proof/keygen evidence."""

    blockers: list[dict[str, Any]] = []
    if require_canonical_filename and path.name != LINEAGE_PROOF_EVIDENCE_FILENAME:
        blockers.append(
            blocker(
                "lineage_proof_evidence_filename",
                (
                    "Reserved-lineage proof evidence file must be named "
                    f"{LINEAGE_PROOF_EVIDENCE_FILENAME}"
                ),
                expected=LINEAGE_PROOF_EVIDENCE_FILENAME,
            )
        )
        return {
            "path": LINEAGE_PROOF_EVIDENCE_SUMMARY_LABEL,
            "schema": None,
            "artifact_sha256": {},
            "artifact_size_bytes": {},
            "test_log_sha256": {},
            "min_generated_at_utc": (
                min_generated_at.isoformat().replace("+00:00", "Z")
                if min_generated_at is not None
                else None
            ),
            "max_generated_at_utc": (
                max_generated_at.isoformat().replace("+00:00", "Z")
                if max_generated_at is not None
                else None
            ),
            "ok": False,
            "blockers": blockers,
        }
    evidence_file_errors = [
        *device_lab.validate_no_symlink_ancestors(
            path,
            "Reserved-lineage proof evidence ancestor directory",
        )
    ]
    for error in validate_lineage_local_file(
        path,
        "Reserved-lineage proof evidence file",
    ):
        if error != "Reserved-lineage proof evidence file is missing":
            evidence_file_errors.append(error)
    if evidence_file_errors:
        for error in evidence_file_errors:
            blockers.append(blocker("lineage_proof_evidence_file_shape", error))
        return {
            "path": LINEAGE_PROOF_EVIDENCE_SUMMARY_LABEL,
            "schema": None,
            "artifact_sha256": {},
            "artifact_size_bytes": {},
            "test_log_sha256": {},
            "min_generated_at_utc": (
                min_generated_at.isoformat().replace("+00:00", "Z")
                if min_generated_at is not None
                else None
            ),
            "max_generated_at_utc": (
                max_generated_at.isoformat().replace("+00:00", "Z")
                if max_generated_at is not None
                else None
            ),
            "ok": False,
            "blockers": blockers,
        }
    evidence, load_blockers = _load_json_artifact(
        path,
        missing_code="lineage_proof_evidence_missing",
        invalid_code="lineage_proof_evidence_invalid_json",
        unreadable_code="lineage_proof_evidence_unreadable",
        shape_code="lineage_proof_evidence_file_shape",
        not_object_code="lineage_proof_evidence_not_object",
        label="Reserved-lineage proof evidence",
        max_bytes=MAX_LINEAGE_PROOF_EVIDENCE_JSON_BYTES,
    )
    blockers.extend(load_blockers)
    details: dict[str, Any] = {
        "path": LINEAGE_PROOF_EVIDENCE_SUMMARY_LABEL,
        "schema": None,
        "artifact_sha256": {},
        "artifact_size_bytes": {},
        "test_log_sha256": {},
        "min_generated_at_utc": (
            min_generated_at.isoformat().replace("+00:00", "Z")
            if min_generated_at is not None
            else None
        ),
        "max_generated_at_utc": (
            max_generated_at.isoformat().replace("+00:00", "Z")
            if max_generated_at is not None
            else None
        ),
    }
    if evidence is None:
        details["ok"] = False
        details["blockers"] = blockers
        return details

    for field in sorted(set(evidence) - LINEAGE_PROOF_EVIDENCE_FIELDS):
        blockers.append(
            blocker(
                "lineage_proof_evidence_unexpected_field",
                "Reserved-lineage proof evidence contains unexpected field",
                field=_display_evidence_field(field),
            )
        )

    details["schema"] = _display_evidence_value(evidence.get("schema"))
    details["generated_at_utc"] = None
    if evidence.get("schema") != LINEAGE_PROOF_EVIDENCE_SCHEMA:
        blockers.append(
            blocker(
                "lineage_proof_evidence_schema",
                "Reserved-lineage proof evidence schema mismatch",
            )
        )

    generated_at_text = evidence.get("generated_at_utc")
    if not isinstance(generated_at_text, str) or not generated_at_text.strip():
        blockers.append(
            blocker(
                "lineage_proof_evidence_timestamp_missing",
                "Reserved-lineage proof evidence generated_at_utc is required",
            )
        )
    else:
        generated_at_raw = generated_at_text
        details["generated_at_utc"] = _display_evidence_value(generated_at_raw)
        skip_timestamp_parse = _append_evidence_timestamp_string_blockers(
            blockers,
            code_prefix="lineage_proof_evidence",
            label="Reserved-lineage proof evidence",
            raw=generated_at_raw,
        )
        if (
            not skip_timestamp_parse
            and device_lab.SIGNED_AT_UTC_RE.fullmatch(generated_at_raw) is None
        ):
            blockers.append(
                blocker(
                    "lineage_proof_evidence_timestamp_noncanonical",
                    "Reserved-lineage proof evidence generated_at_utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
                    generated_at_utc=_display_evidence_value(generated_at_raw),
                )
            )
        if not skip_timestamp_parse:
            generated_at, parse_blocker = parse_utc_timestamp(
                generated_at_raw,
                "Reserved-lineage proof evidence generated_at_utc",
            )
            if parse_blocker is not None:
                parse_blocker["code"] = "lineage_proof_evidence_timestamp_invalid"
                blockers.append(parse_blocker)
            elif min_generated_at is not None and generated_at is not None and generated_at < min_generated_at:
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_stale",
                        "Reserved-lineage proof evidence predates the required release evidence cutoff",
                        generated_at_utc=_display_evidence_value(generated_at_raw),
                        min_generated_at_utc=min_generated_at.isoformat().replace("+00:00", "Z"),
                    )
                )
            elif max_generated_at is not None and generated_at is not None and generated_at > max_generated_at:
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_future_dated",
                        "Reserved-lineage proof evidence is ahead of the release validator clock skew",
                        generated_at_utc=_display_evidence_value(generated_at_raw),
                        max_generated_at_utc=max_generated_at.isoformat().replace("+00:00", "Z"),
                    )
                )

    expected_scalars = {
        "opening_len": EXPECTED_LINEAGE_PROOF_OPENING_LEN,
        "ipa_k": EXPECTED_LINEAGE_PROOF_IPA_K,
    }
    for field, expected in expected_scalars.items():
        scalar_value = evidence.get(field)
        if (
            not isinstance(scalar_value, int)
            or isinstance(scalar_value, bool)
            or scalar_value != expected
        ):
            blockers.append(
                blocker(
                    f"lineage_proof_evidence_{field}",
                    f"Reserved-lineage proof evidence {field} must be integer {expected}",
                    field=field,
                )
            )
    details["opening_len"] = evidence.get("opening_len")
    details["ipa_k"] = evidence.get("ipa_k")

    for field, expected in {
        "verifier_backend": EXPECTED_LINEAGE_PROOF_BACKEND,
        "verifier_witness_profile": EXPECTED_LINEAGE_VERIFIER_WITNESS_PROFILE,
        "record_archive_proof_runtime_keygen_env": "unset",
    }.items():
        if evidence.get(field) != expected:
            blockers.append(
                blocker(
                    f"lineage_proof_evidence_{field}",
                    f"Reserved-lineage proof evidence {field} mismatch",
                    field=field,
                    expected=expected,
                )
            )
    details["record_archive_proof_runtime_keygen_env"] = _display_evidence_value(
        evidence.get("record_archive_proof_runtime_keygen_env")
    )

    circuit_ids = evidence.get("circuit_ids")
    if not isinstance(circuit_ids, dict):
        blockers.append(
            blocker(
                "lineage_proof_evidence_circuit_ids",
                "Reserved-lineage proof evidence circuit_ids must be an object",
            )
        )
    else:
        for key in sorted(set(circuit_ids) - set(EXPECTED_LINEAGE_CIRCUIT_IDS)):
            blockers.append(
                blocker(
                    "lineage_proof_evidence_circuit_ids_unexpected_field",
                    "Reserved-lineage proof evidence circuit_ids contains unexpected field",
                    field=_display_evidence_field(key),
                )
            )
        details["circuit_ids"] = {
            key: _display_evidence_value(circuit_ids.get(key))
            for key in sorted(EXPECTED_LINEAGE_CIRCUIT_IDS)
        }
        for key, expected in EXPECTED_LINEAGE_CIRCUIT_IDS.items():
            if circuit_ids.get(key) != expected:
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_circuit_id",
                        f"Reserved-lineage proof evidence circuit id {key} mismatch",
                        field=f"circuit_ids.{key}",
                        expected=expected,
                    )
                )

    artifacts = evidence.get("artifacts")
    artifact_sizes = evidence.get("artifact_size_bytes")
    artifact_count = 0
    validated_artifact_sha256: dict[str, str] = {}
    validated_artifact_sizes: dict[str, int] = {}
    artifact_sizes_valid = isinstance(artifact_sizes, dict)
    if not artifact_sizes_valid:
        blockers.append(
            blocker(
                "lineage_proof_evidence_artifact_sizes",
                "Reserved-lineage proof evidence artifact_size_bytes must be an object",
            )
        )
    else:
        for artifact in sorted(set(artifact_sizes) - set(LINEAGE_PROOF_REQUIRED_ARTIFACTS)):
            blockers.append(
                blocker(
                    "lineage_proof_evidence_artifact_sizes_unexpected_field",
                    "Reserved-lineage proof evidence artifact_size_bytes contains unexpected field",
                    field=_display_evidence_field(artifact),
                )
            )
    if not isinstance(artifacts, dict):
        blockers.append(
            blocker(
                "lineage_proof_evidence_artifacts",
                "Reserved-lineage proof evidence artifacts must be an object",
            )
        )
    else:
        for artifact in sorted(set(artifacts) - set(LINEAGE_PROOF_REQUIRED_ARTIFACTS)):
            blockers.append(
                blocker(
                    "lineage_proof_evidence_artifacts_unexpected_field",
                    "Reserved-lineage proof evidence artifacts contains unexpected field",
                    field=_display_evidence_field(artifact),
                )
            )
        artifact_count = len(artifacts)
        artifact_root = path.parent
        for artifact in LINEAGE_PROOF_REQUIRED_ARTIFACTS:
            expected_digest = artifacts.get(artifact)
            expected_size = artifact_sizes.get(artifact) if artifact_sizes_valid else None
            _require_lineage_sha256(
                blockers,
                value=expected_digest,
                field=f"artifacts.{artifact}",
                code="lineage_proof_evidence_artifact_digest",
            )
            artifact_path = artifact_root / artifact
            artifact_file_errors = validate_lineage_local_file(
                artifact_path,
                "Reserved-lineage proof evidence artifact file",
            )
            if artifact_file_errors:
                if artifact_file_errors == [
                    "Reserved-lineage proof evidence artifact file is missing"
                ]:
                    blockers.append(
                        blocker(
                            "lineage_proof_evidence_artifact_missing",
                            "Reserved-lineage proof evidence artifact file is missing",
                            artifact=artifact,
                        )
                    )
                else:
                    for error in artifact_file_errors:
                        blockers.append(
                            blocker(
                                "lineage_proof_evidence_artifact_file_shape",
                                error,
                                artifact=artifact,
                            )
                        )
                continue
            (
                actual_digest,
                artifact_size,
                artifact_prefix,
                digest_errors,
            ) = _sha256_file_with_size_and_prefix(
                artifact_path,
                "Reserved-lineage proof evidence artifact file",
                allow_empty=True,
            )
            if digest_errors:
                for error in digest_errors:
                    blockers.append(
                        blocker(
                            "lineage_proof_evidence_artifact_file_shape",
                            error,
                            artifact=artifact,
                        )
                    )
                continue
            assert (
                actual_digest is not None
                and artifact_size is not None
                and artifact_prefix is not None
            )
            size_matches = _require_lineage_artifact_size(
                blockers,
                value=expected_size,
                artifact=artifact,
                actual_size=artifact_size,
            )
            if artifact_size <= 0:
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_artifact_empty",
                        "Reserved-lineage proof evidence artifact file must be non-empty",
                        artifact=artifact,
                    )
                )
                continue
            content_errors = validate_lineage_artifact_prefix(artifact_prefix, artifact)
            if content_errors:
                for error in content_errors:
                    blockers.append(
                        blocker(
                            "lineage_proof_evidence_artifact_placeholder",
                            error,
                            artifact=artifact,
                        )
                    )
                continue
            if _is_lower_sha256_hex(expected_digest):
                if actual_digest != expected_digest:
                    blockers.append(
                        blocker(
                            "lineage_proof_evidence_artifact_file_digest",
                            "Reserved-lineage proof evidence artifact digest does not match local artifact bytes",
                            artifact=artifact,
                        )
                    )
                elif size_matches:
                    validated_artifact_sha256[artifact] = actual_digest
                    validated_artifact_sizes[artifact] = artifact_size
    details["artifact_count"] = artifact_count
    details["artifact_sha256"] = validated_artifact_sha256
    details["artifact_size_bytes"] = validated_artifact_sizes

    tests = evidence.get("tests")
    validated_test_log_sha256: dict[str, str] = {}
    if not isinstance(tests, dict):
        blockers.append(
            blocker(
                "lineage_proof_evidence_tests",
                "Reserved-lineage proof evidence tests must be an object",
            )
        )
    else:
        for key in sorted(set(tests) - set(LINEAGE_PROOF_REQUIRED_TESTS)):
            blockers.append(
                blocker(
                    "lineage_proof_evidence_tests_unexpected_field",
                    "Reserved-lineage proof evidence tests contains unexpected field",
                    field=_display_evidence_field(key),
                )
            )
        details["tests"] = [_display_evidence_field(key) for key in sorted(tests)]
        for key, expected_name in LINEAGE_PROOF_REQUIRED_TESTS.items():
            test = tests.get(key)
            if not isinstance(test, dict):
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_test_missing",
                        f"Reserved-lineage proof evidence test {key} is required",
                        test=key,
                    )
                )
                continue
            for field in sorted(set(test) - LINEAGE_PROOF_TEST_FIELDS):
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_test_unexpected_field",
                        f"Reserved-lineage proof evidence test {key} contains unexpected field",
                        test=key,
                        field=_display_evidence_field(field),
                    )
                )
            if test.get("name") != expected_name:
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_test_name",
                        f"Reserved-lineage proof evidence test {key} name mismatch",
                        test=key,
                    )
                )
            if test.get("status") != "passed":
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_test_status",
                        f"Reserved-lineage proof evidence test {key} must have passed",
                        test=key,
                    )
                )
            if test.get("ignored") is not True:
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_test_ignored",
                        f"Reserved-lineage proof evidence test {key} must record ignored=true",
                        test=key,
                    )
                )
            command = test.get("command")
            command_errors = validate_lineage_proof_command(command, expected_name)
            for error in command_errors:
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_test_command",
                        f"Reserved-lineage proof evidence test {key} command is not the production-width ignored proof run",
                        test=key,
                        issue=error,
                    )
                )
            elapsed_seconds = test.get("elapsed_seconds")
            if (
                not isinstance(elapsed_seconds, (int, float))
                or isinstance(elapsed_seconds, bool)
                or not math.isfinite(float(elapsed_seconds))
                or elapsed_seconds <= 0
            ):
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_test_elapsed",
                        f"Reserved-lineage proof evidence test {key} elapsed_seconds must be positive",
                        test=key,
                    )
                )
            _require_lineage_sha256(
                blockers,
                value=test.get("log_sha256"),
                field=f"tests.{key}.log_sha256",
                code="lineage_proof_evidence_test_log_digest",
            )
            expected_log_path = LINEAGE_PROOF_REQUIRED_TEST_LOGS[key]
            log_path = test.get("log_path")
            if log_path != expected_log_path:
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_test_log_path",
                        f"Reserved-lineage proof evidence test {key} log_path mismatch",
                        test=key,
                        expected=expected_log_path,
                    )
                )
                continue
            log_artifact_path = path.parent / expected_log_path
            actual_log_digest, log_errors = validate_lineage_proof_log(
                log_artifact_path, expected_name
            )
            log_file_missing = log_errors == ["missing production proof log"]
            if actual_log_digest is None:
                blockers.append(
                    blocker(
                        (
                            "lineage_proof_evidence_test_log_unreadable"
                            if not log_file_missing
                            else "lineage_proof_evidence_test_log_missing"
                        ),
                        (
                            f"Reserved-lineage proof evidence test {key} log file could not be validated"
                            if not log_file_missing
                            else f"Reserved-lineage proof evidence test {key} log file is missing"
                        ),
                        test=key,
                    )
                )
            elif _is_lower_sha256_hex(test.get("log_sha256")) and actual_log_digest != test.get(
                "log_sha256"
            ):
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_test_log_file_digest",
                        f"Reserved-lineage proof evidence test {key} log digest does not match local log bytes",
                        test=key,
                    )
                )
            elif actual_log_digest is not None and not log_errors:
                validated_test_log_sha256[key] = actual_log_digest
            for error in log_errors:
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_test_log_content",
                        f"Reserved-lineage proof evidence test {key} log content is not a passing production proof log",
                        test=key,
                        issue=error,
                    )
                )

    details["test_log_sha256"] = validated_test_log_sha256
    details["ok"] = not blockers
    details["state"] = "production_width_proof_passed" if not blockers else "blocked"
    details["blockers"] = blockers
    return details


def check_compact_key_evidence(
    path: Path,
    *,
    min_generated_at: dt.datetime | None = None,
    max_generated_at: dt.datetime | None = None,
    require_canonical_filename: bool = True,
) -> dict[str, Any]:
    """Check ABI-7 recursive compact key-artifact release evidence."""

    blockers: list[dict[str, Any]] = []
    if require_canonical_filename and path.name != COMPACT_KEY_EVIDENCE_FILENAME:
        blockers.append(
            blocker(
                "compact_key_evidence_filename",
                (
                    "ABI-7 recursive compact key evidence file must be named "
                    f"{COMPACT_KEY_EVIDENCE_FILENAME}"
                ),
                expected=COMPACT_KEY_EVIDENCE_FILENAME,
            )
        )
        return {
            "path": COMPACT_KEY_EVIDENCE_SUMMARY_LABEL,
            "schema": None,
            "artifact_sha256": {},
            "artifact_size_bytes": {},
            "min_generated_at_utc": (
                min_generated_at.isoformat().replace("+00:00", "Z")
                if min_generated_at is not None
                else None
            ),
            "max_generated_at_utc": (
                max_generated_at.isoformat().replace("+00:00", "Z")
                if max_generated_at is not None
                else None
            ),
            "ok": False,
            "blockers": blockers,
        }
    evidence_file_errors = [
        *device_lab.validate_no_symlink_ancestors(
            path,
            "ABI-7 recursive compact key evidence ancestor directory",
        )
    ]
    for error in validate_lineage_local_file(
        path,
        "ABI-7 recursive compact key evidence file",
    ):
        if error != "ABI-7 recursive compact key evidence file is missing":
            evidence_file_errors.append(error)
    if evidence_file_errors:
        for error in evidence_file_errors:
            blockers.append(blocker("compact_key_evidence_file_shape", error))
        return {
            "path": COMPACT_KEY_EVIDENCE_SUMMARY_LABEL,
            "schema": None,
            "artifact_sha256": {},
            "artifact_size_bytes": {},
            "min_generated_at_utc": (
                min_generated_at.isoformat().replace("+00:00", "Z")
                if min_generated_at is not None
                else None
            ),
            "max_generated_at_utc": (
                max_generated_at.isoformat().replace("+00:00", "Z")
                if max_generated_at is not None
                else None
            ),
            "ok": False,
            "blockers": blockers,
        }
    evidence, load_blockers = _load_json_artifact(
        path,
        missing_code="compact_key_evidence_missing",
        invalid_code="compact_key_evidence_invalid_json",
        unreadable_code="compact_key_evidence_unreadable",
        shape_code="compact_key_evidence_file_shape",
        not_object_code="compact_key_evidence_not_object",
        label="ABI-7 recursive compact key evidence",
        max_bytes=MAX_COMPACT_KEY_EVIDENCE_JSON_BYTES,
    )
    blockers.extend(load_blockers)
    details: dict[str, Any] = {
        "path": COMPACT_KEY_EVIDENCE_SUMMARY_LABEL,
        "schema": None,
        "artifact_sha256": {},
        "artifact_size_bytes": {},
        "generator_log_sha256": None,
        "generator_log_artifact_size_bytes": {},
        "min_generated_at_utc": (
            min_generated_at.isoformat().replace("+00:00", "Z")
            if min_generated_at is not None
            else None
        ),
        "max_generated_at_utc": (
            max_generated_at.isoformat().replace("+00:00", "Z")
            if max_generated_at is not None
            else None
        ),
    }
    if evidence is None:
        details["ok"] = False
        details["blockers"] = blockers
        return details

    for field in sorted(set(evidence) - COMPACT_KEY_EVIDENCE_FIELDS):
        blockers.append(
            blocker(
                "compact_key_evidence_unexpected_field",
                "ABI-7 recursive compact key evidence contains unexpected field",
                field=_display_evidence_field(field),
            )
        )

    details["schema"] = _display_evidence_value(evidence.get("schema"))
    details["generated_at_utc"] = None
    if evidence.get("schema") != COMPACT_KEY_EVIDENCE_SCHEMA:
        blockers.append(
            blocker(
                "compact_key_evidence_schema",
                "ABI-7 recursive compact key evidence schema mismatch",
            )
        )

    generated_at_text = evidence.get("generated_at_utc")
    if not isinstance(generated_at_text, str) or not generated_at_text.strip():
        blockers.append(
            blocker(
                "compact_key_evidence_timestamp_missing",
                "ABI-7 recursive compact key evidence generated_at_utc is required",
            )
        )
    else:
        compact_generated_at_raw = generated_at_text
        details["generated_at_utc"] = _display_evidence_value(compact_generated_at_raw)
        skip_timestamp_parse = _append_evidence_timestamp_string_blockers(
            blockers,
            code_prefix="compact_key_evidence",
            label="ABI-7 recursive compact key evidence",
            raw=compact_generated_at_raw,
        )
        if (
            not skip_timestamp_parse
            and device_lab.SIGNED_AT_UTC_RE.fullmatch(compact_generated_at_raw) is None
        ):
            blockers.append(
                blocker(
                    "compact_key_evidence_timestamp_noncanonical",
                    "ABI-7 recursive compact key evidence generated_at_utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
                    generated_at_utc=_display_evidence_value(compact_generated_at_raw),
                )
            )
        if not skip_timestamp_parse:
            generated_at, parse_blocker = parse_utc_timestamp(
                compact_generated_at_raw,
                "ABI-7 recursive compact key evidence generated_at_utc",
            )
            if parse_blocker is not None:
                parse_blocker["code"] = "compact_key_evidence_timestamp_invalid"
                blockers.append(parse_blocker)
            elif min_generated_at is not None and generated_at is not None and generated_at < min_generated_at:
                blockers.append(
                    blocker(
                        "compact_key_evidence_stale",
                        "ABI-7 recursive compact key evidence predates the required release evidence cutoff",
                        generated_at_utc=_display_evidence_value(compact_generated_at_raw),
                        min_generated_at_utc=min_generated_at.isoformat().replace("+00:00", "Z"),
                    )
                )
            elif max_generated_at is not None and generated_at is not None and generated_at > max_generated_at:
                blockers.append(
                    blocker(
                        "compact_key_evidence_future_dated",
                        "ABI-7 recursive compact key evidence is ahead of the release validator clock skew",
                        generated_at_utc=_display_evidence_value(compact_generated_at_raw),
                        max_generated_at_utc=max_generated_at.isoformat().replace("+00:00", "Z"),
                    )
                )

    expected_scalars = {
        "opening_len": EXPECTED_COMPACT_KEY_OPENING_LEN,
        "ipa_k": EXPECTED_COMPACT_KEY_IPA_K,
        "record_version": EXPECTED_COMPACT_KEY_RECORD_VERSION,
    }
    for field, expected in expected_scalars.items():
        compact_scalar_value = evidence.get(field)
        if (
            not isinstance(compact_scalar_value, int)
            or isinstance(compact_scalar_value, bool)
            or compact_scalar_value != expected
        ):
            blockers.append(
                blocker(
                    f"compact_key_evidence_{field}",
                    f"ABI-7 recursive compact key evidence {field} must be integer {expected}",
                    field=field,
                )
            )
    details["opening_len"] = evidence.get("opening_len")
    details["ipa_k"] = evidence.get("ipa_k")
    details["record_version"] = evidence.get("record_version")

    for field, expected in {
        "verifier_backend": EXPECTED_COMPACT_KEY_BACKEND,
        "circuit_id": EXPECTED_COMPACT_KEY_CIRCUIT_ID,
        "record_namespace": EXPECTED_COMPACT_KEY_RECORD_NAMESPACE,
    }.items():
        if evidence.get(field) != expected:
            blockers.append(
                blocker(
                    f"compact_key_evidence_{field}",
                    f"ABI-7 recursive compact key evidence {field} mismatch",
                    field=field,
                    expected=expected,
                )
            )
        details[field] = _display_evidence_value(evidence.get(field))

    command_errors = validate_compact_key_command(evidence.get("command"))
    for error in command_errors:
        blockers.append(
            blocker(
                "compact_key_evidence_command",
                "ABI-7 recursive compact key evidence command is not the canonical keygen run",
                issue=error,
            )
        )
    details["command_validated"] = not command_errors

    artifacts = evidence.get("artifacts")
    artifact_sizes = evidence.get("artifact_size_bytes")
    artifact_count = 0
    validated_artifact_sha256: dict[str, str] = {}
    validated_artifact_sizes: dict[str, int] = {}
    local_artifact_sha256: dict[str, str] = {}
    local_artifact_sizes: dict[str, int] = {}
    artifact_sizes_valid = isinstance(artifact_sizes, dict)
    if not artifact_sizes_valid:
        blockers.append(
            blocker(
                "compact_key_evidence_artifact_sizes",
                "ABI-7 recursive compact key evidence artifact_size_bytes must be an object",
            )
        )
    else:
        for artifact in sorted(set(artifact_sizes) - set(COMPACT_KEY_REQUIRED_ARTIFACTS)):
            blockers.append(
                blocker(
                    "compact_key_evidence_artifact_sizes_unexpected_field",
                    "ABI-7 recursive compact key evidence artifact_size_bytes contains unexpected field",
                    field=_display_evidence_field(artifact),
                )
            )
    if not isinstance(artifacts, dict):
        blockers.append(
            blocker(
                "compact_key_evidence_artifacts",
                "ABI-7 recursive compact key evidence artifacts must be an object",
            )
        )
    else:
        for artifact in sorted(set(artifacts) - set(COMPACT_KEY_REQUIRED_ARTIFACTS)):
            blockers.append(
                blocker(
                    "compact_key_evidence_artifacts_unexpected_field",
                    "ABI-7 recursive compact key evidence artifacts contains unexpected field",
                    field=_display_evidence_field(artifact),
                )
            )
        artifact_count = len(artifacts)
        artifact_root = path.parent
        for artifact in COMPACT_KEY_REQUIRED_ARTIFACTS:
            expected_digest = artifacts.get(artifact)
            expected_size = artifact_sizes.get(artifact) if artifact_sizes_valid else None
            _require_compact_key_sha256(
                blockers,
                value=expected_digest,
                field=f"artifacts.{artifact}",
                code="compact_key_evidence_artifact_digest",
            )
            artifact_path = artifact_root / artifact
            artifact_file_errors = validate_lineage_local_file(
                artifact_path,
                "ABI-7 recursive compact key evidence artifact file",
            )
            if artifact_file_errors:
                if artifact_file_errors == [
                    "ABI-7 recursive compact key evidence artifact file is missing"
                ]:
                    blockers.append(
                        blocker(
                            "compact_key_evidence_artifact_missing",
                            "ABI-7 recursive compact key evidence artifact file is missing",
                            artifact=artifact,
                        )
                    )
                else:
                    for error in artifact_file_errors:
                        blockers.append(
                            blocker(
                                "compact_key_evidence_artifact_file_shape",
                                error,
                                artifact=artifact,
                            )
                        )
                continue
            (
                actual_digest,
                artifact_size,
                artifact_prefix,
                digest_errors,
            ) = _sha256_file_with_size_and_prefix(
                artifact_path,
                "ABI-7 recursive compact key evidence artifact file",
                allow_empty=True,
            )
            if digest_errors:
                for error in digest_errors:
                    blockers.append(
                        blocker(
                            "compact_key_evidence_artifact_file_shape",
                            error,
                            artifact=artifact,
                        )
                    )
                continue
            assert (
                actual_digest is not None
                and artifact_size is not None
                and artifact_prefix is not None
            )
            size_matches = _require_compact_key_artifact_size(
                blockers,
                value=expected_size,
                artifact=artifact,
                actual_size=artifact_size,
            )
            if artifact_size <= 0:
                blockers.append(
                    blocker(
                        "compact_key_evidence_artifact_empty",
                        "ABI-7 recursive compact key evidence artifact file must be non-empty",
                        artifact=artifact,
                    )
                )
                continue
            local_artifact_sizes[artifact] = artifact_size
            content_errors = validate_compact_key_artifact_prefix(artifact_prefix, artifact)
            if content_errors:
                for error in content_errors:
                    blockers.append(
                        blocker(
                            "compact_key_evidence_artifact_placeholder",
                            error,
                            artifact=artifact,
                        )
                    )
                continue
            local_artifact_sha256[artifact] = actual_digest
            if _is_lower_sha256_hex(expected_digest):
                if actual_digest != expected_digest:
                    blockers.append(
                        blocker(
                            "compact_key_evidence_artifact_file_digest",
                            "ABI-7 recursive compact key evidence artifact digest does not match local artifact bytes",
                            artifact=artifact,
                        )
                    )
                elif size_matches:
                    validated_artifact_sha256[artifact] = actual_digest
                    validated_artifact_sizes[artifact] = artifact_size
    generator_log_path = evidence.get("generator_log_path")
    generator_log_sha256 = evidence.get("generator_log_sha256")
    if generator_log_path != COMPACT_KEY_GENERATOR_LOG_FILENAME:
        _require_compact_key_sha256(
            blockers,
            value=generator_log_sha256,
            field="generator_log_sha256",
            code="compact_key_evidence_generator_log_sha256",
        )
        blockers.append(
            blocker(
                "compact_key_evidence_generator_log_path",
                (
                    "ABI-7 recursive compact key evidence generator_log_path must be "
                    f"{COMPACT_KEY_GENERATOR_LOG_FILENAME}"
                ),
                field=(
                    _display_evidence_field(generator_log_path)
                    if isinstance(generator_log_path, str)
                    else generator_log_path
                ),
            )
        )
    else:
        actual_log_digest, generator_log_sizes, generator_log_digests, generator_log_blockers = (
            validate_compact_key_generator_log(
                path.parent / COMPACT_KEY_GENERATOR_LOG_FILENAME,
                generator_log_sha256,
                local_artifact_sizes,
                local_artifact_sha256,
            )
        )
        blockers.extend(generator_log_blockers)
        if actual_log_digest is not None and not generator_log_blockers:
            details["generator_log_sha256"] = actual_log_digest
            details["generator_log_artifact_size_bytes"] = generator_log_sizes
            details["generator_log_artifact_sha256"] = generator_log_digests
    details["artifact_count"] = artifact_count
    details["artifact_sha256"] = validated_artifact_sha256
    details["artifact_size_bytes"] = validated_artifact_sizes
    details["ok"] = not blockers
    details["state"] = "compact_key_artifacts_validated" if not blockers else "blocked"
    details["blockers"] = blockers
    return details


def check_abi7_fail_closed(repo_root: Path) -> dict[str, Any]:
    """Check ABI-7 recursive compact launch-boundary source markers."""

    repo_root_blockers = validate_repo_root_path(repo_root)
    if repo_root_blockers:
        return {
            "ok": False,
            "state": "unknown",
            "circuit_id": "kagemusha-recursive-compact-v1",
            "blockers": repo_root_blockers,
        }

    blockers: list[dict[str, Any]] = []
    fixture = check_abi7_fixture_manifest(repo_root)
    blockers.extend(fixture["blockers"])
    source_texts: dict[str, str] = {}
    for relative, label in (
        ("crates/iroha_core/src/zk.rs", "ABI-7 core marker file"),
        (
            "crates/connect_norito_bridge/src/lib.rs",
            "ABI-7 bridge marker file",
        ),
    ):
        path = repo_root / relative
        unreadable_error = "ABI-7 source marker file could not be read"
        text, file_errors = _repo_source_marker_text(
            path,
            label,
            unreadable_error,
        )
        if file_errors:
            for error in file_errors:
                code = (
                    "abi7_source_marker_file_unreadable"
                    if error == unreadable_error
                    else "abi7_source_marker_file_shape"
                )
                blockers.append(
                    blocker(
                        code,
                        error,
                        file=relative,
                    )
                )
            continue
        assert text is not None
        source_texts[relative] = text
    core_text = source_texts.get("crates/iroha_core/src/zk.rs", "")
    bridge_text = source_texts.get("crates/connect_norito_bridge/src/lib.rs", "")
    required_core_snippets = (
        "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE",
        "multi-hop proving requires the append verifier batch to be composed into the compact proof",
        "KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_PROOF_UNAVAILABLE",
        "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_OPENING_LEN",
        "KAGEMUSHA_RECURSIVE_COMPACT_MIN_PROOF_BYTES",
        "prove_halo2_ipa_kagemusha_recursive_compact_payment_token_one_hop_envelope",
        "height-aware detached compact Pallas archive must reject before proving",
        "height-aware extra compact Pallas opening must reject before proving",
        "height-aware missing compact Pallas opening must reject before proving",
        "duplicated multi-hop compact Pallas archive must reject before proving",
        "height-aware duplicated multi-hop compact Pallas archive must reject before proving",
        "forged multi-hop compact Pallas metadata must reject before proving",
        "height-aware forged multi-hop compact Pallas metadata must reject before proving",
        "reordered multi-hop compact Pallas archive must reject before proving",
        "height-aware reordered multi-hop compact Pallas archive must reject before proving",
    )
    for snippet in required_core_snippets:
        if snippet not in core_text:
            blockers.append(
                blocker(
                    "abi7_fail_closed_marker_missing",
                    "ABI-7 recursive compact launch-boundary marker is missing",
                    marker=snippet,
                )
            )
    core_function_contracts = (
        (
            "fn prove_kagemusha_recursive_compact_payment_token_one_hop_from_record_bundle_and_pallas_open_envelopes(",
            (
                "kagemusha_pallas_ipa_batch_verifier_preflight_bound_to_hop_proofs(",
                "validate_kagemusha_recursive_one_hop_verifier_slice_preflight_binding(",
                "kagemusha_recursive_spend_lineage_runtime_keygen_enabled()",
                "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_KEY_ARTIFACTS_REQUIRED",
                "missing compact one-hop proving key archive",
                "prove_halo2_ipa_kagemusha_recursive_compact_payment_token_one_hop_envelope_dispatch(",
            ),
        ),
        (
            "fn prove_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelopes(",
            (
                "prove_kagemusha_recursive_compact_payment_token_one_hop_from_record_bundle_and_pallas_open_envelopes(",
                "prove_halo2_ipa_kagemusha_recursive_compact_payment_token_append_envelope_dispatch(",
                "for hop_index in 1..hop_count",
                "kagemusha_recursive_spend_lineage_runtime_keygen_enabled()",
                "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_KEY_ARTIFACTS_REQUIRED",
                "missing compact append proving key archive",
            ),
        ),
        (
            "fn prove_halo2_ipa_kagemusha_recursive_compact_payment_token_one_hop_envelope_dispatch(",
            (
                "prove_halo2_ipa_kagemusha_recursive_compact_payment_token_one_hop_envelope::<$len>",
                "match usize::try_from(preflight.opening_len)",
                "4 => prove_len!(4)",
            ),
        ),
        (
            "pub fn prove_verified_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelope_archive(",
            (
                "prove_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelopes(",
                "proving_key_bytes",
                "None",
            ),
        ),
        (
            "pub fn prove_verified_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelope_archive_at_height(",
            (
                "prove_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelopes(",
                "proving_key_bytes",
                "Some(block_height)",
            ),
        ),
        (
            "pub fn preverify_kagemusha_recursive_compact_payment_token(",
            (
                "preverify_kagemusha_recursive_compact_payment_token_with_expected_circuit_id(",
                "KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1",
            ),
        ),
        (
            "pub fn verify_kagemusha_recursive_compact_payment_token(",
            (
                "preverify_kagemusha_recursive_compact_payment_token_with_expected_circuit_id(",
                "verify_backend(",
            ),
        ),
        (
            "pub fn preverify_kagemusha_recursive_compact_payment_token_with_record(",
            (
                "preverify_kagemusha_recursive_compact_payment_token_with_record_at_optional_height_and_expected_circuit_id(",
                "KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1",
            ),
        ),
        (
            "pub fn preverify_kagemusha_recursive_compact_payment_token_with_record_at_height(",
            (
                "preverify_kagemusha_recursive_compact_payment_token_with_record_at_optional_height_and_expected_circuit_id(",
                "Some(block_height)",
                "KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1",
            ),
        ),
        (
            "pub fn verify_kagemusha_recursive_compact_payment_token_with_record(",
            (
                "preverify_kagemusha_recursive_compact_payment_token_with_record_at_optional_height_and_expected_circuit_id(",
                "verify_backend(",
            ),
        ),
        (
            "pub fn verify_kagemusha_recursive_compact_payment_token_with_record_at_height(",
            (
                "preverify_kagemusha_recursive_compact_payment_token_with_record_at_optional_height_and_expected_circuit_id(",
                "Some(block_height)",
                "verify_backend(",
            ),
        ),
    )
    for signature, snippets in core_function_contracts:
        missing_snippets = _require_rust_function_contract(core_text, signature, snippets)
        for snippet in missing_snippets:
            blockers.append(
                blocker(
                    "abi7_fail_closed_contract_missing",
                    "ABI-7 recursive compact launch-boundary function contract is missing",
                    function=signature,
                    marker=snippet,
                )
            )
    if "ERR_KAGEMUSHA_RECURSIVE_COMPACT_UNAVAILABLE" not in bridge_text:
        blockers.append(
            blocker(
                "abi7_bridge_unavailable_code_missing",
                "native bridge must expose recursive compact unavailable status",
            )
        )
    bridge_function_contracts = (
        (
            "pub unsafe extern \"C\" fn connect_norito_kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes(",
            (
                "prove_verified_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelope_archive_with_key_artifacts(",
                "is_kagemusha_recursive_compact_unavailable_error(&err)",
                "BridgeError::KagemushaRecursiveCompactUnavailable",
            ),
        ),
        (
            "pub unsafe extern \"C\" fn connect_norito_kagemusha_verify_recursive_compact_payment_token(",
            (
                "preverify_kagemusha_recursive_compact_payment_token(&token, vk_box)",
                "Err(err) if is_kagemusha_recursive_compact_unavailable_error(&err) => {}",
                "verify_kagemusha_recursive_compact_payment_token(&token, vk_box)",
                "*out_valid = 0",
            ),
        ),
    )
    for signature, snippets in bridge_function_contracts:
        missing_snippets = _require_rust_function_contract(bridge_text, signature, snippets)
        for snippet in missing_snippets:
            blockers.append(
                blocker(
                    "abi7_bridge_unavailable_contract_missing",
                    "native bridge must map ABI-7 recursive compact unavailable separately",
                    function=signature,
                    marker=snippet,
                )
            )
    details = {
        "ok": not blockers,
        "state": "package_aware_multi_hop_composed" if not blockers else "unknown",
        "circuit_id": "kagemusha-recursive-compact-v1",
        "blockers": blockers,
    }
    details.update(
        {
            key: value
            for key, value in fixture.items()
            if key not in ("ok", "blockers")
        }
    )
    return details


def check_lineage_key_release_tooling(repo_root: Path) -> dict[str, Any]:
    """Check release-time Reserved-lineage key packages and verifier-record tooling."""

    repo_root_blockers = validate_repo_root_path(repo_root)
    if repo_root_blockers:
        return {
            "ok": False,
            "state": "unknown",
            "checked_files": [],
            "blockers": repo_root_blockers,
        }

    blockers: list[dict[str, Any]] = []
    checked_files: list[str] = []
    for relative, snippets in LINEAGE_KEY_RELEASE_TOOLING_REQUIREMENTS.items():
        path = repo_root / relative
        unreadable_error = "Reserved-lineage release-tooling file could not be read"
        text, file_errors = _repo_source_marker_text(
            path,
            "Reserved-lineage release-tooling marker file",
            unreadable_error,
        )
        if file_errors:
            missing_error = "Reserved-lineage release-tooling marker file is missing"
            if file_errors == [missing_error]:
                blockers.append(
                    blocker(
                        "lineage_key_release_file_missing",
                        "Reserved-lineage release-tooling file is missing",
                        file=relative,
                    )
                )
            elif file_errors == [unreadable_error]:
                blockers.append(
                    blocker(
                        "lineage_key_release_file_unreadable",
                        unreadable_error,
                        file=relative,
                    )
                )
            else:
                for error in file_errors:
                    blockers.append(
                        blocker(
                            "lineage_key_release_file_shape",
                            error,
                            file=relative,
                        )
                    )
            continue
        assert text is not None
        checked_files.append(relative)
        for snippet in snippets:
            if snippet not in text:
                blockers.append(
                    blocker(
                        "lineage_key_release_marker_missing",
                        "Reserved-lineage release-tooling marker is missing",
                        file=relative,
                        marker=snippet,
                    )
                )
    return {
        "ok": not blockers,
        "state": "record_artifacts_wired" if not blockers else "unknown",
        "checked_files": checked_files,
        "blockers": blockers,
    }


def _slot_reports(
    root: Path,
    trusted_signer_public_keys: dict[str, Path],
    slot_ids: Iterable[str] | None,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    slot_paths, discovery_errors = device_lab.discover_slots(root, slot_ids)
    if discovery_errors:
        return [], [
            blocker("android_device_lab_root_unreadable", error)
            for error in discovery_errors
        ]
    return [
        device_lab.scan_slot(
            slot_path,
            require_kagemusha_production_evidence=True,
            trusted_signer_public_keys=trusted_signer_public_keys,
        )
        for slot_path in slot_paths
    ], []


def _redact_secret_strings(value: Any) -> tuple[Any, bool, bool, bool, bool]:
    """Return a redacted copy plus unsafe-material/key flags."""

    if isinstance(value, str):
        if device_lab.SECRET_RE.search(value):
            return device_lab.SECRET_PATH_REDACTION, True, False, False, False
        if device_lab._contains_control_character(value):
            return EVIDENCE_CONTROL_STRING_REDACTION, True, False, False, False
        return value, False, False, False, False
    if value is None or isinstance(value, (bool, int)):
        return value, False, False, False, False
    if isinstance(value, float) and not math.isfinite(value):
        return EVIDENCE_NONFINITE_NUMBER_REDACTION, True, False, False, False
    if isinstance(value, float):
        return value, False, False, False, False
    if isinstance(value, list):
        redacted_items = []
        matched = False
        key_collision = False
        non_string_key = False
        unsupported_value = False
        for item in value:
            (
                redacted_item,
                item_matched,
                item_key_collision,
                item_non_string_key,
                item_unsupported_value,
            ) = _redact_secret_strings(item)
            redacted_items.append(redacted_item)
            matched = matched or item_matched
            key_collision = key_collision or item_key_collision
            non_string_key = non_string_key or item_non_string_key
            unsupported_value = unsupported_value or item_unsupported_value
        return redacted_items, matched, key_collision, non_string_key, unsupported_value
    if isinstance(value, dict):
        redacted: dict[Any, Any] = {}
        matched = False
        key_collision = False
        non_string_key = False
        unsupported_value = False
        for key, item in value.items():
            if isinstance(key, str):
                (
                    redacted_key,
                    key_matched,
                    nested_key_collision,
                    nested_non_string_key,
                    nested_unsupported_value,
                ) = _redact_secret_strings(key)
            else:
                redacted_key = EVIDENCE_NON_STRING_KEY_REDACTION
                key_matched = True
                nested_key_collision = False
                nested_non_string_key = True
                nested_unsupported_value = False
            (
                redacted_item,
                item_matched,
                item_key_collision,
                item_non_string_key,
                item_unsupported_value,
            ) = _redact_secret_strings(item)
            if redacted_key in redacted:
                key_collision = True
                matched = True
                continue
            redacted[redacted_key] = redacted_item
            matched = matched or key_matched or item_matched
            key_collision = key_collision or nested_key_collision or item_key_collision
            non_string_key = (
                non_string_key or nested_non_string_key or item_non_string_key
            )
            unsupported_value = (
                unsupported_value
                or nested_unsupported_value
                or item_unsupported_value
            )
        return redacted, matched, key_collision, non_string_key, unsupported_value
    return EVIDENCE_UNSUPPORTED_VALUE_REDACTION, True, False, False, True


def _sanitize_android_reports(
    reports: list[dict[str, Any]],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Redact report-local unsafe material before rollup summary serialization."""

    sanitized: list[dict[str, Any]] = []
    blockers: list[dict[str, Any]] = []
    for report in reports:
        (
            redacted_report,
            matched,
            key_collision,
            non_string_key,
            unsupported_value,
        ) = (
            _redact_secret_strings(report)
        )
        if isinstance(redacted_report, dict):
            sanitized_report = redacted_report
        else:
            sanitized_report = {"slot": "<invalid-slot-report>", "status": "error"}
            matched = True
        errors_normalized = False
        if "errors" in sanitized_report:
            safe_errors, errors_normalized = _android_report_errors_value(
                sanitized_report["errors"]
            )
            sanitized_report["errors"] = safe_errors
            if errors_normalized:
                sanitized_report[EVIDENCE_ERRORS_NORMALIZED_FIELD] = True
        sanitized.append(sanitized_report)
        slot = sanitized_report.get("slot")
        if matched:
            blockers.append(
                blocker(
                    "android_device_lab_report_unsafe_material",
                    "Android device-lab report contains secret-looking, control-character, or non-finite material",
                    slot=slot if isinstance(slot, str) else "<invalid-slot-report>",
                )
            )
        if key_collision:
            blockers.append(
                blocker(
                    "android_device_lab_report_redacted_key_collision",
                    "Android device-lab report keys collide after unsafe-material redaction",
                    slot=slot if isinstance(slot, str) else "<invalid-slot-report>",
                )
            )
        if non_string_key:
            blockers.append(
                blocker(
                    "android_device_lab_report_non_string_key",
                    "Android device-lab report contains a non-string key",
                    slot=slot if isinstance(slot, str) else "<invalid-slot-report>",
                )
            )
        if unsupported_value:
            blockers.append(
                blocker(
                    "android_device_lab_report_unsupported_value",
                    "Android device-lab report contains a non-JSON value",
                    slot=slot if isinstance(slot, str) else "<invalid-slot-report>",
                )
            )
        if errors_normalized:
            blockers.append(
                blocker(
                    "android_device_lab_report_errors_malformed",
                    "Android device-lab report errors must be a list of strings",
                    slot=slot if isinstance(slot, str) else "<invalid-slot-report>",
                )
            )
    return sanitized, blockers


def _android_report_errors_value(value: Any) -> tuple[list[str], bool]:
    """Return a readiness-summary-safe list of Android report error strings."""

    if not isinstance(value, list):
        return [EVIDENCE_ERROR_REDACTION], True
    errors: list[str] = []
    normalized = False
    for item in value:
        if isinstance(item, str):
            safe_item = _display_evidence_value(item)
            errors.append(safe_item if isinstance(safe_item, str) else EVIDENCE_ERROR_REDACTION)
        else:
            errors.append(EVIDENCE_ERROR_REDACTION)
            normalized = True
    return errors, normalized


def _android_report_errors(report: dict[str, Any]) -> list[str]:
    """Return only normalized Android report errors for release-facing blockers."""

    errors, _normalized = _android_report_errors_value(report.get("errors", []))
    return errors


def _android_report_kagemusha(report: dict[str, Any]) -> dict[str, Any]:
    """Return Kagemusha report details only when they are shaped as an object."""

    kagemusha = report.get("kagemusha")
    return kagemusha if isinstance(kagemusha, dict) else {}


def _android_report_device_family(report: dict[str, Any]) -> str | None:
    """Return a canonical Kagemusha device family from a sanitized report."""

    family = _android_report_kagemusha(report).get("device_family")
    if (
        isinstance(family, str)
        and family in device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES
    ):
        return family
    return None


def _android_report_d2d_payment_transport(report: dict[str, Any]) -> str | None:
    """Return a canonical offline D2D payment transport from a sanitized report."""

    transport = _android_report_kagemusha(report).get("d2d_payment_transport")
    if isinstance(transport, str) and transport in device_lab.D2D_PAYMENT_TRANSPORTS:
        return transport
    return None


def _android_report_valid_d2d_transcript_binding(value: Any) -> tuple[str, str] | None:
    """Return a validated D2D transcript path/digest binding."""

    if not isinstance(value, dict) or set(value) != {"path", "sha256"}:
        return None
    path = _valid_android_signed_evidence_summary_value(
        "d2d_payment_transcript_path",
        value.get("path"),
    )
    digest = _valid_android_signed_evidence_summary_value(
        "d2d_payment_transcript_sha256",
        value.get("sha256"),
    )
    if path is None or digest is None:
        return None
    return path, digest


def _android_report_valid_d2d_transcript_bindings(
    kagemusha: dict[str, Any],
    declared_transports: set[str],
    primary_transport: str,
) -> bool:
    """Return whether the declared D2D transports have exact transcript bindings."""

    transcripts = kagemusha.get("d2d_payment_transcripts")
    if not isinstance(transcripts, dict) or set(transcripts) != declared_transports:
        return False
    primary_path = kagemusha.get("d2d_payment_transcript_path")
    primary_digest = kagemusha.get("d2d_payment_transcript_sha256")
    seen_paths: set[str] = set()
    seen_digests: set[str] = set()
    for transport in declared_transports:
        binding = _android_report_valid_d2d_transcript_binding(
            transcripts.get(transport)
        )
        if binding is None:
            return False
        path, digest = binding
        if path in seen_paths:
            return False
        if digest in seen_digests:
            return False
        seen_paths.add(path)
        seen_digests.add(digest)
        if transport == primary_transport and binding != (primary_path, primary_digest):
            return False
    return True


def _android_report_d2d_payment_transports(report: dict[str, Any]) -> list[str]:
    """Return D2D transports only when transcript declarations are release-bound."""

    kagemusha = _android_report_kagemusha(report)
    primary_transport = _android_report_d2d_payment_transport(report)
    transports = kagemusha.get("d2d_payment_transports")
    if isinstance(transports, list) and all(
        isinstance(transport, str) and transport in device_lab.D2D_PAYMENT_TRANSPORTS
        for transport in transports
    ):
        if transports != sorted(set(transports)):
            return []
        declared_transports = set(transports)
        if (
            primary_transport is not None
            and primary_transport in declared_transports
            and _android_report_valid_d2d_transcript_bindings(
                kagemusha,
                declared_transports,
                primary_transport,
            )
        ):
            return sorted(declared_transports)
        return []
    transcripts = kagemusha.get("d2d_payment_transcripts")
    if primary_transport is None:
        return []
    if transcripts is None:
        return [primary_transport]
    if _android_report_valid_d2d_transcript_bindings(
        kagemusha,
        {primary_transport},
        primary_transport,
    ):
        return [primary_transport]
    return []


def _android_d2d_payment_transport_coverage_by_family(
    reports: list[dict[str, Any]],
    signed_evidence: dict[str, dict[str, str]],
) -> dict[str, list[str]]:
    """Return admitted D2D transport coverage keyed by standard device family."""

    coverage: dict[str, set[str]] = {
        family: set() for family in device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES
    }
    for report in reports:
        if report.get("status") != "ok" or not _android_report_has_complete_signed_evidence(
            report,
            signed_evidence,
        ):
            continue
        family = _android_report_device_family(report)
        if family is None:
            continue
        coverage[family].update(_android_report_d2d_payment_transports(report))
    return {family: sorted(transports) for family, transports in coverage.items()}


def _missing_android_d2d_payment_transport_pairs(
    coverage_by_family: dict[str, list[str]],
) -> list[dict[str, str]]:
    """Return required family/transport pairs without admitted signed evidence."""

    missing: list[dict[str, str]] = []
    for family in device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES:
        covered = set(coverage_by_family.get(family, []))
        for transport in ANDROID_REQUIRED_D2D_PAYMENT_TRANSPORTS:
            if transport not in covered:
                missing.append({"device_family": family, "transport": transport})
    return missing


def _check_android_signed_evidence_freshness(
    reports: list[dict[str, Any]],
    min_signed_at: dt.datetime | None,
    max_signed_at: dt.datetime | None,
) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    for report in reports:
        if report.get("status") != "ok":
            continue
        slot_name = report.get("slot")
        if not isinstance(slot_name, str):
            blockers.append(
                blocker(
                    "android_device_lab_slot_name_missing",
                    "Android device-lab report is missing a slot name",
                )
            )
            continue
        signed_at_text = _android_report_kagemusha(report).get("signed_at_utc")
        if not isinstance(signed_at_text, str) or not signed_at_text:
            blockers.append(
                blocker(
                    "android_signed_evidence_timestamp_missing",
                    "validated Android device-lab report is missing signed evidence timestamp",
                    slot=slot_name,
                )
            )
            continue
        if device_lab.SIGNED_AT_UTC_RE.fullmatch(signed_at_text) is None:
            blockers.append(
                blocker(
                    "android_signed_evidence_timestamp_noncanonical",
                    "signed evidence artifact signed_at_utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
                    slot=slot_name,
                    signed_at_utc=_display_evidence_value(signed_at_text),
                )
            )
            continue
        signed_at, parse_blocker = parse_utc_timestamp(
            signed_at_text,
            "signed evidence artifact signed_at_utc",
        )
        if parse_blocker is not None:
            parse_blocker["slot"] = slot_name
            parse_blocker["code"] = "android_signed_evidence_timestamp_invalid"
            blockers.append(parse_blocker)
            continue
        if min_signed_at is not None and signed_at is not None and signed_at < min_signed_at:
            blockers.append(
                blocker(
                    "android_signed_evidence_stale",
                    "signed evidence artifact predates the required release evidence cutoff",
                    slot=slot_name,
                    signed_at_utc=signed_at_text,
                    min_signed_at_utc=min_signed_at.isoformat().replace("+00:00", "Z"),
                )
            )
        if max_signed_at is not None and signed_at is not None and signed_at > max_signed_at:
            blockers.append(
                blocker(
                    "android_signed_evidence_future_dated",
                    "signed evidence artifact is ahead of the release validator clock skew",
                    slot=slot_name,
                    signed_at_utc=signed_at_text,
                    max_signed_at_utc=max_signed_at.isoformat().replace("+00:00", "Z"),
                )
            )
    return blockers


def _android_report_duplicate_matrix_values(
    kagemusha: dict[str, Any],
    field: str,
) -> set[str]:
    """Return canonical matrix-binding values from a sanitized Android report."""

    values: set[str] = set()
    value = kagemusha.get(field)
    if (
        isinstance(value, str)
        and device_lab.SHA256_HEX_RE.fullmatch(value)
        and value != "0" * 64
    ):
        values.add(value)
    if field != "d2d_payment_transcript_sha256":
        return values
    transcripts = kagemusha.get("d2d_payment_transcripts")
    if not isinstance(transcripts, dict):
        return values
    for entry in transcripts.values():
        if not isinstance(entry, dict):
            continue
        digest = entry.get("sha256")
        if (
            isinstance(digest, str)
            and device_lab.SHA256_HEX_RE.fullmatch(digest)
            and digest != "0" * 64
        ):
            values.add(digest)
    return values


def _check_android_matrix_unique_bindings(
    reports: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    """Reject matrix rows copied from the same device or D2D evidence run."""

    blockers: list[dict[str, Any]] = []
    checks = (
        (
            "device_fingerprint_sha256",
            "android_device_lab_duplicate_device_fingerprint",
            "Android device-lab production slots must not reuse a device fingerprint",
        ),
        (
            "attestation_challenge_sha256",
            "android_device_lab_duplicate_attestation_challenge",
            "Android device-lab production slots must not reuse an attestation challenge",
        ),
        (
            "d2d_payment_transcript_sha256",
            "android_device_lab_duplicate_d2d_payment_transcript",
            "Android device-lab production slots must not reuse a D2D payment transcript digest",
        ),
    )
    for field, code, message in checks:
        seen: dict[str, set[str]] = {}
        for report in reports:
            if report.get("status") != "ok":
                continue
            slot = report.get("slot")
            kagemusha = _android_report_kagemusha(report)
            value = kagemusha.get(field)
            if not isinstance(slot, str) or not isinstance(value, str) or not value:
                if not isinstance(slot, str):
                    continue
            else:
                safe_slot = _display_evidence_value(slot)
                if field.endswith("_sha256") and (
                    device_lab.SHA256_HEX_RE.fullmatch(value) is None
                    or value == "0" * 64
                ):
                    blockers.append(
                        blocker(
                            "android_device_lab_binding_digest_invalid",
                            "Android device-lab production binding digests must be non-zero lowercase sha256 hex",
                            slot=safe_slot,
                            field=field,
                            value_sha256=_display_evidence_value(value),
                        )
                    )
                    continue
            safe_slot = _display_evidence_value(slot)
            for duplicate_value in _android_report_duplicate_matrix_values(
                kagemusha,
                field,
            ):
                seen.setdefault(duplicate_value, set()).add(safe_slot)
        for value, slots in sorted(seen.items()):
            if len(slots) <= 1:
                continue
            value_sha256 = (
                value
                if field.endswith("_sha256")
                else hashlib.sha256(value.encode("utf-8")).hexdigest()
            )
            blockers.append(
                blocker(
                    code,
                    message,
                    slots=sorted(slots),
                    value_sha256=value_sha256,
                )
            )
    return blockers


def _is_redacted_evidence_value(value: str) -> bool:
    return value in {
        device_lab.SECRET_PATH_REDACTION,
        device_lab.CONTROL_PATH_REDACTION,
        EVIDENCE_CONTROL_STRING_REDACTION,
        EVIDENCE_NONFINITE_NUMBER_REDACTION,
        EVIDENCE_NON_STRING_KEY_REDACTION,
        EVIDENCE_UNSUPPORTED_VALUE_REDACTION,
    }


def _valid_android_signed_evidence_summary_value(
    target_key: str,
    value: Any,
) -> str | None:
    if not isinstance(value, str) or not value or _is_redacted_evidence_value(value):
        return None
    if target_key == "signed_at_utc":
        return value if device_lab.SIGNED_AT_UTC_RE.fullmatch(value) else None
    if target_key in ANDROID_SIGNED_EVIDENCE_SUMMARY_SHA256_FIELDS:
        if device_lab.SHA256_HEX_RE.fullmatch(value) and value != "0" * 64:
            return value
        return None
    if target_key in ANDROID_SIGNED_EVIDENCE_SUMMARY_PATH_FIELDS:
        path_errors: list[str] = []
        normalized = device_lab._normalise_safe_relative_path(  # type: ignore[attr-defined]
            value,
            path_errors,
            f"Android signed-evidence summary {target_key}",
        )
        if normalized is not None and normalized == value:
            expected_root = ANDROID_SIGNED_EVIDENCE_SUMMARY_PATH_ROOTS[target_key]
            if not device_lab._safe_relative_path_is_child_of(  # type: ignore[attr-defined]
                normalized,
                expected_root,
            ):
                return None
            return value
        return None
    if target_key in ANDROID_SIGNED_EVIDENCE_SUMMARY_IDENTITY_FIELDS:
        if (
            value != value.strip()
            or device_lab._contains_control_character(value)
            or device_lab.SECRET_RE.search(value)
        ):
            return None
        if (
            target_key == "device_family"
            and value not in device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES
        ):
            return None
        return value
    return value


def _check_android_signed_evidence_summary_values(
    reports: list[dict[str, Any]],
    trusted_signer_public_key_sha256: frozenset[str] | None = None,
) -> list[dict[str, Any]]:
    """Reject direct reports with malformed release-facing signed-evidence fields."""

    blockers: list[dict[str, Any]] = []
    seen_slots: set[str] = set()
    for report in reports:
        if report.get("status") != "ok":
            continue
        raw_slot = report.get("slot")
        slot = (
            _display_evidence_value(raw_slot)
            if isinstance(raw_slot, str)
            else "<invalid-slot-report>"
        )
        safe_slot = _android_safe_slot_id(report)
        if safe_slot is None:
            blockers.append(
                blocker(
                    "android_signed_evidence_summary_slot_invalid",
                    "validated Android device-lab reports must use a safe signed-evidence summary slot",
                    slot=slot,
                )
            )
            continue
        if slot in seen_slots:
            blockers.append(
                blocker(
                    "android_signed_evidence_summary_slot_collision",
                    "validated Android device-lab reports must not collapse to the same signed-evidence summary slot",
                    slot=slot,
                )
            )
        else:
            seen_slots.add(slot)
        kagemusha = report.get("kagemusha")
        if not isinstance(kagemusha, dict):
            blockers.append(
                blocker(
                    "android_signed_evidence_summary_missing",
                    "validated Android device-lab report is missing signed-evidence summary fields",
                    slot=slot,
                )
            )
            continue
        for source_key, target_key in ANDROID_SIGNED_EVIDENCE_SUMMARY_FIELDS:
            value = kagemusha.get(source_key)
            if not isinstance(value, str) or not value:
                blockers.append(
                    blocker(
                        "android_signed_evidence_summary_missing",
                        "validated Android device-lab report is missing a signed-evidence summary field",
                        slot=slot,
                        field=target_key,
                    )
                )
                continue
            if _valid_android_signed_evidence_summary_value(target_key, value) is None:
                blockers.append(
                    blocker(
                        "android_signed_evidence_summary_invalid",
                        "validated Android device-lab report has a malformed signed-evidence summary field",
                        slot=slot,
                        field=target_key,
                    )
                )
                continue
            if (
                target_key == "signer_public_key_sha256"
                and trusted_signer_public_key_sha256 is not None
                and value not in trusted_signer_public_key_sha256
            ):
                blockers.append(
                    blocker(
                        "android_signed_evidence_summary_untrusted_signer",
                        "validated Android device-lab report signer must match a trusted signer public key",
                        slot=slot,
                        field=target_key,
                    )
                )
                continue
            if target_key == "signed_at_utc":
                _, parse_blocker = parse_utc_timestamp(
                    value,
                    "Android signed-evidence summary signed_at_utc",
                )
                if parse_blocker is not None:
                    parse_blocker["code"] = "android_signed_evidence_summary_invalid"
                    parse_blocker["slot"] = slot
                    parse_blocker["field"] = target_key
                    blockers.append(parse_blocker)
        device_family = kagemusha.get("device_family")
        device_model = kagemusha.get("device_model")
        device_codename = kagemusha.get("device_codename")
        if (
            isinstance(device_family, str)
            and isinstance(device_model, str)
            and isinstance(device_codename, str)
            and _valid_android_signed_evidence_summary_value(
                "device_family",
                device_family,
            )
            is not None
            and _valid_android_signed_evidence_summary_value(
                "device_model",
                device_model,
            )
            is not None
            and _valid_android_signed_evidence_summary_value(
                "device_codename",
                device_codename,
            )
            is not None
        ):
            inferred_family = device_lab.infer_kagemusha_device_family(
                device_model,
                device_codename,
            )
            if inferred_family != device_family:
                blockers.append(
                    blocker(
                        "android_signed_evidence_summary_invalid",
                        "validated Android device-lab report model/codename must match its device family",
                        slot=slot,
                        field="device_family",
                    )
                )
    return blockers


def _android_signed_evidence_summary(
    reports: list[dict[str, Any]],
    trusted_signer_public_key_sha256: frozenset[str] | None = None,
) -> dict[str, dict[str, str]]:
    """Return path-safe signed-evidence details for valid Android slots."""

    signed_evidence: dict[str, dict[str, str]] = {}
    for report in reports:
        if report.get("status") != "ok":
            continue
        slot = _android_safe_slot_id(report)
        kagemusha = _android_report_kagemusha(report)
        if slot is None or not isinstance(kagemusha, dict):
            continue
        entry: dict[str, str] = {}
        for source_key, target_key in ANDROID_SIGNED_EVIDENCE_SUMMARY_FIELDS:
            value = _valid_android_signed_evidence_summary_value(
                target_key,
                kagemusha.get(source_key),
            )
            if value is not None:
                entry[target_key] = value
        signer_public_key_sha256 = entry.get("signer_public_key_sha256")
        if (
            trusted_signer_public_key_sha256 is not None
            and signer_public_key_sha256 not in trusted_signer_public_key_sha256
        ):
            continue
        for pair in ANDROID_SIGNED_EVIDENCE_SUMMARY_ARTIFACT_PAIRS:
            expected = set(pair)
            artifact_fields = expected & set(entry)
            if artifact_fields and artifact_fields != expected:
                for field in pair:
                    entry.pop(field, None)
        core_fields = ANDROID_SIGNED_EVIDENCE_SUMMARY_CORE_FIELDS & set(entry)
        if core_fields and core_fields != ANDROID_SIGNED_EVIDENCE_SUMMARY_CORE_FIELDS:
            for field in ANDROID_SIGNED_EVIDENCE_SUMMARY_CORE_FIELDS:
                entry.pop(field, None)
        identity_fields = ANDROID_SIGNED_EVIDENCE_SUMMARY_IDENTITY_FIELDS & set(entry)
        if identity_fields and identity_fields != ANDROID_SIGNED_EVIDENCE_SUMMARY_IDENTITY_FIELDS:
            for field in ANDROID_SIGNED_EVIDENCE_SUMMARY_IDENTITY_FIELDS:
                entry.pop(field, None)
        elif identity_fields == ANDROID_SIGNED_EVIDENCE_SUMMARY_IDENTITY_FIELDS:
            inferred_family = device_lab.infer_kagemusha_device_family(
                entry["device_model"],
                entry["device_codename"],
            )
            if inferred_family != entry["device_family"]:
                for field in ANDROID_SIGNED_EVIDENCE_SUMMARY_IDENTITY_FIELDS:
                    entry.pop(field, None)
        if set(entry) != ANDROID_SIGNED_EVIDENCE_SUMMARY_TARGET_FIELDS:
            continue
        if entry:
            if slot not in signed_evidence:
                signed_evidence[slot] = entry
    return signed_evidence


def _android_safe_slot_id(report: dict[str, Any]) -> str | None:
    """Return a release-safe slot id for Android summary cross-checks."""

    slot = report.get("slot")
    if not isinstance(slot, str):
        return None
    slot_ids, slot_errors = device_lab.validate_slot_ids([slot])
    if slot_errors or slot_ids != [slot]:
        return None
    return slot


def _android_report_has_complete_signed_evidence(
    report: dict[str, Any],
    signed_evidence: dict[str, dict[str, str]],
) -> bool:
    """Return true when this report matches its admitted signed-evidence entry."""

    if report.get("status") != "ok":
        return False
    slot = _android_safe_slot_id(report)
    if slot is None:
        return False
    summary_entry = signed_evidence.get(slot)
    if not isinstance(summary_entry, dict):
        return False
    kagemusha = _android_report_kagemusha(report)
    for source_key, target_key in ANDROID_SIGNED_EVIDENCE_SUMMARY_FIELDS:
        if kagemusha.get(source_key) != summary_entry.get(target_key):
            return False
    return True


def _android_slot_reports_summary(
    reports: list[dict[str, Any]],
    signed_evidence: dict[str, dict[str, str]],
) -> list[dict[str, Any]]:
    """Return slot reports without partial release-facing Kagemusha claims."""

    summaries: list[dict[str, Any]] = []
    for report in reports:
        summary = dict(report)
        if report.get("status") == "ok":
            if not _android_report_has_complete_signed_evidence(report, signed_evidence):
                kagemusha = summary.get("kagemusha")
                if isinstance(kagemusha, dict):
                    pruned_kagemusha = dict(kagemusha)
                    for field in ANDROID_SLOT_RELEASE_KAGEMUSHA_FIELDS:
                        pruned_kagemusha.pop(field, None)
                    summary["kagemusha"] = pruned_kagemusha
                else:
                    summary["kagemusha"] = {}
        summaries.append(summary)
    return summaries


def _android_duplicate_matrix_bindings_summary(
    reports: list[dict[str, Any]],
    signed_evidence: dict[str, dict[str, str]],
) -> dict[str, list[dict[str, Any]]]:
    """Return duplicate physical-device bindings only for admitted slot reports."""

    return device_lab.kagemusha_duplicate_matrix_bindings(
        [
            report
            for report in reports
            if _android_report_has_complete_signed_evidence(report, signed_evidence)
        ]
    )


def _safe_trusted_signer_public_key_sha256(
    trusted_signer_public_keys: Mapping[Any, Any] | None,
) -> list[str]:
    """Return only canonical trusted signer ids safe for readiness summaries."""

    return sorted(
        device_lab._trusted_signer_public_key_sha256_set(  # type: ignore[attr-defined]
            trusted_signer_public_keys
        )
    )


def _device_lab_root_list(root: Path | Iterable[Path]) -> list[Path]:
    """Normalize one or more Android device-lab roots."""

    if isinstance(root, (str, os.PathLike)):
        return [Path(root)]
    return [Path(item) for item in root]


def check_android_device_lab(
    root: Path | Iterable[Path],
    trusted_signer_public_keys: dict[str, Path],
    *,
    slot_ids: Iterable[str] | None = None,
    min_signed_at: dt.datetime | None = None,
    max_signed_at: dt.datetime | None = None,
) -> dict[str, Any]:
    """Check strict Android signed evidence and standard family coverage."""

    blockers: list[dict[str, Any]] = []
    roots = _device_lab_root_list(root)
    trusted_signer_public_key_sha256 = _safe_trusted_signer_public_key_sha256(
        trusted_signer_public_keys
    )

    def empty_summary(summary_blockers: list[dict[str, Any]]) -> dict[str, Any]:
        return {
            "ok": False,
            "root": ANDROID_DEVICE_LAB_ROOT_SUMMARY_LABEL,
            "slots": [],
            "covered_device_families": [],
            "missing_device_families": list(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES),
            "covered_d2d_payment_transports": [],
            "missing_d2d_payment_transports": list(ANDROID_REQUIRED_D2D_PAYMENT_TRANSPORTS),
            "covered_d2d_payment_transports_by_family": {
                family: []
                for family in device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES
            },
            "missing_d2d_payment_transport_pairs": _missing_android_d2d_payment_transport_pairs({}),
            "duplicate_bindings": {},
            "signed_evidence": {},
            "min_signed_at_utc": (
                min_signed_at.isoformat().replace("+00:00", "Z")
                if min_signed_at is not None
                else None
            ),
            "max_signed_at_utc": (
                max_signed_at.isoformat().replace("+00:00", "Z")
                if max_signed_at is not None
                else None
            ),
            "trusted_signer_public_key_sha256": trusted_signer_public_key_sha256,
            "blockers": summary_blockers,
        }

    signer_map_blockers = [
        blocker("android_trusted_signer_invalid", error)
        for error in device_lab.validate_trusted_signer_public_key_map(
            trusted_signer_public_keys
        )
    ]
    validated_slot_ids, slot_id_errors = device_lab.validate_slot_ids(slot_ids)
    slot_id_blockers = [
        blocker("android_device_lab_slot_id_invalid", error) for error in slot_id_errors
    ]
    if signer_map_blockers or slot_id_blockers:
        return empty_summary([*signer_map_blockers, *slot_id_blockers])
    blockers.extend(slot_id_blockers)
    existing_roots: list[tuple[int, Path]] = []
    root_blockers: list[dict[str, Any]] = []
    for root_index, candidate_root in enumerate(roots):
        root_exists, root_errors = device_lab.classify_device_lab_root_path(
            candidate_root
        )
        if root_errors:
            for error in root_errors:
                item = blocker("android_device_lab_root_invalid", error)
                if len(roots) > 1:
                    item["root_index"] = root_index
                root_blockers.append(item)
            continue
        if not root_exists:
            item = blocker(
                "android_device_lab_root_missing",
                "Android device-lab root is missing",
            )
            if len(roots) > 1:
                item["root_index"] = root_index
            root_blockers.append(item)
            continue
        existing_roots.append((root_index, candidate_root))
    if not existing_roots:
        return empty_summary([*root_blockers, *slot_id_blockers])
    blockers.extend(root_blockers)
    if not trusted_signer_public_keys:
        blockers.append(
            blocker(
                "android_trusted_signer_missing",
                "trusted signer public key is required for Kagemusha production evidence",
            )
        )
        missing_device_families = list(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES)
        missing_d2d_payment_transports = list(ANDROID_REQUIRED_D2D_PAYMENT_TRANSPORTS)
        covered_d2d_payment_transports_by_family = {
            family: [] for family in device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES
        }
        missing_d2d_payment_transport_pairs = (
            _missing_android_d2d_payment_transport_pairs(
                covered_d2d_payment_transports_by_family,
            )
        )
        blockers.append(
            blocker(
                "android_device_lab_standard_matrix_missing",
                "missing Kagemusha production evidence for one or more Android device families",
                missing_device_families=missing_device_families,
            )
        )
        blockers.append(
            blocker(
                "android_device_lab_d2d_transport_matrix_missing",
                "missing Kagemusha production evidence for one or more offline D2D payment transports",
                missing_d2d_payment_transports=missing_d2d_payment_transports,
                missing_d2d_payment_transport_pairs=missing_d2d_payment_transport_pairs,
            )
        )
        return {
            "ok": False,
            "root": ANDROID_DEVICE_LAB_ROOT_SUMMARY_LABEL,
            "slots": [],
            "covered_device_families": [],
            "missing_device_families": missing_device_families,
            "covered_d2d_payment_transports": [],
            "missing_d2d_payment_transports": missing_d2d_payment_transports,
            "covered_d2d_payment_transports_by_family": covered_d2d_payment_transports_by_family,
            "missing_d2d_payment_transport_pairs": missing_d2d_payment_transport_pairs,
            "duplicate_bindings": {},
            "signed_evidence": {},
            "min_signed_at_utc": (
                min_signed_at.isoformat().replace("+00:00", "Z")
                if min_signed_at is not None
                else None
            ),
            "max_signed_at_utc": (
                max_signed_at.isoformat().replace("+00:00", "Z")
                if max_signed_at is not None
                else None
            ),
            "trusted_signer_public_key_sha256": trusted_signer_public_key_sha256,
            "blockers": blockers,
        }

    android_raw_reports: list[dict[str, Any]] = []
    for root_index, candidate_root in existing_roots:
        raw_reports, discovery_blockers = _slot_reports(
            candidate_root, trusted_signer_public_keys, validated_slot_ids
        )
        if len(roots) > 1:
            for item in discovery_blockers:
                item["root_index"] = root_index
        blockers.extend(discovery_blockers)
        android_raw_reports.extend(raw_reports)
    reports, report_secret_blockers = _sanitize_android_reports(android_raw_reports)
    reports.sort(
        key=lambda report: (
            report.get("slot") if isinstance(report.get("slot"), str) else ""
        )
    )
    blockers.extend(report_secret_blockers)
    if not reports:
        blockers.append(
            blocker("android_device_lab_slots_missing", "no Android device-lab slots found")
        )

    for report in reports:
        if report.get("status") != "ok":
            blockers.append(
                blocker(
                    "android_device_lab_slot_invalid",
                    f"Android device-lab slot {report.get('slot')} is invalid",
                    slot=report.get("slot"),
                    errors=_android_report_errors(report),
                )
            )

    trusted_signer_public_key_sha256_set = frozenset(trusted_signer_public_key_sha256)
    signed_evidence = _android_signed_evidence_summary(
        reports,
        trusted_signer_public_key_sha256_set,
    )
    covered = sorted(
        {
            family
            for report in reports
            for family in [_android_report_device_family(report)]
            if report.get("status") == "ok"
            and _android_report_has_complete_signed_evidence(report, signed_evidence)
            and family is not None
        }
    )
    missing = [
        family
        for family in device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES
        if family not in covered
    ]
    covered_transports = sorted(
        {
            transport
            for report in reports
            for transport in _android_report_d2d_payment_transports(report)
            if report.get("status") == "ok"
            and _android_report_has_complete_signed_evidence(report, signed_evidence)
        }
    )
    missing_transports = [
        transport
        for transport in ANDROID_REQUIRED_D2D_PAYMENT_TRANSPORTS
        if transport not in covered_transports
    ]
    covered_transports_by_family = _android_d2d_payment_transport_coverage_by_family(
        reports,
        signed_evidence,
    )
    missing_transport_pairs = _missing_android_d2d_payment_transport_pairs(
        covered_transports_by_family,
    )
    if missing:
        blockers.append(
            blocker(
                "android_device_lab_standard_matrix_missing",
                "missing Kagemusha production evidence for one or more Android device families",
                missing_device_families=missing,
            )
        )
    if missing_transports or missing_transport_pairs:
        blockers.append(
            blocker(
                "android_device_lab_d2d_transport_matrix_missing",
                "missing Kagemusha production evidence for one or more standard-family offline D2D payment transports",
                missing_d2d_payment_transports=missing_transports,
                missing_d2d_payment_transport_pairs=missing_transport_pairs,
            )
        )
    blockers.extend(_check_android_matrix_unique_bindings(reports))
    blockers.extend(
        _check_android_signed_evidence_summary_values(
            reports,
            trusted_signer_public_key_sha256_set,
        )
    )
    if min_signed_at is not None or max_signed_at is not None:
        blockers.extend(
            _check_android_signed_evidence_freshness(
                reports,
                min_signed_at,
                max_signed_at,
            )
        )

    return {
        "ok": not blockers,
        "root": ANDROID_DEVICE_LAB_ROOT_SUMMARY_LABEL,
        "slots": _android_slot_reports_summary(reports, signed_evidence),
        "covered_device_families": covered,
        "missing_device_families": missing,
        "covered_d2d_payment_transports": covered_transports,
        "missing_d2d_payment_transports": missing_transports,
        "covered_d2d_payment_transports_by_family": covered_transports_by_family,
        "missing_d2d_payment_transport_pairs": missing_transport_pairs,
        "duplicate_bindings": _android_duplicate_matrix_bindings_summary(
            reports,
            signed_evidence,
        ),
        "signed_evidence": signed_evidence,
        "min_signed_at_utc": (
            min_signed_at.isoformat().replace("+00:00", "Z")
            if min_signed_at is not None
            else None
        ),
        "max_signed_at_utc": (
            max_signed_at.isoformat().replace("+00:00", "Z")
            if max_signed_at is not None
            else None
        ),
        "trusted_signer_public_key_sha256": trusted_signer_public_key_sha256,
        "blockers": blockers,
    }


def build_summary(
    *,
    repo_root: Path,
    device_lab_root: Path | Iterable[Path],
    lineage_proof_evidence_path: Path,
    trusted_signer_public_keys: dict[str, Path],
    compact_key_evidence_path: Path | None = None,
    localnet_lifecycle_evidence_path: Path | None = None,
    slot_ids: Iterable[str] | None = None,
    min_signed_at: dt.datetime | None = None,
    max_signed_at: dt.datetime | None = None,
    min_lineage_proof_evidence_at: dt.datetime | None = None,
    max_lineage_proof_evidence_at: dt.datetime | None = None,
    min_compact_key_evidence_at: dt.datetime | None = None,
    max_compact_key_evidence_at: dt.datetime | None = None,
    min_localnet_lifecycle_evidence_at: dt.datetime | None = None,
    max_localnet_lifecycle_evidence_at: dt.datetime | None = None,
) -> dict[str, Any]:
    """Build a complete Kagemusha readiness rollup."""

    if compact_key_evidence_path is None:
        compact_key_evidence_path = (
            lineage_proof_evidence_path.parent / COMPACT_KEY_EVIDENCE_FILENAME
        )
    if localnet_lifecycle_evidence_path is None:
        localnet_lifecycle_evidence_path = (
            lineage_proof_evidence_path.parent / LOCALNET_LIFECYCLE_EVIDENCE_FILENAME
        )
    abi6 = check_abi6_reserved_lineage(repo_root)
    abi7 = check_abi7_fail_closed(repo_root)
    lineage = check_lineage_key_release_tooling(repo_root)
    lineage_proof = check_lineage_proof_evidence(
        lineage_proof_evidence_path,
        min_generated_at=min_lineage_proof_evidence_at,
        max_generated_at=max_lineage_proof_evidence_at,
    )
    compact_key = check_compact_key_evidence(
        compact_key_evidence_path,
        min_generated_at=min_compact_key_evidence_at,
        max_generated_at=max_compact_key_evidence_at,
    )
    localnet_lifecycle = check_localnet_lifecycle_evidence(
        localnet_lifecycle_evidence_path,
        min_generated_at=min_localnet_lifecycle_evidence_at,
        max_generated_at=max_localnet_lifecycle_evidence_at,
    )
    android = check_android_device_lab(
        device_lab_root,
        trusted_signer_public_keys,
        slot_ids=slot_ids,
        min_signed_at=min_signed_at,
        max_signed_at=max_signed_at,
    )
    all_blockers = [
        *abi6["blockers"],
        *abi7["blockers"],
        *lineage["blockers"],
        *lineage_proof["blockers"],
        *compact_key["blockers"],
        *localnet_lifecycle["blockers"],
        *android["blockers"],
    ]
    return {
        "schema": SUMMARY_SCHEMA,
        "generated_at": utc_now(),
        "status": "ready" if not all_blockers else "blocked",
        "ready": not all_blockers,
        "blockers": all_blockers,
        "abi6_reserved_lineage": abi6,
        "abi7_recursive_compact": abi7,
        "lineage_key_release_tooling": lineage,
        "lineage_proof_evidence": lineage_proof,
        "compact_key_evidence": compact_key,
        "localnet_lifecycle_evidence": localnet_lifecycle,
        "android_device_lab": android,
    }


def validate_summary_output_path(path: Path) -> list[dict[str, Any]]:
    """Reject readiness summary output paths that could alias external files."""

    secret_blocker = _secret_looking_path_blocker(
        str(path),
        label="--summary-out",
        code=SUMMARY_OUT_PATH_INVALID_CODE,
    )
    if secret_blocker is not None:
        return [secret_blocker]
    shape_blocker = _cli_path_shape_blocker(
        str(path),
        label="--summary-out",
        code=SUMMARY_OUT_PATH_INVALID_CODE,
    )
    if shape_blocker is not None:
        return [shape_blocker]
    parent = path.parent
    parent_exists, parent_blockers = _validate_summary_output_parent(path)
    if parent_blockers:
        return parent_blockers
    ancestor_errors = device_lab.validate_no_symlink_ancestors(
        path,
        "--summary-out ancestor directory",
    )
    if ancestor_errors:
        return [
            blocker(SUMMARY_OUT_PATH_INVALID_CODE, error)
            for error in ancestor_errors
        ]
    if not parent_exists:
        try:
            parent.mkdir(parents=True, exist_ok=True)
        except OSError:
            return [
                blocker(
                    SUMMARY_OUT_PATH_INVALID_CODE,
                    "--summary-out parent directory could not be created",
                )
            ]
    parent_exists, parent_blockers = _validate_summary_output_parent(
        path,
        missing_message="--summary-out parent must be a directory",
    )
    if parent_blockers:
        return parent_blockers
    if not parent_exists:
        return [
            blocker(
                SUMMARY_OUT_PATH_INVALID_CODE,
                "--summary-out parent must be a directory",
            )
        ]
    ancestor_errors = device_lab.validate_no_symlink_ancestors(
        path,
        "--summary-out ancestor directory",
    )
    if ancestor_errors:
        return [
            blocker(SUMMARY_OUT_PATH_INVALID_CODE, error)
            for error in ancestor_errors
        ]
    try:
        summary_output_mode = path.lstat().st_mode
    except FileNotFoundError:
        return []
    except OSError:
        return [
            blocker(
                SUMMARY_OUT_PATH_INVALID_CODE,
                "--summary-out file metadata could not be read",
            )
        ]
    if stat.S_ISLNK(summary_output_mode):
        return [
            blocker(
                SUMMARY_OUT_PATH_INVALID_CODE,
                "--summary-out must not be a symlink",
            )
        ]
    if not stat.S_ISREG(summary_output_mode):
        return [
            blocker(
                SUMMARY_OUT_PATH_INVALID_CODE,
                "--summary-out must be a regular file",
            )
        ]
    try:
        link_count = path.stat().st_nlink
    except OSError:
        return [
            blocker(
                SUMMARY_OUT_PATH_INVALID_CODE,
                "--summary-out hardlink metadata could not be read",
            )
        ]
    if link_count > 1:
        return [
            blocker(
                SUMMARY_OUT_PATH_INVALID_CODE,
                "--summary-out must not be hardlinked",
            )
        ]
    return []


def _validate_summary_output_parent(
    path: Path,
    *,
    missing_message: str | None = None,
) -> tuple[bool, list[dict[str, Any]]]:
    """Classify the readiness summary output parent without following aliases."""

    parent = path.parent
    try:
        parent_mode = parent.lstat().st_mode
    except FileNotFoundError:
        if missing_message is None:
            return False, []
        return False, [blocker(SUMMARY_OUT_PATH_INVALID_CODE, missing_message)]
    except OSError:
        return False, [
            blocker(
                SUMMARY_OUT_PATH_INVALID_CODE,
                "--summary-out parent directory metadata could not be read",
            )
        ]
    if stat.S_ISLNK(parent_mode):
        return True, [
            blocker(
                SUMMARY_OUT_PATH_INVALID_CODE,
                "--summary-out parent directory must not be a symlink",
            )
        ]
    if not stat.S_ISDIR(parent_mode):
        return True, [
            blocker(
                SUMMARY_OUT_PATH_INVALID_CODE,
                "--summary-out parent must be a directory",
            )
        ]
    return True, []


def _summary_out_blocker(message: str) -> dict[str, Any]:
    return blocker(SUMMARY_OUT_PATH_INVALID_CODE, message)


def _read_summary_output_text(
    path: Path,
    expected_stat: os.stat_result,
) -> tuple[str | None, list[dict[str, Any]]]:
    """Read readiness summary output text without trusting a stale path."""

    chunks: list[bytes] = []
    summary_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            if stat.S_ISLNK(path_stat.st_mode):
                return None, [_summary_out_blocker("--summary-out must not be a symlink")]
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(
                open_stat.st_mode
            ):
                return None, [_summary_out_blocker("--summary-out must be a regular file")]
            summary_open_identity = (open_stat.st_dev, open_stat.st_ino)
            if summary_open_identity != summary_expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != summary_expected_identity:
                return None, [
                    _summary_out_blocker("--summary-out changed while being read")
                ]
            if open_stat.st_nlink > 1:
                return None, [_summary_out_blocker("--summary-out must not be hardlinked")]
            if stat.S_IMODE(open_stat.st_mode) != 0o600:
                return None, [_summary_out_blocker("--summary-out permissions must be 0600")]
            if open_stat.st_size > MAX_READINESS_SUMMARY_JSON_BYTES:
                return None, [
                    _summary_out_blocker(
                        f"--summary-out must be no more than {MAX_READINESS_SUMMARY_JSON_BYTES} bytes"
                    )
                ]
            size = 0
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if size > MAX_READINESS_SUMMARY_JSON_BYTES:
                    return None, [
                        _summary_out_blocker(
                            f"--summary-out must be no more than {MAX_READINESS_SUMMARY_JSON_BYTES} bytes"
                        )
                    ]
                chunks.append(chunk)
            final_path_stat = path.lstat()
            if (
                final_path_stat.st_dev,
                final_path_stat.st_ino,
            ) != summary_expected_identity:
                return None, [
                    _summary_out_blocker("--summary-out changed while being read")
                ]
    except OSError:
        return None, [
            _summary_out_blocker("--summary-out write verification failed")
        ]
    try:
        return b"".join(chunks).decode("utf-8"), []
    except UnicodeDecodeError:
        return None, [
            _summary_out_blocker("--summary-out write verification failed")
        ]


def _cleanup_summary_output(
    path: Path,
    expected_identity: tuple[int, int] | None,
) -> list[dict[str, Any]]:
    if expected_identity is None:
        return [
            _summary_out_blocker("--summary-out temporary file metadata could not be read")
        ]
    try:
        parent_fd = os.open(path.parent, _directory_open_flags())
    except OSError:
        return [
            _summary_out_blocker("--summary-out temporary file could not be removed")
        ]
    try:
        try:
            temp_stat = os.stat(
                path.name,
                dir_fd=parent_fd,
                follow_symlinks=False,
            )
        except FileNotFoundError:
            return []
        except OSError:
            return [
                _summary_out_blocker(
                    "--summary-out temporary file could not be removed"
                )
            ]
        if (
            not stat.S_ISREG(temp_stat.st_mode)
            or _file_identity(temp_stat) != expected_identity
        ):
            return [
                _summary_out_blocker(
                    "--summary-out temporary file changed before cleanup"
                )
            ]
        try:
            os.unlink(path.name, dir_fd=parent_fd)
        except FileNotFoundError:
            return []
        except OSError:
            return [
                _summary_out_blocker(
                    "--summary-out temporary file could not be removed"
                )
            ]
        try:
            os.fsync(parent_fd)
        except OSError:
            return [
                _summary_out_blocker(
                    "--summary-out temporary file cleanup could not be synced"
                )
            ]
    finally:
        os.close(parent_fd)
    return []


def _file_identity(file_stat: os.stat_result) -> tuple[int, int]:
    return file_stat.st_dev, file_stat.st_ino


def _directory_open_flags() -> int:
    flags = os.O_RDONLY
    if hasattr(os, "O_DIRECTORY"):
        flags |= os.O_DIRECTORY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    return flags


def _sync_summary_output_parent(
    parent: Path,
    *,
    expected_identity: tuple[int, int] | None,
) -> list[dict[str, Any]]:
    try:
        parent_fd = os.open(parent, _directory_open_flags())
    except OSError:
        return [_summary_out_blocker("--summary-out parent directory could not be synced")]
    try:
        return _sync_summary_output_parent_fd(
            parent_fd,
            expected_identity=expected_identity,
        )
    finally:
        os.close(parent_fd)


def _sync_summary_output_parent_fd(
    parent_fd: int,
    *,
    expected_identity: tuple[int, int] | None,
) -> list[dict[str, Any]]:
    try:
        parent_stat = os.fstat(parent_fd)
        if not stat.S_ISDIR(parent_stat.st_mode):
            return [_summary_out_blocker("--summary-out parent directory could not be synced")]
        if expected_identity is not None and _file_identity(parent_stat) != expected_identity:
            return [_summary_out_blocker("--summary-out parent directory changed before sync")]
        os.fsync(parent_fd)
    except OSError:
        return [_summary_out_blocker("--summary-out parent directory could not be synced")]
    return []


def write_summary(path: Path, summary: dict[str, Any]) -> list[dict[str, Any]]:
    """Write a readiness summary JSON file."""

    errors = validate_summary_output_path(path)
    if errors:
        return errors
    parent_exists, parent_errors = _validate_summary_output_parent(
        path,
        missing_message="--summary-out parent must be a directory",
    )
    if parent_errors:
        return parent_errors
    if not parent_exists:
        return [_summary_out_blocker("--summary-out parent must be a directory")]
    try:
        parent_identity = _file_identity(path.parent.lstat())
    except OSError:
        return [_summary_out_blocker("--summary-out parent directory metadata could not be read")]
    try:
        summary_text = json.dumps(
            summary,
            indent=2,
            sort_keys=True,
            allow_nan=False,
        ) + "\n"
    except ValueError:
        return [
            blocker(
                SUMMARY_OUT_PATH_INVALID_CODE,
                "--summary-out summary is not strict JSON",
            )
        ]
    if len(summary_text.encode("utf-8")) > MAX_READINESS_SUMMARY_JSON_BYTES:
        return [
            _summary_out_blocker(
                f"--summary-out must be no more than {MAX_READINESS_SUMMARY_JSON_BYTES} bytes"
            )
        ]
    try:
        parent_fd = os.open(path.parent, _directory_open_flags())
    except OSError:
        return [_summary_out_blocker("--summary-out parent directory metadata could not be read")]
    try:
        try:
            opened_parent_stat = os.fstat(parent_fd)
        except OSError:
            return [_summary_out_blocker("--summary-out parent directory metadata could not be read")]
        if (
            not stat.S_ISDIR(opened_parent_stat.st_mode)
            or _file_identity(opened_parent_stat) != parent_identity
        ):
            return [_summary_out_blocker("--summary-out parent directory changed before sync")]
        return _write_summary_with_parent_fd(
            path,
            summary_text,
            parent_fd=parent_fd,
            parent_identity=parent_identity,
        )
    finally:
        os.close(parent_fd)


def _write_summary_with_parent_fd(
    path: Path,
    summary_text: str,
    *,
    parent_fd: int,
    parent_identity: tuple[int, int],
) -> list[dict[str, Any]]:
    tmp_path: Path | None = None
    tmp_identity: tuple[int, int] | None = None
    write_blockers: list[dict[str, Any]] = []
    try:
        with tempfile.NamedTemporaryFile(
            "w",
            dir=path.parent,
            encoding="utf-8",
            prefix=f".{path.name}.",
            suffix=".tmp",
            delete=False,
        ) as handle:
            tmp_path = Path(handle.name)
            os.fchmod(handle.fileno(), 0o600)
            tmp_identity = _file_identity(os.fstat(handle.fileno()))
            handle.write(summary_text)
            handle.flush()
            os.fsync(handle.fileno())
        errors = validate_summary_output_path(path)
        if errors:
            write_blockers.extend(errors)
        else:
            os.replace(
                tmp_path.name,
                path.name,
                src_dir_fd=parent_fd,
                dst_dir_fd=parent_fd,
            )
            tmp_path = None
    except OSError:
        write_blockers.append(
            blocker(
                SUMMARY_OUT_PATH_INVALID_CODE,
                "--summary-out could not be written",
            )
        )
    finally:
        if tmp_path is not None:
            write_blockers.extend(_cleanup_summary_output(tmp_path, tmp_identity))
    if write_blockers:
        return write_blockers
    try:
        expected_stat = os.stat(path.name, dir_fd=parent_fd, follow_symlinks=False)
    except (FileNotFoundError, OSError):
        return [_summary_out_blocker("--summary-out write verification failed")]
    if stat.S_ISLNK(expected_stat.st_mode):
        return [_summary_out_blocker("--summary-out must not be a symlink")]
    if not stat.S_ISREG(expected_stat.st_mode):
        return [_summary_out_blocker("--summary-out write verification failed")]
    output_identity = _file_identity(expected_stat)
    try:
        current_parent_stat = path.parent.lstat()
    except OSError:
        cleanup_blockers = _unlink_summary_if_identity_at(
            parent_fd,
            path.name,
            output_identity,
        )
        return [
            _summary_out_blocker(
                "--summary-out parent directory metadata could not be read"
            ),
            *cleanup_blockers,
        ]
    if _file_identity(current_parent_stat) != parent_identity:
        cleanup_blockers = _unlink_summary_if_identity_at(
            parent_fd,
            path.name,
            output_identity,
        )
        return [
            _summary_out_blocker("--summary-out parent directory changed before sync"),
            *cleanup_blockers,
        ]
    sync_blockers = _sync_summary_output_parent_fd(
        parent_fd,
        expected_identity=parent_identity,
    )
    if sync_blockers:
        cleanup_blockers = _unlink_summary_if_identity_at(
            parent_fd,
            path.name,
            output_identity,
        )
        return [*sync_blockers, *cleanup_blockers]
    errors = validate_summary_output_path(path)
    if errors:
        return errors
    if stat.S_ISLNK(expected_stat.st_mode):
        return [_summary_out_blocker("--summary-out must not be a symlink")]
    if not stat.S_ISREG(expected_stat.st_mode):
        return [_summary_out_blocker("--summary-out must be a regular file")]
    if expected_stat.st_nlink > 1:
        return [_summary_out_blocker("--summary-out must not be hardlinked")]
    readback_text, readback_errors = _read_summary_output_text(path, expected_stat)
    if readback_errors:
        return readback_errors
    if readback_text != summary_text:
        return [_summary_out_blocker("--summary-out write verification failed")]
    return []


def _unlink_summary_if_identity_at(
    parent_fd: int,
    name: str,
    expected_identity: tuple[int, int],
) -> list[dict[str, Any]]:
    try:
        file_stat = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
    except FileNotFoundError:
        return []
    except OSError:
        return [
            _summary_out_blocker(
                "--summary-out rollback cleanup metadata could not be read"
            )
        ]
    if not stat.S_ISREG(file_stat.st_mode) or _file_identity(file_stat) != expected_identity:
        return []
    try:
        os.unlink(name, dir_fd=parent_fd)
    except FileNotFoundError:
        return []
    except OSError:
        return [
            _summary_out_blocker(
                "--summary-out could not be removed after parent sync failure"
            )
        ]
    try:
        os.fsync(parent_fd)
    except OSError:
        return [
            _summary_out_blocker(
                "--summary-out cleanup could not be synced after parent sync failure"
            )
        ]
    return []


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Roll up strict Kagemusha production-readiness evidence."
    )
    parser.add_argument(
        "--repo-root",
        default=str(SCRIPT_DIR.parent),
        help="Repository root used for checked-in Kagemusha release guards.",
    )
    parser.add_argument(
        "--device-lab-root",
        action="append",
        default=None,
        help=(
            "Android device-lab root containing production slots. Repeat to "
            "aggregate separately captured device-family roots."
        ),
    )
    parser.add_argument(
        "--lineage-proof-evidence",
        default=DEFAULT_LINEAGE_PROOF_EVIDENCE_PATH,
        help="Reserved-lineage production proof/keygen evidence JSON.",
    )
    parser.add_argument(
        "--compact-key-evidence",
        default=None,
        help=(
            "ABI-7 recursive compact key-artifact evidence JSON. Defaults to "
            f"{COMPACT_KEY_EVIDENCE_FILENAME} beside --lineage-proof-evidence."
        ),
    )
    parser.add_argument(
        "--localnet-lifecycle-evidence",
        default=None,
        help=(
            "Kagemusha 4-peer localnet lifecycle evidence JSON. Defaults to "
            f"{LOCALNET_LIFECYCLE_EVIDENCE_FILENAME} beside --lineage-proof-evidence."
        ),
    )
    parser.add_argument(
        "--slot",
        action="append",
        dest="slots",
        default=None,
        help="Specific Android device-lab slot id(s) to include.",
    )
    parser.add_argument(
        "--trusted-signer-public-key",
        action="append",
        dest="trusted_signer_public_keys",
        default=None,
        help="PEM public key for a trusted Android lab evidence signer.",
    )
    parser.add_argument(
        "--min-signed-at-utc",
        default=DEFAULT_MIN_SIGNED_AT_UTC,
        help=(
            "Minimum signed_at_utc timestamp accepted for Android lab evidence. "
            "Use an empty value to disable the freshness gate."
        ),
    )
    parser.add_argument(
        "--max-signed-at-future-skew-seconds",
        type=int,
        default=DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS,
        help=(
            "Maximum number of seconds signed_at_utc may be ahead of the "
            "readiness validator clock."
        ),
    )
    parser.add_argument(
        "--min-lineage-proof-evidence-at-utc",
        default=DEFAULT_MIN_SIGNED_AT_UTC,
        help=(
            "Minimum generated_at_utc timestamp accepted for Reserved-lineage proof evidence. "
            "Use an empty value to disable the freshness gate."
        ),
    )
    parser.add_argument(
        "--max-lineage-proof-evidence-future-skew-seconds",
        type=int,
        default=DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS,
        help=(
            "Maximum number of seconds Reserved-lineage proof evidence generated_at_utc "
            "may be ahead of the readiness validator clock."
        ),
    )
    parser.add_argument(
        "--min-compact-key-evidence-at-utc",
        default=DEFAULT_MIN_SIGNED_AT_UTC,
        help=(
            "Minimum generated_at_utc timestamp accepted for ABI-7 recursive compact "
            "key evidence. Use an empty value to disable the freshness gate."
        ),
    )
    parser.add_argument(
        "--max-compact-key-evidence-future-skew-seconds",
        type=int,
        default=DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS,
        help=(
            "Maximum number of seconds ABI-7 recursive compact key evidence "
            "generated_at_utc may be ahead of the readiness validator clock."
        ),
    )
    parser.add_argument(
        "--min-localnet-lifecycle-evidence-at-utc",
        default=DEFAULT_MIN_SIGNED_AT_UTC,
        help=(
            "Minimum generated_at_utc timestamp accepted for Kagemusha localnet "
            "lifecycle evidence. Use an empty value to disable the freshness gate."
        ),
    )
    parser.add_argument(
        "--max-localnet-lifecycle-evidence-future-skew-seconds",
        type=int,
        default=DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS,
        help=(
            "Maximum number of seconds Kagemusha localnet lifecycle evidence "
            "generated_at_utc may be ahead of the readiness validator clock."
        ),
    )
    parser.add_argument("--summary-out", default=None, help="Optional JSON summary path.")
    args = parser.parse_args(argv)

    path_blockers = validate_cli_path_arguments(args)
    repo_root: Path | None = None
    if not path_blockers:
        try:
            repo_root = Path(args.repo_root).resolve()
        except OSError:
            path_blockers.append(
                blocker(
                    "kagemusha_repo_root_path_invalid",
                    "--repo-root could not be resolved",
                )
            )
    if path_blockers:
        summary = {
            "schema": SUMMARY_SCHEMA,
            "generated_at": utc_now(),
            "status": "blocked",
            "ready": False,
            "blockers": path_blockers,
        }
    else:
        assert repo_root is not None
        device_lab_roots: list[Path] = []
        for raw_device_lab_root in _device_lab_root_arg_values(args):
            device_lab_root = Path(raw_device_lab_root)
            if not device_lab_root.is_absolute():
                device_lab_root = repo_root / device_lab_root
            device_lab_roots.append(device_lab_root)
        lineage_proof_evidence_path = Path(args.lineage_proof_evidence)
        if not lineage_proof_evidence_path.is_absolute():
            lineage_proof_evidence_path = repo_root / lineage_proof_evidence_path
        if args.compact_key_evidence:
            compact_key_evidence_path = Path(args.compact_key_evidence)
            if not compact_key_evidence_path.is_absolute():
                compact_key_evidence_path = repo_root / compact_key_evidence_path
        else:
            compact_key_evidence_path = (
                lineage_proof_evidence_path.parent / COMPACT_KEY_EVIDENCE_FILENAME
            )
        if args.localnet_lifecycle_evidence:
            localnet_lifecycle_evidence_path = Path(args.localnet_lifecycle_evidence)
            if not localnet_lifecycle_evidence_path.is_absolute():
                localnet_lifecycle_evidence_path = repo_root / localnet_lifecycle_evidence_path
        else:
            localnet_lifecycle_evidence_path = (
                lineage_proof_evidence_path.parent / LOCALNET_LIFECYCLE_EVIDENCE_FILENAME
            )
        trusted, signer_errors = device_lab.load_trusted_signer_public_keys(
            args.trusted_signer_public_keys
        )
        min_signed_at = None
        if args.min_signed_at_utc:
            min_signed_at, min_signed_at_blocker = parse_utc_timestamp(
                args.min_signed_at_utc,
                "--min-signed-at-utc",
            )
        else:
            min_signed_at_blocker = None
        min_lineage_proof_evidence_at = None
        if args.min_lineage_proof_evidence_at_utc:
            (
                min_lineage_proof_evidence_at,
                min_lineage_proof_evidence_at_blocker,
            ) = parse_utc_timestamp(
                args.min_lineage_proof_evidence_at_utc,
                "--min-lineage-proof-evidence-at-utc",
            )
        else:
            min_lineage_proof_evidence_at_blocker = None
        min_compact_key_evidence_at = None
        if args.min_compact_key_evidence_at_utc:
            (
                min_compact_key_evidence_at,
                min_compact_key_evidence_at_blocker,
            ) = parse_utc_timestamp(
                args.min_compact_key_evidence_at_utc,
                "--min-compact-key-evidence-at-utc",
            )
        else:
            min_compact_key_evidence_at_blocker = None
        min_localnet_lifecycle_evidence_at = None
        if args.min_localnet_lifecycle_evidence_at_utc:
            (
                min_localnet_lifecycle_evidence_at,
                min_localnet_lifecycle_evidence_at_blocker,
            ) = parse_utc_timestamp(
                args.min_localnet_lifecycle_evidence_at_utc,
                "--min-localnet-lifecycle-evidence-at-utc",
            )
        else:
            min_localnet_lifecycle_evidence_at_blocker = None
        max_signed_at = None
        max_signed_at_blocker = None
        if args.max_signed_at_future_skew_seconds < 0:
            max_signed_at_blocker = blocker(
                "android_max_signed_at_invalid",
                "--max-signed-at-future-skew-seconds must be non-negative",
            )
        else:
            max_signed_at = (
                dt.datetime.now(dt.timezone.utc).replace(microsecond=0)
                + dt.timedelta(seconds=args.max_signed_at_future_skew_seconds)
            )
        max_lineage_proof_evidence_at = None
        max_lineage_proof_evidence_at_blocker = None
        if args.max_lineage_proof_evidence_future_skew_seconds < 0:
            max_lineage_proof_evidence_at_blocker = blocker(
                "lineage_proof_evidence_max_timestamp_invalid",
                "--max-lineage-proof-evidence-future-skew-seconds must be non-negative",
            )
        else:
            max_lineage_proof_evidence_at = (
                dt.datetime.now(dt.timezone.utc).replace(microsecond=0)
                + dt.timedelta(seconds=args.max_lineage_proof_evidence_future_skew_seconds)
            )
        max_compact_key_evidence_at = None
        max_compact_key_evidence_at_blocker = None
        if args.max_compact_key_evidence_future_skew_seconds < 0:
            max_compact_key_evidence_at_blocker = blocker(
                "compact_key_evidence_max_timestamp_invalid",
                "--max-compact-key-evidence-future-skew-seconds must be non-negative",
            )
        else:
            max_compact_key_evidence_at = (
                dt.datetime.now(dt.timezone.utc).replace(microsecond=0)
                + dt.timedelta(seconds=args.max_compact_key_evidence_future_skew_seconds)
            )
        max_localnet_lifecycle_evidence_at = None
        max_localnet_lifecycle_evidence_at_blocker = None
        if args.max_localnet_lifecycle_evidence_future_skew_seconds < 0:
            max_localnet_lifecycle_evidence_at_blocker = blocker(
                "localnet_lifecycle_evidence_max_timestamp_invalid",
                "--max-localnet-lifecycle-evidence-future-skew-seconds must be non-negative",
            )
        else:
            max_localnet_lifecycle_evidence_at = (
                dt.datetime.now(dt.timezone.utc).replace(microsecond=0)
                + dt.timedelta(
                    seconds=args.max_localnet_lifecycle_evidence_future_skew_seconds
                )
            )
        if (
            signer_errors
            or min_signed_at_blocker is not None
            or min_lineage_proof_evidence_at_blocker is not None
            or min_compact_key_evidence_at_blocker is not None
            or min_localnet_lifecycle_evidence_at_blocker is not None
            or max_signed_at_blocker is not None
            or max_lineage_proof_evidence_at_blocker is not None
            or max_compact_key_evidence_at_blocker is not None
            or max_localnet_lifecycle_evidence_at_blocker is not None
        ):
            blockers = [
                blocker("android_trusted_signer_invalid", error) for error in signer_errors
            ]
            if min_signed_at_blocker is not None:
                min_signed_at_blocker["code"] = "android_min_signed_at_invalid"
                blockers.append(min_signed_at_blocker)
            if min_lineage_proof_evidence_at_blocker is not None:
                min_lineage_proof_evidence_at_blocker["code"] = (
                    "lineage_proof_evidence_min_timestamp_invalid"
                )
                blockers.append(min_lineage_proof_evidence_at_blocker)
            if min_compact_key_evidence_at_blocker is not None:
                min_compact_key_evidence_at_blocker["code"] = (
                    "compact_key_evidence_min_timestamp_invalid"
                )
                blockers.append(min_compact_key_evidence_at_blocker)
            if min_localnet_lifecycle_evidence_at_blocker is not None:
                min_localnet_lifecycle_evidence_at_blocker["code"] = (
                    "localnet_lifecycle_evidence_min_timestamp_invalid"
                )
                blockers.append(min_localnet_lifecycle_evidence_at_blocker)
            if max_signed_at_blocker is not None:
                blockers.append(max_signed_at_blocker)
            if max_lineage_proof_evidence_at_blocker is not None:
                blockers.append(max_lineage_proof_evidence_at_blocker)
            if max_compact_key_evidence_at_blocker is not None:
                blockers.append(max_compact_key_evidence_at_blocker)
            if max_localnet_lifecycle_evidence_at_blocker is not None:
                blockers.append(max_localnet_lifecycle_evidence_at_blocker)
            summary = {
                "schema": SUMMARY_SCHEMA,
                "generated_at": utc_now(),
                "status": "blocked",
                "ready": False,
                "blockers": blockers,
            }
        else:
            summary = build_summary(
                repo_root=repo_root,
                device_lab_root=device_lab_roots,
                lineage_proof_evidence_path=lineage_proof_evidence_path,
                trusted_signer_public_keys=trusted,
                compact_key_evidence_path=compact_key_evidence_path,
                localnet_lifecycle_evidence_path=localnet_lifecycle_evidence_path,
                slot_ids=args.slots,
                min_signed_at=min_signed_at,
                max_signed_at=max_signed_at,
                min_lineage_proof_evidence_at=min_lineage_proof_evidence_at,
                max_lineage_proof_evidence_at=max_lineage_proof_evidence_at,
                min_compact_key_evidence_at=min_compact_key_evidence_at,
                max_compact_key_evidence_at=max_compact_key_evidence_at,
                min_localnet_lifecycle_evidence_at=min_localnet_lifecycle_evidence_at,
                max_localnet_lifecycle_evidence_at=max_localnet_lifecycle_evidence_at,
            )

    summary_out_invalid = any(
        item["code"] == SUMMARY_OUT_PATH_INVALID_CODE for item in path_blockers
    )
    if args.summary_out and not summary_out_invalid:
        write_blockers = write_summary(Path(args.summary_out), summary)
        if write_blockers:
            summary["ready"] = False
            summary["status"] = "blocked"
            summary["blockers"].extend(write_blockers)
        else:
            print("[kagemusha-readiness] wrote summary")

    if summary["ready"]:
        print("[kagemusha-readiness] ready")
        return 0
    for item in summary["blockers"]:
        print(
            f"[kagemusha-readiness] blocked: {item['code']}: {item['message']}",
            file=sys.stderr,
        )
    return 1


if __name__ == "__main__":  # pragma: no cover
    sys.exit(main())
