from __future__ import annotations

import ast
import base64
import builtins
import hashlib
import json
from pathlib import Path
from typing import Callable

import pytest

from scripts import taira_privacy_action_driver_ipc as action_driver_ipc
from scripts import taira_privacy_governance_authority as authority
from scripts import taira_privacy_sealed_controller as sealed_controller
from scripts import taira_rollout_admission as rollout_admission
from scripts import seal_taira_release_controllers as controller_seal


ROOT = Path(__file__).resolve().parents[2]
RUST_GOVERNANCE_REQUEST_FIXTURE = (
    ROOT / "scripts/tests/fixtures/taira_privacy_governance_request_v1.json"
)
HOSTILE_RUNTIME_UID = 501
AUTHENTICATED_CLIENT_UID = 73
UNMARKED_IROHA_HASH = "1" * 62 + "10"


def _digest(label: str) -> str:
    return hashlib.sha256(label.encode("ascii")).hexdigest()


def _iroha_hash(label: str) -> str:
    value = bytearray(hashlib.sha256(label.encode("ascii")).digest())
    value[-1] |= 1
    return value.hex()


def _request_arguments() -> dict[str, object]:
    instruction = b"canonical-native-verange-activation-v1"
    transaction_payload = b"canonical-unsigned-TransactionPayload-norito-v1"
    genesis_hash = _iroha_hash("genesis expected hash")
    return {
        "protocol": "verange-transparent-range-v1",
        "activation_instruction_norito": instruction,
        "activation_instruction_sha256": hashlib.sha256(instruction).hexdigest(),
        "compiled_profile_sha256": _digest("compiled profile"),
        "proposed_at_height": 602,
        "activate_at_height": 902,
        "transaction_payload_norito": transaction_payload,
        "transaction_payload_sha256": hashlib.sha256(
            transaction_payload
        ).hexdigest(),
        "transaction_payload_hash_hex": _iroha_hash(
            "native typed TransactionPayload prehash"
        ),
        "transaction_creation_time_millis": 1_900_000_000_000,
        "transaction_ttl_millis": 300_000,
        "transaction_nonce": 29,
        "candidate_binding_sha256": _digest("candidate"),
        "source_commit": "1" * 40,
        "dpn_validator_release_commit": "2" * 40,
        "cargo_lock_sha256": _digest("Cargo.lock"),
        "workspace_source_manifest_sha256": _digest("workspace"),
        "reset_manifest_sha256": _digest("reset manifest"),
        "signed_genesis_sha256": _digest("signed genesis"),
        "unsigned_genesis_sha256": _digest("unsigned genesis"),
        "genesis_expected_hash": genesis_hash,
        "genesis_public_key": "ed0120" + "AB" * 32,
        "genesis_authority_account_id": "taira_genesis_authority@genesis",
        "network_id_hex": genesis_hash,
        "controller_digest": _digest("sealed controller"),
        "controller_host_id": "taira-controller-host-01",
        "controller_installation_id": "controller-installation-01",
        "four_peer_binding_sha256": _digest("four peer fleet"),
        "supervisor_binding_sha256": _digest("four supervisors"),
        "run_nonce": _digest("run nonce"),
        "issued_at_unix_millis": 1_900_000_000_000,
        "expires_at_unix_millis": 1_900_000_300_000,
    }


def _request(
    mutate: Callable[[dict[str, object]], None] | None = None,
) -> authority.UntrustedGovernanceAuthorityRequestV1:
    arguments = _request_arguments()
    if mutate is not None:
        mutate(arguments)
    return authority._build_untrusted_governance_authority_request_v1(
        **arguments  # type: ignore[arg-type]
    )


