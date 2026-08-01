"""Regression tests for canonical offline-enabled Taira reset preparation."""

from __future__ import annotations

import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import unittest
from unittest import mock

from scripts import prepare_taira_offline_reset_bundle as offline_reset


PRIVATE_KEY = "802620" + "A5" * 32
PREVIOUS_PRIVATE_KEY = "802620" + "B4" * 32
PUBLIC_KEY = (
    "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
)
EXPECTED_HASH = "00" * 31 + "01"
COMMAND_AUTHORITY = (
    "testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
)
NORITO_FLAGS = offline_reset.NORITO_COMPACT_LEN
# Captured from the release-mode Rust Norito encoder used by the Taira Kagami
# build on 2026-07-28.  This is deliberately not produced by the Python helpers
# below, so the decoder regression is anchored to the implementation it reads.
ACTUAL_NORITO_REGISTER_TRANSFER_VERIFIER = (
    "TlJUMAAAhip9dwddTSP/bBJh2wJ4EQBvAQAAAAAAABZ4qaRg/mTyAjw7aXJvaGFf"
    "ZGF0YV9tb2RlbDo6aXNpOjp2ZXJpZnlpbmdfa2V5czo6UmVnaXN0ZXJWZXJpZnlp"
    "bmdLZXmwAigBAAAAAAAATlJUMAAAYcE+cO3pqQus7y/PtkV0RgAAAQAAAAAAAKFo"
    "HCjzz3YkAjUKCWhhbG8yL2lwYSkoY29uZmlkZW50aWFsX3RyYW5zZmVyX3YyX3Zl"
    "cmlmaWVyX3JlY29yZMgBBAEAAABFRGhhbG8yL3Bhc3RhL2lwYS9jb25maWRlbnRp"
    "YWwtdHJhbnNmZXItMngyLW1lcmtsZTE2LWF4aW9tLXBvc2VpZG9uLXYzAQAIB29m"
    "ZmxpbmUEAAAAAAYFcGFzdGEgDQ0NDQ0NDQ0NDQ0NDQ0NDQ0NDQ0NDQ0NDQ0NDQ0N"
    "DQ0gBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUEAAAAAAQACAAAAQAB"
    "AAEACgEIAgAAAAAAAAABAAEABAEAAAA="
)


class TairaOfflineEntrypointTests(unittest.TestCase):
    """Exercise the hardened standalone command entrypoint."""

    def test_release_artifact_corridor_matches_compact_v5_runtime(self) -> None:
        self.assertEqual(
            offline_reset.KAGEMUSHA_ARTIFACT_MAX_BYTES,
            5 * 1024 * 1024 * 1024,
        )

    def test_help_runs_in_isolated_mode_outside_repository(self) -> None:
        script = (
            Path(__file__).resolve().parents[1]
            / "prepare_taira_offline_reset_bundle.py"
        )
        with tempfile.TemporaryDirectory() as temporary:
            result = subprocess.run(
                [sys.executable, "-I", str(script), "--help"],
                cwd=temporary,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
                text=True,
            )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("{prepare,check}", result.stdout)

    def test_release_move_is_an_explicit_prepare_opt_in(self) -> None:
        arguments = [
            "prepare",
            "--source-bundle",
            "/private/source",
            "--offline-genesis",
            "/private/genesis.json",
            "--release-bundle",
            "/private/release",
            "--operator-identity",
            "/private/operator.json",
            "--genesis-private-key-file",
            "/private/genesis.key",
            "--genesis-public-key",
            PUBLIC_KEY,
            "--command-authority",
            COMMAND_AUTHORITY,
            "--kagami",
            "/private/kagami",
            "--irohad",
            "/private/irohad",
            "--source-commit",
            "1" * 40,
            "--output-bundle",
            "/private/output",
        ]
        copied = offline_reset.parser().parse_args(arguments)
        moved = offline_reset.parser().parse_args(
            [*arguments, "--move-release-bundle"]
        )

        self.assertFalse(copied.move_release_bundle)
        self.assertTrue(moved.move_release_bundle)


def _norito_length(value: int) -> bytes:
    encoded = bytearray()
    while value >= 0x80:
        encoded.append((value & 0x7F) | 0x80)
        value >>= 7
    encoded.append(value)
    return bytes(encoded)


def _norito_field(payload: bytes) -> bytes:
    return _norito_length(len(payload)) + payload


def _norito_string(value: str) -> bytes:
    payload = value.encode()
    return _norito_length(len(payload)) + payload


def _norito_frame(payload: bytes) -> bytes:
    return b"".join(
        (
            b"NRT0",
            b"\0\0",
            b"\x5a" * 16,
            b"\0",
            len(payload).to_bytes(8, "little"),
            offline_reset.crc64_xz(payload).to_bytes(8, "little"),
            bytes([NORITO_FLAGS]),
            payload,
        )
    )


def _norito_instruction(name: str, payload: bytes) -> str:
    inner = _norito_frame(payload)
    pair = b"".join(
        (
            _norito_field(_norito_string(name)),
            _norito_field(len(inner).to_bytes(8, "little") + inner),
        )
    )
    return base64.b64encode(_norito_frame(pair)).decode()


def _norito_option_u64(value: int | None) -> bytes:
    if value is None:
        return b"\0"
    payload = value.to_bytes(8, "little")
    return b"\1" + _norito_length(len(payload)) + payload


def _verifier_id(verifier: dict[str, object]) -> bytes:
    return b"".join(
        (
            _norito_field(_norito_string(str(verifier["backend"]))),
            _norito_field(_norito_string(str(verifier["name"]))),
        )
    )


def _verifier_record(verifier: dict[str, object]) -> bytes:
    fields = (
        int(verifier["version"]).to_bytes(4, "little"),
        _norito_string(str(verifier["circuit_id"])),
        b"\0",
        _norito_string("offline"),
        b"\0",
        _norito_string("pasta"),
        bytes.fromhex(str(verifier["public_inputs_schema_hash"])),
        bytes.fromhex(str(verifier["commitment"])),
        (0).to_bytes(4, "little"),
        int(verifier["max_proof_bytes"]).to_bytes(4, "little"),
        b"\0",
        b"\0",
        b"\0",
        _norito_option_u64(int(verifier["activation_height"])),
        _norito_option_u64(
            None
            if verifier["withdrawal_height"] is None
            else int(verifier["withdrawal_height"])
        ),
        b"\0",
        b"\0",
    )
    return b"".join(_norito_field(field) for field in fields)


def _private_file(path: Path, payload: bytes) -> None:
    path.write_bytes(payload)
    path.chmod(0o600)


def _private_directory(path: Path) -> None:
    path.mkdir()
    path.chmod(0o700)


def _archived_config() -> str:
    return """\
chain = "809574f5-fee7-5e69-bfcf-52451e42d50f"
chain_discriminant = 369
public_key = "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
private_key = "802620AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"

[torii.mcp]
enabled = true

[settlement.offline]
enabled = false
escrow_required = true
escrow_accounts = {}
kagemusha_max_decoded_bytes = 268435456

[genesis]
public_key = "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
file = "/private/old/genesis.signed.nrt"
"""


