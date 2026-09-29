"""Actual-byte bound and Content-Length admission for streamed Torii response bodies."""

from __future__ import annotations

import sys
from pathlib import Path
from typing import Optional

import pytest
from sumeragi_exact_json_test_support import StubResponse

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
if str(PACKAGE_ROOT) not in sys.path:
    sys.path.insert(0, str(PACKAGE_ROOT))

from iroha_torii_client.client import _read_bounded_response_body  # noqa: E402

MAXIMUM_BYTES = 64 * 1024


def _response(body: bytes, content_length: Optional[str]) -> StubResponse:
    headers = {"Content-Type": "application/json"}
    if content_length is not None:
        headers["Content-Length"] = content_length
    return StubResponse(raw=body, headers=headers)


@pytest.mark.parametrize("content_length", [str(MAXIMUM_BYTES), None])
def test_exact_size_body_is_returned_and_the_response_closed(
    content_length: Optional[str],
) -> None:
    body = b"{}" + b" " * (MAXIMUM_BYTES - 2)
    response = _response(body, content_length)

    assert _read_bounded_response_body(response, MAXIMUM_BYTES, "probe") == body
    assert response.was_closed is True


@pytest.mark.parametrize(
    ("label", "content_length"),
    [
        ("negative", "-1"),
        ("explicit plus", "+1"),
        ("leading zero", "01"),
        ("fractional", "1.0"),
        ("coalesced duplicate", "1, 1"),
        ("trailing whitespace", "1 "),
        ("leading whitespace", " 1"),
        ("empty", ""),
    ],
)
def test_noncanonical_content_length_is_rejected_and_the_response_closed(
    label: str, content_length: str
) -> None:
    response = _response(b"{}", content_length)

    with pytest.raises(ValueError, match="canonical unsigned decimal"):
        _read_bounded_response_body(response, MAXIMUM_BYTES, "probe")
    assert response.was_closed is True, label


@pytest.mark.parametrize(
    ("label", "body", "content_length"),
    [
        ("declared overflow", b"{}", str(MAXIMUM_BYTES + 1)),
        ("missing Content-Length", b" " * (MAXIMUM_BYTES + 1), None),
        ("understated Content-Length", b" " * (MAXIMUM_BYTES + 1), "1"),
    ],
)
def test_declared_and_actual_overflow_are_rejected_and_the_response_closed(
    label: str, body: bytes, content_length: Optional[str]
) -> None:
    response = _response(body, content_length)

    with pytest.raises(ValueError, match=f"{MAXIMUM_BYTES}-byte size bound"):
        _read_bounded_response_body(response, MAXIMUM_BYTES, "probe")
    assert response.was_closed is True, label


@pytest.mark.parametrize("maximum_body_bytes", [-1, True, 1.0])
def test_invalid_byte_bound_is_rejected(maximum_body_bytes: object) -> None:
    with pytest.raises(ValueError, match="byte-size bound is invalid"):
        _read_bounded_response_body(
            _response(b"{}", None),
            maximum_body_bytes,  # type: ignore[arg-type]
            "probe",
        )
