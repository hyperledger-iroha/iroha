#!/usr/bin/env python3
"""Preserve the typed lifecycle-launch source seals and their fail-closed behavior."""

from __future__ import annotations

import ast
import hashlib
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = ROOT / "crates/iroha_core/src/sumeragi/v2_lifecycle_launch_tests.rs"
PROVIDER_PATHS = (
    ROOT / "crates/iroha_core/src/sumeragi/v2_lifecycle_source_test_helpers.rs",
    ROOT
    / "crates/iroha_core/src/sumeragi/v2_lifecycle_launch_ready_proposal_sign_test_fixtures.rs",
    ROOT
    / "crates/iroha_core/src/sumeragi/v2_lifecycle_launch_pending_kura_source_tests.rs",
    ROOT
    / "crates/iroha_core/src/sumeragi/v2_lifecycle_launch_recovered_sign_settlement_source_tests.rs",
    ROOT
    / "crates/iroha_core/src/sumeragi/v2_lifecycle_launch_recovered_fetch_source_tests.rs",
    ROOT
    / "crates/iroha_core/src/sumeragi/v2_lifecycle_launch_certified_response_retry_source_tests.rs",
    ROOT
    / "crates/iroha_core/src/sumeragi/v2_lifecycle_launch_recovered_fetch_settlement_source_tests.rs",
)
ROOT_SOURCE = SOURCE_PATH.read_text(encoding="utf-8")
PROVIDER_SOURCES = tuple(path.read_text(encoding="utf-8") for path in PROVIDER_PATHS)
PROVIDER_INCLUDES = tuple(f'include!("{path.name}");\n' for path in PROVIDER_PATHS)


def expand_source_providers(root_source: str) -> str:
    """Expand each explicitly reviewed helper/provider include exactly once."""

    source = root_source
    for provider_include, provider_source in zip(PROVIDER_INCLUDES, PROVIDER_SOURCES):
        if source.count(provider_include) != 1:
            raise AssertionError("lifecycle-launch tests lost an exact reviewed source provider")
        source = source.replace(provider_include, provider_source, 1)
    return source


SOURCE = expand_source_providers(ROOT_SOURCE)
GUARD_START = "#[test]\nfn launch_source_keeps_status_sealed_and_orders_store_transfer()"
GUARD_END = (
    "#[test]\n"
    "fn recovered_decision_fetch_composite_dispatch_reserves_capacity_before_claim_and_commit"
)
EXPECTED_FINGERPRINT = "616120fed09704fee63be477d9a4d96b271360631dfa8d508590c408af48d523"


def guarded_source(source: str = SOURCE) -> str:
    """Return the two source-seal tests and their restart helper functions."""

    start = source.index(GUARD_START)
    end = source.index(GUARD_END, start)
    return source[start:end]


def compact_rust(source: str) -> str:
    """Remove formatting and comments while retaining exact Rust string contents."""

    compact: list[str] = []
    index = 0
    block_depth = 0
    while index < len(source):
        if block_depth:
            if source.startswith("/*", index):
                block_depth += 1
                index += 2
            elif source.startswith("*/", index):
                block_depth -= 1
                index += 2
            else:
                index += 1
            continue
        if source.startswith("//", index):
            newline = source.find("\n", index + 2)
            index = len(source) if newline < 0 else newline + 1
            continue
        if source.startswith("/*", index):
            block_depth = 1
            index += 2
            continue
        character = source[index]
        if character.isspace():
            index += 1
            continue
        if character in {'"', "'"}:
            quote = character
            start = index
            index += 1
            escaped = False
            while index < len(source):
                current = source[index]
                index += 1
                if escaped:
                    escaped = False
                elif current == "\\":
                    escaped = True
                elif current == quote:
                    break
            compact.append(source[start:index])
            continue
        if character == "r":
            raw = re.match(r'r(#{0,16})"', source[index:])
            if raw:
                terminator = '"' + raw.group(1)
                start = index
                content_start = index + raw.end()
                end = source.find(terminator, content_start)
                if end < 0:
                    raise AssertionError("unterminated Rust raw string in guarded source")
                index = end + len(terminator)
                compact.append(source[start:index])
                continue
        compact.append(character)
        index += 1
    if block_depth:
        raise AssertionError("unterminated Rust block comment in guarded source")
    return "".join(compact)


def fingerprint(source: str) -> str:
    """Hash the format-insensitive, literal-sensitive guarded Rust source."""

    return hashlib.sha256(compact_rust(source).encode()).hexdigest()


