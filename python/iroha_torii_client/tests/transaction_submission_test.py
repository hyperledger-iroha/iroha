"""One-shot current transaction submission transport tests."""

from __future__ import annotations

import threading
import time
from typing import Any

import iroha_torii_client as torii_client
import iroha_torii_client.transaction_submission as transaction_submission
import pytest
import requests
from requests.structures import CaseInsensitiveDict

SIGNED_TRANSACTION = b"signed-v1"
HASH_HEX = "ab" * 32
HASH_BYTES = bytes.fromhex(HASH_HEX)
RECEIPT_BYTES = b"\x01receipt"
RECEIPT_SIGNER = "ed0120" + "11" * 32


class NativeCallbacks:
    def __init__(self) -> None:
        self.hash_bytes: Any = HASH_BYTES
        self.canonical_signer: Any = RECEIPT_SIGNER
        self.authentication_error: Exception | None = None
        self.verification_error: Exception | None = None
        self.verified_receipt: Any = {"verified": True}
        self.authenticate_calls: list[tuple[bytes, str]] = []
        self.verify_calls: list[tuple[bytes, str, str]] = []

    def inspect(self, body: bytes, expected_signer: str) -> tuple[bytes, str]:
        self.authenticate_calls.append((body, expected_signer))
        if self.authentication_error is not None:
            raise self.authentication_error
        return self.hash_bytes, self.canonical_signer

    def verify(self, receipt: bytes, hash_hex: str, expected_signer: str) -> Any:
        self.verify_calls.append((receipt, hash_hex, expected_signer))
        if self.verification_error is not None:
            raise self.verification_error
        return self.verified_receipt


class SignerEqualityImpostor:
    """Compare equal to signer text without supplying exact native string evidence."""

    def __eq__(self, other: object) -> bool:
        return other == RECEIPT_SIGNER


class RawHeaders:
    def __init__(self, duplicates: set[str] | None = None) -> None:
        self.duplicates = {name.lower() for name in duplicates or ()}

    def getlist(self, name: str) -> list[str]:
        if name.lower() in self.duplicates:
            return ["first", "second"]
        return []


class Response:
    def __init__(
        self,
        status_code: int = 202,
        *,
        body: bytes = RECEIPT_BYTES,
        headers: dict[str, str] | None = None,
        duplicates: set[str] | None = None,
        chunks: list[bytes] | None = None,
    ) -> None:
        self.status_code = status_code
        self.body = body
        self.chunks = chunks
        self.closed = False
        self.headers = CaseInsensitiveDict(
            {
                "Content-Type": "application/x-norito",
                "x-iroha-entrypoint-hash": HASH_HEX,
                "x-iroha-signed-transaction-hash": HASH_HEX,
                **(headers or {}),
            }
        )
        self.raw = type("Raw", (), {"headers": RawHeaders(duplicates)})()

    def iter_content(self, **_: Any):
        yield from self.chunks if self.chunks is not None else [self.body]

    def close(self) -> None:
        self.closed = True


class Transport:
    def __init__(self, outcome: Response | Exception) -> None:
        self.outcome = outcome
        self.calls: list[dict[str, Any]] = []


_TRANSPORTS: list[Transport] = []
_CLOSED_ADAPTERS: list[requests.adapters.HTTPAdapter] = []


@pytest.fixture(autouse=True)
def _patch_stock_adapter(monkeypatch: pytest.MonkeyPatch):
    original_close = transaction_submission._HTTP_ADAPTER_CLOSE

    def send(adapter: requests.adapters.HTTPAdapter, request: Any, **kwargs: Any):
        if not _TRANSPORTS:
            raise AssertionError("unexpected transaction dispatch")
        transport = _TRANSPORTS.pop(0)
        transport.calls.append({"request": request, **kwargs})
        if isinstance(transport.outcome, Exception):
            raise transport.outcome
        return transport.outcome

    def close(adapter: requests.adapters.HTTPAdapter) -> None:
        _CLOSED_ADAPTERS.append(adapter)
        original_close(adapter)

    monkeypatch.setattr(transaction_submission, "_HTTP_ADAPTER_SEND", send)
    monkeypatch.setattr(transaction_submission, "_HTTP_ADAPTER_CLOSE", close)
    yield
    _TRANSPORTS.clear()
    _CLOSED_ADAPTERS.clear()


