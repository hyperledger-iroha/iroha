"""Connect helpers: URI builders plus frame encode/decode utilities."""

from __future__ import annotations

import base64
import hashlib
import os
import warnings
from dataclasses import dataclass
from datetime import datetime, timedelta
from enum import Enum
from typing import Any, Dict, List, Mapping, Optional, Sequence, Type, Union
from urllib.parse import ParseResult, parse_qs, urlencode, urlparse

from ._native import load_crypto_extension
from .connect_stub import ConnectCodecStub

__all__ = [
    "ConnectUri",
    "build_connect_uri",
    "parse_connect_uri",
    "ConnectDirection",
    "ConnectRole",
    "ConnectKeyPair",
    "ConnectSid",
    "ConnectSessionInfo",
    "ConnectPermissions",
    "ConnectAppMetadata",
    "ConnectSignInProof",
    "ConnectCiphertext",
    "ConnectControlOpen",
    "ConnectControlApprove",
    "ConnectControlReject",
    "ConnectControlClose",
    "ConnectControlPing",
    "ConnectControlPong",
    "ConnectFrame",
    "ConnectSignRequestRawPayload",
    "ConnectSignRequestTxPayload",
    "ConnectSignResultOkPayload",
    "ConnectSignResultErrPayload",
    "ConnectDisplayRequestPayload",
    "ConnectEnvelope",
    "ConnectSessionKeys",
    "ConnectSessionState",
    "ConnectSession",
    "encode_connect_frame",
    "decode_connect_frame",
    "derive_connect_direction_keys",
    "build_connect_approve_preimage",
    "generate_connect_keypair",
    "generate_connect_sid",
    "ConnectSessionPreview",
    "ConnectPreviewTokens",
    "ConnectPreviewBootstrapResult",
    "create_connect_session_preview",
    "bootstrap_connect_preview_session",
    "connect_public_key_from_private",
    "seal_connect_payload",
    "open_connect_payload",
]

_SID_PREFIX = b"iroha-connect|sid|"
_SID_LENGTH = 32
_NONCE_LENGTH = 16
_CONNECT_URI_VERSION = "1"


@dataclass(frozen=True)
class ConnectUri:
    """Structured representation of an `iroha://connect` URI."""

    sid: str
    chain_id: str
    node: Optional[str] = None
    version: int = 1


def build_connect_uri(data: ConnectUri) -> str:
    """Return a canonical `iroha://connect?...` URI."""

    if not data.sid:
        raise ValueError("sid is required")
    if not data.chain_id:
        raise ValueError("chain_id is required")
    if data.version < 1:
        raise ValueError("version must be >= 1")
    query_items = {
        "sid": data.sid,
        "chain_id": data.chain_id,
        "v": str(data.version),
    }
    if data.node:
        query_items["node"] = data.node
    query = urlencode(query_items)
    base = "iroha://connect"
    if query:
        return f"{base}?{query}"
    return base


def parse_connect_uri(uri: str) -> ConnectUri:
    """Parse an `iroha://connect?...` URI into a :class:`ConnectUri`."""

    parsed: ParseResult = urlparse(uri)
    if parsed.scheme != "iroha":
        raise ValueError("URI scheme must be 'iroha'")
    path_is_connect = parsed.path in {"/connect"}
    host_is_connect = parsed.netloc == "connect" and parsed.path in {"", "/"}
    if not (path_is_connect or host_is_connect):
        raise ValueError("URI path must be '/connect'")
    params = parse_qs(parsed.query, strict_parsing=True)
    sid = _require(_get_single(params, "sid"), "sid")
    chain_id = _require(_get_single(params, "chain_id"), "chain_id")
    version_str = _require(_get_single(params, "v", default="1"), "v")
    try:
        version = int(version_str)
    except ValueError as exc:
        raise ValueError("version must be an integer") from exc
    node = _get_single(params, "node", default=None)
    return ConnectUri(sid=sid, chain_id=chain_id, node=node, version=version)


_MISSING = object()


def _get_single(
    mapping: dict[str, List[str]],
    key: str,
    default: object = _MISSING,
) -> Optional[str]:
    if key not in mapping:
        if default is _MISSING:
            raise ValueError(f"missing '{key}' query parameter")
        return default  # type: ignore[return-value]
    values = mapping[key]
    if len(values) != 1:
        raise ValueError(f"parameter '{key}' must appear exactly once")
    if not values[0]:
        raise ValueError(f"parameter '{key}' must not be empty")
    return values[0]


def _require(value: Optional[str], name: str) -> str:
    if value is None:
        raise ValueError(f"{name} must be provided")
    return value


_BytesLike = Union[bytes, bytearray, memoryview]
_CodecModule = Optional[Any]
_CODEC_MODULE: _CodecModule = None
_STUB_NOTICE_EMITTED = False


def _connect_codec_mode() -> Optional[str]:
    value = os.environ.get("IROHA_PYTHON_CONNECT_CODEC")
    if value is None:
        value = os.environ.get("IROHA_PYTHON_CONNECT_STUB")
    if value is None:
        return None
    normalized = value.strip().lower()
    return normalized or None


def _should_force_stub() -> bool:
    mode = _connect_codec_mode()
    return mode in {"1", "true", "yes", "on", "stub", "test"}


def _load_stub(reason: str) -> Any:
    global _CODEC_MODULE, _STUB_NOTICE_EMITTED
    if not _STUB_NOTICE_EMITTED:
        warnings.warn(
            f"Using Connect codec stub ({reason}); do not rely on it in production.",
            RuntimeWarning,
            stacklevel=3,
        )
        _STUB_NOTICE_EMITTED = True
    _CODEC_MODULE = ConnectCodecStub()
    return _CODEC_MODULE


