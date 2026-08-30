"""Source-level checks for the Python governance README example."""

from __future__ import annotations

import ast
from pathlib import Path


def governance_readme_example() -> str:
    """Return the governance Python fence from the package README."""

    readme = (Path(__file__).resolve().parents[1] / "README.md").read_text(
        encoding="utf-8"
    )
    section = readme.split("## Governance helpers", 1)[1].split(
        "## Runtime upgrades and ABI helpers", 1
    )[0]
    return section.split("```python", 1)[1].split("```", 1)[0]


def test_governance_readme_protected_reads_use_canonical_auth() -> None:
    source = governance_readme_example()
    tree = ast.parse(source)
    imported = {
        alias.name
        for node in ast.walk(tree)
        if isinstance(node, ast.ImportFrom)
        for alias in node.names
    }
    assert "ToriiCanonicalRequestAuth" in imported

    constructors = [
        node.value
        for node in tree.body
        if isinstance(node, ast.Assign)
        and any(
            isinstance(target, ast.Name) and target.id == "canonical_auth"
            for target in node.targets
        )
    ]
    assert len(constructors) == 1
    constructor = constructors[0]
    assert isinstance(constructor, ast.Call)
    assert isinstance(constructor.func, ast.Name)
    assert constructor.func.id == "ToriiCanonicalRequestAuth"
    assert {keyword.arg for keyword in constructor.keywords} == {
        "network_id",
        "account_id",
        "signer",
    }

    expected_reads = {
        "get_protected_namespaces",
        "get_governance_contract_typed",
        "get_governance_proposal_typed",
        "get_governance_referendum_typed",
        "get_governance_tally_typed",
        "get_governance_locks_typed",
        "get_governance_unlock_stats_typed",
    }
    calls = {
        node.func.attr: node
        for node in ast.walk(tree)
        if isinstance(node, ast.Call)
        and isinstance(node.func, ast.Attribute)
        and node.func.attr in expected_reads
    }
    assert calls.keys() == expected_reads
    for name, call in calls.items():
        auth_keywords = [
            keyword for keyword in call.keywords if keyword.arg == "canonical_auth"
        ]
        assert len(auth_keywords) == 1, name
        assert isinstance(auth_keywords[0].value, ast.Name), name
        assert auth_keywords[0].value.id == "canonical_auth", name
