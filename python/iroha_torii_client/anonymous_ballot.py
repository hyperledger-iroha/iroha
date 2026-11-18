"""Credential-free HTTP transport of anonymous Parliament ballot frames and public records."""
from __future__ import annotations

import http.client
import ipaddress
import re
from urllib.parse import urlsplit

from .errors import _envelope_fields

__all__ = ["AnonymousBallotClientV1", "AnonymousBallotRefusal"]
_NORITO = "application/x-norito"
_ROUTE = "/v1/gov/anonymous-ballots"
_LOCAL_V4 = tuple(map(ipaddress.ip_network, ("10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16")))


class AnonymousBallotRefusal(Exception):
    """HTTP refusal, with a stable Torii code when present."""

    def __init__(self, status: int, code: str | None, retry_after: str | None):
        self.status = status
        self.code = code if code and re.fullmatch(r"[A-Za-z0-9_.:-]{1,128}", code) else None
        self.retry_after_seconds = int(retry_after) if retry_after and re.fullmatch(r"[0-9]{1,9}", retry_after) else None
        super().__init__(f"Anonymous ballot request refused (HTTP {status})")


def _selector(value: bytes | str, length: int) -> str:
    if isinstance(value, bytes) and len(value) == length:
        return value.hex()
    if isinstance(value, str) and re.fullmatch(f"[0-9a-f]{{{length * 2}}}", value):
        return value
    raise ValueError(f"Expected exactly {length} bytes or their lowercase hexadecimal encoding")


class AnonymousBallotClientV1:
    """Dedicated transport with no account, session, cookie store or automatic redirect."""

    def __init__(self, base_url: str, *, allow_local_http: bool = False, timeout: float = 15):
        url = urlsplit(base_url)
        if not url.hostname or url.username is not None or url.password is not None or "?" in base_url or "#" in base_url:
            raise ValueError("Expected a Torii base URL without credentials, query or fragment")
        try:
            ip = ipaddress.ip_address(url.hostname)
            local = ip.is_loopback or any(ip in block for block in _LOCAL_V4)
        except ValueError:
            local = url.hostname == "localhost"
        if url.scheme != "https" and not (url.scheme == "http" and allow_local_http and local):
            raise ValueError("Expected HTTPS or an explicitly permitted local HTTP origin")
        if not timeout > 0:
            raise ValueError("timeout must be positive")
        self._connection = http.client.HTTPSConnection if url.scheme == "https" else http.client.HTTPConnection
        self._host, self._port, self._path, self._timeout = url.hostname, url.port, url.path.rstrip("/") + _ROUTE, timeout

    def submit_anonymous_ballot(self, envelope: bytes) -> None:
        """Send one wallet-produced canonical frame. An empty 202 means the node handled it."""
        if not isinstance(envelope, bytes) or not 0 < len(envelope) <= 262_144:
            raise ValueError("Expected a canonical ballot frame of 1 to 262144 bytes")
        self._request("", envelope, 0)

    def get_anonymous_ballot_board(self, scope: bytes | str) -> bytes:
        """Fetch the whole canonical board for the 64-byte scope."""
        return self._request("/boards/" + _selector(scope, 64), None, 1_048_576)

    def get_anonymous_parliament_attempt(self, attempt: bytes | str) -> bytes:
        """Fetch the whole canonical attempt for the 32-byte identifier."""
        return self._request("/attempts/" + _selector(attempt, 32), None, 16_777_216)

    def _request(self, suffix: str, envelope: bytes | None, limit: int) -> bytes:
        connection = self._connection(self._host, self._port, timeout=self._timeout)
        try:
            headers = {"Accept": _NORITO} if envelope is None else {"Content-Type": _NORITO}
            connection.request("GET" if envelope is None else "POST", self._path + suffix, body=envelope, headers=headers)
            response = connection.getresponse()
            if not 200 <= response.status < 300:
                code = None
                if response.getheader("Content-Encoding", "identity").lower() == "identity":
                    try:
                        body = response.read(65_537)
                        if len(body) <= 65_536:
                            code = _envelope_fields(body)[0]
                    except (OSError, http.client.HTTPException, RecursionError):
                        pass
                raise AnonymousBallotRefusal(response.status, code, response.getheader("Retry-After"))
            if response.status != (200 if envelope is None else 202):
                raise ValueError("Unexpected anonymous ballot success status")
            if response.getheader("Content-Encoding", "identity").lower() != "identity":
                raise ValueError("Unexpected anonymous ballot content encoding")
            if envelope is None and response.getheader("Content-Type", "").lower() != _NORITO:
                raise ValueError("Expected the exact Norito media type")
            declared = response.getheader("Content-Length")
            if declared is not None and (not re.fullmatch(r"[0-9]{1,19}", declared) or int(declared) > limit):
                raise ValueError("Anonymous ballot response exceeds its bound")
            body = response.read(limit + 1)
            if len(body) > limit or (declared is not None and len(body) != int(declared)) or (envelope is None and not body):
                raise ValueError("Invalid anonymous ballot response length")
            return body
        finally:
            connection.close()
