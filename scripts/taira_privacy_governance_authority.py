#!/usr/bin/env python3
"""Fail-closed shared authority contract for Exact12 governance setup.

This module deliberately implements no signer and no transport.  It fixes the
public contract implemented by the installed native signer service without allowing
Python structure checks, caller-provided paths, environment markers, or
candidate-controlled keys to become governance authority.

The release controller is root-owned, while its validator/action-driver child
runs as a deliberately hostile runtime UID.  A provisioned broker must be a
distinct service behind the fixed Unix socket and root-installed public
binding named below.  The binding's kernel-reported client UID, never a request
field, must authenticate only the controller/authority closer before request
decoding.  The hostile runtime UID must be rejected even for a byte-perfect
request.  A distinct service UID owns the socket and replay journal; a distinct
administrator UID provisions and rotates the encrypted genesis-authority key.
The root closer may hand the resulting public signed transaction bytes to the
runtime case, but credentials, private keys, socket paths, and Torii endpoints
never cross action-driver IPC.

For every signature the native broker must decode and semantically validate a
single canonical Iroha transaction payload.  Its authority must equal the
fixed account/public key proven by the exact signed reset genesis; its sole
instruction must be the requested canonical Exact12 privacy-governance
activation (or another explicitly versioned shared setup instruction in a
future schema); its NetworkId, creation time, TTL, nonce, fee intent, and
metadata must be exact.  The non-genesis transaction NetworkId must equal the
exact reset genesis header hash, as required by ``NetworkId::from_genesis_hash``.
The broker must bind candidate/source/Cargo/workspace,
signed and unsigned genesis, activation/template, sealed controller, host,
installation, four-peer/supervisor, run nonce/time/expiry, and replay identity.
The request carries both the byte-exact canonical ``TransactionPayload`` and
the native typed payload prehash; the broker must re-encode the decoded payload
byte-identically and rederive that prehash before signing it.
It must durably commit a predecessor-bound replay/audit record before returning
the signed transaction and an authenticated receipt.

The response must be verified by a separately pinned native semantic verifier
against the root-installed binding and live broker audit head.  Transaction
signature verification alone does not authenticate peer credentials, policy,
freshness, or replay state.  No implementation may reuse the candidate,
release, receipt, BOI, deploy, publication, validator, or action-driver signer
as this trust root.

Until that service, binding, verifier, and live provisioning exist, both public
entrypoints below fail before socket/path/output/replay access.  The private
helpers only exercise canonical *untrusted* wire shapes in source tests.
"""

from __future__ import annotations

import base64
import binascii
import hashlib
import json
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Mapping, NoReturn, Sequence

try:
    from . import taira_authority_client
except ImportError:
    import taira_authority_client


REQUEST_SCHEMA = "iroha.taira.privacy_governance_authority_request"
RECEIPT_SCHEMA = "iroha.taira.privacy_governance_authority_receipt"
BINDING_SCHEMA = "iroha.taira.privacy_governance_authority_binding"
SCHEMA_VERSION = 1
OPERATION = "sign-exact12-privacy-governance-transaction-v1"
AUTHORITY_ENVELOPE_SCHEMA = "iroha.taira.privacy_governance_authority.v1"
REPLAY_NAMESPACE = "iroha.taira.privacy_governance_authority_replay.v1"
LEGACY_VERANGE_AUTHORITY_ENVELOPE_SCHEMA = (
    "iroha.taira.verange_qualification_governance_authority.v1"
)
LEGACY_VERANGE_REPLAY_NAMESPACE = (
    "iroha.taira.verange_qualification_governance_authority_replay.v1"
)
PROVISIONING_BARRIER = (
    "MissingExactGenesisSourceClosedControllerSetupAuthorityIdentity"
)
GOVERNANCE_ROLE = taira_authority_client.ROLE_REGISTRY["privacy-governance"]
FIXED_BINDING_PATH = GOVERNANCE_ROLE.binding_path
FIXED_REQUEST_SOCKET = GOVERNANCE_ROLE.request_socket
SERVICE_ID = GOVERNANCE_ROLE.service_id
ADMINISTRATOR_ID = GOVERNANCE_ROLE.administrator_id
ROLE = GOVERNANCE_ROLE.role
REQUEST_ID_DOMAIN = b"iroha.taira.privacy_governance_authority_request.v1\0"
MAX_ACTIVATION_BYTES = 4 * 1024 * 1024
MAX_REQUEST_BYTES = 8 * 1024 * 1024
MAX_RECEIPT_BYTES = 16 * 1024 * 1024
MAX_TRANSACTION_PAYLOAD_BYTES = 8 * 1024 * 1024
MAX_SIGNED_TRANSACTION_BYTES = 8 * 1024 * 1024
MAX_RESPONSE_ATTESTATION_BYTES = 64 * 1024
MAX_TEXT_BYTES = 4096
MIN_ACTIVATION_DELAY_BLOCKS = 300
MAX_REQUEST_LIFETIME_MILLIS = 15 * 60 * 1000
MAX_TRANSACTION_CREATION_TIME_MILLIS = 2**63 - 1
MAX_TRANSACTION_NONCE = 2**32 - 1
MAX_UID_U32 = 2**32 - 1
MAX_U64 = 2**64 - 1
TRANSACTION_PAYLOAD_CODEC = "iroha.TransactionPayload/norito-adaptive-default-v1"
TRANSACTION_PAYLOAD_PREHASH = "iroha.HashOf<TransactionPayload>/v1"
TRANSACTION_DOMAIN = "network-from-reset-genesis-header-hash-v1"
TRANSACTION_EXECUTABLE_KIND = "single-direct-instruction-v1"
SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")
PUBLIC_KEY_RE = re.compile(r"ed0120[0-9A-F]{64}")