def _request_from_rust_fixture() -> tuple[
    authority.UntrustedGovernanceAuthorityRequestV1, bytes, dict[str, object]
]:
    fixture_bytes = RUST_GOVERNANCE_REQUEST_FIXTURE.read_bytes()
    value = json.loads(fixture_bytes)
    assert isinstance(value, dict)
    activation = value["activation"]
    candidate = value["candidate"]
    controller = value["controller"]
    fleet = value["fleet"]
    genesis = value["genesis"]
    run = value["run"]
    transaction = value["transaction"]
    assert isinstance(activation, dict)
    assert isinstance(candidate, dict)
    assert isinstance(controller, dict)
    assert isinstance(fleet, dict)
    assert isinstance(genesis, dict)
    assert isinstance(run, dict)
    assert isinstance(transaction, dict)
    request = authority._build_untrusted_governance_authority_request_v1(
        protocol=activation["protocol"],
        activation_instruction_norito=base64.b64decode(
            activation["instruction_norito_base64"], validate=True
        ),
        activation_instruction_sha256=activation["instruction_sha256"],
        compiled_profile_sha256=activation["compiled_profile_sha256"],
        proposed_at_height=activation["proposed_at_height"],
        activate_at_height=activation["activate_at_height"],
        transaction_payload_norito=base64.b64decode(
            transaction["payload_norito_base64"], validate=True
        ),
        transaction_payload_sha256=transaction["payload_sha256"],
        transaction_payload_hash_hex=transaction["payload_hash_hex"],
        transaction_creation_time_millis=transaction["creation_time_millis"],
        transaction_ttl_millis=transaction["time_to_live_millis"],
        transaction_nonce=transaction["nonce"],
        candidate_binding_sha256=candidate["candidate_binding_sha256"],
        source_commit=candidate["source_commit"],
        dpn_validator_release_commit=candidate["dpn_validator_release_commit"],
        cargo_lock_sha256=candidate["cargo_lock_sha256"],
        workspace_source_manifest_sha256=candidate[
            "workspace_source_manifest_sha256"
        ],
        reset_manifest_sha256=genesis["reset_manifest_sha256"],
        signed_genesis_sha256=genesis["signed_genesis_sha256"],
        unsigned_genesis_sha256=genesis["unsigned_genesis_sha256"],
        genesis_expected_hash=genesis["expected_hash"],
        genesis_public_key=genesis["public_key"],
        genesis_authority_account_id=genesis["authority_account_id"],
        network_id_hex=genesis["network_id_hex"],
        controller_digest=controller["digest"],
        controller_host_id=controller["host_id"],
        controller_installation_id=controller["installation_id"],
        four_peer_binding_sha256=fleet["four_peer_binding_sha256"],
        supervisor_binding_sha256=fleet["supervisor_binding_sha256"],
        run_nonce=run["nonce"],
        issued_at_unix_millis=run["issued_at_unix_millis"],
        expires_at_unix_millis=run["expires_at_unix_millis"],
    )
    return request, fixture_bytes, value


def _rebound_request(
    mutate: Callable[[dict[str, object]], None],
) -> authority.UntrustedGovernanceAuthorityRequestV1:
    value = json.loads(_request().canonical_bytes)
    mutate(value)
    body = dict(value)
    body.pop("request_id")
    value["request_id"] = hashlib.sha256(
        authority.REQUEST_ID_DOMAIN + authority._canonical(body)
    ).hexdigest()
    payload = authority._canonical(value)
    return authority.UntrustedGovernanceAuthorityRequestV1(
        canonical_bytes=payload,
        request_id=value["request_id"],
        request_sha256=hashlib.sha256(payload).hexdigest(),
    )


def _receipt_value(
    request: authority.UntrustedGovernanceAuthorityRequestV1,
) -> dict[str, object]:
    transaction = b"canonical-signed-iroha-transaction-v1"
    attestation = b"native-broker-response-attestation-v1"
    return {
        "administrator_uid": 72,
        "audit_committed_head_sha256": _digest("committed audit head"),
        "audit_live_head_sha256": _digest("committed audit head"),
        "audit_previous_head_sha256": _digest("previous audit head"),
        "audit_sequence": 41,
        "authority_envelope_schema": authority.AUTHORITY_ENVELOPE_SCHEMA,
        "authority_account_id": "taira_genesis_authority@genesis",
        "authority_public_key": "ed0120" + "AB" * 32,
        "binding_schema": authority.BINDING_SCHEMA,
        "binding_sha256": _digest("installed binding"),
        "broker_binary_sha256": _digest("native broker binary"),
        "kernel_peer_uid": AUTHENTICATED_CLIENT_UID,
        "key_revision": 3,
        "operation_id": _digest("operation"),
        "policy_revision": 7,
        "policy_sha256": _digest("native semantic policy"),
        "replay_namespace": authority.REPLAY_NAMESPACE,
        "request_id": request.request_id,
        "request_sha256": request.request_sha256,
        "response_attestation_base64": base64.b64encode(attestation).decode("ascii"),
        "response_attestation_sha256": hashlib.sha256(attestation).hexdigest(),
        "schema": authority.RECEIPT_SCHEMA,
        "schema_version": authority.SCHEMA_VERSION,
        "service_id": authority.SERVICE_ID,
        "service_uid": 71,
        "signer_role": authority.ROLE,
        "signed_transaction_norito_base64": base64.b64encode(transaction).decode(
            "ascii"
        ),
        "signed_transaction_sha256": hashlib.sha256(transaction).hexdigest(),
        "status": "signed",
        "transaction_hash_hex": _iroha_hash("Iroha transaction hash"),
    }


