"""Google decoder OAuth with a governed public original and inherited custody FD.

The deployment must authenticate ``PlayIntegrityPolicy.policy_digest`` through
the Native release before constructing this adapter. Hashing a public policy
does not itself authenticate a release. Neither policy nor credential is a
mobile request field. No private credential is written to a temporary file.

https://developers.google.com/identity/protocols/oauth2/service-account
"""
from __future__ import annotations

import base64
import hashlib
import json
import os
import re
import ssl
import stat
import threading
import urllib.parse
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

from .native_time_interval import NativeTimeInterval
from .attestation import (AttestationRejected, VerificationUnavailable, children, der_one,
                          oid, positive_integer, primitive, require)
from .play_integrity import PlayIntegrityPolicy, PlayIntegrityUnavailable, _NoRedirect, _unique
from .openssl_private_rsa import acquire_crypto_originals, private_rsa_operation

POLICY_SCHEMA = "iroha.kagemusha.play-integrity-verification-policy.v1"
OAUTH_SCOPE = "https://www.googleapis.com/auth/playintegrity"
TOKEN_URI = "https://oauth2.googleapis.com/token"
MAX_POLICY_BYTES = 16 * 1024
MAX_CREDENTIAL_BYTES = 32 * 1024
MAX_TOKEN_RESPONSE_BYTES = 16 * 1024
_PROJECT = re.compile(r"[a-z][a-z0-9-]{4,28}[a-z0-9]\Z")
_DECIMAL = re.compile(r"[1-9][0-9]{0,39}\Z")
_KEY_ID = re.compile(r"[0-9a-f]{40}\Z")
_HEX32 = re.compile(r"[0-9a-f]{64}\Z")


def _json(original: bytes, bound: int, label: str) -> dict:
    require(type(original) is bytes and 0 < len(original) <= bound,
            f"{label} outside bound")
    try:
        value = json.loads(original.decode("utf-8"), object_pairs_hook=_unique,
                           parse_constant=lambda _: (_ for _ in ()).throw(ValueError()))
    except (ValueError, UnicodeError, RecursionError):
        # Never include source text or a JSON error carrying credential content.
        raise AttestationRejected(f"invalid {label}") from None
    require(type(value) is dict, f"invalid {label}")
    return value


@dataclass(frozen=True)
class GoogleDecoderSelection:
    """Public projection admitted by the independently authenticated Native pin."""
    project_id: str
    project_number: int
    service_account_email: str
    service_account_client_id: str


def select_google_decoder(public_original: bytes,
                          native_policy: PlayIntegrityPolicy) -> GoogleDecoderSelection:
    native_policy.validate()
    require(type(public_original) is bytes and 0 < len(public_original) <= MAX_POLICY_BYTES,
            "Google decoder policy outside bound")
    require(hashlib.sha256(public_original).digest() == native_policy.policy_digest,
            "Google decoder policy differs from Native original pin")
    value = _json(public_original, MAX_POLICY_BYTES, "Google decoder policy")
    require(set(value) == {"schema", "version", "cloudProject", "packageName",
                          "packageVersion", "appSigningCertificateSha256Hex", "credentialSubject"}
            and value["schema"] == POLICY_SCHEMA
            and type(value["version"]) is int and value["version"] == 1,
            "invalid Google decoder policy layout")
    project, subject = value["cloudProject"], value["credentialSubject"]
    require(type(project) is dict and set(project) == {"id", "number"}
            and type(project["id"]) is str and _PROJECT.fullmatch(project["id"])
            and type(project["number"]) is int and 0 < project["number"] < (1 << 64),
            "invalid Google decoder project")
    require(type(subject) is dict and set(subject) == {"email", "clientId"}
            and type(subject["email"]) is str
            and re.fullmatch(r"[a-z][a-z0-9-]{4,28}[a-z0-9]@" + re.escape(project["id"])
                             + r"\.iam\.gserviceaccount\.com", subject["email"])
            and type(subject["clientId"]) is str and _DECIMAL.fullmatch(subject["clientId"]),
            "invalid Google decoder principal")
    require(value["packageName"] == native_policy.package_name
            and type(value["packageVersion"]) is int and value["packageVersion"] > 0
            and value["packageVersion"] == native_policy.package_version
            and type(value["appSigningCertificateSha256Hex"]) is str
            and _HEX32.fullmatch(value["appSigningCertificateSha256Hex"])
            and value["appSigningCertificateSha256Hex"] == native_policy.app_signing_certificate_sha256.hex(),
            "Google decoder app differs from Native policy")
    return GoogleDecoderSelection(project["id"], project["number"],
                                  subject["email"], subject["clientId"])


def _b64url(value: bytes) -> str:
    return base64.urlsafe_b64encode(value).rstrip(b"=").decode("ascii")


