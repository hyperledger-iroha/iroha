#!/usr/bin/env python3
"""Closed one-shot IPC codec for the non-networked Exact12 action driver.

The controller is the only network actor.  This module frames the public
context sent to the Rust action constructor and independently parses the
returned proof-bearing signed transaction bytes.  It never
accepts endpoints, credentials, private keys, witnesses, or outcome claims.
"""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Mapping
from types import MappingProxyType
from typing import NoReturn


REQUEST_SCHEMA = "iroha.taira.privacy_action_driver_request"
RESPONSE_SCHEMA = "iroha.taira.privacy_action_driver_response"
SCHEMA_VERSION = 1
VERANGE_OPERATION = "build-verange-action-v1"
VERANGE_PROTOCOL = "verange-transparent-range-v1"
CONSTRUCTIBLE_OPERATION_BY_PROTOCOL: Mapping[str, str] = MappingProxyType(
    {
        "zk-ace-pq-authorization-v0": "build-zk-ace-action-v1",
        "anonymous-pgc-k-out-of-n-v1": "build-anonymous-pgc-action-v1",
        VERANGE_PROTOCOL: VERANGE_OPERATION,
        "vega-existing-credential-zk-v0": "build-vega-action-v1",
        "iroha-jindo-polynomial-commitment-v0": "build-jindo-action-v1",
        "iroha-bootle-lantern-anoncred-v1": "build-bootle-lantern-action-v1",
        "orchard-halo2-actions-v1": "build-orchard-action-v1",
        "monero-fcmp-plus-plus-v1": "build-fcmp-action-v1",
        "iroha-ivm-private-note-stark-v1": "build-ivm-private-note-action-v1",
        "pq-masp-stark-v0": "build-pq-masp-action-v1",
    }
)
PROTOCOL_BY_CONSTRUCTIBLE_OPERATION: Mapping[str, str] = MappingProxyType(
    {
        operation: protocol
        for protocol, operation in CONSTRUCTIBLE_OPERATION_BY_PROTOCOL.items()
    }
)
QUALIFICATION_SCOPE = "native-action-construction-only"
CONSTRUCTION_ONLY_STATUS = "constructible"
JINDO_EXPERIMENTAL_STATUS = "available-experimental"
MISSING_CONTROLLER_CASE_EVIDENCE = "MissingSealedControllerProtocolCaseEvidence"
MISSING_ADMISSION_ARTIFACT_BUNDLE = "MissingCanonicalAdmissionArtifactBundle"
MISSING_JINDO_KNOWLEDGE_SOUNDNESS = (
    "MissingDistributionWideKnowledgeSoundnessEvidence"
)
MISSING_VERANGE_SETUP_AUTHORITY = (
    "MissingExactGenesisSourceClosedControllerSetupAuthorityIdentity"
)
MISSING_VERANGE_SETUP_TRANSACTION_BUNDLE = (
    "MissingNativePublicOnlyVeRangePolicyActivationTransactionBundle"
)
MISSING_VERANGE_STATE_QUERY_EVIDENCE = (
    "MissingFourPeerCanonicalVeRangeCapabilityRowStateQueriesBeforeAfterRestart"
)
VERANGE_CONTROLLER_BLOCKERS = (
    MISSING_VERANGE_SETUP_AUTHORITY,
    MISSING_VERANGE_SETUP_TRANSACTION_BUNDLE,
    MISSING_VERANGE_STATE_QUERY_EVIDENCE,
)
VERANGE_PUBLIC_ADMISSION_ARTIFACT_SCHEMA = (
    "iroha.taira.verange_public_admission_artifacts"
)
VERANGE_PUBLIC_ADMISSION_ARTIFACT_SCHEMA_VERSION = 1
VERANGE_SETUP_REQUIREMENTS_SCHEMA = (
    "iroha.taira.verange_qualification_setup_requirements"
)
VERANGE_SETUP_REQUIREMENTS_SCHEMA_VERSION = 1
VERANGE_QUALIFICATION_DOMAIN_ID = "privacy.universal"
VERANGE_ACTIVATION_HEIGHT_RULE = (
    "activate_at_height=proposed_at_height+minimum_delay_blocks"
)
VERANGE_ACTIVATION_INSTRUCTION = "register-privacy-protocol-activation-v1"
VERANGE_ACTIVATION_LIFECYCLE = "proposed-relative-height-template-v1"
VERANGE_GOVERNANCE_PERMISSION = "CanEnactGovernance"
VERANGE_ACTIVATION_MINIMUM_DELAY_BLOCKS = 300
VERANGE_ACTIVATION_TEMPLATE_PROPOSED_AT_HEIGHT = 1
VERANGE_ACTIVATION_TEMPLATE_ACTIVATE_AT_HEIGHT = (
    VERANGE_ACTIVATION_TEMPLATE_PROPOSED_AT_HEIGHT
    + VERANGE_ACTIVATION_MINIMUM_DELAY_BLOCKS
)
VERANGE_SETUP_IDENTITY_BINDING_DOMAIN = (
    b"iroha.taira.verange_qualification_setup_identity.v1\0"
)
PROTOCOLS_REQUIRING_UNPRESERVED_ADMISSION_ARTIFACTS = frozenset(
    protocol
    for protocol in CONSTRUCTIBLE_OPERATION_BY_PROTOCOL
    if protocol
    not in {VERANGE_PROTOCOL, "iroha-jindo-polynomial-commitment-v0"}
)
REQUEST_ID_DOMAIN = b"iroha.taira.privacy_action_driver_request.v1\0"
MAX_REQUEST_BYTES = 16 * 1024
MAX_TRANSACTION_BYTES = 9 * 1024 * 1024
MAX_COMPILED_PROFILE_BYTES = 64 * 1024
MAX_ACTIVATION_TEMPLATE_BYTES = 64 * 1024
MAX_RESPONSE_BYTES = (
    2 * MAX_TRANSACTION_BYTES
    + 2 * MAX_COMPILED_PROFILE_BYTES
    + 2 * MAX_ACTIVATION_TEMPLATE_BYTES
    + MAX_REQUEST_BYTES
)
MAX_TTL_MILLIS = 2 * 60 * 60 * 1000
MAX_CREATION_TIME_MILLIS = 2**63 - 1
MAX_ASSET_DEFINITION_ID_BYTES = 1024
SHA256_RE = re.compile(r"[0-9a-f]{64}")


