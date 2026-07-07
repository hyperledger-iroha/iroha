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


class HostileExpectedDestinationBindingHash:
    def __str__(self):
        raise AssertionError(
            "secret-token TON destination expected binding hash was stringified"
        )

    def __repr__(self):
        raise AssertionError(
            "secret-token TON destination expected binding hash was repr'd"
        )

    def __eq__(self, _other):
        raise AssertionError(
            "secret-token TON destination expected binding hash was compared"
        )

    def __ne__(self, _other):
        raise AssertionError(
            "secret-token TON destination expected binding hash was compared"
        )


class HostileLastTransactionLt:
    def __str__(self):
        raise AssertionError("secret-token TON destination LT was stringified")

    def __repr__(self):
        raise AssertionError("secret-token TON destination LT was repr'd")

    def __eq__(self, _other):
        raise AssertionError("secret-token TON destination LT was compared")

    def __ne__(self, _other):
        raise AssertionError("secret-token TON destination LT was compared")

    def strip(self):
        raise AssertionError("secret-token TON destination LT was stripped")


class HostileTomlString(str):
    def __new__(cls):
        return str.__new__(cls, "blocked")

    def __str__(self):
        raise AssertionError("secret-token TON destination TOML string was stringified")

    def __repr__(self):
        raise AssertionError("secret-token TON destination TOML string was repr'd")

    def __eq__(self, _other):
        raise AssertionError("secret-token TON destination TOML string was compared")

    def __ne__(self, _other):
        raise AssertionError("secret-token TON destination TOML string was compared")


class HostileTonDestinationString(str):
    """String subclass that TON destination metadata must reject before hooks."""

    def __new__(cls, value):
        return str.__new__(cls, value)

    def __str__(self):
        raise AssertionError("secret-token TON destination string was stringified")

    def __repr__(self):
        raise AssertionError("secret-token TON destination string was repr'd")

    def __eq__(self, _other):
        raise AssertionError("secret-token TON destination string was compared")

    def __ne__(self, _other):
        raise AssertionError("secret-token TON destination string was compared")

    def __iter__(self):
        raise AssertionError("secret-token TON destination string was iterated")

    def __getitem__(self, _key):
        raise AssertionError("secret-token TON destination string was indexed")

    def strip(self, *args, **kwargs):
        raise AssertionError("secret-token TON destination string was stripped")

    def startswith(self, _prefix):
        raise AssertionError("secret-token TON destination string startswith ran")

    def lower(self):
        raise AssertionError("secret-token TON destination string lower ran")

    def isascii(self):
        raise AssertionError("secret-token TON destination string isascii ran")

    def isdecimal(self):
        raise AssertionError("secret-token TON destination string isdecimal ran")

    def encode(self, *_args, **_kwargs):
        raise AssertionError("secret-token TON destination string encode ran")


class HostileTonDestinationBytes(bytes):
    """Bytes subclass that TON destination metadata must reject before hooks."""

    def __new__(cls, value):
        return bytes.__new__(cls, value)

    def __bytes__(self):
        raise AssertionError("secret-token TON destination bytes coerced")

    def __repr__(self):
        raise AssertionError("secret-token TON destination bytes repr'd")

    def __str__(self):
        raise AssertionError("secret-token TON destination bytes stringified")

    def __len__(self):
        raise AssertionError("secret-token TON destination bytes length read")

    def __iter__(self):
        raise AssertionError("secret-token TON destination bytes iterated")

    def __getitem__(self, _key):
        raise AssertionError("secret-token TON destination bytes indexed")

    def startswith(self, _prefix):
        raise AssertionError("secret-token TON destination bytes startswith ran")

    def hex(self):
        raise AssertionError("secret-token TON destination bytes hex encoded")


class HostileTonDestinationBytearray(bytearray):
    """Bytearray subclass that TON destination metadata must reject before hooks."""

    def __init__(self, value):
        super().__init__(value)

    def __bytes__(self):
        raise AssertionError("secret-token TON destination bytearray coerced")

    def __repr__(self):
        raise AssertionError("secret-token TON destination bytearray repr'd")

    def __str__(self):
        raise AssertionError("secret-token TON destination bytearray stringified")

    def __len__(self):
        raise AssertionError("secret-token TON destination bytearray length read")

    def __iter__(self):
        raise AssertionError("secret-token TON destination bytearray iterated")

    def __getitem__(self, _key):
        raise AssertionError("secret-token TON destination bytearray indexed")

    def startswith(self, _prefix):
        raise AssertionError("secret-token TON destination bytearray startswith ran")

    def hex(self):
        raise AssertionError("secret-token TON destination bytearray hex encoded")


