"""Operator-signed trigger completion history (``GET /v1/triggers/completed``).

The route is a history collection of the Torii collection contract
(``specs/torii/collection_queries.md``): shared ``filter``/``select``/``limit``/
``cursor`` controls, a ``{items, next_cursor}`` page of flat rows, newest first.
"""

from __future__ import annotations

import base64
import json
from typing import Any
from urllib.parse import parse_qsl, urlsplit

import pytest
import requests
from iroha_torii_client.client import canonical_request_message

from iroha_python import (
    F,
    ListQuery,
    ListQueryError,
    NetworkId,
    OperatorSigningContext,
    Page,
    ToriiClient,
    ToriiQueryError,
    TriggerCompletionRecord,
)
from iroha_python.crypto import Ed25519KeyPair

NETWORK_BYTES = bytes([0xA5]) * 32
KEY_PAIR = Ed25519KeyPair.from_private_key(bytes([0x0B]) * 32)
ROUTE = "/v1/triggers/completed"


def _row(height: int = 7, **changes: Any) -> dict[str, Any]:
    row: dict[str, Any] = {
        "block_height": height,
        "output_index": 1,
        "completion_index": 0,
        "trigger_id": "timer",
        "trigger_execution_hash": "abcd",
        "entrypoint_index": None,
        "step_index": 0,
        "outcome": "Failure",
        "message": "out of fuel",
    }
    row.update(changes)
    return row


def _page(*rows: dict[str, Any], next_cursor: str | None = None) -> dict[str, Any]:
    return {"items": list(rows), "next_cursor": next_cursor}


def _response(payload: Any, status: int = 200) -> requests.Response:
    response = requests.Response()
    response.status_code = status
    response.headers["Content-Type"] = "application/json"
    response._content = json.dumps(payload).encode("utf-8")
    response._content_consumed = True
    response.encoding = "utf-8"
    return response


class _Session(requests.Session):
    """Answer each request with the next scripted response and record it."""

    def __init__(self, *responses: requests.Response) -> None:
        super().__init__()
        self.responses = list(responses)
        self.calls: list[dict[str, Any]] = []

    def request(self, method: str | bytes, url: str | bytes, **kwargs: Any) -> requests.Response:
        self.calls.append({"method": method, "url": url, **kwargs})
        if not self.responses:
            raise AssertionError(f"unexpected request {method!r} {url!r}")
        return self.responses.pop(0)


def _client(*payloads: Any) -> tuple[ToriiClient, _Session]:
    session = _Session(*(_response(payload) for payload in payloads))
    client = ToriiClient(
        "https://torii.example",
        session=session,
        operator_signing_context=OperatorSigningContext(
            NetworkId.from_bytes(NETWORK_BYTES), KEY_PAIR
        ),
        max_retries=5,
        retry_on_methods=["GET"],
        retry_on_status=[503],
        chain_discriminant=753,
    )
    return client, session


def _query_pairs(call: dict[str, Any]) -> list[tuple[str, str]]:
    parts = urlsplit(call["url"])
    assert f"{parts.scheme}://{parts.netloc}{parts.path}" == f"https://torii.example{ROUTE}"
    return parse_qsl(parts.query, keep_blank_values=True, strict_parsing=bool(parts.query))


def test_list_sends_the_shared_controls_once_under_the_operator_signature() -> None:
    client, session = _client(_page(_row(), next_cursor="next"))

    page = client.list_trigger_completions(
        trigger_id="timer",
        outcome="Failure",
        from_height=10,
        to_height=20,
        limit=5,
        cursor="opaque-cursor",
    )

    assert len(session.calls) == 1
    call = session.calls[0]
    assert call["method"] == "GET"
    # The same wire request as the Rust client's `get_trigger_completions`.
    assert _query_pairs(call) == [
        (
            "filter",
            'trigger_id = "timer" and outcome = "Failure" '
            "and block_height >= 10 and block_height <= 20",
        ),
        ("limit", "5"),
        ("cursor", "opaque-cursor"),
    ]
    assert call["params"] is None
    assert call["data"] == b""
    assert call["allow_redirects"] is False
    assert call["stream"] is True

    headers = call["headers"]
    assert headers["Accept"] == "application/json"
    assert "Authorization" not in headers
    assert "X-Iroha-Account" not in headers
    assert headers["x-iroha-operator-public-key"] == KEY_PAIR.public_key_multihash
    target = call["url"][len("https://torii.example") :]
    message = b"".join(
        (
            b"iroha.operator.http-request.network.v1\0",
            NETWORK_BYTES,
            canonical_request_message("GET", target, b""),
            f"\n{headers['x-iroha-operator-timestamp-ms']}".encode("ascii"),
            f"\n{headers['x-iroha-operator-nonce']}".encode("ascii"),
        )
    )
    signature = base64.b64decode(headers["x-iroha-operator-signature"], validate=True)
    assert KEY_PAIR.verify(message, signature)

    assert isinstance(page, Page)
    assert page.next_cursor == "next"
    assert page.total is None
    (record,) = page.items
    assert record == TriggerCompletionRecord(
        block_height=7,
        output_index=1,
        completion_index=0,
        trigger_id="timer",
        trigger_execution_hash="abcd",
        entrypoint_index=None,
        step_index=0,
        outcome="Failure",
        message="out of fuel",
    )
    assert record.raw == _row()


