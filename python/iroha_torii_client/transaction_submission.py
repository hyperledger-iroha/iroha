"""Fail-closed, one-shot transport for current signed transactions."""

from __future__ import annotations

import ipaddress
import math
import queue
import re
import threading
import time
from typing import Any, Callable, Mapping, Optional, TypeVar
from urllib.parse import urlsplit

import requests

from .orderbook_submission import (
    prepare_one_shot_request,
    snapshot_one_shot_transport,
    validate_fixed_request_headers,
)

TRANSACTION_SUBMISSION_ROUTE_V1 = "/v1/pipeline/transactions"
TRANSACTION_SUBMISSION_TIMEOUT_SECONDS_V1 = 30.0
TRANSACTION_SUBMISSION_MAX_RESPONSE_BYTES_V1 = 1024 * 1024
TRANSACTION_SUBMISSION_BODY_PREVIEW_BYTES_V1 = 512

__all__ = [
    "AmbiguousTransactionSubmissionError",
    "TRANSACTION_SUBMISSION_BODY_PREVIEW_BYTES_V1",
    "TRANSACTION_SUBMISSION_MAX_RESPONSE_BYTES_V1",
    "TRANSACTION_SUBMISSION_ROUTE_V1",
    "TRANSACTION_SUBMISSION_TIMEOUT_SECONDS_V1",
    "TransactionSubmissionNotDispatchedError",
    "TransactionSubmissionHttpError",
    "submit_transaction_once_v1",
    "transaction_submission_status_is_ambiguous",
]

_HASH_HEX = re.compile(r"[0-9a-f]{64}")
_REJECT_CODE = re.compile(r"[A-Za-z0-9_.:-]{1,128}")
_DEFINITIVE_REJECTION_CODES_V1: Mapping[int, frozenset[str]] = {
    400: frozenset(
        {
            "invalid_transaction_payload",
            "transaction_rejected",
            "PRTRY:NTS_UNHEALTHY",
            "PRTRY:TX_UNSUPPORTED_AUTHORITY",
            "PRTRY:TX_SIGNATURE_ALGO_DENIED",
            "PRTRY:TX_SIGNATURE_INVALID",
            "PRTRY:TX_SIGNATURE_MALFORMED",
            "PRTRY:TX_SIGNATURE_MISSING",
            "PRTRY:TX_SIGNATURE_UNKNOWN_SIGNER",
            "PRTRY:TX_SIGNATURE_INSUFFICIENT",
            "ED07",
            "PRTRY:ROUTE_UNRESOLVED",
        }
    ),
    403: frozenset(
        {
            "PRTRY:QUEUE_GOVERNANCE_REJECTED",
            "PRTRY:QUEUE_LANE_COMPLIANCE_DENIED",
            "PRTRY:QUEUE_LANE_PRIVACY_PROOF_REJECTED",
            "PRTRY:NEXUS_FEE_ADMISSION_REJECTED",
            "PRTRY:CONFIDENTIAL_POLICY_REJECTED",
        }
    ),
    406: frozenset(),
    413: frozenset(),
    415: frozenset(),
}
_HTTP_ADAPTER_SEND = requests.adapters.HTTPAdapter.send
_HTTP_ADAPTER_CLOSE = requests.adapters.HTTPAdapter.close
_VerifiedReceipt = TypeVar("_VerifiedReceipt")


class _TransactionSubmissionDispatchSlot:
    """Identity-owned process slot that stale workers cannot release."""

    def __init__(self) -> None:
        self._guard = threading.Lock()
        self._owner: object | None = None

    def try_acquire(self, owner: object) -> bool:
        with self._guard:
            if self._owner is not None:
                return False
            self._owner = owner
            return True

    def release(self, owner: object) -> None:
        with self._guard:
            if self._owner is owner:
                self._owner = None

    def is_available(self) -> bool:
        with self._guard:
            return self._owner is None


_TRANSACTION_SUBMISSION_DISPATCH_SLOT = _TransactionSubmissionDispatchSlot()


