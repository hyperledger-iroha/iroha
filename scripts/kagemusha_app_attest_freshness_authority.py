"""Durable online freshness authority for production Kagemusha App Attest.

The authority has three deliberately separate operations. ``issue`` durably
reserves a one-time challenge pair and consumption identifier before a request
is copied to a physical iPhone. ``consume`` validates the complete signed
production evidence, refreshes the embedded App Attest receipt against Apple's
production service, verifies the returned CMS receipt, atomically advances the
per-key assertion counter, and only then signs the repository's canonical
freshness/consumption receipt. ``revalidate-catalog`` validates immutable
historical consumption evidence, refreshes every release's Apple status, and
durably reserves one promotion-scoped exact-catalog receipt before signing.

The exact downstream v1 receipt retains fields named ``apple_revocation_*``.
For this authority, ``good`` has the deliberately narrower App Attest meaning:
Apple's production ``attestationData`` endpoint accepted the embedded receipt,
the refreshed CMS receipt chained to the pinned Apple Root CA G3, its signed
app/key/time/type fields matched, and its signed risk metric was within the
operator-supplied cap. This is not a claim that ``attestationData`` is a
general-purpose Apple PKI CRL or OCSP service; static certificate-digest
revocations remain part of the separately signed production policy.

DeviceCheck JWTs are accepted only from an owner-private file or an inherited
file descriptor. They are held in memory for the single HTTPS request and are
never stored in the authority database or emitted in diagnostics.
"""

from __future__ import annotations

import argparse
import base64
from dataclasses import dataclass
from datetime import datetime, timezone
import hashlib
import http.client
import json
import os
from pathlib import Path
import re
import secrets
import socket
import sqlite3
import ssl
import stat
import subprocess
import sys
import tempfile
import time
from typing import Any, Callable, Optional


SCRIPT_DIRECTORY = Path(__file__).resolve().parent
if str(SCRIPT_DIRECTORY) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIRECTORY))

import kagemusha_candidate_ios_evidence as candidate_evidence
import kagemusha_production_ios_evidence as production_evidence


CHALLENGE_LEASE_SCHEMA = (
    "iroha.kagemusha.ios.app_attest_online_challenge_lease.v1"
)
CATALOG_REVALIDATION_REQUEST_SCHEMA = (
    "iroha.kagemusha.ios.app_attest_catalog_revalidation_request.v1"
)
STATE_SCHEMA_VERSION = 3
APPLE_PRODUCTION_HOST = "data.appattest.apple.com"
APPLE_PRODUCTION_PATH = "/v1/attestationData"
APPLE_RECEIPT_ROOT = SCRIPT_DIRECTORY.parent / "certs/apple_root_ca_g3.pem"
# SHA-256 of the decoded DER certificate, intentionally not the PEM file bytes.
APPLE_ROOT_CA_G3_DER_SHA256 = (
    "63343abfb89a6a03ebb57e9b3f5fa7be7c4f5c756f3017b3a8c488c3653e9179"
)
APPLE_RECEIPT_TYPE = "RECEIPT"
MAX_CHALLENGE_LIFETIME_MS = 5 * 60 * 1000
MAX_RECEIPT_CREATION_AGE_MS = 5 * 60 * 1000
MAX_CLOCK_SKEW_MS = 30 * 1000
MAX_JWT_BYTES = 16 * 1024
MAX_APPLE_RESPONSE_BYTES = 128 * 1024
MAX_CMS_CONTENT_BYTES = 128 * 1024
MAX_CAPTURE_APP_MEASUREMENTS_BYTES = 64 * 1024
MAX_RECEIPT_ATTRIBUTES = 64
MAX_RECEIPT_TEXT_BYTES = 1024
MAX_OUTSTANDING_CHALLENGES = 4096
MAX_DURABLE_CONSUMPTIONS = 8192
MAX_DURABLE_CATALOG_REVALIDATIONS = 8192
MAX_CATALOG_REVALIDATION_REQUEST_BYTES = 256 * 1024
DEFAULT_CONNECT_TIMEOUT_SECONDS = 5.0
DEFAULT_TOTAL_TIMEOUT_SECONDS = 20.0
SHA256_RE = re.compile(r"[0-9a-f]{64}")
JWT_PART_RE = re.compile(rb"[A-Za-z0-9_-]+")
RFC3339_MILLISECONDS_RE = re.compile(
    r"([0-9]{4})-([0-9]{2})-([0-9]{2})T"
    r"([0-9]{2}):([0-9]{2}):([0-9]{2})(?:\.([0-9]{3}))?Z"
)
CHALLENGE_TABLE_COLUMNS = (
    "challenge_id",
    "consumption_id",
    "issued_at_unix_ms",
    "expires_at_unix_ms",
    "request_sha256",
    "request_json",
    "attestation_nonce",
    "assertion_nonce",
    "state",
    "evidence_sha256",
    "receipt_id",
    "key_id",
    "assertion_counter",
    "receipt_payload",
    "apple_receipt",
    "apple_risk_metric",
    "consumed_at_unix_ms",
)
APP_ATTEST_KEY_TABLE_COLUMNS = (
    "key_id",
    "public_key_sha256",
    "assertion_counter",
    "apple_receipt",
    "apple_risk_metric",
    "updated_at_unix_ms",
)
CATALOG_REVALIDATION_TABLE_COLUMNS = (
    "promotion_id",
    "catalog_sha256",
    "receipt_id",
    "issued_at_unix_ms",
    "expires_at_unix_ms",
    "authority_key_id",
    "authority_public_key_sha256",
    "receipt_payload",
    "state",
    "retired_at_unix_ms",
)
LEGACY_CATALOG_REVALIDATION_TABLE_COLUMNS_V2 = (
    "promotion_id",
    "catalog_sha256",
    "receipt_id",
    "issued_at_unix_ms",
    "expires_at_unix_ms",
    "authority_key_id",
    "authority_public_key_sha256",
    "receipt_payload",
)
CATALOG_REVALIDATION_REQUEST_FIELDS = frozenset(
    {"schema", "version", "promotion_id", "releases"}
)
CATALOG_REVALIDATION_REQUEST_RELEASE_FIELDS = frozenset(
    {
        "evidence_path",
        "artifact_root",
        "consumption_receipt_path",
        "capture_app_code_sign_measurements_path",
    }
)


class AuthorityError(RuntimeError):
    """Raised when the online authority must fail closed."""


@dataclass(frozen=True)
class ChallengeLease:
    """Durably reserved challenge material for one physical-device run."""

    challenge_id: str
    consumption_id: str
    issued_at_unix_ms: int
    expires_at_unix_ms: int
    request_sha256: str
    request: dict[str, Any]

    def public_value(self) -> dict[str, Any]:
        """Return the canonical operator-facing lease without secret state."""

        return {
            "schema": CHALLENGE_LEASE_SCHEMA,
            "version": 1,
            "challenge_id": self.challenge_id,
            "consumption_id": self.consumption_id,
            "issued_at_unix_ms": self.issued_at_unix_ms,
            "expires_at_unix_ms": self.expires_at_unix_ms,
            "capture_request_sha256": self.request_sha256,
        }


@dataclass(frozen=True)
class ValidatedEvidence:
    """Immutable facts extracted from one fully validated production envelope."""

    evidence: dict[str, Any]
    policy: dict[str, Any]
    facts: Any
    evidence_sha256: str
    policy_sha256: str
    release_manifest_sha256: str
    lab_signer_key_id: str
    lab_signer_public_key_sha256: str
    capture_request: dict[str, Any]
    capture_request_sha256: str
    embedded_apple_receipt: bytes
    app_id: str
    assertion_public_key: bytes


@dataclass(frozen=True)
class AppleReceiptFacts:
    """Security-relevant fields from a verified Apple CMS receipt."""

    creation_time_unix_ms: int
    not_before_unix_ms: int
    expiration_time_unix_ms: int
    risk_metric: int
    verified_at_unix_ms: int


@dataclass(frozen=True)
class CatalogRevalidationRecord:
    """One fully validated durable catalog-revalidation database record."""

    receipt: dict[str, Any]
    bindings: tuple[dict[str, str], ...]
    promotion_id: str
    catalog_sha256: str
    receipt_id: str
    issued_at_unix_ms: int
    expires_at_unix_ms: int
    authority_key_id: str
    authority_public_key_sha256: str
    state: str
    retired_at_unix_ms: Optional[int]


def _now_ms() -> int:
    return time.time_ns() // 1_000_000


def _require_sha256(value: str, label: str) -> str:
    if (
        not isinstance(value, str)
        or SHA256_RE.fullmatch(value) is None
        or value == "0" * 64
    ):
        raise AuthorityError(f"{label} must be a nonzero lowercase SHA-256 digest")
    return value


def _random_digest(domain: bytes) -> str:
    return hashlib.sha256(domain + b"\0" + secrets.token_bytes(32)).hexdigest()


def _directory_identity(value: os.stat_result) -> tuple[int, ...]:
    return (
        value.st_dev,
        value.st_ino,
        value.st_mode,
        value.st_nlink,
        value.st_uid,
        value.st_mtime_ns,
        value.st_ctime_ns,
    )


def _directory_custody_identity(value: os.stat_result) -> tuple[int, ...]:
    return (
        value.st_dev,
        value.st_ino,
        value.st_mode,
        value.st_uid,
    )


def _require_private_state_directory(path: Path) -> Path:
    """Resolve one owner-private non-aliased directory used for durable state."""

    try:
        before = path.lstat()
    except FileNotFoundError:
        try:
            path.mkdir(mode=0o700, parents=False)
        except OSError as error:
            raise AuthorityError("authority state directory could not be created") from error
        before = path.lstat()
    except OSError as error:
        raise AuthorityError("authority state directory metadata could not be read") from error
    if stat.S_ISLNK(before.st_mode) or not stat.S_ISDIR(before.st_mode):
        raise AuthorityError("authority state directory must be a non-symlink directory")
    if before.st_uid != os.geteuid():
        raise AuthorityError("authority state directory must be owned by the current user")
    if stat.S_IMODE(before.st_mode) & 0o077:
        raise AuthorityError("authority state directory must be owner-private (0700)")
    try:
        resolved = path.resolve(strict=True)
        after = resolved.lstat()
    except OSError as error:
        raise AuthorityError("authority state directory could not be resolved") from error
    if _directory_identity(before) != _directory_identity(after):
        raise AuthorityError("authority state directory changed while resolving")
    _validate_state_ancestors(resolved)
    return resolved


def _validate_state_ancestors(path: Path) -> None:
    """Reject writable or foreign-custodied ancestors of durable state."""

    for ancestor in (path, *path.parents):
        try:
            metadata = ancestor.lstat()
        except OSError as error:
            raise AuthorityError("authority state ancestor could not be inspected") from error
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            raise AuthorityError("authority state ancestor must be a real directory")
        if metadata.st_uid not in {0, os.geteuid()}:
            raise AuthorityError("authority state ancestor has foreign custody")
        writable = stat.S_IMODE(metadata.st_mode) & 0o022
        root_sticky = metadata.st_uid == 0 and bool(metadata.st_mode & stat.S_ISVTX)
        if writable and not root_sticky:
            raise AuthorityError(
                "authority state ancestor must not be group/world writable"
            )


def _validate_sqlite_artifact(path: Path, label: str) -> None:
    try:
        metadata = path.lstat()
    except FileNotFoundError:
        return
    except OSError as error:
        raise AuthorityError("authority database metadata could not be read") from error
    if (
        stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISREG(metadata.st_mode)
        or metadata.st_nlink != 1
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) & 0o077
    ):
        raise AuthorityError(
            f"{label} must be an owner-private singly linked regular file"
        )


def _validate_sqlite_artifacts(database_path: Path) -> None:
    _validate_sqlite_artifact(database_path, "authority database")
    _validate_sqlite_artifact(
        Path(str(database_path) + "-wal"), "authority database WAL"
    )
    _validate_sqlite_artifact(
        Path(str(database_path) + "-shm"), "authority database shared memory"
    )
    _validate_sqlite_artifact(
        Path(str(database_path) + "-journal"), "authority database rollback journal"
    )


def _require_new_private_output(path: Path, label: str) -> Path:
    if not path.is_absolute() or path.name in {"", ".", ".."}:
        raise AuthorityError(f"{label} must be a new absolute file")
    try:
        parent = path.parent.resolve(strict=True)
        before = candidate_evidence._validate_private_directory(parent, f"{label} parent")
        _validate_state_ancestors(parent)
        after = parent.lstat()
    except (OSError, candidate_evidence.EvidenceError) as error:
        raise AuthorityError(str(error)) from error
    if _directory_identity(before) != _directory_identity(after):
        raise AuthorityError(f"{label} parent changed while resolving")
    target = parent / path.name
    try:
        target.lstat()
    except FileNotFoundError:
        return target
    except OSError as error:
        raise AuthorityError(f"{label} metadata could not be read") from error
    raise AuthorityError(f"{label} already exists")


def _write_new_private_json(path: Path, value: dict[str, Any], label: str) -> None:
    """Durably publish one owner-private canonical JSON file without replacement."""

    target = _require_new_private_output(path, label)
    payload = candidate_evidence.canonical_json_bytes(value)
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_CLOEXEC"):
        flags |= os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = -1
    created = False
    try:
        descriptor = os.open(target, flags, 0o600)
        created = True
        offset = 0
        while offset < len(payload):
            written = os.write(descriptor, payload[offset:])
            if written <= 0:
                raise OSError("short authority output write")
            offset += written
        os.fsync(descriptor)
        os.close(descriptor)
        descriptor = -1
        _validate_sqlite_artifact(target, label)
        parent_descriptor = os.open(target.parent, os.O_RDONLY)
        try:
            os.fsync(parent_descriptor)
        finally:
            os.close(parent_descriptor)
    except (OSError, candidate_evidence.EvidenceError, AuthorityError) as error:
        if descriptor >= 0:
            os.close(descriptor)
        if created:
            try:
                target.unlink()
            except OSError:
                pass
        if isinstance(error, AuthorityError):
            raise
        raise AuthorityError(f"{label} could not be written durably") from error


