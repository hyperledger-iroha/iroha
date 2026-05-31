import base64
import hashlib
import json
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
from types import SimpleNamespace


SUBSTRATE_SORA2_DESTINATION_BINDING_VECTOR = (
    "da5d48fe26518cd8cff6bdaa7cf8e37c7302d1e66469efed4ef2cf340c55b9e4"
)
SOURCE_VERIFIER_MATERIAL_HASH = "aa" * 32
SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH = "99" * 32
SUBSTRATE_SORA2_ROUTE_ALLOWLIST_HASH_VECTOR = (
    "b0c5af8c972bdd32b95aebe4bf29119667d1fb389bdd8366bd3940fc994a7153"
)
SUBSTRATE_RUNTIME_CODE = bytes.fromhex("0061736d010203040506")


def load_live_module():
    script_path = (
        Path(__file__).resolve().parents[2]
        / "scripts"
        / "sccp_substrate_live_evidence.py"
    )
    spec = spec_from_file_location("sccp_substrate_live_evidence", script_path)
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
        return b"substrate-error" * size

    def close(self):
        return None


def fake_substrate_rpc(
    module,
    *,
    finalized_head=None,
    runtime_code=None,
    spec_name="sora2",
    spec_version=1234,
    transaction_version=7,
):
    if finalized_head is None:
        finalized_head = bytes.fromhex("55" * 32)
    if runtime_code is None:
        runtime_code = SUBSTRATE_RUNTIME_CODE

    def opener(request, timeout):
        assert timeout == 3.0
        payload = json.loads(request.data.decode("utf-8"))
        method = payload["method"]
        if method == "chain_getFinalizedHead":
            return FakeResponse(
                {
                    "jsonrpc": "2.0",
                    "id": payload["id"],
                    "result": "0x" + finalized_head.hex(),
                }
            )
        if method == "state_getRuntimeVersion":
            assert payload["params"] == ["0x" + finalized_head.hex()]
            return FakeResponse(
                {
                    "jsonrpc": "2.0",
                    "id": payload["id"],
                    "result": {
                        "specName": spec_name,
                        "specVersion": spec_version,
                        "transactionVersion": transaction_version,
                    },
                }
            )
        if method == "state_getStorage":
            assert payload["params"] == [
                module.RUNTIME_CODE_STORAGE_KEY,
                "0x" + finalized_head.hex(),
            ]
            return FakeResponse(
                {
                    "jsonrpc": "2.0",
                    "id": payload["id"],
                    "result": "0x" + runtime_code.hex(),
                }
            )
        raise AssertionError(f"unexpected method {method}")

    return SimpleNamespace(
        opener=opener,
        finalized_head=finalized_head,
        runtime_code=runtime_code,
        spec_name=spec_name,
        spec_version=spec_version,
        transaction_version=transaction_version,
    )


