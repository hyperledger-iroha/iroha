"""Exact-network governance ballot dispatch for the high-level Torii client."""

from __future__ import annotations

import hmac
from typing import TYPE_CHECKING, Any, Callable, Dict, Mapping, Optional, Tuple

if TYPE_CHECKING:
    from iroha_torii_client.client import ToriiCanonicalRequestAuth

    from .crypto import NetworkId
else:
    NetworkId = Any
    ToriiCanonicalRequestAuth = Any


def bind_governance_ballot_network_id(
    normalize_network_id: Callable[[Any, str], Any],
) -> Callable[..., Any]:
    """Bind the SDK's nominal ``NetworkId`` validator to ballot records."""

    def normalize(record: Dict[str, Any], *, context: str) -> Any:
        network_id = normalize_network_id(
            record.get("network_id"),
            f"{context}.network_id",
        )
        record["network_id"] = network_id.literal
        return network_id

    return normalize


def create_torii_client_governance_ballot_mixin(
    *,
    network_id_type: type,
    canonical_auth_type: type,
    normalize_network_id: Callable[[Any, str], Any],
    require_exact_non_empty_string: Callable[[Any, str], str],
    normalize_canonical_auth: Callable[..., Any],
    normalize_plain_ballot: Callable[..., Dict[str, Any]],
    normalize_zk_ballot_v1: Callable[..., Dict[str, Any]],
    normalize_zk_ballot_proof_v1: Callable[..., Dict[str, Any]],
) -> Any:
    """Bind client-local validation hooks to exact-network ballot methods."""

    globals()["NetworkId"] = network_id_type
    globals()["ToriiCanonicalRequestAuth"] = canonical_auth_type
    _normalize_network_id = normalize_network_id
    _require_exact_non_empty_string = require_exact_non_empty_string
    _normalize_canonical_auth = normalize_canonical_auth
    _normalize_plain_ballot = normalize_plain_ballot
    _normalize_zk_ballot_v1 = normalize_zk_ballot_v1
    _normalize_zk_ballot_proof_v1 = normalize_zk_ballot_proof_v1

    class ToriiClientGovernanceBallotMixin:
        _require_local_signing_context: Callable[..., Any]
        _normalize_canonical_account_id: Callable[..., str]
        _chain_discriminant: int
        _parse_governance_ballot_draft: Callable[..., Any]
        _post_network_governance_ballot_json: Callable[..., Any]

        def _governance_ballot_identity(
            self,
            payload: Mapping[str, Any],
            canonical_auth: ToriiCanonicalRequestAuth,
            *,
            context: str,
        ) -> Tuple["NetworkId", str, ToriiCanonicalRequestAuth]:
            if not isinstance(payload, Mapping):
                raise TypeError(f"{context} must be a JSON object")
            for retired in ("chain_id", "chainId", "genesis_hash", "genesisHash"):
                if retired in payload:
                    raise ValueError(f"{context}.{retired} is retired; provide typed network_id")
            signing_context = self._require_local_signing_context(context)
            network_id = _normalize_network_id(
                payload.get("network_id"),
                f"{context}.network_id",
            )
            if not hmac.compare_digest(
                bytes(network_id.to_bytes()),
                bytes(signing_context.network_id.to_bytes()),
            ):
                raise ValueError(
                    f"{context}.network_id must equal ToriiClient local_signing_context"
                )
            authority = _require_exact_non_empty_string(
                payload.get("authority"),
                f"{context}.authority",
            )
            if (
                "@" in authority
                or self._normalize_canonical_account_id(
                    authority,
                    f"{context}.authority",
                )
                != authority
            ):
                raise ValueError(f"{context}.authority must be an exact canonical I105 account id")
            canonical_auth = _normalize_canonical_auth(
                canonical_auth,
                f"{context}.canonical_auth",
                expected_discriminant=self._chain_discriminant,
            )
            if "@" in canonical_auth.account_id or canonical_auth.account_id != authority:
                raise ValueError(
                    f"{context}.canonical_auth.account_id must equal payload authority"
                )
            return network_id, authority, canonical_auth

        def _post_governance_ballot(
            self,
            path: str,
            payload: Mapping[str, Any],
            canonical_auth: ToriiCanonicalRequestAuth,
            *,
            normalizer: Callable[..., Dict[str, Any]],
            context: str,
        ) -> Optional[Any]:
            network_id, authority, canonical_auth = self._governance_ballot_identity(
                payload,
                canonical_auth,
                context=context,
            )
            normalized = normalizer(payload, context=context)
            response = self._post_network_governance_ballot_json(
                path,
                normalized,
                network_id=network_id.literal,
                authority=authority,
                canonical_auth=canonical_auth,
                context=context,
            )
            return self._parse_governance_ballot_draft(
                response,
                context=f"{context} response",
            )

        def governance_submit_plain_ballot(
            self,
            payload: Mapping[str, Any],
            *,
            canonical_auth: ToriiCanonicalRequestAuth,
        ) -> Optional[Any]:
            """Draft an exact-network plain ballot with a one-shot signed request."""

            return self._post_governance_ballot(
                "/v1/gov/ballots/plain",
                payload,
                canonical_auth,
                normalizer=_normalize_plain_ballot,
                context="governance plain ballot",
            )

        def governance_submit_zk_ballot_v1(
            self,
            payload: Mapping[str, Any],
            *,
            canonical_auth: ToriiCanonicalRequestAuth,
        ) -> Optional[Any]:
            """Draft an exact-network flat ZK ballot."""

            return self._post_governance_ballot(
                "/v1/gov/ballots/zk-v1",
                payload,
                canonical_auth,
                normalizer=_normalize_zk_ballot_v1,
                context="governance zk ballot v1",
            )

        def governance_submit_zk_ballot_proof_v1(
            self,
            payload: Mapping[str, Any],
            *,
            canonical_auth: ToriiCanonicalRequestAuth,
        ) -> Optional[Any]:
            """Draft an exact-network typed ballot proof."""

            return self._post_governance_ballot(
                "/v1/gov/ballots/zk-v1/ballot-proof",
                payload,
                canonical_auth,
                normalizer=_normalize_zk_ballot_proof_v1,
                context="governance zk ballot proof v1",
            )

    return ToriiClientGovernanceBallotMixin