class AuthorityState:
    """SQLite-backed, restart-safe replay and assertion-counter authority."""

    def __init__(self, state_directory: Path) -> None:
        self.directory = _require_private_state_directory(state_directory)
        self.directory_identity = _directory_custody_identity(self.directory.lstat())
        self.path = self.directory / "app-attest-freshness-authority-v1.sqlite3"
        _validate_sqlite_artifacts(self.path)
        previous_umask = os.umask(0o077)
        try:
            self.connection = sqlite3.connect(
                str(self.path), timeout=10.0, isolation_level=None
            )
        except sqlite3.Error as error:
            raise AuthorityError("authority database could not be opened") from error
        finally:
            os.umask(previous_umask)
        try:
            self.path.chmod(0o600)
            self.connection.execute("PRAGMA busy_timeout = 10000")
            self.connection.execute("PRAGMA journal_mode = WAL")
            self.connection.execute("PRAGMA synchronous = FULL")
            self.connection.execute("PRAGMA fullfsync = ON")
            self.connection.execute("PRAGMA foreign_keys = ON")
            self.connection.execute("PRAGMA trusted_schema = OFF")
            self.connection.execute("PRAGMA temp_store = MEMORY")
            self._initialize()
            self._validate_database_configuration()
            directory_descriptor = os.open(self.directory, os.O_RDONLY)
            try:
                os.fsync(directory_descriptor)
            finally:
                os.close(directory_descriptor)
        except (OSError, sqlite3.Error, AuthorityError) as error:
            self.connection.close()
            if isinstance(error, AuthorityError):
                raise
            raise AuthorityError("authority database initialization failed") from error
        try:
            self._validate_storage()
        except AuthorityError:
            self.connection.close()
            raise

    def __enter__(self) -> "AuthorityState":
        return self

    def __exit__(self, *_: object) -> None:
        self.connection.close()

    def _initialize(self) -> None:
        self.connection.executescript(
            """
            CREATE TABLE IF NOT EXISTS authority_metadata (
                singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                schema_version INTEGER NOT NULL
            );
            INSERT OR IGNORE INTO authority_metadata(singleton, schema_version)
                VALUES (1, 3);
            CREATE TABLE IF NOT EXISTS challenges (
                challenge_id TEXT PRIMARY KEY,
                consumption_id TEXT NOT NULL UNIQUE,
                issued_at_unix_ms INTEGER NOT NULL,
                expires_at_unix_ms INTEGER NOT NULL,
                request_sha256 TEXT NOT NULL UNIQUE,
                request_json BLOB NOT NULL,
                attestation_nonce BLOB NOT NULL UNIQUE,
                assertion_nonce BLOB NOT NULL UNIQUE,
                state TEXT NOT NULL CHECK (state IN ('issued', 'consumed')),
                evidence_sha256 TEXT UNIQUE,
                receipt_id TEXT UNIQUE,
                key_id TEXT,
                assertion_counter INTEGER,
                receipt_payload BLOB,
                apple_receipt BLOB,
                apple_risk_metric INTEGER,
                consumed_at_unix_ms INTEGER,
                CHECK (expires_at_unix_ms > issued_at_unix_ms)
            );
            CREATE TABLE IF NOT EXISTS app_attest_keys (
                key_id TEXT PRIMARY KEY,
                public_key_sha256 TEXT NOT NULL,
                assertion_counter INTEGER NOT NULL,
                apple_receipt BLOB NOT NULL,
                apple_risk_metric INTEGER NOT NULL,
                updated_at_unix_ms INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS catalog_revalidations (
                promotion_id TEXT PRIMARY KEY,
                catalog_sha256 TEXT NOT NULL,
                receipt_id TEXT NOT NULL UNIQUE,
                issued_at_unix_ms INTEGER NOT NULL,
                expires_at_unix_ms INTEGER NOT NULL,
                authority_key_id TEXT NOT NULL,
                authority_public_key_sha256 TEXT NOT NULL,
                receipt_payload BLOB NOT NULL,
                state TEXT NOT NULL CHECK (state IN ('active', 'expired')),
                retired_at_unix_ms INTEGER,
                CHECK (expires_at_unix_ms > issued_at_unix_ms),
                CHECK (
                    (state = 'active' AND retired_at_unix_ms IS NULL)
                    OR (state = 'expired' AND retired_at_unix_ms > expires_at_unix_ms)
                )
            );
            CREATE INDEX IF NOT EXISTS challenges_state_expiry_v1
                ON challenges(state, expires_at_unix_ms);
            """
        )
        row = self.connection.execute(
            "SELECT schema_version FROM authority_metadata WHERE singleton = 1"
        ).fetchone()
        if row in ((1,), (2,)):
            self.connection.execute("BEGIN IMMEDIATE")
            try:
                row = self.connection.execute(
                    "SELECT schema_version FROM authority_metadata WHERE singleton = 1"
                ).fetchone()
                if row in ((1,), (2,)):
                    catalog_revalidation_columns = tuple(
                        value[1]
                        for value in self.connection.execute(
                            "PRAGMA table_info(catalog_revalidations)"
                        )
                    )
                    if (
                        catalog_revalidation_columns
                        == LEGACY_CATALOG_REVALIDATION_TABLE_COLUMNS_V2
                    ):
                        self.connection.execute(
                            """
                            ALTER TABLE catalog_revalidations
                            ADD COLUMN state TEXT NOT NULL DEFAULT 'active'
                                CHECK (state IN ('active', 'expired'))
                            """
                        )
                        self.connection.execute(
                            """
                            ALTER TABLE catalog_revalidations
                            ADD COLUMN retired_at_unix_ms INTEGER
                            """
                        )
                    elif (
                        catalog_revalidation_columns
                        != CATALOG_REVALIDATION_TABLE_COLUMNS
                    ):
                        raise AuthorityError(
                            "authority catalog revalidation table schema is not migratable"
                        )
                    self.connection.execute(
                        """
                        UPDATE authority_metadata
                           SET schema_version = 3 WHERE singleton = 1
                        """
                    )
                self.connection.execute("COMMIT")
            except BaseException:
                try:
                    self.connection.execute("ROLLBACK")
                except sqlite3.Error:
                    pass
                raise
            row = self.connection.execute(
                "SELECT schema_version FROM authority_metadata WHERE singleton = 1"
            ).fetchone()
        if row != (STATE_SCHEMA_VERSION,):
            raise AuthorityError("authority database schema version is unsupported")
        challenge_columns = tuple(
            row[1] for row in self.connection.execute("PRAGMA table_info(challenges)")
        )
        key_columns = tuple(
            row[1] for row in self.connection.execute("PRAGMA table_info(app_attest_keys)")
        )
        catalog_revalidation_columns = tuple(
            row[1]
            for row in self.connection.execute(
                "PRAGMA table_info(catalog_revalidations)"
            )
        )
        if challenge_columns != CHALLENGE_TABLE_COLUMNS:
            raise AuthorityError("authority challenge table schema is not exact")
        if key_columns != APP_ATTEST_KEY_TABLE_COLUMNS:
            raise AuthorityError("authority App Attest key table schema is not exact")
        if catalog_revalidation_columns != CATALOG_REVALIDATION_TABLE_COLUMNS:
            raise AuthorityError("authority catalog revalidation table schema is not exact")

    def _validate_database_configuration(self) -> None:
        expected = {
            "busy_timeout": 10000,
            "journal_mode": "wal",
            "synchronous": 2,
            "fullfsync": 1,
            "foreign_keys": 1,
            "trusted_schema": 0,
            "temp_store": 2,
        }
        for pragma, required in expected.items():
            row = self.connection.execute(f"PRAGMA {pragma}").fetchone()
            if row != (required,):
                raise AuthorityError(
                    f"authority database PRAGMA {pragma} is not fail-closed"
                )
        if self.connection.execute("PRAGMA quick_check").fetchall() != [("ok",)]:
            raise AuthorityError("authority database integrity check failed")

    def _validate_storage(self) -> None:
        try:
            current = self.directory.lstat()
        except OSError as error:
            raise AuthorityError("authority state directory disappeared") from error
        if _directory_custody_identity(current) != self.directory_identity:
            raise AuthorityError("authority state directory changed during database use")
        _validate_state_ancestors(self.directory)
        _validate_sqlite_artifacts(self.path)

    def insert_challenge(
        self,
        lease: ChallengeLease,
        attestation_nonce: bytes,
        assertion_nonce: bytes,
    ) -> None:
        """Commit a fresh challenge and consumption ID before returning it."""

        _require_sha256(lease.challenge_id, "challenge id")
        _require_sha256(lease.consumption_id, "consumption id")
        _require_sha256(lease.request_sha256, "capture request digest")
        if (
            isinstance(lease.issued_at_unix_ms, bool)
            or not isinstance(lease.issued_at_unix_ms, int)
            or lease.issued_at_unix_ms <= 0
            or isinstance(lease.expires_at_unix_ms, bool)
            or not isinstance(lease.expires_at_unix_ms, int)
            or not 1
            <= lease.expires_at_unix_ms - lease.issued_at_unix_ms
            <= MAX_CHALLENGE_LIFETIME_MS
        ):
            raise AuthorityError("challenge lease times are outside their bound")
        if (
            not isinstance(attestation_nonce, bytes)
            or not isinstance(assertion_nonce, bytes)
            or len(attestation_nonce) != 32
            or len(assertion_nonce) != 32
            or secrets.compare_digest(attestation_nonce, assertion_nonce)
        ):
            raise AuthorityError("challenge nonces must be distinct 32-byte values")
        request_payload = candidate_evidence.canonical_json_bytes(lease.request)
        if hashlib.sha256(request_payload).hexdigest() != lease.request_sha256:
            raise AuthorityError("challenge request digest changed before persistence")
        try:
            self._validate_storage()
            self.connection.execute("BEGIN IMMEDIATE")
            self.connection.execute(
                "DELETE FROM challenges WHERE state = 'issued' AND expires_at_unix_ms < ?",
                (lease.issued_at_unix_ms,),
            )
            outstanding = self.connection.execute(
                "SELECT COUNT(*) FROM challenges WHERE state = 'issued'"
            ).fetchone()
            durable = self.connection.execute(
                "SELECT COUNT(*) FROM challenges WHERE state = 'consumed'"
            ).fetchone()
            if outstanding is None or outstanding[0] >= MAX_OUTSTANDING_CHALLENGES:
                raise AuthorityError("authority has too many outstanding challenges")
            if durable is None or durable[0] >= MAX_DURABLE_CONSUMPTIONS:
                raise AuthorityError(
                    "authority durable-consumption cap requires operator archival"
                )
            self.connection.execute(
                """
                INSERT INTO challenges(
                    challenge_id, consumption_id, issued_at_unix_ms,
                    expires_at_unix_ms, request_sha256, request_json,
                    attestation_nonce, assertion_nonce, state
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'issued')
                """,
                (
                    lease.challenge_id,
                    lease.consumption_id,
                    lease.issued_at_unix_ms,
                    lease.expires_at_unix_ms,
                    lease.request_sha256,
                    request_payload,
                    attestation_nonce,
                    assertion_nonce,
                ),
            )
            self.connection.execute("COMMIT")
            self._validate_storage()
        except sqlite3.Error as error:
            try:
                self.connection.execute("ROLLBACK")
            except sqlite3.Error:
                pass
            raise AuthorityError("one-time challenge could not be persisted") from error
        except AuthorityError:
            try:
                self.connection.execute("ROLLBACK")
            except sqlite3.Error:
                pass
            raise

    def challenge_row(self, challenge_id: str) -> tuple[Any, ...]:
        row = self.connection.execute(
            """
            SELECT challenge_id, consumption_id, issued_at_unix_ms,
                   expires_at_unix_ms, request_sha256, request_json,
                   attestation_nonce, assertion_nonce, state,
                   evidence_sha256, receipt_id, key_id, assertion_counter,
                   receipt_payload, consumed_at_unix_ms
              FROM challenges WHERE challenge_id = ?
            """,
            (challenge_id,),
        ).fetchone()
        if row is None:
            raise AuthorityError("challenge id was not issued by this authority")
        return row

    def commit_consumption(
        self,
        *,
        challenge_id: str,
        validated: ValidatedEvidence,
        apple_receipt: bytes,
        apple_facts: AppleReceiptFacts,
        authority_key_id: str,
        authority_public_key_sha256: str,
        consumed_at_unix_ms: int,
    ) -> tuple[dict[str, Any], bool]:
        """Atomically consume a challenge and advance its App Attest counter.

        The returned Boolean is true only for crash recovery of an already
        committed identical consumption. No signature is produced in this
        transaction; callers sign only after COMMIT succeeds.
        """

        _require_sha256(challenge_id, "challenge id")
        if (
            isinstance(consumed_at_unix_ms, bool)
            or not isinstance(consumed_at_unix_ms, int)
            or consumed_at_unix_ms <= 0
        ):
            raise AuthorityError("consumption time must be positive Unix milliseconds")
        try:
            self._validate_storage()
            self.connection.execute("BEGIN IMMEDIATE")
            row = self.challenge_row(challenge_id)
            (
                _,
                consumption_id,
                issued_at,
                challenge_expires_at,
                request_sha256,
                _,
                attestation_nonce,
                assertion_nonce,
                challenge_state,
                prior_evidence_sha256,
                _,
                _,
                _,
                stored_payload,
                _,
            ) = row
            if challenge_state == "consumed":
                if (
                    prior_evidence_sha256 != validated.evidence_sha256
                    or stored_payload is None
                ):
                    raise AuthorityError("one-time challenge was already consumed")
                stored = candidate_evidence.parse_strict_json(
                    bytes(stored_payload) + b"\n",
                    "persisted freshness receipt payload",
                )
                if (
                    stored.get("signer_key_id") != authority_key_id
                    or stored.get("signer_public_key_sha256")
                    != authority_public_key_sha256
                ):
                    raise AuthorityError(
                        "committed receipt requires its original authority signing key"
                    )
                self.connection.execute("COMMIT")
                self._validate_storage()
                return stored, True
            if consumed_at_unix_ms > challenge_expires_at:
                raise AuthorityError("one-time challenge expired before consumption")
            if request_sha256 != validated.capture_request_sha256:
                raise AuthorityError("evidence does not bind the issued capture request")
            if attestation_nonce != validated.facts.attestation_challenge_nonce:
                raise AuthorityError("evidence substituted the attestation challenge nonce")
            if assertion_nonce != validated.facts.assertion_challenge_nonce:
                raise AuthorityError("evidence substituted the assertion challenge nonce")

            key_row = self.connection.execute(
                """
                SELECT public_key_sha256, assertion_counter
                  FROM app_attest_keys WHERE key_id = ?
                """,
                (validated.facts.key_id,),
            ).fetchone()
            public_key_sha256 = hashlib.sha256(
                validated.assertion_public_key
            ).hexdigest()
            if key_row is None:
                previous_counter = 0
            else:
                stored_public_key_sha256, previous_counter = key_row
                if stored_public_key_sha256 != public_key_sha256:
                    raise AuthorityError("App Attest key id was rebound to another public key")
            if validated.facts.assertion_counter <= previous_counter:
                raise AuthorityError("App Attest assertion counter replay or rollback detected")
            if (
                isinstance(apple_facts.verified_at_unix_ms, bool)
                or not isinstance(apple_facts.verified_at_unix_ms, int)
                or apple_facts.verified_at_unix_ms <= 0
                or apple_facts.verified_at_unix_ms
                > consumed_at_unix_ms + MAX_CLOCK_SKEW_MS
                or consumed_at_unix_ms - apple_facts.verified_at_unix_ms
                > 60_000 + MAX_CLOCK_SKEW_MS
            ):
                raise AuthorityError(
                    "Apple receipt verification time is not current at consumption"
                )

            receipt_id = _random_digest(b"kagemusha-online-freshness-receipt-v1")
            receipt_expires_at = (
                consumed_at_unix_ms
                + production_evidence.MAX_ONLINE_RECEIPT_LIFETIME_MS
            )
            if receipt_expires_at <= consumed_at_unix_ms:
                raise AuthorityError("freshness receipt expiration overflowed")
            receipt = _build_unsigned_receipt(
                validated=validated,
                receipt_id=receipt_id,
                consumption_id=consumption_id,
                issued_at_unix_ms=consumed_at_unix_ms,
                consumed_at_unix_ms=consumed_at_unix_ms,
                expires_at_unix_ms=receipt_expires_at,
                apple_checked_at_unix_ms=apple_facts.verified_at_unix_ms,
                previous_assertion_counter=previous_counter,
                authority_key_id=authority_key_id,
                authority_public_key_sha256=authority_public_key_sha256,
            )
            payload = candidate_evidence.canonical_signature_payload(receipt)

            self.connection.execute(
                """
                INSERT INTO app_attest_keys(
                    key_id, public_key_sha256, assertion_counter,
                    apple_receipt, apple_risk_metric, updated_at_unix_ms
                ) VALUES (?, ?, ?, ?, ?, ?)
                ON CONFLICT(key_id) DO UPDATE SET
                    public_key_sha256 = excluded.public_key_sha256,
                    assertion_counter = excluded.assertion_counter,
                    apple_receipt = excluded.apple_receipt,
                    apple_risk_metric = excluded.apple_risk_metric,
                    updated_at_unix_ms = excluded.updated_at_unix_ms
                """,
                (
                    validated.facts.key_id,
                    public_key_sha256,
                    validated.facts.assertion_counter,
                    apple_receipt,
                    apple_facts.risk_metric,
                    consumed_at_unix_ms,
                ),
            )
            self.connection.execute(
                """
                UPDATE challenges
                   SET state = 'consumed', evidence_sha256 = ?, receipt_id = ?,
                       key_id = ?, assertion_counter = ?, receipt_payload = ?,
                       apple_receipt = ?, apple_risk_metric = ?,
                       consumed_at_unix_ms = ?
                 WHERE challenge_id = ? AND state = 'issued'
                """,
                (
                    validated.evidence_sha256,
                    receipt_id,
                    validated.facts.key_id,
                    validated.facts.assertion_counter,
                    payload,
                    apple_receipt,
                    apple_facts.risk_metric,
                    consumed_at_unix_ms,
                    challenge_id,
                ),
            )
            if self.connection.execute("SELECT changes()").fetchone() != (1,):
                raise AuthorityError("one-time challenge changed during consumption")
            self.connection.execute("COMMIT")
            self._validate_storage()
            return receipt, False
        except (sqlite3.Error, candidate_evidence.EvidenceError) as error:
            try:
                self.connection.execute("ROLLBACK")
            except sqlite3.Error:
                pass
            if isinstance(error, candidate_evidence.EvidenceError):
                raise AuthorityError(str(error)) from error
            raise AuthorityError("freshness consumption could not be committed") from error
        except AuthorityError:
            try:
                self.connection.execute("ROLLBACK")
            except sqlite3.Error:
                pass
            raise

    def commit_catalog_revalidation(
        self,
        *,
        promotion_id: str,
        catalog_sha256: str,
        release_statuses: list[dict[str, Any]],
        authority_key_id: str,
        authority_public_key_sha256: str,
        issued_at_unix_ms: int,
    ) -> tuple[dict[str, Any], bool]:
        """Atomically reserve one promotion id and its exact catalog result.

        A promotion id is single-use. Crash recovery may recover the identical
        still-current unsigned payload, but it can never rebind the id to a
        different catalog or signing key and can never revive an expired run.
        """

        _require_sha256(promotion_id, "promotion id")
        _require_sha256(catalog_sha256, "catalog digest")
        _require_sha256(
            authority_public_key_sha256, "catalog authority public key digest"
        )
        try:
            candidate_evidence._validate_key_id(
                authority_key_id, "catalog authority key id"
            )
        except candidate_evidence.EvidenceError as error:
            raise AuthorityError(str(error)) from error
        if (
            isinstance(issued_at_unix_ms, bool)
            or not isinstance(issued_at_unix_ms, int)
            or issued_at_unix_ms <= 0
        ):
            raise AuthorityError(
                "catalog revalidation time must be positive Unix milliseconds"
            )
        incoming_bindings, incoming_catalog_sha256 = (
            _validate_catalog_revalidation_release_statuses(
                release_statuses,
                issued_at_unix_ms=issued_at_unix_ms,
                label="catalog revalidation result",
                validate_current_status=False,
            )
        )
        if incoming_catalog_sha256 != catalog_sha256:
            raise AuthorityError(
                "catalog revalidation status bindings do not match the catalog digest"
            )
        try:
            self._validate_storage()
            self.connection.execute("BEGIN IMMEDIATE")
            existing = self.connection.execute(
                """
                SELECT promotion_id, catalog_sha256, receipt_id,
                       issued_at_unix_ms, expires_at_unix_ms,
                       authority_key_id, authority_public_key_sha256,
                       receipt_payload, state, retired_at_unix_ms
                  FROM catalog_revalidations WHERE promotion_id = ?
                """,
                (promotion_id,),
            ).fetchone()
            if existing is not None:
                record = _validate_persisted_catalog_revalidation_record(
                    existing,
                    expected_promotion_id=promotion_id,
                    expected_catalog_sha256=catalog_sha256,
                    expected_bindings=incoming_bindings,
                    expected_authority_key_id=authority_key_id,
                    expected_authority_public_key_sha256=(
                        authority_public_key_sha256
                    ),
                    evaluation_time_unix_ms=issued_at_unix_ms,
                )
                if record.state == "expired":
                    raise AuthorityError(
                        "promotion id was already consumed by an expired catalog receipt"
                    )
                if issued_at_unix_ms > record.expires_at_unix_ms:
                    self.connection.execute(
                        """
                        UPDATE catalog_revalidations
                           SET state = 'expired', retired_at_unix_ms = ?
                         WHERE promotion_id = ? AND state = 'active'
                        """,
                        (issued_at_unix_ms, promotion_id),
                    )
                    if self.connection.execute("SELECT changes()").fetchone() != (1,):
                        raise AuthorityError(
                            "catalog revalidation changed during expiry retirement"
                        )
                    self.connection.execute("COMMIT")
                    self._validate_storage()
                    raise AuthorityError(
                        "promotion id was already consumed by an expired catalog receipt"
                    )
                _validate_catalog_revalidation_release_statuses(
                    release_statuses,
                    issued_at_unix_ms=issued_at_unix_ms,
                    label="catalog revalidation result",
                )
                self.connection.execute("COMMIT")
                self._validate_storage()
                return record.receipt, True

            _validate_catalog_revalidation_release_statuses(
                release_statuses,
                issued_at_unix_ms=issued_at_unix_ms,
                label="catalog revalidation result",
            )
            durable = self.connection.execute(
                "SELECT COUNT(*) FROM catalog_revalidations"
            ).fetchone()
            if (
                durable is None
                or durable[0] >= MAX_DURABLE_CATALOG_REVALIDATIONS
            ):
                raise AuthorityError(
                    "authority catalog-revalidation cap requires operator archival"
                )
            receipt_id = _random_digest(
                b"kagemusha-catalog-revalidation-receipt-v1"
            )
            expires_at_unix_ms = (
                issued_at_unix_ms
                + production_evidence.MAX_ONLINE_RECEIPT_LIFETIME_MS
            )
            receipt = _build_unsigned_catalog_revalidation_receipt(
                receipt_id=receipt_id,
                promotion_id=promotion_id,
                catalog_sha256=catalog_sha256,
                issued_at_unix_ms=issued_at_unix_ms,
                expires_at_unix_ms=expires_at_unix_ms,
                release_statuses=release_statuses,
                authority_key_id=authority_key_id,
                authority_public_key_sha256=authority_public_key_sha256,
            )
            payload = candidate_evidence.canonical_signature_payload(receipt)
            record = _validate_persisted_catalog_revalidation_record(
                (
                    promotion_id,
                    catalog_sha256,
                    receipt_id,
                    issued_at_unix_ms,
                    expires_at_unix_ms,
                    authority_key_id,
                    authority_public_key_sha256,
                    payload,
                    "active",
                    None,
                ),
                expected_promotion_id=promotion_id,
                expected_catalog_sha256=catalog_sha256,
                expected_bindings=incoming_bindings,
                expected_authority_key_id=authority_key_id,
                expected_authority_public_key_sha256=authority_public_key_sha256,
                evaluation_time_unix_ms=issued_at_unix_ms,
            )
            self.connection.execute(
                """
                INSERT INTO catalog_revalidations(
                    promotion_id, catalog_sha256, receipt_id,
                    issued_at_unix_ms, expires_at_unix_ms,
                    authority_key_id, authority_public_key_sha256,
                    receipt_payload, state
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'active')
                """,
                (
                    promotion_id,
                    catalog_sha256,
                    receipt_id,
                    issued_at_unix_ms,
                    expires_at_unix_ms,
                    authority_key_id,
                    authority_public_key_sha256,
                    payload,
                ),
            )
            self.connection.execute("COMMIT")
            self._validate_storage()
            return record.receipt, False
        except (sqlite3.Error, candidate_evidence.EvidenceError) as error:
            try:
                self.connection.execute("ROLLBACK")
            except sqlite3.Error:
                pass
            if isinstance(error, candidate_evidence.EvidenceError):
                raise AuthorityError(str(error)) from error
            raise AuthorityError(
                "catalog revalidation could not be committed"
            ) from error
        except AuthorityError:
            try:
                self.connection.execute("ROLLBACK")
            except sqlite3.Error:
                pass
            raise

    def recover_catalog_revalidation(
        self,
        *,
        promotion_id: str,
        catalog_sha256: str,
        bindings: list[dict[str, str]],
        authority_key_id: str,
        authority_public_key_sha256: str,
        evaluation_time_unix_ms: int,
    ) -> Optional[dict[str, Any]]:
        """Recover an identical current receipt after commit-before-signing crash."""

        _require_sha256(promotion_id, "promotion id")
        _require_sha256(catalog_sha256, "catalog digest")
        _require_sha256(
            authority_public_key_sha256, "catalog authority public key digest"
        )
        try:
            candidate_evidence._validate_key_id(
                authority_key_id, "catalog authority key id"
            )
            expected_catalog_sha256 = production_evidence.catalog_revalidation_digest(
                bindings, candidate_evidence
            )
        except (ValueError, candidate_evidence.EvidenceError) as error:
            raise AuthorityError(str(error)) from error
        if expected_catalog_sha256 != catalog_sha256:
            raise AuthorityError(
                "catalog recovery bindings do not match the catalog digest"
            )
        if (
            isinstance(evaluation_time_unix_ms, bool)
            or not isinstance(evaluation_time_unix_ms, int)
            or evaluation_time_unix_ms <= 0
        ):
            raise AuthorityError(
                "catalog recovery time must be positive Unix milliseconds"
            )
        try:
            self._validate_storage()
            self.connection.execute("BEGIN IMMEDIATE")
            row = self.connection.execute(
                """
                SELECT promotion_id, catalog_sha256, receipt_id,
                       issued_at_unix_ms, expires_at_unix_ms,
                       authority_key_id, authority_public_key_sha256,
                       receipt_payload, state, retired_at_unix_ms
                  FROM catalog_revalidations WHERE promotion_id = ?
                """,
                (promotion_id,),
            ).fetchone()
            if row is None:
                self.connection.execute("COMMIT")
                self._validate_storage()
                return None
            record = _validate_persisted_catalog_revalidation_record(
                row,
                expected_promotion_id=promotion_id,
                expected_catalog_sha256=catalog_sha256,
                expected_bindings=bindings,
                expected_authority_key_id=authority_key_id,
                expected_authority_public_key_sha256=(
                    authority_public_key_sha256
                ),
                evaluation_time_unix_ms=evaluation_time_unix_ms,
            )
            if record.state == "expired":
                raise AuthorityError(
                    "promotion id was already consumed by an expired catalog receipt"
                )
            if evaluation_time_unix_ms > record.expires_at_unix_ms:
                self.connection.execute(
                    """
                    UPDATE catalog_revalidations
                       SET state = 'expired', retired_at_unix_ms = ?
                     WHERE promotion_id = ? AND state = 'active'
                    """,
                    (evaluation_time_unix_ms, promotion_id),
                )
                if self.connection.execute("SELECT changes()").fetchone() != (1,):
                    raise AuthorityError(
                        "catalog revalidation changed during expiry retirement"
                    )
                self.connection.execute("COMMIT")
                self._validate_storage()
                raise AuthorityError(
                    "promotion id was already consumed by an expired catalog receipt"
                )
            self.connection.execute("COMMIT")
            self._validate_storage()
            return record.receipt
        except (sqlite3.Error, candidate_evidence.EvidenceError) as error:
            try:
                self.connection.execute("ROLLBACK")
            except sqlite3.Error:
                pass
            if isinstance(error, candidate_evidence.EvidenceError):
                raise AuthorityError(str(error)) from error
            raise AuthorityError(
                "catalog revalidation recovery could not be committed"
            ) from error
        except AuthorityError:
            try:
                self.connection.execute("ROLLBACK")
            except sqlite3.Error:
                pass
            raise