def _receipt_bytes(value: dict[str, object]) -> bytes:
    return authority._canonical(value)


def test_untrusted_request_is_canonical_and_binds_shared_source_closed_contract() -> None:
    request = _request()
    value = json.loads(request.canonical_bytes)
    body = dict(value)
    body.pop("request_id")

    assert authority._canonical(value) == request.canonical_bytes
    assert request.request_id == hashlib.sha256(
        authority.REQUEST_ID_DOMAIN + authority._canonical(body)
    ).hexdigest()
    assert request.request_sha256 == hashlib.sha256(
        request.canonical_bytes
    ).hexdigest()
    assert value["authority_envelope_schema"] == authority.AUTHORITY_ENVELOPE_SCHEMA
    assert value["run"]["replay_namespace"] == authority.REPLAY_NAMESPACE
    assert set(value["controller"]) == {"digest", "host_id", "installation_id"}
    assert value["genesis"]["network_id_hex"] == value["genesis"]["expected_hash"]
    transaction = value["transaction"]
    assert transaction["network_id_hex"] == value["genesis"]["expected_hash"]
    assert transaction["authority_account_id"] == value["genesis"][
        "authority_account_id"
    ]
    assert transaction["creation_time_millis"] == value["run"][
        "issued_at_unix_millis"
    ]
    assert transaction["time_to_live_millis"] == (
        value["run"]["expires_at_unix_millis"]
        - value["run"]["issued_at_unix_millis"]
    )
    assert transaction["fee_payment"] == {
        "charge_limits": [],
        "gas_limit": None,
        "payer": "authority",
    }
    assert transaction["metadata"] == {}
    assert transaction["attachments"] is None
    payload = base64.b64decode(transaction["payload_norito_base64"], validate=True)
    assert hashlib.sha256(payload).hexdigest() == transaction["payload_sha256"]
    request_text = request.canonical_bytes.decode("ascii")
    assert "client_uid" not in request_text
    assert "kernel_peer_uid" not in request_text
    assert str(authority.FIXED_BINDING_PATH).encode() not in request.canonical_bytes
    assert str(authority.FIXED_REQUEST_SOCKET).encode() not in request.canonical_bytes
    for forbidden in (b"credential", b"private_key", b"torii_endpoint"):
        assert forbidden not in request.canonical_bytes


def test_python_builder_and_validator_match_real_rust_transaction_fixture() -> None:
    request, fixture_bytes, fixture_value = _request_from_rust_fixture()

    assert request.canonical_bytes == fixture_bytes
    assert request.request_id == fixture_value["request_id"]
    assert request.request_sha256 == hashlib.sha256(fixture_bytes).hexdigest()
    assert authority._validated_untrusted_request_value_v1(request) == fixture_value

    activation = fixture_value["activation"]
    transaction = fixture_value["transaction"]
    assert isinstance(activation, dict)
    assert isinstance(transaction, dict)
    activation_bytes = base64.b64decode(
        activation["instruction_norito_base64"], validate=True
    )
    payload_bytes = base64.b64decode(
        transaction["payload_norito_base64"], validate=True
    )
    assert hashlib.sha256(activation_bytes).hexdigest() == activation[
        "instruction_sha256"
    ]
    assert hashlib.sha256(payload_bytes).hexdigest() == transaction["payload_sha256"]
    assert transaction["instruction_norito_sha256"] == activation[
        "instruction_sha256"
    ]


def _splice_transaction_payload(arguments: dict[str, object]) -> None:
    payload = b"attacker-recomputed-TransactionPayload"
    arguments["transaction_payload_norito"] = payload
    arguments["transaction_payload_sha256"] = hashlib.sha256(payload).hexdigest()
    arguments["transaction_payload_hash_hex"] = _iroha_hash(
        "attacker-recomputed-native-prehash"
    )


def _splice_activation(arguments: dict[str, object]) -> None:
    instruction = b"attacker-recomputed-activation"
    arguments["activation_instruction_norito"] = instruction
    arguments["activation_instruction_sha256"] = hashlib.sha256(
        instruction
    ).hexdigest()


def _reuse_candidate_signer(arguments: dict[str, object]) -> None:
    arguments["genesis_authority_account_id"] = "candidate_signer@candidate"
    arguments["genesis_public_key"] = "ed0120" + "CD" * 32


