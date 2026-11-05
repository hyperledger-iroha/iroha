"""Read-only reproduction of the takeover review's retained-evidence checks.

This checks source custody and the symbolic report, not runtime qualification.
Run from any directory with Python 3. It performs no compilation or network I/O.
"""

from __future__ import annotations

import contextlib
import gzip
import hashlib
import importlib.util
import io
import json
from pathlib import Path


def sha(path: Path) -> str:
    """Return the digest of one retained or current input."""
    return hashlib.sha256(path.read_bytes()).hexdigest()


def review() -> dict:
    """Check the complete retained input chain without upgrading its scope."""
    evidence = Path(__file__).resolve().parent
    root = evidence.parents[2]
    checker = root / "scripts/check_parliament_ballot_resource_accounting.py"
    tests = root / "scripts/tests/check_parliament_ballot_resource_accounting_test.py"
    spec = importlib.util.spec_from_file_location("reviewed_accounting", checker)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)

    provider = json.loads((evidence / "provider-layout.json").read_text())
    producer_path = evidence / provider["producer_receipt"]
    assert sha(producer_path) == provider["producer_receipt_sha256"]
    producer = json.loads(producer_path.read_text())
    assert sha(evidence / "producer-metadata.json") == producer["dependency_metadata_sha256"]
    assert producer["source_stable"] and producer["compile_only"]
    assert not producer["profile_qualified"]
    assert len(producer["source_inputs"]) == 683
    for name, expected in producer["source_inputs"].items():
        assert sha(Path(name)) == expected, name
    layouts = {(row["target"], row["mode"]): row for row in provider["targets"]}
    assert len(layouts) == len(provider["targets"]) == len(producer["commands"]) == 8
    commands = set()
    for row in producer["commands"]:
        key = row["target"], row["mode"]
        assert key not in commands
        commands.add(key)
        assert row["exit_code"] == 0 and not row["source_changes"]
        assert row["ir_sha256"] == layouts[key]["llvm_ir_sha256"]
    assert commands == set(layouts)
    for name, expected in module.ALLOC_SOURCE_SHA256.items():
        archive = evidence / "rust-alloc" / (name.replace("/", "__") + ".gz")
        assert hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest() == expected

    normal, strict = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(normal):
        normal_exit = module.main([])
    with contextlib.redirect_stdout(strict):
        strict_exit = module.main(["--require-complete"])
    assert (normal_exit, strict_exit) == (0, 1)
    assert normal.getvalue() == strict.getvalue() == (evidence / "report.json").read_text()
    report = json.loads(normal.getvalue())
    assert not report["profile_qualified"] and not report["complete_certificate"]
    assert len(report["production_terms_remaining"]) == 5
    historical = json.loads((root / "docs/validation/parliament_ballot_b6/resource_accounting.json").read_text())
    assert report["cases"] == historical["cases"]
    assert report["standalone_symbolic_bounds"]["cases"] == historical["standalone_symbolic_bounds"]["cases"]

    # Do not include this report or SHA256SUMS in its own input identity.
    inputs = {str(path.relative_to(root)): sha(path) for path in (checker, tests, Path(__file__).resolve())}
    for path in sorted(evidence.rglob("*")):
        if path.is_file() and path.name not in {"SHA256SUMS", "independent-review.json"} and "__pycache__" not in path.parts:
            inputs[str(path.relative_to(root))] = sha(path)
    return {
        "scope": "Independent source/evidence review of the unchanged symbolic resource formulas and refreshed compiler layout bindings; no device execution or production qualification.",
        "input_sha256": inputs,
        "source_inventory_sha256": report["standalone_symbolic_bounds"]["sources"]["aggregate_sha256"],
        "producer_inputs_checked": len(producer["source_inputs"]),
        "compiler_target_mode_results_checked": len(commands),
        "rust_alloc_archives_checked": len(module.ALLOC_SOURCE_SHA256),
        "report_byte_identical": True,
        "historical_numeric_cases_identical": True,
        "normal_exit": normal_exit,
        "require_complete_exit": strict_exit,
        "complete_certificate": False,
        "profile_qualified": False,
    }


if __name__ == "__main__":
    print(json.dumps(review(), indent=2, sort_keys=True))
