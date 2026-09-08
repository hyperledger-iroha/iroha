"""Retirement source contracts bind all durable progress owners and errors."""
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
VALIDATOR = "_kura_retirement_progress_production_source_fidelity_errors"
INVENTORY = "_KURA_RETIREMENT_FIXED_PROGRESS_PAIR_CONTRACTS"
RETIREMENT = "ensure_first_release_lane_retirement_admissible_with_certified_locked"
RECOVERY = "recover_geometry_progress_pairs_before_snapshot"
PATH_OWNER = "canonical_autonomous_lane_replica_paths_for_entry"
GEOMETRY = "crates/iroha_core/src/kura/lane_geometry.rs"
REPLICA = "crates/iroha_core/src/kura/canonical_autonomous_replica.rs"
MEMBERSHIP = "fixed retirement progress pairs must preserve exact artifact membership, label expressions and order"


class KuraRetirementContractTest(unittest.TestCase):
    """Each mutation changes an exact owner from an independently valid baseline."""

    @classmethod
    def setUpClass(cls):
        spec = importlib.util.spec_from_file_location("kura_retirement_checker", ROOT_DIR / CHECKER)
        assert spec is not None and spec.loader is not None
        cls.module = importlib.util.module_from_spec(spec)
        sys.modules[spec.name] = cls.module
        spec.loader.exec_module(cls.module)
        if OVERLAY is not None:
            nodes = [node for node in ast.parse((OVERLAY / CHECKER).read_text()).body if
                     isinstance(node, ast.FunctionDef) and node.name == VALIDATOR or
                     isinstance(node, ast.Assign) and any(isinstance(target, ast.Name) and target.id == INVENTORY for target in node.targets)]
            assert len(nodes) == 2
            exec(compile(ast.Module(body=nodes, type_ignores=[]), CHECKER, "exec"), cls.module.__dict__)
        cls.production_errors = getattr(cls.module, VALIDATOR)(ROOT_DIR)
        cls.owners = {}
        for relative, names in ((GEOMETRY, (RETIREMENT, RECOVERY)), (REPLICA, (PATH_OWNER,))):
            source = (ROOT_DIR / relative).read_text()
            for name in names:
                items = cls.module.rust_items(source, name)
                assert len(items) == 1, name
                cls.owners[name] = items[0].source
            constants = ("LANE_RETIREMENT_REGULAR_SIDECARS_PER_ROUTE", "LANE_RETIREMENT_FIXED_ARTIFACT_FILES_PER_ROUTE") if relative == GEOMETRY else ("CANONICAL_AUTONOMOUS_LANE_REPLICA_FORMAT_LABEL",)
            for name in constants:
                statements = [item for item in cls.module.rust_top_level_statements(source) if item.tokens[:2] == ("const", name)]
                assert len(statements) == 1, name
                cls.owners[name] = statements[0].source
        source = cls.owners[RETIREMENT]
        start = source.index("let fixed_progress_pairs:")
        cls.array = source[start:source.index("];", start) + 2]

    def errors(self, owners):
        geometry = "\n".join(owners[name] for name in ("LANE_RETIREMENT_REGULAR_SIDECARS_PER_ROUTE", "LANE_RETIREMENT_FIXED_ARTIFACT_FILES_PER_ROUTE"))
        geometry += "\nimpl Kura {\n" + owners[RETIREMENT] + "\n" + owners[RECOVERY] + "\n}\n"
        replica = owners["CANONICAL_AUTONOMOUS_LANE_REPLICA_FORMAT_LABEL"] + "\nimpl Kura {\n" + owners[PATH_OWNER] + "\n}\n"
        replacements = {ROOT_DIR / GEOMETRY: geometry, ROOT_DIR / REPLICA: replica}
        read_text = Path.read_text
        with patch.object(Path, "read_text", lambda path, *args, **kwargs: replacements[path] if path in replacements else read_text(path, *args, **kwargs)):
            return getattr(self.module, VALIDATOR)(ROOT_DIR)

    def baseline(self):
        self.assertEqual(self.production_errors, [])
        self.assertEqual(self.errors(self.owners), [])

    def assert_mutation(self, owner, old, new, expected):
        self.baseline()
        self.assertEqual(self.owners[owner].count(old), 1, (owner, old))
        changed = dict(self.owners)
        changed[owner] = changed[owner].replace(old, new, 1)
        self.assertNotEqual(changed, self.owners)
        messages = [message.split(": ", 1)[1] for message in self.errors(changed)]
        self.assertIn(expected, messages)

    def token_error(self, label, owner=RETIREMENT):
        return f"{label} must occur exactly 1 time(s) in the real {owner} item; found 0"

    def array_mutation(self, old, new, expected=MEMBERSHIP):
        self.assertEqual(self.array.count(old), 1, old)
        self.assert_mutation(RETIREMENT, self.array, self.array.replace(old, new, 1), expected)

    def test_current_production_and_owned_baselines_pass(self):
        self.baseline()

    def test_every_data_and_index_owner_and_label_expression_is_required(self):
        for data, index, _path, kind in getattr(self.module, INVENTORY):
            for value in (f"&{data}", f"&{index}", kind):
                with self.subTest(owner=value):
                    self.array_mutation(value, "UNKNOWN_PROGRESS_OWNER")

    def test_array_membership_cannot_hide_omission_duplication_reordering_or_extra_expressions(self):
        replica = """                (
                    &canonical_replica_data,
                    &canonical_replica_index,
                    CANONICAL_AUTONOMOUS_LANE_REPLICA_FORMAT_LABEL,
                ),
"""
        for replacement in ("", replica + replica, "/*" + replica + "*/", replica.replace("CANONICAL_AUTONOMOUS_LANE_REPLICA_FORMAT_LABEL", '"lane.canonical_autonomous_replica.v1"')):
            with self.subTest(replacement=replacement):
                self.array_mutation(replica, replacement)
        data = self.array.replace("&canonical_replica_data", "&receipt_data").replace("&canonical_replica_index", "&receipt_index")
        self.assert_mutation(RETIREMENT, self.array, data, MEMBERSHIP)
        extra = "(&unknown_data, &unknown_index, unknown_kind()),\n"
        self.array_mutation("];", extra + "];")
        # Swap the two complete tuple bodies; every original token remains.
        receipt = self.array[self.array.index("                (\n                    &receipt_data"):self.array.index("            ];")]
        swapped = self.array.replace(replica, "__REPLICA__").replace(receipt, replica).replace("__REPLICA__", receipt)
        self.assert_mutation(RETIREMENT, self.array, swapped, MEMBERSHIP)

    def test_declaration_bound_and_configured_regular_pair_bound_agree(self):
        count = len(getattr(self.module, INVENTORY))
        self.array_mutation(f"; {count}]", f"; {count - 1}]", self.token_error("fixed retirement progress-pair declaration and bound"))
        name = "LANE_RETIREMENT_REGULAR_SIDECARS_PER_ROUTE"
        self.assert_mutation(name, f"= {count};", f"= {count - 1};", f"retirement owner constant {name} must retain its exact declaration")
        name = "LANE_RETIREMENT_FIXED_ARTIFACT_FILES_PER_ROUTE"
        self.assert_mutation(name, "* 2", "* 1", f"retirement owner constant {name} must retain its exact declaration")

    def test_canonical_pair_uses_its_own_paths_and_format_label(self):
        self.assert_mutation(RETIREMENT, "Self::canonical_autonomous_lane_replica_paths_for_entry(&entry, &self.store_root)", "Self::autonomous_lane_merge_bundle_paths_for_entry(&entry, &self.store_root)", self.token_error("retirement canonical_replica_data/canonical_replica_index path binding"))
        self.assert_mutation(PATH_OWNER, "directory.join(CANONICAL_AUTONOMOUS_LANE_REPLICAS_INDEX_FILE)", "directory.join(AUTONOMOUS_LANE_MERGE_BUNDLES_INDEX_FILE)", "canonical retirement replica paths must bind both files to the active entry must match the exact reviewed Rust/Verus item body")
        name = "CANONICAL_AUTONOMOUS_LANE_REPLICA_FORMAT_LABEL"
        expected = f"retirement owner constant {name} must retain its exact declaration"
        self.assert_mutation(name, '"lane.canonical_autonomous_replica.v1"', '"lane.canonical autonomous_replica.v1"', expected)
        self.assert_mutation(name, "const ", "#[cfg(test)]\nconst ", expected)
        self.assert_mutation(name, self.owners[name], "mod decoy {\n" + self.owners[name] + "\n}\n", expected)

    def test_complete_recovery_precedes_snapshot_and_propagates_failure(self):
        expected = self.token_error("all fixed retirement progress pairs must recover before the immutable snapshot")
        self.assert_mutation(RETIREMENT, "    &fixed_progress_pairs,\n", "    &fixed_progress_pairs[..1],\n", expected)
        old = '                "first-release lane retirement",\n            )?;'
        self.assert_mutation(RETIREMENT, old, old.replace(")?;", ").ok();"), expected)
        source = self.owners[RETIREMENT]
        start = source.index("            let lane_artifacts_guard = self.recover_geometry_progress_pairs_before_snapshot(")
        middle = source.index("            let artifact_snapshot =", start)
        end = source.index("            artifact_files_seen =", middle)
        self.assert_mutation(RETIREMENT, source[start:end], source[middle:end] + source[start:middle], expected)
        self.assert_mutation(RECOVERY, "in pairs {", "in pairs.iter().take(1) {", self.token_error("retirement recovery must visit every pair inside the authenticated directory", RECOVERY))
        old = "                    error_kind,\n                    format!(\"{kind} recovery did not reach a durable fixed point\"),"
        self.assert_mutation(RECOVERY, old, old.replace("error_kind,", "ErrorKind::InvalidData,"), self.token_error("retirement recovery failure classification", RECOVERY))

    def test_pair_and_directory_identity_errors_cannot_be_discarded(self):
        for text, label in (
            ('"{kind} namespace changed during progress recovery"', "retirement recovery must reject a replaced pair namespace"),
            ('"{kind} artifact directory changed during progress recovery"', "retirement recovery must reject a changed directory after each pair"),
            ('"{context} artifact namespace changed before its immutable scan"', "retirement recovery must reject a changed final snapshot directory"),
        ):
            source = self.owners[RECOVERY]
            at = source.index(text)
            start = source.rindex("return Err(", 0, at)
            end = source.index("));", at) + 3
            with self.subTest(label=label):
                rejected = source[start:end]
                ignored = "let _ = " + rejected.removeprefix("return Err(")[:-2] + ";"
                self.assert_mutation(RECOVERY, rejected, ignored, self.token_error(label, RECOVERY))
        self.assert_mutation(RECOVERY, "recovery_directory = refreshed_directory;", "let _ = refreshed_directory;", self.token_error("retirement recovery must rebind the authenticated directory after every pair", RECOVERY))

    def test_per_pair_authentication_recovery_refresh_and_snapshot_order_is_preserved(self):
        source = self.owners[RECOVERY]
        start = source.index("            if let Err(failure)")
        middle = source.index("            if !Self::progress_mutation_namespace_unchanged", start)
        end = source.index("            #[cfg(test)]", middle)
        self.assert_mutation(RECOVERY, source[start:end], source[middle:end] + source[start:middle], "retirement recovery must retain authentication, recovery, per-pair refresh and final snapshot order")

    def test_replica_accounting_authentication_conflict_and_sync_remain_strict(self):
        for old, new, label in (
            ("self.validate_canonical_autonomous_lane_replica_pair_layout_locked(bound)", "self.read_unbounded_replica_heights(bound)", "canonical retirement replica must propagate bounded pair-layout rejection"),
            (".read_canonical_autonomous_lane_replica_from_bound_locked(", ".read_candidate_filtered_replica_record(", "canonical retirement replica must reject corrupt or missing occupied records"),
            ("count_work_items(&mut work_items_seen, canonical_replica_heights.len())?;", "let _ = count_work_items(&mut work_items_seen, canonical_replica_heights.len());", "canonical retirement replica heights must consume the aggregate work bound"),
            ("                &canonical_replica_index,\n            )?;", "                &canonical_replica_index,\n            ).ok();", "canonical retirement replica must open a strict snapshot-bound pair"),
            ("&record.bundle.certified.proposal.descriptor,\n                )?;", "&record.bundle.certified.proposal.descriptor,\n                ).ok();", "canonical retirement replica must propagate active identity and carrier validation failures"),
            ("existing != &record.input", "existing == &record.input", "canonical retirement replica must reject conflicting committee evidence before filling absent records"),
            ("!self.sync_bound_progress_sidecar(\n                    bound,\n                    CANONICAL_AUTONOMOUS_LANE_REPLICA_FORMAT_LABEL,", "self.sync_bound_progress_sidecar(\n                    bound,\n                    CANONICAL_AUTONOMOUS_LANE_REPLICA_FORMAT_LABEL,", "canonical retirement replica must propagate strict durability failure"),
        ):
            with self.subTest(label=label):
                self.assert_mutation(RETIREMENT, old, new, self.token_error(label))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=ROOT_DIR)
    parser.add_argument("--overlay", type=Path)
    args, remaining = parser.parse_known_args()
    ROOT_DIR = args.root.resolve()
    OVERLAY = args.overlay.resolve() if args.overlay is not None else None
    unittest.main(argv=[sys.argv[0], *remaining])
