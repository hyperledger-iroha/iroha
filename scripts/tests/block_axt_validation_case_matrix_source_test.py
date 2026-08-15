#!/usr/bin/env python3
"""Protect the name-preserving AXT block-validation rejection matrix."""

from __future__ import annotations

import hashlib
import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = REPO_ROOT / "crates/iroha_core/src/block.rs"
MAX_SOURCE_LINES = 32_064

REGION_START = (
    "        #[derive(Clone, Copy, Debug)]\n"
    "        enum AxtSinglePolicyRejectionCase"
)
REGION_END = (
    "        #[test]\n"
    "        fn axt_validation_rejects_mismatched_commit_heights"
)
REGION_HASH = "662107aa5a67a88ffa1b4226477186b403cd4ba8a7d4949d8ec4aa81e1b1712e"

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
        "f9bdc9ec4ecaed3e82812050c6b3363659712eebf86d95784e42939a138f9a69",
    "axt_validation_accepts_authenticated_hidden_amount":
        "f2936a4bb7c4fcf69040f64a27d6e83095f9c7ced022a45881e73a6c292951ab",
    "axt_validation_rejects_two_copy_attacker_amount_commitment":
        "019e8e7a8be7a0735ddbc7057c7fc3575f08325d82c71d96655818fd68cfa15e",
    "axt_validation_rejects_stale_fragment_commitment":
        "588142a47facc24d1859f7b33986ff97ecf526439f0e57d9851f07213a74fa56",
    "axt_validation_rejects_duplicate_handle_use_across_dataspaces":
        "1e8a686c0e64670ba9f22b727c5d42e62fde86b7cca0d89df57fc543faadf678",
    "axt_validation_rejects_raw_manifest_root_proof":
        "6335fb820ab2e56f65788dbee3cd38bddf11469f3f9d35e97620df8e7c3862dc",
    "axt_validation_rejects_manifest_mismatch_in_proof":
        "e717c725627d5e78a48da62055efb566e917020f95318f89c4bfefe03ad56fe8",
    "axt_validation_rejects_proof_dsid_mismatch":
        "ab491c7e0f17f37169ecbfc3a3a966be77ecad831a26c184a711de520d17a0c8",
    "axt_validation_accepts_block_snapshot_when_state_cache_empty":
        "1248b5463bb415ff7e27db835557c0e7d9927ed7fb0efaa20c64403b2041c006",
    "axt_validation_uses_policy_slot_per_dataspace":
        "0a949d77247c4e705bda5db959d06c3b8328c226177449931772acbfad8d93d2",
    "axt_validation_rejects_empty_policy_snapshot":
        "6e8e94abf75abc05f10bac97d2fe3bcda60d16285f10faeed32442b5dab45041",
    "axt_validation_rejects_zero_manifest_root_from_snapshot":
        "52432ff64fca545f2202562facf01c55bc9a9386fcab24e3582b42790ec237b5",
    "axt_validation_accepts_hidden_amount_commitment":
        "edaf3249e0692a0b993842897ae1deeed44c80439f97dc5bfe2a3ee437e2d46d",
    "axt_validation_rejects_hidden_amount_commitment_mismatch":
        "9c63b351283174fd25b762ad2575afdce2ebe5831a0b57d21944ee1e8eab8713",
}

SHARED_HELPER_HASHES = {
    "sample_handle":
        "76743d421a1d39d8d3115345478ea4f56bfa9b04b533dd28fa2bb5db52ee6551",
    "proof_blob_for":
        "5b860f61ef654bfc76cc341d75d052c2abba21bf25dc74a712e1c8af3d812485",
    "proof_blob_for_with_amount":
        "6b74aafdb38e35e59cb1a5dcaf405fec08be50968382921ca4aba83c6710c490",
    "proof_blob_for_with_authenticated_amount":
        "2c4acd11286984a0d457f909ed9d0b55820e1e70b5b426dce7c29b351d4b0103",
    "build_block_with_envelopes":
        "f29a80312bf576a8af52c5af2739b8148bca9cc181dd708d4f8348de40bb5a53",
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
    'expect_axt_error(err, spec.reason, spec.needle);',
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
    if len(logical_names) != 35 or len(set(logical_names)) != 35:
        raise GuardError("AXT validation test names must remain 35 unique entries")
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
            'Some((b"handle-era", 10)), HandleEra,\n'
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