def test_list_without_controls_sends_no_query_string() -> None:
    client, session = _client(_page())

    page = client.list_trigger_completions()

    assert session.calls[0]["url"] == f"https://torii.example{ROUTE}"
    assert page.items == ()
    assert page.next_cursor is None


@pytest.mark.parametrize(
    ("controls", "expected"),
    [
        ({"trigger_id": "clock"}, 'trigger_id = "clock"'),
        ({"trigger_id": 'say "hi"'}, 'trigger_id = "say \\"hi\\""'),
        ({"outcome": "Success"}, 'outcome = "Success"'),
        ({"from_height": 1_200}, "block_height >= 1200"),
        ({"to_height": (1 << 64) - 1}, "block_height <= 18446744073709551615"),
        ({"from_height": 0, "to_height": 0}, "block_height >= 0 and block_height <= 0"),
        # The filter comes first and the shorthands follow as conjuncts, as in the CLI.
        (
            {"filter": F.step_index == 0, "outcome": "Failure"},
            'step_index = 0 and outcome = "Failure"',
        ),
        (
            {"filter": (F.step_index == 0) | (F.step_index == 1), "trigger_id": "clock"},
            '(step_index = 0 or step_index = 1) and trigger_id = "clock"',
        ),
        # A text filter is parsed to join the shorthands and sent in canonical form.
        (
            {"filter": "message IS NOT NULL", "to_height": 9},
            "message is not null and block_height <= 9",
        ),
        # Without a shorthand a text filter is sent unchanged.
        ({"filter": "message IS NOT NULL"}, "message IS NOT NULL"),
        ({"filter": F.entrypoint_index.is_null()}, "entrypoint_index is null"),
    ],
)
def test_filter_shorthands_join_the_filter_as_conjuncts(
    controls: dict[str, Any], expected: str
) -> None:
    client, session = _client(_page())

    client.list_trigger_completions(**controls)

    assert _query_pairs(session.calls[0]) == [("filter", expected)]


@pytest.mark.parametrize(
    ("controls", "error"),
    [
        ({"outcome": "failure"}, ValueError),
        ({"outcome": "all"}, ValueError),
        ({"outcome": 1}, ValueError),
        ({"trigger_id": ""}, ValueError),
        ({"trigger_id": " clock"}, ValueError),
        ({"trigger_id": 7}, TypeError),
        ({"from_height": True}, TypeError),
        ({"from_height": "10"}, TypeError),
        ({"from_height": -1}, ValueError),
        ({"to_height": 1 << 64}, ValueError),
    ],
)
def test_filter_shorthands_are_validated_before_any_request(
    controls: dict[str, Any], error: type[Exception]
) -> None:
    client, session = _client()

    with pytest.raises(error, match="trigger completions"):
        client.list_trigger_completions(**controls)
    with pytest.raises(error, match="trigger completions"):
        client.iter_trigger_completions(**controls)

    assert session.calls == []


@pytest.mark.parametrize(
    ("controls", "code"),
    [
        ({"limit": 0}, "invalid_limit"),
        ({"limit": -3}, "invalid_limit"),
        ({"cursor": ""}, "invalid_cursor"),
        ({"cursor": "not a cursor"}, "invalid_cursor"),
        ({"select": []}, "invalid_select"),
        ({"select": ["block_height", "block_height"]}, "invalid_select"),
        ({"filter": "outcome = ", "trigger_id": "clock"}, "invalid_filter"),
        ({"filter": F.metadata.tag == {"a": 1}}, "invalid_filter"),
    ],
)
def test_invalid_controls_raise_torii_codes_before_any_request(
    controls: dict[str, Any], code: str
) -> None:
    client, session = _client()

    with pytest.raises(ListQueryError) as raised:
        client.list_trigger_completions(**controls)

    assert raised.value.code == code
    assert session.calls == []


