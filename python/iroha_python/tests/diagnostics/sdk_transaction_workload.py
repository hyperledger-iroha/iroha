"""Real Python SDK host diagnostic; no HTTP, ledger writes or production hooks.

Invoke through the original resource harness with ``success``, ``signing_failure``
or ``exception``. SDK/native imports and public fixture construction happen only
when a workload is called. Importing this module needs no native extension.

Every recorded label is static. Only actual transaction buffer lengths and
completed public-operation counts leave these controls. Keys, payloads, hashes,
signatures and exception arguments stay in memory. Buffer sizes are not native
allocation or pool-funding measurements. An authenticated matching native SDK is
required to execute this diagnostic; a Python executable pin alone is not enough.
"""

from __future__ import annotations

import argparse
from contextlib import nullcontext
from dataclasses import dataclass
import os
from pathlib import Path
import sys
from typing import Any, Callable


TRANSACTIONS = 128
CHAIN_DISCRIMINANT = 753


@dataclass(frozen=True, repr=False)
class _Fixture:
    draft: Any
    crypto: Any
    network_id: Any
    authority: str
    public_key: bytes
    private_key: bytes
    wrong_private_key: bytes


@dataclass(frozen=True, repr=False)
class _Result:
    payload_json: str
    frame: bytes
    transaction_hash: bytes
    signature: bytes
    envelope_json: str


class DiagnosticBoundaryError(RuntimeError):
    """Deliberate test exception, separate from an actual SDK signing failure."""


def _require(condition: bool, code: str) -> None:
    # Static messages only; never format actual SDK output into a failed check.
    if not condition:
        raise AssertionError(code)


def _fixture() -> _Fixture:
    # Existing public fixtures: transaction_account_context_test.py uses these
    # two 32-byte keys, this NetworkId, the selected prefix and this instruction.
    import json
    from iroha_python import (
        Ed25519KeyPair,
        Instruction,
        NetworkId,
        TransactionConfig,
        TransactionDraft,
    )
    import iroha_python.crypto as crypto

    key = Ed25519KeyPair.from_private_key(bytes([0x51]) * 32)
    wrong_key = Ed25519KeyPair.from_private_key(bytes([0x52]) * 32)
    network_id = NetworkId.from_bytes(bytes([0xA5]) * 32)
    authority = key.account_id(discriminant=CHAIN_DISCRIMINANT)
    config = TransactionConfig(
        network_id=network_id,
        authority=authority,
        fee_payment={
            "payer": "authority",
            "value": {"charge_limits": [], "gas_limit": 1000},
        },
        chain_discriminant=CHAIN_DISCRIMINANT,
        creation_time_ms=42,
        ttl_ms=30000,
        nonce=7,
    )
    draft = TransactionDraft(config)
    draft.add_instruction(
        Instruction.from_json(
            json.dumps({
                "name": "SetAssetHoldingLimit",
                "params": {
                    "account_id": authority,
                    "asset_definition_id": "66owaQmAQMuHxPzxUN3bqZ6FJfDa",
                    "holding_limit": "100",
                },
            }),
            chain_discriminant=CHAIN_DISCRIMINANT,
        )
    )
    _require(type(key.private_key) is bytes and len(key.private_key) == 32,
             "fixture_key_shape")
    _require(type(wrong_key.private_key) is bytes and len(wrong_key.private_key) == 32,
             "wrong_fixture_key_shape")
    _require(key.public_key != wrong_key.public_key, "fixture_authority_collision")
    return _Fixture(draft, crypto, network_id, authority, key.public_key,
                    key.private_key, wrong_key.private_key)


def _phase(session: Any, label: str) -> Any:
    # The unmeasured control calls the identical SDK operations with no recorder.
    return nullcontext() if session is None else session.phase(label)


def _bytes(session: Any, label: str, length: int) -> None:
    if session is not None:
        session.bytes(label, length)


def _work(session: Any, label: str) -> None:
    if session is not None:
        session.work(label, 1)


def _prepare(fixture: _Fixture, session: Any) -> str:
    with _phase(session, "prepare"):
        builder = fixture.draft.to_builder()
        payload_json = builder.payload_json()
        _bytes(session, "payload_json_utf8", len(payload_json.encode("utf-8")))
        _work(session, "prepared_transactions")
        return payload_json


