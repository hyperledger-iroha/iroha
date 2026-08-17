#!/usr/bin/env python3
"""Fail-closed authority contract for public-Taira 24-hour soak evidence.

The public soak has a dedicated authority and replay namespace. Fresh
admission is a short-lived, consume-once operation. Historical verification
uses a separate durable broker receipt that binds the consumed replay identity
to the exact evidence subject. This module defines both closed wire layouts,
but deliberately provisions neither trust root, signature verifier, nor replay
broker. Public entry points therefore refuse before inspecting caller data.
"""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
import hashlib
import json
import re
from typing import NoReturn

try:
    from . import taira_authority_client
except ImportError:
    import taira_authority_client


AUTHORITY_SCHEMA = "iroha.taira.public-v2-24h-soak-authority-envelope.v1"
CLAIMS_SCHEMA = "iroha.taira.public-v2-24h-soak-authority-claims.v1"
ADMISSION_RECEIPT_SCHEMA = (
    "iroha.taira.public-v2-24h-soak-durable-admission-receipt.v1"
)
ADMISSION_CLAIMS_SCHEMA = (
    "iroha.taira.public-v2-24h-soak-durable-admission-claims.v1"
)
SUBJECT_SCHEMA = "iroha.taira.public-v2-24h-soak-authority-subject.v1"
REPLAY_NAMESPACE = "iroha.taira.public-v2-24h-soak-authority-replay.v1"
SUBJECT_DOMAIN = b"iroha.taira.public-v2-24h-soak.authority-subject.v1\0"
AUTHORITY_SIGNATURE_DOMAIN = (
    b"iroha.taira.public-v2-24h-soak.authority-envelope-signature.v1\0"
)
BROKER_SIGNATURE_DOMAIN = (
    b"iroha.taira.public-v2-24h-soak.durable-admission-signature.v1\0"
)
SIGNATURE_ALGORITHM = "ed25519"
MAX_AUTHORITY_LIFETIME_MS = 15 * 60 * 1_000
MAX_AUTHORITY_ISSUANCE_DELAY_MS = 15 * 60 * 1_000

SHA256_RE = re.compile(r"[0-9a-f]{64}")
SIGNATURE_RE = re.compile(r"[0-9a-f]{128}")
ENVELOPE_FIELDS = {
    "schema", "schema_version", "authority_key_id", "signature_algorithm",
    "claims", "signature",
}
CLAIMS_FIELDS = {
    "schema", "subject_digest", "replay_namespace", "replay_id",
    "issued_at_unix_ms", "expires_at_unix_ms",
}
ADMISSION_RECEIPT_FIELDS = {
    "schema", "schema_version", "broker_key_id", "signature_algorithm",
    "claims", "signature",
}
ADMISSION_CLAIMS_FIELDS = {
    "schema", "decision", "receipt_id", "subject_digest",
    "authority_envelope_sha256", "authority_key_id", "replay_namespace",
    "replay_id", "admitted_at_unix_ms",
}
SUBJECT_FIELDS = {
    "schema", "receipt", "source", "prerequisites", "anchor", "samples",
    "workload", "submission_receipts", "applied_statuses", "blocks",
    "lifecycle", "native_verifier",
}
RECEIPT_IDENTITY_FIELDS = {"sha256", "size_bytes"}
SOURCE_IDENTITY_FIELDS = {"tuple_sha256"}
PREREQUISITE_IDENTITY_FIELDS = {
    "candidate_handoff_sha256", "publication_handoff_sha256",
    "deploy_handoff_sha256",
}
ANCHOR_IDENTITY_FIELDS = {"sha256", "validator_count"}
SET_IDENTITY_FIELDS = {"sha256", "count"}
INVENTORY_IDENTITY_FIELDS = {
    "artifact_sha256", "records_sha256", "record_count",
}
LIFECYCLE_IDENTITY_FIELDS = {
    "artifact_sha256", "identity_sha256", "journal_artifact_sha256",
    "journal_records_sha256", "journal_record_count",
    "native_verifier_receipt_sha256", "window_sha256",
}
NATIVE_VERIFIER_IDENTITY_FIELDS = {"binary_sha256", "source_sha256"}

PROVISIONING_ERROR = (
    f"{AUTHORITY_SCHEMA} is not provisioned: public-soak admission requires "
    "an independent Ed25519 authority trust root, a pinned native evidence "
    "verifier, and an atomic replay broker outside the soak runner, workload "
    "signer, deploy controller, release signer, and publication host; fresh "
    f"admission must consume one replay identity in {REPLAY_NAMESPACE}, then "
    f"emit a separately broker-signed {ADMISSION_RECEIPT_SCHEMA} that remains "
    "verifiable after the short-lived envelope expires. Environment switches, "
    "marker files, workflow IDs, caller keys, self-signatures, self-hashes, and "
    "reuse of privacy-rollout authority are not authority"
)


