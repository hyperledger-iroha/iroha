"""Exact runtime trace contracts reject disconnected readers, fences and witnesses."""
from __future__ import annotations

import argparse
import ast
import importlib.util
import sys
import unittest
from contextlib import contextmanager
from pathlib import Path
from unittest.mock import patch

ROOT_DIR = Path(__file__).resolve().parents[2]
OVERLAY: Path | None = None
CONTRACTS = "scripts/formal/sumeragi_v2_proof_ledger_production_trace_contracts.py"
EVIDENCE = "scripts/formal/sumeragi_v2_proof_ledger_production_trace_evidence_contracts.py"
CHECKER = "scripts/formal/check_sumeragi_v2_proof_ledger.py"
REFINEMENT = "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
DISPATCH = "production_in_flight_first_release_transition_body"
OBSERVE = "IN_FLIGHT_FIRST_RELEASE_ACTION_OBSERVE_REPLICA_QUEUE_RELEASE"


def selected(path: Path) -> Path:
    if OVERLAY is not None:
        try:
            candidate = OVERLAY / path.relative_to(ROOT_DIR)
        except ValueError:
            return path
        if candidate.is_file():
            return candidate
    return path


def source(relative: str) -> str:
    return selected(ROOT_DIR / relative).read_text(encoding="utf-8")


@contextmanager
def candidate_reads(module):
    """Read candidate bytes through the same bounded reader and source lexer."""
    if OVERLAY is None:
        yield
        return
    bounded = module._bounded_regular_file_bytes
    read_bytes = Path.read_bytes
    read_text = Path.read_text
    with (
        patch.object(module, "_bounded_regular_file_bytes", lambda path, **kw: bounded(selected(path), **kw)),
        patch.object(Path, "read_bytes", lambda path: read_bytes(selected(path))),
        patch.object(Path, "read_text", lambda path, *a, **kw: read_text(selected(path), *a, **kw)),
    ):
        yield