def _require_codec_module() -> Any:
    global _CODEC_MODULE
    if _CODEC_MODULE is not None:
        return _CODEC_MODULE
    if _should_force_stub():
        return _load_stub("IROHA_PYTHON_CONNECT_CODEC=stub")
    try:
        _CODEC_MODULE = load_crypto_extension()
    except Exception as exc:  # pragma: no cover - defensive
        raise RuntimeError(
            "native Connect codec unavailable; install the extension or set "
            "IROHA_PYTHON_CONNECT_CODEC=stub explicitly for tests"
        ) from exc
    return _CODEC_MODULE


def _register_stub_session_sequence(
    sid: bytes,
    direction: ConnectDirection,
    sequence: int,
) -> None:
    codec = _require_codec_module()
    register = getattr(codec, "register_session_sequence", None)
    if callable(register):
        register(bytes(sid), direction.value, int(sequence))


def _ensure_bytes(payload: _BytesLike, *, size: Optional[int], field: str) -> bytes:
    data = bytes(payload)
    if size is not None and len(data) != size:
        raise ValueError(f"{field} must be {size} bytes, got {len(data)}")
    return data


class ConnectDirection(str, Enum):
    """Direction of a Connect frame."""

    APP_TO_WALLET = "AppToWallet"
    WALLET_TO_APP = "WalletToApp"

    @classmethod
    def normalize(cls, value: Union["ConnectDirection", str]) -> "ConnectDirection":
        if isinstance(value, ConnectDirection):
            return value
        try:
            return ConnectDirection(value)
        except ValueError as exc:  # pragma: no cover - defensive
            raise ValueError(f"invalid connect direction {value!r}") from exc


class ConnectRole(str, Enum):
    """Role responsible for a Connect control message."""

    APP = "App"
    WALLET = "Wallet"

    @classmethod
    def normalize(cls, value: Union["ConnectRole", str]) -> "ConnectRole":
        if isinstance(value, ConnectRole):
            return value
        try:
            return ConnectRole(value)
        except ValueError as exc:  # pragma: no cover - defensive
            raise ValueError(f"invalid connect role {value!r}") from exc


@dataclass(frozen=True)
class ConnectKeyPair:
    """Ephemeral X25519 key pair used for Connect sessions."""

    private_key: bytes
    public_key: bytes


@dataclass(frozen=True)
class ConnectSid:
    """Deterministic Connect session identifier plus helper encodings."""

    sid_bytes: bytes
    sid_base64url: str
    nonce: bytes


@dataclass(frozen=True)
class ConnectSessionPreview:
    """Pre-registration preview bundle used by dashboards and wallets."""

    chain_id: str
    node: Optional[str]
    sid_bytes: bytes
    sid_base64url: str
    nonce: bytes
    app_key_pair: ConnectKeyPair
    wallet_uri: str
    app_uri: str


@dataclass(frozen=True)
class ConnectPreviewTokens:
    """Convenience container for Torii-issued Connect session tokens."""

    wallet: str
    app: str


@dataclass(frozen=True)
class ConnectPreviewBootstrapResult:
    """Return value for :func:`bootstrap_connect_preview_session`."""

    preview: ConnectSessionPreview
    session: Optional["ConnectSessionInfo"]
    tokens: Optional[ConnectPreviewTokens]


@dataclass(frozen=True)
class ConnectSessionInfo:
    """Response payload returned by `ToriiClient.create_connect_session`."""

    sid: str
    app_uri: str
    app_token: str
    wallet_token: str
    expires_at: Optional[datetime] = None

    @classmethod
    def from_mapping(
        cls,
        payload: Mapping[str, Any],
        *,
        session_ttl_ms: Optional[int] = None,
    ) -> "ConnectSessionInfo":
        try:
            expires_at = None
            if session_ttl_ms is not None and session_ttl_ms > 0:
                expires_at = datetime.utcnow() + timedelta(milliseconds=session_ttl_ms)
            return cls(
                sid=str(payload["sid"]),
                app_uri=str(payload["app_uri"]),
                app_token=str(payload["token_app"]),
                wallet_token=str(payload["token_wallet"]),
                expires_at=expires_at,
            )
        except KeyError as exc:  # pragma: no cover - defensive
            raise ValueError("connect session response is missing required fields") from exc

    def as_dict(self) -> Dict[str, Optional[str]]:
        """Return the raw response as a JSON-friendly dict."""

        return {
            "sid": self.sid,
            "app_uri": self.app_uri,
            "token_app": self.app_token,
            "token_wallet": self.wallet_token,
            "expires_at": self.expires_at.isoformat(timespec="seconds") if self.expires_at else None,
        }


@dataclass
class ConnectPermissions:
    """Requested methods/events/resources for a Connect session."""

    methods: Sequence[str]
    events: Sequence[str]
    resources: Optional[Sequence[str]] = None

    def __post_init__(self) -> None:
        self.methods = list(self.methods)
        self.events = list(self.events)
        if self.resources is not None:
            self.resources = list(self.resources)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "methods": list(self.methods),
            "events": list(self.events),
            "resources": list(self.resources) if self.resources is not None else None,
        }

    @classmethod
    def from_dict(cls, payload: Mapping[str, Any]) -> "ConnectPermissions":
        return cls(
            methods=payload.get("methods", ()),
            events=payload.get("events", ()),
            resources=payload.get("resources"),
        )


@dataclass
class ConnectAppMetadata:
    """Display metadata for applications."""

    name: str
    url: Optional[str] = None
    icon_hash: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
        return {"name": self.name, "url": self.url, "icon_hash": self.icon_hash}

    @classmethod
    def from_dict(cls, payload: Mapping[str, Any]) -> "ConnectAppMetadata":
        return cls(
            name=payload["name"],
            url=payload.get("url"),
            icon_hash=payload.get("icon_hash"),
        )