class TransactionSubmissionHttpError(RuntimeError):
    """A definitive non-202 HTTP response to one transaction submission."""

    def __init__(
        self,
        *,
        hash_hex: str,
        status_code: int,
        reject_code: Optional[str],
        body_preview: Optional[str],
        body_truncated: bool,
    ) -> None:
        self.hash_hex = hash_hex
        self.status_code = status_code
        self.reject_code = reject_code
        self.body_preview = body_preview
        self.body_truncated = body_truncated
        message = (
            f"transaction submission for {hash_hex} requires HTTP 202; received HTTP {status_code}"
        )
        if reject_code is not None:
            message += f" (reject_code={reject_code})"
        if body_preview is not None:
            suffix = "..." if body_truncated else ""
            message += f"; body={body_preview}{suffix}"
        super().__init__(message)


class TransactionSubmissionNotDispatchedError(TimeoutError):
    """The wall-clock deadline expired before any network dispatch began."""

    def __init__(self, *, hash_hex: str) -> None:
        self.hash_hex = hash_hex
        super().__init__(
            f"transaction {hash_hex} was not dispatched before its deadline; "
            "the exact signed bytes may be retried"
        )


class AmbiguousTransactionSubmissionError(RuntimeError):
    """One dispatch began without authoritative admission evidence."""

    def __init__(
        self,
        *,
        hash_hex: str,
        status_code: Optional[int],
        reject_code: Optional[str],
        body_preview: Optional[str],
        body_truncated: bool,
        cause_kind: Optional[str] = None,
    ) -> None:
        self.hash_hex = hash_hex
        self.status_code = status_code
        self.reject_code = reject_code
        self.body_preview = body_preview
        self.body_truncated = body_truncated
        self.cause_kind = cause_kind
        message = (
            f"transaction {hash_hex} had one dispatch attempt, but its admission outcome is unknown"
        )
        if status_code is not None:
            message += f" after HTTP {status_code}"
        if reject_code is not None:
            message += f" (reject_code={reject_code})"
        if body_preview is not None:
            suffix = "..." if body_truncated else ""
            message += f"; body={body_preview}{suffix}"
        if cause_kind is not None:
            message += f"; transport={cause_kind}"
        message += "; do not resend the signed bytes—reconcile by transaction hash"
        super().__init__(message)


def transaction_submission_status_is_ambiguous(status_code: int) -> bool:
    """Return whether an HTTP status cannot prove that admission did not occur."""

    return status_code != 202 and status_code not in _DEFINITIVE_REJECTION_CODES_V1


def _has_definitive_rejection_evidence(
    status_code: int,
    reject_code: Optional[str],
) -> bool:
    allowed_codes = _DEFINITIVE_REJECTION_CODES_V1.get(status_code)
    return allowed_codes is not None and (reject_code is None or reject_code in allowed_codes)


def _validate_submission_inputs(
    signed_transaction: Any,
    timeout: Any,
) -> tuple[bytes, float]:
    if type(signed_transaction) is not bytes:
        raise TypeError("signed_transaction must be exact immutable bytes")
    if not signed_transaction:
        raise ValueError("signed_transaction must not be empty")
    if isinstance(timeout, bool) or not isinstance(timeout, (int, float)):
        raise TypeError("timeout must be a positive finite number")
    timeout_seconds = float(timeout)
    if not math.isfinite(timeout_seconds) or timeout_seconds <= 0:
        raise ValueError("timeout must be a positive finite number")
    return signed_transaction, timeout_seconds


def _validate_base_url(base_url: Any) -> str:
    if type(base_url) is not str or not base_url or base_url.endswith("/"):
        raise ValueError("base_url must be an exact Torii origin without a trailing slash")
    try:
        parsed = urlsplit(base_url)
        hostname = parsed.hostname
        _ = parsed.port
    except ValueError as error:
        raise ValueError("base_url must be an exact Torii origin") from error
    if (
        hostname is None
        or parsed.username is not None
        or parsed.password is not None
        or parsed.path
        or parsed.query
        or parsed.fragment
    ):
        raise ValueError("base_url must be an exact Torii origin")
    if parsed.scheme == "https":
        return base_url
    if parsed.scheme != "http":
        raise ValueError("base_url must use HTTPS or exact loopback HTTP")
    try:
        is_loopback = ipaddress.ip_address(hostname).is_loopback
    except ValueError:
        is_loopback = False
    if not is_loopback:
        raise ValueError("cleartext transaction submission requires a literal loopback address")
    return base_url