def stock_submission(
    outcome: Response | Exception,
) -> tuple[requests.Session, Transport]:
    session = requests.Session()
    session.trust_env = False
    transport = Transport(outcome)
    _TRANSPORTS.append(transport)
    return session, transport


def wait_for_dispatch_slot_release() -> None:
    deadline = time.monotonic() + 1.0
    while time.monotonic() < deadline:
        if transaction_submission._TRANSACTION_SUBMISSION_DISPATCH_SLOT.is_available():
            return
        time.sleep(0.005)
    raise AssertionError("transaction submission worker did not release its dispatch slot")


def submit(
    session: requests.Session,
    callbacks: NativeCallbacks,
    *,
    base_url: str = "https://torii.example",
    signed_transaction: Any = SIGNED_TRANSACTION,
    headers: Any = None,
    timeout: Any = transaction_submission.TRANSACTION_SUBMISSION_TIMEOUT_SECONDS_V1,
) -> Any:
    return transaction_submission.submit_transaction_once_v1(
        session=session,
        base_url=base_url,
        signed_transaction=signed_transaction,
        inspect_transaction_submission=callbacks.inspect,
        expected_receipt_signer=RECEIPT_SIGNER,
        verify_receipt=callbacks.verify,
        headers=headers,
        timeout=timeout,
    )


@pytest.mark.parametrize(
    "name",
    [
        "submit_transaction_once_v1",
        "TransactionSubmissionHttpError",
        "TransactionSubmissionNotDispatchedError",
        "AmbiguousTransactionSubmissionError",
    ],
)
def test_current_submission_contract_is_exported(name: str) -> None:
    assert getattr(torii_client, name) is getattr(transaction_submission, name)


@pytest.mark.parametrize(
    "status_code, expected",
    [
        (202, False),
        (400, False),
        (403, False),
        (406, False),
        (413, False),
        (415, False),
        (200, True),
        (401, True),
        (409, True),
        (429, True),
        (460, True),
        (499, True),
        (503, True),
    ],
)
def test_status_classification_matches_exact_v1_contract(
    status_code: int,
    expected: bool,
) -> None:
    assert (
        transaction_submission.transaction_submission_status_is_ambiguous(status_code) is expected
    )


def test_dispatch_slot_ignores_stale_owner_release() -> None:
    slot = transaction_submission._TransactionSubmissionDispatchSlot()
    first_owner = object()
    second_owner = object()

    assert slot.try_acquire(first_owner)
    slot.release(first_owner)
    assert slot.try_acquire(second_owner)
    slot.release(first_owner)
    assert not slot.is_available()
    slot.release(second_owner)
    assert slot.is_available()


def test_exact_202_returns_only_native_verified_receipt_and_dispatches_once() -> None:
    response = Response()
    session, transport = stock_submission(response)
    callbacks = NativeCallbacks()

    verified = submit(session, callbacks)

    assert verified is callbacks.verified_receipt
    assert callbacks.authenticate_calls == [(SIGNED_TRANSACTION, RECEIPT_SIGNER)]
    assert callbacks.verify_calls == [(RECEIPT_BYTES, HASH_HEX, RECEIPT_SIGNER)]
    assert response.closed
    assert len(transport.calls) == 1
    call = transport.calls[0]
    assert call["request"].method == "POST"
    assert call["request"].url == "https://torii.example/v1/pipeline/transactions"
    assert call["request"].body == SIGNED_TRANSACTION
    assert call["request"].headers["Accept"] == "application/x-norito"
    assert call["request"].headers["Accept-Encoding"] == "identity"
    assert call["request"].headers["Content-Type"] == "application/x-norito"
    assert call["stream"] is True
    assert call["timeout"] == (30.0, 5.0)
    assert len(_CLOSED_ADAPTERS) == 1


def test_hash_is_derived_by_authenticator_and_bound_to_receipt() -> None:
    derived_hash = "cd" * 32
    response = Response(
        headers={
            "x-iroha-entrypoint-hash": derived_hash,
            "x-iroha-signed-transaction-hash": derived_hash,
        }
    )
    session, transport = stock_submission(response)
    callbacks = NativeCallbacks()
    callbacks.hash_bytes = bytes.fromhex(derived_hash)

    verified = submit(session, callbacks)

    assert verified is callbacks.verified_receipt
    assert callbacks.verify_calls == [(RECEIPT_BYTES, derived_hash, RECEIPT_SIGNER)]
    assert len(transport.calls) == 1


