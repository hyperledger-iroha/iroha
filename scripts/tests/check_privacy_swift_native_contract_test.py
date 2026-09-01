#!/usr/bin/env python3
"""Freeze the authenticated, no-skip ABI-22 Swift privacy lane."""

from __future__ import annotations

import os
import re
import shutil
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
            "IROHA_KOTLIN_OFFLINE_CASH_FIXTURE_BIN",
            "authoritative Offline Cash fixture must be an absolute regular executable",
            "authoritative Offline Cash fixture must remain outside the source tree",
            "authoritative Offline Cash fixture cannot be identity-sealed",
            "privacy_sdk_executable_seal",
            "privacy_sdk_assert_executable_seal",
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
        self.assertNotIn("external-lock requalification", source)
        authenticated = "privacy Swift external Cargo.lock is not the frozen release lock"
        self.assertIn(authenticated, source)
        for invocation in (
            'if [[ -z "${DEVELOPER_DIR:-}" ]]; then',
            'DEVELOPER_DIR="$(xcode-select -p)"',
            "export DEVELOPER_DIR",
            '"${NORITO_BRIDGE_DEVELOPER_DIR}" != "${DEVELOPER_DIR}"',
            "privacy Swift Xcode does not match the authenticated Apple artifact toolchain",
            "xcodebuild -version",
            'bash "${APPLE_ARTIFACT_CHECKER}" --apple-only',
            '"${SWIFTC_BIN}" --version',
            '"${SWIFT_BIN}" test',
        ):
            self.assertLess(source.index(authenticated), source.index(invocation))

    def test_swift_external_lock_requalification_runs_fail_closed(self) -> None:
        source = read("ci/check_privacy_swift_sdk.sh")
        tracked_digest = re.search(
            r'TRACKED_ROOT_CARGO_LOCK_SHA256="([0-9a-f]{64})"', source
        )
        release_digest = re.search(
            r'FROZEN_CARGO_LOCK_SHA256="([0-9a-f]{64})"', source
        )
        self.assertIsNotNone(tracked_digest)
        self.assertIsNotNone(release_digest)
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary).resolve()
            root, artifact, scratch, tools = (
                base / name for name in ("repo", "artifact", "scratch", "bin")
            )
            for directory in (root / "scripts", root / "ci", artifact, scratch, tools):
                directory.mkdir(parents=True)
            (artifact / "NoritoBridge.xcframework").mkdir()
            tracked, release, fixture, log = (
                root / "Cargo.lock",
                base / "Cargo.lock",
                base / "kotlin_offline_cash_v1",
                base / "calls",
            )
            tracked.write_text("tracked\n", encoding="utf-8")
            release.write_text("release\n", encoding="utf-8")
            fixture.write_text("fixture\n", encoding="utf-8")
            fake_python = tools / "python"
            fake_python.write_text(
                "#!/usr/bin/env bash\n"
                "last=${!#}\n"
                "if [[ \" $* \" != *\" -S \"* ]]; then\n"
                f'  if [[ "$last" == "{tracked}" ]]; then echo tracked-seal; '
                f'elif [[ "$last" == "{release}" ]]; then '
                f'grep -qx release "{release}" && echo release-seal || echo changed-release-seal; '
                f'elif [[ "$last" == "{fixture}" ]]; then '
                f'grep -qx fixture "{fixture}" && echo fixture-seal || echo changed-fixture-seal; '
                "else echo unknown-seal; fi\n"
                f'elif [[ "$last" == "{tracked}" ]]; then\n'
                f'  [[ "${{PRIVACY_TEST_BAD_DIGEST:-}}" == tracked ]] && echo "{'0' * 64}" || echo "{tracked_digest.group(1)}"\n'
                "else\n"
                f'  [[ "${{PRIVACY_TEST_BAD_DIGEST:-}}" == release ]] && echo "{'0' * 64}" || echo "{release_digest.group(1)}"\n'
                "fi\n",
                encoding="utf-8",
            )
            (tools / "uname").write_text("#!/usr/bin/env bash\necho Darwin\n", encoding="utf-8")
            tool_stub = (
                '#!/usr/bin/env bash\necho "${0##*/}" >>"$PRIVACY_TEST_LOG"\n'
                '[[ "${0##*/}" == xcode-select ]] && echo /Applications/Xcode.app/Contents/Developer\n'
                'if [[ "${0##*/}" == swift && "${PRIVACY_TEST_MUTATE_RELEASE:-0}" == 1 ]]; then\n'
                '  printf "mutated\\n" >"$IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH"\n'
                "fi\n"
                'if [[ "${0##*/}" == swift && "${PRIVACY_TEST_MUTATE_FIXTURE:-0}" == 1 ]]; then\n'
                '  printf "mutated\\n" >"$IROHA_KOTLIN_OFFLINE_CASH_FIXTURE_BIN"\n'
                "fi\n"
                "exit 0\n"
            )
            for name in ("xcode-select", "xcodebuild", "swiftc", "swift"):
                (tools / name).write_text(tool_stub, encoding="utf-8")
            (root / "scripts/check_mobile_sdk_artifacts.sh").write_text(
                '#!/usr/bin/env bash\necho artifact-checker >>"$PRIVACY_TEST_LOG"\n',
                encoding="utf-8",
            )
            for executable in (*tools.iterdir(), root / "scripts/check_mobile_sdk_artifacts.sh"):
                executable.chmod(0o700)
            fixture.chmod(0o700)
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
                "IROHA_KOTLIN_OFFLINE_CASH_FIXTURE_BIN": str(fixture),
            }
            environment.pop("DEVELOPER_DIR", None)
            environment.pop("NORITO_BRIDGE_DEVELOPER_DIR", None)
            shutil.copy2(
                REPO_ROOT / "ci/privacy_sdk_cargo_lockfile.sh",
                root / "ci/privacy_sdk_cargo_lockfile.sh",
            )
            gate = root / "ci/check_privacy_swift_sdk.sh"
            gate.write_text(source, encoding="utf-8")
            result = subprocess.run(
                ["bash", str(gate)], env=environment, text=True, capture_output=True
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            calls = log.read_text(encoding="utf-8")
            for invocation in ("xcode-select", "xcodebuild", "artifact-checker", "swiftc", "swift"):
                self.assertIn(invocation, calls)

            log.unlink()
            pinned_environment = {
                **environment,
                "DEVELOPER_DIR": "/Applications/Xcode_26.6.app/Contents/Developer",
                "NORITO_BRIDGE_DEVELOPER_DIR": "/Applications/Xcode_26.6.app/Contents/Developer",
            }
            pinned = subprocess.run(
                ["bash", str(gate)],
                env=pinned_environment,
                text=True,
                capture_output=True,
            )
            self.assertEqual(pinned.returncode, 0, pinned.stderr)
            pinned_calls = log.read_text(encoding="utf-8")
            self.assertNotIn("xcode-select", pinned_calls)
            for invocation in ("xcodebuild", "artifact-checker", "swiftc", "swift"):
                self.assertIn(invocation, pinned_calls)

            log.unlink()
            mismatched_environment = {
                **pinned_environment,
                "NORITO_BRIDGE_DEVELOPER_DIR": "/Applications/Xcode_25.app/Contents/Developer",
            }
            rejected = subprocess.run(
                ["bash", str(gate)],
                env=mismatched_environment,
                text=True,
                capture_output=True,
            )
            self.assertEqual(rejected.returncode, 1)
            self.assertIn(
                "does not match the authenticated Apple artifact toolchain",
                rejected.stderr,
            )
            self.assertFalse(log.exists(), "mismatched Xcode allowed tool execution")

            log.unlink(missing_ok=True)
            missing_fixture_environment = dict(environment)
            missing_fixture_environment.pop(
                "IROHA_KOTLIN_OFFLINE_CASH_FIXTURE_BIN"
            )
            rejected = subprocess.run(
                ["bash", str(gate)],
                env=missing_fixture_environment,
                text=True,
                capture_output=True,
            )
            self.assertEqual(rejected.returncode, 1)
            self.assertIn(
                "Offline Cash fixture paths are required",
                rejected.stderr,
            )
            self.assertFalse(log.exists(), "missing fixture allowed tool execution")

            for bad_digest, expected_error in (
                ("tracked", "tracked root Cargo.lock authority changed"),
                ("release", "external Cargo.lock is not the frozen release lock"),
            ):
                log.unlink(missing_ok=True)
                rejected = subprocess.run(
                    ["bash", str(gate)],
                    env={**environment, "PRIVACY_TEST_BAD_DIGEST": bad_digest},
                    text=True,
                    capture_output=True,
                )
                self.assertEqual(rejected.returncode, 1)
                self.assertIn(expected_error, rejected.stderr)
                self.assertFalse(log.exists(), "digest failure allowed tool execution")

            release.write_text("release\n", encoding="utf-8")
            log.unlink(missing_ok=True)
            rejected = subprocess.run(
                ["bash", str(gate)],
                env={**environment, "PRIVACY_TEST_MUTATE_RELEASE": "1"},
                text=True,
                capture_output=True,
            )
            self.assertEqual(rejected.returncode, 1)
            self.assertIn(
                "privacy Swift external Cargo.lock changed",
                rejected.stderr,
            )
            self.assertIn("swift", log.read_text(encoding="utf-8"))

            release.write_text("release\n", encoding="utf-8")
            fixture.write_text("fixture\n", encoding="utf-8")
            fixture.chmod(0o700)
            log.unlink(missing_ok=True)
            rejected = subprocess.run(
                ["bash", str(gate)],
                env={**environment, "PRIVACY_TEST_MUTATE_FIXTURE": "1"},
                text=True,
                capture_output=True,
            )
            self.assertEqual(rejected.returncode, 1)
            self.assertIn(
                "authoritative Offline Cash fixture changed",
                rejected.stderr,
            )
            self.assertIn("swift", log.read_text(encoding="utf-8"))

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

    def test_apple_artifact_tools_bind_an_explicit_privacy_release_lock(self) -> None:
        for relative in (
            "scripts/build_norito_xcframework.sh",
            "scripts/check_mobile_sdk_artifacts.sh",
        ):
            source = read(relative)
            self.assertIn(
                '${IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH+x}', source, relative
            )
            self.assertIn(
                "IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH must not be empty",
                source,
                relative,
            )
            self.assertIn('$ROOT_DIR/Cargo.lock', source, relative)
        builder = read("scripts/build_norito_xcframework.sh")
        checker = read("scripts/check_mobile_sdk_artifacts.sh")
        updater = builder[builder.index(
            '"$ROOT_DIR/scripts/update_norito_bridge_swift_pins.py"'
        ) : builder.index('run_isolated_python - \\\n', builder.index(
            '"$ROOT_DIR/scripts/update_norito_bridge_swift_pins.py"'
        ))]
        validator = builder[builder.index(
            '"$ROOT_DIR/scripts/validate_norito_bridge_xcframework.py"'
        ) : builder.index('assert_bridge_source_seal "staged artifact validation"')]
        archive = builder[builder.index(
            '"$PYTHON_BINARY" -I -S -B "$ARCHIVE_OWNER"'
        ) : builder.index('assert_bridge_source_seal "the archive publication"')]
        self.assertIn('--lockfile-path "$CARGO_LOCKFILE"', updater)
        self.assertIn('--lockfile-path "$CARGO_LOCKFILE"', validator)
        self.assertIn('--lockfile-path "$CARGO_LOCKFILE"', archive)
        self.assertIn('--lockfile-path "$APPLE_CARGO_LOCKFILE"', checker)

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
        self.assertIn("Build authoritative Offline Cash Swift fixture", workflow)
        self.assertIn('- "ci/build_offline_cash_swift_fixture.sh"', workflow)
        self.assertIn('- "ci/xcode-swift-parity"', workflow)
        self.assertIn('- "scripts/dev_workflow.sh"', workflow)
        self.assertIn("build_offline_cash_swift_fixture.sh --locked --offline", workflow)
        self.assertIn(
            'echo "IROHA_KOTLIN_OFFLINE_CASH_FIXTURE_BIN=$fixture" >> "$GITHUB_ENV"',
            workflow,
        )
        self.assertLess(
            apple_job.index("build_offline_cash_swift_fixture.sh"),
            apple_job.index("swift test"),
        )
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
            5,
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

    def test_workflow_produces_authenticated_slices_then_assembles_and_tests(self) -> None:
        source = read(".github/workflows/pr_privacy_sdk_guard.yml")
        slice_job = workflow_job(source, "privacy_swift_sdk_slice")
        assembly_job = workflow_job(source, "privacy_swift_sdk_parse")
        self.assertIn("permissions:\n  contents: read", source)
        self.assertIn('APPLE_PRIVACY_PRODUCTION_ENABLED: "false"', source)
        self.assertIn("Build authoritative Offline Cash Swift fixture", assembly_job)
        self.assertIn("build_offline_cash_swift_fixture.sh", assembly_job)
        self.assertIn("--lockfile-path", assembly_job)
        self.assertIn("IROHA_KOTLIN_OFFLINE_CASH_FIXTURE_BIN=$fixture", assembly_job)
        self.assertLess(
            assembly_job.index("build_offline_cash_swift_fixture.sh"),
            assembly_job.index("ci/check_privacy_swift_sdk.sh"),
        )
        for trigger in (
            ".github/workflows/mobile_sdk_artifacts.yml",
            "ci/build_offline_cash_swift_fixture.sh",
            "ci/xcode-swift-parity",
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
            "scripts/dev_workflow.sh",
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
        for job in (slice_job, assembly_job):
            for marker in (
                "runs-on: macos-26",
                "timeout-minutes: 180",
                "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
                "persist-credentials: false",
                "actions/setup-python@ece7cb06caefa5fff74198d8649806c4678c61a1",
                'python-version: "3.12"',
                "update-environment: false",
                "DEVELOPER_DIR: /Applications/Xcode_26.6.app/Contents/Developer",
                "NORITO_BRIDGE_DEVELOPER_DIR: /Applications/Xcode_26.6.app/Contents/Developer",
                "NORITO_BRIDGE_SLICE_BUILD_ID: ${{ github.run_id }}.${{ github.run_attempt }}",
                "Bind reviewed Apple privacy mode",
                'case "${APPLE_PRIVACY_PRODUCTION_ENABLED:-}" in',
                "APPLE_PRIVACY_PRODUCTION_ENABLED must be exactly true or false",
                'echo "PRIVACY_PRODUCTION_ENABLED=$APPLE_PRIVACY_PRODUCTION_ENABLED" >> "$GITHUB_ENV"',
                "Require the exact Xcode 26.6 release toolchain",
                "Xcode 26.6\\nBuild version 17F113",
                "unexpected DEVELOPER_DIR",
                "bridge and job Xcode identities differ",
                "unable to query Xcode identity",
                "unexpected Xcode identity",
                "MOBILE_SDK_PYTHON_BINARY",
                '"${HOME}/.cargo/bin/rustup" toolchain install',
                '"1.93.1-aarch64-apple-darwin"',
                "aarch64-apple-ios-sim",
                "x86_64-apple-darwin",
                "cargo fetch --locked",
                "privacy-jvm-native-abi22-${{ github.sha }}",
                "IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH",
                "SOURCE_DATE_EPOCH",
                '[[ "$(git rev-parse HEAD)" == "$GITHUB_SHA" ]]',
                "MOBILE_SDK_APPLE_ARTIFACT_DIR",
                "NORITO_BRIDGE_OUT_DIR",
                "NORITO_BRIDGE_BUILD_DIR",
                'chmod -R a-w "$GITHUB_WORKSPACE"',
                "scripts/build_norito_xcframework.sh",
                'build_args+=(--privacy-production-enabled)',
            ):
                self.assertIn(marker, job)
            for forbidden in (
                "--allow-dirty-source",
                "NORITO_BRIDGE_TEST_PREBUILT_SLICES",
                "MOBILE_SDK_SKIP_BINARY_INSPECTION",
                "NORITO_BRIDGE_PRESERVE_CARGO_TARGETS",
            ):
                self.assertNotIn(forbidden, job)
            self.assertLess(
                job.index("Require the exact Xcode 26.6 release toolchain"),
                job.index("scripts/build_norito_xcframework.sh"),
            )
            self.assertLess(
                job.index('chmod -R a-w "$GITHUB_WORKSPACE"'),
                job.index("scripts/build_norito_xcframework.sh"),
            )
            self.assertEqual(job.count("exit 1; }"), 4)
            self.assertEqual(job.count("persist-credentials: false"), 1)
            self.assertEqual(job.count("scripts/build_norito_xcframework.sh"), 1)

        self.assertIn("needs: privacy_jvm_sdk_tests", slice_job)
        self.assertIn("fail-fast: false", slice_job)
        self.assertIn("max-parallel: 5", slice_job)
        for slice_id in (
            "ios-arm64",
            "ios-sim-arm64",
            "ios-sim-x64",
            "macos-arm64",
            "macos-x64",
        ):
            self.assertEqual(slice_job.count(f"          - {slice_id}\n"), 1)
        self.assertIn('--produce-slice "${{ matrix.slice }}"', slice_job)
        self.assertIn(
            '--slice-output-root "$NORITO_BRIDGE_SLICE_OUTPUT_ROOT"',
            slice_job,
        )
        self.assertNotIn("--assemble-slices", slice_job)
        self.assertIn("actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02", slice_job)
        self.assertIn(
            "name: privacy-swift-apple-slice-${{ github.run_id }}-${{ github.run_attempt }}-${{ matrix.slice }}",
            slice_job,
        )
        self.assertIn(
            "path: ${{ runner.temp }}/iroha-privacy-swift-slice-output/${{ matrix.slice }}/*",
            slice_job,
        )
        self.assertIn("Revalidate frozen privacy Swift slice inputs", slice_job)
        for job in (slice_job, assembly_job):
            self.assertNotIn("nohup", job)
            self.assertIsNone(re.search(r"(?<![>&])&(?![>&])", job))

        self.assertIn("needs: privacy_swift_sdk_slice", assembly_job)
        self.assertIn('--assemble-slices "$NORITO_BRIDGE_SLICE_INPUT_ROOT"', assembly_job)
        self.assertNotIn("--produce-slice", assembly_job)
        self.assertNotIn("run: scripts/build_norito_xcframework.sh", assembly_job)
        self.assertEqual(
            assembly_job.count(
                "actions/download-artifact@d3f86a106a0bac45b974a628896c90dbdf5c8093"
            ),
            6,
        )
        for slice_id in (
            "ios-arm64",
            "ios-sim-arm64",
            "ios-sim-x64",
            "macos-arm64",
            "macos-x64",
        ):
            self.assertIn(
                f"name: privacy-swift-apple-slice-${{{{ github.run_id }}}}-${{{{ github.run_attempt }}}}-{slice_id}",
                assembly_job,
            )
            self.assertIn(
                f"path: ${{{{ runner.temp }}}}/iroha-privacy-swift-slices/{slice_id}",
                assembly_job,
            )
        for marker in (
            "MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT=1",
            "MOBILE_SDK_SWIFT_SCRATCH_DIR",
            "NORITO_BRIDGE_SLICE_INPUT_ROOT",
            '"$MOBILE_SDK_PYTHON_BINARY" -I -S -B scripts/tests/check_privacy_swift_native_contract_test.py',
            "Build authoritative Offline Cash Swift fixture",
            "run: ci/check_privacy_swift_sdk.sh",
            "Revalidate frozen Swift inputs and ABI22 artifacts",
            "scripts/check_mobile_sdk_artifacts.sh --apple-only",
            "NoritoBridge.xcframework/.privacy-production-enabled",
        ):
            self.assertIn(marker, assembly_job)
        background_mutation = assembly_job.replace(
            '            "${build_args[@]}"\n',
            '            "${build_args[@]}" & wait\n',
            1,
        )
        self.assertNotEqual(background_mutation, assembly_job)
        self.assertIsNotNone(re.search(r"(?<![>&])&(?![>&])", background_mutation))
        self.assertLess(
            assembly_job.index("Download iOS arm64 privacy Swift slice"),
            assembly_job.index("--assemble-slices"),
        )


if __name__ == "__main__":
    unittest.main()
