"""High-level SORA Nexus app facade.

The facade is additive: it composes existing Connect, transaction-codec, Torii,
and pipeline-status clients while keeping those lower-level APIs available for
advanced callers.
"""

from __future__ import annotations

import hashlib
from dataclasses import dataclass, replace
from typing import Any, Callable, Mapping, Optional, Protocol, Union

from .crypto import hash_blake2b_32

BytesLike = Union[bytes, bytearray, memoryview, str]


class NexusAppError(RuntimeError):
    """Typed error raised by :class:`NexusAppClient`."""

    def __init__(self, code: str, message: str):
        super().__init__(message)
        self.code = code


@dataclass(frozen=True)
class NexusAppConfig:
    """Static configuration for a Nexus app facade instance."""

    chain_id: str
    authority: Optional[str] = None
    base_url: Optional[str] = None
    node: Optional[str] = None
    signing_public_key: Optional[bytes] = None
    app_metadata: Optional[Mapping[str, Any]] = None


@dataclass(frozen=True)
class NexusConnectOptions:
    """Options used to create a Connect app session."""

    sid: Optional[str] = None
    node: Optional[str] = None


@dataclass(frozen=True)
class NexusConnectSession:
    """Registered Connect session and wallet launch metadata."""

    sid: str
    wallet_launch_uri: str
    app_launch_uri: Optional[str] = None
    token_app: Optional[str] = None
    token_wallet: Optional[str] = None
    token_management: Optional[str] = None
    token_relay: Optional[str] = None
    approved_account: Optional[str] = None
    signing_public_key: Optional[bytes] = None
    app_session: Any = None


@dataclass(frozen=True)
class NexusApprovedAccount:
    """Wallet-approved account plus the updated approved Connect session."""

    account_id: str
    signing_public_key: bytes
    session: NexusConnectSession

    def __iter__(self):
        # Backwards-compatible tuple unpacking: account, session = await_approval(...)
        yield self.account_id
        yield self.session


@dataclass(frozen=True)
class NexusTransferInput:
    """V1 numeric asset transfer input."""

    source_asset_id: str
    quantity: Union[str, int, float]
    destination_account_id: str
    authority: Optional[str] = None
    metadata: Optional[Mapping[str, Any]] = None
    creation_time_ms: Optional[int] = None
    ttl_ms: Optional[int] = None
    nonce: Optional[int] = None


@dataclass(frozen=True)
class NexusSignableTransaction:
    """Canonical transaction payload to sign with a wallet."""

    payload_bytes: bytes
    payload_hash_hex: str
    authority: str
    signing_public_key: Optional[bytes] = None
    signature_algorithm: str = "ed25519"
    native: Any = None


@dataclass(frozen=True)
class NexusTransferDraft:
    """Transfer draft with canonical signable payload."""

    input: NexusTransferInput
    signable: NexusSignableTransaction


@dataclass(frozen=True)
class NexusWalletSignature:
    """Wallet signature response."""

    signature: bytes
    algorithm: str = "ed25519"


@dataclass(frozen=True)
class NexusTransferReceipt:
    """Result returned after transaction finalization/submission."""

    signed_transaction: bytes
    signed_transaction_hash_hex: str
    submission: Any = None
    status: Any = None


class NexusConnectTransport(Protocol):
    """Connect dependency used by :class:`NexusAppClient`."""

    def start_connect(
        self,
        options: NexusConnectOptions,
        config: NexusAppConfig,
    ) -> NexusConnectSession: ...

    def await_approval(
        self,
        session: NexusConnectSession,
        config: NexusAppConfig,
    ) -> Mapping[str, Any]: ...

    def request_signature(
        self,
        session: NexusConnectSession,
        signable: NexusSignableTransaction,
        config: NexusAppConfig,
    ) -> Union[NexusWalletSignature, Mapping[str, Any], BytesLike]: ...


@dataclass
class _DefaultConnectState:
    preview: Any
    torii_client: Any
    ws: Any = None
    connect_session: Any = None
    approved_account: Optional[str] = None
    signing_public_key: Optional[bytes] = None