def _validate_policy_snapshot(path: Path) -> tuple[Any, dict[str, Any], str]:
    snapshot = candidate_evidence._snapshot_private_file(
        path.resolve(strict=True),
        "production iOS policy",
        maximum=production_evidence.MAX_POLICY_BYTES,
        retain_payload=True,
    )
    policy = candidate_evidence.parse_strict_json(
        snapshot.payload, "production iOS policy"
    )
    errors: list[str] = []
    if not production_evidence._validate_policy(policy, snapshot.payload, errors):
        raise AuthorityError("production iOS policy is invalid: " + "; ".join(errors))
    return snapshot, policy, snapshot.sha256


def _validate_capture_app_measurements_snapshot(
    path: Path, policy: dict[str, Any]
) -> tuple[Any, dict[str, Any], str]:
    try:
        snapshot = candidate_evidence._snapshot_private_file(
            path.resolve(strict=True),
            "prepared capture-app code-sign measurements",
            maximum=MAX_CAPTURE_APP_MEASUREMENTS_BYTES,
            retain_payload=True,
        )
        value = candidate_evidence.parse_strict_json(
            snapshot.payload, "prepared capture-app code-sign measurements"
        )
    except (OSError, candidate_evidence.EvidenceError) as error:
        raise AuthorityError(str(error)) from error
    errors: list[str] = []
    digest = production_evidence._validate_capture_app_code_sign_measurements(
        value, policy, candidate_evidence, errors
    )
    if digest is None or errors:
        raise AuthorityError(
            "prepared capture-app code-sign measurements are invalid: "
            + "; ".join(errors)
        )
    if snapshot.sha256 != digest:
        raise AuthorityError(
            "prepared capture-app measurement bytes are not the canonical digest input"
        )
    return snapshot, value, digest


def _validate_static_evidence_snapshot(
    *,
    evidence: dict[str, Any],
    policy: dict[str, Any],
    policy_sha256: str,
    raw_snapshot: Any,
    trusted_lab_key_id: str,
    trusted_lab_public_der: bytes,
) -> list[str]:
    """Validate the exact in-memory envelope snapshot used by the authority."""

    errors: list[str] = []
    if (
        production_evidence._exact_fields(
            evidence,
            production_evidence.PRODUCTION_SIGNED_EVIDENCE_FIELDS,
            "signed production evidence",
            errors,
        )
        is None
    ):
        return errors
    if evidence.get("schema") != production_evidence.PRODUCTION_SIGNED_EVIDENCE_SCHEMA:
        errors.append(
            "signed production evidence schema must be "
            f"{production_evidence.PRODUCTION_SIGNED_EVIDENCE_SCHEMA}"
        )
    if evidence.get("version") != 1 or isinstance(evidence.get("version"), bool):
        errors.append("signed production evidence version must be integer 1")
    if evidence.get("production_policy_id") != policy.get("policy_id"):
        errors.append("signed production evidence production_policy_id must match policy")
    if evidence.get("production_policy_sha256") != policy_sha256:
        errors.append(
            "signed production evidence production_policy_sha256 must match exact policy bytes"
        )
    if evidence.get("signer_key_id") != trusted_lab_key_id:
        errors.append("signed production evidence signer_key_id must match trusted key id")
    trusted_digest = hashlib.sha256(trusted_lab_public_der).hexdigest()
    if evidence.get("signer_public_key_sha256") != trusted_digest:
        errors.append("signed production evidence public key digest must match trusted key")
    if evidence.get("signature_algorithm") != "ed25519":
        errors.append("signed production evidence signature_algorithm must be ed25519")

    expected_artifacts = {
        relative: {
            "size_bytes": raw_snapshot.sizes[relative],
            "sha256": digest,
        }
        for relative, digest in raw_snapshot.digests.items()
    }
    if evidence.get("artifact_digests") != expected_artifacts:
        errors.append(
            "signed production evidence artifact_digests must equal the exact raw tree"
        )
    errors.extend(candidate_evidence.validate_raw_evidence(raw_snapshot))

    try:
        signature_payload = candidate_evidence.canonical_signature_payload(evidence)
    except candidate_evidence.EvidenceError as error:
        errors.append(str(error))
        return errors
    if evidence.get("signature_payload_sha256") != hashlib.sha256(
        signature_payload
    ).hexdigest():
        errors.append("signed production evidence signature_payload_sha256 mismatch")
    signature_text = evidence.get("signature")
    if not isinstance(signature_text, str) or re.fullmatch(
        r"[0-9a-f]{128}", signature_text
    ) is None:
        errors.append("signed production evidence signature must be 64 lowercase hex bytes")
    else:
        try:
            candidate_evidence._verify_ed25519_bytes(
                trusted_lab_public_der[len(candidate_evidence.ED25519_SPKI_PREFIX) :],
                signature_payload,
                bytes.fromhex(signature_text),
            )
        except candidate_evidence.EvidenceError as error:
            errors.append(str(error))
    return errors


