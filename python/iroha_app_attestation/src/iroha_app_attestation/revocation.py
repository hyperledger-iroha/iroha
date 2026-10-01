"""Live Google KeyMint attestation-chain revocation check.

Only a caller that has already verified a chain against a pinned Google
attestation root should use this check. Other OEM roots need their own
governed revocation policy. A successful lookup is not an issuance token.
"""

from __future__ import annotations

import json
import re
import ssl
import urllib.error
import urllib.request
from datetime import date
from typing import Sequence

from .attestation import AttestationRejected, children, der_one, positive_integer, require


GOOGLE_STATUS_URL = "https://android.googleapis.com/attestation/status"
MAX_STATUS_BYTES = 2 * 1024 * 1024
MAX_ENTRIES = 50_000
MAX_CHAIN_CERTIFICATES = 8
_SERIAL = re.compile(r"[a-f1-9][a-f0-9]*\Z")
_REASONS = frozenset({
    "UNSPECIFIED", "KEY_COMPROMISE", "CA_COMPROMISE", "SUPERSEDED", "SOFTWARE_FLAW",
})


def _unique_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        require(key not in result, "duplicate Android revocation JSON member")
        result[key] = value
    return result


def parse_google_revocation_status(body: bytes) -> frozenset[int]:
    """Parse Google's documented status schema; every listed key is disallowed."""
    require(0 < len(body) <= MAX_STATUS_BYTES, "Android revocation status outside bound")
    try:
        value = json.loads(body.decode("utf-8"), object_pairs_hook=_unique_object)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise AttestationRejected("invalid Android revocation JSON") from error
    require(type(value) is dict and set(value) == {"entries"}
            and type(value["entries"]) is dict
            and len(value["entries"]) <= MAX_ENTRIES,
            "invalid Android revocation status envelope")
    result: set[int] = set()
    for serial, entry in value["entries"].items():
        require(type(serial) is str and _SERIAL.fullmatch(serial) is not None
                and len(serial) <= 40, "invalid Android revocation serial")
        require(type(entry) is dict and "status" in entry
                and set(entry) <= {"status", "expires", "reason", "comment"}
                and entry["status"] in ("REVOKED", "SUSPENDED"),
                "invalid Android revocation entry")
        if "expires" in entry:
            expires = entry["expires"]
            require(type(expires) is str and len(expires) == 10,
                    "invalid Android revocation expiry")
            try:
                require(date.fromisoformat(expires).isoformat() == expires,
                        "invalid Android revocation expiry")
            except ValueError as error:
                raise AttestationRejected("invalid Android revocation expiry") from error
        if "reason" in entry:
            require(type(entry["reason"]) is str and entry["reason"] in _REASONS,
                    "invalid Android revocation reason")
        if "comment" in entry:
            require(type(entry["comment"]) is str and len(entry["comment"]) <= 140,
                    "invalid Android revocation comment")
        result.add(int(serial, 16))
    return frozenset(result)


class _NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, msg, headers, newurl):
        return None


def fetch_google_revocation_status() -> frozenset[int]:
    """Fetch the exact Google HTTPS resource using system TLS roots, without redirects."""
    opener = urllib.request.build_opener(
        urllib.request.ProxyHandler({}),
        urllib.request.HTTPSHandler(context=ssl.create_default_context()),
        _NoRedirect(),
    )
    request = urllib.request.Request(GOOGLE_STATUS_URL, headers={"Accept": "application/json"})
    try:
        with opener.open(request, timeout=5) as response:
            require(response.status == 200 and response.geturl() == GOOGLE_STATUS_URL,
                    "Android revocation source changed")
            media_type = response.headers.get("Content-Type", "").split(";", 1)[0].strip().lower()
            require(media_type == "application/json"
                    and response.headers.get("Content-Encoding", "identity").lower() == "identity",
                    "invalid Android revocation response type")
            body = response.read(MAX_STATUS_BYTES + 1)
    except (OSError, urllib.error.URLError) as error:
        raise AttestationRejected("Android revocation status unavailable") from error
    return parse_google_revocation_status(body)


def certificate_serial(der: bytes) -> int:
    """Read the positive X.509 TBS serial number from bounded strict DER."""
    require(0 < len(der) <= 16 * 1024, "Android attestation certificate outside bound")
    certificate = children(der_one(der))
    require(len(certificate) == 3, "invalid Android attestation certificate")
    tbs = children(certificate[0])
    start = 1 if tbs and tbs[0].tag_class == 2 and tbs[0].number == 0 else 0
    require(len(tbs) >= start + 6, "invalid Android attestation TBS")
    serial = positive_integer(tbs[start])
    require(0 < serial < (1 << 160), "Android attestation serial outside bound")
    return serial


def verify_google_chain_not_revoked(chain_der: Sequence[bytes]) -> None:
    """Reject any listed leaf, intermediate or root serial in a pinned Google chain.

    This performs a live lookup on every call, so a network failure fails
    closed. Chain signature/root/time checks remain the caller's obligation.
    """
    require(2 <= len(chain_der) <= MAX_CHAIN_CERTIFICATES,
            "invalid Android attestation chain length")
    serials = [certificate_serial(cert) for cert in chain_der]
    listed = fetch_google_revocation_status()
    require(not listed.intersection(serials), "revoked Android attestation certificate")
