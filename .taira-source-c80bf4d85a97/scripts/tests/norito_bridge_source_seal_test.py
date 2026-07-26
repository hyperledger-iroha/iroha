from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


SCRIPT = Path(__file__).parents[1] / "norito_bridge_source_seal.py"
APPLE_BUILDER = Path(__file__).parents[1] / "build_norito_xcframework.sh"
SPEC = importlib.util.spec_from_file_location("norito_bridge_source_seal", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
seal = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = seal
SPEC.loader.exec_module(seal)


class NoritoBridgeSourceSealTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        for relative, contents in {
            "Cargo.toml": "[workspace]\n",
            "Cargo.lock": "# locked\n",
            "rust-toolchain.toml": "[toolchain]\nchannel = 'stable'\n",
            "IrohaSwift/Package.swift": "// package\n",
            "IrohaSwift/Package.resolved": '{"pins":[],"version":3}\n',
            "IrohaSwift/Sources/IrohaSwift/Core.swift": "public struct Core {}\n",
            "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift": (
                'let hashes = [\n'
                '    "macos-arm64": "' + ("1" * 64) + '",\n'
                '    "ios-arm64": "' + ("2" * 64) + '",\n'
                '    "ios-arm64_x86_64-simulator": "' + ("3" * 64) + '"\n'
                ']\n'
            ),
            "IrohaSwift/Sources/IrohaSwiftMobileTransports/Nfc.swift":
                "public struct Nfc {}\n",
            "scripts/build_norito_xcframework.sh": "#!/bin/sh\n",
            "scripts/check_mobile_sdk_artifact_pin_commit.py": "#!/usr/bin/env python3\n",
            "scripts/check_mobile_sdk_artifacts.sh": "#!/bin/sh\n",
            "scripts/exec_with_file_lock.py": "#!/usr/bin/env python3\n",
            "scripts/norito_bridge_source_seal.py": "# fixture\n",
            "kotlin/client-android/build.gradle.kts": "// android\n",
        }.items():
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(contents, encoding="utf-8")
        self.git("init", "-q")
        self.git("config", "user.name", "Source Seal Test")
        self.git("config", "user.email", "source-seal@example.invalid")
        self.git("add", "-A")
        self.git("commit", "-q", "-m", "fixture")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def git(self, *arguments: str) -> bytes:
        environment = os.environ.copy()
        environment["GIT_CONFIG_GLOBAL"] = os.devnull
        environment["GIT_CONFIG_NOSYSTEM"] = "1"
        return subprocess.run(
            ["git", "-C", str(self.root), *arguments],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=environment,
        ).stdout

    def inputs(self, platform: str) -> list[str]:
        with mock.patch.object(seal, "local_dependency_roots", return_value=set()):
            return seal.seal_inputs(self.root, platform)

    def test_apple_seal_includes_package_lock_and_mobile_transports(self) -> None:
        apple = self.inputs("apple")
        self.assertIn("IrohaSwift/Package.swift", apple)
        self.assertIn("IrohaSwift/Package.resolved", apple)
        self.assertIn("IrohaSwift/Sources/IrohaSwift", apple)
        self.assertIn("IrohaSwift/Sources/IrohaSwiftMobileTransports", apple)
        self.assertIn("scripts/exec_with_file_lock.py", apple)
        self.assertIn("scripts/check_mobile_sdk_artifact_pin_commit.py", apple)

        android = self.inputs("android")
        self.assertNotIn("IrohaSwift/Package.resolved", android)
        self.assertNotIn("IrohaSwift/Sources/IrohaSwiftMobileTransports", android)
        self.assertNotIn("scripts/exec_with_file_lock.py", android)
        self.assertIn("scripts/check_mobile_sdk_artifact_pin_commit.py", android)

    def test_apple_fingerprint_normalizes_only_native_bridge_hash_pins(self) -> None:
        inputs = self.inputs("apple")
        original = seal.fingerprint(self.root, inputs)
        loader = self.root / "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"
        contents = loader.read_text(encoding="utf-8")
        loader.write_text(contents.replace("1" * 64, "a" * 64), encoding="utf-8")
        self.assertEqual(original, seal.fingerprint(self.root, inputs))

        loader.write_text(
            loader.read_text(encoding="utf-8") + "let changedLogic = true\n",
            encoding="utf-8",
        )
        self.assertNotEqual(original, seal.fingerprint(self.root, inputs))

    def test_apple_fingerprint_and_dirty_state_bind_mobile_transport_bytes(self) -> None:
        inputs = self.inputs("apple")
        original = seal.fingerprint(self.root, inputs)
        transport = self.root / "IrohaSwift/Sources/IrohaSwiftMobileTransports/Nfc.swift"
        transport.write_text("public struct MutatedNfc {}\n", encoding="utf-8")

        self.assertNotEqual(original, seal.fingerprint(self.root, inputs))
        self.assertIn("Nfc.swift", seal.status(self.root, inputs))

    def test_apple_fingerprint_binds_package_resolution_and_untracked_transport(self) -> None:
        inputs = self.inputs("apple")
        original = seal.fingerprint(self.root, inputs)
        resolved = self.root / "IrohaSwift/Package.resolved"
        resolved.write_text('{"pins":[{"identity":"changed"}],"version":3}\n', encoding="utf-8")
        changed_lock = seal.fingerprint(self.root, inputs)
        self.assertNotEqual(original, changed_lock)

        extra = self.root / "IrohaSwift/Sources/IrohaSwiftMobileTransports/Extra.swift"
        extra.write_text("public struct Extra {}\n", encoding="utf-8")
        self.assertNotEqual(changed_lock, seal.fingerprint(self.root, inputs))
        dirty = seal.status(self.root, inputs)
        self.assertIn("Package.resolved", dirty)
        self.assertIn("Extra.swift", dirty)

    def test_unknown_platform_fails_closed(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "unsupported source-seal platform"):
            self.inputs("windows")

    def test_apple_builder_never_relies_on_the_default_seal_platform(self) -> None:
        builder = APPLE_BUILDER.read_text(encoding="utf-8")
        invocations = re.findall(
            r"run_source_seal (?:fingerprint|status)[^\n]*",
            builder,
        )
        self.assertEqual(len(invocations), 2)
        for invocation in invocations:
            self.assertIn("--platform apple", invocation)

    def test_symlinked_rustup_style_proxies_preserve_dispatch_and_fail_on_retarget(
        self,
    ) -> None:
        tools = self.root / "tools"
        tools.mkdir()
        multiplexer = tools / "rustup"
        replacement = tools / "replacement"
        for executable in (multiplexer, replacement):
            executable.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            executable.chmod(0o755)
        cargo_proxy = tools / "cargo"
        rustc_proxy = tools / "rustc"
        cargo_proxy.symlink_to(multiplexer.name)
        rustc_proxy.symlink_to(multiplexer.name)

        with mock.patch.dict(
            os.environ,
            {
                "NORITO_BRIDGE_SEAL_CARGO": str(cargo_proxy),
                "NORITO_BRIDGE_SEAL_RUSTC": str(rustc_proxy),
            },
            clear=False,
        ):
            cargo = seal.required_tool("NORITO_BRIDGE_SEAL_CARGO", "cargo")
            rustc = seal.required_tool("NORITO_BRIDGE_SEAL_RUSTC", "rustc")

        self.assertEqual(cargo.invocation, cargo_proxy)
        self.assertEqual(rustc.invocation, rustc_proxy)
        canonical_multiplexer = multiplexer.resolve()
        self.assertEqual(cargo.canonical, canonical_multiplexer)
        self.assertEqual(rustc.canonical, canonical_multiplexer)

        completed = subprocess.CompletedProcess([], 0, stdout=b"")
        with mock.patch.object(seal.subprocess, "run", return_value=completed) as run:
            seal.run(self.root, cargo, ["metadata"], {"PATH": str(tools)})
        self.assertEqual(
            run.call_args.args[0],
            [str(cargo_proxy), "metadata"],
        )
        self.assertEqual(
            run.call_args.kwargs["executable"], str(canonical_multiplexer)
        )

        cargo_proxy.unlink()
        cargo_proxy.symlink_to(replacement.name)
        with self.assertRaisesRegex(RuntimeError, "changed after authentication"):
            seal.run(self.root, cargo, ["metadata"], {"PATH": str(tools)})


if __name__ == "__main__":
    unittest.main()
