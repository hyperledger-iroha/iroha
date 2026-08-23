"""Tests for the durable production App Attest freshness authority."""

from __future__ import annotations

import base64
from dataclasses import replace
from datetime import datetime, timedelta, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
from types import SimpleNamespace
import subprocess
import sys
import tempfile
import threading
import unittest
from unittest import mock


SCRIPT_DIR = Path(__file__).resolve().parents[1]
TEST_DIR = Path(__file__).resolve().parent
REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
OFFICIAL_APPLE_2026_ATTESTATION_OBJECT = (
    REPOSITORY_ROOT
    / "fixtures/sdk/apple_app_attest_official_2026_attestation_object.base64"
)
# Extracted from Apple's current primary Attestation Object Validation Guide:
# https://developer.apple.com/tutorials/data/documentation/devicecheck/attestation-object-validation-guide.md
OFFICIAL_APPLE_2026_ATTESTATION_OBJECT_SHA256 = (
    "e4ca508153f6619a29d0887eb0ffe19540b9f15c5a6aeccdac26c49affd5f61a"
)
OFFICIAL_APPLE_2026_RECEIPT_SHA256 = (
    "26a7ef09ab4cff17140e02780c57bec69672f4933d1a175ee84229b0107b6de0"
)
for directory in (SCRIPT_DIR, TEST_DIR):
    if str(directory) not in sys.path:
        sys.path.insert(0, str(directory))

import check_kagemusha_candidate_ios_evidence_test as fixture_support  # noqa: E402
import kagemusha_app_attest_freshness_authority as authority  # noqa: E402
import kagemusha_candidate_ios_evidence as candidate_evidence  # noqa: E402
import kagemusha_production_ios_evidence as production_evidence  # noqa: E402


