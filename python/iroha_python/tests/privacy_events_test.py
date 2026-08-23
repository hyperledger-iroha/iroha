"""SoraNet privacy-event feed schema tests."""

from __future__ import annotations

import pytest

from iroha_python.privacy import (
    PrivacyEventGarAbuseCategory,
    PrivacyEventKind,
    parse_privacy_event,
)


def _gar_event(category_hash: object) -> dict[str, object]:
    return {
        "timestamp_unix": 1_723_456_789,
        "mode": "exit",
        "kind": "GarAbuseCategory",
        "payload": {"category_hash": category_hash},
    }


def test_gar_event_parses_only_the_fixed_hash() -> None:
    event = parse_privacy_event(_gar_event([0xA5] * 8))

    assert event.kind is PrivacyEventKind.GAR_ABUSE_CATEGORY
    assert event.payload == PrivacyEventGarAbuseCategory(category_hash=bytes([0xA5] * 8))


@pytest.mark.parametrize(
    "category_hash",
    (
        [1] * 7,
        [1] * 9,
        [1] * 7 + [-1],
        [1] * 7 + [256],
        [1] * 7 + [True],
        "0101010101010101",
    ),
)
def test_gar_event_rejects_noncanonical_hashes(category_hash: object) -> None:
    with pytest.raises(TypeError, match="category_hash"):
        parse_privacy_event(_gar_event(category_hash))


def test_gar_event_rejects_retired_raw_label_payload() -> None:
    event = _gar_event(None)
    event["payload"] = {"label": "policy.secret"}

    with pytest.raises(TypeError, match="category_hash"):
        parse_privacy_event(event)
