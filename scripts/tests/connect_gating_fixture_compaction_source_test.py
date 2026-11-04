#!/usr/bin/env python3
"""Guard current typed Connect fixtures, direct assertions and protocol routes.

Current tests use strict typed relay strategies; retired aliases are absent.
"""

from __future__ import annotations

import hashlib
import json
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = ROOT / "crates/iroha_torii/tests/connect_gating.rs"
# The grouped protocol test binary is the sole owner of the shared fixture modules.
OWNER_PATH = ROOT / "crates/iroha_torii/tests/grouped/protocols.rs"
OWNER_MODULES = {
    "build_identity_test_fixture": "../../src/build_identity_test_fixture.rs",
    "fixtures": "../fixtures.rs",
    "connect_gating": "../connect_gating.rs",
}
# Retired synthetic genesis trust anchor and fixed network: every Torii now runs over
# an applied signed genesis whose hash and network bind each Connect session.
RETIRED_GENESIS_FRAGMENTS = (
    "from_untyped_unchecked",
    "Connect gating test genesis trust anchor",
    "signed_query_network_id",
)
GENESIS_ANCHORS = (
    "let native_chain = fixtures::commit_signed_genesis(cfg, World::default(), Vec::new());",
    "native_chain.network_id(),",
    "native_chain.kura().clone(),",
    "native_chain.state().clone(),",
)
EXPECTED_TESTS = (('connect_config_fixture_uses_checked_key_generation', ('#[test]',)),
 ('connect_endpoints_report_typed_unavailability_when_disabled', ('#[tokio::test]',)),
 ('connect_status_present_when_enabled', ('#[tokio::test]',)),
 ('connect_status_reports_exact_local_only_strategy', ('#[tokio::test]',)),
 ('connect_status_reports_broadcast_effective_when_p2p_attached', ('#[tokio::test]',)),
 ('connect_status_reports_local_only_when_relay_disabled_with_p2p_attached',
  ('#[tokio::test]',)),
 ('connect_session_delete_endpoint_removes_tokens', ('#[tokio::test]',)),
 ('connect_session_status_requires_management_token', ('#[tokio::test]',)),
 ('connect_session_delete_rejects_ws_attach',
  ('#[cfg(feature = "ws_integration_tests")]', '#[tokio::test]')),
 ('connect_ws_handshake_succeeds_when_enabled',
  ('#[cfg(feature = "ws_integration_tests")]', '#[tokio::test]')),
 ('connect_ws_accepts_protocol_token',
  ('#[cfg(feature = "ws_integration_tests")]', '#[tokio::test]')),
 ('connect_ws_closes_on_role_direction_mismatch',
  ('#[cfg(feature = "ws_integration_tests")]', '#[tokio::test]')),
 ('connect_ws_duplicate_frame_does_not_close_session',
  ('#[cfg(feature = "ws_integration_tests")]', '#[tokio::test]')),
 ('connect_ws_broadcast_relay_updates_p2p_rebroadcast_counter',
  ('#[cfg(feature = "ws_integration_tests")]', '#[tokio::test]')),
 ('connect_ws_broadcast_without_p2p_increments_skipped_rebroadcast_counter',
  ('#[cfg(feature = "ws_integration_tests")]', '#[tokio::test]')),
 ('connect_ws_local_only_with_p2p_does_not_rebroadcast',
  ('#[cfg(feature = "ws_integration_tests")]', '#[tokio::test]')),
 ('connect_ws_relay_disabled_with_p2p_does_not_rebroadcast',
  ('#[cfg(feature = "ws_integration_tests")]', '#[tokio::test]')),
 ('connect_ws_rejects_query_token',
  ('#[cfg(feature = "ws_integration_tests")]', '#[tokio::test]')))
