#!/usr/bin/env python3
"""Fail closed on Sumeragi source-contract asset or test-inventory drift."""

from __future__ import annotations

import hashlib
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
ASSET_PATH = ROOT / "crates/iroha_core/src/sumeragi/source_contracts_v1.txt"
SUPPORT_PATH = ROOT / "crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator_support.rs"
SUMERAGI_PATH = ROOT / "crates/iroha_core/src/sumeragi"
EXPECTED_CASE_COUNT = 55
# Pin the reviewed semantic asset. Historical compaction byte counts and host
# hashes belong to Git history: current Rust hosts may add independent tests.
EXPECTED_ASSET_LENGTH = 662_778
EXPECTED_ASSET_SHA256 = "7c1316210d124f01d6eb47601e037c1c59804cf596c3c1a1b7a70c0e5772511c"
EXPECTED_CASE_IDS_SHA256 = "56f95aaddfabd9dd1c08286c64f0e8fe2814c308ad86046342622ff42d85a2df"

MIGRATED_TESTS = {
    "crates/iroha_core/src/sumeragi/tests/v2_adapter_05_direct_lifecycle_recovered_wal_seal_case.rs": (
        "recovered_wal_vote_sign_seal_is_move_only_exact_and_owner_wired",
    ),
    "crates/iroha_core/src/sumeragi/tests/v2_lifecycle_replay_authority_cases.rs": (
        "direct_signed_replay_wrappers_are_opaque_nondecodable_and_fixed_class",
        "remote_proposal_replay_wrappers_are_opaque_exact_and_have_one_runtime_mint",
        "invalid_body_runtime_evidence_is_nondecodable_exact_and_fixed_join_only",
    ),
    "crates/iroha_core/src/sumeragi/tests/v2_lifecycle_work_registry_exact_registry_cases.rs": (
        "remote_proposal_replay_pre_admission_is_closed_exact_and_live",
        "stored_replay_store_coalescing_and_cleanup_are_owner_closed",
        "invalid_body_replay_pre_admission_is_closed_exact_and_lifecycle_owned",
        "live_validate_sign_join_is_linear_opaque_and_scheduler_owned",
        "ready_validate_execution_surface_is_closed_borrow_bound_and_scheduler_owned",
    ),
    "crates/iroha_core/src/sumeragi/tests/v2_lifecycle_work_registry_replay_evidence_cases.rs": (
        "certified_pipeline_replay_evidence_is_retained_by_every_closed_carrier",
    ),
}

# The merged remote-Proposal guards retain exact adapter context/owner and runtime
# publication checks together with the current durable CommitIntent consumer.
# The FIFO case retains the authenticated physical ordering checks from both
# branches. Pin the combined cases; the 55th case still owns body retirement.
NEW_CASE_CONTRACT_COUNTS = {
    "remote_proposal_replay_pre_admission_is_closed_exact_and_live": 174,
    "registry_remains_inert_and_scheduler_free": 89,
    "superseded_certified_body_retirement_is_exact_and_durably_sealed": 90,
    "recovered_wal_vote_sign_seal_is_move_only_exact_and_owner_wired": 338,
    "stored_replay_store_coalescing_and_cleanup_are_owner_closed": 312,
    "ready_validate_execution_surface_is_closed_borrow_bound_and_scheduler_owned": 196,
    "certified_pipeline_replay_evidence_is_retained_by_every_closed_carrier": 35,
    "nonqueue_replica_release_is_fifo_proved_move_only_and_restart_closed": 92,
}
MIGRATED_CASE_SHA256 = {
    "remote_proposal_replay_pre_admission_is_closed_exact_and_live": "77882a8e8fd37e5df2d4257614823819ada01e155c93502c4fd0c3e3a9e3d6df",
    "registry_remains_inert_and_scheduler_free": "941a48e2f28cc22d3167c86a9a9cd58a9e96e4a1d956537a28aa5527109183fe",
    "superseded_certified_body_retirement_is_exact_and_durably_sealed": "bca10f8cce321aba00188cfa24e3b78dd5aebb7fed15d6124bcd51bc6b144d3f",
    "recovered_wal_vote_sign_seal_is_move_only_exact_and_owner_wired": "7e61f7612fa106e3a3649ba8720b172f5d1ec4e901f35c4cf310038b46ba521e",
    "stored_replay_store_coalescing_and_cleanup_are_owner_closed": "8f3f95091ffa52b95610e093ccd34c68bc48895034c13d4cad0ab6eaafc9330c",
    "ready_validate_execution_surface_is_closed_borrow_bound_and_scheduler_owned": "a56c319557fc0fd0eda26924c60de29940a77cb38cbd11ba551a1ec15c131ad5",
    "certified_pipeline_replay_evidence_is_retained_by_every_closed_carrier": "dc5a58896a12211ec735952b05a411112a8fda45ed60923b1b5f114913a14a12",
    "nonqueue_replica_release_is_fifo_proved_move_only_and_restart_closed": "b6afba431c1205460d1601e0dd68f6688a9ca93bce808b88d9ab30733cb81f13",
}


