"""Complete source contracts for the shipped SoraFS hedging/billing service.

The rollout suite retains the collected test entrypoints. This module owns the
route, authentication, acknowledgement, freshness and CLI surface assertions.
"""

from __future__ import annotations

import json
import re
from pathlib import Path

from scripts.tests.sorafs_hedging_billing_freshness_contract import (
    assert_shipped_hedging_billing_freshness_contract,
)
from scripts.tests.sorafs_rollout_gate_source_support import read_source as read


REPO_ROOT = Path(__file__).resolve().parents[2]
IROHA_CLIENT_RS = REPO_ROOT / "crates" / "iroha" / "src" / "client.rs"
TORII_LIB_RS = REPO_ROOT / "crates" / "iroha_torii" / "src" / "lib.rs"
TORII_SORAFS_HEDGING_BILLING_API_RS = (
    REPO_ROOT
    / "crates"
    / "iroha_torii"
    / "src"
    / "sorafs"
    / "hedging_billing_api.rs"
)
TORII_SHARED_SORAFS_HEDGING_BILLING_API_RS = (
    REPO_ROOT
    / "crates"
    / "iroha_torii_shared"
    / "src"
    / "sorafs_hedging_billing_api.rs"
)
TORII_OPENAPI_RS = (
    REPO_ROOT / "crates" / "iroha_torii" / "assets" / "openapi" / "torii.json"
)
TORII_ROUTE_CATALOG_RS = (
    REPO_ROOT / "crates" / "iroha_torii_shared" / "src" / "route_catalog.rs"
)
IROHAD_SORAFS_HEDGING_BILLING_RUNTIME_RS = (
    REPO_ROOT / "crates" / "irohad" / "src" / "sorafs_hedging_billing_runtime.rs"
)
SORAFS_HEDGING_BILLING_SERVICE_RS = (
    REPO_ROOT / "crates" / "sorafs_node" / "src" / "hedging_billing_service.rs"
)
IROHA_CLI_SORAFS_RS = REPO_ROOT / "crates" / "iroha_cli" / "src" / "commands" / "sorafs.rs"
SORAFS_CLI_RS = REPO_ROOT / "crates" / "sorafs_orchestrator" / "src" / "bin" / "sorafs_cli.rs"


SHIPPED_HEDGING_BILLING_ROUTES = (
    ("/v1/sorafs/billing/status", "get"),
    ("/v1/sorafs/billing/statements", "get"),
    ("/v1/sorafs/billing/statements/{statement_id}", "get"),
    (
        "/v1/sorafs/billing/statements/{statement_id}/acknowledgements",
        "post",
    ),
    ("/v1/sorafs/billing/reconciliation", "get"),
    ("/v1/sorafs/hedging/exposure", "get"),
    ("/v1/sorafs/hedging/intents", "get"),
)

UNIMPLEMENTED_HEDGING_BILLING_CLI_SUBCOMMANDS = (
    "hedgingd",
    "billingd",
    "hedging-daemon",
    "billing-daemon",
    "price-feed-collector",
    "collector-service",
    "hedge-execute",
    "exposure-status",
    "statement-publish",
    "statement-ack",
    "billing-api",
    "hedging-status",
)

UNIMPLEMENTED_HEDGING_BILLING_NESTED_CLI_COMMANDS = (
    "hedging daemon",
    "hedging price-feed-collector",
    "hedging collector-service",
    "hedging hedge-execute",
    "hedging exposure-status",
    "hedging status",
    "billing daemon",
    "billing statement-publish",
    "billing statement-ack",
    "billing api",
)


def unimplemented_hedging_billing_cli_matches(source: str) -> list[str]:
    hyphenated_matches = [
        subcommand
        for subcommand in UNIMPLEMENTED_HEDGING_BILLING_CLI_SUBCOMMANDS
        if f'"{subcommand}"' in source or f"`{subcommand}`" in source
    ]
    nested_matches = [
        command
        for command in UNIMPLEMENTED_HEDGING_BILLING_NESTED_CLI_COMMANDS
        if re.search(rf"(?<![A-Za-z0-9_/-]){re.escape(command)}(?=$|[\"`\s])", source)
    ]
    return hyphenated_matches + nested_matches


