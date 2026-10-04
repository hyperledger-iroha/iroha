"""Exercise authenticated Apple Cargo cache selection without invoking Cargo."""

from __future__ import annotations

import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
BUILDER = ROOT / "scripts/build_norito_xcframework.sh"


class CargoHomeSelectionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name).resolve()
        self.root = self.base / "source"
        self.build = self.root / "target/norito-bridge-local/build"
        self.target = self.build.parent / "cargo"
        self.output = self.build.parent / "artifacts"
        self.home = self.base / "home"
        self.cache = self.base / "isolated-cargo"
        for path in (self.build, self.target, self.output, self.home, self.cache):
            path.mkdir(parents=True, mode=0o700)
        self.build.parent.chmod(0o700)
        (self.root / ".gitignore").write_text("target/\n", encoding="utf-8")
        subprocess.run(["/usr/bin/git", "init", "-q", str(self.root)], check=True)
        (self.root / "scripts").mkdir()
        (self.root / "scripts/norito_bridge_local_integration.py").write_text(
            (ROOT / "scripts/norito_bridge_local_integration.py").read_text(encoding="utf-8"),
            encoding="utf-8",
        )
        (self.root / "scripts/run_mobile_hermetic_command.py").write_text(
            (ROOT / "scripts/run_mobile_hermetic_command.py").read_text(encoding="utf-8"),
            encoding="utf-8",
        )
        self.source = BUILDER.read_text(encoding="utf-8")
        self.fragment = self.source.split("resolve_mobile_cargo_home() {", 1)[1]
        self.fragment = "resolve_mobile_cargo_home() {" + self.fragment.split(
            '\nMOBILE_RUSTUP_HOME=', 1
        )[0]
        self.setup = "\n".join(
            f"{key}={shlex.quote(str(value))}"
            for key, value in {
                "USER_HOME_DIR": self.home,
                "ROOT_DIR": self.root,
                "BUILD_DIR": self.build,
                "CARGO_TARGET_DIR": self.target,
                "OUT_DIR": self.output,
                "PYTHON_BINARY": sys.executable,
            }.items()
        )
        self.setup += '\nrun_python312_clean() { "$PYTHON_BINARY" -I -S -B "$@"; }\n'
        self.setup += re.search(
            r"^paths_overlap\(\) \{.*?^\}", self.source, re.MULTILINE | re.DOTALL
        ).group(0)

    def select(self, value: str | None, *, local: bool = False) -> subprocess.CompletedProcess[str]:
        environment = dict(os.environ)
        environment.pop("MOBILE_SDK_CARGO_HOME", None)
        environment.pop("MOBILE_SDK_CARGO_INVOCATION_DIR", None)
        # Ordinary inherited Cargo configuration cannot select this build input.
        environment["CARGO_HOME"] = "/untrusted/inherited-cache"
        if value is not None:
            environment["MOBILE_SDK_CARGO_HOME"] = value
        command = self.setup + f"\nLOCAL_INTEGRATION={int(local)}\n"
        command += "LOCAL_INTEGRATION_ARGS=(--local-integration)\n" if local else "LOCAL_INTEGRATION_ARGS=()\n"
        command += self.fragment
        command += '\nprintf "%s\\n" "$MOBILE_CARGO_HOME"\n'
        return subprocess.run(
            ["/bin/bash", "-eu", "-c", command], env=environment,
            capture_output=True, text=True, check=False,
        )

    def test_default_and_existing_external_cache_are_selected_exactly(self) -> None:
        for value, expected in ((None, self.home / ".cargo"), (str(self.cache), self.cache)):
            with self.subTest(value=value):
                result = self.select(value)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout.strip(), str(expected))

    def test_empty_relative_missing_and_noncanonical_inputs_are_rejected(self) -> None:
        for value in ("", "isolated-cargo", str(self.base / "missing"),
                      str(self.cache) + "/.", str(self.cache) + "/../isolated-cargo"):
            with self.subTest(value=value):
                result = self.select(value)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("MOBILE_SDK_CARGO_HOME", result.stderr)
        regular = self.base / "regular-file"
        regular.write_text("not a directory", encoding="utf-8")
        self.assertNotEqual(self.select(str(regular)).returncode, 0)

    def test_symbolic_directory_and_symbolic_parent_are_rejected(self) -> None:
        symbolic = self.base / "symbolic-cache"
        symbolic.symlink_to(self.cache, target_is_directory=True)
        parent = self.base / "symbolic-parent"
        parent.symlink_to(self.base, target_is_directory=True)
        for value in (symbolic, parent / self.cache.name):
            with self.subTest(value=value):
                result = self.select(str(value))
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("non-symbolic canonical", result.stderr)

    def test_nonwritable_cache_is_rejected(self) -> None:
        self.cache.chmod(0o500)
        self.addCleanup(self.cache.chmod, 0o700)
        result = self.select(str(self.cache))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("owned writable", result.stderr)

    def test_only_private_local_build_cache_is_admitted_inside_source(self) -> None:
        local_cache = self.build / "cargo-home"
        local_cache.mkdir(mode=0o700)
        accepted = self.select(str(local_cache), local=True)
        self.assertEqual(accepted.returncode, 0, accepted.stderr)
        self.assertEqual(accepted.stdout.strip(), str(local_cache))
        self.assertNotEqual(self.select(str(local_cache)).returncode, 0)
        local_cache.chmod(0o755)
        self.assertNotEqual(self.select(str(local_cache), local=True).returncode, 0)
        for path in (self.root, self.target, self.build, self.output):
            with self.subTest(path=path):
                self.assertNotEqual(self.select(str(path), local=True).returncode, 0)

    def test_external_cache_cannot_contain_build_or_artifact_roots(self) -> None:
        # Point source-independent build roots into the otherwise valid cache.
        for label in ("BUILD_DIR", "CARGO_TARGET_DIR", "OUT_DIR"):
            with self.subTest(label=label):
                setup = self.setup
                self.setup += f"\n{label}={shlex.quote(str(self.cache / 'nested'))}\n"
                try:
                    result = self.select(str(self.cache))
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("disjoint", result.stderr)
                finally:
                    self.setup = setup

    def test_selected_cache_reaches_rustup_and_authenticated_source_seal(self) -> None:
        envelope = self.source.split("RUSTUP_ENV=(", 1)[1].split('\nCARGO_BINARY=', 1)[0]
        seal_function = re.search(
            r"^run_source_seal\(\) \{.*?^\}", self.source, re.MULTILINE | re.DOTALL
        )
        self.assertIsNotNone(seal_function)
        command = self.setup + "\n" + "\n".join([
            f"MOBILE_CARGO_HOME={shlex.quote(str(self.cache))}",
            "MOBILE_CARGO_INVOCATION_DIR=/fixture/invocation",
            "MOBILE_RUSTUP_HOME=/fixture/rustup", "MOBILE_TMPDIR=/fixture/tmp",
            "RUSTUP_BINARY=/fixture/rustup-bin", "CARGO_BINARY=/fixture/cargo",
            "RUSTC_BINARY=/fixture/rustc", "RUSTDOC_BINARY=/fixture/rustdoc",
            "GIT_BINARY=/fixture/git", "SOURCE_SEAL_SCRIPT=/fixture/seal.py",
            "env() { printf '%s\\n' \"$@\"; }",
            "RUSTUP_ENV=(" + envelope,
            "printf '%s\\n' \"${RUSTUP_ENV[@]}\"",
            seal_function.group(0), "run_source_seal fingerprint",
        ])
        result = subprocess.run(
            ["/bin/bash", "-eu", "-c", command], capture_output=True, text=True, check=True,
        )
        arguments = result.stdout.splitlines()
        self.assertIn(f"CARGO_HOME={self.cache}", arguments)
        self.assertIn(f"NORITO_BRIDGE_SEAL_CARGO_HOME={self.cache}", arguments)
        self.assertFalse(any(value.startswith("MOBILE_SDK_CARGO_HOME=") for value in arguments))

    def test_artifact_checker_selects_same_cache_and_actual_cargo_cwd(self) -> None:
        checker = (ROOT / "scripts/check_mobile_sdk_artifacts.sh").read_text(encoding="utf-8")
        fragment = 'SOURCE_SEAL_CARGO_HOME="$CHECK_USER_HOME_DIR/.cargo"' + checker.split(
            'SOURCE_SEAL_CARGO_HOME="$CHECK_USER_HOME_DIR/.cargo"', 1
        )[1].split('\nSOURCE_SEAL_RUSTUP_HOME=', 1)[0]
        invocation = self.base / "private-cwd"
        invocation.mkdir(mode=0o700)
        local_cache = self.build / "cargo-home"
        local_cache.mkdir(mode=0o700)
        for cache, local, accepted in ((self.cache, False, True), (local_cache, True, True), (local_cache, False, False)):
            with self.subTest(cache=cache, local=local):
                command = self.setup + '\nrun_isolated_checker_python() { run_python312_clean "$@"; }\n'
                command += '\n'.join([
                    f"CHECK_USER_HOME_DIR={shlex.quote(str(self.home))}",
                    f"LOCAL_INTEGRATION={int(local)}",
                    f"MOBILE_SDK_CARGO_HOME={shlex.quote(str(cache))}",
                    f"MOBILE_SDK_CARGO_INVOCATION_DIR={shlex.quote(str(invocation))}",
                    fragment,
                    'printf "%s\\n" "$SOURCE_SEAL_CARGO_HOME" "$SOURCE_SEAL_CARGO_INVOCATION_DIR"',
                ])
                result = subprocess.run(["/bin/bash", "-eu", "-c", command], capture_output=True, text=True)
                self.assertEqual(result.returncode == 0, accepted, result.stderr)
                if accepted:
                    self.assertEqual(result.stdout.splitlines(), [str(cache), str(invocation)])


if __name__ == "__main__":
    unittest.main()