def _one(fixture: _Fixture, session: Any) -> _Result:
    payload_json = _prepare(fixture, session)
    with _phase(session, "sign"):
        envelope = fixture.draft.sign(fixture.private_key)
        _work(session, "signed_transactions")
    with _phase(session, "canonical_frame"):
        # This getter materializes the actual native-produced canonical wire.
        # Encoding already occurred during sign; do not attribute it to this phase.
        frame = envelope.signed_transaction_versioned
        transaction_hash = envelope.hash
        signature = envelope.signature
        _require(type(frame) is bytes and len(frame) > 0, "canonical_frame_shape")
        _bytes(session, "signed_transaction_versioned", len(frame))
        _work(session, "canonical_frames")
    with _phase(session, "decode_verify"):
        # Native readback rejects noncanonical bytes, the wrong network and bad
        # authority signatures; no replacement frame or decoder is supplied here.
        rebuilt = fixture.crypto.signed_transaction_envelope_from_versioned_v1(
            frame, fixture.network_id, chain_discriminant=CHAIN_DISCRIMINANT,
        )
        _require(rebuilt.signed_transaction_versioned == frame, "readback_wire")
        _require(rebuilt.hash == transaction_hash, "readback_hash")
        _require(rebuilt.signature == signature, "readback_signature")
        _require(rebuilt.public_key == fixture.public_key, "readback_public_key")
        _require(rebuilt.authority == fixture.authority, "readback_authority")
        _require(rebuilt.network_id.to_bytes() == fixture.network_id.to_bytes(),
                 "readback_network")
        _work(session, "decoded_transactions")
    with _phase(session, "json_roundtrip"):
        envelope_json = envelope.to_json()
        imported = fixture.crypto.signed_transaction_envelope_from_json(
            envelope_json, chain_discriminant=CHAIN_DISCRIMINANT,
        )
        _require(imported.signed_transaction_versioned == frame, "json_wire")
        _require(imported.hash == transaction_hash, "json_hash")
        _require(imported.signature == signature, "json_signature")
        _require(imported.public_key == fixture.public_key, "json_public_key")
        _require(imported.authority == fixture.authority, "json_authority")
        _require(imported.network_id.to_bytes() == fixture.network_id.to_bytes(),
                 "json_network")
        _require(imported.to_json() == envelope_json, "json_roundtrip_bytes")
        _bytes(session, "envelope_json_utf8", len(envelope_json.encode("utf-8")))
        _work(session, "json_roundtrips")
    return _Result(payload_json, frame, transaction_hash, signature, envelope_json)


def _many(fixture: _Fixture, session: Any, count: int) -> tuple[_Result, ...]:
    with _phase(session, "sdk_calls"):
        return tuple(_one(fixture, session) for _ in range(count))


def _observe_success(
    context: Any, workload: str, fixture: _Fixture, count: int,
    sink: Callable[[Any], None], expected: tuple[_Result, ...],
) -> tuple[_Result, ...]:
    from measurement_record import observe

    completed: list[tuple[_Result, ...]] = []

    def operation(session: Any) -> tuple[_Result, ...]:
        result = _many(fixture, session, count)
        completed.append(result)
        return result

    result = observe(context, workload, operation, sink)
    _require(len(completed) == 1 and result is completed[0], "operation_return_identity")
    # These assertions judge the diagnostic, never an SDK validity decision.
    _require(result == expected, "measured_and_unmeasured_results")
    _require(fixture.draft.to_builder().payload_json() == expected[0].payload_json,
             "successful_draft_unchanged")
    return result


def run_success(context: Any, sink: Callable[[Any], None]) -> tuple[_Result, ...]:
    """Compare 128 actual measured transactions with identical unmeasured calls."""
    fixture = _fixture()
    expected = _many(fixture, None, TRANSACTIONS)
    return _observe_success(context, "python.sdk.transaction.128", fixture,
                            TRANSACTIONS, sink, expected)


def _signing_failure(
    fixture: _Fixture, session: Any, caught: list[BaseException],
) -> None:
    with _phase(session, "sdk_calls"):
        _prepare(fixture, session)
        with _phase(session, "sign"):
            try:
                fixture.draft.sign(fixture.wrong_private_key)
            except BaseException as original:
                caught.append(original)
                raise
        raise AssertionError("wrong_authority_was_accepted")


def _compare_original_error(
    original: BaseException, captured: list[BaseException], expected: BaseException,
) -> None:
    _require(len(captured) == 1 and original is captured[0], "exception_identity")
    # Exception types and arguments are compared only in memory, never recorded.
    _require(type(original) is type(expected), "exception_type")
    _require(original.args == expected.args, "exception_arguments")


