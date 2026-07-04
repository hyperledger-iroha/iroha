import json
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
from types import SimpleNamespace


BSC_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR = (
    "12536f25748a6520f10ebd42a7bcccd6ec181b9d53129795c8e186dc6e8b18cc"
)
BSC_TESTNET_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR = (
    "8b4240a5a0cdd4b237f9237a3ec12ca20a9386d71f506addbcb50587f8ee2e88"
)
BSC_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR = (
    "1630e4d75e2676cc443e07b0477303240ae4cff13bdf9fe61725b4a9a4ee959a"
)
BSC_TESTNET_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR = (
    "92f820f9d79f36916f94b3b35bf07ca199b1b9b716cc35293d08a3a88d1a5581"
)
BSC_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR = (
    "7d47ade779a5bddb3a5f283600af677db8605b75a00516a4328f3823ff28fb2d"
)
BSC_TESTNET_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR = (
    "5327093f2f34daa6efa791b43a87593eccf7ef8395b6ee41ed2fb6c254c3299a"
)


def load_evidence_module():
    script_path = (
        Path(__file__).resolve().parents[2]
        / "scripts"
        / "sccp_bsc_source_bridge_evidence.py"
    )
    spec = spec_from_file_location("sccp_bsc_source_bridge_evidence", script_path)
    module = module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


class HostileExpectedRecordHash:
    def __str__(self):
        raise AssertionError("secret-token BSC expected record hash was stringified")

    def __repr__(self):
        raise AssertionError("secret-token BSC expected record hash was repr'd")

    def __eq__(self, _other):
        raise AssertionError("secret-token BSC expected record hash was compared")

    def __ne__(self, _other):
        raise AssertionError("secret-token BSC expected record hash was compared")


def bsc_args(module, *, bsc_network="mainnet"):
    adapter_verifier_vk_hash = (
        BSC_TESTNET_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
        if bsc_network == "testnet"
        else BSC_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
    )
    expected_material_hash = (
        BSC_TESTNET_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
        if bsc_network == "testnet"
        else BSC_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
    )
    expected_deployment_hash = (
        BSC_TESTNET_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
        if bsc_network == "testnet"
        else BSC_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
    )
    return SimpleNamespace(
        bsc_network=bsc_network,
        source_domain=2,
        target_domain=0,
        bridge_address=bytes.fromhex("11" * 20),
        source_trust_anchor_hash=bytes.fromhex("44" * 32),
        consensus_verifier_hash=bytes.fromhex("55" * 32),
        message_inclusion_verifier_hash=bytes.fromhex("66" * 32),
        source_bridge_emitter_code_hash=bytes.fromhex("77" * 32),
        finality_policy_hash=bytes.fromhex("88" * 32),
        adapter_verifier_vk_hash=bytes.fromhex(adapter_verifier_vk_hash),
        deployment_receipt_hash=bytes.fromhex("aa" * 32),
        deployment_transaction_hash=bytes.fromhex("bd" * 32),
        deployment_transaction_block_hash=bytes.fromhex("bb" * 32),
        deployment_transaction_block_number=4660,
        deployment_transaction_input_sha256=bytes.fromhex("cd" * 32),
        deployment_receipt_contract_address=bytes.fromhex("11" * 20),
        deployment_receipt_block_hash=bytes.fromhex("bb" * 32),
        deployment_receipt_block_number=4660,
        deployment_receipt_block_receipts_root=bytes.fromhex("bc" * 32),
        expected_source_verifier_material_hash=bytes.fromhex(expected_material_hash),
        expected_source_adapter_engine_deployment_hash=bytes.fromhex(
            expected_deployment_hash
        ),
    )


def active_template_hash(module, lane, field):
    for template_lane, template_field, template_hash in (
        module.sccp_active_source_template_component_hashes()
    ):
        if template_lane == lane and template_field == field:
            return template_hash
    raise AssertionError(f"missing active {lane} template hash for {field}")


def test_bsc_cli_redacts_top_level_exception_details(monkeypatch, capsys):
    module = load_evidence_module()

    for exception_type in (
        module.argparse.ArgumentTypeError,
        OSError,
        SystemExit,
        RuntimeError,
        TypeError,
        ValueError,
    ):

        def fail_apply(_args, exception_type=exception_type):
            raise exception_type("secret-token /tmp/operator/private-path")

        with monkeypatch.context() as patch:
            patch.setattr(module, "apply_runtime_bytecode_hash", fail_apply)
            try:
                module.main(
                    [
                        "--bridge-address",
                        "0x" + "11" * 20,
                        "--source-trust-anchor-hash",
                        "0x" + "44" * 32,
                        "--consensus-verifier-hash",
                        "0x" + "55" * 32,
                        "--message-inclusion-verifier-hash",
                        "0x" + "66" * 32,
                        "--finality-policy-hash",
                        "0x" + "88" * 32,
                        "--adapter-verifier-vk-hash",
                        "0x" + BSC_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
                        "--deployment-receipt-hash",
                        "0x" + "aa" * 32,
                    ]
                )
            except SystemExit as exc:
                assert exc.code == 2
            else:
                raise AssertionError("BSC source CLI accepted top-level render failure")

            captured = capsys.readouterr()
            assert "SCCP BSC source bridge evidence rendering failed" in captured.err
            assert "secret-token" not in captured.err
            assert "private-path" not in captured.err
            assert exception_type.__name__ not in captured.err