@pytest.mark.parametrize(
    "retired",
    [
        {"id": "clock"},
        {"entrypoint_hash": "abcd"},
        {"scan_limit_blocks": 100},
        {"offset": 5},
        {"sort": "-block_height"},
        {"include_total": True},
        {"aggregate": None},
    ],
)
def test_retired_and_fixed_order_parameters_are_not_accepted(retired: dict[str, Any]) -> None:
    client, session = _client()

    with pytest.raises(TypeError, match="unexpected keyword argument"):
        client.list_trigger_completions(**retired)
    with pytest.raises(TypeError, match="unexpected keyword argument"):
        client.iter_trigger_completions(**retired)
    with pytest.raises(TypeError, match="positional"):
        client.list_trigger_completions(ListQuery(limit=1))  # type: ignore[misc]

    assert session.calls == []


def test_history_rules_reject_sort_total_and_aggregate_before_dispatch() -> None:
    client, session = _client()
    collection = client._trigger_completions()

    for query, code in (
        (ListQuery(sort="-block_height"), "invalid_sort"),
        (ListQuery(include_total=True), "invalid_include_total"),
    ):
        with pytest.raises(ListQueryError) as raised:
            collection.list(query)
        assert raised.value.code == code
        assert "trigger_completions" in raised.value.message

    assert session.calls == []


def test_select_sends_the_projection_and_returns_json_rows() -> None:
    projected = {"block_height": 7, "trigger_id": "timer", "outcome": "Failure", "message": None}
    client, session = _client(_page(projected), _page(projected))

    page = client.list_trigger_completions(
        trigger_id="timer",
        select=["block_height", "trigger_id", F.outcome, "message"],
        limit=5,
    )

    assert _query_pairs(session.calls[0]) == [
        ("filter", 'trigger_id = "timer"'),
        ("select", "block_height,trigger_id,outcome,message"),
        ("limit", "5"),
    ]
    assert page.items == (projected,)
    assert type(page.items[0]) is dict
    assert list(client.iter_trigger_completions(select=["block_height"])) == [projected]
    assert _query_pairs(session.calls[1]) == [("select", "block_height")]


def test_rows_keep_unknown_fields_and_accept_network_entrypoints() -> None:
    row = _row(entrypoint_index=3, outcome="Success", message=None, future_field={"x": 1})
    client, _session = _client(_page(row))

    (record,) = client.list_trigger_completions().items

    assert record.entrypoint_index == 3
    assert record.outcome == "Success"
    assert record.message is None
    assert record.raw == row
    assert record.raw is not row


@pytest.mark.parametrize(
    ("payload", "error", "match"),
    [
        # The retired bespoke envelope is not a page.
        (
            {"latest_height": 42, "from_height": 40, "to_height": 42, "completions": []},
            ValueError,
            "malformed page",
        ),
        ({"items": []}, ValueError, "next_cursor"),
        ({"items": [], "next_cursor": ""}, ValueError, "next_cursor"),
        ({"items": {}, "next_cursor": None}, ValueError, "items"),
        # History collections have no total.
        ({"items": [], "next_cursor": None, "total": 0}, ValueError, "omit total"),
        ({"items": ["row"], "next_cursor": None}, TypeError, "must be an object"),
        ({"items": [_row(outcome="success")], "next_cursor": None}, ValueError, "outcome"),
        ({"items": [_row(message=7)], "next_cursor": None}, TypeError, "message"),
        ({"items": [_row(block_height="7")], "next_cursor": None}, TypeError, "block_height"),
        ({"items": [_row(output_index=True)], "next_cursor": None}, TypeError, "output_index"),
        ({"items": [_row(completion_index=-1)], "next_cursor": None}, ValueError, "completion_index"),
        ({"items": [_row(step_index=1 << 64)], "next_cursor": None}, ValueError, "step_index"),
        ({"items": [_row(entrypoint_index="0")], "next_cursor": None}, TypeError, "entrypoint_index"),
        ({"items": [_row(trigger_id="")], "next_cursor": None}, ValueError, "trigger_id"),
        ({"items": [_row(trigger_execution_hash=None)], "next_cursor": None}, TypeError, "hash"),
    ],
)
def test_malformed_pages_and_rows_are_rejected(
    payload: dict[str, Any], error: type[Exception], match: str
) -> None:
    client, _session = _client(payload)

    with pytest.raises(error, match=match):
        client.list_trigger_completions()


@pytest.mark.parametrize(
    "field",
    [
        "block_height",
        "output_index",
        "completion_index",
        "trigger_id",
        "trigger_execution_hash",
        "entrypoint_index",
        "step_index",
        "outcome",
        "message",
    ],
)
def test_a_complete_row_requires_every_field(field: str) -> None:
    row = _row()
    del row[field]

    with pytest.raises(TypeError, match=f"missing required `{field}` field"):
        TriggerCompletionRecord.from_payload(row)


