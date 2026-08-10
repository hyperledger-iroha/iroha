"""Compose split Rust and Python sources for SoraFS rollout contract scans."""

from __future__ import annotations

import ast
from pathlib import Path


_SOURCE_COMPONENTS = {
    "check_sorafs_ai_prescreen_rollout_evidence_test.py": (
        "sorafs_ai_prescreen_live_evidence_cases.py",
    ),
    "check_sorafs_production_readiness_test.py": (
        "sorafs_production_foundational_cases.py",
    ),
    "check_sorafs_rollout_gate_contract_test.py": (
        "sorafs_rollout_gate_contract_inventory.py",
    ),
    # Mirror the Rust test module's three include! components for static scans.
    "provider_ingest_runtime.rs": (
        "provider_ingest_runtime/tests/support.rs",
        "provider_ingest_runtime/tests/capture_source.rs",
        "provider_ingest_runtime/tests/runtime.rs",
    ),
    "actual.rs": ("actual/sorafs_pop_credentials.rs",),
}


def read_source(path: Path) -> str:
    """Read a source plus the same-scope components it includes."""
    source = path.read_text(encoding="utf-8")
    for component_name in _SOURCE_COMPONENTS.get(path.name, ()):
        source += "\n" + (path.parent / component_name).read_text(encoding="utf-8")
    return source


def function_source(path: Path, function_name: str) -> str:
    """Return one top-level Python function from a composed source."""
    source_text = read_source(path)
    module = ast.parse(source_text)
    for node in module.body:
        if isinstance(node, ast.FunctionDef) and node.name == function_name:
            return ast.get_source_segment(source_text, node) or ""
    return ""


def governance_service_source(path: Path) -> str:
    """Read the governance service and its split same-scope test support."""
    return "\n".join(
        (
            read_source(path),
            read_source(path.parent / "governance_service/tests/support.rs"),
            read_source(path.parent / "governance_service/tests/restart_and_live_kubo.rs"),
        )
    )


def governance_source(path: Path) -> str:
    """Read governance state plus its split same-scope test support."""
    return read_source(path) + read_source(path.parent / "governance/tests/support.rs")
