"""Unit tests for scripts/swift_fixture_provenance.py."""

from __future__ import annotations

import importlib.util
import sys
import json
import tempfile
from pathlib import Path
from typing import Any
from unittest import TestCase

MODULE_PATH = Path(__file__).resolve().parents[1] / "swift_fixture_provenance.py"
SPEC = importlib.util.spec_from_file_location("swift_fixture_provenance", MODULE_PATH)
PROV = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules["swift_fixture_provenance"] = PROV
SPEC.loader.exec_module(PROV)  # type: ignore[attr-defined]


def write_file(directory: Path, relative: str, content: str) -> Path:
    path = directory / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    return path


class SwiftFixtureProvenanceTests(TestCase):
    def test_tree_digest_is_deterministic(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            write_file(root, "a/norito1.norito", "alpha")
            write_file(root, "b/norito2.norito", "beta")

            first = PROV.tree_digest(root)
            # Rewrite files (order/mtime may change) but the digest should remain stable.
            (root / "a/norito1.norito").unlink()
            write_file(root, "a/norito1.norito", "alpha")
            second = PROV.tree_digest(root)

            self.assertEqual(first, second)
            self.assertEqual(len(first), 64)

    def test_build_and_write_provenance_payload(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            fixture_root = root / "Fixtures"
            fixture_root.mkdir()
            write_file(fixture_root, "transaction_payloads.json", "{}")
            state_file = write_file(root, "artifacts/swift_fixture_regen_state.json", '{"ok":true}')
            out_path = root / "artifacts/provenance.json"

            payload = PROV.build_provenance(
                fixture_root,
                state_file,
                git_ref="deadbeef",
                approver="swift-lead",
                change_ticket="IOS2-123",
                source_kind="archive",
                archive_sha256="abc123",
                archive_kind="zip",
                notes="regenerated after ABI bump",
            )
            PROV.write_provenance(out_path, payload)

            raw = json.loads(out_path.read_text(encoding="utf-8"))
            self.assertEqual(raw["fixture_root"], str(fixture_root))
            self.assertEqual(raw["state_file"], str(state_file))
            self.assertEqual(raw["git_revision"], "deadbeef")
            self.assertEqual(raw["approver"], "swift-lead")
            self.assertEqual(raw["change_ticket"], "IOS2-123")
            self.assertEqual(raw["source_kind"], "archive")
            self.assertEqual(raw["archive_sha256"], "abc123")
            self.assertEqual(raw["archive_kind"], "zip")
            self.assertIn("generated_at", raw)
            # digests should be 64 hex characters
            self.assertEqual(len(raw["fixture_sha256"]), 64)
            self.assertEqual(len(raw["state_sha256"]), 64)

    def test_main_writes_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            fixture_root = root / "Fixtures"
            fixture_root.mkdir()
            write_file(fixture_root, "sample.norito", "payload")
            state_file = write_file(root, "state.json", "{}")
            out_path = root / "out/provenance.json"

            rc = PROV.main(
                [
                    "--root",
                    str(fixture_root),
                    "--state-file",
                    str(state_file),
                    "--out",
                    str(out_path),
                    "--git-ref",
                    "cafebabe",
                ]
            )
            self.assertEqual(rc, 0)
            payload: dict[str, Any] = json.loads(out_path.read_text(encoding="utf-8"))
            self.assertEqual(payload["git_revision"], "cafebabe")
            self.assertEqual(payload["fixture_root"], str(fixture_root))
            self.assertEqual(payload["state_file"], str(state_file))
            self.assertEqual(len(payload["fixture_sha256"]), 64)
            self.assertEqual(len(payload["state_sha256"]), 64)
