#!/usr/bin/env python3
"""Adversarial tests for the Taira first-release privacy bootstrap contract."""

from __future__ import annotations

import base64
import contextlib
import hashlib
import io
import json
import os
import shutil
import tempfile
import unittest
from pathlib import Path

import validate_privacy_bootstrap as target


class PrivacyBootstrapValidationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        root = Path(self.temp.name)
        self.plan = root / "privacy_bootstrap_plan.json"
        self.config = root / "config.toml"
        self.genesis = root / "genesis.json"
        self.matrix = root / "exact12_v1.tsv"
        self.broker = root / "broker-public.json"
        shutil.copyfile(target.DEFAULT_PLAN, self.plan)
        shutil.copyfile(target.DEFAULT_CONFIG, self.config)
        shutil.copyfile(target.DEFAULT_GENESIS, self.genesis)
        shutil.copyfile(target.DEFAULT_MATRIX, self.matrix)
        self.paths = target.ValidationPaths(
            self.plan, self.config, self.genesis, self.matrix, self.broker
        )

    def tearDown(self) -> None:
        self.temp.cleanup()

    def load_plan(self) -> dict:
        return json.loads(self.plan.read_text(encoding="utf-8"))

    def write_plan(self, plan: dict) -> None:
        self.plan.write_text(
            json.dumps(plan, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
        )

    def load_genesis(self) -> dict:
        return json.loads(self.genesis.read_text(encoding="utf-8"))

    def write_genesis(self, genesis: dict) -> None:
        self.genesis.write_text(
            json.dumps(genesis, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
        )

    def load_broker(self) -> dict:
        return json.loads(self.broker.read_text(encoding="utf-8"))

    def write_broker(self, broker: dict, *, rebind_plan: bool = False) -> None:
        payload = (
            json.dumps(
                broker,
                ensure_ascii=False,
                allow_nan=False,
                separators=(",", ":"),
                sort_keys=True,
            )
            + "\n"
        ).encode()
        self.broker.write_bytes(payload)
        if rebind_plan:
            plan = self.load_plan()
            plan["bootle_lantern_issuer"]["public_export_sha256"] = hashlib.sha256(
                payload
            ).hexdigest()
            self.write_plan(plan)

    def assert_rejected(self, expected: str, *, release: bool = False) -> None:
        with self.assertRaisesRegex(
            target.PrivacyBootstrapValidationError, expected
        ):
            target.validate(self.paths, release=release)

    def materialize_release_public_inputs(self) -> tuple[list[bytes], bytes]:
        activations = [f"activation-{index}".encode() for index in range(12)]
        policy_instruction = b"issuer-policy"
        plan = self.load_plan()
        plan["genesis_registration"]["instruction_norito_sha256"] = [
            hashlib.sha256(value).hexdigest() for value in activations
        ]
        bootle = plan["bootle_lantern_issuer"]
        provider_digest = hashlib.sha256(b"provider-policy").hexdigest()
        bootle["runtime_provider"]["qualification_policy_digest_hex"] = provider_digest
        bootle["governed_issuer_policy"].update(
            {
                "instruction_norito_sha256": hashlib.sha256(
                    policy_instruction
                ).hexdigest(),
                "issuer_parameter_id_hex": hashlib.sha256(
                    b"issuer-parameter-id"
                ).hexdigest(),
                "issuer_parameter_digest_hex": hashlib.sha256(
                    b"issuer-parameter"
                ).hexdigest(),
                "record_digest_hex": hashlib.sha256(b"issuer-record").hexdigest(),
            }
        )
        policy = bootle["governed_issuer_policy"]
        broker = {
            "schema": target.EXPECTED_BROKER_SCHEMA,
            "chain_id": target.EXPECTED_CHAIN_ID,
            "runtime_provider_handle": target.EXPECTED_PROVIDER_HANDLE,
            "runtime_provider_revision": 1,
            "runtime_provider_policy_digest_hex": provider_digest,
            "issuer_id_hex": bootle["issuer_id_hex"],
            "policy_id_hex": bootle["policy_id_hex"],
            "authorization_lifetime_blocks": 300,
            "issuer_parameter_id_hex": policy["issuer_parameter_id_hex"],
            "issuer_parameter_digest_hex": policy["issuer_parameter_digest_hex"],
            "policy_record_digest_hex": policy["record_digest_hex"],
            "stable_principal_digest_hex": hashlib.sha256(b"principal").hexdigest(),
            "issuer_profile_digest_hex": hashlib.sha256(b"profile").hexdigest(),
            "broker_contract_digest_hex": hashlib.sha256(b"contract").hexdigest(),
            "registration_instruction_norito_hex": policy_instruction.hex(),
            "registration_instruction_norito_sha256": hashlib.sha256(
                policy_instruction
            ).hexdigest(),
            "registration_instruction": {
                "policy": {
                    "issuer_id": list(bytes.fromhex(bootle["issuer_id_hex"])),
                    "policy_id": list(bytes.fromhex(bootle["policy_id_hex"])),
                    "epoch": 1,
                    "lifecycle": {"state": "active", "value": None},
                    "issuer_parameter_id": list(
                        bytes.fromhex(policy["issuer_parameter_id_hex"])
                    ),
                    "issuer_parameter_digest": list(
                        bytes.fromhex(policy["issuer_parameter_digest_hex"])
                    ),
                    "issuer_public_matrix": {
                        "entries": [
                            {"coefficients": [1] * 64} for _ in range(64)
                        ]
                    },
                    "required_disclosure_bitmap": 0,
                    "allowed_values": [{"values": []} for _ in range(8)],
                    "record_digest": list(
                        bytes.fromhex(policy["record_digest_hex"])
                    ),
                }
            },
        }
        broker_payload = (
            json.dumps(
                broker,
                ensure_ascii=False,
                allow_nan=False,
                separators=(",", ":"),
                sort_keys=True,
            )
            + "\n"
        ).encode()
        self.broker.write_bytes(broker_payload)
        bootle["public_export_sha256"] = hashlib.sha256(broker_payload).hexdigest()
        self.write_plan(plan)
        config = self.config.read_text(encoding="utf-8")
        config = config.replace(
            "[torii.privacy_bootle_lantern_issuer]\nenabled = false",
            "[torii.privacy_bootle_lantern_issuer]\nenabled = true",
            1,
        )
        bindings = (
            f'issuer_id_hex = "{bootle["issuer_id_hex"]}"\n'
            f'policy_id_hex = "{bootle["policy_id_hex"]}"\n'
            f'runtime_provider_registry_handle = "{bootle["runtime_provider"]["handle"]}"\n'
            f'runtime_provider_registry_revision = {bootle["runtime_provider"]["revision"]}\n'
            f'runtime_provider_registry_policy_digest_hex = "{provider_digest}"\n'
        )
        config = config.replace(
            "terminal_retention_blocks = 4096\n\n[torii.cors]",
            f"terminal_retention_blocks = 4096\n{bindings}\n[torii.cors]",
            1,
        )
        self.config.write_text(config, encoding="utf-8")
        return activations, policy_instruction

    def test_canonical_staging_contract_passes(self) -> None:
        target.validate(self.paths, release=False)

    def test_auto_mode_selects_canonical_staging(self) -> None:
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            status = target.main(
                [
                    "--mode",
                    "auto",
                    "--plan",
                    str(self.plan),
                    "--config",
                    str(self.config),
                    "--genesis",
                    str(self.genesis),
                    "--matrix",
                    str(self.matrix),
                ]
            )
        self.assertEqual(status, 0)
        self.assertIn("staging contract", output.getvalue())

    def test_auto_mode_rejects_hybrid_partial_material(self) -> None:
        plan = self.load_plan()
        plan["genesis_registration"]["instruction_norito_sha256"] = ["11" * 32]
        self.write_plan(plan)
        errors = io.StringIO()
        with contextlib.redirect_stderr(errors):
            status = target.main(
                [
                    "--mode",
                    "auto",
                    "--plan",
                    str(self.plan),
                    "--config",
                    str(self.config),
                    "--genesis",
                    str(self.genesis),
                    "--matrix",
                    str(self.matrix),
                ]
            )
        self.assertEqual(status, 1)
        self.assertIn("failed in both modes", errors.getvalue())

    def test_release_rejects_missing_activation_inventory(self) -> None:
        self.assert_rejected("exactly 12 activation", release=True)

    def test_reordered_protocols_are_rejected(self) -> None:
        plan = self.load_plan()
        protocols = plan["privacy_catalog"]["protocols"]
        protocols[0], protocols[1] = protocols[1], protocols[0]
        self.write_plan(plan)
        self.assert_rejected("canonical order")

    def test_retired_sis_label_cannot_replace_an_active_row(self) -> None:
        plan = self.load_plan()
        plan["privacy_catalog"]["protocols"][7]["label"] = "sis-with-hints"
        self.write_plan(plan)
        self.assert_rejected("canonical order")

    def test_retired_label_inventory_cannot_omit_sis(self) -> None:
        plan = self.load_plan()
        plan["privacy_catalog"]["retired_labels"].remove("sis-with-hints")
        self.write_plan(plan)
        self.assert_rejected("retirement set")

    def test_activation_delay_cannot_be_shortened(self) -> None:
        plan = self.load_plan()
        plan["genesis_registration"]["activate_at_height"] = 300
        self.write_plan(plan)
        self.assert_rejected("height 301")

    def test_genesis_lifecycle_must_start_proposed(self) -> None:
        plan = self.load_plan()
        plan["genesis_registration"]["lifecycle"] = "Active"
        self.write_plan(plan)
        self.assert_rejected("Proposed")

    def test_partial_activation_inventory_is_rejected(self) -> None:
        plan = self.load_plan()
        plan["genesis_registration"]["instruction_norito_sha256"] = ["11" * 32]
        self.write_plan(plan)
        self.assert_rejected("partial activation")

    def test_provider_slot_substitution_is_rejected(self) -> None:
        plan = self.load_plan()
        plan["bootle_lantern_issuer"]["runtime_provider"]["slot_wire_id"] = 53
        self.write_plan(plan)
        self.assert_rejected("slot-56")

    def test_custom_launcher_transport_is_rejected(self) -> None:
        plan = self.load_plan()
        plan["bootle_lantern_issuer"]["runtime_provider"]["transport"] = (
            "custom-launcher-registry"
        )
        self.write_plan(plan)
        self.assert_rejected("stock broker")

    def test_zero_provider_digest_is_rejected_even_during_staging(self) -> None:
        plan = self.load_plan()
        plan["bootle_lantern_issuer"]["runtime_provider"][
            "qualification_policy_digest_hex"
        ] = "0" * 64
        self.write_plan(plan)
        self.assert_rejected("must be nonzero")

    def test_unknown_provider_secret_field_is_rejected(self) -> None:
        plan = self.load_plan()
        plan["bootle_lantern_issuer"]["runtime_provider"]["private_key"] = "11" * 32
        self.write_plan(plan)
        self.assert_rejected("fields differ")

    def test_partial_governed_issuer_policy_is_rejected(self) -> None:
        plan = self.load_plan()
        plan["bootle_lantern_issuer"]["governed_issuer_policy"][
            "record_digest_hex"
        ] = "12" * 32
        self.write_plan(plan)
        self.assert_rejected("partial governed issuer policy")

    def test_staging_broker_public_digest_must_remain_null(self) -> None:
        plan = self.load_plan()
        plan["bootle_lantern_issuer"]["public_export_sha256"] = "12" * 32
        self.write_plan(plan)
        self.assert_rejected("must not bind a broker public export")

    def test_missing_governance_grant_is_rejected(self) -> None:
        genesis = self.load_genesis()
        for transaction in genesis["transactions"]:
            transaction["instructions"] = [
                instruction
                for instruction in transaction["instructions"]
                if not (
                    isinstance(instruction, dict)
                    and instruction.get("Grant", {})
                    .get("Permission", {})
                    .get("object", {})
                    .get("name")
                    == "CanEnactGovernance"
                )
            ]
        self.write_genesis(genesis)
        self.assert_rejected("grant CanEnactGovernance")

    def test_governance_grant_destination_substitution_is_rejected(self) -> None:
        genesis = self.load_genesis()
        for transaction in genesis["transactions"]:
            for instruction in transaction["instructions"]:
                try:
                    permission = instruction["Grant"]["Permission"]
                    if permission["object"]["name"] == "CanEnactGovernance":
                        permission["destination"] = "attacker@wonderland"
                except (KeyError, TypeError):
                    pass
        self.write_genesis(genesis)
        self.assert_rejected("wrong authority")

    def test_coordinated_plan_and_genesis_authority_substitution_is_rejected(self) -> None:
        genesis = self.load_genesis()
        replacement = "attacker@wonderland"
        authority = self.load_plan()["genesis_authority"]
        for transaction in genesis["transactions"]:
            for instruction in transaction["instructions"]:
                try:
                    account = instruction["Register"]["Account"]
                    if account["id"] == authority:
                        account["id"] = replacement
                except (KeyError, TypeError):
                    pass
                try:
                    permission = instruction["Grant"]["Permission"]
                    if (
                        permission["object"]["name"] == "CanEnactGovernance"
                        and permission["destination"] == authority
                    ):
                        permission["destination"] = replacement
                except (KeyError, TypeError):
                    pass
        plan = self.load_plan()
        plan["genesis_authority"] = replacement
        self.write_plan(plan)
        self.write_genesis(genesis)
        self.assert_rejected("wrong Taira governance authority")

    def test_scoped_governance_grant_is_rejected(self) -> None:
        genesis = self.load_genesis()
        for transaction in genesis["transactions"]:
            for instruction in transaction["instructions"]:
                try:
                    permission = instruction["Grant"]["Permission"]
                    if permission["object"]["name"] == "CanEnactGovernance":
                        permission["object"]["payload"] = {"scope": "privacy"}
                except (KeyError, TypeError):
                    pass
        self.write_genesis(genesis)
        self.assert_rejected("must be unscoped")

    def test_duplicate_governance_grant_is_rejected(self) -> None:
        genesis = self.load_genesis()
        grant = None
        for transaction in genesis["transactions"]:
            for instruction in transaction["instructions"]:
                try:
                    if (
                        instruction["Grant"]["Permission"]["object"]["name"]
                        == "CanEnactGovernance"
                    ):
                        grant = instruction
                except (KeyError, TypeError):
                    pass
        self.assertIsNotNone(grant)
        genesis["transactions"][-1]["instructions"].append(grant)
        self.write_genesis(genesis)
        self.assert_rejected("exactly once")

    def test_enabled_staging_config_is_rejected(self) -> None:
        config = self.config.read_text(encoding="utf-8").replace(
            "[torii.privacy_bootle_lantern_issuer]\nenabled = false",
            "[torii.privacy_bootle_lantern_issuer]\nenabled = true",
            1,
        )
        self.config.write_text(config, encoding="utf-8")
        self.assert_rejected("disabled in staging")

    def test_materialized_validator_private_key_is_rejected_from_public_config(self) -> None:
        config = self.config.read_text(encoding="utf-8").replace(
            "REPLACE_WITH_VALIDATOR_PRIVATE_KEY", "materialized-private-key", 1
        )
        self.config.write_text(config, encoding="utf-8")
        self.assert_rejected("must not materialize a validator private key")

    def test_materialized_soranet_transport_identity_is_rejected_from_public_config(self) -> None:
        for placeholder, materialized in (
            (
                "REPLACE_WITH_SORANET_TRANSPORT_PUBLIC_KEY",
                "ed01200000000000000000000000000000000000000000000000000000000000000000",
            ),
            (
                "REPLACE_WITH_SORANET_TRANSPORT_PRIVATE_KEY",
                "802620000000000000000000000000000000000000000000000000000000000000000000",
            ),
        ):
            with self.subTest(placeholder=placeholder):
                original = self.config.read_text(encoding="utf-8")
                self.config.write_text(
                    original.replace(placeholder, materialized, 1), encoding="utf-8"
                )
                self.assert_rejected("must retain every runtime-identity placeholder")
                self.config.write_text(original, encoding="utf-8")

    def test_unrelated_config_extension_is_rejected(self) -> None:
        with self.config.open("a", encoding="utf-8") as stream:
            stream.write("\n[unreviewed_release_input]\nopaque_value = \"forbidden\"\n")
        self.assert_rejected("outside the privacy issuer section")

    def test_dormant_public_binding_is_rejected_while_disabled(self) -> None:
        config = self.config.read_text(encoding="utf-8").replace(
            "terminal_retention_blocks = 4096\n",
            f'terminal_retention_blocks = 4096\nissuer_id_hex = "{"11" * 32}"\n',
            1,
        )
        self.config.write_text(config, encoding="utf-8")
        self.assert_rejected("dormant")

    def test_undersized_authorization_store_is_rejected(self) -> None:
        config = self.config.read_text(encoding="utf-8").replace(
            "max_total_bytes = 13557760", "max_total_bytes = 13557759", 1
        )
        self.config.write_text(config, encoding="utf-8")
        self.assert_rejected("store/lifetime bounds")

    def test_changed_issuance_concurrency_bound_is_rejected(self) -> None:
        config = self.config.read_text(encoding="utf-8").replace(
            "max_inflight = 2", "max_inflight = 3", 1
        )
        self.config.write_text(config, encoding="utf-8")
        self.assert_rejected("store/lifetime bounds")

    def test_plan_concurrency_bound_substitution_is_rejected(self) -> None:
        plan = self.load_plan()
        plan["bootle_lantern_issuer"]["max_inflight"] = 3
        self.write_plan(plan)
        self.assert_rejected("public issuer contract")

    def test_relative_state_directory_is_rejected(self) -> None:
        config = self.config.read_text(encoding="utf-8").replace(
            'state_dir = "/var/lib/iroha/taira-validator-1/privacy/bootle-lantern/issuer"',
            'state_dir = "storage/issuer"',
            1,
        )
        self.config.write_text(config, encoding="utf-8")
        self.assert_rejected("store/lifetime bounds")

    def test_unverified_encoded_instruction_is_rejected_in_staging(self) -> None:
        genesis = self.load_genesis()
        genesis["transactions"][-1]["instructions"].append(
            base64.b64encode(b"unverified").decode("ascii")
        )
        self.write_genesis(genesis)
        self.assert_rejected("unverified partial encoded bootstrap")

    def test_decoded_privacy_instruction_is_rejected_in_staging(self) -> None:
        genesis = self.load_genesis()
        genesis["transactions"][-1]["instructions"].append(
            {"RegisterPrivacyProtocolActivationV1": {"protocol": "forged"}}
        )
        self.write_genesis(genesis)
        self.assert_rejected("decoded privacy bootstrap")

    def test_unrelated_genesis_extension_is_rejected(self) -> None:
        genesis = self.load_genesis()
        genesis["unreviewed_release_input"] = {"opaque_value": "forbidden"}
        self.write_genesis(genesis)
        self.assert_rejected("canonical first-release base template")

    def test_duplicate_json_key_is_rejected(self) -> None:
        payload = self.plan.read_text(encoding="utf-8").replace(
            '  "schema_version": 1,',
            '  "schema_version": 1,\n  "schema_version": 1,',
            1,
        )
        self.plan.write_text(payload, encoding="utf-8")
        self.assert_rejected("duplicate key")

    def test_symlinked_plan_is_rejected(self) -> None:
        real_plan = self.plan.with_name("real-plan.json")
        self.plan.rename(real_plan)
        os.symlink(real_plan, self.plan)
        self.assert_rejected("non-symlink regular file")

    def test_matrix_label_tamper_is_rejected(self) -> None:
        payload = self.matrix.read_text(encoding="utf-8").replace(
            "iroha-jindo-polynomial-commitment-v0",
            "jindo-lattice-pcs-zk-v0",
            1,
        )
        self.matrix.write_text(payload, encoding="utf-8")
        self.assert_rejected("matrix file digest")

    def test_release_requires_provider_qualification_after_exact12(self) -> None:
        plan = self.load_plan()
        plan["genesis_registration"]["instruction_norito_sha256"] = [
            hashlib.sha256(f"activation-{index}".encode()).hexdigest()
            for index in range(12)
        ]
        plan["bootle_lantern_issuer"]["public_export_sha256"] = "22" * 32
        self.write_plan(plan)
        self.assert_rejected("provider qualification digest", release=True)

    def test_release_requires_explicit_broker_public_path(self) -> None:
        activations, policy_instruction = self.materialize_release_public_inputs()
        genesis = self.load_genesis()
        genesis["transactions"][-1]["instructions"].extend(
            [base64.b64encode(value).decode("ascii") for value in activations]
            + [base64.b64encode(policy_instruction).decode("ascii")]
        )
        self.write_genesis(genesis)
        without_broker = target.ValidationPaths(
            self.plan, self.config, self.genesis, self.matrix, None
        )
        with self.assertRaisesRegex(
            target.PrivacyBootstrapValidationError, "requires --broker-public"
        ):
            target.validate(without_broker, release=True)

    def test_release_rejects_noncanonical_broker_public_json(self) -> None:
        self.materialize_release_public_inputs()
        self.broker.write_bytes(b" " + self.broker.read_bytes())
        self.assert_rejected("canonical compact emitted form", release=True)

    def test_release_rejects_unknown_broker_secret_field_even_if_rebound(self) -> None:
        self.materialize_release_public_inputs()
        broker = self.load_broker()
        broker["issuer_seed"] = "forbidden"
        self.write_broker(broker, rebind_plan=True)
        self.assert_rejected("fields differ", release=True)

    def test_release_rejects_structured_policy_substitution_even_if_rebound(self) -> None:
        self.materialize_release_public_inputs()
        broker = self.load_broker()
        broker["registration_instruction"]["policy"]["issuer_id"][0] ^= 1
        self.write_broker(broker, rebind_plan=True)
        self.assert_rejected("structured issuer policy", release=True)

    def test_release_rejects_missing_encoded_genesis_after_public_inputs(self) -> None:
        self.materialize_release_public_inputs()
        self.assert_rejected("exactly 12 activations", release=True)

    def test_release_rejects_noncanonical_base64_instruction(self) -> None:
        activations, policy_instruction = self.materialize_release_public_inputs()
        genesis = self.load_genesis()
        encoded = [base64.b64encode(value).decode("ascii") for value in activations]
        encoded.append(base64.b64encode(policy_instruction).decode("ascii"))
        encoded[0] = encoded[0] + "="
        genesis["transactions"][-1]["instructions"].extend(encoded)
        self.write_genesis(genesis)
        self.assert_rejected("canonical base64", release=True)

    def test_release_rejects_encoded_instruction_digest_substitution(self) -> None:
        activations, policy_instruction = self.materialize_release_public_inputs()
        genesis = self.load_genesis()
        encoded = [base64.b64encode(value).decode("ascii") for value in activations]
        encoded.append(base64.b64encode(policy_instruction + b"-substituted").decode("ascii"))
        genesis["transactions"][-1]["instructions"].extend(encoded)
        self.write_genesis(genesis)
        self.assert_rejected("digest inventory", release=True)


if __name__ == "__main__":
    unittest.main()
