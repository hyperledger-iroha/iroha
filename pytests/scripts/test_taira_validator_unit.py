"""Exercise rendered native signer-descriptor custody with disposable byte fixtures."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/taira_validator_unit.py"
SPEC = importlib.util.spec_from_file_location("taira_validator_unit_under_test", SCRIPT)
unit = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(unit)


class ValidatorUnitTests(unittest.TestCase):
    def fixture(self, directory, name, size):
        path = Path(directory) / name
        path.write_bytes(bytes((index % 251 for index in range(size))))
        path.chmod(0o600)
        return path

    def inputs(self, directory):
        return (
            self.fixture(directory, "runtime", 71),
            self.fixture(directory, "mint", 32),
            self.fixture(directory, "beacon.norito", 257),
        )

    def execute(self, paths, *, beacon=True, reached_exec=True, occupy=False, config_file="config.toml"):
        runtime, mint, credential = paths
        code = unit.launcher(
            "taira-validator-1", str(runtime), str(mint),
            str(credential) if beacon else None, config_file=config_file,
        )
        # Execute the actual generated custody body in an isolated process.
        # The exec boundary observes inherited files, then raises to exercise
        # production cleanup. Fixture bytes never enter test output.
        driver = r'''
import json, os, sys
request = json.load(sys.stdin)
seen = False
owned = None
if request["occupy"]:
    owned = os.open(request["paths"][0], os.O_RDONLY)
    os.dup2(owned, 200, inheritable=True)
def observe_exec(executable, argv):
    global seen
    seen = True
    assert executable == argv[0]
    assert argv == ["/srv/taira/taira-validator-1/current/bin/iroha3d_taira",
                    "--config", "/srv/taira/taira-validator-1/current/config/" + request["config_file"],
                    "--sora"]
    expected = [(198, request["paths"][0]), (199, request["paths"][1])]
    if request["beacon"]:
        expected.append((200, request["paths"][2]))
    else:
        try: os.fstat(200)
        except OSError: pass
        else: raise AssertionError("bootstrap must not invent a beacon descriptor")
    for descriptor, source in expected:
        info = os.fstat(descriptor)
        assert info.st_mode & 0o7777 == 0o600 and info.st_nlink == 1
        assert os.get_inheritable(descriptor)
        with open(source, "rb") as file:
            assert os.read(descriptor, info.st_size + 1) == file.read()
        assert info.st_ino != os.stat(source).st_ino
    raise RuntimeError("simulated exec failure")
os.execv = observe_exec
try:
    exec(compile(request["code"], "<rendered-unit>", "exec"), {})
except (OSError, RuntimeError):
    pass
assert seen == request["reached_exec"]
if owned is not None:
    assert os.fstat(200).st_ino == os.stat(request["paths"][0]).st_ino
    os.close(200)
    os.close(owned)
'''
        result = subprocess.run(
            [sys.executable, "-c", driver],
            input=json.dumps({"code": code, "paths": list(map(str, paths)),
                              "beacon": beacon, "reached_exec": reached_exec,
                              "occupy": occupy, "config_file": config_file}),
            text=True, capture_output=True, timeout=10, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        return result

    def assert_no_launch_copies(self, paths):
        for descriptor, path in zip((198, 199, 200), paths):
            self.assertFalse(Path(str(path) + f".fd{descriptor}").exists())

    def test_beacon_stage_uses_independent_inherited_copy_and_cleans_failed_exec(self):
        with tempfile.TemporaryDirectory() as directory:
            paths = self.inputs(directory)
            originals = [path.read_bytes() for path in paths]
            for config_file in unit.CONFIG_FILES:
                with self.subTest(config_file=config_file):
                    self.execute(paths, config_file=config_file)
                    self.assert_no_launch_copies(paths)
                    self.assertEqual([path.read_bytes() for path in paths], originals)

    def test_initial_bootstrap_never_opens_or_fabricates_a_beacon_credential(self):
        with tempfile.TemporaryDirectory() as directory:
            paths = self.inputs(directory)
            paths[2].unlink()
            self.execute(paths, beacon=False)
            self.assert_no_launch_copies(paths)

    def test_missing_or_untrusted_beacon_cleans_prior_descriptors(self):
        for mutation in ("missing", "mode", "hardlink", "symlink", "empty", "oversized"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                paths = self.inputs(directory)
                credential = paths[2]
                originals = [path.read_bytes() for path in paths[:2]]
                if mutation == "missing":
                    credential.unlink()
                elif mutation == "mode":
                    credential.chmod(0o644)
                elif mutation == "hardlink":
                    os.link(credential, Path(directory) / "second-link")
                elif mutation == "symlink":
                    target = Path(directory) / "retained-beacon"
                    credential.rename(target)
                    credential.symlink_to(target)
                elif mutation == "empty":
                    credential.write_bytes(b"")
                elif mutation == "oversized":
                    with credential.open("r+b") as file:
                        file.truncate(16 * 1024 * 1024 + 1)
                self.execute(paths, reached_exec=False)
                self.assert_no_launch_copies(paths)
                self.assertEqual([path.read_bytes() for path in paths[:2]], originals)

    def test_occupied_beacon_descriptor_is_never_replaced_or_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            paths = self.inputs(directory)
            self.execute(paths, reached_exec=False, occupy=True)
            self.assert_no_launch_copies(paths)

    def test_stale_untrusted_beacon_launch_copy_is_never_followed(self):
        with tempfile.TemporaryDirectory() as directory:
            paths = self.inputs(directory)
            target = self.fixture(directory, "other-owner-file", 19)
            original = target.read_bytes()
            launch = Path(str(paths[2]) + ".fd200")
            launch.symlink_to(target)
            self.execute(paths, reached_exec=False)
            self.assertTrue(launch.is_symlink())
            self.assertEqual(target.read_bytes(), original)
            for descriptor, path in zip((198, 199), paths):
                self.assertFalse(Path(str(path) + f".fd{descriptor}").exists())

    def test_render_only_uses_paths_and_rejects_cross_role_collisions(self):
        with patch.object(os, "open", side_effect=AssertionError("render must not read keys")):
            rendered = unit.render("taira-validator-2", "/private/runtime/a", "/private/runtime/b", "/private/runtime/c")
        self.assertIn("Type=exec", rendered)
        self.assertIn(".fd", unit.CUSTODY)
        for credential in ("relative", "/private/runtime/a", "/private/runtime/b.fd199"):
            with self.subTest(credential=credential), self.assertRaises(ValueError):
                unit.render("taira-validator-2", "/private/runtime/a", "/private/runtime/b", credential)
        for config_file in ("", "/etc/other.toml", "../beacon.toml", "config.toml/../beacon.toml",
                            "beacon.toml\nExecStart=/bin/false", "$(touch nope)", "%n.toml", "other.toml"):
            with self.subTest(config_file=config_file), self.assertRaises(ValueError):
                unit.render("taira-validator-2", "/private/runtime/a", "/private/runtime/b",
                            "/private/runtime/c", config_file=config_file)
        self.assertEqual(
            unit.render("taira-validator-2", "/private/runtime/a", "/private/runtime/b"),
            unit.render("taira-validator-2", "/private/runtime/a", "/private/runtime/b", config_file="config.toml"),
        )

    def test_cli_publishes_exact_public_unit_without_opening_credentials(self):
        for config_file in (None, *unit.CONFIG_FILES):
            with self.subTest(config_file=config_file), tempfile.TemporaryDirectory() as directory:
                output = Path(directory) / "iroha3d-taira-validator-3.service"
                command = [sys.executable, str(SCRIPT), "--role", "taira-validator-3",
                           "--runtime-key", "/absent/runtime", "--mint-finality-seed", "/absent/mint",
                           "--global-beacon-credential", "/absent/beacon", "--output", str(output)]
                if config_file is not None:
                    command += ["--config-file", config_file]
                first = subprocess.run(command, capture_output=True, text=True, timeout=10)
                self.assertEqual(first.returncode, 0, first.stderr)
                self.assertEqual(output.stat().st_mode & 0o777, 0o644)
                original = output.read_bytes()
                self.assertEqual(original.decode(), unit.render(
                    "taira-validator-3", "/absent/runtime", "/absent/mint", "/absent/beacon",
                    config_file=config_file or "config.toml"))
                second = subprocess.run(command, capture_output=True, text=True, timeout=10)
                self.assertNotEqual(second.returncode, 0)
                self.assertEqual(output.read_bytes(), original)
                for invalid in ("../beacon.toml", "beacon.toml\nExecStart=/bin/false", "other.toml"):
                    rejected = subprocess.run(command + ["--config-file", invalid], capture_output=True, text=True, timeout=10)
                    self.assertEqual(rejected.returncode, 2)
                    self.assertEqual(output.read_bytes(), original)


if __name__ == "__main__":
    unittest.main()
