#!/usr/bin/env python3
"""Guard the typed Connect-gating fixture compaction.

The guard authenticates the indexed preimage and the current direct-test
inventory, preserves protocol literals, pins the compacted fixture/assertion
surface, and rejects callback DSLs, source relocation, or line packing.
"""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = ROOT / "crates/iroha_torii/tests/connect_gating.rs"
SHARED_CONFIG_PATH = ROOT / "crates/iroha_torii/src/test_utils.rs"
PREIMAGE_BLOB = "3db5717cb13b3d1e88b63612e5e8cd11ce9e8266"
PREIMAGE_SHA256 = "b79bce39ebc92a466a63b26ec4fb1a7609928409ce38b7ff658018a49ea5030d"
PREIMAGE_LINES = 2_794
MINIMUM_RUST_LINE_REDUCTION = 1_000
SOURCE_LINE_CEILING = 1_535
MAX_LINE_LENGTH = 120

CURRENT_TEST_INVENTORY_SHA256 = (
    "d3dd47efcfe18064641d2db4d9dcb14e71793cd59f0cccfaf54c5bc3476a9f07"
)
CONFIG_SHA256 = "e0c6e44c9b068c5fb8129c9b9bf70c18e9b69db4756e82fcce53facb21ef49a8"
ASSERTION_SURFACE_SHA256 = "5a005e40077588612220913ad640449e45b8b511ba03a2d3fbe720592e30f757"
DIAGNOSTIC_SURFACE_SHA256 = "57f0899b5bf3abd819d31de10bbdba976ef25deac10962a5aa44dcc1d0db5f99"
CONNECT_URI_SURFACE_SHA256 = "8ae0493017455d5c7d6480e491828760e68aa4ed9bfe3fb2d0d7da7de68ee1f0"
SHARED_CONFIG_SHA256 = "2568d151248629c8fb939293d0e829265f76819cb54e24dd27512d5e45fa4b60"

EXPECTED_HELPERS = (
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
    'b"Connect gating test genesis trust anchor"',
    "cfg.torii.connect = A::Connect {",
    "enabled: connect_enabled,",
    "cfg.torii.sorafs_gateway = A::SorafsGateway::default();",
    "cfg.torii.webhook = A::Webhook::default();",
    "cfg.kura.fsync_mode = iroha_config::kura::FsyncMode::Batched;",
    "cfg.tiered_state.enabled = false;",
    "cfg.tiered_state.hot_retained_keys = 0;",
    "cfg.tiered_state.max_snapshots = 0;",
    "cfg.settlement = A::Settlement {",
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
    r"\b(?:Action|Step)\b|include_(?:str|bytes)!|#\[path\s*=|macro_rules!"
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
SESSION_SEED = re.compile(
    r"connect_session_request_body\(\s*(?P<direct>0x[0-9A-Fa-f]+)\s*\)|"
    r"create_connect_(?:session_payload|app_session)\("
    r"[^,]+,\s*(?P<typed>0x[0-9A-Fa-f]+)\s*\)"
)


class GuardError(AssertionError):
    """The compacted source no longer matches its authenticated contract."""


def _sha256(text: str) -> str:
    return hashlib.sha256(text.encode()).hexdigest()


def _preimage() -> str:
    try:
        return subprocess.check_output(
            ["git", "cat-file", "blob", PREIMAGE_BLOB],
            cwd=ROOT,
            text=True,
            encoding="utf-8",
        )
    except subprocess.CalledProcessError as error:
        raise GuardError("authenticated Connect-gating preimage is unavailable") from error


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


def _validate(source: str, preimage: str) -> None:
    if _sha256(preimage) != PREIMAGE_SHA256:
        raise GuardError("authenticated preimage digest changed")
    if len(preimage.splitlines()) != PREIMAGE_LINES:
        raise GuardError("authenticated preimage line count changed")
    lines = source.splitlines()
    if len(lines) > SOURCE_LINE_CEILING:
        raise GuardError("Connect-gating source exceeded its compacted line ceiling")
    if PREIMAGE_LINES - len(lines) < MINIMUM_RUST_LINE_REDUCTION:
        raise GuardError("Connect-gating Rust reduction fell below 1,000 lines")
    if max(map(len, lines), default=0) > MAX_LINE_LENGTH:
        raise GuardError("Connect-gating source appears line-packed")
    if FORBIDDEN.search(source):
        raise GuardError("forbidden callback, body DSL, source relocation, or macro found")
    if tuple(re.findall(r'include!\("([^"]+)"\);', source)) != (
        "connect_gating_disabled_ws_test.rs",
    ):
        raise GuardError("the single historical include boundary changed")
    current_tests = _test_inventory(source)
    if (
        _sha256(json.dumps(current_tests, separators=(",", ":")))
        != CURRENT_TEST_INVENTORY_SHA256
    ):
        raise GuardError("current Connect test names, attributes, or order changed")
    if _seed_inventory(source) != _seed_inventory(preimage):
        raise GuardError("Connect session seed order changed")
    if tuple(re.findall(r"Ping \{ nonce: (\d+) \}", source)) != tuple(
        re.findall(r"Ping \{ nonce: (\d+) \}", preimage)
    ):
        raise GuardError("Connect ping nonce order changed")
    functions = tuple(match.group("name") for match in DIRECT_FUNCTION.finditer(source))
    if any(functions.count(name) != 1 for name in EXPECTED_HELPERS):
        raise GuardError("typed helper inventory changed")
    config = _function(source, "minimal_actual_config")
    for anchor in CONFIG_ANCHORS:
        if config.count(anchor) != 1:
            raise GuardError(f"configuration override changed: {anchor}")
    if _sha256(_normalise(config)) != CONFIG_SHA256:
        raise GuardError("configuration override surface changed")
    shared_config = _function(
        SHARED_CONFIG_PATH.read_text(encoding="utf-8"), "mk_minimal_root_cfg"
    )
    if _sha256(_normalise(shared_config)) != SHARED_CONFIG_SHA256:
        raise GuardError("shared minimal-root fixture changed without re-auditing overrides")
    diagnostics = _diagnostic_surface(source)
    if any(diagnostic not in diagnostics for diagnostic in REQUIRED_JSON_DIAGNOSTICS):
        raise GuardError("required JSON diagnostic changed")
    surfaces = (
        (_macro_surface(source), ASSERTION_SURFACE_SHA256, "assertion"),
        (diagnostics, DIAGNOSTIC_SURFACE_SHA256, "diagnostic"),
        (_uri_surface(source), CONNECT_URI_SURFACE_SHA256, "Connect URI"),
    )
    for surface, expected, label in surfaces:
        if _sha256(json.dumps(surface, separators=(",", ":"))) != expected:
            raise GuardError(f"{label} surface changed")


class ConnectGatingFixtureCompactionSourceTest(unittest.TestCase):
    """Exercise the source contract and deliberate fail-closed mutations."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.source = SOURCE_PATH.read_text(encoding="utf-8")
        cls.preimage = _preimage()

    def test_compacted_source_contract(self) -> None:
        _validate(self.source, self.preimage)

    def assert_mutation_rejected(self, source: str) -> None:
        with self.assertRaises(GuardError):
            _validate(source, self.preimage)

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

    def test_source_growth_is_rejected(self) -> None:
        extra = "\n// growth mutation" * (SOURCE_LINE_CEILING + 1)
        self.assert_mutation_rejected(self.source + extra)

    def test_line_packing_is_rejected(self) -> None:
        self.assert_mutation_rejected(self.source + "\n// " + "x" * MAX_LINE_LENGTH)


if __name__ == "__main__":
    unittest.main()