class TracePreflightTest(unittest.TestCase):
    """Each rejection is checked against an independently passing validator."""

    @classmethod
    def setUpClass(cls):
        spec = importlib.util.spec_from_file_location("trace_preflight_test_checker", ROOT_DIR / CHECKER)
        assert spec is not None and spec.loader is not None
        cls.module = importlib.util.module_from_spec(spec)
        sys.modules[spec.name] = cls.module
        spec.loader.exec_module(cls.module)
        if OVERLAY is not None:
            tree = ast.parse(source(CHECKER))
            assignment = next(node for node in tree.body if isinstance(node, ast.Assign) and any(isinstance(target, ast.Name) and target.id == "PRODUCTION_TRACE_EXTRACTION_REQUIRED_MODEL_ACTIONS" for target in node.targets))
            readers = [node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name in ("_rust_function_body_span", "rust_items", "rust_function_items_from_structural", "_rust_all_function_items")]
            assert len(readers) == 4
            exec(compile(ast.Module(body=[assignment, *readers], type_ignores=[]), CHECKER, "exec"), cls.module.__dict__)
            for relative in (CONTRACTS, EVIDENCE):
                exec(compile(source(relative), relative, "exec"), cls.module.__dict__)
        cls.bindings = {binding["id"]: binding for binding in cls.module.PRODUCTION_TRACE_EXTRACTION_BINDINGS}
        macros = cls.module.rust_macro_items(source(REFINEMENT), DISPATCH)
        assert len(macros) == 1
        cls.dispatch = macros[0].source

    def tokens(self, value):
        return self.module.rust_code_tokens(value)

    def owner_tokens(self, binding):
        errors = []
        with candidate_reads(self.module):
            item = self.module._production_trace_unique_function(root_dir=ROOT_DIR, relative=binding["path"], symbol=binding["symbol"], impl_name=binding["impl"], errors=errors)
        self.assertEqual(errors, [])
        self.assertIsNotNone(item)
        return self.tokens(item.source)

    def missing(self, tokens, required):
        return [token for token in required if self.module._token_sequence_count(tokens, self.tokens(token)) == 0]

    def replace_tokens(self, original, old, new):
        old_tokens, new_tokens = self.tokens(old), self.tokens(new)
        positions = [i for i in range(len(original)) if original[i:i + len(old_tokens)] == old_tokens]
        self.assertEqual(len(positions), 1, old)
        start = positions[0]
        changed = original[:start] + new_tokens + original[start + len(old_tokens):]
        self.assertNotEqual(changed, original)
        return changed

    def assert_required_mutation(self, binding, required, old, new, expected_missing):
        baseline = self.owner_tokens(binding)
        self.assertEqual(self.missing(baseline, required), [])
        self.assertIsNone(self.module._production_trace_ordered_token_sequence_error(baseline, binding.get("ordered_tokens", ())))
        changed = self.replace_tokens(baseline, old, new)
        self.assertEqual(self.missing(changed, required), expected_missing)

    def test_complete_source_snapshot_authenticates_all_28_actions(self):
        with candidate_reads(self.module):
            snapshot = self.module._production_trace_extraction_source_snapshot()
        mappings = snapshot["operational_correspondence"]["action_mappings"]
        self.assertEqual({item["discriminant"] for item in mappings}, set(range(1, 29)))
        self.assertTrue(all(item["shared_kernel_occurrences"] == 1 for item in mappings))
        binding = next(item for item in snapshot["source_bindings"] if item["id"] == "replica_queue_disposition_observation")
        self.assertTrue(binding["authenticated"])
        for key in ("checked_transition_source", "authorization_source", "checked_transition_consumer", "canonical_commit_sink"):
            self.assertIsNotNone(binding[key], key)
        self.assertEqual(len(binding["supporting_sources"]), 5)

    def test_dispatch_separates_payload_custody_from_actual_activation(self):
        arms = self.module._production_trace_first_release_dispatch_arms(self.dispatch)
        self.assertEqual(len(arms), 28)
        self.assertEqual(arms["IN_FLIGHT_FIRST_RELEASE_ACTION_ACTIVATE_KURA"], 1)
        self.assertEqual(self.module._token_sequence_count(self.tokens(self.dispatch), self.tokens("refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_ACTION_ACTIVATE_KURA)")), 2)
        for old, new, diagnostic in (
            ("after.payload_binding_a == (before.payload_binding_a | projection.actor)", "after.payload_binding_a == before.payload_binding_a", "first-release dispatch lost its exact payload-custody guard"),
            (OBSERVE, "IN_FLIGHT_FIRST_RELEASE_ACTION_UNMAPPED", "first-release dispatch must contain each canonical action exactly once"),
            (OBSERVE, "IN_FLIGHT_FIRST_RELEASE_ACTION_ACTIVATE_KURA", "first-release dispatch must contain each canonical action exactly once"),
        ):
            with self.subTest(mutation=old, replacement=new):
                self.assertEqual(len(self.module._production_trace_first_release_dispatch_arms(self.dispatch)), 28)
                changed = self.dispatch.replace(old, new)
                self.assertNotEqual(changed, self.dispatch)
                with self.assertRaisesRegex(ValueError, "^" + diagnostic + "$"):
                    self.module._production_trace_first_release_dispatch_arms(changed)
        # Replacing only the last (dispatch) occurrence leaves the custody guard.
        old = "IN_FLIGHT_FIRST_RELEASE_ACTION_ACTIVATE_KURA"
        position = self.dispatch.rfind(old)
        changed = self.dispatch[:position] + self.dispatch[position:].replace(old, "IN_FLIGHT_FIRST_RELEASE_ACTION_MISSING_ACTIVATION", 1)
        self.assertNotEqual(changed, self.dispatch)
        with self.assertRaisesRegex(ValueError, "^first-release dispatch must contain each canonical action exactly once$"):
            self.module._production_trace_first_release_dispatch_arms(changed)

    def test_model_inventory_and_replica_observation_contract_remain_exact(self):
        namespace = {}
        exec(compile(source("scripts/formal/sumeragi_v2_multilane_inflight_contract.py"), "inflight_contract", "exec"), namespace)
        self.assertEqual(namespace["INFLIGHT_LAYOUT_REQUIRED_ACTIONS"], self.module.PRODUCTION_TRACE_EXTRACTION_REQUIRED_MODEL_ACTIONS)
        code = source("formal/sumeragi_v2/SumeragiV2InFlightFirstRelease.tla")
        anchors = tuple(token for token in namespace["INFLIGHT_COMPOSED_TLA_ALIGNMENT_TOKENS"] if "ObserveReplicaQueueRelease" in token)
        self.assertEqual(len(anchors), 2)
        for anchor in anchors:
            self.assertEqual(code.count(anchor), 1)
            changed = code.replace(anchor, anchor.replace("ObserveReplicaQueueRelease", "DisconnectedReplicaObservation"))
            self.assertNotEqual(changed, code)
            self.assertEqual(changed.count(anchor), 0)
        action = next(token for token in anchors if token.startswith("ObserveReplicaQueueRelease"))
        for guard in ("decision.releaseOwner \\in Validators \\ {Producer}", "release.pendingPrefix = queue.selectedCount", "release.releasedPrefix = 0", "~release.fifoRestored"):
            self.assertEqual(code.count(action), 1)
            self.assertIn(guard, action)
            changed_action = action.replace(guard, "TRUE")
            changed = code.replace(action, changed_action)
            self.assertNotEqual(changed, code)
            self.assertEqual(changed.count(action), 0)

    def test_dispatch_must_reject_unknown_actions(self):
        self.assertEqual(len(self.module._production_trace_first_release_dispatch_arms(self.dispatch)), 28)
        position = self.dispatch.rfind("false")
        self.assertGreaterEqual(position, 0)
        changed = self.dispatch[:position] + self.dispatch[position:].replace("false", "true", 1)
        self.assertNotEqual(changed, self.dispatch)
        with self.assertRaisesRegex(ValueError, "^first-release dispatch must reject unknown actions$"):
            self.module._production_trace_first_release_dispatch_arms(changed)

    def test_unmapped_action_cannot_be_hidden_from_exact_inventory(self):
        self.assertEqual(len(self.module._production_trace_first_release_dispatch_arms(self.dispatch)), 28)
        mappings = self.module.PRODUCTION_TRACE_EXTRACTION_ACTION_WITNESS_MAPPINGS
        reduced = tuple(entry for entry in mappings if entry[1] != OBSERVE)
        self.assertNotEqual(reduced, mappings)
        with patch.object(self.module, "PRODUCTION_TRACE_EXTRACTION_ACTION_WITNESS_MAPPINGS", reduced):
            with self.assertRaisesRegex(ValueError, "^first-release dispatch must contain each canonical action exactly once$"):
                self.module._production_trace_first_release_dispatch_arms(self.dispatch)

    def test_bootstrap_receipt_requires_propagated_errors_and_exact_proposal(self):
        bridge = next(binding for binding in self.module.PRODUCTION_SNAPSHOT_RECOVERY_BRIDGE_BINDINGS if binding["symbol"] == "complete_autonomous_lifecycle_bootstrap")
        sink = self.bindings["producer_kura_activation"]["commit_sink"]
        for binding in (bridge, sink):
            with self.subTest(role=binding is bridge):
                required = binding["required_tokens"]
                self.assert_required_mutation(binding, required, "receipt.proposal == payload.origin_proposal", "true", ["receipt.proposal == payload.origin_proposal"])
                ordered = binding["ordered_tokens"]
                baseline = self.owner_tokens(binding)
                self.assertIsNone(self.module._production_trace_ordered_token_sequence_error(baseline, ordered))
                receipt = next(token for token in ordered if "read_lane_application_receipt" in token)
                changed = self.replace_tokens(baseline, receipt, receipt.replace(")?", ").ok().flatten()"))
                expected = f"ordered code token {receipt!r} must occur exactly once, found 0"
                self.assertEqual(self.module._production_trace_ordered_token_sequence_error(changed, ordered), expected)
                fence = next(token for token in ordered if "consume_autonomous_lifecycle_bootstrap_completion_fence" in token)
                changed = self.replace_tokens(baseline, fence, "")
                receipt_start = next(i for i in range(len(changed)) if changed[i:i + len(self.tokens(receipt))] == self.tokens(receipt))
                changed = changed[:receipt_start] + self.tokens(fence) + changed[receipt_start:]
                self.assertNotEqual(changed, baseline)
                expected = f"ordered code token {fence!r} moved before its predecessor"
                self.assertEqual(self.module._production_trace_ordered_token_sequence_error(changed, ordered), expected)

    def test_all_fanout_and_late_body_reads_propagate_errors_and_compare_payloads(self):
        for identifier in ("producer_payload_transport_fanout", "producer_payload_fanout_queue_fence", "producer_payload_retransmission_fanout", "authenticated_autonomous_late_body_service"):
            binding = self.bindings[identifier]
            required = (*binding["action_tags"], *binding["additional_tokens"])
            for token in required:
                if ".map_err(" not in token:
                    continue
                with self.subTest(binding=identifier, read=token):
                    self.assert_required_mutation(binding, required, token, token.replace("?", ".ok().flatten()"), [token])
            self.assert_required_mutation(binding, required, "if durable_payload != *payload { return Err", "if false { return Err", ["if durable_payload != *payload { return Err"])
        late = self.bindings["authenticated_autonomous_late_body_service"]
        required = (*late["action_tags"], *late["additional_tokens"])
        for old in ("durable_autonomous.executable_payload != *payload", "is_none_or(|certificate| certificate.certificate != *prepare_qc)"):
            self.assert_required_mutation(late, required, old, "false", [old])

    def readers(self, code, name):
        structural = self.module.mask_rust_comments_and_literals(code)
        return (
            self.module.rust_items(code, name),
            self.module.rust_function_items_from_structural(code, structural, name),
            tuple(item for item in self.module._rust_all_function_items(code) if item.name == name),
        )

    def test_function_readers_skip_contract_branches_and_authenticate_final_body(self):
        expressions = (
            "if flag { left } else { right }",
            "if flag { if nested { left } else { right } } else if second { other } else { final_value }",
            "if if flag { nested } else { second } { left } else { right }",
            "(if flag { left } else { right })",
            "match value { Some(inner) => { inner }, None => { fallback } }",
        )
        for expression in expressions:
            code = "pub proof fn observed(flag: bool)\n    requires if flag { true } else { false },\n    ensures result == " + expression + ", unchanged == old,\n{\n    reveal(exact_kernel);\n    assert(final_obligation);\n}\npub proof fn following() { reveal(other_kernel); }\n"
            with self.subTest(expression=expression):
                for items in self.readers(code, "observed"):
                    self.assertEqual(len(items), 1)
                    item = items[0]
                    self.assertEqual(item.body, "\n    reveal(exact_kernel);\n    assert(final_obligation);\n")
                    self.assertIn("unchanged == old", item.source)
                    self.assertNotIn("following", item.source)
                    self.assertEqual(self.module._token_sequence_count(self.tokens(item.body), self.tokens("reveal(exact_kernel)")), 1)
                changed = code.replace("reveal(exact_kernel)", "reveal(unrelated_kernel)")
                self.assertNotEqual(changed, code)
                for items in self.readers(changed, "observed"):
                    self.assertEqual(len(items), 1)
                    self.assertEqual(self.missing(self.tokens(items[0].body), ("reveal(exact_kernel)",)), ["reveal(exact_kernel)"])
                filtered = self.module._rust_all_function_items(code, references=("exact_kernel",))
                # References select calls in the final body, not contract branches.
                self.assertEqual(filtered, ())
                filtered = self.module._rust_all_function_items(code, references=("reveal",))
                self.assertEqual({item.name for item in filtered}, {"observed", "following"})

    def test_function_readers_preserve_plain_rust_and_reject_unclosed_proofs(self):
        code = 'pub async fn plain(Config { size, .. }: Config) { let text = "}"; if size > 0 { run(); } }'
        for items in self.readers(code, "plain"):
            self.assertEqual(len(items), 1)
            self.assertIn("if size > 0 { run(); }", items[0].body)
        for code in (
            "pub proof fn missing() ensures if flag { true } else { false },",
            "pub proof fn missing() ensures if flag { true } else { false }, { reveal(kernel);",
            "pub fn missing();",
        ):
            for items in self.readers(code, "missing"):
                self.assertEqual(items, ())

    def test_replica_wrapper_fence_and_proof_connections_are_required(self):
        binding = self.bindings["replica_queue_disposition_observation"]
        for support in binding["supporting_sources"]:
            with self.subTest(role=support["role"]):
                required = support["required_tokens"]
                selected_tokens = (required[0], required[-1]) if support["role"] == "exact replica Queue observation theorem" else (required[0],)
                for token in selected_tokens:
                    self.assert_required_mutation(support, required, token, "disconnected_contract", [token])
        consumer = binding["checked_transition_consumer"]
        required = consumer["required_tokens"]
        token = "authorization.consume_for_kura(&cursor_read, &barrier.ordered_keys)"
        self.assert_required_mutation(consumer, required, token, "unfenced_disposition()", [token])
        token = "if observed.after.queue.reservation_state != expected_reservation_state"
        self.assert_required_mutation(consumer, required, token, "if false", [token])
        baseline = self.owner_tokens(consumer)
        ordered = consumer["ordered_tokens"]
        self.assertIsNone(self.module._production_trace_ordered_token_sequence_error(baseline, ordered))
        token = "into_projection()"
        changed = self.replace_tokens(baseline, token, "projection_without_check()")
        self.assertEqual(self.module._production_trace_ordered_token_sequence_error(changed, ordered), f"ordered code token {token!r} must occur exactly once, found 0")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=ROOT_DIR)
    parser.add_argument("--overlay", type=Path)
    args, remaining = parser.parse_known_args()
    ROOT_DIR = args.root.resolve()
    OVERLAY = args.overlay.resolve() if args.overlay is not None else None
    unittest.main(argv=[sys.argv[0], *remaining])