def _splice_reset_network(arguments: dict[str, object]) -> None:
    forged_genesis = _iroha_hash("attacker-spliced-genesis-header")
    arguments["genesis_expected_hash"] = forged_genesis
    arguments["network_id_hex"] = forged_genesis
    arguments["signed_genesis_sha256"] = _digest("attacker-signed-genesis")
    arguments["unsigned_genesis_sha256"] = _digest("attacker-unsigned-genesis")


def _stale_run(arguments: dict[str, object]) -> None:
    arguments["issued_at_unix_millis"] = 1_000
    arguments["expires_at_unix_millis"] = 301_000
    arguments["transaction_creation_time_millis"] = 1_000


@pytest.mark.parametrize(
    "mutation",
    (
        lambda row: row.update(candidate_binding_sha256=_digest("spliced candidate")),
        lambda row: row.update(source_commit="3" * 40),
        lambda row: row.update(signed_genesis_sha256=_digest("spliced genesis")),
        lambda row: row.update(controller_host_id="spliced-controller-host"),
        lambda row: row.update(four_peer_binding_sha256=_digest("spliced fleet")),
        lambda row: row.update(run_nonce=_digest("replayed nonce")),
        _splice_transaction_payload,
        _splice_activation,
        _reuse_candidate_signer,
        _splice_reset_network,
    ),
)
def test_candidate_source_genesis_controller_fleet_and_run_splices_change_request_id(
    mutation: Callable[[dict[str, object]], None],
) -> None:
    assert _request(mutation).request_id != _request().request_id


def test_network_id_must_be_the_exact_reset_genesis_hash() -> None:
    with pytest.raises(
        authority.PrivacyGovernanceAuthorityError,
        match="NetworkId must equal the exact reset genesis header hash",
    ):
        _request(
            lambda row: row.update(network_id_hex=_iroha_hash("foreign NetworkId"))
        )


@pytest.mark.parametrize(
    "mutation",
    (
        lambda row: row.update(
            genesis_expected_hash=UNMARKED_IROHA_HASH,
            network_id_hex=UNMARKED_IROHA_HASH,
        ),
        lambda row: row.update(network_id_hex=UNMARKED_IROHA_HASH),
        lambda row: row.update(transaction_payload_hash_hex=UNMARKED_IROHA_HASH),
    ),
    ids=("genesis", "network-id", "typed-payload-prehash"),
)
def test_native_iroha_hashes_require_the_canonical_marker_bit(
    mutation: Callable[[dict[str, object]], None],
) -> None:
    with pytest.raises(
        authority.PrivacyGovernanceAuthorityError,
        match="canonical Iroha marker bit",
    ):
        _request(mutation)


@pytest.mark.parametrize(
    ("field", "forged"),
    (
        ("proposed_at_height", True),
        ("proposed_at_height", 602.0),
        ("proposed_at_height", authority.MAX_U64 + 1),
        ("activate_at_height", True),
        ("activate_at_height", 902.0),
        ("activate_at_height", authority.MAX_U64 + 1),
    ),
)
def test_activation_heights_are_exact_bounded_u64(
    field: str, forged: object
) -> None:
    with pytest.raises(authority.PrivacyGovernanceAuthorityError):
        _request(lambda row: row.update({field: forged}))


def test_activation_delay_addition_rejects_u64_overflow() -> None:
    with pytest.raises(
        authority.PrivacyGovernanceAuthorityError,
        match="minimum governance delay",
    ):
        _request(
            lambda row: row.update(
                proposed_at_height=authority.MAX_U64 - 299,
                activate_at_height=authority.MAX_U64,
            )
        )


@pytest.mark.parametrize(
    ("field", "forged"),
    (
        ("transaction_creation_time_millis", 1_900_000_000_001),
        ("transaction_creation_time_millis", 1_900_000_000_000.0),
        ("transaction_ttl_millis", 299_999),
        ("transaction_ttl_millis", 300_000.0),
        ("transaction_ttl_millis", True),
        ("transaction_nonce", 0),
        ("transaction_nonce", 2**32),
    ),
)
def test_transaction_time_ttl_and_nonce_axes_are_closed(
    field: str, forged: object
) -> None:
    with pytest.raises(authority.PrivacyGovernanceAuthorityError):
        _request(lambda row: row.update({field: forged}))


