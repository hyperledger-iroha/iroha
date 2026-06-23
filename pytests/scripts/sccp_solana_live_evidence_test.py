import base64
import hashlib
import json
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
from types import SimpleNamespace


SOLANA_DESTINATION_BINDING_VECTOR = (
    "078578f0aa27daa2972d6c19d1d26dbb6bf6ba1e8df84e283d7ef101fc46abf6"
)
SOURCE_VERIFIER_MATERIAL_HASH = "aa" * 32
SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH = "99" * 32
SOLANA_ROUTE_ALLOWLIST_HASH_VECTOR = (
    "c23e048cdfabc169c3567c201f31869efa4dbcac6478f6f80b31bfe410c64a34"
)
SOLANA_VERIFIER_PROGRAM_BYTES = bytes.fromhex("7f454c460102030405")


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


def _default_program_id(module):
    return module._encode_solana_base58(bytes.fromhex("33" * 32))


def load_live_module():
    script_path = (
        Path(__file__).resolve().parents[2] / "scripts" / "sccp_solana_live_evidence.py"
    )
    spec = spec_from_file_location("sccp_solana_live_evidence", script_path)
    module = module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


class FakeResponse:
    def __init__(self, payload):
        self.payload = payload

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, traceback):
        return False

    def read(self, size=-1):
        payload = json.dumps(self.payload).encode("utf-8")
        if size is None or size < 0:
            return payload
        return payload[:size]


class RawResponse:
    def __init__(self, payload):
        self.payload = payload

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, traceback):
        return False

    def read(self, size=-1):
        if size is None or size < 0:
            return self.payload
        return self.payload[:size]


class OversizedResponse:
    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, traceback):
        return False

    def read(self, size=-1):
        if size is None or size < 0:
            size = 1024 * 1024 + 1
        return b"{" * size


class OversizedErrorBody:
    def read(self, size=-1):
        if size is None or size < 0:
            size = 4097
        return b"secret-token-solana-error" * size

    def close(self):
        return None


def _program_account_data(module, programdata_raw):
    return module.UPGRADEABLE_LOADER_PROGRAM_TAG.to_bytes(4, "little") + programdata_raw


def _programdata_account_data(module, *, slot, program_bytes, authority=None):
    data = bytearray()
    data.extend(module.UPGRADEABLE_LOADER_PROGRAMDATA_TAG.to_bytes(4, "little"))
    data.extend(slot.to_bytes(8, "little"))
    if authority is None:
        data.append(0)
        data.extend(bytes(32))
    else:
        data.append(1)
        data.extend(authority)
    data.extend(program_bytes)
    return bytes(data)


def _account_payload(module, data, *, executable):
    return {
        "owner": module.UPGRADEABLE_LOADER_ID,
        "executable": executable,
        "data": [base64.b64encode(data).decode("ascii"), "base64"],
    }


def _fake_solana_rpc(
    module,
    *,
    program_id,
    programdata_address,
    programdata_data,
    context_slot=9000,
    program_context_slot=None,
    programdata_context_slot=None,
):
    if program_context_slot is None:
        program_context_slot = context_slot
    if programdata_context_slot is None:
        programdata_context_slot = context_slot
    programdata_raw = bytes.fromhex("11" * 32)
    accounts = {
        program_id: _account_payload(
            module,
            _program_account_data(module, programdata_raw),
            executable=True,
        ),
        programdata_address: _account_payload(
            module,
            programdata_data,
            executable=False,
        ),
    }

    def opener(request, timeout):
        assert timeout == 3.0
        body = json.loads(request.data.decode("utf-8"))
        assert body["method"] == "getAccountInfo"
        address = body["params"][0]
        account = accounts[address]
        slot = (
            program_context_slot
            if address == program_id
            else programdata_context_slot
        )
        return FakeResponse(
            {
                "jsonrpc": "2.0",
                "id": body["id"],
                "result": {"context": {"slot": slot}, "value": account},
            }
        )

    return opener


