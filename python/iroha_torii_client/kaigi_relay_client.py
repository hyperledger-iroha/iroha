"""Operator-authenticated Kaigi relay reads for the low-level Torii client."""

from __future__ import annotations

from typing import Optional
from urllib.parse import quote


def create_kaigi_relay_client_mixin(
    *,
    relay_summary_list_type: type,
    relay_detail_type: type,
    relay_health_type: type,
) -> type:
    """Bind the public Kaigi relay result models to their client methods."""

    globals()["KaigiRelaySummaryList"] = relay_summary_list_type
    globals()["KaigiRelayDetail"] = relay_detail_type
    globals()["KaigiRelayHealthSnapshot"] = relay_health_type

    class KaigiRelayClientMixin:
        def list_kaigi_relays(self) -> KaigiRelaySummaryList:
            """Return relays using one exact-network operator-signed GET."""

            payload = self._get_kaigi_relay_json_object(
                "/v1/kaigi/relays",
                context="kaigi relay summary response",
            )
            if payload is None:
                raise RuntimeError("kaigi relay summary endpoint returned no payload")
            return self._parse_kaigi_relay_summary_list(
                payload,
                context="kaigi relay summary response",
            )

        def get_kaigi_relay(self, relay_id: str) -> Optional[KaigiRelayDetail]:
            """Return one relay diagnostic using an exact-network operator-signed GET."""

            canonical = self._normalize_canonical_account_id(relay_id, "relay_id")
            payload = self._get_kaigi_relay_json_object(
                f"/v1/kaigi/relays/{quote(canonical, safe='')}",
                context="kaigi relay detail response",
                allow_not_found=True,
            )
            if payload is None:
                return None
            return self._parse_kaigi_relay_detail(
                payload,
                context="kaigi relay detail response",
            )

        def get_kaigi_relays_health(self) -> KaigiRelayHealthSnapshot:
            """Return aggregate relay health using an exact-network operator-signed GET."""

            payload = self._get_kaigi_relay_json_object(
                "/v1/kaigi/relays/health",
                context="kaigi relay health snapshot",
            )
            if payload is None:
                raise RuntimeError("kaigi relay health endpoint returned no payload")
            return self._parse_kaigi_relay_health_snapshot(
                payload,
                context="kaigi relay health snapshot",
            )

    return KaigiRelayClientMixin
