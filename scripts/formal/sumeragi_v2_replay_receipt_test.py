#!/usr/bin/env python3
"""Focused fake-process and tamper tests for replay receipt V1.

Run with the supported Xcode interpreter:

    /usr/bin/python3 scripts/formal/sumeragi_v2_replay_receipt_test.py
"""

from __future__ import annotations

import argparse
import copy
import dataclasses
import importlib.util
import json
import os
from pathlib import Path
import signal
import sys
import tempfile
import time
import unittest


ROOT = Path(__file__).resolve().parents[2]
FORMAL = ROOT / "scripts/formal"


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


COLLECTOR = load_module(
    "sumeragi_v2_replay_collector",
    FORMAL / "collect_sumeragi_v2_replay_receipt.py",
)
CHECKER = load_module(
    "sumeragi_v2_replay_checker",
    FORMAL / "check_sumeragi_v2_replay_receipt.py",
)


def message(code: int, severity: int, payload: str) -> str:
    return (
        f"@!@!@STARTMSG {code}:{severity} @!@!@\n"
        f"{payload}\n"
        f"@!@!@ENDMSG {code} @!@!@"
    )


def marker(action: str, node: str, peer: str, view: str, phase: str, subject: str) -> str:
    def number(value: str) -> str:
        return "-1" if value == "-" else value

    return (
        f"[ node |-> {number(node)},\n"
        f"  peer |-> {number(peer)},\n"
        f'  phase |-> "{phase}",\n'
        f'  subject |-> "{subject}",\n'
        f'  action |-> "{action}",\n'
        f"  view |-> {number(view)} ]"
    )


def canonical_tlc_log() -> str:
    fixture = ROOT / "crates/iroha_sumeragi_core/tests/fixtures/tlc_replay_witness.tsv"
    rows = [line.split("\t") for line in fixture.read_text(encoding="utf-8").splitlines() if line[:1].isdigit()]
    items = [
        message(2262, 0, COLLECTOR.TLC_VERSION if hasattr(COLLECTOR, "TLC_VERSION") else "TLC2 Version 2.19 of 08 August 2024 (rev: 5a47802)"),
        message(
            2188,
            0,
            "Running Random Simulation with seed 19349663 with 1 worker on 8 cores "
            "with 1024MB heap and 64MB offheap memory [pid: 123] "
            "(Mac OS X 14.0 aarch64, Homebrew 21.0.1 aarch64, MSBDiskFPSet).",
        ),
        message(2220, 0, "Starting SANY..."),
    ]
    items.extend(f"Parsing file /sealed/{module}.tla" for module in (
        "SumeragiV2TraceWitness", "SumeragiV2", "SumeragiV2Inductive",
        "SumeragiV2Reconfiguration", "SumeragiV2SafetyDefinitions",
        "SumeragiV2CrashRecovery", "SumeragiV2Core", "SumeragiV2Availability",
        "Sequences", "SumeragiV2Quorums", "Naturals", "Integers", "FiniteSets",
    ))
    items.extend(f"Semantic processing of module {module}" for module in (
        "Naturals", "Integers", "Sequences", "FiniteSets", "SumeragiV2Quorums",
        "SumeragiV2Availability", "SumeragiV2Core", "SumeragiV2CrashRecovery",
        "SumeragiV2Reconfiguration", "SumeragiV2SafetyDefinitions",
        "SumeragiV2Inductive", "SumeragiV2", "SumeragiV2TraceWitness",
    ))
    items.extend(
        (
            message(2219, 0, "SANY finished."),
            message(2185, 0, "Starting... (2026-08-21 12:00:00)"),
            message(2269, 0, "Computed 1 initial states..."),
            message(2110, 1, "Invariant NoDecision is violated."),
            message(2121, 1, "The behavior up to this point is:"),
            message(
                2217,
                4,
                "1: <Initial predicate>\n/\\ witnessAction = "
                + marker("Initial", "-", "-", "-", "-", "-")
                + "\n",
            ),
        )
    )
    for step, action, node, peer, view, phase, subject in rows:
        state_number = int(step) + 1
        items.append(
            message(
                2217,
                4,
                f"{state_number}: <WitnessNext line 1, col 1 to line 1, col 1 "
                "of module SumeragiV2TraceWitness>\n/\\ witnessAction = "
                + marker(action, node, peer, view, phase, subject)
                + "\n",
            )
        )
    items.extend(
        (
            message(
                2209,
                0,
                "Progress(-1) at 2026-08-21 12:00:01: 1,001 states generated, "
                "-1 distinct states found, -1 states left on queue.",
            ),
            message(
                2210,
                0,
                "The number of states generated: 1,001\n"
                "Simulation using seed 19349663 and aril 0",
            ),
            message(
                2209,
                0,
                "Progress(-1) at 2026-08-21 12:00:01: 1,002 states generated, "
                "-1 distinct states found, -1 states left on queue.",
            ),
            message(2186, 0, "Finished in 01s at (2026-08-21 12:00:01)"),
        )
    )
    return "\n".join(items) + "\n"


class ReplayReceiptTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.temporary = tempfile.TemporaryDirectory(prefix="sumeragi-receipt-test.")
        cls.work = Path(cls.temporary.name).resolve()
        cls.fake_jar = cls.work / "tla2tools.jar"
        cls.fake_jar.write_bytes(b"fake pinned TLA2Tools for local tests\n")
        cls.projection = cls.work / "projection"
        cls.projection.mkdir(mode=0o700)
        for name, data in {
            "Functions.tla": b"---- MODULE Functions ----\n====\n",
            "Folds.tla": b"---- MODULE Folds ----\n====\n",
        }.items():
            path = cls.projection / name
            path.write_bytes(data)
            path.chmod(0o444)
        cls.projection.chmod(0o555)
        cls.fake_log = cls.work / "fake-tlc.log"
        cls.fake_log.write_text(canonical_tlc_log(), encoding="utf-8")
        cls.fake_java = cls.work / "java"
        cls.fake_java.write_text(
            "#!/usr/bin/python3\n"
            "import pathlib, sys\n"
            "if 'tla2sany.SANY' in sys.argv:\n"
            "    print('Semantic processing of module SumeragiV2TraceWitness')\n"
            "    raise SystemExit(0)\n"
            "if 'tlc2.TLC' in sys.argv:\n"
            "    data = pathlib.Path(__file__).with_name('fake-tlc.log').read_bytes()\n"
            "    sys.stdout.buffer.write(data)\n"
            "    raise SystemExit(12)\n"
            "raise SystemExit(99)\n",
            encoding="utf-8",
        )
        cls.fake_java.chmod(0o755)

        cls.original_collector_jar = COLLECTOR.TLA2TOOLS_SHA256
        cls.original_collector_modules = COLLECTOR.TLAPM_MODULES
        cls.original_checker_jar = CHECKER.TLA2TOOLS_SHA256
        cls.original_checker_modules = CHECKER.TLAPM_HASHES
        fake_jar_hash = COLLECTOR.sha256_bytes(cls.fake_jar.read_bytes())
        module_hashes = {
            name: COLLECTOR.sha256_bytes((cls.projection / name).read_bytes())
            for name in ("Folds.tla", "Functions.tla")
        }
        COLLECTOR.TLA2TOOLS_SHA256 = fake_jar_hash
        COLLECTOR.TLAPM_MODULES = module_hashes
        CHECKER.TLA2TOOLS_SHA256 = fake_jar_hash
        CHECKER.TLAPM_HASHES = {
            f"tlapm-projection/{name}": digest for name, digest in module_hashes.items()
        }
        cls.output = cls.work / "receipt"
        args = argparse.Namespace(
            root=ROOT,
            java_bin=cls.fake_java,
            python_bin=Path("/usr/bin/python3"),
            tla2tools_jar=cls.fake_jar,
            tlapm_projection=cls.projection,
            output_root=cls.output,
            mode="formal-only",
            timeout_seconds=10.0,
        )
        original_argv = sys.argv
        sys.argv = [
            str(FORMAL / "collect_sumeragi_v2_replay_receipt.py"),
            "--root", str(ROOT),
            "--java-bin", str(cls.fake_java),
            "--python-bin", "/usr/bin/python3",
            "--tla2tools-jar", str(cls.fake_jar),
            "--tlapm-projection", str(cls.projection),
            "--output-root", str(cls.output),
            "--mode", "formal-only",
            "--timeout-seconds", "10.0",
        ]
        try:
            cls.receipt_path = COLLECTOR.collect(args)
        finally:
            sys.argv = original_argv
        cls.receipt_bytes = cls.receipt_path.read_bytes()
        cls.receipt = json.loads(cls.receipt_bytes.decode("utf-8"))

    @classmethod
    def tearDownClass(cls) -> None:
        COLLECTOR.TLA2TOOLS_SHA256 = cls.original_collector_jar
        COLLECTOR.TLAPM_MODULES = cls.original_collector_modules
        CHECKER.TLA2TOOLS_SHA256 = cls.original_checker_jar
        CHECKER.TLAPM_HASHES = cls.original_checker_modules
        cls.temporary.cleanup()

    def tearDown(self) -> None:
        self.receipt_path.write_bytes(self.receipt_bytes)
        self.receipt_path.chmod(0o600)

    def _reject_mutation(self, mutate) -> None:
        value = copy.deepcopy(self.receipt)
        mutate(value)
        self.receipt_path.write_bytes(CHECKER.canonical_json(value))
        with self.assertRaises(CHECKER.ReceiptError):
            CHECKER.check(self.receipt_path)

    def test_fake_process_receipt_passes_and_unsigned_is_not_release(self) -> None:
        checked = CHECKER.check(self.receipt_path)
        self.assertEqual(checked["result"]["tool_states"], 101)
        self.assertEqual(checked["result"]["actions"], 100)
        with self.assertRaisesRegex(CHECKER.ReceiptError, "not release evidence"):
            CHECKER.check(self.receipt_path, require_release=True)

    def test_schema_and_collector_contract_are_diagnostic_only(self) -> None:
        schema = json.loads(
            (FORMAL / "sumeragi_v2_replay_receipt_v1.schema.json").read_text(
                encoding="utf-8"
            )
        )
        properties = schema["properties"]
        self.assertEqual(properties["evidence_class"], {"const": "diagnostic"})
        self.assertEqual(properties["mode"], {"const": "formal-only"})
        self.assertEqual(
            set(properties["result"]["required"]), set(self.receipt["result"])
        )
        self.assertEqual(set(COLLECTOR.EVENT_TEMPLATES), {"formal-only"})
        self.assertEqual(self.receipt["runner"]["event_graph"]["nodes"], [
            "standalone_sany", "raw_tlc", "normalizer"
        ])

    def test_forged_source_attestation_cannot_promote_receipt(self) -> None:
        value = copy.deepcopy(self.receipt)
        value["evidence_class"] = "release"
        value["signing"] = {
            "status": "verified-project-ssh-git-identity",
            "provider": "scripts/verify_sumeragi_v2_release_identity.py",
            "release_evidence": True,
            "attestation": "signing/attestation.json",
        }
        self.receipt_path.write_bytes(CHECKER.canonical_json(value))
        with self.assertRaisesRegex(CHECKER.ReceiptError, "evidence class"):
            CHECKER.check(self.receipt_path)

    def test_persisted_file_identity_is_filesystem_location_independent(self) -> None:
        snapshot = COLLECTOR._read_snapshot(self.fake_jar, "tool/tla2tools.jar")
        relocated = dataclasses.replace(
            snapshot,
            device=snapshot.device + 1,
            inode=snapshot.inode + 1,
        )
        self.assertEqual(snapshot.receipt_record(), relocated.receipt_record())
        self.assertEqual(COLLECTOR._manifest([snapshot]), COLLECTOR._manifest([relocated]))
        self.assertNotIn("device", snapshot.receipt_record())
        self.assertNotIn("inode", snapshot.receipt_record())

    def test_checker_rejects_process_source_tool_and_graph_tampering(self) -> None:
        mutations = (
            lambda value: value["events"][0]["argv"].append("--alias"),
            lambda value: value["invocation"]["argv"].append("--alias"),
            lambda value: value["invocation"].__setitem__("cwd", "/tmp"),
            lambda value: value["events"][0].__setitem__("cwd", "/tmp"),
            lambda value: value["events"][0]["environment"].__setitem__("HOME", "/tmp"),
            lambda value: value["events"][0]["descriptors"].__setitem__("close_fds", False),
            lambda value: value["events"][1]["status"].__setitem__("actual", 0),
            lambda value: value["events"][1]["timeout"].__setitem__("occurred", True),
            lambda value: value["events"][1]["cleanup"].__setitem__("scope", "all-descendants"),
            lambda value: value["events"][1]["cleanup"].__setitem__("process_group_quiescent", False),
            lambda value: value["events"][2]["outputs"]["stdout"].__setitem__("sha256", "0" * 64),
            lambda value: value["source_identity"]["files"][0].__setitem__("sha256", "1" * 64),
            lambda value: value["source_identity"]["files"][0].__setitem__("device", 1),
            lambda value: value["tool_identity"]["files"][0].__setitem__("inode", 1),
            lambda value: value["tool_identity"]["files"][0].__setitem__("sha256", "2" * 64),
            lambda value: value["runner"]["event_graph"]["edges"].clear(),
        )
        for mutate in mutations:
            with self.subTest(mutate=mutate):
                self._reject_mutation(mutate)

    def test_checker_rejects_artifact_tampering_and_unexpected_files(self) -> None:
        artifact = self.output / "events/03-normalizer.stdout"
        original = artifact.read_bytes()
        artifact.write_bytes(original + b"tamper\n")
        with self.assertRaises(CHECKER.ReceiptError):
            CHECKER.check(self.receipt_path)
        artifact.write_bytes(original)
        unexpected = self.output / "unexpected"
        unexpected.write_bytes(b"unexpected\n")
        unexpected.chmod(0o600)
        with self.assertRaisesRegex(CHECKER.ReceiptError, "file set differs"):
            CHECKER.check(self.receipt_path)
        unexpected.unlink()

        extra = self.output / "events/extra.stdout"
        extra.write_bytes(b"self-consistent extra\n")
        extra.chmod(0o600)
        value = copy.deepcopy(self.receipt)
        value["artifact_inventory"].append(
            CHECKER._file_record(extra, "events/extra.stdout", single_link=True)
        )
        value["artifact_inventory"].sort(key=lambda item: item["path"])
        self.receipt_path.write_bytes(CHECKER.canonical_json(value))
        with self.assertRaisesRegex(
            CHECKER.ReceiptError, "does not match exact event outputs"
        ):
            CHECKER.check(self.receipt_path)
        extra.unlink()
        self.receipt_path.write_bytes(self.receipt_bytes)

        empty = self.output / "empty"
        empty.mkdir(mode=0o700)
        with self.assertRaisesRegex(CHECKER.ReceiptError, "directory set differs"):
            CHECKER.check(self.receipt_path)
        empty.rmdir()

    def test_timeout_terminates_process_group_and_records_cleanup(self) -> None:
        timeout_root = COLLECTOR._safe_output_root(self.work / "timeout-event")
        timeout_events = COLLECTOR._create_owned_named_child(timeout_root, "events")
        sleeper = self.work / "sleeper"
        sleeper.write_text(
            "#!/usr/bin/python3\n"
            "import os, time\n"
            "if os.fork() == 0:\n"
            "    time.sleep(30)\n"
            "    raise SystemExit(0)\n"
            "time.sleep(30)\n",
            encoding="utf-8",
        )
        sleeper.chmod(0o755)
        try:
            event = COLLECTOR.run_process_event(
                name="timeout_probe",
                argv=[str(sleeper)],
                cwd=self.work,
                environment={"LANG": "C", "LC_ALL": "C", "TMPDIR": str(self.work), "TZ": "UTC"},
                expected_status=0,
                timeout_seconds=0.2,
                events_directory=timeout_events,
                sequence=1,
            )
        finally:
            os.close(timeout_events.descriptor)
            timeout_events.descriptor = -1
            os.close(timeout_root.descriptor)
            timeout_root.descriptor = -1
        self.assertTrue(event["timeout"]["occurred"])
        self.assertTrue(event["timeout"]["sigterm_sent"])
        self.assertEqual(event["cleanup"]["scope"], "new-session-process-group")
        self.assertTrue(event["cleanup"]["process_group_quiescent"])
        self.assertFalse(event["status"]["matched"])

    def test_owned_cleanup_rejects_rename_and_replacement(self) -> None:
        output = COLLECTOR._safe_output_root(self.work / "cleanup-owner")
        child = COLLECTOR._create_owned_child(output, ".runtime.")
        marker = child.path / "owned-marker"
        marker.write_bytes(b"owned\n")
        moved = self.work / "moved-runtime"
        child.path.rename(moved)
        child.path.mkdir(mode=0o700)
        victim = child.path / "must-survive"
        victim.write_bytes(b"replacement\n")
        try:
            with self.assertRaisesRegex(
                COLLECTOR.CollectionError, "renamed or replaced"
            ):
                COLLECTOR._remove_owned_child(output, child)
            self.assertEqual(victim.read_bytes(), b"replacement\n")
            self.assertEqual((moved / "owned-marker").read_bytes(), b"owned\n")
        finally:
            if child.descriptor >= 0:
                os.close(child.descriptor)
                child.descriptor = -1
            os.close(output.descriptor)
            output.descriptor = -1

    def test_publication_rejects_output_root_replacement(self) -> None:
        output = COLLECTOR._safe_output_root(self.work / "publication-owner")
        moved = self.work / "moved-publication-owner"
        output.path.rename(moved)
        output.path.mkdir(mode=0o700)
        try:
            with self.assertRaisesRegex(
                COLLECTOR.CollectionError, "renamed or replaced"
            ):
                COLLECTOR._require_owned_path(output)
            self.assertEqual(list(output.path.iterdir()), [])
            self.assertEqual(list(moved.iterdir()), [])
        finally:
            os.close(output.descriptor)
            output.descriptor = -1

    def test_cleanup_contract_does_not_claim_escaped_session_descendants(self) -> None:
        output = COLLECTOR._safe_output_root(self.work / "escape-event")
        events = COLLECTOR._create_owned_named_child(output, "events")
        pid_file = self.work / "escaped.pid"
        escape = self.work / "escape-session"
        escape.write_text(
            "#!/usr/bin/python3\n"
            "import os, pathlib, sys, time\n"
            "if os.fork() != 0:\n"
            "    raise SystemExit(0)\n"
            "os.setsid()\n"
            "os.close(1)\n"
            "os.close(2)\n"
            "pathlib.Path(sys.argv[1]).write_text(str(os.getpid()), encoding='ascii')\n"
            "time.sleep(30)\n",
            encoding="utf-8",
        )
        escape.chmod(0o755)
        escaped_pid = None
        try:
            event = COLLECTOR.run_process_event(
                name="escape_probe",
                argv=[str(escape), str(pid_file)],
                cwd=self.work,
                environment={"LANG": "C", "LC_ALL": "C", "TMPDIR": str(self.work), "TZ": "UTC"},
                expected_status=0,
                timeout_seconds=2.0,
                events_directory=events,
                sequence=1,
            )
            deadline = time.monotonic() + 2.0
            while not pid_file.exists() and time.monotonic() < deadline:
                time.sleep(0.01)
            escaped_pid = int(pid_file.read_text(encoding="ascii"))
            os.kill(escaped_pid, 0)
            self.assertTrue(COLLECTOR._event_succeeded(event))
            self.assertEqual(
                event["cleanup"]["scope"], "new-session-process-group"
            )
            self.assertNotIn("unexpected_descendants", event["cleanup"])
        finally:
            if escaped_pid is not None:
                try:
                    os.kill(escaped_pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            os.close(events.descriptor)
            events.descriptor = -1
            os.close(output.descriptor)
            output.descriptor = -1


if __name__ == "__main__":
    unittest.main()
