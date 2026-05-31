from __future__ import annotations

import json
from dataclasses import replace
from pathlib import Path
from urllib.parse import urlsplit

import pytest
import requests

from iroha_python import DataspaceSpec, ToriiClient, plan_dataspace, write_dataspace_plan


class FakeSession:
    def __init__(self, responses: list[requests.Response]):
        self.responses = responses
        self.calls: list[dict[str, object]] = []

    def request(self, method: str, url: str, **kwargs: object) -> requests.Response:
        self.calls.append(
            {
                "method": method,
                "path": urlsplit(url).path,
                "params": kwargs.get("params"),
                "data": kwargs.get("data"),
            }
        )
        if not self.responses:
            raise AssertionError(f"unexpected request {method} {url}")
        response = self.responses.pop(0)
        response.url = url
        return response


def response(status: int, payload: object | None = None) -> requests.Response:
    result = requests.Response()
    result.status_code = status
    if payload is None:
        result._content = b""
    else:
        result._content = json.dumps(payload).encode("utf-8")
        result.headers["Content-Type"] = "application/json"
    return result


def test_plan_dataspace_generates_manifest_catalog_and_summary(tmp_path: Path) -> None:
    spec = DataspaceSpec(
        dataspace_alias="boi",
        dataspace_id=42,
        lane_alias="boi-payments",
        lane_id=7,
        governance_module="parliament",
        settlement_handle="xor_global",
        validators=["validator01"],
        route_instructions=["TransferAsset"],
    )

    plan = plan_dataspace(spec)

    assert plan.manifest["lane"] == "boi-payments"
    assert plan.manifest["quorum"] == 1
    assert 'alias = "boi"' in plan.catalog_snippet
    assert "dataspace:42" not in plan.catalog_snippet
    assert plan.summary["dataspace_id"] == 42

    outputs = write_dataspace_plan(plan, tmp_path)
    assert outputs["manifest"].read_text(encoding="utf-8").startswith("{\n")
    assert outputs["catalog"].read_text(encoding="utf-8") == plan.catalog_snippet


def test_plan_dataspace_escapes_toml_strings() -> None:
    spec = DataspaceSpec(
        dataspace_alias='boi"retail',
        dataspace_id=42,
        lane_alias='lane\none',
        lane_id=7,
        governance_module="parliament",
        settlement_handle="xor_global",
        validators=["validator01"],
        metadata={'owner"team': "payments\nops"},
        route_accounts=['alice"retail'],
        route_instructions=['Transfer"Asset'],
    )

    plan = plan_dataspace(spec)

    assert 'alias = "lane\\none"' in plan.catalog_snippet
    assert 'dataspace = "boi\\"retail"' in plan.catalog_snippet
    assert '"owner\\"team" = "payments\\nops"' in plan.catalog_snippet


@pytest.mark.parametrize(
    ("patch", "match"),
    [
        ({"dataspace_alias": ""}, "dataspace_alias"),
        ({"lane_alias": ""}, "lane_alias"),
        ({"lane_id": -1}, "lane_id"),
        ({"validators": []}, "validators"),
        ({"validators": ["validator 01"]}, "whitespace"),
        ({"validators": ["validator@is"]}, "encoded account identifier"),
        ({"quorum": 0}, "quorum"),
        ({"quorum": 3}, "quorum"),
        ({"dataspace_id": -1}, "dataspace_id"),
        ({"dataspace_hash": "not-hex"}, "dataspace_hash"),
    ],
)
def test_plan_dataspace_rejects_invalid_inputs(
    patch: dict[str, object],
    match: str,
) -> None:
    fields: dict[str, object] = {
        "dataspace_alias": "boi",
        "lane_alias": "boi-payments",
        "lane_id": 7,
        "governance_module": "parliament",
        "settlement_handle": "xor_global",
        "validators": ["validator01", "validator02"],
        "quorum": 2,
    }
    fields.update(patch)
    spec = DataspaceSpec(**fields)

    with pytest.raises(ValueError, match=match):
        plan_dataspace(spec)


@pytest.mark.parametrize(
    "name_patch",
    [
        {"manifest_name": "../escape.json"},
        {"catalog_name": "/tmp/escape.toml"},
        {"summary_name": "nested/summary.json"},
        {"space_directory_name": r"nested\\manifest.to"},
    ],
)
def test_plan_dataspace_rejects_artifact_path_traversal(
    name_patch: dict[str, str],
) -> None:
    spec = DataspaceSpec(
        dataspace_alias="boi",
        dataspace_id=42,
        lane_alias="boi-payments",
        lane_id=7,
        governance_module="parliament",
        settlement_handle="xor_global",
        validators=["validator01"],
        **name_patch,
    )

    with pytest.raises(ValueError, match="file name"):
        plan_dataspace(spec)


def test_write_dataspace_plan_rejects_tampered_artifact_path(
    tmp_path: Path,
) -> None:
    plan = plan_dataspace(
        DataspaceSpec(
            dataspace_alias="boi",
            dataspace_id=42,
            lane_alias="boi-payments",
            lane_id=7,
            governance_module="parliament",
            settlement_handle="xor_global",
            validators=["validator01"],
        )
    )
    tampered = replace(plan, manifest_name="../escape.json")

    with pytest.raises(ValueError, match="file name"):
        write_dataspace_plan(tampered, tmp_path)

    assert not (tmp_path.parent / "escape.json").exists()


def test_client_dataspace_readiness_accepts_lane_governance_status() -> None:
    client = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)
    status = {
        "lane_governance": [
            {
                "lane_id": 3,
                "alias": "boi",
                "dataspace_id": 42,
                "manifest_required": True,
                "manifest_ready": True,
            }
        ],
        "lane_governance_sealed_aliases": [],
    }

    assert client.get_dataspace("boi", status=status)["dataspace_id"] == 42
    assert client.dataspace_ready(42, status=status)
    assert client.smoke_dataspace("boi", status=status).ready


def test_client_dataspace_smoke_rejects_missing_or_sealed_status() -> None:
    client = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)

    missing = client.dataspace_status("boi", status={"lane_governance": []})
    assert missing.found is False
    assert missing.ready is False

    sealed_status = {
        "lane_governance": [
            {
                "lane_id": 1,
                "alias": "boi",
                "dataspace_id": 9,
                "manifest_required": False,
                "manifest_ready": True,
            }
        ],
        "lane_governance_sealed_aliases": ["boi"],
    }
    sealed = client.dataspace_status("boi", status=sealed_status)
    assert sealed.found is True
    assert sealed.ready is False


def test_compose_asset_id_accepts_dataspace_scope_shortcut() -> None:
    assert (
        ToriiClient.compose_asset_id("abc", "alice@boi", scope="42")
        == "abc#alice@boi#dataspace:42"
    )
