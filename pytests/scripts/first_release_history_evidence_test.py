"""Exercise the obsolete-history evidence manifest on synthetic stores."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys

import pytest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "first_release_history_evidence.py"
SPEC = importlib.util.spec_from_file_location("first_release_history_evidence", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
TOOL = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = TOOL
SPEC.loader.exec_module(TOOL)

STORAGE_REPORT = {
    "tip_height": 7,
    "tip_hash": "ab" * 32,
    "snapshot_height": None,
    "prefix_hash_at_snapshot_height": None,
    "snapshot_restore_dry_run": "ok",
    "snapshot_restore_error": None,
}
CONFIG_REPORT = {
    "status": "ready",
    "node_identity": {"genesis_hash": "cd" * 32, "network_id": "hash:" + "CD" * 32 + "#0000"},
    "diagnostic_build": {
        "version": "2.0.0-rc.2.0",
        "source_revision": "1c4f9014bc6f",
        "build_fingerprint": "ef" * 32,
    },
    "protocol_version": 1,
    "wire_schema_hash": "01" * 32,
    "gas_schedule_hash": "02" * 32,
}


@pytest.fixture
def evidence(tmp_path: Path) -> dict[str, Path]:
    """A stopped store, its genesis, the two probe reports and a log."""
    store = tmp_path / "store"
    (store / "blocks" / "canonical").mkdir(parents=True)
    (store / "blocks" / "canonical" / "blocks.data").write_bytes(b"\x01frame-one\x01frame-two")
    (store / "blocks" / "canonical" / "blocks.index").write_bytes(bytes(32))
    (store / "lane_geometry_journal.norito").write_bytes(b"journal")
    (store / ".kura.lock").write_bytes(b"")
    genesis = tmp_path / "genesis.signed.nrt"
    genesis.write_bytes(b"\x01signed genesis")
    storage = tmp_path / "check-storage.json"
    storage.write_text(json.dumps(STORAGE_REPORT), encoding="utf-8")
    config = tmp_path / "check-config.json"
    config.write_text(json.dumps(CONFIG_REPORT), encoding="utf-8")
    log = tmp_path / "node.log"
    log.write_text("halted at height 7\n", encoding="utf-8")
    return {"store": store, "genesis": genesis, "storage": storage, "config": config, "log": log}


def arguments(evidence: dict[str, Path]) -> list[str]:
    """The input arguments shared by `record` and `verify`."""
    return [
        "--store",
        str(evidence["store"]),
        "--genesis",
        str(evidence["genesis"]),
        "--check-storage-report",
        str(evidence["storage"]),
        "--check-config-report",
        str(evidence["config"]),
        "--log",
        str(evidence["log"]),
    ]


def record(evidence: dict[str, Path], output: Path, *extra: str) -> int:
    """Run `record` in-process."""
    return TOOL.main(["record", *arguments(evidence), "--output", str(output), *extra])


def verify(evidence: dict[str, Path], manifest: Path) -> int:
    """Run `verify` in-process."""
    return TOOL.main(["verify", *arguments(evidence), "--manifest", str(manifest)])


def store_bytes(store: Path) -> dict[str, bytes]:
    """Every file of a store with its bytes."""
    return {
        path.relative_to(store).as_posix(): path.read_bytes()
        for path in sorted(store.rglob("*"))
        if path.is_file()
    }


def test_record_lists_every_store_file_with_its_digest(evidence: dict[str, Path], tmp_path: Path) -> None:
    before = store_bytes(evidence["store"])
    output = tmp_path / "evidence.json"
    assert record(evidence, output) == 0
    manifest = json.loads(output.read_text(encoding="utf-8"))
    assert manifest["schema"] == TOOL.SCHEMA == "iroha.obsolete_history_evidence.v1"
    entries = {entry["path"]: entry for entry in manifest["store"]["entries"]}
    assert set(entries) == {
        "blocks/canonical/blocks.data",
        "blocks/canonical/blocks.index",
        "lane_geometry_journal.norito",
    }
    data = before["blocks/canonical/blocks.data"]
    assert entries["blocks/canonical/blocks.data"] == {
        "path": "blocks/canonical/blocks.data",
        "kind": "file",
        "bytes": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
    }
    assert manifest["store"]["file_count"] == 3
    assert manifest["store"]["bytes"] == sum(len(before[path]) for path in entries)
    roles = {entry["role"]: entry for entry in manifest["files"]}
    assert set(roles) == {"genesis", "check_storage_report", "check_config_report", "log"}
    assert roles["genesis"]["sha256"] == hashlib.sha256(b"\x01signed genesis").hexdigest()
    # Reading is the only thing done to the inputs.
    assert store_bytes(evidence["store"]) == before


def test_record_copies_what_the_writing_build_reported(evidence: dict[str, Path], tmp_path: Path) -> None:
    output = tmp_path / "evidence.json"
    assert record(evidence, output) == 0
    manifest = json.loads(output.read_text(encoding="utf-8"))
    assert manifest["source_revision"] == "1c4f9014bc6f"
    assert manifest["build_identity"] == "ef" * 32
    reported = manifest["reported_by_writing_build"]
    assert reported["genesis_hash"] == "cd" * 32
    assert reported["tip_height"] == 7
    assert reported["tip_hash"] == "ab" * 32
    assert reported["gas_schedule_hash"] == "02" * 32


def test_explicit_revision_and_identity_override_the_report(evidence: dict[str, Path], tmp_path: Path) -> None:
    output = tmp_path / "evidence.json"
    assert record(evidence, output, "--source-revision", "feedface", "--build-identity", "local") == 0
    manifest = json.loads(output.read_text(encoding="utf-8"))
    assert (manifest["source_revision"], manifest["build_identity"]) == ("feedface", "local")
    assert verify(evidence, output) == 0


def test_verify_accepts_unchanged_evidence_and_names_what_changed(
    evidence: dict[str, Path], tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    output = tmp_path / "evidence.json"
    assert record(evidence, output) == 0
    assert verify(evidence, output) == 0
    data = evidence["store"] / "blocks" / "canonical" / "blocks.data"
    data.write_bytes(data.read_bytes()[:-1] + b"\x00")
    capsys.readouterr()
    assert verify(evidence, output) == 1
    assert "store file blocks/canonical/blocks.data changed" in capsys.readouterr().err


@pytest.mark.parametrize(
    "change,fragment",
    [
        (lambda e: (e["store"] / "blocks" / "canonical" / "blocks.index").unlink(), "is missing"),
        (lambda e: (e["store"] / "extra.bin").write_bytes(b"x"), "is not in the manifest"),
        (lambda e: e["genesis"].write_bytes(b"other genesis"), "`files` differs"),
        (lambda e: e["log"].write_text("edited\n", encoding="utf-8"), "`files` differs"),
        (
            lambda e: e["storage"].write_text(
                json.dumps({**STORAGE_REPORT, "tip_height": 8}), encoding="utf-8"
            ),
            "`reported_by_writing_build` differs",
        ),
    ],
)
def test_verify_fails_on_any_changed_input(
    evidence: dict[str, Path], tmp_path: Path, capsys: pytest.CaptureFixture[str], change, fragment: str
) -> None:
    output = tmp_path / "evidence.json"
    assert record(evidence, output) == 0
    change(evidence)
    capsys.readouterr()
    assert verify(evidence, output) == 1
    assert fragment in capsys.readouterr().err


def test_lock_file_is_not_history(evidence: dict[str, Path], tmp_path: Path) -> None:
    output = tmp_path / "evidence.json"
    assert record(evidence, output) == 0
    (evidence["store"] / ".kura.lock").write_bytes(b"another process")
    assert verify(evidence, output) == 0


def test_symlinks_are_recorded_and_never_followed(evidence: dict[str, Path], tmp_path: Path) -> None:
    outside = tmp_path / "outside"
    outside.mkdir()
    (outside / "secret").write_bytes(b"not part of the store")
    os.symlink(outside, evidence["store"] / "linked_dir")
    os.symlink(outside / "secret", evidence["store"] / "linked_file")
    output = tmp_path / "evidence.json"
    assert record(evidence, output) == 0
    manifest = json.loads(output.read_text(encoding="utf-8"))
    entries = {entry["path"]: entry for entry in manifest["store"]["entries"]}
    assert entries["linked_dir"] == {"path": "linked_dir", "kind": "symlink", "target": str(outside)}
    assert entries["linked_file"]["kind"] == "symlink"
    assert not any(path.startswith("linked_dir/") for path in entries)
    assert "sha256" not in entries["linked_file"]


@pytest.mark.parametrize(
    "prepare,fragment",
    [
        (lambda e, t: e["config"].write_text(json.dumps({"status": "pending"}), encoding="utf-8"),
         "source revision and build identity"),
        (
            lambda e, t: e["config"].write_text(
                json.dumps({**CONFIG_REPORT, "node_identity": None}), encoding="utf-8"
            ),
            "carries no genesis hash",
        ),
        (lambda e, t: e["storage"].write_text("not json", encoding="utf-8"), "unreadable --check-storage report"),
        (lambda e, t: e["config"].write_text("[]", encoding="utf-8"), "must be a JSON object"),
        (lambda e, t: e["genesis"].unlink(), "genesis must be a regular file"),
        (lambda e, t: [path.unlink() for path in e["store"].rglob("*") if path.is_file()],
         "the store holds no file"),
    ],
)
def test_incomplete_evidence_is_refused(
    evidence: dict[str, Path], tmp_path: Path, capsys: pytest.CaptureFixture[str], prepare, fragment: str
) -> None:
    prepare(evidence, tmp_path)
    output = tmp_path / "evidence.json"
    assert record(evidence, output) == 1
    assert fragment in capsys.readouterr().err
    assert not output.exists()


def test_manifest_is_never_written_into_the_store_or_overwritten(
    evidence: dict[str, Path], tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    inside = evidence["store"] / "blocks" / "evidence.json"
    assert record(evidence, inside) == 1
    assert "outside the store" in capsys.readouterr().err
    assert not inside.exists()
    output = tmp_path / "evidence.json"
    assert record(evidence, output) == 0
    first = output.read_bytes()
    assert record(evidence, output) == 1
    assert "never overwritten" in capsys.readouterr().err
    assert output.read_bytes() == first


def test_tree_digest_binds_paths_sizes_and_digests() -> None:
    entries = [
        {"path": "a", "kind": "file", "bytes": 1, "sha256": "00"},
        {"path": "b", "kind": "symlink", "target": "a"},
    ]
    base = TOOL.tree_digest(entries)
    assert base == TOOL.tree_digest(json.loads(json.dumps(entries)))
    for index, key, value in ((0, "path", "c"), (0, "bytes", 2), (0, "sha256", "01"), (1, "target", "b")):
        changed = json.loads(json.dumps(entries))
        changed[index][key] = value
        assert TOOL.tree_digest(changed) != base


def test_command_line_records_and_verifies(evidence: dict[str, Path], tmp_path: Path) -> None:
    output = tmp_path / "evidence.json"
    recorded = subprocess.run(
        [sys.executable, str(SCRIPT), "record", *arguments(evidence), "--output", str(output)],
        capture_output=True,
        text=True,
        check=False,
    )
    assert recorded.returncode == 0, recorded.stderr
    assert "recorded 3 store files" in recorded.stdout
    verified = subprocess.run(
        [sys.executable, str(SCRIPT), "verify", *arguments(evidence), "--manifest", str(output)],
        capture_output=True,
        text=True,
        check=False,
    )
    assert verified.returncode == 0, verified.stderr
    assert "evidence matches the manifest" in verified.stdout
