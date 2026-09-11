"""Exercise the generated launcher with disposable signer inputs and no daemon."""

import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


SOURCE = Path(__file__).resolve().parents[1] / "taira_validator_unit.py"
SPEC = importlib.util.spec_from_file_location("taira_validator_unit", SOURCE)
UNIT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(UNIT)


class ValidatorUnitTests(unittest.TestCase):
    def test_render_does_not_open_signers_and_waits_for_launcher_exec(self):
        with patch("os.open", side_effect=AssertionError("renderer read an input")):
            for role in UNIT.ROLES:
                text = UNIT.render(role, "/runtime/key", "/runtime/seed")
                self.assertIn("[Service]\nType=exec\n", text)
                self.assertEqual(text.count("ExecStart="), 1)
                self.assertIn("ExecStart=/usr/bin/python3 -c ", text)
                self.assertIn("UMask=0077\n", text)

    def test_generated_launcher_passes_independent_fds_and_cleans_failed_exec(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            key, seed = root / "key", root / "seed"
            for path, size in ((key, 71), (seed, 32)):
                path.write_bytes(os.urandom(size))
                path.chmod(0o600)
            before = {path: path.read_bytes() for path in (key, seed)}
            code = UNIT.launcher(UNIT.ROLES[0], str(key), str(seed))
            # Run descriptor manipulation in a child. Intercept only the final
            # exec: the generated custody code does the real opens and copies.
            child = """
import os
from pathlib import Path
key, seed, code = __import__('sys').argv[1:]
expected = [Path(key).read_bytes(), Path(seed).read_bytes()]
class StopExec(Exception): pass
def observe_exec(path, argv):
    assert argv == ['/srv/taira/taira-validator-1/current/bin/iroha3d_taira',
                    '--config', '/srv/taira/taira-validator-1/current/config/config.toml', '--sora']
    assert path == argv[0]
    for fd, value in zip((198, 199), expected):
        assert os.get_inheritable(fd)
        assert os.read(fd, len(value) + 1) == value
    raise StopExec()
os.execv = observe_exec
try:
    exec(compile(code, '<generated-launcher>', 'exec'))
except StopExec:
    pass
else:
    raise AssertionError('launcher did not reach foreground exec')
for fd in (198, 199):
    try: os.fstat(fd)
    except OSError: pass
    else: raise AssertionError('launch descriptor leaked')
assert not Path(key + '.fd198').exists()
assert not Path(seed + '.fd199').exists()
assert Path(key).read_bytes() == expected[0]
assert Path(seed).read_bytes() == expected[1]
"""
            result = subprocess.run(
                [sys.executable, "-I", "-c", child, str(key), str(seed), code],
                capture_output=True, timeout=10, check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr.decode())
            self.assertEqual(result.stdout, b"")
            self.assertEqual({path: path.read_bytes() for path in (key, seed)}, before)

    def test_rejects_ambiguous_roles_paths_and_overlapping_launch_copies(self):
        for path in ("relative", "//runtime/key", "/runtime/../key", "/runtime/key\n"):
            with self.subTest(path=path), self.assertRaises(ValueError):
                UNIT.render(UNIT.ROLES[0], path, "/runtime/seed")
        for role, key, seed in (("unknown", "/runtime/key", "/runtime/seed"),
                                (UNIT.ROLES[0], "/runtime/key", "/runtime/key.fd198")):
            with self.assertRaises(ValueError):
                UNIT.render(role, key, seed)


if __name__ == "__main__":
    unittest.main()
