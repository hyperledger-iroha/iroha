import base64
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


def load_substrate_live_module():
    script_path = (
        Path(__file__).resolve().parents[2]
        / "scripts"
        / "sccp_substrate_live_evidence.py"
    )
    spec = spec_from_file_location("sccp_substrate_live_evidence_for_all_lanes", script_path)
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
    proof_words = [
        abi_word_u32(1),
        message_id,
        abi_word_u32(source_domain),
        commitment_root,
        *(bytes([index]) * 32 for index in range(1, 9)),
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
    network_id = bytes.fromhex("33" * 32)
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
                        "blockHash": "0x" + "aa" * 32,
                        "blockNumber": "0x1234",
                        "logs": [
                            {
                                "address": bridge,
                                "logIndex": "0x0",
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
        if method == "eth_getTransactionByHash":
            assert params[0] == "0x" + route_canary_transaction_hash.hex()
            return FakeResponse(
                {
                    "jsonrpc": "2.0",
                    "id": payload["id"],
                    "result": {
                        "hash": "0x" + route_canary_transaction_hash.hex(),
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
        raise AssertionError(f"unexpected method {method}")

    return SimpleNamespace(
        opener=opener,
        bridge=bridge,
        bridge_runtime=bridge_runtime,
        bridge_code_hash=bridge_code_hash,
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


def fake_substrate_live_opener(
    module,
    *,
    finalized_head,
    runtime_code,
    spec_name="sora2",
    spec_version=1234,
    transaction_version=7,
):
    def opener(request, timeout):
        del timeout
        payload = json.loads(request.data.decode("utf-8"))
        method = payload["method"]
        if method == "chain_getFinalizedHead":
            return FakeResponse(
                {
                    "jsonrpc": "2.0",
                    "id": payload["id"],
                    "result": "0x" + finalized_head.hex(),
                }
            )
        if method == "state_getRuntimeVersion":
            assert payload["params"] == ["0x" + finalized_head.hex()]
            return FakeResponse(
                {
                    "jsonrpc": "2.0",
                    "id": payload["id"],
                    "result": {
                        "specName": spec_name,
                        "specVersion": spec_version,
                        "transactionVersion": transaction_version,
                    },
                }
            )
        if method == "state_getStorage":
            assert payload["params"] == [
                module.RUNTIME_CODE_STORAGE_KEY,
                "0x" + finalized_head.hex(),
            ]
            return FakeResponse(
                {
                    "jsonrpc": "2.0",
                    "id": payload["id"],
                    "result": "0x" + runtime_code.hex(),
                }
            )
        raise AssertionError(f"unexpected method {method}")

    return opener


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
            record["_comment_evm_source_deployment_receipt_status"] = "0x1"
            record["_comment_evm_source_deployment_contract_address"] = record[
                "source_bridge_emitter_address"
            ]
            record["_comment_evm_source_deployment_block_hash"] = hex32(seed + 28)
            record["_comment_evm_source_deployment_block_number"] = str(1000 + seed)
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
        record["destination_network_id"] = hex32(seed + 23)
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
    elif profile.chain.startswith("sora"):
        substrate_module = module._load_sibling_module(
            "sccp_substrate_destination_evidence.py"
        )
        runtime_code = bytes([0x00, 0x61, 0x73, 0x6D, seed & 0xFF, 0x01])
        record["verifier_code_hash"] = (
            "0x" + substrate_module.substrate_runtime_code_hash(runtime_code).hex()
        )
        record["destination_binding_key"] = substrate_module.substrate_destination_binding_key(
            profile.domain
        )
        record["destination_binding_hash"] = (
            "0x"
            + substrate_module.substrate_destination_binding_hash(profile.domain).hex()
        )
        record["_comment_substrate_finalized_head"] = hex32(seed + 25)
        record["_comment_substrate_runtime_spec_name"] = profile.chain
        record["_comment_substrate_runtime_spec_version"] = str(1000 + seed)
        record["_comment_substrate_runtime_transaction_version"] = str(10 + seed)
        record["_comment_substrate_runtime_code_hash"] = record["verifier_code_hash"]
        record["_comment_substrate_runtime_code_base64"] = base64.b64encode(
            runtime_code
        ).decode("ascii")
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
        canary_hash = evm_module.evm_route_canary_transaction_evidence_hash(
            route_allowlist_hash=raw_hex(route_hash),
            bridge_address=raw_hex(destination["destination_bridge_address"]),
            transaction_hash=transaction_hash,
            log_index=0,
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
        )
        route["_comment_route_canary_evidence_hash"] = "0x" + canary_hash.hex()
        route["_comment_evm_route_canary_transaction_hash"] = (
            "0x" + transaction_hash.hex()
        )
        route["_comment_evm_route_canary_log_index"] = "0"
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
        signature_sha256 = bytes([seed + 32]) * 32
        signature_recovered_address = transaction_owner_address
        route_hash_raw = raw_hex(route_hash)
        destination_binding_raw = raw_hex(destination_binding_hash)
        canary_hash = live_module._tron_route_canary_transaction_evidence_hash(
            route_allowlist_hash=route_hash_raw,
            transaction_id=transaction_id,
            transaction_owner_address=transaction_owner_address,
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
    elif (
        profile.chain.startswith("sora")
        and destination is not None
        and source_record_hashes is not None
        and destination_binding_hash is not None
    ):
        substrate_module = module._load_sibling_module(
            "sccp_substrate_destination_evidence.py"
        )
        canary_hash = substrate_module.substrate_route_canary_evidence_hash(
            domain=profile.domain,
            route_allowlist_hash=raw_hex(route_hash),
            destination_binding_hash=raw_hex(destination_binding_hash),
            source_verifier_material_hash=raw_hex(
                source_record_hashes["source_verifier_material_hash"]
            ),
            source_adapter_engine_deployment_hash=raw_hex(
                source_record_hashes["source_adapter_engine_deployment_hash"]
            ),
            verifier_entrypoint=destination["verifier_identity"],
            verifier_code_hash=raw_hex(destination["verifier_code_hash"]),
            finalized_head=raw_hex(destination["_comment_substrate_finalized_head"]),
            runtime_spec_name=destination["_comment_substrate_runtime_spec_name"],
            runtime_spec_version=int(
                destination["_comment_substrate_runtime_spec_version"]
            ),
            runtime_transaction_version=int(
                destination["_comment_substrate_runtime_transaction_version"]
            ),
            runtime_code=substrate_module.parse_runtime_code_base64(
                destination["_comment_substrate_runtime_code_base64"],
                label="Substrate runtime code",
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
        if profile.chain == "bsc":
            bsc_module = module._load_sibling_module("sccp_bsc_source_bridge_evidence.py")
            deployment["adapter_verifier_vk_hash"] = (
                "0x" + bsc_module.bsc_source_adapter_verifier_vk_hash().hex()
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
        if profile.chain.startswith("sora"):
            substrate_module = module._load_sibling_module(
                "sccp_substrate_source_evidence.py"
            )
            deployment["adapter_verifier_vk_hash"] = (
                "0x"
                + substrate_module.substrate_source_adapter_verifier_vk_hash(
                    profile.domain
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
    return [
        "# sccp_substrate_finalized_head = "
        + toml_value(entry["_comment_substrate_finalized_head"]),
        "# sccp_substrate_runtime_spec_name = "
        + toml_value(entry["_comment_substrate_runtime_spec_name"]),
        "# sccp_substrate_runtime_spec_version = "
        + toml_value(entry["_comment_substrate_runtime_spec_version"]),
        "# sccp_substrate_runtime_transaction_version = "
        + toml_value(entry["_comment_substrate_runtime_transaction_version"]),
        "# sccp_substrate_runtime_code_hash = "
        + toml_value(entry["_comment_substrate_runtime_code_hash"]),
        "# sccp_substrate_runtime_code_base64 = "
        + toml_value(entry["_comment_substrate_runtime_code_base64"]),
        "# sccp_substrate_destination_binding_hash = "
        + toml_value(entry["destination_binding_hash"])
    ]


SOURCE_VERIFIER_MATERIAL_COMMENT_KEYS = {
    "eth": "sccp_eth_source_verifier_material_hash",
    "bsc": "sccp_bsc_source_verifier_material_hash",
    "sol": "sccp_solana_source_verifier_material_hash",
    "ton": "sccp_ton_source_verifier_material_hash",
    "tron": "sccp_tron_source_verifier_material_hash",
    "sora-kusama": "sccp_substrate_source_verifier_material_hash",
    "sora-polkadot": "sccp_substrate_source_verifier_material_hash",
    "sora2": "sccp_substrate_source_verifier_material_hash",
}
SOURCE_DEPLOYMENT_COMMENT_KEYS = {
    "eth": "sccp_eth_source_adapter_engine_deployment_hash",
    "bsc": "sccp_bsc_source_adapter_engine_deployment_hash",
    "sol": "sccp_solana_source_adapter_engine_deployment_hash",
    "ton": "sccp_ton_source_adapter_engine_deployment_hash",
    "tron": "sccp_tron_source_adapter_engine_deployment_hash",
    "sora-kusama": "sccp_substrate_source_adapter_engine_deployment_hash",
    "sora-polkadot": "sccp_substrate_source_adapter_engine_deployment_hash",
    "sora2": "sccp_substrate_source_adapter_engine_deployment_hash",
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
        "# sccp_evm_source_bridge_address = "
        + toml_value(entry["_comment_evm_source_bridge_address"]),
        "# sccp_evm_source_bridge_runtime_code_hash = "
        + toml_value(entry["_comment_evm_source_bridge_code_hash"]),
        "# sccp_evm_source_bridge_runtime_bytecode_hex = "
        + toml_value(entry["_comment_evm_source_bridge_runtime_bytecode_hex"]),
        "# sccp_evm_source_deployment_transaction_hash = "
        + toml_value(entry["_comment_evm_source_deployment_transaction_hash"]),
        "# sccp_evm_source_deployment_receipt_status = "
        + toml_value(entry["_comment_evm_source_deployment_receipt_status"]),
        "# sccp_evm_source_deployment_contract_address = "
        + toml_value(entry["_comment_evm_source_deployment_contract_address"]),
        "# sccp_evm_source_deployment_block_hash = "
        + toml_value(entry["_comment_evm_source_deployment_block_hash"]),
        "# sccp_evm_source_deployment_block_number = "
        + toml_value(entry["_comment_evm_source_deployment_block_number"]),
        ]
    )
    return comments


def source_deployment_comment_lines(entry):
    chain = entry["source_chain"]
    if "_comment_source_adapter_engine_deployment_hash" not in entry:
        return []
    return [
        "# "
        + SOURCE_DEPLOYMENT_COMMENT_KEYS[chain]
        + " = "
        + toml_value(entry["_comment_source_adapter_engine_deployment_hash"])
    ]


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
        ("sccp_evm_route_canary_log_index", "_comment_evm_route_canary_log_index"),
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
            "sccp_tron_route_canary_transaction_id",
            "_comment_tron_route_canary_transaction_id",
        ),
        (
            "sccp_tron_route_canary_transaction_owner_address",
            "_comment_tron_route_canary_transaction_owner_address",
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
        source_adapter_gate = lane["source_adapter_gate"]
        if lane["chain"] == "sol":
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
        else:
            assert source_adapter_gate == {
                "required": False,
                "ready": True,
                "gate_hash": "",
                "audit_hashes": {},
                "blockers": [],
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
        "sccp_source_verifier_materials: record 8 uses unsupported source_domain 99"
        in blockers
    )
    assert (
        "sccp_source_adapter_engine_deployments: record 8 uses unsupported source_domain 99"
        in blockers
    )
    assert "sccp_destination_rollouts: record 8 uses unsupported domain 99" in blockers
    assert "sccp_route_allowlists: record 8 uses unsupported domain 99" in blockers


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


def test_all_lanes_evidence_rejects_ton_audit_hash_reusing_template_material():
    module = load_evidence_module()
    ton_module = module._load_sibling_module("sccp_ton_source_state_evidence.py")
    records = complete_bundle(module)
    ton_deployment = records["sccp_source_adapter_engine_deployments"][3]
    template_hash = ton_module._ton_template_component_hash(
        ton_module.TON_SOURCE_TRUST_ANCHOR_ID,
        "source-trust-anchor",
    )
    ton_deployment["ton_masterchain_config_verifier_hash"] = "0x" + template_hash.hex()

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "domain 4 (ton): TON full light-client gate cannot be recomputed" in blockers
    assert "built-in template material" in blockers


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
        route_canary_log_index=int(route["_comment_evm_route_canary_log_index"]),
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
    )
    evm_toml = evm_module.render_toml(
        args,
        raw_hex(destination["destination_binding_hash"]),
    )
    assert '# sccp_evm_rpc_chain_id = "1"' in evm_toml
    assert "# sccp_evm_bridge_runtime_code_hash" in evm_toml
    assert "# sccp_evm_bridge_runtime_bytecode_hex" in evm_toml
    assert "# sccp_evm_verifier_runtime_code_hash" in evm_toml
    assert "# sccp_evm_verifier_runtime_bytecode_hex" in evm_toml
    assert "# sccp_evm_verifier_key_hash" in evm_toml
    assert "# sccp_evm_route_canary_transaction_hash" in evm_toml
    assert "evm_route_canary_transaction_hash = " in evm_toml
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
    eth_material.pop("_comment_evm_source_bridge_address")
    eth_material.pop("_comment_evm_source_bridge_code_hash")
    eth_material.pop("_comment_evm_source_bridge_runtime_bytecode_hex")
    eth_material.pop("_comment_evm_source_deployment_transaction_hash")
    eth_material.pop("_comment_evm_source_deployment_receipt_status")
    eth_material.pop("_comment_evm_source_deployment_contract_address")
    eth_material.pop("_comment_evm_source_deployment_block_hash")
    eth_material.pop("_comment_evm_source_deployment_block_number")

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "EVM source live RPC chain-id metadata is required" in blockers
    assert "EVM source bridge address metadata must be a non-zero" in blockers
    assert "EVM source bridge runtime code hash metadata must be a non-zero" in blockers
    assert "EVM source bridge runtime bytecode metadata must be present" in blockers
    assert "EVM source deployment transaction hash metadata must be a non-zero" in blockers
    assert "EVM source deployment receipt status metadata must be 0x1" in blockers
    assert "EVM source deployment contract address metadata must be a non-zero" in blockers
    assert "EVM source deployment block hash metadata must be a non-zero" in blockers
    assert "EVM source deployment block number metadata must be a positive" in blockers


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
    assert '# sccp_evm_source_deployment_block_number = "4660"' in source_toml

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
    args.deployment_receipt_contract_address = raw_hex(
        material["_comment_evm_source_deployment_contract_address"]
    )
    args.deployment_receipt_block_hash = raw_hex(
        material["_comment_evm_source_deployment_block_hash"]
    )
    args.deployment_receipt_block_number = int(
        material["_comment_evm_source_deployment_block_number"]
    )
    args.source_bridge_runtime_bytecode_hex = eth_module.parse_runtime_bytecode_hex(
        material["_comment_evm_source_bridge_runtime_bytecode_hex"],
        label="source bridge runtime bytecode",
    )

    source_toml = eth_module.render_toml(args)
    assert '# sccp_evm_source_rpc_chain_id = "1"' in source_toml
    assert "# sccp_evm_source_bridge_runtime_code_hash" in source_toml
    assert "# sccp_evm_source_bridge_runtime_bytecode_hex" in source_toml
    assert "# sccp_evm_source_deployment_transaction_hash" in source_toml
    assert "# sccp_evm_source_deployment_block_number" in source_toml
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
    substrate_destination = records["sccp_destination_rollouts"][5]

    eth_destination["_comment_solana_programdata_address"] = sol_destination[
        "_comment_solana_programdata_address"
    ]
    eth_destination["ton_account_state_hash"] = ton_destination[
        "ton_account_state_hash"
    ]
    eth_destination["_comment_substrate_runtime_spec_name"] = substrate_destination[
        "_comment_substrate_runtime_spec_name"
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
        "domain 1 (eth): _comment_substrate_runtime_spec_name is only valid "
        "for Substrate destination live evidence"
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
    assert "source_bridge_network_id must be an exact 32-byte hex value" in blockers

    records = complete_bundle(module)
    destination = records["sccp_destination_rollouts"][tron_index]
    destination["verifier_code_hash"] = " " + destination["verifier_code_hash"]

    summary = module.validate_evidence_bundle(records)

    blockers = "\n".join(summary["blockers"])
    assert summary["production_ready"] is False
    assert "verifier_code_hash must be an exact 32-byte hex value" in blockers


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
    substrate_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(
        module.SCCP_DOMAIN_SORA2
    )

    records["sccp_destination_rollouts"][sol_index][
        "_comment_solana_programdata_slot"
    ] = "01003"
    records["sccp_destination_rollouts"][ton_index][
        "_comment_ton_last_transaction_lt"
    ] = "02004"
    records["sccp_destination_rollouts"][ton_index]["ton_last_transaction_lt"] = (
        "02004"
    )
    records["sccp_destination_rollouts"][substrate_index][
        "_comment_substrate_runtime_spec_version"
    ] = "01234"

    summary = module.validate_evidence_bundle(records)
    blockers = "\n".join(
        blocker for lane in summary["lanes"] for blocker in lane["blockers"]
    )

    assert "Solana ProgramData slot metadata must be a positive decimal string" in blockers
    assert "TON last transaction LT metadata must be a positive decimal string" in blockers
    assert "Substrate runtime specVersion metadata must be a decimal string" in blockers


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
    assert "canonical base64" in blockers

    records = complete_bundle(module)
    solana_destination = records["sccp_destination_rollouts"][2]
    solana_destination["_comment_solana_programdata_executable_base64"] = (
        base64.b64encode(b"not-elf").decode("ascii")
    )

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "Solana ProgramData executable base64 metadata is invalid" in blockers
    assert "BPF ELF" in blockers


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
    assert "canonical base64" in blockers


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


def test_all_lanes_accepts_verified_substrate_live_toml(tmp_path):
    module = load_evidence_module()
    live_module = load_substrate_live_module()
    records = complete_bundle(module)
    domain = module.SCCP_DOMAIN_SORA2
    substrate_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(domain)
    profile = module.LANE_PROFILES[domain]
    material = records["sccp_source_verifier_materials"][substrate_index]
    deployment = records["sccp_source_adapter_engine_deployments"][substrate_index]
    source_hashes = module._canonical_source_record_hashes(profile, material, deployment)
    destination_hash = raw_hex(
        records["sccp_destination_rollouts"][substrate_index][
            "destination_binding_hash"
        ]
    )
    route_allowlist_hash = module.route_allowlist_hash_for_lane_evidence(
        profile,
        raw_hex(source_hashes["source_verifier_material_hash"]),
        raw_hex(source_hashes["source_adapter_engine_deployment_hash"]),
        destination_hash,
    )
    finalized_head = bytes.fromhex("55" * 32)
    runtime_code = bytes.fromhex("0061736d010203040506")
    runtime_code_hash = live_module.runtime_code_hash(runtime_code)
    live = live_module.collect_live_evidence(
        "https://substrate.example",
        domain=domain,
        opener=fake_substrate_live_opener(
            live_module,
            finalized_head=finalized_head,
            runtime_code=runtime_code,
        ),
        timeout=1.0,
    )
    route_canary_evidence_hash = (
        live_module.evidence.substrate_route_canary_evidence_hash(
            domain=domain,
            route_allowlist_hash=route_allowlist_hash,
            destination_binding_hash=destination_hash,
            source_verifier_material_hash=raw_hex(
                source_hashes["source_verifier_material_hash"]
            ),
            source_adapter_engine_deployment_hash=raw_hex(
                source_hashes["source_adapter_engine_deployment_hash"]
            ),
            verifier_entrypoint=live["verifier_entrypoint"],
            verifier_code_hash=runtime_code_hash,
            finalized_head=raw_hex(live["finalized_head"]),
            runtime_spec_name=live["runtime_spec_name"],
            runtime_spec_version=live["runtime_spec_version"],
            runtime_transaction_version=live["runtime_transaction_version"],
            runtime_code=runtime_code,
        )
    )
    args = SimpleNamespace(
        route_allowlist_hash=route_allowlist_hash,
        source_verifier_material_hash=raw_hex(
            source_hashes["source_verifier_material_hash"]
        ),
        source_adapter_engine_deployment_hash=raw_hex(
            source_hashes["source_adapter_engine_deployment_hash"]
        ),
        expected_destination_binding_hash=destination_hash,
        expected_finalized_head=finalized_head,
        expected_runtime_code_hash=runtime_code_hash,
        expected_spec_name="sora2",
        expected_spec_version=1234,
        expected_transaction_version=7,
        route_canary_evidence_hash=route_canary_evidence_hash,
    )
    live_summary = live_module._summary(args, live)
    substrate_toml = live_module.render_toml(args, live)

    assert live_summary["toml_ready"] is True
    assert '# sccp_substrate_finalized_head = "0x' + "55" * 32 + '"' in substrate_toml
    assert '# sccp_substrate_runtime_spec_name = "sora2"' in substrate_toml
    assert '# sccp_substrate_runtime_spec_version = "1234"' in substrate_toml
    assert '# sccp_substrate_runtime_transaction_version = "7"' in substrate_toml
    assert (
        '# sccp_substrate_runtime_code_hash = "0x'
        + runtime_code_hash.hex()
        + '"'
        in substrate_toml
    )
    assert (
        '# sccp_substrate_runtime_code_base64 = "'
        + base64.b64encode(runtime_code).decode("ascii")
        + '"'
        in substrate_toml
    )

    substrate_path = tmp_path / "substrate-live.toml"
    substrate_path.write_text(substrate_toml, encoding="utf-8")
    substrate_records = module.load_evidence_bundle([substrate_path])

    for section in ("sccp_destination_rollouts", "sccp_route_allowlists"):
        records[section] = [
            record
            for record in records[section]
            if record.get("source_domain", record.get("domain")) != domain
        ]
        records[section].extend(substrate_records[section])

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is True
    substrate_lane = next(lane for lane in summary["lanes"] if lane["domain"] == domain)
    assert substrate_lane["blockers"] == []
    assert substrate_lane["destination_binding"]["destination_binding_hash"] == (
        live_summary["destination_binding_hash"]
    )
    assert substrate_lane["route_allowlist"]["route_allowlist_hash"] == (
        "0x" + route_allowlist_hash.hex()
    )


def test_all_lanes_accepts_direct_substrate_destination_toml_with_audited_metadata(
    tmp_path,
):
    module = load_evidence_module()
    records = complete_bundle(module)
    domain = module.SCCP_DOMAIN_SORA2
    substrate_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(domain)
    profile = module.LANE_PROFILES[domain]
    material = records["sccp_source_verifier_materials"][substrate_index]
    deployment = records["sccp_source_adapter_engine_deployments"][substrate_index]
    destination = records["sccp_destination_rollouts"][substrate_index]
    route = records["sccp_route_allowlists"][substrate_index]
    substrate_module = module._load_sibling_module(
        "sccp_substrate_destination_evidence.py"
    )
    source_hashes = module._canonical_source_record_hashes(profile, material, deployment)
    args = SimpleNamespace(
        domain=domain,
        verifier_entrypoint=destination["verifier_identity"],
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
        finalized_head=raw_hex(destination["_comment_substrate_finalized_head"]),
        runtime_spec_name=destination["_comment_substrate_runtime_spec_name"],
        runtime_spec_version=int(destination["_comment_substrate_runtime_spec_version"]),
        runtime_transaction_version=int(
            destination["_comment_substrate_runtime_transaction_version"]
        ),
        runtime_code_base64=substrate_module.parse_runtime_code_base64(
            destination["_comment_substrate_runtime_code_base64"],
            label="runtime code",
        ),
    )

    substrate_toml = substrate_module.render_toml(args)
    assert "# sccp_substrate_finalized_head" in substrate_toml
    assert "# sccp_substrate_runtime_spec_name" in substrate_toml
    assert "# sccp_substrate_runtime_code_hash" in substrate_toml
    assert "# sccp_substrate_runtime_code_base64" in substrate_toml
    substrate_path = tmp_path / "substrate-direct.toml"
    substrate_path.write_text(substrate_toml, encoding="utf-8")
    substrate_records = module.load_evidence_bundle([substrate_path])

    for section in ("sccp_destination_rollouts", "sccp_route_allowlists"):
        records[section] = [
            record
            for record in records[section]
            if record.get("source_domain", record.get("domain")) != domain
        ]
        records[section].extend(substrate_records[section])

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is True
    substrate_lane = next(lane for lane in summary["lanes"] if lane["domain"] == domain)
    assert substrate_lane["blockers"] == []


def test_all_lanes_rejects_substrate_destination_without_live_runtime_metadata():
    module = load_evidence_module()
    records = complete_bundle(module)
    substrate_destination = next(
        record
        for record in records["sccp_destination_rollouts"]
        if record["domain"] == module.SCCP_DOMAIN_SORA2
    )
    substrate_destination.pop("_comment_substrate_finalized_head")
    substrate_destination.pop("_comment_substrate_runtime_spec_name")
    substrate_destination.pop("_comment_substrate_runtime_spec_version")
    substrate_destination.pop("_comment_substrate_runtime_transaction_version")
    substrate_destination.pop("_comment_substrate_runtime_code_hash")
    substrate_destination.pop("_comment_substrate_runtime_code_base64")

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "Substrate finalized head metadata must be a non-zero" in blockers
    assert "Substrate runtime specName metadata is required" in blockers
    assert "Substrate runtime specVersion metadata must be a decimal" in blockers
    assert "Substrate runtime transactionVersion metadata must be a decimal" in blockers
    assert "Substrate runtime code hash metadata must be a non-zero" in blockers
    assert "Substrate runtime code base64 metadata must be present" in blockers


def test_all_lanes_rejects_substrate_destination_with_invalid_runtime_code_base64():
    module = load_evidence_module()
    records = complete_bundle(module)
    substrate_destination = next(
        record
        for record in records["sccp_destination_rollouts"]
        if record["domain"] == module.SCCP_DOMAIN_SORA2
    )
    substrate_destination["_comment_substrate_runtime_code_base64"] = "not base64!"

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert "Substrate runtime code base64 metadata is invalid" in "\n".join(
        summary["blockers"]
    )

    records = complete_bundle(module)
    substrate_destination = next(
        record
        for record in records["sccp_destination_rollouts"]
        if record["domain"] == module.SCCP_DOMAIN_SORA2
    )
    substrate_destination["_comment_substrate_runtime_code_base64"] = (
        noncanonical_base64_alias(b"\x00asm\x01")
    )

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "Substrate runtime code base64 metadata is invalid" in blockers
    assert "canonical base64" in blockers


def test_all_lanes_rejects_substrate_destination_when_runtime_code_hash_drifts():
    module = load_evidence_module()
    records = complete_bundle(module)
    substrate_destination = next(
        record
        for record in records["sccp_destination_rollouts"]
        if record["domain"] == module.SCCP_DOMAIN_SORA2
    )
    substrate_destination["_comment_substrate_runtime_code_base64"] = (
        base64.b64encode(b"\x00asm-drift").decode("ascii")
    )

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert "Substrate runtime code base64 hash must match runtime code hash metadata" in (
        "\n".join(summary["blockers"])
    )


def test_all_lanes_rejects_substrate_destination_with_foreign_runtime_spec_name():
    module = load_evidence_module()
    records = complete_bundle(module)
    substrate_destination = next(
        record
        for record in records["sccp_destination_rollouts"]
        if record["domain"] == module.SCCP_DOMAIN_SORA2
    )
    substrate_destination["_comment_substrate_runtime_spec_name"] = "sora-polkadot"

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert (
        "Substrate runtime specName metadata must match the destination domain sora2"
        in blockers
    )


def test_all_lanes_rejects_substrate_route_canary_runtime_hash_drift():
    module = load_evidence_module()
    records = complete_bundle(module)
    domain = module.SCCP_DOMAIN_SORA2
    substrate_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(domain)
    substrate_route = records["sccp_route_allowlists"][substrate_index]
    drifted = bytearray(
        raw_hex(substrate_route["_comment_route_canary_evidence_hash"])
    )
    drifted[0] ^= 0x01
    substrate_route["_comment_route_canary_evidence_hash"] = (
        "0x" + bytes(drifted).hex()
    )

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    assert (
        "Substrate route canary evidence hash must match finalized runtime metadata"
        in "\n".join(summary["blockers"])
    )


def test_all_lanes_rejects_substrate_route_canary_verifier_code_hash_role_reuse():
    module = load_evidence_module()
    records = complete_bundle(module)
    domain = module.SCCP_DOMAIN_SORA2
    substrate_index = list(module.SCCP_CORE_REMOTE_DOMAINS).index(domain)
    profile = module.LANE_PROFILES[domain]
    material = records["sccp_source_verifier_materials"][substrate_index]
    deployment = records["sccp_source_adapter_engine_deployments"][substrate_index]
    source_hashes = module._canonical_source_record_hashes(
        profile,
        material,
        deployment,
    )
    substrate_destination = records["sccp_destination_rollouts"][substrate_index]
    substrate_destination["verifier_code_hash"] = source_hashes[
        "source_adapter_engine_deployment_hash"
    ]

    summary = module.validate_evidence_bundle(records)

    blockers = "\n".join(summary["blockers"])
    assert summary["production_ready"] is False
    assert (
        "Substrate route canary hash role verifier_code_hash must not reuse "
        "source_adapter_engine_deployment_hash"
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


def test_all_lanes_evidence_recomputes_audit_and_tron_config_hashes():
    module = load_evidence_module()
    records = complete_bundle(module)

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
    assert "domain 4 (ton): ton_full_light_client_gate_hash does not match" in blockers
    assert "domain 5 (tron): TRON DPoS source gate cannot be recomputed" in blockers
    assert "domain 5 (tron): source_bridge_config_hash does not match" in blockers
    lanes = {lane["domain"]: lane for lane in summary["lanes"]}
    sol_gate = lanes[module.SCCP_DOMAIN_SOL]["source_adapter_gate"]
    ton_gate = lanes[module.SCCP_DOMAIN_TON]["source_adapter_gate"]
    tron_gate = lanes[module.SCCP_DOMAIN_TRON]["source_adapter_gate"]
    assert sol_gate["ready"] is False
    assert ton_gate["ready"] is False
    assert tron_gate["ready"] is False
    assert "solana_full_light_client_gate_hash does not match" in "\n".join(
        sol_gate["blockers"]
    )
    assert "ton_full_light_client_gate_hash does not match" in "\n".join(
        ton_gate["blockers"]
    )
    assert "TRON DPoS source gate cannot be recomputed" in "\n".join(
        tron_gate["blockers"]
    )


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
    assert (
        "domain 5 (tron): route_allowlist_hash cannot be recomputed: "
        "source_verifier_material_hash must be a non-zero 32-byte hex value"
        in blockers
    )


def test_all_lanes_evidence_rejects_canonical_source_validator_drift():
    module = load_evidence_module()
    records = complete_bundle(module)

    records["sccp_source_adapter_engine_deployments"][0][
        "adapter_verifier_vk_hash"
    ] = hex32(0xA1)
    substrate_module = module._load_sibling_module("sccp_substrate_source_evidence.py")
    kusama_material = records["sccp_source_verifier_materials"][5]
    profile = module.LANE_PROFILES[module.SCCP_DOMAIN_SORA_KUSAMA]
    kusama_material["source_state_verifier_hash"] = (
        "0x"
        + substrate_module._substrate_template_component_hash(
            profile.domain,
            profile.source_state_verifier_id,
            "source-state-verifier",
        ).hex()
    )
    records["sccp_source_adapter_engine_deployments"][5][
        "source_state_verifier_hash"
    ] = kusama_material["source_state_verifier_hash"]

    summary = module.validate_evidence_bundle(records)

    assert summary["production_ready"] is False
    blockers = "\n".join(summary["blockers"])
    assert "domain 1 (eth): eth source evidence rejected by canonical validator" in blockers
    assert (
        "domain 6 (sora-kusama): sora-kusama source evidence rejected by "
        "canonical validator"
    ) in blockers
    assert "template-derived source state verifier hash is not deployable" in blockers


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