def _key_command(private_pem: bytes, openssl: Path, arguments: list[str],
                 message: bytes = b"") -> bytes:
    """Keep private PEM in this process; OpenSSL CLI is public-only."""
    require(openssl.is_absolute() and openssl.is_file(), "Google OAuth crypto environment absent")
    require(arguments in (["pkey", "-pubout", "-outform", "DER", "-in"],
                          ["dgst", "-sha256", "-sign"]),
            "Google OAuth signing purpose differs")
    return private_rsa_operation(private_pem, public_only=arguments[0] == 'pkey', message=message)


def _rsa_bytes(private_pem: bytes, openssl: Path) -> int:
    original = _key_command(private_pem, openssl, ["pkey", "-pubout", "-outform", "DER", "-in"])
    spki = children(der_one(original))
    require(len(spki) == 2, "Google OAuth key must be RSA")
    algorithm = children(spki[0])
    require(len(algorithm) == 2 and oid(algorithm[0]) == "1.2.840.113549.1.1.1"
            and primitive(algorithm[1], 5) == b"", "Google OAuth key must be RSA")
    bits = primitive(spki[1], 3)
    require(bits[:1] == b"\0", "invalid Google OAuth RSA public key")
    values = children(der_one(bits[1:]))
    require(len(values) == 2, "invalid Google OAuth RSA public key")
    modulus, exponent = map(positive_integer, values)
    require(2048 <= modulus.bit_length() <= 4096 and exponent >= 3 and exponent % 2 == 1,
            "Google OAuth RSA key is outside allowed strength")
    return (modulus.bit_length() + 7) // 8


