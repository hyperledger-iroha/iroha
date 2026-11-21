#!/usr/bin/env python3
"""Authenticate the first-release typed `InstructionBox` conversion surface."""

from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "crates/iroha_data_model/src/isi/mod.rs"
DIRECT_CALL = re.compile(r"impl_direct_instruction_box!\(([^)]+)\);")


def direct_instruction_types(source: str) -> list[str]:
    """Return each type in invocation order, including repeated types."""
    return [
        path.strip()
        for arguments in DIRECT_CALL.findall(source)
        for path in arguments.split(",")
        if path.strip()
    ]


class InstructionBoxMacroSourceTest(unittest.TestCase):
    """Check the wallet ledger and governed Spartan lifecycle conversion surface."""

    def test_grouped_and_standalone_conversions_preserve_duplicates(self) -> None:
        source = """
            impl_direct_instruction_box!(crate::A, crate::B,);
            impl_direct_instruction_box!(crate::B);
        """
        self.assertEqual(direct_instruction_types(source), ["crate::A", "crate::B", "crate::B"])

    def test_typed_direct_conversions_are_unique(self) -> None:
        source = SOURCE.read_text(encoding="utf-8")
        self.assertIn("macro_rules! impl_direct_instruction_box", source)
        calls = direct_instruction_types(source)
        self.assertTrue(calls)
        self.assertEqual(len(calls), len(set(calls)))

    def test_retired_kagemusha_instructions_are_not_boxable(self) -> None:
        source = SOURCE.read_text(encoding="utf-8")
        kagemusha_calls = {
            call for call in direct_instruction_types(source)
            if "kagemusha" in call.lower()
        }
        self.assertEqual(
            kagemusha_calls,
            {
                "crate::isi::kagemusha_wallet::KagemushaWalletLedgerV1",
                "crate::isi::kagemusha_wallet::spartan::InstallKagemushaSpartanProgramsV1",
                "crate::isi::kagemusha_wallet::spartan::ActivateKagemushaSpartanV1",
                "crate::isi::kagemusha_wallet::spartan::UnloadKagemushaSpartanV1",
            },
        )
        for retired in ("TopUpKagemushaV1", "TopUpKagemushaOrdinaryV1", "RedeemKagemushaV1"):
            self.assertNotIn(retired, source)


if __name__ == "__main__":
    unittest.main()
