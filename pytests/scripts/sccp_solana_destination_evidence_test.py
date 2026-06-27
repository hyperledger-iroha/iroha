import base64
import hashlib
import json
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
from types import SimpleNamespace


SOLANA_VERIFIER_PROGRAM_ID = "So11111111111111111111111111111111111111112"
SOLANA_PROGRAMDATA_ADDRESS = "29d2S7vB453rNYFdR5Ycwt7y9haRT5fwVwL9zTmBhfV2"
SOLANA_VERIFIER_PROGRAM_BYTES = b"\x7fELF\x02\x01solana-sccp-verifier"
SOLANA_VERIFIER_PROGRAM_BYTES_BASE64 = base64.b64encode(
    SOLANA_VERIFIER_PROGRAM_BYTES
).decode("ascii")
SOLANA_DESTINATION_BINDING_VECTOR = (
    "078578f0aa27daa2972d6c19d1d26dbb6bf6ba1e8df84e283d7ef101fc46abf6"
)
SOURCE_VERIFIER_MATERIAL_HASH = "aa" * 32
SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH = "99" * 32
SOLANA_ROUTE_ALLOWLIST_HASH_VECTOR = (
    "c23e048cdfabc169c3567c201f31869efa4dbcac6478f6f80b31bfe410c64a34"
)


def load_evidence_module():
    script_path = (
        Path(__file__).resolve().parents[2]
        / "scripts"
        / "sccp_solana_destination_evidence.py"
    )
    spec = spec_from_file_location("sccp_solana_destination_evidence", script_path)
    module = module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def test_solana_destination_cli_redacts_top_level_exception_details(monkeypatch, capsys):
    module = load_evidence_module()

    for exception_type in (OSError, RuntimeError, TypeError, ValueError):

        def fail_apply(_args, exception_type=exception_type):
            raise exception_type("secret-token /tmp/operator/private-path")

        with monkeypatch.context() as patch:
            patch.setattr(module, "apply_verifier_program_code_hash", fail_apply)
            try:
                module.main(["--verifier-program-id", SOLANA_VERIFIER_PROGRAM_ID])
            except SystemExit as exc:
                assert exc.code == 2
            else:
                raise AssertionError(
                    "Solana destination CLI accepted top-level render failure"
                )

            captured = capsys.readouterr()
            assert "SCCP Solana destination evidence rendering failed" in captured.err
            assert "secret-token" not in captured.err
            assert "private-path" not in captured.err
            assert exception_type.__name__ not in captured.err


def test_solana_destination_redacts_verifier_program_parser_failures(monkeypatch):
    """Destination verifier program parser failures must not echo parser payloads."""

    module = load_evidence_module()
    args = solana_args(module)

    failure_cases = (
        (
            module.argparse.ArgumentTypeError,
            "secret-token {label} parser detail",
            "parser detail",
        ),
        (
            SystemExit,
            "secret-token {label} helper SystemExit detail",
            "helper SystemExit detail",
        ),
        (
            RuntimeError,
            "secret-token {label} helper RuntimeError detail",
            "helper RuntimeError detail",
        ),
        (
            TypeError,
            "secret-token {label} helper TypeError detail",
            "helper TypeError detail",
        ),
        (
            ValueError,
            "secret-token {label} helper ValueError detail",
            "helper ValueError detail",
        ),
    )
    for exception_type, secret_template, forbidden_detail in failure_cases:
        with monkeypatch.context() as patch:

            def fail_program_id(
                _value,
                *,
                label,
                exception_type=exception_type,
                secret_template=secret_template,
            ):
                raise exception_type(secret_template.format(label=label))

            patch.setattr(module, "normalize_solana_program_id", fail_program_id)
            try:
                module._require_destination_evidence(args)
            except ValueError as exc:
                rendered = str(exc)
                assert rendered == "verifier_program_id metadata is invalid"
                assert "secret-token" not in rendered
                assert forbidden_detail not in rendered
                assert exc.__cause__ is None
                assert exc.__suppress_context__ is True
            else:
                raise AssertionError(
                    f"Solana destination leaked {exception_type.__name__} detail"
                )


