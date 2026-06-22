import copy
import hashlib
import json
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
from types import SimpleNamespace


def load_live_module():
    script_path = (
        Path(__file__).resolve().parents[2]
        / "scripts"
        / "sccp_evm_source_live_evidence.py"
    )
    spec = spec_from_file_location("sccp_evm_source_live_evidence", script_path)
    module = module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def render_replayed_offline_toml(live_module, domain, offline_args):
    evidence = live_module._load_evidence_module(domain)
    args = evidence.build_parser().parse_args([*offline_args, "--toml"])
    return evidence.render_toml(args)


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
        return b"secret-token-evm-source-error" * size

    def close(self):
        return None


def test_evm_source_live_cli_redacts_top_level_exception_details(monkeypatch, capsys):
    module = load_live_module()

    for exception_type in (OSError, RuntimeError, TypeError, ValueError):

        def fail_collect(_args, exception_type=exception_type):
            raise exception_type("secret-token /tmp/operator/private-path")

        with monkeypatch.context() as patch:
            patch.setattr(module, "collect_live_evidence", fail_collect)
            try:
                module.main(
                    [
                        "--rpc-url",
                        "https://evm.example.invalid",
                        "--domain",
                        "eth",
                        "--bridge-address",
                        "0x" + "11" * 20,
                    ]
                )
            except SystemExit as exc:
                assert exc.code == 2
            else:
                raise AssertionError(
                    "EVM source live CLI accepted top-level collection failure"
                )

            captured = capsys.readouterr()
            assert "SCCP EVM source live evidence collection failed" in captured.err
            assert "secret-token" not in captured.err
            assert "private-path" not in captured.err
            assert exception_type.__name__ not in captured.err


def test_evm_source_live_cli_omits_runtime_endpoint_and_unknown_summary_fields(
    monkeypatch,
    capsys,
):
    module = load_live_module()

    monkeypatch.setattr(
        module,
        "collect_live_evidence",
        lambda _args: {
            "rpc_url": "https://rpc.example.invalid/secret-token-provider",
            "read_only": True,
            "block_tag": "finalized",
            "source_bridge": {"domain": module.SCCP_DOMAIN_ETH},
            "offline_evidence_args": ["--domain", "eth"],
            "operator_note": "safe note",
            "secret-token-summary": "secret-token-value",
            7: "secret-token-int-key",
            "_source_args": "private source args",
        },
    )

    exit_code = module.main(
        [
            "--rpc-url",
            "https://rpc.example.invalid/secret-token-provider",
            "--domain",
            "eth",
            "--bridge-address",
            "0x" + "11" * 20,
        ]
    )

    assert exit_code == 0
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    assert payload == {
        "block_tag": "finalized",
        "offline_evidence_args": ["--domain", "eth"],
        "read_only": True,
        "source_bridge": {"domain": module.SCCP_DOMAIN_ETH},
    }
    assert "rpc_url" not in payload
    assert "operator_note" not in payload
    assert "secret-token-summary" not in payload
    assert "7" not in payload
    assert "secret-token" not in captured.out
    assert "private source args" not in captured.out
    assert "Traceback" not in captured.err


def fake_opener_for(
    module,
    *,
    domain=1,
    rpc_chain_id=1,
    runtime=None,
    receipt_block_runtime=None,
    receipt_transaction_hash=None,
    receipt_contract_address=None,
    omit_receipt_contract_address=False,
    receipt_block_hash=None,
    receipt_block_number="0x1234",
    block_response_hash=None,
    block_response_number=None,
    block_response_receipts_root=None,
    finalized_block_hash=None,
    finalized_block_number=None,
    deployment_transaction_hash=None,
    deployment_transaction_block_hash=None,
    deployment_transaction_block_number=None,
    deployment_transaction_to=None,
    deployment_transaction_input=None,
):
    bridge = "0x" + "11" * 20
    runtime = runtime or bytes.fromhex("60806040526003")
    receipt_block_runtime = receipt_block_runtime or runtime
    receipt_block_hash = receipt_block_hash or ("0x" + "99" * 32)
    block_response_hash = block_response_hash or receipt_block_hash
    block_response_number = block_response_number or receipt_block_number
    block_response_receipts_root = block_response_receipts_root or ("0x" + "bc" * 32)
    finalized_block_hash = finalized_block_hash or receipt_block_hash
    finalized_block_number = finalized_block_number or receipt_block_number
    deployment_transaction_block_hash = (
        deployment_transaction_block_hash or receipt_block_hash
    )
    deployment_transaction_block_number = (
        deployment_transaction_block_number or receipt_block_number
    )
    deployment_transaction_input = deployment_transaction_input or (
        "0x" + runtime.hex()
    )
    evidence = module._load_evidence_module(domain)
    bridge_code_hash = evidence.runtime_bytecode_hash(runtime)

    def opener(request, timeout):
        del timeout
        payload = json.loads(request.data.decode("utf-8"))
        method = payload["method"]
        params = payload["params"]
        if method == "eth_chainId":
            return FakeResponse(
                {"jsonrpc": "2.0", "id": payload["id"], "result": hex(rpc_chain_id)}
            )
        if method == "eth_getCode":
            assert params[0].lower() == bridge
            selected_runtime = (
                receipt_block_runtime if params[1] == receipt_block_number else runtime
            )
            return FakeResponse(
                {
                    "jsonrpc": "2.0",
                    "id": payload["id"],
                    "result": "0x" + selected_runtime.hex(),
                }
            )
        if method == "eth_getTransactionReceipt":
            transaction_hash = (
                "0x" + receipt_transaction_hash.hex()
                if receipt_transaction_hash is not None
                else params[0]
            )
            receipt = {
                "transactionHash": transaction_hash,
                "status": "0x1",
                "blockHash": receipt_block_hash,
                "blockNumber": receipt_block_number,
            }
            if not omit_receipt_contract_address:
                receipt["contractAddress"] = (
                    receipt_contract_address
                    if receipt_contract_address is not None
                    else bridge
                )
            return FakeResponse(
                {
                    "jsonrpc": "2.0",
                    "id": payload["id"],
                    "result": receipt,
                }
            )
        if method == "eth_getTransactionByHash":
            transaction_hash = (
                "0x" + deployment_transaction_hash.hex()
                if deployment_transaction_hash is not None
                else params[0]
            )
            return FakeResponse(
                {
                    "jsonrpc": "2.0",
                    "id": payload["id"],
                    "result": {
                        "hash": transaction_hash,
                        "blockHash": deployment_transaction_block_hash,
                        "blockNumber": deployment_transaction_block_number,
                        "to": deployment_transaction_to,
                        "input": deployment_transaction_input,
                    },
                }
            )
        if method == "eth_getBlockByNumber":
            if params[0] == "finalized":
                assert params[1] is False
                return FakeResponse(
                    {
                        "jsonrpc": "2.0",
                        "id": payload["id"],
                        "result": {
                            "hash": finalized_block_hash,
                            "number": finalized_block_number,
                            "receiptsRoot": block_response_receipts_root,
                        },
                    }
                )
            assert params[0] == receipt_block_number
            assert params[1] is False
            return FakeResponse(
                {
                    "jsonrpc": "2.0",
                    "id": payload["id"],
                    "result": {
                        "hash": block_response_hash,
                        "number": block_response_number,
                        "receiptsRoot": block_response_receipts_root,
                    },
                }
            )
        raise AssertionError(f"unexpected method {method}")

    return SimpleNamespace(
        opener=opener,
        bridge=bridge,
        bridge_runtime=runtime,
        bridge_code_hash=bridge_code_hash,
    )


