"""Offline regressions for browser build admission and artifact publication."""

import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location("build_wasm", Path(__file__).with_name("build-wasm.py"))
BUILD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUILD)


class BrowserBuildTests(unittest.TestCase):
    def test_target_flags_leave_native_toolchain_unchanged(self):
        with patch.dict(os.environ, {"CC": "/native/clang", "CC_wasm32-unknown-unknown": "/wrong/clang"}, clear=True):
            env = BUILD.build_environment(Path("/warm/browser"), Path("/sdk/clang"),
                                          Path("/sdk/llvm-ar"), ["-ffreestanding"])
        self.assertEqual(env["CC"], "/native/clang")
        self.assertEqual(env["CC_wasm32_unknown_unknown"], "/sdk/clang")
        self.assertEqual(env["CC_wasm32-unknown-unknown"], "/sdk/clang")
        self.assertEqual(env["AR_wasm32-unknown-unknown"], "/sdk/llvm-ar")
        self.assertEqual(env["CFLAGS_wasm32-unknown-unknown"], "-ffreestanding")
        self.assertEqual(env["CFLAGS_wasm32_unknown_unknown"], "")
        self.assertEqual(env["CARGO_TARGET_DIR"], "/warm/browser")
        self.assertNotIn("RUSTFLAGS", env)

    def test_inherited_abi_flags_fail_before_compilation(self):
        with patch.dict(os.environ, {"RUSTFLAGS": "-Ctarget-feature=+atomics"}, clear=True):
            with self.assertRaisesRegex(ValueError, "unset RUSTFLAGS"):
                BUILD.build_environment(Path("/warm"), Path("/clang"), Path("/ar"), [])

    def test_memory_and_wasi_host_imports_are_not_stubbed(self):
        for module in ["env", "wbg", "wasi_snapshot_preview1", "wasi:random/random@0.2.0"]:
            with self.subTest(module=module), patch.object(
                BUILD, "capture", return_value='[{"module":"' + module + '","name":"memory"}]'
            ):
                with self.assertRaisesRegex(ValueError, "unsupported host module"):
                    BUILD.inspect_imports(Path("/codec.wasm"), Path("/tools/node"))
        with patch.object(BUILD, "capture", return_value='[{"module":"./iroha_js_codec_wasm_bg.js","name":"crypto"}]'):
            BUILD.inspect_imports(Path("/codec.wasm"), Path("/tools/node"))

    def test_sdk_headers_must_be_complete(self):
        with tempfile.TemporaryDirectory() as directory:
            sdk = Path(directory)
            (sdk / "bin").mkdir()
            for name in ["clang", "llvm-ar"]:
                path = sdk / "bin" / name
                path.write_text("tool fixture")
                path.chmod(0o700)
            include = sdk / "share/wasi-sysroot/include/wasm32-wasip1"
            include.mkdir(parents=True)
            for name in ["stdlib.h", "string.h"]:
                (include / name).write_text("header fixture")
            with self.assertRaisesRegex(ValueError, "complete C header"):
                BUILD.c_toolchain(sdk)
            (include / "stdint.h").write_text("header fixture")
            clang, ar, flags = BUILD.c_toolchain(sdk)
            self.assertEqual(clang, sdk / "bin/clang")
            self.assertEqual(ar, sdk / "bin/llvm-ar")
            self.assertIn(str(include), flags)
            self.assertIn("-D__wasi__=1", flags)
            self.assertIn("-ffreestanding", flags)

    def test_standalone_rust_requires_same_sysroot_and_browser_std(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            (root / "bin").mkdir()
            for name in ["rustc", "cargo"]:
                path = root / "bin" / name
                path.write_text("tool fixture")
                path.chmod(0o700)
            libraries = root / "lib/rustlib" / BUILD.TARGET / "lib"
            libraries.mkdir(parents=True)
            outputs = [f"release: {BUILD.RUST_VERSION}", f"cargo {BUILD.RUST_VERSION} (fixture)",
                       str(root), str(libraries)]
            with patch.object(BUILD, "capture", side_effect=outputs):
                with self.assertRaisesRegex(ValueError, "std/core"):
                    BUILD.rust_toolchain(root)
            for crate in ["std", "core"]:
                (libraries / f"lib{crate}-fixture.rlib").write_text("library fixture")
            with patch.object(BUILD, "capture", side_effect=outputs):
                self.assertEqual(BUILD.rust_toolchain(root), (root / "bin/rustc", root / "bin/cargo"))
            wrong = list(outputs)
            wrong[2] = str(root / "different")
            with patch.object(BUILD, "capture", side_effect=wrong):
                with self.assertRaisesRegex(ValueError, "different sysroot"):
                    BUILD.rust_toolchain(root)

    def test_publication_uses_the_distribution_lock_owner(self):
        staging, output, node = Path("/tmp/staged codec"), Path("/tmp/package/wasm"), Path("/tools/node")
        with patch.object(BUILD, "run") as run:
            BUILD.publish(staging, output, node)
        run.assert_called_once_with([
            node, BUILD.PACKAGE / "scripts/publish-browser-codec.mjs",
            "--staging", staging, "--output", output,
        ])


if __name__ == "__main__":
    unittest.main()