EXPECTED_SEEDS = ('0x24', '0x34', '0x44', '0x52', '0x62', '0x92', '0xA3', '0xB4', '0xC5', '0xD6', '0xE7')
EXPECTED_PINGS = ('1', '41', '42', '7', '8', '9', '10')
ASSERTION_SURFACE_SHA256 = '10e8692fffe350e6341c7e7402d7a0f231d7b274b41db5ee04dc1138db470f0f'
CONNECT_URI_SURFACE_SHA256 = '8ae0493017455d5c7d6480e491828760e68aa4ed9bfe3fb2d0d7da7de68ee1f0'
EXPECTED_HELPERS = (
    "connect_network_id",
    "request_with_loopback_connect_info",
    "connect_aggregate_status",
    "connect_aggregate_status_json",
    "connect_aggregate_status_payload",
    "connect_status_policy",
    "connect_status_counters",
    "relay_strategy",
    "attached_connect_relay_status",
    "await_connect_p2p_attachment",
    "connect_session_request_body",
    "create_connect_session_payload",
    "spawn_test_server",
    "bind_connect_test_listener",
    "create_connect_app_session",
    "open_connect_app_websocket",
    "connect_status_counters_or_defaults",
    "wait_for_connect_relay_p2p_attachment",
    "checked_connect_key_fixture",
    "checked_connect_transport_key_fixture",
    "minimal_actual_config",
    "build_torii",
)
CONFIG_ANCHORS = (
    "let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();",
    "cfg.common.key_pair = checked_connect_key_fixture();",
    "cfg.common.soranet_transport_key_pair = checked_connect_transport_key_fixture();",
    "cfg.common.trusted_peers = WithOrigin::inline(A::TrustedPeers {",
    "cfg.network.lane_profile = A::LaneProfile::Core;",
    "cfg.torii.connect = A::Connect {",
    "enabled: connect_enabled,",
    "cfg.torii.sorafs_gateway = A::SorafsGateway::default();",
    "cfg.torii.webhook = A::Webhook::default();",
    "cfg.kura.fsync_mode = iroha_config::kura::FsyncMode::Batched;",
    "cfg.tiered_state.enabled = false;",
    "cfg.tiered_state.hot_retained_keys = 0;",
    "cfg.tiered_state.max_snapshots = 0;",
    "cfg.settlement = A::Settlement::default();",
    "cfg.fraud_monitoring = A::FraudMonitoring {",
    "cfg.gov.approval_threshold_q_den = 1;",
    "cfg.accel.merkle_min_leaves_gpu = defaults::accel::MERKLE_MIN_LEAVES_GPU;",
    "cfg.concurrency.rayon_global_threads = defaults::concurrency::RAYON_GLOBAL;",
    "cfg.zk.fastpq.metal_max_in_flight = None;",
    "cfg.zk.fastpq.metal_threadgroup_width = None;",
    "StreamingKeyMaterial::new(checked_connect_key_fixture())",
    "cfg.streaming.codec = A::StreamingCodec::from_defaults();",
)
REQUIRED_JSON_DIAGNOSTICS = (
    "typed Connect disabled error envelope",
    "status should be valid JSON",
    "connect status should include p2p_rebroadcasts_total",
    "connect status should include p2p_rebroadcast_skipped_total",
    "connect status should include policy.relay_strategy",
    "connect status should include policy.relay_effective_strategy",
    "connect status should include policy.relay_p2p_attached",
    "session status should be JSON",
)
FORBIDDEN = re.compile(
    r"dyn\s+Fn|FnMut|FnOnce|\bfn\s*\(|\$(?:body|setup|action)|"
    r"\b(?:Action|Step)\b|include_(?:str|bytes)!|macro_rules!"
)
DIRECT_TEST = re.compile(
    r"(?P<attrs>(?:^#\[[^\n]+\]\n)+)"
    r"^(?:async\s+)?fn\s+(?P<name>connect_[a-z0-9_]+)\s*\(",
    re.MULTILINE,
)
DIRECT_FUNCTION = re.compile(
    r"^(?:async\s+)?fn\s+(?P<name>[a-z_][a-z0-9_]*)\s*\(",
    re.MULTILINE,
)
# Each session derives its id from the network of the test's own applied genesis (`&cfg`).
SESSION_SEED = re.compile(
    r"connect_session_request_body\(\s*&cfg,\s*(?P<direct>0x[0-9A-Fa-f]+)\s*\)|"
    r"create_connect_(?:session_payload|app_session)\("
    r"[^,]+,\s*&cfg,\s*(?P<typed>0x[0-9A-Fa-f]+)\s*\)"
)


class GuardError(AssertionError):
    """The compacted source no longer matches its authenticated contract."""


def _sha256(text: str) -> str:
    return hashlib.sha256(text.encode()).hexdigest()




def _skip_quoted(source: str, start: int) -> int:
    raw = re.match(r'(?:b?r)(#*)"', source[start:])
    if raw:
        terminator = '"' + raw.group(1)
        end = source.find(terminator, start + raw.end())
        if end < 0:
            raise GuardError("unterminated Rust raw string")
        return end + len(terminator)
    quote_start = start + (1 if source.startswith('b"', start) else 0)
    quote = source[quote_start]
    cursor = quote_start + 1
    while cursor < len(source):
        if source[cursor] == "\\":
            cursor += 2
        elif source[cursor] == quote:
            return cursor + 1
        else:
            cursor += 1
    raise GuardError("unterminated Rust string or character literal")