def _authenticate_signed_transaction(
    body: bytes,
    expected_receipt_signer: str,
    inspect: Callable[[bytes, str], tuple[bytes, str]],
) -> str:
    result = inspect(body, expected_receipt_signer)
    if type(result) is not tuple or len(result) != 2:
        raise RuntimeError("native transaction-submission inspection returned invalid evidence")
    hash_bytes, canonical_signer = result
    if (
        type(hash_bytes) is not bytes
        or len(hash_bytes) != 32
        or hash_bytes[-1] & 1 != 1
        or type(canonical_signer) is not str
        or canonical_signer != expected_receipt_signer
    ):
        raise RuntimeError("native transaction-submission inspection returned invalid evidence")
    return hash_bytes.hex()


def _response_header(response: Any, name: str, *, strict: bool) -> Optional[str]:
    raw_headers = getattr(getattr(response, "raw", None), "headers", None)
    getlist = getattr(raw_headers, "getlist", None)
    if callable(getlist):
        values = getlist(name)
        if len(values) > 1:
            if strict:
                raise RuntimeError(f"response contains duplicate {name} headers")
            return None
    value = getattr(response, "headers", {}).get(name)
    if value is None:
        return None
    if not isinstance(value, str) or "," in value:
        if strict:
            raise RuntimeError(f"response contains a non-canonical {name} header")
        return None
    return value


def _reject_code(response: Any, *, strict: bool) -> Optional[str]:
    value = _response_header(response, "x-iroha-reject-code", strict=strict)
    if value is None:
        return None
    if _REJECT_CODE.fullmatch(value) is None:
        if strict:
            raise RuntimeError("response contains a non-canonical x-iroha-reject-code header")
        return None
    return value


def _read_bounded_body(
    response: Any,
    maximum_bytes: int,
    deadline: float,
) -> tuple[bytes, bool]:
    body = bytearray()
    for chunk in response.iter_content(chunk_size=8192, decode_unicode=False):
        if time.monotonic() > deadline:
            raise TimeoutError("transaction submission response exceeded its wall-clock deadline")
        if not isinstance(chunk, (bytes, bytearray)):
            raise RuntimeError("transaction submission response yielded a non-byte chunk")
        remaining = maximum_bytes + 1 - len(body)
        if remaining <= 0:
            return bytes(body[:maximum_bytes]), True
        body.extend(chunk[:remaining])
        if len(chunk) > remaining or len(body) > maximum_bytes:
            return bytes(body[:maximum_bytes]), True
    return bytes(body), False


def _body_preview(response: Any, deadline: float) -> tuple[Optional[str], bool]:
    body, truncated = _read_bounded_body(
        response,
        TRANSACTION_SUBMISSION_BODY_PREVIEW_BYTES_V1,
        deadline,
    )
    text = body.decode("utf-8", "replace")
    text = "".join(character if character.isprintable() else "�" for character in text).strip()
    return (text or None), truncated


def _close_response(response: Any) -> None:
    try:
        response.close()
    except BaseException:
        pass


def _close_adapter(adapter: requests.adapters.HTTPAdapter) -> None:
    try:
        _HTTP_ADAPTER_CLOSE(adapter)
    except BaseException:
        pass


