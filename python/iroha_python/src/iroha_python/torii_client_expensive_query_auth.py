"""Exact-network authentication for expensive application query POSTs."""

from __future__ import annotations

import hmac
from typing import Any

from .crypto import NetworkId


class ToriiClientExpensiveQueryAuthMixin:
    """Dispatch expensive query envelopes once with canonical account auth."""

    def _expensive_query_json(
        self,
        path: str,
        body: dict[str, Any],
        *,
        context: str,
    ) -> Any:
        signing_context = self._require_local_signing_context(context)
        canonical_auth = self._require_canonical_auth(self._canonical_request_auth, context)
        self._require_exact_i105_account_id(
            canonical_auth.account_id,
            f"{context}.canonical_auth.account_id",
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
        return self._account_request_json(
            "POST",
            path,
            canonical_auth=canonical_auth,
            json_body=body,
            expected_status=(200,),
            context=context,
        )
