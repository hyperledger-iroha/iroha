#!/usr/bin/env python3
"""Verify the closed Kagemusha V1 release-evidence filesystem projection.

This verifier never executes code selected by the candidate evidence bundle.
It pins every input by descriptor, derives byte/count measurements from the
referenced files, and verifies threshold-signed raw observations against a
separately pinned local verifier/observer policy.  The signed observations bind
the trusted verifier identity, exact arguments, exact stdout/stderr bytes,
resource observations, and the typed report consumed by this projection.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Mapping, NoReturn, Sequence

from release_artifact_contract import (
    ReleaseArtifactError,
    StableFile,
    canonical_json_bytes,
    canonical_relative_path,
    load_json_object,
    stable_hash_relative,
    stable_read_path,
    stable_read_relative,
)


MANIFEST_SCHEMA = "iroha.kagemusha_v1.release_evidence_manifest"
PROJECTION_SCHEMA = "iroha.kagemusha_v1.authority_review_projection"
SCHEMA_VERSION = 1
WIRE_VERSION = 1

MAX_MANIFEST_BYTES = 16 * 1024 * 1024
MAX_REPORT_BYTES = 4 * 1024 * 1024
MAX_EVENT_LOG_BYTES = 64 * 1024 * 1024
MAX_ARTIFACT_BYTES = 64 * 1024 * 1024
MAX_SOURCE_ARCHIVE_BYTES = 4 * 1024 * 1024 * 1024
MAX_TRANSCRIPT_BYTES = 16 * 1024 * 1024
MAX_OBSERVER_POLICY_BYTES = 1024 * 1024
MAX_OBSERVATION_BYTES = 256 * 1024
MAX_EVIDENCE_FILES = 65_536
MAX_EVIDENCE_DIRECTORIES = 4_096
MAX_EVIDENCE_TREE_ENTRIES = 70_000
MAX_EVIDENCE_TREE_DEPTH = 32
MAX_COMMANDS = 8_192
MAX_TOTAL_EVIDENCE_BYTES = 6 * 1024 * 1024 * 1024
MAX_RETAINED_PAYLOAD_BYTES = 128 * 1024 * 1024
MAX_TOTAL_TRANSCRIPT_BYTES = 64 * 1024 * 1024
MAX_TOTAL_COMMAND_INPUT_BYTES = 48 * 1024 * 1024 * 1024
MAX_TOTAL_OBSERVED_DURATION_MS = 24 * 60 * 60 * 1_000
MAX_TOTAL_OBSERVED_CPU_MS = 24 * 60 * 60 * 1_000
MAX_OBSERVATION_DURATION_MS = 60 * 60 * 1_000
MAX_OBSERVATION_CPU_MS = 60 * 60 * 1_000
MAX_OBSERVATION_RSS_BYTES = 4 * 1024 * 1024 * 1024
MAX_JSONL_ROWS = 5_000_000

PAIRED_PROOF_MAX_BYTES = 6_528
# Internal helper proofs are not wire payloads. This artifact-sized resource
# ceiling limits filesystem work only; their admissible lengths come from the
# authenticated compiled-protocol profile and are checked exactly below.
INTERNAL_PROOF_RESOURCE_MAX_BYTES = MAX_ARTIFACT_BYTES
RAW_SESSION_MAX_BYTES = 9_211
TEXT_SESSION_MAX_BYTES = 12_288
PROCESS_RSS_MAX_BYTES = 128 * 1024 * 1024
PROVE_P95_MAX_MS = 10_000
VERIFY_P95_MAX_MS = 1_000
HANDOFF_P95_MAX_MS = 30_000
MIN_FUZZ_CASES = 10_000_000
MIN_AGGREGATED_CREDITS = 1_000
MIN_THERMAL_FOLDED_CREDITS = 1_000
HALO2_K = 16
MAX_CIRCUIT_ROWS = 1 << HALO2_K

ARTIFACT_ROLES = (
    "params_eq",
    "params_ep",
    "state_pk_eq",
    "state_vk_eq",
    "state_pk_ep",
    "state_vk_ep",
    "mint_authorization_pk_eq",
    "mint_authorization_vk_eq",
    "mint_authorization_pk_ep",
    "mint_authorization_vk_ep",
    "mint_credit_pk_eq",
    "mint_credit_vk_eq",
    "mint_credit_pk_ep",
    "mint_credit_vk_ep",
    "platform_credential_pk_eq",
    "platform_credential_vk_eq",
    "platform_credential_pk_ep",
    "platform_credential_vk_ep",
    "guard_bundle_pk_eq",
    "guard_bundle_vk_eq",
    "guard_bundle_pk_ep",
    "guard_bundle_vk_ep",
    "commit_wrapper_pk_eq",
    "commit_wrapper_vk_eq",
    "commit_wrapper_pk_ep",
    "commit_wrapper_vk_ep",
)

RELATIONS = (
    "bootstrap",
    "mint_fold",
    "send_split",
    "receive_fold",
    "redeem_split",
    "rotate",
    "acceptance_intent_authorization",
    "commit_wrapper",
)

HELPERS = (
    "mint_authorization",
    "mint_credit",
    "platform_credential",
    "guard_bundle",
)

INTERNAL_PROOF_HELPERS = frozenset({"platform_credential", "guard_bundle"})

ACCEPTANCE_CASES = (
    "receiver_capacity_exhaustion",
    "sender_outbox_capacity_exhaustion",
    "crash_after_prepare",
    "crash_during_prove",
    "crash_after_candidate_persist",
    "crash_during_hardware_commit",
    "crash_after_hardware_commit",
    "crash_during_commit_wrapper",
    "crash_after_commit_wrapper_generated_before_install",
    "crash_before_exposure",
    "recovery_idempotence",
    "delayed_delivery_across_suite_rotation",
    "clock_rollback",
    "lease_expiry",
    "epoch_and_counter_rollover",
    "suspension_online_recovery",
    "single_exact",
    "invoice_overpayment",
    "partial_until_total",
    "bounded_multi_payment",
    "open_receive_reuse",
    "acceptance_ticket_replay",
    "transcript_unlinkability",
    "reserve_underflow",
    "concurrent_redemption",
    "animated_qr_loss_recovery",
    "animated_qr_reordering_recovery",
    "static_qr_size_guard",
    "four_peer_activation_restart_replay",
    "physical_airplane_mode",
    "physical_restart",
    "physical_power_loss",
    "physical_backup_restore_rejection",
    "physical_memory_and_latency",
    "physical_thermal_folding",
    "no_software_fallback",
    "native_fixture_swift",
    "native_fixture_kotlin",
    "native_fixture_java",
    "native_fixture_java_script",
    "native_fixture_python",
    "native_fixture_c_sharp",
    "native_fixture_jni",
    "native_fixture_qr",
    "native_fixture_nfc",
)

PHYSICAL_PROFILE_CHECKS = (
    "airplane_mode",
    "restart",
    "power_loss",
    "backup_restore_rejection",
    "memory_and_latency",
    "thermal_folding",
    "no_software_fallback",
)

FILE_KINDS = frozenset(
    {
        "artifact",
        "cargo_lock",
        "event_log",
        "internal_proof",
        "proof",
        "raw_session",
        "report",
        "source_archive",
        "text_session",
        "observation",
        "transcript",
    }
)

OBSERVER_POLICY_SCHEMA = "iroha.kagemusha_v1.trusted_observer_policy"
OBSERVATION_SCHEMA = "iroha.kagemusha_v1.verification_observation"
OBSERVER_AUTHORITY_ID_DOMAIN = b"iroha:kagemusha:v1:observer-authority\0"
OBSERVATION_APPROVAL_DOMAIN = b"iroha:kagemusha:v1:observation-approval\0"
CANDIDATE_CONTEXT_DIGEST_DOMAIN = b"iroha:kagemusha:v1:release-candidate-context"
ARTIFACT_SET_DIGEST_DOMAIN = b"iroha:kagemusha:v1:artifact-set"
VK_SET_DIGEST_DOMAIN = b"iroha:kagemusha:v1:vk-set"
PROFILE_QUALIFICATION_DIGEST_DOMAIN = b"iroha:kagemusha:v1:profile-qualification"
HARDWARE_POLICY_DIGEST_DOMAIN = b"iroha:kagemusha:v1:hardware-policy"
HARDWARE_PROFILE_DIGEST_DOMAIN = b"iroha:kagemusha:v1:hardware-profile"
SUITE_COMMITMENT_DOMAIN = b"iroha:kagemusha:v1:suite-commitment"
RELEASE_PROFILE_DIGEST_DOMAIN = b"iroha:kagemusha:v1:release-profile"
VERIFICATION_RECORDS_DIGEST_DOMAIN = b"iroha:kagemusha:v1:verification-records"
CANDIDATE_CONTEXT_SCHEMA = "iroha.kagemusha_v1.release_candidate_context"

ARTIFACT_SET_SCHEMA = "iroha.kagemusha.v1.release-artifact-set-digest-subject"
VK_SET_SCHEMA = "iroha.kagemusha.v1.release-vk-set-digest-subject"
PROFILE_QUALIFICATION_SCHEMA = (
    "iroha.kagemusha.v1.release-profile-qualification-digest-subject"
)
HARDWARE_POLICY_SCHEMA = "iroha.kagemusha.v1.release-hardware-policy-digest-subject"
HARDWARE_PROFILE_SCHEMA = "iroha.kagemusha.v1.hardware-profile-id-preimage"
RELEASE_PROFILE_SCHEMA = "iroha.kagemusha.v1.release-profile-digest-subject"

REPORT_SCHEMAS = frozenset(
    {
        "iroha.kagemusha_v1.circuit_shape_report",
        "iroha.kagemusha_v1.security_review_report",
        "iroha.kagemusha_v1.kat_report",
        "iroha.kagemusha_v1.fuzz_report",
        "iroha.kagemusha_v1.resource_report",
        "iroha.kagemusha_v1.hardware_profile_qualification_report",
        "iroha.kagemusha_v1.relation_qualification_report",
        "iroha.kagemusha_v1.helper_qualification_report",
        "iroha.kagemusha_v1.recursive_depth_report",
        "iroha.kagemusha_v1.aggregate_balance_report",
        "iroha.kagemusha_v1.thermal_report",
        "iroha.kagemusha_v1.envelope_report",
        "iroha.kagemusha_v1.acceptance_case_report",
        "iroha.kagemusha_v1.reproducible_build_report",
    }
)

_HEX_64 = re.compile(r"[0-9a-f]{64}")
_SAFE_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9._+-]{0,127}")
_SAFE_LITERAL = re.compile(r"[A-Za-z0-9][A-Za-z0-9._+=:-]{0,127}")


class KagemushaEvidenceError(RuntimeError):
    """Raised when the Kagemusha evidence closure is invalid."""


def _fail(message: str) -> NoReturn:
    raise KagemushaEvidenceError(message)


def _object(
    value: object,
    label: str,
    fields: set[str] | frozenset[str],
) -> Mapping[str, object]:
    if not isinstance(value, Mapping) or set(value) != set(fields):
        _fail(f"{label} fields must be exactly {', '.join(sorted(fields))}")
    return value


def _array(value: object, label: str) -> list[object]:
    if not isinstance(value, list):
        _fail(f"{label} must be an array")
    return value


def _string(value: object, label: str) -> str:
    if not isinstance(value, str) or not value:
        _fail(f"{label} must be a non-empty string")
    return value


def _safe_id(value: object, label: str) -> str:
    text = _string(value, label)
    if _SAFE_ID.fullmatch(text) is None:
        _fail(f"{label} must be a bounded safe identifier")
    return text


def _digest(value: object, label: str, *, nonzero: bool = True) -> str:
    if not isinstance(value, str) or _HEX_64.fullmatch(value) is None:
        _fail(f"{label} must be exactly 64 lowercase hexadecimal characters")
    if nonzero and value == "0" * 64:
        _fail(f"{label} must not be zero")
    return value


def _integer(
    value: object,
    label: str,
    *,
    minimum: int = 0,
    maximum: int | None = None,
) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
        _fail(f"{label} must be an integer at least {minimum}")
    if maximum is not None and value > maximum:
        _fail(f"{label} exceeds {maximum}")
    return value


def _true(value: object, label: str) -> None:
    if value is not True:
        _fail(f"{label} must be true")


def _binding(info: StableFile) -> dict[str, object]:
    return {"byte_len": info.size, "sha256": info.sha256}


def _review_digest(value: object) -> str:
    return hashlib.sha256(canonical_json_bytes(value)).hexdigest()


def _hex_bytes(value: object, label: str, *, size: int = 32) -> bytes:
    text = _string(value, label)
    if len(text) != size * 2 or re.fullmatch(r"[0-9a-f]+", text) is None:
        _fail(f"{label} must be exactly {size * 2} lowercase hexadecimal characters")
    return bytes.fromhex(text)


def _varint(value: int) -> bytes:
    if value < 0 or value > (1 << 64) - 1:
        _fail("Norito compact length does not fit u64")
    result = bytearray()
    while value >= 0x80:
        result.append((value & 0x7F) | 0x80)
        value >>= 7
    result.append(value)
    return bytes(result)


def _u8(value: int) -> bytes:
    return value.to_bytes(1, "little")


def _u16(value: int) -> bytes:
    return value.to_bytes(2, "little")


def _u32(value: int) -> bytes:
    return value.to_bytes(4, "little")


def _u64(value: int) -> bytes:
    return value.to_bytes(8, "little")


def _norito_struct(*fields: bytes) -> bytes:
    return b"".join(_varint(len(field)) + field for field in fields)


def _norito_vec(items: Sequence[bytes]) -> bytes:
    return _u64(len(items)) + b"".join(_varint(len(item)) + item for item in items)


def _crc64_xz(payload: bytes) -> int:
    crc = (1 << 64) - 1
    polynomial = 0xC96C5795D7870F42
    for byte in payload:
        crc ^= byte
        for _ in range(8):
            crc = (crc >> 1) ^ (polynomial if crc & 1 else 0)
    return crc ^ ((1 << 64) - 1)


def _norito_frame(schema_name: str, payload: bytes) -> bytes:
    schema = hashlib.sha256(
        b"norito:v1:type-name\0" + schema_name.encode("utf-8")
    ).digest()[:16]
    return (
        b"NRT0"
        + b"\0\0"
        + schema
        + b"\0"
        + _u64(len(payload))
        + _u64(_crc64_xz(payload))
        + b"\x02"
        + payload
    )


def _rust_digest(domain: bytes, schema_name: str, payload: bytes) -> str:
    frame = _norito_frame(schema_name, payload)
    return hashlib.sha256(domain + b"\0" + _u64(len(frame)) + frame).hexdigest()


def _raw_domain_digest(domain: bytes, value: bytes) -> str:
    return hashlib.sha256(domain + b"\0" + _u64(len(value)) + value).hexdigest()


def release_candidate_context(
    *,
    source_archive: Mapping[str, object],
    cargo_lock: Mapping[str, object],
    artifacts: Sequence[Mapping[str, object]],
    artifact_set_digest: str,
    vk_digest: str,
    protocols: Mapping[str, object],
    profile_inputs: Sequence[Mapping[str, object]],
    observer_policy: Mapping[str, object],
) -> tuple[dict[str, object], str]:
    """Derive the non-circular candidate identity signed by every observer."""

    context: dict[str, object] = {
        "schema": CANDIDATE_CONTEXT_SCHEMA,
        "schema_version": SCHEMA_VERSION,
        "release_evidence_schema": MANIFEST_SCHEMA,
        "release_evidence_schema_version": SCHEMA_VERSION,
        "wire_version": WIRE_VERSION,
        "halo2_k": HALO2_K,
        "source": {
            "source_archive": dict(source_archive),
            "cargo_lock": dict(cargo_lock),
        },
        "artifact_inventory": [dict(artifact) for artifact in artifacts],
        "artifact_set_digest": artifact_set_digest,
        "vk_digest": vk_digest,
        "protocols": dict(protocols),
        "profile_inputs": [dict(profile) for profile in profile_inputs],
        "observer_policy": dict(observer_policy),
    }
    encoded = canonical_json_bytes(context)
    return context, _raw_domain_digest(CANDIDATE_CONTEXT_DIGEST_DOMAIN, encoded)


def _artifact_payload(value: Mapping[str, object]) -> bytes:
    role = _string(value["role"], "artifact role")
    try:
        role_index = ARTIFACT_ROLES.index(role)
    except ValueError:
        _fail(f"unsupported artifact role {role!r}")
    return _norito_struct(
        _u32(role_index),
        _hex_bytes(value["sha256"], f"artifact {role} SHA-256"),
        _u64(_integer(value["byte_len"], f"artifact {role} length", minimum=1)),
    )


def _evidence_file_payload(value: Mapping[str, object]) -> bytes:
    return _norito_struct(
        _hex_bytes(value["sha256"], "evidence SHA-256"),
        _u64(_integer(value["byte_len"], "evidence length", minimum=1)),
    )


def _suite_commitment(suite_id: str) -> str:
    raw = _hex_bytes(suite_id, "suite id")
    return _raw_domain_digest(SUITE_COMMITMENT_DOMAIN, raw)


_PLATFORM_CLASSES = (
    "android_oem_service",
    "apple_oem_service",
    "dedicated_secure_element",
    "other_qualified",
)
_P256_PRIME = 0xFFFFFFFF00000001000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFF
_P256_B = 0x5AC635D8AA3A93E7B3EBBD55769886BC651D06B0CC53B0F63BCE3C3E27D2604B


def _device_public_key(value: object, label: str) -> bytes:
    raw = _hex_bytes(value, label, size=65)
    if raw[0] != 4:
        _fail(f"{label} must be an uncompressed SEC1 P-256 point")
    x = int.from_bytes(raw[1:33], "big")
    y = int.from_bytes(raw[33:], "big")
    if x >= _P256_PRIME or y >= _P256_PRIME:
        _fail(f"{label} coordinates are outside P-256")
    if (y * y - (x * x * x - 3 * x + _P256_B)) % _P256_PRIME != 0:
        _fail(f"{label} is not on P-256")
    return raw


_HARDWARE_PROFILE_FIELDS = {
    "version",
    "protocol_version",
    "hardware_profile_id",
    "provider_id",
    "platform_class",
    "product_class_digest",
    "firmware_policy_digest",
    "enrollment_attestation_verifier_digest",
    "attestation_trust_roots_digest",
    "allowed_suite_commitment",
    "policy_epoch",
    "governance_credential_public_key",
    "capability_mask",
    "qualification_report_digest",
    "valid_from_ms",
    "expires_at_ms",
}


def _hardware_profile_preimage_payload(value: Mapping[str, object]) -> bytes:
    platform = _string(value["platform_class"], "hardware platform class")
    try:
        platform_index = _PLATFORM_CLASSES.index(platform)
    except ValueError:
        _fail(f"unsupported hardware platform class {platform!r}")
    return _norito_struct(
        _u16(_integer(value["version"], "hardware profile version", minimum=1, maximum=1)),
        _u16(
            _integer(
                value["protocol_version"],
                "hardware profile protocol version",
                minimum=1,
                maximum=1,
            )
        ),
        _hex_bytes(value["provider_id"], "hardware provider id"),
        _u32(platform_index),
        _hex_bytes(value["product_class_digest"], "hardware product class digest"),
        _hex_bytes(value["firmware_policy_digest"], "hardware firmware policy digest"),
        _hex_bytes(
            value["enrollment_attestation_verifier_digest"],
            "hardware enrollment verifier digest",
        ),
        _hex_bytes(value["attestation_trust_roots_digest"], "hardware trust roots digest"),
        _hex_bytes(value["allowed_suite_commitment"], "hardware suite commitment"),
        _u64(_integer(value["policy_epoch"], "hardware policy epoch", minimum=1)),
        _device_public_key(
            value["governance_credential_public_key"],
            "hardware governance credential public key",
        ),
        _u16(
            _integer(
                value["capability_mask"],
                "hardware capability mask",
                minimum=0,
                maximum=(1 << 16) - 1,
            )
        ),
        _hex_bytes(value["qualification_report_digest"], "hardware qualification digest"),
        _u64(_integer(value["valid_from_ms"], "hardware valid-from time")),
        _u64(_integer(value["expires_at_ms"], "hardware expiry time", minimum=1)),
    )


def rust_hardware_profile_id(value: Mapping[str, object]) -> str:
    """Return the exact Rust `HardwareProfileV1` identity."""

    return _rust_digest(
        HARDWARE_PROFILE_DIGEST_DOMAIN,
        HARDWARE_PROFILE_SCHEMA,
        _hardware_profile_preimage_payload(value),
    )


def _hardware_profile_payload(value: Mapping[str, object]) -> bytes:
    # Insert the derived identity after the first two fields. Rebuild explicitly
    # because the identity is excluded only from the profile-id preimage.
    platform = _PLATFORM_CLASSES.index(str(value["platform_class"]))
    return _norito_struct(
        _u16(int(value["version"])),
        _u16(int(value["protocol_version"])),
        _hex_bytes(value["hardware_profile_id"], "hardware profile id"),
        _hex_bytes(value["provider_id"], "hardware provider id"),
        _u32(platform),
        _hex_bytes(value["product_class_digest"], "hardware product class digest"),
        _hex_bytes(value["firmware_policy_digest"], "hardware firmware policy digest"),
        _hex_bytes(
            value["enrollment_attestation_verifier_digest"],
            "hardware enrollment verifier digest",
        ),
        _hex_bytes(value["attestation_trust_roots_digest"], "hardware trust roots digest"),
        _hex_bytes(value["allowed_suite_commitment"], "hardware suite commitment"),
        _u64(int(value["policy_epoch"])),
        _device_public_key(
            value["governance_credential_public_key"],
            "hardware governance credential public key",
        ),
        _u16(int(value["capability_mask"])),
        _hex_bytes(value["qualification_report_digest"], "hardware qualification digest"),
        _u64(int(value["valid_from_ms"])),
        _u64(int(value["expires_at_ms"])),
    )


def _enabled_profile_payload(value: Mapping[str, object]) -> bytes:
    hardware = _object(
        value["hardware_profile"], "embedded hardware profile", _HARDWARE_PROFILE_FIELDS
    )
    report = _object(
        value["qualification_report"],
        "profile qualification report binding",
        {"sha256", "byte_len"},
    )
    return _norito_struct(
        _hardware_profile_payload(hardware),
        _hex_bytes(value["hardware_profile_id"], "enabled hardware profile id"),
        _hex_bytes(value["suite_id"], "enabled suite id"),
        _hex_bytes(value["vk_digest"], "enabled VK digest"),
        _hex_bytes(value["qualification_digest"], "enabled qualification digest"),
        _u64(int(value["policy_epoch"])),
        _evidence_file_payload(report),
    )


def _helper_protocol_payload(value: Mapping[str, object]) -> bytes:
    helper = _string(value["helper"], "helper protocol name")
    try:
        index = HELPERS.index(helper)
    except ValueError:
        _fail(f"unsupported helper protocol {helper!r}")
    return _norito_struct(
        _u32(index),
        _hex_bytes(value["eq_protocol_digest"], f"{helper} Eq protocol digest"),
        _hex_bytes(value["ep_protocol_digest"], f"{helper} Ep protocol digest"),
        _u32(int(value["eq_proof_bytes"])),
        _u32(int(value["ep_proof_bytes"])),
    )


def rust_artifact_set_digest(artifacts: Sequence[Mapping[str, object]]) -> str:
    """Return the exact Rust authenticated artifact-set identity."""

    payload = _norito_struct(_norito_vec([_artifact_payload(row) for row in artifacts]))
    return _rust_digest(ARTIFACT_SET_DIGEST_DOMAIN, ARTIFACT_SET_SCHEMA, payload)


def rust_vk_set_digest(
    artifacts: Sequence[Mapping[str, object]], protocols: Mapping[str, object]
) -> str:
    """Return the exact Rust state/wrapper/helper verifier-set identity."""

    helpers = _array(protocols["helper_protocols"], "helper protocols")
    verifying_keys = [row for row in artifacts if "_vk_" in str(row["role"])]
    payload = _norito_struct(
        _u16(WIRE_VERSION),
        _hex_bytes(protocols["state_eq_protocol_digest"], "state Eq protocol digest"),
        _hex_bytes(protocols["state_ep_protocol_digest"], "state Ep protocol digest"),
        _hex_bytes(
            protocols["commit_wrapper_eq_protocol_digest"], "wrapper Eq protocol digest"
        ),
        _hex_bytes(
            protocols["commit_wrapper_ep_protocol_digest"], "wrapper Ep protocol digest"
        ),
        _norito_vec([_helper_protocol_payload(row) for row in helpers]),
        _norito_vec([_artifact_payload(row) for row in verifying_keys]),
    )
    return _rust_digest(VK_SET_DIGEST_DOMAIN, VK_SET_SCHEMA, payload)


def _relation_qualification_payload(value: Mapping[str, object]) -> bytes:
    relation = _string(value["relation"], "qualified relation")
    return _norito_struct(
        _u32(RELATIONS.index(relation)),
        _hex_bytes(value["eq_protocol_digest"], "qualified Eq protocol digest"),
        _hex_bytes(value["ep_protocol_digest"], "qualified Ep protocol digest"),
        _artifact_payload(_object(value["eq_verifying_key"], "Eq VK", {"role", "sha256", "byte_len"})),
        _artifact_payload(_object(value["ep_verifying_key"], "Ep VK", {"role", "sha256", "byte_len"})),
        _u32(int(value["eq_circuit_rows"])),
        _u32(int(value["ep_circuit_rows"])),
        _u32(int(value["complete_proof_bytes"])),
        _u32(int(value["prove_p95_ms"])),
        _u32(int(value["verify_p95_ms"])),
        _u64(int(value["process_rss_bytes"])),
        _u64(int(value["operation_energy_millijoules"])),
        _evidence_file_payload(_object(value["report"], "relation report", {"sha256", "byte_len"})),
    )


def _helper_qualification_payload(value: Mapping[str, object]) -> bytes:
    helper = _string(value["helper"], "qualified helper")
    return _norito_struct(
        _u32(HELPERS.index(helper)),
        _hex_bytes(value["eq_protocol_digest"], "helper Eq protocol digest"),
        _hex_bytes(value["ep_protocol_digest"], "helper Ep protocol digest"),
        _artifact_payload(_object(value["eq_verifying_key"], "helper Eq VK", {"role", "sha256", "byte_len"})),
        _artifact_payload(_object(value["ep_verifying_key"], "helper Ep VK", {"role", "sha256", "byte_len"})),
        _u32(int(value["eq_circuit_rows"])),
        _u32(int(value["ep_circuit_rows"])),
        _u32(int(value["eq_proof_bytes"])),
        _u32(int(value["ep_proof_bytes"])),
        _u32(int(value["complete_proof_bytes"])),
        _u32(int(value["prove_p95_ms"])),
        _u32(int(value["verify_p95_ms"])),
        _u64(int(value["process_rss_bytes"])),
        _u64(int(value["operation_energy_millijoules"])),
        _evidence_file_payload(_object(value["report"], "helper report", {"sha256", "byte_len"})),
    )


def _depth_payload(value: Mapping[str, object]) -> bytes:
    return _norito_struct(
        _u32(int(value["depth"])),
        _u32(int(value["verified_handoffs"])),
        _u32(int(value["complete_proof_bytes"])),
        _u32(int(value["raw_session_bytes"])),
        _u32(int(value["text_session_bytes"])),
        _evidence_file_payload(_object(value["report"], "depth report", {"sha256", "byte_len"})),
    )


def _aggregate_payload(value: Mapping[str, object]) -> bytes:
    return _norito_struct(
        _u32(int(value["independent_payments"])),
        _u32(int(value["folded_credits"])),
        _u32(int(value["spend_payments"])),
        _evidence_file_payload(_object(value["report"], "aggregate report", {"sha256", "byte_len"})),
    )


def _thermal_payload(value: Mapping[str, object]) -> bytes:
    return _norito_struct(
        _u32(int(value["folded_credits"])),
        _u32(int(value["fold_p95_ms"])),
        _u64(int(value["process_rss_bytes"])),
        _u64(int(value["operation_energy_millijoules"])),
        _evidence_file_payload(_object(value["report"], "thermal report", {"sha256", "byte_len"})),
    )


def _envelope_payload(value: Mapping[str, object]) -> bytes:
    return _norito_struct(
        _u32(int(value["raw_session_bytes"])),
        _u32(int(value["text_session_bytes"])),
        _u32(int(value["handoff_p95_ms"])),
        _evidence_file_payload(_object(value["report"], "envelope report", {"sha256", "byte_len"})),
    )


def _acceptance_payload(value: Mapping[str, object]) -> bytes:
    case = _string(value["case"], "acceptance case")
    return _norito_struct(
        _u32(ACCEPTANCE_CASES.index(case)),
        _u8(int(value["validator_count"])),
        _evidence_file_payload(_object(value["report"], "acceptance report", {"sha256", "byte_len"})),
    )


def _profile_qualification_payload(value: Mapping[str, object]) -> bytes:
    profile = dict(_object(value["profile"], "enabled profile", {
        "hardware_profile", "hardware_profile_id", "suite_id", "vk_digest",
        "qualification_digest", "policy_epoch", "qualification_report",
    }))
    profile["qualification_digest"] = "0" * 64
    return _norito_struct(
        _enabled_profile_payload(profile),
        _norito_vec([_relation_qualification_payload(row) for row in _array(value["relations"], "relations")]),
        _norito_vec([_helper_qualification_payload(row) for row in _array(value["helper_circuits"], "helper circuits")]),
        _norito_vec([_depth_payload(row) for row in _array(value["recursive_depths"], "recursive depths")]),
        _aggregate_payload(_object(value["aggregate_balance"], "aggregate qualification", {
            "independent_payments", "folded_credits", "spend_payments", "report",
        })),
        _thermal_payload(_object(value["thermal"], "thermal qualification", {
            "folded_credits", "fold_p95_ms", "process_rss_bytes",
            "operation_energy_millijoules", "report",
        })),
        _envelope_payload(_object(value["envelope"], "envelope qualification", {
            "raw_session_bytes", "text_session_bytes", "handoff_p95_ms", "report",
        })),
        _norito_vec([_acceptance_payload(row) for row in _array(value["acceptance_cases"], "acceptance cases")]),
    )


def rust_profile_qualification_digest(value: Mapping[str, object]) -> str:
    """Return the exact Rust typed profile-qualification identity."""

    payload = _norito_struct(_profile_qualification_payload(value))
    return _rust_digest(
        PROFILE_QUALIFICATION_DIGEST_DOMAIN, PROFILE_QUALIFICATION_SCHEMA, payload
    )


def rust_hardware_policy_digest(profiles: Sequence[Mapping[str, object]]) -> str:
    """Return the exact Rust authenticated enabled-profile policy identity."""

    payload = _norito_struct(_norito_vec([_enabled_profile_payload(row) for row in profiles]))
    return _rust_digest(HARDWARE_POLICY_DIGEST_DOMAIN, HARDWARE_POLICY_SCHEMA, payload)


def rust_release_profile_digest(
    circuit_shape_report: Mapping[str, object], protocols: Mapping[str, object]
) -> str:
    """Return the exact Rust release circuit/profile identity."""

    payload = _norito_struct(
        _u16(WIRE_VERSION),
        _u32(HALO2_K),
        _evidence_file_payload(circuit_shape_report),
        _hex_bytes(protocols["state_eq_protocol_digest"], "state Eq protocol digest"),
        _hex_bytes(protocols["state_ep_protocol_digest"], "state Ep protocol digest"),
        _hex_bytes(protocols["commit_wrapper_eq_protocol_digest"], "wrapper Eq protocol digest"),
        _hex_bytes(protocols["commit_wrapper_ep_protocol_digest"], "wrapper Ep protocol digest"),
        _norito_vec(
            [_helper_protocol_payload(row) for row in _array(protocols["helper_protocols"], "helper protocols")]
        ),
    )
    return _rust_digest(RELEASE_PROFILE_DIGEST_DOMAIN, RELEASE_PROFILE_SCHEMA, payload)


# Minimal, allocation-bounded Ed25519 verification used only for detached
# observer approvals. It implements RFC 8032 verification and rejects non-prime
# order public keys and R encodings.
_ED_Q = 2**255 - 19
_ED_L = 2**252 + 27742317777372353535851937790883648493
_ED_D = (-121665 * pow(121666, _ED_Q - 2, _ED_Q)) % _ED_Q
_ED_I = pow(2, (_ED_Q - 1) // 4, _ED_Q)
_ED_IDENTITY = (0, 1)


def _ed_xrecover(y: int) -> int:
    xx = (y * y - 1) * pow(_ED_D * y * y + 1, _ED_Q - 2, _ED_Q) % _ED_Q
    x = pow(xx, (_ED_Q + 3) // 8, _ED_Q)
    if (x * x - xx) % _ED_Q:
        x = x * _ED_I % _ED_Q
    if x & 1:
        x = _ED_Q - x
    return x


_ED_B = (_ed_xrecover(4 * pow(5, _ED_Q - 2, _ED_Q) % _ED_Q), 4 * pow(5, _ED_Q - 2, _ED_Q) % _ED_Q)


def _ed_add(left: tuple[int, int], right: tuple[int, int]) -> tuple[int, int]:
    return _ed_from_extended(_ed_extended_add(_ed_to_extended(left), _ed_to_extended(right)))


def _ed_to_extended(point: tuple[int, int]) -> tuple[int, int, int, int]:
    x, y = point
    return x, y, 1, x * y % _ED_Q


def _ed_from_extended(point: tuple[int, int, int, int]) -> tuple[int, int]:
    x, y, z, _ = point
    inverse = pow(z, _ED_Q - 2, _ED_Q)
    return x * inverse % _ED_Q, y * inverse % _ED_Q


def _ed_extended_add(
    left: tuple[int, int, int, int], right: tuple[int, int, int, int]
) -> tuple[int, int, int, int]:
    x1, y1, z1, t1 = left
    x2, y2, z2, t2 = right
    a = (y1 - x1) * (y2 - x2) % _ED_Q
    b = (y1 + x1) * (y2 + x2) % _ED_Q
    c = 2 * _ED_D * t1 * t2 % _ED_Q
    d = 2 * z1 * z2 % _ED_Q
    e = b - a
    f = d - c
    g = d + c
    h = b + a
    return e * f % _ED_Q, g * h % _ED_Q, f * g % _ED_Q, e * h % _ED_Q


def _ed_scalarmult(point: tuple[int, int], scalar: int) -> tuple[int, int]:
    result = _ed_to_extended(_ED_IDENTITY)
    addend = _ed_to_extended(point)
    while scalar:
        if scalar & 1:
            result = _ed_extended_add(result, addend)
        addend = _ed_extended_add(addend, addend)
        scalar >>= 1
    return _ed_from_extended(result)


def _ed_decode(raw: bytes) -> tuple[int, int] | None:
    if len(raw) != 32:
        return None
    encoded = int.from_bytes(raw, "little")
    y = encoded & ((1 << 255) - 1)
    sign = encoded >> 255
    if y >= _ED_Q:
        return None
    x = _ed_xrecover(y)
    if x == 0 and sign == 1:
        return None
    if (x & 1) != sign:
        x = _ED_Q - x
    if (y * y - x * x - 1 - _ED_D * x * x * y * y) % _ED_Q:
        return None
    point = (x, y)
    if _ed_scalarmult(point, _ED_L) != _ED_IDENTITY or point == _ED_IDENTITY:
        return None
    return point


def _ed_encode(point: tuple[int, int]) -> bytes:
    x, y = point
    return (y | ((x & 1) << 255)).to_bytes(32, "little")


def _ed25519_verify(public_key: bytes, message: bytes, signature: bytes) -> bool:
    if len(signature) != 64:
        return False
    encoded_r, encoded_s = signature[:32], signature[32:]
    scalar = int.from_bytes(encoded_s, "little")
    if scalar >= _ED_L:
        return False
    public = _ed_decode(public_key)
    point_r = _ed_decode(encoded_r)
    if public is None or point_r is None:
        return False
    challenge = int.from_bytes(
        hashlib.sha512(encoded_r + public_key + message).digest(), "little"
    ) % _ED_L
    return _ed_encode(_ed_scalarmult(_ED_B, scalar)) == _ed_encode(
        _ed_add(point_r, _ed_scalarmult(public, challenge))
    )


def _max_for_kind(kind: str) -> int:
    return {
        "artifact": MAX_ARTIFACT_BYTES,
        "cargo_lock": MAX_MANIFEST_BYTES,
        "event_log": MAX_EVENT_LOG_BYTES,
        "internal_proof": INTERNAL_PROOF_RESOURCE_MAX_BYTES,
        "proof": PAIRED_PROOF_MAX_BYTES,
        "raw_session": RAW_SESSION_MAX_BYTES,
        "report": MAX_REPORT_BYTES,
        "source_archive": MAX_SOURCE_ARCHIVE_BYTES,
        "text_session": TEXT_SESSION_MAX_BYTES,
        "observation": MAX_OBSERVATION_BYTES,
        "transcript": MAX_TRANSCRIPT_BYTES,
    }[kind]


def _scan_evidence_tree(root: Path) -> list[str]:
    """Return a bounded descriptor-pinned inventory without following links."""

    directory_flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )

    def open_root() -> tuple[int, os.stat_result]:
        absolute = Path(os.path.abspath(root))
        current_fd = -1
        try:
            current_fd = os.open("/", directory_flags)
            for component in absolute.parts[1:]:
                before = os.stat(component, dir_fd=current_fd, follow_symlinks=False)
                next_fd = os.open(component, directory_flags, dir_fd=current_fd)
                opened = os.fstat(next_fd)
                if not stat.S_ISDIR(opened.st_mode) or (
                    before.st_dev,
                    before.st_ino,
                ) != (opened.st_dev, opened.st_ino):
                    os.close(next_fd)
                    _fail("evidence root changed while its path was opened")
                os.close(current_fd)
                current_fd = next_fd
            identity = os.fstat(current_fd)
            if identity.st_mode & (stat.S_IWGRP | stat.S_IWOTH):
                _fail("evidence root must not be group- or world-writable")
            return current_fd, identity
        except Exception:
            if current_fd >= 0:
                os.close(current_fd)
            raise

    root_fd = -1
    reopened_fd = -1
    inventory: list[str] = []
    directory_count = 1
    entry_count = 0

    def visit(directory_fd: int, prefix: str, depth: int) -> None:
        nonlocal directory_count, entry_count
        saw_entry = False
        with os.scandir(directory_fd) as entries:
            for entry in entries:
                saw_entry = True
                entry_count += 1
                if entry_count > MAX_EVIDENCE_TREE_ENTRIES:
                    _fail("evidence tree exceeds the aggregate directory-entry cap")
                relative = f"{prefix}/{entry.name}" if prefix else entry.name
                if canonical_relative_path(relative) != relative:
                    _fail("evidence tree contains a non-canonical path")
                entry_depth = depth + 1
                if entry_depth > MAX_EVIDENCE_TREE_DEPTH:
                    _fail("evidence tree exceeds the maximum directory depth")
                info = os.stat(entry.name, dir_fd=directory_fd, follow_symlinks=False)
                if stat.S_ISLNK(info.st_mode):
                    _fail(f"evidence tree contains a symlink: {relative!r}")
                if stat.S_ISDIR(info.st_mode):
                    if info.st_mode & (stat.S_IWGRP | stat.S_IWOTH):
                        _fail(
                            f"evidence directory is group- or world-writable: {relative!r}"
                        )
                    directory_count += 1
                    if directory_count > MAX_EVIDENCE_DIRECTORIES:
                        _fail("evidence tree exceeds the aggregate directory cap")
                    child_fd = -1
                    try:
                        child_fd = os.open(
                            entry.name,
                            directory_flags,
                            dir_fd=directory_fd,
                        )
                        opened = os.fstat(child_fd)
                        if (info.st_dev, info.st_ino) != (
                            opened.st_dev,
                            opened.st_ino,
                        ):
                            _fail(f"evidence directory changed while opened: {relative!r}")
                        visit(child_fd, relative, entry_depth)
                        after = os.stat(
                            entry.name,
                            dir_fd=directory_fd,
                            follow_symlinks=False,
                        )
                        if (info.st_dev, info.st_ino) != (
                            after.st_dev,
                            after.st_ino,
                        ):
                            _fail(f"evidence directory changed while scanned: {relative!r}")
                    finally:
                        if child_fd >= 0:
                            os.close(child_fd)
                elif stat.S_ISREG(info.st_mode):
                    inventory.append(relative)
                    if len(inventory) > MAX_EVIDENCE_FILES:
                        _fail("evidence tree exceeds the aggregate file cap")
                else:
                    _fail(f"evidence tree contains a non-regular entry: {relative!r}")
        if not saw_entry:
            _fail("evidence tree contains an empty directory")

    try:
        root_fd, root_identity = open_root()
        visit(root_fd, "", 0)
        reopened_fd, reopened_identity = open_root()
        if (root_identity.st_dev, root_identity.st_ino) != (
            reopened_identity.st_dev,
            reopened_identity.st_ino,
        ):
            _fail("evidence root changed while it was scanned")
    except OSError as error:
        raise KagemushaEvidenceError(f"failed to scan bounded evidence tree: {error}") from error
    finally:
        if root_fd >= 0:
            os.close(root_fd)
        if reopened_fd >= 0:
            os.close(reopened_fd)
    return sorted(inventory)


@dataclass(frozen=True)
class TrustedVerifier:
    """One locally trusted verifier identity and its admitted reports."""

    verifier_id: str
    sha256: str
    report_schemas: frozenset[str]


@dataclass(frozen=True)
class TrustedObserverPolicy:
    """Separately pinned authority and verifier allowlist."""

    info: StableFile
    path: Path
    payload: bytes
    threshold: int
    authorities: Mapping[str, bytes]
    verifiers: Mapping[str, TrustedVerifier]


@dataclass(frozen=True)
class CommandSpec:
    """One exact threshold-observed trusted verifier invocation."""

    command_id: str
    verifier_id: str
    verifier_sha256: str
    report_schema: str
    arguments: tuple[tuple[str, str], ...]
    stdout: str
    stderr: str
    observation: str
    projection: Mapping[str, object]


def _approval_message(subject: Mapping[str, object]) -> bytes:
    encoded = canonical_json_bytes(dict(subject))
    return OBSERVATION_APPROVAL_DOMAIN + _u64(len(encoded)) + encoded


def _load_observer_policy(
    path: Path, expected_sha256: str
) -> TrustedObserverPolicy:
    expected = _digest(expected_sha256, "expected observer-policy SHA-256")
    info, payload = stable_read_path(path, max_size=MAX_OBSERVER_POLICY_BYTES)
    if info.sha256 != expected:
        _fail("observer policy differs from its explicit immutable identity")
    policy = load_json_object(payload, "trusted observer policy")
    _object(
        policy,
        "trusted observer policy",
        {"schema", "schema_version", "threshold", "authorities", "verifiers"},
    )
    if (
        policy["schema"] != OBSERVER_POLICY_SCHEMA
        or policy["schema_version"] != SCHEMA_VERSION
        or canonical_json_bytes(dict(policy)) != payload
    ):
        _fail("trusted observer policy is not canonical supported V1 JSON")
    authority_rows = _array(policy["authorities"], "trusted observer authorities")
    if not authority_rows or len(authority_rows) > 32:
        _fail("trusted observer policy must contain 1 through 32 authorities")
    authorities: dict[str, bytes] = {}
    ids: list[str] = []
    for index, raw in enumerate(authority_rows):
        row = _object(
            raw,
            f"trusted observer authority {index}",
            {"authority_id", "ed25519_public_key"},
        )
        public_key = _hex_bytes(
            row["ed25519_public_key"], f"trusted observer authority {index} public key"
        )
        if _ed_decode(public_key) is None:
            _fail("trusted observer policy contains an invalid Ed25519 public key")
        authority_id = _digest(
            row["authority_id"], f"trusted observer authority {index} id"
        )
        expected_id = hashlib.sha256(
            OBSERVER_AUTHORITY_ID_DOMAIN + public_key
        ).hexdigest()
        if authority_id != expected_id:
            _fail("trusted observer authority id is not derived from its public key")
        authorities[authority_id] = public_key
        ids.append(authority_id)
    if ids != sorted(set(ids)):
        _fail("trusted observer authorities must be uniquely sorted by id")
    threshold = _integer(
        policy["threshold"],
        "trusted observer threshold",
        minimum=1,
        maximum=len(authorities),
    )

    verifier_rows = _array(policy["verifiers"], "trusted verifier allowlist")
    if not verifier_rows or len(verifier_rows) > 64:
        _fail("trusted verifier allowlist must contain 1 through 64 entries")
    verifiers: dict[str, TrustedVerifier] = {}
    verifier_ids: list[str] = []
    for index, raw in enumerate(verifier_rows):
        row = _object(
            raw,
            f"trusted verifier {index}",
            {"id", "sha256", "report_schemas"},
        )
        verifier_id = _safe_id(row["id"], f"trusted verifier {index} id")
        schemas = [
            _string(value, f"trusted verifier {verifier_id} report schema")
            for value in _array(row["report_schemas"], "trusted verifier report schemas")
        ]
        if not schemas or schemas != sorted(set(schemas)) or not set(schemas) <= REPORT_SCHEMAS:
            _fail("trusted verifier report schemas are empty, duplicated, unordered, or unknown")
        verifiers[verifier_id] = TrustedVerifier(
            verifier_id=verifier_id,
            sha256=_digest(row["sha256"], f"trusted verifier {verifier_id} SHA-256"),
            report_schemas=frozenset(schemas),
        )
        verifier_ids.append(verifier_id)
    if verifier_ids != sorted(set(verifier_ids)):
        _fail("trusted verifier allowlist must be uniquely sorted by id")
    return TrustedObserverPolicy(
        info=info,
        path=path,
        payload=payload,
        threshold=threshold,
        authorities=authorities,
        verifiers=verifiers,
    )


class EvidenceVerifier:
    """Stateful verifier for one immutable evidence closure."""

    def __init__(
        self,
        *,
        root: Path,
        manifest_path: Path,
        manifest_info: StableFile,
        manifest_bytes: bytes,
        manifest_sha256: str,
        manifest: Mapping[str, object],
        observer_policy: TrustedObserverPolicy,
    ) -> None:
        self.root = root
        self.manifest_path = manifest_path
        self.manifest_info = manifest_info
        self.manifest_bytes = manifest_bytes
        self.manifest_sha256 = manifest_sha256
        self.manifest = manifest
        self.observer_policy = observer_policy
        self.files: dict[str, StableFile] = {}
        self.file_kinds: dict[str, str] = {}
        self.payloads: dict[str, bytes] = {}
        self.artifacts: dict[str, str] = {}
        self.commands: dict[str, CommandSpec] = {}
        self.command_requirements: dict[str, frozenset[str]] = {}
        self.used_commands: set[str] = set()
        self.used_reports: set[str] = set()
        self.used_samples: dict[str, str] = {}
        self.used_files: set[str] = set()
        self.total_transcript_bytes = 0
        self.total_command_input_bytes = 0
        self.total_observed_duration_ms = 0
        self.total_observed_cpu_ms = 0
        self.total_jsonl_rows = 0

    def verify(self) -> dict[str, object]:
        required = {
            "schema",
            "schema_version",
            "source",
            "files",
            "artifacts",
            "protocols",
            "global_reports",
            "profiles",
            "reproducible_builds",
            "commands",
        }
        _object(self.manifest, "evidence manifest", required)
        if self.manifest["schema"] != MANIFEST_SCHEMA:
            _fail("evidence manifest schema is unsupported")
        if self.manifest["schema_version"] != SCHEMA_VERSION:
            _fail("evidence manifest schema version is unsupported")

        self._capture_files(self.manifest["files"])
        self._capture_artifacts(self.manifest["artifacts"])
        artifact_projection = [
            {
                "role": role,
                **_binding(self.files[self.artifacts[role]]),
            }
            for role in ARTIFACT_ROLES
        ]
        source = self._verify_source(self.manifest["source"])
        protocols = self._verify_protocols(self.manifest["protocols"])
        artifact_set_digest = rust_artifact_set_digest(artifact_projection)
        vk_digest = rust_vk_set_digest(artifact_projection, protocols)
        profile_inputs = self._candidate_profile_inputs(self.manifest["profiles"])
        candidate_context, candidate_context_digest = release_candidate_context(
            source_archive=_object(
                source["source_archive"], "candidate source archive", {"sha256", "byte_len"}
            ),
            cargo_lock=_object(
                source["cargo_lock"], "candidate Cargo.lock", {"sha256", "byte_len"}
            ),
            artifacts=artifact_projection,
            artifact_set_digest=artifact_set_digest,
            vk_digest=vk_digest,
            protocols=protocols,
            profile_inputs=profile_inputs,
            observer_policy=_binding(self.observer_policy.info),
        )
        self._capture_commands(
            self.manifest["commands"],
            candidate_context_digest=candidate_context_digest,
        )
        global_reports = self._verify_global_reports(
            self.manifest["global_reports"],
            source=source,
            artifact_set_digest=artifact_set_digest,
        )
        profiles = self._verify_profiles(
            self.manifest["profiles"], protocols=protocols, vk_digest=vk_digest
        )
        enabled_profiles = [profile["profile"] for profile in profiles]
        hardware_policy_digest = rust_hardware_policy_digest(enabled_profiles)
        profile_digest = rust_release_profile_digest(
            global_reports["circuit_shape_report"], protocols
        )
        builds = self._verify_reproducible_builds(
            self.manifest["reproducible_builds"],
            source=source,
            artifact_set_digest=artifact_set_digest,
        )

        if self.used_commands != set(self.commands):
            unused = sorted(set(self.commands) - self.used_commands)
            _fail(f"verifier commands are undeclared by typed reports: {unused!r}")
        if self.used_files != set(self.files):
            unused = sorted(set(self.files) - self.used_files)
            _fail(f"evidence files are declared but semantically unused: {unused!r}")

        command_projection = [
            dict(self.commands[command_id].projection)
            for command_id in sorted(self.commands)
        ]
        self._revalidate_closure()
        encoded_commands = canonical_json_bytes(command_projection)
        verification_records_digest = _raw_domain_digest(
            VERIFICATION_RECORDS_DIGEST_DOMAIN, encoded_commands
        )
        evidence_closure = {
            "evidence_manifest": {
                "sha256": self.manifest_sha256,
                "byte_len": self.manifest_info.size,
            },
            "observer_policy": _binding(self.observer_policy.info),
            "verification_records_digest": verification_records_digest,
            "candidate_context_digest": candidate_context_digest,
            "verification_record_count": len(command_projection),
            "total_evidence_bytes": sum(info.size for info in self.files.values()),
            "total_transcript_bytes": self.total_transcript_bytes,
            "total_command_input_bytes": self.total_command_input_bytes,
            "total_observed_duration_ms": self.total_observed_duration_ms,
            "total_observed_cpu_ms": self.total_observed_cpu_ms,
        }
        receipt_projection = {
            "version": WIRE_VERSION,
            "source_tree_digest": source["source_tree_digest"],
            "cargo_lock_digest": source["cargo_lock_digest"],
            "profile_digest": profile_digest,
            "eq_protocol_digest": protocols["state_eq_protocol_digest"],
            "ep_protocol_digest": protocols["state_ep_protocol_digest"],
            "commit_wrapper_eq_protocol_digest": protocols[
                "commit_wrapper_eq_protocol_digest"
            ],
            "commit_wrapper_ep_protocol_digest": protocols[
                "commit_wrapper_ep_protocol_digest"
            ],
            "artifact_set_digest": artifact_set_digest,
            "hardware_policy_digest": hardware_policy_digest,
            "evidence_closure": evidence_closure,
            **global_reports,
            "profile_qualifications": profiles,
            "helper_protocols": protocols["helper_protocols"],
            "reproducible_builds": builds,
        }
        return {
            "schema": PROJECTION_SCHEMA,
            "schema_version": SCHEMA_VERSION,
            "manifest_sha256": self.manifest_sha256,
            "artifact_inventory": artifact_projection,
            "artifact_inventory_review_sha256": _review_digest(artifact_projection),
            "candidate_context": candidate_context,
            "receipt_projection": receipt_projection,
            "verifier_commands": command_projection,
            "verification_scope": (
                "closed filesystem provenance, exact Rust-compatible release "
                "identities, derived measurements, and threshold-signed observations "
                "from a separately pinned trusted verifier policy; candidate code is "
                "never executed by this verifier"
            ),
        }

    def _capture_files(self, raw_rows: object) -> None:
        rows = _array(raw_rows, "evidence manifest files")
        if not rows or len(rows) > MAX_EVIDENCE_FILES:
            _fail(f"evidence manifest must contain 1 through {MAX_EVIDENCE_FILES} files")
        normalized_paths: list[str] = []
        expected: dict[str, tuple[str, str, int]] = {}
        for index, raw in enumerate(rows):
            row = _object(
                raw,
                f"evidence file row {index}",
                {"path", "kind", "sha256", "byte_len"},
            )
            path = canonical_relative_path(_string(row["path"], "evidence path"))
            kind = _string(row["kind"], f"evidence file {path!r} kind")
            if kind not in FILE_KINDS:
                _fail(f"evidence file {path!r} has unsupported kind {kind!r}")
            digest = _digest(row["sha256"], f"evidence file {path!r} SHA-256")
            byte_len = _integer(
                row["byte_len"],
                f"evidence file {path!r} length",
                minimum=1,
                maximum=_max_for_kind(kind),
            )
            if path in expected:
                _fail(f"evidence file path is duplicated: {path!r}")
            expected[path] = (kind, digest, byte_len)
            normalized_paths.append(path)
        if normalized_paths != sorted(normalized_paths):
            _fail("evidence file rows must be sorted by canonical path")
        total_declared_bytes = sum(value[2] for value in expected.values())
        if total_declared_bytes > MAX_TOTAL_EVIDENCE_BYTES:
            _fail("closed evidence exceeds the 6-GiB aggregate byte cap")
        retained_declared_bytes = sum(
            byte_len
            for kind, _, byte_len in expected.values()
            if kind in {"event_log", "report", "observation"}
        )
        if retained_declared_bytes > MAX_RETAINED_PAYLOAD_BYTES:
            _fail("retained evidence payloads exceed the 128-MiB aggregate byte cap")

        inventory = _scan_evidence_tree(self.root)
        if inventory != normalized_paths:
            missing = sorted(set(normalized_paths) - set(inventory))
            extra = sorted(set(inventory) - set(normalized_paths))
            _fail(
                "evidence root is not closed over the manifest; "
                f"missing={missing!r}, undeclared={extra!r}"
            )

        identities: set[tuple[int, int]] = set()
        for path in normalized_paths:
            kind, expected_sha, expected_len = expected[path]
            retain = kind in {"event_log", "report", "observation"}
            info, payload = stable_read_relative(
                self.root,
                path,
                max_size=_max_for_kind(kind),
                return_payload=retain,
            )
            if info.sha256 != expected_sha or info.size != expected_len:
                _fail(f"evidence file {path!r} digest or length does not match")
            identity = (info.device, info.inode)
            if identity in identities:
                _fail(f"evidence file {path!r} aliases another declared file")
            identities.add(identity)
            self.files[path] = info
            self.file_kinds[path] = kind
            if payload is not None:
                self.payloads[path] = payload

    def _capture_artifacts(self, raw_rows: object) -> None:
        rows = _array(raw_rows, "artifact inventory")
        if len(rows) != len(ARTIFACT_ROLES):
            _fail("artifact inventory must contain exactly the 26 V1 roles")
        for index, (raw, expected_role) in enumerate(zip(rows, ARTIFACT_ROLES)):
            row = _object(raw, f"artifact row {index}", {"role", "path"})
            if row["role"] != expected_role:
                _fail("artifact inventory is not in the canonical V1 role order")
            path = self._path(row["path"], "artifact")
            self._expect_kind(path, "artifact")
            if path in self.artifacts.values():
                _fail(f"artifact path is reused by multiple roles: {path!r}")
            self.artifacts[expected_role] = path
            self.used_files.add(path)

        for role in ("params_eq", "params_ep"):
            if self.files[self.artifacts[role]].size != 4_194_372:
                _fail(f"artifact {role!r} must be exactly 4194372 bytes")
        for role in ("state_pk_eq", "state_pk_ep"):
            if self.files[self.artifacts[role]].size > 48_234_934:
                _fail(f"artifact {role!r} exceeds the state proving-key limit")
        for role in ARTIFACT_ROLES:
            size = self.files[self.artifacts[role]].size
            if role.endswith("_vk_eq") or role.endswith("_vk_ep"):
                if size > 64 * 1024:
                    _fail(f"artifact {role!r} exceeds the verifying-key limit")
            elif "_pk_" in role and not role.startswith("state_pk_"):
                if size > 64 * 1024 * 1024:
                    _fail(f"artifact {role!r} exceeds the helper proving-key limit")
        if sum(self.files[path].size for path in self.artifacts.values()) > 512 * 1024 * 1024:
            _fail("complete artifact inventory exceeds the 512-MiB limit")
        artifact_digests = [
            self.files[self.artifacts[role]].sha256 for role in ARTIFACT_ROLES
        ]
        if len(set(artifact_digests)) != len(artifact_digests):
            _fail("every artifact role must bind distinct file bytes")

    def _candidate_profile_inputs(self, raw_rows: object) -> list[dict[str, object]]:
        rows = _array(raw_rows, "candidate profile inputs")
        profile_fields = {
            "hardware_profile",
            "suite_id",
            "qualification_report",
            "relations",
            "helpers",
            "recursive_depths",
            "aggregate_balance",
            "thermal",
            "envelope",
            "acceptance_cases",
        }
        result: list[dict[str, object]] = []
        for index, raw in enumerate(rows):
            row = _object(raw, f"candidate profile input {index}", profile_fields)
            hardware = _object(
                row["hardware_profile"],
                f"candidate hardware profile {index}",
                _HARDWARE_PROFILE_FIELDS,
            )
            result.append(
                {
                    "hardware_profile": dict(hardware),
                    "suite_id": _digest(row["suite_id"], f"candidate profile {index} suite id"),
                }
            )
        return result

    def _capture_commands(
        self, raw_rows: object, *, candidate_context_digest: str
    ) -> None:
        rows = _array(raw_rows, "verifier commands")
        if not rows or len(rows) > MAX_COMMANDS:
            _fail(f"verifier observations must contain 1 through {MAX_COMMANDS} commands")
        command_ids: list[str] = []
        for index, raw in enumerate(rows):
            row = _object(
                raw,
                f"verifier command {index}",
                {
                    "id",
                    "verifier_id",
                    "report_schema",
                    "arguments",
                    "stdout",
                    "stderr",
                    "observation",
                },
            )
            command_id = _safe_id(row["id"], f"verifier command {index} id")
            if command_id in self.commands:
                _fail(f"verifier command id is duplicated: {command_id!r}")
            verifier_id = _safe_id(row["verifier_id"], f"verifier command {index} verifier id")
            trusted = self.observer_policy.verifiers.get(verifier_id)
            if trusted is None:
                _fail(f"verifier command {command_id!r} is not in the trusted local allowlist")
            report_schema = _string(
                row["report_schema"], f"verifier command {command_id!r} report schema"
            )
            if report_schema not in trusted.report_schemas:
                _fail(f"verifier command {command_id!r} is not allowed for its report schema")
            args_raw = _array(row["arguments"], f"verifier command {command_id!r} arguments")
            if not args_raw or len(args_raw) > 64:
                _fail(f"verifier command {command_id!r} must have 1 through 64 arguments")
            arguments: list[tuple[str, str]] = []
            projected_arguments: list[dict[str, object]] = []
            file_arguments: set[str] = set()
            for arg_index, raw_argument in enumerate(args_raw):
                if not isinstance(raw_argument, Mapping) or len(raw_argument) != 1:
                    _fail(
                        f"verifier command {command_id!r} argument {arg_index} "
                        "must contain exactly one of literal or file"
                    )
                if "literal" in raw_argument:
                    literal = _string(
                        raw_argument["literal"],
                        f"verifier command {command_id!r} literal",
                    )
                    if _SAFE_LITERAL.fullmatch(literal) is None:
                        _fail(
                            f"verifier command {command_id!r} contains an unsafe literal"
                        )
                    arguments.append(("literal", literal))
                    projected_arguments.append({"literal": literal})
                elif "file" in raw_argument:
                    path = self._path(
                        raw_argument["file"],
                        f"verifier command {command_id!r} file argument",
                    )
                    if path in file_arguments:
                        _fail(
                            f"verifier command {command_id!r} repeats an input file"
                        )
                    file_arguments.add(path)
                    arguments.append(("file", path))
                    projected_arguments.append(
                        {"file": path, **_binding(self.files[path])}
                    )
                else:
                    _fail(
                        f"verifier command {command_id!r} argument must be literal or file"
                    )
            stdout = self._path(row["stdout"], f"verifier command {command_id!r} stdout")
            stderr = self._path(row["stderr"], f"verifier command {command_id!r} stderr")
            observation_path = self._path(
                row["observation"], f"verifier command {command_id!r} observation"
            )
            self._expect_kind(stdout, "transcript")
            self._expect_kind(stderr, "transcript")
            self._expect_kind(observation_path, "observation")
            if len({stdout, stderr, observation_path}) != 3:
                _fail("stdout, stderr, and observation must be distinct files")
            self.used_files.update((stdout, stderr, observation_path))

            observed = load_json_object(
                self.payloads[observation_path], f"verification observation {observation_path!r}"
            )
            observed = _object(
                observed,
                f"verification observation {command_id!r}",
                {"schema", "schema_version", "subject", "approvals"},
            )
            if (
                observed["schema"] != OBSERVATION_SCHEMA
                or observed["schema_version"] != SCHEMA_VERSION
                or canonical_json_bytes(dict(observed)) != self.payloads[observation_path]
            ):
                _fail(f"verification observation {command_id!r} is not canonical supported V1 JSON")
            subject = _object(
                observed["subject"],
                f"verification observation {command_id!r} subject",
                {
                    "command_id",
                    "verifier_id",
                    "verifier_sha256",
                    "candidate_context_digest",
                    "report_schema",
                    "arguments",
                    "exit_code",
                    "stdout",
                    "stderr",
                    "started_at_ms",
                    "duration_ms",
                    "cpu_millis",
                    "peak_rss_bytes",
                },
            )
            expected_static = {
                "command_id": command_id,
                "verifier_id": verifier_id,
                "verifier_sha256": trusted.sha256,
                "candidate_context_digest": candidate_context_digest,
                "report_schema": report_schema,
                "arguments": projected_arguments,
                "exit_code": 0,
                "stdout": _binding(self.files[stdout]),
                "stderr": _binding(self.files[stderr]),
            }
            if any(subject[name] != value for name, value in expected_static.items()):
                _fail(f"verification observation {command_id!r} substitutes its trusted subject")
            started_at_ms = _integer(
                subject["started_at_ms"], f"verification observation {command_id!r} start", minimum=1
            )
            duration_ms = _integer(
                subject["duration_ms"],
                f"verification observation {command_id!r} duration",
                minimum=1,
                maximum=MAX_OBSERVATION_DURATION_MS,
            )
            cpu_millis = _integer(
                subject["cpu_millis"],
                f"verification observation {command_id!r} CPU",
                minimum=1,
                maximum=MAX_OBSERVATION_CPU_MS,
            )
            peak_rss_bytes = _integer(
                subject["peak_rss_bytes"],
                f"verification observation {command_id!r} RSS",
                minimum=1,
                maximum=MAX_OBSERVATION_RSS_BYTES,
            )
            del started_at_ms, peak_rss_bytes
            approvals = _array(observed["approvals"], f"verification observation {command_id!r} approvals")
            if not approvals or len(approvals) > len(self.observer_policy.authorities):
                _fail(f"verification observation {command_id!r} has an invalid approval count")
            approval_ids: list[str] = []
            message = _approval_message(subject)
            for approval_index, raw_approval in enumerate(approvals):
                approval = _object(
                    raw_approval,
                    f"verification observation {command_id!r} approval {approval_index}",
                    {"authority_id", "signature"},
                )
                authority_id = _digest(
                    approval["authority_id"],
                    f"verification observation {command_id!r} approval authority",
                )
                public_key = self.observer_policy.authorities.get(authority_id)
                signature = _hex_bytes(
                    approval["signature"],
                    f"verification observation {command_id!r} approval signature",
                    size=64,
                )
                if public_key is None or not _ed25519_verify(public_key, message, signature):
                    _fail(f"verification observation {command_id!r} has an invalid approval")
                approval_ids.append(authority_id)
            if approval_ids != sorted(set(approval_ids)):
                _fail(f"verification observation {command_id!r} approvals are not uniquely sorted")
            if len(approval_ids) < self.observer_policy.threshold:
                _fail(f"verification observation {command_id!r} lacks the trusted threshold")

            transcript_bytes = self.files[stdout].size + self.files[stderr].size
            command_input_bytes = sum(self.files[path].size for path in file_arguments)
            self.total_transcript_bytes += transcript_bytes
            self.total_command_input_bytes += command_input_bytes
            self.total_observed_duration_ms += duration_ms
            self.total_observed_cpu_ms += cpu_millis
            self.commands[command_id] = CommandSpec(
                command_id=command_id,
                verifier_id=verifier_id,
                verifier_sha256=trusted.sha256,
                report_schema=report_schema,
                arguments=tuple(arguments),
                stdout=stdout,
                stderr=stderr,
                observation=observation_path,
                projection={
                    **dict(subject),
                    "observation": _binding(self.files[observation_path]),
                    "approved_by": approval_ids,
                },
            )
            command_ids.append(command_id)
        if command_ids != sorted(command_ids):
            _fail("verifier commands must be sorted by id")
        if self.total_transcript_bytes > MAX_TOTAL_TRANSCRIPT_BYTES:
            _fail("verification transcripts exceed the 64-MiB aggregate cap")
        if self.total_command_input_bytes > MAX_TOTAL_COMMAND_INPUT_BYTES:
            _fail("verification inputs exceed the 48-GiB aggregate command cap")
        if self.total_observed_duration_ms > MAX_TOTAL_OBSERVED_DURATION_MS:
            _fail("verification observations exceed the 24-hour aggregate wall-time cap")
        if self.total_observed_cpu_ms > MAX_TOTAL_OBSERVED_CPU_MS:
            _fail("verification observations exceed the 24-hour aggregate CPU cap")

    def _verify_source(self, raw: object) -> dict[str, object]:
        source = _object(raw, "source binding", {"source_archive", "cargo_lock"})
        archive = self._path(source["source_archive"], "source archive")
        cargo_lock = self._path(source["cargo_lock"], "Cargo.lock evidence")
        self._expect_kind(archive, "source_archive")
        self._expect_kind(cargo_lock, "cargo_lock")
        self.used_files.update((archive, cargo_lock))
        return {
            "source_tree_digest": self.files[archive].sha256,
            "source_archive": _binding(self.files[archive]),
            "cargo_lock_digest": self.files[cargo_lock].sha256,
            "cargo_lock": _binding(self.files[cargo_lock]),
        }

    def _verify_protocols(self, raw: object) -> dict[str, object]:
        protocols = _object(
            raw,
            "compiled protocols",
            {
                "state_eq_protocol_digest",
                "state_ep_protocol_digest",
                "commit_wrapper_eq_protocol_digest",
                "commit_wrapper_ep_protocol_digest",
                "helper_protocols",
            },
        )
        result: dict[str, object] = {}
        all_digests: list[str] = []
        for name in (
            "state_eq_protocol_digest",
            "state_ep_protocol_digest",
            "commit_wrapper_eq_protocol_digest",
            "commit_wrapper_ep_protocol_digest",
        ):
            value = _digest(protocols[name], f"compiled protocol {name}")
            result[name] = value
            all_digests.append(value)
        helper_rows = _array(protocols["helper_protocols"], "helper protocols")
        if len(helper_rows) != len(HELPERS):
            _fail("compiled helper protocols must contain exactly four rows")
        helper_projection: list[dict[str, object]] = []
        for index, (raw_row, expected_helper) in enumerate(zip(helper_rows, HELPERS)):
            row = _object(
                raw_row,
                f"helper protocol {index}",
                {
                    "helper",
                    "eq_protocol_digest",
                    "ep_protocol_digest",
                    "eq_proof_bytes",
                    "ep_proof_bytes",
                },
            )
            if row["helper"] != expected_helper:
                _fail("compiled helper protocols are not in canonical order")
            eq = _digest(row["eq_protocol_digest"], f"{expected_helper} Eq protocol")
            ep = _digest(row["ep_protocol_digest"], f"{expected_helper} Ep protocol")
            eq_proof_bytes = _integer(
                row["eq_proof_bytes"],
                f"{expected_helper} Eq exact proof length",
                maximum=(1 << 32) - 1,
            )
            ep_proof_bytes = _integer(
                row["ep_proof_bytes"],
                f"{expected_helper} Ep exact proof length",
                maximum=(1 << 32) - 1,
            )
            if expected_helper in INTERNAL_PROOF_HELPERS:
                if (
                    eq_proof_bytes == 0
                    or ep_proof_bytes == 0
                    or eq_proof_bytes % 32 != 0
                    or ep_proof_bytes % 32 != 0
                    or eq_proof_bytes + ep_proof_bytes > (1 << 32) - 1
                ):
                    _fail(
                        f"{expected_helper} exact internal proof lengths must be "
                        "nonzero 32-byte multiples with a bounded sum"
                    )
            elif eq_proof_bytes != 0 or ep_proof_bytes != 0:
                _fail(
                    f"{expected_helper} is a paired-wire helper and must not "
                    "declare internal exact proof lengths"
                )
            all_digests.extend((eq, ep))
            helper_projection.append(
                {
                    "helper": expected_helper,
                    "eq_protocol_digest": eq,
                    "ep_protocol_digest": ep,
                    "eq_proof_bytes": eq_proof_bytes,
                    "ep_proof_bytes": ep_proof_bytes,
                }
            )
        if len(set(all_digests)) != len(all_digests):
            _fail("state, wrapper, and helper protocol digests must all be distinct")
        result["helper_protocols"] = helper_projection
        return result

    def _verify_global_reports(
        self,
        raw: object,
        *,
        source: Mapping[str, object],
        artifact_set_digest: str,
    ) -> dict[str, object]:
        reports = _object(
            raw,
            "global reports",
            {"circuit_shape", "security_review", "kat", "fuzz", "resource"},
        )
        paths = {
            name: self._path(reports[name], f"global {name} report")
            for name in sorted(reports)
        }

        shape = self._report(
            paths["circuit_shape"],
            "iroha.kagemusha_v1.circuit_shape_report",
            {"k", "relations", "helpers"},
        )
        if shape["k"] != HALO2_K:
            _fail("circuit-shape report must exercise k = 16")
        relation_shapes = self._shape_rows(shape["relations"], RELATIONS, "relation")
        helper_shapes = self._shape_rows(shape["helpers"], HELPERS, "helper")
        self.circuit_shapes = {
            "relations": relation_shapes,
            "helpers": helper_shapes,
        }
        self._bind_report_command(paths["circuit_shape"], shape, {paths["circuit_shape"]})

        security = self._report(
            paths["security_review"],
            "iroha.kagemusha_v1.security_review_report",
            {"source_tree_sha256", "artifact_set_digest", "approved"},
        )
        if security["source_tree_sha256"] != source["source_tree_digest"]:
            _fail("security-review report names a different source tree")
        if security["artifact_set_digest"] != artifact_set_digest:
            _fail("security-review report names a different artifact set")
        _true(security["approved"], "security-review approval")
        self._bind_report_command(paths["security_review"], security, {paths["security_review"]})

        kat = self._report(
            paths["kat"],
            "iroha.kagemusha_v1.kat_report",
            {"positive_cases", "adversarial_cases", "failures"},
        )
        _integer(kat["positive_cases"], "KAT positive cases", minimum=1)
        _integer(kat["adversarial_cases"], "KAT adversarial cases", minimum=1)
        if kat["failures"] != 0:
            _fail("KAT report contains failures")
        self._bind_report_command(paths["kat"], kat, {paths["kat"]})

        fuzz = self._report(
            paths["fuzz"],
            "iroha.kagemusha_v1.fuzz_report",
            {"cases_executed", "failures"},
        )
        fuzz_cases = _integer(
            fuzz["cases_executed"], "fuzz cases", minimum=MIN_FUZZ_CASES
        )
        if fuzz["failures"] != 0:
            _fail("fuzz report contains failures")
        self._bind_report_command(paths["fuzz"], fuzz, {paths["fuzz"]})

        resource = self._report(
            paths["resource"],
            "iroha.kagemusha_v1.resource_report",
            {"process_rss_bytes", "passed"},
        )
        _integer(
            resource["process_rss_bytes"],
            "resource report RSS",
            minimum=1,
            maximum=PROCESS_RSS_MAX_BYTES,
        )
        _true(resource["passed"], "resource report passed")
        self._bind_report_command(paths["resource"], resource, {paths["resource"]})

        return {
            "circuit_shape_report": _binding(self.files[paths["circuit_shape"]]),
            "security_review_report": _binding(self.files[paths["security_review"]]),
            "kat_report": _binding(self.files[paths["kat"]]),
            "fuzz_report": _binding(self.files[paths["fuzz"]]),
            "resource_report": _binding(self.files[paths["resource"]]),
            "fuzz_cases": fuzz_cases,
        }

    def _shape_rows(
        self, raw: object, expected_names: Sequence[str], discriminator: str
    ) -> dict[str, tuple[int, int]]:
        rows = _array(raw, f"circuit-shape {discriminator} rows")
        if len(rows) != len(expected_names):
            _fail(f"circuit-shape report has the wrong {discriminator} count")
        result: dict[str, tuple[int, int]] = {}
        for index, (raw_row, expected) in enumerate(zip(rows, expected_names)):
            row = _object(
                raw_row,
                f"circuit-shape {discriminator} row {index}",
                {discriminator, "eq_circuit_rows", "ep_circuit_rows"},
            )
            if row[discriminator] != expected:
                _fail(f"circuit-shape {discriminator} rows are not canonical")
            eq = _integer(
                row["eq_circuit_rows"],
                f"{expected} Eq circuit rows",
                minimum=1,
                maximum=MAX_CIRCUIT_ROWS,
            )
            ep = _integer(
                row["ep_circuit_rows"],
                f"{expected} Ep circuit rows",
                minimum=1,
                maximum=MAX_CIRCUIT_ROWS,
            )
            result[expected] = (eq, ep)
        return result

    def _verify_profiles(
        self,
        raw: object,
        *,
        protocols: Mapping[str, object],
        vk_digest: str,
    ) -> list[dict[str, object]]:
        rows = _array(raw, "profile qualifications")
        if not rows or len(rows) > 64:
            _fail("profile qualifications must contain 1 through 64 profiles")
        result: list[dict[str, object]] = []
        ids: list[str] = []
        for index, raw_profile in enumerate(rows):
            projection = self._verify_profile(
                raw_profile, index=index, protocols=protocols, vk_digest=vk_digest
            )
            profile_id = str(projection["profile"]["hardware_profile_id"])
            ids.append(profile_id)
            result.append(projection)
        if ids != sorted(set(ids)):
            _fail("profile qualifications must be uniquely sorted by hardware profile id")
        return result

    def _verify_profile(
        self,
        raw: object,
        *,
        index: int,
        protocols: Mapping[str, object],
        vk_digest: str,
    ) -> dict[str, object]:
        fields = {
            "hardware_profile",
            "suite_id",
            "qualification_report",
            "relations",
            "helpers",
            "recursive_depths",
            "aggregate_balance",
            "thermal",
            "envelope",
            "acceptance_cases",
        }
        profile = _object(raw, f"profile qualification {index}", fields)
        hardware_profile = _object(
            profile["hardware_profile"],
            f"profile {index} embedded hardware profile",
            _HARDWARE_PROFILE_FIELDS,
        )
        profile_id = _digest(
            hardware_profile["hardware_profile_id"], f"profile {index} hardware profile id"
        )
        expected_profile_id = rust_hardware_profile_id(hardware_profile)
        suite_id = _digest(profile["suite_id"], f"profile {profile_id} suite id")
        policy_epoch = _integer(
            hardware_profile["policy_epoch"], f"profile {profile_id} policy epoch", minimum=1
        )
        if (
            expected_profile_id != profile_id
            or int(hardware_profile["version"]) != WIRE_VERSION
            or int(hardware_profile["protocol_version"]) != WIRE_VERSION
            or int(hardware_profile["capability_mask"]) != (1 << 16) - 1
            or int(hardware_profile["valid_from_ms"]) >= int(hardware_profile["expires_at_ms"])
            or hardware_profile["allowed_suite_commitment"] != _suite_commitment(suite_id)
        ):
            _fail(
                f"profile {profile_id} is not the exact Rust-derived qualified hardware/VK binding"
            )
        for field in (
            "provider_id",
            "product_class_digest",
            "firmware_policy_digest",
            "enrollment_attestation_verifier_digest",
            "attestation_trust_roots_digest",
            "allowed_suite_commitment",
            "qualification_report_digest",
        ):
            _digest(hardware_profile[field], f"profile {profile_id} {field}")
        profile_projection: dict[str, object] = {
            "hardware_profile": dict(hardware_profile),
            "hardware_profile_id": profile_id,
            "suite_id": suite_id,
            "vk_digest": vk_digest,
            "qualification_digest": "0" * 64,
            "policy_epoch": policy_epoch,
        }

        qualification_path = self._path(
            profile["qualification_report"], f"profile {profile_id} qualification report"
        )
        qualification = self._report(
            qualification_path,
            "iroha.kagemusha_v1.hardware_profile_qualification_report",
            {"provider_id", "policy_epoch", "physical_checks", "passed"},
        )
        if (
            qualification["provider_id"] != hardware_profile["provider_id"]
            or qualification["policy_epoch"] != policy_epoch
        ):
            _fail("physical qualification report names a different hardware policy body")
        if qualification["physical_checks"] != list(PHYSICAL_PROFILE_CHECKS):
            _fail("physical qualification report omits a required physical check")
        _true(qualification["passed"], "physical qualification passed")
        self._bind_report_command(qualification_path, qualification, {qualification_path})
        profile_projection["qualification_report"] = _binding(
            self.files[qualification_path]
        )
        if (
            hardware_profile["qualification_report_digest"]
            != profile_projection["qualification_report"]["sha256"]
        ):
            _fail("hardware profile does not bind its exact physical qualification report")

        relation_rows = _array(profile["relations"], f"profile {profile_id} relations")
        if len(relation_rows) != len(RELATIONS):
            _fail("profile relation matrix must contain exactly nine rows")
        relations: list[dict[str, object]] = []
        for relation_index, (raw_row, expected_relation) in enumerate(
            zip(relation_rows, RELATIONS)
        ):
            row = _object(
                raw_row,
                f"profile relation row {relation_index}",
                {"relation", "report"},
            )
            if row["relation"] != expected_relation:
                _fail("profile relation matrix is not in canonical order")
            report_path = self._path(row["report"], f"{expected_relation} report")
            relations.append(
                self._verify_relation(
                    report_path,
                    profile_id=profile_id,
                    relation=expected_relation,
                    protocols=protocols,
                )
            )

        helper_rows = _array(profile["helpers"], f"profile {profile_id} helpers")
        if len(helper_rows) != len(HELPERS):
            _fail("profile helper matrix must contain exactly four rows")
        helpers: list[dict[str, object]] = []
        for helper_index, (raw_row, expected_helper) in enumerate(zip(helper_rows, HELPERS)):
            row = _object(
                raw_row,
                f"profile helper row {helper_index}",
                {"helper", "report"},
            )
            if row["helper"] != expected_helper:
                _fail("profile helper matrix is not in canonical order")
            report_path = self._path(row["report"], f"{expected_helper} helper report")
            helpers.append(
                self._verify_helper(
                    report_path,
                    profile_id=profile_id,
                    helper=expected_helper,
                    protocols=protocols,
                )
            )

        depth_rows = _array(
            profile["recursive_depths"], f"profile {profile_id} recursive depths"
        )
        if len(depth_rows) != 4:
            _fail("recursive-depth matrix must contain exactly four rows")
        depth_values = []
        depths: list[dict[str, object]] = []
        for depth_index, raw_row in enumerate(depth_rows):
            row = _object(
                raw_row,
                f"recursive-depth row {depth_index}",
                {"depth", "report"},
            )
            depth = _integer(row["depth"], "recursive depth", minimum=1)
            depth_values.append(depth)
            report_path = self._path(row["report"], "recursive-depth report")
            depths.append(
                self._verify_depth(
                    report_path,
                    profile_id=profile_id,
                    depth=depth,
                    protocols=protocols,
                )
            )
        if depth_values[:3] != [8, 64, 1024] or depth_values[3] <= 1024:
            _fail("recursive depths must be exactly 8, 64, 1024, and one greater depth")
        invariant = {
            (
                row["complete_proof_bytes"],
                row["raw_session_bytes"],
                row["text_session_bytes"],
            )
            for row in depths
        }
        if len(invariant) != 1:
            _fail("proof/raw/text byte measurements must remain invariant across depth")

        aggregate_path = self._path(profile["aggregate_balance"], "aggregate-balance report")
        aggregate = self._verify_aggregate(
            aggregate_path, profile_id=profile_id, protocols=protocols
        )
        thermal_path = self._path(profile["thermal"], "thermal report")
        thermal = self._verify_thermal(
            thermal_path, profile_id=profile_id, protocols=protocols
        )
        envelope_path = self._path(profile["envelope"], "envelope report")
        envelope = self._verify_envelope(envelope_path, profile_id=profile_id)
        depth_measurement = next(iter(invariant))
        if (
            envelope["raw_session_bytes"] != depth_measurement[1]
            or envelope["text_session_bytes"] != depth_measurement[2]
        ):
            _fail("envelope and invariant-depth session measurements differ")

        acceptance_rows = _array(
            profile["acceptance_cases"], f"profile {profile_id} acceptance cases"
        )
        if len(acceptance_rows) != len(ACCEPTANCE_CASES):
            _fail("every profile must contain exactly the 45 acceptance cases")
        acceptance: list[dict[str, object]] = []
        for case_index, (raw_row, expected_case) in enumerate(
            zip(acceptance_rows, ACCEPTANCE_CASES)
        ):
            row = _object(
                raw_row,
                f"acceptance case {case_index}",
                {"case", "report"},
            )
            if row["case"] != expected_case:
                _fail("acceptance cases are not in the canonical 45-case order")
            report_path = self._path(row["report"], f"{expected_case} report")
            acceptance.append(
                self._verify_acceptance(
                    report_path, profile_id=profile_id, case=expected_case
                )
            )

        projection = {
            "profile": profile_projection,
            "relations": relations,
            "helper_circuits": helpers,
            "recursive_depths": depths,
            "aggregate_balance": aggregate,
            "thermal": thermal,
            "envelope": envelope,
            "acceptance_cases": acceptance,
        }
        qualification_digest = rust_profile_qualification_digest(projection)
        profile_projection["qualification_digest"] = qualification_digest
        return projection

    def _verify_relation(
        self,
        path: str,
        *,
        profile_id: str,
        relation: str,
        protocols: Mapping[str, object],
    ) -> dict[str, object]:
        report = self._report(
            path,
            "iroha.kagemusha_v1.relation_qualification_report",
            {
                "hardware_profile_id",
                "relation",
                "eq_protocol_digest",
                "ep_protocol_digest",
                "eq_verifying_key",
                "ep_verifying_key",
                "eq_circuit_rows",
                "ep_circuit_rows",
                "proof",
                "prove_p95_ms",
                "verify_p95_ms",
                "process_rss_bytes",
                "operation_energy_millijoules",
            },
        )
        self._profile_and_name(report, profile_id, "relation", relation)
        if relation in {"acceptance_intent_authorization", "commit_wrapper"}:
            eq_protocol = protocols["commit_wrapper_eq_protocol_digest"]
            ep_protocol = protocols["commit_wrapper_ep_protocol_digest"]
            eq_role, ep_role = "commit_wrapper_vk_eq", "commit_wrapper_vk_ep"
        else:
            eq_protocol = protocols["state_eq_protocol_digest"]
            ep_protocol = protocols["state_ep_protocol_digest"]
            eq_role, ep_role = "state_vk_eq", "state_vk_ep"
        self._protocol_and_keys(
            report,
            eq_protocol=str(eq_protocol),
            ep_protocol=str(ep_protocol),
            eq_role=eq_role,
            ep_role=ep_role,
        )
        eq_rows, ep_rows = self._measurement_rows(report, relation, "relations")
        proof = self._sample(report["proof"], "proof", f"relation:{profile_id}:{relation}")
        metrics = self._performance_metrics(report, f"relation {relation}")
        requirements = {
            path,
            proof,
            self.artifacts[eq_role],
            self.artifacts[ep_role],
        }
        self._bind_report_command(path, report, requirements)
        return {
            "relation": relation,
            "eq_protocol_digest": eq_protocol,
            "ep_protocol_digest": ep_protocol,
            "eq_verifying_key": self._artifact_binding(eq_role),
            "ep_verifying_key": self._artifact_binding(ep_role),
            "eq_circuit_rows": eq_rows,
            "ep_circuit_rows": ep_rows,
            "complete_proof_bytes": self.files[proof].size,
            **metrics,
            "report": _binding(self.files[path]),
        }

    def _verify_helper(
        self,
        path: str,
        *,
        profile_id: str,
        helper: str,
        protocols: Mapping[str, object],
    ) -> dict[str, object]:
        proof_fields = (
            {"eq_proof", "ep_proof"}
            if helper in INTERNAL_PROOF_HELPERS
            else {"proof"}
        )
        report = self._report(
            path,
            "iroha.kagemusha_v1.helper_qualification_report",
            {
                "hardware_profile_id",
                "helper",
                "eq_protocol_digest",
                "ep_protocol_digest",
                "eq_verifying_key",
                "ep_verifying_key",
                "eq_circuit_rows",
                "ep_circuit_rows",
                "prove_p95_ms",
                "verify_p95_ms",
                "process_rss_bytes",
                "operation_energy_millijoules",
            }
            | proof_fields,
        )
        self._profile_and_name(report, profile_id, "helper", helper)
        helper_protocols = {
            row["helper"]: row for row in protocols["helper_protocols"]  # type: ignore[index]
        }
        protocol = helper_protocols[helper]
        eq_role = f"{helper}_vk_eq"
        ep_role = f"{helper}_vk_ep"
        self._protocol_and_keys(
            report,
            eq_protocol=protocol["eq_protocol_digest"],
            ep_protocol=protocol["ep_protocol_digest"],
            eq_role=eq_role,
            ep_role=ep_role,
        )
        eq_rows, ep_rows = self._measurement_rows(report, helper, "helpers")
        metrics = self._performance_metrics(report, f"helper {helper}")
        requirements = {path, self.artifacts[eq_role], self.artifacts[ep_role]}
        if helper in INTERNAL_PROOF_HELPERS:
            eq_proof = self._sample(
                report["eq_proof"],
                "internal_proof",
                f"helper:{profile_id}:{helper}:eq",
            )
            ep_proof = self._sample(
                report["ep_proof"],
                "internal_proof",
                f"helper:{profile_id}:{helper}:ep",
            )
            eq_proof_bytes = self.files[eq_proof].size
            ep_proof_bytes = self.files[ep_proof].size
            if (
                eq_proof_bytes != protocol["eq_proof_bytes"]
                or ep_proof_bytes != protocol["ep_proof_bytes"]
            ):
                _fail(
                    f"{helper} internal proof evidence does not match its "
                    "release-pinned per-parity lengths"
                )
            complete_proof_bytes = eq_proof_bytes + ep_proof_bytes
            if complete_proof_bytes > (1 << 32) - 1:
                _fail(f"{helper} internal proof evidence length sum exceeds u32")
            requirements.update((eq_proof, ep_proof))
        else:
            proof = self._sample(
                report["proof"], "proof", f"helper:{profile_id}:{helper}"
            )
            eq_proof_bytes = 0
            ep_proof_bytes = 0
            complete_proof_bytes = self.files[proof].size
            requirements.add(proof)
        self._bind_report_command(
            path,
            report,
            requirements,
        )
        return {
            "helper": helper,
            "eq_protocol_digest": protocol["eq_protocol_digest"],
            "ep_protocol_digest": protocol["ep_protocol_digest"],
            "eq_verifying_key": self._artifact_binding(eq_role),
            "ep_verifying_key": self._artifact_binding(ep_role),
            "eq_circuit_rows": eq_rows,
            "ep_circuit_rows": ep_rows,
            "eq_proof_bytes": eq_proof_bytes,
            "ep_proof_bytes": ep_proof_bytes,
            "complete_proof_bytes": complete_proof_bytes,
            **metrics,
            "report": _binding(self.files[path]),
        }

    def _verify_depth(
        self,
        path: str,
        *,
        profile_id: str,
        depth: int,
        protocols: Mapping[str, object],
    ) -> dict[str, object]:
        report = self._report(
            path,
            "iroha.kagemusha_v1.recursive_depth_report",
            {
                "hardware_profile_id",
                "depth",
                "eq_protocol_digest",
                "ep_protocol_digest",
                "eq_verifying_key",
                "ep_verifying_key",
                "proof",
                "raw_session",
                "text_session",
                "handoff_log",
            },
        )
        if report["hardware_profile_id"] != profile_id or report["depth"] != depth:
            _fail("recursive-depth report binding differs from its matrix row")
        self._protocol_and_keys(
            report,
            eq_protocol=str(protocols["state_eq_protocol_digest"]),
            ep_protocol=str(protocols["state_ep_protocol_digest"]),
            eq_role="state_vk_eq",
            ep_role="state_vk_ep",
        )
        owner = f"depth:{profile_id}:{depth}"
        proof = self._sample(report["proof"], "proof", owner)
        raw_session = self._sample(report["raw_session"], "raw_session", owner)
        text_session = self._sample(report["text_session"], "text_session", owner)
        handoff_log = self._sample(report["handoff_log"], "event_log", owner)
        rows = self._json_lines(handoff_log, "recursive handoff log")
        for expected_index, row in enumerate(rows, start=1):
            typed = _object(
                row,
                f"recursive handoff {expected_index}",
                {"index", "result"},
            )
            if typed["index"] != expected_index or typed["result"] != "verified":
                _fail("recursive handoff log is not a contiguous verified sequence")
        if len(rows) != depth:
            _fail("recursive handoff count must be derived equal to the required depth")
        self._bind_report_command(
            path,
            report,
            {
                path,
                proof,
                raw_session,
                text_session,
                handoff_log,
                self.artifacts["state_vk_eq"],
                self.artifacts["state_vk_ep"],
            },
        )
        return {
            "depth": depth,
            "verified_handoffs": len(rows),
            "complete_proof_bytes": self.files[proof].size,
            "raw_session_bytes": self.files[raw_session].size,
            "text_session_bytes": self.files[text_session].size,
            "report": _binding(self.files[path]),
        }

    def _verify_aggregate(
        self,
        path: str,
        *,
        profile_id: str,
        protocols: Mapping[str, object],
    ) -> dict[str, object]:
        report = self._report(
            path,
            "iroha.kagemusha_v1.aggregate_balance_report",
            {
                "hardware_profile_id",
                "eq_protocol_digest",
                "ep_protocol_digest",
                "eq_verifying_key",
                "ep_verifying_key",
                "proof",
                "events",
            },
        )
        if report["hardware_profile_id"] != profile_id:
            _fail("aggregate-balance report names a different profile")
        self._protocol_and_keys(
            report,
            eq_protocol=str(protocols["state_eq_protocol_digest"]),
            ep_protocol=str(protocols["state_ep_protocol_digest"]),
            eq_role="state_vk_eq",
            ep_role="state_vk_ep",
        )
        owner = f"aggregate:{profile_id}"
        proof = self._sample(report["proof"], "proof", owner)
        events = self._sample(report["events"], "event_log", owner)
        rows = self._json_lines(events, "aggregate-balance event log")
        created: list[str] = []
        folded: list[str] = []
        spends = 0
        stage = "created"
        for row_index, row in enumerate(rows, start=1):
            if not isinstance(row, Mapping):
                _fail(f"aggregate event {row_index} must be an object")
            event = row.get("event")
            if event in {"payment_created", "credit_folded"}:
                typed = _object(row, f"aggregate event {row_index}", {"event", "index", "credit_id"})
                credit_id = _digest(typed["credit_id"], "aggregate credit id")
                if event == "payment_created":
                    if stage != "created" or typed["index"] != len(created) + 1:
                        _fail("aggregate payment-created events are not contiguous")
                    created.append(credit_id)
                else:
                    stage = "folded"
                    if typed["index"] != len(folded) + 1:
                        _fail("aggregate credit-folded events are not contiguous")
                    folded.append(credit_id)
            elif event == "spend_emitted":
                typed = _object(row, f"aggregate event {row_index}", {"event", "index", "result"})
                stage = "spent"
                spends += 1
                if typed["index"] != spends or typed["result"] != "verified":
                    _fail("aggregate spend event is not verified")
            else:
                _fail("aggregate event log contains an unsupported event")
        if (
            len(created) < MIN_AGGREGATED_CREDITS
            or len(set(created)) != len(created)
            or folded != created
            or spends != 1
            or not rows
            or rows[-1].get("event") != "spend_emitted"
        ):
            _fail("aggregate event log does not prove 1000 independent credits folded and spent once")
        self._bind_report_command(
            path,
            report,
            {
                path,
                proof,
                events,
                self.artifacts["state_vk_eq"],
                self.artifacts["state_vk_ep"],
            },
        )
        return {
            "independent_payments": len(created),
            "folded_credits": len(folded),
            "spend_payments": spends,
            "report": _binding(self.files[path]),
        }

    def _verify_thermal(
        self,
        path: str,
        *,
        profile_id: str,
        protocols: Mapping[str, object],
    ) -> dict[str, object]:
        report = self._report(
            path,
            "iroha.kagemusha_v1.thermal_report",
            {
                "hardware_profile_id",
                "eq_protocol_digest",
                "ep_protocol_digest",
                "eq_verifying_key",
                "ep_verifying_key",
                "proof",
                "fold_log",
                "fold_p95_ms",
                "process_rss_bytes",
                "operation_energy_millijoules",
            },
        )
        if report["hardware_profile_id"] != profile_id:
            _fail("thermal report names a different profile")
        self._protocol_and_keys(
            report,
            eq_protocol=str(protocols["state_eq_protocol_digest"]),
            ep_protocol=str(protocols["state_ep_protocol_digest"]),
            eq_role="state_vk_eq",
            ep_role="state_vk_ep",
        )
        owner = f"thermal:{profile_id}"
        proof = self._sample(report["proof"], "proof", owner)
        log = self._sample(report["fold_log"], "event_log", owner)
        rows = self._json_lines(log, "thermal fold log")
        credit_ids: list[str] = []
        for expected_index, raw_row in enumerate(rows, start=1):
            row = _object(
                raw_row,
                f"thermal fold {expected_index}",
                {"index", "credit_id", "result"},
            )
            if row["index"] != expected_index or row["result"] != "folded":
                _fail("thermal fold log is not a contiguous folded sequence")
            credit_ids.append(_digest(row["credit_id"], "thermal credit id"))
        if len(credit_ids) < MIN_THERMAL_FOLDED_CREDITS or len(set(credit_ids)) != len(credit_ids):
            _fail("thermal report does not contain 1000 distinct folded credits")
        fold_p95 = _integer(
            report["fold_p95_ms"],
            "thermal fold p95",
            minimum=1,
            maximum=PROVE_P95_MAX_MS,
        )
        rss = _integer(
            report["process_rss_bytes"],
            "thermal RSS",
            minimum=1,
            maximum=PROCESS_RSS_MAX_BYTES,
        )
        energy = _integer(
            report["operation_energy_millijoules"],
            "thermal operation energy",
            minimum=1,
        )
        self._bind_report_command(
            path,
            report,
            {
                path,
                proof,
                log,
                self.artifacts["state_vk_eq"],
                self.artifacts["state_vk_ep"],
            },
        )
        return {
            "folded_credits": len(credit_ids),
            "fold_p95_ms": fold_p95,
            "process_rss_bytes": rss,
            "operation_energy_millijoules": energy,
            "report": _binding(self.files[path]),
        }

    def _verify_envelope(
        self, path: str, *, profile_id: str
    ) -> dict[str, object]:
        report = self._report(
            path,
            "iroha.kagemusha_v1.envelope_report",
            {"hardware_profile_id", "raw_session", "text_session", "handoff_p95_ms"},
        )
        if report["hardware_profile_id"] != profile_id:
            _fail("envelope report names a different profile")
        owner = f"envelope:{profile_id}"
        raw_session = self._sample(report["raw_session"], "raw_session", owner)
        text_session = self._sample(report["text_session"], "text_session", owner)
        handoff = _integer(
            report["handoff_p95_ms"],
            "complete handoff p95",
            minimum=1,
            maximum=HANDOFF_P95_MAX_MS,
        )
        self._bind_report_command(path, report, {path, raw_session, text_session})
        return {
            "raw_session_bytes": self.files[raw_session].size,
            "text_session_bytes": self.files[text_session].size,
            "handoff_p95_ms": handoff,
            "report": _binding(self.files[path]),
        }

    def _verify_acceptance(
        self, path: str, *, profile_id: str, case: str
    ) -> dict[str, object]:
        report = self._report(
            path,
            "iroha.kagemusha_v1.acceptance_case_report",
            {"hardware_profile_id", "case", "validators", "passed"},
        )
        self._profile_and_name(report, profile_id, "case", case)
        validators_raw = _array(report["validators"], f"acceptance case {case} validators")
        validators = [
            _digest(value, f"acceptance case {case} validator id")
            for value in validators_raw
        ]
        expected_count = 4 if case == "four_peer_activation_restart_replay" else 0
        if (
            len(validators) != expected_count
            or validators != sorted(set(validators))
        ):
            _fail(f"acceptance case {case!r} has the wrong validator set")
        _true(report["passed"], f"acceptance case {case} passed")
        self._bind_report_command(path, report, {path})
        return {
            "case": case,
            "validator_count": len(validators),
            "report": _binding(self.files[path]),
        }

    def _verify_reproducible_builds(
        self,
        raw: object,
        *,
        source: Mapping[str, object],
        artifact_set_digest: str,
    ) -> list[dict[str, object]]:
        rows = _array(raw, "reproducible builds")
        if len(rows) < 2 or len(rows) > 8:
            _fail("reproducible-build evidence must contain 2 through 8 builders")
        builder_ids: list[str] = []
        projection: list[dict[str, object]] = []
        for index, raw_row in enumerate(rows):
            row = _object(raw_row, f"reproducible build {index}", {"builder_id", "report"})
            builder_id = _digest(row["builder_id"], f"reproducible builder {index} id")
            path = self._path(row["report"], "reproducible-build report")
            report = self._report(
                path,
                "iroha.kagemusha_v1.reproducible_build_report",
                {"builder_id", "source_tree_sha256", "artifact_set_digest", "succeeded"},
            )
            if (
                report["builder_id"] != builder_id
                or report["source_tree_sha256"] != source["source_tree_digest"]
                or report["artifact_set_digest"] != artifact_set_digest
            ):
                _fail("reproducible-build report binding differs from its manifest row")
            _true(report["succeeded"], "reproducible build succeeded")
            requirements = {path, self._source_path("source_archive")}
            requirements.add(self._source_path("cargo_lock"))
            requirements.update(self.artifacts.values())
            self._bind_report_command(path, report, requirements)
            builder_ids.append(builder_id)
            projection.append(
                {
                    "builder_id": builder_id,
                    "artifact_set_digest": artifact_set_digest,
                    "report": _binding(self.files[path]),
                }
            )
        if builder_ids != sorted(set(builder_ids)):
            _fail("reproducible builds must be uniquely sorted by builder id")
        return projection

    def _source_path(self, name: str) -> str:
        source = self.manifest["source"]
        assert isinstance(source, Mapping)
        return str(source[name])

    def _report(
        self, path: str, schema: str, body_fields: set[str]
    ) -> Mapping[str, object]:
        self._expect_kind(path, "report")
        if path in self.used_reports:
            _fail(f"typed report path is reused by multiple matrix cells: {path!r}")
        self.used_reports.add(path)
        self.used_files.add(path)
        payload = self.payloads[path]
        report = load_json_object(payload, f"typed report {path!r}")
        expected_fields = body_fields | {"schema", "schema_version", "verification_id"}
        _object(report, f"typed report {path!r}", expected_fields)
        if report["schema"] != schema or report["schema_version"] != SCHEMA_VERSION:
            _fail(f"typed report {path!r} has the wrong schema or version")
        try:
            encoded = canonical_json_bytes(dict(report))
        except (TypeError, ValueError) as error:
            raise KagemushaEvidenceError(
                f"typed report {path!r} is not canonical JSON"
            ) from error
        if encoded != payload:
            _fail(f"typed report {path!r} is not canonical JSON")
        return report

    def _bind_report_command(
        self,
        path: str,
        report: Mapping[str, object],
        required_files: set[str],
    ) -> None:
        command_id = _safe_id(report["verification_id"], f"report {path!r} verification id")
        if command_id not in self.commands:
            _fail(f"report {path!r} names an unknown verifier command")
        if command_id in self.used_commands:
            _fail(f"verifier command {command_id!r} is reused by multiple reports")
        if self.commands[command_id].report_schema != report["schema"]:
            _fail(f"verifier command {command_id!r} is authorized for a different report schema")
        command_files = {
            value for kind, value in self.commands[command_id].arguments if kind == "file"
        }
        if command_files != required_files:
            _fail(
                f"verifier command {command_id!r} does not receive exactly its "
                "typed report inputs"
            )
        self.used_commands.add(command_id)
        self.command_requirements[command_id] = frozenset(required_files)
        self.used_files.update(required_files)

    def _path(self, raw: object, label: str) -> str:
        path = canonical_relative_path(_string(raw, label))
        if path not in self.files:
            _fail(f"{label} references an undeclared evidence file")
        return path

    def _expect_kind(self, path: str, expected: str) -> None:
        if self.file_kinds[path] != expected:
            _fail(f"evidence file {path!r} must have kind {expected!r}")

    def _sample(self, raw: object, kind: str, owner: str) -> str:
        path = self._path(raw, f"{owner} {kind}")
        self._expect_kind(path, kind)
        previous = self.used_samples.get(path)
        if previous is not None:
            _fail(
                f"measurement sample {path!r} is aliased by {previous!r} and {owner!r}"
            )
        self.used_samples[path] = owner
        self.used_files.add(path)
        return path

    def _json_lines(self, path: str, label: str) -> list[Mapping[str, object]]:
        payload = self.payloads[path]
        if not payload.endswith(b"\n"):
            _fail(f"{label} must end with one newline")
        raw_lines = payload[:-1].split(b"\n")
        if not raw_lines or any(not line for line in raw_lines):
            _fail(f"{label} must contain only non-empty canonical rows")
        self.total_jsonl_rows += len(raw_lines)
        if self.total_jsonl_rows > MAX_JSONL_ROWS:
            _fail("typed event logs exceed the 5,000,000-row aggregate cap")
        rows: list[Mapping[str, object]] = []
        for index, line in enumerate(raw_lines, start=1):
            row = load_json_object(line, f"{label} row {index}")
            try:
                canonical = json.dumps(
                    row,
                    sort_keys=True,
                    separators=(",", ":"),
                    ensure_ascii=True,
                    allow_nan=False,
                ).encode("utf-8")
            except (TypeError, ValueError) as error:
                raise KagemushaEvidenceError(
                    f"{label} row {index} is not canonical JSON"
                ) from error
            if canonical != line:
                _fail(f"{label} row {index} is not canonical JSON")
            rows.append(row)
        return rows

    def _profile_and_name(
        self, report: Mapping[str, object], profile_id: str, field: str, expected: str
    ) -> None:
        if report["hardware_profile_id"] != profile_id or report[field] != expected:
            _fail(f"typed report {field} binding differs from its matrix row")

    def _protocol_and_keys(
        self,
        report: Mapping[str, object],
        *,
        eq_protocol: str,
        ep_protocol: str,
        eq_role: str,
        ep_role: str,
    ) -> None:
        if (
            report["eq_protocol_digest"] != eq_protocol
            or report["ep_protocol_digest"] != ep_protocol
            or report["eq_verifying_key"] != self.artifacts[eq_role]
            or report["ep_verifying_key"] != self.artifacts[ep_role]
        ):
            _fail("typed report protocol or verifying-key binding is substituted")

    def _measurement_rows(
        self, report: Mapping[str, object], name: str, group: str
    ) -> tuple[int, int]:
        eq = _integer(
            report["eq_circuit_rows"],
            f"{name} Eq rows",
            minimum=1,
            maximum=MAX_CIRCUIT_ROWS,
        )
        ep = _integer(
            report["ep_circuit_rows"],
            f"{name} Ep rows",
            minimum=1,
            maximum=MAX_CIRCUIT_ROWS,
        )
        if (eq, ep) != self.circuit_shapes[group][name]:
            _fail(f"{name} row measurements differ from the circuit-shape report")
        return eq, ep

    def _performance_metrics(
        self, report: Mapping[str, object], label: str
    ) -> dict[str, int]:
        return {
            "prove_p95_ms": _integer(
                report["prove_p95_ms"],
                f"{label} prove p95",
                minimum=1,
                maximum=PROVE_P95_MAX_MS,
            ),
            "verify_p95_ms": _integer(
                report["verify_p95_ms"],
                f"{label} verify p95",
                minimum=1,
                maximum=VERIFY_P95_MAX_MS,
            ),
            "process_rss_bytes": _integer(
                report["process_rss_bytes"],
                f"{label} RSS",
                minimum=1,
                maximum=PROCESS_RSS_MAX_BYTES,
            ),
            "operation_energy_millijoules": _integer(
                report["operation_energy_millijoules"],
                f"{label} operation energy",
                minimum=1,
            ),
        }

    def _artifact_binding(self, role: str) -> dict[str, object]:
        return {"role": role, **_binding(self.files[self.artifacts[role]])}

    def _revalidate_closure(self) -> None:
        if _scan_evidence_tree(self.root) != sorted(self.files):
            _fail("evidence root changed while it was verified")
        for path in sorted(self.files):
            current = stable_hash_relative(
                self.root, path, max_size=_max_for_kind(self.file_kinds[path])
            )
            if current != self.files[path]:
                _fail(f"evidence file {path!r} changed while commands ran")
        manifest_info, manifest_bytes = stable_read_path(
            self.manifest_path, max_size=MAX_MANIFEST_BYTES
        )
        if manifest_info != self.manifest_info or manifest_bytes != self.manifest_bytes:
            _fail("evidence manifest changed while it was verified")
        policy_info, policy_bytes = stable_read_path(
            self.observer_policy.path, max_size=MAX_OBSERVER_POLICY_BYTES
        )
        if policy_info != self.observer_policy.info or policy_bytes != self.observer_policy.payload:
            _fail("trusted observer policy changed while evidence was verified")

def verify_evidence(
    *,
    manifest_path: Path,
    expected_manifest_sha256: str,
    evidence_root: Path,
    observer_policy_path: Path,
    expected_observer_policy_sha256: str,
) -> dict[str, object]:
    """Verify one explicit immutable manifest/root pair and return its projection."""

    expected = _digest(expected_manifest_sha256, "expected manifest SHA-256")
    if (
        not manifest_path.is_absolute()
        or Path(os.path.abspath(manifest_path)) != manifest_path
        or not evidence_root.is_absolute()
        or Path(os.path.abspath(evidence_root)) != evidence_root
        or not observer_policy_path.is_absolute()
        or Path(os.path.abspath(observer_policy_path)) != observer_policy_path
    ):
        _fail("manifest, evidence-root, and observer-policy paths must be absolute and normalized")
    for outside, label in (
        (manifest_path, "evidence manifest"),
        (observer_policy_path, "trusted observer policy"),
    ):
        try:
            outside.relative_to(evidence_root)
        except ValueError:
            pass
        else:
            _fail(f"{label} must be outside the closed evidence root")
    if manifest_path == observer_policy_path:
        _fail("evidence manifest and trusted observer policy must be distinct files")
    observer_policy = _load_observer_policy(
        observer_policy_path, expected_observer_policy_sha256
    )
    manifest_info, payload = stable_read_path(
        manifest_path, max_size=MAX_MANIFEST_BYTES
    )
    if manifest_info.sha256 != expected:
        _fail("evidence manifest SHA-256 differs from the explicit immutable identity")
    manifest = load_json_object(payload, "kagemusha evidence manifest")
    try:
        canonical = canonical_json_bytes(dict(manifest))
    except (TypeError, ValueError) as error:
        raise KagemushaEvidenceError("evidence manifest is not canonical JSON") from error
    if canonical != payload:
        _fail("evidence manifest is not canonical JSON")
    verifier = EvidenceVerifier(
        root=evidence_root,
        manifest_path=manifest_path,
        manifest_info=manifest_info,
        manifest_bytes=payload,
        manifest_sha256=expected,
        manifest=manifest,
        observer_policy=observer_policy,
    )
    return verifier.verify()


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Verify an immutable Kagemusha V1 release-evidence closure."
    )
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--manifest-sha256", required=True)
    parser.add_argument("--evidence-root", required=True, type=Path)
    parser.add_argument("--observer-policy", required=True, type=Path)
    parser.add_argument("--observer-policy-sha256", required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        projection = verify_evidence(
            manifest_path=args.manifest,
            expected_manifest_sha256=args.manifest_sha256,
            evidence_root=args.evidence_root,
            observer_policy_path=args.observer_policy,
            expected_observer_policy_sha256=args.observer_policy_sha256,
        )
    except (KagemushaEvidenceError, ReleaseArtifactError, OSError, ValueError) as error:
        print(f"Kagemusha release evidence rejected: {error}", file=sys.stderr)
        return 1
    sys.stdout.buffer.write(canonical_json_bytes(projection))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