class HostileTomlInt(int):
    def __new__(cls):
        return int.__new__(cls, 1)

    def __str__(self):
        raise AssertionError("secret-token TON destination TOML integer was stringified")

    def __repr__(self):
        raise AssertionError("secret-token TON destination TOML integer was repr'd")


class HostileTomlList(list):
    def __iter__(self):
        raise AssertionError("secret-token TON destination TOML list was iterated")

    def __repr__(self):
        raise AssertionError("secret-token TON destination TOML list was repr'd")


def test_ton_destination_cli_redacts_top_level_exception_details(monkeypatch, capsys):
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
            patch.setattr(module, "apply_verifier_code_boc_hash", fail_apply)
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
            assert exception_type.__name__ not in captured.err


def test_ton_destination_redacts_verifier_address_parser_failures(monkeypatch):
    """Destination verifier address parser failures must not echo parser payloads."""

    module = load_evidence_module()
    args = ton_args(module)

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

            def fail_address(
                _value,
                *,
                label,
                exception_type=exception_type,
                secret_template=secret_template,
            ):
                raise exception_type(secret_template.format(label=label))

            patch.setattr(module, "normalize_ton_raw_address", fail_address)
            try:
                module._require_destination_evidence(args)
            except ValueError as exc:
                rendered = str(exc)
                assert rendered == "verifier_contract_address metadata is invalid"
                assert "secret-token" not in rendered
                assert forbidden_detail not in rendered
                assert exc.__cause__ is None
                assert exc.__suppress_context__ is True
            else:
                raise AssertionError(
                    f"TON destination leaked {exception_type.__name__} detail"
                )


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


def test_ton_destination_rejects_hostile_expected_binding_hash_without_hooks(
    monkeypatch,
):
    module = load_evidence_module()
    destination_binding_hash = bytes.fromhex(TON_DESTINATION_BINDING_VECTOR)

    class FakeParser:
        prog = "sccp-ton-destination-evidence-test"

        def __init__(self, args):
            self.args = args

        def parse_args(self, _argv):
            return self.args

        def exit(self, code, message=None):
            raise SystemExit((code, message or ""))

    cli_args = [
        "--verifier-contract-address",
        TON_VERIFIER_CONTRACT_ADDRESS,
        "--verifier-code-hash",
        "0x" + TON_CODE_BOC_ROOT_HASH,
        "--verifier-code-boc-hex",
        "0x" + TON_CODE_BOC_HEX,
        "--expected-destination-binding-hash",
        "0x" + TON_DESTINATION_BINDING_VECTOR,
    ]
    malformed_cases = (
        (
            HostileExpectedDestinationBindingHash,
            "--expected-destination-binding-hash must be 32 bytes",
        ),
        (
            lambda: bytes(32),
            "--expected-destination-binding-hash must not be zero",
        ),
        (
            lambda: b"\x11" * 31,
            "--expected-destination-binding-hash must be 32 bytes",
        ),
    )

    for make_value, expected_error in malformed_cases:
        for direct_call in (
            lambda args: module.render_toml(args, destination_binding_hash),
            lambda args: module._json_summary(args, destination_binding_hash, False),
        ):
            direct_args = ton_args(module)
            direct_args.expected_destination_binding_hash = make_value()
            try:
                direct_call(direct_args)
            except ValueError as exc:
                message = str(exc)
                assert expected_error in message
                assert "secret-token" not in message
                assert exc.__cause__ is None
            else:
                raise AssertionError(
                    "direct TON destination accepted malformed expected binding"
                )

        main_args = module.build_parser().parse_args(cli_args)
        main_args.expected_destination_binding_hash = make_value()
        fake_parser = FakeParser(main_args)
        with monkeypatch.context() as patch:
            patch.setattr(module, "build_parser", lambda: fake_parser)
            try:
                module.main([])
            except SystemExit as exc:
                code, message = exc.code
                assert code == 2
                assert expected_error in message
                assert "secret-token" not in message
            else:
                raise AssertionError(
                    "TON destination main accepted malformed expected binding"
                )