def test_a_page_larger_than_the_requested_limit_is_rejected() -> None:
    client, _session = _client(_page(_row(9), _row(8)))

    with pytest.raises(ValueError, match="exceed the requested limit"):
        client.list_trigger_completions(limit=1)


def test_iteration_follows_every_cursor_through_short_and_empty_pages() -> None:
    client, session = _client(
        _page(_row(9), _row(8), next_cursor="first"),
        _page(next_cursor="second"),
        _page(_row(3)),
    )

    heights = [
        record.block_height
        for record in client.iter_trigger_completions(trigger_id="clock", limit=2)
    ]

    assert heights == [9, 8, 3]
    assert [_query_pairs(call) for call in session.calls] == [
        [("filter", 'trigger_id = "clock"'), ("limit", "2")],
        [("filter", 'trigger_id = "clock"'), ("limit", "2"), ("cursor", "first")],
        [("filter", 'trigger_id = "clock"'), ("limit", "2"), ("cursor", "second")],
    ]
    # Every page is its own signed request.
    nonces = {call["headers"]["x-iroha-operator-nonce"] for call in session.calls}
    assert len(nonces) == 3


def test_iteration_is_lazy_and_resumes_from_a_cursor() -> None:
    client, session = _client(_page(_row(9), next_cursor="first"), _page(_row(8)))

    rows = client.iter_trigger_completions(cursor="resume")
    assert session.calls == []
    assert next(rows).block_height == 9
    assert len(session.calls) == 1
    assert _query_pairs(session.calls[0]) == [("cursor", "resume")]
    rows.close()
    assert len(session.calls) == 1


def test_iteration_refuses_a_cursor_that_does_not_advance() -> None:
    client, session = _client(_page(_row(9), next_cursor="stuck"), _page(next_cursor="stuck"))

    with pytest.raises(RuntimeError, match="refusing to loop"):
        list(client.iter_trigger_completions())

    assert len(session.calls) == 2


def test_manual_paging_passes_the_next_cursor_back() -> None:
    client, session = _client(_page(_row(9), next_cursor="first"), _page(_row(8)))

    first = client.list_trigger_completions(outcome="Failure", limit=1)
    second = client.list_trigger_completions(outcome="Failure", limit=1, cursor=first.next_cursor)

    assert first.has_more and not second.has_more
    assert _query_pairs(session.calls[1]) == [
        ("filter", 'outcome = "Failure"'),
        ("limit", "1"),
        ("cursor", "first"),
    ]


def test_torii_refusals_raise_the_shared_query_error() -> None:
    envelope = {
        "code": "invalid_query",
        "message": "unknown parameter `scan_limit_blocks`; expected one of: filter, select, limit, cursor",
        "details": {"field": "query"},
    }
    session = _Session(_response(envelope, status=400))
    client = ToriiClient(
        "https://torii.example",
        session=session,
        operator_signing_context=OperatorSigningContext(
            NetworkId.from_bytes(NETWORK_BYTES), KEY_PAIR
        ),
        chain_discriminant=753,
    )

    with pytest.raises(ToriiQueryError) as raised:
        client.list_trigger_completions()

    assert raised.value.code == "invalid_query"
    assert raised.value.parameter == "query"
    assert isinstance(raised.value, ListQueryError)


def test_reads_fail_before_dispatch_without_an_operator_context() -> None:
    session = _Session()
    client = ToriiClient("https://torii.example", session=session, chain_discriminant=753)

    with pytest.raises(ValueError, match="operator_signing_context"):
        client.list_trigger_completions()
    with pytest.raises(ValueError, match="operator_signing_context"):
        next(client.iter_trigger_completions())

    assert session.calls == []


@pytest.mark.parametrize(
    "retired_name",
    [
        "list_triggers",
        "list_triggers_typed",
        "get_trigger",
        "get_trigger_typed",
        "register_trigger",
        "register_trigger_typed",
        "delete_trigger",
        "delete_trigger_typed",
        "query_triggers",
    ],
)
def test_trigger_registry_methods_without_a_torii_route_are_absent(retired_name: str) -> None:
    assert not hasattr(ToriiClient, retired_name)


@pytest.mark.parametrize(
    "retired_export",
    [
        "TriggerRecord",
        "TriggerListPage",
        "TriggerMutationResponse",
        "TriggerCompletionSummary",
        "TriggerCompletionList",
    ],
)
def test_retired_trigger_exports_are_absent(retired_export: str) -> None:
    import iroha_torii_client

    import iroha_python

    assert not hasattr(iroha_python, retired_export)
    assert retired_export not in iroha_python.__all__
    assert not hasattr(iroha_torii_client, retired_export)
    assert "TriggerCompletionRecord" in iroha_python.__all__