def sha256(data: bytes) -> str:
    """Return a lowercase SHA-256 digest."""

    return hashlib.sha256(data).hexdigest()


def parse_cases(asset: str) -> tuple[tuple[str, tuple[str, ...]], ...]:
    """Parse closed case blocks while retaining their exact row bytes."""

    lines = asset.splitlines()
    if not lines or lines[0] != "sumeragi-source-contracts-v1":
        raise AssertionError("source-contract asset lost its exact v1 header")
    if any(not line for line in lines):
        raise AssertionError("source-contract asset contains a blank row")
    cases: list[tuple[str, tuple[str, ...]]] = []
    current_id: str | None = None
    current_rows: list[str] = []
    for line in lines[1:]:
        if line.startswith("case|"):
            if current_id is not None:
                raise AssertionError(f"unclosed case {current_id}")
            current_id = line.removeprefix("case|")
            if not re.fullmatch(r"[a-z_][a-z0-9_]*", current_id):
                raise AssertionError(f"invalid case ID {current_id!r}")
            current_rows = [line]
        elif line == "end":
            if current_id is None:
                raise AssertionError("orphan case terminator")
            current_rows.append(line)
            cases.append((current_id, tuple(current_rows)))
            current_id = None
            current_rows = []
        else:
            if current_id is None:
                raise AssertionError(f"row outside case: {line!r}")
            tag = line.split("|", 1)[0]
            if tag not in {"region", "required", "forbidden", "count", "order"}:
                raise AssertionError(f"unsupported contract tag {tag!r}")
            current_rows.append(line)
    if current_id is not None:
        raise AssertionError(f"unclosed case {current_id}")
    return tuple(cases)


def macro_inventory_failures(
    sources: dict[str, str], expected_case_ids: set[str]
) -> list[str]:
    """Require one macro per case and preserve each migrated test's host."""

    pattern = re.compile(
        r"source_contract_test!\(\s*"
        r"(?:#\[allow\(clippy::too_many_lines\)\]\s*)?"
        r"([a-z_][a-z0-9_]*)\s*\)"
    )
    invocations = {
        path: pattern.findall(source) for path, source in sources.items()
    }
    actual = [name for names in invocations.values() for name in names]
    failures: list[str] = []
    for name in sorted(expected_case_ids | set(actual)):
        count = actual.count(name)
        if name not in expected_case_ids or count != 1:
            failures.append(f"{name}: expected one known macro, found {count}")
    for path, names in MIGRATED_TESTS.items():
        source = sources.get(path, "")
        for name in names:
            if invocations.get(path, []).count(name) != 1:
                failures.append(f"{path}: missing unique migrated macro {name}")
            if re.search(rf"\bfn\s+{re.escape(name)}\s*\(", source):
                failures.append(f"{path}: re-inlined migrated test {name}")
    return failures


