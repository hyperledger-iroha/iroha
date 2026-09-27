from __future__ import annotations

import json

from iroha_python.client import ToriiClient
from iroha_python.query import rwa_query_envelope

from .helpers import RecordingSession, StubResponse


SAMPLE_RWA_ID = "lot-001$commodities"
SAMPLE_OWNER = "ed0120111111111111111111111111111111111111111111111111111111111111@wonderland"


def test_rwa_query_envelope_builds_expected_shape() -> None:
    payload = rwa_query_envelope(
        filter={"eq": [{"name": "id"}, SAMPLE_RWA_ID]},
        sort=[{"key": "id", "order": "desc"}],
        limit=5,
        offset=2,
        fetch_size=10,
        query_name="find_rwas",
    )

    assert payload == {
        "filter": {"eq": [{"name": "id"}, SAMPLE_RWA_ID]},
        "sort": [{"key": "id", "order": "desc"}],
        "pagination": {"limit": 5, "offset": 2},
        "fetch_size": 10,
        "query": "find_rwas",
    }


def test_list_rwas_typed_encodes_params_and_decodes_page() -> None:
    session = RecordingSession(
        StubResponse(payload={"items": [{"id": SAMPLE_RWA_ID}], "total": 1})
    )
    client = ToriiClient("http://node.test", session=session)

    page = client.list_rwas_typed(
        filter={"eq": [{"name": "id"}, SAMPLE_RWA_ID]},
        sort="id:desc",
        limit=5,
        offset=2,
    )

    params = session.calls[0]["params"]
    assert json.loads(params["filter"]) == {"eq": [{"name": "id"}, SAMPLE_RWA_ID]}
    assert params["sort"] == "id:desc"
    assert params["limit"] == 5
    assert params["offset"] == 2
    assert page.total == 1
    assert page.items[0].id == SAMPLE_RWA_ID


def test_query_rwas_typed_posts_envelope_and_decodes_page() -> None:
    session = RecordingSession(
        StubResponse(payload={"items": [{"id": SAMPLE_RWA_ID}], "total": 1})
    )
    client = ToriiClient("http://node.test", session=session)

    page = client.query_rwas_typed(
        filter={"eq": [{"name": "id"}, SAMPLE_RWA_ID]},
        sort=[{"key": "id", "order": "asc"}],
        limit=3,
        offset=1,
        fetch_size=8,
        query_name="find_rwas",
    )

    body = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert body == {
        "filter": {"eq": [{"name": "id"}, SAMPLE_RWA_ID]},
        "sort": [{"key": "id", "order": "asc"}],
        "pagination": {"limit": 3, "offset": 1},
        "fetch_size": 8,
        "query": "find_rwas",
    }
    assert page.total == 1
    assert page.items[0].id == SAMPLE_RWA_ID


def test_list_explorer_rwas_typed_encodes_filters_and_decodes_page() -> None:
    session = RecordingSession(
        StubResponse(
            payload={
                "pagination": {
                    "page": 2,
                    "per_page": 25,
                    "total_pages": 4,
                    "total_items": 88,
                },
                "items": [
                    {
                        "id": SAMPLE_RWA_ID,
                        "owned_by": SAMPLE_OWNER,
                        "quantity": "10",
                        "held_quantity": "2.5",
                        "primary_reference": "vault-cert-001",
                        "status": None,
                        "is_frozen": False,
                        "metadata": {"grade": "AA"},
                    }
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    page = client.list_explorer_rwas_typed(
        page=2,
        per_page=25,
        owned_by=SAMPLE_OWNER,
        domain="commodities",
    )

    params = session.calls[0]["params"]
    assert params == {
        "page": 2,
        "per_page": 25,
        "owned_by": SAMPLE_OWNER,
        "domain": "commodities",
    }
    assert page.pagination.page == 2
    assert page.pagination.total_items == 88
    assert page.items[0].id == SAMPLE_RWA_ID
    assert page.items[0].status is None
    assert page.items[0].metadata == {"grade": "AA"}


def test_get_explorer_rwa_detail_typed_encodes_path_and_decodes_payload() -> None:
    session = RecordingSession(
        StubResponse(
            payload={
                "id": SAMPLE_RWA_ID,
                "owned_by": SAMPLE_OWNER,
                "quantity": "10",
                "held_quantity": "0",
                "primary_reference": "vault-cert-001",
                "status": "Active",
                "is_frozen": True,
                "metadata": {},
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    detail = client.get_explorer_rwa_detail_typed(SAMPLE_RWA_ID)

    assert session.calls[0]["url"].endswith("/v1/explorer/rwas/lot-001%24commodities")
    assert detail.id == SAMPLE_RWA_ID
    assert detail.status == "Active"
    assert detail.is_frozen is True
