"""Authenticated controller for the Rust-owned privacy wallet worker.

The controller deliberately has no API accepting an owner bundle or witness
bytes.  It sends an absolute owner-only file path to the worker, then deals
only in opaque handles, canonical public intent/plan bytes, and public signed
transaction results.  The Rust process owns all credential reads, decoding,
proving, signing, single-use custody, and zeroization.

This IPWW controller is closed over the eleven generic protocols plus one
purpose-separated atomic private-settlement command family. ZK-X509 uses its
separately authenticated, profile-owned worker transport and is intentionally
rejected here rather than routed through a generic bundle.

Release qualification must also reject any wheel that still exposes a direct
``execution_bundle`` PyO3 transaction-builder argument.  This controller does
not wrap, emulate, or silently fall back to that legacy in-process custody
path.
"""

from __future__ import annotations

import hashlib
import hmac
import os
import secrets
import stat
import struct
import subprocess
import threading
from dataclasses import dataclass
from enum import IntEnum
from pathlib import Path
from typing import IO

PRIVACY_WALLET_WORKER_PROTOCOL_VERSION_V1 = 1
PRIVACY_WALLET_WORKER_MAX_FRAME_BYTES_V1 = 34 * 1_024 * 1_024
PRIVACY_WALLET_WORKER_MAX_PUBLIC_INTENT_BYTES_V1 = 524_288
PRIVACY_WALLET_WORKER_MAX_EXECUTION_PLAN_BYTES_V1 = 2 * 1_024 * 1_024
PRIVACY_WALLET_WORKER_MAX_SETTLEMENT_PUBLIC_OBJECT_BYTES_V1 = 4 * 1_024 * 1_024
PRIVACY_WALLET_WORKER_MAX_SETTLEMENT_PROOF_BYTES_V1 = 8 * 1_024 * 1_024
PRIVACY_WALLET_WORKER_MIN_TTL_MILLIS_V1 = 1_000
PRIVACY_WALLET_WORKER_MAX_TTL_MILLIS_V1 = 15 * 60 * 1_000
ATOMIC_PRIVATE_SETTLEMENT_PROTOCOL_LABEL_V1 = "atomic-private-settlement-v1"

_MAGIC_V1 = b"IPWW"
_AUTH_KEY_BYTES_V1 = 32
_AUTH_TAG_BYTES_V1 = 32
_HANDLE_BYTES_V1 = 32
_DIGEST_BYTES_V1 = 32
_MAX_SIGNER_BYTES_V1 = 512
_MAX_PROTOCOL_BYTES_V1 = 96
_MAX_PATH_BYTES_V1 = 4_096
_MAX_OPERATION_SCHEMA_BYTES_V1 = 128
_MAX_RESPONSE_TEXT_BYTES_V1 = 512
_MAX_PUBLIC_KEY_BYTES_V1 = 4_096
_MAX_SIGNATURE_BYTES_V1 = 4_096
_MAX_WORKER_BINARY_BYTES_V1 = 512 * 1_024 * 1_024
_U64_MAX = (1 << 64) - 1


PRIVACY_GENERIC11_WORKER_OPERATION_SCHEMAS_V1: dict[str, str] = {
    "zk-ace-pq-authorization-v1": "zk_ace_authorization_action_v1",
    "anonymous-pgc-k-out-of-n-v1": "anonymous_pgc_payment_action_v1",
    "verange-transparent-range-v1": "verange_range_proof_v1",
    "iroha-zk-ams-v1": "zk_ams_admission_and_provisioning_v1",
    "vega-existing-credential-zk-v1": "vega_credential_presentation_v1",
    "iroha-jindo-polynomial-commitment-v1": "jindo_polynomial_evaluation_v1",
    "iroha-bootle-lantern-anoncred-v1": "bootle_lantern_credential_presentation_v1",
    "orchard-halo2-actions-v1": "orchard_note_action_v1",
    "monero-fcmp-plus-plus-v1": "fcmp_membership_payment_v1",
    "iroha-ivm-private-note-stark-v1": "ivm_private_note_action_v1",
    "pq-masp-stark-v1": "pq_masp_note_action_v1",
}


class PrivacyWalletWorkerCommandV1(IntEnum):
    """Closed IPWW v1 command registry."""

    PING = 1
    IMPORT = 2
    INSPECT = 3
    CANCEL = 4
    EXECUTE = 5
    IMPORT_PRIVATE_SETTLEMENT = 6
    INSPECT_PRIVATE_SETTLEMENT = 7
    CANCEL_PRIVATE_SETTLEMENT = 8
    PROVE_PRIVATE_SETTLEMENT = 9


class PrivacyWalletWorkerErrorCodeV1(IntEnum):
    """Stable Rust-owned non-secret error codes."""

    AUTHENTICATION_FAILED = 1
    CAPACITY_EXCEEDED = 2
    CREDENTIAL_CHANGED_DURING_IMPORT = 3
    CREDENTIAL_FILE_EMPTY = 4
    CREDENTIAL_FILE_INSECURE = 5
    CREDENTIAL_FILE_TOO_LARGE = 6
    ENTROPY_UNAVAILABLE = 7
    EXPIRED = 8
    FRAME_TOO_LARGE = 9
    INVALID_BINDING = 10
    INVALID_CREDENTIAL_PATH = 11
    INVALID_FRAME = 12
    INVALID_HANDLE = 13
    INVALID_PAYLOAD = 14
    INVALID_TTL = 15
    IO = 16
    PUBLIC_INTENT_DIGEST_MISMATCH = 17
    REPLAY_OR_OUT_OF_ORDER = 18
    UNKNOWN_COMMAND = 19
    UNKNOWN_HANDLE = 20
    UNSUPPORTED_PROTOCOL = 21
    WRONG_BINDING = 22
    INVALID_PUBLIC_INTENT = 23
    CLOCK_UNAVAILABLE = 24
    INVALID_EXECUTION_BUNDLE = 25
    INVALID_EXECUTION_PLAN = 26
    NATIVE_ACTION_FAILED = 27
    NATIVE_SELF_INSPECTION_FAILED = 28
    PUBLIC_ACTION_MISMATCH = 29
    INVALID_PRIVATE_SETTLEMENT_BUNDLE = 30
    NATIVE_PRIVATE_SETTLEMENT_PROOF_FAILED = 31


