"""Source contract for bounded SoraFS hedging/billing client responses."""

from __future__ import annotations

import json
from pathlib import Path


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


def test_hedging_billing_client_response_bounds_match_server_contract() -> None:
    client = CLIENT.read_text(encoding="utf-8")
    torii = TORII_API.read_text(encoding="utf-8")

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
            "    pub fn get_sorafs_billing_status(",
            "    /// Fetch one exact-checkpoint owner-isolated page",
        ),
        (
            "    pub fn get_sorafs_billing_statements(",
            "    /// Fetch one exact owned published billing statement",
        ),
        (
            "    pub fn post_sorafs_billing_statement_acknowledgement(",
            "    /// Fetch payload-free `SoraFS` billing delivery reconciliation status.",
        ),
        (
            "    pub fn get_sorafs_billing_reconciliation(",
            "    /// Fetch one exact-checkpoint page of finalized `SoraFS` hedging exposure.",
        ),
        (
            "    fn get_sorafs_hedging_projection(",
            "    /// Convenience: GET `/v1/sorafs/moderation/quarantine`",
        ),
    ):
        method = _source_slice(client, start, end)
        assert f".max_response_bytes({JSON_LIMIT})" in method, start

    statement_method = _source_slice(
        client,
        "    pub fn get_sorafs_billing_statement(",
        "    /// Submit one canonical owner acknowledgement",
    )
    assert f".max_response_bytes({STATEMENT_LIMIT})" in statement_method

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

    assert client.count(f".max_response_bytes({JSON_LIMIT})") == 5
    assert client.count(f".max_response_bytes({STATEMENT_LIMIT})") == 1

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