def _http_202_receipt_evidence(
    response: Any,
    hash_hex: str,
    deadline: float,
) -> bytes:
    if _response_header(response, "x-iroha-reject-code", strict=True) is not None:
        raise RuntimeError("HTTP 202 response contains contradictory rejection evidence")
    content_type = _response_header(response, "Content-Type", strict=True)
    if content_type != "application/x-norito":
        raise RuntimeError("HTTP 202 response Content-Type must be application/x-norito")
    content_encoding = _response_header(response, "Content-Encoding", strict=True)
    if content_encoding not in (None, "identity"):
        raise RuntimeError("HTTP 202 response Content-Encoding must be absent or identity")
    identities: dict[str, str] = {}
    for name in (
        "x-iroha-entrypoint-hash",
        "x-iroha-signed-transaction-hash",
    ):
        value = _response_header(response, name, strict=True)
        if value is None or _HASH_HEX.fullmatch(value) is None:
            raise RuntimeError(f"HTTP 202 response {name} must be one lowercase hash")
        if value != hash_hex:
            raise RuntimeError(f"HTTP 202 response {name} does not match the transaction")
        identities[name] = value
    body, truncated = _read_bounded_body(
        response,
        TRANSACTION_SUBMISSION_MAX_RESPONSE_BYTES_V1,
        deadline,
    )
    if truncated:
        raise RuntimeError("HTTP 202 transaction receipt exceeds its V1 byte bound")
    if not body:
        raise RuntimeError("HTTP 202 transaction receipt is empty")
    return body


class _DispatchOutcome:
    __slots__ = (
        "body",
        "body_preview",
        "body_truncated",
        "cause_kind",
        "kind",
        "reject_code",
        "status_code",
    )

    def __init__(
        self,
        kind: str,
        *,
        status_code: Optional[int] = None,
        reject_code: Optional[str] = None,
        body: Optional[bytes] = None,
        body_preview: Optional[str] = None,
        body_truncated: bool = False,
        cause_kind: Optional[str] = None,
    ) -> None:
        self.kind = kind
        self.status_code = status_code
        self.reject_code = reject_code
        self.body = body
        self.body_preview = body_preview
        self.body_truncated = body_truncated
        self.cause_kind = cause_kind


def _dispatch_and_collect_submission(
    *,
    adapter: requests.adapters.HTTPAdapter,
    prepared: requests.PreparedRequest,
    transport: Mapping[str, Any],
    hash_hex: str,
    timeout_seconds: float,
    deadline: float,
    dispatch_gate: threading.Event,
    dispatch_owner: object,
    cancelled: threading.Event,
    response_lock: threading.Lock,
    shared_state: dict[str, Any],
    outcomes: queue.Queue[_DispatchOutcome],
) -> None:
    response = None
    status_code: Optional[int] = None
    reject_code: Optional[str] = None
    outcome: Optional[_DispatchOutcome] = None
    try:
        dispatch_gate.wait()
        with response_lock:
            if (
                shared_state["phase"] != "pending"
                or cancelled.is_set()
                or time.monotonic() > deadline
            ):
                return
            shared_state["phase"] = "dispatching"
            shared_state["dispatch_started"] = True
        try:
            response = _HTTP_ADAPTER_SEND(
                adapter,
                prepared,
                stream=True,
                timeout=(timeout_seconds, min(timeout_seconds, 5.0)),
                **transport,
            )
        except BaseException as error:
            outcome = _DispatchOutcome("ambiguous", cause_kind=type(error).__name__)
            return

        with response_lock:
            shared_state["response"] = response
        if cancelled.is_set():
            return

        observed_status = getattr(response, "status_code", None)
        if (
            isinstance(observed_status, bool)
            or not isinstance(observed_status, int)
            or not 100 <= observed_status <= 599
        ):
            outcome = _DispatchOutcome("ambiguous", cause_kind="InvalidHttpStatus")
            return
        status_code = observed_status
        reject_code = _reject_code(response, strict=True)
        if status_code == 202:
            try:
                receipt = _http_202_receipt_evidence(response, hash_hex, deadline)
            except BaseException as error:
                outcome = _DispatchOutcome(
                    "ambiguous",
                    status_code=status_code,
                    reject_code=reject_code,
                    cause_kind=type(error).__name__,
                )
            else:
                outcome = _DispatchOutcome(
                    "receipt",
                    status_code=status_code,
                    body=receipt,
                )
            return

        try:
            preview, truncated = _body_preview(response, deadline)
        except BaseException:
            preview, truncated = None, False
        outcome = _DispatchOutcome(
            "response",
            status_code=status_code,
            reject_code=reject_code,
            body_preview=preview,
            body_truncated=truncated,
        )
    except BaseException as error:
        outcome = _DispatchOutcome(
            "ambiguous",
            status_code=status_code,
            reject_code=reject_code,
            cause_kind=type(error).__name__,
        )
    finally:
        with response_lock:
            shared_state["response"] = None
            shared_state["phase"] = "finished"
        if response is not None:
            _close_response(response)
        _close_adapter(adapter)
        _TRANSACTION_SUBMISSION_DISPATCH_SLOT.release(dispatch_owner)
        if outcome is not None and not cancelled.is_set() and time.monotonic() <= deadline:
            try:
                outcomes.put_nowait(outcome)
            except queue.Full:
                pass