class TairaOfflineConfigTests(unittest.TestCase):
    """Exercise exact runtime configuration projection."""

    def test_rotation_rejects_the_archived_key_in_any_runtime_projection(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            _private_directory(source)
            _private_file(
                source / "validator-secrets.toml",
                (
                    "[shared]\n"
                    f'kagemusha_commands_private_key = "{PREVIOUS_PRIVATE_KEY}"\n'
                ).encode(),
            )
            previous_sha256 = (
                offline_reset.source_command_private_key_sha256(source)
            )
            self.assertEqual(
                previous_sha256,
                hashlib.sha256(PREVIOUS_PRIVATE_KEY.encode()).digest(),
            )

            bundle = root / "bundle"
            _private_directory(bundle)
            _private_file(
                bundle / "base-config.toml",
                (
                    "[torii.kagemusha_commands]\n"
                    f'private_key = "{PRIVATE_KEY}"\n'
                ).encode(),
            )
            _private_file(
                bundle / "validator-secrets.toml",
                (
                    "[shared]\n"
                    f'kagemusha_commands_private_key = "{PRIVATE_KEY}"\n'
                ).encode(),
            )
            rendered = bundle / "rendered"
            _private_directory(rendered)
            for slug in offline_reset.VALIDATOR_SLUGS:
                validator = rendered / slug
                _private_directory(validator)
                _private_file(
                    validator / "config.toml",
                    (
                        "[torii.kagemusha_commands]\n"
                        f'private_key = "{PRIVATE_KEY}"\n'
                    ).encode(),
                )

            offline_reset.require_rotated_command_key_projection(
                bundle,
                command_private_key=PRIVATE_KEY,
                previous_private_key_sha256=previous_sha256,
            )
            _private_file(
                rendered
                / offline_reset.VALIDATOR_SLUGS[-1]
                / "config.toml",
                (
                    "[torii.kagemusha_commands]\n"
                    f'private_key = "{PREVIOUS_PRIVATE_KEY}"\n'
                ).encode(),
            )
            with self.assertRaisesRegex(
                RuntimeError, "stale or inconsistent"
            ):
                offline_reset.require_rotated_command_key_projection(
                    bundle,
                    command_private_key=PRIVATE_KEY,
                    previous_private_key_sha256=previous_sha256,
                )

    def test_rewrites_archived_config_into_self_contained_mandatory_offline(self) -> None:
        bundle = Path("/private/taira/bundle-v21-canonical-offline")
        rendered = offline_reset.runtime_config_text(
            _archived_config(),
            bundle=bundle,
            release_tree_sha256="ab" * 32,
            genesis_public_key=PUBLIC_KEY,
            genesis_expected_hash=EXPECTED_HASH,
            command_private_key=PRIVATE_KEY,
        )
        config = tomllib.loads(rendered)

        self.assertEqual(config["chain"], offline_reset.PUBLIC_TAIRA_CHAIN_ID)
        self.assertEqual(
            config["chain_discriminant"],
            offline_reset.PUBLIC_TAIRA_CHAIN_DISCRIMINANT,
        )
        commands = config["torii"]["kagemusha_commands"]
        self.assertTrue(commands["enabled"])
        self.assertEqual(commands["private_key"], PRIVATE_KEY)
        self.assertEqual(commands["minimum_xor_balance"], "1")
        self.assertEqual(commands["operation_registry_max_entries"], 4096)
        offline = config["settlement"]["offline"]
        self.assertTrue(offline["enabled"])
        self.assertTrue(offline["escrow_required"])
        self.assertEqual(
            offline["escrow_accounts"],
            {
                offline_reset.PUBLIC_TAIRA_OFFLINE_ASSET_ID:
                    offline_reset.PUBLIC_TAIRA_OFFLINE_ESCROW_ACCOUNT
            },
        )
        self.assertEqual(
            offline["kagemusha_release_policy_path"],
            str(
                offline_reset.TAIRA_RELEASE_INSTALL_ROOT
                / ("ab" * 32)
                / "release-policy-v1.norito"
            ),
        )
        self.assertEqual(
            offline["kagemusha_artifact_dir"],
            str(
                offline_reset.TAIRA_RELEASE_INSTALL_ROOT
                / ("ab" * 32)
                / "catalog"
            ),
        )
        self.assertEqual(
            offline["kagemusha_catalog_qualification_seal_path"],
            str(
                offline_reset.TAIRA_QUALIFICATION_SEAL_ROOT
                / f"kagemusha-v4-{'ab' * 32}.norito"
            ),
        )
        self.assertEqual(
            config["genesis"]["file"], str(bundle / "genesis.signed.nrt")
        )
        self.assertEqual(config["genesis"]["public_key"], PUBLIC_KEY)
        self.assertEqual(config["genesis"]["expected_hash"], EXPECTED_HASH)
        self.assertEqual(
            config["nexus"]["storage"]["local_budget_bytes"],
            offline_reset.PUBLIC_TAIRA_NODE_STORAGE_BUDGET_BYTES,
        )
        self.assertEqual(
            config["nexus"]["storage"]["disk_budget_weights"],
            offline_reset.PUBLIC_TAIRA_STORAGE_BUDGET_WEIGHTS,
        )
        offline_reset.validate_runtime_config(
            config,
            bundle,
            "ab" * 32,
            PUBLIC_KEY,
            EXPECTED_HASH,
        )
        with self.assertRaisesRegex(RuntimeError, "fresh public key"):
            offline_reset.validate_runtime_config(
                config,
                bundle,
                "ab" * 32,
                "ed0120" + "6B" * 32,
                EXPECTED_HASH,
            )
        staged = tomllib.loads(
            offline_reset.staged_check_config_text(
                rendered, bundle, "ab" * 32
            )
        )
        staged_offline = staged["settlement"]["offline"]
        self.assertEqual(
            staged_offline["kagemusha_release_policy_path"],
            str(bundle / "kagemusha/release-policy-v1.norito"),
        )
        self.assertEqual(
            staged_offline["kagemusha_artifact_dir"],
            str(bundle / "kagemusha/catalog"),
        )
        self.assertNotIn(
            "kagemusha_catalog_qualification_seal_path",
            staged_offline,
        )
        self.assertEqual(
            config["settlement"]["offline"]["kagemusha_artifact_dir"],
            str(
                offline_reset.TAIRA_RELEASE_INSTALL_ROOT
                / ("ab" * 32)
                / "catalog"
            ),
        )

    def test_runtime_validation_rejects_a_stale_qualification_seal_hash(
        self,
    ) -> None:
        bundle = Path("/private/taira/bundle-v21-canonical-offline")
        config = tomllib.loads(
            offline_reset.runtime_config_text(
                _archived_config(),
                bundle=bundle,
                release_tree_sha256="ab" * 32,
                genesis_public_key=PUBLIC_KEY,
                genesis_expected_hash=EXPECTED_HASH,
                command_private_key=PRIVATE_KEY,
            )
        )
        config["settlement"]["offline"][
            "kagemusha_catalog_qualification_seal_path"
        ] = str(
            offline_reset.TAIRA_QUALIFICATION_SEAL_ROOT
            / f"kagemusha-v4-{'cd' * 32}.norito"
        )

        with self.assertRaisesRegex(
            RuntimeError,
            "mandatory offline settlement projection",
        ):
            offline_reset.validate_runtime_config(
                config,
                bundle,
                "ab" * 32,
                PUBLIC_KEY,
                EXPECTED_HASH,
            )

    def test_pre_signing_placeholder_binds_once_to_the_exact_signed_hash(
        self,
    ) -> None:
        bundle = Path("/private/taira/bundle-v21-canonical-offline")
        staged = offline_reset.runtime_config_text(
            _archived_config(),
            bundle=bundle,
            release_tree_sha256="ab" * 32,
            genesis_public_key=PUBLIC_KEY,
            genesis_expected_hash=(
                offline_reset.GENESIS_EXPECTED_HASH_PLACEHOLDER
            ),
            command_private_key=PRIVATE_KEY,
        )
        self.assertEqual(
            tomllib.loads(staged)["genesis"]["expected_hash"],
            offline_reset.GENESIS_EXPECTED_HASH_PLACEHOLDER,
        )

        bound = offline_reset.bind_runtime_genesis_expected_hash(
            staged, EXPECTED_HASH
        )
        self.assertEqual(
            tomllib.loads(bound)["genesis"]["expected_hash"], EXPECTED_HASH
        )
        with self.assertRaisesRegex(RuntimeError, "marker bit"):
            offline_reset.bind_runtime_genesis_expected_hash(
                staged, "00" * 32
            )

    def test_reads_only_one_canonical_kagami_expected_hash_line(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "genesis.expected_hash"
            _private_file(path, f"{EXPECTED_HASH}\n".encode("ascii"))
            self.assertEqual(
                offline_reset.read_genesis_expected_hash(path), EXPECTED_HASH
            )
            _private_file(path, EXPECTED_HASH.encode("ascii"))
            with self.assertRaisesRegex(RuntimeError, "one canonical line"):
                offline_reset.read_genesis_expected_hash(path)

    def test_replaces_existing_storage_budget_without_losing_other_settings(
        self,
    ) -> None:
        source = (
            _archived_config()
            + "\n[nexus.storage]\n"
            + "local_budget_bytes = 123\n"
            + "budget_enforce_interval_blocks = 9\n"
        )
        rendered = offline_reset.runtime_config_text(
            source,
            bundle=Path("/private/v21"),
            release_tree_sha256="ab" * 32,
            genesis_public_key=PUBLIC_KEY,
            genesis_expected_hash=EXPECTED_HASH,
            command_private_key=PRIVATE_KEY,
        )
        storage = tomllib.loads(rendered)["nexus"]["storage"]
        self.assertEqual(
            storage["local_budget_bytes"],
            offline_reset.PUBLIC_TAIRA_NODE_STORAGE_BUDGET_BYTES,
        )
        self.assertEqual(storage["budget_enforce_interval_blocks"], 9)
        self.assertEqual(
            storage["disk_budget_weights"],
            offline_reset.PUBLIC_TAIRA_STORAGE_BUDGET_WEIGHTS,
        )

    def test_rejects_duplicate_offline_sections(self) -> None:
        source = _archived_config() + "\n[settlement.offline]\nenabled = false\n"
        with self.assertRaisesRegex(RuntimeError, "duplicate|non-unique"):
            offline_reset.runtime_config_text(
                source,
                bundle=Path("/private/v21"),
                release_tree_sha256="ab" * 32,
                genesis_public_key=PUBLIC_KEY,
                genesis_expected_hash=EXPECTED_HASH,
                command_private_key=PRIVATE_KEY,
            )

    def test_runtime_secret_projection_updates_only_required_shared_fields(self) -> None:
        source = """\
[shared]
kagemusha_commands_private_key = "802620AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
offline_asset_alias = "sbd#cbsi"
offline_asset_definition_id = "old"
offline_asset_scale = 9
offline_escrow_account = "old-account"
unrelated = "preserved"

[[validators]]
slug = "taira-validator-1"
private_key = "802620BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"
"""
        rendered = offline_reset.patch_runtime_secrets(
            source, command_private_key=PRIVATE_KEY
        )
        secrets = tomllib.loads(rendered)
        shared = secrets["shared"]
        self.assertEqual(shared["kagemusha_commands_private_key"], PRIVATE_KEY)
        self.assertEqual(
            shared["offline_asset_alias"],
            offline_reset.PUBLIC_TAIRA_OFFLINE_ASSET_ALIAS,
        )
        self.assertEqual(
            shared["offline_escrow_account"],
            offline_reset.PUBLIC_TAIRA_OFFLINE_ESCROW_ACCOUNT,
        )
        self.assertEqual(shared["unrelated"], "preserved")
        self.assertEqual(
            secrets["validators"][0]["private_key"],
            "802620" + "B" * 64,
        )


class TairaOfflineGenesisTests(unittest.TestCase):
    """Exercise canonical chain and slow-genesis timing admission."""

    def test_command_authority_is_bound_to_the_fresh_genesis_key(self) -> None:
        self.assertEqual(
            offline_reset.command_authority_for_genesis_public_key(PUBLIC_KEY),
            COMMAND_AUTHORITY,
        )
        self.assertEqual(
            offline_reset.require_command_authority(
                COMMAND_AUTHORITY,
                genesis_public_key=PUBLIC_KEY,
            ),
            COMMAND_AUTHORITY,
        )
        with self.assertRaisesRegex(RuntimeError, "fresh genesis public key"):
            offline_reset.require_command_authority(
                COMMAND_AUTHORITY + "1",
                genesis_public_key=PUBLIC_KEY,
            )

    def test_norito_crc64_xz_matches_standard_check_value(self) -> None:
        self.assertEqual(
            offline_reset.crc64_xz(b"123456789"),
            0x995DC9BBDF1939FA,
        )

    def test_decodes_actual_kagami_norito_instruction_fixture(self) -> None:
        name, payload, flags = offline_reset.decode_genesis_instruction(
            ACTUAL_NORITO_REGISTER_TRANSFER_VERIFIER,
            "actual Kagami/Norito fixture",
        )
        self.assertEqual(
            name,
            (
                "iroha_data_model::isi::verifying_keys::"
                "RegisterVerifyingKey"
            ),
        )
        identifier_payload, record_payload = (
            offline_reset.norito_struct_fields(
                payload, 2, flags, "actual verifier registration"
            )
        )
        identifier = offline_reset.decode_verifier_id(
            identifier_payload, flags, "actual verifier id"
        )
        record = offline_reset.decode_verifier_record(
            identifier, record_payload, flags, "actual verifier record"
        )
        self.assertEqual(
            identifier,
            (
                "halo2/ipa",
                "confidential_transfer_v2_verifier_record",
            ),
        )
        self.assertEqual(record["commitment"], "05" * 32)
        self.assertEqual(record["public_inputs_schema_hash"], "0d" * 32)
        self.assertEqual(record["activation_height"], 2)

    def test_rejects_tampered_norito_header_checksum_and_payload(self) -> None:
        original = base64.b64decode(
            ACTUAL_NORITO_REGISTER_TRANSFER_VERIFIER,
            validate=True,
        )
        mutations = {
            "header": (0, original[0] ^ 0x01, "Norito V1 frame"),
            "checksum": (31, original[31] ^ 0x01, "checksum"),
            "payload": (-1, original[-1] ^ 0x01, "checksum"),
        }
        for label, (offset, value, expected_error) in mutations.items():
            with self.subTest(label=label):
                tampered = bytearray(original)
                tampered[offset] = value
                encoded = base64.b64encode(tampered).decode()
                with self.assertRaisesRegex(RuntimeError, expected_error):
                    offline_reset.decode_genesis_instruction(
                        encoded, f"tampered {label}"
                    )

    def _genesis(self, cadence: int) -> dict[str, object]:
        markers = [
            {name: {}}
            for name in (
                "ActivateKagemushaRecursiveReleaseV4",
                "RegisterVerifyingKey",
                "RegisterZkAsset",
                "CanManageOfflineEscrow",
                "CanActivateKagemushaRecursiveReleaseV4",
                "CanManageOfflineDeviceAttestationPolicy",
            )
        ]
        return {
            "chain": offline_reset.PUBLIC_TAIRA_CHAIN_ID,
            "chain_discriminant": offline_reset.PUBLIC_TAIRA_CHAIN_DISCRIMINANT,
            "transactions": [
                {
                    "parameters": {
                        "sumeragi": {"block_cadence_ms": cadence},
                    },
                    "instructions": [
                        {
                            "Register": {
                                "AssetDefinition": {
                                    "id":
                                        offline_reset.PUBLIC_TAIRA_OFFLINE_ASSET_ID,
                                    "logo": None,
                                    "metadata":
                                        offline_reset.PUBLIC_TAIRA_OFFLINE_ASSET_METADATA,
                                    "mintable": "Infinitely",
                                    "spec": {
                                        "scale":
                                            offline_reset.PUBLIC_TAIRA_OFFLINE_ASSET_SCALE,
                                    },
                                    "name":
                                        offline_reset.PUBLIC_TAIRA_OFFLINE_ASSET_NAME,
                                }
                            }
                        },
                        {
                            "Mint": {
                                "Asset": {
                                    "destination": (
                                        offline_reset.PUBLIC_TAIRA_OFFLINE_ASSET_ID
                                        + "#testu-non-escrow"
                                    ),
                                    "object": "1000000000.00",
                                }
                            }
                        },
                        {
                            "Mint": {
                                "Asset": {
                                    "destination": (
                                        offline_reset.PUBLIC_TAIRA_FEE_ASSET_ID
                                        + "#"
                                        + COMMAND_AUTHORITY
                                    ),
                                    "object": "1000000",
                                }
                            }
                        },
                        {
                            "SetAssetDefinitionAlias": {
                                "alias": offline_reset.PUBLIC_TAIRA_OFFLINE_ASSET_ALIAS,
                                "asset_definition_id":
                                    offline_reset.PUBLIC_TAIRA_OFFLINE_ASSET_ID,
                                "lease_expiry_ms": None,
                            }
                        },
                        {"SetKeyValue": {"key": "offline.enabled"}},
                        *markers,
                    ],
                }
            ],
        }

    def _release_binding(self) -> dict[str, object]:
        manifest_payload = b"canonical manifest payload"
        attestation_payload = b"canonical release attestation payload"
        benchmark_evidence = b"physical evidence summary"
        cryptographic_review = b"cryptographic review summary"
        promotion_payload = b"canonical promotion record payload"
        manifest = {
            "chain_id": offline_reset.PUBLIC_TAIRA_CHAIN_ID,
            "asset": offline_reset.PUBLIC_TAIRA_OFFLINE_ASSET_ID,
            "asset_scale": offline_reset.PUBLIC_TAIRA_OFFLINE_ASSET_SCALE,
            "activation_height":
                offline_reset.PUBLIC_TAIRA_RELEASE_ACTIVATION_HEIGHT,
            "withdrawal_height": 1_000_000_000,
            "max_proof_bytes": 4096,
            "generation": offline_reset.KAGEMUSHA_RELEASE_GENERATION,
            "bridge_abi_version": offline_reset.KAGEMUSHA_BRIDGE_ABI_VERSION,
        }
        return {
            "manifest": manifest,
            "manifest_sha256": "11" * 32,
            "manifest_payload_sha256":
                hashlib.sha256(manifest_payload).hexdigest(),
            "release_policy_sha256": "22" * 32,
            "release_attestation_sha256": "33" * 32,
            "release_attestation_payload_sha256":
                hashlib.sha256(attestation_payload).hexdigest(),
            "benchmark_evidence_sha256":
                hashlib.sha256(benchmark_evidence).hexdigest(),
            "cryptographic_review_sha256":
                hashlib.sha256(cryptographic_review).hexdigest(),
            "promotion_record_sha256": "44" * 32,
            "promotion_record_payload_sha256":
                hashlib.sha256(promotion_payload).hexdigest(),
            "_manifest_payload": manifest_payload,
            "_release_attestation_payload": attestation_payload,
            "_benchmark_evidence": benchmark_evidence,
            "_cryptographic_review": cryptographic_review,
            "_promotion_record_payload": promotion_payload,
        }

    def _operator_identity(
        self, release: dict[str, object]
    ) -> dict[str, object]:
        manifest = release["manifest"]
        self.assertIsInstance(manifest, dict)
        verifiers = {}
        for index, (field, role) in enumerate(
            offline_reset.KAGEMUSHA_VERIFIER_ROLES.items(), start=5
        ):
            recursive = field.startswith("active_recursive_")
            verifiers[field] = {
                "backend": role[0],
                "name": role[1],
                "version": 1,
                "circuit_id": role[2],
                "commitment": f"{index:02x}" * 32,
                "public_inputs_schema_hash": f"{index + 8:02x}" * 32,
                "max_proof_bytes":
                    manifest["max_proof_bytes"] if recursive else 2048,
                "activation_height":
                    offline_reset.PUBLIC_TAIRA_RELEASE_ACTIVATION_HEIGHT,
                "withdrawal_height":
                    manifest["withdrawal_height"] if recursive else None,
            }
        return {
            "cash_handoff_capability": "cash_handoff_v1",
            "required_bridge_abi_version":
                offline_reset.KAGEMUSHA_BRIDGE_ABI_VERSION,
            "max_hops": offline_reset.KAGEMUSHA_MAX_HOPS,
            "asset_definition_id":
                offline_reset.PUBLIC_TAIRA_OFFLINE_ASSET_ID,
            "asset_scale": offline_reset.PUBLIC_TAIRA_OFFLINE_ASSET_SCALE,
            "artifact_set": {
                "generation": offline_reset.KAGEMUSHA_RELEASE_GENERATION,
                "manifest_sha256": release["manifest_sha256"],
                "release_policy_sha256":
                    release["release_policy_sha256"],
                "release_attestation_sha256":
                    release["release_attestation_sha256"],
                "activation_height":
                    offline_reset.PUBLIC_TAIRA_RELEASE_ACTIVATION_HEIGHT,
                "withdrawal_height": manifest["withdrawal_height"],
                "max_proof_bytes": manifest["max_proof_bytes"],
                "asset_scale":
                    offline_reset.PUBLIC_TAIRA_OFFLINE_ASSET_SCALE,
            },
            "verifiers": verifiers,
        }

    def _bind_genesis(
        self,
        genesis: dict[str, object],
        release: dict[str, object],
        operator_identity: dict[str, object],
        *,
        promotion_payload: bytes | None = None,
    ) -> None:
        instructions = genesis["transactions"][0]["instructions"]
        instructions[:] = [
            instruction
            for instruction in instructions
            if "ActivateKagemushaRecursiveReleaseV4" not in instruction
            and "RegisterVerifyingKey" not in instruction
        ]
        verifiers = operator_identity["verifiers"]

        for field in tuple(offline_reset.KAGEMUSHA_VERIFIER_ROLES)[:3]:
            verifier = verifiers[field]
            if field == "active_transfer_verifier":
                instructions.append(
                    ACTUAL_NORITO_REGISTER_TRANSFER_VERIFIER
                )
                continue
            instructions.append(
                _norito_instruction(
                    (
                        "iroha_data_model::isi::verifying_keys::"
                        "RegisterVerifyingKey"
                    ),
                    b"".join(
                        (
                            _norito_field(_verifier_id(verifier)),
                            _norito_field(_verifier_record(verifier)),
                        )
                    ),
                )
            )
        release_record = b"".join(
            _norito_field(field)
            for field in (
                release["_manifest_payload"],
                release["_release_attestation_payload"],
                (
                    len(release["_benchmark_evidence"]).to_bytes(8, "little")
                    + release["_benchmark_evidence"]
                ),
                (
                    len(release["_cryptographic_review"]).to_bytes(8, "little")
                    + release["_cryptographic_review"]
                ),
                (
                    release["_promotion_record_payload"]
                    if promotion_payload is None
                    else promotion_payload
                ),
            )
        )
        eq_verifier = verifiers["active_recursive_step_eq_verifier"]
        ep_verifier = verifiers["active_recursive_step_ep_verifier"]
        activation = b"".join(
            _norito_field(field)
            for field in (
                release_record,
                bytes.fromhex(str(release["release_policy_sha256"])),
                _verifier_id(eq_verifier),
                _verifier_record(eq_verifier),
                _verifier_id(ep_verifier),
                _verifier_record(ep_verifier),
            )
        )
        instructions.append(
            _norito_instruction(
                (
                    "iroha_data_model::isi::offline::"
                    "ActivateKagemushaRecursiveReleaseV4"
                ),
                b"".join(
                    (
                        _norito_field(activation),
                        _norito_field(b"device attestation policy"),
                    )
                ),
            )
        )

    def test_accepts_complete_four_second_genesis(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "genesis.json"
            _private_file(
                path,
                json.dumps(self._genesis(4_000), ensure_ascii=False).encode(),
            )
            summary = offline_reset.genesis_summary(
                path,
                command_authority=COMMAND_AUTHORITY,
                genesis_public_key=PUBLIC_KEY,
            )
        self.assertEqual(summary["block_cadence_ms"], 4_000)

    def test_rejects_old_one_second_genesis(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "genesis.json"
            _private_file(
                path,
                json.dumps(self._genesis(1_000), ensure_ascii=False).encode(),
            )
            with self.assertRaisesRegex(RuntimeError, "block_cadence_ms=4000"):
                offline_reset.genesis_summary(
                    path,
                    command_authority=COMMAND_AUTHORITY,
                    genesis_public_key=PUBLIC_KEY,
                )

    def test_rejects_legacy_sbd_projection(self) -> None:
        genesis = self._genesis(4_000)
        registration = genesis["transactions"][0]["instructions"][0]["Register"][
            "AssetDefinition"
        ]
        registration["name"] = "sbd"
        registration["metadata"] = {
            "currency_code": "SBD",
            "display_code": "e-SBD",
            "display_name": "Digital Solomon Islands Dollar",
        }
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "genesis.json"
            _private_file(path, json.dumps(genesis, ensure_ascii=False).encode())
            with self.assertRaisesRegex(RuntimeError, "Digital Shekel"):
                offline_reset.genesis_summary(
                    path,
                    command_authority=COMMAND_AUTHORITY,
                    genesis_public_key=PUBLIC_KEY,
                )

    def test_rejects_duplicate_or_wrong_ds_alias_binding(self) -> None:
        genesis = self._genesis(4_000)
        instructions = genesis["transactions"][0]["instructions"]
        instructions.append(
            {
                "SetAssetDefinitionAlias": {
                    "alias": offline_reset.PUBLIC_TAIRA_OFFLINE_ASSET_ALIAS,
                    "asset_definition_id": "wrong-asset",
                    "lease_expiry_ms": None,
                }
            }
        )
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "genesis.json"
            _private_file(path, json.dumps(genesis, ensure_ascii=False).encode())
            with self.assertRaisesRegex(RuntimeError, "exactly one ds#boi.is"):
                offline_reset.genesis_summary(
                    path,
                    command_authority=COMMAND_AUTHORITY,
                    genesis_public_key=PUBLIC_KEY,
                )

    def test_rejects_explicit_implicit_genesis_signer_registration(self) -> None:
        genesis = self._genesis(4_000)
        genesis["transactions"][0]["instructions"].append(
            {
                "Register": {
                    "Account": {
                        "id": COMMAND_AUTHORITY,
                        "metadata": {},
                    }
                }
            }
        )
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "genesis.json"
            _private_file(path, json.dumps(genesis, ensure_ascii=False).encode())
            with self.assertRaisesRegex(RuntimeError, "must not explicitly register"):
                offline_reset.genesis_summary(
                    path,
                    command_authority=COMMAND_AUTHORITY,
                    genesis_public_key=PUBLIC_KEY,
                )

    def test_rejects_unfunded_pinned_command_authority(self) -> None:
        genesis = self._genesis(4_000)
        genesis["transactions"][0]["instructions"][2]["Mint"]["Asset"][
            "destination"
        ] = (
            offline_reset.PUBLIC_TAIRA_FEE_ASSET_ID
            + "#"
            + offline_reset.PUBLIC_TAIRA_OFFLINE_ESCROW_ACCOUNT
        )
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "genesis.json"
            _private_file(path, json.dumps(genesis, ensure_ascii=False).encode())
            with self.assertRaisesRegex(
                RuntimeError, "explicitly pinned command authority"
            ):
                offline_reset.genesis_summary(
                    path,
                    command_authority=COMMAND_AUTHORITY,
                    genesis_public_key=PUBLIC_KEY,
                )

    def test_rejects_missing_online_backing_liquidity(self) -> None:
        genesis = self._genesis(4_000)
        genesis["transactions"][0]["instructions"][1]["Mint"]["Asset"][
            "object"
        ] = "0"
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "genesis.json"
            _private_file(path, json.dumps(genesis, ensure_ascii=False).encode())
            with self.assertRaisesRegex(RuntimeError, "non-zero non-escrow"):
                offline_reset.genesis_summary(
                    path,
                    command_authority=COMMAND_AUTHORITY,
                    genesis_public_key=PUBLIC_KEY,
                )

    def test_cross_binds_catalog_operator_identity_and_genesis_activation(
        self,
    ) -> None:
        release = self._release_binding()
        identity = self._operator_identity(release)
        validated_identity = offline_reset.operator_identity_binding(
            json.dumps(identity).encode(), release
        )
        genesis = self._genesis(4_000)
        self._bind_genesis(genesis, release, validated_identity)
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "genesis.json"
            _private_file(
                path, json.dumps(genesis, ensure_ascii=False).encode()
            )
            summary = offline_reset.genesis_summary(
                path,
                command_authority=COMMAND_AUTHORITY,
                genesis_public_key=PUBLIC_KEY,
                release=release,
                operator_identity=validated_identity,
            )
        self.assertTrue(summary["online_backing_source_ready"])

    def test_rejects_operator_identity_with_wrong_catalog_digest(self) -> None:
        release = self._release_binding()
        identity = self._operator_identity(release)
        identity["artifact_set"]["manifest_sha256"] = "aa" * 32
        with self.assertRaisesRegex(RuntimeError, "exact release catalog"):
            offline_reset.operator_identity_binding(
                json.dumps(identity).encode(), release
            )

    def test_rejects_genesis_verifier_different_from_operator_identity(
        self,
    ) -> None:
        release = self._release_binding()
        genesis_identity = self._operator_identity(release)
        genesis = self._genesis(4_000)
        self._bind_genesis(genesis, release, genesis_identity)
        reviewed_identity = json.loads(json.dumps(genesis_identity))
        reviewed_identity["verifiers"][
            "active_recursive_step_eq_verifier"
        ]["commitment"] = "aa" * 32
        reviewed_identity = offline_reset.operator_identity_binding(
            json.dumps(reviewed_identity).encode(), release
        )
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "genesis.json"
            _private_file(
                path, json.dumps(genesis, ensure_ascii=False).encode()
            )
            with self.assertRaisesRegex(RuntimeError, "recursive eq verifier"):
                offline_reset.genesis_summary(
                    path,
                    command_authority=COMMAND_AUTHORITY,
                    genesis_public_key=PUBLIC_KEY,
                    release=release,
                    operator_identity=reviewed_identity,
                )

    def test_rejects_genesis_activation_with_wrong_promotion_digest(self) -> None:
        release = self._release_binding()
        identity = self._operator_identity(release)
        genesis = self._genesis(4_000)
        self._bind_genesis(
            genesis,
            release,
            identity,
            promotion_payload=b"wrong promotion record payload",
        )
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "genesis.json"
            _private_file(
                path, json.dumps(genesis, ensure_ascii=False).encode()
            )
            with self.assertRaisesRegex(RuntimeError, "release record"):
                offline_reset.genesis_summary(
                    path,
                    command_authority=COMMAND_AUTHORITY,
                    genesis_public_key=PUBLIC_KEY,
                    release=release,
                    operator_identity=identity,
                )

    def test_rejects_genesis_activation_catalog_manifest_mismatch(self) -> None:
        release = self._release_binding()
        identity = self._operator_identity(release)
        genesis = self._genesis(4_000)
        self._bind_genesis(genesis, release, identity)
        different_catalog = dict(release)
        different_catalog["manifest_payload_sha256"] = hashlib.sha256(
            b"different catalog manifest payload"
        ).hexdigest()
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "genesis.json"
            _private_file(
                path, json.dumps(genesis, ensure_ascii=False).encode()
            )
            with self.assertRaisesRegex(RuntimeError, "release record"):
                offline_reset.genesis_summary(
                    path,
                    command_authority=COMMAND_AUTHORITY,
                    genesis_public_key=PUBLIC_KEY,
                    release=different_catalog,
                    operator_identity=identity,
                )


class TairaOfflineCatalogTests(unittest.TestCase):
    """Exercise exact private single-link catalog copying."""

    def _release_bundle(self, root: Path) -> tuple[Path, Path]:
        root.chmod(0o700)
        _private_file(root / "release-policy-v1.norito", b"policy")
        catalog = root / "catalog"
        _private_directory(catalog)
        manifest_norito = b"canonical manifest fixture"
        digest = hashlib.sha256(manifest_norito).hexdigest()
        release = catalog / digest
        _private_directory(release)
        manifest = {
            "chain_id": offline_reset.PUBLIC_TAIRA_CHAIN_ID,
            "asset": offline_reset.PUBLIC_TAIRA_OFFLINE_ASSET_ID,
            "asset_scale": offline_reset.PUBLIC_TAIRA_OFFLINE_ASSET_SCALE,
            "activation_height":
                offline_reset.PUBLIC_TAIRA_RELEASE_ACTIVATION_HEIGHT,
        }
        _private_file(
            release / "manifest.json",
            json.dumps(manifest).encode(),
        )
        _private_file(release / "manifest.norito", manifest_norito)
        _private_file(release / "manifest.norito.sha256", f"{digest}\n".encode())
        for index in range(13):
            _private_file(release / f"artifact-{index:02}.bin", bytes([index]))
        return release, root

    def _move_plan(
        self, temporary_path: Path
    ) -> tuple[
        offline_reset.ReleaseBundleMove,
        Path,
        Path,
        str,
    ]:
        source = temporary_path / "source-release"
        _private_directory(source)
        release, source = self._release_bundle(source)
        output = temporary_path / "output-bundle"
        binding = {"manifest_sha256": release.name}
        with mock.patch.object(
            offline_reset,
            "release_bundle_binding",
            return_value=binding,
        ):
            plan = offline_reset.ReleaseBundleMove.preflight(source, output)
        return plan, source, output, release.name

    def _prepare_arguments(
        self,
        temporary_path: Path,
        source: Path,
        output: Path,
        *,
        move: bool,
    ) -> object:
        arguments = [
            "prepare",
            "--source-bundle",
            str(temporary_path / "sealed-source"),
            "--offline-genesis",
            str(temporary_path / "offline-genesis.json"),
            "--release-bundle",
            str(source),
            "--operator-identity",
            str(temporary_path / "operator.json"),
            "--genesis-private-key-file",
            str(temporary_path / "genesis.key"),
            "--genesis-public-key",
            PUBLIC_KEY,
            "--command-authority",
            COMMAND_AUTHORITY,
            "--kagami",
            str(temporary_path / "kagami"),
            "--irohad",
            str(temporary_path / "irohad"),
            "--source-commit",
            "1" * 40,
            "--output-bundle",
            str(output),
            "--minimum-free-bytes",
            "0",
        ]
        if move:
            arguments.append("--move-release-bundle")
        return offline_reset.parser().parse_args(arguments)

    def _empty_reset_skeleton(self, output: Path):
        def materialize(
            _command: list[str], *, cwd: Path | None = None
        ) -> None:
            self.assertIsNone(cwd)
            _private_directory(output)

        return materialize

    def test_prepare_rejects_reused_command_key_before_materialization(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            temporary_path = Path(temporary).resolve()
            arguments = self._prepare_arguments(
                temporary_path,
                temporary_path / "release",
                temporary_path / "output",
                move=False,
            )
            reused_sha256 = hashlib.sha256(PRIVATE_KEY.encode()).digest()
            with (
                mock.patch.object(
                    offline_reset,
                    "require_private_key",
                    return_value=PRIVATE_KEY,
                ),
                mock.patch.object(
                    offline_reset,
                    "source_command_private_key_sha256",
                    return_value=reused_sha256,
                ),
                mock.patch.object(offline_reset, "run_checked") as materialize,
                self.assertRaisesRegex(RuntimeError, "rotate the archived"),
            ):
                offline_reset.prepare(arguments)

            materialize.assert_not_called()

    def test_moves_release_once_and_fsyncs_both_parents(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            temporary_path = Path(temporary).resolve()
            plan, source, output, digest = self._move_plan(temporary_path)
            source_inode = source.lstat().st_ino
            _private_directory(output)

            with mock.patch.object(
                offline_reset,
                "_fsync_release_move_directory",
                wraps=offline_reset._fsync_release_move_directory,
            ) as fsync_directory:
                actual_digest = plan.move_into_output()

            self.assertEqual(actual_digest, digest)
            self.assertFalse(source.exists())
            self.assertEqual(
                (output / "kagemusha").lstat().st_ino,
                source_inode,
            )
            self.assertEqual(
                fsync_directory.call_args_list,
                [mock.call(source.parent), mock.call(output)],
            )

    def test_default_prepare_path_still_copies_release(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            temporary_path = Path(temporary).resolve()
            source = temporary_path / "release"
            output = temporary_path / "output"
            arguments = self._prepare_arguments(
                temporary_path, source, output, move=False
            )
            digest = "ab" * 32
            binding = {"manifest_sha256": digest}

            with (
                mock.patch.object(
                    offline_reset,
                    "require_private_key",
                    return_value=PRIVATE_KEY,
                ),
                mock.patch.object(
                    offline_reset,
                    "source_command_private_key_sha256",
                    return_value=hashlib.sha256(
                        PREVIOUS_PRIVATE_KEY.encode()
                    ).digest(),
                ),
                mock.patch.object(
                    offline_reset,
                    "require_private_file",
                    return_value=b"operator identity",
                ),
                mock.patch.object(
                    offline_reset, "genesis_summary", return_value={}
                ),
                mock.patch.object(offline_reset, "require_regular_file"),
                mock.patch.object(
                    offline_reset, "sha256", return_value="cd" * 32
                ),
                mock.patch.object(
                    offline_reset,
                    "run_checked",
                    side_effect=self._empty_reset_skeleton(output),
                ),
                mock.patch.object(
                    offline_reset,
                    "copy_release_bundle",
                    return_value=digest,
                ) as copy_release,
                mock.patch.object(
                    offline_reset,
                    "release_bundle_binding",
                    return_value=binding,
                ),
                mock.patch.object(
                    offline_reset,
                    "operator_identity_binding",
                    side_effect=RuntimeError("stop after copy selection"),
                ),
                mock.patch.object(
                    offline_reset.ReleaseBundleMove,
                    "preflight",
                ) as move_preflight,
            ):
                with self.assertRaisesRegex(RuntimeError, "copy selection"):
                    offline_reset.prepare(arguments)

            copy_release.assert_called_once_with(
                source, output / "kagemusha"
            )
            move_preflight.assert_not_called()
            self.assertFalse(output.exists())

    def test_move_preflight_rejects_cross_device_source(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            temporary_path = Path(temporary).resolve()
            source = temporary_path / "source-release"
            _private_directory(source)
            self._release_bundle(source)
            output = temporary_path / "output-bundle"
            original_metadata = offline_reset._release_move_directory_metadata

            def cross_device_metadata(
                path: Path, *, writable: bool = False
            ) -> object:
                metadata = original_metadata(path, writable=writable)
                if path == source:
                    return mock.Mock(
                        st_dev=metadata.st_dev + 1,
                        st_ino=metadata.st_ino,
                    )
                return metadata

            with (
                mock.patch.object(
                    offline_reset,
                    "_release_move_directory_metadata",
                    side_effect=cross_device_metadata,
                ),
                self.assertRaisesRegex(RuntimeError, "same device"),
            ):
                offline_reset.ReleaseBundleMove.preflight(source, output)

            self.assertTrue(source.exists())
            self.assertFalse(output.exists())

    def test_move_rejects_existing_destination_without_touching_source(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            temporary_path = Path(temporary).resolve()
            plan, source, output, _ = self._move_plan(temporary_path)
            _private_directory(output)
            _private_directory(output / "kagemusha")

            with self.assertRaisesRegex(RuntimeError, "already exists"):
                plan.move_into_output()

            self.assertTrue(source.exists())
            self.assertFalse(plan.moved)

    def test_post_move_validation_failure_restores_source(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            temporary_path = Path(temporary).resolve()
            plan, source, output, _ = self._move_plan(temporary_path)
            arguments = self._prepare_arguments(
                temporary_path, source, output, move=True
            )
            original_assert = offline_reset._assert_release_bundle_snapshot
            injected = False

            def reject_moved_tree(
                root: Path,
                expected: offline_reset.ReleaseBundleSnapshot,
                *,
                hash_contents: bool,
            ) -> None:
                nonlocal injected
                if root == plan.destination and hash_contents and not injected:
                    injected = True
                    raise RuntimeError("injected post-move validation failure")
                original_assert(
                    root, expected, hash_contents=hash_contents
                )

            with (
                mock.patch.object(
                    offline_reset,
                    "require_private_key",
                    return_value=PRIVATE_KEY,
                ),
                mock.patch.object(
                    offline_reset,
                    "source_command_private_key_sha256",
                    return_value=hashlib.sha256(
                        PREVIOUS_PRIVATE_KEY.encode()
                    ).digest(),
                ),
                mock.patch.object(
                    offline_reset,
                    "require_private_file",
                    return_value=b"operator identity",
                ),
                mock.patch.object(
                    offline_reset, "genesis_summary", return_value={}
                ),
                mock.patch.object(offline_reset, "require_regular_file"),
                mock.patch.object(
                    offline_reset, "sha256", return_value="cd" * 32
                ),
                mock.patch.object(
                    offline_reset,
                    "run_checked",
                    side_effect=self._empty_reset_skeleton(output),
                ),
                mock.patch.object(
                    offline_reset.ReleaseBundleMove,
                    "preflight",
                    return_value=plan,
                ),
                mock.patch.object(
                    offline_reset,
                    "_assert_release_bundle_snapshot",
                    side_effect=reject_moved_tree,
                ),
            ):
                with self.assertRaisesRegex(RuntimeError, "post-move"):
                    offline_reset.prepare(arguments)

            self.assertTrue(injected)
            self.assertTrue(source.exists())
            self.assertFalse(output.exists())

    def test_later_prepare_failure_restores_release_before_cleanup(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            temporary_path = Path(temporary).resolve()
            plan, source, output, _ = self._move_plan(temporary_path)
            arguments = self._prepare_arguments(
                temporary_path, source, output, move=True
            )

            with (
                mock.patch.object(
                    offline_reset,
                    "require_private_key",
                    return_value=PRIVATE_KEY,
                ),
                mock.patch.object(
                    offline_reset,
                    "source_command_private_key_sha256",
                    return_value=hashlib.sha256(
                        PREVIOUS_PRIVATE_KEY.encode()
                    ).digest(),
                ),
                mock.patch.object(
                    offline_reset,
                    "require_private_file",
                    return_value=b"operator identity",
                ),
                mock.patch.object(
                    offline_reset, "genesis_summary", return_value={}
                ),
                mock.patch.object(offline_reset, "require_regular_file"),
                mock.patch.object(
                    offline_reset, "sha256", return_value="cd" * 32
                ),
                mock.patch.object(
                    offline_reset,
                    "run_checked",
                    side_effect=self._empty_reset_skeleton(output),
                ),
                mock.patch.object(
                    offline_reset.ReleaseBundleMove,
                    "preflight",
                    return_value=plan,
                ),
                mock.patch.object(
                    offline_reset,
                    "operator_identity_binding",
                    side_effect=RuntimeError("injected later failure"),
                ),
            ):
                with self.assertRaisesRegex(RuntimeError, "later failure"):
                    offline_reset.prepare(arguments)

            self.assertTrue(source.exists())
            self.assertFalse(output.exists())

    def test_rollback_failure_preserves_output_for_recovery(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            temporary_path = Path(temporary).resolve()
            plan, source, output, _ = self._move_plan(temporary_path)
            arguments = self._prepare_arguments(
                temporary_path, source, output, move=True
            )
            original_rename = os.rename

            def fail_rollback(source_path: Path, destination_path: Path) -> None:
                if (
                    Path(source_path) == plan.destination
                    and Path(destination_path) == plan.source
                ):
                    raise OSError("injected rollback rename failure")
                original_rename(source_path, destination_path)

            with (
                mock.patch.object(
                    offline_reset,
                    "require_private_key",
                    return_value=PRIVATE_KEY,
                ),
                mock.patch.object(
                    offline_reset,
                    "source_command_private_key_sha256",
                    return_value=hashlib.sha256(
                        PREVIOUS_PRIVATE_KEY.encode()
                    ).digest(),
                ),
                mock.patch.object(
                    offline_reset,
                    "require_private_file",
                    return_value=b"operator identity",
                ),
                mock.patch.object(
                    offline_reset, "genesis_summary", return_value={}
                ),
                mock.patch.object(offline_reset, "require_regular_file"),
                mock.patch.object(
                    offline_reset, "sha256", return_value="cd" * 32
                ),
                mock.patch.object(
                    offline_reset,
                    "run_checked",
                    side_effect=self._empty_reset_skeleton(output),
                ),
                mock.patch.object(
                    offline_reset.ReleaseBundleMove,
                    "preflight",
                    return_value=plan,
                ),
                mock.patch.object(
                    offline_reset,
                    "operator_identity_binding",
                    side_effect=RuntimeError("injected later failure"),
                ),
                mock.patch.object(os, "rename", side_effect=fail_rollback),
            ):
                with self.assertRaisesRegex(
                    RuntimeError, "output preserved for recovery"
                ):
                    offline_reset.prepare(arguments)

            self.assertFalse(source.exists())
            self.assertTrue(output.exists())
            self.assertTrue((output / "kagemusha").exists())

    def test_move_preflight_rejects_aliases_symlinks_and_unsafe_modes(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            temporary_path = Path(temporary).resolve()
            source = temporary_path / "source-release"
            _private_directory(source)
            release, source = self._release_bundle(source)

            alias = temporary_path / "release-alias"
            alias.symlink_to(source, target_is_directory=True)
            with self.assertRaisesRegex(RuntimeError, "canonical"):
                offline_reset.ReleaseBundleMove.preflight(
                    alias, temporary_path / "alias-output"
                )

            artifact = release / "artifact-12.bin"
            artifact.unlink()
            artifact.symlink_to(release / "artifact-11.bin")
            with self.assertRaisesRegex(RuntimeError, "unsafe"):
                offline_reset.ReleaseBundleMove.preflight(
                    source, temporary_path / "symlink-output"
                )

        with tempfile.TemporaryDirectory() as temporary:
            temporary_path = Path(temporary).resolve()
            source = temporary_path / "source-release"
            _private_directory(source)
            self._release_bundle(source)
            (source / "release-policy-v1.norito").chmod(0o644)
            with self.assertRaisesRegex(RuntimeError, "unsafe"):
                offline_reset.ReleaseBundleMove.preflight(
                    source, temporary_path / "mode-output"
                )

        with tempfile.TemporaryDirectory() as temporary:
            temporary_path = Path(temporary).resolve()
            source = temporary_path / "source-release"
            _private_directory(source)
            self._release_bundle(source)
            actual_uid = os.getuid()
            with (
                mock.patch.object(os, "getuid", return_value=actual_uid + 1),
                self.assertRaisesRegex(RuntimeError, "unsafe"),
            ):
                offline_reset.ReleaseBundleMove.preflight(
                    source, temporary_path / "owner-output"
                )

        with tempfile.TemporaryDirectory() as temporary:
            temporary_path = Path(temporary).resolve()
            source = temporary_path / "source-release"
            _private_directory(source)
            self._release_bundle(source)
            _private_file(source / "unexpected", b"not canonical")
            with self.assertRaisesRegex(RuntimeError, "only its catalog"):
                offline_reset.ReleaseBundleMove.preflight(
                    source, temporary_path / "inventory-output"
                )

        with tempfile.TemporaryDirectory() as temporary:
            temporary_path = Path(temporary).resolve()
            source = temporary_path / "source-release"
            _private_directory(source)
            self._release_bundle(source)
            with self.assertRaisesRegex(RuntimeError, "contain"):
                offline_reset.ReleaseBundleMove.preflight(
                    source, source / "nested-output"
                )

    def test_copies_exact_catalog_as_new_single_link_files(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            temporary_path = Path(temporary).resolve()
            source = temporary_path / "source"
            _private_directory(source)
            _, source = self._release_bundle(source)
            destination = temporary_path / "destination"
            digest = offline_reset.copy_release_bundle(source, destination)
            copied = destination / "catalog" / digest
            self.assertEqual(len(list(copied.iterdir())), 16)
            self.assertTrue(
                all(path.lstat().st_nlink == 1 for path in copied.iterdir())
            )

    def test_release_copy_streams_source_files_without_path_read_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            temporary_path = Path(temporary).resolve()
            source = temporary_path / "source"
            _private_directory(source)
            _, source = self._release_bundle(source)
            destination = temporary_path / "destination"
            original_read_bytes = Path.read_bytes

            def reject_source_read_bytes(path: Path) -> bytes:
                if path == source or source in path.parents:
                    raise AssertionError(
                        f"release source was read into memory: {path}"
                    )
                return original_read_bytes(path)

            with mock.patch.object(
                Path, "read_bytes", new=reject_source_read_bytes
            ):
                digest = offline_reset.copy_release_bundle(
                    source, destination
                )

            copied_artifact = (
                destination / "catalog" / digest / "artifact-12.bin"
            )
            self.assertEqual(copied_artifact.read_bytes(), bytes([12]))

    def test_release_tree_hash_streams_without_path_read_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary).resolve() / "source"
            _private_directory(source)
            _, source = self._release_bundle(source)

            def reject_read_bytes(path: Path) -> bytes:
                raise AssertionError(f"release file was read into memory: {path}")

            with mock.patch.object(Path, "read_bytes", new=reject_read_bytes):
                digest = offline_reset.release_tree_sha256(source)

            self.assertRegex(digest, r"\A[0-9a-f]{64}\Z")

    def test_rejects_hardlinked_release_entries(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            temporary_path = Path(temporary).resolve()
            source = temporary_path / "source"
            _private_directory(source)
            release, source = self._release_bundle(source)
            os.link(release / "artifact-00.bin", release / "duplicate-link.bin")
            (release / "artifact-12.bin").unlink()
            with self.assertRaisesRegex(RuntimeError, "unsafe owner-private file"):
                offline_reset.copy_release_bundle(
                    source, temporary_path / "destination"
                )

    def test_rejects_manifest_bytes_that_do_not_match_named_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            temporary_path = Path(temporary).resolve()
            source = temporary_path / "source"
            _private_directory(source)
            release, source = self._release_bundle(source)
            _private_file(
                release / "manifest.norito",
                b"different manifest bytes",
            )

            with self.assertRaisesRegex(
                RuntimeError, "manifest.norito digest"
            ):
                offline_reset.copy_release_bundle(
                    source, temporary_path / "destination"
                )


if __name__ == "__main__":
    unittest.main()
