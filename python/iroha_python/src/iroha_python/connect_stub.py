"""Lightweight fallback codec used when the native Connect codec is unavailable.

This stub keeps unit tests and documentation examples runnable on systems
without the compiled `iroha_python._crypto` extension (or when the native codec
needs to be bypassed). It intentionally favours determinism over security:
payloads are simply pickled, and symmetric keys derive from hashed inputs.
Never use this stub for production traffic.
"""

from __future__ import annotations

import hashlib
import inspect
import pickle
import secrets
from typing import Any, Mapping, Tuple


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
    return pickle.dumps(payload, protocol=pickle.HIGHEST_PROTOCOL)


def _deserialize(blob: bytes) -> Mapping[str, Any]:
    return pickle.loads(blob)


def _mac(key: bytes, sid: bytes, direction: str, payload: bytes) -> bytes:
    hasher = hashlib.blake2s(digest_size=32)
    hasher.update(key)
    hasher.update(sid)
    hasher.update(direction.encode("utf-8"))
    hasher.update(payload)
    return hasher.digest()


def _called_from_session_decrypt() -> bool:
    frame = inspect.currentframe()
    while frame:
        code = frame.f_code
        if code.co_name == "decrypt" and frame.f_globals.get("__name__") == "iroha_python.connect":
            return True
        frame = frame.f_back
    return False
