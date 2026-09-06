"""Check the route-derived SDK inventory, authentication scope, and drift behavior."""

from __future__ import annotations

import csv
import shutil
import subprocess
from pathlib import Path

import pytest

from scripts import sdk_operation_inventory as inventory


@pytest.fixture(scope="module")
def generated() -> bytes:
    """Execute the real std-only Rust catalog exporter once for this suite."""
    return inventory.generate(inventory.ROOT)


def test_inventory_is_exact_and_preserves_every_explicit_operation(generated: bytes) -> None:
    assert generated == (inventory.ROOT / inventory.INVENTORY).read_bytes()
    rows = list(csv.DictReader(generated.decode().splitlines()[1:], delimiter="\t"))
    ids = [row["route_id"] for row in rows]
    assert ids == sorted(set(ids))
    assert all(row["method"] not in {"HEAD", "OPTIONS"} for row in rows)
    operations = {row["route_id"]: row for row in rows}
    submission = operations["pipeline.transaction.submit"]
    assert (submission["method"], submission["authentication"], submission["admission"]) == (
        "POST", "canonical_signed_body", "authenticated_account",
    )
    assert submission["effect"] == "mutation"
    operator = operations["operator.configuration.read"]
    assert operator["authentication"] == "operator_signature"
    assert operator["admission"] == "operator"
    assert operator["private_no_store"] == "true"
    stream = operations["blocks.stream_websocket"]
    assert stream["transport"] == "websocket"
    assert stream["admission"] == "authenticated_account"
    assert stream["feature_gate"] == "feature(app_api)"
    # SDK projection flags describe the current catalog. Retain routes excluded
    # from that projection so migration can account for existing handwritten SDK calls.
    assert operations["diagnostic.status"]["sdk"] == "false"
    assert operator["sdk"] == "false"
    assert stream["sdk"] == "false"


@pytest.mark.parametrize("mutation", ["missing", "extra", "changed"])
def test_default_check_rejects_drift_without_rewriting(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, generated: bytes, mutation: str,
) -> None:
    path = tmp_path / inventory.INVENTORY
    path.parent.mkdir()
    modified = {
        "missing": b"\n".join(generated.splitlines()[:-1]) + b"\n",
        "extra": generated + b"invented.operation\n",
        "changed": generated.replace(b"operator_signature", b"unauthenticated", 1),
    }[mutation]
    path.write_bytes(modified)
    monkeypatch.setattr(inventory, "ROOT", tmp_path)
    monkeypatch.setattr(inventory, "generate", lambda _root: generated)
    assert inventory.main([]) == 1
    assert path.read_bytes() == modified


def test_explicit_refresh_writes_the_complete_inventory(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, generated: bytes,
) -> None:
    path = tmp_path / inventory.INVENTORY
    path.parent.mkdir()
    path.write_bytes(b"stale\n")
    monkeypatch.setattr(inventory, "ROOT", tmp_path)
    monkeypatch.setattr(inventory, "generate", lambda _root: generated)
    assert inventory.main(["--write"]) == 0
    assert path.read_bytes() == generated
    assert inventory.main([]) == 0


def test_missing_inventory_is_not_created_by_check(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, generated: bytes,
) -> None:
    monkeypatch.setattr(inventory, "ROOT", tmp_path)
    monkeypatch.setattr(inventory, "generate", lambda _root: generated)
    assert inventory.main([]) == 2
    assert not (tmp_path / inventory.INVENTORY).exists()


def test_compiler_errors_fail_closed(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(inventory, "ROOT", tmp_path)
    assert inventory.main(["--write"]) == 2
    assert not (tmp_path / inventory.INVENTORY).exists()


def test_invalid_catalog_cannot_produce_an_inventory(tmp_path: Path) -> None:
    exporter = Path("scripts/sdk_operation_inventory.rs")
    catalog = Path("crates/iroha_torii_shared/src/route_catalog.rs")
    for relative in (exporter, catalog):
        target = tmp_path / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(inventory.ROOT / relative, target)
    shutil.copytree(inventory.ROOT / catalog.with_suffix(""), tmp_path / catalog.with_suffix(""))
    path = tmp_path / catalog
    source = path.read_text()
    assert source.count("    pipeline::ROUTES,") == 1
    path.write_text(source.replace("    pipeline::ROUTES,", "    pipeline::ROUTES,\n    pipeline::ROUTES,"))
    with pytest.raises(subprocess.CalledProcessError) as failure:
        inventory.generate(tmp_path)
    assert b"invalid canonical route catalog" in failure.value.stderr