def call_bodies(name: str, source: str = SOURCE) -> list[str]:
    """Extract balanced argument bodies for calls to one helper."""

    bodies: list[str] = []
    pattern = re.compile(rf"\b{re.escape(name)}\s*\(")
    for match in pattern.finditer(guarded_source(source)):
        text = guarded_source(source)
        opening = text.find("(", match.start())
        depth = 1
        index = opening + 1
        quote: str | None = None
        escaped = False
        while index < len(text) and depth:
            character = text[index]
            if quote is not None:
                if escaped:
                    escaped = False
                elif character == "\\":
                    escaped = True
                elif character == quote:
                    quote = None
            elif character in {'"', "'"}:
                quote = character
            elif character == "(":
                depth += 1
            elif character == ")":
                depth -= 1
            index += 1
        if depth:
            raise AssertionError(f"unterminated call to {name}")
        bodies.append(text[opening + 1 : index - 1])
    return bodies


RUST_STRING = re.compile(r'"(?:\\.|[^"\\])*"', re.DOTALL)


def string_literals(body: str) -> tuple[str, ...]:
    """Decode ordinary Rust strings used by the source-seal helper calls."""

    return tuple(ast.literal_eval(match.group()) for match in RUST_STRING.finditer(body))



def count_guard_arguments(body: str, source: str) -> tuple[str, int]:
    """Read one exact count and its inline or consistently bound literal token.

    This is a bounded reader for the sealed source assertions, not a Rust
    expression evaluator. Computed, mutable, absent, or conflicting named
    bindings fail closed; identical immutable bindings in separate guards agree.
    """

    identifier = r"[A-Za-z_][A-Za-z_0-9]*"
    arguments = re.fullmatch(
        rf"\s*{identifier}\s*,\s*({RUST_STRING.pattern}|{identifier})"
        r"\s*,\s*(0|[1-9][0-9]*)\s*,?\s*",
        body,
    )
    if arguments is None:
        raise AssertionError("count guard must have one source, token, and exact integer")
    token, count = arguments.groups()
    if token.startswith('"'):
        return ast.literal_eval(token), int(count)
    declarations = re.findall(
        rf"\blet\s+{re.escape(token)}\s*=\s*({RUST_STRING.pattern})\s*;",
        source,
    )
    definitions = re.findall(rf"\blet\s+(?:mut\s+)?{re.escape(token)}\b", source)
    if not declarations or len(declarations) != len(definitions):
        raise AssertionError("named count token must have only immutable literal definitions")
    values = {ast.literal_eval(value) for value in declarations}
    if len(values) != 1:
        raise AssertionError("named count token has conflicting literal definitions")
    return values.pop(), int(count)


def assert_region(source: str, start: str, end: str) -> str:
    """Reference the Rust helper's first-boundary region behavior."""

    if start not in source:
        raise AssertionError("missing start")
    remainder = source.split(start, 1)[1]
    if end not in remainder:
        raise AssertionError("missing end")
    return remainder.split(end, 1)[0]


def assert_required(source: str, tokens: tuple[str, ...]) -> None:
    for token in tokens:
        if token not in source:
            raise AssertionError(f"missing {token}")


def assert_forbidden(source: str, tokens: tuple[str, ...]) -> None:
    for token in tokens:
        if token in source:
            raise AssertionError(f"found {token}")


def assert_ordered(source: str, tokens: tuple[str, ...]) -> None:
    positions = []
    for token in tokens:
        position = source.find(token)
        if position < 0:
            raise AssertionError(f"missing {token}")
        positions.append(position)
    if any(left >= right for left, right in zip(positions, positions[1:])):
        raise AssertionError("reordered token")


