"""Tests for the fail-closed public-soak authority and replay contract."""

from __future__ import annotations

import copy
import hashlib

import pytest

from scripts import taira_privacy_rollout_contract as privacy_authority
from scripts import taira_public_soak_authority_contract as authority


COMPLETED_MS = 1_800_000_000_000
ISSUED_MS = COMPLETED_MS + 1_000
ADMITTED_MS = ISSUED_MS + 2_000
EXPIRES_MS = ISSUED_MS + 60_000


def digest(label: str) -> str:
    """Return one deterministic fixture digest."""

    return hashlib.sha256(label.encode("ascii")).hexdigest()


def inventory(label: str, count: int = 10) -> dict[str, object]:
    """Return one exact inventory identity."""

    return {
        "artifact_sha256": digest(f"{label}-artifact"),
        "records_sha256": digest(f"{label}-records"),
        "record_count": count,
    }


def subject_core() -> dict[str, object]:
    """Return one exact durable evidence subject."""

    return {
        "schema": authority.SUBJECT_SCHEMA,
        "receipt": {"sha256": digest("receipt"), "size_bytes": 100},
        "source": {"tuple_sha256": digest("source")},
        "prerequisites": {
            "candidate_handoff_sha256": digest("candidate"),
            "publication_handoff_sha256": digest("publication"),
            "deploy_handoff_sha256": digest("deploy"),
        },
        "anchor": {"sha256": digest("anchor"), "validator_count": 4},
        "samples": {"sha256": digest("samples"), "count": 2},
        "workload": inventory("workload"),
        "submission_receipts": inventory("submissions"),
        "applied_statuses": inventory("statuses"),
        "blocks": inventory("blocks", 3),
        "lifecycle": {
            "artifact_sha256": digest("lifecycle-artifact"),
            "identity_sha256": digest("lifecycle-identity"),
            "journal_artifact_sha256": digest("lifecycle-journal-artifact"),
            "journal_records_sha256": digest("lifecycle-journal-records"),
            "journal_record_count": 8,
            "native_verifier_receipt_sha256": digest(
                "lifecycle-native-receipt"
            ),
            "window_sha256": digest("lifecycle-window"),
        },
        "native_verifier": {
            "binary_sha256": digest("native-binary"),
            "source_sha256": digest("native-source"),
        },
    }


def envelope(core: dict[str, object]) -> bytes:
    """Build a structurally bound, deliberately unauthenticated envelope."""

    return authority._canonical_json(
        {
            "schema": authority.AUTHORITY_SCHEMA,
            "schema_version": 1,
            "authority_key_id": digest("independent-key"),
            "signature_algorithm": authority.SIGNATURE_ALGORITHM,
            "claims": {
                "schema": authority.CLAIMS_SCHEMA,
                "subject_digest": authority.subject_digest(core),
                "replay_namespace": authority.REPLAY_NAMESPACE,
                "replay_id": digest("replay"),
                "issued_at_unix_ms": ISSUED_MS,
                "expires_at_unix_ms": EXPIRES_MS,
            },
            "signature": "ab" * 64,
        }
    )


def durable_receipt(core: dict[str, object], fresh: bytes) -> bytes:
    """Build a structurally bound, deliberately unauthenticated broker receipt."""

    return authority._canonical_json(
        {
            "schema": authority.ADMISSION_RECEIPT_SCHEMA,
            "schema_version": 1,
            "broker_key_id": digest("independent-broker-key"),
            "signature_algorithm": authority.SIGNATURE_ALGORITHM,
            "claims": {
                "schema": authority.ADMISSION_CLAIMS_SCHEMA,
                "decision": "admitted",
                "receipt_id": digest("durable-receipt"),
                "subject_digest": authority.subject_digest(core),
                "authority_envelope_sha256": hashlib.sha256(fresh).hexdigest(),
                "authority_key_id": digest("independent-key"),
                "replay_namespace": authority.REPLAY_NAMESPACE,
                "replay_id": digest("replay"),
                "admitted_at_unix_ms": ADMITTED_MS,
            },
            "signature": "cd" * 64,
        }
    )


