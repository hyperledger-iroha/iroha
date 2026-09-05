"""Pin Kagami's sole KAGEMUSHA V1 release-authentication command.

These source-only tests require no compiled binary, network access, release
artifacts, or environment variables. Runtime authentication is covered by the
Rust tests after the shared Cargo lane is available.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MAIN = ROOT / "crates/iroha_kagami/src/main.rs"
COMMAND = ROOT / "crates/iroha_kagami/src/kagemusha.rs"
HELP = ROOT / "crates/iroha_kagami/CommandLineHelp.md"
RELEASE_MODEL = (
    ROOT / "crates/iroha_data_model/src/kagemusha/kagemusha_release_v1.rs"
)


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
        self.assertIn("same_input_metadata", source)
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
        self.assertEqual(command_names, ["authenticate-release-v1"])
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

    def test_cli_and_model_pin_the_complete_42_role_inventory(self) -> None:
        command_source = COMMAND.read_text(encoding="utf-8")
        model_source = RELEASE_MODEL.read_text(encoding="utf-8")
        self.assertIn(
            "const KAGEMUSHA_RELEASE_ARTIFACT_ROLE_COUNT_V1: usize = 42;",
            command_source,
        )
        self.assertIn("KagemushaArtifactRoleV1::ALL.len()", command_source)
        all_match = re.search(
            r"pub const ALL: \[Self; 42\] = \[(.*?)\n    \];",
            model_source,
            re.S,
        )
        self.assertIsNotNone(all_match)
        assert all_match is not None
        self.assertEqual(len(re.findall(r"\bSelf::[A-Za-z0-9_]+", all_match.group(1))), 42)

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
        self.assertIn('"iroha.native-sdk-abi23-artifact.v1"', source)
        self.assertIn('manifest.sdk != "c-jni"', source)
        self.assertIn("manifest.bridge_abi_version != 23", source)
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


if __name__ == "__main__":
    unittest.main()
