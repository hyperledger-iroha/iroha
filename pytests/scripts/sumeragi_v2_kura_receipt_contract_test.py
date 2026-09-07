"""Kura receipt source contracts preserve strict observation and publication."""
from __future__ import annotations

import argparse
import ast
import importlib.util
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT_DIR = Path(__file__).resolve().parents[2]
OVERLAY: Path | None = None
CHECKER = "scripts/formal/check_sumeragi_v2_proof_ledger.py"
VALIDATOR = "_kura_application_receipt_production_source_fidelity_errors"
READER = "read_lane_completion_receipt_structural"
WRITER = "write_lane_block_application_receipt_artifact"
RECOVERY = "ensure_bound_progress_pair_has_no_recovery_artifacts_locked"
SLOT = "read_populated_consensus_lane_slot"
DECODE = "read_lane_block_application_receipt_from_bound_locked"


class KuraReceiptContractTest(unittest.TestCase):
    """All mutations start with passing production and focused baselines."""

    @classmethod
    def setUpClass(cls):
        spec = importlib.util.spec_from_file_location("kura_receipt_contract_checker", ROOT_DIR / CHECKER)
        assert spec is not None and spec.loader is not None
        cls.module = importlib.util.module_from_spec(spec)
        sys.modules[spec.name] = cls.module
        spec.loader.exec_module(cls.module)
        if OVERLAY is not None:
            source = (OVERLAY / CHECKER).read_text(encoding="utf-8")
            node = next(node for node in ast.parse(source).body if isinstance(node, ast.FunctionDef) and node.name == VALIDATOR)
            exec(compile(ast.Module(body=[node], type_ignores=[]), CHECKER, "exec"), cls.module.__dict__)
        cls.production_errors = getattr(cls.module, VALIDATOR)(ROOT_DIR)
        path, source, components, errors = cls.module._kura_production_source_inventory(ROOT_DIR)
        assert errors == [], errors
        cls.path = path
        _relative, cls.reads_path, reads_source = next(entry for entry in components if entry[0] == "kura/consensus_storage_reads.rs")
        cls.owners = {}
        for name in (READER, WRITER, RECOVERY, SLOT, DECODE):
            items = cls.module.rust_items(reads_source if name in (READER, SLOT) else source, name)
            assert len(items) == 1, name
            cls.owners[name] = items[0].source

    def errors(self, owners):
        # The complete production inventory is independently checked above.
        # Isolated mutation fixtures retain its exact real method bodies and
        # enclosing impl while exercising the same source-contract validator.
        source = "impl Kura {\n" + "\n".join(value for name, value in owners.items() if name not in (READER, SLOT)) + "\n}\n"
        reads = "impl Kura {\n" + "\n".join(owners[name] for name in (READER, SLOT)) + "\n}\n"
        components = (("kura/consensus_storage_reads.rs", self.reads_path, reads),)
        with patch.object(self.module, "_kura_production_source_inventory", return_value=(self.path, source, components, [])):
            return getattr(self.module, VALIDATOR)(ROOT_DIR)

    def assert_mutation(self, owner, old, new, expected):
        self.assertEqual(self.production_errors, [])
        self.assertEqual(self.errors(self.owners), [])
        self.assertEqual(self.owners[owner].count(old), 1, old)
        changed = dict(self.owners)
        changed[owner] = changed[owner].replace(old, new, 1)
        self.assertNotEqual(changed, self.owners)
        diagnostics = [message.split(": ", 1)[1] for message in self.errors(changed)]
        for message in expected:
            self.assertIn(message, diagnostics)

    def writer_error(self, label):
        return f"locked application-receipt writer must contain exactly one {label}; found 0"

    def owner_error(self, owner):
        return f"receipt strict observation owner {owner} must retain its exact failure semantics must occur exactly 1 time(s) in the real {owner} item; found 0"

    def test_current_production_and_owned_method_baselines_pass(self):
        self.assertEqual(self.production_errors, [])
        self.assertEqual(self.errors(self.owners), [])

    def test_structural_observation_preserves_errors_identity_and_attestation_gate(self):
        exact = "application-receipt writer observation control flow must match the exact reviewed Rust/Verus item body"
        for old, new in (
            ("let entry = self.lane_storage_entry(lane_id)?;", "let entry = self.lane_storage_entry(lane_id).ok()?;"),
            ("self.active_lane_incarnation_marker(&entry)?;", "let _ = self.active_lane_incarnation_marker(&entry);"),
            ("self.ensure_prune_recovery_not_required()?;", "let _ = self.ensure_prune_recovery_not_required();"),
            ("if attest_durability", "if true"),
            ("self.require_active_lane_artifact(&entry, &artifact.proposal.descriptor)?;", "let _ = self.require_active_lane_artifact(&entry, &artifact.proposal.descriptor);"),
            (".open_bound_progress_pair(&data_path, &index_path)?", ".open_bound_progress_pair(&data_path, &index_path).ok()?"),
        ):
            with self.subTest(mutation=old):
                self.assert_mutation(READER, old, new, (exact,))
        prefix = "let _geometry = self.lane_geometry_lock.lock();"
        for call, label in (
            ("recover_bound_progress_sidecar_artifacts", "may not execute sidecar recovery"),
            ("sync_bound_progress_namespace", "may not sync a namespace"),
            ("read_active_lane_block_application_receipt_durability_attested", "may not use an attesting reader"),
        ):
            with self.subTest(call=call):
                self.assert_mutation(READER, prefix, f"self.{call}();\n{prefix}", (exact, f"application-receipt writer observation {label} must occur exactly 0 time(s) in the real {READER} item; found 1"))

    def test_writer_requires_nonattesting_observation_and_exact_locked_reread(self):
        for old, new, label in (
            ("read_lane_completion_receipt_structural(lane_id, lane_block_height, false)?", "read_lane_completion_receipt_structural(lane_id, lane_block_height, true)?", "non-attesting strict observation"),
            ("read_lane_completion_receipt_structural(lane_id, lane_block_height, false)?", "read_lane_completion_receipt_structural(lane_id, lane_block_height, false).ok().flatten()", "non-attesting strict observation"),
            ("if existing != observed_existing", "if false", "exact observation revalidation"),
            ("read_populated_consensus_lane_slot(", "read_candidate_filtered_slot(", "strict occupied-slot reread"),
            ("if observed_existing_matches_evidence", "if false", "competing valid receipt rejection"),
            ("self.read_block_body_under_prune_and_canonical_guards(canonical_height)?;", "let _ = self.read_block_body_under_prune_and_canonical_guards(canonical_height);", "canonical carrier authentication"),
            ("self.require_active_lane_artifact(&entry, descriptor)?;", "let _ = self.require_active_lane_artifact(&entry, descriptor);", "active geometry identity"),
            ("ensure_bound_progress_pair_has_no_recovery_artifacts_locked(", "recover_bound_progress_sidecar_artifacts(", "sidecar lock and recovery-artifact rejection"),
        ):
            with self.subTest(mutation=old):
                self.assert_mutation(WRITER, old, new, (self.writer_error(label),))

    def test_writer_preserves_lock_order_and_strict_retry_and_append_barriers(self):
        canonical = "let _canonical_chain_guard = self.canonical_chain_lock.lock();"
        prune = "let _prune_guard = self.prune_lock.lock();"
        self.assertEqual(self.production_errors, [])
        self.assertEqual(self.errors(self.owners), [])
        changed = dict(self.owners)
        self.assertEqual(changed[WRITER].count(canonical), 1)
        self.assertEqual(changed[WRITER].count(prune), 1)
        changed[WRITER] = changed[WRITER].replace(canonical, "").replace(prune, canonical + "\n" + prune)
        self.assertNotEqual(changed, self.owners)
        messages = [message.split(": ", 1)[1] for message in self.errors(changed)]
        self.assertIn("application-receipt observation, canonical and geometry locks, recovery-artifact rejection, exact-existing barrier reissue, append and attestation must retain their reviewed production order", messages)
        for old, new, label in (
            (".sync_bound_progress_sidecar(&existing_bound,", ".bound_progress_sidecar_unchanged(&existing_bound,", "exact-existing strict barrier reissue"),
            ("if !wrote {", "if wrote {", "bound append and failure propagation"),
            ("!= Some(artifact)", "== Some(artifact)", "exact post-write durability attestation"),
        ):
            with self.subTest(mutation=old):
                self.assert_mutation(WRITER, old, new, (self.writer_error(label),))
        old = "if existing == *artifact {"
        new = "if existing == *artifact { return Ok(()); }\n" + old
        self.assert_mutation(WRITER, old, new, (
            f"locked application-receipt writer exact-existing condition must occur exactly 1 time(s) in the real {WRITER} item; found 2",
            f"locked application-receipt writer success return must occur exactly 1 time(s) in the real {WRITER} item; found 2",
        ))

    def test_shared_owners_preserve_recovery_corruption_and_receipt_identity_errors(self):
        for owner, old, new in (
            (RECOVERY, ".open_optional_bound_progress_file(namespace, &path)?", ".open_optional_bound_progress_file(namespace, &path).ok().flatten()"),
            (RECOVERY, 'Self::bound_progress_append_intent_path(index_path),', ''),
            (SLOT, "if slot.offset != 0", "if false"),
            (SLOT, "if !self.bound_progress_sidecar_unchanged(pair)", "if false"),
            (SLOT, 'decode(pair).ok_or_else(|| invalid("occupied indexed slot is unreadable, malformed, or conflicts with its authority"))?', 'decode(pair)?'),
            (DECODE, "norito::decode_canonical::<LaneBlockApplicationReceiptArtifact>", "decode_unchecked_receipt"),
            (DECODE, "descriptor.lane_id != lane_id || descriptor.lane_block_height != lane_block_height", "descriptor.lane_id != lane_id"),
            (DECODE, "Self::validate_lane_block_application_receipt_artifact(&artifact)", "validate_unchecked_receipt(&artifact)"),
        ):
            with self.subTest(owner=owner, mutation=old):
                self.assert_mutation(owner, old, new, (self.owner_error(owner),))

    def test_release_runner_retains_the_exact_strict_retry_regression(self):
        self.assertEqual(self.production_errors, [])
        self.assertEqual(self.errors(self.owners), [])
        path = ROOT_DIR / "scripts/run_sumeragi_v2_release_gates.sh"
        original = path.read_text(encoding="utf-8")
        name = "kura::tests::progress_witness_durability::lane_block_application_receipt_strict_retry_reissues_every_barrier"
        self.assertEqual(original.count(name), 1)
        changed = original.replace(name, "unqualified_receipt_retry")
        self.assertNotEqual(changed, original)
        read_text = Path.read_text
        with patch.object(Path, "read_text", lambda selected, *args, **kwargs: changed if selected == path else read_text(selected, *args, **kwargs)):
            errors = self.errors(self.owners)
        self.assertEqual(errors, [f"{path}: strict application-receipt retry regression must be pinned exactly once; found 0"])


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=ROOT_DIR)
    parser.add_argument("--overlay", type=Path)
    args, remaining = parser.parse_known_args()
    ROOT_DIR = args.root.resolve()
    OVERLAY = args.overlay.resolve() if args.overlay is not None else None
    unittest.main(argv=[sys.argv[0], *remaining])