@dataclass
class ConnectSignInProof:
    """Sign-in proof carried alongside approvals."""

    domain: str
    uri: str
    statement: str
    issued_at: str
    nonce: str

    def to_dict(self) -> Dict[str, str]:
        return {
            "domain": self.domain,
            "uri": self.uri,
            "statement": self.statement,
            "issued_at": self.issued_at,
            "nonce": self.nonce,
        }

    @classmethod
    def from_dict(cls, payload: Mapping[str, Any]) -> "ConnectSignInProof":
        return cls(
            domain=payload["domain"],
            uri=payload["uri"],
            statement=payload["statement"],
            issued_at=payload["issued_at"],
            nonce=payload["nonce"],
        )


@dataclass
class ConnectCiphertext:
    """Encrypted payload delivered after session approval."""

    direction: ConnectDirection
    aead: bytes

    def __post_init__(self) -> None:
        self.direction = ConnectDirection.normalize(self.direction)
        self.aead = bytes(self.aead)

    def to_dict(self) -> Dict[str, Any]:
        return {"direction": self.direction.value, "aead": self.aead}

    @classmethod
    def from_dict(cls, payload: Mapping[str, Any]) -> "ConnectCiphertext":
        return cls(
            direction=ConnectDirection.normalize(payload["direction"]),
            aead=_ensure_bytes(payload["aead"], size=None, field="ciphertext.aead"),
        )


@dataclass
class _ConnectControlBase:
    variant: str

    def to_dict(self) -> Dict[str, Any]:
        raise NotImplementedError

    @property
    def endpoint_kind(self) -> str:
        """Return the lowercase variant name suitable for Torii REST endpoints."""

        return self.variant.lower()


@dataclass
class ConnectControlOpen(_ConnectControlBase):
    app_public_key: bytes
    chain_id: str
    permissions: Optional[ConnectPermissions] = None
    metadata: Optional[ConnectAppMetadata] = None

    def __init__(
        self,
        *,
        app_public_key: _BytesLike,
        chain_id: str,
        permissions: Optional[ConnectPermissions] = None,
        metadata: Optional[ConnectAppMetadata] = None,
    ) -> None:
        super().__init__(variant="Open")
        self.app_public_key = _ensure_bytes(app_public_key, size=32, field="app_public_key")
        self.chain_id = chain_id
        self.permissions = permissions
        self.metadata = metadata

    def to_dict(self) -> Dict[str, Any]:
        return {
            "app_public_key": self.app_public_key,
            "chain_id": self.chain_id,
            "permissions": self.permissions.to_dict() if self.permissions else None,
            "metadata": self.metadata.to_dict() if self.metadata else None,
        }

    @classmethod
    def from_dict(cls, payload: Mapping[str, Any]) -> "ConnectControlOpen":
        permissions = (
            ConnectPermissions.from_dict(payload["permissions"])
            if payload.get("permissions")
            else None
        )
        metadata = (
            ConnectAppMetadata.from_dict(payload["metadata"])
            if payload.get("metadata")
            else None
        )
        return cls(
            app_public_key=payload["app_public_key"],
            chain_id=payload["chain_id"],
            permissions=permissions,
            metadata=metadata,
        )


@dataclass
class ConnectControlApprove(_ConnectControlBase):
    wallet_public_key: bytes
    account_id: str
    signature: bytes
    algorithm: str = "ed25519"
    permissions: Optional[ConnectPermissions] = None
    proof: Optional[ConnectSignInProof] = None

    def __init__(
        self,
        *,
        wallet_public_key: _BytesLike,
        account_id: str,
        signature: _BytesLike,
        algorithm: str = "ed25519",
        permissions: Optional[ConnectPermissions] = None,
        proof: Optional[ConnectSignInProof] = None,
    ) -> None:
        super().__init__(variant="Approve")
        self.wallet_public_key = _ensure_bytes(
            wallet_public_key, size=32, field="wallet_public_key"
        )
        self.account_id = account_id
        self.signature = _ensure_bytes(signature, size=64, field="signature")
        self.algorithm = algorithm
        self.permissions = permissions
        self.proof = proof

    def to_dict(self) -> Dict[str, Any]:
        return {
            "wallet_public_key": self.wallet_public_key,
            "account_id": self.account_id,
            "signature": self.signature,
            "algorithm": self.algorithm,
            "permissions": self.permissions.to_dict() if self.permissions else None,
            "proof": self.proof.to_dict() if self.proof else None,
        }

    @classmethod
    def from_dict(cls, payload: Mapping[str, Any]) -> "ConnectControlApprove":
        permissions = (
            ConnectPermissions.from_dict(payload["permissions"])
            if payload.get("permissions")
            else None
        )
        proof = (
            ConnectSignInProof.from_dict(payload["proof"])
            if payload.get("proof")
            else None
        )
        return cls(
            wallet_public_key=payload["wallet_public_key"],
            account_id=payload["account_id"],
            signature=payload["signature"],
            algorithm=payload.get("algorithm", "Ed25519"),
            permissions=permissions,
            proof=proof,
        )


@dataclass
class ConnectControlReject(_ConnectControlBase):
    code: int
    code_id: str
    reason: str

    def __init__(self, *, code: int, code_id: str, reason: str) -> None:
        super().__init__(variant="Reject")
        self.code = int(code)
        self.code_id = code_id
        self.reason = reason

    def to_dict(self) -> Dict[str, Any]:
        return {"code": self.code, "code_id": self.code_id, "reason": self.reason}

    @classmethod
    def from_dict(cls, payload: Mapping[str, Any]) -> "ConnectControlReject":
        return cls(code=payload["code"], code_id=payload["code_id"], reason=payload["reason"])


