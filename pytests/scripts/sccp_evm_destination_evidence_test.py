import json
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
from types import SimpleNamespace


ETH_DESTINATION_BINDING_VECTOR = (
    "c86f9d904df50c4522d01da3773916ebecce816f3fdfa664e2dff7cfbe697c45"
)
BSC_DESTINATION_BINDING_VECTOR = (
    "5e97d6da2b4ca7d64171ae717cfa31340a736c125485812a7cb9641570bc27d6"
)
BSC_TESTNET_DESTINATION_BINDING_VECTOR = (
    "16eb6817844e492f8fea4fc4742b9e464a80ae392f25d5e6fad9960d49414dcc"
)
ETH_MAINNET_NETWORK_ID = "00" * 31 + "01"
BSC_MAINNET_NETWORK_ID = "00" * 31 + "38"
BSC_TESTNET_NETWORK_ID = "00" * 31 + "61"
EVM_SOURCE_VERIFIER_MATERIAL_HASH = "aa" * 32
EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH = "99" * 32
ETH_ROUTE_ALLOWLIST_HASH_VECTOR = (
    "3bf99a87cc501ee17858c86eaea872a7e4a75d60bfd01e872cdd5a843895ea6e"
)
BSC_ROUTE_ALLOWLIST_HASH_VECTOR = (
    "03492e28a9c71c56c7702eb438b5aff0df0f5e263a6173f3b950a7b45cc1bda6"
)
BSC_TESTNET_ROUTE_ALLOWLIST_HASH_VECTOR = (
    "27573a75bd6d18056533bcf09049f155f2966553124219d0a464d1c9953cc4a7"
)
EVM_ROUTE_CANARY_EVIDENCE_HASH = "e1" * 32
ETH_ROUTE_CANARY_TRANSACTION_HASH_VECTOR = (
    "84b93b0050b6bc9696ba55d56a8c957171e6a4ebd2f242b683762d52d88db9d7"
)
BSC_ROUTE_CANARY_TRANSACTION_HASH_VECTOR = (
    "66a7bdfe287e79a350688ca84699cde4df4c6cbf38926f0ac4f027c7a2c43744"
)
BSC_TESTNET_ROUTE_CANARY_TRANSACTION_HASH_VECTOR = (
    "903b4afe339398216c02663eea270634494a1f12b166dc322d9d4d9c1c3e544b"
)


def load_evidence_module():
    script_path = (
        Path(__file__).resolve().parents[2]
        / "scripts"
        / "sccp_evm_destination_evidence.py"
    )
    spec = spec_from_file_location("sccp_evm_destination_evidence", script_path)
    module = module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def test_evm_destination_cli_redacts_top_level_exception_details(monkeypatch, capsys):
    module = load_evidence_module()

    for exception_type in (OSError, RuntimeError, TypeError, ValueError):

        def fail_scope(_args, exception_type=exception_type):
            raise exception_type("secret-token /tmp/operator/private-path")

        with monkeypatch.context() as patch:
            patch.setattr(module, "validate_bsc_network_scope", fail_scope)
            try:
                module.main(
                    [
                        "--domain",
                        "eth",
                        "--verifier-address",
                        "0x" + "11" * 20,
                        "--bridge-address",
                        "0x" + "22" * 20,
                        "--verifier-key-hash",
                        "0x" + "cc" * 32,
                    ]
                )
            except SystemExit as exc:
                assert exc.code == 2
            else:
                raise AssertionError(
                    "EVM destination CLI accepted top-level render failure"
                )

            captured = capsys.readouterr()
            assert "SCCP EVM destination evidence rendering failed" in captured.err
            assert "secret-token" not in captured.err
            assert "private-path" not in captured.err
            assert exception_type.__name__ not in captured.err


def evm_runtime_material(module, *, domain=1, bsc_network="mainnet"):
    bridge_runtime = bytes.fromhex("6001600255")
    verifier_runtime = bytes.fromhex("6080604052")
    network_id = module.evm_network_id_for_domain(domain, bsc_network=bsc_network)
    verifier_address = bytes.fromhex("11" * 20)
    bridge_address = bytes.fromhex("22" * 20)
    verifier_key_hash = bytes.fromhex("cc" * 32)
    bridge_code_hash = module.runtime_bytecode_hash(bridge_runtime)
    verifier_code_hash = module.runtime_bytecode_hash(verifier_runtime)
    destination_binding_hash = module.evm_destination_binding_hash(
        network_id=network_id,
        source_domain=0,
        target_domain=domain,
        verifier_address=verifier_address,
        bridge_address=bridge_address,
        verifier_code_hash=verifier_code_hash,
        verifier_key_hash=verifier_key_hash,
        bsc_network=bsc_network,
    )
    route_allowlist_hash = module.evm_route_allowlist_hash(
        domain=domain,
        source_verifier_material_hash=bytes.fromhex(EVM_SOURCE_VERIFIER_MATERIAL_HASH),
        source_adapter_engine_deployment_hash=bytes.fromhex(
            EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
        ),
        destination_binding_hash=destination_binding_hash,
        bsc_network=bsc_network,
    )
    route_canary_transaction_hash = bytes.fromhex("44" * 32)
    route_canary_receipt_block_number = 0x1234
    route_canary_receipt_block_hash = bytes.fromhex("45" * 32)
    route_canary_block_receipts_root = bytes.fromhex("46" * 32)
    route_canary_message_id = bytes.fromhex("55" * 32)
    route_canary_call_data_sha256 = bytes.fromhex("88" * 32)
    route_canary_payload_hash = bytes.fromhex("99" * 32)
    route_canary_statement_hash = bytes.fromhex("66" * 32)
    route_canary_commitment_root = bytes.fromhex("77" * 32)
    route_canary_finality_height = bytes.fromhex("aa" * 32)
    route_canary_finality_block_hash = bytes.fromhex("ab" * 32)
    route_canary_evidence_hash = module.evm_route_canary_transaction_evidence_hash(
        route_allowlist_hash=route_allowlist_hash,
        bridge_address=bridge_address,
        transaction_hash=route_canary_transaction_hash,
        log_index=0,
        receipt_block_number=route_canary_receipt_block_number,
        receipt_block_hash=route_canary_receipt_block_hash,
        block_receipts_root=route_canary_block_receipts_root,
        call_data_sha256=route_canary_call_data_sha256,
        message_id=route_canary_message_id,
        payload_hash=route_canary_payload_hash,
        source_domain=0,
        target_domain=domain,
        commitment_root=route_canary_commitment_root,
        finality_height=route_canary_finality_height,
        finality_block_hash=route_canary_finality_block_hash,
        statement_hash=route_canary_statement_hash,
        proof_version=1,
        proof_source_domain=0,
        destination_binding_hash=destination_binding_hash,
        verifier_backend_hash=module.evm_verifier_backend_hash(),
        proof_family_hash=module.evm_proof_family_hash(),
        network_id=network_id,
        used_message_proof=True,
        receipt_block_finalized=True,
        bsc_network=bsc_network,
    )
    return SimpleNamespace(
        domain=domain,
        bsc_network=bsc_network,
        network_id=network_id,
        verifier_address=verifier_address,
        bridge_address=bridge_address,
        bridge_runtime=bridge_runtime,
        verifier_runtime=verifier_runtime,
        bridge_code_hash=bridge_code_hash,
        verifier_code_hash=verifier_code_hash,
        verifier_key_hash=verifier_key_hash,
        destination_binding_hash=destination_binding_hash,
        route_allowlist_hash=route_allowlist_hash,
        route_canary_transaction_hash=route_canary_transaction_hash,
        route_canary_transaction_block_number=route_canary_receipt_block_number,
        route_canary_transaction_block_hash=route_canary_receipt_block_hash,
        route_canary_log_index=0,
        route_canary_receipt_block_number=route_canary_receipt_block_number,
        route_canary_receipt_block_hash=route_canary_receipt_block_hash,
        route_canary_block_receipts_root=route_canary_block_receipts_root,
        route_canary_call_data_sha256=route_canary_call_data_sha256,
        route_canary_message_id=route_canary_message_id,
        route_canary_payload_hash=route_canary_payload_hash,
        route_canary_target_domain=domain,
        route_canary_statement_hash=route_canary_statement_hash,
        route_canary_commitment_root=route_canary_commitment_root,
        route_canary_finality_height=route_canary_finality_height,
        route_canary_finality_block_hash=route_canary_finality_block_hash,
        route_canary_proof_version=1,
        route_canary_proof_source_domain=0,
        route_canary_evidence_hash=route_canary_evidence_hash,
        route_canary_receipt_block_finalized=True,
    )


