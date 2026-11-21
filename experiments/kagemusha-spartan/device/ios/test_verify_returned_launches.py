"""Regression tests against synthetic copies of the Swift report and Native history."""
import hashlib
import json
import plistlib
import runpy
import tempfile
import unittest
from pathlib import Path

verifier = runpy.run_path(str(Path(__file__).with_name("verify-returned-launches.py")))
verify, fixture_namespace = verifier["verify"], verifier["fixture_namespace"]
DOMAIN = b"kagemusha-spartan-fixture-build-namespace-v1"


def executable(path, marker, build=b"build-a", size=4096, at=None):
    """Write a stand-in signed executable; the verifier only hashes and scans its bytes."""
    data = bytearray(build.ljust(size, b"\0"))
    if marker:
        at = size // 2 if at is None else at
        data[at:at + len(DOMAIN) + 1] = DOMAIN + b"\0"
    path.write_bytes(bytes(data))
    return hashlib.sha256(data).hexdigest()


class ReturnedLaunchTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        self.app = root / "fixture.app"
        assets = self.app / "native-device"
        assets.mkdir(parents=True)
        (self.app / "Info.plist").write_bytes(plistlib.dumps(
            {"CFBundleIdentifier": "jp.co.soramitsu.kagemusha.spartanfixture", "CFBundleExecutable": "Fixture"}))
        self.build = executable(self.app / "Fixture", marker=False)
        (assets / "parameter-manifest.norito").write_bytes(b"unit-test manifest")
        (assets / "program-bundle.norito").write_bytes(b"unit-test programs")
        self.launches = [root / str(i) for i in range(3)]
        self.reports = []
        self.metrics = []
        funded = ["Bootstrap", "Load", "Send"]
        for i, directory in enumerate(self.launches):
            directory.mkdir()
            phases = [(op, "execute") for op in (funded, ["Receive"], [])[i]]
            if i == 0:
                phases.insert(2, ("Send", "confirmation_wait"))
            else:
                phases.append(("Receive", "credit_check_drill"))
            self.metrics.append({
                "schema": "kagemusha.experimental.device-observations.v2",
                "succeeded": True, "program_bundle_sha256": hashlib.sha256(b"unit-test programs").hexdigest(),
                "outgoing_payment_bytes": 17, "outgoing_payment_sha256": "11" * 32,
                "incoming_payment_bytes": 23, "incoming_payment_sha256": "22" * 32,
                "retained_operations": ([], funded, funded + ["Receive"])[i],
                "events": [{"operation": op, "phase": name, "returned_ok": True} for op, name in phases],
                "receive_observed_flow_us": 105, "receive_test_drill_us": 2, "receive_instrumentation_us": 3,
                "receive_normal_flow_us": 100, "process_lifetime_peak_rss_bytes": 1200})
            self.reports.append({
                "schema": "kagemusha-spartan-device-runtime-v1",
                "manifest_sha256": hashlib.sha256(b"unit-test manifest").hexdigest(),
                "result": "PASS: " + ("stage 1 of 2: " if i == 0 else "") + "device hardware keys."})

    def write_copies(self, attempts=()):
        history = b'{"earlier":"interrupted observation"}\n'
        for attempt in attempts:
            history += json.dumps(attempt).encode() + b"\n"
        for directory, report, metrics in zip(self.launches, self.reports, self.metrics):
            line = json.dumps(metrics)
            history += line.encode() + b"\n"
            (directory / "result.json").write_text(json.dumps(dict(report, result=report["result"] + "\n" + line)))
            (directory / "device-measurements.jsonl").write_bytes(history)

    def build_scoped(self, **change):
        self.build = executable(self.app / "Fixture", marker=True, **change)
        manifest = hashlib.sha256(b"unit-test manifest").hexdigest()
        scope = hashlib.sha256(DOMAIN + b"\0" + bytes.fromhex(self.build) + bytes.fromhex(manifest)).hexdigest()
        for report in self.reports:
            report.update(schema="kagemusha-spartan-device-runtime-v2", fixture_namespace=scope,
                          build_executable_sha256=self.build)
        return scope

    def test_consecutive_launches_and_receive(self):
        self.write_copies()
        result = verify(self.app, self.launches)
        self.assertEqual(result["receive"], {"normal_flow_us": 100, "process_lifetime_peak_rss_bytes": 1200})
        self.assertEqual(result["payments"]["incoming"]["bytes"], 23)
        manifest = hashlib.sha256(b"unit-test manifest").hexdigest()
        self.assertEqual((result["scheme"], result["fixture_namespace"], result["build_executable_sha256"]),
                         ("retained-manifest-only", manifest, self.build))

    def test_retained_app_refuses_build_scoped_reports(self):
        self.build_scoped()
        executable(self.app / "Fixture", marker=False)
        self.write_copies()
        with self.assertRaisesRegex(ValueError, "build namespace"):
            verify(self.app, self.launches)

    def test_build_scoped_reports_bind_the_executable_and_manifest(self):
        scope = self.build_scoped()
        self.write_copies()
        result = verify(self.app, self.launches)
        self.assertEqual((result["scheme"], result["fixture_namespace"], result["build_executable_sha256"]),
                         ("build-and-manifest-v1", scope, self.build))
        self.assertNotEqual(scope, result["manifest_sha256"])
        described = fixture_namespace(self.app)
        self.assertEqual(described["history_path"], "Library/Application Support/kagemusha-wallet-v1/"
                         f"spartan-runtime-fixture-{scope}/device-measurements.jsonl")
        self.assertEqual((described["report_path"], described["payment_key_tag_prefix"]),
                         (f"Documents/spartan-runtime-result-{scope}.json", f"kgm-ns-w1-{scope}-"))
        for key, value in [("schema", "kagemusha-spartan-device-runtime-v1"), ("fixture_namespace", None),
                           ("fixture_namespace", result["manifest_sha256"]),
                           ("build_executable_sha256", "00" * 32), ("manifest_sha256", "00" * 32)]:
            with self.subTest(key=key, value=value), self.assertRaisesRegex(ValueError, "build namespace"):
                old, self.reports[2][key] = self.reports[2][key], value
                self.write_copies()
                verify(self.app, self.launches)
            self.reports[2][key] = old
        # Another build (or a re-signed one) with the same manifest selects another namespace.
        executable(self.app / "Fixture", marker=True, build=b"build-b")
        self.write_copies()
        with self.assertRaisesRegex(ValueError, "build namespace"):
            verify(self.app, self.launches)

    def test_marker_spanning_read_chunks_selects_the_build_scheme(self):
        for at in [(1 << 20) - 20, (2 << 20) - len(DOMAIN), (1 << 20) - 1]:
            with self.subTest(at=at):
                scope = self.build_scoped(size=3 << 20, at=at)
                self.assertEqual(fixture_namespace(self.app)["fixture_namespace"], scope)
        executable(self.app / "Fixture", marker=False, size=3 << 20)
        self.assertEqual(fixture_namespace(self.app)["scheme"], "retained-manifest-only")

    def test_resumes_funding_after_interrupted_send_review(self):
        funded = self.metrics[0]
        previous = dict(funded, succeeded=False, events=[
            {"operation": "Bootstrap", "phase": "execute", "returned_ok": True},
            {"operation": "Load", "phase": "execute", "returned_ok": True},
            {"operation": "Send", "phase": "confirmation_wait", "returned_ok": False},
        ])
        funded["retained_operations"] = ["Bootstrap", "Load"]
        funded["events"] = [
            {"operation": "Send", "phase": "confirmation_wait", "returned_ok": True},
            {"operation": "Send", "phase": "execute", "returned_ok": True},
        ]
        self.write_copies([previous])
        self.assertIn("receive", verify(self.app, self.launches))
        for change in [{"events": previous["events"][1:]},
                       {"program_bundle_sha256": "00" * 32}, {"succeeded": True}]:
            with self.subTest(change=change), self.assertRaises(ValueError):
                self.write_copies([dict(previous, **change)])
                verify(self.app, self.launches)

    def test_send_cannot_precede_confirmation(self):
        self.metrics[0]["events"][-2:] = self.metrics[0]["events"][-2:][::-1]
        self.write_copies()
        with self.assertRaisesRegex(ValueError, "confirmation"):
            verify(self.app, self.launches)

    def test_resumed_send_requires_recorded_hardware_confirmation(self):
        funded = self.metrics[0]
        confirmed = {"operation": "Send", "phase": "confirmation_wait", "returned_ok": True}
        debited = {"operation": "Send", "phase": "execute", "returned_ok": True}
        prior = dict(funded, succeeded=False, events=[
            {"operation": op, "phase": "execute", "returned_ok": True} for op in ("Bootstrap", "Load")
        ])
        retry = dict(funded, succeeded=False, retained_operations=["Bootstrap", "Load"],
                     events=[confirmed, debited])
        funded["retained_operations"] = ["Bootstrap", "Load", "Send"]
        funded["events"] = []
        self.write_copies([prior, retry])
        self.assertIn("receive", verify(self.app, self.launches))
        for wrong in [[debited], [debited, confirmed], [dict(confirmed, returned_ok=False), debited]]:
            with self.subTest(wrong=wrong), self.assertRaises(ValueError):
                self.write_copies([prior, dict(retry, events=wrong)])
                verify(self.app, self.launches)

    def test_out_of_sequence_history(self):
        self.write_copies()
        path = self.launches[2] / "device-measurements.jsonl"
        path.write_bytes(b"{}\n" + path.read_bytes())
        with self.assertRaisesRegex(ValueError, "consecutive"):
            verify(self.app, self.launches)

    def test_stale_history(self):
        self.write_copies()
        name = "device-measurements.jsonl"
        (self.launches[1] / name).write_bytes((self.launches[0] / name).read_bytes())
        with self.assertRaises(ValueError):
            verify(self.app, self.launches)

    def test_refuses_mismatched_assets_payment_measurement_and_phase(self):
        for mapping, key, value in [
            (self.reports[1], "manifest_sha256", "00" * 32),
            (self.reports[1], "result", "PASS: host software keys."),
            (self.metrics[2], "incoming_payment_sha256", "33" * 32),
            (self.metrics[0], "program_bundle_sha256", "44" * 32),
            (self.metrics[1], "receive_normal_flow_us", 104),
            (self.metrics[2], "retained_operations", []),
            (self.metrics[1]["events"][0], "returned_ok", False),
        ]:
            with self.subTest(key=key):
                old, mapping[key] = mapping[key], value
                self.write_copies()
                with self.assertRaises(ValueError):
                    verify(self.app, self.launches)
                mapping[key] = old


if __name__ == "__main__":
    unittest.main()