@dataclass
class ConnectControlClose(_ConnectControlBase):
    role: ConnectRole
    code: int
    reason: str
    retryable: bool

    def __init__(
        self, *, role: Union[ConnectRole, str], code: int, reason: str, retryable: bool
    ) -> None:
        super().__init__(variant="Close")
        self.role = ConnectRole.normalize(role)
        self.code = int(code)
        self.reason = reason
        self.retryable = bool(retryable)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "role": self.role.value,
            "code": self.code,
            "reason": self.reason,
            "retryable": self.retryable,
        }

    @classmethod
    def from_dict(cls, payload: Mapping[str, Any]) -> "ConnectControlClose":
        return cls(
            role=payload["role"],
            code=payload["code"],
            reason=payload["reason"],
            retryable=payload["retryable"],
        )


@dataclass
class ConnectControlPing(_ConnectControlBase):
    nonce: int

    def __init__(self, *, nonce: int) -> None:
        super().__init__(variant="Ping")
        self.nonce = int(nonce)

    def to_dict(self) -> Dict[str, Any]:
        return {"nonce": self.nonce}

    @classmethod
    def from_dict(cls, payload: Mapping[str, Any]) -> "ConnectControlPing":
        return cls(nonce=payload["nonce"])


@dataclass
class ConnectControlPong(_ConnectControlBase):
    nonce: int

    def __init__(self, *, nonce: int) -> None:
        super().__init__(variant="Pong")
        self.nonce = int(nonce)

    def to_dict(self) -> Dict[str, Any]:
        return {"nonce": self.nonce}

    @classmethod
    def from_dict(cls, payload: Mapping[str, Any]) -> "ConnectControlPong":
        return cls(nonce=payload["nonce"])


ControlVariant = Union[
    ConnectControlOpen,
    ConnectControlApprove,
    ConnectControlReject,
    ConnectControlClose,
    ConnectControlPing,
    ConnectControlPong,
]

_CONTROL_CLASS_MAP: Dict[str, Type[ControlVariant]] = {
    "Open": ConnectControlOpen,
    "Approve": ConnectControlApprove,
    "Reject": ConnectControlReject,
    "Close": ConnectControlClose,
    "Ping": ConnectControlPing,
    "Pong": ConnectControlPong,
}


@dataclass
class ConnectFrame:
    """Structured representation of a Connect frame."""

    sid: bytes
    direction: ConnectDirection
    sequence: int
    control: Optional[ControlVariant] = None
    ciphertext: Optional[ConnectCiphertext] = None

    def __post_init__(self) -> None:
        self.sid = _ensure_bytes(self.sid, size=32, field="sid")
        self.direction = ConnectDirection.normalize(self.direction)
        self.sequence = int(self.sequence)
        if (self.control is None) == (self.ciphertext is None):
            raise ValueError("provide exactly one of `control` or `ciphertext` for a frame")

    def to_dict(self) -> Dict[str, Any]:
        payload: Dict[str, Any] = {
            "sid": self.sid,
            "direction": self.direction.value,
            "sequence": self.sequence,
        }
        if self.control is not None:
            payload["kind"] = {
                "type": "Control",
                "control_type": self.control.variant,
                "fields": self.control.to_dict(),
            }
        else:
            payload["kind"] = {"type": "Ciphertext", "fields": self.ciphertext.to_dict()}  # type: ignore[union-attr]
        return payload

    def to_bytes(self) -> bytes:
        """Return Norito-encoded bytes for the frame."""

        codec = _require_codec_module()
        return bytes(codec.encode_connect_frame(self.to_dict()))

    @classmethod
    def from_dict(cls, payload: Mapping[str, Any]) -> "ConnectFrame":
        kind = payload["kind"]
        sid = payload["sid"]
        direction = payload["direction"]
        sequence = payload["sequence"]
        if kind["type"] == "Control":
            variant = kind["control_type"]
            fields = kind["fields"]
            control_cls = _CONTROL_CLASS_MAP.get(variant)
            if control_cls is None:
                raise ValueError(f"unsupported control variant {variant!r}")
            control = control_cls.from_dict(fields)
            return cls(sid=sid, direction=direction, sequence=sequence, control=control)
        if kind["type"] == "Ciphertext":
            return cls(
                sid=sid,
                direction=direction,
                sequence=sequence,
                ciphertext=ConnectCiphertext.from_dict(kind["fields"]),
            )
        raise ValueError(f"unsupported frame kind {kind['type']!r}")

    @classmethod
    def from_bytes(cls, payload: _BytesLike) -> "ConnectFrame":
        """Decode Norito-encoded frame bytes."""

        codec = _require_codec_module()
        decoded = codec.decode_connect_frame(bytes(payload))
        return cls.from_dict(decoded)



ConnectCiphertextPayload = Union[
    "ConnectControlClose",
    "ConnectControlReject",
    "ConnectSignRequestRawPayload",
    "ConnectSignRequestTxPayload",
    "ConnectSignResultOkPayload",
    "ConnectSignResultErrPayload",
    "ConnectDisplayRequestPayload",
]


@dataclass
class ConnectSignRequestRawPayload:
    domain_tag: str
    payload: bytes

    def __init__(self, *, domain_tag: str, payload: _BytesLike) -> None:
        self.domain_tag = domain_tag
        self.payload = _ensure_bytes(payload, size=None, field="payload")

    def to_wire_dict(self) -> Dict[str, Any]:
        return {
            "type": "SignRequestRaw",
            "domain_tag": self.domain_tag,
            "bytes": self.payload,
        }

    @classmethod
    def from_dict(cls, payload: Mapping[str, Any]) -> "ConnectSignRequestRawPayload":
        return cls(
            domain_tag=str(payload["domain_tag"]),
            payload=_ensure_bytes(payload["bytes"], size=None, field="bytes"),
        )


