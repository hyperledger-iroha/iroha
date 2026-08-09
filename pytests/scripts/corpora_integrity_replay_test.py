"""Contracts for the security-corpus integrity-only replay."""

from __future__ import annotations

import contextlib
import hashlib
import importlib.util
import io
import json
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "replay_security_corpora.py"
WORKFLOW = ROOT / ".github" / "workflows" / "workspace_release.yml"


def load_runner():
    """Load the replay runner from this isolated source tree."""

    spec = importlib.util.spec_from_file_location("security_corpora_replay", SCRIPT)
    if spec is None or spec.loader is None:
        raise RuntimeError("unable to load security corpus replay runner")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class SecurityCorporaReplayTests(unittest.TestCase):
    """Prove fingerprint maintenance cannot impersonate semantic validation."""

    def setUp(self) -> None:
        self.runner = load_runner()
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.payload = self.root / "seed.bin"
        self.payload.write_bytes(b"security-corpus")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def entry(
        self,
        *,
        expected: str | None = None,
    ) -> dict[str, object]:
        """Build one structurally valid temporary metadata row."""

        row: dict[str, object] = {
            "path": self.payload.name,
            "kind": "test",
            "provenance": "isolated unit test",
            "expected_validation_fail": expected,
            "fingerprint": hashlib.blake2b(
                self.payload.read_bytes(), digest_size=32
            ).hexdigest(),
        }
        return row

    def write_metadata(self, rows: list[dict[str, object]]) -> Path:
        """Write one temporary metadata ledger."""

        path = self.root / "corpora.json"
        path.write_text(json.dumps(rows), encoding="utf-8")
        return path

    def test_fingerprint_replay_explicitly_reports_integrity_only(self) -> None:
        """A declared expected outcome is metadata and is never reported as observed."""

        metadata = self.write_metadata(
            [self.entry(expected="ValidationFail::NotPermitted")]
        )
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            self.assertEqual(self.runner.main(["--metadata", str(metadata)]), 0)
        self.assertIn("no validation outcomes were executed", output.getvalue())
        self.assertNotIn("Semantically validated", output.getvalue())

    def test_metadata_rejects_duplicate_paths_and_unsafe_files(self) -> None:
        """Duplicate inventory and path substitution fail before validation."""

        metadata = self.write_metadata([self.entry(), self.entry()])
        with self.assertRaisesRegex(ValueError, "duplicate corpus path"):
            self.runner.load_metadata(metadata)

        linked = self.root / "linked.bin"
        linked.symlink_to(self.payload)
        row = self.entry()
        row["path"] = linked.name
        with self.assertRaisesRegex(ValueError, "must not be a symlink"):
            self.runner.corpus_path(row, self.root)

    def test_update_mode_can_initialize_a_new_fingerprint(self) -> None:
        """Corpus onboarding can calculate a missing fingerprint without weakening checks."""

        row = self.entry()
        del row["fingerprint"]
        metadata = self.write_metadata([row])
        self.assertEqual(
            self.runner.main(["--metadata", str(metadata), "--update"]), 0
        )
        refreshed = json.loads(metadata.read_text(encoding="utf-8"))
        self.assertRegex(refreshed[0]["fingerprint"], r"^[0-9a-f]{64}$")

    def test_release_workflow_labels_fingerprints_and_fuzzing_separately(self) -> None:
        """The release workflow does not present integrity replay as validation."""

        workflow = WORKFLOW.read_text(encoding="utf-8")
        readme = (ROOT / "fuzz" / "README.md").read_text(encoding="utf-8")
        self.assertIn("name: Adversarial Norito and IVM fuzz gate", workflow)
        self.assertIn(
            "cargo +nightly-2025-05-08 install cargo-fuzz", workflow
        )
        self.assertIn(
            "Verify immutable security-corpus integrity fingerprints", workflow
        )
        self.assertIn("scripts/fuzz_smoke.sh --strict", workflow)
        self.assertNotIn("--require-semantic", workflow)
        self.assertNotIn("automatically page", readme)
        self.assertIn("does not\n  create or label an issue", readme)


if __name__ == "__main__":
    unittest.main()