RETAINED_PROTOCOLS = (
    "zk-ace-pq-authorization-v0",
    "anonymous-pgc-k-out-of-n-v1",
    "verange-transparent-range-v1",
    "iroha-zk-ams-v1",
    "vega-existing-credential-zk-v0",
    "iroha-zk-x509-stark-p256-v0",
    "iroha-jindo-polynomial-commitment-v0",
    "iroha-bootle-lantern-anoncred-v1",
    "orchard-halo2-actions-v1",
    "monero-fcmp-plus-plus-v1",
    "iroha-ivm-private-note-stark-v1",
    "pq-masp-stark-v0",
)


class PrivacyGovernanceAuthorityError(RuntimeError):
    """The shared governance authority is absent, invalid, or unauthenticated."""


def _fail(message: str) -> NoReturn:
    raise PrivacyGovernanceAuthorityError(message)


def _require_provisioned_privacy_governance_authority_v1(
    *, require_signing: bool = True
) -> int:
    """Authenticate the fixed privacy-governance binding and live service."""

    try:
        status = taira_authority_client.preflight(
            "privacy-governance", require_signing=require_signing
        )
    except taira_authority_client.TairaAuthorityClientError as error:
        raise PrivacyGovernanceAuthorityError(
            f"{PROVISIONING_BARRIER}: fixed {AUTHORITY_ENVELOPE_SCHEMA} "
            f"authority and {REPLAY_NAMESPACE} service are unavailable: {error}"
        ) from error
    client_uid = status.get("client_uid")
    if (
        isinstance(client_uid, bool)
        or not isinstance(client_uid, int)
        or client_uid <= 0
        or client_uid >= MAX_UID_U32
    ):
        _fail("authenticated privacy-governance binding omitted its client UID")
    return client_uid


def _sha256(value: object, label: str, *, nonzero: bool = True) -> str:
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        _fail(f"{label} must be one lowercase SHA-256 digest")
    if nonzero and value == "0" * 64:
        _fail(f"{label} must be nonzero")
    return value


def _hash32(value: object, label: str, *, nonzero: bool = True) -> str:
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        _fail(f"{label} must be one lowercase 32-byte hash")
    if nonzero and value == "0" * 64:
        _fail(f"{label} must be nonzero")
    if int(value[-2:], 16) & 1 == 0:
        _fail(f"{label} must carry the canonical Iroha marker bit")
    return value


def _commit(value: object, label: str) -> str:
    if (
        not isinstance(value, str)
        or COMMIT_RE.fullmatch(value) is None
        or value == "0" * 40
    ):
        _fail(f"{label} must be one nonzero full lowercase source commit")
    return value


def _text(value: object, label: str) -> str:
    return _bounded_ascii(value, label, MAX_TEXT_BYTES)


def _bounded_ascii(value: object, label: str, maximum_bytes: int) -> str:
    if (
        not isinstance(value, str)
        or not value
        or not value.isascii()
        or len(value.encode("ascii")) > maximum_bytes
    ):
        _fail(f"{label} must be bounded nonempty ASCII")
    return value


