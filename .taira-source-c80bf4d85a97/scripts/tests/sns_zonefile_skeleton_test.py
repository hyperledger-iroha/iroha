"""Unit tests for ``scripts/sns_zonefile_skeleton.py`` (roadmap SN-7 helper)."""

from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path
import importlib.util
import sys

import pytest

MODULE_PATH = Path(__file__).resolve().parents[1] / "sns_zonefile_skeleton.py"
SPEC = importlib.util.spec_from_file_location("sns_zonefile_skeleton", MODULE_PATH)
SNS_ZONEFILE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = SNS_ZONEFILE
SPEC.loader.exec_module(SNS_ZONEFILE)  # type: ignore[attr-defined]
ZonefileOptions = SNS_ZONEFILE.ZonefileOptions
build_zonefile = SNS_ZONEFILE.build_zonefile
main = SNS_ZONEFILE.main


def _sample_plan() -> dict[str, object]:
    return {
        "cutover_window": "2026-02-24T12:00Z/2026-02-24T12:30Z",
        "dns": {"hostname": "docs.sora.name", "zone": "sora.name"},
        "alias": {
            "namespace": "docs",
            "name": "portal",
            "manifest_digest_hex": "abbcdd",
        },
        "manifest": {"blake3_hex": "1234abcd"},
        "car": {"car_digest_hex": "4321fedc"},
        "release": {"tag": "portal-v1.2.3"},
        "change_ticket": "OPS-4242",
        "gateway_binding": {
            "alias": "docs.sora",
            "hostname": "docs.gw.sora.id",
            "content_cid": "bafy-docs-cid",
            "proof_status": "attested",
            "headers": {
                "Cache-Control": "max-age=600",
                "Sora-Route-Binding": "public",
                "Sora-Proof": "sig-abcd",
                "Sora-GAR-Digest": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            },
        },
    }


def test_build_zonefile_includes_metadata_and_gateway_records() -> None:
    plan = _sample_plan()
    options = ZonefileOptions(
        ttl=600,
        ipv4=("192.0.2.10",),
        ipv6=("2001:db8::10",),
        cname_target="docs.gateway.sora.net",
        extra_txt=("CustomKey=CustomValue",),
        spki_pins=("abc123==", "def456=="),
        source_path="/tmp/cutover.json",
        now=datetime(2026, 2, 24, 1, 2, 3, tzinfo=timezone.utc),
    )

    skeleton = build_zonefile(plan, options)
    records = skeleton["static_zone"]["records"]
    txt_values = {
        value
        for record in records
        if record["type"] == "TXT"
        for value in record["text"]
    }

    assert skeleton["schema_version"] == 1
    assert skeleton["dns"]["hostname"] == "docs.sora.name"
    assert skeleton["source_cutover_plan"] == "/tmp/cutover.json"
    assert {"type": "A", "ttl": 600, "address": "192.0.2.10"} in records
    assert {"type": "AAAA", "ttl": 600, "address": "2001:db8::10"} in records
    assert {
        "type": "CNAME",
        "ttl": 600,
        "target": "docs.gateway.sora.net",
    } in records

    assert "Sora-Alias=docs:portal" in txt_values
    assert "AliasManifest=abbcdd" in txt_values
    assert "ManifestDigest=1234abcd" in txt_values
    assert "CarDigest=4321fedc" in txt_values
    assert "ReleaseTag=portal-v1.2.3" in txt_values
    assert "ChangeTicket=OPS-4242" in txt_values
    assert "DNSZone=sora.name" in txt_values
    assert "Sora-Name=docs.sora" in txt_values
    assert "Sora-Content-CID=bafy-docs-cid" in txt_values
    assert "Sora-Proof-Status=attested" in txt_values
    assert "Sora-Route-Binding=public" in txt_values
    assert "Cache-Control=max-age=600" in txt_values
    assert "CustomKey=CustomValue" in txt_values
    assert "Sora-SPKI=sha256/abc123==" in txt_values
    assert "Sora-SPKI=sha256/def456==" in txt_values
    assert "Sora-Proof=sig-abcd" in txt_values
    zonefile_meta = skeleton["zonefile"]
    assert zonefile_meta["name"] == "docs.sora.name"
    assert zonefile_meta["ttl"] == 600
    assert zonefile_meta["version"] == "portal-v1.2.3"
    assert zonefile_meta["effective_at"] == "2026-02-24T12:00:00Z"
    assert zonefile_meta["cid"] == "bafy-docs-cid"
    assert zonefile_meta["gar_digest"] == (
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    )
    assert zonefile_meta["proof"] == "sig-abcd"


def test_build_zonefile_includes_freeze_metadata() -> None:
    plan = _sample_plan()
    options = ZonefileOptions(
        ttl=600,
        ipv4=("192.0.2.10",),
        ipv6=(),
        cname_target=None,
        extra_txt=(),
        spki_pins=(),
        source_path="/tmp/cutover.json",
        now=datetime(2026, 2, 24, 1, 2, 3, tzinfo=timezone.utc),
        freeze_state="soft",
        freeze_ticket="SNS-DF-42",
        freeze_expires_at="2026-03-01T00:00:00Z",
        freeze_notes=("guardians reviewing",),
    )

    skeleton = build_zonefile(plan, options)
    assert skeleton["freeze"] == {
        "state": "soft",
        "ticket": "SNS-DF-42",
        "expires_at": "2026-03-01T00:00:00Z",
        "notes": ["guardians reviewing"],
    }

    txt_values = {
        value
        for record in skeleton["static_zone"]["records"]
        if record["type"] == "TXT"
        for value in record["text"]
    }
    assert "Sora-Freeze-State=soft" in txt_values
    assert "Sora-Freeze-Ticket=SNS-DF-42" in txt_values
    assert "Sora-Freeze-ExpiresAt=2026-03-01T00:00:00Z" in txt_values
    assert "Sora-Freeze-Note=guardians reviewing" in txt_values


