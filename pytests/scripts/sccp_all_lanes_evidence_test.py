import base64
import copy
import json
import hashlib
import sys
import urllib.parse
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
from types import SimpleNamespace


TON_CODE_BOC_HEX = "b5ee9c720101020100070001020101000202"
TON_CODE_BOC_BASE64 = base64.b64encode(bytes.fromhex(TON_CODE_BOC_HEX)).decode("ascii")
TON_CODE_BOC_ROOT_HASH = (
    "49725ad44ef5ed5feaa27f88679cabae427209a6bea318cb9b66030131aae6fe"
)


class HostilePublicKey:
    def __str__(self):
        raise AssertionError("secret-token hostile __str__")

    def __repr__(self):
        return "secret-token-hostile-key"


def load_evidence_module():
    script_path = (
        Path(__file__).resolve().parents[2] / "scripts" / "sccp_all_lanes_evidence.py"
    )
    spec = spec_from_file_location("sccp_all_lanes_evidence", script_path)
    module = module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def load_tron_live_module():
    script_path = (
        Path(__file__).resolve().parents[2] / "scripts" / "sccp_tron_live_evidence.py"
    )
    spec = spec_from_file_location("sccp_tron_live_evidence_for_all_lanes", script_path)
    module = module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def load_evm_live_module():
    script_path = (
        Path(__file__).resolve().parents[2] / "scripts" / "sccp_evm_live_evidence.py"
    )
    spec = spec_from_file_location("sccp_evm_live_evidence_for_all_lanes", script_path)
    module = module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def load_evm_source_live_module():
    script_path = (
        Path(__file__).resolve().parents[2]
        / "scripts"
        / "sccp_evm_source_live_evidence.py"
    )
    spec = spec_from_file_location(
        "sccp_evm_source_live_evidence_for_all_lanes",
        script_path,
    )
    module = module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def load_solana_live_module():
    script_path = (
        Path(__file__).resolve().parents[2] / "scripts" / "sccp_solana_live_evidence.py"
    )
    spec = spec_from_file_location("sccp_solana_live_evidence_for_all_lanes", script_path)
    module = module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def load_ton_live_module():
    script_path = (
        Path(__file__).resolve().parents[2] / "scripts" / "sccp_ton_live_evidence.py"
    )
    spec = spec_from_file_location("sccp_ton_live_evidence_for_all_lanes", script_path)
    module = module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def hex32(seed):
    return "0x" + f"{seed % 256:02x}" * 32


def hex20(seed):
    return "0x" + f"{seed % 256:02x}" * 20


def raw_hex(value):
    return bytes.fromhex(value.removeprefix("0x"))


def noncanonical_base64_alias(raw: bytes) -> str:
    encoded = base64.b64encode(raw).decode("ascii")
    alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
    if encoded.endswith("=="):
        position = len(encoded) - 3
    elif encoded.endswith("="):
        position = len(encoded) - 2
    else:
        raise AssertionError("test fixture must have base64 padding")
    replacement = alphabet[alphabet.index(encoded[position]) ^ 1]
    return encoded[:position] + replacement + encoded[position + 1 :]


def tron_signature_hex(module, message_hash, *, expected_owner20, nonce_start=2):
    scalar_order = module.SECP256K1_SCALAR_ORDER
    private_key = 1
    z = int.from_bytes(message_hash, "big")
    for nonce in range(nonce_start, nonce_start + 128):
        point = module._secp256k1_scalar_mul(nonce, module.SECP256K1_GENERATOR)
        assert point is not None
        x, y = point
        r = x % scalar_order
        if r == 0:
            continue
        s = (pow(nonce, -1, scalar_order) * (z + r * private_key)) % scalar_order
        if s == 0:
            continue
        recovery_id = (1 if y & 1 else 0) + (2 if x >= scalar_order else 0)
        if s > module.SECP256K1_SCALAR_HALF_ORDER:
            s = scalar_order - s
            recovery_id ^= 1
        signature = r.to_bytes(32, "big") + s.to_bytes(32, "big") + bytes([recovery_id])
        if (
            module._tron_recovered_signature_address20(message_hash, signature)
            == expected_owner20
        ):
            return signature.hex()
    raise AssertionError("could not build recoverable TRON signature")


BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


def base58_encode(raw):
    numeric = int.from_bytes(raw, "big")
    encoded = ""
    while numeric:
        numeric, remainder = divmod(numeric, 58)
        encoded = BASE58_ALPHABET[remainder] + encoded
    leading_zeros = len(raw) - len(raw.lstrip(b"\x00"))
    return "1" * leading_zeros + (encoded or "")


def tron_base58check(address20):
    payload = b"\x41" + address20
    checksum = hashlib.sha256(hashlib.sha256(payload).digest()).digest()[:4]
    return base58_encode(payload + checksum)


class FakeResponse:
    def __init__(self, payload):
        self.payload = json.dumps(payload).encode("utf-8")

    def __enter__(self):
        return self

    def __exit__(self, _exc_type, _exc, _traceback):
        return False

    def read(self, size=-1):
        if size is None or size < 0:
            return self.payload
        return self.payload[:size]


def abi_word_u32(value):
    return value.to_bytes(32, "big")


def abi_word_address(address20):
    return b"\x00" * 12 + address20


def abi_word_hex(value):
    return int(value, 16).to_bytes(32, "big")


def tron_route_canary_submit_call_data(
    *,
    message_id=bytes.fromhex("dd" * 32),
    source_domain=0,
    target_domain=5,
    commitment_root=bytes.fromhex("ee" * 32),
    statement_hash=bytes.fromhex("f1" * 32),
    payload_hash=bytes.fromhex("ab" * 32),
    finality_height=123,
    finality_block_hash=bytes.fromhex("cd" * 32),
):
    g2_generator_words = tuple(
        abi_word_hex(value)
        for value in (
            "1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed",
            "198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2",
            "12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa",
            "090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b",
        )
    )
    proof_words = [
        abi_word_u32(1),
        message_id,
        abi_word_u32(source_domain),
        commitment_root,
        abi_word_u32(1),
        abi_word_u32(2),
        *g2_generator_words,
        abi_word_u32(1),
        abi_word_u32(2),
    ]
    proof_bytes = b"".join(proof_words)
    public_inputs = [
        message_id,
        payload_hash,
        abi_word_u32(target_domain),
        commitment_root,
        abi_word_u32(finality_height),
        finality_block_hash,
    ]
    call_data = bytearray(bytes.fromhex("bd57826c"))
    call_data.extend(abi_word_u32(32 * 8))
    for word in public_inputs:
        call_data.extend(word)
    call_data.extend(statement_hash)
    call_data.extend(abi_word_u32(len(proof_bytes)))
    call_data.extend(proof_bytes)
    return bytes(call_data)


def fake_tron_live_opener(module):
    network_id = bytes.fromhex("33" * 32)
    bridge20 = bytes.fromhex("11" * 20)
    owner20 = bytes.fromhex("7e5f4552091a69125d5dfcb7b8c2659029395bdf")
    destination20 = bytes.fromhex("44" * 20)
    source_runtime = bytes.fromhex("6001600055")
    destination_runtime = bytes.fromhex("6002600055")
    verifier_code_hash = module.evidence.runtime_bytecode_hash(destination_runtime)
    verifier_key_hash = bytes.fromhex("cc" * 32)
    verifier_backend_hash = module.evidence._keccak_256(
        module.evidence.TRON_GROTH16_BACKEND.encode("utf-8")
    )
    proof_family_hash = module.evidence._keccak_256(
        module.evidence.SCCP_PROOF_FAMILY_STARK_FRI.encode("utf-8")
    )
    bridge = module.tron_base58check_from_address20(bridge20)
    destination = module.tron_base58check_from_address20(destination20)
    source_config = module.evidence.tron_source_bridge_config_hash(
        bridge_address=bridge20,
        network_id=network_id,
        source_domain=5,
        target_domain=0,
        owner_address=owner20,
    )
    destination_binding = module.evidence.tron_destination_binding_hash(
        network_id=network_id,
        source_domain=0,
        target_domain=5,
        verifier_address=destination,
        verifier_code_hash=verifier_code_hash,
        verifier_key_hash=verifier_key_hash,
    )
    route_canary_message_id = bytes.fromhex("dd" * 32)
    route_canary_commitment_root = bytes.fromhex("ee" * 32)
    route_canary_statement_hash = bytes.fromhex("f1" * 32)
    route_canary_call_data = tron_route_canary_submit_call_data(
        message_id=route_canary_message_id,
        commitment_root=route_canary_commitment_root,
        statement_hash=route_canary_statement_hash,
    )
    trigger = b"".join(
        [
            module._protobuf_bytes_field(1, b"\x41" + owner20),
            module._protobuf_bytes_field(2, b"\x41" + destination20),
            module._protobuf_bytes_field(4, route_canary_call_data),
        ]
    )
    parameter = b"".join(
        [
            module._protobuf_bytes_field(
                1,
                b"type.googleapis.com/protocol.TriggerSmartContract",
            ),
            module._protobuf_bytes_field(2, trigger),
        ]
    )
    contract = b"".join(
        [
            module._protobuf_u64_field(1, 31),
            module._protobuf_bytes_field(2, parameter),
        ]
    )
    route_canary_raw_data = b"".join(
        [
            module._protobuf_bytes_field(1, b"\x12\x34"),
            module._protobuf_u64_field(3, 12_345),
            module._protobuf_bytes_field(4, b"\x56" * 8),
            module._protobuf_u64_field(8, 123_456_789),
            module._protobuf_bytes_field(11, contract),
            module._protobuf_u64_field(14, 123_450_000),
            module._protobuf_u64_field(18, 50_000_000),
        ]
    )
    route_canary_raw_data_hex = route_canary_raw_data.hex()
    route_canary_transaction_id = hashlib.sha256(route_canary_raw_data).hexdigest()
    route_canary_signature = tron_signature_hex(
        module,
        hashlib.sha256(route_canary_raw_data).digest(),
        expected_owner20=owner20,
        nonce_start=17,
    )
    constant_words = {
        (bridge, "networkId()"): network_id,
        (bridge, "sourceDomain()"): abi_word_u32(5),
        (bridge, "targetDomain()"): abi_word_u32(0),
        (bridge, "owner()"): abi_word_address(owner20),
        (bridge, "sourceBridgeConfigHash()"): source_config,
        (destination, "networkId()"): network_id,
        (destination, "expectedSourceDomain()"): abi_word_u32(0),
        (destination, "expectedTargetDomain()"): abi_word_u32(5),
        (destination, "verifierCodeHash()"): verifier_code_hash,
        (destination, "verifierKeyHash()"): verifier_key_hash,
        (destination, "verifierBackendHash()"): verifier_backend_hash,
        (destination, "proofFamilyHash()"): proof_family_hash,
        (destination, "destinationBindingHash()"): destination_binding,
    }

    def opener(request, timeout):
        del timeout
        payload = json.loads(request.data.decode("utf-8"))
        if request.full_url.endswith("/wallet/getcontract"):
            if payload["value"] == bridge:
                return FakeResponse(
                    {
                        "contract_address": bridge,
                        "bytecode": source_runtime.hex(),
                        "code_hash": "source-observer-code-hash",
                    }
                )
            if payload["value"] == destination:
                return FakeResponse(
                    {
                        "contract_address": destination,
                        "bytecode": destination_runtime.hex(),
                        "code_hash": "destination-observer-code-hash",
                    }
                )
            raise AssertionError(f"unexpected getcontract value {payload['value']}")
        if request.full_url.endswith(("/wallet/gettransactioninfobyid", "/walletsolidity/gettransactioninfobyid")):
            assert payload["value"] == route_canary_transaction_id
            return FakeResponse(
                {
                    "id": route_canary_transaction_id,
                    "blockNumber": 234,
                    "blockTimeStamp": 567000,
                    "receipt": {"result": "SUCCESS"},
                    "log": [
                        {
                            "address": destination20.hex(),
                            "topics": [
                                module.TRON_MESSAGE_PROOF_ACCEPTED_TOPIC.hex(),
                                route_canary_message_id.hex(),
                                abi_word_u32(0).hex(),
                            ],
                            "data": b"".join(
                                (
                                    route_canary_commitment_root,
                                    route_canary_statement_hash,
                                    destination_binding,
                                    verifier_backend_hash,
                                    proof_family_hash,
                                    network_id,
                                )
                            ).hex(),
                        }
                    ],
                }
            )
        if request.full_url.endswith(("/wallet/gettransactionbyid", "/walletsolidity/gettransactionbyid")):
            assert payload["value"] == route_canary_transaction_id
            return FakeResponse(
                {
                    "txID": route_canary_transaction_id,
                    "ret": [{"contractRet": "SUCCESS"}],
                    "signature": [route_canary_signature],
                    "raw_data_hex": route_canary_raw_data_hex,
                    "raw_data": {
                        "contract": [
                            {
                                "type": "TriggerSmartContract",
                                "parameter": {
                                    "type_url": (
                                        "type.googleapis.com/protocol."
                                        "TriggerSmartContract"
                                    ),
                                    "value": {
                                        "owner_address": "41" + owner20.hex(),
                                        "contract_address": (
                                            "41" + destination20.hex()
                                        ),
                                        "data": route_canary_call_data.hex(),
                                    },
                                },
                            }
                        ]
                    },
                }
            )
        key = (payload["contract_address"], payload["function_selector"])
        if key == (destination, "usedMessageProofs(bytes32)"):
            assert payload["parameter"] == route_canary_message_id.hex()
            return FakeResponse(
                {
                    "result": {"result": True},
                    "constant_result": [abi_word_u32(1).hex()],
                }
            )
        return FakeResponse(
            {
                "result": {"result": True},
                "constant_result": [constant_words[key].hex()],
            }
        )

    return SimpleNamespace(
        opener=opener,
        network_id=network_id,
        bridge=bridge,
        bridge20=bridge20,
        owner20=owner20,
        destination=destination,
        source_config=source_config,
        destination_binding=destination_binding,
        source_runtime=source_runtime,
        source_code_hash=module.evidence.runtime_bytecode_hash(source_runtime),
        destination_runtime=destination_runtime,
        verifier_code_hash=verifier_code_hash,
        route_canary_transaction_id=route_canary_transaction_id,
    )


def fake_evm_live_opener(module, *, domain):
    bridge = "0x" + "11" * 20
    verifier = "0x" + "22" * 20
    network_id = module.evidence.evm_mainnet_network_id_for_domain(domain)
    bridge_runtime = bytes.fromhex("60806040526001")
    verifier_runtime = bytes.fromhex("60806040526002")
    verifier_code_hash = module.evidence.runtime_bytecode_hash(verifier_runtime)
    verifier_key_hash = bytes.fromhex("cc" * 32)
    bridge_code_hash = module.evidence.runtime_bytecode_hash(bridge_runtime)
    destination_binding = module.evidence.evm_destination_binding_hash(
        network_id=network_id,
        source_domain=0,
        target_domain=domain,
        verifier_address=bytes.fromhex("22" * 20),
        bridge_address=bytes.fromhex("11" * 20),
        verifier_code_hash=verifier_code_hash,
        verifier_key_hash=verifier_key_hash,
    )
    backend_hash = module.evidence._keccak_256(
        module.evidence.SCCP_EVM_GROTH16_BACKEND.encode("utf-8")
    )
    family_hash = module.evidence._keccak_256(
        module.evidence.SCCP_PROOF_FAMILY_STARK_FRI.encode("utf-8")
    )
    route_canary_transaction_hash = bytes.fromhex("44" * 32)
    route_canary_message_id = bytes.fromhex("55" * 32)
    route_canary_commitment_root = bytes.fromhex("66" * 32)
    route_canary_statement_hash = bytes.fromhex("77" * 32)
    route_canary_payload_hash = bytes.fromhex("ab" * 32)
    route_canary_finality_height = abi_word_u32(123)
    route_canary_finality_block_hash = bytes.fromhex("cd" * 32)
    route_canary_receipt_block_number = 0x1234
    route_canary_receipt_block_hash = bytes.fromhex("aa" * 32)
    route_canary_block_receipts_root = bytes.fromhex("bb" * 32)
    route_canary_call_data = tron_route_canary_submit_call_data(
        message_id=route_canary_message_id,
        payload_hash=route_canary_payload_hash,
        target_domain=domain,
        commitment_root=route_canary_commitment_root,
        statement_hash=route_canary_statement_hash,
        finality_height=123,
        finality_block_hash=route_canary_finality_block_hash,
    )
    route_canary_call_data_sha256 = hashlib.sha256(route_canary_call_data).digest()
    call_words = {
        (bridge, "verifier()"): abi_word_address(bytes.fromhex("22" * 20)),
        (bridge, "verifierCodeHash()"): verifier_code_hash,
        (bridge, "verifierKeyHash()"): verifier_key_hash,
        (bridge, "verifierBackendHash()"): backend_hash,
        (bridge, "proofFamilyHash()"): family_hash,
        (bridge, "networkId()"): network_id,
        (bridge, "expectedSourceDomain()"): abi_word_u32(0),
        (bridge, "expectedTargetDomain()"): abi_word_u32(domain),
        (bridge, "destinationBindingHash()"): destination_binding,
        (verifier, "verifyingKeyHash()"): verifier_key_hash,
    }
    selectors = {
        module._selector(signature): signature
        for signature in (*module.BRIDGE_VIEW_SIGNATURES, "verifyingKeyHash()")
    }
    used_message_selector = (
        "0x" + module.evidence._keccak_256(b"usedMessageProofs(bytes32)")[:4].hex()
    )

    def opener(request, timeout):
        del timeout
        payload = json.loads(request.data.decode("utf-8"))
        method = payload["method"]
        params = payload["params"]
        if method == "eth_chainId":
            return FakeResponse(
                {
                    "jsonrpc": "2.0",
                    "id": payload["id"],
                    "result": hex(module.EXPECTED_RPC_CHAIN_IDS[domain]),
                }
            )
        if method == "eth_getCode":
            address = params[0].lower()
            if address == bridge:
                return FakeResponse(
                    {
                        "jsonrpc": "2.0",
                        "id": payload["id"],
                        "result": "0x" + bridge_runtime.hex(),
                    }
                )
            if address == verifier:
                return FakeResponse(
                    {
                        "jsonrpc": "2.0",
                        "id": payload["id"],
                        "result": "0x" + verifier_runtime.hex(),
                    }
                )
            raise AssertionError(f"unexpected eth_getCode address {address}")
        if method == "eth_call":
            call = params[0]
            address = call["to"].lower()
            data = call["data"].lower()
            if data.startswith(used_message_selector):
                assert address == bridge
                assert data == used_message_selector + route_canary_message_id.hex()
                return FakeResponse(
                    {
                        "jsonrpc": "2.0",
                        "id": payload["id"],
                        "result": "0x" + abi_word_u32(1).hex(),
                    }
                )
            signature = selectors[data]
            return FakeResponse(
                {
                    "jsonrpc": "2.0",
                    "id": payload["id"],
                    "result": "0x" + call_words[(address, signature)].hex(),
                }
            )
        if method == "eth_getTransactionReceipt":
            assert params[0] == "0x" + route_canary_transaction_hash.hex()
            return FakeResponse(
                {
                    "jsonrpc": "2.0",
                    "id": payload["id"],
                    "result": {
                        "transactionHash": "0x" + route_canary_transaction_hash.hex(),
                        "status": "0x1",
                        "blockHash": "0x" + route_canary_receipt_block_hash.hex(),
                        "blockNumber": "0x1234",
                        "logs": [
                            {
                                "address": bridge,
                                "logIndex": "0x0",
                                "transactionHash": (
                                    "0x" + route_canary_transaction_hash.hex()
                                ),
                                "blockHash": "0x" + route_canary_receipt_block_hash.hex(),
                                "blockNumber": "0x1234",
                                "topics": [
                                    "0x" + module.EVM_MESSAGE_PROOF_ACCEPTED_TOPIC.hex(),
                                    "0x" + route_canary_message_id.hex(),
                                    "0x" + abi_word_u32(0).hex(),
                                ],
                                "data": (
                                    "0x"
                                    + b"".join(
                                        (
                                            route_canary_commitment_root,
                                            route_canary_statement_hash,
                                            destination_binding,
                                            backend_hash,
                                            family_hash,
                                            network_id,
                                        )
                                    ).hex()
                                ),
                            }
                        ],
                    },
                }
            )
        if method == "eth_getBlockByNumber":
            if params == ["finalized", False]:
                return FakeResponse(
                    {
                        "jsonrpc": "2.0",
                        "id": payload["id"],
                        "result": {
                            "hash": "0x" + route_canary_receipt_block_hash.hex(),
                            "number": "0x1234",
                            "receiptsRoot": (
                                "0x" + route_canary_block_receipts_root.hex()
                            ),
                        },
                    }
                )
            assert params == ["0x1234", False]
            return FakeResponse(
                {
                    "jsonrpc": "2.0",
                    "id": payload["id"],
                    "result": {
                        "hash": "0x" + route_canary_receipt_block_hash.hex(),
                        "number": "0x1234",
                        "receiptsRoot": "0x" + route_canary_block_receipts_root.hex(),
                    },
                }
            )
        if method == "eth_getTransactionByHash":
            assert params[0] == "0x" + route_canary_transaction_hash.hex()
            return FakeResponse(
                {
                    "jsonrpc": "2.0",
                    "id": payload["id"],
                    "result": {
                        "hash": "0x" + route_canary_transaction_hash.hex(),
                        "blockHash": "0x" + route_canary_receipt_block_hash.hex(),
                        "blockNumber": "0x1234",
                        "to": bridge,
                        "input": "0x" + route_canary_call_data.hex(),
                    },
                }
            )
        raise AssertionError(f"unexpected method {method}")

    return SimpleNamespace(
        opener=opener,
        bridge=bridge,
        verifier=verifier,
        network_id=network_id,
        bridge_code_hash=bridge_code_hash,
        bridge_runtime=bridge_runtime,
        verifier_code_hash=verifier_code_hash,
        verifier_runtime=verifier_runtime,
        verifier_key_hash=verifier_key_hash,
        destination_binding=destination_binding,
        route_canary_transaction_hash=route_canary_transaction_hash,
        route_canary_log_index=0,
        route_canary_receipt_block_number=route_canary_receipt_block_number,
        route_canary_receipt_block_hash=route_canary_receipt_block_hash,
        route_canary_block_receipts_root=route_canary_block_receipts_root,
        route_canary_message_id=route_canary_message_id,
        route_canary_call_data_sha256=route_canary_call_data_sha256,
        route_canary_payload_hash=route_canary_payload_hash,
        route_canary_target_domain=domain,
        route_canary_statement_hash=route_canary_statement_hash,
        route_canary_commitment_root=route_canary_commitment_root,
        route_canary_finality_height=route_canary_finality_height,
        route_canary_finality_block_hash=route_canary_finality_block_hash,
        route_canary_proof_version=1,
        route_canary_proof_source_domain=0,
    )


def fake_evm_source_live_opener(module, *, domain):
    bridge = "0x" + "11" * 20
    bridge_runtime = bytes.fromhex("60806040526003")
    bridge_code_hash = module._load_evidence_module(domain).runtime_bytecode_hash(
        bridge_runtime
    )
    deployment_input = bytes.fromhex("60016002")

    def opener(request, timeout):
        del timeout
        payload = json.loads(request.data.decode("utf-8"))
        method = payload["method"]
        params = payload["params"]
        if method == "eth_chainId":
            return FakeResponse(
                {
                    "jsonrpc": "2.0",
                    "id": payload["id"],
                    "result": hex(module.EXPECTED_RPC_CHAIN_IDS[domain]),
                }
            )
        if method == "eth_getCode":
            assert params[0].lower() == bridge
            return FakeResponse(
                {
                    "jsonrpc": "2.0",
                    "id": payload["id"],
                    "result": "0x" + bridge_runtime.hex(),
                }
            )
        if method == "eth_getTransactionReceipt":
            return FakeResponse(
                {
                    "jsonrpc": "2.0",
                    "id": payload["id"],
                    "result": {
                        "transactionHash": params[0],
                        "status": "0x1",
                        "contractAddress": bridge,
                        "blockHash": "0x" + "99" * 32,
                        "blockNumber": "0x1234",
                    },
                }
            )
        if method == "eth_getTransactionByHash":
            return FakeResponse(
                {
                    "jsonrpc": "2.0",
                    "id": payload["id"],
                    "result": {
                        "hash": params[0],
                        "blockHash": "0x" + "99" * 32,
                        "blockNumber": "0x1234",
                        "to": None,
                        "input": "0x" + deployment_input.hex(),
                    },
                }
            )
        if method == "eth_getBlockByNumber":
            if params == ["finalized", False]:
                return FakeResponse(
                    {
                        "jsonrpc": "2.0",
                        "id": payload["id"],
                        "result": {
                            "hash": "0x" + "99" * 32,
                            "number": "0x1234",
                            "receiptsRoot": "0x" + "bc" * 32,
                        },
                    }
                )
            assert params == ["0x1234", False]
            return FakeResponse(
                {
                    "jsonrpc": "2.0",
                    "id": payload["id"],
                    "result": {
                        "hash": "0x" + "99" * 32,
                        "number": "0x1234",
                        "receiptsRoot": "0x" + "bc" * 32,
                    },
                }
            )
        raise AssertionError(f"unexpected method {method}")

    return SimpleNamespace(
        opener=opener,
        bridge=bridge,
        bridge_runtime=bridge_runtime,
        bridge_code_hash=bridge_code_hash,
        deployment_input=deployment_input,
    )


def fake_solana_live_opener(module, *, program_id, programdata_address, program_bytes):
    programdata_raw = bytes.fromhex("11" * 32)
    program_account_data = (
        module.UPGRADEABLE_LOADER_PROGRAM_TAG.to_bytes(4, "little")
        + programdata_raw
    )
    programdata_account_data = bytearray()
    programdata_account_data.extend(
        module.UPGRADEABLE_LOADER_PROGRAMDATA_TAG.to_bytes(4, "little")
    )
    programdata_account_data.extend((4321).to_bytes(8, "little"))
    programdata_account_data.append(0)
    programdata_account_data.extend(bytes(32))
    programdata_account_data.extend(program_bytes)

    def account_payload(data, *, executable):
        return {
            "owner": module.UPGRADEABLE_LOADER_ID,
            "executable": executable,
            "data": [base64.b64encode(data).decode("ascii"), "base64"],
        }

    accounts = {
        program_id: account_payload(program_account_data, executable=True),
        programdata_address: account_payload(
            bytes(programdata_account_data),
            executable=False,
        ),
    }

    def opener(request, timeout):
        del timeout
        payload = json.loads(request.data.decode("utf-8"))
        assert payload["method"] == "getAccountInfo"
        address = payload["params"][0]
        return FakeResponse(
            {
                "jsonrpc": "2.0",
                "id": payload["id"],
                "result": {"context": {"slot": 9000}, "value": accounts[address]},
            }
        )

    return opener


def fake_ton_live_opener(
    module,
    *,
    verifier_contract_address,
    code_hash,
    account_state_hash,
):
    last_transaction_hash = bytes.fromhex("66" * 32)

    def opener(request, timeout):
        del timeout
        assert "x-api-key" not in {key.lower() for key, _value in request.header_items()}
        parsed = urllib.parse.urlparse(request.full_url)
        query = urllib.parse.parse_qs(parsed.query)
        assert query["address"] == [verifier_contract_address]
        assert query["include_boc"] == ["true"]
        return FakeResponse(
            {
                "accounts": [
                    {
                        "address": verifier_contract_address,
                        "status": "active",
                        "code_hash": "0x" + code_hash.hex(),
                        "code_boc": base64.b64encode(
                            bytes.fromhex(TON_CODE_BOC_HEX)
                        ).decode("ascii"),
                        "account_state_hash": "0x" + account_state_hash.hex(),
                        "last_transaction_lt": "123456",
                        "last_transaction_hash": "0x" + last_transaction_hash.hex(),
                    }
                ]
            }
        )

    return SimpleNamespace(
        opener=opener,
        last_transaction_hash=last_transaction_hash,
    )


def verifier_identity(profile, seed):
    if profile.chain in ("eth", "bsc"):
        return hex20(seed + 20)
    if profile.chain == "sol":
        return base58_encode(bytes([seed % 256]) * 32)
    if profile.chain == "ton":
        return "0:" + f"{seed % 256:02x}" * 32
    if profile.chain == "tron":
        return tron_base58check(bytes([seed % 256]) * 20)
    return "SccpBridge.submit_message_proof"


def source_material(module, profile, seed):
    record = {
        "version": 1,
        "source_domain": profile.domain,
        "source_chain": profile.chain,
        "source_proof_plan": profile.source_proof_plan,
        "finality_model": profile.finality_model,
        "adapter_circuit_id": module.SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID,
        "source_trust_anchor_id": profile.source_trust_anchor_id,
        "source_trust_anchor_hash": hex32(seed),
        "consensus_verifier_id": profile.consensus_verifier_id,
        "consensus_verifier_hash": hex32(seed + 1),
        "message_inclusion_verifier_id": profile.message_inclusion_verifier_id,
        "message_inclusion_verifier_hash": hex32(seed + 2),
        "finality_policy_id": profile.finality_policy_id,
        "finality_policy_hash": hex32(seed + 3),
        "placeholder_material": False,
    }
    if profile.source_state_verifier_id:
        record["source_state_verifier_id"] = profile.source_state_verifier_id
        record["source_state_verifier_hash"] = hex32(seed + 4)
    if profile.source_bridge_emitter_id:
        record["source_bridge_emitter_id"] = profile.source_bridge_emitter_id
        record["source_bridge_emitter_address"] = hex20(seed + 5)
        record["source_bridge_emitter_code_hash"] = hex32(seed + 6)
        if profile.chain in ("eth", "bsc"):
            evm_source_module = module._load_sibling_module(
                "sccp_eth_source_bridge_evidence.py"
                if profile.chain == "eth"
                else "sccp_bsc_source_bridge_evidence.py"
            )
            source_runtime = bytes([0x60, 0x80, 0x60, seed & 0xFF, 0x52])
            record["source_bridge_emitter_code_hash"] = (
                "0x" + evm_source_module.runtime_bytecode_hash(source_runtime).hex()
            )
            record["_comment_evm_source_rpc_chain_id"] = str(
                module.EVM_EXPECTED_RPC_CHAIN_IDS[profile.domain]
            )
            record["_comment_evm_source_block_tag"] = (
                "finalized" if profile.chain == "eth" else "latest"
            )
            record["_comment_evm_source_bridge_address"] = record[
                "source_bridge_emitter_address"
            ]
            record["_comment_evm_source_bridge_code_hash"] = record[
                "source_bridge_emitter_code_hash"
            ]
            record["_comment_evm_source_bridge_runtime_bytecode_hex"] = (
                "0x" + source_runtime.hex()
            )
            record["_comment_evm_source_deployment_transaction_hash"] = hex32(seed + 27)
            record["_comment_evm_source_deployment_transaction_block_hash"] = hex32(
                seed + 28
            )
            record["_comment_evm_source_deployment_transaction_block_number"] = str(
                1000 + seed
            )
            record["_comment_evm_source_deployment_transaction_input_sha256"] = hex32(
                seed + 30
            )
            record["_comment_evm_source_deployment_receipt_status"] = "0x1"
            record["_comment_evm_source_deployment_contract_address"] = record[
                "source_bridge_emitter_address"
            ]
            record["_comment_evm_source_deployment_block_hash"] = hex32(seed + 28)
            record["_comment_evm_source_deployment_block_number"] = str(1000 + seed)
            record["_comment_evm_source_deployment_block_receipts_root"] = hex32(
                seed + 29
            )
            if profile.chain == "eth":
                record["source_bridge_network_id"] = (
                    "0x" + evm_source_module.eth_source_bridge_network_id().hex()
                )
                record["source_bridge_config_hash"] = (
                    "0x"
                    + evm_source_module.eth_source_bridge_config_hash(
                        bridge_address=raw_hex(
                            record["source_bridge_emitter_address"]
                        ),
                        source_bridge_code_hash=raw_hex(
                            record["source_bridge_emitter_code_hash"]
                        ),
                        network_id=raw_hex(record["source_bridge_network_id"]),
                        source_domain=profile.domain,
                        target_domain=module.SCCP_DOMAIN_SORA,
                    ).hex()
                )
                record["_comment_eth_source_bridge_network_id"] = record[
                    "source_bridge_network_id"
                ]
                record["_comment_eth_source_bridge_config_hash"] = record[
                    "source_bridge_config_hash"
                ]
    if profile.tron_source_bridge_config_required:
        tron_module = module._load_sibling_module("sccp_tron_source_bridge_evidence.py")
        runtime = bytes([0x60, 0x80, 0x60, seed & 0xFF, 0x55])
        record["source_bridge_emitter_code_hash"] = (
            "0x" + tron_module.runtime_bytecode_hash(runtime).hex()
        )
        record["source_bridge_network_id"] = hex32(seed + 7)
        record["source_bridge_owner_address"] = hex20(seed + 8)
        record["source_bridge_config_hash"] = hex32(seed + 9)
        record["_comment_tron_source_bridge_address"] = record[
            "source_bridge_emitter_address"
        ]
        record["_comment_tron_source_bridge_code_hash"] = record[
            "source_bridge_emitter_code_hash"
        ]
        record["_comment_tron_source_bridge_runtime_bytecode_hex"] = (
            "0x" + runtime.hex()
        )
        record["_comment_tron_source_bridge_config_hash"] = record[
            "source_bridge_config_hash"
        ]
    return record


def source_deployment(module, material, profile, seed):
    record = {
        key: value for key, value in material.items() if key != "placeholder_material"
    }
    record.update(
        {
            "target_domain": module.SCCP_DOMAIN_SORA,
            "adapter_proof_family": module.SCCP_PROOF_FAMILY_STARK_FRI,
            "adapter_verifier_vk_hash": hex32(seed + 10),
            "deployment_receipt_hash": hex32(seed + 11),
        }
    )
    if profile.solana_full_light_client_audit_required:
        record.update(
            {
                "solana_tower_replay_verifier_hash": hex32(seed + 12),
                "solana_full_accountsdb_lattice_verifier_hash": hex32(seed + 13),
                "solana_bank_fork_choice_verifier_hash": hex32(seed + 14),
                "solana_full_light_client_gate_hash": hex32(seed + 15),
            }
        )
    if profile.ton_full_light_client_audit_required:
        record.update(
            {
                "ton_masterchain_config_verifier_hash": hex32(seed + 16),
                "ton_validator_set_transition_verifier_hash": hex32(seed + 17),
                "ton_shard_accounts_dictionary_verifier_hash": hex32(seed + 18),
                "ton_full_light_client_gate_hash": hex32(seed + 19),
            }
        )
    return record


def destination_rollout(module, profile, material, seed):
    record = {
        "version": 1,
        "domain": profile.domain,
        "chain": profile.chain,
        "verifier_plan": profile.destination_verifier_plan,
        "immutable_verifier_ready": True,
        "anchors_ready": True,
        "verifier_identity": verifier_identity(profile, seed),
        "verifier_code_hash": hex32(seed + 20),
        "anchor_id": profile.destination_anchor_id,
        "blockers": [],
    }
    if profile.destination_verifier_key_hash_required:
        record["verifier_key_hash"] = hex32(seed + 21)
    if profile.chain in ("eth", "bsc"):
        evm_module = module._load_sibling_module("sccp_evm_destination_evidence.py")
        bridge_runtime = bytes([0x60, 0x80, 0x60, seed & 0xFF, 0x52])
        verifier_runtime = bytes([0x60, 0x80, 0x60, (seed + 1) & 0xFF, 0x52])
        record["verifier_code_hash"] = (
            "0x" + evm_module.runtime_bytecode_hash(verifier_runtime).hex()
        )
        record["_comment_evm_rpc_chain_id"] = str(
            module.EVM_EXPECTED_RPC_CHAIN_IDS[profile.domain]
        )
        record["_comment_evm_block_tag"] = (
            "finalized" if profile.chain == "eth" else "latest"
        )
        record["_comment_evm_bridge_code_hash"] = (
            "0x" + evm_module.runtime_bytecode_hash(bridge_runtime).hex()
        )
        record["_comment_evm_bridge_runtime_bytecode_hex"] = (
            "0x" + bridge_runtime.hex()
        )
        record["_comment_evm_verifier_code_hash"] = record["verifier_code_hash"]
        record["_comment_evm_verifier_runtime_bytecode_hex"] = (
            "0x" + verifier_runtime.hex()
        )
        record["_comment_evm_verifier_key_hash"] = record["verifier_key_hash"]
        record["_comment_evm_verifier_backend_hash"] = (
            "0x" + evm_module.evm_verifier_backend_hash().hex()
        )
        record["_comment_evm_proof_family_hash"] = (
            "0x" + evm_module.evm_proof_family_hash().hex()
        )
        record["destination_network_id"] = (
            "0x" + evm_module.evm_mainnet_network_id_for_domain(profile.domain).hex()
        )
        record["destination_bridge_address"] = hex20(seed + 24)
        record["destination_binding_hash"] = "0x" + evm_module.evm_destination_binding_hash(
            network_id=raw_hex(record["destination_network_id"]),
            source_domain=module.SCCP_DOMAIN_SORA,
            target_domain=profile.domain,
            verifier_address=raw_hex(record["verifier_identity"]),
            bridge_address=raw_hex(record["destination_bridge_address"]),
            verifier_code_hash=raw_hex(record["verifier_code_hash"]),
            verifier_key_hash=raw_hex(record["verifier_key_hash"]),
        ).hex()
        record["destination_binding_key"] = evm_module.evm_destination_binding_key(
            network_id=raw_hex(record["destination_network_id"]),
            source_domain=module.SCCP_DOMAIN_SORA,
            target_domain=profile.domain,
            verifier_address=raw_hex(record["verifier_identity"]),
            bridge_address=raw_hex(record["destination_bridge_address"]),
            verifier_code_hash=raw_hex(record["verifier_code_hash"]),
            verifier_key_hash=raw_hex(record["verifier_key_hash"]),
        )
    elif profile.chain == "sol":
        solana_module = module._load_sibling_module(
            "sccp_solana_destination_evidence.py"
        )
        program_bytes = bytes([0x7F, 0x45, 0x4C, 0x46, seed & 0xFF, 0x01])
        record["verifier_code_hash"] = (
            "0x" + solana_module.solana_verifier_program_code_hash(program_bytes).hex()
        )
        record["destination_binding_key"] = solana_module.solana_destination_binding_key()
        record["destination_binding_hash"] = (
            "0x" + solana_module.solana_destination_binding_hash().hex()
        )
        record["_comment_solana_rpc_commitment"] = "finalized"
        record["_comment_solana_program_owner"] = module.SOLANA_UPGRADEABLE_LOADER_ID
        record["_comment_solana_programdata_owner"] = module.SOLANA_UPGRADEABLE_LOADER_ID
        record["_comment_solana_program_immutable"] = "true"
        record["_comment_solana_program_account_data_len"] = "36"
        record["_comment_solana_programdata_address"] = base58_encode(
            bytes([seed + 25]) * 32
        )
        record["_comment_solana_programdata_slot"] = str(1000 + seed)
        record["_comment_solana_expected_programdata_slot"] = str(1000 + seed)
        record["_comment_solana_program_account_context_slot"] = str(2000 + seed)
        record["_comment_solana_programdata_account_context_slot"] = str(2000 + seed)
        program_account_data = solana_module.solana_upgradeable_program_account_data(
            record["_comment_solana_programdata_address"]
        )
        programdata_metadata = solana_module.solana_immutable_programdata_metadata(
            1000 + seed
        )
        record["_comment_solana_program_account_data_base64"] = base64.b64encode(
            program_account_data
        ).decode("ascii")
        record["_comment_solana_programdata_metadata_blake2b256"] = (
            "0x" + hashlib.blake2b(programdata_metadata, digest_size=32).hexdigest()
        )
        record["_comment_solana_programdata_metadata_base64"] = base64.b64encode(
            programdata_metadata
        ).decode("ascii")
        record["_comment_solana_programdata_code_hash"] = record["verifier_code_hash"]
        record["_comment_solana_programdata_executable_base64"] = base64.b64encode(
            program_bytes
        ).decode("ascii")
    elif profile.chain == "ton":
        ton_module = module._load_sibling_module("sccp_ton_destination_evidence.py")
        record["verifier_code_hash"] = "0x" + TON_CODE_BOC_ROOT_HASH
        record["destination_binding_key"] = ton_module.ton_destination_binding_key()
        record["destination_binding_hash"] = (
            "0x" + ton_module.ton_destination_binding_hash().hex()
        )
        record["_comment_ton_account_status"] = "active"
        record["_comment_ton_account_state_hash"] = hex32(seed + 25)
        record["_comment_ton_last_transaction_lt"] = str(2000 + seed)
        record["_comment_ton_last_transaction_hash"] = hex32(seed + 26)
        record["_comment_ton_code_hash"] = record["verifier_code_hash"]
        record["_comment_ton_code_boc_root_hash"] = record["verifier_code_hash"]
        record["_comment_ton_code_boc_base64"] = TON_CODE_BOC_BASE64
        record["_comment_ton_code_boc_hash_matches"] = "true"
        record["ton_account_status"] = record["_comment_ton_account_status"]
        record["ton_account_state_hash"] = record["_comment_ton_account_state_hash"]
        record["ton_last_transaction_lt"] = record["_comment_ton_last_transaction_lt"]
        record["ton_last_transaction_hash"] = record["_comment_ton_last_transaction_hash"]
        record["ton_verifier_code_boc_root_hash"] = record[
            "_comment_ton_code_boc_root_hash"
        ]
        record["ton_verifier_code_boc"] = "0x" + TON_CODE_BOC_HEX
    elif profile.chain == "tron":
        tron_module = module._load_sibling_module("sccp_tron_source_bridge_evidence.py")
        runtime = bytes([0x60, 0x80, 0x60, seed & 0xFF, 0x56])
        record["verifier_code_hash"] = (
            "0x" + tron_module.runtime_bytecode_hash(runtime).hex()
        )
        record["destination_network_id"] = material["source_bridge_network_id"]
        record["_comment_tron_destination_verifier_address"] = record[
            "verifier_identity"
        ]
        record["_comment_tron_destination_verifier_code_hash"] = record[
            "verifier_code_hash"
        ]
        record["_comment_tron_destination_verifier_runtime_bytecode_hex"] = (
            "0x" + runtime.hex()
        )
        record["_comment_tron_destination_verifier_key_hash"] = record[
            "verifier_key_hash"
        ]
        record["_comment_tron_destination_verifier_backend_hash"] = (
            "0x"
            + tron_module._keccak_256(
                tron_module.TRON_GROTH16_BACKEND.encode("utf-8")
            ).hex()
        )
        record["_comment_tron_destination_proof_family_hash"] = (
            "0x"
            + tron_module._keccak_256(
                tron_module.SCCP_PROOF_FAMILY_STARK_FRI.encode("utf-8")
            ).hex()
        )
        binding_args = {
            "network_id": raw_hex(material["source_bridge_network_id"]),
            "source_domain": module.SCCP_DOMAIN_SORA,
            "target_domain": profile.domain,
            "verifier_address": record["verifier_identity"],
            "verifier_code_hash": raw_hex(record["verifier_code_hash"]),
            "verifier_key_hash": raw_hex(record["verifier_key_hash"]),
            "proof_family": module.SCCP_PROOF_FAMILY_STARK_FRI,
        }
        record["destination_binding_hash"] = (
            "0x" + tron_module.tron_destination_binding_hash(**binding_args).hex()
        )
        record["destination_binding_key"] = tron_module.tron_destination_binding_key(
            **binding_args
        )
    return record


def route_allowlist(
    module,
    profile,
    seed,
    source_record_hashes=None,
    destination_binding_hash=None,
    destination=None,
):
    route_hash = hex32(seed + 22)
    if source_record_hashes is not None and destination_binding_hash is not None:
        route_hash = "0x" + module.route_allowlist_hash_for_lane_evidence(
            profile,
            raw_hex(source_record_hashes["source_verifier_material_hash"]),
            raw_hex(source_record_hashes["source_adapter_engine_deployment_hash"]),
            raw_hex(destination_binding_hash),
        ).hex()
    route = {
        "version": 1,
        "domain": profile.domain,
        "chain": profile.chain,
        "activation_policy": "GovernanceAllowlist",
        "route_allowlist_id": profile.route_allowlist_id,
        "route_allowlist_hash": route_hash,
        "routes_allowlisted": True,
        "blockers": [],
        "_comment_route_canary_status": "passed",
        "_comment_route_canary_evidence_hash": hex32(seed + 27),
        "_comment_route_canary_route_allowlist_hash": route_hash,
        "_comment_route_canary_destination_binding_hash": destination_binding_hash,
    }
    if (
        profile.chain in ("eth", "bsc")
        and destination is not None
        and destination_binding_hash is not None
    ):
        evm_module = module._load_sibling_module("sccp_evm_destination_evidence.py")
        transaction_hash = bytes([seed + 28]) * 32
        message_id = bytes([seed + 29]) * 32
        statement_hash = bytes([seed + 30]) * 32
        commitment_root = bytes([seed + 31]) * 32
        call_data_sha256 = bytes([seed + 34]) * 32
        payload_hash = bytes([seed + 35]) * 32
        finality_height = bytes([seed + 36]) * 32
        finality_block_hash = bytes([seed + 37]) * 32
        receipt_block_number = 10_000 + seed
        receipt_block_hash = bytes([seed + 38]) * 32
        block_receipts_root = bytes([seed + 39]) * 32
        canary_hash = evm_module.evm_route_canary_transaction_evidence_hash(
            route_allowlist_hash=raw_hex(route_hash),
            bridge_address=raw_hex(destination["destination_bridge_address"]),
            transaction_hash=transaction_hash,
            log_index=0,
            receipt_block_number=receipt_block_number,
            receipt_block_hash=receipt_block_hash,
            block_receipts_root=block_receipts_root,
            call_data_sha256=call_data_sha256,
            message_id=message_id,
            payload_hash=payload_hash,
            source_domain=module.SCCP_DOMAIN_SORA,
            target_domain=profile.domain,
            commitment_root=commitment_root,
            finality_height=finality_height,
            finality_block_hash=finality_block_hash,
            statement_hash=statement_hash,
            proof_version=1,
            proof_source_domain=module.SCCP_DOMAIN_SORA,
            destination_binding_hash=raw_hex(destination_binding_hash),
            verifier_backend_hash=raw_hex(
                destination["_comment_evm_verifier_backend_hash"]
            ),
            proof_family_hash=raw_hex(destination["_comment_evm_proof_family_hash"]),
            network_id=raw_hex(destination["destination_network_id"]),
            used_message_proof=True,
            receipt_block_finalized=True,
        )
        route["_comment_route_canary_evidence_hash"] = "0x" + canary_hash.hex()
        route["_comment_evm_route_canary_transaction_hash"] = (
            "0x" + transaction_hash.hex()
        )
        route["_comment_evm_route_canary_transaction_block_number"] = str(
            receipt_block_number
        )
        route["_comment_evm_route_canary_transaction_block_hash"] = (
            "0x" + receipt_block_hash.hex()
        )
        route["_comment_evm_route_canary_log_index"] = "0"
        route["_comment_evm_route_canary_receipt_block_number"] = str(
            receipt_block_number
        )
        route["_comment_evm_route_canary_receipt_block_hash"] = (
            "0x" + receipt_block_hash.hex()
        )
        route["_comment_evm_route_canary_block_receipts_root"] = (
            "0x" + block_receipts_root.hex()
        )
        route["_comment_evm_route_canary_call_data_sha256"] = (
            "0x" + call_data_sha256.hex()
        )
        route["_comment_evm_route_canary_message_id"] = "0x" + message_id.hex()
        route["_comment_evm_route_canary_payload_hash"] = "0x" + payload_hash.hex()
        route["_comment_evm_route_canary_target_domain"] = str(profile.domain)
        route["_comment_evm_route_canary_statement_hash"] = (
            "0x" + statement_hash.hex()
        )
        route["_comment_evm_route_canary_commitment_root"] = (
            "0x" + commitment_root.hex()
        )
        route["_comment_evm_route_canary_finality_height"] = (
            "0x" + finality_height.hex()
        )
        route["_comment_evm_route_canary_finality_block_hash"] = (
            "0x" + finality_block_hash.hex()
        )
        route["_comment_evm_route_canary_proof_version"] = "1"
        route["_comment_evm_route_canary_proof_source_domain"] = str(
            module.SCCP_DOMAIN_SORA
        )
        route["_comment_evm_route_canary_used_message_proof"] = "true"
        route["_comment_evm_route_canary_receipt_block_finalized"] = "true"
    elif profile.chain == "tron" and destination is not None:
        tron_module = module._load_sibling_module("sccp_tron_source_bridge_evidence.py")
        live_module = module._load_sibling_module("sccp_tron_live_evidence.py")
        transaction_id = bytes([seed + 28]) * 32
        message_id = bytes([seed + 29]) * 32
        statement_hash = bytes([seed + 30]) * 32
        commitment_root = bytes([seed + 31]) * 32
        call_data_sha256 = bytes([seed + 34]) * 32
        payload_hash = bytes([seed + 35]) * 32
        finality_height = bytes([seed + 36]) * 32
        finality_block_hash = bytes([seed + 37]) * 32
        transaction_owner_address = b"\x41" + (bytes([seed + 33]) * 20)
        block_number = 10_000 + seed
        block_timestamp = 1_700_000 + seed
        signature_sha256 = bytes([seed + 32]) * 32
        signature_recovered_address = transaction_owner_address
        route_hash_raw = raw_hex(route_hash)
        destination_binding_raw = raw_hex(destination_binding_hash)
        canary_hash = live_module._tron_route_canary_transaction_evidence_hash(
            route_allowlist_hash=route_hash_raw,
            transaction_id=transaction_id,
            transaction_owner_address=transaction_owner_address,
            block_number=block_number,
            block_timestamp=block_timestamp,
            log_index=0,
            verifier_address20=tron_module.parse_tron_address(
                destination["verifier_identity"],
                label="TRON destination verifier",
            ),
            call_data_sha256=call_data_sha256,
            message_id=message_id,
            source_domain=module.SCCP_DOMAIN_SORA,
            target_domain=module.SCCP_DOMAIN_TRON,
            payload_hash=payload_hash,
            commitment_root=commitment_root,
            finality_height=finality_height,
            finality_block_hash=finality_block_hash,
            statement_hash=statement_hash,
            proof_version=1,
            proof_source_domain=module.SCCP_DOMAIN_SORA,
            destination_binding_hash=destination_binding_raw,
            verifier_backend_hash=raw_hex(
                destination["_comment_tron_destination_verifier_backend_hash"]
            ),
            proof_family_hash=raw_hex(
                destination["_comment_tron_destination_proof_family_hash"]
            ),
            network_id=raw_hex(destination["destination_network_id"]),
            used_message_proof=True,
            raw_data_owner_matches_transaction=True,
            signature_sha256=signature_sha256,
            signature_recovered_address=signature_recovered_address,
            signature_recovers_to_owner=True,
        )
        route["_comment_route_canary_evidence_hash"] = "0x" + canary_hash.hex()
        route["_comment_tron_route_canary_transaction_id"] = (
            "0x" + transaction_id.hex()
        )
        route["_comment_tron_route_canary_transaction_owner_address"] = (
            "0x" + transaction_owner_address.hex()
        )
        route["_comment_tron_route_canary_block_number"] = str(block_number)
        route["_comment_tron_route_canary_block_timestamp"] = str(block_timestamp)
        route["_comment_tron_route_canary_log_index"] = "0"
        route["_comment_tron_route_canary_message_id"] = "0x" + message_id.hex()
        route["_comment_tron_route_canary_call_data_sha256"] = (
            "0x" + call_data_sha256.hex()
        )
        route["_comment_tron_route_canary_payload_hash"] = (
            "0x" + payload_hash.hex()
        )
        route["_comment_tron_route_canary_target_domain"] = str(module.SCCP_DOMAIN_TRON)
        route["_comment_tron_route_canary_statement_hash"] = (
            "0x" + statement_hash.hex()
        )
        route["_comment_tron_route_canary_commitment_root"] = (
            "0x" + commitment_root.hex()
        )
        route["_comment_tron_route_canary_finality_height"] = (
            "0x" + finality_height.hex()
        )
        route["_comment_tron_route_canary_finality_block_hash"] = (
            "0x" + finality_block_hash.hex()
        )
        route["_comment_tron_route_canary_proof_version"] = "1"
        route["_comment_tron_route_canary_proof_source_domain"] = str(
            module.SCCP_DOMAIN_SORA
        )
        route["_comment_tron_route_canary_used_message_proof"] = "true"
        route["_comment_tron_route_canary_raw_data_owner_matches_transaction"] = "true"
        route["_comment_tron_route_canary_signature_sha256"] = (
            "0x" + signature_sha256.hex()
        )
        route["_comment_tron_route_canary_signature_recovered_address"] = (
            "0x" + signature_recovered_address.hex()
        )
        route["_comment_tron_route_canary_signature_recovers_to_owner"] = "true"
    elif (
        profile.chain == "ton"
        and destination is not None
        and source_record_hashes is not None
        and destination_binding_hash is not None
    ):
        ton_module = module._load_sibling_module("sccp_ton_destination_evidence.py")
        account_state_hash = destination["ton_account_state_hash"]
        last_transaction_lt = destination["ton_last_transaction_lt"]
        last_transaction_hash = destination["ton_last_transaction_hash"]
        canary_hash = ton_module.ton_route_canary_evidence_hash(
            route_allowlist_hash=raw_hex(route_hash),
            destination_binding_hash=raw_hex(destination_binding_hash),
            source_verifier_material_hash=raw_hex(
                source_record_hashes["source_verifier_material_hash"]
            ),
            source_adapter_engine_deployment_hash=raw_hex(
                source_record_hashes["source_adapter_engine_deployment_hash"]
            ),
            verifier_contract_address=destination["verifier_identity"],
            verifier_code_hash=raw_hex(destination["verifier_code_hash"]),
            account_status=destination["_comment_ton_account_status"],
            account_state_hash=raw_hex(account_state_hash),
            last_transaction_lt=last_transaction_lt,
            last_transaction_hash=raw_hex(last_transaction_hash),
            verifier_code_boc_root_hash=raw_hex(
                destination["ton_verifier_code_boc_root_hash"]
            ),
        )
        route["_comment_route_canary_evidence_hash"] = "0x" + canary_hash.hex()
        route["_comment_ton_route_canary_account_state_hash"] = account_state_hash
        route["_comment_ton_route_canary_last_transaction_lt"] = last_transaction_lt
        route["_comment_ton_route_canary_last_transaction_hash"] = last_transaction_hash
        route["ton_route_canary_account_state_hash"] = account_state_hash
        route["ton_route_canary_last_transaction_lt"] = last_transaction_lt
        route["ton_route_canary_last_transaction_hash"] = last_transaction_hash
    elif (
        profile.chain == "sol"
        and destination is not None
        and source_record_hashes is not None
        and destination_binding_hash is not None
    ):
        solana_module = module._load_sibling_module(
            "sccp_solana_destination_evidence.py"
        )
        canary_hash = solana_module.solana_route_canary_evidence_hash(
            route_allowlist_hash=raw_hex(route_hash),
            destination_binding_hash=raw_hex(destination_binding_hash),
            source_verifier_material_hash=raw_hex(
                source_record_hashes["source_verifier_material_hash"]
            ),
            source_adapter_engine_deployment_hash=raw_hex(
                source_record_hashes["source_adapter_engine_deployment_hash"]
            ),
            verifier_program_id=destination["verifier_identity"],
            verifier_code_hash=raw_hex(destination["verifier_code_hash"]),
            rpc_commitment=destination["_comment_solana_rpc_commitment"],
            program_owner=destination["_comment_solana_program_owner"],
            programdata_owner=destination["_comment_solana_programdata_owner"],
            program_immutable=(
                destination["_comment_solana_program_immutable"] == "true"
            ),
            program_account_data=base64.b64decode(
                destination["_comment_solana_program_account_data_base64"],
                validate=True,
            ),
            programdata_address=destination["_comment_solana_programdata_address"],
            programdata_slot=int(destination["_comment_solana_programdata_slot"]),
            expected_programdata_slot=int(
                destination["_comment_solana_expected_programdata_slot"]
            ),
            program_account_context_slot=int(
                destination["_comment_solana_program_account_context_slot"]
            ),
            programdata_account_context_slot=int(
                destination["_comment_solana_programdata_account_context_slot"]
            ),
            programdata_metadata=base64.b64decode(
                destination["_comment_solana_programdata_metadata_base64"],
                validate=True,
            ),
            programdata_executable=solana_module.parse_program_bytes_base64(
                destination["_comment_solana_programdata_executable_base64"],
                label="Solana ProgramData executable",
            ),
        )
        route["_comment_route_canary_evidence_hash"] = "0x" + canary_hash.hex()
    return route


def complete_bundle(module):
    records = {name: [] for name in module.SECTION_NAMES}
    for index, domain in enumerate(module.SCCP_CORE_REMOTE_DOMAINS):
        profile = module.LANE_PROFILES[domain]
        seed = 0x10 + index * 0x20
        material = source_material(module, profile, seed)
        if profile.tron_source_bridge_config_required:
            tron_module = module._load_sibling_module("sccp_tron_source_bridge_evidence.py")
            material["source_bridge_config_hash"] = "0x" + tron_module.tron_source_bridge_config_hash(
                bridge_address=raw_hex(material["source_bridge_emitter_address"]),
                network_id=raw_hex(material["source_bridge_network_id"]),
                source_domain=profile.domain,
                target_domain=module.SCCP_DOMAIN_SORA,
                owner_address=raw_hex(material["source_bridge_owner_address"]),
            ).hex()
            material["_comment_tron_source_bridge_config_hash"] = material[
                "source_bridge_config_hash"
            ]
        deployment = source_deployment(module, material, profile, seed)
        if profile.chain == "eth":
            eth_module = module._load_sibling_module("sccp_eth_source_bridge_evidence.py")
            deployment["adapter_verifier_vk_hash"] = (
                "0x" + eth_module.eth_source_adapter_verifier_vk_hash().hex()
            )
            args = module._evm_source_bridge_args(material, deployment)
            deployment["evm_source_gate_hash"] = (
                "0x" + eth_module.eth_source_gate_hash(args).hex()
            )
        if profile.chain == "bsc":
            bsc_module = module._load_sibling_module("sccp_bsc_source_bridge_evidence.py")
            deployment["adapter_verifier_vk_hash"] = (
                "0x" + bsc_module.bsc_source_adapter_verifier_vk_hash().hex()
            )
            args = module._evm_source_bridge_args(material, deployment)
            deployment["evm_source_gate_hash"] = (
                "0x" + bsc_module.bsc_source_gate_hash(args).hex()
            )
        if profile.solana_full_light_client_audit_required:
            solana_module = module._load_sibling_module("sccp_solana_source_state_evidence.py")
            deployment["adapter_verifier_vk_hash"] = (
                "0x" + solana_module.solana_source_adapter_verifier_vk_hash().hex()
            )
            args = module._source_adapter_args(material, deployment)
            args.tower_replay_verifier_hash = raw_hex(
                deployment["solana_tower_replay_verifier_hash"]
            )
            args.full_accountsdb_lattice_verifier_hash = raw_hex(
                deployment["solana_full_accountsdb_lattice_verifier_hash"]
            )
            args.bank_fork_choice_verifier_hash = raw_hex(
                deployment["solana_bank_fork_choice_verifier_hash"]
            )
            deployment["solana_full_light_client_gate_hash"] = (
                "0x" + solana_module.solana_full_light_client_gate_hash(args).hex()
            )
        if profile.ton_full_light_client_audit_required:
            ton_module = module._load_sibling_module("sccp_ton_source_state_evidence.py")
            deployment["adapter_verifier_vk_hash"] = (
                "0x" + ton_module.ton_source_adapter_verifier_vk_hash().hex()
            )
            args = module._source_adapter_args(material, deployment)
            args.masterchain_config_verifier_hash = raw_hex(
                deployment["ton_masterchain_config_verifier_hash"]
            )
            args.validator_set_transition_verifier_hash = raw_hex(
                deployment["ton_validator_set_transition_verifier_hash"]
            )
            args.shard_accounts_dictionary_verifier_hash = raw_hex(
                deployment["ton_shard_accounts_dictionary_verifier_hash"]
            )
            deployment["ton_full_light_client_gate_hash"] = (
                "0x" + ton_module.ton_full_light_client_gate_hash(args).hex()
            )
        if profile.chain == "tron":
            tron_module = module._load_sibling_module("sccp_tron_source_bridge_evidence.py")
            deployment["adapter_verifier_vk_hash"] = (
                "0x" + tron_module.tron_source_adapter_verifier_vk_hash().hex()
            )
            args = module._tron_source_bridge_args(material, deployment)
            deployment["tron_dpos_source_gate_hash"] = (
                "0x"
                + tron_module.tron_dpos_source_gate_hash(
                    args,
                    raw_hex(material["source_bridge_config_hash"]),
                ).hex()
            )
        source_record_hashes = module._canonical_source_record_hashes(
            profile,
            material,
            deployment,
        )
        material["_comment_source_verifier_material_hash"] = source_record_hashes[
            "source_verifier_material_hash"
        ]
        deployment["_comment_source_adapter_engine_deployment_hash"] = (
            source_record_hashes["source_adapter_engine_deployment_hash"]
        )
        records["sccp_source_verifier_materials"].append(material)
        records["sccp_source_adapter_engine_deployments"].append(deployment)
        destination = destination_rollout(module, profile, material, seed)
        records["sccp_destination_rollouts"].append(destination)
        records["sccp_route_allowlists"].append(
            route_allowlist(
                module,
                profile,
                seed,
                source_record_hashes,
                destination["destination_binding_hash"],
                destination=destination,
            )
        )
    return records


def toml_value(value):
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, str):
        return json.dumps(value)
    if isinstance(value, list) and all(isinstance(item, str) for item in value):
        return "[" + ", ".join(json.dumps(item) for item in value) + "]"
    raise TypeError(f"unsupported TOML value: {value!r}")


def test_minimal_toml_rejects_noncanonical_integer_values():
    module = load_evidence_module()
    canonical = module._load_toml_minimal(
        "[[zk.sccp_source_verifier_materials]]\nsource_domain = 1\n",
        label="fixture.toml",
    )
    assert canonical["zk"]["sccp_source_verifier_materials"][0]["source_domain"] == 1

    for value in ("01", "+1", "\u0661", "1_000"):
        try:
            module._load_toml_minimal(
                f"[[zk.sccp_source_verifier_materials]]\nsource_domain = {value}\n",
                label="fixture.toml",
            )
        except ValueError as exc:
            assert "unsupported TOML value" in str(exc)
        else:
            raise AssertionError(f"noncanonical TOML integer {value!r} was accepted")


def destination_comment_lines(entry):
    if entry["chain"] in ("eth", "bsc"):
        return [
            "# sccp_evm_rpc_chain_id = "
            + toml_value(entry["_comment_evm_rpc_chain_id"]),
            "# sccp_evm_block_tag = "
            + toml_value(entry["_comment_evm_block_tag"]),
            "# sccp_evm_bridge_runtime_code_hash = "
            + toml_value(entry["_comment_evm_bridge_code_hash"]),
            "# sccp_evm_bridge_runtime_bytecode_hex = "
            + toml_value(entry["_comment_evm_bridge_runtime_bytecode_hex"]),
            "# sccp_evm_verifier_runtime_code_hash = "
            + toml_value(entry["_comment_evm_verifier_code_hash"]),
            "# sccp_evm_verifier_runtime_bytecode_hex = "
            + toml_value(entry["_comment_evm_verifier_runtime_bytecode_hex"]),
            "# sccp_evm_verifier_key_hash = "
            + toml_value(entry["_comment_evm_verifier_key_hash"]),
            "# sccp_evm_verifier_backend_hash = "
            + toml_value(entry["_comment_evm_verifier_backend_hash"]),
            "# sccp_evm_proof_family_hash = "
            + toml_value(entry["_comment_evm_proof_family_hash"]),
            "# sccp_evm_destination_network_id = "
            + toml_value(entry["destination_network_id"]),
            "# sccp_evm_destination_bridge_address = "
            + toml_value(entry["destination_bridge_address"]),
            "# sccp_evm_destination_binding_key = "
            + toml_value(entry["destination_binding_key"]),
            "# sccp_evm_destination_binding_hash = "
            + toml_value(entry["destination_binding_hash"]),
        ]
    if entry["chain"] == "sol":
        return [
            "# sccp_solana_rpc_commitment = "
            + toml_value(entry["_comment_solana_rpc_commitment"]),
            "# sccp_solana_program_owner = "
            + toml_value(entry["_comment_solana_program_owner"]),
            "# sccp_solana_programdata_owner = "
            + toml_value(entry["_comment_solana_programdata_owner"]),
            "# sccp_solana_program_immutable = "
            + toml_value(entry["_comment_solana_program_immutable"]),
            "# sccp_solana_program_account_data_len = "
            + toml_value(entry["_comment_solana_program_account_data_len"]),
            "# sccp_solana_program_account_data_base64 = "
            + toml_value(entry["_comment_solana_program_account_data_base64"]),
            "# sccp_solana_programdata_address = "
            + toml_value(entry["_comment_solana_programdata_address"]),
            "# sccp_solana_programdata_slot = "
            + toml_value(entry["_comment_solana_programdata_slot"]),
            "# sccp_solana_expected_programdata_slot = "
            + toml_value(entry["_comment_solana_expected_programdata_slot"]),
            "# sccp_solana_program_account_context_slot = "
            + toml_value(entry["_comment_solana_program_account_context_slot"]),
            "# sccp_solana_programdata_account_context_slot = "
            + toml_value(entry["_comment_solana_programdata_account_context_slot"]),
            "# sccp_solana_programdata_metadata_blake2b256 = "
            + toml_value(entry["_comment_solana_programdata_metadata_blake2b256"]),
            "# sccp_solana_programdata_metadata_base64 = "
            + toml_value(entry["_comment_solana_programdata_metadata_base64"]),
            "# sccp_solana_programdata_executable_blake2b256 = "
            + toml_value(entry["_comment_solana_programdata_code_hash"]),
            "# sccp_solana_programdata_executable_base64 = "
            + toml_value(entry["_comment_solana_programdata_executable_base64"]),
            "# sccp_solana_destination_binding_hash = "
            + toml_value(entry["destination_binding_hash"]),
        ]
    if entry["chain"] == "ton":
        return [
            "# sccp_ton_account_status = "
            + toml_value(entry["_comment_ton_account_status"]),
            "# sccp_ton_account_state_hash = "
            + toml_value(entry["_comment_ton_account_state_hash"]),
            "# sccp_ton_last_transaction_lt = "
            + toml_value(entry["_comment_ton_last_transaction_lt"]),
            "# sccp_ton_last_transaction_hash = "
            + toml_value(entry["_comment_ton_last_transaction_hash"]),
            "# sccp_ton_code_hash = "
            + toml_value(entry["_comment_ton_code_hash"]),
            "# sccp_ton_code_boc_root_hash = "
            + toml_value(entry["_comment_ton_code_boc_root_hash"]),
            "# sccp_ton_code_boc_base64 = "
            + toml_value(entry["_comment_ton_code_boc_base64"]),
            "# sccp_ton_code_boc_hash_matches = "
            + toml_value(entry["_comment_ton_code_boc_hash_matches"]),
            "# sccp_ton_destination_binding_hash = "
            + toml_value(entry["destination_binding_hash"])
        ]
    if entry["chain"] == "tron":
        return [
            "# sccp_tron_destination_verifier_address = "
            + toml_value(entry["_comment_tron_destination_verifier_address"]),
            "# sccp_tron_destination_verifier_runtime_code_hash = "
            + toml_value(entry["_comment_tron_destination_verifier_code_hash"]),
            "# sccp_tron_destination_verifier_runtime_bytecode_hex = "
            + toml_value(
                entry["_comment_tron_destination_verifier_runtime_bytecode_hex"]
            ),
            "# sccp_tron_destination_verifier_key_hash = "
            + toml_value(entry["_comment_tron_destination_verifier_key_hash"]),
            "# sccp_tron_destination_verifier_backend_hash = "
            + toml_value(entry["_comment_tron_destination_verifier_backend_hash"]),
            "# sccp_tron_destination_proof_family_hash = "
            + toml_value(entry["_comment_tron_destination_proof_family_hash"]),
            "# sccp_tron_destination_binding_hash = "
            + toml_value(entry["destination_binding_hash"]),
            "# sccp_tron_destination_binding_key = "
            + toml_value(entry["destination_binding_key"]),
        ]
    return []


SOURCE_VERIFIER_MATERIAL_COMMENT_KEYS = {
    "eth": "sccp_eth_source_verifier_material_hash",
    "bsc": "sccp_bsc_source_verifier_material_hash",
    "sol": "sccp_solana_source_verifier_material_hash",
    "ton": "sccp_ton_source_verifier_material_hash",
    "tron": "sccp_tron_source_verifier_material_hash",
}
SOURCE_DEPLOYMENT_COMMENT_KEYS = {
    "eth": "sccp_eth_source_adapter_engine_deployment_hash",
    "bsc": "sccp_bsc_source_adapter_engine_deployment_hash",
    "sol": "sccp_solana_source_adapter_engine_deployment_hash",
    "ton": "sccp_ton_source_adapter_engine_deployment_hash",
    "tron": "sccp_tron_source_adapter_engine_deployment_hash",
}


def source_material_comment_lines(entry):
    comments = []
    source_hash_key = SOURCE_VERIFIER_MATERIAL_COMMENT_KEYS[entry["source_chain"]]
    if "_comment_source_verifier_material_hash" in entry:
        comments.append(
            "# "
            + source_hash_key
            + " = "
            + toml_value(entry["_comment_source_verifier_material_hash"])
        )
    if entry["source_chain"] == "tron":
        comments.extend(
            [
            "# sccp_tron_source_bridge_address = "
            + toml_value(entry["_comment_tron_source_bridge_address"]),
            "# sccp_tron_source_bridge_runtime_code_hash = "
            + toml_value(entry["_comment_tron_source_bridge_code_hash"]),
            "# sccp_tron_source_bridge_runtime_bytecode_hex = "
            + toml_value(entry["_comment_tron_source_bridge_runtime_bytecode_hex"]),
            "# sccp_tron_source_bridge_config_hash = "
            + toml_value(entry["_comment_tron_source_bridge_config_hash"]),
            ]
        )
        return comments
    if entry["source_chain"] not in ("eth", "bsc"):
        return comments
    comments.extend(
        [
        "# sccp_evm_source_rpc_chain_id = "
        + toml_value(entry["_comment_evm_source_rpc_chain_id"]),
        "# sccp_evm_source_block_tag = "
        + toml_value(entry["_comment_evm_source_block_tag"]),
        "# sccp_evm_source_bridge_address = "
        + toml_value(entry["_comment_evm_source_bridge_address"]),
        "# sccp_evm_source_bridge_runtime_code_hash = "
        + toml_value(entry["_comment_evm_source_bridge_code_hash"]),
        "# sccp_evm_source_bridge_runtime_bytecode_hex = "
        + toml_value(entry["_comment_evm_source_bridge_runtime_bytecode_hex"]),
        "# sccp_evm_source_deployment_transaction_hash = "
        + toml_value(entry["_comment_evm_source_deployment_transaction_hash"]),
        "# sccp_evm_source_deployment_transaction_block_hash = "
        + toml_value(entry["_comment_evm_source_deployment_transaction_block_hash"]),
        "# sccp_evm_source_deployment_transaction_block_number = "
        + toml_value(entry["_comment_evm_source_deployment_transaction_block_number"]),
        "# sccp_evm_source_deployment_transaction_input_sha256 = "
        + toml_value(entry["_comment_evm_source_deployment_transaction_input_sha256"]),
        "# sccp_evm_source_deployment_receipt_status = "
        + toml_value(entry["_comment_evm_source_deployment_receipt_status"]),
        "# sccp_evm_source_deployment_contract_address = "
        + toml_value(entry["_comment_evm_source_deployment_contract_address"]),
        "# sccp_evm_source_deployment_block_hash = "
        + toml_value(entry["_comment_evm_source_deployment_block_hash"]),
        "# sccp_evm_source_deployment_block_number = "
        + toml_value(entry["_comment_evm_source_deployment_block_number"]),
        "# sccp_evm_source_deployment_block_receipts_root = "
        + toml_value(entry["_comment_evm_source_deployment_block_receipts_root"]),
    ]
    )
    return comments


def source_deployment_comment_lines(entry):
    chain = entry["source_chain"]
    if "_comment_source_adapter_engine_deployment_hash" not in entry:
        return []
    comments = [
        "# "
        + SOURCE_DEPLOYMENT_COMMENT_KEYS[chain]
        + " = "
        + toml_value(entry["_comment_source_adapter_engine_deployment_hash"])
    ]
    return comments


def route_comment_lines(entry):
    comments = [
        "# sccp_route_canary_status = "
        + toml_value(entry["_comment_route_canary_status"]),
        "# sccp_route_canary_evidence_hash = "
        + toml_value(entry["_comment_route_canary_evidence_hash"]),
        "# sccp_route_canary_route_allowlist_hash = "
        + toml_value(entry["_comment_route_canary_route_allowlist_hash"]),
        "# sccp_route_canary_destination_binding_hash = "
        + toml_value(entry["_comment_route_canary_destination_binding_hash"]),
    ]
    optional_keys = (
        (
            "sccp_evm_route_canary_transaction_hash",
            "_comment_evm_route_canary_transaction_hash",
        ),
        (
            "sccp_evm_route_canary_transaction_block_number",
            "_comment_evm_route_canary_transaction_block_number",
        ),
        (
            "sccp_evm_route_canary_transaction_block_hash",
            "_comment_evm_route_canary_transaction_block_hash",
        ),
        ("sccp_evm_route_canary_log_index", "_comment_evm_route_canary_log_index"),
        (
            "sccp_evm_route_canary_receipt_block_number",
            "_comment_evm_route_canary_receipt_block_number",
        ),
        (
            "sccp_evm_route_canary_receipt_block_hash",
            "_comment_evm_route_canary_receipt_block_hash",
        ),
        (
            "sccp_evm_route_canary_block_receipts_root",
            "_comment_evm_route_canary_block_receipts_root",
        ),
        (
            "sccp_evm_route_canary_call_data_sha256",
            "_comment_evm_route_canary_call_data_sha256",
        ),
        ("sccp_evm_route_canary_message_id", "_comment_evm_route_canary_message_id"),
        (
            "sccp_evm_route_canary_payload_hash",
            "_comment_evm_route_canary_payload_hash",
        ),
        (
            "sccp_evm_route_canary_target_domain",
            "_comment_evm_route_canary_target_domain",
        ),
        (
            "sccp_evm_route_canary_statement_hash",
            "_comment_evm_route_canary_statement_hash",
        ),
        (
            "sccp_evm_route_canary_commitment_root",
            "_comment_evm_route_canary_commitment_root",
        ),
        (
            "sccp_evm_route_canary_finality_height",
            "_comment_evm_route_canary_finality_height",
        ),
        (
            "sccp_evm_route_canary_finality_block_hash",
            "_comment_evm_route_canary_finality_block_hash",
        ),
        (
            "sccp_evm_route_canary_proof_version",
            "_comment_evm_route_canary_proof_version",
        ),
        (
            "sccp_evm_route_canary_proof_source_domain",
            "_comment_evm_route_canary_proof_source_domain",
        ),
        (
            "sccp_evm_route_canary_used_message_proof",
            "_comment_evm_route_canary_used_message_proof",
        ),
        (
            "sccp_evm_route_canary_receipt_block_finalized",
            "_comment_evm_route_canary_receipt_block_finalized",
        ),
        (
            "sccp_tron_route_canary_transaction_id",
            "_comment_tron_route_canary_transaction_id",
        ),
        (
            "sccp_tron_route_canary_transaction_owner_address",
            "_comment_tron_route_canary_transaction_owner_address",
        ),
        (
            "sccp_tron_route_canary_block_number",
            "_comment_tron_route_canary_block_number",
        ),
        (
            "sccp_tron_route_canary_block_timestamp",
            "_comment_tron_route_canary_block_timestamp",
        ),
        ("sccp_tron_route_canary_log_index", "_comment_tron_route_canary_log_index"),
        ("sccp_tron_route_canary_message_id", "_comment_tron_route_canary_message_id"),
        (
            "sccp_tron_route_canary_call_data_sha256",
            "_comment_tron_route_canary_call_data_sha256",
        ),
        (
            "sccp_tron_route_canary_payload_hash",
            "_comment_tron_route_canary_payload_hash",
        ),
        (
            "sccp_tron_route_canary_target_domain",
            "_comment_tron_route_canary_target_domain",
        ),
        (
            "sccp_tron_route_canary_statement_hash",
            "_comment_tron_route_canary_statement_hash",
        ),
        (
            "sccp_tron_route_canary_commitment_root",
            "_comment_tron_route_canary_commitment_root",
        ),
        (
            "sccp_tron_route_canary_finality_height",
            "_comment_tron_route_canary_finality_height",
        ),
        (
            "sccp_tron_route_canary_finality_block_hash",
            "_comment_tron_route_canary_finality_block_hash",
        ),
        (
            "sccp_tron_route_canary_proof_version",
            "_comment_tron_route_canary_proof_version",
        ),
        (
            "sccp_tron_route_canary_proof_source_domain",
            "_comment_tron_route_canary_proof_source_domain",
        ),
        (
            "sccp_tron_route_canary_used_message_proof",
            "_comment_tron_route_canary_used_message_proof",
        ),
        (
            "sccp_tron_route_canary_raw_data_owner_matches_transaction",
            "_comment_tron_route_canary_raw_data_owner_matches_transaction",
        ),
        (
            "sccp_tron_route_canary_signature_sha256",
            "_comment_tron_route_canary_signature_sha256",
        ),
        (
            "sccp_tron_route_canary_signature_recovered_address",
            "_comment_tron_route_canary_signature_recovered_address",
        ),
        (
            "sccp_tron_route_canary_signature_recovers_to_owner",
            "_comment_tron_route_canary_signature_recovers_to_owner",
        ),
        (
            "sccp_ton_route_canary_account_state_hash",
            "_comment_ton_route_canary_account_state_hash",
        ),
        (
            "sccp_ton_route_canary_last_transaction_lt",
            "_comment_ton_route_canary_last_transaction_lt",
        ),
        (
            "sccp_ton_route_canary_last_transaction_hash",
            "_comment_ton_route_canary_last_transaction_hash",
        ),
    )
    for comment_key, record_key in optional_keys:
        if record_key in entry:
            comments.append("# " + comment_key + " = " + toml_value(entry[record_key]))
    return comments


def route_canary_comments(route_allowlist_hash, destination_binding_hash, seed):
    return "\n".join(
        [
            "# sccp_route_canary_status = " + toml_value("passed"),
            "# sccp_route_canary_evidence_hash = " + toml_value(hex32(seed)),
            "# sccp_route_canary_route_allowlist_hash = "
            + toml_value("0x" + route_allowlist_hash.hex()),
            "# sccp_route_canary_destination_binding_hash = "
            + toml_value("0x" + destination_binding_hash.hex()),
        ]
    )


def attach_route_canary_comments(
    rendered_toml,
    *,
    route_allowlist_hash,
    destination_binding_hash,
    seed,
):
    return rendered_toml.replace(
        "[[zk.sccp_route_allowlists]]",
        route_canary_comments(
            route_allowlist_hash,
            destination_binding_hash,
            seed,
        )
        + "\n[[zk.sccp_route_allowlists]]",
        1,
    )


def render_records(records):
    lines = []
    for section, entries in records.items():
        for entry in entries:
            if section == "sccp_source_verifier_materials":
                lines.extend(source_material_comment_lines(entry))
            if section == "sccp_source_adapter_engine_deployments":
                lines.extend(source_deployment_comment_lines(entry))
            if section == "sccp_destination_rollouts":
                lines.extend(destination_comment_lines(entry))
            if section == "sccp_route_allowlists":
                lines.extend(route_comment_lines(entry))
            lines.append(f"[[zk.{section}]]")
            for key, value in entry.items():
                if key.startswith("_comment_"):
                    continue
                lines.append(f"{key} = {toml_value(value)}")
            lines.append("")
    return "\n".join(lines)


def test_minimal_toml_loader_rejects_duplicate_keys():
    module = load_evidence_module()
    original_tomllib = module.tomllib
    module.tomllib = None
    try:
        try:
            module._load_toml(
                """
[[zk.sccp_destination_rollouts]]
version = 1
version = 1
""",
                label="duplicate.toml",
            )
        except ValueError as exc:
            assert "duplicate.toml:4: duplicate key version" in str(exc)
        else:
            raise AssertionError("fallback TOML loader accepted a duplicate key")
    finally:
        module.tomllib = original_tomllib


def test_all_lanes_minimal_toml_parser_redacts_sensitive_duplicate_keys():
    module = load_evidence_module()
    original_tomllib = module.tomllib
    module.tomllib = None
    try:
        cases = (
            (
                (
                    "[[zk.sccp_destination_rollouts]]\n"
                    'secret-token-duplicate-key = "first"\n'
                    'secret-token-duplicate-key = "second"\n'
                ),
                "operator evidence:3: duplicate key with sensitive name",
                ("secret-token-duplicate-key", "first", "second"),
            ),
            (
                (
                    "[[zk.sccp_destination_rollouts]]\n"
                    'recovery_phrase_duplicate_key = "first"\n'
                    'recovery_phrase_duplicate_key = "second"\n'
                ),
                "operator evidence:3: duplicate key with sensitive name",
                ("recovery_phrase_duplicate_key", "first", "second"),
            ),
            (
                (
                    "[[zk.sccp_destination_rollouts]]\n"
                    'mnemonic_duplicate_key = "first"\n'
                    'mnemonic_duplicate_key = "second"\n'
                ),
                "operator evidence:3: duplicate key with sensitive name",
                ("mnemonic_duplicate_key", "first", "second"),
            ),
            (
                (
                    "[[zk.sccp_destination_rollouts]]\n"
                    'route|operator-duplicate-key = "first"\n'
                    'route|operator-duplicate-key = "second"\n'
                ),
                "operator evidence:3: duplicate key with malformed name",
                ("route|operator-duplicate-key", "first", "second"),
            ),
        )
        for toml_text, expected, redacted_tokens in cases:
            try:
                module._load_toml(toml_text, label="operator evidence")
            except ValueError as exc:
                rendered = str(exc)
                assert rendered == expected
                for token in redacted_tokens:
                    assert token not in rendered
                assert exc.__cause__ is None
            else:
                raise AssertionError(
                    "minimal TOML loader accepted unsafe duplicate key"
                )
    finally:
        module.tomllib = original_tomllib


def test_all_lanes_minimal_toml_sensitive_helpers_cover_marker_families():
    module = load_evidence_module()
    helper_cases = (
        (
            module._minimal_toml_duplicate_key_detail,
            "duplicate key with sensitive name",
        ),
        (
            module._toml_unsupported_section_detail,
            "unsupported zk section with sensitive name",
        ),
        (
            module._evidence_unsupported_section_detail,
            "unsupported evidence section with sensitive name",
        ),
        (
            module._unexpected_record_field_detail,
            "unexpected field with sensitive name",
        ),
        (
            module._unexpected_audit_hash_field_detail,
            "source adapter gate audit hashes contains unexpected field with sensitive name",
        ),
    )

    for marker in module.SENSITIVE_MINIMAL_TOML_KEY_MARKERS:
        field_name = f"{marker}_operator_field"
        for helper, expected_detail in helper_cases:
            detail = helper(field_name)
            assert detail == expected_detail, (helper.__name__, marker)
            assert field_name not in detail

    assert module._minimal_toml_duplicate_key_detail(
        "public_operator_field"
    ) == "duplicate key public_operator_field"
    assert module._toml_unsupported_section_detail(
        "public_operator_field"
    ) == "unsupported zk section public_operator_field"
    assert module._evidence_unsupported_section_detail(
        "public_operator_field"
    ) == "unsupported evidence section public_operator_field"
    assert module._unexpected_record_field_detail(
        "public_operator_field"
    ) == "unexpected field public_operator_field"
    assert module._unexpected_audit_hash_field_detail(
        "public_operator_field"
    ) == "source adapter gate audit hashes contains unexpected field: public_operator_field"


def test_all_lanes_minimal_toml_parser_redacts_unsupported_section_names():
    module = load_evidence_module()
    original_tomllib = module.tomllib
    module.tomllib = None
    try:
        cases = (
            (
                "[[zk.secret-token-section]]\nversion = 1\n",
                "operator evidence:1: unsupported zk section with sensitive name",
                "secret-token-section",
            ),
            (
                "[[zk.recovery-phrase-section]]\nversion = 1\n",
                "operator evidence:1: unsupported zk section with sensitive name",
                "recovery-phrase-section",
            ),
            (
                "[[zk.seed_phrase_section]]\nversion = 1\n",
                "operator evidence:1: unsupported zk section with sensitive name",
                "seed_phrase_section",
            ),
            (
                "[[zk.route|operator-section]]\nversion = 1\n",
                "operator evidence:1: unsupported zk section with malformed name",
                "route|operator-section",
            ),
        )
        for toml_text, expected, redacted_token in cases:
            try:
                module._load_toml(toml_text, label="operator evidence")
            except ValueError as exc:
                rendered = str(exc)
                assert rendered == expected
                assert redacted_token not in rendered
                assert exc.__cause__ is None
            else:
                raise AssertionError(
                    "minimal TOML loader accepted unsafe unsupported section"
                )
    finally:
        module.tomllib = original_tomllib


def test_all_lanes_loader_redacts_toml_parser_failures():
    module = load_evidence_module()

    class FakeTomllib:
        class TOMLDecodeError(ValueError):
            pass

        @staticmethod
        def loads(_text):
            raise FakeTomllib.TOMLDecodeError("secret-token parser detail")

    original_tomllib = module.tomllib
    module.tomllib = FakeTomllib
    try:
        try:
            module._load_toml("not = valid", label="operator evidence")
        except ValueError as exc:
            rendered = str(exc)
            assert rendered == "operator evidence: invalid TOML"
            assert "secret-token" not in rendered
            assert "parser detail" not in rendered
            assert exc.__cause__ is None
            assert exc.__suppress_context__ is True
        else:
            raise AssertionError("all-lanes TOML loader accepted parser failure")
    finally:
        module.tomllib = original_tomllib


def test_all_lanes_minimal_toml_parser_redacts_json_exception_causes():
    module = load_evidence_module()
    original_tomllib = module.tomllib
    module.tomllib = None
    try:
        cases = (
            (
                '[[zk.sccp_route_allowlists]]\nroute_id = "secret-token string',
                "operator evidence:2: invalid string",
                "secret-token string",
            ),
            (
                '[[zk.sccp_route_allowlists]]\nallowed_domains = ["secret-token array",',
                "operator evidence:2: invalid array",
                "secret-token array",
            ),
        )
        for toml_text, expected, secret in cases:
            try:
                module._load_toml(toml_text, label="operator evidence")
            except ValueError as exc:
                rendered = str(exc)
                assert rendered == expected
                assert "secret-token" not in rendered
                assert secret not in rendered
                assert exc.__cause__ is None
                assert exc.__suppress_context__ is True
            else:
                raise AssertionError("minimal TOML loader accepted invalid JSON value")
    finally:
        module.tomllib = original_tomllib


def test_all_lanes_metadata_comment_redacts_json_exception_causes():
    module = load_evidence_module()

    try:
        module._route_allowlist_comment_metadata(
            '# sccp_route_canary_status = "secret-token comment\n'
            "[[zk.sccp_route_allowlists]]\n"
            "version = 1\n",
            label="operator evidence",
        )
    except ValueError as exc:
        rendered = str(exc)
        assert rendered == "operator evidence:1: invalid metadata comment"
        assert "secret-token" not in rendered
        assert "secret-token comment" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("all-lanes metadata comment accepted invalid JSON")


def test_all_lanes_cli_redacts_top_level_exception_details(
    tmp_path,
    monkeypatch,
    capsys,
):
    module = load_evidence_module()

    top_level_exception_types = (
        module.argparse.ArgumentTypeError,
        OSError,
        SystemExit,
        RuntimeError,
        TypeError,
        ValueError,
    )
    for exception_type in top_level_exception_types:
        sensitive_messages = (
            ("secret-token /tmp/operator/private-path", ("secret-token", "private-path")),
            ("operator secret%2dtoken value", ("secret%2dtoken",)),
            ("operator private&#95;key value", ("private&#95;key",)),
            ("operator password value", ("password",)),
            ("operator passphrase value", ("passphrase",)),
            ("operator bearer%20credential value", ("bearer%20credential",)),
            ("operator authorization value", ("authorization",)),
            ("operator access&#45;key value", ("access&#45;key",)),
            ("operator api_key value", ("api_key",)),
            ("operator client&#45;secret value", ("client&#45;secret",)),
            ("operator recovery%2dphrase value", ("recovery%2dphrase",)),
            ("operator recovery_phrase value", ("recovery_phrase",)),
            ("operator session%3dabc value", ("session%3dabc",)),
            ("operator token&#61;abc value", ("token&#61;abc",)),
        )
        for sensitive_message, leaked_markers in sensitive_messages:

            def fail_load(
                _paths,
                exception_type=exception_type,
                sensitive_message=sensitive_message,
            ):
                raise exception_type(sensitive_message)

            with monkeypatch.context() as patch:
                patch.setattr(module, "load_evidence_bundle", fail_load)
                try:
                    module.main([str(tmp_path / "evidence.toml")])
                except SystemExit as exc:
                    assert exc.code == 2
                else:
                    raise AssertionError("all-lanes CLI accepted top-level load failure")

                captured = capsys.readouterr()
                assert "SCCP all-lanes evidence validation failed" in captured.err
                for leaked_marker in leaked_markers:
                    assert leaked_marker not in captured.err
                assert exception_type.__name__ not in captured.err


def test_all_lanes_cli_preserves_safe_top_level_exception_detail(
    tmp_path,
    monkeypatch,
    capsys,
):
    module = load_evidence_module()
    safe_message = "route evidence is temporarily unavailable"

    def fail_load(_paths):
        raise RuntimeError(safe_message)

    with monkeypatch.context() as patch:
        patch.setattr(module, "load_evidence_bundle", fail_load)
        try:
            module.main([str(tmp_path / "evidence.toml")])
        except SystemExit as exc:
            assert exc.code == 2
        else:
            raise AssertionError("all-lanes CLI accepted top-level load failure")

    captured = capsys.readouterr()
    assert safe_message in captured.err
    assert "SCCP all-lanes evidence validation failed" not in captured.err
    assert "RuntimeError" not in captured.err


def test_all_lanes_loader_rejects_duplicate_metadata_comments(tmp_path):
    module = load_evidence_module()
    toml_path = tmp_path / "duplicate-comments.toml"
    toml_path.write_text(
        "\n".join(
            [
                '# sccp_route_canary_status = "passed"',
                '# sccp_route_canary_status = "passed"',
                "[[zk.sccp_route_allowlists]]",
                "version = 1",
            ]
        ),
        encoding="utf-8",
    )

    try:
        module.load_evidence_bundle([toml_path])
    except ValueError as exc:
        assert (
            "duplicate-comments.toml:2: duplicate metadata comment "
            "for _comment_route_canary_status"
        ) in str(exc)
    else:
        raise AssertionError("all-lanes loader accepted duplicate metadata comments")


def test_all_lanes_loader_rejects_duplicate_metadata_comment_aliases(tmp_path):
    module = load_evidence_module()
    toml_path = tmp_path / "duplicate-alias-comments.toml"
    toml_path.write_text(
        "\n".join(
            [
                f'# sccp_eth_source_verifier_material_hash = "{hex32(0xA0)}"',
                f'# sccp_bsc_source_verifier_material_hash = "{hex32(0xA1)}"',
                "[[zk.sccp_source_verifier_materials]]",
                "version = 1",
            ]
        ),
        encoding="utf-8",
    )

    try:
        module.load_evidence_bundle([toml_path])
    except ValueError as exc:
        assert (
            "duplicate-alias-comments.toml:2: duplicate metadata comment "
            "for _comment_source_verifier_material_hash"
        ) in str(exc)
    else:
        raise AssertionError("all-lanes loader accepted duplicate metadata aliases")


def test_all_lanes_evidence_bundle_is_ready():
    module = load_evidence_module()
    records = complete_bundle(module)
    deployments_by_domain = {
        deployment["source_domain"]: deployment
        for deployment in records["sccp_source_adapter_engine_deployments"]
    }

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is True
    assert summary["blockers"] == []
    assert summary["required_domains"] == list(module.SCCP_CORE_REMOTE_DOMAINS)
    assert summary["supported_launch_domains"] == list(
        module.SCCP_SUPPORTED_LAUNCH_REMOTE_DOMAINS
    )
    assert summary["unsupported_launch_domains"] == list(
        module.SCCP_UNSUPPORTED_LAUNCH_REMOTE_DOMAINS
    )
    assert [lane["domain"] for lane in summary["lanes"]] == list(
        module.SCCP_CORE_REMOTE_DOMAINS
    )
    for lane in summary["lanes"]:
        assert set(lane["source_record_hashes"]) == {
            "source_verifier_material_hash",
            "source_adapter_engine_deployment_hash",
        }
        for value in lane["source_record_hashes"].values():
            assert value.startswith("0x")
            assert len(value) == 66
        assert lane["production_ready"] is True
        assert lane["blockers"] == []
        source_adapter_gate = lane["source_adapter_gate"]
        if lane["chain"] in ("eth", "bsc"):
            deployment = deployments_by_domain[lane["domain"]]
            assert source_adapter_gate["required"] is True
            assert source_adapter_gate["ready"] is True
            assert source_adapter_gate["gate_hash"] == deployment[
                "evm_source_gate_hash"
            ]
            assert set(source_adapter_gate["audit_hashes"]) == {"evm_source_gate_hash"}
            assert source_adapter_gate["blockers"] == []
        elif lane["chain"] == "sol":
            deployment = deployments_by_domain[lane["domain"]]
            assert source_adapter_gate["required"] is True
            assert source_adapter_gate["ready"] is True
            assert source_adapter_gate["gate_hash"] == deployment[
                "solana_full_light_client_gate_hash"
            ]
            assert set(source_adapter_gate["audit_hashes"]) == set(
                module.SOLANA_FULL_LIGHT_CLIENT_AUDIT_FIELDS
            )
            assert source_adapter_gate["blockers"] == []
        elif lane["chain"] == "ton":
            deployment = deployments_by_domain[lane["domain"]]
            assert source_adapter_gate["required"] is True
            assert source_adapter_gate["ready"] is True
            assert source_adapter_gate["gate_hash"] == deployment[
                "ton_full_light_client_gate_hash"
            ]
            assert set(source_adapter_gate["audit_hashes"]) == set(
                module.TON_FULL_LIGHT_CLIENT_AUDIT_FIELDS
            )
            assert source_adapter_gate["blockers"] == []
        elif lane["chain"] == "tron":
            deployment = deployments_by_domain[lane["domain"]]
            assert source_adapter_gate["required"] is True
            assert source_adapter_gate["ready"] is True
            assert source_adapter_gate["gate_hash"] == deployment[
                "tron_dpos_source_gate_hash"
            ]
            assert set(source_adapter_gate["audit_hashes"]) == {
                "tron_dpos_source_gate_hash"
            }
            assert source_adapter_gate["blockers"] == []
        evm_live_metadata = lane["evm_live_metadata"]
        if lane["chain"] == "eth":
            assert evm_live_metadata == {
                "required": True,
                "ready": True,
                "source_rpc_chain_id": "1",
                "source_block_tag": "finalized",
                "destination_rpc_chain_id": "1",
                "destination_block_tag": "finalized",
            }
        elif lane["chain"] == "bsc":
            assert evm_live_metadata == {
                "required": True,
                "ready": True,
                "source_rpc_chain_id": "56",
                "source_block_tag": "latest",
                "destination_rpc_chain_id": "56",
                "destination_block_tag": "latest",
            }
        else:
            assert evm_live_metadata == {
                "required": False,
                "ready": True,
                "source_rpc_chain_id": "",
                "source_block_tag": "",
                "destination_rpc_chain_id": "",
                "destination_block_tag": "",
            }
        binding = lane["destination_binding"]
        assert binding["destination_binding_hash"].startswith("0x")
        assert len(binding["destination_binding_hash"]) == 66
        assert binding["expected_destination_binding_hash_matches"] is True
        assert binding["recomputed"] is True
        if lane["chain"] in ("eth", "bsc"):
            assert binding["destination_network_id"].startswith("0x")
            assert binding["destination_bridge_address"].startswith("0x")
            assert binding["destination_binding_key"].startswith(
                f"evm:0:{lane['domain']}:"
            )
        if lane["chain"] == "tron":
            assert binding["destination_binding_key"].startswith("tron:0:5:")
        route = lane["route_allowlist"]
        assert route["route_allowlist_hash"].startswith("0x")
        assert len(route["route_allowlist_hash"]) == 66
        assert route["expected_route_allowlist_hash_matches"] is True
        assert route["route_canary"]["status"] == "passed"
        assert route["route_canary"]["evidence_hash"].startswith("0x")
        assert route["route_canary"]["route_allowlist_hash"] == route["route_allowlist_hash"]
        assert route["route_canary"]["destination_binding_hash"] == (
            binding["destination_binding_hash"]
        )
        assert route["route_canary"]["evidence_bound"] is True


def test_all_lanes_evidence_rejects_bare_fixed_hex_aliases():
    module = load_evidence_module()
    source_records = complete_bundle(module)
    eth_material = source_records["sccp_source_verifier_materials"][0]
    eth_material["source_trust_anchor_hash"] = eth_material[
        "source_trust_anchor_hash"
    ][2:]

    source_summary = module.validate_evidence_bundle(source_records)

    source_blockers = "\n".join(source_summary["blockers"])
    assert source_summary["production_ready"] is False
    assert (
        "domain 1 (eth): source_trust_anchor_hash must be a non-zero "
        "32-byte hex value"
    ) in source_blockers

    route_records = complete_bundle(module)
    eth_route = route_records["sccp_route_allowlists"][0]
    eth_route["_comment_route_canary_evidence_hash"] = eth_route[
        "_comment_route_canary_evidence_hash"
    ][2:]

    route_summary = module.validate_evidence_bundle(route_records)

    route_blockers = "\n".join(route_summary["blockers"])
    assert route_summary["production_ready"] is False
    assert (
        "domain 1 (eth): route canary evidence hash metadata must be a "
        "non-zero bytes32"
    ) in route_blockers


def test_all_lanes_evidence_accepts_config_route_canary_fields():
    module = load_evidence_module()
    records = complete_bundle(module)
    route = records["sccp_route_allowlists"][0]
    route["route_canary_status"] = route.pop("_comment_route_canary_status")
    route["route_canary_evidence_hash"] = route.pop(
        "_comment_route_canary_evidence_hash"
    )
    route["route_canary_route_allowlist_hash"] = route.pop(
        "_comment_route_canary_route_allowlist_hash"
    )
    route["route_canary_destination_binding_hash"] = route.pop(
        "_comment_route_canary_destination_binding_hash"
    )

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is True
    first_route = summary["lanes"][0]["route_allowlist"]
    assert first_route["route_canary"]["status"] == "passed"
    assert first_route["route_canary"]["route_allowlist_hash"] == (
        first_route["route_allowlist_hash"]
    )


def test_all_lanes_evidence_rejects_route_canary_comment_drift():
    module = load_evidence_module()
    records = complete_bundle(module)
    route = records["sccp_route_allowlists"][0]
    route["route_canary_status"] = route["_comment_route_canary_status"]
    route["route_canary_evidence_hash"] = route["_comment_route_canary_evidence_hash"]
    route["route_canary_route_allowlist_hash"] = route[
        "_comment_route_canary_route_allowlist_hash"
    ]
    route["route_canary_destination_binding_hash"] = route[
        "_comment_route_canary_destination_binding_hash"
    ]

    route["_comment_route_canary_status"] = "failed"
    route["_comment_route_canary_evidence_hash"] = hex32(0xB0)
    route["_comment_route_canary_route_allowlist_hash"] = hex32(0xB1)
    route["_comment_route_canary_destination_binding_hash"] = hex32(0xB2)

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert (
        "domain 1 (eth): route_canary_status comment must match "
        "route_canary_status"
    ) in blockers
    assert (
        "domain 1 (eth): route_canary_evidence_hash comment must match "
        "route_canary_evidence_hash"
    ) in blockers
    assert (
        "domain 1 (eth): route_canary_route_allowlist_hash comment must match "
        "route_canary_route_allowlist_hash"
    ) in blockers
    assert (
        "domain 1 (eth): route_canary_destination_binding_hash comment must match "
        "route_canary_destination_binding_hash"
    ) in blockers


def test_all_lanes_evidence_rejects_unsupported_domain_records():
    module = load_evidence_module()
    records = complete_bundle(module)
    expected_record_index = len(module.SCCP_SUPPORTED_LAUNCH_REMOTE_DOMAINS)

    extra_material = dict(records["sccp_source_verifier_materials"][0])
    extra_material["source_domain"] = 99
    records["sccp_source_verifier_materials"].append(extra_material)
    extra_deployment = dict(records["sccp_source_adapter_engine_deployments"][0])
    extra_deployment["source_domain"] = 99
    records["sccp_source_adapter_engine_deployments"].append(extra_deployment)
    extra_destination = dict(records["sccp_destination_rollouts"][0])
    extra_destination["domain"] = 99
    records["sccp_destination_rollouts"].append(extra_destination)
    extra_route = dict(records["sccp_route_allowlists"][0])
    extra_route["domain"] = 99
    records["sccp_route_allowlists"].append(extra_route)

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert (
        f"sccp_source_verifier_materials: record {expected_record_index} "
        "uses unsupported source_domain 99"
        in blockers
    )
    assert (
        f"sccp_source_adapter_engine_deployments: record {expected_record_index} "
        "uses unsupported source_domain 99"
        in blockers
    )
    assert (
        f"sccp_destination_rollouts: record {expected_record_index} "
        "uses unsupported domain 99"
    ) in blockers
    assert (
        f"sccp_route_allowlists: record {expected_record_index} "
        "uses unsupported domain 99"
    ) in blockers


def test_all_lanes_evidence_rejects_boolean_domain_fields():
    module = load_evidence_module()
    records = complete_bundle(module)

    records["sccp_source_verifier_materials"][0]["source_domain"] = True
    records["sccp_source_adapter_engine_deployments"][0]["source_domain"] = True
    records["sccp_destination_rollouts"][0]["domain"] = True
    records["sccp_route_allowlists"][0]["domain"] = True

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert (
        "sccp_source_verifier_materials: record 0 missing integer source_domain"
        in blockers
    )
    assert (
        "sccp_source_adapter_engine_deployments: record 0 missing integer source_domain"
        in blockers
    )
    assert "sccp_destination_rollouts: record 0 missing integer domain" in blockers
    assert "sccp_route_allowlists: record 0 missing integer domain" in blockers


def test_all_lanes_evidence_rejects_unknown_record_fields():
    module = load_evidence_module()
    records = complete_bundle(module)

    records["sccp_source_verifier_materials"][0]["unexpected_material_hash"] = hex32(
        0xA0
    )
    records["sccp_source_adapter_engine_deployments"][1][
        "unexpected_deployment_hash"
    ] = hex32(0xA1)
    records["sccp_destination_rollouts"][2]["unexpected_destination_hash"] = hex32(
        0xA2
    )
    records["sccp_route_allowlists"][3]["unexpected_route_hash"] = hex32(0xA3)

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "domain 1 (eth): unexpected field unexpected_material_hash" in blockers
    assert "domain 2 (bsc): unexpected field unexpected_deployment_hash" in blockers
    assert "domain 3 (sol): unexpected field unexpected_destination_hash" in blockers
    assert "domain 4 (ton): unexpected field unexpected_route_hash" in blockers


def test_all_lanes_evidence_redacts_hostile_root_section_names():
    module = load_evidence_module()
    records = complete_bundle(module)
    records[HostilePublicKey()] = []

    summary = module.validate_evidence_bundle(records)

    blockers = "\n".join(summary["blockers"])
    assert summary["production_ready"] is False
    assert "evidence section name must be a string" in blockers
    assert "secret-token" not in blockers
    assert "hostile" not in blockers


def test_all_lanes_evidence_redacts_unsafe_unknown_record_fields():
    module = load_evidence_module()
    cases = (
        (
            "secret-token-material-field",
            "unexpected field with sensitive name",
        ),
        (
            "route|operator-material-field",
            "unexpected field with malformed name",
        ),
        (
            7,
            "unexpected non-string field name",
        ),
    )
    for field, expected in cases:
        records = complete_bundle(module)
        records["sccp_source_verifier_materials"][0][field] = "operator-controlled"

        summary = module.validate_evidence_bundle(records)

        blockers = "\n".join(summary["blockers"])
        assert summary["production_ready"] is False
        assert expected in blockers
        assert str(field) not in blockers

    records = complete_bundle(module)
    records["sccp_source_verifier_materials"][0][HostilePublicKey()] = (
        "operator-controlled"
    )

    summary = module.validate_evidence_bundle(records)

    blockers = "\n".join(summary["blockers"])
    assert summary["production_ready"] is False
    assert "unexpected non-string field name" in blockers
    assert "secret-token" not in blockers
    assert "hostile" not in blockers


def test_all_lanes_evidence_rejects_reused_source_material_role_hashes():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_material = records["sccp_source_verifier_materials"][0]
    eth_material["consensus_verifier_hash"] = eth_material["source_trust_anchor_hash"]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert (
        "domain 1 (eth): source verifier material role hash "
        "consensus_verifier_hash must not reuse source_trust_anchor_hash"
    ) in blockers


def test_all_lanes_evidence_rejects_reused_source_deployment_role_hashes():
    module = load_evidence_module()
    records = complete_bundle(module)
    bsc_deployment = records["sccp_source_adapter_engine_deployments"][1]
    bsc_deployment["deployment_receipt_hash"] = bsc_deployment[
        "adapter_verifier_vk_hash"
    ]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert (
        "domain 2 (bsc): source adapter deployment role hash "
        "deployment_receipt_hash must not reuse adapter_verifier_vk_hash"
    ) in blockers


def test_all_lanes_evidence_rejects_stale_source_record_hash_comments():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_material = records["sccp_source_verifier_materials"][0]
    bsc_deployment = records["sccp_source_adapter_engine_deployments"][1]
    eth_material["_comment_source_verifier_material_hash"] = hex32(0xD0)
    bsc_deployment["_comment_source_adapter_engine_deployment_hash"] = hex32(0xD1)

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert (
        "domain 1 (eth): source verifier material hash metadata must match "
        "the canonical record hash"
    ) in blockers
    assert (
        "domain 2 (bsc): source adapter deployment hash metadata must match "
        "the canonical record hash"
    ) in blockers


def test_all_lanes_evidence_requires_source_record_hash_comments():
    module = load_evidence_module()
    records = complete_bundle(module)
    records["sccp_source_verifier_materials"][0].pop(
        "_comment_source_verifier_material_hash"
    )
    records["sccp_source_adapter_engine_deployments"][1].pop(
        "_comment_source_adapter_engine_deployment_hash"
    )

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert (
        "domain 1 (eth): source verifier material hash metadata must be a "
        "non-zero 32-byte hex value"
    ) in blockers
    assert (
        "domain 2 (bsc): source adapter deployment hash metadata must be a "
        "non-zero 32-byte hex value"
    ) in blockers


def test_all_lanes_evidence_redacts_source_record_hash_comment_failures(
    monkeypatch,
) -> None:
    """Source-record metadata blockers must not echo exception payloads."""

    module = load_evidence_module()
    records = complete_bundle(module)
    profile = module.LANE_PROFILES[module.SCCP_DOMAIN_ETH]
    material = records["sccp_source_verifier_materials"][0]
    deployment = records["sccp_source_adapter_engine_deployments"][0]
    source_record_exception_types = (
        module.argparse.ArgumentTypeError,
        SystemExit,
        TypeError,
        ValueError,
        RuntimeError,
    )

    for exception_type in source_record_exception_types:

        def fail_hashes(_profile, _material, _deployment):
            raise exception_type("secret-token source record material")

        monkeypatch.setattr(module, "_canonical_source_record_hashes", fail_hashes)

        errors = module._check_source_record_hash_comments(profile, material, deployment)
        rendered = "\n".join(errors)

        assert errors == ["eth source record hash metadata cannot be recomputed"]
        assert "source record hash metadata cannot be recomputed:" not in rendered
        assert "secret-token" not in rendered
        assert exception_type.__name__ not in rendered


def test_all_lanes_evidence_redacts_source_record_hash_summary_failures(
    monkeypatch,
) -> None:
    """Source-record summary blockers must not echo exception payloads."""

    module = load_evidence_module()
    original_hashes = module._canonical_source_record_hashes
    source_record_exception_types = (
        module.argparse.ArgumentTypeError,
        SystemExit,
        TypeError,
        ValueError,
        RuntimeError,
    )

    for exception_type in source_record_exception_types:
        monkeypatch.setattr(module, "_canonical_source_record_hashes", original_hashes)
        records = complete_bundle(module)
        call_count = 0

        def fail_summary_hashes(profile, material, deployment):
            nonlocal call_count
            call_count += 1
            if call_count % 2 == 0:
                raise exception_type("secret-token source record material")
            return original_hashes(profile, material, deployment)

        monkeypatch.setattr(
            module,
            "_canonical_source_record_hashes",
            fail_summary_hashes,
        )

        summary = module.validate_evidence_bundle(records)

        assert summary["production_ready"] is False
        blockers = "\n".join(summary["blockers"])
        assert "source record hashes cannot be recomputed" in blockers
        assert "source record hashes cannot be recomputed:" not in blockers
        assert "secret-token" not in blockers
        assert exception_type.__name__ not in blockers


def test_all_lanes_loads_source_record_hash_comments_from_toml(tmp_path):
    module = load_evidence_module()
    records = complete_bundle(module)
    toml_path = tmp_path / "sccp-all-lanes.toml"
    toml_path.write_text(render_records(records), encoding="utf-8")

    loaded = module.load_evidence_bundle([toml_path])

    assert loaded["sccp_source_verifier_materials"][0][
        "_comment_source_verifier_material_hash"
    ] == records["sccp_source_verifier_materials"][0][
        "_comment_source_verifier_material_hash"
    ]
    assert loaded["sccp_source_adapter_engine_deployments"][0][
        "_comment_source_adapter_engine_deployment_hash"
    ] == records["sccp_source_adapter_engine_deployments"][0][
        "_comment_source_adapter_engine_deployment_hash"
    ]
    assert module.validate_evidence_bundle(loaded)["production_ready"] is True


def test_all_lanes_evidence_rejects_reused_light_client_audit_role_hashes():
    module = load_evidence_module()
    records = complete_bundle(module)
    solana_deployment = records["sccp_source_adapter_engine_deployments"][2]
    solana_deployment["solana_bank_fork_choice_verifier_hash"] = solana_deployment[
        "solana_tower_replay_verifier_hash"
    ]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert (
        "domain 3 (sol): source adapter deployment role hash "
        "solana_bank_fork_choice_verifier_hash must not reuse "
        "solana_tower_replay_verifier_hash"
    ) in blockers


def test_all_lanes_evidence_rejects_source_adapter_audit_hash_template_replays():
    module = load_evidence_module()
    solana_module = module._load_sibling_module("sccp_solana_source_state_evidence.py")
    ton_module = module._load_sibling_module("sccp_ton_source_state_evidence.py")
    cases = (
        (
            module.SCCP_DOMAIN_SOL,
            module.SOLANA_FULL_LIGHT_CLIENT_AUDIT_ROLE_HASH_FIELDS,
            solana_module._template_component_hashes(),
        ),
        (
            module.SCCP_DOMAIN_TON,
            module.TON_FULL_LIGHT_CLIENT_AUDIT_ROLE_HASH_FIELDS,
            ton_module._template_component_hashes(),
        ),
    )

    for domain, audit_fields, template_hashes in cases:
        profile = module.LANE_PROFILES[domain]
        for audit_field in audit_fields:
            for template_field, template_hash in template_hashes.items():
                records = complete_bundle(module)
                deployment_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(domain)
                deployment = records["sccp_source_adapter_engine_deployments"][
                    deployment_index
                ]
                deployment[audit_field] = "0x" + template_hash.hex()

                summary = module.validate_evidence_bundle(records)

                assert summary["production_ready"] is False, (
                    domain,
                    audit_field,
                    template_field,
                )
                assert (
                    f"domain {domain} ({profile.chain}): {audit_field} must be "
                    "deployed audit evidence, not built-in template material"
                ) in summary["blockers"]


def test_all_lanes_evidence_rejects_source_material_template_hashes_for_all_lanes():
    module = load_evidence_module()
    eth_module = module._load_sibling_module("sccp_eth_source_bridge_evidence.py")
    bsc_module = module._load_sibling_module("sccp_bsc_source_bridge_evidence.py")
    solana_module = module._load_sibling_module("sccp_solana_source_state_evidence.py")
    ton_module = module._load_sibling_module("sccp_ton_source_state_evidence.py")
    tron_module = module._load_sibling_module("sccp_tron_source_bridge_evidence.py")
    template_cases = [
        (
            module.SCCP_DOMAIN_ETH,
            field,
            eth_module._evm_family_template_component_hash(component_id, component_kind),
        )
        for field, (component_id, component_kind) in (
            eth_module.ETH_TEMPLATE_COMPONENTS.items()
        )
    ]
    template_cases.extend(
        (
            module.SCCP_DOMAIN_BSC,
            field,
            bsc_module._evm_family_template_component_hash(component_id, component_kind),
        )
        for field, (component_id, component_kind) in (
            bsc_module.bsc_template_components().items()
        )
    )
    template_cases.extend(
        (module.SCCP_DOMAIN_SOL, field, template_hash)
        for field, template_hash in solana_module._template_component_hashes().items()
    )
    template_cases.extend(
        (module.SCCP_DOMAIN_TON, field, template_hash)
        for field, template_hash in ton_module._template_component_hashes().items()
    )
    template_cases.extend(
        (
            module.SCCP_DOMAIN_TRON,
            field,
            tron_module._tron_template_component_hash(component_id, component_kind),
        )
        for field, (component_id, component_kind) in (
            tron_module.TRON_TEMPLATE_COMPONENTS.items()
        )
    )

    assert template_cases
    for domain, field, template_hash in template_cases:
        records = complete_bundle(module)
        material_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(domain)
        profile = module.LANE_PROFILES[domain]
        records["sccp_source_verifier_materials"][material_index][field] = (
            "0x" + template_hash.hex()
        )

        summary = module.validate_evidence_bundle(records)

        assert summary["production_ready"] is False, (domain, field)
        assert (
            f"domain {domain} ({profile.chain}): {field} must be deployed "
            "evidence, not built-in template material"
        ) in summary["blockers"]


def test_all_lanes_evidence_rejects_source_adapter_deployment_template_hashes_for_all_lanes():
    module = load_evidence_module()
    eth_module = module._load_sibling_module("sccp_eth_source_bridge_evidence.py")
    bsc_module = module._load_sibling_module("sccp_bsc_source_bridge_evidence.py")
    solana_module = module._load_sibling_module("sccp_solana_source_state_evidence.py")
    ton_module = module._load_sibling_module("sccp_ton_source_state_evidence.py")
    tron_module = module._load_sibling_module("sccp_tron_source_bridge_evidence.py")
    template_cases = [
        (
            module.SCCP_DOMAIN_ETH,
            field,
            eth_module._evm_family_template_component_hash(component_id, component_kind),
        )
        for field, (component_id, component_kind) in (
            eth_module.ETH_TEMPLATE_COMPONENTS.items()
        )
    ]
    template_cases.extend(
        (
            module.SCCP_DOMAIN_BSC,
            field,
            bsc_module._evm_family_template_component_hash(component_id, component_kind),
        )
        for field, (component_id, component_kind) in (
            bsc_module.bsc_template_components().items()
        )
    )
    template_cases.extend(
        (module.SCCP_DOMAIN_SOL, field, template_hash)
        for field, template_hash in solana_module._template_component_hashes().items()
    )
    template_cases.extend(
        (module.SCCP_DOMAIN_TON, field, template_hash)
        for field, template_hash in ton_module._template_component_hashes().items()
    )
    template_cases.extend(
        (
            module.SCCP_DOMAIN_TRON,
            field,
            tron_module._tron_template_component_hash(component_id, component_kind),
        )
        for field, (component_id, component_kind) in (
            tron_module.TRON_TEMPLATE_COMPONENTS.items()
        )
    )

    assert template_cases
    for domain, field, template_hash in template_cases:
        records = complete_bundle(module)
        material_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(domain)
        profile = module.LANE_PROFILES[domain]
        template_value = "0x" + template_hash.hex()
        records["sccp_source_verifier_materials"][material_index][field] = (
            template_value
        )
        records["sccp_source_adapter_engine_deployments"][material_index][field] = (
            template_value
        )

        summary = module.validate_evidence_bundle(records)

        assert summary["production_ready"] is False, (domain, field)
        assert (
            f"domain {domain} ({profile.chain}): {field} must be deployed "
            "source-adapter evidence, not built-in template material"
        ) in summary["blockers"]


def test_all_lanes_evidence_rejects_source_adapter_deployment_control_hash_template_replays():
    module = load_evidence_module()

    for domain in module.SCCP_CORE_REMOTE_DOMAINS:
        profile = module.LANE_PROFILES[domain]
        template_hash = next(
            iter(module._source_material_template_hashes(profile).values())
        )
        template_value = "0x" + template_hash.hex()
        record_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(domain)
        for field in ("adapter_verifier_vk_hash", "deployment_receipt_hash"):
            records = complete_bundle(module)
            records["sccp_source_adapter_engine_deployments"][record_index][field] = (
                template_value
            )

            summary = module.validate_evidence_bundle(records)

            assert summary["production_ready"] is False, (domain, field)
            assert (
                f"domain {domain} ({profile.chain}): {field} must be deployed "
                "source-adapter evidence, not built-in template material"
            ) in summary["blockers"]


def test_all_lanes_evidence_reports_deployment_source_role_shape_independently():
    module = load_evidence_module()
    cases = (
        (
            module.SCCP_DOMAIN_BSC,
            ("source_bridge_emitter_address",),
            "source adapter deployment source_bridge_emitter_address must be a non-zero 20-byte hex value",
        ),
        (
            module.SCCP_DOMAIN_SOL,
            ("source_state_verifier_id", "source_state_verifier_hash"),
            "source adapter deployment source_state_verifier_hash must be a non-zero 32-byte hex value",
        ),
        (
            module.SCCP_DOMAIN_ETH,
            ("source_state_verifier_id", "source_state_verifier_hash"),
            "source adapter deployment source_state_verifier_hash must be empty for this lane",
        ),
    )

    for domain, fields, expected_blocker in cases:
        records = complete_bundle(module)
        record_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(domain)
        profile = module.LANE_PROFILES[domain]
        material = records["sccp_source_verifier_materials"][record_index]
        deployment = records["sccp_source_adapter_engine_deployments"][record_index]
        for field in fields:
            if domain == module.SCCP_DOMAIN_ETH and field == "source_state_verifier_id":
                material[field] = module.LANE_PROFILES[
                    module.SCCP_DOMAIN_SOL
                ].source_state_verifier_id
                deployment[field] = material[field]
            elif domain == module.SCCP_DOMAIN_ETH and field == "source_state_verifier_hash":
                material[field] = hex32(0xE1)
                deployment[field] = material[field]
            else:
                material.pop(field, None)
                deployment.pop(field, None)

        summary = module.validate_evidence_bundle(records)

        assert summary["production_ready"] is False, (domain, fields)
        assert (
            f"domain {domain} ({profile.chain}): {expected_blocker}"
            in "\n".join(summary["blockers"])
        )


def test_all_lanes_evidence_rejects_unknown_sections(tmp_path, capsys):
    module = load_evidence_module()
    records = complete_bundle(module)
    records["sccp_shadow_rollouts"] = [{"domain": 1}]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert "unsupported evidence section sccp_shadow_rollouts" in summary["blockers"]

    path = tmp_path / "unknown-section.toml"
    path.write_text("[[zk.sccp_shadow_rollouts]]\ndomain = 1\n", encoding="utf-8")

    try:
        module.main([str(path)])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        assert False, "unknown TOML sections must abort the CLI preflight"
    assert "unsupported zk section sccp_shadow_rollouts" in capsys.readouterr().err


def test_all_lanes_evidence_redacts_unsafe_direct_section_names():
    module = load_evidence_module()

    cases = (
        (
            "secret-token-direct-section",
            "unsupported evidence section with sensitive name",
        ),
        (
            "route|operator-direct-section",
            "unsupported evidence section with malformed name",
        ),
    )
    for section, expected in cases:
        records = complete_bundle(module)
        records[section] = [{"domain": 1}]

        summary = module.validate_evidence_bundle(records)

        blockers = "\n".join(summary["blockers"])
        assert summary["production_ready"] is False
        assert expected in blockers
        assert section not in blockers


def test_all_lanes_loader_redacts_unsupported_zk_section_names(tmp_path):
    module = load_evidence_module()
    if module.tomllib is None:
        return

    cases = (
        (
            '[[zk."secret-token-zk-section"]]\ndomain = 1\n',
            "unsupported zk section with sensitive name",
            "secret-token-zk-section",
        ),
        (
            '[[zk."recovery-phrase-zk-section"]]\ndomain = 1\n',
            "unsupported zk section with sensitive name",
            "recovery-phrase-zk-section",
        ),
        (
            '[[zk."mnemonic-zk-section"]]\ndomain = 1\n',
            "unsupported zk section with sensitive name",
            "mnemonic-zk-section",
        ),
        (
            '[[zk."route|operator-zk-section"]]\ndomain = 1\n',
            "unsupported zk section with malformed name",
            "route|operator-zk-section",
        ),
    )
    for index, (toml_text, expected_detail, redacted_token) in enumerate(cases):
        path = tmp_path / f"unsafe-section-{index}.toml"
        path.write_text(toml_text, encoding="utf-8")

        try:
            module.load_evidence_bundle([path])
        except ValueError as exc:
            rendered = str(exc)
            assert expected_detail in rendered
            assert redacted_token not in rendered
        else:
            raise AssertionError("unsafe unsupported TOML section was accepted")


def test_all_lanes_evidence_rejects_malformed_direct_sections():
    module = load_evidence_module()
    records = complete_bundle(module)
    records["sccp_source_verifier_materials"] = {"source_domain": 1}
    records["sccp_route_allowlists"][0] = "not a table"

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "sccp_source_verifier_materials: records must be a list" in blockers
    assert "sccp_route_allowlists: record 0 must be a table" in blockers


def test_all_lanes_evidence_rejects_malformed_root_inputs():
    module = load_evidence_module()

    summary = module.validate_evidence_bundle(["not", "an", "object"])

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "evidence bundle root must be an object" in blockers
    assert "domain 1 (eth): missing source verifier material" in blockers


def test_all_lanes_evidence_rejects_non_string_section_keys():
    module = load_evidence_module()
    records = complete_bundle(module)
    records[1] = []

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert "evidence section name must be a string: 1" in summary["blockers"]


def test_route_allowlist_hash_matches_rust_vector():
    module = load_evidence_module()

    assert (
        module.route_allowlist_hash_for_lane_evidence(
            module.LANE_PROFILES[module.SCCP_DOMAIN_ETH],
            bytes([0x11]) * 32,
            bytes([0x22]) * 32,
            bytes([0x33]) * 32,
        ).hex()
        == "5cbb92b2e55d2cad382b687ec60703217fd13be788f38a8e92d459e5fe82aca5"
    )


def test_route_allowlist_hash_rejects_zero_or_replayed_roles():
    module = load_evidence_module()
    profile = module.LANE_PROFILES[module.SCCP_DOMAIN_ETH]

    for source_material, source_deployment, destination_binding in (
        (bytes(32), bytes([0x22]) * 32, bytes([0x33]) * 32),
        (bytes([0x11]) * 32, bytes([0x11]) * 32, bytes([0x33]) * 32),
        (bytes([0x11]) * 32, bytes([0x22]) * 32, bytes([0x11]) * 32),
    ):
        try:
            module.route_allowlist_hash_for_lane_evidence(
                profile,
                source_material,
                source_deployment,
                destination_binding,
            )
        except ValueError as exc:
            assert "non-zero" in str(exc) or "must be distinct" in str(exc)
        else:
            raise AssertionError("replayed route allowlist evidence role was accepted")


def test_route_allowlist_hash_rejects_template_source_record_inputs():
    module = load_evidence_module()

    for domain in module.SCCP_CORE_REMOTE_DOMAINS:
        profile = module.LANE_PROFILES[domain]
        template_hashes = tuple(
            module._source_material_template_hashes(profile).values()
        )
        deployed_hashes = [
            bytes([byte]) * 32
            for byte in range(0x11, 0x40)
            if bytes([byte]) * 32 not in template_hashes
        ][:3]
        assert len(deployed_hashes) == 3
        source_material, source_deployment, destination_binding = deployed_hashes
        template_hash = template_hashes[0]

        for label, evidence in (
            (
                "source_verifier_material_hash",
                (template_hash, source_deployment, destination_binding),
            ),
            (
                "source_adapter_engine_deployment_hash",
                (source_material, template_hash, destination_binding),
            ),
            (
                "destination_binding_hash",
                (source_material, source_deployment, template_hash),
            ),
        ):
            try:
                module.route_allowlist_hash_for_lane_evidence(profile, *evidence)
            except ValueError as exc:
                assert (
                    f"{label} must be deployed evidence, "
                    "not built-in template material"
                ) in str(exc)
            else:
                raise AssertionError(
                    f"{label} accepted built-in source material template hash"
                )


def test_all_lanes_accepts_verified_evm_live_toml(tmp_path):
    module = load_evidence_module()
    live_module = load_evm_live_module()
    records = complete_bundle(module)
    profile = module.LANE_PROFILES[module.SCCP_DOMAIN_ETH]
    eth_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_ETH)
    material = records["sccp_source_verifier_materials"][eth_index]
    deployment = records["sccp_source_adapter_engine_deployments"][eth_index]
    source_hashes = module._canonical_source_record_hashes(profile, material, deployment)
    fake = fake_evm_live_opener(live_module, domain=module.SCCP_DOMAIN_ETH)
    route_allowlist_hash = module.route_allowlist_hash_for_lane_evidence(
        profile,
        raw_hex(source_hashes["source_verifier_material_hash"]),
        raw_hex(source_hashes["source_adapter_engine_deployment_hash"]),
        fake.destination_binding,
    )
    route_canary_evidence_hash = live_module.evidence.evm_route_canary_transaction_evidence_hash(
        route_allowlist_hash=route_allowlist_hash,
        bridge_address=raw_hex(fake.bridge),
        transaction_hash=fake.route_canary_transaction_hash,
        log_index=fake.route_canary_log_index,
        receipt_block_number=fake.route_canary_receipt_block_number,
        receipt_block_hash=fake.route_canary_receipt_block_hash,
        block_receipts_root=fake.route_canary_block_receipts_root,
        call_data_sha256=fake.route_canary_call_data_sha256,
        message_id=fake.route_canary_message_id,
        payload_hash=fake.route_canary_payload_hash,
        source_domain=module.SCCP_DOMAIN_SORA,
        target_domain=module.SCCP_DOMAIN_ETH,
        commitment_root=fake.route_canary_commitment_root,
        finality_height=fake.route_canary_finality_height,
        finality_block_hash=fake.route_canary_finality_block_hash,
        statement_hash=fake.route_canary_statement_hash,
        proof_version=1,
        proof_source_domain=module.SCCP_DOMAIN_SORA,
        destination_binding_hash=fake.destination_binding,
        verifier_backend_hash=live_module.evidence.evm_verifier_backend_hash(),
        proof_family_hash=live_module.evidence.evm_proof_family_hash(),
        network_id=fake.network_id,
        used_message_proof=True,
        receipt_block_finalized=True,
    )
    live_summary = live_module.collect_live_evidence(
        SimpleNamespace(
            rpc_url="https://ethereum.example",
            domain=live_module.evidence.SCCP_DOMAIN_ETH,
            bridge_address=fake.bridge,
            expected_network_id=fake.network_id,
            expected_rpc_chain_id=1,
            expected_bridge_code_hash=fake.bridge_code_hash,
            expected_destination_binding_hash=fake.destination_binding,
            route_allowlist_hash=route_allowlist_hash,
            route_canary_evidence_hash=route_canary_evidence_hash,
            route_canary_transaction_hash=fake.route_canary_transaction_hash,
            route_canary_log_index=fake.route_canary_log_index,
            source_verifier_material_hash=raw_hex(
                source_hashes["source_verifier_material_hash"]
            ),
            source_adapter_engine_deployment_hash=raw_hex(
                source_hashes["source_adapter_engine_deployment_hash"]
            ),
            block_tag="finalized",
            timeout=1.0,
        ),
        opener=fake.opener,
    )
    evm_toml = live_module.render_offline_toml(live_summary)
    assert '# sccp_evm_block_tag = "finalized"' in evm_toml
    assert '# sccp_evm_rpc_chain_id = "1"' in evm_toml
    assert (
        '# sccp_evm_bridge_runtime_code_hash = "0x'
        + fake.bridge_code_hash.hex()
        + '"'
        in evm_toml
    )
    assert (
        '# sccp_evm_bridge_runtime_bytecode_hex = "0x'
        + fake.bridge_runtime.hex()
        + '"'
        in evm_toml
    )
    assert (
        '# sccp_evm_verifier_runtime_bytecode_hex = "0x'
        + fake.verifier_runtime.hex()
        + '"'
        in evm_toml
    )

    evm_path = tmp_path / "eth-live.toml"
    evm_path.write_text(evm_toml, encoding="utf-8")
    evm_records = module.load_evidence_bundle([evm_path])

    for section in ("sccp_destination_rollouts", "sccp_route_allowlists"):
        records[section] = [
            record
            for record in records[section]
            if record.get("source_domain", record.get("domain"))
            != module.SCCP_DOMAIN_ETH
        ]
        records[section].extend(evm_records[section])

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is True
    eth_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    assert eth_lane["blockers"] == []
    assert eth_lane["destination_binding"]["destination_binding_hash"] == (
        live_summary["destination_bridge"]["destination_binding_hash"]
    )
    assert eth_lane["route_allowlist"]["route_allowlist_hash"] == (
        "0x" + route_allowlist_hash.hex()
    )


def test_all_lanes_accepts_direct_evm_destination_toml_with_audited_metadata(tmp_path):
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_ETH)
    profile = module.LANE_PROFILES[module.SCCP_DOMAIN_ETH]
    material = records["sccp_source_verifier_materials"][eth_index]
    deployment = records["sccp_source_adapter_engine_deployments"][eth_index]
    destination = records["sccp_destination_rollouts"][eth_index]
    route = records["sccp_route_allowlists"][eth_index]
    evm_module = module._load_sibling_module("sccp_evm_destination_evidence.py")
    args = SimpleNamespace(
        domain=module.SCCP_DOMAIN_ETH,
        network_id=raw_hex(destination["destination_network_id"]),
        verifier_address=raw_hex(destination["verifier_identity"]),
        bridge_address=raw_hex(destination["destination_bridge_address"]),
        bridge_code_hash=raw_hex(destination["_comment_evm_bridge_code_hash"]),
        bridge_runtime_bytecode_hex=evm_module.parse_runtime_bytecode_hex(
            destination["_comment_evm_bridge_runtime_bytecode_hex"],
            label="bridge runtime bytecode",
        ),
        verifier_code_hash=raw_hex(destination["verifier_code_hash"]),
        verifier_runtime_bytecode_hex=evm_module.parse_runtime_bytecode_hex(
            destination["_comment_evm_verifier_runtime_bytecode_hex"],
            label="verifier runtime bytecode",
        ),
        verifier_key_hash=raw_hex(destination["verifier_key_hash"]),
        expected_destination_binding_hash=raw_hex(destination["destination_binding_hash"]),
        source_verifier_material_hash=raw_hex(
            module._canonical_source_record_hashes(profile, material, deployment)[
                "source_verifier_material_hash"
            ]
        ),
        source_adapter_engine_deployment_hash=raw_hex(
            module._canonical_source_record_hashes(profile, material, deployment)[
                "source_adapter_engine_deployment_hash"
            ]
        ),
        route_allowlist_hash=raw_hex(route["route_allowlist_hash"]),
        route_canary_evidence_hash=raw_hex(route["_comment_route_canary_evidence_hash"]),
        route_canary_transaction_hash=raw_hex(
            route["_comment_evm_route_canary_transaction_hash"]
        ),
        route_canary_transaction_block_number=int(
            route["_comment_evm_route_canary_transaction_block_number"]
        ),
        route_canary_transaction_block_hash=raw_hex(
            route["_comment_evm_route_canary_transaction_block_hash"]
        ),
        route_canary_log_index=int(route["_comment_evm_route_canary_log_index"]),
        route_canary_receipt_block_number=int(
            route["_comment_evm_route_canary_receipt_block_number"]
        ),
        route_canary_receipt_block_hash=raw_hex(
            route["_comment_evm_route_canary_receipt_block_hash"]
        ),
        route_canary_block_receipts_root=raw_hex(
            route["_comment_evm_route_canary_block_receipts_root"]
        ),
        route_canary_call_data_sha256=raw_hex(
            route["_comment_evm_route_canary_call_data_sha256"]
        ),
        route_canary_message_id=raw_hex(route["_comment_evm_route_canary_message_id"]),
        route_canary_payload_hash=raw_hex(route["_comment_evm_route_canary_payload_hash"]),
        route_canary_target_domain=int(route["_comment_evm_route_canary_target_domain"]),
        route_canary_statement_hash=raw_hex(
            route["_comment_evm_route_canary_statement_hash"]
        ),
        route_canary_commitment_root=raw_hex(
            route["_comment_evm_route_canary_commitment_root"]
        ),
        route_canary_finality_height=raw_hex(
            route["_comment_evm_route_canary_finality_height"]
        ),
        route_canary_finality_block_hash=raw_hex(
            route["_comment_evm_route_canary_finality_block_hash"]
        ),
        route_canary_proof_version=int(route["_comment_evm_route_canary_proof_version"]),
        route_canary_proof_source_domain=int(
            route["_comment_evm_route_canary_proof_source_domain"]
        ),
        route_canary_used_message_proof=True,
        route_canary_receipt_block_finalized=True,
    )
    evm_toml = evm_module.render_toml(
        args,
        raw_hex(destination["destination_binding_hash"]),
    )
    assert '# sccp_evm_rpc_chain_id = "1"' in evm_toml
    assert '# sccp_evm_block_tag = "finalized"' in evm_toml
    assert "# sccp_evm_bridge_runtime_code_hash" in evm_toml
    assert "# sccp_evm_bridge_runtime_bytecode_hex" in evm_toml
    assert "# sccp_evm_verifier_runtime_code_hash" in evm_toml
    assert "# sccp_evm_verifier_runtime_bytecode_hex" in evm_toml
    assert "# sccp_evm_verifier_key_hash" in evm_toml
    assert "# sccp_evm_route_canary_transaction_hash" in evm_toml
    assert "# sccp_evm_route_canary_transaction_block_number" in evm_toml
    assert "# sccp_evm_route_canary_transaction_block_hash" in evm_toml
    assert "# sccp_evm_route_canary_receipt_block_hash" in evm_toml
    assert "# sccp_evm_route_canary_receipt_block_finalized" in evm_toml
    assert "evm_route_canary_transaction_hash = " in evm_toml
    assert "evm_route_canary_transaction_block_hash = " in evm_toml
    assert "evm_route_canary_block_receipts_root = " in evm_toml
    assert "evm_route_canary_receipt_block_finalized = true" in evm_toml
    evm_path = tmp_path / "eth-direct.toml"
    evm_path.write_text(evm_toml, encoding="utf-8")
    evm_records = module.load_evidence_bundle([evm_path])
    for section in ("sccp_destination_rollouts", "sccp_route_allowlists"):
        records[section] = [
            record
            for record in records[section]
            if record.get("source_domain", record.get("domain"))
            != module.SCCP_DOMAIN_ETH
        ]
        records[section].extend(evm_records[section])

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is True
    eth_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    assert eth_lane["blockers"] == []


def test_all_lanes_rejects_evm_destination_without_live_bridge_metadata():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_destination = records["sccp_destination_rollouts"][0]
    eth_destination.pop("_comment_evm_rpc_chain_id")
    eth_destination.pop("_comment_evm_block_tag")
    eth_destination.pop("_comment_evm_bridge_code_hash")
    eth_destination.pop("_comment_evm_bridge_runtime_bytecode_hex")
    eth_destination.pop("_comment_evm_verifier_code_hash")
    eth_destination.pop("_comment_evm_verifier_runtime_bytecode_hex")
    eth_destination.pop("_comment_evm_verifier_key_hash")
    eth_destination.pop("_comment_evm_verifier_backend_hash")
    eth_destination.pop("_comment_evm_proof_family_hash")

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "EVM live RPC chain-id metadata is required" in blockers
    assert "Ethereum destination live block-tag metadata must be finalized" in blockers
    assert "EVM bridge runtime code hash metadata must be a non-zero" in blockers
    assert "EVM bridge runtime bytecode metadata must be present" in blockers
    assert "EVM verifier runtime code hash metadata must be a non-zero" in blockers
    assert "EVM verifier runtime bytecode metadata must be present" in blockers
    assert "EVM verifier key hash metadata must be a non-zero" in blockers
    assert "EVM verifier backend hash metadata must be a non-zero" in blockers
    assert "EVM proof family hash metadata must be a non-zero" in blockers


def test_all_lanes_rejects_evm_destination_with_invalid_runtime_bytecode_metadata():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_destination = records["sccp_destination_rollouts"][0]
    eth_destination["_comment_evm_bridge_runtime_bytecode_hex"] = "0xxyz"

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert "EVM bridge runtime bytecode metadata is invalid" in "\n".join(
        summary["blockers"]
    )


def test_all_lanes_rejects_evm_destination_noncanonical_runtime_bytecode_metadata():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_destination = records["sccp_destination_rollouts"][0]
    eth_destination["_comment_evm_bridge_runtime_bytecode_hex"] = (
        "0X" + eth_destination["_comment_evm_bridge_runtime_bytecode_hex"][2:]
    )
    eth_destination["_comment_evm_verifier_runtime_bytecode_hex"] = (
        "0X" + eth_destination["_comment_evm_verifier_runtime_bytecode_hex"][2:]
    )

    summary = module.validate_evidence_bundle(records)

    blockers = "\n".join(summary["blockers"])
    assert summary["production_ready"] is False
    assert "EVM bridge runtime bytecode metadata is invalid" in blockers
    assert "must use lowercase 0x prefix" not in blockers
    assert "EVM verifier runtime bytecode metadata is invalid" in blockers


def test_all_lanes_redacts_evm_runtime_bytecode_parser_failures(
    monkeypatch,
) -> None:
    """EVM runtime bytecode metadata blockers must not echo parser payloads."""

    module = load_evidence_module()
    records = complete_bundle(module)
    profile = module.LANE_PROFILES[module.SCCP_DOMAIN_ETH]
    material = records["sccp_source_verifier_materials"][0]
    destination = records["sccp_destination_rollouts"][0]
    original_loader = module._load_sibling_module

    for exception_type in (SystemExit, RuntimeError, TypeError, ValueError):

        def fail_runtime(_value, *, label, exception_type=exception_type):
            raise exception_type(f"secret-token {label} runtime parser")

        def load_sibling(name, fail_runtime=fail_runtime):
            if name in (
                "sccp_eth_source_bridge_evidence.py",
                "sccp_evm_destination_evidence.py",
            ):
                real_module = original_loader(name)
                module_attrs = dict(real_module.__dict__)
                module_attrs["parse_runtime_bytecode_hex"] = fail_runtime
                return SimpleNamespace(**module_attrs)
            return original_loader(name)

        monkeypatch.setattr(module, "_load_sibling_module", load_sibling)

        source_errors = module._check_evm_live_source_bridge_evidence(profile, material)
        destination_errors = module._check_evm_live_bridge_evidence(
            profile,
            destination,
        )
        rendered = "\n".join([*source_errors, *destination_errors])

        assert "EVM source bridge runtime bytecode metadata is invalid" in source_errors
        assert "EVM bridge runtime bytecode metadata is invalid" in destination_errors
        assert "EVM verifier runtime bytecode metadata is invalid" in destination_errors
        assert "metadata is invalid:" not in rendered
        assert "secret-token" not in rendered
        assert exception_type.__name__ not in rendered


def test_all_lanes_rejects_evm_destination_when_runtime_bytecode_hash_drifts():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_destination = records["sccp_destination_rollouts"][0]
    eth_destination["_comment_evm_verifier_runtime_bytecode_hex"] = "0x608060405260ff"

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert "EVM verifier runtime bytecode hash must match verifier code hash metadata" in (
        "\n".join(summary["blockers"])
    )


def test_all_lanes_rejects_evm_source_without_live_bridge_metadata():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_material = records["sccp_source_verifier_materials"][0]
    eth_material.pop("_comment_evm_source_rpc_chain_id")
    eth_material.pop("_comment_evm_source_block_tag")
    eth_material.pop("_comment_evm_source_bridge_address")
    eth_material.pop("_comment_evm_source_bridge_code_hash")
    eth_material.pop("_comment_evm_source_bridge_runtime_bytecode_hex")
    eth_material.pop("_comment_evm_source_deployment_transaction_hash")
    eth_material.pop("_comment_evm_source_deployment_transaction_block_hash")
    eth_material.pop("_comment_evm_source_deployment_transaction_block_number")
    eth_material.pop("_comment_evm_source_deployment_transaction_input_sha256")
    eth_material.pop("_comment_evm_source_deployment_receipt_status")
    eth_material.pop("_comment_evm_source_deployment_contract_address")
    eth_material.pop("_comment_evm_source_deployment_block_hash")
    eth_material.pop("_comment_evm_source_deployment_block_number")
    eth_material.pop("_comment_evm_source_deployment_block_receipts_root")

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "EVM source live RPC chain-id metadata is required" in blockers
    assert "Ethereum source live block-tag metadata must be finalized" in blockers
    assert "EVM source bridge address metadata must be a non-zero" in blockers
    assert "EVM source bridge runtime code hash metadata must be a non-zero" in blockers
    assert "EVM source bridge runtime bytecode metadata must be present" in blockers
    assert "EVM source deployment transaction hash metadata must be a non-zero" in blockers
    assert (
        "EVM source deployment transaction block hash metadata must be a non-zero"
        in blockers
    )
    assert (
        "EVM source deployment transaction block number metadata must be a positive"
        in blockers
    )
    assert (
        "EVM source deployment transaction input SHA-256 metadata must be a non-zero"
        in blockers
    )
    assert "EVM source deployment receipt status metadata must be 0x1" in blockers
    assert "EVM source deployment contract address metadata must be a non-zero" in blockers
    assert "EVM source deployment block hash metadata must be a non-zero" in blockers
    assert "EVM source deployment block number metadata must be a positive" in blockers
    assert "EVM source deployment block receiptsRoot metadata must be a non-zero" in blockers


def test_all_lanes_rejects_ethereum_nonfinalized_evm_live_metadata():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_destination = records["sccp_destination_rollouts"][0]
    eth_material = records["sccp_source_verifier_materials"][0]
    eth_destination["_comment_evm_block_tag"] = "latest"
    eth_material["_comment_evm_source_block_tag"] = "latest"

    summary = module.validate_evidence_bundle(records)
    blockers = "\n".join(summary["blockers"])

    assert summary["production_ready"] is False
    assert "Ethereum destination live block-tag metadata must be finalized" in blockers
    assert "Ethereum source live block-tag metadata must be finalized" in blockers


def test_all_lanes_rejects_ethereum_evm_live_rpc_chain_id_drift():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_destination = records["sccp_destination_rollouts"][0]
    eth_material = records["sccp_source_verifier_materials"][0]
    eth_destination["_comment_evm_rpc_chain_id"] = "2"
    eth_material["_comment_evm_source_rpc_chain_id"] = "2"

    summary = module.validate_evidence_bundle(records)
    eth_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    blockers = "\n".join(summary["blockers"])

    assert summary["production_ready"] is False
    assert eth_lane["evm_live_metadata"]["source_rpc_chain_id"] == "2"
    assert eth_lane["evm_live_metadata"]["destination_rpc_chain_id"] == "2"
    assert eth_lane["evm_live_metadata"]["ready"] is False
    assert "EVM live RPC chain-id must be canonical for eth: expected 1" in blockers
    assert (
        "EVM source live RPC chain-id must be canonical for eth: expected 1"
        in blockers
    )


def test_all_lanes_rejects_evm_source_deployment_transaction_readback_drift():
    module = load_evidence_module()
    cases = [
        (
            "_comment_evm_source_deployment_transaction_block_hash",
            "0x" + "ab" * 32,
            "EVM source deployment transaction block hash metadata must match",
        ),
        (
            "_comment_evm_source_deployment_transaction_block_number",
            "999999",
            "EVM source deployment transaction block number metadata must match",
        ),
        (
            "_comment_evm_source_deployment_transaction_input_sha256",
            "0x" + "00" * 32,
            "EVM source deployment transaction input SHA-256 metadata must be a non-zero",
        ),
    ]
    for field, value, expected in cases:
        records = complete_bundle(module)
        eth_material = records["sccp_source_verifier_materials"][0]
        eth_material[field] = value

        summary = module.validate_evidence_bundle(records)

        assert summary["production_ready"] is False
        assert expected in "\n".join(summary["blockers"])


def test_all_lanes_rejects_evm_source_with_invalid_runtime_bytecode_metadata():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_material = records["sccp_source_verifier_materials"][0]
    eth_material["_comment_evm_source_bridge_runtime_bytecode_hex"] = "0xxyz"

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert "EVM source bridge runtime bytecode metadata is invalid" in "\n".join(
        summary["blockers"]
    )


def test_all_lanes_rejects_evm_source_noncanonical_runtime_bytecode_metadata():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_material = records["sccp_source_verifier_materials"][0]
    eth_material["_comment_evm_source_bridge_runtime_bytecode_hex"] = (
        "0X" + eth_material["_comment_evm_source_bridge_runtime_bytecode_hex"][2:]
    )

    summary = module.validate_evidence_bundle(records)

    blockers = "\n".join(summary["blockers"])
    assert summary["production_ready"] is False
    assert "EVM source bridge runtime bytecode metadata is invalid" in blockers
    assert "must use lowercase 0x prefix" not in blockers


def test_all_lanes_rejects_evm_source_when_runtime_bytecode_hash_drifts():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_material = records["sccp_source_verifier_materials"][0]
    eth_material["_comment_evm_source_bridge_runtime_bytecode_hex"] = (
        "0x6080604052ff"
    )

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert (
        "EVM source bridge runtime bytecode hash must match "
        "bridge runtime code hash metadata"
    ) in "\n".join(summary["blockers"])


def test_all_lanes_accepts_verified_evm_source_live_toml(tmp_path):
    module = load_evidence_module()
    live_module = load_evm_source_live_module()
    records = complete_bundle(module)
    profile = module.LANE_PROFILES[module.SCCP_DOMAIN_ETH]
    eth_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_ETH)
    material = records["sccp_source_verifier_materials"][eth_index]
    deployment = records["sccp_source_adapter_engine_deployments"][eth_index]
    fake = fake_evm_source_live_opener(live_module, domain=module.SCCP_DOMAIN_ETH)

    expected_args = module._evm_source_bridge_args(material, deployment)
    expected_args.bridge_address = raw_hex(fake.bridge)
    expected_args.source_bridge_emitter_code_hash = fake.bridge_code_hash
    eth_module = module._load_sibling_module("sccp_eth_source_bridge_evidence.py")
    expected_material_hash = eth_module.eth_source_verifier_material_record_hash(
        expected_args
    )
    expected_deployment_hash = (
        eth_module.eth_source_adapter_engine_deployment_record_hash(expected_args)
    )
    live_summary = live_module.collect_live_evidence(
        SimpleNamespace(
            rpc_url="https://ethereum.example",
            domain=module.SCCP_DOMAIN_ETH,
            bridge_address=fake.bridge,
            expected_rpc_chain_id=1,
            expected_source_bridge_code_hash=fake.bridge_code_hash,
            deployment_transaction_hash=bytes.fromhex("de" * 32),
            source_trust_anchor_hash=raw_hex(material["source_trust_anchor_hash"]),
            consensus_verifier_hash=raw_hex(material["consensus_verifier_hash"]),
            message_inclusion_verifier_hash=raw_hex(
                material["message_inclusion_verifier_hash"]
            ),
            finality_policy_hash=raw_hex(material["finality_policy_hash"]),
            adapter_verifier_vk_hash=raw_hex(deployment["adapter_verifier_vk_hash"]),
            deployment_receipt_hash=raw_hex(deployment["deployment_receipt_hash"]),
            expected_source_verifier_material_hash=expected_material_hash,
            expected_source_adapter_engine_deployment_hash=expected_deployment_hash,
            block_tag="finalized",
            timeout=1.0,
        ),
        opener=fake.opener,
    )
    source_toml = live_module.render_offline_toml(live_summary)
    assert '# sccp_evm_source_block_tag = "finalized"' in source_toml
    assert '# sccp_evm_source_rpc_chain_id = "1"' in source_toml
    assert (
        '# sccp_evm_source_bridge_runtime_code_hash = "0x'
        + fake.bridge_code_hash.hex()
        + '"'
        in source_toml
    )
    assert (
        '# sccp_evm_source_bridge_runtime_bytecode_hex = "0x'
        + fake.bridge_runtime.hex()
        + '"'
        in source_toml
    )
    assert '# sccp_evm_source_deployment_receipt_status = "0x1"' in source_toml
    assert (
        '# sccp_evm_source_deployment_transaction_block_hash = "0x'
        + "99" * 32
        + '"'
        in source_toml
    )
    assert (
        '# sccp_evm_source_deployment_transaction_block_number = "4660"'
        in source_toml
    )
    assert (
        '# sccp_evm_source_deployment_transaction_input_sha256 = "0x'
        + hashlib.sha256(fake.deployment_input).hexdigest()
        + '"'
        in source_toml
    )
    assert '# sccp_evm_source_deployment_block_number = "4660"' in source_toml
    assert "# sccp_evm_source_deployment_block_receipts_root" in source_toml

    source_path = tmp_path / "eth-source-live.toml"
    source_path.write_text(source_toml, encoding="utf-8")
    source_records = module.load_evidence_bundle([source_path])
    replacement_material = source_records["sccp_source_verifier_materials"][0]
    replacement_deployment = source_records[
        "sccp_source_adapter_engine_deployments"
    ][0]

    records["sccp_source_verifier_materials"][eth_index] = replacement_material
    records["sccp_source_adapter_engine_deployments"][eth_index] = (
        replacement_deployment
    )
    source_hashes = module._canonical_source_record_hashes(
        profile,
        replacement_material,
        replacement_deployment,
    )
    destination_hash = raw_hex(
        records["sccp_destination_rollouts"][eth_index]["destination_binding_hash"]
    )
    route_allowlist_hash = (
        "0x"
        + module.route_allowlist_hash_for_lane_evidence(
            profile,
            raw_hex(source_hashes["source_verifier_material_hash"]),
            raw_hex(source_hashes["source_adapter_engine_deployment_hash"]),
            destination_hash,
        ).hex()
    )
    route = records["sccp_route_allowlists"][eth_index]
    destination = records["sccp_destination_rollouts"][eth_index]
    evm_destination_module = module._load_sibling_module(
        "sccp_evm_destination_evidence.py"
    )
    route["route_allowlist_hash"] = route_allowlist_hash
    route["_comment_route_canary_status"] = "passed"
    route["_comment_route_canary_route_allowlist_hash"] = route_allowlist_hash
    route["_comment_route_canary_destination_binding_hash"] = (
        destination["destination_binding_hash"]
    )
    route["_comment_route_canary_evidence_hash"] = (
        "0x"
        + evm_destination_module.evm_route_canary_transaction_evidence_hash(
            route_allowlist_hash=raw_hex(route_allowlist_hash),
            bridge_address=raw_hex(destination["destination_bridge_address"]),
            transaction_hash=raw_hex(route["_comment_evm_route_canary_transaction_hash"]),
            log_index=int(route["_comment_evm_route_canary_log_index"]),
            receipt_block_number=int(
                route["_comment_evm_route_canary_receipt_block_number"]
            ),
            receipt_block_hash=raw_hex(
                route["_comment_evm_route_canary_receipt_block_hash"]
            ),
            block_receipts_root=raw_hex(
                route["_comment_evm_route_canary_block_receipts_root"]
            ),
            call_data_sha256=raw_hex(
                route["_comment_evm_route_canary_call_data_sha256"]
            ),
            message_id=raw_hex(route["_comment_evm_route_canary_message_id"]),
            payload_hash=raw_hex(route["_comment_evm_route_canary_payload_hash"]),
            source_domain=module.SCCP_DOMAIN_SORA,
            target_domain=module.SCCP_DOMAIN_ETH,
            commitment_root=raw_hex(
                route["_comment_evm_route_canary_commitment_root"]
            ),
            finality_height=raw_hex(route["_comment_evm_route_canary_finality_height"]),
            finality_block_hash=raw_hex(
                route["_comment_evm_route_canary_finality_block_hash"]
            ),
            statement_hash=raw_hex(route["_comment_evm_route_canary_statement_hash"]),
            proof_version=int(route["_comment_evm_route_canary_proof_version"]),
            proof_source_domain=int(
                route["_comment_evm_route_canary_proof_source_domain"]
            ),
            destination_binding_hash=raw_hex(destination["destination_binding_hash"]),
            verifier_backend_hash=raw_hex(
                destination["_comment_evm_verifier_backend_hash"]
            ),
            proof_family_hash=raw_hex(destination["_comment_evm_proof_family_hash"]),
            network_id=raw_hex(destination["destination_network_id"]),
            used_message_proof=True,
            receipt_block_finalized=True,
        ).hex()
    )

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is True
    eth_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    assert eth_lane["source_record_hashes"] == {
        "source_verifier_material_hash": "0x" + expected_material_hash.hex(),
        "source_adapter_engine_deployment_hash": "0x" + expected_deployment_hash.hex(),
    }


def test_all_lanes_accepts_direct_evm_source_toml_with_audited_metadata(tmp_path):
    module = load_evidence_module()
    records = complete_bundle(module)
    profile = module.LANE_PROFILES[module.SCCP_DOMAIN_ETH]
    eth_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_ETH)
    material = records["sccp_source_verifier_materials"][eth_index]
    deployment = records["sccp_source_adapter_engine_deployments"][eth_index]
    eth_module = module._load_sibling_module("sccp_eth_source_bridge_evidence.py")

    source_hashes = module._canonical_source_record_hashes(
        profile,
        material,
        deployment,
    )
    args = module._evm_source_bridge_args(material, deployment)
    args.expected_source_verifier_material_hash = raw_hex(
        source_hashes["source_verifier_material_hash"]
    )
    args.expected_source_adapter_engine_deployment_hash = raw_hex(
        source_hashes["source_adapter_engine_deployment_hash"]
    )
    args.deployment_transaction_hash = raw_hex(
        material["_comment_evm_source_deployment_transaction_hash"]
    )
    args.deployment_transaction_block_hash = raw_hex(
        material["_comment_evm_source_deployment_transaction_block_hash"]
    )
    args.deployment_transaction_block_number = int(
        material["_comment_evm_source_deployment_transaction_block_number"]
    )
    args.deployment_transaction_input_sha256 = raw_hex(
        material["_comment_evm_source_deployment_transaction_input_sha256"]
    )
    args.deployment_receipt_contract_address = raw_hex(
        material["_comment_evm_source_deployment_contract_address"]
    )
    args.deployment_receipt_block_hash = raw_hex(
        material["_comment_evm_source_deployment_block_hash"]
    )
    args.deployment_receipt_block_number = int(
        material["_comment_evm_source_deployment_block_number"]
    )
    args.deployment_receipt_block_receipts_root = raw_hex(
        material["_comment_evm_source_deployment_block_receipts_root"]
    )
    args.source_bridge_runtime_bytecode_hex = eth_module.parse_runtime_bytecode_hex(
        material["_comment_evm_source_bridge_runtime_bytecode_hex"],
        label="source bridge runtime bytecode",
    )

    source_toml = eth_module.render_toml(args)
    assert '# sccp_evm_source_rpc_chain_id = "1"' in source_toml
    assert '# sccp_evm_source_block_tag = "finalized"' in source_toml
    assert "# sccp_evm_source_bridge_runtime_code_hash" in source_toml
    assert "# sccp_evm_source_bridge_runtime_bytecode_hex" in source_toml
    assert "# sccp_evm_source_deployment_transaction_hash" in source_toml
    assert "# sccp_evm_source_deployment_transaction_block_hash" in source_toml
    assert "# sccp_evm_source_deployment_transaction_block_number" in source_toml
    assert "# sccp_evm_source_deployment_transaction_input_sha256" in source_toml
    assert "# sccp_evm_source_deployment_block_number" in source_toml
    assert "# sccp_evm_source_deployment_block_receipts_root" in source_toml
    source_path = tmp_path / "eth-source-direct.toml"
    source_path.write_text(source_toml, encoding="utf-8")
    source_records = module.load_evidence_bundle([source_path])
    records["sccp_source_verifier_materials"][eth_index] = source_records[
        "sccp_source_verifier_materials"
    ][0]
    records["sccp_source_adapter_engine_deployments"][eth_index] = source_records[
        "sccp_source_adapter_engine_deployments"
    ][0]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is True
    eth_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    assert eth_lane["blockers"] == []


def test_all_lanes_accepts_verified_tron_live_full_toml(tmp_path):
    module = load_evidence_module()
    live_module = load_tron_live_module()
    fake = fake_tron_live_opener(live_module)
    source_trust_anchor_hash = bytes.fromhex("44" * 32)
    consensus_verifier_hash = bytes.fromhex("55" * 32)
    message_inclusion_verifier_hash = bytes.fromhex("66" * 32)
    finality_policy_hash = bytes.fromhex("88" * 32)
    deployment_receipt_hash = bytes.fromhex("aa" * 32)
    record_args = SimpleNamespace(
        source_domain=module.SCCP_DOMAIN_TRON,
        target_domain=module.SCCP_DOMAIN_SORA,
        bridge_address=fake.bridge20,
        owner_address=fake.owner20,
        network_id=fake.network_id,
        source_trust_anchor_hash=source_trust_anchor_hash,
        consensus_verifier_hash=consensus_verifier_hash,
        message_inclusion_verifier_hash=message_inclusion_verifier_hash,
        source_bridge_emitter_code_hash=fake.source_code_hash,
        finality_policy_hash=finality_policy_hash,
        adapter_verifier_vk_hash=None,
        deployment_receipt_hash=deployment_receipt_hash,
    )
    live_module.evidence.apply_source_adapter_verifier_vk_hash(record_args)
    expected_material_hash = live_module.evidence.tron_source_verifier_material_record_hash(
        record_args,
        fake.source_config,
    )
    expected_deployment_hash = (
        live_module.evidence.tron_source_adapter_engine_deployment_record_hash(
            record_args,
            fake.source_config,
        )
    )
    expected_gate_hash = live_module.evidence.tron_dpos_source_gate_hash(
        record_args,
        fake.source_config,
    )
    route_allowlist_hash = module.route_allowlist_hash_for_lane_evidence(
        module.LANE_PROFILES[module.SCCP_DOMAIN_TRON],
        expected_material_hash,
        expected_deployment_hash,
        fake.destination_binding,
    )
    live_summary = live_module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=fake.bridge,
            destination_verifier_address=fake.destination,
            caller_address=None,
            no_getcontract=False,
            timeout=1.0,
            tron_pro_api_key=None,
            tron_pro_api_key_file=None,
            solid=False,
            source_trust_anchor_hash=source_trust_anchor_hash,
            consensus_verifier_hash=consensus_verifier_hash,
            message_inclusion_verifier_hash=message_inclusion_verifier_hash,
            source_bridge_emitter_code_hash=fake.source_code_hash,
            expected_source_bridge_config_hash=fake.source_config,
            finality_policy_hash=finality_policy_hash,
            deployment_receipt_hash=deployment_receipt_hash,
            adapter_verifier_vk_hash=None,
            expected_source_verifier_material_hash=expected_material_hash,
            expected_source_adapter_engine_deployment_hash=expected_deployment_hash,
            expected_tron_dpos_source_gate_hash=expected_gate_hash,
            expected_destination_binding_hash=fake.destination_binding,
            route_allowlist_hash=route_allowlist_hash,
            route_canary_evidence_hash=None,
            route_canary_transaction_id=bytes.fromhex(fake.route_canary_transaction_id),
        ),
        opener=fake.opener,
    )
    tron_toml = live_module.render_offline_full_toml(live_summary)
    assert "# sccp_tron_source_bridge_address" in tron_toml
    assert "# sccp_tron_source_bridge_runtime_bytecode_hex" in tron_toml
    assert "# sccp_tron_source_bridge_config_hash" in tron_toml
    assert "# sccp_tron_destination_verifier_runtime_code_hash" in tron_toml
    assert "# sccp_tron_destination_verifier_runtime_bytecode_hex" in tron_toml
    assert "# sccp_tron_destination_verifier_key_hash" in tron_toml
    assert "# sccp_tron_route_canary_transaction_id" in tron_toml
    assert "tron_route_canary_transaction_id = " in tron_toml
    assert "# sccp_tron_route_canary_transaction_owner_address" in tron_toml
    assert "tron_route_canary_transaction_owner_address = " in tron_toml
    assert '# sccp_tron_route_canary_block_number = "234"' in tron_toml
    assert '# sccp_tron_route_canary_block_timestamp = "567000"' in tron_toml
    assert "tron_route_canary_used_message_proof = true" in tron_toml
    assert "tron_route_canary_raw_data_owner_matches_transaction = true" in tron_toml
    assert "tron_route_canary_signature_recovers_to_owner = true" in tron_toml
    assert "# sccp_tron_route_canary_signature_sha256" in tron_toml
    assert "# sccp_tron_route_canary_signature_recovered_address" in tron_toml
    assert "tron_route_canary_signature_recovers_to_owner = true" in tron_toml
    assert (
        hashlib.sha256(tron_toml.encode("utf-8")).hexdigest()
        == live_summary["offline_full_toml_sha256"]
    )
    tron_path = tmp_path / "tron-live.toml"
    tron_path.write_text(tron_toml, encoding="utf-8")
    tron_records = module.load_evidence_bundle([tron_path])

    records = complete_bundle(module)
    for section in module.SECTION_NAMES:
        records[section] = [
            record
            for record in records[section]
            if record.get("source_domain", record.get("domain"))
            != module.SCCP_DOMAIN_TRON
        ]
        records[section].extend(tron_records[section])

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is True
    tron_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_TRON
    )
    assert tron_lane["blockers"] == []
    assert (
        tron_lane["route_allowlist"]["route_canary"]["transaction_owner_address"]
        == tron_lane["route_allowlist"]["route_canary"]["signature_recovered_address"]
    )
    assert tron_lane["source_record_hashes"] == {
        "source_verifier_material_hash": live_summary["source_records"][
            "source_verifier_material_hash"
        ],
        "source_adapter_engine_deployment_hash": live_summary["source_records"][
            "source_adapter_engine_deployment_hash"
        ],
    }
    assert tron_lane["route_allowlist"]["route_canary"]["evidence_source"] == (
        "tron_message_proof_accepted_transaction"
    )
    assert tron_lane["route_allowlist"]["route_canary"]["message_proof_used"] is True
    assert tron_lane["route_allowlist"]["route_canary"]["block_number"] == 234
    assert tron_lane["route_allowlist"]["route_canary"]["block_timestamp"] == 567000
    assert (
        tron_lane["route_allowlist"]["route_canary"]["transaction_owner_address"]
        == tron_lane["route_allowlist"]["route_canary"]["signature_recovered_address"]
    )
    assert (
        tron_lane["route_allowlist"]["route_canary"][
            "raw_data_owner_matches_transaction"
        ]
        is True
    )
    assert (
        tron_lane["route_allowlist"]["route_canary"]["signature_recovers_to_owner"]
        is True
    )


def test_all_lanes_rejects_tron_route_canary_transaction_metadata_drift():
    module = load_evidence_module()
    records = complete_bundle(module)
    tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TRON)
    route = records["sccp_route_allowlists"][tron_index]
    route["_comment_tron_route_canary_transaction_id"] = hex32(0xFA)
    route["_comment_tron_route_canary_log_index"] = "0"
    route["_comment_tron_route_canary_message_id"] = hex32(0xDD)
    route["_comment_tron_route_canary_statement_hash"] = hex32(0xF1)
    route["_comment_tron_route_canary_commitment_root"] = hex32(0xEE)

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert (
        "TRON route canary evidence hash must match "
        "MessageProofAccepted transaction metadata"
    ) in "\n".join(summary["blockers"])


def test_all_lanes_redacts_tron_route_canary_address_parser_failures(
    monkeypatch,
) -> None:
    """TRON route-canary address blockers must not echo exception payloads."""

    module = load_evidence_module()
    records = complete_bundle(module)
    tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TRON)
    profile = module.LANE_PROFILES[module.SCCP_DOMAIN_TRON]
    material = records["sccp_source_verifier_materials"][tron_index]
    deployment = records["sccp_source_adapter_engine_deployments"][tron_index]
    destination = records["sccp_destination_rollouts"][tron_index]
    route = records["sccp_route_allowlists"][tron_index]
    source_hashes = module._canonical_source_record_hashes(
        profile,
        material,
        deployment,
    )

    for exception_type in (SystemExit, RuntimeError, TypeError, ValueError):

        def fail_address(_value, *, label, exception_type=exception_type):
            raise exception_type(f"secret-token {label} route canary parser")

        monkeypatch.setattr(
            module,
            "_load_sibling_module",
            lambda _name, fail_address=fail_address: SimpleNamespace(
                parse_tron_address=fail_address
            ),
        )

        canary: dict[str, object] = {}
        errors = module._check_tron_route_canary_transaction_evidence(
            route,
            destination_record=destination,
            source_record_hashes=source_hashes,
            evidence_hash=raw_hex(route["_comment_route_canary_evidence_hash"]),
            route_allowlist_hash=raw_hex(route["route_allowlist_hash"]),
            destination_binding_hash=raw_hex(destination["destination_binding_hash"]),
            canary=canary,
        )
        rendered = "\n".join(errors)

        assert "TRON route canary verifier address metadata is invalid" in errors
        assert "metadata is invalid:" not in rendered
        assert "secret-token" not in rendered
        assert exception_type.__name__ not in rendered


def test_all_lanes_rejects_tron_route_canary_call_transcript_metadata_drift():
    module = load_evidence_module()

    for field, value, expected in (
        (
            "_comment_tron_route_canary_call_data_sha256",
            hex32(0x01),
            "TRON route canary evidence hash must match",
        ),
        (
            "_comment_tron_route_canary_payload_hash",
            hex32(0x02),
            "TRON route canary evidence hash must match",
        ),
        (
            "_comment_tron_route_canary_block_number",
            "10006",
            "TRON route canary evidence hash must match",
        ),
        (
            "_comment_tron_route_canary_block_timestamp",
            "1700006",
            "TRON route canary evidence hash must match",
        ),
        (
            "_comment_tron_route_canary_finality_height",
            hex32(0x03),
            "TRON route canary evidence hash must match",
        ),
        (
            "_comment_tron_route_canary_finality_block_hash",
            hex32(0x04),
            "TRON route canary evidence hash must match",
        ),
        (
            "_comment_tron_route_canary_target_domain",
            "6",
            "TRON route canary target domain metadata must be TRON",
        ),
        (
            "_comment_tron_route_canary_proof_version",
            "2",
            "TRON route canary proof version metadata must be 1",
        ),
        (
            "_comment_tron_route_canary_proof_source_domain",
            "1",
            "TRON route canary proof source domain metadata must be SORA",
        ),
    ):
        records = complete_bundle(module)
        tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(
            module.SCCP_DOMAIN_TRON
        )
        route = records["sccp_route_allowlists"][tron_index]
        route[field] = value

        summary = module.validate_evidence_bundle(records)

        assert summary["production_ready"] is False
        assert expected in "\n".join(summary["blockers"])


def test_all_lanes_rejects_tron_route_canary_transcript_hash_reuse():
    module = load_evidence_module()

    for field, source_field in (
        (
            "_comment_tron_route_canary_message_id",
            "_comment_tron_route_canary_transaction_id",
        ),
        (
            "_comment_tron_route_canary_payload_hash",
            "_comment_tron_route_canary_call_data_sha256",
        ),
        (
            "_comment_tron_route_canary_commitment_root",
            "_comment_tron_route_canary_statement_hash",
        ),
        (
            "_comment_tron_route_canary_finality_height",
            "_comment_tron_route_canary_transaction_id",
        ),
        (
            "_comment_tron_route_canary_signature_sha256",
            "_comment_tron_route_canary_finality_block_hash",
        ),
    ):
        records = complete_bundle(module)
        tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(
            module.SCCP_DOMAIN_TRON
        )
        route = records["sccp_route_allowlists"][tron_index]
        route[field] = route[source_field]

        summary = module.validate_evidence_bundle(records)

        assert summary["production_ready"] is False
        assert "TRON route canary transcript hash" in "\n".join(
            summary["blockers"]
        )


def test_all_lanes_rejects_tron_route_canary_governed_hash_role_reuse():
    module = load_evidence_module()
    records = complete_bundle(module)
    tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TRON)
    profile = module.LANE_PROFILES[module.SCCP_DOMAIN_TRON]
    material = records["sccp_source_verifier_materials"][tron_index]
    deployment = records["sccp_source_adapter_engine_deployments"][tron_index]
    source_hashes = module._canonical_source_record_hashes(
        profile,
        material,
        deployment,
    )
    route = records["sccp_route_allowlists"][tron_index]
    route["_comment_tron_route_canary_message_id"] = source_hashes[
        "source_adapter_engine_deployment_hash"
    ]

    summary = module.validate_evidence_bundle(records)

    blockers = "\n".join(summary["blockers"])
    assert summary["production_ready"] is False
    assert (
        "TRON route canary hash role tron_route_canary_message_id must not reuse "
        "source_adapter_engine_deployment_hash"
    ) in blockers


def test_all_lanes_rejects_tron_route_canary_signature_metadata_drift():
    module = load_evidence_module()

    for field, value, expected in (
        (
            "_comment_tron_route_canary_signature_sha256",
            hex32(0x6B),
            "TRON route canary evidence hash must match",
        ),
        (
            "_comment_tron_route_canary_signature_recovered_address",
            "0x41" + "97" * 20,
            "TRON route canary signature recovered address must match transaction owner",
        ),
    ):
        records = complete_bundle(module)
        tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(
            module.SCCP_DOMAIN_TRON
        )
        route = records["sccp_route_allowlists"][tron_index]
        route[field] = value

        summary = module.validate_evidence_bundle(records)

        assert summary["production_ready"] is False
        assert expected in "\n".join(summary["blockers"])


def test_all_lanes_rejects_tron_route_canary_missing_block_metadata():
    module = load_evidence_module()

    for field, expected in (
        (
            "_comment_tron_route_canary_block_number",
            "TRON route canary block number metadata must be a canonical positive decimal",
        ),
        (
            "_comment_tron_route_canary_block_timestamp",
            "TRON route canary block timestamp metadata must be a canonical decimal",
        ),
    ):
        records = complete_bundle(module)
        tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(
            module.SCCP_DOMAIN_TRON
        )
        route = records["sccp_route_allowlists"][tron_index]
        del route[field]

        summary = module.validate_evidence_bundle(records)

        assert summary["production_ready"] is False
        assert expected in "\n".join(summary["blockers"])


def test_all_lanes_rejects_tron_route_canary_missing_used_message_state():
    module = load_evidence_module()
    records = complete_bundle(module)
    tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TRON)
    route = records["sccp_route_allowlists"][tron_index]
    del route["_comment_tron_route_canary_used_message_proof"]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert "TRON route canary usedMessageProofs metadata must be true" in "\n".join(
        summary["blockers"]
    )


def test_all_lanes_rejects_tron_route_canary_missing_raw_data_owner_binding():
    module = load_evidence_module()
    records = complete_bundle(module)
    tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TRON)
    route = records["sccp_route_allowlists"][tron_index]
    del route["_comment_tron_route_canary_raw_data_owner_matches_transaction"]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert "TRON route canary raw_data owner binding metadata must be true" in (
        "\n".join(summary["blockers"])
    )


def test_all_lanes_rejects_tron_route_canary_missing_signature_recovery():
    module = load_evidence_module()
    records = complete_bundle(module)
    tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TRON)
    route = records["sccp_route_allowlists"][tron_index]
    del route["_comment_tron_route_canary_signature_recovers_to_owner"]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert "TRON route canary signature recovery metadata must be true" in (
        "\n".join(summary["blockers"])
    )


def test_all_lanes_rejects_tron_route_canary_bad_signature_recovered_address():
    module = load_evidence_module()
    records = complete_bundle(module)
    tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TRON)
    route = records["sccp_route_allowlists"][tron_index]
    route["_comment_tron_route_canary_signature_recovered_address"] = "0x" + "99" * 20

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert "TRON route canary signature recovered address metadata must be" in (
        "\n".join(summary["blockers"])
    )


def test_all_lanes_rejects_tron_route_canary_non_canonical_hex_metadata():
    module = load_evidence_module()
    records = complete_bundle(module)
    tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TRON)
    route = records["sccp_route_allowlists"][tron_index]
    route["_comment_tron_route_canary_transaction_id"] = (
        "0X" + route["_comment_tron_route_canary_transaction_id"][2:]
    )
    route["_comment_tron_route_canary_signature_sha256"] = (
        "0x" + route["_comment_tron_route_canary_signature_sha256"][2:].upper()
    )

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "TRON route canary transaction id metadata must be a non-zero bytes32" in (
        blockers
    )
    assert "TRON route canary signature hash metadata must be a non-zero bytes32" in (
        blockers
    )


def test_all_lanes_rejects_tron_route_canary_signature_owner_mismatch():
    module = load_evidence_module()
    records = complete_bundle(module)
    tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TRON)
    route = records["sccp_route_allowlists"][tron_index]
    route["_comment_tron_route_canary_transaction_owner_address"] = (
        "0x41" + "98" * 20
    )

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert "TRON route canary signature recovered address must match transaction owner" in (
        "\n".join(summary["blockers"])
    )


def test_all_lanes_rejects_tron_route_canary_transcript_on_non_tron_route():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_route = records["sccp_route_allowlists"][0]
    eth_route["_comment_tron_route_canary_call_data_sha256"] = hex32(0x91)
    eth_route["tron_route_canary_payload_hash"] = hex32(0x92)
    eth_route["_comment_tron_route_canary_target_domain"] = str(
        module.SCCP_DOMAIN_TRON
    )
    eth_route["tron_route_canary_finality_height"] = hex32(0x93)
    eth_route["_comment_tron_route_canary_finality_block_hash"] = hex32(0x94)
    eth_route["tron_route_canary_proof_version"] = 1
    eth_route["_comment_tron_route_canary_proof_source_domain"] = str(
        module.SCCP_DOMAIN_SORA
    )

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert (
        "_comment_tron_route_canary_call_data_sha256 is only valid for "
        "TRON route canary evidence"
    ) in blockers
    assert (
        "tron_route_canary_payload_hash is only valid for TRON route canary evidence"
        in blockers
    )
    assert (
        "_comment_tron_route_canary_target_domain is only valid for "
        "TRON route canary evidence"
    ) in blockers
    assert (
        "tron_route_canary_finality_height is only valid for TRON route canary evidence"
        in blockers
    )
    assert (
        "_comment_tron_route_canary_finality_block_hash is only valid for "
        "TRON route canary evidence"
    ) in blockers
    assert (
        "tron_route_canary_proof_version is only valid for TRON route canary evidence"
        in blockers
    )
    assert (
        "_comment_tron_route_canary_proof_source_domain is only valid for "
        "TRON route canary evidence"
    ) in blockers


def test_all_lanes_rejects_evm_route_canary_transaction_metadata_drift():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_ETH)
    route = records["sccp_route_allowlists"][eth_index]
    route["_comment_evm_route_canary_transaction_hash"] = hex32(0xFA)

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert (
        "EVM route canary evidence hash must match "
        "MessageProofAccepted transaction metadata"
    ) in "\n".join(summary["blockers"])


def test_all_lanes_rejects_evm_route_canary_transaction_readback_drift():
    module = load_evidence_module()
    eth_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_ETH)
    cases = (
        (
            "_comment_evm_route_canary_transaction_block_number",
            "999999",
            "EVM route canary transaction block number metadata must match",
        ),
        (
            "_comment_evm_route_canary_transaction_block_hash",
            hex32(0xFB),
            "EVM route canary transaction block hash metadata must match",
        ),
    )
    for field, value, expected in cases:
        records = complete_bundle(module)
        route = records["sccp_route_allowlists"][eth_index]
        route[field] = value

        summary = module.validate_evidence_bundle(records)

        assert summary["production_ready"] is False
        assert expected in "\n".join(summary["blockers"])


def test_all_lanes_rejects_evm_route_canary_call_transcript_metadata_drift():
    module = load_evidence_module()
    eth_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_ETH)
    cases = (
        (
            "_comment_evm_route_canary_call_data_sha256",
            hex32(0x91),
            "EVM route canary evidence hash must match",
        ),
        (
            "_comment_evm_route_canary_payload_hash",
            hex32(0x92),
            "EVM route canary evidence hash must match",
        ),
        (
            "_comment_evm_route_canary_finality_height",
            hex32(0x93),
            "EVM route canary evidence hash must match",
        ),
        (
            "_comment_evm_route_canary_finality_block_hash",
            hex32(0x94),
            "EVM route canary evidence hash must match",
        ),
        (
            "_comment_evm_route_canary_target_domain",
            str(module.SCCP_DOMAIN_BSC),
            "EVM route canary target domain metadata must match destination rollout",
        ),
        (
            "_comment_evm_route_canary_proof_version",
            "2",
            "EVM route canary proof version metadata must be 1",
        ),
        (
            "_comment_evm_route_canary_proof_source_domain",
            str(module.SCCP_DOMAIN_ETH),
            "EVM route canary proof source domain metadata must be SORA",
        ),
    )
    for field, value, expected in cases:
        records = complete_bundle(module)
        route = records["sccp_route_allowlists"][eth_index]
        route[field] = value

        summary = module.validate_evidence_bundle(records)

        assert summary["production_ready"] is False
        assert expected in "\n".join(summary["blockers"])


def test_all_lanes_rejects_evm_route_canary_transcript_hash_reuse():
    module = load_evidence_module()
    eth_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_ETH)

    for field, source_field in (
        (
            "_comment_evm_route_canary_message_id",
            "_comment_evm_route_canary_transaction_hash",
        ),
        (
            "_comment_evm_route_canary_payload_hash",
            "_comment_evm_route_canary_call_data_sha256",
        ),
        (
            "_comment_evm_route_canary_commitment_root",
            "_comment_evm_route_canary_statement_hash",
        ),
        (
            "_comment_evm_route_canary_finality_height",
            "_comment_evm_route_canary_transaction_hash",
        ),
        (
            "_comment_evm_route_canary_finality_block_hash",
            "_comment_evm_route_canary_transaction_hash",
        ),
    ):
        records = complete_bundle(module)
        route = records["sccp_route_allowlists"][eth_index]
        route[field] = route[source_field]

        summary = module.validate_evidence_bundle(records)

        assert summary["production_ready"] is False
        assert "EVM route canary transcript hash" in "\n".join(
            summary["blockers"]
        )


def test_all_lanes_rejects_evm_route_canary_governed_hash_role_reuse():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_ETH)
    profile = module.LANE_PROFILES[module.SCCP_DOMAIN_ETH]
    material = records["sccp_source_verifier_materials"][eth_index]
    deployment = records["sccp_source_adapter_engine_deployments"][eth_index]
    source_hashes = module._canonical_source_record_hashes(
        profile,
        material,
        deployment,
    )
    route = records["sccp_route_allowlists"][eth_index]
    route["_comment_evm_route_canary_message_id"] = source_hashes[
        "source_verifier_material_hash"
    ]

    summary = module.validate_evidence_bundle(records)

    blockers = "\n".join(summary["blockers"])
    assert summary["production_ready"] is False
    assert (
        "EVM route canary hash role evm_route_canary_message_id must not reuse "
        "source_verifier_material_hash"
    ) in blockers


def test_all_lanes_rejects_evm_route_canary_transcript_on_non_evm_route():
    module = load_evidence_module()
    records = complete_bundle(module)
    tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TRON)
    route = records["sccp_route_allowlists"][tron_index]
    route["_comment_evm_route_canary_call_data_sha256"] = hex32(0x91)
    route["evm_route_canary_payload_hash"] = hex32(0x92)
    route["_comment_evm_route_canary_target_domain"] = str(module.SCCP_DOMAIN_ETH)
    route["evm_route_canary_finality_height"] = hex32(0x93)
    route["_comment_evm_route_canary_finality_block_hash"] = hex32(0x94)
    route["evm_route_canary_proof_version"] = 1
    route["_comment_evm_route_canary_proof_source_domain"] = str(
        module.SCCP_DOMAIN_SORA
    )

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert (
        "_comment_evm_route_canary_call_data_sha256 is only valid for "
        "EVM route canary evidence"
    ) in blockers
    assert (
        "evm_route_canary_payload_hash is only valid for EVM route canary evidence"
        in blockers
    )
    assert (
        "_comment_evm_route_canary_target_domain is only valid for "
        "EVM route canary evidence"
    ) in blockers
    assert (
        "evm_route_canary_finality_height is only valid for EVM route canary evidence"
        in blockers
    )
    assert (
        "_comment_evm_route_canary_finality_block_hash is only valid for "
        "EVM route canary evidence"
    ) in blockers
    assert (
        "evm_route_canary_proof_version is only valid for EVM route canary evidence"
        in blockers
    )
    assert (
        "_comment_evm_route_canary_proof_source_domain is only valid for "
        "EVM route canary evidence"
    ) in blockers


def test_all_lanes_rejects_evm_route_canary_missing_used_message_state():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_ETH)
    route = records["sccp_route_allowlists"][eth_index]
    del route["_comment_evm_route_canary_used_message_proof"]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert "EVM route canary usedMessageProofs metadata must be true" in "\n".join(
        summary["blockers"]
    )


def test_all_lanes_rejects_evm_route_canary_missing_finalized_receipt_state():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_ETH)
    route = records["sccp_route_allowlists"][eth_index]
    del route["_comment_evm_route_canary_receipt_block_finalized"]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert (
        "EVM route canary receipt block finalized metadata must be true"
        in "\n".join(summary["blockers"])
    )


def test_all_lanes_accepts_direct_tron_full_toml_with_audited_metadata(tmp_path):
    module = load_evidence_module()
    records = complete_bundle(module)
    tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TRON)
    profile = module.LANE_PROFILES[module.SCCP_DOMAIN_TRON]
    material = records["sccp_source_verifier_materials"][tron_index]
    deployment = records["sccp_source_adapter_engine_deployments"][tron_index]
    destination = records["sccp_destination_rollouts"][tron_index]
    route = records["sccp_route_allowlists"][tron_index]
    source_hashes = module._canonical_source_record_hashes(profile, material, deployment)
    tron_module = module._load_sibling_module("sccp_tron_source_bridge_evidence.py")
    args = SimpleNamespace(
        source_domain=module.SCCP_DOMAIN_TRON,
        target_domain=module.SCCP_DOMAIN_SORA,
        bridge_address=raw_hex(material["source_bridge_emitter_address"]),
        owner_address=raw_hex(material["source_bridge_owner_address"]),
        network_id=raw_hex(material["source_bridge_network_id"]),
        expected_config_hash=raw_hex(material["source_bridge_config_hash"]),
        source_trust_anchor_hash=raw_hex(material["source_trust_anchor_hash"]),
        consensus_verifier_hash=raw_hex(material["consensus_verifier_hash"]),
        message_inclusion_verifier_hash=raw_hex(
            material["message_inclusion_verifier_hash"]
        ),
        source_bridge_emitter_code_hash=raw_hex(
            material["source_bridge_emitter_code_hash"]
        ),
        source_bridge_runtime_bytecode_hex=tron_module.parse_runtime_bytecode_hex(
            material["_comment_tron_source_bridge_runtime_bytecode_hex"],
            label="source bridge runtime bytecode",
        ),
        source_bridge_runtime_bytecode_file=None,
        finality_policy_hash=raw_hex(material["finality_policy_hash"]),
        adapter_verifier_vk_hash=raw_hex(deployment["adapter_verifier_vk_hash"]),
        deployment_receipt_hash=raw_hex(deployment["deployment_receipt_hash"]),
        expected_source_verifier_material_hash=raw_hex(
            source_hashes["source_verifier_material_hash"]
        ),
        expected_source_adapter_engine_deployment_hash=raw_hex(
            source_hashes["source_adapter_engine_deployment_hash"]
        ),
        destination_verifier_address=destination["verifier_identity"],
        destination_verifier_code_hash=raw_hex(destination["verifier_code_hash"]),
        destination_verifier_runtime_bytecode_hex=(
            tron_module.parse_runtime_bytecode_hex(
                destination[
                    "_comment_tron_destination_verifier_runtime_bytecode_hex"
                ],
                label="destination verifier runtime bytecode",
            )
        ),
        destination_verifier_runtime_bytecode_file=None,
        destination_verifier_key_hash=raw_hex(destination["verifier_key_hash"]),
        destination_source_domain=module.SCCP_DOMAIN_SORA,
        destination_target_domain=module.SCCP_DOMAIN_TRON,
        destination_proof_family=module.SCCP_PROOF_FAMILY_STARK_FRI,
        expected_destination_binding_hash=raw_hex(destination["destination_binding_hash"]),
        route_allowlist_hash=raw_hex(route["route_allowlist_hash"]),
        route_canary_evidence_hash=None,
        route_canary_transaction_id=raw_hex(
            route["_comment_tron_route_canary_transaction_id"]
        ),
        route_canary_transaction_owner_address=raw_hex(
            route["_comment_tron_route_canary_transaction_owner_address"]
        ),
        route_canary_block_number=int(
            route["_comment_tron_route_canary_block_number"]
        ),
        route_canary_block_timestamp=int(
            route["_comment_tron_route_canary_block_timestamp"]
        ),
        route_canary_log_index=int(route["_comment_tron_route_canary_log_index"]),
        route_canary_message_id=raw_hex(route["_comment_tron_route_canary_message_id"]),
        route_canary_call_data_sha256=raw_hex(
            route["_comment_tron_route_canary_call_data_sha256"]
        ),
        route_canary_payload_hash=raw_hex(
            route["_comment_tron_route_canary_payload_hash"]
        ),
        route_canary_target_domain=int(
            route["_comment_tron_route_canary_target_domain"]
        ),
        route_canary_statement_hash=raw_hex(
            route["_comment_tron_route_canary_statement_hash"]
        ),
        route_canary_commitment_root=raw_hex(
            route["_comment_tron_route_canary_commitment_root"]
        ),
        route_canary_finality_height=raw_hex(
            route["_comment_tron_route_canary_finality_height"]
        ),
        route_canary_finality_block_hash=raw_hex(
            route["_comment_tron_route_canary_finality_block_hash"]
        ),
        route_canary_proof_version=int(route["_comment_tron_route_canary_proof_version"]),
        route_canary_proof_source_domain=int(
            route["_comment_tron_route_canary_proof_source_domain"]
        ),
        route_canary_used_message_proof=True,
        route_canary_raw_data_owner_matches_transaction=True,
        route_canary_signature_sha256=raw_hex(
            route["_comment_tron_route_canary_signature_sha256"]
        ),
        route_canary_signature_recovered_address=raw_hex(
            route["_comment_tron_route_canary_signature_recovered_address"]
        ),
        route_canary_signature_recovers_to_owner=True,
    )
    tron_toml = tron_module.render_full_toml(
        args,
        raw_hex(material["source_bridge_config_hash"]),
    )
    assert "# sccp_tron_source_bridge_address" in tron_toml
    assert "# sccp_tron_source_bridge_runtime_bytecode_hex" in tron_toml
    assert "# sccp_tron_destination_verifier_runtime_bytecode_hex" in tron_toml
    assert "# sccp_tron_destination_verifier_key_hash" in tron_toml
    assert "# sccp_tron_route_canary_transaction_id" in tron_toml
    assert "tron_route_canary_transaction_id = " in tron_toml
    assert "# sccp_tron_route_canary_transaction_owner_address" in tron_toml
    assert "tron_route_canary_transaction_owner_address = " in tron_toml
    assert "# sccp_tron_route_canary_block_number" in tron_toml
    assert "tron_route_canary_block_number = " in tron_toml
    assert "# sccp_tron_route_canary_block_timestamp" in tron_toml
    assert "tron_route_canary_block_timestamp = " in tron_toml
    assert "tron_route_canary_used_message_proof = true" in tron_toml
    assert "tron_route_canary_raw_data_owner_matches_transaction = true" in tron_toml
    tron_path = tmp_path / "tron-direct.toml"
    tron_path.write_text(tron_toml, encoding="utf-8")
    tron_records = module.load_evidence_bundle([tron_path])
    tron_route_record = tron_records["sccp_route_allowlists"][0]
    assert tron_route_record["tron_route_canary_log_index"] == 0
    assert (
        tron_route_record["tron_route_canary_transaction_owner_address"]
        == tron_route_record["tron_route_canary_signature_recovered_address"]
    )
    assert tron_route_record["tron_route_canary_block_number"] == int(
        route["_comment_tron_route_canary_block_number"]
    )
    assert tron_route_record["tron_route_canary_block_timestamp"] == int(
        route["_comment_tron_route_canary_block_timestamp"]
    )
    assert tron_route_record["tron_route_canary_used_message_proof"] is True
    assert (
        tron_route_record["tron_route_canary_raw_data_owner_matches_transaction"]
        is True
    )
    assert tron_route_record["tron_route_canary_signature_recovers_to_owner"] is True
    for section in module.SECTION_NAMES:
        records[section] = [
            record
            for record in records[section]
            if record.get("source_domain", record.get("domain"))
            != module.SCCP_DOMAIN_TRON
        ]
        records[section].extend(tron_records[section])

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is True
    tron_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_TRON
    )
    assert tron_lane["blockers"] == []


def test_all_lanes_rejects_tron_records_without_live_metadata():
    module = load_evidence_module()
    records = complete_bundle(module)
    tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TRON)
    material = records["sccp_source_verifier_materials"][tron_index]
    destination = records["sccp_destination_rollouts"][tron_index]
    material.pop("_comment_tron_source_bridge_address")
    material.pop("_comment_tron_source_bridge_code_hash")
    material.pop("_comment_tron_source_bridge_runtime_bytecode_hex")
    material.pop("_comment_tron_source_bridge_config_hash")
    destination.pop("_comment_tron_destination_verifier_address")
    destination.pop("_comment_tron_destination_verifier_code_hash")
    destination.pop("_comment_tron_destination_verifier_runtime_bytecode_hex")
    destination.pop("_comment_tron_destination_verifier_key_hash")

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "TRON source bridge address metadata must be a non-zero" in blockers
    assert "TRON source bridge runtime code hash metadata must be a non-zero" in blockers
    assert "TRON source bridge runtime bytecode metadata must be present" in blockers
    assert "TRON source bridge config hash metadata must be a non-zero" in blockers
    assert "TRON destination verifier address metadata must be a non-zero" in blockers
    assert (
        "TRON destination verifier runtime code hash metadata must be a non-zero"
        in blockers
    )
    assert (
        "TRON destination verifier runtime bytecode metadata must be present"
        in blockers
    )
    assert "TRON destination verifier key hash metadata must be a non-zero" in blockers


def test_all_lanes_rejects_tron_live_comments_on_foreign_lanes():
    module = load_evidence_module()
    records = complete_bundle(module)
    tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TRON)
    eth_material = records["sccp_source_verifier_materials"][0]
    tron_material = records["sccp_source_verifier_materials"][tron_index]
    eth_destination = records["sccp_destination_rollouts"][0]
    tron_destination = records["sccp_destination_rollouts"][tron_index]
    for field in module.TRON_SOURCE_BRIDGE_LIVE_COMMENT_FIELDS:
        eth_material[field] = tron_material[field]
    for field in module.TRON_DESTINATION_VERIFIER_LIVE_COMMENT_FIELDS:
        eth_destination[field] = tron_destination[field]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    for field in module.TRON_SOURCE_BRIDGE_LIVE_COMMENT_FIELDS:
        assert (
            f"domain 1 (eth): {field} is only valid for "
            "TRON source bridge live evidence"
        ) in blockers
    for field in module.TRON_DESTINATION_VERIFIER_LIVE_COMMENT_FIELDS:
        assert (
            f"domain 1 (eth): {field} is only valid for "
            "TRON destination verifier live evidence"
        ) in blockers


def test_all_lanes_rejects_evm_source_comments_on_foreign_lanes():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_material = records["sccp_source_verifier_materials"][0]
    sol_material = records["sccp_source_verifier_materials"][2]
    for field in module.EVM_SOURCE_BRIDGE_LIVE_COMMENT_FIELDS:
        sol_material[field] = eth_material[field]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    for field in module.EVM_SOURCE_BRIDGE_LIVE_COMMENT_FIELDS:
        assert (
            f"domain 3 (sol): {field} is only valid for "
            "EVM source bridge live evidence"
        ) in blockers


def test_all_lanes_rejects_foreign_destination_live_fields():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_destination = records["sccp_destination_rollouts"][0]
    sol_destination = records["sccp_destination_rollouts"][2]
    ton_destination = records["sccp_destination_rollouts"][3]
    tron_destination = records["sccp_destination_rollouts"][4]

    eth_destination["_comment_solana_programdata_address"] = sol_destination[
        "_comment_solana_programdata_address"
    ]
    eth_destination["ton_account_state_hash"] = ton_destination[
        "ton_account_state_hash"
    ]
    sol_destination["_comment_evm_verifier_key_hash"] = eth_destination[
        "_comment_evm_verifier_key_hash"
    ]
    ton_destination["destination_network_id"] = eth_destination[
        "destination_network_id"
    ]
    tron_destination["destination_bridge_address"] = eth_destination[
        "destination_bridge_address"
    ]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert (
        "domain 1 (eth): _comment_solana_programdata_address is only valid "
        "for Solana destination live evidence"
    ) in blockers
    assert (
        "domain 1 (eth): ton_account_state_hash is only valid for "
        "TON destination live evidence"
    ) in blockers
    assert (
        "domain 3 (sol): _comment_evm_verifier_key_hash is only valid for "
        "EVM destination verifier live evidence"
    ) in blockers
    assert (
        "domain 4 (ton): destination_network_id is only valid for "
        "EVM/TRON destination network binding evidence"
    ) in blockers
    assert (
        "domain 5 (tron): destination_bridge_address is only valid for "
        "EVM destination bridge binding evidence"
    ) in blockers


def test_all_lanes_rejects_all_zero_tron_live_address_metadata():
    module = load_evidence_module()
    records = complete_bundle(module)
    tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TRON)
    material = records["sccp_source_verifier_materials"][tron_index]
    destination = records["sccp_destination_rollouts"][tron_index]

    material["_comment_tron_source_bridge_address"] = (
        "T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb"
    )
    material["_comment_tron_source_bridge_config_hash"] = hex32(0)
    destination["_comment_tron_destination_verifier_address"] = (
        "T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb"
    )

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "TRON source bridge address metadata is invalid" in blockers
    assert "TRON source bridge address metadata must be a non-zero" in blockers
    assert "TRON source bridge config hash metadata must be a non-zero" in blockers
    assert "TRON destination verifier address metadata is invalid" in blockers
    assert "TRON destination verifier address metadata must be a non-zero" in blockers


def test_all_lanes_rejects_mismatched_tron_live_config_hash_metadata():
    module = load_evidence_module()
    records = complete_bundle(module)
    tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TRON)
    material = records["sccp_source_verifier_materials"][tron_index]
    material["_comment_tron_source_bridge_config_hash"] = hex32(0xAE)

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert (
        "TRON source bridge config hash metadata must match "
        "source_bridge_config_hash"
    ) in blockers


def test_all_lanes_rejects_tron_invalid_runtime_bytecode_metadata():
    module = load_evidence_module()
    records = complete_bundle(module)
    tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TRON)
    material = records["sccp_source_verifier_materials"][tron_index]
    destination = records["sccp_destination_rollouts"][tron_index]
    material["_comment_tron_source_bridge_runtime_bytecode_hex"] = "0xxyz"
    destination["_comment_tron_destination_verifier_runtime_bytecode_hex"] = "0xxyz"

    summary = module.validate_evidence_bundle(records)

    blockers = "\n".join(summary["blockers"])
    assert summary["production_ready"] is False
    assert "TRON source bridge runtime bytecode metadata is invalid" in blockers
    assert "TRON destination verifier runtime bytecode metadata is invalid" in blockers


def test_all_lanes_redacts_tron_live_metadata_parser_failures(
    monkeypatch,
) -> None:
    """TRON live metadata parser blockers must not echo exception payloads."""

    module = load_evidence_module()
    records = complete_bundle(module)
    tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TRON)
    material = records["sccp_source_verifier_materials"][tron_index]
    destination = records["sccp_destination_rollouts"][tron_index]
    real_tron_module = module._load_sibling_module(
        "sccp_tron_source_bridge_evidence.py"
    )

    for exception_type in (SystemExit, RuntimeError, TypeError, ValueError):

        def fail_address(_value, *, label, exception_type=exception_type):
            raise exception_type(f"secret-token {label} address parser")

        def fail_runtime(_value, *, label, exception_type=exception_type):
            raise exception_type(f"secret-token {label} runtime parser")

        module_attrs = dict(real_tron_module.__dict__)
        module_attrs["parse_tron_address"] = fail_address
        module_attrs["parse_runtime_bytecode_hex"] = fail_runtime

        monkeypatch.setattr(
            module,
            "_load_sibling_module",
            lambda _name, module_attrs=module_attrs: SimpleNamespace(**module_attrs),
        )

        source_errors = module._check_tron_live_source_bridge_evidence(material)
        destination_errors = module._check_tron_live_destination_verifier_evidence(
            destination
        )
        rendered = "\n".join([*source_errors, *destination_errors])

        assert "TRON source bridge address metadata is invalid" in source_errors
        assert (
            "TRON source bridge runtime bytecode metadata is invalid"
            in source_errors
        )
        assert (
            "TRON destination verifier address metadata is invalid"
            in destination_errors
        )
        assert (
            "TRON destination verifier runtime bytecode metadata is invalid"
            in destination_errors
        )
        assert "metadata is invalid:" not in rendered
        assert "secret-token" not in rendered
        assert exception_type.__name__ not in rendered


def test_all_lanes_rejects_padded_tron_live_metadata():
    module = load_evidence_module()
    records = complete_bundle(module)
    tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TRON)
    material = records["sccp_source_verifier_materials"][tron_index]
    destination = records["sccp_destination_rollouts"][tron_index]
    material["_comment_tron_source_bridge_config_hash"] = (
        " " + material["_comment_tron_source_bridge_config_hash"]
    )
    material["_comment_tron_source_bridge_runtime_bytecode_hex"] = (
        " " + material["_comment_tron_source_bridge_runtime_bytecode_hex"]
    )
    destination["_comment_tron_destination_verifier_key_hash"] = (
        " " + destination["_comment_tron_destination_verifier_key_hash"]
    )
    destination["_comment_tron_destination_verifier_runtime_bytecode_hex"] = (
        " " + destination["_comment_tron_destination_verifier_runtime_bytecode_hex"]
    )

    summary = module.validate_evidence_bundle(records)

    blockers = "\n".join(summary["blockers"])
    assert summary["production_ready"] is False
    assert "TRON source bridge config hash metadata must be a non-zero" in blockers
    assert "TRON source bridge runtime bytecode metadata is invalid" in blockers
    assert "TRON destination verifier key hash metadata must be a non-zero" in blockers
    assert "TRON destination verifier runtime bytecode metadata is invalid" in blockers


def test_all_lanes_rejects_padded_tron_structured_hashes():
    module = load_evidence_module()
    records = complete_bundle(module)
    tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TRON)
    material = records["sccp_source_verifier_materials"][tron_index]
    material["source_bridge_network_id"] = " " + material["source_bridge_network_id"]

    summary = module.validate_evidence_bundle(records)

    blockers = "\n".join(summary["blockers"])
    assert summary["production_ready"] is False
    assert "source_bridge_network_id must be a non-zero 32-byte hex value" in blockers

    records = complete_bundle(module)
    destination = records["sccp_destination_rollouts"][tron_index]
    destination["verifier_code_hash"] = " " + destination["verifier_code_hash"]

    summary = module.validate_evidence_bundle(records)

    blockers = "\n".join(summary["blockers"])
    assert summary["production_ready"] is False
    assert "verifier_code_hash must be a non-zero 32-byte hex value" in blockers


def test_all_lanes_rejects_tron_runtime_bytecode_hash_drift():
    module = load_evidence_module()
    records = complete_bundle(module)
    tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TRON)
    material = records["sccp_source_verifier_materials"][tron_index]
    destination = records["sccp_destination_rollouts"][tron_index]
    material["_comment_tron_source_bridge_runtime_bytecode_hex"] = "0x600160ff55"
    destination["_comment_tron_destination_verifier_runtime_bytecode_hex"] = (
        "0x600260ff55"
    )

    summary = module.validate_evidence_bundle(records)

    blockers = "\n".join(summary["blockers"])
    assert summary["production_ready"] is False
    assert (
        "TRON source bridge runtime bytecode hash must match runtime code hash "
        "metadata"
    ) in blockers
    assert (
        "TRON destination verifier runtime bytecode hash must match runtime "
        "code hash metadata"
    ) in blockers


def test_all_lanes_rejects_tron_retired_receipt_root_profile_id():
    module = load_evidence_module()
    records = complete_bundle(module)
    tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TRON)
    material = records["sccp_source_verifier_materials"][tron_index]
    deployment = records["sccp_source_adapter_engine_deployments"][tron_index]
    retired_suffix = "-".join(("receipt-root", "branch-mainnet:v1"))
    retired_profile_id = (
        f"sccp:tron:message-inclusion-verifier:{retired_suffix}"
    )
    material["message_inclusion_verifier_id"] = retired_profile_id
    deployment["message_inclusion_verifier_id"] = retired_profile_id

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert (
        "domain 5 (tron): message_inclusion_verifier_id must be "
        "'sccp:tron:message-inclusion-verifier:transaction-source-mainnet:v1'"
    ) in blockers


def test_all_lanes_accepts_verified_solana_live_toml(tmp_path):
    module = load_evidence_module()
    live_module = load_solana_live_module()
    records = complete_bundle(module)
    sol_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_SOL)
    profile = module.LANE_PROFILES[module.SCCP_DOMAIN_SOL]
    material = records["sccp_source_verifier_materials"][sol_index]
    deployment = records["sccp_source_adapter_engine_deployments"][sol_index]
    source_hashes = module._canonical_source_record_hashes(profile, material, deployment)
    route_allowlist_hash = module.route_allowlist_hash_for_lane_evidence(
        profile,
        raw_hex(source_hashes["source_verifier_material_hash"]),
        raw_hex(source_hashes["source_adapter_engine_deployment_hash"]),
        raw_hex(records["sccp_destination_rollouts"][sol_index]["destination_binding_hash"]),
    )
    program_id = records["sccp_destination_rollouts"][sol_index]["verifier_identity"]
    programdata_address = live_module._encode_solana_base58(bytes.fromhex("11" * 32))
    program_bytes = bytes.fromhex("7f454c460102030405")
    live = live_module.collect_live_evidence(
        "https://solana.example",
        verifier_program_id=program_id,
        opener=fake_solana_live_opener(
            live_module,
            program_id=program_id,
            programdata_address=programdata_address,
            program_bytes=program_bytes,
        ),
        timeout=1.0,
    )
    code_hash = live_module.evidence.solana_verifier_program_code_hash(program_bytes)
    route_canary_evidence_hash = live_module.evidence.solana_route_canary_evidence_hash(
        route_allowlist_hash=route_allowlist_hash,
        destination_binding_hash=raw_hex(
            records["sccp_destination_rollouts"][sol_index]["destination_binding_hash"]
        ),
        source_verifier_material_hash=raw_hex(
            source_hashes["source_verifier_material_hash"]
        ),
        source_adapter_engine_deployment_hash=raw_hex(
            source_hashes["source_adapter_engine_deployment_hash"]
        ),
        verifier_program_id=program_id,
        verifier_code_hash=code_hash,
        rpc_commitment=live["rpc_commitment"],
        program_owner=live["program_owner"],
        programdata_owner=live["programdata_owner"],
        program_immutable=live["program_immutable"],
        program_account_data=base64.b64decode(
            live["program_account_data_base64"],
            validate=True,
        ),
        programdata_address=live["programdata_address"],
        programdata_slot=int(live["programdata_slot"]),
        expected_programdata_slot=4321,
        program_account_context_slot=int(live["program_account_context_slot"]),
        programdata_account_context_slot=int(
            live["programdata_account_context_slot"]
        ),
        programdata_metadata=base64.b64decode(
            live["programdata_metadata_base64"],
            validate=True,
        ),
        programdata_executable=live_module.evidence.parse_program_bytes_base64(
            live["programdata_executable_base64"],
            label="Solana ProgramData executable",
        ),
    )
    args = SimpleNamespace(
        route_allowlist_hash=route_allowlist_hash,
        source_verifier_material_hash=raw_hex(
            source_hashes["source_verifier_material_hash"]
        ),
        source_adapter_engine_deployment_hash=raw_hex(
            source_hashes["source_adapter_engine_deployment_hash"]
        ),
        expected_destination_binding_hash=raw_hex(
            records["sccp_destination_rollouts"][sol_index]["destination_binding_hash"]
        ),
        route_canary_evidence_hash=route_canary_evidence_hash,
        expected_verifier_code_hash=code_hash,
        expected_programdata_address=programdata_address,
        expected_programdata_slot=4321,
    )
    live_summary = live_module._summary(args, live)
    solana_toml = live_module.render_toml(args, live)
    assert live_summary["toml_ready"] is True
    assert live_summary["destination_toml_ready"] is True
    assert live_summary["full_toml_ready"] is True
    assert live_summary["offline_toml_sha256"] == hashlib.sha256(
        live_module.evidence.render_toml(
            live_module._destination_args_from_live(args, live),
            live_module.evidence.solana_destination_binding_hash(),
        ).encode("utf-8")
    ).hexdigest()

    solana_path = tmp_path / "solana-live.toml"
    solana_path.write_text(solana_toml, encoding="utf-8")
    solana_records = module.load_evidence_bundle([solana_path])

    for section in ("sccp_destination_rollouts", "sccp_route_allowlists"):
        records[section] = [
            record
            for record in records[section]
            if record.get("source_domain", record.get("domain"))
            != module.SCCP_DOMAIN_SOL
        ]
        records[section].extend(solana_records[section])

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is True
    solana_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_SOL
    )
    assert solana_lane["blockers"] == []
    assert solana_lane["destination_binding"]["destination_binding_hash"] == (
        live_summary["destination_binding_hash"]
    )
    assert solana_lane["route_allowlist"]["route_allowlist_hash"] == (
        "0x" + route_allowlist_hash.hex()
    )


def test_all_lanes_accepts_direct_solana_destination_toml_with_audited_metadata(
    tmp_path,
):
    module = load_evidence_module()
    records = complete_bundle(module)
    sol_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_SOL)
    profile = module.LANE_PROFILES[module.SCCP_DOMAIN_SOL]
    material = records["sccp_source_verifier_materials"][sol_index]
    deployment = records["sccp_source_adapter_engine_deployments"][sol_index]
    destination = records["sccp_destination_rollouts"][sol_index]
    route = records["sccp_route_allowlists"][sol_index]
    solana_module = module._load_sibling_module("sccp_solana_destination_evidence.py")
    source_hashes = module._canonical_source_record_hashes(profile, material, deployment)
    args = SimpleNamespace(
        verifier_program_id=destination["verifier_identity"],
        verifier_code_hash=raw_hex(destination["verifier_code_hash"]),
        route_allowlist_hash=raw_hex(route["route_allowlist_hash"]),
        source_verifier_material_hash=raw_hex(
            source_hashes["source_verifier_material_hash"]
        ),
        source_adapter_engine_deployment_hash=raw_hex(
            source_hashes["source_adapter_engine_deployment_hash"]
        ),
        route_canary_evidence_hash=raw_hex(route["_comment_route_canary_evidence_hash"]),
        expected_destination_binding_hash=raw_hex(destination["destination_binding_hash"]),
        verifier_program_bytes_hex=None,
        verifier_program_bytes_base64=solana_module.parse_program_bytes_base64(
            destination["_comment_solana_programdata_executable_base64"],
            label="verifier program bytes",
        ),
        verifier_program_bytes_file=None,
        programdata_address=destination["_comment_solana_programdata_address"],
        programdata_slot=int(destination["_comment_solana_programdata_slot"]),
        program_account_context_slot=int(
            destination["_comment_solana_program_account_context_slot"]
        ),
        programdata_account_context_slot=int(
            destination["_comment_solana_programdata_account_context_slot"]
        ),
    )

    solana_toml = solana_module.render_toml(args)
    assert '# sccp_solana_rpc_commitment = "finalized"' in solana_toml
    assert 'solana_rpc_commitment = "finalized"' in solana_toml
    assert 'solana_program_immutable = true' in solana_toml
    assert "solana_programdata_executable_base64 = " in solana_toml
    assert "# sccp_solana_program_account_data_base64" in solana_toml
    assert "# sccp_solana_programdata_address" in solana_toml
    assert "# sccp_solana_programdata_metadata_blake2b256" in solana_toml
    assert "# sccp_solana_programdata_metadata_base64" in solana_toml
    assert "# sccp_solana_programdata_executable_blake2b256" in solana_toml
    assert "# sccp_solana_programdata_executable_base64" in solana_toml
    solana_path = tmp_path / "solana-direct.toml"
    solana_path.write_text(solana_toml, encoding="utf-8")
    solana_records = module.load_evidence_bundle([solana_path])

    for section in ("sccp_destination_rollouts", "sccp_route_allowlists"):
        records[section] = [
            record
            for record in records[section]
            if record.get("source_domain", record.get("domain"))
            != module.SCCP_DOMAIN_SOL
        ]
        records[section].extend(solana_records[section])

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is True
    solana_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_SOL
    )
    assert solana_lane["blockers"] == []


def test_all_lanes_rejects_solana_destination_without_live_programdata_metadata():
    module = load_evidence_module()
    records = complete_bundle(module)
    solana_destination = records["sccp_destination_rollouts"][2]
    solana_destination.pop("_comment_solana_rpc_commitment")
    solana_destination.pop("_comment_solana_program_owner")
    solana_destination.pop("_comment_solana_programdata_owner")
    solana_destination.pop("_comment_solana_program_immutable")
    solana_destination.pop("_comment_solana_program_account_data_len")
    solana_destination.pop("_comment_solana_program_account_data_base64")
    solana_destination.pop("_comment_solana_programdata_address")
    solana_destination.pop("_comment_solana_programdata_slot")
    solana_destination.pop("_comment_solana_expected_programdata_slot")
    solana_destination.pop("_comment_solana_program_account_context_slot")
    solana_destination.pop("_comment_solana_programdata_account_context_slot")
    solana_destination.pop("_comment_solana_programdata_metadata_blake2b256")
    solana_destination.pop("_comment_solana_programdata_metadata_base64")
    solana_destination.pop("_comment_solana_programdata_code_hash")
    solana_destination.pop("_comment_solana_programdata_executable_base64")

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "Solana live RPC commitment metadata must be finalized" in blockers
    assert "Solana verifier program owner metadata must be the BPF upgradeable loader" in blockers
    assert "Solana ProgramData owner metadata must be the BPF upgradeable loader" in blockers
    assert "Solana verifier program immutable metadata must be true" in blockers
    assert "Solana Program account data length metadata must be a positive decimal string" in blockers
    assert "Solana Program account data base64 metadata must be present" in blockers
    assert "Solana live ProgramData account metadata is required" in blockers
    assert "Solana ProgramData slot metadata must be a positive decimal string" in blockers
    assert "Solana expected ProgramData slot metadata must be a positive decimal string" in blockers
    assert (
        "Solana program account RPC context slot metadata must be a positive decimal string"
        in blockers
    )
    assert (
        "Solana ProgramData account RPC context slot metadata must be a positive decimal string"
        in blockers
    )
    assert "Solana ProgramData metadata hash must be a non-zero" in blockers
    assert "Solana ProgramData metadata base64 metadata must be present" in blockers
    assert "Solana ProgramData executable hash metadata must be a non-zero" in blockers
    assert "Solana ProgramData executable base64 metadata must be present" in blockers


def test_all_lanes_rejects_noncanonical_destination_decimal_metadata():
    module = load_evidence_module()
    records = complete_bundle(module)

    sol_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_SOL)
    ton_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TON)

    records["sccp_destination_rollouts"][sol_index][
        "_comment_solana_programdata_slot"
    ] = "01003"
    records["sccp_destination_rollouts"][ton_index][
        "_comment_ton_last_transaction_lt"
    ] = "02004"
    records["sccp_destination_rollouts"][ton_index]["ton_last_transaction_lt"] = (
        "02004"
    )

    summary = module.validate_evidence_bundle(records)
    blockers = "\n".join(
        blocker for lane in summary["lanes"] for blocker in lane["blockers"]
    )

    assert "Solana ProgramData slot metadata must be a positive decimal string" in blockers
    assert "TON last transaction LT metadata must be a positive decimal string" in blockers


def test_all_lanes_rejects_solana_programdata_slot_mismatch():
    module = load_evidence_module()
    records = complete_bundle(module)
    solana_destination = records["sccp_destination_rollouts"][2]
    solana_destination["_comment_solana_programdata_slot"] = "4321"
    solana_destination["_comment_solana_expected_programdata_slot"] = "4322"

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert (
        "Solana ProgramData slot metadata must match expected ProgramData slot"
        in "\n".join(summary["blockers"])
    )


def test_all_lanes_rejects_solana_programdata_program_alias():
    module = load_evidence_module()
    records = complete_bundle(module)
    solana_destination = records["sccp_destination_rollouts"][2]
    solana_destination["_comment_solana_programdata_address"] = solana_destination[
        "verifier_identity"
    ]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert (
        "Solana ProgramData account metadata must differ from verifier_identity"
        in "\n".join(summary["blockers"])
    )


def test_all_lanes_rejects_solana_noncanonical_program_account_length():
    module = load_evidence_module()
    records = complete_bundle(module)
    solana_destination = records["sccp_destination_rollouts"][2]
    solana_destination["_comment_solana_program_account_data_len"] = "37"

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert (
        "Solana Program account data length metadata must be 36 bytes"
        in "\n".join(summary["blockers"])
    )


def test_all_lanes_rejects_solana_account_preimage_drift():
    module = load_evidence_module()
    records = complete_bundle(module)
    solana_destination = records["sccp_destination_rollouts"][2]
    bad_program_account = (
        module.SOLANA_UPGRADEABLE_LOADER_PROGRAM_TAG.to_bytes(4, "little")
        + bytes.fromhex("55" * 32)
    )
    bad_programdata_metadata = (
        module.SOLANA_UPGRADEABLE_LOADER_PROGRAMDATA_TAG.to_bytes(4, "little")
        + int(solana_destination["_comment_solana_programdata_slot"]).to_bytes(
            8,
            "little",
        )
        + b"\x01"
        + bytes.fromhex("66" * 32)
    )
    solana_destination["_comment_solana_program_account_data_base64"] = (
        base64.b64encode(bad_program_account).decode("ascii")
    )
    solana_destination["_comment_solana_programdata_metadata_base64"] = (
        base64.b64encode(bad_programdata_metadata).decode("ascii")
    )

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "Solana Program account data metadata must reference ProgramData" in blockers
    assert "Solana ProgramData metadata base64 hash must match" in blockers
    assert "Solana ProgramData metadata must encode no upgrade authority" in blockers


def test_all_lanes_redacts_solana_live_base64_comment_failures():
    module = load_evidence_module()
    records = complete_bundle(module)
    solana_destination = records["sccp_destination_rollouts"][2]
    solana_destination["_comment_solana_program_account_data_base64"] = "not@@base64"
    solana_destination["_comment_solana_programdata_metadata_base64"] = (
        noncanonical_base64_alias(b"secret-token-metadata!")
    )

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "Solana Program account data base64 metadata is invalid" in blockers
    assert "Solana ProgramData metadata base64 metadata is invalid" in blockers
    assert "secret-token" not in blockers
    assert "must be base64" not in blockers
    assert "canonical base64" not in blockers


def test_all_lanes_base64_helper_redacts_parser_causes(monkeypatch):
    module = load_evidence_module()

    for exception_type in (SystemExit, RuntimeError, TypeError, ValueError):

        def fail_decode(*_args, exception_type=exception_type, **_kwargs):
            raise exception_type("secret-token all-lanes base64")

        with monkeypatch.context() as patch:
            patch.setattr(module.base64, "b64decode", fail_decode)
            try:
                module._decode_canonical_base64(
                    "ignored",
                    label="Solana Program account data base64 metadata",
                )
            except ValueError as exc:
                rendered = str(exc)
                assert rendered == "Solana Program account data base64 metadata must be base64"
                assert "secret-token" not in rendered
                assert "all-lanes base64" not in rendered
                assert exception_type.__name__ not in rendered
                assert exc.__cause__ is None
                assert exc.__suppress_context__ is True
            else:
                raise AssertionError("all-lanes base64 helper accepted invalid base64")


def test_all_lanes_hex_helpers_redact_parser_exit_causes(monkeypatch):
    module = load_evidence_module()

    for exception_type in (SystemExit, RuntimeError, TypeError, ValueError):

        class SecretBytes:
            @staticmethod
            def fromhex(_text, exception_type=exception_type):
                raise exception_type(
                    f"secret-token all-lanes hex {exception_type.__name__} detail"
                )

        with monkeypatch.context() as patch:
            patch.setattr(module, "bytes", SecretBytes, raising=False)

            assert module._hex_bytes("0x" + "11" * 32, byte_length=32) is None
            assert module._exact_hex_bytes("0x" + "22" * 32, byte_length=32) is None

            for helper, field, expected_message in (
                (
                    module._required_hex_bytes,
                    "source_verifier_material_hash",
                    "source_verifier_material_hash must be a 32-byte hex value",
                ),
                (
                    module._required_exact_hex_bytes,
                    "destination_binding_hash",
                    "destination_binding_hash must be an exact 32-byte hex value",
                ),
            ):
                try:
                    helper({field: "0x" + "33" * 32}, field, byte_length=32)
                except ValueError as exc:
                    rendered = str(exc)
                    assert rendered == expected_message
                    assert "secret-token" not in rendered
                    assert exception_type.__name__ not in rendered
                    assert exc.__cause__ is None
                    assert exc.__suppress_context__ is False
                else:
                    raise AssertionError(
                        f"{field} helper {exception_type.__name__} was accepted"
                    )


def test_all_lanes_rejects_solana_programdata_invalid_executable_base64():
    module = load_evidence_module()
    records = complete_bundle(module)
    solana_destination = records["sccp_destination_rollouts"][2]
    solana_destination["_comment_solana_programdata_executable_base64"] = "not@@base64"

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert "Solana ProgramData executable base64 metadata is invalid" in "\n".join(
        summary["blockers"]
    )

    records = complete_bundle(module)
    solana_destination = records["sccp_destination_rollouts"][2]
    solana_destination["_comment_solana_programdata_executable_base64"] = (
        noncanonical_base64_alias(b"\x7fELFsol")
    )

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "Solana ProgramData executable base64 metadata is invalid" in blockers
    assert "canonical base64" not in blockers

    records = complete_bundle(module)
    solana_destination = records["sccp_destination_rollouts"][2]
    solana_destination["_comment_solana_programdata_executable_base64"] = (
        base64.b64encode(b"not-elf").decode("ascii")
    )

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "Solana ProgramData executable base64 metadata is invalid" in blockers
    assert "BPF ELF" not in blockers


def test_all_lanes_redacts_solana_route_canary_base64_comment_failures():
    module = load_evidence_module()
    records = complete_bundle(module)
    sol_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_SOL)
    profile = module.LANE_PROFILES[module.SCCP_DOMAIN_SOL]
    material = records["sccp_source_verifier_materials"][sol_index]
    deployment = records["sccp_source_adapter_engine_deployments"][sol_index]
    destination = records["sccp_destination_rollouts"][sol_index]
    route = records["sccp_route_allowlists"][sol_index]
    destination["_comment_solana_program_account_data_base64"] = "not@@base64"
    destination["_comment_solana_programdata_metadata_base64"] = noncanonical_base64_alias(
        b"secret-token-route-metadata!"
    )
    source_hashes = module._canonical_source_record_hashes(
        profile,
        material,
        deployment,
    )

    errors = module._check_solana_route_canary_live_program_evidence(
        route,
        destination_record=destination,
        source_record_hashes=source_hashes,
        evidence_hash=raw_hex(route["_comment_route_canary_evidence_hash"]),
        route_allowlist_hash=raw_hex(route["route_allowlist_hash"]),
        destination_binding_hash=raw_hex(destination["destination_binding_hash"]),
        canary={},
    )
    rendered = "\n".join(errors)

    assert "Solana route canary Program account data is invalid" in errors
    assert "Solana route canary ProgramData metadata is invalid" in errors
    assert "secret-token" not in rendered
    assert "must be base64" not in rendered
    assert "canonical base64" not in rendered


def test_all_lanes_solana_base64_callers_redact_helper_exit_causes(
    monkeypatch,
) -> None:
    """Solana all-lanes base64 callers must not leak helper exits."""

    module = load_evidence_module()
    records = complete_bundle(module)
    sol_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_SOL)
    profile = module.LANE_PROFILES[module.SCCP_DOMAIN_SOL]
    material = records["sccp_source_verifier_materials"][sol_index]
    deployment = records["sccp_source_adapter_engine_deployments"][sol_index]
    destination = records["sccp_destination_rollouts"][sol_index]
    route = records["sccp_route_allowlists"][sol_index]
    source_hashes = module._canonical_source_record_hashes(
        profile,
        material,
        deployment,
    )

    for exception_type in (SystemExit, RuntimeError, TypeError, ValueError):

        def fail_base64(_value, *, label, exception_type=exception_type):
            raise exception_type(
                f"secret-token all-lanes {label} {exception_type.__name__} detail"
            )

        with monkeypatch.context() as patch:
            patch.setattr(module, "_decode_canonical_base64", fail_base64)

            destination_errors = module._check_solana_live_programdata_evidence(
                destination
            )
            route_errors = module._check_solana_route_canary_live_program_evidence(
                route,
                destination_record=destination,
                source_record_hashes=source_hashes,
                evidence_hash=raw_hex(route["_comment_route_canary_evidence_hash"]),
                route_allowlist_hash=raw_hex(route["route_allowlist_hash"]),
                destination_binding_hash=raw_hex(destination["destination_binding_hash"]),
                canary={},
            )
            rendered = "\n".join([*destination_errors, *route_errors])

            assert (
                "Solana Program account data base64 metadata is invalid"
                in destination_errors
            )
            assert (
                "Solana ProgramData metadata base64 metadata is invalid"
                in destination_errors
            )
            assert "Solana route canary Program account data is invalid" in route_errors
            assert "Solana route canary ProgramData metadata is invalid" in route_errors
            assert "secret-token" not in rendered
            assert exception_type.__name__ not in rendered


def test_all_lanes_redacts_solana_programdata_parser_failures(
    monkeypatch,
) -> None:
    """Solana ProgramData parser blockers must not echo exception payloads."""

    module = load_evidence_module()
    records = complete_bundle(module)
    sol_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_SOL)
    profile = module.LANE_PROFILES[module.SCCP_DOMAIN_SOL]
    material = records["sccp_source_verifier_materials"][sol_index]
    deployment = records["sccp_source_adapter_engine_deployments"][sol_index]
    destination = records["sccp_destination_rollouts"][sol_index]
    route = records["sccp_route_allowlists"][sol_index]
    source_hashes = module._canonical_source_record_hashes(
        profile,
        material,
        deployment,
    )
    real_solana_module = module._load_sibling_module(
        "sccp_solana_destination_evidence.py"
    )

    for exception_type in (SystemExit, RuntimeError, TypeError, ValueError):

        def fail_program_id(_value, *, label, exception_type=exception_type):
            raise exception_type(f"secret-token {label} program id")

        def fail_program_bytes(_value, *, label, exception_type=exception_type):
            raise exception_type(f"secret-token {label} program bytes")

        def load_solana_module(
            _name,
            fail_program_id=fail_program_id,
            fail_program_bytes=fail_program_bytes,
        ):
            return SimpleNamespace(
                _require_solana_program_id=fail_program_id,
                decode_solana_base58=fail_program_id,
                parse_program_bytes_base64=fail_program_bytes,
                solana_verifier_program_code_hash=lambda _program: bytes(32),
                solana_route_canary_evidence_hash=lambda **_kwargs: bytes(32),
            )

        monkeypatch.setattr(module, "_load_sibling_module", load_solana_module)

        destination_errors = module._check_solana_live_programdata_evidence(destination)
        canary: dict[str, object] = {}
        route_errors = module._check_solana_route_canary_live_program_evidence(
            route,
            destination_record=destination,
            source_record_hashes=source_hashes,
            evidence_hash=raw_hex(route["_comment_route_canary_evidence_hash"]),
            route_allowlist_hash=raw_hex(route["route_allowlist_hash"]),
            destination_binding_hash=raw_hex(destination["destination_binding_hash"]),
            canary=canary,
        )
        rendered = "\n".join([*destination_errors, *route_errors])

        assert "Solana ProgramData account is not canonical" in destination_errors
        assert (
            "Solana ProgramData executable base64 metadata is invalid"
            in destination_errors
        )
        assert "Solana route canary ProgramData executable is invalid" in route_errors
        assert "metadata is invalid:" not in rendered
        assert "is not canonical:" not in rendered
        assert "secret-token" not in rendered
        assert exception_type.__name__ not in rendered

    for exception_type in (SystemExit, RuntimeError, TypeError, ValueError):

        def fail_canary_hash(**_kwargs):
            raise exception_type("secret-token route canary live program detail")

        module_attrs = dict(real_solana_module.__dict__)
        module_attrs["solana_route_canary_evidence_hash"] = fail_canary_hash
        monkeypatch.setattr(
            module,
            "_load_sibling_module",
            lambda _name, module_attrs=module_attrs: SimpleNamespace(**module_attrs),
        )

        route_errors = module._check_solana_route_canary_live_program_evidence(
            route,
            destination_record=destination,
            source_record_hashes=source_hashes,
            evidence_hash=raw_hex(route["_comment_route_canary_evidence_hash"]),
            route_allowlist_hash=raw_hex(route["route_allowlist_hash"]),
            destination_binding_hash=raw_hex(destination["destination_binding_hash"]),
            canary={},
        )
        rendered = "\n".join(route_errors)

        assert "Solana route canary live program metadata is invalid" in route_errors
        assert "metadata is invalid:" not in rendered
        assert "secret-token" not in rendered
        assert exception_type.__name__ not in rendered


def test_all_lanes_rejects_solana_programdata_executable_hash_drift():
    module = load_evidence_module()
    records = complete_bundle(module)
    solana_destination = records["sccp_destination_rollouts"][2]
    solana_destination["_comment_solana_programdata_executable_base64"] = (
        base64.b64encode(b"\x7fELFsolana-drift").decode("ascii")
    )

    summary = module.validate_evidence_bundle(records)

    blockers = "\n".join(summary["blockers"])
    assert summary["production_ready"] is False
    assert (
        "Solana ProgramData executable base64 hash must match ProgramData "
        "executable hash metadata"
    ) in blockers


def test_all_lanes_rejects_confirmed_solana_live_evidence():
    module = load_evidence_module()
    records = complete_bundle(module)
    solana_destination = records["sccp_destination_rollouts"][2]
    solana_destination["_comment_solana_rpc_commitment"] = "confirmed"

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert (
        "Solana live RPC commitment metadata must be finalized"
        in "\n".join(summary["blockers"])
    )


def test_all_lanes_rejects_mutable_solana_live_metadata():
    module = load_evidence_module()
    records = complete_bundle(module)
    solana_destination = records["sccp_destination_rollouts"][2]
    solana_destination["_comment_solana_program_owner"] = "11111111111111111111111111111111"
    solana_destination["_comment_solana_programdata_owner"] = "11111111111111111111111111111111"
    solana_destination["_comment_solana_program_immutable"] = "false"

    summary = module.validate_evidence_bundle(records)

    blockers = "\n".join(summary["blockers"])
    assert summary["production_ready"] is False
    assert "Solana verifier program owner metadata must be the BPF upgradeable loader" in blockers
    assert "Solana ProgramData owner metadata must be the BPF upgradeable loader" in blockers
    assert "Solana verifier program immutable metadata must be true" in blockers


def test_all_lanes_rejects_stale_solana_programdata_context_slot():
    module = load_evidence_module()
    records = complete_bundle(module)
    solana_destination = records["sccp_destination_rollouts"][2]
    solana_destination["_comment_solana_programdata_slot"] = "4321"
    solana_destination["_comment_solana_expected_programdata_slot"] = "4321"
    solana_destination["_comment_solana_programdata_account_context_slot"] = "4000"

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert (
        "Solana ProgramData account RPC context slot must be at or after "
        "ProgramData deployment slot"
        in "\n".join(summary["blockers"])
    )


def test_all_lanes_rejects_stale_solana_program_context_slot():
    module = load_evidence_module()
    records = complete_bundle(module)
    solana_destination = records["sccp_destination_rollouts"][2]
    solana_destination["_comment_solana_programdata_slot"] = "4321"
    solana_destination["_comment_solana_expected_programdata_slot"] = "4321"
    solana_destination["_comment_solana_program_account_context_slot"] = "4000"

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert (
        "Solana program account RPC context slot must be at or after "
        "ProgramData deployment slot"
        in "\n".join(summary["blockers"])
    )


def test_all_lanes_rejects_solana_route_canary_live_program_hash_drift():
    module = load_evidence_module()
    records = complete_bundle(module)
    sol_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_SOL)
    sol_route = records["sccp_route_allowlists"][sol_index]
    drifted = bytearray(raw_hex(sol_route["_comment_route_canary_evidence_hash"]))
    drifted[0] ^= 0x01
    sol_route["_comment_route_canary_evidence_hash"] = "0x" + bytes(drifted).hex()

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert (
        "Solana route canary evidence hash must match live program metadata"
        in "\n".join(summary["blockers"])
    )


def test_all_lanes_rejects_solana_route_canary_verifier_code_hash_role_reuse():
    module = load_evidence_module()
    records = complete_bundle(module)
    sol_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_SOL)
    profile = module.LANE_PROFILES[module.SCCP_DOMAIN_SOL]
    material = records["sccp_source_verifier_materials"][sol_index]
    deployment = records["sccp_source_adapter_engine_deployments"][sol_index]
    source_hashes = module._canonical_source_record_hashes(
        profile,
        material,
        deployment,
    )
    solana_destination = records["sccp_destination_rollouts"][sol_index]
    solana_destination["verifier_code_hash"] = source_hashes[
        "source_verifier_material_hash"
    ]

    summary = module.validate_evidence_bundle(records)

    blockers = "\n".join(summary["blockers"])
    assert summary["production_ready"] is False
    assert (
        "Solana route canary hash role verifier_code_hash must not reuse "
        "source_verifier_material_hash"
    ) in blockers


def test_all_lanes_rejects_solana_programdata_field_comment_drift():
    module = load_evidence_module()
    records = complete_bundle(module)
    sol_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_SOL)
    sol_destination = records["sccp_destination_rollouts"][sol_index]
    sol_destination["solana_programdata_address"] = (
        "3JF3sEqM796hk5WFqA6EtmEwJQ9quALszsfJyvXNQKy3"
    )

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert (
        "Solana ProgramData address field must match "
        "_comment_solana_programdata_address comment"
        in "\n".join(summary["blockers"])
    )


def test_all_lanes_accepts_verified_ton_live_toml(tmp_path):
    module = load_evidence_module()
    live_module = load_ton_live_module()
    records = complete_bundle(module)
    ton_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TON)
    profile = module.LANE_PROFILES[module.SCCP_DOMAIN_TON]
    material = records["sccp_source_verifier_materials"][ton_index]
    deployment = records["sccp_source_adapter_engine_deployments"][ton_index]
    source_hashes = module._canonical_source_record_hashes(profile, material, deployment)
    verifier = records["sccp_destination_rollouts"][ton_index]["verifier_identity"]
    code_hash = bytes.fromhex(TON_CODE_BOC_ROOT_HASH)
    account_state_hash = bytes.fromhex("55" * 32)
    fake = fake_ton_live_opener(
        live_module,
        verifier_contract_address=verifier,
        code_hash=code_hash,
        account_state_hash=account_state_hash,
    )
    live = live_module.collect_live_evidence(
        "https://toncenter.example",
        verifier_contract_address=verifier,
        opener=fake.opener,
        timeout=1.0,
    )
    destination_binding_hash = raw_hex(
        records["sccp_destination_rollouts"][ton_index]["destination_binding_hash"]
    )
    route_allowlist_hash = module.route_allowlist_hash_for_lane_evidence(
        profile,
        raw_hex(source_hashes["source_verifier_material_hash"]),
        raw_hex(source_hashes["source_adapter_engine_deployment_hash"]),
        destination_binding_hash,
    )
    route_canary_evidence_hash = live_module.evidence.ton_route_canary_evidence_hash(
        route_allowlist_hash=route_allowlist_hash,
        destination_binding_hash=destination_binding_hash,
        source_verifier_material_hash=raw_hex(
            source_hashes["source_verifier_material_hash"]
        ),
        source_adapter_engine_deployment_hash=raw_hex(
            source_hashes["source_adapter_engine_deployment_hash"]
        ),
        verifier_contract_address=verifier,
        verifier_code_hash=code_hash,
        account_status="active",
        account_state_hash=account_state_hash,
        last_transaction_lt="123456",
        last_transaction_hash=fake.last_transaction_hash,
        verifier_code_boc_root_hash=code_hash,
    )
    args = SimpleNamespace(
        route_allowlist_hash=route_allowlist_hash,
        source_verifier_material_hash=raw_hex(
            source_hashes["source_verifier_material_hash"]
        ),
        source_adapter_engine_deployment_hash=raw_hex(
            source_hashes["source_adapter_engine_deployment_hash"]
        ),
        expected_destination_binding_hash=destination_binding_hash,
        route_canary_evidence_hash=route_canary_evidence_hash,
        expected_verifier_code_hash=code_hash,
        expected_account_state_hash=account_state_hash,
    )
    live_summary = live_module._summary(args, live)
    ton_toml = live_module.render_toml(args, live)
    assert live_summary["destination_toml_ready"] is True
    assert live_summary["full_toml_ready"] is True
    assert live_summary["toml_ready"] is True
    assert '# sccp_ton_account_status = "active"' in ton_toml
    assert '# sccp_ton_account_state_hash = "0x' + "55" * 32 + '"' in ton_toml
    assert '# sccp_ton_code_hash = "0x' + TON_CODE_BOC_ROOT_HASH + '"' in ton_toml
    assert (
        '# sccp_ton_code_boc_root_hash = "0x'
        + TON_CODE_BOC_ROOT_HASH
        + '"'
        in ton_toml
    )
    assert '# sccp_ton_code_boc_base64 = "' + TON_CODE_BOC_BASE64 + '"' in ton_toml
    assert '# sccp_ton_code_boc_hash_matches = "true"' in ton_toml
    assert 'ton_account_status = "active"' in ton_toml
    assert 'ton_account_state_hash = "0x' + "55" * 32 + '"' in ton_toml
    assert 'ton_verifier_code_boc_root_hash = "0x' + TON_CODE_BOC_ROOT_HASH + '"' in ton_toml
    assert 'ton_verifier_code_boc = "0x' + TON_CODE_BOC_HEX + '"' in ton_toml

    ton_path = tmp_path / "ton-live.toml"
    ton_path.write_text(ton_toml, encoding="utf-8")
    ton_records = module.load_evidence_bundle([ton_path])

    for section in ("sccp_destination_rollouts", "sccp_route_allowlists"):
        records[section] = [
            record
            for record in records[section]
            if record.get("source_domain", record.get("domain"))
            != module.SCCP_DOMAIN_TON
        ]
        records[section].extend(ton_records[section])

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is True
    ton_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_TON
    )
    assert ton_lane["blockers"] == []
    assert ton_lane["destination_binding"]["destination_binding_hash"] == (
        live_summary["destination_binding_hash"]
    )
    assert ton_lane["route_allowlist"]["route_allowlist_hash"] == (
        "0x" + route_allowlist_hash.hex()
    )


def test_all_lanes_accepts_direct_ton_destination_toml_with_audited_metadata(tmp_path):
    module = load_evidence_module()
    records = complete_bundle(module)
    ton_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TON)
    profile = module.LANE_PROFILES[module.SCCP_DOMAIN_TON]
    material = records["sccp_source_verifier_materials"][ton_index]
    deployment = records["sccp_source_adapter_engine_deployments"][ton_index]
    destination = records["sccp_destination_rollouts"][ton_index]
    route = records["sccp_route_allowlists"][ton_index]
    ton_module = module._load_sibling_module("sccp_ton_destination_evidence.py")
    source_hashes = module._canonical_source_record_hashes(profile, material, deployment)
    args = SimpleNamespace(
        verifier_contract_address=destination["verifier_identity"],
        verifier_code_hash=raw_hex(destination["verifier_code_hash"]),
        route_allowlist_hash=raw_hex(route["route_allowlist_hash"]),
        source_verifier_material_hash=raw_hex(
            source_hashes["source_verifier_material_hash"]
        ),
        source_adapter_engine_deployment_hash=raw_hex(
            source_hashes["source_adapter_engine_deployment_hash"]
        ),
        route_canary_evidence_hash=raw_hex(route["_comment_route_canary_evidence_hash"]),
        expected_destination_binding_hash=raw_hex(destination["destination_binding_hash"]),
        account_status=destination["_comment_ton_account_status"],
        account_state_hash=raw_hex(destination["_comment_ton_account_state_hash"]),
        last_transaction_lt=destination["_comment_ton_last_transaction_lt"],
        last_transaction_hash=raw_hex(destination["_comment_ton_last_transaction_hash"]),
        verifier_code_boc_hex=bytes.fromhex(TON_CODE_BOC_HEX),
    )

    ton_toml = ton_module.render_toml(args)
    assert '# sccp_ton_account_status = "active"' in ton_toml
    assert "# sccp_ton_account_state_hash" in ton_toml
    assert "# sccp_ton_last_transaction_hash" in ton_toml
    assert "# sccp_ton_code_hash" in ton_toml
    assert "# sccp_ton_code_boc_root_hash" in ton_toml
    assert '# sccp_ton_code_boc_base64 = "' + TON_CODE_BOC_BASE64 + '"' in ton_toml
    assert '# sccp_ton_code_boc_hash_matches = "true"' in ton_toml
    assert 'ton_account_status = "active"' in ton_toml
    assert 'ton_verifier_code_boc_root_hash = "0x' + TON_CODE_BOC_ROOT_HASH + '"' in ton_toml
    assert 'ton_verifier_code_boc = "0x' + TON_CODE_BOC_HEX + '"' in ton_toml
    ton_path = tmp_path / "ton-direct.toml"
    ton_path.write_text(ton_toml, encoding="utf-8")
    ton_records = module.load_evidence_bundle([ton_path])

    for section in ("sccp_destination_rollouts", "sccp_route_allowlists"):
        records[section] = [
            record
            for record in records[section]
            if record.get("source_domain", record.get("domain"))
            != module.SCCP_DOMAIN_TON
        ]
        records[section].extend(ton_records[section])

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is True
    ton_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_TON
    )
    assert ton_lane["blockers"] == []


def test_all_lanes_rejects_ton_destination_without_live_account_metadata():
    module = load_evidence_module()
    records = complete_bundle(module)
    ton_destination = records["sccp_destination_rollouts"][3]
    ton_destination.pop("ton_account_status")
    ton_destination.pop("ton_account_state_hash")
    ton_destination.pop("ton_last_transaction_lt")
    ton_destination.pop("ton_last_transaction_hash")
    ton_destination.pop("ton_verifier_code_boc_root_hash")
    ton_destination.pop("ton_verifier_code_boc")
    ton_destination.pop("_comment_ton_account_status")
    ton_destination.pop("_comment_ton_account_state_hash")
    ton_destination.pop("_comment_ton_last_transaction_lt")
    ton_destination.pop("_comment_ton_last_transaction_hash")
    ton_destination.pop("_comment_ton_code_hash")
    ton_destination.pop("_comment_ton_code_boc_root_hash")
    ton_destination.pop("_comment_ton_code_boc_base64")
    ton_destination.pop("_comment_ton_code_boc_hash_matches")

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "TON live account status metadata must be active" in blockers
    assert "TON account state hash metadata must be a non-zero" in blockers
    assert "TON last transaction LT metadata must be a positive decimal" in blockers
    assert "TON last transaction hash metadata must be a non-zero" in blockers
    assert "TON code hash metadata must be a non-zero" in blockers
    assert "TON code BoC root hash metadata must be a non-zero" in blockers
    assert "TON verifier code BoC metadata must be present" in blockers


def test_all_lanes_rejects_ton_destination_without_live_account_comments():
    module = load_evidence_module()
    records = complete_bundle(module)
    ton_destination = records["sccp_destination_rollouts"][3]
    for field in (
        "_comment_ton_account_status",
        "_comment_ton_account_state_hash",
        "_comment_ton_last_transaction_lt",
        "_comment_ton_last_transaction_hash",
    ):
        ton_destination.pop(field)

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "TON live account status comment must be present" in blockers
    assert "TON account state hash comment must be present" in blockers
    assert "TON last transaction LT comment must be present" in blockers
    assert "TON last transaction hash comment must be present" in blockers


def test_all_lanes_rejects_ton_route_canary_without_active_account_status():
    module = load_evidence_module()
    records = complete_bundle(module)
    ton_destination = records["sccp_destination_rollouts"][3]
    ton_destination["ton_account_status"] = "uninit"
    ton_destination["_comment_ton_account_status"] = "uninit"

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "TON live account status metadata must be active" in blockers
    assert "TON route canary account status must match active destination rollout" in blockers


def test_all_lanes_rejects_ton_destination_without_live_code_hash_metadata():
    module = load_evidence_module()
    records = complete_bundle(module)
    ton_destination = records["sccp_destination_rollouts"][3]
    ton_destination.pop("_comment_ton_code_hash")

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert "TON code hash metadata must be a non-zero" in "\n".join(
        summary["blockers"]
    )


def test_all_lanes_rejects_ton_destination_without_code_boc_match_metadata():
    module = load_evidence_module()
    records = complete_bundle(module)
    ton_destination = records["sccp_destination_rollouts"][3]
    ton_destination.pop("_comment_ton_code_boc_hash_matches")

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert "TON code BoC hash match metadata must be true" in "\n".join(
        summary["blockers"]
    )


def test_all_lanes_rejects_ton_destination_with_false_code_boc_match_metadata():
    module = load_evidence_module()
    records = complete_bundle(module)
    ton_destination = records["sccp_destination_rollouts"][3]
    ton_destination["_comment_ton_code_boc_hash_matches"] = "false"

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert "TON code BoC hash match metadata must be true" in "\n".join(
        summary["blockers"]
    )


def test_all_lanes_rejects_ton_destination_without_code_boc_base64_metadata():
    module = load_evidence_module()
    records = complete_bundle(module)
    ton_destination = records["sccp_destination_rollouts"][3]
    ton_destination.pop("_comment_ton_code_boc_base64")

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert "TON code BoC base64 metadata must be present" in "\n".join(
        summary["blockers"]
    )


def test_all_lanes_rejects_ton_destination_with_invalid_code_boc_base64():
    module = load_evidence_module()
    records = complete_bundle(module)
    ton_destination = records["sccp_destination_rollouts"][3]
    ton_destination["_comment_ton_code_boc_base64"] = "not@@base64"

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert "TON code BoC base64 metadata is invalid" in "\n".join(
        summary["blockers"]
    )

    records = complete_bundle(module)
    ton_destination = records["sccp_destination_rollouts"][3]
    ton_destination["_comment_ton_code_boc_base64"] = noncanonical_base64_alias(
        bytes.fromhex(TON_CODE_BOC_HEX) + b"\x01"
    )

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "TON code BoC base64 metadata is invalid" in blockers
    assert "canonical base64" not in blockers


def test_all_lanes_redacts_ton_live_account_parser_failures(
    monkeypatch,
) -> None:
    """TON live account parser blockers must not echo exception payloads."""

    module = load_evidence_module()
    records = complete_bundle(module)
    ton_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TON)
    profile = module.LANE_PROFILES[module.SCCP_DOMAIN_TON]
    material = records["sccp_source_verifier_materials"][ton_index]
    deployment = records["sccp_source_adapter_engine_deployments"][ton_index]
    destination = records["sccp_destination_rollouts"][ton_index]
    route = records["sccp_route_allowlists"][ton_index]
    source_hashes = module._canonical_source_record_hashes(
        profile,
        material,
        deployment,
    )
    real_ton_module = module._load_sibling_module("sccp_ton_destination_evidence.py")

    for exception_type in (TypeError, ValueError):

        def fail_code_boc_hex(_value, *, label, exception_type=exception_type):
            raise exception_type(f"secret-token {label} code BoC parser")

        def fail_code_boc_base64(_value, *, label, exception_type=exception_type):
            raise exception_type(f"secret-token {label} code BoC base64 parser")

        def fail_raw_address(_value, *, label, exception_type=exception_type):
            raise exception_type(f"secret-token {label} raw address parser")

        module_attrs = dict(real_ton_module.__dict__)
        module_attrs["parse_code_boc_hex"] = fail_code_boc_hex
        monkeypatch.setattr(
            module,
            "_load_sibling_module",
            lambda _name, module_attrs=module_attrs: SimpleNamespace(**module_attrs),
        )
        hex_errors = module._check_ton_live_account_evidence(destination)

        module_attrs = dict(real_ton_module.__dict__)
        module_attrs["parse_code_boc_base64"] = fail_code_boc_base64
        monkeypatch.setattr(
            module,
            "_load_sibling_module",
            lambda _name, module_attrs=module_attrs: SimpleNamespace(**module_attrs),
        )
        base64_errors = module._check_ton_live_account_evidence(destination)

        module_attrs = dict(real_ton_module.__dict__)
        module_attrs["normalize_ton_raw_address"] = fail_raw_address
        monkeypatch.setattr(
            module,
            "_load_sibling_module",
            lambda _name, module_attrs=module_attrs: SimpleNamespace(**module_attrs),
        )
        canary: dict[str, object] = {}
        route_errors = module._check_ton_route_canary_live_account_evidence(
            route,
            destination_record=destination,
            source_record_hashes=source_hashes,
            evidence_hash=raw_hex(route["_comment_route_canary_evidence_hash"]),
            route_allowlist_hash=raw_hex(route["route_allowlist_hash"]),
            destination_binding_hash=raw_hex(destination["destination_binding_hash"]),
            canary=canary,
        )
        rendered = "\n".join([*hex_errors, *base64_errors, *route_errors])

        assert "TON verifier code BoC metadata is invalid" in hex_errors
        assert "TON code BoC base64 metadata is invalid" in base64_errors
        assert "TON route canary verifier identity is invalid" in route_errors
        assert "metadata is invalid:" not in rendered
        assert "identity is invalid:" not in rendered
        assert "secret-token" not in rendered
        assert exception_type.__name__ not in rendered


def test_all_lanes_rejects_ton_destination_when_code_boc_replay_hash_drifts():
    module = load_evidence_module()
    records = complete_bundle(module)
    ton_destination = records["sccp_destination_rollouts"][3]
    ton_destination["_comment_ton_code_boc_root_hash"] = "0x" + "aa" * 32
    ton_destination["ton_verifier_code_boc_root_hash"] = "0x" + "aa" * 32

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "TON code BoC root hash metadata must match verifier_code_hash" in blockers
    assert "TON verifier code BoC root must match root metadata" in blockers


def test_all_lanes_rejects_ton_route_canary_live_account_drift():
    module = load_evidence_module()
    records = complete_bundle(module)
    ton_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TON)
    ton_route = records["sccp_route_allowlists"][ton_index]
    ton_route["ton_route_canary_last_transaction_lt"] = "123456789"
    ton_route["_comment_ton_route_canary_last_transaction_lt"] = "123456789"

    summary = module.validate_evidence_bundle(records)

    blockers = "\n".join(summary["blockers"])
    assert summary["production_ready"] is False
    assert (
        "TON route canary last transaction LT must match destination rollout live account LT"
        in blockers
    )

    records = complete_bundle(module)
    ton_destination = records["sccp_destination_rollouts"][ton_index]
    ton_route = records["sccp_route_allowlists"][ton_index]
    ton_destination["ton_last_transaction_lt"] = "123456789"
    ton_destination["_comment_ton_last_transaction_lt"] = "123456789"
    ton_route["ton_route_canary_last_transaction_lt"] = "123456789"
    ton_route["_comment_ton_route_canary_last_transaction_lt"] = "123456789"

    summary = module.validate_evidence_bundle(records)

    blockers = "\n".join(summary["blockers"])
    assert summary["production_ready"] is False
    assert (
        "TON route canary evidence hash must match live account route canary metadata"
        in blockers
    )


def test_all_lanes_rejects_ton_route_canary_live_account_hash_role_reuse():
    module = load_evidence_module()
    records = complete_bundle(module)
    ton_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TON)
    ton_destination = records["sccp_destination_rollouts"][ton_index]
    ton_route = records["sccp_route_allowlists"][ton_index]
    account_state_hash = ton_route["_comment_ton_route_canary_account_state_hash"]

    for record, fields in (
        (
            ton_destination,
            (
                "ton_last_transaction_hash",
                "_comment_ton_last_transaction_hash",
            ),
        ),
        (
            ton_route,
            (
                "ton_route_canary_last_transaction_hash",
                "_comment_ton_route_canary_last_transaction_hash",
            ),
        ),
    ):
        for field in fields:
            record[field] = account_state_hash

    summary = module.validate_evidence_bundle(records)

    blockers = "\n".join(summary["blockers"])
    assert summary["production_ready"] is False
    assert (
        "TON route canary account state hash must differ from last transaction hash"
        in blockers
    )


def test_all_lanes_rejects_ton_route_canary_governed_hash_role_reuse():
    module = load_evidence_module()
    records = complete_bundle(module)
    ton_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TON)
    profile = module.LANE_PROFILES[module.SCCP_DOMAIN_TON]
    material = records["sccp_source_verifier_materials"][ton_index]
    deployment = records["sccp_source_adapter_engine_deployments"][ton_index]
    source_hashes = module._canonical_source_record_hashes(
        profile,
        material,
        deployment,
    )
    source_material_hash = source_hashes["source_verifier_material_hash"]
    ton_destination = records["sccp_destination_rollouts"][ton_index]
    ton_route = records["sccp_route_allowlists"][ton_index]
    ton_destination["ton_account_state_hash"] = source_material_hash
    ton_destination["_comment_ton_account_state_hash"] = source_material_hash
    ton_route["ton_route_canary_account_state_hash"] = source_material_hash
    ton_route["_comment_ton_route_canary_account_state_hash"] = source_material_hash

    summary = module.validate_evidence_bundle(records)

    blockers = "\n".join(summary["blockers"])
    assert summary["production_ready"] is False
    assert (
        "TON route canary hash role ton_route_canary_account_state_hash must not "
        "reuse source_verifier_material_hash"
    ) in blockers



def test_all_lanes_evidence_rejects_missing_ton_audit_and_route_blocker():
    module = load_evidence_module()
    records = complete_bundle(module)
    ton_deployment = records["sccp_source_adapter_engine_deployments"][3]
    ton_deployment.pop("ton_full_light_client_gate_hash")
    eth_route = records["sccp_route_allowlists"][0]
    eth_route["routes_allowlisted"] = False
    eth_route["blockers"] = ["governance canary has not passed"]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "domain 4 (ton): ton_full_light_client_gate_hash" in blockers
    assert "domain 1 (eth): routes_allowlisted must be True" in blockers
    assert "domain 1 (eth): route allowlist blockers must be empty" in blockers
    ton_gate = next(
        lane["source_adapter_gate"]
        for lane in summary["lanes"]
        if lane["domain"] == module.SCCP_DOMAIN_TON
    )
    assert ton_gate["required"] is True
    assert ton_gate["ready"] is False
    assert "ton_full_light_client_gate_hash must be a non-zero 32-byte hex value" in (
        "\n".join(ton_gate["blockers"])
    )


def test_all_lanes_evidence_rejects_malformed_governed_blocker_containers():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_destination = next(
        rollout
        for rollout in records["sccp_destination_rollouts"]
        if rollout["domain"] == module.SCCP_DOMAIN_ETH
    )
    tron_destination = next(
        rollout
        for rollout in records["sccp_destination_rollouts"]
        if rollout["domain"] == module.SCCP_DOMAIN_TRON
    )
    sol_destination = next(
        rollout
        for rollout in records["sccp_destination_rollouts"]
        if rollout["domain"] == module.SCCP_DOMAIN_SOL
    )
    eth_route = next(
        route
        for route in records["sccp_route_allowlists"]
        if route["domain"] == module.SCCP_DOMAIN_ETH
    )
    bsc_route = next(
        route
        for route in records["sccp_route_allowlists"]
        if route["domain"] == module.SCCP_DOMAIN_BSC
    )
    sol_route = next(
        route
        for route in records["sccp_route_allowlists"]
        if route["domain"] == module.SCCP_DOMAIN_SOL
    )
    ton_route = next(
        route
        for route in records["sccp_route_allowlists"]
        if route["domain"] == module.SCCP_DOMAIN_TON
    )
    bsc_destination = next(
        rollout
        for rollout in records["sccp_destination_rollouts"]
        if rollout["domain"] == module.SCCP_DOMAIN_BSC
    )
    tron_route = next(
        route
        for route in records["sccp_route_allowlists"]
        if route["domain"] == module.SCCP_DOMAIN_TRON
    )

    confusable_blocker = "operator public bl\u043ecker"
    eth_destination["blockers"] = "operator says destination rollout is ready"
    tron_destination["blockers"] = [123]
    sol_destination["blockers"] = ["operator\nsecret-token-governed-blocker"]
    bsc_destination["blockers"] = [
        "destination verifier deployment still pending",
        "destination verifier deployment still pending",
    ]
    eth_route["blockers"] = ["operator secret-token-governed-blocker"]
    bsc_route["blockers"] = [""]
    sol_route["blockers"] = [" route canary still pending"]
    ton_route["blockers"] = ["operator|governed-blocker", confusable_blocker]
    tron_route["blockers"] = [
        "governance canary has not passed",
        "governance canary has not passed",
    ]

    summary = module.validate_evidence_bundle(records)

    blockers = "\n".join(summary["blockers"])
    assert summary["production_ready"] is False
    assert (
        "domain 1 (eth): destination rollout blockers must be a list of "
        "non-empty canonical strings"
    ) in blockers
    assert (
        "domain 5 (tron): destination rollout blockers[0] must be a non-empty "
        "canonical string"
    ) in blockers
    assert (
        "domain 2 (bsc): route allowlist blockers[0] must be a non-empty "
        "canonical string"
    ) in blockers
    assert (
        "domain 3 (sol): route allowlist blockers[0] must be a non-empty "
        "canonical string"
    ) in blockers
    assert (
        "domain 3 (sol): destination rollout blockers[0] contains control "
        "character"
    ) in blockers
    assert (
        "domain 1 (eth): route allowlist blockers[0] contains sensitive name"
        in blockers
    )
    assert (
        "domain 4 (ton): route allowlist blockers[0] contains "
        "Markdown-unsafe character"
    ) in blockers
    assert (
        "domain 4 (ton): route allowlist blockers[1] contains non-ASCII "
        "character"
    ) in blockers
    assert (
        # Source-inventory marker: destination rollout blockers must not contain duplicate strings
        "domain 2 (bsc): destination rollout blockers must not contain "
        "duplicate strings"
    ) in blockers
    assert (
        # Source-inventory marker: route allowlist blockers must not contain duplicate strings
        "domain 5 (tron): route allowlist blockers must not contain duplicate "
        "strings"
    ) in blockers
    assert "domain 2 (bsc): route allowlist blockers must be empty" in blockers
    assert "domain 3 (sol): route allowlist blockers must be empty" in blockers
    assert "destination verifier deployment still pending" not in blockers
    assert "governance canary has not passed" not in blockers
    assert "secret-token-governed-blocker" not in blockers
    assert "operator|governed-blocker" not in blockers
    assert confusable_blocker not in blockers



def test_all_lanes_evidence_rejects_source_gate_audit_hash_role_reuse():
    module = load_evidence_module()
    records = complete_bundle(module)
    by_domain = {
        material["source_domain"]: (material, deployment, destination)
        for material, deployment, destination in zip(
            records["sccp_source_verifier_materials"],
            records["sccp_source_adapter_engine_deployments"],
            records["sccp_destination_rollouts"],
        )
    }

    _, sol_deployment, _ = by_domain[module.SCCP_DOMAIN_SOL]
    sol_route = next(
        route
        for route in records["sccp_route_allowlists"]
        if route["domain"] == module.SCCP_DOMAIN_SOL
    )
    sol_deployment["solana_tower_replay_verifier_hash"] = sol_route[
        "_comment_route_canary_evidence_hash"
    ]

    ton_material, ton_deployment, ton_destination = by_domain[module.SCCP_DOMAIN_TON]
    assert ton_material["source_domain"] == module.SCCP_DOMAIN_TON
    ton_deployment["ton_validator_set_transition_verifier_hash"] = ton_deployment[
        "ton_masterchain_config_verifier_hash"
    ]
    ton_deployment["ton_masterchain_config_verifier_hash"] = ton_destination[
        "destination_binding_hash"
    ]

    _, tron_deployment, tron_destination = by_domain[module.SCCP_DOMAIN_TRON]
    tron_deployment["tron_dpos_source_gate_hash"] = tron_destination[
        "destination_binding_hash"
    ]
    _, bsc_deployment, _ = by_domain[module.SCCP_DOMAIN_BSC]
    bsc_deployment["evm_source_gate_hash"] = bsc_deployment[
        "deployment_receipt_hash"
    ]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert (
        "domain 2 (bsc): source adapter deployment role hash "
        "evm_source_gate_hash must not reuse deployment_receipt_hash"
    ) in blockers
    assert (
        "domain 3 (sol): source_adapter_gate hash role "
        "audit_hashes.solana_tower_replay_verifier_hash must not reuse "
        "route_canary_evidence_hash"
    ) in blockers
    assert (
        "domain 4 (ton): source_adapter_gate hash role "
        "audit_hashes.ton_masterchain_config_verifier_hash must not reuse "
        "destination_binding_hash"
    ) in blockers
    assert (
        "domain 5 (tron): source_adapter_gate hash role "
        "audit_hashes.tron_dpos_source_gate_hash must not reuse "
        "destination_binding_hash"
    ) in blockers


def test_all_lanes_evidence_rejects_source_gate_audit_route_canary_transcript_replay():
    module = load_evidence_module()
    records = complete_bundle(module)
    by_domain = {
        material["source_domain"]: (material, deployment)
        for material, deployment in zip(
            records["sccp_source_verifier_materials"],
            records["sccp_source_adapter_engine_deployments"],
        )
    }
    _, eth_deployment = by_domain[module.SCCP_DOMAIN_ETH]
    eth_route = next(
        route
        for route in records["sccp_route_allowlists"]
        if route["domain"] == module.SCCP_DOMAIN_ETH
    )
    eth_deployment["evm_source_gate_hash"] = eth_route[
        "_comment_evm_route_canary_message_id"
    ]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    eth_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    source_gate_blockers = "\n".join(eth_lane["source_adapter_gate"]["blockers"])
    assert (
        "source_adapter_gate hash role audit_hashes.evm_source_gate_hash "
        "must not reuse route_canary.message_id"
    ) in source_gate_blockers


def test_all_lanes_evidence_rejects_missing_evm_source_gate_hash():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_ETH)
    records["sccp_source_adapter_engine_deployments"][eth_index].pop(
        "evm_source_gate_hash"
    )

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "domain 1 (eth): evm_source_gate_hash" in blockers
    eth_gate = next(
        lane["source_adapter_gate"]
        for lane in summary["lanes"]
        if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    assert eth_gate["required"] is True
    assert eth_gate["ready"] is False
    assert "evm_source_gate_hash must be a non-zero 32-byte hex value" in (
        "\n".join(eth_gate["blockers"])
    )


def test_all_lanes_evidence_recomputes_audit_and_tron_config_hashes():
    module = load_evidence_module()
    records = complete_bundle(module)

    records["sccp_source_adapter_engine_deployments"][0][
        "evm_source_gate_hash"
    ] = hex32(0xAC)
    records["sccp_source_adapter_engine_deployments"][1][
        "evm_source_gate_hash"
    ] = hex32(0xAD)
    records["sccp_source_adapter_engine_deployments"][2][
        "solana_full_light_client_gate_hash"
    ] = hex32(0xAB)
    records["sccp_source_adapter_engine_deployments"][3][
        "ton_full_light_client_gate_hash"
    ] = hex32(0xCD)
    records["sccp_source_adapter_engine_deployments"][4][
        "tron_dpos_source_gate_hash"
    ] = hex32(0xCE)
    records["sccp_source_verifier_materials"][4]["source_bridge_config_hash"] = hex32(0xEF)

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "domain 3 (sol): solana_full_light_client_gate_hash does not match" in blockers
    assert "domain 1 (eth): evm_source_gate_hash does not match" in blockers
    assert "domain 2 (bsc): evm_source_gate_hash does not match" in blockers
    assert "domain 4 (ton): ton_full_light_client_gate_hash does not match" in blockers
    assert "domain 5 (tron): TRON DPoS source gate cannot be recomputed" in blockers
    assert "domain 5 (tron): source_bridge_config_hash does not match" in blockers
    lanes = {lane["domain"]: lane for lane in summary["lanes"]}
    eth_gate = lanes[module.SCCP_DOMAIN_ETH]["source_adapter_gate"]
    bsc_gate = lanes[module.SCCP_DOMAIN_BSC]["source_adapter_gate"]
    sol_gate = lanes[module.SCCP_DOMAIN_SOL]["source_adapter_gate"]
    ton_gate = lanes[module.SCCP_DOMAIN_TON]["source_adapter_gate"]
    tron_gate = lanes[module.SCCP_DOMAIN_TRON]["source_adapter_gate"]
    assert eth_gate["ready"] is False
    assert bsc_gate["ready"] is False
    assert sol_gate["ready"] is False
    assert ton_gate["ready"] is False
    assert tron_gate["ready"] is False
    assert "evm_source_gate_hash does not match" in "\n".join(
        eth_gate["blockers"]
    )
    assert "evm_source_gate_hash does not match" in "\n".join(
        bsc_gate["blockers"]
    )
    assert "solana_full_light_client_gate_hash does not match" in "\n".join(
        sol_gate["blockers"]
    )
    assert "ton_full_light_client_gate_hash does not match" in "\n".join(
        ton_gate["blockers"]
    )
    assert "TRON DPoS source gate cannot be recomputed" in "\n".join(
        tron_gate["blockers"]
    )


def test_all_lanes_source_gate_transcripts_bind_deployment_receipts():
    module = load_evidence_module()
    records = complete_bundle(module)
    cases = (
        (
            module.SCCP_DOMAIN_SOL,
            "solana_full_light_client_gate_hash",
            "solana_full_light_client_gate_hash does not match source and audit material",
        ),
        (
            module.SCCP_DOMAIN_TON,
            "ton_full_light_client_gate_hash",
            "ton_full_light_client_gate_hash does not match source and audit material",
        ),
        (
            module.SCCP_DOMAIN_TRON,
            "tron_dpos_source_gate_hash",
            "tron_dpos_source_gate_hash does not match source and deployment material",
        ),
    )

    for domain, gate_field, blocker in cases:
        drifted = copy.deepcopy(records)
        index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(domain)
        profile = module.LANE_PROFILES[domain]
        material = drifted["sccp_source_verifier_materials"][index]
        deployment = drifted["sccp_source_adapter_engine_deployments"][index]
        original_gate = deployment[gate_field]
        deployment["deployment_receipt_hash"] = hex32(0xEB + index)
        deployment["_comment_source_adapter_engine_deployment_hash"] = (
            module._canonical_source_record_hashes(profile, material, deployment)[
                "source_adapter_engine_deployment_hash"
            ]
        )

        summary = module.validate_evidence_bundle(drifted)

        assert summary["production_ready"] is False
        blockers = "\n".join(summary["blockers"])
        assert f"domain {domain} ({profile.chain}): {blocker}" in blockers
        source_gate = next(
            lane["source_adapter_gate"]
            for lane in summary["lanes"]
            if lane["domain"] == domain
        )
        assert source_gate["gate_hash"] == original_gate
        assert source_gate["ready"] is False
        assert blocker in "\n".join(source_gate["blockers"])


def test_all_lanes_evidence_redacts_source_gate_recompute_failures(
    monkeypatch,
) -> None:
    """Source gate recomputation blockers must not echo exception payloads."""

    module = load_evidence_module()
    records = complete_bundle(module)

    cases = (
        (
            lambda material, deployment: module._check_evm_source_gate(
                module.LANE_PROFILES[module.SCCP_DOMAIN_ETH],
                material,
                deployment,
            ),
            (
                records["sccp_source_verifier_materials"][0],
                records["sccp_source_adapter_engine_deployments"][0],
            ),
            "EVM source gate cannot be recomputed",
        ),
        (
            lambda material, deployment: module._check_evm_source_gate(
                module.LANE_PROFILES[module.SCCP_DOMAIN_BSC],
                material,
                deployment,
            ),
            (
                records["sccp_source_verifier_materials"][1],
                records["sccp_source_adapter_engine_deployments"][1],
            ),
            "EVM source gate cannot be recomputed",
        ),
        (
            module._check_solana_full_light_client_gate,
            (
                records["sccp_source_verifier_materials"][2],
                records["sccp_source_adapter_engine_deployments"][2],
            ),
            "Solana full light-client gate cannot be recomputed",
        ),
        (
            module._check_ton_full_light_client_gate,
            (
                records["sccp_source_verifier_materials"][3],
                records["sccp_source_adapter_engine_deployments"][3],
            ),
            "TON full light-client gate cannot be recomputed",
        ),
        (
            module._check_tron_dpos_source_gate,
            (
                records["sccp_source_verifier_materials"][4],
                records["sccp_source_adapter_engine_deployments"][4],
            ),
            "TRON DPoS source gate cannot be recomputed",
        ),
        (
            module._check_tron_source_bridge_config_hash,
            (records["sccp_source_verifier_materials"][4],),
            "TRON source bridge config hash cannot be recomputed",
        ),
        (
            module._check_eth_source_bridge_config_hash,
            (records["sccp_source_verifier_materials"][0],),
            "ETH source bridge config hash cannot be recomputed",
        ),
    )

    for exception_type in (SystemExit, TypeError, ValueError, RuntimeError):

        def fail_recompute(*_args, exception_type=exception_type, **_kwargs):
            raise exception_type("secret-token source gate material")

        fake_modules = {
            "sccp_solana_source_state_evidence.py": SimpleNamespace(
                solana_full_light_client_gate_hash=fail_recompute,
            ),
            "sccp_ton_source_state_evidence.py": SimpleNamespace(
                ton_full_light_client_gate_hash=fail_recompute,
            ),
            "sccp_tron_source_bridge_evidence.py": SimpleNamespace(
                tron_dpos_source_gate_hash=fail_recompute,
                tron_source_bridge_config_hash=fail_recompute,
            ),
            "sccp_eth_source_bridge_evidence.py": SimpleNamespace(
                eth_source_gate_hash=fail_recompute,
                eth_source_bridge_config_hash=fail_recompute,
            ),
            "sccp_bsc_source_bridge_evidence.py": SimpleNamespace(
                bsc_source_gate_hash=fail_recompute,
            ),
        }

        monkeypatch.setattr(
            module,
            "_load_sibling_module",
            lambda name, fake_modules=fake_modules: fake_modules[name],
        )

        for check, args, expected in cases:
            errors = check(*args)
            rendered = "\n".join(errors)
            assert errors == [expected]
            assert f"{expected}:" not in rendered
            assert "secret-token" not in rendered
            assert exception_type.__name__ not in rendered


def test_all_lanes_public_source_gate_summaries_redact_signature_drift(
    monkeypatch,
) -> None:
    """Public source-gate summaries must not leak helper signature drift."""

    module = load_evidence_module()
    records = complete_bundle(module)

    def secret_token_source_gate_signature_drift():
        raise AssertionError("unreachable signature-drift body")

    original_load_sibling_module = module._load_sibling_module

    def load_with_signature_drift(name):
        sibling = original_load_sibling_module(name)
        drifted_functions = {
            "sccp_solana_source_state_evidence.py": (
                "solana_full_light_client_gate_hash",
            ),
            "sccp_ton_source_state_evidence.py": (
                "ton_full_light_client_gate_hash",
            ),
            "sccp_tron_source_bridge_evidence.py": (
                "tron_dpos_source_gate_hash",
                "tron_source_bridge_config_hash",
            ),
            "sccp_eth_source_bridge_evidence.py": (
                "eth_source_gate_hash",
                "eth_source_bridge_config_hash",
            ),
            "sccp_bsc_source_bridge_evidence.py": ("bsc_source_gate_hash",),
        }.get(name, ())
        for function_name in drifted_functions:
            setattr(
                sibling,
                function_name,
                secret_token_source_gate_signature_drift,
            )
        return sibling

    monkeypatch.setattr(
        module,
        "_load_sibling_module",
        load_with_signature_drift,
    )

    summary = module.validate_evidence_bundle(records)

    expected_blockers = {
        module.SCCP_DOMAIN_ETH: "EVM source gate cannot be recomputed",
        module.SCCP_DOMAIN_BSC: "EVM source gate cannot be recomputed",
        module.SCCP_DOMAIN_SOL: "Solana full light-client gate cannot be recomputed",
        module.SCCP_DOMAIN_TON: "TON full light-client gate cannot be recomputed",
        module.SCCP_DOMAIN_TRON: "TRON DPoS source gate cannot be recomputed",
    }
    assert summary["production_ready"] is False
    rendered = json.dumps(summary, sort_keys=True)
    assert "secret_token_source_gate_signature_drift" not in rendered
    assert "unreachable signature-drift body" not in rendered

    for domain, expected in expected_blockers.items():
        profile = module.LANE_PROFILES[domain]
        lane = next(item for item in summary["lanes"] if item["domain"] == domain)
        source_gate = lane["source_adapter_gate"]
        assert source_gate["ready"] is False
        assert expected in source_gate["blockers"]
        assert f"domain {domain} ({profile.chain}): {expected}" in summary["blockers"]


def test_all_lanes_evidence_rejects_tron_dpos_source_gate_hash_drift():
    module = load_evidence_module()
    records = complete_bundle(module)
    tron_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(module.SCCP_DOMAIN_TRON)
    records["sccp_source_adapter_engine_deployments"][tron_index][
        "tron_dpos_source_gate_hash"
    ] = hex32(0xCE)

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert (
        "domain 5 (tron): tron_dpos_source_gate_hash does not match source and deployment material"
        in blockers
    )
    tron_gate = next(
        lane["source_adapter_gate"]
        for lane in summary["lanes"]
        if lane["domain"] == module.SCCP_DOMAIN_TRON
    )
    assert tron_gate["required"] is True
    assert tron_gate["ready"] is False
    assert tron_gate["gate_hash"] == hex32(0xCE)
    assert set(tron_gate["audit_hashes"]) == {"tron_dpos_source_gate_hash"}
    assert "tron_dpos_source_gate_hash does not match" in "\n".join(
        tron_gate["blockers"]
    )


def test_all_lanes_evidence_rejects_lane_foreign_audit_fields():
    module = load_evidence_module()
    records = complete_bundle(module)

    eth_deployment = records["sccp_source_adapter_engine_deployments"][0]
    eth_deployment["solana_tower_replay_verifier_hash"] = hex32(0x90)
    eth_deployment["solana_full_light_client_gate_hash"] = hex32(0x91)
    solana_deployment = records["sccp_source_adapter_engine_deployments"][2]
    solana_deployment["evm_source_gate_hash"] = hex32(0x96)
    solana_deployment["ton_masterchain_config_verifier_hash"] = hex32(0x92)
    solana_deployment["ton_full_light_client_gate_hash"] = hex32(0x93)
    bsc_deployment = records["sccp_source_adapter_engine_deployments"][1]
    bsc_deployment["tron_dpos_source_gate_hash"] = hex32(0x94)

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert (
        "domain 1 (eth): solana_tower_replay_verifier_hash must be empty for this lane"
        in blockers
    )
    assert (
        "domain 1 (eth): solana_full_light_client_gate_hash must be empty for this lane"
        in blockers
    )
    assert (
        "domain 3 (sol): ton_masterchain_config_verifier_hash must be empty for this lane"
        in blockers
    )
    assert (
        "domain 3 (sol): evm_source_gate_hash must be empty for this lane"
        in blockers
    )
    assert (
        "domain 3 (sol): ton_full_light_client_gate_hash must be empty for this lane"
        in blockers
    )
    assert (
        "domain 2 (bsc): tron_dpos_source_gate_hash must be empty for this lane"
        in blockers
    )


def test_all_lanes_evidence_rejects_destination_binding_drift():
    module = load_evidence_module()
    records = complete_bundle(module)

    records["sccp_destination_rollouts"][0]["destination_bridge_address"] = hex20(0xA4)
    del records["sccp_destination_rollouts"][0]["destination_binding_key"]
    records["sccp_destination_rollouts"][1]["destination_binding_key"] = "evm:wrong"
    records["sccp_destination_rollouts"][2]["destination_binding_hash"] = hex32(0xA5)
    del records["sccp_destination_rollouts"][2]["destination_binding_key"]
    records["sccp_destination_rollouts"][3]["destination_binding_key"] = "ton:wrong"
    records["sccp_destination_rollouts"][4]["destination_network_id"] = hex32(0xA6)
    records["sccp_destination_rollouts"][4]["destination_binding_key"] = "tron:wrong"

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "domain 1 (eth): destination_binding_hash does not match" in blockers
    assert "domain 1 (eth): destination_binding_key must be supplied" in blockers
    assert "domain 2 (bsc): destination_binding_key does not match" in blockers
    assert "domain 3 (sol): destination_binding_hash does not match" in blockers
    assert "domain 3 (sol): destination_binding_key must be supplied" in blockers
    assert "domain 4 (ton): destination_binding_key does not match" in blockers
    assert "domain 5 (tron): destination_network_id does not match" in blockers
    assert "domain 5 (tron): destination_binding_key does not match" in blockers


def test_all_lanes_evidence_redacts_destination_binding_recompute_failures(
    monkeypatch,
) -> None:
    """Destination binding recomputation blockers must not echo exceptions."""

    module = load_evidence_module()
    records = complete_bundle(module)

    for exception_type in (SystemExit, TypeError, ValueError, RuntimeError):

        def fail_destination_binding(
            _profile,
            _material,
            _destination,
            exception_type=exception_type,
        ):
            raise exception_type("secret-token destination binding material")

        monkeypatch.setattr(
            module,
            "_expected_destination_binding",
            fail_destination_binding,
        )

        summary = module.validate_evidence_bundle(records)

        assert summary["production_ready"] is False
        blockers = "\n".join(summary["blockers"])
        assert "destination binding cannot be recomputed" in blockers
        assert "destination binding cannot be recomputed:" not in blockers
        assert "secret-token" not in blockers
        assert exception_type.__name__ not in blockers


def test_all_lanes_evidence_rejects_destination_comment_drift():
    module = load_evidence_module()
    records = complete_bundle(module)

    eth_destination = records["sccp_destination_rollouts"][0]
    eth_destination["_comment_destination_network_id"] = hex32(0xB0)
    eth_destination["_comment_destination_bridge_address"] = hex20(0xB1)
    eth_destination["_comment_destination_binding_key"] = "evm:wrong-comment"
    eth_destination["_comment_destination_binding_hash"] = hex32(0xB2)
    eth_destination["_comment_evm_verifier_code_hash"] = hex32(0xB3)
    eth_destination["_comment_evm_verifier_key_hash"] = hex32(0xB4)
    eth_destination["_comment_evm_verifier_backend_hash"] = hex32(0xB8)
    eth_destination["_comment_evm_proof_family_hash"] = hex32(0xB9)

    sol_destination = records["sccp_destination_rollouts"][2]
    sol_destination["_comment_destination_binding_hash"] = "not-a-hex-hash"

    tron_destination = records["sccp_destination_rollouts"][4]
    tron_destination["_comment_destination_binding_key"] = "tron:wrong-comment"
    tron_destination["_comment_tron_destination_verifier_key_hash"] = hex32(0xB5)
    tron_destination["_comment_tron_destination_verifier_backend_hash"] = hex32(0xB6)
    tron_destination["_comment_tron_destination_proof_family_hash"] = hex32(0xB7)

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert (
        "domain 1 (eth): destination_network_id comment must match "
        "destination_network_id"
    ) in blockers
    assert (
        "domain 1 (eth): destination_bridge_address comment must match "
        "destination_bridge_address"
    ) in blockers
    assert (
        "domain 1 (eth): destination_binding_key comment must match "
        "destination_binding_key"
    ) in blockers
    assert (
        "domain 1 (eth): destination_binding_hash comment must match "
        "destination_binding_hash"
    ) in blockers
    assert (
        "domain 1 (eth): EVM verifier runtime code hash metadata must match "
        "verifier_code_hash"
    ) in blockers
    assert (
        "domain 1 (eth): EVM verifier key hash metadata must match "
        "verifier_key_hash"
    ) in blockers
    assert (
        "domain 1 (eth): EVM verifier backend hash metadata must match "
        "evm-groth16-bn254-v1"
    ) in blockers
    assert (
        "domain 1 (eth): EVM proof family hash metadata must match stark-fri-v1"
        in blockers
    )
    assert (
        "domain 3 (sol): destination_binding_hash comment must be a 32-byte hex value"
        in blockers
    )
    assert (
        "domain 5 (tron): destination_binding_key comment must match "
        "destination_binding_key"
    ) in blockers
    assert (
        "domain 5 (tron): TRON destination verifier key hash metadata must match "
        "verifier_key_hash"
    ) in blockers
    assert (
        "domain 5 (tron): TRON destination verifier backend hash metadata must "
        "match tron-groth16-bn254-v1"
    ) in blockers
    assert (
        "domain 5 (tron): TRON destination proof family hash metadata must match "
        "stark-fri-v1"
    ) in blockers


def test_all_lanes_evidence_rejects_evm_destination_verifier_bridge_alias():
    module = load_evidence_module()
    records = complete_bundle(module)

    eth_destination = records["sccp_destination_rollouts"][0]
    eth_destination["destination_bridge_address"] = eth_destination["verifier_identity"]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert (
        "EVM destination verifier_identity must differ from destination_bridge_address"
        in "\n".join(summary["blockers"])
    )


def test_all_lanes_evidence_rejects_route_allowlist_hash_drift():
    module = load_evidence_module()
    records = complete_bundle(module)

    records["sccp_route_allowlists"][0]["route_allowlist_hash"] = hex32(0xDE)

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    eth_lane = next(lane for lane in summary["lanes"] if lane["chain"] == "eth")
    route = eth_lane["route_allowlist"]
    assert route["route_allowlist_hash"] == hex32(0xDE)
    assert route["expected_route_allowlist_hash"].startswith("0x")
    assert route["expected_route_allowlist_hash_matches"] is False
    assert (
        "domain 1 (eth): route_allowlist_hash does not match canonical source"
        in "\n".join(summary["blockers"])
    )


def test_all_lanes_evidence_requires_route_canary_metadata():
    module = load_evidence_module()
    records = complete_bundle(module)

    sol_route = records["sccp_route_allowlists"][2]
    del sol_route["_comment_route_canary_status"]
    del sol_route["_comment_route_canary_evidence_hash"]
    del sol_route["_comment_route_canary_route_allowlist_hash"]
    del sol_route["_comment_route_canary_destination_binding_hash"]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "domain 3 (sol): route canary status metadata must be passed" in blockers
    assert "domain 3 (sol): route canary evidence hash metadata" in blockers
    assert "domain 3 (sol): route canary route allowlist hash metadata" in blockers
    assert "domain 3 (sol): route canary destination binding hash metadata" in blockers


def test_all_lanes_evidence_rejects_route_canary_evidence_hash_template_replays():
    module = load_evidence_module()

    for domain in module.SCCP_CORE_REMOTE_DOMAINS:
        records = complete_bundle(module)
        profile = module.LANE_PROFILES[domain]
        route = next(
            record
            for record in records["sccp_route_allowlists"]
            if record["domain"] == domain
        )
        template_hash = next(iter(module._source_material_template_hashes(profile).values()))
        template_value = "0x" + template_hash.hex()
        route["_comment_route_canary_evidence_hash"] = template_value
        if "route_canary_evidence_hash" in route:
            route["route_canary_evidence_hash"] = template_value

        summary = module.validate_evidence_bundle(records)

        assert summary["production_ready"] is False
        assert (
            f"domain {domain} ({profile.chain}): route canary evidence hash "
            "must be live evidence, not built-in template material"
        ) in "\n".join(summary["blockers"])


def test_all_lanes_release_checklist_reports_ready_bundle():
    module = load_evidence_module()
    summary = module.validate_evidence_bundle(complete_bundle(module))

    checklist = summary["release_checklist"]
    items = {item["id"]: item for item in checklist["items"]}

    assert checklist["ready"] is True
    assert set(items) == {
        "all_required_lane_records",
        "governed_deployment_evidence",
        "route_allowlist_binding",
        "live_route_canary_evidence",
        "no_unresolved_blockers",
    }
    assert all(item["ready"] for item in items.values())


def test_all_lanes_public_summary_accepts_active_ready_with_future_lane_blockers():
    module = load_evidence_module()
    records = complete_bundle(module)
    active_domain = module.SCCP_DOMAIN_ETH
    for section, domain_key in {
        "sccp_source_verifier_materials": "source_domain",
        "sccp_destination_rollouts": "domain",
        "sccp_route_allowlists": "domain",
    }.items():
        records[section] = [
            record
            for record in records[section]
            if record.get(domain_key) == active_domain
        ]

    summary = module.validate_evidence_bundle(records)
    public_summary = module._public_summary_payload(copy.deepcopy(summary))

    assert public_summary == summary
    assert summary["production_ready"] is False
    active_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == active_domain
    )
    assert active_lane["production_ready"] is True
    blocked_future_lanes = [
        lane
        for lane in summary["lanes"]
        if lane["domain"] != active_domain and not lane["production_ready"]
    ]
    assert blocked_future_lanes
    assert all(
        blocker.startswith("domain ") and "missing " in blocker
        for blocker in summary["blockers"]
    )
    assert not any(
        "all-lanes summary lanes[" in blocker
        for blocker in public_summary["blockers"]
    )


def test_all_lanes_release_checklist_rejects_malformed_record_flags():
    module = load_evidence_module()
    summary = module.validate_evidence_bundle(complete_bundle(module))
    eth_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    eth_lane["records"]["source_verifier_material"] = "true"

    checklist = module._release_checklist(summary["lanes"], [])
    items = {item["id"]: item for item in checklist["items"]}

    assert checklist["ready"] is False
    assert items["all_required_lane_records"]["ready"] is False
    assert (
        "domain 1 (eth): missing source verifier material"
        in items["all_required_lane_records"]["blockers"]
    )


def test_all_lanes_release_checklist_rejects_malformed_lane_containers():
    module = load_evidence_module()
    lane = {
        "domain": module.SCCP_DOMAIN_BSC,
        "chain": "bsc",
        "records": "all-present",
        "source_adapter_gate": {
            "required": False,
            "ready": True,
            "blockers": [],
        },
        "destination_binding": "bound",
        "route_allowlist": "bound",
        "blockers": "route canary hidden",
    }

    checklist = module._release_checklist([lane], [])
    items = {item["id"]: item for item in checklist["items"]}

    assert checklist["ready"] is False
    assert (
        "domain 2 (bsc): lane record summary is malformed"
        in items["all_required_lane_records"]["blockers"]
    )
    assert (
        "domain 2 (bsc): lane record summary is malformed"
        in items["no_unresolved_blockers"]["blockers"]
    )
    assert (
        "domain 2 (bsc): destination binding summary is malformed"
        in items["governed_deployment_evidence"]["blockers"]
    )
    assert (
        "domain 2 (bsc): destination binding summary is malformed"
        in items["no_unresolved_blockers"]["blockers"]
    )
    assert (
        "domain 2 (bsc): route allowlist summary is malformed"
        in items["route_allowlist_binding"]["blockers"]
    )
    assert (
        "domain 2 (bsc): route allowlist summary is malformed"
        in items["no_unresolved_blockers"]["blockers"]
    )
    assert (
        "domain 2 (bsc): lane blockers must be a list of "
        "non-empty canonical strings"
        in items["live_route_canary_evidence"]["blockers"]
    )
    assert (
        "domain 2 (bsc): lane blockers must be a list of "
        "non-empty canonical strings"
        in items["no_unresolved_blockers"]["blockers"]
    )


def test_all_lanes_release_checklist_rejects_malformed_lane_rows():
    module = load_evidence_module()

    checklist = module._release_checklist(["operator secret-token-row", 123], [])
    items = {item["id"]: item for item in checklist["items"]}
    all_blockers = "\n".join(
        blocker for item in items.values() for blocker in item["blockers"]
    )

    assert checklist["ready"] is False
    for item_id in (
        "all_required_lane_records",
        "governed_deployment_evidence",
        "route_allowlist_binding",
        "live_route_canary_evidence",
        "no_unresolved_blockers",
    ):
        assert items[item_id]["ready"] is False
        assert "lane 0: lane summary must be an object" in items[item_id]["blockers"]
        assert "lane 1: lane summary must be an object" in items[item_id]["blockers"]
    assert "secret-token-row" not in all_blockers
    assert "Traceback" not in all_blockers


def test_all_lanes_release_checklist_rejects_malformed_lane_metadata():
    module = load_evidence_module()
    base_lane = {
        "domain": module.SCCP_DOMAIN_BSC,
        "chain": "bsc",
        "records": {
            "source_verifier_material": True,
            "source_adapter_deployment": True,
            "destination_rollout": True,
            "route_allowlist": True,
        },
        "source_adapter_gate": {
            "required": False,
            "ready": True,
            "blockers": [],
        },
        "destination_binding": {
            "expected_destination_binding_hash_matches": True,
        },
        "route_allowlist": {
            "expected_route_allowlist_hash_matches": True,
            "route_canary": {
                "status": "passed",
                "evidence_hash": hex32(0x5D),
                "evidence_source": "evm_message_proof_accepted_transaction",
                "evidence_bound": True,
            },
        },
        "blockers": [],
    }
    cases = (
        (
            "domain.missing",
            {"domain": None, "chain": "bsc"},
            "lane bsc: lane domain must be an integer",
            "lane bsc: live route canary evidence source cannot be validated for malformed lane domain",
        ),
        (
            "domain.string",
            {"domain": "2", "chain": "bsc"},
            "lane bsc: lane domain must be an integer",
            "lane bsc: live route canary evidence source cannot be validated for malformed lane domain",
        ),
        (
            "domain.unsupported",
            {"domain": 99, "chain": "bsc"},
            "domain 99 (bsc): lane domain must be a production remote domain",
            "domain 99 (bsc): live route canary evidence source cannot be validated for malformed lane domain",
        ),
        (
            "chain.padded",
            {"domain": module.SCCP_DOMAIN_BSC, "chain": " bsc "},
            "domain 2: lane chain must be a non-empty canonical string",
            None,
        ),
        (
            "chain.mismatch",
            {"domain": module.SCCP_DOMAIN_BSC, "chain": "eth"},
            "domain 2 (eth): lane chain must be bsc",
            None,
        ),
    )

    for case_id, metadata, expected_record_blocker, expected_canary_blocker in cases:
        lane = copy.deepcopy(base_lane)
        if metadata["domain"] is None:
            lane.pop("domain")
        else:
            lane["domain"] = metadata["domain"]
        lane["chain"] = metadata["chain"]

        checklist = module._release_checklist([lane], [])
        items = {item["id"]: item for item in checklist["items"]}
        record_blockers = "\n".join(items["all_required_lane_records"]["blockers"])
        canary_blockers = "\n".join(items["live_route_canary_evidence"]["blockers"])

        assert checklist["ready"] is False, case_id
        assert items["all_required_lane_records"]["ready"] is False, case_id
        assert items["no_unresolved_blockers"]["ready"] is False, case_id
        assert expected_record_blocker in record_blockers, case_id
        assert (
            expected_record_blocker in items["no_unresolved_blockers"]["blockers"]
        ), case_id
        if expected_canary_blocker is not None:
            assert items["live_route_canary_evidence"]["ready"] is False, case_id
            assert expected_canary_blocker in canary_blockers, case_id
            assert (
                expected_canary_blocker in items["no_unresolved_blockers"]["blockers"]
            ), case_id
        assert "None" not in record_blockers
        assert "Traceback" not in canary_blockers


def test_all_lanes_release_checklist_rejects_malformed_lane_blockers():
    module = load_evidence_module()
    base_lane = {
        "domain": module.SCCP_DOMAIN_BSC,
        "chain": "bsc",
        "records": {
            "source_verifier_material": True,
            "source_adapter_deployment": True,
            "destination_rollout": True,
            "route_allowlist": True,
        },
        "source_adapter_gate": {
            "required": False,
            "ready": True,
            "blockers": [],
        },
        "destination_binding": {
            "expected_destination_binding_hash_matches": True,
        },
        "route_allowlist": {
            "expected_route_allowlist_hash_matches": True,
            "route_canary": {
                "status": "passed",
                "evidence_hash": hex32(0x5B),
                "evidence_source": "evm_message_proof_accepted_transaction",
                "evidence_bound": True,
            },
        },
        "blockers": [],
    }
    cases = (
        (
            "operator hold",
            "domain 2 (bsc): lane blockers must be a list of "
            "non-empty canonical strings",
        ),
        (
            [123],
            "domain 2 (bsc): lane blockers[0] must be a "
            "non-empty canonical string",
        ),
        (
            [" padded "],
            "domain 2 (bsc): lane blockers[0] must be a "
            "non-empty canonical string",
        ),
        (
            ["route canary operator hold"],
            "domain 2 (bsc): route canary operator hold",
        ),
    )

    for lane_blockers, expected in cases:
        lane = copy.deepcopy(base_lane)
        lane["blockers"] = lane_blockers

        checklist = module._release_checklist([lane], [])
        items = {item["id"]: item for item in checklist["items"]}

        assert checklist["ready"] is False, repr(lane_blockers)
        assert items["live_route_canary_evidence"]["ready"] is False
        assert items["no_unresolved_blockers"]["ready"] is False
        assert expected in items["live_route_canary_evidence"]["blockers"]
        assert expected in items["no_unresolved_blockers"]["blockers"]
        if not isinstance(lane_blockers, list):
            assert "o" not in items["live_route_canary_evidence"]["blockers"]


def test_all_lanes_release_checklist_compares_item_ready_exactly():
    module = load_evidence_module()
    summary = module.validate_evidence_bundle(complete_bundle(module))
    original_item = module._release_checklist_item

    def malformed_item(item_id, title, blockers):
        item = original_item(item_id, title, blockers)
        if item_id == "all_required_lane_records":
            item["ready"] = "true"
        return item

    module._release_checklist_item = malformed_item
    try:
        checklist = module._release_checklist(summary["lanes"], [])
    finally:
        module._release_checklist_item = original_item
    items = {item["id"]: item for item in checklist["items"]}

    assert items["all_required_lane_records"]["ready"] == "true"
    assert checklist["ready"] is False


def test_all_lanes_cli_exit_compares_production_ready_exactly(capsys):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = module.validate_evidence_bundle(complete_bundle(module))
    summary["production_ready"] = "true"

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: copy.deepcopy(summary)
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    payload = json.loads(capsys.readouterr().out)
    assert payload["production_ready"] is False
    assert payload["blockers"] == [
        "all-lanes summary production_ready must be boolean"
    ]


def test_all_lanes_cli_suppresses_malformed_summary_roots(capsys):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: "operator secret-token-summary"
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    assert payload == {
        "production_ready": False,
        "blockers": ["all-lanes summary must be an object"],
    }
    assert "secret-token-summary" not in captured.out


def test_all_lanes_cli_suppresses_malformed_summary_blockers(capsys):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = module.validate_evidence_bundle(complete_bundle(module))
    summary["blockers"] = ["operator secret%2dtoken-blocker"]

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: copy.deepcopy(summary)
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    assert payload["production_ready"] is False
    assert payload["blockers"] == [
        "all-lanes summary blockers[0] contains sensitive name"
    ]
    assert "secret%2dtoken-blocker" not in captured.out


def test_all_lanes_cli_suppresses_encoded_recovery_phrase_blockers(capsys):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    eth_lane = summary["lanes"][0]
    summary["production_ready"] = False
    summary["blockers"] = ["operator recovery%20phrase-root-blocker"]
    eth_lane["production_ready"] = False
    eth_lane["blockers"] = ["operator recovery&#32;phrase-lane-blocker"]
    eth_lane["source_adapter_gate"]["ready"] = False
    eth_lane["source_adapter_gate"]["blockers"] = [
        "operator recovery_phrase-gate-blocker"
    ]
    eth_lane["source_record_hashes"][
        "source_verifier_material_hash"
    ] = "operator recovery%20phrase-lane-value"

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: copy.deepcopy(summary)
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "all-lanes summary blockers[0] contains sensitive name" in blockers
    assert "all-lanes summary lanes[0] blockers[0] contains sensitive name" in blockers
    assert (
        "all-lanes summary lanes[0].source_adapter_gate blockers[0] "
        "contains sensitive name"
    ) in blockers
    assert (
        "all-lanes summary lanes[0].source_record_hashes."
        "source_verifier_material_hash contains sensitive value"
    ) in blockers
    assert "recovery%20phrase" not in captured.out
    assert "recovery&#32;phrase" not in captured.out
    assert "recovery_phrase" not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_public_blocker_sanitizers_cover_marker_families():
    module = load_evidence_module()
    nested_label = (
        "all-lanes summary lanes[0].source_record_hashes."
        "source_verifier_material_hash"
    )

    assert module._blocker_text_issue("operator public rollout pending") is None
    assert (
        module._public_nested_lane_value_errors(
            "operator public rollout pending",
            nested_label,
        )
        == []
    )

    for marker in module.SENSITIVE_PUBLIC_BLOCKER_MARKERS:
        encoded_marker = (
            marker.replace("-", "%2d").replace("_", "%5f").replace(" ", "%20")
        )
        blocker = f"operator-{encoded_marker}-blocker"
        value = f"operator-{encoded_marker}-lane-value"

        assert module._blocker_text_issue(blocker) == "contains sensitive name"
        assert module._public_nested_lane_value_errors(value, nested_label) == [
            f"{nested_label} contains sensitive value"
        ]

    encoded_confusable_marker = "s%D0%B5cret-token"
    assert (
        module._blocker_text_issue(f"operator-{encoded_confusable_marker}-blocker")
        == "contains sensitive name"
    )
    assert module._public_nested_lane_value_errors(
        f"operator-{encoded_confusable_marker}-lane-value",
        nested_label,
    ) == [f"{nested_label} contains sensitive value"]

    decoded_unsafe_blockers = (
        ("safe%0Apublic-blocker", "contains control character"),
        ("safe%E2%80%AEpublic-blocker", "contains non-ASCII character"),
        ("safe%7Cpublic-blocker", "contains Markdown-unsafe character"),
        ("safe%3Cpublic-blocker%3E", "contains Markdown-unsafe character"),
    )
    for blocker, issue in decoded_unsafe_blockers:
        assert module._blocker_text_issue(blocker) == issue

    decoded_unsafe_values = (
        ("safe%0Apublic-value", "contains control character"),
        ("safe%E2%80%AEpublic-value", "contains non-ASCII value"),
        ("safe%7Cpublic-value", "contains Markdown-unsafe value"),
        ("safe%3Cpublic-value%3E", "contains Markdown-unsafe value"),
    )
    for value, issue in decoded_unsafe_values:
        assert module._public_nested_lane_value_errors(value, nested_label) == [
            f"{nested_label} {issue}"
        ]


def test_all_lanes_cli_error_detail_rejects_decoded_unsafe_messages():
    module = load_evidence_module()
    fallback = "SCCP all-lanes evidence validation failed"

    for detail in (
        "safe%0Acollector detail",
        "safe%E2%80%AEcollector detail",
        "safe%7Ccollector detail",
        "safe%3Ccollector detail%3E",
    ):
        assert module._cli_error_detail(RuntimeError(detail), fallback=fallback) == fallback

    assert (
        module._cli_error_detail(RuntimeError("safe collector detail"), fallback=fallback)
        == "safe collector detail"
    )


def test_all_lanes_cli_rejects_duplicate_public_blockers_without_leaking(capsys):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = module.validate_evidence_bundle(complete_bundle(module))
    root_blocker = "safe duplicated root blocker"
    encoded_root_blocker = "safe%20duplicated%20root%20blocker"
    lane_blocker = "safe duplicated lane blocker"
    encoded_lane_blocker = "safe%20duplicated%20lane%20blocker"
    gate_blocker = "safe duplicated source gate blocker"
    encoded_gate_blocker = "safe duplicated source&#32;gate blocker"
    checklist_blocker = "safe duplicated checklist blocker"
    encoded_checklist_blocker = "safe%20duplicated%20checklist%20blocker"
    eth_lane = summary["lanes"][0]
    summary["production_ready"] = False
    summary["blockers"] = [root_blocker, encoded_root_blocker]
    eth_lane["production_ready"] = False
    eth_lane["blockers"] = [lane_blocker, encoded_lane_blocker]
    eth_lane["source_adapter_gate"]["ready"] = False
    eth_lane["source_adapter_gate"]["blockers"] = [
        gate_blocker,
        encoded_gate_blocker,
    ]
    summary["release_checklist"]["items"][0]["ready"] = False
    summary["release_checklist"]["items"][0]["blockers"] = [
        checklist_blocker,
        encoded_checklist_blocker,
    ]

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: copy.deepcopy(summary)
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert (
        "all-lanes summary blockers must not contain duplicate strings"
    ) in blockers
    assert (
        "all-lanes summary lanes[0] blockers must not contain duplicate strings"
    ) in blockers
    assert (
        # Source-inventory marker: all-lanes summary lanes[0].source_adapter_gate blockers must not contain duplicate strings
        "all-lanes summary lanes[0].source_adapter_gate blockers must not "
        "contain duplicate strings"
    ) in blockers
    assert (
        # Source-inventory marker: all-lanes summary release_checklist items[0] blockers must not contain duplicate strings
        "all-lanes summary release_checklist items[0] blockers must not contain "
        "duplicate strings"
    ) in blockers
    assert "lanes" not in payload
    assert "release_checklist" not in payload
    for copied_blocker in (
        root_blocker,
        encoded_root_blocker,
        lane_blocker,
        encoded_lane_blocker,
        gate_blocker,
        encoded_gate_blocker,
        checklist_blocker,
        encoded_checklist_blocker,
    ):
        assert copied_blocker not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_unknown_summary_fields_without_leaking(capsys):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: {
        "production_ready": True,
        "blockers": [],
        "required_domains": [1, 2, 3, 4, 5],
        "lanes": [],
        "operator_note": "safe note",
        "secret-token-summary": "secret-token-value",
        7: "secret-token-int-key",
        HostilePublicKey(): "secret-token-hostile-root-key",
    }
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    assert payload["production_ready"] is False
    assert "operator_note" not in payload
    assert "secret-token-summary" not in payload
    assert "7" not in payload
    assert "safe note" not in captured.out
    assert "secret-token" not in captured.out
    assert "hostile" not in captured.out
    assert "Traceback" not in captured.err

    blockers = "\n".join(payload["blockers"])
    assert "all-lanes summary unexpected field operator_note" in blockers
    assert "all-lanes summary unexpected field with sensitive name" in blockers
    assert "all-lanes summary unexpected non-string field name" in blockers


def test_all_lanes_cli_rejects_malformed_allowed_summary_roots_without_leaking(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: {
        "production_ready": True,
        "blockers": [],
        "required_domains": "operator secret-token-required-domains",
        "supported_launch_domains": ["operator secret-token-supported-domains"],
        "unsupported_launch_domains": {
            "operator": "secret-token-unsupported-domains"
        },
        "lanes": ["operator secret-token-lane"],
        "release_checklist": "operator secret-token-release-checklist",
    }
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    assert payload == {
        "production_ready": False,
        "blockers": [
            "all-lanes summary required_domains must be a list of integers",
            "all-lanes summary supported_launch_domains must be a list of integers",
            "all-lanes summary unsupported_launch_domains must be a list of integers",
            "all-lanes summary lanes must be a list of objects",
            "all-lanes summary release_checklist must be an object",
        ],
    }
    assert "secret-token" not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_missing_copied_summary_roots_without_leaking(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["blockers"] = ["operator pending copied root field audit"]
    missing_fields = tuple(
        field for field in module.ALL_LANES_PUBLIC_SUMMARY_FIELDS if field != "blockers"
    )
    for field in missing_fields:
        summary.pop(field, None)

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "operator pending copied root field audit" in blockers
    for field in missing_fields:
        assert f"all-lanes summary {field} missing" in blockers
    assert "all-lanes summary production_ready must be boolean" in blockers
    assert "secret-token" not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_drifted_domain_lists(capsys):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = module.validate_evidence_bundle(complete_bundle(module))
    summary["required_domains"] = [1, 1, 3, 4, 5]
    summary["supported_launch_domains"] = [1, 2, 3, 4, 5, 6]
    summary["unsupported_launch_domains"] = [1, 6]

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "required_domains" not in payload
    assert "supported_launch_domains" not in payload
    assert "unsupported_launch_domains" not in payload
    assert (
        "all-lanes summary required_domains must not contain duplicate integers"
        in blockers
    )
    assert (
        "all-lanes summary required_domains must be the production remote domains"
        in blockers
    )
    assert (
        "all-lanes summary supported_launch_domains must be the supported launch "
        "remote domains"
    ) in blockers
    assert (
        "all-lanes summary unsupported_launch_domains must be the diagnostic "
        "unsupported remote domains"
    ) in blockers
    assert (
        "all-lanes summary supported_launch_domains and "
        "unsupported_launch_domains must be disjoint"
    ) in blockers
    assert (
        "all-lanes summary supported_launch_domains plus "
        "unsupported_launch_domains must match required_domains"
    ) in blockers
    assert "all-lanes summary required_domains is invalid" in blockers
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_nested_lane_field_drift_without_leaking(capsys):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    lane = summary["lanes"][0]
    lane[7] = "safe lane int-key note"
    lane[HostilePublicKey()] = "secret-token hostile lane note"
    lane["source_record_hashes"]["operator_note"] = "safe source note"
    lane["source_record_hashes"][7] = "safe source int-key note"
    lane["source_record_hashes"][HostilePublicKey()] = (
        "secret-token hostile source note"
    )
    lane["source_adapter_gate"]["operator_note"] = "safe gate note"
    lane["source_adapter_gate"][7] = "safe gate int-key note"
    lane["source_adapter_gate"][HostilePublicKey()] = (
        "secret-token hostile gate note"
    )
    lane["source_adapter_gate"]["audit_hashes"]["operator_override"] = hex32(0x73)
    lane["source_adapter_gate"]["audit_hashes"][7] = hex32(0x74)
    lane["source_adapter_gate"]["audit_hashes"][HostilePublicKey()] = hex32(0x75)
    lane["evm_live_metadata"]["operator_note"] = "safe evm note"
    lane["evm_live_metadata"][7] = "safe evm int-key note"
    lane["evm_live_metadata"][HostilePublicKey()] = "secret-token hostile evm note"
    lane["destination_binding"]["operator_note"] = "safe destination note"
    lane["destination_binding"][7] = "safe destination int-key note"
    lane["destination_binding"][HostilePublicKey()] = (
        "secret-token hostile destination note"
    )
    lane["route_allowlist"]["operator_note"] = "safe route note"
    lane["route_allowlist"][7] = "safe route int-key note"
    lane["route_allowlist"][HostilePublicKey()] = "secret-token hostile route note"
    lane["route_allowlist"]["route_canary"]["operator_note"] = "safe canary note"
    lane["route_allowlist"]["route_canary"][7] = "safe canary int-key note"
    lane["route_allowlist"]["route_canary"][HostilePublicKey()] = (
        "secret-token hostile canary note"
    )

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "all-lanes summary lanes[0] unexpected non-string field name" in blockers
    assert (
        "all-lanes summary lanes[0].source_record_hashes unexpected field "
        "operator_note"
    ) in blockers
    assert (
        "all-lanes summary lanes[0].source_record_hashes unexpected non-string "
        "field name"
    ) in blockers
    assert (
        "all-lanes summary lanes[0].source_adapter_gate unexpected field "
        "operator_note"
    ) in blockers
    assert (
        "all-lanes summary lanes[0].source_adapter_gate unexpected non-string "
        "field name"
    ) in blockers
    assert (
        "all-lanes summary lanes[0].source_adapter_gate.audit_hashes unexpected "
        "field operator_override"
    ) in blockers
    assert (
        "all-lanes summary lanes[0].source_adapter_gate.audit_hashes unexpected "
        "non-string field name"
    ) in blockers
    assert (
        "all-lanes summary lanes[0].evm_live_metadata unexpected field operator_note"
        in blockers
    )
    assert (
        "all-lanes summary lanes[0].evm_live_metadata unexpected non-string field "
        "name"
    ) in blockers
    assert (
        "all-lanes summary lanes[0].destination_binding unexpected field "
        "operator_note"
    ) in blockers
    assert (
        "all-lanes summary lanes[0].destination_binding unexpected non-string "
        "field name"
    ) in blockers
    assert (
        "all-lanes summary lanes[0].route_allowlist unexpected field operator_note"
        in blockers
    )
    assert (
        "all-lanes summary lanes[0].route_allowlist unexpected non-string field "
        "name"
    ) in blockers
    assert (
        "all-lanes summary lanes[0].route_allowlist.route_canary unexpected field "
        "operator_note"
    ) in blockers
    assert (
        "all-lanes summary lanes[0].route_allowlist.route_canary unexpected "
        "non-string field name"
    ) in blockers
    assert "all-lanes summary lanes[0] contains non-string field name" in blockers
    assert "all-lanes summary lanes are invalid" in blockers
    assert "safe lane int-key note" not in captured.out
    assert "safe source note" not in captured.out
    assert "safe source int-key note" not in captured.out
    assert "safe gate note" not in captured.out
    assert "safe gate int-key note" not in captured.out
    assert "safe evm note" not in captured.out
    assert "safe evm int-key note" not in captured.out
    assert "safe destination note" not in captured.out
    assert "safe destination int-key note" not in captured.out
    assert "safe route note" not in captured.out
    assert "safe route int-key note" not in captured.out
    assert "safe canary note" not in captured.out
    assert "safe canary int-key note" not in captured.out
    assert "secret-token" not in captured.out
    assert "hostile" not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_malformed_lanes_without_leaking(capsys):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    checklist_items = [
        {
            "id": item_id,
            "title": title,
            "ready": True,
            "blockers": [],
        }
        for item_id, title in module.RELEASE_CHECKLIST_TITLES.items()
    ]

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: {
        "production_ready": True,
        "blockers": [],
        "required_domains": [1, 2, 3, 4, 5],
        "supported_launch_domains": [1, 2, 3, 4, 5],
        "unsupported_launch_domains": [],
        "release_checklist": {"ready": True, "items": checklist_items},
        "lanes": [
            {
                "domain": module.SCCP_DOMAIN_ETH,
                "chain": "bsc",
                "production_ready": "true",
                "records": {
                    "source_verifier_material": True,
                    "source_adapter_deployment": True,
                    "destination_rollout": True,
                    "route_allowlist": True,
                    "secret-token-record": True,
                },
                "source_record_hashes": {
                    "source_verifier_material_hash": "secret-token-hash",
                },
                "source_adapter_gate": {
                    "required": True,
                    "ready": True,
                    "gate_hash": "0x" + "11" * 32,
                    "audit_hashes": {},
                    "blockers": [],
                },
                "destination_binding": {},
                "route_allowlist": {},
                "blockers": ["secret-token-lane-blocker"],
                "secret-token-lane": "secret-token-value",
            },
        ],
    }
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "all-lanes summary lanes[0] unexpected field with sensitive name" in blockers
    assert "all-lanes summary lanes[0] chain must match the lane domain" in blockers
    assert "all-lanes summary lanes[0] production_ready must be a boolean" in blockers
    assert (
        "all-lanes summary lanes[0] records unexpected field with sensitive name"
        in blockers
    )
    assert (
        "all-lanes summary lanes[0] blockers[0] contains sensitive name"
        in blockers
    )
    assert (
        "all-lanes summary lanes[0].source_record_hashes."
        "source_verifier_material_hash contains sensitive value"
    ) in blockers
    assert "all-lanes summary lanes missing domain 2" in blockers
    assert "all-lanes summary lanes are invalid" in blockers
    assert "secret-token" not in captured.out
    assert "secret-token" not in captured.err
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_source_adapter_gate_drift_without_leaking(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    eth_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    bsc_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_BSC
    )
    eth_lane["source_adapter_gate"]["required"] = "true"
    eth_lane["source_adapter_gate"]["ready"] = "true"
    eth_lane["source_adapter_gate"]["blockers"] = ["private&#45;key-gate-blocker"]
    bsc_lane["source_adapter_gate"]["gate_hash"] = hex32(0)
    bsc_lane["source_adapter_gate"]["audit_hashes"]["evm_source_gate_hash"] = hex32(0)

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert (
        "all-lanes summary lanes[0].source_adapter_gate.required must be a boolean"
        in blockers
    )
    assert (
        "all-lanes summary lanes[0].source_adapter_gate.ready must be a boolean"
        in blockers
    )
    assert (
        "all-lanes summary lanes[0].source_adapter_gate blockers[0] "
        "contains sensitive name"
    ) in blockers
    expected_gate_hash_blocker = "source adapter gate hash must be a canonical non-zero bytes32"
    assert (
        f"all-lanes summary lanes[1]: {expected_gate_hash_blocker} when required"
        in blockers
    )
    assert (
        "all-lanes summary lanes[1]: source adapter gate audit hashes "
        "evm_source_gate_hash must be a canonical non-zero bytes32"
    ) in blockers
    assert "private&#45;key" not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_not_ready_source_adapter_gate_semantics(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    lanes = {
        lane["domain"]: (index, lane)
        for index, lane in enumerate(summary["lanes"])
    }
    eth_index, eth_lane = lanes[module.SCCP_DOMAIN_ETH]
    bsc_index, bsc_lane = lanes[module.SCCP_DOMAIN_BSC]
    for lane in (eth_lane, bsc_lane):
        lane["production_ready"] = False
        lane["blockers"] = ["operator pending lane certification"]

    eth_gate = eth_lane["source_adapter_gate"]
    eth_gate["required"] = False
    eth_gate["ready"] = False
    eth_gate["blockers"] = ["operator pending source gate certification"]
    bsc_gate = bsc_lane["source_adapter_gate"]
    bsc_gate["ready"] = False
    bsc_gate["gate_hash"] = hex32(0xD1)
    bsc_gate["blockers"] = ["operator pending source gate certification"]
    forged_values = {hex32(0xD1), "operator pending source gate certification"}

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    expected_blockers = (
        (eth_index, "source adapter gate required flag must match lane policy"),
        (
            eth_index,
            "source adapter gate hash must be empty when not required",
        ),
        (
            eth_index,
            "source adapter gate audit hashes must be empty when not required",
        ),
        (
            eth_index,
            "source adapter gate ready must be true when not required",
        ),
        (
            eth_index,
            "source adapter gate blockers must be empty when not required",
        ),
        (
            bsc_index,
            "source adapter gate ready must be true when required",
        ),
        (
            bsc_index,
            "source adapter gate blockers must be empty when required",
        ),
        (
            bsc_index,
            "source adapter gate hash must match audit_hashes.evm_source_gate_hash",
        ),
    )
    for lane_index, expected in expected_blockers:
        assert f"all-lanes summary lanes[{lane_index}]: {expected}" in blockers
    assert "operator pending lane certification" not in captured.out
    for forged_value in forged_values:
        assert forged_value not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_active_copied_source_gate_missing_fields_when_lane_not_ready(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    eth_index, eth_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    assert eth_lane["production_ready"] is True

    eth_lane["production_ready"] = False
    eth_lane["blockers"] = ["operator pending active source gate field audit"]
    source_gate = eth_lane["source_adapter_gate"]
    missing_fields = tuple(module.PUBLIC_LANE_SOURCE_ADAPTER_GATE_FIELDS)
    for field in missing_fields:
        source_gate.pop(field, None)

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    for field in missing_fields:
        assert (
            f"all-lanes summary lanes[{eth_index}]."
            f"source_adapter_gate missing field {field}"
        ) in blockers
    assert "operator pending active source gate field audit" not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_not_ready_active_lane_ready_drift(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    eth_index, eth_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    assert eth_lane["production_ready"] is True

    eth_lane["production_ready"] = False
    eth_lane["blockers"] = ["operator pending active lane certification"]
    eth_lane["records"]["source_verifier_material"] = False
    eth_lane["records"]["route_allowlist"] = False
    eth_gate = eth_lane["source_adapter_gate"]
    eth_gate["ready"] = False
    eth_gate["blockers"] = ["operator pending active source gate certification"]

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    expected_root_blockers = (
        "production_ready must be true",
        "records.source_verifier_material must be true",
        "records.route_allowlist must be true",
        "blockers must be empty",
    )
    for expected in expected_root_blockers:
        assert f"all-lanes summary lanes[{eth_index}] {expected}" in blockers
    expected_nested_blockers = (
        "source_adapter_gate.ready must be true",
        "source_adapter_gate blockers must be empty",
    )
    for expected in expected_nested_blockers:
        assert f"all-lanes summary lanes[{eth_index}].{expected}" in blockers
    assert (
        f"all-lanes summary lanes[{eth_index}]: "
        "source adapter gate ready must be true when required"
    ) in blockers
    assert (
        f"all-lanes summary lanes[{eth_index}]: "
        "source adapter gate blockers must be empty when required"
    ) in blockers
    assert "operator pending active lane certification" not in captured.out
    assert "operator pending active source gate certification" not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_active_copied_record_flags_missing_when_lane_not_ready(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    eth_index, eth_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    assert eth_lane["production_ready"] is True

    eth_lane["production_ready"] = False
    eth_lane["blockers"] = ["operator pending active record flag audit"]
    eth_lane["records"] = {}

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    for field in module.PUBLIC_LANE_RECORD_FIELDS:
        assert (
            f"all-lanes summary lanes[{eth_index}] records.{field} "
            "must be a boolean"
        ) in blockers
    assert "operator pending active record flag audit" not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_active_copied_lane_root_missing_fields_when_lane_not_ready(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    eth_index, eth_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    assert eth_lane["production_ready"] is True

    eth_lane["production_ready"] = False
    eth_lane["blockers"] = ["operator pending active lane-root field audit"]
    missing_fields = tuple(
        field for field in module.PUBLIC_LANE_FIELDS if field != "domain"
    )
    for field in missing_fields:
        eth_lane.pop(field, None)

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    for field in missing_fields:
        assert f"all-lanes summary lanes[{eth_index}] missing field {field}" in (
            blockers
        )
    assert "operator pending active lane-root field audit" not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_ready_lane_hash_shape_drift(capsys):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    eth_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    bsc_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_BSC
    )
    bare_source_hash = eth_lane["source_record_hashes"][
        "source_verifier_material_hash"
    ][2:]
    bare_route_hash = bsc_lane["route_allowlist"]["route_allowlist_hash"][2:]
    eth_lane["source_record_hashes"][
        "source_verifier_material_hash"
    ] = bare_source_hash
    eth_lane["destination_binding"][
        "expected_destination_binding_hash"
    ] = "not-a-destination-hash"
    bsc_lane["route_allowlist"]["route_allowlist_hash"] = bare_route_hash
    bsc_lane["route_allowlist"]["route_canary"]["evidence_hash"] = (
        "not-a-canary-hash"
    )
    bsc_lane["route_allowlist"]["route_canary"]["message_id"] = hex32(0)

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    expected_source_hash_blocker = (
        "source_record_hashes.source_verifier_material_hash must be a "
        "canonical non-zero bytes32"
    )
    expected_destination_hash_blocker = (
        "destination_binding.expected_destination_binding_hash must be a "
        "canonical non-zero bytes32"
    )
    expected_route_hash_blocker = (
        "route_allowlist.route_allowlist_hash must be a canonical non-zero bytes32"
    )
    expected_canary_hash_blocker = (
        "route_allowlist.route_canary.evidence_hash must be a canonical "
        "non-zero bytes32"
    )
    expected_canary_message_blocker = (
        "route_allowlist.route_canary.message_id must be a canonical non-zero bytes32"
    )
    assert f"all-lanes summary lanes[0].{expected_source_hash_blocker}" in blockers
    assert f"all-lanes summary lanes[0].{expected_destination_hash_blocker}" in blockers
    assert f"all-lanes summary lanes[1].{expected_route_hash_blocker}" in blockers
    assert f"all-lanes summary lanes[1].{expected_canary_hash_blocker}" in blockers
    assert f"all-lanes summary lanes[1].{expected_canary_message_blocker}" in blockers
    assert bare_source_hash not in captured.out
    assert "not-a-destination-hash" not in captured.out
    assert bare_route_hash not in captured.out
    assert "not-a-canary-hash" not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_ready_source_record_template_replay(capsys):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    lanes = {
        lane["domain"]: (index, lane)
        for index, lane in enumerate(summary["lanes"])
    }
    forged_hashes = set()

    for domain, (_index, lane) in lanes.items():
        profile = module.LANE_PROFILES[domain]
        template_hashes = tuple(
            module._source_material_template_hashes(profile).values()
        )
        assert template_hashes
        for offset, field in enumerate(module.PUBLIC_LANE_SOURCE_RECORD_HASH_FIELDS):
            forged_hash = "0x" + template_hashes[offset].hex()
            lane["source_record_hashes"][field] = forged_hash
            forged_hashes.add(forged_hash)

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    for _domain, (index, _lane) in lanes.items():
        for field in module.PUBLIC_LANE_SOURCE_RECORD_HASH_FIELDS:
            assert (
                f"all-lanes summary lanes[{index}].source_record_hashes.{field} "
                "must be deployed evidence, not built-in template material"
            ) in blockers
    for forged_hash in forged_hashes:
        assert forged_hash not in captured.out
        assert forged_hash[2:] not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_route_canary_template_replay(capsys):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    lanes = {
        lane["domain"]: (index, lane)
        for index, lane in enumerate(summary["lanes"])
    }
    forged_hashes = set()

    for domain, (_index, lane) in lanes.items():
        profile = module.LANE_PROFILES[domain]
        template_hash = next(
            iter(module._source_material_template_hashes(profile).values())
        )
        forged_hash = "0x" + template_hash.hex()
        lane["route_allowlist"]["route_canary"]["evidence_hash"] = forged_hash
        forged_hashes.add(forged_hash)

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    for _domain, (index, _lane) in lanes.items():
        assert (
            f"all-lanes summary lanes[{index}].route_allowlist.route_canary."
            "evidence_hash must be live evidence, not built-in template material"
        ) in blockers
    for forged_hash in forged_hashes:
        assert forged_hash not in captured.out
        assert forged_hash[2:] not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_route_canary_transcript_template_replay(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    lanes = {
        lane["domain"]: (index, lane)
        for index, lane in enumerate(summary["lanes"])
    }
    forged_hashes = set()
    forged_fields: dict[int, tuple[int, str]] = {}

    for domain, (index, lane) in lanes.items():
        candidate_fields = tuple(
            field
            for field in module.PUBLIC_LANE_ROUTE_CANARY_TEMPLATE_HASH_FIELDS_BY_DOMAIN[
                domain
            ]
            if field != "evidence_hash"
        )
        if not candidate_fields:
            continue
        profile = module.LANE_PROFILES[domain]
        template_hash = next(
            iter(module._source_material_template_hashes(profile).values())
        )
        forged_hash = "0x" + template_hash.hex()
        field = candidate_fields[0]
        lane["route_allowlist"]["route_canary"][field] = forged_hash
        forged_fields[domain] = (index, field)
        forged_hashes.add(forged_hash)

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert forged_fields
    for _domain, (index, field) in forged_fields.items():
        assert (
            f"all-lanes summary lanes[{index}].route_allowlist.route_canary."
            f"{field} must be live evidence, not built-in template material"
        ) in blockers
    for forged_hash in forged_hashes:
        assert forged_hash not in captured.out
        assert forged_hash[2:] not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_template_loader_failure_without_leaking(capsys):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    original_template_hashes = module._source_material_template_hashes
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))

    def broken_template_hashes(_profile):
        raise RuntimeError("secret-token-template-loader")

    module._source_material_template_hashes = broken_template_hashes
    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate
        module._source_material_template_hashes = original_template_hashes

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "template material validation failed" in blockers
    assert "secret-token-template-loader" not in captured.out
    assert "secret-token-template-loader" not in captured.err
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_source_gate_requirement_failure_without_leaking(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    original_requirements = module._source_adapter_gate_requirements
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))

    def broken_requirements(_domain):
        raise RuntimeError("secret-token-source-gate-requirements")

    module._source_adapter_gate_requirements = broken_requirements
    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate
        module._source_adapter_gate_requirements = original_requirements

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "source adapter gate requirement validation failed" in blockers
    assert "secret-token-source-gate-requirements" not in captured.out
    assert "secret-token-source-gate-requirements" not in captured.err
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_not_ready_source_record_template_replay(capsys):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    eth_index, eth_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    template_hash = next(
        iter(
            module._source_material_template_hashes(
                module.LANE_PROFILES[module.SCCP_DOMAIN_ETH]
            ).values()
        )
    )
    forged_hash = "0x" + template_hash.hex()
    eth_lane["source_record_hashes"]["source_verifier_material_hash"] = forged_hash

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    assert (
        f"all-lanes summary lanes[{eth_index}].source_record_hashes."
        "source_verifier_material_hash must be deployed evidence, "
        "not built-in template material"
    ) in blockers
    assert forged_hash not in captured.out
    assert forged_hash[2:] not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_active_copied_source_record_template_replay_when_lane_not_ready(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    eth_index, eth_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    eth_lane["production_ready"] = False
    eth_lane["blockers"] = ["operator pending active source-record audit"]
    template_hashes = tuple(
        module._source_material_template_hashes(
            module.LANE_PROFILES[module.SCCP_DOMAIN_ETH]
        ).values()
    )
    assert len(template_hashes) >= 2
    forged_material_hash = "0x" + template_hashes[0].hex()
    forged_deployment_hash = "0x" + template_hashes[1].hex()
    eth_lane["source_record_hashes"][
        "source_verifier_material_hash"
    ] = forged_material_hash
    eth_lane["source_record_hashes"][
        "source_adapter_engine_deployment_hash"
    ] = forged_deployment_hash

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    expected_fields = (
        "source_verifier_material_hash",
        "source_adapter_engine_deployment_hash",
    )
    for field in expected_fields:
        assert (
            f"all-lanes summary lanes[{eth_index}].source_record_hashes.{field} "
            "must be deployed evidence, not built-in template material"
        ) in blockers
    assert "operator pending active source-record audit" not in captured.out
    for forged_hash in (forged_material_hash, forged_deployment_hash):
        assert forged_hash not in captured.out
        assert forged_hash[2:] not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_active_copied_source_gate_template_replay_when_required_false(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    eth_index, eth_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    eth_lane["production_ready"] = False
    eth_lane["blockers"] = ["operator pending active source-gate audit"]
    template_hash = next(
        iter(
            module._source_material_template_hashes(
                module.LANE_PROFILES[module.SCCP_DOMAIN_ETH]
            ).values()
        )
    )
    forged_hash = "0x" + template_hash.hex()
    gate_field, _audit_fields = module._source_adapter_gate_requirements(
        module.SCCP_DOMAIN_ETH
    )
    source_gate = eth_lane["source_adapter_gate"]
    source_gate["required"] = False
    source_gate["gate_hash"] = forged_hash
    source_gate["audit_hashes"][gate_field] = forged_hash

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    assert (
        f"all-lanes summary lanes[{eth_index}]: source adapter gate hash "
        "must be deployed evidence, not built-in template material"
    ) in blockers
    assert (
        f"all-lanes summary lanes[{eth_index}]: source adapter gate audit hashes "
        f"{gate_field} must be deployed evidence, not built-in template material"
    ) in blockers
    assert "operator pending active source-gate audit" not in captured.out
    assert forged_hash not in captured.out
    assert forged_hash[2:] not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_active_copied_source_gate_template_replay_when_required_malformed(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    eth_index, eth_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    eth_lane["production_ready"] = False
    eth_lane["blockers"] = ["operator pending active source-gate audit"]
    template_hash = next(
        iter(
            module._source_material_template_hashes(
                module.LANE_PROFILES[module.SCCP_DOMAIN_ETH]
            ).values()
        )
    )
    forged_hash = "0x" + template_hash.hex()
    gate_field, _audit_fields = module._source_adapter_gate_requirements(
        module.SCCP_DOMAIN_ETH
    )
    source_gate = eth_lane["source_adapter_gate"]
    source_gate["required"] = "true"
    source_gate["gate_hash"] = forged_hash
    source_gate["audit_hashes"][gate_field] = forged_hash

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert (
        f"all-lanes summary lanes[{eth_index}].source_adapter_gate.required "
        "must be a boolean"
    ) in blockers
    assert (
        f"all-lanes summary lanes[{eth_index}]: source adapter gate hash "
        "must be deployed evidence, not built-in template material"
    ) in blockers
    assert (
        f"all-lanes summary lanes[{eth_index}]: source adapter gate audit hashes "
        f"{gate_field} must be deployed evidence, not built-in template material"
    ) in blockers
    assert "operator pending active source-gate audit" not in captured.out
    assert forged_hash not in captured.out
    assert forged_hash[2:] not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_active_copied_source_gate_template_replay_when_ready_malformed(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    eth_index, eth_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    eth_lane["production_ready"] = False
    eth_lane["blockers"] = ["operator pending active source-gate audit"]
    template_hash = next(
        iter(
            module._source_material_template_hashes(
                module.LANE_PROFILES[module.SCCP_DOMAIN_ETH]
            ).values()
        )
    )
    forged_hash = "0x" + template_hash.hex()
    gate_field, _audit_fields = module._source_adapter_gate_requirements(
        module.SCCP_DOMAIN_ETH
    )
    source_gate = eth_lane["source_adapter_gate"]
    source_gate["required"] = True
    source_gate["ready"] = "true"
    source_gate["gate_hash"] = forged_hash
    source_gate["audit_hashes"][gate_field] = forged_hash

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert (
        f"all-lanes summary lanes[{eth_index}].source_adapter_gate.ready "
        "must be a boolean"
    ) in blockers
    assert (
        f"all-lanes summary lanes[{eth_index}]: source adapter gate hash "
        "must be deployed evidence, not built-in template material"
    ) in blockers
    assert (
        f"all-lanes summary lanes[{eth_index}]: source adapter gate audit hashes "
        f"{gate_field} must be deployed evidence, not built-in template material"
    ) in blockers
    assert "operator pending active source-gate audit" not in captured.out
    assert forged_hash not in captured.out
    assert forged_hash[2:] not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_active_copied_source_record_missing_fields_when_lane_not_ready(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    eth_index, eth_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    assert eth_lane["production_ready"] is True

    eth_lane["production_ready"] = False
    eth_lane["blockers"] = ["operator pending active source-record field audit"]
    source_hashes = eth_lane["source_record_hashes"]
    missing_fields = tuple(module.PUBLIC_LANE_SOURCE_RECORD_HASH_FIELDS)
    for field in missing_fields:
        source_hashes.pop(field, None)

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    for field in missing_fields:
        assert (
            f"all-lanes summary lanes[{eth_index}]."
            f"source_record_hashes missing field {field}"
        ) in blockers
    assert "operator pending active source-record field audit" not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_not_ready_nested_hash_shape_drift(capsys):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    lanes = {
        lane["domain"]: (index, lane)
        for index, lane in enumerate(summary["lanes"])
    }
    eth_index, eth_lane = lanes[module.SCCP_DOMAIN_ETH]
    bsc_index, bsc_lane = lanes[module.SCCP_DOMAIN_BSC]
    sol_index, sol_lane = lanes[module.SCCP_DOMAIN_SOL]
    ton_index, ton_lane = lanes[module.SCCP_DOMAIN_TON]
    tron_index, tron_lane = lanes[module.SCCP_DOMAIN_TRON]
    bsc_gate_field, _bsc_audit_fields = module._source_adapter_gate_requirements(
        module.SCCP_DOMAIN_BSC
    )
    forged_values = {
        "not-a-source-hash",
        "not-a-gate-hash",
        "not-an-audit-hash",
        "not-a-destination-hash",
        "not-a-route-hash",
        "not-a-canary-hash",
    }

    eth_lane["source_record_hashes"][
        "source_adapter_engine_deployment_hash"
    ] = "not-a-source-hash"
    bsc_lane["source_adapter_gate"]["gate_hash"] = "not-a-gate-hash"
    bsc_lane["source_adapter_gate"]["audit_hashes"][bsc_gate_field] = (
        "not-an-audit-hash"
    )
    sol_lane["destination_binding"][
        "destination_binding_hash"
    ] = "not-a-destination-hash"
    ton_lane["route_allowlist"]["route_allowlist_hash"] = "not-a-route-hash"
    tron_lane["route_allowlist"]["route_canary"]["evidence_hash"] = (
        "not-a-canary-hash"
    )

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    expected_blockers = (
        (
            eth_index,
            "source_record_hashes.source_adapter_engine_deployment_hash "
            "must be a canonical non-zero bytes32",
        ),
        (
            bsc_index,
            "source_adapter_gate.gate_hash must be a canonical non-zero bytes32",
        ),
        (
            bsc_index,
            f"source_adapter_gate.audit_hashes.{bsc_gate_field} must be a "
            "canonical non-zero bytes32",
        ),
        (
            sol_index,
            "destination_binding.destination_binding_hash must be a canonical "
            "non-zero bytes32",
        ),
        (
            ton_index,
            "route_allowlist.route_allowlist_hash must be a canonical non-zero "
            "bytes32",
        ),
        (
            tron_index,
            "route_allowlist.route_canary.evidence_hash must be a canonical "
            "non-zero bytes32",
        ),
    )
    for lane_index, expected in expected_blockers:
        assert f"all-lanes summary lanes[{lane_index}].{expected}" in blockers
    for forged_value in forged_values:
        assert forged_value not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_not_ready_nested_scalar_shape_drift(capsys):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    lanes = {
        lane["domain"]: (index, lane)
        for index, lane in enumerate(summary["lanes"])
    }
    eth_index, eth_lane = lanes[module.SCCP_DOMAIN_ETH]
    bsc_index, bsc_lane = lanes[module.SCCP_DOMAIN_BSC]
    sol_index, sol_lane = lanes[module.SCCP_DOMAIN_SOL]
    ton_index, ton_lane = lanes[module.SCCP_DOMAIN_TON]
    tron_index, tron_lane = lanes[module.SCCP_DOMAIN_TRON]
    forged_values = {
        " 01 ",
        " finalized ",
        "yes-recomputed",
        " bsc-mainnet ",
        "yes-route",
        " forged-status ",
        "yes-bound",
        "123-string",
        "bad-owner-address",
        "42-int-slot",
        "lt-int-zero",
    }

    eth_lane["evm_live_metadata"]["source_rpc_chain_id"] = " 01 "
    eth_lane["evm_live_metadata"]["source_block_tag"] = " finalized "
    bsc_lane["destination_binding"]["recomputed"] = "yes-recomputed"
    bsc_lane["destination_binding"]["destination_network_id"] = " bsc-mainnet "
    sol_lane["route_allowlist"]["expected_route_allowlist_hash_matches"] = (
        "yes-route"
    )
    sol_lane["route_allowlist"]["route_canary"]["solana_programdata_slot"] = (
        "42-int-slot"
    )
    ton_lane["route_allowlist"]["route_canary"]["ton_last_transaction_lt"] = (
        "lt-int-zero"
    )
    tron_canary = tron_lane["route_allowlist"]["route_canary"]
    tron_canary["status"] = " forged-status "
    tron_canary["evidence_bound"] = "yes-bound"
    tron_canary["receipt_block_number"] = "123-string"
    tron_canary["signature_recovered_address"] = "bad-owner-address"

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    expected_blockers = (
        (
            eth_index,
            "evm_live_metadata.source_rpc_chain_id must be a canonical "
            "positive decimal string",
        ),
        (
            eth_index,
            "evm_live_metadata.source_block_tag must be a non-empty "
            "canonical string",
        ),
        (
            bsc_index,
            "destination_binding.recomputed must be a boolean",
        ),
        (
            bsc_index,
            "destination_binding.destination_network_id must be a non-empty "
            "canonical string",
        ),
        (
            sol_index,
            "route_allowlist.expected_route_allowlist_hash_matches must be a "
            "boolean",
        ),
        (
            sol_index,
            "route_allowlist.route_canary.solana_programdata_slot must be a "
            "canonical positive decimal string",
        ),
        (
            ton_index,
            "route_allowlist.route_canary.ton_last_transaction_lt must be a "
            "canonical positive decimal string",
        ),
        (
            tron_index,
            "route_allowlist.route_canary.status must be a non-empty "
            "canonical string",
        ),
        (
            tron_index,
            "route_allowlist.route_canary.evidence_bound must be a boolean",
        ),
        (
            tron_index,
            "route_allowlist.route_canary.receipt_block_number must be a "
            "positive integer",
        ),
        (
            tron_index,
            "route_allowlist.route_canary.signature_recovered_address must be "
            "a non-zero TRON address",
        ),
    )
    for lane_index, expected in expected_blockers:
        assert f"all-lanes summary lanes[{lane_index}].{expected}" in blockers
    for forged_value in forged_values:
        assert forged_value not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_not_ready_nested_semantic_drift(capsys):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    lanes = {
        lane["domain"]: (index, lane)
        for index, lane in enumerate(summary["lanes"])
    }
    eth_index, eth_lane = lanes[module.SCCP_DOMAIN_ETH]
    bsc_index, bsc_lane = lanes[module.SCCP_DOMAIN_BSC]
    tron_index, tron_lane = lanes[module.SCCP_DOMAIN_TRON]
    forged_values = {
        "safe-block-tag",
        "failed",
        "wrong_source",
        "0x41" + "22" * 20,
        "0x41" + "33" * 20,
    }

    eth_lane["evm_live_metadata"]["source_rpc_chain_id"] = "2"
    eth_lane["evm_live_metadata"]["source_block_tag"] = "safe-block-tag"
    eth_canary = eth_lane["route_allowlist"]["route_canary"]
    eth_canary["status"] = "failed"
    eth_canary["evidence_source"] = "wrong_source"
    eth_canary["evidence_bound"] = False
    eth_canary["target_domain"] = module.SCCP_DOMAIN_BSC
    eth_canary["proof_version"] = 2
    eth_canary["proof_source_domain"] = module.SCCP_DOMAIN_ETH
    eth_canary["message_proof_used"] = False
    eth_canary["receipt_block_finalized"] = False
    bsc_lane["destination_binding"][
        "expected_destination_binding_hash_matches"
    ] = False
    bsc_lane["destination_binding"]["recomputed"] = False
    bsc_lane["route_allowlist"]["expected_route_allowlist_hash_matches"] = False
    tron_canary = tron_lane["route_allowlist"]["route_canary"]
    tron_canary["transaction_owner_address"] = "0x41" + "22" * 20
    tron_canary["signature_recovered_address"] = "0x41" + "33" * 20
    tron_canary["raw_data_owner_matches_transaction"] = False
    tron_canary["signature_recovers_to_owner"] = False

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    expected_blockers = (
        (eth_index, "evm_live_metadata.source_rpc_chain_id must be 1"),
        (
            eth_index,
            "evm_live_metadata.source_block_tag must be finalized",
        ),
        (eth_index, "route_allowlist.route_canary.status must be passed"),
        (
            eth_index,
            "route_allowlist.route_canary.evidence_source must be "
            "evm_message_proof_accepted_transaction",
        ),
        (eth_index, "route_allowlist.route_canary.evidence_bound must be true"),
        (
            eth_index,
            "route_allowlist.route_canary.target_domain must match lane domain",
        ),
        (eth_index, "route_allowlist.route_canary.proof_version must be 1"),
        (
            eth_index,
            "route_allowlist.route_canary.proof_source_domain must be "
            "SORA domain",
        ),
        (
            eth_index,
            "route_allowlist.route_canary.message_proof_used must be true",
        ),
        (
            eth_index,
            "route_allowlist.route_canary.receipt_block_finalized must be true",
        ),
        (
            bsc_index,
            "destination_binding.expected_destination_binding_hash_matches "
            "must be true",
        ),
        (bsc_index, "destination_binding.recomputed must be true"),
        (
            bsc_index,
            "route_allowlist.expected_route_allowlist_hash_matches must be true",
        ),
        (
            tron_index,
            "route_allowlist.route_canary.signature_recovered_address must "
            "match transaction_owner_address",
        ),
        (
            tron_index,
            "route_allowlist.route_canary.raw_data_owner_matches_transaction "
            "must be true",
        ),
        (
            tron_index,
            "route_allowlist.route_canary.signature_recovers_to_owner must be true",
        ),
    )
    for lane_index, expected in expected_blockers:
        assert f"all-lanes summary lanes[{lane_index}].{expected}" in blockers
    for forged_value in forged_values:
        assert forged_value not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_active_copied_route_canary_semantic_drift_when_lane_not_ready(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    eth_index, eth_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    eth_lane["production_ready"] = False
    eth_lane["blockers"] = ["operator pending active route canary audit"]

    taken_hashes: set[str] = set()

    def collect_hashes(value):
        if isinstance(value, dict):
            for child in value.values():
                collect_hashes(child)
        elif isinstance(value, list):
            for child in value:
                collect_hashes(child)
        elif isinstance(value, str) and value.startswith("0x") and len(value) == 66:
            taken_hashes.add(value)

    collect_hashes(summary)
    forged_hashes: list[str] = []
    for seed in range(0x6A, 0x16A):
        candidate = hex32(seed)
        if candidate not in taken_hashes:
            forged_hashes.append(candidate)
        if len(forged_hashes) == 2:
            break
    if len(forged_hashes) != 2:
        raise AssertionError("could not forge distinct active canary hashes")

    route_canary = eth_lane["route_allowlist"]["route_canary"]
    route_canary["status"] = "failed"
    route_canary["evidence_source"] = "operator_review_note"
    route_canary["evidence_bound"] = False
    route_canary["route_allowlist_hash"] = forged_hashes[0]
    route_canary["destination_binding_hash"] = forged_hashes[1]
    forged_values = {
        "failed",
        "operator_review_note",
        *forged_hashes,
    }

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    expected_blockers = (
        "route_allowlist.route_canary.status must be passed",
        (
            "route_allowlist.route_canary.evidence_source must be "
            "evm_message_proof_accepted_transaction"
        ),
        "route_allowlist.route_canary.evidence_bound must be true",
        (
            "route_allowlist.route_canary.route_allowlist_hash must match "
            "lane route_allowlist_hash"
        ),
        (
            "route_allowlist.route_canary.destination_binding_hash must match "
            "lane destination_binding_hash"
        ),
    )
    for expected in expected_blockers:
        assert f"all-lanes summary lanes[{eth_index}].{expected}" in blockers
    assert "operator pending active route canary audit" not in captured.out
    for forged_value in forged_values:
        assert forged_value not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_active_copied_evm_route_canary_proof_drift_when_lane_not_ready(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    eth_index, eth_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    eth_lane["production_ready"] = False
    eth_lane["blockers"] = ["operator pending active route canary proof audit"]

    route_canary = eth_lane["route_allowlist"]["route_canary"]
    route_canary["target_domain"] = module.SCCP_DOMAIN_BSC
    route_canary["proof_version"] = 2
    route_canary["proof_source_domain"] = module.SCCP_DOMAIN_ETH
    route_canary["message_proof_used"] = False
    route_canary["receipt_block_finalized"] = False

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    expected_blockers = (
        "route_allowlist.route_canary.target_domain must match lane domain",
        "route_allowlist.route_canary.proof_version must be 1",
        "route_allowlist.route_canary.proof_source_domain must be SORA domain",
        "route_allowlist.route_canary.message_proof_used must be true",
        "route_allowlist.route_canary.receipt_block_finalized must be true",
    )
    for expected in expected_blockers:
        assert f"all-lanes summary lanes[{eth_index}].{expected}" in blockers
    assert "operator pending active route canary proof audit" not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_active_copied_evm_route_canary_transcript_drift_when_lane_not_ready(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    eth_index, eth_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    eth_lane["production_ready"] = False
    eth_lane["blockers"] = ["operator pending active route canary transcript audit"]

    route_canary = eth_lane["route_allowlist"]["route_canary"]
    route_canary["finality_block_hash"] = "0x" + "00" * 32
    route_canary["receipt_block_hash"] = route_canary["transaction_hash"]
    route_canary["message_id"] = eth_lane["route_allowlist"]["route_allowlist_hash"]

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    expected_blockers = (
        (
            "route_allowlist.route_canary.finality_block_hash must be a "
            "canonical non-zero bytes32"
        ),
        (
            "route_allowlist.route_canary hash role receipt_block_hash must not "
            "reuse transaction_hash"
        ),
        (
            "route_allowlist.route_canary hash role message_id must not reuse "
            "route_allowlist_hash"
        ),
    )
    for expected in expected_blockers:
        assert f"all-lanes summary lanes[{eth_index}].{expected}" in blockers
    assert "operator pending active route canary transcript audit" not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_active_copied_evm_route_canary_scalar_drift_when_lane_not_ready(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    eth_index, eth_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    eth_lane["production_ready"] = False
    eth_lane["blockers"] = ["operator pending active route canary scalar audit"]

    route_canary = eth_lane["route_allowlist"]["route_canary"]
    route_canary["log_index"] = -1
    route_canary["receipt_block_number"] = 0

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    expected_blockers = (
        "route_allowlist.route_canary.log_index must be a u32 integer",
        "route_allowlist.route_canary.receipt_block_number must be a positive integer",
    )
    for expected in expected_blockers:
        assert f"all-lanes summary lanes[{eth_index}].{expected}" in blockers
    assert "operator pending active route canary scalar audit" not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_active_copied_evm_route_canary_missing_fields_when_lane_not_ready(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    eth_index, eth_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    eth_lane["production_ready"] = False
    eth_lane["blockers"] = ["operator pending active route canary field audit"]

    route_canary = eth_lane["route_allowlist"]["route_canary"]
    missing_fields = tuple(module.PUBLIC_LANE_EVM_ROUTE_CANARY_FIELDS)
    for field in missing_fields:
        route_canary.pop(field, None)

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    for field in missing_fields:
        assert (
            f"all-lanes summary lanes[{eth_index}]."
            f"route_allowlist.route_canary missing field {field}"
        ) in blockers
    assert "operator pending active route canary field audit" not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_not_ready_claimed_ready_lane_drift(capsys):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    eth_index, eth_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    assert eth_lane["production_ready"] is True
    eth_lane["records"]["source_verifier_material"] = False
    eth_lane["source_record_hashes"]["source_verifier_material_hash"] = ""
    eth_lane["source_adapter_gate"]["ready"] = False
    eth_lane["destination_binding"][
        "expected_destination_binding_hash_matches"
    ] = False
    eth_lane["route_allowlist"]["route_canary"]["evidence_hash"] = ""
    eth_lane["blockers"] = ["claimed-ready lane blocker"]

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    expected_blockers = (
        f"all-lanes summary lanes[{eth_index}] "
        "records.source_verifier_material must be true",
        f"all-lanes summary lanes[{eth_index}].source_record_hashes."
        "source_verifier_material_hash must be a canonical non-zero bytes32",
        f"all-lanes summary lanes[{eth_index}].source_adapter_gate.ready "
        "must be true",
        f"all-lanes summary lanes[{eth_index}].destination_binding."
        "expected_destination_binding_hash_matches must be true",
        f"all-lanes summary lanes[{eth_index}].route_allowlist.route_canary."
        "evidence_hash must be a canonical non-zero bytes32",
        f"all-lanes summary lanes[{eth_index}] blockers must be empty",
    )
    for expected in expected_blockers:
        assert expected in blockers
    assert "claimed-ready lane blocker" not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_not_ready_claimed_ready_destination_family_drift(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    lanes = {
        lane["domain"]: (index, lane)
        for index, lane in enumerate(summary["lanes"])
    }
    bsc_index, bsc_lane = lanes[module.SCCP_DOMAIN_BSC]
    sol_index, sol_lane = lanes[module.SCCP_DOMAIN_SOL]
    ton_index, ton_lane = lanes[module.SCCP_DOMAIN_TON]
    tron_index, tron_lane = lanes[module.SCCP_DOMAIN_TRON]
    forged_values = {
        "bsc-mainnet",
        "0X" + "44" * 20,
        "0x" + "55" * 20,
        "0x" + "66" * 32,
    }

    assert bsc_lane["production_ready"] is True
    bsc_lane["destination_binding"].pop("destination_network_id", None)
    bsc_lane["destination_binding"]["destination_bridge_address"] = "0X" + "44" * 20
    sol_lane["destination_binding"]["destination_bridge_address"] = "0x" + "55" * 20
    ton_lane["destination_binding"]["destination_network_id"] = "0x" + "66" * 32
    tron_lane["destination_binding"].pop("destination_network_id", None)
    tron_lane["destination_binding"]["destination_bridge_address"] = "0x" + "55" * 20

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    expected_blockers = (
        (
            bsc_index,
            "destination_binding.destination_network_id is required for "
            "EVM-family lanes",
        ),
        (
            bsc_index,
            "destination_binding.destination_network_id must be a canonical "
            "non-zero bytes32",
        ),
        (
            bsc_index,
            "destination_binding.destination_bridge_address must be a "
            "canonical non-zero 20-byte hex value",
        ),
        (
            sol_index,
            "destination_binding.destination_bridge_address is only valid for "
            "EVM-family lanes",
        ),
        (
            ton_index,
            "destination_binding.destination_network_id is only valid for "
            "EVM-family or TRON lanes",
        ),
        (
            tron_index,
            "destination_binding.destination_network_id is required for TRON lanes",
        ),
        (
            tron_index,
            "destination_binding.destination_network_id must be a canonical "
            "non-zero bytes32",
        ),
        (
            tron_index,
            "destination_binding.destination_bridge_address is only valid for "
            "EVM-family lanes",
        ),
    )
    for lane_index, expected in expected_blockers:
        assert f"all-lanes summary lanes[{lane_index}].{expected}" in blockers
    for forged_value in forged_values:
        assert forged_value not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_active_copied_destination_family_drift_when_lane_not_ready(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    eth_index, eth_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    eth_lane["production_ready"] = False
    eth_lane["blockers"] = ["operator pending active destination family audit"]

    destination = eth_lane["destination_binding"]
    destination.pop("destination_network_id", None)
    destination["destination_bridge_address"] = "0X" + "44" * 20
    forged_values = {"0X" + "44" * 20}

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    expected_blockers = (
        "destination_binding.destination_network_id is required for EVM-family lanes",
        (
            "destination_binding.destination_network_id must be a canonical "
            "non-zero bytes32"
        ),
        (
            "destination_binding.destination_bridge_address must be a "
            "canonical non-zero 20-byte hex value"
        ),
    )
    for expected in expected_blockers:
        assert f"all-lanes summary lanes[{eth_index}].{expected}" in blockers
    assert "operator pending active destination family audit" not in captured.out
    for forged_value in forged_values:
        assert forged_value not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_active_copied_destination_binding_missing_fields_when_lane_not_ready(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    eth_index, eth_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    assert eth_lane["production_ready"] is True

    eth_lane["production_ready"] = False
    eth_lane["blockers"] = ["operator pending active destination-binding field audit"]
    destination = eth_lane["destination_binding"]
    missing_fields = tuple(module.PUBLIC_LANE_DESTINATION_BINDING_FIELDS)
    for field in missing_fields:
        destination.pop(field, None)

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    for field in module.PUBLIC_LANE_DESTINATION_BINDING_REQUIRED_FIELDS:
        assert (
            f"all-lanes summary lanes[{eth_index}]."
            f"destination_binding missing field {field}"
        ) in blockers
    expected_family_blockers = (
        "destination_binding.destination_network_id is required for EVM-family lanes",
        "destination_binding.destination_bridge_address is required for EVM-family lanes",
    )
    for expected in expected_family_blockers:
        assert f"all-lanes summary lanes[{eth_index}].{expected}" in blockers
    assert (
        "operator pending active destination-binding field audit"
        not in captured.out
    )
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_not_ready_claimed_ready_hash_binding_drift(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    bsc_index, bsc_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_BSC
    )
    assert bsc_lane["production_ready"] is True

    def replacement_hash(current: str, seed: int) -> str:
        candidate = hex32(seed)
        return hex32(seed + 1) if candidate == current else candidate

    destination = bsc_lane["destination_binding"]
    route = bsc_lane["route_allowlist"]
    canary = route["route_canary"]
    forged_destination_expected = replacement_hash(
        destination["destination_binding_hash"],
        0xA0,
    )
    forged_route_expected = replacement_hash(route["route_allowlist_hash"], 0xA2)
    forged_canary_route = replacement_hash(route["route_allowlist_hash"], 0xA4)
    forged_canary_destination = replacement_hash(
        destination["destination_binding_hash"],
        0xA6,
    )
    forged_values = {
        forged_destination_expected,
        forged_route_expected,
        forged_canary_route,
        forged_canary_destination,
    }
    destination["expected_destination_binding_hash"] = forged_destination_expected
    destination["expected_destination_binding_hash_matches"] = True
    destination["recomputed"] = True
    route["expected_route_allowlist_hash"] = forged_route_expected
    route["expected_route_allowlist_hash_matches"] = True
    canary["route_allowlist_hash"] = forged_canary_route
    canary["destination_binding_hash"] = forged_canary_destination

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    expected_blockers = (
        (
            "destination_binding.expected_destination_binding_hash must match "
            "destination_binding_hash"
        ),
        "route_allowlist.expected_route_allowlist_hash must match route_allowlist_hash",
        (
            "route_allowlist.route_canary.route_allowlist_hash must match lane "
            "route_allowlist_hash"
        ),
        (
            "route_allowlist.route_canary.destination_binding_hash must match lane "
            "destination_binding_hash"
        ),
    )
    for expected in expected_blockers:
        assert f"all-lanes summary lanes[{bsc_index}].{expected}" in blockers
    for forged_value in forged_values:
        assert forged_value not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_not_ready_hash_binding_drift(capsys):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    bsc_index, bsc_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_BSC
    )
    assert bsc_lane["production_ready"] is True
    bsc_lane["production_ready"] = False
    bsc_lane["blockers"] = ["operator pending diagnostic BSC lane certification"]

    def replacement_hash(current: str, seed: int) -> str:
        candidate = hex32(seed)
        return hex32(seed + 1) if candidate == current else candidate

    destination = bsc_lane["destination_binding"]
    route = bsc_lane["route_allowlist"]
    canary = route["route_canary"]
    forged_destination_expected = replacement_hash(
        destination["destination_binding_hash"],
        0xB0,
    )
    forged_route_expected = replacement_hash(route["route_allowlist_hash"], 0xB2)
    forged_canary_route = replacement_hash(route["route_allowlist_hash"], 0xB4)
    forged_canary_destination = replacement_hash(
        destination["destination_binding_hash"],
        0xB6,
    )
    forged_values = {
        forged_destination_expected,
        forged_route_expected,
        forged_canary_route,
        forged_canary_destination,
    }
    destination["expected_destination_binding_hash"] = forged_destination_expected
    destination["expected_destination_binding_hash_matches"] = True
    destination["recomputed"] = True
    route["expected_route_allowlist_hash"] = forged_route_expected
    route["expected_route_allowlist_hash_matches"] = True
    canary["route_allowlist_hash"] = forged_canary_route
    canary["destination_binding_hash"] = forged_canary_destination

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    expected_blockers = (
        (
            "destination_binding.expected_destination_binding_hash must match "
            "destination_binding_hash"
        ),
        "route_allowlist.expected_route_allowlist_hash must match route_allowlist_hash",
        (
            "route_allowlist.route_canary.route_allowlist_hash must match lane "
            "route_allowlist_hash"
        ),
        (
            "route_allowlist.route_canary.destination_binding_hash must match lane "
            "destination_binding_hash"
        ),
    )
    for expected in expected_blockers:
        assert f"all-lanes summary lanes[{bsc_index}].{expected}" in blockers
    assert "operator pending diagnostic BSC lane certification" not in captured.out
    for forged_value in forged_values:
        assert forged_value not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_not_ready_route_allowlist_recompute_drift(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    bsc_index, bsc_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_BSC
    )
    bsc_lane["production_ready"] = False
    bsc_lane["blockers"] = ["operator pending route recompute certification"]

    def replacement_hash(current: str, seed: int) -> str:
        candidate = hex32(seed)
        return hex32(seed + 1) if candidate == current else candidate

    route = bsc_lane["route_allowlist"]
    forged_route_hash = replacement_hash(route["route_allowlist_hash"], 0xC0)
    route["route_allowlist_hash"] = forged_route_hash
    route["expected_route_allowlist_hash"] = forged_route_hash
    route["expected_route_allowlist_hash_matches"] = True
    route["route_canary"]["route_allowlist_hash"] = forged_route_hash

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    assert (
        f"all-lanes summary lanes[{bsc_index}].route_allowlist."
        "route_allowlist_hash must recompute from source material, source "
        "adapter deployment, and destination binding hashes"
    ) in blockers
    assert "operator pending route recompute certification" not in captured.out
    assert forged_route_hash not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_route_allowlist_recompute_helper_failures(
    monkeypatch,
    capsys,
):
    module = load_evidence_module()
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    bsc_index, bsc_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_BSC
    )
    bsc_lane["production_ready"] = False
    bsc_lane["blockers"] = ["operator pending route recompute certification"]
    expected_blocker = (
        f"all-lanes summary lanes[{bsc_index}].route_allowlist."
        "route_allowlist_hash must recompute from source material, source "
        "adapter deployment, and destination binding hashes"
    )

    for exception_type in (SystemExit, RuntimeError, TypeError, ValueError):

        def fail_recompute(
            _profile,
            _source_hashes,
            _destination_binding,
            exception_type=exception_type,
        ):
            raise exception_type(
                f"secret-token copied route material {exception_type.__name__}"
            )

        with monkeypatch.context() as patch:
            patch.setattr(module, "load_evidence_bundle", lambda paths: {})
            patch.setattr(module, "validate_evidence_bundle", lambda records: summary)
            patch.setattr(module, "_expected_route_allowlist_hash", fail_recompute)

            assert module.main(["evidence.toml"]) == 1

        captured = capsys.readouterr()
        payload = json.loads(captured.out)
        blockers = "\n".join(payload["blockers"])
        assert payload["production_ready"] is False
        assert "lanes" not in payload
        assert "operator pending external verifier deployment" in blockers
        assert expected_blocker in blockers
        assert "operator pending route recompute certification" not in captured.out
        assert "secret-token" not in captured.out
        assert "secret-token" not in captured.err
        assert exception_type.__name__ not in captured.out
        assert exception_type.__name__ not in captured.err
        assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_active_copied_route_allowlist_recompute_drift_when_lane_not_ready(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    eth_index, eth_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    eth_lane["production_ready"] = False
    eth_lane["blockers"] = ["operator pending active route recompute certification"]

    taken_hashes: set[str] = set()

    def collect_hashes(value):
        if isinstance(value, dict):
            for child in value.values():
                collect_hashes(child)
        elif isinstance(value, list):
            for child in value:
                collect_hashes(child)
        elif isinstance(value, str) and value.startswith("0x") and len(value) == 66:
            taken_hashes.add(value)

    collect_hashes(summary)
    for seed in range(0xC7, 0x1C7):
        forged_route_hash = hex32(seed)
        if forged_route_hash not in taken_hashes:
            break
    else:
        raise AssertionError("could not forge distinct active route hash")

    route = eth_lane["route_allowlist"]
    route["route_allowlist_hash"] = forged_route_hash
    route["expected_route_allowlist_hash"] = forged_route_hash
    route["expected_route_allowlist_hash_matches"] = True
    route["route_canary"]["route_allowlist_hash"] = forged_route_hash

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    assert (
        f"all-lanes summary lanes[{eth_index}].route_allowlist."
        "route_allowlist_hash must recompute from source material, source "
        "adapter deployment, and destination binding hashes"
    ) in blockers
    assert "operator pending active route recompute certification" not in captured.out
    assert forged_route_hash not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_active_copied_route_allowlist_missing_fields_when_lane_not_ready(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    eth_index, eth_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    assert eth_lane["production_ready"] is True

    eth_lane["production_ready"] = False
    eth_lane["blockers"] = ["operator pending active route-allowlist field audit"]
    route = eth_lane["route_allowlist"]
    missing_fields = tuple(module.PUBLIC_LANE_ROUTE_ALLOWLIST_FIELDS)
    for field in missing_fields:
        route.pop(field, None)

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    for field in missing_fields:
        assert (
            f"all-lanes summary lanes[{eth_index}]."
            f"route_allowlist missing field {field}"
        ) in blockers
    assert (
        "operator pending active route-allowlist field audit"
        not in captured.out
    )
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_not_ready_destination_binding_recompute_drift(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    bsc_index, bsc_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_BSC
    )
    bsc_lane["production_ready"] = False
    bsc_lane["blockers"] = ["operator pending destination recompute certification"]

    def replacement_hash(current: str, seed: int) -> str:
        candidate = hex32(seed)
        return hex32(seed + 1) if candidate == current else candidate

    destination = bsc_lane["destination_binding"]
    forged_destination_hash = replacement_hash(
        destination["destination_binding_hash"],
        0xD0,
    )
    destination["destination_binding_hash"] = forged_destination_hash
    destination["expected_destination_binding_hash"] = forged_destination_hash
    destination["expected_destination_binding_hash_matches"] = True
    destination["recomputed"] = True

    route = bsc_lane["route_allowlist"]
    profile = module.LANE_PROFILES[module.SCCP_DOMAIN_BSC]
    source_record_hashes = bsc_lane["source_record_hashes"]
    forged_route_hash = "0x" + module.route_allowlist_hash_for_lane_evidence(
        profile,
        raw_hex(source_record_hashes["source_verifier_material_hash"]),
        raw_hex(source_record_hashes["source_adapter_engine_deployment_hash"]),
        raw_hex(forged_destination_hash),
    ).hex()
    route["route_allowlist_hash"] = forged_route_hash
    route["expected_route_allowlist_hash"] = forged_route_hash
    route["expected_route_allowlist_hash_matches"] = True
    route["route_canary"]["route_allowlist_hash"] = forged_route_hash
    route["route_canary"]["destination_binding_hash"] = forged_destination_hash

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    # Source-inventory marker: destination_binding_hash must recompute from destination_binding_key
    assert (
        f"all-lanes summary lanes[{bsc_index}].destination_binding."
        "destination_binding_hash must recompute from destination_binding_key"
    ) in blockers
    assert "operator pending destination recompute certification" not in captured.out
    assert forged_destination_hash not in captured.out
    assert forged_route_hash not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_active_copied_destination_binding_recompute_drift_when_lane_not_ready(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    eth_index, eth_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    eth_lane["production_ready"] = False
    eth_lane["blockers"] = [
        "operator pending active destination recompute certification"
    ]

    taken_hashes: set[str] = set()

    def collect_hashes(value):
        if isinstance(value, dict):
            for child in value.values():
                collect_hashes(child)
        elif isinstance(value, list):
            for child in value:
                collect_hashes(child)
        elif isinstance(value, str) and value.startswith("0x") and len(value) == 66:
            taken_hashes.add(value)

    collect_hashes(summary)
    destination = eth_lane["destination_binding"]
    route = eth_lane["route_allowlist"]
    source_record_hashes = eth_lane["source_record_hashes"]
    profile = module.LANE_PROFILES[module.SCCP_DOMAIN_ETH]
    for seed in range(0xD7, 0x1D7):
        forged_destination_hash = hex32(seed)
        if forged_destination_hash in taken_hashes:
            continue
        forged_route_hash = "0x" + module.route_allowlist_hash_for_lane_evidence(
            profile,
            raw_hex(source_record_hashes["source_verifier_material_hash"]),
            raw_hex(source_record_hashes["source_adapter_engine_deployment_hash"]),
            raw_hex(forged_destination_hash),
        ).hex()
        if forged_route_hash not in taken_hashes | {forged_destination_hash}:
            break
    else:
        raise AssertionError("could not forge distinct active destination hash")

    destination["destination_binding_hash"] = forged_destination_hash
    destination["expected_destination_binding_hash"] = forged_destination_hash
    destination["expected_destination_binding_hash_matches"] = True
    destination["recomputed"] = True
    route["route_allowlist_hash"] = forged_route_hash
    route["expected_route_allowlist_hash"] = forged_route_hash
    route["expected_route_allowlist_hash_matches"] = True
    route["route_canary"]["route_allowlist_hash"] = forged_route_hash
    route["route_canary"]["destination_binding_hash"] = forged_destination_hash

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    assert (
        f"all-lanes summary lanes[{eth_index}].destination_binding."
        "destination_binding_hash must recompute from destination_binding_key"
    ) in blockers
    assert (
        "operator pending active destination recompute certification"
        not in captured.out
    )
    assert forged_destination_hash not in captured.out
    assert forged_route_hash not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_not_ready_route_canary_hash_role_reuse(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    lanes = {
        lane["domain"]: (index, lane)
        for index, lane in enumerate(summary["lanes"])
    }
    bsc_index, bsc_lane = lanes[module.SCCP_DOMAIN_BSC]
    sol_index, sol_lane = lanes[module.SCCP_DOMAIN_SOL]
    ton_index, ton_lane = lanes[module.SCCP_DOMAIN_TON]
    for lane in (bsc_lane, sol_lane, ton_lane):
        lane["production_ready"] = False
        lane["blockers"] = ["operator pending route-canary role audit"]

    bsc_canary = bsc_lane["route_allowlist"]["route_canary"]
    bsc_canary["evidence_hash"] = bsc_lane["route_allowlist"][
        "route_allowlist_hash"
    ]
    bsc_canary["message_id"] = bsc_lane["source_record_hashes"][
        "source_verifier_material_hash"
    ]
    sol_canary = sol_lane["route_allowlist"]["route_canary"]
    sol_canary["evidence_hash"] = sol_lane["source_adapter_gate"]["gate_hash"]
    ton_canary = ton_lane["route_allowlist"]["route_canary"]
    ton_canary["ton_account_state_hash"] = ton_lane["destination_binding"][
        "destination_binding_hash"
    ]
    forged_values = {
        bsc_canary["evidence_hash"],
        bsc_canary["message_id"],
        sol_canary["evidence_hash"],
        ton_canary["ton_account_state_hash"],
    }

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    # Source-inventory marker: route_allowlist.route_canary hash role evidence_hash must not reuse route_allowlist_hash
    # Source-inventory marker: route_allowlist.route_canary hash role message_id must not reuse source_verifier_material_hash
    # Source-inventory marker: route_allowlist.route_canary hash role evidence_hash must not reuse source_adapter_gate_hash
    # Source-inventory marker: route_allowlist.route_canary hash role ton_account_state_hash must not reuse destination_binding_hash
    expected_blockers = (
        (
            bsc_index,
            "route_allowlist.route_canary hash role evidence_hash must not "
            "reuse route_allowlist_hash",
        ),
        (
            bsc_index,
            "route_allowlist.route_canary hash role message_id must not reuse "
            "source_verifier_material_hash",
        ),
        (
            sol_index,
            "route_allowlist.route_canary hash role evidence_hash must not "
            "reuse source_adapter_gate_hash",
        ),
        (
            ton_index,
            "route_allowlist.route_canary hash role ton_account_state_hash must "
            "not reuse destination_binding_hash",
        ),
    )
    for lane_index, expected in expected_blockers:
        assert f"all-lanes summary lanes[{lane_index}].{expected}" in blockers
    assert "operator pending route-canary role audit" not in captured.out
    for forged_value in forged_values:
        assert forged_value not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_active_copied_route_canary_hash_role_reuse_when_lane_not_ready(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    eth_index, eth_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    eth_lane["production_ready"] = False
    eth_lane["blockers"] = ["operator pending active route-canary role audit"]

    eth_canary = eth_lane["route_allowlist"]["route_canary"]
    eth_canary["evidence_hash"] = eth_lane["route_allowlist"][
        "route_allowlist_hash"
    ]
    eth_canary["message_id"] = eth_lane["source_record_hashes"][
        "source_verifier_material_hash"
    ]
    forged_values = {
        eth_canary["evidence_hash"],
        eth_canary["message_id"],
    }

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    # Source-inventory marker: active route_canary hash role evidence_hash must not reuse route_allowlist_hash
    # Source-inventory marker: active route_canary hash role message_id must not reuse source_verifier_material_hash
    expected_blockers = (
        "route_allowlist.route_canary hash role evidence_hash must not "
        "reuse route_allowlist_hash",
        "route_allowlist.route_canary hash role message_id must not reuse "
        "source_verifier_material_hash",
    )
    for expected in expected_blockers:
        assert f"all-lanes summary lanes[{eth_index}].{expected}" in blockers
    assert "operator pending active route-canary role audit" not in captured.out
    for forged_value in forged_values:
        assert forged_value not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_not_ready_claimed_ready_evm_metadata_drift(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    lanes = {
        lane["domain"]: (index, lane)
        for index, lane in enumerate(summary["lanes"])
    }
    eth_index, eth_lane = lanes[module.SCCP_DOMAIN_ETH]
    bsc_index, bsc_lane = lanes[module.SCCP_DOMAIN_BSC]
    assert eth_lane["production_ready"] is True
    assert bsc_lane["production_ready"] is True

    evm_metadata = eth_lane["evm_live_metadata"]
    evm_metadata.pop("required", None)
    evm_metadata["ready"] = False
    evm_metadata.pop("source_rpc_chain_id", None)
    evm_metadata["destination_rpc_chain_id"] = "2"
    evm_metadata.pop("source_block_tag", None)
    evm_metadata["destination_block_tag"] = "safe-block-tag"
    eth_lane["destination_binding"].pop("destination_binding_key", None)
    bsc_lane["evm_live_metadata"].pop("ready", None)

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    expected_blockers = (
        "evm_live_metadata.required must be a boolean",
        "evm_live_metadata.ready must be true",
        (
            "evm_live_metadata.source_rpc_chain_id must be a canonical "
            "positive decimal string"
        ),
        "evm_live_metadata.destination_rpc_chain_id must be 1",
        "evm_live_metadata.source_block_tag must be a non-empty canonical string",
        "evm_live_metadata.destination_block_tag must be finalized",
        "destination_binding.destination_binding_key must be a non-empty canonical string",
    )
    for expected in expected_blockers:
        assert f"all-lanes summary lanes[{eth_index}].{expected}" in blockers
    bsc_ready_blocker = "evm_live_metadata.ready must be a boolean"
    assert f"all-lanes summary lanes[{bsc_index}].{bsc_ready_blocker}" in blockers
    assert "safe-block-tag" not in captured.out
    assert '"2"' not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_active_copied_evm_metadata_drift_when_lane_not_ready(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    eth_index, eth_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    eth_lane["production_ready"] = False
    eth_lane["blockers"] = ["operator pending active EVM metadata audit"]

    evm_metadata = eth_lane["evm_live_metadata"]
    evm_metadata["required"] = False
    evm_metadata["ready"] = False
    evm_metadata["source_rpc_chain_id"] = "0x1"
    evm_metadata["destination_rpc_chain_id"] = "2"
    evm_metadata["source_block_tag"] = "latest"
    evm_metadata["destination_block_tag"] = "safe-block-tag"
    forged_values = {
        "0x1",
        '"2"',
        "latest",
        "safe-block-tag",
    }

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    expected_blockers = (
        "evm_live_metadata.required must be true",
        "evm_live_metadata.ready must be true",
        (
            "evm_live_metadata.source_rpc_chain_id must be a canonical "
            "positive decimal string"
        ),
        "evm_live_metadata.destination_rpc_chain_id must be 1",
        "evm_live_metadata.source_block_tag must be finalized",
        "evm_live_metadata.destination_block_tag must be finalized",
    )
    for expected in expected_blockers:
        assert f"all-lanes summary lanes[{eth_index}].{expected}" in blockers
    assert "operator pending active EVM metadata audit" not in captured.out
    for forged_value in forged_values:
        assert forged_value not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_active_copied_evm_metadata_missing_fields_when_lane_not_ready(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    eth_index, eth_lane = next(
        (index, lane)
        for index, lane in enumerate(summary["lanes"])
        if lane["domain"] == module.SCCP_DOMAIN_ETH
    )
    assert eth_lane["production_ready"] is True

    eth_lane["production_ready"] = False
    eth_lane["blockers"] = ["operator pending active EVM metadata field audit"]
    evm_metadata = eth_lane["evm_live_metadata"]
    missing_fields = tuple(module.PUBLIC_LANE_EVM_LIVE_METADATA_FIELDS)
    for field in missing_fields:
        evm_metadata.pop(field, None)

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    for field in missing_fields:
        assert (
            f"all-lanes summary lanes[{eth_index}]."
            f"evm_live_metadata missing field {field}"
        ) in blockers
    assert "operator pending active EVM metadata field audit" not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_not_ready_non_evm_metadata_drift(capsys):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    lanes = {
        lane["domain"]: (index, lane)
        for index, lane in enumerate(summary["lanes"])
    }
    sol_index, sol_lane = lanes[module.SCCP_DOMAIN_SOL]
    ton_index, ton_lane = lanes[module.SCCP_DOMAIN_TON]
    tron_index, tron_lane = lanes[module.SCCP_DOMAIN_TRON]
    assert sol_lane["production_ready"] is True
    assert ton_lane["production_ready"] is True
    assert tron_lane["production_ready"] is True

    sol_metadata = sol_lane["evm_live_metadata"]
    sol_metadata["required"] = True
    sol_metadata["ready"] = False
    sol_metadata["source_rpc_chain_id"] = "999999"
    sol_metadata["source_block_tag"] = "safe-solana-block-tag"
    ton_metadata = ton_lane["evm_live_metadata"]
    ton_metadata.pop("required", None)
    ton_metadata.pop("ready", None)
    tron_metadata = tron_lane["evm_live_metadata"]
    tron_metadata["destination_rpc_chain_id"] = "888888"
    tron_metadata["destination_block_tag"] = "safe-tron-block-tag"

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    expected_blockers = (
        (sol_index, "evm_live_metadata.required must be false for non-EVM lanes"),
        (sol_index, "evm_live_metadata.ready must be true for non-EVM lanes"),
        (
            sol_index,
            "evm_live_metadata.source_rpc_chain_id must be empty for non-EVM lanes",
        ),
        (
            sol_index,
            "evm_live_metadata.source_block_tag must be empty for non-EVM lanes",
        ),
        (ton_index, "evm_live_metadata.required must be false for non-EVM lanes"),
        (ton_index, "evm_live_metadata.ready must be true for non-EVM lanes"),
        (
            tron_index,
            "evm_live_metadata.destination_rpc_chain_id must be empty for non-EVM lanes",
        ),
        (
            tron_index,
            "evm_live_metadata.destination_block_tag must be empty for non-EVM lanes",
        ),
    )
    for lane_index, expected in expected_blockers:
        assert f"all-lanes summary lanes[{lane_index}].{expected}" in blockers
    assert "safe-solana-block-tag" not in captured.out
    assert "safe-tron-block-tag" not in captured.out
    assert "999999" not in captured.out
    assert "888888" not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_not_ready_route_canary_domain_field_drift(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    lanes = {
        lane["domain"]: (index, lane)
        for index, lane in enumerate(summary["lanes"])
    }
    bsc_index, bsc_lane = lanes[module.SCCP_DOMAIN_BSC]
    sol_index, sol_lane = lanes[module.SCCP_DOMAIN_SOL]
    ton_index, ton_lane = lanes[module.SCCP_DOMAIN_TON]
    tron_index, tron_lane = lanes[module.SCCP_DOMAIN_TRON]
    for lane in (bsc_lane, sol_lane, ton_lane, tron_lane):
        assert lane["production_ready"] is True

    bsc_lane["route_allowlist"]["route_canary"]["solana_programdata_slot"] = (
        "777777"
    )
    sol_lane["route_allowlist"]["route_canary"]["transaction_hash"] = hex32(0xC1)
    ton_lane["route_allowlist"]["route_canary"]["transaction_id"] = hex32(0xC2)
    tron_lane["route_allowlist"]["route_canary"]["ton_last_transaction_lt"] = (
        "888888"
    )
    forged_values = {
        "777777",
        "888888",
        hex32(0xC1),
        hex32(0xC2),
    }

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    expected_blockers = (
        (
            bsc_index,
            "route_allowlist.route_canary unexpected field solana_programdata_slot",
        ),
        (sol_index, "route_allowlist.route_canary unexpected field transaction_hash"),
        (ton_index, "route_allowlist.route_canary unexpected field transaction_id"),
        (
            tron_index,
            "route_allowlist.route_canary unexpected field ton_last_transaction_lt",
        ),
    )
    for lane_index, expected in expected_blockers:
        assert f"all-lanes summary lanes[{lane_index}].{expected}" in blockers
    for forged_value in forged_values:
        assert forged_value not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_not_ready_route_canary_missing_fields(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    lanes = {
        lane["domain"]: (index, lane)
        for index, lane in enumerate(summary["lanes"])
    }
    eth_index, eth_lane = lanes[module.SCCP_DOMAIN_ETH]
    bsc_index, bsc_lane = lanes[module.SCCP_DOMAIN_BSC]
    sol_index, sol_lane = lanes[module.SCCP_DOMAIN_SOL]
    ton_index, ton_lane = lanes[module.SCCP_DOMAIN_TON]
    tron_index, tron_lane = lanes[module.SCCP_DOMAIN_TRON]
    for lane in (eth_lane, bsc_lane, sol_lane, ton_lane, tron_lane):
        lane["production_ready"] = False
        lane["blockers"] = ["operator pending lane certification"]

    eth_lane["route_allowlist"]["route_canary"].pop("transaction_hash", None)
    eth_lane["route_allowlist"]["route_canary"].pop("message_proof_used", None)
    bsc_lane["route_allowlist"]["route_canary"].pop(
        "receipt_block_finalized",
        None,
    )
    sol_lane["route_allowlist"]["route_canary"].pop(
        "solana_programdata_address",
        None,
    )
    ton_lane["route_allowlist"]["route_canary"].pop(
        "ton_last_transaction_hash",
        None,
    )
    tron_lane["route_allowlist"]["route_canary"].pop("signature_sha256", None)
    tron_lane["route_allowlist"]["route_canary"].pop("message_proof_used", None)
    tron_lane["route_allowlist"]["route_canary"].pop(
        "raw_data_owner_matches_transaction",
        None,
    )
    tron_lane["route_allowlist"]["route_canary"].pop(
        "signature_recovers_to_owner",
        None,
    )

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    # Source-inventory marker: missing field: message_proof_used
    # Source-inventory marker: missing field: receipt_block_finalized
    # Source-inventory marker: missing field: raw_data_owner_matches_transaction
    # Source-inventory marker: missing field: signature_recovers_to_owner
    expected_blockers = (
        (eth_index, "route_allowlist.route_canary missing field transaction_hash"),
        (eth_index, "route_allowlist.route_canary missing field message_proof_used"),
        (
            bsc_index,
            "route_allowlist.route_canary missing field receipt_block_finalized",
        ),
        (
            sol_index,
            "route_allowlist.route_canary missing field solana_programdata_address",
        ),
        (
            ton_index,
            "route_allowlist.route_canary missing field ton_last_transaction_hash",
        ),
        (tron_index, "route_allowlist.route_canary missing field signature_sha256"),
        (tron_index, "route_allowlist.route_canary missing field message_proof_used"),
        (
            tron_index,
            "route_allowlist.route_canary missing field raw_data_owner_matches_transaction",
        ),
        (
            tron_index,
            "route_allowlist.route_canary missing field signature_recovers_to_owner",
        ),
    )
    for lane_index, expected in expected_blockers:
        assert f"all-lanes summary lanes[{lane_index}].{expected}" in blockers
    assert "operator pending lane certification" not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_not_ready_nested_missing_fields(capsys):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    lanes = {
        lane["domain"]: (index, lane)
        for index, lane in enumerate(summary["lanes"])
    }
    eth_index, eth_lane = lanes[module.SCCP_DOMAIN_ETH]
    bsc_index, bsc_lane = lanes[module.SCCP_DOMAIN_BSC]
    sol_index, sol_lane = lanes[module.SCCP_DOMAIN_SOL]
    ton_index, ton_lane = lanes[module.SCCP_DOMAIN_TON]
    tron_index, tron_lane = lanes[module.SCCP_DOMAIN_TRON]
    for lane in (eth_lane, bsc_lane, sol_lane, ton_lane, tron_lane):
        lane["production_ready"] = False
        lane["blockers"] = ["operator pending lane certification"]
    bsc_gate_field, _bsc_audit_fields = module._source_adapter_gate_requirements(
        module.SCCP_DOMAIN_BSC
    )

    eth_lane["source_record_hashes"].pop("source_verifier_material_hash", None)
    bsc_lane["source_adapter_gate"].pop("gate_hash", None)
    bsc_lane["source_adapter_gate"]["audit_hashes"].pop(bsc_gate_field, None)
    sol_lane["evm_live_metadata"].pop("source_block_tag", None)
    ton_lane["destination_binding"].pop("expected_destination_binding_hash", None)
    tron_lane["route_allowlist"].pop("route_canary", None)

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    expected_blockers = (
        (
            eth_index,
            "source_record_hashes missing field source_verifier_material_hash",
        ),
        (bsc_index, "source_adapter_gate missing field gate_hash"),
        (
            bsc_index,
            f"source_adapter_gate.audit_hashes missing field {bsc_gate_field}",
        ),
        (sol_index, "evm_live_metadata missing field source_block_tag"),
        (
            ton_index,
            "destination_binding missing field expected_destination_binding_hash",
        ),
        (tron_index, "route_allowlist missing field route_canary"),
    )
    for lane_index, expected in expected_blockers:
        assert f"all-lanes summary lanes[{lane_index}].{expected}" in blockers
    assert "operator pending lane certification" not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_not_ready_false_record_structural_drift(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    lanes = {
        lane["domain"]: (index, lane)
        for index, lane in enumerate(summary["lanes"])
    }
    bsc_index, bsc_lane = lanes[module.SCCP_DOMAIN_BSC]
    sol_index, sol_lane = lanes[module.SCCP_DOMAIN_SOL]
    ton_index, ton_lane = lanes[module.SCCP_DOMAIN_TON]
    tron_index, tron_lane = lanes[module.SCCP_DOMAIN_TRON]
    for lane in (bsc_lane, sol_lane, ton_lane, tron_lane):
        lane["production_ready"] = False
        lane["blockers"] = ["operator pending lane certification"]
        lane["records"] = {
            field: False for field in module.PUBLIC_LANE_RECORD_FIELDS
        }

    bsc_gate_field, _bsc_audit_fields = module._source_adapter_gate_requirements(
        module.SCCP_DOMAIN_BSC
    )
    bsc_lane["source_record_hashes"].pop("source_verifier_material_hash", None)
    bsc_lane["source_adapter_gate"].pop("gate_hash", None)
    bsc_lane["source_adapter_gate"]["audit_hashes"].pop(bsc_gate_field, None)
    sol_lane["destination_binding"].pop("expected_destination_binding_hash", None)
    ton_lane["route_allowlist"].pop("route_canary", None)
    tron_lane["route_allowlist"]["route_canary"]["destination_binding_hash"] = ""

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    expected_blockers = (
        (
            bsc_index,
            "source_record_hashes missing field source_verifier_material_hash",
        ),
        (bsc_index, "source_adapter_gate missing field gate_hash"),
        (
            bsc_index,
            f"source_adapter_gate.audit_hashes missing field {bsc_gate_field}",
        ),
        (
            sol_index,
            "destination_binding missing field expected_destination_binding_hash",
        ),
        (ton_index, "route_allowlist missing field route_canary"),
        (
            tron_index,
            "route_allowlist.route_canary.destination_binding_hash must be a canonical non-zero bytes32",
        ),
    )
    for lane_index, expected in expected_blockers:
        assert f"all-lanes summary lanes[{lane_index}].{expected}" in blockers
    assert "operator pending lane certification" not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_not_ready_required_hash_empties(capsys):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    lanes = {
        lane["domain"]: (index, lane)
        for index, lane in enumerate(summary["lanes"])
    }
    eth_index, eth_lane = lanes[module.SCCP_DOMAIN_ETH]
    bsc_index, bsc_lane = lanes[module.SCCP_DOMAIN_BSC]
    sol_index, sol_lane = lanes[module.SCCP_DOMAIN_SOL]
    ton_index, ton_lane = lanes[module.SCCP_DOMAIN_TON]
    tron_index, tron_lane = lanes[module.SCCP_DOMAIN_TRON]
    for lane in (eth_lane, bsc_lane, sol_lane, ton_lane, tron_lane):
        lane["production_ready"] = False
        lane["blockers"] = ["operator pending lane certification"]

    eth_lane["source_record_hashes"]["source_verifier_material_hash"] = ""
    bsc_lane["destination_binding"]["expected_destination_binding_hash"] = ""
    sol_lane["route_allowlist"]["route_allowlist_hash"] = ""
    ton_lane["route_allowlist"]["route_canary"]["evidence_hash"] = ""
    tron_lane["route_allowlist"]["route_canary"]["destination_binding_hash"] = ""

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    expected_blockers = (
        (
            eth_index,
            "source_record_hashes.source_verifier_material_hash must be a canonical non-zero bytes32",
        ),
        (
            bsc_index,
            "destination_binding.expected_destination_binding_hash must be a canonical non-zero bytes32",
        ),
        (
            sol_index,
            "route_allowlist.route_allowlist_hash must be a canonical non-zero bytes32",
        ),
        (
            ton_index,
            "route_allowlist.route_canary.evidence_hash must be a canonical non-zero bytes32",
        ),
        (
            tron_index,
            "route_allowlist.route_canary.destination_binding_hash must be a canonical non-zero bytes32",
        ),
    )
    for lane_index, expected in expected_blockers:
        assert f"all-lanes summary lanes[{lane_index}].{expected}" in blockers
    assert "operator pending lane certification" not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_not_ready_evm_required_field_drift(capsys):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending external verifier deployment"]
    lanes = {
        lane["domain"]: (index, lane)
        for index, lane in enumerate(summary["lanes"])
    }
    eth_index, eth_lane = lanes[module.SCCP_DOMAIN_ETH]
    bsc_index, bsc_lane = lanes[module.SCCP_DOMAIN_BSC]
    tron_index, tron_lane = lanes[module.SCCP_DOMAIN_TRON]
    for lane in (eth_lane, bsc_lane, tron_lane):
        lane["production_ready"] = False
        lane["blockers"] = ["operator pending lane certification"]

    eth_lane["evm_live_metadata"]["source_rpc_chain_id"] = ""
    eth_lane["evm_live_metadata"]["source_block_tag"] = ""
    bsc_lane["evm_live_metadata"]["destination_rpc_chain_id"] = ""
    bsc_lane["evm_live_metadata"]["destination_block_tag"] = ""
    bsc_lane["destination_binding"].pop("destination_network_id", None)
    bsc_lane["destination_binding"].pop("destination_bridge_address", None)
    tron_lane["destination_binding"].pop("destination_network_id", None)
    tron_lane["destination_binding"]["destination_bridge_address"] = "0x" + "55" * 20
    forged_values = {"0x" + "55" * 20}

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    assert "operator pending external verifier deployment" in blockers
    expected_blockers = (
        (
            eth_index,
            "evm_live_metadata.source_rpc_chain_id must be a canonical positive decimal string",
        ),
        (
            eth_index,
            "evm_live_metadata.source_block_tag must be a non-empty canonical string",
        ),
        (
            bsc_index,
            "evm_live_metadata.destination_rpc_chain_id must be a canonical positive decimal string",
        ),
        (
            bsc_index,
            "evm_live_metadata.destination_block_tag must be a non-empty canonical string",
        ),
        (
            bsc_index,
            "destination_binding.destination_network_id is required for EVM-family lanes",
        ),
        (
            bsc_index,
            "destination_binding.destination_bridge_address is required for EVM-family lanes",
        ),
        (
            tron_index,
            "destination_binding.destination_network_id is required for TRON lanes",
        ),
        (
            tron_index,
            "destination_binding.destination_bridge_address is only valid for EVM-family lanes",
        ),
    )
    for lane_index, expected in expected_blockers:
        assert f"all-lanes summary lanes[{lane_index}].{expected}" in blockers
    assert "operator pending lane certification" not in captured.out
    for forged_value in forged_values:
        assert forged_value not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_copied_ready_route_canary_scalar_drift(capsys):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    lanes = {
        lane["domain"]: (index, lane)
        for index, lane in enumerate(summary["lanes"])
    }
    eth_index, eth_lane = lanes[module.SCCP_DOMAIN_ETH]
    tron_index, tron_lane = lanes[module.SCCP_DOMAIN_TRON]
    sol_index, sol_lane = lanes[module.SCCP_DOMAIN_SOL]
    ton_index, ton_lane = lanes[module.SCCP_DOMAIN_TON]
    eth_canary = eth_lane["route_allowlist"]["route_canary"]
    eth_canary["status"] = "forged-status"
    eth_canary["evidence_source"] = "forged-source"
    eth_canary["evidence_bound"] = False
    eth_canary["receipt_block_number"] = "123"
    eth_canary["log_index"] = -1
    eth_canary["target_domain"] = module.SCCP_DOMAIN_BSC
    eth_canary["proof_version"] = 2
    eth_canary["proof_source_domain"] = module.SCCP_DOMAIN_ETH
    eth_canary["message_proof_used"] = "true"
    eth_canary["receipt_block_finalized"] = False
    tron_canary = tron_lane["route_allowlist"]["route_canary"]
    tron_canary["block_number"] = 0
    tron_canary["block_timestamp"] = -1
    tron_canary["raw_data_owner_matches_transaction"] = "yes"
    tron_canary["signature_recovers_to_owner"] = False
    tron_canary["signature_recovered_address"] = "0x41" + "99" * 20
    sol_canary = sol_lane["route_allowlist"]["route_canary"]
    sol_canary["solana_programdata_address"] = " forged-address "
    sol_canary["solana_programdata_slot"] = 42
    ton_lane["route_allowlist"]["route_canary"]["ton_last_transaction_lt"] = 0

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "lanes" not in payload
    eth_scalar_blockers = (
        "route_allowlist.route_canary.status must be passed",
        "route_allowlist.route_canary.evidence_source must be "
        "evm_message_proof_accepted_transaction",
        "route_allowlist.route_canary.evidence_bound must be true",
        "route_allowlist.route_canary.receipt_block_number must be a positive integer",
        "route_allowlist.route_canary.log_index must be a u32 integer",
        "route_allowlist.route_canary.target_domain must match lane domain",
        "route_allowlist.route_canary.proof_version must be 1",
        "route_allowlist.route_canary.proof_source_domain must be SORA domain",
        "route_allowlist.route_canary.message_proof_used must be a boolean",
        "route_allowlist.route_canary.receipt_block_finalized must be true",
    )
    for expected in eth_scalar_blockers:
        assert f"all-lanes summary lanes[{eth_index}].{expected}" in blockers
    tron_scalar_blockers = (
        "route_allowlist.route_canary.block_number must be a positive integer",
        "route_allowlist.route_canary.block_timestamp must be a non-negative integer",
        "route_allowlist.route_canary.raw_data_owner_matches_transaction must be a boolean",
        "route_allowlist.route_canary.signature_recovers_to_owner must be true",
        "route_allowlist.route_canary.signature_recovered_address must match "
        "transaction_owner_address",
    )
    for expected in tron_scalar_blockers:
        assert f"all-lanes summary lanes[{tron_index}].{expected}" in blockers
    sol_scalar_blockers = (
        "route_allowlist.route_canary.solana_programdata_address must be a "
        "non-empty canonical string",
        "route_allowlist.route_canary.solana_programdata_slot must be a "
        "canonical positive decimal string",
    )
    for expected in sol_scalar_blockers:
        assert f"all-lanes summary lanes[{sol_index}].{expected}" in blockers
    ton_scalar_blockers = (
        "route_allowlist.route_canary.ton_last_transaction_lt must be a "
        "canonical positive decimal string",
    )
    for expected in ton_scalar_blockers:
        assert f"all-lanes summary lanes[{ton_index}].{expected}" in blockers
    assert "forged-status" not in captured.out
    assert "forged-source" not in captured.out
    assert "forged-address" not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_empty_copied_release_checklist_without_leaking(
    capsys,
):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    summary = copy.deepcopy(module.validate_evidence_bundle(complete_bundle(module)))
    summary["production_ready"] = False
    summary["blockers"] = ["operator pending copied checklist certification"]
    summary["release_checklist"] = {}

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: summary
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "release_checklist" not in payload
    assert "operator pending copied checklist certification" in blockers
    assert "all-lanes summary release_checklist ready must be a boolean" in blockers
    assert "all-lanes summary release_checklist items must be a list" in blockers
    assert "all-lanes summary release_checklist is invalid" in blockers
    assert "Traceback" not in captured.err


def test_all_lanes_cli_rejects_malformed_release_checklist_without_leaking(capsys):
    module = load_evidence_module()
    original_load = module.load_evidence_bundle
    original_validate = module.validate_evidence_bundle
    canonical_titles = module.RELEASE_CHECKLIST_TITLES

    module.load_evidence_bundle = lambda paths: {}
    module.validate_evidence_bundle = lambda records: {
        "production_ready": True,
        "blockers": [],
        "required_domains": [1, 2, 3, 4, 5],
        "supported_launch_domains": [1, 2, 3, 4, 5],
        "unsupported_launch_domains": [],
        "lanes": [],
        "release_checklist": {
            "ready": "true",
            "secret-token-checklist": "secret-token-value",
            7: "safe checklist int-key note",
            HostilePublicKey(): "secret-token hostile checklist note",
            "items": [
                {
                    "id": "all_required_lane_records",
                    "title": "secret-token-title",
                    "ready": True,
                    "blockers": [],
                    7: "safe checklist item int-key note",
                    HostilePublicKey(): "secret-token hostile checklist item note",
                },
                {
                    "id": "all_required_lane_records",
                    "title": canonical_titles["all_required_lane_records"],
                    "ready": True,
                    "blockers": [],
                },
                {
                    "id": "live_route_canary_evidence",
                    "title": canonical_titles["live_route_canary_evidence"],
                    "ready": False,
                    "blockers": ["operator hold"],
                },
                "secret-token-item",
            ],
        },
    }
    try:
        exit_code = module.main(["evidence.toml"])
    finally:
        module.load_evidence_bundle = original_load
        module.validate_evidence_bundle = original_validate

    assert exit_code == 1
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    blockers = "\n".join(payload["blockers"])
    assert payload["production_ready"] is False
    assert "release_checklist" not in payload
    assert "all-lanes summary release_checklist ready must be a boolean" in blockers
    assert (
        "all-lanes summary release_checklist unexpected field with sensitive name"
        in blockers
    )
    assert (
        "all-lanes summary release_checklist unexpected non-string field name"
        in blockers
    )
    assert (
        "all-lanes summary release_checklist items[0] unexpected non-string "
        "field name"
    ) in blockers
    assert (
        "all-lanes summary release_checklist items[0] title must match the "
        "canonical checklist title"
    ) in blockers
    assert (
        "all-lanes summary release_checklist item all_required_lane_records "
        "is duplicated"
    ) in blockers
    assert "all-lanes summary release_checklist items[2] ready must be true" in blockers
    assert (
        "all-lanes summary release_checklist items[2] blockers must be empty"
        in blockers
    )
    assert "all-lanes summary release_checklist items[3] must be an object" in blockers
    assert (
        "all-lanes summary release_checklist missing item no_unresolved_blockers"
        in blockers
    )
    assert "all-lanes summary release_checklist is invalid" in blockers
    assert "secret-token" not in captured.out
    assert "hostile" not in captured.out
    assert "safe checklist int-key note" not in captured.out
    assert "safe checklist item int-key note" not in captured.out
    assert "operator hold" not in captured.out
    assert "Traceback" not in captured.err


def test_all_lanes_release_checklist_pinpoints_canary_gaps():
    module = load_evidence_module()
    records = complete_bundle(module)
    sol_route = records["sccp_route_allowlists"][2]
    del sol_route["_comment_route_canary_status"]
    del sol_route["_comment_route_canary_evidence_hash"]
    del sol_route["_comment_route_canary_route_allowlist_hash"]
    del sol_route["_comment_route_canary_destination_binding_hash"]

    summary = module.validate_evidence_bundle(records)
    items = {item["id"]: item for item in summary["release_checklist"]["items"]}

    assert summary["release_checklist"]["ready"] is False
    assert items["all_required_lane_records"]["ready"] is True
    assert items["live_route_canary_evidence"]["ready"] is False
    canary_blockers = "\n".join(items["live_route_canary_evidence"]["blockers"])
    assert "domain 3 (sol): route canary status is not passed" in canary_blockers
    assert "domain 3 (sol): route canary evidence hash is missing" in canary_blockers
    assert "domain 3 (sol): live route canary evidence source is missing" in canary_blockers
    assert items["no_unresolved_blockers"]["ready"] is False


def test_all_lanes_release_checklist_rejects_malformed_source_gate_flags():
    module = load_evidence_module()
    lane = {
        "domain": module.SCCP_DOMAIN_SOL,
        "chain": "sol",
        "records": {
            "source_verifier_material": True,
            "source_adapter_deployment": True,
            "destination_rollout": True,
            "route_allowlist": True,
        },
        "source_adapter_gate": {
            "required": "true",
            "ready": "true",
            "blockers": "none",
        },
        "destination_binding": {
            "expected_destination_binding_hash_matches": True,
        },
        "route_allowlist": {
            "expected_route_allowlist_hash_matches": True,
            "route_canary": {
                "status": "passed",
                "evidence_hash": hex32(0x51),
                "evidence_source": "solana_live_programdata_snapshot",
                "evidence_bound": True,
            },
        },
        "blockers": [],
    }

    checklist = module._release_checklist([lane], [])
    items = {item["id"]: item for item in checklist["items"]}

    assert checklist["ready"] is False
    assert items["governed_deployment_evidence"]["ready"] is False
    assert (
        "domain 3 (sol): source adapter gate required flag must be boolean"
        in items["governed_deployment_evidence"]["blockers"]
    )

    lane["source_adapter_gate"]["required"] = True
    checklist = module._release_checklist([lane], [])
    items = {item["id"]: item for item in checklist["items"]}

    assert checklist["ready"] is False
    assert items["governed_deployment_evidence"]["ready"] is False
    assert (
        "domain 3 (sol): source adapter gate ready flag must be boolean"
        in items["governed_deployment_evidence"]["blockers"]
    )

    lane["source_adapter_gate"]["ready"] = False
    lane["source_adapter_gate"]["blockers"] = []
    checklist = module._release_checklist([lane], [])
    items = {item["id"]: item for item in checklist["items"]}

    assert checklist["ready"] is False
    assert items["governed_deployment_evidence"]["ready"] is False
    assert (
        "domain 3 (sol): source adapter gate is not ready"
        in items["governed_deployment_evidence"]["blockers"]
    )


def test_all_lanes_release_checklist_requires_source_gate_hash_and_audits():
    module = load_evidence_module()
    lane = {
        "domain": module.SCCP_DOMAIN_SOL,
        "chain": "sol",
        "records": {
            "source_verifier_material": True,
            "source_adapter_deployment": True,
            "destination_rollout": True,
            "route_allowlist": True,
        },
        "source_adapter_gate": {
            "required": True,
            "ready": True,
            "blockers": [],
        },
        "destination_binding": {
            "expected_destination_binding_hash_matches": True,
        },
        "route_allowlist": {
            "expected_route_allowlist_hash_matches": True,
            "route_canary": {
                "status": "passed",
                "evidence_hash": hex32(0x51),
                "evidence_source": "solana_live_programdata_snapshot",
                "evidence_bound": True,
            },
        },
        "blockers": [],
    }

    checklist = module._release_checklist([lane], [])
    items = {item["id"]: item for item in checklist["items"]}

    assert checklist["ready"] is False
    assert items["governed_deployment_evidence"]["ready"] is False
    blockers = "\n".join(items["governed_deployment_evidence"]["blockers"])
    assert (
        "domain 3 (sol): source adapter gate hash must be a canonical non-zero "
        "bytes32 when required"
    ) in blockers
    assert (
        "domain 3 (sol): source adapter gate audit hashes must not be empty "
        "when required"
    ) in blockers

    lane["source_adapter_gate"]["gate_hash"] = hex32(0x61)
    lane["source_adapter_gate"]["audit_hashes"] = {
        "operator_override": hex32(0x61),
        "solana_tower_replay_verifier_hash": hex32(0x62),
        "solana_full_accountsdb_lattice_verifier_hash": hex32(0x63),
        "solana_bank_fork_choice_verifier_hash": hex32(0x64),
    }

    checklist = module._release_checklist([lane], [])
    items = {item["id"]: item for item in checklist["items"]}
    blockers = "\n".join(items["governed_deployment_evidence"]["blockers"])

    assert checklist["ready"] is False
    assert (
        "domain 3 (sol): source adapter gate audit hashes contains unexpected "
        "field: operator_override"
    ) in blockers
    assert (
        "domain 3 (sol): source adapter gate audit hashes "
        "solana_full_light_client_gate_hash must be a canonical non-zero bytes32"
    ) in blockers
    assert (
        "domain 3 (sol): source adapter gate hash must match "
        "audit_hashes.solana_full_light_client_gate_hash"
    ) in blockers


def checklist_source_gate_lane(module, domain, seed):
    gate_field, audit_fields = module._source_adapter_gate_requirements(domain)
    profile = module.LANE_PROFILES[domain]
    gate_hash = hex32(seed)
    audit_hashes = {
        field: hex32(seed + index + 1)
        for index, field in enumerate(audit_fields)
    }
    audit_hashes[gate_field] = gate_hash
    return {
        "domain": domain,
        "chain": profile.chain,
        "records": {
            "source_verifier_material": True,
            "source_adapter_deployment": True,
            "destination_rollout": True,
            "route_allowlist": True,
        },
        "source_record_hashes": {
            "source_verifier_material_hash": hex32(seed + 20),
            "source_adapter_engine_deployment_hash": hex32(seed + 21),
        },
        "source_adapter_gate": {
            "required": True,
            "ready": True,
            "gate_hash": gate_hash,
            "audit_hashes": audit_hashes,
            "blockers": [],
        },
        "destination_binding": {
            "expected_destination_binding_hash_matches": True,
            "destination_binding_hash": hex32(seed + 22),
        },
        "route_allowlist": {
            "expected_route_allowlist_hash_matches": True,
            "route_allowlist_hash": hex32(seed + 23),
            "route_canary": {
                "status": "passed",
                "evidence_hash": hex32(seed + 24),
                "evidence_source": module.ROUTE_CANARY_EVIDENCE_SOURCE_BY_DOMAIN[
                    domain
                ],
                "evidence_bound": True,
            },
        },
        "blockers": [],
    }


def test_all_lanes_release_checklist_rejects_source_gate_template_replays():
    module = load_evidence_module()
    domains = (
        module.SCCP_DOMAIN_ETH,
        module.SCCP_DOMAIN_BSC,
        module.SCCP_DOMAIN_SOL,
        module.SCCP_DOMAIN_TON,
        module.SCCP_DOMAIN_TRON,
    )

    for index, domain in enumerate(domains):
        seed = 0x81 + index * 0x20
        template_hashes = module._source_material_template_hashes(
            module.LANE_PROFILES[domain]
        )
        gate_field, audit_fields = module._source_adapter_gate_requirements(domain)
        component_audit_fields = tuple(
            field for field in audit_fields if field != gate_field
        )
        for template_field, template_hash in template_hashes.items():
            template_value = "0x" + template_hash.hex()

            gate_lane = checklist_source_gate_lane(module, domain, seed)
            gate_lane["source_adapter_gate"]["gate_hash"] = template_value
            gate_lane["source_adapter_gate"]["audit_hashes"][gate_field] = (
                template_value
            )

            gate_checklist = module._release_checklist([gate_lane], [])
            gate_items = {item["id"]: item for item in gate_checklist["items"]}
            gate_blockers = gate_items["governed_deployment_evidence"]["blockers"]

            assert gate_checklist["ready"] is False, (domain, template_field)
            assert (
                f"domain {domain} ({module.LANE_PROFILES[domain].chain}): "
                "source adapter gate hash must be deployed evidence, "
                "not built-in template material"
            ) in gate_blockers
            assert (
                f"domain {domain} ({module.LANE_PROFILES[domain].chain}): "
                f"source adapter gate audit hashes {gate_field} must be "
                "deployed evidence, not built-in template material"
            ) in gate_blockers

            for audit_field in component_audit_fields:
                audit_lane = checklist_source_gate_lane(module, domain, seed)
                audit_lane["source_adapter_gate"]["audit_hashes"][audit_field] = (
                    template_value
                )

                audit_checklist = module._release_checklist([audit_lane], [])
                audit_items = {item["id"]: item for item in audit_checklist["items"]}
                audit_blockers = audit_items["governed_deployment_evidence"][
                    "blockers"
                ]

                assert audit_checklist["ready"] is False, (
                    domain,
                    audit_field,
                    template_field,
                )
                assert (
                    f"domain {domain} ({module.LANE_PROFILES[domain].chain}): "
                    f"source adapter gate audit hashes {audit_field} must be "
                    "deployed evidence, not built-in template material"
                ) in audit_blockers


def test_all_lanes_release_checklist_redacts_unsafe_source_gate_audit_fields():
    module = load_evidence_module()
    base_lane = {
        "domain": module.SCCP_DOMAIN_SOL,
        "chain": "sol",
        "records": {
            "source_verifier_material": True,
            "source_adapter_deployment": True,
            "destination_rollout": True,
            "route_allowlist": True,
        },
        "source_record_hashes": {
            "source_verifier_material_hash": hex32(0x62),
            "source_adapter_engine_deployment_hash": hex32(0x63),
        },
        "source_adapter_gate": {
            "required": True,
            "ready": True,
            "gate_hash": hex32(0x61),
            "audit_hashes": {
                "solana_tower_replay_verifier_hash": hex32(0x70),
                "solana_full_accountsdb_lattice_verifier_hash": hex32(0x71),
                "solana_bank_fork_choice_verifier_hash": hex32(0x72),
                "solana_full_light_client_gate_hash": hex32(0x61),
            },
            "blockers": [],
        },
        "destination_binding": {
            "expected_destination_binding_hash_matches": True,
            "destination_binding_hash": hex32(0x65),
        },
        "route_allowlist": {
            "expected_route_allowlist_hash_matches": True,
            "route_allowlist_hash": hex32(0x68),
            "route_canary": {
                "status": "passed",
                "evidence_hash": hex32(0x69),
                "evidence_source": "solana_live_programdata_snapshot",
                "evidence_bound": True,
            },
        },
        "blockers": [],
    }

    cases = (
        (
            "secret-token-audit-field",
            hex32(0x73),
            "domain 3 (sol): source adapter gate audit hashes contains "
            "unexpected field with sensitive name",
        ),
        (
            "route|operator-audit-field",
            hex32(0x74),
            "domain 3 (sol): source adapter gate audit hashes contains "
            "unexpected field with malformed name",
        ),
        (
            7,
            hex32(0x75),
            "domain 3 (sol): source adapter gate audit hashes contains "
            "non-string field name",
        ),
        (
            HostilePublicKey(),
            hex32(0x76),
            "domain 3 (sol): source adapter gate audit hashes contains "
            "non-string field name",
        ),
        (
            "secret-token-replayed-audit-field",
            hex32(0x69),
            "domain 3 (sol): source adapter gate audit hashes contains "
            "unexpected field with sensitive name",
        ),
    )
    for field, value, expected_blocker in cases:
        lane = copy.deepcopy(base_lane)
        lane["source_adapter_gate"]["audit_hashes"][field] = value

        checklist = module._release_checklist([lane], [])
        items = {item["id"]: item for item in checklist["items"]}
        blockers = "\n".join(items["governed_deployment_evidence"]["blockers"])

        assert checklist["ready"] is False
        assert expected_blocker in blockers
        assert "secret-token" not in blockers
        assert "hostile" not in blockers
        if isinstance(field, str):
            assert field not in blockers

    safe_lane = copy.deepcopy(base_lane)
    safe_lane["source_adapter_gate"]["audit_hashes"]["operator_override"] = hex32(0x73)

    checklist = module._release_checklist([safe_lane], [])
    items = {item["id"]: item for item in checklist["items"]}
    blockers = "\n".join(items["governed_deployment_evidence"]["blockers"])

    assert (
        "domain 3 (sol): source adapter gate audit hashes contains unexpected "
        "field: operator_override"
    ) in blockers


def test_all_lanes_release_checklist_rejects_source_gate_hash_role_replay():
    module = load_evidence_module()
    route_canary_hash = hex32(0x66)
    duplicate_audit_hash = hex32(0x67)
    lane = {
        "domain": module.SCCP_DOMAIN_SOL,
        "chain": "sol",
        "records": {
            "source_verifier_material": True,
            "source_adapter_deployment": True,
            "destination_rollout": True,
            "route_allowlist": True,
        },
        "source_record_hashes": {
            "source_verifier_material_hash": hex32(0x62),
            "source_adapter_engine_deployment_hash": hex32(0x63),
        },
        "source_adapter_gate": {
            "required": True,
            "ready": True,
            "gate_hash": route_canary_hash,
            "audit_hashes": {
                "solana_tower_replay_verifier_hash": duplicate_audit_hash,
                "solana_full_accountsdb_lattice_verifier_hash": duplicate_audit_hash,
                "solana_bank_fork_choice_verifier_hash": hex32(0x64),
                "solana_full_light_client_gate_hash": route_canary_hash,
                "secret-token-route-canary-audit": route_canary_hash,
                HostilePublicKey(): route_canary_hash,
            },
            "blockers": [],
        },
        "destination_binding": {
            "expected_destination_binding_hash_matches": True,
            "destination_binding_hash": hex32(0x65),
        },
        "route_allowlist": {
            "expected_route_allowlist_hash_matches": True,
            "route_allowlist_hash": hex32(0x68),
            "route_canary": {
                "status": "passed",
                "evidence_hash": route_canary_hash,
                "evidence_source": "solana_live_programdata_snapshot",
                "evidence_bound": True,
            },
        },
        "blockers": [],
    }

    checklist = module._release_checklist([lane], [])
    items = {item["id"]: item for item in checklist["items"]}
    blockers = "\n".join(items["governed_deployment_evidence"]["blockers"])

    assert checklist["ready"] is False
    assert items["governed_deployment_evidence"]["ready"] is False
    assert (
        "domain 3 (sol): source adapter gate hash role "
        "audit_hashes.solana_tower_replay_verifier_hash must not reuse "
        "audit_hashes.solana_full_accountsdb_lattice_verifier_hash"
    ) in blockers
    assert (
        "domain 3 (sol): source adapter gate hash role "
        "audit_hashes.solana_full_light_client_gate_hash must not reuse "
        "route_canary_evidence_hash"
    ) in blockers
    assert (
        "domain 3 (sol): source adapter gate audit hashes contains unexpected "
        "field with sensitive name"
    ) in blockers
    assert (
        "domain 3 (sol): source adapter gate audit hashes contains non-string "
        "field name"
    ) in blockers
    assert "secret-token-route-canary-audit" not in blockers
    assert "hostile" not in blockers
    assert "__str__" not in blockers


def test_all_lanes_release_checklist_rejects_evm_source_gate_policy_downgrade():
    module = load_evidence_module()
    lane = {
        "domain": module.SCCP_DOMAIN_ETH,
        "chain": "eth",
        "records": {
            "source_verifier_material": True,
            "source_adapter_deployment": True,
            "destination_rollout": True,
            "route_allowlist": True,
        },
        "source_adapter_gate": {
            "required": False,
            "ready": True,
            "gate_hash": hex32(0x71),
            "audit_hashes": {"operator_override": hex32(0x71)},
            "blockers": [],
        },
        "destination_binding": {
            "expected_destination_binding_hash_matches": True,
        },
        "route_allowlist": {
            "expected_route_allowlist_hash_matches": True,
            "route_canary": {
                "status": "passed",
                "evidence_hash": hex32(0x72),
                "evidence_source": "evm_message_proof_accepted_transaction",
                "evidence_bound": True,
            },
        },
        "blockers": [],
    }

    checklist = module._release_checklist([lane], [])
    items = {item["id"]: item for item in checklist["items"]}
    blockers = "\n".join(items["governed_deployment_evidence"]["blockers"])

    assert checklist["ready"] is False
    assert items["governed_deployment_evidence"]["ready"] is False
    assert (
        "domain 1 (eth): source adapter gate required flag must match lane policy"
    ) in blockers
    assert (
        "domain 1 (eth): source adapter gate hash must be empty when not "
        "required"
    ) in blockers
    assert (
        "domain 1 (eth): source adapter gate audit hashes must be empty when "
        "not required"
    ) in blockers


def test_all_lanes_release_checklist_rejects_malformed_source_gate_blockers():
    module = load_evidence_module()
    base_lane = {
        "domain": module.SCCP_DOMAIN_SOL,
        "chain": "sol",
        "records": {
            "source_verifier_material": True,
            "source_adapter_deployment": True,
            "destination_rollout": True,
            "route_allowlist": True,
        },
        "source_adapter_gate": {
            "required": True,
            "ready": False,
            "blockers": [],
        },
        "destination_binding": {
            "expected_destination_binding_hash_matches": True,
        },
        "route_allowlist": {
            "expected_route_allowlist_hash_matches": True,
            "route_canary": {
                "status": "passed",
                "evidence_hash": hex32(0x51),
                "evidence_source": "solana_live_programdata_snapshot",
                "evidence_bound": True,
            },
        },
        "blockers": [],
    }
    cases = (
        (
            "source_gate.checklist_scalar",
            "operator hold",
            "domain 3 (sol): source adapter gate blockers must be a list of "
            "non-empty canonical strings",
        ),
        (
            "source_gate.checklist_numeric",
            [123],
            "domain 3 (sol): source adapter gate blockers[0] must be a "
            "non-empty canonical string",
        ),
        (
            "source_gate.checklist_empty",
            [""],
            "domain 3 (sol): source adapter gate blockers[0] must be a "
            "non-empty canonical string",
        ),
        (
            "source_gate.checklist_padded",
            [" padded "],
            "domain 3 (sol): source adapter gate blockers[0] must be a "
            "non-empty canonical string",
        ),
        (
            "source_gate.checklist_valid",
            ["operator hold"],
            "domain 3 (sol): operator hold",
        ),
    )

    for case_id, blockers, expected in cases:
        lane = copy.deepcopy(base_lane)
        lane["source_adapter_gate"]["blockers"] = blockers

        checklist = module._release_checklist([lane], [])
        items = {item["id"]: item for item in checklist["items"]}

        assert checklist["ready"] is False, case_id
        assert items["governed_deployment_evidence"]["ready"] is False, case_id
        assert items["no_unresolved_blockers"]["ready"] is False, case_id
        assert expected in items["governed_deployment_evidence"]["blockers"], case_id
        assert expected in items["no_unresolved_blockers"]["blockers"], case_id

    ready_cases = (
        (
            "source_gate.ready_scalar",
            "operator hold",
            "domain 3 (sol): source adapter gate blockers must be a list of "
            "non-empty canonical strings",
        ),
        (
            "source_gate.ready_valid",
            ["operator hold"],
            "domain 3 (sol): operator hold",
        ),
    )
    for case_id, blockers, expected in ready_cases:
        lane = copy.deepcopy(base_lane)
        lane["source_adapter_gate"]["ready"] = True
        lane["source_adapter_gate"]["blockers"] = blockers

        checklist = module._release_checklist([lane], [])
        items = {item["id"]: item for item in checklist["items"]}

        assert checklist["ready"] is False, case_id
        assert items["governed_deployment_evidence"]["ready"] is False, case_id
        assert items["no_unresolved_blockers"]["ready"] is False, case_id
        assert expected in items["governed_deployment_evidence"]["blockers"], case_id
        assert expected in items["no_unresolved_blockers"]["blockers"], case_id
        assert (
            "domain 3 (sol): source adapter gate is not ready"
            not in items["governed_deployment_evidence"]["blockers"]
        ), case_id


def test_all_lanes_summary_rejects_malformed_source_gate_blockers():
    module = load_evidence_module()
    original_gate_summary = module._source_adapter_gate_summary
    cases = (
        (
            "source_gate.summary_scalar",
            "operator hold",
            "domain 3 (sol): source adapter gate blockers must be a list of "
            "non-empty canonical strings",
        ),
        (
            "source_gate.summary_numeric",
            [123],
            "domain 3 (sol): source adapter gate blockers[0] must be a "
            "non-empty canonical string",
        ),
        (
            "source_gate.summary_empty",
            [""],
            "domain 3 (sol): source adapter gate blockers[0] must be a "
            "non-empty canonical string",
        ),
        (
            "source_gate.summary_padded",
            [" padded "],
            "domain 3 (sol): source adapter gate blockers[0] must be a "
            "non-empty canonical string",
        ),
        (
            "source_gate.summary_valid",
            ["operator hold"],
            "domain 3 (sol): operator hold",
        ),
    )

    for case_id, blocker_value, expected in cases:
        def patched_gate_summary(profile, *args, blocker_value=blocker_value):
            summary = original_gate_summary(profile, *args)
            if profile.domain == module.SCCP_DOMAIN_SOL:
                return {
                    **summary,
                    "required": True,
                    "ready": False,
                    "blockers": blocker_value,
                }
            return summary

        module._source_adapter_gate_summary = patched_gate_summary
        try:
            summary = module.validate_evidence_bundle(complete_bundle(module))
        finally:
            module._source_adapter_gate_summary = original_gate_summary

        assert summary["production_ready"] is False, case_id
        assert expected in summary["blockers"], case_id

    def patched_ready_gate_summary(profile, *args):
        summary = original_gate_summary(profile, *args)
        if profile.domain == module.SCCP_DOMAIN_SOL:
            return {
                **summary,
                "required": True,
                "ready": True,
                "blockers": ["operator hold"],
            }
        return summary

    module._source_adapter_gate_summary = patched_ready_gate_summary
    try:
        summary = module.validate_evidence_bundle(complete_bundle(module))
    finally:
        module._source_adapter_gate_summary = original_gate_summary

    assert summary["production_ready"] is False
    assert "domain 3 (sol): operator hold" in summary["blockers"]


def test_all_lanes_release_checklist_rejects_malformed_route_canary_summary():
    module = load_evidence_module()
    lane = {
        "domain": module.SCCP_DOMAIN_ETH,
        "chain": "eth",
        "records": {
            "source_verifier_material": True,
            "source_adapter_deployment": True,
            "destination_rollout": True,
            "route_allowlist": True,
        },
        "source_adapter_gate": {
            "required": False,
            "ready": True,
            "blockers": [],
        },
        "destination_binding": {
            "expected_destination_binding_hash_matches": True,
        },
        "route_allowlist": {
            "expected_route_allowlist_hash_matches": True,
            "route_canary": {
                "status": "passed",
                "evidence_hash": "0x" + "00" * 32,
                "evidence_source": "true",
                "evidence_bound": True,
            },
        },
        "blockers": [],
    }

    checklist = module._release_checklist([lane], [])
    items = {item["id"]: item for item in checklist["items"]}
    blockers = "\n".join(items["live_route_canary_evidence"]["blockers"])

    assert checklist["ready"] is False
    assert items["live_route_canary_evidence"]["ready"] is False
    assert (
        "domain 1 (eth): route canary evidence hash must be a canonical "
        "non-zero bytes32"
    ) in blockers
    assert (
        "domain 1 (eth): live route canary evidence source must be "
        "evm_message_proof_accepted_transaction"
    ) in blockers

    lane["route_allowlist"]["route_canary"] = "passed"
    checklist = module._release_checklist([lane], [])
    items = {item["id"]: item for item in checklist["items"]}
    blockers = "\n".join(items["live_route_canary_evidence"]["blockers"])

    assert checklist["ready"] is False
    assert "domain 1 (eth): route canary summary is malformed" in blockers


def test_all_lanes_release_checklist_rejects_malformed_route_canary_scalars():
    module = load_evidence_module()
    base_lane = {
        "domain": module.SCCP_DOMAIN_ETH,
        "chain": "eth",
        "records": {
            "source_verifier_material": True,
            "source_adapter_deployment": True,
            "destination_rollout": True,
            "route_allowlist": True,
        },
        "source_adapter_gate": {
            "required": False,
            "ready": True,
            "blockers": [],
        },
        "destination_binding": {
            "expected_destination_binding_hash_matches": True,
        },
        "route_allowlist": {
            "expected_route_allowlist_hash_matches": True,
            "route_canary": {
                "status": "passed",
                "evidence_hash": hex32(0x5C),
                "evidence_source": "evm_message_proof_accepted_transaction",
                "evidence_bound": True,
                "message_proof_used": True,
                "receipt_block_finalized": True,
            },
        },
        "blockers": [],
    }
    route_canary_missing_scalar_cases = (
        "missing.evidence_bound",
        "missing.message_proof_used",
        "missing.receipt_block_finalized",
    )
    cases = (
        (
            "status",
            123,
            "domain 1 (eth): route canary status must be a non-empty canonical string",
        ),
        (
            "status",
            " passed ",
            "domain 1 (eth): route canary status must be a non-empty canonical string",
        ),
        (
            "evidence_source",
            123,
            "domain 1 (eth): live route canary evidence source must be a non-empty canonical string",
        ),
        (
            "evidence_source",
            " evm_message_proof_accepted_transaction ",
            "domain 1 (eth): live route canary evidence source must be a non-empty canonical string",
        ),
        (
            "evidence_source",
            None,
            "domain 1 (eth): live route canary evidence source is missing",
        ),
        (
            "evidence_source",
            "",
            "domain 1 (eth): live route canary evidence source is missing",
        ),
        (
            "evidence_bound",
            "true",
            "domain 1 (eth): route canary evidence_bound must be boolean",
        ),
        (
            "evidence_bound",
            1,
            "domain 1 (eth): route canary evidence_bound must be boolean",
        ),
        (
            "evidence_bound",
            None,
            "domain 1 (eth): route canary evidence_bound must be boolean",
        ),
        (
            "evidence_bound",
            False,
            "domain 1 (eth): route canary evidence is not bound",
        ),
        (
            "missing.evidence_bound",
            None,
            "domain 1 (eth): route canary evidence is not bound",
        ),
        (
            "message_proof_used",
            "true",
            "domain 1 (eth): route canary message_proof_used must be boolean",
        ),
        (
            "message_proof_used",
            1,
            "domain 1 (eth): route canary message_proof_used must be boolean",
        ),
        (
            "message_proof_used",
            None,
            "domain 1 (eth): route canary message_proof_used must be boolean",
        ),
        (
            "message_proof_used",
            False,
            "domain 1 (eth): route canary message proof must be used",
        ),
        (
            "missing.message_proof_used",
            None,
            "domain 1 (eth): route canary message proof must be used",
        ),
        (
            "receipt_block_finalized",
            "true",
            "domain 1 (eth): route canary receipt_block_finalized must be boolean",
        ),
        (
            "receipt_block_finalized",
            1,
            "domain 1 (eth): route canary receipt_block_finalized must be boolean",
        ),
        (
            "receipt_block_finalized",
            None,
            "domain 1 (eth): route canary receipt_block_finalized must be boolean",
        ),
        (
            "receipt_block_finalized",
            False,
            "domain 1 (eth): route canary receipt block must be finalized",
        ),
        (
            "missing.receipt_block_finalized",
            None,
            "domain 1 (eth): route canary receipt block must be finalized",
        ),
    )

    for field, value, expected in cases:
        lane = copy.deepcopy(base_lane)
        canary = lane["route_allowlist"]["route_canary"]
        if field in route_canary_missing_scalar_cases:
            _, target_field = field.split(".", 1)
            canary.pop(target_field)
        else:
            canary[field] = value

        checklist = module._release_checklist([lane], [])
        items = {item["id"]: item for item in checklist["items"]}
        blockers = "\n".join(items["live_route_canary_evidence"]["blockers"])

        assert checklist["ready"] is False, repr((field, value))
        assert items["live_route_canary_evidence"]["ready"] is False
        assert expected in blockers


def test_all_lanes_route_canary_evidence_sources_cover_launch_lanes():
    module = load_evidence_module()

    assert module.ROUTE_CANARY_EVIDENCE_SOURCE_BY_DOMAIN == {
        module.SCCP_DOMAIN_ETH: "evm_message_proof_accepted_transaction",
        module.SCCP_DOMAIN_BSC: "evm_message_proof_accepted_transaction",
        module.SCCP_DOMAIN_SOL: "solana_live_programdata_snapshot",
        module.SCCP_DOMAIN_TON: "ton_live_account_snapshot",
        module.SCCP_DOMAIN_TRON: "tron_message_proof_accepted_transaction",
    }


def test_all_lanes_release_checklist_rejects_malformed_tron_route_canary_boolean_scalars():
    module = load_evidence_module()
    base_lane = {
        "domain": module.SCCP_DOMAIN_TRON,
        "chain": "tron",
        "records": {
            "source_verifier_material": True,
            "source_adapter_deployment": True,
            "destination_rollout": True,
            "route_allowlist": True,
        },
        "source_adapter_gate": {
            "required": True,
            "ready": True,
            "gate_hash": hex32(0x71),
            "audit_hashes": {
                "tron_receipt_mpt_verifier_hash": hex32(0x72),
                "tron_solid_block_header_verifier_hash": hex32(0x73),
                "tron_witness_schedule_verifier_hash": hex32(0x74),
                "tron_message_event_verifier_hash": hex32(0x75),
                "tron_signature_recovery_verifier_hash": hex32(0x76),
            },
            "blockers": [],
        },
        "destination_binding": {
            "expected_destination_binding_hash_matches": True,
        },
        "route_allowlist": {
            "expected_route_allowlist_hash_matches": True,
            "route_canary": {
                "status": "passed",
                "evidence_hash": hex32(0x7C),
                "evidence_source": module.ROUTE_CANARY_EVIDENCE_SOURCE_BY_DOMAIN[
                    module.SCCP_DOMAIN_TRON
                ],
                "evidence_bound": True,
                "message_proof_used": True,
                "raw_data_owner_matches_transaction": True,
                "signature_recovers_to_owner": True,
            },
        },
        "blockers": [],
    }
    route_canary_missing_scalar_cases = (
        "missing.message_proof_used",
        "missing.raw_data_owner_matches_transaction",
        "missing.signature_recovers_to_owner",
    )
    cases = (
        (
            "message_proof_used",
            "true",
            "domain 5 (tron): route canary message_proof_used must be boolean",
        ),
        (
            "message_proof_used",
            None,
            "domain 5 (tron): route canary message_proof_used must be boolean",
        ),
        (
            "message_proof_used",
            False,
            "domain 5 (tron): route canary message proof must be used",
        ),
        (
            "missing.message_proof_used",
            None,
            "domain 5 (tron): route canary message proof must be used",
        ),
        (
            "raw_data_owner_matches_transaction",
            "true",
            "domain 5 (tron): route canary raw_data_owner_matches_transaction must be boolean",
        ),
        (
            "raw_data_owner_matches_transaction",
            1,
            "domain 5 (tron): route canary raw_data_owner_matches_transaction must be boolean",
        ),
        (
            "raw_data_owner_matches_transaction",
            None,
            "domain 5 (tron): route canary raw_data_owner_matches_transaction must be boolean",
        ),
        (
            "raw_data_owner_matches_transaction",
            False,
            "domain 5 (tron): route canary raw_data owner must match transaction",
        ),
        (
            "missing.raw_data_owner_matches_transaction",
            None,
            "domain 5 (tron): route canary raw_data owner must match transaction",
        ),
        (
            "signature_recovers_to_owner",
            "true",
            "domain 5 (tron): route canary signature_recovers_to_owner must be boolean",
        ),
        (
            "signature_recovers_to_owner",
            1,
            "domain 5 (tron): route canary signature_recovers_to_owner must be boolean",
        ),
        (
            "signature_recovers_to_owner",
            None,
            "domain 5 (tron): route canary signature_recovers_to_owner must be boolean",
        ),
        (
            "signature_recovers_to_owner",
            False,
            "domain 5 (tron): route canary signature must recover to owner",
        ),
        (
            "missing.signature_recovers_to_owner",
            None,
            "domain 5 (tron): route canary signature must recover to owner",
        ),
    )

    for field, value, expected in cases:
        lane = copy.deepcopy(base_lane)
        canary = lane["route_allowlist"]["route_canary"]
        if field in route_canary_missing_scalar_cases:
            _, target_field = field.split(".", 1)
            canary.pop(target_field)
        else:
            canary[field] = value

        checklist = module._release_checklist([lane], [])
        items = {item["id"]: item for item in checklist["items"]}
        blockers = "\n".join(items["live_route_canary_evidence"]["blockers"])

        assert checklist["ready"] is False, repr((field, value))
        assert items["live_route_canary_evidence"]["ready"] is False
        assert expected in blockers


def test_all_lanes_evidence_rejects_stale_route_canary_metadata():
    module = load_evidence_module()
    records = complete_bundle(module)

    ton_route = records["sccp_route_allowlists"][3]
    ton_route["_comment_route_canary_status"] = "pending"
    ton_route["_comment_route_canary_route_allowlist_hash"] = hex32(0xAC)
    ton_route["_comment_route_canary_destination_binding_hash"] = hex32(0xAD)

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "domain 4 (ton): route canary status metadata must be passed" in blockers
    assert (
        "domain 4 (ton): route canary route allowlist hash must match route_allowlist_hash"
        in blockers
    )
    assert (
        "domain 4 (ton): route canary destination binding hash must match destination_binding_hash"
        in blockers
    )


def test_all_lanes_evidence_rejects_padded_route_hashes():
    module = load_evidence_module()
    records = complete_bundle(module)

    route = records["sccp_route_allowlists"][0]
    route["route_allowlist_hash"] = " " + route["route_allowlist_hash"]
    route["_comment_route_canary_evidence_hash"] = (
        " " + route["_comment_route_canary_evidence_hash"]
    )

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "domain 1 (eth): route_allowlist_hash must be a non-zero" in blockers
    assert "domain 1 (eth): route canary evidence hash metadata" in blockers


def test_all_lanes_evidence_rejects_route_canary_hash_replay():
    module = load_evidence_module()
    records = complete_bundle(module)

    eth_route = records["sccp_route_allowlists"][0]
    eth_route["_comment_route_canary_evidence_hash"] = eth_route[
        "route_allowlist_hash"
    ]
    bsc_route = records["sccp_route_allowlists"][1]
    bsc_route["_comment_route_canary_evidence_hash"] = records[
        "sccp_source_verifier_materials"
    ][1]["_comment_source_verifier_material_hash"]
    sol_route = records["sccp_route_allowlists"][2]
    sol_route["_comment_route_canary_evidence_hash"] = sol_route[
        "_comment_route_canary_destination_binding_hash"
    ]
    ton_route = records["sccp_route_allowlists"][3]
    ton_route["_comment_route_canary_evidence_hash"] = records[
        "sccp_source_adapter_engine_deployments"
    ][3]["_comment_source_adapter_engine_deployment_hash"]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert (
        "domain 1 (eth): route canary evidence hash must be distinct from "
        "route_allowlist_hash"
    ) in blockers
    assert (
        "domain 2 (bsc): route canary evidence hash must be distinct from "
        "source_verifier_material_hash"
    ) in blockers
    assert (
        "domain 3 (sol): route canary evidence hash must be distinct from "
        "destination_binding_hash"
    ) in blockers
    assert (
        "domain 4 (ton): route canary evidence hash must be distinct from "
        "source_adapter_engine_deployment_hash"
    ) in blockers


def test_all_lanes_evidence_rejects_cross_lane_route_canary_replay():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_route = records["sccp_route_allowlists"][0]
    bsc_route = records["sccp_route_allowlists"][1]
    bsc_route["_comment_route_canary_evidence_hash"] = eth_route[
        "_comment_route_canary_evidence_hash"
    ]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    bsc_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_BSC
    )
    assert bsc_lane["production_ready"] is False
    assert (
        "route canary evidence hash for domain 2 must be distinct from domain 1"
        in "\n".join(summary["blockers"])
    )
    assert (
        "route canary evidence hash for domain 2 must be distinct from domain 1"
        in "\n".join(bsc_lane["blockers"])
    )


def test_all_lanes_evidence_rejects_cross_lane_route_canary_source_hash_replay():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_material = records["sccp_source_verifier_materials"][0]
    bsc_route = records["sccp_route_allowlists"][1]
    bsc_route["_comment_route_canary_evidence_hash"] = eth_material[
        "_comment_source_verifier_material_hash"
    ]
    ton_deployment = records["sccp_source_adapter_engine_deployments"][3]
    tron_route = records["sccp_route_allowlists"][4]
    tron_route["_comment_route_canary_evidence_hash"] = ton_deployment[
        "_comment_source_adapter_engine_deployment_hash"
    ]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    bsc_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_BSC
    )
    tron_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_TRON
    )
    assert bsc_lane["production_ready"] is False
    assert tron_lane["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert (
        "route canary evidence hash for domain 2 must be distinct from "
        "source_verifier_material_hash for domain 1"
    ) in blockers
    assert (
        "route canary evidence hash for domain 5 must be distinct from "
        "source_adapter_engine_deployment_hash for domain 4"
    ) in blockers
    assert (
        "route canary evidence hash for domain 2 must be distinct from "
        "source_verifier_material_hash for domain 1"
    ) in "\n".join(bsc_lane["blockers"])
    assert (
        "route canary evidence hash for domain 5 must be distinct from "
        "source_adapter_engine_deployment_hash for domain 4"
    ) in "\n".join(tron_lane["blockers"])


def test_all_lanes_evidence_rejects_cross_lane_route_canary_governed_hash_replay():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_destination = records["sccp_destination_rollouts"][0]
    bsc_route = records["sccp_route_allowlists"][1]
    bsc_route["_comment_route_canary_evidence_hash"] = eth_destination[
        "destination_binding_hash"
    ]
    sol_route_source = records["sccp_route_allowlists"][2]
    tron_route = records["sccp_route_allowlists"][4]
    tron_route["_comment_route_canary_evidence_hash"] = sol_route_source[
        "route_allowlist_hash"
    ]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    bsc_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_BSC
    )
    tron_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_TRON
    )
    assert bsc_lane["production_ready"] is False
    assert tron_lane["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert (
        "route canary evidence hash for domain 2 must be distinct from "
        "destination_binding_hash for domain 1"
    ) in blockers
    assert (
        "route canary evidence hash for domain 5 must be distinct from "
        "route_allowlist_hash for domain 3"
    ) in blockers
    assert (
        "route canary evidence hash for domain 2 must be distinct from "
        "destination_binding_hash for domain 1"
    ) in "\n".join(bsc_lane["blockers"])
    assert (
        "route canary evidence hash for domain 5 must be distinct from "
        "route_allowlist_hash for domain 3"
    ) in "\n".join(tron_lane["blockers"])


def test_all_lanes_evidence_rejects_cross_lane_route_canary_source_gate_replay():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_deployment = records["sccp_source_adapter_engine_deployments"][0]
    bsc_route = records["sccp_route_allowlists"][1]
    bsc_route["_comment_route_canary_evidence_hash"] = eth_deployment[
        "evm_source_gate_hash"
    ]
    sol_deployment = records["sccp_source_adapter_engine_deployments"][2]
    tron_route = records["sccp_route_allowlists"][4]
    tron_route["_comment_route_canary_evidence_hash"] = sol_deployment[
        "solana_tower_replay_verifier_hash"
    ]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    bsc_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_BSC
    )
    tron_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_TRON
    )
    assert bsc_lane["production_ready"] is False
    assert tron_lane["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert (
        "route canary evidence hash for domain 2 must be distinct from "
        "source_adapter_gate_hash for domain 1"
    ) in blockers
    assert (
        "route canary evidence hash for domain 5 must be distinct from "
        "source_adapter_gate.audit_hashes.solana_tower_replay_verifier_hash for "
        "domain 3"
    ) in blockers
    assert (
        "route canary evidence hash for domain 2 must be distinct from "
        "source_adapter_gate_hash for domain 1"
    ) in "\n".join(bsc_lane["blockers"])
    assert (
        "route canary evidence hash for domain 5 must be distinct from "
        "source_adapter_gate.audit_hashes.solana_tower_replay_verifier_hash for "
        "domain 3"
    ) in "\n".join(tron_lane["blockers"])


def test_all_lanes_evidence_rejects_cross_lane_route_canary_transcript_replay():
    module = load_evidence_module()
    records = complete_bundle(module)
    eth_route = records["sccp_route_allowlists"][0]
    bsc_route = records["sccp_route_allowlists"][1]
    bsc_route["_comment_route_canary_evidence_hash"] = eth_route[
        "_comment_evm_route_canary_message_id"
    ]
    tron_route_source = records["sccp_route_allowlists"][4]
    ton_route = records["sccp_route_allowlists"][3]
    ton_route["_comment_route_canary_evidence_hash"] = tron_route_source[
        "_comment_tron_route_canary_transaction_id"
    ]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    bsc_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_BSC
    )
    ton_lane = next(
        lane for lane in summary["lanes"] if lane["domain"] == module.SCCP_DOMAIN_TON
    )
    assert bsc_lane["production_ready"] is False
    assert ton_lane["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert (
        "route canary evidence hash for domain 2 must be distinct from "
        "route_canary.message_id for domain 1"
    ) in blockers
    assert (
        "route canary evidence hash for domain 4 must be distinct from "
        "route_canary.transaction_id for domain 5"
    ) in blockers
    assert (
        "route canary evidence hash for domain 2 must be distinct from "
        "route_canary.message_id for domain 1"
    ) in "\n".join(bsc_lane["blockers"])
    assert (
        "route canary evidence hash for domain 4 must be distinct from "
        "route_canary.transaction_id for domain 5"
    ) in "\n".join(ton_lane["blockers"])


def test_all_lanes_evidence_requires_route_component_hashes():
    module = load_evidence_module()
    records = complete_bundle(module)

    records["sccp_source_adapter_engine_deployments"][4][
        "source_bridge_owner_address"
    ] = hex20(0xA7)

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "domain 5 (tron): source_bridge_owner_address must match" in blockers
    assert "domain 5 (tron): route_allowlist_hash cannot be recomputed" in blockers
    assert "source_verifier_material_hash must be a non-zero" not in blockers


def test_all_lanes_evidence_redacts_route_allowlist_recompute_failures(
    monkeypatch,
) -> None:
    """Route allowlist recomputation blockers must not echo exception payloads."""

    module = load_evidence_module()
    records = complete_bundle(module)

    for exception_type in (SystemExit, TypeError, ValueError, RuntimeError):

        def fail_recompute(
            _profile,
            _source_hashes,
            _destination_binding,
            exception_type=exception_type,
        ):
            raise exception_type("secret-token operator route material")

        monkeypatch.setattr(module, "_expected_route_allowlist_hash", fail_recompute)

        summary = module.validate_evidence_bundle(records)

        assert summary["production_ready"] is False
        blockers = "\n".join(summary["blockers"])
        assert "route_allowlist_hash cannot be recomputed" in blockers
        assert "route_allowlist_hash cannot be recomputed:" not in blockers
        assert "secret-token" not in blockers
        assert exception_type.__name__ not in blockers


def test_all_lanes_evidence_rejects_canonical_source_validator_drift():
    module = load_evidence_module()
    records = complete_bundle(module)

    records["sccp_source_adapter_engine_deployments"][0][
        "adapter_verifier_vk_hash"
    ] = hex32(0xA1)

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "domain 1 (eth): eth source evidence rejected by canonical validator" in blockers


def test_all_lanes_evidence_redacts_source_validator_failures(
    monkeypatch,
) -> None:
    """Canonical source validator blockers must not echo exception payloads."""

    module = load_evidence_module()
    original_loader = module._load_sibling_module
    source_validator_exception_types = (
        module.argparse.ArgumentTypeError,
        SystemExit,
        TypeError,
        ValueError,
        RuntimeError,
    )

    for exception_type in source_validator_exception_types:
        records = complete_bundle(module)

        def fail_validator(_args):
            raise exception_type("secret-token source validator material")

        def load_sibling(name):
            if name == "sccp_eth_source_bridge_evidence.py":
                real_module = original_loader(name)
                module_attrs = dict(real_module.__dict__)
                module_attrs["_validate_eth_source_evidence_args"] = fail_validator
                return SimpleNamespace(**module_attrs)
            return original_loader(name)

        monkeypatch.setattr(module, "_load_sibling_module", load_sibling)

        summary = module.validate_evidence_bundle(records)

        assert summary["production_ready"] is False
        blockers = "\n".join(summary["blockers"])
        assert "eth source evidence rejected by canonical validator" in blockers
        assert "eth source evidence rejected by canonical validator:" not in blockers
        assert "secret-token" not in blockers
        assert exception_type.__name__ not in blockers


def test_all_lanes_evidence_rejects_malformed_destination_identities():
    module = load_evidence_module()
    records = complete_bundle(module)

    records["sccp_destination_rollouts"][2]["verifier_identity"] = "not-base58"
    records["sccp_destination_rollouts"][3][
        "verifier_identity"
    ] = "-1:" + "22" * 32
    records["sccp_destination_rollouts"][4][
        "verifier_identity"
    ] = "T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb"

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert (
        "domain 3 (sol): verifier_identity is not canonical for sol" in blockers
    )
    assert (
        "domain 4 (ton): verifier_identity is not canonical for ton" in blockers
    )
    assert (
        "domain 5 (tron): verifier_identity is not canonical for tron" in blockers
    )


def test_all_lanes_evidence_redacts_destination_identity_failures(
    monkeypatch,
) -> None:
    """Destination verifier identity blockers must not echo parser payloads."""

    module = load_evidence_module()
    records = complete_bundle(module)
    profile = module.LANE_PROFILES[module.SCCP_DOMAIN_ETH]
    destination = records["sccp_destination_rollouts"][0]

    for exception_type in (SystemExit, TypeError, ValueError, RuntimeError):

        def fail_parse(_identity, *, label, exception_type=exception_type):
            raise exception_type(f"secret-token {label} parser detail")

        monkeypatch.setattr(
            module,
            "_load_sibling_module",
            lambda _name, fail_parse=fail_parse: SimpleNamespace(
                parse_evm_address=fail_parse
            ),
        )

        errors = module._check_destination_verifier_identity(profile, destination)
        rendered = "\n".join(errors)

        assert errors == ["verifier_identity is not canonical for eth"]
        assert "verifier_identity is not canonical for eth:" not in rendered
        assert "secret-token" not in rendered
        assert exception_type.__name__ not in rendered


def test_all_lanes_cli_merges_toml_snippets(tmp_path, capsys):
    module = load_evidence_module()
    records = complete_bundle(module)
    source_records = {
        "sccp_source_verifier_materials": records["sccp_source_verifier_materials"],
        "sccp_source_adapter_engine_deployments": records[
            "sccp_source_adapter_engine_deployments"
        ],
        "sccp_destination_rollouts": [],
        "sccp_route_allowlists": [],
    }
    rollout_records = {
        "sccp_source_verifier_materials": [],
        "sccp_source_adapter_engine_deployments": [],
        "sccp_destination_rollouts": records["sccp_destination_rollouts"],
        "sccp_route_allowlists": records["sccp_route_allowlists"],
    }
    source_path = tmp_path / "source.toml"
    rollout_path = tmp_path / "rollout.toml"
    source_path.write_text(render_records(source_records), encoding="utf-8")
    rollout_path.write_text(render_records(rollout_records), encoding="utf-8")

    assert module.main([str(source_path), str(rollout_path)]) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["production_ready"] is True
    assert all(
        lane["destination_binding"]["expected_destination_binding_hash_matches"]
        for lane in output["lanes"]
    )

    bad_records = complete_bundle(module)
    bad_records["sccp_destination_rollouts"].pop()
    bad_path = tmp_path / "bad.toml"
    bad_path.write_text(render_records(bad_records), encoding="utf-8")

    assert module.main([str(bad_path)]) == 1
    output = json.loads(capsys.readouterr().out)
    assert output["production_ready"] is False
    assert "missing destination rollout" in "\n".join(output["blockers"])
