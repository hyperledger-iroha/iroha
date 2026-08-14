"""Exact-account Space Directory draft methods for the low-level Torii client."""

from __future__ import annotations

import hmac
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, Dict, Mapping, Optional

if TYPE_CHECKING:
    from .client import AppApiTransactionDraft, ToriiCanonicalRequestAuth
else:
    AppApiTransactionDraft = Any
    ToriiCanonicalRequestAuth = Any


@dataclass(frozen=True)
class ToriiLocalSigningContext:
    """Immutable exact-network context for locally authenticated requests."""

    network_id: str


def create_space_directory_client_mixin(
    *,
    canonical_auth_type: type,
    local_signing_context_type: type,
    normalize_network_id: Any,
    transaction_draft_type: type,
) -> type:
    """Bind client-local authentication and draft models to Space Directory methods."""

    globals()["ToriiCanonicalRequestAuth"] = canonical_auth_type
    globals()["AppApiTransactionDraft"] = transaction_draft_type
    _local_signing_context_type = local_signing_context_type
    _normalize_network_id = normalize_network_id
    _transaction_draft_type = transaction_draft_type

    class SpaceDirectoryClientMixin:
        def _require_space_directory_signing_context(
            self,
            canonical_auth: ToriiCanonicalRequestAuth,
            context: str,
        ) -> None:
            signing_context = self._local_signing_context
            if not isinstance(signing_context, _local_signing_context_type):
                raise ValueError(f"{context} requires an immutable local_signing_context")
            local_network = _normalize_network_id(
                signing_context.network_id,
                f"{context}.local_signing_context.network_id",
            )
            auth_network = _normalize_network_id(
                canonical_auth.network_id,
                f"{context}.canonical_auth.network_id",
            )
            if not hmac.compare_digest(local_network, auth_network):
                raise ValueError(
                    f"{context}.canonical_auth.network_id must match the immutable "
                    "local_signing_context network"
                )

        def publish_space_directory_manifest(
            self,
            *,
            authority: str,
            manifest: Mapping[str, Any],
            reason: Optional[str] = None,
            canonical_auth: ToriiCanonicalRequestAuth,
        ) -> AppApiTransactionDraft:
            """Prepare an account-authenticated manifest-publication draft."""

            canonical_authority = self._require_exact_i105_account_id(
                authority,
                "publish_space_directory_manifest.authority",
            )
            canonical_auth = self._require_canonical_auth(
                canonical_auth,
                "publish_space_directory_manifest",
            )
            self._require_space_directory_signing_context(
                canonical_auth,
                "publish_space_directory_manifest",
            )
            if canonical_auth.account_id != canonical_authority:
                raise ValueError(
                    "publish_space_directory_manifest.canonical_auth.account_id "
                    "must equal the exact payload authority"
                )
            payload: Dict[str, Any] = {
                "authority": canonical_authority,
                "manifest": self._clone_json_payload(
                    manifest,
                    context="publish_space_directory_manifest.manifest",
                ),
            }
            if reason is not None:
                payload["reason"] = self._require_string(
                    reason,
                    "publish_space_directory_manifest.reason",
                )
            ack = self._account_request_json(
                "POST",
                "/v1/space-directory/manifests",
                canonical_auth=canonical_auth,
                json_body=payload,
                expected_status=(200,),
                context="space directory manifest publish response",
            )
            if ack is None:
                raise RuntimeError("space directory manifest publish endpoint returned no payload")
            return _transaction_draft_type.from_payload(
                self._ensure_mapping(ack, "space directory manifest publish response"),
                context="space directory manifest publish response",
            )

        def revoke_space_directory_manifest(
            self,
            *,
            authority: str,
            uaid: str,
            dataspace: int,
            revoked_epoch: int,
            reason: Optional[str] = None,
            canonical_auth: ToriiCanonicalRequestAuth,
        ) -> AppApiTransactionDraft:
            """Prepare an account-authenticated manifest-revocation draft."""

            canonical_authority = self._require_exact_i105_account_id(
                authority,
                "revoke_space_directory_manifest.authority",
            )
            canonical_auth = self._require_canonical_auth(
                canonical_auth,
                "revoke_space_directory_manifest",
            )
            self._require_space_directory_signing_context(
                canonical_auth,
                "revoke_space_directory_manifest",
            )
            if canonical_auth.account_id != canonical_authority:
                raise ValueError(
                    "revoke_space_directory_manifest.canonical_auth.account_id "
                    "must equal the exact payload authority"
                )
            payload: Dict[str, Any] = {
                "authority": canonical_authority,
                "uaid": self._normalize_uaid_literal(
                    uaid,
                    context="revoke_space_directory_manifest.uaid",
                ),
                "dataspace": self._coerce_unsigned(
                    dataspace,
                    "revoke_space_directory_manifest.dataspace",
                ),
                "revoked_epoch": self._coerce_unsigned(
                    revoked_epoch,
                    "revoke_space_directory_manifest.revoked_epoch",
                ),
            }
            if reason is not None:
                payload["reason"] = self._require_string(
                    reason,
                    "revoke_space_directory_manifest.reason",
                )
            ack = self._account_request_json(
                "POST",
                "/v1/space-directory/manifests/revoke",
                canonical_auth=canonical_auth,
                json_body=payload,
                expected_status=(200,),
                context="space directory manifest revoke response",
            )
            if ack is None:
                raise RuntimeError("space directory manifest revoke endpoint returned no payload")
            return _transaction_draft_type.from_payload(
                self._ensure_mapping(ack, "space directory manifest revoke response"),
                context="space directory manifest revoke response",
            )

    return SpaceDirectoryClientMixin