class PrivacyActionDriverIpcError(RuntimeError):
    """The action-driver frame is noncanonical, unbounded, or substituted."""


def _fail(message: str) -> NoReturn:
    raise PrivacyActionDriverIpcError(message)


def _canonical(value: object) -> bytes:
    try:
        return (
            json.dumps(
                value,
                ensure_ascii=True,
                allow_nan=False,
                sort_keys=True,
                separators=(",", ":"),
            )
            + "\n"
        ).encode("ascii")
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise PrivacyActionDriverIpcError(
            f"action-driver frame is not canonically encodable: {exc}"
        ) from exc


def _object(payload: bytes, *, maximum: int, label: str) -> dict[str, object]:
    if not payload or len(payload) > maximum:
        _fail(f"{label} is empty or exceeds its {maximum}-byte bound")
    try:
        value = json.loads(payload, object_pairs_hook=_unique_object)
    except (UnicodeDecodeError, ValueError, json.JSONDecodeError) as exc:
        raise PrivacyActionDriverIpcError(f"{label} is not JSON") from exc
    if not isinstance(value, dict) or _canonical(value) != payload:
        _fail(f"{label} is not one canonical closed JSON object")
    return value


def _unique_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            raise ValueError(f"duplicate action-driver field {key!r}")
        value[key] = item
    return value


def _exact(value: Mapping[str, object], fields: set[str], label: str) -> None:
    if set(value) != fields:
        _fail(f"{label} fields are not exact")


