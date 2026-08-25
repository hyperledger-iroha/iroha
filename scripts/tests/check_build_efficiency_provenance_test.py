"""Tests for the fail-closed build-efficiency Git provenance guard."""

from __future__ import annotations

import base64
import copy
import hashlib
import importlib.util
import json
import sys
from pathlib import Path
from typing import Any

import pytest


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_build_efficiency_provenance.py"
MANIFEST_PATH = Path(__file__).resolve().parents[2] / "ci" / "build_efficiency_provenance.json"
SPEC = importlib.util.spec_from_file_location("check_build_efficiency_provenance", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def load_manifest() -> dict[str, Any]:
    """Load an independent copy of the checked-in fixture."""
    return json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))


def armor_signature(fingerprint: str, key_id: str) -> str:
    """Build a small structurally valid v4 OpenPGP signature packet."""
    hashed = bytes([22, 33, 4]) + bytes.fromhex(fingerprint)
    unhashed = bytes([9, 16]) + bytes.fromhex(key_id)
    body = (
        bytes([4, 0, 22, 10])
        + len(hashed).to_bytes(2, "big")
        + hashed
        + len(unhashed).to_bytes(2, "big")
        + unhashed
        + b"\x00\x00"
    )
    packet = bytes([0x88, len(body)]) + body
    checksum = MODULE._crc24(packet).to_bytes(3, "big")
    return "\n".join(
        [
            "-----BEGIN PGP SIGNATURE-----",
            "",
            base64.b64encode(packet).decode("ascii"),
            "=" + base64.b64encode(checksum).decode("ascii"),
            "-----END PGP SIGNATURE-----",
        ]
    )


def commit_bytes(record: dict[str, Any], signature: str | None = None) -> bytes:
    """Build commit bytes whose relevant headers match a manifest record."""
    lines = [f"tree {record['tree']}"]
    lines.extend(f"parent {parent}" for parent in record["parents"])
    if signature is not None:
        signature_lines = signature.splitlines()
        lines.append(f"gpgsig {signature_lines[0]}")
        lines.extend(f" {line}" for line in signature_lines[1:])
    lines.extend(
        [
            "author Fixture <fixture@example.invalid> 0 +0000",
            "committer Fixture <fixture@example.invalid> 0 +0000",
            "",
            "fixture",
            "",
        ]
    )
    return "\n".join(lines).encode("utf-8")


def rendered_source_budget(manifest: dict[str, Any]) -> bytes:
    """Render the exact source-budget fixture consumed by the guard."""
    contract = manifest["source_budget"]
    return json.dumps(
        {
            "schema_version": contract["schema_version"],
            "limits": {"production": 5_000, "test": 3_000},
            "exceptions": {"crates/large.rs": 10_000},
            "aggregate_rust": {
                "baseline": contract["baseline"],
                "ceiling": contract["ceiling"],
            },
            "excluded_prefixes": contract["excluded_prefixes"],
        }
    ).encode("utf-8")


