import json
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
from types import SimpleNamespace


ETH_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR = (
    "2140903293411cad0f0eb217d8beb18d3a188edf7bba455098589a2409445e46"
)
ETH_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR = (
    "4d1e9d15bc59c0a2157aa967eb033f5778c805aea4707785a31ef6b60f694d77"
)
ETH_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR = (
    "feb62925410b1376a2cd3704c3822e335da96c3dcc283b041a559d7b08ab1cc4"
)


def load_evidence_module():
    script_path = (
        Path(__file__).resolve().parents[2]
        / "scripts"
        / "sccp_eth_source_bridge_evidence.py"
    )
    spec = spec_from_file_location("sccp_eth_source_bridge_evidence", script_path)
    module = module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


class HostileExpectedRecordHash:
    def __str__(self):
        raise AssertionError("secret-token ETH expected record hash was stringified")

    def __repr__(self):
        raise AssertionError("secret-token ETH expected record hash was repr'd")

    def __eq__(self, _other):
        raise AssertionError("secret-token ETH expected record hash was compared")

    def __ne__(self, _other):
        raise AssertionError("secret-token ETH expected record hash was compared")


class HostileEthSourceBridgeBytes(bytes):
    """Bytes subclass that source-bridge evidence must reject before hooks."""

    def __new__(cls, value):
        return bytes.__new__(cls, value)

    def __bytes__(self):
        raise AssertionError("secret-token ETH source bytes coerced")

    def __repr__(self):
        raise AssertionError("secret-token ETH source bytes repr'd")

    def __str__(self):
        raise AssertionError("secret-token ETH source bytes stringified")

    def __len__(self):
        raise AssertionError("secret-token ETH source bytes length read")

    def __iter__(self):
        raise AssertionError("secret-token ETH source bytes iterated")

    def __getitem__(self, _key):
        raise AssertionError("secret-token ETH source bytes indexed")

    def __eq__(self, _other):
        raise AssertionError("secret-token ETH source bytes compared")

    def __ne__(self, _other):
        raise AssertionError("secret-token ETH source bytes compared")

    def __hash__(self):
        raise AssertionError("secret-token ETH source bytes hashed")


class HostileEthSourceBridgeBytearray(bytearray):
    """Bytearray subclass that source-bridge evidence must reject before hooks."""

    def __init__(self, value):
        super().__init__(value)

    def __bytes__(self):
        raise AssertionError("secret-token ETH source bytearray coerced")

    def __repr__(self):
        raise AssertionError("secret-token ETH source bytearray repr'd")

    def __str__(self):
        raise AssertionError("secret-token ETH source bytearray stringified")

    def __len__(self):
        raise AssertionError("secret-token ETH source bytearray length read")

    def __iter__(self):
        raise AssertionError("secret-token ETH source bytearray iterated")

    def __getitem__(self, _key):
        raise AssertionError("secret-token ETH source bytearray indexed")

    def __eq__(self, _other):
        raise AssertionError("secret-token ETH source bytearray compared")

    def __ne__(self, _other):
        raise AssertionError("secret-token ETH source bytearray compared")

    def __hash__(self):
        raise AssertionError("secret-token ETH source bytearray hashed")


class HostileDeploymentBlockNumber:
    def __str__(self):
        raise AssertionError("secret-token ETH deployment block number was stringified")

    def __repr__(self):
        raise AssertionError("secret-token ETH deployment block number was repr'd")

    def __eq__(self, _other):
        raise AssertionError("secret-token ETH deployment block number was compared")

    def __ne__(self, _other):
        raise AssertionError("secret-token ETH deployment block number was compared")


class HostileTomlString(str):
    def __new__(cls):
        return str.__new__(cls, "blocked")

    def __str__(self):
        raise AssertionError("secret-token ETH source TOML string was stringified")

    def __repr__(self):
        raise AssertionError("secret-token ETH source TOML string was repr'd")

    def __eq__(self, _other):
        raise AssertionError("secret-token ETH source TOML string was compared")

    def __ne__(self, _other):
        raise AssertionError("secret-token ETH source TOML string was compared")


class HostileTomlInt(int):
    def __new__(cls):
        return int.__new__(cls, 1)

    def __str__(self):
        raise AssertionError("secret-token ETH source TOML integer was stringified")

    def __repr__(self):
        raise AssertionError("secret-token ETH source TOML integer was repr'd")


class HostileTomlList(list):
    def __iter__(self):
        raise AssertionError("secret-token ETH source TOML list was iterated")

    def __repr__(self):
        raise AssertionError("secret-token ETH source TOML list was repr'd")