@pytest.mark.parametrize("forged", (True, 1.0))
def test_recomputed_request_rejects_non_integer_schema_version(forged: object) -> None:
    request = _rebound_request(lambda value: value.update(schema_version=forged))
    with pytest.raises(
        authority.PrivacyGovernanceAuthorityError,
        match="request schema version must be one positive integer",
    ):
        authority._validated_untrusted_request_value_v1(request)


@pytest.mark.parametrize("forged", (True, 300_000.0))
def test_recomputed_request_rejects_non_integer_transaction_ttl(
    forged: object,
) -> None:
    def mutate(value: dict[str, object]) -> None:
        transaction = value["transaction"]
        assert isinstance(transaction, dict)
        transaction["time_to_live_millis"] = forged

    request = _rebound_request(mutate)
    with pytest.raises(
        authority.PrivacyGovernanceAuthorityError,
        match="request transaction TTL must be one positive integer",
    ):
        authority._validated_untrusted_request_value_v1(request)


@pytest.mark.parametrize("forged", (True, 602.0, authority.MAX_U64 + 1))
def test_recomputed_request_rejects_non_u64_activation_height(forged: object) -> None:
    def mutate(value: dict[str, object]) -> None:
        activation = value["activation"]
        assert isinstance(activation, dict)
        activation["proposed_at_height"] = forged

    request = _rebound_request(mutate)
    with pytest.raises(authority.PrivacyGovernanceAuthorityError):
        authority._validated_untrusted_request_value_v1(request)


