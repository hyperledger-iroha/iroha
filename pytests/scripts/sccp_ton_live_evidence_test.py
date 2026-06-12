import base64
import hashlib
import json
import urllib.parse
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
    "386d9b0be7366a993a10a6a148341726ea37d3ea018f203c5441a840b98d2d39"
)
TON_CODE_BOC_HEX = "b5ee9c720101020100070001020101000202"
TON_CODE_BOC_CRC32C_HEX = "b5ee9c724101020100070001020101000202be1c1df5"
TON_CODE_BOC_ROOT_HASH = (
    "49725ad44ef5ed5feaa27f88679cabae427209a6bea318cb9b66030131aae6fe"
)


def load_live_module():
    script_path = (
        Path(__file__).resolve().parents[2] / "scripts" / "sccp_ton_live_evidence.py"
    )
    spec = spec_from_file_location("sccp_ton_live_evidence", script_path)
    module = module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


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
        return b"secret-token-ton-error" * size

    def close(self):
        return None


def fake_ton_opener(
    module,
    *,
    status="active",
    code_hash=None,
    code_hash_text=None,
    code_boc_text=None,
    account_state_hash_text=None,
    last_transaction_lt_text="123456",
    last_transaction_hash_text=None,
    account_address_text=TON_VERIFIER_CONTRACT_ADDRESS,
    api_key=None,
):
    code_hash = code_hash or bytes.fromhex(TON_CODE_BOC_ROOT_HASH)
    account_state_hash = bytes.fromhex("55" * 32)
    last_transaction_hash = bytes.fromhex("66" * 32)

    def opener(request, timeout):
        assert timeout == 3.0
        parsed = urllib.parse.urlparse(request.full_url)
        assert parsed.path.endswith("/api/v3/accountStates")
        query = urllib.parse.parse_qs(parsed.query)
        assert query["address"] == [TON_VERIFIER_CONTRACT_ADDRESS]
        assert query["include_boc"] == ["true"]
        headers = {key.lower(): value for key, value in request.header_items()}
        if api_key is None:
            assert "x-api-key" not in headers
        else:
            assert headers["x-api-key"] == api_key
        return FakeResponse(
            {
                "accounts": [
                    {
                        "address": account_address_text,
                        "status": status,
                        "code_hash": code_hash_text
                        if code_hash_text is not None
                        else base64.b64encode(code_hash).decode("ascii"),
                        "code_boc": code_boc_text
                        if code_boc_text is not None
                        else base64.b64encode(
                            bytes.fromhex(TON_CODE_BOC_HEX)
                        ).decode("ascii"),
                        "account_state_hash": account_state_hash_text
                        if account_state_hash_text is not None
                        else "0x" + account_state_hash.hex(),
                        "last_transaction_lt": last_transaction_lt_text,
                        "last_transaction_hash": last_transaction_hash_text
                        if last_transaction_hash_text is not None
                        else base64.b64encode(last_transaction_hash).decode("ascii"),
                    }
                ]
            }
        )

    return SimpleNamespace(
        opener=opener,
        code_hash=code_hash,
        account_state_hash=account_state_hash,
        last_transaction_hash=last_transaction_hash,
    )


def test_live_ton_api_url_rejects_hidden_request_state():
    module = load_live_module()

    assert module._account_states_url("https://toncenter.example") == (
        "https://toncenter.example/api/v3/accountStates"
    )
    assert module._account_states_url(
        "https://toncenter.example/api/v3/accountStates"
    ) == "https://toncenter.example/api/v3/accountStates"

    for api_url, expected_error in (
        ("https://token@toncenter.example", "credentials"),
        ("https://toncenter.example/root;param", "params, query, or fragment"),
        ("https://toncenter.example?api_key=secret", "params, query, or fragment"),
        ("https://toncenter.example#fragment", "params, query, or fragment"),
    ):
        try:
            module._account_states_url(api_url)
        except ValueError as exc:
            assert expected_error in str(exc)
        else:
            raise AssertionError(f"hidden TON API URL state {api_url!r} was accepted")