def test_substrate_json_rpc_response_size_is_bounded():
    module = load_live_module()

    def oversized_opener(_request, timeout):
        assert timeout == 3.0
        return OversizedResponse()

    try:
        module._json_rpc(
            "https://substrate.example.invalid",
            "chain_getFinalizedHead",
            [],
            opener=oversized_opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        assert "response exceeds" in str(exc)
    else:
        raise AssertionError("oversized Substrate JSON-RPC response was accepted")


def test_substrate_json_rpc_http_error_detail_is_bounded():
    module = load_live_module()

    def failing_opener(request, timeout):
        assert timeout == 3.0
        raise module.urllib.error.HTTPError(
            request.full_url,
            503,
            "unavailable",
            {},
            OversizedErrorBody(),
        )

    try:
        module._json_rpc(
            "https://substrate.example.invalid",
            "chain_getFinalizedHead",
            [],
            opener=failing_opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        message = str(exc)
        assert "HTTP 503" in message
        assert "...<truncated>" in message
        assert len(message) < 4300
    else:
        raise AssertionError("oversized Substrate JSON-RPC error body was accepted")


def test_substrate_json_rpc_rejects_duplicate_json_keys():
    module = load_live_module()
    duplicate_payload = (
        b'{"jsonrpc":"2.0","id":1,"result":{"specName":"sora2",'
        b'"specVersion":1,"specVersion":2,"transactionVersion":1}}'
    )

    def duplicate_json_opener(_request, timeout):
        assert timeout == 3.0
        return RawResponse(duplicate_payload)

    try:
        module._json_rpc(
            "https://substrate.example.invalid",
            "state_getRuntimeVersion",
            [],
            opener=duplicate_json_opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        assert "duplicate JSON key" in str(exc)
    else:
        raise AssertionError("duplicate-key Substrate JSON-RPC response was accepted")


def live_route_canary_hash(
    module,
    *,
    runtime_code,
    finalized_head,
):
    return module.evidence.substrate_route_canary_evidence_hash(
        domain=8,
        route_allowlist_hash=bytes.fromhex(
            SUBSTRATE_SORA2_ROUTE_ALLOWLIST_HASH_VECTOR
        ),
        destination_binding_hash=bytes.fromhex(
            SUBSTRATE_SORA2_DESTINATION_BINDING_VECTOR
        ),
        source_verifier_material_hash=bytes.fromhex(SOURCE_VERIFIER_MATERIAL_HASH),
        source_adapter_engine_deployment_hash=bytes.fromhex(
            SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
        ),
        verifier_entrypoint=module.evidence.SUBSTRATE_RUNTIME_VERIFIER_ID,
        verifier_code_hash=module.runtime_code_hash(runtime_code),
        finalized_head=finalized_head,
        runtime_spec_name="sora2",
        runtime_spec_version=1234,
        runtime_transaction_version=7,
        runtime_code=runtime_code,
    )


def live_args(
    module,
    *,
    code_hash,
    finalized_head,
    runtime_code=SUBSTRATE_RUNTIME_CODE,
):
    return SimpleNamespace(
        route_allowlist_hash=bytes.fromhex(
            SUBSTRATE_SORA2_ROUTE_ALLOWLIST_HASH_VECTOR
        ),
        route_canary_evidence_hash=live_route_canary_hash(
            module,
            runtime_code=runtime_code,
            finalized_head=finalized_head,
        ),
        source_verifier_material_hash=bytes.fromhex(SOURCE_VERIFIER_MATERIAL_HASH),
        source_adapter_engine_deployment_hash=bytes.fromhex(
            SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
        ),
        expected_destination_binding_hash=bytes.fromhex(
            SUBSTRATE_SORA2_DESTINATION_BINDING_VECTOR
        ),
        expected_finalized_head=finalized_head,
        expected_runtime_code_hash=code_hash,
        expected_spec_name="sora2",
        expected_spec_version=1234,
        expected_transaction_version=7,
    )


def test_live_substrate_evidence_collects_finalized_runtime_and_toml():
    module = load_live_module()
    fake = fake_substrate_rpc(module)
    live = module.collect_live_evidence(
        "https://substrate.example.invalid",
        domain=module.evidence.parse_substrate_domain("sora2"),
        opener=fake.opener,
        timeout=3.0,
    )
    code_hash = module.runtime_code_hash(fake.runtime_code)

    assert live["domain"] == 8
    assert live["chain"] == "sora2"
    assert live["finalized_head"] == "0x" + fake.finalized_head.hex()
    assert live["runtime_code_storage_key"] == module.RUNTIME_CODE_STORAGE_KEY
    assert live["runtime_spec_name"] == "sora2"
    assert live["runtime_spec_version"] == 1234
    assert live["runtime_transaction_version"] == 7
    assert live["runtime_code_len"] == len(fake.runtime_code)
    assert live["runtime_code_base64"] == base64.b64encode(fake.runtime_code).decode(
        "ascii"
    )
    assert live["runtime_code_hash_algorithm"] == "blake2b-256"
    assert live["verifier_code_hash"] == "0x" + code_hash.hex()

    args = live_args(module, code_hash=code_hash, finalized_head=fake.finalized_head)
    summary = module._summary(args, live)
    assert summary["expected_finalized_head_matches"] is True
    assert summary["expected_runtime_code_hash_matches"] is True
    assert summary["expected_spec_name_matches"] is True
    assert summary["expected_spec_version_matches"] is True
    assert summary["expected_transaction_version_matches"] is True
    assert summary["expected_destination_binding_hash_matches"] is True
    assert summary["expected_route_allowlist_hash_matches"] is True
    assert summary["toml_ready"] is True
    assert summary["offline_evidence_args"] == [
        "--domain",
        "sora2",
        "--verifier-code-hash",
        "0x" + code_hash.hex(),
        "--runtime-code-base64",
        base64.b64encode(fake.runtime_code).decode("ascii"),
        "--finalized-head",
        "0x" + fake.finalized_head.hex(),
        "--runtime-spec-name",
        "sora2",
        "--runtime-spec-version",
        "1234",
        "--runtime-transaction-version",
        "7",
        "--expected-destination-binding-hash",
        "0x" + SUBSTRATE_SORA2_DESTINATION_BINDING_VECTOR,
        "--route-allowlist-hash",
        "0x" + SUBSTRATE_SORA2_ROUTE_ALLOWLIST_HASH_VECTOR,
        "--source-verifier-material-hash",
        "0x" + SOURCE_VERIFIER_MATERIAL_HASH,
        "--source-adapter-engine-deployment-hash",
        "0x" + SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH,
        "--route-canary-evidence-hash",
        "0x" + args.route_canary_evidence_hash.hex(),
    ]
    offline_toml = module.evidence.render_toml(
        module._destination_args_from_live(args, live),
        module.evidence.substrate_destination_binding_hash(8),
    )
    assert summary["offline_toml_sha256"] == hashlib.sha256(
        offline_toml.encode("utf-8")
    ).hexdigest()
    assert summary["route_canary"]["evidence_hash"] == (
        "0x" + args.route_canary_evidence_hash.hex()
    )

    rendered = module.render_toml(args, live)
    assert '# sccp_substrate_finalized_head = "0x' + "55" * 32 + '"' in rendered
    assert '# sccp_substrate_runtime_spec_name = "sora2"' in rendered
    assert '# sccp_substrate_runtime_spec_version = "1234"' in rendered
    assert '# sccp_substrate_runtime_transaction_version = "7"' in rendered
    assert (
        '# sccp_substrate_runtime_code_hash = "0x' + code_hash.hex() + '"'
        in rendered
    )
    assert (
        '# sccp_substrate_runtime_code_base64 = "'
        + base64.b64encode(fake.runtime_code).decode("ascii")
        + '"'
        in rendered
    )
    assert rendered.count("# sccp_substrate_finalized_head") == 1
    assert rendered.count("# sccp_substrate_runtime_spec_name") == 1
    assert rendered.count("# sccp_substrate_runtime_spec_version") == 1
    assert rendered.count("# sccp_substrate_runtime_transaction_version") == 1
    assert rendered.count("# sccp_substrate_runtime_code_hash") == 1
    assert rendered.count("# sccp_substrate_runtime_code_base64") == 1
    assert "[[zk.sccp_destination_rollouts]]" in rendered
    assert "[[zk.sccp_route_allowlists]]" in rendered
    assert (
        '# sccp_route_canary_evidence_hash = "0x'
        + args.route_canary_evidence_hash.hex()
        + '"'
        in rendered
    )
    assert 'chain = "sora2"' in rendered
    assert f'verifier_code_hash = "0x{code_hash.hex()}"' in rendered
    assert '# sccp_route_canary_status = "passed"' in rendered


def test_live_substrate_evidence_rejects_foreign_runtime_spec_name():
    module = load_live_module()
    fake = fake_substrate_rpc(module, spec_name="sora-polkadot")

    try:
        module.collect_live_evidence(
            "https://substrate.example.invalid",
            domain=module.evidence.parse_substrate_domain("sora2"),
            opener=fake.opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        message = str(exc)
        assert "specName" in message
        assert "destination domain" in message
        assert "sora2" in message
        assert "sora-polkadot" in message
    else:
        raise AssertionError("foreign Substrate runtime specName was accepted")


def test_live_substrate_evidence_rejects_padded_runtime_spec_name():
    module = load_live_module()
    fake = fake_substrate_rpc(module, spec_name=" sora2 ")

    try:
        module.collect_live_evidence(
            "https://substrate.example.invalid",
            domain=module.evidence.parse_substrate_domain("sora2"),
            opener=fake.opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        assert "specName must be non-empty" in str(exc)
    else:
        raise AssertionError("padded Substrate runtime specName was accepted")


def test_live_substrate_runtime_version_parser_rejects_noncanonical_text():
    module = load_live_module()

    assert module._parse_nonnegative_u32("0", label="runtime spec version") == 0
    assert module._parse_nonnegative_u32("1234", label="runtime spec version") == 1234

    for value in ("00", "01234", "+1234", " 1234 ", "١٢٣٤"):
        try:
            module._parse_nonnegative_u32(value, label="runtime spec version")
        except module.argparse.ArgumentTypeError as exc:
            assert "must be a decimal u32" in str(exc)
        else:
            raise AssertionError(
                f"noncanonical Substrate runtime version {value!r} was accepted"
            )

    try:
        module._parse_nonempty(" sora2 ", label="expected spec name")
    except module.argparse.ArgumentTypeError as exc:
        assert "must be non-empty" in str(exc)
    else:
        raise AssertionError("padded Substrate expected specName was accepted")

    try:
        module._rpc_hex_bytes(" 0x0102", method="state_getStorage")
    except RuntimeError as exc:
        assert "non-canonical" in str(exc)
    else:
        raise AssertionError("padded Substrate runtime hex was accepted")

    for value in ("0102", "0X0102", "0xABCD"):
        try:
            module._rpc_hex_bytes(value, method="state_getStorage")
        except RuntimeError as exc:
            assert "lowercase 0x hex" in str(exc)
        else:
            raise AssertionError(
                f"noncanonical Substrate runtime hex {value!r} was accepted"
            )


def test_live_substrate_evidence_rejects_empty_runtime_code():
    module = load_live_module()
    fake = fake_substrate_rpc(module, runtime_code=b"")

    try:
        module.collect_live_evidence(
            "https://substrate.example.invalid",
            domain=module.evidence.parse_substrate_domain("sora2"),
            opener=fake.opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        assert "state_getStorage :code returned empty or all-zero data" in str(exc)
    else:
        raise AssertionError("empty Substrate runtime :code was accepted")


def test_live_substrate_evidence_rejects_runtime_code_hash_drift():
    module = load_live_module()
    fake = fake_substrate_rpc(module)
    live = module.collect_live_evidence(
        "https://substrate.example.invalid",
        domain=module.evidence.parse_substrate_domain("sora2"),
        opener=fake.opener,
        timeout=3.0,
    )
    args = live_args(
        module,
        code_hash=bytes.fromhex("cc" * 32),
        finalized_head=fake.finalized_head,
    )

    try:
        module._summary(args, live)
    except ValueError as exc:
        assert "--expected-runtime-code-hash" in str(exc)
    else:
        raise AssertionError("drifted Substrate runtime code hash pin was accepted")


def test_live_substrate_evidence_rejects_finalized_head_drift():
    module = load_live_module()
    fake = fake_substrate_rpc(module)
    live = module.collect_live_evidence(
        "https://substrate.example.invalid",
        domain=module.evidence.parse_substrate_domain("sora2"),
        opener=fake.opener,
        timeout=3.0,
    )
    args = live_args(
        module,
        code_hash=module.runtime_code_hash(fake.runtime_code),
        finalized_head=bytes.fromhex("44" * 32),
    )

    try:
        module._summary(args, live)
    except ValueError as exc:
        assert "--expected-finalized-head" in str(exc)
    else:
        raise AssertionError("drifted Substrate finalized head pin was accepted")


def test_live_substrate_direct_api_rejects_forged_live_metadata():
    module = load_live_module()
    fake = fake_substrate_rpc(module)
    live = module.collect_live_evidence(
        "https://substrate.example.invalid",
        domain=module.evidence.parse_substrate_domain("sora2"),
        opener=fake.opener,
        timeout=3.0,
    )
    args = live_args(
        module,
        code_hash=module.runtime_code_hash(fake.runtime_code),
        finalized_head=fake.finalized_head,
    )

    for field, forged_value, expected_message in (
        ("chain", "sora-polkadot", "chain"),
        ("runtime_code_storage_key", "0x00", "storage key"),
        ("runtime_code_hash_algorithm", "sha256", "hash algorithm"),
        ("verifier_entrypoint", "wrong_entrypoint", "entrypoint"),
        ("runtime_spec_name", "sora-polkadot", "specName"),
        ("runtime_code_len", live["runtime_code_len"] + 1, "length"),
        ("runtime_code_base64", " " + live["runtime_code_base64"], "exact"),
        ("verifier_code_hash", "0x" + "bb" * 32, "verifier_code_hash"),
    ):
        forged = dict(live)
        forged[field] = forged_value
        try:
            module._summary(args, forged)
        except ValueError as exc:
            assert expected_message in str(exc)
        else:
            raise AssertionError(f"Substrate live summary accepted forged {field}")

    forged = dict(live)
    forged["runtime_code_hash_algorithm"] = "sha256"
    try:
        module.render_toml(args, forged)
    except ValueError as exc:
        assert "hash algorithm" in str(exc)
    else:
        raise AssertionError("Substrate live TOML accepted forged hash algorithm")


def test_live_substrate_summary_requires_boolean_destination_readiness(monkeypatch):
    module = load_live_module()
    fake = fake_substrate_rpc(module)
    live = module.collect_live_evidence(
        "https://substrate.example.invalid",
        domain=module.evidence.parse_substrate_domain("sora2"),
        opener=fake.opener,
        timeout=3.0,
    )
    args = live_args(
        module,
        code_hash=module.runtime_code_hash(fake.runtime_code),
        finalized_head=fake.finalized_head,
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
    assert summary["toml_ready"] is False
    assert "offline_toml_sha256" not in summary


def test_live_substrate_evidence_requires_live_pins_for_toml():
    module = load_live_module()
    fake = fake_substrate_rpc(module)
    live = module.collect_live_evidence(
        "https://substrate.example.invalid",
        domain=module.evidence.parse_substrate_domain("sora2"),
        opener=fake.opener,
        timeout=3.0,
    )
    code_hash = module.runtime_code_hash(fake.runtime_code)
    args = live_args(module, code_hash=code_hash, finalized_head=fake.finalized_head)

    args.expected_runtime_code_hash = None
    try:
        module.render_toml(args, live)
    except ValueError as exc:
        assert "--expected-runtime-code-hash" in str(exc)
    else:
        raise AssertionError("Substrate live TOML accepted without a code hash pin")

    args = live_args(module, code_hash=code_hash, finalized_head=fake.finalized_head)
    args.expected_finalized_head = None
    try:
        module.render_toml(args, live)
    except ValueError as exc:
        assert "--expected-finalized-head" in str(exc)
    else:
        raise AssertionError("Substrate live TOML accepted without a finalized head pin")

    args = live_args(module, code_hash=code_hash, finalized_head=fake.finalized_head)
    args.route_canary_evidence_hash = None
    try:
        module.render_toml(args, live)
    except ValueError as exc:
        assert "--route-canary-evidence-hash" in str(exc)
    else:
        raise AssertionError("Substrate live TOML accepted without route canary evidence")