def source_args(module, fake, *, domain=1):
    evidence = module._load_evidence_module(domain)
    args = SimpleNamespace(
        source_domain=domain,
        target_domain=0,
        bridge_address=bytes.fromhex("11" * 20),
        source_trust_anchor_hash=bytes.fromhex("44" * 32),
        consensus_verifier_hash=bytes.fromhex("55" * 32),
        message_inclusion_verifier_hash=bytes.fromhex("66" * 32),
        source_bridge_emitter_code_hash=fake.bridge_code_hash,
        finality_policy_hash=bytes.fromhex("88" * 32),
        adapter_verifier_vk_hash=module._adapter_verifier_vk_hash(evidence, domain),
        deployment_receipt_hash=bytes.fromhex("aa" * 32),
        expected_source_verifier_material_hash=None,
        expected_source_adapter_engine_deployment_hash=None,
    )
    material_hash, deployment_hash = module._source_record_hashes(
        evidence,
        domain,
        args,
    )
    return args, material_hash, deployment_hash


def test_evm_source_json_rpc_response_size_is_bounded():
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
        raise AssertionError("oversized EVM source JSON-RPC response was accepted")


def test_evm_source_json_rpc_http_error_detail_is_bounded():
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
        assert message == "JSON-RPC eth_chainId failed with HTTP 502"
        assert "secret-token" not in message
        assert len(message) < 100
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("oversized EVM source JSON-RPC error body was accepted")


