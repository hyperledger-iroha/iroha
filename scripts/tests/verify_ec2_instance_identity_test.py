#!/usr/bin/env python3
"""Focused tests for the fail-closed EC2 IID verifier."""

from __future__ import annotations

import base64
import contextlib
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock


SCRIPT = Path(__file__).resolve().parents[1] / "verify_ec2_instance_identity.py"
SPEC = importlib.util.spec_from_file_location("verify_ec2_instance_identity", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def iid_document(**updates: object) -> bytes:
    value: dict[str, object] = {
        "accountId": "123456789012",
        "architecture": "arm64",
        "availabilityZone": "ap-northeast-1a",
        "billingProducts": None,
        "devpayProductCodes": None,
        "marketplaceProductCodes": None,
        "imageId": "ami-0123456789abcdef0",
        "instanceId": "i-0123456789abcdef0",
        "instanceType": "c7g.4xlarge",
        "pendingTime": "2026-08-26T00:00:00Z",
        "privateIp": "10.0.0.42",
        "region": "ap-northeast-1",
        "version": "2017-09-30",
    }
    value.update(updates)
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def bounded_test_command(source: str, *arguments: Path | str) -> list[str]:
    return [
        sys.executable,
        "-I",
        "-S",
        "-c",
        source,
        *(os.fspath(argument) for argument in arguments),
    ]


def bounded_test_environment() -> dict[str, str]:
    return {"LANG": "C", "LC_ALL": "C", "PATH": "/usr/bin:/bin"}


class BoundedProcessRunnerTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    @unittest.skipUnless(
        sys.platform.startswith("linux"),
        "bounded process execution is intentionally Linux-only",
    )
    def test_rejects_stdout_and_stderr_floods(self) -> None:
        for descriptor, stream in ((1, "stdout"), (2, "stderr")):
            with self.subTest(stream=stream), self.assertRaisesRegex(
                MODULE._BoundedProcessError, f"{stream} exceeded"
            ):
                MODULE._run_bounded_process(
                    bounded_test_command(
                        f"import os; os.write({descriptor}, b'x' * 8192)"
                    ),
                    environment=bounded_test_environment(),
                    timeout=5,
                    stdout_limit=1024,
                    stderr_limit=1024,
                )

    @unittest.skipUnless(
        sys.platform.startswith("linux"),
        "bounded process execution is intentionally Linux-only",
    )
    def test_contains_setsid_descendant_after_parent_success(self) -> None:
        escape_marker = self.root / "escaped-descendant"
        command = bounded_test_command(
            "\n".join(
                (
                    "import os, sys, time",
                    "pid = os.fork()",
                    "if pid == 0:",
                    "    os.setsid()",
                    "    for descriptor in (0, 1, 2):",
                    "        try:",
                    "            os.close(descriptor)",
                    "        except OSError:",
                    "            pass",
                    "    time.sleep(0.25)",
                    "    fd = os.open(sys.argv[1], os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)",
                    "    os.write(fd, b'escaped')",
                    "    os.close(fd)",
                    "    os._exit(0)",
                    "os._exit(0)",
                )
            ),
            escape_marker,
        )
        completed = MODULE._run_bounded_process(
            command,
            environment=bounded_test_environment(),
            timeout=2,
            stdout_limit=1024,
            stderr_limit=4096,
        )
        self.assertEqual(completed.returncode, 0)
        time.sleep(0.5)
        self.assertFalse(
            escape_marker.exists(),
            "a detached subprocess descendant outlived the bounded runner",
        )

    @unittest.skipUnless(
        sys.platform.startswith("linux"),
        "the direct-target namespace teardown attack is Linux-specific",
    )
    def test_contains_direct_target_after_timeout(self) -> None:
        armed = self.root / "armed"
        trigger = self.root / "trigger"
        escape_marker = self.root / "escaped-target"
        command = bounded_test_command(
            "\n".join(
                (
                    "import ctypes, os, sys, time",
                    "if ctypes.CDLL(None).prctl(1, 0, 0, 0, 0) != 0:",
                    "    os._exit(91)",
                    "os.setsid()",
                    "fd = os.open(sys.argv[1], os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)",
                    "os.write(fd, b'armed')",
                    "os.close(fd)",
                    "for descriptor in (0, 1, 2):",
                    "    try:",
                    "        os.close(descriptor)",
                    "    except OSError:",
                    "        pass",
                    "while not os.path.exists(sys.argv[2]):",
                    "    time.sleep(0.01)",
                    "fd = os.open(sys.argv[3], os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)",
                    "os.write(fd, b'escaped')",
                    "os.close(fd)",
                )
            ),
            armed,
            trigger,
            escape_marker,
        )
        with self.assertRaisesRegex(MODULE._BoundedProcessError, "timed out"):
            MODULE._run_bounded_process(
                command,
                environment=bounded_test_environment(),
                timeout=0.5,
                stdout_limit=1024,
                stderr_limit=1024,
            )
        self.assertTrue(armed.exists(), "the direct target did not reach its escape posture")
        trigger.write_bytes(b"go")
        time.sleep(0.25)
        self.assertFalse(
            escape_marker.exists(),
            "a direct target escaped namespace teardown after timeout",
        )

    @unittest.skipUnless(
        sys.platform.startswith("linux"),
        "bounded process execution is intentionally Linux-only",
    )
    def test_preserves_signal_status(self) -> None:
        for termination_signal in (signal.SIGINT, signal.SIGTERM, signal.SIGKILL):
            with self.subTest(termination_signal=termination_signal):
                completed = MODULE._run_bounded_process(
                    bounded_test_command(
                        f"import os; os.kill(os.getpid(), {int(termination_signal)})"
                    ),
                    environment=bounded_test_environment(),
                    timeout=5,
                    stdout_limit=1024,
                    stderr_limit=1024,
                )
                self.assertEqual(completed.returncode, -int(termination_signal))

    @unittest.skipUnless(
        sys.platform.startswith("linux"),
        "the exec handshake is implemented by the Linux containment supervisor",
    )
    def test_rejects_failed_exec_handshake(self) -> None:
        with self.assertRaisesRegex(
            MODULE._BoundedProcessError,
            "OS-enforced descendant containment",
        ):
            MODULE._run_bounded_process(
                ["/definitely/missing-zk-x509-target"],
                environment=bounded_test_environment(),
                timeout=5,
                stdout_limit=1024,
                stderr_limit=1024,
            )

    def test_fails_before_exec_on_unsupported_host(self) -> None:
        marker = self.root / "ran"
        with mock.patch.object(sys, "platform", "unsupported-zk-x509-host"), self.assertRaisesRegex(
            MODULE._BoundedProcessError,
            "requires Linux user and PID namespaces",
        ):
            MODULE._run_bounded_process(
                bounded_test_command(
                    "import pathlib, sys; pathlib.Path(sys.argv[1]).write_bytes(b'ran')",
                    marker,
                ),
                environment=bounded_test_environment(),
                timeout=5,
                stdout_limit=1024,
                stderr_limit=1024,
            )
        self.assertFalse(marker.exists())

    @unittest.skipUnless(
        sys.platform.startswith("linux"),
        "the unavailable namespace bootstrap is Linux-specific",
    )
    def test_never_execs_if_linux_containment_bootstrap_fails(self) -> None:
        marker = self.root / "ran"
        with mock.patch.object(
            MODULE,
            "_LINUX_PID_NAMESPACE_SUPERVISOR",
            "import os; os._exit(125)",
        ), self.assertRaisesRegex(
            MODULE._BoundedProcessError,
            "could not be established",
        ):
            MODULE._run_bounded_process(
                bounded_test_command(
                    "import pathlib, sys; pathlib.Path(sys.argv[1]).write_bytes(b'ran')",
                    marker,
                ),
                environment=bounded_test_environment(),
                timeout=5,
                stdout_limit=1024,
                stderr_limit=1024,
            )
        self.assertFalse(marker.exists())


@unittest.skipUnless(
    sys.platform.startswith("linux") and shutil.which("openssl"),
    "Linux and OpenSSL are required for the production verifier corridor",
)
class InstanceIdentityVerifierTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve()
        os.chmod(self.root, 0o700)
        self.openssl = Path(shutil.which("openssl") or "").resolve(strict=True)
        self.openssl_digest = digest(self.openssl)
        self.certificate, self.key = self._new_identity("fixture-one")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _run(self, *arguments: str) -> None:
        subprocess.run(
            [str(self.openssl), *arguments],
            check=True,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            env={"LANG": "C", "LC_ALL": "C", "PATH": "/usr/bin:/bin"},
        )

    def _new_identity(self, name: str) -> tuple[Path, Path]:
        certificate = self.root / f"{name}.pem"
        key = self.root / f"{name}.key"
        self._run(
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-days",
            "2",
            "-subj",
            f"/CN={name}",
            "-keyout",
            str(key),
            "-out",
            str(certificate),
        )
        os.chmod(key, 0o600)
        os.chmod(certificate, 0o600)
        return certificate, key

    def _sign(self, document: bytes, certificate: Path | None = None, key: Path | None = None) -> bytes:
        certificate = self.certificate if certificate is None else certificate
        key = self.key if key is None else key
        document_path = self.root / "document-to-sign.json"
        cms_path = self.root / "document.cms"
        document_path.write_bytes(document)
        self._run(
            "cms",
            "-sign",
            "-binary",
            "-nodetach",
            "-nosmimecap",
            "-in",
            str(document_path),
            "-signer",
            str(certificate),
            "-inkey",
            str(key),
            "-outform",
            "DER",
            "-out",
            str(cms_path),
        )
        return base64.b64encode(cms_path.read_bytes())

    def _verify(
        self,
        document: bytes,
        signature: bytes,
        certificate: Path | None = None,
    ) -> dict[str, object]:
        certificate = self.certificate if certificate is None else certificate
        return MODULE.verify_instance_identity(
            document,
            signature,
            expected_region="ap-northeast-1",
            certificate_path=certificate,
            certificate_sha256=digest(certificate),
            openssl_path=self.openssl,
            openssl_sha256=self.openssl_digest,
        )

    def test_authenticates_exact_content_with_oob_certificate(self) -> None:
        document = iid_document()
        verified = self._verify(document, self._sign(document))
        self.assertTrue(verified["verified"])
        self.assertEqual(verified["document_sha256"], hashlib.sha256(document).hexdigest())
        self.assertEqual(verified["document"]["instanceType"], "c7g.4xlarge")

    def test_executes_immutable_openssl_snapshot_after_original_path_replacement(self) -> None:
        document = iid_document()
        signature = self._sign(document)
        candidate = self.root / "replaceable-openssl"
        shutil.copyfile(self.openssl, candidate)
        candidate.chmod(0o700)
        candidate_digest = digest(candidate)
        original_snapshot = MODULE._immutable_input_snapshot
        replaced = False

        @contextlib.contextmanager
        def replace_after_hash(
            root: Path,
            name: str,
            payload: bytes,
            *,
            executable: bool,
        ):
            nonlocal replaced
            if executable and not replaced:
                candidate.write_bytes(b"replaced after its pinned bytes were captured")
                candidate.chmod(0o700)
                replaced = True
            with original_snapshot(
                root, name, payload, executable=executable
            ) as snapshot:
                yield snapshot

        with mock.patch.object(
            MODULE, "_immutable_input_snapshot", replace_after_hash
        ):
            verified = MODULE.verify_instance_identity(
                document,
                signature,
                expected_region="ap-northeast-1",
                certificate_path=self.certificate,
                certificate_sha256=digest(self.certificate),
                openssl_path=candidate,
                openssl_sha256=candidate_digest,
            )
        self.assertTrue(replaced)
        self.assertTrue(verified["verified"])
        self.assertEqual(verified["openssl_path"], str(candidate))

    def test_rejects_certificate_pin_mismatch(self) -> None:
        document = iid_document()
        with self.assertRaisesRegex(MODULE.VerificationError, "does not match the OOB pin"):
            MODULE.verify_instance_identity(
                document,
                self._sign(document),
                expected_region="ap-northeast-1",
                certificate_path=self.certificate,
                certificate_sha256="0" * 64,
                openssl_path=self.openssl,
                openssl_sha256=self.openssl_digest,
            )

    def test_embedded_signer_cannot_replace_oob_certificate(self) -> None:
        other_certificate, _ = self._new_identity("fixture-two")
        document = iid_document()
        with self.assertRaisesRegex(MODULE.VerificationError, "signature verification failed"):
            self._verify(document, self._sign(document), other_certificate)

    def test_rejects_document_or_signature_mutation(self) -> None:
        document = iid_document()
        signature = self._sign(document)
        changed = document.replace(b"10.0.0.42", b"10.0.0.43")
        with self.assertRaisesRegex(MODULE.VerificationError, "not byte-for-byte equal"):
            self._verify(changed, signature)
        corrupt = base64.b64decode(signature)[:-1]
        with self.assertRaisesRegex(
            MODULE.VerificationError,
            "OpenSSL verification command failed|signature verification failed",
        ):
            self._verify(document, base64.b64encode(corrupt))

    def test_rejects_duplicate_and_wrong_identity_fields(self) -> None:
        duplicate = (
            b'{"accountId":"123456789012","accountId":"123456789012",'
            b'"architecture":"arm64","availabilityZone":"ap-northeast-1a",'
            b'"imageId":"ami-0123456789abcdef0","instanceId":"i-0123456789abcdef0",'
            b'"instanceType":"c7g.4xlarge","pendingTime":"2026-08-26T00:00:00Z",'
            b'"privateIp":"10.0.0.42","region":"ap-northeast-1","version":"1"}'
        )
        with self.assertRaisesRegex(MODULE.VerificationError, "duplicate JSON key"):
            self._verify(duplicate, self._sign(duplicate))
        wrong_type = iid_document(instanceType="c7g.2xlarge")
        with self.assertRaisesRegex(MODULE.VerificationError, "expected IID instanceType"):
            self._verify(wrong_type, self._sign(wrong_type))
        wrong_region = iid_document(region="us-east-1", availabilityZone="us-east-1a")
        with self.assertRaisesRegex(MODULE.VerificationError, "expected IID region"):
            self._verify(wrong_region, self._sign(wrong_region))

    def test_cli_outputs_are_create_new_and_rolled_back_as_a_set(self) -> None:
        document = iid_document()
        signature = self._sign(document)
        document_out = self.root / "raw-document.json"
        signature_out = self.root / "raw-signature.txt"
        verified_out = self.root / "verified.json"
        arguments = [
            "--region",
            "ap-northeast-1",
            "--certificate",
            str(self.certificate),
            "--certificate-sha256",
            digest(self.certificate),
            "--openssl",
            str(self.openssl),
            "--openssl-sha256",
            self.openssl_digest,
            "--document-out",
            str(document_out),
            "--signature-out",
            str(signature_out),
            "--verified-out",
            str(verified_out),
        ]
        with mock.patch.object(MODULE, "fetch_from_imdsv2", return_value=(document, signature)):
            self.assertEqual(MODULE.main(arguments), 0)
            self.assertEqual(document_out.read_bytes(), document)
            self.assertEqual(signature_out.read_bytes(), signature)
            self.assertEqual(stat_mode(verified_out), 0o600)
            self.assertEqual(MODULE.main(arguments), 1)
        self.assertEqual(document_out.read_bytes(), document)


def stat_mode(path: Path) -> int:
    return path.stat().st_mode & 0o777


if __name__ == "__main__":
    unittest.main()