def eth_args(module):
    return SimpleNamespace(
        source_domain=1,
        target_domain=0,
        bridge_address=bytes.fromhex("11" * 20),
        source_trust_anchor_hash=bytes.fromhex("44" * 32),
        consensus_verifier_hash=bytes.fromhex("55" * 32),
        message_inclusion_verifier_hash=bytes.fromhex("66" * 32),
        source_bridge_emitter_code_hash=bytes.fromhex("77" * 32),
        finality_policy_hash=bytes.fromhex("88" * 32),
        adapter_verifier_vk_hash=bytes.fromhex(
            ETH_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
        ),
        deployment_receipt_hash=bytes.fromhex("aa" * 32),
        deployment_transaction_hash=bytes.fromhex("de" * 32),
        deployment_transaction_block_hash=bytes.fromhex("bb" * 32),
        deployment_transaction_block_number=4660,
        deployment_transaction_input_sha256=bytes.fromhex("cd" * 32),
        deployment_receipt_contract_address=bytes.fromhex("11" * 20),
        deployment_receipt_block_hash=bytes.fromhex("bb" * 32),
        deployment_receipt_block_number=4660,
        deployment_receipt_block_receipts_root=bytes.fromhex("bc" * 32),
        expected_source_verifier_material_hash=bytes.fromhex(
            ETH_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
        ),
        expected_source_adapter_engine_deployment_hash=bytes.fromhex(
            ETH_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
        ),
    )


def active_template_hash(module, lane, field):
    for template_lane, template_field, template_hash in (
        module.sccp_active_source_template_component_hashes()
    ):
        if template_lane == lane and template_field == field:
            return template_hash
    raise AssertionError(f"missing active {lane} template hash for {field}")


def test_eth_cli_redacts_top_level_exception_details(monkeypatch, capsys):
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
                        "0x" + ETH_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
                        "--deployment-receipt-hash",
                        "0x" + "aa" * 32,
                    ]
                )
            except SystemExit as exc:
                assert exc.code == 2
            else:
                raise AssertionError("ETH source CLI accepted top-level render failure")

            captured = capsys.readouterr()
            assert "SCCP Ethereum source bridge evidence rendering failed" in captured.err
            assert "secret-token" not in captured.err
            assert "private-path" not in captured.err
            assert exception_type.__name__ not in captured.err


def test_eth_address_parser_rejects_zero_and_wrong_width(tmp_path):
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
            raise AssertionError("non-canonical ETH bridge address was accepted")

    for value in (" 0x6080604052", "0x6080\n604052"):
        try:
            module.parse_runtime_bytecode_hex(
                value,
                label="source bridge runtime bytecode",
            )
        except module.argparse.ArgumentTypeError as exc:
            assert "must not contain whitespace" in str(exc)
        else:
            raise AssertionError("padded ETH runtime bytecode was accepted")

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
            raise AssertionError("non-canonical ETH runtime bytecode was accepted")

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
        raise AssertionError("zero ETH bridge address was accepted")

    try:
        module.parse_evm_address(
            " 0x" + "11" * 20 + " ",
            label="bridge address",
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must not contain whitespace" in str(exc)
    else:
        raise AssertionError("padded ETH bridge address was accepted")

    try:
        module.parse_evm_address("0x" + "11" * 19, label="bridge address")
    except module.argparse.ArgumentTypeError as exc:
        assert "must be 20 bytes" in str(exc)
    else:
        raise AssertionError("short ETH bridge address was accepted")


def test_eth_source_bridge_direct_parsers_redact_parser_causes(tmp_path):
    module = load_evidence_module()

    fixed_payload = "secret-token-eth-source-hex"
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
        raise AssertionError("secret ETH source bridge fixed hex was accepted")

    runtime_payload = "secret-token-eth-source-runtime0"
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
        raise AssertionError("secret ETH source bridge runtime hex was accepted")

    secret_path = tmp_path / "secret-token-eth-source-file-path.hex"
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
        raise AssertionError("missing secret ETH source bridge runtime file was accepted")


def test_eth_runtime_bytecode_file_rejects_unreadable_file_shapes(tmp_path):
    module = load_evidence_module()
    outside = tmp_path / "secret-token-eth-runtime-outside.hex"
    outside.write_text("0x6080604052\n", encoding="utf-8")
    symlink_input = tmp_path / "secret-token-eth-runtime-link.hex"
    symlink_input.symlink_to(outside)
    directory_input = tmp_path / "secret-token-eth-runtime-dir.hex"
    directory_input.mkdir()
    missing_input = tmp_path / "secret-token-eth-runtime-missing.hex"

    class HostileRuntimeBytecodePath(str):
        def __new__(cls):
            return str.__new__(cls, str(outside))

        def __str__(self):
            raise AssertionError("secret-token ETH runtime path was stringified")

        def __repr__(self):
            raise AssertionError("secret-token ETH runtime path was repr'd")

        def __fspath__(self):
            raise AssertionError("secret-token ETH runtime path was coerced")

    class HostileRuntimeBytecodePathLike:
        def __str__(self):
            raise AssertionError("secret-token ETH runtime path-like was stringified")

        def __repr__(self):
            raise AssertionError("secret-token ETH runtime path-like was repr'd")

        def __fspath__(self):
            raise AssertionError("secret-token ETH runtime path-like was coerced")

    for path in (
        str(symlink_input),
        str(directory_input),
        str(missing_input),
        outside,
        HostileRuntimeBytecodePath(),
        HostileRuntimeBytecodePathLike(),
    ):
        try:
            module.parse_runtime_bytecode_file(
                path,
                label="source bridge runtime bytecode",
            )
        except module.argparse.ArgumentTypeError as exc:
            rendered = str(exc)
            suppress_context = exc.__suppress_context__
        else:
            raise AssertionError("ETH runtime bytecode file shape was accepted")

        assert rendered == "source bridge runtime bytecode file cannot be read"
        assert "secret-token" not in rendered
        assert "IsADirectoryError" not in rendered
        assert "FileNotFoundError" not in rendered
        assert suppress_context is True


def test_eth_source_bridge_direct_parsers_redact_helper_exit_parser_causes(monkeypatch):
    module = load_evidence_module()

    for exception_type in (
        module.argparse.ArgumentTypeError,
        SystemExit,
        RuntimeError,
        TypeError,
        ValueError,
    ):
        detail = (
            "secret-token ETH source hex TypeError detail"
            if exception_type is TypeError
            else f"secret-token ETH source hex {exception_type.__name__} detail"
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
                        ), "ETH source hex ArgumentTypeError detail leaked"
                    assert exc.__cause__ is None
                    assert exc.__suppress_context__ is True
                else:
                    raise AssertionError(
                        f"{label} parser {exception_type.__name__} was accepted"
                    )


def test_eth_source_bridge_fixed_hex_nonzero_controls_reject_non_booleans():
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
                "ETH source bridge fixed hex nonzero must be a boolean"
            )
        else:
            raise AssertionError(
                "malformed ETH source bridge fixed-hex nonzero control accepted"
            )


