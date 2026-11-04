"""Check production-toggle attribution when the base inventory is stale."""

import contextlib
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


GUARD = Path(__file__).resolve().parents[1] / "check_env_config_surface.sh"
SCANNER = "scripts/inventory_env_toggles.py"
SOURCE = "crates/example/src/lib.rs"


class BaseSourceAttributionTests(unittest.TestCase):
    """Exercise the shell guard's actual embedded comparison with Git blobs."""

    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.previous = Path.cwd()
        os.chdir(self.root)
        self.addCleanup(os.chdir, self.previous)
        self.blobs = {
            SCANNER: b"unchanged inventory classifier\n",
            SOURCE: b'fn example() { std::env::var("EXAMPLE_TOGGLE"); }\n',
        }
        for name, body in self.blobs.items():
            path = self.root / name
            path.parent.mkdir(parents=True)
            path.write_bytes(body)
        self.rows = [{"path": SOURCE, "scope": "prod", "context": "original read"}]

    def detect(self, base_prod=0):
        base = {"variables": {"EXAMPLE_TOGGLE": {"counts": {"prod": base_prod}}}}
        head = {"variables": {"EXAMPLE_TOGGLE": {
            "counts": {"prod": 1}, "paths": self.rows,
        }}}
        (self.root / "base.json").write_text(json.dumps(base))
        (self.root / "head.json").write_text(json.dumps(head))
        source = GUARD.read_text().split("python3 - <<'PY'\n", 1)[1].split("\nPY\n", 1)[0]

        def git_blob(command, **kwargs):
            self.assertEqual(command[:2], ["git", "show"])
            self.assertTrue(command[2].startswith("reviewed-base:"))
            body = self.blobs.get(command[2].split(":", 1)[1])
            return subprocess.CompletedProcess(command, 1 if body is None else 0,
                                               stdout=body or b"", stderr=b"")

        output, errors = io.StringIO(), io.StringIO()
        with patch.dict(os.environ, {
            "BASE_JSON": str(self.root / "base.json"),
            "HEAD_JSON": str(self.root / "head.json"),
            "BASE_COMMIT": "reviewed-base",
        }), patch("subprocess.run", side_effect=git_blob), \
                contextlib.redirect_stdout(output), contextlib.redirect_stderr(errors):
            exec(compile(source, str(GUARD), "exec"), {})
        return output.getvalue(), errors.getvalue()

    def test_stale_inventory_accepts_only_identical_existing_source(self):
        output, errors = self.detect()
        self.assertEqual(output, "")
        self.assertIn("already present in identical base source", errors)

    def test_new_source_remains_a_new_production_toggle(self):
        del self.blobs[SOURCE]
        self.assertIn("EXAMPLE_TOGGLE (prod refs: 1)", self.detect()[0])

    def test_any_source_change_requires_review(self):
        (self.root / SOURCE).write_bytes(self.blobs[SOURCE] + b"// changed\n")
        self.assertIn("EXAMPLE_TOGGLE (prod refs: 1)", self.detect()[0])

    def test_classifier_change_requires_review(self):
        (self.root / SCANNER).write_text("changed classifier\n")
        self.assertIn("EXAMPLE_TOGGLE (prod refs: 1)", self.detect()[0])

    def test_missing_base_classifier_requires_review(self):
        del self.blobs[SCANNER]
        self.assertIn("EXAMPLE_TOGGLE (prod refs: 1)", self.detect()[0])

    def test_missing_current_source_requires_review(self):
        (self.root / SOURCE).unlink()
        self.assertIn("EXAMPLE_TOGGLE (prod refs: 1)", self.detect()[0])

    def test_all_production_paths_must_match(self):
        self.rows.append({"path": "crates/another/src/lib.rs", "scope": "prod"})
        self.assertIn("EXAMPLE_TOGGLE (prod refs: 1)", self.detect()[0])

    def test_no_production_path_cannot_establish_inheritance(self):
        self.rows = []
        self.assertIn("EXAMPLE_TOGGLE (prod refs: 1)", self.detect()[0])

    def test_outside_repository_path_cannot_establish_inheritance(self):
        for name in (str(self.root / SOURCE), "../outside.rs"):
            with self.subTest(name=name):
                self.rows = [{"path": name, "scope": "prod"}]
                self.assertIn("EXAMPLE_TOGGLE (prod refs: 1)", self.detect()[0])

    def test_existing_reviewed_production_toggle_remains_allowed(self):
        (self.root / SOURCE).write_text("changed source\n")
        self.assertEqual(self.detect(base_prod=1)[0], "")


if __name__ == "__main__":
    unittest.main()
