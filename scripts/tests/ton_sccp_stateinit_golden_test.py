#!/usr/bin/env python3
"""Source and artifact guards for the Tolk-emitted TON SCCP StateInit golden."""

from __future__ import annotations

import copy
import importlib.util
import json
import sys
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "generate_ton_sccp_stateinit_golden.py"
SPEC = importlib.util.spec_from_file_location("ton_sccp_stateinit_golden", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
golden = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = golden
SPEC.loader.exec_module(golden)


class TonSccpStateInitGoldenTests(unittest.TestCase):
    """Guard the closed Tolk protocol and the checked-in source provenance."""

    def test_checked_in_golden_is_canonical_and_current(self) -> None:
        data = golden.DEFAULT_OUTPUT.read_bytes()
        value = golden.validate_checked_in_golden(data)
        self.assertEqual(value["schema"], golden.SCHEMA)
        self.assertEqual(value["network"]["sora_profile"], 0x40)
        self.assertEqual(value["network"]["ton_profile"], 0x44)
        self.assertEqual(value["network"]["ton_global_id"], -239)
        self.assertEqual(value["network"]["ton_workchain"], 0)
        self.assertNotEqual(
            value["route"]["state_init_hash"], value["master"]["state_init_hash"]
        )
        for role in ("route", "master"):
            for field in ("code_depth", "initial_data_cell_depth"):
                depth = value[role][field]
                self.assertIs(type(depth), int)
                self.assertGreaterEqual(depth, 0)
                self.assertLessEqual(depth, 0xFFFF)
        self.assertNotEqual(value["route"]["code_depth"], value["master"]["code_depth"])

    def test_parser_rejects_missing_unknown_duplicate_and_reordered_fields(
        self,
    ) -> None:
        value = json.loads(golden.DEFAULT_OUTPUT.read_text(encoding="ascii"))
        payload = golden._line_protocol_from_golden(value)
        lines = payload.decode("ascii").splitlines()
        missing = [line for line in lines if not line.startswith("route_code_depth=")]
        with self.assertRaises(golden.GoldenError):
            golden.parse_line_protocol(("\n".join(missing) + "\n").encode())
        with self.assertRaises(golden.GoldenError):
            golden.parse_line_protocol(payload + b"unknown=1\n")
        with self.assertRaises(golden.GoldenError):
            golden.parse_line_protocol(payload + (lines[-1] + "\n").encode())
        swapped = lines[:]
        swapped[-1], swapped[-2] = swapped[-2], swapped[-1]
        with self.assertRaises(golden.GoldenError):
            golden.parse_line_protocol(("\n".join(swapped) + "\n").encode())

    def test_parser_rejects_noncanonical_or_out_of_range_cell_depths(self) -> None:
        value = json.loads(golden.DEFAULT_OUTPUT.read_text(encoding="ascii"))
        payload = golden._line_protocol_from_golden(value).decode("ascii")
        current = str(value["route"]["code_depth"])
        needle = f"route_code_depth={current}\n"
        self.assertIn(needle, payload)
        for replacement in (
            f"route_code_depth=0{current}\n",
            f"route_code_depth=+{current}\n",
            "route_code_depth=-0\n",
            "route_code_depth=65536\n",
        ):
            with (
                self.subTest(replacement=replacement.strip()),
                self.assertRaises(golden.GoldenError),
            ):
                golden.parse_line_protocol(
                    payload.replace(needle, replacement).encode()
                )

    def test_tolk_emitter_calls_canonical_layout_and_address_constructors(self) -> None:
        source = golden.EMITTER.read_text(encoding="utf-8")
        fixture = golden.FIXTURE.read_text(encoding="utf-8")
        for required in (
            "stateInitGoldenBridgeState()",
            "stateInitGoldenBridgeAddress()",
            "stateInitGoldenMasterState()",
            "stateInitGoldenMasterAddress()",
            "route_initial_data_cell_hash",
            "master_initial_data_cell_hash",
            "bridgeState.code.depth()",
            "bridgeState.data.depth()",
            "masterState.code.depth()",
            "masterState.data.depth()",
        ):
            self.assertIn(required, source)
        for required in (
            "canonicalBridgeInitialStorage(config)",
            "canonicalMasterInitialStorage(config, stateInitGoldenBridgeAddress())",
            'build("TairaXorSccpBridge")',
            'build("TairaXorJettonMaster")',
        ):
            self.assertIn(required, fixture)

    def test_source_closure_is_sorted_unique_and_domain_separated(self) -> None:
        checked_in = json.loads(golden.DEFAULT_OUTPUT.read_text(encoding="ascii"))
        inventory, digest = golden.source_closure(
            checked_in["provenance"]["source_inventory"]
        )
        paths = [item["path"] for item in inventory]
        self.assertEqual(paths, sorted(paths))
        self.assertEqual(len(paths), len(set(paths)))
        self.assertRegex(digest, r"^[0-9a-f]{64}$")
        self.assertIn(
            "contracts/ton/sccp/contracts/storage.tolk",
            paths,
        )
        self.assertIn(
            "contracts/ton/sccp/scripts/generate-stateinit-golden.tolk",
            paths,
        )

    def test_checked_in_golden_validates_without_ignored_acton_outputs(self) -> None:
        data = golden.DEFAULT_OUTPUT.read_bytes()
        direct_lstat = Path.lstat

        def without_generated(path: Path):
            if path in golden.GENERATED_SOURCE_PATHS:
                raise FileNotFoundError(path)
            return direct_lstat(path)

        with mock.patch.object(Path, "lstat", without_generated):
            value = golden.validate_checked_in_golden(data)

        self.assertEqual(value["schema"], golden.SCHEMA)

    def test_fixture_uses_string_for_u128_supply_and_no_legacy_alias(self) -> None:
        value = json.loads(golden.DEFAULT_OUTPUT.read_text(encoding="ascii"))
        self.assertIsInstance(value["configuration"]["max_wrapped_supply"], str)
        self.assertNotIn("bridge", value)
        self.assertIn("route", value)

    def test_checked_in_validator_rejects_address_and_shape_mutations(self) -> None:
        value = json.loads(golden.DEFAULT_OUTPUT.read_text(encoding="ascii"))
        wrong_address = copy.deepcopy(value)
        wrong_address["route"]["address"]["account_hash"] = "11" * 32
        with self.assertRaises(golden.GoldenError):
            golden.validate_checked_in_golden(golden.canonical_json(wrong_address))
        extra = copy.deepcopy(value)
        extra["legacy_bridge_hash"] = "22" * 32
        with self.assertRaises(golden.GoldenError):
            golden.validate_checked_in_golden(golden.canonical_json(extra))

    def test_checked_in_validator_rejects_depth_shape_and_value_mutations(self) -> None:
        value = json.loads(golden.DEFAULT_OUTPUT.read_text(encoding="ascii"))

        missing = copy.deepcopy(value)
        missing["route"].pop("code_depth")
        with self.assertRaises(golden.GoldenError):
            golden.validate_checked_in_golden(golden.canonical_json(missing))

        extra = copy.deepcopy(value)
        extra["master"]["legacy_state_init_depth"] = 0
        with self.assertRaises(golden.GoldenError):
            golden.validate_checked_in_golden(golden.canonical_json(extra))

        for invalid in (True, "1", -1, 0x1_0000):
            mutated = copy.deepcopy(value)
            mutated["route"]["initial_data_cell_depth"] = invalid
            with self.subTest(invalid=invalid), self.assertRaises(golden.GoldenError):
                golden.validate_checked_in_golden(golden.canonical_json(mutated))

        mutated = copy.deepcopy(value)
        old_depth = mutated["route"]["code_depth"]
        mutated["route"]["code_depth"] = (old_depth + 1) & 0xFFFF
        with self.assertRaises(golden.GoldenError):
            golden.validate_checked_in_golden(golden.canonical_json(mutated))

        swapped = copy.deepcopy(value)
        swapped["route"]["code_depth"], swapped["master"]["code_depth"] = (
            swapped["master"]["code_depth"],
            swapped["route"]["code_depth"],
        )
        with self.assertRaises(golden.GoldenError):
            golden.validate_checked_in_golden(golden.canonical_json(swapped))


if __name__ == "__main__":
    unittest.main()
