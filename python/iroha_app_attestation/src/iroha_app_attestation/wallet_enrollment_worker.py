"""Private current E1 verifier. No listener, issuer key, credential or money grant.

Native launches the frozen archive/runtime and supplies its admitted operator configuration
only through inherited FD20. Requests travel on private stdin/stdout and carry opaque raw
platform evidence. FD13 is Google OAuth custody; FD17 retains the protected SQLite directory;
FD21 retains the configured public OpenSSL executable. No credential path or signing key is
accepted in a request. The parent must already authenticate all runtime/policy originals.

Before exposing E1, Core anchors this journal's incarnation and exact prepared operation in
its protected challenge record. Complete and recover can claim only that prepared original.
Claim commits before any external verification; a claimed row without a result stays unknown.
Fresh Core dispatch time governs a delayed first claim without rewriting its captured request.
Inspect reads only an exact retained outcome; it never claims a prepared operation, including
before E1 expires. The issuer uses it for passive recovery after the verification window.
"""
from __future__ import annotations

import base64
import hashlib
import json
import os
import sqlite3
import stat
import sys
import time
from contextlib import closing
from collections import deque
from pathlib import Path

from .attestation import (
    AttestationRejected, VerificationUnavailable,
    _verify_apple_assertion_with_hash, require, verify_apple_wallet_attestation_raw,
)
from .wallet_enrollment import (
    WalletEnrollmentScope, _digest,
    VerifiedWalletEnrollmentEvidence, verify_android_wallet_enrollment,
)
from .wallet_policy import (
    AndroidAppIdentityV1, AndroidEnrollmentPlatformV1, AppleAppIdentityV1,
    AppleEnrollmentPlatformV1, ConfiguredWalletEnrollmentPolicyV1,
    RegulatoryPolicyV1, WalletAppPolicyV1, WalletEnrollmentPolicyV1,
)
from .play_integrity import GooglePlayIntegrityVerifier, PlayIntegrityEnrollmentPolicy, _unique
from .google_oauth import GoogleServiceAccountTokenProvider
from .native_time_interval import NativeTimeInterval
from .wallet_enrollment_store import E1CounterStore

SCHEMA = "iroha.kagemusha.wallet-e1-verifier.v1"
CONFIG_SCHEMA = "iroha.kagemusha.wallet-e1-verifier-config.v1"
PREPARATION_SCHEMA = "iroha.kagemusha.wallet-e1-worker-preparation.v1"
MAX_PACKET = 768 * 1024
MAX_REQUEST = 320 * 1024
MAX_ORIGINAL = 360 * 1024
MAX_CONFIG = 128 * 1024
MAX_PREPARATION = 16 * 1024
EXCHANGE_WINDOW = 4096


def exact_json(original: bytes, bound: int) -> dict:
    require(type(original) is bytes and 0 < len(original) <= bound, "private packet outside bound")
    try:
        value = json.loads(original.decode("utf-8"), object_pairs_hook=_unique,
                           parse_constant=lambda _: (_ for _ in ()).throw(ValueError()))
    except (ValueError, UnicodeError, RecursionError):
        raise AttestationRejected("invalid private packet") from None
    require(type(value) is dict, "invalid private packet")
    return value


def encode(value: dict) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")


