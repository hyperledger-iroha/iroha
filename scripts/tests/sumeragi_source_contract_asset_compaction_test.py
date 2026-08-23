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
EXPECTED_CASE_COUNT = 50
BASELINE_RUST_LINES = 5_779
MAX_POSTIMAGE_RUST_LINES = 3_641
MINIMUM_NET_REDUCTION = 2_138
EXPECTED_ASSET_LENGTH = 466_108
EXPECTED_ASSET_SHA256 = "da728945fc0a86eb648a3b29b2a45bf116f21562dbb5907e76acdadcf918c9ec"
EXPECTED_CASE_IDS_SHA256 = "c3f73d112d37828df1ab8c29af1c06b901e5284f1072aca80b9eb21d4c4f4131"

HOST_PREIMAGE_SHA256 = {
    "crates/iroha_core/src/sumeragi/tests/v2_adapter_05_direct_lifecycle_recovered_wal_seal_case.rs": "fe0afaffcbabfeb1d2fdae88d871e380ca1484c80cc2cd0d3a8ce492c6949446",
    "crates/iroha_core/src/sumeragi/tests/v2_lifecycle_replay_authority_cases.rs": "dd5da4ddcbba6cc3aff8faa86f6366ae0bab3bfa06189350e13e2f08db321a58",
    "crates/iroha_core/src/sumeragi/tests/v2_lifecycle_work_registry_exact_registry_cases.rs": "1b3cebc4dd29a624e970ca90f7fa8a2677eb650af9cdd05c48f498a5fd2e1a10",
    "crates/iroha_core/src/sumeragi/tests/v2_lifecycle_work_registry_replay_evidence_cases.rs": "5af2c411d6d1c7d5579004760e8c9ae0b48f2335e1468f456f72e0d614dede6b",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator_support.rs": "0afc9993189c5d118e5da3e9d1b37376687bfd316c66512b1605913b1a1908f4",
}
HOST_POSTIMAGE_SHA256 = {
    "crates/iroha_core/src/sumeragi/tests/v2_adapter_05_direct_lifecycle_recovered_wal_seal_case.rs": "5b3988299c7873cb3cd0cf70f4007007d570cbb324c7c9adbf237ef4fbc6afda",
    "crates/iroha_core/src/sumeragi/tests/v2_lifecycle_replay_authority_cases.rs": "cafb63f1161f2cd95185b4c46ccb6bd7eb09cd0afee0f5c07afa894c39ad4678",
    "crates/iroha_core/src/sumeragi/tests/v2_lifecycle_work_registry_exact_registry_cases.rs": "bc57cf6e598cf680a57ac43295e7efbd5dc44cd036199144a3e325f4fe3b8fa6",
    "crates/iroha_core/src/sumeragi/tests/v2_lifecycle_work_registry_replay_evidence_cases.rs": "c6427c6b098be208556e08222f31507d024f5c63524fb43a5e5c7822b65711e7",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator_support.rs": "75c4bc31584841cf39396c80b93b19e7b10ad4a256e7523ca8ec9a147dad2742",
}

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
        "invalid_body_replay_pre_admission_is_closed_exact_and_lifecycle_owned",
        "live_validate_sign_join_is_linear_opaque_and_scheduler_owned",
        "ready_validate_execution_surface_is_closed_borrow_bound_and_scheduler_owned",
    ),
    "crates/iroha_core/src/sumeragi/tests/v2_lifecycle_work_registry_replay_evidence_cases.rs": (
        "certified_pipeline_replay_evidence_is_retained_by_every_closed_carrier",
    ),
}

NEW_CASE_CONTRACT_COUNTS = {
    "recovered_wal_vote_sign_seal_is_move_only_exact_and_owner_wired": 338,
    "ready_validate_execution_surface_is_closed_borrow_bound_and_scheduler_owned": 196,
    "certified_pipeline_replay_evidence_is_retained_by_every_closed_carrier": 35,
}
MIGRATED_CASE_SHA256 = {
    "recovered_wal_vote_sign_seal_is_move_only_exact_and_owner_wired": "ee0bcd395cfd267eff5ead853ee70308528ebdf568f38a4ec6051e881cfc1f89",
    "ready_validate_execution_surface_is_closed_borrow_bound_and_scheduler_owned": "03b7d7a3a9843536bca8c686937561c0c12eea4281e9850de7ee7c841cf6ac48",
    "certified_pipeline_replay_evidence_is_retained_by_every_closed_carrier": "ddb35ea319aba502839875f5548a17c0329dabe8e6d44e99b865e146417e6e23",
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


class SumeragiSourceContractAssetCompactionTest(unittest.TestCase):
    """Pin the semantic asset, migrated inventory, and honest Rust reduction."""

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

    def test_migrated_test_names_remain_exact_macro_tests(self) -> None:
        for relative_path, names in MIGRATED_TESTS.items():
            source = (ROOT / relative_path).read_text(encoding="utf-8")
            for name in names:
                self.assertNotRegex(source, rf"\bfn\s+{re.escape(name)}\s*\(")
                invocations = re.findall(
                    rf"source_contract_test!\(\s*{re.escape(name)}\s*\)", source
                )
                self.assertEqual(len(invocations), 1)

    def test_postimages_and_line_reduction_are_frozen(self) -> None:
        postimage_lines = 0
        for relative_path, expected_hash in HOST_POSTIMAGE_SHA256.items():
            data = (ROOT / relative_path).read_bytes()
            self.assertEqual(sha256(data), expected_hash)
            postimage_lines += len(data.decode("utf-8").splitlines())
        self.assertLessEqual(postimage_lines, MAX_POSTIMAGE_RUST_LINES)
        self.assertGreaterEqual(BASELINE_RUST_LINES - postimage_lines, MINIMUM_NET_REDUCTION)
        self.assertTrue(all(len(value) == 64 for value in HOST_PREIMAGE_SHA256.values()))

    def test_rust_runner_pins_the_same_closed_inventory(self) -> None:
        support = SUPPORT_PATH.read_text(encoding="utf-8")
        self.assertIn(f"cases.len() != {EXPECTED_CASE_COUNT}", support)
        self.assertIn(f"assert_eq!(ids.len(), {EXPECTED_CASE_COUNT}", support)
        for source_id in ("ReplayAuthorityBase", "ReplayAuthorityCertifiedBody"):
            self.assertIn(source_id, support)


if __name__ == "__main__":
    unittest.main()
