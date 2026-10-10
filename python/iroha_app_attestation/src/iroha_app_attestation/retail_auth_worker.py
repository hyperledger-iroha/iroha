"""Private Android first-device authentication verifier, separate from wallet E1.

No issuer key, account, wallet, credential, money or custody-readiness constructor is
present. Native admits the signed policy/runtime, drops UID, and passes descriptors
13 OAuth, 15 archive, 16 Python, 17 protected directory, 20 config, 21 OpenSSL.
Only Native's private length-framed channel reaches this worker. No public listener.
"""
from __future__ import annotations

import base64
import hashlib
import json
import os
import sqlite3
import stat
import subprocess
import sys
import tempfile
from contextlib import closing
from collections import deque
from pathlib import Path

from .attestation import (
    AttestationRejected, VerificationUnavailable, require, children, der_one,
    positive_integer, public_key_pem, validate_android_patch_floor,
    verify_android_wallet_payment_key_raw,
)
from .revocation import verify_google_chain_not_revoked
from .play_integrity import GooglePlayIntegrityVerifier, PlayIntegrityEnrollmentPolicy, MAX_TOKEN_BYTES, _unique
from .google_oauth import GoogleServiceAccountTokenProvider
from .native_time_interval import NativeTimeInterval

SCHEMA = "bpng.first-device-auth-verifier.v1"
CONFIG_SCHEMA = "bpng.first-device-auth-verifier-config.v1"
RAW_SCHEMA = "bpng.first-device-auth-raw.v1"
FINISH_SCHEMA = "bpng.first-device-auth-finish.v1"
CHALLENGE_DOMAIN = b"BPNG.FIRST_DEVICE.AUTH.CHALLENGE.V1\0"
CHAIN_DOMAIN = b"BPNG.FIRST_DEVICE.AUTH.CHAIN.V1\0"
POSSESSION_DOMAIN = b"BPNG.FIRST_DEVICE.AUTH.POSSESSION.V1\0"
INTEGRITY_DOMAIN = b"BPNG.FIRST_DEVICE.AUTH.INTEGRITY.V1\0"
CHALLENGE_BYTES = len(CHALLENGE_DOMAIN) + 7 * 32 + 16
MAX_PACKET, MAX_ORIGINAL, MAX_CONFIG = 768 * 1024, 360 * 1024, 128 * 1024
EXCHANGE_WINDOW = 4096
STORE_DIRECTORY = Path("/var/lib/bpng-taira/kagemusha/first-device-auth")
P256_ORDER = 0xffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551


def sha(value: bytes) -> bytes:
    return hashlib.sha256(value).digest()


def encode(value: dict) -> bytes:
    return json.dumps(value, separators=(",", ":"), sort_keys=True, allow_nan=False).encode("utf-8")


def exact_json(original: bytes, bound: int) -> dict:
    require(type(original) is bytes and 0 < len(original) <= bound, "packet_bound")
    try:
        value = json.loads(original.decode("utf-8"), object_pairs_hook=_unique,
                          parse_constant=lambda _: (_ for _ in ()).throw(ValueError()))
    except (ValueError, UnicodeError, RecursionError):
        raise AttestationRejected("packet_json") from None
    require(type(value) is dict, "packet_object")
    return value