def test_evm_source_json_rpc_redacts_invalid_json_parser_details():
    module = load_live_module()

    def invalid_json_opener(_request, timeout):
        del timeout
        return RawResponse(b'{"secret-token invalid EVM source JSON-RPC payload": ')

    try:
        module._json_rpc(
            "https://ethereum.example",
            "eth_chainId",
            [],
            opener=invalid_json_opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        message = str(exc)
        assert message == "JSON-RPC eth_chainId returned invalid JSON"
        assert "secret-token" not in message
        assert "JSON-RPC payload" not in message
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError(
            "secret-bearing invalid EVM source JSON-RPC payload was accepted"
        )


def test_evm_source_json_rpc_rejects_duplicate_json_keys():
    module = load_live_module()
    duplicate_payload = (
        b'{"jsonrpc":"2.0","id":1,'
        b'"secret-token-result":"0x1","secret-token-result":"0x2",'
        b'"result":"0x3"}'
    )

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
        message = str(exc)
        assert message == "JSON-RPC eth_chainId returned duplicate JSON keys"
        assert "secret-token" not in message
        assert "duplicate JSON key " not in message
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("duplicate-key EVM source JSON-RPC response was accepted")


def test_evm_source_json_rpc_redacts_transport_and_error_response_details():
    module = load_live_module()

    def secret_url_error_opener(_request, timeout):
        del timeout
        raise module.urllib.error.URLError(
            "secret-token provider URL leaked from transport"
        )

    def secret_error_object_opener(_request, timeout):
        del timeout
        return FakeResponse(
            {
                "jsonrpc": "2.0",
                "id": 1,
                "error": {
                    "code": -32000,
                    "message": "secret-token source provider error object",
                },
            }
        )

    try:
        module._json_rpc(
            "https://bsc.example",
            "eth_chainId",
            [],
            opener=secret_url_error_opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        message = str(exc)
        assert message == "JSON-RPC eth_chainId request failed"
        assert "secret-token" not in message
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("secret-bearing EVM source transport error was accepted")

    try:
        module._json_rpc(
            "https://bsc.example",
            "eth_chainId",
            [],
            opener=secret_error_object_opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        message = str(exc)
        assert message == "JSON-RPC eth_chainId returned error response"
        assert "secret-token" not in message
        assert "provider error object" not in message
    else:
        raise AssertionError("secret-bearing EVM source JSON-RPC error was accepted")


def test_evm_source_json_rpc_rejects_envelope_drift():
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


def test_evm_source_live_numeric_parsers_require_canonical_decimal():
    module = load_live_module()

    assert module.parse_domain("eth") == module.SCCP_DOMAIN_ETH
    assert module.parse_domain("1") == module.SCCP_DOMAIN_ETH
    assert module._parse_rpc_chain_id("1") == 1
    assert module._parse_hex32("0x" + "11" * 32, label="component hash") == (
        bytes.fromhex("11" * 32)
    )
    assert module._rpc_quantity("0xa", method="eth_test") == 10
    assert module._rpc_hex_data("0x6000", method="eth_test") == bytes.fromhex("6000")

    for value in ("01", "0x1", "+1", " 1 ", "١"):
        try:
            module.parse_domain(value)
        except module.argparse.ArgumentTypeError as exc:
            assert "domain must be eth, bsc, 1, or 2" in str(exc)
        else:
            raise AssertionError(f"noncanonical EVM source domain {value!r} was accepted")

        try:
            module._parse_rpc_chain_id(value)
        except module.argparse.ArgumentTypeError as exc:
            assert "expected-rpc-chain-id" in str(exc)
        else:
            raise AssertionError(f"noncanonical EVM source chain id {value!r} was accepted")

    try:
        module._parse_hex32(" 0x" + "11" * 32, label="component hash")
    except module.argparse.ArgumentTypeError as exc:
        assert "must not contain whitespace" in str(exc)
    else:
        raise AssertionError("padded EVM source component hash was accepted")

    payload = "secret-token-evm-source-live-hex"
    try:
        module._parse_hex32(
            "0x" + payload + ("a" * (64 - len(payload))),
            label="component hash",
        )
    except module.argparse.ArgumentTypeError as exc:
        rendered = str(exc)
        assert rendered == "component hash must be hex"
        assert "secret-token" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("invalid EVM source component hash hex was accepted")

    for value, expected in (
        ("0X" + "11" * 32, "lowercase 0x prefix"),
        ("0x" + "AA" * 32, "lowercase hex"),
        ("aa" * 32, "lowercase 0x hex"),
    ):
        try:
            module._parse_hex32(value, label="component hash")
        except module.argparse.ArgumentTypeError as exc:
            assert expected in str(exc)
        else:
            raise AssertionError(
                f"noncanonical EVM source component hash {value!r} was accepted"
            )

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
            raise AssertionError(f"padded EVM source RPC result {value!r} was accepted")

    try:
        module._require_exact_positive_u64(
            "4660",
            label="deployment receipt block number",
        )
    except ValueError as exc:
        assert "exact positive u64" in str(exc)
    else:
        raise AssertionError("string EVM source receipt block number was accepted")

    source_bridge = {
        "deployment_receipt_status": "0x1",
        "deployment_transaction_hash": "0x" + "de" * 32,
        "deployment_receipt_block_hash": "0x" + "99" * 32,
        "deployment_receipt_block_number": "4660",
        "deployment_receipt_block_receipts_root": "0x" + "bc" * 32,
        "deployment_receipt_block_receipts_root_verified": True,
    }
    assert module._source_bridge_deployment_receipt_is_verified(source_bridge) is False


def test_evm_source_live_hex_parsers_redact_typeerror_parser_causes(monkeypatch):
    module = load_live_module()

    class SecretBytes:
        @staticmethod
        def fromhex(_text):
            raise TypeError("secret-token EVM source live hex TypeError detail")

    monkeypatch.setattr(module, "bytes", SecretBytes, raising=False)

    try:
        module._parse_hex32("0x" + "11" * 32, label="component hash")
    except module.argparse.ArgumentTypeError as exc:
        rendered = str(exc)
        assert rendered == "component hash must be hex"
        assert "secret-token" not in rendered
        assert "TypeError" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("EVM source live component parser TypeError was accepted")

    try:
        module._rpc_hex_data("0x6000", method="eth_getCode source bridge")
    except RuntimeError as exc:
        rendered = str(exc)
        assert (
            rendered
            == "eth_getCode source bridge returned non-canonical lowercase 0x hex data"
        )
        assert "secret-token" not in rendered
        assert "TypeError" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("EVM source live RPC hex parser TypeError was accepted")


def test_evm_source_live_block_tag_parser_rejects_unstable_or_noncanonical_tags():
    module = load_live_module()

    assert module.parse_block_tag("latest") == "latest"
    assert module.parse_block_tag("safe") == "safe"
    assert module.parse_block_tag("finalized") == "finalized"
    assert module.parse_block_tag("0x1234") == "0x1234"

    for value in (
        "pending",
        "earliest",
        " 0x1234",
        "0x01234",
        "0X1234",
        "1234",
        "0x0",
    ):
        try:
            module.parse_block_tag(value)
        except module.argparse.ArgumentTypeError as exc:
            assert "block-tag" in str(exc)
        else:
            raise AssertionError(
                f"unstable/noncanonical block tag {value!r} was accepted"
            )


def test_evm_source_live_cli_defaults_eth_to_finalized_and_bsc_to_latest():
    module = load_live_module()
    parser = module.build_parser()

    eth_fake = fake_opener_for(module)
    eth_args = parser.parse_args(
        [
            "--rpc-url",
            "https://ethereum.example",
            "--domain",
            "eth",
            "--bridge-address",
            eth_fake.bridge,
        ]
    )
    eth_summary = module.collect_live_evidence(eth_args, opener=eth_fake.opener)
    assert eth_summary["block_tag"] == "finalized"

    bsc_fake = fake_opener_for(
        module,
        domain=module.SCCP_DOMAIN_BSC,
        rpc_chain_id=56,
    )
    bsc_args = parser.parse_args(
        [
            "--rpc-url",
            "https://bsc.example",
            "--domain",
            "bsc",
            "--bridge-address",
            bsc_fake.bridge,
        ]
    )
    bsc_summary = module.collect_live_evidence(bsc_args, opener=bsc_fake.opener)
    assert bsc_summary["block_tag"] == "latest"


def test_evm_source_live_direct_collector_rejects_unstable_block_tag_before_rpc():
    module = load_live_module()

    def opener(_request, _timeout):
        raise AssertionError("collector should reject block tag before JSON-RPC")

    try:
        module.collect_source_bridge_evidence(
            "https://ethereum.example",
            domain=module.SCCP_DOMAIN_ETH,
            bridge_address="0x" + "11" * 20,
            block_tag="pending",
            opener=opener,
            timeout=1.0,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "block-tag" in str(exc)
    else:
        raise AssertionError("unstable direct collector block tag was accepted")


def test_evm_source_live_evidence_collects_source_records_and_toml():
    module = load_live_module()
    fake = fake_opener_for(module)
    args, material_hash, deployment_hash = source_args(module, fake)

    summary = module.collect_live_evidence(
        SimpleNamespace(
            rpc_url="https://ethereum.example",
            domain=module.SCCP_DOMAIN_ETH,
            bridge_address=fake.bridge,
            expected_rpc_chain_id=1,
            expected_source_bridge_code_hash=fake.bridge_code_hash,
            deployment_transaction_hash=bytes.fromhex("de" * 32),
            source_trust_anchor_hash=args.source_trust_anchor_hash,
            consensus_verifier_hash=args.consensus_verifier_hash,
            message_inclusion_verifier_hash=args.message_inclusion_verifier_hash,
            finality_policy_hash=args.finality_policy_hash,
            adapter_verifier_vk_hash=args.adapter_verifier_vk_hash,
            deployment_receipt_hash=args.deployment_receipt_hash,
            expected_source_verifier_material_hash=material_hash,
            expected_source_adapter_engine_deployment_hash=deployment_hash,
            block_tag="finalized",
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    source = summary["source_bridge"]
    assert summary["read_only"] is True
    assert source["chain"] == "eth"
    assert source["rpc_chain_id"] == 1
    assert source["bridge_address"] == fake.bridge
    assert source["bridge_code_hash"] == "0x" + fake.bridge_code_hash.hex()
    assert source["bridge_runtime_bytecode_hex"] == "0x" + fake.bridge_runtime.hex()
    assert source["expected_source_bridge_code_hash_matches"] is True
    assert source["deployment_receipt_status"] == "0x1"
    assert source["deployment_receipt_contract_address"] == fake.bridge
    assert source["deployment_receipt_block_number"] == 0x1234
    assert source["deployment_transaction_block_hash"] == "0x" + "99" * 32
    assert source["deployment_transaction_block_number"] == 0x1234
    assert source["deployment_transaction_contract_creation"] is True
    assert source["deployment_transaction_input_sha256"] == hashlib.sha256(
        fake.bridge_runtime
    ).hexdigest()
    assert source["deployment_transaction_block_matches"] is True
    assert source["deployment_receipt_block_hash_matches"] is True
    assert source["deployment_receipt_block_receipts_root"] == "0x" + "bc" * 32
    assert source["deployment_receipt_block_receipts_root_verified"] is True
    assert source["deployment_receipt_block_code_hash_matches"] is True
    assert source["deployment_receipt_block_finalized"] is True
    assert source["deployment_receipt_finalized_block_number"] == 0x1234
    assert source["deployment_receipt_finalized_block_hash"] == "0x" + "99" * 32
    assert summary["source_records"] == {
        "source_verifier_material_hash": "0x" + material_hash.hex(),
        "source_adapter_engine_deployment_hash": "0x" + deployment_hash.hex(),
        "expected_source_verifier_material_hash_matches": True,
        "expected_source_adapter_engine_deployment_hash_matches": True,
        "expected_source_verifier_material_hash": "0x" + material_hash.hex(),
        "expected_source_adapter_engine_deployment_hash": "0x"
        + deployment_hash.hex(),
    }
    assert summary["offline_toml_sha256"]

    rendered = module.render_offline_toml(summary)
    assert '# sccp_evm_source_block_tag = "finalized"' in rendered
    assert '# sccp_evm_source_rpc_chain_id = "1"' in rendered
    assert '# sccp_evm_source_bridge_address = "' + fake.bridge + '"' in rendered
    assert (
        '# sccp_evm_source_bridge_runtime_code_hash = "0x'
        + fake.bridge_code_hash.hex()
        + '"'
        in rendered
    )
    assert (
        '# sccp_evm_source_bridge_runtime_bytecode_hex = "0x'
        + fake.bridge_runtime.hex()
        + '"'
        in rendered
    )
    assert '# sccp_evm_source_deployment_receipt_status = "0x1"' in rendered
    assert (
        '# sccp_evm_source_deployment_transaction_block_hash = "0x'
        + "99" * 32
        + '"'
        in rendered
    )
    assert '# sccp_evm_source_deployment_transaction_block_number = "4660"' in rendered
    assert "# sccp_evm_source_deployment_transaction_input_sha256" in rendered
    assert '# sccp_evm_source_deployment_block_number = "4660"' in rendered
    assert (
        '# sccp_evm_source_deployment_block_receipts_root = "0x'
        + "bc" * 32
        + '"'
        in rendered
    )
    assert (
        '# sccp_evm_source_deployment_finalized_block_hash = "0x'
        + "99" * 32
        + '"'
        in rendered
    )
    assert '# sccp_evm_source_deployment_finalized_block_number = "4660"' in rendered
    assert "# sccp_evm_source_deployment_block_finalized = true" in rendered
    assert rendered.count("# sccp_evm_source_rpc_chain_id") == 1
    assert rendered.count("# sccp_evm_source_block_tag") == 1
    assert rendered.count("# sccp_evm_source_bridge_address") == 1
    assert rendered.count("# sccp_evm_source_bridge_runtime_code_hash") == 1
    assert rendered.count("# sccp_evm_source_bridge_runtime_bytecode_hex") == 1
    assert rendered.count("# sccp_evm_source_deployment_transaction_hash") == 1
    assert rendered.count("# sccp_evm_source_deployment_transaction_block_hash") == 1
    assert rendered.count("# sccp_evm_source_deployment_transaction_block_number") == 1
    assert rendered.count("# sccp_evm_source_deployment_transaction_input_sha256") == 1
    assert rendered.count("# sccp_evm_source_deployment_receipt_status") == 1
    assert rendered.count("# sccp_evm_source_deployment_contract_address") == 1
    assert rendered.count("# sccp_evm_source_deployment_block_hash") == 1
    assert rendered.count("# sccp_evm_source_deployment_block_number") == 1
    assert rendered.count("# sccp_evm_source_deployment_block_receipts_root") == 1
    assert rendered.count("# sccp_evm_source_deployment_finalized_block_hash") == 1
    assert rendered.count("# sccp_evm_source_deployment_finalized_block_number") == 1
    assert rendered.count("# sccp_evm_source_deployment_block_finalized") == 1
    assert 'source_chain = "eth"' in rendered
    assert 'source_bridge_emitter_code_hash = "0x' + fake.bridge_code_hash.hex() in rendered
    offline_args = summary["offline_evidence_args"]
    assert "--source-bridge-runtime-bytecode-hex" in offline_args
    assert "0x" + fake.bridge_runtime.hex() in offline_args
    assert "--deployment-transaction-block-hash" in offline_args
    assert "0x" + "99" * 32 in offline_args
    assert "--deployment-transaction-block-number" in offline_args
    assert "4660" in offline_args
    assert "--deployment-transaction-input-sha256" in offline_args
    assert "0x" + hashlib.sha256(fake.bridge_runtime).hexdigest() in offline_args
    replayed = render_replayed_offline_toml(
        module,
        module.SCCP_DOMAIN_ETH,
        offline_args,
    )
    assert 'source_chain = "eth"' in replayed
    assert "# sccp_evm_source_deployment_transaction_block_hash" in replayed


def test_evm_source_live_eth_toml_requires_finalized_block_tag():
    module = load_live_module()
    fake = fake_opener_for(module)
    args, material_hash, deployment_hash = source_args(module, fake)

    summary = module.collect_live_evidence(
        SimpleNamespace(
            rpc_url="https://ethereum.example",
            domain=module.SCCP_DOMAIN_ETH,
            bridge_address=fake.bridge,
            expected_rpc_chain_id=1,
            expected_source_bridge_code_hash=fake.bridge_code_hash,
            deployment_transaction_hash=bytes.fromhex("de" * 32),
            source_trust_anchor_hash=args.source_trust_anchor_hash,
            consensus_verifier_hash=args.consensus_verifier_hash,
            message_inclusion_verifier_hash=args.message_inclusion_verifier_hash,
            finality_policy_hash=args.finality_policy_hash,
            adapter_verifier_vk_hash=args.adapter_verifier_vk_hash,
            deployment_receipt_hash=args.deployment_receipt_hash,
            expected_source_verifier_material_hash=material_hash,
            expected_source_adapter_engine_deployment_hash=deployment_hash,
            block_tag="latest",
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    assert summary["block_tag"] == "latest"
    assert "offline_toml_sha256" not in summary
    try:
        module.render_offline_toml(summary)
    except ValueError as exc:
        assert "--block-tag finalized" in str(exc)
    else:
        raise AssertionError("Ethereum source TOML rendered from non-finalized block tag")


def test_evm_source_live_toml_revalidates_imported_summary_metadata(monkeypatch):
    module = load_live_module()
    fake = fake_opener_for(module)
    args, material_hash, deployment_hash = source_args(module, fake)
    summary = module.collect_live_evidence(
        SimpleNamespace(
            rpc_url="https://ethereum.example",
            domain=module.SCCP_DOMAIN_ETH,
            bridge_address=fake.bridge,
            expected_rpc_chain_id=1,
            expected_source_bridge_code_hash=fake.bridge_code_hash,
            deployment_transaction_hash=bytes.fromhex("de" * 32),
            source_trust_anchor_hash=args.source_trust_anchor_hash,
            consensus_verifier_hash=args.consensus_verifier_hash,
            message_inclusion_verifier_hash=args.message_inclusion_verifier_hash,
            finality_policy_hash=args.finality_policy_hash,
            adapter_verifier_vk_hash=args.adapter_verifier_vk_hash,
            deployment_receipt_hash=args.deployment_receipt_hash,
            expected_source_verifier_material_hash=material_hash,
            expected_source_adapter_engine_deployment_hash=deployment_hash,
            block_tag="finalized",
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    for mutate, expected_message in (
        (
            lambda forged: forged["source_bridge"].__setitem__("rpc_chain_id", 56),
            "RPC chain id",
        ),
        (
            lambda forged: forged["source_bridge"].__setitem__(
                "expected_source_bridge_code_hash",
                "0x" + "aa" * 32,
            ),
            "expected source bridge code hash",
        ),
        (
            lambda forged: forged["source_bridge"].__setitem__(
                "bridge_runtime_bytecode_hex",
                "0x6001",
            ),
            "runtime bytecode",
        ),
        (
            lambda forged: forged["source_bridge"].__setitem__(
                "deployment_receipt_block_hash_matches",
                False,
            ),
            "deployment receipt block hash",
        ),
        (
            lambda forged: forged["source_bridge"].__setitem__(
                "deployment_receipt_block_receipts_root_verified",
                False,
            ),
            "deployment receipt block receiptsRoot",
        ),
        (
            lambda forged: forged["source_bridge"].__setitem__(
                "deployment_receipt_block_code_hash_matches",
                False,
            ),
            "deployment receipt block code hash",
        ),
        (
            lambda forged: forged["source_bridge"].__setitem__(
                "deployment_receipt_block_finalized",
                False,
            ),
            "finality",
        ),
        (
            lambda forged: forged["source_records"].__setitem__(
                "source_verifier_material_hash",
                "0x" + "bb" * 32,
            ),
            "source verifier material hash",
        ),
        (
            lambda forged: forged["source_records"].__setitem__(
                "expected_source_adapter_engine_deployment_hash",
                "0x" + "cc" * 32,
            ),
            "expected source adapter engine deployment hash",
        ),
    ):
        forged = copy.deepcopy(summary)
        mutate(forged)
        try:
            module.render_offline_toml(forged)
        except ValueError as exc:
            assert expected_message in str(exc)
        else:
            raise AssertionError("forged EVM source live summary rendered TOML")

    forged = copy.deepcopy(summary)
    forged["source_bridge"]["bridge_runtime_bytecode_hex"] = (
        "0xsecret-token-source-bridge-runtime"
    )
    try:
        module.render_offline_toml(forged)
    except ValueError as exc:
        rendered = str(exc)
        assert rendered == "EVM source bridge runtime bytecode metadata is invalid"
        assert "secret-token" not in rendered
        assert "must be hex" not in rendered
        assert exc.__cause__ is None
    else:
        raise AssertionError("invalid EVM source live runtime metadata rendered TOML")

    source_module = module._load_evidence_module(module.SCCP_DOMAIN_ETH)
    original_parse_runtime_bytecode_hex = source_module.parse_runtime_bytecode_hex
    with monkeypatch.context() as patch:
        def fail_source_runtime_parse(value, *, label):
            if label == "source bridge runtime bytecode":
                raise TypeError(f"secret-token {label} imported parser detail")
            return original_parse_runtime_bytecode_hex(value, label=label)

        patch.setattr(
            source_module,
            "parse_runtime_bytecode_hex",
            fail_source_runtime_parse,
        )
        try:
            module.render_offline_toml(summary)
        except ValueError as exc:
            rendered = str(exc)
            assert rendered == "EVM source bridge runtime bytecode metadata is invalid"
            assert "secret-token" not in rendered
            assert "imported parser detail" not in rendered
            assert exc.__cause__ is None
            assert exc.__suppress_context__ is True
        else:
            raise AssertionError(
                "EVM source live TOML accepted imported runtime parser TypeError"
            )

    original_parse_hex_bytes = module._parse_hex_bytes
    with monkeypatch.context() as patch:
        def fail_bridge_address_parse(value, *, label, byte_length):
            if label == "source bridge address":
                raise module.argparse.ArgumentTypeError(
                    "secret-token source bridge address parser detail"
                )
            return original_parse_hex_bytes(value, label=label, byte_length=byte_length)

        patch.setattr(module, "_parse_hex_bytes", fail_bridge_address_parse)
        try:
            module.render_offline_toml(summary)
        except ValueError as exc:
            rendered = str(exc)
            assert rendered == "source bridge address metadata is invalid"
            assert "secret-token" not in rendered
            assert "parser detail" not in rendered
            assert exc.__cause__ is None
            assert exc.__suppress_context__ is True
        else:
            raise AssertionError(
                "EVM source live TOML leaked parser detail for bridge address"
            )


def test_bsc_source_live_evidence_uses_canonical_bsc_profile():
    module = load_live_module()
    fake = fake_opener_for(module, domain=module.SCCP_DOMAIN_BSC, rpc_chain_id=56)
    args, material_hash, deployment_hash = source_args(
        module,
        fake,
        domain=module.SCCP_DOMAIN_BSC,
    )

    summary = module.collect_live_evidence(
        SimpleNamespace(
            rpc_url="https://bsc.example",
            domain=module.SCCP_DOMAIN_BSC,
            bridge_address=fake.bridge,
            expected_rpc_chain_id=56,
            expected_source_bridge_code_hash=fake.bridge_code_hash,
            deployment_transaction_hash=bytes.fromhex("bd" * 32),
            source_trust_anchor_hash=args.source_trust_anchor_hash,
            consensus_verifier_hash=args.consensus_verifier_hash,
            message_inclusion_verifier_hash=args.message_inclusion_verifier_hash,
            finality_policy_hash=args.finality_policy_hash,
            adapter_verifier_vk_hash=args.adapter_verifier_vk_hash,
            deployment_receipt_hash=args.deployment_receipt_hash,
            expected_source_verifier_material_hash=material_hash,
            expected_source_adapter_engine_deployment_hash=deployment_hash,
            block_tag="latest",
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    assert summary["source_bridge"]["chain"] == "bsc"
    assert summary["source_bridge"]["rpc_chain_id"] == 56
    rendered = module.render_offline_toml(summary)
    assert 'source_domain = 2' in rendered
    assert 'source_chain = "bsc"' in rendered
    offline_args = summary["offline_evidence_args"]
    assert "--deployment-transaction-block-hash" in offline_args
    assert "--deployment-transaction-block-number" in offline_args
    assert "--deployment-transaction-input-sha256" in offline_args
    replayed = render_replayed_offline_toml(
        module,
        module.SCCP_DOMAIN_BSC,
        offline_args,
    )
    assert 'source_chain = "bsc"' in replayed


def test_evm_source_live_evidence_rejects_rpc_and_code_hash_drift():
    module = load_live_module()
    calls = []

    def wrong_chain_opener(request, timeout):
        del timeout
        payload = json.loads(request.data.decode("utf-8"))
        method = payload["method"]
        calls.append(method)
        if method == "eth_chainId":
            return FakeResponse(
                {"jsonrpc": "2.0", "id": payload["id"], "result": "0x38"}
            )
        raise AssertionError(f"unexpected RPC after wrong chain id: {method}")

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                rpc_url="https://ethereum.example",
                domain=module.SCCP_DOMAIN_ETH,
                bridge_address="0x" + "11" * 20,
                expected_rpc_chain_id=None,
                expected_source_bridge_code_hash=None,
                deployment_transaction_hash=None,
                source_trust_anchor_hash=None,
                consensus_verifier_hash=None,
                message_inclusion_verifier_hash=None,
                finality_policy_hash=None,
                adapter_verifier_vk_hash=None,
                deployment_receipt_hash=None,
                expected_source_verifier_material_hash=None,
                expected_source_adapter_engine_deployment_hash=None,
                block_tag="latest",
                timeout=1.0,
            ),
            opener=wrong_chain_opener,
        )
    except ValueError as exc:
        assert "eth_chainId for eth lane" in str(exc)
        assert "canonical mainnet chain id 1, got 56" in str(exc)
    else:
        raise AssertionError("wrong source RPC chain id was accepted")
    assert calls == ["eth_chainId"]

    fake = fake_opener_for(module)
    try:
        module.collect_live_evidence(
            SimpleNamespace(
                rpc_url="https://ethereum.example",
                domain=module.SCCP_DOMAIN_ETH,
                bridge_address=fake.bridge,
                expected_rpc_chain_id=1,
                expected_source_bridge_code_hash=bytes.fromhex("bb" * 32),
                deployment_transaction_hash=None,
                source_trust_anchor_hash=None,
                consensus_verifier_hash=None,
                message_inclusion_verifier_hash=None,
                finality_policy_hash=None,
                adapter_verifier_vk_hash=None,
                deployment_receipt_hash=None,
                expected_source_verifier_material_hash=None,
                expected_source_adapter_engine_deployment_hash=None,
                block_tag="latest",
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "expected-source-bridge-code-hash" in str(exc)
    else:
        raise AssertionError("wrong source bridge code hash was accepted")


def test_evm_source_live_rejects_noncanonical_expected_rpc_chain_id_before_rpc():
    module = load_live_module()

    def no_rpc_opener(_request, timeout):
        del timeout
        raise AssertionError("noncanonical expected chain id should fail before RPC")

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                rpc_url="https://bsc.example",
                domain=module.SCCP_DOMAIN_ETH,
                bridge_address="0x" + "11" * 20,
                expected_rpc_chain_id=56,
                expected_source_bridge_code_hash=None,
                deployment_transaction_hash=None,
                source_trust_anchor_hash=None,
                consensus_verifier_hash=None,
                message_inclusion_verifier_hash=None,
                finality_policy_hash=None,
                adapter_verifier_vk_hash=None,
                deployment_receipt_hash=None,
                expected_source_verifier_material_hash=None,
                expected_source_adapter_engine_deployment_hash=None,
                block_tag="latest",
                timeout=1.0,
            ),
            opener=no_rpc_opener,
        )
    except ValueError as exc:
        assert "canonical eth mainnet chain id 1" in str(exc)
    else:
        raise AssertionError("noncanonical explicit source RPC chain id was accepted")


def test_evm_source_live_toml_requires_deployment_receipt_evidence():
    module = load_live_module()
    fake = fake_opener_for(module)
    args, material_hash, deployment_hash = source_args(module, fake)

    summary = module.collect_live_evidence(
        SimpleNamespace(
            rpc_url="https://ethereum.example",
            domain=module.SCCP_DOMAIN_ETH,
            bridge_address=fake.bridge,
            expected_rpc_chain_id=1,
            expected_source_bridge_code_hash=fake.bridge_code_hash,
            deployment_transaction_hash=None,
            source_trust_anchor_hash=args.source_trust_anchor_hash,
            consensus_verifier_hash=args.consensus_verifier_hash,
            message_inclusion_verifier_hash=args.message_inclusion_verifier_hash,
            finality_policy_hash=args.finality_policy_hash,
            adapter_verifier_vk_hash=args.adapter_verifier_vk_hash,
            deployment_receipt_hash=args.deployment_receipt_hash,
            expected_source_verifier_material_hash=material_hash,
            expected_source_adapter_engine_deployment_hash=deployment_hash,
            block_tag="latest",
            timeout=1.0,
        ),
        opener=fake.opener,
    )
    assert "offline_toml_sha256" not in summary
    try:
        module.render_offline_toml(summary)
    except ValueError as exc:
        assert "--deployment-transaction-hash" in str(exc)
    else:
        raise AssertionError("source live TOML rendered without deployment receipt evidence")


def test_evm_source_live_rejects_receipt_transaction_hash_drift():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        receipt_transaction_hash=bytes.fromhex("ef" * 32),
    )

    try:
        module.collect_source_bridge_evidence(
            "https://ethereum.example",
            domain=module.SCCP_DOMAIN_ETH,
            bridge_address=fake.bridge,
            block_tag="latest",
            deployment_transaction_hash=bytes.fromhex("de" * 32),
            opener=fake.opener,
            timeout=1.0,
        )
    except RuntimeError as exc:
        assert "transactionHash does not match" in str(exc)
    else:
        raise AssertionError("drifted deployment receipt transactionHash was accepted")


def test_evm_source_live_redacts_receipt_field_parser_exception_causes(monkeypatch):
    module = load_live_module()
    original_rpc_fixed_hex_data = module._rpc_fixed_hex_data
    cases = (
        (
            "eth_getTransactionReceipt transactionHash",
            "deployment receipt transactionHash must be a non-zero bytes32",
        ),
        (
            "eth_getTransactionReceipt contractAddress",
            "deployment receipt contractAddress must be a non-zero 20-byte EVM address",
        ),
        (
            "eth_getTransactionReceipt blockHash",
            "deployment receipt blockHash must be a non-zero bytes32",
        ),
    )

    for target_method, expected_message in cases:
        for exception_type in (TypeError, RuntimeError, ValueError):
            fake = fake_opener_for(module)

            def fail_target_receipt_field(
                result,
                *,
                method,
                byte_length,
                nonzero=True,
                exception_type=exception_type,
                target_method=target_method,
            ):
                if method == target_method:
                    raise exception_type(
                        f"secret-token {target_method} parser detail"
                    )
                return original_rpc_fixed_hex_data(
                    result,
                    method=method,
                    byte_length=byte_length,
                    nonzero=nonzero,
                )

            with monkeypatch.context() as patch:
                patch.setattr(module, "_rpc_fixed_hex_data", fail_target_receipt_field)
                try:
                    module.collect_source_bridge_evidence(
                        "https://ethereum.example",
                        domain=module.SCCP_DOMAIN_ETH,
                        bridge_address=fake.bridge,
                        block_tag="latest",
                        deployment_transaction_hash=bytes.fromhex("de" * 32),
                        opener=fake.opener,
                        timeout=1.0,
                    )
                except RuntimeError as exc:
                    rendered = str(exc)
                    assert rendered == expected_message
                    assert "secret-token" not in rendered
                    assert "parser detail" not in rendered
                    assert exception_type.__name__ not in rendered
                    assert exc.__cause__ is None
                    assert exc.__suppress_context__ is True
                else:
                    raise AssertionError(f"{target_method} parser detail was accepted")


def test_evm_source_live_rejects_deployment_transaction_readback_drift():
    module = load_live_module()
    cases = (
        (
            fake_opener_for(
                module,
                deployment_transaction_hash=bytes.fromhex("ef" * 32),
            ),
            "transaction hash does not match",
        ),
        (
            fake_opener_for(
                module,
                deployment_transaction_block_hash="0x" + "98" * 32,
            ),
            "transaction blockHash does not match",
        ),
        (
            fake_opener_for(module, deployment_transaction_block_number="0x1235"),
            "transaction blockNumber does not match",
        ),
        (
            fake_opener_for(module, deployment_transaction_to="0x" + "22" * 20),
            "transaction to must be null",
        ),
        (
            fake_opener_for(module, deployment_transaction_input="0x"),
            "transaction input",
        ),
        (
            fake_opener_for(module, deployment_transaction_input="0x" + "00" * 4),
            "transaction input",
        ),
    )

    for fake, expected in cases:
        try:
            module.collect_source_bridge_evidence(
                "https://ethereum.example",
                domain=module.SCCP_DOMAIN_ETH,
                bridge_address=fake.bridge,
                block_tag="latest",
                deployment_transaction_hash=bytes.fromhex("de" * 32),
                opener=fake.opener,
                timeout=1.0,
            )
        except RuntimeError as exc:
            assert expected in str(exc)
        else:
            raise AssertionError(
                "drifted deployment transaction readback was accepted"
            )


def test_evm_source_live_rejects_missing_or_drifted_receipt_contract_address():
    module = load_live_module()
    cases = (
        fake_opener_for(module, omit_receipt_contract_address=True),
        fake_opener_for(module, receipt_contract_address="0x"),
        fake_opener_for(module, receipt_contract_address="0x" + "22" * 20),
    )

    for fake in cases:
        try:
            module.collect_source_bridge_evidence(
                "https://ethereum.example",
                domain=module.SCCP_DOMAIN_ETH,
                bridge_address=fake.bridge,
                block_tag="latest",
                deployment_transaction_hash=bytes.fromhex("de" * 32),
                opener=fake.opener,
                timeout=1.0,
            )
        except RuntimeError as exc:
            assert "contractAddress" in str(exc)
        else:
            raise AssertionError(
                "deployment receipt without matching contractAddress was accepted"
            )


def test_evm_source_live_rejects_missing_or_zero_receipt_block_number():
    module = load_live_module()
    cases = (
        fake_opener_for(module, receipt_block_number=None),
        fake_opener_for(module, receipt_block_number="0x0"),
    )

    for fake in cases:
        try:
            module.collect_source_bridge_evidence(
                "https://ethereum.example",
                domain=module.SCCP_DOMAIN_ETH,
                bridge_address=fake.bridge,
                block_tag="latest",
                deployment_transaction_hash=bytes.fromhex("de" * 32),
                opener=fake.opener,
                timeout=1.0,
            )
        except RuntimeError as exc:
            assert "blockNumber" in str(exc)
        else:
            raise AssertionError(
                "deployment receipt without non-zero blockNumber was accepted"
            )


def test_evm_source_live_rejects_noncanonical_receipt_block_hash():
    module = load_live_module()
    fake = fake_opener_for(module, receipt_block_hash="0x" + "AA" * 32)

    try:
        module.collect_source_bridge_evidence(
            "https://ethereum.example",
            domain=module.SCCP_DOMAIN_ETH,
            bridge_address=fake.bridge,
            block_tag="latest",
            deployment_transaction_hash=bytes.fromhex("de" * 32),
            opener=fake.opener,
            timeout=1.0,
        )
    except RuntimeError as exc:
        assert "blockHash" in str(exc)
    else:
        raise AssertionError("noncanonical deployment receipt blockHash was accepted")


def test_evm_source_live_rejects_receipt_block_hash_drift():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        domain=module.SCCP_DOMAIN_BSC,
        rpc_chain_id=56,
        receipt_block_hash="0x" + "99" * 32,
        block_response_hash="0x" + "98" * 32,
    )

    try:
        module.collect_source_bridge_evidence(
            "https://ethereum.example",
            domain=module.SCCP_DOMAIN_BSC,
            bridge_address=fake.bridge,
            block_tag="latest",
            deployment_transaction_hash=bytes.fromhex("de" * 32),
            opener=fake.opener,
            timeout=1.0,
        )
    except RuntimeError as exc:
        assert "blockHash does not match eth_getBlockByNumber" in str(exc)
    else:
        raise AssertionError("drifted deployment receipt blockHash was accepted")


def test_evm_source_live_rejects_receipt_block_number_drift():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        receipt_block_number="0x1234",
        block_response_number="0x1235",
    )

    try:
        module.collect_source_bridge_evidence(
            "https://ethereum.example",
            domain=module.SCCP_DOMAIN_ETH,
            bridge_address=fake.bridge,
            block_tag="latest",
            deployment_transaction_hash=bytes.fromhex("de" * 32),
            opener=fake.opener,
            timeout=1.0,
        )
    except RuntimeError as exc:
        assert "block number does not match eth_getBlockByNumber" in str(exc)
    else:
        raise AssertionError("drifted deployment receipt block number was accepted")


def test_evm_source_live_rejects_unfinalized_deployment_receipt_block():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        finalized_block_number="0x1233",
    )

    try:
        module.collect_source_bridge_evidence(
            "https://ethereum.example",
            domain=module.SCCP_DOMAIN_ETH,
            bridge_address=fake.bridge,
            block_tag="finalized",
            deployment_transaction_hash=bytes.fromhex("de" * 32),
            opener=fake.opener,
            timeout=1.0,
        )
    except RuntimeError as exc:
        assert "newer than the finalized execution block" in str(exc)
    else:
        raise AssertionError("unfinalized Ethereum source deployment receipt was accepted")


def test_evm_source_live_rejects_finalized_deployment_receipt_hash_drift():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        finalized_block_hash="0x" + "98" * 32,
    )

    try:
        module.collect_source_bridge_evidence(
            "https://ethereum.example",
            domain=module.SCCP_DOMAIN_ETH,
            bridge_address=fake.bridge,
            block_tag="finalized",
            deployment_transaction_hash=bytes.fromhex("de" * 32),
            opener=fake.opener,
            timeout=1.0,
        )
    except RuntimeError as exc:
        assert "does not match the finalized execution block" in str(exc)
    else:
        raise AssertionError("finalized Ethereum source deployment hash drift was accepted")


def test_evm_source_live_rejects_zero_receipt_block_receipts_root():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        block_response_receipts_root="0x" + "00" * 32,
    )

    try:
        module.collect_source_bridge_evidence(
            "https://ethereum.example",
            domain=module.SCCP_DOMAIN_ETH,
            bridge_address=fake.bridge,
            block_tag="latest",
            deployment_transaction_hash=bytes.fromhex("de" * 32),
            opener=fake.opener,
            timeout=1.0,
        )
    except RuntimeError as exc:
        assert "receiptsRoot" in str(exc)
    else:
        raise AssertionError("zero deployment receipt block receiptsRoot was accepted")


def test_evm_source_live_rejects_receipt_block_code_hash_drift():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        receipt_block_runtime=bytes.fromhex("60806040526004"),
    )

    try:
        module.collect_source_bridge_evidence(
            "https://ethereum.example",
            domain=module.SCCP_DOMAIN_ETH,
            bridge_address=fake.bridge,
            block_tag="latest",
            deployment_transaction_hash=bytes.fromhex("de" * 32),
            opener=fake.opener,
            timeout=1.0,
        )
    except RuntimeError as exc:
        assert "deployment receipt block" in str(exc)
    else:
        raise AssertionError("drifted deployment receipt block bytecode was accepted")


def test_evm_source_live_toml_requires_independent_pins():
    module = load_live_module()
    fake = fake_opener_for(module)
    args, material_hash, deployment_hash = source_args(module, fake)

    summary = module.collect_live_evidence(
        SimpleNamespace(
            rpc_url="https://ethereum.example",
            domain=module.SCCP_DOMAIN_ETH,
            bridge_address=fake.bridge,
            expected_rpc_chain_id=1,
            expected_source_bridge_code_hash=fake.bridge_code_hash,
            deployment_transaction_hash=bytes.fromhex("de" * 32),
            source_trust_anchor_hash=args.source_trust_anchor_hash,
            consensus_verifier_hash=args.consensus_verifier_hash,
            message_inclusion_verifier_hash=args.message_inclusion_verifier_hash,
            finality_policy_hash=args.finality_policy_hash,
            adapter_verifier_vk_hash=args.adapter_verifier_vk_hash,
            deployment_receipt_hash=args.deployment_receipt_hash,
            expected_source_verifier_material_hash=None,
            expected_source_adapter_engine_deployment_hash=deployment_hash,
            block_tag="latest",
            timeout=1.0,
        ),
        opener=fake.opener,
    )
    assert "offline_toml_sha256" not in summary
    try:
        module.render_offline_toml(summary)
    except ValueError as exc:
        assert "expected-source-verifier-material-hash" in str(exc)
    else:
        raise AssertionError("source live TOML rendered without material pin")

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                rpc_url="https://ethereum.example",
                domain=module.SCCP_DOMAIN_ETH,
                bridge_address=fake.bridge,
                expected_rpc_chain_id=1,
                expected_source_bridge_code_hash=fake.bridge_code_hash,
                deployment_transaction_hash=bytes.fromhex("de" * 32),
                source_trust_anchor_hash=args.source_trust_anchor_hash,
                consensus_verifier_hash=args.consensus_verifier_hash,
                message_inclusion_verifier_hash=args.message_inclusion_verifier_hash,
                finality_policy_hash=args.finality_policy_hash,
                adapter_verifier_vk_hash=args.adapter_verifier_vk_hash,
                deployment_receipt_hash=args.deployment_receipt_hash,
                expected_source_verifier_material_hash=bytes.fromhex("cc" * 32),
                expected_source_adapter_engine_deployment_hash=deployment_hash,
                block_tag="latest",
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "expected-source-verifier-material-hash" in str(exc)
        assert material_hash.hex() in str(exc)
    else:
        raise AssertionError("drifted source material hash was accepted")


def test_evm_source_live_cli_json_and_toml_outputs(capsys):
    module = load_live_module()
    fake = fake_opener_for(module)
    args, material_hash, deployment_hash = source_args(module, fake)
    original_collect = module.collect_live_evidence

    def collect_with_fake(parsed_args):
        return original_collect(parsed_args, opener=fake.opener)

    module.collect_live_evidence = collect_with_fake
    cli_args = [
        "--rpc-url",
        "https://ethereum.example",
        "--domain",
        "eth",
        "--bridge-address",
        fake.bridge,
        "--expected-rpc-chain-id",
        "1",
        "--expected-source-bridge-code-hash",
        "0x" + fake.bridge_code_hash.hex(),
        "--deployment-transaction-hash",
        "0x" + "de" * 32,
        "--source-trust-anchor-hash",
        "0x" + args.source_trust_anchor_hash.hex(),
        "--consensus-verifier-hash",
        "0x" + args.consensus_verifier_hash.hex(),
        "--message-inclusion-verifier-hash",
        "0x" + args.message_inclusion_verifier_hash.hex(),
        "--finality-policy-hash",
        "0x" + args.finality_policy_hash.hex(),
        "--adapter-verifier-vk-hash",
        "0x" + args.adapter_verifier_vk_hash.hex(),
        "--deployment-receipt-hash",
        "0x" + args.deployment_receipt_hash.hex(),
        "--expected-source-verifier-material-hash",
        "0x" + material_hash.hex(),
        "--expected-source-adapter-engine-deployment-hash",
        "0x" + deployment_hash.hex(),
    ]
    try:
        assert module.main(cli_args) == 0
        output = json.loads(capsys.readouterr().out)
        assert output["source_bridge"]["bridge_code_hash"] == (
            "0x" + fake.bridge_code_hash.hex()
        )
        assert output["source_bridge"]["bridge_runtime_bytecode_hex"] == (
            "0x" + fake.bridge_runtime.hex()
        )

        assert module.main([*cli_args, "--toml"]) == 0
        rendered = capsys.readouterr().out
        assert "[[zk.sccp_source_verifier_materials]]" in rendered
        assert "[[zk.sccp_source_adapter_engine_deployments]]" in rendered
        assert "# sccp_evm_source_block_tag" in rendered
        assert "# sccp_evm_source_bridge_runtime_bytecode_hex" in rendered
    finally:
        module.collect_live_evidence = original_collect