def add_route_canary_args(args, material):
    args.route_canary_evidence_hash = material.route_canary_evidence_hash
    args.route_canary_transaction_hash = material.route_canary_transaction_hash
    args.route_canary_transaction_block_number = (
        material.route_canary_transaction_block_number
    )
    args.route_canary_transaction_block_hash = material.route_canary_transaction_block_hash
    args.route_canary_log_index = material.route_canary_log_index
    args.route_canary_receipt_block_number = material.route_canary_receipt_block_number
    args.route_canary_receipt_block_hash = material.route_canary_receipt_block_hash
    args.route_canary_block_receipts_root = material.route_canary_block_receipts_root
    args.route_canary_call_data_sha256 = material.route_canary_call_data_sha256
    args.route_canary_message_id = material.route_canary_message_id
    args.route_canary_payload_hash = material.route_canary_payload_hash
    args.route_canary_target_domain = material.route_canary_target_domain
    args.route_canary_statement_hash = material.route_canary_statement_hash
    args.route_canary_commitment_root = material.route_canary_commitment_root
    args.route_canary_finality_height = material.route_canary_finality_height
    args.route_canary_finality_block_hash = material.route_canary_finality_block_hash
    args.route_canary_proof_version = material.route_canary_proof_version
    args.route_canary_proof_source_domain = material.route_canary_proof_source_domain
    args.route_canary_used_message_proof = True
    args.route_canary_receipt_block_finalized = (
        material.route_canary_receipt_block_finalized
    )
    return args


def full_toml_args(material):
    return add_route_canary_args(
        SimpleNamespace(
            domain=material.domain,
            bsc_network=material.bsc_network,
            network_id=material.network_id,
            verifier_address=material.verifier_address,
            bridge_address=material.bridge_address,
            bridge_code_hash=material.bridge_code_hash,
            bridge_runtime_bytecode_hex=material.bridge_runtime,
            verifier_code_hash=material.verifier_code_hash,
            verifier_runtime_bytecode_hex=material.verifier_runtime,
            verifier_key_hash=material.verifier_key_hash,
            expected_destination_binding_hash=material.destination_binding_hash,
            source_verifier_material_hash=bytes.fromhex(
                EVM_SOURCE_VERIFIER_MATERIAL_HASH
            ),
            source_adapter_engine_deployment_hash=bytes.fromhex(
                EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
            ),
            route_allowlist_hash=material.route_allowlist_hash,
        ),
        material,
    )


def test_evm_destination_domain_parser_accepts_eth_and_bsc_only():
    module = load_evidence_module()

    assert module.parse_destination_domain("eth") == 1
    assert module.parse_destination_domain("ethereum") == 1
    assert module.parse_destination_domain("1") == 1
    assert module.parse_destination_domain("bsc") == 2
    assert module.parse_destination_domain("bnb") == 2
    assert module.parse_destination_domain("2") == 2

    try:
        module.parse_destination_domain("tron")
    except module.argparse.ArgumentTypeError as exc:
        assert "domain must be eth or bsc" in str(exc)
    else:
        raise AssertionError("non-EVM destination domain was accepted")

    try:
        module.parse_destination_domain(" eth ")
    except module.argparse.ArgumentTypeError as exc:
        assert "domain must be eth or bsc" in str(exc)
    else:
        raise AssertionError("padded EVM destination domain was accepted")

    assert module.parse_bsc_network("mainnet") == "mainnet"
    assert module.parse_bsc_network("bsc-mainnet") == "mainnet"
    assert module.parse_bsc_network("56") == "mainnet"
    assert module.parse_bsc_network("testnet") == "testnet"
    assert module.parse_bsc_network("bsc-testnet") == "testnet"
    assert module.parse_bsc_network("chapel") == "testnet"
    assert module.parse_bsc_network("97") == "testnet"

    for value in (" testnet ", "nile", "bnb-testnet"):
        try:
            module.parse_bsc_network(value)
        except module.argparse.ArgumentTypeError as exc:
            assert "BSC network must be mainnet or testnet" in str(exc)
        else:
            raise AssertionError(f"non-canonical BSC network {value!r} was accepted")


def test_evm_destination_domain_parsers_redact_parser_causes():
    module = load_evidence_module()

    try:
        module.parse_destination_domain("secret-token-evm-destination-domain")
    except module.argparse.ArgumentTypeError as exc:
        rendered = str(exc)
        assert rendered == "domain must be eth or bsc"
        assert "secret-token" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("secret EVM destination domain was accepted")

    try:
        module.parse_bsc_network("secret-token-evm-destination-bsc-network")
    except module.argparse.ArgumentTypeError as exc:
        rendered = str(exc)
        assert rendered == "BSC network must be mainnet or testnet"
        assert "secret-token" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("secret EVM destination BSC network was accepted")