@dataclass
class ConnectSignRequestTxPayload:
    tx_bytes: bytes

    def __init__(self, *, tx_bytes: _BytesLike) -> None:
        self.tx_bytes = _ensure_bytes(tx_bytes, size=None, field="tx_bytes")

    def to_wire_dict(self) -> Dict[str, Any]:
        return {"type": "SignRequestTx", "tx_bytes": self.tx_bytes}

    @classmethod
    def from_dict(cls, payload: Mapping[str, Any]) -> "ConnectSignRequestTxPayload":
        return cls(tx_bytes=_ensure_bytes(payload["tx_bytes"], size=None, field="tx_bytes"))


@dataclass
class ConnectSignResultOkPayload:
    signature: bytes
    algorithm: str = "Ed25519"

    def __init__(self, *, signature: _BytesLike, algorithm: str = "Ed25519") -> None:
        self.signature = _ensure_bytes(signature, size=None, field="signature")
        self.algorithm = algorithm

    def to_wire_dict(self) -> Dict[str, Any]:
        return {
            "type": "SignResultOk",
            "signature": {
                "algorithm": self.algorithm,
                "signature": self.signature,
            },
        }

    @classmethod
    def from_dict(cls, payload: Mapping[str, Any]) -> "ConnectSignResultOkPayload":
        signature_payload = payload["signature"]
        if not isinstance(signature_payload, Mapping):
            raise TypeError("signature payload must be a mapping")
        return cls(
            signature=_ensure_bytes(
                signature_payload["signature"], size=None, field="signature"
            ),
            algorithm=signature_payload.get("algorithm", "Ed25519"),
        )


@dataclass
class ConnectSignResultErrPayload:
    code: str
    message: str

    def to_wire_dict(self) -> Dict[str, Any]:
        return {"type": "SignResultErr", "code": self.code, "message": self.message}

    @classmethod
    def from_dict(cls, payload: Mapping[str, Any]) -> "ConnectSignResultErrPayload":
        return cls(code=str(payload["code"]), message=str(payload["message"]))


@dataclass
class ConnectDisplayRequestPayload:
    title: str
    body: str

    def to_wire_dict(self) -> Dict[str, Any]:
        return {"type": "DisplayRequest", "title": self.title, "body": self.body}

    @classmethod
    def from_dict(cls, payload: Mapping[str, Any]) -> "ConnectDisplayRequestPayload":
        return cls(title=str(payload["title"]), body=str(payload["body"]))


@dataclass
class ConnectEnvelope:
    sequence: int
    payload: ConnectCiphertextPayload

    def to_dict(self) -> Dict[str, Any]:
        return {"seq": int(self.sequence), "payload": _payload_to_dict(self.payload)}

    @classmethod
    def from_dict(cls, payload: Mapping[str, Any]) -> "ConnectEnvelope":
        seq = int(payload["seq"])
        payload_obj = payload["payload"]
        if not isinstance(payload_obj, Mapping):
            raise TypeError("connect payload must be a mapping")
        parsed = _payload_from_dict(payload_obj)
        return cls(sequence=seq, payload=parsed)


def _payload_to_dict(payload: ConnectCiphertextPayload) -> Dict[str, Any]:
    if isinstance(payload, ConnectControlClose):
        return {"type": "Control", "variant": "Close", "fields": payload.to_dict()}
    if isinstance(payload, ConnectControlReject):
        return {"type": "Control", "variant": "Reject", "fields": payload.to_dict()}
    if isinstance(payload, ConnectSignRequestRawPayload):
        return payload.to_wire_dict()
    if isinstance(payload, ConnectSignRequestTxPayload):
        return payload.to_wire_dict()
    if isinstance(payload, ConnectSignResultOkPayload):
        return payload.to_wire_dict()
    if isinstance(payload, ConnectSignResultErrPayload):
        return payload.to_wire_dict()
    if isinstance(payload, ConnectDisplayRequestPayload):
        return payload.to_wire_dict()
    raise TypeError(f"unsupported connect payload type {type(payload)!r}")


def _payload_from_dict(payload: Mapping[str, Any]) -> ConnectCiphertextPayload:
    kind = payload.get("type")
    if kind == "Control":
        variant = payload.get("variant")
        fields = payload.get("fields")
        if not isinstance(fields, Mapping):
            raise TypeError("control payload `fields` must be a mapping")
        if variant == "Close":
            return ConnectControlClose.from_dict(fields)
        if variant == "Reject":
            return ConnectControlReject.from_dict(fields)
        raise ValueError(f"unsupported control variant {variant!r}")
    if kind == "SignRequestRaw":
        return ConnectSignRequestRawPayload.from_dict(payload)
    if kind == "SignRequestTx":
        return ConnectSignRequestTxPayload.from_dict(payload)
    if kind == "SignResultOk":
        return ConnectSignResultOkPayload.from_dict(payload)
    if kind == "SignResultErr":
        return ConnectSignResultErrPayload.from_dict(payload)
    if kind == "DisplayRequest":
        return ConnectDisplayRequestPayload.from_dict(payload)
    raise ValueError(f"unsupported connect payload type {kind!r}")

def encode_connect_frame(frame: Union[ConnectFrame, Mapping[str, Any]]) -> bytes:
    """Encode a Connect frame to Norito bytes."""

    if isinstance(frame, ConnectFrame):
        payload = frame.to_dict()
    else:
        payload = dict(frame)
    codec = _require_codec_module()
    return bytes(codec.encode_connect_frame(payload))


def decode_connect_frame(payload: _BytesLike) -> ConnectFrame:
    """Decode Norito-encoded Connect frame bytes into a :class:`ConnectFrame`."""

    codec = _require_codec_module()
    decoded = codec.decode_connect_frame(bytes(payload))
    return ConnectFrame.from_dict(decoded)


