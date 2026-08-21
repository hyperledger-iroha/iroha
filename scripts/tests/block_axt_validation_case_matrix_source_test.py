#!/usr/bin/env python3
"""Protect the name-preserving AXT block-validation rejection matrix."""

from __future__ import annotations

import hashlib
import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = REPO_ROOT / "crates/iroha_core/src/block.rs"
MAX_SOURCE_LINES = 30_566

REGION_START = (
    "        #[derive(Clone, Copy, Debug)]\n"
    "        enum AxtSinglePolicyRejectionCase"
)
REGION_END = (
    "        #[test]\n"
    "        fn axt_validation_rejects_mismatched_commit_heights"
)
REGION_HASH = "48986d39bda50caea7f2adcc304eb0ce770c1f86c16e1fc7e1e8c31481a79750"

MATRIX_CASES = (
    (
        "axt_validation_rejects_handle_clock_skew_above_config",
        "HandleClockSkew",
    ),
    (
        "axt_validation_rejects_duplicate_handle_fragment_key",
        "DuplicateHandleFragmentKey",
    ),
    ("axt_validation_rejects_handle_amount_mismatch", "HandleAmountMismatch"),
    ("axt_validation_rejects_missing_touch_manifest", "MissingTouchManifest"),
    (
        "axt_validation_rejects_handle_without_touch_manifest",
        "HandleWithoutTouchManifest",
    ),
    (
        "axt_validation_rejects_touch_manifest_prefix_violation",
        "TouchManifestPrefixViolation",
    ),
    (
        "axt_validation_rejects_descriptor_binding_mismatch",
        "DescriptorBindingMismatch",
    ),
    (
        "axt_validation_rejects_budget_overspend_across_sub_nonces",
        "BudgetOverspendAcrossSubNonces",
    ),
    (
        "axt_validation_rejects_missing_proof_for_dataspace",
        "MissingProofForDataspace",
    ),
    ("axt_validation_rejects_expired_proof", "ExpiredProof"),
    (
        "axt_validation_rejects_zero_proof_expiry_slot",
        "ZeroProofExpirySlot",
    ),
    (
        "axt_validation_rejects_proof_expiry_before_handle_with_skew",
        "ProofExpiryBeforeHandleWithSkew",
    ),
    (
        "axt_validation_rejects_budget_overspend_in_block",
        "BudgetOverspendInBlock",
    ),
    (
        "axt_validation_rejects_handle_era_below_policy",
        "HandleEraBelowPolicy",
    ),
    (
        "axt_validation_rejects_zero_handle_expiry_slot",
        "ZeroHandleExpirySlot",
    ),
    ("axt_validation_rejects_zero_manifest_root", "ZeroManifestRoot"),
    (
        "axt_validation_rejects_zero_manifest_root_in_policy",
        "ZeroManifestRootInPolicy",
    ),
    (
        "axt_validation_rejects_zero_manifest_root_in_handle",
        "ZeroManifestRootInHandle",
    ),
)

