from __future__ import annotations

import pytest

from iroha_python import kagemusha

RECURSIVE_AGGREGATION_METHOD = (
    "kagemusha_prove_verified_recursive_aggregation_proof_bundle"
    "_with_records_and_pallas_open_envelopes"
)
RECURSIVE_SPEND_METHODS = (
    "kagemusha_recursive_spend_init",
    "kagemusha_recursive_spend_append",
    "kagemusha_recursive_spend_transition_profile_init",
    "kagemusha_recursive_spend_transition_profile_append",
    "kagemusha_recursive_spend_lineage_append_boundary",
    "kagemusha_recursive_spend_lineage_witness_from_init_result",
    "kagemusha_recursive_spend_lineage_witness_append_result",
    "kagemusha_recursive_spend_verify",
    "kagemusha_recursive_spend_redeem",
)
MALFORMED_PROBE_ARCHIVE = b"\x00"


def _is_malformed_probe_archive(value: bytes) -> bool:
    return bytes(value) == MALFORMED_PROBE_ARCHIVE


class _Native:
    def __init__(self) -> None:
        self.calls: list[tuple[str, bytes]] = []
        setattr(self, RECURSIVE_AGGREGATION_METHOD, self._recursive_aggregation)

    def _reject_probe(self, context: str, *archives: bytes) -> None:
        if archives and all(_is_malformed_probe_archive(archive) for archive in archives):
            raise ValueError(f"invalid Kagemusha {context} probe archive")

    def kagemusha_recursive_spend_bridge_abi_version(self) -> int:
        return 6

    def kagemusha_prove_verified_compact_payment_token_with_records(
        self,
        record_bundle: bytes,
    ) -> bytes:
        self._reject_probe("compact", record_bundle)
        self.calls.append(("compact", record_bundle))
        return b"compact"

    def _recursive_aggregation(
        self,
        record_bundle: bytes,
        pallas_open_envelopes: bytes,
    ) -> bytes:
        self._reject_probe("recursive aggregation", record_bundle, pallas_open_envelopes)
        self.calls.append(
            ("recursive_aggregation", record_bundle + b"|" + pallas_open_envelopes)
        )
        return b"recursive_aggregation"

    def kagemusha_recursive_spend_init(self, request: bytes) -> bytes:
        self._reject_probe("init", request)
        self.calls.append(("init", request))
        return b"init"

    def kagemusha_recursive_spend_append(self, request: bytes) -> bytes:
        self._reject_probe("append", request)
        self.calls.append(("append", request))
        return b"append"

    def kagemusha_recursive_spend_transition_profile_init(self, request: bytes) -> bytes:
        self._reject_probe("transition profile init", request)
        self.calls.append(("transition-profile-init", request))
        return b"transition-profile-init"

    def kagemusha_recursive_spend_transition_profile_append(self, request: bytes) -> bytes:
        self._reject_probe("transition profile append", request)
        self.calls.append(("transition-profile-append", request))
        return b"transition-profile-append"

    def kagemusha_recursive_spend_lineage_append_boundary(self, profile: bytes) -> bytes:
        self._reject_probe("lineage append boundary", profile)
        self.calls.append(("lineage-append-boundary", profile))
        return b"lineage-append-boundary"

    def kagemusha_recursive_spend_lineage_witness_from_init_result(
        self,
        request: bytes,
        bundle: bytes,
    ) -> bytes:
        self._reject_probe("lineage init", request, bundle)
        self.calls.append(("lineage-init", request + b"|" + bundle))
        return b"lineage-init"

    def kagemusha_recursive_spend_lineage_witness_append_result(
        self,
        previous_witness: bytes,
        request: bytes,
        bundle: bytes,
    ) -> bytes:
        self._reject_probe("lineage append", previous_witness, request, bundle)
        self.calls.append(("lineage-append", previous_witness + b"|" + request + b"|" + bundle))
        return b"lineage-append"

    def kagemusha_recursive_spend_verify(self, request: bytes) -> bytes:
        self._reject_probe("verify", request)
        self.calls.append(("verify", request))
        return b"verify"

    def kagemusha_recursive_spend_redeem(self, request: bytes) -> bytes:
        self._reject_probe("redeem", request)
        self.calls.append(("redeem", request))
        return b"redeem"


