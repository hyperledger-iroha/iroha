#!/usr/bin/env python3
"""Verify retained focused SNS evidence and its explicitly scoped input identity."""

import hashlib
import json
from pathlib import Path
import re


def digest(path):
    """Hash an existing input without modifying it."""
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    """Check retained evidence without compiling or promoting qualification."""
    evidence = Path(__file__).resolve().parent
    root = evidence.parents[2]
    receipt = json.loads((evidence / "receipt.json").read_text())
    manifest = {}
    for line in (evidence / "SHA256SUMS").read_text().splitlines():
        expected, name = line.split("  ", 1)
        assert name not in manifest, f"duplicate artifact: {name}"
        manifest[name] = expected
        assert digest(evidence / name) == expected, name
    actual = {
        str(path.relative_to(evidence))
        for path in evidence.rglob("*")
        if path.is_file()
        and path.name != "SHA256SUMS"
        and "__pycache__" not in path.parts
    }
    assert actual == set(manifest), "artifact manifest differs"
    assert receipt["sources"] == receipt["completion_owned_source_sha256"]
    assert receipt["schema_sources"] == receipt["completion_registry_source_sha256"]
    for name, expected in {
        **receipt["sources"],
        **receipt["schema_sources"],
        "Cargo.lock": receipt["completion_lock_sha256"],
    }.items():
        assert digest(root / name) == expected, name
    total = 0
    for group in ("schema_test", "current_cut", "snapshot", "state_certificate", "torii_sns_lease"):
        result = receipt[group]
        log = evidence / result["log"]
        assert digest(log) == result["log_sha256"], group
        match = re.search(
            r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;",
            log.read_text(),
        )
        assert match, group
        counts = tuple(map(int, match.groups()))
        assert counts == (result["passed"], 0, 0) and counts[0] > 0, group
        total += counts[0]
    assert total == 36 == receipt["total_focused_results"]["passed"]
    assert receipt["complete_transitive_build_custody"] is False
    assert receipt["whole_node_qualified"] is False
    print(json.dumps({
        "artifacts": len(manifest),
        "focused_tests_passed": total,
        "sns_sources": len(receipt["sources"]),
        "registry_sources": len(receipt["schema_sources"]),
        "scoped_inputs_match": True,
        "complete_transitive_build_custody": False,
        "whole_node_qualified": False,
    }))


if __name__ == "__main__":
    main()