def run_signing_failure(
    context: Any, failed_sink: Callable[[Any], None], retry_sink: Callable[[Any], None],
) -> tuple[_Result, ...]:
    """Retain a real wrong-authority failure, then retry the same valid draft."""
    from measurement_record import observe

    fixture = _fixture()
    expected_result = _many(fixture, None, 1)
    baseline: list[BaseException] = []
    try:
        _signing_failure(fixture, None, baseline)
    except ValueError as expected:
        expected_error = expected
    else:
        raise AssertionError("unmeasured_signing_failure_missing")
    _require(len(baseline) == 1 and baseline[0] is expected_error,
             "unmeasured_exception_identity")
    _require(fixture.draft.to_builder().payload_json() == expected_result[0].payload_json,
             "unmeasured_failure_draft_unchanged")
    captured: list[BaseException] = []
    try:
        observe(context, "python.sdk.signing_failure",
                lambda session: _signing_failure(fixture, session, captured),
                failed_sink, expected_error=ValueError)
    except ValueError as original:
        _compare_original_error(original, captured, expected_error)
    else:
        raise AssertionError("measured_signing_failure_missing")
    _require(fixture.draft.to_builder().payload_json() == expected_result[0].payload_json,
             "measured_failure_draft_unchanged")
    return _observe_success(context, "python.sdk.retry", fixture, 1,
                            retry_sink, expected_result)


def _exception_boundary(
    fixture: _Fixture, session: Any, sentinel: DiagnosticBoundaryError,
    caught: list[BaseException],
) -> None:
    with _phase(session, "sdk_calls"):
        _prepare(fixture, session)
        with _phase(session, "exception_boundary"):
            try:
                raise sentinel
            except BaseException as original:
                caught.append(original)
                raise


def run_exception(
    context: Any, unwound_sink: Callable[[Any], None], retry_sink: Callable[[Any], None],
) -> tuple[_Result, ...]:
    """Retain a deliberate unwind after real preparation, then retry unchanged input."""
    from measurement_record import observe

    fixture = _fixture()
    expected_result = _many(fixture, None, 1)
    baseline: list[BaseException] = []
    expected_error = DiagnosticBoundaryError()
    try:
        _exception_boundary(fixture, None, expected_error, baseline)
    except DiagnosticBoundaryError as original:
        _require(original is expected_error, "unmeasured_boundary_identity")
    else:
        raise AssertionError("unmeasured_boundary_missing")
    _require(len(baseline) == 1 and baseline[0] is expected_error,
             "unmeasured_boundary_capture")
    captured: list[BaseException] = []
    sentinel = DiagnosticBoundaryError()
    try:
        observe(context, "python.sdk.exception",
                lambda session: _exception_boundary(fixture, session, sentinel, captured),
                unwound_sink)
    except DiagnosticBoundaryError as original:
        _require(original is sentinel, "original_boundary_identity")
        _compare_original_error(original, captured, expected_error)
    else:
        raise AssertionError("measured_boundary_missing")
    _require(fixture.draft.to_builder().payload_json() == expected_result[0].payload_json,
             "unwound_draft_unchanged")
    return _observe_success(context, "python.sdk.retry", fixture, 1,
                            retry_sink, expected_result)


def main(argv: list[str] | None = None) -> int:
    """Run one explicit diagnostic mode using the original harness context."""
    parser = argparse.ArgumentParser(
        description="Run the real Python SDK host diagnostic through the resource harness.",
    )
    parser.add_argument("mode", choices=("success", "signing_failure", "exception"))
    arguments = parser.parse_args(argv)
    output = os.environ.get("IROHA_MEASUREMENT_OUTPUT_DIR")
    if not output:
        sys.stderr.write("measurement output directory is required\n")
        return 2
    try:
        from measurement_record import directory_sink, read_context

        output_dir = Path(output)
        context = read_context(output_dir)
        if arguments.mode == "success":
            success_sink = directory_sink(output_dir, "sdk-python-success")
            run_success(context, success_sink)
            success_sink.check()
        elif arguments.mode == "signing_failure":
            failed_sink = directory_sink(output_dir, "sdk-python-signing-failure")
            retry_sink = directory_sink(output_dir, "sdk-python-retry")
            run_signing_failure(context, failed_sink, retry_sink)
            failed_sink.check()
            retry_sink.check()
        else:
            unwound_sink = directory_sink(output_dir, "sdk-python-exception")
            retry_sink = directory_sink(output_dir, "sdk-python-retry")
            run_exception(context, unwound_sink, retry_sink)
            unwound_sink.check()
            retry_sink.check()
    except BaseException:
        # The CLI never formats an SDK exception or traceback into public output.
        # observe has already retained any started failed/unwound measurement.
        sys.stderr.write("sdk diagnostic failed\n")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