BESPOKE_TEST_HASHES = {
    "axt_validation_rejects_mismatched_commit_heights":
        "832a1e2ce9b8e9688fb5395e42308be66e375665a53a1e7f62bde499ea6f064a",
    "axt_validation_rejects_resultless_block_without_policy_snapshot":
        "1b902371e7e9893101ef7d9a984091c5f9fc9d2bfdddb752419ea36612f865f2",
    "axt_validation_rejects_noncanonical_embedded_policy_snapshots":
        "9be312da6261ceecb9a0c40d96e716fc129623e32c069ba4dc3fb046964c71a0",
    "axt_validation_accepts_cross_lane_handles":
        "dd6e061db7e058f95b571accad0d59b10b1bba7a42d5502be0a9c0a65036ee3f",
    "axt_validation_reuses_one_dataspace_proof_for_two_bound_intents":
        "ea57607d88751d7fc8e27f404c8200e4d29f2fb07380b15e93746d04e76ed87f",
    "axt_validation_accepts_authenticated_hidden_amount":
        "d90374537db0c8f926f1f5684071405ccf9a2b2d94e0e82c365e2b672370af7e",
    "axt_validation_rejects_opaque_authorization_carrier_at_generic_boundary":
        "65459e230970dce76966b41295974d312da2c838ded18e4513f8eac460c1eb05",
    "axt_validation_enforces_registered_asset_balance_policy":
        "ba1dfb49d466125760c266edcccdbaf946df5e54dc1d31bde57d3bc147be77a9",
    "axt_validation_rejects_duplicate_use_of_one_proof_claim":
        "b4f9a6b5c4bb09c9574b8ba8071a1229dcbeaaa7005eb95b79e308ed050cc031",
    "axt_validation_rejects_signed_origin_outside_bound_descriptor":
        "5e68e73626090e29536b5457a432e4232b0eb082b12f2a2c46cfc744dca3c11b",
    "axt_validation_rejects_correctly_signed_handle_for_another_asset":
        "71b289cf3965c6df6588e7be5f7c3e84cad7d99af1ab9da3d97e4fd582e2c382",
    "axt_validation_enforces_exact_block_start_asset_incarnation":
        "79e4e82067f9d340635a4d7cb9e1904f6909f5c604fcc86a4f73771016d48447",
    "axt_validation_rejects_proof_reused_for_another_remote_spend_recipient":
        "84cda80adfffb5cc452104f7113da30d5a0519f965a8e432cf98189e742e916b",
    "axt_validation_rejects_mutated_proof_amount_with_recomputed_commitment":
        "c3552547faf5bb61290414f7ebc1570a9eae42d546c13abbce56fa2d73df1926",
    "axt_validation_rejects_stale_fragment_commitment":
        "91fc4b3405e47d47008987847d5d1faf9acb078454ab0ffcaca491313e26662a",
    "axt_validation_rejects_account_alias_in_remote_spend_intent":
        "a6bae05750bb8a95b4e6ea11975bfa10c1023acc1164988467f43072adf1c42b",
    "axt_validation_accepts_same_replay_and_budget_tuple_from_distinct_dataspaces":
        "7eb9984ecf5c40a94396e6c1697f2b98cdec838e676b1b467fdea4e3a6fde7f9",
    "axt_validation_rejects_raw_manifest_root_proof":
        "441b0b032913293f3d9dc6ac58383833c346a93f4d4a75b7598bd9fda4eb3e1c",
    "axt_validation_rejects_extended_proof_expiry":
        "47efb1dbe587e0c23f31fd68114a33ec69803494bd84f2ee7e720aca27a40368",
    "axt_validation_rejects_manifest_rotation_and_da_relabelling":
        "7a0877403b74cfb2b5b1afe90f456d10d2604708408faabfb4dafce31e03d93a",
    "axt_validation_rejects_manifest_mismatch_in_proof":
        "13f781b7e5850d45b3bbc7fd3b133f7e4c95e2c8be13a19a0afacc0776a1afd2",
    "axt_validation_rejects_proof_dsid_mismatch":
        "327ad232b9f4e597baa417a676be8677d59b33b12d0d53d6e827bef075fb9d25",
    "axt_validation_accepts_authenticated_block_snapshot":
        "c7b57523b52202f6808e17e2e15445dd6034d05028956204f5eb1a61c08f9195",
    "axt_validation_uses_policy_slot_per_dataspace":
        "108deed49d721bf57f57a59ca0681cc9dfc665ef602137697e89b68d015e2a74",
    "axt_validation_rejects_empty_policy_snapshot":
        "6e8e94abf75abc05f10bac97d2fe3bcda60d16285f10faeed32442b5dab45041",
    "axt_validation_rejects_zero_manifest_root_from_snapshot":
        "52432ff64fca545f2202562facf01c55bc9a9386fcab24e3582b42790ec237b5",
    "axt_validation_accepts_hidden_amount_commitment":
        "fe79a9fcc3ec22393dd6609edf8f11f8e83204f06393cf84ebf36c0378c2f07d",
    "axt_validation_rejects_hidden_amount_commitment_mismatch":
        "a0a3e14b38719128bc06d714b7adacf7218bb271f6804f0c65316c13e0898333",
}