def assert_unimplemented_hedging_billing_cli_matcher_has_negative_controls() -> None:
    shipped_local_subcommands = (
        "hedging",
        "billing",
        "iroha",
        "feed",
        "reference-price",
        "billing-cycle",
        "statement-publication",
        "metrics-alerts",
        "hedging-canary",
        "billing-cycle-canary",
        "statement-publish-canary",
        "iroha app sorafs toolkit validate hedging",
        "iroha app sorafs toolkit billing",
        "hedging feed",
        "hedging reference-price",
        "hedging metrics-alerts",
        "hedging status-canary",
        "billing billing-cycle",
        "billing statement-publication",
        "billing statement-publish-canary",
        "billing statement-ack-evidence",
    )

    assert unimplemented_hedging_billing_cli_matches(
        '"hedgingd" `price-feed-collector` "statement-publish" "billing-api"'
    ) == [
        "hedgingd",
        "price-feed-collector",
        "statement-publish",
        "billing-api",
    ]
    assert unimplemented_hedging_billing_cli_matches(
        "`sorafs hedging daemon` "
        '"sorafs hedging price-feed-collector" '
        "`billing statement-publish` "
        '"billing api --listen :8080"'
    ) == [
        "hedging daemon",
        "hedging price-feed-collector",
        "billing statement-publish",
        "billing api",
    ]
    assert unimplemented_hedging_billing_cli_matches(
        " ".join(f'"{subcommand}"' for subcommand in shipped_local_subcommands)
    ) == []


