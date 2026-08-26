#!/usr/bin/env python3
"""Guard the Halo2 backend test-shard shared-circuit compaction.

The two audited shards reuse the fixed ``zk::pasta_tiny`` circuits instead of
redeclaring equivalent Halo2 ``Circuit`` implementations inside individual
tests.  This guard authenticates the preimages and current test inventories,
pins the shared circuit implementations, and separately authenticates the
callback-bearing permutation test outside the compaction.
"""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import unittest
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
ZK_PATH = ROOT / "crates/iroha_core/src/zk.rs"
SHARD_01_PATH = ROOT / "crates/iroha_core/src/zk/halo2_backend_01_tests.rs"
SHARD_03_PATH = ROOT / "crates/iroha_core/src/zk/halo2_backend_03_tests.rs"


@dataclass(frozen=True)
class ShardContract:
    """Authenticated source contract for one compacted Halo2 test shard."""

    path: Path
    preimage_blob: str
    preimage_sha256: str
    opening_lines: int
    line_ceiling: int
    postimage_sha256: str
    test_contract_sha256: str
    tests: tuple[str, ...]


SHARDS = (
    ShardContract(
        path=SHARD_01_PATH,
        preimage_blob="4bef4a3fd87ffe9b1451c8f922b6278b2b6936f0",
        preimage_sha256=(
            "f131f0e3c9efeeb90364bce5c6679bf8b2ca4b3d3d0b277e5e1307b2bcaa6fe6"
        ),
        opening_lines=1_710,
        line_ceiling=1_065,
        postimage_sha256=(
            "ae11bcdbb3754eafe12d08c0e62e3409347f188502e8ba543f0522955d1863d2"
        ),
        test_contract_sha256=(
            "2c6a684d7d56cefd9b85ca18d034972416caf5ace14db5fda1d46c7cf5c6d20a"
        ),
        tests=(
            "vote_bool_commit_merkle8_mock_prover_succeeds",
            "constrained_pow5_vote_membership_rejects_a_forged_commitment",
            "commit_open_rejects_additive_placeholder_commitment",
            "tiny_merkle2_rejects_additive_placeholder_root",
            "anon_transfer_commit_rejects_unshifted_placeholder_commitment",
            "vote_bool_merkle2_rejects_stale_merkle_shortcut",
            "vk_cache_reuses_entries",
            "verifier_key_cache_rejects_parseable_key_for_another_circuit",
            "packaged_vk_cache_rejects_unparseable_key_without_runtime_keygen",
            "zk1_envelope_pasta_ipa_verify_add_public",
            "kaigi_roster_backend_accepts_valid_proof",
            "kaigi_usage_backend_accepts_valid_proof",
            "proof_hash_stable",
            "proof_and_vk_hash_domains_are_distinct",
            "proof_hash_length_prefixes_backend_and_payload",
            "dedup_works",
            "hash_vk_stable",
            "preverify_basic",
            "halo2_gate_requires_vk_and_valid_encoding",
            "halo2_end_to_end_proof_verification",
            "halo2_verify_with_instance_add_kzg",
            "halo2_verify_add_2rows_kzg",
            "halo2_verify_id_public_kzg_with_and_without_inst",
            "halo2_verify_ipa_acceptance_variants",
            "halo2_verify_add_2rows_ipa",
            "halo2_verify_add3_ipa",
        ),
    ),
    ShardContract(
        path=SHARD_03_PATH,
        preimage_blob="84b646ef70d4769682a8c542686f4cb76fc2a5d6",
        preimage_sha256=(
            "2bf04114dd343ce6533185813d3bd3dea1bb1bd393f65b67bdb85e351bc21858"
        ),
        opening_lines=1_904,
        line_ceiling=906,
        postimage_sha256=(
            "257c06020fa11f94c6aef99fc61e7d85c30991cb49f278e4453a76de01af679a"
        ),
        test_contract_sha256=(
            "e0a813fba2f82587efbb55b2e655dee3b1497833a13b9802491a78012a55e842"
        ),
        tests=(
            "halo2_verify_anon_transfer_2x2_merkle8_pow5_ipa_zk1_noncanonical",
            "halo2_verify_anon_transfer_2x2_merkle8_pow5_ipa_zk1_invalid_header",
            "halo2_verify_tiny_commit_open_ipa_zk1_truncated_prof",
            "halo2_verify_tiny_merkle2_ipa_zk1_invalid_header_extreme",
            "halo2_verify_tiny_commit_open_ipa_zk1_positive",
            "halo2_verify_tiny_merkle2_ipa_zk1_positive",
            "halo2_verify_tiny_commit_open_ipa_zk1_multiple_prof_and_unknown_rejects",
            "halo2_verify_tiny_commit_open_ipa_zk1_unknown_tlv_stress_rejects",
            "halo2_verify_tiny_commit_open_ipa_zk1_duplicate_i10p_last_correct_rejects",
            "halo2_verify_tiny_commit_open_ipa_zk1_duplicate_i10p_last_wrong_rejects",
            "halo2_verify_tiny_commit_open_ipa_zk1_unknown_tlv_randomized_rejects",
            "halo2_verify_tiny_commit_open_ipa_zk1_permutation_harness",
            "halo2_verify_zk1_prof_length_exceeds_cap_rejected",
            "halo2_verify_add2inst_public_ipa",
            "halo2_verify_anon_transfer_ipa",
            "halo2_verify_vote_bool_ipa",
            "halo2_verify_id_public_ipa_with_and_without_inst",
            "halo2_verify_with_instance_add_ipa",
            "halo2_verify_with_instance_mul_kzg",
            "halo2_verify_with_instance_malformed_length_kzg",
            "halo2_verify_with_instance_noncanonical_kzg",
        ),
    ),
)

