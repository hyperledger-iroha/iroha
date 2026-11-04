"""Dependency-free forwarding tests; native cryptographic acceptance is tested in Rust."""
from __future__ import annotations

import importlib.util
from pathlib import Path
import sys
from types import ModuleType, SimpleNamespace
import unittest
from unittest.mock import patch


def load_boundary():
    """Load only the wrapper with an explicitly fake native boundary, never SDK crypto."""
    package = ModuleType("_scheduled_boundary_test")
    crypto = ModuleType("_scheduled_boundary_test.crypto")
    crypto._crypto = SimpleNamespace()
    crypto._require_network_id = lambda value, name: value
    crypto._require_chain_discriminant = lambda value, name: value
    source = Path(__file__).parents[1] / "src/iroha_python/scheduled_execution.py"
    spec = importlib.util.spec_from_file_location("_scheduled_boundary_test.scheduled_execution", source)
    module = importlib.util.module_from_spec(spec)
    with patch.dict(sys.modules, {
        "_scheduled_boundary_test": package,
        "_scheduled_boundary_test.crypto": crypto,
    }):
        spec.loader.exec_module(module)
    return module


class ScheduledLifecycleBoundaryTest(unittest.TestCase):
    def setUp(self):
        self.module = load_boundary()
        self.selected = {
            "native_finality_proof_chain_json": "[]",
            "expected_network_id": object(),
            "expected_chain": "selected-chain",
            "trusted_checkpoint": b"checkpoint",
            "expected_chain_discriminant": 753,
        }

    def verify(self, **extra):
        return self.module.verify_scheduled_trigger_execution_v1(
            b"original", "fixture", **self.selected, **extra
        )

    def test_activation_requires_bounded_exact_text_before_native_call(self):
        def unexpected(*args, **kwargs):
            self.fail("invalid witness reached native execution")
        self.module._crypto.verify_scheduled_trigger_execution_v1 = unexpected
        for witness in (False, b"{}", {}, "", "x" * (32 * 1024 * 1024 + 1)):
            with self.subTest(kind=type(witness).__name__), self.assertRaises(ValueError):
                self.verify(lifecycle_activation_json=witness)

    def test_v1_does_not_supply_an_activation_keyword(self):
        called = []
        def native(*args, **kwargs):
            called.append((args, kwargs))
            return ('{"terminal":"pending"}', b"promoted")
        self.module._crypto.verify_scheduled_trigger_execution_v1 = native
        result = self.verify()
        self.assertEqual(result["promoted_checkpoint"], b"promoted")
        self.assertEqual(called[0][1], {"expected_chain_discriminant": 753})

    def test_v2_passes_original_witness_without_claiming_python_authentication(self):
        witness = '{"runtime_proof":"native-verifies-this"}'
        called = []
        def native(*args, **kwargs):
            called.append((args, kwargs))
            return ('{"terminal":"pending"}', b"promoted")
        self.module._crypto.verify_scheduled_trigger_execution_v1 = native
        result = self.verify(lifecycle_activation_json=witness)
        self.assertEqual(result["promoted_checkpoint"], b"promoted")
        self.assertIs(called[0][1]["lifecycle_activation_json"], witness)
        self.assertEqual(called[0][1]["expected_chain_discriminant"], 753)

    def test_policy_is_forwarded_exactly_and_null_height_is_not_omitted(self):
        called = []
        self.module._crypto.verify_scheduled_trigger_execution_v1 = lambda *a, **kw: (
            called.append(kw) or '{"terminal":"pending"}', b"promoted")
        raw = ' {"v2_activation_height":null} '
        self.verify(lifecycle_activation_policy_json=raw)
        self.assertIs(called[0]["lifecycle_activation_policy_json"], raw)
        for value in (False, b"{}", {}, "", "x" * 4097):
            with self.assertRaises(ValueError):
                self.verify(lifecycle_activation_policy_json=value)
        self.assertEqual(len(called), 1)

    def test_native_activation_refusal_is_propagated(self):
        def native(*args, **kwargs):
            raise ValueError("registration did not select the supported lifecycle runtime")
        self.module._crypto.verify_scheduled_trigger_execution_v1 = native
        with self.assertRaisesRegex(ValueError, "supported lifecycle runtime"):
            self.verify(lifecycle_activation_json="{}")


if __name__ == "__main__":
    unittest.main()