def _matching_delimiter(source: str, opening: int) -> int:
    pairs = {"(": ")", "[": "]", "{": "}"}
    stack: list[str] = []
    cursor = opening
    while cursor < len(source):
        if source.startswith("//", cursor):
            newline = source.find("\n", cursor + 2)
            cursor = len(source) if newline < 0 else newline + 1
            continue
        if source.startswith("/*", cursor):
            depth = 1
            cursor += 2
            while cursor < len(source) and depth:
                if source.startswith("/*", cursor):
                    depth += 1
                    cursor += 2
                elif source.startswith("*/", cursor):
                    depth -= 1
                    cursor += 2
                else:
                    cursor += 1
            if depth:
                raise GuardError("unterminated Rust block comment")
            continue
        if source[cursor] == '"' or source.startswith(('b"', 'r"', 'br"'), cursor):
            cursor = _skip_quoted(source, cursor)
            continue
        if re.match(r'(?:b?r)#+"', source[cursor:]):
            cursor = _skip_quoted(source, cursor)
            continue
        if source[cursor] == "'" and cursor + 2 < len(source):
            close = cursor + 2 if source[cursor + 1] != "\\" else cursor + 3
            if close < len(source) and source[close] == "'":
                cursor = close + 1
                continue
        character = source[cursor]
        if character in pairs:
            stack.append(character)
        elif character in pairs.values():
            if not stack or pairs[stack[-1]] != character:
                raise GuardError(f"unbalanced Rust delimiter at byte {cursor}")
            stack.pop()
            if not stack:
                return cursor
        cursor += 1
    raise GuardError("unterminated Rust delimiter")


def _function(source: str, name: str) -> str:
    match = re.search(
        rf"^(?:pub\s+)?(?:async\s+)?fn\s+{re.escape(name)}\s*\(",
        source,
        re.MULTILINE,
    )
    if match is None:
        raise GuardError(f"missing function: {name}")
    opening = source.find("{", match.end())
    if opening < 0:
        raise GuardError(f"missing function body: {name}")
    closing = _matching_delimiter(source, opening)
    return source[match.start() : closing + 1]


def _normalise(source: str) -> str:
    return re.sub(r"\s+", " ", source).strip()


def _test_inventory(source: str) -> tuple[tuple[str, tuple[str, ...]], ...]:
    inventory = []
    for match in DIRECT_TEST.finditer(source):
        attributes = tuple(match.group("attrs").splitlines())
        if "#[test]" in attributes or "#[tokio::test]" in attributes:
            inventory.append((match.group("name"), attributes))
    return tuple(inventory)


def _seed_inventory(source: str) -> tuple[str, ...]:
    return tuple(
        match.group("direct") or match.group("typed")
        for match in SESSION_SEED.finditer(source)
    )


def _macro_surface(source: str) -> tuple[str, ...]:
    calls = []
    pattern = re.compile(r"\b(?:assert|assert_eq|assert_ne|panic)!\s*\(")
    for match in pattern.finditer(source):
        opening = source.find("(", match.start(), match.end())
        closing = _matching_delimiter(source, opening)
        calls.append(_normalise(source[match.start() : closing + 1]))
    return tuple(calls)


def _diagnostic_surface(source: str) -> tuple[str, ...]:
    return tuple(
        match.group(1)
        for match in re.finditer(r'\.expect\(\s*"((?:[^"\\]|\\.)*)"\s*\)', source)
    )


def _uri_surface(source: str) -> tuple[str, ...]:
    return tuple(re.findall(r'"(/v1/connect[^"]*)"', source))


def _validate_owner(owner: str) -> None:
    modules = re.findall(r'^#\[path\s*=\s*"([^"]+)"\]\nmod ([a-z_]+);$', owner, re.MULTILINE)
    for name, path in OWNER_MODULES.items():
        if [declared for declared, module in modules if module == name] != [path]:
            raise GuardError(f"grouped protocol test root no longer owns shared module: {name}")


