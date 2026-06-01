from __future__ import annotations

import pytest

from iroha_python import kagemusha

RECURSIVE_AGGREGATION_METHOD = (
    "kagemusha_prove_verified_recursive_aggregation_proof_bundle"
    "_with_records_and_pallas_open_envelopes"
)


class _Native:
    def __init__(self) -> None:
        self.calls: list[tuple[str, bytes]] = []
        setattr(self, RECURSIVE_AGGREGATION_METHOD, self._recursive_aggregation)

    def kagemusha_prove_verified_compact_payment_token_with_records(
        self,
        record_bundle: bytes,
    ) -> bytes:
        self.calls.append(("compact", record_bundle))
        return b"compact"

    def _recursive_aggregation(
        self,
        record_bundle: bytes,
        pallas_open_envelopes: bytes,
    ) -> bytes:
        self.calls.append(
            ("recursive_aggregation", record_bundle + b"|" + pallas_open_envelopes)
        )
        return b"recursive_aggregation"

    def kagemusha_recursive_spend_init(self, request: bytes) -> bytes:
        self.calls.append(("init", request))
        return b"init"

    def kagemusha_recursive_spend_append(self, request: bytes) -> bytes:
        self.calls.append(("append", request))
        return b"append"

    def kagemusha_recursive_spend_verify(self, request: bytes) -> bytes:
        self.calls.append(("verify", request))
        return b"verify"

    def kagemusha_recursive_spend_redeem(self, request: bytes) -> bytes:
        self.calls.append(("redeem", request))
        return b"redeem"


def test_recursive_kagemusha_helpers_reject_empty_requests(monkeypatch: pytest.MonkeyPatch) -> None:
    native = _Native()
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    for helper in (
        kagemusha.kagemusha_recursive_spend_init,
        kagemusha.kagemusha_recursive_spend_append,
        kagemusha.kagemusha_recursive_spend_verify,
        kagemusha.kagemusha_recursive_spend_redeem,
    ):
        with pytest.raises(ValueError, match="request_archive must not be empty"):
            helper(b"")

    assert native.calls == []


def test_kagemusha_native_prover_helpers_reject_empty_requests(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    with pytest.raises(ValueError, match="record_bundle_archive must not be empty"):
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(b"")

    with pytest.raises(ValueError, match="record_bundle_archive must not be empty"):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            b"",
            b"pallas",
        )

    with pytest.raises(ValueError, match="pallas_open_envelopes_archive must not be empty"):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            b"record",
            b"",
        )

    assert native.calls == []


def test_recursive_kagemusha_helpers_probe_and_delegate(monkeypatch: pytest.MonkeyPatch) -> None:
    native = _Native()
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    assert kagemusha.is_kagemusha_compact_payment_token_prover_available() is True
    assert kagemusha.is_kagemusha_recursive_aggregation_proof_bundle_prover_available() is True
    assert kagemusha.is_kagemusha_recursive_spend_available() is True
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode(True)
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1
    )
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode(False)
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1
    )
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode()
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1
    )
    assert (
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(b"r")
        == b"compact"
    )
    recursive_aggregation = getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)
    assert recursive_aggregation(b"r", b"p") == b"recursive_aggregation"
    assert kagemusha.kagemusha_recursive_spend_init(b"a") == b"init"
    assert kagemusha.kagemusha_recursive_spend_append(bytearray(b"b")) == b"append"
    assert kagemusha.kagemusha_recursive_spend_verify(memoryview(b"c")) == b"verify"
    assert kagemusha.kagemusha_recursive_spend_redeem(b"d") == b"redeem"
    assert native.calls == [
        ("compact", b"r"),
        ("recursive_aggregation", b"r|p"),
        ("init", b"a"),
        ("append", b"b"),
        ("verify", b"c"),
        ("redeem", b"d"),
    ]


def test_recursive_kagemusha_helpers_reject_empty_native_outputs(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()
    native.kagemusha_prove_verified_compact_payment_token_with_records = lambda request: b""
    setattr(native, RECURSIVE_AGGREGATION_METHOD, lambda record, pallas: b"")
    native.kagemusha_recursive_spend_init = lambda request: b""
    native.kagemusha_recursive_spend_append = lambda request: b""
    native.kagemusha_recursive_spend_verify = lambda request: b""
    native.kagemusha_recursive_spend_redeem = lambda request: b""
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    with pytest.raises(RuntimeError, match="returned empty output"):
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(b"record")
    with pytest.raises(RuntimeError, match="returned empty output"):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(b"record", b"pallas")

    for helper in (
        kagemusha.kagemusha_recursive_spend_init,
        kagemusha.kagemusha_recursive_spend_append,
        kagemusha.kagemusha_recursive_spend_verify,
        kagemusha.kagemusha_recursive_spend_redeem,
    ):
        with pytest.raises(RuntimeError, match="returned empty output"):
            helper(b"request")


def test_recursive_kagemusha_helpers_reject_missing_native_outputs(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()
    native.kagemusha_prove_verified_compact_payment_token_with_records = lambda request: None
    setattr(native, RECURSIVE_AGGREGATION_METHOD, lambda record, pallas: None)
    native.kagemusha_recursive_spend_init = lambda request: None
    native.kagemusha_recursive_spend_append = lambda request: None
    native.kagemusha_recursive_spend_verify = lambda request: None
    native.kagemusha_recursive_spend_redeem = lambda request: None
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    with pytest.raises(RuntimeError, match="returned no output"):
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(b"record")
    with pytest.raises(RuntimeError, match="returned no output"):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(b"record", b"pallas")

    for helper in (
        kagemusha.kagemusha_recursive_spend_init,
        kagemusha.kagemusha_recursive_spend_append,
        kagemusha.kagemusha_recursive_spend_verify,
        kagemusha.kagemusha_recursive_spend_redeem,
    ):
        with pytest.raises(RuntimeError, match="returned no output"):
            helper(b"request")


def test_recursive_kagemusha_availability_fails_closed(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: object())

    assert kagemusha.is_kagemusha_compact_payment_token_prover_available() is False
    assert kagemusha.is_kagemusha_recursive_aggregation_proof_bundle_prover_available() is False
    assert kagemusha.is_kagemusha_recursive_spend_available() is False
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode()
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1
    )
    with pytest.raises(RuntimeError, match="Kagemusha support"):
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(b"x")
    with pytest.raises(RuntimeError, match="recursive Kagemusha support"):
        kagemusha.kagemusha_recursive_spend_init(b"x")
