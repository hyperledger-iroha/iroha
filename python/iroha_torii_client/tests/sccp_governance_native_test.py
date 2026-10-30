"""SCCP release acceptance requires the actual canonical native account codec."""

import copy

import pytest
from sccp_governance_test import fixture, read, subject, thaw


def test_release_accepts_exact_amount_memo_and_canonical_recipient():
    value = fixture("release_stranded")
    body = value["payload"]["proposal"]["actions"][0]["payload"]
    for amount in ("1", str((1 << 128) - 1)):
        for memo in ("", "a" * 256, "é" * 128, "🦊" * 64, " \n\x00 "):
            body.update(amount=amount, memo=memo)
            assert thaw(read(value)) == value["payload"]["proposal"]
    for recipient in ("alice@wonderland", "", "invalid", " " + body["recipient"]):
        invalid = copy.deepcopy(value)
        invalid["payload"]["proposal"]["actions"][0]["payload"]["recipient"] = recipient
        with pytest.raises(TypeError):
            read(invalid)
    for field in list(body) + ["unexpected"]:
        invalid = copy.deepcopy(value)
        target = invalid["payload"]["proposal"]["actions"][0]["payload"]
        if field == "unexpected":
            target[field] = None
        else:
            del target[field]
        with pytest.raises(TypeError):
            read(invalid)
    value["payload"]["proposal"]["base_revisions"][0]["subject"] = subject("route_control")
    with pytest.raises(TypeError, match="subjects"):
        read(value)