class PrivacyWalletWorkerErrorV1(RuntimeError):
    """Local fail-closed controller or authenticated-wire rejection."""


class PrivacyWalletWorkerRemoteErrorV1(PrivacyWalletWorkerErrorV1):
    """Typed non-secret error returned by the authenticated Rust worker."""

    def __init__(self, code: PrivacyWalletWorkerErrorCodeV1, message: str) -> None:
        super().__init__(f"privacy wallet worker rejected request ({int(code)}): {message}")
        self.code = code
        self.remote_message = message


@dataclass(frozen=True)
class PrivacyWalletWitnessBindingV1:
    """Public release, roster, network, intent, and replay binding."""

    network_id: bytes
    signer_wallet_id: str
    protocol_id: str
    compiled_profile_digest: bytes
    public_intent_digest: bytes
    nonce: bytes
    signed_release_authority_digest: bytes

    def __post_init__(self) -> None:
        _require_text(self.signer_wallet_id, _MAX_SIGNER_BYTES_V1, "signer_wallet_id")
        _require_text(self.protocol_id, _MAX_PROTOCOL_BYTES_V1, "protocol_id")
        if (
            self.protocol_id not in PRIVACY_GENERIC11_WORKER_OPERATION_SCHEMAS_V1
            and self.protocol_id != ATOMIC_PRIVATE_SETTLEMENT_PROTOCOL_LABEL_V1
        ):
            raise ValueError("protocol_id is not in the closed worker registry")
        for field_name in (
            "network_id",
            "compiled_profile_digest",
            "public_intent_digest",
            "nonce",
            "signed_release_authority_digest",
        ):
            _require_nonzero_bytes(getattr(self, field_name), _DIGEST_BYTES_V1, field_name)
        if (
            self.protocol_id != ATOMIC_PRIVATE_SETTLEMENT_PROTOCOL_LABEL_V1
            and self.network_id[-1] & 1 != 1
        ):
            raise ValueError("network_id must carry the canonical Iroha hash marker bit")


@dataclass(frozen=True)
class PrivacyWalletWitnessHandleV1:
    """Opaque process-local single-use handle; never a witness encoding."""

    value: bytes

    def __post_init__(self) -> None:
        _require_nonzero_bytes(self.value, _HANDLE_BYTES_V1, "witness handle")

    @property
    def hex(self) -> str:
        return self.value.hex()


@dataclass(frozen=True)
class PrivacyWalletWitnessLeaseV1:
    """Public inspection of one Rust-custodied owner bundle."""

    handle: PrivacyWalletWitnessHandleV1
    expires_at_millis: int
    wallet_id: str
    authority: str
    authority_public_key: str
    protocol_id: str
    operation_schema: str


@dataclass(frozen=True)
class PrivateSettlementWalletLeaseV1:
    """Public inspection of one Rust-custodied settlement witness bundle."""

    handle: PrivacyWalletWitnessHandleV1
    expires_at_millis: int
    wallet_id: str
    proof_binding_digest: bytes
    statement_digest: bytes
    capsule_digest: bytes
    audit_plaintext_commitment: bytes


@dataclass(frozen=True, repr=False)
class PrivateSettlementPreparedProofV1:
    """Public proof, canonical delta, and capsule returned by native proving."""

    wallet_id: str
    canonical_genesis_hash: bytes
    proof_binding_digest: bytes
    statement_digest: bytes
    capsule_digest: bytes
    audit_plaintext_commitment: bytes
    statement_norito: bytes
    proof: bytes
    delta_norito: bytes
    audit_capsule_norito: bytes

    def __repr__(self) -> str:
        """Return a log-safe representation without proof or capsule bytes."""

        return "PrivateSettlementPreparedProofV1(<restricted>)"


@dataclass(frozen=True)
class PrivacyWalletSignedActionV1:
    """Public signed result returned after terminal witness consumption."""

    protocol_id: str
    operation_schema: str
    network_id: bytes
    authority: str
    authority_public_key: str
    adaptive_signed_transaction: bytes
    versioned_signed_transaction: bytes
    signature: bytes
    public_key: bytes
    transaction_hash: bytes
    transaction_intent_digest: bytes
    statement_digest: bytes
    proof_envelope_hash: bytes
    statement_bytes: int
    proof_bytes: int
    encoded_proof_envelope_bytes: int
    adaptive_signed_transaction_bytes: int
    submitted_versioned_transaction_bytes: int


class _Cursor:
    __slots__ = ("_source", "_offset")

    def __init__(self, source: bytes) -> None:
        self._source = memoryview(source)
        self._offset = 0

    def take(self, count: int) -> bytes:
        if count < 0 or self._offset + count > len(self._source):
            raise PrivacyWalletWorkerErrorV1("worker response is truncated")
        value = bytes(self._source[self._offset : self._offset + count])
        self._offset += count
        return value

    def u8(self) -> int:
        return self.take(1)[0]

    def u16(self) -> int:
        return struct.unpack(">H", self.take(2))[0]

    def u32(self) -> int:
        return struct.unpack(">I", self.take(4))[0]

    def u64(self) -> int:
        return struct.unpack(">Q", self.take(8))[0]

    def text(self, maximum: int, label: str) -> str:
        length = self.u16()
        if not 1 <= length <= maximum:
            raise PrivacyWalletWorkerErrorV1(f"worker {label} has an invalid length")
        try:
            value = self.take(length).decode("utf-8")
        except UnicodeDecodeError as error:
            raise PrivacyWalletWorkerErrorV1(f"worker {label} is not UTF-8") from error
        _require_text(value, maximum, f"worker {label}")
        return value

    def bytes_u16(self, maximum: int, label: str) -> bytes:
        length = self.u16()
        if not 1 <= length <= maximum:
            raise PrivacyWalletWorkerErrorV1(f"worker {label} has an invalid length")
        return self.take(length)

    def bytes_u32(self, maximum: int, label: str) -> bytes:
        length = self.u32()
        if not 1 <= length <= maximum:
            raise PrivacyWalletWorkerErrorV1(f"worker {label} has an invalid length")
        return self.take(length)

    def finish(self) -> None:
        if self._offset != len(self._source):
            raise PrivacyWalletWorkerErrorV1("worker response contains trailing bytes")


