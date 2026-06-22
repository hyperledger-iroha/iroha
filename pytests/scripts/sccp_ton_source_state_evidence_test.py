import json
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
from types import SimpleNamespace


TON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR = (
    "f03f70e8cb504e69b0611df224c2783d04d8f4ee93beae7a62e1cd0a49703bad"
)
TON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR = (
    "08b11177113ac2d9f612abdf767a017de560d805e965b3dc32e28c8748ea2ebc"
)
TON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR = (
    "5c4e226c1f4619311762a9c889f8e3b99ea6f020317c2e8a0c76a08d7a70f887"
)
TON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR = (
    "61e5d710ccbc902be00a38a5a80d05c19de97105605a3f93d4f8067862d81f07"
)
TON_FULL_LIGHT_CLIENT_GATE_HASH_VECTOR = (
    "5047e655523aa7ce8db0cc4dfb8f9551b7912c262e0b65177620c494c57faa48"
)


def load_evidence_module():
    script_path = (
        Path(__file__).resolve().parents[2]
        / "scripts"
        / "sccp_ton_source_state_evidence.py"
    )
    spec = spec_from_file_location("sccp_ton_source_state_evidence", script_path)
    module = module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def ton_args(module):
    return SimpleNamespace(
        source_domain=4,
        target_domain=0,
        source_trust_anchor_hash=bytes.fromhex("44" * 32),
        consensus_verifier_hash=bytes.fromhex("55" * 32),
        message_inclusion_verifier_hash=bytes.fromhex("66" * 32),
        source_state_verifier_hash=bytes.fromhex("77" * 32),
        finality_policy_hash=bytes.fromhex("88" * 32),
        adapter_verifier_vk_hash=bytes.fromhex(TON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR),
        deployment_receipt_hash=bytes.fromhex("aa" * 32),
        expected_source_verifier_material_hash=bytes.fromhex(
            TON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
        ),
        expected_source_adapter_engine_deployment_hash=bytes.fromhex(
            TON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
        ),
    )


def test_ton_source_cli_redacts_top_level_exception_details(monkeypatch, capsys):
    module = load_evidence_module()

    for exception_type in (SystemExit, OSError, RuntimeError, TypeError, ValueError):

        def fail_validate(_args, exception_type=exception_type):
            raise exception_type("secret-token /tmp/operator/private-path")

        with monkeypatch.context() as patch:
            patch.setattr(module, "_validate_ton_source_evidence_args", fail_validate)
            try:
                module.main(
                    [
                        "--source-trust-anchor-hash",
                        "0x" + "44" * 32,
                        "--consensus-verifier-hash",
                        "0x" + "55" * 32,
                        "--message-inclusion-verifier-hash",
                        "0x" + "66" * 32,
                        "--source-state-verifier-hash",
                        "0x" + "77" * 32,
                        "--finality-policy-hash",
                        "0x" + "88" * 32,
                        "--adapter-verifier-vk-hash",
                        "0x" + TON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
                        "--deployment-receipt-hash",
                        "0x" + "aa" * 32,
                    ]
                )
            except SystemExit as exc:
                assert exc.code == 2
            else:
                raise AssertionError("TON source CLI accepted top-level render failure")

            captured = capsys.readouterr()
            assert "SCCP TON source-state evidence rendering failed" in captured.err
            assert "secret-token" not in captured.err
            assert "private-path" not in captured.err
            assert exception_type.__name__ not in captured.err


