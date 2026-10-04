"""Current native private-counter construction, bounded transport and verification.

Counts and categorical groups are computed by native nodes. This module retains
original frames, delegates certificate collection and admission to the native
model, and exposes a result only after independently anchored verification.
"""

from __future__ import annotations

import json
import math
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
from types import MappingProxyType
from typing import Any, Mapping, Sequence

import requests

MAX_PRIVATE_COUNTER_FRAME_BYTES_V1 = 64 * 1024
_MAX_POLICY = 64 * 1024
_MAX_MANIFEST = 1024 * 1024
_MAX_CHAIN = 16 * 1024 * 1024
_MAX_CHECKPOINT = 68 * 1024 * 1024
_MAX_AUTHORED_EXECUTABLE = 32 * 1024 * 1024
_MAX_PROJECTION = 4 * MAX_PRIVATE_COUNTER_FRAME_BYTES_V1
_MAX_PRIVATE_GENESIS = 64 * 1024 * 1024
_ROUTE = "/v1/private/transaction-counters"


def _native() -> Any:
    from iroha_native import load_crypto_extension

    return load_crypto_extension()


def _bytes(value: Any, maximum: int, name: str) -> bytes:
    if type(value) is not bytes:
        raise TypeError(f"{name} must be exact immutable bytes")
    if not value or len(value) > maximum:
        raise ValueError(f"{name} exceeds its native frame bound")
    return value


def _json(value: Any, maximum: int, name: str) -> str:
    if type(value) is not str:
        raise TypeError(f"{name} must be exact native-model JSON text")
    if not value or len(value.encode("utf-8")) > maximum:
        raise ValueError(f"{name} exceeds its native JSON bound")
    return value


def _freeze(value: Any) -> Any:
    if type(value) is dict:
        return MappingProxyType({key: _freeze(item) for key, item in value.items()})
    if type(value) is list:
        return tuple(_freeze(item) for item in value)
    return value


def canonical_private_genesis_authority_v1(original_signed_genesis: bytes) -> str:
    """Read the canonical native authority of an independently authenticated private genesis.

    First obtain these exact bytes from the installed AuthenticatedFinalityRoot
    snapshot admission. Native code checks current canonical framing, original
    block and transaction signatures, one exact authority and private dataspace
    scope. Valid offered bytes alone do not select or install the trusted root.
    """
    _bytes(original_signed_genesis, _MAX_PRIVATE_GENESIS, "original_signed_genesis")
    return _native().canonical_private_genesis_authority_v1(original_signed_genesis)


def encode_private_transaction_counters_policy_v1(policy_json: str) -> tuple[bytes, str]:
    """Construct the current native policy and its native commitment.

    Author its exact executable bindings and nominal contract errors from the
    release-owned plan before submitting transactions. The native model admits
    every mandatory policy field; a receipt never supplies this trust input.
    """
    return _native().encode_private_transaction_counters_policy_v1(
        _json(policy_json, _MAX_POLICY, "policy_json")
    )


def encode_private_transaction_counters_manifest_v1(manifest_json: str) -> tuple[bytes, str]:
    """Construct the sole canonical native run manifest and its native commitment."""
    return _native().encode_private_transaction_counters_manifest_v1(
        _json(manifest_json, _MAX_MANIFEST, "manifest_json")
    )


def commitments_private_transaction_counters_v1(
    original_policy: bytes, original_manifest: bytes, expected_run_binding_json: str,
) -> tuple[str, str]:
    """Admit both current original frames against an independently selected run.

    Select the exact session/run binding before reading fixed-authority metadata
    or receiving a counter response. Native code requires that binding and its
    purpose to equal the policy, admits the manifest against that policy, then
    returns their native commitments. The original frames remain unchanged.
    """
    _bytes(original_policy, _MAX_POLICY, "original_policy")
    _bytes(original_manifest, _MAX_MANIFEST, "original_manifest")
    _json(expected_run_binding_json, _MAX_POLICY, "expected_run_binding_json")
    return _native().commitments_private_transaction_counters_v1(
        original_policy, original_manifest, expected_run_binding_json,
    )