def _positive_int(value: object, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        _fail(f"{label} must be one positive integer")
    return value


def _bounded_positive_int(value: object, label: str, maximum: int) -> int:
    parsed = _positive_int(value, label)
    if parsed > maximum:
        _fail(f"{label} exceeds its maximum")
    return parsed


def _nonnegative_int(value: object, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        _fail(f"{label} must be one nonnegative integer")
    return value


def _bounded_nonnegative_int(value: object, label: str, maximum: int) -> int:
    parsed = _nonnegative_int(value, label)
    if parsed > maximum:
        _fail(f"{label} exceeds its maximum")
    return parsed


def _canonical(value: object) -> bytes:
    try:
        encoded = (
            json.dumps(
                value,
                allow_nan=False,
                ensure_ascii=True,
                separators=(",", ":"),
                sort_keys=True,
            )
            + "\n"
        ).encode("ascii")
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise PrivacyGovernanceAuthorityError(
            f"governance authority value is not canonical JSON: {exc}"
        ) from exc
    return encoded


def _strict_object(payload: bytes, label: str, maximum: int) -> dict[str, object]:
    if not isinstance(payload, bytes) or not payload or len(payload) > maximum:
        _fail(f"{label} violates its byte bound")

    def object_pairs(pairs: list[tuple[str, object]]) -> dict[str, object]:
        result: dict[str, object] = {}
        for key, value in pairs:
            if key in result:
                _fail(f"{label} repeats field {key!r}")
            result[key] = value
        return result

    try:
        value = json.loads(payload.decode("ascii"), object_pairs_hook=object_pairs)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise PrivacyGovernanceAuthorityError(
            f"{label} is not strict canonical ASCII JSON"
        ) from exc
    if not isinstance(value, dict) or _canonical(value) != payload:
        _fail(f"{label} is not the one canonical JSON encoding")
    return value


def _exact_fields(
    value: Mapping[str, object], expected: Sequence[str], label: str
) -> None:
    if set(value) != set(expected):
        _fail(f"{label} fields are not exact")


def _canonical_base64(value: object, label: str, maximum_bytes: int) -> bytes:
    encoded = _bounded_ascii(
        value,
        f"{label} base64",
        4 * ((maximum_bytes + 2) // 3),
    )
    try:
        decoded = base64.b64decode(encoded, validate=True)
    except (ValueError, binascii.Error) as exc:
        raise PrivacyGovernanceAuthorityError(
            f"{label} is not valid canonical base64"
        ) from exc
    if (
        not decoded
        or len(decoded) > maximum_bytes
        or base64.b64encode(decoded).decode("ascii") != encoded
    ):
        _fail(f"{label} is not bounded canonical base64")
    return decoded


@dataclass(frozen=True)
class UntrustedGovernanceAuthorityRequestV1:
    """Canonical request bytes only; this object carries no authority."""

    canonical_bytes: bytes
    request_id: str
    request_sha256: str


@dataclass(frozen=True)
class UntrustedGovernanceAuthorityReceiptV1:
    """Structurally decoded receipt only; signatures/replay are unverified."""

    canonical_bytes: bytes
    request_id: str
    signed_transaction_norito: bytes
    signed_transaction_sha256: str


@dataclass(frozen=True)
class AuthenticatedGovernanceAuthorityResultV1:
    """Distinct authenticated sidecars plus the decoded signed transaction."""

    authority_envelope: bytes
    durable_receipt: bytes
    receipt: UntrustedGovernanceAuthorityReceiptV1


def _build_untrusted_governance_authority_request_v1(
    *,
    protocol: str,
    activation_instruction_norito: bytes,
    activation_instruction_sha256: str,
    compiled_profile_sha256: str,
    proposed_at_height: int,
    activate_at_height: int,
    transaction_payload_norito: bytes,
    transaction_payload_sha256: str,
    transaction_payload_hash_hex: str,
    transaction_creation_time_millis: int,
    transaction_ttl_millis: int,
    transaction_nonce: int,
    candidate_binding_sha256: str,
    source_commit: str,
    dpn_validator_release_commit: str,
    cargo_lock_sha256: str,
    workspace_source_manifest_sha256: str,
    reset_manifest_sha256: str,
    signed_genesis_sha256: str,
    unsigned_genesis_sha256: str,
    genesis_expected_hash: str,
    genesis_public_key: str,
    genesis_authority_account_id: str,
    network_id_hex: str,
    controller_digest: str,
    controller_host_id: str,
    controller_installation_id: str,
    four_peer_binding_sha256: str,
    supervisor_binding_sha256: str,
    run_nonce: str,
    issued_at_unix_millis: int,
    expires_at_unix_millis: int,
) -> UntrustedGovernanceAuthorityRequestV1:
    """Build a self-consistent but explicitly unauthenticated request shape."""

    if protocol not in RETAINED_PROTOCOLS:
        _fail("governance authority request selected an unknown Exact12 protocol")
    if (
        not isinstance(activation_instruction_norito, bytes)
        or not activation_instruction_norito
        or len(activation_instruction_norito) > MAX_ACTIVATION_BYTES
    ):
        _fail("activation instruction violates its byte bound")
    activation_digest = _sha256(
        activation_instruction_sha256, "activation instruction digest"
    )
    if hashlib.sha256(activation_instruction_norito).hexdigest() != activation_digest:
        _fail("activation instruction differs from its digest")
    proposed = _bounded_positive_int(proposed_at_height, "proposal height", MAX_U64)
    activate = _bounded_positive_int(activate_at_height, "activation height", MAX_U64)
    if (
        proposed > MAX_U64 - MIN_ACTIVATION_DELAY_BLOCKS
        or activate < proposed + MIN_ACTIVATION_DELAY_BLOCKS
    ):
        _fail("activation instruction violates the minimum governance delay")
    issued = _bounded_positive_int(issued_at_unix_millis, "issued time", MAX_U64)
    expires = _bounded_positive_int(expires_at_unix_millis, "expiry time", MAX_U64)
    if expires <= issued or expires - issued > MAX_REQUEST_LIFETIME_MILLIS:
        _fail("governance authority request lifetime is invalid")
    if (
        not isinstance(transaction_payload_norito, bytes)
        or not transaction_payload_norito
        or len(transaction_payload_norito) > MAX_TRANSACTION_PAYLOAD_BYTES
    ):
        _fail("canonical TransactionPayload violates its byte bound")
    transaction_payload_digest = _sha256(
        transaction_payload_sha256, "canonical TransactionPayload digest"
    )
    if (
        hashlib.sha256(transaction_payload_norito).hexdigest()
        != transaction_payload_digest
    ):
        _fail("canonical TransactionPayload differs from its digest")
    transaction_payload_hash = _hash32(
        transaction_payload_hash_hex, "native typed TransactionPayload prehash"
    )
    creation_time = _bounded_positive_int(
        transaction_creation_time_millis,
        "transaction creation time",
        MAX_TRANSACTION_CREATION_TIME_MILLIS,
    )
    transaction_ttl = _bounded_positive_int(
        transaction_ttl_millis,
        "transaction TTL",
        MAX_REQUEST_LIFETIME_MILLIS,
    )
    nonce = _bounded_positive_int(
        transaction_nonce, "transaction nonce", MAX_TRANSACTION_NONCE
    )
    if creation_time != issued or transaction_ttl != expires - issued:
        _fail(
            "transaction creation time and TTL must equal the exact request window"
        )
    public_key = _text(genesis_public_key, "genesis public key")
    if PUBLIC_KEY_RE.fullmatch(public_key) is None:
        _fail("genesis public key is not one canonical Ed25519 key")
    genesis_authority = _text(
        genesis_authority_account_id, "genesis authority account ID"
    )
    genesis_hash = _hash32(genesis_expected_hash, "genesis expected hash")
    network_id = _hash32(network_id_hex, "NetworkId")
    if network_id != genesis_hash:
        _fail("NetworkId must equal the exact reset genesis header hash")

    body: dict[str, object] = {
        "activation": {
            "activate_at_height": activate,
            "compiled_profile_sha256": _sha256(
                compiled_profile_sha256, "compiled profile digest"
            ),
            "instruction_norito_base64": base64.b64encode(
                activation_instruction_norito
            ).decode("ascii"),
            "instruction_sha256": activation_digest,
            "proposed_at_height": proposed,
            "protocol": protocol,
        },
        "authority_envelope_schema": AUTHORITY_ENVELOPE_SCHEMA,
        "candidate": {
            "candidate_binding_sha256": _sha256(
                candidate_binding_sha256, "candidate binding"
            ),
            "cargo_lock_sha256": _sha256(cargo_lock_sha256, "Cargo.lock digest"),
            "dpn_validator_release_commit": _commit(
                dpn_validator_release_commit, "DPN validator release commit"
            ),
            "source_commit": _commit(source_commit, "source commit"),
            "workspace_source_manifest_sha256": _sha256(
                workspace_source_manifest_sha256,
                "workspace source manifest digest",
            ),
        },
        "controller": {
            "digest": _sha256(controller_digest, "controller digest"),
            "host_id": _text(controller_host_id, "controller host ID"),
            "installation_id": _text(
                controller_installation_id, "controller installation ID"
            ),
        },
        "fleet": {
            "four_peer_binding_sha256": _sha256(
                four_peer_binding_sha256, "four-peer binding"
            ),
            "supervisor_binding_sha256": _sha256(
                supervisor_binding_sha256, "supervisor binding"
            ),
        },
        "genesis": {
            "authority_account_id": genesis_authority,
            "expected_hash": genesis_hash,
            "network_id_hex": network_id,
            "public_key": public_key,
            "reset_manifest_sha256": _sha256(
                reset_manifest_sha256, "reset manifest digest"
            ),
            "signed_genesis_sha256": _sha256(
                signed_genesis_sha256, "signed genesis digest"
            ),
            "unsigned_genesis_sha256": _sha256(
                unsigned_genesis_sha256, "unsigned genesis digest"
            ),
        },
        "operation": OPERATION,
        "run": {
            "expires_at_unix_millis": expires,
            "issued_at_unix_millis": issued,
            "nonce": _sha256(run_nonce, "run nonce"),
            "replay_namespace": REPLAY_NAMESPACE,
        },
        "schema": REQUEST_SCHEMA,
        "schema_version": SCHEMA_VERSION,
        "transaction": {
            "admission_intent": {
                "intent": "queue_plan_synced",
                "value": None,
            },
            "attachments": None,
            "authority_account_id": genesis_authority,
            "creation_time_millis": creation_time,
            "domain": TRANSACTION_DOMAIN,
            "executable_kind": TRANSACTION_EXECUTABLE_KIND,
            "fee_payment": {
                "charge_limits": [],
                "gas_limit": None,
                "payer": "authority",
            },
            "instruction_norito_sha256": activation_digest,
            "metadata": {},
            "network_id_hex": network_id,
            "nonce": nonce,
            "payload_codec": TRANSACTION_PAYLOAD_CODEC,
            "payload_hash_hex": transaction_payload_hash,
            "payload_norito_base64": base64.b64encode(
                transaction_payload_norito
            ).decode("ascii"),
            "payload_prehash": TRANSACTION_PAYLOAD_PREHASH,
            "payload_sha256": transaction_payload_digest,
            "time_to_live_millis": transaction_ttl,
        },
    }
    body_bytes = _canonical(body)
    request_id = hashlib.sha256(REQUEST_ID_DOMAIN + body_bytes).hexdigest()
    payload = _canonical({**body, "request_id": request_id})
    if len(payload) > MAX_REQUEST_BYTES:
        _fail("governance authority request exceeds its byte bound")
    return UntrustedGovernanceAuthorityRequestV1(
        canonical_bytes=payload,
        request_id=request_id,
        request_sha256=hashlib.sha256(payload).hexdigest(),
    )


_REQUEST_FIELDS = (
    "activation",
    "authority_envelope_schema",
    "candidate",
    "controller",
    "fleet",
    "genesis",
    "operation",
    "request_id",
    "run",
    "schema",
    "schema_version",
    "transaction",
)
_REQUEST_ACTIVATION_FIELDS = (
    "activate_at_height",
    "compiled_profile_sha256",
    "instruction_norito_base64",
    "instruction_sha256",
    "proposed_at_height",
    "protocol",
)
_REQUEST_CANDIDATE_FIELDS = (
    "candidate_binding_sha256",
    "cargo_lock_sha256",
    "dpn_validator_release_commit",
    "source_commit",
    "workspace_source_manifest_sha256",
)
_REQUEST_CONTROLLER_FIELDS = ("digest", "host_id", "installation_id")
_REQUEST_FLEET_FIELDS = (
    "four_peer_binding_sha256",
    "supervisor_binding_sha256",
)
_REQUEST_GENESIS_FIELDS = (
    "authority_account_id",
    "expected_hash",
    "network_id_hex",
    "public_key",
    "reset_manifest_sha256",
    "signed_genesis_sha256",
    "unsigned_genesis_sha256",
)
_REQUEST_RUN_FIELDS = (
    "expires_at_unix_millis",
    "issued_at_unix_millis",
    "nonce",
    "replay_namespace",
)
_REQUEST_TRANSACTION_FIELDS = (
    "admission_intent",
    "attachments",
    "authority_account_id",
    "creation_time_millis",
    "domain",
    "executable_kind",
    "fee_payment",
    "instruction_norito_sha256",
    "metadata",
    "network_id_hex",
    "nonce",
    "payload_codec",
    "payload_hash_hex",
    "payload_norito_base64",
    "payload_prehash",
    "payload_sha256",
    "time_to_live_millis",
)


def _validated_untrusted_request_value_v1(
    request: UntrustedGovernanceAuthorityRequestV1,
) -> dict[str, object]:
    """Recheck only canonical request shape; this still grants no authority."""

    if not isinstance(request, UntrustedGovernanceAuthorityRequestV1):
        _fail("governance authority request has the wrong in-memory type")
    value = _strict_object(
        request.canonical_bytes,
        "governance authority request",
        MAX_REQUEST_BYTES,
    )
    _exact_fields(value, _REQUEST_FIELDS, "governance authority request")
    schema_version = _positive_int(value["schema_version"], "request schema version")
    if (
        value["schema"] != REQUEST_SCHEMA
        or schema_version != SCHEMA_VERSION
        or value["authority_envelope_schema"] != AUTHORITY_ENVELOPE_SCHEMA
        or value["operation"] != OPERATION
        or value["request_id"] != request.request_id
        or hashlib.sha256(request.canonical_bytes).hexdigest()
        != request.request_sha256
    ):
        _fail("governance authority request contract or self-binding differs")
    body = dict(value)
    body.pop("request_id")
    expected_request_id = hashlib.sha256(
        REQUEST_ID_DOMAIN + _canonical(body)
    ).hexdigest()
    if expected_request_id != request.request_id:
        _fail("governance authority request ID differs from its canonical body")

    candidate = value["candidate"]
    controller = value["controller"]
    fleet = value["fleet"]
    genesis = value["genesis"]
    run = value["run"]
    transaction = value["transaction"]
    activation = value["activation"]
    if not all(
        isinstance(item, dict)
        for item in (
            activation,
            candidate,
            controller,
            fleet,
            genesis,
            run,
            transaction,
        )
    ):
        _fail("governance authority request nested contracts are not objects")
    assert isinstance(candidate, dict)
    assert isinstance(controller, dict)
    assert isinstance(fleet, dict)
    assert isinstance(genesis, dict)
    assert isinstance(run, dict)
    assert isinstance(transaction, dict)
    assert isinstance(activation, dict)
    _exact_fields(activation, _REQUEST_ACTIVATION_FIELDS, "request activation")
    _exact_fields(candidate, _REQUEST_CANDIDATE_FIELDS, "request candidate")
    _exact_fields(controller, _REQUEST_CONTROLLER_FIELDS, "request controller")
    _exact_fields(fleet, _REQUEST_FLEET_FIELDS, "request fleet")
    _exact_fields(genesis, _REQUEST_GENESIS_FIELDS, "request genesis")
    _exact_fields(run, _REQUEST_RUN_FIELDS, "request run")
    _exact_fields(transaction, _REQUEST_TRANSACTION_FIELDS, "request transaction")
    protocol = activation["protocol"]
    proposed = _bounded_positive_int(
        activation["proposed_at_height"], "proposal height", MAX_U64
    )
    activate = _bounded_positive_int(
        activation["activate_at_height"], "activation height", MAX_U64
    )
    activation_instruction = _canonical_base64(
        activation["instruction_norito_base64"],
        "activation instruction",
        MAX_ACTIVATION_BYTES,
    )
    activation_digest = _sha256(
        activation["instruction_sha256"], "activation instruction digest"
    )
    if (
        protocol not in RETAINED_PROTOCOLS
        or proposed > MAX_U64 - MIN_ACTIVATION_DELAY_BLOCKS
        or activate < proposed + MIN_ACTIVATION_DELAY_BLOCKS
        or hashlib.sha256(activation_instruction).hexdigest() != activation_digest
    ):
        _fail("request activation instruction contract differs")
    _sha256(activation["compiled_profile_sha256"], "compiled profile digest")
    for field in (
        "candidate_binding_sha256",
        "cargo_lock_sha256",
        "workspace_source_manifest_sha256",
    ):
        _sha256(candidate[field], field.replace("_", " "))
    _commit(candidate["source_commit"], "source commit")
    _commit(candidate["dpn_validator_release_commit"], "DPN validator release commit")
    _sha256(controller["digest"], "controller digest")
    _text(controller["host_id"], "controller host ID")
    _text(controller["installation_id"], "controller installation ID")
    _sha256(fleet["four_peer_binding_sha256"], "four-peer binding")
    _sha256(fleet["supervisor_binding_sha256"], "supervisor binding")
    genesis_hash = _hash32(
        genesis["expected_hash"], "request genesis expected hash"
    )
    if (
        genesis["network_id_hex"] != genesis_hash
        or transaction["network_id_hex"] != genesis_hash
        or transaction["authority_account_id"] != genesis["authority_account_id"]
        or transaction["instruction_norito_sha256"]
        != activation.get("instruction_sha256")
        or transaction["domain"] != TRANSACTION_DOMAIN
        or transaction["executable_kind"] != TRANSACTION_EXECUTABLE_KIND
        or transaction["payload_codec"] != TRANSACTION_PAYLOAD_CODEC
        or transaction["payload_prehash"] != TRANSACTION_PAYLOAD_PREHASH
        or transaction["admission_intent"]
        != {"intent": "queue_plan_synced", "value": None}
        or transaction["attachments"] is not None
        or transaction["metadata"] != {}
        or transaction["fee_payment"]
        != {"charge_limits": [], "gas_limit": None, "payer": "authority"}
    ):
        _fail("request transaction intent differs from the exact reset authority")
    issued = _bounded_positive_int(
        run["issued_at_unix_millis"], "request issued time", MAX_U64
    )
    expires = _bounded_positive_int(
        run["expires_at_unix_millis"], "request expiry time", MAX_U64
    )
    _sha256(run["nonce"], "request run nonce")
    transaction_ttl = _bounded_positive_int(
        transaction["time_to_live_millis"],
        "request transaction TTL",
        MAX_REQUEST_LIFETIME_MILLIS,
    )
    if (
        run["replay_namespace"] != REPLAY_NAMESPACE
        or expires <= issued
        or expires - issued > MAX_REQUEST_LIFETIME_MILLIS
        or transaction["creation_time_millis"] != issued
        or transaction_ttl != expires - issued
    ):
        _fail("request transaction time or replay contract differs")
    _bounded_positive_int(
        transaction["nonce"], "request transaction nonce", MAX_TRANSACTION_NONCE
    )
    _bounded_positive_int(
        transaction["creation_time_millis"],
        "request transaction creation time",
        MAX_TRANSACTION_CREATION_TIME_MILLIS,
    )
    payload = _canonical_base64(
        transaction["payload_norito_base64"],
        "canonical TransactionPayload",
        MAX_TRANSACTION_PAYLOAD_BYTES,
    )
    payload_sha256 = _sha256(
        transaction["payload_sha256"], "canonical TransactionPayload digest"
    )
    _hash32(
        transaction["payload_hash_hex"], "native typed TransactionPayload prehash"
    )
    if hashlib.sha256(payload).hexdigest() != payload_sha256:
        _fail("canonical TransactionPayload differs from its digest")
    public_key = _text(genesis["public_key"], "request genesis public key")
    if PUBLIC_KEY_RE.fullmatch(public_key) is None:
        _fail("request genesis public key is not canonical Ed25519")
    _text(genesis["authority_account_id"], "request genesis authority account ID")
    _hash32(genesis["network_id_hex"], "request NetworkId")
    for field in (
        "reset_manifest_sha256",
        "signed_genesis_sha256",
        "unsigned_genesis_sha256",
    ):
        _sha256(genesis[field], field.replace("_", " "))
    return value


_RECEIPT_FIELDS = (
    "administrator_uid",
    "audit_committed_head_sha256",
    "audit_live_head_sha256",
    "audit_previous_head_sha256",
    "audit_sequence",
    "authority_envelope_schema",
    "authority_account_id",
    "authority_public_key",
    "binding_schema",
    "binding_sha256",
    "broker_binary_sha256",
    "kernel_peer_uid",
    "key_revision",
    "operation_id",
    "policy_revision",
    "policy_sha256",
    "replay_namespace",
    "request_id",
    "request_sha256",
    "response_attestation_base64",
    "response_attestation_sha256",
    "schema",
    "schema_version",
    "service_id",
    "service_uid",
    "signer_role",
    "signed_transaction_norito_base64",
    "signed_transaction_sha256",
    "status",
    "transaction_hash_hex",
)


def _validate_untrusted_governance_authority_receipt_structure_v1(
    request: UntrustedGovernanceAuthorityRequestV1,
    receipt_payload: bytes,
    *,
    authenticated_client_uid: int,
) -> UntrustedGovernanceAuthorityReceiptV1:
    """Validate self-consistent receipt structure without granting authority."""

    request_value = _validated_untrusted_request_value_v1(request)
    value = _strict_object(
        receipt_payload,
        "governance authority receipt",
        MAX_RECEIPT_BYTES,
    )
    _exact_fields(value, _RECEIPT_FIELDS, "governance authority receipt")
    schema_version = _positive_int(value["schema_version"], "receipt schema version")
    if (
        value["schema"] != RECEIPT_SCHEMA
        or schema_version != SCHEMA_VERSION
        or value["authority_envelope_schema"] != AUTHORITY_ENVELOPE_SCHEMA
        or value["binding_schema"] != BINDING_SCHEMA
        or value["status"] != "signed"
        or value["request_id"] != request.request_id
        or value["request_sha256"] != request.request_sha256
        or value["replay_namespace"] != REPLAY_NAMESPACE
    ):
        _fail("governance authority receipt contract or request binding differs")
    administrator_uid = _bounded_positive_int(
        value["administrator_uid"], "administrator UID", MAX_UID_U32
    )
    service_uid = _bounded_positive_int(
        value["service_uid"], "service UID", MAX_UID_U32
    )
    for field in ("audit_sequence", "key_revision", "policy_revision"):
        _bounded_positive_int(value[field], field.replace("_", " "), MAX_U64)
    if administrator_uid == service_uid:
        _fail("receipt service and administrator UIDs must be distinct")
    if (
        _bounded_nonnegative_int(
            value["kernel_peer_uid"], "kernel peer UID", MAX_UID_U32
        )
        != _bounded_positive_int(
            authenticated_client_uid,
            "authenticated binding client UID",
            MAX_UID_U32,
        )
    ):
        _fail("receipt was not issued to the authenticated binding client UID")
    signed_transaction_sha256 = _sha256(
        value["signed_transaction_sha256"], "signed transaction sha256"
    )
    audit_committed_head = _sha256(
        value["audit_committed_head_sha256"], "audit committed head sha256"
    )
    audit_live_head = _sha256(
        value["audit_live_head_sha256"], "audit live head sha256"
    )
    audit_previous_head = _sha256(
        value["audit_previous_head_sha256"], "audit previous head sha256"
    )
    if (
        audit_committed_head != audit_live_head
        or audit_previous_head == audit_committed_head
    ):
        _fail("receipt audit heads do not prove one newly committed live record")
    for field in (
        "binding_sha256",
        "broker_binary_sha256",
        "operation_id",
        "policy_sha256",
        "request_id",
        "request_sha256",
        "response_attestation_sha256",
    ):
        _sha256(value[field], field.replace("_", " "))
    _hash32(value["transaction_hash_hex"], "Iroha transaction hash")
    request_genesis = request_value["genesis"]
    assert isinstance(request_genesis, dict)
    authority_account_id = _text(value["authority_account_id"], "authority account ID")
    public_key = _text(value["authority_public_key"], "authority public key")
    if PUBLIC_KEY_RE.fullmatch(public_key) is None:
        _fail("receipt authority public key is not canonical Ed25519")
    if (
        authority_account_id != request_genesis["authority_account_id"]
        or public_key != request_genesis["public_key"]
    ):
        _fail("receipt signer identity differs from the exact reset genesis authority")
    if value["service_id"] != SERVICE_ID or value["signer_role"] != ROLE:
        _fail("receipt service or signer role differs from the fixed authority")
    transaction = _canonical_base64(
        value["signed_transaction_norito_base64"],
        "signed transaction",
        MAX_SIGNED_TRANSACTION_BYTES,
    )
    response_attestation = _canonical_base64(
        value["response_attestation_base64"],
        "response attestation",
        MAX_RESPONSE_ATTESTATION_BYTES,
    )
    if (
        hashlib.sha256(transaction).hexdigest() != signed_transaction_sha256
        or hashlib.sha256(response_attestation).hexdigest()
        != value["response_attestation_sha256"]
    ):
        _fail("receipt transaction or response attestation differs from its self-hash")
    return UntrustedGovernanceAuthorityReceiptV1(
        canonical_bytes=receipt_payload,
        request_id=request.request_id,
        signed_transaction_norito=transaction,
        signed_transaction_sha256=signed_transaction_sha256,
    )


def request_authenticated_governance_transaction_v1(
    request: UntrustedGovernanceAuthorityRequestV1,
) -> AuthenticatedGovernanceAuthorityResultV1:
    """Request the exact validated transaction from the fixed native authority."""

    client_uid = _require_provisioned_privacy_governance_authority_v1()
    subject = _validated_untrusted_request_value_v1(request)
    try:
        result = taira_authority_client.authorize(
            "privacy-governance", subject
        )
    except taira_authority_client.TairaAuthorityClientError as error:
        raise PrivacyGovernanceAuthorityError(
            f"privacy-governance authority refused the exact request: {error}"
        ) from error
    receipt = _canonical(result.durable_receipt)
    structural = _validate_untrusted_governance_authority_receipt_structure_v1(
        request, receipt, authenticated_client_uid=client_uid
    )
    return AuthenticatedGovernanceAuthorityResultV1(
        authority_envelope=result.authority_envelope_bytes,
        durable_receipt=receipt,
        receipt=structural,
    )


def validate_authenticated_governance_receipt_v1(
    request: UntrustedGovernanceAuthorityRequestV1,
    receipt_payload: bytes,
    *,
    authority_envelope_payload: bytes | None = None,
) -> UntrustedGovernanceAuthorityReceiptV1:
    """Historically verify one receipt without replay consumption or re-signing."""

    client_uid = _require_provisioned_privacy_governance_authority_v1(
        require_signing=False
    )
    if authority_envelope_payload is None:
        _fail("governance authority envelope sidecar is required")
    structural = _validate_untrusted_governance_authority_receipt_structure_v1(
        request,
        receipt_payload,
        authenticated_client_uid=client_uid,
    )
    subject = _validated_untrusted_request_value_v1(request)
    receipt = _strict_object(
        receipt_payload, "governance authority receipt", MAX_RECEIPT_BYTES
    )
    try:
        envelope = taira_authority_client.decode_canonical_json(
            authority_envelope_payload, "governance authority envelope"
        )
    except taira_authority_client.TairaAuthorityClientError as error:
        raise PrivacyGovernanceAuthorityError(str(error)) from error
    try:
        taira_authority_client.verify_receipt(
            "privacy-governance",
            subject,
            authority_envelope=envelope,
            durable_receipt=receipt,
        )
    except taira_authority_client.TairaAuthorityClientError as error:
        raise PrivacyGovernanceAuthorityError(
            f"privacy-governance historical receipt verification failed: {error}"
        ) from error
    return structural
