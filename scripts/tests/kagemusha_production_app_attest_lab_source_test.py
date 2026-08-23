"""Contract tests for the standalone production App Attest iPhone lab."""

from __future__ import annotations

import base64
import hashlib
import os
import pathlib
import plistlib
import stat
import subprocess
import sys
import tempfile
import unittest
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPT_DIR = ROOT / "scripts"
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import kagemusha_candidate_ios_evidence as candidate_evidence  # noqa: E402
import kagemusha_production_app_attest_capture as capture_evidence  # noqa: E402
import kagemusha_production_ios_evidence as production_evidence  # noqa: E402


APP = (
    ROOT
    / "IrohaSwift/KagemushaProductionAppAttestLab/App/"
    "LabHostApp.swift"
)
ENTITLEMENTS = (
    ROOT
    / "IrohaSwift/KagemushaProductionAppAttestLab/App/"
    "KagemushaProductionAppAttestLab.entitlements"
)
PROJECT = ROOT / "IrohaSwift/KagemushaProductionAppAttestLab/project.yml"
CANDIDATE_PROJECT = ROOT / "IrohaSwift/KagemushaCandidateEvidenceLab/project.yml"
RUNNER = ROOT / "scripts/run_kagemusha_production_app_attest_lab.sh"
MEASURER = ROOT / "scripts/measure_kagemusha_production_app_attest_bundle.py"
CUSTODY_TEST_PARENT = pathlib.Path(
    os.environ.get("TMPDIR", str(pathlib.Path.home()))
    if sys.platform == "darwin"
    else pathlib.Path.home()
).resolve()


def custody_helper_source() -> str:
    """Extract the runner's executable filesystem-custody helper."""

    source = RUNNER.read_text(encoding="utf-8")
    begin_marker = "# KAGEMUSHA_CUSTODY_HELPER_BEGIN\n"
    end_marker = "# KAGEMUSHA_CUSTODY_HELPER_END\n"
    begin = source.index(begin_marker) + len(begin_marker)
    end = source.index(end_marker, begin)
    return source[begin:end]


def run_custody(helper: pathlib.Path, *arguments: object) -> subprocess.CompletedProcess[str]:
    """Run one isolated custody-helper command and retain its diagnostic."""

    return subprocess.run(
        [sys.executable, "-I", str(helper), *(str(item) for item in arguments)],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
        text=True,
        encoding="utf-8",
    )


def cbor(value: Any) -> bytes:
    """Encode the small definite-length CBOR subset needed by this test."""

    def header(major: int, argument: int) -> bytes:
        if argument < 24:
            return bytes([(major << 5) | argument])
        if argument <= 0xFF:
            return bytes([(major << 5) | 24, argument])
        if argument <= 0xFFFF:
            return bytes([(major << 5) | 25]) + argument.to_bytes(2, "big")
        raise ValueError("test CBOR argument exceeds its bound")

    if isinstance(value, int) and not isinstance(value, bool):
        return header(0, value) if value >= 0 else header(1, -1 - value)
    if isinstance(value, bytes):
        return header(2, len(value)) + value
    if isinstance(value, str):
        payload = value.encode("ascii")
        return header(3, len(payload)) + payload
    if isinstance(value, (list, tuple)):
        return header(4, len(value)) + b"".join(cbor(item) for item in value)
    if isinstance(value, dict):
        return header(5, len(value)) + b"".join(
            cbor(key) + cbor(item) for key, item in value.items()
        )
    raise TypeError(f"unsupported CBOR fixture type: {type(value)!r}")