def authored_private_transaction_counters_executable_v1(builder: Any) -> tuple[bytes, str]:
    """Capture the actual native builder's executable before signing clears it.

    Retain its original frame and typed native hash in owner custody before
    submission. Never construct this policy input from a receipt.
    """
    return _native().authored_private_transaction_counters_executable_v1(builder)


def authored_private_transaction_counters_prepared_executable_v1(executable_json: str) -> tuple[bytes, str]:
    """Encode an independently authored prepared Executable in the current native model.

    Supply the exact instruction/contract arguments before submitting the plan.
    This is a native construction codec, never a receipt decoder or hash fallback.
    """
    return _native().authored_private_transaction_counters_prepared_executable_v1(
        _json(executable_json, _MAX_AUTHORED_EXECUTABLE, "authored executable JSON"),
    )


def private_transaction_counters_run_commitment_v1(run_binding_json: str) -> str:
    """Commit an independently selected current run binding in the native model."""
    return _native().private_transaction_counters_run_commitment_v1(
        _json(run_binding_json, _MAX_POLICY, "run_binding_json"),
    )


def authored_private_transaction_counters_signed_executable_v1(
    original_signed_transaction_versioned: bytes, expected_network_id: Any, expected_authority: str,
) -> tuple[bytes, str]:
    """Capture an independently authored, signed current transaction BEFORE its first submit.

    Native code admits its exact canonical original, signature, selected network
    and authority. A committed transaction or receipt is not an input to this API.
    """
    _bytes(original_signed_transaction_versioned, _MAX_AUTHORED_EXECUTABLE, "authored signed transaction")
    return _native().authored_private_transaction_counters_signed_executable_v1(
        original_signed_transaction_versioned, expected_network_id, expected_authority,
    )


def private_transaction_counters_plan_commitment_v1(
    semantics_json: str, executable_bindings_json: str,
) -> str:
    """Commit the complete pre-submit semantic and executable selection in the native model."""
    return _native().private_transaction_counters_plan_commitment_v1(
        _json(semantics_json, _MAX_MANIFEST, "semantics_json"),
        _json(executable_bindings_json, _MAX_POLICY, "executable_bindings_json"),
    )


@dataclass(frozen=True)
class PreparedPrivateTransactionCountersV1:
    """Original signed request with independently selected immutable verifier inputs."""

    original_request: bytes = field(repr=False)
    original_policy: bytes = field(repr=False)
    native_finality_proof_chain_json: str = field(repr=False)
    expected_json: str = field(repr=False)
    expected_chain: str
    trusted_checkpoint: bytes = field(repr=False)

    def __post_init__(self) -> None:
        _bytes(self.original_request, MAX_PRIVATE_COUNTER_FRAME_BYTES_V1, "original_request")
        _bytes(self.original_policy, _MAX_POLICY, "original_policy")
        _json(self.native_finality_proof_chain_json, _MAX_CHAIN, "native_finality_proof_chain_json")
        _json(self.expected_json, MAX_PRIVATE_COUNTER_FRAME_BYTES_V1, "expected_json")
        _json(self.expected_chain, 1024, "expected_chain")
        _bytes(self.trusted_checkpoint, _MAX_CHECKPOINT, "trusted_checkpoint")