class GoogleServiceAccountTokenProvider:
    """Production OAuth source with FD custody and only the playintegrity scope.

    The caller owns the admitted public policy, credential descriptor, trusted
    clock and reviewed OpenSSL executable. Actual Root-owned loaded TLS/crypto
    originals are held before duplicating the credential FD and rechecked before
    every private use and cached-token reuse. This does not admit a Native runtime
    policy or construct issuer authority.
    Use ``close`` at deployment shutdown. No credential path is accepted.
    """
    def __init__(self, *, public_policy_original: bytes, native_policy: PlayIntegrityPolicy,
                 credential_fd: int, trusted_time_interval: Callable[[], NativeTimeInterval],
                 openssl_path: Path, credential_owner_uid: int | None = None) -> None:
        self.selection = select_google_decoder(public_policy_original, native_policy)
        self._owner_uid = os.getuid() if credential_owner_uid is None else credential_owner_uid
        require(type(self._owner_uid) is int and self._owner_uid in (0,os.getuid())
                and type(credential_fd) is int and credential_fd >= 3
                and callable(trusted_time_interval) and openssl_path.is_absolute()
                and openssl_path.is_file(), "Google OAuth custody absent")
        # Code custody precedes even duplication/parsing of private credential
        # bytes. It is a local original check, not Native release admission.
        self._crypto_code=acquire_crypto_originals()
        try:self._fd = os.dup(credential_fd)
        except Exception:
            self._crypto_code.close();raise
        self._clock = trusted_time_interval
        self._openssl = openssl_path
        self._lock = threading.Lock()
        self._access: str | None = None
        self._issued = 0
        self._refresh = 0
        try:
            self._metadata = self._stat()
            original = self._read()
            self._credential_digest = hashlib.sha256(original).digest()
            credential = self._credential(original)
            self._rsa_size = _rsa_bytes(credential["private_key"].encode("ascii"), self._openssl)
        except Exception:
            os.close(self._fd)
            self._fd = -1
            self._crypto_code.close()
            raise

    def _stat(self) -> tuple:
        value = os.fstat(self._fd)
        require(stat.S_ISREG(value.st_mode) and value.st_uid == self._owner_uid
                and value.st_mode & 0o077 == 0 and 0 < value.st_size <= MAX_CREDENTIAL_BYTES,
                "Google OAuth credential must be an owner-only regular original")
        return (value.st_dev, value.st_ino, value.st_size, value.st_mode,
                value.st_uid, value.st_mtime_ns, value.st_ctime_ns)

    def _read(self) -> bytes:
        self._crypto_code.recheck()
        require(self._stat() == self._metadata, "Google OAuth credential original changed")
        original = os.pread(self._fd, MAX_CREDENTIAL_BYTES + 1, 0)
        require(len(original) == self._metadata[2] and self._stat() == self._metadata,
                "Google OAuth credential original changed")
        return original

    def _credential(self, original: bytes) -> dict:
        value = _json(original, MAX_CREDENTIAL_BYTES, "Google OAuth credential")
        required = {"type", "project_id", "private_key_id", "private_key", "client_email",
                    "client_id", "auth_uri", "token_uri", "auth_provider_x509_cert_url",
                    "client_x509_cert_url"}
        require(required <= set(value) <= required | {"universe_domain"}
                and value["type"] == "service_account"
                and value["project_id"] == self.selection.project_id
                and value["client_email"] == self.selection.service_account_email
                and value["client_id"] == self.selection.service_account_client_id
                and value["token_uri"] == TOKEN_URI
                and value["auth_uri"] == "https://accounts.google.com/o/oauth2/auth"
                and value["auth_provider_x509_cert_url"] == "https://www.googleapis.com/oauth2/v1/certs"
                and value.get("universe_domain", "googleapis.com") == "googleapis.com"
                and type(value["private_key_id"]) is str and _KEY_ID.fullmatch(value["private_key_id"])
                and type(value["private_key"]) is str and len(value["private_key"]) <= 8192
                and value["private_key"].startswith("-----BEGIN PRIVATE KEY-----\n")
                and value["private_key"].endswith("-----END PRIVATE KEY-----\n")
                and value["private_key"].isascii(), "Google OAuth credential differs from governed principal")
        require(value["client_x509_cert_url"] == "https://www.googleapis.com/robot/v1/metadata/x509/"
                + urllib.parse.quote(self.selection.service_account_email, safe=""),
                "Google OAuth credential certificate source differs")
        return value

    def __call__(self) -> str:
        with self._lock:
            self._crypto_code.recheck()
            require(self._fd >= 3, "Google OAuth custody closed")
            original = self._read()
            require(hashlib.sha256(original).digest() == self._credential_digest,
                    "Google OAuth credential original changed")
            credential = self._credential(original)
            interval = self._clock()
            require(type(interval) is NativeTimeInterval, "Google OAuth Native interval absent")
            interval.validate()
            now_ms = interval.lower_at_ms
            now = now_ms // 1000
            upper = interval.upper_at_ms // 1000
            if self._access is not None and self._issued <= now <= upper < self._refresh:
                return self._access
            self._access = None
            header = {"alg": "RS256", "typ": "JWT", "kid": credential["private_key_id"]}
            claims = {"iss": self.selection.service_account_email, "scope": OAUTH_SCOPE,
                      "aud": TOKEN_URI, "iat": now, "exp": now + 300}
            message = (_b64url(json.dumps(header, separators=(",", ":")).encode("ascii"))
                        + "." + _b64url(json.dumps(claims, separators=(",", ":")).encode("ascii"))).encode("ascii")
            signature = _key_command(credential["private_key"].encode("ascii"), self._openssl,
                                     ["dgst", "-sha256", "-sign"], message)
            require(len(signature) == self._rsa_size, "Google OAuth RSA signature differs")
            assertion = message.decode("ascii") + "." + _b64url(signature)
            request = urllib.request.Request(TOKEN_URI,
                data=urllib.parse.urlencode({"grant_type": "urn:ietf:params:oauth:grant-type:jwt-bearer",
                                            "assertion": assertion}).encode("ascii"),
                headers={"Content-Type": "application/x-www-form-urlencoded", "Accept": "application/json"},
                method="POST")
            try:
                self._crypto_code.recheck()
                opener = urllib.request.build_opener(urllib.request.ProxyHandler({}),
                    urllib.request.HTTPSHandler(context=ssl.create_default_context()), _NoRedirect())
                with opener.open(request, timeout=5) as response:
                    require(response.status == 200 and response.geturl() == TOKEN_URI
                            and response.headers.get("Content-Type", "").split(";", 1)[0].strip().lower() == "application/json"
                            and response.headers.get("Content-Encoding", "identity").lower() == "identity",
                            "invalid Google OAuth response source")
                    body = response.read(MAX_TOKEN_RESPONSE_BYTES + 1)
                value = _json(body, MAX_TOKEN_RESPONSE_BYTES, "Google OAuth response")
                require({"access_token", "token_type", "expires_in"} <= set(value)
                        <= {"access_token", "token_type", "expires_in", "scope"}
                        and value["token_type"] == "Bearer"
                        and type(value["access_token"]) is str and 0 < len(value["access_token"]) <= 8192
                        and all(33 <= ord(char) <= 126 for char in value["access_token"])
                        and type(value["expires_in"]) is int and 60 < value["expires_in"] <= 3600
                        and value.get("scope", OAUTH_SCOPE) == OAUTH_SCOPE,
                        "invalid Google OAuth access token")
                # Network wait may cross clock boundaries; never extend expiry
                # from the response time or reuse a token after clock rollback.
                after = self._clock()
                require(type(after) is NativeTimeInterval, "Google OAuth Native interval absent")
                after.validate()
                require(now_ms <= after.lower_at_ms
                        and after.upper_at_ms // 1000 < now + value["expires_in"] - 60,
                        "Google OAuth trusted time changed")
                require(self._stat() == self._metadata
                        and hashlib.sha256(self._read()).digest() == self._credential_digest,
                        "Google OAuth credential original changed")
                self._crypto_code.recheck()
                self._access, self._issued = value["access_token"], now
                self._refresh = now + value["expires_in"] - 60
                return self._access
            except VerificationUnavailable:
                raise
            except AttestationRejected as error:
                # A changed Google response, trusted clock or held original
                # is an operational fault, never a verdict on mobile evidence.
                raise PlayIntegrityUnavailable(str(error)) from None
            except Exception:
                raise PlayIntegrityUnavailable("Google OAuth token exchange unavailable") from None

    def close(self) -> None:
        with self._lock:
            if self._fd >= 0:
                os.close(self._fd)
                self._fd = -1
            self._access = None
            self._crypto_code.close()
