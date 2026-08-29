#!/usr/bin/env python3
"""Protect the readable typed-matrix residuals that actually landed."""

from __future__ import annotations

import hashlib
import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
CARGO_LOCK = REPO_ROOT / "Cargo.lock"
CARGO_LOCK_SHA256 = (
    "d5b8bf5efbdc3ce2a8b1c0d2d75e1c5d1a343a072f836cfb76205bc6ea4cf15f"
)

TOKEN_PATH = REPO_ROOT / "crates/iroha_crypto/src/soranet/token.rs"
TOKEN_GIT_BLOB = "7331ba54756c25dc65c10d273a47fd41d66548aa"
TOKEN_SHA256 = "78e38332ac027909dcdb6b7c4a35880866b7775ca7d1593213fbc3514f583d4a"
TOKEN_WHOLE_LINES = 3_051
TOKEN_TOTAL_TESTS = 41
TOKEN_ORIGINAL_BASELINE_LINES = 2_462
TOKEN_SECURITY_HARDENING_GROWTH_LINES = 665
TOKEN_BASELINE_LINES = TOKEN_ORIGINAL_BASELINE_LINES + TOKEN_SECURITY_HARDENING_GROWTH_LINES
TOKEN_LEGACY_SELECTED_LINES = 596
TOKEN_SELECTED_CAP = 526
TOKEN_ROWS_START = "// typed-matrix-residual:start token-rows"
TOKEN_ROWS_END = "// typed-matrix-residual:end token-rows"
TOKEN_RUNNERS_START = "// typed-matrix-residual:start token-runners"
TOKEN_RUNNERS_END = "// typed-matrix-residual:end token-runners"
TOKEN_IDS_SHA256 = (
    "aa4457f2408a762027e520a5911f1ce07257ee52b942a0e9ee2e2c538fb8d067"
)
TOKEN_DIRECT_SHA256 = (
    "8f0ea9e631da54b7e242e39c4c684e321d1fb0cc292667e53baf968df864ce47"
)

TOKEN_IDS = (
    "decode_rejects_all_zero_signature_material",
    "decode_rejects_unrepresentable_timestamps",
    "verifier_try_new_rejects_invalid_public_key_before_fingerprint",
    "decode_rejects_non_zero_flags",
    "mint_rejects_non_zero_flags",
    "mint_rejects_invalid_secret_key_length_before_backend",
    "mint_rejects_all_zero_secret_key_material_before_backend",
    "mint_rejects_issuer_fingerprint_mismatch_before_rng_or_signing",
    "decode_rejects_zero_ttl",
    "token_store_rejects_expired_and_ttl_overflow",
    "verifier_rejects_replay_with_store",
    "verifier_rejects_invalid_public_key_length_before_backend",
    "verifier_rejects_signature_length_before_replay_store",
    "verifier_rejects_short_all_zero_signature_as_bad_encoding",
    "verifier_rejects_all_zero_signature_before_backend_and_replay_store",
    "persistent_store_rejects_duplicate_token_ids_on_load",
    "persistent_store_rejects_active_snapshot_beyond_ttl",
    "persistent_store_rejects_snapshot_over_capacity",
    "persistent_store_rejects_empty_snapshot",
    "persistent_store_rejects_overflowing_expiry_on_load",
    "persistent_store_rejects_non_norito_snapshot",
    "persistent_store_rejects_concurrent_ledger_owner",
)

TOKEN_RUNNERS = (
    ("admission_token_decode_matrix", (0, 1, 3, 8)),
    ("admission_token_verifier_rejects_invalid_public_keys_at_construction", (2, 11)),
    ("admission_token_verifier_rejects_malformed_signatures_before_replay", (12, 13)),
    ("admission_token_verifier_rejects_inert_signature_before_replay", (14,)),
    ("admission_token_mint_matrix", (4, 5, 6, 7)),
    ("admission_token_temporal_matrix", (9,)),
    ("admission_token_replay_store_matrix", (10,)),
    ("admission_token_persistence_matrix", (15, 16, 17, 18, 19, 20, 21)),
)

