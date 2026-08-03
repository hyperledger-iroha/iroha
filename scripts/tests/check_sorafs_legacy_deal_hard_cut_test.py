"""Static guards for the SoraFS V1 process-local deal-engine hard cut."""

from __future__ import annotations

import json
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
RETIRED_PATHS = tuple(
    "/v1/sorafs/" + "deal/" + suffix
    for suffix in (
        "fund-provider",
        "fund-client",
        "open",
        "cancel",
        "usage",
        "settle",
    )
)
RETIRED_SCHEMAS = (
    "FundProviderBondRequest",
    "FundProviderBondResponse",
    "FundClientCreditRequest",
    "FundClientCreditResponse",
    "SorafsDealFixed32Bytes",
    "SorafsDealStorageClass",
    "SorafsDealTerms",
    "SorafsDealProposal",
    "OpenDealRequest",
    "OpenDealResponse",
    "DealUsageTicket",
    "RecordDealUsageRequest",
    "RecordDealUsageResponse",
    "SettleDealRequest",
    "CancelDealRequest",
    "DealSettlementResponse",
)
OPENAPI_COPIES = (
    REPO_ROOT / "artifacts" / "openapi" / "torii.json",
    REPO_ROOT
    / "artifacts"
    / "openapi"
    / "versions"
    / "current"
    / "torii.json",
)


def test_process_local_deal_engine_cannot_reenter_sorafs_node() -> None:
    node_source = REPO_ROOT / "crates" / "sorafs_node" / "src"
    data_model_source = REPO_ROOT / "crates" / "iroha_data_model" / "src"

    assert not (node_source / "deal.rs").exists()
    assert not (data_model_source / "sorafs" / "deal.rs").exists()
    for path in node_source.rglob("*.rs"):
        source = path.read_text(encoding="utf-8")
        assert "DealEngine" not in source, path
        assert "deal_engine" not in source, path
    event_source = (data_model_source / "events" / "data" / "sorafs.rs").read_text(
        encoding="utf-8"
    )
    assert "SorafsDealUsage" not in event_source
    assert "SorafsDealSettlement" not in event_source


def test_retired_deal_telemetry_quota_is_not_configurable() -> None:
    for path in (
        REPO_ROOT / "crates" / "iroha_config" / "src" / "parameters" / "actual.rs",
        REPO_ROOT / "crates" / "iroha_config" / "src" / "parameters" / "user.rs",
        REPO_ROOT / "crates" / "iroha_config" / "src" / "parameters" / "defaults.rs",
        REPO_ROOT / "crates" / "iroha_torii" / "src" / "sorafs" / "limits.rs",
    ):
        source = path.read_text(encoding="utf-8")
        assert "deal_telemetry" not in source, path
        assert "DealTelemetry" not in source, path


def test_canonical_quantity_and_governed_settlement_primitives_remain() -> None:
    manifest_deal = (
        REPO_ROOT / "crates" / "sorafs_manifest" / "src" / "deal.rs"
    ).read_text(encoding="utf-8")

    assert "XOR_QUANTITY_SCALE, XorQuantity" in manifest_deal
    assert "pub struct DealLedgerSnapshotV1" in manifest_deal
    assert "pub struct DealSettlementV1" in manifest_deal


def test_retired_routes_exist_only_as_runtime_catalog_and_openapi_negatives() -> None:
    crate_sources = {
        path: path.read_text(encoding="utf-8")
        for path in (REPO_ROOT / "crates").rglob("*.rs")
    }
    allowed_negative_tests = {
        REPO_ROOT
        / "crates"
        / "iroha_torii"
        / "src"
        / "tests"
        / "lib_strict_request_targets.rs",
        REPO_ROOT / "crates" / "iroha_torii" / "src" / "openapi.rs",
        REPO_ROOT
        / "crates"
        / "iroha_torii_shared"
        / "src"
        / "route_catalog"
        / "tests.rs",
    }

    for retired_path in RETIRED_PATHS:
        occurrences = {
            path
            for path, source in crate_sources.items()
            if retired_path in source
        }
        assert occurrences == allowed_negative_tests, retired_path


def test_checked_openapi_copies_exclude_retired_routes_and_schemas() -> None:
    documents = [
        json.loads(path.read_text(encoding="utf-8")) for path in OPENAPI_COPIES
    ]

    assert documents[0] == documents[1]
    for document in documents:
        paths = document["paths"]
        schemas = document["components"]["schemas"]
        assert not set(RETIRED_PATHS).intersection(paths)
        assert not set(RETIRED_SCHEMAS).intersection(schemas)


def test_pin_register_openapi_is_dual_input_and_json_only_acknowledgement() -> None:
    for path in OPENAPI_COPIES:
        document = json.loads(path.read_text(encoding="utf-8"))
        operation = document["paths"]["/v1/sorafs/pin/register"]["post"]
        request_content = operation["requestBody"]["content"]
        assert set(request_content) == {"application/json", "application/x-norito"}
        assert (
            request_content["application/json"]["schema"]["$ref"]
            == "#/components/schemas/VersionedSignedTransactionJson"
        )
        assert (
            request_content["application/x-norito"]["schema"][
                "x-iroha-norito-schema"
            ]
            == "SignedTransaction"
        )
        assert set(operation["responses"]["202"]["content"]) == {"application/json"}
        assert (
            operation["responses"]["202"]["content"]["application/json"]["schema"][
                "$ref"
            ]
            == "#/components/schemas/SorafsPinRegisterResponseV1"
        )


def test_docs_do_not_advertise_the_retired_process_local_api() -> None:
    docs = REPO_ROOT / "docs"
    for path in docs.rglob("*.md"):
        source = path.read_text(encoding="utf-8")
        assert "DealEngine" not in source, path
        assert "deal engine" not in source.lower(), path
        assert "deal_engine" not in source.lower(), path
        for retired_path in RETIRED_PATHS:
            assert retired_path not in source, path