def test_structural_receipt_is_accepted_only_after_native_historical_verification(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    request = _request()
    payload = _receipt_bytes(_receipt_value(request))
    structural = authority._validate_untrusted_governance_authority_receipt_structure_v1(
        request,
        payload,
        authenticated_client_uid=AUTHENTICATED_CLIENT_UID,
    )
    assert structural.canonical_bytes == payload
    assert structural.request_id == request.request_id
    assert structural.signed_transaction_norito
    assert structural.signed_transaction_sha256 == hashlib.sha256(
        structural.signed_transaction_norito
    ).hexdigest()

    calls: list[str] = []
    def preflight(role: str, *, require_signing: bool = True) -> dict[str, object]:
        assert require_signing is False
        calls.append(f"preflight:{role}")
        return {"client_uid": AUTHENTICATED_CLIENT_UID}

    monkeypatch.setattr(authority.taira_authority_client, "preflight", preflight)
    monkeypatch.setattr(
        authority.taira_authority_client,
        "verify_receipt",
        lambda role, *_args, **_kwargs: calls.append(f"verify:{role}"),
    )
    verified = authority.validate_authenticated_governance_receipt_v1(
        request,
        payload,
        authority_envelope_payload=authority.taira_authority_client.canonical_json_bytes(
            {"schema": "test-governance-envelope"}
        ),
    )
    assert verified == structural
    assert calls == [
        "preflight:privacy-governance",
        "verify:privacy-governance",
    ]


def test_governance_request_authorizes_exact_validated_subject(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """The request path returns the structural view of the native receipt."""

    request = _request()
    receipt = _receipt_value(request)
    calls: list[str] = []
    result = authority.taira_authority_client.AuthorityResult(
        role="privacy-governance",
        operation_id=_digest("operation"),
        run_id=_digest("run"),
        status="authorized",
        authority_envelope={"schema": "test-envelope"},
        durable_receipt=receipt,
    )
    def preflight(role: str, *, require_signing: bool = True) -> dict[str, object]:
        assert require_signing is True
        calls.append(f"preflight:{role}")
        return {"client_uid": AUTHENTICATED_CLIENT_UID}

    monkeypatch.setattr(authority.taira_authority_client, "preflight", preflight)
    monkeypatch.setattr(
        authority.taira_authority_client,
        "authorize",
        lambda role, _subject: calls.append(f"authorize:{role}") or result,
    )

    validated = authority.request_authenticated_governance_transaction_v1(request)
    assert validated.receipt.canonical_bytes == _receipt_bytes(receipt)
    assert validated.authority_envelope == (
        authority.taira_authority_client.canonical_json_bytes(
            {"schema": "test-envelope"}
        )
    )
    assert validated.durable_receipt == _receipt_bytes(receipt)
    assert calls == [
        "preflight:privacy-governance",
        "authorize:privacy-governance",
    ]


@pytest.mark.parametrize(
    ("field", "forged"),
    (
        ("signed_transaction_norito_base64", 7),
        ("signed_transaction_norito_base64", ["YQ=="]),
        ("signed_transaction_norito_base64", {"encoded": "YQ=="}),
        ("response_attestation_base64", 7),
        ("response_attestation_base64", ["YQ=="]),
        ("response_attestation_base64", {"encoded": "YQ=="}),
    ),
)
def test_receipt_base64_fields_reject_non_string_json_without_coercion(
    field: str, forged: object
) -> None:
    request = _request()
    value = _receipt_value(request)
    value[field] = forged
    with pytest.raises(
        authority.PrivacyGovernanceAuthorityError,
        match="bounded nonempty ASCII",
    ):
        authority._validate_untrusted_governance_authority_receipt_structure_v1(
            request,
            _receipt_bytes(value),
            authenticated_client_uid=AUTHENTICATED_CLIENT_UID,
        )


@pytest.mark.parametrize(
    ("field", "limit_name"),
    (
        ("signed_transaction_norito_base64", "MAX_SIGNED_TRANSACTION_BYTES"),
        ("response_attestation_base64", "MAX_RESPONSE_ATTESTATION_BYTES"),
    ),
)
def test_receipt_decoded_byte_bounds_are_enforced_after_valid_base64(
    monkeypatch: pytest.MonkeyPatch, field: str, limit_name: str
) -> None:
    request = _request()
    value = _receipt_value(request)
    oversized = b"12345"
    monkeypatch.setattr(authority, limit_name, 4)
    value[field] = base64.b64encode(oversized).decode("ascii")
    digest_field = (
        "signed_transaction_sha256"
        if field == "signed_transaction_norito_base64"
        else "response_attestation_sha256"
    )
    value[digest_field] = hashlib.sha256(oversized).hexdigest()
    with pytest.raises(
        authority.PrivacyGovernanceAuthorityError,
        match="bounded canonical base64",
    ):
        authority._validate_untrusted_governance_authority_receipt_structure_v1(
            request,
            _receipt_bytes(value),
            authenticated_client_uid=AUTHENTICATED_CLIENT_UID,
        )


@pytest.mark.parametrize(
    ("field", "forged", "message"),
    (
        (
            "kernel_peer_uid",
            HOSTILE_RUNTIME_UID,
            "authenticated binding client UID",
        ),
        ("kernel_peer_uid", True, "nonnegative integer"),
        ("schema_version", True, "receipt schema version must be one positive integer"),
        ("schema_version", 1.0, "receipt schema version must be one positive integer"),
        ("service_uid", 0, "positive integer"),
        ("administrator_uid", 0, "positive integer"),
        ("service_uid", authority.MAX_UID_U32 + 1, "exceeds its maximum"),
        ("administrator_uid", authority.MAX_UID_U32 + 1, "exceeds its maximum"),
        ("kernel_peer_uid", authority.MAX_UID_U32 + 1, "exceeds its maximum"),
        ("service_uid", 72, "service and administrator UIDs must be distinct"),
        ("audit_sequence", authority.MAX_U64 + 1, "exceeds its maximum"),
        ("key_revision", True, "positive integer"),
        ("policy_revision", 7.0, "positive integer"),
        ("service_id", "taira-release-candidate-signer", "service or signer role"),
        ("signer_role", "candidate-release-signing", "service or signer role"),
        (
            "authority_envelope_schema",
            authority.LEGACY_VERANGE_AUTHORITY_ENVELOPE_SCHEMA,
            "contract or request binding",
        ),
        ("binding_schema", "candidate.binding.v1", "contract or request binding"),
        (
            "replay_namespace",
            authority.LEGACY_VERANGE_REPLAY_NAMESPACE,
            "contract or request binding",
        ),
    ),
)
def test_runtime_uid_signer_role_reuse_and_legacy_contracts_are_rejected(
    field: str, forged: object, message: str
) -> None:
    request = _request()
    value = _receipt_value(request)
    value[field] = forged
    with pytest.raises(authority.PrivacyGovernanceAuthorityError, match=message):
        authority._validate_untrusted_governance_authority_receipt_structure_v1(
            request,
            _receipt_bytes(value),
            authenticated_client_uid=AUTHENTICATED_CLIENT_UID,
        )


@pytest.mark.parametrize(
    ("field", "forged", "message"),
    (
        (
            "authority_account_id",
            "candidate_signer@candidate",
            "exact reset genesis authority",
        ),
        ("authority_public_key", "ed0120" + "CD" * 32, "exact reset genesis authority"),
        ("audit_live_head_sha256", _digest("uncommitted live head"), "audit heads"),
        (
            "audit_previous_head_sha256",
            _digest("committed audit head"),
            "audit heads",
        ),
    ),
)
def test_receipt_signer_and_committed_audit_heads_match_exact_request(
    field: str, forged: object, message: str
) -> None:
    request = _request()
    value = _receipt_value(request)
    value[field] = forged
    with pytest.raises(authority.PrivacyGovernanceAuthorityError, match=message):
        authority._validate_untrusted_governance_authority_receipt_structure_v1(
            request,
            _receipt_bytes(value),
            authenticated_client_uid=AUTHENTICATED_CLIENT_UID,
        )


def test_receipt_rejects_noncanonical_but_decodable_base64() -> None:
    request = _request()
    value = _receipt_value(request)
    value["signed_transaction_norito_base64"] = "YR=="
    value["signed_transaction_sha256"] = hashlib.sha256(b"a").hexdigest()
    with pytest.raises(
        authority.PrivacyGovernanceAuthorityError,
        match="bounded canonical base64",
    ):
        authority._validate_untrusted_governance_authority_receipt_structure_v1(
            request,
            _receipt_bytes(value),
            authenticated_client_uid=AUTHENTICATED_CLIENT_UID,
        )


def test_receipt_rejects_unmarked_native_transaction_hash() -> None:
    request = _request()
    value = _receipt_value(request)
    value["transaction_hash_hex"] = UNMARKED_IROHA_HASH
    with pytest.raises(
        authority.PrivacyGovernanceAuthorityError,
        match="canonical Iroha marker bit",
    ):
        authority._validate_untrusted_governance_authority_receipt_structure_v1(
            request,
            _receipt_bytes(value),
            authenticated_client_uid=AUTHENTICATED_CLIENT_UID,
        )


def test_self_consistent_forgery_cannot_reach_signer_validator_path_or_replay_io(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    request = _request()
    forged_requests = (
        _request(_splice_transaction_payload),
        _request(lambda row: row.update(source_commit="3" * 40)),
        _request(_reuse_candidate_signer),
        _request(_splice_reset_network),
        _request(_stale_run),
    )
    receipt = _receipt_bytes(_receipt_value(request))
    replay_state = {"head": _digest("unchanged replay head"), "sequence": 9}
    before = dict(replay_state)

    def forbidden(*_args: object, **_kwargs: object) -> object:
        raise AssertionError("provisioning barrier allowed signer, path, or replay I/O")

    preflight_modes: list[bool] = []

    def unavailable(_role: str, *, require_signing: bool) -> object:
        assert isinstance(require_signing, bool)
        preflight_modes.append(require_signing)
        raise authority.taira_authority_client.TairaAuthorityClientError(
            "fixed service unavailable"
        )

    monkeypatch.setattr(authority.taira_authority_client, "preflight", unavailable)

    for owner, name in (
        (builtins, "open"),
        (Path, "open"),
        (Path, "read_bytes"),
        (Path, "read_text"),
        (Path, "write_bytes"),
        (Path, "write_text"),
        (authority, "_validate_untrusted_governance_authority_receipt_structure_v1"),
        (authority, "_build_untrusted_governance_authority_request_v1"),
    ):
        monkeypatch.setattr(owner, name, forbidden)

    operations = [
        lambda: authority.request_authenticated_governance_transaction_v1(request),
        lambda: authority.request_authenticated_governance_transaction_v1(object()),
        lambda: authority.validate_authenticated_governance_receipt_v1(
            request, receipt
        ),
        lambda: authority.validate_authenticated_governance_receipt_v1(
            object(), b"synthetic self-hashed receipt"
        ),
    ]
    operations.extend(
        lambda forged=forged: authority.request_authenticated_governance_transaction_v1(
            forged
        )
        for forged in forged_requests
    )
    for operation in operations:
        with pytest.raises(
            authority.PrivacyGovernanceAuthorityError,
            match=authority.PROVISIONING_BARRIER,
        ) as raised:
            operation()
        assert authority.AUTHORITY_ENVELOPE_SCHEMA in str(raised.value)
        assert authority.REPLAY_NAMESPACE in str(raised.value)

    assert preflight_modes == [
        True,
        True,
        False,
        False,
        True,
        True,
        True,
        True,
        True,
    ]
    assert replay_state == before
    assert list(tmp_path.iterdir()) == []


def test_both_public_entrypoints_preflight_before_authenticated_operations() -> None:
    source = (ROOT / "scripts/taira_privacy_governance_authority.py").read_text(
        encoding="utf-8"
    )
    module = ast.parse(source)
    functions = {
        node.name: node
        for node in module.body
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
    }
    for name in (
        "request_authenticated_governance_transaction_v1",
        "validate_authenticated_governance_receipt_v1",
    ):
        operations = list(functions[name].body)
        if isinstance(operations[0], ast.Expr) and isinstance(
            operations[0].value, ast.Constant
        ):
            operations.pop(0)
        first = operations[0]
        assert isinstance(first, ast.Assign)
        assert len(first.targets) == 1
        target = first.targets[0]
        assert isinstance(target, ast.Name)
        assert target.id == "client_uid"
        assert isinstance(first.value, ast.Call)
        assert isinstance(first.value.func, ast.Name)
        assert (
            first.value.func.id
            == "_require_provisioned_privacy_governance_authority_v1"
        )
        assert first.value.args == []
        if name == "request_authenticated_governance_transaction_v1":
            assert first.value.keywords == []
        else:
            assert len(first.value.keywords) == 1
            keyword = first.value.keywords[0]
            assert keyword.arg == "require_signing"
            assert isinstance(keyword.value, ast.Constant)
            assert keyword.value.value is False
        arguments = functions[name].args
        assert "client_uid" not in {
            argument.arg
            for argument in (
                *arguments.posonlyargs,
                *arguments.args,
                *arguments.kwonlyargs,
            )
        }
        structural_calls = [
            node
            for node in ast.walk(functions[name])
            if isinstance(node, ast.Call)
            and isinstance(node.func, ast.Name)
            and node.func.id
            == "_validate_untrusted_governance_authority_receipt_structure_v1"
        ]
        assert len(structural_calls) == 1
        structural_call = structural_calls[0]
        assert len(structural_call.args) == 2
        assert len(structural_call.keywords) == 1
        client_uid_keyword = structural_call.keywords[0]
        assert client_uid_keyword.arg == "authenticated_client_uid"
        assert isinstance(client_uid_keyword.value, ast.Name)
        assert client_uid_keyword.value.id == "client_uid"
        client_methods = {
            node.func.attr
            for node in ast.walk(functions[name])
            if isinstance(node, ast.Call)
            and isinstance(node.func, ast.Attribute)
            and isinstance(node.func.value, ast.Name)
            and node.func.value.id == "taira_authority_client"
        }
        expected = (
            "authorize"
            if name == "request_authenticated_governance_transaction_v1"
            else "verify_receipt"
        )
        assert expected in client_methods

    assert "os.environ" not in source
    assert "getenv(" not in source
    assert authority.AUTHORITY_ENVELOPE_SCHEMA in source
    assert authority.REPLAY_NAMESPACE in source
    assert authority.LEGACY_VERANGE_AUTHORITY_ENVELOPE_SCHEMA != (
        authority.AUTHORITY_ENVELOPE_SCHEMA
    )
    assert authority.LEGACY_VERANGE_REPLAY_NAMESPACE != authority.REPLAY_NAMESPACE


def test_private_untrusted_helpers_have_no_production_caller_or_installed_bypass() -> None:
    module_path = ROOT / "scripts/taira_privacy_governance_authority.py"
    private_helpers = (
        "_build_untrusted_governance_authority_request_v1",
        "_validated_untrusted_request_value_v1",
        "_validate_untrusted_governance_authority_receipt_structure_v1",
    )
    production_callers: dict[str, list[str]] = {name: [] for name in private_helpers}
    for path in sorted((ROOT / "scripts").glob("*.py")):
        if path == module_path:
            continue
        source = path.read_text(encoding="utf-8")
        for name in private_helpers:
            if name in source:
                production_callers[name].append(path.name)
    assert production_callers == {name: [] for name in private_helpers}
    assert controller_seal.MACOS_FILES.count(
        "scripts/taira_privacy_governance_authority.py"
    ) == 1
    assert rollout_admission.MACOS_CONTROLLER_FILES == tuple(
        sorted(controller_seal.MACOS_FILES)
    )
    assert dict(sealed_controller.CONTROLLER_CASE_RUNNERS) == {}

    request = json.loads(
        action_driver_ipc.build_verange_request(
            asset_definition_id="rose#wonderland",
            candidate_binding_sha256=_digest("candidate"),
            creation_time_millis=1_900_000_000_000,
            network_id_hex="1" * 64,
            nonce=7,
            ttl_millis=60_000,
        )
    )
    assert not (
        {
            "authority_socket",
            "client_uid",
            "credential",
            "endpoint",
            "genesis_private_key",
            "torii_endpoint",
        }
        & set(request)
    )