OPENING_LINES = sum(shard.opening_lines for shard in SHARDS)
LINE_CEILING = sum(shard.line_ceiling for shard in SHARDS)
MINIMUM_REDUCTION = 1_500

SHARED_CIRCUIT_SHA256 = {
    "Add": "a7465fce4eabbe8a21825ae6b647d022be7cc8ec45987da9b4ad3c1d96b5f0b0",
    "Mul": "7d185cd3eca3d3f70ecfd63549add480dc7e9c7b68e112c89856d376f3bc2dc5",
    "AddPublic": "8148693638ec188586877ac7bf2c7c083851d26017f1f718507d33e2881a8304",
    "MulPublic": "c0b756ce3b344588fe943082a659f77a14b7639ae3bbd843a0915bd9d551e430",
    "IdPublic": "7e96b7c6e35ac13e65838f1f3ec6d9f00683e513971cf0703c4f1b9a9e576e45",
    "AddTwoRows": "3d5953ee10cf5a25a7774cd99134f7a781f3794df1f1b213e95770b85e23fc79",
    "AddThree": "a04deaaa23a704c0f40690d3393eb56d0fa5a7ff886af0924d94ba50a5069a19",
    "AddTwoInstPublic": "d89d9af0c55e75f8baaa22eda6aaefc1af9b7beba5d16544f02d7665024e83b7",
    "VoteBool": "820a97969b26a367515a61a5fd31f7068807a0d8134ff28c9bf8de265b62c827",
    "AnonTransfer2x2": "a616e78688f8eadf22753f37463f82c7664d16b92d9c6e16f1d8343ebc107811",
}

