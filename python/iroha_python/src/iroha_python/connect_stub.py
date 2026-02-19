"""Lightweight fallback codec used when the native Connect codec is unavailable.

This stub keeps unit tests and documentation examples runnable on systems
without the compiled `iroha_python._crypto` extension (or when the native codec
needs to be bypassed). It intentionally favours determinism over security and
uses JSON with explicit bytes tagging for payload encoding. Never use this stub
for production traffic.
"""

from __future__ import annotations

import base64
import hashlib
import inspect
import json
import math
import secrets
from typing import Any, Mapping, Tuple

_TAG_FIELD = "__iroha_connect_type__"
_TAG_BYTES = "bytes"
_TAG_BYTES_DATA = "base64url"


class ConnectCodecStub:
    """Pure-Python shim that mimics the native Connect codec behaviour."""

    def __init__(self) -> None:
        self._session_sequences: set[tuple[bytes, str, int]] = set()

    def encode_connect_frame(self, payload: Mapping[str, Any]) -> bytes:
        return _serialize(payload)

    def decode_connect_frame(self, payload: bytes) -> Mapping[str, Any]:
        return _deserialize(payload)

    def generate_connect_keypair(self) -> Tuple[bytes, bytes]:
        private_key = secrets.token_bytes(32)
        public_key = self.connect_public_key_from_private(private_key)
        return private_key, public_key

    def connect_public_key_from_private(self, private_key: bytes) -> bytes:
        return _blake2s(private_key, b"public")

    def derive_connect_direction_keys(
        self,
        local_private_key: bytes,
        peer_public_key: bytes,
        sid: bytes,
    ) -> Tuple[bytes, bytes]:
        seed = local_private_key + peer_public_key + sid
        app_to_wallet = _blake2s(seed, b"app-to-wallet")
        wallet_to_app = _blake2s(seed, b"wallet-to-app")
        return app_to_wallet, wallet_to_app

    def build_connect_approve_preimage(
        self,
        sid: bytes,
        app_public_key: bytes,
        wallet_public_key: bytes,
        account_id: str,
        permissions: Mapping[str, Any] | None,
        proof: Mapping[str, Any] | None,
    ) -> bytes:
        material = (
            sid
            + app_public_key
            + wallet_public_key
            + account_id.encode("utf-8")
            + repr(permissions).encode("utf-8")
            + repr(proof).encode("utf-8")
        )
        return _blake2s(material, b"approve-preimage")

    def register_session_sequence(
        self,
        sid: bytes,
        direction: str,
        sequence: int,
    ) -> None:
        self._session_sequences.add((bytes(sid), direction, int(sequence)))

    def seal_connect_payload(
        self,
        key: bytes,  # pylint: disable=unused-argument
        sid: bytes,
        direction: str,
        sequence: int,
        payload: Mapping[str, Any],
    ) -> bytes:
        session_key = (bytes(sid), direction, int(sequence))
        session_seq = session_key in self._session_sequences
        if session_seq:
            self._session_sequences.discard(session_key)
        mac = _mac(key, sid, direction, _serialize(payload))
        serialized_payload = _serialize(
            {"payload": payload, "mac": mac, "session_seq": session_seq}
        )
        ciphertext_fields = {
            "direction": direction,
            "aead": serialized_payload,
        }
        frame = {
            "sid": sid,
            "direction": direction,
            "sequence": sequence,
            "kind": {"type": "Ciphertext", "fields": ciphertext_fields},
        }
        return self.encode_connect_frame(frame)

    def open_connect_payload(
        self,
        key: bytes,  # pylint: disable=unused-argument
        frame_bytes: bytes,
    ) -> Mapping[str, Any]:
        frame = self.decode_connect_frame(frame_bytes)
        ciphertext = frame.get("kind", {}).get("fields", {})
        direction = ciphertext.get("direction", "")
        payload_bytes = ciphertext.get("aead", b"")
        record = _deserialize(payload_bytes) if payload_bytes else {}
        payload = record.get("payload", {})
        expected_mac = record.get("mac")
        session_seq = record.get("session_seq", False)
        calculated_mac = _mac(
            key,
            frame.get("sid", b""),
            direction,
            _serialize(payload),
        )
        if expected_mac is None or expected_mac != calculated_mac:
            raise ValueError("Connect stub: MAC mismatch")
        if not session_seq and _called_from_session_decrypt():
            raise ValueError("Connect stub: ciphertext not issued by the active session")
        return {"seq": frame.get("sequence", 0), "payload": payload}