# These are the narrow production regions whose guards moved during the current
# owner reconciliation. Use their exact source providers, without source globbing
# or depending on a compiled Core test executable.
BOUNDARY_CASE_REGIONS = {
    "remote_proposal_replay_pre_admission_is_closed_exact_and_live": (
        "leader_wire_replay_lock_authority",
        "actual_consumer_factory",
        "actual_consumer_publication",
        "leader_wire_live_runtime_cut",
        "leader_wire_live_lock_authority",
        "leader_wire_exact_entered_view",
    ),
    "certified_serve_replay_pair_is_opaque_exact_and_fixed_admission_only": (
        "terminal_next_sign_pair",
    ),
    "selected_certified_response_priority_is_closed_and_exactly_routed": (
        "recovered_response_preparation",
    ),
    "nonqueue_replica_release_is_fifo_proved_move_only_and_restart_closed": (
        "physical_fifo_proof",
    ),
}
BOUNDARY_SOURCE_PATHS = {
    "adapter": "crates/iroha_core/src/sumeragi/v2.rs",
    "effects": "crates/iroha_core/src/sumeragi/v2_effects.rs",
    "leader_wire_consumer": "crates/iroha_core/src/sumeragi/v2_leader_wire_consumer.rs",
    "worker": "crates/iroha_core/src/sumeragi/v2_worker/effect_services_impl.rs",
    "registry": "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery_census_impl.rs",
    "turn_driver": "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
    "queue": "crates/iroha_core/src/queue.rs",
}
BOUNDARY_MUTATIONS = (
    (
        "physical global FIFO inversion",
        "queue",
        "previous_global_fifo_ordinal.is_some_and(|previous| previous >= order.ordinal)",
        "false",
    ),
    (
        "missing paired Sign terminal authentication",
        "registry",
        "!broadcast\n                            .paired_next_sign_matches_terminal_record(coordinator, &exact_ledger)",
        "false",
    ),
    (
        "recovered queue refresh retry",
        "turn_driver",
        "ProductionLifecycleIngressSelectionV1::RecoveredDecisionFetchPreparationRetry,\n                                );",
        "ProductionLifecycleIngressSelectionV1::RestartRequired,\n                                );",
    ),
    (
        "actual adapter factory delegation",
        "adapter",
        "LeaderWireRecoveryAuthority::from_adapter(self)",
        "LeaderWireRecoveryAuthority::from_replayed_adapter(self)",
    ),
    (
        "actual locked proposal round",
        "leader_wire_consumer",
        "adapter.registry.round_to_wire(certificate.proposal_round())",
        "adapter.registry.round_to_wire(certificate.round())",
    ),
    (
        "historical CommitIntent authority",
        "leader_wire_consumer",
        "durable.commit_intent_for_lock(locked).is_some()",
        "true",
    ),
    (
        "exact WAL persistence frontier",
        "leader_wire_consumer",
        "wal_id: durable.last_id()",
        "wal_id: reducer::PersistenceId::new(0)",
    ),
    (
        "monotonic consumer publication",
        "worker",
        "if !next.monotonically_extends(self.leader_wire_recovery_authority)",
        "if false",
    ),
    (
        "persist before exposing consumer authority",
        "worker",
        "self.leader_wire_ingress\n            .advance_leader_wire_recovery_cut(next)?;\n        self.leader_wire_recovery_authority = next;",
        "self.leader_wire_recovery_authority = next;\n        self.leader_wire_ingress\n            .advance_leader_wire_recovery_cut(next)?;",
    ),
    (
        "entered-view exact consumer tag",
        "leader_wire_consumer",
        "self.consumer_tag == tag && self.protected_lock == protected_lock",
        "true && self.protected_lock == protected_lock",
    ),
    (
        "entered-view exact protected lock",
        "leader_wire_consumer",
        "self.consumer_tag == tag && self.protected_lock == protected_lock",
        "self.consumer_tag == tag && true",
    ),
    (
        "entered-view exact consumer and lock",
        "worker",
        ".matches_entered_view(tag, protected_lock)",
        ".matches_entered_view(tag, None)",
    ),
)