def _validate(source: str) -> None:
    if FORBIDDEN.search(source):
        raise GuardError("forbidden callback, body DSL, source relocation, or macro found")
    if re.search(r"#\[path\s*=", source) or tuple(
        re.findall(r"^use crate::([a-z_]+);$", source, re.MULTILINE)
    ) != ("fixtures", "build_identity_test_fixture"):
        raise GuardError("shared build-identity fixture ownership changed")
    if any(fragment in source for fragment in RETIRED_GENESIS_FRAGMENTS):
        raise GuardError("retired synthetic genesis trust anchor or fixed network found")
    build = _function(source, "build_torii")
    for anchor in GENESIS_ANCHORS:
        if build.count(anchor) != 1:
            raise GuardError(f"applied signed genesis binding changed: {anchor}")
    if tuple(re.findall(r'include!\("([^"]+)"\);', source)) != (
        "connect_gating_disabled_ws_test.rs",
    ):
        raise GuardError("the explicit disabled-websocket test include changed")
    if _test_inventory(source) != EXPECTED_TESTS:
        raise GuardError("direct Connect test names, attributes, or order changed")
    if _seed_inventory(source) != EXPECTED_SEEDS:
        raise GuardError("Connect session seed order changed")
    if tuple(re.findall(r"Ping \{ nonce: (\d+) \}", source)) != EXPECTED_PINGS:
        raise GuardError("Connect ping nonce order changed")
    functions = tuple(match.group("name") for match in DIRECT_FUNCTION.finditer(source))
    if any(functions.count(name) != 1 for name in EXPECTED_HELPERS):
        raise GuardError("typed helper inventory changed")
    config = _function(source, "minimal_actual_config")
    for anchor in CONFIG_ANCHORS:
        if config.count(anchor) != 1:
            raise GuardError(f"configuration override changed: {anchor}")
    diagnostics = _diagnostic_surface(source)
    if any(diagnostic not in diagnostics for diagnostic in REQUIRED_JSON_DIAGNOSTICS):
        raise GuardError("required JSON diagnostic changed")
    for surface, expected, label in (
        (_macro_surface(source), ASSERTION_SURFACE_SHA256, "assertion"),
        (_uri_surface(source), CONNECT_URI_SURFACE_SHA256, "Connect URI"),
    ):
        if _sha256(json.dumps(surface, separators=(",", ":"))) != expected:
            raise GuardError(f"{label} surface changed")


class ConnectGatingFixtureCompactionSourceTest(unittest.TestCase):
    """Exercise the source contract and deliberate fail-closed mutations."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.source = SOURCE_PATH.read_text(encoding="utf-8")
        cls.owner = OWNER_PATH.read_text(encoding="utf-8")

    def test_compacted_source_contract(self) -> None:
        _validate(self.source)
        _validate_owner(self.owner)

    def test_fixture_owner_relocation_is_rejected(self) -> None:
        with self.assertRaises(GuardError):
            _validate_owner(
                self.owner.replace(
                    '"../../src/build_identity_test_fixture.rs"',
                    '"local_build_identity_fixture.rs"',
                    1,
                )
            )

    def test_module_local_fixture_copy_is_rejected(self) -> None:
        self.assert_mutation_rejected(
            self.source.replace(
                "use crate::build_identity_test_fixture;",
                '#[path = "../src/build_identity_test_fixture.rs"]\n'
                "mod build_identity_test_fixture;",
                1,
            )
        )

    def test_synthetic_genesis_anchor_is_rejected(self) -> None:
        self.assert_mutation_rejected(
            self.source.replace(
                "cfg.network.lane_profile = A::LaneProfile::Core;",
                "cfg.network.lane_profile = A::LaneProfile::Core;\n"
                "    cfg.genesis.expected_hash = HashOf::<BlockHeader>::from_untyped_unchecked("
                'Hash::new(b"Connect gating test genesis trust anchor"));',
                1,
            )
        )

    def test_unbound_session_network_is_rejected(self) -> None:
        self.assert_mutation_rejected(
            self.source.replace(
                "connect_session_request_body(&cfg, 0x44)",
                "connect_session_request_body(&other_cfg, 0x44)",
                1,
            )
        )

    def assert_mutation_rejected(self, source: str) -> None:
        with self.assertRaises(GuardError):
            _validate(source)

    def test_test_name_mutation_is_rejected(self) -> None:
        self.assert_mutation_rejected(
            self.source.replace(
                "connect_status_present_when_enabled",
                "connect_status_present_when_enabled_changed",
                1,
            )
        )

    def test_seed_mutation_is_rejected(self) -> None:
        self.assert_mutation_rejected(self.source.replace("0xB4", "0xB5", 1))

    def test_assertion_axis_mutation_is_rejected(self) -> None:
        self.assert_mutation_rejected(self.source.replace("Some(false)", "Some(true)", 1))

    def test_config_override_mutation_is_rejected(self) -> None:
        self.assert_mutation_rejected(
            self.source.replace("cfg.tiered_state.max_snapshots = 0;", "", 1)
        )

    def test_callback_escape_hatch_is_rejected(self) -> None:
        self.assert_mutation_rejected(self.source + "\ntype HiddenCallback = fn();\n")

    def test_whitespace_growth_preserves_semantic_contract(self) -> None:
        _validate(self.source + "\n" * 100_000)

    def test_long_comment_preserves_semantic_contract(self) -> None:
        _validate(self.source + "\n// " + "x" * 1_000)


if __name__ == "__main__":
    unittest.main()