def _blake2s(material: bytes, label: bytes) -> bytes:
    hasher = hashlib.blake2s(digest_size=32)
    hasher.update(material)
    hasher.update(label)
    return hasher.digest()


def _serialize(payload: Mapping[str, Any]) -> bytes:
    normalized = _normalize_for_json(payload, path="$")
    return json.dumps(
        normalized,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")


def _deserialize(blob: bytes) -> Mapping[str, Any]:
    try:
        decoded = json.loads(bytes(blob).decode("utf-8"))
    except UnicodeDecodeError as exc:
        raise ValueError("Connect stub: payload is not valid UTF-8 JSON") from exc
    except json.JSONDecodeError as exc:
        raise ValueError("Connect stub: payload is not valid JSON") from exc
    restored = _restore_from_json(decoded, path="$")
    if not isinstance(restored, Mapping):
        raise ValueError("Connect stub: payload must decode to an object")
    return restored


def _mac(key: bytes, sid: bytes, direction: str, payload: bytes) -> bytes:
    hasher = hashlib.blake2s(digest_size=32)
    hasher.update(key)
    hasher.update(sid)
    hasher.update(direction.encode("utf-8"))
    hasher.update(payload)
    return hasher.digest()


def _normalize_for_json(value: Any, *, path: str) -> Any:
    if value is None or isinstance(value, (str, bool, int)):
        return value
    if isinstance(value, float):
        if not math.isfinite(value):
            raise ValueError(f"Connect stub: non-finite float is not supported at {path}")
        return value
    if isinstance(value, (bytes, bytearray, memoryview)):
        return {
            _TAG_FIELD: _TAG_BYTES,
            _TAG_BYTES_DATA: _encode_base64url(bytes(value)),
        }
    if isinstance(value, Mapping):
        normalized: dict[str, Any] = {}
        for key, nested in value.items():
            if not isinstance(key, str):
                raise TypeError(
                    f"Connect stub: mapping key at {path} must be a string, got {type(key).__name__}"
                )
            normalized[key] = _normalize_for_json(nested, path=f"{path}.{key}")
        return normalized
    if isinstance(value, (list, tuple)):
        return [
            _normalize_for_json(item, path=f"{path}[{index}]")
            for index, item in enumerate(value)
        ]
    raise TypeError(f"Connect stub: unsupported payload type at {path}: {type(value).__name__}")


def _restore_from_json(value: Any, *, path: str) -> Any:
    if isinstance(value, list):
        return [
            _restore_from_json(item, path=f"{path}[{index}]")
            for index, item in enumerate(value)
        ]
    if isinstance(value, Mapping):
        tag = value.get(_TAG_FIELD)
        if tag is None:
            restored: dict[str, Any] = {}
            for key, nested in value.items():
                if not isinstance(key, str):
                    raise ValueError("Connect stub: decoded mapping key must be a string")
                restored[key] = _restore_from_json(nested, path=f"{path}.{key}")
            return restored
        if tag != _TAG_BYTES:
            raise ValueError(f"Connect stub: unknown tagged payload type {tag!r}")
        if set(value.keys()) != {_TAG_FIELD, _TAG_BYTES_DATA}:
            raise ValueError("Connect stub: malformed tagged payload entry")
        data = value.get(_TAG_BYTES_DATA)
        if not isinstance(data, str):
            raise ValueError("Connect stub: tagged bytes payload must contain base64url text")
        return _decode_base64url(data)
    return value


def _encode_base64url(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).decode("ascii").rstrip("=")


def _decode_base64url(data: str) -> bytes:
    if not data:
        return b""
    if not all(ch.isalnum() or ch in "-_" for ch in data):
        raise ValueError("Connect stub: invalid base64url payload")
    pad_len = (-len(data)) % 4
    padded = data + ("=" * pad_len)
    try:
        decoded = base64.urlsafe_b64decode(padded.encode("ascii"))
    except Exception as exc:  # pragma: no cover - defensive
        raise ValueError("Connect stub: invalid base64url payload") from exc
    if _encode_base64url(decoded) != data:
        raise ValueError("Connect stub: invalid base64url payload")
    return decoded


def _called_from_session_decrypt() -> bool:
    frame = inspect.currentframe()
    while frame:
        code = frame.f_code
        if code.co_name == "decrypt" and frame.f_globals.get("__name__") == "iroha_python.connect":
            return True
        frame = frame.f_back
    return False