def _sha256(value: object, label: str, *, nonzero: bool = False) -> str:
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        _fail(f"{label} must be one lowercase SHA-256 digest")
    if nonzero and value == "0" * 64:
        _fail(f"{label} must be nonzero")
    return value


def _network_id_hex(value: object) -> str:
    encoded = _sha256(value, "NetworkId", nonzero=True)
    if int(encoded[-2:], 16) & 1 == 0:
        _fail("NetworkId must carry the canonical Iroha hash marker bit")
    return encoded


def protocol_status(protocol: str) -> str:
    if protocol not in CONSTRUCTIBLE_OPERATION_BY_PROTOCOL:
        _fail("action-driver protocol is not constructible")
    return (
        JINDO_EXPERIMENTAL_STATUS
        if protocol == "iroha-jindo-polynomial-commitment-v0"
        else CONSTRUCTION_ONLY_STATUS
    )


def protocol_limitations(protocol: str) -> tuple[str, ...]:
    if protocol not in CONSTRUCTIBLE_OPERATION_BY_PROTOCOL:
        _fail("action-driver protocol is not constructible")
    limitations = [MISSING_CONTROLLER_CASE_EVIDENCE]
    if protocol == VERANGE_PROTOCOL:
        limitations.extend(VERANGE_CONTROLLER_BLOCKERS)
    elif protocol in PROTOCOLS_REQUIRING_UNPRESERVED_ADMISSION_ARTIFACTS:
        limitations.append(MISSING_ADMISSION_ARTIFACT_BUNDLE)
    if protocol == "iroha-jindo-polynomial-commitment-v0":
        limitations.append(MISSING_JINDO_KNOWLEDGE_SOUNDNESS)
    return tuple(limitations)


def _positive_integer(value: object, label: str, maximum: int) -> int:
    if (
        isinstance(value, bool)
        or not isinstance(value, int)
        or value <= 0
        or value > maximum
    ):
        _fail(f"{label} must be an integer in 1..={maximum}")
    return value


def _request_body(
    *,
    asset_definition_id: str,
    candidate_binding_sha256: str,
    creation_time_millis: int,
    network_id_hex: str,
    nonce: int,
    operation: str,
    ttl_millis: int,
) -> dict[str, object]:
    if (
        not isinstance(asset_definition_id, str)
        or not asset_definition_id
        or not asset_definition_id.isascii()
        or len(asset_definition_id) > MAX_ASSET_DEFINITION_ID_BYTES
    ):
        _fail("action-driver asset definition ID is not bounded ASCII")
    _sha256(candidate_binding_sha256, "candidate binding", nonzero=True)
    _network_id_hex(network_id_hex)
    _positive_integer(
        creation_time_millis, "creation time", MAX_CREATION_TIME_MILLIS
    )
    _positive_integer(nonce, "nonce", 2**32 - 1)
    _positive_integer(ttl_millis, "TTL", MAX_TTL_MILLIS)
    if operation not in PROTOCOL_BY_CONSTRUCTIBLE_OPERATION:
        _fail("action-driver request selects an unsupported contract")
    return {
        "asset_definition_id": asset_definition_id,
        "candidate_binding_sha256": candidate_binding_sha256,
        "creation_time_millis": creation_time_millis,
        "network_id_hex": network_id_hex,
        "nonce": nonce,
        "operation": operation,
        "schema": REQUEST_SCHEMA,
        "schema_version": SCHEMA_VERSION,
        "ttl_millis": ttl_millis,
    }


