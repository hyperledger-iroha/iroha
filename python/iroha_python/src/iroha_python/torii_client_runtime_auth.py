"""Authenticated node/runtime helpers extracted from the main Torii client."""

from __future__ import annotations

from typing import Any, Callable


def _validate_client_data_model(
    client: Any,
    *,
    canonical_auth: Any,
    expected_version: int,
    mismatch_error_type: type[RuntimeError],
) -> None:
    """Validate the node data-model version once for one client instance."""

    if client._data_model_validation == "matched":
        return
    if client._data_model_validation == "mismatched":
        raise mismatch_error_type(expected_version, client._data_model_actual)

    auth = client._require_canonical_auth(
        canonical_auth,
        "transaction data-model validation",
    )
    try:
        capabilities = client.get_node_capabilities_typed(canonical_auth=auth)
    except RuntimeError as error:
        if "data_model_version" in str(error):
            client._data_model_validation = "mismatched"
            client._data_model_actual = None
            raise mismatch_error_type(expected_version, None) from error
        raise
    actual = capabilities.data_model_version
    if actual != expected_version:
        client._data_model_validation = "mismatched"
        client._data_model_actual = actual
        raise mismatch_error_type(expected_version, actual)
    client._data_model_validation = "matched"
    client._data_model_actual = actual


def create_torii_client_runtime_auth_mixin(
    *,
    node_capabilities_type: Any,
    node_admin_snapshot_type: Any,
    runtime_metrics_type: Any,
    runtime_abi_active_type: Any,
) -> Any:
    """Create protected runtime helpers without coupling to client model imports."""

    class ToriiClientRuntimeAuthMixin:
        get_configuration_typed: Callable[..., Any]
        list_peers_typed: Callable[..., Any]
        get_time_status_typed: Callable[..., Any]
        get_time_now_typed: Callable[..., Any]
        list_telemetry_peers_info_typed: Callable[..., Any]
        _account_request_json: Callable[..., Any]

        def capture_node_admin_snapshot(
            self, *, canonical_auth: Any, include_peer_telemetry: bool = True
        ) -> Any:
            configuration = self.get_configuration_typed()
            peers = self.list_peers_typed()
            time_status = self.get_time_status_typed()
            time_now = self.get_time_now_typed()
            node_capabilities = self.get_node_capabilities_typed(
                canonical_auth=canonical_auth
            )
            telemetry_peers = (
                self.list_telemetry_peers_info_typed()
                if include_peer_telemetry
                else None
            )
            return node_admin_snapshot_type(
                configuration=configuration,
                peers=peers,
                time_now=time_now,
                time_status=time_status,
                node_capabilities=node_capabilities,
                telemetry_peers=telemetry_peers,
            )

        def get_node_capabilities(self, *, canonical_auth: Any) -> Any:
            return self._account_request_json(
                "GET", "/v1/node/capabilities", canonical_auth=canonical_auth,
                context="node capabilities", expected_status=(200,),
            )

        def get_node_capabilities_typed(self, *, canonical_auth: Any) -> Any:
            payload = self.get_node_capabilities(canonical_auth=canonical_auth)
            if payload is None:
                raise RuntimeError("node capabilities endpoint returned no payload")
            return node_capabilities_type.from_payload(payload)

        def get_runtime_metrics(self, *, canonical_auth: Any) -> Any:
            return self._account_request_json(
                "GET", "/v1/runtime/metrics", canonical_auth=canonical_auth,
                context="runtime metrics", expected_status=(200,),
            )

        def get_runtime_metrics_typed(self, *, canonical_auth: Any) -> Any:
            payload = self.get_runtime_metrics(canonical_auth=canonical_auth)
            if payload is None:
                raise RuntimeError("runtime metrics endpoint returned no payload")
            return runtime_metrics_type.from_payload(payload)

        def get_runtime_abi_active(self, *, canonical_auth: Any) -> Any:
            return self._account_request_json(
                "GET", "/v1/runtime/abi/active", canonical_auth=canonical_auth,
                context="runtime ABI active", expected_status=(200,),
            )

        def get_runtime_abi_active_typed(self, *, canonical_auth: Any) -> Any:
            payload = self.get_runtime_abi_active(canonical_auth=canonical_auth)
            if payload is None:
                raise RuntimeError("runtime ABI active endpoint returned no payload")
            return runtime_abi_active_type.from_payload(payload)

    return ToriiClientRuntimeAuthMixin