def _bytes(value: BytesLike, field: str) -> bytes:
    if isinstance(value, bytes):
        return value
    if isinstance(value, (bytearray, memoryview)):
        return bytes(value)
    if isinstance(value, str):
        raw = value[2:] if value.startswith("0x") else value
        if len(raw) % 2 == 0:
            try:
                return bytes.fromhex(raw)
            except ValueError:
                pass
    raise TypeError(f"{field} must be bytes or a hex string")


def _payload_hash_hex(payload_bytes: bytes) -> str:
    return hash_blake2b_32(payload_bytes).hex()


def _account_ed25519_public_key(account_id: str) -> bytes:
    from .address import AccountAddress, CurveId

    try:
        address = AccountAddress.from_i105(account_id)
    except Exception as exc:
        raise NexusAppError(
            "missing_signing_public_key",
            "approved account must be a canonical single-key Ed25519 I105 account",
        ) from exc
    controller = address.controller
    if controller.tag != controller.CONTROLLER_SINGLE_KEY_TAG or controller.curve != CurveId.ED25519:
        raise NexusAppError(
            "missing_signing_public_key",
            "approved account must be a canonical single-key Ed25519 I105 account",
        )
    public_key = bytes(controller.public_key)
    return _validate_ed25519_public_key(public_key, "approved Ed25519 public key")


def _validate_ed25519_public_key(value: BytesLike, field: str) -> bytes:
    public_key = _bytes(value, field)
    if len(public_key) != 32:
        raise NexusAppError("invalid_signing_public_key", f"{field} must be 32 bytes")
    return public_key


def _resolve_signing_public_key(authority: str, explicit: Optional[BytesLike]) -> bytes:
    if explicit is not None:
        return _validate_ed25519_public_key(explicit, "signing_public_key")
    return _account_ed25519_public_key(authority)


def _normalize_algorithm(algorithm: Any) -> str:
    if algorithm is None:
        return "ed25519"
    if not isinstance(algorithm, str):
        raise NexusAppError(
            "unsupported_signature_algorithm",
            f"unsupported signature algorithm {algorithm}",
        )
    if not algorithm or any(ord(ch) < 0x20 or ord(ch) > 0x7E for ch in algorithm):
        raise NexusAppError(
            "unsupported_signature_algorithm",
            f"unsupported signature algorithm {algorithm}",
        )
    if algorithm != algorithm.strip():
        raise NexusAppError(
            "unsupported_signature_algorithm",
            f"unsupported signature algorithm {algorithm}",
        )
    if algorithm not in {
        "ed25519",
        "0",
    }:
        raise NexusAppError(
            "unsupported_signature_algorithm",
            f"unsupported signature algorithm {algorithm}",
        )
    return "ed25519"


def _submission_hash_hex(submission: Any) -> Optional[str]:
    if submission is None:
        return None
    if isinstance(submission, Mapping):
        payload = submission.get("payload")
        candidates = (
            submission.get("hash_hex"),
            submission.get("hashHex"),
            submission.get("hash"),
            submission.get("tx_hash"),
            submission.get("txHash"),
            submission.get("transaction_hash_hex"),
            submission.get("transactionHashHex"),
            submission.get("signed_transaction_hash"),
            submission.get("signedTransactionHash"),
            payload.get("tx_hash") if isinstance(payload, Mapping) else None,
            payload.get("txHash") if isinstance(payload, Mapping) else None,
            payload.get("hash") if isinstance(payload, Mapping) else None,
            payload.get("signed_transaction_hash") if isinstance(payload, Mapping) else None,
            payload.get("signedTransactionHash") if isinstance(payload, Mapping) else None,
        )
        for candidate in candidates:
            if candidate:
                return _bytes(candidate, "submission.hash").hex()
    for attr in (
        "hash_hex",
        "hashHex",
        "hash",
        "tx_hash",
        "txHash",
        "transaction_hash_hex",
        "transactionHashHex",
        "signed_transaction_hash",
        "signedTransactionHash",
    ):
        candidate = getattr(submission, attr, None)
        if candidate:
            return _bytes(candidate, "submission.hash").hex()
    return None


