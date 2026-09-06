#!/usr/bin/env python3
"""Fail closed if a callable or decodable consensus-VRF surface returns.

First-release Sumeragi carries finalized global threshold-beacon shares only.
The pre-release per-validator VRF commit/reveal protocol has no compatibility
wire variant, producer, state projection, penalty path, HTTP/MCP/CLI route, or
SDK method. These stdlib-only checks use in-memory mutations for their negative
controls so the release guard cannot silently become a presence test for a
retired tombstone.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]

RETIRED_FILES = (
    "crates/iroha_core/src/sumeragi/epoch_report.rs",
    "crates/iroha_torii/tests/sumeragi_vrf_penalties_endpoint.rs",
)
RETIRED_ROUTE_PREFIX = "/v1/sumeragi/vrf/"
RETIRED_CONSENSUS_SYMBOLS = (
    "VrfCommit",
    "VrfReveal",
    "V2Vrf",
    "ActiveVrfLifecycle",
    "vrf_epoch_seals",
    "derive_vrf_material_from_key",
    "derive_vrf_penalty_actions",
    "verify_vrf_reveal_for_chain",
    "verify_vrf_commit",
    "verify_vrf_reveal",
)
RETIRED_RUNTIME_TOKENS = (
    "handle_v1_sumeragi_vrf_",
    "iroha.sumeragi.vrf.",
    "VrfPenaltiesReport",
    "SumeragiVrfPenaltiesReport",
    "get_sumeragi_vrf_",
    "post_sumeragi_vrf_",
    "submit_sumeragi_vrf_",
    "GOV_COUNCIL_PERSIST",
    "GOV_COUNCIL_REPLACE",
    "GOV_COUNCIL_AUDIT",
)
RETIRED_CLI_TOKENS = ("vrf-epoch", "vrf-penalties")

RUST_RUNTIME_ROOTS = (
    "crates/iroha_torii/src",
    "crates/iroha_torii_shared/src",
    "crates/iroha_torii_client/src",
    "crates/iroha_cli/src",
)
SDK_RUNTIME_ROOTS = (
    "python/iroha_python/src/iroha_python",
    "python/iroha_torii_client",
    "javascript/iroha_js/src",
    "IrohaSwift/Sources",
)
CONSENSUS_RUNTIME_ROOTS = (
    "crates/iroha_core/src/sumeragi",
    "crates/iroha_data_model/src/block",
)


class GuardError(AssertionError):
    """Raised when a retired consensus-VRF surface returns."""


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise GuardError(message)


def _is_test_or_generated_source(path: Path) -> bool:
    """Return whether ``path`` is test-only or generated API material."""

    parts = path.parts
    return (
        "tests" in parts
        or "test" in parts
        or "openapi" in parts
        or "build" in parts
        or "__pycache__" in parts
        or any(part.startswith(".") for part in parts)
        or path.name.endswith(("_test.rs", "_tests.rs"))
    )


def _strip_trailing_cfg_test_module(source: str) -> str:
    """Remove the conventional trailing inline Rust test module, if present."""

    match = re.search(r"(?m)^#\[cfg\(test\)\]\s*\nmod tests\s*\{", source)
    return source if match is None else source[: match.start()]


def _iter_runtime_sources(root: Path, relative_roots: tuple[str, ...]) -> list[Path]:
    paths: list[Path] = []
    for relative_root in relative_roots:
        base = root / relative_root
        if not base.exists():
            continue
        for path in base.rglob("*"):
            relative = path.relative_to(root)
            if not path.is_file() or _is_test_or_generated_source(relative):
                continue
            if path.suffix in {".rs", ".py", ".js", ".ts", ".kt", ".java", ".swift"}:
                paths.append(path)
    return sorted(set(paths))


def _iter_sdk_sources(root: Path) -> list[Path]:
    paths = _iter_runtime_sources(root, (*RUST_RUNTIME_ROOTS, *SDK_RUNTIME_ROOTS))
    for tree in (root / "kotlin", root / "java"):
        if not tree.exists():
            continue
        for path in tree.rglob("*"):
            relative = path.relative_to(root)
            if (
                path.is_file()
                and "src" in relative.parts
                and "main" in relative.parts
                and not _is_test_or_generated_source(relative)
                and path.suffix in {".kt", ".java"}
            ):
                paths.append(path)
    return sorted(set(paths))


def _guard_runtime_source(relative: str, source: str) -> None:
    production = _strip_trailing_cfg_test_module(source)
    forbidden = (RETIRED_ROUTE_PREFIX, *RETIRED_RUNTIME_TOKENS)
    if relative.startswith("crates/iroha_cli/"):
        forbidden += RETIRED_CLI_TOKENS
    for token in forbidden:
        _require(token not in production, f"retired token {token!r} returned in {relative}")


def _guard_consensus_source(relative: str, source: str) -> None:
    production = _strip_trailing_cfg_test_module(source)
    for token in RETIRED_CONSENSUS_SYMBOLS:
        _require(
            token not in production,
            f"retired consensus-VRF symbol {token!r} returned in {relative}",
        )


def _item_body(source: str, declaration: str) -> str:
    """Return the brace-delimited body starting at ``declaration``."""

    start = source.find(declaration)
    _require(start >= 0, f"missing protected declaration {declaration}")
    opening = source.find("{", start + len(declaration))
    _require(opening >= 0, f"missing body for protected declaration {declaration}")
    depth = 0
    for offset in range(opening, len(source)):
        char = source[offset]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return source[opening + 1 : offset]
    raise GuardError(f"unterminated body for protected declaration {declaration}")


def _guard_wire_hard_cut(source: str) -> None:
    payload = _item_body(source, "pub enum ConsensusMessageV2Payload")
    _require(
        "GlobalBeaconPartialSignature(GlobalBeaconPartialSignature)" in payload,
        "threshold-beacon partial traffic disappeared from the v2 wire enum",
    )
    for token in RETIRED_CONSENSUS_SYMBOLS:
        _require(token not in payload, f"retired wire variant {token!r} returned")


def audit(root: Path) -> None:
    """Audit the checkout's complete legacy consensus-VRF hard-cut boundary."""

    for relative in RETIRED_FILES:
        _require(not (root / relative).exists(), f"retired source file returned: {relative}")

    module_source = (root / "crates/iroha_core/src/sumeragi/mod.rs").read_text(
        encoding="utf-8"
    )
    _require(
        re.search(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+epoch_report\s*;", module_source)
        is None,
        "retired public epoch-report module returned",
    )

    for path in _iter_sdk_sources(root):
        _guard_runtime_source(
            str(path.relative_to(root)), path.read_text(encoding="utf-8")
        )

    for path in _iter_runtime_sources(root, CONSENSUS_RUNTIME_ROOTS):
        _guard_consensus_source(
            str(path.relative_to(root)), path.read_text(encoding="utf-8")
        )

    wire = (root / "crates/iroha_data_model/src/block/consensus_v2.rs").read_text(
        encoding="utf-8"
    )
    _guard_wire_hard_cut(wire)

    npos = (root / "crates/iroha_core/src/sumeragi/v2_npos.rs").read_text(
        encoding="utf-8"
    )
    for token in (
        "Consensus randomness is supplied exclusively by finalized global",
        "threshold-beacon pulses",
        "pub(crate) fn validate_candidate_context",
    ):
        _require(token in npos, f"first-release NPoS threshold-beacon contract lost {token!r}")


class SumeragiConsensusVrfRetirementSourceTest(unittest.TestCase):
    """Exercise the landed hard cut and representative in-memory regressions."""

    def test_checkout_has_no_callable_or_decodable_legacy_consensus_vrf_surface(self) -> None:
        audit(ROOT)

    def test_runtime_route_and_handler_mutations_fail_closed(self) -> None:
        for token in (RETIRED_ROUTE_PREFIX, "handle_v1_sumeragi_vrf_epoch"):
            with self.assertRaisesRegex(GuardError, "retired token"):
                _guard_runtime_source(
                    "crates/iroha_torii/src/lib.rs", f"const BAD: &str = {token!r};"
                )

    def test_wire_variant_mutation_fails_closed(self) -> None:
        source = (ROOT / "crates/iroha_data_model/src/block/consensus_v2.rs").read_text(
            encoding="utf-8"
        )
        marker = "GlobalBeaconPartialSignature(GlobalBeaconPartialSignature),"
        mutated = source.replace(marker, f"{marker}\n    VrfCommit(VrfCommit),", 1)
        with self.assertRaisesRegex(GuardError, "retired wire variant"):
            _guard_wire_hard_cut(mutated)

    def test_non_test_producer_mutation_fails_closed(self) -> None:
        with self.assertRaisesRegex(GuardError, "retired consensus-VRF symbol"):
            _guard_consensus_source(
                "crates/iroha_core/src/sumeragi/v2_npos.rs",
                "fn derive_vrf_material_from_key() {}",
            )


if __name__ == "__main__":
    unittest.main()
