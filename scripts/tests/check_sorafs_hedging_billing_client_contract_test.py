"""Source contract for bounded SoraFS hedging/billing client responses."""

from __future__ import annotations

import json
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
CLIENT = REPO_ROOT / "crates" / "iroha" / "src" / "client.rs"
TORII_API = (
    REPO_ROOT
    / "crates"
    / "iroha_torii"
    / "src"
    / "sorafs"
    / "hedging_billing_api.rs"
)
OPENAPI_DOCUMENTS = (
    REPO_ROOT / "artifacts" / "openapi" / "torii.json",
    REPO_ROOT
    / "artifacts"
    / "openapi"
    / "versions"
    / "current"
    / "torii.json",
)

JSON_LIMIT = "SORAFS_HEDGING_BILLING_JSON_RESPONSE_MAX_BYTES_V1"
STATEMENT_LIMIT = "SORAFS_BILLING_STATEMENT_RESPONSE_MAX_BYTES_V1"
STATEMENT_RESPONSE_MAX_BYTES = 22 * 1024 * 1024
ACKNOWLEDGEMENT_SCHEMA_NAME = (
    "iroha.torii.v1.sorafs.billing.acknowledgement_proof"
)
ACKNOWLEDGEMENT_SCHEMA_HASH = "fe75acabe03d788012f2e7c556319997"


def _source_slice(source: str, start: str, end: str) -> str:
    start_offset = source.index(start)
    return source[start_offset : source.index(end, start_offset + len(start))]


def _validate_response_bounds(client: str, torii: str) -> None:
    assert f"const {JSON_LIMIT}: usize = 1024 * 1024;" in client
    assert f"const {STATEMENT_LIMIT}: usize = 22 * 1024 * 1024;" in client
    assert "const MAX_JSON_RESPONSE_BYTES_V1: usize = 1024 * 1024;" in torii
    assert (
        "const MAX_PUBLISHED_STATEMENT_RESPONSE_BYTES_V1: usize =\n"
        "    SIGNED_GOVERNED_BILLING_STATEMENT_MAX_BYTES_V1 + 2 * 1024 * 1024;"
        in torii
    )

    for start, end in (
        (
            "        get_sorafs_billing_status => SorafsEndpoint::account_json_get(",
            "    /// Fetch one exact-checkpoint owner-isolated page",
        ),
        (
            "    pub fn get_sorafs_billing_statements(",
            "    /// Fetch one exact owned published billing statement",
        ),
        (
            "    pub fn post_sorafs_billing_statement_acknowledgement(",
            "        get_sorafs_billing_reconciliation => SorafsEndpoint::account_json_get(",
        ),
        (
            "        get_sorafs_billing_reconciliation => SorafsEndpoint::account_json_get(",
            "    /// Fetch one exact-checkpoint page of finalized `SoraFS` hedging exposure.",
        ),
        (
            "    fn get_sorafs_hedging_projection(",
            "    sorafs_filtered_get_methods!(",
        ),
    ):
        method = _source_slice(client, start, end)
        assert f".with_max_response_bytes({JSON_LIMIT})" in method, start

    statement_method = _source_slice(
        client,
        "    pub fn get_sorafs_billing_statement(",
        "    /// Submit one canonical owner acknowledgement",
    )
    assert f".with_max_response_bytes({STATEMENT_LIMIT})" in statement_method

    exposure_method = _source_slice(
        client,
        "    pub fn get_sorafs_hedging_exposure(",
        "    /// Fetch one exact-checkpoint page of governed `SoraFS` hedge intents.",
    )
    intents_method = _source_slice(
        client,
        "    pub fn get_sorafs_hedging_intents(",
        "    fn get_sorafs_hedging_projection(",
    )
    assert (
        'self.get_sorafs_hedging_projection("v1/sorafs/hedging/exposure", filter)'
        in exposure_method
    )
    assert (
        'self.get_sorafs_hedging_projection("v1/sorafs/hedging/intents", filter)'
        in intents_method
    )

    assert client.count(f".with_max_response_bytes({JSON_LIMIT})") == 5
    assert client.count(f".with_max_response_bytes({STATEMENT_LIMIT})") == 1
    endpoint = _source_slice(client, "    const fn with_max_response_bytes(", "macro_rules! sorafs_static_get_methods")
    assert "max_response_bytes: Some(max_response_bytes)" in endpoint
    methods = _source_slice(client, "macro_rules! sorafs_static_get_methods", "macro_rules! sorafs_typed_body_post_methods")
    assert "self.send_sorafs_endpoint($endpoint, Vec::new(), |_| {})" in methods
    request = _source_slice(client, "    fn send_sorafs_url(", "    fn send_sorafs_reserve_read(")
    assert "if let Some(max_response_bytes) = endpoint.max_response_bytes" in request
    assert "builder = builder.max_response_bytes(max_response_bytes);" in request

    for document_path in OPENAPI_DOCUMENTS:
        document = json.loads(document_path.read_text(encoding="utf-8"))
        statement_schema = document["paths"][
            "/v1/sorafs/billing/statements/{statement_id}"
        ]["get"]["responses"]["200"]["content"]["application/x-norito"]["schema"]
        assert statement_schema["maxLength"] == STATEMENT_RESPONSE_MAX_BYTES
        acknowledgement_schema = document["paths"][
            "/v1/sorafs/billing/statements/{statement_id}/acknowledgements"
        ]["post"]["requestBody"]["content"]["application/x-norito"]["schema"]
        assert (
            acknowledgement_schema["x-iroha-norito-schema"]
            == ACKNOWLEDGEMENT_SCHEMA_NAME
        )
        assert (
            acknowledgement_schema["x-iroha-norito-schema-hash"]
            == ACKNOWLEDGEMENT_SCHEMA_HASH
        )


def test_hedging_billing_client_response_bounds_match_server_contract() -> None:
    _validate_response_bounds(CLIENT.read_text(encoding="utf-8"), TORII_API.read_text(encoding="utf-8"))


@pytest.mark.parametrize("old,new", [
    (f".with_max_response_bytes({JSON_LIMIT})", ".with_max_response_bytes(0)"),
    ("max_response_bytes: Some(max_response_bytes)", "max_response_bytes: None"),
    ("builder = builder.max_response_bytes(max_response_bytes);", "drop(max_response_bytes);"),
    ("self.send_sorafs_endpoint($endpoint, Vec::new(), |_| {})", "self.send_sorafs_endpoint(SorafsEndpoint::account_json_get(\"changed\"), Vec::new(), |_| {})"),
])
def test_response_bound_is_preserved_through_endpoint_and_transport(old: str, new: str) -> None:
    source = CLIENT.read_text(encoding="utf-8")
    mutated = source.replace(old, new, 1)
    assert mutated != source
    with pytest.raises(AssertionError):
        _validate_response_bounds(mutated, TORII_API.read_text(encoding="utf-8"))
