"""Tests for scripts/render_taira_validator_bundle.py."""

from __future__ import annotations

import importlib.util
import json
import stat
import sys
import tomllib
from pathlib import Path

import pytest


MODULE_PATH = Path(__file__).resolve().parents[1] / "render_taira_validator_bundle.py"
SPEC = importlib.util.spec_from_file_location("render_taira_validator_bundle", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

TAIRA_CONFIG_PATH = MODULE_PATH.parents[1] / "configs/soranexus/taira/config.toml"
TAIRA_GENESIS_PATH = MODULE_PATH.parents[1] / "configs/soranexus/taira/genesis.json"
TAIRA_SECRETS_EXAMPLE_PATH = (
    MODULE_PATH.parents[1] / "configs/soranexus/taira/validator_secrets.example.toml"
)
TAIRA_CHAIN_ID = "fc56984b-2be7-431d-840e-21514d1883f0"
TAIRA_CHAIN_DISCRIMINANT = 369
TAIRA_CITIZEN_ID = (
    "testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
)
TAIRA_GENESIS_DEPLOYER_ID = (
    "testuﾛ1PｵEmｷjMZZﾑﾙeｱﾁﾎﾅﾂﾊmECepdbﾎｳ2uWﾃｸﾊﾘvｵi2ｦP1Y18A"
)
TAIRA_GAS_ASSET_ID = "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
TAIRA_CONSENSUS_FINGERPRINT = (
    "0x21591690e3c4d51fb3b81425aa8b9986eb417cc6a211dcfb8bce51c7600a6a7e"
)
TAIRA_FEE_SPONSOR_SELECTORS = [
    "iroha.log",
    "iroha.register",
    "iroha.grant",
    "iroha.alias.ensure",
    "nexus::EnrollFeeSponsorBeneficiary",
    "iroha_data_model::isi::space_directory::PublishSpaceDirectoryManifest",
    "iroha.account.alias.primary.compare_and_set",
    "iroha_data_model::isi::smart_contract_code::UploadSmartContractCodeChunk",
    "iroha_data_model::isi::smart_contract_code::FinalizeSmartContractCodeUpload",
    "iroha_data_model::isi::smart_contract_code::RegisterSmartContractCode",
    "iroha_data_model::isi::smart_contract_code::CommitContractDeployment",
    "iroha.contract.alias.set",
    "iroha_data_model::isi::sorafs::RegisterCapacityDeclaration",
    "iroha.set_key_value",
    "soracloud::HeartbeatSoracloudModelHost",
    "soracloud::AdvertiseSoracloudInrouHost",
    "soracloud::ReconcileSoracloudInrouPlacements",
    "iroha_data_model::isi::soracloud::ReconcileSoracloudModelHosts",
    "iroha_data_model::isi::soracloud::ReportSoracloudModelHostViolation",
    "soracloud::SetSoracloudInrouReplicaRuntimeState",
    "soracloud::ClearSoracloudInrouReplicaRuntimeState",
    "soracloud::ReportSoracloudServiceLeaseUsage",
]

COUNCIL_KEY_1 = "ed01202152F8D19B791D24453242E15F2EAB6CB7CFFA7B6A5ED30097960E069881DB12"
COUNCIL_KEY_2 = "ed012022FC297792F0B6FFC0BFCFDB7EDB0C0AA14E025A365EC0E342E86E3829CB74B6"
COUNCIL_KEY_3 = "ed01206355691C178A8FF91007A7478AFB955EF7352C63E7B25703984CF78B26E21A56"


def test_taira_templates_require_operator_provisioned_scale_2_ds_offline_binding() -> None:
    config_text = TAIRA_CONFIG_PATH.read_text(encoding="utf-8")
    secrets_text = TAIRA_SECRETS_EXAMPLE_PATH.read_text(encoding="utf-8")

    assert "wonderland.universal" not in secrets_text
    assert 'account_onboarding_scope_dataspace = "is"' in secrets_text
    assert 'offline_asset_alias = "ds#boi.is"' in secrets_text
    assert "offline_asset_scale = 2" in secrets_text
    assert "REPLACE_WITH_REGISTERED_SCALE_2_DS_ASSET_DEFINITION_ID" in secrets_text
    assert "REPLACE_WITH_REGISTERED_SCALE_2_DS_ASSET_DEFINITION_ID" in config_text
    offline_section = config_text.split("[settlement.offline]", 1)[1].split("\n[", 1)[0]
    assert TAIRA_GAS_ASSET_ID not in offline_section


def test_taira_runtime_paths_and_deploy_rate_are_release_pinned() -> None:
    config = tomllib.loads(TAIRA_CONFIG_PATH.read_text(encoding="utf-8"))
    genesis = json.loads(TAIRA_GENESIS_PATH.read_text(encoding="utf-8"))

    assert config["torii"]["address"] == "addr:0.0.0.0:18080#2F16"
    assert config["torii"]["deploy_rate_per_origin_per_sec"] == 4
    assert config["torii"]["deploy_burst_per_origin"] == 8
    assert config["sumeragi"]["block"]["max_payload_bytes"] == 21 * MODULE.MIB
    assert config["sumeragi"]["queues"]["body_source_bytes"] == 43 * MODULE.MIB
    assert config["sumeragi"]["queues"]["body_bytes"] == 7 * 43 * MODULE.MIB
    assert (
        config["network"]["max_frame_bytes_block_sync"]
        == MODULE.TAIRA_BLOCK_SYNC_PLAINTEXT_FRAME_BYTES
    )
    assert (
        config["network"]["max_frame_bytes_tx_gossip"]
        == MODULE.TAIRA_TX_GOSSIP_PLAINTEXT_FRAME_BYTES
    )
    assert config["network"]["max_frame_bytes"] == MODULE.TAIRA_MAX_FRAME_BYTES
    assert (
        genesis["sumeragi_v2"]["da_layout"]["max_payload_size_bytes"]
        == config["sumeragi"]["block"]["max_payload_bytes"]
    )
    assert (
        config["settlement"]["offline"]["kagemusha_release_policy_path"]
        == "/etc/iroha/taira-validator/kagemusha/release-policy.norito"
    )
    assert (
        config["settlement"]["offline"]["kagemusha_artifact_dir"]
        == "/var/lib/iroha/taira-validator/kagemusha/v4"
    )


def test_taira_governance_timing_contract_is_release_pinned() -> None:
    config = tomllib.loads(TAIRA_CONFIG_PATH.read_text(encoding="utf-8"))
    governance = config["gov"]

    assert governance["plain_voting_enabled"] is True
    assert governance["min_enactment_delay"] == 600
    assert governance["window_span"] == 3_600
    assert governance["min_turnout"] == 1
    assert governance["pipeline_enactment_sla_blocks"] == 3_600
    assert {
        key: governance[key]
        for key in (
            "parliament_committee_size",
            "parliament_term_blocks",
            "parliament_min_stake",
            "parliament_eligibility_asset_id",
            "parliament_alternate_size",
            "parliament_quorum_bps",
            "rules_committee_size",
            "agenda_council_size",
            "interest_panel_size",
            "review_panel_size",
            "policy_jury_size",
            "oversight_committee_size",
            "fma_committee_size",
        )
    } == {
        "parliament_committee_size": 21,
        "parliament_term_blocks": 43_200,
        "parliament_min_stake": "1",
        "parliament_eligibility_asset_id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
        "parliament_alternate_size": 21,
        "parliament_quorum_bps": 6_667,
        "rules_committee_size": 7,
        "agenda_council_size": 9,
        "interest_panel_size": 11,
        "review_panel_size": 13,
        "policy_jury_size": 25,
        "oversight_committee_size": 7,
        "fma_committee_size": 5,
    }


def _genesis_instructions(payload: dict) -> list[dict]:
    return [
        instruction
        for transaction in payload["transactions"]
        for instruction in transaction.get("instructions", [])
    ]


def _contract_deployment_gate_projection(payload: dict) -> dict:
    instructions = _genesis_instructions(payload)
    sponsor_revisions = [
        instruction["StageFeeSponsorProgramRevision"]["revision"]
        for instruction in instructions
        if "StageFeeSponsorProgramRevision" in instruction
    ]
    assert len(sponsor_revisions) == 1
    selectors = [
        selector
        for rule in sponsor_revisions[0]["rules"]
        for selector in rule["selectors"]
    ]
    return {
        "chain": payload["chain"],
        "chain_discriminant": payload["chain_discriminant"],
        "citizens": [
            instruction["RegisterCitizen"]
            for instruction in instructions
            if "RegisterCitizen" in instruction
        ],
        "sponsor_program_id": sponsor_revisions[0]["program_id"],
        "selectors": selectors,
        "code_registration_grants": [
            instruction["Grant"]["Permission"]
            for instruction in instructions
            if instruction.get("Grant", {})
            .get("Permission", {})
            .get("object", {})
            .get("name")
            == "CanRegisterSmartContractCode"
        ],
        "account_registration_grants": [
            instruction["Grant"]["Permission"]
            for instruction in instructions
            if instruction.get("Grant", {})
            .get("Permission", {})
            .get("object", {})
            .get("name")
            == "CanRegisterAccount"
        ],
        "deployer_gas_mints": [
            instruction["Mint"]["Asset"]
            for instruction in instructions
            if instruction.get("Mint", {}).get("Asset", {}).get("destination")
            == f"{TAIRA_GAS_ASSET_ID}#{TAIRA_GENESIS_DEPLOYER_ID}"
        ],
        "fee_sponsor_funding": [
            instruction["FundFeeSponsorProgram"]
            for instruction in instructions
            if "FundFeeSponsorProgram" in instruction
        ],
    }


def test_checked_in_taira_genesis_contract_deployment_gate_is_release_pinned() -> None:
    genesis = json.loads(TAIRA_GENESIS_PATH.read_text(encoding="utf-8"))
    projection = _contract_deployment_gate_projection(genesis)

    assert projection == {
        "chain": TAIRA_CHAIN_ID,
        "chain_discriminant": TAIRA_CHAIN_DISCRIMINANT,
        "citizens": [{"owner": TAIRA_CITIZEN_ID, "amount": "10000"}],
        "sponsor_program_id": {
            "sponsor": TAIRA_GENESIS_DEPLOYER_ID,
            "name": "cbsi_web",
        },
        "selectors": [
            {
                "kind": "native_instruction",
                "value": {"wire_id": wire_id},
            }
            for wire_id in TAIRA_FEE_SPONSOR_SELECTORS
        ],
        "code_registration_grants": [
            {
                "destination": TAIRA_GENESIS_DEPLOYER_ID,
                "object": {"name": "CanRegisterSmartContractCode"},
            }
        ],
        "account_registration_grants": [
            {
                "destination": TAIRA_GENESIS_DEPLOYER_ID,
                "object": {"name": "CanRegisterAccount"},
            }
        ],
        "deployer_gas_mints": [
            {
                "destination": (
                    f"{TAIRA_GAS_ASSET_ID}#{TAIRA_GENESIS_DEPLOYER_ID}"
                ),
                "object": "100000001",
            }
        ],
        "fee_sponsor_funding": [
            {
                "program_id": {
                    "sponsor": TAIRA_GENESIS_DEPLOYER_ID,
                    "name": "cbsi_web",
                },
                "asset_definition_id": TAIRA_GAS_ASSET_ID,
                "amount": "100000000",
            }
        ],
    }
    selector_wire_ids = [
        selector["value"]["wire_id"] for selector in projection["selectors"]
    ]
    assert (
        "iroha_data_model::isi::smart_contract_code::ActivateContractInstance"
        not in selector_wire_ids
    )
    assert "iroha.custom" not in selector_wire_ids


def test_checked_in_taira_genesis_gas_parameters_are_structured_parameters() -> None:
    genesis = json.loads(TAIRA_GENESIS_PATH.read_text(encoding="utf-8"))
    custom = genesis["transactions"][0]["parameters"]["custom"]

    assert genesis["consensus_fingerprint"] == TAIRA_CONSENSUS_FINGERPRINT
    assert {
        key: custom[key]
        for key in (
            "ivm_gas_limit_per_block",
            "ivm_gas_accepted_assets",
            "ivm_gas_units_per_gas",
        )
    } == {
        "ivm_gas_limit_per_block": {
            "id": "ivm_gas_limit_per_block",
            "payload": 50_000_000,
        },
        "ivm_gas_accepted_assets": {
            "id": "ivm_gas_accepted_assets",
            "payload": [TAIRA_GAS_ASSET_ID],
        },
        "ivm_gas_units_per_gas": {
            "id": "ivm_gas_units_per_gas",
            "payload": [
                {
                    "asset": TAIRA_GAS_ASSET_ID,
                    "units_per_gas": 0,
                    "twap_local_per_xor": "1",
                    "liquidity_profile": "tier1",
                    "volatility_class": "stable",
                }
            ],
        },
    }
    assert all(
        isinstance(instruction, dict) and len(instruction) == 1
        for instruction in _genesis_instructions(genesis)
    )
    assert {
        key: custom["sumeragi_npos_parameters"]["payload"][key]
        for key in ("min_self_bond", "min_nomination_bond")
    } == {
        "min_self_bond": "1000",
        "min_nomination_bond": "1",
    }


BASE_CONFIG = """# baseline
public_key = "peer-1-public"
private_key = "peer-1-private"

trusted_peers = [
  "peer-1-public@taira-validator-1.sora.org:1337",
]
trusted_peers_pop = [
  { public_key = "peer-1-public", pop_hex = "peer-1-pop" },
]

[network]
address = "0.0.0.0:1337"
public_address = "taira-validator-1.sora.org:1337"
max_frame_bytes = 23068700
max_frame_bytes_block_sync = 23068672
max_frame_bytes_tx_gossip = 11534336

[sumeragi.block]
max_payload_bytes = 22020096

[sumeragi.queues]
authenticated_non_validator_sources = 2
body_bytes = 315621376
body_source_bytes = 45088768

[torii]
address = "0.0.0.0:18080"
public_address = "https://taira-validator-1.sora.org"

[torii.mcp]
enabled = true

[torii.kagemusha_commands]
enabled = true
private_key = "REPLACE_WITH_TAIRA_KAGEMUSHA_COMMANDS_PRIVATE_KEY"

[settlement.offline]
escrow_required = true
escrow_accounts = { "REPLACE_WITH_REGISTERED_SCALE_2_DS_ASSET_DEFINITION_ID" = "REPLACE_WITH_TAIRA_OFFLINE_ESCROW_ACCOUNT" }
kagemusha_release_policy_path = "/etc/iroha/taira-validator/kagemusha/release-policy.norito"
kagemusha_artifact_dir = "/var/lib/iroha/taira-validator/kagemusha/v4"
kagemusha_max_decoded_bytes = 268435456

[nexus.registry]
manifest_directory = "configs/soranexus/taira/manifests"
cache_directory = "configs/soranexus/taira/manifests"
poll_interval_ms = 10000

[torii.account_onboarding]
authority = "REPLACE_WITH_TAIRA_ONBOARDING_AUTHORITY"
private_key_file = "REPLACE_WITH_TAIRA_ONBOARDING_PRIVATE_KEY_FILE"
lease_term_years = 1
additional_permissions = []

[[torii.account_onboarding.credentials]]
id = "REPLACE_WITH_TAIRA_ONBOARDING_CREDENTIAL_ID"
scope = { dataspace = "REPLACE_WITH_TAIRA_ONBOARDING_SCOPE" }
token_hash = "REPLACE_WITH_TAIRA_ONBOARDING_TOKEN_HASH"

[torii.faucet]
authority = "REPLACE_WITH_TAIRA_FAUCET_AUTHORITY"
private_key_file = "REPLACE_WITH_TAIRA_FAUCET_PRIVATE_KEY_FILE"

[streaming]
identity_public_key = "REPLACE_WITH_STREAMING_IDENTITY_PUBLIC_KEY"
identity_private_key = "REPLACE_WITH_STREAMING_IDENTITY_PRIVATE_KEY"

[sorafs.discovery.admission]
envelopes_dir = "configs/soranexus/taira/sorafs_admission"
trusted_council_keys = ["REPLACE_WITH_TAIRA_SORAFS_COUNCIL_PUBLIC_KEY"]
signature_threshold = "REPLACE_WITH_TAIRA_SORAFS_COUNCIL_SIGNATURE_THRESHOLD"
"""


def _write_roster(path: Path, validator_count: int = 4, inline_private_keys: bool = True) -> None:
    validators = []
    for index in range(1, validator_count + 1):
        entry = [
            "[[validators]]",
            f'slug = "taira-validator-{index}"',
            f'account_id = "test-validator-{index}"',
            f'public_key = "peer-{index}-public"',
        ]
        if inline_private_keys:
            entry.append(f'private_key = "peer-{index}-private"')
        entry.extend(
            [
                f'pop_hex = "peer-{index}-pop"',
                f'public_address = "taira-validator-{index}.sora.org:1337"',
                f'torii_public_address = "https://taira-validator-{index}.sora.org"',
                "",
            ]
        )
        validators.extend(entry)
    path.write_text(
        "\n".join(validators),
        encoding="utf-8",
    )


def _write_secrets(path: Path, validator_count: int = 4) -> None:
    validators = [
        "[shared]",
        'account_onboarding_authority = "bootstrap-authority"',
        'account_onboarding_private_key = "bootstrap-private-key"',
        'account_onboarding_api_token = "bootstrap-api-token"',
        'account_onboarding_credential_id = "local-dev"',
        'account_onboarding_scope_dataspace = "is"',
        'torii_faucet_authority = "faucet-authority"',
        'torii_faucet_private_key = "faucet-private-key"',
        'kagemusha_commands_private_key = "kagemusha-commands-private-key"',
        'offline_asset_alias = "ds#boi.is"',
        f'offline_asset_definition_id = "{TAIRA_GAS_ASSET_ID}"',
        "offline_asset_scale = 2",
        f'offline_escrow_account = "{TAIRA_GENESIS_DEPLOYER_ID}"',
        'streaming_identity_public_key = "streaming-public-key"',
        'streaming_identity_private_key = "streaming-private-key"',
        f'sorafs_council_public_keys = ["{COUNCIL_KEY_1}", "{COUNCIL_KEY_2}", "{COUNCIL_KEY_3}"]',
        "sorafs_council_signature_threshold = 2",
        "",
    ]
    for index in range(1, validator_count + 1):
        validators.extend(
            [
                "[[validators]]",
                f'slug = "taira-validator-{index}"',
                f'private_key = "peer-{index}-private"',
                "",
            ]
        )
    path.write_text("\n".join(validators), encoding="utf-8")


def test_render_bundle_rewrites_peer_specific_sections(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    base_config_path = tmp_path / "config.toml"
    output_dir = tmp_path / "out"
    _write_roster(roster_path)
    _write_secrets(secrets_path)
    base_config_path.write_text(BASE_CONFIG, encoding="utf-8")

    written = MODULE.render_bundle(
        base_config_path, roster_path, output_dir, secrets_path=secrets_path
    )

    assert len(written) == 4
    config = (output_dir / "taira-validator-3" / "config.toml").read_text(
        encoding="utf-8"
    )
    assert 'public_key = "peer-3-public"' in config
    assert 'private_key = "peer-3-private"' in config
    assert (
        'public_address = "addr:taira-validator-3.sora.org:1337#99FF"' in config
    )
    assert 'address = "addr:0.0.0.0:1337#BF18"' in config
    assert 'address = "addr:0.0.0.0:18080#2F16"' in config
    assert (
        '"peer-4-public@addr:taira-validator-4.sora.org:1337#E168"' in config
    )
    assert '{ public_key = "peer-2-public", pop_hex = "peer-2-pop" }' in config
    assert 'authority = "bootstrap-authority"' in config
    assert 'authority = "faucet-authority"' in config
    assert 'private_key = "kagemusha-commands-private-key"' in config
    assert (
        f'escrow_accounts = {{ "{TAIRA_GAS_ASSET_ID}" = '
        f'"{TAIRA_GENESIS_DEPLOYER_ID}" }}' in config
    )
    assert 'id = "local-dev"' in config
    assert 'scope = { dataspace = "is" }' in config
    assert MODULE._blake3_token_hash("bootstrap-api-token") in config
    assert "bootstrap-private-key" not in config
    assert "faucet-private-key" not in config
    assert "bootstrap-api-token" not in config
    runtime_dir = output_dir / "taira-validator-3" / "runtime"
    onboarding_key = runtime_dir / "onboarding-signer.key"
    faucet_key = runtime_dir / "faucet-signer.key"
    token_file = runtime_dir / "onboarding-token"
    assert (
        'private_key_file = "/etc/iroha/taira-validator/runtime/'
        'onboarding-signer.key"' in config
    )
    assert (
        'private_key_file = "/etc/iroha/taira-validator/runtime/'
        'faucet-signer.key"' in config
    )
    assert str(onboarding_key.resolve()) not in config
    assert str(faucet_key.resolve()) not in config
    assert onboarding_key.read_text(encoding="utf-8") == "bootstrap-private-key\n"
    assert faucet_key.read_text(encoding="utf-8") == "faucet-private-key\n"
    assert token_file.read_text(encoding="utf-8") == "bootstrap-api-token\n"
    assert stat.S_IMODE(output_dir.stat().st_mode) == 0o700
    assert stat.S_IMODE((output_dir / "taira-validator-3").stat().st_mode) == 0o700
    assert stat.S_IMODE(runtime_dir.stat().st_mode) == 0o700
    assert stat.S_IMODE(onboarding_key.stat().st_mode) == 0o600
    assert stat.S_IMODE(faucet_key.stat().st_mode) == 0o600
    assert stat.S_IMODE(token_file.stat().st_mode) == 0o600
    assert (output_dir / ".gitignore").read_text(encoding="utf-8") == "*\n!.gitignore\n"
    assert 'identity_public_key = "streaming-public-key"' in config
    assert 'identity_private_key = "streaming-private-key"' in config
    assert (
        f'trusted_council_keys = ["{COUNCIL_KEY_1}", "{COUNCIL_KEY_2}", "{COUNCIL_KEY_3}"]'
        in config
    )
    assert "signature_threshold = 2" in config
    assert (
        'manifest_directory = "/etc/iroha/taira-validator/manifests"' in config
    )
    assert 'cache_directory = "/etc/iroha/taira-validator/manifests"' in config
    assert (
        'kagemusha_release_policy_path = '
        '"/etc/iroha/taira-validator/kagemusha/release-policy.norito"' in config
    )
    assert (
        'envelopes_dir = "/etc/iroha/taira-validator/sorafs_admission"' in config
    )
    assert stat.S_IMODE(
        (output_dir / "taira-validator-3" / "kagemusha").stat().st_mode
    ) == 0o700
    assert stat.S_IMODE(
        (output_dir / "taira-validator-3" / "sorafs_admission").stat().st_mode
    ) == 0o700

    manifest_path = output_dir / "taira-validator-3" / "manifests" / "governance.manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    assert manifest["lane"] == "governance"
    assert manifest["governance"] == "parliament"
    assert manifest["quorum"] == 3
    assert manifest["validators"] == [
        {"validator": "test-validator-1", "peer_id": "peer-1-public"},
        {"validator": "test-validator-2", "peer_id": "peer-2-public"},
        {"validator": "test-validator-3", "peer_id": "peer-3-public"},
        {"validator": "test-validator-4", "peer_id": "peer-4-public"},
    ]


def test_render_bundle_uses_explicit_canonical_install_root(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    base_config_path = tmp_path / "config.toml"
    output_dir = tmp_path / "out"
    install_root = Path("/srv/iroha/taira-validator")
    _write_roster(roster_path)
    _write_secrets(secrets_path)
    base_config_path.write_text(BASE_CONFIG, encoding="utf-8")

    MODULE.render_bundle(
        base_config_path,
        roster_path,
        output_dir,
        secrets_path=secrets_path,
        install_root=install_root,
    )

    config = (output_dir / "taira-validator-1" / "config.toml").read_text(
        encoding="utf-8"
    )
    assert (
        'private_key_file = "/srv/iroha/taira-validator/runtime/'
        'onboarding-signer.key"' in config
    )
    assert (
        'private_key_file = "/srv/iroha/taira-validator/runtime/'
        'faucet-signer.key"' in config
    )
    assert (
        'manifest_directory = "/srv/iroha/taira-validator/manifests"' in config
    )
    assert 'cache_directory = "/srv/iroha/taira-validator/manifests"' in config
    assert (
        'kagemusha_release_policy_path = '
        '"/srv/iroha/taira-validator/kagemusha/release-policy.norito"' in config
    )
    assert (
        'envelopes_dir = "/srv/iroha/taira-validator/sorafs_admission"' in config
    )


def test_render_bundle_rejects_relative_install_root(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    base_config_path = tmp_path / "config.toml"
    _write_roster(roster_path)
    _write_secrets(secrets_path)
    base_config_path.write_text(BASE_CONFIG, encoding="utf-8")

    for invalid_root in (
        Path("../escape"),
        Path("/"),
        Path("/etc/iroha/../escape"),
        Path("//etc/iroha/taira-validator"),
        Path("/etc/iroha/taira-validator\ninjected"),
    ):
        with pytest.raises(
            ValueError,
            match="install_root must be a canonical, non-root absolute path",
        ):
            MODULE.render_bundle(
                base_config_path,
                roster_path,
                tmp_path / "out",
                secrets_path=secrets_path,
                install_root=invalid_root,
            )


def test_socket_addresses_are_crc_bound_and_fail_closed() -> None:
    assert (
        MODULE._canonical_socket_address(
            "TAIRA-VALIDATOR-1.SORA.ORG:1337", "fixture"
        )
        == "addr:taira-validator-1.sora.org:1337#D426"
    )
    assert (
        MODULE._canonical_socket_address(
            "addr:127.0.0.1:39080#4B72", "fixture"
        )
        == "addr:127.0.0.1:39080#4B72"
    )

    for invalid in (
        "addr:127.0.0.1:39080#0000",
        "127.0.0.1:70000",
        "https://taira.sora.org",
        "addr:127.0.0.1:39080",
    ):
        try:
            MODULE._canonical_socket_address(invalid, "fixture")
        except ValueError:
            pass
        else:  # pragma: no cover - defensive assertion
            raise AssertionError(f"renderer accepted invalid socket address {invalid!r}")


def test_render_bundle_injects_public_roster_into_unsigned_genesis(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    base_config_path = tmp_path / "config.toml"
    base_genesis_path = tmp_path / "genesis.json"
    output_dir = tmp_path / "out"
    _write_roster(roster_path, inline_private_keys=False)
    _write_secrets(secrets_path)
    base_config_path.write_text(BASE_CONFIG, encoding="utf-8")
    base_genesis_path.write_text(
        json.dumps(
            {
                "sumeragi_v2": {
                    "da_layout": {
                        "max_payload_size_bytes": MODULE.TAIRA_BLOCK_MAX_PAYLOAD_BYTES
                    },
                    "nexus_amx_context_hash": "01" * 32,
                },
                "transactions": [
                    {
                        "instructions": [],
                        "ivm_triggers": [],
                        "parameters": {
                            "transaction": {
                                "max_tx_bytes": MODULE.TAIRA_TRANSACTION_MAX_BYTES,
                                "max_decompressed_bytes": MODULE.TAIRA_TRANSACTION_MAX_BYTES,
                            }
                        },
                        "topology": [],
                    }
                ],
            }
        ),
        encoding="utf-8",
    )

    MODULE.render_bundle(
        base_config_path,
        roster_path,
        output_dir,
        secrets_path=secrets_path,
        base_genesis_path=base_genesis_path,
    )

    rendered = json.loads((output_dir / "genesis.json").read_text(encoding="utf-8"))
    topology = [
        entry
        for transaction in rendered["transactions"]
        for entry in transaction.get("topology", [])
    ]
    assert topology == [
        {"peer": f"peer-{index}-public", "pop_hex": f"peer-{index}-pop"}
        for index in range(1, 5)
    ]
    registered_validator_accounts = [
        instruction["Register"]["Account"]
        for transaction in rendered["transactions"]
        for instruction in transaction.get("instructions", [])
        if "Account" in instruction.get("Register", {})
    ]
    assert registered_validator_accounts == [
        {
            "id": f"test-validator-{index}",
            "metadata": {
                "purpose": "taira_validator_payout_recipient",
                "validator_slug": f"taira-validator-{index}",
            },
        }
        for index in range(1, 5)
    ]
    assert "peer-1-private" not in (output_dir / "genesis.json").read_text(
        encoding="utf-8"
    )
    signing_command = (output_dir / "genesis-signing-command.txt").read_text(
        encoding="utf-8"
    )
    assert "$TAIRA_GENESIS_PRIVATE_KEY" in signing_command
    assert "taira-validator-1/config.toml" in signing_command


def test_genesis_renderer_preserves_fresh_contract_deployment_gate(
    tmp_path: Path,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    output_dir = tmp_path / "out"
    _write_roster(roster_path)
    output_dir.mkdir()
    validators = MODULE.load_roster(roster_path)
    checked_in = json.loads(TAIRA_GENESIS_PATH.read_text(encoding="utf-8"))

    rendered_path = MODULE.render_genesis_template(
        TAIRA_GENESIS_PATH,
        validators,
        output_dir,
    )
    rendered = json.loads(rendered_path.read_text(encoding="utf-8"))

    assert _contract_deployment_gate_projection(
        rendered
    ) == _contract_deployment_gate_projection(checked_in)
    assert (
        rendered["transactions"][0]["parameters"]["custom"]
        == checked_in["transactions"][0]["parameters"]["custom"]
    )


def test_genesis_renderer_rejects_merged_instruction_objects(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    base_genesis_path = tmp_path / "genesis.json"
    output_dir = tmp_path / "out"
    _write_roster(roster_path)
    validators = MODULE.load_roster(roster_path)
    base_genesis_path.write_text(
        json.dumps(
            {
                "sumeragi_v2": {
                    "da_layout": {
                        "max_payload_size_bytes": MODULE.TAIRA_BLOCK_MAX_PAYLOAD_BYTES
                    },
                    "nexus_amx_context_hash": "01" * 32,
                },
                "transactions": [
                    {
                        "instructions": [
                            {
                                "Register": {"Domain": {"id": "test.universal"}},
                                "ivm_gas_limit_per_block": {
                                    "id": "ivm_gas_limit_per_block",
                                    "payload": 50_000_000,
                                },
                            }
                        ],
                        "ivm_triggers": [],
                        "parameters": {
                            "transaction": {
                                "max_tx_bytes": MODULE.TAIRA_TRANSACTION_MAX_BYTES,
                                "max_decompressed_bytes": MODULE.TAIRA_TRANSACTION_MAX_BYTES,
                            }
                        },
                        "topology": [],
                    }
                ],
            }
        ),
        encoding="utf-8",
    )

    try:
        MODULE.render_genesis_template(
            base_genesis_path,
            validators,
            output_dir,
        )
    except ValueError as error:
        assert "transaction 0 instruction 0" in str(error)
        assert "single-key structured instruction object" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("renderer accepted a merged genesis instruction object")


def test_load_roster_requires_explicit_direct_torii_hostname(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    roster_path.write_text(
        "\n".join(
            [
                "[[validators]]",
                'slug = "taira-validator-1"',
                'account_id = "test-validator-1"',
                'public_key = "peer-1-public"',
                'private_key = "peer-1-private"',
                'pop_hex = "peer-1-pop"',
                'public_address = "taira-validator-1.sora.org:1337"',
                "",
                "[[validators]]",
                'slug = "taira-validator-2"',
                'account_id = "test-validator-2"',
                'public_key = "peer-2-public"',
                'private_key = "peer-2-private"',
                'pop_hex = "peer-2-pop"',
                'public_address = "taira-validator-2.sora.org:1337"',
                "",
                "[[validators]]",
                'slug = "taira-validator-3"',
                'account_id = "test-validator-3"',
                'public_key = "peer-3-public"',
                'private_key = "peer-3-private"',
                'pop_hex = "peer-3-pop"',
                'public_address = "taira-validator-3.sora.org:1337"',
                "",
                "[[validators]]",
                'slug = "taira-validator-4"',
                'account_id = "test-validator-4"',
                'public_key = "peer-4-public"',
                'private_key = "peer-4-private"',
                'pop_hex = "peer-4-pop"',
                'public_address = "taira-validator-4.sora.org:1337"',
                "",
            ]
        ),
        encoding="utf-8",
    )

    try:
        MODULE.load_roster(roster_path)
    except ValueError as error:
        assert "must set `torii_public_address` explicitly" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("load_roster accepted a roster without public Torii URLs")


def test_load_roster_requires_four_validators(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    _write_roster(roster_path, validator_count=3)

    try:
        MODULE.load_roster(roster_path)
    except ValueError as error:
        assert "at least 4 validators" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("load_roster accepted a too-small validator roster")


def test_load_roster_rejects_more_than_protocol_maximum(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    _write_roster(roster_path, validator_count=MODULE.MAX_VALIDATORS + 1)

    try:
        MODULE.load_roster(roster_path)
    except ValueError as error:
        assert "at most 128 validators" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("load_roster accepted a roster above the protocol maximum")


def test_render_bundle_scales_body_budget_for_five_validators(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    base_config_path = tmp_path / "config.toml"
    output_dir = tmp_path / "out"
    _write_roster(roster_path, validator_count=5, inline_private_keys=False)
    _write_secrets(secrets_path, validator_count=5)
    base_config_path.write_text(BASE_CONFIG, encoding="utf-8")

    MODULE.render_bundle(
        base_config_path,
        roster_path,
        output_dir,
        secrets_path=secrets_path,
    )

    rendered = MODULE._load_toml(
        output_dir / "taira-validator-5" / "config.toml"
    )
    queues = rendered["sumeragi"]["queues"]
    assert queues["authenticated_non_validator_sources"] == 2
    assert queues["body_source_bytes"] == 45_088_768
    assert queues["body_bytes"] == 8 * queues["body_source_bytes"]


def test_render_bundle_rejects_non_positive_queue_template_values(
    tmp_path: Path,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    _write_roster(roster_path)

    malformed = {
        "authenticated_non_validator_sources": ["0", "-1", '"2"', "true"],
        "body_bytes": ["0", "-1", '"315621376"', "true"],
        "body_source_bytes": ["0", "-1", '"45088768"', "true"],
    }
    defaults = {
        "authenticated_non_validator_sources": 2,
        "body_bytes": 315621376,
        "body_source_bytes": 45088768,
    }
    for key, values in malformed.items():
        for index, value in enumerate(values):
            base_config_path = tmp_path / f"config-{key}-{index}.toml"
            output_dir = tmp_path / f"out-{key}-{index}"
            template = BASE_CONFIG.replace(
                f"{key} = {defaults[key]}",
                f"{key} = {value}",
            )
            base_config_path.write_text(template, encoding="utf-8")

            try:
                MODULE.render_bundle(base_config_path, roster_path, output_dir)
            except ValueError as error:
                assert f"field `{key}` must be a positive integer" in str(error)
            else:  # pragma: no cover - defensive assertion
                raise AssertionError(
                    f"render_bundle accepted malformed {key} value {value}"
                )


@pytest.mark.parametrize(
    ("field", "current", "replacement", "expected"),
    [
        (
            "max_payload_bytes",
            "22020096",
            str(MODULE.TAIRA_BLOCK_MAX_PAYLOAD_BYTES - 1),
            "must be at least 22020096 bytes",
        ),
        (
            "body_source_bytes",
            "45088768",
            str(MODULE.TAIRA_BODY_SOURCE_MIN_BYTES - 1),
            "must be at least 44270600 bytes",
        ),
        (
            "max_frame_bytes_block_sync",
            "23068672",
            str(MODULE.TAIRA_BLOCK_SYNC_PLAINTEXT_FRAME_BYTES - 1),
            "must be at least 23068672 bytes",
        ),
        (
            "max_frame_bytes_tx_gossip",
            "11534336",
            str(MODULE.TAIRA_TX_GOSSIP_PLAINTEXT_FRAME_BYTES - 1),
            "must be at least 11534336 bytes",
        ),
        (
            "max_frame_bytes",
            "23068700",
            str(MODULE.TAIRA_MAX_FRAME_BYTES - 1),
            "must include 28 AEAD bytes",
        ),
    ],
)
def test_render_bundle_rejects_privacy_transport_corridor_below_boundary(
    tmp_path: Path,
    field: str,
    current: str,
    replacement: str,
    expected: str,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    _write_roster(roster_path)
    base_config_path = tmp_path / f"config-{field}.toml"
    base_config_path.write_text(
        BASE_CONFIG.replace(
            f"{field} = {current}",
            f"{field} = {replacement}",
        ),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match=expected):
        MODULE.render_bundle(base_config_path, roster_path, tmp_path / "out")


def test_genesis_renderer_rejects_da_payload_one_byte_below_privacy_corridor(
    tmp_path: Path,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    _write_roster(roster_path)
    validators = MODULE.load_roster(roster_path)
    genesis = json.loads(TAIRA_GENESIS_PATH.read_text(encoding="utf-8"))
    genesis["sumeragi_v2"]["da_layout"]["max_payload_size_bytes"] = (
        MODULE.TAIRA_BLOCK_MAX_PAYLOAD_BYTES - 1
    )
    base_genesis_path = tmp_path / "genesis.json"
    base_genesis_path.write_text(json.dumps(genesis), encoding="utf-8")

    with pytest.raises(ValueError, match="must be at least 22020096"):
        MODULE.render_genesis_template(
            base_genesis_path,
            validators,
            tmp_path / "out",
        )


@pytest.mark.parametrize(
    ("field", "value", "expected"),
    [
        (
            "max_tx_bytes",
            MODULE.TAIRA_TRANSACTION_MAX_BYTES - 1,
            "must be at least 10485760 bytes",
        ),
        (
            "max_decompressed_bytes",
            MODULE.TAIRA_TRANSACTION_MAX_BYTES - 1,
            "must be at least `max_tx_bytes`",
        ),
    ],
)
def test_genesis_renderer_rejects_transaction_corridor_below_boundary(
    tmp_path: Path,
    field: str,
    value: int,
    expected: str,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    _write_roster(roster_path)
    validators = MODULE.load_roster(roster_path)
    genesis = json.loads(TAIRA_GENESIS_PATH.read_text(encoding="utf-8"))
    genesis["transactions"][0]["parameters"]["transaction"][field] = value
    base_genesis_path = tmp_path / "genesis.json"
    base_genesis_path.write_text(json.dumps(genesis), encoding="utf-8")

    with pytest.raises(ValueError, match=expected):
        MODULE.render_genesis_template(
            base_genesis_path,
            validators,
            tmp_path / "out",
        )


def test_load_roster_merges_private_keys_from_secrets(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    _write_roster(roster_path, inline_private_keys=False)
    _write_secrets(secrets_path)

    validators = MODULE.load_roster(roster_path, secrets_path=secrets_path)

    assert validators[0].private_key == "peer-1-private"
    assert validators[-1].private_key == "peer-4-private"


def test_render_bundle_rejects_unpopulated_template_placeholders(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    base_config_path = tmp_path / "config.toml"
    output_dir = tmp_path / "out"
    _write_roster(roster_path)
    base_config_path.write_text(BASE_CONFIG, encoding="utf-8")

    try:
        MODULE.render_bundle(base_config_path, roster_path, output_dir)
    except ValueError as error:
        assert "template placeholder values" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("render_bundle accepted placeholder secrets without a secrets file")


def test_secret_material_rejects_invalid_sorafs_council_quorum(tmp_path: Path) -> None:
    secrets_path = tmp_path / "validator_secrets.toml"
    secrets_path.write_text(
        "\n".join(
            [
                "[shared]",
                f'sorafs_council_public_keys = ["{COUNCIL_KEY_1}", "{COUNCIL_KEY_1}"]',
                "sorafs_council_signature_threshold = 2",
            ]
        ),
        encoding="utf-8",
    )
    try:
        MODULE.load_secret_material(secrets_path)
    except ValueError as error:
        assert "must not contain duplicates" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("duplicate SoraFS council roots were accepted")

    secrets_path.write_text(
        "\n".join(
            [
                "[shared]",
                f'sorafs_council_public_keys = ["{COUNCIL_KEY_1}"]',
                "sorafs_council_signature_threshold = 2",
            ]
        ),
        encoding="utf-8",
    )
    try:
        MODULE.load_secret_material(secrets_path)
    except ValueError as error:
        assert "threshold exceeds" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("unsatisfiable SoraFS council quorum was accepted")

    secrets_path.write_text(
        "\n".join(
            [
                "[shared]",
                'sorafs_council_public_keys = ["not-an-ed25519-key"]',
                "sorafs_council_signature_threshold = 1",
            ]
        ),
        encoding="utf-8",
    )
    try:
        MODULE.load_secret_material(secrets_path)
    except ValueError as error:
        assert "canonical non-zero Ed25519" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("malformed SoraFS council root was accepted")


def test_secret_material_rejects_removed_or_incomplete_signer_shapes(
    tmp_path: Path,
) -> None:
    secrets_path = tmp_path / "validator_secrets.toml"
    secrets_path.write_text(
        "[shared]\n"
        'torii_onboarding_authority = "legacy"\n'
        'torii_onboarding_private_key = "legacy-secret"\n',
        encoding="utf-8",
    )
    try:
        MODULE.load_secret_material(secrets_path)
    except ValueError as error:
        assert "removed onboarding fields" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("removed onboarding secret schema was accepted")

    secrets_path.write_text(
        "[shared]\n"
        'account_onboarding_authority = "authority"\n'
        'account_onboarding_private_key = "private"\n',
        encoding="utf-8",
    )
    try:
        MODULE.load_secret_material(secrets_path)
    except ValueError as error:
        assert "account onboarding is incomplete" in str(error)
        assert "account_onboarding_api_token" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("incomplete structural onboarding secrets were accepted")

    secrets_path.write_text(
        "[shared]\n"
        'torii_faucet_authority = "authority"\n',
        encoding="utf-8",
    )
    try:
        MODULE.load_secret_material(secrets_path)
    except ValueError as error:
        assert "must configure both torii_faucet_authority" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("incomplete faucet signer material was accepted")


def test_secret_material_requires_exact_scale_2_ds_offline_binding(
    tmp_path: Path,
) -> None:
    secrets_path = tmp_path / "validator_secrets.toml"
    _write_secrets(secrets_path)
    valid = secrets_path.read_text(encoding="utf-8")

    mutations = (
        (
            valid.replace('offline_asset_alias = "ds#boi.is"\n', ""),
            "offline cash configuration is mandatory",
        ),
        (
            valid.replace(
                f'offline_asset_definition_id = "{TAIRA_GAS_ASSET_ID}"',
                'offline_asset_definition_id = "REPLACE_WITH_DS_ID"',
            ),
            "still contain placeholders",
        ),
        (
            valid.replace('offline_asset_alias = "ds#boi.is"', 'offline_asset_alias = "xor#sora"'),
            "registered ds#boi.is alias",
        ),
        (
            valid.replace("offline_asset_scale = 2", "offline_asset_scale = 9"),
            "offline_asset_scale must be 2",
        ),
        (
            valid.replace(
                f'offline_asset_definition_id = "{TAIRA_GAS_ASSET_ID}"',
                'offline_asset_definition_id = "not-canonical"',
            ),
            "canonical asset definition id",
        ),
        (
            valid.replace(
                f'offline_escrow_account = "{TAIRA_GENESIS_DEPLOYER_ID}"',
                'offline_escrow_account = "offline-escrow@boi.is"',
            ),
            "canonical Taira I105 account id",
        ),
    )
    for index, (text, expected) in enumerate(mutations):
        candidate = tmp_path / f"validator_secrets_{index}.toml"
        candidate.write_text(text, encoding="utf-8")
        try:
            MODULE.load_secret_material(candidate)
        except ValueError as error:
            assert expected in str(error)
        else:  # pragma: no cover - defensive assertion
            raise AssertionError(f"renderer accepted offline mutation #{index}")


def test_secret_material_rejects_wonderland_onboarding_scope(tmp_path: Path) -> None:
    secrets_path = tmp_path / "validator_secrets.toml"
    _write_secrets(secrets_path)
    text = secrets_path.read_text(encoding="utf-8").replace(
        'account_onboarding_scope_dataspace = "is"',
        'account_onboarding_scope_domain = "wonderland.universal"',
    )
    secrets_path.write_text(text, encoding="utf-8")

    try:
        MODULE.load_secret_material(secrets_path)
    except ValueError as error:
        assert "deployed `is` dataspace" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("renderer accepted the stale Wonderland onboarding scope")


def test_main_supports_single_validator_render(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    base_config_path = tmp_path / "config.toml"
    output_dir = tmp_path / "out"
    _write_roster(roster_path, inline_private_keys=False)
    _write_secrets(secrets_path)
    base_config_path.write_text(BASE_CONFIG, encoding="utf-8")

    exit_code = MODULE.main(
        [
            "--base-config",
            str(base_config_path),
            "--roster",
            str(roster_path),
            "--secrets",
            str(secrets_path),
            "--output-dir",
            str(output_dir),
            "--only",
            "taira-validator-2",
        ]
    )

    assert exit_code == 0
    assert (output_dir / "taira-validator-2" / "config.toml").exists()
    assert not (output_dir / "taira-validator-1" / "config.toml").exists()