def test_ton_hex_parser_rejects_zero_and_wrong_width():
    module = load_evidence_module()

    assert module.parse_hex_bytes(
        "0x" + "11" * 32,
        label="source state verifier hash",
        byte_length=32,
    ) == bytes.fromhex("11" * 32)

    try:
        module.parse_hex_bytes(
            " 0x" + "11" * 32,
            label="source state verifier hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must not contain whitespace" in str(exc)
    else:
        raise AssertionError("padded TON source-state verifier hash was accepted")

    try:
        module.parse_hex_bytes(
            "0x" + "00" * 32,
            label="source state verifier hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must not be zero" in str(exc)
    else:
        raise AssertionError("zero TON source-state verifier hash was accepted")

    try:
        module.parse_hex_bytes(
            "0x" + "11" * 31,
            label="source state verifier hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must be 32 bytes" in str(exc)
    else:
        raise AssertionError("short TON source-state verifier hash was accepted")


def test_ton_source_hex_parser_redacts_parser_causes():
    """Invalid TON source-state hex inputs must not chain parser payloads."""

    module = load_evidence_module()
    payload = "secret-token-ton-source-hex"

    try:
        module.parse_hex_bytes(
            "0x" + payload + ("a" * (64 - len(payload))),
            label="source state verifier hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        rendered = str(exc)
        assert rendered == "source state verifier hash must be hex"
        assert "secret-token" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("invalid TON source-state hex was accepted")


def test_ton_source_hex_parser_redacts_typeerror_parser_causes(monkeypatch):
    module = load_evidence_module()

    class SecretBytes:
        @staticmethod
        def fromhex(_text):
            raise TypeError("secret-token TON source hex TypeError detail")

    monkeypatch.setattr(module, "bytes", SecretBytes, raising=False)

    try:
        module.parse_hex_bytes(
            "0x" + "11" * 32,
            label="source state verifier hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        rendered = str(exc)
        assert rendered == "source state verifier hash must be hex"
        assert "secret-token" not in rendered
        assert "TypeError" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("TON source-state parser TypeError was accepted")


def test_ton_source_domain_parser_requires_canonical_ascii_decimal():
    module = load_evidence_module()

    assert module.parse_u32("4", label="source domain") == 4
    assert module.parse_u32("0", label="target domain") == 0

    for value in ("04", "0x4", "+4", " 4 ", "٤"):
        try:
            module.parse_u32(value, label="source domain")
        except module.argparse.ArgumentTypeError as exc:
            assert "must be a u32" in str(exc)
        else:
            raise AssertionError(f"noncanonical TON domain {value!r} was accepted")


def test_ton_toml_rendering_carries_mainnet_profile_ids():
    module = load_evidence_module()
    args = ton_args(module)
    args.masterchain_config_verifier_hash = bytes.fromhex("bb" * 32)
    args.validator_set_transition_verifier_hash = bytes.fromhex("cc" * 32)
    args.shard_accounts_dictionary_verifier_hash = bytes.fromhex("dd" * 32)
    args.expected_source_adapter_engine_deployment_hash = bytes.fromhex(
        TON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR
    )
    args.expected_full_light_client_gate_hash = bytes.fromhex(
        TON_FULL_LIGHT_CLIENT_GATE_HASH_VECTOR
    )

    rendered = module.render_toml(args)

    assert (
        '# sccp_ton_source_verifier_material_hash = "0x'
        + TON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
        + '"'
        in rendered
    )
    assert "[[zk.sccp_source_verifier_materials]]" in rendered
    assert (
        '# sccp_ton_source_adapter_engine_deployment_hash = "0x'
        + TON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR
        + '"'
        in rendered
    )
    assert "[[zk.sccp_source_adapter_engine_deployments]]" in rendered
    assert 'source_domain = 4' in rendered
    assert 'target_domain = 0' in rendered
    assert 'source_chain = "ton"' in rendered
    assert 'source_proof_plan = "TonMasterchainShardProof"' in rendered
    assert 'finality_model = "TonMasterchain"' in rendered
    assert (
        'source_state_verifier_id = "sccp:ton:source-state-verifier:shard-state-light-client-mainnet:v1"'
        in rendered
    )
    assert 'source_state_verifier_hash = "0x' + "77" * 32 + '"' in rendered
    assert module.ton_source_adapter_verifier_vk_hash().hex() == (
        TON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
    )
    assert (
        'adapter_verifier_vk_hash = "0x'
        + TON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
        + '"'
        in rendered
    )
    assert 'deployment_receipt_hash = "0x' + "aa" * 32 + '"' in rendered
    assert "# full_light_client_evidence_ready = true" in rendered
    assert (
        'ton_full_light_client_gate_hash = "0x'
        + TON_FULL_LIGHT_CLIENT_GATE_HASH_VECTOR
        + '"'
        in rendered
    )


def test_ton_source_state_evidence_rejects_boolean_target_domain():
    module = load_evidence_module()
    args = ton_args(module)
    args.target_domain = False

    try:
        module._json_summary(args)
    except SystemExit as exc:
        assert "target_domain must be an exact u32" in str(exc)
    else:
        raise AssertionError("boolean TON target domain was accepted")

    try:
        module.ton_source_adapter_verifier_vk_hash(target_domain=False)
    except ValueError as exc:
        assert "target_domain must be an exact u32" in str(exc)
    else:
        raise AssertionError("boolean TON target domain reached vk hash derivation")


def test_ton_toml_rendering_requires_full_light_client_evidence():
    module = load_evidence_module()
    args = ton_args(module)

    try:
        module.render_toml(args)
    except SystemExit as exc:
        assert "requires the masterchain config" in str(exc)
        assert "--expected-full-light-client-gate-hash" in str(exc)
    else:
        raise AssertionError("TON TOML rendered without full light-client evidence")


def test_ton_toml_rendering_rejects_reused_role_hashes():
    module = load_evidence_module()
    args = ton_args(module)
    args.consensus_verifier_hash = args.source_trust_anchor_hash

    try:
        module.render_toml(args)
    except SystemExit as exc:
        assert "consensus_verifier_hash matches source_trust_anchor_hash" in str(exc)
    else:
        raise AssertionError("TON TOML accepted reused source-adapter role hashes")


def test_ton_direct_record_hashes_reject_reused_role_hashes():
    module = load_evidence_module()

    material_args = ton_args(module)
    material_args.consensus_verifier_hash = material_args.source_trust_anchor_hash
    try:
        module.ton_source_verifier_material_record_hash(material_args)
    except SystemExit as exc:
        assert "consensus_verifier_hash matches source_trust_anchor_hash" in str(exc)
    else:
        raise AssertionError("TON material hash accepted reused role hashes")

    deployment_args = ton_args(module)
    deployment_args.deployment_receipt_hash = deployment_args.adapter_verifier_vk_hash
    try:
        module.ton_source_adapter_engine_deployment_record_hash(deployment_args)
    except SystemExit as exc:
        assert "deployment_receipt_hash matches adapter_verifier_vk_hash" in str(exc)
    else:
        raise AssertionError("TON deployment hash accepted reused role hashes")


def test_ton_toml_rendering_rejects_reused_audit_role_hashes():
    module = load_evidence_module()
    args = ton_args(module)
    args.masterchain_config_verifier_hash = bytes.fromhex("bb" * 32)
    args.validator_set_transition_verifier_hash = bytes.fromhex("bb" * 32)
    args.shard_accounts_dictionary_verifier_hash = bytes.fromhex("dd" * 32)

    try:
        module.render_toml(args)
    except SystemExit as exc:
        assert (
            "validator_set_transition_verifier_hash matches "
            "masterchain_config_verifier_hash"
        ) in str(exc)
    else:
        raise AssertionError("TON TOML accepted reused full-light-client audit hashes")


def test_ton_toml_rendering_rejects_audit_hashes_reusing_source_material():
    module = load_evidence_module()

    for audit_field, source_field in (
        ("masterchain_config_verifier_hash", "source_trust_anchor_hash"),
        ("validator_set_transition_verifier_hash", "adapter_verifier_vk_hash"),
        ("shard_accounts_dictionary_verifier_hash", "deployment_receipt_hash"),
    ):
        args = ton_args(module)
        args.masterchain_config_verifier_hash = bytes.fromhex("bb" * 32)
        args.validator_set_transition_verifier_hash = bytes.fromhex("cc" * 32)
        args.shard_accounts_dictionary_verifier_hash = bytes.fromhex("dd" * 32)
        setattr(args, audit_field, getattr(args, source_field))

        try:
            module.render_toml(args)
        except SystemExit as exc:
            assert (
                "full-light-client verifier hashes must not reuse "
                f"existing source-adapter material: {audit_field} matches {source_field}"
            ) in str(exc)
        else:
            raise AssertionError(
                f"TON TOML accepted {audit_field} copied from {source_field}"
            )


def test_ton_source_record_hashes_match_rust_vectors():
    module = load_evidence_module()
    args = ton_args(module)

    assert (
        module.ton_source_verifier_material_record_hash(args).hex()
        == TON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
    )
    assert (
        module.ton_source_adapter_engine_deployment_record_hash(args).hex()
        == TON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
    )
    assert module.ton_full_light_client_gate_hash(args) is None

    args.masterchain_config_verifier_hash = bytes.fromhex("bb" * 32)
    args.validator_set_transition_verifier_hash = bytes.fromhex("cc" * 32)
    args.shard_accounts_dictionary_verifier_hash = bytes.fromhex("dd" * 32)
    assert (
        module.ton_source_adapter_engine_deployment_record_hash(args).hex()
        == TON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR
    )
    assert (
        module.ton_full_light_client_gate_hash(args).hex()
        == TON_FULL_LIGHT_CLIENT_GATE_HASH_VECTOR
    )
    gate_hash = module.ton_full_light_client_gate_hash(args)
    replayed_receipt = SimpleNamespace(**vars(args))
    replayed_receipt.deployment_receipt_hash = bytes.fromhex("ab" * 32)
    assert module.ton_full_light_client_gate_hash(replayed_receipt) != gate_hash


def test_ton_direct_record_hashes_reject_zero_production_hashes():
    module = load_evidence_module()
    material_fields = (
        "source_trust_anchor_hash",
        "consensus_verifier_hash",
        "message_inclusion_verifier_hash",
        "source_state_verifier_hash",
        "finality_policy_hash",
    )
    deployment_fields = (
        *material_fields,
        "adapter_verifier_vk_hash",
        "deployment_receipt_hash",
    )

    for field in material_fields:
        args = ton_args(module)
        setattr(args, field, bytes(32))
        try:
            module.ton_source_verifier_material_record_hash(args)
        except ValueError as exc:
            assert f"{field} must not be zero" in str(exc)
        else:
            raise AssertionError(f"TON material hash accepted zero {field}")

    for field in deployment_fields:
        args = ton_args(module)
        setattr(args, field, bytes(32))
        try:
            module.ton_source_adapter_engine_deployment_record_hash(args)
        except ValueError as exc:
            assert f"{field} must not be zero" in str(exc)
        else:
            raise AssertionError(f"TON deployment hash accepted zero {field}")


def test_ton_direct_light_client_hashes_are_nonzero_and_complete():
    module = load_evidence_module()
    args = ton_args(module)
    args.masterchain_config_verifier_hash = bytes(32)
    args.validator_set_transition_verifier_hash = bytes.fromhex("cc" * 32)
    args.shard_accounts_dictionary_verifier_hash = bytes.fromhex("dd" * 32)

    try:
        module.ton_full_light_client_gate_hash(args)
    except ValueError as exc:
        assert "masterchain_config_verifier_hash must not be zero" in str(exc)
    else:
        raise AssertionError("zero TON full light-client hash was accepted")

    args = ton_args(module)
    args.masterchain_config_verifier_hash = bytes.fromhex("bb" * 32)
    try:
        module.ton_source_adapter_engine_deployment_record_hash(args)
    except SystemExit as exc:
        assert "must include all verifier hashes" in str(exc)
    else:
        raise AssertionError("partial TON full light-client deployment hash was accepted")


def test_ton_direct_record_hashes_reject_template_component_hashes():
    module = load_evidence_module()
    for field, (component_id, component_kind) in module.TON_TEMPLATE_COMPONENTS.items():
        template_hash = module._ton_template_component_hash(
            component_id,
            component_kind,
        )
        label = field.replace("_", " ")

        material_args = ton_args(module)
        setattr(material_args, field, template_hash)
        try:
            module.ton_source_verifier_material_record_hash(material_args)
        except SystemExit as exc:
            assert f"template-derived {label}" in str(exc)
        else:
            raise AssertionError(
                f"TON material hash accepted template {label}"
            )

        deployment_args = ton_args(module)
        setattr(deployment_args, field, template_hash)
        try:
            module.ton_source_adapter_engine_deployment_record_hash(deployment_args)
        except SystemExit as exc:
            assert f"template-derived {label}" in str(exc)
        else:
            raise AssertionError(
                f"TON deployment hash accepted template {label}"
            )


def test_ton_source_deployment_hash_rejects_noncanonical_adapter_vk_hash():
    module = load_evidence_module()
    args = ton_args(module)
    args.adapter_verifier_vk_hash = bytes.fromhex("99" * 32)

    try:
        module.ton_source_adapter_engine_deployment_record_hash(args)
    except ValueError as exc:
        assert "canonical TON source-adapter verifier profile" in str(exc)
    else:
        raise AssertionError("noncanonical TON adapter vk hash was accepted")


def test_ton_source_evidence_rejects_template_component_hashes():
    module = load_evidence_module()
    for field, (component_id, component_kind) in module.TON_TEMPLATE_COMPONENTS.items():
        args = ton_args(module)
        setattr(
            args,
            field,
            module._ton_template_component_hash(
                component_id,
                component_kind,
            ),
        )
        label = field.replace("_", " ")

        try:
            module.render_toml(args)
        except SystemExit as exc:
            assert f"template-derived {label}" in str(exc)
        else:
            raise AssertionError(f"template TON {label} was accepted")


def test_ton_source_evidence_rejects_adapter_verifier_vk_hash_mismatch():
    module = load_evidence_module()
    args = SimpleNamespace(
        source_domain=4,
        target_domain=0,
        source_trust_anchor_hash=bytes.fromhex("44" * 32),
        consensus_verifier_hash=bytes.fromhex("55" * 32),
        message_inclusion_verifier_hash=bytes.fromhex("66" * 32),
        source_state_verifier_hash=bytes.fromhex("77" * 32),
        finality_policy_hash=bytes.fromhex("88" * 32),
        adapter_verifier_vk_hash=bytes.fromhex("99" * 32),
        deployment_receipt_hash=bytes.fromhex("aa" * 32),
    )

    try:
        module.render_toml(args)
    except SystemExit as exc:
        assert "canonical TON source-adapter verifier profile" in str(exc)
    else:
        raise AssertionError("mismatched TON adapter verifier vk hash was accepted")


def test_ton_cli_json_summary_and_toml_output(capsys):
    module = load_evidence_module()
    args = [
        "--source-trust-anchor-hash",
        "0x" + "44" * 32,
        "--consensus-verifier-hash",
        "0x" + "55" * 32,
        "--message-inclusion-verifier-hash",
        "0x" + "66" * 32,
        "--source-state-verifier-hash",
        "0x" + "77" * 32,
        "--finality-policy-hash",
        "0x" + "88" * 32,
        "--adapter-verifier-vk-hash",
        "0x" + TON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
        "--deployment-receipt-hash",
        "0x" + "aa" * 32,
        "--expected-source-verifier-material-hash",
        "0x" + TON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR,
        "--expected-source-adapter-engine-deployment-hash",
        "0x" + TON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR,
    ]
    unpinned_args = args[:-4]

    assert module.main(unpinned_args) == 0
    unpinned = json.loads(capsys.readouterr().out)
    assert unpinned["expected_source_verifier_material_hash_matches"] is False
    assert unpinned["expected_source_adapter_engine_deployment_hash_matches"] is False
    assert unpinned["source_verifier_material_ready"] is False
    assert unpinned["source_adapter_engine_deployment_ready"] is False
    assert (
        unpinned["source_adapter_gate_ready_with_full_light_client_evidence"] is False
    )
    assert unpinned["source_adapter_gate_blockers"] == [
        "source verifier material hash is not pinned or mismatched",
        "source adapter deployment hash is not pinned or mismatched",
        "full light-client verifier hashes are incomplete",
    ]
    assert unpinned["full_toml_ready"] is False
    assert unpinned["toml_ready"] is False

    try:
        module.main([*unpinned_args, "--toml"])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("unpinned TON source TOML was accepted")

    assert module.main(args) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["source_domain"] == 4
    assert output["target_domain"] == 0
    assert output["source_state_verifier_hash"] == "0x" + "77" * 32
    assert (
        output["adapter_verifier_vk_hash"]
        == "0x" + TON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
    )
    assert (
        output["source_verifier_material_hash"]
        == "0x" + TON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
    )
    assert (
        output["source_adapter_engine_deployment_hash"]
        == "0x" + TON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
    )
    assert output["source_adapter_gate_closed_until_full_light_client"] is True
    assert output["source_adapter_gate_ready_with_full_light_client_evidence"] is False
    assert output["source_adapter_gate_blockers"] == [
        "full light-client verifier hashes are incomplete",
    ]
    assert output["full_light_client_evidence_ready"] is False
    assert output["full_light_client_gate_hash"] is None
    assert output["missing_full_light_client_verifier_ids"] == (
        output["full_light_client_verifier_ids"]
    )
    assert output["expected_source_verifier_material_hash_matches"] is True
    assert output["expected_source_adapter_engine_deployment_hash_matches"] is True
    assert output["source_verifier_material_ready"] is True
    assert output["source_adapter_engine_deployment_ready"] is True
    assert output["expected_full_light_client_gate_hash_matches"] is False
    assert output["full_toml_ready"] is False
    assert output["toml_ready"] is False

    try:
        module.main([*args, "--toml"])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("TON source TOML rendered without full light-client evidence")


def test_ton_cli_full_light_client_audit_hash(capsys):
    module = load_evidence_module()
    args = [
        "--source-trust-anchor-hash",
        "0x" + "44" * 32,
        "--consensus-verifier-hash",
        "0x" + "55" * 32,
        "--message-inclusion-verifier-hash",
        "0x" + "66" * 32,
        "--source-state-verifier-hash",
        "0x" + "77" * 32,
        "--finality-policy-hash",
        "0x" + "88" * 32,
        "--adapter-verifier-vk-hash",
        "0x" + TON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
        "--deployment-receipt-hash",
        "0x" + "aa" * 32,
        "--masterchain-config-verifier-hash",
        "0x" + "bb" * 32,
        "--validator-set-transition-verifier-hash",
        "0x" + "cc" * 32,
        "--shard-accounts-dictionary-verifier-hash",
        "0x" + "dd" * 32,
        "--expected-source-verifier-material-hash",
        "0x" + TON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR,
        "--expected-source-adapter-engine-deployment-hash",
        "0x" + TON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR,
        "--expected-full-light-client-gate-hash",
        "0x" + TON_FULL_LIGHT_CLIENT_GATE_HASH_VECTOR,
    ]

    assert module.main(args) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["source_adapter_engine_deployment_hash"] == (
        "0x" + TON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR
    )
    assert output["source_adapter_gate_closed_until_full_light_client"] is True
    assert output["source_adapter_gate_ready_with_full_light_client_evidence"] is True
    assert output["source_adapter_gate_blockers"] == []
    assert output["full_light_client_evidence_ready"] is True
    assert output["full_light_client_gate_hash"] == (
        "0x" + TON_FULL_LIGHT_CLIENT_GATE_HASH_VECTOR
    )
    assert output["missing_full_light_client_verifier_ids"] == []
    assert output["expected_source_verifier_material_hash_matches"] is True
    assert output["expected_source_adapter_engine_deployment_hash_matches"] is True
    assert output["source_verifier_material_ready"] is True
    assert output["source_adapter_engine_deployment_ready"] is True
    assert output["full_toml_ready"] is True
    assert output["toml_ready"] is True
    assert output["full_light_client_verifier_hashes"] == {
        "masterchain_config_verifier_hash": "0x" + "bb" * 32,
        "validator_set_transition_verifier_hash": "0x" + "cc" * 32,
        "shard_accounts_dictionary_verifier_hash": "0x" + "dd" * 32,
    }

    assert module.main([*args, "--toml"]) == 0
    rendered = capsys.readouterr().out
    assert (
        '# sccp_ton_source_adapter_engine_deployment_hash = "0x'
        + TON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR
        + '"'
        in rendered
    )
    assert 'ton_masterchain_config_verifier_hash = "0x' + "bb" * 32 + '"' in rendered
    assert (
        'ton_validator_set_transition_verifier_hash = "0x' + "cc" * 32 + '"'
        in rendered
    )
    assert (
        'ton_shard_accounts_dictionary_verifier_hash = "0x' + "dd" * 32 + '"'
        in rendered
    )
    assert (
        'ton_full_light_client_gate_hash = "0x'
        + TON_FULL_LIGHT_CLIENT_GATE_HASH_VECTOR
        + '"'
        in rendered
    )
    assert "# full_light_client_evidence_ready = true" in rendered


def test_ton_cli_toml_requires_expected_full_light_client_gate_hash(capsys):
    module = load_evidence_module()
    args = [
        "--source-trust-anchor-hash",
        "0x" + "44" * 32,
        "--consensus-verifier-hash",
        "0x" + "55" * 32,
        "--message-inclusion-verifier-hash",
        "0x" + "66" * 32,
        "--source-state-verifier-hash",
        "0x" + "77" * 32,
        "--finality-policy-hash",
        "0x" + "88" * 32,
        "--adapter-verifier-vk-hash",
        "0x" + TON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
        "--deployment-receipt-hash",
        "0x" + "aa" * 32,
        "--masterchain-config-verifier-hash",
        "0x" + "bb" * 32,
        "--validator-set-transition-verifier-hash",
        "0x" + "cc" * 32,
        "--shard-accounts-dictionary-verifier-hash",
        "0x" + "dd" * 32,
        "--expected-source-verifier-material-hash",
        "0x" + TON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR,
        "--expected-source-adapter-engine-deployment-hash",
        "0x" + TON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR,
    ]

    assert module.main(args) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["source_adapter_gate_ready_with_full_light_client_evidence"] is False
    assert output["source_adapter_gate_blockers"] == [
        "full light-client gate hash is not pinned or mismatched",
    ]
    assert output["full_light_client_evidence_ready"] is True
    assert output["expected_full_light_client_gate_hash_matches"] is False
    assert output["source_verifier_material_ready"] is True
    assert output["source_adapter_engine_deployment_ready"] is True
    assert output["full_toml_ready"] is False
    assert output["toml_ready"] is False

    try:
        module.main([*args, "--toml"])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError(
            "TON TOML rendering accepted audit evidence without an expected gate hash"
        )


def test_ton_cli_rejects_partial_full_light_client_audit_evidence():
    module = load_evidence_module()
    args = [
        "--source-trust-anchor-hash",
        "0x" + "44" * 32,
        "--consensus-verifier-hash",
        "0x" + "55" * 32,
        "--message-inclusion-verifier-hash",
        "0x" + "66" * 32,
        "--source-state-verifier-hash",
        "0x" + "77" * 32,
        "--finality-policy-hash",
        "0x" + "88" * 32,
        "--adapter-verifier-vk-hash",
        "0x" + TON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
        "--deployment-receipt-hash",
        "0x" + "aa" * 32,
        "--masterchain-config-verifier-hash",
        "0x" + "bb" * 32,
    ]

    try:
        module.main(args)
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("partial TON full light-client audit evidence was accepted")

    direct_args = ton_args(module)
    direct_args.masterchain_config_verifier_hash = bytes.fromhex("bb" * 32)
    try:
        module._require_full_light_client_evidence_consistency(direct_args)
    except SystemExit as exc:
        assert "must include all verifier hashes" in str(exc)
    else:
        raise AssertionError("partial direct TON audit evidence was accepted")


def test_ton_cli_rejects_full_light_client_audit_hash_reusing_template_material():
    module = load_evidence_module()
    args = ton_args(module)
    args.masterchain_config_verifier_hash = module._ton_template_component_hash(
        module.TON_SOURCE_TRUST_ANCHOR_ID,
        "source-trust-anchor",
    )
    args.validator_set_transition_verifier_hash = bytes.fromhex("cc" * 32)
    args.shard_accounts_dictionary_verifier_hash = bytes.fromhex("dd" * 32)

    try:
        module.ton_full_light_client_gate_hash(args)
    except SystemExit as exc:
        assert "built-in template material" in str(exc)
    else:
        raise AssertionError(
            "TON full light-client audit hash reused template material"
        )


def test_ton_cli_rejects_full_light_client_audit_hash_mismatch():
    module = load_evidence_module()
    args = [
        "--source-trust-anchor-hash",
        "0x" + "44" * 32,
        "--consensus-verifier-hash",
        "0x" + "55" * 32,
        "--message-inclusion-verifier-hash",
        "0x" + "66" * 32,
        "--source-state-verifier-hash",
        "0x" + "77" * 32,
        "--finality-policy-hash",
        "0x" + "88" * 32,
        "--adapter-verifier-vk-hash",
        "0x" + TON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
        "--deployment-receipt-hash",
        "0x" + "aa" * 32,
        "--masterchain-config-verifier-hash",
        "0x" + "bb" * 32,
        "--validator-set-transition-verifier-hash",
        "0x" + "cc" * 32,
        "--shard-accounts-dictionary-verifier-hash",
        "0x" + "dd" * 32,
        "--expected-full-light-client-gate-hash",
        "0x" + "99" * 32,
    ]

    try:
        module.main(args)
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("mismatched TON full light-client audit hash was accepted")


def test_ton_cli_rejects_expected_record_hash_mismatch():
    module = load_evidence_module()
    args = [
        "--source-trust-anchor-hash",
        "0x" + "44" * 32,
        "--consensus-verifier-hash",
        "0x" + "55" * 32,
        "--message-inclusion-verifier-hash",
        "0x" + "66" * 32,
        "--source-state-verifier-hash",
        "0x" + "77" * 32,
        "--finality-policy-hash",
        "0x" + "88" * 32,
        "--adapter-verifier-vk-hash",
        "0x" + TON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
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
        raise AssertionError("mismatched TON expected material hash was accepted")


def test_ton_cli_rejects_non_production_lane():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--source-domain",
                "5",
                "--source-trust-anchor-hash",
                "0x" + "44" * 32,
                "--consensus-verifier-hash",
                "0x" + "55" * 32,
                "--message-inclusion-verifier-hash",
                "0x" + "66" * 32,
                "--source-state-verifier-hash",
                "0x" + "77" * 32,
                "--finality-policy-hash",
                "0x" + "88" * 32,
                "--adapter-verifier-vk-hash",
                "0x" + TON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
                "--deployment-receipt-hash",
                "0x" + "aa" * 32,
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("non-TON source evidence was accepted")