def build_action_request(
    *,
    asset_definition_id: str,
    candidate_binding_sha256: str,
    creation_time_millis: int,
    network_id_hex: str,
    nonce: int,
    protocol: str,
    ttl_millis: int,
) -> bytes:
    """Build one canonical retained-operation request without secret material."""

    if (
        not isinstance(protocol, str)
        or protocol not in CONSTRUCTIBLE_OPERATION_BY_PROTOCOL
    ):
        _fail("action-driver request selects an unsupported protocol")

    body = _request_body(
        asset_definition_id=asset_definition_id,
        candidate_binding_sha256=candidate_binding_sha256,
        creation_time_millis=creation_time_millis,
        network_id_hex=network_id_hex,
        nonce=nonce,
        operation=CONSTRUCTIBLE_OPERATION_BY_PROTOCOL[protocol],
        ttl_millis=ttl_millis,
    )
    digest = hashlib.sha256(REQUEST_ID_DOMAIN + _canonical(body)[:-1]).hexdigest()
    payload = _canonical({**body, "request_id": digest})
    if len(payload) > MAX_REQUEST_BYTES:
        _fail("action-driver request exceeds its canonical byte bound")
    return payload


def build_verange_request(
    *,
    asset_definition_id: str,
    candidate_binding_sha256: str,
    creation_time_millis: int,
    network_id_hex: str,
    nonce: int,
    ttl_millis: int,
) -> bytes:
    """Build the canonical VeRange request through the shared operation table."""

    return build_action_request(
        asset_definition_id=asset_definition_id,
        candidate_binding_sha256=candidate_binding_sha256,
        creation_time_millis=creation_time_millis,
        network_id_hex=network_id_hex,
        nonce=nonce,
        protocol=VERANGE_PROTOCOL,
        ttl_millis=ttl_millis,
    )


def validate_request(payload: bytes) -> dict[str, object]:
    """Parse and rederive one request before it reaches the native driver."""

    request = _object(payload, maximum=MAX_REQUEST_BYTES, label="action-driver request")
    _exact(
        request,
        {
            "asset_definition_id",
            "candidate_binding_sha256",
            "creation_time_millis",
            "network_id_hex",
            "nonce",
            "operation",
            "request_id",
            "schema",
            "schema_version",
            "ttl_millis",
        },
        "action-driver request",
    )
    if (
        request["operation"] not in PROTOCOL_BY_CONSTRUCTIBLE_OPERATION
        or request["schema"] != REQUEST_SCHEMA
        or request["schema_version"] != SCHEMA_VERSION
    ):
        _fail("action-driver request selects an unsupported contract")
    body = _request_body(
        asset_definition_id=request["asset_definition_id"],
        candidate_binding_sha256=request["candidate_binding_sha256"],
        creation_time_millis=request["creation_time_millis"],
        network_id_hex=request["network_id_hex"],
        nonce=request["nonce"],
        operation=request["operation"],
        ttl_millis=request["ttl_millis"],
    )
    expected_id = hashlib.sha256(
        REQUEST_ID_DOMAIN + _canonical(body)[:-1]
    ).hexdigest()
    if _sha256(request["request_id"], "request ID", nonzero=True) != expected_id:
        _fail("action-driver request ID is not derived from the canonical body")
    return request


def _decode_hex_bytes(value: object, label: str, maximum: int) -> bytes:
    if (
        not isinstance(value, str)
        or not value
        or len(value) % 2
        or len(value) > maximum * 2
        or any(byte not in "0123456789abcdef" for byte in value)
    ):
        _fail(f"{label} is not bounded canonical lowercase hexadecimal")
    decoded = bytes.fromhex(value)
    if not decoded or decoded.hex() != value:
        _fail(f"{label} is empty or noncanonical")
    return decoded


_VERANGE_PUBLIC_ADMISSION_ARTIFACT_FIELDS = {
    "action_authority_account_id",
    "action_authority_public_key_hex",
    "compiled_profile_norito_hex",
    "compiled_profile_sha256",
    "engine_id",
    "engine_manifest_digest_hex",
    "max_aggregation_count",
    "parameter_digest_hex",
    "parameter_id_hex",
    "policy_id_hex",
    "proof_system_id",
    "protocol_id",
    "schema",
    "schema_version",
    "setup_requirements",
    "setup_requirements_sha256",
    "statement_schema_digest_hex",
    "verifier_digest_hex",
}

