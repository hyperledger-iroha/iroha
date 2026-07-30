from __future__ import annotations

import base64
import importlib
import json
import sys
import types
from pathlib import Path
from typing import Any

import requests
import pytest
from blake3 import blake3


# These tests exercise the pure Python SDK layer independently of a previously
# built local extension. CI still builds and tests the Rust extension itself.
PACKAGE_ROOT = Path(__file__).resolve().parents[1] / "src" / "iroha_python"
PURE_PACKAGE = "_iroha_python_source_test"
if PURE_PACKAGE not in sys.modules:
    package = types.ModuleType(PURE_PACKAGE)
    package.__path__ = [str(PACKAGE_ROOT)]
    package.__package__ = PURE_PACKAGE
    sys.modules[PURE_PACKAGE] = package
if f"{PURE_PACKAGE}.sorafs" not in sys.modules:
    sorafs = types.ModuleType(f"{PURE_PACKAGE}.sorafs")
    sorafs.SorafsAliasError = type("SorafsAliasError", (Exception,), {})
    for name in ("SorafsAliasEvaluation", "SorafsAliasWarning"):
        setattr(sorafs, name, type(name, (), {}))

    class SorafsAliasPolicy:
        @classmethod
        def defaults(cls) -> "SorafsAliasPolicy":
            return cls()

    sorafs.SorafsAliasPolicy = SorafsAliasPolicy
    sorafs.enforce_alias_policy = lambda *args, **kwargs: None
    sys.modules[f"{PURE_PACKAGE}.sorafs"] = sorafs

client_module = importlib.import_module(f"{PURE_PACKAGE}.client")
settlement_module = importlib.import_module(f"{PURE_PACKAGE}.settlement")

BatchMode = settlement_module.BatchMode
DeadlineCondition = settlement_module.DeadlineCondition
EscrowValue = settlement_module.EscrowValue
OracleCondition = settlement_module.OracleCondition
Payment = settlement_module.Payment
ContractCallIntent = client_module.ContractCallIntent
ToriiClient = client_module.ToriiClient
VerifiedCommittedTransaction = client_module.VerifiedCommittedTransaction


def _response(payload: Any, status: int = 200) -> requests.Response:
    response = requests.Response()
    response.status_code = status
    response.headers["Content-Type"] = "application/json"
    response._content = json.dumps(payload).encode("utf-8")
    return response


def _norito_response(payload: bytes, status: int = 200) -> requests.Response:
    response = requests.Response()
    response.status_code = status
    response.headers["Content-Type"] = "application/x-norito"
    response._content = payload
    return response


class FakeSession:
    def __init__(self, responses: list[requests.Response]) -> None:
        self.responses = list(responses)
        self.calls: list[dict[str, Any]] = []

    def request(self, method: str, url: str, **kwargs: Any) -> requests.Response:
        self.calls.append({"method": method, "url": url, **kwargs})
        return self.responses.pop(0)


def test_typed_batch_and_conditional_escrow_payloads_are_canonical() -> None:
    payment = Payment("leg-1", "recipient@payments", "7.5")
    assert payment.to_payload() == {
        "id": "leg-1",
        "to": "recipient@payments",
        "amount": "7.5",
    }
    assert BatchMode.INDEPENDENT.value == "Independent"

    approved = OracleCondition.equals(
        "approved",
        EscrowValue.boolean(True),
        "oracle@payments",
        order=1,
    )
    maximum = OracleCondition.quantity_at_most(
        "amount",
        "100",
        "oracle@payments",
        order=2,
    )
    deadline = DeadlineCondition.within(hours=1, minutes=30)

    assert approved.to_payload()["value"]["predicate"] == {
        "kind": "Equals",
        "value": {"kind": "Bool", "value": True},
    }
    assert maximum.to_payload()["value"]["predicate"] == {
        "kind": "QuantityAtMost",
        "value": "100",
    }
    assert deadline.to_payload() == {
        "kind": "Within",
        "value": {"id": "deadline", "duration_ms": 5_400_000},
    }
    with pytest.raises(ValueError, match="payment id"):
        Payment(" leg-1", "recipient@payments", "1")
    with pytest.raises(ValueError, match="positive integer"):
        OracleCondition.equals(
            "approved",
            EscrowValue.boolean(True),
            "oracle@payments",
            order=0,
        )