def test_eth_hash_parser_rejects_zero_and_wrong_width():
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
        raise AssertionError("bare ETH component hash was accepted")

    try:
        module.parse_hex_bytes(
            "0x" + "00" * 32,
            label="source trust anchor hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must not be zero" in str(exc)
    else:
        raise AssertionError("zero ETH component hash was accepted")

    try:
        module.parse_hex_bytes(
            " 0x" + "44" * 32 + " ",
            label="source trust anchor hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must not contain whitespace" in str(exc)
    else:
        raise AssertionError("padded ETH component hash was accepted")

    try:
        module.parse_hex_bytes(
            "0x" + "44" * 31,
            label="source trust anchor hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must be 32 bytes" in str(exc)
    else:
        raise AssertionError("short ETH component hash was accepted")


def test_eth_source_bridge_fixed_bytes_reject_subclasses_without_hooks():
    module = load_evidence_module()
    raw = b"\x44" * 32

    assert (
        module._require_fixed_bytes(
            bytearray(raw),
            label="source_trust_anchor_hash",
            byte_length=32,
        )
        == raw
    )
    exact_bytearray_args = eth_args(module)
    exact_bytearray_args.source_trust_anchor_hash = bytearray(raw)
    assert type(module.eth_source_verifier_material_record_hash(exact_bytearray_args)) is bytes

    hostile_values = (
        HostileEthSourceBridgeBytes(raw),
        HostileEthSourceBridgeBytearray(raw),
    )
    for hostile in hostile_values:
        cases = (
            (
                lambda hostile=hostile: module._require_fixed_bytes(
                    hostile,
                    label="source_trust_anchor_hash",
                    byte_length=32,
                ),
                "source_trust_anchor_hash must be bytes",
            ),
            (
                lambda hostile=hostile: module._require_nonzero_fixed_bytes(
                    hostile,
                    label="source_trust_anchor_hash",
                    byte_length=32,
                ),
                "source_trust_anchor_hash must be bytes",
            ),
            (
                lambda hostile=hostile: module._optional_expected_record_hash(
                    SimpleNamespace(expected_source_verifier_material_hash=hostile),
                    "expected_source_verifier_material_hash",
                    label="--expected-source-verifier-material-hash",
                ),
                "--expected-source-verifier-material-hash must be bytes",
            ),
        )
        for call, expected_message in cases:
            try:
                call()
            except ValueError as exc:
                rendered = str(exc)
                assert rendered == expected_message
                assert "secret-token" not in rendered
                assert exc.__cause__ is None
            else:
                raise AssertionError(
                    "ETH source-bridge byte subclass value was accepted"
                )

        material_args = eth_args(module)
        material_args.source_trust_anchor_hash = hostile
        try:
            module.eth_source_verifier_material_record_hash(material_args)
        except ValueError as exc:
            rendered = str(exc)
            assert rendered == "source_trust_anchor_hash must be bytes"
            assert "secret-token" not in rendered
            assert exc.__cause__ is None
        else:
            raise AssertionError("ETH source material accepted hostile source hash")

        adapter_args = eth_args(module)
        adapter_args.adapter_verifier_vk_hash = hostile
        try:
            module._require_canonical_adapter_verifier_vk_hash(adapter_args)
        except ValueError as exc:
            rendered = str(exc)
            assert rendered == "adapter_verifier_vk_hash must be bytes"
            assert "secret-token" not in rendered
            assert exc.__cause__ is None
        else:
            raise AssertionError(
                "ETH source adapter accepted hostile verifier hash"
            )


def test_eth_source_numeric_parsers_require_canonical_ascii_decimal():
    module = load_evidence_module()

    assert module.parse_u32("1", label="source domain") == 1
    assert module.parse_positive_u64("4660", label="deployment block number") == 4660

    for value in ("01", "0x1", "+1", " 1 ", "١"):
        try:
            module.parse_u32(value, label="source domain")
        except module.argparse.ArgumentTypeError as exc:
            assert "must be a u32" in str(exc)
        else:
            raise AssertionError(f"noncanonical ETH source domain {value!r} was accepted")

    for value in ("0", "04660", "0x1234", "+4660", " 4660 ", "٤٦٦٠"):
        try:
            module.parse_positive_u64(value, label="deployment block number")
        except module.argparse.ArgumentTypeError as exc:
            assert "must be a positive u64" in str(exc)
        else:
            raise AssertionError(f"noncanonical ETH block number {value!r} was accepted")


def test_eth_runtime_bytecode_derives_source_bridge_code_hash():
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


def test_eth_runtime_bytecode_rejects_mismatched_source_bridge_code_hash():
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
        raise AssertionError("mismatched ETH runtime bytecode hash was accepted")


def test_eth_direct_renderers_derive_source_bridge_code_hash_from_runtime_bytecode():
    module = load_evidence_module()
    runtime_bytecode = bytes.fromhex("6080604052")
    source_bridge_code_hash = module.runtime_bytecode_hash(runtime_bytecode)

    expected_args = eth_args(module)
    expected_args.source_bridge_emitter_code_hash = source_bridge_code_hash
    expected_material_hash = module.eth_source_verifier_material_record_hash(
        expected_args
    )
    expected_deployment_hash = module.eth_source_adapter_engine_deployment_record_hash(
        expected_args
    )

    render_args = eth_args(module)
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
        '# sccp_eth_source_verifier_material_hash = "0x'
        + expected_material_hash.hex()
        + '"'
        in rendered
    )
    assert (
        '# sccp_evm_source_bridge_runtime_bytecode_hex = "0x'
        + runtime_bytecode.hex()
        + '"'
        in rendered
    )

    summary_args = eth_args(module)
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


def test_eth_toml_rendering_carries_mainnet_profile_ids_and_emitter_binding():
    module = load_evidence_module()
    assert (
        module.eth_source_adapter_verifier_vk_hash().hex()
        == ETH_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
    )
    args = eth_args(module)
    runtime_bytecode = bytes.fromhex("6080604052")
    source_bridge_code_hash = module.runtime_bytecode_hash(runtime_bytecode)
    args.source_bridge_emitter_code_hash = source_bridge_code_hash
    args.source_bridge_runtime_bytecode_hex = runtime_bytecode
    args.source_bridge_runtime_bytecode_file = None
    args.expected_source_verifier_material_hash = (
        module.eth_source_verifier_material_record_hash(args)
    )
    args.expected_source_adapter_engine_deployment_hash = (
        module.eth_source_adapter_engine_deployment_record_hash(args)
    )

    rendered = module.render_toml(args)

    assert '# sccp_evm_source_rpc_chain_id = "1"' in rendered
    assert '# sccp_evm_source_block_tag = "finalized"' in rendered
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
        + "de" * 32
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
        '# sccp_eth_source_verifier_material_hash = "0x'
        + args.expected_source_verifier_material_hash.hex()
        + '"'
        in rendered
    )
    assert "[[zk.sccp_source_verifier_materials]]" in rendered
    assert (
        '# sccp_eth_source_adapter_engine_deployment_hash = "0x'
        + args.expected_source_adapter_engine_deployment_hash.hex()
        + '"'
        in rendered
    )
    assert "[[zk.sccp_source_adapter_engine_deployments]]" in rendered
    assert 'source_domain = 1' in rendered
    assert 'target_domain = 0' in rendered
    assert 'source_chain = "eth"' in rendered
    assert 'source_proof_plan = "EthereumBeaconReceiptProof"' in rendered
    assert 'finality_model = "EthereumBeaconExecution"' in rendered
    assert (
        'source_trust_anchor_id = "sccp:eth:source-trust-anchor:ethereum-mainnet-beacon-finalized-checkpoint:v1"'
        in rendered
    )
    assert (
        'consensus_verifier_id = "sccp:eth:consensus-verifier:beacon-sync-committee-execution-header-mainnet:v1"'
        in rendered
    )
    assert (
        'source_bridge_emitter_id = "sccp:eth:source-bridge-emitter:ethereum-mainnet:v1"'
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
        + ETH_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
        + '"'
        in rendered
    )
    assert 'deployment_receipt_hash = "0x' + "aa" * 32 + '"' in rendered
    assert (
        'evm_source_gate_hash = "0x' + module.eth_source_gate_hash(args).hex() + '"'
        in rendered
    )
    assert (
        'source_bridge_network_id = "0x'
        + module.eth_source_bridge_network_id().hex()
        + '"'
        in rendered
    )
    assert "source_bridge_owner_address" not in rendered
    assert 'source_bridge_config_hash = "0x' in rendered
    assert "# sccp_eth_source_bridge_config_hash" in rendered


def test_eth_source_toml_rejects_nonfinalized_block_tag():
    module = load_evidence_module()
    args = eth_args(module)
    runtime_bytecode = bytes.fromhex("6080604052")
    args.source_bridge_emitter_code_hash = module.runtime_bytecode_hash(runtime_bytecode)
    args.source_bridge_runtime_bytecode_hex = runtime_bytecode
    args.source_bridge_runtime_bytecode_file = None
    args.expected_source_verifier_material_hash = (
        module.eth_source_verifier_material_record_hash(args)
    )
    args.expected_source_adapter_engine_deployment_hash = (
        module.eth_source_adapter_engine_deployment_record_hash(args)
    )
    args.block_tag = "latest"

    try:
        module.render_toml(args)
    except ValueError as exc:
        assert "Ethereum source TOML requires --block-tag finalized" in str(exc)
    else:
        raise AssertionError("non-finalized ETH source TOML was accepted")

    summary = module._json_summary(args)
    assert summary["block_tag"] == "latest"


def test_eth_source_evidence_rejects_boolean_receipt_block_number():
    module = load_evidence_module()
    args = eth_args(module)
    args.deployment_receipt_block_number = True

    try:
        module.render_toml(args)
    except ValueError as exc:
        assert "--deployment-receipt-block-number must be positive" in str(exc)
    else:
        raise AssertionError("boolean ETH source deployment block number was accepted")

    assert module._toml_receipt_metadata_ready(args) is False


def test_eth_source_evidence_rejects_deployment_transaction_readback_drift():
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
        args = eth_args(module)
        setattr(args, field, value)

        try:
            module.render_toml(args)
        except ValueError as exc:
            assert expected in str(exc)
        else:
            raise AssertionError(f"ETH source TOML accepted drifted {field}")

        assert module._toml_receipt_metadata_ready(args) is False


def test_eth_source_rejects_non_integer_deployment_block_numbers_without_stringifying(
    monkeypatch,
):
    module = load_evidence_module()
    runtime_bytecode = bytes.fromhex("6080604052")

    for field, expected_error in (
        (
            "deployment_transaction_block_number",
            "--deployment-transaction-block-number must be positive",
        ),
        (
            "deployment_receipt_block_number",
            "--deployment-receipt-block-number must be positive",
        ),
    ):
        args = eth_args(module)
        args.source_bridge_emitter_code_hash = module.runtime_bytecode_hash(
            runtime_bytecode
        )
        args.source_bridge_runtime_bytecode_hex = runtime_bytecode
        args.source_bridge_runtime_bytecode_file = None
        args.expected_source_verifier_material_hash = (
            module.eth_source_verifier_material_record_hash(args)
        )
        args.expected_source_adapter_engine_deployment_hash = (
            module.eth_source_adapter_engine_deployment_record_hash(args)
        )
        setattr(args, field, HostileDeploymentBlockNumber())

        try:
            module.render_toml(args)
        except ValueError as exc:
            rendered = str(exc)
            assert rendered == expected_error
            assert "secret-token" not in rendered
            assert exc.__cause__ is None
            assert exc.__suppress_context__ is True
        else:
            raise AssertionError(f"ETH source TOML accepted hostile {field}")

        with monkeypatch.context() as patch:
            patch.setattr(module, "_toml_receipt_metadata_ready", lambda _args: True)
            try:
                module._json_summary(args)
            except ValueError as exc:
                rendered = str(exc)
                assert rendered == expected_error
                assert "secret-token" not in rendered
                assert exc.__cause__ is None
                assert exc.__suppress_context__ is True
            else:
                raise AssertionError(f"ETH source JSON accepted hostile {field}")


def test_eth_source_toml_renderer_rejects_string_subclasses_without_hooks():
    module = load_evidence_module()

    cases = (
        lambda: module._toml_string(HostileTomlString()),
        lambda: module._toml_line("source_chain", HostileTomlString()),
        lambda: module._toml_line("version", HostileTomlInt()),
        lambda: module._toml_line("blockers", HostileTomlList(["blocked"])),
    )

    for render in cases:
        try:
            render()
        except TypeError as exc:
            rendered = str(exc)
            assert "unsupported TOML" in rendered
            assert "secret-token" not in rendered
            assert exc.__cause__ is None
        else:
            raise AssertionError(
                "ETH source TOML renderer accepted hostile subclass value"
            )


def test_eth_source_evidence_requires_receipt_block_receipts_root_for_toml():
    module = load_evidence_module()
    args = eth_args(module)
    args.deployment_receipt_block_receipts_root = None

    try:
        module.render_toml(args)
    except ValueError as exc:
        assert "--deployment-receipt-block-receipts-root" in str(exc)
    else:
        raise AssertionError("ETH source TOML without receiptsRoot was accepted")

    assert module._toml_receipt_metadata_ready(args) is False


def test_eth_source_json_summary_rejects_non_boolean_readiness_helpers(monkeypatch):
    module = load_evidence_module()
    cases = (
        (
            "_toml_receipt_metadata_ready",
            "ETH source bridge receipt metadata readiness must be a boolean",
        ),
        (
            "_toml_runtime_bytecode_metadata_ready",
            "ETH source bridge runtime bytecode readiness must be a boolean",
        ),
    )

    for helper_name, expected_error in cases:
        args = eth_args(module)

        with monkeypatch.context() as patch:
            patch.setattr(module, helper_name, lambda _args: "ready")
            try:
                module._json_summary(args)
            except ValueError as exc:
                assert expected_error in str(exc)
            else:
                raise AssertionError(
                    f"ETH source summary accepted non-boolean {helper_name}"
                )


def test_eth_source_bridge_rejects_hostile_expected_record_hashes_without_hooks():
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
            args = eth_args(module)
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
                    f"ETH source {action_name} accepted malformed {field}"
                )


