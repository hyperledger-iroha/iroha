"""Exact-network Space Directory draft methods for the high-level Torii client."""

from __future__ import annotations

import hmac
from typing import TYPE_CHECKING, Any, Callable, Mapping

from .crypto import NetworkId

if TYPE_CHECKING:
    from iroha_torii_client.client import ToriiCanonicalRequestAuth
else:
    ToriiCanonicalRequestAuth = Any


def create_torii_client_space_directory_mixin(
    *,
    canonical_auth_type: type,
    normalize_publish_request: Callable[[Mapping[str, Any]], dict[str, Any]],
    normalize_revoke_request: Callable[[Mapping[str, Any]], dict[str, Any]],
    normalize_transaction_draft: Callable[[Any, str], Any],
) -> type:
    """Bind client-local normalizers to authenticated Space Directory drafts."""

    globals()["ToriiCanonicalRequestAuth"] = canonical_auth_type
    _normalize_publish_request = normalize_publish_request
    _normalize_revoke_request = normalize_revoke_request
    _normalize_transaction_draft = normalize_transaction_draft

    class ToriiClientSpaceDirectoryMixin:
        def _require_authenticated_draft_context(
            self,
            authority: Any,
            context: str,
        ) -> ToriiCanonicalRequestAuth:
            """Bind one draft request to the immutable local network and authority."""

            signing_context = self._require_local_signing_context(context)
            canonical_authority = self._require_exact_i105_account_id(
                authority,
                f"{context}.authority",
            )
            canonical_auth = self._require_canonical_auth(
                self._canonical_request_auth,
                context,
            )
            auth_authority = self._require_exact_i105_account_id(
                canonical_auth.account_id,
                f"{context}.canonical_auth.account_id",
            )
            if auth_authority != canonical_authority:
                raise ValueError(
                    f"{context}.canonical_auth.account_id must equal the exact payload authority"
                )
            auth_network = NetworkId.from_bytes(
                bytes.fromhex(canonical_auth.network_id[5:69])
            )
            if not hmac.compare_digest(
                bytes(auth_network.to_bytes()),
                bytes(signing_context.network_id.to_bytes()),
            ):
                raise ValueError(
                    f"{context}.canonical_auth.network_id must match the immutable "
                    "local_signing_context network"
                )
            return canonical_auth

        def publish_space_directory_manifest(
            self,
            request: Mapping[str, Any],
        ) -> Any:
            """Prepare an exact-network authenticated publication draft."""

            payload = _normalize_publish_request(request)
            canonical_auth = self._require_authenticated_draft_context(
                payload["authority"],
                "publish_space_directory_manifest",
            )
            body = self._account_request_json(
                "POST",
                "/v1/space-directory/manifests",
                canonical_auth=canonical_auth,
                json_body=payload,
                expected_status=(200,),
                context="Space Directory manifest publish response",
            )
            if body is None:
                raise RuntimeError("Space Directory manifest publish response was empty")
            return _normalize_transaction_draft(
                body,
                "Space Directory manifest publish response",
            )

        def revoke_space_directory_manifest(
            self,
            request: Mapping[str, Any],
        ) -> Any:
            """Prepare an exact-network authenticated revocation draft."""

            payload = _normalize_revoke_request(request)
            canonical_auth = self._require_authenticated_draft_context(
                payload["authority"],
                "revoke_space_directory_manifest",
            )
            body = self._account_request_json(
                "POST",
                "/v1/space-directory/manifests/revoke",
                canonical_auth=canonical_auth,
                json_body=payload,
                expected_status=(200,),
                context="Space Directory manifest revoke response",
            )
            if body is None:
                raise RuntimeError("Space Directory manifest revoke response was empty")
            return _normalize_transaction_draft(
                body,
                "Space Directory manifest revoke response",
            )

    return ToriiClientSpaceDirectoryMixin