def _tagged_connect_field(tag: str, value: bytes) -> bytes:
    tag_bytes = tag.encode("utf-8")
    return len(tag_bytes).to_bytes(2, "little") + tag_bytes + len(value).to_bytes(8, "little") + value


def _connect_relay_auth_hash(sid: bytes, relay_token: str) -> bytes:
    hasher = hashlib.sha256()
    hasher.update(b"iroha-connect|relay-auth|v1")
    hasher.update(sid)
    hasher.update(relay_token.encode("utf-8"))
    return hasher.digest()


def _connect_approval_preimage(
    *,
    sid: bytes,
    app_public_key: bytes,
    wallet_public_key: bytes,
    account_id: str,
    relay_token: Optional[str],
) -> bytes:
    parts = [
        _tagged_connect_field("domain", b"iroha-connect|approve|v1"),
        _tagged_connect_field("sid", sid),
        _tagged_connect_field("app_pk", app_public_key),
        _tagged_connect_field("wallet_pk", wallet_public_key),
        _tagged_connect_field("account_id", account_id.encode("utf-8")),
    ]
    if relay_token:
        parts.append(_tagged_connect_field("relay_auth", _connect_relay_auth_hash(sid, relay_token)))
    return b"".join(parts)


def _normalize_signature(value: Union[NexusWalletSignature, Mapping[str, Any], BytesLike]) -> NexusWalletSignature:
    if isinstance(value, NexusWalletSignature):
        signature = value
    elif isinstance(value, Mapping):
        algorithm = _normalize_algorithm(value.get("algorithm", "ed25519"))
        payload = value.get("signature", value.get("bytes", value.get("payload")))
        signature = NexusWalletSignature(_bytes(payload, "signature"), algorithm)
    else:
        signature = NexusWalletSignature(_bytes(value, "signature"), "ed25519")
    _normalize_algorithm(signature.algorithm)
    if len(signature.signature) != 64:
        raise NexusAppError(
            "invalid_signature",
            f"Ed25519 signature must be 64 bytes, got {len(signature.signature)}",
        )
    return NexusWalletSignature(signature.signature, "ed25519")


def _validate_ed25519_signature_for_payload(
    public_key: bytes,
    payload_bytes: bytes,
    signature: bytes,
) -> None:
    from .crypto import verify_ed25519

    try:
        verified = verify_ed25519(public_key, hash_blake2b_32(payload_bytes), signature)
    except Exception as exc:
        raise NexusAppError(
            "invalid_signature",
            "Ed25519 signature does not verify for the signable payload",
        ) from exc
    if not verified:
        raise NexusAppError(
            "invalid_signature",
            "Ed25519 signature does not verify for the signable payload",
        )


class DefaultNexusTransactionCodec:
    """Default transaction codec backed by the Python SDK's native Norito builder."""

    def build_transfer_payload(self, payload_input: Mapping[str, Any]) -> Mapping[str, Any]:
        from .tx import TransactionConfig, TransactionDraft

        draft = TransactionDraft(
            TransactionConfig(
                chain_id=str(payload_input["chain_id"]),
                authority=str(payload_input["authority"]),
                creation_time_ms=payload_input.get("creation_time_ms"),
                ttl_ms=payload_input.get("ttl_ms"),
                nonce=payload_input.get("nonce"),
                metadata=payload_input.get("metadata"),
            )
        )
        draft.transfer_asset_numeric(
            str(payload_input.get("source_asset_id", payload_input.get("sourceAssetId"))),
            payload_input["quantity"],
            str(payload_input.get("destination_account_id", payload_input.get("destinationAccountId"))),
        )
        builder = draft.to_builder()
        return {
            "payload_bytes": bytes(builder.encode_payload()),
            "payload_hash_hex": builder.payload_hash_hex(),
            "native": builder,
        }

    def payload_hash_hex(self, payload_bytes: BytesLike) -> str:
        return _payload_hash_hex(_bytes(payload_bytes, "payload_bytes"))

    def finalize_signed_transaction(
        self,
        signable: NexusSignableTransaction,
        signature: NexusWalletSignature,
        signing_public_key: bytes,
    ) -> Mapping[str, Any]:
        _ = signing_public_key
        builder = signable.native
        if builder is None or not hasattr(builder, "build_with_signature"):
            raise NexusAppError(
                "transaction_codec_unavailable",
                "native transaction builder is required to finalize a wallet-signed transaction",
            )
        try:
            envelope = builder.build_with_signature(signature.signature)
        except Exception as exc:  # pragma: no cover - native error formatting
            raise NexusAppError("invalid_signature", str(exc)) from exc
        return {
            "signed_transaction": bytes(envelope.signed_transaction_versioned),
            "hash_hex": envelope.hash_hex(),
            "envelope": envelope,
        }