def test_eth_source_evidence_rejects_boolean_target_domain():
    module = load_evidence_module()
    args = eth_args(module)
    args.target_domain = False

    try:
        module._json_summary(args)
    except ValueError as exc:
        assert "target_domain must be an exact u32" in str(exc)
    else:
        raise AssertionError("boolean ETH target domain was accepted")

    try:
        module.eth_source_adapter_verifier_vk_hash(target_domain=False)
    except ValueError as exc:
        assert "target_domain must be an exact u32" in str(exc)
    else:
        raise AssertionError("boolean ETH target domain reached vk hash derivation")


def test_eth_source_evidence_rejects_boolean_source_domain():
    module = load_evidence_module()
    args = eth_args(module)
    args.source_domain = True

    try:
        module._json_summary(args)
    except ValueError as exc:
        assert "source_domain must be an exact u32" in str(exc)
    else:
        raise AssertionError("boolean ETH source domain was accepted")

    try:
        module.eth_source_adapter_verifier_vk_hash(source_domain=True)
    except ValueError as exc:
        assert "source_domain must be an exact u32" in str(exc)
    else:
        raise AssertionError("boolean ETH source domain reached vk hash derivation")


def test_eth_toml_rendering_rejects_reused_role_hashes():
    module = load_evidence_module()
    args = eth_args(module)
    args.deployment_receipt_hash = args.adapter_verifier_vk_hash

    try:
        module.render_toml(args)
    except ValueError as exc:
        assert "deployment_receipt_hash matches adapter_verifier_vk_hash" in str(exc)
    else:
        raise AssertionError("ETH TOML accepted reused source-adapter role hashes")


