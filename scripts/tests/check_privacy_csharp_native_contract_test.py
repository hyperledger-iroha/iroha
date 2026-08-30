#!/usr/bin/env python3
"""Freeze the fail-closed ABI-22 C# privacy test and workflow contract."""

from __future__ import annotations

import re
import unittest
from pathlib import Path

from scripts import check_native_sdk_abi22_artifact as checker


REPO_ROOT = Path(__file__).resolve().parents[2]


def read(relative: str) -> str:
    """Read one UTF-8 repository file."""

    return (REPO_ROOT / relative).read_text(encoding="utf-8")


def workflow_job(source: str, name: str) -> str:
    """Return one top-level workflow job block."""

    match = re.search(
        rf"(?ms)^  {re.escape(name)}:\n.*?(?=^  [A-Za-z0-9_-]+:\n|\Z)",
        source,
    )
    if match is None:
        raise AssertionError(f"missing workflow job: {name}")
    return match.group(0)


class PrivacyCsharpNativeContractTests(unittest.TestCase):
    """Guard C# privacy tests against native capability skips."""

    def test_exact12_inspector_binds_canonical_credentials_through_pinvoke(self) -> None:
        native = read("csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs")
        torii = read(
            "csharp/src/Hyperledger.Iroha.Sdk/Torii/"
            "ToriiClient.PrivacyExact12Actions.cs"
        )

        inspector_start = native.index(
            "internal static PrivacySignedExact12ActionInspectionV1 "
            "InspectSignedExact12ActionV1("
        )
        worker_start = native.index(
            "private static PrivacySignedExact12ActionInspectionV1 "
            "InspectSignedExact12ActionOnWorker(",
            inspector_start,
        )
        inspector = native[inspector_start:worker_start]
        pinvoke_start = native.index(
            "private static extern int NativeInspectSignedExact12Action("
        )
        pinvoke_end = native.index(");", pinvoke_start)
        pinvoke = native[pinvoke_start:pinvoke_end]
        self.assertIn("string authorityAccountId", inspector)
        self.assertIn("StrictUtf8Bytes(authorityAccountId", inspector)
        self.assertIn("[In] byte[] authorityAccountId", pinvoke)
        self.assertIn("UIntPtr authorityAccountIdLength", pinvoke)
        self.assertIn("context.Credentials.AccountId,", torii)

    def test_exact12_receipt_uses_native_id105_and_fail_closed_terminal_semantics(self) -> None:
        model = read(
            "csharp/src/Hyperledger.Iroha.Sdk/Privacy/"
            "PrivacyExact12ActionModelsV1.cs"
        )
        native = read("csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs")
        torii = read(
            "csharp/src/Hyperledger.Iroha.Sdk/Torii/"
            "ToriiClient.PrivacyExact12Actions.cs"
        )
        tests = read(
            "csharp/tests/Hyperledger.Iroha.Sdk.Tests/"
            "ToriiPrivacyExact12ActionFlowTests.cs"
        )

        for managed, symbol in (
            (
                "NativeAuthenticatedActionReceiptPrepare",
                "iroha_privacy_authenticated_action_receipt_prepare_v1",
            ),
            (
                "NativeAuthenticatedActionReceiptFinalize",
                "iroha_privacy_authenticated_action_receipt_finalize_v1",
            ),
            (
                "NativeAuthenticatedActionReceiptProjectResult",
                "iroha_privacy_authenticated_action_receipt_project_result_v1",
            ),
        ):
            self.assertIn(f"private static extern int {managed}(", native)
            self.assertIn(f'EntryPoint = "{symbol}"', native)
        for marker in (
            "operation.TransactionIntentDigest.CopyTo(requestedBinding, 0)",
            "operation.StatementDigest.CopyTo(requestedBinding, 32)",
            "operation.ProofEnvelopeHash.CopyTo(requestedBinding, 64)",
            "StrictUtf8Bytes(credentials.AccountId",
            "observedFields.SetEquals(expectedFields)",
            "CryptographicOperations.FixedTimeEquals",
            "capabilityCommittedHeight > admittedAtHeight",
            "admittedAtHeight > finalizedHeight",
        ):
            self.assertIn(marker, native)
        for marker in (
            "ExecutionCapabilityManifestDigest",
            "ExecutionCapabilityCommittedHeight",
            "ExecutionReceiptFinalizedHeight",
            "ExecutionReceiptFinalizedBlockHash",
            "terminalCommittedHeight < capabilityCommittedHeight",
        ):
            self.assertIn(marker, model)
        for marker in (
            'PrivacyActionReceiptQueryPathV1 = "/v1/query"',
            "PipelineTransactionState.Committed",
            'string.Equals(status.ResolvedFrom, "cache"',
            "HttpStatusCode.NotFound",
            "details is null || receipt is null",
            "Rejected Exact12 status contradicts an authenticated execution receipt",
            "receipt.AdmittedAtHeight != details.CommittedBlockHeight",
        ):
            self.assertIn(marker, torii)
        for marker in (
            "AuthenticatedReceiptProjectionRequiresExactBoundFifteenFieldContract",
            "CommittedAndCacheExpiryRemainNonterminal",
            "DurableStateExpiryIsTerminalButLagging404EvidenceRetries",
            "ContradictoryTerminalEvidenceFailsClosedAndTerminalEvidenceIsStable",
        ):
            self.assertIn(marker, tests)

    def test_finalized_state_helpers_use_native_ids97_through104_and_only_404_is_null(self) -> None:
        model = read(
            "csharp/src/Hyperledger.Iroha.Sdk/Privacy/"
            "PrivacyFinalizedStateModelsV1.cs"
        )
        native = read("csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs")
        torii = read(
            "csharp/src/Hyperledger.Iroha.Sdk/Torii/"
            "ToriiClient.PrivacyFinalizedStateQueries.cs"
        )
        tests = read(
            "csharp/tests/Hyperledger.Iroha.Sdk.Tests/"
            "PrivacyFinalizedStateModelsV1Tests.cs"
        )

        for managed, symbol in (
            (
                "NativeAuthenticatedPrivacyStateQueryPrepare",
                "iroha_privacy_authenticated_state_query_prepare_v1",
            ),
            (
                "NativeAuthenticatedPrivacyStateQueryFinalize",
                "iroha_privacy_authenticated_state_query_finalize_v1",
            ),
            (
                "NativeAuthenticatedPrivacyStateQueryProjectResult",
                "iroha_privacy_authenticated_state_query_project_result_v1",
            ),
        ):
            self.assertIn(f"private static extern int {managed}(", native)
            self.assertIn(f'EntryPoint = "{symbol}"', native)

        binding_start = native.index("internal static void RequirePrivacyStateQueryBindingV1(")
        binding_end = native.index(
            "private static PrivacySignedExact12ActionInspectionV1", binding_start
        )
        binding = native[binding_start:binding_end]
        for query_id, size in (
            (97, 64),
            (98, 32),
            (99, 32),
            (100, 64),
            (101, 32),
            (102, 128),
            (103, 128),
            (104, 96),
        ):
            self.assertIn(f"{query_id} => {size}", binding)
        for marker in (
            "protocolIndex > 2",
            "protocolIndex != 0",
            "nonzero |= requestBinding[index] != 0",
            "StrictUtf8Bytes(credentials.AccountId",
            "Ed25519Signer.Sign(signingDigest, privateKey)",
            "CryptographicOperations.ZeroMemory(privateKey)",
            "CryptographicOperations.ZeroMemory(nonce)",
        ):
            self.assertIn(marker, native)

        for method in (
            "GetPrivacyZkAceReplayNullifierV1Async",
            "GetPrivacyProofManagedPoolStateV1Async",
            "GetPrivacyOrchardPoolStateV1Async",
            "GetPrivacyOrchardNullifierV1Async",
            "GetPrivacyAnonymousPgcPoolStateV1Async",
            "GetPrivacyZkAmsAdmissionV1Async",
            "GetPrivacyZkAmsProvisionV1Async",
            "GetPrivacyZkX509CertificateNullifierV1Async",
        ):
            self.assertIn(method, torii)
        for marker in (
            "RequirePrivacyActionContextV1(",
            "PrivacyNative.BuildAuthenticatedPrivacyStateQueryV1(",
            "PrivacyNative.ProjectAuthenticatedPrivacyStateQueryResultV1(",
            "PrivacyFinalizedStateContractV1.ParseProjectionV1(",
            "response.StatusCode == HttpStatusCode.NotFound",
            "response.StatusCode != HttpStatusCode.OK",
            "PrivacyActionNoritoMediaTypeV1",
        ):
            self.assertIn(marker, torii)
        self.assertEqual(torii.count("return null;"), 1)

        for marker in (
            "CryptographicOperations.FixedTimeEquals",
            "NetworkId.Parse(literal)",
            'literal.StartsWith("hash:", StringComparison.Ordinal)',
            "supplied != Crc16(",
            "RequireCanonicalU64String(",
            "fields.SetEquals(expected)",
            "PrivacyProofManagedPoolTransitionViewV1",
            "PrivacyOrchardPoolTransitionViewV1",
            "PrivacyAnonymousPgcPoolTransitionViewV1",
        ):
            self.assertIn(marker, model)
        for marker in (
            "RequestsExposeTheClosedNativeQueryUnionAndDefensiveBindings",
            "ProofManagedProjectionBindsNetworkSelectorAndCanonicalWireForms",
            "ProjectionRejectsHostileSchemaAndBindingMutations",
            "FinalityHashesRequireCanonicalChecksummedHashLiterals",
        ):
            self.assertIn(marker, tests)

    def test_exact12_test_requires_native_bridge_unconditionally(self) -> None:
        source = read(
            "csharp/tests/Hyperledger.Iroha.Sdk.Tests/PrivacyNativeTests.cs"
        )
        method_name = (
            "Exact12FixtureBundleRoundTripsAndRejectsAdversarialBytesThroughNativeAbi22()"
        )
        start = source.index(method_name)
        native_assertion = source.index("var bundle =", start)
        preflight = source[start:native_assertion]

        self.assertIn(
            "Assert.True(\n"
            "            PrivacyNative.IsAvailable(),\n"
            "            \"ABI-22 connect_norito_bridge with exact-12 fixture "
            "symbols is required.\");",
            preflight,
        )

        catalog_method = (
            "CompiledProfileCatalogRoundTripsAndRejectsAdversarialBytesThroughNativeAbi22()"
        )
        catalog_start = source.index(catalog_method)
        catalog_native_call = source.index("var catalog =", catalog_start)
        catalog_preflight = source[catalog_start:catalog_native_call]
        self.assertIn(
            "Assert.True(\n"
            "            PrivacyNative.IsAvailable(),\n"
            "            \"ABI-22 connect_norito_bridge with compiled-profile "
            "catalog symbols is required.\");",
            catalog_preflight,
        )
        self.assertNotIn("WhenAvailable", source)
        self.assertNotIn("IROHA_REQUIRE_PRIVACY_EXACT12_NATIVE", source)
        self.assertNotIn("if (!PrivacyNative.IsAvailable())", source)

    def test_csharp_artifact_manifest_requires_all_privacy_symbols(self) -> None:
        self.assertEqual(
            checker.REQUIRED_SYMBOLS["csharp"],
            (
                "connect_norito_bridge_abi_version",
                "connect_norito_free",
                "connect_norito_sorafs_reference_validate_appeal_finance_cancel_asset_lock_json",
                "iroha_privacy_compiled_profile_catalog_v1",
                "iroha_privacy_validate_compiled_profile_catalog_v1",
                "iroha_privacy_exact12_fixture_bundle_v1",
                "iroha_privacy_validate_exact12_fixture_bundle_v1",
                "iroha_privacy_inspect_signed_exact12_action_v1",
                "iroha_privacy_authenticated_transaction_details_prepare_v1",
                "iroha_privacy_authenticated_transaction_details_finalize_v1",
                "iroha_privacy_authenticated_transaction_details_project_result_v1",
                "iroha_privacy_authenticated_transaction_details_prepare_v2",
                "iroha_privacy_authenticated_transaction_details_finalize_v2",
                "iroha_privacy_authenticated_transaction_details_project_result_v2",
                "iroha_privacy_authenticated_finality_proof_page_bind_v1",
                "iroha_privacy_authenticated_finality_page_verify_v1",
                "iroha_privacy_authenticated_finalized_kagemusha_outcome_project_v1",
                "iroha_privacy_authenticated_finalized_action_rejection_project_v1",
                "iroha_privacy_kagemusha_topup_finality_project_v4",
                "iroha_privacy_authenticated_offline_device_registration_result_project_v1",
                "iroha_privacy_authenticated_action_receipt_prepare_v1",
                "iroha_privacy_authenticated_action_receipt_finalize_v1",
                "iroha_privacy_authenticated_action_receipt_project_result_v1",
                "iroha_privacy_authenticated_state_query_prepare_v1",
                "iroha_privacy_authenticated_state_query_finalize_v1",
                "iroha_privacy_authenticated_state_query_project_result_v1",
                "iroha_privacy_free_buffer",
            ),
        )

    def test_csharp_runner_authenticates_the_only_loadable_bridge(self) -> None:
        source = read("ci/check_privacy_csharp_sdk.sh")
        for marker in (
            '"${IROHA_REQUIRE_PRIVACY_EXACT12_NATIVE:-}" != "1"',
            "PRIVACY_CSHARP_NATIVE_ARTIFACT",
            "PRIVACY_CSHARP_NATIVE_MANIFEST",
            "scripts/check_native_sdk_abi22_artifact.py",
            '"${PYTHON_BIN}" -I -B "${ABI22_ARTIFACT_CHECKER}" verify',
            '--artifact "${PRIVACY_CSHARP_NATIVE_ARTIFACT}"',
            '--manifest "${PRIVACY_CSHARP_NATIVE_MANIFEST}"',
            '--source-root "${ROOT_DIR}"',
            '"${LD_LIBRARY_PATH:-}" != "${NATIVE_DIRECTORY}"',
            "Hyperledger.Iroha.Sdk.Tests.PrivacyNativeTests",
            "Hyperledger.Iroha.Sdk.Tests.PrivacyExact12FixtureCodecV1Tests",
            "Hyperledger.Iroha.Sdk.Tests.VerifyingKeyBackendTagTests",
            '--filter-class "${test_class}"',
        ):
            self.assertIn(marker, source)

    def test_workflow_builds_authenticates_and_consumes_bridge(self) -> None:
        source = read(".github/workflows/pr_privacy_sdk_guard.yml")
        csharp = workflow_job(source, "privacy_csharp_sdk_tests")

        for path in (
            '      - "scripts/check_native_sdk_abi22_artifact.py"',
            '      - "scripts/tests/check_privacy_csharp_native_contract_test.py"',
        ):
            self.assertIn(path, source)
        for marker in (
            "needs: privacy_jvm_sdk_tests",
            "actions/download-artifact@d3f86a106a0bac45b974a628896c90dbdf5c8093",
            "privacy-jvm-native-abi22-${{ github.sha }}",
            "Authenticate exact ABI22 C# privacy input",
            "scripts/check_native_sdk_abi22_artifact.py verify",
            'IROHA_REQUIRE_PRIVACY_EXACT12_NATIVE: "1"',
            "Bind authenticated privacy-native runtime path",
            'if [[ -z "${RUNNER_TEMP:-}" || "$RUNNER_TEMP" != /* || -z "${GITHUB_ENV:-}" ]]; then',
            'echo "LD_LIBRARY_PATH=$RUNNER_TEMP/privacy-jvm-native-abi22" >> "$GITHUB_ENV"',
            "refusing an unauthenticated privacy-native fallback",
            "PRIVACY_CSHARP_NATIVE_ARTIFACT: ${{ runner.temp }}/"
            "privacy-jvm-native-abi22/libconnect_norito_bridge.so",
            "PRIVACY_CSHARP_NATIVE_MANIFEST: ${{ runner.temp }}/"
            "privacy-jvm-native-abi22/native-sdk-abi22-csharp.json",
            "run: ci/check_privacy_csharp_sdk.sh",
        ):
            self.assertIn(marker, csharp)
        self.assertNotIn(
            "LD_LIBRARY_PATH: ${{ runner.temp }}/privacy-jvm-native-abi22",
            csharp,
        )

        ordered = (
            csharp.index("actions/setup-dotnet@"),
            csharp.index("actions/download-artifact@"),
            csharp.index("Authenticate exact ABI22 C# privacy input"),
            csharp.index("Privacy C# SDK tests"),
        )
        self.assertEqual(ordered, tuple(sorted(ordered)))


if __name__ == "__main__":
    unittest.main()