def prepare_private_transaction_counters_v1(
    *, private_key: bytes, time_to_live_ms: int, original_policy: bytes,
    native_finality_proof_chain_json: str, expected_json: str,
    expected_chain: str, trusted_checkpoint: bytes,
) -> PreparedPrivateTransactionCountersV1:
    """Authenticate the selected original prefix and sign with the current Ed25519 reader key.

    ``expected_json`` independently binds network, immutable private scope,
    authority, purpose, policy, sealed manifest and fresh nonce. The request's
    cut is constructed from the verified original prefix's tip. Native code supplies
    the signing domain and creation clock, rejects multisig, and signs the current model.
    """
    if type(time_to_live_ms) is not int or not 0 < time_to_live_ms <= 60000:
        raise ValueError("time_to_live_ms must be an exact integer in 1..60000")
    _bytes(private_key, 32, "private_key")
    if len(private_key) != 32:
        raise ValueError("private_key must contain exactly 32 bytes")
    _bytes(original_policy, _MAX_POLICY, "original_policy")
    _json(native_finality_proof_chain_json, _MAX_CHAIN, "native_finality_proof_chain_json")
    _json(expected_json, MAX_PRIVATE_COUNTER_FRAME_BYTES_V1, "expected_json")
    _json(expected_chain, 1024, "expected_chain")
    _bytes(trusted_checkpoint, _MAX_CHECKPOINT, "trusted_checkpoint")
    original = _native().build_private_transaction_counters_request_v1(
        private_key, time_to_live_ms, original_policy,
        native_finality_proof_chain_json, expected_json,
        expected_chain, trusted_checkpoint,
    )
    return PreparedPrivateTransactionCountersV1(
        _bytes(original, MAX_PRIVATE_COUNTER_FRAME_BYTES_V1, "native original_request"),
        original_policy, native_finality_proof_chain_json, expected_json,
        expected_chain, trusted_checkpoint,
    )


@dataclass(frozen=True)
class VerifiedPrivateTransactionCountersV1:
    """Native admitted categorical claim and its separately retained finality checkpoint."""

    claim: Mapping[str, Any] = field(repr=False)
    promoted_checkpoint: bytes = field(repr=False)
    original_certificate: bytes = field(repr=False)
    _groups: tuple[Mapping[str, Any], ...] = field(repr=False)

    @property
    def groups(self) -> tuple[Mapping[str, Any], ...]:
        """Only native categorical groups; private evidence bindings stay in claim custody."""
        return self._groups


def _read_original(client: Any, original: bytes) -> bytes:
    if type(client._api_token) is not str or not client._api_token:
        raise ValueError("private counters require an explicit private-root API token")
    response = client._request(
        "POST", _ROUTE, data=original,
        headers={"Content-Type": "application/x-norito", "Accept": "application/x-norito",
                 "Accept-Encoding": "identity"},
        stream=True, allow_retry=False, allow_redirects=False,
        timeout=client._timeout,
    )
    try:
        if response.history or response.url != f"{client._base_url}{_ROUTE}":
            raise ValueError("private counters response changed the selected route")
        if response.status_code != 200:
            raise requests.HTTPError("private counters request refused", response=response)
        if response.headers.get("Content-Type", "") != "application/x-norito":
            raise ValueError("private counters response requires exact Norito MIME")
        if response.headers.get("Content-Encoding", "identity") != "identity":
            raise ValueError("private counters response requires identity encoding")
        declared = response.headers.get("Content-Length")
        if declared is not None:
            if not declared.isascii() or not declared.isdecimal() or str(int(declared)) != declared:
                raise ValueError("private counters response length is noncanonical")
            if not 0 < int(declared) <= MAX_PRIVATE_COUNTER_FRAME_BYTES_V1:
                raise ValueError("private counters response exceeds its frame bound")
        chunks: list[bytes] = []
        length = 0
        for chunk in response.iter_content(chunk_size=8192):
            if type(chunk) is not bytes:
                raise TypeError("private counters response chunks must be bytes")
            length += len(chunk)
            if length > MAX_PRIVATE_COUNTER_FRAME_BYTES_V1:
                raise ValueError("private counters response exceeds its frame bound")
            chunks.append(chunk)
        original_response = b"".join(chunks)
        if not original_response or (declared is not None and int(declared) != length):
            raise ValueError("private counters response length differs from its original frame")
        return original_response
    finally:
        response.close()