def der_length(length: int) -> bytes:
    if length < 0x80:
        return bytes([length])
    encoded = length.to_bytes((length.bit_length() + 7) // 8, "big")
    return bytes([0x80 | len(encoded)]) + encoded


def der(tag: int, payload: bytes) -> bytes:
    return bytes([tag]) + der_length(len(payload)) + payload


def der_integer(value: int) -> bytes:
    encoded = value.to_bytes(max(1, (value.bit_length() + 7) // 8), "big")
    if encoded[0] & 0x80:
        encoded = b"\0" + encoded
    return der(0x02, encoded)


def der_oid(value: str) -> bytes:
    arcs = [int(component) for component in value.split(".")]
    encoded = bytearray()
    for number in [40 * arcs[0] + arcs[1], *arcs[2:]]:
        component = bytearray([number & 0x7F])
        number >>= 7
        while number:
            component.append(0x80 | (number & 0x7F))
            number >>= 7
        encoded.extend(reversed(component))
    return der(0x06, bytes(encoded))


def p256_spki(public_key: bytes) -> bytes:
    algorithm = der(
        0x30,
        der_oid(production_evidence.OID_EC_PUBLIC_KEY)
        + der_oid(production_evidence.OID_PRIME256V1),
    )
    return der(0x30, algorithm + der(0x03, b"\0" + public_key))


def receipt_attribute(field: int, value: bytes) -> bytes:
    return der(0x30, der_integer(field) + der_integer(1) + der(0x04, value))


def receipt_attribute_with_version(field: int, version: int, value: bytes) -> bytes:
    return der(
        0x30,
        der_integer(field) + der_integer(version) + der(0x04, value),
    )


def receipt_string(value: str) -> bytes:
    return value.encode("ascii")


def receipt_timestamp(value: datetime) -> str:
    return value.astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.000Z")


def jwt(team_id: str, issued_at_seconds: int) -> bytes:
    def segment(value: dict[str, object]) -> bytes:
        payload = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
        return base64.urlsafe_b64encode(payload).rstrip(b"=")

    return b".".join(
        (
            segment({"alg": "ES256", "kid": "DEVICEKEY1"}),
            segment({"iss": team_id, "iat": issued_at_seconds}),
            base64.urlsafe_b64encode(b"\x01" * 64).rstrip(b"="),
        )
    )


class AppAttestFreshnessAuthorityTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.openssl = Path(shutil.which("openssl") or "")
        if not cls.openssl.is_file():
            raise unittest.SkipTest("OpenSSL is unavailable")
        cls.keys = tempfile.TemporaryDirectory()
        cls.key_root = Path(cls.keys.name)
        cls._make_ed25519_pair("lab")
        cls._make_ed25519_pair("authority")
        cls._make_cms_authority("apple-test")
        cls._make_cms_authority("non-apple")

    @classmethod
    def tearDownClass(cls) -> None:
        cls.keys.cleanup()

    @classmethod
    def _run(cls, arguments: list[str]) -> None:
        subprocess.run(
            [str(cls.openssl), *arguments],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    @classmethod
    def _make_ed25519_pair(cls, name: str) -> None:
        private = cls.key_root / f"{name}.key.pem"
        public = cls.key_root / f"{name}.pub.pem"
        cls._run(["genpkey", "-algorithm", "ED25519", "-out", str(private)])
        cls._run(
            ["pkey", "-in", str(private), "-pubout", "-out", str(public)]
        )
        private.chmod(0o600)
        public.chmod(0o600)

    @classmethod
    def _make_cms_authority(cls, name: str) -> None:
        root_key = cls.key_root / f"{name}-root.key"
        root_cert = cls.key_root / f"{name}-root.pem"
        signer_key = cls.key_root / f"{name}-signer.key"
        signer_request = cls.key_root / f"{name}-signer.csr"
        signer_cert = cls.key_root / f"{name}-signer.pem"
        extensions = cls.key_root / f"{name}-signer.ext"
        cls._run(
            [
                "ecparam",
                "-name",
                "prime256v1",
                "-genkey",
                "-noout",
                "-out",
                str(root_key),
            ]
        )
        cls._run(
            [
                "req",
                "-new",
                "-x509",
                "-key",
                str(root_key),
                "-out",
                str(root_cert),
                "-days",
                "3650",
                "-subj",
                f"/CN={name} Root",
                "-addext",
                "basicConstraints=critical,CA:true",
                "-addext",
                "keyUsage=critical,keyCertSign,cRLSign",
            ]
        )
        cls._run(
            [
                "ecparam",
                "-name",
                "prime256v1",
                "-genkey",
                "-noout",
                "-out",
                str(signer_key),
            ]
        )
        cls._run(
            [
                "req",
                "-new",
                "-key",
                str(signer_key),
                "-out",
                str(signer_request),
                "-subj",
                f"/CN={name} Signer",
            ]
        )
        extensions.write_text(
            "basicConstraints=critical,CA:false\n"
            "keyUsage=critical,digitalSignature\n",
            encoding="ascii",
        )
        cls._run(
            [
                "x509",
                "-req",
                "-in",
                str(signer_request),
                "-CA",
                str(root_cert),
                "-CAkey",
                str(root_key),
                "-CAcreateserial",
                "-out",
                str(signer_cert),
                "-days",
                "3650",
                "-sha256",
                "-extfile",
                str(extensions),
            ]
        )
        for path in (root_key, root_cert, signer_key, signer_request, signer_cert):
            path.chmod(0o600)

    @classmethod
    def _root_digest(cls, name: str) -> str:
        pem = (cls.key_root / f"{name}-root.pem").read_text(encoding="ascii")
        encoded = "".join(
            line for line in pem.splitlines() if not line.startswith("-----")
        )
        return hashlib.sha256(base64.b64decode(encoded)).hexdigest()

    @classmethod
    def _cms_receipt(
        cls,
        *,
        authority_name: str = "apple-test",
        app_id: str = "A1B2C3D4E5.org.example.app",
        public_key: bytes = b"",
        risk_metric: int = 2,
        receipt_type: str = "RECEIPT",
        creation: datetime | None = None,
    ) -> bytes:
        if not public_key:
            public_key = b"\x04" + production_evidence.P256_G[0].to_bytes(
                32, "big"
            ) + production_evidence.P256_G[1].to_bytes(32, "big")
        now = creation or datetime.now(tz=timezone.utc).replace(microsecond=0)
        attributes = [
            receipt_attribute(2, receipt_string(app_id)),
            receipt_attribute(3, p256_spki(public_key)),
            receipt_attribute(6, receipt_string(receipt_type)),
            receipt_attribute(12, receipt_string(receipt_timestamp(now))),
            receipt_attribute(17, receipt_string(str(risk_metric))),
            receipt_attribute(
                19, receipt_string(receipt_timestamp(now + timedelta(days=1)))
            ),
            receipt_attribute(
                21, receipt_string(receipt_timestamp(now + timedelta(days=30)))
            ),
        ]
        payload = der(0x31, b"".join(attributes))
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary) / "payload.der"
            output = Path(temporary) / "receipt.der"
            source.write_bytes(payload)
            cls._run(
                [
                    "cms",
                    "-sign",
                    "-binary",
                    "-nodetach",
                    "-in",
                    str(source),
                    "-signer",
                    str(cls.key_root / f"{authority_name}-signer.pem"),
                    "-inkey",
                    str(cls.key_root / f"{authority_name}-signer.key"),
                    "-outform",
                    "DER",
                    "-out",
                    str(output),
                ]
            )
            return output.read_bytes()

    @staticmethod
    def _fake_validated(
        *, counter: int = 1, evidence_label: str = "evidence"
    ) -> authority.ValidatedEvidence:
        attestation_nonce = hashlib.sha256(b"attestation-message").digest()
        assertion_nonce = hashlib.sha256(b"assertion-message").digest()
        request = {"request": evidence_label}
        request_digest = hashlib.sha256(
            candidate_evidence.canonical_json_bytes(request)
        ).hexdigest()
        public_key = b"\x04" + production_evidence.P256_G[0].to_bytes(
            32, "big"
        ) + production_evidence.P256_G[1].to_bytes(32, "big")
        facts = SimpleNamespace(
            key_id=base64.b64encode(hashlib.sha256(public_key).digest()).decode(),
            assertion_counter=counter,
            attestation_challenge_nonce=hashlib.sha256(
                (evidence_label + "-attestation-challenge").encode()
            ).digest(),
            assertion_challenge_nonce=hashlib.sha256(
                (evidence_label + "-assertion-challenge").encode()
            ).digest(),
            attestation_client_data=b"attestation-client",
            attestation_object=b"attestation-object",
            assertion_client_data=b"assertion-client",
            assertion_object=b"assertion-object",
            attestation_nonce=attestation_nonce,
            assertion_nonce=assertion_nonce,
            certificate_chain=(b"leaf", b"intermediate"),
        )
        return authority.ValidatedEvidence(
            evidence={"platform_evidence": {"value": evidence_label}},
            policy={"app_id_prefix": "A1B2C3D4E5"},
            facts=facts,
            evidence_sha256=hashlib.sha256(evidence_label.encode()).hexdigest(),
            policy_sha256=hashlib.sha256(b"policy").hexdigest(),
            release_manifest_sha256=hashlib.sha256(b"release").hexdigest(),
            lab_signer_key_id="lab-key",
            lab_signer_public_key_sha256=hashlib.sha256(b"lab-key").hexdigest(),
            capture_request=request,
            capture_request_sha256=request_digest,
            embedded_apple_receipt=b"embedded",
            app_id="A1B2C3D4E5.org.example.app",
            assertion_public_key=public_key,
        )

    @staticmethod
    def _insert_lease(
        state: authority.AuthorityState,
        validated: authority.ValidatedEvidence,
        *,
        challenge_label: str = "challenge",
        now_ms: int = 2_000_000_000_000,
    ) -> authority.ChallengeLease:
        lease = authority.ChallengeLease(
            challenge_id=hashlib.sha256(challenge_label.encode()).hexdigest(),
            consumption_id=hashlib.sha256(
                (challenge_label + "-consumption").encode()
            ).hexdigest(),
            issued_at_unix_ms=now_ms - 1000,
            expires_at_unix_ms=now_ms + 299_000,
            request_sha256=validated.capture_request_sha256,
            request=validated.capture_request,
        )
        state.insert_challenge(
            lease,
            validated.facts.attestation_challenge_nonce,
            validated.facts.assertion_challenge_nonce,
        )
        return lease

    def test_fresh_state_directory_initializes_and_reopens(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            state_dir = Path(temporary) / "state"
            with authority.AuthorityState(state_dir) as state:
                tables = {
                    row[0]
                    for row in state.connection.execute(
                        "SELECT name FROM sqlite_master WHERE type = 'table'"
                    )
                }
                self.assertIn("challenges", tables)
                self.assertEqual(state.path.stat().st_mode & 0o777, 0o600)
                columns = tuple(
                    row[1]
                    for row in state.connection.execute(
                        "PRAGMA table_info(challenges)"
                    )
                )
                self.assertEqual(columns, authority.CHALLENGE_TABLE_COLUMNS)
                catalog_columns = tuple(
                    row[1]
                    for row in state.connection.execute(
                        "PRAGMA table_info(catalog_revalidations)"
                    )
                )
                self.assertEqual(
                    catalog_columns, authority.CATALOG_REVALIDATION_TABLE_COLUMNS
                )
                self.assertEqual(
                    state.connection.execute("PRAGMA journal_mode").fetchone(),
                    ("wal",),
                )
                self.assertEqual(
                    state.connection.execute("PRAGMA synchronous").fetchone(),
                    (2,),
                )
                for suffix in ("-wal", "-shm"):
                    sidecar = Path(str(state.path) + suffix)
                    self.assertTrue(sidecar.is_file())
                    self.assertEqual(sidecar.stat().st_mode & 0o777, 0o600)
                database_path = state.path
            legacy = authority.sqlite3.connect(str(database_path))
            try:
                legacy.execute("DROP TABLE catalog_revalidations")
                legacy.execute(
                    "UPDATE authority_metadata SET schema_version = 1 WHERE singleton = 1"
                )
                legacy.commit()
            finally:
                legacy.close()
            with authority.AuthorityState(state_dir) as reopened:
                self.assertEqual(
                    reopened.connection.execute(
                        "SELECT schema_version FROM authority_metadata"
                    ).fetchone(),
                    (authority.STATE_SCHEMA_VERSION,),
                )
                self.assertEqual(
                    tuple(
                        row[1]
                        for row in reopened.connection.execute(
                            "PRAGMA table_info(catalog_revalidations)"
                        )
                    ),
                    authority.CATALOG_REVALIDATION_TABLE_COLUMNS,
                )

    def test_authority_output_is_private_durable_and_never_replaced(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            root.chmod(0o700)
            output = root / "authority-output.json"
            value = {"schema": "test", "sequence": 1}

            authority._write_new_private_json(output, value, "authority output")

            expected = candidate_evidence.canonical_json_bytes(value)
            self.assertEqual(output.read_bytes(), expected)
            self.assertEqual(output.stat().st_mode & 0o777, 0o600)
            with self.assertRaisesRegex(authority.AuthorityError, "already exists"):
                authority._write_new_private_json(
                    output,
                    {"schema": "replacement", "sequence": 2},
                    "authority output",
                )
            self.assertEqual(output.read_bytes(), expected)

    def test_authority_output_zero_length_write_fails_and_removes_partial_file(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            root.chmod(0o700)
            output = root / "authority-output.json"
            with (
                mock.patch.object(authority.os, "write", return_value=0),
                self.assertRaisesRegex(authority.AuthorityError, "written durably"),
            ):
                authority._write_new_private_json(
                    output,
                    {"schema": "test"},
                    "authority output",
                )
            self.assertFalse(output.exists())

    def test_cms_staging_zero_length_write_fails_closed(self) -> None:
        writer = mock.Mock(side_effect=[0, OSError("write loop continued")])
        with (
            mock.patch.object(authority.os, "write", writer),
            self.assertRaisesRegex(OSError, "short CMS receipt write"),
        ):
            authority._write_all(123, b"signed CMS receipt", "CMS receipt")
        self.assertEqual(writer.call_count, 1)

    def test_schema_v2_catalog_rows_migrate_to_active_terminal_state(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            state_dir = Path(temporary) / "state"
            state_dir.mkdir(mode=0o700)
            database_path = (
                state_dir / "app-attest-freshness-authority-v1.sqlite3"
            )
            legacy = authority.sqlite3.connect(str(database_path))
            try:
                legacy.executescript(
                    """
                    CREATE TABLE authority_metadata (
                        singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                        schema_version INTEGER NOT NULL
                    );
                    INSERT INTO authority_metadata VALUES (1, 2);
                    CREATE TABLE catalog_revalidations (
                        promotion_id TEXT PRIMARY KEY,
                        catalog_sha256 TEXT NOT NULL,
                        receipt_id TEXT NOT NULL UNIQUE,
                        issued_at_unix_ms INTEGER NOT NULL,
                        expires_at_unix_ms INTEGER NOT NULL,
                        authority_key_id TEXT NOT NULL,
                        authority_public_key_sha256 TEXT NOT NULL,
                        receipt_payload BLOB NOT NULL,
                        CHECK (expires_at_unix_ms > issued_at_unix_ms)
                    );
                    """
                )
                legacy.execute(
                    """
                    INSERT INTO catalog_revalidations
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                    """,
                    (
                        "1" * 64,
                        "2" * 64,
                        "3" * 64,
                        1,
                        2,
                        "authority-key",
                        "4" * 64,
                        b"{}",
                    ),
                )
                legacy.commit()
            finally:
                legacy.close()
            database_path.chmod(0o600)

            with authority.AuthorityState(state_dir) as migrated:
                self.assertEqual(
                    tuple(
                        row[1]
                        for row in migrated.connection.execute(
                            "PRAGMA table_info(catalog_revalidations)"
                        )
                    ),
                    authority.CATALOG_REVALIDATION_TABLE_COLUMNS,
                )
                self.assertEqual(
                    migrated.connection.execute(
                        """
                        SELECT state, retired_at_unix_ms
                          FROM catalog_revalidations WHERE promotion_id = ?
                        """,
                        ("1" * 64,),
                    ).fetchone(),
                    ("active", None),
                )
    def test_state_rejects_writable_ancestor_and_preplanted_wal_alias(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            unsafe = Path(temporary) / "unsafe"
            unsafe.mkdir(mode=0o700)
            unsafe.chmod(0o777)
            with self.assertRaisesRegex(authority.AuthorityError, "ancestor"):
                authority.AuthorityState(unsafe / "state")
            unsafe.chmod(0o700)

            target = Path(temporary) / "target"
            target.write_bytes(b"not sqlite")
            target.chmod(0o600)
            for suffix, label in (
                ("-wal", "WAL"),
                ("-shm", "shared memory"),
                ("-journal", "rollback journal"),
            ):
                with self.subTest(suffix=suffix):
                    state_dir = Path(temporary) / f"aliased-state-{label}"
                    state_dir.mkdir(mode=0o700)
                    sidecar = state_dir / (
                        "app-attest-freshness-authority-v1.sqlite3" + suffix
                    )
                    sidecar.symlink_to(target)
                    with self.assertRaisesRegex(
                        authority.AuthorityError,
                        f"{label} must be an owner-private",
                    ):
                        authority.AuthorityState(state_dir)

    def test_state_rejects_ancestor_substitution_after_open(self) -> None:
        validated = self._fake_validated()
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary) / "custody"
            parent.mkdir(mode=0o700)
            state_dir = parent / "state"
            with authority.AuthorityState(state_dir) as state:
                displaced = Path(temporary) / "displaced-custody"
                parent.rename(displaced)
                parent.mkdir(mode=0o700)
                state_dir.mkdir(mode=0o700)
                with self.assertRaisesRegex(
                    authority.AuthorityError,
                    "state directory changed",
                ):
                    self._insert_lease(state, validated)

    def test_jwt_descriptor_rejects_blocking_pipe(self) -> None:
        read_descriptor, write_descriptor = os.pipe()
        try:
            with self.assertRaisesRegex(authority.AuthorityError, "regular file"):
                authority.read_devicecheck_jwt(
                    file_path=None, descriptor=read_descriptor
                )
        finally:
            os.close(read_descriptor)
            os.close(write_descriptor)

    def test_jwt_metadata_is_strict_and_signature_is_canonical_es256(self) -> None:
        now_ms = authority._now_ms()
        token = jwt("A1B2C3D4E5", now_ms // 1000)
        self.assertEqual(
            authority.validate_devicecheck_jwt(
                token,
                expected_issuer="A1B2C3D4E5",
                evaluation_time_unix_ms=now_ms,
            ),
            token.decode("ascii"),
        )
        header, claims, signature = token.split(b".")
        duplicate_claims = base64.urlsafe_b64encode(
            (
                b'{"iat":'
                + str(now_ms // 1000).encode("ascii")
                + b',"iss":"A1B2C3D4E5","iss":"A1B2C3D4E5"}'
            )
        ).rstrip(b"=")
        with self.assertRaisesRegex(authority.AuthorityError, "claims is invalid"):
            authority.validate_devicecheck_jwt(
                b".".join((header, duplicate_claims, signature)),
                expected_issuer="A1B2C3D4E5",
                evaluation_time_unix_ms=now_ms,
            )
        with self.assertRaisesRegex(
            authority.AuthorityError, "signature must be canonical ES256"
        ):
            authority.validate_devicecheck_jwt(
                b".".join((header, claims, b"AA")),
                expected_issuer="A1B2C3D4E5",
                evaluation_time_unix_ms=now_ms,
            )

    def test_apple_transport_is_fixed_bounded_and_does_not_follow_redirects(
        self,
    ) -> None:
        class FakeSocket:
            def settimeout(self, _: float) -> None:
                pass

        class FakeResponse:
            def __init__(self, status: int, body: bytes) -> None:
                self.status = status
                self.body = body
                self.offset = 0

            def read(self, maximum: int) -> bytes:
                chunk = self.body[self.offset : self.offset + maximum]
                self.offset += len(chunk)
                return chunk

        class FakeConnection:
            instances: list["FakeConnection"] = []
            response_status = 200
            response_body = base64.b64encode(b"refreshed-receipt")

            def __init__(
                self,
                host: str,
                port: int,
                *,
                timeout: float,
                context: object,
            ) -> None:
                self.host = host
                self.port = port
                self.timeout = timeout
                self.context = context
                self.sock = FakeSocket()
                self.request_value: tuple[object, ...] | None = None
                self.__class__.instances.append(self)

            def request(self, *value: object, **named: object) -> None:
                self.request_value = (*value, named)

            def getresponse(self) -> FakeResponse:
                return FakeResponse(self.response_status, self.response_body)

            def close(self) -> None:
                pass

        with mock.patch.object(
            authority.http.client, "HTTPSConnection", FakeConnection
        ):
            refreshed = authority.request_apple_receipt(b"embedded", "jwt")
        self.assertEqual(refreshed, b"refreshed-receipt")
        connection = FakeConnection.instances[-1]
        self.assertEqual(
            (connection.host, connection.port),
            (authority.APPLE_PRODUCTION_HOST, 443),
        )
        assert connection.request_value is not None
        method, path, request_options = connection.request_value
        self.assertEqual((method, path), ("POST", authority.APPLE_PRODUCTION_PATH))
        self.assertEqual(request_options["body"], base64.b64encode(b"embedded"))
        self.assertEqual(request_options["headers"]["Authorization"], "jwt")

        FakeConnection.response_status = 302
        FakeConnection.response_body = b""
        with mock.patch.object(
            authority.http.client, "HTTPSConnection", FakeConnection
        ), self.assertRaisesRegex(authority.AuthorityError, "HTTP 302"):
            authority.request_apple_receipt(b"embedded", "jwt")

        FakeConnection.response_status = 200
        FakeConnection.response_body = b"A" * (authority.MAX_APPLE_RESPONSE_BYTES + 1)
        with mock.patch.object(
            authority.http.client, "HTTPSConnection", FakeConnection
        ), self.assertRaisesRegex(authority.AuthorityError, "exceeds its bound"):
            authority.request_apple_receipt(b"embedded", "jwt")

    def test_outstanding_challenge_limit_is_durable(self) -> None:
        validated = self._fake_validated()
        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(
            authority, "MAX_OUTSTANDING_CHALLENGES", 1
        ):
            with authority.AuthorityState(Path(temporary) / "state") as state:
                self._insert_lease(state, validated, challenge_label="first")
                with self.assertRaisesRegex(
                    authority.AuthorityError, "too many outstanding challenges"
                ):
                    self._insert_lease(state, validated, challenge_label="second")

    def test_official_apple_2026_cms_receipt_uses_raw_numeric_attributes(self) -> None:
        """Keep Apple's primary 2026 App Attest vector wire-compatible."""

        encoded_payload = OFFICIAL_APPLE_2026_ATTESTATION_OBJECT.read_text(
            encoding="ascii"
        )
        self.assertTrue(encoded_payload.endswith("\n"))
        encoded = "".join(encoded_payload.splitlines())
        attestation_object = base64.b64decode(encoded, validate=True)
        self.assertEqual(
            base64.b64encode(attestation_object).decode("ascii"), encoded
        )
        self.assertEqual(len(attestation_object), 5_906)
        self.assertEqual(
            hashlib.sha256(attestation_object).hexdigest(),
            OFFICIAL_APPLE_2026_ATTESTATION_OBJECT_SHA256,
        )
        decoded = production_evidence._cbor_object(
            production_evidence._decode_cbor(
                attestation_object, "official Apple attestation object"
            ),
            {"fmt", "attStmt", "authData"},
            "official Apple attestation object",
        )
        statement = production_evidence._cbor_object(
            decoded["attStmt"],
            {"x5c", "receipt"},
            "official Apple attestation statement",
        )
        receipt = statement["receipt"]
        self.assertIsInstance(receipt, bytes)
        self.assertEqual(
            hashlib.sha256(receipt).hexdigest(),
            OFFICIAL_APPLE_2026_RECEIPT_SHA256,
        )
        content = authority._verify_cms_signature(
            receipt,
            root_pem_path=authority.APPLE_RECEIPT_ROOT,
            openssl_path=Path("/usr/bin/openssl"),
            timeout_seconds=10.0,
            evaluation_time_unix_ms=1_776_795_192_153,
        )
        attributes = authority._receipt_attributes(content)
        self.assertEqual(list(attributes), [2, 3, 4, 5, 6, 7, 12, 21])
        self.assertEqual(
            authority._receipt_string(attributes[2], "official App ID"),
            "1234567890.com.example.myapp",
        )
        self.assertEqual(
            authority._receipt_string(attributes[6], "official receipt type"),
            "ATTEST",
        )
        self.assertEqual(
            authority._receipt_string(attributes[7], "official environment"),
            "production",
        )
        self.assertEqual(
            base64.b64encode(authority._attested_public_key(attributes[3])).decode(
                "ascii"
            ),
            "BEMyVErPMj23dEQ8qvM59W5+lcck+sLBQlnzZeJEVlCytfsoW89Um8tgWUQS52gqJCfuran7Ut/tCxqxftCfqb0=",
        )
        self.assertEqual(
            authority._receipt_time(attributes[12], "official creation time"),
            1_776_795_192_153,
        )
        with self.assertRaisesRegex(
            authority.AuthorityError, "not a refreshed RECEIPT"
        ):
            authority.verify_apple_receipt(
                receipt,
                expected_app_id="1234567890.com.example.myapp",
                expected_public_key=authority._attested_public_key(attributes[3]),
                maximum_risk_metric=3,
                evaluation_time_unix_ms=1_776_795_192_153,
            )

    def test_receipt_attributes_require_numeric_order_unique_ids_and_v1(self) -> None:
        ordered = der(
            0x31,
            receipt_attribute(2, b"app") + receipt_attribute(3, b"key"),
        )
        self.assertEqual(authority._receipt_attributes(ordered), {2: b"app", 3: b"key"})
        duplicate = der(
            0x31,
            receipt_attribute(2, b"app") + receipt_attribute(2, b"again"),
        )
        with self.assertRaisesRegex(authority.AuthorityError, "duplicated"):
            authority._receipt_attributes(duplicate)
        out_of_order = der(
            0x31,
            receipt_attribute(3, b"key") + receipt_attribute(2, b"app"),
        )
        with self.assertRaisesRegex(
            authority.AuthorityError, "strictly increasing"
        ):
            authority._receipt_attributes(out_of_order)
        unsupported_version = der(
            0x31, receipt_attribute_with_version(2, 2, b"app")
        )
        with self.assertRaisesRegex(authority.AuthorityError, "version is unsupported"):
            authority._receipt_attributes(unsupported_version)
        wrong_value_type = der(
            0x31,
            der(
                0x30,
                der_integer(2) + der_integer(1) + der(0x0C, b"wrapped"),
            ),
        )
        with self.assertRaisesRegex(authority.AuthorityError, "expected 0x04"):
            authority._receipt_attributes(wrong_value_type)

    def test_receipt_raw_text_and_public_key_reject_wrappers_and_bad_lengths(self) -> None:
        public_key = self._fake_validated().assertion_public_key
        spki = p256_spki(public_key)
        self.assertEqual(authority._receipt_string(b"RECEIPT", "type"), "RECEIPT")
        self.assertEqual(authority._attested_public_key(public_key), public_key)
        self.assertEqual(authority._attested_public_key(spki), public_key)
        for value, message in (
            (b"", "size"),
            (b"A" * (authority.MAX_RECEIPT_TEXT_BYTES + 1), "size"),
            (der(0x0C, b"RECEIPT"), "canonical"),
            (b"\xff", "valid text"),
        ):
            with self.subTest(text=value[:12]), self.assertRaisesRegex(
                authority.AuthorityError, message
            ):
                authority._receipt_string(value, "receipt text")
        for value in (
            der(0x04, spki),
            base64.b64encode(spki),
            public_key[:-1],
            b"A" * (production_evidence.MAX_CERTIFICATE_BYTES + 1),
        ):
            with self.subTest(public_key_length=len(value)), self.assertRaises(
                authority.AuthorityError
            ):
                authority._attested_public_key(value)

    def test_valid_cms_receipt_is_bound_to_app_key_type_time_and_risk(self) -> None:
        public_key = self._fake_validated().assertion_public_key
        receipt = self._cms_receipt(public_key=public_key)
        facts = authority.verify_apple_receipt(
            receipt,
            expected_app_id="A1B2C3D4E5.org.example.app",
            expected_public_key=public_key,
            maximum_risk_metric=3,
            evaluation_time_unix_ms=authority._now_ms(),
            root_pem_path=self.key_root / "apple-test-root.pem",
            openssl_path=Path("/usr/bin/openssl"),
            expected_root_sha256=self._root_digest("apple-test"),
            enforce_trusted_executable=True,
        )
        self.assertEqual(facts.risk_metric, 2)

    def test_checked_in_apple_root_pin_is_over_der_not_pem_bytes(self) -> None:
        payload = authority.APPLE_RECEIPT_ROOT.read_bytes()
        self.assertNotEqual(
            hashlib.sha256(payload).hexdigest(),
            authority.APPLE_ROOT_CA_G3_DER_SHA256,
        )
        self.assertEqual(
            hashlib.sha256(
                authority._pem_certificate_der(payload, "checked-in Apple root")
            ).hexdigest(),
            authority.APPLE_ROOT_CA_G3_DER_SHA256,
        )

    def test_valid_non_apple_cms_chain_is_rejected_by_pinned_root(self) -> None:
        public_key = self._fake_validated().assertion_public_key
        receipt = self._cms_receipt(
            authority_name="non-apple", public_key=public_key
        )
        with mock.patch.dict(
            os.environ,
            {"SSL_CERT_FILE": str(self.key_root / "non-apple-root.pem")},
        ), self.assertRaisesRegex(authority.AuthorityError, "pinned certificate"):
            authority.verify_apple_receipt(
                receipt,
                expected_app_id="A1B2C3D4E5.org.example.app",
                expected_public_key=public_key,
                maximum_risk_metric=3,
                evaluation_time_unix_ms=authority._now_ms(),
                root_pem_path=self.key_root / "apple-test-root.pem",
                openssl_path=Path("/usr/bin/openssl"),
                expected_root_sha256=self._root_digest("apple-test"),
                enforce_trusted_executable=True,
            )

    def test_cms_receipt_rejects_substitution_staleness_and_risk(self) -> None:
        public_key = self._fake_validated().assertion_public_key
        cases = (
            (
                self._cms_receipt(app_id="A1B2C3D4E5.org.example.other"),
                public_key,
                5,
                "App ID",
            ),
            (
                self._cms_receipt(public_key=public_key, receipt_type="ATTEST"),
                public_key,
                5,
                "not a refreshed RECEIPT",
            ),
            (
                self._cms_receipt(public_key=public_key, risk_metric=9),
                public_key,
                5,
                "risk metric exceeds",
            ),
            (
                self._cms_receipt(
                    public_key=public_key,
                    creation=datetime.now(tz=timezone.utc) - timedelta(minutes=6),
                ),
                public_key,
                5,
                "older than five minutes",
            ),
        )
        for receipt, expected_key, risk, message in cases:
            with self.subTest(message=message), self.assertRaisesRegex(
                authority.AuthorityError, message
            ):
                authority.verify_apple_receipt(
                    receipt,
                    expected_app_id="A1B2C3D4E5.org.example.app",
                    expected_public_key=expected_key,
                    maximum_risk_metric=risk,
                    evaluation_time_unix_ms=authority._now_ms(),
                    root_pem_path=self.key_root / "apple-test-root.pem",
                    openssl_path=Path("/usr/bin/openssl"),
                    expected_root_sha256=self._root_digest("apple-test"),
                    enforce_trusted_executable=True,
                )

    def test_crash_recovery_replay_and_counter_survive_restart(self) -> None:
        now_ms = 2_000_000_000_000
        validated = self._fake_validated()
        apple_facts = authority.AppleReceiptFacts(
            now_ms - 1000, now_ms + 1000, now_ms + 1_000_000, 2, now_ms
        )
        with tempfile.TemporaryDirectory() as temporary:
            state_dir = Path(temporary) / "state"
            with authority.AuthorityState(state_dir) as state:
                lease = self._insert_lease(state, validated, now_ms=now_ms)
                receipt, recovered = state.commit_consumption(
                    challenge_id=lease.challenge_id,
                    validated=validated,
                    apple_receipt=b"signed-apple-receipt",
                    apple_facts=apple_facts,
                    authority_key_id="authority-key",
                    authority_public_key_sha256=candidate_evidence.signer_public_key_sha256(
                        self.key_root / "authority.pub.pem"
                    ),
                    consumed_at_unix_ms=now_ms,
                )
                self.assertFalse(recovered)
                self.assertEqual(
                    receipt["apple_revocation_checked_at_unix_ms"], now_ms
                )
                self.assertEqual(
                    state.connection.execute(
                        "SELECT receipt_id FROM challenges WHERE challenge_id = ?",
                        (lease.challenge_id,),
                    ).fetchone(),
                    (receipt["receipt_id"],),
                )
            with authority.AuthorityState(state_dir) as restarted:
                recovered_receipt, recovered = restarted.commit_consumption(
                    challenge_id=lease.challenge_id,
                    validated=validated,
                    apple_receipt=b"",
                    apple_facts=apple_facts,
                    authority_key_id="authority-key",
                    authority_public_key_sha256=candidate_evidence.signer_public_key_sha256(
                        self.key_root / "authority.pub.pem"
                    ),
                    consumed_at_unix_ms=now_ms + 1,
                )
                self.assertTrue(recovered)
                self.assertEqual(recovered_receipt, receipt)
                with self.assertRaisesRegex(
                    authority.AuthorityError, "already consumed"
                ):
                    restarted.commit_consumption(
                        challenge_id=lease.challenge_id,
                        validated=replace(
                            validated,
                            evidence_sha256=hashlib.sha256(b"substitution").hexdigest(),
                        ),
                        apple_receipt=b"",
                        apple_facts=apple_facts,
                        authority_key_id="authority-key",
                        authority_public_key_sha256=candidate_evidence.signer_public_key_sha256(
                            self.key_root / "authority.pub.pem"
                        ),
                        consumed_at_unix_ms=now_ms + 1,
                    )
                rollback = self._fake_validated(evidence_label="rollback")
                second = self._insert_lease(
                    restarted,
                    rollback,
                    challenge_label="second-challenge",
                    now_ms=now_ms,
                )
                with self.assertRaisesRegex(authority.AuthorityError, "replay or rollback"):
                    restarted.commit_consumption(
                        challenge_id=second.challenge_id,
                        validated=rollback,
                        apple_receipt=b"new-apple-receipt",
                        apple_facts=apple_facts,
                        authority_key_id="authority-key",
                        authority_public_key_sha256=candidate_evidence.signer_public_key_sha256(
                            self.key_root / "authority.pub.pem"
                        ),
                        consumed_at_unix_ms=now_ms + 2,
                    )

    def test_catalog_promotion_id_is_durable_and_cannot_be_rebound(self) -> None:
        now_ms = 2_000_000_000_000
        promotion_id = hashlib.sha256(b"promotion-run-one").hexdigest()
        bindings = sorted(
            (
                {
                    "release_manifest_sha256": hashlib.sha256(
                        b"release-one"
                    ).hexdigest(),
                    "evidence_sha256": hashlib.sha256(b"evidence-one").hexdigest(),
                    "consumption_receipt_sha256": hashlib.sha256(
                        b"consumption-one"
                    ).hexdigest(),
                },
                {
                    "release_manifest_sha256": hashlib.sha256(
                        b"release-two"
                    ).hexdigest(),
                    "evidence_sha256": hashlib.sha256(b"evidence-two").hexdigest(),
                    "consumption_receipt_sha256": hashlib.sha256(
                        b"consumption-two"
                    ).hexdigest(),
                },
            ),
            key=lambda value: value["release_manifest_sha256"],
        )
        statuses = [
            {
                **binding,
                "app_attest_key_id": f"key-{index}",
                "apple_status_checked_at_unix_ms": now_ms - index,
                "apple_status": "good",
                "apple_status_source": production_evidence.ONLINE_REVOCATION_SOURCE,
                "refreshed_apple_receipt_sha256": hashlib.sha256(
                    f"apple-{index}".encode()
                ).hexdigest(),
                "risk_metric": index,
            }
            for index, binding in enumerate(bindings)
        ]
        catalog_sha256 = production_evidence.catalog_revalidation_digest(
            bindings, candidate_evidence
        )
        authority_key_sha256 = candidate_evidence.signer_public_key_sha256(
            self.key_root / "authority.pub.pem"
        )
        with tempfile.TemporaryDirectory() as temporary:
            state_dir = Path(temporary) / "state"
            with authority.AuthorityState(state_dir) as state:
                receipt, recovered = state.commit_catalog_revalidation(
                    promotion_id=promotion_id,
                    catalog_sha256=catalog_sha256,
                    release_statuses=statuses,
                    authority_key_id="authority-key",
                    authority_public_key_sha256=authority_key_sha256,
                    issued_at_unix_ms=now_ms,
                )
                self.assertFalse(recovered)
            with authority.AuthorityState(state_dir) as restarted:
                replayed, recovered = restarted.commit_catalog_revalidation(
                    promotion_id=promotion_id,
                    catalog_sha256=catalog_sha256,
                    release_statuses=statuses,
                    authority_key_id="authority-key",
                    authority_public_key_sha256=authority_key_sha256,
                    issued_at_unix_ms=now_ms + 1,
                )
                self.assertTrue(recovered)
                self.assertEqual(replayed, receipt)
                substituted = [dict(status) for status in statuses]
                substituted[0]["risk_metric"] = 99
                replayed, recovered = restarted.commit_catalog_revalidation(
                    promotion_id=promotion_id,
                    catalog_sha256=catalog_sha256,
                    release_statuses=substituted,
                    authority_key_id="authority-key",
                    authority_public_key_sha256=authority_key_sha256,
                    issued_at_unix_ms=now_ms + 2,
                )
                self.assertTrue(recovered)
                self.assertEqual(replayed, receipt)

                rebound_statuses = [dict(status) for status in statuses]
                rebound_statuses[0]["evidence_sha256"] = hashlib.sha256(
                    b"substituted-evidence"
                ).hexdigest()
                rebound_bindings = [
                    {
                        field: status[field]
                        for field in production_evidence.CATALOG_REVALIDATION_BINDING_FIELDS
                    }
                    for status in rebound_statuses
                ]
                rebound_catalog_sha256 = (
                    production_evidence.catalog_revalidation_digest(
                        rebound_bindings, candidate_evidence
                    )
                )
                with self.assertRaisesRegex(
                    authority.AuthorityError, "immutable release catalog"
                ):
                    restarted.commit_catalog_revalidation(
                        promotion_id=promotion_id,
                        catalog_sha256=rebound_catalog_sha256,
                        release_statuses=rebound_statuses,
                        authority_key_id="authority-key",
                        authority_public_key_sha256=authority_key_sha256,
                        issued_at_unix_ms=now_ms + 3,
                    )
                with self.assertRaisesRegex(
                    authority.AuthorityError, "another catalog or authority"
                ):
                    restarted.commit_catalog_revalidation(
                        promotion_id=promotion_id,
                        catalog_sha256=catalog_sha256,
                        release_statuses=statuses,
                        authority_key_id="substituted-authority-key",
                        authority_public_key_sha256=authority_key_sha256,
                        issued_at_unix_ms=now_ms + 4,
                    )
                with self.assertRaisesRegex(
                    authority.AuthorityError, "already consumed by an expired"
                ):
                    expiry_observation = (
                        now_ms
                        + production_evidence.MAX_ONLINE_RECEIPT_LIFETIME_MS
                        + 1
                    )
                    restarted.commit_catalog_revalidation(
                        promotion_id=promotion_id,
                        catalog_sha256=catalog_sha256,
                        release_statuses=statuses,
                        authority_key_id="authority-key",
                        authority_public_key_sha256=authority_key_sha256,
                        issued_at_unix_ms=expiry_observation,
                    )
                self.assertEqual(
                    restarted.connection.execute(
                        """
                        SELECT state, retired_at_unix_ms
                          FROM catalog_revalidations WHERE promotion_id = ?
                        """,
                        (promotion_id,),
                    ).fetchone(),
                    ("expired", expiry_observation),
                )

                recovery_promotion_id = hashlib.sha256(
                    b"promotion-run-expired-during-recovery"
                ).hexdigest()
                restarted.commit_catalog_revalidation(
                    promotion_id=recovery_promotion_id,
                    catalog_sha256=catalog_sha256,
                    release_statuses=statuses,
                    authority_key_id="authority-key",
                    authority_public_key_sha256=authority_key_sha256,
                    issued_at_unix_ms=now_ms,
                )
                with self.assertRaisesRegex(
                    authority.AuthorityError, "already consumed by an expired"
                ):
                    restarted.recover_catalog_revalidation(
                        promotion_id=recovery_promotion_id,
                        catalog_sha256=catalog_sha256,
                        bindings=bindings,
                        authority_key_id="authority-key",
                        authority_public_key_sha256=authority_key_sha256,
                        evaluation_time_unix_ms=expiry_observation,
                    )

            # A wall-clock rollback, including after restart, cannot revive
            # either promotion id after its first observed expiry.
            with authority.AuthorityState(state_dir) as rolled_back:
                for retired_promotion_id in (
                    promotion_id,
                    recovery_promotion_id,
                ):
                    with self.assertRaisesRegex(
                        authority.AuthorityError, "already consumed by an expired"
                    ):
                        rolled_back.recover_catalog_revalidation(
                            promotion_id=retired_promotion_id,
                            catalog_sha256=catalog_sha256,
                            bindings=bindings,
                            authority_key_id="authority-key",
                            authority_public_key_sha256=authority_key_sha256,
                            evaluation_time_unix_ms=now_ms + 1,
                        )
                with self.assertRaisesRegex(
                    authority.AuthorityError, "already consumed by an expired"
                ):
                    rolled_back.commit_catalog_revalidation(
                        promotion_id=promotion_id,
                        catalog_sha256=catalog_sha256,
                        release_statuses=statuses,
                        authority_key_id="authority-key",
                        authority_public_key_sha256=authority_key_sha256,
                        issued_at_unix_ms=now_ms + 1,
                    )

    def test_persisted_catalog_record_rejects_row_and_payload_corruption(
        self,
    ) -> None:
        now_ms = 2_000_000_000_000
        promotion_id = hashlib.sha256(b"persisted-record-promotion").hexdigest()
        binding = {
            "release_manifest_sha256": hashlib.sha256(b"manifest").hexdigest(),
            "evidence_sha256": hashlib.sha256(b"evidence").hexdigest(),
            "consumption_receipt_sha256": hashlib.sha256(
                b"consumption"
            ).hexdigest(),
        }
        bindings = [binding]
        statuses = [
            {
                **binding,
                "app_attest_key_id": "app-attest-key",
                "apple_status_checked_at_unix_ms": now_ms,
                "apple_status": "good",
                "apple_status_source": production_evidence.ONLINE_REVOCATION_SOURCE,
                "refreshed_apple_receipt_sha256": hashlib.sha256(
                    b"refreshed"
                ).hexdigest(),
                "risk_metric": 3,
            }
        ]
        catalog_sha256 = production_evidence.catalog_revalidation_digest(
            bindings, candidate_evidence
        )
        authority_key_sha256 = candidate_evidence.signer_public_key_sha256(
            self.key_root / "authority.pub.pem"
        )
        with tempfile.TemporaryDirectory() as temporary:
            with authority.AuthorityState(Path(temporary) / "state") as state:
                receipt, recovered = state.commit_catalog_revalidation(
                    promotion_id=promotion_id,
                    catalog_sha256=catalog_sha256,
                    release_statuses=statuses,
                    authority_key_id="authority-key",
                    authority_public_key_sha256=authority_key_sha256,
                    issued_at_unix_ms=now_ms,
                )
                self.assertFalse(recovered)
                row = state.connection.execute(
                    """
                    SELECT promotion_id, catalog_sha256, receipt_id,
                           issued_at_unix_ms, expires_at_unix_ms,
                           authority_key_id, authority_public_key_sha256,
                           receipt_payload, state, retired_at_unix_ms
                      FROM catalog_revalidations WHERE promotion_id = ?
                    """,
                    (promotion_id,),
                ).fetchone()
                self.assertIsNotNone(row)
                assert row is not None

                def validate(candidate_row: tuple[object, ...]) -> None:
                    observed = authority._validate_persisted_catalog_revalidation_record(
                        candidate_row,
                        expected_promotion_id=promotion_id,
                        expected_catalog_sha256=catalog_sha256,
                        expected_bindings=bindings,
                        expected_authority_key_id="authority-key",
                        expected_authority_public_key_sha256=authority_key_sha256,
                        evaluation_time_unix_ms=now_ms,
                    )
                    self.assertEqual(observed.receipt, receipt)

                validate(row)

                def mutate_payload(
                    mutation: object,
                ) -> tuple[object, ...]:
                    value = candidate_evidence.parse_strict_json(
                        row[7] + b"\n", "persisted catalog test payload"
                    )
                    assert callable(mutation)
                    mutation(value)
                    changed = list(row)
                    changed[7] = candidate_evidence.canonical_signature_payload(value)
                    return tuple(changed)

                payload_mutations = (
                    ("top-level extra", lambda value: value.__setitem__("extra", 1)),
                    ("top-level missing", lambda value: value.pop("status")),
                    (
                        "nested extra",
                        lambda value: value["release_statuses"][0].__setitem__(
                            "extra", 1
                        ),
                    ),
                    (
                        "nested missing",
                        lambda value: value["release_statuses"][0].pop(
                            "risk_metric"
                        ),
                    ),
                    ("schema", lambda value: value.__setitem__("schema", "wrong")),
                    ("version", lambda value: value.__setitem__("version", True)),
                    ("status", lambda value: value.__setitem__("status", "good")),
                    (
                        "receipt id",
                        lambda value: value.__setitem__(
                            "receipt_id", hashlib.sha256(b"other-receipt").hexdigest()
                        ),
                    ),
                    (
                        "promotion id",
                        lambda value: value.__setitem__(
                            "promotion_id", hashlib.sha256(b"other-run").hexdigest()
                        ),
                    ),
                    (
                        "catalog digest",
                        lambda value: value.__setitem__(
                            "catalog_sha256", hashlib.sha256(b"other-catalog").hexdigest()
                        ),
                    ),
                    (
                        "timestamp",
                        lambda value: value.__setitem__(
                            "issued_at_unix_ms", now_ms + 1
                        ),
                    ),
                    (
                        "signer",
                        lambda value: value.__setitem__(
                            "signer_key_id", "other-authority-key"
                        ),
                    ),
                    (
                        "binding",
                        lambda value: value["release_statuses"][0].__setitem__(
                            "evidence_sha256",
                            hashlib.sha256(b"other-evidence").hexdigest(),
                        ),
                    ),
                )
                for label, mutation in payload_mutations:
                    with self.subTest(label=label), self.assertRaises(
                        authority.AuthorityError
                    ):
                        validate(mutate_payload(mutation))

                row_mutations = (
                    row[:-1],
                    (*row[:1], hashlib.sha256(b"other-catalog").hexdigest(), *row[2:]),
                    (*row[:2], hashlib.sha256(b"other-receipt").hexdigest(), *row[3:]),
                    (*row[:5], "other-authority-key", *row[6:]),
                    (*row[:6], hashlib.sha256(b"other-authority").hexdigest(), *row[7:]),
                    (*row[:7], "not-a-blob", *row[8:]),
                    (*row[:8], "active", now_ms),
                    (*row[:8], "expired", None),
                )
                for index, changed_row in enumerate(row_mutations):
                    with self.subTest(row_mutation=index), self.assertRaises(
                        authority.AuthorityError
                    ):
                        validate(changed_row)

                noncanonical = list(row)
                noncanonical[7] = row[7] + b" "
                with self.assertRaises(authority.AuthorityError):
                    validate(tuple(noncanonical))

                state.connection.execute(
                    "UPDATE catalog_revalidations SET receipt_payload = ? "
                    "WHERE promotion_id = ?",
                    (row[7] + b" ", promotion_id),
                )
                with self.assertRaises(authority.AuthorityError):
                    state.recover_catalog_revalidation(
                        promotion_id=promotion_id,
                        catalog_sha256=catalog_sha256,
                        bindings=bindings,
                        authority_key_id="authority-key",
                        authority_public_key_sha256=authority_key_sha256,
                        evaluation_time_unix_ms=now_ms + 1,
                    )

    def test_concurrent_substitutions_have_exactly_one_winner(self) -> None:
        now_ms = 2_000_000_000_000
        first = self._fake_validated(evidence_label="first")
        second = replace(
            first, evidence_sha256=hashlib.sha256(b"second").hexdigest()
        )
        apple_facts = authority.AppleReceiptFacts(
            now_ms - 1000, now_ms + 1000, now_ms + 1_000_000, 2, now_ms
        )
        with tempfile.TemporaryDirectory() as temporary:
            state_dir = Path(temporary) / "state"
            with authority.AuthorityState(state_dir) as state:
                lease = self._insert_lease(state, first, now_ms=now_ms)
            barrier = threading.Barrier(2)
            results: list[str] = []
            lock = threading.Lock()

            def worker(validated: authority.ValidatedEvidence) -> None:
                try:
                    with authority.AuthorityState(state_dir) as worker_state:
                        barrier.wait(timeout=5)
                        worker_state.commit_consumption(
                            challenge_id=lease.challenge_id,
                            validated=validated,
                            apple_receipt=b"apple-receipt",
                            apple_facts=apple_facts,
                            authority_key_id="authority-key",
                            authority_public_key_sha256=(
                                candidate_evidence.signer_public_key_sha256(
                                    self.key_root / "authority.pub.pem"
                                )
                            ),
                            consumed_at_unix_ms=now_ms,
                        )
                except authority.AuthorityError:
                    result = "rejected"
                else:
                    result = "committed"
                with lock:
                    results.append(result)

            threads = [
                threading.Thread(target=worker, args=(value,))
                for value in (first, second)
            ]
            for thread in threads:
                thread.start()
            for thread in threads:
                thread.join(timeout=10)
            self.assertEqual(sorted(results), ["committed", "rejected"])

    def test_concurrent_catalog_statuses_recover_one_durable_winner(self) -> None:
        now_ms = 2_000_000_000_000
        promotion_id = hashlib.sha256(b"concurrent-catalog-promotion").hexdigest()
        binding = {
            "release_manifest_sha256": hashlib.sha256(b"manifest").hexdigest(),
            "evidence_sha256": hashlib.sha256(b"evidence").hexdigest(),
            "consumption_receipt_sha256": hashlib.sha256(
                b"consumption"
            ).hexdigest(),
        }
        bindings = [binding]
        catalog_sha256 = production_evidence.catalog_revalidation_digest(
            bindings, candidate_evidence
        )
        authority_key_sha256 = candidate_evidence.signer_public_key_sha256(
            self.key_root / "authority.pub.pem"
        )

        def statuses(sequence: int) -> list[dict[str, object]]:
            return [
                {
                    **binding,
                    "app_attest_key_id": "app-attest-key",
                    "apple_status_checked_at_unix_ms": now_ms - sequence,
                    "apple_status": "good",
                    "apple_status_source": production_evidence.ONLINE_REVOCATION_SOURCE,
                    "refreshed_apple_receipt_sha256": hashlib.sha256(
                        f"refreshed-{sequence}".encode()
                    ).hexdigest(),
                    "risk_metric": sequence,
                }
            ]

        with tempfile.TemporaryDirectory() as temporary:
            state_dir = Path(temporary) / "state"
            with authority.AuthorityState(state_dir):
                pass
            barrier = threading.Barrier(2)
            results: list[tuple[dict[str, object], bool]] = []
            failures: list[BaseException] = []
            lock = threading.Lock()

            def worker(sequence: int) -> None:
                try:
                    with authority.AuthorityState(state_dir) as worker_state:
                        barrier.wait(timeout=5)
                        result = worker_state.commit_catalog_revalidation(
                            promotion_id=promotion_id,
                            catalog_sha256=catalog_sha256,
                            release_statuses=statuses(sequence),
                            authority_key_id="authority-key",
                            authority_public_key_sha256=authority_key_sha256,
                            issued_at_unix_ms=now_ms,
                        )
                except BaseException as error:
                    with lock:
                        failures.append(error)
                else:
                    with lock:
                        results.append(result)

            threads = [
                threading.Thread(target=worker, args=(sequence,))
                for sequence in (1, 2)
            ]
            for thread in threads:
                thread.start()
            for thread in threads:
                thread.join(timeout=15)
                self.assertFalse(thread.is_alive())
            self.assertEqual(failures, [])
            self.assertEqual(sorted(recovered for _, recovered in results), [False, True])
            self.assertEqual(results[0][0], results[1][0])
            self.assertEqual(
                candidate_evidence.canonical_signature_payload(results[0][0]),
                candidate_evidence.canonical_signature_payload(results[1][0]),
            )
            with authority.AuthorityState(state_dir) as state:
                self.assertEqual(
                    state.connection.execute(
                        "SELECT COUNT(*) FROM catalog_revalidations"
                    ).fetchone(),
                    (1,),
                )

    def test_complete_evidence_consumes_after_commit_and_recovers_crash(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = fixture_support.ProductionFixture(
                fixture_support.Fixture(
                    Path(temporary),
                    self.key_root / "lab.key.pem",
                    self.key_root / "lab.pub.pem",
                ),
                self.key_root / "authority.key.pem",
                self.key_root / "authority.pub.pem",
            )
            capture_measurements = Path(temporary) / "capture-app-measurements.json"
            candidate_evidence.write_private_json(
                capture_measurements,
                fixture.capture_app_code_sign_measurements,
            )
            issued_state_dir = Path(temporary) / "issued-state"
            with authority.AuthorityState(issued_state_dir) as issued_state:
                issued_lease = authority.issue_challenge(
                    issued_state,
                    artifact_root=fixture.raw,
                    production_policy_path=fixture.policy,
                    capture_app_code_sign_measurements_path=capture_measurements,
                    release_manifest_sha256=fixture.release_manifest_sha256,
                )
                self.assertNotEqual(
                    issued_lease.challenge_id, issued_lease.consumption_id
                )
                self.assertEqual(
                    issued_lease.request["schema"],
                    "iroha.kagemusha.ios.app_attest_capture_request.v1",
                )
                issued_attestation = candidate_evidence.parse_strict_json(
                    base64.b64decode(
                        issued_lease.request["attestation_client_data_base64"],
                        validate=True,
                    ),
                    "issued attestation challenge",
                )
                self.assertEqual(
                    issued_attestation[
                        "capture_app_code_sign_measurements_sha256"
                    ],
                    hashlib.sha256(capture_measurements.read_bytes()).hexdigest(),
                )
            with authority.AuthorityState(issued_state_dir) as restarted_issue:
                self.assertEqual(
                    restarted_issue.challenge_row(issued_lease.challenge_id)[1],
                    issued_lease.consumption_id,
                )
            validated = authority._load_validated_evidence(
                evidence_path=fixture.evidence,
                artifact_root=fixture.raw,
                production_policy_path=fixture.policy,
                capture_app_code_sign_measurements_path=capture_measurements,
                trusted_lab_key_id=fixture.key_id,
                trusted_lab_public_key_path=fixture.public_key,
            )
            substituted_measurements = dict(
                fixture.capture_app_code_sign_measurements
            )
            substituted_measurements["executable_sha256"] = hashlib.sha256(
                b"substituted-capture-app"
            ).hexdigest()
            substituted_path = Path(temporary) / "substituted-measurements.json"
            candidate_evidence.write_private_json(
                substituted_path, substituted_measurements
            )
            with self.assertRaisesRegex(
                authority.AuthorityError,
                "does not embed the exact prepared capture-app measurement",
            ):
                authority._load_validated_evidence(
                    evidence_path=fixture.evidence,
                    artifact_root=fixture.raw,
                    production_policy_path=fixture.policy,
                    capture_app_code_sign_measurements_path=substituted_path,
                    trusted_lab_key_id=fixture.key_id,
                    trusted_lab_public_key_path=fixture.public_key,
                )
            state_dir = Path(temporary) / "authority-state"
            output = Path(temporary) / "authority-output"
            output.mkdir(mode=0o700)
            receipt_output = output / "online-receipt.json"
            now_ms = authority._now_ms()
            apple_receipt = self._cms_receipt(
                app_id=validated.app_id,
                public_key=validated.assertion_public_key,
                creation=datetime.fromtimestamp(now_ms / 1000, tz=timezone.utc),
            )
            with authority.AuthorityState(state_dir) as state:
                lease = authority.ChallengeLease(
                    challenge_id=hashlib.sha256(b"physical-challenge").hexdigest(),
                    consumption_id=hashlib.sha256(b"physical-consumption").hexdigest(),
                    issued_at_unix_ms=validated.facts.evaluated_at_unix_ms,
                    expires_at_unix_ms=(
                        validated.facts.evaluated_at_unix_ms
                        + authority.MAX_CHALLENGE_LIFETIME_MS
                    ),
                    request_sha256=validated.capture_request_sha256,
                    request=validated.capture_request,
                )
                state.insert_challenge(
                    lease,
                    validated.facts.attestation_challenge_nonce,
                    validated.facts.assertion_challenge_nonce,
                )
                with self.assertRaisesRegex(RuntimeError, "synthetic crash"):
                    authority.consume_evidence(
                        state,
                        challenge_id=lease.challenge_id,
                        evidence_path=fixture.evidence,
                        artifact_root=fixture.raw,
                        production_policy_path=fixture.policy,
                        capture_app_code_sign_measurements_path=capture_measurements,
                        trusted_lab_key_id=fixture.key_id,
                        trusted_lab_public_key_path=fixture.public_key,
                        authority_key_id=fixture.freshness_key_id,
                        authority_private_key_path=self.key_root / "authority.key.pem",
                        authority_public_key_path=self.key_root / "authority.pub.pem",
                        maximum_risk_metric=3,
                        devicecheck_jwt_fd=None,
                        devicecheck_jwt_file=self._write_jwt(
                            Path(temporary), validated.policy["app_id_prefix"], now_ms
                        ),
                        apple_transport=lambda embedded, _: (
                            apple_receipt
                            if embedded == validated.embedded_apple_receipt
                            else b"substituted"
                        ),
                        evaluation_time=lambda: now_ms,
                        after_commit=lambda: (_ for _ in ()).throw(
                            RuntimeError("synthetic crash")
                        ),
                        apple_root_path=self.key_root / "apple-test-root.pem",
                        openssl_path=Path("/usr/bin/openssl"),
                        expected_apple_root_sha256=self._root_digest("apple-test"),
                    )
            with authority.AuthorityState(state_dir) as restarted:
                signed = authority.consume_evidence(
                    restarted,
                    challenge_id=lease.challenge_id,
                    evidence_path=fixture.evidence,
                    artifact_root=fixture.raw,
                    production_policy_path=fixture.policy,
                    capture_app_code_sign_measurements_path=capture_measurements,
                    trusted_lab_key_id=fixture.key_id,
                    trusted_lab_public_key_path=fixture.public_key,
                    authority_key_id=fixture.freshness_key_id,
                    authority_private_key_path=self.key_root / "authority.key.pem",
                    authority_public_key_path=self.key_root / "authority.pub.pem",
                    maximum_risk_metric=3,
                    output_path=receipt_output,
                    evaluation_time=lambda: now_ms + 1,
                )
            self.assertEqual(signed["status"], "issued-and-consumed-once")
            errors = production_evidence.validate_production_signed_evidence(
                fixture.evidence,
                fixture.raw,
                fixture.key_id,
                fixture.public_key,
                fixture.policy,
                candidate_evidence,
                freshness_receipt_path=receipt_output,
                trusted_freshness_key_id=fixture.freshness_key_id,
                trusted_freshness_public_key_path=self.key_root / "authority.pub.pem",
                evaluation_time_unix_ms=now_ms + 1,
            )
            self.assertEqual(errors, [])

    def test_two_time_separated_releases_receive_one_current_catalog_receipt(
        self,
    ) -> None:
        now_ms = authority._now_ms()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fixtures: list[fixture_support.ProductionFixture] = []
            measurements: list[Path] = []
            for index, age_days in enumerate((2, 1), start=1):
                release_root = root / f"release-{index}"
                release_root.mkdir(mode=0o700)
                fixture = fixture_support.ProductionFixture(
                    fixture_support.Fixture(
                        release_root,
                        self.key_root / "lab.key.pem",
                        self.key_root / "lab.pub.pem",
                    ),
                    self.key_root / "authority.key.pem",
                    self.key_root / "authority.pub.pem",
                    release_manifest_sha256=hashlib.sha256(
                        f"release-manifest-{index}".encode()
                    ).hexdigest(),
                    evaluated_at_unix_ms=(
                        now_ms - age_days * 24 * 60 * 60 * 1000
                    ),
                )
                measurement = root / f"capture-measurements-{index}.json"
                candidate_evidence.write_private_json(
                    measurement, fixture.capture_app_code_sign_measurements
                )
                fixtures.append(fixture)
                measurements.append(measurement)
            self.assertEqual(
                fixtures[0].policy.read_bytes(), fixtures[1].policy.read_bytes()
            )
            promotion_id = hashlib.sha256(b"catalog-promotion-run").hexdigest()
            request = {
                "schema": authority.CATALOG_REVALIDATION_REQUEST_SCHEMA,
                "version": 1,
                "promotion_id": promotion_id,
                "releases": [
                    {
                        "evidence_path": str(fixture.evidence.resolve()),
                        "artifact_root": str(fixture.raw.resolve()),
                        "consumption_receipt_path": str(
                            fixture.freshness_receipt.resolve()
                        ),
                        "capture_app_code_sign_measurements_path": str(
                            measurement.resolve()
                        ),
                    }
                    for fixture, measurement in reversed(
                        list(zip(fixtures, measurements))
                    )
                ],
            }
            request_path = root / "catalog-revalidation-request.json"
            candidate_evidence.write_private_json(request_path, request)
            output = root / "catalog-revalidation-receipt.json"
            refreshed = self._cms_receipt(
                app_id="A1B2C3D4E5.org.hyperledger.iroha.kagemusha.appattestlab",
                public_key=fixtures[0].assertion_public_key,
                creation=datetime.fromtimestamp(now_ms / 1000, tz=timezone.utc),
            )
            jwt_path = self._write_jwt(root, "A1B2C3D4E5", now_ms)
            with authority.AuthorityState(root / "state") as state:
                with self.assertRaisesRegex(RuntimeError, "synthetic catalog crash"):
                    authority.revalidate_catalog(
                        state,
                        request_path=request_path,
                        production_policy_path=fixtures[0].policy,
                        trusted_lab_key_id=fixtures[0].key_id,
                        trusted_lab_public_key_path=fixtures[0].public_key,
                        original_receipt_authority_key_id=(
                            fixtures[0].freshness_key_id
                        ),
                        original_receipt_authority_public_key_path=(
                            fixtures[0].freshness_public_key
                        ),
                        authority_key_id=fixtures[0].freshness_key_id,
                        authority_private_key_path=(
                            self.key_root / "authority.key.pem"
                        ),
                        authority_public_key_path=(
                            self.key_root / "authority.pub.pem"
                        ),
                        maximum_risk_metric=3,
                        devicecheck_jwt_file=jwt_path,
                        output_path=output,
                        apple_transport=lambda embedded, _jwt: (
                            refreshed
                            if embedded == b"synthetic-receipt-not-production"
                            else b"substituted"
                        ),
                        evaluation_time=lambda: now_ms,
                        after_commit=lambda: (_ for _ in ()).throw(
                            RuntimeError("synthetic catalog crash")
                        ),
                        apple_root_path=self.key_root / "apple-test-root.pem",
                        openssl_path=Path("/usr/bin/openssl"),
                        expected_apple_root_sha256=self._root_digest("apple-test"),
                    )
                self.assertFalse(output.exists())
                self.assertEqual(
                    state.connection.execute(
                        "SELECT COUNT(*) FROM catalog_revalidations"
                    ).fetchone(),
                    (1,),
                )
            with authority.AuthorityState(root / "state") as restarted:
                receipt = authority.revalidate_catalog(
                    restarted,
                    request_path=request_path,
                    production_policy_path=fixtures[0].policy,
                    trusted_lab_key_id=fixtures[0].key_id,
                    trusted_lab_public_key_path=fixtures[0].public_key,
                    original_receipt_authority_key_id=(
                        fixtures[0].freshness_key_id
                    ),
                    original_receipt_authority_public_key_path=(
                        fixtures[0].freshness_public_key
                    ),
                    authority_key_id=fixtures[0].freshness_key_id,
                    authority_private_key_path=(
                        self.key_root / "authority.key.pem"
                    ),
                    authority_public_key_path=(
                        self.key_root / "authority.pub.pem"
                    ),
                    maximum_risk_metric=3,
                    output_path=output,
                    apple_transport=lambda _embedded, _jwt: (_ for _ in ()).throw(
                        AssertionError("crash recovery must not call Apple")
                    ),
                    evaluation_time=lambda: now_ms + 1,
                )
            self.assertTrue(output.is_file())
            self.assertEqual(len(receipt["release_statuses"]), 2)
            bindings = sorted(
                (
                    production_evidence.catalog_revalidation_binding(
                        fixture.release_manifest_sha256,
                        fixture.evidence.read_bytes(),
                        fixture.freshness_receipt.read_bytes(),
                    )
                    for fixture in fixtures
                ),
                key=lambda value: value["release_manifest_sha256"],
            )
            validation_errors = (
                production_evidence.validate_catalog_revalidation_receipt(
                    output,
                    fixtures[0].freshness_key_id,
                    fixtures[0].freshness_public_key,
                    promotion_id,
                    bindings,
                    fixtures[0].key_id,
                    fixtures[0].public_key,
                    candidate_evidence,
                    evaluation_time_unix_ms=now_ms + 1,
                )
            )
            self.assertEqual(validation_errors, [])

    @staticmethod
    def _write_jwt(directory: Path, team_id: str, now_ms: int) -> Path:
        path = directory / "devicecheck.jwt"
        path.write_bytes(jwt(team_id, now_ms // 1000))
        path.chmod(0o600)
        return path


if __name__ == "__main__":
    unittest.main()
