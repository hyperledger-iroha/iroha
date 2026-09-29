"""Closed protocol-8 native status observations; these values are not finality proofs."""
from __future__ import annotations

from binascii import crc_hqx
from dataclasses import dataclass
import json
import re
from typing import Any, Mapping
from .governance_proposals import _kagemusha_public_key
from .bls_public_key import decode_bls_normal_peer_id

STATUS_MAX_BYTES = 1024 * 1024
_STATUS = frozenset("protocol_version config_fingerprint beacon_horizon instance height view stage leader proxy_tail high_qc_view level start_level t_retx_ms committed_height applied_height awaiting signer unanchored abstaining halted footprint".split())
_FOOTPRINT = tuple("votes timeouts blocks exec_entries wants pending_apply sync_entries sync_bytes peers recent_headers configs cert_cache evidence_keys probe".split())
_HORIZON = frozenset("epoch_length_blocks next_required_pulse_height active_session_id session_covers_next_pulse local_provider_ready".split())
_HEIGHT_REASONS = frozenset(("safety_violation", "apply_diverged", "publication_recovery_required"))
_UNIT_REASONS = frozenset(("safety_record_corrupt", "safety_record_inconsistent", "driver_anomaly"))

def _record(value: Any, fields: frozenset[str] | tuple[str, ...], label: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping) or set(value) != set(fields):
        raise ValueError(f"{label} requires exactly its native fields")
    return value

def _uint(value: Any, bits: int = 64) -> int:
    if type(value) is not int or not 0 <= value < (1 << bits):
        raise ValueError(f"native status value must be an unsigned {bits}-bit integer")
    return value

def _boolean(value: Any) -> bool:
    if type(value) is not bool:
        raise ValueError("native status value must be a boolean")
    return value

def _optional_uint(value: Any) -> int | None:
    return None if value is None else _uint(value)

def _public_key(value: Any) -> str | None:
    if value is None:
        return None
    literal, (algorithm, _payload) = _kagemusha_public_key(value, "native status public key")
    if literal.startswith("ea0130"):
        decode_bls_normal_peer_id(literal, "native status public key")
    return literal

def _hash(value: Any) -> str:
    if not isinstance(value, str) or re.fullmatch(r"hash:[0-9A-F]{64}#[0-9A-F]{4}", value) is None:
        raise ValueError("native status fingerprint must be a canonical hash")
    raw = bytes.fromhex(value[5:69])
    if not raw[-1] & 1 or int(value[70:], 16) != crc_hqx(value[:69].encode("ascii"), 0xFFFF):
        raise ValueError("native status fingerprint has invalid marker or checksum")
    return value

@dataclass(frozen=True)
class SumeragiFootprint:
    """Unsigned native core memory counters observed at this cut."""
    votes: int
    timeouts: int
    blocks: int
    exec_entries: int
    wants: int
    pending_apply: int
    sync_entries: int
    sync_bytes: int
    peers: int
    recent_headers: int
    configs: int
    cert_cache: int
    evidence_keys: int
    probe: int

@dataclass(frozen=True)
class SumeragiBeaconHorizon:
    """Readiness observed by the actual beacon owner at the same applied cut."""
    epoch_length_blocks: int
    next_required_pulse_height: int | None
    active_session_id: str | None
    session_covers_next_pulse: bool
    local_provider_ready: bool

@dataclass(frozen=True)
class SumeragiHaltReason:
    """Closed native halt reason with explicit unit or height details."""
    reason: str
    details: int | None

