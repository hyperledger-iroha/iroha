"""Static guards for the SoraFS V1 process-local economics hard cut."""

from __future__ import annotations

import json
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
RETIRED_PATHS = (
    "/v1/sorafs/" + "economics/pricing/manifests",
    "/v1/sorafs/" + "economics/hedging/feeds",
    "/v1/sorafs/" + "economics/status",
    "/v1/sorafs/" + "economics/pricing/active",
    "/v1/sorafs/" + "economics/hedging/reference",
)
RETIRED_SCHEMAS = (
    "SorafsEconomicsPricingAdmissionResponse",
    "SorafsEconomicsHedgingAdmissionResponse",
    "SorafsEconomicsPricingHead",
    "SorafsEconomicsPricingStatus",
    "SorafsEconomicsHedgingStatus",
    "SorafsEconomicsStatusResponse",
    "SorafsEconomicsActivePricingResponse",
    "SorafsEconomicsHedgingReferenceResponse",
)
OPENAPI_COPIES = (
    REPO_ROOT / "docs" / "portal" / "static" / "openapi" / "torii.json",
    REPO_ROOT
    / "docs"
    / "portal"
    / "static"
    / "openapi"
    / "versions"
    / "current"
    / "torii.json",
)


def test_process_local_economics_authority_cannot_reenter_sorafs_node() -> None:
    node_source = REPO_ROOT / "crates" / "sorafs_node" / "src"
    assert not (node_source / "economics.rs").exists()

    prohibited = (
        "GovernedPricingAdmissionOutcome",
        "SignedHedgingFeedAdmissionOutcome",
        "admit_governed_pricing_manifest",
        "governed_pricing_series",
        "active_governed_pricing",
        "admit_signed_hedging_feed",
        "signed_hedging_feed_ledger",
        "latest_signed_hedging_feeds",
        "derive_latest_hedging_reference_price",
        "pricing_checkpoint_path",
        "hedging_checkpoint_path",
        "signed_hedging_feeds",
    )
    for path in node_source.rglob("*.rs"):
        if path.name == "hedging_billing_service.rs":
            continue
        source = path.read_text(encoding="utf-8")
        for marker in prohibited:
            assert marker not in source, (path, marker)


def test_pricing_policy_config_is_retired_but_billing_feed_policy_remains() -> None:
    sources = [
        path.read_text(encoding="utf-8")
        for path in (
            REPO_ROOT / "crates" / "iroha_config" / "src" / "parameters" / "actual.rs",
            REPO_ROOT / "crates" / "iroha_config" / "src" / "parameters" / "user.rs",
            REPO_ROOT / "crates" / "sorafs_node" / "src" / "config.rs",
        )
    ]
    combined = "\n".join(sources)
    assert "pricing_trust_policy_path" not in combined
    assert "hedging_feed_trust_policy_path" in combined

    node = (REPO_ROOT / "crates" / "sorafs_node" / "src" / "lib.rs").read_text(
        encoding="utf-8"
    )
    assert "pub fn hedging_feed_trust_policy(" in node
    assert "decode_hedging_feed_trust_policy" in node


def test_retired_routes_exist_only_as_catalog_and_openapi_negatives() -> None:
    crate_sources = {
        path: path.read_text(encoding="utf-8")
        for path in (REPO_ROOT / "crates").rglob("*.rs")
    }
    allowed_negative_tests = {
        REPO_ROOT / "crates" / "iroha_torii" / "src" / "openapi.rs",
        REPO_ROOT
        / "crates"
        / "iroha_torii_shared"
        / "src"
        / "route_catalog.rs",
    }

    for retired_path in RETIRED_PATHS:
        occurrences = {
            path for path, source in crate_sources.items() if retired_path in source
        }
        assert occurrences == allowed_negative_tests, retired_path

    torii_api = (
        REPO_ROOT / "crates" / "iroha_torii" / "src" / "sorafs" / "api.rs"
    ).read_text(encoding="utf-8")
    assert "sorafs_economics_operator" not in torii_api
    assert "EconomicsRuntimeError" not in torii_api


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


def test_canonical_docs_require_discard_and_reseed() -> None:
    hedging = (
        REPO_ROOT / "docs" / "source" / "sorafs_hedging_plan.md"
    ).read_text(encoding="utf-8")
    pricing = (REPO_ROOT / "docs" / "source" / "sorafs_pricing.md").read_text(
        encoding="utf-8"
    )
    combined = " ".join(f"{hedging}\n{pricing}".split())

    for marker in (
        "The pre-release embedded economics authority has been hard-cut.",
        "does not mount the former `/v1/sorafs/economics/*`",
        "The `sorafs_economics_operator` role is retired.",
        "archive or delete `economics/governed-pricing.to`",
        "`economics/signed-hedging-feeds.to`",
        "V1 does not migrate those files",
        "there are no aliases or compatibility handlers",
    ):
        assert marker in combined