SHARED_HELPER_HASHES = {
    "sample_handle":
        "2f963dc46f34877fd2b431d7262d9219eb054963910e4026e100be7c97ad2ddf",
    "proof_blob_for":
        "87f6a919670fed9c3118ad26cea7bffc91c461a21019d6bd122dc9ecd0c23c4c",
    "proof_blob_for_with_amount":
        "fe01c562c6820a2f4ea5e37b59b76b2a98fc8390c59b6fe880608f3b6302e363",
    "proof_blob_for_with_authenticated_amount":
        "27226d9ecc262e8c3fa44c7f8abc94a030d66a535413707af02182a41728df6a",
    "build_block_with_envelopes":
        "ae61d2835072ddb8549e33233c76b8a6d119d82908e49e748e11537f28c9eb7d",
    "axt_policy_snapshot_for_validation_test":
        "5c19b10c8a2065426bc8b1c648c6d47d6411edcf4f9eb0249789e6cfdfdd9dc4",
    "axt_validation_state":
        "9695c139539017c51b864707dcc4dbb22a46c5a3b358c05c9c9beff7ab603397",
    "expect_axt_error":
        "d99f08145f10c87da834c05e096d1efa846285c9d222b74d9a1d3ee5a3723857",
}

EXPECTED_TESTS = frozenset(
    name for name, _variant in MATRIX_CASES
) | frozenset(BESPOKE_TEST_HASHES)

REQUIRED_REGION_TOKENS = (
    'Some((b"handle-clock-skew", 50))',
    'Some((b"duplicate-handle", 12))',
    'Some((b"amount-mismatch", 12))',
    'Some((b"missing-touch", 12))',
    'Some((b"handle-without-touch", 12))',
    'Some((b"touch-prefix", 12))',
    'Some((b"descriptor-binding", 12))',
    'Some((b"overspend-subnonce", 15))',
    'Some((b"expired-proof", 4))',
    'Some((b"zero-proof-expiry", 0))',
    'Some((b"proof-before-handle", 8))',
    'Some((b"budget-block", 10))',
    'Some((b"handle-era", 10))',
    'Some((b"zero-handle-expiry", 10))',
    'Some((b"zero-root-handle", 8))',
    'wrong_bytes[0] ^= 0xFF;',
    'handle.handle.max_clock_skew_ms = Some(1_000);',
    'read: vec!["orders/".to_owned()]',
    'write: vec!["ledger/".to_owned()]',
    'read: vec!["payments/123".to_owned()]',
    'first.handle.sub_nonce = 3;',
    'second.handle.sub_nonce = 4;',
    'payload: vec![0; 32]',
    'handles[0].handle.manifest_view_root = [0x55; 32];',
    'expect_axt_envelope_error(&state, envelope, spec.reason, spec.needle);',
)
FORBIDDEN_REGION_TOKENS = ("Box::new", "Custom(", "=> |", "move |")


class GuardError(AssertionError):
    """Raised when the protected AXT source contract changes."""


def _normalized_hash(source: str) -> str:
    normalized = re.sub(r"\s+", "", source)
    return hashlib.sha256(normalized.encode()).hexdigest()


