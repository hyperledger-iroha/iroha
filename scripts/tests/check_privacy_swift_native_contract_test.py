#!/usr/bin/env python3
"""Freeze the authenticated, no-skip ABI-22 Swift privacy lane."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


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


def load_python_module(relative: str, name: str):
    """Load one repository Python module for cross-contract assertions."""

    path = REPO_ROOT / relative
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise AssertionError(f"unable to load module: {relative}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def nul_delimited_sha256(values: list[str]) -> str:
    """Mirror Package.swift's ordered string-inventory digest."""

    digest = hashlib.sha256()
    for value in values:
        digest.update(value.encode("utf-8"))
        digest.update(b"\0")
    return digest.hexdigest()


def swift_sha256_constant(source: str, name: str) -> str:
    """Read one split-line SHA-256 literal from Package.swift."""

    match = re.search(
        rf'let {re.escape(name)} =\n\s+"([0-9a-f]{{64}})"',
        source,
    )
    if match is None:
        raise AssertionError(f"missing Swift SHA-256 constant: {name}")
    return match.group(1)


def swift_string_set_constant(source: str, name: str) -> set[str]:
    """Read one literal Set<String> inventory from Package.swift."""

    match = re.search(
        rf"let {re.escape(name)}: Set<String> = \[(.*?)\n\]",
        source,
        re.DOTALL,
    )
    if match is None:
        raise AssertionError(f"missing Swift string-set constant: {name}")
    return set(re.findall(r'"([^"\\]+)"', match.group(1)))


