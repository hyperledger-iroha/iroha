#!/usr/bin/env python3
"""Freeze the authenticated, no-skip ABI-22 Swift privacy lane."""

from __future__ import annotations

import os
import re
import subprocess
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


class PrivacySwiftNativeContractTests(unittest.TestCase):
    """Guard the release Swift tests against native capability skips."""

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
            "MOBILE_SDK_APPLE_ARTIFACT_DIR",
            "MOBILE_SDK_SWIFT_SCRATCH_DIR",
            "MOBILE_SDK_PYTHON_BINARY",
            "scripts/check_mobile_sdk_artifacts.sh",
            '--apple-only',
            "SorafsOrchestratorParityTests.swift",
            "--disable-automatic-resolution",
            '--scratch-path "${SWIFT_SCRATCH_DIRECTORY}"',
        ):
            self.assertIn(marker, source)
        self.assertLess(
            source.index('bash "${APPLE_ARTIFACT_CHECKER}" --apple-only'),
            source.index('"${SWIFT_BIN}" test'),
        )
        blocker = "external-lock requalification"
        self.assertIn(blocker, source)
        for invocation in (
            'DEVELOPER_DIR="$(xcode-select -p)"',
            "xcodebuild -version",
            'bash "${APPLE_ARTIFACT_CHECKER}" --apple-only',
            '"${SWIFTC_BIN}" --version',
            '"${SWIFT_BIN}" test',
        ):
            self.assertLess(source.index(blocker), source.index(invocation))

    def test_swift_requalification_blocker_stops_direct_execution(self) -> None:
        source = read("ci/check_privacy_swift_sdk.sh")
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary).resolve()
            root, artifact, scratch, tools = (
                base / name for name in ("repo", "artifact", "scratch", "bin")
            )
            for directory in (root / "scripts", artifact, scratch, tools):
                directory.mkdir(parents=True)
            (artifact / "NoritoBridge.xcframework").mkdir()
            tracked, release, log = root / "Cargo.lock", base / "Cargo.lock", base / "calls"
            tracked.write_text("tracked\n", encoding="utf-8")
            release.write_text("release\n", encoding="utf-8")
            fake_python = tools / "python"
            fake_python.write_text(
                "#!/usr/bin/env bash\n"
                f'[[ "${{!#}}" == "{tracked}" ]] && echo "0ddb3f3938cf32035371317100674cd1601c3cb41232237f7a7d28b3aeab6222" || echo "cd9e829e454171f17540abeb7fd1aa14129252082bd8b076a0199b0ffa4e3f79"\n',
                encoding="utf-8",
            )
            (tools / "uname").write_text("#!/usr/bin/env bash\necho Darwin\n", encoding="utf-8")
            tool_stub = (
                '#!/usr/bin/env bash\necho "${0##*/}" >>"$PRIVACY_TEST_LOG"\n'
                '[[ "${0##*/}" == xcode-select ]] && echo /Applications/Xcode.app/Contents/Developer\n'
            )
            for name in ("xcode-select", "xcodebuild", "swiftc", "swift"):
                (tools / name).write_text(tool_stub, encoding="utf-8")
            (root / "scripts/check_mobile_sdk_artifacts.sh").write_text(
                '#!/usr/bin/env bash\necho artifact-checker >>"$PRIVACY_TEST_LOG"\n',
                encoding="utf-8",
            )
            for executable in (*tools.iterdir(), root / "scripts/check_mobile_sdk_artifacts.sh"):
                executable.chmod(0o700)
            environment = {
                **os.environ,
                "PATH": f"{tools}:{os.environ['PATH']}",
                "PRIVACY_TEST_LOG": str(log),
                "PRIVACY_SWIFT_SDK_ROOT": str(root),
                "PRIVACY_SWIFT_SDK_SWIFTC_BIN": str(tools / "swiftc"),
                "PRIVACY_SWIFT_SDK_SWIFT_BIN": str(tools / "swift"),
                "MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT": "1",
                "MOBILE_SDK_APPLE_ARTIFACT_DIR": str(artifact),
                "MOBILE_SDK_SWIFT_SCRATCH_DIR": str(scratch),
                "MOBILE_SDK_PYTHON_BINARY": str(fake_python),
                "IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH": str(release),
            }
            gate = base / "gate.sh"
            gate.write_text(source, encoding="utf-8")
            result = subprocess.run(
                ["bash", str(gate)], env=environment, text=True, capture_output=True
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("external-lock requalification", result.stderr)
            self.assertFalse(log.exists(), "blocker allowed artifact/Xcode execution")

            marker = source.index("external-lock requalification")
            exit_at = source.index("exit 1", marker)
            gate.write_text(source[:exit_at] + ": # negative control" + source[exit_at + 6 :], encoding="utf-8")
            result = subprocess.run(
                ["bash", str(gate)], env=environment, text=True, capture_output=True
            )
            self.assertIn("xcode-select", log.read_text(encoding="utf-8"))

    def test_package_manifest_requires_the_external_artifact(self) -> None:
        source = read("IrohaSwift/Package.swift")
        for marker in (
            '"MOBILE_SDK_APPLE_ARTIFACT_DIR"',
            '"MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT"',
            "configuredArtifactDirectory == nil",
            "must be outside the reviewed Iroha source tree",
            "requiredBridgeAbiVersion = 22",
            '"NoritoBridge.artifacts.json"',
            'manifest["native_bridge_abi_version"]',
            "validateBridgeArtifact(at: bridgeAbsolutePath)",
        ):
            self.assertIn(marker, source)

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
            "MOBILE_SDK_SWIFT_SCRATCH_DIR",
            "NORITO_BRIDGE_OUT_DIR",
            "NORITO_BRIDGE_BUILD_DIR",
            'chmod -R a-w "$GITHUB_WORKSPACE"',
            "scripts/build_norito_xcframework.sh",
            "scripts/check_mobile_sdk_artifacts.sh --apple-only",
            "python3 -I -B scripts/tests/check_privacy_swift_native_contract_test.py",
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