def issue_challenge(
    state: AuthorityState,
    *,
    artifact_root: Path,
    production_policy_path: Path,
    capture_app_code_sign_measurements_path: Path,
    release_manifest_sha256: str,
    now_unix_ms: Optional[int] = None,
    lifetime_ms: int = MAX_CHALLENGE_LIFETIME_MS,
    nonce_source: Callable[[int], bytes] = secrets.token_bytes,
) -> ChallengeLease:
    """Create and durably reserve an exact artifact-bound capture request."""

    release_manifest_sha256 = _require_sha256(
        release_manifest_sha256, "release manifest digest"
    )
    if not 1 <= lifetime_ms <= MAX_CHALLENGE_LIFETIME_MS:
        raise AuthorityError("challenge lifetime must be between 1 ms and five minutes")
    issued_at = _now_ms() if now_unix_ms is None else now_unix_ms
    if isinstance(issued_at, bool) or not isinstance(issued_at, int) or issued_at <= 0:
        raise AuthorityError("challenge issuance time must be positive Unix milliseconds")
    policy_snapshot, policy, policy_sha256 = _validate_policy_snapshot(
        production_policy_path
    )
    measurement_snapshot, _, measurement_sha256 = (
        _validate_capture_app_measurements_snapshot(
            capture_app_code_sign_measurements_path, policy
        )
    )
    raw_snapshot = candidate_evidence.snapshot_raw_artifacts(
        artifact_root.resolve(strict=True)
    )
    raw_errors = candidate_evidence.validate_raw_evidence(raw_snapshot)
    if raw_errors:
        raise AuthorityError("raw production evidence is invalid: " + "; ".join(raw_errors))
    artifact_digests = {
        relative: {
            "size_bytes": raw_snapshot.sizes[relative],
            "sha256": digest,
        }
        for relative, digest in raw_snapshot.digests.items()
    }
    attestation_nonce = nonce_source(32)
    assertion_nonce = nonce_source(32)
    if (
        len(attestation_nonce) != 32
        or len(assertion_nonce) != 32
        or attestation_nonce == assertion_nonce
    ):
        raise AuthorityError("challenge nonce source did not return distinct 32-byte values")
    attestation = production_evidence._challenge_bindings(
        artifact_digests,
        schema=production_evidence.ATTESTATION_CHALLENGE_SCHEMA,
        domain=production_evidence.ATTESTATION_CHALLENGE_DOMAIN,
        policy_id=policy["policy_id"],
        policy_sha256=policy_sha256,
        release_manifest_sha256=release_manifest_sha256,
        capture_app_code_sign_measurements_sha256=measurement_sha256,
        evaluated_at_unix_ms=issued_at,
        nonce_base64=base64.b64encode(attestation_nonce).decode("ascii"),
    )
    assertion = production_evidence._challenge_bindings(
        artifact_digests,
        schema=production_evidence.ASSERTION_CHALLENGE_SCHEMA,
        domain=production_evidence.ASSERTION_CHALLENGE_DOMAIN,
        policy_id=policy["policy_id"],
        policy_sha256=policy_sha256,
        release_manifest_sha256=release_manifest_sha256,
        capture_app_code_sign_measurements_sha256=measurement_sha256,
        evaluated_at_unix_ms=issued_at,
        nonce_base64=base64.b64encode(assertion_nonce).decode("ascii"),
    )
    request = {
        "schema": "iroha.kagemusha.ios.app_attest_capture_request.v1",
        "version": 1,
        "attestation_client_data_base64": base64.b64encode(
            candidate_evidence.canonical_json_bytes(attestation)
        ).decode("ascii"),
        "assertion_client_data_template": assertion,
    }
    request_sha256 = hashlib.sha256(
        candidate_evidence.canonical_json_bytes(request)
    ).hexdigest()
    lease = ChallengeLease(
        challenge_id=_random_digest(b"kagemusha-app-attest-challenge-v1"),
        consumption_id=_random_digest(b"kagemusha-app-attest-consumption-v1"),
        issued_at_unix_ms=issued_at,
        expires_at_unix_ms=issued_at + lifetime_ms,
        request_sha256=request_sha256,
        request=request,
    )
    state.insert_challenge(lease, attestation_nonce, assertion_nonce)
    candidate_evidence._require_raw_snapshot_unchanged(raw_snapshot)
    candidate_evidence._require_private_file_snapshot_unchanged(
        policy_snapshot,
        "production iOS policy",
        maximum=production_evidence.MAX_POLICY_BYTES,
    )
    candidate_evidence._require_private_file_snapshot_unchanged(
        measurement_snapshot,
        "prepared capture-app code-sign measurements",
        maximum=MAX_CAPTURE_APP_MEASUREMENTS_BYTES,
    )
    return lease


def _load_validated_evidence(
    *,
    evidence_path: Path,
    artifact_root: Path,
    production_policy_path: Path,
    capture_app_code_sign_measurements_path: Path,
    trusted_lab_key_id: str,
    trusted_lab_public_key_path: Path,
) -> ValidatedEvidence:
    """Validate one exact evidence snapshot and return its online facts."""

    try:
        root_absolute = artifact_root.resolve(strict=True)
        evidence_absolute = evidence_path.resolve(strict=True)
        policy_absolute = production_policy_path.resolve(strict=True)
        measurement_absolute = capture_app_code_sign_measurements_path.resolve(
            strict=True
        )
        lab_key_absolute = trusted_lab_public_key_path.resolve(strict=True)
        for path, label in (
            (evidence_absolute, "signed production evidence"),
            (policy_absolute, "production iOS policy"),
            (measurement_absolute, "prepared capture-app code-sign measurements"),
            (lab_key_absolute, "lab signer public key"),
        ):
            try:
                path.relative_to(root_absolute)
            except ValueError:
                pass
            else:
                raise AuthorityError(f"{label} must stay outside the artifact root")
        candidate_evidence._validate_key_id(
            trusted_lab_key_id, "trusted lab signer key id"
        )
        evidence_snapshot = candidate_evidence._snapshot_private_file(
            evidence_absolute,
            "signed production evidence",
            maximum=candidate_evidence.MAX_JSON_BYTES,
            retain_payload=True,
        )
        policy_snapshot = candidate_evidence._snapshot_private_file(
            policy_absolute,
            "production iOS policy",
            maximum=production_evidence.MAX_POLICY_BYTES,
            retain_payload=True,
        )
        lab_key_snapshot = candidate_evidence._snapshot_key_file(
            lab_key_absolute, "lab signer public key", private=False
        )
        raw_snapshot = candidate_evidence.snapshot_raw_artifacts(root_absolute)
        evidence = candidate_evidence.parse_strict_json(
            evidence_snapshot.payload, "signed production evidence"
        )
        policy = candidate_evidence.parse_strict_json(
            policy_snapshot.payload, "production iOS policy"
        )
        lab_public_der = candidate_evidence._public_key_der_from_payload(
            lab_key_snapshot.payload
        )
    except (OSError, candidate_evidence.EvidenceError) as error:
        raise AuthorityError(str(error)) from error

    direct_errors: list[str] = []
    if not production_evidence._validate_policy(
        policy, policy_snapshot.payload, direct_errors
    ):
        raise AuthorityError(
            "production iOS policy is invalid: " + "; ".join(direct_errors)
        )
    static_errors = _validate_static_evidence_snapshot(
        evidence=evidence,
        policy=policy,
        policy_sha256=policy_snapshot.sha256,
        raw_snapshot=raw_snapshot,
        trusted_lab_key_id=trusted_lab_key_id,
        trusted_lab_public_der=lab_public_der,
    )
    if static_errors:
        raise AuthorityError(
            "production evidence failed static validation: "
            + "; ".join(static_errors)
        )
    direct_errors = []
    measurement_snapshot, measurement_value, measurement_sha256 = (
        _validate_capture_app_measurements_snapshot(
            measurement_absolute, policy
        )
    )
    release_manifest_sha256 = evidence.get("release_manifest_sha256")
    if not isinstance(release_manifest_sha256, str):
        raise AuthorityError("production evidence omitted release manifest digest")
    _require_sha256(release_manifest_sha256, "release manifest digest")
    artifact_digests = evidence.get("artifact_digests")
    if not isinstance(artifact_digests, dict):
        raise AuthorityError("production evidence artifact digests are not an object")
    facts = production_evidence._validate_platform_evidence(
        evidence.get("platform_evidence"),
        policy,
        policy_snapshot.sha256,
        release_manifest_sha256,
        artifact_digests,
        raw_snapshot,
        candidate_evidence,
        direct_errors,
    )
    if facts is None or direct_errors:
        raise AuthorityError(
            "production platform evidence is invalid: " + "; ".join(direct_errors)
        )
    platform_value = evidence.get("platform_evidence")
    if (
        not isinstance(platform_value, dict)
        or platform_value.get("capture_app_code_sign_measurements")
        != measurement_value
    ):
        raise AuthorityError(
            "signed evidence does not embed the exact prepared capture-app measurement"
        )
    if (
        hashlib.sha256(
            candidate_evidence.canonical_json_bytes(measurement_value)
        ).hexdigest()
        != measurement_sha256
    ):
        raise AuthorityError("prepared capture-app measurement digest changed")

    try:
        candidate_evidence._require_private_file_snapshot_unchanged(
            evidence_snapshot,
            "signed production evidence",
            maximum=candidate_evidence.MAX_JSON_BYTES,
        )
        candidate_evidence._require_private_file_snapshot_unchanged(
            policy_snapshot,
            "production iOS policy",
            maximum=production_evidence.MAX_POLICY_BYTES,
        )
        candidate_evidence._require_private_file_snapshot_unchanged(
            measurement_snapshot,
            "prepared capture-app code-sign measurements",
            maximum=MAX_CAPTURE_APP_MEASUREMENTS_BYTES,
        )
        candidate_evidence._require_key_snapshot_unchanged(
            lab_key_snapshot, "lab signer public key", private=False
        )
        candidate_evidence._require_raw_snapshot_unchanged(raw_snapshot)
    except candidate_evidence.EvidenceError as error:
        raise AuthorityError(str(error)) from error

    platform = evidence.get("platform_evidence")
    if not isinstance(platform, dict):
        raise AuthorityError("production platform evidence is not an object")
    try:
        attestation_client_data = base64.b64decode(
            platform["attestation_client_data_base64"], validate=True
        )
        assertion_client_data = base64.b64decode(
            platform["assertion_client_data_base64"], validate=True
        )
        attestation_object = base64.b64decode(
            platform["attestation_object_base64"], validate=True
        )
        assertion_public_key = base64.b64decode(
            platform["assertion_public_key_sec1_base64"], validate=True
        )
        attestation_challenge = candidate_evidence.parse_strict_json(
            attestation_client_data, "attestation challenge"
        )
        assertion_challenge = candidate_evidence.parse_strict_json(
            assertion_client_data, "assertion challenge"
        )
        attestation = production_evidence._cbor_object(
            production_evidence._decode_cbor(
                attestation_object, "App Attest attestation object"
            ),
            {"fmt", "attStmt", "authData"},
            "App Attest attestation object",
        )
        statement = production_evidence._cbor_object(
            attestation["attStmt"], {"x5c", "receipt"}, "App Attest attStmt"
        )
    except (KeyError, ValueError, candidate_evidence.EvidenceError) as error:
        raise AuthorityError("validated App Attest evidence could not be reloaded") from error
    embedded_apple_receipt = statement.get("receipt")
    if (
        not isinstance(embedded_apple_receipt, bytes)
        or not 1 <= len(embedded_apple_receipt) <= production_evidence.MAX_RECEIPT_BYTES
    ):
        raise AuthorityError("App Attest attStmt receipt is missing or oversized")
    assertion_template = dict(assertion_challenge)
    if assertion_template.pop("attestation_object_sha256", None) is None:
        raise AuthorityError("assertion challenge omitted attestation object binding")
    if assertion_template.pop("key_id", None) is None:
        raise AuthorityError("assertion challenge omitted App Attest key binding")
    capture_request = {
        "schema": "iroha.kagemusha.ios.app_attest_capture_request.v1",
        "version": 1,
        "attestation_client_data_base64": base64.b64encode(
            attestation_client_data
        ).decode("ascii"),
        "assertion_client_data_template": assertion_template,
    }
    capture_request_sha256 = hashlib.sha256(
        candidate_evidence.canonical_json_bytes(capture_request)
    ).hexdigest()
    app_id = f"{policy['app_id_prefix']}.{policy['bundle_id']}"
    return ValidatedEvidence(
        evidence=evidence,
        policy=policy,
        facts=facts,
        evidence_sha256=evidence_snapshot.sha256,
        policy_sha256=policy_snapshot.sha256,
        release_manifest_sha256=release_manifest_sha256,
        lab_signer_key_id=trusted_lab_key_id,
        lab_signer_public_key_sha256=hashlib.sha256(lab_public_der).hexdigest(),
        capture_request=capture_request,
        capture_request_sha256=capture_request_sha256,
        embedded_apple_receipt=embedded_apple_receipt,
        app_id=app_id,
        assertion_public_key=assertion_public_key,
    )


def _snapshot_public_file(path: Path, label: str, maximum: int) -> bytes:
    """Read a bounded public trust input without following filesystem aliases."""

    try:
        before = path.lstat()
    except OSError as error:
        raise AuthorityError(f"{label} metadata could not be read") from error
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_size <= 0
        or before.st_size > maximum
        or stat.S_IMODE(before.st_mode) & 0o022
    ):
        raise AuthorityError(
            f"{label} must be a bounded singly linked non-writable regular file"
        )
    flags = os.O_RDONLY
    if hasattr(os, "O_CLOEXEC"):
        flags |= os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise AuthorityError(f"{label} could not be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        if candidate_evidence._metadata_identity(
            opened
        ) != candidate_evidence._metadata_identity(before):
            raise AuthorityError(f"{label} changed while opening")
        payload = bytearray()
        while len(payload) <= maximum:
            chunk = os.read(descriptor, min(65536, maximum + 1 - len(payload)))
            if not chunk:
                break
            payload.extend(chunk)
        after_fd = os.fstat(descriptor)
        after_path = path.lstat()
        if (
            len(payload) > maximum
            or candidate_evidence._metadata_identity(after_fd)
            != candidate_evidence._metadata_identity(before)
            or candidate_evidence._metadata_identity(after_path)
            != candidate_evidence._metadata_identity(before)
        ):
            raise AuthorityError(f"{label} changed while reading")
        return bytes(payload)
    finally:
        os.close(descriptor)


def _pem_certificate_der(payload: bytes, label: str) -> bytes:
    if not payload.endswith(b"\n"):
        raise AuthorityError(f"{label} is not one canonical PEM certificate")
    lines = payload.splitlines()
    if (
        len(lines) < 3
        or lines[0] != b"-----BEGIN CERTIFICATE-----"
        or lines[-1] != b"-----END CERTIFICATE-----"
    ):
        raise AuthorityError(f"{label} is not one canonical PEM certificate")
    body = lines[1:-1]
    if any(
        not 1 <= len(line) <= 64
        or re.fullmatch(rb"[A-Za-z0-9+/]+={0,2}", line) is None
        or (index != len(body) - 1 and len(line) != 64)
        or (index != len(body) - 1 and b"=" in line)
        for index, line in enumerate(body)
    ):
        raise AuthorityError(f"{label} is not one canonical PEM certificate")
    encoded = b"".join(body)
    try:
        der = base64.b64decode(encoded, validate=True)
    except ValueError as error:
        raise AuthorityError(f"{label} has invalid PEM base64") from error
    if base64.b64encode(der) != encoded:
        raise AuthorityError(f"{label} PEM base64 is not canonical")
    return der


def _validate_official_apple_root(path: Path) -> bytes:
    payload = _snapshot_public_file(path, "Apple receipt root", 64 * 1024)
    der = _pem_certificate_der(payload, "Apple receipt root")
    if hashlib.sha256(der).hexdigest() != APPLE_ROOT_CA_G3_DER_SHA256:
        raise AuthorityError("Apple receipt root is not the pinned Apple Root CA G3")
    return payload


def _trusted_openssl(path: Path) -> Path:
    try:
        metadata = path.lstat()
    except OSError as error:
        raise AuthorityError("OpenSSL executable metadata could not be read") from error
    if (
        stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != 0
        or stat.S_IMODE(metadata.st_mode) & 0o022
        or not os.access(path, os.X_OK)
    ):
        raise AuthorityError(
            "OpenSSL executable must be an absolute root-owned non-writable regular file"
        )
    if not path.is_absolute():
        raise AuthorityError("OpenSSL executable path must be absolute")
    return path