def _cancel_submission_worker(
    cancelled: threading.Event,
    dispatch_gate: threading.Event,
    response_lock: threading.Lock,
    shared_state: dict[str, Any],
    adapter: requests.adapters.HTTPAdapter,
) -> bool:
    cancelled.set()
    dispatch_gate.set()
    with response_lock:
        if shared_state["phase"] == "pending":
            shared_state["phase"] = "cancelled"
        response = shared_state["response"]
        dispatch_started = shared_state["dispatch_started"]
    if response is not None:
        _close_response(response)
    _close_adapter(adapter)
    return dispatch_started


def submit_transaction_once_v1(
    *,
    session: requests.Session,
    base_url: str,
    signed_transaction: bytes,
    inspect_transaction_submission: Callable[[bytes, str], tuple[bytes, str]],
    expected_receipt_signer: str,
    verify_receipt: Callable[[bytes, str, str], _VerifiedReceipt],
    headers: Optional[Mapping[str, str]] = None,
    timeout: float = TRANSACTION_SUBMISSION_TIMEOUT_SECONDS_V1,
) -> _VerifiedReceipt:
    """Authenticate, dispatch once, and return only native-verified receipt evidence.

    ``timeout`` bounds the network dispatch and receipt-body transport. The injected native
    inspector and verifier are trusted local code and must themselves be bounded. Submissions
    are process-wide single-flight: if the operating system never returns an in-flight network
    call, reconcile its hash and restart the process before submitting another transaction.
    """

    body, timeout_seconds = _validate_submission_inputs(
        signed_transaction,
        timeout,
    )
    canonical_base_url = _validate_base_url(base_url)
    if not callable(inspect_transaction_submission):
        raise TypeError("inspect_transaction_submission must be callable")
    if not callable(verify_receipt):
        raise TypeError("verify_receipt must be callable")
    if (
        type(expected_receipt_signer) is not str
        or not expected_receipt_signer
        or expected_receipt_signer.strip() != expected_receipt_signer
    ):
        raise ValueError("expected_receipt_signer must be an exact non-empty public-key literal")
    context = "submit_transaction"
    if getattr(session, "trust_env", None) is not False:
        raise ValueError("submit_transaction requires Session.trust_env to be false")
    transport_state = snapshot_one_shot_transport(session, context)
    if urlsplit(canonical_base_url).scheme == "http" and transport_state["proxies"]:
        raise ValueError("cleartext loopback transaction submission forbids proxies")
    request_headers = validate_fixed_request_headers(
        headers,
        context=context,
        allow_default_json_accept=True,
    )
    canonical_hash = _authenticate_signed_transaction(
        body,
        expected_receipt_signer,
        inspect_transaction_submission,
    )
    request_headers.update(
        {
            "Accept": "application/x-norito",
            "Accept-Encoding": "identity",
            "Content-Type": "application/x-norito",
        }
    )
    adapter, prepared, transport = prepare_one_shot_request(
        transport_state,
        canonical_base_url,
        TRANSACTION_SUBMISSION_ROUTE_V1,
        request_headers,
        body,
        context,
    )
    deadline = time.monotonic() + timeout_seconds
    cancelled = threading.Event()
    dispatch_gate = threading.Event()
    dispatch_owner = object()
    response_lock = threading.Lock()
    shared_state: dict[str, Any] = {
        "phase": "pending",
        "response": None,
        "dispatch_started": False,
    }
    outcomes: queue.Queue[_DispatchOutcome] = queue.Queue(maxsize=1)
    try:
        worker = threading.Thread(
            target=_dispatch_and_collect_submission,
            kwargs={
                "adapter": adapter,
                "prepared": prepared,
                "transport": transport,
                "hash_hex": canonical_hash,
                "timeout_seconds": timeout_seconds,
                "deadline": deadline,
                "dispatch_gate": dispatch_gate,
                "dispatch_owner": dispatch_owner,
                "cancelled": cancelled,
                "response_lock": response_lock,
                "shared_state": shared_state,
                "outcomes": outcomes,
            },
            name="iroha-transaction-submission-v1",
            daemon=True,
        )
        worker.start()
        if not _TRANSACTION_SUBMISSION_DISPATCH_SLOT.try_acquire(dispatch_owner):
            raise RuntimeError(
                "another transaction submission remains in flight; reconcile it by hash and "
                "restart the process if its network worker cannot terminate"
            )
        dispatch_gate.set()
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise queue.Empty
        outcome = outcomes.get(timeout=remaining)
        if time.monotonic() > deadline:
            raise queue.Empty

        if outcome.kind == "receipt":
            if outcome.body is None:
                raise RuntimeError("submission worker omitted the verified receipt body")
            try:
                return verify_receipt(outcome.body, canonical_hash, expected_receipt_signer)
            except BaseException as error:
                raise AmbiguousTransactionSubmissionError(
                    hash_hex=canonical_hash,
                    status_code=202,
                    reject_code=None,
                    body_preview=None,
                    body_truncated=False,
                    cause_kind=type(error).__name__,
                ) from None

        if outcome.kind == "ambiguous" or (
            outcome.status_code is not None
            and (
                transaction_submission_status_is_ambiguous(outcome.status_code)
                or not _has_definitive_rejection_evidence(
                    outcome.status_code,
                    outcome.reject_code,
                )
            )
        ):
            raise AmbiguousTransactionSubmissionError(
                hash_hex=canonical_hash,
                status_code=outcome.status_code,
                reject_code=outcome.reject_code,
                body_preview=outcome.body_preview,
                body_truncated=outcome.body_truncated,
                cause_kind=outcome.cause_kind,
            ) from None
        assert outcome.status_code is not None
        raise TransactionSubmissionHttpError(
            hash_hex=canonical_hash,
            status_code=outcome.status_code,
            reject_code=outcome.reject_code,
            body_preview=outcome.body_preview,
            body_truncated=outcome.body_truncated,
        ) from None
    except queue.Empty:
        dispatch_started = _cancel_submission_worker(
            cancelled,
            dispatch_gate,
            response_lock,
            shared_state,
            adapter,
        )
        if not dispatch_started:
            raise TransactionSubmissionNotDispatchedError(hash_hex=canonical_hash) from None
        raise AmbiguousTransactionSubmissionError(
            hash_hex=canonical_hash,
            status_code=None,
            reject_code=None,
            body_preview=None,
            body_truncated=False,
            cause_kind="WallClockTimeout",
        ) from None
    except (
        AmbiguousTransactionSubmissionError,
        TransactionSubmissionHttpError,
        TransactionSubmissionNotDispatchedError,
    ):
        raise
    except BaseException as error:
        dispatch_started = _cancel_submission_worker(
            cancelled,
            dispatch_gate,
            response_lock,
            shared_state,
            adapter,
        )
        if dispatch_started:
            raise AmbiguousTransactionSubmissionError(
                hash_hex=canonical_hash,
                status_code=None,
                reject_code=None,
                body_preview=None,
                body_truncated=False,
                cause_kind=type(error).__name__,
            ) from None
        raise
    finally:
        dispatch_gate.set()