def generate_connect_keypair() -> ConnectKeyPair:
    """Return a freshly generated Connect X25519 key pair."""

    codec = _require_codec_module()
    private_key, public_key = codec.generate_connect_keypair()
    return ConnectKeyPair(private_key=bytes(private_key), public_key=bytes(public_key))


def connect_public_key_from_private(private_key: _BytesLike) -> bytes:
    """Derive the Connect X25519 public key corresponding to `private_key`."""

    codec = _require_codec_module()
    result = codec.connect_public_key_from_private(
        _ensure_bytes(private_key, size=32, field="private_key")
    )
    return bytes(result)


def derive_connect_direction_keys(
    local_private_key: _BytesLike,
    peer_public_key: _BytesLike,
    sid: _BytesLike,
) -> tuple[bytes, bytes]:
    """Derive the App→Wallet and Wallet→App symmetric keys for a session."""

    codec = _require_codec_module()
    app_key, wallet_key = codec.derive_connect_direction_keys(
        _ensure_bytes(local_private_key, size=32, field="local_private_key"),
        _ensure_bytes(peer_public_key, size=32, field="peer_public_key"),
        _ensure_bytes(sid, size=32, field="sid"),
    )
    return bytes(app_key), bytes(wallet_key)


def build_connect_approve_preimage(
    *,
    sid: _BytesLike,
    app_public_key: _BytesLike,
    wallet_public_key: _BytesLike,
    account_id: str,
    permissions: Optional[ConnectPermissions] = None,
    proof: Optional[ConnectSignInProof] = None,
) -> bytes:
    """Return the canonical byte preimage wallets must sign for approval frames."""

    codec = _require_codec_module()
    payload = codec.build_connect_approve_preimage(
        _ensure_bytes(sid, size=32, field="sid"),
        _ensure_bytes(app_public_key, size=32, field="app_public_key"),
        _ensure_bytes(wallet_public_key, size=32, field="wallet_public_key"),
        account_id,
        permissions.to_dict() if permissions else None,
        proof.to_dict() if proof else None,
    )
    return bytes(payload)

def seal_connect_payload(
    key: _BytesLike,
    sid: _BytesLike,
    *,
    direction: Union[ConnectDirection, str],
    sequence: int,
    payload: ConnectCiphertextPayload,
) -> ConnectFrame:
    """Encrypt a Connect payload and return the resulting ciphertext frame."""

    direction_obj = ConnectDirection.normalize(direction)
    payload_dict = _payload_to_dict(payload)
    frame_bytes = _require_codec_module().seal_connect_payload(
        _ensure_bytes(key, size=32, field="key"),
        _ensure_bytes(sid, size=32, field="sid"),
        direction_obj.value,
        int(sequence),
        payload_dict,
    )
    return ConnectFrame.from_bytes(frame_bytes)


def open_connect_payload(
    key: _BytesLike,
    frame: Union[ConnectFrame, _BytesLike],
) -> ConnectEnvelope:
    """Decrypt a Connect ciphertext frame using the provided direction key."""

    if isinstance(frame, ConnectFrame):
        frame_bytes = frame.to_bytes()
    else:
        frame_bytes = _ensure_bytes(frame, size=None, field="frame")
    envelope_dict = _require_codec_module().open_connect_payload(
        _ensure_bytes(key, size=32, field="key"),
        frame_bytes,
    )
    if not isinstance(envelope_dict, Mapping):
        raise TypeError("connect payload decoder returned unexpected response")
    return ConnectEnvelope.from_dict(envelope_dict)


@dataclass(frozen=True)
class ConnectSessionKeys:
    """Container for per-direction symmetric keys used after Connect approval."""

    app_to_wallet: bytes
    wallet_to_app: bytes

    @classmethod
    def derive(
        cls,
        *,
        local_private_key: _BytesLike,
        peer_public_key: _BytesLike,
        sid: _BytesLike,
    ) -> "ConnectSessionKeys":
        """Derive session keys via X25519 using the provided local key and peer public key."""

        app_key, wallet_key = derive_connect_direction_keys(local_private_key, peer_public_key, sid)
        return cls(app_to_wallet=app_key, wallet_to_app=wallet_key)


@dataclass(frozen=True)
class ConnectSessionState:
    """Serializable snapshot of Connect session counters and replay guards."""

    sid: bytes
    next_sequence_app_to_wallet: int = 1
    next_sequence_wallet_to_app: int = 1
    last_received_app_to_wallet: Optional[int] = None
    last_received_wallet_to_app: Optional[int] = None

    def to_dict(self) -> Dict[str, Any]:
        """Convert the state into a JSON-friendly dictionary."""

        return {
            "sid_base64url": _to_base64url(self.sid),
            "next_sequence": {
                "app_to_wallet": self.next_sequence_app_to_wallet,
                "wallet_to_app": self.next_sequence_wallet_to_app,
            },
            "last_received": {
                "app_to_wallet": self.last_received_app_to_wallet,
                "wallet_to_app": self.last_received_wallet_to_app,
            },
        }

    @classmethod
    def from_dict(cls, payload: Mapping[str, Any]) -> "ConnectSessionState":
        """Restore a state snapshot from :meth:`to_dict` output."""

        if not isinstance(payload, Mapping):
            raise TypeError("ConnectSessionState payload must be a mapping")
        sid_base64url = payload.get("sid_base64url")
        if not isinstance(sid_base64url, str) or not sid_base64url:
            raise TypeError("ConnectSessionState payload must include string sid_base64url")
        try:
            sid = _from_base64url(sid_base64url)
        except ValueError as exc:  # pragma: no cover - defensive
            raise ValueError(
                "ConnectSessionState sid_base64url must be valid base64url"
            ) from exc
        if len(sid) != _SID_LENGTH:
            raise ValueError("ConnectSessionState sid_base64url must decode to 32 bytes")

        next_sequence = payload.get("next_sequence", {})
        if not isinstance(next_sequence, Mapping):
            raise TypeError("ConnectSessionState next_sequence must be a mapping")
        last_received = payload.get("last_received", {})
        if not isinstance(last_received, Mapping):
            raise TypeError("ConnectSessionState last_received must be a mapping")

        def _coerce_int(
            holder: Mapping[str, Any],
            key: str,
            *,
            default: int,
        ) -> int:
            value = holder.get(key, default)
            if not isinstance(value, int):
                raise TypeError(f"ConnectSessionState field `{key}` must be an integer")
            if value < 0:
                raise ValueError(f"ConnectSessionState field `{key}` must be non-negative")
            return value

        def _coerce_optional_int(holder: Mapping[str, Any], key: str) -> Optional[int]:
            value = holder.get(key, None)
            if value is None:
                return None
            if not isinstance(value, int):
                raise TypeError(f"ConnectSessionState field `{key}` must be an integer or null")
            if value < 0:
                raise ValueError(f"ConnectSessionState field `{key}` must be non-negative when present")
            return value

        return cls(
            sid=sid,
            next_sequence_app_to_wallet=_coerce_int(
                next_sequence,
                "app_to_wallet",
                default=1,
            ),
            next_sequence_wallet_to_app=_coerce_int(
                next_sequence,
                "wallet_to_app",
                default=1,
            ),
            last_received_app_to_wallet=_coerce_optional_int(last_received, "app_to_wallet"),
            last_received_wallet_to_app=_coerce_optional_int(last_received, "wallet_to_app"),
        )


