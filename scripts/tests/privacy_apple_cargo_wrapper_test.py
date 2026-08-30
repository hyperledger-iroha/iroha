#!/usr/bin/env python3
"""Exercise the real privacy Apple runner/wrapper corridor without Cargo."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


SOURCE_ROOT = Path(__file__).resolve().parents[2]
SELECTOR = "1.93.1-aarch64-apple-darwin"
PROFILE_SPECS = {
    "privacy-apple-ios-device-arm64": {
        "target": "aarch64-apple-ios",
        "platform": "iPhoneOS",
        "environment": {"IPHONEOS_DEPLOYMENT_TARGET": "15.0"},
    },
    "privacy-apple-ios-simulator-arm64": {
        "target": "aarch64-apple-ios-sim",
        "platform": "iPhoneSimulator",
        "environment": {
            "IPHONEOS_DEPLOYMENT_TARGET": "15.0",
            "IPHONESIMULATOR_DEPLOYMENT_TARGET": "15.0",
        },
    },
    "privacy-apple-ios-simulator-x86_64": {
        "target": "x86_64-apple-ios",
        "platform": "iPhoneSimulator",
        "environment": {
            "IPHONEOS_DEPLOYMENT_TARGET": "15.0",
            "IPHONESIMULATOR_DEPLOYMENT_TARGET": "15.0",
        },
    },
    "privacy-apple-macos-arm64": {
        "target": "aarch64-apple-darwin",
        "platform": "MacOSX",
        "environment": {"MACOSX_DEPLOYMENT_TARGET": "12.0"},
    },
    "privacy-apple-macos-x86_64": {
        "target": "x86_64-apple-darwin",
        "platform": "MacOSX",
        "environment": {"MACOSX_DEPLOYMENT_TARGET": "12.0"},
    },
}


class PrivacyAppleCargoWrapperTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.fixture = Path(self.temporary_directory.name).resolve()
        self.root = self.fixture / "repository"
        for directory in (self.root / "ci", self.root / "scripts", self.root / ".cargo"):
            directory.mkdir(parents=True, exist_ok=True)

        self.wrapper = self.root / "ci" / "privacy_sdk_cargo_wrapper.sh"
        self.helper = self.root / "ci" / "privacy_sdk_cargo_lockfile.sh"
        self.runner = self.root / "scripts" / "run_mobile_hermetic_command.py"
        for source, destination in (
            (SOURCE_ROOT / "ci" / self.wrapper.name, self.wrapper),
            (SOURCE_ROOT / "ci" / self.helper.name, self.helper),
            (SOURCE_ROOT / "scripts" / self.runner.name, self.runner),
        ):
            shutil.copy2(source, destination)
        self.wrapper.chmod(0o755)
        self.runner.chmod(0o755)

        self._write(
            self.root / "rust-toolchain.toml",
            '[toolchain]\nchannel = "1.93.1"\n',
        )
        self._write(
            self.root / ".cargo" / "config.toml",
            '[alias]\nxtask = "run --package xtask --features dev-tools --bin xtask --"\n',
        )
        self._write(
            self.root / "Cargo.lock",
            'version = 4\n\n[[package]]\nname = "workspace"\nversion = "0.0.0"\n',
        )
        lock_directory = self.fixture / "authority"
        lock_directory.mkdir()
        self.lockfile = lock_directory / "Cargo.lock"
        self._write(
            self.lockfile,
            'version = 4\n\n[[package]]\nname = "release"\nversion = "0.0.0"\n',
        )

        self.home = self.fixture / "home"
        self.cargo_home = self.fixture / "cargo-home"
        self.target_dir = self.fixture / "cargo-target"
        self.tmpdir = self.fixture / "tmp"
        for directory in (self.home, self.cargo_home, self.target_dir, self.tmpdir):
            directory.mkdir()
        self.audit = self.fixture / "cargo-audit"
        self.audit.write_bytes(b"")
        self.log = self.fixture / "fake-cargo-log.jsonl"

        self.toolchain = self.fixture / "rustup" / "toolchains" / SELECTOR
        self.toolchain_bin = self.toolchain / "bin"
        self.toolchain_bin.mkdir(parents=True)
        self.real_cargo = self.toolchain_bin / "cargo"
        self.rustc = self.toolchain_bin / "rustc"
        self.rustdoc = self.toolchain_bin / "rustdoc"
        self.rustup = self.fixture / "rustup" / "bin" / "rustup"
        self.rustup.parent.mkdir()
        self._write_fake_cargo()
        for executable in (self.rustc, self.rustdoc, self.rustup):
            self._write(executable, "#!/bin/sh\nexit 0\n", mode=0o755)

        for target in (spec["target"] for spec in PROFILE_SPECS.values()):
            library = self.toolchain / "lib" / "rustlib" / str(target) / "lib" / "libstd.rlib"
            self._write(library, f"sealed fixture for {target}\n")

        self.developer = self.fixture / "Xcode.app" / "Contents" / "Developer"
        self.sdkroots: dict[str, Path] = {}
        for platform in {str(spec["platform"]) for spec in PROFILE_SPECS.values()}:
            sdkroot = (
                self.developer
                / "Platforms"
                / f"{platform}.platform"
                / "Developer"
                / "SDKs"
                / f"{platform}.sdk"
            )
            sdkroot.mkdir(parents=True)
            self.sdkroots[platform] = sdkroot

        self.base_environment = self._capture_corridor_environment()
        self.manifest = self.fixture / "apple-targets.json"
        completed = subprocess.run(
            [
                sys.executable,
                "-I",
                "-B",
                str(self.runner),
                "seal-apple-targets",
                "--toolchain-cargo",
                str(self.real_cargo),
                "--toolchain-selector",
                SELECTOR,
                "--output",
                str(self.manifest),
            ],
            check=True,
            cwd=self.root,
            text=True,
            capture_output=True,
        )
        self.manifest_seal = completed.stdout.strip()

        module_spec = importlib.util.spec_from_file_location(
            "privacy_runner_fixture", self.runner
        )
        if module_spec is None or module_spec.loader is None:
            raise AssertionError("unable to import copied hermetic runner")
        runner_module = importlib.util.module_from_spec(module_spec)
        module_spec.loader.exec_module(runner_module)
        self.runner_profiles = runner_module.PROFILES
        self.assertEqual(runner_module.PRIVACY_APPLE_PROFILE_SPECS, PROFILE_SPECS)

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    @staticmethod
    def _write(path: Path, contents: str, *, mode: int = 0o644) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents, encoding="utf-8")
        path.chmod(mode)

    def _write_fake_cargo(self) -> None:
        script = f"""#!{sys.executable}