def test_recursive_kagemusha_helpers_reject_empty_requests(monkeypatch: pytest.MonkeyPatch) -> None:
    native = _Native()
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    for helper in (
        kagemusha.kagemusha_recursive_spend_init,
        kagemusha.kagemusha_recursive_spend_append,
        kagemusha.kagemusha_recursive_spend_transition_profile_init,
        kagemusha.kagemusha_recursive_spend_transition_profile_append,
        kagemusha.kagemusha_recursive_spend_verify,
        kagemusha.kagemusha_recursive_spend_redeem,
    ):
        with pytest.raises(ValueError, match="request_archive must not be empty"):
            helper(b"")
    with pytest.raises(ValueError, match="profile_archive must not be empty"):
        kagemusha.kagemusha_recursive_spend_lineage_append_boundary(b"")
    with pytest.raises(ValueError, match="request_archive must not be empty"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(b"", b"bundle")
    with pytest.raises(ValueError, match="bundle_archive must not be empty"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(b"request", b"")
    with pytest.raises(ValueError, match="previous_witness_archive must not be empty"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(b"", b"request", b"bundle")
    with pytest.raises(ValueError, match="request_archive must not be empty"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(b"witness", b"", b"bundle")
    with pytest.raises(ValueError, match="bundle_archive must not be empty"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(b"witness", b"request", b"")

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
    assert (
        kagemusha.kagemusha_recursive_spend_transition_profile_init(b"ti")
        == b"transition-profile-init"
    )
    assert (
        kagemusha.kagemusha_recursive_spend_transition_profile_append(b"ta")
        == b"transition-profile-append"
    )
    assert (
        kagemusha.kagemusha_recursive_spend_lineage_append_boundary(b"profile")
        == b"lineage-append-boundary"
    )
    assert (
        kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(
            b"request",
            b"bundle",
        )
        == b"lineage-init"
    )
    assert (
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            b"witness",
            b"request",
            b"bundle",
        )
        == b"lineage-append"
    )
    assert kagemusha.kagemusha_recursive_spend_verify(memoryview(b"c")) == b"verify"
    assert kagemusha.kagemusha_recursive_spend_redeem(b"d") == b"redeem"
    assert native.calls == [
        ("compact", b"r"),
        ("recursive_aggregation", b"r|p"),
        ("init", b"a"),
        ("append", b"b"),
        ("transition-profile-init", b"ti"),
        ("transition-profile-append", b"ta"),
        ("lineage-append-boundary", b"profile"),
        ("lineage-init", b"request|bundle"),
        ("lineage-append", b"witness|request|bundle"),
        ("verify", b"c"),
        ("redeem", b"d"),
    ]


def test_recursive_kagemusha_availability_rejects_permissive_native_probes(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    for method_name in (
        "kagemusha_prove_verified_compact_payment_token_with_records",
        RECURSIVE_AGGREGATION_METHOD,
        *RECURSIVE_SPEND_METHODS,
    ):
        native = _Native()
        if method_name == RECURSIVE_AGGREGATION_METHOD:
            setattr(native, method_name, lambda record, pallas: b"accepted")
        elif method_name == "kagemusha_recursive_spend_lineage_witness_from_init_result":
            setattr(native, method_name, lambda request, bundle: b"accepted")
        elif method_name == "kagemusha_recursive_spend_lineage_witness_append_result":
            setattr(native, method_name, lambda witness, request, bundle: b"accepted")
        else:
            setattr(native, method_name, lambda archive: b"accepted")
        monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda native=native: native)

        if method_name == "kagemusha_prove_verified_compact_payment_token_with_records":
            assert kagemusha.is_kagemusha_compact_payment_token_prover_available() is False
        elif method_name == RECURSIVE_AGGREGATION_METHOD:
            assert (
                kagemusha.is_kagemusha_recursive_aggregation_proof_bundle_prover_available()
                is False
            )
        else:
            assert kagemusha.is_kagemusha_recursive_spend_available() is False
            with pytest.raises(RuntimeError, match="reject malformed probe archives"):
                kagemusha.kagemusha_recursive_spend_verify(b"request")


def test_recursive_kagemusha_exports_stable_circuit_ids() -> None:
    assert kagemusha.KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION == 6
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
        == "kagemusha-recursive-aggregation-v1"
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
        == "kagemusha-recursive-spend-lineage-v1"
    )
    assert kagemusha.KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS == 64
    assert kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 == 64
    assert kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1
        == 1
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES
        == 8 * 1024 * 1024
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES
        == 128
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN
        == "iroha:kagemusha:v1:recursive-spend-transition-profile"
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN
        == "iroha:kagemusha:v1:recursive-spend-transition-profile-digest"
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN
        == "iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest"
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1
        == "iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1"
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1
        == "iroha:kagemusha:recursive-spend-lineage-append-boundary:v1"
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1
        == "iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1"
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1
        == "iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1"
    )
    assert (
        kagemusha.normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
            None,
        )
        == kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
    )
    assert (
        kagemusha.normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
            "",
        )
        == kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
    )
    assert (
        kagemusha.normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        )
        == kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
    )
    assert (
        kagemusha.normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
            "unknown-kagemusha-recursive-spend-circuit",
        )
        == "unknown-kagemusha-recursive-spend-circuit"
    )
    for circuit_id in (
        None,
        "",
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    ):
        assert (
            kagemusha.is_supported_kagemusha_recursive_spend_append_output_proof_circuit_id(
                circuit_id,
            )
        )
    assert not (
        kagemusha.is_supported_kagemusha_recursive_spend_append_output_proof_circuit_id(
            "unknown-kagemusha-recursive-spend-circuit",
        )
    )
    assert kagemusha.is_supported_kagemusha_recursive_spend_previous_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    )
    assert kagemusha.is_supported_kagemusha_recursive_spend_previous_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    )
    for previous_circuit_id in (
        "unknown-kagemusha-recursive-spend-circuit",
        None,
        True,
    ):
        assert not (
            kagemusha.is_supported_kagemusha_recursive_spend_previous_proof_circuit_id(
                previous_circuit_id,  # type: ignore[arg-type]
            )
        )
    assert kagemusha.is_supported_kagemusha_recursive_spend_append_proof_transition(
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    )
    assert kagemusha.is_supported_kagemusha_recursive_spend_append_proof_transition(
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        "",
    )
    assert kagemusha.is_supported_kagemusha_recursive_spend_append_proof_transition(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    )
    assert kagemusha.is_supported_kagemusha_recursive_spend_append_proof_transition(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    )
    assert not kagemusha.is_supported_kagemusha_recursive_spend_append_proof_transition(
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    )
    assert not kagemusha.is_supported_kagemusha_recursive_spend_append_proof_transition(
        "unknown-kagemusha-recursive-spend-circuit",
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    )
    assert not kagemusha.is_supported_kagemusha_recursive_spend_append_proof_transition(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        "unknown-kagemusha-recursive-spend-circuit",
    )
    assert not (
        kagemusha.requires_kagemusha_recursive_spend_previous_lineage_verifier_record_for_append(
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        )
    )
    assert (
        kagemusha.requires_kagemusha_recursive_spend_previous_lineage_verifier_record_for_append(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        )
    )
    for previous_circuit_id in (
        "unknown-kagemusha-recursive-spend-circuit",
        None,
        True,
    ):
        assert not (
            kagemusha.requires_kagemusha_recursive_spend_previous_lineage_verifier_record_for_append(
                previous_circuit_id,  # type: ignore[arg-type]
            )
        )
    assert kagemusha.can_redeem_kagemusha_recursive_spend_witnessless(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert not kagemusha.requires_kagemusha_recursive_spend_lineage_witness_for_redeem(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert kagemusha.can_redeem_kagemusha_recursive_spend_witnessless(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        64,
    )
    assert not kagemusha.requires_kagemusha_recursive_spend_lineage_witness_for_redeem(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        64,
    )
    for circuit_id, hop_count in (
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 1),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 0),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, -1),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 65),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 2**63),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 1.5),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, float("nan")),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, float("inf")),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, float("-inf")),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, True),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, "1"),
        (None, 1),
        ("", 1),
        ("unknown-kagemusha-recursive-spend-circuit", 1),
    ):
        assert not kagemusha.can_redeem_kagemusha_recursive_spend_witnessless(
            circuit_id,
            hop_count,  # type: ignore[arg-type]
        )
        assert kagemusha.requires_kagemusha_recursive_spend_lineage_witness_for_redeem(
            circuit_id,
            hop_count,  # type: ignore[arg-type]
        )
    assert not kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(0)
    assert kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(1)
    assert kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(63)
    assert not kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(64)
    assert not kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(-1)
    assert not kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(2**63)
    for previous_hop_count in (
        1.5,
        float("nan"),
        float("inf"),
        float("-inf"),
        True,
        "1",
    ):
        assert not (
            kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(
                previous_hop_count,  # type: ignore[arg-type]
            )
        )
    assert (
        kagemusha.preferred_kagemusha_recursive_spend_append_output_proof_circuit_id(
            1,
        )
        == kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
    )
    assert (
        kagemusha.preferred_kagemusha_recursive_spend_append_output_proof_circuit_id(
            63,
        )
        == kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
    )
    assert (
        kagemusha.preferred_kagemusha_recursive_spend_append_output_proof_circuit_id(
            64,
        )
        == kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
    ), "preferred append selector falls back at the witnessless hop cap"
    assert (
        kagemusha.preferred_kagemusha_recursive_spend_append_output_proof_circuit_id(
            0,
        )
        == kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
    )
    assert kagemusha.can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert kagemusha.can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
        None,
        1,
    )
    assert kagemusha.can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS - 1,
    )
    assert kagemusha.can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert kagemusha.can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        63,
    )
    for circuit_id, previous_hop_count in (
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 0),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
        ),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 64),
        ("unknown-kagemusha-recursive-spend-circuit", 1),
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 1.5),
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, float("nan")),
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, float("inf")),
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, float("-inf")),
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, True),
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, "1"),
    ):
        assert not (
            kagemusha.can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
                circuit_id,
                previous_hop_count,  # type: ignore[arg-type]
            )
        )
    assert kagemusha.can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert kagemusha.can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert kagemusha.can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert not (
        kagemusha.can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            1,
        )
    ), "semantic previous proofs cannot select Reserved-lineage output"
    for previous_circuit_id, output_circuit_id, previous_hop_count in (
        (
            "unknown-kagemusha-recursive-spend-circuit",
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            1,
        ),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            1,
        ),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            "unknown-kagemusha-recursive-spend-circuit",
            1,
        ),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            0,
        ),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            1.5,
        ),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            float("nan"),
        ),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            float("inf"),
        ),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            float("-inf"),
        ),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            True,
        ),
    ):
        assert not (
            kagemusha.can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
                previous_circuit_id,
                output_circuit_id,
                previous_hop_count,  # type: ignore[arg-type]
            )
        )
    assert (
        kagemusha.requires_kagemusha_recursive_spend_previous_proof_open_envelopes_for_append(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            1,
        )
    )
    assert (
        kagemusha.requires_kagemusha_recursive_spend_previous_proof_open_envelopes_for_append(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            64,
        )
    )
    for circuit_id, previous_hop_count in (
        ("", 1),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 0),
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 1),
        ("unknown-kagemusha-recursive-spend-circuit", 1),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 1.5),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, float("nan")),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, float("inf")),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, float("-inf")),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, True),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, "1"),
    ):
        assert not (
            kagemusha.requires_kagemusha_recursive_spend_previous_proof_open_envelopes_for_append(
                circuit_id,
                previous_hop_count,  # type: ignore[arg-type]
            )
        )


