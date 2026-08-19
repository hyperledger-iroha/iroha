"""Tests for scripts/render_taira_validator_bundle.py."""

from __future__ import annotations

import importlib.util
import hashlib
import json
import os
import stat
import sys
from pathlib import Path

import pytest
try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - exercised on Python 3.9/3.10
    import tomli as tomllib

MODULE_PATH = Path(__file__).resolve().parents[1] / "render_taira_validator_bundle.py"
SPEC = importlib.util.spec_from_file_location(
    "render_taira_validator_bundle", MODULE_PATH
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

TAIRA_CONFIG_PATH = MODULE_PATH.parents[1] / "configs/soranexus/taira/config.toml"
TAIRA_README_PATH = MODULE_PATH.parents[1] / "configs/soranexus/taira/README.md"
DEFAULT_TAIRA_CONFIG_PATH = (
    MODULE_PATH.parents[1] / "defaults/kagami/iroha3-taira/config.toml"
)
TAIRA_GENESIS_PATH = MODULE_PATH.parents[1] / "configs/soranexus/taira/genesis.json"
TAIRA_SECRETS_EXAMPLE_PATH = (
    MODULE_PATH.parents[1] / "configs/soranexus/taira/validator_secrets.example.toml"
)
TAIRA_CHAIN_ID = "fc56984b-2be7-431d-840e-21514d1883f0"
TAIRA_CHAIN_DISCRIMINANT = 369
TAIRA_CITIZEN_ID = "testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
TAIRA_GENESIS_DEPLOYER_ID = "testuﾛ1PｵEmｷjMZZﾑﾙeｱﾁﾎﾅﾂﾊmECepdbﾎｳ2uWﾃｸﾊﾘvｵi2ｦP1Y18A"
TAIRA_GAS_ASSET_ID = "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
TAIRA_CONSENSUS_FINGERPRINT = (
    "0x6679f78af39ad62147e20c504d21eadb579a9840fda4979e67b6b623d218cf37"
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
SORACLOUD_SIGNER_HANDLE = "hsm://soracloud/runtime-mutation/primary"
SORACLOUD_SIGNER_AUTHORITY = "testuﾛ1Nﾉﾏ2ｽﾍﾈdmcLfDBwoｽﾘｼoF5sHeｶdｶQbﾈxGVヰ8tCﾎ4P4KX7"
SORACLOUD_SIGNER_PUBLIC_KEY_HEX = (
    "4164bf554923ece1fd412d241036d863a6ae430476c898248b8237d77534cfc4"
)
SORACLOUD_SIGNER_POLICY_DIGEST_HEX = "95" * 32
ONBOARDING_TOKEN = "bootstrap-api-token-0123456789abcdef"
DPN_ONBOARDING_TOKEN = "dpn-api-token-0123456789abcdef012345"


def _receipt_keypair(index: int) -> tuple[str, str]:
    private_payload = index.to_bytes(32, "big")
    public_payload = MODULE._secp256k1_public_payload(private_payload)
    return (
        MODULE.RECEIPT_PUBLIC_KEY_PREFIX + public_payload.hex().upper(),
        MODULE.RECEIPT_PRIVATE_KEY_PREFIX + private_payload.hex().upper(),
    )


def test_receipt_keypair_and_node_id_use_canonical_secp256k1_domain() -> None:
    public_key, private_key = _receipt_keypair(1)

    assert public_key == (
        "e701210279BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798"
    )
    assert private_key == "812620" + "0" * 63 + "1"
    expected_node_id = MODULE.RECEIPT_NODE_ID_PREFIX + hashlib.sha256(
        MODULE.RECEIPT_NODE_ID_DOMAIN + public_key.encode("ascii")
    ).hexdigest()
    assert MODULE.validate_receipt_keypair(public_key, private_key, "fixture") == (
        expected_node_id
    )
    assert MODULE.receipt_node_id(public_key) == expected_node_id


def test_native_onboarding_token_hash_tool_receives_secret_only_on_stdin(
    tmp_path: Path,
) -> None:
    record = tmp_path / "record.json"
    tool = tmp_path / "onboarding-token-hash"
    tool.write_text(
        f"#!{sys.executable}\n"
        "import json, os, sys\n"
        "token = sys.stdin.buffer.read().decode('ascii')\n"
        f"open({str(record)!r}, 'w').write(json.dumps({{"
        "'token': token, 'argv': sys.argv, 'env': dict(os.environ)}))\n"
        f"print({'ab' * 32!r})\n",
        encoding="utf-8",
    )
    tool.chmod(0o700)

    digest = MODULE._blake3_token_hash(ONBOARDING_TOKEN, tool)

    invocation = json.loads(record.read_text(encoding="utf-8"))
    assert digest == "blake3:" + "ab" * 32
    assert invocation["token"] == ONBOARDING_TOKEN
    assert ONBOARDING_TOKEN not in json.dumps(invocation["argv"])
    assert ONBOARDING_TOKEN not in json.dumps(invocation["env"])


@pytest.mark.parametrize(
    "token",
    ["short", "x" * 257, "x" * 31 + " ", "é" * 32],
)
def test_onboarding_token_hash_rejects_noncanonical_secrets(token: str) -> None:
    with pytest.raises(ValueError, match="onboarding token must contain"):
        MODULE._blake3_token_hash(token)


def test_taira_templates_require_no_backend_offline_enrollment() -> None:
    config_text = TAIRA_CONFIG_PATH.read_text(encoding="utf-8")
    secrets_text = TAIRA_SECRETS_EXAMPLE_PATH.read_text(encoding="utf-8")

    assert "scope_domain" not in secrets_text
    assert secrets_text.count("[[shared.account_onboarding_credentials]]") == 2
    assert 'id = "boi-mobile"' in secrets_text
    assert 'scope_dataspace = "is2"' in secrets_text
    assert 'id = "dpn-api"' in secrets_text
    assert 'scope_dataspace = "dpn"' in secrets_text
    assert config_text.count("[[torii.account_onboarding.credentials]]") == 2
    assert 'scope = { dataspace = "is2" }' in config_text
    assert 'scope = { dataspace = "dpn" }' in config_text
    assert "offline_asset_alias" not in secrets_text
    assert "offline_asset_definition_id" not in secrets_text
    assert "offline_asset_scale" not in secrets_text
    assert "offline_escrow_account" not in secrets_text
    assert "escrow_required" not in config_text
    assert "escrow_accounts" not in config_text
    assert "REPLACE_WITH_SORACLOUD_RUNTIME_SIGNER_HANDLE" in secrets_text
    assert "REPLACE_WITH_SORACLOUD_RUNTIME_SIGNER_HANDLE" in config_text
    assert "REPLACE_WITH_SECP256K1_RECEIPT_PUBLIC_KEY_1" in secrets_text
    assert "REPLACE_WITH_SECP256K1_RECEIPT_PRIVATE_KEY_1" in secrets_text
    assert "operation_registry_max_entries = 4096" in config_text
    assert "operation_registry_max_bytes = 524288" in config_text
    assert "\n[settlement.offline]\n" not in config_text
    assert "kagemusha_release_policy_path" not in config_text
    assert "kagemusha_artifact_dir" not in config_text
    assert "--kagemusha-release-root" in config_text


def test_taira_kagemusha_release_docs_preserve_both_production_boundaries() -> None:
    readme = TAIRA_README_PATH.read_text(encoding="utf-8")

    assert "cargo run" not in readme
    assert "--kagemusha-release-catalog" not in readme
    assert "--kagemusha-release-root /srv/iroha-kagemusha/taira-v4-r1" in readme
    assert "/srv/iroha-kagemusha/taira-v4-r1/seals/" in readme
    assert "/etc/iroha/taira-validator/kagemusha-qualified" not in readme
    assert (
        "KAGEMUSHA_V4_KAGAMI_BIN=/absolute/root-custodied/kagami" in readme
    )
    assert (
        "KAGEMUSHA_V4_KAGAMI_SHA256='<reviewed-kagami-64-lowercase-hex>'"
        in readme
    )
    assert (
        "export KAGEMUSHA_V4_KAGAMI_BIN KAGEMUSHA_V4_KAGAMI_SHA256"
        in readme
    )
    assert (
        "readonly KAGEMUSHA_V4_KAGAMI_BIN KAGEMUSHA_V4_KAGAMI_SHA256"
        in readme
    )
    pinned_invocation_prefix = (
        "assert_kagemusha_v4_kagami_custody || exit 1\n"
        "if /usr/bin/env -i LANG=C LC_ALL=C PATH=/usr/bin:/bin \\\n"
        '  "${KAGEMUSHA_V4_KAGAMI_BIN}" \\\n'
        "  kagemusha "
    )
    for subcommand in (
        "prepare-taira-release-roster-v4",
        "prepare-release-circuit-params-v4",
        "prepare-activation-v4",
    ):
        assert pinned_invocation_prefix + subcommand in readme
    assert readme.count(pinned_invocation_prefix) == 3
    assert readme.count("assert_kagemusha_v4_kagami_custody || exit 1") == 6
    pinned_invocation_postcheck = (
        "then\n"
        "  KAGEMUSHA_COMMAND_STATUS=0\n"
        "else\n"
        "  KAGEMUSHA_COMMAND_STATUS=$?\n"
        "fi\n"
        "assert_kagemusha_v4_kagami_custody || exit 1\n"
        'test "${KAGEMUSHA_COMMAND_STATUS}" -eq 0 || '
        'exit "${KAGEMUSHA_COMMAND_STATUS}"'
    )
    assert readme.count(pinned_invocation_postcheck) == 3
    for pinned_invocation_suffix in (
        "--output /absolute/private/path/taira-release-roster.norito\n"
        + pinned_invocation_postcheck,
        "--output-dir /absolute/private/path/kagemusha-release-inputs/"
        "circuit-params-v4\n"
        + pinned_invocation_postcheck,
        '| /usr/bin/tee "${PREPARE_REPORT}"\n' + pinned_invocation_postcheck,
    ):
        assert pinned_invocation_suffix in readme
    assert 'path.resolve(strict=True)' in readme
    assert "metadata.st_uid != 0" in readme
    assert "stat.S_IMODE(metadata.st_mode) & 0o022" in readme
    assert "before.st_nlink != 1" in readme
    assert "digest.hexdigest() != expected_sha256" in readme
    assert "inherits the exported read-only `KAGEMUSHA_V4_KAGAMI_BIN`" in readme
    assert "kagemusha prepare-activation-v4" in readme
    report_validation = readme.index("if ! /usr/bin/jq -e")
    report_extraction = readme.index(
        'INSTRUCTIONS_HASH="$(/usr/bin/jq -er \'.instructions_hash\''
    )
    assert report_validation < report_extraction
    report_contract = readme[report_validation:report_extraction]
    assert '.status == "prepared"' in report_contract
    assert '.manifest_sha256 == $manifest_sha256' in report_contract
    assert '.verifier_version == $verifier_version' in report_contract
    assert ".instruction_count == 1" in report_contract
    assert (
        ".device_attestation_policy_state_sha256 == $device_policy_state_sha256"
        in report_contract
    )
    assert "REVIEWED_DEVICE_ATTESTATION_POLICY_STATE_SHA256" in report_contract
    assert "PREPARED_REPORT_LINE" in report_contract
    assert 'test("^[0-9a-f]{64}$")' in report_contract
    instructions_hash_contract = report_contract[
        report_contract.index("(.instructions_hash |") :
    ]
    assert 'test("^[0-9a-f]{64}$")' in instructions_hash_contract
    assert '(test("^0{64}$") | not)' in instructions_hash_contract
    assert "execution_policy_hash" in readme
    assert "/usr/local/libexec/iroha-taira-release-controller-v1" in readme
    assert 'prepare-reset -- \\\n' in readme
    assert "--genesis-native-verifier" in readme
    assert "--trusted-genesis-native-verifier-sha256" in readme
    for activation_field in (
        "genesis_native_verifier_sha256",
        "genesis_external_signer_sha256",
        "genesis_artifact_linkage_sha256",
        "nevo_review_sha256",
        "reviewed_unsigned_genesis_sha256",
        "pre_sign_rendered_genesis_sha256",
        "native_verifier_peer_config_set_sha256",
        "bound_genesis_manifest_sha256",
        "signed_genesis_sha256",
    ):
        assert f'"{activation_field}"' in readme
    assert 'GENESIS="${SIGNED_GENESIS}"' in readme
    assert 'IROHAD_BIN="${IROHAD_BIN:?' in readme
    assert (
        'VALIDATOR_CONFIG="${RESET_BUNDLE}/rendered/${VALIDATOR_SLUG}/config.toml"'
        in readme
    )
    assert 'BOUND_GENESIS_MANIFEST="${RESET_BUNDLE}/genesis.json"' in readme
    assert ".irohad_sha256" in readme
    assert "--genesis-manifest-json" in readme
    assert "ACTIVATION_HEIGHT=2" not in readme
    assert "ACTIVATION_SUBMISSION_MARGIN_BLOCKS" in readme
    assert "MAX_COMMITTED_HEIGHT" in readme
    assert "CLI_INSTRUCTIONS_HASH" in readme
    assert "--fee-payer authority" in readme
    assert "ledger multisig propose" in readme
    assert "ledger multisig approve" in readme
    assert "tx status" in readme
    assert "ops sumeragi status" in readme
    assert "app zk vk get" in readme
    assert "/v1/offline/readiness" in readme
    assert "POST /v1/offline/top-up" in readme
    assert "POST /v1/offline/redeem" in readme
    assert "terminal `applied`" in readme


def test_taira_runtime_paths_and_deploy_rate_are_release_pinned() -> None:
    config = tomllib.loads(TAIRA_CONFIG_PATH.read_text(encoding="utf-8"))
    default_config = tomllib.loads(
        DEFAULT_TAIRA_CONFIG_PATH.read_text(encoding="utf-8")
    )
    genesis = json.loads(TAIRA_GENESIS_PATH.read_text(encoding="utf-8"))

    assert config["torii"]["address"] == "addr:0.0.0.0:18080#2F16"
    assert config["torii"]["deploy_rate_per_origin_per_sec"] == 4
    assert config["torii"]["deploy_burst_per_origin"] == 8
    assert config["sumeragi"]["block"]["max_payload_bytes"] == 16 * MODULE.MIB
    assert config["sumeragi"]["queues"]["body_source_bytes"] == 33 * MODULE.MIB
    assert config["sumeragi"]["queues"]["body_bytes"] == 7 * 33 * MODULE.MIB
    assert (
        config["network"]["max_frame_bytes_block_sync"]
        == MODULE.TAIRA_BLOCK_SYNC_PLAINTEXT_FRAME_BYTES
    )
    assert (
        config["network"]["max_frame_bytes_tx_gossip"]
        == MODULE.TAIRA_TX_GOSSIP_PLAINTEXT_FRAME_BYTES
    )
    assert config["network"]["max_frame_bytes"] == MODULE.TAIRA_MAX_FRAME_BYTES
    assert default_config["network"]["max_frame_bytes"] == MODULE.TAIRA_MAX_FRAME_BYTES
    assert (
        default_config["network"]["max_frame_bytes_block_sync"]
        == MODULE.TAIRA_BLOCK_SYNC_PLAINTEXT_FRAME_BYTES
    )
    assert (
        default_config["network"]["max_frame_bytes_tx_gossip"]
        == MODULE.TAIRA_TX_GOSSIP_PLAINTEXT_FRAME_BYTES
    )
    assert (
        genesis["sumeragi_v2"]["da_layout"]["max_payload_size_bytes"]
        == config["sumeragi"]["block"]["max_payload_bytes"]
    )
    assert "offline" not in config.get("settlement", {})


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


def test_taira_disables_ungoverned_embedded_sorafs_storage() -> None:
    config = tomllib.loads(TAIRA_CONFIG_PATH.read_text(encoding="utf-8"))

    assert config["sorafs"]["storage"]["enabled"] is False
    assert "compliance" not in config["sorafs"].get("gateway", {})


def test_taira_bounds_each_shared_host_validator_storage_budget() -> None:
    config = tomllib.loads(TAIRA_CONFIG_PATH.read_text(encoding="utf-8"))
    storage = config["nexus"]["storage"]

    assert storage["local_budget_bytes"] == 64 * 1024 * 1024 * 1024
    assert storage["disk_budget_weights"] == {
        "kura_blocks_bps": 7_499,
        "wsv_snapshots_bps": 2_000,
        "sorafs_bps": 1,
        "soranet_spool_bps": 250,
        "soravpn_spool_bps": 250,
    }
    assert sum(storage["disk_budget_weights"].values()) == 10_000


def _genesis_instructions(payload: dict) -> list[dict]:
    return [
        instruction
        for transaction in payload["transactions"]
        for instruction in transaction.get("instructions", [])
    ]


def _asset_alias_projection(payload: dict) -> dict[str, str]:
    return {
        instruction["SetAssetDefinitionAlias"]["asset_definition_id"]: instruction[
            "SetAssetDefinitionAlias"
        ]["alias"]
        for instruction in _genesis_instructions(payload)
        if "SetAssetDefinitionAlias" in instruction
    }


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
                "object": {
                    "name": "CanRegisterSmartContractCode",
                    "payload": None,
                },
            }
        ],
        "account_registration_grants": [
            {
                "destination": TAIRA_GENESIS_DEPLOYER_ID,
                "object": {
                    "name": "CanRegisterAccount",
                    "payload": {"domain": "taira.universal"},
                },
            }
        ],
        "deployer_gas_mints": [
            {
                "destination": (f"{TAIRA_GAS_ASSET_ID}#{TAIRA_GENESIS_DEPLOYER_ID}"),
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


def test_checked_in_taira_genesis_permission_payloads_match_typed_shapes() -> None:
    genesis = json.loads(TAIRA_GENESIS_PATH.read_text(encoding="utf-8"))
    permission_grants = [
        instruction["Grant"]["Permission"]
        for instruction in _genesis_instructions(genesis)
        if "Permission" in instruction.get("Grant", {})
    ]

    assert permission_grants == [
        {
            "destination": TAIRA_GENESIS_DEPLOYER_ID,
            "object": {
                "name": "CanEnrollFeeSponsorProgram",
                "payload": {
                    "program_id": {
                        "sponsor": TAIRA_GENESIS_DEPLOYER_ID,
                        "name": "cbsi_web",
                    }
                },
            },
        },
        {
            "destination": TAIRA_GENESIS_DEPLOYER_ID,
            "object": {
                "name": "CanRegisterSmartContractCode",
                "payload": None,
            },
        },
        {
            "destination": TAIRA_GENESIS_DEPLOYER_ID,
            "object": {
                "name": "CanRegisterAccount",
                "payload": {"domain": "taira.universal"},
            },
        },
        {
            "destination": TAIRA_GENESIS_DEPLOYER_ID,
            "object": {"name": "CanEnactGovernance", "payload": None},
        },
        {
            "destination": TAIRA_CITIZEN_ID,
            "object": {"name": "CanSetParameters", "payload": None},
        },
        {
            "destination": TAIRA_CITIZEN_ID,
            "object": {"name": "CanReadAllLedgerData", "payload": None},
        },
    ]
    object_payload_names = {
        grant["object"]["name"]
        for grant in permission_grants
        if isinstance(grant["object"]["payload"], dict)
    }
    assert object_payload_names == {
        "CanEnrollFeeSponsorProgram",
        "CanRegisterAccount",
    }


def test_checked_in_taira_genesis_aliases_name_only_active_physical_dataspaces() -> None:
    genesis = json.loads(TAIRA_GENESIS_PATH.read_text(encoding="utf-8"))

    assert _asset_alias_projection(genesis) == {
        "6TEAJqbb8oEPmLncoNiMRbLEK6tw": "xor#universal",
        "7ZepsJTHCVLKsrFFNZGSRGZgvBhv": "sbd#cbsi",
        "61CtjvNd9T3THAR65GsMVHr82Bjc": "xor#sora.universal",
    }


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
private_key_file = "/run/secrets/iroha/taira-validator-private-key"
soranet_transport_public_key = "peer-1-soranet-public"
soranet_transport_private_key_file = "/run/secrets/iroha/taira-soranet-transport-private-key"

trusted_peers = [
  "peer-1-public@taira-validator-1.sora.org:1337",
]
trusted_peers_pop = [
  { public_key = "peer-1-public", pop_hex = "peer-1-pop" },
]

[genesis]
public_key = "genesis-public"
expected_hash_file = "/run/iroha/genesis.expected_hash"

[network]
address = "0.0.0.0:1337"
public_address = "taira-validator-1.sora.org:1337"
max_frame_bytes = 23068700
max_frame_bytes_block_sync = 23068672
max_frame_bytes_tx_gossip = 13631488

[sumeragi.block]
max_payload_bytes = 16777216

[sumeragi.queues]
authenticated_non_validator_sources = 2
body_bytes = 242221056
body_source_bytes = 34603008

[torii]
address = "0.0.0.0:18080"
public_address = "https://taira-validator-1.sora.org"

[torii.mcp]
enabled = true

[torii.kagemusha_commands]
enabled = true
private_key_file = "/run/secrets/iroha/taira-kagemusha-commands-private-key"

[soracloud_runtime.submission.signer]
handle = "REPLACE_WITH_SORACLOUD_RUNTIME_SIGNER_HANDLE"
authority = "REPLACE_WITH_SORACLOUD_RUNTIME_SIGNER_AUTHORITY"
algorithm = "REPLACE_WITH_SORACLOUD_RUNTIME_SIGNER_ALGORITHM"
public_key_hex = "REPLACE_WITH_SORACLOUD_RUNTIME_SIGNER_PUBLIC_KEY_HEX"
revision = "REPLACE_WITH_SORACLOUD_RUNTIME_SIGNER_REVISION"
policy_digest_hex = "REPLACE_WITH_SORACLOUD_RUNTIME_SIGNER_POLICY_DIGEST_HEX"

[nexus.registry]
manifest_directory = "configs/soranexus/taira/manifests"
cache_directory = "configs/soranexus/taira/manifests"
poll_interval_ms = 10000

[[nexus.dataspace_catalog]]
alias = "dpn"
id = 10

[[nexus.dataspace_catalog]]
alias = "is"
id = 11

[[nexus.dataspace_catalog]]
alias = "is2"
id = 12

[torii.account_onboarding]
authority = "REPLACE_WITH_TAIRA_ONBOARDING_AUTHORITY"
private_key_file = "REPLACE_WITH_TAIRA_ONBOARDING_PRIVATE_KEY_FILE"
lease_term_years = 1
additional_permissions = []

[[torii.account_onboarding.credentials]]
id = "REPLACE_WITH_TAIRA_BOI_ONBOARDING_CREDENTIAL_ID"
scope = { dataspace = "is2" }
token_hash = "REPLACE_WITH_TAIRA_BOI_ONBOARDING_TOKEN_HASH"

[[torii.account_onboarding.credentials]]
id = "REPLACE_WITH_TAIRA_DPN_ONBOARDING_CREDENTIAL_ID"
scope = { dataspace = "dpn" }
token_hash = "REPLACE_WITH_TAIRA_DPN_ONBOARDING_TOKEN_HASH"

[torii.faucet]
authority = "REPLACE_WITH_TAIRA_FAUCET_AUTHORITY"
private_key_file = "REPLACE_WITH_TAIRA_FAUCET_PRIVATE_KEY_FILE"

[streaming]
identity_public_key = "REPLACE_WITH_STREAMING_IDENTITY_PUBLIC_KEY"
identity_private_key_file = "/run/secrets/iroha/taira-streaming-identity-private-key"

[sorafs.discovery.admission]
envelopes_dir = "configs/soranexus/taira/sorafs_admission"
trusted_council_keys = ["REPLACE_WITH_TAIRA_SORAFS_COUNCIL_PUBLIC_KEY"]
signature_threshold = "REPLACE_WITH_TAIRA_SORAFS_COUNCIL_SIGNATURE_THRESHOLD"
"""

RELEASE_PRIVACY_ISSUER_SECTION = """
[torii.privacy_bootle_lantern_issuer]
enabled = true
state_dir = "/var/lib/iroha/taira-validator-1/privacy/bootle-lantern/issuer"
max_inflight = 2
authorization_lifetime_blocks = 300
max_records = 4096
max_total_bytes = 13557760
terminal_retention_blocks = 4096
issuer_id_hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
policy_id_hex = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"
runtime_provider_registry_handle = "runtime://privacy/bootle-lantern/taira-primary"
runtime_provider_registry_revision = 1
runtime_provider_registry_policy_digest_hex = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"
"""


def _write_roster(
    path: Path, validator_count: int = 4, inline_private_keys: bool = True
) -> None:
    validators = []
    for index in range(1, validator_count + 1):
        entry = [
            "[[validators]]",
            f'slug = "taira-validator-{index}"',
            f'account_id = "test-validator-{index}"',
            f'public_key = "peer-{index}-public"',
        ]
        if inline_private_keys:
            entry.extend(
                [
                    f'private_key = "peer-{index}-private"',
                    f'soranet_transport_public_key = "peer-{index}-soranet-public"',
                    f'soranet_transport_private_key = "peer-{index}-soranet-private"',
                ]
            )
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
        'torii_faucet_authority = "faucet-authority"',
        'torii_faucet_private_key = "faucet-private-key"',
        'kagemusha_commands_private_key = "kagemusha-commands-private-key"',
        f'soracloud_runtime_signer_handle = "{SORACLOUD_SIGNER_HANDLE}"',
        f'soracloud_runtime_signer_authority = "{SORACLOUD_SIGNER_AUTHORITY}"',
        'soracloud_runtime_signer_algorithm = "ed25519"',
        f'soracloud_runtime_signer_public_key_hex = "{SORACLOUD_SIGNER_PUBLIC_KEY_HEX}"',
        "soracloud_runtime_signer_revision = 1",
        (
            "soracloud_runtime_signer_policy_digest_hex = "
            f'"{SORACLOUD_SIGNER_POLICY_DIGEST_HEX}"'
        ),
        'streaming_identity_public_key = "streaming-public-key"',
        'streaming_identity_private_key = "streaming-private-key"',
        f'sorafs_council_public_keys = ["{COUNCIL_KEY_1}", "{COUNCIL_KEY_2}", "{COUNCIL_KEY_3}"]',
        "sorafs_council_signature_threshold = 2",
        "",
        "[[shared.account_onboarding_credentials]]",
        'id = "boi-mobile"',
        f'api_token = "{ONBOARDING_TOKEN}"',
        'scope_dataspace = "is2"',
        "",
        "[[shared.account_onboarding_credentials]]",
        'id = "dpn-api"',
        f'api_token = "{DPN_ONBOARDING_TOKEN}"',
        'scope_dataspace = "dpn"',
        "",
    ]
    for index in range(1, validator_count + 1):
        receipt_public_key, receipt_private_key = _receipt_keypair(index)
        validators.extend(
            [
                "[[validators]]",
                f'slug = "taira-validator-{index}"',
                f'private_key = "peer-{index}-private"',
                f'soranet_transport_public_key = "peer-{index}-soranet-public"',
                f'soranet_transport_private_key = "peer-{index}-soranet-private"',
                f'receipt_public_key = "{receipt_public_key}"',
                f'receipt_private_key = "{receipt_private_key}"',
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
    assert 'private_key_file = "/etc/iroha/taira-validator/runtime/validator-signer.key"' in config
    assert 'soranet_transport_public_key = "peer-3-soranet-public"' in config
    assert (
        'soranet_transport_private_key_file = '
        '"/etc/iroha/taira-validator/runtime/soranet-transport.key"' in config
    )
    receipt_public_key, receipt_private_key = _receipt_keypair(3)
    assert f'receipt_public_key = "{receipt_public_key}"' in config
    assert f'receipt_private_key = "{receipt_private_key}"' in config
    assert 'expected_hash_file = "/run/iroha/genesis.expected_hash"' in config
    assert 'public_address = "addr:taira-validator-3.sora.org:1337#99FF"' in config
    assert 'address = "addr:0.0.0.0:1337#BF18"' in config
    assert 'address = "addr:0.0.0.0:18080#2F16"' in config
    assert '"peer-4-public@addr:taira-validator-4.sora.org:1337#E168"' in config
    assert '{ public_key = "peer-2-public", pop_hex = "peer-2-pop" }' in config
    assert f"bodies = {MODULE.SUMERAGI_DEFAULT_BODY_CAPACITY}" in config
    assert 'authority = "bootstrap-authority"' in config
    assert 'authority = "faucet-authority"' in config
    assert (
        'private_key_file = "/etc/iroha/taira-validator/runtime/'
        'kagemusha-command-signer.key"' in config
    )
    assert f'handle = "{SORACLOUD_SIGNER_HANDLE}"' in config
    assert f'authority = "{SORACLOUD_SIGNER_AUTHORITY}"' in config
    assert 'algorithm = "ed25519"' in config
    assert f'public_key_hex = "{SORACLOUD_SIGNER_PUBLIC_KEY_HEX}"' in config
    assert "revision = 1" in config
    assert f'policy_digest_hex = "{SORACLOUD_SIGNER_POLICY_DIGEST_HEX}"' in config
    assert "escrow_accounts" not in config
    assert 'id = "boi-mobile"' in config
    assert 'scope = { dataspace = "is2" }' in config
    assert 'id = "dpn-api"' in config
    assert 'scope = { dataspace = "dpn" }' in config
    assert MODULE._blake3_token_hash(ONBOARDING_TOKEN) in config
    assert MODULE._blake3_token_hash(DPN_ONBOARDING_TOKEN) in config
    assert "bootstrap-private-key" not in config
    assert "faucet-private-key" not in config
    assert "peer-3-private" not in config
    assert "peer-3-soranet-private" not in config
    assert "kagemusha-commands-private-key" not in config
    assert "streaming-private-key" not in config
    assert ONBOARDING_TOKEN not in config
    assert DPN_ONBOARDING_TOKEN not in config
    credentials = tomllib.loads(config)["torii"]["account_onboarding"]["credentials"]
    assert credentials == [
        {
            "id": "boi-mobile",
            "scope": {"dataspace": "is2"},
            "token_hash": MODULE._blake3_token_hash(ONBOARDING_TOKEN),
        },
        {
            "id": "dpn-api",
            "scope": {"dataspace": "dpn"},
            "token_hash": MODULE._blake3_token_hash(DPN_ONBOARDING_TOKEN),
        },
    ]
    runtime_dir = output_dir / "taira-validator-3" / "runtime"
    validator_key = runtime_dir / "validator-signer.key"
    soranet_key = runtime_dir / "soranet-transport.key"
    onboarding_key = runtime_dir / "onboarding-signer.key"
    faucet_key = runtime_dir / "faucet-signer.key"
    kagemusha_key = runtime_dir / "kagemusha-command-signer.key"
    streaming_key = runtime_dir / "streaming-identity.key"
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
    assert validator_key.read_text(encoding="utf-8") == "peer-3-private\n"
    assert soranet_key.read_text(encoding="utf-8") == "peer-3-soranet-private\n"
    assert onboarding_key.read_text(encoding="utf-8") == "bootstrap-private-key\n"
    assert faucet_key.read_text(encoding="utf-8") == "faucet-private-key\n"
    assert (
        kagemusha_key.read_text(encoding="utf-8")
        == "kagemusha-commands-private-key\n"
    )
    assert streaming_key.read_text(encoding="utf-8") == "streaming-private-key\n"
    assert token_file.read_text(encoding="utf-8") == ONBOARDING_TOKEN + "\n"
    assert stat.S_IMODE(output_dir.stat().st_mode) == 0o700
    assert stat.S_IMODE((output_dir / "taira-validator-3").stat().st_mode) == 0o700
    assert stat.S_IMODE(runtime_dir.stat().st_mode) == 0o700
    assert stat.S_IMODE(validator_key.stat().st_mode) == 0o600
    assert stat.S_IMODE(soranet_key.stat().st_mode) == 0o600
    assert stat.S_IMODE(onboarding_key.stat().st_mode) == 0o600
    assert stat.S_IMODE(faucet_key.stat().st_mode) == 0o600
    assert stat.S_IMODE(kagemusha_key.stat().st_mode) == 0o600
    assert stat.S_IMODE(streaming_key.stat().st_mode) == 0o600
    assert stat.S_IMODE(token_file.stat().st_mode) == 0o600
    assert (output_dir / ".gitignore").read_text(encoding="utf-8") == "*\n!.gitignore\n"
    assert 'identity_public_key = "streaming-public-key"' in config
    assert (
        'identity_private_key_file = "/etc/iroha/taira-validator/runtime/'
        'streaming-identity.key"' in config
    )
    assert (
        f'trusted_council_keys = ["{COUNCIL_KEY_1}", "{COUNCIL_KEY_2}", "{COUNCIL_KEY_3}"]'
        in config
    )
    assert "signature_threshold = 2" in config
    assert 'manifest_directory = "/etc/iroha/taira-validator/manifests"' in config
    assert 'cache_directory = "/etc/iroha/taira-validator/manifests"' in config
    assert 'envelopes_dir = "/etc/iroha/taira-validator/sorafs_admission"' in config
    assert not (output_dir / "taira-validator-3" / "kagemusha").exists()
    assert (
        stat.S_IMODE(
            (output_dir / "taira-validator-3" / "sorafs_admission").stat().st_mode
        )
        == 0o700
    )

    manifest_path = (
        output_dir / "taira-validator-3" / "manifests" / "governance.manifest.json"
    )
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    assert manifest["lane"] == "governance"
    assert manifest["governance"] == "parliament"
    assert manifest["quorum"] == 3
    assert manifest["validators"] == [
        {
            "validator": "test-validator-1",
            "peer_id": "peer-1-public",
            "torii_url": "https://taira-validator-1.sora.org",
        },
        {
            "validator": "test-validator-2",
            "peer_id": "peer-2-public",
            "torii_url": "https://taira-validator-2.sora.org",
        },
        {
            "validator": "test-validator-3",
            "peer_id": "peer-3-public",
            "torii_url": "https://taira-validator-3.sora.org",
        },
        {
            "validator": "test-validator-4",
            "peer_id": "peer-4-public",
            "torii_url": "https://taira-validator-4.sora.org",
        },
    ]


def test_render_bundle_rejects_roster_slug_traversal_before_writing(
    tmp_path: Path,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    base_config_path = tmp_path / "config.toml"
    output_dir = tmp_path / "out"
    escaped = tmp_path / "escaped"
    _write_roster(roster_path)
    roster_path.write_text(
        roster_path.read_text(encoding="utf-8").replace(
            'slug = "taira-validator-1"', 'slug = "../escaped"', 1
        ),
        encoding="utf-8",
    )
    _write_secrets(secrets_path)
    base_config_path.write_text(BASE_CONFIG, encoding="utf-8")

    with pytest.raises(ValueError, match="slug must be exactly"):
        MODULE.render_bundle(
            base_config_path, roster_path, output_dir, secrets_path=secrets_path
        )

    assert not output_dir.exists()
    assert not escaped.exists()


@pytest.mark.parametrize(
    "relative_target",
    (
        "taira-validator-1/config.toml",
        "taira-validator-1/runtime/onboarding-signer.key",
        "taira-validator-1/manifests/governance.manifest.json",
    ),
)
def test_render_bundle_never_follows_planted_output_symlinks(
    tmp_path: Path, relative_target: str
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    base_config_path = tmp_path / "config.toml"
    output_dir = tmp_path / "out"
    planted = output_dir / relative_target
    victim = tmp_path / "victim"
    _write_roster(roster_path)
    _write_secrets(secrets_path)
    base_config_path.write_text(BASE_CONFIG, encoding="utf-8")
    victim.write_text("must-survive\n", encoding="utf-8")
    victim.chmod(0o600)
    planted.parent.mkdir(parents=True, mode=0o700)
    for parent in (output_dir, output_dir / "taira-validator-1"):
        parent.chmod(0o700)
    if planted.parent.name in {"runtime", "manifests"}:
        planted.parent.chmod(0o700)
    planted.symlink_to(victim)

    with pytest.raises(ValueError, match="safe regular file"):
        MODULE.render_bundle(
            base_config_path, roster_path, output_dir, secrets_path=secrets_path
        )

    assert victim.read_text(encoding="utf-8") == "must-survive\n"


def test_render_bundle_never_overwrites_a_hardlinked_output(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    base_config_path = tmp_path / "config.toml"
    output_dir = tmp_path / "out"
    planted = output_dir / "taira-validator-1/config.toml"
    victim = tmp_path / "victim"
    _write_roster(roster_path)
    _write_secrets(secrets_path)
    base_config_path.write_text(BASE_CONFIG, encoding="utf-8")
    victim.write_text("must-survive\n", encoding="utf-8")
    victim.chmod(0o600)
    planted.parent.mkdir(parents=True, mode=0o700)
    output_dir.chmod(0o700)
    planted.parent.chmod(0o700)
    os.link(victim, planted)

    with pytest.raises(ValueError, match="safe regular file"):
        MODULE.render_bundle(
            base_config_path, roster_path, output_dir, secrets_path=secrets_path
        )

    assert victim.read_text(encoding="utf-8") == "must-survive\n"


def test_release_privacy_issuer_is_enabled_only_on_designated_peer_one(
    tmp_path: Path,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    base_config_path = tmp_path / "config.toml"
    output_dir = tmp_path / "out"
    _write_roster(roster_path)
    _write_secrets(secrets_path)
    base_config_path.write_text(
        BASE_CONFIG + RELEASE_PRIVACY_ISSUER_SECTION,
        encoding="utf-8",
    )

    MODULE.render_bundle(
        base_config_path,
        roster_path,
        output_dir,
        secrets_path=secrets_path,
    )

    for index in range(1, 5):
        config = tomllib.loads(
            (output_dir / f"taira-validator-{index}" / "config.toml").read_text(
                encoding="utf-8"
            )
        )
        issuer = config["torii"]["privacy_bootle_lantern_issuer"]
        assert issuer["enabled"] is (index == 1)
        assert issuer["state_dir"] == (
            f"/var/lib/iroha/taira-validator-{index}/privacy/bootle-lantern/issuer"
        )
        if index == 1:
            assert set(issuer) == (
                MODULE.TAIRA_PRIVACY_ISSUER_BASE_FIELDS
                | MODULE.TAIRA_PRIVACY_ISSUER_BINDING_FIELDS
            )
            assert issuer["runtime_provider_registry_revision"] == 1
        else:
            assert set(issuer) == MODULE.TAIRA_PRIVACY_ISSUER_BASE_FIELDS
            for field in MODULE.TAIRA_PRIVACY_ISSUER_BINDING_FIELDS:
                assert field not in issuer


def test_bundle_local_release_render_binds_every_runtime_path_inside_reset(
    tmp_path: Path,
) -> None:
    bundle_root = tmp_path / "private-reset"
    bundle_root.mkdir(mode=0o700)
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    base_config_path = tmp_path / "config.toml"
    _write_roster(roster_path)
    _write_secrets(secrets_path)
    base_config_path.write_text(
        BASE_CONFIG + RELEASE_PRIVACY_ISSUER_SECTION,
        encoding="utf-8",
    )
    expected_hash = "00" * 31 + "01"

    written = MODULE.render_bundle(
        base_config_path,
        roster_path,
        bundle_root / "rendered",
        secrets_path=secrets_path,
        genesis_expected_hash=expected_hash,
        bundle_root=bundle_root,
    )

    assert len(written) == 4
    assert not (bundle_root / "rendered/.gitignore").exists()
    expected_hash_literal = MODULE._format_literal("hash", expected_hash.upper())
    for index, config_path in enumerate(written, start=1):
        root = config_path.parent
        config = tomllib.loads(config_path.read_text(encoding="utf-8"))
        assert config["genesis"]["file"] == str(bundle_root / "genesis.signed.nrt")
        assert config["genesis"]["expected_hash"] == expected_hash_literal
        issuer = config["torii"]["privacy_bootle_lantern_issuer"]
        assert issuer["state_dir"] == str(
            root / "runtime/privacy/bootle-lantern/issuer"
        )
        assert issuer["enabled"] is (index == 1)
        assert config["nexus"]["registry"]["manifest_directory"] == str(
            root / "manifests"
        )
        assert config["nexus"]["registry"]["cache_directory"] == str(root / "manifests")
        assert config["sorafs"]["discovery"]["admission"]["envelopes_dir"] == str(
            root / "configs/soranexus/taira/sorafs_admission"
        )
        assert not (root / "sorafs_admission").exists()


def test_bundle_local_release_render_rejects_output_outside_bundle(
    tmp_path: Path,
) -> None:
    bundle_root = tmp_path / "private-reset"
    bundle_root.mkdir(mode=0o700)
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    base_config_path = tmp_path / "config.toml"
    _write_roster(roster_path)
    _write_secrets(secrets_path)
    base_config_path.write_text(BASE_CONFIG, encoding="utf-8")

    with pytest.raises(ValueError, match="bundle_root/rendered"):
        MODULE.render_bundle(
            base_config_path,
            roster_path,
            tmp_path / "foreign-rendered",
            secrets_path=secrets_path,
            bundle_root=bundle_root,
        )


@pytest.mark.parametrize(
    "mutation",
    (
        lambda section: section.replace(
            'policy_id_hex = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"\n',
            "",
            1,
        ),
        lambda section: section.replace("max_inflight = 2", "max_inflight = 3", 1),
        lambda section: section.replace("enabled = true", 'enabled = "true"', 1),
        lambda section: (
            section + 'runtime_provider_registry_policy_digest_hex_alias = "00"\n'
        ),
    ),
    ids=("partial-binding", "wrong-bound", "non-boolean", "unknown-binding"),
)
def test_release_privacy_issuer_template_rejects_adversarial_mutations(
    tmp_path: Path,
    mutation,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    base_config_path = tmp_path / "config.toml"
    _write_roster(roster_path)
    _write_secrets(secrets_path)
    base_config_path.write_text(
        BASE_CONFIG + mutation(RELEASE_PRIVACY_ISSUER_SECTION),
        encoding="utf-8",
    )

    with pytest.raises((ValueError, tomllib.TOMLDecodeError)):
        MODULE.render_bundle(
            base_config_path,
            roster_path,
            tmp_path / "out",
            secrets_path=secrets_path,
        )


def test_render_bundle_binds_exact_genesis_hash_after_signing(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    base_config_path = tmp_path / "config.toml"
    output_dir = tmp_path / "out"
    _write_roster(roster_path)
    _write_secrets(secrets_path)
    base_config_path.write_text(BASE_CONFIG, encoding="utf-8")
    expected_hash = "ab" * 31 + "cd"

    written = MODULE.render_bundle(
        base_config_path,
        roster_path,
        output_dir,
        secrets_path=secrets_path,
        genesis_expected_hash=expected_hash,
    )

    config = written[0].read_text(encoding="utf-8")
    expected_hash_literal = MODULE._format_literal("hash", expected_hash.upper())
    assert expected_hash_literal == f"hash:{expected_hash.upper()}#BF3A"
    assert f'expected_hash = "{expected_hash_literal}"' in config
    assert MODULE.GENESIS_EXPECTED_HASH_PLACEHOLDER not in config


def test_private_genesis_staging_hash_is_canonical_and_marker_bearing() -> None:
    staging_hash = MODULE.GENESIS_EXPECTED_HASH_PLACEHOLDER
    assert MODULE.GENESIS_EXPECTED_HASH_RE.fullmatch(staging_hash)
    assert int(staging_hash[-2:], 16) & 1 == 1


@pytest.mark.parametrize("expected_hash", ["00" * 32, "AA" * 31 + "01", "short"])
def test_render_bundle_rejects_invalid_genesis_hash(
    tmp_path: Path, expected_hash: str
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    base_config_path = tmp_path / "config.toml"
    _write_roster(roster_path)
    _write_secrets(secrets_path)
    base_config_path.write_text(BASE_CONFIG, encoding="utf-8")

    with pytest.raises(ValueError, match="genesis_expected_hash"):
        MODULE.render_bundle(
            base_config_path,
            roster_path,
            tmp_path / "out",
            secrets_path=secrets_path,
            genesis_expected_hash=expected_hash,
        )


def test_render_bundle_rejects_a_template_without_expected_hash(
    tmp_path: Path,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    base_config_path = tmp_path / "config.toml"
    _write_roster(roster_path)
    _write_secrets(secrets_path)
    base_config_path.write_text(
        BASE_CONFIG.replace(
            'expected_hash_file = "/run/iroha/genesis.expected_hash"\n', ""
        ),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="mandatory.*expected_hash"):
        MODULE.render_bundle(
            base_config_path,
            roster_path,
            tmp_path / "out",
            secrets_path=secrets_path,
        )


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
    assert 'manifest_directory = "/srv/iroha/taira-validator/manifests"' in config
    assert 'cache_directory = "/srv/iroha/taira-validator/manifests"' in config
    assert "kagemusha_release_policy_path" not in config
    assert 'envelopes_dir = "/srv/iroha/taira-validator/sorafs_admission"' in config


def test_render_bundle_opt_in_rewrites_complete_kagemusha_catalog_paths(
    tmp_path: Path,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    base_config_path = tmp_path / "config.toml"
    output_dir = tmp_path / "out"
    install_root = Path("/srv/iroha/taira-validator")
    release_root = Path("/srv/iroha-kagemusha/taira")
    _write_roster(roster_path)
    _write_secrets(secrets_path)
    base_config_path.write_text(BASE_CONFIG, encoding="utf-8")

    MODULE.render_bundle(
        base_config_path,
        roster_path,
        output_dir,
        secrets_path=secrets_path,
        install_root=install_root,
        kagemusha_release_root=release_root,
    )

    config_path = output_dir / "taira-validator-1" / "config.toml"
    config = tomllib.loads(config_path.read_text(encoding="utf-8"))
    offline = config["settlement"]["offline"]
    assert offline == {
        "kagemusha_release_policy_path": (
            "/srv/iroha-kagemusha/taira/policy/release-policy-v1.norito"
        ),
        "kagemusha_artifact_dir": "/srv/iroha-kagemusha/taira/catalog",
        "kagemusha_catalog_qualification_seal_path": (
            "/srv/iroha-kagemusha/taira/seals/catalog-qualification-v1.norito"
        ),
        "kagemusha_max_decoded_bytes": 256 * MODULE.MIB,
    }
    assert not (output_dir / "taira-validator-1" / "taira-release").exists()
    assert not (output_dir / "taira-validator-1" / "kagemusha-qualified").exists()


def test_render_bundle_genesis_staging_omits_only_kagemusha_qualification_seal(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    base_config_path = tmp_path / "config.toml"
    output_dir = tmp_path / "out"
    release_root = Path("/srv/iroha-kagemusha/taira")
    _write_roster(roster_path)
    _write_secrets(secrets_path)
    base_config_path.write_text(BASE_CONFIG, encoding="utf-8")
    monkeypatch.setattr(MODULE, "_blake3_token_hash", lambda *_args: "blake3:" + "11" * 32)

    MODULE.render_bundle(
        base_config_path,
        roster_path,
        output_dir,
        secrets_path=secrets_path,
        install_root=Path("/srv/iroha/taira-validator"),
        kagemusha_release_root=release_root,
        include_kagemusha_qualification_seal=False,
    )

    config = tomllib.loads(
        (output_dir / "taira-validator-1" / "config.toml").read_text(
            encoding="utf-8"
        )
    )
    offline = config["settlement"]["offline"]
    assert offline["kagemusha_release_policy_path"] == str(
        release_root / MODULE.KAGEMUSHA_RELEASE_POLICY_RELATIVE_PATH
    )
    assert offline["kagemusha_artifact_dir"] == str(
        release_root / MODULE.KAGEMUSHA_ARTIFACT_RELATIVE_PATH
    )
    assert "kagemusha_catalog_qualification_seal_path" not in offline
    assert offline["kagemusha_max_decoded_bytes"] == MODULE.KAGEMUSHA_MAX_DECODED_BYTES


def test_render_bundle_opt_in_replaces_existing_kagemusha_catalog_settings(
    tmp_path: Path,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    base_config_path = tmp_path / "config.toml"
    output_dir = tmp_path / "out"
    install_root = Path("/srv/iroha/taira-validator")
    release_root = Path("/srv/iroha-kagemusha/taira")
    _write_roster(roster_path)
    _write_secrets(secrets_path)
    base_config_path.write_text(
        BASE_CONFIG
        + "\n[settlement.offline]\n"
        + 'kagemusha_release_policy_path = "/stale/policy"\n'
        + 'kagemusha_artifact_dir = "/stale/catalog"\n'
        + 'kagemusha_catalog_qualification_seal_path = "/stale/seal"\n'
        + "kagemusha_max_decoded_bytes = 1\n",
        encoding="utf-8",
    )

    MODULE.render_bundle(
        base_config_path,
        roster_path,
        output_dir,
        secrets_path=secrets_path,
        install_root=install_root,
        kagemusha_release_root=release_root,
    )

    config_path = output_dir / "taira-validator-1" / "config.toml"
    offline = tomllib.loads(config_path.read_text(encoding="utf-8"))["settlement"][
        "offline"
    ]
    assert offline["kagemusha_release_policy_path"] == str(
        release_root / MODULE.KAGEMUSHA_RELEASE_POLICY_RELATIVE_PATH
    )
    assert offline["kagemusha_artifact_dir"] == str(
        release_root / MODULE.KAGEMUSHA_ARTIFACT_RELATIVE_PATH
    )
    assert offline["kagemusha_catalog_qualification_seal_path"] == str(
        release_root / MODULE.KAGEMUSHA_QUALIFICATION_SEAL_RELATIVE_PATH
    )
    assert offline["kagemusha_max_decoded_bytes"] == MODULE.KAGEMUSHA_MAX_DECODED_BYTES


@pytest.mark.parametrize(
    "managed_settings",
    (
        'kagemusha_release_policy_path = "/stale/policy"\n',
        (
            'kagemusha_release_policy_path = "/stale/policy"\n'
            'kagemusha_artifact_dir = "/stale/catalog"\n'
            'kagemusha_catalog_qualification_seal_path = "/stale/seal"\n'
        ),
    ),
)
def test_render_bundle_requires_explicit_opt_in_for_managed_kagemusha_paths(
    tmp_path: Path, managed_settings: str
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    base_config_path = tmp_path / "config.toml"
    _write_roster(roster_path)
    base_config_path.write_text(
        BASE_CONFIG + "\n[settlement.offline]\n" + managed_settings,
        encoding="utf-8",
    )

    with pytest.raises(
        ValueError, match="managed Kagemusha release paths without"
    ):
        MODULE.render_bundle(
            base_config_path,
            roster_path,
            tmp_path / "out",
            install_root=Path("/srv/iroha/taira-validator"),
        )


def test_render_bundle_opt_in_keeps_kagemusha_paths_outside_bundle_root(
    tmp_path: Path,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    base_config_path = tmp_path / "config.toml"
    bundle_root = tmp_path / "private-reset"
    release_root = Path("/srv/iroha-kagemusha/taira")
    bundle_root.mkdir(mode=0o700)
    output_dir = bundle_root / "rendered"
    _write_roster(roster_path)
    _write_secrets(secrets_path)
    base_config_path.write_text(BASE_CONFIG, encoding="utf-8")

    MODULE.render_bundle(
        base_config_path,
        roster_path,
        output_dir,
        secrets_path=secrets_path,
        bundle_root=bundle_root,
        kagemusha_release_root=release_root,
    )

    validator_root = output_dir / "taira-validator-1"
    config = tomllib.loads((validator_root / "config.toml").read_text(encoding="utf-8"))
    offline = config["settlement"]["offline"]
    assert offline["kagemusha_release_policy_path"] == str(
        release_root / MODULE.KAGEMUSHA_RELEASE_POLICY_RELATIVE_PATH
    )
    assert offline["kagemusha_artifact_dir"] == str(
        release_root / MODULE.KAGEMUSHA_ARTIFACT_RELATIVE_PATH
    )
    assert offline["kagemusha_catalog_qualification_seal_path"] == str(
        release_root / MODULE.KAGEMUSHA_QUALIFICATION_SEAL_RELATIVE_PATH
    )


def test_render_bundle_rejects_noncanonical_or_overlapping_kagemusha_release_roots(
    tmp_path: Path,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    base_config_path = tmp_path / "config.toml"
    install_root = Path("/srv/iroha/taira-validator")
    _write_roster(roster_path)
    base_config_path.write_text(BASE_CONFIG, encoding="utf-8")

    for index, invalid_root in enumerate(
        (
            Path("relative/kagemusha"),
            Path("/"),
            Path("/srv/iroha/../kagemusha"),
            Path("//srv/iroha-kagemusha"),
            Path("/srv/iroha-kagemusha\ninjected"),
            Path("/srv/iroha"),
            install_root,
            install_root / "kagemusha",
        )
    ):
        with pytest.raises(ValueError, match="kagemusha_release_root"):
            MODULE.render_bundle(
                base_config_path,
                roster_path,
                tmp_path / f"invalid-release-root-{index}",
                install_root=install_root,
                kagemusha_release_root=invalid_root,
            )


def test_render_bundle_rejects_kagemusha_release_root_overlapping_output_dir(
    tmp_path: Path,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    base_config_path = tmp_path / "config.toml"
    output_dir = tmp_path / "rendered"
    _write_roster(roster_path)
    base_config_path.write_text(BASE_CONFIG, encoding="utf-8")

    for release_root in (output_dir, output_dir / "release", tmp_path):
        with pytest.raises(ValueError, match="kagemusha_release_root"):
            MODULE.render_bundle(
                base_config_path,
                roster_path,
                output_dir,
                install_root=Path("/srv/iroha/taira-validator"),
                kagemusha_release_root=release_root,
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
        MODULE._canonical_socket_address("TAIRA-VALIDATOR-1.SORA.ORG:1337", "fixture")
        == "addr:taira-validator-1.sora.org:1337#D426"
    )
    assert (
        MODULE._canonical_socket_address("addr:127.0.0.1:39080#4B72", "fixture")
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
            raise AssertionError(
                f"renderer accepted invalid socket address {invalid!r}"
            )


def test_render_bundle_injects_public_roster_into_unsigned_genesis(
    tmp_path: Path,
) -> None:
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
    assert '"$TAIRA_GENESIS_EXTERNAL_SIGNER"' in signing_command
    assert "private-key" not in signing_command.lower()
    assert "kagami" not in signing_command.lower()
    assert "--unsigned-genesis" in signing_command
    assert "--peer-config" in signing_command
    assert "--signed-genesis-out" in signing_command
    assert "taira-validator-1/config.toml" in signing_command
    assert f"--bound-manifest-out {output_dir / 'genesis.json'}" in signing_command
    assert (
        f"--expected-hash-out {output_dir / 'genesis.expected_hash'}" in signing_command
    )


def test_genesis_renderer_preserves_fresh_contract_deployment_gate(
    tmp_path: Path,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    output_dir = tmp_path / "out"
    _write_roster(roster_path)
    output_dir.mkdir(mode=0o700)
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
    assert _asset_alias_projection(rendered) == _asset_alias_projection(checked_in)
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
                'soranet_transport_public_key = "peer-1-soranet-public"',
                'soranet_transport_private_key = "peer-1-soranet-private"',
                'pop_hex = "peer-1-pop"',
                'public_address = "taira-validator-1.sora.org:1337"',
                "",
                "[[validators]]",
                'slug = "taira-validator-2"',
                'account_id = "test-validator-2"',
                'public_key = "peer-2-public"',
                'private_key = "peer-2-private"',
                'soranet_transport_public_key = "peer-2-soranet-public"',
                'soranet_transport_private_key = "peer-2-soranet-private"',
                'pop_hex = "peer-2-pop"',
                'public_address = "taira-validator-2.sora.org:1337"',
                "",
                "[[validators]]",
                'slug = "taira-validator-3"',
                'account_id = "test-validator-3"',
                'public_key = "peer-3-public"',
                'private_key = "peer-3-private"',
                'soranet_transport_public_key = "peer-3-soranet-public"',
                'soranet_transport_private_key = "peer-3-soranet-private"',
                'pop_hex = "peer-3-pop"',
                'public_address = "taira-validator-3.sora.org:1337"',
                "",
                "[[validators]]",
                'slug = "taira-validator-4"',
                'account_id = "test-validator-4"',
                'public_key = "peer-4-public"',
                'private_key = "peer-4-private"',
                'soranet_transport_public_key = "peer-4-soranet-public"',
                'soranet_transport_private_key = "peer-4-soranet-private"',
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


def test_load_roster_canonicalizes_and_deduplicates_torii_origins(
    tmp_path: Path,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    _write_roster(roster_path)
    roster = roster_path.read_text(encoding="utf-8").replace(
        'torii_public_address = "https://taira-validator-2.sora.org"',
        'torii_public_address = "https://TAIRA-VALIDATOR-1.SORA.ORG.:443/"',
        1,
    )
    roster_path.write_text(roster, encoding="utf-8")

    with pytest.raises(ValueError, match="torii_public_address.*duplicated"):
        MODULE.load_roster(roster_path)


@pytest.mark.parametrize(
    "torii_origin",
    (
        "http://taira-validator-1.sora.org",
        "https://user@taira-validator-1.sora.org",
        "https://taira-validator-1.sora.org/v1",
        "https://taira-validator-1.sora.org?query=1",
        "https://taira-validator-1.sora.org#fragment",
        "https://bad host.example",
        "https://%41.example",
        "https://127.000.000.001",
        "https://taira-validator-1.sora.org:0",
        "https://taira-validator-1.sora.org:65536",
        " https://taira-validator-1.sora.org",
    ),
)
def test_load_roster_rejects_noncanonical_torii_origin(
    tmp_path: Path,
    torii_origin: str,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    _write_roster(roster_path)
    roster = roster_path.read_text(encoding="utf-8").replace(
        "https://taira-validator-1.sora.org",
        torii_origin,
        1,
    )
    roster_path.write_text(roster, encoding="utf-8")

    with pytest.raises(ValueError, match="torii_public_address"):
        MODULE.load_roster(roster_path)


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
        assert "at most 31 validators" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("load_roster accepted a roster above the protocol maximum")


def test_load_roster_rejects_non_three_f_plus_one_geometry(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    _write_roster(roster_path, validator_count=5)

    try:
        MODULE.load_roster(roster_path)
    except ValueError as error:
        assert "exact 3f + 1 validator committee" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("load_roster accepted non-3f+1 committee geometry")


def test_render_bundle_scales_body_budget_for_seven_validators(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    base_config_path = tmp_path / "config.toml"
    output_dir = tmp_path / "out"
    _write_roster(roster_path, validator_count=7, inline_private_keys=False)
    _write_secrets(secrets_path, validator_count=7)
    base_config_path.write_text(BASE_CONFIG, encoding="utf-8")

    MODULE.render_bundle(
        base_config_path,
        roster_path,
        output_dir,
        secrets_path=secrets_path,
    )

    rendered = MODULE._load_toml(
        output_dir / "taira-validator-7" / "config.toml"
    )
    queues = rendered["sumeragi"]["queues"]
    assert queues["authenticated_non_validator_sources"] == 2
    assert queues["body_source_bytes"] == 34_603_008
    assert queues["body_bytes"] == 10 * queues["body_source_bytes"]


def test_render_bundle_rejects_non_positive_queue_template_values(
    tmp_path: Path,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    _write_roster(roster_path)

    malformed = {
        "authenticated_non_validator_sources": ["0", "-1", '"2"', "true"],
        "body_bytes": ["0", "-1", '"242221056"', "true"],
        "body_source_bytes": ["0", "-1", '"34603008"', "true"],
    }
    defaults = {
        "authenticated_non_validator_sources": 2,
        "body_bytes": 242221056,
        "body_source_bytes": 34603008,
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
    ("key", "current"),
    [
        ("authenticated_non_validator_sources", "2"),
        ("bodies", None),
        ("body_bytes", "242221056"),
        ("body_source_bytes", "34603008"),
    ],
)
def test_render_bundle_rejects_queue_values_above_toml_i64(
    tmp_path: Path,
    key: str,
    current: str | None,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    _write_roster(roster_path)
    base_config_path = tmp_path / f"config-{key}.toml"
    template = BASE_CONFIG
    if current is None:
        template = template.replace(
            "[sumeragi.queues]\n",
            "[sumeragi.queues]\n"
            f"{key} = {MODULE.TOML_I64_MAX + 1}\n",
            1,
        )
    else:
        template = template.replace(
            f"{key} = {current}",
            f"{key} = {MODULE.TOML_I64_MAX + 1}",
        )
    base_config_path.write_text(
        template,
        encoding="utf-8",
    )

    with pytest.raises(
        ValueError,
        match=rf"field `{key}` must not exceed {MODULE.TOML_I64_MAX}",
    ):
        MODULE.render_bundle(base_config_path, roster_path, tmp_path / "out")


def test_scaled_sumeragi_bodies_increases_only_underbudget_capacity(
    tmp_path: Path,
) -> None:
    base_config_path = tmp_path / "config.toml"
    base_config_path.write_text(BASE_CONFIG, encoding="utf-8")
    defaulted = MODULE._load_toml(base_config_path)
    assert (
        MODULE._scaled_sumeragi_bodies(defaulted, 7)
        == MODULE.SUMERAGI_DEFAULT_BODY_CAPACITY
    )

    base_config_path.write_text(
        BASE_CONFIG.replace("[sumeragi.queues]\n", "[sumeragi.queues]\nbodies = 1\n"),
        encoding="utf-8",
    )
    underbudget = MODULE._load_toml(base_config_path)
    assert MODULE._scaled_sumeragi_bodies(underbudget, 7) == 43

    base_config_path.write_text(
        BASE_CONFIG.replace(
            "[sumeragi.queues]\n", "[sumeragi.queues]\nbodies = 211\n"
        ),
        encoding="utf-8",
    )
    authored = MODULE._load_toml(base_config_path)
    assert MODULE._scaled_sumeragi_bodies(authored, 7) == 211


def test_scaled_sumeragi_bodies_accepts_exact_toml_i64_boundaries(
    tmp_path: Path,
) -> None:
    base_config_path = tmp_path / "config.toml"
    base_config_path.write_text(
        BASE_CONFIG.replace(
            "[sumeragi.queues]\n",
            f"[sumeragi.queues]\nbodies = {MODULE.TOML_I64_MAX}\n",
        ),
        encoding="utf-8",
    )
    configured_boundary = MODULE._load_toml(base_config_path)
    assert (
        MODULE._scaled_sumeragi_bodies(configured_boundary, 7)
        == MODULE.TOML_I64_MAX
    )

    exact_authenticated_sources = (MODULE.TOML_I64_MAX - 1) // 3
    assert 3 * exact_authenticated_sources + 1 == MODULE.TOML_I64_MAX
    base_config_path.write_text(
        BASE_CONFIG.replace(
            "[sumeragi.queues]\nauthenticated_non_validator_sources = 2",
            "[sumeragi.queues]\n"
            "bodies = 1\n"
            f"authenticated_non_validator_sources = {exact_authenticated_sources}",
        ),
        encoding="utf-8",
    )
    derived_boundary = MODULE._load_toml(base_config_path)
    assert (
        MODULE._scaled_sumeragi_bodies(derived_boundary, 0)
        == MODULE.TOML_I64_MAX
    )


@pytest.mark.parametrize(
    ("validator_count", "authenticated_non_validator_sources"),
    [
        (0, (MODULE.TOML_I64_MAX - 1) // 3 + 1),
        (MODULE.TOML_I64_MAX // 5 + 1, 2),
    ],
)
def test_scaled_sumeragi_bodies_rejects_derived_toml_i64_overflow(
    tmp_path: Path,
    validator_count: int,
    authenticated_non_validator_sources: int,
) -> None:
    base_config_path = tmp_path / "config.toml"
    base_config_path.write_text(
        BASE_CONFIG.replace(
            "[sumeragi.queues]\nauthenticated_non_validator_sources = 2",
            "[sumeragi.queues]\n"
            "bodies = 1\n"
            "authenticated_non_validator_sources = "
            f"{authenticated_non_validator_sources}",
        ),
        encoding="utf-8",
    )
    template = MODULE._load_toml(base_config_path)

    with pytest.raises(
        ValueError,
        match=(
            "derived `sumeragi.queues.bodies` exceeds the Rust/TOML signed "
            "64-bit integer maximum"
        ),
    ):
        MODULE._scaled_sumeragi_bodies(template, validator_count)


def test_scaled_sumeragi_body_bytes_accepts_exact_toml_i64_boundaries(
    tmp_path: Path,
) -> None:
    base_config_path = tmp_path / "config.toml"

    base_config_path.write_text(
        BASE_CONFIG.replace(
            "body_bytes = 242221056",
            f"body_bytes = {MODULE.TOML_I64_MAX}",
        ),
        encoding="utf-8",
    )
    configured_boundary = MODULE._load_toml(base_config_path)
    assert (
        MODULE._scaled_sumeragi_body_bytes(configured_boundary, 4)
        == MODULE.TOML_I64_MAX
    )

    assert MODULE.TOML_I64_MAX % 7 == 0
    source_bytes = MODULE.TOML_I64_MAX // 7
    base_config_path.write_text(
        BASE_CONFIG.replace(
            "body_source_bytes = 34603008",
            f"body_source_bytes = {source_bytes}",
        ),
        encoding="utf-8",
    )
    derived_boundary = MODULE._load_toml(base_config_path)
    assert (
        MODULE._scaled_sumeragi_body_bytes(derived_boundary, 4)
        == MODULE.TOML_I64_MAX
    )


def test_render_bundle_rejects_derived_body_product_above_toml_i64(
    tmp_path: Path,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    base_config_path = tmp_path / "config.toml"
    _write_roster(roster_path)
    overflowing_source_bytes = MODULE.TOML_I64_MAX // 7 + 1
    base_config_path.write_text(
        BASE_CONFIG.replace(
            "body_source_bytes = 34603008",
            f"body_source_bytes = {overflowing_source_bytes}",
        ),
        encoding="utf-8",
    )

    with pytest.raises(
        ValueError,
        match=(
            "derived `sumeragi.queues.body_bytes` exceeds the Rust/TOML signed "
            "64-bit integer maximum"
        ),
    ):
        MODULE.render_bundle(base_config_path, roster_path, tmp_path / "out")


def test_render_bundle_rejects_derived_source_count_above_toml_i64(
    tmp_path: Path,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    base_config_path = tmp_path / "config.toml"
    _write_roster(roster_path)
    base_config_path.write_text(
        BASE_CONFIG.replace(
            "authenticated_non_validator_sources = 2",
            f"authenticated_non_validator_sources = {MODULE.TOML_I64_MAX}",
        ),
        encoding="utf-8",
    )

    with pytest.raises(
        ValueError,
        match=(
            "derived Sumeragi body source partition count .* exceeds the "
            "Rust/TOML signed 64-bit integer maximum"
        ),
    ):
        MODULE.render_bundle(base_config_path, roster_path, tmp_path / "out")


@pytest.mark.parametrize(
    ("field", "current", "replacement", "expected"),
    [
        (
            "max_payload_bytes",
            "16777216",
            str(MODULE.TAIRA_BLOCK_MAX_PAYLOAD_BYTES - 1),
            "must equal the revision-4 protocol ceiling of 16777216 bytes",
        ),
        (
            "max_payload_bytes",
            "16777216",
            str(MODULE.TAIRA_BLOCK_MAX_PAYLOAD_BYTES + 1),
            "must equal the revision-4 protocol ceiling of 16777216 bytes",
        ),
        (
            "body_source_bytes",
            "34603008",
            str(MODULE.TAIRA_BODY_SOURCE_MIN_BYTES - 1),
            "must be at least 33784840 bytes",
        ),
        (
            "max_frame_bytes_block_sync",
            "23068672",
            str(MODULE.TAIRA_BLOCK_SYNC_PLAINTEXT_FRAME_BYTES - 1),
            "must be at least 23068672 bytes",
        ),
        (
            "max_frame_bytes_tx_gossip",
            "13631488",
            str(MODULE.TAIRA_TX_GOSSIP_PLAINTEXT_FRAME_BYTES - 1),
            "must be at least 13631488 bytes",
        ),
        (
            "max_frame_bytes",
            "23068700",
            str(MODULE.TAIRA_MAX_FRAME_BYTES - 1),
            "must include 28 AEAD bytes",
        ),
    ],
)
def test_render_bundle_rejects_invalid_privacy_transport_corridor(
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


@pytest.mark.parametrize("delta", [-1, 1])
def test_genesis_renderer_rejects_da_payload_outside_protocol_ceiling(
    tmp_path: Path,
    delta: int,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    _write_roster(roster_path)
    validators = MODULE.load_roster(roster_path)
    genesis = json.loads(TAIRA_GENESIS_PATH.read_text(encoding="utf-8"))
    genesis["sumeragi_v2"]["da_layout"]["max_payload_size_bytes"] = (
        MODULE.TAIRA_BLOCK_MAX_PAYLOAD_BYTES + delta
    )
    base_genesis_path = tmp_path / "genesis.json"
    base_genesis_path.write_text(json.dumps(genesis), encoding="utf-8")

    with pytest.raises(
        ValueError,
        match="must equal the revision-4 protocol ceiling of 16777216",
    ):
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
    assert validators[0].soranet_transport_public_key == "peer-1-soranet-public"
    assert validators[-1].soranet_transport_private_key == "peer-4-soranet-private"
    assert validators[0].receipt_public_key == _receipt_keypair(1)[0]
    assert validators[-1].receipt_private_key == _receipt_keypair(4)[1]
    public_projection = MODULE.receipt_signer_map(validators)
    assert list(public_projection) == [
        "taira-validator-1",
        "taira-validator-2",
        "taira-validator-3",
        "taira-validator-4",
    ]
    assert "private" not in json.dumps(public_projection).lower()
    assert public_projection["taira-validator-1"]["node_id"] == (
        MODULE.receipt_node_id(_receipt_keypair(1)[0])
    )


@pytest.mark.parametrize("field", ["receipt_public_key", "receipt_private_key"])
def test_secret_material_requires_each_validator_receipt_key(
    tmp_path: Path,
    field: str,
) -> None:
    secrets_path = tmp_path / "validator_secrets.toml"
    _write_secrets(secrets_path)
    public_key, private_key = _receipt_keypair(2)
    value = public_key if field == "receipt_public_key" else private_key
    secrets_path.write_text(
        secrets_path.read_text(encoding="utf-8").replace(
            f'{field} = "{value}"\n',
            "",
            1,
        ),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match=field):
        MODULE.load_secret_material(secrets_path)


def test_secret_material_rejects_receipt_keypair_mismatch(tmp_path: Path) -> None:
    secrets_path = tmp_path / "validator_secrets.toml"
    _write_secrets(secrets_path)
    _, private_one = _receipt_keypair(1)
    _, private_two = _receipt_keypair(2)
    secrets_path.write_text(
        secrets_path.read_text(encoding="utf-8").replace(
            f'receipt_private_key = "{private_one}"',
            f'receipt_private_key = "{private_two}"',
            1,
        ),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="do not form one secp256k1 keypair"):
        MODULE.load_secret_material(secrets_path)


@pytest.mark.parametrize(
    ("field", "replacement", "expected"),
    [
        ("receipt_public_key", "bls12381g2" + "00" * 96, "compressed secp256k1"),
        ("receipt_private_key", "bls12381g1" + "00" * 48, "secp256k1 private"),
        ("receipt_private_key", "812620" + "00" * 32, "outside the secp256k1"),
        ("receipt_public_key", "e7012102" + "FF" * 32, "outside secp256k1"),
        ("receipt_public_key", "e7012102" + "00" * 32, "not a secp256k1"),
    ],
)
def test_secret_material_rejects_noncanonical_receipt_algorithms_and_points(
    tmp_path: Path,
    field: str,
    replacement: str,
    expected: str,
) -> None:
    secrets_path = tmp_path / "validator_secrets.toml"
    _write_secrets(secrets_path)
    public_key, private_key = _receipt_keypair(1)
    current = public_key if field == "receipt_public_key" else private_key
    secrets_path.write_text(
        secrets_path.read_text(encoding="utf-8").replace(
            f'{field} = "{current}"',
            f'{field} = "{replacement}"',
            1,
        ),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match=expected):
        MODULE.load_secret_material(secrets_path)


def test_secret_material_rejects_duplicate_receipt_signer(tmp_path: Path) -> None:
    secrets_path = tmp_path / "validator_secrets.toml"
    _write_secrets(secrets_path)
    public_one, private_one = _receipt_keypair(1)
    public_two, private_two = _receipt_keypair(2)
    text = secrets_path.read_text(encoding="utf-8")
    text = text.replace(public_two, public_one, 1).replace(private_two, private_one, 1)
    secrets_path.write_text(text, encoding="utf-8")

    with pytest.raises(ValueError, match="duplicates a Torii receipt"):
        MODULE.load_secret_material(secrets_path)


def test_load_roster_rejects_receipt_keys_in_public_roster(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    _write_roster(roster_path)
    public_key, private_key = _receipt_keypair(1)
    roster_path.write_text(
        roster_path.read_text(encoding="utf-8").replace(
            'private_key = "peer-1-private"',
            'private_key = "peer-1-private"\n'
            f'receipt_public_key = "{public_key}"\n'
            f'receipt_private_key = "{private_key}"',
            1,
        ),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="runtime-only --secrets file"):
        MODULE.load_roster(roster_path)


@pytest.mark.parametrize("field", ["receipt_public_key", "receipt_private_key"])
def test_render_bundle_rejects_receipt_keys_in_shared_template(
    tmp_path: Path,
    field: str,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    base_config_path = tmp_path / "config.toml"
    _write_roster(roster_path, inline_private_keys=False)
    _write_secrets(secrets_path)
    value = _receipt_keypair(1)[0 if field == "receipt_public_key" else 1]
    base_config_path.write_text(
        BASE_CONFIG.replace("[torii]\n", f'[torii]\n{field} = "{value}"\n', 1),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="receipt signer material"):
        MODULE.render_bundle(
            base_config_path,
            roster_path,
            tmp_path / "out",
            secrets_path=secrets_path,
        )


def test_secret_material_requires_each_validator_transport_pair(tmp_path: Path) -> None:
    secrets_path = tmp_path / "validator_secrets.toml"
    _write_secrets(secrets_path)
    secrets_path.write_text(
        secrets_path.read_text(encoding="utf-8").replace(
            'soranet_transport_private_key = "peer-2-soranet-private"\n',
            "",
            1,
        ),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="soranet_transport_private_key"):
        MODULE.load_secret_material(secrets_path)


def test_load_roster_rejects_duplicate_transport_public_keys(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    _write_roster(roster_path)
    roster_path.write_text(
        roster_path.read_text(encoding="utf-8").replace(
            'soranet_transport_public_key = "peer-2-soranet-public"',
            'soranet_transport_public_key = "peer-1-soranet-public"',
            1,
        ),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="soranet_transport_public_key.*duplicated"):
        MODULE.load_roster(roster_path)


def test_load_roster_rejects_streaming_identity_reuse(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    _write_roster(roster_path, inline_private_keys=False)
    _write_secrets(secrets_path)
    secrets_path.write_text(
        secrets_path.read_text(encoding="utf-8").replace(
            'streaming_identity_public_key = "streaming-public-key"',
            'streaming_identity_public_key = "peer-1-soranet-public"',
            1,
        ),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="must not reuse the shared streaming identity"):
        MODULE.load_roster(roster_path, secrets_path=secrets_path)


def test_render_bundle_rejects_unpopulated_template_placeholders(
    tmp_path: Path,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    base_config_path = tmp_path / "config.toml"
    output_dir = tmp_path / "out"
    _write_roster(roster_path)
    base_config_path.write_text(BASE_CONFIG, encoding="utf-8")

    try:
        MODULE.render_bundle(base_config_path, roster_path, output_dir)
    except ValueError as error:
        assert "runtime-only Torii receipt keypair" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError(
            "render_bundle accepted placeholder secrets without a secrets file"
        )


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
        'account_onboarding_api_token = "legacy-token"\n'
        'account_onboarding_credential_id = "legacy"\n'
        'account_onboarding_scope_dataspace = "is2"\n',
        encoding="utf-8",
    )
    with pytest.raises(
        ValueError,
        match=r"removed onboarding fields.*\[\[shared\.account_onboarding_credentials\]\]",
    ):
        MODULE.load_secret_material(secrets_path)

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
        assert "account_onboarding_credentials" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("incomplete structural onboarding secrets were accepted")

    secrets_path.write_text(
        '[shared]\ntorii_faucet_authority = "authority"\n',
        encoding="utf-8",
    )
    try:
        MODULE.load_secret_material(secrets_path)
    except ValueError as error:
        assert "must configure both torii_faucet_authority" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("incomplete faucet signer material was accepted")


def test_secret_material_rejects_removed_offline_enrollment_fields(
    tmp_path: Path,
) -> None:
    secrets_path = tmp_path / "validator_secrets.toml"
    _write_secrets(secrets_path)
    valid = secrets_path.read_text(encoding="utf-8")

    fields = {
        "offline_asset_alias": '"ds#boi.is"',
        "offline_asset_definition_id": f'"{TAIRA_GAS_ASSET_ID}"',
        "offline_asset_scale": "2",
        "offline_escrow_account": f'"{TAIRA_GENESIS_DEPLOYER_ID}"',
    }
    for index, (field, value) in enumerate(fields.items()):
        candidate = tmp_path / f"validator_secrets_{index}.toml"
        candidate.write_text(
            valid.replace("[shared]\n", f"[shared]\n{field} = {value}\n"),
            encoding="utf-8",
        )
        try:
            MODULE.load_secret_material(candidate)
        except ValueError as error:
            assert "removed offline enrollment fields" in str(error)
            assert field in str(error)
        else:  # pragma: no cover - defensive assertion
            raise AssertionError(f"renderer accepted removed offline field {field}")


def test_secret_material_requires_exact_provider_backed_soracloud_signer(
    tmp_path: Path,
) -> None:
    secrets_path = tmp_path / "validator_secrets.toml"
    _write_secrets(secrets_path)
    valid = secrets_path.read_text(encoding="utf-8")

    mutations = (
        (
            valid.replace(
                f'soracloud_runtime_signer_handle = "{SORACLOUD_SIGNER_HANDLE}"\n',
                "",
            ),
            "production Soracloud runtime signer is mandatory",
        ),
        (
            valid.replace(
                f'soracloud_runtime_signer_handle = "{SORACLOUD_SIGNER_HANDLE}"',
                'soracloud_runtime_signer_handle = "REPLACE_WITH_SIGNER"',
            ),
            "still contains placeholders",
        ),
        (
            valid.replace(SORACLOUD_SIGNER_HANDLE, "hsm://soracloud/dummy"),
            "credential-free production provider handle",
        ),
        (
            valid.replace(SORACLOUD_SIGNER_AUTHORITY, TAIRA_GENESIS_DEPLOYER_ID),
            "must be derived from soracloud_runtime_signer_public_key_hex",
        ),
        (
            valid.replace(
                SORACLOUD_SIGNER_PUBLIC_KEY_HEX,
                SORACLOUD_SIGNER_PUBLIC_KEY_HEX.upper(),
            ),
            "canonical lowercase hexadecimal",
        ),
        (
            valid.replace(
                "soracloud_runtime_signer_revision = 1",
                "soracloud_runtime_signer_revision = 0",
            ),
            "must be a positive integer",
        ),
        (
            valid.replace(SORACLOUD_SIGNER_POLICY_DIGEST_HEX, "00" * 32),
            "canonical nonzero 32-byte lowercase digest",
        ),
    )
    for index, (text, expected) in enumerate(mutations):
        candidate = tmp_path / f"soracloud_signer_{index}.toml"
        candidate.write_text(text, encoding="utf-8")
        with pytest.raises(ValueError, match=expected):
            MODULE.load_secret_material(candidate)


def test_secret_material_rejects_non_application_onboarding_scope(
    tmp_path: Path,
) -> None:
    secrets_path = tmp_path / "validator_secrets.toml"
    _write_secrets(secrets_path)
    text = secrets_path.read_text(encoding="utf-8").replace(
        'scope_dataspace = "is2"',
        'scope_dataspace = "universal"',
    )
    secrets_path.write_text(text, encoding="utf-8")

    with pytest.raises(ValueError, match="reviewed Taira application dataspaces"):
        MODULE.load_secret_material(secrets_path)


def test_secret_material_rejects_duplicate_onboarding_ids_and_tokens(
    tmp_path: Path,
) -> None:
    secrets_path = tmp_path / "validator_secrets.toml"
    _write_secrets(secrets_path)
    valid = secrets_path.read_text(encoding="utf-8")

    duplicate_id = tmp_path / "duplicate-id.toml"
    duplicate_id.write_text(
        valid.replace('id = "dpn-api"', 'id = "boi-mobile"'),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="credential id `boi-mobile` is duplicated"):
        MODULE.load_secret_material(duplicate_id)

    duplicate_token = tmp_path / "duplicate-token.toml"
    duplicate_token.write_text(
        valid.replace(DPN_ONBOARDING_TOKEN, ONBOARDING_TOKEN),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="must not reuse API tokens"):
        MODULE.load_secret_material(duplicate_token)


def test_secret_material_requires_distinct_boi_and_dpn_onboarding_credentials(
    tmp_path: Path,
) -> None:
    secrets_path = tmp_path / "validator_secrets.toml"
    _write_secrets(secrets_path)
    valid = secrets_path.read_text(encoding="utf-8")
    dpn_credential = (
        "[[shared.account_onboarding_credentials]]\n"
        'id = "dpn-api"\n'
        f'api_token = "{DPN_ONBOARDING_TOKEN}"\n'
        'scope_dataspace = "dpn"\n\n'
    )
    assert dpn_credential in valid
    secrets_path.write_text(valid.replace(dpn_credential, ""), encoding="utf-8")

    with pytest.raises(ValueError, match=r"required `dpn` and `is2`.*missing dpn"):
        MODULE.load_secret_material(secrets_path)


def test_secret_material_allows_additional_is_onboarding_credential(
    tmp_path: Path,
) -> None:
    secrets_path = tmp_path / "validator_secrets.toml"
    _write_secrets(secrets_path)
    text = secrets_path.read_text(encoding="utf-8")
    extra_credential = (
        "[[shared.account_onboarding_credentials]]\n"
        'id = "external-poc"\n'
        'api_token = "external-poc-token-0123456789abcdef012345"\n'
        'scope_dataspace = "is"\n\n'
    )
    secrets_path.write_text(
        text.replace("[[validators]]", extra_credential + "[[validators]]", 1),
        encoding="utf-8",
    )

    material = MODULE.load_secret_material(secrets_path)
    assert [
        credential.scope_dataspace
        for credential in material.shared.account_onboarding_credentials
    ] == ["is2", "dpn", "is"]


def test_render_bundle_rejects_onboarding_scope_missing_from_catalog(
    tmp_path: Path,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    base_config_path = tmp_path / "config.toml"
    output_dir = tmp_path / "out"
    _write_roster(roster_path)
    _write_secrets(secrets_path)
    base_config_path.write_text(
        BASE_CONFIG.replace(
            '[[nexus.dataspace_catalog]]\nalias = "dpn"\nid = 10\n\n',
            "",
        ),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="scopes are absent.*dpn"):
        MODULE.render_bundle(
            base_config_path,
            roster_path,
            output_dir,
            secrets_path=secrets_path,
        )
    assert not output_dir.exists()


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
            "--kagemusha-release-root",
            "/srv/iroha-kagemusha/taira",
        ]
    )

    assert exit_code == 0
    rendered_config = output_dir / "taira-validator-2" / "config.toml"
    assert rendered_config.exists()
    assert "kagemusha_catalog_qualification_seal_path" in rendered_config.read_text(
        encoding="utf-8"
    )
    assert not (output_dir / "taira-validator-1" / "config.toml").exists()