@pytest.mark.parametrize(
    "invalid_hash",
    [
        bytearray(HASH_BYTES),
        HASH_BYTES[:-1],
        bytes.fromhex("aa" * 32),
        HASH_HEX,
    ],
)
def test_invalid_authenticator_hash_fails_before_dispatch(invalid_hash: Any) -> None:
    session, transport = stock_submission(Response())
    callbacks = NativeCallbacks()
    callbacks.hash_bytes = invalid_hash

    with pytest.raises(RuntimeError, match="inspection returned invalid evidence"):
        submit(session, callbacks)

    assert callbacks.authenticate_calls == [(SIGNED_TRANSACTION, RECEIPT_SIGNER)]
    assert callbacks.verify_calls == []
    assert transport.calls == []


@pytest.mark.parametrize(
    "invalid_signer",
    [
        "ed0120" + "22" * 32,
        SignerEqualityImpostor(),
    ],
)
def test_inspector_must_canonically_validate_expected_receipt_signer(
    invalid_signer: Any,
) -> None:
    session, transport = stock_submission(Response())
    callbacks = NativeCallbacks()
    callbacks.canonical_signer = invalid_signer

    with pytest.raises(RuntimeError, match="inspection returned invalid evidence"):
        submit(session, callbacks)

    assert callbacks.authenticate_calls == [(SIGNED_TRANSACTION, RECEIPT_SIGNER)]
    assert callbacks.verify_calls == []
    assert transport.calls == []


def test_authenticator_failure_propagates_before_dispatch() -> None:
    session, transport = stock_submission(Response())
    callbacks = NativeCallbacks()
    callbacks.authentication_error = ValueError("native authentication rejected transaction")

    with pytest.raises(ValueError, match="native authentication rejected transaction"):
        submit(session, callbacks)

    assert callbacks.authenticate_calls == [(SIGNED_TRANSACTION, RECEIPT_SIGNER)]
    assert callbacks.verify_calls == []
    assert transport.calls == []


@pytest.mark.parametrize(
    "base_url",
    [
        "https://torii.example",
        "https://torii.example:8443",
        "http://127.0.0.1:8080",
        "http://127.9.8.7",
        "http://[::1]:8080",
    ],
)
def test_https_and_exact_loopback_http_origins_are_accepted(base_url: str) -> None:
    session, transport = stock_submission(Response())
    callbacks = NativeCallbacks()

    submit(session, callbacks, base_url=base_url)

    assert transport.calls[0]["request"].url == (
        f"{base_url}{transaction_submission.TRANSACTION_SUBMISSION_ROUTE_V1}"
    )
    assert len(transport.calls) == 1


@pytest.mark.parametrize(
    "base_url",
    [
        "http://torii.example",
        "http://192.0.2.1:8080",
        "http://localhost",
        "http://localhost:8080",
        "http://localhost.example",
        "ftp://torii.example",
        "https://user@torii.example",
        "https://torii.example/path",
        "https://torii.example?query=one",
        "https://torii.example#fragment",
        "https://torii.example/",
    ],
)
def test_noncanonical_or_cleartext_remote_origins_fail_before_dispatch(
    base_url: str,
) -> None:
    session, transport = stock_submission(Response())
    callbacks = NativeCallbacks()

    with pytest.raises(ValueError, match="base_url|cleartext"):
        submit(session, callbacks, base_url=base_url)

    assert callbacks.authenticate_calls == []
    assert transport.calls == []


def test_session_must_disable_environment_credentials_and_proxies() -> None:
    session = requests.Session()
    transport = Transport(Response())
    _TRANSPORTS.append(transport)
    callbacks = NativeCallbacks()

    with pytest.raises(ValueError, match="Session.trust_env"):
        submit(session, callbacks)

    assert callbacks.authenticate_calls == []
    assert transport.calls == []


def test_cleartext_loopback_forbids_explicit_proxy_before_authentication() -> None:
    session, transport = stock_submission(Response())
    session.proxies["http"] = "http://proxy.example:3128"
    callbacks = NativeCallbacks()

    with pytest.raises(ValueError, match="forbids proxies"):
        submit(session, callbacks, base_url="http://127.0.0.1:8080")

    assert callbacks.authenticate_calls == []
    assert transport.calls == []