def test_recursive_kagemusha_availability_requires_bridge_abi_6(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()
    native.kagemusha_recursive_spend_bridge_abi_version = lambda: 5
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    assert kagemusha.is_kagemusha_recursive_spend_available() is False
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode()
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1
    )
    with pytest.raises(RuntimeError, match="native bridge ABI 6"):
        kagemusha.kagemusha_recursive_spend_init(b"request")


def test_recursive_kagemusha_availability_rejects_broken_abi_probe(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()

    def broken_abi_probe() -> int:
        raise OSError("bridge denied")

    native.kagemusha_recursive_spend_bridge_abi_version = broken_abi_probe
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    assert kagemusha.is_kagemusha_recursive_spend_available() is False
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode()
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1
    )
    with pytest.raises(RuntimeError, match="native bridge ABI 6"):
        kagemusha.kagemusha_recursive_spend_init(b"request")


def test_recursive_kagemusha_helpers_require_complete_abi_surface(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class PartialNative:
        def kagemusha_recursive_spend_bridge_abi_version(self) -> int:
            return 6

        def kagemusha_recursive_spend_init(self, request: bytes) -> bytes:
            return b"init"

        def kagemusha_recursive_spend_append(self, request: bytes) -> bytes:
            return b"append"

        def kagemusha_recursive_spend_transition_profile_init(self, request: bytes) -> bytes:
            return b"transition-profile-init"

        def kagemusha_recursive_spend_transition_profile_append(self, request: bytes) -> bytes:
            return b"transition-profile-append"

        def kagemusha_recursive_spend_lineage_witness_from_init_result(
            self,
            request: bytes,
            bundle: bytes,
        ) -> bytes:
            return b"lineage-init"

        def kagemusha_recursive_spend_verify(self, request: bytes) -> bytes:
            return b"verify"

        def kagemusha_recursive_spend_redeem(self, request: bytes) -> bytes:
            return b"redeem"

    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: PartialNative())

    assert kagemusha.is_kagemusha_recursive_spend_available() is False
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode()
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1
    )
    with pytest.raises(RuntimeError, match="complete native bridge ABI 6 surface"):
        kagemusha.kagemusha_recursive_spend_init(b"request")