class ToriiClientPrivateTransactionCountersMixin:
    """One-shot selected-peer collection without history rows or host counter logic."""

    def get_verified_private_transaction_counters_v1(
        self, *, request: PreparedPrivateTransactionCountersV1, other_peers: Sequence[Any] = (),
    ) -> VerifiedPrivateTransactionCountersV1:
        """Collect identical original claims and admit their native committee quorum.

        Select peer clients and independent trust inputs before any request.
        Every peer receives the same original signed frame exactly once. A
        refusal aborts this call; the SDK never retries a replay nonce or changes
        the selected cut. Atomically persist the returned promoted checkpoint.
        """
        if type(request) is not PreparedPrivateTransactionCountersV1:
            raise TypeError("private counters require an exact prepared current request")
        if not isinstance(other_peers, (tuple, list)) or len(other_peers) >= 31:
            raise ValueError("select a finite sequence of at most 30 other peers")
        peers = (self, *other_peers)
        roots = tuple(peer._base_url for peer in peers)
        if len(set(roots)) != len(roots):
            raise ValueError("private counters require distinct selected peer roots")
        # Validate every peer before the first one-shot signed request is sent.
        if any(type(peer._api_token) is not str or not peer._api_token for peer in peers):
            raise ValueError("every private counter peer requires an explicit API token")
        if any(type(peer._timeout) not in (int, float) or not math.isfinite(peer._timeout)
               or not 0 < peer._timeout <= 60 for peer in peers):
            raise ValueError("every private counter peer requires a finite timeout in (0,60] seconds")
        native = _native()
        collect = native.collect_private_transaction_counters_v1
        verify = native.verify_private_transaction_counters_v1
        if not callable(collect) or not callable(verify):
            raise RuntimeError("current native counter collector and verifier are required")
        # All selected nodes receive the same certified cut concurrently. Waiting
        # for every bounded request preserves original peer order and never retries.
        with ThreadPoolExecutor(max_workers=len(peers)) as workers:
            pending = tuple(workers.submit(_read_original, peer, request.original_request) for peer in peers)
            originals = [future.result() for future in pending]
        certificate = _bytes(collect(originals),
                             MAX_PRIVATE_COUNTER_FRAME_BYTES_V1, "native certificate")
        projection_json, promoted = verify(
            request.original_request, certificate, request.original_policy,
            request.native_finality_proof_chain_json, request.expected_json,
            request.expected_chain, request.trusted_checkpoint,
        )
        # JSON is a projection emitted only AFTER native verification. It is never
        # an incoming history page, a count source, or an authentication substitute.
        projection = json.loads(_json(projection_json, _MAX_PROJECTION, "verified projection"))
        if type(projection) is not dict or set(projection) != {"claim", "groups"}:
            raise RuntimeError("native verified counter projection has an invalid current shape")
        claim, groups = projection["claim"], projection["groups"]
        if type(claim) is not dict or type(groups) is not list:
            raise RuntimeError("native verified counter projection must contain current claim and groups")
        return VerifiedPrivateTransactionCountersV1(
            _freeze(claim), _bytes(promoted, _MAX_CHECKPOINT, "promoted_checkpoint"), certificate, _freeze(groups),
        )


__all__ = [
    "canonical_private_genesis_authority_v1",
    "PreparedPrivateTransactionCountersV1", "VerifiedPrivateTransactionCountersV1",
    "prepare_private_transaction_counters_v1", "encode_private_transaction_counters_policy_v1",
    "encode_private_transaction_counters_manifest_v1",
    "commitments_private_transaction_counters_v1",
    "authored_private_transaction_counters_executable_v1",
    "authored_private_transaction_counters_prepared_executable_v1",
    "authored_private_transaction_counters_signed_executable_v1",
    "private_transaction_counters_plan_commitment_v1",
    "private_transaction_counters_run_commitment_v1",
]