def test_eth_direct_record_hashes_reject_reused_role_hashes():
    module = load_evidence_module()

    material_args = eth_args(module)
    material_args.source_bridge_emitter_code_hash = (
        material_args.consensus_verifier_hash
    )
    try:
        module.eth_source_verifier_material_record_hash(material_args)
    except ValueError as exc:
        assert "source_bridge_emitter_code_hash matches consensus_verifier_hash" in str(
            exc
        )
    else:
        raise AssertionError("ETH material hash accepted reused role hashes")

    deployment_args = eth_args(module)
    deployment_args.deployment_receipt_hash = deployment_args.adapter_verifier_vk_hash
    try:
        module.eth_source_adapter_engine_deployment_record_hash(deployment_args)
    except ValueError as exc:
        assert "deployment_receipt_hash matches adapter_verifier_vk_hash" in str(exc)
    else:
        raise AssertionError("ETH deployment hash accepted reused role hashes")


def test_eth_source_bridge_config_hash_rejects_network_id_code_hash_reuse():
    module = load_evidence_module()
    args = eth_args(module)
    network_id = module.eth_source_bridge_network_id()

    try:
        module.eth_source_bridge_config_hash(
            bridge_address=args.bridge_address,
            source_bridge_code_hash=network_id,
            network_id=network_id,
            source_domain=1,
            target_domain=0,
        )
    except ValueError as exc:
        assert (
            "source_bridge_emitter_code_hash must not match source_bridge_network_id"
            in str(exc)
        )
    else:
        raise AssertionError("ETH config hash accepted network-id/code-hash reuse")

    material_args = eth_args(module)
    material_args.source_bridge_emitter_code_hash = network_id
    try:
        module.eth_source_verifier_material_record_hash(material_args)
    except ValueError as exc:
        assert (
            "source_bridge_emitter_code_hash must not match source_bridge_network_id"
            in str(exc)
        )
    else:
        raise AssertionError("ETH material hash accepted network-id/code-hash reuse")

    toml_args = eth_args(module)
    toml_args.source_bridge_emitter_code_hash = network_id
    try:
        module.render_toml(toml_args)
    except ValueError as exc:
        assert (
            "source_bridge_emitter_code_hash must not match source_bridge_network_id"
            in str(exc)
        )
    else:
        raise AssertionError("ETH TOML accepted network-id/code-hash reuse")