def test_bsc_address_parser_rejects_zero_and_wrong_width(tmp_path):
    module = load_evidence_module()

    assert module.parse_evm_address(
        "0x" + "11" * 20,
        label="bridge address",
    ) == bytes.fromhex("11" * 20)
    assert module.parse_runtime_bytecode_hex(
        "0x6080604052",
        label="source bridge runtime bytecode",
    ) == bytes.fromhex("6080604052")

    for value, expected in (
        ("11" * 20, "canonical lowercase 0x hex"),
        ("0X" + "11" * 20, "lowercase 0x prefix"),
        ("0x" + "AA" * 20, "lowercase hex"),
    ):
        try:
            module.parse_evm_address(value, label="bridge address")
        except module.argparse.ArgumentTypeError as exc:
            assert expected in str(exc)
        else:
            raise AssertionError("non-canonical BSC bridge address was accepted")

    for value in (" 0x6080604052", "0x6080\n604052"):
        try:
            module.parse_runtime_bytecode_hex(
                value,
                label="source bridge runtime bytecode",
            )
        except module.argparse.ArgumentTypeError as exc:
            assert "must not contain whitespace" in str(exc)
        else:
            raise AssertionError("padded BSC runtime bytecode was accepted")

    for value, expected in (
        ("6080604052", "canonical lowercase 0x hex"),
        ("0X6080604052", "lowercase 0x prefix"),
        ("0x60806040AB", "lowercase hex"),
    ):
        try:
            module.parse_runtime_bytecode_hex(
                value,
                label="source bridge runtime bytecode",
            )
        except module.argparse.ArgumentTypeError as exc:
            assert expected in str(exc)
        else:
            raise AssertionError("non-canonical BSC runtime bytecode was accepted")

    runtime_file = tmp_path / "runtime.hex"
    runtime_file.write_text("0x6080\n604052\n", encoding="ascii")
    assert module.parse_runtime_bytecode_file(
        str(runtime_file),
        label="source bridge runtime bytecode",
    ) == bytes.fromhex("6080604052")

    try:
        module.parse_evm_address("0x" + "00" * 20, label="bridge address")
    except module.argparse.ArgumentTypeError as exc:
        assert "must not be zero" in str(exc)
    else:
        raise AssertionError("zero BSC bridge address was accepted")

    try:
        module.parse_evm_address(
            " 0x" + "11" * 20 + " ",
            label="bridge address",
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must not contain whitespace" in str(exc)
    else:
        raise AssertionError("padded BSC bridge address was accepted")

    try:
        module.parse_evm_address("0x" + "11" * 19, label="bridge address")
    except module.argparse.ArgumentTypeError as exc:
        assert "must be 20 bytes" in str(exc)
    else:
        raise AssertionError("short BSC bridge address was accepted")


def test_bsc_hash_parser_rejects_zero_and_wrong_width():
    module = load_evidence_module()

    assert module.parse_hex_bytes(
        "0x" + "44" * 32,
        label="source trust anchor hash",
        byte_length=32,
    ) == bytes.fromhex("44" * 32)

    try:
        module.parse_hex_bytes(
            "44" * 32,
            label="source trust anchor hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "canonical lowercase 0x hex" in str(exc)
    else:
        raise AssertionError("bare BSC component hash was accepted")

    try:
        module.parse_hex_bytes(
            "0x" + "00" * 32,
            label="source trust anchor hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must not be zero" in str(exc)
    else:
        raise AssertionError("zero BSC component hash was accepted")

    try:
        module.parse_hex_bytes(
            " 0x" + "44" * 32 + " ",
            label="source trust anchor hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must not contain whitespace" in str(exc)
    else:
        raise AssertionError("padded BSC component hash was accepted")

    try:
        module.parse_hex_bytes(
            "0x" + "44" * 31,
            label="source trust anchor hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must be 32 bytes" in str(exc)
    else:
        raise AssertionError("short BSC component hash was accepted")


def test_bsc_source_bridge_direct_parsers_redact_parser_causes(tmp_path):
    module = load_evidence_module()

    fixed_payload = "secret-token-bsc-source-hex"
    try:
        module.parse_hex_bytes(
            "0x" + fixed_payload + ("a" * (64 - len(fixed_payload))),
            label="source trust anchor hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        rendered = str(exc)
        assert rendered == "source trust anchor hash must be hex"
        assert "secret-token" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("secret BSC source bridge fixed hex was accepted")

    runtime_payload = "secret-token-bsc-source-runtime0"
    try:
        module.parse_runtime_bytecode_hex(
            "0x" + runtime_payload,
            label="source bridge runtime bytecode",
        )
    except module.argparse.ArgumentTypeError as exc:
        rendered = str(exc)
        assert rendered == "source bridge runtime bytecode must be hex"
        assert "secret-token" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("secret BSC source bridge runtime hex was accepted")

    secret_path = tmp_path / "secret-token-bsc-source-file-path.hex"
    try:
        module.parse_runtime_bytecode_file(
            str(secret_path),
            label="source bridge runtime bytecode",
        )
    except module.argparse.ArgumentTypeError as exc:
        rendered = str(exc)
        assert rendered == "source bridge runtime bytecode file cannot be read"
        assert "secret-token" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("missing secret BSC source bridge runtime file was accepted")


def test_bsc_runtime_bytecode_file_rejects_unreadable_file_shapes(tmp_path):
    module = load_evidence_module()
    outside = tmp_path / "secret-token-bsc-runtime-outside.hex"
    outside.write_text("0x6080604052\n", encoding="utf-8")
    symlink_input = tmp_path / "secret-token-bsc-runtime-link.hex"
    symlink_input.symlink_to(outside)
    directory_input = tmp_path / "secret-token-bsc-runtime-dir.hex"
    directory_input.mkdir()
    missing_input = tmp_path / "secret-token-bsc-runtime-missing.hex"

    for path in (symlink_input, directory_input, missing_input):
        try:
            module.parse_runtime_bytecode_file(
                str(path),
                label="source bridge runtime bytecode",
            )
        except module.argparse.ArgumentTypeError as exc:
            rendered = str(exc)
            suppress_context = exc.__suppress_context__
        else:
            raise AssertionError("BSC runtime bytecode file shape was accepted")

        assert rendered == "source bridge runtime bytecode file cannot be read"
        assert "secret-token" not in rendered
        assert "IsADirectoryError" not in rendered
        assert "FileNotFoundError" not in rendered
        assert suppress_context is True


def test_bsc_source_bridge_direct_parsers_redact_helper_exit_parser_causes(monkeypatch):
    module = load_evidence_module()

    for exception_type in (
        module.argparse.ArgumentTypeError,
        SystemExit,
        RuntimeError,
        TypeError,
        ValueError,
    ):
        detail = (
            "secret-token BSC source hex TypeError detail"
            if exception_type is TypeError
            else f"secret-token BSC source hex {exception_type.__name__} detail"
        )

        class SecretBytes:
            @staticmethod
            def fromhex(_text, detail=detail, exception_type=exception_type):
                raise exception_type(detail)

        with monkeypatch.context() as patch:
            patch.setattr(module, "bytes", SecretBytes, raising=False)

            for parser, value, label, kwargs in (
                (
                    module.parse_hex_bytes,
                    "0x" + "11" * 32,
                    "source trust anchor hash",
                    {"byte_length": 32},
                ),
                (
                    module.parse_runtime_bytecode_hex,
                    "0x6001600055",
                    "source bridge runtime bytecode",
                    {},
                ),
            ):
                try:
                    parser(value, label=label, **kwargs)
                except module.argparse.ArgumentTypeError as exc:
                    rendered = str(exc)
                    assert rendered == f"{label} must be hex"
                    assert "secret-token" not in rendered
                    assert exception_type.__name__ not in rendered
                    if exception_type is module.argparse.ArgumentTypeError:
                        assert (
                            "ArgumentTypeError" not in rendered
                        ), "BSC source hex ArgumentTypeError detail leaked"
                    assert exc.__cause__ is None
                    assert exc.__suppress_context__ is True
                else:
                    raise AssertionError(
                        f"{label} parser {exception_type.__name__} was accepted"
                    )


def test_bsc_source_bridge_fixed_hex_nonzero_controls_reject_non_booleans():
    module = load_evidence_module()

    for nonzero in (1, "true", None):
        try:
            module.parse_hex_bytes(
                "0x" + "00" * 32,
                label="source trust anchor hash",
                byte_length=32,
                nonzero=nonzero,
            )
        except ValueError as exc:
            assert str(exc) == (
                "BSC source bridge fixed hex nonzero must be a boolean"
            )
        else:
            raise AssertionError(
                "malformed BSC source bridge fixed-hex nonzero control accepted"
            )


def test_bsc_source_numeric_parsers_require_canonical_ascii_decimal():
    module = load_evidence_module()

    assert module.parse_u32("2", label="source domain") == 2
    assert module.parse_positive_u64("4660", label="deployment block number") == 4660
    assert module.parse_bsc_network("mainnet") == "mainnet"
    assert module.parse_bsc_network("bsc-mainnet") == "mainnet"
    assert module.parse_bsc_network("56") == "mainnet"
    assert module.parse_bsc_network("testnet") == "testnet"
    assert module.parse_bsc_network("bsc-testnet") == "testnet"
    assert module.parse_bsc_network("chapel") == "testnet"
    assert module.parse_bsc_network("97") == "testnet"

    try:
        module.parse_bsc_network("secret-token-bsc-source-network")
    except module.argparse.ArgumentTypeError as exc:
        rendered = str(exc)
        assert rendered == "BSC network must be mainnet or testnet"
        assert "secret-token" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("secret BSC source network was accepted")

    for value in ("02", "0x2", "+2", " 2 ", "٢"):
        try:
            module.parse_u32(value, label="source domain")
        except module.argparse.ArgumentTypeError as exc:
            assert "must be a u32" in str(exc)
        else:
            raise AssertionError(f"noncanonical BSC source domain {value!r} was accepted")

    for value in ("0", "04660", "0x1234", "+4660", " 4660 ", "٤٦٦٠"):
        try:
            module.parse_positive_u64(value, label="deployment block number")
        except module.argparse.ArgumentTypeError as exc:
            assert "must be a positive u64" in str(exc)
        else:
            raise AssertionError(f"noncanonical BSC block number {value!r} was accepted")

    for value in (" testnet ", "nile", "bnb-testnet"):
        try:
            module.parse_bsc_network(value)
        except module.argparse.ArgumentTypeError as exc:
            assert "BSC network must be mainnet or testnet" in str(exc)
        else:
            raise AssertionError(f"noncanonical BSC network {value!r} was accepted")


def test_bsc_runtime_bytecode_derives_source_bridge_code_hash():
    module = load_evidence_module()
    args = SimpleNamespace(
        source_bridge_runtime_bytecode_hex=bytes.fromhex("6080604052"),
        source_bridge_runtime_bytecode_file=None,
        source_bridge_emitter_code_hash=None,
    )

    module.apply_runtime_bytecode_hash(args)

    assert len(args.source_bridge_emitter_code_hash) == 32
    assert args.source_bridge_emitter_code_hash == module.runtime_bytecode_hash(
        bytes.fromhex("6080604052")
    )


def test_bsc_runtime_bytecode_rejects_mismatched_source_bridge_code_hash():
    module = load_evidence_module()
    args = SimpleNamespace(
        source_bridge_runtime_bytecode_hex=bytes.fromhex("6080604052"),
        source_bridge_runtime_bytecode_file=None,
        source_bridge_emitter_code_hash=bytes.fromhex("77" * 32),
    )

    try:
        module.apply_runtime_bytecode_hash(args)
    except ValueError as exc:
        assert "does not match source bridge runtime bytecode" in str(exc)
    else:
        raise AssertionError("mismatched BSC runtime bytecode hash was accepted")


def test_bsc_direct_renderers_derive_source_bridge_code_hash_from_runtime_bytecode():
    module = load_evidence_module()
    runtime_bytecode = bytes.fromhex("6080604052")
    source_bridge_code_hash = module.runtime_bytecode_hash(runtime_bytecode)

    expected_args = bsc_args(module)
    expected_args.source_bridge_emitter_code_hash = source_bridge_code_hash
    expected_material_hash = module.bsc_source_verifier_material_record_hash(
        expected_args
    )
    expected_deployment_hash = module.bsc_source_adapter_engine_deployment_record_hash(
        expected_args
    )

    render_args = bsc_args(module)
    render_args.source_bridge_emitter_code_hash = None
    render_args.source_bridge_runtime_bytecode_hex = runtime_bytecode
    render_args.expected_source_verifier_material_hash = expected_material_hash
    render_args.expected_source_adapter_engine_deployment_hash = (
        expected_deployment_hash
    )
    rendered = module.render_toml(render_args)

    assert render_args.source_bridge_emitter_code_hash == source_bridge_code_hash
    assert (
        'source_bridge_emitter_code_hash = "0x'
        + source_bridge_code_hash.hex()
        + '"'
        in rendered
    )
    assert (
        '# sccp_bsc_source_verifier_material_hash = "0x'
        + expected_material_hash.hex()
        + '"'
        in rendered
    )
    assert '# sccp_evm_source_block_tag = "latest"' in rendered
    assert (
        '# sccp_evm_source_bridge_runtime_bytecode_hex = "0x'
        + runtime_bytecode.hex()
        + '"'
        in rendered
    )

    summary_args = bsc_args(module)
    summary_args.source_bridge_emitter_code_hash = None
    summary_args.source_bridge_runtime_bytecode_file = runtime_bytecode
    summary_args.expected_source_verifier_material_hash = expected_material_hash
    summary_args.expected_source_adapter_engine_deployment_hash = (
        expected_deployment_hash
    )
    summary = module._json_summary(summary_args)

    assert summary_args.source_bridge_emitter_code_hash == source_bridge_code_hash
    assert (
        summary["source_bridge_emitter_code_hash"]
        == "0x" + source_bridge_code_hash.hex()
    )
    assert summary["source_verifier_material_hash"] == (
        "0x" + expected_material_hash.hex()
    )
    assert summary["source_adapter_engine_deployment_hash"] == (
        "0x" + expected_deployment_hash.hex()
    )
    assert summary["source_bridge_runtime_bytecode_hex"] == (
        "0x" + runtime_bytecode.hex()
    )
    assert summary["toml_ready"] is True


def test_bsc_toml_rendering_carries_mainnet_profile_ids_and_emitter_binding():
    module = load_evidence_module()
    assert (
        module.bsc_source_adapter_verifier_vk_hash().hex()
        == BSC_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
    )
    args = bsc_args(module)
    runtime_bytecode = bytes.fromhex("6080604052")
    source_bridge_code_hash = module.runtime_bytecode_hash(runtime_bytecode)
    args.source_bridge_emitter_code_hash = source_bridge_code_hash
    args.source_bridge_runtime_bytecode_hex = runtime_bytecode
    args.source_bridge_runtime_bytecode_file = None
    args.expected_source_verifier_material_hash = (
        module.bsc_source_verifier_material_record_hash(args)
    )
    args.expected_source_adapter_engine_deployment_hash = (
        module.bsc_source_adapter_engine_deployment_record_hash(args)
    )

    rendered = module.render_toml(args)

    assert '# sccp_evm_source_rpc_chain_id = "56"' in rendered
    assert '# sccp_evm_source_bridge_address = "0x' + "11" * 20 + '"' in rendered
    assert (
        '# sccp_evm_source_bridge_runtime_code_hash = "0x'
        + source_bridge_code_hash.hex()
        + '"'
        in rendered
    )
    assert (
        '# sccp_evm_source_bridge_runtime_bytecode_hex = "0x'
        + runtime_bytecode.hex()
        + '"'
        in rendered
    )
    assert (
        '# sccp_evm_source_deployment_transaction_hash = "0x'
        + "bd" * 32
        + '"'
        in rendered
    )
    assert (
        '# sccp_evm_source_deployment_transaction_block_hash = "0x'
        + "bb" * 32
        + '"'
        in rendered
    )
    assert (
        '# sccp_evm_source_deployment_transaction_block_number = "4660"'
        in rendered
    )
    assert (
        '# sccp_evm_source_deployment_transaction_input_sha256 = "0x'
        + "cd" * 32
        + '"'
        in rendered
    )
    assert '# sccp_evm_source_deployment_receipt_status = "0x1"' in rendered
    assert (
        '# sccp_evm_source_deployment_contract_address = "0x'
        + "11" * 20
        + '"'
        in rendered
    )
    assert (
        '# sccp_evm_source_deployment_block_hash = "0x' + "bb" * 32 + '"'
        in rendered
    )
    assert '# sccp_evm_source_deployment_block_number = "4660"' in rendered
    assert (
        '# sccp_evm_source_deployment_block_receipts_root = "0x'
        + "bc" * 32
        + '"'
        in rendered
    )
    assert (
        '# sccp_bsc_source_verifier_material_hash = "0x'
        + args.expected_source_verifier_material_hash.hex()
        + '"'
        in rendered
    )
    assert "[[zk.sccp_source_verifier_materials]]" in rendered
    assert (
        '# sccp_bsc_source_adapter_engine_deployment_hash = "0x'
        + args.expected_source_adapter_engine_deployment_hash.hex()
        + '"'
        in rendered
    )
    assert "[[zk.sccp_source_adapter_engine_deployments]]" in rendered
    assert 'source_domain = 2' in rendered
    assert 'target_domain = 0' in rendered
    assert 'source_chain = "bsc"' in rendered
    assert 'source_proof_plan = "BscValidatorSetReceiptProof"' in rendered
    assert 'finality_model = "BscValidatorSet"' in rendered
    assert (
        'source_trust_anchor_id = "sccp:bsc:source-trust-anchor:bsc-mainnet-validator-set:v1"'
        in rendered
    )
    assert (
        'source_bridge_emitter_id = "sccp:bsc:source-bridge-emitter:bsc-mainnet:v1"'
        in rendered
    )
    assert 'source_bridge_emitter_address = "0x' + "11" * 20 + '"' in rendered
    assert (
        'source_bridge_emitter_code_hash = "0x'
        + source_bridge_code_hash.hex()
        + '"'
        in rendered
    )
    assert (
        'adapter_verifier_vk_hash = "0x'
        + BSC_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
        + '"'
        in rendered
    )
    assert 'deployment_receipt_hash = "0x' + "aa" * 32 + '"' in rendered
    assert (
        'evm_source_gate_hash = "0x' + module.bsc_source_gate_hash(args).hex() + '"'
        in rendered
    )
    assert "source_bridge_network_id" not in rendered
    assert "source_bridge_owner_address" not in rendered
    assert "source_bridge_config_hash" not in rendered

    testnet_args = bsc_args(module, bsc_network="testnet")
    testnet_runtime_bytecode = bytes.fromhex("6080604052")
    testnet_source_bridge_code_hash = module.runtime_bytecode_hash(
        testnet_runtime_bytecode
    )
    testnet_args.source_bridge_emitter_code_hash = testnet_source_bridge_code_hash
    testnet_args.source_bridge_runtime_bytecode_hex = testnet_runtime_bytecode
    testnet_args.source_bridge_runtime_bytecode_file = None
    testnet_args.expected_source_verifier_material_hash = (
        module.bsc_source_verifier_material_record_hash(testnet_args)
    )
    testnet_args.expected_source_adapter_engine_deployment_hash = (
        module.bsc_source_adapter_engine_deployment_record_hash(testnet_args)
    )

    testnet_rendered = module.render_toml(testnet_args)
    assert '# sccp_evm_source_rpc_chain_id = "97"' in testnet_rendered
    assert "evm_source_gate_hash" not in testnet_rendered
    assert 'source_chain = "bsc-testnet"' in testnet_rendered
    assert (
        'source_trust_anchor_id = "sccp:bsc:source-trust-anchor:'
        'bsc-testnet-validator-set:v1"'
        in testnet_rendered
    )
    assert (
        'consensus_verifier_id = "sccp:bsc:consensus-verifier:'
        'validator-set-seal-testnet:v1"'
        in testnet_rendered
    )
    assert (
        'message_inclusion_verifier_id = "sccp:bsc:message-inclusion-verifier:'
        'receipt-trie-branch-testnet:v1"'
        in testnet_rendered
    )
    assert (
        'source_bridge_emitter_id = "sccp:bsc:source-bridge-emitter:'
        'bsc-testnet:v1"'
        in testnet_rendered
    )
    assert (
        'finality_policy_id = "sccp:bsc:finality-policy:'
        'validator-set-finality-testnet:v1"'
        in testnet_rendered
    )
    assert (
        'adapter_verifier_vk_hash = "0x'
        + BSC_TESTNET_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
        + '"'
        in testnet_rendered
    )

    testnet_summary = module._json_summary(testnet_args)
    assert testnet_summary["source_chain"] == "bsc-testnet"
    assert testnet_summary["rpc_chain_id"] == 97
    assert testnet_summary["source_bridge_emitter_id"] == (
        "sccp:bsc:source-bridge-emitter:bsc-testnet:v1"
    )
    assert testnet_summary["adapter_verifier_vk_hash"] == (
        "0x" + BSC_TESTNET_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
    )
    assert testnet_summary["source_verifier_material_hash"] == (
        "0x" + testnet_args.expected_source_verifier_material_hash.hex()
    )
    assert testnet_summary["source_adapter_engine_deployment_hash"] == (
        "0x" + testnet_args.expected_source_adapter_engine_deployment_hash.hex()
    )
    assert testnet_summary["toml_ready"] is True


def test_bsc_source_evidence_rejects_boolean_receipt_block_number():
    module = load_evidence_module()
    args = bsc_args(module)
    args.deployment_receipt_block_number = True

    try:
        module.render_toml(args)
    except ValueError as exc:
        assert "--deployment-receipt-block-number must be positive" in str(exc)
    else:
        raise AssertionError("boolean BSC source deployment block number was accepted")

    assert module._toml_receipt_metadata_ready(args) is False


def test_bsc_source_evidence_rejects_deployment_transaction_readback_drift():
    module = load_evidence_module()
    cases = [
        (
            "deployment_transaction_block_hash",
            bytes.fromhex("ab" * 32),
            "--deployment-transaction-block-hash must match",
        ),
        (
            "deployment_transaction_block_number",
            4661,
            "--deployment-transaction-block-number must match",
        ),
        (
            "deployment_transaction_input_sha256",
            None,
            "--deployment-transaction-input-sha256",
        ),
    ]
    for field, value, expected in cases:
        args = bsc_args(module)
        setattr(args, field, value)

        try:
            module.render_toml(args)
        except ValueError as exc:
            assert expected in str(exc)
        else:
            raise AssertionError(f"BSC source TOML accepted drifted {field}")

        assert module._toml_receipt_metadata_ready(args) is False


def test_bsc_source_evidence_requires_receipt_block_receipts_root_for_toml():
    module = load_evidence_module()
    args = bsc_args(module)
    args.deployment_receipt_block_receipts_root = None

    try:
        module.render_toml(args)
    except ValueError as exc:
        assert "--deployment-receipt-block-receipts-root" in str(exc)
    else:
        raise AssertionError("BSC source TOML without receiptsRoot was accepted")

    assert module._toml_receipt_metadata_ready(args) is False


def test_bsc_source_json_summary_rejects_non_boolean_readiness_helpers(monkeypatch):
    module = load_evidence_module()
    cases = (
        (
            "_toml_receipt_metadata_ready",
            "BSC source bridge receipt metadata readiness must be a boolean",
        ),
        (
            "_toml_runtime_bytecode_metadata_ready",
            "BSC source bridge runtime bytecode readiness must be a boolean",
        ),
    )

    for helper_name, expected_error in cases:
        args = bsc_args(module)

        with monkeypatch.context() as patch:
            patch.setattr(module, helper_name, lambda _args: "ready")
            try:
                module._json_summary(args)
            except ValueError as exc:
                assert expected_error in str(exc)
            else:
                raise AssertionError(
                    f"BSC source summary accepted non-boolean {helper_name}"
                )


def test_bsc_source_bridge_rejects_hostile_expected_record_hashes_without_hooks():
    module = load_evidence_module()
    cases = (
        (
            "expected_source_verifier_material_hash",
            HostileExpectedRecordHash,
            "--expected-source-verifier-material-hash must be bytes",
        ),
        (
            "expected_source_verifier_material_hash",
            lambda: b"\x00" * 32,
            "--expected-source-verifier-material-hash must not be zero",
        ),
        (
            "expected_source_verifier_material_hash",
            lambda: b"\x11" * 31,
            "--expected-source-verifier-material-hash must be 32 bytes",
        ),
        (
            "expected_source_adapter_engine_deployment_hash",
            HostileExpectedRecordHash,
            "--expected-source-adapter-engine-deployment-hash must be bytes",
        ),
        (
            "expected_source_adapter_engine_deployment_hash",
            lambda: b"\x00" * 32,
            "--expected-source-adapter-engine-deployment-hash must not be zero",
        ),
        (
            "expected_source_adapter_engine_deployment_hash",
            lambda: b"\x11" * 31,
            "--expected-source-adapter-engine-deployment-hash must be 32 bytes",
        ),
    )
    actions = (
        ("_json_summary", module._json_summary),
        ("_require_expected_record_hashes", module._require_expected_record_hashes),
    )

    for field, make_value, expected_error in cases:
        for action_name, action in actions:
            args = bsc_args(module)
            setattr(args, field, make_value())

            try:
                action(args)
            except ValueError as exc:
                message = str(exc)
                assert expected_error in message
                assert "secret-token" not in message
                assert exc.__cause__ is None
            else:
                raise AssertionError(
                    f"BSC source {action_name} accepted malformed {field}"
                )


def test_bsc_source_evidence_rejects_boolean_target_domain():
    module = load_evidence_module()
    args = bsc_args(module)
    args.target_domain = False

    try:
        module._json_summary(args)
    except ValueError as exc:
        assert "target_domain must be an exact u32" in str(exc)
    else:
        raise AssertionError("boolean BSC target domain was accepted")

    try:
        module.bsc_source_adapter_verifier_vk_hash(target_domain=False)
    except ValueError as exc:
        assert "target_domain must be an exact u32" in str(exc)
    else:
        raise AssertionError("boolean BSC target domain reached vk hash derivation")


def test_bsc_source_evidence_rejects_boolean_source_domain():
    module = load_evidence_module()
    args = bsc_args(module)
    args.source_domain = True

    try:
        module._json_summary(args)
    except ValueError as exc:
        assert "source_domain must be an exact u32" in str(exc)
    else:
        raise AssertionError("boolean BSC source domain was accepted")

    try:
        module.bsc_source_adapter_verifier_vk_hash(source_domain=True)
    except ValueError as exc:
        assert "source_domain must be an exact u32" in str(exc)
    else:
        raise AssertionError("boolean BSC source domain reached vk hash derivation")


def test_bsc_toml_rendering_rejects_reused_role_hashes():
    module = load_evidence_module()
    args = bsc_args(module)
    args.source_bridge_emitter_code_hash = args.consensus_verifier_hash

    try:
        module.render_toml(args)
    except ValueError as exc:
        assert "source_bridge_emitter_code_hash matches consensus_verifier_hash" in str(exc)
    else:
        raise AssertionError("BSC TOML accepted reused source-adapter role hashes")


def test_bsc_direct_record_hashes_reject_reused_role_hashes():
    module = load_evidence_module()

    material_args = bsc_args(module)
    material_args.source_bridge_emitter_code_hash = (
        material_args.consensus_verifier_hash
    )
    try:
        module.bsc_source_verifier_material_record_hash(material_args)
    except ValueError as exc:
        assert "source_bridge_emitter_code_hash matches consensus_verifier_hash" in str(
            exc
        )
    else:
        raise AssertionError("BSC material hash accepted reused role hashes")

    deployment_args = bsc_args(module)
    deployment_args.deployment_receipt_hash = deployment_args.adapter_verifier_vk_hash
    try:
        module.bsc_source_adapter_engine_deployment_record_hash(deployment_args)
    except ValueError as exc:
        assert "deployment_receipt_hash matches adapter_verifier_vk_hash" in str(exc)
    else:
        raise AssertionError("BSC deployment hash accepted reused role hashes")


def test_bsc_source_record_hashes_match_rust_vectors():
    module = load_evidence_module()
    args = bsc_args(module)
    testnet_args = bsc_args(module, bsc_network="testnet")

    assert (
        module.bsc_source_verifier_material_record_hash(args).hex()
        == BSC_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
    )
    assert (
        module.bsc_source_adapter_engine_deployment_record_hash(args).hex()
        == BSC_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
    )
    assert (
        module.bsc_source_adapter_verifier_vk_hash(bsc_network="testnet").hex()
        == BSC_TESTNET_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
    )
    assert (
        module.bsc_source_verifier_material_record_hash(testnet_args).hex()
        == BSC_TESTNET_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
    )
    assert (
        module.bsc_source_adapter_engine_deployment_record_hash(testnet_args).hex()
        == BSC_TESTNET_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
    )
    assert (
        module.bsc_source_verifier_material_record_hash(args)
        != module.bsc_source_verifier_material_record_hash(testnet_args)
    )

    forged_testnet = bsc_args(module, bsc_network="testnet")
    forged_testnet.adapter_verifier_vk_hash = bytes.fromhex(
        BSC_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
    )
    try:
        module.bsc_source_adapter_engine_deployment_record_hash(forged_testnet)
    except ValueError as exc:
        assert "canonical BSC source-adapter verifier profile" in str(exc)
    else:
        raise AssertionError("BSC testnet source evidence accepted mainnet vk hash")


def test_bsc_direct_record_hashes_reject_zero_production_inputs():
    module = load_evidence_module()
    material_fields = (
        "source_trust_anchor_hash",
        "consensus_verifier_hash",
        "message_inclusion_verifier_hash",
        "source_bridge_emitter_code_hash",
        "finality_policy_hash",
        "bridge_address",
    )
    deployment_fields = (
        *material_fields,
        "adapter_verifier_vk_hash",
        "deployment_receipt_hash",
    )

    for field in material_fields:
        args = bsc_args(module)
        setattr(args, field, bytes(20 if field == "bridge_address" else 32))
        try:
            module.bsc_source_verifier_material_record_hash(args)
        except ValueError as exc:
            assert f"{field} must not be zero" in str(exc)
        else:
            raise AssertionError(f"BSC material hash accepted zero {field}")

    for field in deployment_fields:
        args = bsc_args(module)
        setattr(args, field, bytes(20 if field == "bridge_address" else 32))
        try:
            module.bsc_source_adapter_engine_deployment_record_hash(args)
        except ValueError as exc:
            assert f"{field} must not be zero" in str(exc)
        else:
            raise AssertionError(f"BSC deployment hash accepted zero {field}")


def test_bsc_direct_record_hashes_reject_template_component_hashes():
    module = load_evidence_module()
    for bsc_network in ("mainnet", "testnet"):
        for field, (component_id, component_kind) in module.bsc_template_components(
            bsc_network
        ).items():
            template_hash = module._evm_family_template_component_hash(
                component_id,
                component_kind,
                bsc_network=bsc_network,
            )
            label = field.replace("_", " ")

            material_args = bsc_args(module, bsc_network=bsc_network)
            setattr(material_args, field, template_hash)
            try:
                module.bsc_source_verifier_material_record_hash(material_args)
            except ValueError as exc:
                assert f"template-derived {label}" in str(exc)
            else:
                raise AssertionError(
                    f"BSC {bsc_network} material hash accepted template {label}"
                )

            deployment_args = bsc_args(module, bsc_network=bsc_network)
            setattr(deployment_args, field, template_hash)
            try:
                module.bsc_source_adapter_engine_deployment_record_hash(deployment_args)
            except ValueError as exc:
                assert f"template-derived {label}" in str(exc)
            else:
                raise AssertionError(
                    f"BSC {bsc_network} deployment hash accepted template {label}"
                )


def test_bsc_direct_record_hashes_reject_cross_role_template_component_hashes():
    module = load_evidence_module()

    for bsc_network in ("mainnet", "testnet"):
        template_hash = module._template_component_hashes(bsc_network)[
            "consensus_verifier_hash"
        ]
        for record_hash, field in (
            (module.bsc_source_verifier_material_record_hash, "source_trust_anchor_hash"),
            (
                module.bsc_source_adapter_engine_deployment_record_hash,
                "source_bridge_emitter_code_hash",
            ),
        ):
            args = bsc_args(module, bsc_network=bsc_network)
            setattr(args, field, template_hash)
            label = field.replace("_", " ")

            try:
                record_hash(args)
            except ValueError as exc:
                assert f"live {label}" in str(exc)
                assert "template-derived consensus verifier hash" in str(exc)
            else:
                raise AssertionError(
                    f"BSC {bsc_network} record hash accepted cross-role template hash for {field}"
                )


def test_bsc_direct_record_hashes_reject_other_supported_profile_template_component_hashes():
    module = load_evidence_module()

    for bsc_network, template_network in (("mainnet", "testnet"), ("testnet", "mainnet")):
        template_hash = module._template_component_hashes(template_network)[
            "consensus_verifier_hash"
        ]
        for record_hash, field in (
            (module.bsc_source_verifier_material_record_hash, "source_trust_anchor_hash"),
            (
                module.bsc_source_adapter_engine_deployment_record_hash,
                "source_bridge_emitter_code_hash",
            ),
        ):
            args = bsc_args(module, bsc_network=bsc_network)
            setattr(args, field, template_hash)
            label = field.replace("_", " ")

            try:
                record_hash(args)
            except ValueError as exc:
                assert f"live {label}" in str(exc)
                assert (
                    f"template-derived {template_network} consensus verifier hash"
                    in str(exc)
                )
            else:
                raise AssertionError(
                    f"BSC {bsc_network} record hash accepted {template_network} template hash for {field}"
                )


def test_bsc_direct_record_hashes_reject_foreign_active_lane_template_component_hashes():
    module = load_evidence_module()
    template_hash = active_template_hash(
        module,
        "Solana",
        "consensus_verifier_hash",
    )

    for bsc_network in ("mainnet", "testnet"):
        for record_hash, field in (
            (module.bsc_source_verifier_material_record_hash, "source_trust_anchor_hash"),
            (
                module.bsc_source_adapter_engine_deployment_record_hash,
                "source_bridge_emitter_code_hash",
            ),
        ):
            args = bsc_args(module, bsc_network=bsc_network)
            setattr(args, field, template_hash)
            label = field.replace("_", " ")

            try:
                record_hash(args)
            except ValueError as exc:
                assert f"live {label}" in str(exc)
                assert "template-derived Solana consensus verifier hash" in str(exc)
            else:
                raise AssertionError(
                    f"BSC {bsc_network} record hash accepted foreign template hash for {field}"
                )


def test_bsc_source_deployment_hash_rejects_noncanonical_adapter_vk_hash():
    module = load_evidence_module()
    args = bsc_args(module)
    args.adapter_verifier_vk_hash = bytes.fromhex("99" * 32)

    try:
        module.bsc_source_adapter_engine_deployment_record_hash(args)
    except ValueError as exc:
        assert "canonical BSC source-adapter verifier profile" in str(exc)
    else:
        raise AssertionError("noncanonical BSC adapter vk hash was accepted")


def test_bsc_source_evidence_rejects_template_component_hashes():
    module = load_evidence_module()
    for bsc_network in ("mainnet", "testnet"):
        for field, (component_id, component_kind) in module.bsc_template_components(
            bsc_network
        ).items():
            args = bsc_args(module, bsc_network=bsc_network)
            setattr(
                args,
                field,
                module._evm_family_template_component_hash(
                    component_id,
                    component_kind,
                    bsc_network=bsc_network,
                ),
            )
            label = field.replace("_", " ")

            try:
                module.render_toml(args)
            except ValueError as exc:
                assert f"template-derived {label}" in str(exc)
            else:
                raise AssertionError(
                    f"template BSC {bsc_network} {label} was accepted"
                )


def test_bsc_source_evidence_rejects_wrong_lane_domains_with_named_constants():
    module = load_evidence_module()

    source_args = bsc_args(module)
    source_args.source_domain = module.SCCP_DOMAIN_SORA
    try:
        module.render_toml(source_args)
    except ValueError as exc:
        assert "source_domain = SCCP_DOMAIN_BSC (2)" in str(exc)
    else:
        raise AssertionError("BSC source evidence accepted non-BSC source domain")

    target_args = bsc_args(module)
    target_args.target_domain = module.SCCP_DOMAIN_BSC
    try:
        module.render_toml(target_args)
    except ValueError as exc:
        assert "target_domain = SCCP_DOMAIN_SORA (0)" in str(exc)
    else:
        raise AssertionError("BSC source evidence accepted non-SORA target domain")


def test_bsc_cli_json_summary_and_toml_output(capsys):
    module = load_evidence_module()
    hash_only_args = [
        "--bridge-address",
        "0x" + "11" * 20,
        "--source-trust-anchor-hash",
        "0x" + "44" * 32,
        "--consensus-verifier-hash",
        "0x" + "55" * 32,
        "--message-inclusion-verifier-hash",
        "0x" + "66" * 32,
        "--source-bridge-emitter-code-hash",
        "0x" + "77" * 32,
        "--finality-policy-hash",
        "0x" + "88" * 32,
        "--adapter-verifier-vk-hash",
        "0x" + BSC_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
        "--deployment-receipt-hash",
        "0x" + "aa" * 32,
        "--deployment-transaction-hash",
        "0x" + "bd" * 32,
        "--deployment-transaction-block-hash",
        "0x" + "bb" * 32,
        "--deployment-transaction-block-number",
        "4660",
        "--deployment-transaction-input-sha256",
        "0x" + "cd" * 32,
        "--deployment-receipt-contract-address",
        "0x" + "11" * 20,
        "--deployment-receipt-block-hash",
        "0x" + "bb" * 32,
        "--deployment-receipt-block-number",
        "4660",
        "--deployment-receipt-block-receipts-root",
        "0x" + "bc" * 32,
        "--expected-source-verifier-material-hash",
        "0x" + BSC_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR,
        "--expected-source-adapter-engine-deployment-hash",
        "0x" + BSC_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR,
    ]
    unpinned_args = hash_only_args[:-4]

    assert module.main(unpinned_args) == 0
    unpinned = json.loads(capsys.readouterr().out)
    assert unpinned["expected_source_verifier_material_hash_matches"] is False
    assert unpinned["expected_source_adapter_engine_deployment_hash_matches"] is False
    assert unpinned["toml_ready"] is False

    try:
        module.main([*unpinned_args, "--toml"])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("unpinned BSC source TOML was accepted")

    assert module.main(hash_only_args) == 0
    hash_only = json.loads(capsys.readouterr().out)
    assert hash_only["expected_source_verifier_material_hash_matches"] is True
    assert hash_only["expected_source_adapter_engine_deployment_hash_matches"] is True
    assert hash_only["toml_ready"] is False

    try:
        module.main([*hash_only_args, "--toml"])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("hash-only BSC source TOML was accepted")

    runtime_bytecode = bytes.fromhex("6080604052")
    expected = bsc_args(module)
    expected.source_bridge_emitter_code_hash = module.runtime_bytecode_hash(
        runtime_bytecode
    )
    expected_material_hash = module.bsc_source_verifier_material_record_hash(expected)
    expected_deployment_hash = module.bsc_source_adapter_engine_deployment_record_hash(
        expected
    )
    args = [
        "--bridge-address",
        "0x" + "11" * 20,
        "--source-trust-anchor-hash",
        "0x" + "44" * 32,
        "--consensus-verifier-hash",
        "0x" + "55" * 32,
        "--message-inclusion-verifier-hash",
        "0x" + "66" * 32,
        "--source-bridge-runtime-bytecode-hex",
        "0x" + runtime_bytecode.hex(),
        "--finality-policy-hash",
        "0x" + "88" * 32,
        "--adapter-verifier-vk-hash",
        "0x" + BSC_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
        "--deployment-receipt-hash",
        "0x" + "aa" * 32,
        "--deployment-transaction-hash",
        "0x" + "bd" * 32,
        "--deployment-transaction-block-hash",
        "0x" + "bb" * 32,
        "--deployment-transaction-block-number",
        "4660",
        "--deployment-transaction-input-sha256",
        "0x" + "cd" * 32,
        "--deployment-receipt-contract-address",
        "0x" + "11" * 20,
        "--deployment-receipt-block-hash",
        "0x" + "bb" * 32,
        "--deployment-receipt-block-number",
        "4660",
        "--deployment-receipt-block-receipts-root",
        "0x" + "bc" * 32,
        "--expected-source-verifier-material-hash",
        "0x" + expected_material_hash.hex(),
        "--expected-source-adapter-engine-deployment-hash",
        "0x" + expected_deployment_hash.hex(),
    ]

    assert module.main(args) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["source_domain"] == 2
    assert output["target_domain"] == 0
    assert output["source_bridge_emitter_address"] == "0x" + "11" * 20
    assert output["source_bridge_emitter_code_hash"] == (
        "0x" + expected.source_bridge_emitter_code_hash.hex()
    )
    assert output["source_bridge_runtime_bytecode_hex"] == (
        "0x" + runtime_bytecode.hex()
    )
    assert output["deployment_transaction_block_hash"] == "0x" + "bb" * 32
    assert output["deployment_transaction_block_number"] == 4660
    assert output["deployment_transaction_input_sha256"] == "0x" + "cd" * 32
    assert (
        output["adapter_verifier_vk_hash"]
        == "0x" + BSC_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
    )
    assert (
        output["source_verifier_material_hash"]
        == "0x" + expected_material_hash.hex()
    )
    assert (
        output["source_adapter_engine_deployment_hash"]
        == "0x" + expected_deployment_hash.hex()
    )
    assert output["expected_source_verifier_material_hash_matches"] is True
    assert output["expected_source_adapter_engine_deployment_hash_matches"] is True
    assert output["toml_ready"] is True

    assert module.main([*args, "--toml"]) == 0
    rendered = capsys.readouterr().out
    assert '# sccp_evm_source_rpc_chain_id = "56"' in rendered
    assert (
        '# sccp_evm_source_bridge_runtime_bytecode_hex = "0x'
        + runtime_bytecode.hex()
        + '"'
        in rendered
    )
    assert '# sccp_evm_source_deployment_transaction_hash = "0x' + "bd" * 32 in rendered
    assert "# sccp_evm_source_deployment_transaction_block_hash" in rendered
    assert "# sccp_evm_source_deployment_transaction_input_sha256" in rendered
    assert '# sccp_evm_source_deployment_block_number = "4660"' in rendered
    assert "[[zk.sccp_source_verifier_materials]]" in rendered
    assert "[[zk.sccp_source_adapter_engine_deployments]]" in rendered

    testnet_expected = bsc_args(module, bsc_network="testnet")
    testnet_expected.source_bridge_emitter_code_hash = module.runtime_bytecode_hash(
        runtime_bytecode
    )
    testnet_material_hash = module.bsc_source_verifier_material_record_hash(
        testnet_expected
    )
    testnet_deployment_hash = module.bsc_source_adapter_engine_deployment_record_hash(
        testnet_expected
    )
    testnet_args = [
        "--bsc-network",
        "testnet",
        "--bridge-address",
        "0x" + "11" * 20,
        "--source-trust-anchor-hash",
        "0x" + "44" * 32,
        "--consensus-verifier-hash",
        "0x" + "55" * 32,
        "--message-inclusion-verifier-hash",
        "0x" + "66" * 32,
        "--source-bridge-runtime-bytecode-hex",
        "0x" + runtime_bytecode.hex(),
        "--finality-policy-hash",
        "0x" + "88" * 32,
        "--adapter-verifier-vk-hash",
        "0x" + BSC_TESTNET_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
        "--deployment-receipt-hash",
        "0x" + "aa" * 32,
        "--deployment-transaction-hash",
        "0x" + "bd" * 32,
        "--deployment-transaction-block-hash",
        "0x" + "bb" * 32,
        "--deployment-transaction-block-number",
        "4660",
        "--deployment-transaction-input-sha256",
        "0x" + "cd" * 32,
        "--deployment-receipt-contract-address",
        "0x" + "11" * 20,
        "--deployment-receipt-block-hash",
        "0x" + "bb" * 32,
        "--deployment-receipt-block-number",
        "4660",
        "--deployment-receipt-block-receipts-root",
        "0x" + "bc" * 32,
        "--expected-source-verifier-material-hash",
        "0x" + testnet_material_hash.hex(),
        "--expected-source-adapter-engine-deployment-hash",
        "0x" + testnet_deployment_hash.hex(),
    ]

    assert module.main(testnet_args) == 0
    testnet_output = json.loads(capsys.readouterr().out)
    assert testnet_output["source_chain"] == "bsc-testnet"
    assert testnet_output["rpc_chain_id"] == 97
    assert testnet_output["source_bridge_emitter_id"] == (
        "sccp:bsc:source-bridge-emitter:bsc-testnet:v1"
    )
    assert testnet_output["source_verifier_material_hash"] == (
        "0x" + testnet_material_hash.hex()
    )
    assert testnet_output["source_adapter_engine_deployment_hash"] == (
        "0x" + testnet_deployment_hash.hex()
    )
    assert testnet_output["toml_ready"] is True

    try:
        module.main(
            [
                value
                if value != "0x" + testnet_material_hash.hex()
                else "0x" + BSC_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
                for value in testnet_args
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("BSC testnet CLI accepted mainnet expected material hash")


def test_bsc_cli_rejects_expected_record_hash_mismatch():
    module = load_evidence_module()
    args = [
        "--bridge-address",
        "0x" + "11" * 20,
        "--source-trust-anchor-hash",
        "0x" + "44" * 32,
        "--consensus-verifier-hash",
        "0x" + "55" * 32,
        "--message-inclusion-verifier-hash",
        "0x" + "66" * 32,
        "--source-bridge-emitter-code-hash",
        "0x" + "77" * 32,
        "--finality-policy-hash",
        "0x" + "88" * 32,
        "--adapter-verifier-vk-hash",
        "0x" + BSC_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
        "--deployment-receipt-hash",
        "0x" + "aa" * 32,
        "--expected-source-verifier-material-hash",
        "0x" + "99" * 32,
    ]

    try:
        module.main(args)
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("mismatched BSC expected material hash was accepted")


def test_bsc_source_evidence_rejects_adapter_verifier_vk_hash_mismatch():
    module = load_evidence_module()
    args = SimpleNamespace(
        source_domain=2,
        target_domain=0,
        bridge_address=bytes.fromhex("11" * 20),
        source_trust_anchor_hash=bytes.fromhex("44" * 32),
        consensus_verifier_hash=bytes.fromhex("55" * 32),
        message_inclusion_verifier_hash=bytes.fromhex("66" * 32),
        source_bridge_emitter_code_hash=bytes.fromhex("77" * 32),
        finality_policy_hash=bytes.fromhex("88" * 32),
        adapter_verifier_vk_hash=bytes.fromhex("99" * 32),
        deployment_receipt_hash=bytes.fromhex("aa" * 32),
    )

    try:
        module.render_toml(args)
    except ValueError as exc:
        assert "canonical BSC source-adapter verifier profile" in str(exc)
    else:
        raise AssertionError("mismatched BSC adapter verifier vk hash was accepted")


def test_bsc_cli_rejects_non_production_lane():
    module = load_evidence_module()
    args = [
        "--bridge-address",
        "0x" + "11" * 20,
        "--source-trust-anchor-hash",
        "0x" + "44" * 32,
        "--consensus-verifier-hash",
        "0x" + "55" * 32,
        "--message-inclusion-verifier-hash",
        "0x" + "66" * 32,
        "--source-bridge-emitter-code-hash",
        "0x" + "77" * 32,
        "--finality-policy-hash",
        "0x" + "88" * 32,
        "--adapter-verifier-vk-hash",
        "0x" + "99" * 32,
        "--deployment-receipt-hash",
        "0x" + "aa" * 32,
    ]

    try:
        module.main(["--source-domain", "1", *args])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("non-BSC source evidence was accepted")

    try:
        module.main(["--target-domain", "2", *args])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("non-SORA target evidence was accepted")


def test_bsc_cli_requires_code_hash_or_runtime_bytecode():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--bridge-address",
                "0x" + "11" * 20,
                "--source-trust-anchor-hash",
                "0x" + "44" * 32,
                "--consensus-verifier-hash",
                "0x" + "55" * 32,
                "--message-inclusion-verifier-hash",
                "0x" + "66" * 32,
                "--finality-policy-hash",
                "0x" + "88" * 32,
                "--adapter-verifier-vk-hash",
                "0x" + "99" * 32,
                "--deployment-receipt-hash",
                "0x" + "aa" * 32,
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("BSC evidence without code hash was accepted")
