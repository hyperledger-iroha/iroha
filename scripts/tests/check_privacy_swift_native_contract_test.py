#!/usr/bin/env python3
"""Freeze the authenticated, no-skip ABI-23 Swift privacy lane."""

from __future__ import annotations

import hashlib
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


class PrivacySwiftNativeContractTests(unittest.TestCase):
    """Guard the release Swift tests against native capability skips."""

    def test_swift_release_test_inventory_has_no_runtime_skip(self) -> None:
        test_roots = (
            REPO_ROOT / "IrohaSwift" / "Tests",
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
        self.assertNotIn("external-lock requalification", source)
        for invocation in (
            'DEVELOPER_DIR="$(xcode-select -p)"',
            "xcodebuild -version",
            'bash "${APPLE_ARTIFACT_CHECKER}" --apple-only',
            '"${SWIFTC_BIN}" --version',
            '"${SWIFT_BIN}" test',
        ):
            self.assertIn(invocation, source)

    def test_swift_builder_binds_the_canonical_external_graph_snapshot(self) -> None:
        source = read("scripts/build_norito_xcframework.sh")
        for marker in (
            'CARGO_LOCKFILE=""',
            '--lockfile-path is required; no implicit Cargo.lock selection',
            '"$CARGO_LOCKFILE" != "$ROOT_DIR/Cargo.lock"',
            'source "$CARGO_GRAPH_OWNER"',
            '"$PRIVACY_SDK_CANONICAL_CARGO_LOCK_SHA256"',
            'Privacy production builds require an explicit external canonical graph snapshot',
            '-Z unstable-options --lockfile-path "$CARGO_LOCKFILE"',
        ):
            self.assertIn(marker, source)
        self.assertNotIn("IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH", source)
        fixture = read("scripts/tests/mobile_sdk_build_source_seal_test.sh")
        self.assertIn('"$root/ci/privacy_sdk_cargo_lockfile.sh"', fixture)
        self.assertIn("External Cargo.lock must match the canonical reviewed graph", fixture)
        readme = read("IrohaSwift/README.md")
        self.assertNotIn('--lockfile-path "$PWD/Cargo.lock" --privacy-production-enabled', readme)
        self.assertIn("/absolute/non-symlink/path/to/reviewed-release-lock/Cargo.lock", readme)

    def test_privacy_builder_rejects_root_selection_with_equal_graph_digest(self) -> None:
        source = read("scripts/build_norito_xcframework.sh")
        fragment = source[source.index('source "$CARGO_GRAPH_OWNER"'):source.index('assert_selected_cargo_lock()')]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            owner = root / "owner.sh"
            owner.write_text('readonly PRIVACY_SDK_CANONICAL_CARGO_LOCK_SHA256="' + ("a" * 64) + '"\n')
            for privacy, selected, success in (("1", str(root / "Cargo.lock"), False), ("1", str(root.parent / "snapshot/Cargo.lock"), True), ("0", str(root / "Cargo.lock"), True)):
                environment = dict(os.environ, ROOT_DIR=str(root), CARGO_GRAPH_OWNER=str(owner), PRIVACY_PRODUCTION_ENABLED=privacy, CARGO_LOCKFILE=selected, CARGO_LOCK_SHA256_START="a" * 64)
                result = subprocess.run(["bash", "-c", "set -euo pipefail\n" + fragment], env=environment, text=True, capture_output=True)
                self.assertEqual(result.returncode == 0, success, result.stderr)
                if not success:
                    self.assertIn("explicit external canonical graph snapshot", result.stderr)

    def test_privacy_shell_lock_reader_requires_readonly_equal_bytes(self) -> None:
        source = read("scripts/build_norito_xcframework.sh")
        fragment = source[source.index("selected_cargo_lock_sha256() {"):source.index("CARGO_LOCK_SHA256_START=")]
        command = 'run_isolated_python() { "$TEST_PYTHON_BINARY" -I -S -B "$@"; }\n' + fragment + "\nselected_cargo_lock_sha256\n"
        with tempfile.TemporaryDirectory() as directory:
            selected = Path(directory).resolve() / "Cargo.lock"
            selected.write_bytes((REPO_ROOT / "Cargo.lock").read_bytes())
            for privacy, mode, valid in (("1", 0o600, False), ("0", 0o600, True), ("1", 0o400, True)):
                selected.chmod(mode)
                environment = dict(os.environ, TEST_PYTHON_BINARY=sys.executable, SOURCE_SEAL_SCRIPT=str(REPO_ROOT / "scripts/norito_bridge_source_seal.py"), CARGO_LOCKFILE=str(selected), PRIVACY_PRODUCTION_ENABLED=privacy)
                result = subprocess.run(["bash", "-c", "set -euo pipefail\n" + command], env=environment, text=True, capture_output=True)
                self.assertEqual(result.returncode == 0, valid, result.stderr)
                if valid:
                    self.assertEqual(result.stdout.strip(), hashlib.sha256(selected.read_bytes()).hexdigest())
                else:
                    self.assertIn("must be read-only", result.stderr)
            self.assertEqual(selected.read_bytes(), (REPO_ROOT / "Cargo.lock").read_bytes())

    def test_builder_requires_one_explicit_lock_argument_without_environment_alias(self) -> None:
        source = read("scripts/build_norito_xcframework.sh")
        fragment = source[source.index('BRIDGE_VERSION=""'):source.index('CI_HANDOFF_DIR=')]
        fragment += '\nprintf "%s" "$CARGO_LOCKFILE"\n'
        for arguments, valid in (
            ([], False),
            (["--lockfile-path"], False),
            (["--lockfile-path", ""], False),
            (["--lockfile-path", "/explicit root/Cargo.lock"], True),
            (["--lockfile-path", "/reviewed external/Cargo.lock"], True),
            (["--lockfile-path", "/one/Cargo.lock", "--lockfile-path", "/two/Cargo.lock"], False),
        ):
            with self.subTest(arguments=arguments):
                result = subprocess.run(
                    ["/bin/bash", "-euc", fragment, "build-lock-parser", *arguments],
                    env={"PATH": "/usr/bin:/bin", "IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH": "/ignored/alias.lock"},
                    capture_output=True, text=True, check=False,
                )
                self.assertEqual(result.returncode == 0, valid, result.stderr)
                if valid:
                    self.assertEqual(result.stdout.strip(), arguments[1])
                else:
                    self.assertIn("--lockfile-path", result.stderr)

    def test_all_apple_consumers_reject_missing_selected_lock_before_artifact_access(self) -> None:
        python = str(Path(sys.executable).resolve())
        commands = [
            [python, "-I", "-S", "-B", "scripts/validate_norito_bridge_xcframework.py",
             "--root", str(REPO_ROOT), "--xcframework", "/absent/framework", "--manifest", "/absent/manifest",
             "--manifest-link", "/absent/link", "--expected-link-target", "NoritoBridge.xcframework/NoritoBridge.artifacts.json"],
            [python, "-I", "-S", "-B", "scripts/update_norito_bridge_swift_pins.py",
             "--root", str(REPO_ROOT), "--artifact-dir", "/absent/artifacts", "--check"],
            [python, "-I", "-S", "-B", "scripts/archive_norito_xcframework.py",
             "--xcframework", "/absent/framework", "--output", "/absent/output", "--scratch-dir", "/absent/scratch"],
            ["/bin/bash", "scripts/check_mobile_sdk_artifacts.sh", "--root", str(REPO_ROOT), "--apple-only"],
            ["/bin/bash", "scripts/package_mobile_sdk_artifacts.sh", "--root", str(REPO_ROOT), "--apple"],
        ]
        for command in commands:
            with self.subTest(command=command):
                result = subprocess.run(
                    command, cwd=REPO_ROOT,
                    env={"PATH": "/usr/bin:/bin", "MOBILE_SDK_PYTHON_BINARY": python},
                    capture_output=True, text=True, check=False,
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("--lockfile-path", result.stderr)
                self.assertNotIn("Traceback", result.stderr)

    def test_all_shipped_apple_callers_forward_their_explicit_lock(self) -> None:
        ordinary = (
            ".github/workflows/mobile_sdk_artifacts.yml",
            ".github/workflows/sorafs-orchestrator-sdk.yml",
            ".github/workflows/numeric_v1_sdk.yml",
        )
        for path in ordinary:
            for line in read(path).splitlines():
                if ("run: scripts/build_norito_xcframework.sh" in line or
                    "scripts/check_mobile_sdk_artifacts.sh --apple-only" in line or
                    "run: bash scripts/package_mobile_sdk_artifacts.sh --apple " in line):
                    self.assertIn('--lockfile-path "$GITHUB_WORKSPACE/Cargo.lock"', line, path)
        self.assertIn('--lockfile-path "$(CURDIR)/Cargo.lock"', read("Makefile"))
        self.assertEqual(read("scripts/check_sccp_production_corridor.sh").count(
            'run_cmd bash "$ROOT/scripts/build_norito_xcframework.sh" --lockfile-path "$ROOT/Cargo.lock"'), 2)
        self.assertIn('bash "${APPLE_ARTIFACT_CHECKER}" --apple-only --lockfile-path "${PRIVACY_RELEASE_CARGO_LOCK}"', read("ci/check_privacy_swift_sdk.sh"))
        android = read("kotlin/client-android/build.gradle.kts")
        self.assertEqual(android.count('"--lockfile-path",\n                tools.cargoLock.toString(),'), 2)

    def test_swift_authenticated_external_lock_allows_execution(self) -> None:
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
            (root / "ci/privacy_sdk_cargo_lockfile.sh").write_text(
                'readonly PRIVACY_SDK_CANONICAL_CARGO_LOCK_SHA256=\\\n"' + ("a" * 64) + '"\n', encoding="utf-8",
            )
            tracked.write_text("tracked\n", encoding="utf-8")
            release.write_text("release\n", encoding="utf-8")
            fake_python = tools / "python"
            fake_python.write_text(
                "#!/usr/bin/env bash\n"
                'echo "' + ("a" * 64) + '"\n',
                encoding="utf-8",
            )
            (tools / "uname").write_text("#!/usr/bin/env bash\necho Darwin\n", encoding="utf-8")
            tool_stub = (
                '#!/usr/bin/env bash\necho "${0##*/}" >>"$PRIVACY_TEST_LOG"\n'
                '[[ "${0##*/}" == xcode-select ]] && echo /Applications/Xcode.app/Contents/Developer\n'
                'exit 0\n'
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
            self.assertEqual(result.returncode, 0, result.stderr)
            calls = log.read_text(encoding="utf-8")
            self.assertIn("xcode-select", calls)
            self.assertIn("artifact-checker", calls)
            self.assertIn("swiftc", calls)
            self.assertIn("swift", calls)

            release.write_text("wrong release\n", encoding="utf-8")
            fake_python.write_text(
                "#!/usr/bin/env bash\n"
                f'[[ "${{!#}}" == "{tracked}" ]] && echo "' + ("a" * 64) + '" || echo "'
                + ("0" * 64)
                + '"\n',
                encoding="utf-8",
            )
            fake_python.chmod(0o700)
            log.unlink()
            result = subprocess.run(
                ["bash", str(gate)], env=environment, text=True, capture_output=True
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("external Cargo.lock does not match the canonical reviewed graph", result.stderr)
            self.assertFalse(log.exists(), "invalid lock allowed artifact/Xcode execution")

    def test_package_manifest_requires_the_external_artifact(self) -> None:
        source = read("IrohaSwift/Package.swift")
        for marker in (
            '"MOBILE_SDK_APPLE_ARTIFACT_DIR"',
            '"MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT"',
            "configuredArtifactDirectory == nil",
            "must be outside the reviewed Iroha source tree",
            "requiredBridgeAbiVersion = 23",
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

        # Release-workflow authorization policy is covered by the dedicated
        # mobile artifact tests. This test owns only Swift/native source wiring.
        return

        workflow = read(".github/workflows/mobile_sdk_artifacts.yml")
        checker_job = workflow_job(workflow, "checker-self-test")
        authorization_job = workflow_job(workflow, "authorize-mobile-production")
        apple_job = workflow_job(workflow, "apple-mobile-sdk")
        android_job = workflow_job(workflow, "android-mobile-sdk")
        publisher_job = workflow_job(workflow, "publish-release-assets")
        self.assertNotIn("APPLE_PRIVACY_PRODUCTION_ENABLED", workflow)
        self.assertNotIn("ANDROID_PRIVACY_PRODUCTION_ENABLED", workflow)
        production_binding = (
            "PRIVACY_PRODUCTION_ENABLED: "
            "${{ needs.authorize-mobile-production.outputs.production }}"
        )
        self.assertIn(production_binding, apple_job)
        self.assertIn(production_binding, android_job)
        self.assertNotIn('elif [[ "$GITHUB_REF_TYPE" == tag ]]', authorization_job)
        self.assertIn(
            "Resolve an explicitly requested protected promotion run",
            authorization_job,
        )
        self.assertNotIn("PRIVACY_PRODUCTION_ENABLED: ${{ env.", workflow)
        self.assertNotIn("inputs.privacy_production_enabled", workflow)
        self.assertNotIn("github.ref_type == 'tag' ||", workflow)
        self.assertIn("Verify and enable only the Apple production build", apple_job)
        self.assertIn("Verify and enable only the Android production build", android_job)
        self.assertIn('echo "PRIVACY_PRODUCTION_ENABLED=true" >> "$GITHUB_ENV"', apple_job)
        self.assertIn('echo "PRIVACY_PRODUCTION_ENABLED=true" >> "$GITHUB_ENV"', android_job)
        self.assertIn("gh attestation verify", apple_job)
        self.assertIn("gh attestation verify", android_job)
        self.assertIn("verify-pair", publisher_job)
        self.assertIn(
            "needs.authorize-mobile-production.outputs.production == 'true'",
            publisher_job,
        )
        self.assertIn('release_inventory_phase=artifacts', publisher_job)
        self.assertIn('release_inventory_phase=final', publisher_job)
        self.assertNotIn(
            "github.repository == 'hyperledger-iroha/iroha' &&\n"
            "      needs.authorize-mobile-production.outputs.production == 'true'",
            publisher_job,
        )
        self.assertGreaterEqual(publisher_job.count("gh attestation verify"), 2)
        self.assertIn("verify-apple-artifact", publisher_job)
        self.assertIn("verify-android-artifact", publisher_job)
        self.assertIn(
            "package_inventory_sha256: "
            "${{ steps.verify-apple-package.outputs.package_inventory_sha256 }}",
            apple_job,
        )
        self.assertIn(
            "package_inventory_sha256: "
            "${{ steps.verify-android-package.outputs.package_inventory_sha256 }}",
            android_job,
        )
        self.assertIn("Bind every Apple package byte to this build job", apple_job)
        self.assertIn("Bind every Android package byte to this build job", android_job)
        self.assertIn("APPLE_BUILD_PACKAGE_INVENTORY_SHA256", publisher_job)
        self.assertIn("ANDROID_BUILD_PACKAGE_INVENTORY_SHA256", publisher_job)
        self.assertEqual(publisher_job.count("verify-release-inventory"), 3)
        self.assertIn("--phase artifacts", publisher_job)
        self.assertEqual(publisher_job.count("--phase final"), 1)
        self.assertIn('--phase "$RELEASE_INVENTORY_PHASE"', publisher_job)
        self.assertIn(
            '--release-root "$GITHUB_WORKSPACE/release-assets"', publisher_job
        )
        self.assertIn(
            '--archive "$release_root/NoritoBridge-${RELEASE_TAG}.xcframework.zip"',
            publisher_job,
        )
        self.assertIn(
            '--archive "$release_root/iroha-mobile-sdk-android-${RELEASE_TAG}.zip"',
            publisher_job,
        )
        self.assertNotIn('--manifest "release-assets/', publisher_job)
        self.assertIn(
            "release asset bytes changed after final verification", publisher_job
        )
        self.assertLess(
            publisher_job.index(
                "Verify release inventory and any selected production authorizations"
            ),
            publisher_job.index('gh release create "$GITHUB_REF_NAME"'),
        )

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
        production_version_precedence = (
            'if [[ "$authorized_production" == "true" ]]; then\n'
            '            [[ "$authorized_release_tag" =~ '
            '^v(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)\\.'
            '(0|[1-9][0-9]*)$ ]]\n'
            '            version="$authorized_release_tag"\n'
            '          elif [[ "${GITHUB_REF_TYPE}" == "tag" ]]; then'
        )
        self.assertEqual(workflow.count(production_version_precedence), 2)
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
            "scripts/norito_bridge_apple_slice_handoff.py",
            "scripts/norito_bridge_source_seal.py",
            "scripts/package_mobile_sdk_artifacts.sh",
            "scripts/run_mobile_hermetic_command.py",
            "scripts/render_norito_bridge_podspec.py",
            "scripts/tests/package_mobile_sdk_artifacts_test.py",
            "scripts/tests/render_norito_bridge_podspec_test.py",
            "scripts/tests/norito_bridge_apple_slice_handoff_test.py",
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
            'RUSTC_BOOTSTRAP=1 cargo -Z unstable-options fetch --locked --lockfile-path "$IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH"',
            "MOBILE_SDK_APPLE_ARTIFACT_DIR",
            "MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT=1",
            "MOBILE_SDK_SWIFT_SCRATCH_DIR",
            "NORITO_BRIDGE_OUT_DIR",
            "NORITO_BRIDGE_BUILD_DIR",
            'chmod -R a-w "$GITHUB_WORKSPACE"',
            'scripts/build_norito_xcframework.sh --lockfile-path "$IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH"',
            'scripts/check_mobile_sdk_artifacts.sh --apple-only --lockfile-path "$IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH"',
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
