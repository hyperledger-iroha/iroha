import json
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
from types import SimpleNamespace


SOLANA_TEMPLATE_HASHES = {
    "source_trust_anchor_hash": (
        "113bdb7601d84f2098daec386346a7123857d181b3ac5bd23df50fa9e1b2cbe3"
    ),
    "consensus_verifier_hash": (
        "97ea89019e6c79305d06dfc27640ee14a6b42ba6eaf86e1835ee9b433dba48ba"
    ),
    "message_inclusion_verifier_hash": (
        "b8358bfef1e428a6a7e9115687cb2b88d9c21dad4021bea3e11d43489eb3dcb0"
    ),
    "source_state_verifier_hash": (
        "6b4e4106bbb6b343ae1a4a36c9c68756d4454d2167c9b8b2ee3225e39fb0a48b"
    ),
    "finality_policy_hash": (
        "9df7ea90cf1bbba036788b14804f63f4be1e908390be89524fd4486f74344f56"
    ),
}
SOLANA_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR = (
    "e7bc29d06bf56184183c3fc59a0e934cd1d8e16751f1eda2efaaf88aa350b9d6"
)
SOLANA_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR = (
    "499a7363142d5fcfe3a79b11a29ae2ad897e853649e80e39a162b8942f908331"
)
SOLANA_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR = (
    "cdb2a81cb31e58d9bc1f4292d33c3f4990b2d2008dda1b9b1275aaac087461cc"
)
SOLANA_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR = (
    "97e5c4196aff6387b9d973e663de3ce9345e1d8c3de89d22505b2197e282dc61"
)
SOLANA_FULL_LIGHT_CLIENT_GATE_HASH_VECTOR = (
    "e23b2c175909e222c1ebe371661bda8c0687cf8d7e7acf2b62957a51c420be02"
)


def load_evidence_module():
    script_path = (
        Path(__file__).resolve().parents[2]
        / "scripts"
        / "sccp_solana_source_state_evidence.py"
    )
    spec = spec_from_file_location("sccp_solana_source_state_evidence", script_path)
    module = module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def solana_args(module):
    return SimpleNamespace(
        source_domain=3,
        target_domain=0,
        source_trust_anchor_hash=bytes.fromhex("44" * 32),
        consensus_verifier_hash=bytes.fromhex("55" * 32),
        message_inclusion_verifier_hash=bytes.fromhex("66" * 32),
        source_state_verifier_hash=bytes.fromhex("77" * 32),
        finality_policy_hash=bytes.fromhex("88" * 32),
        adapter_verifier_vk_hash=bytes.fromhex(
            SOLANA_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
        ),
        deployment_receipt_hash=bytes.fromhex("aa" * 32),
        expected_source_verifier_material_hash=bytes.fromhex(
            SOLANA_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
        ),
        expected_source_adapter_engine_deployment_hash=bytes.fromhex(
            SOLANA_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
        ),
    )


def solana_light_client_cli_args():
    return [
        "--tower-replay-verifier-hash",
        "0x" + "bb" * 32,
        "--full-accountsdb-lattice-verifier-hash",
        "0x" + "cc" * 32,
        "--bank-fork-choice-verifier-hash",
        "0x" + "dd" * 32,
    ]


def test_solana_source_cli_redacts_top_level_exception_details(monkeypatch, capsys):
    module = load_evidence_module()

    for exception_type in (SystemExit, OSError, RuntimeError, TypeError, ValueError):

        def fail_validate(_args, exception_type=exception_type):
            raise exception_type("secret-token /tmp/operator/private-path")

        with monkeypatch.context() as patch:
            patch.setattr(module, "_validate_solana_evidence", fail_validate)
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
                        "0x" + SOLANA_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
                        "--deployment-receipt-hash",
                        "0x" + "aa" * 32,
                    ]
                )
            except SystemExit as exc:
                assert exc.code == 2
            else:
                raise AssertionError("Solana source CLI accepted top-level render failure")

            captured = capsys.readouterr()
            assert "SCCP Solana source-state evidence rendering failed" in captured.err
            assert "secret-token" not in captured.err
            assert "private-path" not in captured.err
            assert exception_type.__name__ not in captured.err