def validate_durable(core: dict[str, object], fresh: bytes, receipt: bytes):
    """Run the private structural durable-receipt validator."""

    return authority.validate_durable_admission_receipt_claims(
        receipt,
        authority_envelope=fresh,
        subject_core=core,
        completed_at_unix_ms=COMPLETED_MS,
    )


def test_public_soak_authority_and_replay_are_distinct_from_privacy() -> None:
    assert authority.AUTHORITY_SCHEMA != (
        privacy_authority.AUTHENTICATED_ROLLOUT_OBSERVATION_AUTHORITY_SCHEMA
    )
    assert authority.REPLAY_NAMESPACE != (
        privacy_authority.AUTHENTICATED_ROLLOUT_OBSERVATION_REPLAY_NAMESPACE
    )
    assert authority.ADMISSION_RECEIPT_SCHEMA != authority.AUTHORITY_SCHEMA


def test_fresh_and_durable_claims_bind_one_subject_and_replay() -> None:
    core = subject_core()
    fresh = envelope(core)
    claims = validate_durable(core, fresh, durable_receipt(core, fresh))
    assert claims.replay_id == digest("replay")
    assert claims.authority_envelope_sha256 == hashlib.sha256(fresh).hexdigest()
    assert claims.subject_digest == authority.subject_digest(core)


def test_expired_envelope_remains_historically_valid_at_recorded_admission() -> None:
    core = subject_core()
    fresh = envelope(core)
    # No current-time argument exists: only the durable broker admission time
    # is checked against the short-lived envelope.
    claims = validate_durable(core, fresh, durable_receipt(core, fresh))
    assert claims.admitted_at_unix_ms == ADMITTED_MS


@pytest.mark.parametrize(
    "component",
    (
        "receipt", "source", "prerequisites", "anchor", "samples", "workload",
        "submission_receipts", "applied_statuses", "blocks", "lifecycle",
        "native_verifier",
    ),
)
def test_every_evidence_identity_is_covered_by_subject_digest(component: str) -> None:
    original = subject_core()
    fresh = envelope(original)
    receipt = durable_receipt(original, fresh)
    hostile = copy.deepcopy(original)
    record = hostile[component]
    assert isinstance(record, dict)
    digest_field = next(key for key in record if "sha256" in key)
    record[digest_field] = digest(f"hostile-{component}")
    with pytest.raises(authority.PublicSoakAuthorityError, match="exact evidence set"):
        validate_durable(hostile, fresh, receipt)


def test_prerequisite_handoff_digests_must_be_distinct() -> None:
    core = subject_core()
    prerequisites = core["prerequisites"]
    assert isinstance(prerequisites, dict)
    prerequisites["publication_handoff_sha256"] = prerequisites[
        "candidate_handoff_sha256"
    ]
    with pytest.raises(authority.PublicSoakAuthorityError, match="aliased"):
        authority.subject_digest(core)


def test_fresh_envelope_must_have_bounded_lifetime_and_prompt_issuance() -> None:
    core = subject_core()
    value = authority._decode_canonical(envelope(core))
    claims = value["claims"]
    assert isinstance(claims, dict)
    claims["expires_at_unix_ms"] = (
        int(claims["issued_at_unix_ms"]) + authority.MAX_AUTHORITY_LIFETIME_MS + 1
    )
    with pytest.raises(authority.PublicSoakAuthorityError, match="validity interval"):
        authority.validate_envelope_claims(
            authority._canonical_json(value),
            subject_core=core,
            completed_at_unix_ms=COMPLETED_MS,
            admission_time_unix_ms=ADMITTED_MS,
        )


