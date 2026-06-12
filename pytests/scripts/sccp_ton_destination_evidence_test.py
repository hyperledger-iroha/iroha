import base64
import json
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
from types import SimpleNamespace


TON_VERIFIER_CONTRACT_ADDRESS = "0:" + "11" * 32
TON_DESTINATION_BINDING_VECTOR = (
    "8651c1b818973f92050f69e66e8491e9681d23db1cb37393b9ea15c5e7e02799"
)
TON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR = (
    "08b11177113ac2d9f612abdf767a017de560d805e965b3dc32e28c8748ea2ebc"
)
TON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR = (
    "61e5d710ccbc902be00a38a5a80d05c19de97105605a3f93d4f8067862d81f07"
)
TON_ROUTE_ALLOWLIST_HASH_VECTOR = (
    "8b2e4cb6bf59ad66004085d8be2035a788611c0bfd5bcf60c3023b9f94ed9ed5"
)
TON_ROUTE_CANARY_EVIDENCE_HASH = (
    "e417bcb63179911639e30be2d18c3f3a4a6eb44a9d998c491d6f455f6ebe5d0a"
)
TON_CODE_BOC_HEX = "b5ee9c720101020100070001020101000202"
TON_CODE_BOC_BASE64 = base64.b64encode(bytes.fromhex(TON_CODE_BOC_HEX)).decode("ascii")
TON_CODE_BOC_CRC32C_HEX = "b5ee9c724101020100070001020101000202be1c1df5"
TON_CODE_BOC_ROOT_HASH = (
    "49725ad44ef5ed5feaa27f88679cabae427209a6bea318cb9b66030131aae6fe"
)