def test_ton_destination_json_summary_rejects_non_boolean_metadata_readiness(
    monkeypatch,
) -> None:
    """Destination metadata readiness helper drift must not coerce into readiness."""

    module = load_evidence_module()
    cases = (
        (
            "_toml_account_metadata_ready",
            "TON destination account metadata readiness must be a boolean",
        ),
        (
            "_code_boc_root_metadata_ready",
            "TON destination code BoC root readiness must be a boolean",
        ),
    )

    for helper_name, expected_error in cases:
        args = ton_args(module)
        with monkeypatch.context() as patch:
            patch.setattr(module, "_toml_account_metadata_ready", lambda _args: True)
            patch.setattr(module, "_code_boc_root_metadata_ready", lambda _args: True)
            patch.setattr(module, helper_name, lambda _args: "ready")
            try:
                module._json_summary(
                    args,
                    bytes.fromhex(TON_DESTINATION_BINDING_VECTOR),
                    True,
                )
            except ValueError as exc:
                assert str(exc) == expected_error
            else:
                raise AssertionError(
                    f"TON destination JSON summary accepted non-boolean {helper_name}"
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
                f"noncanonical TON verifier hash {value!r} was accepted"
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
        raise AssertionError("short TON verifier code hash was accepted")


def test_ton_destination_direct_parsers_redact_parser_causes(tmp_path):
    module = load_evidence_module()

    fixed_hex_payload_body = "secret-token-ton-destination-fixed-hex"
    fixed_hex_payload = "0x" + fixed_hex_payload_body
    fixed_hex_payload += "g" * (66 - len(fixed_hex_payload))
    code_hex_payload_body = "secret-token-ton-destination-code-hex"
    if len(code_hex_payload_body) % 2:
        code_hex_payload_body += "g"
    code_hex_payload = "0x" + code_hex_payload_body

    cases = (
        (
            lambda: module.parse_hex_bytes(
                fixed_hex_payload,
                label="verifier code hash",
                byte_length=32,
            ),
            "verifier code hash must be hex",
        ),
        (
            lambda: module.parse_code_boc_hex(
                code_hex_payload,
                label="code BoC",
            ),
            "code BoC must be hex",
        ),
        (
            lambda: module.parse_code_boc_base64(
                "secret-token-ton-destination-code-boc",
                label="code BoC",
            ),
            "code BoC must be base64 or base64url",
        ),
        (
            lambda: module.parse_code_boc_file(
                str(tmp_path / "secret-token-ton-destination-file-path.boc"),
                label="code BoC",
            ),
            "code BoC file cannot be read",
        ),
    )

    for parse, expected_message in cases:
        try:
            parse()
        except module.argparse.ArgumentTypeError as exc:
            rendered = str(exc)
            assert rendered == expected_message
            assert "secret-token" not in rendered
            assert "destination" not in rendered
            assert exc.__cause__ is None
            assert exc.__suppress_context__ is True
        else:
            raise AssertionError("TON destination parser leaked nested details")


def test_ton_destination_cli_parsers_reject_non_string_values_without_stringification():
    module = load_evidence_module()

    class HostileTonDestinationParserValue:
        def __str__(self):
            raise AssertionError(
                "secret-token-ton-destination-parser-value stringified"
            )

        def __repr__(self):
            raise AssertionError(
                "secret-token-ton-destination-parser-value repr leaked"
            )

        def strip(self):
            raise AssertionError(
                "secret-token-ton-destination-parser-value strip called"
            )

        def startswith(self, _prefix):
            raise AssertionError(
                "secret-token-ton-destination-parser-value startswith called"
            )

        def lower(self):
            raise AssertionError(
                "secret-token-ton-destination-parser-value lower called"
            )

        def isascii(self):
            raise AssertionError(
                "secret-token-ton-destination-parser-value isascii called"
            )

        def isdecimal(self):
            raise AssertionError(
                "secret-token-ton-destination-parser-value isdecimal called"
            )

    hostile = HostileTonDestinationParserValue()
    cases = (
        (
            lambda value: module.parse_hex_bytes(
                value,
                label="verifier code hash",
                byte_length=32,
            ),
            "verifier code hash must be canonical lowercase 0x hex",
        ),
        (
            lambda value: module.parse_code_boc_hex(value, label="code BoC"),
            "code BoC must be hex",
        ),
        (
            lambda value: module.parse_code_boc_base64(value, label="code BoC"),
            "code BoC must be base64 or base64url",
        ),
        (
            lambda value: module.parse_code_boc_file(value, label="code BoC"),
            "code BoC file cannot be read",
        ),
        (
            lambda value: module.parse_positive_decimal_text(
                value,
                label="last transaction LT",
            ),
            "last transaction LT must be a positive decimal",
        ),
        (
            lambda value: module.parse_account_status(value, label="account status"),
            "account status must be active",
        ),
        (
            lambda value: module.normalize_ton_raw_address(
                value,
                label="verifier contract address",
            ),
            "verifier contract address must be workchain:account_hex",
        ),
    )

    for parser, expected_message in cases:
        try:
            parser(hostile)
        except module.argparse.ArgumentTypeError as exc:
            assert str(exc) == expected_message
        else:
            raise AssertionError("non-string TON destination parser value was accepted")


def test_ton_destination_direct_parsers_redact_helper_exit_parser_causes(monkeypatch):
    module = load_evidence_module()

    for exception_type in (
        module.argparse.ArgumentTypeError,
        SystemExit,
        RuntimeError,
        TypeError,
        ValueError,
    ):
        detail = (
            "secret-token TON destination hex TypeError detail"
            if exception_type is TypeError
            else f"secret-token TON destination hex {exception_type.__name__} detail"
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
                    "0x" + "33" * 32,
                    "verifier code hash",
                    {"byte_length": 32},
                ),
                (
                    module.parse_code_boc_hex,
                    "0x" + TON_CODE_BOC_HEX,
                    "code BoC",
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
                        ), "TON destination hex ArgumentTypeError detail leaked"
                    assert exc.__cause__ is None
                    assert exc.__suppress_context__ is True
                else:
                    raise AssertionError(
                        f"{label} parser {exception_type.__name__} was accepted"
                    )


def test_ton_destination_nonzero_controls_reject_non_booleans():
    module = load_evidence_module()

    for nonzero in (1, "true", None):
        try:
            module.parse_hex_bytes(
                "0x" + "00" * 32,
                label="verifier code hash",
                byte_length=32,
                nonzero=nonzero,
            )
        except ValueError as exc:
            assert str(exc) == "TON destination fixed hex nonzero must be a boolean"
        else:
            raise AssertionError(
                "malformed TON destination fixed-hex nonzero control accepted"
            )

        try:
            module._require_fixed_bytes(
                bytes(32),
                label="verifier_code_hash",
                byte_length=32,
                nonzero=nonzero,
            )
        except ValueError as exc:
            assert str(exc) == "TON destination fixed bytes nonzero must be a boolean"
        else:
            raise AssertionError(
                "malformed TON destination fixed-bytes nonzero control accepted"
            )


def test_ton_destination_boolean_controls_reject_non_booleans():
    module = load_evidence_module()

    for control in (1, "true", None):
        try:
            module._json_summary(
                ton_args(module),
                bytes.fromhex(TON_DESTINATION_BINDING_VECTOR),
                control,
            )
        except ValueError as exc:
            assert str(exc) == (
                "TON destination JSON expected_matches must be a boolean"
            )
        else:
            raise AssertionError(
                "malformed TON destination JSON expected-matches control accepted"
            )


def test_ton_destination_code_boc_base64_redacts_helper_exit_decoder_causes(
    monkeypatch,
):
    module = load_evidence_module()

    for exception_type in (
        module.argparse.ArgumentTypeError,
        SystemExit,
        RuntimeError,
        TypeError,
        ValueError,
    ):

        def fail_decode(*_args, exception_type=exception_type, **_kwargs):
            raise exception_type(
                "secret-token TON code BoC base64 TypeError detail"
            )

        with monkeypatch.context() as patch:
            patch.setattr(module.base64, "b64decode", fail_decode)
            patch.setattr(module.base64, "urlsafe_b64decode", fail_decode)

            try:
                module.parse_code_boc_base64(
                    TON_CODE_BOC_BASE64,
                    label="code BoC",
                )
            except module.argparse.ArgumentTypeError as exc:
                rendered = str(exc)
                assert rendered == "code BoC must be base64 or base64url"
                assert "secret-token" not in rendered
                assert exception_type.__name__ not in rendered
                if exception_type is module.argparse.ArgumentTypeError:
                    assert (
                        "ArgumentTypeError" not in rendered
                    ), "TON destination base64 ArgumentTypeError detail leaked"
                assert exc.__cause__ is None
                assert exc.__suppress_context__ is True
            else:
                raise AssertionError(
                    "TON code BoC base64 decoder "
                    f"{exception_type.__name__} was accepted"
                )


def test_ton_code_boc_inline_parsers_reject_padded_values(tmp_path):
    module = load_evidence_module()

    assert module.parse_code_boc_hex("0x" + TON_CODE_BOC_HEX, label="code BoC") == (
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

    for value, expected in (
        (TON_CODE_BOC_HEX, "canonical lowercase 0x hex"),
        ("0X" + TON_CODE_BOC_HEX, "lowercase 0x prefix"),
        ("0x" + TON_CODE_BOC_HEX.upper(), "lowercase hex"),
    ):
        try:
            module.parse_code_boc_hex(value, label="code BoC")
        except module.argparse.ArgumentTypeError as exc:
            assert expected in str(exc)
        else:
            raise AssertionError(
                f"noncanonical TON code BoC hex {value!r} was accepted"
            )

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


def test_ton_code_boc_file_rejects_unreadable_file_shapes(tmp_path):
    module = load_evidence_module()
    valid_boc = bytes.fromhex(TON_CODE_BOC_HEX)
    outside = tmp_path / "secret-token-ton-code-outside.boc"
    outside.write_bytes(valid_boc)
    symlink_input = tmp_path / "secret-token-ton-code-link.boc"
    symlink_input.symlink_to(outside)
    directory_input = tmp_path / "secret-token-ton-code-dir.boc"
    directory_input.mkdir()
    real_dir = tmp_path / "real-ton-code-dir"
    real_dir.mkdir()
    (real_dir / "code.boc").write_bytes(valid_boc)
    parent_link = tmp_path / "secret-token-ton-code-parent"
    parent_link.symlink_to(real_dir, target_is_directory=True)
    parent_symlink_input = parent_link / "code.boc"
    missing_input = tmp_path / "secret-token-ton-code-missing.boc"

    class HostileCodeBocPath(str):
        def __new__(cls):
            return str.__new__(cls, str(outside))

        def __str__(self):
            raise AssertionError("secret-token TON code path was stringified")

        def __repr__(self):
            raise AssertionError("secret-token TON code path was repr'd")

        def __fspath__(self):
            raise AssertionError("secret-token TON code path was coerced")

    class HostileCodeBocPathLike:
        def __str__(self):
            raise AssertionError("secret-token TON code path-like was stringified")

        def __repr__(self):
            raise AssertionError("secret-token TON code path-like was repr'd")

        def __fspath__(self):
            raise AssertionError("secret-token TON code path-like was coerced")

    for path in (
        str(symlink_input),
        str(directory_input),
        str(parent_symlink_input),
        str(missing_input),
        outside,
        HostileCodeBocPath(),
        HostileCodeBocPathLike(),
    ):
        try:
            module.parse_code_boc_file(path, label="code BoC")
        except module.argparse.ArgumentTypeError as exc:
            rendered = str(exc)
            suppress_context = exc.__suppress_context__
        else:
            raise AssertionError("TON code BoC file shape was accepted")

        assert rendered == "code BoC file cannot be read"
        assert "secret-token" not in rendered
        assert "IsADirectoryError" not in rendered
        assert "FileNotFoundError" not in rendered
        assert suppress_context is True


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


def test_ton_route_canary_rejects_boolean_last_transaction_lt():
    module = load_evidence_module()
    args = ton_args(module)

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
            last_transaction_lt=True,
            last_transaction_hash=args.last_transaction_hash,
            verifier_code_boc_root_hash=args.verifier_code_hash,
        )
    except ValueError as exc:
        assert str(exc) == "last_transaction_lt must be a positive decimal"
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("boolean TON route-canary last_transaction_lt was accepted")


def test_ton_destination_rejects_non_string_last_transaction_lt_without_stringifying():
    module = load_evidence_module()
    destination_binding_hash = bytes.fromhex(TON_DESTINATION_BINDING_VECTOR)

    toml_args = ton_args(module)
    toml_args.last_transaction_lt = HostileLastTransactionLt()
    try:
        module.render_toml(toml_args)
    except ValueError as exc:
        rendered = str(exc)
        assert rendered == "--toml requires --last-transaction-lt"
        assert "secret-token" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError(
            "TON destination TOML accepted hostile last transaction LT"
        )

    summary_args = ton_args(module)
    summary_args.last_transaction_lt = HostileLastTransactionLt()
    try:
        module._json_summary(summary_args, destination_binding_hash, True)
    except ValueError as exc:
        rendered = str(exc)
        assert rendered == "last_transaction_lt must be a positive decimal"
        assert "secret-token" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError(
            "TON destination JSON accepted hostile last transaction LT"
        )


def test_ton_destination_exact_string_metadata_rejects_string_subclasses_without_hooks():
    module = load_evidence_module()
    destination_binding_hash = bytes.fromhex(TON_DESTINATION_BINDING_VECTOR)
    hostile_status = HostileTonDestinationString("active")
    hostile_boc = HostileTonDestinationString(TON_CODE_BOC_BASE64)
    args = ton_args(module)

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
            account_status=hostile_status,
            account_state_hash=args.account_state_hash,
            last_transaction_lt=args.last_transaction_lt,
            last_transaction_hash=args.last_transaction_hash,
            verifier_code_boc_root_hash=args.verifier_code_hash,
        )
    except ValueError as exc:
        rendered = str(exc)
        assert rendered == "account_status must be active"
        assert "secret-token" not in rendered
        assert exc.__cause__ is None
    else:
        raise AssertionError("TON route-canary accepted hostile account status")

    copied_boc_args = SimpleNamespace(
        verifier_code_hash=bytes.fromhex(TON_CODE_BOC_ROOT_HASH),
        verifier_code_boc_root_hash=bytes.fromhex(TON_CODE_BOC_ROOT_HASH),
        verifier_code_boc_hash_matches=True,
        verifier_code_boc_base64_text=hostile_boc,
    )
    try:
        module._require_code_boc_root_metadata(copied_boc_args, output="toml")
    except ValueError as exc:
        rendered = str(exc)
        assert rendered == "--toml has invalid verifier code BoC base64 evidence"
        assert "secret-token" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("TON TOML accepted hostile copied code BoC base64")

    status_args = ton_args(module)
    status_args.account_status = hostile_status
    try:
        module._json_summary(status_args, destination_binding_hash, True)
    except ValueError as exc:
        rendered = str(exc)
        assert rendered == "account_status must be active"
        assert "secret-token" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("TON JSON summary accepted hostile account status")

    summary_boc_args = ton_args(module)
    summary_boc_args.verifier_code_boc_hex = None
    summary_boc_args.verifier_code_boc_base64 = None
    summary_boc_args.verifier_code_boc_file = None
    summary_boc_args.verifier_code_boc_base64_text = hostile_boc
    try:
        module._json_summary(summary_boc_args, destination_binding_hash, True)
    except ValueError as exc:
        rendered = str(exc)
        assert rendered == "TON verifier code BoC base64 metadata is inconsistent"
        assert "secret-token" not in rendered
        assert exc.__cause__ is None
    else:
        raise AssertionError("TON JSON summary accepted hostile copied code BoC base64")