TEST_ITEM = re.compile(
    r"((?:#\[[^\]]+\]\s*)*#\[test\]\s*fn\s+([A-Za-z_]\w*))",
    re.MULTILINE,
)
RAW_STRING_START = re.compile(r'(?:b?r)(#*)"')
FORBIDDEN = re.compile(
    r"impl\s+Fn|FnOnce|FnMut|macro_rules!|\$(?:body|setup|action)|"
    r"\b(?:struct|enum|type)\s+(?:Action|Step)\b|"
    r"rustfmt::skip|include_(?:str|bytes)!"
)
PROTECTED_CALLBACK_TEST = "halo2_verify_tiny_commit_open_ipa_zk1_permutation_harness"
PROTECTED_CALLBACK_SHA256 = (
    "813f2609c3dcdf103221621a4ba4f0f44215dec08f865e24953365bef6f7a9ee"
)


class GuardError(AssertionError):
    """The compacted Halo2 source no longer matches its audited contract."""


def _sha256(source: str) -> str:
    return hashlib.sha256(source.encode()).hexdigest()


def _compact(source: str) -> str:
    return re.sub(r"\s+", "", source)


def _blob(blob: str) -> str:
    try:
        return subprocess.check_output(
            ["git", "cat-file", "blob", blob],
            cwd=ROOT,
            text=True,
            encoding="utf-8",
        )
    except subprocess.CalledProcessError as error:
        raise GuardError(f"authenticated preimage {blob} is unavailable") from error


def _matching_brace(source: str, opening: int) -> int:
    depth = 0
    cursor = opening
    while cursor < len(source):
        char = source[cursor]
        if source.startswith("//", cursor):
            newline = source.find("\n", cursor + 2)
            cursor = len(source) if newline < 0 else newline + 1
            continue
        if source.startswith("/*", cursor):
            comment_depth = 1
            cursor += 2
            while cursor < len(source) and comment_depth:
                if source.startswith("/*", cursor):
                    comment_depth += 1
                    cursor += 2
                elif source.startswith("*/", cursor):
                    comment_depth -= 1
                    cursor += 2
                else:
                    cursor += 1
            if comment_depth:
                raise GuardError("unterminated Rust block comment")
            continue
        raw = RAW_STRING_START.match(source, cursor)
        if raw:
            terminator = '"' + raw.group(1)
            close = source.find(terminator, raw.end())
            if close < 0:
                raise GuardError("unterminated Rust raw string")
            cursor = close + len(terminator)
            continue
        if char == '"' or source.startswith('b"', cursor):
            cursor += 2 if source.startswith('b"', cursor) else 1
            while cursor < len(source):
                if source[cursor] == "\\":
                    cursor += 2
                elif source[cursor] == '"':
                    cursor += 1
                    break
                else:
                    cursor += 1
            continue
        if char == "'" and cursor + 2 < len(source):
            if source[cursor + 2] == "'":
                cursor += 3
                continue
            if source[cursor + 1] == "\\":
                close = source.find("'", cursor + 2)
                if 0 < close - cursor <= 12:
                    cursor = close + 1
                    continue
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return cursor
        cursor += 1
    raise GuardError("unbalanced Rust braces")


def _function(source: str, name: str) -> str:
    marker = f"fn {name}("
    start = source.find(marker)
    if start < 0:
        raise GuardError(f"missing function {name}")
    opening = source.find("{", start)
    if opening < 0:
        raise GuardError(f"missing body for {name}")
    return source[start : _matching_brace(source, opening) + 1]


def _circuit_contract(source: str, name: str) -> str:
    marker = f"impl Circuit<Scalar> for {name} {{"
    start = source.find(marker)
    if start < 0:
        raise GuardError(f"missing shared circuit {name}")
    opening = source.find("{", start)
    implementation = source[start : _matching_brace(source, opening) + 1]
    return _compact(f"pub struct {name};{implementation}")


def _test_contract(source: str) -> tuple[tuple[str, str], ...]:
    return tuple(
        (match.group(2), _compact(match.group(1)))
        for match in TEST_ITEM.finditer(source)
    )


