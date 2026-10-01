"""Durable exact-result register and same-source encoder request boundary."""

import hashlib
import os
import shutil
import sqlite3
import subprocess
import tempfile
import threading
import unittest
from pathlib import Path
from unittest.mock import patch

from iroha_app_attestation.attestation import (
    AttestationRejected, PREPARATION_DOMAIN, RawPlatformProof, Selection,
    device_key_reference,
)
from iroha_app_attestation.issuance import (
    CertificateFields,
    DurableCertificateStore,
    GovernedIssuanceScope,
    SIGNING_REQUEST_BYTES,
    SIGNING_REQUEST_MAGIC,
    encode_with_iroha,
)


def fields() -> CertificateFields:
    return CertificateFields(*(bytes([index]) * 32 for index in range(1, 11)), 1_000, 2_000)


class IssuanceTests(unittest.TestCase):
    def test_pinned_encoder_executes_verified_copy_after_configured_path_changes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            encoder = directory / "encoder"
            trusted = b"#!/bin/sh\ncat >/dev/null\nprintf original-certificate\n"
            encoder.write_bytes(trusted)
            encoder.chmod(0o700)
            request = fields().request(b"\x0a" * 32)
            key_read_fd, key_write_fd = os.pipe()
            real_run = subprocess.run
            def replace_configured_path(arguments, **kwargs):
                self.assertNotEqual(Path(arguments[0]), encoder)
                self.assertEqual(Path(arguments[0]).read_bytes(), trusted)
                encoder.write_bytes(b"#!/bin/sh\ncat >/dev/null\nprintf replaced\n")
                return real_run(arguments, **kwargs)
            try:
                with patch("iroha_app_attestation.issuance.subprocess.run", side_effect=replace_configured_path):
                    actual = encode_with_iroha(
                        request, encoder, hashlib.sha256(trusted).digest(), key_read_fd,
                    )
                self.assertEqual(actual, b"original-certificate")
                with self.assertRaises(AttestationRejected):
                    encode_with_iroha(request, encoder, hashlib.sha256(trusted).digest(),
                                      key_read_fd)
            finally:
                os.close(key_read_fd)
                os.close(key_write_fd)

    def test_public_issuance_requires_signed_preparation_and_exact_raw_key_binding(self) -> None:
        executable = shutil.which("openssl")
        if executable is None:
            self.skipTest("OpenSSL CLI unavailable")
        openssl = Path(executable).resolve()
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            os.chmod(directory, 0o700)

            def run(*arguments: str) -> None:
                subprocess.run([str(openssl), *arguments], cwd=directory,
                               capture_output=True, check=True)

            run("genpkey", "-algorithm", "ED25519", "-out", "issuer.pem")
            run("pkey", "-in", "issuer.pem", "-pubout", "-outform", "DER", "-out", "issuer.der")
            issuer_spki = (directory / "issuer.der").read_bytes()
            point = b"\x04" + b"\x51" * 64  # fabricated raw-result fixture; not an attestation
            evidence = b"synthetic raw-evidence fixture"
            proof = RawPlatformProof(hashlib.sha256(evidence).digest(), point,
                                     device_key_reference(point), "apple_app_attest")
            original = fields()
            actual = dict(vars(original))
            actual["platform_evidence_digest"] = proof.evidence_sha256
            actual["device_key_reference"] = proof.device_key_reference
            actual["attested_key_id"] = hashlib.sha256(point).digest()
            value = CertificateFields(**actual)
            selected = Selection(value.client_nonce, value.server_nonce, value.release_id,
                                 value.hardware_profile_id, hashlib.sha256(point).digest(),
                                 value.lane_id)
            policy_id = b"\x7a" * 32
            account = "owner"
            header = (b"\x01" + (1_000).to_bytes(8, "little")
                      + (121_000).to_bytes(8, "little") + selected.client_nonce
                      + selected.server_nonce + selected.release_id
                      + selected.hardware_profile_id + selected.attested_key_id
                      + selected.lane_id)
            (directory / "message.bin").write_bytes(
                PREPARATION_DOMAIN + header[1:] + policy_id
                + hashlib.sha256(account.encode()).digest()
            )
            run("pkeyutl", "-sign", "-inkey", "issuer.pem", "-rawin",
                "-in", "message.bin", "-out", "signature.bin")
            token = header + (directory / "signature.bin").read_bytes()
            scope = GovernedIssuanceScope(
                selected, account, policy_id, issuer_spki,
                hashlib.sha256(issuer_spki).digest(), 1_500, openssl,
                "apple_app_attest", proof, value.release_id, value.hardware_profile_id,
                value.lane_id, value.app_signing_identity_digest,
                value.app_release_digest, 10_000, b"\x0a" * 32, value,
            )
            store = DurableCertificateStore(directory / "certificates.sqlite")
            calls = []

            def signer(request: bytes) -> bytes:
                calls.append(request)
                return b"canonical-certificate-fixture"

            original_result = store.issue_once(scope, token, evidence, signer,
                                               lambda: scope)
            self.assertEqual(original_result, b"canonical-certificate-fixture")
            self.assertEqual(calls, [value.request(b"\x0a" * 32)])
            late = dict(vars(scope))
            late["trusted_time_ms"] = 121_001
            self.assertEqual(store.issue_once(GovernedIssuanceScope(**late), token, evidence,
                                              lambda _: self.fail("resigned"),
                                              lambda: self.fail("revalidated existing result")),
                             original_result)
            with self.assertRaises(AttestationRejected):
                DurableCertificateStore(directory / "other.sqlite").issue_once(
                    GovernedIssuanceScope(**late), token, evidence, signer, lambda: scope)
            for mutation in (
                {"selection": Selection(selected.client_nonce, selected.server_nonce,
                                        selected.release_id, selected.hardware_profile_id,
                                        b"\x01" * 32, selected.lane_id)},
                {"governed_app_release_digest": b"\x0b" * 32},
                {"governed_release_id": b"\x0b" * 32},
                {"governed_max_lifetime_ms": 999},
                {"issuer_public_spki_sha256": b"\x0b" * 32},
                {"fields": CertificateFields(**{**vars(value),
                                                  "attested_key_id": b"\x0b" * 32})},
            ):
                changed = dict(vars(scope))
                changed.update(mutation)
                with self.subTest(mutation=mutation), self.assertRaises(AttestationRejected):
                    store.issue_once(GovernedIssuanceScope(**changed), token, evidence,
                                     signer, lambda: scope)
            tampered = token[:-1] + bytes([token[-1] ^ 1])
            with self.assertRaises(AttestationRejected):
                store.issue_once(scope, tampered, evidence, signer, lambda: scope)
            with self.assertRaises(AttestationRejected):
                store.issue_once(scope, token, evidence + b"x", signer, lambda: scope)

            # The first check can pass before waiting on SQLite's write lock.
            # Expiry or governance drift observed under the lock must stop the
            # signer, while a fresh exact retry can still publish once.
            queued = DurableCertificateStore(directory / "queued.sqlite")
            expired = dict(vars(scope))
            expired["trusted_time_ms"] = 121_000
            with self.assertRaises(AttestationRejected):
                queued.issue_once(scope, token, evidence,
                                  lambda _: self.fail("signed after expiry"),
                                  lambda: GovernedIssuanceScope(**expired))
            changed_policy = dict(vars(scope))
            changed_policy["governed_app_release_digest"] = b"\x0b" * 32
            with self.assertRaises(AttestationRejected):
                queued.issue_once(scope, token, evidence,
                                  lambda _: self.fail("signed after policy drift"),
                                  lambda: GovernedIssuanceScope(**changed_policy))
            self.assertEqual(queued.issue_once(scope, token, evidence, signer,
                                                lambda: scope), original_result)

            # Hold a real writer lock while the request passes its first check.
            # Only the recheck after that lock is released can see the expiry.
            contended = DurableCertificateStore(directory / "contended.sqlite")
            holder = sqlite3.connect(contended.path, isolation_level=None)
            holder.execute("BEGIN IMMEDIATE")
            entered_store = threading.Event()
            trusted_time = [1_500]
            failures: list[Exception] = []
            signed: list[bytes] = []
            original_issue = contended._issue_once

            def issue_after_entry(*args):
                entered_store.set()
                return original_issue(*args)

            contended._issue_once = issue_after_entry

            def queued_issue() -> None:
                try:
                    fresh = lambda: GovernedIssuanceScope(**{
                        **vars(scope), "trusted_time_ms": trusted_time[0],
                    })
                    contended.issue_once(scope, token, evidence,
                                         lambda frame: signed.append(frame) or b"certificate",
                                         fresh)
                except Exception as error:
                    failures.append(error)

            worker = threading.Thread(target=queued_issue)
            try:
                worker.start()
                self.assertTrue(entered_store.wait(2), "issuer did not reach locked store")
                trusted_time[0] = 121_000
            finally:
                holder.rollback()
                holder.close()
                worker.join(3)
            self.assertFalse(worker.is_alive(), "issuer remained blocked")
            self.assertEqual(len(failures), 1)
            self.assertIsInstance(failures[0], AttestationRejected)
            self.assertEqual(signed, [])

    def test_encoder_request_binds_all_fields_and_selection(self) -> None:
        value = fields()
        request = value.request(b"\x0a" * 32)
        self.assertEqual(len(request), SIGNING_REQUEST_BYTES)
        self.assertTrue(request.startswith(SIGNING_REQUEST_MAGIC))
        self.assertEqual(request[5:37], b"\x01" * 32)
        self.assertEqual(request[5 + 8 * 32:5 + 9 * 32], value.attested_key_id)
        self.assertEqual(request[5 + 9 * 32:5 + 10 * 32], value.lane_id)
        self.assertEqual(request[-32:], b"\x0a" * 32)
        selected = Selection(value.client_nonce, value.server_nonce, value.release_id,
                             value.hardware_profile_id, b"\0" * 32, value.lane_id)
        value.matches_selection(selected, value.device_key_reference)
        with self.assertRaises(AttestationRejected):
            value.matches_selection(selected, b"\x0b" * 32)
        changed = dict(vars(value))
        changed["expires_at_ms"] = 121_001
        with self.assertRaises(AttestationRejected):
            CertificateFields(**changed).request(b"\x0a" * 32)
        changed = dict(vars(value))
        changed["client_nonce"] = value.server_nonce
        with self.assertRaises(AttestationRejected):
            CertificateFields(**changed).request(b"\x0a" * 32)
        changed = dict(vars(value))
        changed["attested_key_id"] = bytes(32)
        with self.assertRaises(AttestationRejected):
            CertificateFields(**changed).request(b"\x0a" * 32)

    def test_exact_recovery_conflicts_crash_and_reopen(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            os.chmod(directory, 0o700)
            path = directory / "certificates.sqlite"
            store = DurableCertificateStore(path)
            token = b"\x01" + b"\x11" * 272
            request = fields().request(b"\x0a" * 32)
            evidence = b"retained checked raw evidence"
            calls = []

            def signer(frame: bytes) -> bytes:
                calls.append(frame)
                return b"canonical-original-certificate"

            self.assertEqual(store._issue_once(token, request, evidence, signer),
                             b"canonical-original-certificate")
            reopened = DurableCertificateStore(path)
            self.assertEqual(reopened._issue_once(token, request, evidence, lambda _: self.fail("resigned")),
                             b"canonical-original-certificate")
            self.assertEqual(calls, [request])
            for changed_request, changed_evidence in (
                (request[:-1] + b"\x0b", evidence),
                (request, evidence + b"x"),
            ):
                with self.assertRaises(AttestationRejected):
                    reopened._issue_once(token, changed_request, changed_evidence,
                                        lambda _: self.fail("conflict signed"))

            failed_token = b"\x01" + b"\x22" * 272
            with self.assertRaises(AttestationRejected):
                reopened._issue_once(
                    failed_token, request, evidence,
                    lambda _: self.fail("same attested key signed twice"),
                )
            second_fields = dict(vars(fields()))
            second_fields["device_key_reference"] = b"\x15" * 32
            second_request = CertificateFields(**second_fields).request(b"\x0a" * 32)
            def crash(_: bytes) -> bytes:
                raise RuntimeError("signer died")
            with self.assertRaisesRegex(RuntimeError, "signer died"):
                reopened._issue_once(failed_token, second_request, evidence, crash)
            self.assertEqual(reopened._issue_once(failed_token, second_request, evidence, signer),
                             b"canonical-original-certificate")
            self.assertEqual(len(calls), 2)

    def test_concurrent_retries_call_signer_once(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            os.chmod(directory, 0o700)
            store = DurableCertificateStore(directory / "certificates.sqlite")
            token = b"\x01" + b"\x33" * 272
            request = fields().request(b"\x0a" * 32)
            calls = []
            outputs = []

            def signer(frame: bytes) -> bytes:
                calls.append(hashlib.sha256(frame).digest())
                return b"canonical-original-certificate"

            def worker() -> None:
                outputs.append(store._issue_once(token, request, b"evidence", signer))

            threads = [threading.Thread(target=worker) for _ in range(8)]
            for thread in threads:
                thread.start()
            for thread in threads:
                thread.join()
            self.assertEqual(len(calls), 1)
            self.assertEqual(outputs, [b"canonical-original-certificate"] * 8)

    def test_store_rejects_public_directory_or_symlink(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            os.chmod(directory, 0o755)
            with self.assertRaises(AttestationRejected):
                DurableCertificateStore(directory / "certificates.sqlite")
            os.chmod(directory, 0o700)
            target = directory / "target"
            target.write_bytes(b"other")
            (directory / "certificates.sqlite").symlink_to(target)
            with self.assertRaises(AttestationRejected):
                DurableCertificateStore(directory / "certificates.sqlite")


if __name__ == "__main__":
    unittest.main()