_VERANGE_SETUP_REQUIREMENT_FIELDS = {
    "action_authority_account_id",
    "action_authority_public_key_hex",
    "activation_height_rule",
    "activation_instruction",
    "activation_lifecycle",
    "activation_minimum_delay_blocks",
    "activation_template_activate_at_height",
    "activation_template_norito_hex",
    "activation_template_proposed_at_height",
    "activation_template_sha256",
    "asset_definition_id",
    "candidate_binding_sha256",
    "compiled_profile_sha256",
    "domain_id",
    "governance_permission",
    "protocol_id",
    "schema",
    "schema_version",
    "setup_authority_account_id",
    "setup_authority_public_key_hex",
    "setup_identity_binding_sha256",
}


def _bounded_ascii(value: object, label: str) -> str:
    if (
        not isinstance(value, str)
        or not value
        or not value.isascii()
        or len(value.encode("ascii")) > MAX_ASSET_DEFINITION_ID_BYTES
    ):
        _fail(f"{label} is not bounded ASCII")
    return value


def _verange_setup_identity_binding(
    candidate_binding_sha256: str,
    setup_account_id: str,
    setup_public_key: bytes,
) -> str:
    hasher = hashlib.sha256()
    hasher.update(VERANGE_SETUP_IDENTITY_BINDING_DOMAIN)
    hasher.update(bytes.fromhex(candidate_binding_sha256))
    encoded_account = setup_account_id.encode("ascii")
    hasher.update(len(encoded_account).to_bytes(8, "little"))
    hasher.update(encoded_account)
    hasher.update(setup_public_key)
    return hasher.hexdigest()


def _verange_setup_requirements(
    value: object,
    *,
    expected_request: Mapping[str, object],
    action_account_id: str,
    action_public_key_hex: str,
    compiled_profile_sha256: str,
) -> dict[str, object]:
    if not isinstance(value, dict):
        _fail("VeRange setup requirements must be one typed object")
    _exact(value, _VERANGE_SETUP_REQUIREMENT_FIELDS, "VeRange setup requirements")
    candidate = _sha256(
        value["candidate_binding_sha256"], "VeRange setup candidate", nonzero=True
    )
    if (
        candidate != expected_request["candidate_binding_sha256"]
        or value["asset_definition_id"] != expected_request["asset_definition_id"]
        or value["action_authority_account_id"] != action_account_id
        or value["action_authority_public_key_hex"] != action_public_key_hex
        or value["compiled_profile_sha256"] != compiled_profile_sha256
        or value["activation_height_rule"] != VERANGE_ACTIVATION_HEIGHT_RULE
        or value["activation_instruction"] != VERANGE_ACTIVATION_INSTRUCTION
        or value["activation_lifecycle"] != VERANGE_ACTIVATION_LIFECYCLE
        or value["activation_minimum_delay_blocks"]
        != VERANGE_ACTIVATION_MINIMUM_DELAY_BLOCKS
        or value["activation_template_proposed_at_height"]
        != VERANGE_ACTIVATION_TEMPLATE_PROPOSED_AT_HEIGHT
        or value["activation_template_activate_at_height"]
        != VERANGE_ACTIVATION_TEMPLATE_ACTIVATE_AT_HEIGHT
        or value["domain_id"] != VERANGE_QUALIFICATION_DOMAIN_ID
        or value["governance_permission"] != VERANGE_GOVERNANCE_PERMISSION
        or value["protocol_id"] != VERANGE_PROTOCOL
        or value["schema"] != VERANGE_SETUP_REQUIREMENTS_SCHEMA
        or value["schema_version"] != VERANGE_SETUP_REQUIREMENTS_SCHEMA_VERSION
    ):
        _fail("VeRange setup requirements differ from the exact public contract")
    setup_account_id = _bounded_ascii(
        value["setup_authority_account_id"],
        "VeRange qualification setup authority account ID",
    )
    setup_public_key = _decode_hex_bytes(
        value["setup_authority_public_key_hex"],
        "VeRange qualification setup authority key",
        32,
    )
    if len(setup_public_key) != 32 or setup_public_key == bytes(32):
        _fail("VeRange qualification setup authority key is not nonzero Ed25519")
    expected_identity_binding = _verange_setup_identity_binding(
        candidate, setup_account_id, setup_public_key
    )
    if _sha256(
        value["setup_identity_binding_sha256"],
        "VeRange setup identity binding",
        nonzero=True,
    ) != expected_identity_binding:
        _fail("VeRange setup identity is not bound to the exact candidate")
    activation_template = _decode_hex_bytes(
        value["activation_template_norito_hex"],
        "VeRange activation template",
        MAX_ACTIVATION_TEMPLATE_BYTES,
    )
    activation_template_sha256 = _sha256(
        value["activation_template_sha256"],
        "VeRange activation template digest",
        nonzero=True,
    )
    if hashlib.sha256(activation_template).hexdigest() != activation_template_sha256:
        _fail("VeRange activation template digest differs from its canonical bytes")
    return {
        **value,
        "activation_template_norito": activation_template,
        "setup_authority_public_key_hex": setup_public_key.hex(),
    }