TOKEN_DIRECT_TESTS = (
    "signing_body_matches_legacy_contiguous_layout",
    "minimum_frame_length_counts_the_version_once",
    "encode_decode_round_trip",
    "admission_token_debug_output_redacts_bearer_material",
    "admission_token_zeroize_clears_all_bearer_fields",
    "decode_truncated_token_prefixes_fail_closed",
    "token_signature_reader_rejects_mismatch_and_overflow_without_advancing",
    "try_encode_rejects_oversized_direct_signature_without_panic",
    "verify_accepts_valid_token",
    "required_replay_store_makes_admission_tokens_single_use",
    "verify_rejects_relay_mismatch",
    "verify_rejects_invalid_temporal_bounds_before_signature_preflight",
    "frame_detection",
    "mint_rejects_subsecond_timestamps_in_whole_second_wire_format",
    "mint_reports_rng_failure",
    "fill_random_rejects_all_zero_nonce_material",
    "token_store_limits_enforce_first_release_ceiling",
    "token_store_fails_closed_without_evicting_active_records",
    "in_memory_store_never_regresses_observed_time",
    "known_replay_is_rejected_before_signature_verification",
    "pending_replay_reservation_rejects_concurrent_duplicate",
    "verifier_retains_replay_marker_through_clock_skew_window",
    "invalid_signatures_do_not_poison_replay_store",
    "persistent_store_materializes_empty_ledger_on_load",
    "persistent_store_rejects_bare_legacy_snapshot",
    "persistent_store_blocks_replay_after_restart",
    "persistent_store_queries_do_not_write_and_rollback_fails_closed",
    "persistent_store_durably_observes_rejected_insert_time",
    "persistent_store_retries_dirty_snapshot_after_failed_write",
    "persistent_store_preserves_subsecond_expiry_and_high_watermark",
    "persistent_store_capacity_preserves_active_records_across_restart",
    "persistent_store_prunes_expired_on_load",
    "persistent_store_rejects_noncanonical_snapshot_nanoseconds",
)

TOKEN_SECURITY_MARKERS = (
    'field("signature", &"[REDACTED]")',
    "fn zeroize_sensitive_fields(&mut self)",
    "const TOKEN_STORE_SNAPSHOT_VERSION_V1: u8 = 1;",
    "high_watermark_nanos: u32",
    "fn required_replay_store_makes_admission_tokens_single_use()",
    "fn persistent_store_rejects_bare_legacy_snapshot()",
    "fn persistent_store_retries_dirty_snapshot_after_failed_write()",
)

TOKEN_SUPPORT_START = "    struct MintedTokenFixture {"
TOKEN_SUPPORT_END = TOKEN_ROWS_END
TOKEN_SUPPORT_COMPONENTS = (
    ("minted fixture", TOKEN_SUPPORT_START),
    ("minted fixture methods", "    impl MintedTokenFixture {"),
    ("minted fixture constructor", "    fn minted_token_with_expectation("),
    ("replay-store constructor", "    fn replay_store("),
    ("persistent fixture", "    struct PersistentStoreFixture {"),
    ("persistent fixture methods", "    impl PersistentStoreFixture {"),
    ("snapshot writer", "    fn write_token_store_snapshot("),
    ("verify-error extractor", "    fn assert_mldsa_bad_encoding("),
    ("mint-error extractor", "    fn assert_mint_mldsa_bad_encoding("),
    ("store-error extractor", "    fn store_parse_message("),
    ("ordered row ledger", TOKEN_ROWS_START),
)

FORBIDDEN_SELECTED_LITERALS = (
    "rustfmt::skip",
    "macro_rules!",
    "dyn Fn",
    "impl Fn",
    "FnOnce",
    "FnMut",
    ": fn(",
    ": fn (",
    "Box<dyn",
    "$body",
    "$action",
    "$step",
)
FORBIDDEN_DSL_NAMES = re.compile(r"\b(?:Action|Step|Assertion|Body)\b")
FORBIDDEN_CLOSURE = re.compile(
    r"(?m)(?:^|[=(,]\s*)(?:move\s+)?\|[^|\n]*\|"
)


class GuardError(AssertionError):
    """Raised when a protected Rust source contract cannot be parsed."""


def _read_regular(path: Path) -> bytes:
    if path.is_symlink() or not path.is_file():
        raise GuardError(f"missing or non-regular source: {path.relative_to(REPO_ROOT)}")
    path.resolve(strict=True).relative_to(REPO_ROOT)
    return path.read_bytes()


def _git_blob_sha1(payload: bytes) -> str:
    header = f"blob {len(payload)}\0".encode()
    return hashlib.sha1(header + payload).hexdigest()  # noqa: S324


def _unique_region(source: str, start_marker: str, end_marker: str) -> str:
    if source.count(start_marker) != 1 or source.count(end_marker) != 1:
        raise GuardError(
            f"markers must occur exactly once: {start_marker!r}, {end_marker!r}"
        )
    start = source.index(start_marker)
    end = source.index(end_marker, start) + len(end_marker)
    return source[start:end]


def _is_char_literal(source: str, index: int) -> bool:
    """Distinguish a Rust character literal from a lifetime apostrophe."""

    if index + 2 >= len(source):
        return False
    if source[index + 1] == "\\":
        cursor = index + 2
        if cursor < len(source) and source[cursor] == "u":
            opening = source.find("{", cursor)
            closing = source.find("}", opening + 1)
            return opening >= 0 and closing >= 0 and source[closing + 1 : closing + 2] == "'"
        return source[index + 3 : index + 4] == "'"
    return source[index + 2] == "'"