def _matching_delimiter(source: str, opening: int) -> int:
    pairs = {"{": "}", "[": "]", "(": ")"}
    stack: list[str] = []
    state = "code"
    block_depth = 0
    index = opening
    while index < len(source):
        char = source[index]
        following = source[index + 1] if index + 1 < len(source) else ""
        if state == "code":
            if char == "/" and following == "/":
                state = "line_comment"
                index += 2
                continue
            if char == "/" and following == "*":
                state = "block_comment"
                block_depth = 1
                index += 2
                continue
            if char == '"':
                state = "string"
            elif char == "'":
                state = "char"
            elif char in pairs:
                stack.append(pairs[char])
            elif char in "}])":
                if not stack or stack.pop() != char:
                    raise GuardError(f"mismatched delimiter at byte {index}")
                if not stack:
                    return index
        elif state == "line_comment":
            if char == "\n":
                state = "code"
        elif state == "block_comment":
            if char == "/" and following == "*":
                block_depth += 1
                index += 2
                continue
            if char == "*" and following == "/":
                block_depth -= 1
                index += 2
                if block_depth == 0:
                    state = "code"
                continue
        else:
            if char == "\\":
                index += 2
                continue
            if (state == "string" and char == '"') or (
                state == "char" and char == "'"
            ):
                state = "code"
        index += 1
    raise GuardError("unterminated Rust delimiter")


def _function_source(source: str, name: str) -> str:
    pattern = re.compile(rf"(?m)^[ \t]*fn {re.escape(name)}\s*\(")
    matches = list(pattern.finditer(source))
    if len(matches) != 1:
        raise GuardError(f"{name}: expected one direct function, found {len(matches)}")
    opening = source.find("{", matches[0].end())
    if opening < 0:
        raise GuardError(f"{name}: missing function body")
    closing = _matching_delimiter(source, opening)
    return source[matches[0].start(): closing + 1]


def _unique_region(source: str) -> str:
    if source.count(REGION_START) != 1 or source.count(REGION_END) != 1:
        raise GuardError("AXT matrix region markers must each occur exactly once")
    start = source.index(REGION_START)
    end = source.index(REGION_END, start)
    return source[start:end]


def _matrix_rows(region: str) -> tuple[tuple[str, str], ...]:
    rows = re.findall(
        r"(?m)^\s*(axt_validation_[a-z0-9_]+)\s*=>\s*([A-Za-z0-9_]+),$",
        region,
    )
    return tuple(rows)


def _direct_test_names(source: str) -> tuple[str, ...]:
    return tuple(
        re.findall(r"(?m)^[ \t]*fn (axt_validation_[a-z0-9_]+)\(\) \{", source)
    )


def _require_single_test_attribute(source: str, name: str) -> None:
    marker = f"fn {name}() {{"
    function_at = source.index(marker)
    line_start = source.rfind("\n", 0, function_at) + 1
    preceding = source[:line_start].splitlines()
    if not preceding or preceding[-1].strip() != "#[test]":
        raise GuardError(f"{name}: direct function lost its ordered #[test] attribute")
    if len(preceding) > 1 and preceding[-2].lstrip().startswith("#["):
        raise GuardError(f"{name}: unexpected extra ordered attribute")