class ConnectSession:
    """Manage Connect ciphertext sealing/decryption with monotonic counters."""

    def __init__(
        self,
        *,
        sid: _BytesLike,
        keys: ConnectSessionKeys,
        app_initial_sequence: int = 1,
        wallet_initial_sequence: int = 1,
    ) -> None:
        self._sid = _ensure_bytes(sid, size=32, field="sid")
        self._keys = keys
        self._next_sequence: Dict[ConnectDirection, int] = {
            ConnectDirection.APP_TO_WALLET: int(app_initial_sequence),
            ConnectDirection.WALLET_TO_APP: int(wallet_initial_sequence),
        }
        self._last_received: Dict[ConnectDirection, Optional[int]] = {
            ConnectDirection.APP_TO_WALLET: None,
            ConnectDirection.WALLET_TO_APP: None,
        }

    @property
    def sid(self) -> bytes:
        """Return the session identifier."""

        return self._sid

    def _seal(
        self,
        direction: ConnectDirection,
        payload: ConnectCiphertextPayload,
    ) -> ConnectFrame:
        seq = self._next_sequence[direction]
        _register_stub_session_sequence(self._sid, direction, seq)
        frame = seal_connect_payload(
            self._key_for(direction),
            self._sid,
            direction=direction,
            sequence=seq,
            payload=payload,
        )
        self._next_sequence[direction] += 1
        return frame

    def encrypt_app_to_wallet(self, payload: ConnectCiphertextPayload) -> ConnectFrame:
        """Seal a payload with the App→Wallet key, incrementing the sequence counter."""

        return self._seal(ConnectDirection.APP_TO_WALLET, payload)

    def encrypt_wallet_to_app(self, payload: ConnectCiphertextPayload) -> ConnectFrame:
        """Seal a payload with the Wallet→App key, incrementing the sequence counter."""

        return self._seal(ConnectDirection.WALLET_TO_APP, payload)

    def decrypt(self, frame: Union[ConnectFrame, _BytesLike]) -> ConnectEnvelope:
        """Decrypt a ciphertext frame, enforcing monotonic sequence progression."""

        frame_obj = frame if isinstance(frame, ConnectFrame) else ConnectFrame.from_bytes(frame)
        if frame_obj.control is not None:
            raise ValueError("expected ciphertext frame")
        key = self._key_for(frame_obj.direction)
        envelope = open_connect_payload(key, frame_obj)
        last_seq = self._last_received[frame_obj.direction]
        if last_seq is not None and envelope.sequence <= last_seq:
            raise ValueError("connect sequence must be strictly increasing")
        self._last_received[frame_obj.direction] = envelope.sequence
        return envelope

    def _key_for(self, direction: ConnectDirection) -> bytes:
        if direction == ConnectDirection.APP_TO_WALLET:
            return self._keys.app_to_wallet
        if direction == ConnectDirection.WALLET_TO_APP:
            return self._keys.wallet_to_app
        raise ValueError(f"unsupported direction {direction!r}")

    def snapshot_state(self) -> ConnectSessionState:
        """Capture the current replay guard and sequence counters.

        Use :meth:`from_state` with the same session keys to resume encryption
        after persisting the snapshot.
        """

        return ConnectSessionState(
            sid=self._sid,
            next_sequence_app_to_wallet=self._next_sequence[ConnectDirection.APP_TO_WALLET],
            next_sequence_wallet_to_app=self._next_sequence[ConnectDirection.WALLET_TO_APP],
            last_received_app_to_wallet=self._last_received[ConnectDirection.APP_TO_WALLET],
            last_received_wallet_to_app=self._last_received[ConnectDirection.WALLET_TO_APP],
        )

    @classmethod
    def from_state(
        cls,
        *,
        keys: ConnectSessionKeys,
        state: ConnectSessionState,
    ) -> "ConnectSession":
        """Recreate a session from a :class:`ConnectSessionState` snapshot."""

        session = cls(
            sid=state.sid,
            keys=keys,
            app_initial_sequence=state.next_sequence_app_to_wallet,
            wallet_initial_sequence=state.next_sequence_wallet_to_app,
        )
        session._last_received[ConnectDirection.APP_TO_WALLET] = state.last_received_app_to_wallet
        session._last_received[ConnectDirection.WALLET_TO_APP] = state.last_received_wallet_to_app
        return session


