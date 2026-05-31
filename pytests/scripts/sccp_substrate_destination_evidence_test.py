import json
import base64
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
from types import SimpleNamespace


SUBSTRATE_RUNTIME_VERIFIER_ID = "SccpBridge.submit_message_proof"
SUBSTRATE_KUSAMA_DESTINATION_BINDING_VECTOR = (
    "2ee5c37634c3fab7e9086ea43af7553089fc24dc2ce27d76c46ef4c3da57bb56"
)
SUBSTRATE_POLKADOT_DESTINATION_BINDING_VECTOR = (
    "570ec340d4fee4a84eaa7a53b19baa53c9f4f8d7f64c3c43639fde0c6b3fdef0"
)
SUBSTRATE_SORA2_DESTINATION_BINDING_VECTOR = (
    "da5d48fe26518cd8cff6bdaa7cf8e37c7302d1e66469efed4ef2cf340c55b9e4"
)
SOURCE_VERIFIER_MATERIAL_HASH = "aa" * 32
SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH = "99" * 32
SUBSTRATE_ROUTE_ALLOWLIST_HASH_VECTORS = {
    6: "0f55a05d621b0f2dbdf6dfbfaf64b2505171ff6530c110348640b9b490cb8ae8",
    7: "238353f061006661fbd3f823cea76d13391fd452da5db85a7ee1cfc607b3b8de",
    8: "b0c5af8c972bdd32b95aebe4bf29119667d1fb389bdd8366bd3940fc994a7153",
}
SUBSTRATE_RUNTIME_CODE = b"\x00asm\x01\x00sora-runtime"


def load_evidence_module():
    script_path = (
        Path(__file__).resolve().parents[2]
        / "scripts"
        / "sccp_substrate_destination_evidence.py"
    )
    spec = spec_from_file_location("sccp_substrate_destination_evidence", script_path)
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


def substrate_route_canary_evidence_hash(
    module,
    *,
    domain=8,
    runtime_code=SUBSTRATE_RUNTIME_CODE,
):
    destination_hashes = {
        6: SUBSTRATE_KUSAMA_DESTINATION_BINDING_VECTOR,
        7: SUBSTRATE_POLKADOT_DESTINATION_BINDING_VECTOR,
        8: SUBSTRATE_SORA2_DESTINATION_BINDING_VECTOR,
    }
    return module.substrate_route_canary_evidence_hash(
        domain=domain,
        route_allowlist_hash=bytes.fromhex(
            SUBSTRATE_ROUTE_ALLOWLIST_HASH_VECTORS[domain]
        ),
        destination_binding_hash=bytes.fromhex(destination_hashes[domain]),
        source_verifier_material_hash=bytes.fromhex(SOURCE_VERIFIER_MATERIAL_HASH),
        source_adapter_engine_deployment_hash=bytes.fromhex(
            SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
        ),
        verifier_entrypoint=SUBSTRATE_RUNTIME_VERIFIER_ID,
        verifier_code_hash=module.substrate_runtime_code_hash(runtime_code),
        finalized_head=bytes.fromhex("55" * 32),
        runtime_spec_name={6: "sora-kusama", 7: "sora-polkadot", 8: "sora2"}[
            domain
        ],
        runtime_spec_version=1234,
        runtime_transaction_version=7,
        runtime_code=runtime_code,
    ).hex()


