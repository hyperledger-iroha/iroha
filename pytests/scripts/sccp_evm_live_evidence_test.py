import json
import copy
import hashlib
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
from types import SimpleNamespace


EVM_SOURCE_VERIFIER_MATERIAL_HASH = "aa" * 32
EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH = "99" * 32
ETH_MAINNET_NETWORK_ID = "00" * 31 + "01"
EVM_LIVE_ROUTE_ALLOWLIST_HASH_VECTOR = (
    "06d0aa09a6d3613931fd1cdb7885fc94e82e12197802020bc1d137cf81dcda5b"
)
EVM_ROUTE_CANARY_EVIDENCE_HASH = "e1" * 32


def load_live_module():
    script_path = (
        Path(__file__).resolve().parents[2] / "scripts" / "sccp_evm_live_evidence.py"
    )
    spec = spec_from_file_location("sccp_evm_live_evidence", script_path)
    module = module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


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


class RawResponse:
    def __init__(self, payload):
        self.payload = payload

    def __enter__(self):
        return self

    def __exit__(self, _exc_type, _exc, _traceback):
        return False

    def read(self, size=-1):
        if size is None or size < 0:
            return self.payload
        return self.payload[:size]


class OversizedResponse:
    def __enter__(self):
        return self

    def __exit__(self, _exc_type, _exc, _traceback):
        return False

    def read(self, size=-1):
        if size is None or size < 0:
            size = 1024 * 1024 + 1
        return b"{" * size


class OversizedErrorBody:
    def read(self, size=-1):
        if size is None or size < 0:
            size = 4097
        return b"evm-error" * size

    def close(self):
        return None


def abi_word_u32(value):
    return value.to_bytes(32, "big")


def abi_word_address(address):
    return b"\x00" * 12 + address