def _matching_brace(source: str, opening: int) -> int:
    depth = 0
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
            elif char == "'" and _is_char_literal(source, index):
                state = "char"
            elif char == "{":
                depth += 1
            elif char == "}":
                depth -= 1
                if depth == 0:
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
    raise GuardError("unterminated Rust brace")


def _function_source(source: str, name: str, *, include_test_attr: bool = False) -> str:
    pattern = re.compile(rf"(?m)^[ \t]*fn {re.escape(name)}\s*\(")
    matches = list(pattern.finditer(source))
    if len(matches) != 1:
        raise GuardError(f"{name}: expected one direct function, found {len(matches)}")
    opening = source.find("{", matches[0].end())
    if opening < 0:
        raise GuardError(f"{name}: missing body")
    closing = _matching_brace(source, opening)
    start = matches[0].start()
    if include_test_attr:
        attributes: list[str] = []
        cursor = start - 1
        while cursor >= 0:
            line_start = source.rfind("\n", 0, cursor) + 1
            line = source[line_start:cursor].strip()
            if not line.startswith("#[") or not line.endswith("]"):
                break
            attributes.insert(0, line)
            start = line_start
            cursor = line_start - 1
        if attributes.count("#[test]") != 1:
            raise GuardError(f"{name}: expected one #[test] attribute")
    return source[start : closing + 1]


def _direct_digest(source: str, names: tuple[str, ...]) -> str:
    digest = hashlib.sha256()
    previous = -1
    for name in names:
        item = _function_source(source, name, include_test_attr=True)
        position = source.index(item)
        if position <= previous:
            raise GuardError(f"direct test order drifted at {name}")
        previous = position
        digest.update(name.encode())
        digest.update(b"\0")
        digest.update(item.encode())
        digest.update(b"\n\0")
    return digest.hexdigest()


def _assert_readable(test: unittest.TestCase, label: str, source: str) -> None:
    test.assertLessEqual(len(source.splitlines()), 120, label)


def _assert_no_forbidden(test: unittest.TestCase, selected: str) -> None:
    for literal in FORBIDDEN_SELECTED_LITERALS:
        test.assertNotIn(literal, selected)
    test.assertNotRegex(selected.lower(), r"\bcallback\b")
    test.assertIsNone(FORBIDDEN_DSL_NAMES.search(selected))
    test.assertIsNone(FORBIDDEN_CLOSURE.search(selected))


