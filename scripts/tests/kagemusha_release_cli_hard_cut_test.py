"""Pin Kagami's KAGEMUSHA V1 authentication command and retired provisioning cut.

These source-only tests require no compiled binary, network access, release
artifacts, or environment variables. Runtime authentication is covered by the
Rust tests after the shared Cargo lane is available.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

from zk_source_tokens import rust_tokens


ROOT = Path(__file__).resolve().parents[2]
MAIN = ROOT / "crates/iroha_kagami/src/main.rs"
COMMAND = ROOT / "crates/iroha_kagami/src/kagemusha.rs"
HELP = ROOT / "crates/iroha_kagami/CommandLineHelp.md"
RELEASE_MODEL = (
    ROOT / "crates/iroha_data_model/src/kagemusha/kagemusha_release_v1.rs"
)


def _artifact_role_identities_v1(model_source: str) -> list[tuple[str, int]]:
    """Read the closed unit-variant enum, honoring Rust implicit discriminants."""
    tokens = rust_tokens(model_source)
    header = ("pub", "enum", "KagemushaArtifactRoleV1", "{")
    starts = [index for index in range(len(tokens) - len(header) + 1)
              if tokens[index:index + len(header)] == header]
    if len(starts) != 1:
        raise AssertionError("artifact role enum is missing or duplicated")
    cursor = starts[0] + len(header)
    result = []
    next_identity = 0
    while cursor < len(tokens) and tokens[cursor] != "}":
        name = tokens[cursor]
        if re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", name) is None:
            raise AssertionError("artifact role is not a unit variant")
        cursor += 1
        identity = next_identity
        if cursor < len(tokens) and tokens[cursor] == "=":
            cursor += 1
            if cursor >= len(tokens) or re.fullmatch(r"[0-9]+", tokens[cursor]) is None:
                raise AssertionError("artifact role discriminant is not a decimal identity")
            identity = int(tokens[cursor])
            cursor += 1
        if cursor >= len(tokens) or tokens[cursor] != ",":
            raise AssertionError("artifact role declaration is malformed")
        result.append((name, identity))
        next_identity = identity + 1
        cursor += 1
    if cursor >= len(tokens):
        raise AssertionError("artifact role enum is unterminated")
    return result


class KagemushaReleaseCliHardCutTests(unittest.TestCase):
    """Reject compatibility paths around the current authentication contract."""

    def test_command_requires_the_complete_authenticated_release_inputs(self) -> None:
        source = COMMAND.read_text(encoding="utf-8")
        for type_name in (
            "KagemushaReleaseManifestV1",
            "KagemushaInternalValidationReceiptV1",
            "KagemushaReleaseAuthorityPolicyV1",
            "KagemushaReleaseAttestationV1",
        ):
            self.assertIn(f"{type_name}::decode_canonical_exact", source)
        self.assertIn(
            ".authenticate(&receipt, &policy, &attestation)",
            source,
        )
        for maximum in (
            "KAGEMUSHA_RELEASE_MANIFEST_MAX_BYTES_V1",
            "KAGEMUSHA_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1",
            "KAGEMUSHA_RELEASE_AUTHORITY_POLICY_MAX_BYTES_V1",
            "KAGEMUSHA_RELEASE_ATTESTATION_MAX_BYTES_V1",
        ):
            self.assertIn(maximum, source)
        self.assertIn("read_bounded_immutable_file", source)
        self.assertIn("O_NOFOLLOW", source)
        self.assertEqual(
            source.count("crate::secure_fs::same_single_link_input_snapshot("),
            4,
        )
        self.assertNotIn("fn same_input_metadata", source)
        self.assertIn("immutable snapshot was not read in full", source)
        self.assertIn("#[cfg(not(unix))]", source)
        self.assertIn("authentication is unavailable on this platform", source)

    def test_command_has_no_compatibility_alias_or_abi_selector(self) -> None:
        main_source = MAIN.read_text(encoding="utf-8").split("#[cfg(test)]", 1)[0]
        command_source = COMMAND.read_text(encoding="utf-8").split(
            "#[cfg(test)]", 1
        )[0]
        help_source = HELP.read_text(encoding="utf-8")
        production = "\n".join((main_source, command_source, help_source))
        retired = "".join(("verify-release-", "v4"))
        self.assertNotIn(retired, production.lower())
        self.assertNotIn("--abi-version", production)
        self.assertNotRegex(command_source, r"\babi_version\b|\bserde(?:_json)?\b")
        command_names = re.findall(r'#\[command\(name = "([^"]+)"\)\]', command_source)
        self.assertEqual(
            command_names,
            [
                "prepare-mobile-bootstrap-v1",
                "sign-mobile-bootstrap-approval-v1",
                "assemble-mobile-bootstrap-v1",
                "prepare-experimental-release-v1",
                "authenticate-release-v1",
                "authenticate-experimental-release-v1",
                "sign-experimental-release-approval-v1",
                "assemble-experimental-release-v1",
            ],
        )
        self.assertIn(
            "Command::AuthenticateReleaseV1(args) => authenticate_release_v1(&args, writer)",
            command_source,
        )
        self.assertIn(
            "Command::AuthenticateExperimentalReleaseV1(args)",
            command_source,
        )
        for retired_command in (
            "derive-mint-finality-next-epoch-v1",
            "derive-mint-finality-epoch-schedule-v1",
        ):
            self.assertNotIn(retired_command, production)
        self.assertNotIn("derive_mint_finality_next_epoch_v1", command_source)
        self.assertFalse(
            (COMMAND.parent / "kagemusha/derive_mint_finality_next_epoch_v1.rs").exists()
        )
        for field in (
            "recursive_profile",
            "artifact_root",
            "authority_review_projection",
            "authority_review_projection_sha256",
            "native_artifact_manifest",
            "native_artifact_manifest_sha256",
            "native_artifact",
        ):
            self.assertIn(field, command_source)

    def test_source_declares_exact_full_path_and_digest_option_inventory(self) -> None:
        source = COMMAND.read_text(encoding="utf-8").split("impl<T: Write>", 1)[0]
        fields = re.findall(r"^    ([a-z][a-z0-9_]+): (?:PathBuf|String),$", source, re.M)
        self.assertEqual(
            fields,
            [
                "manifest",
                "validation_receipt",
                "authority_policy",
                "attestation",
                "recursive_profile",
                "artifact_root",
                "authority_review_projection",
                "authority_review_projection_sha256",
                "native_artifact_manifest",
                "native_artifact_manifest_sha256",
                "native_artifact",
            ],
        )

    def test_help_exposes_the_complete_fail_closed_input_inventory(self) -> None:
        help_source = HELP.read_text(encoding="utf-8")
        match = re.search(
            r"## `kagami kagemusha authenticate-release-v1`\n(.*?)(?=\n## `|\Z)",
            help_source,
            re.S,
        )
        self.assertIsNotNone(match)
        assert match is not None
        options = re.findall(
            r"^\* `(--[a-z0-9-]+) <(?:PATH|LOWER_HEX)>`", match.group(1), re.M
        )
        self.assertEqual(
            options,
            [
                "--manifest",
                "--validation-receipt",
                "--authority-policy",
                "--attestation",
                "--recursive-profile",
                "--artifact-root",
                "--authority-review-projection",
                "--authority-review-projection-sha256",
                "--native-artifact-manifest",
                "--native-artifact-manifest-sha256",
                "--native-artifact",
            ],
        )
        self.assertNotIn("--abi-version", match.group(1))

    def test_cli_and_model_pin_the_complete_54_role_inventory(self) -> None:
        command_source = COMMAND.read_text(encoding="utf-8")
        model_source = RELEASE_MODEL.read_text(encoding="utf-8")
        self.assertNotIn("KAGEMUSHA_RELEASE_ARTIFACT_ROLE_COUNT_V1", command_source)
        self.assertIn("KagemushaArtifactRoleV1::ALL.len()", command_source)
        all_match = re.search(
            r"pub const ALL: \[Self; 54\] = \[(.*?)\n    \];",
            model_source,
            re.S,
        )
        self.assertIsNotNone(all_match)
        assert all_match is not None
        self.assertEqual(len(re.findall(r"\bSelf::[A-Za-z0-9_]+", all_match.group(1))), 54)
        roles = re.findall(r"\bSelf::([A-Za-z0-9_]+)", all_match.group(1))
        self.assertEqual(len(set(roles)), 54)
        self.assertEqual(roles[-4:], [
            "OrdinaryAppGuardPkEq", "OrdinaryAppGuardVkEq",
            "OrdinaryAppGuardPkEp", "OrdinaryAppGuardVkEp",
        ])
        identities = _artifact_role_identities_v1(model_source)
        self.assertEqual([name for name, _ in identities], roles)
        self.assertEqual([value for _, value in identities], list(range(54)))


    def test_role_identity_parser_preserves_implicit_explicit_and_trivia_semantics(self) -> None:
        source = "pub enum KagemushaArtifactRoleV1 { First, Second = 34, Third, }"
        self.assertEqual(_artifact_role_identities_v1(source), [("First", 0), ("Second", 34), ("Third", 35)])
        model = RELEASE_MODEL.read_text(encoding="utf-8")
        identities = _artifact_role_identities_v1(model)
        grown = model.replace("    ParamsEq,", "/* " + "\n" * 25_000 + " */\n    ParamsEq ,", 1)
        self.assertGreater(len(grown), len(model) + 25_000)
        self.assertEqual(_artifact_role_identities_v1(grown), identities)

    def test_role_identity_guard_detects_implicit_explicit_order_and_name_changes(self) -> None:
        source = RELEASE_MODEL.read_text(encoding="utf-8")
        original = _artifact_role_identities_v1(source)
        mutations = [
            ("    ParamsEq,", "    ParamsEq = 1,"),
            ("    InnerMintAuthorizationPkEq = 34,", "    InnerMintAuthorizationPkEq = 35,"),
            ("    OrdinaryAppGuardVkEp = 53,", "    OrdinaryAppGuardVkEp = 54,"),
            ("    OrdinaryAppGuardPkEq = 50,", "    OrdinaryAppGuardVkEq = 50,"),
            ("    ParamsEq,", "    ParamsEp,"),
        ]
        for old, new in mutations:
            with self.subTest(old=old, new=new):
                self.assertIn(old, source)
                changed = _artifact_role_identities_v1(source.replace(old, new, 1))
                self.assertNotEqual(changed, original)
                self.assertFalse([name for name, _ in changed] == [name for name, _ in original]
                                 and [value for _, value in changed] == list(range(54)))
        for invalid in ["First(u8),", "First = OTHER,", "First = -1,", "First = 0x1,", "First"]:
            with self.subTest(invalid=invalid), self.assertRaises(AssertionError):
                _artifact_role_identities_v1("pub enum KagemushaArtifactRoleV1 { " + invalid + " }")

    def test_success_requires_projection_artifacts_runtime_and_native_evidence(self) -> None:
        source = COMMAND.read_text(encoding="utf-8").split("#[cfg(test)]", 1)[0]
        self.assertIn("validate_authority_review_projection_v1", source)
        self.assertIn("KagemushaDirectoryArtifactResolverV1", source)
        self.assertIn("sha256_reader_bounded", source)
        self.assertIn("load_authenticated_kagemusha_v1_runtime_verifier", source)
        self.assertIn("validate_native_artifact_manifest_v1", source)
        self.assertIn("hash_immutable_file_exact", source)
        self.assertIn("norito::json::to_json", source)
        for field in (
            '"release_id"',
            '"manifest_digest"',
            '"validation_receipt_digest"',
            '"authority_policy_digest"',
            '"attestation_digest"',
            '"runtime_loaded"',
            '"native_artifact_manifest_authenticated"',
            '"native_artifact_hash_verified"',
            '"native_bridge_probe_performed"',
            '"approved_signers"',
            '"artifacts"',
            '"enabled_profiles"',
        ):
            self.assertIn(field, source)
        self.assertIn('"status", "authenticated"', source)
        self.assertNotIn("release_identity_authenticated", source)
        self.assertNotIn("writeln!(writer", source)

    def test_native_manifest_and_report_contracts_are_exact_first_release(self) -> None:
        source = COMMAND.read_text(encoding="utf-8").split("#[cfg(test)]", 1)[0]
        self.assertIn('"iroha.native-sdk-abi25-artifact.v1"', source)
        self.assertIn('manifest.sdk != "c-jni"', source)
        self.assertIn("manifest.bridge_abi_version != 25", source)
        self.assertIn("REQUIRED_C_JNI_SYMBOLS_V1", source)
        self.assertIn("REQUIRED_PRIVACY_C_EXPORTS_V1", source)
        self.assertIn('"native_bridge_probe_performed", &false', source)
        self.assertNotIn("dlopen", source)
        self.assertNotIn("libloading", source)

    def test_report_has_no_chain_deployment_binding(self) -> None:
        source = COMMAND.read_text(encoding="utf-8").split("#[cfg(test)]", 1)[0]
        report = source.split("fn authenticated_release_report_v1", 1)[1]
        for retired_binding in (
            '"network_id"',
            '"asset_id"',
            '"dataspace_id"',
            '"lane_id"',
        ):
            self.assertNotIn(retired_binding, report)

    def test_experimental_release_requires_separate_signature_and_operator_scope(self) -> None:
        source = COMMAND.read_text(encoding="utf-8").split("#[cfg(test)]", 1)[0]
        self.assertIn("decode_canonical_experimental_exact", source)
        self.assertIn(".authenticate_experimental(&receipt, &policy, &attestation)", source)
        self.assertIn("validate_experimental_operator_pins_v1", source)
        self.assertIn("rehash_all_release_artifacts_v1(&manifest.artifacts", source)
        for pin in (
            "expected_network_id",
            "expected_release_id",
            "expected_asset_identity_digest",
            "expected_asset_incarnation",
            "asset_scale",
            "expected_liability_pool_id",
        ):
            self.assertIn(pin, source)
        self.assertIn('long = "expected-asset-scale"', source)
        self.assertIn("asset_scale: pins.asset_scale", source)
        self.assertIn('"hardware_qualified", &false', source)
        self.assertIn('"monetary_admission", &false', source)
        self.assertIn('"runtime_loaded", &false', source)
        self.assertIn(".experimental_release_attestation_subject(receipt, policy)", source)
        self.assertIn("crate::secure_fs::read_private_file(path)", source)
        self.assertIn("SignatureOf::try_new(signing_key.private_key(), &payload)", source)
        self.assertIn(".authenticate_experimental(&receipt, &policy, &attestation)", source)
        self.assertIn("crate::secure_fs::write_private_file_atomic", source)
        self.assertNotIn("signer_private_keys: Vec", source)
        self.assertIn("experimental_receipt_evidence_files_v1", source)
        self.assertIn("rehash_experimental_receipt_evidence_v1", source)
        self.assertIn("KagemushaReleasePurposeV1::TestnetExperiment(scope)", source)
        self.assertIn(".canonical_experimental_digest()", source)
        self.assertRegex(
            source,
            r"validate_authority_review_projection_v1\(\s*&projection_bytes,\s*&manifest,\s*&receipt,\s*AuthorityReviewPurposeV1::TestnetExperiment",
        )
        self.assertIn('"prepared_unsigned_experimental_candidate"', source)


if __name__ == "__main__":
    unittest.main()