class DefaultNexusConnectTransport:
    """Default app-role Connect transport using `ToriiClient` and Connect frame helpers."""

    def start_connect(
        self,
        options: NexusConnectOptions,
        config: NexusAppConfig,
    ) -> NexusConnectSession:
        if not config.base_url:
            raise NexusAppError("connect_transport_unavailable", "config.base_url is required for Connect")
        from .client import ToriiClient
        from .connect import bootstrap_connect_preview_session

        torii_client = ToriiClient(config.base_url)
        bootstrap = bootstrap_connect_preview_session(
            torii_client,
            chain_id=config.chain_id,
            node=options.node or config.node,
            register=True,
        )
        if bootstrap.tokens is None:
            raise NexusAppError("connect_transport_unavailable", "Connect session registration failed")
        state = _DefaultConnectState(preview=bootstrap.preview, torii_client=torii_client)
        return NexusConnectSession(
            sid=bootstrap.preview.sid_base64url,
            wallet_launch_uri=bootstrap.preview.wallet_uri,
            app_launch_uri=bootstrap.preview.app_uri,
            token_app=bootstrap.tokens.app,
            token_wallet=bootstrap.tokens.wallet,
            token_management=bootstrap.tokens.management,
            token_relay=bootstrap.tokens.relay,
            app_session=state,
        )

    def await_approval(
        self,
        session: NexusConnectSession,
        config: NexusAppConfig,
    ) -> Mapping[str, Any]:
        state = self._state(session)
        from .connect import (
            ConnectControlApprove,
            ConnectControlOpen,
            ConnectDirection,
            ConnectFrame,
            ConnectPermissions,
            ConnectSession,
            ConnectSessionKeys,
        )
        from .crypto import verify_ed25519

        ws = self._websocket(session, state)
        permissions = ConnectPermissions(methods=["SIGN_REQUEST_TX"], events=[])
        metadata = self._metadata(config)
        open_frame = ConnectFrame(
            sid=state.preview.sid_bytes,
            direction=ConnectDirection.APP_TO_WALLET,
            sequence=1,
            control=ConnectControlOpen(
                app_public_key=state.preview.app_key_pair.public_key,
                chain_id=config.chain_id,
                permissions=permissions,
                metadata=metadata,
            ),
        )
        self._send_binary(ws, open_frame.to_bytes())

        while True:
            frame = ConnectFrame.from_bytes(self._recv_bytes(ws))
            if not isinstance(frame.control, ConnectControlApprove):
                continue
            approval = frame.control
            try:
                _normalize_algorithm(approval.algorithm)
            except NexusAppError as exc:
                raise NexusAppError(
                    "unsupported_signature_algorithm",
                    f"unsupported Connect approval signature algorithm {approval.algorithm}",
                ) from exc
            signing_public_key = config.signing_public_key or _account_ed25519_public_key(
                approval.account_id
            )
            preimage = _connect_approval_preimage(
                sid=state.preview.sid_bytes,
                app_public_key=state.preview.app_key_pair.public_key,
                wallet_public_key=approval.wallet_public_key,
                account_id=approval.account_id,
                relay_token=session.token_relay,
            )
            if not verify_ed25519(signing_public_key, preimage, approval.signature):
                raise NexusAppError(
                    "connect_approval_invalid",
                    "Connect approval signature verification failed",
                )
            keys = ConnectSessionKeys.derive(
                local_private_key=state.preview.app_key_pair.private_key,
                peer_public_key=approval.wallet_public_key,
                sid=state.preview.sid_bytes,
            )
            state.connect_session = ConnectSession(sid=state.preview.sid_bytes, keys=keys)
            state.approved_account = approval.account_id
            state.signing_public_key = bytes(signing_public_key)
            return {
                "account_id": approval.account_id,
                "signing_public_key": bytes(signing_public_key),
            }

    def request_signature(
        self,
        session: NexusConnectSession,
        signable: NexusSignableTransaction,
        config: NexusAppConfig,
    ) -> NexusWalletSignature:
        _ = config
        state = self._state(session)
        if state.connect_session is None:
            raise NexusAppError(
                "connect_approval_required",
                "await_approval must complete before requesting a wallet signature",
            )
        from .connect import ConnectSignRequestTxPayload, ConnectSignResultErrPayload, ConnectSignResultOkPayload

        ws = self._websocket(session, state)
        request = state.connect_session.encrypt_app_to_wallet(
            ConnectSignRequestTxPayload(tx_bytes=signable.payload_bytes)
        )
        self._send_binary(ws, request.to_bytes())

        while True:
            envelope = state.connect_session.decrypt(self._recv_bytes(ws))
            if isinstance(envelope.payload, ConnectSignResultOkPayload):
                return NexusWalletSignature(
                    signature=bytes(envelope.payload.signature),
                    algorithm=envelope.payload.algorithm,
                )
            if isinstance(envelope.payload, ConnectSignResultErrPayload):
                raise NexusAppError("connect_signature_rejected", envelope.payload.message)

    @staticmethod
    def _state(session: NexusConnectSession) -> _DefaultConnectState:
        if not isinstance(session.app_session, _DefaultConnectState):
            raise NexusAppError(
                "connect_transport_unavailable",
                "session was not created by DefaultNexusConnectTransport",
            )
        return session.app_session

    @staticmethod
    def _metadata(config: NexusAppConfig) -> Any:
        if not config.app_metadata:
            return None
        from .connect import ConnectAppMetadata

        return ConnectAppMetadata.from_dict(dict(config.app_metadata))

    @staticmethod
    def _websocket(session: NexusConnectSession, state: _DefaultConnectState) -> Any:
        if state.ws is None:
            if not session.token_app:
                raise NexusAppError("connect_transport_unavailable", "session token_app is missing")
            state.ws = state.torii_client.connect_websocket(session.sid, "app", session.token_app)
        return state.ws

    @staticmethod
    def _send_binary(ws: Any, payload: bytes) -> None:
        if hasattr(ws, "send_binary"):
            ws.send_binary(payload)
        else:
            ws.send(payload)

    @staticmethod
    def _recv_bytes(ws: Any) -> bytes:
        message = ws.recv()
        if isinstance(message, bytes):
            return message
        if isinstance(message, bytearray):
            return bytes(message)
        if isinstance(message, memoryview):
            return message.tobytes()
        if isinstance(message, str):
            return bytes.fromhex(message)
        raise TypeError(f"unsupported Connect WebSocket message type {type(message)!r}")