def test_live_ton_account_states_response_size_is_bounded():
    module = load_live_module()

    def oversized_opener(_request, timeout):
        assert timeout == 3.0
        return OversizedResponse()

    try:
        module.collect_live_evidence(
            "https://toncenter.example",
            verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
            opener=oversized_opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        assert "response exceeds" in str(exc)
    else:
        raise AssertionError("oversized TON accountStates response was accepted")


def test_live_ton_http_error_detail_is_bounded():
    module = load_live_module()

    def failing_opener(request, timeout):
        assert timeout == 3.0
        raise module.urllib.error.HTTPError(
            request.full_url,
            500,
            "boom",
            {},
            OversizedErrorBody(),
        )

    try:
        module.collect_live_evidence(
            "https://toncenter.example",
            verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
            opener=failing_opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        message = str(exc)
        assert message == "TON accountStates failed with HTTP 500"
        assert "secret-token" not in message
        assert len(message) < 100
    else:
        raise AssertionError("oversized TON accountStates error body was accepted")


def test_live_ton_account_states_json_rejects_duplicate_keys():
    module = load_live_module()
    duplicate_payload = (
        '{"accounts":[{"address":"'
        + TON_VERIFIER_CONTRACT_ADDRESS
        + '","secret-token-status":"active","secret-token-status":"uninit",'
        + '"status":"active"}]}'
    ).encode("utf-8")

    def duplicate_json_opener(_request, timeout):
        assert timeout == 3.0
        return RawResponse(duplicate_payload)

    try:
        module.collect_live_evidence(
            "https://toncenter.example",
            verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
            opener=duplicate_json_opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        message = str(exc)
        assert message == "TON accountStates returned duplicate JSON keys"
        assert "secret-token" not in message
        assert "duplicate JSON key " not in message
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("duplicate-key TON accountStates JSON was accepted")


def test_live_ton_account_states_redacts_transport_and_error_response_details():
    module = load_live_module()

    def secret_url_error_opener(_request, timeout):
        assert timeout == 3.0
        raise module.urllib.error.URLError(
            "secret-token provider URL leaked from transport"
        )

    def secret_error_object_opener(_request, timeout):
        assert timeout == 3.0
        return FakeResponse(
            {
                "error": {
                    "code": 429,
                    "message": "secret-token TON Center error object",
                }
            }
        )

    try:
        module.collect_live_evidence(
            "https://toncenter.example",
            verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
            opener=secret_url_error_opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        message = str(exc)
        assert message == "TON accountStates request failed"
        assert "secret-token" not in message
    else:
        raise AssertionError("secret-bearing TON transport error was accepted")

    try:
        module.collect_live_evidence(
            "https://toncenter.example",
            verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
            opener=secret_error_object_opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        message = str(exc)
        assert message == "TON accountStates returned error response"
        assert "secret-token" not in message
        assert "error object" not in message
    else:
        raise AssertionError("secret-bearing TON accountStates error was accepted")


def test_ton_live_cli_redacts_top_level_exception_details(monkeypatch, capsys):
    module = load_live_module()

    def fail_collect(*_args, **_kwargs):
        raise RuntimeError("secret-token /tmp/operator/private-path")

    monkeypatch.setattr(module, "collect_live_evidence", fail_collect)

    try:
        module.main(
            [
                "--api-url",
                "https://toncenter.example",
                "--verifier-contract-address",
                TON_VERIFIER_CONTRACT_ADDRESS,
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("TON live CLI accepted top-level collection failure")

    captured = capsys.readouterr()
    assert "SCCP TON live evidence collection failed" in captured.err
    assert "secret-token" not in captured.err
    assert "private-path" not in captured.err


def test_live_ton_last_transaction_lt_requires_canonical_ascii_decimal():
    module = load_live_module()

    assert module._positive_decimal("123456", label="last transaction LT") == "123456"

    for value in ("0", "0123456", "+123456", "١٢٣٤٥٦"):
        try:
            module._positive_decimal(value, label="last transaction LT")
        except RuntimeError as exc:
            assert "decimal string" in str(exc) or "positive" in str(exc)
        else:
            raise AssertionError(f"noncanonical TON LT {value!r} was accepted")

        fake = fake_ton_opener(module, last_transaction_lt_text=value)
        try:
            module.collect_live_evidence(
                "https://toncenter.example",
                verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
                opener=fake.opener,
                timeout=3.0,
            )
        except RuntimeError as exc:
            assert "last_transaction_lt" in str(exc)
        else:
            raise AssertionError(f"noncanonical live TON LT {value!r} was accepted")


def live_args(module, *, code_hash, account_state_hash):
    return SimpleNamespace(
        route_allowlist_hash=bytes.fromhex(TON_ROUTE_ALLOWLIST_HASH_VECTOR),
        route_canary_evidence_hash=bytes.fromhex(TON_ROUTE_CANARY_EVIDENCE_HASH),
        source_verifier_material_hash=bytes.fromhex(
            TON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
        ),
        source_adapter_engine_deployment_hash=bytes.fromhex(
            TON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR
        ),
        expected_destination_binding_hash=bytes.fromhex(TON_DESTINATION_BINDING_VECTOR),
        expected_verifier_code_hash=code_hash,
        expected_account_state_hash=account_state_hash,
    )


def test_live_ton_evidence_collects_account_state_and_toml():
    module = load_live_module()
    fake = fake_ton_opener(module)
    live = module.collect_live_evidence(
        "https://toncenter.example",
        verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
        opener=fake.opener,
        timeout=3.0,
    )

    assert live["verifier_contract_address"] == TON_VERIFIER_CONTRACT_ADDRESS
    assert live["account_status"] == "active"
    assert live["account_state_hash"] == "0x" + fake.account_state_hash.hex()
    assert live["last_transaction_lt"] == "123456"
    assert live["last_transaction_hash"] == "0x" + fake.last_transaction_hash.hex()
    assert live["code_boc_present"] is True
    assert live["code_boc_base64"] == base64.b64encode(
        bytes.fromhex(TON_CODE_BOC_HEX)
    ).decode("ascii")
    assert live["code_boc_root_hash"] == "0x" + fake.code_hash.hex()
    assert live["code_boc_hash_matches"] is True
    assert live["verifier_code_hash"] == "0x" + fake.code_hash.hex()

    args = live_args(
        module,
        code_hash=fake.code_hash,
        account_state_hash=fake.account_state_hash,
    )
    summary = module._summary(args, live)
    assert summary["expected_verifier_code_hash_matches"] is True
    assert summary["expected_account_state_hash_matches"] is True
    assert summary["code_boc_root_hash"] == "0x" + fake.code_hash.hex()
    assert summary["code_boc_hash_matches"] is True
    assert summary["destination_toml_ready"] is True
    assert summary["full_toml_ready"] is True
    assert summary["toml_ready"] is True
    assert summary["offline_evidence_args"] == [
        "--verifier-contract-address",
        TON_VERIFIER_CONTRACT_ADDRESS,
        "--verifier-code-hash",
        "0x" + fake.code_hash.hex(),
        "--verifier-code-boc-base64",
        live["code_boc_base64"],
        "--account-status",
        "active",
        "--account-state-hash",
        "0x" + fake.account_state_hash.hex(),
        "--last-transaction-lt",
        "123456",
        "--last-transaction-hash",
        "0x" + fake.last_transaction_hash.hex(),
        "--expected-destination-binding-hash",
        "0x" + TON_DESTINATION_BINDING_VECTOR,
        "--route-allowlist-hash",
        "0x" + TON_ROUTE_ALLOWLIST_HASH_VECTOR,
        "--source-verifier-material-hash",
        "0x" + TON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR,
        "--source-adapter-engine-deployment-hash",
        "0x" + TON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR,
        "--route-canary-evidence-hash",
        "0x" + TON_ROUTE_CANARY_EVIDENCE_HASH,
    ]
    offline_toml = module.evidence.render_toml(
        module._destination_args_from_live(args, live),
        module.evidence.ton_destination_binding_hash(),
    )
    assert summary["offline_toml_sha256"] == hashlib.sha256(
        offline_toml.encode("utf-8")
    ).hexdigest()
    assert summary["route_canary"]["evidence_hash"] == (
        "0x" + TON_ROUTE_CANARY_EVIDENCE_HASH
    )

    rendered = module.render_toml(args, live)
    assert '# sccp_ton_account_status = "active"' in rendered
    assert (
        '# sccp_ton_account_state_hash = "0x'
        + fake.account_state_hash.hex()
        + '"'
        in rendered
    )
    assert '# sccp_ton_last_transaction_lt = "123456"' in rendered
    assert (
        '# sccp_ton_last_transaction_hash = "0x'
        + fake.last_transaction_hash.hex()
        + '"'
        in rendered
    )
    assert '# sccp_ton_code_hash = "0x' + fake.code_hash.hex() + '"' in rendered
    assert (
        '# sccp_ton_code_boc_root_hash = "0x'
        + fake.code_hash.hex()
        + '"'
        in rendered
    )
    assert '# sccp_ton_code_boc_base64 = "' + live["code_boc_base64"] + '"' in rendered
    assert '# sccp_ton_code_boc_hash_matches = "true"' in rendered
    assert 'ton_account_status = "active"' in rendered
    assert (
        'ton_account_state_hash = "0x'
        + fake.account_state_hash.hex()
        + '"'
        in rendered
    )
    assert 'ton_last_transaction_lt = "123456"' in rendered
    assert (
        'ton_last_transaction_hash = "0x'
        + fake.last_transaction_hash.hex()
        + '"'
        in rendered
    )
    assert 'ton_verifier_code_boc_root_hash = "0x' + fake.code_hash.hex() + '"' in rendered
    assert 'ton_verifier_code_boc = "0x' + TON_CODE_BOC_HEX + '"' in rendered
    assert rendered.count("# sccp_ton_account_status") == 1
    assert rendered.count("# sccp_ton_account_state_hash") == 1
    assert rendered.count("# sccp_ton_last_transaction_lt") == 1
    assert rendered.count("# sccp_ton_last_transaction_hash") == 1
    assert rendered.count("# sccp_ton_code_hash") == 1
    assert rendered.count("# sccp_ton_code_boc_root_hash") == 1
    assert rendered.count("# sccp_ton_code_boc_base64") == 1
    assert rendered.count("# sccp_ton_code_boc_hash_matches") == 1
    assert "[[zk.sccp_destination_rollouts]]" in rendered
    assert "[[zk.sccp_route_allowlists]]" in rendered
    assert '# sccp_route_canary_status = "passed"' in rendered
    assert (
        '# sccp_route_canary_evidence_hash = "0x'
        + TON_ROUTE_CANARY_EVIDENCE_HASH
        + '"'
        in rendered
    )


def test_live_ton_evidence_normalizes_collected_code_boc_base64():
    module = load_live_module()
    code_boc = bytes.fromhex(TON_CODE_BOC_CRC32C_HEX)
    stripped_padding = base64.b64encode(code_boc).decode("ascii").rstrip("=")
    fake = fake_ton_opener(module, code_boc_text=stripped_padding)

    live = module.collect_live_evidence(
        "https://toncenter.example",
        verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
        opener=fake.opener,
        timeout=3.0,
    )

    assert live["code_boc_base64"] == base64.b64encode(code_boc).decode("ascii")


def test_live_ton_evidence_rejects_inactive_account():
    module = load_live_module()
    fake = fake_ton_opener(module, status="uninit")

    try:
        module.collect_live_evidence(
            "https://toncenter.example",
            verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
            opener=fake.opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        assert "must be active" in str(exc)
    else:
        raise AssertionError("inactive TON verifier account was accepted")


def test_live_ton_evidence_rejects_mismatched_account_address():
    module = load_live_module()
    cases = [
        ("", "must be present"),
        (" " + TON_VERIFIER_CONTRACT_ADDRESS + " ", "must not contain whitespace"),
        ("-1:" + "22" * 32, "basechain 0"),
        ("0:" + "22" * 32, "does not match verifier contract"),
        ("0:" + "AA" * 32, "canonical raw address"),
    ]

    for account_address, expected_error in cases:
        fake = fake_ton_opener(module, account_address_text=account_address)
        try:
            module.collect_live_evidence(
                "https://toncenter.example",
                verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
                opener=fake.opener,
                timeout=3.0,
            )
        except RuntimeError as exc:
            assert expected_error in str(exc)
        else:
            raise AssertionError(f"TON account address {account_address!r} was accepted")


def test_live_ton_evidence_rejects_padded_remote_hash_text():
    module = load_live_module()
    valid_code_hash = base64.b64encode(bytes.fromhex(TON_CODE_BOC_ROOT_HASH)).decode(
        "ascii"
    )
    fake = fake_ton_opener(module, code_hash_text=" " + valid_code_hash + " ")

    try:
        module.collect_live_evidence(
            "https://toncenter.example",
            verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
            opener=fake.opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        assert "code_hash must not contain whitespace" in str(exc)
    else:
        raise AssertionError("padded live TON code_hash was accepted")


def test_live_ton_evidence_rejects_noncanonical_remote_hash_base64():
    module = load_live_module()

    for label, kwargs in (
        (
            "code_hash",
            {
                "code_hash_text": noncanonical_base64_alias(
                    bytes.fromhex(TON_CODE_BOC_ROOT_HASH)
                )
            },
        ),
        (
            "last_transaction_hash",
            {
                "last_transaction_hash_text": noncanonical_base64_alias(
                    bytes.fromhex("66" * 32)
                )
            },
        ),
    ):
        fake = fake_ton_opener(module, **kwargs)
        try:
            module.collect_live_evidence(
                "https://toncenter.example",
                verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
                opener=fake.opener,
                timeout=3.0,
            )
        except RuntimeError as exc:
            assert label in str(exc)
            assert "canonical base64" in str(exc)
        else:
            raise AssertionError(f"noncanonical live TON {label} was accepted")


def test_live_ton_evidence_rejects_padded_code_boc_text():
    module = load_live_module()
    valid_code_boc = base64.b64encode(bytes.fromhex(TON_CODE_BOC_HEX)).decode("ascii")
    fake = fake_ton_opener(module, code_boc_text=" " + valid_code_boc)

    try:
        module.collect_live_evidence(
            "https://toncenter.example",
            verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
            opener=fake.opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        assert "code_boc" in str(exc)
    else:
        raise AssertionError("padded live TON code_boc was accepted")

    live = module.collect_live_evidence(
        "https://toncenter.example",
        verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
        opener=fake_ton_opener(module).opener,
        timeout=3.0,
    )
    live["code_boc_base64"] = live["code_boc_base64"] + " "
    args = live_args(
        module,
        code_hash=bytes.fromhex(TON_CODE_BOC_ROOT_HASH),
        account_state_hash=bytes.fromhex("55" * 32),
    )
    try:
        module._summary(args, live)
    except ValueError as exc:
        assert "code_boc_base64" in str(exc)
    else:
        raise AssertionError("padded imported TON code_boc_base64 was accepted")


def test_live_ton_evidence_redacts_code_boc_parser_failures(monkeypatch):
    """TON code BoC parser failures must not echo parser exception payloads."""

    module = load_live_module()
    live = module.collect_live_evidence(
        "https://toncenter.example",
        verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
        opener=fake_ton_opener(module).opener,
        timeout=3.0,
    )
    args = live_args(
        module,
        code_hash=bytes.fromhex(TON_CODE_BOC_ROOT_HASH),
        account_state_hash=bytes.fromhex("55" * 32),
    )

    def fail_code_boc(_value, *, label):
        raise ValueError(f"secret-token {label} parser detail")

    monkeypatch.setattr(module.evidence, "parse_code_boc_base64", fail_code_boc)

    try:
        module.collect_live_evidence(
            "https://toncenter.example",
            verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
            opener=fake_ton_opener(module).opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        live_error = str(exc)
    else:
        raise AssertionError("TON live collection accepted parser failure")

    try:
        module._summary(args, live)
    except ValueError as exc:
        summary_error = str(exc)
    else:
        raise AssertionError("TON live summary accepted parser failure")

    rendered = "\n".join((live_error, summary_error))
    assert live_error == "TON verifier account code_boc is invalid"
    assert summary_error == "TON live code BoC base64 metadata is invalid"
    assert "secret-token" not in rendered
    assert "parser detail" not in rendered
    assert "ValueError" not in rendered
    assert "is invalid:" not in rendered


def test_live_ton_evidence_redacts_account_address_parser_failures(monkeypatch):
    """TON accountStates address parser failures must not echo parser payloads."""

    module = load_live_module()
    normalize = module.evidence.normalize_ton_raw_address

    def fail_account_address(value, *, label):
        if label == "accountStates account address":
            raise module.argparse.ArgumentTypeError(
                f"secret-token {label} parser detail"
            )
        return normalize(value, label=label)

    monkeypatch.setattr(
        module.evidence,
        "normalize_ton_raw_address",
        fail_account_address,
    )

    try:
        module.collect_live_evidence(
            "https://toncenter.example",
            verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
            opener=fake_ton_opener(module).opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        rendered = str(exc)
    else:
        raise AssertionError("TON live collection accepted parser failure")

    assert rendered == "TON accountStates account address must be a canonical raw address"
    assert "secret-token" not in rendered
    assert "parser detail" not in rendered
    assert "canonical raw address:" not in rendered


def test_live_ton_evidence_redacts_imported_parser_failures(monkeypatch):
    """Imported TON live parser failures must not echo parser payloads."""

    module = load_live_module()
    fake = fake_ton_opener(module)
    live = module.collect_live_evidence(
        "https://toncenter.example",
        verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
        opener=fake.opener,
        timeout=3.0,
    )
    args = live_args(
        module,
        code_hash=fake.code_hash,
        account_state_hash=fake.account_state_hash,
    )

    original_normalize = module.evidence.normalize_ton_raw_address
    with monkeypatch.context() as patch:
        def fail_account_address(value, *, label):
            if label == "account address":
                raise module.argparse.ArgumentTypeError(
                    "secret-token account address parser detail"
                )
            return original_normalize(value, label=label)

        patch.setattr(module.evidence, "normalize_ton_raw_address", fail_account_address)
        try:
            module._summary(args, live)
        except ValueError as exc:
            rendered = str(exc)
            assert rendered == "TON live account address metadata is invalid"
            assert "secret-token" not in rendered
            assert "parser detail" not in rendered
            assert exc.__cause__ is None
            assert exc.__suppress_context__ is True
        else:
            raise AssertionError("TON live summary leaked account parser detail")

    with monkeypatch.context() as patch:
        def fail_last_transaction_lt(_value, *, label):
            raise RuntimeError(f"secret-token {label} parser detail")

        patch.setattr(module, "_positive_decimal", fail_last_transaction_lt)
        try:
            module._summary(args, live)
        except ValueError as exc:
            rendered = str(exc)
            assert rendered == "TON live last_transaction_lt metadata is invalid"
            assert "secret-token" not in rendered
            assert "parser detail" not in rendered
            assert exc.__cause__ is None
            assert exc.__suppress_context__ is True
        else:
            raise AssertionError("TON live summary leaked LT parser detail")


def test_live_ton_evidence_rejects_code_hash_drift():
    module = load_live_module()
    fake = fake_ton_opener(module)
    live = module.collect_live_evidence(
        "https://toncenter.example",
        verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
        opener=fake.opener,
        timeout=3.0,
    )
    args = live_args(
        module,
        code_hash=bytes.fromhex("cc" * 32),
        account_state_hash=fake.account_state_hash,
    )

    try:
        module._summary(args, live)
    except ValueError as exc:
        assert "--expected-verifier-code-hash" in str(exc)
    else:
        raise AssertionError("drifted TON verifier code hash pin was accepted")


def test_live_ton_evidence_rejects_code_boc_hash_drift():
    module = load_live_module()
    fake = fake_ton_opener(module, code_hash=bytes.fromhex("bb" * 32))

    try:
        module.collect_live_evidence(
            "https://toncenter.example",
            verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
            opener=fake.opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        assert "code_boc root hash does not match code_hash" in str(exc)
    else:
        raise AssertionError("TON code_hash drift from code_boc was accepted")


def test_live_ton_direct_api_rejects_forged_live_metadata():
    module = load_live_module()
    fake = fake_ton_opener(module)
    live = module.collect_live_evidence(
        "https://toncenter.example",
        verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
        opener=fake.opener,
        timeout=3.0,
    )
    args = live_args(
        module,
        code_hash=fake.code_hash,
        account_state_hash=fake.account_state_hash,
    )

    for field, forged_value, expected_message in (
        ("account_address", "0:" + "22" * 32, "account address"),
        ("account_status", "uninit", "status"),
        ("code_boc_present", False, "presence"),
        ("code_boc_hash_matches", False, "hash match"),
        ("code_boc_root_hash", "0x" + "bb" * 32, "root hash"),
        ("code_boc_base64", " " + live["code_boc_base64"], "exact"),
    ):
        forged = dict(live)
        forged[field] = forged_value
        try:
            module._summary(args, forged)
        except ValueError as exc:
            assert expected_message in str(exc)
        else:
            raise AssertionError(f"TON live summary accepted forged {field}")

    forged = dict(live)
    forged["account_status"] = "uninit"
    try:
        module.render_toml(args, forged)
    except ValueError as exc:
        assert "status" in str(exc)
    else:
        raise AssertionError("TON live TOML accepted forged account status")


def test_live_ton_summary_requires_boolean_destination_readiness(monkeypatch):
    module = load_live_module()
    fake = fake_ton_opener(module)
    live = module.collect_live_evidence(
        "https://toncenter.example",
        verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
        opener=fake.opener,
        timeout=3.0,
    )
    args = live_args(
        module,
        code_hash=fake.code_hash,
        account_state_hash=fake.account_state_hash,
    )
    original_summary = module.evidence._json_summary

    def malformed_summary(destination_args, destination_binding_hash, expected_matches):
        summary = original_summary(
            destination_args,
            destination_binding_hash,
            expected_matches,
        )
        summary["toml_ready"] = "true"
        return summary

    monkeypatch.setattr(module.evidence, "_json_summary", malformed_summary)

    summary = module._summary(args, live)
    assert summary["destination_toml_ready"] is False
    assert summary["full_toml_ready"] is False
    assert summary["toml_ready"] is False
    assert "offline_toml_sha256" not in summary


def test_live_ton_evidence_rejects_malformed_remote_hash_text():
    module = load_live_module()
    valid_code_hash = base64.b64encode(bytes.fromhex("bb" * 32)).decode("ascii")
    cases = [
        ("code_hash", {"code_hash_text": valid_code_hash + "!"}),
        ("code_boc", {"code_boc_text": valid_code_hash + "!"}),
        ("account_state_hash", {"account_state_hash_text": "0x" + "55" * 31 + "zz"}),
        (
            "last_transaction_hash",
            {"last_transaction_hash_text": valid_code_hash[:8] + "!" + valid_code_hash[8:]},
        ),
    ]

    for label, kwargs in cases:
        fake = fake_ton_opener(module, **kwargs)
        try:
            module.collect_live_evidence(
                "https://toncenter.example",
                verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
                opener=fake.opener,
                timeout=3.0,
            )
        except RuntimeError as exc:
            assert label in str(exc)
            if label == "code_boc":
                assert str(exc) == "TON verifier account code_boc is invalid"
                assert "base64" not in str(exc)
            else:
                assert "base64" in str(exc) or "32-byte hex" in str(exc)
        else:
            raise AssertionError(f"malformed TON {label} was accepted")


def test_live_ton_evidence_requires_pins_for_toml():
    module = load_live_module()
    fake = fake_ton_opener(module)
    live = module.collect_live_evidence(
        "https://toncenter.example",
        verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
        opener=fake.opener,
        timeout=3.0,
    )
    args = live_args(
        module,
        code_hash=fake.code_hash,
        account_state_hash=fake.account_state_hash,
    )

    args.expected_verifier_code_hash = None
    summary = module._summary(args, live)
    assert summary["destination_toml_ready"] is True
    assert summary["full_toml_ready"] is False
    assert summary["toml_ready"] is False
    try:
        module.render_toml(args, live)
    except ValueError as exc:
        assert "--expected-verifier-code-hash" in str(exc)
    else:
        raise AssertionError("TON live TOML accepted without a code hash pin")

    args = live_args(
        module,
        code_hash=fake.code_hash,
        account_state_hash=fake.account_state_hash,
    )
    args.expected_account_state_hash = None
    summary = module._summary(args, live)
    assert summary["destination_toml_ready"] is True
    assert summary["full_toml_ready"] is False
    assert summary["toml_ready"] is False
    try:
        module.render_toml(args, live)
    except ValueError as exc:
        assert "--expected-account-state-hash" in str(exc)
    else:
        raise AssertionError("TON live TOML accepted without an account-state pin")

    args = live_args(
        module,
        code_hash=fake.code_hash,
        account_state_hash=fake.account_state_hash,
    )
    args.route_canary_evidence_hash = None
    summary = module._summary(args, live)
    assert summary["destination_toml_ready"] is False
    assert summary["full_toml_ready"] is False
    assert summary["toml_ready"] is False
    try:
        module.render_toml(args, live)
    except ValueError as exc:
        assert "--route-canary-evidence-hash" in str(exc)
    else:
        raise AssertionError("TON live TOML accepted without route canary evidence")


def test_live_ton_evidence_uses_runtime_only_api_key(tmp_path):
    module = load_live_module()
    assert module._read_api_key(
        SimpleNamespace(api_key="secret-token", api_key_file=None)
    ) == "secret-token"

    token_file = tmp_path / "toncenter.key"
    token_file.write_text("secret-token\n", encoding="ascii")
    assert module._read_api_key(
        SimpleNamespace(api_key=None, api_key_file=str(token_file))
    ) == "secret-token"

    for api_key in (
        " secret-token",
        "secret-token ",
        "secret token",
        "secret\nX",
        "sëcret",
    ):
        try:
            module._read_api_key(SimpleNamespace(api_key=api_key, api_key_file=None))
        except ValueError as exc:
            assert "--api-key" in str(exc)
        else:
            raise AssertionError(f"non-exact TON API key {api_key!r} was accepted")

    padded_token_file = tmp_path / "padded-toncenter.key"
    padded_token_file.write_text(" secret-token\n", encoding="ascii")
    try:
        module._read_api_key(
            SimpleNamespace(api_key=None, api_key_file=str(padded_token_file))
        )
    except ValueError as exc:
        assert "--api-key-file" in str(exc)
    else:
        raise AssertionError("padded TON API key file token was accepted")

    fake = fake_ton_opener(module, api_key="secret-token")
    live = module.collect_live_evidence(
        "https://toncenter.example/api/v3/accountStates",
        verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
        api_key="secret-token",
        opener=fake.opener,
        timeout=3.0,
    )

    assert live["verifier_code_hash"] == "0x" + fake.code_hash.hex()

    try:
        module.collect_live_evidence(
            "https://toncenter.example/api/v3/accountStates",
            verifier_contract_address=TON_VERIFIER_CONTRACT_ADDRESS,
            api_key=" secret-token",
            opener=fake.opener,
            timeout=3.0,
        )
    except ValueError as exc:
        assert "api_key" in str(exc)
    else:
        raise AssertionError("non-exact direct TON API key was accepted")
