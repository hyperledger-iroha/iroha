#!/usr/bin/env python3
"""Source-contract tests for the developer-only Offline Cash V1 generator.

Prerequisites: Python 3.11+ and a repository checkout. No environment variables,
network services, Cargo build, artifact generation, or live-chain access are used.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CORE_CARGO = ROOT / "crates/iroha_core/Cargo.toml"
OFFLINE_MODULE = ROOT / "crates/iroha_core/src/zk/offline_cash_v1.rs"
GENERATOR = ROOT / "crates/iroha_core/src/zk/offline_cash_v1/dev_artifact_generator.rs"
CLI = ROOT / "crates/iroha_core/src/bin/offline_cash_artifact_candidate_v1.rs"
MODEL = ROOT / "crates/iroha_data_model/src/offline/offline_cash_release_v1.rs"
SWIFT_ARTIFACTS = ROOT / "IrohaSwift/Sources/IrohaSwift/OfflineCashArtifactsV1.swift"
KOTLIN_ARTIFACTS = (
    ROOT
    / "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/OfflineCashArtifactsV1.kt"
)


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


class OfflineCashArtifactCandidateSourceTests(unittest.TestCase):
    def test_binary_and_facade_are_dev_only(self) -> None:
        cargo = read(CORE_CARGO)
        module = read(OFFLINE_MODULE)
        self.assertRegex(
            cargo,
            r'(?s)name = "offline_cash_artifact_candidate_v1".*?required-features = '
            r'\["dev-tools", "zk-halo2-ipa"\]',
        )
        self.assertIn('#[cfg(feature = "dev-tools")]\nmod dev_artifact_generator;', module)
        self.assertIn('#[cfg(feature = "dev-tools")]\npub use dev_artifact_generator', module)

    def test_file_inventory_matches_all_34_roles_exactly(self) -> None:
        model = read(MODEL)
        generator = read(GENERATOR)
        swift = read(SWIFT_ARTIFACTS)
        kotlin = read(KOTLIN_ARTIFACTS)
        all_body = re.search(
            r"pub const ALL: \[Self; 34\] = \[(.*?)\n    \];", model, re.DOTALL
        )
        self.assertIsNotNone(all_body)
        all_roles = re.findall(r"Self::([A-Za-z0-9_]+)", all_body.group(1))
        swift_body = re.search(
            r"public enum OfflineCashArtifactRoleV1:.*?\{(.*?)\n\}", swift, re.DOTALL
        )
        self.assertIsNotNone(swift_body)
        swift_roles = re.findall(
            r"^\s*case ([A-Za-z0-9_]+)\s*$", swift_body.group(1), re.MULTILINE
        )
        kotlin_body = re.search(
            r"enum class OfflineCashArtifactRoleV1 \{(.*?)\n\}", kotlin, re.DOTALL
        )
        self.assertIsNotNone(kotlin_body)
        kotlin_roles = re.findall(
            r"^\s*([A-Z][A-Z0-9_]*),\s*$", kotlin_body.group(1), re.MULTILINE
        )
        mappings = re.findall(r'Role::([A-Za-z0-9_]+) => "([a-z0-9_]+\.bin)"', generator)
        self.assertEqual(len(all_roles), 34)
        self.assertEqual(swift_roles, [role[0].lower() + role[1:] for role in all_roles])
        self.assertEqual(
            kotlin_roles,
            [re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", role).upper() for role in all_roles],
        )
        self.assertEqual([role for role, _ in mappings], all_roles)
        names = [name for _, name in mappings]
        self.assertEqual(len(names), len(set(names)))
        self.assertNotIn("/", "".join(names))
        self.assertNotIn("..", "".join(names))

        generation_body = generator.split(
            "fn generate_cryptographic_artifacts_v1", 1
        )[1].split("\n#[cfg(test)]", 1)[0]
        generated_roles = set(re.findall(r"Artifact::([A-Za-z0-9_]+)", generation_body))
        self.assertEqual(generated_roles, set(all_roles))

    def test_cli_has_only_non_authority_options_and_atomic_new_output(self) -> None:
        cli = read(CLI)
        accepted_options = set(re.findall(r'argument == "(--[a-z0-9-]+)"', cli))
        self.assertEqual(accepted_options, {"--help", "--out-dir"})
        for required in (
            ".is_absolute()",
            ".canonicalize()",
            ".create_new(true)",
            "rustix::fs::RenameFlags::NOREPLACE",
            "directory.sync_all()",
            "authenticated_release: false",
            "not_qualified_not_attested_not_promoted",
        ):
            self.assertIn(required, cli)
        self.assertNotRegex(cli, r"\b(?:KeyPair|PrivateKey|SignatureOf)\b")

    def test_generator_cannot_return_a_partial_role_set(self) -> None:
        generator = read(GENERATOR)
        self.assertIn("artifacts.len() != OfflineCashArtifactRoleV1::ALL.len()", generator)
        self.assertIn("OfflineCashArtifactRoleV1::ALL", generator)
        self.assertIn("offline_cash_artifact_set_digest_v1(&bindings)", generator)
        self.assertIn("generated.emit_all", read(CLI))
        self.assertNotRegex(generator, r"\b(?:KeyPair|PrivateKey|SignatureOf)\b")

    def test_generator_closes_and_terminally_qualifies_both_recursive_pairs(self) -> None:
        generator = read(GENERATOR)
        for required in (
            "create_poseidon_direct_instance_proof_v1",
            "create_poseidon_accumulator_fold_proof_v1",
            "offline_cash_guard_bundle_keygen_audit_digests_v1",
            "GuardBundle audit binding was not stable after final substitution",
            "offline_cash_state_keygen_audit_digests_v1",
            "final-State audit binding was not stable after final substitution",
            "build_state_prover_pair_v1",
            "OFFLINE_CASH_PAIRED_PROOF_TARGET_BYTES_V1 / 2",
            "FINAL_STATE_EXPECTED_ORDINARY_PROOF_BYTES_V1: usize = 3_072",
            "GUARD_BUNDLE_EXPECTED_ORDINARY_PROOF_BYTES_V1: usize = 3_264",
            "P256_V3_EXPECTED_ORDINARY_PROOF_BYTES_V1: usize = 4_544",
            "terminal_verify_eq_outer_and_carried_v1",
            "terminal_verify_ep_outer_and_carried_v1",
        ):
            self.assertIn(required, generator)
        self.assertNotIn("developer generator is unavailable", generator)

    def test_private_calibration_material_has_no_output_surface(self) -> None:
        generator = read(GENERATOR)
        cli = read(CLI)
        self.assertNotRegex(generator, r"\b(?:print|println|eprint|eprintln|dbg)!\s*\(")
        key_drop = generator.index("drop(key);")
        relation_close = generator.index(
            "OfflineCashValidatedHelperRelationV1::new", key_drop
        )
        self.assertLess(key_drop, relation_close)
        self.assertIn('authenticated_release: false', cli)
        self.assertNotRegex(
            cli,
            r"struct ArtifactManifestCandidateV1 \{[^}]*\b(?:private_key|signature_bytes)\b",
        )


if __name__ == "__main__":
    unittest.main()