def test_eth_source_record_hashes_match_rust_vectors():
    module = load_evidence_module()
    args = eth_args(module)

    assert (
        module.eth_source_verifier_material_record_hash(args).hex()
        == ETH_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
    )
    assert (
        module.eth_source_adapter_engine_deployment_record_hash(args).hex()
        == ETH_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
    )


def test_eth_source_bridge_config_hash_binds_mainnet_lane_and_code_hash():
    module = load_evidence_module()
    args = eth_args(module)
    config_hash = module.eth_source_bridge_config_hash(
        bridge_address=args.bridge_address,
        source_bridge_code_hash=args.source_bridge_emitter_code_hash,
        network_id=module.eth_source_bridge_network_id(),
        source_domain=1,
        target_domain=0,
    )

    assert any(config_hash)
    assert config_hash != module.eth_source_bridge_config_hash(
        bridge_address=args.bridge_address,
        source_bridge_code_hash=bytes.fromhex("78" * 32),
        network_id=module.eth_source_bridge_network_id(),
        source_domain=1,
        target_domain=0,
    )
    for kwargs, expected in (
        ({"network_id": (56).to_bytes(32, "big")}, "source_bridge_network_id"),
        ({"source_domain": 2}, "source_domain must be ETH"),
        ({"target_domain": 1}, "target_domain must be SORA"),
    ):
        params = {
            "bridge_address": args.bridge_address,
            "source_bridge_code_hash": args.source_bridge_emitter_code_hash,
            "network_id": module.eth_source_bridge_network_id(),
            "source_domain": 1,
            "target_domain": 0,
        }
        params.update(kwargs)
        try:
            module.eth_source_bridge_config_hash(**params)
        except ValueError as exc:
            assert expected in str(exc)
        else:
            raise AssertionError("invalid ETH source bridge config hash input was accepted")