class ProductionAppAttestLabSourceTest(unittest.TestCase):
    def test_production_entitlement_is_explicit_and_physical_only(self) -> None:
        entitlements = ENTITLEMENTS.read_text(encoding="utf-8")
        project = PROJECT.read_text(encoding="utf-8")
        self.assertIn(
            "com.apple.developer.devicecheck.appattest-environment", entitlements
        )
        self.assertIn("<string>production</string>", entitlements)
        self.assertNotIn("<string>development</string>", entitlements)
        self.assertIn("SUPPORTED_PLATFORMS: iphoneos", project)
        self.assertIn("SUPPORTS_MACCATALYST: NO", project)
        self.assertIn("INFOPLIST_KEY_UIRequiresFullScreen: YES", project)

    def test_benchmark_and_capture_apps_share_the_policy_bound_app_id(self) -> None:
        expected = (
            "PRODUCT_BUNDLE_IDENTIFIER: "
            "org.hyperledger.iroha.kagemusha.appattestlab"
        )
        self.assertIn(expected, PROJECT.read_text(encoding="utf-8"))
        self.assertIn(expected, CANDIDATE_PROJECT.read_text(encoding="utf-8"))
        self.assertIn(
            'PRODUCTION_BUNDLE_ID="org.hyperledger.iroha.kagemusha.appattestlab"',
            RUNNER.read_text(encoding="utf-8"),
        )

    def test_app_calls_the_three_real_apis_in_order(self) -> None:
        source = APP.read_text(encoding="utf-8")
        calls = (
            source.index("let keyID = try await generateKey(service)"),
            source.index("let attestationObject = try await attest("),
            source.index("let assertionObject = try await assertKey("),
        )
        self.assertEqual(calls, tuple(sorted(calls)))
        self.assertIn("service.generateKey", source)
        self.assertIn("service.attestKey", source)
        self.assertIn("service.generateAssertion", source)
        self.assertIn("service.isSupported", source)
        self.assertIn('"requested_environment": "production"', source)

    def test_runner_checks_signed_entitlement_and_profile_before_install(self) -> None:
        source = RUNNER.read_text(encoding="utf-8")
        signed_check = source.index('SIGNED_ENVIRONMENT" != "production"')
        profile_check = source.index(
            "profile App Attest entitlement does not authorize production"
        )
        install = source.index("devicectl device install app")
        self.assertLess(signed_check, install)
        self.assertLess(profile_check, install)
        self.assertIn("No Accounts", source)
        self.assertIn("-allowProvisioningUpdates", source)
        self.assertIn("-allowProvisioningDeviceRegistration", source)
        self.assertIn(
            r"-extract 'com\.apple\.developer\.devicecheck\.appattest-environment' raw",
            source,
        )
        self.assertIn(
            r"-extract 'com\.apple\.developer\.team-identifier' raw", source
        )
        self.assertNotIn(
            "-extract com.apple.developer.devicecheck.appattest-environment raw",
            source,
        )
        self.assertNotIn(
            "-extract com.apple.developer.team-identifier raw", source
        )
        self.assertNotIn("device_udid_sha256", source)
        self.assertNotIn("device_serial_sha256", source)

    def test_profile_authorization_accepts_apple_scalar_or_bounded_array(self) -> None:
        source = RUNNER.read_text(encoding="utf-8")
        marker = "# PROFILE_APP_ATTEST_AUTHORIZATION_V1\n"
        embedded = source.split(marker, 1)[1].split("<<'PY'\n", 1)[1]
        embedded = embedded.split("\nPY\n", 1)[0]

        accepted = (
            "production",
            ["production"],
            ["development", "production"],
            ["production", "development"],
        )
        rejected = (
            "development",
            [],
            ["development"],
            ["production", "production"],
            ["development", "production", "future"],
            ["production", 1],
            {"production": True},
        )
        with tempfile.TemporaryDirectory(prefix="kagemusha-profile-") as temporary:
            profile_path = pathlib.Path(temporary) / "profile.plist"
            for environment in (*accepted, *rejected):
                with profile_path.open("wb") as destination:
                    plistlib.dump(
                        {
                            "Entitlements": {
                                "com.apple.developer.devicecheck."
                                "appattest-environment": environment
                            }
                        },
                        destination,
                    )
                result = subprocess.run(
                    [sys.executable, "-I", "-", str(profile_path)],
                    input=embedded,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    check=False,
                    text=True,
                    encoding="utf-8",
                )
                if environment in accepted:
                    self.assertEqual(result.returncode, 0, result.stderr)
                else:
                    self.assertNotEqual(result.returncode, 0, environment)

    def test_capture_filename_is_bound_to_exact_request_digest(self) -> None:
        runner = RUNNER.read_text(encoding="utf-8")
        app = APP.read_text(encoding="utf-8")
        self.assertIn('hashlib.sha256(payload).hexdigest()', runner)
        self.assertIn('capture-$REQUEST_SHA256.json', runner)
        self.assertNotIn('--source "Documents/kagemusha-production-app-attest/capture-v1.json"', runner)
        self.assertIn('hex(sha256(data))', app)
        self.assertIn('capture-\\(requestDigest).json', app)

    def test_release_capture_uses_the_exact_prepared_signed_app(self) -> None:
        runner = RUNNER.read_text(encoding="utf-8")
        self.assertIn("a production request requires an independently prepared signed app", runner)
        self.assertIn("capture-app-code-sign-measurements-v1.json", runner)
        self.assertIn('cmp -s "$CAPTURE_APP_MEASUREMENTS" "$PREPARED_MEASUREMENTS"', runner)
        self.assertIn("signed capture app changed during the physical run", runner)
        self.assertIn("--capture-app-code-sign-measurements", runner)
        self.assertIn('PLATFORM_OUTPUT="$EVIDENCE_ROOT/platform-evidence-v1.json"', runner)
        self.assertIn(
            'PLATFORM_OUTPUT="$EVIDENCE_ROOT/qualification-material-v1.json"',
            runner,
        )
        measurer = MEASURER.read_text(encoding="utf-8")
        self.assertIn("codesign omitted an exact identity field", measurer)
        self.assertIn("executable_sha256", measurer)
        self.assertIn("app_attest_environment", measurer)

    def test_runner_shell_and_embedded_python_parse(self) -> None:
        subprocess.run(["/bin/bash", "-n", str(RUNNER)], check=True, cwd=ROOT)
        compile(MEASURER.read_text(encoding="utf-8"), str(MEASURER), "exec")
        source = RUNNER.read_text(encoding="utf-8")
        blocks = source.split("<<'PY'\n")[1:]
        self.assertGreaterEqual(len(blocks), 5)
        for index, remainder in enumerate(blocks):
            block, separator, _ = remainder.partition("\nPY\n")
            self.assertTrue(separator, f"missing heredoc terminator for block {index}")
            compile(block, f"{RUNNER.name}:heredoc:{index}", "exec")

    def test_runner_custody_gates_surround_copy_install_capture_and_publish(self) -> None:
        source = RUNNER.read_text(encoding="utf-8")
        prepared_snapshot = source.index(
            'custody snapshot-prepared "$PREPARED_ROOT" "$PREPARED_TREE_SNAPSHOT"'
        )
        prepared_copy = source.index(
            '"$DITTO_BINARY" --noqtn "$PREPARED_APP" "$APP"'
        )
        preinstall_gate = source.index(
            "# Final identity gate immediately before the exact signed executable"
        )
        install = source.index("devicectl device install app")
        capture_copy = source.index(
            'custody copy-exclusive "$DEVICE_CAPTURE" "$CAPTURE"'
        )
        post_capture_gate = source.index(
            "# The physical capture is accepted only if every host-side object"
        )
        validator = source.index(
            '"$ROOT_DIR/scripts/check_kagemusha_production_app_attest_capture.py"'
        )
        publication = source.index('custody copy-exclusive "$PLATFORM_OUTPUT_TRANSIENT"')
        self.assertLess(prepared_snapshot, prepared_copy)
        self.assertLess(prepared_copy, preinstall_gate)
        self.assertLess(preinstall_gate, install)
        self.assertLess(install, capture_copy)
        self.assertLess(capture_copy, post_capture_gate)
        self.assertLess(post_capture_gate, validator)
        self.assertLess(validator, publication)
        for required in (
            "canonical transient parent must be euid-owned with exact mode 0700",
            "ancestor is not owned by root or the effective uid",
            "private custody path must have exact mode 0700",
            "extended ACLs are forbidden",
            "O_NOFOLLOW",
            "O_EXCL",
            "st_nlink != 1",
            "renamex_np",
            "prepared app tree identity or contents changed",
            'custody revalidate-parent "$TRANSIENT"',
            'custody revalidate-tree "$APP" "$CAPTURE_APP_TREE_SNAPSHOT"',
        ):
            self.assertIn(required, source)
        self.assertLess(
            source.index('TRANSIENT_PARENT="$($PYTHON3_BINARY'),
            source.index('TRANSIENT="$(mktemp -d'),
        )

    def test_custody_helper_rejects_non_private_immediate_parent(self) -> None:
        with tempfile.TemporaryDirectory(
            prefix=".kagemusha-custody-test-", dir=CUSTODY_TEST_PARENT
        ) as temporary:
            root = pathlib.Path(temporary)
            helper = root / "custody.py"
            helper.write_text(custody_helper_source(), encoding="utf-8")
            unsafe_parent = root / "unsafe-parent"
            unsafe_parent.mkdir(mode=0o700)
            unsafe_parent.chmod(0o750)
            result = run_custody(
                helper,
                "snapshot-parent",
                unsafe_parent / "evidence",
                root / "parent-state.json",
                "absent",
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("exact mode 0700", result.stderr)

    def test_custody_helper_detects_parent_inode_substitution(self) -> None:
        with tempfile.TemporaryDirectory(
            prefix=".kagemusha-custody-test-", dir=CUSTODY_TEST_PARENT
        ) as temporary:
            root = pathlib.Path(temporary)
            helper = root / "custody.py"
            helper.write_text(custody_helper_source(), encoding="utf-8")
            parent = root / "private-parent"
            parent.mkdir(mode=0o700)
            target = parent / "evidence"
            snapshot = root / "parent-state.json"
            initial = run_custody(
                helper, "snapshot-parent", target, snapshot, "absent"
            )
            self.assertEqual(initial.returncode, 0, initial.stderr)
            displaced = root / "displaced-parent"
            parent.rename(displaced)
            parent.mkdir(mode=0o700)
            result = run_custody(
                helper, "revalidate-parent", target, snapshot, "absent"
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("parent identity changed", result.stderr)

    def test_custody_helper_rejects_hardlinks_in_prepared_app(self) -> None:
        with tempfile.TemporaryDirectory(
            prefix=".kagemusha-custody-test-", dir=CUSTODY_TEST_PARENT
        ) as temporary:
            root = pathlib.Path(temporary)
            helper = root / "custody.py"
            helper.write_text(custody_helper_source(), encoding="utf-8")
            prepared = root / "prepared"
            prepared.mkdir(mode=0o700)
            app = prepared / "KagemushaProductionAppAttestLab.app"
            app.mkdir(mode=0o755)
            executable = app / "KagemushaProductionAppAttestLab"
            executable.write_bytes(b"signed-app-placeholder")
            executable.chmod(0o700)
            os.link(executable, app / "hardlink-substitution")
            measurement = prepared / "capture-app-code-sign-measurements-v1.json"
            measurement.write_text("{}\n", encoding="ascii")
            measurement.chmod(0o600)
            result = run_custody(
                helper,
                "snapshot-prepared",
                prepared,
                root / "prepared-state.json",
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("singly-linked regular file", result.stderr)

    def test_custody_helper_creates_once_and_preserves_existing_output(self) -> None:
        with tempfile.TemporaryDirectory(
            prefix=".kagemusha-custody-test-", dir=CUSTODY_TEST_PARENT
        ) as temporary:
            root = pathlib.Path(temporary)
            helper = root / "custody.py"
            helper.write_text(custody_helper_source(), encoding="utf-8")
            parent = root / "private-parent"
            parent.mkdir(mode=0o700)
            target = parent / "evidence"
            parent_state = root / "parent-state.json"
            root_state = root / "root-state.json"
            snapshotted = run_custody(
                helper, "snapshot-parent", target, parent_state, "absent"
            )
            self.assertEqual(snapshotted.returncode, 0, snapshotted.stderr)
            created = run_custody(
                helper, "create-root", target, parent_state, root_state
            )
            self.assertEqual(created.returncode, 0, created.stderr)
            self.assertEqual(stat.S_IMODE(target.stat().st_mode), 0o700)
            source = root / "source.json"
            source.write_bytes(b'{"trusted":true}\n')
            destination = target / "evidence.json"
            first = run_custody(helper, "copy-exclusive", source, destination)
            self.assertEqual(first.returncode, 0, first.stderr)
            source.write_bytes(b'{"trusted":false}\n')
            second = run_custody(helper, "copy-exclusive", source, destination)
            self.assertNotEqual(second.returncode, 0)
            self.assertIn("refusing to overwrite or follow", second.stderr)
            self.assertEqual(destination.read_bytes(), b'{"trusted":true}\n')

    @unittest.skipUnless(os.geteuid() == 0, "foreign-owner mutation needs root")
    def test_custody_helper_rejects_foreign_owned_immediate_parent(self) -> None:
        with tempfile.TemporaryDirectory(
            prefix=".kagemusha-custody-test-", dir=CUSTODY_TEST_PARENT
        ) as temporary:
            root = pathlib.Path(temporary)
            helper = root / "custody.py"
            helper.write_text(custody_helper_source(), encoding="utf-8")
            foreign_parent = root / "foreign-parent"
            foreign_parent.mkdir(mode=0o700)
            original_gid = foreign_parent.stat().st_gid
            try:
                os.chown(foreign_parent, 1, original_gid)
                result = run_custody(
                    helper,
                    "snapshot-parent",
                    foreign_parent / "evidence",
                    root / "foreign-state.json",
                    "absent",
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("not owned by root or the effective uid", result.stderr)
            finally:
                os.chown(foreign_parent, os.geteuid(), original_gid)
                foreign_parent.chmod(stat.S_IRWXU)

    def test_extracts_attested_cose_public_key(self) -> None:
        x_coordinate = bytes(range(32))
        y_coordinate = bytes(range(32, 64))
        key_id = b"k" * 32
        cose = {1: 2, 3: -7, -1: 1, -2: x_coordinate, -3: y_coordinate}
        auth_data = b"\0" * 53 + len(key_id).to_bytes(2, "big") + key_id + cbor(cose)
        attestation = cbor(
            {
                "fmt": "apple-appattest",
                "attStmt": {"x5c": [b"leaf", b"intermediate"], "receipt": b"receipt"},
                "authData": auth_data,
            }
        )
        public_key, chain = capture_evidence._extract_attested_public_key(
            attestation, production_evidence
        )
        self.assertEqual(public_key, b"\x04" + x_coordinate + y_coordinate)
        self.assertEqual(chain, (b"leaf", b"intermediate"))

    def test_release_bound_challenge_keeps_its_exact_evaluation_time(self) -> None:
        policy = {"policy_id": "physical-production-policy"}
        policy_payload = candidate_evidence.canonical_json_bytes(policy)
        policy_sha256 = hashlib.sha256(policy_payload).hexdigest()
        release_manifest_sha256 = hashlib.sha256(b"release-manifest").hexdigest()
        capture_measurements_sha256 = hashlib.sha256(
            b"prepared-capture-app-measurements"
        ).hexdigest()
        artifact_digests = {
            artifact: {"sha256": hashlib.sha256(field.encode("ascii")).hexdigest()}
            for field, artifact in production_evidence.ARTIFACT_CHALLENGE_BINDINGS.items()
        }
        evaluated_at = 1_800_000_000_000
        attestation_object = b"bounded-real-object-placeholder"
        key_id = base64.b64encode(b"k" * 32).decode("ascii")
        attestation = production_evidence._challenge_bindings(
            artifact_digests,
            schema=production_evidence.ATTESTATION_CHALLENGE_SCHEMA,
            domain=production_evidence.ATTESTATION_CHALLENGE_DOMAIN,
            policy_id=policy["policy_id"],
            policy_sha256=policy_sha256,
            release_manifest_sha256=release_manifest_sha256,
            capture_app_code_sign_measurements_sha256=capture_measurements_sha256,
            evaluated_at_unix_ms=evaluated_at,
            nonce_base64=base64.b64encode(b"a" * 32).decode("ascii"),
        )
        assertion = production_evidence._challenge_bindings(
            artifact_digests,
            schema=production_evidence.ASSERTION_CHALLENGE_SCHEMA,
            domain=production_evidence.ASSERTION_CHALLENGE_DOMAIN,
            policy_id=policy["policy_id"],
            policy_sha256=policy_sha256,
            release_manifest_sha256=release_manifest_sha256,
            capture_app_code_sign_measurements_sha256=capture_measurements_sha256,
            evaluated_at_unix_ms=evaluated_at,
            nonce_base64=base64.b64encode(b"b" * 32).decode("ascii"),
        )
        assertion.update(
            {
                "attestation_object_sha256": hashlib.sha256(
                    attestation_object
                ).hexdigest(),
                "key_id": key_id,
            }
        )
        errors: list[str] = []
        kind, observed_time = capture_evidence._validate_capture_challenge_pair(
            candidate_evidence.canonical_json_bytes(attestation),
            candidate_evidence.canonical_json_bytes(assertion),
            attestation_object,
            key_id,
            policy,
            policy_payload,
            capture_measurements_sha256,
            candidate_evidence,
            production_evidence,
            errors,
        )
        self.assertEqual(errors, [])
        self.assertEqual(kind, "production-artifact-bound")
        self.assertEqual(observed_time, evaluated_at)
        source = (SCRIPT_DIR / "kagemusha_production_app_attest_capture.py").read_text(
            encoding="utf-8"
        )
        self.assertIn('"evaluated_at_unix_ms": evidence_evaluation_time', source)
        self.assertIn('"promotion_eligible": False', source)

    def test_qualification_challenge_requires_distinct_bound_nonces(self) -> None:
        issued_at = 1_800_000_000_000
        nonce = base64.b64encode(b"q" * 32).decode("ascii")
        attestation_object = b"qualification-attestation"
        key_id = base64.b64encode(b"k" * 32).decode("ascii")
        attestation = {
            "schema": capture_evidence.QUALIFICATION_ATTESTATION_CHALLENGE_SCHEMA,
            "version": 1,
            "domain": capture_evidence.QUALIFICATION_ATTESTATION_CHALLENGE_DOMAIN,
            "issued_at_unix_ms": issued_at,
            "nonce_base64": nonce,
        }
        assertion = {
            "schema": capture_evidence.QUALIFICATION_ASSERTION_CHALLENGE_SCHEMA,
            "version": 1,
            "domain": capture_evidence.QUALIFICATION_ASSERTION_CHALLENGE_DOMAIN,
            "issued_at_unix_ms": issued_at,
            "nonce_base64": nonce,
            "attestation_object_sha256": hashlib.sha256(attestation_object).hexdigest(),
            "key_id": key_id,
        }
        errors: list[str] = []
        kind, observed_time = capture_evidence._validate_capture_challenge_pair(
            candidate_evidence.canonical_json_bytes(attestation),
            candidate_evidence.canonical_json_bytes(assertion),
            attestation_object,
            key_id,
            {"policy_id": "qualification"},
            candidate_evidence.canonical_json_bytes({"policy_id": "qualification"}),
            None,
            candidate_evidence,
            production_evidence,
            errors,
        )
        self.assertEqual(kind, "qualification-only")
        self.assertIsNone(observed_time)
        self.assertTrue(any("distinct nonces" in error for error in errors), errors)

    def test_failed_device_capture_is_reported_exactly(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            request_path = root / "request.json"
            capture_path = root / "capture.json"
            policy_path = root / "policy.json"
            for path, value in (
                (
                    request_path,
                    {
                        "schema": capture_evidence.REQUEST_SCHEMA,
                        "version": 1,
                        "attestation_client_data_base64": base64.b64encode(b"{}\n").decode(
                            "ascii"
                        ),
                        "assertion_client_data_template": {},
                    },
                ),
                (
                    capture_path,
                    {
                        "schema": capture_evidence.CAPTURE_SCHEMA,
                        "version": 1,
                        "status": "failed",
                        "app_attest_supported": True,
                        "requested_environment": "production",
                        "started_at_unix_ms": 1,
                        "captured_at_unix_ms": 2,
                        "bundle_id": "org.example.app",
                        "bundle_version": "1",
                        "error_domain": "DCErrorDomain",
                        "error_code": 2,
                        "error_description": "server unavailable",
                    },
                ),
                (policy_path, {}),
            ):
                candidate_evidence.write_private_json(path, value)
            errors, platform, summary = capture_evidence.validate_capture(
                capture_path,
                request_path,
                policy_path,
                candidate_evidence,
                production_evidence,
            )
            self.assertTrue(
                any(
                    "DCErrorDomain 2: server unavailable" in error for error in errors
                ),
                errors,
            )
            self.assertIsNone(platform)
            self.assertIsNone(summary)


if __name__ == "__main__":
    unittest.main()