def test_broker_admission_must_occur_while_envelope_is_fresh() -> None:
    core = subject_core()
    fresh = envelope(core)
    receipt = authority._decode_canonical(
        durable_receipt(core, fresh), "durable admission receipt"
    )
    claims = receipt["claims"]
    assert isinstance(claims, dict)
    claims["admitted_at_unix_ms"] = EXPIRES_MS + 1
    with pytest.raises(authority.PublicSoakAuthorityError, match="not fresh"):
        validate_durable(core, fresh, authority._canonical_json(receipt))


@pytest.mark.parametrize(
    ("field", "hostile", "message"),
    (
        ("authority_envelope_sha256", digest("other-envelope"), "envelope bytes"),
        ("authority_key_id", digest("other-authority"), "authority key differs"),
        ("replay_id", digest("other-replay"), "replay identity differs"),
        ("subject_digest", digest("other-subject"), "evidence subject"),
        ("decision", "rejected", "decision is not admitted"),
    ),
)
def test_durable_receipt_rejects_each_cross_binding_tamper(
    field: str, hostile: object, message: str,
) -> None:
    core = subject_core()
    fresh = envelope(core)
    value = authority._decode_canonical(
        durable_receipt(core, fresh), "durable admission receipt"
    )
    claims = value["claims"]
    assert isinstance(claims, dict)
    claims[field] = hostile
    with pytest.raises(authority.PublicSoakAuthorityError, match=message):
        validate_durable(core, fresh, authority._canonical_json(value))


def test_durable_receipt_rejects_a_substituted_fresh_envelope() -> None:
    core = subject_core()
    fresh = envelope(core)
    other = authority._decode_canonical(fresh)
    claims = other["claims"]
    assert isinstance(claims, dict)
    claims["replay_id"] = digest("other-replay")
    with pytest.raises(authority.PublicSoakAuthorityError, match="envelope bytes|replay"):
        validate_durable(core, authority._canonical_json(other),
                         durable_receipt(core, fresh))


@pytest.mark.parametrize("entrypoint", ("consume", "historical"))
def test_public_interfaces_fail_before_structural_parsing(
    monkeypatch: pytest.MonkeyPatch, entrypoint: str,
) -> None:
    called = False

    def forbidden(*_args: object, **_kwargs: object) -> object:
        nonlocal called
        called = True
        raise AssertionError("untrusted authority input was parsed")

    monkeypatch.setattr(authority, "validate_envelope_claims", forbidden)
    monkeypatch.setattr(authority, "validate_durable_admission_receipt_claims", forbidden)
    with pytest.raises(authority.PublicSoakAuthorityError,
                       match=authority.AUTHORITY_SCHEMA):
        if entrypoint == "consume":
            authority.consume_fresh_public_soak_admission(
                b"attacker controlled", subject_core=subject_core(),
                completed_at_unix_ms=1)
        else:
            authority.verify_authenticated_public_soak_authority_envelope(
                b"attacker controlled",
                durable_admission_receipt=b"attacker controlled",
                subject_core=subject_core(), completed_at_unix_ms=1)
    assert called is False


@pytest.mark.parametrize(
    "payload",
    (b'{"x":1,"x":2}\n', b'{"x":NaN}\n', b'{"x":Infinity}\n'),
)
def test_authority_json_is_strict(payload: bytes) -> None:
    with pytest.raises(authority.PublicSoakAuthorityError):
        authority._decode_canonical(payload)


def test_authority_and_broker_signing_bytes_are_distinct_domains() -> None:
    core = subject_core()
    fresh = envelope(core)
    durable = durable_receipt(core, fresh)
    authority_bytes = authority.authority_envelope_signing_bytes(fresh)
    broker_bytes = authority.durable_admission_receipt_signing_bytes(durable)
    assert authority_bytes.startswith(authority.AUTHORITY_SIGNATURE_DOMAIN)
    assert broker_bytes.startswith(authority.BROKER_SIGNATURE_DOMAIN)
    assert authority.AUTHORITY_SIGNATURE_DOMAIN != authority.BROKER_SIGNATURE_DOMAIN
    assert authority_bytes != broker_bytes