def b64(value: object, maximum: int, exact: int | None = None) -> bytes:
    require(type(value) is str and 0 < len(value) <= 4 * ((maximum + 2) // 3), "binary_bound")
    try:
        raw = base64.b64decode(value, validate=True)
    except (ValueError, TypeError):
        raise AttestationRejected("binary_encoding") from None
    require(0 < len(raw) <= maximum and (exact is None or len(raw) == exact)
            and base64.b64encode(raw).decode("ascii") == value, "binary_canonical")
    return raw


def text(value: bytes) -> str:
    return base64.b64encode(value).decode("ascii")


def hex32(value: object) -> bytes:
    require(type(value) is str and len(value) == 64 and all(c in "0123456789abcdef" for c in value), "digest")
    raw = bytes.fromhex(value)
    require(any(raw), "digest_empty")
    return raw


def challenge(value: bytes) -> tuple[bytes, int, int]:
    require(type(value) is bytes and len(value) == CHALLENGE_BYTES
            and value.startswith(CHALLENGE_DOMAIN), "challenge_layout")
    fields = [value[len(CHALLENGE_DOMAIN) + n * 32:len(CHALLENGE_DOMAIN) + (n + 1) * 32] for n in range(7)]
    issued, expires = int.from_bytes(value[-16:-8], "little"), int.from_bytes(value[-8:], "little")
    require(all(any(field) for field in fields) and fields[2] != fields[3]
            and 0 < issued < expires and expires - issued <= 600_000, "challenge_fields")
    # Native separately compares policy_sha256 and its tighter signed finite lifetime.
    return sha(value), issued, expires


def chain_digest(chain: list[bytes]) -> bytes:
    require(type(chain) is list and 2 <= len(chain) <= 8
            and all(type(item) is bytes and 0 < len(item) <= 16384 for item in chain), "chain_bound")
    return sha(CHAIN_DOMAIN + len(chain).to_bytes(4, "little") +
               b"".join(len(item).to_bytes(4, "little") + item for item in chain))


def possession_message(transcript: bytes, chain: list[bytes], point: bytes) -> bytes:
    require(len(point) == 65 and point[0] == 4, "point_encoding")
    return POSSESSION_DOMAIN + challenge(transcript)[0] + chain_digest(chain) + point


def integrity_request_hash(transcript: bytes, raw: bytes, message: bytes, signature: bytes) -> bytes:
    require(all(type(item) is bytes and 0 < len(item) <= MAX_ORIGINAL
                for item in (transcript,raw,message,signature)), "integrity_frame_bound")
    return sha(INTEGRITY_DOMAIN + b"".join(len(item).to_bytes(4, "little") + item
                                         for item in (transcript, raw, message, signature)))


def verify_possession(openssl: Path, point: bytes, expected: bytes, offered: bytes, signature: bytes) -> None:
    require(offered == expected and 8 <= len(signature) <= 72, "possession_binding")
    integers = children(der_one(signature))
    require(len(integers) == 2 and all(0 < positive_integer(item) < P256_ORDER for item in integers),
            "possession_der")
    # Strict DER, including minimal INTEGER/length encoding, is checked above. No low-S
    # normalization: the exact platform DER original is retained and bound into requestHash.
    spki = bytes.fromhex("3059301306072a8648ce3d020106082a8648ce3d030107034200") + point
    with tempfile.TemporaryDirectory(prefix="bpng-auth-possession-") as temporary:
        directory = Path(temporary)
        (directory / "public.pem").write_bytes(public_key_pem(spki))
        (directory / "message").write_bytes(offered)
        (directory / "signature").write_bytes(signature)
        result = subprocess.run([str(openssl), "dgst", "-sha256", "-verify", str(directory / "public.pem"),
            "-signature", str(directory / "signature"), str(directory / "message")],
            stdin=subprocess.DEVNULL, capture_output=True, timeout=5, check=False)
        require(result.returncode == 0, "possession_signature")


class HeldPublicFile:
    """Local original identity check, never signed release admission."""
    def __init__(self, descriptor: int, digest: bytes, path: Path | None = None):
        self.fd, self.digest, self.path = descriptor, digest, path
        self.identity = self.metadata()
        self.recheck()

    def metadata(self):
        value = os.fstat(self.fd)
        require(stat.S_ISREG(value.st_mode) and value.st_uid in (0, os.getuid())
                and value.st_mode & 0o022 == 0 and value.st_nlink == 1, "public_custody")
        return (value.st_dev, value.st_ino, value.st_size, value.st_mtime_ns, value.st_ctime_ns,
                value.st_mode, value.st_uid, value.st_gid)

    def recheck(self):
        require(self.metadata() == self.identity and 0 < self.identity[2] <= 64 * 1024 * 1024, "public_changed")
        digest, offset = hashlib.sha256(), 0
        while offset < self.identity[2]:
            part = os.pread(self.fd, min(65536, self.identity[2] - offset), offset)
            require(bool(part), "public_truncated")
            digest.update(part); offset += len(part)
        require(digest.digest() == self.digest and self.metadata() == self.identity, "public_digest")
        if self.path is not None:
            require(self.path.is_absolute() and self.path.resolve(strict=True) == self.path, "public_path")
            value = self.path.stat()
            require((value.st_dev, value.st_ino, value.st_size, value.st_mtime_ns, value.st_ctime_ns,
                     value.st_mode, value.st_uid, value.st_gid) == self.identity, "public_replaced")


class AuthJournal:
    """Separate owner-only durable reserve/result store; no wallet or Apple-counter tables."""
    def __init__(self, directory: Path, descriptor: int):
        self.directory = directory
        self.fd = os.dup(descriptor)
        mode = os.fstat(self.fd)
        self.identity = (mode.st_dev, mode.st_ino)
        self.recheck()
        self.path = directory / "first-device-auth.sqlite3"
        try:
            created = os.open(self.path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        except FileExistsError:
            pass
        else:
            os.fsync(created); os.close(created); os.fsync(self.fd)
        self.file_identity = self._file_identity()
        with closing(self.connect()) as connection:
            connection.execute("""CREATE TABLE IF NOT EXISTS auth_attempts (
                phase TEXT NOT NULL CHECK(phase IN ('raw','finish')), challenge BLOB NOT NULL,
                request_sha256 BLOB NOT NULL, original BLOB NOT NULL, result BLOB,
                PRIMARY KEY(phase,challenge))""")
            columns = connection.execute("PRAGMA table_info(auth_attempts)").fetchall()
            require([(r[1], r[2], r[3], r[5]) for r in columns] == [
                ('phase','TEXT',1,1), ('challenge','BLOB',1,2), ('request_sha256','BLOB',1,0),
                ('original','BLOB',1,0), ('result','BLOB',0,0)], "journal_schema")
        os.fsync(self.fd)

    def recheck(self):
        require(self.directory.is_absolute() and self.directory.resolve(strict=True) == self.directory,
                "store_path")
        held, path = os.fstat(self.fd), self.directory.stat()
        require(stat.S_ISDIR(held.st_mode) and held.st_uid == os.getuid() and held.st_mode & 0o077 == 0
                and (held.st_dev, held.st_ino) == self.identity == (path.st_dev, path.st_ino), "store_custody")

    def recheck_database(self):
        self.recheck()
        require(self._file_identity() == self.file_identity, "journal_replaced")

    @staticmethod
    def request_binding(phase: str, digest: bytes, original: bytes):
        require(type(phase) is str and phase in ('raw', 'finish') and type(digest) is bytes
                and len(digest) == 32 and any(digest) and type(original) is bytes
                and 0 < len(original) <= MAX_ORIGINAL, "journal_request")

    def _file_identity(self):
        value = self.path.lstat()
        require(stat.S_ISREG(value.st_mode) and value.st_uid == os.getuid()
                and value.st_mode & 0o077 == 0 and value.st_nlink == 1, "journal_custody")
        return value.st_dev, value.st_ino

    def connect(self):
        self.recheck()
        require(self._file_identity() == self.file_identity, "journal_replaced")
        sidecar = self.path.with_name(self.path.name + "-journal")
        if sidecar.exists() or sidecar.is_symlink():
            value = sidecar.lstat()
            require(stat.S_ISREG(value.st_mode) and value.st_uid == os.getuid()
                    and value.st_mode & 0o077 == 0 and value.st_nlink == 1, "journal_sidecar")
        connection = sqlite3.connect(self.path, isolation_level=None, timeout=5)
        try:
            connection.execute("PRAGMA journal_mode=DELETE")
            connection.execute("PRAGMA synchronous=FULL")
            self.recheck_database()
            return connection
        except Exception:
            connection.close()
            raise

    def cached(self, phase: str, digest: bytes, original: bytes) -> bytes | None:
        self.request_binding(phase, digest, original)
        with closing(self.connect()) as connection:
            row = connection.execute("SELECT request_sha256,original,result FROM auth_attempts WHERE phase=? AND challenge=?",
                                     (phase, digest)).fetchone()
            self.recheck_database()
            if row is not None:
                require(row[0] == sha(original) and row[1] == original, "recovery_original")
            return None if row is None else row[2]

    def reserve(self, phase: str, digest: bytes, original: bytes):
        self.request_binding(phase, digest, original)
        with closing(self.connect()) as connection:
            connection.execute("BEGIN IMMEDIATE")
            try:
                require(connection.execute("SELECT 1 FROM auth_attempts WHERE phase=? AND challenge=?",
                        (phase, digest)).fetchone() is None, "attempt_consumed")
                connection.execute("INSERT INTO auth_attempts VALUES (?,?,?,?,NULL)",
                                   (phase, digest, sha(original), original))
                self.recheck_database()
                connection.execute("COMMIT")
            except Exception:
                connection.execute("ROLLBACK"); raise
        self.recheck_database()

    def retain(self, phase: str, digest: bytes, original: bytes, result: bytes):
        self.request_binding(phase, digest, original)
        require(type(result) is bytes and 0 < len(result) <= MAX_ORIGINAL, "journal_result")
        with closing(self.connect()) as connection:
            require(connection.execute("UPDATE auth_attempts SET result=? WHERE phase=? AND challenge=? "
                "AND request_sha256=? AND original=? AND result IS NULL",
                (result, phase, digest, sha(original), original)).rowcount == 1, "result_replaced")
        self.recheck_database()

    def retained_raw(self, digest: bytes, offered: bytes) -> dict:
        with closing(self.connect()) as connection:
            row = connection.execute("SELECT request_sha256,original,result FROM auth_attempts WHERE phase='raw' AND challenge=?",
                                     (digest,)).fetchone()
            require(row is not None and row[2] is not None and row[2] == offered, "raw_not_retained")
            result = exact_json(row[2], MAX_ORIGINAL)
            require(hex32(result.get('raw_request_sha256')) == row[0] == sha(row[1]), "raw_original_changed")
            request = exact_json(row[1], MAX_ORIGINAL)
            require(result.get('original_chain_base64') == request.get('certificate_chain_der_base64'), "raw_chain_changed")
            self.recheck_database()
            return result

    def close(self):
        os.close(self.fd)


class AuthVerifierOwner:
    def __init__(self, config_original: bytes, *, directory_fd: int = 17, crypto_fd: int = 21, oauth_fd: int = 13):
        config = exact_json(config_original, MAX_CONFIG)
        require(set(config) == {'schema','version','openssl_path','openssl_sha256','store_directory','policy'}
                and config['schema'] == CONFIG_SCHEMA and type(config['version']) is int and config['version'] == 1,
                "config_schema")
        self.config_digest = sha(config_original)
        self.openssl = Path(config['openssl_path'])
        self.crypto = HeldPublicFile(crypto_fd, hex32(config['openssl_sha256']), self.openssl)
        directory = Path(config['store_directory'])
        require(directory == STORE_DIRECTORY, "store_selection")
        policy = config['policy']
        require(type(policy) is dict and set(policy) == {'package_name','package_version','app_certificate_sha256',
            'root_base64','root_sha256','security_levels','patch_floor_yyyymm','google_policy_base64','google_policy_sha256',
            'maximum_evidence_age_ms','require_play_recognized','require_licensed','minimum_device_integrity'}, "policy_fields")
        self.pi_policy = PlayIntegrityEnrollmentPolicy(hex32(policy['google_policy_sha256']), policy['package_name'],
            policy['package_version'], hex32(policy['app_certificate_sha256']), policy['maximum_evidence_age_ms'],
            policy['require_play_recognized'], policy['require_licensed'], policy['minimum_device_integrity'])
        self.pi_policy.validate()
        self.root = b64(policy['root_base64'], 16384)
        self.root_digest = hex32(policy['root_sha256'])
        require(sha(self.root) == self.root_digest and type(policy['security_levels']) is list
                and all(type(n) is int for n in policy['security_levels'])
                and policy['security_levels'] in ([1], [2], [1,2]), "hardware_policy")
        self.security_levels = frozenset(policy['security_levels'])
        # Same policy semantics as the genuine generic verifier: no invented patch-floor rejection.
        validate_android_patch_floor(policy['patch_floor_yyyymm'])
        self.current_time = None
        self.oauth = GoogleServiceAccountTokenProvider(public_policy_original=b64(policy['google_policy_base64'],16384),
            native_policy=self.pi_policy, credential_fd=oauth_fd, trusted_time_interval=self._clock,
            openssl_path=self.openssl, credential_owner_uid=os.geteuid())
        try:
            self.google = GooglePlayIntegrityVerifier(self.oauth)
            self.journal = AuthJournal(directory, directory_fd)
        except Exception:
            self.oauth.close(); raise

    def _clock(self):
        require(type(self.current_time) is int, "trusted_time_absent")
        return NativeTimeInterval(self.current_time, self.current_time)

    def recheck(self):
        self.crypto.recheck(); self.journal.recheck()

    def parse_request(self, original: bytes, phase: str):
        value = exact_json(original, MAX_ORIGINAL)
        keys = {'phase','challenge_transcript_base64','trusted_time_ms'}
        keys |= {'certificate_chain_der_base64'} if phase == 'raw' else {
            'raw_verifier_original_base64','possession_message_base64','possession_signature_der_base64','play_integrity_token'}
        require(set(value) == keys and value['phase'] == phase, "request_fields")
        transcript = b64(value['challenge_transcript_base64'], CHALLENGE_BYTES, CHALLENGE_BYTES)
        digest, issued, expires = challenge(transcript)
        now = value['trusted_time_ms']
        require(type(now) is int and 0 < now < 1 << 64, "trusted_time")
        if phase == 'raw':
            chain = value['certificate_chain_der_base64']
            require(type(chain) is list and 2 <= len(chain) <= 8, "chain_count")
            for item in chain: b64(item,16384)
        else:
            b64(value['raw_verifier_original_base64'],MAX_ORIGINAL)
            b64(value['possession_message_base64'],1024)
            b64(value['possession_signature_der_base64'],72)
            token = value['play_integrity_token']
            require(type(token) is str and 0 < len(token) <= MAX_TOKEN_BYTES
                    and all(33 <= ord(c) <= 126 for c in token), "token_bound")
        return value, transcript, digest, issued, expires, now

    def perform(self, original: bytes, action: str) -> bytes | None:
        require(type(action) is str and action in ('verify-raw','recover-raw','verify-finish','recover-finish'), "action")
        phase = action.split('-')[1]
        self.recheck()
        value, transcript, digest, issued, expires, now = self.parse_request(original, phase)
        if action.startswith('recover-'):
            result = self.journal.cached(phase, digest, original)
            if result is not None:
                retained = exact_json(result,MAX_ORIGINAL)
                require(retained.get('schema') == (RAW_SCHEMA if phase == 'raw' else FINISH_SCHEMA)
                        and hex32(retained.get('config_sha256')) == self.config_digest
                        and hex32(retained.get('challenge_digest')) == digest, "recovery_binding")
            self.recheck()
            return result  # Never call a verifier, create a reservation or refresh capture time.
        require(issued <= now < expires, "capture_time")
        self.current_time = now
        # Durable reservation precedes revocation lookup, possession subprocess and Google decode.
        self.journal.reserve(phase, digest, original)
        result = self.raw(value, transcript, digest, now, original) if phase == 'raw' else self.finish(value, transcript, digest, now)
        self.recheck()
        require(len(result) <= MAX_ORIGINAL, "result_bound")
        self.journal.retain(phase, digest, original, result)
        self.recheck()
        return result

    def raw(self, value, transcript, digest, now, original):
        encoded_chain = value['certificate_chain_der_base64']
        require(type(encoded_chain) is list and 2 <= len(encoded_chain) <= 8, "chain_count")
        chain = [b64(item,16384) for item in encoded_chain]
        p = self.pi_policy
        raw = verify_android_wallet_payment_key_raw(chain, digest, p.package_name, p.package_version,
            p.app_signing_certificate_sha256, self.root, self.root_digest, now, self.openssl,
            allowed_security_levels=self.security_levels)
        self.crypto.recheck()
        verify_google_chain_not_revoked(chain)
        require(raw.platform == 'android_keymint' and raw.android_security_level in self.security_levels, "hardware_result")
        return encode({'schema':RAW_SCHEMA,'version':1,'config_sha256':self.config_digest.hex(),
            'challenge_digest':digest.hex(),'raw_request_sha256':sha(original).hex(),
            'app_public_key_sec1_base64':text(raw.attested_public_key_sec1),
            'security_level':raw.android_security_level,'checked_at_ms':now,'original_chain_base64':encoded_chain})

    def finish(self, value, transcript, digest, now):
        original_raw = b64(value['raw_verifier_original_base64'],MAX_ORIGINAL)
        raw = self.journal.retained_raw(digest, original_raw)
        require(raw.get('schema') == RAW_SCHEMA and type(raw.get('version')) is int and raw['version'] == 1
                and hex32(raw.get('config_sha256')) == self.config_digest
                and hex32(raw.get('challenge_digest')) == digest, "raw_binding")
        _, issued, expires = challenge(transcript)
        require(type(raw.get('checked_at_ms')) is int
                and issued <= raw['checked_at_ms'] <= now < expires, "raw_capture_time")
        chain = [b64(item,16384) for item in raw['original_chain_base64']]
        point = b64(raw['app_public_key_sec1_base64'],65,65)
        message = b64(value['possession_message_base64'],1024)
        signature = b64(value['possession_signature_der_base64'],72)
        expected = possession_message(transcript, chain, point)
        verify_possession(self.openssl, point, expected, message, signature)
        self.crypto.recheck()
        token = value['play_integrity_token']
        require(type(token) is str and 0 < len(token) <= MAX_TOKEN_BYTES
                and all(33 <= ord(c) <= 126 for c in token), "token_bound")
        request_hash = integrity_request_hash(transcript, original_raw, message, signature)
        decoded = self.google.decode(token, self.pi_policy, request_hash, now)
        return encode({'schema':FINISH_SCHEMA,'version':1,'config_sha256':self.config_digest.hex(),
            'challenge_digest':digest.hex(),'raw_verifier_original_sha256':sha(original_raw).hex(),
            'possession_message_sha256':sha(message).hex(),'possession_der_sha256':sha(signature).hex(),
            'token_original_sha256':sha(token.encode('ascii')).hex(),'integrity_request_hash':request_hash.hex(),
            'verified_at_ms':now,'google_response_original_base64':text(decoded.google_response)})

    def close(self):
        try: self.oauth.close()
        finally: self.journal.close()


def read_packet(stream):
    head = stream.read(4)
    if head == b'': return None
    while len(head) < 4:
        part = stream.read(4 - len(head))
        require(bool(part), "packet_header")
        head += part
    width = int.from_bytes(head,'little')
    require(0 < width <= MAX_PACKET, "packet_bound")
    parts, remaining = [], width
    while remaining:
        part = stream.read(remaining)
        require(bool(part), "packet_truncated")
        parts.append(part); remaining -= len(part)
    return b''.join(parts)


def serve(owner: AuthVerifierOwner, input_stream, output_stream, recheck_config=lambda: None):
    seen, recent = set(), deque()
    while (packet := read_packet(input_stream)) is not None:
        value = exact_json(packet,MAX_PACKET)
        require(set(value) == {'schema','version','exchange_id','action','original_base64'}
                and value['schema'] == SCHEMA and type(value['version']) is int and value['version'] == 1, "exchange_fields")
        exchange = hex32(value['exchange_id'])
        require(any(exchange) and exchange not in seen, "exchange_reused")
        seen.add(exchange)
        recent.append(exchange)
        if len(recent) > EXCHANGE_WINDOW:
            seen.remove(recent.popleft())
        # Transport tags are not authentication authority. Native owns this private
        # ordered pipe and checks every echoed tag and exact request digest. The
        # durable phase/challenge reservation, not this bounded recent-tag window,
        # prohibits external re-verification and binds recover to cached originals.
        original = b64(value['original_base64'],MAX_ORIGINAL)
        response = {'schema':SCHEMA,'version':1,'exchange_id':exchange.hex(),'original_sha256':sha(original).hex()}
        try:
            recheck_config()
            result = owner.perform(original,value['action'])
            recheck_config()
            response.update(outcome='result' if result is not None else 'outcome_unknown',
                            result_base64=None if result is None else text(result))
        except (VerificationUnavailable,sqlite3.Error,OSError,subprocess.SubprocessError):
            response.update(outcome='unavailable',result_base64=None)
        except (AttestationRejected,ValueError,TypeError,KeyError,OverflowError):
            response.update(outcome='rejected',result_base64=None)
        packet = encode(response)
        require(len(packet) <= MAX_PACKET, "response_bound")
        output_stream.write(len(packet).to_bytes(4,'little') + packet); output_stream.flush()


def main():
    require(sys.platform.startswith('linux') and len(sys.argv) == 1, "runtime")
    metadata = os.fstat(20)
    require(stat.S_ISREG(metadata.st_mode) and 0 < metadata.st_size <= MAX_CONFIG, "config_custody")
    original = os.pread(20,MAX_CONFIG+1,0)
    require(len(original) == metadata.st_size, "config_truncated")
    config = HeldPublicFile(20,sha(original))
    owner = AuthVerifierOwner(original)
    try: serve(owner,sys.stdin.buffer,sys.stdout.buffer,config.recheck)
    finally: owner.close()


if __name__ == '__main__':
    try: main()
    except Exception: raise SystemExit(1) from None