def evm_route_canary_submit_call_data(
    module,
    *,
    message_id=bytes.fromhex("55" * 32),
    source_domain=0,
    target_domain=1,
    commitment_root=bytes.fromhex("66" * 32),
    statement_hash=bytes.fromhex("77" * 32),
    payload_hash=bytes.fromhex("88" * 32),
    finality_height=123,
    finality_block_hash=bytes.fromhex("99" * 32),
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
    call_data = bytearray(module.EVM_SUBMIT_MESSAGE_PROOF_SELECTOR)
    call_data.extend(abi_word_u32(32 * 8))
    for word in public_inputs:
        call_data.extend(word)
    call_data.extend(statement_hash)
    call_data.extend(abi_word_u32(len(proof_bytes)))
    call_data.extend(proof_bytes)
    return bytes(call_data)


def fake_opener_for(
    module,
    *,
    rpc_chain_id=1,
    source_domain=0,
    target_domain=1,
    network_id_override=None,
    verifier_code_hash_override=None,
    backend_hash_override=None,
    destination_binding_override=None,
    verifier_equals_bridge=False,
    route_canary_used=True,
    route_canary_destination_binding_override=None,
    route_canary_wrong_selector=False,
    route_canary_receipt_block_number=None,
    route_canary_block_response_hash=None,
    route_canary_block_response_number=None,
    route_canary_block_receipts_root=None,
    duplicate_route_canary_log=False,
):
    bridge = "0x" + "11" * 20
    verifier_address_bytes = (
        bytes.fromhex("11" * 20)
        if verifier_equals_bridge
        else bytes.fromhex("22" * 20)
    )
    verifier = "0x" + verifier_address_bytes.hex()
    network_id = module.evidence.evm_mainnet_network_id_for_domain(target_domain)
    if network_id_override is not None:
        network_id = network_id_override
    bridge_runtime = bytes.fromhex("60806040526001")
    verifier_runtime = bytes.fromhex("60806040526002")
    verifier_code_hash = module.evidence.runtime_bytecode_hash(verifier_runtime)
    if verifier_code_hash_override is not None:
        verifier_code_hash = verifier_code_hash_override
    verifier_key_hash = bytes.fromhex("cc" * 32)
    backend_hash = module.evidence._keccak_256(
        module.evidence.SCCP_EVM_GROTH16_BACKEND.encode("utf-8")
    )
    if backend_hash_override is not None:
        backend_hash = backend_hash_override
    family_hash = module.evidence._keccak_256(
        module.evidence.SCCP_PROOF_FAMILY_STARK_FRI.encode("utf-8")
    )
    destination_binding = (
        bytes.fromhex("44" * 32)
        if (
            verifier_equals_bridge
            or source_domain != module.evidence.SCCP_DOMAIN_SORA
            or network_id_override is not None
        )
        else module.evidence.evm_destination_binding_hash(
            network_id=network_id,
            source_domain=source_domain,
            target_domain=target_domain,
            verifier_address=verifier_address_bytes,
            bridge_address=bytes.fromhex("11" * 20),
            verifier_code_hash=module.evidence.runtime_bytecode_hash(verifier_runtime),
            verifier_key_hash=verifier_key_hash,
        )
    )
    observed_destination_binding = (
        destination_binding_override
        if destination_binding_override is not None
        else destination_binding
    )
    bridge_code_hash = module.evidence.runtime_bytecode_hash(bridge_runtime)
    route_canary_transaction_hash = bytes.fromhex("44" * 32)
    route_canary_message_id = bytes.fromhex("55" * 32)
    route_canary_commitment_root = bytes.fromhex("66" * 32)
    route_canary_statement_hash = bytes.fromhex("77" * 32)
    route_canary_payload_hash = bytes.fromhex("88" * 32)
    route_canary_finality_height = abi_word_u32(123)
    route_canary_finality_block_hash = bytes.fromhex("99" * 32)
    route_canary_receipt_block_hash = "0x" + "aa" * 32
    route_canary_receipt_block_number = (
        route_canary_receipt_block_number or "0x1234"
    )
    route_canary_block_response_hash = (
        route_canary_block_response_hash or route_canary_receipt_block_hash
    )
    route_canary_block_response_number = (
        route_canary_block_response_number or route_canary_receipt_block_number
    )
    route_canary_block_receipts_root = route_canary_block_receipts_root or (
        "0x" + "bb" * 32
    )
    route_canary_call_data = evm_route_canary_submit_call_data(
        module,
        message_id=route_canary_message_id,
        target_domain=target_domain,
        commitment_root=route_canary_commitment_root,
        statement_hash=route_canary_statement_hash,
        payload_hash=route_canary_payload_hash,
        finality_height=123,
        finality_block_hash=route_canary_finality_block_hash,
    )
    if route_canary_wrong_selector:
        route_canary_call_data = b"\x12\x34\x56\x78" + route_canary_call_data[4:]
    route_canary_log_destination_binding = (
        route_canary_destination_binding_override
        if route_canary_destination_binding_override is not None
        else destination_binding
    )
    route_canary_log = {
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
                    route_canary_log_destination_binding,
                    backend_hash,
                    family_hash,
                    network_id,
                )
            ).hex()
        ),
    }
    route_canary_logs = [route_canary_log]
    if duplicate_route_canary_log:
        route_canary_logs.append(dict(route_canary_log))
    call_words = {
        (bridge, "verifier()"): abi_word_address(verifier_address_bytes),
        (bridge, "verifierCodeHash()"): verifier_code_hash,
        (bridge, "verifierKeyHash()"): verifier_key_hash,
        (bridge, "verifierBackendHash()"): backend_hash,
        (bridge, "proofFamilyHash()"): family_hash,
        (bridge, "networkId()"): network_id,
        (bridge, "expectedSourceDomain()"): abi_word_u32(source_domain),
        (bridge, "expectedTargetDomain()"): abi_word_u32(target_domain),
        (bridge, "destinationBindingHash()"): observed_destination_binding,
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
                {"jsonrpc": "2.0", "id": 1, "result": hex(rpc_chain_id)}
            )
        if method == "eth_getCode":
            address = params[0].lower()
            if address == bridge:
                return FakeResponse(
                    {"jsonrpc": "2.0", "id": 1, "result": "0x" + bridge_runtime.hex()}
                )
            if address == verifier:
                return FakeResponse(
                    {
                        "jsonrpc": "2.0",
                        "id": 1,
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
                        "id": 1,
                        "result": "0x" + abi_word_u32(1 if route_canary_used else 0).hex(),
                    }
                )
            signature = selectors[data]
            return FakeResponse(
                {
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": "0x" + call_words[(address, signature)].hex(),
                }
            )
        if method == "eth_getTransactionReceipt":
            assert params[0] == "0x" + route_canary_transaction_hash.hex()
            return FakeResponse(
                {
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": {
                        "transactionHash": "0x" + route_canary_transaction_hash.hex(),
                        "status": "0x1",
                        "blockHash": route_canary_receipt_block_hash,
                        "blockNumber": route_canary_receipt_block_number,
                        "logs": route_canary_logs,
                    },
                }
            )
        if method == "eth_getBlockByNumber":
            assert params == [route_canary_receipt_block_number, False]
            return FakeResponse(
                {
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": {
                        "hash": route_canary_block_response_hash,
                        "number": route_canary_block_response_number,
                        "receiptsRoot": route_canary_block_receipts_root,
                    },
                }
            )
        if method == "eth_getTransactionByHash":
            assert params[0] == "0x" + route_canary_transaction_hash.hex()
            return FakeResponse(
                {
                    "jsonrpc": "2.0",
                    "id": 1,
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
        rpc_chain_id=rpc_chain_id,
        bridge_code_hash=bridge_code_hash,
        verifier_code_hash=module.evidence.runtime_bytecode_hash(verifier_runtime),
        verifier_key_hash=verifier_key_hash,
        destination_binding=destination_binding,
        route_canary_transaction_hash=route_canary_transaction_hash,
        route_canary_log_index=0,
        route_canary_receipt_block_number=int(route_canary_receipt_block_number, 16),
        route_canary_receipt_block_hash=bytes.fromhex(
            route_canary_receipt_block_hash.removeprefix("0x")
        ),
        route_canary_block_receipts_root=bytes.fromhex(
            route_canary_block_receipts_root.removeprefix("0x")
        ),
        route_canary_call_data_sha256=hashlib.sha256(route_canary_call_data).digest(),
        route_canary_message_id=route_canary_message_id,
        route_canary_payload_hash=route_canary_payload_hash,
        route_canary_target_domain=target_domain,
        route_canary_statement_hash=route_canary_statement_hash,
        route_canary_commitment_root=route_canary_commitment_root,
        route_canary_finality_height=route_canary_finality_height,
        route_canary_finality_block_hash=route_canary_finality_block_hash,
        route_canary_proof_version=1,
        route_canary_proof_source_domain=source_domain,
    )


def route_canary_hash_for(module, fake, route_allowlist_hash):
    return module.evidence.evm_route_canary_transaction_evidence_hash(
        route_allowlist_hash=route_allowlist_hash,
        bridge_address=bytes.fromhex(fake.bridge.removeprefix("0x")),
        transaction_hash=fake.route_canary_transaction_hash,
        log_index=fake.route_canary_log_index,
        receipt_block_number=fake.route_canary_receipt_block_number,
        receipt_block_hash=fake.route_canary_receipt_block_hash,
        block_receipts_root=fake.route_canary_block_receipts_root,
        call_data_sha256=fake.route_canary_call_data_sha256,
        message_id=fake.route_canary_message_id,
        payload_hash=fake.route_canary_payload_hash,
        source_domain=module.evidence.SCCP_DOMAIN_SORA,
        target_domain=fake.route_canary_target_domain,
        commitment_root=fake.route_canary_commitment_root,
        finality_height=fake.route_canary_finality_height,
        finality_block_hash=fake.route_canary_finality_block_hash,
        statement_hash=fake.route_canary_statement_hash,
        proof_version=fake.route_canary_proof_version,
        proof_source_domain=fake.route_canary_proof_source_domain,
        destination_binding_hash=fake.destination_binding,
        verifier_backend_hash=module.evidence.evm_verifier_backend_hash(),
        proof_family_hash=module.evidence.evm_proof_family_hash(),
        network_id=fake.network_id,
        used_message_proof=True,
    )


def test_evm_json_rpc_response_size_is_bounded():
    module = load_live_module()

    def oversized_opener(_request, timeout):
        del timeout
        return OversizedResponse()

    try:
        module._json_rpc(
            "https://ethereum.example",
            "eth_chainId",
            [],
            opener=oversized_opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        assert "response exceeds" in str(exc)
    else:
        raise AssertionError("oversized EVM JSON-RPC response was accepted")


def test_evm_json_rpc_http_error_detail_is_bounded():
    module = load_live_module()

    def failing_opener(request, timeout):
        del timeout
        raise module.urllib.error.HTTPError(
            request.full_url,
            502,
            "bad gateway",
            {},
            OversizedErrorBody(),
        )

    try:
        module._json_rpc(
            "https://ethereum.example",
            "eth_chainId",
            [],
            opener=failing_opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        message = str(exc)
        assert "HTTP 502" in message
        assert "...<truncated>" in message
        assert len(message) < 4300
    else:
        raise AssertionError("oversized EVM JSON-RPC error body was accepted")


def test_evm_json_rpc_rejects_duplicate_json_keys():
    module = load_live_module()
    duplicate_payload = b'{"jsonrpc":"2.0","id":1,"result":"0x1","result":"0x2"}'

    def duplicate_json_opener(_request, timeout):
        del timeout
        return RawResponse(duplicate_payload)

    try:
        module._json_rpc(
            "https://ethereum.example",
            "eth_chainId",
            [],
            opener=duplicate_json_opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        assert "duplicate JSON key" in str(exc)
    else:
        raise AssertionError("duplicate-key EVM JSON-RPC response was accepted")


def test_evm_json_rpc_rejects_envelope_drift():
    module = load_live_module()

    cases = (
        (
            {"jsonrpc": "2.0", "id": 2, "result": "0x38"},
            "response id",
            "mismatched JSON-RPC id was accepted",
        ),
        (
            {"jsonrpc": "2.0", "id": "1", "result": "0x38"},
            "response id",
            "string JSON-RPC id was accepted",
        ),
        (
            {"id": 1, "result": "0x38"},
            "protocol version",
            "missing JSON-RPC protocol version was accepted",
        ),
        (
            {"jsonrpc": "2.0 ", "id": 1, "result": "0x38"},
            "protocol version",
            "padded JSON-RPC protocol version was accepted",
        ),
    )

    for payload, expected_message, failure in cases:
        def opener(_request, timeout, payload=payload):
            del timeout
            return FakeResponse(payload)

        try:
            module._json_rpc(
                "https://bsc.example",
                "eth_chainId",
                [],
                opener=opener,
                timeout=3.0,
            )
        except RuntimeError as exc:
            assert expected_message in str(exc)
        else:
            raise AssertionError(failure)


def test_live_evm_evidence_collects_destination_and_offline_toml():
    module = load_live_module()
    fake = fake_opener_for(module)
    route_allowlist_hash = bytes.fromhex(EVM_LIVE_ROUTE_ALLOWLIST_HASH_VECTOR)
    route_canary_hash = route_canary_hash_for(module, fake, route_allowlist_hash)

    summary = module.collect_live_evidence(
        SimpleNamespace(
            rpc_url="https://ethereum.example",
            domain=module.evidence.SCCP_DOMAIN_ETH,
            bridge_address=fake.bridge,
            expected_network_id=fake.network_id,
            expected_bridge_code_hash=fake.bridge_code_hash,
            expected_destination_binding_hash=fake.destination_binding,
            route_allowlist_hash=route_allowlist_hash,
            route_canary_evidence_hash=route_canary_hash,
            route_canary_transaction_hash=fake.route_canary_transaction_hash,
            route_canary_log_index=fake.route_canary_log_index,
            source_verifier_material_hash=bytes.fromhex(
                EVM_SOURCE_VERIFIER_MATERIAL_HASH
            ),
            source_adapter_engine_deployment_hash=bytes.fromhex(
                EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
            ),
            block_tag="finalized",
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    destination = summary["destination_bridge"]
    assert summary["read_only"] is True
    assert summary["block_tag"] == "finalized"
    assert destination["chain"] == "eth"
    assert destination["rpc_chain_id"] == 1
    assert destination["expected_rpc_chain_id"] == 1
    assert destination["expected_rpc_chain_id_matches"] is True
    assert destination["bridge_address"] == fake.bridge
    assert destination["verifier_address"] == fake.verifier
    assert destination["network_id"] == "0x" + ETH_MAINNET_NETWORK_ID
    assert destination["bridge_code_hash"] == "0x" + fake.bridge_code_hash.hex()
    assert destination["expected_bridge_code_hash_matches"] is True
    assert destination["verifier_code_hash"] == "0x" + fake.verifier_code_hash.hex()
    assert destination["verifier_key_hash"] == "0x" + "cc" * 32
    assert destination["destination_binding_key"].startswith("evm:0:1:")
    assert destination["destination_binding_hash"] == (
        "0x" + fake.destination_binding.hex()
    )
    assert destination["destination_binding_hash_matches_bridge_view"] is True
    assert destination["expected_network_id_matches"] is True
    assert destination["expected_destination_binding_hash_matches"] is True
    assert summary["torii_destination_query_params"] == {
        "network_id_hex": "0x" + ETH_MAINNET_NETWORK_ID,
        "verifier_address_hex": fake.verifier,
        "bridge_address_hex": fake.bridge,
        "verifier_code_hash_hex": "0x" + fake.verifier_code_hash.hex(),
        "verifier_key_hash_hex": "0x" + "cc" * 32,
        "expected_destination_binding_hash_hex": "0x" + fake.destination_binding.hex(),
    }
    assert summary["torii_destination_query_proof_bytes_hex_required"] is True
    offline_args = summary["offline_evidence_args"]
    assert "--expected-destination-binding-hash" in offline_args
    assert "--route-allowlist-hash" in offline_args
    assert "--route-canary-evidence-hash" in offline_args
    assert "--route-canary-transaction-hash" in offline_args
    assert "--route-canary-call-data-sha256" in offline_args
    assert "--route-canary-payload-hash" in offline_args
    assert "--route-canary-finality-height" in offline_args
    assert "--route-canary-finality-block-hash" in offline_args
    assert "--route-canary-proof-version" in offline_args
    assert "--route-canary-proof-source-domain" in offline_args
    assert "--source-verifier-material-hash" in offline_args
    assert "--source-adapter-engine-deployment-hash" in offline_args
    assert summary["expected_route_allowlist_hash"] == (
        "0x" + EVM_LIVE_ROUTE_ALLOWLIST_HASH_VECTOR
    )
    assert summary["expected_route_allowlist_hash_matches"] is True
    assert summary["route_canary"]["evidence_hash"] == (
        "0x" + route_canary_hash.hex()
    )
    assert summary["route_canary"]["evidence_source"] == (
        "evm_message_proof_accepted_transaction"
    )
    assert summary["route_canary_transaction"]["message_proof_used"] is True
    assert summary["route_canary_transaction"]["call_data_sha256"] == (
        "0x" + fake.route_canary_call_data_sha256.hex()
    )
    assert summary["route_canary_transaction"]["public_inputs_payload_hash"] == (
        "0x" + fake.route_canary_payload_hash.hex()
    )
    assert summary["route_canary_transaction"]["public_inputs_finality_height"] == (
        "0x" + fake.route_canary_finality_height.hex()
    )
    assert summary["route_canary_transaction"]["public_inputs_finality_block_hash"] == (
        "0x" + fake.route_canary_finality_block_hash.hex()
    )
    assert summary["route_canary_transaction"]["receipt_block_matches"] is True
    assert summary["route_canary_transaction"]["block_receipts_root"] == "0x" + "bb" * 32
    assert summary["source_record_hashes"] == {
        "source_verifier_material_hash": "0x" + EVM_SOURCE_VERIFIER_MATERIAL_HASH,
        "source_adapter_engine_deployment_hash": (
            "0x" + EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
        ),
    }

    rendered = module.render_offline_toml(summary)
    assert '# sccp_evm_rpc_chain_id = "1"' in rendered
    assert (
        '# sccp_evm_bridge_runtime_code_hash = "0x'
        + fake.bridge_code_hash.hex()
        + '"'
        in rendered
    )
    assert (
        '# sccp_evm_verifier_runtime_code_hash = "0x'
        + fake.verifier_code_hash.hex()
        + '"'
        in rendered
    )
    assert '# sccp_evm_verifier_key_hash = "0x' + "cc" * 32 + '"' in rendered
    assert (
        '# sccp_evm_verifier_backend_hash = "0x'
        + module.evidence.evm_verifier_backend_hash().hex()
        + '"'
        in rendered
    )
    assert (
        '# sccp_evm_proof_family_hash = "0x'
        + module.evidence.evm_proof_family_hash().hex()
        + '"'
        in rendered
    )
    assert '# sccp_evm_destination_network_id = "0x' + ETH_MAINNET_NETWORK_ID + '"' in rendered
    assert "# sccp_evm_destination_binding_key = " in rendered
    assert 'destination_binding_key = "evm:0:1:' in rendered
    assert '# sccp_evm_destination_binding_hash = "0x' in rendered
    assert (
        '# sccp_route_canary_evidence_hash = "0x'
        + route_canary_hash.hex()
        + '"'
        in rendered
    )
    assert "# sccp_evm_route_canary_transaction_hash" in rendered
    assert "# sccp_evm_route_canary_call_data_sha256" in rendered
    assert "# sccp_evm_route_canary_payload_hash" in rendered
    assert "# sccp_evm_route_canary_finality_height" in rendered
    assert "# sccp_evm_route_canary_finality_block_hash" in rendered
    assert "# sccp_evm_route_canary_proof_version" in rendered
    assert "# sccp_evm_route_canary_proof_source_domain" in rendered
    assert "evm_route_canary_transaction_hash = " in rendered
    for key in (
        "# sccp_evm_rpc_chain_id = ",
        "# sccp_evm_bridge_runtime_code_hash = ",
        "# sccp_evm_verifier_runtime_code_hash = ",
        "# sccp_evm_verifier_key_hash = ",
        "# sccp_evm_verifier_backend_hash = ",
        "# sccp_evm_proof_family_hash = ",
    ):
        assert rendered.count(key) == 1
    assert 'chain = "eth"' in rendered
    assert (
        'route_allowlist_hash = "0x'
        + EVM_LIVE_ROUTE_ALLOWLIST_HASH_VECTOR
        + '"'
        in rendered
    )


def test_live_evm_evidence_rejects_aliased_verifier_and_bridge():
    module = load_live_module()
    fake = fake_opener_for(module, verifier_equals_bridge=True)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                rpc_url="https://ethereum.example",
                domain=module.evidence.SCCP_DOMAIN_ETH,
                bridge_address=fake.bridge,
                expected_network_id=fake.network_id,
                expected_bridge_code_hash=None,
                expected_destination_binding_hash=None,
                route_allowlist_hash=None,
                route_canary_evidence_hash=None,
                source_verifier_material_hash=None,
                source_adapter_engine_deployment_hash=None,
                block_tag="finalized",
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "destination verifier address must differ from bridge address" in str(exc)
    else:
        raise AssertionError("aliased EVM destination verifier and bridge were accepted")


def test_live_evm_diagnostic_offline_args_do_not_self_pin_binding():
    module = load_live_module()
    fake = fake_opener_for(module)

    summary = module.collect_live_evidence(
        SimpleNamespace(
            rpc_url="https://ethereum.example",
            domain=module.evidence.SCCP_DOMAIN_ETH,
            bridge_address=fake.bridge,
            expected_network_id=None,
            expected_bridge_code_hash=None,
            expected_destination_binding_hash=None,
            route_allowlist_hash=None,
            source_verifier_material_hash=None,
            source_adapter_engine_deployment_hash=None,
            block_tag="latest",
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    offline_args = summary["offline_evidence_args"]
    assert "--expected-destination-binding-hash" not in offline_args
    assert "torii_destination_query_params" not in summary
    assert "torii_destination_query_proof_bytes_hex_required" not in summary
    assert "offline_toml_sha256" not in summary


def test_live_evm_full_toml_requires_route_canary_evidence():
    module = load_live_module()
    fake = fake_opener_for(module)

    summary = module.collect_live_evidence(
        SimpleNamespace(
            rpc_url="https://ethereum.example",
            domain=module.evidence.SCCP_DOMAIN_ETH,
            bridge_address=fake.bridge,
            expected_network_id=fake.network_id,
            expected_bridge_code_hash=fake.bridge_code_hash,
            expected_destination_binding_hash=fake.destination_binding,
            route_allowlist_hash=bytes.fromhex(EVM_LIVE_ROUTE_ALLOWLIST_HASH_VECTOR),
            route_canary_evidence_hash=None,
            source_verifier_material_hash=bytes.fromhex(
                EVM_SOURCE_VERIFIER_MATERIAL_HASH
            ),
            source_adapter_engine_deployment_hash=bytes.fromhex(
                EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
            ),
            block_tag="latest",
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    assert "route_canary" not in summary
    assert "offline_toml_sha256" not in summary
    try:
        module.render_offline_toml(summary)
    except ValueError as exc:
        assert "route-canary-evidence-hash" in str(exc)
    else:
        raise AssertionError("full EVM TOML rendered without route canary evidence")


def test_live_evm_route_canary_rejects_unverified_transaction_metadata():
    module = load_live_module()

    def collect_with(fake, *, evidence_hash=None):
        route_allowlist_hash = bytes.fromhex(EVM_LIVE_ROUTE_ALLOWLIST_HASH_VECTOR)
        if evidence_hash is None:
            try:
                evidence_hash = route_canary_hash_for(module, fake, route_allowlist_hash)
            except ValueError as exc:
                if "block_receipts_root" not in str(exc):
                    raise
                evidence_hash = bytes.fromhex("e1" * 32)
        return module.collect_live_evidence(
            SimpleNamespace(
                rpc_url="https://ethereum.example",
                domain=module.evidence.SCCP_DOMAIN_ETH,
                bridge_address=fake.bridge,
                expected_network_id=fake.network_id,
                expected_bridge_code_hash=fake.bridge_code_hash,
                expected_destination_binding_hash=fake.destination_binding,
                route_allowlist_hash=route_allowlist_hash,
                route_canary_evidence_hash=evidence_hash,
                route_canary_transaction_hash=fake.route_canary_transaction_hash,
                route_canary_log_index=fake.route_canary_log_index,
                source_verifier_material_hash=bytes.fromhex(
                    EVM_SOURCE_VERIFIER_MATERIAL_HASH
                ),
                source_adapter_engine_deployment_hash=bytes.fromhex(
                    EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
                ),
                block_tag="latest",
                timeout=1.0,
            ),
            opener=fake.opener,
        )

    placeholder_hash = bytes.fromhex("e1" * 32)
    for fake, expected_message, evidence_hash in (
        (
            fake_opener_for(module, route_canary_used=False),
            "usedMessageProofs(bytes32) is false",
            None,
        ),
        (
            fake_opener_for(module, route_canary_wrong_selector=True),
            "submitSccpMessageProof",
            None,
        ),
        (
            fake_opener_for(
                module,
                route_canary_destination_binding_override=bytes.fromhex("ab" * 32),
            ),
            "destinationBindingHash",
            None,
        ),
        (
            fake_opener_for(module, route_canary_receipt_block_number="0x0"),
            "receipt blockNumber must be non-zero",
            placeholder_hash,
        ),
        (
            fake_opener_for(
                module,
                route_canary_block_response_hash="0x" + "ab" * 32,
            ),
            "block hash does not match receipt blockHash",
            None,
        ),
        (
            fake_opener_for(
                module,
                route_canary_block_receipts_root="0x" + "00" * 32,
            ),
            "block receiptsRoot must not be zero",
            placeholder_hash,
        ),
        (
            fake_opener_for(module, duplicate_route_canary_log=True),
            "more than one matching MessageProofAccepted",
            None,
        ),
    ):
        try:
            collect_with(fake, evidence_hash=evidence_hash)
        except RuntimeError as exc:
            assert expected_message in str(exc)
        else:
            raise AssertionError("unverified EVM route canary transaction was accepted")

    fake = fake_opener_for(module)
    try:
        collect_with(fake, evidence_hash=bytes.fromhex("e1" * 32))
    except ValueError as exc:
        assert "MessageProofAccepted transaction evidence hash" in str(exc)
    else:
        raise AssertionError("forged EVM route canary evidence hash was accepted")


def test_live_evm_full_toml_revalidates_imported_summary_metadata():
    module = load_live_module()
    fake = fake_opener_for(module)
    route_allowlist_hash = bytes.fromhex(EVM_LIVE_ROUTE_ALLOWLIST_HASH_VECTOR)
    route_canary_hash = route_canary_hash_for(module, fake, route_allowlist_hash)
    summary = module.collect_live_evidence(
        SimpleNamespace(
            rpc_url="https://ethereum.example",
            domain=module.evidence.SCCP_DOMAIN_ETH,
            bridge_address=fake.bridge,
            expected_network_id=fake.network_id,
            expected_bridge_code_hash=fake.bridge_code_hash,
            expected_destination_binding_hash=fake.destination_binding,
            route_allowlist_hash=route_allowlist_hash,
            route_canary_evidence_hash=route_canary_hash,
            route_canary_transaction_hash=fake.route_canary_transaction_hash,
            route_canary_log_index=fake.route_canary_log_index,
            source_verifier_material_hash=bytes.fromhex(
                EVM_SOURCE_VERIFIER_MATERIAL_HASH
            ),
            source_adapter_engine_deployment_hash=bytes.fromhex(
                EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
            ),
            block_tag="latest",
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    for field, forged_value, expected_message in (
        ("rpc_chain_id", 56, "RPC chain id"),
        ("verifier_backend_hash", "0x" + "bb" * 32, "backend"),
        ("bridge_runtime_bytecode_hex", "0x6001", "bridge runtime"),
        ("expected_network_id", "0x" + "44" * 32, "expected network id"),
    ):
        forged = copy.deepcopy(summary)
        forged["destination_bridge"][field] = forged_value
        try:
            module.render_offline_toml(forged)
        except ValueError as exc:
            assert expected_message in str(exc)
        else:
            raise AssertionError(
                f"EVM full TOML accepted forged {field} summary metadata"
            )

    forged = copy.deepcopy(summary)
    forged["route_canary_transaction"]["receipt_block_matches"] = False
    try:
        module.render_offline_toml(forged)
    except ValueError as exc:
        assert "route-canary-transaction-hash" in str(exc)
    else:
        raise AssertionError("EVM full TOML accepted unverified route-canary block")


def test_live_evm_diagnostic_offline_args_withhold_route_until_binding_pin():
    module = load_live_module()
    fake = fake_opener_for(module)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                rpc_url="https://ethereum.example",
                domain=module.evidence.SCCP_DOMAIN_ETH,
                bridge_address=fake.bridge,
                expected_network_id=fake.network_id,
                expected_bridge_code_hash=fake.bridge_code_hash,
                expected_destination_binding_hash=None,
                route_allowlist_hash=bytes.fromhex(EVM_LIVE_ROUTE_ALLOWLIST_HASH_VECTOR),
                source_verifier_material_hash=bytes.fromhex(
                    EVM_SOURCE_VERIFIER_MATERIAL_HASH
                ),
                source_adapter_engine_deployment_hash=bytes.fromhex(
                    EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
                ),
                block_tag="latest",
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "route-allowlist-hash requires --expected-destination-binding-hash" in str(exc)
    else:
        raise AssertionError("EVM route hash accepted an unpinned destination binding")


def test_live_evm_evidence_rejects_missing_route_source_hashes_and_drift():
    module = load_live_module()
    fake = fake_opener_for(module)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                rpc_url="https://ethereum.example",
                domain=module.evidence.SCCP_DOMAIN_ETH,
                bridge_address=fake.bridge,
                expected_network_id=None,
                expected_bridge_code_hash=None,
                expected_destination_binding_hash=None,
                route_allowlist_hash=bytes.fromhex(
                    EVM_LIVE_ROUTE_ALLOWLIST_HASH_VECTOR
                ),
                block_tag="latest",
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "--source-verifier-material-hash" in str(exc)
    else:
        raise AssertionError("EVM route hash without source records was accepted")

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                rpc_url="https://ethereum.example",
                domain=module.evidence.SCCP_DOMAIN_ETH,
                bridge_address=fake.bridge,
                expected_network_id=None,
                expected_bridge_code_hash=None,
                expected_destination_binding_hash=fake.destination_binding,
                route_allowlist_hash=bytes.fromhex("dd" * 32),
                source_verifier_material_hash=bytes.fromhex(
                    EVM_SOURCE_VERIFIER_MATERIAL_HASH
                ),
                source_adapter_engine_deployment_hash=bytes.fromhex(
                    EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
                ),
                block_tag="latest",
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "canonical source, deployment, and destination evidence" in str(exc)
    else:
        raise AssertionError("drifted live EVM route allowlist hash was accepted")


def test_live_evm_evidence_rejects_verifier_code_hash_drift():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        verifier_code_hash_override=bytes.fromhex("aa" * 32),
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                rpc_url="https://ethereum.example",
                domain=module.evidence.SCCP_DOMAIN_ETH,
                bridge_address=fake.bridge,
                expected_network_id=None,
                expected_bridge_code_hash=None,
                expected_destination_binding_hash=None,
                route_allowlist_hash=None,
                block_tag="latest",
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "verifierCodeHash" in str(exc)
    else:
        raise AssertionError("mismatched verifier code hash was accepted")


def test_live_evm_evidence_rejects_bridge_code_hash_drift():
    module = load_live_module()
    fake = fake_opener_for(module)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                rpc_url="https://ethereum.example",
                domain=module.evidence.SCCP_DOMAIN_ETH,
                bridge_address=fake.bridge,
                expected_network_id=None,
                expected_bridge_code_hash=bytes.fromhex("ab" * 32),
                expected_destination_binding_hash=None,
                route_allowlist_hash=None,
                block_tag="latest",
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "expected-bridge-code-hash" in str(exc)
    else:
        raise AssertionError("mismatched bridge code hash was accepted")


def test_live_evm_evidence_rejects_bridge_destination_binding_drift():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        destination_binding_override=bytes.fromhex("ab" * 32),
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                rpc_url="https://ethereum.example",
                domain=module.evidence.SCCP_DOMAIN_ETH,
                bridge_address=fake.bridge,
                expected_network_id=None,
                expected_bridge_code_hash=None,
                expected_destination_binding_hash=None,
                route_allowlist_hash=None,
                block_tag="latest",
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "destinationBindingHash" in str(exc)
    else:
        raise AssertionError("mismatched bridge destination binding was accepted")


def test_live_evm_evidence_rejects_rpc_chain_id_drift():
    module = load_live_module()
    fake = fake_opener_for(module, rpc_chain_id=56)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                rpc_url="https://ethereum.example",
                domain=module.evidence.SCCP_DOMAIN_ETH,
                bridge_address=fake.bridge,
                expected_rpc_chain_id=None,
                expected_network_id=None,
                expected_bridge_code_hash=None,
                expected_destination_binding_hash=None,
                route_allowlist_hash=None,
                block_tag="latest",
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "expected-rpc-chain-id" in str(exc)
        assert "expected 1, got 56" in str(exc)
    else:
        raise AssertionError("wrong EVM RPC chain id was accepted")


def test_live_evm_evidence_rejects_noncanonical_expected_rpc_chain_id():
    module = load_live_module()
    fake = fake_opener_for(module, rpc_chain_id=56)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                rpc_url="https://bsc.example",
                domain=module.evidence.SCCP_DOMAIN_ETH,
                bridge_address=fake.bridge,
                expected_rpc_chain_id=56,
                expected_network_id=None,
                expected_bridge_code_hash=None,
                expected_destination_binding_hash=None,
                route_allowlist_hash=None,
                block_tag="latest",
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "canonical eth mainnet chain id 1" in str(exc)
    else:
        raise AssertionError("noncanonical explicit EVM RPC chain id was accepted")


def test_live_evm_expected_rpc_chain_id_parser_requires_canonical_decimal():
    module = load_live_module()

    assert module._parse_rpc_chain_id("1") == 1
    assert module._rpc_quantity("0xa", method="eth_test") == 10
    assert module._rpc_hex_data("0x6000", method="eth_test") == bytes.fromhex("6000")

    for value in ("01", "0x1", "+1", " 1 ", "١"):
        try:
            module._parse_rpc_chain_id(value)
        except module.argparse.ArgumentTypeError as exc:
            assert "expected-rpc-chain-id" in str(exc)
        else:
            raise AssertionError(f"noncanonical EVM chain id {value!r} was accepted")

    for value, parser, expected in (
        ("0x1 ", module._rpc_quantity, "non-canonical"),
        ("0x01", module._rpc_quantity, "non-canonical"),
        ("0xA", module._rpc_quantity, "non-canonical"),
        (" 0x6000", module._rpc_hex_data, "non-canonical"),
        ("6000", module._rpc_hex_data, "lowercase 0x hex"),
        ("0X6000", module._rpc_hex_data, "lowercase 0x hex"),
        ("0xABCD", module._rpc_hex_data, "lowercase 0x hex"),
    ):
        try:
            parser(value, method="eth_test")
        except RuntimeError as exc:
            assert expected in str(exc)
        else:
            raise AssertionError(f"padded EVM RPC result {value!r} was accepted")

    for value in ("44" * 32, "0X" + "44" * 32, "0x" + "AA" * 32):
        try:
            module._parse_exact_hex32_blob(value, label="route-canary receipt blockHash")
        except RuntimeError as exc:
            assert "canonical lowercase 0x hex" in str(exc)
        else:
            raise AssertionError(f"noncanonical EVM exact hex {value!r} was accepted")


def test_live_evm_full_toml_requires_expected_bridge_code_hash():
    module = load_live_module()
    fake = fake_opener_for(module)

    summary = module.collect_live_evidence(
        SimpleNamespace(
            rpc_url="https://ethereum.example",
            domain=module.evidence.SCCP_DOMAIN_ETH,
            bridge_address=fake.bridge,
            expected_network_id=fake.network_id,
            expected_bridge_code_hash=None,
            expected_destination_binding_hash=fake.destination_binding,
            route_allowlist_hash=bytes.fromhex(EVM_LIVE_ROUTE_ALLOWLIST_HASH_VECTOR),
            source_verifier_material_hash=bytes.fromhex(
                EVM_SOURCE_VERIFIER_MATERIAL_HASH
            ),
            source_adapter_engine_deployment_hash=bytes.fromhex(
                EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
            ),
            block_tag="latest",
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    assert "offline_toml_sha256" not in summary
    try:
        module.render_offline_toml(summary)
    except ValueError as exc:
        assert "expected-bridge-code-hash" in str(exc)
    else:
        raise AssertionError("full TOML rendered without pinned bridge code hash")


def test_live_evm_defaults_expected_network_id_to_canonical_mainnet_id():
    module = load_live_module()
    fake = fake_opener_for(module)

    summary = module.collect_live_evidence(
        SimpleNamespace(
            rpc_url="https://ethereum.example",
            domain=module.evidence.SCCP_DOMAIN_ETH,
            bridge_address=fake.bridge,
            expected_network_id=None,
            expected_bridge_code_hash=fake.bridge_code_hash,
            expected_destination_binding_hash=fake.destination_binding,
            route_allowlist_hash=bytes.fromhex(EVM_LIVE_ROUTE_ALLOWLIST_HASH_VECTOR),
            source_verifier_material_hash=bytes.fromhex(
                EVM_SOURCE_VERIFIER_MATERIAL_HASH
            ),
            source_adapter_engine_deployment_hash=bytes.fromhex(
                EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
            ),
            block_tag="latest",
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    destination = summary["destination_bridge"]
    assert destination["expected_network_id"] == "0x" + ETH_MAINNET_NETWORK_ID
    assert destination["expected_network_id_matches"] is True
    assert "offline_toml_sha256" not in summary
    try:
        module.render_offline_toml(summary)
    except ValueError as exc:
        message = str(exc)
        assert "expected-network-id" not in message
        assert "route-canary" in message
    else:
        raise AssertionError("full TOML rendered without route canary evidence")


def test_live_evm_rejects_wrong_expected_network_id():
    module = load_live_module()
    fake = fake_opener_for(module)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                rpc_url="https://ethereum.example",
                domain=module.evidence.SCCP_DOMAIN_ETH,
                bridge_address=fake.bridge,
                expected_network_id=bytes.fromhex("44" * 32),
                expected_bridge_code_hash=None,
                expected_destination_binding_hash=None,
                route_allowlist_hash=None,
                block_tag="latest",
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "expected-network-id must match the canonical" in str(exc)
    else:
        raise AssertionError("wrong EVM destination network id was accepted")


def test_live_evm_rejects_noncanonical_bridge_network_id_by_default():
    module = load_live_module()
    fake = fake_opener_for(module, network_id_override=bytes.fromhex("44" * 32))

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                rpc_url="https://ethereum.example",
                domain=module.evidence.SCCP_DOMAIN_ETH,
                bridge_address=fake.bridge,
                expected_network_id=None,
                expected_bridge_code_hash=None,
                expected_destination_binding_hash=None,
                route_allowlist_hash=None,
                block_tag="latest",
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "network_id must match ETH mainnet EIP-155 chain id 1" in str(exc)
    else:
        raise AssertionError("noncanonical EVM destination network id was accepted")


def test_live_evm_evidence_rejects_wrong_backend_and_target():
    module = load_live_module()
    wrong_backend = fake_opener_for(
        module,
        backend_hash_override=bytes.fromhex("ab" * 32),
    )
    try:
        module.collect_live_evidence(
            SimpleNamespace(
                rpc_url="https://ethereum.example",
                domain=module.evidence.SCCP_DOMAIN_ETH,
                bridge_address=wrong_backend.bridge,
                expected_network_id=None,
                expected_bridge_code_hash=None,
                expected_destination_binding_hash=None,
                route_allowlist_hash=None,
                block_tag="latest",
                timeout=1.0,
            ),
            opener=wrong_backend.opener,
        )
    except RuntimeError as exc:
        assert "verifierBackendHash" in str(exc)
    else:
        raise AssertionError("wrong EVM verifier backend was accepted")

    wrong_source = fake_opener_for(module, source_domain=module.evidence.SCCP_DOMAIN_BSC)
    try:
        module.collect_live_evidence(
            SimpleNamespace(
                rpc_url="https://ethereum.example",
                domain=module.evidence.SCCP_DOMAIN_ETH,
                bridge_address=wrong_source.bridge,
                expected_network_id=None,
                expected_bridge_code_hash=None,
                expected_destination_binding_hash=None,
                route_allowlist_hash=None,
                block_tag="latest",
                timeout=1.0,
            ),
            opener=wrong_source.opener,
        )
    except RuntimeError as exc:
        assert "expectedSourceDomain" in str(exc)
    else:
        raise AssertionError("wrong EVM source domain was accepted")

    wrong_target = fake_opener_for(module, target_domain=module.evidence.SCCP_DOMAIN_BSC)
    try:
        module.collect_live_evidence(
            SimpleNamespace(
                rpc_url="https://ethereum.example",
                domain=module.evidence.SCCP_DOMAIN_ETH,
                bridge_address=wrong_target.bridge,
                expected_network_id=None,
                expected_bridge_code_hash=None,
                expected_destination_binding_hash=None,
                route_allowlist_hash=None,
                block_tag="latest",
                timeout=1.0,
            ),
            opener=wrong_target.opener,
        )
    except RuntimeError as exc:
        assert "expectedTargetDomain" in str(exc)
    else:
        raise AssertionError("wrong EVM target domain was accepted")


def test_live_evm_cli_json_and_toml_outputs(capsys):
    module = load_live_module()
    fake = fake_opener_for(module)
    route_allowlist_hash = bytes.fromhex(EVM_LIVE_ROUTE_ALLOWLIST_HASH_VECTOR)
    route_canary_hash = route_canary_hash_for(module, fake, route_allowlist_hash)
    original_collect = module.collect_live_evidence

    def collect_with_fake(args):
        return original_collect(args, opener=fake.opener)

    module.collect_live_evidence = collect_with_fake
    args = [
        "--rpc-url",
        "https://ethereum.example",
        "--domain",
        "eth",
        "--bridge-address",
        fake.bridge,
        "--expected-network-id",
        "0x" + ETH_MAINNET_NETWORK_ID,
        "--expected-rpc-chain-id",
        "1",
        "--expected-bridge-code-hash",
        "0x" + fake.bridge_code_hash.hex(),
        "--expected-destination-binding-hash",
        "0x" + fake.destination_binding.hex(),
        "--route-allowlist-hash",
        "0x" + EVM_LIVE_ROUTE_ALLOWLIST_HASH_VECTOR,
        "--source-verifier-material-hash",
        "0x" + EVM_SOURCE_VERIFIER_MATERIAL_HASH,
        "--source-adapter-engine-deployment-hash",
        "0x" + EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH,
    ]
    full_args = [
        *args,
        "--route-canary-evidence-hash",
        "0x" + route_canary_hash.hex(),
        "--route-canary-transaction-hash",
        "0x" + fake.route_canary_transaction_hash.hex(),
        "--route-canary-log-index",
        str(fake.route_canary_log_index),
        "--route-canary-receipt-block-number",
        str(fake.route_canary_receipt_block_number),
        "--route-canary-receipt-block-hash",
        "0x" + fake.route_canary_receipt_block_hash.hex(),
        "--route-canary-block-receipts-root",
        "0x" + fake.route_canary_block_receipts_root.hex(),
    ]
    try:
        assert module.main(args) == 0
        output = json.loads(capsys.readouterr().out)
        assert output["destination_bridge"]["destination_binding_hash"] == (
            "0x" + fake.destination_binding.hex()
        )
        assert "offline_toml_sha256" not in output
        assert "route_canary" not in output

        try:
            module.main([*args, "--full-toml"])
        except SystemExit as exc:
            assert exc.code == 2
        else:
            raise AssertionError("EVM live full TOML accepted without route canary")

        assert module.main([*full_args, "--full-toml"]) == 0
        rendered = capsys.readouterr().out
        assert "[[zk.sccp_destination_rollouts]]" in rendered
        assert "[[zk.sccp_route_allowlists]]" in rendered
        assert '# sccp_route_canary_status = "passed"' in rendered
        assert "# sccp_evm_route_canary_transaction_hash" in rendered
        assert "evm_route_canary_transaction_hash = " in rendered
    finally:
        module.collect_live_evidence = original_collect