def b64(value: object, maximum: int, exact: int | None = None) -> bytes:
    require(type(value) is str and 0 < len(value) <= ((maximum + 2) // 3) * 4,
            "invalid private binary field")
    try:
        raw = base64.b64decode(value, validate=True)
    except (ValueError, TypeError):
        raise AttestationRejected("invalid private binary field") from None
    require(0 < len(raw) <= maximum and (exact is None or len(raw) == exact)
            and base64.b64encode(raw).decode("ascii") == value, "noncanonical private binary field")
    return raw


def hex32(value: object) -> bytes:
    require(type(value) is str and len(value) == 64
            and all(c in "0123456789abcdef" for c in value), "invalid private digest")
    return bytes.fromhex(value)


def read_packet(stream) -> bytes | None:
    head = b""
    while len(head) < 4:
        part = stream.read(4 - len(head))
        if not part:
            if not head:
                return None
            raise AttestationRejected("partial private packet header")
        head += part
    width = int.from_bytes(head, "little")
    require(0 < width <= MAX_PACKET, "private packet outside bound")
    parts, remaining = [], width
    while remaining:
        chunk = stream.read(remaining)
        require(bool(chunk), "partial private packet")
        parts.append(chunk)
        remaining -= len(chunk)
    return b"".join(parts)


def write_packet(stream, value: dict) -> None:
    original = encode(value)
    require(0 < len(original) <= MAX_PACKET, "private response outside bound")
    stream.write(len(original).to_bytes(4, "little") + original)
    stream.flush()


class HeldPublicFile:
    """Descriptor/path identity and complete bytes rechecked before and after each use.

    This local check is not release admission. Native must hold the independently signed
    installed runtime and operator originals; hashing an arbitrary file creates no authority.
    """
    def __init__(self, fd: int, expected: bytes, path: Path | None = None):
        self.fd, self.expected, self.path = fd, expected, path
        self.metadata = self._metadata()
        self.recheck()

    def _metadata(self):
        value = os.fstat(self.fd)
        require(stat.S_ISREG(value.st_mode) and value.st_uid in (0, os.getuid())
                and value.st_mode & 0o022 == 0 and value.st_nlink == 1,
                "private original custody unavailable")
        return (value.st_dev, value.st_ino, value.st_size, value.st_mtime_ns,
                value.st_ctime_ns, value.st_mode, value.st_uid, value.st_gid)

    def recheck(self):
        require(self._metadata() == self.metadata and 0 < self.metadata[2] <= 64 * 1024 * 1024,
                "private original changed")
        digest, offset = hashlib.sha256(), 0
        while offset < self.metadata[2]:
            chunk = os.pread(self.fd, min(65536, self.metadata[2] - offset), offset)
            require(bool(chunk), "private original truncated")
            digest.update(chunk)
            offset += len(chunk)
        require(digest.digest() == self.expected and self._metadata() == self.metadata,
                "private original differs")
        if self.path is not None:
            require(self.path.is_absolute() and self.path.resolve(strict=True) == self.path,
                    "invalid crypto path")
            value = self.path.stat()
            require((value.st_dev, value.st_ino, value.st_size, value.st_mtime_ns,
                     value.st_ctime_ns, value.st_mode, value.st_uid, value.st_gid) == self.metadata,
                    "crypto path differs from held original")


def configured_policy(value: dict, platform: str) -> ConfiguredWalletEnrollmentPolicyV1:
    """Exact private projection of authenticated current Model policy originals.

    Hashes establish consistency only. Native authenticates this complete configuration,
    selects the current scheme/asset/account and holds its independently signed inventory.
    """
    common = {"scheme_id_hex", "asset_digest_hex", "regulatory_policy", "challenge_lifetime_ms",
              "attestation_lease_lifetime_ms", "root_base64", "root_sha256"}
    android = {"package_name", "package_version", "app_certificate_sha256", "security_levels",
               "patch_floor_yyyymm", "google_policy_base64", "google_policy_sha256",
               "maximum_evidence_age_ms", "require_play_recognized", "require_licensed",
               "minimum_device_integrity"}
    require(type(value) is dict and set(value) == common | (android if platform == "android" else {"app_id"}),
            "invalid private platform policy")
    scheme, asset, pin = (hex32(value[k]) for k in ("scheme_id_hex", "asset_digest_hex", "root_sha256"))
    require(any(scheme) and any(asset) and any(pin), "empty private policy binding")
    root = b64(value["root_base64"], 16384)
    require(hashlib.sha256(root).digest() == pin, "private root differs from policy")
    regulator = value["regulatory_policy"]
    require(type(regulator) is dict and set(regulator) == {
        "permitted_controls", "blacklist_max_age_ms", "time_anchor_max_response_ms"}, "invalid regulator projection")
    selected_regulator = RegulatoryPolicyV1(**regulator)
    require(type(value["challenge_lifetime_ms"]) is int and 0 < value["challenge_lifetime_ms"] < (1 << 64),
            "invalid private challenge lifetime")
    if platform == "apple":
        identity = AppleAppIdentityV1(value["app_id"])
        selected = AppleEnrollmentPlatformV1(pin)
    else:
        identity = AndroidAppIdentityV1(value["package_name"], value["package_version"],
                                      hex32(value["app_certificate_sha256"]))
        require(type(value["security_levels"]) is list and all(type(n) is int for n in value["security_levels"])
                and value["security_levels"] in ([1], [2], [1, 2]), "invalid private hardware policy")
        require(type(value["minimum_device_integrity"]) is str and value["minimum_device_integrity"] in
                ("MEETS_DEVICE_INTEGRITY", "MEETS_STRONG_INTEGRITY"), "invalid Google device policy")
        selected = AndroidEnrollmentPlatformV1(pin, {(1,): 1, (2,): 2, (1, 2): 3}[tuple(value["security_levels"])], value["patch_floor_yyyymm"],
            value["maximum_evidence_age_ms"], value["require_play_recognized"], value["require_licensed"],
            1 if value["minimum_device_integrity"] == "MEETS_DEVICE_INTEGRITY" else 2)
    app = WalletAppPolicyV1(1, scheme, identity)
    enrollment = WalletEnrollmentPolicyV1(1, scheme, asset, app.policy_digest(), selected,
        selected_regulator, value["challenge_lifetime_ms"], value["attestation_lease_lifetime_ms"])
    enrollment.validate_for_app(app)
    return ConfiguredWalletEnrollmentPolicyV1(app, enrollment, root)


def _realtime_ns() -> int:
    return time.time_ns()


def _sleep_inclusive_ns() -> int:
    # This worker is a Linux-only private release executable. A clock excluding
    # suspend would let a sleeping process reuse an expired enrollment window.
    if not sys.platform.startswith("linux") or not hasattr(time, "CLOCK_BOOTTIME"):
        raise VerificationUnavailable("sleep-inclusive private clock unavailable")
    return time.clock_gettime_ns(time.CLOCK_BOOTTIME)


class DispatchClock:
    """Fresh worker clock anchored to the exact Core dispatch and Linux boottime.

    Neither a request field nor a caller callback supplies elapsed time. Realtime
    and the retained sleep-inclusive reference must both stay within the original
    challenge window. Clock loss or regression leaves the claimed result unknown.
    """
    def __init__(self, dispatch_ms: int, issued_ms: int, expires_ms: int):
        self.dispatch_ms, self.issued_ms, self.expires_ms = dispatch_ms, issued_ms, expires_ms
        self.elapsed_start = self.elapsed_last = _sleep_inclusive_ns()
        self.wall_start = self.wall_last = _realtime_ns()
        if self.wall_start // 1_000_000 < dispatch_ms:
            raise VerificationUnavailable("private clock predates Core dispatch")
        self.sample()

    def sample(self) -> NativeTimeInterval:
        elapsed, wall = _sleep_inclusive_ns(), _realtime_ns()
        if elapsed < self.elapsed_last or wall < self.wall_last:
            raise VerificationUnavailable("private clock regressed")
        self.elapsed_last, self.wall_last = elapsed, wall
        delta = elapsed - self.elapsed_start
        # Anchor elapsed duration to the actual worker sample, not the older
        # Core dispatch. Socket or queue latency must never extend this window.
        lower = max(wall, self.wall_start + delta) // 1_000_000
        upper = (max(wall, self.wall_start + delta) + 999_999) // 1_000_000
        interval = NativeTimeInterval(lower, upper)
        interval.require_window(self.issued_ms, self.expires_ms)
        return interval


class VerifierOwner:
    """Actual private process owner; its only successful result comes from real verifiers."""
    def __init__(self, config_original: bytes, *, directory_fd: int = 17,
                 crypto_fd: int = 21, oauth_fd: int = 13, configuration_fd: int | None = None):
        config = exact_json(config_original, MAX_CONFIG)
        require(set(config) == {"schema", "version", "platform", "app_policy_hex",
                                "enrollment_policy_hex", "openssl_path", "openssl_sha256",
                                "store_directory", "policy"}
                and config["schema"] == CONFIG_SCHEMA and type(config["version"]) is int
                and config["version"] == 1 and type(config["platform"]) is str
                and config["platform"] in ("android", "apple"), "invalid private configuration")
        self.config_digest = hashlib.sha256(config_original).digest()
        self.configuration = (None if configuration_fd is None else
                              HeldPublicFile(configuration_fd, self.config_digest))
        self.platform = config["platform"]
        self.app_policy, self.enrollment_policy = hex32(config["app_policy_hex"]), hex32(config["enrollment_policy_hex"])
        require(any(self.app_policy) and any(self.enrollment_policy), "empty private policy")
        self.openssl = Path(config["openssl_path"])
        self.crypto = HeldPublicFile(crypto_fd, hex32(config["openssl_sha256"]), self.openssl)
        self.policy = configured_policy(config["policy"], self.platform)
        require(self.policy.app.policy_digest() == self.app_policy
                and self.policy.enrollment.policy_digest() == self.enrollment_policy,
                "private policy projection differs from Native pins")
        self.directory = Path(config["store_directory"])
        self.dispatch_clock: DispatchClock | None = None
        self.google, self.oauth, self.counters = None, None, None
        self.directory_fd = os.dup(directory_fd)
        try:
            # Validate the protected directory before reading OAuth or opening retained storage.
            held, path = os.fstat(self.directory_fd), self.directory.stat()
            require(self.directory.is_absolute() and self.directory.resolve(strict=True) == self.directory
                    and stat.S_ISDIR(held.st_mode) and held.st_uid == os.getuid() and held.st_mode & 0o077 == 0
                    and (held.st_dev, held.st_ino) == (path.st_dev, path.st_ino),
                    "private store directory custody unavailable")
            if self.platform == "android":
                selected = self.policy.play_integrity_policy()
                # The separate governed decoder original has its own pin. Its app fields
                # must match the authenticated current policy; this is not an alias between
                # the decoder-policy digest and the Model enrollment-policy digest.
                pi = PlayIntegrityEnrollmentPolicy(hex32(config["policy"]["google_policy_sha256"]),
                    selected.package_name, selected.package_version, selected.app_signing_certificate_sha256,
                    selected.maximum_evidence_age_ms, selected.require_play_recognized,
                    selected.require_licensed, selected.minimum_device_integrity)
                self.oauth = GoogleServiceAccountTokenProvider(
                    public_policy_original=b64(config["policy"]["google_policy_base64"], 16384), native_policy=pi,
                    credential_fd=oauth_fd, trusted_time_interval=self._clock, openssl_path=self.openssl,
                    credential_owner_uid=os.geteuid())
                self.google = GooglePlayIntegrityVerifier(self.oauth)
            self.counters = E1CounterStore(self.directory, self.directory_fd)
            self.recheck()
        except Exception:
            if self.counters is not None:
                self.counters.close()
            if self.oauth is not None:
                self.oauth.close()
            os.close(self.directory_fd)
            self.directory_fd = -1
            raise

    def _clock(self):
        if self.dispatch_clock is None:
            raise VerificationUnavailable("private dispatch clock unavailable")
        return self.dispatch_clock.sample()

    def recheck(self):
        try:
            self.crypto.recheck()
            if self.configuration is not None:
                self.configuration.recheck()
            self.counters.recheck()
        except (AttestationRejected, OSError):
            # A lost or substituted process original is not a platform-evidence rejection.
            raise VerificationUnavailable("private verifier custody unavailable") from None

    def request(self, original: bytes):
        value = exact_json(original, MAX_REQUEST)
        require(set(value) == {"challenge_transcript_base64", "payment_key_base64", "issued_at_ms",
                               "expires_at_ms", "trusted_time_ms", "platform", "evidence"}
                and value["platform"] == self.platform, "invalid private E1 request")
        scope = WalletEnrollmentScope(b64(value["challenge_transcript_base64"], 194, 194),
                                      b64(value["payment_key_base64"], 65, 65))
        scope.validate()
        require(scope.challenge_transcript[98:130] == self.app_policy
                and scope.challenge_transcript[130:162] == self.enrollment_policy,
                "E1 differs from selected private policy")
        issued, expires, now = (value[k] for k in ("issued_at_ms", "expires_at_ms", "trusted_time_ms"))
        # The authenticated issuer may narrow its selected policy lifetime to its
        # session/proof deadline. Retain those exact endpoints; the worker adds no
        # independent ceiling and cannot extend the original half-open interval.
        require(all(type(t) is int for t in (issued, expires, now))
                and 0 < issued <= now < expires < 1 << 64,
                "private E1 time outside original window")
        require(type(value["evidence"]) is dict, "invalid private platform evidence")
        require(expires <= issued + self.policy.enrollment.challenge_lifetime_ms,
                "private E1 lifetime exceeds selected policy")
        self.policy.validate_scope(scope.challenge_transcript, issued, now)
        return value, scope

    def journal(self) -> bytes:
        self.recheck()
        with closing(self.counters._connect()) as connection:
            self.counters.require_incarnation(connection, self.counters.incarnation)
        self.recheck()
        return self.counters.incarnation

    def preparation(self, original: bytes):
        """Validate private DATA; canonical AccountId and Ed ownership remain Core checks."""
        value = exact_json(original, MAX_PREPARATION)
        require(set(value) == {"schema", "operation_id", "challenge_transcript_base64",
            "account_original_base64", "account_owner_public_hex", "issued_at_ms", "expires_at_ms",
            "platform", "app_policy_hex", "enrollment_policy_hex", "config_sha256"}
            and value["schema"] == PREPARATION_SCHEMA and value["platform"] == self.platform,
            "invalid private preparation")
        transcript = b64(value["challenge_transcript_base64"], 194, 194)
        require(transcript[:2] == b"\x01\0"
                and all(any(transcript[n:n + 32]) for n in range(2, 194, 32))
                and transcript[98:130] == self.app_policy
                and transcript[130:162] == self.enrollment_policy,
                "preparation differs from selected private policy")
        operation = hex32(value["operation_id"])
        require(operation == _digest(b"enrollment-challenge", transcript)
                and hex32(value["app_policy_hex"]) == self.app_policy
                and hex32(value["enrollment_policy_hex"]) == self.enrollment_policy
                and hex32(value["config_sha256"]) == self.config_digest,
                "private preparation binding differs")
        b64(value["account_original_base64"], 4096)
        require(any(hex32(value["account_owner_public_hex"])), "invalid private account owner")
        issued, expires = value["issued_at_ms"], value["expires_at_ms"]
        require(type(issued) is int and type(expires) is int and 0 < issued < expires < 1 << 64
                and expires <= issued + self.policy.enrollment.challenge_lifetime_ms,
                "invalid preparation window")
        self.policy.validate_scope(transcript, issued, issued)
        return value, operation

    def prepare(self, incarnation: bytes, original: bytes) -> None:
        self.recheck()
        _, operation = self.preparation(original)
        with closing(self.counters._connect()) as connection:
            connection.execute("BEGIN IMMEDIATE")
            try:
                self.counters.require_incarnation(connection, incarnation)
                row = connection.execute("SELECT preparation, config_sha256 FROM wallet_e1_attempts WHERE operation_id=?",
                                         (operation,)).fetchone()
                if row is None:
                    connection.execute("INSERT INTO wallet_e1_attempts (operation_id, preparation, config_sha256) VALUES (?, ?, ?)",
                                       (operation, original, self.config_digest))
                else:
                    require(row == (original, self.config_digest), "private preparation already differs")
                self.recheck()
                connection.execute("COMMIT")
            except Exception:
                if connection.in_transaction:
                    connection.execute("ROLLBACK")
                raise
        self.recheck()

    def projection(self, scope, evidence):
        # No success boolean, issuer output or transferable admission constructor.
        return {"config_sha256": self.config_digest.hex(),
            "challenge_digest": scope.challenge_digest().hex(),
            "key_binding": scope.enrollment_key_binding().hex(),
            "payment_key_base64": base64.b64encode(scope.payment_key_sec1).decode("ascii"),
            "kind_tag": evidence.kind_tag, "time_ms": evidence.time_ms, "facts": evidence.facts,
            "os_patch_level": evidence.os_patch_level, "vendor_patch_level": evidence.vendor_patch_level,
            "boot_patch_level": evidence.boot_patch_level, "evidence_digest": evidence.evidence_digest().hex(),
            "original_items_base64": [base64.b64encode(item).decode("ascii") for item in evidence.original_items],
            "app_attest_key_id": None if evidence.app_attest_key_id is None else evidence.app_attest_key_id.hex(),
            "app_attest_counter": evidence.app_attest_counter}

    def perform(self, original: bytes, action: str, *, incarnation: bytes,
                preparation: bytes, account_signature: bytes, dispatch_time_ms: int) -> bytes | None:
        require(action in ("complete", "recover", "inspect"), "invalid private operation")
        self.recheck()
        prepared, operation = self.preparation(preparation)
        value, scope = self.request(original)
        require(all(value[name] == prepared[name] for name in (
            "challenge_transcript_base64", "platform", "issued_at_ms", "expires_at_ms"))
            and scope.challenge_digest() == operation, "private request differs from preparation")
        require(type(account_signature) is bytes and len(account_signature) == 64,
                "invalid retained account signature")
        require(type(dispatch_time_ms) is int and value["trusted_time_ms"] <= dispatch_time_ms < 1 << 64,
                "invalid private dispatch time")
        digest = hashlib.sha256(original).digest()
        with closing(self.counters._connect()) as connection:
            connection.execute("BEGIN IMMEDIATE")
            try:
                self.counters.require_incarnation(connection, incarnation)
                row = connection.execute("""SELECT preparation, config_sha256, request_sha256,
                    key_binding, original, account_signature, result
                    FROM wallet_e1_attempts WHERE operation_id=?""", (operation,)).fetchone()
                # Missing custody cannot establish a definitive evidence rejection or
                # authorize a new preparation: this row may have been claimed before loss.
                if row is None:
                    raise VerificationUnavailable("private prepared original unavailable")
                require(row[:2] == (preparation, self.config_digest),
                        "private prepared original differs")
                if row[2] is not None:
                    require(row[2:6] == (digest, scope.enrollment_key_binding(), original, account_signature),
                            "private recovery scope differs")
                    self.recheck()
                    connection.execute("COMMIT")
                    return row[6]  # Claimed NULL is unknown; no action repeats verification.
                require(all(item is None for item in row[2:]), "invalid prepared journal state")
                if action == "inspect":
                    raise VerificationUnavailable("private operation has no claimed outcome")
                require(dispatch_time_ms < value["expires_at_ms"], "private first dispatch expired")
                clock = DispatchClock(dispatch_time_ms, value["issued_at_ms"], value["expires_at_ms"])
                changed = connection.execute("""UPDATE wallet_e1_attempts SET request_sha256=?,
                    key_binding=?, original=?, account_signature=?
                    WHERE operation_id=? AND request_sha256 IS NULL""",
                    (digest, scope.enrollment_key_binding(), original, account_signature, operation)).rowcount
                require(changed == 1, "private first dispatch claim changed")
                self.recheck()
                connection.execute("COMMIT")
            except Exception:
                if connection.in_transaction:
                    connection.execute("ROLLBACK")
                raise
        # Only the transaction winner reaches external verification. Its fresh trusted Core
        # time is separate from the immutable captured request and becomes evidence time.
        self.dispatch_clock = clock
        self._clock()
        offered = value["evidence"]
        if self.platform == "android":
            require(set(offered) == {"chain_base64", "play_integrity_token"}
                    and type(offered["chain_base64"]) is list
                    and 2 <= len(offered["chain_base64"]) <= 8
                    and type(offered["play_integrity_token"]) is str,
                    "invalid private Android evidence")
            chain = [b64(item, 16384) for item in offered["chain_base64"]]
            evidence = verify_android_wallet_enrollment(chain, offered["play_integrity_token"],
                scope, self.policy, self.google, dispatch_time_ms, self.openssl,
                challenge_created_at_ms=value["issued_at_ms"], trusted_time_interval=self._clock)
            result = encode(self.projection(scope, evidence))
            require(0 < len(result) <= MAX_ORIGINAL, "private evidence result outside bound")
            self.recheck()
            with closing(self.counters._connect()) as connection:
                connection.execute("BEGIN IMMEDIATE")
                try:
                    self.counters.require_incarnation(connection, incarnation)
                    self._clock()
                    changed = connection.execute("UPDATE wallet_e1_attempts SET result=? WHERE request_sha256=? AND result IS NULL",
                                                 (result, digest)).rowcount
                    require(changed == 1, "private result changed concurrently")
                    self._clock()
                    connection.execute("COMMIT")
                except Exception:
                    if connection.in_transaction:
                        connection.execute("ROLLBACK")
                    raise
        else:
            require(set(offered) == {"attestation_base64", "assertion_base64", "key_id_hex"},
                    "invalid private Apple evidence")
            attestation = b64(offered["attestation_base64"], 65536)
            assertion = b64(offered["assertion_base64"], 4096)
            key_id = hex32(offered["key_id_hex"])
            verification_time = self._clock().upper_at_ms
            raw = verify_apple_wallet_attestation_raw(attestation, key_id, self.policy.app.identity.app_id,
                "production", scope.challenge_digest(), self.policy.attestation_root_der,
                self.policy.enrollment.platform.attestation_root_sha256, verification_time, self.openssl)
            self._clock()
            self.counters.register_verified_key(raw, self.policy.app.identity.app_id, "production")
            # Counter/challenge consumption and the exact recoverable verified original are
            # one FULL-synchronous transaction in the existing counter database.
            with closing(self.counters._connect()) as connection:
                connection.execute("BEGIN IMMEDIATE")
                try:
                    self.counters.require_incarnation(connection, incarnation)
                    row = connection.execute("SELECT point, app_id, environment, counter FROM apple_keys WHERE key_id=?",
                                             (key_id,)).fetchone()
                    require(row is not None and row[1:] and row[1] == self.policy.app.identity.app_id
                            and row[2] == "production", "private Apple key scope differs")
                    self._clock()
                    checked = _verify_apple_assertion_with_hash(assertion, scope.enrollment_key_binding(),
                        row[0], key_id, self.policy.app.identity.app_id, row[3], self.openssl,
                        expected_validation_category=None, expected_bundle_version=None)
                    require(connection.execute("SELECT 1 FROM apple_client_data WHERE client_data_sha256=?",
                                               (checked.client_data_sha256,)).fetchone() is None,
                            "private Apple challenge already consumed")
                    evidence = VerifiedWalletEnrollmentEvidence(3, dispatch_time_ms,
                        (1 << 6) | (1 << 7) | (1 << 8), 0, 0, 0,
                        (attestation, assertion), key_id, checked.counter)
                    result = encode(self.projection(scope, evidence))
                    require(0 < len(result) <= MAX_ORIGINAL, "private evidence result outside bound")
                    self.recheck()
                    self._clock()
                    connection.execute("INSERT INTO apple_client_data VALUES (?, ?)",
                                       (checked.client_data_sha256, key_id))
                    require(connection.execute("UPDATE apple_keys SET counter=? WHERE key_id=? AND counter=?",
                                               (checked.counter, key_id, row[3])).rowcount == 1,
                            "private Apple counter changed concurrently")
                    require(connection.execute("UPDATE wallet_e1_attempts SET result=? WHERE request_sha256=? AND result IS NULL",
                                               (result, digest)).rowcount == 1, "private result changed concurrently")
                    self._clock()
                    connection.execute("COMMIT")
                except Exception:
                    if connection.in_transaction:
                        connection.execute("ROLLBACK")
                    raise
        self.recheck()
        self._clock()
        return result

    def close(self):
        if self.counters is not None:
            self.counters.close()
        if self.oauth is not None:
            self.oauth.close()
        if self.directory_fd >= 0:
            os.close(self.directory_fd)
            self.directory_fd = -1


def serve(owner: VerifierOwner, input_stream, output_stream):
    seen, recent = set(), deque()
    while (packet := read_packet(input_stream)) is not None:
        value = exact_json(packet, MAX_PACKET)
        require(set(value) == {"schema", "version", "exchange_id", "action", "journal_incarnation",
                              "preparation_base64", "original_base64", "account_signature_base64",
                              "dispatch_time_ms"}
                and value["schema"] == SCHEMA and type(value["version"]) is int
                and value["version"] == 1, "invalid private exchange")
        exchange = hex32(value["exchange_id"])
        require(any(exchange) and exchange not in seen, "private exchange reused")
        seen.add(exchange)
        recent.append(exchange)
        if len(recent) > EXCHANGE_WINDOW:
            seen.remove(recent.popleft())
        response = {"schema": SCHEMA, "version": 1, "exchange_id": exchange.hex(),
                    "request_sha256": hashlib.sha256(packet).hexdigest(),
                    "journal_incarnation": owner.counters.incarnation.hex(),
                    "config_sha256": owner.config_digest.hex()}
        try:
            action = value["action"]
            if action == "journal":
                require(all(value[name] is None for name in ("journal_incarnation", "preparation_base64",
                    "original_base64", "account_signature_base64", "dispatch_time_ms")),
                    "journal exchange carries operation inputs")
                response["journal_incarnation"] = owner.journal().hex()
                response.update({"outcome": "journal", "evidence_base64": None})
            else:
                incarnation = hex32(value["journal_incarnation"])
                preparation = b64(value["preparation_base64"], MAX_PREPARATION)
                if action == "prepare":
                    require(all(value[name] is None for name in (
                        "original_base64", "account_signature_base64", "dispatch_time_ms")),
                        "preparation exchange carries dispatch inputs")
                    owner.prepare(incarnation, preparation)
                    response.update({"outcome": "prepared", "evidence_base64": None})
                else:
                    result = owner.perform(b64(value["original_base64"], MAX_REQUEST), action,
                        incarnation=incarnation, preparation=preparation,
                        account_signature=b64(value["account_signature_base64"], 64, 64),
                        dispatch_time_ms=value["dispatch_time_ms"])
                    response.update({"outcome": "evidence" if result is not None else "outcome_unknown",
                        "evidence_base64": None if result is None else base64.b64encode(result).decode("ascii")})
        except VerificationUnavailable:
            response.update({"outcome": "unavailable", "evidence_base64": None})
        except (AttestationRejected, sqlite3.IntegrityError):
            response.update({"outcome": "rejected", "evidence_base64": None})
        except (sqlite3.Error, OSError):
            response.update({"outcome": "unavailable", "evidence_base64": None})
        write_packet(output_stream, response)


def main():
    # No environment/provider/argument selector and no public authority intake.
    require(sys.platform.startswith("linux") and len(sys.argv) == 1, "private Linux runtime required")
    metadata = os.fstat(20)
    require(stat.S_ISREG(metadata.st_mode) and metadata.st_uid in (0, os.getuid())
            and metadata.st_mode & 0o022 == 0 and metadata.st_nlink == 1
            and 0 < metadata.st_size <= MAX_CONFIG,
            "private configuration custody unavailable")
    original = os.pread(20, MAX_CONFIG + 1, 0)
    require(len(original) == metadata.st_size, "private configuration truncated")
    owner = VerifierOwner(original, configuration_fd=20)
    try:
        serve(owner, sys.stdin.buffer, sys.stdout.buffer)
    finally:
        owner.close()


if __name__ == "__main__":
    try:
        main()
    except Exception:
        # Do not put raw evidence, OAuth secrets or source exception text on inherited stderr.
        raise SystemExit(1) from None