def test_explicit_auth_and_session_defaults_are_preserved_with_tuple_timeout() -> None:
    session, transport = stock_submission(Response())
    callbacks = NativeCallbacks()
    default_user_agent = session.headers["User-Agent"]
    session.headers["Authorization"] = "Bearer session-default"
    session.headers["X-Default"] = "default"

    submit(
        session,
        callbacks,
        headers={
            "Accept": "application/json",
            "Authorization": "Bearer explicit",
            "X-Trace": "one",
        },
        timeout=2.5,
    )

    assert len(transport.calls) == 1
    call = transport.calls[0]
    request_headers = call["request"].headers
    assert request_headers["Authorization"] == "Bearer explicit"
    assert request_headers["X-Default"] == "default"
    assert request_headers["X-Trace"] == "one"
    assert request_headers["User-Agent"] == default_user_agent
    assert request_headers["Accept"] == "application/x-norito"
    assert call["timeout"] == (2.5, 2.5)
    assert call["verify"] is True
    assert call["cert"] is None
    assert call["proxies"] == {}


@pytest.mark.parametrize(
    "status_code, reject_code",
    [
        (400, "transaction_rejected"),
        (400, None),
        (403, "PRTRY:QUEUE_GOVERNANCE_REJECTED"),
        (403, None),
        (406, None),
        (413, None),
        (415, None),
    ],
)
def test_exact_documented_rejections_fail_once(
    status_code: int,
    reject_code: str | None,
) -> None:
    headers = {} if reject_code is None else {"x-iroha-reject-code": reject_code}
    response = Response(
        status_code,
        body=b"invalid transaction",
        headers=headers,
    )
    session, transport = stock_submission(response)
    callbacks = NativeCallbacks()

    with pytest.raises(transaction_submission.TransactionSubmissionHttpError) as caught:
        submit(session, callbacks)

    assert caught.value.hash_hex == HASH_HEX
    assert caught.value.status_code == status_code
    assert caught.value.reject_code == reject_code
    assert caught.value.body_preview == "invalid transaction"
    assert caught.value.body_truncated is False
    assert callbacks.verify_calls == []
    assert len(transport.calls) == 1
    assert response.closed


@pytest.mark.parametrize(
    "status_code",
    [
        100,
        199,
        200,
        201,
        204,
        300,
        307,
        308,
        401,
        404,
        408,
        409,
        425,
        429,
        460,
        499,
        500,
        503,
        504,
    ],
)
def test_unknown_outcome_statuses_are_ambiguous_and_never_redispatched(
    status_code: int,
) -> None:
    response = Response(
        status_code,
        body=b"outcome unknown",
        headers={"x-iroha-reject-code": "PRTRY:QUEUE_FULL"},
    )
    session, transport = stock_submission(response)
    callbacks = NativeCallbacks()

    with pytest.raises(transaction_submission.AmbiguousTransactionSubmissionError) as caught:
        submit(session, callbacks)

    assert caught.value.hash_hex == HASH_HEX
    assert caught.value.status_code == status_code
    assert caught.value.reject_code == "PRTRY:QUEUE_FULL"
    assert callbacks.verify_calls == []
    assert len(transport.calls) == 1
    assert response.closed


@pytest.mark.parametrize(
    "status_code, reject_code",
    [
        (400, "PRTRY:QUEUE_FULL"),
        (403, "PRTRY:TX_SIGNATURE_INVALID"),
        (406, "transaction_rejected"),
        (413, "invalid_transaction_payload"),
        (415, "PRTRY:QUEUE_GOVERNANCE_REJECTED"),
        (400, "bad code"),
    ],
)
def test_contradictory_or_noncanonical_rejection_evidence_is_ambiguous(
    status_code: int,
    reject_code: str,
) -> None:
    response = Response(
        status_code,
        body=b"contradictory rejection",
        headers={"x-iroha-reject-code": reject_code},
    )
    session, transport = stock_submission(response)

    with pytest.raises(transaction_submission.AmbiguousTransactionSubmissionError):
        submit(session, NativeCallbacks())

    assert len(transport.calls) == 1
    assert response.closed