def test_solana_json_rpc_response_size_is_bounded():
    module = load_live_module()

    def oversized_opener(_request, timeout):
        assert timeout == 3.0
        return OversizedResponse()

    try:
        module._json_rpc(
            "https://solana.example.invalid",
            "getAccountInfo",
            [],
            opener=oversized_opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        assert "response exceeds" in str(exc)
    else:
        raise AssertionError("oversized Solana JSON-RPC response was accepted")


def test_solana_json_rpc_http_error_detail_is_bounded():
    module = load_live_module()

    def failing_opener(request, timeout):
        assert timeout == 3.0
        raise module.urllib.error.HTTPError(
            request.full_url,
            429,
            "rate limited",
            {},
            OversizedErrorBody(),
        )

    try:
        module._json_rpc(
            "https://solana.example.invalid",
            "getAccountInfo",
            [],
            opener=failing_opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        message = str(exc)
        assert message == "JSON-RPC getAccountInfo failed with HTTP 429"
        assert "secret-token" not in message
        assert len(message) < 100
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("oversized Solana JSON-RPC error body was accepted")


def test_solana_json_rpc_redacts_invalid_json_parser_details():
    module = load_live_module()

    def invalid_json_opener(_request, timeout):
        assert timeout == 3.0
        return RawResponse(b'{"secret-token invalid Solana JSON-RPC payload": ')

    try:
        module._json_rpc(
            "https://solana.example.invalid",
            "getAccountInfo",
            [],
            opener=invalid_json_opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        message = str(exc)
        assert message == "JSON-RPC getAccountInfo returned invalid JSON"
        assert "secret-token" not in message
        assert "JSON-RPC payload" not in message
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("secret-bearing invalid Solana JSON-RPC was accepted")


def test_solana_json_rpc_rejects_duplicate_json_keys():
    module = load_live_module()
    duplicate_payload = (
        b'{"jsonrpc":"2.0","id":1,"result":{"context":{"slot":1},'
        b'"secret-token-value":null,"secret-token-value":{},"value":null}}'
    )

    def duplicate_json_opener(_request, timeout):
        assert timeout == 3.0
        return RawResponse(duplicate_payload)

    try:
        module._json_rpc(
            "https://solana.example.invalid",
            "getAccountInfo",
            [],
            opener=duplicate_json_opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        message = str(exc)
        assert message == "JSON-RPC getAccountInfo returned duplicate JSON keys"
        assert "secret-token" not in message
        assert "duplicate JSON key " not in message
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("duplicate-key Solana JSON-RPC response was accepted")


def test_solana_json_rpc_redacts_transport_and_error_response_details():
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
                "jsonrpc": "2.0",
                "id": 1,
                "error": {
                    "code": -32000,
                    "message": "secret-token Solana provider error object",
                },
            }
        )

    try:
        module._json_rpc(
            "https://solana.example.invalid",
            "getAccountInfo",
            [],
            opener=secret_url_error_opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        message = str(exc)
        assert message == "JSON-RPC getAccountInfo request failed"
        assert "secret-token" not in message
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("secret-bearing Solana transport error was accepted")

    try:
        module._json_rpc(
            "https://solana.example.invalid",
            "getAccountInfo",
            [],
            opener=secret_error_object_opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        message = str(exc)
        assert message == "JSON-RPC getAccountInfo returned error response"
        assert "secret-token" not in message
        assert "provider error object" not in message
    else:
        raise AssertionError("secret-bearing Solana JSON-RPC error was accepted")


def test_solana_live_cli_redacts_top_level_exception_details(monkeypatch, capsys):
    module = load_live_module()

    for exception_type in (OSError, RuntimeError, TypeError, ValueError):

        def fail_collect(*_args, exception_type=exception_type, **_kwargs):
            raise exception_type("secret-token /tmp/operator/private-path")

        with monkeypatch.context() as patch:
            patch.setattr(module, "collect_live_evidence", fail_collect)
            try:
                module.main(
                    [
                        "--rpc-url",
                        "https://solana.example.invalid",
                        "--verifier-program-id",
                        _default_program_id(module),
                    ]
                )
            except SystemExit as exc:
                assert exc.code == 2
            else:
                raise AssertionError(
                    "Solana live CLI accepted top-level collection failure"
                )

            captured = capsys.readouterr()
            assert "SCCP Solana live evidence collection failed" in captured.err
            assert "secret-token" not in captured.err
            assert "private-path" not in captured.err
            assert exception_type.__name__ not in captured.err


def _live_route_canary_hash(
    module,
    *,
    program_id,
    programdata_address,
    program_bytes=SOLANA_VERIFIER_PROGRAM_BYTES,
):
    return module.evidence.solana_route_canary_evidence_hash(
        route_allowlist_hash=bytes.fromhex(SOLANA_ROUTE_ALLOWLIST_HASH_VECTOR),
        destination_binding_hash=bytes.fromhex(SOLANA_DESTINATION_BINDING_VECTOR),
        source_verifier_material_hash=bytes.fromhex(SOURCE_VERIFIER_MATERIAL_HASH),
        source_adapter_engine_deployment_hash=bytes.fromhex(
            SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
        ),
        verifier_program_id=program_id,
        verifier_code_hash=module.evidence.solana_verifier_program_code_hash(
            program_bytes
        ),
        rpc_commitment="finalized",
        program_owner=module.UPGRADEABLE_LOADER_ID,
        programdata_owner=module.UPGRADEABLE_LOADER_ID,
        program_immutable=True,
        program_account_data=module.evidence.solana_upgradeable_program_account_data(
            programdata_address
        ),
        programdata_address=programdata_address,
        programdata_slot=4321,
        expected_programdata_slot=4321,
        program_account_context_slot=9000,
        programdata_account_context_slot=9000,
        programdata_metadata=module.evidence.solana_immutable_programdata_metadata(
            4321
        ),
        programdata_executable=program_bytes,
    )


def _live_args(
    module,
    *,
    code_hash,
    programdata_address,
    program_id=None,
    program_bytes=SOLANA_VERIFIER_PROGRAM_BYTES,
):
    if program_id is None:
        program_id = _default_program_id(module)
    return SimpleNamespace(
        route_allowlist_hash=bytes.fromhex(SOLANA_ROUTE_ALLOWLIST_HASH_VECTOR),
        source_verifier_material_hash=bytes.fromhex(SOURCE_VERIFIER_MATERIAL_HASH),
        source_adapter_engine_deployment_hash=bytes.fromhex(
            SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
        ),
        route_canary_evidence_hash=_live_route_canary_hash(
            module,
            program_id=program_id,
            programdata_address=programdata_address,
            program_bytes=program_bytes,
        ),
        expected_destination_binding_hash=bytes.fromhex(SOLANA_DESTINATION_BINDING_VECTOR),
        expected_verifier_code_hash=code_hash,
        expected_programdata_address=programdata_address,
        expected_programdata_slot=4321,
    )


def _live_record(module, *, program_id, programdata_address, program_bytes):
    code_hash = module.evidence.solana_verifier_program_code_hash(program_bytes)
    programdata_raw = module.evidence.decode_solana_base58(
        programdata_address,
        label="programdata address",
    )
    program_account_data = (
        module.UPGRADEABLE_LOADER_PROGRAM_TAG.to_bytes(4, "little")
        + programdata_raw
    )
    programdata_metadata = (
        module.UPGRADEABLE_LOADER_PROGRAMDATA_TAG.to_bytes(4, "little")
        + (4321).to_bytes(8, "little")
        + b"\x00"
        + bytes(32)
    )
    return {
        "verifier_program_id": program_id,
        "rpc_commitment": "finalized",
        "programdata_address": programdata_address,
        "programdata_slot": "4321",
        "program_account_context_slot": "9000",
        "programdata_account_context_slot": "9000",
        "program_owner": module.UPGRADEABLE_LOADER_ID,
        "programdata_owner": module.UPGRADEABLE_LOADER_ID,
        "program_immutable": True,
        "program_account_data_len": "36",
        "program_account_data_base64": base64.b64encode(program_account_data).decode(
            "ascii"
        ),
        "programdata_metadata_blake2b256": "0x"
        + hashlib.blake2b(programdata_metadata, digest_size=32).hexdigest(),
        "programdata_metadata_base64": base64.b64encode(programdata_metadata).decode(
            "ascii"
        ),
        "program_bytes_len": len(program_bytes),
        "programdata_executable_base64": base64.b64encode(program_bytes).decode(
            "ascii"
        ),
        "verifier_code_hash": "0x" + code_hash.hex(),
    }


def test_solana_live_cli_omits_unknown_summary_fields(monkeypatch, capsys):
    module = load_live_module()
    program_id = _default_program_id(module)
    programdata_address = module._encode_solana_base58(bytes.fromhex("11" * 32))
    live = _live_record(
        module,
        program_id=program_id,
        programdata_address=programdata_address,
        program_bytes=SOLANA_VERIFIER_PROGRAM_BYTES,
    )
    live.update(
        {
            "rpc_url": "https://solana.example.invalid/secret-token-provider",
            "operator_note": "safe note",
            "secret-token-summary": "secret-token-value",
            7: "secret-token-int-key",
        }
    )
    monkeypatch.setattr(module, "collect_live_evidence", lambda *args, **kwargs: live)

    exit_code = module.main(
        [
            "--rpc-url",
            "https://solana.example.invalid/secret-token-provider",
            "--verifier-program-id",
            program_id,
        ]
    )

    assert exit_code == 0
    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    assert payload["verifier_program_id"] == program_id
    assert payload["programdata_address"] == programdata_address
    assert "rpc_url" not in payload
    assert "operator_note" not in payload
    assert "secret-token-summary" not in payload
    assert "7" not in payload
    assert "safe note" not in captured.out
    assert "secret-token" not in captured.out
    assert "Traceback" not in captured.err


def test_live_solana_evidence_collects_immutable_program_hash_and_toml():
    module = load_live_module()
    program_id = module._encode_solana_base58(bytes.fromhex("33" * 32))
    programdata_address = module._encode_solana_base58(bytes.fromhex("11" * 32))
    program_bytes = bytes.fromhex("7f454c460102030405")
    programdata_data = _programdata_account_data(
        module,
        slot=4321,
        program_bytes=program_bytes,
    )
    program_account_data = _program_account_data(module, bytes.fromhex("11" * 32))
    programdata_metadata = programdata_data[: module.PROGRAMDATA_METADATA_LEN]
    live = module.collect_live_evidence(
        "https://solana.example.invalid",
        verifier_program_id=program_id,
        opener=_fake_solana_rpc(
            module,
            program_id=program_id,
            programdata_address=programdata_address,
            programdata_data=programdata_data,
        ),
        timeout=3.0,
    )
    code_hash = module.evidence.solana_verifier_program_code_hash(program_bytes)

    assert live["verifier_program_id"] == program_id
    assert live["rpc_commitment"] == "finalized"
    assert live["programdata_address"] == programdata_address
    assert live["programdata_slot"] == "4321"
    assert live["program_account_context_slot"] == "9000"
    assert live["programdata_account_context_slot"] == "9000"
    assert live["program_immutable"] is True
    assert live["program_account_data_len"] == "36"
    assert live["program_account_data_base64"] == base64.b64encode(
        program_account_data
    ).decode("ascii")
    assert live["programdata_metadata_blake2b256"] == (
        "0x" + hashlib.blake2b(programdata_metadata, digest_size=32).hexdigest()
    )
    assert live["programdata_metadata_base64"] == base64.b64encode(
        programdata_metadata
    ).decode("ascii")
    assert live["program_bytes_len"] == len(program_bytes)
    assert live["programdata_executable_base64"] == base64.b64encode(
        program_bytes
    ).decode("ascii")
    assert live["verifier_code_hash"] == "0x" + code_hash.hex()

    args = _live_args(module, code_hash=code_hash, programdata_address=programdata_address)
    summary = module._summary(args, live)
    assert summary["expected_verifier_code_hash_matches"] is True
    assert summary["rpc_commitment_finalized"] is True
    assert summary["expected_programdata_address_matches"] is True
    assert summary["expected_programdata_slot_matches"] is True
    assert summary["expected_destination_binding_hash_matches"] is True
    assert summary["expected_route_allowlist_hash_matches"] is True
    assert summary["destination_toml_ready"] is True
    assert summary["full_toml_ready"] is True
    assert summary["toml_ready"] is True
    assert summary["offline_evidence_args"] == [
        "--verifier-program-id",
        program_id,
        "--verifier-code-hash",
        "0x" + code_hash.hex(),
        "--programdata-address",
        programdata_address,
        "--programdata-slot",
        "4321",
        "--program-account-context-slot",
        "9000",
        "--programdata-account-context-slot",
        "9000",
        "--verifier-program-bytes-base64",
        base64.b64encode(program_bytes).decode("ascii"),
        "--expected-destination-binding-hash",
        "0x" + SOLANA_DESTINATION_BINDING_VECTOR,
        "--route-allowlist-hash",
        "0x" + SOLANA_ROUTE_ALLOWLIST_HASH_VECTOR,
        "--source-verifier-material-hash",
        "0x" + SOURCE_VERIFIER_MATERIAL_HASH,
        "--source-adapter-engine-deployment-hash",
        "0x" + SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH,
        "--route-canary-evidence-hash",
        "0x" + args.route_canary_evidence_hash.hex(),
    ]
    offline_toml = module.evidence.render_toml(
        module._destination_args_from_live(args, live),
        module.evidence.solana_destination_binding_hash(),
    )
    assert summary["offline_toml_sha256"] == hashlib.sha256(
        offline_toml.encode("utf-8")
    ).hexdigest()

    rendered = module.render_toml(args, live)
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
    assert f'# sccp_solana_programdata_address = "{programdata_address}"' in rendered
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
        + code_hash.hex()
        + '"'
        in rendered
    )
    assert (
        '# sccp_solana_programdata_executable_base64 = "'
        + base64.b64encode(program_bytes).decode("ascii")
        + '"'
        in rendered
    )
    assert rendered.count("# sccp_solana_rpc_commitment") == 1
    assert rendered.count("# sccp_solana_program_owner") == 1
    assert rendered.count("# sccp_solana_programdata_owner") == 1
    assert rendered.count("# sccp_solana_program_immutable") == 1
    assert rendered.count("# sccp_solana_program_account_data_len") == 1
    assert rendered.count("# sccp_solana_program_account_data_base64") == 1
    assert rendered.count("# sccp_solana_programdata_address") == 1
    assert rendered.count("# sccp_solana_programdata_slot") == 1
    assert rendered.count("# sccp_solana_expected_programdata_slot") == 1
    assert rendered.count("# sccp_solana_program_account_context_slot") == 1
    assert rendered.count("# sccp_solana_programdata_account_context_slot") == 1
    assert rendered.count("# sccp_solana_programdata_metadata_blake2b256") == 1
    assert rendered.count("# sccp_solana_programdata_metadata_base64") == 1
    assert rendered.count("# sccp_solana_programdata_executable_blake2b256") == 1
    assert rendered.count("# sccp_solana_programdata_executable_base64") == 1
    assert "[[zk.sccp_destination_rollouts]]" in rendered
    assert "[[zk.sccp_route_allowlists]]" in rendered
    assert f'verifier_identity = "{program_id}"' in rendered
    assert f'verifier_code_hash = "0x{code_hash.hex()}"' in rendered
    assert (
        'route_allowlist_hash = "0x' + SOLANA_ROUTE_ALLOWLIST_HASH_VECTOR + '"'
        in rendered
    )


def test_live_solana_direct_api_rejects_forged_live_metadata():
    module = load_live_module()
    program_id = module._encode_solana_base58(bytes.fromhex("33" * 32))
    programdata_address = module._encode_solana_base58(bytes.fromhex("11" * 32))
    program_bytes = b"\x7fELFsol"
    code_hash = module.evidence.solana_verifier_program_code_hash(program_bytes)
    args = _live_args(module, code_hash=code_hash, programdata_address=programdata_address)

    for field, forged_value, expected_message in (
        ("program_owner", "11111111111111111111111111111111", "program owner"),
        ("programdata_owner", "11111111111111111111111111111111", "ProgramData owner"),
        ("program_immutable", False, "immutable"),
        ("program_account_data_len", "37", "36 bytes"),
        (
            "program_account_data_base64",
            base64.b64encode(b"short").decode("ascii"),
            "Program account data",
        ),
        (
            "programdata_metadata_blake2b256",
            "0x" + "bb" * 32,
            "metadata bytes",
        ),
        (
            "programdata_metadata_base64",
            base64.b64encode(b"short").decode("ascii"),
            "ProgramData metadata",
        ),
        ("program_bytes_len", len(program_bytes) + 1, "executable length"),
        ("program_account_context_slot", "4000", "context slot"),
        ("programdata_account_context_slot", "4000", "context slot"),
        ("rpc_commitment", "processed", "commitment"),
        ("verifier_code_hash", "0x" + "bb" * 32, "executable bytes"),
        (
            "programdata_executable_base64",
            base64.b64encode(b"not-elf").decode("ascii"),
            "Solana ProgramData executable base64 metadata is invalid",
        ),
        (
            "programdata_executable_base64",
            " " + base64.b64encode(program_bytes).decode("ascii"),
            "executable base64 metadata",
        ),
        (
            "programdata_executable_base64",
            noncanonical_base64_alias(program_bytes),
            "Solana ProgramData executable base64 metadata is invalid",
        ),
    ):
        live = _live_record(
            module,
            program_id=program_id,
            programdata_address=programdata_address,
            program_bytes=program_bytes,
        )
        live[field] = forged_value
        try:
            module._summary(args, live)
        except ValueError as exc:
            assert expected_message in str(exc)
        else:
            raise AssertionError(
                f"Solana live summary accepted forged {field} metadata"
            )

    live = _live_record(
        module,
        program_id=program_id,
        programdata_address=programdata_address,
        program_bytes=program_bytes,
    )
    live["program_owner"] = "11111111111111111111111111111111"
    try:
        module.render_toml(args, live)
    except ValueError as exc:
        assert "program owner" in str(exc)
    else:
        raise AssertionError("Solana live TOML accepted forged owner metadata")


def test_live_solana_evidence_redacts_imported_parser_failures(monkeypatch):
    """Imported Solana live parser failures must not echo parser payloads."""

    module = load_live_module()
    program_id = module._encode_solana_base58(bytes.fromhex("33" * 32))
    programdata_address = module._encode_solana_base58(bytes.fromhex("11" * 32))
    program_bytes = b"\x7fELFsol"
    code_hash = module.evidence.solana_verifier_program_code_hash(program_bytes)
    args = _live_args(module, code_hash=code_hash, programdata_address=programdata_address)
    live = _live_record(
        module,
        program_id=program_id,
        programdata_address=programdata_address,
        program_bytes=program_bytes,
    )

    original_normalize = module.evidence.normalize_solana_program_id
    original_parse_hex32 = module._parse_hex32
    original_decode_base58 = module.evidence.decode_solana_base58

    def fail_with(exception_type, message):
        raise exception_type(message)

    def normalize_failure_factory(target_label, message):
        def build(exception_type):
            def fail(value, *, label):
                if label == target_label:
                    fail_with(exception_type, message)
                return original_normalize(value, label=label)

            return fail

        return build

    def parse_hex_failure_factory(target_label, message):
        def build(exception_type):
            def fail(value, *, label):
                if label == target_label:
                    fail_with(exception_type, message)
                return original_parse_hex32(value, label=label)

            return fail

        return build

    def unconditional_failure_factory(message):
        def build(exception_type):
            def fail(_value, *, label):
                fail_with(exception_type, message.format(label=label))

            return fail

        return build

    def decode_after_normalization_failure_factory(message):
        def build(exception_type):
            calls = 0

            def fail(value, *, label):
                nonlocal calls
                calls += 1
                if calls > 2:
                    fail_with(exception_type, message.format(label=label))
                return original_decode_base58(value, label=label)

            return fail

        return build

    parser_exception_types = (
        module.argparse.ArgumentTypeError,
        TypeError,
        ValueError,
    )
    cases = (
        (
            "normalize_solana_program_id",
            "verifier program id",
            "Solana live verifier program id metadata is invalid",
            normalize_failure_factory(
                "verifier program id",
                "secret-token verifier program id parser detail",
            ),
            module.evidence,
        ),
        (
            "normalize_solana_program_id",
            "programdata address",
            "Solana live ProgramData address metadata is invalid",
            normalize_failure_factory(
                "programdata address",
                "secret-token programdata address parser detail",
            ),
            module.evidence,
        ),
        (
            "_parse_hex32",
            "verifier_code_hash",
            "Solana live verifier code hash metadata is invalid",
            parse_hex_failure_factory(
                "verifier_code_hash",
                "secret-token verifier_code_hash parser detail",
            ),
            module,
        ),
        (
            "parse_program_bytes_base64",
            "Solana ProgramData executable base64 metadata",
            "Solana ProgramData executable base64 metadata is invalid",
            unconditional_failure_factory(
                "secret-token {label} parser detail",
            ),
            module.evidence,
        ),
        (
            "decode_solana_base58",
            "programdata address",
            "Solana Program account ProgramData address metadata is invalid",
            decode_after_normalization_failure_factory(
                "secret-token {label} decode detail",
            ),
            module.evidence,
        ),
    )

    for attr_name, _label, expected, factory, owner in cases:
        for exception_type in parser_exception_types:
            with monkeypatch.context() as patch:
                patch.setattr(owner, attr_name, factory(exception_type))
                try:
                    module._summary(args, live)
                except ValueError as exc:
                    rendered = str(exc)
                    assert rendered == expected
                    assert "secret-token" not in rendered
                    assert "parser detail" not in rendered
                    assert "decode detail" not in rendered
                    assert exception_type.__name__ not in rendered
                    assert exc.__cause__ is None
                    assert exc.__suppress_context__ is True
                else:
                    raise AssertionError(
                        f"Solana live summary leaked {_label} parser detail"
                    )


def test_live_solana_account_data_redacts_base64_parser_causes(monkeypatch):
    module = load_live_module()
    account = {"data": ["secret-token live account base64", "base64"]}

    try:
        module._account_data(account, label="Solana Program")
    except RuntimeError as exc:
        rendered = str(exc)
        assert rendered == "Solana Program account data is invalid base64"
        assert "secret-token" not in rendered
        assert "live account base64" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("Solana account data accepted invalid base64")

    account_exception_types = (TypeError, ValueError)
    for exception_type in account_exception_types:

        def fail_decode(*_args, exception_type=exception_type, **_kwargs):
            raise exception_type("secret-token account-data decoder detail")

        with monkeypatch.context() as patch:
            patch.setattr(module.base64, "b64decode", fail_decode)
            try:
                module._account_data(
                    {"data": ["ignored", "base64"]},
                    label="Solana Program",
                )
            except RuntimeError as exc:
                rendered = str(exc)
                assert rendered == "Solana Program account data is invalid base64"
                assert "secret-token" not in rendered
                assert "decoder detail" not in rendered
                assert exception_type.__name__ not in rendered
                assert exc.__cause__ is None
                assert exc.__suppress_context__ is True
            else:
                raise AssertionError("Solana account data accepted parser failure")


def test_live_solana_metadata_base64_redacts_parser_causes(monkeypatch):
    module = load_live_module()
    live = {"programdata_metadata_base64": "secret-token live metadata base64"}

    for exception_type in (TypeError, ValueError):

        def fail_decode(*_args, exception_type=exception_type, **_kwargs):
            raise exception_type("secret-token live metadata base64")

        with monkeypatch.context() as patch:
            patch.setattr(module.base64, "b64decode", fail_decode)
            try:
                module._live_base64_bytes(
                    live,
                    "programdata_metadata_base64",
                    label="Solana ProgramData metadata base64 metadata",
                )
            except ValueError as exc:
                rendered = str(exc)
                assert (
                    rendered
                    == "Solana ProgramData metadata base64 metadata must be base64"
                )
                assert "secret-token" not in rendered
                assert "live metadata base64" not in rendered
                assert exception_type.__name__ not in rendered
                assert exc.__cause__ is None
                assert exc.__suppress_context__ is True
            else:
                raise AssertionError("Solana live metadata accepted invalid base64")


def test_live_solana_summary_requires_boolean_destination_readiness(monkeypatch):
    module = load_live_module()
    program_id = module._encode_solana_base58(bytes.fromhex("33" * 32))
    programdata_address = module._encode_solana_base58(bytes.fromhex("11" * 32))
    program_bytes = bytes.fromhex("7f454c460102030405")
    code_hash = module.evidence.solana_verifier_program_code_hash(program_bytes)
    args = _live_args(module, code_hash=code_hash, programdata_address=programdata_address)
    live = _live_record(
        module,
        program_id=program_id,
        programdata_address=programdata_address,
        program_bytes=program_bytes,
    )
    original_summary = module.evidence._json_summary

    def malformed_summary(destination_args, destination_binding_hash, expected_matches):
        summary = original_summary(
            destination_args,
            destination_binding_hash,
            expected_matches,
        )
        summary["full_toml_ready"] = "true"
        summary["toml_ready"] = "true"
        return summary

    monkeypatch.setattr(module.evidence, "_json_summary", malformed_summary)

    summary = module._summary(args, live)
    assert summary["destination_toml_ready"] is False
    assert summary["full_toml_ready"] is False
    assert summary["toml_ready"] is False
    assert "offline_toml_sha256" not in summary


def test_live_solana_evidence_rejects_mutable_program():
    module = load_live_module()
    program_id = module._encode_solana_base58(bytes.fromhex("33" * 32))
    programdata_address = module._encode_solana_base58(bytes.fromhex("11" * 32))
    programdata_data = _programdata_account_data(
        module,
        slot=4321,
        program_bytes=bytes.fromhex("7f454c460102030405"),
        authority=bytes.fromhex("44" * 32),
    )

    try:
        module.collect_live_evidence(
            "https://solana.example.invalid",
            verifier_program_id=program_id,
            opener=_fake_solana_rpc(
                module,
                program_id=program_id,
                programdata_address=programdata_address,
                programdata_data=programdata_data,
            ),
            timeout=3.0,
        )
    except RuntimeError as exc:
        assert "upgrade authority" in str(exc)
    else:
        raise AssertionError("mutable Solana verifier program was accepted")


def test_live_solana_decimal_parsers_reject_noncanonical_text():
    module = load_live_module()

    assert module._parse_positive_u64("4321", label="programdata slot") == 4321
    assert (
        module._live_positive_u64(
            {"programdata_slot": "4321"},
            "programdata_slot",
            label="programdata slot",
        )
        == 4321
    )

    for value in ("0", "04321", "+4321", " 4321 ", "٤٣٢١"):
        try:
            module._parse_positive_u64(value, label="programdata slot")
        except module.argparse.ArgumentTypeError as exc:
            assert "must be a positive u64" in str(exc)
        else:
            raise AssertionError(f"noncanonical Solana slot {value!r} was accepted")

        try:
            module._live_positive_u64(
                {"programdata_slot": value},
                "programdata_slot",
                label="programdata slot",
            )
        except ValueError as exc:
            assert "must be a positive decimal" in str(exc)
        else:
            raise AssertionError(f"noncanonical live Solana slot {value!r} was accepted")


def test_live_solana_evidence_rejects_zero_programdata_slot():
    module = load_live_module()
    program_id = module._encode_solana_base58(bytes.fromhex("33" * 32))
    programdata_address = module._encode_solana_base58(bytes.fromhex("11" * 32))
    programdata_data = _programdata_account_data(
        module,
        slot=0,
        program_bytes=bytes.fromhex("7f454c460102030405"),
    )

    try:
        module.collect_live_evidence(
            "https://solana.example.invalid",
            verifier_program_id=program_id,
            opener=_fake_solana_rpc(
                module,
                program_id=program_id,
                programdata_address=programdata_address,
                programdata_data=programdata_data,
            ),
            timeout=3.0,
        )
    except RuntimeError as exc:
        assert "programdata slot" in str(exc)
    else:
        raise AssertionError("zero Solana ProgramData slot was accepted")


def test_live_solana_evidence_rejects_non_elf_programdata_executable():
    module = load_live_module()
    program_id = module._encode_solana_base58(bytes.fromhex("33" * 32))
    programdata_address = module._encode_solana_base58(bytes.fromhex("11" * 32))
    programdata_data = _programdata_account_data(
        module,
        slot=4321,
        program_bytes=b"not-elf",
    )

    try:
        module.collect_live_evidence(
            "https://solana.example.invalid",
            verifier_program_id=program_id,
            opener=_fake_solana_rpc(
                module,
                program_id=program_id,
                programdata_address=programdata_address,
                programdata_data=programdata_data,
            ),
            timeout=3.0,
        )
    except RuntimeError as exc:
        assert "BPF ELF" in str(exc)
    else:
        raise AssertionError("non-ELF Solana ProgramData executable was accepted")


def test_live_solana_evidence_rejects_programdata_program_alias():
    module = load_live_module()
    program_raw = bytes.fromhex("33" * 32)
    program_id = module._encode_solana_base58(program_raw)
    program_account = _account_payload(
        module,
        _program_account_data(module, program_raw),
        executable=True,
    )

    def opener(request, timeout):
        assert timeout == 3.0
        body = json.loads(request.data.decode("utf-8"))
        assert body["method"] == "getAccountInfo"
        return FakeResponse(
            {
                "jsonrpc": "2.0",
                "id": body["id"],
                "result": {"context": {"slot": 9000}, "value": program_account},
            }
        )

    try:
        module.collect_live_evidence(
            "https://solana.example.invalid",
            verifier_program_id=program_id,
            opener=opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        assert "ProgramData account must differ from program id" in str(exc)
    else:
        raise AssertionError("aliased Solana ProgramData account was accepted")


def test_live_solana_evidence_rejects_noncanonical_program_account_length():
    module = load_live_module()
    program_id = module._encode_solana_base58(bytes.fromhex("33" * 32))
    programdata_raw = bytes.fromhex("11" * 32)
    program_account = _account_payload(
        module,
        _program_account_data(module, programdata_raw) + b"\x00",
        executable=True,
    )

    def opener(request, timeout):
        assert timeout == 3.0
        body = json.loads(request.data.decode("utf-8"))
        assert body["method"] == "getAccountInfo"
        return FakeResponse(
            {
                "jsonrpc": "2.0",
                "id": body["id"],
                "result": {"context": {"slot": 9000}, "value": program_account},
            }
        )

    try:
        module.collect_live_evidence(
            "https://solana.example.invalid",
            verifier_program_id=program_id,
            opener=opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        assert "program account must be exactly 36 bytes" in str(exc)
    else:
        raise AssertionError("noncanonical Solana Program account data was accepted")


def test_live_solana_evidence_rejects_noncanonical_programdata_account_base64():
    module = load_live_module()
    program_id = module._encode_solana_base58(bytes.fromhex("33" * 32))
    programdata_raw = bytes.fromhex("11" * 32)
    programdata_address = module._encode_solana_base58(programdata_raw)
    program_bytes = b"\x7fELFsol"
    program_account = _account_payload(
        module,
        _program_account_data(module, programdata_raw),
        executable=True,
    )
    programdata_data = _programdata_account_data(
        module,
        slot=4321,
        program_bytes=program_bytes,
    )
    programdata_account = _account_payload(module, programdata_data, executable=False)
    programdata_account["data"][0] = noncanonical_base64_alias(programdata_data)

    def opener(request, timeout):
        assert timeout == 3.0
        body = json.loads(request.data.decode("utf-8"))
        assert body["method"] == "getAccountInfo"
        address = body["params"][0]
        account = program_account if address == program_id else programdata_account
        return FakeResponse(
            {
                "jsonrpc": "2.0",
                "id": body["id"],
                "result": {"context": {"slot": 9000}, "value": account},
            }
        )

    try:
        module.collect_live_evidence(
            "https://solana.example.invalid",
            verifier_program_id=program_id,
            opener=opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        assert "canonical base64" in str(exc)
    else:
        raise AssertionError("noncanonical Solana ProgramData account base64 was accepted")


def test_live_solana_evidence_rejects_stale_programdata_read_context():
    module = load_live_module()
    program_id = module._encode_solana_base58(bytes.fromhex("33" * 32))
    programdata_address = module._encode_solana_base58(bytes.fromhex("11" * 32))
    programdata_data = _programdata_account_data(
        module,
        slot=4321,
        program_bytes=bytes.fromhex("7f454c460102030405"),
    )

    try:
        module.collect_live_evidence(
            "https://solana.example.invalid",
            verifier_program_id=program_id,
            opener=_fake_solana_rpc(
                module,
                program_id=program_id,
                programdata_address=programdata_address,
                programdata_data=programdata_data,
                context_slot=4000,
            ),
            timeout=3.0,
        )
    except RuntimeError as exc:
        assert "read context slot" in str(exc)
    else:
        raise AssertionError("stale Solana ProgramData read context was accepted")


def test_live_solana_evidence_rejects_stale_program_read_context():
    module = load_live_module()
    program_id = module._encode_solana_base58(bytes.fromhex("33" * 32))
    programdata_address = module._encode_solana_base58(bytes.fromhex("11" * 32))
    programdata_data = _programdata_account_data(
        module,
        slot=4321,
        program_bytes=bytes.fromhex("7f454c460102030405"),
    )

    try:
        module.collect_live_evidence(
            "https://solana.example.invalid",
            verifier_program_id=program_id,
            opener=_fake_solana_rpc(
                module,
                program_id=program_id,
                programdata_address=programdata_address,
                programdata_data=programdata_data,
                program_context_slot=4000,
                programdata_context_slot=9000,
            ),
            timeout=3.0,
        )
    except RuntimeError as exc:
        assert "program read context slot" in str(exc)
    else:
        raise AssertionError("stale Solana program read context was accepted")


def test_live_solana_evidence_rejects_boolean_context_slot():
    module = load_live_module()
    program_id = module._encode_solana_base58(bytes.fromhex("33" * 32))
    programdata_address = module._encode_solana_base58(bytes.fromhex("11" * 32))
    programdata_data = _programdata_account_data(
        module,
        slot=4321,
        program_bytes=bytes.fromhex("7f454c460102030405"),
    )

    try:
        module.collect_live_evidence(
            "https://solana.example.invalid",
            verifier_program_id=program_id,
            opener=_fake_solana_rpc(
                module,
                program_id=program_id,
                programdata_address=programdata_address,
                programdata_data=programdata_data,
                program_context_slot=True,
                programdata_context_slot=9000,
            ),
            timeout=3.0,
        )
    except RuntimeError as exc:
        assert "context slot must be a positive integer" in str(exc)
    else:
        raise AssertionError("boolean Solana program read context slot was accepted")


def test_live_solana_evidence_keeps_confirmed_reads_diagnostic():
    module = load_live_module()
    program_id = module._encode_solana_base58(bytes.fromhex("33" * 32))
    programdata_address = module._encode_solana_base58(bytes.fromhex("11" * 32))
    program_bytes = bytes.fromhex("7f454c460102030405")
    programdata_data = _programdata_account_data(
        module,
        slot=4321,
        program_bytes=program_bytes,
    )
    live = module.collect_live_evidence(
        "https://solana.example.invalid",
        verifier_program_id=program_id,
        commitment="confirmed",
        opener=_fake_solana_rpc(
            module,
            program_id=program_id,
            programdata_address=programdata_address,
            programdata_data=programdata_data,
        ),
        timeout=3.0,
    )
    code_hash = module.evidence.solana_verifier_program_code_hash(program_bytes)
    args = _live_args(module, code_hash=code_hash, programdata_address=programdata_address)
    summary = module._summary(args, live)

    assert summary["rpc_commitment_finalized"] is False
    assert summary["destination_toml_ready"] is True
    assert summary["full_toml_ready"] is False
    assert summary["toml_ready"] is False
    try:
        module.render_toml(args, live)
    except ValueError as exc:
        assert "finalized Solana JSON-RPC commitment" in str(exc)
    else:
        raise AssertionError("confirmed Solana live evidence rendered production TOML")


def test_live_solana_evidence_rejects_code_hash_drift():
    module = load_live_module()
    program_id = module._encode_solana_base58(bytes.fromhex("33" * 32))
    programdata_address = module._encode_solana_base58(bytes.fromhex("11" * 32))
    program_bytes = bytes.fromhex("7f454c460102030405")
    live = _live_record(
        module,
        program_id=program_id,
        programdata_address=programdata_address,
        program_bytes=program_bytes,
    )
    args = _live_args(
        module,
        code_hash=bytes.fromhex("bb" * 32),
        programdata_address=programdata_address,
    )

    try:
        module._summary(args, live)
    except ValueError as exc:
        assert "--expected-verifier-code-hash" in str(exc)
    else:
        raise AssertionError("drifted Solana verifier code hash pin was accepted")


def test_live_solana_evidence_rejects_programdata_drift():
    module = load_live_module()
    program_id = module._encode_solana_base58(bytes.fromhex("33" * 32))
    programdata_address = module._encode_solana_base58(bytes.fromhex("11" * 32))
    program_bytes = bytes.fromhex("7f454c460102030405")
    code_hash = module.evidence.solana_verifier_program_code_hash(program_bytes)
    live = _live_record(
        module,
        program_id=program_id,
        programdata_address=programdata_address,
        program_bytes=program_bytes,
    )
    args = _live_args(
        module,
        code_hash=code_hash,
        programdata_address=module._encode_solana_base58(bytes.fromhex("22" * 32)),
    )

    try:
        module._summary(args, live)
    except ValueError as exc:
        assert "--expected-programdata-address" in str(exc)
    else:
        raise AssertionError("drifted Solana programdata pin was accepted")


def test_live_solana_evidence_rejects_programdata_slot_drift():
    module = load_live_module()
    program_id = module._encode_solana_base58(bytes.fromhex("33" * 32))
    programdata_address = module._encode_solana_base58(bytes.fromhex("11" * 32))
    program_bytes = bytes.fromhex("7f454c460102030405")
    code_hash = module.evidence.solana_verifier_program_code_hash(program_bytes)
    live = _live_record(
        module,
        program_id=program_id,
        programdata_address=programdata_address,
        program_bytes=program_bytes,
    )
    args = _live_args(module, code_hash=code_hash, programdata_address=programdata_address)
    args.expected_programdata_slot = 4322

    try:
        module._summary(args, live)
    except ValueError as exc:
        assert "--expected-programdata-slot" in str(exc)
    else:
        raise AssertionError("drifted Solana ProgramData slot pin was accepted")


def test_live_solana_evidence_requires_live_pins_for_toml():
    module = load_live_module()
    program_id = module._encode_solana_base58(bytes.fromhex("33" * 32))
    programdata_address = module._encode_solana_base58(bytes.fromhex("11" * 32))
    program_bytes = bytes.fromhex("7f454c460102030405")
    code_hash = module.evidence.solana_verifier_program_code_hash(program_bytes)
    live = _live_record(
        module,
        program_id=program_id,
        programdata_address=programdata_address,
        program_bytes=program_bytes,
    )
    args = _live_args(module, code_hash=code_hash, programdata_address=programdata_address)

    args.expected_verifier_code_hash = None
    try:
        module.render_toml(args, live)
    except ValueError as exc:
        assert "--expected-verifier-code-hash" in str(exc)
    else:
        raise AssertionError("Solana live TOML accepted without a code hash pin")

    args = _live_args(module, code_hash=code_hash, programdata_address=programdata_address)
    args.expected_programdata_address = None
    try:
        module.render_toml(args, live)
    except ValueError as exc:
        assert "--expected-programdata-address" in str(exc)
    else:
        raise AssertionError("Solana live TOML accepted without a ProgramData pin")

    args = _live_args(module, code_hash=code_hash, programdata_address=programdata_address)
    args.expected_programdata_slot = None
    try:
        module.render_toml(args, live)
    except ValueError as exc:
        assert "--expected-programdata-slot" in str(exc)
    else:
        raise AssertionError("Solana live TOML accepted without a ProgramData slot pin")

    args = _live_args(module, code_hash=code_hash, programdata_address=programdata_address)
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
        raise AssertionError("Solana live TOML accepted without route canary evidence")