class PublicSoakAuthorityError(RuntimeError):
    """The public-soak authority envelope or admission receipt is invalid."""


@dataclass(frozen=True)
class AuthorityClaims:
    """Structurally checked fresh claims; this is not authentication."""

    authority_key_id: str
    signature: str
    subject_digest: str
    replay_id: str
    issued_at_unix_ms: int
    expires_at_unix_ms: int


@dataclass(frozen=True)
class DurableAdmissionClaims:
    """Structurally checked durable claims; this is not authentication."""

    broker_key_id: str
    signature: str
    receipt_id: str
    subject_digest: str
    authority_envelope_sha256: str
    authority_key_id: str
    replay_id: str
    admitted_at_unix_ms: int


def _fail(message: str) -> NoReturn:
    raise PublicSoakAuthorityError(message)


def _canonical_json(value: object) -> bytes:
    try:
        return (
            json.dumps(value, ensure_ascii=True, allow_nan=False, sort_keys=True,
                       separators=(",", ":")) + "\n"
        ).encode("ascii")
    except (TypeError, ValueError, UnicodeEncodeError) as error:
        raise PublicSoakAuthorityError(
            f"authority value is not canonically encodable: {error}"
        ) from error


def _reject_constant(value: str) -> NoReturn:
    _fail(f"non-finite authority JSON number is forbidden: {value}")