def test_ton_destination_byte_helpers_reject_subclasses_without_hooks():
    module = load_evidence_module()
    fixed_hash = b"\x11" * 32
    code_boc = bytes.fromhex(TON_CODE_BOC_HEX)
    code_boc_root_hash = bytes.fromhex(TON_CODE_BOC_ROOT_HASH)

    assert (
        module._require_fixed_bytes(
            bytearray(fixed_hash),
            label="verifier_code_hash",
            byte_length=32,
        )
        == fixed_hash
    )
    assert module.ton_boc_single_root_hash(bytearray(code_boc)) == code_boc_root_hash

    exact_bytearray_args = ton_args(module)
    exact_bytearray_args.verifier_code_boc_hex = bytearray(code_boc)
    module.apply_verifier_code_boc_hash(exact_bytearray_args)
    assert exact_bytearray_args.verifier_code_boc_bytes == code_boc

    exact_metadata_args = SimpleNamespace(
        verifier_code_hash=code_boc_root_hash,
        verifier_code_boc_root_hash=code_boc_root_hash,
        verifier_code_boc_hash_matches=True,
        verifier_code_boc_bytes=bytearray(code_boc),
        verifier_code_boc_base64_text=None,
    )
    module._require_code_boc_root_metadata(exact_metadata_args, output="toml")

    hostile_hash_values = (
        HostileTonDestinationBytes(fixed_hash),
        HostileTonDestinationBytearray(fixed_hash),
    )
    hostile_boc_values = (
        HostileTonDestinationBytes(code_boc),
        HostileTonDestinationBytearray(code_boc),
    )

    for hostile_hash in hostile_hash_values:
        try:
            module._require_fixed_bytes(
                hostile_hash,
                label="verifier_code_hash",
                byte_length=32,
            )
        except ValueError as exc:
            rendered = str(exc)
            assert rendered == "verifier_code_hash must be 32 bytes"
            assert "secret-token" not in rendered
            assert exc.__cause__ is None
        else:
            raise AssertionError("TON destination accepted hostile fixed bytes")

    for hostile_boc in hostile_boc_values:
        for call, expected_message in (
            (
                lambda hostile_boc=hostile_boc: module.ton_boc_single_root_hash(
                    hostile_boc
                ),
                "TON code BoC must be bytes",
            ),
            (
                lambda hostile_boc=hostile_boc: module.apply_verifier_code_boc_hash(
                    SimpleNamespace(
                        verifier_code_hash=code_boc_root_hash,
                        verifier_code_boc_root_hash=None,
                        verifier_code_boc_hex=hostile_boc,
                        verifier_code_boc_base64=None,
                        verifier_code_boc_file=None,
                    )
                ),
                "TON code BoC must be bytes",
            ),
            (
                lambda hostile_boc=hostile_boc: (
                    module._require_code_boc_root_metadata(
                        SimpleNamespace(
                            verifier_code_hash=code_boc_root_hash,
                            verifier_code_boc_root_hash=code_boc_root_hash,
                            verifier_code_boc_hash_matches=True,
                            verifier_code_boc_bytes=hostile_boc,
                            verifier_code_boc_base64_text=None,
                        ),
                        output="toml",
                    )
                ),
                "--toml requires verifier code BoC byte evidence "
                "(use --verifier-code-boc-hex, --verifier-code-boc-base64, "
                "or --verifier-code-boc-file)",
            ),
        ):
            try:
                call()
            except ValueError as exc:
                rendered = str(exc)
                assert rendered == expected_message
                assert "secret-token" not in rendered
                assert exc.__cause__ is None
            else:
                raise AssertionError("TON destination accepted hostile code BoC")


