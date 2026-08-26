"""One-shot canonical account transport shared by Torii SDK clients."""

from __future__ import annotations

from typing import Any, Callable, Iterable, Mapping, Optional, Sequence


class RuntimeGovernanceAuthMixin:
    """Authenticate protected runtime/governance requests before dispatch."""

    _canonical_request_headers: Callable[..., Any]
    _encode_json_body: Callable[..., bytes]
    _ensure_mapping: Callable[..., Mapping[str, Any]]
    _expect_status: Callable[..., None]
    _maybe_json: Callable[..., Any]
    _parse_node_capabilities: Callable[..., Any]
    _parse_runtime_abi_active: Callable[..., Any]
    _parse_runtime_metrics: Callable[..., Any]
    _request: Callable[..., Any]
    _require_canonical_auth: Callable[..., Any]
    _require_exact_i105_account_id: Callable[..., str]

    def _account_request(
        self,
        method: str,
        path: str,
        *,
        canonical_auth: Any,
        data: Optional[bytes] = None,
        headers: Optional[Mapping[str, str]] = None,
        context: str,
    ) -> Any:
        auth = self._require_canonical_auth(canonical_auth, context)
        self._require_exact_i105_account_id(
            auth.account_id, f"{context}.canonical_auth.account_id"
        )
        final_headers = self._canonical_request_headers(
            method,
            path,
            data or b"",
            canonical_auth=auth,
            headers=headers,
            has_body=data is not None,
        )
        return self._request(
            method,
            path,
            headers=final_headers,
            data=data,
            allow_retry=False,
            allow_redirects=False,
        )

    def _account_json_request(
        self,
        method: str,
        path: str,
        *,
        canonical_auth: Any,
        body_payload: Optional[Mapping[str, Any]] = None,
        context: str,
        expected_status: Iterable[int] = (200,),
    ) -> Mapping[str, Any]:
        data = self._encode_json_body(body_payload) if body_payload is not None else None
        response = self._account_request(
            method,
            path,
            canonical_auth=canonical_auth,
            data=data,
            context=context,
        )
        self._expect_status(response, expected_status)
        if response.status_code == 204:
            return {}
        return self._ensure_mapping(response.json(), context)

    def _account_request_json(
        self,
        method: str,
        path: str,
        *,
        canonical_auth: Any,
        json_body: Optional[Mapping[str, Any]] = None,
        expected_status: Sequence[int] = (200,),
        context: str,
    ) -> Optional[Any]:
        data = self._encode_json_body(json_body) if json_body is not None else None
        response = self._account_request(
            method,
            path,
            canonical_auth=canonical_auth,
            data=data,
            context=context,
        )
        self._expect_status(response, expected_status)
        return self._maybe_json(response)

    def get_node_capabilities(self, *, canonical_auth: Any) -> Any:
        payload = self._account_json_request(
            "GET", "/v1/node/capabilities", canonical_auth=canonical_auth,
            context="node capabilities",
        )
        return self._parse_node_capabilities(payload, context="node capabilities")

    def get_runtime_abi_active(self, *, canonical_auth: Any) -> Any:
        payload = self._account_json_request(
            "GET", "/v1/runtime/abi/active", canonical_auth=canonical_auth,
            context="runtime abi active response",
        )
        return self._parse_runtime_abi_active(payload, context="runtime abi active response")

    def get_runtime_metrics(self, *, canonical_auth: Any) -> Any:
        payload = self._account_json_request(
            "GET", "/v1/runtime/metrics", canonical_auth=canonical_auth,
            context="runtime metrics response",
        )
        return self._parse_runtime_metrics(payload, context="runtime metrics response")