def test_eth_direct_record_hashes_reject_zero_production_inputs():
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
        args = eth_args(module)
        setattr(args, field, bytes(20 if field == "bridge_address" else 32))
        try:
            module.eth_source_verifier_material_record_hash(args)
        except ValueError as exc:
            assert f"{field} must not be zero" in str(exc)
        else:
            raise AssertionError(f"ETH material hash accepted zero {field}")

    for field in deployment_fields:
        args = eth_args(module)
        setattr(args, field, bytes(20 if field == "bridge_address" else 32))
        try:
            module.eth_source_adapter_engine_deployment_record_hash(args)
        except ValueError as exc:
            assert f"{field} must not be zero" in str(exc)
        else:
            raise AssertionError(f"ETH deployment hash accepted zero {field}")


def test_eth_direct_record_hashes_reject_template_component_hashes():
    module = load_evidence_module()
    for field, (component_id, component_kind) in module.ETH_TEMPLATE_COMPONENTS.items():
        template_hash = module._evm_family_template_component_hash(
            component_id,
            component_kind,
        )
        label = field.replace("_", " ")

        material_args = eth_args(module)
        setattr(material_args, field, template_hash)
        try:
            module.eth_source_verifier_material_record_hash(material_args)
        except ValueError as exc:
            assert f"template-derived {label}" in str(exc)
        else:
            raise AssertionError(
                f"ETH material hash accepted template {label}"
            )

        deployment_args = eth_args(module)
        setattr(deployment_args, field, template_hash)
        try:
            module.eth_source_adapter_engine_deployment_record_hash(deployment_args)
        except ValueError as exc:
            assert f"template-derived {label}" in str(exc)
        else:
            raise AssertionError(
                f"ETH deployment hash accepted template {label}"
            )


def test_eth_direct_record_hashes_reject_cross_role_template_component_hashes():
    module = load_evidence_module()
    template_hash = module._template_component_hashes()["consensus_verifier_hash"]

    for record_hash, field in (
        (module.eth_source_verifier_material_record_hash, "source_trust_anchor_hash"),
        (
            module.eth_source_adapter_engine_deployment_record_hash,
            "source_bridge_emitter_code_hash",
        ),
    ):
        args = eth_args(module)
        setattr(args, field, template_hash)
        label = field.replace("_", " ")

        try:
            record_hash(args)
        except ValueError as exc:
            assert f"live {label}" in str(exc)
            assert "template-derived consensus verifier hash" in str(exc)
        else:
            raise AssertionError(
                f"ETH record hash accepted cross-role template hash for {field}"
            )


def test_eth_direct_record_hashes_reject_foreign_active_lane_template_component_hashes():
    module = load_evidence_module()
    template_hash = active_template_hash(
        module,
        "Solana",
        "consensus_verifier_hash",
    )

    for record_hash, field in (
        (module.eth_source_verifier_material_record_hash, "source_trust_anchor_hash"),
        (
            module.eth_source_adapter_engine_deployment_record_hash,
            "source_bridge_emitter_code_hash",
        ),
    ):
        args = eth_args(module)
        setattr(args, field, template_hash)
        label = field.replace("_", " ")

        try:
            record_hash(args)
        except ValueError as exc:
            assert f"live {label}" in str(exc)
            assert "template-derived Solana consensus verifier hash" in str(exc)
        else:
            raise AssertionError(
                f"ETH record hash accepted foreign template hash for {field}"
            )