class PrivacyWalletWorkerControllerV1:
    """Thin authenticated controller for one isolated Rust worker process."""

    __slots__ = ("_auth_key", "_closed", "_lock", "_next_sequence", "_process")

    def __init__(
        self,
        worker_path: str | os.PathLike[str],
        *,
        expected_worker_sha256: str,
    ) -> None:
        executable, initial_identity = _require_worker_executable(
            worker_path, expected_worker_sha256
        )
        auth_key = bytearray(secrets.token_bytes(_AUTH_KEY_BYTES_V1))
        if len(auth_key) != _AUTH_KEY_BYTES_V1 or not any(auth_key):
            _zeroize(auth_key)
            raise PrivacyWalletWorkerErrorV1("secure worker authentication key is unavailable")
        try:
            process = subprocess.Popen(
                [os.fspath(executable)],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
                bufsize=0,
                close_fds=True,
                cwd=os.path.abspath(os.sep),
                env={},
                start_new_session=True,
            )
        except OSError as error:
            _zeroize(auth_key)
            raise PrivacyWalletWorkerErrorV1("failed to start native privacy wallet worker") from error
        if process.stdin is None or process.stdout is None:
            _zeroize(auth_key)
            process.kill()
            raise PrivacyWalletWorkerErrorV1("native privacy wallet worker has no private pipes")
        self._process = process
        self._auth_key = auth_key
        self._closed = False
        self._lock = threading.RLock()
        self._next_sequence = 1
        try:
            _, launched_identity = _require_worker_executable(
                executable, expected_worker_sha256
            )
            if launched_identity != initial_identity:
                raise PrivacyWalletWorkerErrorV1(
                    "native privacy wallet worker changed while it was started"
                )
            process.stdin.write(auth_key)
            process.stdin.flush()
        except (BrokenPipeError, OSError, ValueError, PrivacyWalletWorkerErrorV1) as error:
            self._abort()
            raise PrivacyWalletWorkerErrorV1(
                "native privacy wallet worker rejected session authentication"
            ) from error

    def __enter__(self) -> PrivacyWalletWorkerControllerV1:
        return self

    def __exit__(self, _type: object, _value: object, _traceback: object) -> None:
        self.close()

    def __del__(self) -> None:
        self._abort()

    @property
    def closed(self) -> bool:
        return self._closed

    def ping(self) -> None:
        payload = self._exchange(PrivacyWalletWorkerCommandV1.PING, b"")
        try:
            cursor = _Cursor(payload)
            if cursor.u8() != 0:
                raise PrivacyWalletWorkerErrorV1("ping response has the wrong result tag")
            cursor.finish()
        except PrivacyWalletWorkerErrorV1 as error:
            raise self._malformed(str(error)) from error

    def import_credential(
        self,
        credential_path: str | os.PathLike[str],
        binding: PrivacyWalletWitnessBindingV1,
        *,
        ttl_millis: int,
    ) -> PrivacyWalletWitnessLeaseV1:
        """Ask Rust to read and custody a bundle; Python never opens the path."""

        _require_generic_binding(binding)
        path = _require_credential_path(credential_path)
        if type(ttl_millis) is not int or not (
            PRIVACY_WALLET_WORKER_MIN_TTL_MILLIS_V1
            <= ttl_millis
            <= PRIVACY_WALLET_WORKER_MAX_TTL_MILLIS_V1
        ):
            raise ValueError("ttl_millis is outside the closed worker range")
        payload = _put_text(os.fspath(path)) + _encode_binding(binding) + struct.pack(">Q", ttl_millis)
        response = self._exchange(PrivacyWalletWorkerCommandV1.IMPORT, payload)
        return self._decode_lease(response, binding)

    def inspect(
        self,
        handle: PrivacyWalletWitnessHandleV1,
        binding: PrivacyWalletWitnessBindingV1,
    ) -> PrivacyWalletWitnessLeaseV1:
        _require_generic_binding(binding)
        response = self._exchange(
            PrivacyWalletWorkerCommandV1.INSPECT,
            _encode_handle_binding(handle, binding),
        )
        lease = self._decode_lease(response, binding)
        if lease.handle != handle:
            raise self._malformed("inspect response substituted the witness handle")
        return lease

    def cancel(
        self,
        handle: PrivacyWalletWitnessHandleV1,
        binding: PrivacyWalletWitnessBindingV1,
    ) -> None:
        _require_generic_binding(binding)
        response = self._exchange(
            PrivacyWalletWorkerCommandV1.CANCEL,
            _encode_handle_binding(handle, binding),
        )
        try:
            cursor = _Cursor(response)
            if cursor.u8() != 2:
                raise PrivacyWalletWorkerErrorV1("cancel response has the wrong result tag")
            cursor.finish()
        except PrivacyWalletWorkerErrorV1 as error:
            raise self._malformed(str(error)) from error

    def execute(
        self,
        handle: PrivacyWalletWitnessHandleV1,
        binding: PrivacyWalletWitnessBindingV1,
        *,
        canonical_public_intent: bytes,
        canonical_execution_plan: bytes,
    ) -> PrivacyWalletSignedActionV1:
        """Consume a handle inside Rust and return public self-inspected signed wire."""

        _require_generic_binding(binding)
        public_intent = _require_public_bytes(
            canonical_public_intent,
            PRIVACY_WALLET_WORKER_MAX_PUBLIC_INTENT_BYTES_V1,
            "canonical_public_intent",
        )
        execution_plan = _require_public_bytes(
            canonical_execution_plan,
            PRIVACY_WALLET_WORKER_MAX_EXECUTION_PLAN_BYTES_V1,
            "canonical_execution_plan",
        )
        payload = b"".join(
            (
                _encode_handle_binding(handle, binding),
                struct.pack(">I", len(public_intent)),
                public_intent,
                struct.pack(">I", len(execution_plan)),
                execution_plan,
            )
        )
        response = self._exchange(PrivacyWalletWorkerCommandV1.EXECUTE, payload)
        return self._decode_signed_action(response, binding)

    def import_private_settlement_credential(
        self,
        credential_path: str | os.PathLike[str],
        binding: PrivacyWalletWitnessBindingV1,
        *,
        ttl_millis: int,
    ) -> PrivateSettlementWalletLeaseV1:
        """Ask Rust to custody one owner-only settlement bundle by path."""

        _require_private_settlement_binding(binding)
        path = _require_credential_path(credential_path)
        if type(ttl_millis) is not int or not (
            PRIVACY_WALLET_WORKER_MIN_TTL_MILLIS_V1
            <= ttl_millis
            <= PRIVACY_WALLET_WORKER_MAX_TTL_MILLIS_V1
        ):
            raise ValueError("ttl_millis is outside the closed worker range")
        payload = (
            _put_text(os.fspath(path))
            + _encode_binding(binding)
            + struct.pack(">Q", ttl_millis)
        )
        response = self._exchange(
            PrivacyWalletWorkerCommandV1.IMPORT_PRIVATE_SETTLEMENT, payload
        )
        return self._decode_private_settlement_lease(response, binding)

    def inspect_private_settlement(
        self,
        handle: PrivacyWalletWitnessHandleV1,
        binding: PrivacyWalletWitnessBindingV1,
    ) -> PrivateSettlementWalletLeaseV1:
        """Inspect only public commitments retained beside a settlement handle."""

        _require_private_settlement_binding(binding)
        response = self._exchange(
            PrivacyWalletWorkerCommandV1.INSPECT_PRIVATE_SETTLEMENT,
            _encode_handle_binding(handle, binding),
        )
        lease = self._decode_private_settlement_lease(response, binding)
        if lease.handle != handle:
            raise self._malformed(
                "private-settlement inspect response substituted the witness handle"
            )
        return lease

    def cancel_private_settlement(
        self,
        handle: PrivacyWalletWitnessHandleV1,
        binding: PrivacyWalletWitnessBindingV1,
    ) -> None:
        """Cancel and wipe an unused settlement witness handle."""

        _require_private_settlement_binding(binding)
        response = self._exchange(
            PrivacyWalletWorkerCommandV1.CANCEL_PRIVATE_SETTLEMENT,
            _encode_handle_binding(handle, binding),
        )
        try:
            cursor = _Cursor(response)
            if cursor.u8() != 2:
                raise PrivacyWalletWorkerErrorV1(
                    "private-settlement cancel response has the wrong result tag"
                )
            cursor.finish()
        except PrivacyWalletWorkerErrorV1 as error:
            raise self._malformed(str(error)) from error

    def prove_private_settlement(
        self,
        handle: PrivacyWalletWitnessHandleV1,
        binding: PrivacyWalletWitnessBindingV1,
        *,
        manifest_norito: bytes,
        statement_norito: bytes,
        audit_capsule_norito: bytes,
        audit_policy_norito: bytes,
        canonical_genesis_hash: bytes,
        current_height: int,
        successor_root: bytes,
    ) -> PrivateSettlementPreparedProofV1:
        """Consume one handle in Rust and return the proof-bound public leg."""

        _require_private_settlement_binding(binding)
        manifest = _require_opaque_public_bytes(
            manifest_norito,
            PRIVACY_WALLET_WORKER_MAX_SETTLEMENT_PUBLIC_OBJECT_BYTES_V1,
            "manifest_norito",
        )
        statement = _require_opaque_public_bytes(
            statement_norito,
            PRIVACY_WALLET_WORKER_MAX_SETTLEMENT_PUBLIC_OBJECT_BYTES_V1,
            "statement_norito",
        )
        capsule = _require_opaque_public_bytes(
            audit_capsule_norito,
            PRIVACY_WALLET_WORKER_MAX_SETTLEMENT_PUBLIC_OBJECT_BYTES_V1,
            "audit_capsule_norito",
        )
        policy = _require_opaque_public_bytes(
            audit_policy_norito,
            PRIVACY_WALLET_WORKER_MAX_SETTLEMENT_PUBLIC_OBJECT_BYTES_V1,
            "audit_policy_norito",
        )
        genesis_hash = _require_nonzero_bytes(
            canonical_genesis_hash,
            _DIGEST_BYTES_V1,
            "canonical_genesis_hash",
        )
        if not hmac.compare_digest(genesis_hash, binding.network_id):
            raise ValueError("canonical_genesis_hash does not match the witness binding")
        if type(current_height) is not int or not 1 <= current_height <= _U64_MAX:
            raise ValueError("current_height is outside the canonical u64 range")
        new_root = _require_nonzero_bytes(
            successor_root,
            _DIGEST_BYTES_V1,
            "successor_root",
        )
        payload = b"".join(
            (
                _encode_handle_binding(handle, binding),
                _put_bytes_u32(manifest),
                _put_bytes_u32(statement),
                _put_bytes_u32(capsule),
                _put_bytes_u32(policy),
                genesis_hash,
                struct.pack(">Q", current_height),
                new_root,
            )
        )
        response = self._exchange(
            PrivacyWalletWorkerCommandV1.PROVE_PRIVATE_SETTLEMENT, payload
        )
        return self._decode_private_settlement_proof(
            response,
            binding,
            expected_statement=statement,
            expected_capsule=capsule,
        )

    def close(self) -> None:
        with self._lock:
            if self._closed:
                return
            self._closed = True
            _zeroize(self._auth_key)
            try:
                if self._process.stdin is not None:
                    self._process.stdin.close()
            except OSError:
                pass
            try:
                self._process.wait(timeout=1)
            except (subprocess.TimeoutExpired, OSError):
                self._process.terminate()
                try:
                    self._process.wait(timeout=1)
                except (subprocess.TimeoutExpired, OSError):
                    self._process.kill()
                    try:
                        self._process.wait(timeout=1)
                    except (subprocess.TimeoutExpired, OSError):
                        pass

    def _abort(self) -> None:
        if getattr(self, "_closed", True):
            return
        self._closed = True
        _zeroize(self._auth_key)
        try:
            self._process.kill()
        except OSError:
            pass

    def _malformed(self, message: str) -> PrivacyWalletWorkerErrorV1:
        self._abort()
        return PrivacyWalletWorkerErrorV1(message)

    def _exchange(self, command: PrivacyWalletWorkerCommandV1, payload: bytes) -> bytes:
        with self._lock:
            if self._closed:
                raise PrivacyWalletWorkerErrorV1("privacy wallet worker session is closed")
            sequence = self._next_sequence
            if sequence > _U64_MAX:
                raise self._malformed("privacy wallet worker sequence space is exhausted")
            encoded = _encode_frame(command, sequence, payload, self._auth_key)
            try:
                assert self._process.stdin is not None
                self._process.stdin.write(encoded)
                self._process.stdin.flush()
                assert self._process.stdout is not None
                response = _read_frame(self._process.stdout, self._auth_key)
            except PrivacyWalletWorkerErrorV1:
                self._abort()
                raise
            except (BrokenPipeError, EOFError, OSError, ValueError) as error:
                self._abort()
                raise PrivacyWalletWorkerErrorV1(
                    "native privacy wallet worker transport failed"
                ) from error
            response_command, response_sequence, response_payload = response
            if response_command != command or response_sequence != sequence:
                raise self._malformed("worker response command or sequence does not match request")
            self._next_sequence = sequence + 1
            try:
                return _raise_remote_error(response_payload)
            except PrivacyWalletWorkerRemoteErrorV1:
                raise
            except PrivacyWalletWorkerErrorV1:
                self._abort()
                raise

    def _decode_lease(
        self,
        payload: bytes,
        binding: PrivacyWalletWitnessBindingV1,
    ) -> PrivacyWalletWitnessLeaseV1:
        try:
            cursor = _Cursor(payload)
            if cursor.u8() != 1:
                raise PrivacyWalletWorkerErrorV1("lease response has the wrong result tag")
            handle = PrivacyWalletWitnessHandleV1(cursor.take(_HANDLE_BYTES_V1))
            expires_at_millis = cursor.u64()
            if cursor.u8() != 1:
                raise PrivacyWalletWorkerErrorV1("lease has the wrong bundle schema version")
            wallet_id = cursor.text(_MAX_SIGNER_BYTES_V1, "wallet_id")
            authority = cursor.text(_MAX_SIGNER_BYTES_V1, "authority")
            authority_public_key = cursor.text(_MAX_PUBLIC_KEY_BYTES_V1, "authority public key")
            protocol_id = cursor.text(_MAX_PROTOCOL_BYTES_V1, "protocol_id")
            operation_schema = cursor.text(_MAX_OPERATION_SCHEMA_BYTES_V1, "operation_schema")
            cursor.finish()
            expected_schema = PRIVACY_GENERIC11_WORKER_OPERATION_SCHEMAS_V1.get(protocol_id)
            if (
                handle.value[0] & 0x80 != 0
                or expires_at_millis == 0
                or wallet_id != binding.signer_wallet_id
                or protocol_id != binding.protocol_id
                or expected_schema != operation_schema
            ):
                raise PrivacyWalletWorkerErrorV1("lease manifest does not match the request binding")
            return PrivacyWalletWitnessLeaseV1(
                handle=handle,
                expires_at_millis=expires_at_millis,
                wallet_id=wallet_id,
                authority=authority,
                authority_public_key=authority_public_key,
                protocol_id=protocol_id,
                operation_schema=operation_schema,
            )
        except (PrivacyWalletWorkerErrorV1, TypeError, ValueError) as error:
            if isinstance(error, PrivacyWalletWorkerRemoteErrorV1):
                raise
            raise self._malformed(str(error)) from error

    def _decode_signed_action(
        self,
        payload: bytes,
        binding: PrivacyWalletWitnessBindingV1,
    ) -> PrivacyWalletSignedActionV1:
        try:
            cursor = _Cursor(payload)
            if cursor.u8() != 3:
                raise PrivacyWalletWorkerErrorV1("signed action has the wrong result tag")
            protocol_id = cursor.text(_MAX_PROTOCOL_BYTES_V1, "protocol_id")
            operation_schema = cursor.text(_MAX_OPERATION_SCHEMA_BYTES_V1, "operation_schema")
            network_id = cursor.take(_DIGEST_BYTES_V1)
            authority = cursor.text(_MAX_SIGNER_BYTES_V1, "authority")
            authority_public_key = cursor.text(_MAX_PUBLIC_KEY_BYTES_V1, "authority public key")
            adaptive = cursor.bytes_u32(
                PRIVACY_WALLET_WORKER_MAX_FRAME_BYTES_V1, "adaptive signed transaction"
            )
            versioned = cursor.bytes_u32(
                PRIVACY_WALLET_WORKER_MAX_FRAME_BYTES_V1, "versioned signed transaction"
            )
            signature = cursor.bytes_u16(_MAX_SIGNATURE_BYTES_V1, "signature")
            public_key = cursor.bytes_u16(_MAX_PUBLIC_KEY_BYTES_V1, "public key")
            digests = tuple(cursor.take(_DIGEST_BYTES_V1) for _ in range(4))
            counts = tuple(cursor.u32() for _ in range(5))
            cursor.finish()
            expected_schema = PRIVACY_GENERIC11_WORKER_OPERATION_SCHEMAS_V1.get(protocol_id)
            if (
                protocol_id != binding.protocol_id
                or operation_schema != expected_schema
                or network_id != binding.network_id
            ):
                raise PrivacyWalletWorkerErrorV1(
                    "signed action identity does not match the request binding"
                )
            if any(not any(digest) for digest in digests):
                raise PrivacyWalletWorkerErrorV1("signed action contains a zero public digest")
            if len(signature) != 64 or len(public_key) != 32:
                raise PrivacyWalletWorkerErrorV1(
                    "signed action is not an exact Ed25519 signature/public-key pair"
                )
            if counts[3] != len(adaptive) or counts[4] != len(versioned):
                raise PrivacyWalletWorkerErrorV1("signed action byte counts are inconsistent")
            if counts[0] == 0 or counts[1] == 0 or counts[2] < counts[1]:
                raise PrivacyWalletWorkerErrorV1("signed action proof counts are inconsistent")
            return PrivacyWalletSignedActionV1(
                protocol_id=protocol_id,
                operation_schema=operation_schema,
                network_id=network_id,
                authority=authority,
                authority_public_key=authority_public_key,
                adaptive_signed_transaction=adaptive,
                versioned_signed_transaction=versioned,
                signature=signature,
                public_key=public_key,
                transaction_hash=digests[0],
                transaction_intent_digest=digests[1],
                statement_digest=digests[2],
                proof_envelope_hash=digests[3],
                statement_bytes=counts[0],
                proof_bytes=counts[1],
                encoded_proof_envelope_bytes=counts[2],
                adaptive_signed_transaction_bytes=counts[3],
                submitted_versioned_transaction_bytes=counts[4],
            )
        except (PrivacyWalletWorkerErrorV1, TypeError, ValueError) as error:
            raise self._malformed(str(error)) from error

    def _decode_private_settlement_lease(
        self,
        payload: bytes,
        binding: PrivacyWalletWitnessBindingV1,
    ) -> PrivateSettlementWalletLeaseV1:
        try:
            cursor = _Cursor(payload)
            if cursor.u8() != 4:
                raise PrivacyWalletWorkerErrorV1(
                    "private-settlement lease has the wrong result tag"
                )
            handle = PrivacyWalletWitnessHandleV1(cursor.take(_HANDLE_BYTES_V1))
            expires_at_millis = cursor.u64()
            wallet_id = cursor.text(_MAX_SIGNER_BYTES_V1, "wallet_id")
            digests = tuple(cursor.take(_DIGEST_BYTES_V1) for _ in range(4))
            cursor.finish()
            if (
                handle.value[0] & 0x80 == 0
                or expires_at_millis == 0
                or wallet_id != binding.signer_wallet_id
                or not hmac.compare_digest(digests[0], binding.public_intent_digest)
                or any(not any(digest) for digest in digests)
            ):
                raise PrivacyWalletWorkerErrorV1(
                    "private-settlement lease does not match the request binding"
                )
            return PrivateSettlementWalletLeaseV1(
                handle=handle,
                expires_at_millis=expires_at_millis,
                wallet_id=wallet_id,
                proof_binding_digest=digests[0],
                statement_digest=digests[1],
                capsule_digest=digests[2],
                audit_plaintext_commitment=digests[3],
            )
        except (PrivacyWalletWorkerErrorV1, TypeError, ValueError) as error:
            raise self._malformed(str(error)) from error

    def _decode_private_settlement_proof(
        self,
        payload: bytes,
        binding: PrivacyWalletWitnessBindingV1,
        *,
        expected_statement: bytes,
        expected_capsule: bytes,
    ) -> PrivateSettlementPreparedProofV1:
        substituted = "private-settlement proof response substituted public artifacts"
        try:
            cursor = _Cursor(payload)
            if cursor.u8() != 5:
                raise PrivacyWalletWorkerErrorV1(
                    "private-settlement proof has the wrong result tag"
                )
            wallet_id = cursor.text(_MAX_SIGNER_BYTES_V1, "wallet_id")
            genesis_hash = cursor.take(_DIGEST_BYTES_V1)
            digests = tuple(cursor.take(_DIGEST_BYTES_V1) for _ in range(4))
            statement = cursor.bytes_u32(
                PRIVACY_WALLET_WORKER_MAX_SETTLEMENT_PUBLIC_OBJECT_BYTES_V1,
                "private-settlement statement",
            )
            proof = cursor.bytes_u32(
                PRIVACY_WALLET_WORKER_MAX_SETTLEMENT_PROOF_BYTES_V1,
                "private-settlement proof",
            )
            delta = cursor.bytes_u32(
                PRIVACY_WALLET_WORKER_MAX_SETTLEMENT_PUBLIC_OBJECT_BYTES_V1,
                "private-settlement delta",
            )
            capsule = cursor.bytes_u32(
                PRIVACY_WALLET_WORKER_MAX_SETTLEMENT_PUBLIC_OBJECT_BYTES_V1,
                "private-settlement audit capsule",
            )
            cursor.finish()
            if (
                wallet_id != binding.signer_wallet_id
                or not hmac.compare_digest(genesis_hash, binding.network_id)
                or not hmac.compare_digest(digests[0], binding.public_intent_digest)
                or any(not any(digest) for digest in digests)
                or not proof
                or not delta
                or not hmac.compare_digest(statement, expected_statement)
                or not hmac.compare_digest(capsule, expected_capsule)
            ):
                raise PrivacyWalletWorkerErrorV1(substituted)
            return PrivateSettlementPreparedProofV1(
                wallet_id=wallet_id,
                canonical_genesis_hash=genesis_hash,
                proof_binding_digest=digests[0],
                statement_digest=digests[1],
                capsule_digest=digests[2],
                audit_plaintext_commitment=digests[3],
                statement_norito=statement,
                proof=proof,
                delta_norito=delta,
                audit_capsule_norito=capsule,
            )
        except (PrivacyWalletWorkerErrorV1, TypeError, ValueError):
            raise self._malformed(substituted) from None


