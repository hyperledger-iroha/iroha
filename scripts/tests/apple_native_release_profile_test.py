#!/usr/bin/env python3
"""Check actual Cargo planning for the static Apple SDK without compiling it."""

from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import subprocess
import unittest

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 uses the scripts dependency.
    import tomli as tomllib

ROOT = Path(__file__).resolve().parents[2]


class AppleNativeReleaseProfileTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
        cls.bridge_manifest = tomllib.loads(
            (ROOT / "crates/connect_norito_bridge/Cargo.toml").read_text()
        )
        environment = dict(os.environ)
        environment["RUSTC_BOOTSTRAP"] = "1"
        command = [
            "cargo", "rustc", "--locked", "--offline", "-Z", "unstable-options",
            "--unit-graph", "-p", "connect_norito_bridge", "--lib",
            "--crate-type", "staticlib", "--profile", "apple-release",
            "--features", "privacy-production-enabled", "--target", "aarch64-apple-darwin",
        ]
        cls.graph = json.loads(subprocess.check_output(command, cwd=ROOT, env=environment))

    def test_final_c_abi_target_can_perform_full_graph_thin_lto(self) -> None:
        self.assertEqual(len(self.graph["roots"]), 1)
        unit = self.graph["units"][self.graph["roots"][0]]
        self.assertEqual(unit["target"]["name"], "connect_norito_bridge")
        self.assertEqual(unit["target"]["crate_types"], ["staticlib"])
        self.assertEqual(unit["features"], ["privacy-production-enabled"])
        self.assertEqual(unit["profile"]["lto"], "thin")
        self.assertEqual(unit["profile"]["opt_level"], "1")
        self.assertEqual(unit["profile"]["panic"], "unwind")
        self.assertFalse(unit["profile"]["incremental"])
        self.assertEqual(unit["profile"]["strip"]["resolved"], "None")

    def test_dependency_optimization_and_host_loadability_are_retained(self) -> None:
        units = self.graph["units"]
        for name, level in (("iroha_data_model", "1"), ("iroha_model_base", "1"),
                            ("iroha_crypto", "3"), ("iroha_core_zk", "3")):
            matches = [unit for unit in units if unit["target"]["name"] == name
                       and unit["platform"] == "aarch64-apple-darwin"]
            self.assertEqual(len(matches), 1, name)
            self.assertEqual(matches[0]["profile"]["opt_level"], level, name)
        host_macros = [unit for unit in units if unit["target"]["kind"] == ["proc-macro"]]
        self.assertTrue(host_macros)
        for unit in host_macros:
            self.assertEqual(unit["profile"]["strip"]["resolved"], "None", unit["target"]["name"])
        self.assertEqual(self.manifest["profile"]["apple-release"]["inherits"], "release")
        self.assertEqual(set(self.bridge_manifest["lib"]["crate-type"]),
                         {"staticlib", "cdylib", "rlib"})

    def test_packaging_limits_remain_the_existing_release_limits(self) -> None:
        specification = importlib.util.spec_from_file_location(
            "norito_renderer_budget", ROOT / "scripts/render_norito_bridge_podspec.py"
        )
        self.assertIsNotNone(specification)
        self.assertIsNotNone(specification.loader)
        renderer = importlib.util.module_from_spec(specification)
        specification.loader.exec_module(renderer)
        self.assertEqual(renderer.MAX_ENTRY_BYTES, 256 * 1024 * 1024)
        self.assertEqual(renderer.MAX_ARCHIVE_BYTES, 512 * 1024 * 1024)
        self.assertEqual(renderer.MAX_TOTAL_UNCOMPRESSED_BYTES, 1024 * 1024 * 1024)

    def test_all_five_slice_commands_and_provenance_paths_use_the_same_profile(self) -> None:
        builder = (ROOT / "scripts/build_norito_xcframework.sh").read_text()
        self.assertEqual(builder.count("--crate-type staticlib --profile apple-release"), 5)
        self.assertIn('$CARGO_TARGET_DIR/$target_triple/apple-release/lib${LIB_CRATE_NAME}.a', builder)
        self.assertIn('--cargo-build-dir "$CARGO_TARGET_DIR/$target_triple/apple-release/build"', builder)
        self.assertNotIn('build --locked --offline --jobs 1 -p "$LIB_CRATE_NAME" --lib --release', builder)
        self.assertIn('-Wl,-all_load "$library"', builder)
        self.assertIn('"$PQCRYPTO_ARCHIVE_NORMALIZER"', builder)


if __name__ == "__main__":
    unittest.main()