def test_eth_source_deployment_hash_rejects_noncanonical_adapter_vk_hash():
    module = load_evidence_module()
    args = eth_args(module)
    args.adapter_verifier_vk_hash = bytes.fromhex("99" * 32)

    try:
        module.eth_source_adapter_engine_deployment_record_hash(args)
    except ValueError as exc:
        assert "canonical ETH source-adapter verifier profile" in str(exc)
    else:
        raise AssertionError("noncanonical ETH adapter vk hash was accepted")


def test_eth_source_evidence_rejects_template_component_hashes():
    module = load_evidence_module()
    for field, (component_id, component_kind) in module.ETH_TEMPLATE_COMPONENTS.items():
        args = eth_args(module)
        setattr(
            args,
            field,
            module._evm_family_template_component_hash(
                component_id,
                component_kind,
            ),
        )
        label = field.replace("_", " ")

        try:
            module.render_toml(args)
        except ValueError as exc:
            assert f"template-derived {label}" in str(exc)
        else:
            raise AssertionError(f"template ETH {label} was accepted")


def test_eth_source_evidence_rejects_wrong_lane_domains_with_named_constants():
    module = load_evidence_module()

    source_args = eth_args(module)
    source_args.source_domain = module.SCCP_DOMAIN_SORA
    try:
        module.render_toml(source_args)
    except ValueError as exc:
        assert "source_domain = SCCP_DOMAIN_ETH (1)" in str(exc)
    else:
        raise AssertionError("ETH source evidence accepted non-ETH source domain")

    target_args = eth_args(module)
    target_args.target_domain = module.SCCP_DOMAIN_ETH
    try:
        module.render_toml(target_args)
    except ValueError as exc:
        assert "target_domain = SCCP_DOMAIN_SORA (0)" in str(exc)
    else:
        raise AssertionError("ETH source evidence accepted non-SORA target domain")


def test_eth_cli_json_summary_and_toml_output(capsys):
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
        "0x" + ETH_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
        "--deployment-receipt-hash",
        "0x" + "aa" * 32,
        "--deployment-transaction-hash",
        "0x" + "de" * 32,
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
        "0x" + ETH_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR,
        "--expected-source-adapter-engine-deployment-hash",
        "0x" + ETH_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR,
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
        raise AssertionError("unpinned ETH source TOML was accepted")

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
        raise AssertionError("hash-only ETH source TOML was accepted")

    runtime_bytecode = bytes.fromhex("6080604052")
    expected = eth_args(module)
    expected.source_bridge_emitter_code_hash = module.runtime_bytecode_hash(
        runtime_bytecode
    )
    expected_material_hash = module.eth_source_verifier_material_record_hash(expected)
    expected_deployment_hash = module.eth_source_adapter_engine_deployment_record_hash(
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
        "0x" + ETH_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
        "--deployment-receipt-hash",
        "0x" + "aa" * 32,
        "--deployment-transaction-hash",
        "0x" + "de" * 32,
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
    assert output["source_domain"] == 1
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
        == "0x" + ETH_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
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
    assert '# sccp_evm_source_rpc_chain_id = "1"' in rendered
    assert (
        '# sccp_evm_source_bridge_runtime_bytecode_hex = "0x'
        + runtime_bytecode.hex()
        + '"'
        in rendered
    )
    assert '# sccp_evm_source_deployment_transaction_hash = "0x' + "de" * 32 in rendered
    assert "# sccp_evm_source_deployment_transaction_block_hash" in rendered
    assert "# sccp_evm_source_deployment_transaction_input_sha256" in rendered
    assert '# sccp_evm_source_deployment_block_number = "4660"' in rendered
    assert "[[zk.sccp_source_verifier_materials]]" in rendered
    assert "[[zk.sccp_source_adapter_engine_deployments]]" in rendered


def test_eth_cli_rejects_expected_record_hash_mismatch():
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
        "0x" + ETH_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
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
        raise AssertionError("mismatched ETH expected material hash was accepted")


def test_eth_cli_rejects_expected_deployment_record_hash_mismatch():
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
        "0x" + ETH_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
        "--deployment-receipt-hash",
        "0x" + "aa" * 32,
        "--expected-source-verifier-material-hash",
        "0x" + ETH_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR,
        "--expected-source-adapter-engine-deployment-hash",
        "0x" + "99" * 32,
    ]

    try:
        module.main(args)
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("mismatched ETH expected deployment hash was accepted")


def test_eth_source_evidence_rejects_adapter_verifier_vk_hash_mismatch():
    module = load_evidence_module()
    args = SimpleNamespace(
        source_domain=1,
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
        assert "canonical ETH source-adapter verifier profile" in str(exc)
    else:
        raise AssertionError("mismatched ETH adapter verifier vk hash was accepted")


def test_eth_cli_rejects_non_production_lane():
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
        module.main(["--source-domain", "2", *args])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("non-ETH source evidence was accepted")

    try:
        module.main(["--target-domain", "1", *args])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("non-SORA target evidence was accepted")


def test_eth_cli_requires_code_hash_or_runtime_bytecode():
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
        raise AssertionError("ETH evidence without code hash was accepted")