def assert_shipped_hedging_billing_service_surface_is_exact_and_authenticated() -> None:
    irohad_main = REPO_ROOT / "crates" / "irohad" / "src" / "main.rs"
    route_sources = (
        TORII_SORAFS_HEDGING_BILLING_API_RS,
        TORII_OPENAPI_RS,
        TORII_ROUTE_CATALOG_RS,
    )
    for source_path in route_sources:
        source = read(source_path)
        missing = [
            route
            for route, _method in SHIPPED_HEDGING_BILLING_ROUTES
            if route not in source
        ]
        assert missing == [], source_path

    torii = read(TORII_LIB_RS)
    daemon = read(irohad_main)
    for required in (
        "HedgingBillingRuntimeApiV1",
        "with_sorafs_hedging_billing_runtime",
        "sorafs_hedging_billing_runtime",
    ):
        assert required in torii
        assert required in daemon

    api_source = read(TORII_SORAFS_HEDGING_BILLING_API_RS)
    client_source = read(IROHA_CLIENT_RS)
    acknowledgement_wire_source = read(
        TORII_SHARED_SORAFS_HEDGING_BILLING_API_RS
    )
    service_source = read(SORAFS_HEDGING_BILLING_SERVICE_RS)
    runtime_source = read(IROHAD_SORAFS_HEDGING_BILLING_RUNTIME_RS)
    route_catalog_source = read(TORII_ROUTE_CATALOG_RS)

    status_handler = api_source[
        api_source.index("async fn billing_status_inner(") : api_source.index(
            "pub(crate) async fn handle_get_sorafs_billing_statements"
        )
    ]
    assert "require_canonical_auth" in status_handler
    assert "runtime.daemon_status()" in status_handler
    assert "require_billing_manager" not in status_handler

    assert_shipped_hedging_billing_freshness_contract(
        service_source=service_source,
        runtime_source=runtime_source,
    )

    assert "struct BillingAcknowledgementProofBodyV1" not in api_source
    assert "struct SorafsBillingAcknowledgementProof" not in client_source
    for required_wire_contract in (
        "pub struct BillingAcknowledgementProofV1",
        'name = "iroha_torii_shared::sorafs_hedging_billing_api::BillingAcknowledgementProofV1"',
        'frame = "iroha.torii.v1.sorafs.billing.acknowledgement_proof"',
        '"fe75acabe03d788012f2e7c556319997"',
        "pub request_nonce: [u8; 32]",
        "pub authentication_proof: Vec<u8>",
        "impl fmt::Debug for BillingAcknowledgementProofV1",
        '"[REDACTED]"',
    ):
        assert required_wire_contract in acknowledgement_wire_source
    assert (
        "BillingAcknowledgementProofV1 as BillingAcknowledgementProofBodyV1"
        in api_source
    )
    assert (
        "BillingAcknowledgementProofV1 as SorafsBillingAcknowledgementProof"
        in client_source
    )
    acknowledgement_decoder = api_source[
        api_source.index(
            "fn decode_acknowledgement_proof("
        ) : api_source.index("fn server_time_unix(")
    ]
    assert "request.request_nonce == [0; 32]" in acknowledgement_decoder
    assert "request_nonce: proof.request_nonce" in api_source
    assert "acknowledgement_http_binding_digest" not in api_source

    acknowledgement_api = service_source[
        service_source.index(
            "pub fn api_acknowledge_statement("
        ) : service_source.index("pub fn api_exposure_page(")
    ]
    assert "request.request_nonce == [0; 32]" in acknowledgement_api
    digest_call = re.search(
        r"billing_statement_acknowledgement_request_digest_v1\((.*?)\)\?;",
        acknowledgement_api,
        re.S,
    )
    assert digest_call is not None
    digest_call_arguments = digest_call.group(1)
    for required_argument in (
        "request.statement_id",
        "&request.owner_account_id",
        "request.request_nonce",
    ):
        assert required_argument in digest_call_arguments
    assert "authentication_proof" not in digest_call_arguments

    digest_function = service_source[
        service_source.index(
            "pub fn billing_statement_acknowledgement_request_digest_v1("
        ) : service_source.index("fn projection_close_start(")
    ]
    for required_preimage in (
        "hasher.update(&statement_id)",
        "hasher.update(owner_account_id)",
        "hasher.update(&request_nonce)",
    ):
        assert required_preimage in digest_function
    assert "request_nonce == [0; 32]" in digest_function
    assert "authentication_proof" not in digest_function

    method_guard = api_source[
        api_source.index("fn require_method(") : api_source.index(
            "fn runtime_error_response("
        )
    ]
    assert api_source.count("require_method(&method, Method::GET)") == 5
    assert "actual == &expected" in method_guard
    assert "Method::HEAD" not in method_guard
    sorafs_catalog_start = route_catalog_source.index("pub mod sorafs {")
    public_get_start = route_catalog_source.index(
        "    const fn public_get(", sorafs_catalog_start
    )
    public_post_start = route_catalog_source.index(
        "    const fn public_post(", public_get_start
    )
    public_get_catalog = route_catalog_source[
        public_get_start:public_post_start
    ]
    assert ".with_implicit_head(" not in public_get_catalog

    expected_routes = dict(SHIPPED_HEDGING_BILLING_ROUTES)
    for spec_path in (
        REPO_ROOT / "artifacts" / "openapi" / "torii.json",
        REPO_ROOT
        / "artifacts"
        / "openapi"
        / "versions"
        / "current"
        / "torii.json",
    ):
        spec = json.loads(read(spec_path))
        paths = spec["paths"]
        observed = {
            route: method
            for route, method in expected_routes.items()
            if route in paths and method in paths[route]
        }
        assert observed == expected_routes
        exposed_family = {
            route
            for route in paths
            if route.startswith("/v1/sorafs/billing/")
            or route.startswith("/v1/sorafs/hedging/")
        }
        assert exposed_family == set(expected_routes)

        schemas = spec["components"]["schemas"]
        bytes32_schema = schemas["HedgingBillingBytes32V1"]
        assert bytes32_schema["type"] == "string"
        assert bytes32_schema["minLength"] == 64
        assert bytes32_schema["maxLength"] == 64
        assert bytes32_schema["pattern"] == "^[0-9A-F]{64}$"

        acknowledgement_schema = schemas["BillingAcknowledgementProofBodyV1"]
        assert set(acknowledgement_schema["required"]) == {
            "request_nonce",
            "authentication_proof",
        }
        assert (
            acknowledgement_schema["properties"]["request_nonce"]["$ref"]
            == "#/components/schemas/HedgingBillingBytes32V1"
        )
        assert "non-zero" in acknowledgement_schema["properties"][
            "request_nonce"
        ]["description"].lower()
        assert (
            acknowledgement_schema["properties"]["authentication_proof"][
                "writeOnly"
            ]
            is True
        )

        status_schema = schemas["HedgingBillingDaemonStatusV1"]
        assert {
            "anchor",
            "last_tick_fresh",
            "finalized_projection_ready",
            "finalized_head_height",
            "finalized_lag_blocks",
            "ready",
        } <= set(status_schema["required"])
        assert (
            status_schema["properties"]["anchor"]["$ref"]
            == "#/components/schemas/HedgingBillingProjectionAnchorV1"
        )

        for route, method in expected_routes.items():
            operation = paths[route][method]
            header_names = {
                parameter.get("name")
                for parameter in operation.get("parameters", [])
                if parameter.get("in") == "header"
            }
            assert {
                "X-Iroha-Account",
                "X-Iroha-Signature",
                "X-Iroha-Timestamp-Ms",
                "X-Iroha-Nonce",
                "X-Iroha-Witness",
            } <= header_names, (spec_path, route)
            responses = operation.get("responses", {})
            assert {"200", "401"} <= set(responses), (spec_path, route)
            if route == "/v1/sorafs/billing/status":
                assert "bootstrap" in operation.get("description", "").lower()
                assert "403" not in responses, (spec_path, route)
            if route in {
                "/v1/sorafs/billing/reconciliation",
                "/v1/sorafs/hedging/exposure",
                "/v1/sorafs/hedging/intents",
            }:
                assert "403" in responses, (spec_path, route)
            success_headers = responses["200"].get("headers", {})
            assert (
                success_headers.get("Cache-Control", {})
                .get("schema", {})
                .get("const")
                == "private, no-store"
            ), (spec_path, route)

    unexpected_cli: dict[str, list[str]] = {}
    for source_path in (IROHA_CLI_SORAFS_RS, SORAFS_CLI_RS):
        matched = unimplemented_hedging_billing_cli_matches(read(source_path))
        if matched:
            unexpected_cli[str(source_path.relative_to(REPO_ROOT))] = matched
    assert unexpected_cli == {}