def test_build_zonefile_requires_at_least_one_record() -> None:
    plan = {"dns": {"hostname": "docs.sora.name"}}
    options = ZonefileOptions(
        ttl=300,
        ipv4=(),
        ipv6=(),
        cname_target=None,
        extra_txt=(),
        spki_pins=(),
        source_path="cutover.json",
        now=datetime.now(timezone.utc),
    )

    with pytest.raises(ValueError) as exc:
        build_zonefile(plan, options)
    assert "no DNS records were generated" in str(exc.value)


def test_build_zonefile_requires_gar_digest_when_gateway_present() -> None:
    plan = _sample_plan()
    gateway = plan["gateway_binding"]
    assert isinstance(gateway, dict)
    headers = gateway["headers"]
    assert isinstance(headers, dict)
    headers.pop("Sora-GAR-Digest")
    options = ZonefileOptions(
        ttl=300,
        ipv4=("192.0.2.1",),
        ipv6=(),
        cname_target=None,
        extra_txt=(),
        spki_pins=(),
        source_path="cutover.json",
        now=datetime.now(timezone.utc),
    )
    with pytest.raises(ValueError) as exc:
        build_zonefile(plan, options)
    assert "GAR digest" in str(exc.value)


def test_build_zonefile_accepts_cli_gar_digest_override() -> None:
    plan = _sample_plan()
    gateway = plan["gateway_binding"]
    assert isinstance(gateway, dict)
    headers = gateway["headers"]
    assert isinstance(headers, dict)
    headers.pop("Sora-GAR-Digest")
    override = "a" * 64
    options = ZonefileOptions(
        ttl=600,
        ipv4=("192.0.2.10",),
        ipv6=(),
        cname_target=None,
        extra_txt=(),
        spki_pins=(),
        source_path="/tmp/cutover.json",
        now=datetime(2026, 2, 24, 1, 2, 3, tzinfo=timezone.utc),
        gar_digest=override,
    )
    skeleton = build_zonefile(plan, options)
    assert skeleton["zonefile"]["gar_digest"] == override


def test_main_rejects_freeze_metadata_without_state(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    plan = _sample_plan()
    plan_path = tmp_path / "cutover.json"
    plan_path.write_text(json.dumps(plan), encoding="utf-8")
    out_path = tmp_path / "zone.json"

    exit_code = main(
        [
            "--cutover-plan",
            str(plan_path),
            "--out",
            str(out_path),
            "--freeze-note",
            "appeal pending",
        ]
    )

    captured = capsys.readouterr()
    assert exit_code == 1
    assert "require --freeze-state" in captured.err
    assert not out_path.exists()


def test_main_writes_zonefile_and_resolver_snippet(tmp_path: Path) -> None:
    plan = _sample_plan()
    plan_path = tmp_path / "cutover.json"
    plan_path.write_text(json.dumps(plan), encoding="utf-8")

    out_path = tmp_path / "zone.json"
    snippet_path = tmp_path / "resolver.json"

    exit_code = main(
        [
            "--cutover-plan",
            str(plan_path),
            "--out",
            str(out_path),
            "--resolver-snippet-out",
            str(snippet_path),
            "--ttl",
            "450",
            "--ipv4",
            "198.51.100.4",
            "--freeze-state",
            "hard",
            "--freeze-ticket",
            "SNS-DF-99",
            "--freeze-expires-at",
            "2026-04-01T00:00Z",
            "--freeze-note",
            "appeal pending",
            "--txt",
            "ChangeTicket=OPS-777",
        ]
    )

    assert exit_code == 0
    skeleton = json.loads(out_path.read_text(encoding="utf-8"))
    records = skeleton["static_zone"]["records"]
    assert {"type": "A", "ttl": 450, "address": "198.51.100.4"} in records
    assert skeleton["freeze"]["state"] == "hard"
    assert skeleton["freeze"]["ticket"] == "SNS-DF-99"
    assert skeleton["freeze"]["expires_at"] == "2026-04-01T00:00:00Z"
    assert (
        skeleton["zonefile"]["gar_digest"]
        == "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    )

    resolver_payload = json.loads(snippet_path.read_text(encoding="utf-8"))
    snippet_zone = resolver_payload["static_zones"][0]
    assert snippet_zone["domain"] == skeleton["static_zone"]["domain"]
    assert snippet_zone["freeze"] == skeleton["freeze"]


def test_main_rejects_invalid_freeze_state(tmp_path: Path) -> None:
    plan = _sample_plan()
    plan_path = tmp_path / "cutover.json"
    plan_path.write_text(json.dumps(plan), encoding="utf-8")
    out_path = tmp_path / "zone.json"

    exit_code = main(
        [
            "--cutover-plan",
            str(plan_path),
            "--out",
            str(out_path),
            "--freeze-state",
            "unknown",
        ]
    )
    assert exit_code == 1


def test_main_rejects_invalid_freeze_timestamp(tmp_path: Path) -> None:
    plan = _sample_plan()
    plan_path = tmp_path / "cutover.json"
    plan_path.write_text(json.dumps(plan), encoding="utf-8")
    out_path = tmp_path / "zone.json"

    exit_code = main(
        [
            "--cutover-plan",
            str(plan_path),
            "--out",
            str(out_path),
            "--freeze-expires-at",
            "bad-value",
        ]
    )
    assert exit_code == 1
