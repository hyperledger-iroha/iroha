import json
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
from types import SimpleNamespace


SUBSTRATE_SOURCE_ADAPTER_VERIFIER_VK_HASHES = {
    6: "f7768653132995511594e6e7edb4af22f78bba615650d9dda72f14bb18984daf",
    7: "4f8456bf8626436a16d763c40bf23dffb962232f0766c4ae33d6e594f8be1635",
    8: "96bbfa08489249b28a1444d0dcb9d5b4023bd688091f31c0b435601dad48dbb4",
}
SUBSTRATE_SOURCE_VERIFIER_MATERIAL_HASHES = {
    6: "012c66498a85190d6075c441fad30fe01816796ee1713838fe8bb97f2ad1c924",
    7: "40cd55d64e92d688b839242e170f1722485cddf2e42b4ff22e53c5e7723e570d",
    8: "6fc968441106993502dd05ebeadea1dbfee0f7814680f1ad006d4584c99a8a2d",
}
SUBSTRATE_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASHES = {
    6: "da47a31715813ef5bff0882cd0e0e8b0cc89d426e005e37e0f94a2bdba2043cd",
    7: "2a57fe4beb69e8201299f2c01259a025cafc8388bb38e2a727c2fc872893e13a",
    8: "dac819bff0aa57f7596f06297dfec39027aaab63213497020b772c355a6eaecb",
}
SUBSTRATE_RUNTIME_STORAGE_GATE_HASHES = {
    6: "ddc8810dfb1ff75b37f80db8f77ab3d8a765c59db422ce9e433ba3d56ae9b841",
    7: "f35f6069d12a12c941858026634815aa02567414df8105f44769dd17d1b3e9b4",
    8: "c149b8f8e7f626085304c7ec172462403dc08c0f27368d826a60c4c744b9fafa",
}
SUBSTRATE_TEMPLATE_SOURCE_STATE_VERIFIER_HASHES = {
    6: "af2d28b3e07447239f28e90ce4fdee7e6cd3778c087eaeda7170781eb4b76b9c",
    7: "664576f1a2409099c3b7dba82512c8757501f2869aedda0e45f858572b940b5d",
    8: "20509eb56524c727b6d028cc6b43f10c17048d31b92d5a96d41c0512d16267ef",
}


def load_evidence_module():
    script_path = (
        Path(__file__).resolve().parents[2]
        / "scripts"
        / "sccp_substrate_source_evidence.py"
    )
    spec = spec_from_file_location("sccp_substrate_source_evidence", script_path)
    module = module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def substrate_args(module, domain=8):
    return SimpleNamespace(
        domain=domain,
        target_domain=0,
        source_trust_anchor_hash=bytes.fromhex("44" * 32),
        consensus_verifier_hash=bytes.fromhex("55" * 32),
        message_inclusion_verifier_hash=bytes.fromhex("66" * 32),
        source_state_verifier_hash=bytes.fromhex("77" * 32),
        finality_policy_hash=bytes.fromhex("88" * 32),
        adapter_verifier_vk_hash=bytes.fromhex(
            SUBSTRATE_SOURCE_ADAPTER_VERIFIER_VK_HASHES[domain]
        ),
        deployment_receipt_hash=bytes.fromhex("aa" * 32),
        expected_source_verifier_material_hash=bytes.fromhex(
            SUBSTRATE_SOURCE_VERIFIER_MATERIAL_HASHES[domain]
        ),
        expected_source_adapter_engine_deployment_hash=bytes.fromhex(
            SUBSTRATE_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASHES[domain]
        ),
        expected_runtime_storage_gate_hash=bytes.fromhex(
            SUBSTRATE_RUNTIME_STORAGE_GATE_HASHES[domain]
        ),
    )


def test_substrate_domain_parser_accepts_runtime_lanes_only():
    module = load_evidence_module()

    assert module.parse_substrate_domain("sora-kusama") == 6
    assert module.parse_substrate_domain("sora-polkadot") == 7
    assert module.parse_substrate_domain("sora2") == 8

    try:
        module.parse_substrate_domain(" sora2 ")
    except module.argparse.ArgumentTypeError as exc:
        assert "domain must be sora-kusama, sora-polkadot, or sora2" in str(exc)
    else:
        raise AssertionError("padded Substrate source domain was accepted")

    try:
        module.parse_substrate_domain("tron")
    except module.argparse.ArgumentTypeError as exc:
        assert "domain must be sora-kusama, sora-polkadot, or sora2" in str(exc)
    else:
        raise AssertionError("non-Substrate source domain was accepted")