def test_solana_hex_parser_rejects_zero_and_wrong_width():
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
        raise AssertionError("padded Solana source-state verifier hash was accepted")

    try:
        module.parse_hex_bytes(
            "0x" + "00" * 32,
            label="source state verifier hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must not be zero" in str(exc)
    else:
        raise AssertionError("zero Solana source-state verifier hash was accepted")

    try:
        module.parse_hex_bytes(
            "0x" + "11" * 31,
            label="source state verifier hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must be 32 bytes" in str(exc)
    else:
        raise AssertionError("short Solana source-state verifier hash was accepted")

    for value, expected in (
        ("11" * 32, "canonical lowercase 0x hex"),
        ("0X" + "11" * 32, "lowercase 0x prefix"),
        ("0x" + "AA" * 32, "lowercase hex"),
    ):
        try:
            module.parse_hex_bytes(
                value,
                label="source state verifier hash",
                byte_length=32,
            )
        except module.argparse.ArgumentTypeError as exc:
            assert expected in str(exc)
        else:
            raise AssertionError(
                f"noncanonical Solana source-state verifier hash {value!r} "
                "was accepted"
            )


def test_solana_source_hex_parser_redacts_parser_causes():
    """Invalid Solana source-state hex inputs must not chain parser payloads."""

    module = load_evidence_module()
    payload = "secret-token-solana-source-hex"

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
        raise AssertionError("invalid Solana source-state hex was accepted")


def test_solana_source_hex_parser_redacts_helper_exit_parser_causes(monkeypatch):
    module = load_evidence_module()

    for exception_type in (SystemExit, RuntimeError, TypeError, ValueError):
        detail = (
            "secret-token Solana source hex TypeError detail"
            if exception_type is TypeError
            else f"secret-token Solana source hex {exception_type.__name__} detail"
        )

        class SecretBytes:
            @staticmethod
            def fromhex(_text, detail=detail, exception_type=exception_type):
                raise exception_type(detail)

        with monkeypatch.context() as patch:
            patch.setattr(module, "bytes", SecretBytes, raising=False)

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
                assert exception_type.__name__ not in rendered
                assert exc.__cause__ is None
                assert exc.__suppress_context__ is True
            else:
                raise AssertionError(
                    "Solana source-state parser "
                    f"{exception_type.__name__} was accepted"
                )


def test_solana_source_domain_parser_requires_canonical_ascii_decimal():
    module = load_evidence_module()

    assert module.parse_u32("3", label="source domain") == 3
    assert module.parse_u32("0", label="target domain") == 0

    for value in ("03", "0x3", "+3", " 3 ", "٣"):
        try:
            module.parse_u32(value, label="source domain")
        except module.argparse.ArgumentTypeError as exc:
            assert "must be a u32" in str(exc)
        else:
            raise AssertionError(f"noncanonical Solana domain {value!r} was accepted")


def test_solana_toml_rendering_carries_mainnet_profile_ids():
    module = load_evidence_module()
    args = solana_args(module)
    args.tower_replay_verifier_hash = bytes.fromhex("bb" * 32)
    args.full_accountsdb_lattice_verifier_hash = bytes.fromhex("cc" * 32)
    args.bank_fork_choice_verifier_hash = bytes.fromhex("dd" * 32)
    args.expected_source_adapter_engine_deployment_hash = bytes.fromhex(
        SOLANA_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR
    )
    args.expected_full_light_client_gate_hash = bytes.fromhex(
        SOLANA_FULL_LIGHT_CLIENT_GATE_HASH_VECTOR
    )
    rendered = module.render_toml(args)

    assert (
        '# sccp_solana_source_verifier_material_hash = "0x'
        + SOLANA_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
        + '"'
        in rendered
    )
    assert "[[zk.sccp_source_verifier_materials]]" in rendered
    assert (
        '# sccp_solana_source_adapter_engine_deployment_hash = "0x'
        + SOLANA_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR
        + '"'
        in rendered
    )
    assert "[[zk.sccp_source_adapter_engine_deployments]]" in rendered
    assert "source_domain = 3" in rendered
    assert "target_domain = 0" in rendered
    assert 'source_chain = "sol"' in rendered
    assert 'source_proof_plan = "SolanaFinalizedTransactionProof"' in rendered
    assert 'finality_model = "SolanaFinalizedSlot"' in rendered
    assert (
        'source_state_verifier_id = "sccp:sol:accounts-db-verifier:accounts-lt-hash-mainnet-beta:v1"'
        in rendered
    )
    assert 'source_state_verifier_hash = "0x' + "77" * 32 + '"' in rendered
    assert module.solana_source_adapter_verifier_vk_hash().hex() == (
        SOLANA_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
    )
    assert (
        'adapter_verifier_vk_hash = "0x'
        + SOLANA_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
        + '"'
        in rendered
    )
    assert 'deployment_receipt_hash = "0x' + "aa" * 32 + '"' in rendered
    assert "# full_light_client_evidence_ready = true" in rendered
    assert "sccp:sol:light-client:tower-replay-mainnet-beta:v1" in rendered


def test_solana_source_state_evidence_rejects_boolean_target_domain():
    module = load_evidence_module()
    args = solana_args(module)
    args.target_domain = False

    try:
        module._json_summary(args)
    except SystemExit as exc:
        assert "target_domain must be an exact u32" in str(exc)
    else:
        raise AssertionError("boolean Solana target domain was accepted")


def test_solana_toml_rendering_rejects_reused_role_hashes():
    module = load_evidence_module()
    args = solana_args(module)
    args.source_state_verifier_hash = args.finality_policy_hash

    try:
        module.render_toml(args)
    except SystemExit as exc:
        assert "source_state_verifier_hash matches finality_policy_hash" in str(exc)
    else:
        raise AssertionError("Solana TOML accepted reused source-adapter role hashes")


def test_solana_direct_record_hashes_reject_reused_role_hashes():
    module = load_evidence_module()

    material_args = solana_args(module)
    material_args.source_state_verifier_hash = material_args.finality_policy_hash
    try:
        module.solana_source_verifier_material_record_hash(material_args)
    except SystemExit as exc:
        assert "source_state_verifier_hash matches finality_policy_hash" in str(exc)
    else:
        raise AssertionError("Solana material hash accepted reused role hashes")

    deployment_args = solana_args(module)
    deployment_args.deployment_receipt_hash = (
        deployment_args.adapter_verifier_vk_hash
    )
    try:
        module.solana_source_adapter_engine_deployment_record_hash(deployment_args)
    except SystemExit as exc:
        assert "deployment_receipt_hash matches adapter_verifier_vk_hash" in str(exc)
    else:
        raise AssertionError("Solana deployment hash accepted reused role hashes")


def test_solana_source_record_hashes_match_rust_vectors():
    module = load_evidence_module()
    args = solana_args(module)

    assert (
        module.solana_source_verifier_material_record_hash(args).hex()
        == SOLANA_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
    )
    assert (
        module.solana_source_adapter_engine_deployment_record_hash(args).hex()
        == SOLANA_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
    )
    assert module.solana_full_light_client_gate_hash(args) is None

    args.tower_replay_verifier_hash = bytes.fromhex("bb" * 32)
    args.full_accountsdb_lattice_verifier_hash = bytes.fromhex("cc" * 32)
    args.bank_fork_choice_verifier_hash = bytes.fromhex("dd" * 32)
    assert (
        module.solana_source_adapter_engine_deployment_record_hash(args).hex()
        == SOLANA_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR
    )
    gate_hash = module.solana_full_light_client_gate_hash(args)
    assert gate_hash is not None
    assert gate_hash.hex() == SOLANA_FULL_LIGHT_CLIENT_GATE_HASH_VECTOR

    replayed_receipt = SimpleNamespace(**vars(args))
    replayed_receipt.deployment_receipt_hash = bytes.fromhex("ab" * 32)
    assert module.solana_full_light_client_gate_hash(replayed_receipt) != gate_hash


def test_direct_record_hashes_reject_template_component_hashes():
    module = load_evidence_module()
    for field, component_id, component_kind in module._template_hash_fields():
        template_hash = module.solana_template_component_hash(
            component_id,
            component_kind,
        )

        material_args = solana_args(module)
        setattr(material_args, field, template_hash)
        try:
            module.solana_source_verifier_material_record_hash(material_args)
        except SystemExit as exc:
            assert field in str(exc)
            assert "template hash" in str(exc)
        else:
            raise AssertionError(
                f"Solana material hash accepted template {field}"
            )

        deployment_args = solana_args(module)
        setattr(deployment_args, field, template_hash)
        try:
            module.solana_source_adapter_engine_deployment_record_hash(
                deployment_args
            )
        except SystemExit as exc:
            assert field in str(exc)
            assert "template hash" in str(exc)
        else:
            raise AssertionError(
                f"Solana deployment hash accepted template {field}"
            )


def test_direct_record_hashes_reject_zero_component_hashes():
    module = load_evidence_module()
    material_fields = tuple(
        field for field, _id, _kind in module._template_hash_fields()
    )
    deployment_fields = module._component_hash_args()
    audit_fields = tuple(
        field for field, _engine_id in module._light_client_evidence_fields()
    )

    for field in material_fields:
        args = solana_args(module)
        setattr(args, field, bytes(32))
        try:
            module.solana_source_verifier_material_record_hash(args)
        except ValueError as exc:
            assert f"{field} must not be zero" in str(exc)
        else:
            raise AssertionError(f"Solana material hash accepted zero {field}")

    for field in deployment_fields:
        args = solana_args(module)
        setattr(args, field, bytes(32))
        try:
            module.solana_source_adapter_engine_deployment_record_hash(args)
        except ValueError as exc:
            assert f"{field} must not be zero" in str(exc)
        else:
            raise AssertionError(f"Solana deployment hash accepted zero {field}")

    for field in audit_fields:
        args = solana_args(module)
        args.tower_replay_verifier_hash = bytes.fromhex("bb" * 32)
        args.full_accountsdb_lattice_verifier_hash = bytes.fromhex("cc" * 32)
        args.bank_fork_choice_verifier_hash = bytes.fromhex("dd" * 32)
        setattr(args, field, bytes(32))
        try:
            module.solana_full_light_client_gate_hash(args)
        except ValueError as exc:
            assert f"{field} must not be zero" in str(exc)
        else:
            raise AssertionError(f"Solana audit gate accepted zero {field}")


def test_solana_source_deployment_hash_rejects_noncanonical_adapter_vk_hash():
    module = load_evidence_module()
    args = solana_args(module)
    args.adapter_verifier_vk_hash = bytes.fromhex("99" * 32)

    try:
        module.solana_source_adapter_engine_deployment_record_hash(args)
    except ValueError as exc:
        assert "canonical Solana source-adapter verifier profile" in str(exc)
    else:
        raise AssertionError("noncanonical Solana adapter vk hash was accepted")


def test_solana_template_component_hashes_are_rejected():
    module = load_evidence_module()
    assert module.SOLANA_FINALIZED_VOTE_PREFIX == b"sccp:solana:finalized-vote:v1"
    for field, component_id, component_kind in module._template_hash_fields():
        template_hash = module.solana_template_component_hash(
            component_id,
            component_kind,
        )
        assert template_hash.hex() == SOLANA_TEMPLATE_HASHES[field]
        args = solana_args(module)
        setattr(args, field, template_hash)

        try:
            module.render_toml(args)
        except SystemExit as exc:
            assert field in str(exc)
            assert "template hash" in str(exc)
        else:
            raise AssertionError(f"Solana template {field} was accepted")


def test_solana_source_evidence_rejects_adapter_verifier_vk_hash_mismatch():
    module = load_evidence_module()
    args = solana_args(module)
    args.adapter_verifier_vk_hash = bytes.fromhex("99" * 32)

    try:
        module.render_toml(args)
    except SystemExit as exc:
        assert "canonical Solana source-adapter verifier profile" in str(exc)
    else:
        raise AssertionError("mismatched Solana adapter verifier vk hash was accepted")


def test_solana_cli_json_summary_and_toml_output(capsys):
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
        "0x" + SOLANA_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
        "--deployment-receipt-hash",
        "0x" + "aa" * 32,
        "--expected-source-verifier-material-hash",
        "0x" + SOLANA_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR,
        "--expected-source-adapter-engine-deployment-hash",
        "0x" + SOLANA_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR,
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
        raise AssertionError("unpinned Solana source TOML was accepted")

    assert module.main(args) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["source_domain"] == 3
    assert output["target_domain"] == 0
    assert output["source_state_verifier_hash"] == "0x" + "77" * 32
    assert (
        output["adapter_verifier_vk_hash"]
        == "0x" + SOLANA_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
    )
    assert (
        output["source_verifier_material_hash"]
        == "0x" + SOLANA_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
    )
    assert (
        output["source_adapter_engine_deployment_hash"]
        == "0x" + SOLANA_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
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
        raise AssertionError("Solana source TOML rendered without full light-client evidence")


def test_solana_cli_full_light_client_audit_hash(capsys):
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
        "0x" + SOLANA_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
        "--deployment-receipt-hash",
        "0x" + "aa" * 32,
        *solana_light_client_cli_args(),
        "--expected-source-verifier-material-hash",
        "0x" + SOLANA_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR,
        "--expected-source-adapter-engine-deployment-hash",
        "0x" + SOLANA_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR,
        "--expected-full-light-client-gate-hash",
        "0x" + SOLANA_FULL_LIGHT_CLIENT_GATE_HASH_VECTOR,
    ]

    assert module.main(args) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["source_adapter_engine_deployment_hash"] == (
        "0x" + SOLANA_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR
    )
    assert output["source_adapter_gate_closed_until_full_light_client"] is True
    assert output["source_adapter_gate_ready_with_full_light_client_evidence"] is True
    assert output["source_adapter_gate_blockers"] == []
    assert output["full_light_client_evidence_ready"] is True
    assert output["full_light_client_gate_hash"] == (
        "0x" + SOLANA_FULL_LIGHT_CLIENT_GATE_HASH_VECTOR
    )
    assert output["missing_full_light_client_verifier_ids"] == []
    assert output["expected_source_verifier_material_hash_matches"] is True
    assert output["expected_source_adapter_engine_deployment_hash_matches"] is True
    assert output["source_verifier_material_ready"] is True
    assert output["source_adapter_engine_deployment_ready"] is True
    assert output["full_toml_ready"] is True
    assert output["toml_ready"] is True
    assert output["full_light_client_verifier_hashes"] == {
        "tower_replay_verifier_hash": "0x" + "bb" * 32,
        "full_accountsdb_lattice_verifier_hash": "0x" + "cc" * 32,
        "bank_fork_choice_verifier_hash": "0x" + "dd" * 32,
    }

    assert module.main([*args, "--toml"]) == 0
    rendered = capsys.readouterr().out
    assert (
        '# sccp_solana_source_adapter_engine_deployment_hash = "0x'
        + SOLANA_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR
        + '"'
        in rendered
    )
    assert (
        '# solana_full_light_client_gate_hash = "0x'
        + SOLANA_FULL_LIGHT_CLIENT_GATE_HASH_VECTOR
        + '"'
        in rendered
    )
    assert 'solana_tower_replay_verifier_hash = "0x' + "bb" * 32 + '"' in rendered
    assert (
        'solana_full_accountsdb_lattice_verifier_hash = "0x'
        + "cc" * 32
        + '"'
        in rendered
    )
    assert 'solana_bank_fork_choice_verifier_hash = "0x' + "dd" * 32 + '"' in rendered
    assert (
        'solana_full_light_client_gate_hash = "0x'
        + SOLANA_FULL_LIGHT_CLIENT_GATE_HASH_VECTOR
        + '"'
        in rendered
    )
    assert "# full_light_client_evidence_ready = true" in rendered


def test_solana_cli_toml_requires_expected_full_light_client_gate_hash(capsys):
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
        "0x" + SOLANA_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
        "--deployment-receipt-hash",
        "0x" + "aa" * 32,
        *solana_light_client_cli_args(),
        "--expected-source-verifier-material-hash",
        "0x" + SOLANA_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR,
        "--expected-source-adapter-engine-deployment-hash",
        "0x" + SOLANA_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_WITH_AUDIT_HASH_VECTOR,
    ]

    assert module.main(args) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["source_adapter_gate_ready_with_full_light_client_evidence"] is False
    assert output["source_adapter_gate_blockers"] == [
        "full light-client gate hash is not pinned or mismatched",
    ]
    assert output["full_light_client_evidence_ready"] is True
    assert output["expected_full_light_client_gate_hash_matches"] is False
    assert output["toml_ready"] is False

    try:
        module.main([*args, "--toml"])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError(
            "Solana TOML rendering accepted audit evidence without an expected gate hash"
        )


def test_solana_cli_rejects_partial_full_light_client_audit_evidence():
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
        "0x" + SOLANA_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
        "--deployment-receipt-hash",
        "0x" + "aa" * 32,
        "--tower-replay-verifier-hash",
        "0x" + "bb" * 32,
    ]

    try:
        module.main(args)
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("partial Solana full light-client evidence was accepted")

    direct_args = solana_args(module)
    direct_args.tower_replay_verifier_hash = bytes.fromhex("bb" * 32)
    try:
        module.solana_source_adapter_engine_deployment_record_hash(direct_args)
    except SystemExit as exc:
        assert "Solana full light-client evidence must include all verifier hashes" in str(exc)
    else:
        raise AssertionError(
            "partial Solana full light-client deployment hash was accepted"
        )

    try:
        module._json_summary(direct_args)
    except SystemExit as exc:
        assert "Solana full light-client evidence must include all verifier hashes" in str(exc)
    else:
        raise AssertionError(
            "partial Solana full light-client evidence was accepted by JSON summary"
        )