def test_evm_address_and_hash_parsers_reject_zero_and_wrong_width(tmp_path):
    module = load_evidence_module()

    assert module.parse_evm_address(
        "0x" + "11" * 20,
        label="verifier address",
    ) == bytes.fromhex("11" * 20)
    assert module.parse_hex_bytes(
        "0x" + "33" * 32,
        label="network id",
        byte_length=32,
    ) == bytes.fromhex("33" * 32)
    assert module.parse_runtime_bytecode_hex(
        "0x6080604052",
        label="bridge runtime bytecode",
    ) == bytes.fromhex("6080604052")

    for value, expected in (
        ("0X" + "11" * 20, "lowercase 0x prefix"),
        ("0x" + "AA" * 20, "lowercase hex"),
    ):
        try:
            module.parse_evm_address(value, label="verifier address")
        except module.argparse.ArgumentTypeError as exc:
            assert expected in str(exc)
        else:
            raise AssertionError("non-canonical EVM address was accepted")

    for value, expected in (
        ("0X" + "33" * 32, "lowercase 0x prefix"),
        ("0x" + "AA" * 32, "lowercase hex"),
    ):
        try:
            module.parse_hex_bytes(value, label="network id", byte_length=32)
        except module.argparse.ArgumentTypeError as exc:
            assert expected in str(exc)
        else:
            raise AssertionError("non-canonical EVM network id was accepted")

    for value in (" 0x6080604052", "0x6080\n604052"):
        try:
            module.parse_runtime_bytecode_hex(
                value,
                label="bridge runtime bytecode",
            )
        except module.argparse.ArgumentTypeError as exc:
            assert "must not contain whitespace" in str(exc)
        else:
            raise AssertionError("padded EVM runtime bytecode was accepted")

    for value, expected in (
        ("0X6080604052", "lowercase 0x prefix"),
        ("0x60806040AB", "lowercase hex"),
    ):
        try:
            module.parse_runtime_bytecode_hex(
                value,
                label="bridge runtime bytecode",
            )
        except module.argparse.ArgumentTypeError as exc:
            assert expected in str(exc)
        else:
            raise AssertionError("non-canonical EVM runtime bytecode was accepted")

    runtime_file = tmp_path / "runtime.hex"
    runtime_file.write_text("0x6080\n604052\n", encoding="ascii")
    assert module.parse_runtime_bytecode_file(
        str(runtime_file),
        label="bridge runtime bytecode",
    ) == bytes.fromhex("6080604052")

    try:
        module.parse_evm_address("0x" + "00" * 20, label="verifier address")
    except module.argparse.ArgumentTypeError as exc:
        assert "must not be zero" in str(exc)
    else:
        raise AssertionError("zero EVM address was accepted")

    try:
        module.parse_evm_address(
            " 0x" + "11" * 20 + " ",
            label="verifier address",
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must not contain whitespace" in str(exc)
    else:
        raise AssertionError("padded EVM address was accepted")

    try:
        module.parse_hex_bytes("0x" + "33" * 31, label="network id", byte_length=32)
    except module.argparse.ArgumentTypeError as exc:
        assert "must be 32 bytes" in str(exc)
    else:
        raise AssertionError("short EVM network id was accepted")


def test_evm_destination_direct_parsers_redact_parser_causes(tmp_path):
    module = load_evidence_module()

    fixed_payload = "secret-token-evm-destination-hex"
    try:
        module.parse_hex_bytes(
            "0x" + fixed_payload + ("a" * (64 - len(fixed_payload))),
            label="network id",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        rendered = str(exc)
        assert rendered == "network id must be hex"
        assert "secret-token" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("secret EVM destination fixed hex was accepted")

    runtime_payload = "secret-token-evm-destination-runtime"
    try:
        module.parse_runtime_bytecode_hex(
            "0x" + runtime_payload,
            label="bridge runtime bytecode",
        )
    except module.argparse.ArgumentTypeError as exc:
        rendered = str(exc)
        assert rendered == "bridge runtime bytecode must be hex"
        assert "secret-token" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("secret EVM destination runtime hex was accepted")

    secret_path = tmp_path / "secret-token-evm-destination-file-path.hex"
    try:
        module.parse_runtime_bytecode_file(
            str(secret_path),
            label="bridge runtime bytecode",
        )
    except module.argparse.ArgumentTypeError as exc:
        rendered = str(exc)
        assert rendered == "bridge runtime bytecode file cannot be read"
        assert "secret-token" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("missing secret EVM destination runtime file was accepted")


def test_evm_destination_direct_parsers_redact_typeerror_parser_causes(monkeypatch):
    module = load_evidence_module()

    class SecretBytes:
        @staticmethod
        def fromhex(_text):
            raise TypeError("secret-token EVM destination hex TypeError detail")

    monkeypatch.setattr(module, "bytes", SecretBytes, raising=False)

    for parser, value, label, kwargs in (
        (
            module.parse_hex_bytes,
            "0x" + "11" * 32,
            "network id",
            {"byte_length": 32},
        ),
        (
            module.parse_runtime_bytecode_hex,
            "0x6001600055",
            "bridge runtime bytecode",
            {},
        ),
    ):
        try:
            parser(value, label=label, **kwargs)
        except module.argparse.ArgumentTypeError as exc:
            rendered = str(exc)
            assert rendered == f"{label} must be hex"
            assert "secret-token" not in rendered
            assert "TypeError" not in rendered
            assert exc.__cause__ is None
            assert exc.__suppress_context__ is True
        else:
            raise AssertionError(f"{label} parser TypeError was accepted")


def test_evm_destination_binding_hash_matches_vectors_and_domain_separates():
    module = load_evidence_module()
    common_eth = {
        "network_id": bytes.fromhex(ETH_MAINNET_NETWORK_ID),
        "source_domain": 0,
        "verifier_address": bytes.fromhex("11" * 20),
        "bridge_address": bytes.fromhex("22" * 20),
        "verifier_code_hash": bytes.fromhex("bb" * 32),
        "verifier_key_hash": bytes.fromhex("cc" * 32),
    }
    common_bsc = {**common_eth, "network_id": bytes.fromhex(BSC_MAINNET_NETWORK_ID)}
    common_bsc_testnet = {
        **common_eth,
        "network_id": bytes.fromhex(BSC_TESTNET_NETWORK_ID),
    }

    eth_hash = module.evm_destination_binding_hash(target_domain=1, **common_eth)
    bsc_hash = module.evm_destination_binding_hash(target_domain=2, **common_bsc)
    bsc_testnet_hash = module.evm_destination_binding_hash(
        target_domain=2,
        bsc_network="testnet",
        **common_bsc_testnet,
    )
    eth_key = module.evm_destination_binding_key(target_domain=1, **common_eth)
    bsc_testnet_key = module.evm_destination_binding_key(
        target_domain=2,
        bsc_network="testnet",
        **common_bsc_testnet,
    )

    assert eth_hash.hex() == ETH_DESTINATION_BINDING_VECTOR
    assert bsc_hash.hex() == BSC_DESTINATION_BINDING_VECTOR
    assert bsc_testnet_hash.hex() == BSC_TESTNET_DESTINATION_BINDING_VECTOR
    assert module.evm_network_id_for_domain(2).hex() == BSC_MAINNET_NETWORK_ID
    assert (
        module.evm_network_id_for_domain(2, bsc_network="testnet").hex()
        == BSC_TESTNET_NETWORK_ID
    )
    assert eth_hash != bsc_hash
    assert bsc_hash != bsc_testnet_hash
    assert eth_key == (
        "evm:0:1:"
        + ETH_MAINNET_NETWORK_ID
        + ":0x"
        + "11" * 20
        + ":0x"
        + "22" * 20
        + ":0x"
        + "bb" * 32
        + ":0x"
        + "cc" * 32
    )
    assert bsc_testnet_key == (
        "evm:0:2:"
        + BSC_TESTNET_NETWORK_ID
        + ":0x"
        + "11" * 20
        + ":0x"
        + "22" * 20
        + ":0x"
        + "bb" * 32
        + ":0x"
        + "cc" * 32
    )

    try:
        module.evm_destination_binding_hash(
            target_domain=2,
            bsc_network="testnet",
            **common_bsc,
        )
    except ValueError as exc:
        assert "chain id 97" in str(exc)
    else:
        raise AssertionError("BSC testnet binding accepted mainnet network id")

    try:
        module.evm_destination_binding_hash(
            target_domain=2,
            **common_bsc_testnet,
        )
    except ValueError as exc:
        assert "chain id 56" in str(exc)
    else:
        raise AssertionError("BSC mainnet binding accepted testnet network id")


def test_evm_destination_domain_wrappers_redact_nested_causes():
    module = load_evidence_module()
    common = {
        "network_id": bytes.fromhex(ETH_MAINNET_NETWORK_ID),
        "source_domain": 0,
        "target_domain": module.SCCP_DOMAIN_SORA,
        "verifier_address": bytes.fromhex("11" * 20),
        "bridge_address": bytes.fromhex("22" * 20),
        "verifier_code_hash": bytes.fromhex("bb" * 32),
        "verifier_key_hash": bytes.fromhex("cc" * 32),
    }

    for call, expected in (
        (
            lambda: module.profile_for_domain(module.SCCP_DOMAIN_SORA),
            "domain must be ETH or BSC",
        ),
        (
            lambda: module.evm_destination_binding_hash(**common),
            "target_domain must be ETH or BSC",
        ),
        (
            lambda: module.evm_destination_binding_key(**common),
            "target_domain must be ETH or BSC",
        ),
    ):
        try:
            call()
        except ValueError as exc:
            rendered = str(exc)
            assert rendered == expected
            assert exc.__cause__ is None
            assert exc.__suppress_context__ is True
        else:
            raise AssertionError(f"EVM destination wrapper accepted {expected}")


def test_evm_route_allowlist_hash_matches_lane_evidence_vectors():
    module = load_evidence_module()

    eth_hash = module.evm_route_allowlist_hash(
        domain=1,
        source_verifier_material_hash=bytes.fromhex(EVM_SOURCE_VERIFIER_MATERIAL_HASH),
        source_adapter_engine_deployment_hash=bytes.fromhex(
            EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
        ),
        destination_binding_hash=bytes.fromhex(ETH_DESTINATION_BINDING_VECTOR),
    )
    bsc_hash = module.evm_route_allowlist_hash(
        domain=2,
        source_verifier_material_hash=bytes.fromhex(EVM_SOURCE_VERIFIER_MATERIAL_HASH),
        source_adapter_engine_deployment_hash=bytes.fromhex(
            EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
        ),
        destination_binding_hash=bytes.fromhex(BSC_DESTINATION_BINDING_VECTOR),
    )
    bsc_testnet_hash = module.evm_route_allowlist_hash(
        domain=2,
        bsc_network="testnet",
        source_verifier_material_hash=bytes.fromhex(EVM_SOURCE_VERIFIER_MATERIAL_HASH),
        source_adapter_engine_deployment_hash=bytes.fromhex(
            EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
        ),
        destination_binding_hash=bytes.fromhex(BSC_TESTNET_DESTINATION_BINDING_VECTOR),
    )

    assert eth_hash.hex() == ETH_ROUTE_ALLOWLIST_HASH_VECTOR
    assert bsc_hash.hex() == BSC_ROUTE_ALLOWLIST_HASH_VECTOR
    assert bsc_testnet_hash.hex() == BSC_TESTNET_ROUTE_ALLOWLIST_HASH_VECTOR
    assert eth_hash != bsc_hash
    assert bsc_hash != bsc_testnet_hash

    for replayed in (
        {
            "source_verifier_material_hash": bytes.fromhex(
                EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
            ),
            "source_adapter_engine_deployment_hash": bytes.fromhex(
                EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
            ),
            "destination_binding_hash": bytes.fromhex(ETH_DESTINATION_BINDING_VECTOR),
        },
        {
            "source_verifier_material_hash": bytes.fromhex(EVM_SOURCE_VERIFIER_MATERIAL_HASH),
            "source_adapter_engine_deployment_hash": bytes.fromhex(
                EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
            ),
            "destination_binding_hash": bytes.fromhex(EVM_SOURCE_VERIFIER_MATERIAL_HASH),
        },
        {
            "source_verifier_material_hash": bytes.fromhex(EVM_SOURCE_VERIFIER_MATERIAL_HASH),
            "source_adapter_engine_deployment_hash": bytes.fromhex(
                EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
            ),
            "destination_binding_hash": bytes.fromhex(
                EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
            ),
        },
    ):
        try:
            module.evm_route_allowlist_hash(domain=1, **replayed)
        except ValueError as exc:
            assert "EVM route allowlist evidence hashes must be distinct" in str(exc)
        else:
            raise AssertionError("EVM route allowlist accepted replayed governed hash role")


def test_evm_route_canary_transaction_hash_binds_target_domain():
    module = load_evidence_module()
    common = {
        "bridge_address": bytes.fromhex("22" * 20),
        "transaction_hash": bytes.fromhex("44" * 32),
        "log_index": 0,
        "receipt_block_number": 0x1234,
        "receipt_block_hash": bytes.fromhex("45" * 32),
        "block_receipts_root": bytes.fromhex("46" * 32),
        "call_data_sha256": bytes.fromhex("88" * 32),
        "message_id": bytes.fromhex("55" * 32),
        "payload_hash": bytes.fromhex("99" * 32),
        "source_domain": 0,
        "commitment_root": bytes.fromhex("77" * 32),
        "finality_height": bytes.fromhex("aa" * 32),
        "finality_block_hash": bytes.fromhex("ab" * 32),
        "statement_hash": bytes.fromhex("66" * 32),
        "proof_version": 1,
        "proof_source_domain": 0,
        "verifier_backend_hash": module.evm_verifier_backend_hash(),
        "proof_family_hash": module.evm_proof_family_hash(),
        "used_message_proof": True,
        "receipt_block_finalized": True,
    }
    eth_common = {
        **common,
        "route_allowlist_hash": bytes.fromhex(ETH_ROUTE_ALLOWLIST_HASH_VECTOR),
        "destination_binding_hash": bytes.fromhex(ETH_DESTINATION_BINDING_VECTOR),
        "network_id": bytes.fromhex(ETH_MAINNET_NETWORK_ID),
    }
    bsc_common = {
        **common,
        "route_allowlist_hash": bytes.fromhex(BSC_ROUTE_ALLOWLIST_HASH_VECTOR),
        "destination_binding_hash": bytes.fromhex(BSC_DESTINATION_BINDING_VECTOR),
        "network_id": bytes.fromhex(BSC_MAINNET_NETWORK_ID),
    }
    bsc_testnet_common = {
        **common,
        "route_allowlist_hash": bytes.fromhex(BSC_TESTNET_ROUTE_ALLOWLIST_HASH_VECTOR),
        "destination_binding_hash": bytes.fromhex(
            BSC_TESTNET_DESTINATION_BINDING_VECTOR
        ),
        "network_id": bytes.fromhex(BSC_TESTNET_NETWORK_ID),
    }

    eth_hash = module.evm_route_canary_transaction_evidence_hash(
        target_domain=module.SCCP_DOMAIN_ETH,
        **eth_common,
    )
    bsc_hash = module.evm_route_canary_transaction_evidence_hash(
        target_domain=module.SCCP_DOMAIN_BSC,
        **bsc_common,
    )
    bsc_testnet_hash = module.evm_route_canary_transaction_evidence_hash(
        target_domain=module.SCCP_DOMAIN_BSC,
        bsc_network="testnet",
        **bsc_testnet_common,
    )

    assert eth_hash != bsc_hash
    assert bsc_hash != bsc_testnet_hash
    assert eth_hash.hex() == ETH_ROUTE_CANARY_TRANSACTION_HASH_VECTOR
    assert bsc_hash.hex() == BSC_ROUTE_CANARY_TRANSACTION_HASH_VECTOR
    assert (
        bsc_testnet_hash.hex()
        == BSC_TESTNET_ROUTE_CANARY_TRANSACTION_HASH_VECTOR
    )
    try:
        module.evm_route_canary_transaction_evidence_hash(
            target_domain=module.SCCP_DOMAIN_SORA,
            **eth_common,
        )
    except ValueError as exc:
        assert "target_domain must be ETH or BSC" in str(exc)
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("non-EVM route canary target domain was accepted")

    try:
        module.evm_route_canary_transaction_evidence_hash(
            target_domain=module.SCCP_DOMAIN_ETH,
            **{**eth_common, "route_allowlist_hash": eth_common["destination_binding_hash"]},
        )
    except ValueError as exc:
        assert "EVM route canary governed hashes must be distinct" in str(exc)
    else:
        raise AssertionError("EVM route canary accepted route/destination hash role reuse")

    for field, source_field in (
        ("message_id", "transaction_hash"),
        ("payload_hash", "call_data_sha256"),
        ("commitment_root", "statement_hash"),
        ("finality_height", "transaction_hash"),
        ("finality_block_hash", "transaction_hash"),
    ):
        reused = dict(eth_common)
        reused[field] = reused[source_field]
        try:
            module.evm_route_canary_transaction_evidence_hash(
                target_domain=module.SCCP_DOMAIN_ETH,
                **reused,
            )
        except ValueError as exc:
            assert "EVM route canary transcript hashes must be distinct" in str(exc)
        else:
            raise AssertionError(f"reused EVM route canary hash role {field} accepted")

    non_finalized_hash = module.evm_route_canary_transaction_evidence_hash(
        target_domain=module.SCCP_DOMAIN_ETH,
        **{**eth_common, "receipt_block_finalized": False},
    )
    assert non_finalized_hash != eth_hash


def test_evm_destination_binding_hash_rejects_malformed_direct_material():
    module = load_evidence_module()
    common = {
        "network_id": bytes.fromhex(ETH_MAINNET_NETWORK_ID),
        "source_domain": 0,
        "target_domain": 1,
        "verifier_address": bytes.fromhex("11" * 20),
        "bridge_address": bytes.fromhex("22" * 20),
        "verifier_code_hash": bytes.fromhex("bb" * 32),
        "verifier_key_hash": bytes.fromhex("cc" * 32),
    }

    try:
        module.evm_destination_binding_hash(
            network_id=bytes(32),
            **{key: value for key, value in common.items() if key != "network_id"},
        )
    except ValueError as exc:
        assert "network_id must not be zero" in str(exc)
    else:
        raise AssertionError("zero EVM network id was accepted")

    try:
        module.evm_destination_binding_hash(
            network_id=bytes.fromhex("33" * 32),
            **{key: value for key, value in common.items() if key != "network_id"},
        )
    except ValueError as exc:
        assert "network_id must match ETH mainnet EIP-155 chain id 1" in str(exc)
    else:
        raise AssertionError("non-mainnet ETH network id was accepted")

    try:
        module.evm_destination_binding_key(
            network_id=bytes.fromhex("33" * 32),
            **{key: value for key, value in common.items() if key != "network_id"},
        )
    except ValueError as exc:
        assert "network_id must match ETH mainnet EIP-155 chain id 1" in str(exc)
    else:
        raise AssertionError("non-mainnet ETH network id binding key was accepted")

    try:
        module.evm_destination_binding_hash(
            source_domain=1,
            **{key: value for key, value in common.items() if key != "source_domain"},
        )
    except ValueError as exc:
        assert "source_domain must be SORA" in str(exc)
    else:
        raise AssertionError("non-SORA EVM destination source was accepted")

    try:
        module.evm_destination_binding_hash(
            source_domain=False,
            **{key: value for key, value in common.items() if key != "source_domain"},
        )
    except ValueError as exc:
        assert "source_domain must be an exact u32" in str(exc)
    else:
        raise AssertionError("boolean EVM destination source domain was accepted")

    try:
        module.evm_destination_binding_key(
            target_domain=True,
            **{key: value for key, value in common.items() if key != "target_domain"},
        )
    except ValueError as exc:
        assert "target_domain must be an exact u32" in str(exc)
    else:
        raise AssertionError("boolean EVM destination target domain was accepted")

    try:
        module.evm_route_allowlist_hash(
            domain=True,
            source_verifier_material_hash=bytes.fromhex(EVM_SOURCE_VERIFIER_MATERIAL_HASH),
            source_adapter_engine_deployment_hash=bytes.fromhex(
                EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
            ),
            destination_binding_hash=bytes.fromhex(ETH_DESTINATION_BINDING_VECTOR),
        )
    except ValueError as exc:
        assert "domain must be an exact u32" in str(exc)
    else:
        raise AssertionError("boolean EVM route allowlist domain was accepted")

    try:
        module.evm_destination_binding_hash(
            verifier_backend="debug-groth16",
            **common,
        )
    except ValueError as exc:
        assert "verifier_backend must be evm-groth16-bn254-v1" in str(exc)
    else:
        raise AssertionError("non-production EVM destination backend was accepted")

    try:
        module.evm_destination_binding_hash(
            proof_family="debug-proof-family",
            **common,
        )
    except ValueError as exc:
        assert "proof_family must be stark-fri-v1" in str(exc)
    else:
        raise AssertionError("non-production EVM destination proof family was accepted")

    aliased_addresses = dict(common)
    aliased_addresses["bridge_address"] = aliased_addresses["verifier_address"]
    for helper in (
        module.evm_destination_binding_hash,
        module.evm_destination_binding_key,
    ):
        try:
            helper(**aliased_addresses)
        except ValueError as exc:
            assert "verifier_address must differ from bridge_address" in str(exc)
        else:
            raise AssertionError(
                f"{helper.__name__} accepted aliased EVM verifier and bridge addresses"
            )


def test_evm_runtime_bytecode_derives_and_rejects_mismatched_code_hash():
    module = load_evidence_module()
    args = SimpleNamespace(
        verifier_runtime_bytecode_hex=bytes.fromhex("6080604052"),
        verifier_runtime_bytecode_file=None,
        verifier_code_hash=None,
    )

    module.apply_runtime_bytecode_hash(args)

    assert args.verifier_code_hash == module.runtime_bytecode_hash(
        bytes.fromhex("6080604052")
    )

    mismatch = SimpleNamespace(
        verifier_runtime_bytecode_hex=bytes.fromhex("6080604052"),
        verifier_runtime_bytecode_file=None,
        verifier_code_hash=bytes.fromhex("bb" * 32),
    )
    try:
        module.apply_runtime_bytecode_hash(mismatch)
    except ValueError as exc:
        assert "does not match verifier runtime bytecode" in str(exc)
    else:
        raise AssertionError("mismatched EVM verifier runtime bytecode was accepted")


def test_evm_direct_renderers_derive_code_hashes_from_runtime_bytecode():
    module = load_evidence_module()
    verifier_runtime = bytes.fromhex("6080604052")
    bridge_runtime = bytes.fromhex("6001600255")
    verifier_code_hash = module.runtime_bytecode_hash(verifier_runtime)
    bridge_code_hash = module.runtime_bytecode_hash(bridge_runtime)
    verifier_key_hash = bytes.fromhex("cc" * 32)
    network_id = module.evm_mainnet_network_id_for_domain(module.SCCP_DOMAIN_ETH)
    verifier_address = bytes.fromhex("11" * 20)
    bridge_address = bytes.fromhex("22" * 20)
    destination_binding_hash = module.evm_destination_binding_hash(
        network_id=network_id,
        source_domain=0,
        target_domain=1,
        verifier_address=verifier_address,
        bridge_address=bridge_address,
        verifier_code_hash=verifier_code_hash,
        verifier_key_hash=verifier_key_hash,
    )
    route_allowlist_hash = module.evm_route_allowlist_hash(
        domain=1,
        source_verifier_material_hash=bytes.fromhex(EVM_SOURCE_VERIFIER_MATERIAL_HASH),
        source_adapter_engine_deployment_hash=bytes.fromhex(
            EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
        ),
        destination_binding_hash=destination_binding_hash,
    )
    route_canary_transaction_hash = bytes.fromhex("44" * 32)
    route_canary_receipt_block_number = 0x1234
    route_canary_receipt_block_hash = bytes.fromhex("45" * 32)
    route_canary_block_receipts_root = bytes.fromhex("46" * 32)
    route_canary_message_id = bytes.fromhex("55" * 32)
    route_canary_call_data_sha256 = bytes.fromhex("88" * 32)
    route_canary_payload_hash = bytes.fromhex("99" * 32)
    route_canary_statement_hash = bytes.fromhex("66" * 32)
    route_canary_commitment_root = bytes.fromhex("77" * 32)
    route_canary_finality_height = bytes.fromhex("aa" * 32)
    route_canary_finality_block_hash = bytes.fromhex("ab" * 32)
    route_canary_evidence_hash = module.evm_route_canary_transaction_evidence_hash(
        route_allowlist_hash=route_allowlist_hash,
        bridge_address=bridge_address,
        transaction_hash=route_canary_transaction_hash,
        log_index=0,
        receipt_block_number=route_canary_receipt_block_number,
        receipt_block_hash=route_canary_receipt_block_hash,
        block_receipts_root=route_canary_block_receipts_root,
        call_data_sha256=route_canary_call_data_sha256,
        message_id=route_canary_message_id,
        payload_hash=route_canary_payload_hash,
        source_domain=0,
        target_domain=1,
        commitment_root=route_canary_commitment_root,
        finality_height=route_canary_finality_height,
        finality_block_hash=route_canary_finality_block_hash,
        statement_hash=route_canary_statement_hash,
        proof_version=1,
        proof_source_domain=0,
        destination_binding_hash=destination_binding_hash,
        verifier_backend_hash=module.evm_verifier_backend_hash(),
        proof_family_hash=module.evm_proof_family_hash(),
        network_id=network_id,
        used_message_proof=True,
        receipt_block_finalized=True,
    )
    common = dict(
        domain=1,
        network_id=network_id,
        verifier_address=verifier_address,
        bridge_address=bridge_address,
        verifier_key_hash=verifier_key_hash,
        expected_destination_binding_hash=destination_binding_hash,
        source_verifier_material_hash=bytes.fromhex(EVM_SOURCE_VERIFIER_MATERIAL_HASH),
        source_adapter_engine_deployment_hash=bytes.fromhex(
            EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
        ),
        route_allowlist_hash=route_allowlist_hash,
        route_canary_evidence_hash=route_canary_evidence_hash,
        route_canary_transaction_hash=route_canary_transaction_hash,
        route_canary_transaction_block_number=route_canary_receipt_block_number,
        route_canary_transaction_block_hash=route_canary_receipt_block_hash,
        route_canary_log_index=0,
        route_canary_receipt_block_number=route_canary_receipt_block_number,
        route_canary_receipt_block_hash=route_canary_receipt_block_hash,
        route_canary_block_receipts_root=route_canary_block_receipts_root,
        route_canary_call_data_sha256=route_canary_call_data_sha256,
        route_canary_message_id=route_canary_message_id,
        route_canary_payload_hash=route_canary_payload_hash,
        route_canary_target_domain=1,
        route_canary_statement_hash=route_canary_statement_hash,
        route_canary_commitment_root=route_canary_commitment_root,
        route_canary_finality_height=route_canary_finality_height,
        route_canary_finality_block_hash=route_canary_finality_block_hash,
        route_canary_proof_version=1,
        route_canary_proof_source_domain=0,
        route_canary_used_message_proof=True,
        route_canary_receipt_block_finalized=True,
    )

    render_args = SimpleNamespace(
        **common,
        verifier_runtime_bytecode_hex=verifier_runtime,
        bridge_runtime_bytecode_hex=bridge_runtime,
    )
    rendered = module.render_toml(render_args, destination_binding_hash)

    assert render_args.verifier_code_hash == verifier_code_hash
    assert render_args.bridge_code_hash == bridge_code_hash
    assert 'verifier_code_hash = "0x' + verifier_code_hash.hex() + '"' in rendered
    assert (
        '# sccp_evm_bridge_runtime_code_hash = "0x'
        + bridge_code_hash.hex()
        + '"'
        in rendered
    )
    assert (
        'route_allowlist_hash = "0x' + route_allowlist_hash.hex() + '"'
        in rendered
    )

    summary_args = SimpleNamespace(
        **common,
        verifier_runtime_bytecode_file=verifier_runtime,
        bridge_runtime_bytecode_file=bridge_runtime,
    )
    summary = module._json_summary(summary_args, destination_binding_hash, True)

    assert summary_args.verifier_code_hash == verifier_code_hash
    assert summary_args.bridge_code_hash == bridge_code_hash
    assert summary["verifier_code_hash"] == "0x" + verifier_code_hash.hex()
    assert summary["bridge_code_hash"] == "0x" + bridge_code_hash.hex()
    assert summary["destination_binding_hash"] == "0x" + destination_binding_hash.hex()
    assert summary["route_allowlist_hash"] == "0x" + route_allowlist_hash.hex()
    assert summary["toml_ready"] is True


def test_evm_toml_rendering_carries_eth_and_bsc_profile_ids():
    module = load_evidence_module()
    eth = evm_runtime_material(module, domain=1)

    def toml_args(material):
        return SimpleNamespace(
            domain=material.domain,
            bsc_network=material.bsc_network,
            network_id=material.network_id,
            verifier_address=material.verifier_address,
            bridge_address=material.bridge_address,
            bridge_code_hash=material.bridge_code_hash,
            bridge_runtime_bytecode_hex=material.bridge_runtime,
            verifier_code_hash=material.verifier_code_hash,
            verifier_runtime_bytecode_hex=material.verifier_runtime,
            verifier_key_hash=material.verifier_key_hash,
            expected_destination_binding_hash=material.destination_binding_hash,
            source_verifier_material_hash=bytes.fromhex(
                EVM_SOURCE_VERIFIER_MATERIAL_HASH
            ),
            source_adapter_engine_deployment_hash=bytes.fromhex(
                EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
            ),
            route_allowlist_hash=material.route_allowlist_hash,
            route_canary_evidence_hash=material.route_canary_evidence_hash,
            route_canary_transaction_hash=material.route_canary_transaction_hash,
            route_canary_transaction_block_number=(
                material.route_canary_transaction_block_number
            ),
            route_canary_transaction_block_hash=(
                material.route_canary_transaction_block_hash
            ),
            route_canary_log_index=material.route_canary_log_index,
            route_canary_receipt_block_number=material.route_canary_receipt_block_number,
            route_canary_receipt_block_hash=material.route_canary_receipt_block_hash,
            route_canary_block_receipts_root=material.route_canary_block_receipts_root,
            route_canary_call_data_sha256=material.route_canary_call_data_sha256,
            route_canary_message_id=material.route_canary_message_id,
            route_canary_payload_hash=material.route_canary_payload_hash,
            route_canary_target_domain=material.route_canary_target_domain,
            route_canary_statement_hash=material.route_canary_statement_hash,
            route_canary_commitment_root=material.route_canary_commitment_root,
            route_canary_finality_height=material.route_canary_finality_height,
            route_canary_finality_block_hash=material.route_canary_finality_block_hash,
            route_canary_proof_version=material.route_canary_proof_version,
            route_canary_proof_source_domain=material.route_canary_proof_source_domain,
            route_canary_used_message_proof=True,
            route_canary_receipt_block_finalized=True,
        )

    eth_args = toml_args(eth)
    eth_hash = eth.destination_binding_hash
    eth_binding_hex = eth.destination_binding_hash.hex()
    eth_route_hex = eth.route_allowlist_hash.hex()

    rendered = module.render_toml(eth_args, eth_hash)

    assert (
        '# sccp_evm_destination_binding_hash = "0x'
        + eth_binding_hex
        + '"'
        in rendered
    )
    assert (
        '# sccp_evm_route_allowlist_hash = "0x'
        + eth_route_hex
        + '"'
        in rendered
    )
    assert '# sccp_evm_rpc_chain_id = "1"' in rendered
    assert '# sccp_evm_block_tag = "finalized"' in rendered
    assert (
        '# sccp_evm_bridge_runtime_code_hash = "0x'
        + eth.bridge_code_hash.hex()
        + '"'
        in rendered
    )
    assert (
        '# sccp_evm_verifier_runtime_code_hash = "0x'
        + eth.verifier_code_hash.hex()
        + '"'
        in rendered
    )
    assert '# sccp_evm_verifier_key_hash = "0x' + "cc" * 32 + '"' in rendered
    assert (
        '# sccp_evm_verifier_backend_hash = "0x'
        + module.evm_verifier_backend_hash().hex()
        + '"'
        in rendered
    )
    assert (
        '# sccp_evm_proof_family_hash = "0x'
        + module.evm_proof_family_hash().hex()
        + '"'
        in rendered
    )
    assert '# sccp_evm_destination_network_id = "0x' + ETH_MAINNET_NETWORK_ID + '"' in rendered
    assert '# sccp_evm_destination_bridge_address = "0x' + "22" * 20 + '"' in rendered
    assert "# sccp_evm_destination_binding_key = " in rendered
    assert 'destination_network_id = "0x' + ETH_MAINNET_NETWORK_ID + '"' in rendered
    assert 'destination_bridge_address = "0x' + "22" * 20 + '"' in rendered
    assert 'destination_binding_key = "evm:0:1:' in rendered
    assert (
        'destination_binding_hash = "0x'
        + eth_binding_hex
        + '"'
        in rendered
    )
    assert "[[zk.sccp_destination_rollouts]]" in rendered
    assert "[[zk.sccp_route_allowlists]]" in rendered
    assert 'domain = 1' in rendered
    assert 'chain = "eth"' in rendered
    assert 'verifier_plan = "EvmGroth16Bn254Adapter"' in rendered
    assert 'verifier_identity = "0x' + "11" * 20 + '"' in rendered
    assert (
        'anchor_id = "sccp:eth:destination-anchor:ethereum-mainnet:v1"'
        in rendered
    )
    assert (
        'route_allowlist_id = "sccp:eth:route-allowlist:ethereum-mainnet:v1"'
        in rendered
    )
    assert (
        'route_allowlist_hash = "0x' + eth_route_hex + '"'
        in rendered
    )
    assert '# sccp_route_canary_status = "passed"' in rendered
    assert 'route_canary_status = "passed"' in rendered
    assert (
        '# sccp_route_canary_evidence_hash = "0x'
        + eth.route_canary_evidence_hash.hex()
        + '"'
        in rendered
    )
    assert (
        'route_canary_evidence_hash = "0x'
        + eth.route_canary_evidence_hash.hex()
        + '"'
        in rendered
    )
    assert (
        '# sccp_evm_route_canary_transaction_hash = "0x'
        + eth.route_canary_transaction_hash.hex()
        + '"'
        in rendered
    )
    assert (
        '# sccp_evm_route_canary_transaction_block_number = "4660"'
        in rendered
    )
    assert (
        '# sccp_evm_route_canary_transaction_block_hash = "0x'
        + eth.route_canary_transaction_block_hash.hex()
        + '"'
        in rendered
    )
    assert (
        'evm_route_canary_transaction_hash = "0x'
        + eth.route_canary_transaction_hash.hex()
        + '"'
        in rendered
    )
    assert (
        'evm_route_canary_transaction_block_hash = "0x'
        + eth.route_canary_transaction_block_hash.hex()
        + '"'
        in rendered
    )
    assert '# sccp_evm_route_canary_receipt_block_finalized = "true"' in rendered
    assert "evm_route_canary_receipt_block_finalized = true" in rendered
    assert 'blockers = []' in rendered

    bsc = evm_runtime_material(module, domain=2)
    bsc_args = toml_args(bsc)
    bsc_rendered = module.render_toml(bsc_args, bsc.destination_binding_hash)
    assert 'domain = 2' in bsc_rendered
    assert 'chain = "bsc"' in bsc_rendered
    assert '# sccp_evm_block_tag = "latest"' in bsc_rendered
    assert 'anchor_id = "sccp:bsc:destination-anchor:bsc-mainnet:v1"' in bsc_rendered
    assert (
        'route_allowlist_id = "sccp:bsc:route-allowlist:bsc-mainnet:v1"'
        in bsc_rendered
    )

    bsc_testnet = evm_runtime_material(module, domain=2, bsc_network="testnet")
    bsc_testnet_args = toml_args(bsc_testnet)
    bsc_testnet_rendered = module.render_toml(
        bsc_testnet_args,
        bsc_testnet.destination_binding_hash,
    )
    assert 'domain = 2' in bsc_testnet_rendered
    assert 'chain = "bsc-testnet"' in bsc_testnet_rendered
    assert '# sccp_evm_rpc_chain_id = "97"' in bsc_testnet_rendered
    assert '# sccp_evm_block_tag = "latest"' in bsc_testnet_rendered
    assert (
        '# sccp_evm_destination_network_id = "0x'
        + BSC_TESTNET_NETWORK_ID
        + '"'
        in bsc_testnet_rendered
    )
    assert (
        'destination_network_id = "0x' + BSC_TESTNET_NETWORK_ID + '"'
        in bsc_testnet_rendered
    )
    assert (
        'destination_binding_key = "evm:0:2:' + BSC_TESTNET_NETWORK_ID
        in bsc_testnet_rendered
    )
    assert (
        'destination_binding_hash = "0x'
        + bsc_testnet.destination_binding_hash.hex()
        + '"'
        in bsc_testnet_rendered
    )
    assert (
        'route_allowlist_hash = "0x'
        + bsc_testnet.route_allowlist_hash.hex()
        + '"'
        in bsc_testnet_rendered
    )
    assert (
        'anchor_id = "sccp:bsc:destination-anchor:bsc-testnet:v1"'
        in bsc_testnet_rendered
    )
    assert (
        'route_allowlist_id = "sccp:bsc:route-allowlist:bsc-testnet:v1"'
        in bsc_testnet_rendered
    )
    assert '# sccp_route_canary_status = "passed"' in bsc_testnet_rendered
    assert 'route_canary_status = "passed"' in bsc_testnet_rendered

    bsc_testnet_summary = module._json_summary(
        bsc_testnet_args,
        bsc_testnet.destination_binding_hash,
        True,
    )
    assert bsc_testnet_summary["chain"] == "bsc-testnet"
    assert bsc_testnet_summary["network_id"] == "0x" + BSC_TESTNET_NETWORK_ID
    assert bsc_testnet_summary["destination_binding_hash"] == (
        "0x" + bsc_testnet.destination_binding_hash.hex()
    )
    assert bsc_testnet_summary["route_allowlist_hash"] == (
        "0x" + bsc_testnet.route_allowlist_hash.hex()
    )
    assert bsc_testnet_summary["toml_ready"] is True

    try:
        module.render_toml(eth_args, bytes.fromhex("ee" * 32))
    except ValueError as exc:
        assert "canonical SORA -> ETH binding" in str(exc)
    else:
        raise AssertionError("mismatched direct EVM destination binding hash was accepted")

    try:
        module._json_summary(eth_args, bytes.fromhex("ee" * 32), False)
    except ValueError as exc:
        assert "canonical SORA -> ETH binding" in str(exc)
    else:
        raise AssertionError("mismatched direct EVM JSON binding hash was accepted")

    bad_allowlist_args = SimpleNamespace(
        **{**eth_args.__dict__, "route_allowlist_hash": bytes(32)}
    )
    try:
        module.render_toml(bad_allowlist_args, eth_hash)
    except ValueError as exc:
        assert "route_allowlist_hash must not be zero" in str(exc)
    else:
        raise AssertionError("zero direct EVM route allowlist hash was accepted")

    try:
        module._json_summary(bad_allowlist_args, eth_hash, False)
    except ValueError as exc:
        assert "route_allowlist_hash must not be zero" in str(exc)
    else:
        raise AssertionError("zero direct EVM JSON route allowlist hash was accepted")

    drifted_allowlist_args = SimpleNamespace(
        **{**eth_args.__dict__, "route_allowlist_hash": bytes.fromhex("dd" * 32)}
    )
    try:
        module.render_toml(drifted_allowlist_args, eth_hash)
    except ValueError as exc:
        assert "canonical source, deployment, and destination evidence" in str(exc)
    else:
        raise AssertionError("drifted direct EVM route allowlist hash was accepted")

    try:
        module._json_summary(drifted_allowlist_args, eth_hash, False)
    except ValueError as exc:
        assert "canonical source, deployment, and destination evidence" in str(exc)
    else:
        raise AssertionError("drifted direct EVM JSON route allowlist hash was accepted")

    missing_canary_args = SimpleNamespace(
        **{
            **eth_args.__dict__,
                "route_canary_evidence_hash": None,
                "route_canary_transaction_hash": None,
                "route_canary_transaction_block_number": None,
                "route_canary_transaction_block_hash": None,
                "route_canary_log_index": None,
                "route_canary_receipt_block_number": None,
                "route_canary_receipt_block_hash": None,
                "route_canary_block_receipts_root": None,
                "route_canary_call_data_sha256": None,
                "route_canary_message_id": None,
            "route_canary_payload_hash": None,
            "route_canary_target_domain": None,
            "route_canary_statement_hash": None,
            "route_canary_commitment_root": None,
            "route_canary_finality_height": None,
            "route_canary_finality_block_hash": None,
            "route_canary_proof_version": None,
            "route_canary_proof_source_domain": None,
            "route_canary_used_message_proof": None,
            "route_canary_receipt_block_finalized": None,
        }
    )
    try:
        module.render_toml(missing_canary_args, eth_hash)
    except ValueError as exc:
        assert "--route-canary-transaction-hash" in str(exc)
    else:
        raise AssertionError("EVM destination TOML accepted without route canary evidence")

    missing_canary_summary = module._json_summary(missing_canary_args, eth_hash, True)
    assert missing_canary_summary["toml_ready"] is False
    assert "route_canary" not in missing_canary_summary

    missing_bridge_runtime_args = SimpleNamespace(
        **{
            **toml_args(eth).__dict__,
            "bridge_runtime_bytecode_hex": None,
        }
    )
    try:
        module.render_toml(missing_bridge_runtime_args, eth_hash)
    except ValueError as exc:
        assert "--bridge-runtime-bytecode-hex" in str(exc)
    else:
        raise AssertionError("EVM destination TOML accepted without bridge bytecode")

    missing_bridge_runtime_summary = module._json_summary(
        missing_bridge_runtime_args,
        eth_hash,
        True,
    )
    assert missing_bridge_runtime_summary["toml_ready"] is False
    assert "bridge_runtime_bytecode_hex" not in missing_bridge_runtime_summary
    assert missing_bridge_runtime_summary["route_canary"]["status"] == "passed"

    forged_canary_args = SimpleNamespace(
        **{**eth_args.__dict__, "route_canary_evidence_hash": bytes.fromhex("e1" * 32)}
    )
    try:
        module.render_toml(forged_canary_args, eth_hash)
    except ValueError as exc:
        assert "route canary transaction metadata" in str(exc)
    else:
        raise AssertionError("EVM destination TOML accepted forged route canary hash")

    try:
        module._json_summary(forged_canary_args, eth_hash, True)
    except ValueError as exc:
        assert "route canary transaction metadata" in str(exc)
    else:
        raise AssertionError("EVM destination JSON accepted forged route canary hash")


def test_evm_destination_eth_toml_rejects_nonfinalized_block_tag():
    module = load_evidence_module()
    eth = evm_runtime_material(module, domain=1)
    args = full_toml_args(eth)
    args.block_tag = "latest"

    try:
        module.render_toml(args, eth.destination_binding_hash)
    except ValueError as exc:
        assert "Ethereum destination TOML requires --block-tag finalized" in str(exc)
    else:
        raise AssertionError("non-finalized ETH destination TOML was accepted")

    summary = module._json_summary(args, eth.destination_binding_hash, True)
    assert summary["block_tag"] == "latest"


def test_evm_full_toml_rejects_route_canary_transaction_readback_drift():
    module = load_evidence_module()
    eth = evm_runtime_material(module, domain=1)
    cases = [
        (
            "route_canary_transaction_block_number",
            eth.route_canary_receipt_block_number + 1,
            "transaction block number must match receipt block number",
        ),
        (
            "route_canary_transaction_block_hash",
            bytes.fromhex("fe" * 32),
            "transaction block hash must match receipt block hash",
        ),
        (
            "route_canary_receipt_block_finalized",
            False,
            "route-canary-receipt-block-finalized=true",
        ),
    ]
    for field, value, expected in cases:
        args = full_toml_args(eth)
        setattr(args, field, value)

        try:
            module.render_toml(args, eth.destination_binding_hash)
        except ValueError as exc:
            assert expected in str(exc)
        else:
            raise AssertionError(f"EVM destination TOML accepted drifted {field}")


def test_evm_full_toml_rejects_route_canary_transcript_hash_reuse():
    module = load_evidence_module()
    eth = evm_runtime_material(module, domain=1)

    for attr_name, source_attr_name in (
        ("route_canary_message_id", "route_canary_transaction_hash"),
        ("route_canary_payload_hash", "route_canary_call_data_sha256"),
        ("route_canary_commitment_root", "route_canary_statement_hash"),
        ("route_canary_finality_block_hash", "route_canary_transaction_hash"),
    ):
        args = full_toml_args(eth)
        setattr(args, attr_name, getattr(args, source_attr_name))

        try:
            module.render_toml(args, eth.destination_binding_hash)
        except ValueError as exc:
            assert "EVM route canary transcript hashes must be distinct" in str(exc)
        else:
            raise AssertionError(
                "EVM destination TOML accepted reused route canary transcript "
                f"hash {attr_name}"
            )


def test_evm_cli_json_summary_toml_and_expected_binding_check(capsys):
    module = load_evidence_module()
    eth = evm_runtime_material(module, domain=1)
    args = [
        "--domain",
        "eth",
        "--network-id",
        "0x" + eth.network_id.hex(),
        "--verifier-address",
        "0x" + eth.verifier_address.hex(),
        "--bridge-address",
        "0x" + eth.bridge_address.hex(),
        "--bridge-code-hash",
        "0x" + eth.bridge_code_hash.hex(),
        "--verifier-code-hash",
        "0x" + eth.verifier_code_hash.hex(),
        "--verifier-key-hash",
        "0x" + eth.verifier_key_hash.hex(),
        "--route-allowlist-hash",
        "0x" + eth.route_allowlist_hash.hex(),
        "--source-verifier-material-hash",
        "0x" + EVM_SOURCE_VERIFIER_MATERIAL_HASH,
        "--source-adapter-engine-deployment-hash",
        "0x" + EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH,
        "--expected-destination-binding-hash",
        "0x" + eth.destination_binding_hash.hex(),
    ]
    binding_only_args = args[:14]
    route_unpinned_args = args[:-2]
    default_network_args = [*args[:2], *args[4:]]
    full_args = [
        *args,
        "--bridge-runtime-bytecode-hex",
        "0x" + eth.bridge_runtime.hex(),
        "--verifier-runtime-bytecode-hex",
        "0x" + eth.verifier_runtime.hex(),
        "--route-canary-evidence-hash",
        "0x" + eth.route_canary_evidence_hash.hex(),
        "--route-canary-transaction-hash",
        "0x" + eth.route_canary_transaction_hash.hex(),
        "--route-canary-transaction-block-number",
        str(eth.route_canary_transaction_block_number),
        "--route-canary-transaction-block-hash",
        "0x" + eth.route_canary_transaction_block_hash.hex(),
        "--route-canary-log-index",
        str(eth.route_canary_log_index),
        "--route-canary-receipt-block-number",
        str(eth.route_canary_receipt_block_number),
        "--route-canary-receipt-block-hash",
        "0x" + eth.route_canary_receipt_block_hash.hex(),
        "--route-canary-block-receipts-root",
        "0x" + eth.route_canary_block_receipts_root.hex(),
        "--route-canary-call-data-sha256",
        "0x" + eth.route_canary_call_data_sha256.hex(),
        "--route-canary-message-id",
        "0x" + eth.route_canary_message_id.hex(),
        "--route-canary-payload-hash",
        "0x" + eth.route_canary_payload_hash.hex(),
        "--route-canary-target-domain",
        str(eth.route_canary_target_domain),
        "--route-canary-statement-hash",
        "0x" + eth.route_canary_statement_hash.hex(),
        "--route-canary-commitment-root",
        "0x" + eth.route_canary_commitment_root.hex(),
        "--route-canary-finality-height",
        "0x" + eth.route_canary_finality_height.hex(),
        "--route-canary-finality-block-hash",
        "0x" + eth.route_canary_finality_block_hash.hex(),
        "--route-canary-proof-version",
        str(eth.route_canary_proof_version),
        "--route-canary-proof-source-domain",
        str(eth.route_canary_proof_source_domain),
        "--route-canary-used-message-proof",
        "true",
        "--route-canary-receipt-block-finalized",
        "true",
    ]

    assert module.main(default_network_args) == 0
    defaulted = json.loads(capsys.readouterr().out)
    assert defaulted["network_id"] == "0x" + ETH_MAINNET_NETWORK_ID
    assert defaulted["destination_binding_hash"] == (
        "0x" + eth.destination_binding_hash.hex()
    )

    wrong_network_args = list(args)
    wrong_network_args[3] = "0x" + "33" * 32
    try:
        module.main(wrong_network_args)
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("mismatched ETH mainnet network id override was accepted")

    assert module.main(binding_only_args) == 0
    unpinned = json.loads(capsys.readouterr().out)
    assert unpinned["expected_destination_binding_hash_matches"] is False
    assert unpinned["toml_ready"] is False
    assert unpinned["destination_binding_hash"] == (
        "0x" + eth.destination_binding_hash.hex()
    )
    assert "route_allowlist_hash" not in unpinned

    try:
        module.main(route_unpinned_args)
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("unpinned EVM route allowlist hash was accepted")

    try:
        module.main([*binding_only_args, "--toml"])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("unpinned EVM destination TOML was accepted")

    assert module.main(args) == 0
    no_canary = json.loads(capsys.readouterr().out)
    assert no_canary["expected_destination_binding_hash_matches"] is True
    assert no_canary["expected_route_allowlist_hash_matches"] is True
    assert no_canary["toml_ready"] is False
    assert "route_canary" not in no_canary

    try:
        module.main([*args, "--toml"])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("EVM destination TOML rendered without route canary evidence")

    assert module.main(full_args) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["source_domain"] == 0
    assert output["target_domain"] == 1
    assert output["network_id"] == "0x" + ETH_MAINNET_NETWORK_ID
    assert output["destination_binding_key"].startswith("evm:0:1:")
    assert output["destination_binding_hash"] == (
        "0x" + eth.destination_binding_hash.hex()
    )
    assert output["expected_destination_binding_hash_matches"] is True
    assert output["bridge_code_hash"] == "0x" + eth.bridge_code_hash.hex()
    assert output["verifier_backend_hash"] == (
        "0x" + module.evm_verifier_backend_hash().hex()
    )
    assert output["proof_family_hash"] == "0x" + module.evm_proof_family_hash().hex()
    assert output["toml_ready"] is True
    assert output["expected_route_allowlist_hash"] == (
        "0x" + eth.route_allowlist_hash.hex()
    )
    assert output["expected_route_allowlist_hash_matches"] is True
    assert output["route_canary"]["status"] == "passed"
    assert output["route_canary"]["evidence_hash"] == (
        "0x" + eth.route_canary_evidence_hash.hex()
    )

    assert module.main([*full_args, "--toml"]) == 0
    rendered = capsys.readouterr().out
    assert "[[zk.sccp_destination_rollouts]]" in rendered
    assert "[[zk.sccp_route_allowlists]]" in rendered
    assert '# sccp_evm_rpc_chain_id = "1"' in rendered
    assert (
        '# sccp_evm_bridge_runtime_code_hash = "0x'
        + eth.bridge_code_hash.hex()
        + '"'
        in rendered
    )
    assert (
        '# sccp_evm_verifier_runtime_code_hash = "0x'
        + eth.verifier_code_hash.hex()
        + '"'
        in rendered
    )
    assert '# sccp_evm_verifier_key_hash = "0x' + "cc" * 32 + '"' in rendered
    assert "# sccp_evm_verifier_backend_hash" in rendered
    assert "# sccp_evm_proof_family_hash" in rendered

    try:
        module.main(
            [
                value
                if value != "0x" + eth.destination_binding_hash.hex()
                else "0x" + "ee" * 32
                for value in full_args
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("mismatched EVM destination binding hash was accepted")


def test_evm_cli_bsc_testnet_profile_defaults_and_scope(capsys):
    module = load_evidence_module()
    bsc_testnet = evm_runtime_material(module, domain=2, bsc_network="testnet")
    args = [
        "--domain",
        "bsc",
        "--bsc-network",
        "testnet",
        "--verifier-address",
        "0x" + bsc_testnet.verifier_address.hex(),
        "--bridge-address",
        "0x" + bsc_testnet.bridge_address.hex(),
        "--bridge-code-hash",
        "0x" + bsc_testnet.bridge_code_hash.hex(),
        "--verifier-code-hash",
        "0x" + bsc_testnet.verifier_code_hash.hex(),
        "--verifier-key-hash",
        "0x" + bsc_testnet.verifier_key_hash.hex(),
        "--expected-destination-binding-hash",
        "0x" + bsc_testnet.destination_binding_hash.hex(),
    ]

    assert module.main(args) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["chain"] == "bsc-testnet"
    assert output["target_domain"] == 2
    assert output["network_id"] == "0x" + BSC_TESTNET_NETWORK_ID
    assert output["destination_binding_key"].startswith(
        "evm:0:2:" + BSC_TESTNET_NETWORK_ID
    )
    assert output["destination_binding_hash"] == (
        "0x" + bsc_testnet.destination_binding_hash.hex()
    )

    wrong_network_args = [
        *args[:4],
        "--network-id",
        "0x" + BSC_MAINNET_NETWORK_ID,
        *args[4:],
    ]
    try:
        module.main(wrong_network_args)
    except SystemExit as exc:
        assert exc.code == 2
        assert "chain id 97" in capsys.readouterr().err
    else:
        raise AssertionError("BSC testnet CLI accepted the BSC mainnet network id")

    try:
        module.main(
            [
                "--domain",
                "eth",
                "--bsc-network",
                "testnet",
                "--verifier-address",
                "0x" + "11" * 20,
                "--bridge-address",
                "0x" + "22" * 20,
                "--verifier-code-hash",
                "0x" + "bb" * 32,
                "--verifier-key-hash",
                "0x" + "cc" * 32,
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
        assert "--bsc-network only applies when --domain bsc" in capsys.readouterr().err
    else:
        raise AssertionError("ETH destination CLI accepted a BSC testnet selector")


def test_evm_cli_requires_code_hash_or_runtime_bytecode():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--domain",
                "eth",
                "--network-id",
                "0x" + ETH_MAINNET_NETWORK_ID,
                "--verifier-address",
                "0x" + "11" * 20,
                "--bridge-address",
                "0x" + "22" * 20,
                "--verifier-key-hash",
                "0x" + "cc" * 32,
                "--route-allowlist-hash",
                "0x" + ETH_ROUTE_ALLOWLIST_HASH_VECTOR,
                "--source-verifier-material-hash",
                "0x" + EVM_SOURCE_VERIFIER_MATERIAL_HASH,
                "--source-adapter-engine-deployment-hash",
                "0x" + EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH,
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("EVM destination evidence without code hash was accepted")


def test_evm_toml_runtime_bytecode_reparse_redacts_parser_detail():
    module = load_evidence_module()
    cases = (
        (
            SimpleNamespace(
                bridge_runtime_bytecode_hex_text="0xsecret-token-bridge-runtime",
                bridge_code_hash=bytes.fromhex("11" * 32),
                verifier_runtime_bytecode_bytes=b"\x60\x01",
                verifier_code_hash=module.runtime_bytecode_hash(b"\x60\x01"),
            ),
            "--toml has invalid bridge runtime bytecode evidence",
        ),
        (
            SimpleNamespace(
                bridge_runtime_bytecode_bytes=b"\x60\x02",
                bridge_code_hash=module.runtime_bytecode_hash(b"\x60\x02"),
                verifier_runtime_bytecode_hex_text="0xsecret-token-verifier-runtime",
                verifier_code_hash=bytes.fromhex("22" * 32),
            ),
            "--toml has invalid verifier runtime bytecode evidence",
        ),
    )

    for args, expected_message in cases:
        try:
            module._require_runtime_bytecode_evidence(args, output="toml")
        except ValueError as exc:
            rendered = str(exc)
            assert rendered == expected_message
            assert "secret-token" not in rendered
            assert "must be hex" not in rendered
            assert exc.__cause__ is None
        else:
            raise AssertionError(
                "invalid copied EVM runtime bytecode evidence was accepted"
            )


def test_evm_toml_runtime_bytecode_reparse_redacts_helper_typeerror(
    monkeypatch,
):
    module = load_evidence_module()
    parser_calls: list[str] = []

    def reject_runtime_bytecode(_text, *, label):
        parser_calls.append(label)
        raise TypeError(f"secret-token {label} copied parser detail")

    monkeypatch.setattr(
        module,
        "parse_runtime_bytecode_hex",
        reject_runtime_bytecode,
    )

    cases = (
        (
            SimpleNamespace(
                bridge_runtime_bytecode_hex_text="0x6001",
                bridge_code_hash=bytes.fromhex("11" * 32),
                verifier_runtime_bytecode_bytes=b"\x60\x01",
                verifier_code_hash=module.runtime_bytecode_hash(b"\x60\x01"),
            ),
            "bridge runtime bytecode",
            "--toml has invalid bridge runtime bytecode evidence",
        ),
        (
            SimpleNamespace(
                bridge_runtime_bytecode_bytes=b"\x60\x02",
                bridge_code_hash=module.runtime_bytecode_hash(b"\x60\x02"),
                verifier_runtime_bytecode_hex_text="0x6002",
                verifier_code_hash=bytes.fromhex("22" * 32),
            ),
            "verifier runtime bytecode",
            "--toml has invalid verifier runtime bytecode evidence",
        ),
    )

    for args, label, expected_message in cases:
        try:
            module._require_runtime_bytecode_evidence(args, output="toml")
        except ValueError as exc:
            rendered = str(exc)
            assert rendered == expected_message
            assert "secret-token" not in rendered
            assert "copied parser detail" not in rendered
            assert exc.__cause__ is None
            assert exc.__suppress_context__ is True
        else:
            raise AssertionError(f"{label} helper TypeError was accepted")

    assert parser_calls == ["bridge runtime bytecode", "verifier runtime bytecode"]


def test_evm_cli_derives_bridge_code_hash_from_runtime_bytecode(capsys):
    module = load_evidence_module()
    bridge_runtime = "6001600255"
    bridge_code_hash = module.runtime_bytecode_hash(bytes.fromhex(bridge_runtime)).hex()
    args = [
        "--domain",
        "eth",
        "--network-id",
        "0x" + ETH_MAINNET_NETWORK_ID,
        "--verifier-address",
        "0x" + "11" * 20,
        "--bridge-address",
        "0x" + "22" * 20,
        "--bridge-runtime-bytecode-hex",
        "0x" + bridge_runtime,
        "--verifier-code-hash",
        "0x" + "bb" * 32,
        "--verifier-key-hash",
        "0x" + "cc" * 32,
        "--route-allowlist-hash",
        "0x" + ETH_ROUTE_ALLOWLIST_HASH_VECTOR,
        "--source-verifier-material-hash",
        "0x" + EVM_SOURCE_VERIFIER_MATERIAL_HASH,
        "--source-adapter-engine-deployment-hash",
        "0x" + EVM_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH,
        "--expected-destination-binding-hash",
        "0x" + ETH_DESTINATION_BINDING_VECTOR,
    ]

    assert module.main(args) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["bridge_code_hash"] == "0x" + bridge_code_hash
    assert output["toml_ready"] is False

    try:
        module.main([*args, "--bridge-code-hash", "0x" + "aa" * 32])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("mismatched bridge runtime bytecode hash was accepted")