import json
import os
from pathlib import Path
import sys

expected = {{
    "CARGO": {str(self.wrapper)!r},
    "RUSTC": {str(self.rustc)!r},
    "RUSTDOC": {str(self.rustdoc)!r},
}}
for name, value in expected.items():
    if os.environ.get(name) != value:
        raise SystemExit(f"fake Cargo rejected {{name}}={{os.environ.get(name)!r}}")
if any(name.startswith("IROHA_PRIVACY_") for name in os.environ):
    raise SystemExit("fake Cargo inherited a private corridor variable")
if "RUSTUP_HOME" in os.environ or "RUSTUP_TOOLCHAIN" in os.environ:
    raise SystemExit("fake Cargo inherited a rustup selector")
record = {{
    "argv": sys.argv[1:],
    "developer_dir": os.environ.get("DEVELOPER_DIR"),
    "sdkroot": os.environ.get("SDKROOT"),
    "iphoneos": os.environ.get("IPHONEOS_DEPLOYMENT_TARGET"),
    "iphonesimulator": os.environ.get("IPHONESIMULATOR_DEPLOYMENT_TARGET"),
    "macosx": os.environ.get("MACOSX_DEPLOYMENT_TARGET"),
}}
with Path({str(self.log)!r}).open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(record, sort_keys=True) + "\\n")
"""
        self._write(self.real_cargo, script, mode=0o755)

    def _helper_call(self, function: str, *arguments: str | Path) -> str:
        command = (
            'source "$1"; function_name="$2"; shift 2; '
            '"$function_name" "$@"'
        )
        completed = subprocess.run(
            [
                "/bin/bash",
                "-c",
                command,
                "fixture-helper",
                str(self.helper),
                function,
                *(str(argument) for argument in arguments),
            ],
            check=True,
            text=True,
            capture_output=True,
            env={
                "HOME": str(self.home),
                "LANG": "C.UTF-8",
                "LC_ALL": "C.UTF-8",
                "PATH": "/usr/bin:/bin",
            },
        )
        return completed.stdout.strip()

    def _capture_corridor_environment(self) -> dict[str, str]:
        file_seal = lambda path: self._helper_call(
            "privacy_sdk_file_seal", path, sys.executable
        )
        executable_seal = lambda path: self._helper_call(
            "privacy_sdk_executable_seal", path, sys.executable
        )
        directory_state = lambda path: self._helper_call(
            "privacy_sdk_capture_directory_state", path, sys.executable
        )
        cache_state = lambda component: self._helper_call(
            "privacy_sdk_capture_cargo_cache_component_state",
            self.cargo_home,
            component,
            sys.executable,
        )
        return {
            "CARGO": str(self.wrapper),
            "CARGO_BUILD_JOBS": "1",
            "CARGO_ENCODED_RUSTFLAGS": "",
            "CARGO_HOME": str(self.cargo_home),
            "CARGO_INCREMENTAL": "0",
            "CARGO_NET_OFFLINE": "true",
            "CARGO_TARGET_DIR": str(self.target_dir),
            "HOME": str(self.home),
            "LANG": "C.UTF-8",
            "LC_ALL": "C.UTF-8",
            "NORITO_SKIP_BINDINGS_SYNC": "1",
            "PATH": "/usr/bin:/bin",
            "RUSTC_BOOTSTRAP": "1",
            "TMPDIR": str(self.tmpdir),
            "IROHA_PRIVACY_CARGO_LOCKFILE_PATH": str(self.lockfile),
            "IROHA_PRIVACY_AUTHENTICATED_CARGO_LOCKFILE_PATH": str(self.lockfile),
            "IROHA_PRIVACY_AUTHENTICATED_CARGO_LOCKFILE_SEAL": file_seal(self.lockfile),
            "IROHA_PRIVACY_AUTHENTICATED_WORKSPACE_CARGO_LOCK_STATE": self._helper_call(
                "privacy_sdk_capture_optional_file_state",
                self.root / "Cargo.lock",
                "workspace Cargo.lock",
                sys.executable,
            ),
            "IROHA_PRIVACY_AUTHENTICATED_CARGO_CONFIG_PATH": str(
                self.root / ".cargo" / "config.toml"
            ),
            "IROHA_PRIVACY_AUTHENTICATED_CARGO_CONFIG_SEAL": file_seal(
                self.root / ".cargo" / "config.toml"
            ),
            "IROHA_PRIVACY_AUTHENTICATED_CARGO_HOME": str(self.cargo_home),
            "IROHA_PRIVACY_AUTHENTICATED_CARGO_HOME_DIRECTORY_STATE": directory_state(
                self.cargo_home
            ),
            "IROHA_PRIVACY_AUTHENTICATED_CARGO_REGISTRY_LINK_STATE": cache_state(
                "registry"
            ),
            "IROHA_PRIVACY_AUTHENTICATED_CARGO_GIT_LINK_STATE": cache_state("git"),
            "IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIR": str(self.target_dir),
            "IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIRECTORY_STATE": directory_state(
                self.target_dir
            ),
            "IROHA_PRIVACY_AUTHENTICATED_RUST_TOOLCHAIN_PATH": str(
                self.root / "rust-toolchain.toml"
            ),
            "IROHA_PRIVACY_AUTHENTICATED_RUST_TOOLCHAIN_SEAL": file_seal(
                self.root / "rust-toolchain.toml"
            ),
            "IROHA_PRIVACY_AUTHENTICATED_RUST_TOOLCHAIN_SELECTOR": SELECTOR,
            "IROHA_PRIVACY_AUTHENTICATED_RUSTUP_PATH": str(self.rustup),
            "IROHA_PRIVACY_AUTHENTICATED_RUSTUP_SEAL": executable_seal(self.rustup),
            "IROHA_PRIVACY_AUTHENTICATED_CARGO_PATH": str(self.real_cargo),
            "IROHA_PRIVACY_AUTHENTICATED_CARGO_SEAL": executable_seal(self.real_cargo),
            "IROHA_PRIVACY_AUTHENTICATED_RUSTC_PATH": str(self.rustc),
            "IROHA_PRIVACY_AUTHENTICATED_RUSTC_SEAL": executable_seal(self.rustc),
            "IROHA_PRIVACY_AUTHENTICATED_RUSTDOC_PATH": str(self.rustdoc),
            "IROHA_PRIVACY_AUTHENTICATED_RUSTDOC_SEAL": executable_seal(self.rustdoc),
            "IROHA_PRIVACY_SDK_ROOT": str(self.root),
            "IROHA_PRIVACY_REAL_CARGO": str(self.real_cargo),
            "IROHA_PRIVACY_CARGO_AUDIT_PATH": str(self.audit),
            "IROHA_PRIVACY_LOCKFILE_PYTHON_BIN": sys.executable,
        }

    def _environment(self, profile: str) -> dict[str, str]:
        spec = PROFILE_SPECS[profile]
        environment = dict(self.base_environment)
        environment.update(
            {
                "DEVELOPER_DIR": str(self.developer),
                "SDKROOT": str(self.sdkroots[str(spec["platform"])]),
                "IROHA_PRIVACY_AUTHENTICATED_APPLE_TARGETS_MANIFEST_PATH": str(
                    self.manifest
                ),
                "IROHA_PRIVACY_AUTHENTICATED_APPLE_TARGETS_MANIFEST_SEAL": (
                    self.manifest_seal
                ),
                "IROHA_PRIVACY_AUTHENTICATED_APPLE_CARGO_PROFILE": profile,
                "IROHA_PRIVACY_AUTHENTICATED_APPLE_TARGET": str(spec["target"]),
                "IROHA_PRIVACY_AUTHENTICATED_DEVELOPER_DIR": str(self.developer),
                "IROHA_PRIVACY_AUTHENTICATED_SDKROOT": str(
                    self.sdkroots[str(spec["platform"])]
                ),
                **dict(spec["environment"]),
            }
        )
        self.assertEqual(set(environment), set(self.runner_profiles[profile]))
        return environment

    def _invoke(
        self,
        profile: str,
        *,
        arguments: list[str] | None = None,
        environment_changes: dict[str, str] | None = None,
    ) -> subprocess.CompletedProcess[str]:
        spec = PROFILE_SPECS[profile]
        environment = self._environment(profile)
        if environment_changes:
            environment.update(environment_changes)
        cargo_arguments = arguments or [
            "build",
            "--locked",
            "--offline",
            "--jobs",
            "1",
            "-p",
            "connect_norito_bridge",
            "--lib",
            "--release",
            "--target",
            str(spec["target"]),
        ]
        command = [
            sys.executable,
            "-I",
            "-B",
            str(self.runner),
            "--profile",
            profile,
        ]
        for name in sorted(environment):
            command.extend(("--set", f"{name}={environment[name]}"))
        command.extend(("--", str(self.wrapper), *cargo_arguments))
        return subprocess.run(
            command,
            cwd=self.root,
            text=True,
            capture_output=True,
            check=False,
        )

    def _assert_rejected(self, completed: subprocess.CompletedProcess[str]) -> None:
        self.assertNotEqual(completed.returncode, 0, completed.stdout + completed.stderr)

    def test_five_exact_profiles_reach_real_cargo_through_wrapper(self) -> None:
        for profile in PROFILE_SPECS:
            completed = self._invoke(profile)
            self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)

        expected_audit = [
            f"build\t{profile}\t{spec['target']}"
            for profile, spec in PROFILE_SPECS.items()
        ]
        self.assertEqual(
            self.audit.read_text(encoding="utf-8").splitlines(), expected_audit
        )
        records = [
            json.loads(line)
            for line in self.log.read_text(encoding="utf-8").splitlines()
        ]
        self.assertEqual(len(records), 5)
        for (profile, spec), record in zip(
            PROFILE_SPECS.items(), records, strict=True
        ):
            arguments = record["argv"]
            self.assertEqual(arguments[:2], ["-Z", "unstable-options"])
            self.assertEqual(arguments.count("-Z"), 1)
            self.assertEqual(arguments.count("--lockfile-path"), 1)
            lock_index = arguments.index("--lockfile-path")
            self.assertEqual(arguments[lock_index + 1], str(self.lockfile))
            self.assertEqual(arguments.count("--target"), 1)
            target_index = arguments.index("--target")
            self.assertEqual(arguments[target_index + 1], spec["target"])
            self.assertEqual(record["developer_dir"], str(self.developer))
            self.assertEqual(
                record["sdkroot"], str(self.sdkroots[str(spec["platform"])])
            )
            expected_deployment = dict(spec["environment"])
            self.assertEqual(
                record["iphoneos"],
                expected_deployment.get("IPHONEOS_DEPLOYMENT_TARGET"),
            )
            self.assertEqual(
                record["iphonesimulator"],
                expected_deployment.get("IPHONESIMULATOR_DEPLOYMENT_TARGET"),
            )
            self.assertEqual(
                record["macosx"],
                expected_deployment.get("MACOSX_DEPLOYMENT_TARGET"),
            )

    def test_runner_rejects_caller_lock_and_unstable_options(self) -> None:
        profile = "privacy-apple-ios-device-arm64"
        target = str(PROFILE_SPECS[profile]["target"])
        for injected in (
            ["-Z", "unstable-options"],
            ["--lockfile-path", str(self.lockfile)],
            [f"--lockfile-path={self.lockfile}"],
        ):
            completed = self._invoke(
                profile,
                arguments=[
                    "build",
                    "--locked",
                    "--offline",
                    "--jobs",
                    "1",
                    *injected,
                    "--target",
                    target,
                ],
            )
            self._assert_rejected(completed)
        self.assertFalse(self.log.exists())

    def test_wrong_target_and_profile_descriptor_are_rejected(self) -> None:
        profile = "privacy-apple-ios-device-arm64"
        completed = self._invoke(
            profile,
            arguments=[
                "build",
                "--locked",
                "--offline",
                "--jobs",
                "1",
                "--target",
                "x86_64-apple-ios",
            ],
        )
        self._assert_rejected(completed)
        completed = self._invoke(
            profile,
            environment_changes={
                "IROHA_PRIVACY_AUTHENTICATED_APPLE_TARGET": "x86_64-apple-ios"
            },
        )
        self._assert_rejected(completed)
        self.assertFalse(self.log.exists())

    def test_mutated_target_sysroot_is_rejected(self) -> None:
        target = str(PROFILE_SPECS["privacy-apple-ios-device-arm64"]["target"])
        library = self.toolchain / "lib" / "rustlib" / target / "lib" / "libstd.rlib"
        library.write_text("mutated target sysroot\n", encoding="utf-8")
        completed = self._invoke("privacy-apple-ios-device-arm64")
        self._assert_rejected(completed)
        self.assertFalse(self.log.exists())

    def test_mutated_manifest_and_toolchain_binary_are_rejected(self) -> None:
        self.manifest.chmod(0o600)
        completed = self._invoke("privacy-apple-ios-device-arm64")
        self._assert_rejected(completed)
        self.manifest.chmod(0o400)

        with self.real_cargo.open("a", encoding="utf-8") as handle:
            handle.write("\n# mutation\n")
        completed = self._invoke("privacy-apple-ios-device-arm64")
        self._assert_rejected(completed)
        self.assertFalse(self.log.exists())

    def test_replaced_cargo_home_and_cache_link_are_rejected(self) -> None:
        replacement = self.fixture / "replacement-home"
        replacement.mkdir()
        moved = self.fixture / "original-home"
        self.cargo_home.rename(moved)
        replacement.rename(self.cargo_home)
        completed = self._invoke("privacy-apple-ios-device-arm64")
        self._assert_rejected(completed)

        self.cargo_home.rename(replacement)
        moved.rename(self.cargo_home)
        registry = self.fixture / "registry-cache"
        registry.mkdir()
        (self.cargo_home / "registry").symlink_to(registry)
        completed = self._invoke("privacy-apple-ios-device-arm64")
        self._assert_rejected(completed)
        self.assertFalse(self.log.exists())


if __name__ == "__main__":
    unittest.main()
