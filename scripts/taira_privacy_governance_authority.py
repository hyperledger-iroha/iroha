#!/usr/bin/env python3
"""Fail-closed shared authority contract for Exact12 governance setup.

This module deliberately implements no signer and no transport.  It fixes the
public contract a future native signer broker must implement without allowing
Python structure checks, caller-provided paths, environment markers, or
candidate-controlled keys to become governance authority.

The release controller is root-owned, but its hostile validator/action-driver
child runs as the attested runtime UID.  A provisioned broker must therefore be
a distinct service behind the fixed Unix socket and root-installed public
binding named below.  The kernel-reported client UID, not a request field, must
authenticate the runtime client before request decoding.  A distinct service
UID owns the socket and replay journal; a distinct administrator UID provisions
and rotates the encrypted genesis-authority key.  Credentials, private keys,
socket paths, and Torii endpoints never cross action-driver IPC.

For every signature the native broker must decode and semantically validate a
single canonical Iroha transaction payload.  Its authority must equal the
fixed account/public key proven by the exact signed reset genesis; its sole
instruction must be the requested canonical Exact12 privacy-governance
activation (or another explicitly versioned shared setup instruction in a
future schema); its NetworkId, creation time, TTL, nonce, fee intent, and
metadata must be exact.  The broker must bind candidate/source/Cargo/workspace,
signed and unsigned genesis, activation/template, sealed controller, host,
installation, four-peer/supervisor, run nonce/time/expiry, and replay identity.
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


REQUEST_SCHEMA = "iroha.taira.privacy_governance_authority_request"
RECEIPT_SCHEMA = "iroha.taira.privacy_governance_authority_receipt"
BINDING_SCHEMA = "iroha.taira.privacy_governance_authority_binding"
SCHEMA_VERSION = 1
OPERATION = "sign-exact12-privacy-governance-transaction-v1"
AUTHORITY_ENVELOPE_SCHEMA = (
    "iroha.taira.verange_qualification_governance_authority.v1"
)
REPLAY_NAMESPACE = (
    "iroha.taira.verange_qualification_governance_authority_replay.v1"
)
PROVISIONING_BARRIER = (
    "MissingExactGenesisSourceClosedControllerSetupAuthorityIdentity"
)
FIXED_BINDING_PATH = Path(
    "/private/etc/iroha/privacy-governance-authority/binding-v1.norito"
)
FIXED_REQUEST_SOCKET = Path(
    "/private/var/run/iroha-privacy-governance-authority/request-v1.sock"
)
REQUEST_ID_DOMAIN = b"iroha.taira.privacy_governance_authority_request.v1\0"
MAX_ACTIVATION_BYTES = 4 * 1024 * 1024
MAX_REQUEST_BYTES = 8 * 1024 * 1024
MAX_RECEIPT_BYTES = 16 * 1024 * 1024
MAX_TEXT_BYTES = 4096
MIN_ACTIVATION_DELAY_BLOCKS = 300
MAX_REQUEST_LIFETIME_MILLIS = 15 * 60 * 1000
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


def _require_provisioned_privacy_governance_authority_v1() -> NoReturn:
    """Keep signing and verification closed until the native service exists."""

    _fail(
        f"{PROVISIONING_BARRIER}: no independently provisioned native Exact12 "
        "governance transaction signer broker, root-installed binding, live "
        "replay journal, or separately pinned semantic receipt verifier exists; "
        f"authority must use exact envelope {AUTHORITY_ENVELOPE_SCHEMA} and "
        f"replay namespace {REPLAY_NAMESPACE}"
    )


def _sha256(value: object, label: str, *, nonzero: bool = True) -> str:
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        _fail(f"{label} must be one lowercase SHA-256 digest")
    if nonzero and value == "0" * 64:
        _fail(f"{label} must be nonzero")
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
    if (
        not isinstance(value, str)
        or not value
        or not value.isascii()
        or len(value.encode("ascii")) > MAX_TEXT_BYTES
    ):
        _fail(f"{label} must be bounded nonempty ASCII")
    return value


def _positive_int(value: object, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        _fail(f"{label} must be one positive integer")
    return value


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


def _build_untrusted_governance_authority_request_v1(
    *,
    protocol: str,
    activation_instruction_norito: bytes,
    activation_instruction_sha256: str,
    compiled_profile_sha256: str,
    proposed_at_height: int,
    activate_at_height: int,
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
    controller_client_uid: int,
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
    proposed = _positive_int(proposed_at_height, "proposal height")
    activate = _positive_int(activate_at_height, "activation height")
    if activate < proposed + MIN_ACTIVATION_DELAY_BLOCKS:
        _fail("activation instruction violates the minimum governance delay")
    issued = _positive_int(issued_at_unix_millis, "issued time")
    expires = _positive_int(expires_at_unix_millis, "expiry time")
    if expires <= issued or expires - issued > MAX_REQUEST_LIFETIME_MILLIS:
        _fail("governance authority request lifetime is invalid")
    public_key = _text(genesis_public_key, "genesis public key")
    if PUBLIC_KEY_RE.fullmatch(public_key) is None:
        _fail("genesis public key is not one canonical Ed25519 key")

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
            "client_uid": _positive_int(controller_client_uid, "controller client UID"),
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
            "authority_account_id": _text(
                genesis_authority_account_id, "genesis authority account ID"
            ),
            "expected_hash": _sha256(genesis_expected_hash, "genesis expected hash"),
            "network_id_hex": _sha256(network_id_hex, "NetworkId"),
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


_RECEIPT_FIELDS = (
    "administrator_uid",
    "audit_committed_head_sha256",
    "audit_live_head_sha256",
    "audit_previous_head_sha256",
    "audit_sequence",
    "authority_account_id",
    "authority_public_key",
    "binding_sha256",
    "broker_binary_sha256",
    "client_uid",
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
    "signed_transaction_norito_base64",
    "signed_transaction_sha256",
    "status",
    "transaction_hash_hex",
)


def _validate_untrusted_governance_authority_receipt_structure_v1(
    request: UntrustedGovernanceAuthorityRequestV1,
    receipt_payload: bytes,
) -> UntrustedGovernanceAuthorityReceiptV1:
    """Validate self-consistent receipt structure without granting authority."""

    value = _strict_object(receipt_payload, "governance authority receipt", MAX_RECEIPT_BYTES)
    _exact_fields(value, _RECEIPT_FIELDS, "governance authority receipt")
    if (
        value["schema"] != RECEIPT_SCHEMA
        or value["schema_version"] != SCHEMA_VERSION
        or value["status"] != "signed"
        or value["request_id"] != request.request_id
        or value["request_sha256"] != request.request_sha256
        or value["replay_namespace"] != REPLAY_NAMESPACE
    ):
        _fail("governance authority receipt contract or request binding differs")
    for field in (
        "administrator_uid",
        "audit_sequence",
        "client_uid",
        "key_revision",
        "policy_revision",
        "service_uid",
    ):
        _positive_int(value[field], field.replace("_", " "))
    for field in (
        "audit_committed_head_sha256",
        "audit_live_head_sha256",
        "audit_previous_head_sha256",
        "binding_sha256",
        "broker_binary_sha256",
        "operation_id",
        "policy_sha256",
        "request_id",
        "request_sha256",
        "response_attestation_sha256",
        "signed_transaction_sha256",
        "transaction_hash_hex",
    ):
        _sha256(value[field], field.replace("_", " "))
    _text(value["authority_account_id"], "authority account ID")
    public_key = _text(value["authority_public_key"], "authority public key")
    if PUBLIC_KEY_RE.fullmatch(public_key) is None:
        _fail("receipt authority public key is not canonical Ed25519")
    _text(value["service_id"], "service ID")
    try:
        transaction = base64.b64decode(
            str(value["signed_transaction_norito_base64"]), validate=True
        )
        response_attestation = base64.b64decode(
            str(value["response_attestation_base64"]), validate=True
        )
    except (ValueError, binascii.Error) as exc:
        raise PrivacyGovernanceAuthorityError(
            "governance authority receipt contains invalid base64"
        ) from exc
    if (
        not transaction
        or hashlib.sha256(transaction).hexdigest() != value["signed_transaction_sha256"]
        or not response_attestation
        or hashlib.sha256(response_attestation).hexdigest()
        != value["response_attestation_sha256"]
    ):
        _fail("receipt transaction or response attestation differs from its self-hash")
    return UntrustedGovernanceAuthorityReceiptV1(
        canonical_bytes=receipt_payload,
        request_id=request.request_id,
        signed_transaction_norito=transaction,
        signed_transaction_sha256=str(value["signed_transaction_sha256"]),
    )


def request_authenticated_governance_transaction_v1(
    request: UntrustedGovernanceAuthorityRequestV1,
) -> NoReturn:
    """Refuse transport until the fixed native signer broker is provisioned."""

    _require_provisioned_privacy_governance_authority_v1()


def validate_authenticated_governance_receipt_v1(
    request: UntrustedGovernanceAuthorityRequestV1,
    receipt_payload: bytes,
) -> NoReturn:
    """Refuse structural acceptance until a pinned native verifier exists."""

    _require_provisioned_privacy_governance_authority_v1()