class PrivacySwiftNativeContractTests(unittest.TestCase):
    """Guard the release Swift tests against native capability skips."""

    def test_offline_device_registration_result_is_typed_through_swift_abi22(self) -> None:
        header = read("crates/connect_norito_bridge/include/connect_norito_bridge.h")
        native = read("IrohaSwift/Sources/IrohaSwift/NativeBridge.swift")
        bridge = read("IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift")
        model = read(
            "IrohaSwift/Sources/IrohaSwift/PrivacyExact12ActionModelsV1.swift"
        )
        torii = read("IrohaSwift/Sources/IrohaSwift/ToriiClient.swift")
        runner = read("ci/check_privacy_swift_sdk.sh")
        symbol = (
            "iroha_privacy_authenticated_offline_device_registration_"
            "result_project_v1"
        )
        self.assertIn(f"int32_t {symbol}(", header)
        self.assertIn(f'"{symbol}"', native)
        self.assertIn(
            "authenticatedOfflineDeviceRegistrationResultProjectV1(", native
        )
        self.assertIn(
            "projectAuthenticatedOfflineDeviceRegistrationResultV1(", bridge
        )
        self.assertIn(
            "public struct AuthenticatedOfflineDeviceRegistrationResultV1", model
        )
        for field in (
            '"terminal_state"',
            '"eligibility_outcome"',
            '"eligibility_reason"',
            '"matched_rule_ids"',
            '"rejection_code"',
            '"rejection_message"',
        ):
            self.assertIn(field, model)
        self.assertIn(
            "public func getAuthenticatedOfflineDeviceRegistrationResultV1(", torii
        )
        self.assertIn(
            "authenticatedOfflineDeviceRegistrationResultMaxBytes = 128 * 1024",
            native,
        )
        self.assertIn(
            "PrivacyExact12ActionModelsV1Tests.swift", runner
        )

    def test_authenticated_registration_transport_requires_exact_norito_media_type(self) -> None:
        torii = read("IrohaSwift/Sources/IrohaSwift/ToriiClient.swift")
        start = torii.index(
            "private func getAuthenticatedTransactionDetailsResponseV1("
        )
        end = torii.index(
            "private func getAuthenticatedPrivacyActionExecutionReceiptV1(", start
        )
        transport = torii[start:end]
        self.assertIn(
            'guard contentType == "application/x-norito" else', transport
        )
        self.assertIn(
            "must use exact application/x-norito without parameters", transport
        )
        self.assertNotIn("ensureResponseMediaType", transport)

    def test_exact12_inspector_binds_canonical_auth_authority_through_c_abi(self) -> None:
        header = read("crates/connect_norito_bridge/include/connect_norito_bridge.h")
        native = read("IrohaSwift/Sources/IrohaSwift/NativeBridge.swift")
        bridge = read("IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift")
        torii = read("IrohaSwift/Sources/IrohaSwift/ToriiClient.swift")

        declaration_start = header.index(
            "int32_t iroha_privacy_inspect_signed_exact12_action_v1("
        )
        declaration_end = header.index(");", declaration_start)
        declaration = header[declaration_start:declaration_end]
        self.assertIn("const uint8_t* authority_ptr", declaration)
        self.assertIn("unsigned long authority_len", declaration)
        self.assertIn("let authority = Data(authorityAccountId.utf8)", native)
        self.assertIn("CUnsignedLong(authority.count)", native)
        self.assertIn("authorityAccountId: authorityAccountId", bridge)
        self.assertIn("authorityAccountId: canonicalAuth.accountId", torii)

    def test_exact12_applied_requires_finalized_receipt_bridge_evidence(self) -> None:
        header = read("crates/connect_norito_bridge/include/connect_norito_bridge.h")
        native = read("IrohaSwift/Sources/IrohaSwift/NativeBridge.swift")
        bridge = read("IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift")
        model = read(
            "IrohaSwift/Sources/IrohaSwift/PrivacyExact12ActionModelsV1.swift"
        )
        torii = read("IrohaSwift/Sources/IrohaSwift/ToriiClient.swift")
        symbols = (
            "iroha_privacy_authenticated_action_receipt_prepare_v1",
            "iroha_privacy_authenticated_action_receipt_finalize_v1",
            "iroha_privacy_authenticated_action_receipt_project_result_v1",
        )
        for symbol in symbols:
            self.assertIn(f"int32_t {symbol}(", header)
            self.assertIn(f'"{symbol}"', native)
        for field in (
            "executionCapabilityManifestDigest",
            "executionCapabilityCommittedHeight",
            "executionReceiptFinalizedHeight",
            "executionReceiptFinalizedBlockHash",
        ):
            self.assertIn(f"public let {field}", model)
        for marker in (
            'case .queued, .approved, .committed:',
            'if status.resolvedFrom == "cache"',
            'path: "/v1/query"',
            'if response.statusCode == 404 { return nil }',
            "receipt.admittedAtHeight == details.committedBlockHeight",
            "executionCapabilityManifestDigest: receipt.capabilityManifestDigest",
        ):
            self.assertIn(marker, torii)
        self.assertLess(
            torii.index("guard let details, let receipt else"),
            torii.index("executionCapabilityManifestDigest: receipt.capabilityManifestDigest"),
        )

    def test_finalized_state_queries_use_one_closed_authenticated_native_abi(self) -> None:
        header = read("crates/connect_norito_bridge/include/connect_norito_bridge.h")
        rust = read(
            "crates/connect_norito_bridge/src/authenticated_privacy_state_query.rs"
        )
        native = read("IrohaSwift/Sources/IrohaSwift/NativeBridge.swift")
        bridge = read("IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift")
        models = read(
            "IrohaSwift/Sources/IrohaSwift/PrivacyFinalizedStateModelsV1.swift"
        )
        torii = read("IrohaSwift/Sources/IrohaSwift/ToriiClient.swift")
        symbols = (
            "iroha_privacy_authenticated_state_query_prepare_v1",
            "iroha_privacy_authenticated_state_query_finalize_v1",
            "iroha_privacy_authenticated_state_query_project_result_v1",
        )
        for symbol in symbols:
            self.assertIn(f"int32_t {symbol}(", header)
            self.assertIn(f'"{symbol}"', native)
        self.assertIn("match query_id {", rust)
        self.assertIn("97 => {", rust)
        self.assertIn("104 => {", rust)
        self.assertIn("if query_id != 98 && protocol_index != 0", rust)
        self.assertIn("canonical != response", rust)
        self.assertIn("view.validate()", rust)
        self.assertIn("authority: canonicalAuth.accountId", torii)
        self.assertIn("queryId: Request.queryId.rawValue", torii)
        self.assertIn('path: "/v1/query"', torii)
        self.assertIn('if response.statusCode == 404 { return nil }', torii)
        self.assertIn(
            'contentType == "application/x-norito"',
            torii,
        )
        for method in (
            "getPrivacyZkAceReplayNullifierV1",
            "getPrivacyProofManagedPoolStateV1",
            "getPrivacyOrchardPoolStateV1",
            "getPrivacyOrchardNullifierV1",
            "getPrivacyAnonymousPgcPoolStateV1",
            "getPrivacyZkAmsAdmissionV1",
            "getPrivacyZkAmsProvisionV1",
            "getPrivacyZkX509CertificateNullifierV1",
        ):
            self.assertIn(f"public func {method}(", torii)
        self.assertIn("case zkAceReplayNullifier = 97", models)
        self.assertIn("case zkX509CertificateNullifier = 104", models)
        self.assertIn("try? NetworkId(literal: value)", models)
        self.assertNotIn("PrivacyFinalizedHex32V1", models)
        for marker in (
            "prepareAuthenticatedPrivacyStateQueryV1",
            "finalizeAuthenticatedPrivacyStateQueryV1",
            "projectAuthenticatedPrivacyStateQueryResultV1",
        ):
            self.assertIn(marker, bridge)

    def test_swift_release_test_inventory_has_no_runtime_skip(self) -> None:
        test_roots = (
            REPO_ROOT / "IrohaSwift" / "Tests",
            REPO_ROOT / "IrohaSwift" / "KagemushaCandidateEvidenceLab" / "Tests",
            REPO_ROOT / "examples" / "ios" / "NoritoDemo" / "Tests",
        )
        test_sources = [
            path
            for root in test_roots
            for path in sorted(root.rglob("*.swift"))
        ]
        self.assertTrue(test_sources, "Swift release test inventory is empty")
        for path in test_sources:
            relative = path.relative_to(REPO_ROOT)
            self.assertNotIn("XCTSkip", path.read_text(encoding="utf-8"), relative)
        parity = read(
            "IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift"
        )
        self.assertIn(
            'throw ParityHarnessError.unzipFailed(\n'
            '            "unzip-based bridge materialization is unavailable outside macOS"',
            parity,
        )

    def test_swift_runner_reauthenticates_external_apple_artifact(self) -> None:
        source = read("ci/check_privacy_swift_sdk.sh")
        for marker in (
            '"$(uname -s)" != "Darwin"',
            '"${MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT:-}" != "1"',
            '"${MOBILE_SDK_REQUIRE_PRIVACY_PRODUCTION_APPLE_ARTIFACT:-}" != "1"',
            "MOBILE_SDK_APPLE_ARTIFACT_DIR",
            "MOBILE_SDK_SWIFT_SCRATCH_DIR",
            "MOBILE_SDK_PYTHON_BINARY",
            "scripts/check_mobile_sdk_artifacts.sh",
            '--apple-only',
            "SorafsOrchestratorParityTests.swift",
            "--manifest-cache none",
            "--disable-automatic-resolution",
            '--scratch-path "${SWIFT_SCRATCH_DIRECTORY}"',
        ):
            self.assertIn(marker, source)
        self.assertLess(
            source.index('bash "${APPLE_ARTIFACT_CHECKER}" --apple-only'),
            source.index('"${SWIFT_BIN}" test'),
        )
        lock_state = 'privacy_sdk_assert_ci_cargo_lock_state "${ROOT_DIR}" "${PYTHON_BIN}"'
        path_state = "privacy_sdk_assert_ci_executable_path_order"
        self.assertEqual(source.count(lock_state), 2)
        self.assertEqual(source.count(path_state), 2)
        self.assertNotIn("external-lock requalification", source)
        for invocation in (
            'DEVELOPER_DIR="$(xcode-select -p)"',
            "xcodebuild -version",
            'bash "${APPLE_ARTIFACT_CHECKER}" --apple-only',
            '"${SWIFTC_BIN}" --version',
            '"${SWIFT_BIN}" test',
        ):
            self.assertLess(source.index(lock_state), source.index(invocation))
            self.assertLess(source.index(invocation), source.rindex(lock_state))

    def test_swift_authenticated_lock_corridor_executes_and_rejects_drift(self) -> None:
        source = read("ci/check_privacy_swift_sdk.sh")
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary).resolve()
            root, artifact, scratch, tools = (
                base / name for name in ("repo", "artifact", "scratch", "bin")
            )
            for directory in (root / "scripts", root / "ci", artifact, scratch, tools):
                directory.mkdir(parents=True)
            (artifact / "NoritoBridge.xcframework").mkdir()
            tracked, release, log = root / "Cargo.lock", base / "Cargo.lock", base / "calls"
            tracked.write_text("tracked\n", encoding="utf-8")
            release.write_text("release\n", encoding="utf-8")
            fake_python = tools / "python"
            fake_python.write_text(
                "#!/usr/bin/env bash\n"
                f'[[ "${{!#}}" == "{tracked}" ]] && echo "179f589da420c024725efd9a65adb9c1e34085fa022cc01a8c67bb2262e93bf7" || echo "31b5af592c235ce7a24e9ea219ceaa5c2f74400b650c5121182425d93e39811d"\n',
                encoding="utf-8",
            )
            (tools / "uname").write_text("#!/usr/bin/env bash\necho Darwin\n", encoding="utf-8")
            tool_stub = (
                '#!/usr/bin/env bash\necho "${0##*/}" >>"$PRIVACY_TEST_LOG"\n'
                '[[ "${0##*/}" == xcode-select ]] && echo /Applications/Xcode.app/Contents/Developer\n'
                "exit 0\n"
            )
            for name in ("xcode-select", "xcodebuild", "swiftc", "swift"):
                (tools / name).write_text(tool_stub, encoding="utf-8")
            (root / "scripts/check_mobile_sdk_artifacts.sh").write_text(
                '#!/usr/bin/env bash\necho artifact-checker >>"$PRIVACY_TEST_LOG"\n',
                encoding="utf-8",
            )
            lock_helper = root / "ci/privacy_sdk_cargo_lockfile.sh"
            lock_helper.write_text(
                "privacy_sdk_assert_ci_cargo_lock_state() {\n"
                '  echo lock-state >>"$PRIVACY_TEST_LOG"\n'
                '  [[ "${PRIVACY_TEST_REJECT_LOCK:-0}" != "1" ]]\n'
                "}\n"
                "privacy_sdk_assert_ci_executable_path_order() {\n"
                '  echo path-state >>"$PRIVACY_TEST_LOG"\n'
                "}\n",
                encoding="utf-8",
            )
            for executable in (
                *tools.iterdir(),
                root / "scripts/check_mobile_sdk_artifacts.sh",
                lock_helper,
            ):
                executable.chmod(0o700)
            environment = {
                **os.environ,
                "PATH": f"{tools}:{os.environ['PATH']}",
                "PRIVACY_TEST_LOG": str(log),
                "PRIVACY_SWIFT_SDK_ROOT": str(root),
                "PRIVACY_SWIFT_SDK_SWIFTC_BIN": str(tools / "swiftc"),
                "PRIVACY_SWIFT_SDK_SWIFT_BIN": str(tools / "swift"),
                "MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT": "1",
                "MOBILE_SDK_REQUIRE_PRIVACY_PRODUCTION_APPLE_ARTIFACT": "1",
                "MOBILE_SDK_APPLE_ARTIFACT_DIR": str(artifact),
                "MOBILE_SDK_SWIFT_SCRATCH_DIR": str(scratch),
                "MOBILE_SDK_PYTHON_BINARY": str(fake_python),
                "IROHA_PRIVACY_AUTHENTICATED_CARGO_LOCKFILE_PATH": str(release),
                "IROHA_PRIVACY_CARGO_LOCKFILE_PATH": str(release),
            }
            gate = base / "gate.sh"
            gate.write_text(source, encoding="utf-8")
            result = subprocess.run(
                ["bash", str(gate)], env=environment, text=True, capture_output=True
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            calls = log.read_text(encoding="utf-8").splitlines()
            self.assertEqual(calls.count("lock-state"), 2)
            self.assertEqual(calls.count("path-state"), 2)
            self.assertIn("xcode-select", calls)
            self.assertIn("artifact-checker", calls)

            log.unlink()
            environment["MOBILE_SDK_REQUIRE_PRIVACY_PRODUCTION_APPLE_ARTIFACT"] = "0"
            result = subprocess.run(
                ["bash", str(gate)], env=environment, text=True, capture_output=True
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(
                "MOBILE_SDK_REQUIRE_PRIVACY_PRODUCTION_APPLE_ARTIFACT=1 is required",
                result.stderr,
            )
            self.assertFalse(log.exists())

            environment["MOBILE_SDK_REQUIRE_PRIVACY_PRODUCTION_APPLE_ARTIFACT"] = "1"
            environment["PRIVACY_TEST_REJECT_LOCK"] = "1"
            result = subprocess.run(
                ["bash", str(gate)], env=environment, text=True, capture_output=True
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(log.read_text(encoding="utf-8").splitlines(), ["lock-state"])

    def test_package_manifest_splits_external_and_privacy_release_gates(self) -> None:
        source = read("IrohaSwift/Package.swift")
        for marker in (
            '"MOBILE_SDK_APPLE_ARTIFACT_DIR"',
            '"MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT"',
            '"MOBILE_SDK_REQUIRE_PRIVACY_PRODUCTION_APPLE_ARTIFACT"',
            "configuredArtifactDirectory == nil",
            "if requirePrivacyProductionArtifact, !requireExternalArtifact",
            "must be outside the reviewed Iroha source tree",
            "requiredBridgeAbiVersion = 22",
            '"NoritoBridge.artifacts.json"',
            'manifest["native_bridge_abi_version"]',
            "requiredBridgeManifestFields",
            "requiredWorkspaceCargoLockSha256",
            "requiredPrivacyReleaseCargoLockSha256",
            "canonicalJSONBoolean(",
            "canonicalJSONInteger(",
            "hasNoDuplicateJSONMembers(manifestData)",
            "!requireExternalArtifact || hasNoDuplicateJSONMembers(manifestData)",
            "hasOnlyCanonicalJSONNumberTypes(manifest)",
            "checkedInBridgeSlicePins()",
            "private static let expectedHashes: \\[String: String\\]",
            "manifestHashes == checkedInPins",
            "validateReviewedReleaseBridgeArtifact(",
            "if requirePrivacyProductionArtifact,",
            "Set(environment.keys) == requiredBridgeBuildEnvironmentFields",
            "requiredPrivacyEnvironmentProfileAllowLists",
            "requiredBridgePrivacyEnvironmentProfilesSha256",
            "canonicalJSONSHA256(requiredPrivacyEnvironmentProfileAllowLists)",
            'canonicalJSONInteger(environment["cargo_build_jobs"]) == 1',
            'environment["rust_toolchain_channel"] as? String == "1.93.1"',
            'environment["cargo_release"] as? String == "1.93.1"',
            'environment["rustc_release"] as? String == "1.93.1"',
            'environment["rustdoc_release"] as? String == "1.93.1"',
            'environment["iphoneos_deployment_target"] as? String == "15.0"',
            'environment["iphonesimulator_deployment_target"] as? String == "15.0"',
            'environment["macosx_deployment_target"] as? String == "12.0"',
            "requiredBridgeRequiredSymbolsSha256",
            "requiredBridgeForbiddenSymbolsSha256",
            "requiredBridgeProductionRolesSha256",
            "nulDelimitedStringArraySHA256(requiredSymbols)",
            "canonicalJSONSHA256(artifactRoles)",
            "privacy-sdk-release-v2",
            "iroha.mobile-native-build-environment.v2",
            "reviewedSourceAbiIsExact(headerDigest)",
            "CONNECT_NORITO_BRIDGE_ABI_VERSION[ \\t]+22",
            "PRIVACY_BRIDGE_ABI_VERSION_V1: u32 = 22",
            "requiredBridgeTopLevelEntries",
            "requiredBridgeHeaderEntries",
            'metadata["CFBundlePackageType"] as? String == "XFWK"',
            'metadata["XCFrameworkFormatVersion"] as? String == "1.0"',
            "filesystemType(at: publicManifest) == .typeSymbolicLink",
            "exactDirectoryEntries(",
            "validateBridgeArtifact(at: bridgeAbsolutePath)",
        ):
            self.assertIn(marker, source)
        self.assertNotIn("Process(", source)

        validator = load_python_module(
            "scripts/validate_norito_bridge_xcframework.py",
            "privacy_swift_manifest_validator_contract",
        )
        for swift_name, expected in (
            ("requiredBridgeManifestFields", validator.EXPECTED_MANIFEST_FIELDS),
            (
                "requiredBridgeBuildEnvironmentFields",
                validator.EXPECTED_BUILD_ENVIRONMENT_FIELDS,
            ),
            ("requiredPrivacyBuildEnvironment", validator.PRIVACY_BUILD_ENVIRONMENT),
            ("requiredBridgeSliceIdentifiers", validator.EXPECTED_SLICES),
            ("requiredBridgeHeaderEntries", validator.EXPECTED_HEADER_ENTRIES),
        ):
            self.assertEqual(swift_string_set_constant(source, swift_name), set(expected))
        expected_profiles = json.dumps(
            validator.EXPECTED_PRIVACY_ENVIRONMENT_PROFILES,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("utf-8")
        self.assertEqual(
            swift_sha256_constant(
                source,
                "requiredBridgePrivacyEnvironmentProfilesSha256",
            ),
            hashlib.sha256(expected_profiles).hexdigest(),
        )
        self.assertEqual(
            swift_sha256_constant(source, "requiredBridgeRequiredSymbolsSha256"),
            nul_delimited_sha256(validator.EXPECTED_REQUIRED_SYMBOLS),
        )
        self.assertEqual(
            swift_sha256_constant(source, "requiredBridgeForbiddenSymbolsSha256"),
            nul_delimited_sha256(validator.EXPECTED_FORBIDDEN_SYMBOLS),
        )
        roles = json.dumps(
            validator.expected_kagemusha_roles(True),
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")
        self.assertEqual(
            swift_sha256_constant(source, "requiredBridgeProductionRolesSha256"),
            hashlib.sha256(roles).hexdigest(),
        )

    def test_cocoapods_bridge_lint_cannot_capability_skip(self) -> None:
        source = read("scripts/check_swift_pod_bridge.sh")
        wrapper = read("ci/check_swift_pod_bridge.sh")
        self.assertIn('[[ ! -x "${CHECK_SCRIPT}" ]]', wrapper)
        self.assertIn(
            'fail "cocoapods CLI not available; refusing to skip lint"',
            source,
        )
        self.assertIn(
            "cocoapods CLI not available; refusing to skip lint",
            source,
        )
        self.assertNotIn('write_summary "skipped"', source)
        self.assertNotIn("skipping lint", source)
        self.assertIn("MOBILE_SDK_PACKAGE_OUT_DIR", source)
        self.assertIn("render_norito_bridge_podspec.py", source)
        self.assertIn("packaged NoritoBridge archive authentication failed", source)
        self.assertIn("checksum inventory does not contain the exact Apple package set", source)
        self.assertIn(
            "package directory does not contain the exact five Apple files",
            source,
        )
        self.assertIn(
            'APPLE_MANIFEST="$PACKAGE_DIR/NoritoBridge-v${POD_VERSION}.artifacts.json"',
            source,
        )
        self.assertIn(
            "embedded NoritoBridge manifest version does not match pod SemVer",
            source,
        )
        self.assertIn('spec lint "$LOCAL_PODSPEC"', source)
        self.assertIn('lib lint "$PODSPEC_PATH"', source)
        self.assertIn('"--include-podspecs=$LOCAL_PODSPEC"', source)
        self.assertIn(
            "CocoaPods resolves --include-podspecs through :path",
            source,
        )
        self.assertIn('framework = stage / "NoritoBridge.xcframework"', source)
        self.assertIn('"--configuration=Release"', source)
        self.assertNotIn('"--allow-warnings"', source)
        self.assertNotIn('"--skip-tests"', source)
        self.assertLess(
            source.index('python3 -I -S -B "$RENDERER"'),
            source.index('run_lint "binary pod spec lint"'),
        )

        podspec = read("IrohaSwift/IrohaSwift.podspec")
        template = read("crates/connect_norito_bridge/NoritoBridge.podspec.template")
        self.assertIn("s.dependency       'NoritoBridge', version", podspec)
        self.assertIn(':tag => "v#{version}"', podspec)
        self.assertIn('version_bytes == "#{version}\\n"', podspec)
        self.assertIn(":sha256 => '__ARCHIVE_SHA256__'", template)
        self.assertIn("s.vendored_frameworks = 'NoritoBridge.xcframework'", template)
        for forbidden in ("prepare_command", "curl", "../dist"):
            self.assertNotIn(forbidden, podspec + template)

        workflow = read(".github/workflows/mobile_sdk_artifacts.yml")
        checker_job = workflow_job(workflow, "checker-self-test")
        apple_job = workflow_job(workflow, "apple-mobile-sdk")
        android_job = workflow_job(workflow, "android-mobile-sdk")
        self.assertIn('APPLE_PRIVACY_PRODUCTION_ENABLED: "false"', workflow)
        self.assertIn('ANDROID_PRIVACY_PRODUCTION_ENABLED: "false"', workflow)
        self.assertIn(
            "Bind reviewed Apple privacy mode",
            apple_job,
        )
        self.assertIn(
            'case "${APPLE_PRIVACY_PRODUCTION_ENABLED:-}" in',
            apple_job,
        )
        self.assertIn(
            "APPLE_PRIVACY_PRODUCTION_ENABLED must be exactly true or false",
            apple_job,
        )
        self.assertIn(
            'echo "PRIVACY_PRODUCTION_ENABLED=$APPLE_PRIVACY_PRODUCTION_ENABLED" >> "$GITHUB_ENV"',
            apple_job,
        )
        self.assertIn("true)\n              require_privacy_artifact=1", apple_job)
        self.assertIn("false)\n              require_privacy_artifact=0", apple_job)
        self.assertIn(
            'echo "MOBILE_SDK_REQUIRE_PRIVACY_PRODUCTION_APPLE_ARTIFACT=$require_privacy_artifact" >> "$GITHUB_ENV"',
            apple_job,
        )
        self.assertIn("swift test\n          --manifest-cache none", apple_job)
        self.assertIn(
            "Bind reviewed Android privacy mode",
            android_job,
        )
        self.assertIn(
            'case "${ANDROID_PRIVACY_PRODUCTION_ENABLED:-}" in',
            android_job,
        )
        self.assertIn(
            "ANDROID_PRIVACY_PRODUCTION_ENABLED must be exactly true or false",
            android_job,
        )
        self.assertIn(
            'echo "PRIVACY_PRODUCTION_ENABLED=$ANDROID_PRIVACY_PRODUCTION_ENABLED" >> "$GITHUB_ENV"',
            android_job,
        )
        self.assertNotIn(
            "PRIVACY_PRODUCTION_ENABLED: ${{ env.APPLE_PRIVACY_PRODUCTION_ENABLED }}",
            apple_job,
        )
        self.assertNotIn(
            "PRIVACY_PRODUCTION_ENABLED: ${{ env.ANDROID_PRIVACY_PRODUCTION_ENABLED }}",
            android_job,
        )
        self.assertNotIn("inputs.privacy_production_enabled", workflow)
        self.assertNotIn("github.ref_type == 'tag' ||", workflow)
        for trigger in (
            "ci/check_swift_pod_bridge.sh",
            "scripts/check_swift_pod_bridge.sh",
            "scripts/render_norito_bridge_podspec.py",
            "scripts/tests/render_norito_bridge_podspec_test.py",
        ):
            self.assertIn(f'      - "{trigger}"', workflow)
        self.assertIn("CocoaPods authenticated archive and source lint (no capability skip)", apple_job)
        self.assertIn(
            "SWIFT_POD_REPORT_DIR: ${{ runner.temp }}/iroha-swift-pod-report",
            apple_job,
        )
        self.assertIn("run: ci/check_swift_pod_bridge.sh", apple_job)
        self.assertIn(
            "Reject a noncanonical release tag before setup or build",
            checker_job,
        )
        self.assertIn(
            'version_bytes="$(wc -c < IrohaSwift/VERSION | tr -d \'[:space:]\')"',
            checker_job,
        )
        self.assertLess(
            checker_job.index("Reject a noncanonical release tag before setup or build"),
            checker_job.index("actions/setup-python@"),
        )
        tag_precedence = (
            'if [[ "${GITHUB_REF_TYPE}" == "tag" ]]; then\n'
            '            version="${GITHUB_REF_NAME}"\n'
            '          elif [[ -n "$input_version" ]]; then'
        )
        self.assertEqual(workflow.count(tag_precedence), 2)
        self.assertEqual(
            workflow.count(
                're.fullmatch(rb"(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)\\.'
                '(0|[1-9][0-9]*)\\n", raw)'
            ),
            4,
        )
        self.assertLess(
            apple_job.index("name: Package Apple mobile SDK artifact"),
            apple_job.index("name: CocoaPods authenticated archive and source lint"),
        )
        self.assertEqual(
            apple_job.count('cat > "$consumer/Package.swift" <<\'SWIFT\''),
            1,
        )
        self.assertNotIn("--clobber", workflow)
        for marker in (
            "github.repository == 'hyperledger-iroha/iroha'",
            "gh release create \"$GITHUB_REF_NAME\" --draft --verify-tag",
            "draft release already contains assets; refusing partial upload",
            "uploaded release asset inventory is incomplete",
            "downloaded release asset digest mismatch",
            'gh release edit "$GITHUB_REF_NAME" --draft=false',
        ):
            self.assertIn(marker, workflow)

    def test_release_guidance_distinguishes_source_wiring_from_publication(self) -> None:
        guide = read("docs/norito_bridge_release.md")
        readme = read("IrohaSwift/README.md")
        plan = read("specs/sorafs_reference_sdk_plan.md")
        for source in (guide, readme, plan):
            self.assertIn("CocoaPods", source)
            self.assertIn("vendored", source.lower())
        self.assertIn("This closes repository source wiring", guide)
        self.assertIn("CocoaPods registry publication remains blocked", guide)
        self.assertIn("checksum-pinned `NoritoBridge`", guide)
        self.assertIn("Generated `dist/*`", guide)
        self.assertIn("only `dist/.gitkeep` belongs in Git", guide)
        self.assertNotIn("swift package compute-checksum", guide)
        self.assertNotIn("Commit the generated artifacts", guide)
        for source in (
            guide,
            readme,
            plan,
            read("specs/sdk/swift/index.md"),
            read("ci/README.md"),
        ):
            self.assertNotIn("offline lint", source.lower())
            self.assertNotIn("offline consumer compilation", source.lower())

    def test_workflow_builds_authenticates_and_tests_exact_apple_artifact(self) -> None:
        source = read(".github/workflows/pr_privacy_sdk_guard.yml")
        job = workflow_job(source, "privacy_swift_sdk_parse")
        for trigger in (
            ".github/workflows/mobile_sdk_artifacts.yml",
            "ci/README.md",
            "ci/check_swift_pod_bridge.sh",
            "IrohaSwift/IrohaSwift.podspec",
            "IrohaSwift/Package.swift",
            "IrohaSwift/VERSION",
            "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift",
            "IrohaSwift/Tests/IrohaSwiftTests/NativeBridgeLoaderTests.swift",
            "scripts/tests/check_privacy_swift_native_contract_test.py",
            "scripts/check_swift_pod_bridge.sh",
            "scripts/archive_norito_xcframework.py",
            "scripts/build_norito_xcframework.sh",
            "scripts/check_mobile_sdk_artifact_pin_commit.py",
            "scripts/exec_with_file_lock.py",
            "scripts/norito_bridge_source_seal.py",
            "scripts/package_mobile_sdk_artifacts.sh",
            "scripts/run_mobile_hermetic_command.py",
            "scripts/render_norito_bridge_podspec.py",
            "scripts/tests/package_mobile_sdk_artifacts_test.py",
            "scripts/tests/render_norito_bridge_podspec_test.py",
            "scripts/tests/norito_bridge_source_seal_test.py",
            "scripts/tests/privacy_apple_cargo_wrapper_test.py",
            "scripts/update_norito_bridge_swift_pins.py",
            "scripts/validate_norito_bridge_xcframework.py",
            "crates/connect_norito_bridge/NoritoBridge.podspec.template",
            "crates/connect_norito_bridge/RELEASE_NOTES.md",
            "docs/norito_bridge_release.md",
            "specs/sdk/swift/index.md",
            "specs/sorafs_reference_sdk_plan.md",
        ):
            self.assertIn(f'      - "{trigger}"', source)
        for marker in (
            "runs-on: macos-14",
            "actions/setup-python@a26af69be951a213d495a4c3e4e4022e16d87065",
            'python-version: "3.12"',
            "update-environment: false",
            "MOBILE_SDK_PYTHON_BINARY",
            '"${HOME}/.cargo/bin/rustup" toolchain install',
            '"1.93.1-aarch64-apple-darwin"',
            "aarch64-apple-ios-sim",
            "x86_64-apple-darwin",
            "cargo fetch --locked",
            "MOBILE_SDK_APPLE_ARTIFACT_DIR",
            "MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT=1",
            "MOBILE_SDK_REQUIRE_PRIVACY_PRODUCTION_APPLE_ARTIFACT=1",
            "MOBILE_SDK_SWIFT_SCRATCH_DIR",
            "NORITO_BRIDGE_OUT_DIR",
            "NORITO_BRIDGE_BUILD_DIR",
            'chmod -R a-w "$GITHUB_WORKSPACE"',
            "scripts/build_norito_xcframework.sh --privacy-production-enabled",
            "scripts/check_mobile_sdk_artifacts.sh --apple-only",
            "python3 -I -B scripts/tests/check_privacy_swift_native_contract_test.py",
            '"$MOBILE_SDK_PYTHON_BINARY" -I -B scripts/tests/privacy_apple_cargo_wrapper_test.py',
            "run: ci/check_privacy_swift_sdk.sh",
        ):
            self.assertIn(marker, job)
        for forbidden in (
            "--allow-dirty-source",
            "NORITO_BRIDGE_TEST_PREBUILT_SLICES",
            "MOBILE_SDK_SKIP_BINARY_INSPECTION",
            "NORITO_BRIDGE_PRESERVE_CARGO_TARGETS",
        ):
            self.assertNotIn(forbidden, job)


if __name__ == "__main__":
    unittest.main()