def test_substrate_source_target_domain_parser_requires_canonical_ascii_decimal():
    module = load_evidence_module()

    assert module.parse_u32("0", label="target domain") == 0
    assert module.parse_u32("8", label="source domain") == 8

    for value in ("08", "0x8", "+8", " 8 ", "٨"):
        try:
            module.parse_u32(value, label="source domain")
        except module.argparse.ArgumentTypeError as exc:
            assert "must be a u32" in str(exc)
        else:
            raise AssertionError(f"noncanonical Substrate domain {value!r} was accepted")


def test_substrate_hex_parser_rejects_zero_and_wrong_width():
    module = load_evidence_module()

    assert module.parse_hex_bytes(
        "0x" + "33" * 32,
        label="source trust anchor hash",
        byte_length=32,
    ) == bytes.fromhex("33" * 32)

    try:
        module.parse_hex_bytes(
            " 0x" + "33" * 32,
            label="source trust anchor hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must not contain whitespace" in str(exc)
    else:
        raise AssertionError("padded Substrate source hash was accepted")

    try:
        module.parse_hex_bytes(
            "0x" + "00" * 32,
            label="source trust anchor hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must not be zero" in str(exc)
    else:
        raise AssertionError("zero Substrate source hash was accepted")

    try:
        module.parse_hex_bytes(
            "0x" + "33" * 31,
            label="source trust anchor hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must be 32 bytes" in str(exc)
    else:
        raise AssertionError("short Substrate source hash was accepted")


def test_substrate_toml_rendering_carries_runtime_profile_ids():
    module = load_evidence_module()
    rendered = module.render_toml(substrate_args(module))

    assert (
        '# sccp_substrate_source_verifier_material_hash = "0x'
        + SUBSTRATE_SOURCE_VERIFIER_MATERIAL_HASHES[8]
        + '"'
        in rendered
    )
    assert "[[zk.sccp_source_verifier_materials]]" in rendered
    assert (
        '# sccp_substrate_source_adapter_engine_deployment_hash = "0x'
        + SUBSTRATE_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASHES[8]
        + '"'
        in rendered
    )
    assert "[[zk.sccp_source_adapter_engine_deployments]]" in rendered
    assert "source_domain = 8" in rendered
    assert "target_domain = 0" in rendered
    assert 'source_chain = "sora2"' in rendered
    assert 'source_proof_plan = "SubstrateGrandpaEventProof"' in rendered
    assert 'finality_model = "SubstrateGrandpa"' in rendered
    assert (
        'source_trust_anchor_id = "sccp:sora2:source-trust-anchor:grandpa-authority-set:v1"'
        in rendered
    )
    assert (
        'message_inclusion_verifier_id = "sccp:sora2:message-inclusion-verifier:events-storage-proof:v1"'
        in rendered
    )
    assert (
        'source_state_verifier_id = "sccp:sora2:source-state-verifier:runtime-storage-proof:v1"'
        in rendered
    )
    assert 'source_state_verifier_hash = "0x' + "77" * 32 + '"' in rendered
    for domain, expected_hash in SUBSTRATE_SOURCE_ADAPTER_VERIFIER_VK_HASHES.items():
        assert module.substrate_source_adapter_verifier_vk_hash(domain).hex() == (
            expected_hash
        )
    assert (
        'adapter_verifier_vk_hash = "0x'
        + SUBSTRATE_SOURCE_ADAPTER_VERIFIER_VK_HASHES[8]
        + '"'
        in rendered
    )
    assert 'deployment_receipt_hash = "0x' + "aa" * 32 + '"' in rendered

    kusama = module.render_toml(substrate_args(module, domain=6))
    assert 'source_chain = "sora-kusama"' in kusama
    assert (
        'source_trust_anchor_id = "sccp:sora-kusama:source-trust-anchor:grandpa-authority-set:v1"'
        in kusama
    )


def test_substrate_source_evidence_rejects_boolean_target_domain():
    module = load_evidence_module()
    args = substrate_args(module)
    args.target_domain = False

    try:
        module._json_summary(args)
    except SystemExit as exc:
        assert "target_domain must be an exact u32" in str(exc)
    else:
        raise AssertionError("boolean Substrate target domain was accepted")

    try:
        module.substrate_source_adapter_verifier_vk_hash(8, target_domain=False)
    except ValueError as exc:
        assert "target_domain must be an exact u32" in str(exc)
    else:
        raise AssertionError("boolean Substrate target domain reached vk hash derivation")


def test_substrate_toml_rendering_rejects_reused_role_hashes():
    module = load_evidence_module()
    args = substrate_args(module)
    args.deployment_receipt_hash = args.adapter_verifier_vk_hash

    try:
        module.render_toml(args)
    except SystemExit as exc:
        assert "deployment_receipt_hash matches adapter_verifier_vk_hash" in str(exc)
    else:
        raise AssertionError("Substrate TOML accepted reused source-adapter role hashes")


def test_substrate_direct_record_hashes_reject_reused_role_hashes():
    module = load_evidence_module()

    material_args = substrate_args(module)
    material_args.consensus_verifier_hash = material_args.source_trust_anchor_hash
    try:
        module.substrate_source_verifier_material_record_hash(material_args)
    except SystemExit as exc:
        assert "consensus_verifier_hash matches source_trust_anchor_hash" in str(exc)
    else:
        raise AssertionError("Substrate material hash accepted reused role hashes")

    deployment_args = substrate_args(module)
    deployment_args.deployment_receipt_hash = deployment_args.adapter_verifier_vk_hash
    try:
        module.substrate_source_adapter_engine_deployment_record_hash(deployment_args)
    except SystemExit as exc:
        assert "deployment_receipt_hash matches adapter_verifier_vk_hash" in str(exc)
    else:
        raise AssertionError("Substrate deployment hash accepted reused role hashes")


def test_substrate_source_record_hashes_match_rust_vectors():
    module = load_evidence_module()

    for domain in (6, 7, 8):
        args = substrate_args(module, domain=domain)
        assert (
            module.substrate_source_verifier_material_record_hash(args).hex()
            == SUBSTRATE_SOURCE_VERIFIER_MATERIAL_HASHES[domain]
        )
        assert (
            module.substrate_source_adapter_engine_deployment_record_hash(args).hex()
            == SUBSTRATE_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASHES[domain]
        )
        assert (
            module.substrate_runtime_storage_gate_hash(args).hex()
            == SUBSTRATE_RUNTIME_STORAGE_GATE_HASHES[domain]
        )
        profile = module.SUBSTRATE_SOURCE_PROFILES[domain]
        assert (
            module._substrate_template_component_hash(
                domain,
                profile["source_state_verifier_id"],
                "source-state-verifier",
            ).hex()
            == SUBSTRATE_TEMPLATE_SOURCE_STATE_VERIFIER_HASHES[domain]
        )


def test_substrate_direct_record_hashes_reject_zero_production_hashes():
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
        args = substrate_args(module)
        setattr(args, field, bytes(32))
        try:
            module.substrate_source_verifier_material_record_hash(args)
        except ValueError as exc:
            assert f"{field} must not be zero" in str(exc)
        else:
            raise AssertionError(f"Substrate material hash accepted zero {field}")

    for field in deployment_fields:
        args = substrate_args(module)
        setattr(args, field, bytes(32))
        try:
            module.substrate_source_adapter_engine_deployment_record_hash(args)
        except ValueError as exc:
            assert f"{field} must not be zero" in str(exc)
        else:
            raise AssertionError(f"Substrate deployment hash accepted zero {field}")


def test_substrate_direct_record_hashes_reject_template_component_hashes():
    module = load_evidence_module()
    for domain in (6, 7, 8):
        profile = module.SUBSTRATE_SOURCE_PROFILES[domain]
        for field, id_key, component_kind in module._template_hash_fields():
            template_hash = module._substrate_template_component_hash(
                domain,
                profile[id_key],
                component_kind,
            )
            label = field.replace("_", " ")

            material_args = substrate_args(module, domain=domain)
            setattr(material_args, field, template_hash)
            try:
                module.substrate_source_verifier_material_record_hash(material_args)
            except SystemExit as exc:
                assert f"template-derived {label}" in str(exc)
            else:
                raise AssertionError(
                    f"Substrate material hash accepted template {label}"
                )

            deployment_args = substrate_args(module, domain=domain)
            setattr(deployment_args, field, template_hash)
            try:
                module.substrate_source_adapter_engine_deployment_record_hash(
                    deployment_args
                )
            except SystemExit as exc:
                assert f"template-derived {label}" in str(exc)
            else:
                raise AssertionError(
                    f"Substrate deployment hash accepted template {label}"
                )


def test_substrate_source_deployment_hash_rejects_noncanonical_adapter_vk_hash():
    module = load_evidence_module()
    args = substrate_args(module)
    args.adapter_verifier_vk_hash = bytes.fromhex("99" * 32)

    try:
        module.substrate_source_adapter_engine_deployment_record_hash(args)
    except ValueError as exc:
        assert "canonical sora2 source-adapter verifier profile" in str(exc)
    else:
        raise AssertionError("noncanonical Substrate adapter vk hash was accepted")


def test_substrate_source_evidence_rejects_template_component_hashes():
    module = load_evidence_module()
    for domain in (6, 7, 8):
        profile = module.SUBSTRATE_SOURCE_PROFILES[domain]
        for field, id_key, component_kind in module._template_hash_fields():
            args = substrate_args(module, domain=domain)
            setattr(
                args,
                field,
                module._substrate_template_component_hash(
                    domain,
                    profile[id_key],
                    component_kind,
                ),
            )
            label = field.replace("_", " ")

            try:
                module.render_toml(args)
            except SystemExit as exc:
                assert f"template-derived {label}" in str(exc)
            else:
                raise AssertionError(f"template Substrate {label} was accepted")


def test_substrate_source_evidence_rejects_template_runtime_storage_hash():
    module = load_evidence_module()
    args = substrate_args(module, domain=7)
    profile = module.SUBSTRATE_SOURCE_PROFILES[7]
    args.source_state_verifier_hash = module._substrate_template_component_hash(
        7,
        profile["source_state_verifier_id"],
        "source-state-verifier",
    )

    try:
        module.render_toml(args)
    except SystemExit as exc:
        assert "template-derived source state verifier hash" in str(exc)
    else:
        raise AssertionError("template Substrate runtime storage verifier hash was accepted")


def test_substrate_source_evidence_rejects_adapter_verifier_vk_hash_mismatch():
    module = load_evidence_module()
    args = substrate_args(module)
    args.adapter_verifier_vk_hash = bytes.fromhex("99" * 32)

    try:
        module.render_toml(args)
    except SystemExit as exc:
        assert "canonical sora2 source-adapter verifier profile" in str(exc)
    else:
        raise AssertionError("mismatched Substrate adapter verifier vk hash was accepted")


def test_substrate_cli_json_summary_and_toml_output(capsys):
    module = load_evidence_module()
    args = [
        "--domain",
        "sora-polkadot",
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
        "0x" + SUBSTRATE_SOURCE_ADAPTER_VERIFIER_VK_HASHES[7],
        "--deployment-receipt-hash",
        "0x" + "aa" * 32,
        "--expected-source-verifier-material-hash",
        "0x" + SUBSTRATE_SOURCE_VERIFIER_MATERIAL_HASHES[7],
        "--expected-source-adapter-engine-deployment-hash",
        "0x" + SUBSTRATE_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASHES[7],
        "--expected-runtime-storage-gate-hash",
        "0x" + SUBSTRATE_RUNTIME_STORAGE_GATE_HASHES[7],
    ]
    unpinned_args = args[:-6]

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
        raise AssertionError("unpinned Substrate source TOML was accepted")

    assert module.main(args) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["source_domain"] == 7
    assert output["target_domain"] == 0
    assert output["source_chain"] == "sora-polkadot"
    assert output["source_proof_plan"] == "SubstrateGrandpaEventProof"
    assert (
        output["adapter_verifier_vk_hash"]
        == "0x" + SUBSTRATE_SOURCE_ADAPTER_VERIFIER_VK_HASHES[7]
    )
    assert (
        output["source_state_verifier_id"]
        == "sccp:sora-polkadot:source-state-verifier:runtime-storage-proof:v1"
    )
    assert output["source_state_verifier_hash"] == "0x" + "77" * 32
    assert (
        output["source_verifier_material_hash"]
        == "0x" + SUBSTRATE_SOURCE_VERIFIER_MATERIAL_HASHES[7]
    )
    assert (
        output["source_adapter_engine_deployment_hash"]
        == "0x" + SUBSTRATE_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASHES[7]
    )
    assert (
        output["substrate_runtime_storage_gate_hash"]
        == "0x" + SUBSTRATE_RUNTIME_STORAGE_GATE_HASHES[7]
    )
    assert output["expected_source_verifier_material_hash_matches"] is True
    assert output["expected_source_adapter_engine_deployment_hash_matches"] is True
    assert output["expected_runtime_storage_gate_hash_matches"] is True
    assert output["toml_ready"] is True

    assert module.main([*args, "--toml"]) == 0
    rendered = capsys.readouterr().out
    assert "[[zk.sccp_source_verifier_materials]]" in rendered
    assert (
        '# sccp_substrate_runtime_storage_gate_hash = "0x'
        + SUBSTRATE_RUNTIME_STORAGE_GATE_HASHES[7]
        + '"'
    ) in rendered


def test_substrate_cli_rejects_expected_record_hash_mismatch():
    module = load_evidence_module()
    args = [
        "--domain",
        "sora-polkadot",
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
        "0x" + SUBSTRATE_SOURCE_ADAPTER_VERIFIER_VK_HASHES[7],
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
        raise AssertionError("mismatched Substrate expected material hash was accepted")


def test_substrate_cli_rejects_expected_runtime_storage_gate_hash_mismatch(capsys):
    module = load_evidence_module()
    args = [
        "--domain",
        "sora-polkadot",
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
        "0x" + SUBSTRATE_SOURCE_ADAPTER_VERIFIER_VK_HASHES[7],
        "--deployment-receipt-hash",
        "0x" + "aa" * 32,
        "--expected-source-verifier-material-hash",
        "0x" + SUBSTRATE_SOURCE_VERIFIER_MATERIAL_HASHES[7],
        "--expected-source-adapter-engine-deployment-hash",
        "0x" + SUBSTRATE_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASHES[7],
        "--expected-runtime-storage-gate-hash",
        "0x" + "99" * 32,
    ]

    try:
        module.main(args)
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("mismatched Substrate runtime storage gate hash was accepted")
    assert "--expected-runtime-storage-gate-hash does not match" in capsys.readouterr().err


def test_substrate_cli_requires_expected_runtime_storage_gate_hash_for_toml(capsys):
    module = load_evidence_module()
    args = [
        "--domain",
        "sora-polkadot",
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
        "0x" + SUBSTRATE_SOURCE_ADAPTER_VERIFIER_VK_HASHES[7],
        "--deployment-receipt-hash",
        "0x" + "aa" * 32,
        "--expected-source-verifier-material-hash",
        "0x" + SUBSTRATE_SOURCE_VERIFIER_MATERIAL_HASHES[7],
        "--expected-source-adapter-engine-deployment-hash",
        "0x" + SUBSTRATE_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASHES[7],
    ]

    assert module.main(args) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["expected_source_verifier_material_hash_matches"] is True
    assert output["expected_source_adapter_engine_deployment_hash_matches"] is True
    assert output["expected_runtime_storage_gate_hash_matches"] is False
    assert output["toml_ready"] is False

    try:
        module.main([*args, "--toml"])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("Substrate TOML rendered without expected runtime gate hash")
    assert "--toml requires --expected-runtime-storage-gate-hash" in capsys.readouterr().err


def test_substrate_cli_rejects_non_sora_target():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--domain",
                "sora2",
                "--target-domain",
                "1",
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
                "0x" + SUBSTRATE_SOURCE_ADAPTER_VERIFIER_VK_HASHES[8],
                "--deployment-receipt-hash",
                "0x" + "aa" * 32,
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("non-SORA Substrate source target was accepted")