class FakeObjectStore:
    """A deterministic in-memory object layer for mutation tests."""

    def __init__(self, manifest: dict[str, Any]) -> None:
        self.manifest = copy.deepcopy(manifest)
        self.format = "sha1"
        self.head_oid = manifest["lineage"]["signed_lock_anchor"]["commit"]
        self.false_ancestry: tuple[str, str] | None = None
        signature = manifest["signed_lock_anchor"]["signature"]
        armor = armor_signature(signature["issuer_fingerprint"], signature["issuer_key_id"])
        self.commits: dict[str, bytes] = {}
        for role, record in manifest["lineage"].items():
            self.commits[record["commit"]] = commit_bytes(
                record, armor if role == "signed_lock_anchor" else None
            )
            for parent in record["parents"]:
                self.commits.setdefault(parent, b"fixture parent")
        self.entries: dict[tuple[str, str], Any] = {}
        donor = manifest["lineage"]["donor"]["commit"]
        integration = manifest["lineage"]["protected_integration"]["commit"]
        for selected in manifest["selected_paths"]:
            for commit, state_name in (
                (donor, "donor"),
                (integration, "protected_integration"),
            ):
                state = selected[state_name]
                if state is not None:
                    self.entries[(commit, selected["path"])] = MODULE.TreeEntry(
                        mode=state["mode"],
                        object_type="blob",
                        oid=state["blob"],
                        path=selected["path"],
                    )
        lock = manifest["signed_lock_anchor"]["cargo_lock"]
        lock_entry = MODULE.TreeEntry(
            mode=lock["mode"],
            object_type="blob",
            oid=lock["blob"],
            path=lock["path"],
        )
        anchor = manifest["lineage"]["signed_lock_anchor"]["commit"]
        self.entries[(anchor, lock["path"])] = lock_entry
        self.lock_bytes = b"fixture Cargo.lock\n"
        budget = manifest["signed_lock_anchor"]["source_file_budget"]
        self.entries[(anchor, budget["path"])] = MODULE.TreeEntry(
            mode=budget["mode"],
            object_type="blob",
            oid=budget["blob"],
            path=budget["path"],
        )
        self.source_budget_bytes = rendered_source_budget(manifest)

    def object_format(self) -> str:
        return self.format

    def head(self) -> str:
        return self.head_oid

    def object_bytes(self, oid: str, expected_type: str) -> bytes:
        if expected_type == "commit":
            return self.commits.get(oid, b"fixture commit")
        if expected_type == "tree":
            return b""
        if expected_type == "blob":
            lock_oid = self.manifest["signed_lock_anchor"]["cargo_lock"]["blob"]
            budget_oid = self.manifest["signed_lock_anchor"]["source_file_budget"][
                "blob"
            ]
            if oid == lock_oid:
                return self.lock_bytes
            if oid == budget_oid:
                return self.source_budget_bytes
            return b"fixture blob\n"
        raise AssertionError(expected_type)

    def tree_entries(self, _commit: str) -> list[Any]:
        raise AssertionError("historical counts are supplied by the fixture")

    def blob_bytes_many(self, _oids: list[str]) -> dict[str, bytes]:
        raise AssertionError("historical counts are supplied by the fixture")

    def tree_entry(self, commit: str, path: str) -> Any:
        return self.entries.get((commit, path))

    def is_ancestor(self, ancestor: str, descendant: str) -> bool:
        return self.false_ancestry != (ancestor, descendant)


def write_source_budget(root: Path, manifest: dict[str, Any]) -> None:
    """Write the exact current source-budget fixture consumed by the guard."""
    path = root / manifest["source_budget"]["path"]
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(rendered_source_budget(manifest))