def test_solana_destination_hex_parsers_redact_parser_causes():
    """Invalid Solana destination hex inputs must not chain parser payloads."""

    module = load_evidence_module()
    fixed_payload = "secret-token-solana-fixed-hex"
    program_payload = "secret-token-solana-program-hex"

    try:
        module.parse_hex_bytes(
            "0x" + fixed_payload + ("a" * (64 - len(fixed_payload))),
            label="verifier code hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        rendered = str(exc)
        assert rendered == "verifier code hash must be hex"
        assert "secret-token" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("invalid Solana destination fixed hex was accepted")

    try:
        module.parse_program_bytes_hex(
            "0x" + program_payload + ("a" * (64 - len(program_payload))),
            label="verifier program bytes",
        )
    except module.argparse.ArgumentTypeError as exc:
        rendered = str(exc)
        assert rendered == "verifier program bytes must be hex"
        assert "secret-token" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("invalid Solana destination program hex was accepted")


def test_solana_destination_hex_parsers_redact_helper_exit_parser_causes(monkeypatch):
    module = load_evidence_module()

    for exception_type in (SystemExit, RuntimeError, TypeError, ValueError):
        detail = (
            "secret-token Solana destination hex TypeError detail"
            if exception_type is TypeError
            else f"secret-token Solana destination hex {exception_type.__name__} detail"
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
                    "verifier code hash",
                    {"byte_length": 32},
                ),
                (
                    module.parse_program_bytes_hex,
                    "0x" + SOLANA_VERIFIER_PROGRAM_BYTES.hex(),
                    "verifier program bytes",
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
                    assert exc.__cause__ is None
                    assert exc.__suppress_context__ is True
                else:
                    raise AssertionError(
                        f"{label} parser {exception_type.__name__} was accepted"
                    )


def test_solana_destination_base64_parser_redacts_parser_causes(monkeypatch):
    """Invalid Solana destination base64 inputs must not chain parser payloads."""

    module = load_evidence_module()

    try:
        module.parse_program_bytes_base64(
            "secret-token-solana-destination-base64",
            label="verifier program bytes",
        )
    except module.argparse.ArgumentTypeError as exc:
        rendered = str(exc)
        assert rendered == "verifier program bytes must be base64"
        assert "secret-token" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("invalid Solana destination program base64 was accepted")

    for exception_type in (SystemExit, RuntimeError, TypeError, ValueError):

        def fail_b64decode(_value, *, validate, exception_type=exception_type):
            del validate
            raise exception_type(
                "secret-token Solana destination base64 decoder detail"
            )

        with monkeypatch.context() as patch:
            patch.setattr(module.base64, "b64decode", fail_b64decode)
            try:
                module.parse_program_bytes_base64(
                    SOLANA_VERIFIER_PROGRAM_BYTES_BASE64,
                    label="verifier program bytes",
                )
            except module.argparse.ArgumentTypeError as exc:
                rendered = str(exc)
                assert rendered == "verifier program bytes must be base64"
                assert "secret-token" not in rendered
                assert "decoder detail" not in rendered
                assert exception_type.__name__ not in rendered
                assert exc.__cause__ is None
                assert exc.__suppress_context__ is True
            else:
                raise AssertionError(
                    "Solana destination base64 "
                    f"{exception_type.__name__} decoder detail was accepted"
                )


def test_solana_destination_file_parser_redacts_file_read_causes(tmp_path):
    """Unreadable Solana destination program paths must not chain path details."""

    module = load_evidence_module()
    private_path = tmp_path / "secret-token-private-verifier.so"

    try:
        module.parse_program_bytes_file(
            str(private_path),
            label="verifier program bytes",
        )
    except module.argparse.ArgumentTypeError as exc:
        rendered = str(exc)
        assert rendered == "verifier program bytes file cannot be read"
        assert "secret-token" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("missing Solana destination program file was accepted")


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


def solana_route_canary_evidence_hash(
    module,
    *,
    program_bytes=SOLANA_VERIFIER_PROGRAM_BYTES,
    programdata_address=SOLANA_PROGRAMDATA_ADDRESS,
    programdata_slot=4321,
    program_account_context_slot=9000,
    programdata_account_context_slot=9000,
):
    verifier_code_hash = module.solana_verifier_program_code_hash(program_bytes)
    return module.solana_route_canary_evidence_hash(
        route_allowlist_hash=bytes.fromhex(SOLANA_ROUTE_ALLOWLIST_HASH_VECTOR),
        destination_binding_hash=bytes.fromhex(SOLANA_DESTINATION_BINDING_VECTOR),
        source_verifier_material_hash=bytes.fromhex(SOURCE_VERIFIER_MATERIAL_HASH),
        source_adapter_engine_deployment_hash=bytes.fromhex(
            SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
        ),
        verifier_program_id=SOLANA_VERIFIER_PROGRAM_ID,
        verifier_code_hash=verifier_code_hash,
        rpc_commitment="finalized",
        program_owner=module.SOLANA_UPGRADEABLE_LOADER_ID,
        programdata_owner=module.SOLANA_UPGRADEABLE_LOADER_ID,
        program_immutable=True,
        program_account_data=module.solana_upgradeable_program_account_data(
            programdata_address
        ),
        programdata_address=programdata_address,
        programdata_slot=programdata_slot,
        expected_programdata_slot=programdata_slot,
        program_account_context_slot=program_account_context_slot,
        programdata_account_context_slot=programdata_account_context_slot,
        programdata_metadata=module.solana_immutable_programdata_metadata(
            programdata_slot
        ),
        programdata_executable=program_bytes,
    ).hex()


def solana_args(module):
    return SimpleNamespace(
        verifier_program_id=SOLANA_VERIFIER_PROGRAM_ID,
        verifier_code_hash=bytes.fromhex("bb" * 32),
        source_verifier_material_hash=bytes.fromhex(SOURCE_VERIFIER_MATERIAL_HASH),
        source_adapter_engine_deployment_hash=bytes.fromhex(
            SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
        ),
        route_allowlist_hash=bytes.fromhex(SOLANA_ROUTE_ALLOWLIST_HASH_VECTOR),
        route_canary_evidence_hash=bytes.fromhex(
            solana_route_canary_evidence_hash(module)
        ),
        expected_destination_binding_hash=bytes.fromhex(SOLANA_DESTINATION_BINDING_VECTOR),
        programdata_address=SOLANA_PROGRAMDATA_ADDRESS,
        programdata_slot=4321,
        program_account_context_slot=9000,
        programdata_account_context_slot=9000,
    )


def solana_toml_args(module):
    args = solana_args(module)
    args.verifier_code_hash = module.solana_verifier_program_code_hash(
        SOLANA_VERIFIER_PROGRAM_BYTES
    )
    args.verifier_program_bytes_base64 = SOLANA_VERIFIER_PROGRAM_BYTES
    return args


def test_solana_hex_parser_rejects_zero_and_wrong_width():
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
        raise AssertionError("zero Solana verifier code hash was accepted")

    try:
        module.parse_hex_bytes(
            " 0x" + "33" * 32 + " ",
            label="verifier code hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must not contain whitespace" in str(exc)
    else:
        raise AssertionError("padded Solana verifier code hash was accepted")

    for value, expected in (
        ("33" * 32, "canonical lowercase 0x hex"),
        ("0X" + "33" * 32, "lowercase 0x prefix"),
        ("0x" + "AA" * 32, "lowercase hex"),
    ):
        try:
            module.parse_hex_bytes(
                value,
                label="verifier code hash",
                byte_length=32,
            )
        except module.argparse.ArgumentTypeError as exc:
            assert expected in str(exc)
        else:
            raise AssertionError(
                f"noncanonical Solana verifier hash {value!r} was accepted"
            )

    try:
        module.parse_hex_bytes(
            "0x" + "33" * 31,
            label="verifier code hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must be 32 bytes" in str(exc)
    else:
        raise AssertionError("short Solana verifier code hash was accepted")


def test_solana_positive_u64_parser_rejects_noncanonical_decimal_text():
    module = load_evidence_module()

    assert module.parse_positive_u64("4321", label="programdata slot") == 4321

    for value in ("0", "04321", "+4321", " 4321 ", "٤٣٢١"):
        try:
            module.parse_positive_u64(value, label="programdata slot")
        except module.argparse.ArgumentTypeError as exc:
            assert "must be a positive u64" in str(exc)
        else:
            raise AssertionError(f"noncanonical Solana slot {value!r} was accepted")


def test_solana_program_bytes_parsers_reject_empty_zero_malformed_and_non_elf(tmp_path):
    module = load_evidence_module()

    assert module.parse_program_bytes_base64(
        base64.b64encode(b"\x7fELFsol").decode("ascii"),
        label="verifier program bytes",
    ) == b"\x7fELFsol"
    assert module.parse_program_bytes_hex(
        "0x" + b"\x7fELFsol".hex(),
        label="verifier program bytes",
    ) == b"\x7fELFsol"
    for value, expected in (
        (b"\x7fELFsol".hex(), "canonical lowercase 0x hex"),
        ("0X" + b"\x7fELFsol".hex(), "lowercase 0x prefix"),
        ("0x" + b"\x7fELFsol".hex().upper(), "lowercase hex"),
    ):
        try:
            module.parse_program_bytes_hex(value, label="verifier program bytes")
        except module.argparse.ArgumentTypeError as exc:
            assert expected in str(exc)
        else:
            raise AssertionError(
                f"noncanonical Solana verifier program hex {value!r} was accepted"
            )
    program_file = tmp_path / "verifier.so"
    program_file.write_bytes(b"\x7fELFsol")
    assert module.parse_program_bytes_file(
        str(program_file),
        label="verifier program bytes",
    ) == b"\x7fELFsol"

    for value, parser in (
        (
            " " + base64.b64encode(b"\x7fELFsol").decode("ascii"),
            module.parse_program_bytes_base64,
        ),
        ("0x" + b"\x7fELFsol".hex() + "\n", module.parse_program_bytes_hex),
    ):
        try:
            parser(value, label="verifier program bytes")
        except module.argparse.ArgumentTypeError as exc:
            assert "must not contain whitespace" in str(exc)
        else:
            raise AssertionError("padded Solana verifier program bytes were accepted")

    try:
        module.parse_program_bytes_base64("", label="verifier program bytes")
    except module.argparse.ArgumentTypeError as exc:
        assert "must not be empty" in str(exc)
    else:
        raise AssertionError("empty Solana verifier program base64 was accepted")

    try:
        module.parse_program_bytes_base64(
            base64.b64encode(bytes(8)).decode("ascii"),
            label="verifier program bytes",
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must not be all zero" in str(exc)
    else:
        raise AssertionError("zero Solana verifier program base64 was accepted")

    try:
        module.parse_program_bytes_base64("not@@base64", label="verifier program bytes")
    except module.argparse.ArgumentTypeError as exc:
        assert "must be base64" in str(exc)
    else:
        raise AssertionError("malformed Solana verifier program base64 was accepted")

    try:
        module.parse_program_bytes_base64(
            noncanonical_base64_alias(b"\x7fELFsol"),
            label="verifier program bytes",
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "canonical base64" in str(exc)
    else:
        raise AssertionError("non-canonical Solana verifier program base64 was accepted")

    try:
        module.parse_program_bytes_base64(
            base64.b64encode(b"not-elf").decode("ascii"),
            label="verifier program bytes",
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "BPF ELF" in str(exc)
    else:
        raise AssertionError("non-ELF Solana verifier program base64 was accepted")

    try:
        module.parse_program_bytes_hex(
            "0x" + b"not-elf".hex(),
            label="verifier program bytes",
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "BPF ELF" in str(exc)
    else:
        raise AssertionError("non-ELF Solana verifier program hex was accepted")

    non_elf_file = tmp_path / "not-elf.so"
    non_elf_file.write_bytes(b"not-elf")
    try:
        module.parse_program_bytes_file(str(non_elf_file), label="verifier program bytes")
    except module.argparse.ArgumentTypeError as exc:
        assert "BPF ELF" in str(exc)
    else:
        raise AssertionError("non-ELF Solana verifier program file was accepted")

    try:
        module.solana_verifier_program_code_hash(b"not-elf")
    except ValueError as exc:
        assert "BPF ELF" in str(exc)
    else:
        raise AssertionError("non-ELF Solana verifier program hash was accepted")


def test_solana_program_id_parser_rejects_zero_malformed_and_wrong_width():
    module = load_evidence_module()

    assert (
        module.normalize_solana_program_id(
            SOLANA_VERIFIER_PROGRAM_ID,
            label="verifier program id",
        )
        == SOLANA_VERIFIER_PROGRAM_ID
    )

    try:
        module.normalize_solana_program_id(
            " " + SOLANA_VERIFIER_PROGRAM_ID + " ",
            label="verifier program id",
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must not contain whitespace" in str(exc)
    else:
        raise AssertionError("padded Solana program id was accepted")

    try:
        module.normalize_solana_program_id(
            "11111111111111111111111111111111",
            label="verifier program id",
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must not decode to zero" in str(exc)
    else:
        raise AssertionError("zero Solana program id was accepted")

    try:
        module.normalize_solana_program_id("0OIl", label="verifier program id")
    except module.argparse.ArgumentTypeError as exc:
        assert "must be canonical base58" in str(exc)
    else:
        raise AssertionError("malformed Solana program id was accepted")

    try:
        module.normalize_solana_program_id("So1111", label="verifier program id")
    except module.argparse.ArgumentTypeError as exc:
        assert "must decode to 32 bytes" in str(exc)
    else:
        raise AssertionError("short Solana program id was accepted")


def test_solana_destination_binding_hash_matches_rust_vector():
    module = load_evidence_module()

    assert (
        module.solana_destination_binding_key()
        == "sccp:0:3:sol:solana-program-v1:2"
    )
    assert (
        module.solana_destination_binding_hash().hex()
        == SOLANA_DESTINATION_BINDING_VECTOR
    )


def test_solana_route_allowlist_hash_matches_lane_evidence_vector():
    module = load_evidence_module()

    assert (
        module.solana_route_allowlist_hash(
            source_verifier_material_hash=bytes.fromhex(
                SOURCE_VERIFIER_MATERIAL_HASH
            ),
            source_adapter_engine_deployment_hash=bytes.fromhex(
                SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
            ),
            destination_binding_hash=bytes.fromhex(SOLANA_DESTINATION_BINDING_VECTOR),
        ).hex()
        == SOLANA_ROUTE_ALLOWLIST_HASH_VECTOR
    )

    for replayed in (
        {
            "source_verifier_material_hash": bytes.fromhex(
                SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
            ),
            "source_adapter_engine_deployment_hash": bytes.fromhex(
                SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
            ),
            "destination_binding_hash": bytes.fromhex(SOLANA_DESTINATION_BINDING_VECTOR),
        },
        {
            "source_verifier_material_hash": bytes.fromhex(SOURCE_VERIFIER_MATERIAL_HASH),
            "source_adapter_engine_deployment_hash": bytes.fromhex(
                SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
            ),
            "destination_binding_hash": bytes.fromhex(SOURCE_VERIFIER_MATERIAL_HASH),
        },
        {
            "source_verifier_material_hash": bytes.fromhex(SOURCE_VERIFIER_MATERIAL_HASH),
            "source_adapter_engine_deployment_hash": bytes.fromhex(
                SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
            ),
            "destination_binding_hash": bytes.fromhex(
                SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
            ),
        },
    ):
        try:
            module.solana_route_allowlist_hash(**replayed)
        except ValueError as exc:
            assert "Solana route allowlist evidence hashes must be distinct" in str(exc)
        else:
            raise AssertionError("Solana route allowlist accepted replayed hash role")


def test_solana_route_canary_rejects_verifier_code_hash_role_reuse():
    module = load_evidence_module()
    verifier_code_hash = module.solana_verifier_program_code_hash(
        SOLANA_VERIFIER_PROGRAM_BYTES
    )
    args = {
        "route_allowlist_hash": bytes.fromhex(SOLANA_ROUTE_ALLOWLIST_HASH_VECTOR),
        "destination_binding_hash": bytes.fromhex(SOLANA_DESTINATION_BINDING_VECTOR),
        "source_verifier_material_hash": bytes.fromhex(
            SOURCE_VERIFIER_MATERIAL_HASH
        ),
        "source_adapter_engine_deployment_hash": bytes.fromhex(
            SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
        ),
        "verifier_program_id": SOLANA_VERIFIER_PROGRAM_ID,
        "verifier_code_hash": verifier_code_hash,
        "rpc_commitment": "finalized",
        "program_owner": module.SOLANA_UPGRADEABLE_LOADER_ID,
        "programdata_owner": module.SOLANA_UPGRADEABLE_LOADER_ID,
        "program_immutable": True,
        "program_account_data": module.solana_upgradeable_program_account_data(
            SOLANA_PROGRAMDATA_ADDRESS
        ),
        "programdata_address": SOLANA_PROGRAMDATA_ADDRESS,
        "programdata_slot": 4321,
        "expected_programdata_slot": 4321,
        "program_account_context_slot": 9000,
        "programdata_account_context_slot": 9000,
        "programdata_metadata": module.solana_immutable_programdata_metadata(4321),
        "programdata_executable": SOLANA_VERIFIER_PROGRAM_BYTES,
    }

    for field in (
        "route_allowlist_hash",
        "destination_binding_hash",
        "source_verifier_material_hash",
        "source_adapter_engine_deployment_hash",
    ):
        replay_args = dict(args)
        replay_args[field] = verifier_code_hash
        try:
            module.solana_route_canary_evidence_hash(**replay_args)
        except ValueError as exc:
            assert f"verifier_code_hash must differ from {field}" in str(exc)
        else:
            raise AssertionError(
                f"Solana route canary accepted verifier code hash replay of {field}"
            )

    for field, source_field in (
        ("route_allowlist_hash", "destination_binding_hash"),
        ("route_allowlist_hash", "source_verifier_material_hash"),
        ("route_allowlist_hash", "source_adapter_engine_deployment_hash"),
        ("destination_binding_hash", "source_verifier_material_hash"),
        ("destination_binding_hash", "source_adapter_engine_deployment_hash"),
    ):
        replay_args = dict(args)
        replay_args[field] = replay_args[source_field]
        try:
            module.solana_route_canary_evidence_hash(**replay_args)
        except ValueError as exc:
            assert "Solana route canary governed hashes must be distinct" in str(exc)
        else:
            raise AssertionError(
                f"Solana route canary accepted governed hash replay of {field}"
            )


def test_solana_toml_rendering_carries_destination_profile_ids():
    module = load_evidence_module()
    program_hash = module.solana_verifier_program_code_hash(
        SOLANA_VERIFIER_PROGRAM_BYTES
    )
    program_account_data = module.solana_upgradeable_program_account_data(
        SOLANA_PROGRAMDATA_ADDRESS
    )
    programdata_metadata = module.solana_immutable_programdata_metadata(4321)
    route_canary_hash = solana_route_canary_evidence_hash(module)
    rendered = module.render_toml(solana_toml_args(module))

    assert (
        '# sccp_solana_destination_binding_hash = "0x'
        + SOLANA_DESTINATION_BINDING_VECTOR
        + '"'
        in rendered
    )
    assert (
        '# sccp_solana_route_allowlist_hash = "0x'
        + SOLANA_ROUTE_ALLOWLIST_HASH_VECTOR
        + '"'
        in rendered
    )
    assert '# sccp_solana_rpc_commitment = "finalized"' in rendered
    assert (
        '# sccp_solana_program_owner = "BPFLoaderUpgradeab1e11111111111111111111111"'
        in rendered
    )
    assert (
        '# sccp_solana_programdata_owner = "BPFLoaderUpgradeab1e11111111111111111111111"'
        in rendered
    )
    assert '# sccp_solana_program_immutable = "true"' in rendered
    assert '# sccp_solana_program_account_data_len = "36"' in rendered
    assert (
        '# sccp_solana_program_account_data_base64 = "'
        + base64.b64encode(program_account_data).decode("ascii")
        + '"'
        in rendered
    )
    assert f'# sccp_solana_programdata_address = "{SOLANA_PROGRAMDATA_ADDRESS}"' in rendered
    assert '# sccp_solana_programdata_slot = "4321"' in rendered
    assert '# sccp_solana_expected_programdata_slot = "4321"' in rendered
    assert '# sccp_solana_program_account_context_slot = "9000"' in rendered
    assert '# sccp_solana_programdata_account_context_slot = "9000"' in rendered
    assert (
        '# sccp_solana_programdata_metadata_blake2b256 = "0x'
        + hashlib.blake2b(programdata_metadata, digest_size=32).hexdigest()
        + '"'
        in rendered
    )
    assert (
        '# sccp_solana_programdata_metadata_base64 = "'
        + base64.b64encode(programdata_metadata).decode("ascii")
        + '"'
        in rendered
    )
    assert (
        '# sccp_solana_programdata_executable_blake2b256 = "0x'
        + program_hash.hex()
        + '"'
        in rendered
    )
    assert (
        '# sccp_solana_programdata_executable_base64 = "'
        + SOLANA_VERIFIER_PROGRAM_BYTES_BASE64
        + '"'
        in rendered
    )
    assert 'destination_binding_key = "sccp:0:3:sol:solana-program-v1:2"' in rendered
    assert (
        'destination_binding_hash = "0x'
        + SOLANA_DESTINATION_BINDING_VECTOR
        + '"'
        in rendered
    )
    assert "[[zk.sccp_destination_rollouts]]" in rendered
    assert "[[zk.sccp_route_allowlists]]" in rendered
    assert "domain = 3" in rendered
    assert 'chain = "sol"' in rendered
    assert 'verifier_plan = "SolanaProgramNativeRecursive"' in rendered
    assert f'verifier_identity = "{SOLANA_VERIFIER_PROGRAM_ID}"' in rendered
    assert 'verifier_code_hash = "0x' + program_hash.hex() + '"' in rendered
    assert (
        'anchor_id = "sccp:sol:destination-anchor:solana-mainnet-beta:v1"'
        in rendered
    )
    assert (
        'route_allowlist_id = "sccp:sol:route-allowlist:solana-mainnet-beta:v1"'
        in rendered
    )
    assert (
        'route_allowlist_hash = "0x' + SOLANA_ROUTE_ALLOWLIST_HASH_VECTOR + '"'
        in rendered
    )
    assert '# sccp_route_canary_status = "passed"' in rendered
    assert 'route_canary_status = "passed"' in rendered
    assert (
        '# sccp_route_canary_evidence_hash = "0x'
        + route_canary_hash
        + '"'
        in rendered
    )
    assert (
        'route_canary_evidence_hash = "0x'
        + route_canary_hash
        + '"'
        in rendered
    )
    assert (
        '# sccp_route_canary_route_allowlist_hash = "0x'
        + SOLANA_ROUTE_ALLOWLIST_HASH_VECTOR
        + '"'
        in rendered
    )
    assert (
        'route_canary_route_allowlist_hash = "0x'
        + SOLANA_ROUTE_ALLOWLIST_HASH_VECTOR
        + '"'
        in rendered
    )
    assert (
        '# sccp_route_canary_destination_binding_hash = "0x'
        + SOLANA_DESTINATION_BINDING_VECTOR
        + '"'
        in rendered
    )
    assert (
        'route_canary_destination_binding_hash = "0x'
        + SOLANA_DESTINATION_BINDING_VECTOR
        + '"'
        in rendered
    )
    assert "blockers = []" in rendered

    missing_program_bytes_args = solana_args(module)
    try:
        module.render_toml(missing_program_bytes_args)
    except ValueError as exc:
        assert "--toml requires --verifier-program-bytes" in str(exc)
    else:
        raise AssertionError("Solana destination TOML rendered without program bytes")

    private_program_bytes_args = solana_args(module)
    private_program_bytes_args.verifier_code_hash = program_hash
    private_program_bytes_args.verifier_program_bytes_bytes = (
        SOLANA_VERIFIER_PROGRAM_BYTES
    )
    private_program_bytes_args.verifier_program_bytes_base64_text = (
        SOLANA_VERIFIER_PROGRAM_BYTES_BASE64
    )
    try:
        module.render_toml(private_program_bytes_args)
    except ValueError as exc:
        assert "--toml requires --verifier-program-bytes" in str(exc)
    else:
        raise AssertionError("Solana destination TOML accepted private byte metadata")

    try:
        module.render_toml(
            solana_toml_args(module),
            destination_binding_hash=bytes.fromhex("ee" * 32),
        )
    except ValueError as exc:
        assert "canonical SORA -> Solana binding" in str(exc)
    else:
        raise AssertionError("mismatched direct Solana destination binding hash was accepted")

    try:
        module._json_summary(solana_args(module), bytes.fromhex("ee" * 32), False)
    except ValueError as exc:
        assert "canonical SORA -> Solana binding" in str(exc)
    else:
        raise AssertionError("mismatched direct Solana JSON binding hash was accepted")

    bad_code_args = solana_args(module)
    bad_code_args.verifier_code_hash = bytes(32)
    try:
        module.render_toml(bad_code_args)
    except ValueError as exc:
        assert "verifier_code_hash must not be zero" in str(exc)
    else:
        raise AssertionError("zero direct Solana verifier code hash was accepted")

    try:
        module._json_summary(
            bad_code_args,
            bytes.fromhex(SOLANA_DESTINATION_BINDING_VECTOR),
            False,
        )
    except ValueError as exc:
        assert "verifier_code_hash must not be zero" in str(exc)
    else:
        raise AssertionError("zero direct Solana JSON verifier code hash was accepted")

    stale_program_context_args = solana_args(module)
    stale_program_context_args.program_account_context_slot = 4000
    try:
        module.render_toml(stale_program_context_args)
    except ValueError as exc:
        message = str(exc)
        assert "--program-account-context-slot" in message
        assert "--programdata-slot" in message
    else:
        raise AssertionError("stale direct Solana program context slot was accepted")

    try:
        module._json_summary(
            stale_program_context_args,
            bytes.fromhex(SOLANA_DESTINATION_BINDING_VECTOR),
            True,
        )
    except ValueError as exc:
        message = str(exc)
        assert "--program-account-context-slot" in message
        assert "--programdata-slot" in message
    else:
        raise AssertionError(
            "stale direct Solana JSON program context slot was accepted"
        )

    for field, flag in (
        ("programdata_slot", True),
        ("program_account_context_slot", True),
        ("programdata_account_context_slot", True),
    ):
        boolean_slot_args = solana_args(module)
        setattr(boolean_slot_args, field, flag)
        try:
            module.render_toml(boolean_slot_args)
        except ValueError as exc:
            assert "requires" in str(exc)
        else:
            raise AssertionError(f"boolean direct Solana {field} was accepted")

        try:
            module._json_summary(
                boolean_slot_args,
                bytes.fromhex(SOLANA_DESTINATION_BINDING_VECTOR),
                True,
            )
        except ValueError as exc:
            assert "requires" in str(exc)
        else:
            raise AssertionError(f"boolean direct Solana JSON {field} was accepted")

    try:
        module.solana_immutable_programdata_metadata(True)
    except ValueError as exc:
        assert "programdata_slot must be a positive u64" in str(exc)
    else:
        raise AssertionError("boolean Solana ProgramData metadata slot was accepted")

    aliased_programdata_args = solana_args(module)
    aliased_programdata_args.programdata_address = (
        aliased_programdata_args.verifier_program_id
    )
    try:
        module.render_toml(aliased_programdata_args)
    except ValueError as exc:
        assert "programdata_address must differ from verifier_program_id" in str(exc)
    else:
        raise AssertionError("aliased Solana ProgramData address was accepted")

    try:
        module._json_summary(
            aliased_programdata_args,
            bytes.fromhex(SOLANA_DESTINATION_BINDING_VECTOR),
            True,
        )
    except ValueError as exc:
        assert "programdata_address must differ from verifier_program_id" in str(exc)
    else:
        raise AssertionError("aliased Solana ProgramData JSON metadata was accepted")

    bad_allowlist_args = solana_args(module)
    bad_allowlist_args.route_allowlist_hash = bytes(32)
    try:
        module.render_toml(bad_allowlist_args)
    except ValueError as exc:
        assert "route_allowlist_hash must not be zero" in str(exc)
    else:
        raise AssertionError("zero direct Solana route allowlist hash was accepted")

    try:
        module._json_summary(
            bad_allowlist_args,
            bytes.fromhex(SOLANA_DESTINATION_BINDING_VECTOR),
            False,
        )
    except ValueError as exc:
        assert "route_allowlist_hash must not be zero" in str(exc)
    else:
        raise AssertionError("zero direct Solana JSON route allowlist hash was accepted")

    drifted_allowlist_args = solana_args(module)
    drifted_allowlist_args.route_allowlist_hash = bytes.fromhex("dd" * 32)
    try:
        module.render_toml(drifted_allowlist_args)
    except ValueError as exc:
        assert "canonical source, deployment, and destination evidence" in str(exc)
    else:
        raise AssertionError("drifted direct Solana route allowlist hash was accepted")

    try:
        module._json_summary(
            drifted_allowlist_args,
            bytes.fromhex(SOLANA_DESTINATION_BINDING_VECTOR),
            False,
        )
    except ValueError as exc:
        assert "canonical source, deployment, and destination evidence" in str(exc)
    else:
        raise AssertionError("drifted direct Solana JSON route hash was accepted")

    missing_canary_args = solana_args(module)
    missing_canary_args.route_canary_evidence_hash = None
    try:
        module.render_toml(missing_canary_args)
    except ValueError as exc:
        assert "--route-canary-evidence-hash" in str(exc)
    else:
        raise AssertionError("Solana destination TOML accepted without route canary evidence")

    missing_canary_summary = module._json_summary(
        missing_canary_args,
        bytes.fromhex(SOLANA_DESTINATION_BINDING_VECTOR),
        True,
    )
    assert missing_canary_summary["route_canary_ready"] is False
    assert missing_canary_summary["full_toml_ready"] is False
    assert missing_canary_summary["toml_ready"] is False

    for attr_name, label in (
        ("source_verifier_material_hash", "source_verifier_material_hash"),
        (
            "source_adapter_engine_deployment_hash",
            "source_adapter_engine_deployment_hash",
        ),
    ):
        replay_args = solana_toml_args(module)
        replay_args.route_canary_evidence_hash = getattr(replay_args, attr_name)
        try:
            module.render_toml(replay_args)
        except ValueError as exc:
            assert label in str(exc)
        else:
            raise AssertionError(
                f"Solana destination TOML accepted route canary replay of {label}"
            )

        try:
            module._json_summary(
                replay_args,
                bytes.fromhex(SOLANA_DESTINATION_BINDING_VECTOR),
                True,
            )
        except ValueError as exc:
            assert label in str(exc)
        else:
            raise AssertionError(
                f"Solana destination JSON accepted route canary replay of {label}"
            )


def test_solana_cli_json_summary_and_toml_output(capsys):
    module = load_evidence_module()
    program_hash = module.solana_verifier_program_code_hash(
        SOLANA_VERIFIER_PROGRAM_BYTES
    ).hex()
    route_canary_hash = solana_route_canary_evidence_hash(module)
    args = [
        "--verifier-program-id",
        SOLANA_VERIFIER_PROGRAM_ID,
        "--verifier-code-hash",
        "0x" + "bb" * 32,
        "--programdata-address",
        SOLANA_PROGRAMDATA_ADDRESS,
        "--programdata-slot",
        "4321",
        "--program-account-context-slot",
        "9000",
        "--programdata-account-context-slot",
        "9000",
        "--route-allowlist-hash",
        "0x" + SOLANA_ROUTE_ALLOWLIST_HASH_VECTOR,
        "--source-verifier-material-hash",
        "0x" + SOURCE_VERIFIER_MATERIAL_HASH,
        "--source-adapter-engine-deployment-hash",
        "0x" + SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH,
    ]
    binding_only_args = args[:4]
    full_args = [
        *args,
        "--expected-destination-binding-hash",
        "0x" + SOLANA_DESTINATION_BINDING_VECTOR,
        "--route-canary-evidence-hash",
        "0x" + route_canary_hash,
    ]
    route_args_without_programdata = [
        *binding_only_args,
        "--route-allowlist-hash",
        "0x" + SOLANA_ROUTE_ALLOWLIST_HASH_VECTOR,
        "--source-verifier-material-hash",
        "0x" + SOURCE_VERIFIER_MATERIAL_HASH,
        "--source-adapter-engine-deployment-hash",
        "0x" + SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH,
        "--expected-destination-binding-hash",
        "0x" + SOLANA_DESTINATION_BINDING_VECTOR,
        "--route-canary-evidence-hash",
        "0x" + route_canary_hash,
    ]
    full_args_without_canary = [
        *args,
        "--expected-destination-binding-hash",
        "0x" + SOLANA_DESTINATION_BINDING_VECTOR,
    ]
    program_args = [
        "--verifier-program-id",
        SOLANA_VERIFIER_PROGRAM_ID,
        "--verifier-program-bytes-base64",
        SOLANA_VERIFIER_PROGRAM_BYTES_BASE64,
        *args[4:],
    ]
    full_args_with_program_bytes = [
        *program_args,
        "--expected-destination-binding-hash",
        "0x" + SOLANA_DESTINATION_BINDING_VECTOR,
        "--route-canary-evidence-hash",
        "0x" + route_canary_hash,
    ]

    assert module.main(binding_only_args) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["domain"] == 3
    assert output["chain"] == "sol"
    assert output["verifier_plan"] == "SolanaProgramNativeRecursive"
    assert output["verifier_identity"] == SOLANA_VERIFIER_PROGRAM_ID
    assert output["verifier_code_hash"] == "0x" + "bb" * 32
    assert output["destination_binding_key"] == "sccp:0:3:sol:solana-program-v1:2"
    assert output["destination_binding_hash"] == (
        "0x" + SOLANA_DESTINATION_BINDING_VECTOR
    )
    assert output["expected_destination_binding_hash_matches"] is False
    assert output["route_allowlist_evidence_ready"] is False
    assert output["route_canary_ready"] is False
    assert output["programdata_metadata_ready"] is False
    assert output["verifier_program_bytes_present"] is False
    assert output["full_toml_ready"] is False
    assert output["toml_ready"] is False
    assert "route_allowlist_hash" not in output

    try:
        module.main(args)
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("unpinned Solana route allowlist hash was accepted")

    try:
        module.main(
            [
                *binding_only_args,
                "--expected-destination-binding-hash",
                "0x" + SOLANA_DESTINATION_BINDING_VECTOR,
                "--route-allowlist-hash",
                "0x" + SOLANA_ROUTE_ALLOWLIST_HASH_VECTOR,
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("partial Solana route allowlist evidence was accepted")

    assert module.main(full_args_without_canary) == 0
    no_canary = json.loads(capsys.readouterr().out)
    assert no_canary["expected_destination_binding_hash_matches"] is True
    assert no_canary["expected_route_allowlist_hash_matches"] is True
    assert no_canary["route_allowlist_evidence_ready"] is True
    assert no_canary["route_canary_ready"] is False
    assert no_canary["programdata_metadata_ready"] is True
    assert no_canary["verifier_program_bytes_present"] is False
    assert no_canary["full_toml_ready"] is False
    assert no_canary["toml_ready"] is False
    assert "route_canary" not in no_canary

    try:
        module.main(route_args_without_programdata)
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("Solana route canary was accepted without ProgramData")

    try:
        module.main([*full_args_without_canary, "--toml"])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("Solana destination TOML rendered without route canary evidence")

    try:
        module.main(full_args)
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("Solana route canary was accepted without program bytes")

    assert module.main(full_args_with_program_bytes) == 0
    matched_with_program = json.loads(capsys.readouterr().out)
    assert matched_with_program["verifier_code_hash"] == "0x" + program_hash
    assert matched_with_program["verifier_program_bytes_base64"] == (
        SOLANA_VERIFIER_PROGRAM_BYTES_BASE64
    )
    assert matched_with_program["verifier_program_bytes_present"] is True
    assert matched_with_program["route_canary_ready"] is True
    assert matched_with_program["route_canary"]["status"] == "passed"
    assert matched_with_program["route_canary"]["evidence_hash"] == (
        "0x" + route_canary_hash
    )
    assert matched_with_program["full_toml_ready"] is True
    assert matched_with_program["toml_ready"] is True

    try:
        module.main([*args, "--toml"])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("unpinned Solana destination TOML was accepted")

    try:
        module.main([*full_args, "--toml"])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("Solana destination TOML rendered without program bytes")

    assert module.main([*full_args_with_program_bytes, "--toml"]) == 0
    rendered = capsys.readouterr().out
    assert '# sccp_solana_rpc_commitment = "finalized"' in rendered
    assert f'# sccp_solana_programdata_address = "{SOLANA_PROGRAMDATA_ADDRESS}"' in rendered
    assert '# sccp_solana_programdata_slot = "4321"' in rendered
    assert (
        '# sccp_solana_programdata_executable_blake2b256 = "0x'
        + program_hash
        in rendered
    )
    assert (
        '# sccp_solana_programdata_executable_base64 = "'
        + SOLANA_VERIFIER_PROGRAM_BYTES_BASE64
        + '"'
        in rendered
    )
    assert "[[zk.sccp_destination_rollouts]]" in rendered
    assert "[[zk.sccp_route_allowlists]]" in rendered

    try:
        module.main([*args, "--expected-destination-binding-hash", "0x" + "ee" * 32])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("mismatched Solana destination binding hash was accepted")

    bad_route_args = [
        value if value != "0x" + SOLANA_ROUTE_ALLOWLIST_HASH_VECTOR else "0x" + "dd" * 32
        for value in full_args
    ]
    try:
        module.main(bad_route_args)
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("mismatched Solana route allowlist hash was accepted")


def test_solana_cli_derives_verifier_code_hash_from_program_bytes(
    capsys,
    tmp_path,
):
    module = load_evidence_module()
    program_bytes = b"\x7fELF\x02\x01solana-sccp-verifier"
    program_hash = module.solana_verifier_program_code_hash(program_bytes).hex()
    route_canary_hash = solana_route_canary_evidence_hash(
        module,
        program_bytes=program_bytes,
    )
    program_path = tmp_path / "verifier.so"
    program_path.write_bytes(program_bytes)
    args = [
        "--verifier-program-id",
        SOLANA_VERIFIER_PROGRAM_ID,
        "--verifier-program-bytes-file",
        str(program_path),
        "--programdata-address",
        SOLANA_PROGRAMDATA_ADDRESS,
        "--programdata-slot",
        "4321",
        "--program-account-context-slot",
        "9000",
        "--programdata-account-context-slot",
        "9000",
        "--route-allowlist-hash",
        "0x" + SOLANA_ROUTE_ALLOWLIST_HASH_VECTOR,
        "--source-verifier-material-hash",
        "0x" + SOURCE_VERIFIER_MATERIAL_HASH,
        "--source-adapter-engine-deployment-hash",
        "0x" + SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH,
        "--expected-destination-binding-hash",
        "0x" + SOLANA_DESTINATION_BINDING_VECTOR,
        "--route-canary-evidence-hash",
        "0x" + route_canary_hash,
    ]

    assert module.main(args) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["verifier_code_hash"] == "0x" + program_hash
    assert output["verifier_program_bytes_base64"] == base64.b64encode(
        program_bytes
    ).decode("ascii")
    assert output["verifier_program_bytes_base64_sha256"] == module.hashlib.sha256(
        base64.b64encode(program_bytes)
    ).hexdigest()
    assert output["full_toml_ready"] is True
    assert output["toml_ready"] is True

    hex_args = [
        value if value != "--verifier-program-bytes-file" else "--verifier-program-bytes-hex"
        for value in args
    ]
    hex_args[hex_args.index(str(program_path))] = "0x" + program_bytes.hex()
    assert module.main(hex_args) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["verifier_code_hash"] == "0x" + program_hash
    assert output["verifier_program_bytes_base64"] == base64.b64encode(
        program_bytes
    ).decode("ascii")

    base64_args = [
        value
        if value != "--verifier-program-bytes-file"
        else "--verifier-program-bytes-base64"
        for value in args
    ]
    base64_args[base64_args.index(str(program_path))] = base64.b64encode(
        program_bytes
    ).decode("ascii")
    assert module.main(base64_args) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["verifier_code_hash"] == "0x" + program_hash
    assert output["verifier_program_bytes_base64"] == base64.b64encode(
        program_bytes
    ).decode("ascii")

    try:
        module.main([*hex_args, "--verifier-code-hash", "0x" + "bb" * 32])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("mismatched Solana verifier program hash was accepted")


def test_solana_direct_renderers_derive_verifier_code_hash_from_program_bytes():
    module = load_evidence_module()
    program_bytes = b"\x7fELF\x02\x01solana-sccp-verifier"
    program_hash = module.solana_verifier_program_code_hash(program_bytes)

    args = solana_args(module)
    args.verifier_code_hash = None
    args.verifier_program_bytes_hex = program_bytes
    rendered = module.render_toml(args)
    assert 'verifier_code_hash = "0x' + program_hash.hex() + '"' in rendered
    assert (
        '# sccp_solana_programdata_executable_base64 = "'
        + base64.b64encode(program_bytes).decode("ascii")
        + '"'
        in rendered
    )
    assert args.verifier_code_hash == program_hash

    summary_args = solana_args(module)
    summary_args.verifier_code_hash = None
    summary_args.verifier_program_bytes_file = program_bytes
    summary = module._json_summary(
        summary_args,
        bytes.fromhex(SOLANA_DESTINATION_BINDING_VECTOR),
        True,
    )
    assert summary["verifier_code_hash"] == "0x" + program_hash.hex()
    assert summary["verifier_program_bytes_base64"] == base64.b64encode(
        program_bytes
    ).decode("ascii")
