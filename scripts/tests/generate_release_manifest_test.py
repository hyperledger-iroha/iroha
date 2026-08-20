from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path

import pytest


SCRIPT = Path(__file__).resolve().parents[1] / "generate_release_manifest.py"


def sha256(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def write_inventory(
    root: Path,
    files: dict[str, bytes],
    *,
    checksum_rows: list[tuple[str, str]] | None = None,
) -> None:
    for relative, payload in files.items():
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(payload)
        path.chmod(0o644)
    rows = checksum_rows or sorted(
        (relative, sha256(payload)) for relative, payload in files.items()
    )
    (root / "SHA256SUMS").write_text(
        "".join(f"{digest}  {relative}\n" for relative, digest in rows),
        encoding="ascii",
    )


def command(
    root: Path,
    output: Path,
    specs: list[str],
    *,
    epoch: str = "0",
) -> list[str]:
    result = [
        sys.executable,
        str(SCRIPT),
        "--artifacts-dir",
        str(root),
        "--version",
        "1.0.0",
        "--commit",
        "a" * 40,
        "--source-date-epoch",
        epoch,
        "--os-tag",
        "linux",
        "--arch",
        "x86_64",
    ]
    for spec in specs:
        result.extend(["--artifact", spec])
    result.extend(["--output", str(output)])
    return result


def test_release_manifest_regeneration_is_byte_identical(tmp_path: Path) -> None:
    artifacts = tmp_path / "artifacts"
    artifacts.mkdir()
    files = {
        "iroha3-1.0.0-linux.tar.zst": b"deterministic release bytes",
        "iroha3-1.0.0-linux.tar.zst.sha256": b"sidecar\n",
        "iroha3-1.0.0-manifest.json": b'{"builder":"deterministic"}\n',
    }
    write_inventory(artifacts, files)
    specs = [
        "iroha3:x86_64-unknown-linux-gnu:bundle:tar.zst:iroha3-1.0.0-linux.tar.zst",
        "iroha3:x86_64-unknown-linux-gnu:checksum:sha256:iroha3-1.0.0-linux.tar.zst.sha256",
        "iroha3:x86_64-unknown-linux-gnu:builder-manifest:json:iroha3-1.0.0-manifest.json",
    ]
    first = tmp_path / "first.json"
    second = tmp_path / "second.json"
    subprocess.run(command(artifacts, first, specs, epoch="1"), check=True)
    subprocess.run(command(artifacts, second, specs, epoch="1"), check=True)

    assert first.read_bytes() == second.read_bytes()
    manifest = json.loads(first.read_text(encoding="utf-8"))
    assert manifest["schema"] == "iroha.release_manifest"
    assert manifest["schema_version"] == 1
    assert manifest["source_date_epoch"] == 1
    assert manifest["built_at"] == "1970-01-01T00:00:01Z"
    assert [row["path"] for row in manifest["artifacts"]] == sorted(files)
    assert all(row["size"] > 0 for row in manifest["artifacts"])


@pytest.mark.parametrize(
    "body",
    (
        "",
        "not-a-checksum\n",
        f"{'a' * 64} *artifact\n",
        f"{'A' * 64}  artifact\n",
        f"{'a' * 64}  ./artifact\n",
        f"{'a' * 64}  artifact",
        f"{'a' * 64}  artifact\n\n",
    ),
)
def test_release_manifest_rejects_malformed_checksum_inventory(
    tmp_path: Path,
    body: str,
) -> None:
    artifacts = tmp_path / "artifacts"
    artifacts.mkdir()
    artifact = artifacts / "artifact.tar.zst"
    artifact.write_bytes(b"bytes")
    (artifacts / "SHA256SUMS").write_text(body, encoding="ascii")
    result = subprocess.run(
        command(
            artifacts,
            tmp_path / "manifest.json",
            ["iroha3:x86_64-unknown-linux-gnu:bundle:tar.zst:artifact.tar.zst"],
        ),
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode != 0
    assert "release manifest error" in result.stderr


def test_release_manifest_rejects_duplicate_checksum_rows(tmp_path: Path) -> None:
    artifacts = tmp_path / "artifacts"
    artifacts.mkdir()
    payload = b"bytes"
    (artifacts / "artifact.tar.zst").write_bytes(payload)
    digest = sha256(payload)
    (artifacts / "SHA256SUMS").write_text(
        f"{digest}  artifact.tar.zst\n{digest}  artifact.tar.zst\n",
        encoding="ascii",
    )
    result = subprocess.run(
        command(
            artifacts,
            tmp_path / "manifest.json",
            ["iroha3:x86_64-unknown-linux-gnu:bundle:tar.zst:artifact.tar.zst"],
        ),
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode != 0
    assert "duplicate SHA256SUMS path" in result.stderr


@pytest.mark.parametrize("mode", ("missing", "extra"))
def test_release_manifest_rejects_non_closed_file_sets(
    tmp_path: Path,
    mode: str,
) -> None:
    artifacts = tmp_path / "artifacts"
    artifacts.mkdir()
    files = {"artifact.tar.zst": b"bytes"}
    specs = ["iroha3:x86_64-unknown-linux-gnu:bundle:tar.zst:artifact.tar.zst"]
    if mode == "missing":
        specs.append(
            "iroha3:x86_64-unknown-linux-gnu:builder-manifest:json:missing.json"
        )
    else:
        files["stale.json"] = b"stale"
    write_inventory(artifacts, files)
    result = subprocess.run(
        command(artifacts, tmp_path / "manifest.json", specs),
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode != 0
    assert "closed release inventory mismatch" in result.stderr


def test_release_manifest_rejects_checksum_hash_mismatch(tmp_path: Path) -> None:
    artifacts = tmp_path / "artifacts"
    artifacts.mkdir()
    write_inventory(
        artifacts,
        {"artifact.tar.zst": b"bytes"},
        checksum_rows=[("artifact.tar.zst", "0" * 64)],
    )
    result = subprocess.run(
        command(
            artifacts,
            tmp_path / "manifest.json",
            ["iroha3:x86_64-unknown-linux-gnu:bundle:tar.zst:artifact.tar.zst"],
        ),
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode != 0
    assert "digest mismatch" in result.stderr


@pytest.mark.parametrize("kind", ("symlink", "hardlink", "unsafe"))
def test_release_manifest_rejects_unsafe_artifacts(
    tmp_path: Path,
    kind: str,
) -> None:
    artifacts = tmp_path / "artifacts"
    artifacts.mkdir()
    target = artifacts / "target.tar.zst"
    target.write_bytes(b"bytes")
    artifact = artifacts / "artifact.tar.zst"
    if kind == "symlink":
        artifact.symlink_to(target.name)
    elif kind == "hardlink":
        os.link(target, artifact)
    else:
        target.rename(artifact)
        artifact.chmod(0o666)
    digest = sha256(b"bytes")
    (artifacts / "SHA256SUMS").write_text(
        f"{digest}  artifact.tar.zst\n",
        encoding="ascii",
    )
    result = subprocess.run(
        command(
            artifacts,
            tmp_path / "manifest.json",
            ["iroha3:x86_64-unknown-linux-gnu:bundle:tar.zst:artifact.tar.zst"],
        ),
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode != 0
    assert not (tmp_path / "manifest.json").exists()


@pytest.mark.parametrize(
    "spec",
    (
        "iroha3:x86_64-unknown-linux-gnu:bundle:tar.zst:../artifact.tar.zst",
        "iroha3:x86_64-unknown-linux-gnu:bundle:tar.zst:/artifact.tar.zst",
        r"iroha3:x86_64-unknown-linux-gnu:bundle:tar.zst:dir\\artifact.tar.zst",
        "unknown:x86_64-unknown-linux-gnu:bundle:tar.zst:artifact.tar.zst",
        "iroha3:x86_64-unknown-linux-gnu:bundle:json:artifact.tar.zst",
    ),
)
def test_release_manifest_rejects_invalid_artifact_descriptors(
    tmp_path: Path,
    spec: str,
) -> None:
    artifacts = tmp_path / "artifacts"
    artifacts.mkdir()
    write_inventory(artifacts, {"artifact.tar.zst": b"bytes"})
    result = subprocess.run(
        command(artifacts, tmp_path / "manifest.json", [spec]),
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode != 0


def test_release_manifest_output_is_exclusive(tmp_path: Path) -> None:
    artifacts = tmp_path / "artifacts"
    artifacts.mkdir()
    write_inventory(artifacts, {"artifact.tar.zst": b"bytes"})
    output = tmp_path / "manifest.json"
    output.write_text("preserve", encoding="utf-8")
    result = subprocess.run(
        command(
            artifacts,
            output,
            ["iroha3:x86_64-unknown-linux-gnu:bundle:tar.zst:artifact.tar.zst"],
        ),
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode != 0
    assert output.read_text(encoding="utf-8") == "preserve"


def test_release_manifest_rejects_output_inside_artifact_root(
    tmp_path: Path,
) -> None:
    artifacts = tmp_path / "artifacts"
    artifacts.mkdir()
    write_inventory(artifacts, {"artifact.tar.zst": b"bytes"})
    output = artifacts / "manifest.json"
    result = subprocess.run(
        command(
            artifacts,
            output,
            ["iroha3:x86_64-unknown-linux-gnu:bundle:tar.zst:artifact.tar.zst"],
        ),
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode != 0
    assert "output must be outside the closed artifact root" in result.stderr
    assert not output.exists()


def test_release_manifest_rejects_output_parent_aliasing_artifact_root(
    tmp_path: Path,
) -> None:
    artifacts = tmp_path / "artifacts"
    artifacts.mkdir()
    write_inventory(artifacts, {"artifact.tar.zst": b"bytes"})
    alias = tmp_path / "alias"
    alias.symlink_to(artifacts, target_is_directory=True)
    output = alias / "manifest.json"
    result = subprocess.run(
        command(
            artifacts,
            output,
            ["iroha3:x86_64-unknown-linux-gnu:bundle:tar.zst:artifact.tar.zst"],
        ),
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode != 0
    assert "aliases the artifact root" in result.stderr
    assert not (artifacts / "manifest.json").exists()


@pytest.mark.parametrize("commit", ("abc1234", "A" * 40, "a" * 39, "a" * 41))
def test_release_manifest_rejects_non_full_canonical_commit(
    tmp_path: Path,
    commit: str,
) -> None:
    artifacts = tmp_path / "artifacts"
    artifacts.mkdir()
    write_inventory(artifacts, {"artifact.tar.zst": b"bytes"})
    args = command(
        artifacts,
        tmp_path / "manifest.json",
        ["iroha3:x86_64-unknown-linux-gnu:bundle:tar.zst:artifact.tar.zst"],
    )
    args[args.index("--commit") + 1] = commit
    result = subprocess.run(
        args,
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode != 0
    assert "full 40- or 64-hex identifier" in result.stderr
    assert not (tmp_path / "manifest.json").exists()