class NexusAppClient:
    """App-developer-friendly facade over Connect, transaction signing, and Torii."""

    def __init__(
        self,
        config: NexusAppConfig,
        *,
        connect_transport: Optional[NexusConnectTransport] = None,
        transaction_codec: Any = None,
        torii_client: Any = None,
    ) -> None:
        self.config = config
        self.connect_transport = connect_transport or (
            DefaultNexusConnectTransport() if config.base_url else None
        )
        self.transaction_codec = transaction_codec or DefaultNexusTransactionCodec()
        if torii_client is None and config.base_url:
            from .client import ToriiClient

            torii_client = ToriiClient(config.base_url)
        self.torii_client = torii_client

    def start_connect(self, options: Optional[NexusConnectOptions] = None) -> NexusConnectSession:
        """Create a Connect app session and return wallet launch metadata."""

        if self.connect_transport is None:
            raise NexusAppError("connect_transport_unavailable", "Connect transport is required")
        return self.connect_transport.start_connect(options or NexusConnectOptions(), self.config)

    def await_approval(self, session: NexusConnectSession) -> NexusApprovedAccount:
        """Wait for wallet approval and return the approved account plus updated session."""

        if self.connect_transport is None:
            raise NexusAppError("connect_transport_unavailable", "Connect transport is required")
        approved = self.connect_transport.await_approval(session, self.config)
        account = str(approved.get("account_id", approved.get("accountId", ""))).strip()
        if not account:
            raise NexusAppError("approval_missing_account", "wallet approval did not include an account")
        signing_public_key = approved.get("signing_public_key", approved.get("signingPublicKey"))
        signing_public_key_bytes = (
            _validate_ed25519_public_key(signing_public_key, "signing_public_key")
            if signing_public_key is not None
            else (
                _validate_ed25519_public_key(self.config.signing_public_key, "config.signing_public_key")
                if self.config.signing_public_key is not None
                else _account_ed25519_public_key(account)
            )
        )
        updated = replace(
            session,
            approved_account=account,
            signing_public_key=bytes(signing_public_key_bytes),
        )
        return NexusApprovedAccount(account, bytes(signing_public_key_bytes), updated)

    def build_transfer_draft(self, input: NexusTransferInput) -> NexusTransferDraft:
        """Build a canonical signable transfer payload."""

        authority = input.authority or self.config.authority
        if not authority:
            raise NexusAppError("missing_authority", "transfer authority is required")
        if self.transaction_codec is None or not hasattr(self.transaction_codec, "build_transfer_payload"):
            raise NexusAppError(
                "transaction_codec_unavailable",
                "transaction codec with build_transfer_payload is required",
            )
        payload_input = {
            "chain_id": self.config.chain_id,
            "authority": authority,
            "source_asset_id": input.source_asset_id,
            "quantity": str(input.quantity),
            "destination_account_id": input.destination_account_id,
            "metadata": input.metadata,
            "creation_time_ms": input.creation_time_ms,
            "ttl_ms": input.ttl_ms,
            "nonce": input.nonce,
        }
        signing_public_key = _resolve_signing_public_key(authority, self.config.signing_public_key)
        payload_result = self.transaction_codec.build_transfer_payload(payload_input)
        if isinstance(payload_result, Mapping):
            payload_bytes = _bytes(
                payload_result.get("payload_bytes", payload_result.get("payloadBytes")),
                "payload_bytes",
            )
            payload_hash_hex = str(
                payload_result.get(
                    "payload_hash_hex",
                    payload_result.get("payloadHashHex", _payload_hash_hex(payload_bytes)),
                )
            )
            native = payload_result.get("native")
        else:
            payload_bytes = _bytes(payload_result, "payload_bytes")
            payload_hash_hex = _payload_hash_hex(payload_bytes)
            native = getattr(payload_result, "native", None)
        signable = NexusSignableTransaction(
            payload_bytes=payload_bytes,
            payload_hash_hex=payload_hash_hex,
            authority=authority,
            signing_public_key=signing_public_key,
            native=native,
        )
        return NexusTransferDraft(replace(input, authority=authority), signable)

    def request_signature(
        self,
        session: NexusConnectSession,
        signable: NexusSignableTransaction,
    ) -> NexusWalletSignature:
        """Request a wallet signature for the canonical transaction payload."""

        if self.connect_transport is None:
            raise NexusAppError("connect_transport_unavailable", "Connect transport is required")
        _normalize_algorithm(signable.signature_algorithm)
        response = self.connect_transport.request_signature(session, signable, self.config)
        return _normalize_signature(response)

    def finalize_and_submit(
        self,
        signable: NexusSignableTransaction,
        signature: Union[NexusWalletSignature, Mapping[str, Any], BytesLike],
        *,
        wait: bool = True,
        wait_options: Optional[Mapping[str, Any]] = None,
    ) -> NexusTransferReceipt:
        """Finalize the signed transaction, submit it to Torii, and optionally wait for status."""

        _normalize_algorithm(signable.signature_algorithm)
        normalized = _normalize_signature(signature)
        if self.transaction_codec is None or not hasattr(self.transaction_codec, "finalize_signed_transaction"):
            raise NexusAppError(
                "transaction_codec_unavailable",
                "transaction codec with finalize_signed_transaction is required",
            )
        signing_public_key = _resolve_signing_public_key(
            signable.authority,
            signable.signing_public_key
            if signable.signing_public_key is not None
            else self.config.signing_public_key,
        )
        _validate_ed25519_signature_for_payload(
            signing_public_key,
            signable.payload_bytes,
            normalized.signature,
        )
        finalized = self.transaction_codec.finalize_signed_transaction(
            signable,
            normalized,
            signing_public_key,
        )
        if isinstance(finalized, Mapping):
            signed_transaction = _bytes(
                finalized.get("signed_transaction", finalized.get("signedTransaction")),
                "signed_transaction",
            )
            hash_hex = str(
                finalized.get(
                    "hash_hex",
                    finalized.get("hashHex", _payload_hash_hex(signed_transaction)),
                )
            )
        else:
            signed_transaction = _bytes(finalized, "signed_transaction")
            hash_hex = _payload_hash_hex(signed_transaction)

        submission = None
        status = None
        if self.torii_client is not None:
            try:
                submission = self.torii_client.submit_transaction(signed_transaction)
            except Exception as exc:  # pragma: no cover - transport dependent
                raise NexusAppError("submit_failed", str(exc)) from exc
            submitted_hash_hex = _submission_hash_hex(submission)
            if submitted_hash_hex and submitted_hash_hex != hash_hex:
                raise NexusAppError(
                    "transaction_hash_mismatch",
                    f"Torii returned transaction hash {submitted_hash_hex} but local hash is {hash_hex}",
                )
            if wait and hasattr(self.torii_client, "wait_for_transaction_status"):
                try:
                    options = dict(wait_options or {})
                    status = self.torii_client.wait_for_transaction_status(hash_hex, **options)
                except Exception as exc:  # pragma: no cover - transport dependent
                    raise NexusAppError("status_wait_failed", str(exc)) from exc
        else:
            raise NexusAppError(
                "torii_client_unavailable",
                "Torii client is required to submit the signed transaction",
            )

        return NexusTransferReceipt(
            signed_transaction=signed_transaction,
            signed_transaction_hash_hex=hash_hex,
            submission=submission,
            status=status,
        )

    def transfer_with_wallet(
        self,
        session: NexusConnectSession,
        input: NexusTransferInput,
        *,
        wait: bool = True,
        wait_options: Optional[Mapping[str, Any]] = None,
    ) -> NexusTransferReceipt:
        """One-call transfer wrapper over draft, signature, finalization, submit, and wait."""

        authority = input.authority or session.approved_account or self.config.authority
        if not authority:
            raise NexusAppError("missing_authority", "transfer authority is required")
        if session.approved_account and input.authority and session.approved_account != input.authority:
            raise NexusAppError(
                "approval_account_mismatch",
                "transfer authority does not match the approved wallet account",
            )
        draft = self.build_transfer_draft(replace(input, authority=authority))
        signable = replace(
            draft.signable,
            signing_public_key=session.signing_public_key or draft.signable.signing_public_key,
        )
        signature = self.request_signature(session, signable)
        return self.finalize_and_submit(
            signable,
            signature,
            wait=wait,
            wait_options=wait_options,
        )

    # CamelCase aliases for parity with JS/Swift/Kotlin naming in samples.
    startConnect: Callable[[Optional[NexusConnectOptions]], NexusConnectSession] = start_connect
    awaitApproval: Callable[[NexusConnectSession], NexusApprovedAccount] = await_approval
    buildTransferDraft: Callable[[NexusTransferInput], NexusTransferDraft] = build_transfer_draft
    requestSignature: Callable[[NexusConnectSession, NexusSignableTransaction], NexusWalletSignature] = request_signature
    finalizeAndSubmit = finalize_and_submit
    transferWithWallet = transfer_with_wallet


__all__ = [
    "NexusAppClient",
    "NexusAppConfig",
    "NexusAppError",
    "NexusConnectOptions",
    "NexusConnectSession",
    "NexusApprovedAccount",
    "DefaultNexusConnectTransport",
    "DefaultNexusTransactionCodec",
    "NexusSignableTransaction",
    "NexusTransferDraft",
    "NexusTransferInput",
    "NexusTransferReceipt",
    "NexusWalletSignature",
]