def _pairs(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            _fail(f"duplicate authority JSON field is forbidden: {key}")
        value[key] = item
    return value


def _decode_canonical(payload: bytes,
                      label: str = "authority envelope") -> dict[str, object]:
    try:
        value = json.loads(payload, object_pairs_hook=_pairs,
                           parse_constant=_reject_constant)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise PublicSoakAuthorityError(f"{label} is not strict JSON") from error
    if not isinstance(value, dict) or _canonical_json(value) != payload:
        _fail(f"{label} is not one canonical closed JSON object")
    return value


def _exact(value: object, fields: set[str], label: str) -> Mapping[str, object]:
    if not isinstance(value, dict) or set(value) != fields:
        _fail(f"{label} fields are not exact")
    return value


def _integer(value: object, label: str, *, minimum: int = 0) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
        _fail(f"{label} must be an integer >= {minimum}")
    return value


def _sha256(value: object, label: str) -> str:
    if (not isinstance(value, str) or SHA256_RE.fullmatch(value) is None
            or value == "0" * 64):
        _fail(f"{label} must be one nonzero lowercase SHA-256 digest")
    return value


def _signature(value: object, label: str) -> str:
    if not isinstance(value, str) or SIGNATURE_RE.fullmatch(value) is None:
        _fail(f"{label} must be one lowercase Ed25519 signature")
    return value


def _inventory_identity(value: object, label: str) -> None:
    identity = _exact(value, INVENTORY_IDENTITY_FIELDS, label)
    _sha256(identity["artifact_sha256"], f"{label} artifact digest")
    _sha256(identity["records_sha256"], f"{label} record-set digest")
    _integer(identity["record_count"], f"{label} record count", minimum=1)


def _domain_separated_signing_bytes(
    payload: bytes, *, fields: set[str], domain: bytes, label: str,
) -> bytes:
    document = _exact(_decode_canonical(payload, label), fields, label)
    signed = {field: document[field] for field in fields - {"signature"}}
    return domain + _canonical_json(signed)


def authority_envelope_signing_bytes(payload: bytes) -> bytes:
    """Return the only bytes an authority signature may authenticate."""

    return _domain_separated_signing_bytes(
        payload, fields=ENVELOPE_FIELDS, domain=AUTHORITY_SIGNATURE_DOMAIN,
        label="authority envelope",
    )


def durable_admission_receipt_signing_bytes(payload: bytes) -> bytes:
    """Return the separately domain-bound broker-signature bytes."""

    return _domain_separated_signing_bytes(
        payload, fields=ADMISSION_RECEIPT_FIELDS, domain=BROKER_SIGNATURE_DOMAIN,
        label="durable admission receipt",
    )


def subject_digest(subject: Mapping[str, object]) -> str:
    """Hash one exact, durable evidence subject under the public-soak domain."""

    document = _exact(subject, SUBJECT_FIELDS, "authority subject")
    if document["schema"] != SUBJECT_SCHEMA:
        _fail("authority subject schema is wrong")
    receipt = _exact(document["receipt"], RECEIPT_IDENTITY_FIELDS,
                     "receipt identity")
    _sha256(receipt["sha256"], "receipt identity digest")
    _integer(receipt["size_bytes"], "receipt identity size", minimum=1)
    source = _exact(document["source"], SOURCE_IDENTITY_FIELDS,
                    "source identity")
    _sha256(source["tuple_sha256"], "source tuple digest")
    prerequisites = _exact(document["prerequisites"],
                           PREREQUISITE_IDENTITY_FIELDS,
                           "prerequisite identity")
    for field in sorted(PREREQUISITE_IDENTITY_FIELDS):
        _sha256(prerequisites[field], f"prerequisite {field}")
    if len(set(prerequisites.values())) != len(PREREQUISITE_IDENTITY_FIELDS):
        _fail("prerequisite handoff digests are aliased")
    anchor = _exact(document["anchor"], ANCHOR_IDENTITY_FIELDS,
                    "anchor identity")
    _sha256(anchor["sha256"], "anchor identity digest")
    _integer(anchor["validator_count"], "anchor validator count", minimum=1)
    samples = _exact(document["samples"], SET_IDENTITY_FIELDS,
                     "sample identity")
    _sha256(samples["sha256"], "sample identity digest")
    _integer(samples["count"], "sample identity count", minimum=1)
    for field, label in (
        ("workload", "workload identity"),
        ("submission_receipts", "submission-receipt identity"),
        ("applied_statuses", "Applied-status identity"),
        ("blocks", "block-evidence identity"),
    ):
        _inventory_identity(document[field], label)
    lifecycle = _exact(document["lifecycle"], LIFECYCLE_IDENTITY_FIELDS,
                       "lifecycle identity")
    _sha256(lifecycle["artifact_sha256"], "lifecycle artifact digest")
    _sha256(lifecycle["identity_sha256"], "lifecycle identity digest")
    _sha256(lifecycle["journal_artifact_sha256"],
            "lifecycle journal artifact digest")
    _sha256(lifecycle["journal_records_sha256"],
            "lifecycle journal record-set digest")
    _integer(lifecycle["journal_record_count"],
             "lifecycle journal record count", minimum=1)
    _sha256(lifecycle["native_verifier_receipt_sha256"],
            "lifecycle native-verifier receipt digest")
    _sha256(lifecycle["window_sha256"], "lifecycle window digest")
    native_verifier = _exact(document["native_verifier"],
                             NATIVE_VERIFIER_IDENTITY_FIELDS,
                             "native verifier identity")
    _sha256(native_verifier["binary_sha256"],
            "native verifier binary digest")
    _sha256(native_verifier["source_sha256"],
            "native verifier source digest")
    return hashlib.sha256(SUBJECT_DOMAIN + _canonical_json(document)).hexdigest()


def validate_envelope_claims(
    payload: bytes,
    *,
    subject_core: Mapping[str, object],
    completed_at_unix_ms: int,
    admission_time_unix_ms: int,
) -> AuthorityClaims:
    """Check fresh envelope shape and subject binding without authentication.

    ``admission_time_unix_ms`` is the replay broker's recorded time, not the
    verifier's current time. This permits honest historical verification.
    """

    authority_envelope_signing_bytes(payload)
    envelope = _exact(_decode_canonical(payload), ENVELOPE_FIELDS,
                      "authority envelope")
    if (envelope["schema"] != AUTHORITY_SCHEMA
            or type(envelope["schema_version"]) is not int
            or envelope["schema_version"] != 1):
        _fail("authority envelope schema is wrong")
    key_id = _sha256(envelope["authority_key_id"], "authority key ID")
    if envelope["signature_algorithm"] != SIGNATURE_ALGORITHM:
        _fail("authority signature algorithm is wrong")
    signature = _signature(envelope["signature"], "authority signature")
    claims = _exact(envelope["claims"], CLAIMS_FIELDS, "authority claims")
    if claims["schema"] != CLAIMS_SCHEMA:
        _fail("authority claims schema is wrong")
    if claims["replay_namespace"] != REPLAY_NAMESPACE:
        _fail("authority replay namespace is wrong")
    replay_id = _sha256(claims["replay_id"], "authority replay ID")
    issued = _integer(claims["issued_at_unix_ms"],
                      "authority issued time", minimum=1)
    expires = _integer(claims["expires_at_unix_ms"],
                       "authority expiry", minimum=1)
    admitted = _integer(admission_time_unix_ms,
                        "broker admission time", minimum=1)
    completed = _integer(completed_at_unix_ms,
                         "soak completion time", minimum=1)
    if not completed <= issued <= completed + MAX_AUTHORITY_ISSUANCE_DELAY_MS:
        _fail("authority envelope was not issued promptly after soak completion")
    if not issued < expires <= issued + MAX_AUTHORITY_LIFETIME_MS:
        _fail("authority envelope validity interval is invalid")
    if not issued <= admitted <= expires:
        _fail("authority envelope was not fresh when the broker admitted it")
    expected_digest = subject_digest(subject_core)
    claimed_digest = _sha256(claims["subject_digest"],
                             "authority subject digest")
    if claimed_digest != expected_digest:
        _fail("authority subject digest does not bind the exact evidence set")
    return AuthorityClaims(key_id, signature, claimed_digest, replay_id,
                           issued, expires)


def validate_durable_admission_receipt_claims(
    payload: bytes,
    *,
    authority_envelope: bytes,
    subject_core: Mapping[str, object],
    completed_at_unix_ms: int,
) -> DurableAdmissionClaims:
    """Check a durable broker receipt and fresh envelope structurally only."""

    durable_admission_receipt_signing_bytes(payload)
    receipt = _exact(_decode_canonical(payload, "durable admission receipt"),
                     ADMISSION_RECEIPT_FIELDS, "durable admission receipt")
    if (receipt["schema"] != ADMISSION_RECEIPT_SCHEMA
            or type(receipt["schema_version"]) is not int
            or receipt["schema_version"] != 1):
        _fail("durable admission receipt schema is wrong")
    broker_key_id = _sha256(receipt["broker_key_id"],
                            "replay broker key ID")
    if receipt["signature_algorithm"] != SIGNATURE_ALGORITHM:
        _fail("replay broker signature algorithm is wrong")
    signature = _signature(receipt["signature"], "replay broker signature")
    claims = _exact(receipt["claims"], ADMISSION_CLAIMS_FIELDS,
                    "durable admission claims")
    if claims["schema"] != ADMISSION_CLAIMS_SCHEMA:
        _fail("durable admission claims schema is wrong")
    if claims["decision"] != "admitted":
        _fail("durable admission decision is not admitted")
    receipt_id = _sha256(claims["receipt_id"],
                         "durable admission receipt ID")
    if claims["replay_namespace"] != REPLAY_NAMESPACE:
        _fail("durable admission replay namespace is wrong")
    admitted = _integer(claims["admitted_at_unix_ms"],
                        "durable admission time", minimum=1)
    fresh = validate_envelope_claims(
        authority_envelope,
        subject_core=subject_core,
        completed_at_unix_ms=completed_at_unix_ms,
        admission_time_unix_ms=admitted,
    )
    if broker_key_id == fresh.authority_key_id:
        _fail("replay broker and observation authority key IDs must be distinct")
    envelope_sha256 = hashlib.sha256(authority_envelope).hexdigest()
    if _sha256(claims["authority_envelope_sha256"],
               "authority envelope artifact digest") != envelope_sha256:
        _fail("durable admission receipt does not bind the authority envelope bytes")
    if _sha256(claims["subject_digest"],
               "durable subject digest") != fresh.subject_digest:
        _fail("durable admission receipt does not bind the evidence subject")
    authority_key_id = _sha256(claims["authority_key_id"],
                               "durable authority key ID")
    if authority_key_id != fresh.authority_key_id:
        _fail("durable admission receipt authority key differs")
    replay_id = _sha256(claims["replay_id"], "durable replay ID")
    if replay_id != fresh.replay_id:
        _fail("durable admission receipt replay identity differs")
    return DurableAdmissionClaims(
        broker_key_id, signature, receipt_id, fresh.subject_digest,
        envelope_sha256, authority_key_id, replay_id, admitted,
    )


def _require_public_soak_authority(
    *,
    observation_require_signing: bool = True,
    replay_require_signing: bool = True,
) -> None:
    """Authenticate the distinct observation and replay-admission services."""

    observation = taira_authority_client.ROLE_REGISTRY[
        "public-soak-observation"
    ]
    replay = taira_authority_client.ROLE_REGISTRY[
        "public-soak-replay-admission"
    ]
    if (
        observation.service_id == replay.service_id
        or observation.administrator_id == replay.administrator_id
        or observation.binding_path == replay.binding_path
        or observation.request_socket == replay.request_socket
        or observation.state_directory == replay.state_directory
    ):
        _fail("public-soak observation signer and replay broker are not distinct")
    try:
        if observation_require_signing:
            taira_authority_client.preflight("public-soak-observation")
        else:
            taira_authority_client.preflight(
                "public-soak-observation", require_signing=False
            )
        if replay_require_signing:
            taira_authority_client.preflight("public-soak-replay-admission")
        else:
            taira_authority_client.preflight(
                "public-soak-replay-admission", require_signing=False
            )
    except taira_authority_client.TairaAuthorityClientError as error:
        raise PublicSoakAuthorityError(f"{PROVISIONING_ERROR}: {error}") from error


def require_public_soak_authority_provisioned() -> None:
    """Authenticate the distinct observation and replay-admission services."""

    _require_public_soak_authority()


def _observation_subject(
    subject_core: Mapping[str, object], completed_at_unix_ms: int
) -> dict[str, object]:
    return {
        "completed_at_unix_ms": completed_at_unix_ms,
        "subject": dict(subject_core),
        "subject_digest": subject_digest(subject_core),
    }


def _replay_subject(
    payload: bytes,
    subject_core: Mapping[str, object],
    completed_at_unix_ms: int,
) -> dict[str, object]:
    envelope = _decode_canonical(payload)
    return {
        "authority_envelope": envelope,
        "authority_envelope_sha256": hashlib.sha256(payload).hexdigest(),
        "completed_at_unix_ms": completed_at_unix_ms,
        "replay_namespace": REPLAY_NAMESPACE,
        "subject": dict(subject_core),
        "subject_digest": subject_digest(subject_core),
    }


def _verify_observation_signature(
    payload: bytes,
    subject_core: Mapping[str, object],
    completed_at_unix_ms: int,
) -> None:
    envelope = _decode_canonical(payload)
    try:
        taira_authority_client.verify_receipt(
            "public-soak-observation",
            _observation_subject(subject_core, completed_at_unix_ms),
            authority_envelope=envelope,
            durable_receipt={},
        )
    except taira_authority_client.TairaAuthorityClientError as error:
        raise PublicSoakAuthorityError(
            f"public-soak observation signature verification failed: {error}"
        ) from error


def consume_fresh_public_soak_admission(
    payload: bytes,
    *,
    subject_core: Mapping[str, object],
    completed_at_unix_ms: int,
) -> bytes:
    """Verify the observation and atomically consume its replay identity once."""

    _require_public_soak_authority(
        observation_require_signing=False
    )
    _verify_observation_signature(payload, subject_core, completed_at_unix_ms)
    subject = _replay_subject(payload, subject_core, completed_at_unix_ms)
    try:
        result = taira_authority_client.authorize(
            "public-soak-replay-admission", subject
        )
    except taira_authority_client.TairaAuthorityClientError as error:
        raise PublicSoakAuthorityError(
            f"public-soak replay admission failed: {error}"
        ) from error
    receipt_payload = _canonical_json(result.durable_receipt)
    validate_durable_admission_receipt_claims(
        receipt_payload,
        authority_envelope=payload,
        subject_core=subject_core,
        completed_at_unix_ms=completed_at_unix_ms,
    )
    return receipt_payload


def verify_authenticated_public_soak_authority_envelope(
    payload: bytes,
    *,
    durable_admission_receipt: bytes,
    subject_core: Mapping[str, object],
    completed_at_unix_ms: int,
) -> DurableAdmissionClaims:
    """Historically verify both signatures without consuming replay state.

    Provisioning must authenticate the authority signature over
    :func:`authority_envelope_signing_bytes` and the distinct broker signature
    over :func:`durable_admission_receipt_signing_bytes`, pin both distinct keys
    and the native verifier, and prove that the broker consumed the replay ID.
    It must not consume the replay ID again during historical re-verification.
    """

    _require_public_soak_authority(
        observation_require_signing=False,
        replay_require_signing=False,
    )
    structural = validate_durable_admission_receipt_claims(
        durable_admission_receipt,
        authority_envelope=payload,
        subject_core=subject_core,
        completed_at_unix_ms=completed_at_unix_ms,
    )
    _verify_observation_signature(payload, subject_core, completed_at_unix_ms)
    envelope = _decode_canonical(payload)
    receipt = _decode_canonical(
        durable_admission_receipt, "durable admission receipt"
    )
    try:
        taira_authority_client.verify_receipt(
            "public-soak-replay-admission",
            _replay_subject(payload, subject_core, completed_at_unix_ms),
            authority_envelope=envelope,
            durable_receipt=receipt,
        )
    except taira_authority_client.TairaAuthorityClientError as error:
        raise PublicSoakAuthorityError(
            f"public-soak historical replay receipt verification failed: {error}"
        ) from error
    return structural