def test_solana_cli_rejects_duplicate_full_light_client_audit_hashes():
    module = load_evidence_module()
    args = solana_args(module)
    args.tower_replay_verifier_hash = bytes.fromhex("bb" * 32)
    args.full_accountsdb_lattice_verifier_hash = bytes.fromhex("bb" * 32)
    args.bank_fork_choice_verifier_hash = bytes.fromhex("dd" * 32)

    try:
        module.solana_source_adapter_engine_deployment_record_hash(args)
    except SystemExit as exc:
        assert "role-separated" in str(exc)
    else:
        raise AssertionError("duplicate Solana full-light-client audit hashes were accepted")


def test_solana_cli_rejects_full_light_client_audit_hash_reusing_source_material():
    module = load_evidence_module()
    args = solana_args(module)
    args.tower_replay_verifier_hash = args.source_state_verifier_hash
    args.full_accountsdb_lattice_verifier_hash = bytes.fromhex("cc" * 32)
    args.bank_fork_choice_verifier_hash = bytes.fromhex("dd" * 32)

    try:
        module.solana_full_light_client_gate_hash(args)
    except SystemExit as exc:
        assert "source_state_verifier_hash" in str(exc)
    else:
        raise AssertionError(
            "Solana full-light-client audit hash reused source material"
        )