def test_ton_destination_toml_renderer_rejects_string_subclasses_without_hooks():
    module = load_evidence_module()

    cases = (
        lambda: module._toml_string(HostileTomlString()),
        lambda: module._toml_line("verifier_identity", HostileTomlString()),
        lambda: module._toml_line("blockers", [HostileTomlString()]),
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
                "TON destination TOML renderer accepted hostile subclass value"
            )


def test_ton_destination_account_metadata_redacts_parser_causes(monkeypatch):
    module = load_evidence_module()

    args = ton_args(module)
    args.account_status = "secret-token-ton-destination-account-status"
    try:
        module._json_summary(
            args,
            bytes.fromhex(TON_DESTINATION_BINDING_VECTOR),
            True,
        )
    except ValueError as exc:
        rendered = str(exc)
        assert rendered == "account_status must be active"
        assert "secret-token" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("TON destination account status parser detail leaked")

    helper_failures = (
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

    for exception_type, secret_template, forbidden_detail in helper_failures:
        args = ton_args(module)
        with monkeypatch.context() as patch:
            def reject_account_status(
                _value,
                *,
                label,
                exception_type=exception_type,
                secret_template=secret_template,
            ):
                raise exception_type(secret_template.format(label=label))

            patch.setattr(module, "parse_account_status", reject_account_status)
            try:
                module._json_summary(
                    args,
                    bytes.fromhex(TON_DESTINATION_BINDING_VECTOR),
                    True,
                )
            except ValueError as exc:
                rendered = str(exc)
                assert rendered == "account_status must be active"
                assert "secret-token" not in rendered
                assert forbidden_detail not in rendered
                assert exc.__cause__ is None
                assert exc.__suppress_context__ is True
            else:
                raise AssertionError("TON account status helper detail leaked")

    args = ton_args(module)
    args.last_transaction_lt = "secret-token-ton-destination-last-lt"
    try:
        module._json_summary(
            args,
            bytes.fromhex(TON_DESTINATION_BINDING_VECTOR),
            True,
        )
    except ValueError as exc:
        rendered = str(exc)
        assert rendered == "last_transaction_lt must be a positive decimal"
        assert "secret-token" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("TON destination last-transaction parser detail leaked")

    for exception_type, secret_template, forbidden_detail in helper_failures:
        args = ton_args(module)
        with monkeypatch.context() as patch:
            def reject_last_transaction_lt(
                _value,
                *,
                label,
                exception_type=exception_type,
                secret_template=secret_template,
            ):
                raise exception_type(secret_template.format(label=label))

            patch.setattr(
                module,
                "parse_positive_decimal_text",
                reject_last_transaction_lt,
            )
            try:
                module._json_summary(
                    args,
                    bytes.fromhex(TON_DESTINATION_BINDING_VECTOR),
                    True,
                )
            except ValueError as exc:
                rendered = str(exc)
                assert rendered == "last_transaction_lt must be a positive decimal"
                assert "secret-token" not in rendered
                assert forbidden_detail not in rendered
                assert exc.__cause__ is None
                assert exc.__suppress_context__ is True
            else:
                raise AssertionError("TON last-transaction helper detail leaked")

    args = ton_args(module)
    args.last_transaction_lt = "secret-token-ton-destination-toml-lt"
    try:
        module._require_toml_account_metadata(args, output="toml")
    except ValueError as exc:
        rendered = str(exc)
        assert rendered == "--toml requires --last-transaction-lt"
        assert "secret-token" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("TON destination TOML LT parser detail leaked")

    for exception_type, secret_template, forbidden_detail in helper_failures:
        args = ton_args(module)
        with monkeypatch.context() as patch:
            def reject_toml_last_transaction_lt(
                _value,
                *,
                label,
                exception_type=exception_type,
                secret_template=secret_template,
            ):
                raise exception_type(secret_template.format(label=label))

            patch.setattr(
                module,
                "parse_positive_decimal_text",
                reject_toml_last_transaction_lt,
            )
            try:
                module._require_toml_account_metadata(args, output="toml")
            except ValueError as exc:
                rendered = str(exc)
                assert rendered == "--toml requires --last-transaction-lt"
                assert "secret-token" not in rendered
                assert forbidden_detail not in rendered
                assert exc.__cause__ is None
                assert exc.__suppress_context__ is True
            else:
                raise AssertionError("TON TOML LT helper detail leaked")


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
        ("route_allowlist_hash", "verifier_code_hash"),
        ("expected_destination_binding_hash", "source_verifier_material_hash"),
        ("expected_destination_binding_hash", "source_adapter_engine_deployment_hash"),
        ("source_verifier_material_hash", "verifier_code_hash"),
        ("source_adapter_engine_deployment_hash", "verifier_code_hash"),
        ("account_state_hash", "source_verifier_material_hash"),
        ("account_state_hash", "verifier_code_hash"),
        ("last_transaction_hash", "source_adapter_engine_deployment_hash"),
        ("last_transaction_hash", "verifier_code_hash"),
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

    assert module.main([*common_args, "--verifier-code-boc-hex", "0x" + TON_CODE_BOC_HEX]) == 0
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
                "0x" + TON_CODE_BOC_HEX,
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
        assert (
            exc.__suppress_context__ is True
        ), "TON destination code BoC metadata context leaked"
    else:
        raise AssertionError("invalid copied TON code BoC base64 evidence was accepted")


def test_ton_toml_code_boc_base64_reparse_redacts_helper_failures(
    monkeypatch,
):
    module = load_evidence_module()
    failure_cases = (
        (
            SystemExit,
            "secret-token {label} copied SystemExit detail",
            "copied SystemExit detail",
        ),
        (
            RuntimeError,
            "secret-token {label} copied RuntimeError detail",
            "copied RuntimeError detail",
        ),
        (
            TypeError,
            "secret-token {label} copied parser detail",
            "copied parser detail",
        ),
        (
            ValueError,
            "secret-token {label} copied ValueError detail",
            "copied ValueError detail",
        ),
    )

    for exception_type, secret_template, forbidden_detail in failure_cases:
        args = SimpleNamespace(
            verifier_code_hash=bytes.fromhex(TON_CODE_BOC_ROOT_HASH),
            verifier_code_boc_root_hash=bytes.fromhex(TON_CODE_BOC_ROOT_HASH),
            verifier_code_boc_hash_matches=True,
            verifier_code_boc_base64_text=TON_CODE_BOC_BASE64,
        )
        with monkeypatch.context() as patch:

            def reject_code_boc_base64(
                _value,
                *,
                label,
                exception_type=exception_type,
                secret_template=secret_template,
            ):
                raise exception_type(secret_template.format(label=label))

            patch.setattr(
                module,
                "parse_code_boc_base64",
                reject_code_boc_base64,
            )
            try:
                module._require_code_boc_root_metadata(args, output="toml")
            except ValueError as exc:
                rendered = str(exc)
                assert rendered == "--toml has invalid verifier code BoC base64 evidence"
                assert "secret-token" not in rendered
                assert forbidden_detail not in rendered
                assert exc.__cause__ is None
                assert exc.__suppress_context__ is True
            else:
                raise AssertionError(
                    f"copied TON code BoC base64 {exception_type.__name__} was accepted"
                )


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
        "0x" + TON_CODE_BOC_HEX,
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
