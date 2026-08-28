#!/usr/bin/env python3
"""Fail closed on callable legacy Sumeragi consensus-VRF surfaces.

The first-release wire decoder retains ``VrfCommit`` and ``VrfReveal`` as
tombstones so an old envelope can be classified and rejected deterministically.
That compatibility must never grow back into an HTTP/MCP/CLI/SDK surface, a
producer, a penalty-report store, or an effect-application path.  These tests
are stdlib-only and use in-memory mutations for their negative controls.
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


class GuardError(AssertionError):
    """Raised when a retired callable surface or producer returns."""


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise GuardError(message)


def _is_test_or_openapi_source(path: Path) -> bool:
    """Return whether ``path`` is an allowed absence guard or generated API source."""

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


def _iter_runtime_sources(root: Path) -> list[Path]:
    paths: list[Path] = []
    for relative_root in (*RUST_RUNTIME_ROOTS, *SDK_RUNTIME_ROOTS):
        base = root / relative_root
        if not base.exists():
            continue
        for path in base.rglob("*"):
            if not path.is_file() or _is_test_or_openapi_source(path.relative_to(root)):
                continue
            if path.suffix in {".rs", ".py", ".js", ".ts", ".kt", ".java", ".swift"}:
                paths.append(path)

    for tree in (root / "kotlin", root / "java"):
        if not tree.exists():
            continue
        for path in tree.rglob("*"):
            relative = path.relative_to(root)
            if (
                path.is_file()
                and "src" in relative.parts
                and "main" in relative.parts
                and not _is_test_or_openapi_source(relative)
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


def _function_body(source: str, name: str, *, cfg_not_test: bool) -> str:
    cfg = r"#\[cfg\(not\(test\)\)\]\s+" if cfg_not_test else ""
    pattern = re.compile(cfg + rf"(?:pub\(crate\)\s+)?fn {re.escape(name)}\b")
    match = pattern.search(source)
    _require(match is not None, f"missing protected function {name}")
    opening = source.find("{", match.end())
    _require(opening >= 0, f"missing body for protected function {name}")
    depth = 0
    for offset in range(opening, len(source)):
        char = source[offset]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return source[opening + 1 : offset]
    raise GuardError(f"unterminated body for protected function {name}")


def _guard_empty_non_test_method(source: str, name: str) -> None:
    body = _function_body(source, name, cfg_not_test=True)
    _require("Vec::new()" in body, f"production {name} must return an empty vector")
    for token in ("self.active", "outbound", "retransmit", "pending_record"):
        _require(token not in body, f"production {name} regained legacy state access: {token}")


def _guard_npos_tombstone(source: str) -> None:
    for name in ("accept_commit", "accept_reveal"):
        body = _function_body(source, name, cfg_not_test=True)
        _require(
            "V2VrfIngressOutcome::Rejected(V2VrfRejection::OutOfWindow)" in body,
            f"production {name} must unconditionally reject the tombstone message",
        )
        _require("self.active" not in body, f"production {name} regained mutable VRF state")
    for name in ("take_outbound", "retransmission", "pending_records"):
        _guard_empty_non_test_method(source, name)

    candidate = _function_body(source, "validate_candidate_records", cfg_not_test=True)
    for token in (
        "!effects.vrf_epoch_seals.is_empty()",
        "return Err",
        "legacy VRF epoch effects are retired",
    ):
        _require(token in candidate, f"candidate tombstone gate lost {token!r}")

    for declaration in (
        "fn derive_vrf_material_from_key",
        "struct ActiveVrfLifecycle",
        "impl ActiveVrfLifecycle",
        "fn verify_vrf_reveal_for_chain",
        "fn verify_commit_proof",
        "fn verify_reveal_proof",
    ):
        offset = source.find(declaration)
        _require(offset >= 0, f"missing historical fixture declaration {declaration}")
        prefix = "\n".join(source[:offset].splitlines()[-8:])
        _require(
            "#[cfg(test)]" in prefix,
            f"legacy producer declaration is no longer test-only: {declaration}",
        )


def _guard_penalty_tombstone(source: str) -> None:
    body = _function_body(source, "derive_vrf_penalty_actions", cfg_not_test=True)
    _require("Ok(Vec::new())" in body, "production VRF penalty derivation must stay empty")
    for token in (
        "legacy NPoS VRF effect assembly is retired",
        "legacy NPoS VRF epoch persistence is retired",
        "legacy NPoS VRF penalty actions are retired",
        "legacy NPoS VRF penalty bookkeeping is retired",
    ):
        _require(token in source, f"production penalty tombstone lost {token!r}")


def _guard_block_message_tombstone(source: str) -> None:
    body = _function_body(source, "ensure_live_outbound", cfg_not_test=False)
    for token in (
        "ConsensusMessageV2Payload::VrfCommit(_)",
        "ConsensusMessageV2Payload::VrfReveal(_)",
        "return Err",
        "refusing to emit retired Sumeragi consensus-VRF message",
    ):
        _require(token in body, f"live outbound VRF tombstone gate lost {token!r}")
    _require(
        "message.ensure_supported_wire_version()?;" in source,
        "inbound decoder must retain version-checked tombstone decoding",
    )


def _guard_legacy_preimages_are_test_only(source: str) -> None:
    _require(
        "#[cfg(test)]\npub use iroha_data_model::block::consensus::{VrfCommit, VrfReveal};" in source,
        "legacy consensus-VRF type re-exports must remain test-only",
    )
    for declaration in (
        "pub fn vrf_commit_preimage",
        "pub fn v2_vrf_commit_preimage",
        "fn vrf_commit_preimage_fields",
        "pub fn vrf_reveal_preimage",
        "pub fn v2_vrf_reveal_preimage",
        "fn vrf_reveal_preimage_fields",
        "pub fn vrf_commit(",
        "pub fn vrf_reveal(",
    ):
        offset = source.find(declaration)
        _require(offset >= 0, f"missing historical preimage fixture {declaration}")
        prefix = "\n".join(source[:offset].splitlines()[-4:])
        _require("#[cfg(test)]" in prefix, f"legacy preimage returned to production: {declaration}")


def audit(root: Path) -> None:
    """Audit the checkout's complete callable legacy consensus-VRF boundary."""

    for relative in RETIRED_FILES:
        _require(not (root / relative).exists(), f"retired source file returned: {relative}")

    module_source = (root / "crates/iroha_core/src/sumeragi/mod.rs").read_text(encoding="utf-8")
    _require(
        re.search(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+epoch_report\s*;", module_source)
        is None,
        "retired public epoch-report module returned",
    )

    for path in _iter_runtime_sources(root):
        _guard_runtime_source(str(path.relative_to(root)), path.read_text(encoding="utf-8"))

    npos = (root / "crates/iroha_core/src/sumeragi/v2_npos.rs").read_text(encoding="utf-8")
    _guard_npos_tombstone(npos)
    penalties = (root / "crates/iroha_core/src/sumeragi/penalties.rs").read_text(encoding="utf-8")
    _guard_penalty_tombstone(penalties)
    message = (root / "crates/iroha_core/src/sumeragi/message.rs").read_text(encoding="utf-8")
    _guard_block_message_tombstone(message)
    consensus = (root / "crates/iroha_core/src/sumeragi/consensus.rs").read_text(encoding="utf-8")
    _guard_legacy_preimages_are_test_only(consensus)

    wire = (root / "crates/iroha_data_model/src/block/consensus_v2.rs").read_text(
        encoding="utf-8"
    )
    for token in (
        "Retained legacy `NPoS` randomness-commitment wire type",
        "Retained legacy `NPoS` randomness-reveal wire type",
        "Runtime admission rejects the retained legacy VRF variants below",
        "VrfCommit(VrfCommit)",
        "VrfReveal(VrfReveal)",
    ):
        _require(token in wire, f"wire tombstone contract lost {token!r}")


class SumeragiConsensusVrfRetirementSourceTest(unittest.TestCase):
    """Exercise the landed contract and representative in-memory regressions."""

    def test_checkout_has_no_callable_legacy_consensus_vrf_surface(self) -> None:
        audit(ROOT)

    def test_runtime_route_and_handler_mutations_fail_closed(self) -> None:
        for token in (RETIRED_ROUTE_PREFIX, "handle_v1_sumeragi_vrf_epoch"):
            with self.assertRaisesRegex(GuardError, "retired token"):
                _guard_runtime_source("crates/iroha_torii/src/lib.rs", f"const BAD: &str = {token!r};")

    def test_non_test_producer_mutation_fails_closed(self) -> None:
        source = (ROOT / "crates/iroha_core/src/sumeragi/v2_npos.rs").read_text(
            encoding="utf-8"
        )
        body = _function_body(source, "take_outbound", cfg_not_test=True)
        mutated = source.replace(body, " self.active.iter().flat_map(|_| Vec::new()).collect() ", 1)
        with self.assertRaisesRegex(GuardError, "empty vector|legacy state access"):
            _guard_npos_tombstone(mutated)

    def test_non_test_penalty_mutation_fails_closed(self) -> None:
        source = (ROOT / "crates/iroha_core/src/sumeragi/penalties.rs").read_text(
            encoding="utf-8"
        )
        body = _function_body(source, "derive_vrf_penalty_actions", cfg_not_test=True)
        mutated = source.replace(body, " todo!(\"restore reports\") ", 1)
        with self.assertRaisesRegex(GuardError, "penalty derivation must stay empty"):
            _guard_penalty_tombstone(mutated)


if __name__ == "__main__":
    unittest.main()