def test_transport_exception_retains_only_bounded_reconciliation_evidence() -> None:
    session, transport = stock_submission(ConnectionError("secret transport text"))
    callbacks = NativeCallbacks()

    with pytest.raises(transaction_submission.AmbiguousTransactionSubmissionError) as caught:
        submit(session, callbacks)

    assert caught.value.hash_hex == HASH_HEX
    assert caught.value.status_code is None
    assert caught.value.cause_kind == "ConnectionError"
    assert caught.value.__cause__ is None
    assert "secret transport text" not in str(caught.value)
    assert callbacks.verify_calls == []
    assert len(transport.calls) == 1


def test_wall_clock_timeout_covers_slow_status_and_headers(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    entered = threading.Event()
    release = threading.Event()
    calls: list[Any] = []

    def blocking_send(*args: Any, **kwargs: Any) -> Response:
        calls.append((args, kwargs))
        entered.set()
        release.wait(2.0)
        return Response()

    monkeypatch.setattr(transaction_submission, "_HTTP_ADAPTER_SEND", blocking_send)
    session = requests.Session()
    session.trust_env = False
    callbacks = NativeCallbacks()
    started_at = time.monotonic()
    try:
        with pytest.raises(transaction_submission.AmbiguousTransactionSubmissionError) as caught:
            submit(session, callbacks, timeout=0.03)
        second_session = requests.Session()
        second_session.trust_env = False
        with pytest.raises(RuntimeError, match="remains in flight"):
            submit(second_session, NativeCallbacks())
    finally:
        release.set()
        wait_for_dispatch_slot_release()

    assert entered.is_set()
    assert time.monotonic() - started_at < 0.5
    assert caught.value.cause_kind == "WallClockTimeout"
    assert len(calls) == 1
    assert callbacks.verify_calls == []


def test_timeout_that_wins_pending_state_prevents_late_dispatch(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    entered = threading.Event()
    release = threading.Event()
    send_calls: list[Any] = []
    original_worker = transaction_submission._dispatch_and_collect_submission

    def gated_worker(**kwargs: Any) -> None:
        entered.set()
        release.wait(2.0)
        original_worker(**kwargs)

    def forbidden_late_send(*args: Any, **kwargs: Any) -> Response:
        send_calls.append((args, kwargs))
        return Response()

    monkeypatch.setattr(
        transaction_submission,
        "_dispatch_and_collect_submission",
        gated_worker,
    )
    monkeypatch.setattr(transaction_submission, "_HTTP_ADAPTER_SEND", forbidden_late_send)
    session = requests.Session()
    session.trust_env = False
    callbacks = NativeCallbacks()
    try:
        with pytest.raises(
            transaction_submission.TransactionSubmissionNotDispatchedError
        ) as caught:
            submit(session, callbacks, timeout=0.03)
        assert entered.is_set()
        assert caught.value.hash_hex == HASH_HEX
        assert "may be retried" in str(caught.value)
        assert send_calls == []
    finally:
        release.set()
        wait_for_dispatch_slot_release()

    assert send_calls == []


def test_wall_clock_timeout_covers_slow_trickled_body() -> None:
    entered = threading.Event()
    release = threading.Event()

    class SlowBodyResponse(Response):
        def iter_content(self, **_: Any):
            entered.set()
            release.wait(2.0)
            yield RECEIPT_BYTES

    response = SlowBodyResponse()
    session, transport = stock_submission(response)
    callbacks = NativeCallbacks()
    started_at = time.monotonic()
    try:
        with pytest.raises(transaction_submission.AmbiguousTransactionSubmissionError) as caught:
            submit(session, callbacks, timeout=0.03)
        assert response.closed
    finally:
        release.set()
        wait_for_dispatch_slot_release()

    assert entered.is_set()
    assert time.monotonic() - started_at < 0.5
    assert caught.value.cause_kind == "WallClockTimeout"
    assert len(transport.calls) == 1
    assert callbacks.verify_calls == []


def test_interruption_after_dispatch_is_typed_as_ambiguous(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    entered = threading.Event()
    release = threading.Event()

    def blocking_send(*_args: Any, **_kwargs: Any) -> Response:
        entered.set()
        release.wait(2.0)
        return Response()

    def interrupt_after_dispatch(*_args: Any, **_kwargs: Any) -> Any:
        if not entered.wait(1.0):
            raise AssertionError("dispatch worker did not start")
        raise KeyboardInterrupt

    monkeypatch.setattr(transaction_submission, "_HTTP_ADAPTER_SEND", blocking_send)
    monkeypatch.setattr(transaction_submission.queue.Queue, "get", interrupt_after_dispatch)
    session = requests.Session()
    session.trust_env = False
    callbacks = NativeCallbacks()
    try:
        with pytest.raises(transaction_submission.AmbiguousTransactionSubmissionError) as caught:
            submit(session, callbacks)
    finally:
        release.set()
        wait_for_dispatch_slot_release()

    assert caught.value.cause_kind == "KeyboardInterrupt"
    assert caught.value.hash_hex == HASH_HEX
    assert callbacks.verify_calls == []


def test_thread_start_interruption_cannot_dispatch_or_leak_slot(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    original_worker = transaction_submission._dispatch_and_collect_submission
    original_start = threading.Thread.start
    worker_exited = threading.Event()

    def observed_worker(**kwargs: Any) -> None:
        try:
            original_worker(**kwargs)
        finally:
            worker_exited.set()

    def start_then_interrupt(worker: threading.Thread) -> None:
        original_start(worker)
        raise KeyboardInterrupt

    monkeypatch.setattr(
        transaction_submission,
        "_dispatch_and_collect_submission",
        observed_worker,
    )
    monkeypatch.setattr(threading.Thread, "start", start_then_interrupt)
    session, transport = stock_submission(Response())

    with pytest.raises(KeyboardInterrupt):
        submit(session, NativeCallbacks())

    assert worker_exited.wait(1.0)
    assert transport.calls == []
    assert transaction_submission._TRANSACTION_SUBMISSION_DISPATCH_SLOT.is_available()


def test_interruption_after_slot_acquisition_cancels_before_opening_dispatch_gate(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    slot = transaction_submission._TRANSACTION_SUBMISSION_DISPATCH_SLOT
    original_try_acquire = slot.try_acquire
    original_worker = transaction_submission._dispatch_and_collect_submission
    worker_exited = threading.Event()

    def observed_worker(**kwargs: Any) -> None:
        try:
            original_worker(**kwargs)
        finally:
            worker_exited.set()

    def acquire_then_interrupt(owner: object) -> bool:
        assert original_try_acquire(owner)
        raise KeyboardInterrupt

    monkeypatch.setattr(
        transaction_submission,
        "_dispatch_and_collect_submission",
        observed_worker,
    )
    monkeypatch.setattr(slot, "try_acquire", acquire_then_interrupt)
    session, transport = stock_submission(Response())

    with pytest.raises(KeyboardInterrupt):
        submit(session, NativeCallbacks())

    assert worker_exited.wait(1.0)
    assert transport.calls == []
    assert slot.is_available()


def test_interruption_during_post_dispatch_classification_is_typed_ambiguous(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    session, transport = stock_submission(
        Response(400, headers={"x-iroha-reject-code": "transaction_rejected"})
    )

    def interrupt_classification(*_args: Any, **_kwargs: Any) -> bool:
        raise KeyboardInterrupt

    monkeypatch.setattr(
        transaction_submission,
        "_has_definitive_rejection_evidence",
        interrupt_classification,
    )
    with pytest.raises(transaction_submission.AmbiguousTransactionSubmissionError) as caught:
        submit(session, NativeCallbacks())

    assert caught.value.cause_kind == "KeyboardInterrupt"
    assert len(transport.calls) == 1


@pytest.mark.parametrize(
    "status_code, reject_code",
    [
        (400, "transaction_rejected"),
        (503, "route_unavailable"),
    ],
)
def test_error_evidence_is_bounded(status_code: int, reject_code: str) -> None:
    response = Response(
        status_code,
        body=b"x" * 4096,
        headers={"x-iroha-reject-code": reject_code},
    )
    session, transport = stock_submission(response)
    callbacks = NativeCallbacks()
    expected_error = (
        transaction_submission.AmbiguousTransactionSubmissionError
        if status_code == 503
        else transaction_submission.TransactionSubmissionHttpError
    )

    with pytest.raises(expected_error) as caught:
        submit(session, callbacks)

    assert caught.value.reject_code == reject_code
    assert caught.value.body_preview == "x" * 512
    assert caught.value.body_truncated is True
    assert len(transport.calls) == 1


@pytest.mark.parametrize(
    "headers, duplicates",
    [
        ({"x-iroha-entrypoint-hash": "cd" * 32}, None),
        ({"x-iroha-signed-transaction-hash": "CD" * 32}, None),
        ({"Content-Type": "application/json"}, None),
        ({"Content-Encoding": "gzip"}, None),
        ({}, {"x-iroha-entrypoint-hash"}),
        ({}, {"x-iroha-reject-code"}),
    ],
)
def test_malformed_202_evidence_is_ambiguous(
    headers: dict[str, str],
    duplicates: set[str] | None,
) -> None:
    response = Response(headers=headers, duplicates=duplicates)
    session, transport = stock_submission(response)
    callbacks = NativeCallbacks()

    with pytest.raises(transaction_submission.AmbiguousTransactionSubmissionError) as caught:
        submit(session, callbacks)

    assert caught.value.status_code == 202
    assert caught.value.cause_kind == "RuntimeError"
    assert callbacks.verify_calls == []
    assert len(transport.calls) == 1
    assert response.closed


@pytest.mark.parametrize("reject_code", ["TX_REJECTED", "bad code", "x" * 129])
def test_202_reject_code_is_contradictory_and_ambiguous(reject_code: str) -> None:
    response = Response(headers={"x-iroha-reject-code": reject_code})
    session, transport = stock_submission(response)
    callbacks = NativeCallbacks()

    with pytest.raises(transaction_submission.AmbiguousTransactionSubmissionError) as caught:
        submit(session, callbacks)

    assert caught.value.hash_hex == HASH_HEX
    assert caught.value.status_code == 202
    assert caught.value.reject_code == (
        reject_code if transaction_submission._REJECT_CODE.fullmatch(reject_code) else None
    )
    assert caught.value.cause_kind == "RuntimeError"
    assert callbacks.verify_calls == []
    assert len(transport.calls) == 1
    assert response.closed


@pytest.mark.parametrize(
    "body, chunks",
    [
        (b"", None),
        (
            RECEIPT_BYTES,
            [b"x" * (transaction_submission.TRANSACTION_SUBMISSION_MAX_RESPONSE_BYTES_V1 + 1)],
        ),
        (RECEIPT_BYTES, ["not bytes"]),
    ],
)
def test_invalid_202_receipt_bodies_are_ambiguous(
    body: bytes,
    chunks: Any,
) -> None:
    response = Response(body=body, chunks=chunks)
    session, transport = stock_submission(response)
    callbacks = NativeCallbacks()

    with pytest.raises(transaction_submission.AmbiguousTransactionSubmissionError) as caught:
        submit(session, callbacks)

    assert caught.value.status_code == 202
    assert caught.value.cause_kind == "RuntimeError"
    assert callbacks.verify_calls == []
    assert len(transport.calls) == 1
    assert response.closed


def test_invalid_202_headers_are_rejected_before_body_read() -> None:
    class HeaderFailureResponse(Response):
        body_read = False

        def iter_content(self, **_: Any):
            self.body_read = True
            raise AssertionError("invalid headers must fail before receipt body read")
            yield b""  # pragma: no cover

    response = HeaderFailureResponse(headers={"Content-Type": "application/json"})
    session, transport = stock_submission(response)
    callbacks = NativeCallbacks()

    with pytest.raises(transaction_submission.AmbiguousTransactionSubmissionError) as caught:
        submit(session, callbacks)

    assert caught.value.cause_kind == "RuntimeError"
    assert response.body_read is False
    assert len(transport.calls) == 1


def test_receipt_verifier_failure_is_ambiguous_without_redispatch() -> None:
    response = Response()
    session, transport = stock_submission(response)
    callbacks = NativeCallbacks()
    callbacks.verification_error = ValueError("native receipt verification failed")

    with pytest.raises(transaction_submission.AmbiguousTransactionSubmissionError) as caught:
        submit(session, callbacks)

    assert caught.value.hash_hex == HASH_HEX
    assert caught.value.status_code == 202
    assert caught.value.reject_code is None
    assert caught.value.cause_kind == "ValueError"
    assert caught.value.__cause__ is None
    assert "native receipt verification failed" not in str(caught.value)
    assert callbacks.verify_calls == [(RECEIPT_BYTES, HASH_HEX, RECEIPT_SIGNER)]
    assert len(transport.calls) == 1
    assert response.closed


def test_replaying_or_mutated_session_is_rejected_before_dispatch() -> None:
    class ReplayingSession(requests.Session):
        def request(self, *args: Any, **kwargs: Any):
            raise AssertionError("custom request must never run")

    session = ReplayingSession()
    session.trust_env = False
    transport = Transport(Response())
    _TRANSPORTS.append(transport)
    callbacks = NativeCallbacks()

    with pytest.raises(ValueError, match="unmodified one-shot"):
        submit(session, callbacks)

    assert callbacks.authenticate_calls == []
    assert transport.calls == []


def test_configured_post_retry_adapter_is_never_used() -> None:
    response = Response()
    session, transport = stock_submission(response)
    callbacks = NativeCallbacks()
    retrying = requests.adapters.HTTPAdapter(max_retries=5)
    retrying.send = lambda *_args, **_kwargs: (_ for _ in ()).throw(
        AssertionError("mounted retry adapter must never run")
    )
    session.mount("https://", retrying)

    verified = submit(session, callbacks)

    assert verified is callbacks.verified_receipt
    assert len(transport.calls) == 1


@pytest.mark.parametrize(
    "body, timeout, error_type",
    [
        (bytearray(SIGNED_TRANSACTION), 30.0, TypeError),
        (b"", 30.0, ValueError),
        (SIGNED_TRANSACTION, 0, ValueError),
        (SIGNED_TRANSACTION, float("inf"), ValueError),
        (SIGNED_TRANSACTION, True, TypeError),
    ],
)
def test_invalid_inputs_fail_before_authentication_and_dispatch(
    body: Any,
    timeout: Any,
    error_type: type[Exception],
) -> None:
    session, transport = stock_submission(Response())
    callbacks = NativeCallbacks()

    with pytest.raises(error_type):
        submit(session, callbacks, signed_transaction=body, timeout=timeout)

    assert callbacks.authenticate_calls == []
    assert callbacks.verify_calls == []
    assert transport.calls == []


@pytest.mark.parametrize("missing_callback", ["inspect", "verify"])
def test_callbacks_must_be_callable_before_authentication_or_dispatch(
    missing_callback: str,
) -> None:
    session, transport = stock_submission(Response())
    callbacks = NativeCallbacks()
    arguments = {
        "session": session,
        "base_url": "https://torii.example",
        "signed_transaction": SIGNED_TRANSACTION,
        "inspect_transaction_submission": (
            None if missing_callback == "inspect" else callbacks.inspect
        ),
        "expected_receipt_signer": RECEIPT_SIGNER,
        "verify_receipt": None if missing_callback == "verify" else callbacks.verify,
    }

    with pytest.raises(TypeError, match=f"{missing_callback}.*callable"):
        transaction_submission.submit_transaction_once_v1(**arguments)

    assert callbacks.authenticate_calls == []
    assert callbacks.verify_calls == []
    assert transport.calls == []


@pytest.mark.parametrize("expected_signer", [None, "", " ed0120", "ed0120 "])
def test_expected_receipt_signer_is_required_before_authentication(
    expected_signer: Any,
) -> None:
    session, transport = stock_submission(Response())
    callbacks = NativeCallbacks()

    with pytest.raises(ValueError, match="expected_receipt_signer"):
        transaction_submission.submit_transaction_once_v1(
            session=session,
            base_url="https://torii.example",
            signed_transaction=SIGNED_TRANSACTION,
            inspect_transaction_submission=callbacks.inspect,
            expected_receipt_signer=expected_signer,
            verify_receipt=callbacks.verify,
        )

    assert callbacks.authenticate_calls == []
    assert callbacks.verify_calls == []
    assert transport.calls == []


def test_noncanonical_receipt_signer_is_rejected_by_inspector_before_dispatch() -> None:
    session, transport = stock_submission(Response())
    callbacks = NativeCallbacks()

    with pytest.raises(RuntimeError, match="inspection returned invalid evidence"):
        transaction_submission.submit_transaction_once_v1(
            session=session,
            base_url="https://torii.example",
            signed_transaction=SIGNED_TRANSACTION,
            inspect_transaction_submission=callbacks.inspect,
            expected_receipt_signer="not-a-key",
            verify_receipt=callbacks.verify,
        )

    assert callbacks.authenticate_calls == [(SIGNED_TRANSACTION, "not-a-key")]
    assert callbacks.verify_calls == []
    assert transport.calls == []
