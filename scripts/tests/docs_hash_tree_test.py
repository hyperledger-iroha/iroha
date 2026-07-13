"""Regression tests for the deterministic documentation hash-tree helper."""

from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
from pathlib import Path


def test_hash_tree_excludes_its_output_and_is_repeatable(tmp_path: Path) -> None:
    repo = tmp_path / "repo"
    script = repo / "scripts" / "docs" / "hash_tree.sh"
    script.parent.mkdir(parents=True)
    source = Path(__file__).resolve().parents[1] / "docs" / "hash_tree.sh"
    shutil.copyfile(source, script)

    generated = repo / "docs" / "generated"
    generated.mkdir(parents=True)
    artifact = generated / "artifact.md"
    artifact.write_text("canonical\n", encoding="utf-8")
    output = generated / "codegen_hash_tree.json"

    command = ["bash", str(script), str(generated), str(output)]
    subprocess.run(command, cwd=repo, check=True)
    first = output.read_bytes()
    subprocess.run(command, cwd=repo, check=True)
    second = output.read_bytes()

    assert second == first
    payload = json.loads(second)
    assert payload["root"] == "docs/generated"
    assert payload["files"] == [
        {
            "path": "docs/generated/artifact.md",
            "sha256": hashlib.sha256(artifact.read_bytes()).hexdigest(),
            "size": artifact.stat().st_size,
        }
    ]
    assert all(
        entry["path"] != "docs/generated/codegen_hash_tree.json"
        for entry in payload["files"]
    )
