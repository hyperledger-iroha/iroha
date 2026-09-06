#!/usr/bin/env python3
"""Guard the first-release source tree against retired developer executables."""

from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
RETIRED_SOURCES = (
    "crates/iroha_cli/src/bin/direct_asset_transfer.rs",
    "crates/iroha_cli/src/bin/ivm_contract_call.rs",
    "crates/iroha_cli/src/bin/split_contract_deploy.rs",
    "crates/iroha_core/examples/bench_dag.rs",
)
RETIRED_TARGETS = (
    'name = "direct_asset_transfer"',
    'name = "ivm_contract_call"',
    'name = "split_contract_deploy"',
)
REPLACEMENT_MARKERS = {
    "crates/iroha_cli/src/main_shared.rs": (
        "fn asset_transfer_instructions(",
    ),
    "crates/iroha_cli/src/ivm_cli.rs": (
        "pub enum Command",
    ),
}


class DeveloperExecutableRetirementSourceTest(unittest.TestCase):
    """Require the retained CLI paths and reject resurrected developer binaries."""

    def test_retired_sources_and_targets_are_absent(self) -> None:
        for relative in RETIRED_SOURCES:
            self.assertFalse((ROOT / relative).exists(), relative)
        manifest = (ROOT / "crates/iroha_cli/Cargo.toml").read_text(encoding="utf-8")
        for target in RETIRED_TARGETS:
            self.assertNotIn(target, manifest)

    def test_replacement_paths_remain_present(self) -> None:
        for relative, markers in REPLACEMENT_MARKERS.items():
            source = (ROOT / relative).read_text(encoding="utf-8")
            for marker in markers:
                self.assertIn(marker, source, f"{relative}: {marker}")


if __name__ == "__main__":
    unittest.main()