def substrate_args(module, domain=8):
    runtime_hash = module.substrate_runtime_code_hash(SUBSTRATE_RUNTIME_CODE)
    return SimpleNamespace(
        domain=domain,
        verifier_entrypoint=SUBSTRATE_RUNTIME_VERIFIER_ID,
        verifier_code_hash=runtime_hash,
        runtime_code_hex=SUBSTRATE_RUNTIME_CODE,
        runtime_code_base64=None,
        runtime_code_file=None,
        source_verifier_material_hash=bytes.fromhex(SOURCE_VERIFIER_MATERIAL_HASH),
        source_adapter_engine_deployment_hash=bytes.fromhex(
            SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
        ),
        route_allowlist_hash=bytes.fromhex(
            SUBSTRATE_ROUTE_ALLOWLIST_HASH_VECTORS[domain]
        ),
        route_canary_evidence_hash=bytes.fromhex(
            substrate_route_canary_evidence_hash(module, domain=domain)
        ),
        expected_destination_binding_hash=bytes.fromhex(
            {
                6: SUBSTRATE_KUSAMA_DESTINATION_BINDING_VECTOR,
                7: SUBSTRATE_POLKADOT_DESTINATION_BINDING_VECTOR,
                8: SUBSTRATE_SORA2_DESTINATION_BINDING_VECTOR,
            }[domain]
        ),
        finalized_head=bytes.fromhex("55" * 32),
        runtime_spec_name={6: "sora-kusama", 7: "sora-polkadot", 8: "sora2"}[
            domain
        ],
        runtime_spec_version=1234,
        runtime_transaction_version=7,
    )


def test_substrate_domain_parser_accepts_runtime_lanes_only():
    module = load_evidence_module()

    assert module.parse_substrate_domain("sora-kusama") == 6
    assert module.parse_substrate_domain("kusama") == 6
    assert module.parse_substrate_domain("6") == 6
    assert module.parse_substrate_domain("sora-polkadot") == 7
    assert module.parse_substrate_domain("polkadot") == 7
    assert module.parse_substrate_domain("7") == 7
    assert module.parse_substrate_domain("sora2") == 8
    assert module.parse_substrate_domain("8") == 8

    try:
        module.parse_substrate_domain("ton")
    except module.argparse.ArgumentTypeError as exc:
        assert "domain must be sora-kusama, sora-polkadot, or sora2" in str(exc)
    else:
        raise AssertionError("non-Substrate destination domain was accepted")

    try:
        module.parse_substrate_domain(" sora2 ")
    except module.argparse.ArgumentTypeError as exc:
        assert "domain must be sora-kusama, sora-polkadot, or sora2" in str(exc)
    else:
        raise AssertionError("padded Substrate destination domain was accepted")

    assert module.parse_nonempty_string("sora2", label="runtime specName") == "sora2"
    try:
        module.parse_nonempty_string(" sora2 ", label="runtime specName")
    except module.argparse.ArgumentTypeError as exc:
        assert "must be non-empty" in str(exc)
    else:
        raise AssertionError("padded Substrate runtime specName was accepted")


def test_substrate_decimal_u32_parser_rejects_noncanonical_text():
    module = load_evidence_module()

    assert module.parse_decimal_u32("0", label="runtime spec version") == 0
    assert (
        module.parse_decimal_u32("4294967295", label="runtime spec version")
        == 0xFFFF_FFFF
    )

    for value in ("00", "01", "+1", " 1 ", "١"):
        try:
            module.parse_decimal_u32(value, label="runtime spec version")
        except module.argparse.ArgumentTypeError as exc:
            assert "must be a decimal u32" in str(exc)
        else:
            raise AssertionError(f"noncanonical Substrate u32 {value!r} was accepted")