def _validate_sources(shard_sources: tuple[str, str], zk_source: str) -> None:
    current_lines = 0
    if len(shard_sources) != len(SHARDS):
        raise GuardError("Halo2 shard source count drifted")
    for shard, source in zip(SHARDS, shard_sources):
        preimage = _blob(shard.preimage_blob)
        if _sha256(preimage) != shard.preimage_sha256:
            raise GuardError(f"preimage digest drifted for {shard.path}")
        lines = len(source.splitlines())
        if lines > shard.line_ceiling:
            raise GuardError(f"line ceiling exceeded for {shard.path}: {lines}")
        current_lines += lines
        current_tests = _test_contract(source)
        if tuple(name for name, _ in current_tests) != shard.tests:
            raise GuardError(f"current test inventory drifted for {shard.path}")
        test_contract_sha256 = _sha256(
            json.dumps(current_tests, separators=(",", ":"))
        )
        if test_contract_sha256 != shard.test_contract_sha256:
            raise GuardError(f"current test attributes drifted for {shard.path}")
        if "impl Circuit<" in source:
            raise GuardError(f"duplicate local Circuit implementation in {shard.path}")

    if OPENING_LINES - current_lines < MINIMUM_REDUCTION:
        raise GuardError("shared-circuit reduction fell below the 1,500-line gate")
    if current_lines > LINE_CEILING:
        raise GuardError("combined Halo2 shard line ceiling exceeded")

    protected = _function(shard_sources[1], PROTECTED_CALLBACK_TEST)
    if _sha256(protected) != PROTECTED_CALLBACK_SHA256:
        raise GuardError("callback-bearing permutation test changed")

    audited_03 = shard_sources[1].replace(protected, "")
    if FORBIDDEN.search(shard_sources[0]) or FORBIDDEN.search(audited_03):
        raise GuardError("forbidden callback/body DSL or source relocation detected")

    for name, expected in SHARED_CIRCUIT_SHA256.items():
        actual = _sha256(_circuit_contract(zk_source, name))
        if actual != expected:
            raise GuardError(f"shared circuit contract drifted for {name}")

    for shard, source in zip(SHARDS, shard_sources):
        if _sha256(source) != shard.postimage_sha256:
            raise GuardError(f"postimage digest drifted for {shard.path}")


class Halo2BackendSharedCircuitSourceTest(unittest.TestCase):
    """Authenticate the compacted shards and fail closed under mutations."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.sources = tuple(shard.path.read_text(encoding="utf-8") for shard in SHARDS)
        cls.zk_source = ZK_PATH.read_text(encoding="utf-8")

    def test_shared_circuit_compaction_contract(self) -> None:
        _validate_sources(self.sources, self.zk_source)

    def test_mutations_fail_closed(self) -> None:
        mutations: list[tuple[tuple[str, str], str]] = []

        renamed = self.sources[0].replace(SHARDS[0].tests[0], "mutated_test", 1)
        mutations.append(((renamed, self.sources[1]), self.zk_source))

        local_impl = self.sources[0] + "\nimpl Circuit<Scalar> for Local {}\n"
        mutations.append(((local_impl, self.sources[1]), self.zk_source))

        callback_drift = self.sources[1].replace("expect_ok", "expect_changed", 1)
        mutations.append(((self.sources[0], callback_drift), self.zk_source))

        forbidden = self.sources[1] + "\nmacro_rules! body { () => {} }\n"
        mutations.append(((self.sources[0], forbidden), self.zk_source))

        oversized = self.sources[0] + "\n" * 2
        mutations.append(((oversized, self.sources[1]), self.zk_source))

        add_start = self.zk_source.index("impl Circuit<Scalar> for Add {")
        zk_mutation = self.zk_source[:add_start] + self.zk_source[add_start:].replace(
            "Scalar::from(2)", "Scalar::from(20)", 1
        )
        mutations.append((self.sources, zk_mutation))

        for shard_sources, zk_source in mutations:
            with self.subTest(mutation=_sha256("".join(shard_sources) + zk_source)):
                with self.assertRaises(GuardError):
                    _validate_sources(shard_sources, zk_source)


if __name__ == "__main__":
    unittest.main()