@pytest.mark.parametrize("missing_method", RECURSIVE_SPEND_METHODS)
def test_recursive_kagemusha_helpers_reject_each_missing_abi_method(
    monkeypatch: pytest.MonkeyPatch,
    missing_method: str,
) -> None:
    native = _Native()
    setattr(native, missing_method, None)
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    assert kagemusha.is_kagemusha_recursive_spend_available() is False
    with pytest.raises(RuntimeError, match="complete native bridge ABI 6 surface"):
        kagemusha.kagemusha_recursive_spend_verify(b"request")


def test_recursive_kagemusha_helpers_reject_empty_native_outputs(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()

    def empty_one(archive: bytes) -> bytes:
        native._reject_probe("empty one", archive)
        return b""

    def empty_two(first: bytes, second: bytes) -> bytes:
        native._reject_probe("empty two", first, second)
        return b""

    def empty_three(first: bytes, second: bytes, third: bytes) -> bytes:
        native._reject_probe("empty three", first, second, third)
        return b""

    native.kagemusha_prove_verified_compact_payment_token_with_records = empty_one
    setattr(native, RECURSIVE_AGGREGATION_METHOD, empty_two)
    native.kagemusha_recursive_spend_init = empty_one
    native.kagemusha_recursive_spend_append = empty_one
    native.kagemusha_recursive_spend_transition_profile_init = empty_one
    native.kagemusha_recursive_spend_transition_profile_append = empty_one
    native.kagemusha_recursive_spend_lineage_append_boundary = empty_one
    native.kagemusha_recursive_spend_lineage_witness_from_init_result = empty_two
    native.kagemusha_recursive_spend_lineage_witness_append_result = empty_three
    native.kagemusha_recursive_spend_verify = empty_one
    native.kagemusha_recursive_spend_redeem = empty_one
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    with pytest.raises(RuntimeError, match="returned empty output"):
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(b"record")
    with pytest.raises(RuntimeError, match="returned empty output"):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(b"record", b"pallas")

    for helper in (
        kagemusha.kagemusha_recursive_spend_init,
        kagemusha.kagemusha_recursive_spend_append,
        kagemusha.kagemusha_recursive_spend_transition_profile_init,
        kagemusha.kagemusha_recursive_spend_transition_profile_append,
        kagemusha.kagemusha_recursive_spend_lineage_append_boundary,
        kagemusha.kagemusha_recursive_spend_verify,
        kagemusha.kagemusha_recursive_spend_redeem,
    ):
        with pytest.raises(RuntimeError, match="returned empty output"):
            helper(b"request")
    with pytest.raises(RuntimeError, match="returned empty output"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(
            b"request",
            b"bundle",
        )
    with pytest.raises(RuntimeError, match="returned empty output"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            b"witness",
            b"request",
            b"bundle",
        )


def test_recursive_kagemusha_helpers_reject_missing_native_outputs(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()

    def missing_one(archive: bytes) -> None:
        native._reject_probe("missing one", archive)
        return None

    def missing_two(first: bytes, second: bytes) -> None:
        native._reject_probe("missing two", first, second)
        return None

    def missing_three(first: bytes, second: bytes, third: bytes) -> None:
        native._reject_probe("missing three", first, second, third)
        return None

    native.kagemusha_prove_verified_compact_payment_token_with_records = missing_one
    setattr(native, RECURSIVE_AGGREGATION_METHOD, missing_two)
    native.kagemusha_recursive_spend_init = missing_one
    native.kagemusha_recursive_spend_append = missing_one
    native.kagemusha_recursive_spend_transition_profile_init = missing_one
    native.kagemusha_recursive_spend_transition_profile_append = missing_one
    native.kagemusha_recursive_spend_lineage_append_boundary = missing_one
    native.kagemusha_recursive_spend_lineage_witness_from_init_result = missing_two
    native.kagemusha_recursive_spend_lineage_witness_append_result = missing_three
    native.kagemusha_recursive_spend_verify = missing_one
    native.kagemusha_recursive_spend_redeem = missing_one
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    with pytest.raises(RuntimeError, match="returned no output"):
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(b"record")
    with pytest.raises(RuntimeError, match="returned no output"):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(b"record", b"pallas")

    for helper in (
        kagemusha.kagemusha_recursive_spend_init,
        kagemusha.kagemusha_recursive_spend_append,
        kagemusha.kagemusha_recursive_spend_transition_profile_init,
        kagemusha.kagemusha_recursive_spend_transition_profile_append,
        kagemusha.kagemusha_recursive_spend_lineage_append_boundary,
        kagemusha.kagemusha_recursive_spend_verify,
        kagemusha.kagemusha_recursive_spend_redeem,
    ):
        with pytest.raises(RuntimeError, match="returned no output"):
            helper(b"request")
    with pytest.raises(RuntimeError, match="returned no output"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(
            b"request",
            b"bundle",
        )
    with pytest.raises(RuntimeError, match="returned no output"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            b"witness",
            b"request",
            b"bundle",
        )


def test_recursive_kagemusha_helpers_reject_native_text_outputs(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()

    def text_one(archive: bytes) -> str:
        native._reject_probe("text one", archive)
        return "not-norito"

    def text_two(first: bytes, second: bytes) -> str:
        native._reject_probe("text two", first, second)
        return "not-norito"

    def text_three(first: bytes, second: bytes, third: bytes) -> str:
        native._reject_probe("text three", first, second, third)
        return "not-norito"

    native.kagemusha_prove_verified_compact_payment_token_with_records = text_one
    setattr(native, RECURSIVE_AGGREGATION_METHOD, text_two)
    native.kagemusha_recursive_spend_init = text_one
    native.kagemusha_recursive_spend_append = text_one
    native.kagemusha_recursive_spend_transition_profile_init = text_one
    native.kagemusha_recursive_spend_transition_profile_append = text_one
    native.kagemusha_recursive_spend_lineage_append_boundary = text_one
    native.kagemusha_recursive_spend_lineage_witness_from_init_result = text_two
    native.kagemusha_recursive_spend_lineage_witness_append_result = text_three
    native.kagemusha_recursive_spend_verify = text_one
    native.kagemusha_recursive_spend_redeem = text_one
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    with pytest.raises(RuntimeError, match="returned text instead of Norito bytes"):
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(b"record")
    with pytest.raises(RuntimeError, match="returned text instead of Norito bytes"):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(b"record", b"pallas")

    for helper in (
        kagemusha.kagemusha_recursive_spend_init,
        kagemusha.kagemusha_recursive_spend_append,
        kagemusha.kagemusha_recursive_spend_transition_profile_init,
        kagemusha.kagemusha_recursive_spend_transition_profile_append,
        kagemusha.kagemusha_recursive_spend_lineage_append_boundary,
        kagemusha.kagemusha_recursive_spend_verify,
        kagemusha.kagemusha_recursive_spend_redeem,
    ):
        with pytest.raises(RuntimeError, match="returned text instead of Norito bytes"):
            helper(b"request")
    with pytest.raises(RuntimeError, match="returned text instead of Norito bytes"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(
            b"request",
            b"bundle",
        )
    with pytest.raises(RuntimeError, match="returned text instead of Norito bytes"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            b"witness",
            b"request",
            b"bundle",
        )


def test_recursive_kagemusha_redeem_propagates_native_multi_hop_lineage_rejection(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()
    calls: list[bytes] = []

    def rejecting_redeem(request: bytes) -> bytes:
        native._reject_probe("redeem", request)
        calls.append(request)
        raise RuntimeError(
            "invalid Kagemusha recursive spend request: bundle.accumulator.hop_count"
        )

    native.kagemusha_recursive_spend_redeem = rejecting_redeem
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    assert kagemusha.is_kagemusha_recursive_spend_available() is True
    with pytest.raises(RuntimeError, match=r"bundle\.accumulator\.hop_count"):
        kagemusha.kagemusha_recursive_spend_redeem(b"request")
    assert calls == [b"request"]


def test_recursive_kagemusha_helpers_propagate_forged_lineage_record_rejection(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()
    calls: list[tuple[str, bytes]] = []

    def rejecting_verify(request: bytes) -> bytes:
        native._reject_probe("verify", request)
        calls.append(("verify", request))
        raise RuntimeError(
            "invalid Kagemusha recursive spend request: lineage_verifier_record.commitment"
        )

    def rejecting_redeem(request: bytes) -> bytes:
        native._reject_probe("redeem", request)
        calls.append(("redeem", request))
        raise RuntimeError(
            "invalid Kagemusha recursive spend request: lineage_verifier_record.commitment"
        )

    native.kagemusha_recursive_spend_verify = rejecting_verify
    native.kagemusha_recursive_spend_redeem = rejecting_redeem
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    assert kagemusha.is_kagemusha_recursive_spend_available() is True
    with pytest.raises(RuntimeError, match=r"lineage_verifier_record\.commitment"):
        kagemusha.kagemusha_recursive_spend_verify(b"verify-request")
    with pytest.raises(RuntimeError, match=r"lineage_verifier_record\.commitment"):
        kagemusha.kagemusha_recursive_spend_redeem(b"redeem-request")
    assert calls == [
        ("verify", b"verify-request"),
        ("redeem", b"redeem-request"),
    ]


def test_recursive_kagemusha_transition_profile_append_propagates_forged_opening_rejection(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()
    calls: list[bytes] = []

    def rejecting_transition_profile_append(request: bytes) -> bytes:
        native._reject_probe("transition profile append", request)
        calls.append(request)
        raise RuntimeError(
            "invalid Kagemusha recursive spend request: hop domain metadata mismatch"
        )

    native.kagemusha_recursive_spend_transition_profile_append = (
        rejecting_transition_profile_append
    )
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    assert kagemusha.is_kagemusha_recursive_spend_available() is True
    with pytest.raises(RuntimeError, match="hop domain metadata mismatch"):
        kagemusha.kagemusha_recursive_spend_transition_profile_append(
            b"transition-profile-request"
        )
    assert calls == [b"transition-profile-request"]


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