def _validate_source(source: str) -> None:
    if len(source.splitlines()) > MAX_SOURCE_LINES:
        raise GuardError("block.rs exceeded the post-consolidation source-line budget")
    region = _unique_region(source)
    if _normalized_hash(region) != REGION_HASH:
        raise GuardError("AXT rejection matrix semantic hash changed")
    if _matrix_rows(region) != MATRIX_CASES:
        raise GuardError("AXT rejection matrix names or typed variants changed")
    if source.count("macro_rules! axt_single_policy_rejection_tests") != 1:
        raise GuardError("AXT name-emitting macro must be declared exactly once")
    if source.count("axt_single_policy_rejection_tests!") != 1:
        raise GuardError("AXT name-emitting macro must be invoked exactly once")
    if "$(\n                    #[test]\n                    fn $name()" not in region:
        raise GuardError("AXT matrix macro no longer emits the exact #[test] attribute")
    for token in REQUIRED_REGION_TOKENS:
        if token not in region:
            raise GuardError(f"AXT rejection matrix lost semantic token: {token}")
    for token in FORBIDDEN_REGION_TOKENS:
        if token in region:
            raise GuardError(f"AXT rejection matrix gained opaque callback token: {token}")

    direct_names = _direct_test_names(source)
    if frozenset(direct_names) != frozenset(BESPOKE_TEST_HASHES):
        raise GuardError("bespoke AXT validation test inventory changed")
    logical_names = [name for name, _variant in MATRIX_CASES] + list(direct_names)
    if len(logical_names) != 46 or len(set(logical_names)) != 46:
        raise GuardError("AXT validation test names must remain 46 unique entries")
    if frozenset(logical_names) != EXPECTED_TESTS:
        raise GuardError("AXT validation test name set changed")

    for name, expected_hash in BESPOKE_TEST_HASHES.items():
        _require_single_test_attribute(source, name)
        observed = _normalized_hash(_function_source(source, name))
        if observed != expected_hash:
            raise GuardError(f"{name}: bespoke body changed")
    for name, expected_hash in SHARED_HELPER_HASHES.items():
        observed = _normalized_hash(_function_source(source, name))
        if observed != expected_hash:
            raise GuardError(f"{name}: shared helper changed")


class BlockAxtValidationCaseMatrixSourceTests(unittest.TestCase):
    """Exercise the AXT source contract and representative mutations."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.source = SOURCE_PATH.read_text()

    def test_current_source_preserves_matrix_and_bespoke_contracts(self) -> None:
        _validate_source(self.source)

    def test_guard_rejects_missing_name_preserving_row(self) -> None:
        row = (
            "            axt_validation_rejects_expired_proof => ExpiredProof,\n"
        )
        self.assertIn(row, self.source)
        with self.assertRaises(GuardError):
            _validate_source(self.source.replace(row, "", 1))

    def test_guard_rejects_adversarial_byte_mutation(self) -> None:
        self.assertIn("wrong_bytes[0] ^= 0xFF;", self.source)
        with self.assertRaises(GuardError):
            _validate_source(
                self.source.replace("wrong_bytes[0] ^= 0xFF;", "wrong_bytes[0] ^= 0xFE;", 1)
            )

    def test_guard_rejects_error_category_mutation(self) -> None:
        original = (
            'Some((b"handle-era", 10)),\n'
            '                        HandleEra,\n'
            '                        "handle era differs from the exact active policy era",'
        )
        mutated = original.replace("HandleEra", "Expiry", 1)
        self.assertIn(original, self.source)
        with self.assertRaises(GuardError):
            _validate_source(self.source.replace(original, mutated, 1))

    def test_guard_rejects_bespoke_assertion_mutation(self) -> None:
        original = "details.snapshot_version, None,"
        self.assertIn(original, self.source)
        with self.assertRaises(GuardError):
            _validate_source(self.source.replace(original, "details.snapshot_version, Some(0),", 1))

    def test_guard_rejects_shared_fixture_mutation(self) -> None:
        original = "State::new_for_testing(World::new(), kura, query)"
        helper = _function_source(self.source, "axt_validation_state")
        self.assertIn(original, helper)
        mutated_helper = helper.replace(
            original,
            "State::new_for_testing(World::new(), kura, query.clone())",
            1,
        )
        with self.assertRaises(GuardError):
            _validate_source(self.source.replace(helper, mutated_helper, 1))

    def test_guard_rejects_opaque_callback_escape_hatch(self) -> None:
        marker = "fn run_axt_single_policy_rejection(case: AxtSinglePolicyRejectionCase) {"
        self.assertIn(marker, self.source)
        mutated = self.source.replace(marker, marker + "\n            // Box::new", 1)
        with self.assertRaises(GuardError):
            _validate_source(mutated)


if __name__ == "__main__":
    unittest.main()
