"""Real synthetic AppAttest signatures and durable private-process protocol regressions."""
import base64
from dataclasses import replace
import hashlib
import io
import importlib.util
import json
import os
import shutil
import sqlite3
import subprocess
import sys
import urllib.parse
from contextlib import ExitStack
import tempfile
import time
import unittest
import zipfile
from pathlib import Path
from unittest.mock import patch

from iroha_app_attestation.attestation import AttestationRejected
from iroha_app_attestation.wallet_enrollment_worker import (
    CONFIG_SCHEMA, SCHEMA, MAX_PACKET, MAX_REQUEST, MAX_ORIGINAL, EXCHANGE_WINDOW, VerifierOwner,
    E1CounterStore, configured_policy, encode, exact_json, read_packet, serve,
)
from test_wallet_enrollment import (GENERATOR, scope, assertion, apple_object,
                                    configured_policy as fixture_policy, policy_scope, synthetic_root)
from test_synthetic_platform_evidence import SignedEnvelope, keymint_description
from iroha_app_attestation import openssl_private_rsa as crypto, play_integrity as pi, revocation
from iroha_app_attestation.attestation import ANDROID_KEY_DESCRIPTION_OID, Selection, VerificationUnavailable
from iroha_app_attestation.google_oauth import GoogleServiceAccountTokenProvider, TOKEN_URI, OAUTH_SCOPE, POLICY_SCHEMA
from test_google_oauth import SyntheticCodeOriginal, EMAIL, CLIENT_ID, PROJECT


