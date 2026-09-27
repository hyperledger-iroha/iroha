"""Exact-byte and fail-closed inventory tests for Torii contract pin generation."""
import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("torii_contract_pins", ROOT / "scripts/update_torii_openapi_contract_pins.py")
PINS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PINS)


class ToriiContractPinTests(unittest.TestCase):
    def test_exact_asset_is_idempotent_and_whitespace_changes_pin(self):
        payload = (ROOT / PINS.ASSET).read_bytes()
        source = (ROOT / PINS.CONSUMER).read_text()
        self.assertEqual(PINS.update_source(payload, source), source)
        changed = PINS.update_source(payload + b"\n", source)
        self.assertNotEqual(changed, source)
        self.assertEqual(PINS.update_source(payload + b"\n", changed), changed)

    def test_removed_reordered_or_unknown_contract_section_rejects(self):
        original = json.loads((ROOT / PINS.ASSET).read_bytes())
        source = (ROOT / PINS.CONSUMER).read_text()
        for change in (lambda value: value["sections"].pop(), lambda value: value["sections"].reverse(), lambda value: value.update(unexpected=True)):
            value = json.loads(json.dumps(original))
            change(value)
            with self.assertRaises(ValueError):
                PINS.update_source(json.dumps(value).encode(), source)