def _verange_public_admission_artifacts(
    value: object,
    *,
    expected_request: Mapping[str, object],
) -> dict[str, object]:
    """Validate the driver's bounded public-only VeRange setup material."""

    if not isinstance(value, dict):
        _fail("VeRange public admission artifacts must be one typed object")
    _exact(
        value,
        _VERANGE_PUBLIC_ADMISSION_ARTIFACT_FIELDS,
        "VeRange public admission artifacts",
    )
    account_id = _bounded_ascii(
        value["action_authority_account_id"],
        "VeRange public action authority account ID",
    )
    public_key = _decode_hex_bytes(
        value["action_authority_public_key_hex"],
        "VeRange public action authority key",
        32,
    )
    if len(public_key) != 32 or public_key == bytes(32):
        _fail("VeRange public action authority key must be one nonzero Ed25519 key")
    compiled_profile = _decode_hex_bytes(
        value["compiled_profile_norito_hex"],
        "VeRange compiled profile",
        MAX_COMPILED_PROFILE_BYTES,
    )
    compiled_profile_sha256 = _sha256(
        value["compiled_profile_sha256"],
        "VeRange compiled profile digest",
        nonzero=True,
    )
    if hashlib.sha256(compiled_profile).hexdigest() != compiled_profile_sha256:
        _fail("VeRange compiled profile digest differs from its canonical bytes")
    fixed_bindings = {
        field: _sha256(value[field], f"VeRange {field}", nonzero=True)
        for field in (
            "engine_manifest_digest_hex",
            "parameter_digest_hex",
            "parameter_id_hex",
            "policy_id_hex",
            "statement_schema_digest_hex",
            "verifier_digest_hex",
        )
    }
    if (
        value["engine_id"] != "native-verange-p256"
        or value["max_aggregation_count"] != 8
        or value["proof_system_id"] != "iroha-verange-p256"
        or value["protocol_id"] != VERANGE_PROTOCOL
        or value["schema"] != VERANGE_PUBLIC_ADMISSION_ARTIFACT_SCHEMA
        or value["schema_version"]
        != VERANGE_PUBLIC_ADMISSION_ARTIFACT_SCHEMA_VERSION
    ):
        _fail("VeRange public admission artifacts select a different native profile")
    setup_requirements_sha256 = _sha256(
        value["setup_requirements_sha256"],
        "VeRange setup requirements digest",
        nonzero=True,
    )
    if not isinstance(value["setup_requirements"], dict) or hashlib.sha256(
        _canonical(value["setup_requirements"])[:-1]
    ).hexdigest() != setup_requirements_sha256:
        _fail("VeRange setup requirements digest differs from its canonical object")
    setup_requirements = _verange_setup_requirements(
        value["setup_requirements"],
        expected_request=expected_request,
        action_account_id=account_id,
        action_public_key_hex=public_key.hex(),
        compiled_profile_sha256=compiled_profile_sha256,
    )
    return {
        "action_authority_account_id": account_id,
        "action_authority_public_key_hex": public_key.hex(),
        "compiled_profile_norito": compiled_profile,
        "compiled_profile_sha256": compiled_profile_sha256,
        "engine_id": "native-verange-p256",
        **fixed_bindings,
        "max_aggregation_count": 8,
        "proof_system_id": "iroha-verange-p256",
        "protocol_id": VERANGE_PROTOCOL,
        "schema": VERANGE_PUBLIC_ADMISSION_ARTIFACT_SCHEMA,
        "schema_version": VERANGE_PUBLIC_ADMISSION_ARTIFACT_SCHEMA_VERSION,
        "setup_requirements": setup_requirements,
        "setup_requirements_sha256": setup_requirements_sha256,
    }


