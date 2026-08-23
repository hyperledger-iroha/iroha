#!/usr/bin/env python3
"""Focused fake-process and tamper tests for replay receipt V1.

Run with the supported Xcode interpreter:

    /usr/bin/python3 scripts/formal/sumeragi_v2_replay_receipt_test.py
"""

from __future__ import annotations

import argparse
import base64
import contextlib
import copy
import dataclasses
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import signal
import stat
import subprocess
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


SIGNING = load_module(
    "sumeragi_v2_replay_signing",
    FORMAL / "sumeragi_v2_replay_signing.py",
)
COLLECTOR = load_module(
    "sumeragi_v2_replay_collector",
    FORMAL / "collect_sumeragi_v2_replay_receipt.py",
)
CHECKER = load_module(
    "check_sumeragi_v2_replay_receipt",
    FORMAL / "check_sumeragi_v2_replay_receipt.py",
)
FINALIZER = load_module(
    "finalize_sumeragi_v2_replay_receipt",
    FORMAL / "finalize_sumeragi_v2_replay_receipt.py",
)
RELEASE_VERIFIER = load_module(
    "verify_sumeragi_v2_replay_release",
    FORMAL / "verify_sumeragi_v2_replay_release.py",
)
RECEIPT_WRITER = load_module(
    "write_sumeragi_v2_release_receipt",
    ROOT / "scripts/write_sumeragi_v2_release_receipt.py",
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
        "SumeragiV2TraceWitness", "SumeragiV2Inductive",
        "SumeragiV2Reconfiguration", "SumeragiV2SafetyDefinitions",
        "SumeragiV2CrashRecovery", "SumeragiV2Core", "SumeragiV2Availability",
        "Sequences", "SumeragiV2Quorums", "Naturals", "Integers", "FiniteSets",
    ))
    items.extend(f"Semantic processing of module {module}" for module in (
        "Naturals", "Integers", "Sequences", "FiniteSets", "SumeragiV2Quorums",
        "SumeragiV2Availability", "SumeragiV2Core", "SumeragiV2CrashRecovery",
        "SumeragiV2Reconfiguration", "SumeragiV2SafetyDefinitions",
        "SumeragiV2Inductive", "SumeragiV2TraceWitness",
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

        cls.principal = "sumeragi-release@test.invalid"
        cls.signing_key = cls.work / "release-signing-key"
        subprocess.run(
            [
                "/usr/bin/ssh-keygen",
                "-q",
                "-t",
                "ed25519",
                "-N",
                "",
                "-f",
                str(cls.signing_key),
            ],
            check=True,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        public_fields = Path(str(cls.signing_key) + ".pub").read_text(
            encoding="ascii"
        ).split()
        cls.allowed_signers = cls.work / "allowed_signers"
        cls.allowed_signers.write_text(
            f"{cls.principal} ssh-ed25519 {public_fields[1]}\n",
            encoding="ascii",
        )
        cls.allowed_signers.chmod(0o400)
        cls.revocation = cls.work / "revocation.krl"
        cls.revocation.write_bytes(b"")
        cls.revocation.chmod(0o400)
        subprocess.run(
            [
                "/usr/bin/ssh-keygen",
                "-Y",
                "sign",
                "-f",
                str(cls.signing_key),
                "-n",
                SIGNING.SSHSIG_NAMESPACE,
                str(cls.receipt_path),
            ],
            check=True,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        generated_signature = Path(str(cls.receipt_path) + ".sig")
        cls.signature = cls.work / "receipt.release.sig"
        generated_signature.replace(cls.signature)
        cls.signature.chmod(0o400)
        cls.ssh_keygen = cls.work / "ssh-keygen.release-tool"
        cls.ssh_keygen.write_text(
            "#!/bin/sh\nexec /usr/bin/ssh-keygen \"$@\"\n",
            encoding="ascii",
        )
        cls.ssh_keygen.chmod(0o500)
        fingerprint_fields = subprocess.run(
            [
                "/usr/bin/ssh-keygen",
                "-lf",
                str(cls.signing_key) + ".pub",
                "-E",
                "sha256",
            ],
            check=True,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        ).stdout.split()
        cls.fingerprint = next(
            value for value in fingerprint_fields if value.startswith("SHA256:")
        )
        cls.protected_bytes = {
            cls.signature: cls.signature.read_bytes(),
            cls.ssh_keygen: cls.ssh_keygen.read_bytes(),
            cls.allowed_signers: cls.allowed_signers.read_bytes(),
            cls.revocation: cls.revocation.read_bytes(),
        }
        cls.protected_modes = {
            cls.signature: 0o400,
            cls.ssh_keygen: 0o500,
            cls.allowed_signers: 0o400,
            cls.revocation: 0o400,
        }

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
        for path, data in self.protected_bytes.items():
            path.chmod(0o600)
            path.write_bytes(data)
            path.chmod(self.protected_modes[path])

    @staticmethod
    def _sha256(path: Path) -> str:
        return hashlib.sha256(path.read_bytes()).hexdigest()

    def _signature_inputs(self, **overrides):
        values = {
            "signature": self.signature,
            "expected_signature_sha256": self._sha256(self.signature),
            "ssh_keygen": self.ssh_keygen,
            "expected_ssh_keygen_sha256": self._sha256(self.ssh_keygen),
            "allowed_signers": self.allowed_signers,
            "expected_allowed_signers_sha256": self._sha256(
                self.allowed_signers
            ),
            "revocation_file": self.revocation,
            "expected_revocation_sha256": self._sha256(self.revocation),
            "principal": self.principal,
            "expected_signer_fingerprint": self.fingerprint,
        }
        values.update(overrides)
        return SIGNING.SignatureInputs(**values)

    def _finalizer_args(self, output_root: Path) -> argparse.Namespace:
        inputs = self._signature_inputs()
        return argparse.Namespace(
            receipt=self.receipt_path,
            signature=inputs.signature,
            expected_signature_sha256=inputs.expected_signature_sha256,
            ssh_keygen_bin=inputs.ssh_keygen,
            expected_ssh_keygen_sha256=inputs.expected_ssh_keygen_sha256,
            allowed_signers=inputs.allowed_signers,
            expected_allowed_signers_sha256=inputs.expected_allowed_signers_sha256,
            revocation_file=inputs.revocation_file,
            expected_revocation_sha256=inputs.expected_revocation_sha256,
            principal=inputs.principal,
            expected_signer_fingerprint=inputs.expected_signer_fingerprint,
            output_root=output_root,
        )

    def _release_verifier_args(self, output_root: Path) -> argparse.Namespace:
        inputs = self._signature_inputs()
        return argparse.Namespace(
            source_receipt=self.receipt_path,
            release_root=output_root,
            expected_signature_sha256=inputs.expected_signature_sha256,
            expected_ssh_keygen_sha256=inputs.expected_ssh_keygen_sha256,
            expected_allowed_signers_sha256=inputs.expected_allowed_signers_sha256,
            expected_revocation_sha256=inputs.expected_revocation_sha256,
            principal=inputs.principal,
            expected_signer_fingerprint=inputs.expected_signer_fingerprint,
        )

    def _reject_mutation(self, mutate) -> None:
        value = copy.deepcopy(self.receipt)
        mutate(value)
        self.receipt_path.write_bytes(CHECKER.canonical_json(value))
        with self.assertRaises(CHECKER.ReceiptError):
            CHECKER._check_structure(self.receipt_path)

    def test_signing_request_cannot_succeed_in_release_checker(self) -> None:
        original_argv = sys.argv
        stdout = io.StringIO()
        stderr = io.StringIO()
        sys.argv = [
            str(FORMAL / "check_sumeragi_v2_replay_receipt.py"),
            str(self.receipt_path),
        ]
        try:
            with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(
                stderr
            ):
                status = CHECKER.main()
        finally:
            sys.argv = original_argv
        self.assertEqual(status, 2)
        self.assertEqual(stdout.getvalue(), "")
        self.assertEqual(
            stderr.getvalue(),
            "Sumeragi V2 replay receipt verification failed: release verification "
            "requires every detached SSHSIG input\n",
        )

    def test_schema_and_collector_expose_one_release_contract(self) -> None:
        schema = json.loads(
            (FORMAL / "sumeragi_v2_replay_receipt_v1.schema.json").read_text(
                encoding="utf-8"
            )
        )
        properties = schema["properties"]
        self.assertEqual(properties["evidence_class"], {"const": "release-receipt"})
        self.assertEqual(properties["mode"], {"const": "formal-only"})
        self.assertEqual(
            set(properties["result"]["required"]), set(self.receipt["result"])
        )
        self.assertEqual(self.receipt["signing"], SIGNING.SIGNING_CONTRACT)
        self.assertEqual(set(COLLECTOR.EVENT_TEMPLATES), {"formal-only"})
        self.assertEqual(self.receipt["runner"]["event_graph"]["nodes"], [
            "standalone_sany", "raw_tlc", "normalizer"
        ])

    def test_release_signature_checker_and_finalizer_pass(self) -> None:
        checked = CHECKER.check_release(
            self.receipt_path, self._signature_inputs()
        )
        self.assertTrue(checked["result"]["execution_validated"])
        with tempfile.TemporaryDirectory(
            prefix="finalized-parent.", dir=self.work
        ) as raw_parent:
            parent = Path(raw_parent).resolve()
            output_root = parent / "release"
            marker = FINALIZER.finalize(self._finalizer_args(output_root))
            self.assertEqual(marker, output_root / "release-attestation.json")
            attestation = RELEASE_VERIFIER.verify_release(
                self._release_verifier_args(output_root)
            )
            self.assertEqual(
                attestation["schema"],
                "iroha-sumeragi-v2-replay-release-attestation-v1",
            )
            self.assertEqual(
                attestation["signature"]["scheme"], "detached-ssh"
            )
            self.assertEqual(set(item.name for item in output_root.iterdir()), {
                "receipt.json",
                "receipt.json.sig",
                "ssh-keygen.release-tool",
                "allowed_signers",
                "revocation.krl",
                "release-attestation.json",
            })

    def test_release_checker_binds_structure_to_the_signed_snapshot(self) -> None:
        replacement = self.work / "receipt.structure-snapshot-replacement.json"
        held = self.work / "receipt.structure-snapshot-original.json"
        replacement.write_bytes(self.receipt_path.read_bytes())
        replacement.chmod(0o600)
        original_check_structure = CHECKER._check_structure

        def swap_after_structure(path):
            receipt = original_check_structure(path)
            path.rename(held)
            replacement.rename(path)
            return receipt

        CHECKER._check_structure = swap_after_structure
        try:
            with self.assertRaisesRegex(SIGNING.SigningError, "changed"):
                CHECKER.check_release(
                    self.receipt_path, self._signature_inputs()
                )
        finally:
            CHECKER._check_structure = original_check_structure
            if held.exists():
                if self.receipt_path.exists():
                    self.receipt_path.unlink()
                held.rename(self.receipt_path)
            if replacement.exists():
                replacement.unlink()
            self.receipt_path.chmod(0o600)

    def test_aggregate_writer_rejects_release_root_swap_at_verifier_return(self) -> None:
        with tempfile.TemporaryDirectory(
            prefix="aggregate-swap-parent.", dir=self.work
        ) as raw_parent:
            parent = Path(raw_parent).resolve()
            output_root = parent / "release"
            FINALIZER.finalize(self._finalizer_args(output_root))
            alternate = parent / "alternate"
            shutil.copytree(output_root, alternate, copy_function=shutil.copy2)
            alternate.chmod(0o700)
            held = parent / "held"
            inputs = self._signature_inputs()
            original_runner = RECEIPT_WRITER._run_bounded_python_validator

            def swap_after_verifier(
                _checker, _arguments, *, watched_contracts=(), **_kwargs
            ):
                watched_paths = {item.path for item in watched_contracts}
                self.assertIn(self.receipt_path, watched_paths)
                self.assertTrue(
                    {
                        output_root / "receipt.json",
                        output_root / "receipt.json.sig",
                        output_root / "ssh-keygen.release-tool",
                        output_root / "allowed_signers",
                        output_root / "revocation.krl",
                        output_root / "release-attestation.json",
                    }.issubset(watched_paths)
                )
                output_root.rename(held)
                alternate.rename(output_root)
                return (
                    0,
                    (
                        "verified finalized Sumeragi V2 replay release for "
                        f"{self.fingerprint}\n"
                    ).encode("utf-8"),
                    b"",
                )

            RECEIPT_WRITER._run_bounded_python_validator = swap_after_verifier
            try:
                with self.assertRaisesRegex(
                    RECEIPT_WRITER.ReceiptError,
                    "directories changed during verification",
                ):
                    RECEIPT_WRITER._formal_replay_release(
                        source_receipt_path=self.receipt_path,
                        release_root_path=output_root,
                        expected_signature_sha256=(
                            inputs.expected_signature_sha256
                        ),
                        expected_ssh_keygen_sha256=(
                            inputs.expected_ssh_keygen_sha256
                        ),
                        expected_allowed_signers_sha256=(
                            inputs.expected_allowed_signers_sha256
                        ),
                        expected_revocation_sha256=(
                            inputs.expected_revocation_sha256
                        ),
                        principal=self.principal,
                        expected_signer_fingerprint=self.fingerprint,
                        checker_environment={
                            "LANG": "C",
                            "LC_ALL": "C",
                            "PATH": os.defpath,
                            "TZ": "UTC",
                        },
                        repo_root=ROOT,
                    )
            finally:
                RECEIPT_WRITER._run_bounded_python_validator = original_runner
                if held.exists():
                    if output_root.exists():
                        output_root.rename(alternate)
                    held.rename(output_root)

    def test_signature_receipt_tool_policy_and_revocation_tampering_fail(self) -> None:
        original_signature = self.signature.read_text(encoding="ascii").splitlines()
        encoded = list(original_signature[1])
        encoded[8] = "A" if encoded[8] != "A" else "B"
        original_signature[1] = "".join(encoded)
        self.signature.chmod(0o600)
        self.signature.write_text("\n".join(original_signature) + "\n", encoding="ascii")
        self.signature.chmod(0o400)
        with self.assertRaises(SIGNING.SigningError):
            CHECKER.check_release(self.receipt_path, self._signature_inputs())

        self.signature.chmod(0o600)
        self.signature.write_bytes(self.protected_bytes[self.signature])
        self.signature.chmod(0o400)
        value = copy.deepcopy(self.receipt)
        value["events"][0]["duration_monotonic_ns"] += 1
        self.receipt_path.write_bytes(CHECKER.canonical_json(value))
        self.receipt_path.chmod(0o600)
        with self.assertRaises(SIGNING.SigningError):
            CHECKER.check_release(self.receipt_path, self._signature_inputs())

        self.receipt_path.write_bytes(self.receipt_bytes)
        self.receipt_path.chmod(0o600)
        protected_inputs = self._signature_inputs()
        self.ssh_keygen.chmod(0o700)
        self.ssh_keygen.write_bytes(self.protected_bytes[self.ssh_keygen] + b"#")
        self.ssh_keygen.chmod(0o500)
        with self.assertRaisesRegex(SIGNING.SigningError, "protected SHA-256"):
            CHECKER.check_release(self.receipt_path, protected_inputs)

        self.ssh_keygen.chmod(0o700)
        self.ssh_keygen.write_bytes(self.protected_bytes[self.ssh_keygen])
        self.ssh_keygen.chmod(0o500)
        self.allowed_signers.chmod(0o600)
        self.allowed_signers.write_bytes(
            self.protected_bytes[self.allowed_signers]
            + self.protected_bytes[self.allowed_signers]
        )
        self.allowed_signers.chmod(0o400)
        with self.assertRaisesRegex(SIGNING.SigningError, "exactly one"):
            CHECKER.check_release(self.receipt_path, self._signature_inputs())

        policy_fields = self.protected_bytes[self.allowed_signers].decode(
            "ascii"
        ).split()
        malformed_wire = base64.b64encode(
            base64.b64decode(policy_fields[2]) + b"\x00"
        ).decode("ascii")
        self.allowed_signers.chmod(0o600)
        self.allowed_signers.write_text(
            f"{self.principal} ssh-ed25519 {malformed_wire}\n",
            encoding="ascii",
        )
        self.allowed_signers.chmod(0o400)
        with self.assertRaisesRegex(SIGNING.SigningError, "key is malformed"):
            CHECKER.check_release(self.receipt_path, self._signature_inputs())

        self.allowed_signers.chmod(0o600)
        self.allowed_signers.write_bytes(self.protected_bytes[self.allowed_signers])
        self.allowed_signers.chmod(0o400)
        revoked = self.work / "revoked.krl"
        subprocess.run(
            [
                "/usr/bin/ssh-keygen",
                "-q",
                "-k",
                "-f",
                str(revoked),
                str(self.signing_key) + ".pub",
            ],
            check=True,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        revoked.chmod(0o400)
        with self.assertRaisesRegex(SIGNING.SigningError, "verification failed"):
            CHECKER.check_release(
                self.receipt_path,
                self._signature_inputs(
                    revocation_file=revoked,
                    expected_revocation_sha256=self._sha256(revoked),
                ),
            )

    def test_wrong_sshsig_namespace_fails(self) -> None:
        with tempfile.TemporaryDirectory(prefix="wrong-namespace.", dir=self.work) as raw:
            temporary = Path(raw)
            payload = temporary / "receipt.json"
            payload.write_bytes(self.receipt_bytes)
            subprocess.run(
                [
                    "/usr/bin/ssh-keygen",
                    "-Y",
                    "sign",
                    "-f",
                    str(self.signing_key),
                    "-n",
                    "wrong-sumeragi-replay-namespace",
                    str(payload),
                ],
                check=True,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            wrong_signature = Path(str(payload) + ".sig")
            wrong_signature.chmod(0o400)
            with self.assertRaisesRegex(SIGNING.SigningError, "verification failed"):
                CHECKER.check_release(
                    self.receipt_path,
                    self._signature_inputs(
                        signature=wrong_signature,
                        expected_signature_sha256=self._sha256(wrong_signature),
                    ),
                )

    def test_replace_restore_path_races_fail_closed(self) -> None:
        for target in (self.receipt_path, self.signature, self.ssh_keygen):
            with self.subTest(target=target.name):
                data = target.read_bytes()
                mode = stat.S_IMODE(target.stat().st_mode)
                backup = target.with_name(target.name + ".race-backup")
                original_run = SIGNING._run_verifier

                def raced_run(*args, **kwargs):
                    target.rename(backup)
                    target.write_bytes(data)
                    target.chmod(mode)
                    try:
                        return original_run(*args, **kwargs)
                    finally:
                        target.unlink()
                        backup.rename(target)

                SIGNING._run_verifier = raced_run
                try:
                    with self.assertRaisesRegex(SIGNING.SigningError, "changed"):
                        CHECKER.check_release(
                            self.receipt_path, self._signature_inputs()
                        )
                finally:
                    SIGNING._run_verifier = original_run
                    if backup.exists() and not target.exists():
                        backup.rename(target)
                    elif backup.exists():
                        backup.unlink()
                    target.chmod(mode)

    def test_staging_cleanup_refuses_rename_replacement(self) -> None:
        original_run = SIGNING._run_verifier
        paths: dict[str, Path] = {}

        def replaced_root(*args, **kwargs):
            result = original_run(*args, **kwargs)
            root = Path(kwargs["cwd"])
            moved = root.with_name("evidence-moved")
            root.rename(moved)
            root.mkdir(mode=0o700)
            victim = root / "must-survive"
            victim.write_bytes(b"replacement\n")
            victim.chmod(0o600)
            paths.update(root=root, moved=moved, victim=victim)
            return result

        SIGNING._run_verifier = replaced_root
        try:
            with self.assertRaisesRegex(
                SIGNING.SigningError, "cleanup refused changed ownership"
            ):
                CHECKER.check_release(
                    self.receipt_path, self._signature_inputs()
                )
            self.assertEqual(paths["victim"].read_bytes(), b"replacement\n")
            self.assertEqual(os.listdir(paths["moved"]), [])
        finally:
            SIGNING._run_verifier = original_run
            if paths:
                if paths["victim"].exists():
                    paths["victim"].unlink()
                if paths["root"].exists():
                    paths["root"].rmdir()
                if paths["moved"].exists():
                    paths["moved"].rmdir()
                paths["root"].parent.rmdir()

    def test_finalizer_rejects_unsafe_output_roots(self) -> None:
        with self.assertRaisesRegex(FINALIZER.FinalizationError, "absolute"):
            FINALIZER.finalize(self._finalizer_args(Path("relative-release")))
        with self.assertRaisesRegex(FINALIZER.FinalizationError, "outside"):
            FINALIZER.finalize(
                self._finalizer_args(ROOT / ".formal-release-must-not-exist")
            )
        with tempfile.TemporaryDirectory(
            prefix="public-parent.", dir=self.work
        ) as raw_parent:
            parent = Path(raw_parent).resolve()
            parent.chmod(0o755)
            try:
                with self.assertRaisesRegex(FINALIZER.FinalizationError, "0700"):
                    FINALIZER.finalize(
                        self._finalizer_args(parent / "release")
                    )
            finally:
                parent.chmod(0o700)

    def test_finalizer_detects_alternate_bundle_swap_restore(self) -> None:
        with tempfile.TemporaryDirectory(
            prefix="finalizer-swap-parent.", dir=self.work
        ) as raw_parent:
            parent = Path(raw_parent).resolve()
            output_root = parent / "release"
            alternate = parent / "alternate"
            alternate.mkdir(mode=0o700)
            for name, source, mode in (
                ("receipt.json", self.receipt_path, 0o400),
                ("receipt.json.sig", self.signature, 0o400),
                ("ssh-keygen.release-tool", self.ssh_keygen, 0o500),
                ("allowed_signers", self.allowed_signers, 0o400),
                ("revocation.krl", self.revocation, 0o400),
            ):
                target = alternate / name
                target.write_bytes(source.read_bytes())
                target.chmod(mode)

            original_run = SIGNING._run_verifier
            calls = 0

            def swap_restore(*args, **kwargs):
                nonlocal calls
                result = original_run(*args, **kwargs)
                calls += 1
                if calls == 2:
                    moved = parent / "release-original"
                    output_root.rename(moved)
                    alternate.rename(output_root)
                    output_root.rename(alternate)
                    moved.rename(output_root)
                return result

            SIGNING._run_verifier = swap_restore
            try:
                with self.assertRaisesRegex(
                    FINALIZER.FinalizationError, "parent.*changed|renamed"
                ):
                    FINALIZER.finalize(self._finalizer_args(output_root))
            finally:
                SIGNING._run_verifier = original_run
            self.assertEqual(
                (alternate / "receipt.json").read_bytes(), self.receipt_bytes
            )
            self.assertFalse(output_root.exists())

    def test_release_verifier_detects_alternate_bundle_swap_restore(self) -> None:
        with tempfile.TemporaryDirectory(
            prefix="verifier-swap-parent.", dir=self.work
        ) as raw_parent:
            parent = Path(raw_parent).resolve()
            output_root = parent / "release"
            FINALIZER.finalize(self._finalizer_args(output_root))
            alternate = parent / "alternate"
            alternate.mkdir(mode=0o700)
            for name, mode in RELEASE_VERIFIER.FILE_MODES.items():
                target = alternate / name
                target.write_bytes((output_root / name).read_bytes())
                target.chmod(mode)

            original_run = SIGNING._run_verifier

            def swap_restore(*args, **kwargs):
                result = original_run(*args, **kwargs)
                moved = parent / "release-original"
                output_root.rename(moved)
                alternate.rename(output_root)
                output_root.rename(alternate)
                moved.rename(output_root)
                return result

            SIGNING._run_verifier = swap_restore
            try:
                with self.assertRaisesRegex(
                    RELEASE_VERIFIER.ReleaseVerificationError,
                    "renamed|identity",
                ):
                    RELEASE_VERIFIER.verify_release(
                        self._release_verifier_args(output_root)
                    )
            finally:
                SIGNING._run_verifier = original_run
            self.assertEqual(
                (alternate / "receipt.json").read_bytes(), self.receipt_bytes
            )

    def test_release_attestation_tampering_is_rederived_and_rejected(self) -> None:
        with tempfile.TemporaryDirectory(
            prefix="attestation-parent.", dir=self.work
        ) as raw_parent:
            output_root = Path(raw_parent).resolve() / "release"
            marker = FINALIZER.finalize(self._finalizer_args(output_root))
            value = json.loads(marker.read_text(encoding="ascii"))
            value["signature"]["namespace"] = "wrong-namespace"
            marker.chmod(0o600)
            marker.write_bytes(FINALIZER.canonical_json(value))
            marker.chmod(0o400)
            with self.assertRaisesRegex(
                RELEASE_VERIFIER.ReleaseVerificationError,
                "independently derived",
            ):
                RELEASE_VERIFIER.verify_release(
                    self._release_verifier_args(output_root)
                )

    def test_non_v1_signing_contract_substitution_is_rejected(self) -> None:
        value = copy.deepcopy(self.receipt)
        value["signing"] = {
            "scheme": "embedded",
            "provider": "custom",
            "namespace": "wrong-namespace",
            "payload": "receipt.json",
            "artifact": "receipt.json.sig",
            "policy": {},
        }
        self.receipt_path.write_bytes(CHECKER.canonical_json(value))
        with self.assertRaisesRegex(CHECKER.ReceiptError, "contract"):
            CHECKER._check_structure(self.receipt_path)

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
            CHECKER._check_structure(self.receipt_path)
        artifact.write_bytes(original)
        unexpected = self.output / "unexpected"
        unexpected.write_bytes(b"unexpected\n")
        unexpected.chmod(0o600)
        with self.assertRaisesRegex(CHECKER.ReceiptError, "file set differs"):
            CHECKER._check_structure(self.receipt_path)
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
            CHECKER._check_structure(self.receipt_path)
        extra.unlink()
        self.receipt_path.write_bytes(self.receipt_bytes)

        empty = self.output / "empty"
        empty.mkdir(mode=0o700)
        with self.assertRaisesRegex(CHECKER.ReceiptError, "directory set differs"):
            CHECKER._check_structure(self.receipt_path)
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
