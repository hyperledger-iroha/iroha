from __future__ import annotations

import hashlib
import subprocess
import sys
from pathlib import Path


def test_release_manifest_regeneration_is_byte_identical(tmp_path: Path) -> None:
    artifacts_dir = tmp_path / "artifacts"
    artifacts_dir.mkdir()
    artifact = artifacts_dir / "iroha2-1.0.0-linux.tar.zst"
    artifact.write_bytes(b"deterministic release bytes")
    digest = hashlib.sha256(artifact.read_bytes()).hexdigest()
    (artifacts_dir / "SHA256SUMS").write_text(
        f"{digest}  {artifact.name}\n",
        encoding="utf-8",
    )
    script = Path(__file__).resolve().parents[1] / "generate_release_manifest.py"
    first = tmp_path / "first.json"
    second = tmp_path / "second.json"
    common = [
        sys.executable,
        str(script),
        "--artifacts-dir",
        str(artifacts_dir),
        "--version",
        "1.0.0",
        "--commit",
        "abcdef0",
        "--built-at",
        "2026-07-24T00:00:00Z",
        "--os-tag",
        "linux",
        "--arch",
        "x86_64",
    ]
    subprocess.run(common + ["--output", str(first)], check=True)
    subprocess.run(common + ["--output", str(second)], check=True)

    assert first.read_bytes() == second.read_bytes()
    assert first.read_bytes().startswith(b'{\n  "arch": "x86_64"')