def validate_response(
    payload: bytes,
    *,
    expected_request: Mapping[str, object],
) -> dict[str, object]:
    """Parse the native response and independently hash its action bytes."""

    expected = validate_request(_canonical(dict(expected_request)))
    expected_operation = str(expected["operation"])
    expected_protocol = PROTOCOL_BY_CONSTRUCTIBLE_OPERATION[expected_operation]

    response = _object(
        payload, maximum=MAX_RESPONSE_BYTES, label="action-driver response"
    )
    _exact(
        response,
        {
            "availability",
            "candidate_binding_sha256",
            "limitations",
            "network_outcome_authoritative",
            "operation",
            "protocol",
            "public_admission_artifacts",
            "qualification_scope",
            "request_id",
            "schema",
            "schema_version",
            "transaction_hash_hex",
            "transaction_norito_hex",
            "transaction_sha256",
        },
        "action-driver response",
    )
    if (
        response["schema"] != RESPONSE_SCHEMA
        or response["schema_version"] != SCHEMA_VERSION
        or response["operation"] != expected_operation
        or response["protocol"] != expected_protocol
        or response["availability"] != protocol_status(expected_protocol)
        or response["limitations"] != list(protocol_limitations(expected_protocol))
        or response["network_outcome_authoritative"] is not False
        or response["qualification_scope"] != QUALIFICATION_SCOPE
        or response["request_id"] != expected["request_id"]
        or response["candidate_binding_sha256"]
        != expected["candidate_binding_sha256"]
    ):
        _fail("action-driver response context differs from its exact request")
    transaction_hash = _sha256(
        response["transaction_hash_hex"], "Iroha transaction hash", nonzero=True
    )
    transaction = _decode_hex_bytes(
        response["transaction_norito_hex"],
        "proof-bearing transaction",
        MAX_TRANSACTION_BYTES,
    )
    if hashlib.sha256(transaction).hexdigest() != _sha256(
        response["transaction_sha256"], "transaction digest", nonzero=True
    ):
        _fail("action-driver transaction digest differs from its bytes")
    if expected_protocol == VERANGE_PROTOCOL:
        public_admission_artifacts = _verange_public_admission_artifacts(
            response["public_admission_artifacts"], expected_request=expected
        )
    else:
        if response["public_admission_artifacts"] is not None:
            _fail("non-VeRange action returned unrelated public admission artifacts")
        public_admission_artifacts = None
    return {
        "availability": response["availability"],
        "limitations": tuple(response["limitations"]),
        "network_outcome_authoritative": False,
        "operation": expected_operation,
        "protocol": expected_protocol,
        "public_admission_artifacts": public_admission_artifacts,
        "qualification_scope": QUALIFICATION_SCOPE,
        "request_id": response["request_id"],
        "transaction_hash_hex": transaction_hash,
        "transaction_norito": transaction,
        "transaction_sha256": response["transaction_sha256"],
    }