def test_solana_cli_rejects_full_light_client_audit_hash_reusing_deployment_material():
    module = load_evidence_module()

    for field in ("adapter_verifier_vk_hash", "deployment_receipt_hash"):
        args = solana_args(module)
        args.tower_replay_verifier_hash = getattr(args, field)
        args.full_accountsdb_lattice_verifier_hash = bytes.fromhex("cc" * 32)
        args.bank_fork_choice_verifier_hash = bytes.fromhex("dd" * 32)

        try:
            module.solana_full_light_client_gate_hash(args)
        except SystemExit as exc:
            assert field in str(exc)
        else:
            raise AssertionError(
                f"Solana full-light-client audit hash reused {field}"
            )


def test_solana_cli_rejects_full_light_client_audit_hash_reusing_template_material():
    module = load_evidence_module()
    args = solana_args(module)
    args.tower_replay_verifier_hash = module.solana_template_component_hash(
        module.SOLANA_SOURCE_TRUST_ANCHOR_ID,
        "source-trust-anchor",
    )
    args.full_accountsdb_lattice_verifier_hash = bytes.fromhex("cc" * 32)
    args.bank_fork_choice_verifier_hash = bytes.fromhex("dd" * 32)

    try:
        module.solana_full_light_client_gate_hash(args)
    except SystemExit as exc:
        assert "template material" in str(exc)
        assert "tower_replay_verifier_hash" in str(exc)
    else:
        raise AssertionError(
            "Solana full-light-client audit hash reused template material"
        )


def test_solana_cli_rejects_full_light_client_audit_hash_mismatch():
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
        "0x" + SOLANA_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
        "--deployment-receipt-hash",
        "0x" + "aa" * 32,
        *solana_light_client_cli_args(),
        "--expected-full-light-client-gate-hash",
        "0x" + "99" * 32,
    ]

    try:
        module.main(args)
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("mismatched Solana full light-client audit hash was accepted")


def test_solana_cli_rejects_expected_record_hash_mismatch():
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
        "0x" + SOLANA_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
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
        raise AssertionError("mismatched Solana expected material hash was accepted")


def test_solana_cli_rejects_non_production_lane():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--source-domain",
                "4",
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
                "0x" + SOLANA_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
                "--deployment-receipt-hash",
                "0x" + "aa" * 32,
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("non-Solana source evidence was accepted")