class TypedMatrixResidualSourceTest(unittest.TestCase):
    """Authenticate only the readable typed matrices present in the tree."""

    def test_cargo_lock_pin(self) -> None:
        self.assertEqual(
            hashlib.sha256(_read_regular(CARGO_LOCK)).hexdigest(),
            CARGO_LOCK_SHA256,
        )

    def _assert_token_typed_matrix_contract(
        self, payload: bytes, *, exact_bytes: bool
    ) -> None:
        source = payload.decode("utf-8")
        if exact_bytes:
            self.assertEqual(_git_blob_sha1(payload), TOKEN_GIT_BLOB)
            self.assertEqual(hashlib.sha256(payload).hexdigest(), TOKEN_SHA256)
        whole_lines = len(source.splitlines())
        if exact_bytes:
            self.assertEqual(whole_lines, TOKEN_WHOLE_LINES)
        self.assertLessEqual(
            TOKEN_LEGACY_SELECTED_LINES + whole_lines - TOKEN_BASELINE_LINES,
            TOKEN_SELECTED_CAP,
        )
        self.assertEqual(
            len(re.findall(r"(?m)^[ \t]*#\[test\][ \t]*$", source)),
            TOKEN_TOTAL_TESTS,
        )
        self.assertEqual(len(TOKEN_RUNNERS) + len(TOKEN_DIRECT_TESTS), TOKEN_TOTAL_TESTS)
        self.assertEqual(len(set(TOKEN_DIRECT_TESTS)), len(TOKEN_DIRECT_TESTS))
        for marker in TOKEN_SECURITY_MARKERS:
            self.assertEqual(source.count(marker), 1, marker)

        rows = _unique_region(source, TOKEN_ROWS_START, TOKEN_ROWS_END)
        runners_hull = _unique_region(source, TOKEN_RUNNERS_START, TOKEN_RUNNERS_END)
        row_ids = tuple(re.findall(r'TokenCase\("([^"]+)"\)', rows))
        self.assertEqual(row_ids, TOKEN_IDS)
        self.assertEqual(len(set(row_ids)), len(row_ids))
        self.assertEqual(
            hashlib.sha256("\n".join(row_ids).encode()).hexdigest(),
            TOKEN_IDS_SHA256,
        )
        self.assertEqual(rows.count("struct TokenCase"), 1)
        self.assertEqual(rows.count("const TOKEN_CASES"), 1)
        _assert_readable(self, "token ordered row ledger", rows)

        support = source[
            source.index(TOKEN_SUPPORT_START) : source.index(
                TOKEN_SUPPORT_END, source.index(TOKEN_SUPPORT_START)
            )
            + len(TOKEN_SUPPORT_END)
        ]
        component_positions: list[tuple[str, int]] = []
        for label, marker in TOKEN_SUPPORT_COMPONENTS:
            self.assertEqual(support.count(marker), 1, label)
            component_positions.append((label, support.index(marker)))
        self.assertEqual(
            [position for _label, position in component_positions],
            sorted(position for _label, position in component_positions),
        )
        for offset, (label, start) in enumerate(component_positions):
            end = (
                component_positions[offset + 1][1]
                if offset + 1 < len(component_positions)
                else len(support)
            )
            _assert_readable(self, label, support[start:end])

        selected_parts = [support]
        previous_runner = -1
        seen_indices: list[int] = []
        for runner, expected_indices in TOKEN_RUNNERS:
            body = _function_source(source, runner, include_test_attr=True)
            self.assertIn(body, runners_hull)
            position = source.index(body)
            self.assertGreater(position, previous_runner, runner)
            previous_runner = position
            self.assertRegex(
                body,
                rf"(?m)^[ \t]*#\[test\]\n[ \t]*fn {re.escape(runner)}\(\)",
            )
            actual_indices = tuple(
                int(index)
                for index in re.findall(r"TOKEN_CASES\[(\d+)\]\.0", body)
            )
            self.assertEqual(actual_indices, expected_indices, runner)
            seen_indices.extend(actual_indices)
            assignments = list(
                re.finditer(r"let id = TOKEN_CASES\[(\d+)\]\.0;", body)
            )
            self.assertEqual(len(assignments), len(expected_indices), runner)
            for offset, assignment in enumerate(assignments):
                end = (
                    assignments[offset + 1].start()
                    if offset + 1 < len(assignments)
                    else len(body)
                )
                case_source = body[assignment.end() : end]
                self.assertTrue(
                    "{id" in case_source
                    or ".expect(id)" in case_source
                    or re.search(r",\s*id\s*\)", case_source),
                    f"{runner} case {assignment.group(1)} lacks id diagnostics",
                )
            _assert_readable(self, runner, body)
            selected_parts.append(body)

        self.assertEqual(sorted(seen_indices), list(range(len(TOKEN_IDS))))
        self.assertEqual(len(seen_indices), len(set(seen_indices)))
        for case_id in TOKEN_IDS:
            self.assertNotRegex(source, rf"(?m)^[ \t]*fn {re.escape(case_id)}\s*\(")

        self.assertEqual(_direct_digest(source, TOKEN_DIRECT_TESTS), TOKEN_DIRECT_SHA256)

        selected = "\n".join(selected_parts)
        _assert_no_forbidden(self, selected)

    def test_token_typed_matrix_contract(self) -> None:
        self._assert_token_typed_matrix_contract(
            _read_regular(TOKEN_PATH), exact_bytes=True
        )

    def test_token_guard_rejects_source_mutations(self) -> None:
        source = _read_regular(TOKEN_PATH).decode("utf-8")
        first_rows = (
            '        TokenCase("decode_rejects_all_zero_signature_material"),\n'
            '        TokenCase("decode_rejects_unrepresentable_timestamps"),'
        )
        swapped_rows = (
            '        TokenCase("decode_rejects_unrepresentable_timestamps"),\n'
            '        TokenCase("decode_rejects_all_zero_signature_material"),'
        )
        missing_diagnostic = source.replace(
            'assert!(matches!(err, DecodeError::InertSignature), "{id}");',
            'assert!(matches!(err, DecodeError::InertSignature));',
            1,
        )
        forbidden_callback = source.replace(
            "    fn admission_token_decode_matrix() {\n",
            "    fn admission_token_decode_matrix() {\n"
            "        let callback = |value| value;\n",
            1,
        )
        mutations = {
            "row order": source.replace(first_rows, swapped_rows, 1),
            "missing id diagnostic": missing_diagnostic,
            "callback abstraction": forbidden_callback,
            "line-cap padding": source + "\n" * 10,
        }
        for label, mutation in mutations.items():
            self.assertNotEqual(mutation, source, label)
            with self.subTest(label), self.assertRaises(AssertionError):
                self._assert_token_typed_matrix_contract(
                    mutation.encode(), exact_bytes=False
                )


if __name__ == "__main__":
    unittest.main()