def test_substrate_hex_parser_and_entrypoint_reject_malformed_values():
    module = load_evidence_module()

    assert module.parse_hex_bytes(
        "0x" + "33" * 32,
        label="verifier code hash",
        byte_length=32,
    ) == bytes.fromhex("33" * 32)
    assert (
        module.parse_runtime_entrypoint(SUBSTRATE_RUNTIME_VERIFIER_ID)
        == SUBSTRATE_RUNTIME_VERIFIER_ID
    )
    assert module.parse_runtime_code_hex(
        "0x" + b"\x00asm\x01runtime".hex(),
        label="runtime code",
    ) == b"\x00asm\x01runtime"
    assert module.parse_runtime_code_base64(
        base64.b64encode(b"\x00asm\x01runtime").decode("ascii"),
        label="runtime code",
    ) == b"\x00asm\x01runtime"

    for value, parser in (
        ("0x" + b"\x00asm\x01runtime".hex() + "\n", module.parse_runtime_code_hex),
        (
            " " + base64.b64encode(b"\x00asm\x01runtime").decode("ascii"),
            module.parse_runtime_code_base64,
        ),
    ):
        try:
            parser(value, label="runtime code")
        except module.argparse.ArgumentTypeError as exc:
            assert "must not contain whitespace" in str(exc)
        else:
            raise AssertionError("padded Substrate runtime code was accepted")

    try:
        module.parse_runtime_code_base64(
            noncanonical_base64_alias(b"\x00asm\x01"),
            label="runtime code",
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "canonical base64" in str(exc)
    else:
        raise AssertionError("non-canonical Substrate runtime code was accepted")

    try:
        module.parse_hex_bytes(
            "0x" + "00" * 32,
            label="verifier code hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must not be zero" in str(exc)
    else:
        raise AssertionError("zero Substrate verifier code hash was accepted")

    try:
        module.parse_hex_bytes(
            " 0x" + "33" * 32 + " ",
            label="verifier code hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must not contain whitespace" in str(exc)
    else:
        raise AssertionError("padded Substrate verifier code hash was accepted")

    try:
        module.parse_hex_bytes(
            "0x" + "33" * 31,
            label="verifier code hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must be 32 bytes" in str(exc)
    else:
        raise AssertionError("short Substrate verifier code hash was accepted")

    try:
        module.parse_runtime_entrypoint("SccpBridge.other_call")
    except module.argparse.ArgumentTypeError as exc:
        assert "verifier entrypoint must be" in str(exc)
    else:
        raise AssertionError("wrong Substrate runtime entrypoint was accepted")

    try:
        module.parse_runtime_entrypoint(" " + SUBSTRATE_RUNTIME_VERIFIER_ID + " ")
    except module.argparse.ArgumentTypeError as exc:
        assert "verifier entrypoint must be" in str(exc)
    else:
        raise AssertionError("padded Substrate runtime entrypoint was accepted")


def test_substrate_destination_binding_hash_matches_rust_vectors():
    module = load_evidence_module()

    assert (
        module.substrate_destination_binding_key(6)
        == "sccp:0:6:sora-kusama:substrate-runtime-v1:5"
    )
    assert (
        module.substrate_destination_binding_hash(6).hex()
        == SUBSTRATE_KUSAMA_DESTINATION_BINDING_VECTOR
    )
    assert (
        module.substrate_destination_binding_hash(7).hex()
        == SUBSTRATE_POLKADOT_DESTINATION_BINDING_VECTOR
    )
    assert (
        module.substrate_destination_binding_hash(8).hex()
        == SUBSTRATE_SORA2_DESTINATION_BINDING_VECTOR
    )


def test_substrate_route_allowlist_hash_matches_lane_evidence_vectors():
    module = load_evidence_module()
    destination_hashes = {
        6: SUBSTRATE_KUSAMA_DESTINATION_BINDING_VECTOR,
        7: SUBSTRATE_POLKADOT_DESTINATION_BINDING_VECTOR,
        8: SUBSTRATE_SORA2_DESTINATION_BINDING_VECTOR,
    }

    for domain, route_hash in SUBSTRATE_ROUTE_ALLOWLIST_HASH_VECTORS.items():
        assert (
            module.substrate_route_allowlist_hash(
                domain=domain,
                source_verifier_material_hash=bytes.fromhex(
                    SOURCE_VERIFIER_MATERIAL_HASH
                ),
                source_adapter_engine_deployment_hash=bytes.fromhex(
                    SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
                ),
                destination_binding_hash=bytes.fromhex(destination_hashes[domain]),
            ).hex()
            == route_hash
        )


def test_substrate_route_canary_rejects_verifier_code_hash_role_reuse():
    module = load_evidence_module()
    runtime_hash = module.substrate_runtime_code_hash(SUBSTRATE_RUNTIME_CODE)
    args = {
        "domain": 8,
        "route_allowlist_hash": bytes.fromhex(
            SUBSTRATE_ROUTE_ALLOWLIST_HASH_VECTORS[8]
        ),
        "destination_binding_hash": bytes.fromhex(
            SUBSTRATE_SORA2_DESTINATION_BINDING_VECTOR
        ),
        "source_verifier_material_hash": bytes.fromhex(
            SOURCE_VERIFIER_MATERIAL_HASH
        ),
        "source_adapter_engine_deployment_hash": bytes.fromhex(
            SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH
        ),
        "verifier_entrypoint": SUBSTRATE_RUNTIME_VERIFIER_ID,
        "verifier_code_hash": runtime_hash,
        "finalized_head": bytes.fromhex("55" * 32),
        "runtime_spec_name": "sora2",
        "runtime_spec_version": 1234,
        "runtime_transaction_version": 7,
        "runtime_code": SUBSTRATE_RUNTIME_CODE,
    }

    for field in (
        "route_allowlist_hash",
        "destination_binding_hash",
        "source_verifier_material_hash",
        "source_adapter_engine_deployment_hash",
    ):
        replay_args = dict(args)
        replay_args[field] = runtime_hash
        try:
            module.substrate_route_canary_evidence_hash(**replay_args)
        except ValueError as exc:
            assert f"verifier_code_hash must differ from {field}" in str(exc)
        else:
            raise AssertionError(
                "Substrate route canary accepted runtime code hash replay of "
                f"{field}"
            )


def test_substrate_toml_rendering_carries_runtime_profile_ids():
    module = load_evidence_module()
    runtime_hash = module.substrate_runtime_code_hash(SUBSTRATE_RUNTIME_CODE).hex()
    route_canary_hash = substrate_route_canary_evidence_hash(module)
    rendered = module.render_toml(substrate_args(module))

    assert (
        '# sccp_substrate_destination_binding_hash = "0x'
        + SUBSTRATE_SORA2_DESTINATION_BINDING_VECTOR
        + '"'
        in rendered
    )
    assert (
        '# sccp_substrate_route_allowlist_hash = "0x'
        + SUBSTRATE_ROUTE_ALLOWLIST_HASH_VECTORS[8]
        + '"'
        in rendered
    )
    assert '# sccp_substrate_finalized_head = "0x' + "55" * 32 + '"' in rendered
    assert '# sccp_substrate_runtime_spec_name = "sora2"' in rendered
    assert '# sccp_substrate_runtime_spec_version = "1234"' in rendered
    assert '# sccp_substrate_runtime_transaction_version = "7"' in rendered
    assert '# sccp_substrate_runtime_code_hash = "0x' + runtime_hash + '"' in rendered
    assert (
        'destination_binding_key = "sccp:0:8:sora2:substrate-runtime-v1:5"'
        in rendered
    )
    assert (
        'destination_binding_hash = "0x'
        + SUBSTRATE_SORA2_DESTINATION_BINDING_VECTOR
        + '"'
        in rendered
    )
    assert "[[zk.sccp_destination_rollouts]]" in rendered
    assert "[[zk.sccp_route_allowlists]]" in rendered
    assert "domain = 8" in rendered
    assert 'chain = "sora2"' in rendered
    assert 'verifier_plan = "SubstrateRuntimeNativeRecursive"' in rendered
    assert f'verifier_identity = "{SUBSTRATE_RUNTIME_VERIFIER_ID}"' in rendered
    assert 'verifier_code_hash = "0x' + runtime_hash + '"' in rendered
    assert 'anchor_id = "sccp:sora2:destination-anchor:runtime:v1"' in rendered
    assert (
        'route_allowlist_id = "sccp:sora2:route-allowlist:runtime:v1"'
        in rendered
    )
    assert (
        'route_allowlist_hash = "0x'
        + SUBSTRATE_ROUTE_ALLOWLIST_HASH_VECTORS[8]
        + '"'
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
    assert "blockers = []" in rendered

    polkadot = module.render_toml(substrate_args(module, domain=7))
    assert 'chain = "sora-polkadot"' in polkadot
    assert (
        'anchor_id = "sccp:sora-polkadot:destination-anchor:runtime:v1"'
        in polkadot
    )
    assert (
        'route_allowlist_id = "sccp:sora-polkadot:route-allowlist:runtime:v1"'
        in polkadot
    )

    try:
        module.render_toml(
            substrate_args(module),
            destination_binding_hash=bytes.fromhex("ee" * 32),
        )
    except ValueError as exc:
        assert "canonical SORA -> sora2 binding" in str(exc)
    else:
        raise AssertionError("mismatched direct Substrate destination binding hash was accepted")

    try:
        module._json_summary(substrate_args(module), bytes.fromhex("ee" * 32), False)
    except ValueError as exc:
        assert "canonical SORA -> sora2 binding" in str(exc)
    else:
        raise AssertionError("mismatched direct Substrate JSON binding hash was accepted")

    bad_code_args = substrate_args(module)
    bad_code_args.verifier_code_hash = bytes(32)
    bad_code_args.runtime_code_hex = None
    try:
        module.render_toml(bad_code_args)
    except ValueError as exc:
        assert "verifier_code_hash must not be zero" in str(exc)
    else:
        raise AssertionError("zero direct Substrate verifier code hash was accepted")

    try:
        module._json_summary(
            bad_code_args,
            bytes.fromhex(SUBSTRATE_SORA2_DESTINATION_BINDING_VECTOR),
            False,
        )
    except ValueError as exc:
        assert "verifier_code_hash must not be zero" in str(exc)
    else:
        raise AssertionError("zero direct Substrate JSON verifier code hash was accepted")

    bad_spec_args = substrate_args(module)
    bad_spec_args.runtime_spec_name = "sora-polkadot"
    try:
        module.render_toml(bad_spec_args)
    except ValueError as exc:
        message = str(exc)
        assert "runtime specName" in message
        assert "destination domain" in message
        assert "sora2" in message
        assert "sora-polkadot" in message
    else:
        raise AssertionError("foreign direct Substrate runtime specName was accepted")

    try:
        module._json_summary(
            bad_spec_args,
            bytes.fromhex(SUBSTRATE_SORA2_DESTINATION_BINDING_VECTOR),
            True,
        )
    except ValueError as exc:
        message = str(exc)
        assert "runtime specName" in message
        assert "destination domain" in message
        assert "sora2" in message
        assert "sora-polkadot" in message
    else:
        raise AssertionError(
            "foreign direct Substrate JSON runtime specName was accepted"
        )

    padded_spec_args = substrate_args(module)
    padded_spec_args.runtime_spec_name = " sora2 "
    try:
        module.render_toml(padded_spec_args)
    except ValueError as exc:
        assert "--toml requires --runtime-spec-name" in str(exc)
    else:
        raise AssertionError("padded direct Substrate runtime specName was accepted")
    assert module._toml_runtime_metadata_ready(padded_spec_args) is False

    bool_spec_args = substrate_args(module)
    bool_spec_args.runtime_spec_version = True
    try:
        module.render_toml(bool_spec_args)
    except ValueError as exc:
        assert "--toml requires --runtime-spec-version" in str(exc)
    else:
        raise AssertionError("boolean Substrate runtime specVersion was accepted")
    assert module._toml_runtime_metadata_ready(bool_spec_args) is False

    bool_transaction_args = substrate_args(module)
    bool_transaction_args.runtime_transaction_version = False
    try:
        module._json_summary(
            bool_transaction_args,
            bytes.fromhex(SUBSTRATE_SORA2_DESTINATION_BINDING_VECTOR),
            True,
        )
    except ValueError as exc:
        assert "--json requires --runtime-transaction-version" in str(exc)
    else:
        raise AssertionError(
            "boolean Substrate runtime transactionVersion was accepted"
        )
    assert module._toml_runtime_metadata_ready(bool_transaction_args) is False

    bad_allowlist_args = substrate_args(module)
    bad_allowlist_args.route_allowlist_hash = bytes(32)
    try:
        module.render_toml(bad_allowlist_args)
    except ValueError as exc:
        assert "route_allowlist_hash must not be zero" in str(exc)
    else:
        raise AssertionError("zero direct Substrate route allowlist hash was accepted")

    try:
        module._json_summary(
            bad_allowlist_args,
            bytes.fromhex(SUBSTRATE_SORA2_DESTINATION_BINDING_VECTOR),
            False,
        )
    except ValueError as exc:
        assert "route_allowlist_hash must not be zero" in str(exc)
    else:
        raise AssertionError("zero direct Substrate JSON route allowlist hash was accepted")

    drifted_allowlist_args = substrate_args(module)
    drifted_allowlist_args.route_allowlist_hash = bytes.fromhex("dd" * 32)
    try:
        module.render_toml(drifted_allowlist_args)
    except ValueError as exc:
        assert "canonical source, deployment, and destination evidence" in str(exc)
    else:
        raise AssertionError("drifted direct Substrate route allowlist hash was accepted")

    try:
        module._json_summary(
            drifted_allowlist_args,
            bytes.fromhex(SUBSTRATE_SORA2_DESTINATION_BINDING_VECTOR),
            False,
        )
    except ValueError as exc:
        assert "canonical source, deployment, and destination evidence" in str(exc)
    else:
        raise AssertionError("drifted direct Substrate JSON route hash was accepted")

    missing_canary_args = substrate_args(module)
    missing_canary_args.route_canary_evidence_hash = None
    try:
        module.render_toml(missing_canary_args)
    except ValueError as exc:
        assert "--route-canary-evidence-hash" in str(exc)
    else:
        raise AssertionError("Substrate destination TOML accepted without route canary evidence")

    missing_canary_summary = module._json_summary(
        missing_canary_args,
        bytes.fromhex(SUBSTRATE_SORA2_DESTINATION_BINDING_VECTOR),
        True,
    )
    assert missing_canary_summary["toml_ready"] is False
    assert "route_canary" not in missing_canary_summary

    for attr_name, label in (
        ("source_verifier_material_hash", "source_verifier_material_hash"),
        (
            "source_adapter_engine_deployment_hash",
            "source_adapter_engine_deployment_hash",
        ),
    ):
        replay_args = substrate_args(module)
        replay_args.route_canary_evidence_hash = getattr(replay_args, attr_name)
        try:
            module.render_toml(replay_args)
        except ValueError as exc:
            assert label in str(exc)
        else:
            raise AssertionError(
                f"Substrate destination TOML accepted route canary replay of {label}"
            )

        try:
            module._json_summary(
                replay_args,
                bytes.fromhex(SUBSTRATE_SORA2_DESTINATION_BINDING_VECTOR),
                True,
            )
        except ValueError as exc:
            assert label in str(exc)
        else:
            raise AssertionError(
                f"Substrate destination JSON accepted route canary replay of {label}"
            )

    bad_entrypoint_args = substrate_args(module)
    bad_entrypoint_args.verifier_entrypoint = "SccpBridge.other_call"
    try:
        module.render_toml(bad_entrypoint_args)
    except ValueError as exc:
        assert "verifier entrypoint must be" in str(exc)
    else:
        raise AssertionError("wrong direct Substrate verifier entrypoint was accepted")

    try:
        module._json_summary(
            bad_entrypoint_args,
            bytes.fromhex(SUBSTRATE_SORA2_DESTINATION_BINDING_VECTOR),
            False,
        )
    except ValueError as exc:
        assert "verifier entrypoint must be" in str(exc)
    else:
        raise AssertionError("wrong direct Substrate JSON entrypoint was accepted")


def test_substrate_cli_json_summary_and_toml_output(capsys):
    module = load_evidence_module()
    runtime_hash = module.substrate_runtime_code_hash(SUBSTRATE_RUNTIME_CODE).hex()
    runtime_code_base64 = base64.b64encode(SUBSTRATE_RUNTIME_CODE).decode("ascii")
    route_canary_hash = substrate_route_canary_evidence_hash(module, domain=6)
    args = [
        "--domain",
        "sora-kusama",
        "--verifier-code-hash",
        "0x" + "bb" * 32,
        "--finalized-head",
        "0x" + "55" * 32,
        "--runtime-spec-name",
        "sora-kusama",
        "--runtime-spec-version",
        "1234",
        "--runtime-transaction-version",
        "7",
        "--route-allowlist-hash",
        "0x" + SUBSTRATE_ROUTE_ALLOWLIST_HASH_VECTORS[6],
        "--source-verifier-material-hash",
        "0x" + SOURCE_VERIFIER_MATERIAL_HASH,
        "--source-adapter-engine-deployment-hash",
        "0x" + SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH,
    ]
    binding_only_args = args[:4]
    runtime_args = [
        "--domain",
        "sora-kusama",
        "--verifier-code-hash",
        "0x" + runtime_hash,
        "--runtime-code-base64",
        runtime_code_base64,
        *args[4:],
    ]
    full_args_without_canary = [
        *runtime_args,
        "--expected-destination-binding-hash",
        "0x" + SUBSTRATE_KUSAMA_DESTINATION_BINDING_VECTOR,
    ]
    full_args = [
        *full_args_without_canary,
        "--route-canary-evidence-hash",
        "0x" + route_canary_hash,
    ]

    assert module.main(binding_only_args) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["domain"] == 6
    assert output["chain"] == "sora-kusama"
    assert output["verifier_plan"] == "SubstrateRuntimeNativeRecursive"
    assert output["verifier_identity"] == SUBSTRATE_RUNTIME_VERIFIER_ID
    assert output["verifier_code_hash"] == "0x" + "bb" * 32
    assert output["destination_binding_key"] == (
        "sccp:0:6:sora-kusama:substrate-runtime-v1:5"
    )
    assert output["destination_binding_hash"] == (
        "0x" + SUBSTRATE_KUSAMA_DESTINATION_BINDING_VECTOR
    )
    assert output["expected_destination_binding_hash_matches"] is False
    assert output["toml_ready"] is False
    assert "route_allowlist_hash" not in output

    try:
        module.main(args)
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("unpinned Substrate route allowlist hash was accepted")

    try:
        module.main(
            [
                *binding_only_args,
                "--expected-destination-binding-hash",
                "0x" + SUBSTRATE_KUSAMA_DESTINATION_BINDING_VECTOR,
                "--route-allowlist-hash",
                "0x" + SUBSTRATE_ROUTE_ALLOWLIST_HASH_VECTORS[6],
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("partial Substrate route allowlist evidence was accepted")

    assert module.main(full_args_without_canary) == 0
    no_canary = json.loads(capsys.readouterr().out)
    assert no_canary["expected_destination_binding_hash_matches"] is True
    assert no_canary["expected_route_allowlist_hash_matches"] is True
    assert no_canary["toml_ready"] is False
    assert "route_canary" not in no_canary

    try:
        module.main([*full_args_without_canary, "--toml"])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("Substrate destination TOML rendered without route canary evidence")

    assert module.main(full_args) == 0
    matched = json.loads(capsys.readouterr().out)
    assert matched["expected_destination_binding_hash_matches"] is True
    assert matched["verifier_code_hash"] == "0x" + runtime_hash
    assert matched["toml_ready"] is True
    assert matched["route_allowlist_hash"] == (
        "0x" + SUBSTRATE_ROUTE_ALLOWLIST_HASH_VECTORS[6]
    )
    assert matched["expected_route_allowlist_hash"] == (
        "0x" + SUBSTRATE_ROUTE_ALLOWLIST_HASH_VECTORS[6]
    )
    assert matched["expected_route_allowlist_hash_matches"] is True
    assert matched["route_canary"]["status"] == "passed"
    assert matched["route_canary"]["evidence_hash"] == (
        "0x" + route_canary_hash
    )

    try:
        module.main([*args, "--toml"])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("unpinned Substrate destination TOML was accepted")

    assert module.main([*full_args, "--toml"]) == 0
    rendered = capsys.readouterr().out
    assert '# sccp_substrate_finalized_head = "0x' + "55" * 32 in rendered
    assert '# sccp_substrate_runtime_spec_name = "sora-kusama"' in rendered
    assert '# sccp_substrate_runtime_spec_version = "1234"' in rendered
    assert '# sccp_substrate_runtime_transaction_version = "7"' in rendered
    assert '# sccp_substrate_runtime_code_hash = "0x' + runtime_hash in rendered
    assert "[[zk.sccp_destination_rollouts]]" in rendered
    assert "[[zk.sccp_route_allowlists]]" in rendered

    try:
        module.main([*args, "--expected-destination-binding-hash", "0x" + "ee" * 32])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("mismatched Substrate destination binding hash was accepted")

    bad_route_args = [
        (
            value
            if value != "0x" + SUBSTRATE_ROUTE_ALLOWLIST_HASH_VECTORS[6]
            else "0x" + "dd" * 32
        )
        for value in full_args
    ]
    try:
        module.main(bad_route_args)
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("mismatched Substrate route allowlist hash was accepted")


def test_substrate_cli_derives_verifier_code_hash_from_runtime_code(
    capsys,
    tmp_path,
):
    module = load_evidence_module()
    runtime_code = b"\x00asm\x01\x00sora-runtime"
    runtime_hash = module.substrate_runtime_code_hash(runtime_code).hex()
    route_canary_hash = substrate_route_canary_evidence_hash(
        module,
        domain=6,
        runtime_code=runtime_code,
    )
    runtime_path = tmp_path / "runtime.wasm"
    runtime_path.write_bytes(runtime_code)
    args = [
        "--domain",
        "sora-kusama",
        "--runtime-code-file",
        str(runtime_path),
        "--finalized-head",
        "0x" + "55" * 32,
        "--runtime-spec-name",
        "sora-kusama",
        "--runtime-spec-version",
        "1234",
        "--runtime-transaction-version",
        "7",
        "--route-allowlist-hash",
        "0x" + SUBSTRATE_ROUTE_ALLOWLIST_HASH_VECTORS[6],
        "--source-verifier-material-hash",
        "0x" + SOURCE_VERIFIER_MATERIAL_HASH,
        "--source-adapter-engine-deployment-hash",
        "0x" + SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH,
        "--expected-destination-binding-hash",
        "0x" + SUBSTRATE_KUSAMA_DESTINATION_BINDING_VECTOR,
        "--route-canary-evidence-hash",
        "0x" + route_canary_hash,
    ]

    assert module.main(args) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["verifier_code_hash"] == "0x" + runtime_hash
    assert output["toml_ready"] is True

    hex_args = [
        value if value != "--runtime-code-file" else "--runtime-code-hex"
        for value in args
    ]
    hex_args[hex_args.index(str(runtime_path))] = "0x" + runtime_code.hex()
    assert module.main(hex_args) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["verifier_code_hash"] == "0x" + runtime_hash

    try:
        module.main([*hex_args, "--verifier-code-hash", "0x" + "bb" * 32])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("mismatched Substrate runtime code hash was accepted")


def test_substrate_direct_renderers_derive_verifier_code_hash_from_runtime_code():
    module = load_evidence_module()
    runtime_code = b"\x00asm\x01\x00sora-runtime"
    runtime_hash = module.substrate_runtime_code_hash(runtime_code)

    args = substrate_args(module, domain=6)
    args.verifier_code_hash = None
    args.runtime_code_hex = runtime_code
    rendered = module.render_toml(args)
    assert 'verifier_code_hash = "0x' + runtime_hash.hex() + '"' in rendered
    assert (
        '# sccp_substrate_runtime_code_base64 = "'
        + base64.b64encode(runtime_code).decode("ascii")
        + '"'
        in rendered
    )
    assert args.verifier_code_hash == runtime_hash

    summary_args = substrate_args(module, domain=6)
    summary_args.verifier_code_hash = None
    summary_args.runtime_code_hex = None
    summary_args.runtime_code_file = runtime_code
    summary = module._json_summary(
        summary_args,
        bytes.fromhex(SUBSTRATE_KUSAMA_DESTINATION_BINDING_VECTOR),
        True,
    )
    assert summary["verifier_code_hash"] == "0x" + runtime_hash.hex()
    assert summary["runtime_code_base64"] == base64.b64encode(runtime_code).decode(
        "ascii"
    )