# ---------------------------------------------------------------------------
# Connect session preview helpers
# ---------------------------------------------------------------------------

def generate_connect_sid(
    *,
    chain_id: str,
    app_public_key: _BytesLike,
    nonce: Optional[_BytesLike] = None,
) -> ConnectSid:
    """Derive a deterministic Connect session identifier."""

    normalized_chain = _require_non_empty_string(chain_id, "chain_id")
    public_key = _ensure_bytes(app_public_key, size=32, field="app_public_key")
    if nonce is None:
        nonce_bytes = os.urandom(_NONCE_LENGTH)
    else:
        nonce_bytes = _ensure_bytes(nonce, size=_NONCE_LENGTH, field="nonce")
    hasher = hashlib.blake2b(digest_size=64)
    hasher.update(_SID_PREFIX)
    hasher.update(normalized_chain.encode("utf-8"))
    hasher.update(public_key)
    hasher.update(nonce_bytes)
    digest = hasher.digest()[:_SID_LENGTH]
    sid_base64url = _to_base64url(digest)
    return ConnectSid(
        sid_bytes=bytes(digest),
        sid_base64url=sid_base64url,
        nonce=nonce_bytes,
    )


def create_connect_session_preview(
    *,
    chain_id: str,
    node: Optional[str] = None,
    nonce: Optional[_BytesLike] = None,
    app_key_pair: Optional[ConnectKeyPair] = None,
) -> ConnectSessionPreview:
    """Generate deterministic URIs, SID material, and keypairs for Connect previews."""

    normalized_chain = _require_non_empty_string(chain_id, "chain_id")
    normalized_node = _normalize_optional_string(node, "node")
    key_pair = app_key_pair or generate_connect_keypair()
    sid = generate_connect_sid(
        chain_id=normalized_chain,
        app_public_key=key_pair.public_key,
        nonce=nonce,
    )
    wallet_uri = _build_preview_uri("connect", sid.sid_base64url, normalized_chain, normalized_node)
    app_uri = _build_preview_uri("connect/app", sid.sid_base64url, normalized_chain, normalized_node)
    return ConnectSessionPreview(
        chain_id=normalized_chain,
        node=normalized_node,
        sid_bytes=sid.sid_bytes,
        sid_base64url=sid.sid_base64url,
        nonce=sid.nonce,
        app_key_pair=key_pair,
        wallet_uri=wallet_uri,
        app_uri=app_uri,
    )


def bootstrap_connect_preview_session(
    torii_client: Any,
    *,
    chain_id: str,
    node: Optional[str] = None,
    nonce: Optional[_BytesLike] = None,
    app_key_pair: Optional[ConnectKeyPair] = None,
    register: bool = True,
    session_options: Optional[Mapping[str, Any]] = None,
) -> ConnectPreviewBootstrapResult:
    """Bundle Connect preview generation with optional Torii registration."""

    if not hasattr(torii_client, "create_connect_session"):
        raise TypeError("torii_client must expose create_connect_session()")
    preview = create_connect_session_preview(
        chain_id=chain_id,
        node=node,
        nonce=nonce,
        app_key_pair=app_key_pair,
    )
    if not register:
        return ConnectPreviewBootstrapResult(preview=preview, session=None, tokens=None)
    payload: Dict[str, Any] = {"sid": preview.sid_base64url}
    if session_options is not None:
        if not isinstance(session_options, Mapping):
            raise TypeError("session_options must be a mapping when provided")
        for key, value in session_options.items():
            if key == "node":
                if value is None:
                    continue
                payload["node"] = _require_non_empty_string(str(value), "sessionOptions.node")
            else:
                raise ValueError(f"unsupported session option {key!r}")
    if "node" not in payload and preview.node:
        payload["node"] = preview.node
    session = torii_client.create_connect_session(payload)
    wallet_token = _read_session_token(session, "wallet_token")
    app_token = _read_session_token(session, "app_token")
    tokens = ConnectPreviewTokens(wallet=wallet_token, app=app_token)
    return ConnectPreviewBootstrapResult(preview=preview, session=session, tokens=tokens)


def _build_preview_uri(
    suffix: str,
    sid_base64url: str,
    chain_id: str,
    node: Optional[str],
) -> str:
    params = {
        "sid": sid_base64url,
        "chain_id": chain_id,
        "v": _CONNECT_URI_VERSION,
    }
    if node:
        params["node"] = node
    return f"iroha://{suffix}?{urlencode(params)}"


def _require_non_empty_string(value: str, field: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{field} must be a string")
    normalized = value.strip()
    if not normalized:
        raise ValueError(f"{field} must not be empty")
    return normalized


def _normalize_optional_string(value: Optional[str], field: str) -> Optional[str]:
    if value is None:
        return None
    return _require_non_empty_string(value, field)


def _to_base64url(data: bytes) -> str:
    encoded = base64.urlsafe_b64encode(data)
    return encoded.rstrip(b"=").decode("ascii")

def _from_base64url(value: str) -> bytes:
    normalized = _require_non_empty_string(value, "sid_base64url")
    if "=" in normalized:
        raise ValueError("sid_base64url must not include padding")
    remainder = len(normalized) % 4
    if remainder == 1:
        raise ValueError("sid_base64url has invalid length")
    if remainder:
        normalized = normalized + "=" * (4 - remainder)
    return base64.urlsafe_b64decode(normalized.encode("ascii"))


def _read_session_token(obj: Any, primary: str) -> str:
    if hasattr(obj, primary):
        token = getattr(obj, primary)
    elif isinstance(obj, Mapping):
        token = obj.get(primary)
    else:  # pragma: no cover - defensive
        token = None
    if not isinstance(token, str) or not token:
        raise ValueError(f"session response missing token field {primary!r}")
    return token