def test_native_independent_batch_outcomes_project_to_ordered_receipt() -> None:
    payload = {
        "hash": "ab" * 32,
        "batch_transfer_outcomes": [
            {
                "leg_index": 0,
                "leg_id": "first",
                "destination": "first@payments",
                "amount": "20",
                "status": {"status": "Applied", "value": None},
            },
            {
                "leg_index": 1,
                "leg_id": "second",
                "destination": "second@payments",
                "amount": "60",
                "status": {
                    "status": "Rejected",
                    "value": {
                        "code": {
                            "code": "HoldingLimitExceeded",
                            "value": None,
                        },
                        "message": "destination holding limit exceeded",
                    },
                },
            },
        ],
    }

    projected = client_module._with_batch_transfer_receipt(payload)

    assert projected["batch_receipt"]["mode"] == "Independent"
    assert [leg["id"] for leg in projected["batch_receipt"]["legs"]] == [
        "first",
        "second",
    ]
    assert projected["batch_receipt"]["legs"][0]["status"] == "Applied"
    assert projected["batch_receipt"]["legs"][1]["status"] == "Rejected"
    assert (
        projected["batch_receipt"]["legs"][1]["code"]
        == "HoldingLimitExceeded"
    )


def test_signed_role_scoped_escrow_queries_use_native_query_payloads(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    crypto = types.ModuleType(f"{PURE_PACKAGE}.crypto")
    crypto.build_find_asset_escrows_by_seller_query = (
        lambda authority, private_key, seller: b"seller-query"
    )
    crypto.build_find_asset_escrows_by_buyer_query = (
        lambda authority, private_key, buyer: b"buyer-query"
    )
    monkeypatch.setitem(sys.modules, f"{PURE_PACKAGE}.crypto", crypto)
    records = [
        {
            "id": "escrow-locked",
            "seller": "seller@payments",
            "buyer": "buyer@payments",
            "status": {"status": "Locked", "value": None},
        },
        {
            "id": "escrow-released",
            "seller": "seller@payments",
            "buyer": "buyer@payments",
            "status": {"status": "Released", "value": None},
        },
    ]
    response_payload = {
        "kind": "Iterable",
        "content": {
            "kind": "AssetEscrowRecord",
            "content": records,
        },
    }
    session = FakeSession([_response(response_payload), _response(response_payload)])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    seller_records = client.list_asset_escrows_by_seller(
        seller="seller@payments",
        authority="authority@payments",
        private_key_hex="11" * 32,
        status="Locked",
    )
    buyer_records = client.list_asset_escrows_by_buyer(
        buyer="buyer@payments",
        authority="authority@payments",
        private_key=b"\x11" * 32,
        escrow_id="escrow-released",
    )

    assert [record["id"] for record in seller_records] == ["escrow-locked"]
    assert [record["id"] for record in buyer_records] == ["escrow-released"]
    assert [call["data"] for call in session.calls] == [
        b"seller-query",
        b"buyer-query",
    ]
    assert all(call["url"].endswith("/query") for call in session.calls)


def test_verified_committed_transaction_uses_two_signed_native_queries(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    transaction_hash = "11" * 32
    block_hash = "22" * 32
    result_hash = "33" * 32
    crypto = types.ModuleType(f"{PURE_PACKAGE}.crypto")
    crypto.build_find_committed_transaction_query = (
        lambda authority, private_key, requested_hash: b"transaction-query"
    )
    crypto.committed_transaction_carrier_block_hash = (
        lambda requested_hash, response: block_hash
    )
    crypto.build_find_block_by_hash_query = (
        lambda authority, private_key, requested_hash: b"block-query"
    )
    crypto.verify_committed_transaction_inclusion = (
        lambda requested_hash, transaction_response, block_response: {
            "transaction_hash": transaction_hash,
            "block_hash": block_hash,
            "block_height": 7,
            "result_hash": result_hash,
            "proof_kind": "ordinary",
            "entrypoint_kind": "External",
            "authority": "authority@payments",
            "signer_public_key_hex": "44" * 32,
            "metadata": {"walkthrough": "availability"},
            "executable": {"Instructions": []},
            "result_ok": True,
            "rejection_code": None,
            "rejection_message": None,
            "contract_rejection": None,
            "batch_outcomes": [],
            "committed_transaction": {
                "entrypoint_hash": transaction_hash,
                "block_hash": block_hash,
            },
        }
    )
    monkeypatch.setitem(sys.modules, f"{PURE_PACKAGE}.crypto", crypto)
    session = FakeSession(
        [
            _norito_response(b"transaction-response"),
            _norito_response(b"block-response"),
        ]
    )
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    verified = client.get_verified_committed_transaction(
        transaction_hash=transaction_hash,
        authority="authority@payments",
        private_key_hex="44" * 32,
    )

    assert verified.transaction_hash == transaction_hash
    assert verified.block_hash == block_hash
    assert verified.block_height == 7
    assert verified.result_hash == result_hash
    assert verified.proof_kind == "ordinary"
    assert verified.entrypoint_kind == "External"
    assert verified.authority == "authority@payments"
    assert verified.signer_public_key_hex == "44" * 32
    assert verified.metadata == {"walkthrough": "availability"}
    assert verified.executable == {"Instructions": []}
    assert verified.result_ok
    assert verified.rejection_code is None
    assert verified.batch_outcomes == ()
    assert [call["data"] for call in session.calls] == [
        b"transaction-query",
        b"block-query",
    ]
    assert all(
        call["headers"]["Accept"] == "application/x-norito"
        for call in session.calls
    )


def test_verified_contract_rejection_is_manifest_typed_and_fail_closed() -> None:
    payload = {
        "transaction_hash": "11" * 32,
        "block_hash": "22" * 32,
        "block_height": 7,
        "result_hash": "33" * 32,
        "proof_kind": "ordinary",
        "entrypoint_kind": "External",
        "authority": "authority@payments",
        "signer_public_key_hex": "44" * 32,
        "metadata": {},
        "executable": {"Instructions": []},
        "result_ok": False,
        "rejection_code": "BelowMinimum",
        "rejection_message": "contract rejection",
        "contract_rejection": {
            "contract": "BoiFiLiquidity",
            "namespace": "FiLiquidityError",
            "name": "BelowMinimum",
            "code": 18,
        },
        "batch_outcomes": [],
        "committed_transaction": {},
    }
    verified = VerifiedCommittedTransaction.from_payload(payload)
    assert verified.rejection_code == "BelowMinimum"
    assert verified.contract_rejection == {
        "contract": "BoiFiLiquidity",
        "namespace": "FiLiquidityError",
        "name": "BelowMinimum",
        "code": 18,
    }

    unknown_field = {**payload, "unverified_hint": "ignored"}
    with pytest.raises(ValueError, match="must contain exactly"):
        VerifiedCommittedTransaction.from_payload(unknown_field)

    missing_field = dict(payload)
    del missing_field["committed_transaction"]
    with pytest.raises(ValueError, match="must contain exactly"):
        VerifiedCommittedTransaction.from_payload(missing_field)

    mismatched = dict(payload)
    mismatched["rejection_code"] = "NotPermitted"
    with pytest.raises(ValueError, match="manifest-authenticated"):
        VerifiedCommittedTransaction.from_payload(mismatched)

    for invalid_code in (True, "18", 0, 0x1_0000_0000):
        malformed = dict(payload)
        malformed["contract_rejection"] = {
            **payload["contract_rejection"],
            "code": invalid_code,
        }
        with pytest.raises((TypeError, ValueError), match="contract rejection code"):
            VerifiedCommittedTransaction.from_payload(malformed)


class FakeInstruction:
    def __init__(self, payload: bytes, wire_id: str) -> None:
        self.payload = payload
        self._wire_id = wire_id

    def to_norito_bytes(self) -> bytes:
        return self.payload

    def wire_id(self) -> str:
        return self._wire_id


class FakeDraft:
    def __init__(self) -> None:
        self.entries: list[tuple[Any, ...]] = []
        self.explicit_batch = False

    def use_executable_batch(self) -> "FakeDraft":
        self.explicit_batch = True
        return self

    def add_contract_call(self, *entry: Any) -> None:
        self.entries.append(("contract_call", *entry))

    def add_instruction(self, instruction: FakeInstruction) -> None:
        self.entries.append(("instruction", instruction))


def _prepared_batch_response(
    calls: list[ContractCallIntent],
    instruction: FakeInstruction,
) -> dict[str, Any]:
    arguments = [b"first-arguments", b"second-arguments"]
    binding_items: list[dict[str, Any]] = []
    prepared_entries: list[dict[str, Any]] = []
    for index, (call, encoded_arguments) in enumerate(zip(calls, arguments)):
        address = call.expected_contract_address or call.contract_address
        code_hash = call.expected_code_hash_hex
        abi_hash = call.expected_abi_hash_hex
        binding_items.append(
            {
                "index": index,
                "kind": "contract_call",
                "contract_alias": call.contract_alias,
                "contract_address": address,
                "dataspace": "payments",
                "code_hash_hex": code_hash,
                "abi_hash_hex": abi_hash,
                "entrypoint": call.entrypoint,
                "payload_digest_hex": f"{index + 1:064x}",
                "arguments_digest_hex": blake3(
                    client_module._CONTRACT_CALL_BATCH_ARGUMENTS_DOMAIN_V1
                    + b"\x01"
                    + encoded_arguments
                ).hexdigest(),
            }
        )
        prepared_entries.append(
            {
                "index": index,
                "kind": "contract_call",
                "contract_address": address,
                "code_hash_hex": code_hash,
                "abi_hash_hex": abi_hash,
                "entrypoint": call.entrypoint,
                "arguments_b64": base64.b64encode(encoded_arguments).decode(
                    "ascii"
                ),
            }
        )
    instruction_index = len(calls)
    instruction_bytes = instruction.to_norito_bytes()
    binding_items.append(
        {
            "index": instruction_index,
            "kind": "instruction",
            "wire_id": instruction.wire_id(),
            "instruction_digest_hex": blake3(
                client_module._CONTRACT_CALL_BATCH_INSTRUCTION_DOMAIN_V1
                + instruction_bytes
            ).hexdigest(),
        }
    )
    prepared_entries.append(
        {
            "index": instruction_index,
            "kind": "instruction",
            "wire_id": instruction.wire_id(),
            "instruction_b64": base64.b64encode(instruction_bytes).decode(
                "ascii"
            ),
        }
    )
    binding = {"version": 1, "items": binding_items}
    binding_bytes = json.dumps(
        binding,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    return {
        "ok": True,
        "binding": binding,
        "binding_digest_hex": blake3(
            client_module._CONTRACT_CALL_BATCH_BINDING_DOMAIN_V1 + binding_bytes
        ).hexdigest(),
        "prepared_entries": prepared_entries,
    }


def test_contract_intents_prepare_ordered_batch_and_keep_signing_local(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    crypto = types.ModuleType(f"{PURE_PACKAGE}.crypto")
    crypto._NativeInstruction = FakeInstruction
    monkeypatch.setitem(sys.modules, f"{PURE_PACKAGE}.crypto", crypto)
    first = ContractCallIntent(
        "pay",
        contract_alias="wallet::payments",
        payload={"amount": "10"},
        expected_contract_address="contract:wallet",
        expected_code_hash_hex="a" * 64,
        expected_abi_hash_hex="b" * 64,
    )
    second = ContractCallIntent(
        "record",
        contract_address="contract:audit",
        payload={"reference": "r-1"},
        expected_code_hash_hex="c" * 64,
        expected_abi_hash_hex="d" * 64,
    )
    instruction = FakeInstruction(b"native-instruction", "transfer_asset")
    session = FakeSession(
        [_response(_prepared_batch_response([first, second], instruction))]
    )
    client = ToriiClient("http://torii.example", session=session, max_retries=0)
    draft = FakeDraft()
    submitted: dict[str, Any] = {}
    client._transaction_draft = lambda **kwargs: draft

    def submit(prepared_draft: FakeDraft, **kwargs: Any) -> dict[str, Any]:
        submitted.update(kwargs)
        return {
            "hash": "ef" * 32,
            "submission": {"accepted": True},
        }

    client._submit_transaction_draft_result = submit

    result = client.call_contract_batch_and_wait(
        chain_id="test-chain",
        authority="authority@payments",
        private_key_hex="11" * 32,
        entries=[first, second, instruction],
        metadata={"case_reference": "r-1"},
        wait=False,
    )

    assert draft.explicit_batch is True
    assert [entry[0] for entry in draft.entries] == [
        "contract_call",
        "contract_call",
        "instruction",
    ]
    assert submitted["private_key_hex"] == "11" * 32
    request_body = session.calls[0]["data"]
    assert b"11" * 32 not in request_body
    assert result["tx_hash_hex"] == "ef" * 32


def test_single_contract_call_is_the_local_batch_convenience_form() -> None:
    client = ToriiClient(
        "http://torii.example",
        session=FakeSession([]),
        max_retries=0,
    )
    captured: dict[str, Any] = {}

    def call_batch(**kwargs: Any) -> dict[str, Any]:
        captured.update(kwargs)
        return {
            "hash": "ab" * 32,
            "submission": {"accepted": True},
        }

    client.call_contract_batch_and_wait = call_batch
    result = client.call_contract_and_wait(
        chain_id="test-chain",
        authority="authority@payments",
        private_key=b"\x22" * 32,
        contract_alias="wallet::payments",
        entrypoint="pay",
        payload={"amount": "10"},
        metadata={"case_reference": "r-1"},
        wait=False,
    )

    assert len(captured["entries"]) == 1
    assert isinstance(captured["entries"][0], ContractCallIntent)
    assert captured["private_key"] == b"\x22" * 32
    assert captured["private_key_hex"] is None
    assert result["tx_hashes"] == ["ab" * 32]
