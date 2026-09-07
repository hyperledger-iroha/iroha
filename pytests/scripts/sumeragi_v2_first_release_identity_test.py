"""The first-release witness identity has one source-bound runtime/proof owner."""
from __future__ import annotations

import argparse
import ast
import importlib.util
import sys
import unittest
from pathlib import Path

ROOT_DIR = Path(__file__).resolve().parents[2]
OVERLAY: Path | None = None
OWNER = "crates/iroha_core/src/sumeragi/v2_core/refinement/first_release_witness.rs"
OPERATIONAL = "crates/iroha_core/src/sumeragi/v2_core.rs"
VERUS = "crates/iroha_sumeragi_core/src/verus_proofs/in_flight_first_release_proofs.rs"
CONTRACT = "scripts/formal/sumeragi_v2_proof_ledger_production_trace_evidence_contracts.py"
IDENTITY = "production_in_flight_first_release_source_identity_body"


def source(relative: str) -> str:
    overlay = OVERLAY / relative if OVERLAY is not None else None
    path = overlay if overlay is not None and overlay.is_file() else ROOT_DIR / relative
    return path.read_text(encoding="utf-8")


class FirstReleaseIdentityTest(unittest.TestCase):
    """Every mutation starts with the same passing source/model baseline."""

    @classmethod
    def setUpClass(cls) -> None:
        checker = ROOT_DIR / "scripts/formal/check_sumeragi_v2_proof_ledger.py"
        spec = importlib.util.spec_from_file_location("first_release_identity_checker", checker)
        assert spec is not None and spec.loader is not None
        cls.module = importlib.util.module_from_spec(spec)
        sys.modules[spec.name] = cls.module
        spec.loader.exec_module(cls.module)
        # Candidate checks replace only this pure validator, keeping the owning
        # reviewed-source lexer and token contracts from the current checker.
        cls.contract_source = source(CONTRACT)
        definitions = [
            node for node in ast.parse(cls.contract_source).body
            if isinstance(node, ast.FunctionDef)
            and node.name == "_production_trace_first_release_identity_contract"
        ]
        assert len(definitions) == 1
        if OVERLAY is not None:
            exec(compile(ast.Module(body=definitions, type_ignores=[]), CONTRACT, "exec"), cls.module.__dict__)
        cls.inputs = (
            (ROOT_DIR / "formal/sumeragi_v2/SumeragiV2InFlightFirstRelease.tla").read_bytes(),
            source(OWNER), source(OPERATIONAL), source(VERUS),
        )

    def errors(self, inputs):
        return self.module._production_trace_first_release_identity_contract(*inputs)[0]

    def assert_mutation(self, index: int, old: str, new: str, expected: str) -> None:
        self.assertEqual(self.errors(self.inputs), [])
        changed = list(self.inputs)
        self.assertIn(old, changed[index])
        changed[index] = changed[index].replace(old, new, 1)
        self.assertNotEqual(changed[index], self.inputs[index])
        self.assertEqual(self.errors(changed), [expected])

    def test_actual_model_and_all_consumers_pass(self) -> None:
        errors, declaration = self.module._production_trace_first_release_identity_contract(*self.inputs)
        self.assertEqual(errors, [])
        self.assertIsNotNone(declaration)
        self.assertEqual(declaration.name, IDENTITY)

    def test_changed_model_bytes_fail_even_when_all_consumers_agree(self) -> None:
        self.assertEqual(self.errors(self.inputs), [])
        changed = (self.inputs[0] + b"\n", *self.inputs[1:])
        self.assertNotEqual(changed, self.inputs)
        self.assertEqual(self.errors(changed), ["first-release source identity differs from actual TLA bytes"])

    def test_every_declared_digest_word_is_bound_to_model_bytes(self) -> None:
        digest = self.module._sha256_bytes(self.inputs[0])
        for index in range(4):
            with self.subTest(word=index):
                word = digest[index * 16:(index + 1) * 16]
                self.assert_mutation(1, f"word{index}: 0x{word}u64", f"word{index}: 0u64", "first-release source identity differs from actual TLA bytes")

    def test_constructor_cannot_replace_the_canonical_identity(self) -> None:
        self.assert_mutation(2, f"source_identity: {IDENTITY}!()", "source_identity: ProductionDigest256Projection::default()", "first-release witness constructor is disconnected from its identity owner")

    def test_every_binding_word_must_use_the_canonical_owner(self) -> None:
        for index in range(4):
            with self.subTest(word=index):
                self.assert_mutation(1, f"{IDENTITY}!().word{index}", "0u64", "first-release witness binding is disconnected from its identity owner")

    def test_every_verus_guarantee_must_use_the_canonical_owner(self) -> None:
        for index in range(4):
            with self.subTest(word=index):
                self.assert_mutation(3, f"{IDENTITY}!().word{index}", "0u64", f"first-release Verus source identity word {index} is disconnected")

    def test_declaration_cannot_be_gated_or_duplicated(self) -> None:
        self.assert_mutation(1, f"macro_rules! {IDENTITY}", f"#[cfg(test)]\nmacro_rules! {IDENTITY}", "first-release source identity declaration must be unconditional")
        declaration = self.module.rust_macro_items(self.inputs[1], IDENTITY)[0].source
        self.assert_mutation(1, declaration, declaration + "\n" + declaration, "first-release source identity requires one canonical declaration")
        for index, role in ((2, "production"), (3, "Verus")):
            with self.subTest(role=role):
                self.assert_mutation(index, self.inputs[index], self.inputs[index] + "\n" + declaration, f"first-release source identity is redeclared in {role}")

    def test_retired_operational_constant_cannot_return(self) -> None:
        self.assert_mutation(2, self.inputs[2], self.inputs[2] + "\nconst PRODUCTION_IN_FLIGHT_FIRST_RELEASE_TLA_SOURCE_SHA256: u64 = 0;\n", "first-release source identity retains a duplicate operational constant")

    def test_preflight_calls_the_owner_validator_and_records_its_source(self) -> None:
        snapshot = next(node for node in ast.parse(self.contract_source).body if isinstance(node, ast.FunctionDef) and node.name == "_production_trace_extraction_source_snapshot")
        calls = [node for node in ast.walk(snapshot) if isinstance(node, ast.Call) and isinstance(node.func, ast.Name) and node.func.id == "_production_trace_first_release_identity_contract"]
        self.assertEqual(len(calls), 1)
        self.assertEqual([arg.id for arg in calls[0].args], ["model_payload", "identity_source", "operational_source", "verus_identity_source"])
        self.assertIn('"model_source_identity": identity_entry', self.contract_source)
        self.assertIn('errors.extend(identity_errors)', self.contract_source)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-root", type=Path, default=ROOT_DIR)
    parser.add_argument("--overlay", type=Path)
    arguments, remaining = parser.parse_known_args()
    ROOT_DIR, OVERLAY = arguments.source_root, arguments.overlay
    unittest.main(argv=[sys.argv[0], *remaining])
