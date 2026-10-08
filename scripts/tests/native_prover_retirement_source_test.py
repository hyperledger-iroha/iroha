#!/usr/bin/env python3
"""Prevent retired generic proof engines from returning to first-release consumers.

This replaces guards that pinned the deleted Halo2 test shards. Mathematical
and admission behavior is covered by the native Rust proof/mutation suites;
these checks cover source and shipping dependency retirement only. The
temporary independent oracle and the separate Orchard dependency remain out
of scope until their explicitly tracked retirement/carve-out milestones.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
RETIRED_SOURCES = (
    "halo2_backend.rs",
    "halo2_backend_tests.rs",
    "halo2_backend_01_tests.rs",
    "halo2_backend_02_tests.rs",
    "halo2_backend_03_tests.rs",
    "zk1_test_helpers.rs",
    "zkparse.rs",
    "zkparse",
)
MIGRATED_CONSUMERS = (
    "iroha_core_zk",
    "iroha_core",
    "iroha_js_host",
    "kaigi_zk",
    "sorafs_manifest",
)

SHIPPING_ORACLE_GUARD = re.compile(
    r'^\s*const\s+_\s*:\s*\(\s*\)\s*=\s*assert!\(\s*'
    r'!\s*iroha_plonk\s*::\s*ORACLE_BUILD\s*,\s*'
    r'"iroha_plonk_oracle is test-only"\s*,?\s*\)\s*;',
    re.MULTILINE,
)


def has_unconditional_shipping_guard(source: str) -> bool:
    """Accept rustfmt whitespace, but require one first-item oracle exclusion."""
    matches = list(SHIPPING_ORACLE_GUARD.finditer(source))
    if len(matches) != 1:
        return False
    prefix = source[:matches[0].start()]
    # Admit only whitespace, complete comments and crate lint attributes before
    # the assertion. An item attribute, macro body, or incomplete comment cannot
    # hide the assertion from the compiled crate. This intentionally accepts a
    # narrow source shape; CI separately checks actual oracle-mode compilation.
    lint = r"(?:allow|deny|warn|forbid|expect)\s*\([\w\s:,]+\)"
    attribute = re.compile(r"#\s*!\s*\[\s*(?:" + lint + r"|cfg_attr\s*\(\s*test\s*,\s*" + lint + r"\s*\))\s*\]")
    cursor = 0
    while cursor < len(prefix):
        if prefix[cursor].isspace():
            cursor += 1
        elif prefix.startswith('//', cursor):
            end = prefix.find('\n', cursor)
            cursor = len(prefix) if end < 0 else end + 1
        elif prefix.startswith('/*', cursor):
            cursor += 2
            depth = 1
            while cursor < len(prefix) and depth:
                if prefix.startswith('/*', cursor):
                    depth += 1
                    cursor += 2
                elif prefix.startswith('*/', cursor):
                    depth -= 1
                    cursor += 2
                else:
                    cursor += 1
            if depth:
                return False
        else:
            found = attribute.match(prefix, cursor)
            if found is None:
                return False
            cursor = found.end()
    return True


class NativeProverRetirementSourceTest(unittest.TestCase):
    """Require one native consumer path without resurrecting retired dispatch."""

    def test_shipping_relation_and_bridge_roots_forbid_oracle_mode(self) -> None:
        for consumer in ("iroha_core_zk", "iroha_kagemusha_proof", "kaigi_zk",
                         "sorafs_manifest", "connect_norito_bridge"):
            with self.subTest(consumer=consumer):
                source = (ROOT / "crates" / consumer / "src/lib.rs").read_text(encoding="utf-8")
                # The const is the first crate item, unconditional in shipping and test builds.
                # Its actual rejection under an oracle-enabled dependency is compiled by CI.
                self.assertTrue(has_unconditional_shipping_guard(source))

    def test_shipping_guard_accepts_rustfmt_and_refuses_missing_or_conditional_checks(self) -> None:
        flat = 'const _: () = assert!(!iroha_plonk::ORACLE_BUILD, "iroha_plonk_oracle is test-only");'
        formatted = 'const _: () = assert!(\n    !iroha_plonk::ORACLE_BUILD,\n    "iroha_plonk_oracle is test-only"\n);'
        for guard in (flat, formatted, formatted.replace('"\n)', '",\n)')):
            with self.subTest(accepted=guard):
                self.assertTrue(has_unconditional_shipping_guard('//! Consumer.\n#![deny(missing_docs)]\n' + guard))
                self.assertTrue(has_unconditional_shipping_guard('#![cfg_attr(test, allow(clippy::large_stack_arrays))]\n' + guard))
                self.assertTrue(has_unconditional_shipping_guard('/*! Nested /* comment */. */\n' + guard))
        for rejected in (
            '', '// ' + flat, '\n'.join('/// ' + line for line in formatted.splitlines()),
            flat.replace('!iroha_plonk', 'iroha_plonk'),
            flat.replace('assert!', 'debug_assert!'),
            flat.replace('ORACLE_BUILD', 'OTHER_BUILD'),
            flat + '\n' + formatted,
            '#[cfg(test)]\n' + formatted,
            '#[cfg_attr(test, cfg(feature = "optional"))]\n' + formatted,
            '#![cfg(not(iroha_plonk_oracle))]\n' + formatted,
            '#![cfg_attr(test, cfg(feature = "optional"))]\n' + formatted,
            'mod first;\n' + flat,
            'const FIRST: bool = true;\n' + flat,
            '/*\n' + formatted + '\n*/',
            '#[ cfg(test)]\n' + formatted,
            'macro_rules! unused { () => {\n' + formatted + '\n}; }',
            '#![doc = "\n' + formatted + '\n"]',
        ):
            with self.subTest(rejected=rejected):
                self.assertFalse(has_unconditional_shipping_guard(rejected))

    def test_retired_sources_and_module_declarations_are_absent(self) -> None:
        source_dir = ROOT / "crates/iroha_core_zk/src"
        for name in RETIRED_SOURCES:
            self.assertFalse((source_dir / name).exists(), name)
        source = (source_dir / "lib.rs").read_text(encoding="utf-8")
        self.assertNotRegex(source, r"\bmod\s+(?:halo2_backend|zkparse|zk1_test_helpers)\s*;")

    def test_migrated_consumers_do_not_link_vendored_runtime_engines(self) -> None:
        retired = re.compile(r"\b(?:halo2[-_]axiom|halo2[-_]base|halo2[-_]ecc|snark[-_]verifier|halo2_proofs)\b")
        for consumer in MIGRATED_CONSUMERS:
            path = ROOT / "crates" / consumer / "Cargo.toml"
            runtime = False
            for line in path.read_text(encoding="utf-8").splitlines():
                stripped = line.split("#", 1)[0].strip()
                if stripped.startswith("["):
                    runtime = stripped == "[dependencies]" or (
                        stripped.startswith("[target.") and stripped.endswith(".dependencies]")
                    )
                if runtime:
                    self.assertNotRegex(stripped, retired, f"{consumer}: {line}")

    def test_native_engine_wire_discriminants_stay_closed(self) -> None:
        source = (ROOT / "crates/iroha_data_model/src/zk.rs").read_text(encoding="utf-8")
        match = re.search(r"pub enum BackendTag\s*\{([^}]+)\}", source)
        self.assertIsNotNone(match)
        variants = re.findall(r"^\s*([A-Za-z_]\w*)\s*,\s*$", match.group(1), re.MULTILINE)
        self.assertEqual(variants, ["NativePipaRPasta", "Stark"])


if __name__ == "__main__":
    unittest.main()
