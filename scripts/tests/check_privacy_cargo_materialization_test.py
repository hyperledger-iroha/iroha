"""Exercise canonical graph materialization with real filesystem boundaries."""
from __future__ import annotations

import hashlib
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import types
import unittest

ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / "ci/privacy_sdk_cargo_lockfile.sh"
OWNER = re.findall(
    r'^readonly PRIVACY_SDK_CANONICAL_CARGO_LOCK_SHA256=\\\n"([0-9a-f]{64})"$',
    HELPER.read_text(), re.MULTILINE,
)
assert len(OWNER) == 1


class CanonicalCargoMaterializationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="privacy-graph-boundary-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.source = self.root / "source"
        self.source.mkdir()
        self.lock = self.source / "Cargo.lock"
        self.lock.write_bytes((ROOT / "Cargo.lock").read_bytes())
        self.assertEqual(hashlib.sha256(self.lock.read_bytes()).hexdigest(), OWNER[0])
        self.destination = self.root / "external/Cargo.lock"
        self.destination.parent.mkdir()

    def invoke(self, destination=None, state=None):
        command = '''set -euo pipefail
source "$1"
state="${5:-$(privacy_sdk_capture_optional_file_state "$2/Cargo.lock" graph "$4")}"
privacy_sdk_materialize_canonical_cargo_lock "$2" "$3" "$state" "$4"
'''
        return subprocess.run(
            ["/bin/bash", "-c", command, "snapshot-test", str(HELPER),
             str(self.source), str(destination or self.destination), sys.executable,
             state or ""], capture_output=True, text=True, check=False,
        )

    def assert_rejected(self, result):
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertFalse(self.destination.exists())

    def test_snapshot_preserves_bytes_with_independent_readonly_inode(self):
        before = self.lock.stat()
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.destination.read_bytes(), self.lock.read_bytes())
        self.assertNotEqual(self.destination.stat().st_ino, before.st_ino)
        self.assertEqual(self.destination.stat().st_nlink, 1)
        self.assertEqual(self.destination.stat().st_mode & 0o777, 0o400)
        self.assertEqual(self.lock.stat(), before)

    def test_unreviewed_graph_bytes_are_rejected(self):
        self.lock.write_bytes(self.lock.read_bytes() + b"\n# unreviewed bytes\n")
        self.assert_rejected(self.invoke())

    def test_empty_source_is_rejected(self):
        self.lock.write_bytes(b"")
        self.assert_rejected(self.invoke())

    def test_executable_source_is_rejected(self):
        self.lock.chmod(0o700)
        self.assert_rejected(self.invoke())

    def test_symlink_source_is_rejected(self):
        other = self.source / "other"
        self.lock.rename(other)
        self.lock.symlink_to(other)
        self.assert_rejected(self.invoke())

    def test_hardlinked_source_is_rejected(self):
        os.link(self.lock, self.source / "other")
        self.assert_rejected(self.invoke())

    def run_python_owner(self, function, arguments, *, environment=None, prelude=""):
        # Execute the exact production body directly so a timeout terminates the
        # sole test child, including on the blocking-open regression.
        function_source = HELPER.read_text().split(function + "() {\n", 1)[1]
        body = function_source.split("<<'PY'\n", 1)[1].split("\nPY\n", 1)[0]
        driver = prelude + "\nexec(compile(" + repr(body) + ", " + repr(str(HELPER)) + ", 'exec'))\n"
        return subprocess.run(
            [sys.executable, "-I", "-", *map(str, arguments)], input=driver,
            env=environment, capture_output=True, text=True, check=False, timeout=5,
        )

    def test_materializer_fifo_source_rejects_without_blocking(self):
        self.lock.unlink()
        os.mkfifo(self.lock)
        result = self.run_python_owner(
            "privacy_sdk_materialize_canonical_cargo_lock",
            [self.source, self.destination, "present:unused", OWNER[0]],
        )
        self.assert_rejected(result)
        self.assertIn("regular file", result.stderr)

    def test_resolver_fifo_selection_rejects_without_blocking(self):
        os.mkfifo(self.destination)
        environment = dict(os.environ, IROHA_PRIVACY_CARGO_LOCKFILE_PATH=str(self.destination))
        result = self.run_python_owner(
            "privacy_sdk_resolve_cargo_lockfile", [self.source], environment=environment,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("regular file", result.stderr)

    def test_file_seal_fifo_rejects_without_blocking(self):
        os.mkfifo(self.destination)
        result = self.run_python_owner("privacy_sdk_file_seal", [self.destination])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("regular file", result.stderr)

    def test_executable_seal_fifo_rejects_without_blocking(self):
        os.mkfifo(self.destination, 0o700)
        result = self.run_python_owner("privacy_sdk_executable_seal", [self.destination])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("regular file", result.stderr)

    def test_source_parent_symlink_swap_after_write_is_rejected(self):
        state = self.run_python_owner("privacy_sdk_file_seal", [self.lock])
        self.assertEqual(state.returncode, 0, state.stderr)
        # Swap an ancestor while retaining the original source inode. The final
        # descriptor identity alone cannot distinguish this path mutation.
        prelude = '''import os
from pathlib import Path
original_fsync = os.fsync
def swap_source_parent(descriptor):
    original_fsync(descriptor)
    source = Path(__import__('sys').argv[1])
    moved = source.with_name('moved-source')
    source.rename(moved)
    source.symlink_to(moved, target_is_directory=True)
os.fsync = swap_source_parent
'''
        result = self.run_python_owner(
            "privacy_sdk_materialize_canonical_cargo_lock",
            [self.source, self.destination, "present:" + state.stdout.strip(), OWNER[0]],
            prelude=prelude,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("source changed after materialization", result.stderr)

    def test_resolver_parent_symlink_swap_during_read_is_rejected(self):
        self.destination.write_bytes(self.lock.read_bytes())
        prelude = '''import os
from pathlib import Path
original_read = os.read
swapped = False
def swap_selected_parent(descriptor, count):
    global swapped
    payload = original_read(descriptor, count)
    if not swapped:
        parent = Path(os.environ['IROHA_PRIVACY_CARGO_LOCKFILE_PATH']).parent
        moved = parent.with_name('moved-external')
        parent.rename(moved)
        parent.symlink_to(moved, target_is_directory=True)
        swapped = True
    return payload
os.read = swap_selected_parent
'''
        environment = dict(os.environ, IROHA_PRIVACY_CARGO_LOCKFILE_PATH=str(self.destination))
        result = self.run_python_owner(
            "privacy_sdk_resolve_cargo_lockfile", [self.source],
            environment=environment, prelude=prelude,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("path became noncanonical", result.stderr)

    def test_root_selection_is_rejected_without_mutation(self):
        before = self.lock.stat(), self.lock.read_bytes()
        result = self.invoke(self.lock)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.lock.stat(), self.lock.read_bytes()), before)

    def test_in_tree_selection_is_rejected(self):
        destination = self.source / "private/Cargo.lock"
        destination.parent.mkdir()
        self.assert_rejected(self.invoke(destination))
        self.assertFalse(destination.exists())

    def test_relative_selection_is_rejected(self):
        self.assert_rejected(self.invoke(Path("relative/Cargo.lock")))

    def test_parent_symlink_is_rejected(self):
        alias = self.root / "alias"
        alias.symlink_to(self.destination.parent, target_is_directory=True)
        self.assert_rejected(self.invoke(alias / "Cargo.lock"))

    def test_wrong_basename_is_rejected(self):
        self.assert_rejected(self.invoke(self.destination.with_name("other.lock")))

    def assert_existing_destination_untouched(self, kind):
        other = self.destination.parent / "sentinel"
        other.write_bytes(b"must remain unchanged")
        if kind == "regular":
            self.destination.write_bytes(b"must remain unchanged")
        elif kind == "symlink":
            self.destination.symlink_to(other)
        else:
            os.link(other, self.destination)
        before = self.destination.lstat()
        result = self.invoke()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.destination.lstat(), before)
        self.assertEqual(self.destination.read_bytes(), b"must remain unchanged")
        self.assertEqual(other.read_bytes(), b"must remain unchanged")

    def test_existing_regular_destination_is_never_overwritten(self):
        self.assert_existing_destination_untouched("regular")

    def test_existing_symlink_destination_is_never_overwritten(self):
        self.assert_existing_destination_untouched("symlink")

    def test_existing_hardlink_destination_is_never_overwritten(self):
        self.assert_existing_destination_untouched("hardlink")

    def test_digest_cannot_replace_full_authenticated_source_state(self):
        result = self.invoke(state="present:" + OWNER[0])
        self.assert_rejected(result)
        self.assertIn("authenticated reviewed state", result.stderr)

    def test_historical_external_authority_is_not_a_second_graph(self):
        self.assert_rejected(self.invoke(state="present:cd9e829e454171f17540abeb7fd1aa14129252082bd8b076a0199b0ffa4e3f79:0:0:0:0:0:0o400"))
        text = HELPER.read_text()
        self.assertNotIn("PRIVACY_SDK_FROZEN_RELEASE_CARGO_LOCK_SHA256", text)
        self.assertNotIn("PRIVACY_SDK_TRACKED_ROOT_CARGO_LOCK_SHA256", text)
        self.assertNotIn("generate-lockfile", text)


class CanonicalGraphWorkflowOrderTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        guard = ROOT / "ci/check_privacy_sdk_guard.sh"
        body = guard.read_text().split("<<'PY'\n", 1)[1].split("\nPY\n", 1)[0]
        module = types.ModuleType("canonical_graph_workflow_order_test")
        previous_argv = sys.argv
        sys.modules[module.__name__] = module
        try:
            sys.argv = [str(guard), str(ROOT), ""]
            exec(compile(body[:body.index("def check(overrides:")], str(guard), "exec"), module.__dict__)
        finally:
            sys.argv = previous_argv
            del sys.modules[module.__name__]
        cls.check_workflow = staticmethod(module._check_cargo_workflow)
        cls.workflow = (ROOT / ".github/workflows/pr_privacy_sdk_guard.yml").read_text()

    def test_reviewed_workflow_initializes_every_graph_comparison(self):
        errors = []
        self.check_workflow(self.workflow, errors)
        self.assertEqual(errors, [])

    def test_each_fresh_shell_rejects_graph_use_before_owner_import(self):
        checks = 0
        for step in re.split(r"(?m)(?=^      - )", self.workflow):
            owner = "          source ci/privacy_sdk_cargo_lockfile.sh\n"
            if owner not in step:
                continue
            lines = step.splitlines(keepends=True)
            lines.remove(owner)
            first_use = next(i for i, line in enumerate(lines) if "${PRIVACY_SDK_CANONICAL_CARGO_LOCK_SHA256}" in line)
            lines.insert(first_use + 1, owner)
            with self.subTest(step=lines[0].strip()):
                errors = []
                self.check_workflow(self.workflow.replace(step, "".join(lines), 1), errors)
                # Require the semantic order rejection independently of the
                # complete job digest, whose check also rejects these bytes.
                self.assertTrue(any("must source the sole graph owner before every pin use" in error for error in errors), errors)
            checks += 1
        self.assertEqual(checks, 4)


if __name__ == "__main__":
    unittest.main()