@dataclass(frozen=True)
class SumeragiStatus:
    """Immutable protocol-8 observation with required nullable fields."""
    protocol_version: int
    config_fingerprint: str
    beacon_horizon: SumeragiBeaconHorizon | None
    instance: str
    height: int
    view: int
    stage: int
    leader: str | None
    proxy_tail: str | None
    high_qc_view: int | None
    level: int
    start_level: int
    t_retx_ms: int
    committed_height: int
    applied_height: int
    awaiting: bool
    signer: str | None
    unanchored: bool
    abstaining: bool
    halted: SumeragiHaltReason | None
    footprint: SumeragiFootprint

    @classmethod
    def from_payload(cls, payload: Any) -> "SumeragiStatus":
        """Validate every field against the current native observation schema."""
        r = _record(payload, _STATUS, "native status")
        if _uint(r["protocol_version"], 16) != 8 or _uint(r["stage"], 16) > 2:
            raise ValueError("unsupported native status protocol or stage")
        if not isinstance(r["instance"], str) or re.fullmatch(r"[0-9a-f]{64}", r["instance"]) is None:
            raise ValueError("native instance must be exact lowercase 32-byte hex")
        f = _record(r["footprint"], _FOOTPRINT, "native footprint")
        horizon = None
        if r["beacon_horizon"] is not None:
            h = _record(r["beacon_horizon"], _HORIZON, "native beacon horizon")
            session = h["active_session_id"]
            if session is not None and (not isinstance(session, str) or re.fullmatch(r"[0-9A-F]{64}", session) is None):
                raise ValueError("native beacon session must be exact uppercase 32-byte hex")
            next_height = _optional_uint(h["next_required_pulse_height"])
            covers = _boolean(h["session_covers_next_pulse"])
            ready = _boolean(h["local_provider_ready"])
            if (covers and (session is None or next_height is None)) or (ready and session is None):
                raise ValueError("native beacon readiness requires the exact session and demand")
            horizon = SumeragiBeaconHorizon(_uint(h["epoch_length_blocks"]), next_height, session, covers, ready)
        halted = None
        if r["halted"] is not None:
            h = _record(r["halted"], ("reason", "details"), "native halt reason")
            if h["reason"] in _HEIGHT_REASONS:
                detail = _uint(h["details"])
            elif h["reason"] in _UNIT_REASONS and h["details"] is None:
                detail = None
            else:
                raise ValueError("invalid native halt reason or details")
            halted = SumeragiHaltReason(h["reason"], detail)
        return cls(8, _hash(r["config_fingerprint"]), horizon, r["instance"],
                   _uint(r["height"]), _uint(r["view"]), r["stage"],
                   _public_key(r["leader"]), _public_key(r["proxy_tail"]),
                   _optional_uint(r["high_qc_view"]), _uint(r["level"], 32),
                   _uint(r["start_level"], 32), _uint(r["t_retx_ms"]),
                   _uint(r["committed_height"]), _uint(r["applied_height"]),
                   _boolean(r["awaiting"]), _public_key(r["signer"]),
                   _boolean(r["unanchored"]), _boolean(r["abstaining"]), halted,
                   SumeragiFootprint(**{name: _uint(f[name]) for name in _FOOTPRINT}))

def parse_native_status_json(payload: bytes, label: str = "native status") -> dict[str, Any]:
    """Bounded strict JSON tokenizer; preserve every unsigned bit and reject signed zero."""
    if not isinstance(payload, bytes) or not 0 < len(payload) <= STATUS_MAX_BYTES:
        raise ValueError(f"{label} is empty or exceeds its byte bound")
    def integer(token: str) -> int:
        if token.startswith("-"):
            raise ValueError(f"{label} requires unsigned integer tokens")
        return int(token)
    def reject(token: str) -> None:
        raise ValueError(f"{label} contains a non-integer numeric token {token}")
    def unique(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"{label} contains duplicate field {key}")
            result[key] = value
        return result
    try:
        value = json.loads(payload.decode("utf-8", "strict"), parse_int=integer,
                           parse_float=reject, parse_constant=reject, object_pairs_hook=unique)
    except (UnicodeDecodeError, json.JSONDecodeError, RecursionError) as error:
        raise ValueError(f"{label} must be strict UTF-8 JSON") from error
    _record(value, _STATUS, label)
    return value