def _verify_cms_signature(
    receipt_der: bytes,
    *,
    root_pem_path: Path,
    openssl_path: Path,
    timeout_seconds: float,
    evaluation_time_unix_ms: Optional[int] = None,
    expected_root_sha256: str = APPLE_ROOT_CA_G3_DER_SHA256,
    enforce_trusted_executable: bool = True,
) -> bytes:
    """Verify CMS signature/chain and return its bounded signed content."""

    if not 1 <= len(receipt_der) <= MAX_APPLE_RESPONSE_BYTES:
        raise AuthorityError("Apple CMS receipt size is outside its bound")
    root_payload = _snapshot_public_file(root_pem_path, "Apple receipt root", 64 * 1024)
    root_der = _pem_certificate_der(root_payload, "Apple receipt root")
    if hashlib.sha256(root_der).hexdigest() != expected_root_sha256:
        raise AuthorityError("Apple receipt root digest does not match its pin")
    executable = _trusted_openssl(openssl_path) if enforce_trusted_executable else openssl_path
    if not timeout_seconds > 0:
        raise AuthorityError("CMS verification timeout must be positive")
    if (
        evaluation_time_unix_ms is not None
        and (
            isinstance(evaluation_time_unix_ms, bool)
            or not isinstance(evaluation_time_unix_ms, int)
            or evaluation_time_unix_ms <= 0
        )
    ):
        raise AuthorityError("CMS certificate evaluation time must be positive")
    try:
        version = subprocess.run(
            [str(executable), "version"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=min(2.0, timeout_seconds),
            check=False,
            env={"PATH": "/usr/bin:/bin", "LANG": "C", "LC_ALL": "C"},
        )
        if version.returncode != 0:
            raise AuthorityError("OpenSSL version could not be authenticated")
        is_libressl = version.stdout.startswith(b"LibreSSL ")
        with tempfile.TemporaryDirectory(
            prefix="kagemusha-app-attest-cms."
        ) as temporary:
            temporary_root = Path(temporary)
            temporary_root.chmod(0o700)
            root_copy = temporary_root / "pinned-root.pem"
            descriptor = os.open(
                root_copy,
                os.O_WRONLY | os.O_CREAT | os.O_EXCL,
                0o600,
            )
            try:
                offset = 0
                while offset < len(root_payload):
                    offset += os.write(descriptor, root_payload[offset:])
                os.fsync(descriptor)
            finally:
                os.close(descriptor)
            empty_ca_path = temporary_root / "empty-ca-path"
            empty_ca_path.mkdir(mode=0o700)
            receipt_copy = temporary_root / "receipt.der"
            receipt_descriptor = os.open(
                receipt_copy,
                os.O_WRONLY | os.O_CREAT | os.O_EXCL,
                0o600,
            )
            try:
                offset = 0
                while offset < len(receipt_der):
                    offset += os.write(receipt_descriptor, receipt_der[offset:])
                os.fsync(receipt_descriptor)
            finally:
                os.close(receipt_descriptor)
            command = [
                str(executable),
                "cms",
                "-verify",
                "-binary",
                "-inform",
                "DER",
                "-in",
                str(receipt_copy),
                "-CAfile",
                str(root_copy),
                "-CApath",
                str(empty_ca_path),
                "-purpose",
                "any",
            ]
            if not is_libressl:
                command.extend(["-verify_retcode", "-no-CAstore"])
            if evaluation_time_unix_ms is not None:
                # Use the same explicit instant as the freshness checks below.
                # This avoids a wall-clock race and keeps archived primary
                # vectors verifiable after their signer certificate expires.
                command.extend(["-attime", str(evaluation_time_unix_ms // 1000)])
            completed = subprocess.run(
                command,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=timeout_seconds,
                check=False,
                env={
                    "PATH": "/usr/bin:/bin",
                    "LANG": "C",
                    "LC_ALL": "C",
                    "SSL_CERT_FILE": str(root_copy),
                    "SSL_CERT_DIR": str(empty_ca_path),
                },
            )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise AuthorityError("Apple CMS verification did not complete safely") from error
    if completed.returncode != 0:
        raise AuthorityError("Apple CMS signature or pinned certificate chain is invalid")
    if not 1 <= len(completed.stdout) <= MAX_CMS_CONTENT_BYTES:
        raise AuthorityError("Apple CMS signed content size is outside its bound")
    return completed.stdout


def _receipt_attributes(payload: bytes) -> dict[int, bytes]:
    try:
        outer = production_evidence._der_single(
            payload, 0x31, "Apple receipt ASN.1 payload"
        )
        reader = production_evidence._DerReader(
            outer.content, "Apple receipt attributes"
        )
        attributes: dict[int, bytes] = {}
        previous_field: Optional[int] = None
        while reader.peek_tag() is not None:
            if len(attributes) >= MAX_RECEIPT_ATTRIBUTES:
                raise ValueError("Apple receipt exceeds its attribute-count bound")
            sequence = reader.element(0x30)
            item = production_evidence._DerReader(
                sequence.content, "Apple receipt attribute"
            )
            field = production_evidence._der_positive_integer(
                item.element(0x02).content,
                "Apple receipt field number",
                allow_zero=False,
            )
            version = production_evidence._der_positive_integer(
                item.element(0x02).content,
                f"Apple receipt field {field} version",
                allow_zero=False,
            )
            if version != 1:
                raise ValueError(f"Apple receipt field {field} version is unsupported")
            value = item.element(0x04).content
            item.finish()
            if field in attributes:
                raise ValueError(f"Apple receipt field {field} is duplicated")
            # Apple's signed App Attest receipt payload uses ascending numeric
            # field identifiers.  It is encoded with a SET tag, but the
            # sequences are not sorted by their full DER encodings; rejecting
            # that primary wire format makes every genuine receipt unusable.
            if previous_field is not None and field <= previous_field:
                raise ValueError(
                    "Apple receipt field identifiers must be strictly increasing"
                )
            attributes[field] = value
            previous_field = field
        reader.finish()
        return attributes
    except ValueError as error:
        raise AuthorityError(str(error)) from error


def _receipt_string(value: bytes, label: str) -> str:
    """Decode one raw textual App Attest receipt attribute value.

    The attribute wrapper already carries an OCTET STRING.  Apple's receipt
    contract places the text bytes directly in that string; an additional
    UTF8String/IA5String wrapper is not present and is rejected as ambiguous.
    """

    if not 1 <= len(value) <= MAX_RECEIPT_TEXT_BYTES:
        raise AuthorityError(f"{label} text size is outside its bound")
    try:
        text = value.decode("utf-8")
    except UnicodeDecodeError as error:
        raise AuthorityError(f"{label} is not valid text") from error
    if not text or any(ord(character) < 0x20 or ord(character) > 0x7E for character in text):
        raise AuthorityError(f"{label} contains non-canonical text")
    return text


def _receipt_time(value: bytes, label: str) -> int:
    text = _receipt_string(value, label)
    match = RFC3339_MILLISECONDS_RE.fullmatch(text)
    if match is None:
        raise AuthorityError(f"{label} must be canonical UTC RFC3339 milliseconds")
    year, month, day, hour, minute, second = (
        int(component) for component in match.groups()[:6]
    )
    milliseconds = int(match.group(7) or "0")
    try:
        parsed = datetime(
            year,
            month,
            day,
            hour,
            minute,
            second,
            milliseconds * 1000,
            tzinfo=timezone.utc,
        )
    except ValueError as error:
        raise AuthorityError(f"{label} contains an invalid calendar time") from error
    return int(parsed.timestamp() * 1000)


def _p256_spki_public_key(payload: bytes) -> bytes:
    try:
        sequence = production_evidence._der_single(
            payload, 0x30, "Apple receipt attested public-key SPKI"
        )
        reader = production_evidence._DerReader(
            sequence.content, "Apple receipt attested public-key SPKI"
        )
        algorithm = reader.element(0x30)
        oid, curve = production_evidence._der_algorithm_identifier(
            algorithm.encoded, "Apple receipt attested public-key algorithm"
        )
        bit_string = reader.element(0x03).content
        reader.finish()
        if (
            oid != production_evidence.OID_EC_PUBLIC_KEY
            or curve != production_evidence.OID_PRIME256V1
            or len(bit_string) != production_evidence.P256_PUBLIC_KEY_BYTES + 1
            or bit_string[0] != 0
        ):
            raise ValueError("Apple receipt attested public key is not P-256 SPKI")
        public_key = bit_string[1:]
        production_evidence._parse_p256_public_key(public_key)
        return public_key
    except ValueError as error:
        raise AuthorityError(str(error)) from error


def _attested_public_key(value: bytes) -> bytes:
    """Extract P-256 from Apple's raw certificate/SPKI/SEC1 field value."""

    if not 1 <= len(value) <= production_evidence.MAX_CERTIFICATE_BYTES:
        raise AuthorityError("Apple receipt attested public key size is outside its bound")
    if len(value) == production_evidence.P256_PUBLIC_KEY_BYTES:
        try:
            production_evidence._parse_p256_public_key(value)
        except ValueError as error:
            raise AuthorityError(
                "Apple receipt attested raw public key is not P-256"
            ) from error
        return value
    try:
        certificate = production_evidence._parse_x509_certificate(
            value, "Apple receipt attested public-key certificate"
        )
    except ValueError:
        try:
            return _p256_spki_public_key(value)
        except AuthorityError as spki_error:
            raise AuthorityError(
                "Apple receipt attested public key must be raw P-256 certificate, "
                "SPKI, or uncompressed SEC1 bytes"
            ) from spki_error
    if certificate.public_key_curve is not production_evidence.P256_CURVE:
        raise AuthorityError("Apple receipt attested certificate is not P-256")
    return certificate.public_key


def verify_apple_receipt(
    receipt_der: bytes,
    *,
    expected_app_id: str,
    expected_public_key: bytes,
    maximum_risk_metric: int,
    evaluation_time_unix_ms: int,
    root_pem_path: Path = APPLE_RECEIPT_ROOT,
    openssl_path: Path = Path("/usr/bin/openssl"),
    cms_timeout_seconds: float = 10.0,
    expected_root_sha256: str = APPLE_ROOT_CA_G3_DER_SHA256,
    enforce_trusted_executable: bool = True,
) -> AppleReceiptFacts:
    """Verify and interpret a production App Attest server receipt."""

    if (
        isinstance(maximum_risk_metric, bool)
        or not isinstance(maximum_risk_metric, int)
        or not 0 <= maximum_risk_metric <= 1_000_000
    ):
        raise AuthorityError("maximum App Attest risk metric is outside its bound")
    if (
        isinstance(evaluation_time_unix_ms, bool)
        or not isinstance(evaluation_time_unix_ms, int)
        or evaluation_time_unix_ms <= 0
    ):
        raise AuthorityError("Apple receipt evaluation time must be positive")
    content = _verify_cms_signature(
        receipt_der,
        root_pem_path=root_pem_path,
        openssl_path=openssl_path,
        timeout_seconds=cms_timeout_seconds,
        evaluation_time_unix_ms=evaluation_time_unix_ms,
        expected_root_sha256=expected_root_sha256,
        enforce_trusted_executable=enforce_trusted_executable,
    )
    attributes = _receipt_attributes(content)
    common_required = {2, 3, 6, 12, 21}
    missing = sorted(common_required - set(attributes))
    if missing:
        raise AuthorityError(f"Apple receipt is missing required fields: {missing}")
    app_id = _receipt_string(attributes[2], "Apple receipt App ID")
    if app_id != expected_app_id:
        raise AuthorityError("Apple receipt App ID does not match production policy")
    public_key = _attested_public_key(attributes[3])
    if not secrets.compare_digest(public_key, expected_public_key):
        raise AuthorityError("Apple receipt public key does not match App Attest evidence")
    receipt_type = _receipt_string(attributes[6], "Apple receipt type")
    if receipt_type != APPLE_RECEIPT_TYPE:
        raise AuthorityError("Apple server response is not a refreshed RECEIPT")
    refreshed_required = {17, 19}
    missing = sorted(refreshed_required - set(attributes))
    if missing:
        raise AuthorityError(
            f"Apple refreshed receipt is missing required fields: {missing}"
        )
    creation = _receipt_time(attributes[12], "Apple receipt creation time")
    not_before = _receipt_time(attributes[19], "Apple receipt not-before time")
    expiration = _receipt_time(attributes[21], "Apple receipt expiration time")
    risk_text = _receipt_string(attributes[17], "Apple receipt risk metric")
    if re.fullmatch(r"(?:0|[1-9][0-9]{0,6})", risk_text) is None:
        raise AuthorityError("Apple receipt risk metric is not a bounded decimal string")
    risk_metric = int(risk_text)
    if risk_metric > maximum_risk_metric:
        raise AuthorityError("Apple receipt risk metric exceeds production policy")
    if creation > evaluation_time_unix_ms + MAX_CLOCK_SKEW_MS:
        raise AuthorityError("Apple receipt creation time is in the future")
    if evaluation_time_unix_ms - creation > MAX_RECEIPT_CREATION_AGE_MS:
        raise AuthorityError("Apple receipt creation time is older than five minutes")
    if not creation < not_before < expiration:
        raise AuthorityError("Apple receipt creation/not-before/expiration order is invalid")
    if expiration <= evaluation_time_unix_ms:
        raise AuthorityError("Apple receipt is expired")
    return AppleReceiptFacts(
        creation_time_unix_ms=creation,
        not_before_unix_ms=not_before,
        expiration_time_unix_ms=expiration,
        risk_metric=risk_metric,
        verified_at_unix_ms=evaluation_time_unix_ms,
    )


def _decode_base64url_json(payload: bytes, label: str) -> dict[str, Any]:
    if JWT_PART_RE.fullmatch(payload) is None:
        raise AuthorityError(f"DeviceCheck JWT {label} is not base64url")
    padding = b"=" * ((4 - len(payload) % 4) % 4)
    try:
        decoded = base64.urlsafe_b64decode(payload + padding)
        if base64.urlsafe_b64encode(decoded).rstrip(b"=") != payload:
            raise ValueError("noncanonical base64url")
        value = json.loads(
            decoded.decode("utf-8"),
            object_pairs_hook=candidate_evidence._pairs_to_object,
            parse_constant=candidate_evidence._reject_constant,
        )
    except (
        ValueError,
        UnicodeDecodeError,
        json.JSONDecodeError,
        RecursionError,
        candidate_evidence.EvidenceError,
    ) as error:
        raise AuthorityError(f"DeviceCheck JWT {label} is invalid") from error
    if not isinstance(value, dict):
        raise AuthorityError(f"DeviceCheck JWT {label} must be an object")
    return value


def validate_devicecheck_jwt(
    payload: bytes, *, expected_issuer: str, evaluation_time_unix_ms: int
) -> str:
    """Validate bounded public JWT metadata before sending it to Apple."""

    token = payload.strip()
    if (
        not 1 <= len(token) <= MAX_JWT_BYTES
        or any(byte <= 0x20 or byte >= 0x7F for byte in token)
    ):
        raise AuthorityError("DeviceCheck JWT is empty, oversized, or non-ASCII")
    parts = token.split(b".")
    if len(parts) != 3 or not all(parts):
        raise AuthorityError("DeviceCheck JWT must contain exactly three segments")
    header = _decode_base64url_json(parts[0], "header")
    claims = _decode_base64url_json(parts[1], "claims")
    if header.get("alg") != "ES256":
        raise AuthorityError("DeviceCheck JWT algorithm must be ES256")
    key_id = header.get("kid")
    if not isinstance(key_id, str) or re.fullmatch(r"[A-Za-z0-9_-]{1,128}", key_id) is None:
        raise AuthorityError("DeviceCheck JWT key id is invalid")
    if claims.get("iss") != expected_issuer:
        raise AuthorityError("DeviceCheck JWT issuer does not match App ID prefix")
    issued_at = claims.get("iat")
    if isinstance(issued_at, bool) or not isinstance(issued_at, int) or issued_at <= 0:
        raise AuthorityError("DeviceCheck JWT issued-at claim is invalid")
    issued_at_ms = issued_at * 1000
    if issued_at_ms > evaluation_time_unix_ms + MAX_CLOCK_SKEW_MS:
        raise AuthorityError("DeviceCheck JWT issued-at claim is in the future")
    if evaluation_time_unix_ms - issued_at_ms > 60 * 60 * 1000:
        raise AuthorityError("DeviceCheck JWT is older than one hour")
    if JWT_PART_RE.fullmatch(parts[2]) is None:
        raise AuthorityError("DeviceCheck JWT signature is not base64url")
    padding = b"=" * ((4 - len(parts[2]) % 4) % 4)
    try:
        signature = base64.urlsafe_b64decode(parts[2] + padding)
    except ValueError as error:
        raise AuthorityError("DeviceCheck JWT signature is invalid") from error
    if (
        len(signature) != 64
        or base64.urlsafe_b64encode(signature).rstrip(b"=") != parts[2]
    ):
        raise AuthorityError("DeviceCheck JWT signature must be canonical ES256")
    try:
        return token.decode("ascii")
    except UnicodeDecodeError as error:
        raise AuthorityError("DeviceCheck JWT is not ASCII") from error


def read_devicecheck_jwt(
    *, file_path: Optional[Path], descriptor: Optional[int]
) -> bytes:
    """Read a JWT from exactly one runtime-only private input."""

    if (file_path is None) == (descriptor is None):
        raise AuthorityError("provide exactly one DeviceCheck JWT file or file descriptor")
    if file_path is not None:
        try:
            return candidate_evidence.read_private_file(
                file_path, "DeviceCheck JWT", maximum=MAX_JWT_BYTES
            )
        except candidate_evidence.EvidenceError as error:
            raise AuthorityError(str(error)) from error
    assert descriptor is not None
    if descriptor < 0:
        raise AuthorityError("DeviceCheck JWT file descriptor must be nonnegative")
    try:
        metadata = os.fstat(descriptor)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or metadata.st_nlink != 1
            or metadata.st_uid != os.geteuid()
            or stat.S_IMODE(metadata.st_mode) & 0o077
            or not 1 <= metadata.st_size <= MAX_JWT_BYTES
        ):
            raise AuthorityError(
                "DeviceCheck JWT descriptor must reference one bounded owner-private regular file"
            )
        chunks: list[bytes] = []
        size = 0
        while size < metadata.st_size:
            chunk = os.read(
                descriptor, min(4096, metadata.st_size - size)
            )
            if not chunk:
                raise AuthorityError("DeviceCheck JWT descriptor ended before its declared size")
            size += len(chunk)
            chunks.append(chunk)
        after = os.fstat(descriptor)
        if candidate_evidence._metadata_identity(
            after
        ) != candidate_evidence._metadata_identity(metadata):
            raise AuthorityError("DeviceCheck JWT descriptor changed while reading")
        return b"".join(chunks)
    except OSError as error:
        raise AuthorityError("DeviceCheck JWT descriptor could not be read") from error


def request_apple_receipt(
    embedded_receipt: bytes,
    jwt: str,
    *,
    connect_timeout_seconds: float = DEFAULT_CONNECT_TIMEOUT_SECONDS,
    total_timeout_seconds: float = DEFAULT_TOTAL_TIMEOUT_SECONDS,
) -> bytes:
    """Refresh one embedded receipt at Apple's fixed production endpoint."""

    if not 0 < connect_timeout_seconds <= total_timeout_seconds <= 60:
        raise AuthorityError("Apple HTTPS timeouts must be positive and at most 60 seconds")
    if not 1 <= len(embedded_receipt) <= production_evidence.MAX_RECEIPT_BYTES:
        raise AuthorityError("embedded App Attest receipt size is outside its bound")
    request_body = base64.b64encode(embedded_receipt)
    deadline = time.monotonic() + total_timeout_seconds
    context = ssl.create_default_context(ssl.Purpose.SERVER_AUTH)
    context.minimum_version = ssl.TLSVersion.TLSv1_2
    connection = http.client.HTTPSConnection(
        APPLE_PRODUCTION_HOST,
        443,
        timeout=min(connect_timeout_seconds, total_timeout_seconds),
        context=context,
    )
    try:
        connection.request(
            "POST",
            APPLE_PRODUCTION_PATH,
            body=request_body,
            headers={
                "Authorization": jwt,
                "Content-Type": "application/octet-stream",
                "Content-Length": str(len(request_body)),
                "Accept": "application/octet-stream",
                "User-Agent": "iroha-kagemusha-app-attest-authority/1",
                "Connection": "close",
            },
        )
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise AuthorityError("Apple HTTPS request exceeded its total timeout")
        if connection.sock is not None:
            connection.sock.settimeout(remaining)
        response = connection.getresponse()
        body = bytearray()
        while len(body) <= MAX_APPLE_RESPONSE_BYTES:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise AuthorityError("Apple HTTPS response exceeded its total timeout")
            if connection.sock is not None:
                connection.sock.settimeout(remaining)
            chunk = response.read(min(16 * 1024, MAX_APPLE_RESPONSE_BYTES + 1 - len(body)))
            if not chunk:
                break
            body.extend(chunk)
        if len(body) > MAX_APPLE_RESPONSE_BYTES:
            raise AuthorityError("Apple HTTPS response body exceeds its bound")
        if response.status != 200:
            raise AuthorityError(
                f"Apple attestationData request failed with HTTP {response.status}"
            )
    except (OSError, ssl.SSLError, socket.timeout, http.client.HTTPException) as error:
        raise AuthorityError("Apple production attestationData request failed closed") from error
    finally:
        connection.close()
    encoded = bytes(body).strip()
    if not encoded or re.fullmatch(rb"[A-Za-z0-9+/]*={0,2}", encoded) is None:
        raise AuthorityError("Apple response is not bounded canonical base64")
    try:
        receipt = base64.b64decode(encoded, validate=True)
    except ValueError as error:
        raise AuthorityError("Apple response base64 is invalid") from error
    if base64.b64encode(receipt) != encoded:
        raise AuthorityError("Apple response base64 is not canonical")
    if not 1 <= len(receipt) <= MAX_APPLE_RESPONSE_BYTES:
        raise AuthorityError("Apple decoded receipt size is outside its bound")
    return receipt


def _build_unsigned_receipt(
    *,
    validated: ValidatedEvidence,
    receipt_id: str,
    consumption_id: str,
    issued_at_unix_ms: int,
    consumed_at_unix_ms: int,
    expires_at_unix_ms: int,
    apple_checked_at_unix_ms: int,
    previous_assertion_counter: int,
    authority_key_id: str,
    authority_public_key_sha256: str,
) -> dict[str, Any]:
    platform = validated.evidence["platform_evidence"]
    facts = validated.facts
    # These revocation-labeled values are required by the exact downstream v1
    # schema. ``good`` means a current, cryptographically verified production
    # App Attest receipt refresh/risk decision plus the static policy check; it
    # does not recast Apple's attestationData endpoint as CRL or OCSP.
    return {
        "schema": production_evidence.FRESHNESS_RECEIPT_SCHEMA,
        "version": 1,
        "receipt_id": receipt_id,
        "consumption_id": consumption_id,
        "issued_at_unix_ms": issued_at_unix_ms,
        "consumed_at_unix_ms": consumed_at_unix_ms,
        "expires_at_unix_ms": expires_at_unix_ms,
        "status": "issued-and-consumed-once",
        "apple_revocation_checked_at_unix_ms": apple_checked_at_unix_ms,
        "apple_revocation_status": "good",
        "apple_revocation_source": production_evidence.ONLINE_REVOCATION_SOURCE,
        "evidence_sha256": validated.evidence_sha256,
        "production_policy_sha256": validated.policy_sha256,
        "release_manifest_sha256": validated.release_manifest_sha256,
        "platform_evidence_sha256": hashlib.sha256(
            candidate_evidence.canonical_json_bytes(platform)
        ).hexdigest(),
        "attestation_client_data_sha256": hashlib.sha256(
            facts.attestation_client_data
        ).hexdigest(),
        "attestation_object_sha256": hashlib.sha256(
            facts.attestation_object
        ).hexdigest(),
        "assertion_client_data_sha256": hashlib.sha256(
            facts.assertion_client_data
        ).hexdigest(),
        "assertion_object_sha256": hashlib.sha256(
            facts.assertion_object
        ).hexdigest(),
        "attestation_challenge_nonce_sha256": hashlib.sha256(
            facts.attestation_challenge_nonce
        ).hexdigest(),
        "assertion_challenge_nonce_sha256": hashlib.sha256(
            facts.assertion_challenge_nonce
        ).hexdigest(),
        "attestation_nonce_sha256": facts.attestation_nonce.hex(),
        "assertion_nonce_sha256": facts.assertion_nonce.hex(),
        "key_id": facts.key_id,
        "previous_assertion_counter": previous_assertion_counter,
        "assertion_counter": facts.assertion_counter,
        "certificate_chain_sha256": [
            hashlib.sha256(certificate).hexdigest()
            for certificate in facts.certificate_chain
        ],
        "signer_key_id": authority_key_id,
        "signer_public_key_sha256": authority_public_key_sha256,
        "signature_algorithm": "ed25519",
    }


def _validate_catalog_revalidation_release_statuses(
    release_statuses: list[dict[str, Any]],
    *,
    issued_at_unix_ms: int,
    label: str,
    validate_current_status: bool = True,
) -> tuple[list[dict[str, str]], str]:
    """Validate every current status and return its immutable catalog binding."""

    if not isinstance(release_statuses, list) or not (
        1
        <= len(release_statuses)
        <= production_evidence.MAX_CATALOG_REVALIDATION_RELEASES
    ):
        raise AuthorityError(
            f"{label} must contain between 1 and "
            f"{production_evidence.MAX_CATALOG_REVALIDATION_RELEASES} releases"
        )
    if (
        isinstance(issued_at_unix_ms, bool)
        or not isinstance(issued_at_unix_ms, int)
        or issued_at_unix_ms <= 0
    ):
        raise AuthorityError(f"{label} issuance time must be positive Unix milliseconds")
    bindings: list[dict[str, str]] = []
    for index, status in enumerate(release_statuses):
        status_label = f"{label} release_statuses[{index}]"
        if not isinstance(status, dict) or set(status) != set(
            production_evidence.CATALOG_REVALIDATION_RELEASE_STATUS_FIELDS
        ):
            raise AuthorityError(f"{status_label} fields are not exact")
        binding = {
            field: _require_sha256(status.get(field), f"{status_label}.{field}")
            for field in sorted(
                production_evidence.CATALOG_REVALIDATION_BINDING_FIELDS
            )
        }
        bindings.append(binding)
        if not validate_current_status:
            continue
        status_key_id = status.get("app_attest_key_id")
        if (
            not isinstance(status_key_id, str)
            or not status_key_id
            or len(status_key_id) > 1024
        ):
            raise AuthorityError(f"{status_label}.app_attest_key_id is invalid")
        checked_at = status.get("apple_status_checked_at_unix_ms")
        if (
            isinstance(checked_at, bool)
            or not isinstance(checked_at, int)
            or checked_at <= 0
        ):
            raise AuthorityError(
                f"{status_label}.apple_status_checked_at_unix_ms must be positive"
            )
        age = issued_at_unix_ms - checked_at
        if not (
            -production_evidence.MAX_ONLINE_CLOCK_SKEW_MS
            <= age
            <= production_evidence.MAX_ONLINE_REVOCATION_AGE_MS
        ):
            raise AuthorityError(f"{status_label} Apple status is not fresh at issuance")
        if status.get("apple_status") != "good":
            raise AuthorityError(f"{status_label}.apple_status must be good")
        if (
            status.get("apple_status_source")
            != production_evidence.ONLINE_REVOCATION_SOURCE
        ):
            raise AuthorityError(f"{status_label}.apple_status_source is unsupported")
        _require_sha256(
            status.get("refreshed_apple_receipt_sha256"),
            f"{status_label}.refreshed_apple_receipt_sha256",
        )
        risk_metric = status.get("risk_metric")
        if (
            isinstance(risk_metric, bool)
            or not isinstance(risk_metric, int)
            or not 0 <= risk_metric <= 0x7FFFFFFF
        ):
            raise AuthorityError(f"{status_label}.risk_metric is invalid")
    try:
        catalog_sha256 = production_evidence.catalog_revalidation_digest(
            bindings, candidate_evidence
        )
    except ValueError as error:
        raise AuthorityError(str(error)) from error
    return bindings, catalog_sha256


def _validate_persisted_catalog_revalidation_record(
    row: tuple[Any, ...],
    *,
    expected_promotion_id: str,
    expected_catalog_sha256: str,
    expected_bindings: list[dict[str, str]],
    expected_authority_key_id: str,
    expected_authority_public_key_sha256: str,
    evaluation_time_unix_ms: int,
) -> CatalogRevalidationRecord:
    """Validate one exact durable row and its canonical unsigned payload."""

    if not isinstance(row, tuple) or len(row) != len(
        CATALOG_REVALIDATION_TABLE_COLUMNS
    ):
        raise AuthorityError("persisted catalog revalidation row fields are not exact")
    if (
        isinstance(evaluation_time_unix_ms, bool)
        or not isinstance(evaluation_time_unix_ms, int)
        or evaluation_time_unix_ms <= 0
    ):
        raise AuthorityError(
            "persisted catalog revalidation evaluation time must be positive"
        )
    _require_sha256(expected_promotion_id, "expected catalog promotion id")
    _require_sha256(expected_catalog_sha256, "expected catalog digest")
    _require_sha256(
        expected_authority_public_key_sha256,
        "expected catalog authority public key digest",
    )
    if not isinstance(expected_authority_key_id, str):
        raise AuthorityError("expected catalog authority key id is invalid")
    try:
        candidate_evidence._validate_key_id(
            expected_authority_key_id, "expected catalog authority key id"
        )
        expected_bindings_sha256 = production_evidence.catalog_revalidation_digest(
            expected_bindings, candidate_evidence
        )
    except (ValueError, candidate_evidence.EvidenceError) as error:
        raise AuthorityError(str(error)) from error
    if expected_bindings_sha256 != expected_catalog_sha256:
        raise AuthorityError("expected catalog bindings do not match its digest")

    (
        promotion_id,
        catalog_sha256,
        receipt_id,
        issued_at_unix_ms,
        expires_at_unix_ms,
        authority_key_id,
        authority_public_key_sha256,
        payload,
        state,
        retired_at_unix_ms,
    ) = row
    _require_sha256(promotion_id, "persisted catalog promotion id")
    _require_sha256(catalog_sha256, "persisted catalog digest")
    _require_sha256(receipt_id, "persisted catalog receipt id")
    _require_sha256(
        authority_public_key_sha256,
        "persisted catalog authority public key digest",
    )
    if not isinstance(authority_key_id, str):
        raise AuthorityError("persisted catalog authority key id is invalid")
    try:
        candidate_evidence._validate_key_id(
            authority_key_id, "persisted catalog authority key id"
        )
    except candidate_evidence.EvidenceError as error:
        raise AuthorityError(str(error)) from error
    if receipt_id == promotion_id:
        raise AuthorityError("persisted catalog receipt id equals its promotion id")
    for value, timestamp_label in (
        (issued_at_unix_ms, "issued_at_unix_ms"),
        (expires_at_unix_ms, "expires_at_unix_ms"),
    ):
        if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
            raise AuthorityError(
                f"persisted catalog revalidation {timestamp_label} must be positive"
            )
    if not issued_at_unix_ms < expires_at_unix_ms:
        raise AuthorityError("persisted catalog revalidation timestamp order is invalid")
    if (
        expires_at_unix_ms - issued_at_unix_ms
        > production_evidence.MAX_ONLINE_RECEIPT_LIFETIME_MS
    ):
        raise AuthorityError("persisted catalog revalidation lifetime exceeds its bound")
    if (
        issued_at_unix_ms
        > evaluation_time_unix_ms
        + production_evidence.MAX_ONLINE_CLOCK_SKEW_MS
    ):
        raise AuthorityError("persisted catalog revalidation issuance is in the future")
    if state == "active":
        if retired_at_unix_ms is not None:
            raise AuthorityError("active catalog revalidation has a retirement timestamp")
    elif state == "expired":
        if (
            isinstance(retired_at_unix_ms, bool)
            or not isinstance(retired_at_unix_ms, int)
            or retired_at_unix_ms <= expires_at_unix_ms
        ):
            raise AuthorityError("expired catalog revalidation retirement is invalid")
    else:
        raise AuthorityError("persisted catalog revalidation state is invalid")
    if not isinstance(payload, bytes) or not payload:
        raise AuthorityError("persisted catalog revalidation payload is not a BLOB")
    try:
        receipt = candidate_evidence.parse_strict_json(
            payload + b"\n", "persisted catalog revalidation receipt payload"
        )
        canonical_payload = candidate_evidence.canonical_signature_payload(receipt)
    except candidate_evidence.EvidenceError as error:
        raise AuthorityError(str(error)) from error
    if canonical_payload != payload:
        raise AuthorityError("persisted catalog revalidation payload is not canonical")
    unsigned_fields = production_evidence.CATALOG_REVALIDATION_RECEIPT_FIELDS - {
        "signature_payload_sha256",
        "signature",
    }
    if set(receipt) != set(unsigned_fields):
        raise AuthorityError("persisted catalog revalidation receipt fields are not exact")
    if receipt.get("schema") != production_evidence.CATALOG_REVALIDATION_RECEIPT_SCHEMA:
        raise AuthorityError("persisted catalog revalidation schema is unsupported")
    if receipt.get("version") != 1 or isinstance(receipt.get("version"), bool):
        raise AuthorityError("persisted catalog revalidation version must be integer 1")
    payload_receipt_id = _require_sha256(
        receipt.get("receipt_id"), "persisted payload receipt id"
    )
    payload_promotion_id = _require_sha256(
        receipt.get("promotion_id"), "persisted payload promotion id"
    )
    payload_catalog_sha256 = _require_sha256(
        receipt.get("catalog_sha256"), "persisted payload catalog digest"
    )
    if payload_receipt_id == payload_promotion_id:
        raise AuthorityError("persisted payload receipt id equals its promotion id")
    for value, timestamp_label in (
        (receipt.get("issued_at_unix_ms"), "issued_at_unix_ms"),
        (receipt.get("expires_at_unix_ms"), "expires_at_unix_ms"),
    ):
        if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
            raise AuthorityError(
                f"persisted payload {timestamp_label} must be positive Unix milliseconds"
            )
    if (
        payload_receipt_id != receipt_id
        or payload_promotion_id != promotion_id
        or payload_catalog_sha256 != catalog_sha256
        or receipt.get("issued_at_unix_ms") != issued_at_unix_ms
        or receipt.get("expires_at_unix_ms") != expires_at_unix_ms
        or receipt.get("signer_key_id") != authority_key_id
        or receipt.get("signer_public_key_sha256")
        != authority_public_key_sha256
    ):
        raise AuthorityError("persisted catalog row and receipt payload do not match")
    if receipt.get("status") != "catalog-revalidated-for-one-promotion":
        raise AuthorityError("persisted catalog revalidation status is invalid")
    if receipt.get("signature_algorithm") != "ed25519":
        raise AuthorityError("persisted catalog revalidation algorithm is invalid")
    statuses = receipt.get("release_statuses")
    observed_bindings, observed_catalog_sha256 = (
        _validate_catalog_revalidation_release_statuses(
            statuses,
            issued_at_unix_ms=issued_at_unix_ms,
            label="persisted catalog revalidation receipt",
        )
    )
    if observed_catalog_sha256 != catalog_sha256:
        raise AuthorityError("persisted catalog receipt does not match its catalog digest")
    if observed_bindings != expected_bindings:
        raise AuthorityError(
            "promotion id replay substituted the immutable release catalog"
        )
    if (
        promotion_id != expected_promotion_id
        or catalog_sha256 != expected_catalog_sha256
        or authority_key_id != expected_authority_key_id
        or authority_public_key_sha256
        != expected_authority_public_key_sha256
    ):
        raise AuthorityError(
            "promotion id was already bound to another catalog or authority"
        )
    return CatalogRevalidationRecord(
        receipt=receipt,
        bindings=tuple(observed_bindings),
        promotion_id=promotion_id,
        catalog_sha256=catalog_sha256,
        receipt_id=receipt_id,
        issued_at_unix_ms=issued_at_unix_ms,
        expires_at_unix_ms=expires_at_unix_ms,
        authority_key_id=authority_key_id,
        authority_public_key_sha256=authority_public_key_sha256,
        state=state,
        retired_at_unix_ms=retired_at_unix_ms,
    )


def _build_unsigned_catalog_revalidation_receipt(
    *,
    receipt_id: str,
    promotion_id: str,
    catalog_sha256: str,
    issued_at_unix_ms: int,
    expires_at_unix_ms: int,
    release_statuses: list[dict[str, Any]],
    authority_key_id: str,
    authority_public_key_sha256: str,
) -> dict[str, Any]:
    """Build the exact unsigned current-status receipt persisted before signing."""

    return {
        "schema": production_evidence.CATALOG_REVALIDATION_RECEIPT_SCHEMA,
        "version": 1,
        "receipt_id": receipt_id,
        "promotion_id": promotion_id,
        "catalog_sha256": catalog_sha256,
        "issued_at_unix_ms": issued_at_unix_ms,
        "expires_at_unix_ms": expires_at_unix_ms,
        "status": "catalog-revalidated-for-one-promotion",
        "release_statuses": release_statuses,
        "signer_key_id": authority_key_id,
        "signer_public_key_sha256": authority_public_key_sha256,
        "signature_algorithm": "ed25519",
    }


def _authority_public_key(
    *,
    key_id: str,
    public_key_path: Path,
    validated: ValidatedEvidence,
) -> str:
    try:
        candidate_evidence._validate_key_id(key_id, "online authority key id")
        digest = candidate_evidence.signer_public_key_sha256(public_key_path)
    except candidate_evidence.EvidenceError as error:
        raise AuthorityError(str(error)) from error
    if (
        key_id == validated.lab_signer_key_id
        or digest == validated.lab_signer_public_key_sha256
    ):
        raise AuthorityError(
            "online freshness authority must be independent from the lab signer"
        )
    return digest


def sign_committed_receipt(
    receipt: dict[str, Any],
    *,
    private_key_path: Path,
    public_key_path: Path,
) -> dict[str, Any]:
    """Sign an already committed canonical receipt and verify the key pair."""

    try:
        payload = candidate_evidence.canonical_signature_payload(receipt)
        signature = candidate_evidence.sign_ed25519(private_key_path, payload)
        candidate_evidence.verify_ed25519(public_key_path, payload, signature)
    except candidate_evidence.EvidenceError as error:
        raise AuthorityError(str(error)) from error
    signed = dict(receipt)
    signed["signature_payload_sha256"] = hashlib.sha256(payload).hexdigest()
    signed["signature"] = signature.hex()
    if set(signed) != production_evidence.FRESHNESS_RECEIPT_FIELDS:
        raise AuthorityError("signed freshness receipt fields are not exact")
    return signed


def sign_committed_catalog_revalidation_receipt(
    receipt: dict[str, Any],
    *,
    private_key_path: Path,
    public_key_path: Path,
) -> dict[str, Any]:
    """Sign one durably committed catalog receipt and enforce its exact schema."""

    try:
        payload = candidate_evidence.canonical_signature_payload(receipt)
        signature = candidate_evidence.sign_ed25519(private_key_path, payload)
        candidate_evidence.verify_ed25519(public_key_path, payload, signature)
    except candidate_evidence.EvidenceError as error:
        raise AuthorityError(str(error)) from error
    signed = dict(receipt)
    signed["signature_payload_sha256"] = hashlib.sha256(payload).hexdigest()
    signed["signature"] = signature.hex()
    if set(signed) != production_evidence.CATALOG_REVALIDATION_RECEIPT_FIELDS:
        raise AuthorityError("signed catalog revalidation receipt fields are not exact")
    return signed


def _preflight_challenge(
    state: AuthorityState,
    challenge_id: str,
    validated: ValidatedEvidence,
    evaluation_time_unix_ms: int,
) -> bool:
    row = state.challenge_row(challenge_id)
    (
        _,
        _,
        _,
        expires_at,
        request_sha256,
        _,
        attestation_nonce,
        assertion_nonce,
        challenge_state,
        prior_evidence_sha256,
        _,
        _,
        _,
        _,
        _,
    ) = row
    if challenge_state == "consumed":
        if prior_evidence_sha256 != validated.evidence_sha256:
            raise AuthorityError("one-time challenge was already consumed")
        return True
    if evaluation_time_unix_ms > expires_at:
        raise AuthorityError("one-time challenge expired before Apple validation")
    if request_sha256 != validated.capture_request_sha256:
        raise AuthorityError("evidence does not bind the issued capture request")
    if attestation_nonce != validated.facts.attestation_challenge_nonce:
        raise AuthorityError("evidence substituted the attestation challenge nonce")
    if assertion_nonce != validated.facts.assertion_challenge_nonce:
        raise AuthorityError("evidence substituted the assertion challenge nonce")
    return False


def consume_evidence(
    state: AuthorityState,
    *,
    challenge_id: str,
    evidence_path: Path,
    artifact_root: Path,
    production_policy_path: Path,
    capture_app_code_sign_measurements_path: Path,
    trusted_lab_key_id: str,
    trusted_lab_public_key_path: Path,
    authority_key_id: str,
    authority_private_key_path: Path,
    authority_public_key_path: Path,
    maximum_risk_metric: int,
    devicecheck_jwt_file: Optional[Path] = None,
    devicecheck_jwt_fd: Optional[int] = None,
    output_path: Optional[Path] = None,
    connect_timeout_seconds: float = DEFAULT_CONNECT_TIMEOUT_SECONDS,
    total_timeout_seconds: float = DEFAULT_TOTAL_TIMEOUT_SECONDS,
    openssl_path: Path = Path("/usr/bin/openssl"),
    apple_root_path: Path = APPLE_RECEIPT_ROOT,
    apple_transport: Optional[Callable[[bytes, str], bytes]] = None,
    evaluation_time: Callable[[], int] = _now_ms,
    after_commit: Optional[Callable[[], None]] = None,
    expected_apple_root_sha256: str = APPLE_ROOT_CA_G3_DER_SHA256,
    enforce_trusted_openssl: bool = True,
) -> dict[str, Any]:
    """Consume exact production evidence and emit its canonical signed receipt."""

    _require_sha256(challenge_id, "challenge id")
    receipt_output = (
        _require_new_private_output(output_path, "freshness receipt output")
        if output_path is not None
        else None
    )
    validated = _load_validated_evidence(
        evidence_path=evidence_path,
        artifact_root=artifact_root,
        production_policy_path=production_policy_path,
        capture_app_code_sign_measurements_path=(
            capture_app_code_sign_measurements_path
        ),
        trusted_lab_key_id=trusted_lab_key_id,
        trusted_lab_public_key_path=trusted_lab_public_key_path,
    )
    authority_public_key_sha256 = _authority_public_key(
        key_id=authority_key_id,
        public_key_path=authority_public_key_path,
        validated=validated,
    )
    checked_at = evaluation_time()
    if (
        isinstance(checked_at, bool)
        or not isinstance(checked_at, int)
        or checked_at <= 0
    ):
        raise AuthorityError("authority evaluation time must be positive Unix milliseconds")
    already_consumed = _preflight_challenge(
        state, challenge_id, validated, checked_at
    )
    if already_consumed:
        receipt, _ = state.commit_consumption(
            challenge_id=challenge_id,
            validated=validated,
            apple_receipt=b"",
            apple_facts=AppleReceiptFacts(1, 2, 3, 0, 1),
            authority_key_id=authority_key_id,
            authority_public_key_sha256=authority_public_key_sha256,
            consumed_at_unix_ms=checked_at,
        )
        if checked_at >= receipt.get("expires_at_unix_ms", 0):
            raise AuthorityError("committed freshness receipt is expired")
        signed = sign_committed_receipt(
            receipt,
            private_key_path=authority_private_key_path,
            public_key_path=authority_public_key_path,
        )
        if receipt_output is not None:
            _write_new_private_json(
                receipt_output, signed, "freshness receipt output"
            )
        return signed

    jwt_payload = read_devicecheck_jwt(
        file_path=devicecheck_jwt_file, descriptor=devicecheck_jwt_fd
    )
    jwt = validate_devicecheck_jwt(
        jwt_payload,
        expected_issuer=validated.policy["app_id_prefix"],
        evaluation_time_unix_ms=checked_at,
    )
    if apple_transport is None:
        refreshed_receipt = request_apple_receipt(
            validated.embedded_apple_receipt,
            jwt,
            connect_timeout_seconds=connect_timeout_seconds,
            total_timeout_seconds=total_timeout_seconds,
        )
    else:
        refreshed_receipt = apple_transport(validated.embedded_apple_receipt, jwt)
    if not isinstance(refreshed_receipt, bytes):
        raise AuthorityError("Apple receipt transport returned a non-byte response")
    receipt_checked_at = evaluation_time()
    if (
        isinstance(receipt_checked_at, bool)
        or not isinstance(receipt_checked_at, int)
        or receipt_checked_at <= 0
    ):
        raise AuthorityError("Apple receipt check time must be positive Unix milliseconds")
    apple_facts = verify_apple_receipt(
        refreshed_receipt,
        expected_app_id=validated.app_id,
        expected_public_key=validated.assertion_public_key,
        maximum_risk_metric=maximum_risk_metric,
        evaluation_time_unix_ms=receipt_checked_at,
        root_pem_path=apple_root_path,
        openssl_path=openssl_path,
        cms_timeout_seconds=min(10.0, total_timeout_seconds),
        expected_root_sha256=expected_apple_root_sha256,
        enforce_trusted_executable=enforce_trusted_openssl,
    )

    revalidated = _load_validated_evidence(
        evidence_path=evidence_path,
        artifact_root=artifact_root,
        production_policy_path=production_policy_path,
        capture_app_code_sign_measurements_path=(
            capture_app_code_sign_measurements_path
        ),
        trusted_lab_key_id=trusted_lab_key_id,
        trusted_lab_public_key_path=trusted_lab_public_key_path,
    )
    if (
        revalidated.evidence_sha256 != validated.evidence_sha256
        or revalidated.policy_sha256 != validated.policy_sha256
        or revalidated.capture_request_sha256 != validated.capture_request_sha256
        or revalidated.embedded_apple_receipt != validated.embedded_apple_receipt
    ):
        raise AuthorityError("production evidence changed during Apple validation")
    committed_at = evaluation_time()
    if (
        isinstance(committed_at, bool)
        or not isinstance(committed_at, int)
        or committed_at <= 0
    ):
        raise AuthorityError("authority commit time must be positive Unix milliseconds")
    receipt, _ = state.commit_consumption(
        challenge_id=challenge_id,
        validated=revalidated,
        apple_receipt=refreshed_receipt,
        apple_facts=apple_facts,
        authority_key_id=authority_key_id,
        authority_public_key_sha256=authority_public_key_sha256,
        consumed_at_unix_ms=committed_at,
    )
    if after_commit is not None:
        after_commit()
    signed = sign_committed_receipt(
        receipt,
        private_key_path=authority_private_key_path,
        public_key_path=authority_public_key_path,
    )
    if receipt_output is not None:
        _write_new_private_json(receipt_output, signed, "freshness receipt output")
    return signed


def _load_catalog_revalidation_request(path: Path) -> tuple[Any, dict[str, Any]]:
    """Snapshot and parse one exact private catalog revalidation request."""

    try:
        snapshot = candidate_evidence._snapshot_private_file(
            path.resolve(strict=True),
            "catalog revalidation request",
            maximum=MAX_CATALOG_REVALIDATION_REQUEST_BYTES,
            retain_payload=True,
        )
        request = candidate_evidence.parse_strict_json(
            snapshot.payload, "catalog revalidation request"
        )
    except (OSError, candidate_evidence.EvidenceError) as error:
        raise AuthorityError(str(error)) from error
    if set(request) != set(CATALOG_REVALIDATION_REQUEST_FIELDS):
        raise AuthorityError("catalog revalidation request fields are not exact")
    if request.get("schema") != CATALOG_REVALIDATION_REQUEST_SCHEMA:
        raise AuthorityError("catalog revalidation request schema is unsupported")
    if request.get("version") != 1 or isinstance(request.get("version"), bool):
        raise AuthorityError("catalog revalidation request version must be integer 1")
    _require_sha256(request.get("promotion_id"), "promotion id")
    releases = request.get("releases")
    if not isinstance(releases, list) or not (
        1
        <= len(releases)
        <= production_evidence.MAX_CATALOG_REVALIDATION_RELEASES
    ):
        raise AuthorityError(
            "catalog revalidation request release count is outside its bound"
        )
    for index, release in enumerate(releases):
        if not isinstance(release, dict) or set(release) != set(
            CATALOG_REVALIDATION_REQUEST_RELEASE_FIELDS
        ):
            raise AuthorityError(
                f"catalog revalidation request releases[{index}] fields are not exact"
            )
        for field in CATALOG_REVALIDATION_REQUEST_RELEASE_FIELDS:
            value = release.get(field)
            if not isinstance(value, str) or not value:
                raise AuthorityError(
                    f"catalog revalidation request releases[{index}].{field} is invalid"
                )
            candidate = Path(value)
            if (
                not candidate.is_absolute()
                or candidate.resolve(strict=False) != candidate
            ):
                raise AuthorityError(
                    f"catalog revalidation request releases[{index}].{field} "
                    "must be a canonical absolute path"
                )
    return snapshot, request


def revalidate_catalog(
    state: AuthorityState,
    *,
    request_path: Path,
    production_policy_path: Path,
    trusted_lab_key_id: str,
    trusted_lab_public_key_path: Path,
    original_receipt_authority_key_id: str,
    original_receipt_authority_public_key_path: Path,
    authority_key_id: str,
    authority_private_key_path: Path,
    authority_public_key_path: Path,
    maximum_risk_metric: int,
    devicecheck_jwt_file: Optional[Path] = None,
    devicecheck_jwt_fd: Optional[int] = None,
    output_path: Optional[Path] = None,
    connect_timeout_seconds: float = DEFAULT_CONNECT_TIMEOUT_SECONDS,
    total_timeout_seconds: float = DEFAULT_TOTAL_TIMEOUT_SECONDS,
    apple_transport: Optional[Callable[[bytes, str], bytes]] = None,
    evaluation_time: Callable[[], int] = _now_ms,
    after_commit: Optional[Callable[[], None]] = None,
    apple_root_path: Path = APPLE_RECEIPT_ROOT,
    openssl_path: Path = Path("/usr/bin/openssl"),
    expected_apple_root_sha256: str = APPLE_ROOT_CA_G3_DER_SHA256,
    enforce_trusted_openssl: bool = True,
) -> dict[str, Any]:
    """Refresh Apple status for an exact catalog and issue one promotion receipt."""

    output_path = (
        _require_new_private_output(output_path, "catalog revalidation receipt output")
        if output_path is not None
        else None
    )
    request_snapshot, request = _load_catalog_revalidation_request(request_path)
    promotion_id = request["promotion_id"]
    prepared: list[dict[str, Any]] = []
    authority_public_key_sha256: Optional[str] = None
    initial_time = evaluation_time()
    if (
        isinstance(initial_time, bool)
        or not isinstance(initial_time, int)
        or initial_time <= 0
    ):
        raise AuthorityError("authority evaluation time must be positive Unix milliseconds")
    for index, release in enumerate(request["releases"]):
        evidence_path = Path(release["evidence_path"])
        artifact_root = Path(release["artifact_root"])
        consumption_receipt_path = Path(release["consumption_receipt_path"])
        capture_measurements_path = Path(
            release["capture_app_code_sign_measurements_path"]
        )
        validated = _load_validated_evidence(
            evidence_path=evidence_path,
            artifact_root=artifact_root,
            production_policy_path=production_policy_path,
            capture_app_code_sign_measurements_path=capture_measurements_path,
            trusted_lab_key_id=trusted_lab_key_id,
            trusted_lab_public_key_path=trusted_lab_public_key_path,
        )
        historical_errors = production_evidence.validate_historical_production_evidence_for_catalog_revalidation(
            evidence_path,
            artifact_root,
            trusted_lab_key_id,
            trusted_lab_public_key_path,
            production_policy_path,
            candidate_evidence,
            freshness_receipt_path=consumption_receipt_path,
            trusted_freshness_key_id=original_receipt_authority_key_id,
            trusted_freshness_public_key_path=(
                original_receipt_authority_public_key_path
            ),
            evaluation_time_unix_ms=initial_time,
        )
        if historical_errors:
            raise AuthorityError(
                f"catalog release[{index}] historical consumption evidence is invalid: "
                + "; ".join(historical_errors)
            )
        try:
            consumption_snapshot = candidate_evidence._snapshot_private_file(
                consumption_receipt_path.resolve(strict=True),
                "historical consumption receipt",
                maximum=production_evidence.MAX_FRESHNESS_RECEIPT_BYTES,
                retain_payload=True,
            )
        except (OSError, candidate_evidence.EvidenceError) as error:
            raise AuthorityError(str(error)) from error
        binding = {
            "release_manifest_sha256": validated.release_manifest_sha256,
            "evidence_sha256": validated.evidence_sha256,
            "consumption_receipt_sha256": consumption_snapshot.sha256,
        }
        observed_authority_digest = _authority_public_key(
            key_id=authority_key_id,
            public_key_path=authority_public_key_path,
            validated=validated,
        )
        if authority_public_key_sha256 is None:
            authority_public_key_sha256 = observed_authority_digest
        elif authority_public_key_sha256 != observed_authority_digest:
            raise AuthorityError("catalog authority key identity changed between releases")
        prepared.append(
            {
                "index": index,
                "binding": binding,
                "validated": validated,
                "evidence_path": evidence_path,
                "artifact_root": artifact_root,
                "consumption_receipt_path": consumption_receipt_path,
                "consumption_snapshot": consumption_snapshot,
                "capture_measurements_path": capture_measurements_path,
            }
        )
    prepared.sort(key=lambda value: value["binding"]["release_manifest_sha256"])
    bindings = [value["binding"] for value in prepared]
    try:
        catalog_sha256 = production_evidence.catalog_revalidation_digest(
            bindings, candidate_evidence
        )
    except ValueError as error:
        raise AuthorityError(str(error)) from error
    def require_immutable_inputs_unchanged(evaluation_time_unix_ms: int) -> None:
        for value in prepared:
            prior: ValidatedEvidence = value["validated"]
            current = _load_validated_evidence(
                evidence_path=value["evidence_path"],
                artifact_root=value["artifact_root"],
                production_policy_path=production_policy_path,
                capture_app_code_sign_measurements_path=value[
                    "capture_measurements_path"
                ],
                trusted_lab_key_id=trusted_lab_key_id,
                trusted_lab_public_key_path=trusted_lab_public_key_path,
            )
            if (
                current.evidence_sha256 != prior.evidence_sha256
                or current.policy_sha256 != prior.policy_sha256
                or current.release_manifest_sha256 != prior.release_manifest_sha256
                or current.embedded_apple_receipt != prior.embedded_apple_receipt
            ):
                raise AuthorityError("catalog evidence changed during validation")
            historical_errors = production_evidence.validate_historical_production_evidence_for_catalog_revalidation(
                value["evidence_path"],
                value["artifact_root"],
                trusted_lab_key_id,
                trusted_lab_public_key_path,
                production_policy_path,
                candidate_evidence,
                freshness_receipt_path=value["consumption_receipt_path"],
                trusted_freshness_key_id=original_receipt_authority_key_id,
                trusted_freshness_public_key_path=(
                    original_receipt_authority_public_key_path
                ),
                evaluation_time_unix_ms=evaluation_time_unix_ms,
            )
            if historical_errors:
                raise AuthorityError(
                    "catalog historical consumption evidence changed during validation: "
                    + "; ".join(historical_errors)
                )
            try:
                candidate_evidence._require_private_file_snapshot_unchanged(
                    value["consumption_snapshot"],
                    "historical consumption receipt",
                    maximum=production_evidence.MAX_FRESHNESS_RECEIPT_BYTES,
                )
            except candidate_evidence.EvidenceError as error:
                raise AuthorityError(str(error)) from error
        try:
            candidate_evidence._require_private_file_snapshot_unchanged(
                request_snapshot,
                "catalog revalidation request",
                maximum=MAX_CATALOG_REVALIDATION_REQUEST_BYTES,
            )
        except candidate_evidence.EvidenceError as error:
            raise AuthorityError(str(error)) from error

    assert authority_public_key_sha256 is not None
    require_immutable_inputs_unchanged(initial_time)
    recovered = state.recover_catalog_revalidation(
        promotion_id=promotion_id,
        catalog_sha256=catalog_sha256,
        bindings=bindings,
        authority_key_id=authority_key_id,
        authority_public_key_sha256=authority_public_key_sha256,
        evaluation_time_unix_ms=initial_time,
    )
    if recovered is not None:
        signed = sign_committed_catalog_revalidation_receipt(
            recovered,
            private_key_path=authority_private_key_path,
            public_key_path=authority_public_key_path,
        )
        if output_path is not None:
            _write_new_private_json(
                output_path, signed, "catalog revalidation receipt output"
            )
        return signed
    jwt_payload = read_devicecheck_jwt(
        file_path=devicecheck_jwt_file, descriptor=devicecheck_jwt_fd
    )
    first_validated: ValidatedEvidence = prepared[0]["validated"]
    jwt = validate_devicecheck_jwt(
        jwt_payload,
        expected_issuer=first_validated.policy["app_id_prefix"],
        evaluation_time_unix_ms=initial_time,
    )
    release_statuses: list[dict[str, Any]] = []
    for value in prepared:
        validated = value["validated"]
        if validated.policy != first_validated.policy:
            raise AuthorityError("catalog releases do not share the exact production policy")
        if apple_transport is None:
            refreshed_receipt = request_apple_receipt(
                validated.embedded_apple_receipt,
                jwt,
                connect_timeout_seconds=connect_timeout_seconds,
                total_timeout_seconds=total_timeout_seconds,
            )
        else:
            refreshed_receipt = apple_transport(validated.embedded_apple_receipt, jwt)
        if not isinstance(refreshed_receipt, bytes):
            raise AuthorityError("Apple receipt transport returned a non-byte response")
        checked_at = evaluation_time()
        if (
            isinstance(checked_at, bool)
            or not isinstance(checked_at, int)
            or checked_at <= 0
        ):
            raise AuthorityError(
                "Apple status check time must be positive Unix milliseconds"
            )
        apple_facts = verify_apple_receipt(
            refreshed_receipt,
            expected_app_id=validated.app_id,
            expected_public_key=validated.assertion_public_key,
            maximum_risk_metric=maximum_risk_metric,
            evaluation_time_unix_ms=checked_at,
            root_pem_path=apple_root_path,
            openssl_path=openssl_path,
            cms_timeout_seconds=min(10.0, total_timeout_seconds),
            expected_root_sha256=expected_apple_root_sha256,
            enforce_trusted_executable=enforce_trusted_openssl,
        )
        release_statuses.append(
            {
                **value["binding"],
                "app_attest_key_id": validated.facts.key_id,
                "apple_status_checked_at_unix_ms": apple_facts.verified_at_unix_ms,
                "apple_status": "good",
                "apple_status_source": production_evidence.ONLINE_REVOCATION_SOURCE,
                "refreshed_apple_receipt_sha256": hashlib.sha256(
                    refreshed_receipt
                ).hexdigest(),
                "risk_metric": apple_facts.risk_metric,
            }
        )

    # Close the network race by revalidating every immutable input before the
    # durable promotion-id reservation is committed.
    final_time = evaluation_time()
    if (
        isinstance(final_time, bool)
        or not isinstance(final_time, int)
        or final_time <= 0
    ):
        raise AuthorityError("catalog commit time must be positive Unix milliseconds")
    require_immutable_inputs_unchanged(final_time)
    for status in release_statuses:
        checked_at = status["apple_status_checked_at_unix_ms"]
        age = final_time - checked_at
        if not -MAX_CLOCK_SKEW_MS <= age <= MAX_RECEIPT_CREATION_AGE_MS:
            raise AuthorityError(
                "catalog Apple status became stale before durable receipt commit"
            )
    receipt, recovered_after_network = state.commit_catalog_revalidation(
        promotion_id=promotion_id,
        catalog_sha256=catalog_sha256,
        release_statuses=release_statuses,
        authority_key_id=authority_key_id,
        authority_public_key_sha256=authority_public_key_sha256,
        issued_at_unix_ms=final_time,
    )
    if after_commit is not None and not recovered_after_network:
        after_commit()
    signed = sign_committed_catalog_revalidation_receipt(
        receipt,
        private_key_path=authority_private_key_path,
        public_key_path=authority_public_key_path,
    )
    if output_path is not None:
        _write_new_private_json(
            output_path, signed, "catalog revalidation receipt output"
        )
    return signed


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    issue = subparsers.add_parser(
        "issue", help="durably issue a physical-device capture challenge"
    )
    issue.add_argument("--state-dir", required=True)
    issue.add_argument("--artifact-root", required=True)
    issue.add_argument("--production-policy", required=True)
    issue.add_argument("--capture-app-code-sign-measurements", required=True)
    issue.add_argument("--release-manifest-sha256", required=True)
    issue.add_argument("--request-output", required=True)
    issue.add_argument("--lease-output", required=True)
    issue.add_argument("--lifetime-seconds", type=int, default=300)

    consume = subparsers.add_parser(
        "consume", help="validate, atomically consume, and sign production evidence"
    )
    consume.add_argument("--state-dir", required=True)
    consume.add_argument("--challenge-id", required=True)
    consume.add_argument("--evidence", required=True)
    consume.add_argument("--artifact-root", required=True)
    consume.add_argument("--production-policy", required=True)
    consume.add_argument("--capture-app-code-sign-measurements", required=True)
    consume.add_argument("--trusted-lab-key-id", required=True)
    consume.add_argument("--trusted-lab-public-key", required=True)
    consume.add_argument("--authority-key-id", required=True)
    consume.add_argument("--authority-private-key", required=True)
    consume.add_argument("--authority-public-key", required=True)
    jwt = consume.add_mutually_exclusive_group()
    jwt.add_argument("--devicecheck-jwt-file")
    jwt.add_argument("--devicecheck-jwt-fd", type=int)
    consume.add_argument("--maximum-risk-metric", type=int, required=True)
    consume.add_argument("--receipt-output", required=True)
    consume.add_argument(
        "--connect-timeout-seconds",
        type=float,
        default=DEFAULT_CONNECT_TIMEOUT_SECONDS,
    )
    consume.add_argument(
        "--total-timeout-seconds",
        type=float,
        default=DEFAULT_TOTAL_TIMEOUT_SECONDS,
    )
    revalidate = subparsers.add_parser(
        "revalidate-catalog",
        help="refresh Apple status for one exact multi-release promotion catalog",
    )
    revalidate.add_argument("--state-dir", required=True)
    revalidate.add_argument("--catalog-request", required=True)
    revalidate.add_argument("--production-policy", required=True)
    revalidate.add_argument("--trusted-lab-key-id", required=True)
    revalidate.add_argument("--trusted-lab-public-key", required=True)
    revalidate.add_argument(
        "--original-receipt-authority-key-id", required=True
    )
    revalidate.add_argument(
        "--original-receipt-authority-public-key", required=True
    )
    revalidate.add_argument("--authority-key-id", required=True)
    revalidate.add_argument("--authority-private-key", required=True)
    revalidate.add_argument("--authority-public-key", required=True)
    revalidate_jwt = revalidate.add_mutually_exclusive_group()
    revalidate_jwt.add_argument("--devicecheck-jwt-file")
    revalidate_jwt.add_argument("--devicecheck-jwt-fd", type=int)
    revalidate.add_argument("--maximum-risk-metric", type=int, required=True)
    revalidate.add_argument("--receipt-output", required=True)
    revalidate.add_argument(
        "--connect-timeout-seconds",
        type=float,
        default=DEFAULT_CONNECT_TIMEOUT_SECONDS,
    )
    revalidate.add_argument(
        "--total-timeout-seconds",
        type=float,
        default=DEFAULT_TOTAL_TIMEOUT_SECONDS,
    )
    return parser


def main(argv: Optional[list[str]] = None) -> int:
    args = _parser().parse_args(argv)
    try:
        if args.command == "issue":
            request_output = _require_new_private_output(
                Path(args.request_output), "capture request output"
            )
            lease_output = _require_new_private_output(
                Path(args.lease_output), "challenge lease output"
            )
            if request_output == lease_output:
                raise AuthorityError("capture request and challenge lease outputs must differ")
            receipt_output = None
        elif args.command == "consume":
            request_output = None
            lease_output = None
            receipt_output = _require_new_private_output(
                Path(args.receipt_output), "freshness receipt output"
            )
        else:
            request_output = None
            lease_output = None
            receipt_output = _require_new_private_output(
                Path(args.receipt_output), "catalog revalidation receipt output"
            )
        with AuthorityState(Path(args.state_dir)) as state:
            if args.command == "issue":
                assert request_output is not None
                assert lease_output is not None
                lifetime_ms = args.lifetime_seconds * 1000
                lease = issue_challenge(
                    state,
                    artifact_root=Path(args.artifact_root),
                    production_policy_path=Path(args.production_policy),
                    capture_app_code_sign_measurements_path=Path(
                        args.capture_app_code_sign_measurements
                    ),
                    release_manifest_sha256=args.release_manifest_sha256,
                    lifetime_ms=lifetime_ms,
                )
                _write_new_private_json(
                    request_output, lease.request, "capture request output"
                )
                _write_new_private_json(
                    lease_output, lease.public_value(), "challenge lease output"
                )
                print(
                    "[kagemusha-app-attest-authority] challenge issued durably: "
                    f"{lease.challenge_id}"
                )
                return 0
            if args.command == "consume":
                consume_evidence(
                    state,
                    challenge_id=args.challenge_id,
                    evidence_path=Path(args.evidence),
                    artifact_root=Path(args.artifact_root),
                    production_policy_path=Path(args.production_policy),
                    capture_app_code_sign_measurements_path=Path(
                        args.capture_app_code_sign_measurements
                    ),
                    trusted_lab_key_id=args.trusted_lab_key_id,
                    trusted_lab_public_key_path=Path(args.trusted_lab_public_key),
                    authority_key_id=args.authority_key_id,
                    authority_private_key_path=Path(args.authority_private_key),
                    authority_public_key_path=Path(args.authority_public_key),
                    maximum_risk_metric=args.maximum_risk_metric,
                    devicecheck_jwt_file=(
                        Path(args.devicecheck_jwt_file)
                        if args.devicecheck_jwt_file is not None
                        else None
                    ),
                    devicecheck_jwt_fd=args.devicecheck_jwt_fd,
                    output_path=receipt_output,
                    connect_timeout_seconds=args.connect_timeout_seconds,
                    total_timeout_seconds=args.total_timeout_seconds,
                )
                print(
                    "[kagemusha-app-attest-authority] Apple receipt verified and "
                    "one-time evidence consumed"
                )
            else:
                revalidate_catalog(
                    state,
                    request_path=Path(args.catalog_request),
                    production_policy_path=Path(args.production_policy),
                    trusted_lab_key_id=args.trusted_lab_key_id,
                    trusted_lab_public_key_path=Path(args.trusted_lab_public_key),
                    original_receipt_authority_key_id=(
                        args.original_receipt_authority_key_id
                    ),
                    original_receipt_authority_public_key_path=Path(
                        args.original_receipt_authority_public_key
                    ),
                    authority_key_id=args.authority_key_id,
                    authority_private_key_path=Path(args.authority_private_key),
                    authority_public_key_path=Path(args.authority_public_key),
                    maximum_risk_metric=args.maximum_risk_metric,
                    devicecheck_jwt_file=(
                        Path(args.devicecheck_jwt_file)
                        if args.devicecheck_jwt_file is not None
                        else None
                    ),
                    devicecheck_jwt_fd=args.devicecheck_jwt_fd,
                    output_path=receipt_output,
                    connect_timeout_seconds=args.connect_timeout_seconds,
                    total_timeout_seconds=args.total_timeout_seconds,
                )
                print(
                    "[kagemusha-app-attest-authority] exact catalog Apple status "
                    "revalidated for one promotion"
                )
            return 0
    except (
        AuthorityError,
        candidate_evidence.EvidenceError,
        OSError,
        sqlite3.Error,
    ) as error:
        print(f"[kagemusha-app-attest-authority] ERROR: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