def load_evidence_module():
    script_path = (
        Path(__file__).resolve().parents[2]
        / "scripts"
        / "sccp_ton_destination_evidence.py"
    )
    spec = spec_from_file_location("sccp_ton_destination_evidence", script_path)
    module = module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def test_ton_destination_cli_redacts_top_level_exception_details(monkeypatch, capsys):
    module = load_evidence_module()

    def fail_apply(_args):
        raise ValueError("secret-token /tmp/operator/private-path")

    monkeypatch.setattr(module, "apply_verifier_code_boc_hash", fail_apply)

    try:
        module.main(
            [
                "--verifier-contract-address",
                TON_VERIFIER_CONTRACT_ADDRESS,
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("TON destination CLI accepted top-level render failure")

    captured = capsys.readouterr()
    assert "SCCP TON destination evidence rendering failed" in captured.err
    assert "secret-token" not in captured.err
    assert "private-path" not in captured.err


def test_ton_destination_redacts_verifier_address_parser_failures(monkeypatch):
    """Destination verifier address parser failures must not echo parser payloads."""

    module = load_evidence_module()
    args = ton_args(module)

    def fail_address(_value, *, label):
        raise module.argparse.ArgumentTypeError(
            f"secret-token {label} parser detail"
        )

    monkeypatch.setattr(module, "normalize_ton_raw_address", fail_address)
    try:
        module._require_destination_evidence(args)
    except ValueError as exc:
        rendered = str(exc)
        assert rendered == "verifier_contract_address metadata is invalid"
        assert "secret-token" not in rendered
        assert "parser detail" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("TON destination leaked verifier parser detail")


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


def ton_args(module):
    return SimpleNamespace(
        verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
        verifier_code_hash=bytes.fromhex(TON_CODE_BOC_ROOT_HASH),
        verifier_code_boc_hex=bytes.fromhex(TON_CODE_BOC_HEX),
        source_verifier_material_hash=bytes.fromhex(
            TON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
        ),
        source_adapter_engine_deployment_hash=bytes.fromhex(
            TON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR
        ),
        route_allowlist_hash=bytes.fromhex(TON_ROUTE_ALLOWLIST_HASH_VECTOR),
        route_canary_evidence_hash=bytes.fromhex(TON_ROUTE_CANARY_EVIDENCE_HASH),
        expected_destination_binding_hash=bytes.fromhex(TON_DESTINATION_BINDING_VECTOR),
        account_status="active",
        account_state_hash=bytes.fromhex("cc" * 32),
        last_transaction_lt="123456",
        last_transaction_hash=bytes.fromhex("66" * 32),
    )


def test_ton_hex_parser_rejects_zero_and_wrong_width():
    module = load_evidence_module()

    assert module.parse_hex_bytes(
        "0x" + "33" * 32,
        label="verifier code hash",
        byte_length=32,
    ) == bytes.fromhex("33" * 32)

    try:
        module.parse_hex_bytes(
            "0x" + "00" * 32,
            label="verifier code hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must not be zero" in str(exc)
    else:
        raise AssertionError("zero TON verifier code hash was accepted")

    try:
        module.parse_hex_bytes(
            " 0x" + "33" * 32 + " ",
            label="verifier code hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must not contain whitespace" in str(exc)
    else:
        raise AssertionError("padded TON verifier code hash was accepted")

    try:
        module.parse_hex_bytes(
            "0x" + "33" * 31,
            label="verifier code hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must be 32 bytes" in str(exc)
    else:
        raise AssertionError("short TON verifier code hash was accepted")


def test_ton_code_boc_inline_parsers_reject_padded_values(tmp_path):
    module = load_evidence_module()

    assert module.parse_code_boc_hex(TON_CODE_BOC_HEX, label="code BoC") == (
        bytes.fromhex(TON_CODE_BOC_HEX)
    )
    assert module.parse_code_boc_base64(TON_CODE_BOC_BASE64, label="code BoC") == (
        bytes.fromhex(TON_CODE_BOC_HEX)
    )

    for value, parser in (
        ("0x" + TON_CODE_BOC_HEX + "\n", module.parse_code_boc_hex),
        (" " + TON_CODE_BOC_BASE64, module.parse_code_boc_base64),
    ):
        try:
            parser(value, label="code BoC")
        except module.argparse.ArgumentTypeError as exc:
            assert "must not contain whitespace" in str(exc)
        else:
            raise AssertionError("padded TON code BoC inline value was accepted")

    try:
        module.parse_code_boc_base64(
            noncanonical_base64_alias(bytes.fromhex(TON_CODE_BOC_HEX) + b"\x01"),
            label="code BoC",
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "canonical base64" in str(exc)
    else:
        raise AssertionError("non-canonical TON code BoC base64 was accepted")

    code_boc_file = tmp_path / "code.boc.txt"
    code_boc_file.write_text("0x" + TON_CODE_BOC_HEX + "\n", encoding="ascii")
    assert module.parse_code_boc_file(str(code_boc_file), label="code BoC") == (
        bytes.fromhex(TON_CODE_BOC_HEX)
    )

    spaced_hex_file = tmp_path / "code-spaced-hex.boc.txt"
    spaced_hex_file.write_text(
        "0x" + TON_CODE_BOC_HEX[:8] + "\n" + TON_CODE_BOC_HEX[8:],
        encoding="ascii",
    )
    try:
        module.parse_code_boc_file(str(spaced_hex_file), label="code BoC")
    except module.argparse.ArgumentTypeError as exc:
        assert "must not contain whitespace" in str(exc)
    else:
        raise AssertionError("internally spaced TON code BoC hex file was accepted")

    spaced_base64_file = tmp_path / "code-spaced-base64.boc.txt"
    spaced_base64_file.write_text(
        TON_CODE_BOC_BASE64[:8] + "\n" + TON_CODE_BOC_BASE64[8:],
        encoding="ascii",
    )
    try:
        module.parse_code_boc_file(str(spaced_base64_file), label="code BoC")
    except module.argparse.ArgumentTypeError as exc:
        assert "must not contain whitespace" in str(exc)
    else:
        raise AssertionError("internally spaced TON code BoC base64 file was accepted")


def test_ton_raw_address_parser_rejects_zero_malformed_and_noncanonical():
    module = load_evidence_module()

    assert (
        module.normalize_ton_raw_address(
            TON_VERIFIER_CONTRACT_ADDRESS,
            label="verifier contract address",
        )
        == TON_VERIFIER_CONTRACT_ADDRESS
    )
    try:
        module.normalize_ton_raw_address(
            " " + TON_VERIFIER_CONTRACT_ADDRESS + " ",
            label="verifier contract address",
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must not contain whitespace" in str(exc)
    else:
        raise AssertionError("padded TON verifier contract address was accepted")

    try:
        module.normalize_ton_raw_address(
            "-1:" + "22" * 32,
            label="verifier contract address",
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "workchain must be basechain 0" in str(exc)
    else:
        raise AssertionError("masterchain TON verifier contract address was accepted")

    try:
        module.normalize_ton_raw_address(
            "0:" + "00" * 32,
            label="verifier contract address",
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "account must not be zero" in str(exc)
    else:
        raise AssertionError("zero TON verifier contract address was accepted")

    try:
        module.normalize_ton_raw_address(
            "00:" + "11" * 32,
            label="verifier contract address",
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "workchain must be canonical i32" in str(exc)
    else:
        raise AssertionError("noncanonical TON workchain was accepted")

    try:
        module.normalize_ton_raw_address(
            "\u0660:" + "11" * 32,
            label="verifier contract address",
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "workchain must be canonical i32" in str(exc)
    else:
        raise AssertionError("non-ASCII TON workchain was accepted")

    try:
        module.normalize_ton_raw_address(
            "0:" + "AA" * 32,
            label="verifier contract address",
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "lowercase canonical hex" in str(exc)
    else:
        raise AssertionError("uppercase TON account hex was accepted")

    try:
        module.normalize_ton_raw_address(
            "0:" + "11" * 31,
            label="verifier contract address",
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "account must be 32 bytes" in str(exc)
    else:
        raise AssertionError("short TON account hex was accepted")


def test_ton_last_transaction_lt_requires_canonical_ascii_decimal():
    module = load_evidence_module()

    assert (
        module.parse_positive_decimal_text("123456", label="last transaction LT")
        == "123456"
    )

    for value in ("0", "0123456", "+123456", " 123456 ", "١٢٣٤٥٦"):
        try:
            module.parse_positive_decimal_text(value, label="last transaction LT")
        except module.argparse.ArgumentTypeError as exc:
            assert "must be a positive decimal" in str(exc)
        else:
            raise AssertionError(f"noncanonical TON LT {value!r} was accepted")

        args = ton_args(module)
        args.last_transaction_lt = value
        try:
            module._require_toml_account_metadata(args, output="toml")
        except ValueError as exc:
            assert "--toml requires --last-transaction-lt" in str(exc)
        else:
            raise AssertionError(f"noncanonical TON LT metadata {value!r} was accepted")


def test_ton_destination_binding_hash_matches_rust_vector():
    module = load_evidence_module()

    assert module.ton_destination_binding_key() == "sccp:0:4:ton:ton-contract-v1:3"
    assert module.ton_destination_binding_hash().hex() == TON_DESTINATION_BINDING_VECTOR


def test_ton_code_boc_root_hash_matches_sdk_vectors():
    module = load_evidence_module()

    assert (
        module.ton_boc_single_root_hash(bytes.fromhex(TON_CODE_BOC_HEX)).hex()
        == TON_CODE_BOC_ROOT_HASH
    )
    assert (
        module.ton_boc_single_root_hash(bytes.fromhex(TON_CODE_BOC_CRC32C_HEX)).hex()
        == TON_CODE_BOC_ROOT_HASH
    )

    bad_crc = bytearray.fromhex(TON_CODE_BOC_CRC32C_HEX)
    bad_crc[-1] ^= 0x01
    try:
        module.ton_boc_single_root_hash(bytes(bad_crc))
    except ValueError as exc:
        assert "CRC32C" in str(exc)
    else:
        raise AssertionError("invalid TON code BoC CRC32C was accepted")


def test_ton_route_allowlist_hash_matches_lane_evidence_vector():
    module = load_evidence_module()

    assert (
        module.ton_route_allowlist_hash(
            source_verifier_material_hash=bytes.fromhex(
                TON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
            ),
            source_adapter_engine_deployment_hash=bytes.fromhex(
                TON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR
            ),
            destination_binding_hash=bytes.fromhex(TON_DESTINATION_BINDING_VECTOR),
        ).hex()
        == TON_ROUTE_ALLOWLIST_HASH_VECTOR
    )

    for replayed in (
        {
            "source_verifier_material_hash": bytes.fromhex(
                TON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR
            ),
            "source_adapter_engine_deployment_hash": bytes.fromhex(
                TON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR
            ),
            "destination_binding_hash": bytes.fromhex(TON_DESTINATION_BINDING_VECTOR),
        },
        {
            "source_verifier_material_hash": bytes.fromhex(
                TON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
            ),
            "source_adapter_engine_deployment_hash": bytes.fromhex(
                TON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR
            ),
            "destination_binding_hash": bytes.fromhex(
                TON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
            ),
        },
        {
            "source_verifier_material_hash": bytes.fromhex(
                TON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
            ),
            "source_adapter_engine_deployment_hash": bytes.fromhex(
                TON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR
            ),
            "destination_binding_hash": bytes.fromhex(
                TON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR
            ),
        },
    ):
        try:
            module.ton_route_allowlist_hash(**replayed)
        except ValueError as exc:
            assert "TON route allowlist evidence hashes must be distinct" in str(exc)
        else:
            raise AssertionError("TON route allowlist accepted replayed hash role")


def test_ton_route_canary_rejects_live_account_hash_role_reuse():
    module = load_evidence_module()
    args = ton_args(module)
    args.last_transaction_hash = args.account_state_hash

    try:
        module.ton_route_canary_evidence_hash(
            route_allowlist_hash=args.route_allowlist_hash,
            destination_binding_hash=args.expected_destination_binding_hash,
            source_verifier_material_hash=args.source_verifier_material_hash,
            source_adapter_engine_deployment_hash=(
                args.source_adapter_engine_deployment_hash
            ),
            verifier_contract_address=args.verifier_contract_address,
            verifier_code_hash=args.verifier_code_hash,
            account_status=args.account_status,
            account_state_hash=args.account_state_hash,
            last_transaction_lt=args.last_transaction_lt,
            last_transaction_hash=args.last_transaction_hash,
            verifier_code_boc_root_hash=args.verifier_code_hash,
        )
    except ValueError as exc:
        assert "last_transaction_hash must differ from account_state_hash" in str(exc)
    else:
        raise AssertionError("TON route canary accepted reused live account hash role")

    try:
        module.render_toml(args)
    except ValueError as exc:
        assert "last_transaction_hash must differ from account_state_hash" in str(exc)
    else:
        raise AssertionError("TON destination TOML accepted reused canary hash role")

    base_args = ton_args(module)
    for field, source_field in (
        ("route_allowlist_hash", "expected_destination_binding_hash"),
        ("route_allowlist_hash", "source_verifier_material_hash"),
        ("route_allowlist_hash", "source_adapter_engine_deployment_hash"),
        ("expected_destination_binding_hash", "source_verifier_material_hash"),
        ("expected_destination_binding_hash", "source_adapter_engine_deployment_hash"),
    ):
        replay_args = ton_args(module)
        setattr(replay_args, field, getattr(base_args, source_field))
        try:
            module.ton_route_canary_evidence_hash(
                route_allowlist_hash=replay_args.route_allowlist_hash,
                destination_binding_hash=replay_args.expected_destination_binding_hash,
                source_verifier_material_hash=replay_args.source_verifier_material_hash,
                source_adapter_engine_deployment_hash=(
                    replay_args.source_adapter_engine_deployment_hash
                ),
                verifier_contract_address=replay_args.verifier_contract_address,
                verifier_code_hash=replay_args.verifier_code_hash,
                account_status=replay_args.account_status,
                account_state_hash=replay_args.account_state_hash,
                last_transaction_lt=replay_args.last_transaction_lt,
                last_transaction_hash=replay_args.last_transaction_hash,
                verifier_code_boc_root_hash=replay_args.verifier_code_hash,
            )
        except ValueError as exc:
            assert "TON route canary governed hashes must be distinct" in str(exc)
        else:
            raise AssertionError(f"TON route canary accepted governed replay of {field}")


def test_ton_cli_derives_verifier_code_hash_from_code_boc(capsys, tmp_path):
    module = load_evidence_module()
    code_boc = bytes.fromhex(TON_CODE_BOC_CRC32C_HEX)
    code_boc_path = tmp_path / "verifier-code.boc"
    code_boc_path.write_bytes(code_boc)
    common_args = [
        "--verifier-contract-address",
        TON_VERIFIER_CONTRACT_ADDRESS,
        "--account-status",
        "active",
        "--account-state-hash",
        "0x" + "cc" * 32,
        "--last-transaction-lt",
        "123456",
        "--last-transaction-hash",
        "0x" + "66" * 32,
        "--route-allowlist-hash",
        "0x" + TON_ROUTE_ALLOWLIST_HASH_VECTOR,
        "--source-verifier-material-hash",
        "0x" + TON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR,
        "--source-adapter-engine-deployment-hash",
        "0x" + TON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR,
        "--expected-destination-binding-hash",
        "0x" + TON_DESTINATION_BINDING_VECTOR,
        "--route-canary-evidence-hash",
        "0x" + TON_ROUTE_CANARY_EVIDENCE_HASH,
    ]

    assert module.main([*common_args, "--verifier-code-boc-file", str(code_boc_path)]) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["verifier_code_hash"] == "0x" + TON_CODE_BOC_ROOT_HASH
    assert output["toml_ready"] is True

    assert module.main([*common_args, "--verifier-code-boc-hex", TON_CODE_BOC_HEX]) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["verifier_code_hash"] == "0x" + TON_CODE_BOC_ROOT_HASH

    encoded_boc = base64.b64encode(bytes.fromhex(TON_CODE_BOC_HEX)).decode("ascii")
    assert (
        module.main([*common_args, "--verifier-code-boc-base64", encoded_boc])
        == 0
    )
    output = json.loads(capsys.readouterr().out)
    assert output["verifier_code_hash"] == "0x" + TON_CODE_BOC_ROOT_HASH

    try:
        module.main(
            [
                *common_args,
                "--verifier-code-boc-hex",
                TON_CODE_BOC_HEX,
                "--verifier-code-hash",
                "0x" + "bb" * 32,
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("mismatched TON code BoC hash was accepted")


def test_ton_toml_code_boc_base64_reparse_redacts_parser_detail():
    module = load_evidence_module()
    args = SimpleNamespace(
        verifier_code_hash=bytes.fromhex(TON_CODE_BOC_ROOT_HASH),
        verifier_code_boc_root_hash=bytes.fromhex(TON_CODE_BOC_ROOT_HASH),
        verifier_code_boc_hash_matches=True,
        verifier_code_boc_base64_text="secret-token-ton-code-boc",
    )

    try:
        module._require_code_boc_root_metadata(args, output="toml")
    except ValueError as exc:
        rendered = str(exc)
        assert rendered == "--toml has invalid verifier code BoC base64 evidence"
        assert "secret-token" not in rendered
        assert "must be base64" not in rendered
        assert "canonical base64" not in rendered
        assert exc.__cause__ is None
    else:
        raise AssertionError("invalid copied TON code BoC base64 evidence was accepted")


def test_ton_direct_renderers_derive_verifier_code_hash_from_code_boc():
    module = load_evidence_module()
    args = ton_args(module)
    args.verifier_code_hash = None
    args.verifier_code_boc_hex = bytes.fromhex(TON_CODE_BOC_HEX)

    rendered = module.render_toml(args)
    assert 'verifier_code_hash = "0x' + TON_CODE_BOC_ROOT_HASH + '"' in rendered
    assert '# sccp_ton_code_boc_base64 = "' + TON_CODE_BOC_BASE64 + '"' in rendered
    assert args.verifier_code_hash == bytes.fromhex(TON_CODE_BOC_ROOT_HASH)

    summary_args = ton_args(module)
    summary_args.verifier_code_hash = None
    summary_args.verifier_code_boc_hex = None
    summary_args.verifier_code_boc_base64 = bytes.fromhex(TON_CODE_BOC_HEX)
    summary = module._json_summary(
        summary_args,
        bytes.fromhex(TON_DESTINATION_BINDING_VECTOR),
        True,
    )
    assert summary["verifier_code_hash"] == "0x" + TON_CODE_BOC_ROOT_HASH
    assert summary["code_boc_root_hash"] == "0x" + TON_CODE_BOC_ROOT_HASH
    assert summary["code_boc_base64"] == TON_CODE_BOC_BASE64
    assert summary["code_boc_hash_matches"] is True


def test_ton_direct_toml_requires_code_boc_root_evidence():
    module = load_evidence_module()
    args = ton_args(module)
    args.verifier_code_boc_hex = None

    try:
        module.render_toml(args)
    except ValueError as exc:
        assert "requires verifier code BoC root evidence" in str(exc)
    else:
        raise AssertionError("hash-only TON production TOML was accepted")

    try:
        module._json_summary(
            args,
            bytes.fromhex(TON_DESTINATION_BINDING_VECTOR),
            True,
        )
    except ValueError as exc:
        assert "verifier_code_boc_root_hash" in str(exc)
    else:
        raise AssertionError("hash-only TON route canary summary was accepted")


def test_ton_toml_rendering_carries_destination_profile_ids():
    module = load_evidence_module()
    rendered = module.render_toml(ton_args(module))

    assert (
        '# sccp_ton_destination_binding_hash = "0x'
        + TON_DESTINATION_BINDING_VECTOR
        + '"'
        in rendered
    )
    assert (
        '# sccp_ton_route_allowlist_hash = "0x'
        + TON_ROUTE_ALLOWLIST_HASH_VECTOR
        + '"'
        in rendered
    )
    assert '# sccp_ton_account_status = "active"' in rendered
    assert '# sccp_ton_account_state_hash = "0x' + "cc" * 32 + '"' in rendered
    assert '# sccp_ton_last_transaction_lt = "123456"' in rendered
    assert '# sccp_ton_last_transaction_hash = "0x' + "66" * 32 + '"' in rendered
    assert '# sccp_ton_code_hash = "0x' + TON_CODE_BOC_ROOT_HASH + '"' in rendered
    assert (
        '# sccp_ton_code_boc_root_hash = "0x'
        + TON_CODE_BOC_ROOT_HASH
        + '"'
        in rendered
    )
    assert '# sccp_ton_code_boc_base64 = "' + TON_CODE_BOC_BASE64 + '"' in rendered
    assert '# sccp_ton_code_boc_hash_matches = "true"' in rendered
    assert 'destination_binding_key = "sccp:0:4:ton:ton-contract-v1:3"' in rendered
    assert (
        'destination_binding_hash = "0x'
        + TON_DESTINATION_BINDING_VECTOR
        + '"'
        in rendered
    )
    assert "[[zk.sccp_destination_rollouts]]" in rendered
    assert "[[zk.sccp_route_allowlists]]" in rendered
    assert "domain = 4" in rendered
    assert 'chain = "ton"' in rendered
    assert 'verifier_plan = "TonContractNativeRecursive"' in rendered
    assert f'verifier_identity = "{TON_VERIFIER_CONTRACT_ADDRESS}"' in rendered
    assert 'verifier_code_hash = "0x' + TON_CODE_BOC_ROOT_HASH + '"' in rendered
    assert 'ton_account_status = "active"' in rendered
    assert 'ton_account_state_hash = "0x' + "cc" * 32 + '"' in rendered
    assert 'ton_last_transaction_lt = "123456"' in rendered
    assert 'ton_last_transaction_hash = "0x' + "66" * 32 + '"' in rendered
    assert (
        'ton_verifier_code_boc_root_hash = "0x'
        + TON_CODE_BOC_ROOT_HASH
        + '"'
        in rendered
    )
    assert 'ton_verifier_code_boc = "0x' + TON_CODE_BOC_HEX + '"' in rendered
    assert 'anchor_id = "sccp:ton:destination-anchor:ton-mainnet:v1"' in rendered
    assert (
        'route_allowlist_id = "sccp:ton:route-allowlist:ton-mainnet:v1"'
        in rendered
    )
    assert (
        'route_allowlist_hash = "0x' + TON_ROUTE_ALLOWLIST_HASH_VECTOR + '"'
        in rendered
    )
    assert '# sccp_route_canary_status = "passed"' in rendered
    assert 'route_canary_status = "passed"' in rendered
    assert (
        '# sccp_route_canary_evidence_hash = "0x'
        + TON_ROUTE_CANARY_EVIDENCE_HASH
        + '"'
        in rendered
    )
    assert (
        'route_canary_evidence_hash = "0x'
        + TON_ROUTE_CANARY_EVIDENCE_HASH
        + '"'
        in rendered
    )
    assert "blockers = []" in rendered

    try:
        module.render_toml(
            ton_args(module),
            destination_binding_hash=bytes.fromhex("ee" * 32),
        )
    except ValueError as exc:
        assert "canonical SORA -> TON binding" in str(exc)
    else:
        raise AssertionError("mismatched direct TON destination binding hash was accepted")

    try:
        module._json_summary(ton_args(module), bytes.fromhex("ee" * 32), False)
    except ValueError as exc:
        assert "canonical SORA -> TON binding" in str(exc)
    else:
        raise AssertionError("mismatched direct TON JSON binding hash was accepted")

    bad_code_args = ton_args(module)
    bad_code_args.verifier_code_boc_hex = None
    bad_code_args.verifier_code_hash = bytes(32)
    try:
        module.render_toml(bad_code_args)
    except ValueError as exc:
        assert "verifier_code_hash must not be zero" in str(exc)
    else:
        raise AssertionError("zero direct TON verifier code hash was accepted")

    try:
        module._json_summary(
            bad_code_args,
            bytes.fromhex(TON_DESTINATION_BINDING_VECTOR),
            False,
        )
    except ValueError as exc:
        assert "verifier_code_hash must not be zero" in str(exc)
    else:
        raise AssertionError("zero direct TON JSON verifier code hash was accepted")

    bad_allowlist_args = ton_args(module)
    bad_allowlist_args.route_allowlist_hash = bytes(32)
    try:
        module.render_toml(bad_allowlist_args)
    except ValueError as exc:
        assert "route_allowlist_hash must not be zero" in str(exc)
    else:
        raise AssertionError("zero direct TON route allowlist hash was accepted")

    try:
        module._json_summary(
            bad_allowlist_args,
            bytes.fromhex(TON_DESTINATION_BINDING_VECTOR),
            False,
        )
    except ValueError as exc:
        assert "route_allowlist_hash must not be zero" in str(exc)
    else:
        raise AssertionError("zero direct TON JSON route allowlist hash was accepted")

    drifted_allowlist_args = ton_args(module)
    drifted_allowlist_args.route_allowlist_hash = bytes.fromhex("dd" * 32)
    try:
        module.render_toml(drifted_allowlist_args)
    except ValueError as exc:
        assert "canonical source, deployment, and destination evidence" in str(exc)
    else:
        raise AssertionError("drifted direct TON route allowlist hash was accepted")

    try:
        module._json_summary(
            drifted_allowlist_args,
            bytes.fromhex(TON_DESTINATION_BINDING_VECTOR),
            False,
        )
    except ValueError as exc:
        assert "canonical source, deployment, and destination evidence" in str(exc)
    else:
        raise AssertionError("drifted direct TON JSON route hash was accepted")

    missing_canary_args = ton_args(module)
    missing_canary_args.route_canary_evidence_hash = None
    try:
        module.render_toml(missing_canary_args)
    except ValueError as exc:
        assert "--route-canary-evidence-hash" in str(exc)
    else:
        raise AssertionError("TON destination TOML accepted without route canary evidence")

    missing_canary_summary = module._json_summary(
        missing_canary_args,
        bytes.fromhex(TON_DESTINATION_BINDING_VECTOR),
        True,
    )
    assert missing_canary_summary["toml_ready"] is False
    assert "route_canary" not in missing_canary_summary

    for account_status in (None, "uninit"):
        bad_status_args = ton_args(module)
        bad_status_args.account_status = account_status
        try:
            module._json_summary(
                bad_status_args,
                bytes.fromhex(TON_DESTINATION_BINDING_VECTOR),
                True,
            )
        except ValueError as exc:
            assert "account_status must be active" in str(exc)
        else:
            raise AssertionError(
                "TON destination JSON accepted route canary without active account status"
            )

    for attr_name, label in (
        ("source_verifier_material_hash", "source_verifier_material_hash"),
        (
            "source_adapter_engine_deployment_hash",
            "source_adapter_engine_deployment_hash",
        ),
    ):
        replay_args = ton_args(module)
        replay_args.route_canary_evidence_hash = getattr(replay_args, attr_name)
        try:
            module.render_toml(replay_args)
        except ValueError as exc:
            assert label in str(exc)
        else:
            raise AssertionError(
                f"TON destination TOML accepted route canary replay of {label}"
            )

        try:
            module._json_summary(
                replay_args,
                bytes.fromhex(TON_DESTINATION_BINDING_VECTOR),
                True,
            )
        except ValueError as exc:
            assert label in str(exc)
        else:
            raise AssertionError(
                f"TON destination JSON accepted route canary replay of {label}"
            )


def test_ton_cli_json_summary_and_toml_output(capsys):
    module = load_evidence_module()
    args = [
        "--verifier-contract-address",
        TON_VERIFIER_CONTRACT_ADDRESS,
        "--verifier-code-hash",
        "0x" + TON_CODE_BOC_ROOT_HASH,
        "--verifier-code-boc-hex",
        TON_CODE_BOC_HEX,
        "--account-status",
        "active",
        "--account-state-hash",
        "0x" + "cc" * 32,
        "--last-transaction-lt",
        "123456",
        "--last-transaction-hash",
        "0x" + "66" * 32,
        "--route-allowlist-hash",
        "0x" + TON_ROUTE_ALLOWLIST_HASH_VECTOR,
        "--source-verifier-material-hash",
        "0x" + TON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR,
        "--source-adapter-engine-deployment-hash",
        "0x" + TON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR,
    ]
    binding_only_args = args[:6]
    missing_status_args = [
        value
        for index, value in enumerate(args)
        if args[index - 1] != "--account-status" and value != "--account-status"
    ]
    full_args_without_canary = [
        *args,
        "--expected-destination-binding-hash",
        "0x" + TON_DESTINATION_BINDING_VECTOR,
    ]
    full_args = [
        *full_args_without_canary,
        "--route-canary-evidence-hash",
        "0x" + TON_ROUTE_CANARY_EVIDENCE_HASH,
    ]

    assert module.main(binding_only_args) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["domain"] == 4
    assert output["chain"] == "ton"
    assert output["verifier_plan"] == "TonContractNativeRecursive"
    assert output["verifier_identity"] == TON_VERIFIER_CONTRACT_ADDRESS
    assert output["verifier_code_hash"] == "0x" + TON_CODE_BOC_ROOT_HASH
    assert output["code_boc_root_hash"] == "0x" + TON_CODE_BOC_ROOT_HASH
    assert output["code_boc_base64"] == TON_CODE_BOC_BASE64
    assert output["code_boc_hash_matches"] is True
    assert output["destination_binding_key"] == "sccp:0:4:ton:ton-contract-v1:3"
    assert output["destination_binding_hash"] == "0x" + TON_DESTINATION_BINDING_VECTOR
    assert output["expected_destination_binding_hash_matches"] is False
    assert output["toml_ready"] is False
    assert "route_allowlist_hash" not in output

    try:
        module.main(args)
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("unpinned TON route allowlist hash was accepted")

    try:
        module.main(
            [
                *binding_only_args,
                "--expected-destination-binding-hash",
                "0x" + TON_DESTINATION_BINDING_VECTOR,
                "--route-allowlist-hash",
                "0x" + TON_ROUTE_ALLOWLIST_HASH_VECTOR,
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("partial TON route allowlist evidence was accepted")

    assert module.main(full_args_without_canary) == 0
    no_canary = json.loads(capsys.readouterr().out)
    assert no_canary["expected_destination_binding_hash_matches"] is True
    assert no_canary["expected_route_allowlist_hash_matches"] is True
    assert no_canary["toml_ready"] is False
    assert "route_canary" not in no_canary

    try:
        module.main(
            [
                *missing_status_args,
                "--expected-destination-binding-hash",
                "0x" + TON_DESTINATION_BINDING_VECTOR,
                "--route-canary-evidence-hash",
                "0x" + TON_ROUTE_CANARY_EVIDENCE_HASH,
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("TON route canary JSON accepted missing active account status")
    assert capsys.readouterr().out == ""

    try:
        module.main(
            [
                *missing_status_args,
                "--expected-destination-binding-hash",
                "0x" + TON_DESTINATION_BINDING_VECTOR,
                "--route-canary-evidence-hash",
                "0x" + TON_ROUTE_CANARY_EVIDENCE_HASH,
                "--toml",
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("TON destination TOML rendered without active account status evidence")

    try:
        module.main([*full_args_without_canary, "--toml"])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("TON destination TOML rendered without route canary evidence")

    assert module.main(full_args) == 0
    matched = json.loads(capsys.readouterr().out)
    assert matched["expected_destination_binding_hash_matches"] is True
    assert matched["toml_ready"] is True
    assert matched["route_allowlist_hash"] == "0x" + TON_ROUTE_ALLOWLIST_HASH_VECTOR
    assert matched["expected_route_allowlist_hash"] == (
        "0x" + TON_ROUTE_ALLOWLIST_HASH_VECTOR
    )
    assert matched["expected_route_allowlist_hash_matches"] is True
    assert matched["route_canary"]["status"] == "passed"
    assert matched["route_canary"]["evidence_hash"] == (
        "0x" + TON_ROUTE_CANARY_EVIDENCE_HASH
    )

    try:
        module.main([*args, "--toml"])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("unpinned TON destination TOML was accepted")

    assert module.main([*full_args, "--toml"]) == 0
    rendered = capsys.readouterr().out
    assert '# sccp_ton_account_status = "active"' in rendered
    assert '# sccp_ton_account_state_hash = "0x' + "cc" * 32 in rendered
    assert '# sccp_ton_last_transaction_lt = "123456"' in rendered
    assert '# sccp_ton_code_hash = "0x' + TON_CODE_BOC_ROOT_HASH in rendered
    assert '# sccp_ton_code_boc_root_hash = "0x' + TON_CODE_BOC_ROOT_HASH in rendered
    assert '# sccp_ton_code_boc_base64 = "' + TON_CODE_BOC_BASE64 + '"' in rendered
    assert '# sccp_ton_code_boc_hash_matches = "true"' in rendered
    assert "[[zk.sccp_destination_rollouts]]" in rendered
    assert "[[zk.sccp_route_allowlists]]" in rendered

    try:
        module.main([*args, "--expected-destination-binding-hash", "0x" + "ee" * 32])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("mismatched TON destination binding hash was accepted")

    bad_route_args = [
        value if value != "0x" + TON_ROUTE_ALLOWLIST_HASH_VECTOR else "0x" + "dd" * 32
        for value in full_args
    ]
    try:
        module.main(bad_route_args)
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("mismatched TON route allowlist hash was accepted")