def privacy_wallet_public_intent_digest_v1(canonical_public_intent: bytes) -> bytes:
    """Return the exact public-intent digest used by the Rust witness binding."""

    value = _require_public_bytes(
        canonical_public_intent,
        PRIVACY_WALLET_WORKER_MAX_PUBLIC_INTENT_BYTES_V1,
        "canonical_public_intent",
    )
    return hashlib.sha256(b"iroha-privacy-wallet-binding-v1\0" + value).digest()


def _require_worker_executable(
    value: str | os.PathLike[str], expected_sha256: str
) -> tuple[Path, tuple[int, int, int, int, str]]:
    if os.name != "posix":
        raise ValueError("privacy wallet worker requires a supported POSIX custody host")
    if (
        type(expected_sha256) is not str
        or len(expected_sha256) != 64
        or expected_sha256 == "0" * 64
        or any(character not in "0123456789abcdef" for character in expected_sha256)
    ):
        raise ValueError("expected_worker_sha256 must be one canonical non-zero SHA-256")
    path = Path(os.fspath(value))
    if not path.is_absolute():
        raise ValueError("privacy wallet worker path must be absolute")
    try:
        metadata = path.lstat()
    except OSError as error:
        raise ValueError("privacy wallet worker path is unavailable") from error
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        raise ValueError("privacy wallet worker path must name a regular non-symlink file")
    if metadata.st_mode & 0o022:
        raise ValueError("privacy wallet worker must not be group/world writable")
    if not os.access(path, os.X_OK):
        raise ValueError("privacy wallet worker is not executable")
    flags = os.O_RDONLY
    if hasattr(os, "O_CLOEXEC"):
        flags |= os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise ValueError("privacy wallet worker could not be opened without following links") from error
    try:
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISREG(opened.st_mode)
            or opened.st_dev != metadata.st_dev
            or opened.st_ino != metadata.st_ino
            or opened.st_mode != metadata.st_mode
            or opened.st_size != metadata.st_size
            or opened.st_size <= 0
            or opened.st_size > _MAX_WORKER_BINARY_BYTES_V1
        ):
            raise ValueError("privacy wallet worker changed before authenticated launch")
        digest = hashlib.sha256()
        observed = 0
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            observed += len(chunk)
            if observed > _MAX_WORKER_BINARY_BYTES_V1:
                raise ValueError("privacy wallet worker exceeds the binary size bound")
            digest.update(chunk)
        actual_sha256 = digest.hexdigest()
    finally:
        os.close(descriptor)
    if observed != metadata.st_size or not hmac.compare_digest(actual_sha256, expected_sha256):
        raise ValueError("privacy wallet worker does not match its admitted SHA-256")
    identity = (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_size,
        metadata.st_mtime_ns,
        actual_sha256,
    )
    return path, identity