def boundary_contract_failures(
    cases: dict[str, tuple[str, ...]], sources: dict[str, str]
) -> list[str]:
    """Evaluate only the relocated guards against real or adversely changed sources.

    Region, count and order behavior matches the Rust runner. Restricting this
    check to explicit regions lets negative controls expose lost ownership
    guards without treating an unrelated source-contract failure as rejection.
    """

    def unescape(value: str) -> str:
        return re.sub(
            r"\\([\\nrtp])",
            lambda match: {"\\": "\\", "n": "\n", "r": "\r", "t": "\t", "p": "|"}[match[1]],
            value,
        )

    failures: list[str] = []
    for case_id, region_ids in BOUNDARY_CASE_REGIONS.items():
        rows = [
            [unescape(field) for field in row.split("|")]
            for row in cases[case_id][1:-1]
            if row.split("|", 2)[1] in region_ids
        ]
        regions: dict[str, list[str]] = {}
        for row in rows:
            if row[0] != "region":
                continue
            _, region, source_id, start_mode, start_token, end_mode, end_token = row
            source = sources[source_id]
            try:
                if start_mode == "last":
                    end = len(source) if end_mode == "end" else source.index(end_token)
                    start = source.rindex(start_token, 0, end)
                else:
                    start = (
                        0 if start_mode == "begin" else source.index(start_token)
                        + (len(start_token) if start_mode == "after" else 0)
                    )
                    end = len(source) if end_mode == "end" else source.index(end_token, start)
                regions.setdefault(region, []).append(source[start:end])
            except ValueError:
                failures.append(f"{case_id}/{region}: missing region delimiter")
        for row in rows:
            tag, region, *fields = row
            if tag == "region":
                continue
            parts = regions.get(region, [])
            valid = bool(parts)
            if tag == "required":
                valid &= any(fields[0] in part for part in parts)
            elif tag == "forbidden":
                valid &= all(fields[0] not in part for part in parts)
            elif tag == "count":
                valid &= sum(part.count(fields[0]) for part in parts) == int(fields[1])
            elif tag == "order":
                if len(fields) != int(fields[0]) + 2:
                    raise AssertionError(f"malformed order row: {row}")
                remaining = "\n".join(parts)
                for anchor in fields[1:-1]:
                    offset = remaining.find(anchor)
                    if offset < 0:
                        valid = False
                        break
                    remaining = remaining[offset + len(anchor):]
            else:
                raise AssertionError(f"unsupported boundary row: {row}")
            if not valid:
                failures.append(f"{case_id}/{region}: {fields[-1]}")
        for region in region_ids:
            if not any(row[0] != "region" and row[1] == region for row in rows):
                failures.append(f"{case_id}/{region}: lost all relocated guards")
    return failures


