import json
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
from types import SimpleNamespace


ETH_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR = (
    "2140903293411cad0f0eb217d8beb18d3a188edf7bba455098589a2409445e46"
)
ETH_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR = (
    "035c5a35f6412d45ed10389741016d067bd6d0b874a38cd744922c599e0a2fdd"
)
ETH_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR = (
    "d08e3344760aabfb4ba891990c852846d04a5735647174ce6e3ab0f2cad57f4d"
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
        deployment_receipt_contract_address=bytes.fromhex("11" * 20),
        deployment_receipt_block_hash=bytes.fromhex("bb" * 32),
        deployment_receipt_block_number=4660,
        expected_source_verifier_material_hash=bytes.fromhex(
            ETH_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
        ),
        expected_source_adapter_engine_deployment_hash=bytes.fromhex(
            ETH_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
        ),
    )


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


def test_eth_hash_parser_rejects_zero_and_wrong_width():
    module = load_evidence_module()

    assert module.parse_hex_bytes(
        "0x" + "44" * 32,
        label="source trust anchor hash",
        byte_length=32,
    ) == bytes.fromhex("44" * 32)

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

    rendered = module.render_toml(args)

    assert '# sccp_evm_source_rpc_chain_id = "1"' in rendered
    assert '# sccp_evm_source_bridge_address = "0x' + "11" * 20 + '"' in rendered
    assert (
        '# sccp_evm_source_bridge_runtime_code_hash = "0x'
        + "77" * 32
        + '"'
        in rendered
    )
    assert (
        '# sccp_evm_source_deployment_transaction_hash = "0x'
        + "de" * 32
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
        '# sccp_eth_source_verifier_material_hash = "0x'
        + ETH_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
        + '"'
        in rendered
    )
    assert "[[zk.sccp_source_verifier_materials]]" in rendered
    assert (
        '# sccp_eth_source_adapter_engine_deployment_hash = "0x'
        + ETH_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
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
    assert 'source_bridge_emitter_code_hash = "0x' + "77" * 32 + '"' in rendered
    assert (
        'adapter_verifier_vk_hash = "0x'
        + ETH_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
        + '"'
        in rendered
    )
    assert 'deployment_receipt_hash = "0x' + "aa" * 32 + '"' in rendered
    assert "source_bridge_network_id" not in rendered
    assert "source_bridge_owner_address" not in rendered
    assert "source_bridge_config_hash" not in rendered


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


def test_eth_cli_json_summary_and_toml_output(capsys):
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
        "--deployment-transaction-hash",
        "0x" + "de" * 32,
        "--deployment-receipt-contract-address",
        "0x" + "11" * 20,
        "--deployment-receipt-block-hash",
        "0x" + "bb" * 32,
        "--deployment-receipt-block-number",
        "4660",
        "--expected-source-verifier-material-hash",
        "0x" + ETH_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR,
        "--expected-source-adapter-engine-deployment-hash",
        "0x" + ETH_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR,
    ]
    unpinned_args = args[:-4]

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

    assert module.main(args) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["source_domain"] == 1
    assert output["target_domain"] == 0
    assert output["source_bridge_emitter_address"] == "0x" + "11" * 20
    assert output["source_bridge_emitter_code_hash"] == "0x" + "77" * 32
    assert (
        output["adapter_verifier_vk_hash"]
        == "0x" + ETH_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
    )
    assert (
        output["source_verifier_material_hash"]
        == "0x" + ETH_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
    )
    assert (
        output["source_adapter_engine_deployment_hash"]
        == "0x" + ETH_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
    )
    assert output["expected_source_verifier_material_hash_matches"] is True
    assert output["expected_source_adapter_engine_deployment_hash_matches"] is True
    assert output["toml_ready"] is True

    assert module.main([*args, "--toml"]) == 0
    rendered = capsys.readouterr().out
    assert '# sccp_evm_source_rpc_chain_id = "1"' in rendered
    assert '# sccp_evm_source_deployment_transaction_hash = "0x' + "de" * 32 in rendered
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