def test_every_authority_envelope_field_is_covered_by_signing_bytes() -> None:
    core = subject_core()
    original = authority._decode_canonical(envelope(core))
    baseline = authority.authority_envelope_signing_bytes(
        authority._canonical_json(original)
    )
    mutations: tuple[tuple[str, str | None, object], ...] = (
        ("schema", None, "hostile-authority-schema"),
        ("schema_version", None, 2),
        ("authority_key_id", None, digest("other-key")),
        ("signature_algorithm", None, "hostile-signature"),
        ("claims", "schema", "hostile-claims-schema"),
        ("claims", "subject_digest", digest("other-subject")),
        ("claims", "replay_namespace", "hostile-replay-namespace"),
        ("claims", "replay_id", digest("other-replay")),
        ("claims", "issued_at_unix_ms", ISSUED_MS + 1),
        ("claims", "expires_at_unix_ms", EXPIRES_MS + 1),
    )
    for outer, inner, hostile in mutations:
        changed = copy.deepcopy(original)
        if inner is None:
            changed[outer] = hostile
        else:
            nested = changed[outer]
            assert isinstance(nested, dict)
            nested[inner] = hostile
        assert authority.authority_envelope_signing_bytes(
            authority._canonical_json(changed)
        ) != baseline
    signature_only = copy.deepcopy(original)
    signature_only["signature"] = "ef" * 64
    assert authority.authority_envelope_signing_bytes(
        authority._canonical_json(signature_only)
    ) == baseline


def test_every_durable_receipt_field_is_covered_by_signing_bytes() -> None:
    core = subject_core()
    fresh = envelope(core)
    original = authority._decode_canonical(
        durable_receipt(core, fresh), "durable admission receipt"
    )
    baseline = authority.durable_admission_receipt_signing_bytes(
        authority._canonical_json(original)
    )
    claims = original["claims"]
    assert isinstance(claims, dict)
    mutations: tuple[tuple[str, str | None, object], ...] = (
        ("schema", None, "hostile-durable-schema"),
        ("schema_version", None, 2),
        ("broker_key_id", None, digest("other-broker")),
        ("signature_algorithm", None, "hostile-signature"),
        ("claims", "schema", "hostile-durable-claims"),
        ("claims", "decision", "rejected"),
        ("claims", "receipt_id", digest("other-receipt")),
        ("claims", "subject_digest", digest("other-subject")),
        ("claims", "authority_envelope_sha256", digest("other-envelope")),
        ("claims", "authority_key_id", digest("other-authority")),
        ("claims", "replay_namespace", "hostile-replay-namespace"),
        ("claims", "replay_id", digest("other-replay")),
        ("claims", "admitted_at_unix_ms", ADMITTED_MS + 1),
    )
    for outer, inner, hostile in mutations:
        changed = copy.deepcopy(original)
        if inner is None:
            changed[outer] = hostile
        else:
            nested = changed[outer]
            assert isinstance(nested, dict)
            nested[inner] = hostile
        assert authority.durable_admission_receipt_signing_bytes(
            authority._canonical_json(changed)
        ) != baseline
    signature_only = copy.deepcopy(original)
    signature_only["signature"] = "ef" * 64
    assert authority.durable_admission_receipt_signing_bytes(
        authority._canonical_json(signature_only)
    ) == baseline


def test_broker_and_authority_key_ids_must_be_distinct() -> None:
    core = subject_core()
    fresh = envelope(core)
    value = authority._decode_canonical(
        durable_receipt(core, fresh), "durable admission receipt"
    )
    fresh_value = authority._decode_canonical(fresh)
    value["broker_key_id"] = fresh_value["authority_key_id"]
    with pytest.raises(authority.PublicSoakAuthorityError, match="must be distinct"):
        validate_durable(core, fresh, authority._canonical_json(value))