def prepare_valid_fixture(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> tuple[dict[str, Any], FakeObjectStore]:
    """Prepare a valid manifest and mocked object graph."""
    manifest = load_manifest()
    store = FakeObjectStore(manifest)
    lock = manifest["signed_lock_anchor"]["cargo_lock"]
    lock["bytes"] = len(store.lock_bytes)
    lock["sha256"] = hashlib.sha256(store.lock_bytes).hexdigest()
    budget = manifest["signed_lock_anchor"]["source_file_budget"]
    budget["bytes"] = len(store.source_budget_bytes)
    budget["sha256"] = hashlib.sha256(store.source_budget_bytes).hexdigest()
    write_source_budget(tmp_path, manifest)
    monkeypatch.setattr(MODULE, "verify_object_id", lambda *_args: None)

    counts = {
        record["commit"]: (record["rust"]["paths"], record["rust"]["lines"])
        for record in manifest["lineage"].values()
    }
    monkeypatch.setattr(
        MODULE,
        "historical_rust_count",
        lambda _store, commit, _prefixes, _cache: counts[commit],
    )
    return manifest, store


def test_checked_in_manifest_has_the_complete_strict_schema() -> None:
    manifest = MODULE.strict_json_file(MANIFEST_PATH, "manifest")

    MODULE.validate_manifest_schema(manifest)

    assert len(manifest["selected_paths"]) == 14
    assert manifest["signed_lock_anchor"]["signature"]["cryptographic_signer_authentication"] is False


def test_valid_mocked_object_graph_passes(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    manifest, store = prepare_valid_fixture(tmp_path, monkeypatch)

    report = MODULE.validate_provenance(tmp_path, manifest, store)

    assert report == {
        "roles": 5,
        "selected_paths": 14,
        "historical_rust_paths": 19_654,
        "cargo_lock_bytes": len(store.lock_bytes),
        "source_budget_bytes": len(store.source_budget_bytes),
    }


def test_validate_provenance_uses_supplied_head_snapshot(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    manifest, store = prepare_valid_fixture(tmp_path, monkeypatch)
    expected_head = manifest["lineage"]["signed_lock_anchor"]["commit"]
    monkeypatch.setattr(
        store,
        "head",
        lambda: pytest.fail("explicit HEAD snapshot must not be re-resolved"),
    )

    report = MODULE.validate_provenance(
        tmp_path,
        manifest,
        store,
        head_commit=expected_head,
    )

    assert report["roles"] == 5


def test_main_rejects_head_movement(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    manifest, delegate = prepare_valid_fixture(tmp_path, monkeypatch)
    manifest_path = tmp_path / "manifest.json"
    manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
    initial_head = manifest["lineage"]["signed_lock_anchor"]["commit"]
    moved_head = "f" * 40

    class MovingHeadStore:
        def __init__(self, _root: Path) -> None:
            self.head_calls = 0

        def __getattr__(self, name: str) -> Any:
            return getattr(delegate, name)

        def head(self) -> str:
            self.head_calls += 1
            return initial_head if self.head_calls == 1 else moved_head

    moving_store = MovingHeadStore(tmp_path)
    monkeypatch.setattr(MODULE, "GitObjectStore", lambda _root: moving_store)
    monkeypatch.setattr(
        MODULE,
        "parse_args",
        lambda: MODULE.argparse.Namespace(
            root=tmp_path,
            manifest=Path("manifest.json"),
        ),
    )

    assert MODULE.main() == 2
    assert moving_store.head_calls == 2
    assert "HEAD changed during provenance validation" in capsys.readouterr().err


@pytest.mark.parametrize(
    "mutation",
    [
        lambda payload: payload.update({"unexpected": True}),
        lambda payload: payload["lineage"]["donor"].update({"unexpected": True}),
        lambda payload: payload["lineage"]["donor"].update({"commit": "0" * 40}),
        lambda payload: payload["selected_paths"].pop(),
        lambda payload: payload["selected_paths"].reverse(),
        lambda payload: payload["selected_paths"][0].update({"path": "../escape"}),
        lambda payload: payload["selected_paths"][0].update({"origin": "protected_integration"}),
        lambda payload: payload["selected_paths"][0]["donor"].update({"mode": "120000"}),
        lambda payload: payload["signed_lock_anchor"]["signature"].update(
            {"cryptographic_signer_authentication": True}
        ),
        lambda payload: payload["signed_lock_anchor"]["source_file_budget"].update(
            {"path": "ci/other-budget.json"}
        ),
        lambda payload: payload["signed_lock_anchor"]["source_file_budget"].update(
            {"mode": "100755"}
        ),
        lambda payload: payload["signed_lock_anchor"]["source_file_budget"].update(
            {"bytes": 0}
        ),
        lambda payload: payload["signed_lock_anchor"]["source_file_budget"].update(
            {"sha256": "0" * 63}
        ),
        lambda payload: payload["source_budget"].update({"baseline": 5_067_262}),
        lambda payload: payload["source_budget"].update({"ceiling": 4_540_001}),
        lambda payload: payload["source_budget"]["excluded_prefixes"].reverse(),
    ],
)
def test_schema_mutations_fail_closed(mutation: Any) -> None:
    manifest = load_manifest()
    mutation(manifest)

    with pytest.raises(MODULE.ProvenanceError):
        MODULE.validate_manifest_schema(manifest)


def test_duplicate_json_keys_are_rejected() -> None:
    with pytest.raises(MODULE.ProvenanceError, match="duplicate JSON key"):
        MODULE.strict_json_loads('{"schema_version": 1, "schema_version": 1}', "fixture")


def test_sanitized_git_environment_drops_injected_git_controls() -> None:
    sanitized = MODULE.sanitized_git_environment(
        {
            "PATH": "/usr/bin",
            "GIT_DIR": "/attacker/repository",
            "GIT_WORK_TREE": "/attacker/worktree",
            "GIT_CONFIG_COUNT": "1",
            "GIT_CONFIG_KEY_0": "core.fsmonitor",
            "GIT_CONFIG_VALUE_0": "attacker",
            "XDG_CONFIG_HOME": "/attacker/config",
        }
    )

    assert sanitized["PATH"] == "/usr/bin"
    assert sanitized["GIT_NO_LAZY_FETCH"] == "1"
    assert sanitized["GIT_NO_REPLACE_OBJECTS"] == "1"
    assert sanitized["GIT_CONFIG_NOSYSTEM"] == "1"
    assert "GIT_DIR" not in sanitized
    assert "GIT_WORK_TREE" not in sanitized
    assert "GIT_CONFIG_COUNT" not in sanitized
    assert "GIT_CONFIG_KEY_0" not in sanitized
    assert "GIT_CONFIG_VALUE_0" not in sanitized
    assert "XDG_CONFIG_HOME" not in sanitized


def test_git_object_hash_verification_rejects_changed_bytes() -> None:
    payload = b"pinned bytes"
    oid = MODULE.git_object_id("blob", payload)

    MODULE.verify_object_id(oid, "blob", payload)
    with pytest.raises(MODULE.ProvenanceError, match="unexpected id"):
        MODULE.verify_object_id(oid, "blob", payload + b"!")


def test_historical_rust_count_matches_source_budget_semantics() -> None:
    first = b"one\n\nthree\n"
    second = b"four\r\nfive"
    ignored = b"not governed\n"
    first_oid = MODULE.git_object_id("blob", first)
    second_oid = MODULE.git_object_id("blob", second)
    ignored_oid = MODULE.git_object_id("blob", ignored)

    class RustStore:
        def tree_entries(self, _commit: str) -> list[Any]:
            return [
                MODULE.TreeEntry("100644", "blob", first_oid, "crates/a.rs"),
                MODULE.TreeEntry("100755", "blob", second_oid, "scripts/b.rs"),
                MODULE.TreeEntry("100644", "blob", ignored_oid, "scripts/c.RS"),
                MODULE.TreeEntry("100644", "blob", ignored_oid, "vendor/c.rs"),
                MODULE.TreeEntry("100644", "blob", ignored_oid, "scripts/helper.py"),
            ]

        def blob_bytes_many(self, oids: list[str]) -> dict[str, bytes]:
            blobs = {first_oid: first, second_oid: second, ignored_oid: ignored}
            return {oid: blobs[oid] for oid in dict.fromkeys(oids)}

    assert MODULE.historical_rust_count(
        RustStore(), "0" * 40, ("vendor/",), {}
    ) == (3, 6)


def test_openpgp_issuer_is_structural_only() -> None:
    fingerprint = "9d1c8bfa5a0c1fef5a8b1e5f552c2d0fd7c40beb"
    key_id = fingerprint[-16:]
    armor = armor_signature(fingerprint, key_id)

    MODULE.verify_openpgp_issuer_structure(armor, fingerprint, key_id)

    different_fingerprint = "0" * 24 + key_id
    with pytest.raises(MODULE.ProvenanceError, match="fingerprint"):
        MODULE.verify_openpgp_issuer_structure(armor, different_fingerprint, key_id)


def test_openpgp_crc_mutation_is_rejected() -> None:
    fingerprint = "9d1c8bfa5a0c1fef5a8b1e5f552c2d0fd7c40beb"
    armor = armor_signature(fingerprint, fingerprint[-16:])
    lines = armor.splitlines()
    lines[-2] = "=AAAA"

    with pytest.raises(MODULE.ProvenanceError, match="CRC-24"):
        MODULE.verify_openpgp_issuer_structure(
            "\n".join(lines), fingerprint, fingerprint[-16:]
        )


def test_commit_tree_mutation_is_rejected(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    manifest, store = prepare_valid_fixture(tmp_path, monkeypatch)
    manifest["lineage"]["donor"]["tree"] = "f" * 40

    with pytest.raises(MODULE.ProvenanceError, match="commit records"):
        MODULE.validate_provenance(tmp_path, manifest, store)


def test_ordered_parent_mutation_is_rejected(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    manifest, store = prepare_valid_fixture(tmp_path, monkeypatch)
    manifest["lineage"]["donor"]["parents"].reverse()

    with pytest.raises(MODULE.ProvenanceError, match="ordered parents"):
        MODULE.validate_provenance(tmp_path, manifest, store)


def test_ancestry_mutation_is_rejected(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    manifest, store = prepare_valid_fixture(tmp_path, monkeypatch)
    donor = manifest["lineage"]["donor"]["commit"]
    integration = manifest["lineage"]["protected_integration"]["commit"]
    store.false_ancestry = (donor, integration)

    with pytest.raises(MODULE.ProvenanceError, match="donor -> protected_integration"):
        MODULE.validate_provenance(tmp_path, manifest, store)


def test_historical_rust_count_mutation_is_rejected(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    manifest, store = prepare_valid_fixture(tmp_path, monkeypatch)
    expected = manifest["lineage"]["implementation_origin"]["rust"]

    def wrong_count(_store: Any, commit: str, _prefixes: Any, _cache: Any) -> tuple[int, int]:
        record = next(
            value for value in manifest["lineage"].values() if value["commit"] == commit
        )
        if commit == manifest["lineage"]["implementation_origin"]["commit"]:
            return expected["paths"], expected["lines"] + 1
        return record["rust"]["paths"], record["rust"]["lines"]

    monkeypatch.setattr(MODULE, "historical_rust_count", wrong_count)
    with pytest.raises(MODULE.ProvenanceError, match="observed"):
        MODULE.validate_provenance(tmp_path, manifest, store)


def test_selected_blob_state_mutation_is_rejected(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    manifest, store = prepare_valid_fixture(tmp_path, monkeypatch)
    manifest["selected_paths"][0]["protected_integration"]["blob"] = "f" * 40

    with pytest.raises(MODULE.ProvenanceError, match="state for"):
        MODULE.validate_provenance(tmp_path, manifest, store)


def test_donor_absence_mutation_is_rejected(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    manifest, store = prepare_valid_fixture(tmp_path, monkeypatch)
    selected = next(
        entry
        for entry in manifest["selected_paths"]
        if entry["origin"] == "protected_integration"
    )
    donor = manifest["lineage"]["donor"]["commit"]
    store.entries[(donor, selected["path"])] = MODULE.TreeEntry(
        mode="100644",
        object_type="blob",
        oid="f" * 40,
        path=selected["path"],
    )

    with pytest.raises(MODULE.ProvenanceError, match="unexpectedly contains"):
        MODULE.validate_provenance(tmp_path, manifest, store)


def test_object_format_mutation_is_rejected(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    manifest, store = prepare_valid_fixture(tmp_path, monkeypatch)
    store.format = "sha256"

    with pytest.raises(MODULE.ProvenanceError, match="object format"):
        MODULE.validate_provenance(tmp_path, manifest, store)


@pytest.mark.parametrize("field", ["bytes", "sha256"])
def test_cargo_lock_content_mutations_are_rejected(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, field: str
) -> None:
    manifest, store = prepare_valid_fixture(tmp_path, monkeypatch)
    lock = manifest["signed_lock_anchor"]["cargo_lock"]
    lock[field] = lock[field] + 1 if field == "bytes" else "0" * 64

    with pytest.raises(MODULE.ProvenanceError, match="Cargo.lock"):
        MODULE.validate_provenance(tmp_path, manifest, store)


@pytest.mark.parametrize("field", ["bytes", "sha256"])
def test_source_budget_anchor_content_mutations_are_rejected(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, field: str
) -> None:
    manifest, store = prepare_valid_fixture(tmp_path, monkeypatch)
    budget = manifest["signed_lock_anchor"]["source_file_budget"]
    budget[field] = budget[field] + 1 if field == "bytes" else "0" * 64

    with pytest.raises(MODULE.ProvenanceError, match="source_file_budget.json"):
        MODULE.validate_provenance(tmp_path, manifest, store)


def test_source_budget_anchor_tree_state_mutation_is_rejected(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    manifest, store = prepare_valid_fixture(tmp_path, monkeypatch)
    anchor = manifest["lineage"]["signed_lock_anchor"]["commit"]
    budget = manifest["signed_lock_anchor"]["source_file_budget"]
    store.entries[(anchor, budget["path"])] = MODULE.TreeEntry(
        mode=budget["mode"],
        object_type="blob",
        oid="f" * 40,
        path=budget["path"],
    )

    with pytest.raises(MODULE.ProvenanceError, match="state for"):
        MODULE.validate_provenance(tmp_path, manifest, store)


def test_source_budget_head_tree_state_mutation_is_rejected(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    manifest, store = prepare_valid_fixture(tmp_path, monkeypatch)
    head = "e" * 40
    store.head_oid = head
    store.commits[head] = b"fixture head commit"
    lock = manifest["signed_lock_anchor"]["cargo_lock"]
    store.entries[(head, lock["path"])] = MODULE.TreeEntry(
        mode=lock["mode"], object_type="blob", oid=lock["blob"], path=lock["path"]
    )
    budget = manifest["signed_lock_anchor"]["source_file_budget"]
    store.entries[(head, budget["path"])] = MODULE.TreeEntry(
        mode=budget["mode"],
        object_type="blob",
        oid="f" * 40,
        path=budget["path"],
    )

    with pytest.raises(MODULE.ProvenanceError, match="HEAD state"):
        MODULE.validate_provenance(tmp_path, manifest, store)


@pytest.mark.parametrize("field", ["baseline", "ceiling"])
def test_current_source_budget_mutations_are_rejected(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    field: str,
) -> None:
    manifest, store = prepare_valid_fixture(tmp_path, monkeypatch)
    budget_path = tmp_path / manifest["source_budget"]["path"]
    budget = json.loads(budget_path.read_text(encoding="utf-8"))
    budget["aggregate_rust"][field] += 1
    budget_path.write_text(json.dumps(budget), encoding="utf-8")

    with pytest.raises(MODULE.ProvenanceError, match=f"current source budget {field}"):
        MODULE.validate_provenance(tmp_path, manifest, store)


@pytest.mark.parametrize(
    "mutation",
    [
        lambda payload: payload["limits"].update({"production": 6_000}),
        lambda payload: payload["exceptions"].update({"crates/large.rs": 11_000}),
        lambda payload: payload["exceptions"].update({"crates/new.rs": 9_000}),
    ],
)
def test_current_source_budget_policy_changes_require_signed_anchor_retarget(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    mutation: Any,
) -> None:
    manifest, store = prepare_valid_fixture(tmp_path, monkeypatch)
    budget_path = tmp_path / manifest["source_budget"]["path"]
    payload = json.loads(budget_path.read_text(encoding="utf-8"))
    mutation(payload)
    budget_path.write_text(json.dumps(payload), encoding="utf-8")

    with pytest.raises(MODULE.ProvenanceError, match="signed lock anchor"):
        MODULE.validate_provenance(tmp_path, manifest, store)