class SumeragiSourceContractAssetCompactionTest(unittest.TestCase):
    """Pin the semantic asset and migration inventory without freezing hosts."""

    def test_asset_bytes_case_inventory_and_new_contract_counts_are_exact(self) -> None:
        asset_bytes = ASSET_PATH.read_bytes()
        self.assertEqual(len(asset_bytes), EXPECTED_ASSET_LENGTH)
        self.assertEqual(sha256(asset_bytes), EXPECTED_ASSET_SHA256)
        cases = parse_cases(asset_bytes.decode("utf-8"))
        ids = tuple(case_id for case_id, _ in cases)
        self.assertEqual(len(ids), EXPECTED_CASE_COUNT)
        self.assertEqual(len(set(ids)), EXPECTED_CASE_COUNT)
        self.assertEqual(sha256(("\n".join(ids) + "\n").encode()), EXPECTED_CASE_IDS_SHA256)
        by_id = dict(cases)
        for case_id, expected_count in NEW_CASE_CONTRACT_COUNTS.items():
            rows = by_id[case_id]
            contracts = [row for row in rows if row.split("|", 1)[0] in {"required", "forbidden", "count", "order"}]
            self.assertEqual(len(contracts), expected_count)
            self.assertEqual(sha256(("\n".join(rows) + "\n").encode()), MIGRATED_CASE_SHA256[case_id])

    @staticmethod
    def source_inventory() -> tuple[dict[str, str], set[str]]:
        """Load only Sumeragi Rust source and the reviewed case identifiers."""

        sources = {
            str(path.relative_to(ROOT)): path.read_text(encoding="utf-8")
            for path in sorted(SUMERAGI_PATH.rglob("*.rs"))
        }
        cases = parse_cases(ASSET_PATH.read_text(encoding="utf-8"))
        return sources, {case_id for case_id, _ in cases}

    def test_every_asset_case_has_one_rust_macro_test_in_its_host(self) -> None:
        sources, expected = self.source_inventory()
        self.assertEqual(macro_inventory_failures(sources, expected), [])

    def test_macro_inventory_rejects_missing_duplicate_and_reinlined_tests(self) -> None:
        sources, expected = self.source_inventory()
        path = next(iter(MIGRATED_TESTS))
        name = MIGRATED_TESTS[path][0]
        match = re.search(
            rf"source_contract_test!\(\s*{re.escape(name)}\s*\)", sources[path]
        )
        self.assertIsNotNone(match)
        invocation = match.group()
        mutations = {
            "missing": sources[path].replace(invocation, "", 1),
            "duplicate": sources[path] + "\n" + invocation + ";\n",
            "re-inlined": sources[path] + f"\n#[test]\nfn {name}() {{}}\n",
        }
        for label, source in mutations.items():
            with self.subTest(mutation=label):
                mutated = dict(sources)
                mutated[path] = source
                failures = macro_inventory_failures(mutated, expected)
                self.assertTrue(failures, f"inventory accepted {label} migrated test")
                self.assertTrue(any(name in failure for failure in failures))

    def test_macro_inventory_accepts_unrelated_test_addition(self) -> None:
        sources, expected = self.source_inventory()
        path = next(iter(MIGRATED_TESTS))
        sources[path] += (
            "\n#[test]\nfn unrelated_regression() {\n"
            "    assert_eq!(2 + 2, 4);\n}\n"
        )
        self.assertEqual(macro_inventory_failures(sources, expected), [])

    def test_rust_runner_pins_the_same_closed_inventory(self) -> None:
        support = SUPPORT_PATH.read_text(encoding="utf-8")
        self.assertIn(f"cases.len() != {EXPECTED_CASE_COUNT}", support)
        self.assertIn(f"assert_eq!(ids.len(), {EXPECTED_CASE_COUNT}", support)
        for source_id in (
            "ReplayAuthorityBase",
            "ReplayAuthorityCertifiedBody",
            "BodyRetirement",
            "EffectsBodyRetirement",
            "RegistryBodyRetirement",
            "LeaderWireConsumer",
        ):
            self.assertIn(source_id, support)
        self.assertIn('"leader_wire_consumer" => Self::LeaderWireConsumer', support)
        self.assertIn(
            'SourceId::LeaderWireConsumer => include_str!("v2_leader_wire_consumer.rs").to_owned()',
            support,
        )
        cases = dict(parse_cases(ASSET_PATH.read_text(encoding="utf-8")))
        sources = {
            source_id: (ROOT / relative_path).read_text(encoding="utf-8")
            for source_id, relative_path in BOUNDARY_SOURCE_PATHS.items()
        }
        self.assertEqual(boundary_contract_failures(cases, sources), [])
        for label, source_id, before, after in BOUNDARY_MUTATIONS:
            with self.subTest(boundary=label):
                self.assertEqual(sources[source_id].count(before), 1, label)
                mutated = dict(sources)
                mutated[source_id] = sources[source_id].replace(before, after, 1)
                self.assertTrue(
                    boundary_contract_failures(cases, mutated),
                    f"source contracts accepted removal of {label}",
                )


if __name__ == "__main__":
    unittest.main()