class WorkerTests(unittest.TestCase):
    def setUp(self):
        binary = shutil.which("openssl")
        if binary is None:
            self.skipTest("OpenSSL3 unavailable")
        self.openssl = Path(binary).resolve()

    def owner_fixture(self, directory, challenge_lifetime_ms=120000, *, initialize=True):
        directory = directory.resolve(strict=True)
        os.chmod(directory, 0o700)
        fixture = SignedEnvelope(directory, self.openssl)
        policy = fixture_policy(synthetic_root(fixture), True)
        policy = replace(policy, enrollment=replace(policy.enrollment, challenge_lifetime_ms=challenge_lifetime_ms))
        selected = policy_scope(policy, GENERATOR)
        attestation, key_id, root = apple_object(fixture, selected)
        now = int(time.time() * 1000) + 60000
        request = encode({"challenge_transcript_base64": base64.b64encode(selected.challenge_transcript).decode(),
            "payment_key_base64": base64.b64encode(GENERATOR).decode(),
            "issued_at_ms": now - 1, "expires_at_ms": now - 1 + challenge_lifetime_ms,
            "trusted_time_ms": now, "platform": "apple", "evidence": {
                "attestation_base64": base64.b64encode(attestation).decode(),
                "assertion_base64": base64.b64encode(assertion(fixture, selected.enrollment_key_binding(), 1)).decode(),
                "key_id_hex": key_id.hex()}})
        config = encode({"schema": CONFIG_SCHEMA, "version": 1, "platform": "apple",
            "app_policy_hex": policy.app.policy_digest().hex(),
            "enrollment_policy_hex": policy.enrollment.policy_digest().hex(),
            "openssl_path": str(self.openssl), "openssl_sha256": hashlib.sha256(self.openssl.read_bytes()).hexdigest(),
            "store_directory": str(directory), "policy": {"app_id": "TEAMID.example.app",
                "scheme_id_hex": policy.enrollment.scheme_id.hex(), "asset_digest_hex": policy.enrollment.asset_digest.hex(),
                "regulatory_policy": {"permitted_controls": 0, "blacklist_max_age_ms": 0,
                                      "time_anchor_max_response_ms": 0},
                "challenge_lifetime_ms": challenge_lifetime_ms, "attestation_lease_lifetime_ms": 0,
                "root_base64": base64.b64encode(root).decode(), "root_sha256": hashlib.sha256(root).hexdigest()}})
        directory_fd = os.open(directory, os.O_RDONLY)
        crypto_fd = os.open(self.openssl, os.O_RDONLY)
        self.addCleanup(os.close, directory_fd)
        self.addCleanup(os.close, crypto_fd)
        if initialize:
            E1CounterStore.initialize(directory, directory_fd).close()
        def open_owner(original=config, **arguments):
            return VerifierOwner(original, directory_fd=directory_fd, crypto_fd=crypto_fd, **arguments)
        open_owner.config = config
        return open_owner, request, selected, key_id

    def test_selected_lifetime_has_no_unconfigured_ten_minute_ceiling(self):
        with tempfile.TemporaryDirectory() as temporary:
            open_owner, request, selected, _ = self.owner_fixture(Path(temporary), 600001)
            owner = open_owner()
            try:
                original = owner.perform(request, "verify")
                self.assertEqual(exact_json(original, MAX_PACKET)["challenge_digest"],
                                 selected.challenge_digest().hex())
                self.assertEqual(owner.perform(request, "recover"), original)
            finally:
                owner.close()

    def test_selected_lifetime_keeps_strict_u64_and_half_open_time_bounds(self):
        with tempfile.TemporaryDirectory() as temporary:
            open_owner, request, _, _ = self.owner_fixture(Path(temporary), 1 << 63)
            owner = open_owner()
            try:
                value = exact_json(request, MAX_PACKET)
                for now in (value["issued_at_ms"], value["expires_at_ms"] - 1):
                    original = encode(dict(value, trusted_time_ms=now))
                    self.assertEqual(owner.request(original)[0]["trusted_time_ms"], now)
                for changed in (
                    dict(value, trusted_time_ms=value["issued_at_ms"] - 1),
                    dict(value, trusted_time_ms=value["expires_at_ms"]),
                    dict(value, issued_at_ms=1 << 63, expires_at_ms=1 << 64,
                         trusted_time_ms=(1 << 63) + 1),
                    dict(value, expires_at_ms=value["expires_at_ms"] + 1),
                    dict(value, trusted_time_ms=True),
                ):
                    with self.assertRaises(AttestationRejected):
                        owner.request(encode(changed))
            finally:
                owner.close()

    def test_real_apple_result_and_counter_commit_and_recover_after_restart(self):
        with tempfile.TemporaryDirectory() as temporary:
            open_owner, request, selected, key_id = self.owner_fixture(Path(temporary))
            owner = open_owner()
            original = owner.perform(request, "verify")
            projection = exact_json(original, MAX_PACKET)
            self.assertEqual(projection["key_binding"], selected.enrollment_key_binding().hex())
            self.assertEqual(projection["kind_tag"], 3)
            self.assertEqual(projection["facts"], 448)
            self.assertEqual(projection["app_attest_counter"], 1)
            self.assertEqual(projection["app_attest_key_id"], key_id.hex())
            owner.close()
            restarted = open_owner()
            with patch("iroha_app_attestation.wallet_enrollment_worker.verify_apple_wallet_attestation_raw",
                       side_effect=AssertionError("recovery must not verify again")):
                self.assertEqual(restarted.perform(request, "recover"), original)
            with self.assertRaises(AttestationRejected):
                restarted.perform(request, "verify")
            with sqlite3.connect(Path(temporary) / "wallet-e1.sqlite3") as connection:
                self.assertEqual(connection.execute("SELECT counter FROM apple_keys WHERE key_id=?", (key_id,)).fetchone(), (1,))
                self.assertEqual(connection.execute("SELECT count(*) FROM apple_client_data").fetchone(), (1,))
            restarted.close()

    def test_real_apple_commit_survives_lost_private_pipe(self):
        class LostOutput(io.BytesIO):
            def write(self, _):
                raise BrokenPipeError()
        with tempfile.TemporaryDirectory() as temporary:
            open_owner, request, _, _ = self.owner_fixture(Path(temporary))
            owner = open_owner()
            packet = encode({"schema": SCHEMA, "version": 1, "exchange_id": "31" * 32,
                             "action": "verify", "original_base64": base64.b64encode(request).decode()})
            with self.assertRaises(BrokenPipeError):
                serve(owner, io.BytesIO(len(packet).to_bytes(4, "little") + packet), LostOutput())
            owner.close()
            restarted = open_owner()
            self.assertIsNotNone(restarted.perform(request, "recover"))
            restarted.close()

    def test_unseen_recovery_is_unknown_and_never_verifies_or_inserts_an_attempt(self):
        with tempfile.TemporaryDirectory() as temporary:
            open_owner, request, _, _ = self.owner_fixture(Path(temporary))
            owner = open_owner()
            self.addCleanup(owner.close)
            packet = encode({"schema": SCHEMA, "version": 1, "exchange_id": "32" * 32,
                             "action": "recover", "original_base64": base64.b64encode(request).decode()})
            output = io.BytesIO()
            with patch("iroha_app_attestation.wallet_enrollment_worker.verify_apple_wallet_attestation_raw",
                       side_effect=AssertionError("recovery must not dispatch verification")):
                serve(owner, io.BytesIO(len(packet).to_bytes(4, "little") + packet), output)
            response = exact_json(read_packet(io.BytesIO(output.getvalue())), MAX_PACKET)
            self.assertEqual(response["outcome"], "outcome_unknown")
            self.assertIsNone(response["evidence_base64"])
            with sqlite3.connect(Path(temporary) / "wallet-e1.sqlite3") as connection:
                self.assertEqual(connection.execute("SELECT count(*) FROM wallet_e1_attempts").fetchone(), (0,))
                self.assertEqual(connection.execute("SELECT count(*) FROM apple_client_data").fetchone(), (0,))

    def test_failed_apple_assertion_retains_attempt_without_counter_consumption(self):
        with tempfile.TemporaryDirectory() as temporary:
            open_owner, request, _, key_id = self.owner_fixture(Path(temporary))
            value = exact_json(request, MAX_PACKET)
            raw = bytearray(base64.b64decode(value["evidence"]["assertion_base64"]))
            raw[-1] ^= 1
            value["evidence"]["assertion_base64"] = base64.b64encode(raw).decode()
            changed = encode(value)
            owner = open_owner()
            with self.assertRaises(AttestationRejected):
                owner.perform(changed, "verify")
            self.assertIsNone(owner.perform(changed, "recover"))
            with self.assertRaises(AttestationRejected):
                owner.perform(changed, "verify")
            with sqlite3.connect(Path(temporary) / "wallet-e1.sqlite3") as connection:
                self.assertEqual(connection.execute("SELECT counter FROM apple_keys WHERE key_id=?", (key_id,)).fetchone(), (0,))
                self.assertEqual(connection.execute("SELECT count(*) FROM apple_client_data").fetchone(), (0,))
            owner.close()

    def test_private_packet_bounds_duplicate_unknown_and_exchange_replay(self):
        for packet in (b"x", b"\1\0\0\0", (MAX_PACKET + 1).to_bytes(4, "little")):
            with self.assertRaises(AttestationRejected):
                read_packet(io.BytesIO(packet))
        for original in (b'{"version":1,"version":1}', b'{} trailing', b'{"v":NaN}'):
            with self.assertRaises(AttestationRejected):
                exact_json(original, MAX_PACKET)
        class ProtocolOnlyOwner:
            def perform(self, original, action):
                return None
        packet = encode({"schema": SCHEMA, "version": 1, "exchange_id": "31" * 32,
                         "action": "recover", "original_base64": base64.b64encode(b"{}").decode()})
        frame = len(packet).to_bytes(4, "little") + packet
        with self.assertRaises(AttestationRejected):
            serve(ProtocolOnlyOwner(), io.BytesIO(frame + frame), io.BytesIO())
        value = exact_json(packet, MAX_PACKET)
        value["issuer_key"] = "forbidden"
        changed = encode(value)
        with self.assertRaises(AttestationRejected):
            serve(ProtocolOnlyOwner(), io.BytesIO(len(changed).to_bytes(4, "little") + changed), io.BytesIO())

    def test_fragmented_private_header_and_body_preserve_exact_packet(self):
        class Fragmented(io.BytesIO):
            def read(self, width=-1): return super().read(min(width, 1))
        self.assertEqual(read_packet(Fragmented((3).to_bytes(4, "little") + b"abc")), b"abc")

    def test_private_error_replies_preserve_storage_retry_and_never_expose_secrets(self):
        for error, expected in ((sqlite3.OperationalError("private bearer"), "unavailable"),
                                (FileNotFoundError("private bearer"), "unavailable"),
                                (sqlite3.IntegrityError("private bearer"), "rejected"),
                                (AttestationRejected("private bearer"), "rejected"),
                                (VerificationUnavailable("private bearer"), "unavailable")):
            class FailedOwner:
                def perform(self, original, action): raise error
            packet = encode({"schema": SCHEMA, "version": 1, "exchange_id": "31" * 32,
                             "action": "recover", "original_base64": "e30="})
            output = io.BytesIO()
            serve(FailedOwner(), io.BytesIO(len(packet).to_bytes(4, "little") + packet), output)
            response = exact_json(read_packet(io.BytesIO(output.getvalue())), MAX_PACKET)
            self.assertEqual(response["outcome"], expected)
            self.assertIsNone(response["evidence_base64"])
            self.assertNotIn(b"private bearer", output.getvalue())

    def test_lost_journal_cannot_restart_or_reverify_consumed_apple_evidence(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            open_owner, request, _, _ = self.owner_fixture(directory)
            owner = open_owner()
            self.addCleanup(owner.close)
            original = owner.perform(request, "verify")
            self.assertIsNotNone(original)
            (directory / "wallet-e1.sqlite3").unlink()
            packet = encode({"schema": SCHEMA, "version": 1, "exchange_id": "31" * 32,
                             "action": "recover", "original_base64": base64.b64encode(request).decode()})
            output = io.BytesIO()
            with patch("iroha_app_attestation.wallet_enrollment_worker.verify_apple_wallet_attestation_raw",
                       side_effect=AssertionError("lost custody must never reverify")):
                serve(owner, io.BytesIO(len(packet).to_bytes(4, "little") + packet), output)
                response = exact_json(read_packet(io.BytesIO(output.getvalue())), MAX_PACKET)
                self.assertEqual(response["outcome"], "unavailable")
                self.assertIsNone(response["evidence_base64"])
                owner.close()
                with self.assertRaises(FileNotFoundError):
                    open_owner()
            self.assertFalse((directory / "wallet-e1.sqlite3").exists())

    def test_request_cannot_select_policy_or_future_time(self):
        with tempfile.TemporaryDirectory() as temporary:
            open_owner, request, _, _ = self.owner_fixture(Path(temporary))
            owner = open_owner()
            for mutate in (lambda v: v.update({"roots": []}),
                           lambda v: v.update({"trusted_time_ms": v["expires_at_ms"]}),
                           lambda v: v.update({"platform": "android"})):
                value = exact_json(request, MAX_PACKET)
                mutate(value)
                with self.assertRaises(AttestationRejected):
                    owner.perform(encode(value), "verify")
            owner.close()

    def test_recovery_rechecks_original_custody_after_reading_committed_result(self):
        with tempfile.TemporaryDirectory() as temporary:
            open_owner, request, _, _ = self.owner_fixture(Path(temporary))
            owner = open_owner()
            owner.perform(request, "verify")
            with patch.object(owner, "recheck", side_effect=[None, AttestationRejected("changed original")]):
                with self.assertRaises(AttestationRejected):
                    owner.perform(request, "recover")
            owner.close()


    def test_private_configuration_projection_cannot_select_a_foreign_policy(self):
        with tempfile.TemporaryDirectory() as temporary:
            open_owner, _, _, _ = self.owner_fixture(Path(temporary), initialize=False)
            source = exact_json(open_owner.config, MAX_PACKET)
            for change in (lambda v: v.update({"signer": "forbidden"}),
                           lambda v: v.update({"version": True}),
                           lambda v: v.update({"app_policy_hex": "01" * 32}),
                           lambda v: v["policy"].update({"scheme_id_hex": "00" * 32}),
                           lambda v: v["policy"].update({"asset_digest_hex": "09" * 32}),
                           lambda v: v["policy"].update({"app_id": "FOREIGN.app"}),
                           lambda v: v["policy"].update({"root_base64": "eA=="}),
                           lambda v: v["policy"].update({"attestation_lease_lifetime_ms": True}),
                           lambda v: v["policy"].update({"challenge_lifetime_ms": 600001}),
                           lambda v: v["policy"]["regulatory_policy"].update({"permitted_controls": 8})):
                value = json.loads(json.dumps(source)); change(value)
                with self.subTest(value=value["policy"].get("app_id")), self.assertRaises(AttestationRejected):
                    open_owner(encode(value))
                self.assertFalse((Path(temporary) / "wallet-e1.sqlite3").exists())

    def test_request_requires_exact_selected_lifetime_scheme_asset_and_raw_integer_time(self):
        with tempfile.TemporaryDirectory() as temporary:
            open_owner, request, _, _ = self.owner_fixture(Path(temporary))
            owner = open_owner(); self.addCleanup(owner.close)
            for change in (lambda v: v.update({"expires_at_ms": v["expires_at_ms"] + 1}),
                           lambda v: v.update({"issued_at_ms": True}),
                           lambda v: v.update({"trusted_time_ms": True})):
                value = exact_json(request, MAX_PACKET); change(value)
                with self.assertRaises(AttestationRejected): owner.perform(encode(value), "verify")
            for index in (2, 34):
                value = exact_json(request, MAX_PACKET)
                transcript = bytearray(base64.b64decode(value["challenge_transcript_base64"]))
                transcript[index] ^= 1
                value["challenge_transcript_base64"] = base64.b64encode(transcript).decode()
                with self.assertRaises(AttestationRejected): owner.perform(encode(value), "verify")
            with sqlite3.connect(Path(temporary) / "wallet-e1.sqlite3") as connection:
                self.assertEqual(connection.execute("SELECT count(*) FROM wallet_e1_attempts").fetchone(), (0,))

    def test_recovery_requires_same_complete_configuration_original(self):
        with tempfile.TemporaryDirectory() as temporary:
            open_owner, request, _, _ = self.owner_fixture(Path(temporary))
            owner = open_owner(); original = owner.perform(request, "verify"); owner.close()
            # Same semantic policy, different original: signed inventory and result binding differ.
            changed = open_owner.config + b" "
            restored = open_owner(changed); self.addCleanup(restored.close)
            with self.assertRaises(AttestationRejected): restored.perform(request, "recover")
            unchanged = open_owner(); self.addCleanup(unchanged.close)
            self.assertEqual(unchanged.perform(request, "recover"), original)

    def test_recovery_rejects_replaced_database_original(self):
        with tempfile.TemporaryDirectory() as temporary:
            open_owner, request, _, _ = self.owner_fixture(Path(temporary))
            owner = open_owner(); self.addCleanup(owner.close)
            owner.perform(request, "verify")
            path = Path(temporary) / "wallet-e1.sqlite3"
            replacement = Path(temporary) / "replacement.sqlite3"
            shutil.copyfile(path, replacement); replacement.chmod(0o600); replacement.replace(path)
            with self.assertRaises(VerificationUnavailable): owner.perform(request, "recover")

    def test_recovery_rejects_hardlinked_database_and_shared_permissions(self):
        with tempfile.TemporaryDirectory() as temporary:
            open_owner, request, _, _ = self.owner_fixture(Path(temporary))
            owner = open_owner(); self.addCleanup(owner.close); owner.perform(request, "verify")
            path = Path(temporary) / "wallet-e1.sqlite3"
            link = Path(temporary) / "other.sqlite3"; os.link(path, link)
            with self.assertRaises(VerificationUnavailable): owner.perform(request, "recover")
            link.unlink(); path.chmod(0o640)
            with self.assertRaises(VerificationUnavailable): owner.perform(request, "recover")

    def test_held_configuration_original_is_rechecked_before_recovery(self):
        with tempfile.TemporaryDirectory() as temporary:
            open_owner, request, _, _ = self.owner_fixture(Path(temporary))
            path = Path(temporary) / "config.json"; path.write_bytes(open_owner.config); path.chmod(0o600)
            fd = os.open(path, os.O_RDONLY); self.addCleanup(os.close, fd)
            owner = open_owner(configuration_fd=fd); self.addCleanup(owner.close)
            owner.perform(request, "verify")
            path.write_bytes(open_owner.config + b" ")
            with self.assertRaises(VerificationUnavailable): owner.perform(request, "recover")

    def test_bound_exchanges_continue_past_4096_and_recent_tags_still_reject(self):
        class ReadOnlyProtocolOwner:
            def perform(self, original, action): return None
        def frame(n):
            packet = encode({"schema": SCHEMA, "version": 1, "exchange_id": n.to_bytes(32, "big").hex(),
                             "action": "recover", "original_base64": "e30="})
            return len(packet).to_bytes(4, "little") + packet
        output = io.BytesIO()
        serve(ReadOnlyProtocolOwner(), io.BytesIO(b"".join(frame(n) for n in range(1, EXCHANGE_WINDOW + 4))), output)
        count = 0; replies = io.BytesIO(output.getvalue())
        while read_packet(replies) is not None: count += 1
        self.assertEqual(count, EXCHANGE_WINDOW + 3)
        with self.assertRaises(AttestationRejected):
            serve(ReadOnlyProtocolOwner(), io.BytesIO(frame(1) + frame(2) + frame(1)), io.BytesIO())

    def test_attempt_original_cannot_be_changed_or_reverified_after_unknown_result(self):
        with tempfile.TemporaryDirectory() as temporary:
            open_owner, request, _, _ = self.owner_fixture(Path(temporary))
            owner = open_owner(); self.addCleanup(owner.close)
            with patch("iroha_app_attestation.wallet_enrollment_worker.verify_apple_wallet_attestation_raw",
                       side_effect=VerificationUnavailable("synthetic outage")):
                with self.assertRaises(VerificationUnavailable): owner.perform(request, "verify")
            with patch("iroha_app_attestation.wallet_enrollment_worker.verify_apple_wallet_attestation_raw",
                       side_effect=AssertionError("ambiguous attempt must only recover")):
                self.assertIsNone(owner.perform(request, "recover"))
                with self.assertRaises(AttestationRejected): owner.perform(request, "verify")
            value = exact_json(request, MAX_PACKET); value["trusted_time_ms"] += 1
            with self.assertRaises(sqlite3.IntegrityError): owner.perform(encode(value), "verify")
            with self.assertRaises(AttestationRejected): owner.perform(encode(value), "recover")


class AndroidIntegratedWorkerTests(unittest.TestCase):
    """Real signed synthetic KeyMint and RSA OAuth, fixed scripted HTTPS originals.

    The fixture replaces only owner admission for ephemeral private credentials and
    development loaded TLS files. Production has no bypass; no genuine Google/device claim.
    """
    @classmethod
    def setUpClass(cls):
        cls.openssl = Path(shutil.which("openssl")).resolve()
        result = subprocess.run([str(cls.openssl), "genpkey", "-algorithm", "RSA",
                                 "-pkeyopt", "rsa_keygen_bits:2048"], capture_output=True, check=True)
        cls.pem = result.stdout.decode("ascii")

    def fixture(self, directory, level=2):
        directory = directory.resolve(strict=True); directory.chmod(0o700)
        fixture = SignedEnvelope(directory, self.openssl)
        policy = fixture_policy(synthetic_root(fixture), False)
        selected = policy_scope(policy, fixture.point)
        now = int(time.time() * 1000) + 60000
        old = Selection(*[bytes([i]) * 32 for i in range(11, 17)])
        description = keymint_description(old, "org.example.wallet", 7, b"\x71" * 32,
            security_level=level, keymint_security_level=level, os_patch_level=202608,
            vendor_patch_level=20260805)
        description = description.replace(hashlib.sha256(old.transcript()).digest(), selected.challenge_digest())
        leaf, root = fixture.sign(ANDROID_KEY_DESCRIPTION_OID, description)
        google_original = encode({"schema": POLICY_SCHEMA, "version": 1,
            "cloudProject": {"id": PROJECT, "number": 642560099159}, "packageName": "org.example.wallet",
            "packageVersion": 7, "appSigningCertificateSha256Hex": (b"\x71" * 32).hex(),
            "credentialSubject": {"email": EMAIL, "clientId": CLIENT_ID}})
        config = encode({"schema": CONFIG_SCHEMA, "version": 1, "platform": "android",
            "app_policy_hex": policy.app.policy_digest().hex(), "enrollment_policy_hex": policy.enrollment.policy_digest().hex(),
            "openssl_path": str(self.openssl), "openssl_sha256": hashlib.sha256(self.openssl.read_bytes()).hexdigest(),
            "store_directory": str(directory), "policy": {"scheme_id_hex": policy.enrollment.scheme_id.hex(),
                "asset_digest_hex": policy.enrollment.asset_digest.hex(), "regulatory_policy": {
                    "permitted_controls": 0, "blacklist_max_age_ms": 0, "time_anchor_max_response_ms": 0},
                "challenge_lifetime_ms": 120000, "attestation_lease_lifetime_ms": 0,
                "package_name": "org.example.wallet", "package_version": 7,
                "app_certificate_sha256": (b"\x71" * 32).hex(), "root_base64": base64.b64encode(root).decode(),
                "root_sha256": hashlib.sha256(root).hexdigest(), "security_levels": [1, 2],
                "patch_floor_yyyymm": 202608, "google_policy_base64": base64.b64encode(google_original).decode(),
                "google_policy_sha256": hashlib.sha256(google_original).hexdigest(), "maximum_evidence_age_ms": 120000,
                "require_play_recognized": True, "require_licensed": True, "minimum_device_integrity": "MEETS_DEVICE_INTEGRITY"}})
        request = encode({"challenge_transcript_base64": base64.b64encode(selected.challenge_transcript).decode(),
            "payment_key_base64": base64.b64encode(fixture.point).decode(), "issued_at_ms": now - 1,
            "expires_at_ms": now - 1 + 120000, "trusted_time_ms": now, "platform": "android", "evidence": {
                "chain_base64": [base64.b64encode(x).decode() for x in (leaf, root)],
                "play_integrity_token": "opaque.synthetic.token"}})
        credential = {"type": "service_account", "project_id": PROJECT, "private_key_id": "ab" * 20,
            "private_key": self.pem, "client_email": EMAIL, "client_id": CLIENT_ID,
            "auth_uri": "https://accounts.google.com/o/oauth2/auth", "token_uri": TOKEN_URI,
            "auth_provider_x509_cert_url": "https://www.googleapis.com/oauth2/v1/certs",
            "client_x509_cert_url": "https://www.googleapis.com/robot/v1/metadata/x509/" + urllib.parse.quote(EMAIL, safe=""),
            "universe_domain": "googleapis.com"}
        path = directory / "synthetic-oauth.json"; path.write_bytes(encode(credential)); path.chmod(0o600)
        return config, request, selected, now, path

    def actual_owner(self, config, directory, credential, stack, *, initialize=True):
        descriptors = [os.open(x, os.O_RDONLY) for x in (directory, self.openssl, credential)]
        for fd in descriptors: stack.callback(os.close, fd)
        if initialize:
            E1CounterStore.initialize(directory, descriptors[0]).close()
        stack.enter_context(patch.object(crypto, "_HeldRootCodeOriginal", SyntheticCodeOriginal))
        def synthetic_credential_admission(**kwargs):
            self.assertEqual(kwargs["credential_owner_uid"], 0)
            kwargs["credential_owner_uid"] = os.getuid()  # Ephemeral fixture only.
            return GoogleServiceAccountTokenProvider(**kwargs)
        stack.enter_context(patch("iroha_app_attestation.wallet_enrollment_worker.GoogleServiceAccountTokenProvider",
                                  side_effect=synthetic_credential_admission))
        owner = VerifierOwner(config, directory_fd=descriptors[0], crypto_fd=descriptors[1], oauth_fd=descriptors[2])
        stack.callback(owner.close)
        return owner

    def transport(self, selected, now, *, wrong_hash=False, revoked=False):
        calls, originals = [], {}
        class Response:
            status = 200
            headers = {"Content-Type": "application/json", "Content-Encoding": "identity"}
            def __init__(self, url, body): self.url, self.body = url, body
            def geturl(self): return self.url
            def read(self, bound): return self.body[:bound]
            def __enter__(self): return self
            def __exit__(self, *args): pass
        class Transport:
            def open(inner, request, timeout):
                calls.append(request.full_url)
                if request.full_url == TOKEN_URI:
                    form = urllib.parse.parse_qs(request.data.decode(), strict_parsing=True)
                    self.assertEqual(set(form), {"assertion", "grant_type"})
                    head, body, signature = form["assertion"][0].split(".")
                    decode = lambda value: base64.urlsafe_b64decode(value + "=" * (-len(value) % 4))
                    self.assertEqual(json.loads(decode(body))["scope"], OAUTH_SCOPE)
                    with tempfile.TemporaryDirectory() as check:
                        d = Path(check); (d / "private.pem").write_text(self.pem)
                        subprocess.run([str(self.openssl), "pkey", "-in", str(d / "private.pem"), "-pubout",
                            "-out", str(d / "public.pem")], capture_output=True, check=True)
                        (d / "message").write_bytes((head + "." + body).encode()); (d / "signature").write_bytes(decode(signature))
                        subprocess.run([str(self.openssl), "dgst", "-sha256", "-verify", str(d / "public.pem"),
                            "-signature", str(d / "signature"), str(d / "message")], capture_output=True, check=True)
                    original = encode({"access_token": "synthetic-access-token", "token_type": "Bearer", "expires_in": 3600})
                elif request.full_url == revocation.GOOGLE_STATUS_URL:
                    # Tests of nonempty revocation entries are covered by the real shared suite.
                    original = encode({"entries": {}})
                else:
                    self.assertEqual(request.full_url, "https://playintegrity.googleapis.com/v1/org.example.wallet:decodeIntegrityToken")
                    self.assertEqual(json.loads(request.data), {"integrity_token": "opaque.synthetic.token"})
                    expected = b"\x99" * 32 if wrong_hash else selected.enrollment_key_binding()
                    original = encode({"tokenPayloadExternal": {"requestDetails": {"requestPackageName": "org.example.wallet",
                        "requestHash": pi.request_hash_text(expected), "timestampMillis": str(now)}, "appIntegrity": {
                            "appRecognitionVerdict": "PLAY_RECOGNIZED", "packageName": "org.example.wallet", "versionCode": "7",
                            "certificateSha256Digest": [pi.request_hash_text(b"\x71" * 32)]}, "deviceIntegrity": {
                                "deviceRecognitionVerdict": ["MEETS_DEVICE_INTEGRITY"]}, "accountDetails": {"appLicensingVerdict": "LICENSED"}}})
                    originals["google"] = original
                return Response(request.full_url, original)
        return Transport(), calls, originals

    def check_level(self, level):
        with tempfile.TemporaryDirectory() as temporary, ExitStack() as stack:
            directory = Path(temporary).resolve()
            config, request, selected, now, credential = self.fixture(directory, level)
            owner = self.actual_owner(config, directory, credential, stack)
            transport, calls, originals = self.transport(selected, now)
            stack.enter_context(patch("urllib.request.build_opener", return_value=transport))
            result = owner.perform(request, "verify")
            projection = exact_json(result, MAX_ORIGINAL)
            self.assertEqual(projection["kind_tag"], level)
            self.assertEqual(projection["payment_key_base64"], base64.b64encode(selected.payment_key_sec1).decode())
            self.assertEqual(projection["key_binding"], selected.enrollment_key_binding().hex())
            self.assertEqual(base64.b64decode(projection["original_items_base64"][-1]), originals["google"])
            self.assertEqual(calls, [revocation.GOOGLE_STATUS_URL, TOKEN_URI,
                                    "https://playintegrity.googleapis.com/v1/org.example.wallet:decodeIntegrityToken"])
            self.assertEqual(owner.perform(request, "recover"), result)
            self.assertEqual(len(calls), 3)

    def test_current_tee_enrollment_real_keymint_oauth_and_google_original(self): self.check_level(1)
    def test_current_strongbox_enrollment_real_keymint_oauth_and_google_original(self): self.check_level(2)

    def test_wrong_google_hash_retains_unknown_and_never_reverifies(self):
        with tempfile.TemporaryDirectory() as temporary, ExitStack() as stack:
            directory = Path(temporary).resolve()
            config, request, selected, now, credential = self.fixture(directory)
            owner = self.actual_owner(config, directory, credential, stack)
            transport, calls, _ = self.transport(selected, now, wrong_hash=True)
            stack.enter_context(patch("urllib.request.build_opener", return_value=transport))
            with self.assertRaises(AttestationRejected): owner.perform(request, "verify")
            self.assertIsNone(owner.perform(request, "recover")); self.assertEqual(len(calls), 3)
            with self.assertRaises(AttestationRejected): owner.perform(request, "verify")

    def test_android_software_boolean_or_foreign_google_original_cannot_enter_worker(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary).resolve()
            config, _, _, _, _ = self.fixture(directory)
            source = exact_json(config, MAX_PACKET)
            for change in (lambda v: v["policy"].update({"security_levels": [0]}),
                           lambda v: v["policy"].update({"security_levels": [True]}),
                           lambda v: v["policy"].update({"require_licensed": 1}),
                           lambda v: v["policy"].update({"package_version": True}),
                           lambda v: v["policy"].update({"root_sha256": "09" * 32})):
                value = json.loads(json.dumps(source)); change(value)
                with self.assertRaises(AttestationRejected): configured_policy(value["policy"], "android")
            with ExitStack() as stack:
                changed = json.loads(json.dumps(source)); changed["policy"]["google_policy_base64"] = "e30="
                with self.assertRaises(AttestationRejected): self.actual_owner(encode(changed), directory, directory / "synthetic-oauth.json", stack, initialize=False)
                self.assertFalse((directory / "wallet-e1.sqlite3").exists())


class UnsignedArchiveTests(unittest.TestCase):
    def builder(self):
        path = Path(__file__).resolve().parents[1] / "tools" / "build_wallet_e1_verifier_zipapp.py"
        spec = importlib.util.spec_from_file_location("wallet_e1_unsigned_archive_builder", path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_current_archive_is_deterministic_explicit_and_never_admits_runtime(self):
        builder = self.builder()
        package = Path(__file__).resolve().parents[1]
        with tempfile.TemporaryDirectory() as temporary:
            one, two = Path(temporary).resolve() / "one.pyz", Path(temporary).resolve() / "two.pyz"
            first, second = builder.build(package, one), builder.build(package, two)
            self.assertEqual(one.read_bytes(), two.read_bytes())
            self.assertEqual(first, second)
            self.assertIs(first["signed_runtime_admission"], False)
            with zipfile.ZipFile(one) as archive:
                self.assertEqual(set(archive.namelist()), {"__main__.py"} |
                                 {"iroha_app_attestation/" + name for name in builder.SOURCES})
                self.assertNotIn("iroha_app_attestation/ordinary_worker.py", archive.namelist())
                self.assertNotIn("iroha_app_attestation/kagemusha_native_signer.py", archive.namelist())
            with self.assertRaises(FileExistsError):
                builder.build(package, one)

    def test_archive_builder_refuses_source_alias_without_creating_output(self):
        builder = self.builder()
        with tempfile.TemporaryDirectory() as temporary:
            package = Path(temporary).resolve() / "package"
            source = package / "src" / "iroha_app_attestation"
            source.mkdir(parents=True)
            for name in builder.SOURCES:
                (source / name).write_bytes(b"# bounded original\n")
            (source / "wallet_enrollment_worker.py").unlink()
            (source / "wallet_enrollment_worker.py").symlink_to(source / "__init__.py")
            output = Path(temporary) / "refused.pyz"
            with self.assertRaises(ValueError):
                builder.build(package, output)
            self.assertFalse(output.exists())

    def test_source_change_during_archive_write_is_refused_with_all_inputs_held(self):
        builder = self.builder()
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary).resolve()
            package = directory / "package"
            source = package / "src" / "iroha_app_attestation"; source.mkdir(parents=True)
            for name in builder.SOURCES: (source / name).write_bytes(b"# inert custody fixture\n")
            original = zipfile.ZipFile.writestr
            changed = False
            def mutate(archive, *args, **arguments):
                nonlocal changed
                if not changed:
                    (source / "wallet_policy.py").write_bytes(b"# changed during archive\n"); changed = True
                return original(archive, *args, **arguments)
            with patch.object(zipfile.ZipFile, "writestr", mutate), self.assertRaisesRegex(ValueError, "source changed"):
                builder.build(package, directory / "refused.pyz")

    def test_isolated_archive_imports_actual_current_policy_and_worker_without_source_tree(self):
        builder = self.builder()
        package = Path(__file__).resolve().parents[1]
        with tempfile.TemporaryDirectory() as temporary:
            archive = Path(temporary).resolve() / "current.pyz"
            builder.build(package, archive)
            code = ("import sys; sys.path.insert(0, sys.argv[1]); "
                    "from iroha_app_attestation.wallet_enrollment_worker import CONFIG_SCHEMA, configured_policy; "
                    "from iroha_app_attestation.wallet_policy import ConfiguredWalletEnrollmentPolicyV1; "
                    "assert CONFIG_SCHEMA == 'iroha.kagemusha.wallet-e1-verifier-config.v1'; "
                    "assert callable(configured_policy)")
            result = subprocess.run([sys.executable, "-I", "-B", "-c", code, str(archive)],
                                    cwd=temporary, env={}, capture_output=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stderr.decode())


if __name__ == "__main__":
    unittest.main()