class LifecycleLaunchSourceCompactionTest(unittest.TestCase):
    def test_guarded_source_fingerprint_and_size_gate(self) -> None:
        self.assertEqual(fingerprint(guarded_source()), EXPECTED_FINGERPRINT)
        self.assertLessEqual(len(ROOT_SOURCE.splitlines()), 2_316)
        for provider_source in PROVIDER_SOURCES:
            self.assertLessEqual(len(provider_source.splitlines()), 676)
        for name in (
            "launch_source_keeps_status_sealed_and_orders_store_transfer",
            "recovered_lifecycle_sign_dispatch_source_is_sealed_and_restart_closed",
        ):
            self.assertRegex(SOURCE, rf"#\[test\]\s*fn {name}\(\)")

    def test_typed_helpers_keep_first_match_and_fail_closed_semantics(self) -> None:
        helper_source = SOURCE[: SOURCE.index("#[test]")]
        for required in (
            "source.split_once(start)",
            "after_start.split_once(end)",
            "source.find(token)",
            "source.contains(*token)",
            "!source.contains(*token)",
            "previous_position < position",
            "source.matches(token).count()",
        ):
            self.assertIn(required, helper_source)
        self.assertNotRegex(helper_source, r"\b(?:Fn|FnMut|FnOnce)\b|\bdyn\b")

    def test_recovered_transaction_order_matrices_are_exact(self) -> None:
        actual = {string_literals(body) for body in call_bodies("assert_source_tokens_in_order")}
        expected = (
            (
                "let Some(body_store_identity) = self.body_store_identity.as_ref()",
                "services.matches_lifecycle_body_store(body_store_identity)",
                "services.matches_lifecycle_executor_output_guard(executor)",
                "attest_ready_recovered_lifecycle_sign",
                "capture_recovered_lifecycle_sign_capacity(dispatch_key)",
                "self.coordinator.plan_turn(inputs)",
                "reservation.class() == CapacityClass::Consensus",
                "prepare_recovered_lifecycle_sign_dispatch",
                "reservation.preflight(&prepared)",
                "reservation.commit(prepared)",
            ),
            (
                "PendingLifecycleCompletionV1::take_recovered_sign(pending_lifecycle_completion)",
                "prepare_recovered_lifecycle_sign_completion(authority)",
                "prepare_recovered_lifecycle_sign_broadcast_successor(",
                "prepare_recovered_lifecycle_sign_broadcast_transition(",
                "output_guard.begin_fail_stop_operation()",
                "transition.persist_exact_successor().is_err()",
                "transition.commit_after_publication();",
                "completion.acknowledge_after_publication();",
                "operation.complete();",
            ),
            (
                "if exact_ready != self.coordinator.ready_index",
                "record.work_class != LifecycleWorkClass::Broadcast",
                "recovered_lifecycle_signed_broadcast_paired_next_vote_ordinal",
                "attest_ready_recovered_lifecycle_signed_broadcast",
                "let factory = AuthenticatedSchedulerInputsFactory::new()",
                "attest_ready_recovered_lifecycle_sign(",
                "self.coordinator.plan_turn(inputs)",
                "project_claimed_recovered_lifecycle_signed_broadcast_output",
                "capture_recovered_lifecycle_signed_broadcast_refanout(authority)",
                "settle_turn(lease, super::TurnOutcome::Blocked(wait))",
                "output.commit_after_publication()",
            ),
            (
                "PendingLifecycleCompletionV1::take_recovered_sign(pending_lifecycle_completion)",
                "prepare_recovered_lifecycle_sign_completion_with_body(executor, authority)",
                "RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal",
                "preview.project_proposal_exact_output_authority()",
                "capture_recovered_lifecycle_proposal_exact_output(output_authority)",
                "output.prepare_wal_append_permit()",
                "append_recovered_lifecycle_proposal_prepare_wal(wal_permit)",
                "prepare_recovered_lifecycle_sign_broadcast_and_sign_successor(",
                "prepare_recovered_lifecycle_sign_broadcast_and_sign_transition(",
                "transition.persist_exact_successor().is_err()",
                "transition.commit_after_publication();",
                "completion.acknowledge_after_publication();",
                "output.commit_after_publication();",
            ),
        )
        for chain in expected:
            self.assertIn(chain, actual)

    def test_exact_count_guards_are_preserved(self) -> None:
        actual: set[tuple[str, int]] = set()
        for body in call_bodies("assert_source_token_count"):
            actual.add(count_guard_arguments(body, guarded_source()))
        expected = {
            ("owner.launch(inputs)?", 1),
            ("set_v2_effect_completion_observer(", 1),
            ("self.block_ingress.close()", 3),
            ("self.ingress_ready.store(false, Ordering::Release)", 2),
            ("self.block_ingress.close()", 2),
            ("self.coordinator.rollback_unpublished_turn(&lease)", 1),
            ("rollback_unpublished_reserved_turn(&lease", 3),
            ("reservation.cancel_uncommitted()", 6),
            ("self.matches_current_terminal_parent(coordinator)", 2),
            ("metadata.continuation == super::schema::DurableContinuation::None", 2),
            ("operation.complete()", 4),
        }
        self.assertTrue(expected <= actual)

    def test_recovered_proposal_settlement_forbids_early_output_abort(self) -> None:
        forbidden = [
            string_literals(body)
            for body in call_bodies("assert_forbidden_source_tokens")
            if "output.abort_before_publication()" in string_literals(body)
        ]
        self.assertCountEqual(
            forbidden,
            [
                ("output.abort_before_publication()",),
                ("output.abort_before_publication()", "retry!()"),
            ],
        )

    def test_terminal_authentication_uses_the_terminal_shutdown_branch(self) -> None:
        regions = {compact_rust(body) for body in call_bodies("source_region")}
        self.assertIn(
            'terminal,"if context.height == u64::MAX",'
            '"return Ok(HeightRunOutcome::Terminal);",',
            regions,
        )
        ordered = {string_literals(body) for body in call_bodies("assert_source_tokens_in_order")}
        self.assertIn(
            (
                "close_runner_ingress_for_finalized_drain",
                "DecidedLaneRecoveryIngressDrainMode::FinalizedClosedPrefix",
                "drain_finalized_lane_relay_prefix(",
                "ensure_closed_drained_cut()",
                "if context.height == u64::MAX",
            ),
            ordered,
        )
        self.assertIn(
            (
                "executor.durable_finality()",
                "authenticate_terminal_complete_tip(",
                "activated.into_clean_shutdown(&mut active_runner)?",
            ),
            ordered,
        )

    def test_shared_helper_provider_is_exact_and_cannot_be_omitted(self) -> None:
        helper_include = PROVIDER_INCLUDES[0]
        self.assertEqual(SOURCE.count("fn source_region<'a>"), 1)
        self.assertNotIn(helper_include, SOURCE)
        for changed in (
            ROOT_SOURCE.replace(helper_include, "", 1),
            ROOT_SOURCE.replace(helper_include, helper_include * 2, 1),
        ):
            with self.assertRaises(AssertionError):
                expand_source_providers(changed)

    def test_count_guard_arguments_reject_unresolved_or_computed_tokens(self) -> None:
        self.assertEqual(count_guard_arguments('source, "fixed token", 2', ""), ("fixed token", 2))
        self.assertEqual(
            count_guard_arguments("source, token, 1", 'let token = "fixed token";'),
            ("fixed token", 1),
        )
        self.assertEqual(
            count_guard_arguments(
                "source, token, 1",
                'let token = "fixed token"; let token = "fixed token";',
            ),
            ("fixed token", 1),
        )
        for body, source in (
            ("source, token, 1", ""),
            ("source, token, 1", 'let token = "first"; let token = "second";'),
            ("source, token, 1", 'let mut token = "fixed token";'),
            ("source, token, 1", 'let token = "fixed token"; let token = make_token();'),
            ("source, make_token(), 1", ""),
            ('source, "fixed token", 01', ""),
            ('source, "fixed token", 1 + 1', ""),
            ('source, "fixed token", 1, extra', ""),
        ):
            with self.assertRaises(AssertionError):
                count_guard_arguments(body, source)

    def test_named_rollover_count_guards_keep_both_exact_source_owners(self) -> None:
        source = guarded_source()
        bodies = [body for body in call_bodies("assert_source_token_count") if "fenced_rollover" in body]
        self.assertEqual(
            {compact_rust(body) for body in bodies},
            {"terminal_tail,fenced_rollover,1", "source,fenced_rollover,1"},
        )
        self.assertEqual(len(bodies), 2)
        self.assertEqual(len(re.findall(r"\blet\s+fenced_rollover\s*=", source)), 2)
        expected = (
            "if rollover_ready {\n            let Some(cut) = terminal_finalization_cut.as_ref() else {",
            1,
        )
        for body in bodies:
            self.assertEqual(count_guard_arguments(body, source), expected)

    def test_reference_helpers_reject_source_mutations(self) -> None:
        fixture = "start required first middle second end"
        self.assertEqual(assert_region(fixture, "start", "end"), " required first middle second ")
        assert_required(fixture, ("required", "middle"))
        assert_forbidden(fixture, ("forbidden",))
        assert_ordered(fixture, ("first", "middle", "second"))
        with self.assertRaises(AssertionError):
            assert_region(fixture.replace("start", "stale"), "start", "end")
        with self.assertRaises(AssertionError):
            assert_region(fixture.replace("end", "stale"), "start", "end")
        with self.assertRaises(AssertionError):
            assert_required(fixture.replace("required", "removed"), ("required",))
        with self.assertRaises(AssertionError):
            assert_forbidden(fixture + " forbidden", ("forbidden",))
        with self.assertRaises(AssertionError):
            assert_ordered(fixture, ("second", "middle", "first"))

    def test_fingerprint_rejects_literal_category_count_and_order_mutations(self) -> None:
        original = guarded_source()
        mutations = (
            original.replace("reservation.cancel_uncommitted()\", 6", "reservation.cancel_uncommitted()\", 5", 1),
            original.replace("assert_forbidden_source_tokens(\n        refanout", "assert_required_source_tokens(\n        refanout", 1),
            original.replace("\"drop(output);\",", "\"drop(output); removed\",", 1),
            original.replace("reservation.preflight(&prepared)", "reservation.commit(prepared)", 1),
        )
        for mutated in mutations:
            self.assertNotEqual(mutated, original)
            self.assertNotEqual(fingerprint(mutated), EXPECTED_FINGERPRINT)


if __name__ == "__main__":
    unittest.main()
