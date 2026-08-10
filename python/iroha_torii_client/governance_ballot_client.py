"""Exact-network governance ballot methods for the low-level Torii client."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Callable, Dict, Iterable, Mapping, Optional, Tuple

if TYPE_CHECKING:
    from .client import BallotSubmitResult, ToriiCanonicalRequestAuth
else:
    BallotSubmitResult = Any
    ToriiCanonicalRequestAuth = Any


def create_governance_ballot_client_mixin(
    *,
    canonical_auth_type: type,
    ballot_submit_result_type: type,
    offline_hash_literal: Callable[[Any, str], str],
    canonical_quantity: Callable[[Any, str], str],
    build_canonical_request_headers: Callable[..., Dict[str, str]],
) -> type:
    """Bind client-local models and validators to ballot transport methods."""

    globals()["ToriiCanonicalRequestAuth"] = canonical_auth_type
    globals()["BallotSubmitResult"] = ballot_submit_result_type
    _canonical_auth_type = canonical_auth_type
    _ballot_submit_result_type = ballot_submit_result_type
    _offline_hash_literal = offline_hash_literal
    _canonical_quantity = canonical_quantity
    _build_canonical_request_headers = build_canonical_request_headers

    class GovernanceBallotClientMixin:
        def submit_plain_ballot(
            self,
            *,
            authority: str,
            network_id: str,
            referendum_id: str,
            owner: str,
            amount: str,
            duration_blocks: int,
            direction: str,
            canonical_auth: ToriiCanonicalRequestAuth,
            public: Optional[Mapping[str, Any]] = None,
        ) -> BallotSubmitResult:
            """Draft a quadratic ballot bound to one exact ``NetworkId``."""

            network_id, authority, canonical_auth = self._network_governance_ballot_identity(
                network_id=network_id,
                authority=authority,
                canonical_auth=canonical_auth,
                context="plain ballot",
            )
            referendum_id = self._require_governance_selector_v1(
                referendum_id,
                context="plain ballot referendum_id",
            )
            if isinstance(duration_blocks, bool) or not isinstance(duration_blocks, int):
                raise TypeError("plain ballot duration_blocks must be an unsigned 64-bit integer")
            if duration_blocks < 0 or duration_blocks > (1 << 64) - 1:
                raise ValueError("plain ballot duration_blocks must fit unsigned 64-bit range")
            payload: Dict[str, Any] = {
                "authority": authority,
                "network_id": network_id,
                "referendum_id": referendum_id,
                "owner": owner,
                "amount": _canonical_quantity(amount, "plain ballot amount"),
                "duration_blocks": str(duration_blocks),
                "direction": direction,
            }
            if public is not None:
                payload["public"] = public
            body = self._post_network_governance_ballot_json(
                "/v1/gov/ballots/plain",
                payload,
                network_id=network_id,
                authority=authority,
                canonical_auth=canonical_auth,
                context="plain ballot",
            )
            return _ballot_submit_result_type(
                ok=bool(body.get("ok")),
                accepted=bool(body.get("accepted")),
                reason=body.get("reason"),
                tx_instructions=self._parse_tx_instructions(body.get("tx_instructions")),
            )

        def submit_zk_ballot_v1(
            self,
            *,
            authority: str,
            network_id: str,
            election_id: str,
            backend: str,
            envelope_b64: str,
            root_hint: Optional[str] = None,
            owner: Optional[str] = None,
            amount: Optional[str] = None,
            duration_blocks: Optional[int] = None,
            direction: Optional[str] = None,
            nullifier: Optional[str] = None,
            canonical_auth: ToriiCanonicalRequestAuth,
        ) -> BallotSubmitResult:
            """Draft a BallotProof payload bound to one exact ``NetworkId``.

            Optional hints mirror BallotProof fields: root_hint, owner, amount,
            duration_blocks, direction, and nullifier.
            """

            network_id, authority, canonical_auth = self._network_governance_ballot_identity(
                network_id=network_id,
                authority=authority,
                canonical_auth=canonical_auth,
                context="zk ballot v1",
            )
            election_id = self._require_governance_selector_v1(
                election_id,
                context="zk ballot v1 election_id",
            )
            payload: Dict[str, Any] = {
                "authority": authority,
                "network_id": network_id,
                "election_id": election_id,
                "backend": backend,
                "envelope_b64": envelope_b64,
            }
            self._ensure_governance_lock_hints_complete(
                owner,
                amount,
                duration_blocks,
                context="zk ballot v1",
            )
            self._ensure_governance_owner_canonical(owner, context="zk ballot v1")
            if root_hint is not None:
                payload["root_hint"] = root_hint
            if owner is not None:
                payload["owner"] = owner
            if amount is not None:
                payload["amount"] = _canonical_quantity(
                    amount,
                    "zk ballot v1 amount",
                )
            if duration_blocks is not None:
                payload["duration_blocks"] = duration_blocks
            if direction:
                payload["direction"] = direction
            if nullifier is not None:
                payload["nullifier"] = nullifier
            self._normalize_governance_public_hex_hint(
                payload,
                "root_hint",
                context="zk ballot v1",
            )
            self._normalize_governance_public_hex_hint(
                payload,
                "nullifier",
                context="zk ballot v1",
            )
            body = self._post_network_governance_ballot_json(
                "/v1/gov/ballots/zk-v1",
                payload,
                network_id=network_id,
                authority=authority,
                canonical_auth=canonical_auth,
                context="zk ballot v1",
            )
            return _ballot_submit_result_type(
                ok=bool(body.get("ok")),
                accepted=bool(body.get("accepted")),
                reason=body.get("reason"),
                tx_instructions=self._parse_tx_instructions(body.get("tx_instructions")),
            )

        def _network_governance_ballot_identity(
            self,
            *,
            network_id: str,
            authority: str,
            canonical_auth: ToriiCanonicalRequestAuth,
            context: str,
        ) -> Tuple[str, str, ToriiCanonicalRequestAuth]:
            network_literal = _offline_hash_literal(network_id, f"{context}.network_id")
            exact_authority = self._require_exact_i105_account_id(
                authority,
                f"{context}.authority",
            )
            if not isinstance(canonical_auth, _canonical_auth_type):
                raise TypeError(f"{context}.canonical_auth must be ToriiCanonicalRequestAuth")
            auth_network = _offline_hash_literal(
                canonical_auth.network_id,
                f"{context}.canonical_auth.network_id",
            )
            if auth_network != network_literal:
                raise ValueError(
                    f"{context}.canonical_auth.network_id must equal payload network_id"
                )
            principal = self._require_exact_i105_account_id(
                canonical_auth.account_id,
                f"{context}.canonical_auth.account_id",
            )
            if principal != exact_authority:
                raise ValueError(
                    f"{context}.canonical_auth.account_id must equal payload authority"
                )
            return network_literal, exact_authority, canonical_auth

        def _post_network_governance_ballot_json(
            self,
            path: str,
            payload: Mapping[str, Any],
            *,
            network_id: str,
            authority: str,
            canonical_auth: ToriiCanonicalRequestAuth,
            context: str,
            expected_status: Iterable[int] = (200,),
        ) -> Mapping[str, Any]:
            """Draft one exact-network ballot with one non-replayable request."""

            network_literal, principal, canonical_auth = self._network_governance_ballot_identity(
                network_id=network_id,
                authority=authority,
                canonical_auth=canonical_auth,
                context=context,
            )
            data = self._encode_json_body(payload)
            headers = {
                "Accept": "application/json",
                "Content-Type": "application/json",
            }
            headers.update(
                _build_canonical_request_headers(
                    network_id=canonical_auth.network_id,
                    account_id=principal,
                    signer=canonical_auth.signer,
                    method="POST",
                    path=path,
                    body=data,
                    timestamp_ms=canonical_auth.timestamp_ms,
                    nonce=canonical_auth.nonce,
                )
            )
            response = self._request(
                "POST",
                path,
                headers=headers,
                data=data,
                allow_retry=False,
                allow_redirects=False,
            )
            self._expect_status(response, expected_status)
            if response.status_code == 204:
                return {}
            return self._ensure_mapping(response.json(), context)

    return GovernanceBallotClientMixin