def _require_credential_path(value: str | os.PathLike[str]) -> Path:
    path = Path(os.fspath(value))
    if not path.is_absolute():
        raise ValueError("credential_path must be absolute")
    encoded = os.fspath(path).encode("utf-8")
    if not 1 <= len(encoded) <= _MAX_PATH_BYTES_V1:
        raise ValueError("credential_path exceeds the worker path bound")
    _require_text(os.fspath(path), _MAX_PATH_BYTES_V1, "credential_path")
    return path


def _require_text(value: object, maximum: int, label: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{label} must be a string")
    try:
        encoded = value.encode("utf-8")
    except UnicodeEncodeError as error:
        raise ValueError(f"{label} is not canonical UTF-8") from error
    if (
        not encoded
        or len(encoded) > maximum
        or value.strip() != value
        or any(ord(character) < 32 or 127 <= ord(character) <= 159 for character in value)
    ):
        raise ValueError(f"{label} is not canonical bounded text")
    return value


def _require_nonzero_bytes(value: object, size: int, label: str) -> bytes:
    if type(value) is not bytes:
        raise TypeError(f"{label} must be immutable bytes")
    if len(value) != size or not any(value):
        raise ValueError(f"{label} must contain exactly {size} non-zero-bound bytes")
    return value


def _require_public_bytes(value: object, maximum: int, label: str) -> bytes:
    if type(value) is not bytes:
        raise TypeError(f"{label} must be immutable bytes")
    if not 1 <= len(value) <= maximum or b"\0" in value:
        raise ValueError(f"{label} violates the bounded public-byte contract")
    return value


def _require_opaque_public_bytes(value: object, maximum: int, label: str) -> bytes:
    if type(value) is not bytes:
        raise TypeError(f"{label} must be immutable bytes")
    if not 1 <= len(value) <= maximum:
        raise ValueError(f"{label} violates the bounded public-byte contract")
    return value


def _require_generic_binding(
    binding: PrivacyWalletWitnessBindingV1,
) -> PrivacyWalletWitnessBindingV1:
    if not isinstance(binding, PrivacyWalletWitnessBindingV1):
        raise TypeError("binding must be a PrivacyWalletWitnessBindingV1")
    if binding.protocol_id not in PRIVACY_GENERIC11_WORKER_OPERATION_SCHEMAS_V1:
        raise ValueError("binding does not select a generic privacy-worker protocol")
    return binding


def _require_private_settlement_binding(
    binding: PrivacyWalletWitnessBindingV1,
) -> PrivacyWalletWitnessBindingV1:
    if not isinstance(binding, PrivacyWalletWitnessBindingV1):
        raise TypeError("binding must be a PrivacyWalletWitnessBindingV1")
    if binding.protocol_id != ATOMIC_PRIVATE_SETTLEMENT_PROTOCOL_LABEL_V1:
        raise ValueError("binding does not select atomic private settlement")
    return binding


def _put_text(value: str) -> bytes:
    encoded = value.encode("utf-8")
    if len(encoded) > 0xFFFF:
        raise ValueError("worker text exceeds the u16 wire bound")
    return struct.pack(">H", len(encoded)) + encoded


def _put_bytes_u32(value: bytes) -> bytes:
    if len(value) > 0xFFFFFFFF:
        raise ValueError("worker byte field exceeds the u32 wire bound")
    return struct.pack(">I", len(value)) + value


def _encode_binding(binding: PrivacyWalletWitnessBindingV1) -> bytes:
    return b"".join(
        (
            binding.network_id,
            _put_text(binding.signer_wallet_id),
            _put_text(binding.protocol_id),
            binding.compiled_profile_digest,
            binding.public_intent_digest,
            binding.nonce,
            binding.signed_release_authority_digest,
        )
    )


def _encode_handle_binding(
    handle: PrivacyWalletWitnessHandleV1,
    binding: PrivacyWalletWitnessBindingV1,
) -> bytes:
    if not isinstance(handle, PrivacyWalletWitnessHandleV1):
        raise TypeError("handle must be a PrivacyWalletWitnessHandleV1")
    if not isinstance(binding, PrivacyWalletWitnessBindingV1):
        raise TypeError("binding must be a PrivacyWalletWitnessBindingV1")
    return handle.value + _encode_binding(binding)


def _encode_frame(
    command: PrivacyWalletWorkerCommandV1,
    sequence: int,
    payload: bytes,
    auth_key: bytes | bytearray,
) -> bytes:
    if type(payload) is not bytes or not 1 <= sequence <= _U64_MAX:
        raise PrivacyWalletWorkerErrorV1("invalid worker frame input")
    body = b"".join(
        (
            _MAGIC_V1,
            bytes((PRIVACY_WALLET_WORKER_PROTOCOL_VERSION_V1, int(command))),
            struct.pack(">Q", sequence),
            struct.pack(">I", len(payload)),
            payload,
        )
    )
    tag = hmac.digest(auth_key, body, "sha256")
    framed = body + tag
    if len(framed) > PRIVACY_WALLET_WORKER_MAX_FRAME_BYTES_V1:
        raise PrivacyWalletWorkerErrorV1("worker frame exceeds the closed size bound")
    return struct.pack(">I", len(framed)) + framed


def _read_exact(reader: IO[bytes], count: int) -> bytes:
    chunks = bytearray()
    while len(chunks) < count:
        piece = reader.read(count - len(chunks))
        if not piece:
            _zeroize(chunks)
            raise PrivacyWalletWorkerErrorV1("worker response ended before the frame was complete")
        chunks.extend(piece)
    return bytes(chunks)


def _read_frame(
    reader: IO[bytes], auth_key: bytes | bytearray
) -> tuple[PrivacyWalletWorkerCommandV1, int, bytes]:
    length = struct.unpack(">I", _read_exact(reader, 4))[0]
    if not 18 + _AUTH_TAG_BYTES_V1 <= length <= PRIVACY_WALLET_WORKER_MAX_FRAME_BYTES_V1:
        raise PrivacyWalletWorkerErrorV1("worker response has an invalid frame length")
    framed = _read_exact(reader, length)
    body, actual_tag = framed[:-_AUTH_TAG_BYTES_V1], framed[-_AUTH_TAG_BYTES_V1:]
    expected_tag = hmac.digest(auth_key, body, "sha256")
    if not hmac.compare_digest(actual_tag, expected_tag):
        raise PrivacyWalletWorkerErrorV1("worker response authentication failed")
    if body[:4] != _MAGIC_V1 or body[4] != PRIVACY_WALLET_WORKER_PROTOCOL_VERSION_V1:
        raise PrivacyWalletWorkerErrorV1("worker response has the wrong protocol identity")
    try:
        command = PrivacyWalletWorkerCommandV1(body[5])
    except ValueError as error:
        raise PrivacyWalletWorkerErrorV1("worker response has an unknown command") from error
    sequence = struct.unpack(">Q", body[6:14])[0]
    payload_length = struct.unpack(">I", body[14:18])[0]
    if sequence == 0 or payload_length != len(body) - 18:
        raise PrivacyWalletWorkerErrorV1("worker response is not canonically framed")
    return command, sequence, body[18:]


def _raise_remote_error(payload: bytes) -> bytes:
    if not payload or payload[0] != 255:
        return payload
    cursor = _Cursor(payload)
    cursor.u8()
    raw_code = cursor.u16()
    message = cursor.text(_MAX_RESPONSE_TEXT_BYTES_V1, "error message")
    cursor.finish()
    try:
        code = PrivacyWalletWorkerErrorCodeV1(raw_code)
    except ValueError as error:
        raise PrivacyWalletWorkerErrorV1("worker returned an unknown error code") from error
    raise PrivacyWalletWorkerRemoteErrorV1(code, message)


def _zeroize(value: bytearray) -> None:
    value[:] = b"\0" * len(value)


__all__ = [
    "ATOMIC_PRIVATE_SETTLEMENT_PROTOCOL_LABEL_V1",
    "PRIVACY_GENERIC11_WORKER_OPERATION_SCHEMAS_V1",
    "PRIVACY_WALLET_WORKER_MAX_EXECUTION_PLAN_BYTES_V1",
    "PRIVACY_WALLET_WORKER_MAX_FRAME_BYTES_V1",
    "PRIVACY_WALLET_WORKER_MAX_PUBLIC_INTENT_BYTES_V1",
    "PRIVACY_WALLET_WORKER_MAX_SETTLEMENT_PROOF_BYTES_V1",
    "PRIVACY_WALLET_WORKER_MAX_SETTLEMENT_PUBLIC_OBJECT_BYTES_V1",
    "PRIVACY_WALLET_WORKER_MAX_TTL_MILLIS_V1",
    "PRIVACY_WALLET_WORKER_MIN_TTL_MILLIS_V1",
    "PRIVACY_WALLET_WORKER_PROTOCOL_VERSION_V1",
    "PrivateSettlementPreparedProofV1",
    "PrivateSettlementWalletLeaseV1",
    "PrivacyWalletSignedActionV1",
    "PrivacyWalletWitnessBindingV1",
    "PrivacyWalletWitnessHandleV1",
    "PrivacyWalletWitnessLeaseV1",
    "PrivacyWalletWorkerCommandV1",
    "PrivacyWalletWorkerControllerV1",
    "PrivacyWalletWorkerErrorCodeV1",
    "PrivacyWalletWorkerErrorV1",
    "PrivacyWalletWorkerRemoteErrorV1",
    "privacy_wallet_public_intent_digest_v1",
]
