"""Failure artifacts contain process logs, never fixture keys or linked files."""

import importlib.util
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[2] / "scripts/collect_test_network_logs.py"
SPEC = importlib.util.spec_from_file_location("collect_test_network_logs", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class CollectTestNetworkLogsTests(unittest.TestCase):
    def test_hidden_fixture_root_only_exports_regular_process_logs(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / ".private"
            peer = source / "irohad_test_network_example" / "validator"
            peer.mkdir(parents=True)
            allowed = ["run-1-stdout.log", "run-12-stderr.log"]
            for name in allowed:
                (peer / name).write_text(name)
            for name in ["run-1-config.toml", "private.key", "run-1-genesis.nrt", "run-1-stdout.log.key"]:
                (peer / name).write_text("must stay private")
            secret = root / "secret"
            secret.write_text("must stay private")
            (peer / "run-2-stdout.log").symlink_to(secret)
            (source / "irohad_test_network_link").symlink_to(peer.parent, target_is_directory=True)
            (peer.parent / "linked_peer").symlink_to(peer, target_is_directory=True)
            destination = root / "public-logs"
            self.assertEqual(MODULE.collect_logs(source, destination), 2)
            files = sorted(path for path in destination.rglob("*") if path.is_file())
            self.assertEqual(sorted(path.name for path in files), sorted(allowed))
            self.assertEqual(sorted(path.read_text() for path in files), sorted(allowed))

    def test_missing_fixture_root_is_empty(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.assertEqual(MODULE.collect_logs(root / "missing", root / "output"), 0)
            self.assertFalse((root / "output").exists())


if __name__ == "__main__":
    unittest.main()
